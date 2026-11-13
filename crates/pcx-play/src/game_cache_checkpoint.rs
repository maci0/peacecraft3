//! The world's unsaved cache memory and errors; external profile storage is rebound, never rolled back.
use super::{Budget, Entry, ObjectHost, Profile, State};
use pcx_script::checkpoint::{Reader, Writer};
use std::sync::Arc;

fn fields(budget: Budget) -> [usize; 5] {
    let Budget {
        caches,
        entries,
        text_bytes,
        storage_bytes,
        handle_bytes,
    } = budget;
    [caches, entries, text_bytes, storage_bytes, handle_bytes]
}
fn budget_out(out: &mut Writer, budget: Budget) -> Result<(), String> {
    for count in fields(budget) {
        out.u64(u64::try_from(count).map_err(|_| "cache budget overflow")?)?;
    }
    Ok(())
}
fn budget_in(input: &mut Reader<'_>, current: Budget) -> Result<Budget, String> {
    let mut counts = [0; 5];
    for (count, limit) in counts.iter_mut().zip(fields(current)) {
        *count = usize::try_from(input.u64()?).map_err(|_| "checkpoint cache budget overflow")?;
        if *count > limit {
            return Err("checkpoint cache budget exceeds its candidate policy".into());
        }
    }
    let [caches, entries, text_bytes, storage_bytes, handle_bytes] = counts;
    Ok(Budget {
        caches,
        entries,
        text_bytes,
        storage_bytes,
        handle_bytes,
    })
}
fn rows(budget: Budget) -> Result<usize, String> {
    budget
        .caches
        .checked_add(budget.entries)
        .ok_or("checkpoint cache row budget overflow".into())
}

impl ObjectHost {
    fn checkpoint_cache_handle_bytes(&self) -> Result<usize, String> {
        self.inner
            .arena
            .iter()
            .try_fold(0usize, |size, entry| match entry {
                Entry::GameCache { filename, .. } => size
                    .checked_add(filename.len())
                    .ok_or("checkpoint cache handle bytes overflow".into()),
                _ => Ok(size),
            })
    }
    pub fn write_game_cache_checkpoint(&self, out: &mut Writer) -> Result<(), String> {
        out.boolean(self.game_caches.is_some())?;
        let Some(State {
            profile,
            storage: _,
            budget,
            handle_bytes,
            storage_error,
        }) = &self.game_caches
        else {
            return Ok(());
        };
        if *handle_bytes != self.checkpoint_cache_handle_bytes()?
            || *handle_bytes > budget.handle_bytes
        {
            return Err("checkpoint cache handle accounting differs from its arena".into());
        }
        budget_out(out, *budget)?;
        out.u64(u64::try_from(*handle_bytes).map_err(|_| "cache handle bytes overflow")?)?;
        out.boolean(storage_error.is_some())?;
        if let Some(error) = storage_error {
            out.text(error)?;
        }
        let bytes = profile.encode(*budget)?;
        // The existing profile decoder separately enforces its typed row and content limits.
        out.items(rows(*budget)?)?;
        out.owned(bytes.len())?;
        out.blob(&bytes)
    }
    /// The full candidate arena must already contain all allocated cache identities,
    /// including flushed handles. No storage read or write occurs in this section.
    pub fn restore_game_cache_checkpoint(&mut self, input: &mut Reader<'_>) -> Result<(), String> {
        let admitted = input.boolean()?;
        if admitted != self.game_caches.is_some() {
            return Err("checkpoint cache admission differs from its candidate world".into());
        }
        let Some(current) = &self.game_caches else {
            return Ok(());
        };
        let budget = budget_in(input, current.budget)?;
        let handle_bytes =
            usize::try_from(input.u64()?).map_err(|_| "checkpoint cache handle byte overflow")?;
        if handle_bytes > budget.handle_bytes
            || handle_bytes != self.checkpoint_cache_handle_bytes()?
        {
            return Err(
                "checkpoint cache handle accounting differs from its candidate arena".into(),
            );
        }
        let storage_error = if input.boolean()? {
            Some(input.text()?)
        } else {
            None
        };
        input.items(rows(budget)?)?;
        let bytes = input.blob()?;
        input.owned(bytes.len())?;
        let profile = Profile::decode(&bytes, budget)?;
        self.game_caches = Some(State {
            profile,
            storage: Arc::clone(&current.storage),
            budget,
            handle_bytes,
            storage_error,
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        game_cache::Storage,
        playercolor_host::{EnumHost, Limits},
    };
    use pcx_script::{Host, Prelude, Value, checkpoint::Budget as WireBudget};
    use std::sync::{
        Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };

    const SOURCE: &str = "\
type agent extends handle
type timer extends agent
type gamecache extends agent
native CreateTimer takes nothing returns timer
native DestroyTimer takes timer t returns nothing
native InitGameCache takes string file returns gamecache
native StoreInteger takes gamecache c,string mission,string key,integer value returns nothing
native GetStoredInteger takes gamecache c,string mission,string key returns integer
native SaveGameCache takes gamecache c returns boolean
native StoreString takes gamecache c,string mission,string key,string value returns boolean
native GetStoredString takes gamecache c,string mission,string key returns string
";
    const BUDGET: WireBudget = WireBudget {
        bytes: 1 << 20,
        items: 16384,
        depth: 16,
    };
    #[derive(Default)]
    struct Memory {
        bytes: Mutex<Option<Vec<u8>>>,
        reads: AtomicUsize,
        writes: AtomicUsize,
        fail: AtomicBool,
    }
    impl Storage for Memory {
        fn load(&self, limit: usize) -> Result<Option<Vec<u8>>, String> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            let bytes = self.bytes.lock().unwrap();
            if bytes.as_ref().is_some_and(|bytes| bytes.len() > limit) {
                return Err("synthetic read limit".into());
            }
            Ok(bytes.clone())
        }
        fn save(&self, bytes: &[u8]) -> Result<(), String> {
            self.writes.fetch_add(1, Ordering::SeqCst);
            if self.fail.load(Ordering::SeqCst) {
                return Err("synthetic write refusal".into());
            }
            *self.bytes.lock().unwrap() = Some(bytes.to_vec());
            Ok(())
        }
    }
    fn policy() -> Budget {
        Budget {
            caches: 5,
            entries: 17,
            text_bytes: 4096,
            storage_bytes: 8192,
            handle_bytes: 256,
        }
    }
    fn open(storage: Arc<Memory>, budget: Budget) -> (ObjectHost, Value) {
        let common = pcx_script::parse_script(SOURCE).unwrap();
        let map = pcx_script::parse_script("").unwrap();
        let program = Arc::new(
            pcx_script::resolve_with(&map, &[Prelude::new("synthetic cache source", &common)])
                .unwrap(),
        );
        let enums = EnumHost::checked_selected(
            program,
            "synthetic cache root",
            &[],
            Limits {
                max_handles: 16,
                admission_steps: 128,
                expression_depth: 16,
                expression_work: 128,
            },
        )
        .unwrap();
        let mut host = ObjectHost::checked(enums).unwrap();
        host.admit_game_caches(storage, budget).unwrap();
        let cache = host
            .native("InitGameCache", &[Value::Str("route".into())])
            .unwrap();
        (host, cache)
    }
    fn key(cache: Value) -> [Value; 3] {
        [
            cache,
            Value::Str("mission".into()),
            Value::Str("entry".into()),
        ]
    }
    fn put(host: &mut ObjectHost, cache: Value, value: i32) {
        let mut args = key(cache).to_vec();
        args.push(Value::Int(value));
        host.native("StoreInteger", &args).unwrap();
    }
    fn encode(host: &ObjectHost) -> Vec<u8> {
        let mut out = Writer::new(BUDGET).unwrap();
        host.write_game_cache_checkpoint(&mut out).unwrap();
        out.finish()
    }
    fn decode(host: &mut ObjectHost, bytes: &[u8]) -> Result<(), String> {
        let mut input = Reader::new(bytes, BUDGET)?;
        host.restore_game_cache_checkpoint(&mut input)?;
        input.finish()
    }
    #[test]
    fn world_cache_section_restores_unsaved_memory_and_error_without_reading_or_replacing_storage()
    {
        let disk = Arc::new(Memory::default());
        let (mut live, cache) = open(Arc::clone(&disk), policy());
        put(&mut live, cache.clone(), 37);
        assert_eq!(
            live.native("SaveGameCache", std::slice::from_ref(&cache))
                .unwrap(),
            Value::Bool(true)
        );
        let saved_disk = disk.bytes.lock().unwrap().clone();
        put(&mut live, cache.clone(), 93);
        disk.fail.store(true, Ordering::SeqCst);
        assert_eq!(
            live.native("SaveGameCache", &[cache]).unwrap(),
            Value::Bool(false)
        );
        assert_eq!(
            live.game_cache_storage_error(),
            Some("synthetic write refusal")
        );
        let reads = disk.reads.load(Ordering::SeqCst);
        let writes = disk.writes.load(Ordering::SeqCst);
        let bytes = encode(&live);
        assert_eq!(disk.reads.load(Ordering::SeqCst), reads);
        assert_eq!(disk.writes.load(Ordering::SeqCst), writes);
        let backend = Arc::new(Memory::default());
        *backend.bytes.lock().unwrap() = saved_disk.clone();
        let (mut candidate, cache) = open(Arc::clone(&backend), policy());
        assert_eq!(
            candidate
                .native("GetStoredInteger", &key(cache.clone()))
                .unwrap(),
            Value::Int(37)
        );
        let reads = backend.reads.load(Ordering::SeqCst);
        let writes = backend.writes.load(Ordering::SeqCst);
        decode(&mut candidate, &bytes).unwrap();
        assert_eq!(
            candidate
                .native("GetStoredInteger", &key(cache.clone()))
                .unwrap(),
            Value::Int(93)
        );
        assert_eq!(
            candidate.game_cache_storage_error(),
            Some("synthetic write refusal")
        );
        assert_eq!(backend.reads.load(Ordering::SeqCst), reads);
        assert_eq!(backend.writes.load(Ordering::SeqCst), writes);
        assert_eq!(*backend.bytes.lock().unwrap(), saved_disk);
        assert_eq!(encode(&candidate), bytes);
        put(&mut candidate, cache.clone(), 101);
        assert_eq!(
            candidate.native("SaveGameCache", &[cache]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(candidate.game_cache_storage_error(), None);
        assert_eq!(backend.writes.load(Ordering::SeqCst), writes + 1);
        assert_ne!(*backend.bytes.lock().unwrap(), saved_disk);
        assert_eq!(
            *disk.bytes.lock().unwrap(),
            saved_disk,
            "restore rebinds the candidate backend"
        );
    }
    #[test]
    fn world_cache_section_keeps_non_utf8_stored_string_bytes_through_profile_codec() {
        let (mut live, cache) = open(Arc::new(Memory::default()), policy());
        let value = Value::Str(vec![0xc3, 0xff, 0xa9].into());
        let mut args = key(cache).to_vec();
        args.push(value.clone());
        assert_eq!(
            live.native("StoreString", &args).unwrap(),
            Value::Bool(true)
        );
        let bytes = encode(&live);
        let (mut loaded, cache) = open(Arc::new(Memory::default()), policy());
        decode(&mut loaded, &bytes).unwrap();
        assert_eq!(
            loaded.native("GetStoredString", &key(cache)).unwrap(),
            value
        );
        assert_eq!(encode(&loaded), bytes);
    }
    #[test]
    fn world_cache_section_refuses_truncation_policy_and_arena_counter_mismatch_atomically() {
        let (mut live, cache) = open(Arc::new(Memory::default()), policy());
        put(&mut live, cache, 73);
        let bytes = encode(&live);
        let (mut candidate, cache) = open(Arc::new(Memory::default()), policy());
        put(&mut candidate, cache, 19);
        let before = encode(&candidate);
        for end in 0..bytes.len() {
            assert!(
                decode(&mut candidate, &bytes[..end]).is_err(),
                "length {end}"
            );
            assert_eq!(encode(&candidate), before);
        }
        let mut smaller = policy();
        smaller.entries -= 1;
        let (mut restricted, _) = open(Arc::new(Memory::default()), smaller);
        let before_restricted = encode(&restricted);
        assert!(
            decode(&mut restricted, &bytes)
                .unwrap_err()
                .contains("policy")
        );
        assert_eq!(encode(&restricted), before_restricted);
        candidate
            .native("InitGameCache", &[Value::Str("second".into())])
            .unwrap();
        let before = encode(&candidate);
        assert!(
            decode(&mut candidate, &bytes)
                .unwrap_err()
                .contains("accounting")
        );
        assert_eq!(encode(&candidate), before);
        assert_eq!(encode(&live), bytes);
    }
}

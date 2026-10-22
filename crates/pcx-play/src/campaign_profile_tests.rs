use super::*;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Default)]
struct Memory {
    bytes: Mutex<Option<Vec<u8>>>,
    fail: AtomicBool,
}
impl Storage for Memory {
    fn load(&self, limit: usize) -> Result<Option<Vec<u8>>, String> {
        let bytes = self.bytes.lock().unwrap();
        if bytes.as_ref().is_some_and(|b| b.len() > limit) {
            return Err("read limit".into());
        }
        Ok(bytes.clone())
    }
    fn save(&self, bytes: &[u8]) -> Result<(), String> {
        if self.fail.load(Ordering::Relaxed) {
            return Err("injected save failure".into());
        }
        *self.bytes.lock().unwrap() = Some(bytes.to_vec());
        Ok(())
    }
}

fn cache(name: Option<&str>) -> Vec<u8> {
    let mut p = crate::game_cache::Profile::default();
    if let Some(name) = name {
        p.open(name, BUDGET).unwrap();
        p.put(
            name,
            "fixture".into(),
            "hero-value".into(),
            crate::game_cache::Stored::Integer(37),
            BUDGET,
        )
        .unwrap();
    }
    p.encode(BUDGET).unwrap()
}
fn progress() -> Vec<u8> {
    let mut p = crate::campaign_progress::Progress::default();
    p.set(3, 7, false).unwrap();
    p.encode().unwrap()
}

#[test]
fn legacy_cache_bytes_and_progress_survive_independent_section_writes() {
    let physical = Arc::new(Memory::default());
    let (caches, missions) = split(physical.clone());
    let original = cache(None);
    caches.save(&original).unwrap();
    assert_eq!(
        physical.load(BUDGET.storage_bytes).unwrap(),
        Some(original.clone())
    );
    assert_eq!(missions.load(1000).unwrap(), None);
    let explicit = progress();
    missions.save(&explicit).unwrap();
    assert_eq!(caches.load(1000).unwrap(), Some(original));
    let changed = cache(Some("synthetic"));
    caches.save(&changed).unwrap();
    assert_eq!(missions.load(1000).unwrap(), Some(explicit));
    assert_eq!(caches.load(1000).unwrap(), Some(changed));
    let raw = physical.load(BUDGET.storage_bytes).unwrap().unwrap();
    assert!(raw.starts_with(MAGIC));
    super::super::SessionStorage::new(Some(&raw)).unwrap();
    assert!(caches.load(0).is_err());
}

#[test]
fn failed_saves_and_malformed_sections_preserve_both_sections() {
    let physical = Arc::new(Memory::default());
    let (caches, missions) = split(physical.clone());
    caches.save(&cache(None)).unwrap();
    missions.save(&progress()).unwrap();
    let before = physical.load(BUDGET.storage_bytes).unwrap();
    physical.fail.store(true, Ordering::Relaxed);
    assert!(caches.save(&cache(Some("changed"))).is_err());
    assert!(
        missions
            .save(
                &crate::campaign_progress::Progress::default()
                    .encode()
                    .unwrap()
            )
            .is_err()
    );
    assert_eq!(physical.load(BUDGET.storage_bytes).unwrap(), before);
    physical.fail.store(false, Ordering::Relaxed);
    assert!(caches.save(b"not a cache").is_err());
    assert!(missions.save(b"not progress").is_err());
    assert_eq!(physical.load(BUDGET.storage_bytes).unwrap(), before);
}

#[test]
fn envelope_lengths_and_truncation_are_checked_before_indexing() {
    let c = cache(None);
    let p = progress();
    let bytes = Envelope {
        cache: Some(&c),
        progress: Some(&p),
    }
    .encode()
    .unwrap();
    for end in 0..bytes.len() {
        assert!(validate(&bytes[..end]).is_err(), "prefix {end}");
    }
    let mut oversized = bytes.clone();
    oversized[MAGIC.len()..MAGIC.len() + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(validate(&oversized).is_err());
    let mut extra = bytes;
    extra.push(0);
    assert!(validate(&extra).is_err());
    assert!(validate(b"invalid original or recreation profile").is_err());
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn concurrent_sections_do_not_overwrite_one_another() {
    let physical = Arc::new(Memory::default());
    let (caches, missions) = split(physical.clone());
    std::thread::scope(|scope| {
        scope.spawn(|| caches.save(&cache(Some("concurrent"))).unwrap());
        scope.spawn(|| missions.save(&progress()).unwrap());
    });
    assert_eq!(caches.load(1000).unwrap(), Some(cache(Some("concurrent"))));
    assert_eq!(missions.load(1000).unwrap(), Some(progress()));
}

//! Local PC3 campaign profile storage, separate from original W3V files.
use crate::{
    game_cache::{Budget, Storage},
    map_startup_host::MapStartupHost,
};
use std::sync::{Arc, Mutex};

#[path = "campaign_profile.rs"]
mod profile;

const BUDGET: Budget = Budget {
    caches: 1024,
    entries: 16384,
    text_bytes: 4 << 20,
    storage_bytes: 8 << 20,
    handle_bytes: 1 << 20,
};

pub fn attach(
    host: &mut MapStartupHost,
    storage: Arc<dyn Storage>,
) -> Result<(), crate::playercolor_host::Error> {
    let (cache, progress) = profile::split(storage);
    if host
        .context()
        .scripts
        .program()
        .natives()
        .get("InitGameCache")
        .is_some()
    {
        host.objects_mut().admit_game_caches(cache, BUDGET)?;
    }
    host.objects_mut().admit_campaign_progress_storage(progress)
}

/// Application persistence; headless sessions supply their own storage instead.
pub fn local_storage() -> Arc<dyn Storage> {
    Arc::new(LocalStorage)
}

/// Each headless session owns its snapshot and saves. No machine profile is read or written.
pub(crate) struct SessionStorage(Mutex<Option<Vec<u8>>>);

impl SessionStorage {
    pub fn new(bytes: Option<&[u8]>) -> Result<Self, String> {
        if let Some(bytes) = bytes {
            profile::validate(bytes)?;
        }
        Ok(Self(Mutex::new(bytes.map(Self::copy).transpose()?)))
    }

    fn copy(bytes: &[u8]) -> Result<Vec<u8>, String> {
        if bytes.len() > BUDGET.storage_bytes {
            return Err("campaign profile exceeds byte policy".into());
        }
        let mut copy = Vec::new();
        copy.try_reserve_exact(bytes.len())
            .map_err(|e| e.to_string())?;
        copy.extend_from_slice(bytes);
        Ok(copy)
    }

    pub fn snapshot(&self) -> Result<Option<Vec<u8>>, String> {
        self.load(BUDGET.storage_bytes)
    }
}

impl Storage for SessionStorage {
    fn load(&self, limit: usize) -> Result<Option<Vec<u8>>, String> {
        let bytes = self.0.lock().map_err(|e| e.to_string())?;
        bytes
            .as_deref()
            .map(|bytes| {
                if bytes.len() > limit {
                    return Err("campaign profile exceeds read byte policy".into());
                }
                Self::copy(bytes)
            })
            .transpose()
    }

    fn save(&self, bytes: &[u8]) -> Result<(), String> {
        let copy = Self::copy(bytes)?;
        *self.0.lock().map_err(|e| e.to_string())? = Some(copy);
        Ok(())
    }
}

struct LocalStorage;

#[cfg(not(target_arch = "wasm32"))]
fn path() -> Result<std::path::PathBuf, String> {
    let base = if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(std::path::PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME")
            .map(|home| std::path::PathBuf::from(home).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(|home| std::path::PathBuf::from(home).join(".local/share"))
            })
    };
    base.map(|base| base.join("peacecraft3/campaign-cache.bin"))
        .ok_or_else(|| "campaign profile directory is unavailable".into())
}

#[cfg(not(target_arch = "wasm32"))]
impl Storage for LocalStorage {
    fn load(&self, limit: usize) -> Result<Option<Vec<u8>>, String> {
        read(&path()?, limit)
    }
    fn save(&self, bytes: &[u8]) -> Result<(), String> {
        write(&path()?, bytes)
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn browser_campaign_cache_storage_gate() -> u32 {
    let storage = LocalStorage;
    assert_eq!(storage.load(8).unwrap(), None);
    storage.save(&[0, 255, 128, 1]).unwrap();
    assert_eq!(storage.load(4).unwrap(), Some(vec![0, 255, 128, 1]));
    assert!(storage.load(3).is_err());
    storage.save(&[]).unwrap();
    assert_eq!(storage.load(0).unwrap(), Some(vec![]));
    4
}

#[cfg(not(target_arch = "wasm32"))]
fn read(path: &std::path::Path, limit: usize) -> Result<Option<Vec<u8>>, String> {
    use std::io::Read;
    let mut file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    if file.metadata().map_err(|e| e.to_string())?.len() > limit as u64 {
        return Err("saved campaign cache exceeds byte policy".into());
    }
    let cap = limit
        .checked_add(1)
        .ok_or("campaign cache read limit overflow")?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(cap).map_err(|e| e.to_string())?;
    file.by_ref()
        .take(cap as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    if bytes.len() > limit {
        return Err("saved campaign cache grew beyond byte policy".into());
    }
    Ok(Some(bytes))
}

#[cfg(not(target_arch = "wasm32"))]
fn write(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let parent = path
        .parent()
        .ok_or("campaign cache directory is unavailable")?;
    std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    let temporary = path.with_extension(format!(
        "{}-{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|e| format!("{}: {e}", temporary.display()))?;
    let result = file
        .write_all(bytes)
        .and_then(|_| file.sync_all())
        .and_then(|_| {
            drop(file);
            std::fs::rename(&temporary, path)
        });
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result.map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
extern "C" {
    #[wasm_bindgen(catch,js_namespace=peacecraft3GameCaches,js_name=load)]
    fn browser_load(limit: usize) -> Result<Option<Vec<u8>>, wasm_bindgen::JsValue>;
    #[wasm_bindgen(catch,js_namespace=peacecraft3GameCaches,js_name=save)]
    fn browser_save(bytes: &[u8]) -> Result<(), wasm_bindgen::JsValue>;
}
#[cfg(target_arch = "wasm32")]
impl Storage for LocalStorage {
    fn load(&self, limit: usize) -> Result<Option<Vec<u8>>, String> {
        browser_load(limit).map_err(|e| format!("loading local campaign cache: {e:?}"))
    }
    fn save(&self, bytes: &[u8]) -> Result<(), String> {
        browser_save(bytes).map_err(|e| format!("saving local campaign cache: {e:?}"))
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    #[test]
    fn session_profiles_are_validated_bounded_and_independent() {
        let mut profile = crate::game_cache::Profile::default();
        let original = profile.encode(BUDGET).unwrap();
        let first = SessionStorage::new(Some(&original)).unwrap();
        let second = SessionStorage::new(Some(&original)).unwrap();
        profile.open("synthetic", BUDGET).unwrap();
        let changed = profile.encode(BUDGET).unwrap();
        first.save(&changed).unwrap();
        assert_eq!(first.snapshot().unwrap(), Some(changed.clone()));
        assert_eq!(second.snapshot().unwrap(), Some(original.clone()));
        assert!(first.load(changed.len() - 1).is_err());
        assert!(first.save(&vec![0; BUDGET.storage_bytes + 1]).is_err());
        assert_eq!(first.snapshot().unwrap(), Some(changed));
        assert_eq!(SessionStorage::new(None).unwrap().snapshot().unwrap(), None);
        assert!(SessionStorage::new(Some(b"invalid")).is_err());
        assert!(SessionStorage::new(Some(&[])).is_err());
    }

    #[test]
    fn atomic_profile_storage_distinguishes_missing_empty_invalid_and_oversize_inputs() {
        let directory = std::env::temp_dir().join(format!("pc3-cache-test-{}", std::process::id()));
        std::fs::create_dir(&directory).unwrap();
        let filename = directory.join("profile.bin");
        assert_eq!(read(&filename, 8).unwrap(), None);
        write(&filename, b"old").unwrap();
        write(&filename, b"replaced").unwrap();
        assert_eq!(read(&filename, 8).unwrap(), Some(b"replaced".to_vec()));
        assert!(read(&filename, 7).is_err());
        assert!(read(&directory, 8).is_err());
        write(&filename, b"").unwrap();
        assert_eq!(read(&filename, 8).unwrap(), Some(vec![]));
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
        std::fs::remove_dir_all(directory).unwrap();
    }
}

//! Local PC3 campaign profile storage, separate from original W3V files.
use crate::{
    game_cache::{Budget, Storage},
    map_startup_host::MapStartupHost,
};
use std::sync::Arc;

pub fn attach(host: &mut MapStartupHost) -> Result<(), crate::playercolor_host::Error> {
    if host
        .context()
        .scripts
        .program()
        .natives()
        .get("InitGameCache")
        .is_none()
    {
        return Ok(());
    }
    host.objects_mut().admit_game_caches(
        Arc::new(LocalStorage),
        Budget {
            caches: 1024,
            entries: 16384,
            text_bytes: 4 << 20,
            storage_bytes: 8 << 20,
            handle_bytes: 1 << 20,
        },
    )
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

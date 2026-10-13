//! The browser target (PLAN.md §3.8).
//!
//! A browser has no filesystem and no process, and both facts decide this
//! module rather than a workaround for them.
//!
//! * **No process.** [`crate::main`] ends in `std::process::exit`, which has no
//!   wasm equivalent, so a browser boots through [`start`] instead: a
//!   `#[wasm_bindgen(start)]` function that runs the Bevy app and *returns*,
//!   leaving the tab and the event loop alive.
//! * **No filesystem.** Bevy's platform default asset source on wasm is an HTTP
//!   reader rooted at `assets/`, which cannot see the bytes the launcher is
//!   handed. So the browser build replaces the **default** source with
//!   [`MemoryAssetReader`] holding exactly what JavaScript handed over, and the
//!   archives themselves are read by `pcx_mpq::MpqArchive::from_bytes`, the same
//!   reader the desktop uses, from a buffer instead of a `BufReader<File>`.
//!
//! What does **not** work in a browser, stated here rather than left to be
//! discovered:
//!
//! * **Staging does not.** `launch::stage` copies files into a destination and
//!   there is no destination, so every source is reported as rejected. This is
//!   the honest outcome and the launcher says so rather than pretending to copy.
//! * **Install-level version detection does not.** `Install::open` fingerprints
//!   a *set* of archives in a directory. A single archive can be opened and
//!   counted here, but not identified as 1.28c or 1.29b.
//! * **No rendering without a canvas.** With no DOM there is nothing for winit
//!   to attach a window to, so the GUI only starts when `web/index.html`'s
//!   `#launcher-canvas` is present; the asset API still answers either way.
//! * **`pcx-gfx`'s asset cache is still synchronous**, per PLAN.md §3.8.
//!
//! What works: the whole launcher GUI (it is Bevy nodes, the built-in font and
//! solid colours, with no external assets), the mode axes, and receiving and
//! decoding archives from memory. The crate's README is the build and run
//! recipe.

use std::path::Path;
use std::sync::{Mutex, OnceLock};

use bevy::asset::io::memory::{Dir, MemoryAssetReader};
use bevy::asset::io::{AssetSourceBuilder, AssetSourceId, ErasedAssetReader};
use bevy::asset::AssetApp;
use bevy::prelude::*;
use wasm_bindgen::prelude::*;

use crate::launch_gui;

/// CSS selector of the canvas winit renders into, in `web/index.html`.
const CANVAS: &str = "#launcher-canvas";

/// Every byte the launcher holds in a browser.
///
/// A [`Dir`] is `Arc`-shared and cheap to clone, so the store, the asset source
/// and JS calls all look at the same map. It is Bevy's own in-memory reader, not
/// a reimplementation, so what a browser see is what the reader serves.
static STORE: OnceLock<Dir> = OnceLock::new();

fn store() -> Dir {
    STORE.get_or_init(Dir::default).clone()
}

/// What has been handed over, in the order it arrived.
///
/// Bevy's `Dir` is the authority for the bytes themselves; this index says what
/// was handed over and how big it was, so the launcher can report on its own
/// state without asking a reader for every path.
#[derive(Clone)]
struct Held {
    path: String,
    len: usize,
}

static HELD: OnceLock<Mutex<Vec<Held>>> = OnceLock::new();

/// Locks the index. Hold it in a binding for as long as you need it: on
/// `wasm32-unknown-unknown` `std`'s mutex is single-threaded and *panics* on a
/// second lock from the same thread rather than blocking, so a nested lock is a
/// bug a desktop build would only deadlock on.
fn held() -> std::sync::MutexGuard<'static, Vec<Held>> {
    HELD.get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Lines waiting to reach the launcher's log.
static NOTES: OnceLock<Mutex<Vec<String>>> = OnceLock::new();

fn note(line: impl Into<String>) {
    NOTES
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push(line.into());
}

fn take_notes() -> Vec<String> {
    let mut guard = NOTES
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    std::mem::take(&mut *guard)
}

/// Normalises a path into a key the asset source can be indexed by.
///
/// MPQ member names are spelled with backslashes and Bevy asset paths use
/// forward slashes, so `Units\Human\Footman.mdx` and `Units/Human/Footman.mdx`
/// must land on one entry. `.` and `..` are dropped: a memory map has no parent
/// to name, and Bevy's `UnapprovedPathMode::Forbid` rejects them anyway.
fn normalize(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for part in path.replace('\\', "/").split('/') {
        if part.is_empty() || part == "." || part == ".." {
            continue;
        }
        if !out.is_empty() {
            out.push('/');
        }
        out.push_str(part);
    }
    out
}

/// The last component, for log lines where the whole path is noise.
fn short_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

// --------------------------------------------------------- launcher wiring ---

/// Replaces the platform default asset source with the in-memory one and starts
/// the note pump.
///
/// Must run *before* the plugins: Bevy builds an app's asset sources while
/// `AssetPlugin` initialises, and a registered default source is kept rather
/// than overwritten with the platform one.
fn configure(app: &mut App) {
    app.register_asset_source(
        AssetSourceId::Default,
        // 0.20: `AssetSourceBuilder::new` takes the reader directly;
        // `default().with_reader(..)` is gone.
        AssetSourceBuilder::new(|| {
            Box::new(MemoryAssetReader { root: store() }) as Box<dyn ErasedAssetReader>
        }),
    );
    app.add_systems(Startup, pump_notes)
        .add_systems(Update, pump_notes);
}

/// Moves lines pushed by the platform layer into the log the user is reading,
/// capped like anything else the launcher writes there.
fn pump_notes(mut l: ResMut<launch_gui::Launcher>) {
    for line in take_notes() {
        l.push_log(line);
    }
}

/// One-line summary of an archive, as the launcher's log would state it.
///
/// Deliberately counts what the *tables* say and reports the `(listfile)`
/// outcome rather than claiming a file list: payload decryption and
/// decompression are still open in `pcx-mpq`, so an archive opened here is not
/// an archive whose files can be read.
fn describe(name: &str, archive: &mut pcx_mpq::MpqArchive) -> String {
    let blocks = archive.block_entries();
    let live = blocks.iter().filter(|b| b.exists()).count();
    let listed = match archive.contains("(listfile)") {
        true => "(listfile) present".to_string(),
        false => "no (listfile)".to_string(),
    };
    format!(
        "{} opened from memory: {} B, format v{}, {} live blocks of {}, {listed}",
        short_name(name),
        archive.byte_len(),
        archive.header.format_version,
        live,
        blocks.len(),
    )
}

// ------------------------------------------------------------ entry point ----

/// The browser entry point.
///
/// Runs the launcher and returns. There is no process to exit, and exiting would
/// take the tab's event loop with it.
#[wasm_bindgen(start)]
pub fn start() {
    match web_sys::window() {
        Some(_) => {
            launch_gui::app(launch_gui::window_plugin(Some(CANVAS)), configure).run();
        }
        None => note(
            "no DOM here, so there is nothing to render to; \
             the asset API still answers",
        ),
    }
}

// ------------------------------------------------------------ browser API ----

/// Hands bytes to the launcher's asset source.
///
/// The bytes are what `AssetServer::load` will read for `path`, so a loader can
/// get at them without a filesystem. Returns how many bytes are now stored under
/// that path, `0` if the path was rejected.
#[wasm_bindgen]
pub fn add_asset(path: String, bytes: Vec<u8>) -> usize {
    let key = normalize(&path);
    if key.is_empty() {
        note(format!("✕ {path:?} is not a path anything can load"));
        return 0;
    }
    let n = bytes.len();
    store().insert_asset(Path::new(&key), bytes);
    let mut held = held();
    match held.iter_mut().find(|h| h.path == key) {
        Some(entry) => entry.len = n,
        None => held.push(Held { path: key, len: n }),
    }
    n
}

/// Opens an MPQ from the bytes it was given, and reports what its tables say.
///
/// This is the decode half of the browser path: `MpqArchive::from_bytes` is the
/// same reader the desktop uses. The archive itself is not registered as an asset
/// — these bytes are the whole archive, not something to load by path — and one
/// that will not parse is reported and dropped rather than half-loaded.
///
/// # Limitation: an archive opened here exposes none of its files
///
/// This **opens and counts only**. It reads the header and block table, reports
/// the live block count and whether a `(listfile)` is present, and stops. It does
/// **not** extract or register any member, so after `add_archive("War3x.mpq", …)`
/// a call to [`stored_asset`] for `menu3d-exp/mainmenu3d_exp.mdx` still returns
/// `None`: the store holds only what [`add_asset`] put there by path.
///
/// The render path therefore does not depend on it. `web/loader.js` fetches the
/// scene model and its textures as ordinary files from `web/assets/` and hands
/// each one to [`add_asset`], which is why a browser can draw the original menu
/// without any archive support at all. Extracting a *user-supplied* archive into
/// the store would need a bounded extraction step here — walk the listfile,
/// decrypt and decompress each member, cap the count and total bytes, report the
/// refused ones — and that is deliberately not written yet, because nothing in
/// the current flow needs it.
#[wasm_bindgen]
pub fn add_archive(name: String, bytes: Vec<u8>) -> String {
    let key = normalize(&name);
    if key.is_empty() {
        note(format!("✕ {name:?} is not a path anything can open"));
        return format!("rejected: {name:?} is not a path");
    }
    let line = match pcx_mpq::MpqArchive::from_bytes(bytes, &key) {
        Ok(mut archive) => describe(&key, &mut archive),
        Err(e) => format!("✕ {} would not open as an MPQ: {e}", short_name(&key)),
    };
    note(&line);
    line
}

/// Paths currently held in memory, in the order they were handed over.
#[wasm_bindgen]
pub fn stored_assets() -> Vec<String> {
    held().iter().map(|h| h.path.clone()).collect()
}

/// Reads one asset back out of the browser's store.
///
/// This is the read half of [`add_asset`], and it exists so the menu can use
/// art that was served over HTTP rather than read from a filesystem a browser
/// does not have. A path that was never handed over returns `None` — the same
/// "report the gap rather than pretend" rule the rest of the launcher follows.
#[wasm_bindgen]
pub fn stored_asset(path: String) -> Option<Vec<u8>> {
    let key = normalize(&path);
    if key.is_empty() {
        return None;
    }
    store()
        .get_asset(Path::new(&key))
        .map(|b| b.value().to_vec())
}

/// Non-exported form of [`stored_asset`], for Rust callers inside this crate.
/// The launcher's menu uses this; JavaScript uses the exported one.
pub(crate) fn stored_bytes(path: &str) -> Option<Vec<u8>> {
    let key = normalize(path);
    if key.is_empty() {
        return None;
    }
    store()
        .get_asset(Path::new(&key))
        .map(|b| b.value().to_vec())
}

/// Drops everything in memory and reports how many entries went.
#[wasm_bindgen]
pub fn drop_assets() -> usize {
    let gone = std::mem::take(&mut *held());
    for entry in &gone {
        store().remove_asset(Path::new(&entry.path));
    }
    gone.len()
}

/// Reports what the browser path can currently do.
///
/// The claim it checks is the one that matters: that an asset handed over from
/// JavaScript is still *there* when Bevy's default asset source asks for it. The
/// bytes live inside the reader, so this checks the reader's own entry rather
/// than re-reading the payload — that is what an `AssetServer::load` would find.
#[wasm_bindgen]
pub fn selftest() -> String {
    let held = held().clone();
    if held.is_empty() {
        return "no assets in memory".to_string();
    }
    let dir = store();
    let held_bytes: usize = held.iter().map(|h| h.len).sum();
    let missing: Vec<&str> = held
        .iter()
        .filter(|h| dir.get_asset(Path::new(&h.path)).is_none())
        .map(|h| h.path.as_str())
        .collect();
    format!(
        "{} assets, {} B handed over; still served by the default asset source: {}; missing: [{}]",
        held.len(),
        held_bytes,
        missing.is_empty(),
        missing.join(", "),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mpq_paths_and_asset_paths_normalise_the_same() {
        assert_eq!(normalize("war3.mpq"), "war3.mpq");
        assert_eq!(normalize("War3Patch.mpq"), "War3Patch.mpq");
        // The spelling inside an MPQ, and the one Bevy would load.
        assert_eq!(
            normalize("Units\\Human\\Footman\\Footman.mdx"),
            "Units/Human/Footman/Footman.mdx"
        );
        // A browser URL or a hand-typed path, tidied rather than rewritten.
        assert_eq!(normalize("/assets/war3.mpq"), "assets/war3.mpq");
        assert_eq!(normalize("./units///a.mdx"), "units/a.mdx");
        assert_eq!(normalize("../escape.mpq"), "escape.mpq");
        assert_eq!(normalize(""), "");
        // Nothing that Bevy's unapproved-path mode would refuse.
        assert!(!normalize("..\\x").contains(".."));
    }

    #[test]
    fn a_rejected_path_never_reaches_the_store() {
        // An empty path has no file name; storing it would panic rather than
        // fail, so the guard is the behaviour worth pinning down.
        assert_eq!(add_asset(String::new(), vec![1, 2, 3]), 0);
        assert!(add_archive(String::new(), vec![1]).starts_with("rejected"));
        assert!(stored_assets().is_empty());
    }

    #[test]
    fn a_handed_over_asset_is_served_by_the_reader_it_was_given_to() {
        let bytes = vec![7u8; 12];
        assert_eq!(add_asset("war3.mpq".into(), bytes.clone()), 12);
        assert_eq!(stored_assets(), vec!["war3.mpq".to_string()]);
        // The reader the launcher registers is the one holding the bytes: if an
        // `AssetServer::load` cannot see this, the browser path is a lie.
        assert!(store().get_asset(Path::new("war3.mpq")).is_some());
        assert!(selftest().contains("still served by the default asset source: true"));
        // The same bytes spelled the MPQ way: one entry, not two.
        assert_eq!(add_asset("WAR3.MPQ".into(), bytes), 12);
        assert_eq!(stored_assets(), vec!["war3.mpq".to_string()]);
        assert_eq!(drop_assets(), 1);
        assert!(stored_assets().is_empty());
    }

    #[test]
    fn an_archive_that_is_not_an_mpq_is_reported_not_stored() {
        let line = add_archive("not-an-mpq.bin".into(), b"certainly not an MPQ".to_vec());
        assert!(line.contains("would not open"), "{line}");
        assert!(stored_assets().is_empty(), "a bad archive must not land");
    }
}

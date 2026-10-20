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

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::{Mutex, OnceLock};

static MENU_SNAPSHOT: OnceLock<Mutex<String>> = OnceLock::new();
static MENU_SNAPSHOT_REQUESTED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Read-only, local diagnostics. Requests one fresh post-layout snapshot; it
/// never activates a control or changes an application setting.
#[wasm_bindgen]
pub fn menu_state() -> String {
    MENU_SNAPSHOT_REQUESTED.store(true, std::sync::atomic::Ordering::Relaxed);
    MENU_SNAPSHOT
        .get_or_init(|| Mutex::new("{}".into()))
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
}

pub(crate) fn take_menu_snapshot_request() -> bool {
    MENU_SNAPSHOT_REQUESTED.swap(false, std::sync::atomic::Ordering::Relaxed)
}

pub(crate) fn publish_menu_snapshot(snapshot: String) {
    *MENU_SNAPSHOT
        .get_or_init(|| Mutex::new(String::new()))
        .lock()
        .unwrap_or_else(|p| p.into_inner()) = snapshot;
}

use bevy::asset::AssetApp;
use bevy::asset::io::memory::{Dir, MemoryAssetReader};
use bevy::asset::io::{
    AssetReader, AssetReaderError, AssetSourceBuilder, AssetSourceId, ErasedAssetReader,
    PathStream, Reader,
};
use bevy::prelude::*;
use wasm_bindgen::prelude::*;

use crate::launch_gui;

mod configuration_ffi;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(catch, js_namespace = peacecraft3Audio, js_name = stopEvents)]
    fn stop_browser_menu_events() -> Result<(), JsValue>;
}

fn stop_menu_events() {
    if web_sys::window().is_some()
        && let Err(error) = stop_browser_menu_events()
    {
        note(format!("Menu event audio cleanup failed: {error:?}"));
    }
}

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

#[derive(Default)]
struct DemoControl {
    enabled: bool,
    editions: crate::local_demo::MenuEditions,
    requested: Option<crate::launch::Addon>,
    active: Option<crate::launch::Addon>,
}

static DEMO_CONTROL: OnceLock<Mutex<DemoControl>> = OnceLock::new();

fn demo_control() -> std::sync::MutexGuard<'static, DemoControl> {
    DEMO_CONTROL
        .get_or_init(|| Mutex::new(DemoControl::default()))
        .lock()
        .unwrap_or_else(|p| p.into_inner())
}

/// Enables the asset-free demo; it cannot authorize an absent menu edition.
#[wasm_bindgen]
pub fn enable_local_demo() {
    demo_control().enabled = true;
}

/// Capabilities from successfully hydrated menu models, not release detection.
#[wasm_bindgen]
pub fn local_menu_editions() -> Vec<String> {
    let editions = demo_control().editions;
    [crate::launch::Addon::Roc, crate::launch::Addon::Tft]
        .into_iter()
        .filter(|a| editions.allows(*a))
        .map(|a| a.label().to_owned())
        .collect()
}

#[wasm_bindgen]
pub fn open_local_menu(edition: &str) -> Result<(), JsValue> {
    let addon = match edition {
        "roc" => crate::launch::Addon::Roc,
        "tft" => crate::launch::Addon::Tft,
        _ => return Err(JsValue::from_str("UNKNOWN_MENU_EDITION")),
    };
    let mut control = demo_control();
    if !control.enabled || !control.editions.allows(addon) {
        return Err(JsValue::from_str(
            "UNAVAILABLE_MENU_EDITION: import its local content first",
        ));
    }
    control.requested = Some(addon);
    drop(control);
    stop_menu_events();
    Ok(())
}

#[wasm_bindgen]
pub fn active_local_menu_edition() -> Option<String> {
    demo_control().active.map(|a| a.label().to_owned())
}

pub(crate) fn take_demo_control() -> (
    bool,
    crate::local_demo::MenuEditions,
    Option<crate::launch::Addon>,
) {
    let mut control = demo_control();
    (control.enabled, control.editions, control.requested.take())
}

pub(crate) fn publish_demo_edition(addon: crate::launch::Addon) {
    let mut control = demo_control();
    control.active = control.editions.allows(addon).then_some(addon);
}

/// Normalises a path into a key the asset source can be indexed by.
///
/// MPQ member names are spelled with backslashes and Bevy asset paths use
/// forward slashes, so `Units\Human\Footman.mdx` and `Units/Human/Footman.mdx`
/// must land on one entry. `.` and `..` are dropped: a memory map has no parent
/// to name, and Bevy's `UnapprovedPathMode::Forbid` rejects them anyway.
fn normalize(path: &str) -> String {
    crate::browser_read::normalize(path)
}

/// The last component, for log lines where the whole path is noise.
fn short_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

// Resource limits, independent of any shipped asset's size.
const MEMBER_LIMIT: u32 = 64 * 1024 * 1024;
const HYDRATION_LIMIT: usize = 256 * 1024 * 1024;
static ARCHIVES: OnceLock<Mutex<Vec<pcx_mpq::MpqArchive>>> = OnceLock::new();
static PENDING: OnceLock<Mutex<Option<Vec<pcx_mpq::MpqArchive>>>> = OnceLock::new();
static REVISION: OnceLock<Mutex<u32>> = OnceLock::new();

fn archives() -> std::sync::MutexGuard<'static, Vec<pcx_mpq::MpqArchive>> {
    ARCHIVES
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .unwrap_or_else(|p| p.into_inner())
}
fn pending() -> std::sync::MutexGuard<'static, Option<Vec<pcx_mpq::MpqArchive>>> {
    PENDING
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|p| p.into_inner())
}
fn revision() -> std::sync::MutexGuard<'static, u32> {
    REVISION
        .get_or_init(|| Mutex::new(0))
        .lock()
        .unwrap_or_else(|p| p.into_inner())
}

/// Same image ceiling `Policy::discover` uses for one selected client.
const CLIENT_IMAGE_BYTES: usize = 32 * 1024 * 1024;

struct ClientImage {
    source: String,
    bytes: Vec<u8>,
}

struct CommittedClientImage {
    revision: u32,
    source: String,
    bytes: Vec<u8>,
}

#[derive(Default)]
struct ClientImageTxn {
    pending: Option<ClientImage>,
    committed: Option<CommittedClientImage>,
}

fn client_image_input(source: &str, len: usize) -> Result<(), &'static str> {
    if source.trim().is_empty() {
        return Err("missing source identity");
    }
    if len == 0 {
        return Err("client image is empty");
    }
    if len > CLIENT_IMAGE_BYTES {
        return Err("Options client exceeds image budget");
    }
    Ok(())
}

impl ClientImageTxn {
    fn clear_pending(&mut self) {
        self.pending = None;
    }

    fn clear_all(&mut self) {
        self.pending = None;
        self.committed = None;
    }

    fn stage(&mut self, active: bool, source: String, bytes: Vec<u8>) -> Result<(), &'static str> {
        if !active {
            return Err("no active archive import");
        }
        client_image_input(&source, bytes.len())?;
        if self.pending.is_some() {
            return Err("client image is already staged");
        }
        self.pending = Some(ClientImage { source, bytes });
        Ok(())
    }

    /// Installs the pending image at `revision`. An empty pending slot clears
    /// the previous image so the new revision does not keep it.
    fn publish(&mut self, revision: u32) {
        let pending = self.pending.take();
        self.committed = pending.map(|image| CommittedClientImage {
            revision,
            source: image.source,
            bytes: image.bytes,
        });
    }

    fn committed_at(&self, revision: u32) -> Option<(&str, &[u8])> {
        self.committed
            .as_ref()
            .filter(|image| image.revision == revision)
            .map(|image| (image.source.as_str(), image.bytes.as_slice()))
    }
}

static CLIENT_IMAGES: OnceLock<Mutex<ClientImageTxn>> = OnceLock::new();

fn client_images() -> std::sync::MutexGuard<'static, ClientImageTxn> {
    CLIENT_IMAGES
        .get_or_init(|| Mutex::new(ClientImageTxn::default()))
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

/// Lends the image published for the current asset revision.
/// The bytes stay in this slot. The callback must not re-enter the slot.
pub(crate) fn with_committed_client_image<T>(read: impl FnOnce(&str, &[u8]) -> T) -> Option<T> {
    let revision = *revision();
    let slot = client_images();
    let (source, bytes) = slot.committed_at(revision)?;
    Some(read(source, bytes))
}

/// No priority is inferred from arrival order or filenames. Equal duplicates
/// are safe; distinct bytes at one member path require installation evidence.
fn extract_unique(set: &mut [pcx_mpq::MpqArchive], path: &str) -> Result<Vec<u8>, String> {
    extract_unique_bounded(set, path, MEMBER_LIMIT as usize)
}

fn extract_unique_bounded(
    set: &mut [pcx_mpq::MpqArchive],
    path: &str,
    limit: usize,
) -> Result<Vec<u8>, String> {
    let limit = limit.min(MEMBER_LIMIT as usize);
    crate::browser_read::read_bounded(
        &Dir::default(),
        set,
        *revision(),
        path,
        crate::browser_read::BoundedReadPolicy {
            max_copy_bytes: limit,
            extraction: pcx_mpq::ExtractionBudget {
                max_raw_packed_bytes: limit,
                max_decoded_bytes: limit,
            },
            huffman: None,
        },
    )
    .map_err(|error| error.to_string())?
    .map(|asset| asset.bytes)
    .ok_or_else(|| format!("MISSING_MEMBER: {path}"))
}

struct ArchiveAssetReader {
    memory: MemoryAssetReader,
}
impl AssetReader for ArchiveAssetReader {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        if self.memory.root.get_asset(path).is_none() {
            let name = path.to_string_lossy();
            let bytes = extract_unique(&mut archives(), &name.replace('/', "\\"))
                .map_err(|e| AssetReaderError::from(std::io::Error::other(e)))?;
            add_asset(name.into_owned(), bytes);
        }
        AssetReader::read(&self.memory, path).await
    }
    async fn read_meta<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        AssetReader::read_meta(&self.memory, path).await
    }
    async fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        AssetReader::read_directory(&self.memory, path).await
    }
    async fn is_directory<'a>(&'a self, path: &'a Path) -> Result<bool, AssetReaderError> {
        AssetReader::is_directory(&self.memory, path).await
    }
}

/// Existing native/browser music binding; this does not establish an edition.
#[wasm_bindgen]
pub fn menu_music_path() -> String {
    crate::menu_audio::MENU_MUSIC_PATH.to_owned()
}

/// Changes once per committed import/clear, never while staging archives.
#[wasm_bindgen]
pub fn asset_revision() -> u32 {
    *revision()
}

/// Inspect only the explicitly requested committed archive index/generation.
pub fn committed_archive_selection(
    index: usize,
    revision: u32,
) -> Result<peacecraft3::browser_archive_source::ArchiveSelection, pcx_assets::SourceError> {
    peacecraft3::browser_archive_source::ArchiveSelection::inspect(
        ARCHIVES.get_or_init(|| Mutex::new(Vec::new())),
        REVISION.get_or_init(|| Mutex::new(0)),
        index,
        revision,
    )
}

/// Mount one explicitly selected committed archive for the real bounded resolver.
/// No renderer aliases, archive ordering or release identity is inferred.
pub fn committed_archive_source(
    selected: peacecraft3::browser_archive_source::ArchiveSelection,
    info: pcx_assets::SourceInfo,
    names: Vec<pcx_assets::NamedMember>,
    budget: pcx_mpq::ExtractionBudget,
) -> Result<peacecraft3::browser_archive_source::CommittedArchiveSource, pcx_assets::SourceError> {
    peacecraft3::browser_archive_source::CommittedArchiveSource::mount_no_huffman(
        ARCHIVES.get_or_init(|| Mutex::new(Vec::new())),
        REVISION.get_or_init(|| Mutex::new(0)),
        selected,
        info,
        names,
        budget,
    )
}
static MAP_CONFIGURATION: OnceLock<peacecraft3::browser_configuration::ConfigurationInbox> =
    OnceLock::new();
fn map_configuration_inbox() -> &'static peacecraft3::browser_configuration::ConfigurationInbox {
    MAP_CONFIGURATION.get_or_init(Default::default)
}
/// Rust producer seam for the explicit JS schema adapter; no source policy is derived.
pub fn enqueue_map_configuration(
    request: peacecraft3::browser_configuration::Configuration,
) -> Result<u32, peacecraft3::browser_configuration::InboxError> {
    map_configuration_inbox().enqueue(request)
}
pub fn poll_map_configuration(
    id: u32,
) -> Result<peacecraft3::browser_configuration::Poll, peacecraft3::browser_configuration::InboxError>
{
    map_configuration_inbox().poll(id)
}

/// Decode all explicit metadata before constructing selected sources or queuing work.
pub fn enqueue_map_configuration_json(
    text: &str,
    max_metadata_bytes: usize,
) -> Result<u32, peacecraft3::browser_configuration_json::Error> {
    peacecraft3::browser_configuration_json::decode_and_enqueue(
        text,
        max_metadata_bytes,
        map_configuration_inbox(),
        |index, revision, info, names, budget| {
            let selected = committed_archive_selection(index, revision)?;
            committed_archive_source(selected, info, names, budget)
        },
    )
}

/// Starts a transaction without altering visible archives/assets.
#[wasm_bindgen]
pub fn begin_archive_import() {
    *pending() = Some(Vec::new());
    client_images().clear_pending();
}

#[wasm_bindgen]
pub fn cancel_archive_import() {
    *pending() = None;
    client_images().clear_pending();
}

/// Validate an archive into the pending transaction; no members become visible.
#[wasm_bindgen]
pub fn stage_archive(name: String, bytes: Vec<u8>) -> Result<String, JsValue> {
    let archive = pcx_mpq::MpqArchive::from_bytes(bytes, &name)
        .map_err(|e| JsValue::from_str(&format!("{name}: {e}")))?;
    let mut archive = archive;
    let summary = describe(&name, &mut archive);
    pending()
        .as_mut()
        .ok_or_else(|| JsValue::from_str("no active archive import"))?
        .push(archive);
    Ok(summary)
}

/// One client image for the open archive transaction. The bytes stay in this slot.
#[wasm_bindgen]
pub fn stage_client_image(source: String, bytes: Vec<u8>) -> Result<(), JsValue> {
    // Drop the archive guard before the image slot. wasm mutexes panic on re-entry.
    let active = pending().is_some();
    client_images()
        .stage(active, source, bytes)
        .map_err(JsValue::from_str)
}

/// Source identity committed with the current asset revision. Bytes stay here.
#[wasm_bindgen]
pub fn committed_client_image_identity() -> Option<String> {
    with_committed_client_image(|source, _bytes| source.to_owned())
}

/// Named extraction from the staged set, including codec and conflict errors.
#[wasm_bindgen]
pub fn pending_archive_member(path: String) -> Result<Vec<u8>, JsValue> {
    let mut guard = pending();
    let set = guard
        .as_mut()
        .ok_or_else(|| JsValue::from_str("no active archive import"))?;
    extract_unique(set, &path).map_err(|e| JsValue::from_str(&e))
}

/// Named extraction from committed archives; cached under the original path.
#[wasm_bindgen]
pub fn archive_member(path: String) -> Result<Vec<u8>, JsValue> {
    let bytes = extract_unique(&mut archives(), &path).map_err(|e| JsValue::from_str(&e))?;
    add_asset(path, bytes.clone());
    Ok(bytes)
}

fn derived_menu_fonts(
    fdf: &crate::fdf::Fdf,
    texts: &[&str],
    skin: &str,
) -> Result<Vec<String>, String> {
    let mut measured = fdf.clone();
    crate::menu_screens::prime_text_sizes(&mut measured);
    let (items, _) = crate::glue_menu::control_items(
        &measured,
        crate::glue_menu::CONTROL_LAYER,
        (crate::mainmenu::FRAME_W, crate::mainmenu::FRAME_H),
    )
    .map_err(|e| e.to_string())?;
    let mut paths = BTreeSet::new();
    for item in &items {
        let crate::glue_menu::Kind::Button(button) = &item.kind else {
            continue;
        };
        let Some(frame) = &button.text else {
            continue;
        };
        let style = crate::mainmenu::menu_text_style(texts, frame).map_err(|e| e.to_string())?;
        paths.insert(
            crate::mainmenu::menu_font_file(skin, &style.font)
                .map_err(|e| e.to_string())?
                .file,
        );
    }
    for frame in fdf.frames() {
        if frame.kind == "TEXT" && fdf.record(&frame.name, "FrameFont").is_some() {
            let style = crate::menu_text::render_style(fdf, &frame.name)?;
            paths.insert(
                crate::mainmenu::menu_font_file(skin, &style.font)
                    .map_err(|e| e.to_string())?
                    .file,
            );
        }
        if let Some(target) = &frame.props.button_text
            && let Ok(style) = crate::mainmenu::menu_text_style(texts, target)
            && let Ok(binding) = crate::mainmenu::menu_font_file(skin, &style.font)
        {
            paths.insert(binding.file);
        }
    }
    Ok(paths.into_iter().collect())
}

/// Font paths derived from the currently supplied FDF and skin table.
#[wasm_bindgen]
pub fn menu_font_files() -> Result<Vec<String>, JsValue> {
    let text = |path: &str| -> Result<String, JsValue> {
        let alias = crate::mainmenu::served_name(path).unwrap_or(path);
        let bytes = stored_bytes_fallible(alias)
            .map_err(|error| JsError::new(&error.to_string()))?
            .ok_or_else(|| JsError::new(&format!("missing {path}")))?
            .bytes;
        String::from_utf8(bytes).map_err(|e| JsValue::from_str(&format!("{path}: {e}")))
    };
    let templates = text(crate::mainmenu::STANDARD_TEMPLATES_FDF)?;
    let menu = text(crate::mainmenu::MAIN_MENU_FDF)?;
    let skin = text(crate::mainmenu::SKIN_TABLE)?;
    let mut fdf = crate::fdf::Fdf::default();
    for source in [&templates, &menu] {
        fdf.parse_into(source)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
    }
    derived_menu_fonts(&fdf, &[&templates, &menu], &skin).map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn menu_font_alias(path: String) -> Option<String> {
    crate::mainmenu::served_font_name(&path)
}

type HydratedMenuAssets = (BTreeMap<String, Vec<u8>>, Vec<String>);

fn hydrate(set: &mut [pcx_mpq::MpqArchive]) -> Result<HydratedMenuAssets, String> {
    let mut assets = crate::import_assets::HydratedAssets::new(HYDRATION_LIMIT);
    let mut notes = Vec::new();
    let mut put = |path: &str, alias: Option<&str>, bytes: Vec<u8>| -> Result<(), String> {
        assets.insert(path, normalize(path), alias.map(normalize), bytes)
    };
    let mut fdf = crate::fdf::Fdf::default();
    let mut frame_texts = Vec::new();
    let strings_path = crate::mainmenu::GLOBAL_STRINGS_FDF;
    let strings = extract_unique(set, strings_path)?;
    std::str::from_utf8(&strings).map_err(|e| format!("{strings_path}: {e}"))?;
    put(
        strings_path,
        crate::mainmenu::served_name(strings_path),
        strings,
    )?;
    for path in [
        crate::mainmenu::STANDARD_TEMPLATES_FDF,
        crate::mainmenu::MAIN_MENU_FDF,
    ] {
        let bytes = extract_unique(set, path)?;
        let text = std::str::from_utf8(&bytes).map_err(|e| format!("{path}: {e}"))?;
        fdf.parse_into(text).map_err(|e| format!("{path}: {e}"))?;
        frame_texts.push(text.to_owned());
        put(path, crate::mainmenu::served_name(path), bytes)?;
    }
    // Each submenu is optional for old/local minimal bundles; missing content
    // is reported when opened. Present definitions and their include graph are
    // imported locally with the same aggregate admission budget as the menu.
    let mut loaded = BTreeSet::from([
        normalize(crate::mainmenu::STANDARD_TEMPLATES_FDF),
        normalize(crate::mainmenu::MAIN_MENU_FDF),
    ]);
    let mut pending: Vec<String> = crate::menu_screens::SCREEN_FILES
        .iter()
        .map(|p| p.to_string())
        .collect();
    while let Some(path) = pending.pop() {
        if !loaded.insert(normalize(&path)) {
            continue;
        }
        if loaded.len() > 128 {
            return Err("MENU_INCLUDE_LIMIT: too many supplied screen dependencies".into());
        }
        let bytes = match extract_unique(set, &path) {
            Ok(bytes) => bytes,
            Err(e) if e.starts_with("MISSING_MEMBER:") => {
                notes.push(e);
                continue;
            }
            Err(e) => return Err(e),
        };
        let text = std::str::from_utf8(&bytes).map_err(|e| format!("{path}: {e}"))?;
        pending.extend(crate::menu_screens::includes(text));
        fdf.parse_into(text).map_err(|e| format!("{path}: {e}"))?;
        frame_texts.push(text.to_string());
        put(&path, None, bytes)?;
    }
    let skin_path = crate::mainmenu::SKIN_TABLE;
    let skin_bytes = extract_unique(set, skin_path)?;
    let skin = std::str::from_utf8(&skin_bytes).map_err(|e| format!("{skin_path}: {e}"))?;
    let skin_table = crate::menu_skin::SkinTable::parse(skin).map_err(|e| e.to_string())?;
    let texts: Vec<_> = frame_texts.iter().map(String::as_str).collect();
    for path in derived_menu_fonts(&fdf, &texts, skin)? {
        let alias = crate::mainmenu::served_font_name(&path)
            .ok_or_else(|| format!("invalid font path {path}"))?;
        put(&path, Some(&alias), extract_unique(set, &path)?)?;
    }
    put(
        skin_path,
        crate::mainmenu::served_name(skin_path),
        skin_bytes,
    )?;
    // Backdrop and highlight art comes from the selected frame definitions,
    // not a JS list.
    for frame in fdf.frames() {
        for (key, value) in &frame.props.records {
            if !matches!(
                key.as_str(),
                "BackdropBackground" | "BackdropEdgeFile" | "HighlightAlphaFile"
            ) {
                continue;
            }
            let path = value.trim_matches('"');
            match extract_unique(set, path) {
                Ok(bytes) => put(path, crate::mainmenu::served_name(path), bytes)?,
                Err(e) if e.starts_with("MISSING_MEMBER:") => notes.push(e),
                Err(e) => return Err(e),
            }
        }
    }
    let fog_path = crate::mainmenu::MENU_FOG_CONFIG;
    let fog_bytes = extract_unique(set, fog_path)?;
    let fog_text = std::str::from_utf8(&fog_bytes).map_err(|e| format!("{fog_path}: {e}"))?;
    let fog_table =
        crate::menu_skin::SkinTable::parse(fog_text).map_err(|e| format!("{fog_path}: {e}"))?;
    put(fog_path, crate::mainmenu::served_name(fog_path), fog_bytes)?;
    let mut models = 0;
    let mut available_editions = [false; 2];
    for expansion in [false, true] {
        let (_, alias, directory) = crate::mainmenu_scene::scene_source(expansion);
        let path = match crate::mainmenu_scene::skin_scene_source(&skin_table, expansion) {
            Ok(path) => path,
            Err(crate::menu_skin::SkinError::UnknownFace(key)) => {
                notes.push(format!("MISSING_SKIN_BINDING: {key}"));
                continue;
            }
            Err(error) => return Err(error.to_string()),
        };
        match extract_unique(set, &path) {
            Ok(bytes) => {
                if fdf.frame("EditionButton").is_some() {
                    let client = client_images();
                    if let Some(image) = client.pending.as_ref() {
                        let art = pcx_play::menu_edition::ButtonArt::read(
                            &image.bytes,
                            &pcx_play::scripts::AssetIdentity {
                                source: image.source.clone(),
                                edition: "selected menu client".into(),
                                version: "reviewed Legacy 1.29.2.9231".into(),
                            },
                            expansion,
                        )?;
                        for path in art.textures {
                            put(&path, None, extract_unique(set, &path)?)?;
                        }
                    } else {
                        notes.push("EDITION_ART_UNAVAILABLE: selected client image missing".into());
                    }
                }
                crate::menu_fog::MenuFog::checked(&fog_table, usize::from(expansion)).map_err(
                    |e| {
                        format!(
                            "{fog_path}: edition {} for {path}: {e}",
                            usize::from(expansion)
                        )
                    },
                )?;
                let context = format!(
                    "edition={}; import candidate; member={path}",
                    usize::from(expansion)
                );
                let particle_model =
                    crate::mainmenu_scene::particle_import_needed(&context, &bytes)?;
                let mut decoded = Vec::new();
                let mut remaining_pixels = crate::mainmenu_scene::MENU_IMAGE_BYTES;
                for texture in crate::mainmenu_scene::texture_paths(&bytes)
                    .map_err(|e| format!("{path}: {e}"))?
                {
                    let raw = extract_unique(set, &texture)?;
                    if particle_model {
                        decoded.push(
                            pcx_gfx::load_mipmapped_image_bounded(&raw, &texture, remaining_pixels)
                                .map_err(|e| format!("{context}: TEXS {texture}: {e}"))?,
                        );
                    }
                    if let Some(image) = decoded.last()
                        && particle_model
                    {
                        remaining_pixels -= image.rgba_bytes().expect("validated mip chain");
                    }
                    let alias = crate::mainmenu_scene::texture_served_name_in(&texture, directory);
                    put(&texture, Some(&alias), raw)?;
                }
                if particle_model
                    && let Some(gap) =
                        crate::mainmenu_scene::validate_particle_import(&context, &bytes, &decoded)?
                {
                    notes.push(gap);
                }
                put(&path, Some(alias), bytes)?;
                models += 1;
                available_editions[usize::from(expansion)] = true;
            }
            Err(e) if e.starts_with("MISSING_MEMBER:") => notes.push(e),
            Err(e) => return Err(e),
        }
    }
    if models == 0 {
        return Err(
            "MISSING_MENU_MODEL: neither existing renderer model contract is present".into(),
        );
    }
    for expansion in [false, true] {
        let path = match crate::menu_panel::logo_model_path(&skin_table, expansion) {
            Ok(path) => path,
            Err(crate::menu_skin::SkinError::UnknownFace(key)) => {
                notes.push(format!("MISSING_SKIN_BINDING: {key}"));
                continue;
            }
            Err(error) => return Err(error.to_string()),
        };
        match extract_unique(set, &path) {
            Ok(bytes) => {
                for texture in crate::mainmenu_scene::texture_paths(&bytes)
                    .map_err(|e| format!("{path}: {e}"))?
                {
                    let alias = (texture.eq_ignore_ascii_case(crate::menu_panel::LOGO_TEXTURE))
                        .then_some(crate::menu_panel::LOGO_TEXTURE_SERVED);
                    put(&texture, alias, extract_unique(set, &texture)?)?;
                }
                put(
                    &path,
                    expansion.then_some(crate::menu_panel::LOGO_MODEL_SERVED),
                    bytes,
                )?;
            }
            Err(e) if e.starts_with("MISSING_MEMBER:") => notes.push(e),
            Err(e) => return Err(e),
        }
    }
    // Admit complete panel graphics even when a known audio codec is unavailable.
    // Missing or malformed startup assets still refuse the transaction.
    // Navigation phases are optional; their failure leaves startup usable.
    for edition in [0, 1] {
        if !available_editions[edition as usize] {
            continue;
        }
        let paths = match crate::menu_sprites::model_paths(&skin_table, edition) {
            Ok(paths) => paths,
            Err(crate::menu_skin::SkinError::UnknownFace(key)) => {
                notes.push(format!("MISSING_SKIN_BINDING: {key}"));
                continue;
            }
            Err(error) => return Err(error.to_string()),
        };
        let script_path = crate::menu_sprites::BLIZZARD_SCRIPT;
        let script = extract_unique_bounded(set, script_path, 2 * 1024 * 1024)?;
        let scale = crate::menu_sprites::VolumeScale::checked(&script)
            .map_err(|e| format!("{script_path}: {e}"))?;
        put(script_path, None, script)?;
        let mut tables = Vec::new();
        for table_path in [
            crate::menu_sprites::ANIM_LOOKUPS,
            crate::menu_sprites::ANIM_SOUNDS,
        ] {
            let raw = extract_unique_bounded(set, table_path, 1024 * 1024)?;
            tables.push(
                pcx_slq::Table::parse_bounded(
                    &raw,
                    pcx_slq::CoordinateLimits {
                        max_row: 10_000,
                        max_column: 256,
                    },
                )
                .map_err(|e| format!("{table_path}: {e}"))?,
            );
            put(table_path, None, raw)?;
        }
        let mut sound_paths = std::collections::BTreeSet::new();
        let mut sound_bytes = 0usize;
        for path in paths {
            let bytes = extract_unique_bounded(set, &path, 16 * 1024 * 1024)?;
            let mut textures = Vec::new();
            let mut remaining = crate::mainmenu_scene::MENU_IMAGE_BYTES;
            for texture in
                crate::mainmenu_scene::texture_paths(&bytes).map_err(|e| format!("{path}: {e}"))?
            {
                let raw = extract_unique(set, &texture)?;
                let decoded = pcx_gfx::load_mipmapped_image_bounded(&raw, &texture, remaining)
                    .map_err(|e| format!("{path}: TEXS {texture}: {e}"))?;
                remaining -= decoded.rgba_bytes().expect("validated mip chain");
                textures.push(decoded);
                put(&texture, None, raw)?;
            }
            use crate::menu_sprites::SoundScope;
            for scope in [
                SoundScope::Startup,
                SoundScope::MainSingle,
                SoundScope::MainOptions,
                SoundScope::Category,
            ] {
                let mut unavailable_codecs = Vec::new();
                let candidate = (|| -> Result<_, String> {
                    crate::menu_sprites::preflight_source(&path, &bytes, &textures, scope)
                        .map_err(|e| e.to_string())?;
                    let dependencies = crate::menu_sprites::sound_dependencies(
                        &path, &bytes, &tables[0], &tables[1], scale, scope,
                    )
                    .map_err(|e| e.to_string())?;
                    let mut staged = Vec::new();
                    let mut selected = std::collections::BTreeSet::new();
                    let mut total = sound_bytes;
                    for dependency in dependencies {
                        let key = normalize(&dependency.path);
                        if sound_paths.contains(&key) || !selected.insert(key) {
                            continue;
                        }
                        let limit = (64 * 1024 * 1024 - total).min(16 * 1024 * 1024);
                        let raw = match crate::browser_read::read_bounded(
                            &Dir::default(),
                            set,
                            *revision(),
                            &dependency.path,
                            crate::browser_read::BoundedReadPolicy {
                                max_copy_bytes: limit,
                                extraction: pcx_mpq::ExtractionBudget {
                                    max_raw_packed_bytes: limit,
                                    max_decoded_bytes: limit,
                                },
                                huffman: None,
                            },
                        ) {
                            Ok(Some(asset)) => asset.bytes,
                            Ok(None) => return Err(format!("MISSING_MEMBER: {}", dependency.path)),
                            Err(error) => {
                                if crate::browser_read::panel_codec_mask(&error).is_some() {
                                    unavailable_codecs.push(error.to_string());
                                    continue;
                                }
                                return Err(error.to_string());
                            }
                        };
                        total = total
                            .checked_add(raw.len())
                            .filter(|&n| n <= 64 * 1024 * 1024)
                            .ok_or_else(|| {
                                format!(
                                    "{}: panel sound aggregate byte bound exceeded",
                                    dependency.path
                                )
                            })?;
                        crate::menu_sprites::validate_sound_bytes(
                            &dependency.path,
                            &raw,
                            pcx_play::scripts::AssetIdentity {
                                edition: format!("selected skin V{edition}"),
                                version: "archive release unestablished".into(),
                                source: "staged local browser archive batch".into(),
                            },
                        )
                        .map_err(|e| e.to_string())?;
                        staged.push((dependency.path, raw));
                    }
                    Ok((staged, total))
                })();
                match candidate {
                    Ok((staged, total)) => {
                        // All other required dependencies passed before this capability fallback.
                        for error in unavailable_codecs {
                            notes.push(format!(
                                "PANEL_AUDIO_UNAVAILABLE: edition={edition}; scope={scope:?}; {error}"
                            ));
                        }
                        for (sound, raw) in staged {
                            sound_paths.insert(normalize(&sound));
                            put(&sound, None, raw)?;
                        }
                        sound_bytes = total;
                    }
                    Err(error) if !matches!(scope, SoundScope::Startup) => {
                        let code = match scope {
                            SoundScope::MainOptions => "PANEL_OPTIONS_DISABLED",
                            SoundScope::Category => "PANEL_CATEGORY_DISABLED",
                            _ => "PANEL_NAVIGATION_DISABLED",
                        };
                        notes.push(format!("{code}: edition={edition}; {error}"));
                    }
                    Err(error) => return Err(error),
                }
            }
            put(&path, None, bytes)?;
        }
    }
    for path in [
        crate::menu_audio::MENU_MUSIC_PATH,
        r"UI\SoundInfo\UISounds.slk",
    ] {
        match extract_unique(set, path) {
            Ok(bytes) => put(path, None, bytes)?,
            Err(e) if e.starts_with("AMBIGUOUS_MEMBER:") => return Err(e),
            Err(e) => notes.push(e),
        }
    }
    notes.push("Edition/build and archive priority remain unverified; only unique or byte-identical members were resolved".into());
    Ok((assets.finish(), notes))
}

/// Hydrate renderer contracts and authored dependencies before replacing the
/// visible store. A failure retains the previous store and revision unchanged.
#[wasm_bindgen]
pub fn commit_archive_import() -> Result<Vec<String>, JsValue> {
    let mut guard = pending();
    let set = guard
        .as_mut()
        .ok_or_else(|| JsValue::from_str("no active archive import"))?;
    let (assets, notes) = hydrate(set).map_err(|e| JsValue::from_str(&e))?;
    let editions = crate::local_demo::MenuEditions {
        roc: assets.contains_key(&normalize(crate::mainmenu_scene::scene_source(false).1)),
        tft: assets.contains_key(&normalize(crate::mainmenu_scene::scene_source(true).1)),
    };
    let mut rev = revision();
    let next = rev
        .checked_add(1)
        .ok_or_else(|| JsValue::from_str("asset revision exhausted"))?;
    clear_store();
    *archives() = guard
        .take()
        .ok_or_else(|| JsValue::from_str("import transaction disappeared"))?;
    for (path, bytes) in assets {
        add_asset(path, bytes);
    }
    client_images().publish(next);
    *rev = next;
    let mut control = demo_control();
    control.enabled = true;
    control.editions = editions;
    control.requested = None;
    control.active = None;
    drop(control);
    drop(rev);
    drop(guard);
    stop_menu_events();
    for line in &notes {
        note(line);
    }
    Ok(notes)
}

// --------------------------------------------------------- launcher wiring ---

/// Replaces the platform default asset source with the in-memory one and starts
/// the note pump.
///
/// Must run *before* the plugins: Bevy builds an app's asset sources while
/// `AssetPlugin` initialises, and a registered default source is kept rather
/// than overwritten with the platform one.
fn configure(app: &mut App) {
    peacecraft3::browser_configuration::install(app, map_configuration_inbox().clone());
    app.register_asset_source(
        AssetSourceId::Default,
        // 0.20: `AssetSourceBuilder::new` takes the reader directly;
        // `default().with_reader(..)` is gone.
        AssetSourceBuilder::new(|| {
            Box::new(ArchiveAssetReader {
                memory: MemoryAssetReader { root: store() },
            }) as Box<dyn ErasedAssetReader>
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
/// outcome rather than claiming every member can decode: unsupported payload
/// codecs are reported by named extraction, independently of table parsing.
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
            launch_gui::app(launch_gui::window_plugin(Some(CANVAS)), configure)
                .add_plugins(pcx_gfx::performance::PerformanceOverlayPlugin)
                .run();
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

/// Opens and retains one archive for explicit named extraction. File order does
/// not establish priority. ISO imports use the transactional staging API.
#[wasm_bindgen]
pub fn add_archive(name: String, bytes: Vec<u8>) -> String {
    let key = normalize(&name);
    if key.is_empty() {
        return format!("rejected: {name:?} is not a path");
    }
    let line = match pcx_mpq::MpqArchive::from_bytes(bytes, &key) {
        Ok(mut archive) => {
            let line = describe(&key, &mut archive);
            archives().push(archive);
            line
        }
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

/// A bounded copy from the browser's Rust memory store, with its exact key and
/// current committed revision. Hydrated source origins are unavailable.
#[wasm_bindgen]
pub struct StoredAssetRead {
    asset: crate::browser_read::AssetRead,
}
#[wasm_bindgen]
impl StoredAssetRead {
    #[wasm_bindgen(getter)]
    pub fn key(&self) -> String {
        self.asset.receipt.key.clone()
    }
    #[wasm_bindgen(getter)]
    pub fn requested_path(&self) -> String {
        self.asset.receipt.requested_path.clone()
    }
    #[wasm_bindgen(getter)]
    pub fn revision(&self) -> u32 {
        self.asset.receipt.revision
    }
    #[wasm_bindgen(getter)]
    pub fn byte_len(&self) -> usize {
        self.asset.receipt.byte_len
    }
    #[wasm_bindgen(getter)]
    pub fn archive_origins_available(&self) -> bool {
        self.asset.receipt.archives.is_some()
    }
    /// Ordered contributing source labels; availability distinguishes cached data.
    pub fn archive_names(&self) -> Vec<String> {
        self.asset
            .receipt
            .archives
            .as_ref()
            .map(|origins| origins.iter().map(|origin| origin.name.clone()).collect())
            .unwrap_or_default()
    }
    pub fn archive_ordinals(&self) -> Vec<u32> {
        self.asset
            .receipt
            .archives
            .as_ref()
            .map(|origins| origins.iter().map(|origin| origin.ordinal as u32).collect())
            .unwrap_or_default()
    }
    pub fn into_bytes(self) -> Vec<u8> {
        self.asset.bytes
    }
}
/// Missing returns undefined; invalid, oversized or present uncached content
/// throws an Error. No JS byte input is accepted or copied by this API.
#[wasm_bindgen]
pub fn stored_asset_checked(
    path: String,
    max_bytes: usize,
) -> Result<Option<StoredAssetRead>, JsError> {
    stored_bytes_checked(&path, max_bytes)
        .map(|asset| asset.map(|asset| StoredAssetRead { asset }))
        .map_err(|error| JsError::new(&error.to_string()))
}
pub(crate) fn stored_bytes_checked(
    path: &str,
    max_bytes: usize,
) -> Result<Option<crate::browser_read::AssetRead>, crate::browser_read::ReadError> {
    let revision = *revision();
    crate::browser_read::read_checked(&store(), &archives(), revision, path, max_bytes)
}

/// Bounded internal screen-definition read; no implicit codec discovery or cache write.
pub(crate) fn screen_bytes(path: &str, max_bytes: usize) -> Result<Option<Vec<u8>>, String> {
    panel_bytes(path, max_bytes)
        .map(|asset| asset.map(|asset| asset.bytes))
        .map_err(|e| e.to_string())
}

/// Typed bounded reads let panel audio capability remain separate from graphics.
pub(crate) fn panel_bytes(
    path: &str,
    max_bytes: usize,
) -> Result<Option<crate::browser_read::AssetRead>, crate::browser_read::ReadError> {
    let revision = *revision();
    crate::browser_read::read_bounded(
        &store(),
        &mut archives(),
        revision,
        path,
        crate::browser_read::BoundedReadPolicy {
            max_copy_bytes: max_bytes,
            extraction: pcx_mpq::ExtractionBudget {
                max_raw_packed_bytes: max_bytes,
                max_decoded_bytes: max_bytes,
            },
            huffman: None,
        },
    )
}

/// Reject non-number, non-finite, fractional and out-of-range arguments before
/// any store/archive read. JsValue prevents generated numeric wrap/coercion.
fn browser_budget(value: &JsValue, field: &str) -> Result<usize, JsError> {
    let number = value
        .as_f64()
        .ok_or_else(|| JsError::new(&format!("{field}: budget must be a number")))?;
    if !number.is_finite() || number.fract() != 0.0 || number < 0.0 || number > usize::MAX as f64 {
        return Err(JsError::new(&format!(
            "{field}: budget must be a finite integer within 0..={}",
            usize::MAX
        )));
    }
    Ok(number as usize)
}
/// Explicit bounded bridge to the existing committed store and archives.
/// The caller must choose exactly "no-huffman"; streams needing a supplied table
/// refuse without consulting an installed table or discovering an executable.
#[wasm_bindgen]
pub fn stored_asset_bounded(
    path: String,
    copy_bytes: JsValue,
    packed_bytes: JsValue,
    decoded_bytes: JsValue,
    huffman_policy: JsValue,
) -> Result<Option<StoredAssetRead>, JsError> {
    let max_copy_bytes = browser_budget(&copy_bytes, "copy")?;
    let max_raw_packed_bytes = browser_budget(&packed_bytes, "packed")?;
    let max_decoded_bytes = browser_budget(&decoded_bytes, "decoded")?;
    if huffman_policy.as_string().as_deref() != Some("no-huffman") {
        return Err(JsError::new(
            "huffman policy must be explicitly no-huffman; supplied-table JS binding is unsupported",
        ));
    }
    let policy = crate::browser_read::BoundedReadPolicy {
        max_copy_bytes,
        extraction: pcx_mpq::ExtractionBudget {
            max_raw_packed_bytes,
            max_decoded_bytes,
        },
        huffman: None,
    };
    let revision = *revision();
    let result =
        crate::browser_read::read_bounded(&store(), &mut archives(), revision, &path, policy)
            .map_err(|error| {
                let mut message = format!("request {path:?}: {error}");
                let origins = match &error {
                    crate::browser_read::ReadError::BoundedExtraction {
                        matching_archives, ..
                    }
                    | crate::browser_read::ReadError::BoundedAmbiguous {
                        matching_archives, ..
                    } => matching_archives.as_slice(),
                    _ => &[],
                };
                for origin in origins {
                    message.push_str(&format!("; matching {origin}"));
                }
                JsError::new(&message)
            })?;
    // No implicit cache publication: the returned receipt retains all origins.
    Ok(result.map(|asset| StoredAssetRead { asset }))
}

/// Compatible fallible fallback for callers that explicitly accept the legacy
/// archive decoder's allocation contract. Only Ok(None) means missing.
pub(crate) fn stored_bytes_fallible(
    path: &str,
) -> Result<Option<crate::browser_read::AssetRead>, crate::browser_read::ReadError> {
    let revision = *revision();
    if let Some(asset) =
        crate::browser_read::read_checked(&store(), &[], revision, path, usize::MAX)?
    {
        return Ok(Some(asset));
    }
    let asset = crate::browser_read::extract_fallible(
        &mut archives(),
        revision,
        path,
        MEMBER_LIMIT as usize,
    )?;
    if let Some(asset) = &asset {
        add_asset(path.to_owned(), asset.bytes.clone());
    }
    Ok(asset)
}

/// Compatible menu wrapper. Diagnostic consumers select the fallible or checked API.
pub(crate) fn stored_bytes(path: &str) -> Option<Vec<u8>> {
    match stored_bytes_fallible(path) {
        Ok(asset) => asset.map(|asset| asset.bytes),
        Err(error) => {
            note(error.to_string());
            None
        }
    }
}

/// Drops everything in memory and reports how many entries went.
#[wasm_bindgen]
pub fn drop_assets() -> usize {
    archives().clear();
    *pending() = None;
    let count = clear_store();
    let mut rev = revision();
    client_images().clear_all();
    *rev = rev.checked_add(1).expect("asset revision exhausted");
    let mut control = demo_control();
    control.editions = crate::local_demo::MenuEditions::default();
    control.requested = None;
    control.active = None;
    drop(control);
    drop(rev);
    stop_menu_events();
    count
}

fn clear_store() -> usize {
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
        assert_eq!(
            stored_assets(),
            vec!["war3.mpq".to_string(), "WAR3.MPQ".to_string()]
        );
        assert_eq!(drop_assets(), 2);
        assert!(stored_assets().is_empty());
    }

    #[test]
    fn client_image_publishes_once_and_cancel_keeps_the_prior_image() {
        let mut slot = ClientImageTxn::default();
        slot.clear_pending();
        slot.stage(true, "archive::game.dll".into(), b"one".to_vec())
            .unwrap();
        slot.publish(3);
        assert_eq!(
            slot.committed_at(3).unwrap(),
            ("archive::game.dll", b"one".as_slice())
        );
        assert!(slot.committed_at(2).is_none());

        slot.clear_pending();
        slot.stage(true, "archive::next.dll".into(), b"two".to_vec())
            .unwrap();
        slot.clear_pending();
        assert_eq!(slot.committed_at(3).unwrap().1, b"one");

        slot.clear_pending();
        slot.stage(true, "archive::next.dll".into(), b"two".to_vec())
            .unwrap();
        assert_eq!(
            slot.stage(true, "archive::third.dll".into(), b"three".to_vec()),
            Err("client image is already staged")
        );
        slot.publish(4);
        assert_eq!(slot.committed_at(4).unwrap().1, b"two");
        assert!(slot.committed_at(3).is_none());

        slot.clear_pending();
        slot.publish(5);
        assert!(slot.committed_at(5).is_none());
        assert!(slot.stage(false, "late".into(), b"x".to_vec()).is_err());
        assert_eq!(
            client_image_input("fixture", CLIENT_IMAGE_BYTES + 1),
            Err("Options client exceeds image budget")
        );
        assert_eq!(client_image_input("  ", 1), Err("missing source identity"));
        assert_eq!(
            client_image_input("fixture", 0),
            Err("client image is empty")
        );
    }

    #[wasm_bindgen]
    pub fn browser_import_cancel_failure_gate() -> u32 {
        failed_commit_keeps_the_staged_image_off_the_store_and_revision();
        1
    }

    #[test]
    fn failed_commit_keeps_the_staged_image_off_the_store_and_revision() {
        let _ = drop_assets();
        let revision = asset_revision();
        let stored = stored_assets();
        {
            let mut slot = client_images();
            slot.clear_pending();
            slot.stage(true, "prior::game.dll".into(), b"prior-bytes".to_vec())
                .unwrap();
            slot.publish(revision);
        }
        let prior =
            with_committed_client_image(|source, bytes| (source.to_owned(), bytes.to_vec()));
        assert_eq!(prior.unwrap().1, b"prior-bytes");
        assert_eq!(
            committed_client_image_identity().as_deref(),
            Some("prior::game.dll")
        );

        begin_archive_import();
        stage_client_image("next::game.dll".into(), b"next-bytes".to_vec()).unwrap();
        assert_eq!(stored_assets(), stored);
        assert!(commit_archive_import().is_err());
        assert_eq!(asset_revision(), revision);
        assert_eq!(stored_assets(), stored);
        let kept = with_committed_client_image(|source, bytes| (source.to_owned(), bytes.to_vec()));
        assert_eq!(
            kept.unwrap(),
            ("prior::game.dll".to_owned(), b"prior-bytes".to_vec())
        );

        cancel_archive_import();
        assert!(stage_client_image("late".into(), b"x".to_vec()).is_err());
        let still = with_committed_client_image(|_source, bytes| bytes.to_vec());
        assert_eq!(still.unwrap(), b"prior-bytes");
        let _ = drop_assets();
        assert!(with_committed_client_image(|_, _| ()).is_none());
        assert!(committed_client_image_identity().is_none());
    }

    #[test]
    fn an_archive_that_is_not_an_mpq_is_reported_not_stored() {
        let line = add_archive("not-an-mpq.bin".into(), b"certainly not an MPQ".to_vec());
        assert!(line.contains("would not open"), "{line}");
        assert!(stored_assets().is_empty(), "a bad archive must not land");
    }
}

//! Supported user preferences. Slider units come from the current FDF widgets.
//! Qualified display rows come from the selected client's mode profile and the
//! primary monitor. A selector-2 popup profile lists every current monitor's
//! full rectangle. On the browser, that profile lists logical canvas rows for
//! one physical screen observation. Otherwise the list is measured render sizes.
use crate::menu_screens::{
    Navigation, OfferedResolution, SettingControl, SettingValue, Settings, Widget,
};
use bevy::{
    prelude::*,
    ui::{Checked, InteractionDisabled},
    ui_widgets::{
        Checkbox, Slider, SliderOrientation, SliderRange, SliderStep, SliderValue, TrackClick,
        ValueChange,
    },
    window::{Monitor, OnMonitor, PrimaryMonitor, PrimaryWindow},
};
use std::collections::BTreeMap;

#[path = "menu_resolution.rs"]
mod resolution;

#[path = "menu_slider.rs"]
mod slider;

#[cfg(test)]
#[path = "menu_picking_tests.rs"]
mod picking_tests;

#[cfg(any(test, not(target_arch = "wasm32")))]
#[path = "original_options_audio.rs"]
mod original;

#[path = "original_options_preview.rs"]
pub(crate) mod preview;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct SettingsUpdate;

// Local preference input policy, independent of game assets or authored values.
const MAX_PREFERENCE_BYTES: usize = 8192;

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioSettings {
    pub music_gain: f32,
    pub effects_gain: f32,
    pub music_enabled: bool,
    pub effects_enabled: bool,
}
impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            music_gain: 1.0,
            effects_gain: 1.0,
            music_enabled: true,
            effects_enabled: true,
        }
    }
}
impl AudioSettings {
    pub fn effective_music(self) -> f32 {
        if self.music_enabled {
            self.music_gain
        } else {
            0.0
        }
    }
    pub fn effective_effects(self) -> f32 {
        if self.effects_enabled {
            self.effects_gain
        } else {
            0.0
        }
    }
    fn validate(self) -> Result<(), String> {
        if [self.music_gain, self.effects_gain]
            .into_iter()
            .all(|v| v.is_finite() && (0.0..=1.0).contains(&v))
        {
            Ok(())
        } else {
            Err("Saved audio gain is outside 0..1".into())
        }
    }
}
#[derive(Clone, Copy)]
struct Range {
    min: f32,
    max: f32,
    step: f32,
}
impl Range {
    fn valid(self, value: f32) -> bool {
        value.is_finite()
            && self.min.is_finite()
            && self.max.is_finite()
            && self.step.is_finite()
            && self.min < self.max
            && (self.max - self.min).is_finite()
            && self.step > 0.0
            && (self.min..=self.max).contains(&value)
    }
    fn gain(self, value: f32) -> Result<f32, String> {
        if self.valid(value) {
            Ok((value - self.min) / (self.max - self.min))
        } else {
            Err("Volume is outside its authored slider range".into())
        }
    }
    fn value(self, gain: f32) -> f32 {
        (f64::from(self.min) + f64::from(gain) * (f64::from(self.max) - f64::from(self.min))) as f32
    }
    fn snap(self, value: f32) -> f32 {
        crate::menu_screens::slider_value(
            self.min,
            self.max,
            self.step,
            (value - self.min) / (self.max - self.min),
        )
    }
}
#[derive(Default)]
struct DisplayAdmission {
    #[cfg(not(target_arch = "wasm32"))]
    path: String,
    #[cfg(target_arch = "wasm32")]
    revision: Option<u32>,
    expansion: bool,
    state: DisplayState,
    #[cfg(not(target_arch = "wasm32"))]
    announced: bool,
    #[cfg(not(target_arch = "wasm32"))]
    empty_noted: bool,
}

#[derive(Default)]
enum DisplayState {
    #[default]
    Unchecked,
    Unqualified {
        detail: Option<String>,
    },
    Qualified(std::sync::Arc<pcx_play::options_modes::Policy>),
}

#[derive(Resource, Default)]
pub struct Backend {
    pub audio: AudioSettings,
    ranges: BTreeMap<String, Range>,
    loaded: bool,
    has_preferences: bool,
    #[cfg(not(target_arch = "wasm32"))]
    saved_resolution: bool,
    preference_unreadable: Option<String>,
    #[cfg(not(target_arch = "wasm32"))]
    source: Option<(String, crate::launch::GameMode)>,
    pending_resolution: Option<(u32, u32)>,
    observed_resolution: Option<(u32, u32)>,
    resize: Option<resolution::Pending>,
    display: DisplayAdmission,
}
impl Backend {
    pub(crate) fn applying_resolution(&self) -> bool {
        self.resize.is_some()
    }
    pub(crate) fn cancel_resolution(&mut self) -> bool {
        resolution::cancel(self)
    }
}

pub(crate) enum ApplyOutcome {
    Complete,
    Pending,
}

struct Prepared {
    settings: Settings,
    audio: AudioSettings,
    text: String,
}
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Preferences {
    version: u8,
    settings: Settings,
    audio: AudioSettings,
}

fn validate_settings(settings: &Settings) -> Result<(), String> {
    if settings.resolution.is_some_and(|(w, h)| w == 0 || h == 0) {
        return Err("Saved display dimensions must be positive".into());
    }
    for (name, value) in &settings.values {
        if !matches!(name.as_str(), "MusicVolumeSlider" | "SoundVolumeSlider") || !value.is_finite()
        {
            return Err(format!("Unsupported or invalid saved setting {name}"));
        }
    }
    if settings
        .toggles
        .keys()
        .any(|name| !matches!(name.as_str(), "MusicCheckBox" | "SoundCheckBox"))
    {
        return Err("Saved settings contain an unsupported toggle".into());
    }
    Ok(())
}
fn decode_preferences(text: &str) -> Result<Preferences, String> {
    if text.len() > MAX_PREFERENCE_BYTES {
        return Err("Saved settings exceed the preference byte limit".into());
    }
    let prefs: Preferences =
        serde_json::from_str(text).map_err(|e| format!("Saved settings are malformed: {e}"))?;
    if prefs.version != 1 {
        return Err("Saved settings use an unsupported format version".into());
    }
    validate_settings(&prefs.settings)?;
    prefs.audio.validate()?;
    Ok(prefs)
}
impl Backend {
    #[cfg(all(test, not(target_arch = "wasm32")))]
    pub(crate) fn set_test_preferences(&mut self, saved: bool) {
        self.loaded = true;
        self.has_preferences = saved;
    }

    #[cfg(all(test, not(target_arch = "wasm32")))]
    pub(crate) fn set_test_saved_resolution(&mut self, saved: bool) {
        self.set_test_preferences(true);
        self.saved_resolution = saved;
    }

    #[cfg(all(test, not(target_arch = "wasm32")))]
    pub(crate) fn set_test_source(&mut self, launcher: &crate::launch_gui::Launcher) {
        let (source, mode) = launcher.audio_selection();
        self.source = Some((source.to_owned(), mode));
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn preferences_loaded(&self) -> bool {
        self.loaded && self.preference_unreadable.is_none()
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn source_matches(&self, launcher: &crate::launch_gui::Launcher) -> bool {
        let (source, mode) = launcher.audio_selection();
        self.source
            .as_ref()
            .is_some_and(|(path, selected)| path == source && *selected == mode)
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn has_preferences(&self) -> bool {
        self.has_preferences
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn has_saved_resolution(&self) -> bool {
        self.saved_resolution
    }
    fn candidate_audio(
        &self,
        settings: &Settings,
        previous: &Settings,
    ) -> Result<AudioSettings, String> {
        let mut audio = self.audio;
        for (name, target) in [
            ("MusicVolumeSlider", &mut audio.music_gain),
            ("SoundVolumeSlider", &mut audio.effects_gain),
        ] {
            if let Some(&value) = settings.values.get(name) {
                if let Some(range) = self.ranges.get(name) {
                    let gain = range.gain(value)?;
                    if previous.values.get(name) != Some(&value) {
                        *target = gain;
                    }
                } else if previous.values.get(name) != Some(&value) {
                    return Err(format!("{name}: authored slider range is not loaded"));
                }
            }
        }
        if let Some(value) = settings.toggles.get("MusicCheckBox") {
            audio.music_enabled = *value;
        }
        if let Some(value) = settings.toggles.get("SoundCheckBox") {
            audio.effects_enabled = *value;
        }
        audio.validate()?;
        Ok(audio)
    }
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn admit_source_audio(
    nav: &mut Navigation,
    backend: &mut Backend,
    seed: Option<original::Seed>,
) -> Result<(), String> {
    if !backend.loaded {
        return Err("Audio preferences have not been read yet".into());
    }
    if let Some(error) = &backend.preference_unreadable {
        return Err(format!("Audio preferences could not be read: {error}"));
    }
    let (audio, ranges, mut values, mut toggles) = if let Some(seed) = seed {
        info!(source=%seed.origin.source, edition=%seed.origin.edition,
              image_sha256=?seed.image_sha256, "Source-qualified Options audio fallback admitted");
        (seed.audio, seed.ranges, seed.values, seed.toggles)
    } else {
        (
            AudioSettings::default(),
            BTreeMap::new(),
            BTreeMap::new(),
            BTreeMap::new(),
        )
    };
    let audio = if backend.has_preferences {
        backend.audio
    } else {
        audio
    };
    if backend.has_preferences {
        for (name, range) in &ranges {
            let gain = if name == "MusicVolumeSlider" {
                audio.music_gain
            } else {
                audio.effects_gain
            };
            values.insert(name.clone(), range.value(gain));
        }
        toggles.insert("MusicCheckBox".into(), audio.music_enabled);
        toggles.insert("SoundCheckBox".into(), audio.effects_enabled);
    }
    let mut applied = nav.applied.clone();
    applied.values = values;
    applied.toggles = toggles;
    backend.audio = audio;
    backend.ranges = ranges;
    nav.applied = applied;
    nav.draft = nav.applied.clone();
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn with_source_defaults<T>(
    install: &std::path::Path,
    expansion: bool,
    consume: impl FnOnce(&pcx_play::options_defaults::Defaults<'_>) -> Result<T, String>,
) -> Result<Option<T>, String> {
    use pcx_play::{
        neutral_player::RuleInputBudget,
        options_defaults::{Defaults, DiscoveryBudget, Problem},
    };
    use std::io::Read;
    // Supported client role; no neighboring DLL, executable or default table is borrowed.
    let path = install.join("Warcraft III.exe");
    let mut file = match std::fs::File::open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("{}: {error}", path.display())),
    };
    // Local input/work policy, independent of any original image's measured size.
    const IMAGE_BYTES: usize = 32 * 1024 * 1024;
    if file
        .metadata()
        .map_err(|e| format!("{}: {e}", path.display()))?
        .len()
        > IMAGE_BYTES as u64
    {
        return Err(format!(
            "{}: Options client exceeds image budget",
            path.display()
        ));
    }
    let mut image = Vec::new();
    image
        .try_reserve_exact(IMAGE_BYTES + 1)
        .map_err(|e| e.to_string())?;
    file.by_ref()
        .take((IMAGE_BYTES + 1) as u64)
        .read_to_end(&mut image)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let origin = pcx_play::scripts::AssetIdentity {
        edition: format!("selected skin V{}", u32::from(expansion)),
        version: "pattern-qualified PE32; release identity not inferred".into(),
        source: path.display().to_string(),
    };
    let defaults = match Defaults::discover(
        &image,
        origin,
        DiscoveryBudget {
            image: RuleInputBudget {
                image_bytes: IMAGE_BYTES,
                profile_bytes: 128 * 1024,
                sections: 96,
            },
            string_bytes: 4096,
            candidate_limit: 32,
            work_bytes: 64 * 1024 * 1024,
        },
    ) {
        Ok(defaults) => defaults,
        Err(error)
            if matches!(
                error.problem,
                Problem::DiscoveryNone
                    | Problem::Image(pcx_play::neutral_player::NeutralError::Rule(
                        pcx_play::neutral_player::RuleProblem::UnsupportedArchitecture { .. }
                    ))
            ) =>
        {
            return Ok(None);
        }
        Err(error) => return Err(error.to_string()),
    };
    consume(&defaults).map(Some)
}

#[cfg(not(target_arch = "wasm32"))]
fn prepare_source_audio(
    install: &std::path::Path,
    expansion: bool,
) -> Result<Option<original::Seed>, String> {
    with_source_defaults(install, expansion, |defaults| {
        let (fdf, _) = crate::launch_gui::read_screen_fdf(
            Some(install),
            crate::menu_screens::Screen::Options(crate::menu_screens::Page::Sound),
        )?;
        original::prepare(defaults, &fdf)
    })
}
/// Optional presentation: a discovery failure never enters the audio transaction.
pub(crate) fn source_preview<'a>(
    install: Option<&std::path::Path>,
    expansion: bool,
    controls: impl IntoIterator<Item = (&'a str, &'a Widget)>,
    strings: Option<&str>,
    key: String,
) -> preview::Snapshot {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (install, expansion, controls, strings);
        preview::Snapshot::unavailable(
            key,
            "Archive-only sources have no admitted client defaults.",
        )
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let Some(install) = install else {
            return preview::Snapshot::unavailable(key, "No selected client source is available.");
        };
        let Some(strings) = strings else {
            return preview::Snapshot::unavailable(
                key,
                "The selected localization is unavailable.",
            );
        };
        match with_source_defaults(install, expansion, |defaults| {
            Ok(preview::Snapshot::checked(
                defaults,
                controls,
                strings,
                key.clone(),
            ))
        }) {
            Ok(Some(snapshot)) => snapshot,
            Ok(None) => preview::Snapshot::unavailable(
                key,
                "The selected client has no supported defaults profile.",
            ),
            Err(error) => preview::Snapshot::unavailable(
                key,
                &format!("Source defaults could not be admitted: {error}"),
            ),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn source_audio_candidate(
    backend: &Backend,
    install: Option<&std::path::Path>,
    expansion: bool,
) -> Result<Option<original::Seed>, String> {
    if backend.has_preferences {
        Ok(None)
    } else {
        install
            .map(|path| prepare_source_audio(path, expansion))
            .transpose()
            .map(Option::flatten)
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn activate_source_audio(
    nav: &mut Navigation,
    backend: &mut Backend,
    seed: Option<original::Seed>,
    launcher: &crate::launch_gui::Launcher,
) -> Result<(), String> {
    if seed.is_none() && !backend.has_preferences {
        warn!(
            "Source Options defaults unavailable; using explicitly unqualified neutral menu-audio multipliers"
        );
    }
    admit_source_audio(nav, backend, seed)?;
    let (source, mode) = launcher.audio_selection();
    backend.source = Some((source.to_owned(), mode));
    Ok(())
}

#[cfg(test)]
pub fn select_resolution(nav: &mut Navigation, resolution: (u32, u32)) -> Result<(), String> {
    let Some(offer) = nav
        .resolutions
        .iter()
        .find(|row| row.size == resolution)
        .cloned()
    else {
        return Err("That resolution is no longer available on this display".into());
    };
    select_offer(nav, &offer)
}

pub fn select_offer(nav: &mut Navigation, offer: &OfferedResolution) -> Result<(), String> {
    if !nav.resolutions.iter().any(|row| row.same_mode(offer)) {
        return Err("That resolution is no longer available on this display".into());
    }
    nav.selected_offer = Some(offer.clone());
    nav.draft.resolution = Some(offer.size);
    Ok(())
}
/// Save only after Winit reports the requested client size. This is not a
/// compositor presentation or physical display-mode acknowledgement.
pub(crate) fn apply(
    entity: Entity,
    nav: &mut Navigation,
    window: &mut Window,
    backend: &mut Backend,
    launcher: &crate::launch_gui::Launcher,
) -> Result<ApplyOutcome, String> {
    resolution::apply(entity, nav, window, backend, launcher)
}
fn prepare(nav: &Navigation, backend: &Backend) -> Result<Prepared, String> {
    validate_settings(&nav.draft)?;
    if let Some(size) = nav.draft.resolution {
        if !nav.offers_size(size) && backend.observed_resolution != Some(size) {
            return Err("Selected resolution is unavailable on this display".into());
        }
        if let Some(selected) = &nav.selected_offer
            && selected.size == size
            && !nav.resolutions.iter().any(|row| row.same_mode(selected))
        {
            return Err("Selected resolution is unavailable on this display".into());
        }
    }
    let audio = backend.candidate_audio(&nav.draft, &nav.applied)?;
    let text = serde_json::to_string(&Preferences {
        version: 1,
        settings: nav.draft.clone(),
        audio,
    })
    .map_err(|e| format!("Saving settings: {e}"))?;
    if text.len() > MAX_PREFERENCE_BYTES {
        return Err("Settings exceed the preference byte limit".into());
    }
    Ok(Prepared {
        settings: nav.draft.clone(),
        audio,
        text,
    })
}
fn publish(
    candidate: &Prepared,
    nav: &mut Navigation,
    backend: &mut Backend,
    save: impl FnOnce(&str) -> Result<(), String>,
) -> Result<Option<String>, String> {
    save(&candidate.text)?;
    nav.applied = candidate.settings.clone();
    nav.draft = nav.applied.clone();
    backend.audio = candidate.audio;
    backend.has_preferences = true;
    backend.loaded = true;
    backend.preference_unreadable = None;
    #[cfg(target_arch = "wasm32")]
    if let Err(error) = push_browser_audio(backend.audio) {
        return Ok(Some(format!(
            "Settings saved; render size confirmed, but {error}"
        )));
    }
    Ok(None)
}
fn commit(
    nav: &mut Navigation,
    window: &mut Window,
    backend: &mut Backend,
    save: impl FnOnce(&str) -> Result<(), String>,
) -> Result<(), String> {
    let candidate = prepare(nav, backend)?;
    if candidate
        .settings
        .resolution
        .is_some_and(|size| backend.observed_resolution != Some(size))
    {
        return Err("Render size is not confirmed; settings were not saved".into());
    }
    let size = candidate.settings.resolution;
    let warning = publish(&candidate, nav, backend, save)?;
    if let Some((width, height)) = size {
        #[cfg(target_arch = "wasm32")]
        {
            window.fit_canvas_to_parent = false;
        }
        window.resolution.set_physical_resolution(width, height);
    }
    warning.map_or(Ok(()), Err)
}

pub fn install(app: &mut App) {
    app.init_resource::<Backend>()
        .add_systems(First, resolution::poll)
        .add_observer(slider_changed)
        .add_observer(checkbox_changed)
        .add_systems(
            Update,
            (environment, attach_controls, labels)
                .chain()
                .in_set(SettingsUpdate)
                .before(crate::launch_gui::LauncherInput),
        );
}
fn observe_resolution(nav: &mut Navigation, backend: &mut Backend, current: (u32, u32)) {
    let changed = backend.observed_resolution.replace(current) != Some(current);
    if changed && backend.pending_resolution.is_none() && backend.resize.is_none() {
        let previous = nav.applied.resolution.replace(current);
        if nav.draft.resolution == previous {
            nav.draft.resolution = Some(current);
            if previous != Some(current) {
                nav.selected_offer = None;
            }
        }
    }
}

fn render_offers(mut modes: Vec<(u32, u32)>) -> Vec<OfferedResolution> {
    modes.retain(|(width, height)| *width > 0 && *height > 0);
    modes.sort_unstable();
    modes.dedup();
    modes
        .into_iter()
        .map(|(width, height)| OfferedResolution::render_size(width, height))
        .collect()
}

#[cfg(not(target_arch = "wasm32"))]
fn offered_from_row(row: &pcx_play::options_modes::Row) -> OfferedResolution {
    OfferedResolution {
        label: row.label.clone(),
        size: (row.width, row.height),
        depth: row.depth,
        refresh_millihertz: row.refresh_millihertz,
        adapter: row.adapter,
        mode_index: row.mode_index,
        kind: row.kind,
        flag: row.flag,
        monitor_ordinal: row.monitor_ordinal,
        monitor: None,
        render_extent_only: false,
    }
}

fn catalog_row(offer: &OfferedResolution) -> pcx_play::options_modes::Row {
    pcx_play::options_modes::Row {
        label: offer.label.clone(),
        width: offer.size.0,
        height: offer.size.1,
        depth: offer.depth,
        refresh_millihertz: offer.refresh_millihertz,
        adapter: offer.adapter,
        mode_index: offer.mode_index,
        kind: offer.kind,
        flag: offer.flag,
        monitor_ordinal: offer.monitor_ordinal,
    }
}

fn first_exact(rows: &[OfferedResolution], size: (u32, u32)) -> Option<OfferedResolution> {
    rows.iter().find(|row| row.size == size).cloned()
}

fn same_monitor_mode(selected: &OfferedResolution, row: &OfferedResolution) -> bool {
    selected.monitor.is_some()
        && selected.monitor == row.monitor
        && selected.size == row.size
        && selected.depth == row.depth
        && selected.refresh_millihertz == row.refresh_millihertz
        && selected.adapter == row.adapter
        && selected.mode_index == row.mode_index
        && selected.kind == row.kind
        && selected.flag == row.flag
        && selected.render_extent_only == row.render_extent_only
}

fn choose_offer(nav: &mut Navigation, policy: Option<&pcx_play::options_modes::Policy>) {
    let Some(size) = nav.draft.resolution else {
        nav.selected_offer = None;
        return;
    };
    if let Some(selected) = nav.selected_offer.clone()
        && selected.size == size
        && selected.monitor.is_some()
    {
        if let Some(row) = nav
            .resolutions
            .iter()
            .find(|row| same_monitor_mode(&selected, row))
        {
            nav.selected_offer = Some(row.clone());
        }
        // A missing entity keeps the witness and draft.resolution. prepare refuses Apply.
        return;
    }
    if let Some(selected) = nav.selected_offer.clone()
        && selected.size == size
        && let Some(row) = nav.resolutions.iter().find(|row| row.same_mode(&selected))
    {
        nav.selected_offer = Some(row.clone());
        return;
    }
    if let Some(policy) = policy
        && nav.resolutions.iter().any(|row| !row.render_extent_only)
        && let Some(row) = scored_offer(policy, &nav.resolutions, size)
    {
        nav.selected_offer = Some(row);
        return;
    }
    nav.selected_offer = first_exact(&nav.resolutions, size);
}

fn scored_offer(
    policy: &pcx_play::options_modes::Policy,
    rows: &[OfferedResolution],
    size: (u32, u32),
) -> Option<OfferedResolution> {
    let qualified: Vec<_> = rows
        .iter()
        .filter(|row| !row.render_extent_only)
        .map(catalog_row)
        .collect();
    let (Ok(width), Ok(height)) = (i32::try_from(size.0), i32::try_from(size.1)) else {
        return first_exact(rows, size);
    };
    // Recreation presentation policy for selector-2 rows on the render-extent
    // path. The active-kind compare and the adapter-owner field stay unread.
    let kind = policy
        .popup_presentation_kind()
        .unwrap_or_else(|| policy.kind());
    let query = pcx_play::options_modes::Query {
        width,
        height,
        depth: 0,
        refresh: 0,
        adapter: qualified.first()?.adapter,
        kind,
        flag: 0,
    };
    let winner = policy
        .select(&qualified, query)
        .ok()
        .flatten()
        .and_then(|index| qualified.get(index))
        .filter(|row| row.width == size.0 && row.height == size.1);
    winner
        .and_then(|row| {
            rows.iter().find(|offer| {
                !offer.render_extent_only
                    && offer.size == (row.width, row.height)
                    && offer.depth == row.depth
                    && offer.refresh_millihertz == row.refresh_millihertz
                    && offer.adapter == row.adapter
                    && offer.mode_index == row.mode_index
                    && offer.kind == row.kind
                    && offer.flag == row.flag
                    && offer.monitor_ordinal == row.monitor_ordinal
            })
        })
        .cloned()
        .or_else(|| first_exact(rows, size))
}

#[cfg(not(target_arch = "wasm32"))]
fn observed_depth(bits: u16) -> Option<u32> {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        (bits != 0).then_some(u32::from(bits))
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = bits;
        None
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn session_adapter(policy: &pcx_play::options_modes::Policy) -> Option<u32> {
    (1..64u32).find(|&candidate| {
        i32::try_from(candidate)
            .ok()
            .is_some_and(|signed| signed != policy.sentinel() && signed != policy.wildcard())
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn qualified_offers(
    policy: &pcx_play::options_modes::Policy,
    monitor: &Monitor,
) -> Result<Vec<OfferedResolution>, String> {
    let Some(adapter) = session_adapter(policy) else {
        return Err("Display mode profile has no usable session monitor token".into());
    };
    let mut records = Vec::new();
    for (index, mode) in monitor.video_modes.iter().enumerate() {
        let Ok(mode_index) = u32::try_from(index) else {
            return Err("Display mode list exceeds its index limit".into());
        };
        records.push(pcx_play::options_modes::HostRecord {
            width: mode.physical_size.x,
            height: mode.physical_size.y,
            depth: observed_depth(mode.bit_depth),
            refresh_millihertz: (mode.refresh_rate_millihertz != 0)
                .then_some(mode.refresh_rate_millihertz),
            adapter,
            mode_index,
        });
    }
    policy
        .rows(&records, Some(0))
        .map(|rows| rows.iter().map(offered_from_row).collect())
        .map_err(|error| error.to_string())
}

#[cfg(not(target_arch = "wasm32"))]
fn popup_offers<'a>(
    policy: &pcx_play::options_modes::Policy,
    monitors: impl Iterator<Item = (Entity, &'a Monitor, bool)>,
) -> Result<Vec<OfferedResolution>, String> {
    let Some(adapter) = session_adapter(policy) else {
        return Err("Display mode profile has no usable session monitor token".into());
    };
    let mut bounds = Vec::new();
    let mut ids = Vec::new();
    for (entity, monitor, _) in monitors {
        if bounds.len() >= pcx_play::options_modes::MAX_HOST_RECORDS {
            return Err("Display monitor list exceeds its record limit".into());
        }
        let Ok(ordinal) = u32::try_from(bounds.len()) else {
            return Err("Display monitor list exceeds its index limit".into());
        };
        bounds.try_reserve(1).map_err(|error| error.to_string())?;
        ids.try_reserve(1).map_err(|error| error.to_string())?;
        bounds.push(pcx_play::options_modes::MonitorBound {
            ordinal,
            width: monitor.physical_width,
            height: monitor.physical_height,
        });
        ids.push(entity);
    }
    let rows = policy
        .popup_rows(&bounds, Some(0), adapter)
        .map_err(|error| error.to_string())?;
    let mut offers = Vec::new();
    offers
        .try_reserve(rows.len())
        .map_err(|error| error.to_string())?;
    for row in &rows {
        let Some(ordinal) = row.monitor_ordinal else {
            return Err("Popup row is missing its monitor ordinal".into());
        };
        let Some(entity) = usize::try_from(ordinal)
            .ok()
            .and_then(|index| ids.get(index))
            .copied()
        else {
            return Err("Popup row monitor is missing from this pass".into());
        };
        let mut offer = offered_from_row(row);
        offer.monitor = Some(entity);
        offers.push(offer);
    }
    Ok(offers)
}

const DISPLAY_IMAGE_BYTES: usize = 32 * 1024 * 1024;

/// `source` is the image input identity. The version label records pattern
/// qualification and does not name a release.
fn admit_display_image(
    image: &[u8],
    source: &str,
    expansion: bool,
) -> Result<Option<pcx_play::options_modes::Policy>, String> {
    use pcx_play::{
        neutral_player::RuleInputBudget,
        options_modes::{DiscoveryBudget, Policy, Problem},
    };
    let origin = pcx_play::scripts::AssetIdentity {
        edition: format!("selected skin V{}", u32::from(expansion)),
        version: "pattern-qualified PE32; release identity not inferred".into(),
        source: source.to_owned(),
    };
    match Policy::discover(
        image,
        origin,
        DiscoveryBudget {
            image: RuleInputBudget {
                image_bytes: DISPLAY_IMAGE_BYTES,
                profile_bytes: 128 * 1024,
                sections: 96,
            },
            label_bytes: 256,
            candidate_limit: 32,
            work_bytes: 64 * 1024 * 1024,
        },
    ) {
        Ok(policy) => Ok(Some(policy)),
        Err(error)
            if matches!(
                error.problem,
                Problem::None
                    | Problem::Image(pcx_play::neutral_player::NeutralError::Rule(
                        pcx_play::neutral_player::RuleProblem::UnsupportedArchitecture { .. }
                    ))
            ) =>
        {
            Ok(None)
        }
        Err(error) => Err(error.to_string()),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn discover_display_policy(
    install: &std::path::Path,
    expansion: bool,
) -> Result<Option<pcx_play::options_modes::Policy>, String> {
    use std::io::Read;
    let path = install.join("Warcraft III.exe");
    let mut file = match std::fs::File::open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("{}: {error}", path.display())),
    };
    if file
        .metadata()
        .map_err(|error| format!("{}: {error}", path.display()))?
        .len()
        > DISPLAY_IMAGE_BYTES as u64
    {
        return Err(format!(
            "{}: Options client exceeds image budget",
            path.display()
        ));
    }
    let mut image = Vec::new();
    image
        .try_reserve_exact(DISPLAY_IMAGE_BYTES + 1)
        .map_err(|error| error.to_string())?;
    file.by_ref()
        .take((DISPLAY_IMAGE_BYTES + 1) as u64)
        .read_to_end(&mut image)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    let source = path.display().to_string();
    admit_display_image(&image, &source, expansion)
}

#[cfg(not(target_arch = "wasm32"))]
fn display_policy(
    backend: &mut Backend,
    launcher: &crate::launch_gui::Launcher,
) -> Option<std::sync::Arc<pcx_play::options_modes::Policy>> {
    let install = crate::launch_gui::selected_install(launcher)?;
    let path = install.join("Warcraft III.exe").display().to_string();
    let expansion = launcher.expansion();
    if backend.display.path != path || backend.display.expansion != expansion {
        backend.display = DisplayAdmission {
            path,
            expansion,
            ..Default::default()
        };
    }
    if matches!(backend.display.state, DisplayState::Unchecked) {
        backend.display.state = match discover_display_policy(&install, expansion) {
            Ok(Some(policy)) => DisplayState::Qualified(std::sync::Arc::new(policy)),
            Ok(None) => DisplayState::Unqualified { detail: None },
            Err(detail) => DisplayState::Unqualified {
                detail: Some(detail),
            },
        };
        backend.display.announced = false;
        backend.display.empty_noted = false;
    }
    match &backend.display.state {
        DisplayState::Qualified(policy) => Some(policy.clone()),
        _ => None,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn note_listing_failure(nav: &mut Navigation, backend: &mut Backend, error: &str) {
    if backend.display.empty_noted {
        return;
    }
    warn!("Qualified display modes could not be listed: {error}");
    if nav.notice.is_empty() {
        nav.notice = format!("Qualified display modes could not be listed. {error}");
    }
    backend.display.empty_noted = true;
}

#[cfg(not(target_arch = "wasm32"))]
fn clear_qualified_notice(nav: &mut Navigation) {
    const LISTING: &str = "Qualified display modes could not be listed.";
    const EMPTY: &str = "No display modes passed the qualified client filter for this monitor.";
    if nav.notice == EMPTY || nav.notice.starts_with(LISTING) {
        nav.notice.clear();
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn announce_display(nav: &mut Navigation, backend: &mut Backend, empty_qualified: bool) {
    if !backend.display.announced {
        if let DisplayState::Unqualified {
            detail: Some(detail),
        } = &backend.display.state
        {
            let detail = detail.clone();
            warn!("Display mode profile was not admitted: {detail}");
            if nav.notice.is_empty() {
                nav.notice = format!(
                    "Display mode profile was not admitted; listing render sizes only. {detail}"
                );
            }
        }
        backend.display.announced = true;
    }
    if empty_qualified && !backend.display.empty_noted {
        warn!("Qualified display profile admitted no modes for the primary monitor");
        if nav.notice.is_empty() {
            nav.notice =
                "No display modes passed the qualified client filter for this monitor.".into();
        }
        backend.display.empty_noted = true;
    }
}

#[cfg(any(test, target_arch = "wasm32"))]
const BROWSER_SOURCE_NOTICE: &str =
    "Source choices are bounded by the browser screen. Apply changes the render extent.";
#[cfg(any(test, target_arch = "wasm32"))]
const BROWSER_MEASURED_IMAGE_NOTICE: &str =
    "A missing or unsupported client image keeps the measured choices.";
#[cfg(any(test, target_arch = "wasm32"))]
const BROWSER_MEASURED_BOUND_NOTICE: &str =
    "The browser screen bound is unavailable. Measured choices stay.";
#[cfg(any(test, target_arch = "wasm32"))]
const BROWSER_MEASURED_PROFILE_NOTICE: &str =
    "This client image has no source display rows. Measured choices stay.";
#[cfg(any(test, target_arch = "wasm32"))]
const BROWSER_ADMISSION_NOTICE: &str =
    "Display mode profile was not admitted. Measured choices stay.";
#[cfg(any(test, target_arch = "wasm32"))]
const BROWSER_LISTING_NOTICE: &str = "Source display choices could not be listed.";
#[cfg(any(test, target_arch = "wasm32"))]
const BROWSER_SCREEN_BOUND_NOTICE: &str = "Browser screen bound:";

#[cfg(any(test, target_arch = "wasm32"))]
enum BrowserProfileKind<'a> {
    Absent,
    Failed(&'a str),
    Popup,
    Other,
}

#[cfg(any(test, target_arch = "wasm32"))]
fn same_browser_admission(
    cached_revision: Option<u32>,
    cached_expansion: bool,
    revision: u32,
    expansion: bool,
) -> bool {
    cached_revision == Some(revision) && cached_expansion == expansion
}

/// Logical canvas row for one browser screen observation.
/// `adapter` is the render-only session placeholder 0, the same value
/// `OfferedResolution::render_size` stores. Depth, refresh, monitor, and
/// monitor ordinal stay empty. Label, order, kind, flag, and mode index come
/// from the parser row. Hardware identity and the source current-kind policy
/// stay out of this offer.
#[cfg(any(test, target_arch = "wasm32"))]
fn browser_canvas_offer(row: &pcx_play::options_modes::Row) -> OfferedResolution {
    OfferedResolution {
        label: row.label.clone(),
        size: (row.width, row.height),
        depth: None,
        refresh_millihertz: None,
        adapter: 0,
        mode_index: row.mode_index,
        kind: row.kind,
        flag: row.flag,
        monitor_ordinal: None,
        monitor: None,
        render_extent_only: true,
    }
}

#[cfg(any(test, target_arch = "wasm32"))]
fn resolve_browser_list(
    measured: Vec<OfferedResolution>,
    kind: BrowserProfileKind<'_>,
    bound: Option<(u32, u32)>,
    listed: Result<Vec<OfferedResolution>, String>,
) -> (Vec<OfferedResolution>, String) {
    match kind {
        BrowserProfileKind::Popup if bound.is_some() => match listed {
            Ok(rows) => (rows, BROWSER_SOURCE_NOTICE.to_owned()),
            Err(error) => (Vec::new(), format!("{BROWSER_LISTING_NOTICE} {error}")),
        },
        BrowserProfileKind::Popup => (measured, BROWSER_MEASURED_BOUND_NOTICE.to_owned()),
        BrowserProfileKind::Other => (measured, BROWSER_MEASURED_PROFILE_NOTICE.to_owned()),
        BrowserProfileKind::Failed(detail) => {
            (measured, format!("{BROWSER_ADMISSION_NOTICE} {detail}"))
        }
        BrowserProfileKind::Absent => (measured, BROWSER_MEASURED_IMAGE_NOTICE.to_owned()),
    }
}

#[cfg(any(test, target_arch = "wasm32"))]
fn browser_qualification_notice(text: &str) -> bool {
    text == BROWSER_SOURCE_NOTICE
        || text == BROWSER_MEASURED_IMAGE_NOTICE
        || text == BROWSER_MEASURED_BOUND_NOTICE
        || text == BROWSER_MEASURED_PROFILE_NOTICE
        || text.starts_with(BROWSER_ADMISSION_NOTICE)
        || text.starts_with(BROWSER_LISTING_NOTICE)
        || text.starts_with(BROWSER_SCREEN_BOUND_NOTICE)
}

#[cfg(any(test, target_arch = "wasm32"))]
fn assign_browser_notice(nav: &mut Navigation, notice: String) {
    if nav.notice.is_empty() || browser_qualification_notice(&nav.notice) {
        nav.notice = notice;
    }
}

/// Frameless flag-0 families for one positive screen observation.
/// The session adapter is the render-only placeholder 0. The bound ordinal is
/// only the pass index `popup_rows` copies onto each row; the canvas offer
/// drops it.
#[cfg(target_arch = "wasm32")]
fn browser_popup_offers(
    policy: &pcx_play::options_modes::Policy,
    width: u32,
    height: u32,
) -> Result<Vec<OfferedResolution>, String> {
    let rows = policy
        .popup_rows(
            &[pcx_play::options_modes::MonitorBound {
                ordinal: 0,
                width,
                height,
            }],
            Some(0),
            0,
        )
        .map_err(|error| error.to_string())?;
    let mut offers = Vec::new();
    offers
        .try_reserve(rows.len())
        .map_err(|error| error.to_string())?;
    for row in &rows {
        offers.push(browser_canvas_offer(row));
    }
    Ok(offers)
}

/// Reads the committed image only when the asset revision or selected skin
/// changes. The callback borrows those bytes for admission and does not store
/// them. No image for that revision clears the cached profile.
#[cfg(target_arch = "wasm32")]
fn remember_browser_admission(backend: &mut Backend, expansion: bool) {
    let revision = crate::browser::asset_revision();
    if same_browser_admission(
        backend.display.revision,
        backend.display.expansion,
        revision,
        expansion,
    ) {
        return;
    }
    let admission = crate::browser::with_committed_client_image(|source, bytes| {
        admit_display_image(bytes, source, expansion)
    })
    .unwrap_or(Ok(None));
    backend.display = DisplayAdmission {
        revision: Some(revision),
        expansion,
        state: match admission {
            Ok(Some(policy)) => DisplayState::Qualified(std::sync::Arc::new(policy)),
            Ok(None) => DisplayState::Unqualified { detail: None },
            Err(detail) => DisplayState::Unqualified {
                detail: Some(detail),
            },
        },
    };
}

/// Positive physical pixels from `peacecraft3Settings.screenBound`.
/// `Ok(None)` is a missing bound and keeps measured render sizes.
#[cfg(target_arch = "wasm32")]
fn read_browser_screen_bound() -> Result<Option<(u32, u32)>, String> {
    let text = browser_screen_bound().map_err(browser_failure)?;
    if text.len() > MAX_PREFERENCE_BYTES {
        return Err("exceeds its input limit".into());
    }
    let parsed = serde_json::from_str::<Option<(u32, u32)>>(&text)
        .map_err(|error| format!("malformed: {error}"))?;
    Ok(parsed.filter(|(width, height)| *width > 0 && *height > 0))
}

/// `modes` is the measured window, stage, and screen list from `displayModes`.
/// Popup rows use `screenBound` alone.
#[cfg(target_arch = "wasm32")]
fn load_browser_resolutions(
    nav: &mut Navigation,
    backend: &mut Backend,
    expansion: bool,
    modes: Vec<(u32, u32)>,
) -> String {
    remember_browser_admission(backend, expansion);
    let measured = render_offers(modes);
    let (rows, notice) = match read_browser_screen_bound() {
        Err(error) => (measured, format!("{BROWSER_SCREEN_BOUND_NOTICE} {error}")),
        Ok(bound) => {
            let listed = match (&backend.display.state, bound) {
                (DisplayState::Qualified(policy), Some((width, height)))
                    if policy.has_popup_profile() =>
                {
                    browser_popup_offers(policy, width, height)
                }
                _ => Ok(Vec::new()),
            };
            let kind = match &backend.display.state {
                DisplayState::Qualified(policy) if policy.has_popup_profile() => {
                    BrowserProfileKind::Popup
                }
                DisplayState::Qualified(_) => BrowserProfileKind::Other,
                DisplayState::Unqualified {
                    detail: Some(detail),
                } => BrowserProfileKind::Failed(detail),
                DisplayState::Unqualified { detail: None } | DisplayState::Unchecked => {
                    BrowserProfileKind::Absent
                }
            };
            resolve_browser_list(measured, kind, bound, listed)
        }
    };
    nav.resolutions = rows;
    notice
}

fn environment(
    mut nav: ResMut<Navigation>,
    mut backend: ResMut<Backend>,
    mut windows: Query<(Entity, &mut Window, Option<&OnMonitor>), With<PrimaryWindow>>,
    monitors: Query<(Entity, &Monitor, Has<PrimaryMonitor>)>,
    launcher: Res<crate::launch_gui::Launcher>,
    _main_thread: bevy::ecs::system::NonSendMarker,
) {
    let Ok((entity, mut window, on_monitor)) = windows.single_mut() else {
        return;
    };
    let current = backend
        .observed_resolution
        .unwrap_or((window.physical_width(), window.physical_height()));
    let mut modes = vec![current];
    if let Some((_, monitor, _)) = on_monitor.and_then(|on| monitors.get(on.0).ok()) {
        modes.push((monitor.physical_width, monitor.physical_height));
        modes.extend(
            monitor
                .video_modes
                .iter()
                .map(|mode| (mode.physical_size.x, mode.physical_size.y)),
        );
    }
    #[cfg(target_arch = "wasm32")]
    match browser_modes() {
        Ok(text) if text.len() <= MAX_PREFERENCE_BYTES => {
            if let Ok(actual) = serde_json::from_str::<Vec<(u32, u32)>>(&text) {
                modes.extend(actual);
            }
        }
        Ok(_) => nav.notice = "Browser display information exceeds its input limit".into(),
        Err(error) => nav.notice = format!("Browser display information: {error:?}"),
    }
    #[cfg(target_arch = "wasm32")]
    let browser_notice =
        load_browser_resolutions(&mut nav, &mut backend, launcher.expansion(), modes);
    // Explicit click or exact size. The admitted profile is not a score input.
    #[cfg(target_arch = "wasm32")]
    let policy: Option<std::sync::Arc<pcx_play::options_modes::Policy>> = None;
    #[cfg(not(target_arch = "wasm32"))]
    let policy = display_policy(&mut backend, &launcher);
    #[cfg(not(target_arch = "wasm32"))]
    let (empty_qualified, popup_list) = if let Some(policy) = policy.as_ref() {
        if policy.has_popup_profile() {
            match popup_offers(policy, monitors.iter()) {
                Ok(rows) => {
                    if !rows.is_empty() {
                        clear_qualified_notice(&mut nav);
                    }
                    nav.resolutions = rows;
                    (false, true)
                }
                Err(error) => {
                    nav.resolutions.clear();
                    note_listing_failure(&mut nav, &mut backend, &error);
                    (false, true)
                }
            }
        } else if let Some((_, monitor, _)) = monitors.iter().find(|(_, _, primary)| *primary) {
            match qualified_offers(policy, monitor) {
                Ok(rows) => {
                    let empty = rows.is_empty();
                    if !empty {
                        clear_qualified_notice(&mut nav);
                    }
                    nav.resolutions = rows;
                    (empty, false)
                }
                Err(error) => {
                    nav.resolutions.clear();
                    note_listing_failure(&mut nav, &mut backend, &error);
                    (false, false)
                }
            }
        } else {
            nav.resolutions = render_offers(modes);
            if !nav.resolutions.is_empty() {
                clear_qualified_notice(&mut nav);
            }
            (false, false)
        }
    } else {
        nav.resolutions = render_offers(modes);
        if !nav.resolutions.is_empty() {
            clear_qualified_notice(&mut nav);
        }
        (false, false)
    };
    if !backend.loaded {
        backend.loaded = true;
        match load_preferences() {
            Ok(Some(text)) => match decode_preferences(&text) {
                Ok(prefs) => {
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        backend.saved_resolution = prefs.settings.resolution.is_some();
                    }
                    nav.applied = prefs.settings;
                    backend.audio = prefs.audio;
                    backend.has_preferences = true;
                }
                Err(error) => nav.notice = error,
            },
            Ok(None) => {}
            Err(error) => {
                backend.preference_unreadable = Some(error.clone());
                nav.notice = error;
            }
        }
        backend.pending_resolution = nav.applied.resolution;
        nav.applied.resolution = Some(current);
        nav.draft = nav.applied.clone();
        #[cfg(target_arch = "wasm32")]
        if backend.preference_unreadable.is_none()
            && let Err(error) = push_browser_audio(backend.audio)
        {
            nav.notice = error;
        }
    }
    if backend.resize.is_none()
        && let Some(resolution) = backend.pending_resolution
    {
        if nav.offers_size(resolution) {
            if resolution::startup(
                entity,
                &mut nav,
                &mut window,
                &mut backend,
                &launcher,
                resolution,
            ) {
                backend.pending_resolution = None;
            }
        } else if on_monitor.is_some() || cfg!(target_arch = "wasm32") {
            nav.applied.resolution = Some(current);
            nav.draft.resolution = Some(current);
            backend.pending_resolution = None;
            nav.notice =
                "Saved resolution is unavailable on this display; using the current window".into();
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    announce_display(&mut nav, &mut backend, empty_qualified);
    #[cfg(not(target_arch = "wasm32"))]
    if popup_list && nav.notice.is_empty() {
        nav.notice = crate::menu_screens::POPUP_WORK_AREA_NOTICE.into();
    }
    #[cfg(target_arch = "wasm32")]
    assign_browser_notice(&mut nav, browser_notice);
    choose_offer(&mut nav, policy.as_deref());
}
fn attach_controls(
    mut commands: Commands,
    controls: Query<(Entity, &SettingControl), Added<SettingControl>>,
    mut nav: ResMut<Navigation>,
    mut backend: ResMut<Backend>,
) {
    for (entity, control) in &controls {
        match &control.widget {
            Widget::Slider {
                min,
                max,
                initial,
                step,
            } if matches!(
                control.name.as_str(),
                "MusicVolumeSlider" | "SoundVolumeSlider"
            ) =>
            {
                let range = Range {
                    min: *min,
                    max: *max,
                    step: *step,
                };
                if !range.valid(*initial) {
                    commands.entity(entity).insert(InteractionDisabled);
                    nav.notice = format!("{}: invalid authored slider range", control.name);
                    continue;
                }
                backend.ranges.insert(control.name.clone(), range);
                let gain = if control.name == "MusicVolumeSlider" {
                    backend.audio.music_gain
                } else {
                    backend.audio.effects_gain
                };
                // Opening a control reflects effective playback, never a different
                // widget initial. Saved gains are authoritative across source ranges.
                let projected = range.value(gain);
                let value = if backend.has_preferences {
                    projected
                } else {
                    nav.applied
                        .values
                        .get(&control.name)
                        .copied()
                        .filter(|value| range.valid(*value))
                        .unwrap_or(projected)
                };
                let previous = nav.applied.values.insert(control.name.clone(), value);
                let draft = nav
                    .draft
                    .values
                    .entry(control.name.clone())
                    .or_insert(value);
                if previous == Some(*draft) || !range.valid(*draft) {
                    *draft = value;
                }
                commands.entity(entity).insert((
                    Slider {
                        track_click: TrackClick::Snap,
                        orientation: SliderOrientation::Horizontal,
                    },
                    SliderRange::new(*min, *max),
                    SliderStep(*step),
                    SliderValue(*draft),
                ));
                slider::attach(&mut commands, entity);
            }
            Widget::Checkbox
                if matches!(control.name.as_str(), "MusicCheckBox" | "SoundCheckBox") =>
            {
                commands.entity(entity).insert(Checkbox);
                let checked = nav.draft.toggles.get(&control.name).copied().unwrap_or(
                    if control.name == "MusicCheckBox" {
                        backend.audio.music_enabled
                    } else {
                        backend.audio.effects_enabled
                    },
                );
                let applied = if control.name == "MusicCheckBox" {
                    backend.audio.music_enabled
                } else {
                    backend.audio.effects_enabled
                };
                nav.applied.toggles.insert(control.name.clone(), applied);
                nav.draft
                    .toggles
                    .entry(control.name.clone())
                    .or_insert(applied);
                if checked {
                    commands.entity(entity).insert(Checked);
                } else {
                    commands.entity(entity).remove::<Checked>();
                }
            }
            _ => {}
        }
    }
}
fn slider_changed(
    event: On<ValueChange<f32>>,
    controls: Query<&SettingControl, Without<InteractionDisabled>>,
    mut nav: ResMut<Navigation>,
    mut commands: Commands,
    backend: Res<Backend>,
) {
    if backend.applying_resolution() {
        return;
    }
    let Ok(control) = controls.get(event.source) else {
        return;
    };
    if let Widget::Slider { min, max, step, .. } = control.widget {
        if !matches!(
            control.name.as_str(),
            "MusicVolumeSlider" | "SoundVolumeSlider"
        ) || !event.value.is_finite()
        {
            return;
        }
        let range = Range { min, max, step };
        let value = range.snap(event.value);
        nav.draft.values.insert(control.name.clone(), value);
        commands.entity(event.source).insert(SliderValue(value));
    }
}
fn checkbox_changed(
    event: On<ValueChange<bool>>,
    controls: Query<&SettingControl, Without<InteractionDisabled>>,
    mut nav: ResMut<Navigation>,
    mut commands: Commands,
    backend: Res<Backend>,
) {
    if backend.applying_resolution() {
        return;
    }
    let Ok(control) = controls.get(event.source) else {
        return;
    };
    if !matches!(control.name.as_str(), "MusicCheckBox" | "SoundCheckBox") {
        return;
    }
    nav.draft.toggles.insert(control.name.clone(), event.value);
    if event.value {
        commands.entity(event.source).insert(Checked);
    } else {
        commands.entity(event.source).remove::<Checked>();
    }
}
type SettingLabelData = (
    &'static SettingControl,
    Has<Checked>,
    Option<&'static preview::Control>,
    Option<&'static SliderValue>,
);

fn labels(
    nav: Res<Navigation>,
    mut labels: Query<(&SettingValue, &mut Text)>,
    controls: Query<SettingLabelData>,
) {
    for (owner, mut text) in &mut labels {
        let Ok((control, checked, preview, slider)) = controls.get(owner.0) else {
            continue;
        };
        let value = match &control.widget {
            Widget::Slider { .. } if preview.is_some() => slider
                .map(|value| value.0.to_string())
                .unwrap_or_else(|| "Unavailable".into()),
            Widget::Slider { initial, .. } => nav
                .draft
                .values
                .get(&control.name)
                .copied()
                .unwrap_or(*initial)
                .to_string(),
            Widget::Popup { .. } if control.name == "ResolutionMenu" => nav
                .draft
                .resolution
                .map(|(w, h)| {
                    nav.selected_offer
                        .as_ref()
                        .filter(|offer| offer.size == (w, h))
                        .map(|offer| offer.label.clone())
                        .unwrap_or_else(|| format!("{w} x {h}"))
                })
                .unwrap_or_default(),
            Widget::Popup { .. } => preview
                .and_then(|p| p.popup.clone())
                .unwrap_or_else(|| "Unavailable".into()),
            Widget::Checkbox => {
                if checked {
                    "×".into()
                } else {
                    "".into()
                }
            }
        };
        if text.0 != value {
            text.0 = value;
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn preference_path() -> Result<std::path::PathBuf, String> {
    #[cfg(target_os = "windows")]
    let base = std::env::var_os("APPDATA").map(std::path::PathBuf::from);
    #[cfg(target_os = "macos")]
    let base = std::env::var_os("HOME")
        .map(|path| std::path::PathBuf::from(path).join("Library/Application Support"));
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|path| std::path::PathBuf::from(path).join(".config"))
        });
    base.map(|path| path.join("peacecraft3/settings.json"))
        .ok_or_else(|| "User settings directory is unavailable".into())
}
#[cfg(not(target_arch = "wasm32"))]
fn load_preferences() -> Result<Option<String>, String> {
    use std::io::Read;
    let path = preference_path()?;
    let mut file = match std::fs::File::open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("{}: {error}", path.display())),
    };
    if file.metadata().map_err(|e| e.to_string())?.len() > MAX_PREFERENCE_BYTES as u64 {
        return Err("Saved settings exceed the preference byte limit".into());
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(MAX_PREFERENCE_BYTES + 1)
        .map_err(|e| e.to_string())?;
    file.by_ref()
        .take((MAX_PREFERENCE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_PREFERENCE_BYTES {
        return Err("Saved settings grew beyond the preference byte limit".into());
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|e| format!("Saved settings are not UTF-8: {e}"))
}
#[cfg(not(target_arch = "wasm32"))]
fn save_preferences(text: &str) -> Result<(), String> {
    let path = preference_path()?;
    let parent = path
        .parent()
        .ok_or("User settings directory is unavailable")?;
    std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
    std::fs::write(&temporary, text)
        .and_then(|_| std::fs::rename(&temporary, &path))
        .map_err(|e| format!("{}: {e}", path.display()))
}
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
extern "C" {
    #[wasm_bindgen(catch,js_name=String)]
    fn browser_error_text(value: &wasm_bindgen::JsValue) -> Result<String, wasm_bindgen::JsValue>;
    #[wasm_bindgen(catch,js_namespace=peacecraft3Settings,js_name=displayModes)]
    fn browser_modes() -> Result<String, wasm_bindgen::JsValue>;
    #[wasm_bindgen(catch,js_namespace=peacecraft3Settings,js_name=screenBound)]
    fn browser_screen_bound() -> Result<String, wasm_bindgen::JsValue>;
    #[wasm_bindgen(catch,js_namespace=peacecraft3Settings,js_name=load)]
    fn browser_load(limit: usize) -> Result<Option<String>, wasm_bindgen::JsValue>;
    #[wasm_bindgen(catch,js_namespace=peacecraft3Settings,js_name=save)]
    fn browser_save(text: &str, limit: usize) -> Result<(), wasm_bindgen::JsValue>;
    #[wasm_bindgen(catch,js_namespace=peacecraft3Settings,js_name=resumeParentSizing)]
    fn browser_parent_sizing() -> Result<(), wasm_bindgen::JsValue>;
    #[wasm_bindgen(catch,js_namespace=peacecraft3Audio,js_name=setUserGains)]
    fn browser_audio(
        music: f32,
        effects: f32,
        music_enabled: bool,
        effects_enabled: bool,
    ) -> Result<(), wasm_bindgen::JsValue>;
}
#[cfg(target_arch = "wasm32")]
fn browser_failure(error: wasm_bindgen::JsValue) -> String {
    warn!("Browser settings operation failed: {error:?}");
    // Keep the stack in diagnostics, not across the Options controls.
    browser_error_text(&error)
        .unwrap_or_else(|_| "Browser operation failed".into())
        .lines()
        .next()
        .unwrap_or("Browser operation failed")
        .chars()
        .take(240)
        .collect()
}
#[cfg(target_arch = "wasm32")]
fn load_preferences() -> Result<Option<String>, String> {
    browser_load(MAX_PREFERENCE_BYTES)
        .map_err(|e| format!("Loading browser settings: {}", browser_failure(e)))
}
#[cfg(target_arch = "wasm32")]
fn save_preferences(text: &str) -> Result<(), String> {
    browser_save(text, MAX_PREFERENCE_BYTES)
        .map_err(|e| format!("Saving browser settings: {}", browser_failure(e)))
}
#[cfg(target_arch = "wasm32")]
fn push_browser_audio(audio: AudioSettings) -> Result<(), String> {
    browser_audio(
        audio.music_gain,
        audio.effects_gain,
        audio.music_enabled,
        audio.effects_enabled,
    )
    .map_err(|e| format!("Applying browser audio settings: {}", browser_failure(e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unobserved_render_size_is_not_saved_as_applied() {
        let mut nav = Navigation {
            resolutions: vec![OfferedResolution::render_size(740, 510)],
            ..default()
        };
        nav.draft.resolution = Some((740, 510));
        let mut window = Window::default();
        let mut backend = Backend::default();
        let mut saved = false;
        assert!(
            commit(&mut nav, &mut window, &mut backend, |_| {
                saved = true;
                Ok(())
            })
            .is_err()
        );
        assert!(!saved);
        assert_eq!(nav.applied, Settings::default());
    }
    #[test]
    fn first_host_report_replaces_startup_seed_without_losing_dirty_intent() {
        for dirty in [false, true] {
            let mut nav = Navigation::default();
            nav.applied.resolution = Some((731, 419));
            nav.draft = nav.applied.clone();
            if dirty {
                nav.draft.resolution = Some((997, 613));
            }
            let mut backend = Backend::default();
            observe_resolution(&mut nav, &mut backend, (853, 547));
            assert_eq!(nav.applied.resolution, Some((853, 547)));
            assert_eq!(
                nav.draft.resolution,
                Some(if dirty { (997, 613) } else { (853, 547) })
            );
        }
    }

    #[test]
    fn external_resize_does_not_block_an_audio_only_commit() {
        let mut nav = Navigation::default();
        let mut backend = Backend::default();
        observe_resolution(&mut nav, &mut backend, (740, 510));
        nav.applied.resolution = Some((740, 510));
        nav.draft = nav.applied.clone();
        nav.resolutions = vec![OfferedResolution::render_size(800, 600)];
        observe_resolution(&mut nav, &mut backend, (960, 650));
        assert_eq!(nav.applied.resolution, Some((960, 650)));
        assert_eq!(nav.draft.resolution, Some((960, 650)));
        assert!(!nav.offers_size((960, 650)));
        nav.draft.toggles.insert("MusicCheckBox".into(), false);
        let mut window = Window::default();
        window.resolution.set_physical_resolution(960, 650);
        commit(&mut nav, &mut window, &mut backend, |_| Ok(())).unwrap();
        assert!(!backend.audio.music_enabled);
        assert_eq!(
            (window.physical_width(), window.physical_height()),
            (960, 650)
        );
    }

    #[test]
    fn unavailable_sizes_need_a_current_report_and_stale_selections_still_refuse() {
        let current = (913, 617);
        let mut nav = Navigation::default();
        nav.applied.resolution = Some(current);
        nav.draft = nav.applied.clone();
        let mut backend = Backend::default();
        for observed in [None, Some((701, 503))] {
            backend.observed_resolution = observed;
            assert!(prepare(&nav, &backend).is_err());
        }
        backend.observed_resolution = Some(current);
        nav.draft.resolution = Some((811, 563));
        assert!(prepare(&nav, &backend).is_err());
        nav.draft.resolution = Some(current);
        nav.selected_offer = Some(OfferedResolution::render_size(current.0, current.1));
        assert!(prepare(&nav, &backend).is_err());
    }

    #[test]
    fn external_resize_preserves_a_deliberate_draft_resolution() {
        let mut nav = Navigation::default();
        let mut backend = Backend::default();
        observe_resolution(&mut nav, &mut backend, (740, 510));
        nav.applied.resolution = Some((740, 510));
        nav.draft.resolution = Some((1480, 1020));
        observe_resolution(&mut nav, &mut backend, (960, 650));
        assert_eq!(nav.applied.resolution, Some((960, 650)));
        assert_eq!(nav.draft.resolution, Some((1480, 1020)));
        nav.action("CancelButton");
        assert_eq!(nav.draft.resolution, Some((960, 650)));
    }

    #[test]
    fn observing_a_pending_saved_mode_does_not_replace_it() {
        let mut nav = Navigation::default();
        let mut backend = Backend::default();
        observe_resolution(&mut nav, &mut backend, (740, 510));
        nav.applied.resolution = Some((1480, 1020));
        nav.draft = nav.applied.clone();
        backend.pending_resolution = nav.applied.resolution;
        observe_resolution(&mut nav, &mut backend, (960, 650));
        assert_eq!(nav.applied.resolution, Some((1480, 1020)));
        assert_eq!(nav.draft.resolution, Some((1480, 1020)));
    }

    #[test]
    fn source_range_normalizes_and_steps_without_copying_values() {
        let range = Range {
            min: 7.0,
            max: 31.0,
            step: 3.0,
        };
        assert_eq!(range.gain(19.0).unwrap(), 0.5);
        assert_eq!(range.snap(20.0), 19.0);
        assert!(range.gain(f32::NAN).is_err());
        assert!(range.gain(32.0).is_err());
        assert!(
            !Range {
                min: -f32::MAX,
                max: f32::MAX,
                step: 1.0
            }
            .valid(0.0)
        );
    }
    #[test]
    fn malformed_or_unsupported_preferences_refuse() {
        let settings = Settings::default();
        let text = serde_json::to_string(&Preferences {
            version: 1,
            settings,
            audio: AudioSettings::default(),
        })
        .unwrap();
        assert!(decode_preferences(&text).is_ok());
        assert!(decode_preferences(&" ".repeat(MAX_PREFERENCE_BYTES + 1)).is_err());
        assert!(decode_preferences(&text.replace("\"version\":1", "\"version\":2")).is_err());
        let mut values = Settings::default();
        values.values.insert("unsupported".into(), 1.0);
        assert!(validate_settings(&values).is_err());
    }
    #[test]
    fn saved_render_size_does_not_silently_accept_an_unimplemented_display_mode() {
        let prefs = Preferences {
            version: 1,
            settings: Settings {
                resolution: Some((931, 677)),
                ..default()
            },
            audio: AudioSettings::default(),
        };
        let text = serde_json::to_string(&prefs).unwrap();
        assert_eq!(decode_preferences(&text).unwrap().settings, prefs.settings);
        let unknown = text.replace(
            "\"settings\":{",
            "\"settings\":{\"native_exclusive\":{\"refresh_millihertz\":60123},",
        );
        assert!(decode_preferences(&unknown).is_err());
    }
    #[test]
    fn choosing_resolution_changes_only_draft_and_refuses_stale_modes() {
        let mut nav = Navigation {
            resolutions: vec![
                OfferedResolution::render_size(740, 510),
                OfferedResolution::render_size(1480, 1020),
            ],
            ..default()
        };
        let applied = nav.applied.clone();
        select_resolution(&mut nav, (1480, 1020)).unwrap();
        assert_eq!(nav.applied, applied);
        assert_eq!(nav.draft.resolution, Some((1480, 1020)));
        assert!(select_resolution(&mut nav, (1, 1)).is_err());
    }
    #[test]
    fn audio_user_gain_is_a_multiplier_and_mute_is_reversible() {
        let mut audio = AudioSettings {
            music_gain: 0.25,
            effects_gain: 0.75,
            ..default()
        };
        assert_eq!(audio.effective_effects(), 0.75);
        audio.effects_enabled = false;
        assert_eq!(audio.effective_effects(), 0.0);
        let authored = 0.4;
        assert_eq!(authored * audio.effective_music(), 0.1);
        audio.music_enabled = false;
        assert_eq!(audio.effective_music(), 0.0);
        audio.music_enabled = true;
        assert_eq!(audio.effective_music(), 0.25);
    }
    #[test]
    fn opening_controls_editing_and_cancel_leave_applied_audio_unchanged() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        world.insert_resource(Backend::default());
        world.insert_resource(Navigation::default());
        world.add_observer(slider_changed);
        let make = || SettingControl {
            name: "MusicVolumeSlider".into(),
            widget: Widget::Slider {
                min: 0.0,
                max: 10.0,
                initial: 0.0,
                step: 1.0,
            },
        };
        let entity = world.spawn(make()).id();
        world.run_system_once(attach_controls).unwrap();
        assert_eq!(world.resource::<Backend>().audio.effective_music(), 1.0);
        assert_eq!(
            world.resource::<Navigation>().applied.values["MusicVolumeSlider"],
            10.0
        );
        assert_eq!(
            world.resource::<Navigation>().draft.values["MusicVolumeSlider"],
            10.0
        );
        world.trigger(ValueChange {
            source: entity,
            value: 3.0_f32,
            is_final: true,
        });
        world.flush();
        assert_eq!(
            world.resource::<Navigation>().draft.values["MusicVolumeSlider"],
            3.0
        );
        assert_eq!(world.resource::<Backend>().audio.effective_music(), 1.0);
        world.despawn(entity);
        let replacement = world.spawn(make()).id();
        world.run_system_once(attach_controls).unwrap();
        assert_eq!(world.get::<SliderValue>(replacement).unwrap().0, 3.0);
        world.resource_mut::<Navigation>().action("CancelButton");
        assert_eq!(
            world.resource::<Navigation>().draft.values["MusicVolumeSlider"],
            10.0
        );
        assert_eq!(world.resource::<Backend>().audio.effective_music(), 1.0);
    }
    #[test]
    fn unsaved_slider_reflects_unqualified_playback_without_using_widget_initial() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        world.init_resource::<Backend>();
        world.init_resource::<Navigation>();
        let control = world
            .spawn(SettingControl {
                name: "MusicVolumeSlider".into(),
                widget: Widget::Slider {
                    min: 7.0,
                    max: 31.0,
                    initial: 19.0,
                    step: 2.0,
                },
            })
            .id();
        world.run_system_once(attach_controls).unwrap();
        assert_eq!(world.get::<SliderValue>(control).unwrap().0, 31.0);
        assert_eq!(
            world.resource::<Navigation>().draft.values["MusicVolumeSlider"],
            31.0
        );
        let backend = world.resource::<Backend>();
        assert_eq!(
            backend.audio.music_gain, 1.0,
            "opening controls is not Apply"
        );
        assert_eq!(
            backend
                .candidate_audio(&world.resource::<Navigation>().draft, &Settings::default())
                .unwrap()
                .music_gain,
            1.0
        );
    }

    #[test]
    fn saved_gain_remains_authoritative_when_source_slider_initial_changes() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let mut backend = Backend {
            has_preferences: true,
            ..default()
        };
        backend.audio.music_gain = 0.25;
        world.insert_resource(backend);
        world.init_resource::<Navigation>();
        let control = world
            .spawn(SettingControl {
                name: "MusicVolumeSlider".into(),
                widget: Widget::Slider {
                    min: 7.0,
                    max: 31.0,
                    initial: 19.0,
                    step: 2.0,
                },
            })
            .id();
        world.run_system_once(attach_controls).unwrap();
        assert_eq!(world.get::<SliderValue>(control).unwrap().0, 13.0);
        assert_eq!(world.resource::<Backend>().audio.music_gain, 0.25);
    }

    #[test]
    fn apply_validation_and_storage_failure_preserve_state_then_commit_once() {
        let mut nav = Navigation {
            resolutions: vec![OfferedResolution::render_size(740, 510)],
            ..default()
        };
        nav.draft.resolution = Some((740, 510));
        nav.draft.values.insert("MusicVolumeSlider".into(), 19.0);
        let mut backend = Backend::default();
        backend.ranges.insert(
            "MusicVolumeSlider".into(),
            Range {
                min: 7.0,
                max: 31.0,
                step: 3.0,
            },
        );
        let mut window = Window::default();
        window.resolution.set_physical_resolution(740, 510);
        backend.observed_resolution = Some((740, 510));
        let before = (window.physical_width(), window.physical_height());
        let old = nav.applied.clone();
        assert!(
            commit(&mut nav, &mut window, &mut backend, |_| Err(
                "storage denied".into()
            ))
            .is_err()
        );
        assert_eq!(nav.applied, old);
        assert_eq!(backend.audio.effective_music(), 1.0);
        assert_eq!((window.physical_width(), window.physical_height()), before);
        nav.draft.resolution = Some((1, 1));
        assert!(
            commit(&mut nav, &mut window, &mut backend, |_| panic!(
                "validation must precede persistence"
            ))
            .is_err()
        );
        nav.draft.resolution = Some((740, 510));
        commit(&mut nav, &mut window, &mut backend, |text| {
            let saved = decode_preferences(text)?;
            assert_eq!(saved.audio.music_gain, 0.5);
            assert_eq!(saved.settings.values["MusicVolumeSlider"], 19.0);
            Ok(())
        })
        .unwrap();
        assert_eq!(nav.applied, nav.draft);
        assert_eq!(
            (window.physical_width(), window.physical_height()),
            (740, 510)
        );
        assert_eq!(backend.audio.effective_music(), 0.5);
    }
    #[test]
    fn same_size_offers_keep_the_selected_refresh_until_cancel() {
        let slow = OfferedResolution {
            refresh_millihertz: Some(60_000),
            mode_index: 1,
            label: "800x600".into(),
            render_extent_only: false,
            ..OfferedResolution::render_size(800, 600)
        };
        let fast = OfferedResolution {
            refresh_millihertz: Some(144_000),
            mode_index: 4,
            ..slow.clone()
        };
        let mut nav = Navigation {
            screen: crate::menu_screens::Screen::Options(crate::menu_screens::Page::Video),
            resolutions: vec![slow, fast.clone()],
            ..default()
        };
        select_offer(&mut nav, &fast).unwrap();
        assert_eq!(nav.draft.resolution, Some((800, 600)));
        assert_eq!(
            nav.selected_offer.as_ref().unwrap().refresh_millihertz,
            Some(144_000)
        );
        assert!(select_offer(&mut nav, &OfferedResolution::render_size(1, 1)).is_err());
        assert_eq!(
            nav.selected_offer.as_ref().unwrap().mode_index,
            4,
            "a refused row must not replace the selection"
        );
        nav.action("CancelButton");
        assert!(nav.selected_offer.is_none());
        assert_eq!(nav.draft.resolution, nav.applied.resolution);
    }

    #[test]
    fn choose_offer_keeps_same_size_identity_and_replaces_a_stale_size() {
        let slow = OfferedResolution {
            refresh_millihertz: Some(60_000),
            mode_index: 1,
            label: "800x600".into(),
            render_extent_only: false,
            ..OfferedResolution::render_size(800, 600)
        };
        let fast = OfferedResolution {
            refresh_millihertz: Some(144_000),
            mode_index: 4,
            ..slow.clone()
        };
        let mut nav = Navigation {
            resolutions: vec![slow, fast.clone(), OfferedResolution::render_size(640, 480)],
            ..default()
        };
        nav.draft.resolution = Some((800, 600));
        select_offer(&mut nav, &fast).unwrap();
        choose_offer(&mut nav, None);
        assert_eq!(nav.selected_offer.as_ref().unwrap().mode_index, 4);
        assert_eq!(nav.draft.resolution, Some((800, 600)));
        nav.draft.resolution = Some((640, 480));
        choose_offer(&mut nav, None);
        assert_eq!(nav.draft.resolution, Some((640, 480)));
        let selected = nav.selected_offer.as_ref().unwrap();
        assert!(selected.render_extent_only);
        assert_eq!(selected.size, (640, 480));
        nav.draft.resolution = Some((1, 1));
        choose_offer(&mut nav, None);
        assert!(nav.selected_offer.is_none());
        assert_eq!(nav.draft.resolution, Some((1, 1)));
        assert_eq!(nav.resolutions.len(), 3);
    }

    #[test]
    fn render_offers_drop_nonpositive_sizes_and_add_no_fallback() {
        let rows = render_offers(vec![(0, 10), (10, 0), (800, 600), (800, 600), (640, 480)]);
        assert!(rows.iter().all(|row| row.render_extent_only));
        assert_eq!(
            rows.iter().map(|row| row.size).collect::<Vec<_>>(),
            vec![(640, 480), (800, 600)]
        );
        assert!(render_offers(Vec::new()).is_empty());
    }

    fn parser_row(
        width: u32,
        height: u32,
        mode_index: u32,
        kind: i32,
    ) -> pcx_play::options_modes::Row {
        pcx_play::options_modes::Row {
            label: format!("{width}/{height}"),
            width,
            height,
            depth: Some(32),
            refresh_millihertz: Some(60_000),
            adapter: 7,
            mode_index,
            kind,
            flag: 0,
            monitor_ordinal: Some(4),
        }
    }

    #[test]
    fn browser_canvas_offer_keeps_parser_fields_and_clears_hardware_identity() {
        let row = parser_row(800, 600, 2, 11);
        let offer = browser_canvas_offer(&row);
        assert_eq!(offer.label, "800/600");
        assert_eq!(offer.size, (800, 600));
        assert_eq!(offer.mode_index, 2);
        assert_eq!(offer.kind, 11);
        assert_eq!(offer.flag, 0);
        assert_eq!(offer.adapter, 0);
        assert!(offer.depth.is_none());
        assert!(offer.refresh_millihertz.is_none());
        assert!(offer.monitor.is_none());
        assert!(offer.monitor_ordinal.is_none());
        assert!(offer.render_extent_only);
        assert_eq!(offer.adapter, OfferedResolution::render_size(1, 1).adapter);
    }

    #[test]
    fn browser_popup_list_keeps_empty_results_and_measured_fallbacks() {
        let measured = render_offers(vec![(1920, 1080), (800, 600)]);
        let later = browser_canvas_offer(&parser_row(400, 280, 1, 11));
        let earlier = browser_canvas_offer(&parser_row(151, 97, 0, 11));
        let (rows, notice) = resolve_browser_list(
            measured.clone(),
            BrowserProfileKind::Popup,
            Some((1366, 768)),
            Ok(vec![later.clone(), earlier.clone()]),
        );
        assert_eq!(rows, vec![later, earlier.clone()]);
        assert_eq!(notice, BROWSER_SOURCE_NOTICE);
        let (rows, notice) = resolve_browser_list(
            measured.clone(),
            BrowserProfileKind::Popup,
            Some((100, 80)),
            Ok(Vec::new()),
        );
        assert!(rows.is_empty());
        assert_eq!(notice, BROWSER_SOURCE_NOTICE);
        let (rows, notice) = resolve_browser_list(
            measured.clone(),
            BrowserProfileKind::Popup,
            Some((100, 80)),
            Err("label budget".into()),
        );
        assert!(rows.is_empty());
        assert_eq!(notice, format!("{BROWSER_LISTING_NOTICE} label budget"));
        let (rows, notice) = resolve_browser_list(
            measured.clone(),
            BrowserProfileKind::Popup,
            None,
            Err("unused".into()),
        );
        assert_eq!(rows, measured);
        assert_eq!(notice, BROWSER_MEASURED_BOUND_NOTICE);
        let (rows, notice) = resolve_browser_list(
            measured.clone(),
            BrowserProfileKind::Absent,
            Some((1366, 768)),
            Ok(vec![earlier]),
        );
        assert_eq!(rows, measured);
        assert_eq!(notice, BROWSER_MEASURED_IMAGE_NOTICE);
        let (rows, notice) = resolve_browser_list(
            measured.clone(),
            BrowserProfileKind::Other,
            Some((1366, 768)),
            Ok(Vec::new()),
        );
        assert_eq!(rows, measured);
        assert_eq!(notice, BROWSER_MEASURED_PROFILE_NOTICE);
        let (rows, notice) = resolve_browser_list(
            measured.clone(),
            BrowserProfileKind::Failed("invalid PE image"),
            Some((1366, 768)),
            Ok(Vec::new()),
        );
        assert_eq!(rows, measured);
        assert_eq!(
            notice,
            format!("{BROWSER_ADMISSION_NOTICE} invalid PE image")
        );
    }

    #[test]
    fn browser_selection_stays_on_the_clicked_canvas_row() {
        let first = browser_canvas_offer(&parser_row(800, 600, 0, 11));
        let second = browser_canvas_offer(&parser_row(800, 600, 1, 3));
        let mut nav = Navigation {
            resolutions: vec![first, second.clone()],
            ..default()
        };
        nav.draft.resolution = Some((800, 600));
        select_offer(&mut nav, &second).unwrap();
        choose_offer(&mut nav, None);
        let selected = nav.selected_offer.as_ref().unwrap();
        assert_eq!(selected.mode_index, 1);
        assert_eq!(selected.kind, 3);
        assert!(selected.render_extent_only);
        assert!(selected.monitor.is_none());
        assert_eq!(nav.draft.resolution, Some((800, 600)));
    }

    #[test]
    fn browser_admission_cache_tracks_revision_and_skin() {
        assert!(!same_browser_admission(None, false, 0, false));
        assert!(same_browser_admission(Some(4), true, 4, true));
        assert!(!same_browser_admission(Some(4), true, 5, true));
        assert!(!same_browser_admission(Some(4), false, 4, true));
    }

    #[test]
    fn admit_display_image_keeps_the_input_identity() {
        let blank = admit_display_image(&[0, 1, 2, 3], "  ", false).unwrap_err();
        assert!(blank.contains("selected skin V0"));
        assert!(blank.contains("pattern-qualified PE32; release identity not inferred"));
        let named = admit_display_image(b"not-a-client", "chosen.dll", true).unwrap_err();
        assert!(named.contains("chosen.dll"));
        assert!(named.contains("selected skin V1"));
        assert!(named.contains("pattern-qualified PE32; release identity not inferred"));
    }

    #[test]
    fn browser_qualification_notice_does_not_replace_other_messages() {
        let mut nav = Navigation::default();
        assign_browser_notice(&mut nav, BROWSER_SOURCE_NOTICE.to_owned());
        assert_eq!(nav.notice, BROWSER_SOURCE_NOTICE);
        assign_browser_notice(&mut nav, BROWSER_MEASURED_IMAGE_NOTICE.to_owned());
        assert_eq!(nav.notice, BROWSER_MEASURED_IMAGE_NOTICE);
        assert!(browser_qualification_notice(&format!(
            "{BROWSER_LISTING_NOTICE} label budget"
        )));
        assert!(browser_qualification_notice(&format!(
            "{BROWSER_SCREEN_BOUND_NOTICE} exceeds its input limit"
        )));
        assert!(!browser_qualification_notice(
            "Browser display information exceeds its input limit"
        ));
        nav.notice =
            "Saved resolution is unavailable on this display; using the current window".into();
        assign_browser_notice(&mut nav, BROWSER_SOURCE_NOTICE.to_owned());
        assert!(nav.notice.starts_with("Saved resolution"));
    }

    #[test]
    fn popup_title_separates_qualified_rows_render_sizes_and_an_empty_list() {
        let render = OfferedResolution::render_size(800, 600);
        let qualified = OfferedResolution {
            render_extent_only: false,
            ..render.clone()
        };
        assert_eq!(
            crate::menu_screens::resolution_popup_title(&[]),
            "No display sizes are available."
        );
        assert_eq!(
            crate::menu_screens::resolution_popup_title(&[render]),
            "Render sizes, not exclusive display modes."
        );
        assert_eq!(
            crate::menu_screens::resolution_popup_title(&[qualified]),
            "Sizes from the qualified client and this monitor. Apply changes the window render size."
        );
        let popup = OfferedResolution {
            monitor: Some(Entity::from_raw_u32(3).unwrap()),
            monitor_ordinal: Some(2),
            render_extent_only: false,
            ..OfferedResolution::render_size(1024, 768)
        };
        assert_eq!(
            crate::menu_screens::resolution_popup_title(&[popup]),
            crate::menu_screens::POPUP_WORK_AREA_NOTICE
        );
    }

    #[test]
    fn clicked_monitor_keeps_intent_when_its_entity_disappears() {
        let kept = Entity::from_raw_u32(3).unwrap();
        let other = Entity::from_raw_u32(9).unwrap();
        let first = OfferedResolution {
            monitor: Some(kept),
            monitor_ordinal: Some(0),
            render_extent_only: false,
            ..OfferedResolution::render_size(800, 600)
        };
        let second = OfferedResolution {
            monitor: Some(other),
            monitor_ordinal: Some(1),
            ..first.clone()
        };
        let mut nav = Navigation {
            resolutions: vec![first.clone(), second.clone()],
            ..default()
        };
        nav.draft.resolution = Some((800, 600));
        select_offer(&mut nav, &first).unwrap();
        let mut reordered_other = second;
        reordered_other.monitor_ordinal = Some(0);
        let mut reordered_kept = first;
        reordered_kept.monitor_ordinal = Some(1);
        nav.resolutions = vec![reordered_other.clone(), reordered_kept];
        choose_offer(&mut nav, None);
        let selected = nav.selected_offer.as_ref().unwrap();
        assert_eq!(selected.monitor, Some(kept));
        assert_eq!(selected.monitor_ordinal, Some(1));
        assert_eq!(nav.draft.resolution, Some((800, 600)));
        nav.resolutions = vec![reordered_other];
        let witness = nav.selected_offer.clone();
        choose_offer(&mut nav, None);
        choose_offer(&mut nav, None);
        assert_eq!(nav.selected_offer, witness);
        assert_eq!(nav.selected_offer.as_ref().unwrap().monitor, Some(kept));
        assert_eq!(nav.draft.resolution, Some((800, 600)));
        assert_eq!(nav.resolutions.len(), 1);
        assert_eq!(nav.resolutions[0].monitor, Some(other));
        assert_eq!(nav.resolutions[0].size, (800, 600));
        assert!(!nav.resolution_highlighted(&nav.resolutions[0]));
        assert!(nav.offers_size((800, 600)));
        let backend = Backend::default();
        assert_eq!(
            prepare(&nav, &backend).err().unwrap(),
            "Selected resolution is unavailable on this display"
        );
        let remaining = nav.resolutions[0].clone();
        select_offer(&mut nav, &remaining).unwrap();
        assert_eq!(nav.selected_offer.as_ref().unwrap().monitor, Some(other));
        assert_eq!(
            nav.selected_offer.as_ref().unwrap().monitor_ordinal,
            Some(0)
        );
        assert_eq!(nav.draft.resolution, Some((800, 600)));
        assert!(nav.resolution_highlighted(&nav.resolutions[0]));
        assert!(prepare(&nav, &backend).is_ok());
    }

    fn seed(gain: f32) -> original::Seed {
        let range = Range {
            min: 10.0,
            max: 90.0,
            step: 6.0,
        };
        original::Seed {
            audio: AudioSettings {
                music_gain: gain,
                effects_gain: 0.4,
                music_enabled: false,
                effects_enabled: true,
            },
            values: BTreeMap::from([
                ("MusicVolumeSlider".into(), range.value(gain)),
                ("SoundVolumeSlider".into(), 42.0),
            ]),
            toggles: BTreeMap::from([
                ("MusicCheckBox".into(), false),
                ("SoundCheckBox".into(), true),
            ]),
            ranges: BTreeMap::from([
                ("MusicVolumeSlider".into(), range),
                ("SoundVolumeSlider".into(), range),
            ]),
            origin: pcx_play::scripts::AssetIdentity {
                edition: "synthetic".into(),
                version: "synthetic".into(),
                source: "synthetic".into(),
            },
            image_sha256: [0x25; 32],
        }
    }

    #[test]
    fn source_fallback_activates_all_channels_and_baselines_without_saving() {
        let mut nav = Navigation::default();
        let mut backend = Backend {
            loaded: true,
            ..default()
        };
        admit_source_audio(&mut nav, &mut backend, Some(seed(0.2625))).unwrap();
        assert!(!backend.has_preferences);
        assert_eq!(backend.audio.music_gain, 0.2625);
        assert!(!backend.audio.music_enabled);
        assert_eq!(nav.applied, nav.draft);
        assert_eq!(nav.applied.values["MusicVolumeSlider"], 31.0);
        nav.draft.values.insert("MusicVolumeSlider".into(), 88.0);
        nav.action("CancelButton");
        assert_eq!(nav.applied, nav.draft);
        assert_eq!(backend.audio.music_gain, 0.2625);
    }

    #[test]
    fn saved_gains_win_and_reproject_even_when_old_units_are_in_the_new_range() {
        let mut nav = Navigation::default();
        nav.applied.values.insert("MusicVolumeSlider".into(), 70.0);
        nav.draft = nav.applied.clone();
        let mut backend = Backend {
            loaded: true,
            has_preferences: true,
            ..default()
        };
        backend.audio.music_gain = 0.25;
        admit_source_audio(&mut nav, &mut backend, Some(seed(0.75))).unwrap();
        assert_eq!(backend.audio.music_gain, 0.25);
        assert_eq!(nav.applied.values["MusicVolumeSlider"], 30.0);
        assert_eq!(nav.applied, nav.draft);
        assert!(backend.has_preferences);
    }

    #[test]
    fn unreadable_preferences_and_pending_reads_preserve_dirty_state() {
        for loaded in [false, true] {
            let mut backend = Backend {
                loaded,
                preference_unreadable: loaded.then(|| "synthetic IO refusal".into()),
                ..default()
            };
            let mut nav = Navigation::default();
            nav.draft.values.insert("MusicVolumeSlider".into(), 17.0);
            let old = nav.draft.clone();
            assert!(admit_source_audio(&mut nav, &mut backend, Some(seed(0.25))).is_err());
            assert_eq!(nav.draft, old);
            assert!(nav.applied.values.is_empty());
            assert_eq!(backend.audio.music_gain, 1.0);
            assert!(backend.ranges.is_empty());
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    #[ignore = "requires explicit local selected install and expected complete client SHA256"]
    fn original_audio_defaults_match_selected_source_before_playback() {
        let install =
            std::env::var_os("PEACECRAFT3_OPTIONS_INSTALL").expect("explicit local install");
        let expected =
            std::env::var("PEACECRAFT3_OPTIONS_IMAGE_SHA256").expect("expected client SHA256");
        let seed = prepare_source_audio(std::path::Path::new(&install), true)
            .unwrap()
            .expect("supported source profile");
        let actual: String = seed
            .image_sha256
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(actual, expected);
        let expected_units = seed.values.clone();
        let expected_flags = seed.toggles.clone();
        let audio = seed.audio;
        let mut nav = Navigation::default();
        let mut backend = Backend {
            loaded: true,
            ..default()
        };
        admit_source_audio(&mut nav, &mut backend, Some(seed)).unwrap();
        assert_eq!(nav.applied.values, expected_units);
        assert_eq!(nav.applied.toggles, expected_flags);
        assert_eq!(nav.applied, nav.draft);
        assert_eq!(
            backend.audio.music_gain.to_bits(),
            audio.music_gain.to_bits()
        );
        assert_eq!(
            backend.audio.effects_gain.to_bits(),
            audio.effects_gain.to_bits()
        );
        assert_eq!(backend.audio.music_enabled, audio.music_enabled);
        assert_eq!(backend.audio.effects_enabled, audio.effects_enabled);
        assert!(!backend.has_preferences);
    }
    #[test]
    fn unchanged_controls_preserve_the_saved_normalized_gain_bits() {
        let mut backend = Backend {
            loaded: true,
            has_preferences: true,
            ..default()
        };
        backend.audio.music_gain = f32::from_bits(0x3eaaaaab);
        let range = Range {
            min: -13.0,
            max: 73.0,
            step: 7.0,
        };
        backend.ranges.insert("MusicVolumeSlider".into(), range);
        let mut settings = Settings::default();
        settings.values.insert(
            "MusicVolumeSlider".into(),
            range.value(backend.audio.music_gain),
        );
        let after = backend.candidate_audio(&settings, &settings).unwrap();
        assert_eq!(
            after.music_gain.to_bits(),
            backend.audio.music_gain.to_bits()
        );
        assert_eq!(range.value(0.0), range.min);
        assert_eq!(range.value(1.0), range.max);
    }
}

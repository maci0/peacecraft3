//! Supported user preferences. Slider units come from the current FDF widgets;
//! display choices come from the current window/monitor or browser environment.
use crate::menu_screens::{Navigation, SettingControl, SettingValue, Settings, Widget};
use bevy::{
    prelude::*,
    ui::{Checked, InteractionDisabled},
    ui_widgets::{
        Checkbox, Slider, SliderOrientation, SliderRange, SliderStep, SliderValue, TrackClick,
        ValueChange,
    },
    window::{Monitor, OnMonitor, PrimaryWindow},
};
use std::collections::BTreeMap;

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
    #[cfg(any(test, not(target_arch = "wasm32")))]
    pub fn effective_music(self) -> f32 {
        if self.music_enabled {
            self.music_gain
        } else {
            0.0
        }
    }
    #[cfg(any(test, not(target_arch = "wasm32")))]
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
    fn snap(self, value: f32) -> f32 {
        crate::menu_screens::slider_value(
            self.min,
            self.max,
            self.step,
            (value - self.min) / (self.max - self.min),
        )
    }
}
#[derive(Resource, Default)]
pub struct Backend {
    pub audio: AudioSettings,
    ranges: BTreeMap<String, Range>,
    loaded: bool,
    has_preferences: bool,
    pending_resolution: Option<(u32, u32)>,
    observed_resolution: Option<(u32, u32)>,
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
                    *target = range.gain(value)?;
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

pub fn select_resolution(nav: &mut Navigation, resolution: (u32, u32)) -> Result<(), String> {
    if !nav.resolutions.contains(&resolution) {
        return Err("That resolution is no longer available on this display".into());
    }
    nav.draft.resolution = Some(resolution);
    Ok(())
}
/// Commit supported preferences. Validation/storage failures leave applied
/// state and the window unchanged. A later browser audio failure is reported
/// explicitly as a post-commit failure. Cancel is Navigation's draft reset.
pub fn apply(
    nav: &mut Navigation,
    window: &mut Window,
    backend: &mut Backend,
) -> Result<(), String> {
    commit(nav, window, backend, save_preferences)?;
    #[cfg(target_arch = "wasm32")]
    push_browser_audio(backend.audio)
        .map_err(|error| format!("Settings were saved and the display applied, but {error}"))?;
    Ok(())
}
fn commit(
    nav: &mut Navigation,
    window: &mut Window,
    backend: &mut Backend,
    save: impl FnOnce(&str) -> Result<(), String>,
) -> Result<(), String> {
    validate_settings(&nav.draft)?;
    if nav
        .draft
        .resolution
        .is_some_and(|r| !nav.resolutions.contains(&r))
    {
        return Err("Selected resolution is unavailable on this display".into());
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
    save(&text)?;
    if let Some((width, height)) = nav.draft.resolution {
        #[cfg(target_arch = "wasm32")]
        {
            window.fit_canvas_to_parent = false;
        }
        window.resolution.set_physical_resolution(width, height);
    }
    nav.applied = nav.draft.clone();
    backend.audio = audio;
    backend.has_preferences = true;
    Ok(())
}

pub fn install(app: &mut App) {
    app.init_resource::<Backend>()
        .add_observer(slider_changed)
        .add_observer(checkbox_changed)
        .add_systems(
            Update,
            (environment, attach_controls, labels)
                .chain()
                .before(crate::launch_gui::LauncherInput),
        );
}
fn observe_resolution(nav: &mut Navigation, backend: &mut Backend, current: (u32, u32)) {
    let changed = backend
        .observed_resolution
        .replace(current)
        .is_some_and(|previous| previous != current);
    if changed && backend.pending_resolution.is_none() {
        let previous = nav.applied.resolution.replace(current);
        if nav.draft.resolution == previous {
            nav.draft.resolution = Some(current);
        }
    }
}

fn environment(
    mut nav: ResMut<Navigation>,
    mut backend: ResMut<Backend>,
    mut windows: Query<(&mut Window, Option<&OnMonitor>), With<PrimaryWindow>>,
    monitors: Query<&Monitor>,
) {
    let Ok((mut window, on_monitor)) = windows.single_mut() else {
        return;
    };
    let current = (window.physical_width(), window.physical_height());
    let mut modes = vec![current];
    if let Some(monitor) = on_monitor.and_then(|on| monitors.get(on.0).ok()) {
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
    modes.retain(|(w, h)| *w > 0 && *h > 0);
    modes.sort_unstable();
    modes.dedup();
    nav.resolutions = modes;
    observe_resolution(&mut nav, &mut backend, current);
    if !backend.loaded {
        backend.loaded = true;
        match load_preferences()
            .and_then(|text| text.map(|text| decode_preferences(&text)).transpose())
        {
            Ok(Some(prefs)) => {
                nav.applied = prefs.settings;
                backend.audio = prefs.audio;
                backend.has_preferences = true;
            }
            Ok(None) => {}
            Err(error) => nav.notice = error,
        }
        backend.pending_resolution = nav.applied.resolution;
        if nav.applied.resolution.is_none() {
            nav.applied.resolution = Some(current);
        }
        nav.draft = nav.applied.clone();
        #[cfg(target_arch = "wasm32")]
        if let Err(error) = push_browser_audio(backend.audio) {
            nav.notice = error;
        }
    }
    if let Some(resolution) = backend.pending_resolution {
        if nav.resolutions.contains(&resolution) {
            #[cfg(target_arch = "wasm32")]
            {
                window.fit_canvas_to_parent = false;
            }
            window
                .resolution
                .set_physical_resolution(resolution.0, resolution.1);
            backend.pending_resolution = None;
        } else if on_monitor.is_some() || cfg!(target_arch = "wasm32") {
            nav.applied.resolution = Some(current);
            nav.draft.resolution = Some(current);
            backend.pending_resolution = None;
            nav.notice =
                "Saved resolution is unavailable on this display; using the current window".into();
        }
    }
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
                let current = if backend.has_preferences {
                    range.snap(range.min + gain * (range.max - range.min))
                } else {
                    // A neutral playback multiplier is not an authored UI default.
                    // Keep the source initial in the draft until the user applies it.
                    *initial
                };
                let existing = nav.applied.values.get(&control.name).copied();
                let value = match existing {
                    Some(value) if range.valid(value) => value,
                    Some(_) => {
                        nav.notice = format!(
                            "{}: saved value is outside the current authored range",
                            control.name
                        );
                        current
                    }
                    None => current,
                };
                let draft = nav
                    .draft
                    .values
                    .entry(control.name.clone())
                    .or_insert(value);
                if !range.valid(*draft) {
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
) {
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
) {
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
fn labels(
    nav: Res<Navigation>,
    mut labels: Query<(&SettingValue, &mut Text)>,
    controls: Query<(&SettingControl, Has<Checked>)>,
) {
    for (owner, mut text) in &mut labels {
        let Ok((control, checked)) = controls.get(owner.0) else {
            continue;
        };
        let value = match &control.widget {
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
                .map(|(w, h)| format!("{w} x {h}"))
                .unwrap_or_default(),
            Widget::Popup { .. } => "Unavailable".to_string(),
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
    #[wasm_bindgen(catch,js_namespace=peacecraft3Settings,js_name=displayModes)]
    fn browser_modes() -> Result<String, wasm_bindgen::JsValue>;
    #[wasm_bindgen(catch,js_namespace=peacecraft3Settings,js_name=load)]
    fn browser_load(limit: usize) -> Result<Option<String>, wasm_bindgen::JsValue>;
    #[wasm_bindgen(catch,js_namespace=peacecraft3Settings,js_name=save)]
    fn browser_save(text: &str, limit: usize) -> Result<(), wasm_bindgen::JsValue>;
    #[wasm_bindgen(catch,js_namespace=peacecraft3Audio,js_name=setUserGains)]
    fn browser_audio(
        music: f32,
        effects: f32,
        music_enabled: bool,
        effects_enabled: bool,
    ) -> Result<(), wasm_bindgen::JsValue>;
}
#[cfg(target_arch = "wasm32")]
fn load_preferences() -> Result<Option<String>, String> {
    browser_load(MAX_PREFERENCE_BYTES).map_err(|e| format!("Loading browser settings: {e:?}"))
}
#[cfg(target_arch = "wasm32")]
fn save_preferences(text: &str) -> Result<(), String> {
    browser_save(text, MAX_PREFERENCE_BYTES).map_err(|e| format!("Saving browser settings: {e:?}"))
}
#[cfg(target_arch = "wasm32")]
fn push_browser_audio(audio: AudioSettings) -> Result<(), String> {
    browser_audio(
        audio.music_gain,
        audio.effects_gain,
        audio.music_enabled,
        audio.effects_enabled,
    )
    .map_err(|e| format!("Applying browser audio settings: {e:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn external_resize_does_not_block_an_audio_only_commit() {
        let mut nav = Navigation::default();
        let mut backend = Backend::default();
        observe_resolution(&mut nav, &mut backend, (740, 510));
        nav.applied.resolution = Some((740, 510));
        nav.draft = nav.applied.clone();
        nav.resolutions = vec![(960, 650)];
        observe_resolution(&mut nav, &mut backend, (960, 650));
        assert_eq!(nav.applied.resolution, Some((960, 650)));
        assert_eq!(nav.draft.resolution, Some((960, 650)));
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
    fn choosing_resolution_changes_only_draft_and_refuses_stale_modes() {
        let mut nav = Navigation {
            resolutions: vec![(740, 510), (1480, 1020)],
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
        assert!(world.resource::<Navigation>().applied.values.is_empty());
        assert_eq!(
            world.resource::<Navigation>().draft.values["MusicVolumeSlider"],
            0.0
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
        assert!(world.resource::<Navigation>().draft.values.is_empty());
        assert_eq!(world.resource::<Backend>().audio.effective_music(), 1.0);
    }
    #[test]
    fn unsaved_slider_uses_authored_initial_without_changing_active_audio() {
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
        assert_eq!(world.get::<SliderValue>(control).unwrap().0, 19.0);
        assert_eq!(
            world.resource::<Navigation>().draft.values["MusicVolumeSlider"],
            19.0
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
            0.5
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
            resolutions: vec![(740, 510)],
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
}

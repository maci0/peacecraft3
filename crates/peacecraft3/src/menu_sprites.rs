//! The glue sprite layers over the main menu's 3D background: the
//! `GlueSpriteLayerTopLeft` and `GlueSpriteLayerTopRight` panels, their
//! `MainMenu Birth` entrance and the sounds its event objects fire.
//!
//! Everything is read from the install. The model paths come from
//! `UI\war3skins.txt`; the sequences, bones, geoset opacity and `SNDX` event
//! frames from each model; each event's file and volume from
//! `UI\SoundInfo\AnimLookups.slk` and `AnimSounds.slk`. The panels are posed by
//! the same skinning path as the background ([`scene::skin_geosets`]) and drawn
//! through an orthographic camera over the authored `0.8 × 0.6` glue frame,
//! below the UI. See `docs/re-notes/menu-sprite-rendering.md`.

use std::sync::Arc;

use bevy::camera::ScalingMode;
use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::prelude::*;

use crate::mainmenu_scene as scene;

/// The sprite-layer skin keys the glue initializer builds as `%s_V%d`, where the
/// number is the edition (`0` Reign of Chaos, `1` The Frozen Throne). Both names
/// are strings in `game.dll`.
pub const LAYERS: [&str; 2] = ["GlueSpriteLayerTopLeft", "GlueSpriteLayerTopRight"];
/// The main menu's animation tag. `game.dll`'s tag table lists `mainmenu` with
/// the other glue screens, beside `Death`, `Birth` and `Stand`.
const SCREEN: &str = "MainMenu";
const BIRTH: &str = "Birth";
const STAND: &str = "Stand";
/// `EVTS` object name prefix of a sound event; the four characters after it are
/// the event's `AnimLookups.slk` key.
const SOUND_EVENT: &str = "SNDX";
const SOUND_CODE_LEN: usize = 4;
pub const ANIM_LOOKUPS: &str = r"UI\SoundInfo\AnimLookups.slk";
pub const ANIM_SOUNDS: &str = r"UI\SoundInfo\AnimSounds.slk";
/// Full scale of the sound tables' `Volume` column.
const VOLUME_FULL: f32 = 127.0;
/// Shared by the panel camera and the panel geosets, so the background camera
/// does not draw the panels and the panel camera does not draw the background.
const RENDER_LAYER: usize = 7;
/// Draws after the background camera (`-1`) and before the UI camera (`1`).
const CAMERA_ORDER: isize = 0;

/// Why the panels, or one of their sounds, could not be read.
#[derive(Debug, thiserror::Error)]
pub enum PanelError {
    #[error("{0} is not readable")]
    Missing(String),
    #[error("{path}: {reason}")]
    Decode { path: String, reason: String },
    #[error("skin table: {0}")]
    Skin(crate::menu_skin::SkinError),
    #[error("{path} has no {name:?} sequence")]
    NoSequence { path: String, name: String },
    #[error("sound event {event}: {reason}")]
    Sound { event: String, reason: String },
    #[error(transparent)]
    Scene(#[from] scene::SceneError),
}

/// A sound event the entrance fires, resolved to its file.
#[derive(Debug, Clone)]
pub struct EventSound {
    /// The model frame the event fires at.
    pub frame: u32,
    pub path: String,
    /// Linear gain, `Volume / 127`.
    pub volume: f32,
    pub bytes: Arc<[u8]>,
}

/// One panel as read from the install, before anything is spawned.
pub struct PanelSource {
    pub path: String,
    pub bytes: Vec<u8>,
    pub timeline: Timeline,
    pub sounds: Vec<EventSound>,
}

/// `MainMenu Birth` once, then `MainMenu Stand`, in model frames (milliseconds).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Timeline {
    pub birth: [u32; 2],
    pub stand: [u32; 2],
    /// The `Stand` sequence's `SEQS` looping flag.
    pub stand_loops: bool,
}

impl Timeline {
    fn birth_ms(&self) -> f64 {
        f64::from(self.birth[1] - self.birth[0])
    }

    fn in_birth(&self, ms: f64) -> bool {
        ms < self.birth_ms()
    }

    pub(crate) fn texture_selection<'a>(
        &self,
        ms: f64,
        birth: &'a pcx_gfx::mdx_texture_selection::TextureSelectionPlan,
        stand: &'a pcx_gfx::mdx_texture_selection::TextureSelectionPlan,
    ) -> &'a pcx_gfx::mdx_texture_selection::TextureSelectionPlan {
        if self.in_birth(ms) { birth } else { stand }
    }

    pub(crate) fn uv_translation<'a>(
        &self,
        ms: f64,
        birth: &'a pcx_gfx::mdx_uv_translation::UvTranslationPlan,
        stand: &'a pcx_gfx::mdx_uv_translation::UvTranslationPlan,
    ) -> &'a pcx_gfx::mdx_uv_translation::UvTranslationPlan {
        if self.in_birth(ms) { birth } else { stand }
    }

    /// The model frame `ms` after the entrance began.
    fn frame_at(&self, ms: f64) -> u32 {
        if self.in_birth(ms) {
            return self.birth[0] + ms as u32;
        }
        let period = self.stand[1] - self.stand[0];
        let into = (ms - self.birth_ms()) as u64;
        if !self.stand_loops || period == 0 {
            return self.stand[0] + into.min(u64::from(period)) as u32;
        }
        self.stand[0] + (into % u64::from(period)) as u32
    }

    /// Whether an event at `frame` fires while the clock moves over `[from, to)`.
    fn fires(&self, frame: u32, from: f64, to: f64) -> bool {
        if (self.birth[0]..=self.birth[1]).contains(&frame) {
            let at = f64::from(frame - self.birth[0]);
            return from <= at && at < to;
        }
        if !(self.stand[0]..=self.stand[1]).contains(&frame) {
            return false;
        }
        let first = self.birth_ms() + f64::from(frame - self.stand[0]);
        let period = f64::from(self.stand[1] - self.stand[0]);
        if !self.stand_loops || period == 0.0 {
            return from <= first && first < to;
        }
        let loops = ((from - first) / period).ceil().max(0.0);
        first + loops * period < to
    }
}

/// Whether a sequence name is exactly the screen tag and `anim`, in any order
/// and case, as the engine's lowercase tag table implies.
fn tagged(name: &str, anim: &str) -> bool {
    let tokens: Vec<&str> = name.split_whitespace().collect();
    tokens.len() == 2
        && tokens.iter().any(|t| t.eq_ignore_ascii_case(SCREEN))
        && tokens.iter().any(|t| t.eq_ignore_ascii_case(anim))
}

/// The skin table names `.mdl` paths; the archives ship only the binary `.mdx`
/// for these keys. The native extension rewrite has not been traced.
fn binary_model_path(path: &str) -> String {
    match path.len().checked_sub(4).and_then(|at| path.get(at..)) {
        Some(ext) if ext.eq_ignore_ascii_case(".mdl") => format!("{}.mdx", &path[..path.len() - 4]),
        _ => path.to_string(),
    }
}

/// The selected skin's panel bindings, shared by native reads and local imports.
pub fn model_paths(
    skin: &crate::menu_skin::SkinTable,
    edition: u32,
) -> Result<Vec<String>, crate::menu_skin::SkinError> {
    LAYERS
        .iter()
        .map(|layer| {
            skin.unique_file(&format!("{layer}_V{edition}"))
                .map(|binding| binary_model_path(&binding.file))
        })
        .collect()
}

fn read(install: Option<&std::path::Path>, path: &str) -> Result<Vec<u8>, PanelError> {
    crate::mainmenu::art_bytes_named(install, path, path)
        .ok_or_else(|| PanelError::Missing(path.to_string()))
}

fn table(install: Option<&std::path::Path>, path: &str) -> Result<pcx_slq::Table, PanelError> {
    pcx_slq::Table::parse(&read(install, path)?).map_err(|e| PanelError::Decode {
        path: path.to_string(),
        reason: e.to_string(),
    })
}

/// The file and volume an `SNDX` event plays, from the two sound tables.
///
/// `AnimLookups.slk` maps the event's code to a sound label; `AnimSounds.slk`
/// maps the label to `DirectoryBase`, `FileNames` and `Volume`.
pub fn resolve_sound(
    lookups: &pcx_slq::Table,
    sounds: &pcx_slq::Table,
    event: &str,
) -> Result<(String, f32), PanelError> {
    let fail = |reason: String| PanelError::Sound {
        event: event.to_string(),
        reason,
    };
    let code = event
        .get(SOUND_EVENT.len()..)
        .filter(|code| code.len() == SOUND_CODE_LEN)
        .ok_or_else(|| fail("not an SNDX name".into()))?;
    let label = lookups
        .data_rows()
        .find(|row| row.by_name("AnimSoundEvent") == Some(code))
        .and_then(|row| row.by_name("SoundLabel"))
        .ok_or_else(|| fail(format!("{ANIM_LOOKUPS} has no {code:?}")))?;
    let row = sounds
        .data_rows()
        .find(|row| row.by_name("SoundName") == Some(label))
        .ok_or_else(|| fail(format!("{ANIM_SOUNDS} has no {label:?}")))?;
    let files = row
        .by_name("FileNames")
        .ok_or_else(|| fail(format!("{label:?} has no FileNames")))?;
    // ponytail: the engine picks among comma-separated files at random; every
    // menu panel sound names one, so a list is refused until one needs it.
    if files.contains(',') {
        return Err(fail(format!("{label:?} lists several files: {files}")));
    }
    let directory = row.by_name("DirectoryBase").unwrap_or_default();
    let volume = row
        .by_name("Volume")
        .and_then(|v| v.parse::<f32>().ok())
        .filter(|v| (0.0..=VOLUME_FULL).contains(v))
        .ok_or_else(|| fail(format!("{label:?} has no Volume in 0..=127")))?;
    Ok((format!("{directory}{files}"), volume / VOLUME_FULL))
}

/// Reads both panels for `edition` from the install (or, in a browser, from the
/// imported archives).
///
/// A panel without its model or sequences is an error. A sound that cannot be
/// resolved is logged and left out, so the entrance still plays without it.
pub fn load(
    install: Option<&std::path::Path>,
    edition: u32,
) -> Result<Vec<PanelSource>, PanelError> {
    let skins_text = crate::mainmenu::art_bytes(install, crate::mainmenu::SKIN_TABLE)
        .ok_or_else(|| PanelError::Missing(crate::mainmenu::SKIN_TABLE.to_string()))?;
    let skins = crate::menu_skin::SkinTable::parse(&String::from_utf8_lossy(&skins_text))
        .map_err(PanelError::Skin)?;
    let tables = table(install, ANIM_LOOKUPS).and_then(|l| Ok((l, table(install, ANIM_SOUNDS)?)));
    if let Err(error) = &tables {
        warn!("Menu panel sounds unavailable: {error}");
    }
    let mut out = Vec::with_capacity(LAYERS.len());
    for path in model_paths(&skins, edition).map_err(PanelError::Skin)? {
        let bytes = read(install, &path)?;
        let decode = |reason: String| PanelError::Decode {
            path: path.clone(),
            reason,
        };
        let m = pcx_mdl::Mdlx::parse(&bytes).map_err(|e| decode(e.to_string()))?;
        let sequences = m.sequences(&bytes).map_err(|e| decode(e.to_string()))?;
        let find = |anim: &str| {
            sequences
                .iter()
                .find(|s| tagged(&s.name, anim))
                .ok_or_else(|| PanelError::NoSequence {
                    path: path.clone(),
                    name: format!("{SCREEN} {anim}"),
                })
        };
        let (birth, stand) = (find(BIRTH)?, find(STAND)?);
        let timeline = Timeline {
            birth: [birth.interval_start, birth.interval_end],
            stand: [stand.interval_start, stand.interval_end],
            stand_loops: !stand.non_looping,
        };
        let events = m
            .event_nodes(&bytes, pcx_mdl::tag::EVTS)
            .map_err(|e| decode(e.to_string()))?;
        let mut sounds = Vec::new();
        for event in &events {
            let is_sound = event
                .name
                .get(..SOUND_EVENT.len())
                .is_some_and(|p| p.eq_ignore_ascii_case(SOUND_EVENT));
            let frames: Vec<u32> = event
                .frames
                .iter()
                .copied()
                .filter(|&f| {
                    (timeline.birth[0]..=timeline.birth[1]).contains(&f)
                        || (timeline.stand[0]..=timeline.stand[1]).contains(&f)
                })
                .collect();
            if !is_sound || frames.is_empty() {
                continue;
            }
            let Ok((lookups, table)) = &tables else {
                continue;
            };
            let resolved = resolve_sound(lookups, table, &event.name)
                .and_then(|(file, volume)| Ok((read(install, &file)?, file, volume)));
            match resolved {
                Ok((wav, file, volume)) => {
                    let wav: Arc<[u8]> = wav.into();
                    sounds.extend(frames.into_iter().map(|frame| EventSound {
                        frame,
                        path: file.clone(),
                        volume,
                        bytes: wav.clone(),
                    }));
                }
                Err(error) => warn!("{path}: {error}"),
            }
        }
        out.push(PanelSource {
            path,
            bytes,
            timeline,
            sounds,
        });
    }
    Ok(out)
}

/// One spawned panel.
struct Panel {
    model: pcx_gfx::LoadedModel,
    bones: Vec<pcx_mdl::Bone>,
    pivots: pcx_mdl::Pivots,
    global_durations: Vec<u32>,
    geosets: scene::Geosets,
    timeline: Timeline,
    birth_opacity: pcx_gfx::mdx_opacity::OpacityPlan,
    stand_opacity: pcx_gfx::mdx_opacity::OpacityPlan,
    birth_textures: pcx_gfx::mdx_texture_selection::TextureSelectionPlan,
    stand_textures: pcx_gfx::mdx_texture_selection::TextureSelectionPlan,
    birth_uv: pcx_gfx::mdx_uv_translation::UvTranslationPlan,
    stand_uv: pcx_gfx::mdx_uv_translation::UvTranslationPlan,
    sounds: Vec<EventSound>,
}

/// The live panels and the entrance clock they share.
#[derive(Resource)]
pub struct MenuPanels {
    panels: Vec<Panel>,
    elapsed: std::time::Duration,
}

/// Marker for the panel camera, whose viewport follows the UI camera's.
#[derive(Component)]
pub struct PanelCamera;

/// Spawns the panels and their camera. Every entity carries
/// [`scene::MainMenuSceneEntity`], so it goes when the background does.
pub fn spawn(
    commands: &mut Commands,
    assets: scene::GeosetAssets,
    install: Option<&std::path::Path>,
    sources: Vec<PanelSource>,
) -> Result<usize, PanelError> {
    let scene::GeosetAssets {
        meshes,
        materials,
        images,
    } = assets;
    let mut panels = Vec::with_capacity(sources.len());
    let (mut low, mut high) = (f32::MAX, f32::MIN);
    for source in sources {
        let PanelSource {
            path,
            bytes,
            timeline,
            sounds,
        } = source;
        let decode = |e: &dyn std::fmt::Display| PanelError::Decode {
            path: path.clone(),
            reason: e.to_string(),
        };
        let m = pcx_mdl::Mdlx::parse(&bytes).map_err(|e| decode(&e))?;
        let model = pcx_gfx::load_model(&bytes, &path).map_err(|e| decode(&e))?;
        let runtime = pcx_gfx::mdx_runtime::RuntimePlan::parse(&bytes).map_err(|e| decode(&e))?;
        let birth_opacity = pcx_gfx::mdx_opacity::OpacityPlan::checked(&runtime, timeline.birth)
            .map_err(|e| decode(&e))?;
        let stand_opacity = pcx_gfx::mdx_opacity::OpacityPlan::checked(&runtime, timeline.stand)
            .map_err(|e| decode(&e))?;
        let birth_textures = scene::texture_selection(&runtime, timeline.birth, &path)?;
        let stand_textures = scene::texture_selection(&runtime, timeline.stand, &path)?;
        let birth_uv = pcx_gfx::mdx_uv_translation::UvTranslationPlan::from_runtime(
            &runtime,
            timeline.birth,
            &path,
        )
        .map_err(scene::SceneError::from)?;
        let stand_uv = pcx_gfx::mdx_uv_translation::UvTranslationPlan::from_runtime(
            &runtime,
            timeline.stand,
            &path,
        )
        .map_err(scene::SceneError::from)?;
        let textures: Vec<pcx_gfx::LoadedTexture> = scene::texture_paths(&bytes)?
            .iter()
            .filter_map(|p| pcx_gfx::load_image(&read(install, p).ok()?, p).ok())
            .collect();
        let mut info = scene::SceneInfo::default();
        let geosets = scene::spawn_geosets(
            commands,
            scene::GeosetAssets {
                meshes,
                materials,
                images,
            },
            scene::GeosetSource {
                m: &m,
                bytes: &bytes,
                source: &path,
                model: &model,
                textures: &textures,
                runtime: &runtime,
                selection: &birth_textures,
                uv_translation: &birth_uv,
            },
            &birth_opacity,
            timeline.birth[0],
            &mut info,
            (
                scene::MainMenuSceneEntity,
                RenderLayers::layer(RENDER_LAYER),
                // Skinning rewrites positions without refreshing the bounds the
                // culler reads, and the entrance moves geosets into view.
                NoFrustumCulling,
            ),
        )?;
        if !info.missing_textures.is_empty() {
            warn!("{path}: missing textures {:?}", info.missing_textures);
        }
        let events: Vec<(u32, &str)> = sounds.iter().map(|s| (s.frame, s.path.as_str())).collect();
        info!(?timeline, ?events, "Menu panel {path}");
        low = low.min(model.minimum_extent[2]);
        high = high.max(model.maximum_extent[2]);
        panels.push(Panel {
            model,
            bones: runtime.joints.joints().to_vec(),
            pivots: runtime.joints.pivots().clone(),
            global_durations: runtime.global_sequences,
            geosets,
            timeline,
            birth_opacity,
            stand_opacity,
            birth_textures,
            stand_textures,
            birth_uv,
            stand_uv,
            sounds,
        });
    }
    // The default sprite projection adds model X/Y to the frame's left/bottom
    // (`docs/re-notes/glue-coordinate-runtime.md`), so the camera centres the
    // authored frame. Looking down -Z, depth runs from -maxZ to -minZ of the
    // models' own extents; transparent geosets sort by that depth.
    commands.spawn((
        Camera3d::default(),
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::Fixed {
                width: crate::mainmenu::FRAME_W,
                height: crate::mainmenu::FRAME_H,
            },
            near: -high,
            far: -low,
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_xyz(
            crate::mainmenu::FRAME_W / 2.0,
            crate::mainmenu::FRAME_H / 2.0,
            0.0,
        ),
        Camera {
            order: CAMERA_ORDER,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        // Glue art is texture colour, not scene radiance.
        Tonemapping::None,
        DebandDither::Disabled,
        RenderLayers::layer(RENDER_LAYER),
        PanelCamera,
        scene::MainMenuSceneEntity,
    ));
    let count = panels.len();
    commands.insert_resource(MenuPanels {
        panels,
        elapsed: std::time::Duration::ZERO,
    });
    Ok(count)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = peacecraft3Audio, js_name = event)]
    fn browser_event_sound(bytes: &[u8], volume: f32);
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct SoundSink<'w> {
    assets: ResMut<'w, Assets<AudioSource>>,
    settings: Res<'w, crate::menu_settings::Backend>,
}
#[cfg(target_arch = "wasm32")]
type SoundSink<'w> = ();

#[cfg(not(target_arch = "wasm32"))]
fn play(commands: &mut Commands, sink: &mut SoundSink, sound: &EventSound) {
    commands.spawn((
        AudioPlayer(sink.assets.add(AudioSource {
            bytes: sound.bytes.clone(),
        })),
        PlaybackSettings::DESPAWN.with_volume(bevy::audio::Volume::Linear(
            sound.volume * sink.settings.audio.effective_effects(),
        )),
        crate::menu_audio::AuthoredVolume(sound.volume),
    ));
}

#[cfg(target_arch = "wasm32")]
fn play(_: &mut Commands, _: &mut SoundSink, sound: &EventSound) {
    browser_event_sound(&sound.bytes, sound.volume);
}

/// Advances the entrance clock, fires the events it passed, and poses each
/// panel at its frame.
pub fn advance(
    time: Res<Time>,
    mut commands: Commands,
    mut state: ResMut<MenuPanels>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut visibility: Query<&mut Visibility, With<scene::MainMenuSceneEntity>>,
    mut sink: SoundSink,
) {
    let from = state.elapsed.as_secs_f64() * 1000.0;
    let elapsed_ms = match scene::advance_elapsed("menu panels", &mut state.elapsed, time.delta()) {
        Ok(elapsed) => elapsed,
        Err(error) => {
            error!("{error}");
            return;
        }
    };
    let to = state.elapsed.as_secs_f64() * 1000.0;
    for panel in &mut state.panels {
        for sound in &panel.sounds {
            if panel.timeline.fires(sound.frame, from, to) {
                play(&mut commands, &mut sink, sound);
            }
        }
        let frame = panel.timeline.frame_at(to);
        let opacity = if panel.timeline.in_birth(to) {
            &panel.birth_opacity
        } else {
            &panel.stand_opacity
        };
        let uv = panel
            .timeline
            .uv_translation(to, &panel.birth_uv, &panel.stand_uv);
        if let Err(error) = scene::apply_uv_translation(
            uv,
            &panel.geosets.textures,
            frame,
            elapsed_ms,
            &mut materials,
        ) {
            error!("{error}");
            continue;
        }
        let textures =
            panel
                .timeline
                .texture_selection(to, &panel.birth_textures, &panel.stand_textures);
        if let Err(error) = scene::apply_texture_selection(
            textures,
            &mut panel.geosets.textures,
            frame,
            elapsed_ms,
            &mut materials,
        ) {
            error!("{error}");
            continue;
        }
        scene::apply_opacity(
            opacity,
            &panel.geosets.bound,
            frame,
            elapsed_ms,
            &mut materials,
            &mut visibility,
        );
        scene::skin_geosets(
            scene::Rig {
                model: &panel.model,
                bones: &panel.bones,
                pivots: &panel.pivots,
                global_durations: &panel.global_durations,
            },
            &panel.geosets.handles,
            frame,
            to as u32,
            &mut meshes,
        );
    }
}

/// Keeps the panel camera on the UI camera's viewport, which holds the authored
/// frame, and active only while the menu is shown.
pub fn follow_ui_viewport(
    menu: Query<(), With<crate::launch_gui::MenuLayer>>,
    ui: Query<&Camera, (With<bevy::ui::IsDefaultUiCamera>, Without<PanelCamera>)>,
    mut panels: Query<&mut Camera, With<PanelCamera>>,
) {
    let Ok(ui) = ui.single() else {
        return;
    };
    let key = |v: &Option<bevy::camera::Viewport>| {
        v.as_ref().map(|v| (v.physical_position, v.physical_size))
    };
    for mut camera in &mut panels {
        if key(&camera.viewport) != key(&ui.viewport) {
            camera.viewport = ui.viewport.clone();
        }
        let active = !menu.is_empty();
        if camera.is_active != active {
            camera.is_active = active;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TIMELINE: Timeline = Timeline {
        birth: [2500, 3500],
        stand: [3667, 4667],
        stand_loops: true,
    };

    #[test]
    fn birth_plays_once_then_stand_loops() {
        assert_eq!(TIMELINE.frame_at(0.0), 2500);
        assert_eq!(TIMELINE.frame_at(999.5), 3499);
        assert_eq!(TIMELINE.frame_at(1000.0), 3667);
        assert_eq!(TIMELINE.frame_at(1250.0), 3917);
        assert_eq!(TIMELINE.frame_at(2000.0), 3667);
        let held = Timeline {
            stand_loops: false,
            ..TIMELINE
        };
        assert_eq!(held.frame_at(5000.0), 4667);
    }

    #[test]
    fn events_fire_once_per_pass() {
        // The first frame of Birth fires on the first tick, and only then.
        assert!(TIMELINE.fires(2500, 0.0, 16.0));
        assert!(!TIMELINE.fires(2500, 16.0, 32.0));
        assert!(TIMELINE.fires(3000, 490.0, 510.0));
        // A Stand event fires on every loop.
        assert!(TIMELINE.fires(3767, 1090.0, 1110.0));
        assert!(TIMELINE.fires(3767, 2090.0, 2110.0));
        assert!(!TIMELINE.fires(3767, 1110.0, 2090.0));
        // A frame in neither sequence never fires.
        assert!(!TIMELINE.fires(4833, 0.0, 1.0e9));
    }

    #[test]
    fn sequences_match_screen_and_anim_tokens() {
        assert!(tagged("MainMenu Birth", BIRTH));
        assert!(tagged("mainmenu  birth", BIRTH));
        assert!(!tagged("Birth", BIRTH));
        assert!(!tagged("MainMenu Stand", BIRTH));
        assert!(!tagged("SinglePlayer Birth Alternate", BIRTH));
    }

    #[test]
    fn skin_mdl_paths_load_as_mdx() {
        assert_eq!(binary_model_path(r"UI\A\B.mdl"), r"UI\A\B.mdx");
        assert_eq!(binary_model_path(r"UI\A\B.MDL"), r"UI\A\B.mdx");
        assert_eq!(binary_model_path(r"UI\A\B.mdx"), r"UI\A\B.mdx");
        assert_eq!(binary_model_path("x"), "x");
    }

    #[test]
    fn panel_models_follow_the_selected_skin_and_edition_without_fallback_paths() {
        let skins = crate::menu_skin::SkinTable::parse(
            "[Default]\nGlueSpriteLayerTopLeft_V0=Custom/left.mdl\nGlueSpriteLayerTopRight_V0=Custom/right.mdx\nGlueSpriteLayerTopLeft_V1=Other/one.MDL\nGlueSpriteLayerTopRight_V1=Other/two.mdx\n"
        ).unwrap();
        assert_eq!(
            model_paths(&skins, 0).unwrap(),
            ["Custom/left.mdx", "Custom/right.mdx"]
        );
        assert_eq!(
            model_paths(&skins, 1).unwrap(),
            ["Other/one.mdx", "Other/two.mdx"]
        );
        assert!(matches!(
            model_paths(&skins, 2),
            Err(crate::menu_skin::SkinError::UnknownFace(_))
        ));
    }

    fn slk(rows: &[&[&str]]) -> pcx_slq::Table {
        let mut text = String::from("ID;PWXL;N;E\r\n");
        for (y, row) in rows.iter().enumerate() {
            for (x, cell) in row.iter().enumerate() {
                let value = if cell.parse::<f32>().is_ok() {
                    cell.to_string()
                } else {
                    format!("\"{cell}\"")
                };
                text += &format!("C;X{};Y{};K{value}\r\n", x + 1, y + 1);
            }
        }
        text += "E\r\n";
        pcx_slq::Table::parse(text.as_bytes()).unwrap()
    }

    #[test]
    fn sound_events_resolve_through_both_tables() {
        let lookups = slk(&[
            &["AnimSoundEvent", "SoundLabel"],
            &["ARPD", "RightGlueScreenPopDown"],
            &["AMNY", "Many"],
        ]);
        let sounds = slk(&[
            &["SoundName", "FileNames", "DirectoryBase", "Volume"],
            &[
                "RightGlueScreenPopDown",
                "Pop.wav",
                r"Sound\Interface\",
                "64",
            ],
            &["Many", "A.wav,B.wav", r"Sound\", "64"],
        ]);
        let (path, volume) = resolve_sound(&lookups, &sounds, "SNDXARPD").unwrap();
        assert_eq!(path, r"Sound\Interface\Pop.wav");
        assert_eq!(volume, 64.0 / 127.0);
        assert!(resolve_sound(&lookups, &sounds, "SNDXNONE").is_err());
        assert!(resolve_sound(&lookups, &sounds, "SNDXAMNY").is_err());
        assert!(resolve_sound(&lookups, &sounds, "SNDX").is_err());
    }

    #[test]
    #[ignore = "requires PEACECRAFT3_INSTALL with the original TFT archives"]
    fn original_panels_spawn_with_checked_birth_and_stand_opacity() {
        let install = std::path::PathBuf::from(
            std::env::var_os("PEACECRAFT3_INSTALL").expect("explicit original install required"),
        );
        let sources = load(Some(&install), 1).unwrap();
        let expected = sources.len();
        assert!(expected > 0);
        let mut world = World::new();
        let mut meshes = Assets::<Mesh>::default();
        let mut materials = Assets::<StandardMaterial>::default();
        let mut images = Assets::<Image>::default();
        let count = spawn(
            &mut world.commands(),
            scene::GeosetAssets {
                meshes: &mut meshes,
                materials: &mut materials,
                images: &mut images,
            },
            Some(&install),
            sources,
        )
        .unwrap();
        world.flush();
        assert_eq!(count, expected);
        let panels = world.resource::<MenuPanels>();
        assert_eq!(panels.panels.len(), expected);
        assert!(panels.panels.iter().all(|p| !p.geosets.handles.is_empty()));
    }

    #[test]
    #[ignore = "requires PEACECRAFT3_INSTALL with the original TFT archives"]
    fn original_panels_load_with_entrance_and_pop_down_sound() {
        let install = std::path::PathBuf::from(
            std::env::var_os("PEACECRAFT3_INSTALL").expect("explicit original install required"),
        );
        let panels = load(Some(&install), 1).unwrap();
        assert_eq!(panels.len(), LAYERS.len());
        for panel in &panels {
            assert!(panel.path.ends_with("-Expansion.mdx"), "{}", panel.path);
            assert!(panel.timeline.birth[1] > panel.timeline.birth[0]);
        }
        let right = &panels[1];
        let pop = right
            .sounds
            .iter()
            .find(|s| s.frame == right.timeline.birth[0])
            .expect("the right panel's entrance fires a sound on its first frame");
        assert!(
            pop.path.ends_with("RightGlueScreenPopDown.wav"),
            "{}",
            pop.path
        );
        assert!(pop.bytes.starts_with(b"RIFF"));
    }
}

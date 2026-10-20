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

use crate::menu_material::MenuMaterial;
use bevy::camera::ScalingMode;
use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::prelude::*;
use pcx_mdl::binary_model_path;

use crate::mainmenu_scene as scene;
#[path = "menu_panel_phases.rs"]
mod phases;
pub use peacecraft3::sound_volume::VolumeScale;
use phases::{Clip, Clock, Owner, Phase};

pub const BLIZZARD_SCRIPT: &str = r"Scripts\Blizzard.j";
const MAX_MODEL_BYTES: usize = 16 * 1024 * 1024;
const MAX_SOUND_EVENTS: usize = 4096;
const MAX_SOUND_BYTES: usize = 16 * 1024 * 1024;
const MAX_TOTAL_SOUND_BYTES: usize = 64 * 1024 * 1024;
const MAX_SOUND_METADATA_BYTES: usize = 4096;

/// The sprite-layer skin keys the glue initializer builds as `%s_V%d`, where the
/// number is the edition (`0` Reign of Chaos, `1` The Frozen Throne). Both names
/// are strings in `game.dll`.
pub const LAYERS: [&str; 2] = ["GlueSpriteLayerTopLeft", "GlueSpriteLayerTopRight"];
/// The main menu's animation tag. `game.dll`'s tag table lists `mainmenu` with
/// the other glue screens, beside `Death`, `Birth` and `Stand`.
#[cfg(test)]
const SCREEN: &str = "MainMenu";
#[cfg(test)]
const BIRTH: &str = "Birth";
/// `EVTS` object name prefix of a sound event; the four characters after it are
/// the event's `AnimLookups.slk` key.
const SOUND_EVENT: &str = "SNDX";
const SOUND_CODE_LEN: usize = 4;
pub const ANIM_LOOKUPS: &str = r"UI\SoundInfo\AnimLookups.slk";
pub const ANIM_SOUNDS: &str = r"UI\SoundInfo\AnimSounds.slk";
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
    #[cfg(any(not(target_arch = "wasm32"), test))]
    #[error("{path} in {archive}: {source}")]
    NativeRead {
        path: String,
        archive: String,
        #[source]
        source: pcx_mpq::Error,
    },
    #[cfg(any(target_arch = "wasm32", test))]
    #[error("{path}: {source}")]
    BrowserRead {
        path: String,
        #[source]
        source: Box<peacecraft3::browser_read::ReadError>,
    },
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
    /// Linear recreation gain from Volume / the checked selected-script maximum.
    pub volume: f32,
    pub bytes: Arc<[u8]>,
    /// Original identity/digest/PCM bytes remain owned beside the decoder view.
    pub source: Arc<pcx_play::file_sounds::Assets>,
}

/// One panel as read from the install, before anything is spawned.
pub struct PanelSource {
    layer: &'static str,
    pub path: String,
    pub bytes: Vec<u8>,
    install: Option<std::path::PathBuf>,
    edition: u32,
    clips: Vec<Clip>,
    pub sounds: Vec<EventSound>,
    pub unavailable_sounds: Vec<UnavailableSound>,
    navigation: Result<(Vec<Clip>, LoadedSounds), String>,
    options: Result<(Vec<Clip>, LoadedSounds), String>,
    category: Result<(Vec<Clip>, LoadedSounds), String>,
}

/// `MainMenu Birth` once, then `MainMenu Stand`, in model frames (milliseconds).
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Timeline {
    pub birth: [u32; 2],
    pub stand: [u32; 2],
    /// The `Stand` sequence's `SEQS` looping flag.
    pub stand_loops: bool,
}

#[cfg(test)]
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
#[cfg(test)]
fn tagged(name: &str, anim: &str) -> bool {
    let tokens: Vec<&str> = name.split_whitespace().collect();
    tokens.len() == 2
        && tokens.iter().any(|t| t.eq_ignore_ascii_case(SCREEN))
        && tokens.iter().any(|t| t.eq_ignore_ascii_case(anim))
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

fn read(
    _install: Option<&std::path::Path>,
    path: &str,
    max_bytes: usize,
) -> Result<Vec<u8>, PanelError> {
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(install) = _install {
        let mut archives =
            pcx_mpq::MpqSet::open_wc3(install).map_err(|source| PanelError::NativeRead {
                path: path.into(),
                archive: install.display().to_string(),
                source,
            })?;
        let (index, block) = archives
            .locate(path)
            .ok_or_else(|| PanelError::Missing(path.into()))?;
        // Retain this caller's existing selected archive and codec behavior.
        // Both payload allocation sizes are checked before extract reads either.
        checked_member_size(path, block.compressed_size, block.file_size, max_bytes)?;
        let archive = archives.members()[index].path().display().to_string();
        return archives
            .extract(path)
            .map_err(|source| PanelError::NativeRead {
                path: path.into(),
                archive,
                source,
            });
    }
    #[cfg(target_arch = "wasm32")]
    {
        crate::browser::panel_bytes(path, max_bytes)
            .map_err(|source| PanelError::BrowserRead {
                path: path.into(),
                source: Box::new(source),
            })?
            .map(|asset| asset.bytes)
            .ok_or_else(|| PanelError::Missing(path.into()))
    }
    #[cfg(not(target_arch = "wasm32"))]
    Err(PanelError::Missing(path.into()))
}

#[cfg(any(not(target_arch = "wasm32"), test))]
fn checked_member_size(
    path: &str,
    packed: u32,
    decoded: u32,
    max_bytes: usize,
) -> Result<(), PanelError> {
    if u64::from(packed) > max_bytes as u64 || u64::from(decoded) > max_bytes as u64 {
        return Err(PanelError::Decode {
            path: path.into(),
            reason: "packed or decoded member exceeds panel read byte bound".into(),
        });
    }
    Ok(())
}

fn table(install: Option<&std::path::Path>, path: &str) -> Result<pcx_slq::Table, PanelError> {
    let bytes = read(install, path, 1024 * 1024)?;
    if bytes.len() > 1024 * 1024 {
        return Err(PanelError::Decode {
            path: path.into(),
            reason: "sound table byte bound exceeded".into(),
        });
    }
    pcx_slq::Table::parse_bounded(
        &bytes,
        pcx_slq::CoordinateLimits {
            max_row: 10_000,
            max_column: 256,
        },
    )
    .map_err(|e| PanelError::Decode {
        path: path.into(),
        reason: e.to_string(),
    })
}

fn checked_table(table: &pcx_slq::Table, path: &str, required: &[&str]) -> Result<(), PanelError> {
    let fail = |reason: &str| PanelError::Decode {
        path: path.into(),
        reason: reason.into(),
    };
    if table.cells().any(|(&(r, c), _)| r > 10_000 || c > 256) {
        return Err(fail("sound table coordinate bound exceeded"));
    }
    if table.anomalies().iter().any(|a| {
        a.severity == pcx_slq::Severity::Error
            || matches!(
                a.kind,
                pcx_slq::AnomalyKind::DuplicateCell | pcx_slq::AnomalyKind::StrayDataAfterEnd
            )
    }) {
        return Err(fail("lossy or erroneous sound table records"));
    }
    for name in required {
        if table.headers().filter(|(_, h)| h == name).count() != 1 {
            return Err(fail(&format!(
                "missing or duplicate required header {name}"
            )));
        }
    }
    Ok(())
}

/// The file and volume an `SNDX` event plays, from the two sound tables.
///
/// `AnimLookups.slk` maps the event's code to a sound label; `AnimSounds.slk`
/// maps the label to `DirectoryBase`, `FileNames` and `Volume`.
pub fn resolve_sound(
    lookups: &pcx_slq::Table,
    sounds: &pcx_slq::Table,
    event: &str,
    scale: VolumeScale,
) -> Result<(String, f32), PanelError> {
    checked_table(lookups, ANIM_LOOKUPS, &["AnimSoundEvent", "SoundLabel"])?;
    checked_table(
        sounds,
        ANIM_SOUNDS,
        &["SoundName", "DirectoryBase", "FileNames", "Volume"],
    )?;
    let fail = |reason: String| PanelError::Sound {
        event: event.to_string(),
        reason,
    };
    if !event
        .get(..SOUND_EVENT.len())
        .is_some_and(|p| p.eq_ignore_ascii_case(SOUND_EVENT))
    {
        return Err(fail("not an SNDX name".into()));
    }
    let code = event
        .get(SOUND_EVENT.len()..)
        .filter(|code| code.len() == SOUND_CODE_LEN)
        .ok_or_else(|| fail("not an SNDX name".into()))?;
    let mut matching = lookups
        .data_rows()
        .filter(|row| row.by_name("AnimSoundEvent") == Some(code));
    let lookup = matching
        .next()
        .ok_or_else(|| fail(format!("{ANIM_LOOKUPS} has no {code:?}")))?;
    if matching.next().is_some() {
        return Err(fail(format!("{ANIM_LOOKUPS} has ambiguous {code:?}")));
    }
    let label = lookup
        .by_name("SoundLabel")
        .filter(|s| !s.is_empty())
        .ok_or_else(|| fail("missing SoundLabel".into()))?;
    let mut matching = sounds
        .data_rows()
        .filter(|row| row.by_name("SoundName") == Some(label));
    let row = matching
        .next()
        .ok_or_else(|| fail(format!("{ANIM_SOUNDS} has no {label:?}")))?;
    if matching.next().is_some() {
        return Err(fail(format!("{ANIM_SOUNDS} has ambiguous {label:?}")));
    }
    let files = row
        .by_name("FileNames")
        .ok_or_else(|| fail(format!("{label:?} has no FileNames")))?;
    // shortcut: comma-separated sound lists refuse until a source-backed
    // file-selection policy is implemented.
    if files.is_empty() || files.contains(',') {
        return Err(fail(format!("{label:?} lists several files: {files}")));
    }
    let directory = row
        .by_name("DirectoryBase")
        .ok_or_else(|| fail(format!("{label:?} has no DirectoryBase")))?;
    if !scale.maximum.is_finite() || scale.maximum <= 0.0 {
        return Err(fail("invalid checked sound range".into()));
    }
    let volume = row
        .by_name("Volume")
        .and_then(|v| v.parse::<f32>().ok())
        .filter(|v| (0.0..=scale.maximum).contains(v))
        .ok_or_else(|| {
            fail(format!(
                "{label:?} has no Volume in the selected script range"
            ))
        })?;
    if directory
        .len()
        .checked_add(files.len())
        .is_none_or(|n| n > 4096)
        || files.contains('\0')
        || directory.contains('\0')
    {
        return Err(fail("sound member metadata bound or embedded NUL".into()));
    }
    Ok((format!("{directory}{files}"), volume / scale.maximum))
}

#[derive(Debug, Clone, Copy)]
pub enum SoundScope {
    Startup,
    MainSingle,
    MainOptions,
    Category,
}

impl SoundScope {
    fn phases(self) -> &'static [Phase] {
        match self {
            Self::Startup => &Phase::ALL[..2],
            Self::MainSingle => &Phase::ALL,
            Self::MainOptions => &[
                Phase::MainBirth,
                Phase::MainStand,
                Phase::MainDeath,
                Phase::OPTIONS[0],
                Phase::OPTIONS[1],
                Phase::OPTIONS[2],
            ],
            Self::Category => &[
                Phase::OptionsMorph,
                Phase::OptionsStandAlternate,
                Phase::OptionsMorphAlternate,
            ],
        }
    }
}

#[derive(Debug, Clone)]
pub struct SoundDependency {
    pub event: String,
    pub frame: u32,
    pub path: String,
    pub volume: f32,
}

fn clips(path: &str, bytes: &[u8], scope: SoundScope) -> Result<Vec<Clip>, PanelError> {
    let fail = |reason: String| PanelError::Decode {
        path: path.into(),
        reason,
    };
    if bytes.len() > MAX_MODEL_BYTES {
        return Err(fail("model exceeds project admission byte bound".into()));
    }
    let m = pcx_mdl::Mdlx::parse(bytes).map_err(|e| fail(e.to_string()))?;
    let sequences = m.sequences(bytes).map_err(|e| fail(e.to_string()))?;
    scope
        .phases()
        .iter()
        .map(|phase| {
            let (owner, animation) = phase.selector();
            let full_name = format!("{owner} {animation}");
            let mut matching = sequences.iter().filter(|sequence| {
                if animation.split_whitespace().nth(1).is_some() {
                    sequence.name.eq_ignore_ascii_case(&full_name)
                } else {
                    let tokens: Vec<_> = sequence.name.split_whitespace().collect();
                    tokens.len() == 2
                        && tokens.iter().any(|token| token.eq_ignore_ascii_case(owner))
                        && tokens
                            .iter()
                            .any(|token| token.eq_ignore_ascii_case(animation))
                }
            });
            let s = matching.next().ok_or_else(|| PanelError::NoSequence {
                path: path.into(),
                name: format!("{owner} {animation}"),
            })?;
            if matching.next().is_some() {
                return Err(fail(format!("ambiguous {owner} {animation}")));
            }
            let stand = matches!(
                phase,
                Phase::MainStand
                    | Phase::SingleStand
                    | Phase::OptionsStand
                    | Phase::OptionsStandAlternate
            );
            if !stand && !s.non_looping {
                return Err(fail(format!("{owner} {animation} is looping")));
            }
            Clip::checked([s.interval_start, s.interval_end], !s.non_looping)
                .map_err(|e| fail(format!("{}: {e}", s.name)))
        })
        .collect()
}

/// Pure candidate-source admission shared by desktop and browser hydration.
/// Results are source event occurrences, not deduplicated filenames; preserve
/// repeated events. The caller may cache the exact qualified WAV by path.
pub fn sound_dependencies(
    model_path: &str,
    bytes: &[u8],
    lookups: &pcx_slq::Table,
    sounds: &pcx_slq::Table,
    scale: VolumeScale,
    scope: SoundScope,
) -> Result<Vec<SoundDependency>, PanelError> {
    checked_table(lookups, ANIM_LOOKUPS, &["AnimSoundEvent", "SoundLabel"])?;
    checked_table(
        sounds,
        ANIM_SOUNDS,
        &["SoundName", "DirectoryBase", "FileNames", "Volume"],
    )?;
    let intervals = clips(model_path, bytes, scope)?;
    let fail = |reason: String| PanelError::Decode {
        path: model_path.into(),
        reason,
    };
    let m = pcx_mdl::Mdlx::parse(bytes).map_err(|e| fail(e.to_string()))?;
    if m.chunks
        .iter()
        .filter(|c| c.tag == *pcx_mdl::tag::EVTS)
        .count()
        > 1
    {
        return Err(fail("ambiguous EVTS chunks".into()));
    }
    let events = m
        .event_nodes(bytes, pcx_mdl::tag::EVTS)
        .map_err(|e| fail(e.to_string()))?;
    let payload = m
        .chunk(pcx_mdl::tag::EVTS)
        .map(|c| c.payload(bytes))
        .unwrap_or_default();
    if events.len() > MAX_SOUND_EVENTS {
        return Err(fail("event node count exceeds project bound".into()));
    }
    let mut at = 0usize;
    let mut total = 0usize;
    let mut out = Vec::new();
    for event in events {
        let rest = at
            .checked_add(event.header_size as usize)
            .ok_or_else(|| fail("EVTS offset overflow".into()))?;
        let end = if event.track == *b"KEVT" {
            rest.checked_add(12).and_then(|x| {
                event
                    .frames
                    .len()
                    .checked_mul(4)
                    .and_then(|n| x.checked_add(n))
            })
        } else {
            rest.checked_add(event.trailing.len())
        }
        .ok_or_else(|| fail("EVTS range overflow".into()))?;
        if end > payload.len() {
            return Err(fail("EVTS range exceeds source".into()));
        }
        total = total
            .checked_add(event.frames.len())
            .filter(|&n| n <= MAX_SOUND_EVENTS)
            .ok_or_else(|| fail("event count exceeds project admission bound".into()))?;
        if event
            .name
            .get(..SOUND_EVENT.len())
            .is_some_and(|p| p.eq_ignore_ascii_case(SOUND_EVENT))
        {
            let selected: Vec<_> = event
                .frames
                .iter()
                .copied()
                .filter(|f| {
                    intervals
                        .iter()
                        .any(|c| (c.interval[0]..=c.interval[1]).contains(f))
                })
                .collect();
            if !selected.is_empty() {
                let global = payload
                    .get(rest + 8..rest + 12)
                    .ok_or_else(|| fail("missing KEVT global sequence".into()))?;
                if event.track != *b"KEVT"
                    || u32::from_le_bytes(global.try_into().unwrap()) != pcx_mdl::NO_ID
                {
                    return Err(fail(format!(
                        "{}: unsupported global sound event",
                        event.name
                    )));
                }
                let (path, volume) = resolve_sound(lookups, sounds, &event.name, scale)?;
                out.extend(selected.into_iter().map(|frame| SoundDependency {
                    event: event.name.clone(),
                    frame,
                    path: path.clone(),
                    volume,
                }));
            }
        }
        at = end;
    }
    if at != payload.len() {
        return Err(fail("EVTS records do not tile source".into()));
    }
    Ok(out)
}

/// Qualify the original resource and a bounded decoder view before either
/// native Bevy or browser event transport can receive the waveform.
#[cfg(any(target_arch = "wasm32", test))]
pub fn validate_sound_bytes(
    path: &str,
    bytes: &[u8],
    origin: pcx_play::scripts::AssetIdentity,
) -> Result<(), PanelError> {
    checked_sound_bytes(path, bytes, origin).map(|_| ())
}

fn checked_sound_bytes(
    path: &str,
    bytes: &[u8],
    origin: pcx_play::scripts::AssetIdentity,
) -> Result<(Arc<[u8]>, Arc<pcx_play::file_sounds::Assets>), PanelError> {
    let fail = |e: &dyn std::fmt::Display| PanelError::Sound {
        event: path.into(),
        reason: e.to_string(),
    };
    let inputs = [pcx_play::file_sounds::SoundInput {
        member: path,
        asset: pcx_play::scripts::ScriptAsset { label: path, bytes },
        origin,
    }];
    let source = pcx_play::file_sounds::Assets::read(
        &inputs,
        pcx_play::file_sounds::Budget {
            resources: 1,
            resource_bytes: MAX_SOUND_BYTES,
            total_bytes: MAX_SOUND_BYTES,
            metadata_bytes: 16 * 1024,
        },
    )
    .map_err(|e| fail(&e))?;
    let view = pcx_play::pcm_decoder::DecoderWave::read(&source.resources()[0], MAX_SOUND_BYTES)
        .map_err(|e| fail(&e))?
        .into_bytes();
    Ok((view.into(), Arc::new(source)))
}

/// Unavailable archive audio stays distinct from a decoded, playable event.
/// Every original request/frame is retained; one record covers each qualified
/// normalized path within the selected source batch.
#[derive(Debug, Clone)]
pub struct UnavailableSound {
    pub path: String,
    pub mask: u8,
    pub context: String,
    pub requests: Vec<SoundDependency>,
}

struct LoadedSounds {
    sounds: Vec<EventSound>,
    unavailable: Vec<UnavailableSound>,
}

fn bounded_context(text: &str) -> &str {
    let mut end = text.len().min(MAX_SOUND_METADATA_BYTES);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

fn unavailable_codec(error: &PanelError) -> Option<(u8, String)> {
    match error {
        #[cfg(any(not(target_arch = "wasm32"), test))]
        PanelError::NativeRead {
            archive,
            source:
                pcx_mpq::Error::Payload(pcx_mpq::payload::PayloadError::UnsupportedCompression { mask }),
            ..
        } if matches!(mask, 0x01 | 0x41 | 0x81) => Some((
            *mask,
            format!("selected archive {}", bounded_context(archive)),
        )),
        #[cfg(any(target_arch = "wasm32", test))]
        PanelError::BrowserRead { source, .. } => {
            let mask = peacecraft3::browser_read::panel_codec_mask(source)?;
            let peacecraft3::browser_read::ReadError::BoundedExtraction {
                revision, origin, ..
            } = source.as_ref()
            else {
                return None;
            };
            Some((
                mask,
                format!(
                    "browser revision {revision}, archive {} {:?}",
                    origin.ordinal,
                    bounded_context(&origin.name)
                ),
            ))
        }
        _ => None,
    }
}

fn loaded_sounds_with(
    requests: Vec<SoundDependency>,
    origin: pcx_play::scripts::AssetIdentity,
    mut read_sound: impl FnMut(&str, usize) -> Result<Vec<u8>, PanelError>,
) -> Result<LoadedSounds, PanelError> {
    if requests.len() > MAX_SOUND_EVENTS
        || requests.iter().any(|r| {
            r.path.is_empty()
                || r.path.len() > MAX_SOUND_METADATA_BYTES
                || r.event.len() > MAX_SOUND_METADATA_BYTES
                || r.path.contains('\0')
                || !r.volume.is_finite()
                || !(0.0..=1.0).contains(&r.volume)
        })
    {
        return Err(PanelError::Sound {
            event: "panel sound requests".into(),
            reason: "request metadata, count or gain bound exceeded".into(),
        });
    }
    let mut cache =
        std::collections::BTreeMap::<String, (Arc<[u8]>, Arc<pcx_play::file_sounds::Assets>)>::new(
        );
    let mut gaps = std::collections::BTreeMap::<String, usize>::new();
    let mut out = LoadedSounds {
        sounds: Vec::new(),
        unavailable: Vec::new(),
    };
    let mut total = 0usize;
    for r in requests {
        let key = r.path.replace('\\', "/").to_ascii_lowercase();
        if let Some(&index) = gaps.get(&key) {
            out.unavailable[index].requests.push(r);
            continue;
        }
        let (bytes, source) = if let Some(sound) = cache.get(&r.path) {
            sound.clone()
        } else {
            let limit = MAX_SOUND_BYTES.min(MAX_TOTAL_SOUND_BYTES - total);
            let bytes = match read_sound(&r.path, limit) {
                Ok(bytes) => bytes,
                Err(error) => {
                    if let Some((mask, context)) = unavailable_codec(&error) {
                        gaps.insert(key, out.unavailable.len());
                        out.unavailable.push(UnavailableSound {
                            path: r.path.clone(),
                            mask,
                            context,
                            requests: vec![r],
                        });
                        continue;
                    }
                    return Err(error);
                }
            };
            if bytes.len() > limit {
                return Err(PanelError::Sound {
                    event: r.event,
                    reason: "sound byte or aggregate bound exceeded".into(),
                });
            }
            total += bytes.len();
            let sound = checked_sound_bytes(&r.path, &bytes, origin.clone())?;
            cache.insert(r.path.clone(), sound.clone());
            sound
        };
        out.sounds.push(EventSound {
            frame: r.frame,
            path: r.path,
            volume: r.volume,
            bytes,
            source,
        });
    }
    Ok(out)
}

fn loaded_sounds(
    install: Option<&std::path::Path>,
    edition: u32,
    requests: Vec<SoundDependency>,
) -> Result<LoadedSounds, PanelError> {
    let origin = pcx_play::scripts::AssetIdentity {
        edition: format!("selected skin V{edition}"),
        version: "archive release unestablished".into(),
        source: install.map_or_else(
            || "selected committed browser asset batch".into(),
            |p| {
                format!(
                    "selected install {} (member override provenance not inferred)",
                    p.display()
                )
            },
        ),
    };
    loaded_sounds_with(requests, origin, |path, limit| read(install, path, limit))
}

/// Reads startup dependencies atomically. Navigation admission is retained as a
/// contextual result so a missing optional owner cannot break the main entrance.
pub fn load(
    install: Option<&std::path::Path>,
    edition: u32,
) -> Result<Vec<PanelSource>, PanelError> {
    let skins_text = read(install, crate::mainmenu::SKIN_TABLE, MAX_MODEL_BYTES)?;
    let skins =
        crate::menu_skin::SkinTable::parse(std::str::from_utf8(&skins_text).map_err(|e| {
            PanelError::Decode {
                path: crate::mainmenu::SKIN_TABLE.into(),
                reason: e.to_string(),
            }
        })?)
        .map_err(PanelError::Skin)?;
    let script = read(install, BLIZZARD_SCRIPT, MAX_MODEL_BYTES)?;
    let scale = VolumeScale::checked(&script).map_err(|reason| PanelError::Decode {
        path: BLIZZARD_SCRIPT.into(),
        reason,
    })?;
    info!(member=BLIZZARD_SCRIPT,helper=?scale.helper_span,operand=?scale.operand_span,conversion=?scale.conversion_span,"Source menu sound range admitted; linear gain is recreation convention");
    let (lookups, sounds) = (table(install, ANIM_LOOKUPS)?, table(install, ANIM_SOUNDS)?);
    model_paths(&skins, edition)
        .map_err(PanelError::Skin)?
        .into_iter()
        .zip(LAYERS)
        .map(|(path, layer)| {
            let bytes = read(install, &path, MAX_MODEL_BYTES)?;
            let startup = clips(&path, &bytes, SoundScope::Startup)?;
            let startup_sounds = loaded_sounds(
                install,
                edition,
                sound_dependencies(&path, &bytes, &lookups, &sounds, scale, SoundScope::Startup)?,
            )?;
            let admit = |scope| -> Result<_, PanelError> {
                Ok((
                    clips(&path, &bytes, scope)?,
                    loaded_sounds(
                        install,
                        edition,
                        sound_dependencies(&path, &bytes, &lookups, &sounds, scale, scope)?,
                    )?,
                ))
            };
            let navigation = admit(SoundScope::MainSingle).map_err(|e| e.to_string());
            let options = admit(SoundScope::MainOptions).map_err(|e| e.to_string());
            let category = admit(SoundScope::Category).map_err(|e| e.to_string());
            Ok(PanelSource {
                layer,
                path,
                bytes,
                install: install.map(std::path::Path::to_path_buf),
                edition,
                clips: startup,
                sounds: startup_sounds.sounds,
                unavailable_sounds: startup_sounds.unavailable,
                navigation,
                options,
                category,
            })
        })
        .collect()
}

/// Pure source admission for candidate browser imports and runtime panel batches.
#[cfg(any(target_arch = "wasm32", test))]
pub fn preflight_source(
    model_path: &str,
    bytes: &[u8],
    textures: &[pcx_gfx::LoadedTexture],
    scope: SoundScope,
) -> Result<(), PanelError> {
    let fail = |reason: String| PanelError::Decode {
        path: model_path.into(),
        reason,
    };
    let clips = clips(model_path, bytes, scope)?;
    let m = pcx_mdl::Mdlx::parse(bytes).map_err(|e| fail(e.to_string()))?;
    let model = pcx_gfx::load_model(bytes, model_path).map_err(|e| fail(e.to_string()))?;
    let runtime =
        pcx_gfx::mdx_runtime::RuntimePlan::parse(bytes).map_err(|e| fail(e.to_string()))?;
    let plans = clips
        .into_iter()
        .map(|c| ClipPlan::checked(&runtime, c, model_path))
        .collect::<Result<Vec<_>, _>>()?;
    preflight_plans(model_path, bytes, &m, &model, &runtime, textures, &plans)
}

fn preflight_plans(
    path: &str,
    bytes: &[u8],
    m: &pcx_mdl::Mdlx,
    model: &pcx_gfx::LoadedModel,
    runtime: &pcx_gfx::mdx_runtime::RuntimePlan,
    textures: &[pcx_gfx::LoadedTexture],
    plans: &[ClipPlan],
) -> Result<(), PanelError> {
    if model.meshes.is_empty()
        || !model
            .minimum_extent
            .iter()
            .chain(&model.maximum_extent)
            .all(|x| x.is_finite())
        || model.minimum_extent[2] > model.maximum_extent[2]
    {
        return Err(PanelError::Decode {
            path: path.into(),
            reason: "empty geometry or malformed source camera extents".into(),
        });
    }
    for plan in plans {
        scene::preflight_geosets(
            &scene::GeosetSource {
                m,
                bytes,
                source: path,
                model,
                textures,
                runtime,
                selection: &plan.textures,
                uv_translation: &plan.uv,
                native_cutout_reference: None,
                material_layers: None,
                culls_back_faces: false,
            },
            &plan.opacity,
            plan.clip.interval[0],
        )?;
    }
    Ok(())
}

/// One spawned panel.
struct ClipPlan {
    clip: Clip,
    opacity: pcx_gfx::mdx_opacity::OpacityPlan,
    textures: pcx_gfx::mdx_texture_selection::TextureSelectionPlan,
    uv: pcx_gfx::mdx_uv_translation::UvTranslationPlan,
}

impl ClipPlan {
    fn checked(
        runtime: &pcx_gfx::mdx_runtime::RuntimePlan,
        clip: Clip,
        path: &str,
    ) -> Result<Self, PanelError> {
        let opacity =
            pcx_gfx::mdx_opacity::OpacityPlan::checked(runtime, clip.interval).map_err(|e| {
                PanelError::Decode {
                    path: path.into(),
                    reason: e.to_string(),
                }
            })?;
        let textures = scene::texture_selection(runtime, clip.interval, path)?;
        let uv = pcx_gfx::mdx_uv_translation::UvTranslationPlan::from_runtime(
            runtime,
            clip.interval,
            path,
        )
        .map_err(scene::SceneError::from)?;
        Ok(Self {
            clip,
            opacity,
            textures,
            uv,
        })
    }
}

struct Panel {
    layer: &'static str,
    path: String,
    model: pcx_gfx::LoadedModel,
    bones: Vec<pcx_mdl::Bone>,
    pivots: pcx_mdl::Pivots,
    global_durations: Vec<u32>,
    geosets: scene::Geosets,
    phases: Vec<ClipPlan>,
    navigation_error: Option<String>,
    options: Result<(Vec<ClipPlan>, LoadedSounds), String>,
    category: Result<(Vec<ClipPlan>, LoadedSounds), String>,
    sounds: Vec<EventSound>,
    unavailable_sounds: Vec<UnavailableSound>,
}

impl Panel {
    fn duration(&self, phase: Phase) -> Option<std::time::Duration> {
        if SoundScope::Category.phases().contains(&phase) && self.layer != LAYERS[0] {
            return None;
        }
        self.plan(phase).map(|plan| plan.clip.duration())
    }

    fn plan(&self, phase: Phase) -> Option<&ClipPlan> {
        Self::plan_for(&self.phases, &self.options, &self.category, phase)
    }
    fn plan_for<'a>(
        phases: &'a [ClipPlan],
        options: &'a Result<(Vec<ClipPlan>, LoadedSounds), String>,
        category: &'a Result<(Vec<ClipPlan>, LoadedSounds), String>,
        phase: Phase,
    ) -> Option<&'a ClipPlan> {
        if let Some(plan) = phases.get(phase as usize) {
            return Some(plan);
        }
        if let Some(index) = SoundScope::MainOptions
            .phases()
            .iter()
            .position(|candidate| *candidate == phase)
        {
            return options.as_ref().ok()?.0.get(index);
        }
        let index = SoundScope::Category
            .phases()
            .iter()
            .position(|candidate| *candidate == phase)?;
        category.as_ref().ok()?.0.get(index)
    }

    fn sounds(&self, phase: Phase) -> &[EventSound] {
        if SoundScope::Category.phases().contains(&phase)
            && let Ok((_, sounds)) = &self.category
        {
            return &sounds.sounds;
        }
        if (phase.owner() == Owner::Options || self.navigation_error.is_some())
            && let Ok((_, sounds)) = &self.options
        {
            return &sounds.sounds;
        }
        &self.sounds
    }
}

/// Mesh/material handles and independent global clock survive navigation.
#[derive(Resource)]
pub struct MenuPanels {
    panels: Vec<Panel>,
    clock: Clock,
    destination: Option<crate::menu_screens::Screen>,
    install: Option<std::path::PathBuf>,
    edition: u32,
    category_refused: bool,
    category_stand_origin: Option<std::time::Duration>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Transition {
    Animated,
    Unavailable(String),
}

impl MenuPanels {
    pub fn request(
        &mut self,
        from: crate::menu_screens::Screen,
        to: crate::menu_screens::Screen,
        install: Option<&std::path::Path>,
        edition: u32,
    ) -> Result<Transition, String> {
        if self.install.as_deref() != install || self.edition != edition {
            return Err("panel source selection changed; scene replacement required".into());
        }
        if self.panels.is_empty() {
            return Ok(Transition::Unavailable("no admitted menu panels".into()));
        }
        let owner = |s| match s {
            crate::menu_screens::Screen::Main => Ok(Owner::Main),
            crate::menu_screens::Screen::SinglePlayer => Ok(Owner::Single),
            crate::menu_screens::Screen::Options(_) => Ok(Owner::Options),
            _ => Err("unsupported panel owner"),
        };
        let (from, target) = (owner(from)?, owner(to)?);
        for p in &self.panels {
            let error = if from == Owner::Options || target == Owner::Options {
                p.options.as_ref().err()
            } else {
                p.navigation_error.as_ref()
            };
            if let Some(error) = error {
                return Ok(Transition::Unavailable(format!("{}: {error}", p.path)));
            }
            for phase in from.phases().into_iter().chain(target.phases()) {
                if p.plan(phase).is_none() {
                    return Ok(Transition::Unavailable(format!(
                        "{}: missing {:?}",
                        p.path, phase
                    )));
                }
            }
        }
        if self.clock.phase == Phase::OptionsStandAlternate {
            for p in self.panels.iter().filter(|panel| panel.layer == LAYERS[0]) {
                if p.plan(Phase::OptionsMorphAlternate).is_none() {
                    return Ok(Transition::Unavailable(format!(
                        "{}: missing {:?}",
                        p.path,
                        Phase::OptionsMorphAlternate
                    )));
                }
            }
        }
        self.clock.begin(from, target).map_err(str::to_string)?;
        self.destination = Some(to);
        Ok(Transition::Animated)
    }

    /// Starts the category plate when a Gameplay, Video, or Sound page is
    /// already displayed and the owner clock is on Options Stand.
    pub fn sync_category_plate(
        &mut self,
        screen: crate::menu_screens::Screen,
    ) -> Result<(), String> {
        let crate::menu_screens::Screen::Options(
            crate::menu_screens::Page::Video
            | crate::menu_screens::Page::Sound
            | crate::menu_screens::Page::Gameplay,
        ) = screen
        else {
            return Ok(());
        };
        if self.category_refused
            || self.panels.is_empty()
            || self.clock.target.is_some()
            || self.clock.phase != Phase::OptionsStand
        {
            return Ok(());
        }
        if !self.panels.iter().any(|panel| panel.layer == LAYERS[0]) {
            self.category_refused = true;
            return Err("the selected skin has no left category panel".into());
        }
        for panel in self.panels.iter().filter(|panel| panel.layer == LAYERS[0]) {
            if panel.category.is_err()
                || panel.plan(Phase::OptionsMorph).is_none()
                || panel.plan(Phase::OptionsStandAlternate).is_none()
            {
                self.category_refused = true;
                return Err(match panel.category.as_ref() {
                    Err(error) => format!("{}: {error}", panel.path),
                    Ok(_) => format!("{}: missing category panel phase", panel.path),
                });
            }
        }
        let origin = self
            .clock
            .global
            .checked_sub(self.clock.local)
            .ok_or("Options Stand local clock exceeds the global clock")?;
        self.clock.begin_category().map_err(str::to_string)?;
        self.category_stand_origin = Some(origin);
        Ok(())
    }

    pub fn reset_to_main(&mut self) {
        if matches!(self.clock.phase, Phase::MainBirth | Phase::MainStand)
            && self.clock.target.is_none()
        {
            return;
        }
        let elapsed = self
            .panels
            .iter()
            .map(|p| p.phases[Phase::MainStand as usize].clip.duration())
            .max()
            .unwrap_or_default();
        self.clock.settle_main(elapsed);
        self.destination = None;
        self.category_stand_origin = None;
    }
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
    struct Prepared {
        layer: &'static str,
        path: String,
        bytes: Vec<u8>,
        m: pcx_mdl::Mdlx,
        model: pcx_gfx::LoadedModel,
        runtime: pcx_gfx::mdx_runtime::RuntimePlan,
        textures: Vec<pcx_gfx::LoadedTexture>,
        plans: Vec<ClipPlan>,
        sounds: Vec<EventSound>,
        unavailable_sounds: Vec<UnavailableSound>,
        navigation_error: Option<String>,
        options: Result<(Vec<ClipPlan>, LoadedSounds), String>,
        category: Result<(Vec<ClipPlan>, LoadedSounds), String>,
    }
    let selection = sources
        .first()
        .map(|s| (s.install.clone(), s.edition))
        .ok_or_else(|| PanelError::Decode {
            path: "menu panels".into(),
            reason: "empty panel batch".into(),
        })?;
    if sources
        .iter()
        .any(|s| s.install != selection.0 || s.edition != selection.1)
    {
        return Err(PanelError::Decode {
            path: "menu panels".into(),
            reason: "mixed source selection in panel batch".into(),
        });
    }
    let native_cutout_reference = match crate::menu_lighting::parameters(install) {
        Ok(parameters) => Some(parameters.cutout_reference),
        Err(error) => {
            warn!(%error, "Native panel cutout profile unavailable; retaining unbound material convention");
            None
        }
    };
    let mut prepared = Vec::with_capacity(sources.len());
    for source in sources {
        let PanelSource {
            layer,
            path,
            bytes,
            clips,
            sounds,
            unavailable_sounds,
            navigation,
            options,
            category,
            install: _,
            edition: _,
        } = source;
        let fail = |reason: String| PanelError::Decode {
            path: path.clone(),
            reason,
        };
        let m = pcx_mdl::Mdlx::parse(&bytes).map_err(|e| fail(e.to_string()))?;
        let model = pcx_gfx::load_model(&bytes, &path).map_err(|e| fail(e.to_string()))?;
        let runtime =
            pcx_gfx::mdx_runtime::RuntimePlan::parse(&bytes).map_err(|e| fail(e.to_string()))?;
        let mut plans: Vec<_> = clips
            .into_iter()
            .map(|c| ClipPlan::checked(&runtime, c, &path))
            .collect::<Result<_, _>>()?;
        let mut admitted_sounds = sounds;
        let mut admitted_gaps = unavailable_sounds;
        let navigation_error = match navigation.and_then(|(clips, sounds)| {
            let extra = clips
                .into_iter()
                .skip(2)
                .map(|c| ClipPlan::checked(&runtime, c, &path))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
            Ok((extra, sounds))
        }) {
            Ok((extra, sounds)) => {
                plans.extend(extra);
                admitted_sounds = sounds.sounds;
                admitted_gaps = sounds.unavailable;
                None
            }
            Err(e) => Some(e),
        };
        // Missing/malformed images are admission failures, never silently omitted.
        let textures = scene::texture_paths(&bytes)?
            .into_iter()
            .map(|p| {
                pcx_gfx::load_image(&read(install, &p, scene::MENU_IMAGE_BYTES)?, &p)
                    .map_err(|e| fail(e.to_string()))
            })
            .collect::<Result<Vec<_>, PanelError>>()?;
        preflight_plans(&path, &bytes, &m, &model, &runtime, &textures, &plans[..2])?;
        let mut navigation_error = navigation_error;
        if plans.len() > 2
            && let Err(error) =
                preflight_plans(&path, &bytes, &m, &model, &runtime, &textures, &plans[2..])
        {
            navigation_error = Some(error.to_string());
            plans.truncate(2);
        }
        let options = options.and_then(|(clips, sounds)| {
            let plans = clips
                .into_iter()
                .map(|clip| ClipPlan::checked(&runtime, clip, &path))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| error.to_string())?;
            preflight_plans(&path, &bytes, &m, &model, &runtime, &textures, &plans)
                .map_err(|error| error.to_string())?;
            Ok((plans, sounds))
        });
        let category = category.and_then(|(clips, sounds)| {
            let plans = clips
                .into_iter()
                .map(|clip| ClipPlan::checked(&runtime, clip, &path))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| error.to_string())?;
            preflight_plans(&path, &bytes, &m, &model, &runtime, &textures, &plans)
                .map_err(|error| error.to_string())?;
            Ok((plans, sounds))
        });
        prepared.push(Prepared {
            layer,
            path,
            bytes,
            m,
            model,
            runtime,
            textures,
            plans,
            sounds: admitted_sounds,
            unavailable_sounds: admitted_gaps,
            navigation_error,
            options,
            category,
        });
    }
    if prepared.is_empty() {
        return Err(PanelError::Decode {
            path: "menu panels".into(),
            reason: "empty panel batch".into(),
        });
    }
    let mut panels = Vec::with_capacity(prepared.len());
    let (mut low, mut high) = (f32::MAX, f32::MIN);
    for p in prepared {
        let plan = &p.plans[Phase::MainBirth as usize];
        let mut info = scene::SceneInfo::default();
        let geosets = scene::spawn_geosets(
            commands,
            scene::GeosetAssets {
                meshes,
                materials,
                images,
            },
            scene::GeosetSource {
                m: &p.m,
                bytes: &p.bytes,
                source: &p.path,
                model: &p.model,
                textures: &p.textures,
                runtime: &p.runtime,
                selection: &plan.textures,
                uv_translation: &plan.uv,
                native_cutout_reference,
                material_layers: None,
                culls_back_faces: false,
            },
            &plan.opacity,
            plan.clip.interval[0],
            &mut info,
            (
                scene::MainMenuSceneEntity,
                RenderLayers::layer(RENDER_LAYER),
                NoFrustumCulling,
            ),
        )?;
        low = low.min(p.model.minimum_extent[2]);
        high = high.max(p.model.maximum_extent[2]);
        panels.push(Panel {
            layer: p.layer,
            path: p.path,
            model: p.model,
            bones: p.runtime.joints.joints().to_vec(),
            pivots: p.runtime.joints.pivots().clone(),
            global_durations: p.runtime.global_sequences,
            geosets,
            phases: p.plans,
            navigation_error: p.navigation_error,
            options: p.options,
            category: p.category,
            sounds: p.sounds,
            unavailable_sounds: p.unavailable_sounds,
        });
    }
    // Every sibling's full source preflight completed before publishing gaps.
    let mut reported = std::collections::BTreeSet::new();
    for panel in &panels {
        for gap in panel
            .unavailable_sounds
            .iter()
            .chain(
                panel
                    .options
                    .as_ref()
                    .ok()
                    .into_iter()
                    .flat_map(|(_, sounds)| &sounds.unavailable),
            )
            .chain(
                panel
                    .category
                    .as_ref()
                    .ok()
                    .into_iter()
                    .flat_map(|(_, sounds)| &sounds.unavailable),
            )
        {
            let key = (
                gap.context.as_str(),
                gap.path.replace('\\', "/").to_ascii_lowercase(),
                gap.mask,
            );
            if !reported.insert(key) {
                continue;
            }
            warn!(model=%panel.path, sound=%gap.path, mask=gap.mask, source=%gap.context,
                occurrences=gap.requests.len(), "Original menu panel audio unavailable; admitted geometry retained");
        }
    }
    // The default sprite projection adds model X/Y to the frame's left/bottom
    // (`docs/re-notes/glue-coordinate-runtime.md`), so the camera centres the
    // authored frame. Looking down -Z, depth runs from -maxZ to -minZ of the
    // models' own extents; transparent geosets sort by that depth.
    commands.spawn((
        Camera3d::default(),
        crate::menu_composition::MenuColourCamera,
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
        clock: Clock::default(),
        destination: None,
        install: selection.0,
        edition: selection.1,
        category_refused: false,
        category_stand_origin: None,
    });
    Ok(count)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = peacecraft3Audio, js_name = event)]
    fn browser_event_sound(bytes: &[u8], volume: f32);
    #[wasm_bindgen(catch, js_namespace = peacecraft3Audio, js_name = stopEvents)]
    fn browser_stop_events() -> Result<(), wasm_bindgen::JsValue>;
}

/// Scene replacement cancels already dispatched browser event voices. The
/// API-only worker has no audio namespace and performs no browser side effect.
pub fn cancel_event_sounds() {
    #[cfg(target_arch = "wasm32")]
    if web_sys::window().is_some()
        && let Err(error) = browser_stop_events()
    {
        warn!("Menu event sound cancellation unavailable: {error:?}");
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct SoundSink<'w> {
    assets: ResMut<'w, Assets<AudioSource>>,
    settings: Res<'w, crate::menu_settings::Backend>,
    launcher: Res<'w, crate::launch_gui::Launcher>,
}
#[cfg(target_arch = "wasm32")]
type SoundSink<'w> = ();

#[cfg(not(target_arch = "wasm32"))]
fn play(commands: &mut Commands, sink: &mut SoundSink, sound: &EventSound) {
    if !sink.settings.source_matches(&sink.launcher) {
        return;
    }
    debug_assert_eq!(sound.source.resources().len(), 1);
    debug_assert_eq!(sound.source.resources()[0].member(), sound.path);
    commands.spawn((
        AudioPlayer(sink.assets.add(AudioSource {
            bytes: sound.bytes.clone(),
        })),
        PlaybackSettings::DESPAWN.with_volume(bevy::audio::Volume::Linear(
            sound.volume * sink.settings.audio.effective_effects(),
        )),
        crate::menu_audio::AuthoredVolume(sound.volume),
        scene::MainMenuSceneEntity,
    ));
}

#[cfg(target_arch = "wasm32")]
fn play(_: &mut Commands, _: &mut SoundSink, sound: &EventSound) {
    debug_assert_eq!(sound.source.resources().len(), 1);
    debug_assert_eq!(sound.source.resources()[0].member(), sound.path);
    browser_event_sound(&sound.bytes, sound.volume);
}

fn layer_clock(
    layer: &str,
    phase: Phase,
    local: std::time::Duration,
    global: std::time::Duration,
    stand_origin: Option<std::time::Duration>,
) -> Result<(Phase, std::time::Duration), &'static str> {
    if layer == LAYERS[1] && SoundScope::Category.phases().contains(&phase) {
        let origin = stand_origin.ok_or("category transition has no Options Stand origin")?;
        return global
            .checked_sub(origin)
            .map(|local| (Phase::OptionsStand, local))
            .ok_or("Options Stand origin exceeds the global clock");
    }
    Ok((phase, local))
}

#[derive(bevy::ecs::system::SystemParam)]
pub struct PanelAdvanceAssets<'w, 's> {
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<MenuMaterial>>,
    visibility: Query<'w, 's, &'static mut Visibility, With<scene::MainMenuSceneEntity>>,
}

/// Advances the entrance clock, fires the events it passed, and poses each
/// panel at its frame. Presentation time is uncapped by the simulation clock.
pub fn advance(
    time: Res<Time<Real>>,
    mut commands: Commands,
    mut state: ResMut<MenuPanels>,
    mut navigation: ResMut<crate::menu_screens::Navigation>,
    assets: PanelAdvanceAssets,
    mut sink: SoundSink,
) {
    let PanelAdvanceAssets {
        mut meshes,
        mut materials,
        mut visibility,
    } = assets;
    let mut next = state.clock.clone();
    let result = next.advance(time.delta(), |phase| {
        state
            .panels
            .iter()
            .filter_map(|p| p.duration(phase))
            .max()
            .unwrap_or_default()
    });
    let (segments, entered) = match result {
        Ok(r) => r,
        Err(e) => {
            navigation.notice = e.into();
            return;
        }
    };
    let group_birth = |phase: Phase| {
        phase
            .birth_and_stand()
            .map(|(birth, _)| {
                state
                    .panels
                    .iter()
                    .filter_map(|p| p.duration(birth))
                    .max()
                    .unwrap_or_default()
            })
            .unwrap_or_default()
    };
    let mut events = Vec::new();
    let mut segment_global = state.clock.global;
    for (segment_id, segment) in segments.iter().enumerate() {
        let delta = segment.to - segment.from;
        for (panel_id, panel) in state.panels.iter().enumerate() {
            let (phase, from) = match layer_clock(
                panel.layer,
                segment.phase,
                segment.from,
                segment_global,
                state.category_stand_origin,
            ) {
                Ok(clock) => clock,
                Err(error) => {
                    navigation.notice = error.into();
                    return;
                }
            };
            let mapped = phases::Segment {
                phase,
                from,
                to: from + delta,
                first: segment.first && phase == segment.phase,
            };
            let bias = segment.from.as_nanos() as i128 - from.as_nanos() as i128;
            let birth = phase
                .birth_and_stand()
                .and_then(|(birth, _)| panel.plan(birth).map(|plan| plan.clip.duration()))
                .unwrap_or_default();
            for window in phases::layer_windows(&mapped, birth, group_birth(phase)) {
                let Some(plan) = panel.plan(window.phase) else {
                    navigation.notice = format!(
                        "{}: missing admitted panel phase {:?}",
                        panel.path, window.phase
                    );
                    return;
                };
                let clip = plan.clip;
                for (sound_id, sound) in panel.sounds(window.phase).iter().enumerate() {
                    match clip.event_offsets(
                        sound.frame,
                        window.from,
                        window.to,
                        window.first,
                        MAX_SOUND_EVENTS - events.len(),
                    ) {
                        Ok(offsets) => events.extend(offsets.into_iter().map(|offset| {
                            (
                                segment_id,
                                offset.as_nanos() as i128 + window.event_bias_nanos + bias,
                                panel_id,
                                sound_id,
                                window.phase,
                            )
                        })),
                        Err(error) => {
                            navigation.notice = error.into();
                            return;
                        }
                    }
                }
            }
        }
        segment_global += delta;
    }
    let group_entry = group_birth(next.phase);
    let options_entry = group_birth(Phase::OptionsStand);
    let stand_origin = state.category_stand_origin;
    let elapsed_ms = u64::try_from(next.global.as_millis()).expect("clock preview checked");
    for panel in &mut state.panels {
        let (phase, local) = match layer_clock(
            panel.layer,
            next.phase,
            next.local,
            next.global,
            stand_origin,
        ) {
            Ok(clock) => clock,
            Err(error) => {
                navigation.notice = error.into();
                return;
            }
        };
        let birth = phase
            .birth_and_stand()
            .and_then(|(birth, _)| panel.plan(birth).map(|plan| plan.clip.duration()))
            .unwrap_or_default();
        let entry = if phase == next.phase {
            group_entry
        } else {
            options_entry
        };
        let (phase, local) = phases::layer_sample(phase, local, birth, entry);
        let Some(plan) = Panel::plan_for(&panel.phases, &panel.options, &panel.category, phase)
        else {
            navigation.notice = format!("{}: missing admitted panel phase {:?}", panel.path, phase);
            return;
        };
        let frame = plan.clip.frame(local);
        if let Err(error) = scene::apply_uv_translation(
            &plan.uv,
            &panel.geosets.textures,
            frame,
            elapsed_ms,
            &mut materials,
        )
        .and_then(|()| {
            scene::apply_texture_selection(
                &plan.textures,
                &mut panel.geosets.textures,
                frame,
                elapsed_ms,
                &mut materials,
            )
        }) {
            navigation.notice = format!("{}: {error}", panel.path);
            return;
        }
        if let Err(error) = scene::apply_geoset_colour(
            &plan.opacity,
            &panel.geosets.bound,
            frame,
            elapsed_ms,
            &mut materials,
        ) {
            navigation.notice = format!("{}: {error}", panel.path);
            return;
        }
        scene::apply_opacity(
            &plan.opacity,
            &panel.geosets.bound,
            frame,
            elapsed_ms,
            &mut materials,
            &mut visibility,
        );
        if let Err(error) = scene::skin_geosets(
            scene::Rig {
                interval: plan.clip.interval,
                model: &panel.model,
                bones: &panel.bones,
                pivots: &panel.pivots,
                global_durations: &panel.global_durations,
                camera_pose: None,
            },
            &panel.geosets.handles,
            frame,
            elapsed_ms as u32,
            &mut meshes,
        ) {
            navigation.notice = format!("{}: {error}", panel.path);
            return;
        }
    }
    events.sort_by_key(|&(segment, offset, _, _, _)| (segment, offset));
    for (_, _, panel, sound, phase) in events {
        play(
            &mut commands,
            &mut sink,
            &state.panels[panel].sounds(phase)[sound],
        );
    }
    if let Some(owner) = entered {
        navigation.commit_panel_owner(match owner {
            Owner::Main => crate::menu_screens::Screen::Main,
            Owner::Single => crate::menu_screens::Screen::SinglePlayer,
            Owner::Options => match state.destination {
                Some(screen @ crate::menu_screens::Screen::Options(_)) => screen,
                _ => {
                    navigation.notice = "Missing admitted Options destination".into();
                    return;
                }
            },
        });
    }
    navigation.transition_active = next.target.is_some()
        || matches!(
            next.phase,
            Phase::OptionsMorph | Phase::OptionsMorphAlternate
        );
    if next.phase.owner() != Owner::Options {
        state.category_stand_origin = None;
    }
    state.clock = next;
}

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
        let active = !menu.is_empty() && ui.is_active;
        if camera.is_active != active {
            camera.is_active = active;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    include!("menu_panel_audio_gap_tests.rs");

    #[test]
    fn category_clock_changes_only_the_left_skin_layer_and_keeps_right_stand_time() {
        use std::time::Duration;
        let ms = Duration::from_millis;
        let origin = Some(ms(5));
        let right_stand = Clip {
            interval: [300, 307],
            loops: true,
        };
        for (phase, local, global, expected) in [
            (Phase::OptionsMorph, 2, 9, 4),
            (Phase::OptionsStandAlternate, 1, 12, 7),
            (Phase::OptionsMorphAlternate, 3, 21, 16),
        ] {
            assert_eq!(
                layer_clock(LAYERS[0], phase, ms(local), ms(global), origin),
                Ok((phase, ms(local)))
            );
            let (mapped, elapsed) =
                layer_clock(LAYERS[1], phase, ms(local), ms(global), origin).unwrap();
            assert_eq!(mapped, Phase::OptionsStand);
            assert_eq!(elapsed, ms(expected));
            assert_eq!(right_stand.frame(elapsed), 300 + expected as u32 % 7);
        }
        assert_eq!(
            layer_clock(LAYERS[1], Phase::OptionsDeath, ms(3), ms(22), origin),
            Ok((Phase::OptionsDeath, ms(3)))
        );
        assert!(layer_clock(LAYERS[1], Phase::OptionsMorph, ms(0), ms(3), origin).is_err());
        assert!(layer_clock(LAYERS[1], Phase::OptionsMorph, ms(0), ms(3), None).is_err());
    }

    #[test]
    fn unavailable_panels_allow_navigation_but_stale_selection_refuses() {
        use crate::menu_screens::Screen;
        let mut panels = MenuPanels {
            panels: Vec::new(),
            clock: Clock::default(),
            destination: None,
            install: None,
            edition: 0,
            category_refused: false,
            category_stand_origin: None,
        };
        assert!(matches!(
            panels.request(Screen::Main, Screen::SinglePlayer, None, 0),
            Ok(Transition::Unavailable(_))
        ));
        assert!(panels.clock.target.is_none());
        assert!(
            panels
                .request(Screen::Main, Screen::SinglePlayer, None, 1)
                .is_err()
        );
        assert!(panels.clock.target.is_none());
    }

    #[cfg(not(target_arch = "wasm32"))]
    mod category_pop_slice {
        use super::super::*;
        use super::{origin, request, wave};
        use bevy::ecs::system::RunSystemOnce;
        use bevy::prelude::{Assets, AudioPlayer, AudioSource, Mesh, Time, World};
        use std::time::Duration;

        fn category_app() -> App {
            let runtime =
                pcx_gfx::mdx_runtime::RuntimePlan::checked(pcx_gfx::mdx_runtime::RuntimeInput {
                    bones: Vec::new(),
                    helpers: Vec::new(),
                    pivots: pcx_mdl::Pivots(Vec::new()),
                    geosets: Vec::new(),
                    geoset_animations: Vec::new(),
                    materials: Vec::new(),
                    textures: Vec::new(),
                    texture_animations: Vec::new(),
                    global_sequences: Vec::new(),
                })
                .unwrap();
            // Slots before Options Birth are index placement, not Main or Single coverage.
            let options_specs = [
                ([0, 1], false),
                ([0, 1], false),
                ([0, 1], false),
                ([0, 1], false),
                ([100, 300], true),
                ([400, 440], false),
            ];
            let category_specs = [([0, 40], false), ([0, 40], false), ([200, 240], false)];
            let category_rows = [(0, 0.2), (132, 0.6), (200, 0.7), (172, 0.9), (400, 0.15)];
            let options_rows = [(100, 0.3), (132, 0.5), (250, 0.35), (172, 0.8), (400, 0.4)];
            let plans = |specs: &[([u32; 2], bool)]| -> Vec<ClipPlan> {
                specs
                    .iter()
                    .map(|&(interval, loops)| {
                        ClipPlan::checked(
                            &runtime,
                            Clip::checked(interval, loops).unwrap(),
                            "synthetic",
                        )
                        .unwrap()
                    })
                    .collect()
            };
            let sounds = |rows: &[(u32, f32)]| {
                loaded_sounds_with(
                    rows.iter()
                        .map(|&(frame, volume)| {
                            let mut sound = request("Local/slice.wav", frame);
                            sound.volume = volume;
                            sound
                        })
                        .collect(),
                    origin(),
                    |_, _| Ok(wave()),
                )
                .unwrap()
            };
            let mut clock = Clock::default();
            clock.phase = Phase::OptionsStand;
            clock.local = Duration::from_millis(32);
            clock.global = Duration::from_millis(40);
            let mut panels = MenuPanels {
                panels: LAYERS
                    .into_iter()
                    .map(|layer| Panel {
                        layer,
                        path: format!("synthetic/{layer}"),
                        model: pcx_gfx::LoadedModel {
                            name: String::new(),
                            minimum_extent: [0.0; 3],
                            maximum_extent: [0.0; 3],
                            meshes: Vec::new(),
                            bone_names: Vec::new(),
                            bone_parents: Vec::new(),
                            sequences: Vec::new(),
                        },
                        bones: Vec::new(),
                        pivots: pcx_mdl::Pivots(Vec::new()),
                        global_durations: Vec::new(),
                        geosets: scene::Geosets {
                            handles: Vec::new(),
                            bound: Vec::new(),
                            textures: scene::TextureBindings::default(),
                        },
                        phases: Vec::new(),
                        navigation_error: None,
                        options: Ok((plans(&options_specs), sounds(&options_rows))),
                        category: Ok((plans(&category_specs), sounds(&category_rows))),
                        sounds: Vec::new(),
                        unavailable_sounds: Vec::new(),
                    })
                    .collect(),
                clock,
                destination: None,
                install: None,
                edition: 0,
                category_refused: false,
                category_stand_origin: None,
            };
            panels
                .sync_category_plate(crate::menu_screens::Screen::Options(
                    crate::menu_screens::Page::Video,
                ))
                .unwrap();
            assert_eq!(panels.category_stand_origin, Some(Duration::from_millis(8)));
            assert_eq!(panels.clock.phase, Phase::OptionsMorph);
            assert_eq!(panels.clock.local, Duration::ZERO);
            assert_eq!(panels.clock.global, Duration::from_millis(40));

            let mut app = App::new();
            app.add_plugins(bevy::time::TimePlugin);
            let world = app.world_mut();
            world.init_resource::<Assets<Mesh>>();
            world.init_resource::<Assets<MenuMaterial>>();
            world.init_resource::<Assets<AudioSource>>();
            world.init_resource::<crate::menu_screens::Navigation>();
            let launcher =
                crate::launch_gui::Launcher::test_source("synthetic-panel-source".into(), true);
            let mut backend = crate::menu_settings::Backend::default();
            backend.set_test_source(&launcher);
            world.insert_resource(backend);
            world.insert_resource(launcher);
            world.insert_resource(panels);
            app
        }

        #[test]
        fn category_pops_stay_on_their_source_slice() {
            let mut app = category_app();
            let world = app.world_mut();
            let played = |world: &mut World| {
                let mut gains: Vec<f32> = world
                    .query::<&crate::menu_audio::AuthoredVolume>()
                    .iter(&*world)
                    .map(|volume| volume.0)
                    .collect();
                gains.sort_by(f32::total_cmp);
                assert_eq!(
                    world.query::<&AudioPlayer>().iter(&*world).count(),
                    gains.len()
                );
                let notice = world
                    .resource::<crate::menu_screens::Navigation>()
                    .notice
                    .clone();
                assert!(notice.is_empty(), "{notice}");
                gains
            };

            world
                .resource_mut::<Time<Real>>()
                .advance_by(Duration::from_millis(40));
            world.run_system_once(advance).unwrap();
            assert_eq!(played(world), vec![0.2, 0.5]);
            {
                let panels = world.resource::<MenuPanels>();
                assert_eq!(panels.clock.phase, Phase::OptionsStandAlternate);
                assert_eq!(panels.clock.local, Duration::ZERO);
                assert_eq!(panels.clock.global, Duration::from_millis(80));
                assert_eq!(panels.category_stand_origin, Some(Duration::from_millis(8)));
            }
            assert!(matches!(
                world.resource_mut::<MenuPanels>().request(
                    crate::menu_screens::Screen::Options(crate::menu_screens::Page::Video),
                    crate::menu_screens::Screen::Main,
                    None,
                    0,
                ),
                Ok(Transition::Animated)
            ));
            world
                .resource_mut::<Time<Real>>()
                .advance_by(Duration::from_millis(16));
            world.run_system_once(advance).unwrap();
            assert_eq!(played(world), vec![0.2, 0.5, 0.7, 0.8]);
            assert_eq!(
                world.resource::<MenuPanels>().clock.phase,
                Phase::OptionsMorphAlternate
            );
            assert_eq!(
                world.resource::<MenuPanels>().clock.local,
                Duration::from_millis(16)
            );
            world
                .resource_mut::<Time<Real>>()
                .advance_by(Duration::from_millis(40));
            world.run_system_once(advance).unwrap();
            assert_eq!(played(world), vec![0.2, 0.4, 0.4, 0.5, 0.7, 0.8]);
            {
                let panels = world.resource::<MenuPanels>();
                assert_eq!(panels.clock.phase, Phase::OptionsDeath);
                assert_eq!(panels.clock.local, Duration::from_millis(16));
            }
        }

        #[test]
        fn category_clock_and_sounds_ignore_frame_partition_and_virtual_pause() {
            use bevy::time::{TimeUpdateStrategy, Virtual};
            fn run(steps: &[u64], paused: bool) -> (Phase, Duration, Duration, Vec<u32>) {
                let mut app = category_app();
                app.add_systems(Update, advance);
                app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
                app.update();
                if paused {
                    app.world_mut().resource_mut::<Time<Virtual>>().pause();
                }
                for &step in steps {
                    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                        step,
                    )));
                    app.update();
                }
                let world = app.world_mut();
                let clock = world.resource::<MenuPanels>().clock.clone();
                let sounds = world
                    .query::<&crate::menu_audio::AuthoredVolume>()
                    .iter(world)
                    .map(|gain| gain.0.to_bits())
                    .collect();
                (clock.phase, clock.local, clock.global, sounds)
            }
            let steady = run(&[40; 15], false);
            assert_eq!(steady.0, Phase::OptionsStandAlternate);
            assert_eq!(steady.2, Duration::from_millis(640));
            for (steps, paused) in [
                (&[600][..], false),
                (&[7, 333, 260][..], false),
                (&[600][..], true),
            ] {
                assert_eq!(
                    run(steps, paused),
                    steady,
                    "steps={steps:?}, paused={paused}"
                );
            }
        }
    }

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

    fn synthetic_scale() -> VolumeScale {
        VolumeScale {
            maximum: 211.0,
            helper_span: pcx_script::Span::new(0, 1),
            operand_span: pcx_script::Span::new(0, 1),
            conversion_span: pcx_script::Span::new(0, 1),
        }
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
        let (path, volume) =
            resolve_sound(&lookups, &sounds, "SNDXARPD", synthetic_scale()).unwrap();
        assert_eq!(path, r"Sound\Interface\Pop.wav");
        assert_eq!(volume, 64.0 / 211.0);
        assert!(resolve_sound(&lookups, &sounds, "SNDXNONE", synthetic_scale()).is_err());
        assert!(resolve_sound(&lookups, &sounds, "SNDXAMNY", synthetic_scale()).is_err());
        assert!(resolve_sound(&lookups, &sounds, "SNDX", synthetic_scale()).is_err());
    }

    fn event_model(scope: SoundScope) -> Vec<u8> {
        let mut bytes = b"MDLX".to_vec();
        bytes.extend(b"VERS");
        bytes.extend(4u32.to_le_bytes());
        bytes.extend(800u32.to_le_bytes());
        let mut seqs = Vec::new();
        for (i, phase) in scope.phases().iter().enumerate() {
            let (owner, animation) = phase.selector();
            let mut sequence = vec![0; 132];
            let name = format!("{owner} {animation}");
            sequence[..name.len()].copy_from_slice(name.as_bytes());
            let start = 10 + i as u32 * 20;
            sequence[80..84].copy_from_slice(&start.to_le_bytes());
            sequence[84..88].copy_from_slice(&(start + 4).to_le_bytes());
            sequence[92..96].copy_from_slice(&1u32.to_le_bytes());
            seqs.extend(sequence);
        }
        bytes.extend(b"SEQS");
        bytes.extend((seqs.len() as u32).to_le_bytes());
        bytes.extend(seqs);
        let mut event = vec![0; 96];
        event[..4].copy_from_slice(&96u32.to_le_bytes());
        event[4..12].copy_from_slice(b"SNDXTEST");
        event[88..92].copy_from_slice(&(-1i32).to_le_bytes());
        event.extend(b"KEVT");
        event.extend(3u32.to_le_bytes());
        event.extend(pcx_mdl::NO_ID.to_le_bytes());
        for frame in [10u32, 50, 70] {
            event.extend(frame.to_le_bytes());
        }
        bytes.extend(b"EVTS");
        bytes.extend((event.len() as u32).to_le_bytes());
        bytes.extend(event);
        bytes
    }

    #[test]
    fn optional_options_and_single_phase_admission_are_independent() {
        let options = event_model(SoundScope::MainOptions);
        assert_eq!(
            clips("options only", &options, SoundScope::Startup)
                .unwrap()
                .len(),
            2
        );
        let admitted = clips("options only", &options, SoundScope::MainOptions).unwrap();
        assert_eq!(admitted.len(), SoundScope::MainOptions.phases().len());
        assert_eq!(admitted[3].interval, [70, 74]);
        assert!(clips("options only", &options, SoundScope::MainSingle).is_err());
        let single = event_model(SoundScope::MainSingle);
        assert!(clips("single only", &single, SoundScope::MainSingle).is_ok());
        assert!(clips("single only", &single, SoundScope::MainOptions).is_err());
    }

    #[test]
    fn category_plate_sequences_admit_apart_from_the_options_triplet() {
        let category = event_model(SoundScope::Category);
        let admitted = clips("category only", &category, SoundScope::Category).unwrap();
        assert_eq!(admitted.len(), SoundScope::Category.phases().len());
        assert_eq!(admitted[1].interval, [30, 34]);
        assert!(clips("category only", &category, SoundScope::MainOptions).is_err());
        let options = event_model(SoundScope::MainOptions);
        assert!(clips("options only", &options, SoundScope::Category).is_err());

        let mut both = options;
        for (name, start) in [
            ("Options Morph", 200u32),
            ("Options Stand Alternate", 900),
            ("Options Morph Alternate", 300),
        ] {
            let at = both.windows(4).position(|bytes| bytes == b"SEQS").unwrap();
            let size_at = at + 4;
            let size = u32::from_le_bytes(both[size_at..size_at + 4].try_into().unwrap());
            let mut sequence = vec![0; 132];
            sequence[..name.len()].copy_from_slice(name.as_bytes());
            sequence[80..84].copy_from_slice(&start.to_le_bytes());
            sequence[84..88].copy_from_slice(&(start + 4).to_le_bytes());
            sequence[92..96].copy_from_slice(&1u32.to_le_bytes());
            let insert = size_at + 4 + size as usize;
            both.splice(insert..insert, sequence);
            both[size_at..size_at + 4].copy_from_slice(&(size + 132).to_le_bytes());
        }
        let admitted = clips("stand and alternate", &both, SoundScope::MainOptions).unwrap();
        let stand = SoundScope::MainOptions
            .phases()
            .iter()
            .position(|phase| *phase == Phase::OptionsStand)
            .unwrap();
        assert_eq!(admitted[stand].interval, [90, 94]);
        let admitted = clips("stand and alternate", &both, SoundScope::Category).unwrap();
        assert_eq!(
            admitted
                .iter()
                .map(|clip| clip.interval)
                .collect::<Vec<_>>(),
            [[200, 204], [900, 904], [300, 304]]
        );
    }

    #[test]
    fn options_sound_scope_retains_events_and_rejects_looping_transition() {
        let lookups = slk(&[&["AnimSoundEvent", "SoundLabel"], &["TEST", "Synthetic"]]);
        let sounds = slk(&[
            &["SoundName", "DirectoryBase", "FileNames", "Volume"],
            &["Synthetic", "Local/", "one.wav", "53"],
        ]);
        let mut bytes = event_model(SoundScope::MainOptions);
        let admitted = sound_dependencies(
            "options",
            &bytes,
            &lookups,
            &sounds,
            synthetic_scale(),
            SoundScope::MainOptions,
        )
        .unwrap();
        assert_eq!(
            admitted.iter().map(|e| e.frame).collect::<Vec<_>>(),
            [10, 50, 70]
        );
        let offset = bytes.windows(4).position(|b| b == b"SEQS").unwrap() + 8 + 3 * 132;
        bytes[offset + 92..offset + 96].copy_from_slice(&0u32.to_le_bytes());
        assert!(clips("looping Options Birth", &bytes, SoundScope::MainOptions).is_err());
        assert!(clips("unaffected startup", &bytes, SoundScope::Startup).is_ok());
    }

    #[test]
    fn shared_sound_admission_filters_source_phases_and_refuses_ambiguity_or_loss() {
        let lookups = slk(&[&["AnimSoundEvent", "SoundLabel"], &["TEST", "Synthetic"]]);
        let sounds = slk(&[
            &["SoundName", "DirectoryBase", "FileNames", "Volume"],
            &["Synthetic", "Local/", "one.wav", "53"],
        ]);
        let bytes = event_model(SoundScope::MainSingle);
        let startup = sound_dependencies(
            "synthetic",
            &bytes,
            &lookups,
            &sounds,
            synthetic_scale(),
            SoundScope::Startup,
        )
        .unwrap();
        assert_eq!(startup.iter().map(|r| r.frame).collect::<Vec<_>>(), [10]);
        let nav = sound_dependencies(
            "synthetic",
            &bytes,
            &lookups,
            &sounds,
            synthetic_scale(),
            SoundScope::MainSingle,
        )
        .unwrap();
        assert_eq!(
            nav.iter().map(|r| r.frame).collect::<Vec<_>>(),
            [10, 50, 70]
        );
        assert!(
            nav.iter()
                .all(|r| r.path == "Local/one.wav" && r.volume == 53.0 / 211.0)
        );
        assert!(
            sound_dependencies(
                "synthetic",
                &event_model(SoundScope::Startup),
                &lookups,
                &sounds,
                synthetic_scale(),
                SoundScope::MainSingle
            )
            .is_err()
        );
        let ambiguous = slk(&[
            &["AnimSoundEvent", "SoundLabel"],
            &["TEST", "Synthetic"],
            &["TEST", "Synthetic"],
        ]);
        assert!(
            sound_dependencies(
                "synthetic",
                &bytes,
                &ambiguous,
                &sounds,
                synthetic_scale(),
                SoundScope::Startup
            )
            .is_err()
        );
        let duplicate_header = slk(&[
            &["AnimSoundEvent", "SoundLabel", "SoundLabel"],
            &["TEST", "Synthetic", "Other"],
        ]);
        assert!(
            sound_dependencies(
                "synthetic",
                &bytes,
                &duplicate_header,
                &sounds,
                synthetic_scale(),
                SoundScope::Startup
            )
            .is_err()
        );
        for suffix in ["C;X1;Y2;K\"TEST\"\r\nE\r\n", "E\r\nC;X1;Y3;K\"stray\"\r\n"] {
            let text = format!(
                "ID;PWXL;N;E\r\nC;X1;Y1;K\"AnimSoundEvent\"\r\nC;X2;Y1;K\"SoundLabel\"\r\nC;X1;Y2;K\"TEST\"\r\nC;X2;Y2;K\"Synthetic\"\r\n{suffix}"
            );
            let malformed = pcx_slq::Table::parse(text.as_bytes()).unwrap();
            assert!(
                sound_dependencies(
                    "synthetic",
                    &bytes,
                    &malformed,
                    &sounds,
                    synthetic_scale(),
                    SoundScope::Startup
                )
                .is_err()
            );
        }
        let mut global = bytes.clone();
        let at = global.windows(4).position(|b| b == b"KEVT").unwrap();
        global[at + 8..at + 12].copy_from_slice(&0u32.to_le_bytes());
        assert!(
            sound_dependencies(
                "synthetic",
                &global,
                &lookups,
                &sounds,
                synthetic_scale(),
                SoundScope::Startup
            )
            .is_err()
        );
        assert!(
            validate_sound_bytes(
                "bad.wav",
                b"bad",
                pcx_play::scripts::AssetIdentity {
                    edition: "synthetic".into(),
                    version: "synthetic".into(),
                    source: "synthetic".into()
                }
            )
            .is_err()
        );
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
        for panel in &sources {
            let textures = scene::load_textures(Some(&install), &panel.bytes);
            for scope in [
                SoundScope::Startup,
                SoundScope::MainSingle,
                SoundScope::MainOptions,
                SoundScope::Category,
            ] {
                preflight_source(&panel.path, &panel.bytes, &textures, scope).unwrap();
            }
            assert!(
                panel.options.is_ok(),
                "{}: {:?}",
                panel.path,
                panel.options.as_ref().err()
            );
            assert!(
                panel.category.is_ok(),
                "{}: {:?}",
                panel.path,
                panel.category.as_ref().err()
            );
            assert!(
                panel.navigation.is_ok(),
                "{}: {:?}",
                panel.path,
                panel.navigation.as_ref().err()
            );
        }
        let mut world = World::new();
        let mut meshes = Assets::<Mesh>::default();
        let mut materials = Assets::<MenuMaterial>::default();
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
        assert!(
            panels.panels.iter().any(|panel| {
                let landing = panel.plan(Phase::OptionsStand).unwrap();
                let category = panel.plan(Phase::OptionsStandAlternate).unwrap();
                panel.model.meshes.iter().any(|mesh| {
                    category
                        .opacity
                        .sample(mesh.geoset, category.clip.interval[0], 0)
                        .zip(
                            landing
                                .opacity
                                .sample(mesh.geoset, landing.clip.interval[0], 0),
                        )
                        .is_some_and(|(shown, hidden)| shown > hidden)
                })
            }),
            "the selected category sequence must reveal source geometry absent on the landing"
        );
        let mut panels = world.resource_mut::<MenuPanels>();
        let right = panels
            .panels
            .iter()
            .position(|panel| panel.layer == LAYERS[1])
            .unwrap();
        let left = panels
            .panels
            .iter()
            .position(|panel| panel.layer == LAYERS[0])
            .unwrap();
        assert!(panels.panels[right].duration(Phase::OptionsMorph).is_none());
        assert!(panels.panels[left].duration(Phase::OptionsMorph).is_some());
        let right_panel = &panels.panels[right];
        let stand = right_panel.plan(Phase::OptionsStand).unwrap();
        let alternate = right_panel.plan(Phase::OptionsStandAlternate).unwrap();
        assert!(
            right_panel.model.meshes.iter().any(|mesh| {
                stand
                    .opacity
                    .sample(mesh.geoset, stand.clip.interval[0], 0)
                    .zip(
                        alternate
                            .opacity
                            .sample(mesh.geoset, alternate.clip.interval[0], 0),
                    )
                    .is_some_and(|(visible, hidden)| visible > hidden)
            }),
            "the selected right model loses visible housing geometry in the category sequence"
        );
        panels.clock.phase = Phase::OptionsStand;
        panels.panels[right].category = Err("unused right category dependency".into());
        panels
            .sync_category_plate(crate::menu_screens::Screen::Options(
                crate::menu_screens::Page::Video,
            ))
            .unwrap();
        assert_eq!(panels.clock.phase, Phase::OptionsMorph);
        panels.clock.phase = Phase::OptionsStand;
        panels.panels[left].category = Err("synthetic missing category dependency".into());
        assert!(
            panels
                .sync_category_plate(crate::menu_screens::Screen::Options(
                    crate::menu_screens::Page::Video
                ))
                .is_err()
        );
        assert_eq!(panels.clock.phase, Phase::OptionsStand);
        assert!(panels.panels.iter().all(|panel| panel.options.is_ok()));
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
            assert!(panel.clips[0].interval[1] > panel.clips[0].interval[0]);
        }
        let right = &panels[1];
        let pop = right
            .sounds
            .iter()
            .find(|s| s.frame == right.clips[0].interval[0])
            .expect("the right panel's entrance fires a sound on its first frame");
        assert!(
            pop.path.ends_with("RightGlueScreenPopDown.wav"),
            "{}",
            pop.path
        );
        assert!(pop.bytes.starts_with(b"RIFF"));
    }
}

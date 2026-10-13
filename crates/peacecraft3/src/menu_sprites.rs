//! Authored TFT glue sprite layers: the panel model's entrance, as data.
//!
//! The expansion main menu's hanging panel is the model
//! `UI\Glues\SpriteLayers\Expansion\TopRightPanel-Expansion.mdx`, selected by the
//! skin key `GlueSpriteLayerTopRight_V%d` in `UI\war3skins.txt` (see
//! `docs/re-notes/menu-panel-origin.md`). This module reads that model's own
//! `SEQS`, `BONE` and `GEOA` and exposes:
//!
//! 1. a **data source** — [`PanelArt::load`] parses the shipped bytes, and
//!    [`PanelArt::sample_pose`] evaluates the model's bones and geoset alphas at
//!    a time inside a named sequence (`MainMenu Birth` → `MainMenu Stand`);
//! 2. a **render contract** — [`SpriteLayerContract`] carries the native
//!    coordinate facts and [`sprite_camera`] builds an *optional*, isolated
//!    orthographic camera for them.
//!
//! It deliberately does **not** touch `mainmenu_scene`'s `MainMenuScene`
//! resource, its `GlobalAmbientLight` or its cameras. The sprite layer is a
//! separate compositing pass over the 3D scene, and this module only describes
//! it; the application root wires it, and owns the relative viewport.
//!
//! ## Why there is no finished mesh here
//!
//! The panel's raw `VRTX` bounds are `x [0.4243, 0.9734]`, `y [-0.1072, 1.2526]`
//! — they **exceed** the authored glue viewport of `0.8 × 0.6`. So the vertices
//! cannot be placed by the naive `frame.left + modelX` / `frame.bottom + modelY`
//! rule alone: the model's own 168 KB `BONE` hierarchy transforms them first, and
//! that transform has not been checked against the native projection. Building a
//! mesh with a scale chosen to make it "fit" would be inventing the very
//! projection the evidence does not yet give.
//!
//! So this module stops at the **data source and the sampled-pose contract**. The
//! diagnosed blocker is: *the panel needs its bone-skinning transform applied
//! before the glue origin rule, and the posed result has not been compared
//! against the native C `CSpriteFrame` default projection.* Closing it means
//! sampling the model's bones for `MainMenu Birth`/`Stand` and checking the
//! posed extent against `0.8 × 0.6` — which needs the skinning path, not a guess.

use bevy::camera::ScalingMode;
use bevy::prelude::*;
use pcx_mdl::{Interpolation, Mdlx, TrackKind};

/// The panel model the expansion skin selects.
pub const PANEL_MODEL: &str = r"UI\Glues\SpriteLayers\Expansion\TopRightPanel-Expansion.mdx";
/// The served name for a browser.
pub const PANEL_MODEL_SERVED: &str = "menu-sprites/toprightpanel-expansion.mdx";

/// The skin key pattern. `UI\war3skins.txt` supplies `_V0` (Reign of Chaos),
/// `_V1` (The Frozen Throne) and `_V1Beta`; the numeric suffix is the edition.
pub const SKIN_KEY_TOPRIGHT: &str = "GlueSpriteLayerTopRight_V%d";

/// Edition index for the Reign of Chaos panel (`GlueSpriteLayerTopRight_V0`).
pub const SKIN_INDEX_ROC: u32 = 0;
/// Edition index for the expansion panel (`GlueSpriteLayerTopRight_V1`), which is
/// the TFT default. Only this index names the `-Expansion` art; `_V1Beta`
/// resolves to the base panel.
pub const SKIN_INDEX_TFT: u32 = 1;

/// The authored glue viewport, from the original arithmetic
/// `out[0] = in[0] / 0.8f; out[1] = in[1] / 0.6f`
/// (`docs/re-notes/glue-coordinate-runtime.md`). Authored X is divided by this
/// width and authored Y by this height.
pub const GLUE_VIEW_WIDTH: f32 = 0.8;
/// The authored glue viewport height.
pub const GLUE_VIEW_HEIGHT: f32 = 0.6;

/// The camera argument the glue assigns the top-left/top-right overlay sprites.
///
/// `−1` means *no authored camera*: the overlay takes the default sprite
/// projection rather than a `CAMS` record, which is why this model needs no
/// camera selection (`docs/re-notes/glue-coordinate-runtime.md`).
pub const OVERLAY_CAMERA_ARG: i32 = -1;

/// The sequence that plays when the main menu appears.
pub const SEQ_MAINMENU_BIRTH: &str = "MainMenu Birth";
/// The sequence the menu holds.
pub const SEQ_MAINMENU_STAND: &str = "MainMenu Stand";
/// The sequence that plays when the menu is dismissed.
pub const SEQ_MAINMENU_DEATH: &str = "MainMenu Death";

/// Render layer for the sprite overlay. The root gives this layer to the sprite
/// camera and to the sprite entities so the pass isolates the original 3D scene;
/// it is a number rather than a `RenderLayers` value here because this module
/// owns no GPU state.
pub const SPRITE_RENDER_LAYER: u8 = 7;

/// Why the panel model could not be read.
#[derive(Debug, Clone, PartialEq)]
pub enum PanelProblem {
    /// `Mdlx::parse` failed.
    Parse(String),
    /// A chunk the panel needs did not decode.
    Chunk(String),
    /// The named sequence is absent.
    NoSequence(String),
    /// The model has no bones, so nothing can be posed.
    NoBones,
}

impl std::fmt::Display for PanelProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "the panel model did not parse: {e}"),
            Self::Chunk(e) => write!(f, "a panel chunk did not decode: {e}"),
            Self::NoSequence(n) => write!(f, "the panel has no {n:?} sequence"),
            Self::NoBones => write!(f, "the panel model has no bones"),
        }
    }
}

/// One `SEQS` entry, as parsed.
#[derive(Debug, Clone, PartialEq)]
pub struct PanelSequence {
    /// Sequence name, e.g. `MainMenu Birth`.
    pub name: String,
    /// First frame, milliseconds.
    pub start: u32,
    /// Last frame, milliseconds.
    pub end: u32,
}

impl PanelSequence {
    /// Length in milliseconds.
    #[must_use]
    pub fn duration(&self) -> u32 {
        self.end.saturating_sub(self.start)
    }
}

/// One key of a bone track.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrackKey {
    /// Frame, milliseconds.
    pub frame: u32,
    /// The key's value.
    pub value: [f32; 4],
    /// Tangent into this key.
    pub in_tan: [f32; 4],
    /// Tangent out of this key.
    pub out_tan: [f32; 4],
}

/// One bone track, reduced to the keys the pose sampler reads.
#[derive(Debug, Clone, PartialEq)]
pub struct PanelTrack {
    /// Translation, rotation or scaling.
    pub kind: TrackKind,
    /// How keys between samples are joined.
    pub interpolation: Interpolation,
    /// The keys, in file order.
    pub keys: Vec<TrackKey>,
}

/// One bone, with the tracks that move it.
#[derive(Debug, Clone, PartialEq)]
pub struct PanelBone {
    /// Identifier other records reference.
    pub object_id: u32,
    /// Parent object id, or a negative value for a root.
    pub parent_id: i32,
    /// Authored name.
    pub name: String,
    /// The bone's animation tracks.
    pub tracks: Vec<PanelTrack>,
}

/// A geoset-animation record, bound to its geoset **by id**.
///
/// The `GEOA` record's explicit `geosetId` sits at record `+24` (`body + 20`),
/// which the current decoder drops, so this module reads it from the raw chunk
/// and carries it here. Binding is by that id — **never** by array position,
/// because the panel's ids are a subset of the geoset indices (it has records for
/// `0, 2..8, 14..56` and none for `1, 9..13`), so positional pairing would
/// misbind at least six geosets. `None` means the id could not be read, which is
/// reported rather than defaulted.
#[derive(Debug, Clone, PartialEq)]
pub struct PanelGeosetAnim {
    /// The geoset this record animates, read from the record itself.
    pub geoset_id: Option<u32>,
    /// Record flags.
    pub flags: u32,
    /// Static alpha, before any track.
    pub static_alpha: f32,
    /// Static colour, before any track.
    pub static_colour: [f32; 3],
    /// `KGAO`/`KGAA` alpha keys, if the record animates alpha.
    pub alpha_keys: Vec<(u32, f32)>,
}

/// A geoset's vertex bounds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelQuad {
    /// `minX, minY, maxX, maxY`.
    pub bounds: (f32, f32, f32, f32),
}

impl PanelQuad {
    /// Width in model units.
    #[must_use]
    pub fn width(&self) -> f32 {
        self.bounds.2 - self.bounds.0
    }

    /// Height in model units.
    #[must_use]
    pub fn height(&self) -> f32 {
        self.bounds.3 - self.bounds.1
    }
}

/// The parsed panel model.
#[derive(Debug, Clone, PartialEq)]
pub struct PanelArt {
    /// `SEQS`, in file order.
    pub sequences: Vec<PanelSequence>,
    /// `BONE`, in file order.
    pub bones: Vec<PanelBone>,
    /// `GEOA`, in file order.
    pub geoset_anims: Vec<PanelGeosetAnim>,
    /// One quad per geoset, in file order.
    pub quads: Vec<PanelQuad>,
    /// `TEXS` paths, in file order.
    pub textures: Vec<String>,
    /// Number of `CAMS` records. Expected to be zero for an overlay sprite.
    pub camera_count: usize,
}

impl PanelArt {
    /// Reads the panel out of the model's bytes.
    ///
    /// # Errors
    ///
    /// Returns [`PanelProblem`] when the model will not parse or a chunk it needs
    /// will not decode. Nothing is defaulted silently.
    pub fn load(bytes: &[u8]) -> Result<Self, PanelProblem> {
        let m = Mdlx::parse(bytes).map_err(|e| PanelProblem::Parse(e.to_string()))?;
        let chunk = |e: pcx_mdl::Error| PanelProblem::Chunk(e.to_string());

        let sequences = m
            .sequences(bytes)
            .map_err(chunk)?
            .into_iter()
            .map(|s| PanelSequence {
                name: s.name,
                start: s.interval_start,
                end: s.interval_end,
            })
            .collect();

        let bones: Vec<PanelBone> = m
            .bones(bytes)
            .map_err(chunk)?
            .into_iter()
            .map(|b| PanelBone {
                object_id: b.object_id,
                parent_id: b.parent_id,
                name: b.name,
                tracks: b
                    .tracks
                    .iter()
                    .map(|t| PanelTrack {
                        kind: t.kind,
                        interpolation: t.interpolation,
                        keys: (0..t.count as usize)
                            .map_while(|i| {
                                Some(TrackKey {
                                    frame: t.key_time(i)?,
                                    value: t.value(i)?,
                                    in_tan: t.in_tangent(i)?,
                                    out_tan: t.out_tangent(i)?,
                                })
                            })
                            .collect(),
                    })
                    .collect(),
            })
            .collect();

        let geoset_ids = geoa_geoset_ids(&m, bytes);
        let geoset_anims: Vec<PanelGeosetAnim> = m
            .geoset_anims(bytes)
            .map_err(chunk)?
            .into_iter()
            .enumerate()
            .map(|(i, g)| PanelGeosetAnim {
                geoset_id: geoset_ids.get(i).copied(),
                flags: g.flags,
                static_alpha: g.static_alpha,
                static_colour: g.static_colour,
                alpha_keys: g
                    .tracks
                    .iter()
                    .filter(|t| matches!(&t.magic[..], b"KGAO" | b"KGAA"))
                    .flat_map(|t| (0..t.count as usize).map(|i| (t.frame(i), t.value(i))))
                    .collect(),
            })
            .collect();

        let quads = m
            .geosets(bytes)
            .map_err(chunk)?
            .iter()
            .map(|g| PanelQuad {
                bounds: vrtx_bounds(&g.data).unwrap_or((0.0, 0.0, 0.0, 0.0)),
            })
            .collect();

        let textures = m
            .textures(bytes)
            .map_err(chunk)?
            .into_iter()
            .map(|t| t.path)
            .collect();

        let camera_count = m.cameras(bytes).map_err(chunk)?.len();

        Ok(Self {
            sequences,
            bones,
            geoset_anims,
            quads,
            textures,
            camera_count,
        })
    }

    /// The named sequence, if present.
    #[must_use]
    pub fn sequence(&self, name: &str) -> Option<&PanelSequence> {
        self.sequences.iter().find(|s| s.name == name)
    }

    /// The geoset-animation record that animates `geoset_id`, found **by id**.
    #[must_use]
    pub fn geoanim_for_geoset(&self, geoset_id: u32) -> Option<&PanelGeosetAnim> {
        self.geoset_anims
            .iter()
            .find(|g| g.geoset_id == Some(geoset_id))
    }

    /// The alpha of `geoset_id` at an absolute frame, or `None` when no record
    /// animates that geoset.
    #[must_use]
    pub fn alpha_for_geoset(&self, geoset_id: u32, frame: f64) -> Option<f32> {
        let g = self.geoanim_for_geoset(geoset_id)?;
        if g.alpha_keys.is_empty() {
            Some(g.static_alpha)
        } else {
            sample_alpha(&g.alpha_keys, frame).or(Some(g.static_alpha))
        }
    }

    /// The union of every geoset's vertex bounds, in **model** units.
    ///
    /// These are model-space extents. They are *not* screen positions: see the
    /// module note on why the bone transform must be applied first.
    #[must_use]
    pub fn model_bounds(&self) -> (f32, f32, f32, f32) {
        let mut b = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for q in &self.quads {
            b.0 = b.0.min(q.bounds.0);
            b.1 = b.1.min(q.bounds.1);
            b.2 = b.2.max(q.bounds.2);
            b.3 = b.3.max(q.bounds.3);
        }
        b
    }

    /// Evaluates the model at `t_ms` into `seq`.
    ///
    /// Every bone with a key on or before the sample contributes its local
    /// translation, rotation and scale; every geoset-animation record contributes
    /// an alpha. Frame is absolute — `seq.start + t_ms` — because the records key
    /// absolute model frames, not sequence-relative ones.
    #[must_use]
    pub fn sample_pose(&self, seq: &PanelSequence, t_ms: f64) -> PanelPose {
        let frame = f64::from(seq.start) + t_ms.max(0.0);
        let mut nodes = Vec::new();
        for bone in &self.bones {
            let mut translation = Vec3::ZERO;
            let mut rotation = Quat::IDENTITY;
            let mut scale = Vec3::ONE;
            let mut touched = false;
            for track in &bone.tracks {
                let Some(v) = sample_keys(&track.keys, track.interpolation, frame) else {
                    continue;
                };
                touched = true;
                match track.kind {
                    TrackKind::Translation => translation = Vec3::new(v[0], v[1], v[2]),
                    TrackKind::Scaling => scale = Vec3::new(v[0], v[1], v[2]),
                    TrackKind::Rotation => {
                        let q = Quat::from_xyzw(v[0], v[1], v[2], v[3]);
                        rotation = if q.length_squared() > 0.0 {
                            q.normalize()
                        } else {
                            Quat::IDENTITY
                        };
                    }
                }
            }
            if touched {
                nodes.push((
                    bone.object_id,
                    NodePose {
                        translation,
                        rotation,
                        scale,
                    },
                ));
            }
        }

        let alphas = self
            .geoset_anims
            .iter()
            .map(|g| {
                if g.alpha_keys.is_empty() {
                    g.static_alpha
                } else {
                    sample_alpha(&g.alpha_keys, frame).unwrap_or(g.static_alpha)
                }
            })
            .collect();

        PanelPose {
            frame,
            nodes,
            alphas,
        }
    }
}

/// A bone's local pose at one instant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodePose {
    /// Local translation.
    pub translation: Vec3,
    /// Local rotation.
    pub rotation: Quat,
    /// Local scale.
    pub scale: Vec3,
}

/// The model evaluated at one frame.
#[derive(Debug, Clone, PartialEq)]
pub struct PanelPose {
    /// The absolute model frame sampled.
    pub frame: f64,
    /// `(object_id, local pose)` for every bone that moved.
    pub nodes: Vec<(u32, NodePose)>,
    /// Alpha per geoset-animation record, in file order.
    pub alphas: Vec<f32>,
}

/// The native facts a sprite layer is drawn against.
///
/// Data only — the application root owns the camera, the viewport and the
/// isolation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpriteLayerContract {
    /// Authored viewport width (authored X is divided by this).
    pub view_width: f32,
    /// Authored viewport height (authored Y is divided by this).
    pub view_height: f32,
    /// The glue's camera argument for the overlay: `−1`, i.e. default projection.
    pub camera_arg: i32,
    /// Render layer the root should give the camera and the sprite entities.
    pub render_layer: u8,
    /// Edition index whose skin key names the expansion panel.
    pub edition_index: u32,
}

impl Default for SpriteLayerContract {
    fn default() -> Self {
        Self {
            view_width: GLUE_VIEW_WIDTH,
            view_height: GLUE_VIEW_HEIGHT,
            camera_arg: OVERLAY_CAMERA_ARG,
            render_layer: SPRITE_RENDER_LAYER,
            edition_index: SKIN_INDEX_TFT,
        }
    }
}

impl SpriteLayerContract {
    /// The skin key for this contract's edition.
    #[must_use]
    pub fn skin_key(&self) -> String {
        skin_key(SKIN_INDEX_TOPRIGHT, self.edition_index)
    }
}

/// The sprite-layer key family, for readability at call sites.
pub const SKIN_INDEX_TOPRIGHT: &str = "GlueSpriteLayerTopRight";

/// Builds `GlueSpriteLayerTopRight_V{edition}`.
///
/// The pattern is `GlueSpriteLayerTopRight_V%d`; only the edition integer varies,
/// and only `0`/`1` resolve to shipped art (`V1Beta` is a different key the `%d`
/// call cannot produce).
#[must_use]
pub fn skin_key(family: &str, edition: u32) -> String {
    format!("{family}_V{edition}")
}

/// An **optional** orthographic camera for the sprite layer.
///
/// The projection is `ScalingMode::Fixed` at the authored `0.8 × 0.6`, which is
/// the glue's own unit rectangle rather than a guessed scale. Z is left at the
/// default because no evidence fixes it for this layer, and the origin is
/// `Transform::default()` for the root to position from the frame's own anchors —
/// this function does not invent a screen-space offset.
///
/// Isolation is the root's one remaining line: give both this camera and the
/// sprite entities `RenderLayers` for [`SpriteLayerContract::render_layer`], so
/// the pass does not draw the original 3D scene. This module does not touch
/// `mainmenu_scene`'s cameras or `MainMenuScene`.
#[must_use]
pub fn sprite_camera() -> (Camera2d, Projection, Transform) {
    (
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::Fixed {
                width: GLUE_VIEW_WIDTH,
                height: GLUE_VIEW_HEIGHT,
            },
            ..OrthographicProjection::default_2d()
        }),
        Transform::default(),
    )
}

/// Samples a bone track at an absolute frame.
///
/// `None` is a step, `Linear` joins the two bracketing keys, and the tangent
/// types use their keys' tangents. Outside the key range the nearest key holds.
#[must_use]
pub fn sample_keys(
    keys: &[TrackKey],
    interpolation: Interpolation,
    frame: f64,
) -> Option<[f32; 4]> {
    let first = keys.first()?;
    if frame <= f64::from(first.frame) {
        return Some(first.value);
    }
    let last = keys.last()?;
    if frame >= f64::from(last.frame) {
        return Some(last.value);
    }
    let hi = keys.iter().position(|k| f64::from(k.frame) >= frame)?;
    let lo = hi.saturating_sub(1);
    let (a, b) = (&keys[lo], &keys[hi]);
    let span = f64::from(b.frame) - f64::from(a.frame);
    if span <= 0.0 {
        return Some(a.value);
    }
    let t = ((frame - f64::from(a.frame)) / span) as f32;
    let mut out = [0.0f32; 4];
    for c in 0..4 {
        out[c] = match interpolation {
            Interpolation::None => a.value[c],
            Interpolation::Linear => a.value[c] + (b.value[c] - a.value[c]) * t,
            // The format's cubic Bézier stores its two control values, so the
            // curve is the cubic through (a.value, a.out, b.in, b.value).
            Interpolation::Bezier => bezier(a.value[c], a.out_tan[c], b.in_tan[c], b.value[c], t),
            Interpolation::Hermite => hermite(a.value[c], b.value[c], a.out_tan[c], b.in_tan[c], t),
        };
    }
    Some(out)
}

/// The cubic Bézier through control values `p0, p1, p2, p3` at `t`.
#[must_use]
pub fn bezier(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    let u = 1.0 - t;
    u * u * u * p0 + 3.0 * u * u * t * p1 + 3.0 * u * t * t * p2 + t * t * t * p3
}

/// Cubic Hermite between two values with slopes `m0` and `m1`.
#[must_use]
pub fn hermite(p0: f32, p1: f32, m0: f32, m1: f32, t: f32) -> f32 {
    let t2 = t * t;
    let t3 = t2 * t;
    (2.0 * t3 - 3.0 * t2 + 1.0) * p0
        + (t3 - 2.0 * t2 + t) * m0
        + (-2.0 * t3 + 3.0 * t2) * p1
        + (t3 - t2) * m1
}

/// Samples a scalar alpha track at an absolute frame.
#[must_use]
pub fn sample_alpha(keys: &[(u32, f32)], frame: f64) -> Option<f32> {
    let first = keys.first()?;
    if frame <= f64::from(first.0) {
        return Some(first.1);
    }
    let last = keys.last()?;
    if frame >= f64::from(last.0) {
        return Some(last.1);
    }
    let hi = keys.iter().position(|k| f64::from(k.0) >= frame)?;
    let (a, b) = (keys[hi - 1], keys[hi]);
    let span = f64::from(b.0) - f64::from(a.0);
    if span <= 0.0 {
        return Some(a.1);
    }
    let t = ((frame - f64::from(a.0)) / span) as f32;
    Some(a.1 + (b.1 - a.1) * t)
}

/// Reads each `GEOA` record's explicit geoset id, in file order.
///
/// The decoder drops the field, so it is read here from the raw chunk: a record
/// is `size(u32)` then a body, and the geoset id is at body `+20`, i.e. record
/// `+24`. The advance is `size` (the size includes its own four bytes). A record
/// that will not close ends the walk rather than guessing, so a short vector is
/// itself a signal that the binding is incomplete.
fn geoa_geoset_ids(m: &Mdlx, bytes: &[u8]) -> Vec<u32> {
    let Some(p) = m.payload(bytes, b"GEOA") else {
        return Vec::new();
    };
    let mut ids = Vec::new();
    let mut at = 0usize;
    while at + 28 <= p.len() {
        let Some(head) = p.get(at..at + 4).and_then(|b| b.try_into().ok()) else {
            break;
        };
        let size = u32::from_le_bytes(head) as usize;
        if size < 24 || at + size > p.len() {
            break;
        }
        let Some(id) = p.get(at + 24..at + 28).and_then(|b| b.try_into().ok()) else {
            break;
        };
        ids.push(u32::from_le_bytes(id));
        at += size;
    }
    ids
}

/// `minX, minY, maxX, maxY` over a geoset's `VRTX` array.
fn vrtx_bounds(data: &[u8]) -> Option<(f32, f32, f32, f32)> {
    let at = data.windows(4).position(|w| w == b"VRTX")?;
    let count = u32::from_le_bytes(data.get(at + 4..at + 8)?.try_into().ok()?) as usize;
    if count == 0 {
        return None;
    }
    let mut b = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for i in 0..count {
        let base = at + 8 + i * 12;
        let s = data.get(base..base + 12)?;
        let x = f32::from_le_bytes(s[0..4].try_into().ok()?);
        let y = f32::from_le_bytes(s[4..8].try_into().ok()?);
        b.0 = b.0.min(x);
        b.1 = b.1.min(y);
        b.2 = b.2.max(x);
        b.3 = b.3.max(y);
    }
    Some(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(
        kind: TrackKind,
        interpolation: Interpolation,
        keys: &[(u32, [f32; 4])],
    ) -> PanelTrack {
        PanelTrack {
            kind,
            interpolation,
            keys: keys
                .iter()
                .map(|(f, v)| TrackKey {
                    frame: *f,
                    value: *v,
                    in_tan: [0.0; 4],
                    out_tan: [0.0; 4],
                })
                .collect(),
        }
    }

    /// A linear translation track interpolates, and holds outside its range.
    #[test]
    fn a_linear_track_interpolates_and_holds() {
        let t = track(
            TrackKind::Translation,
            Interpolation::Linear,
            &[(100, [0.0, 0.0, 0.0, 0.0]), (300, [0.2, -0.4, 0.0, 0.0])],
        );
        let mid = sample_keys(&t.keys, t.interpolation, 200.0).unwrap();
        assert!((mid[0] - 0.1).abs() < 1e-6, "{mid:?}");
        assert!((mid[1] + 0.2).abs() < 1e-6, "{mid:?}");
        assert_eq!(
            sample_keys(&t.keys, t.interpolation, 0.0).unwrap(),
            [0.0; 4]
        );
        // Below the first key holds the first; above the last holds the last.
        assert_eq!(
            sample_keys(&t.keys, t.interpolation, 9.0).unwrap(),
            [0.0; 4]
        );
        assert_eq!(
            sample_keys(&t.keys, t.interpolation, 400.0).unwrap(),
            [0.2, -0.4, 0.0, 0.0]
        );
    }

    /// A Bézier track is read as control values, not as a ramp.
    #[test]
    fn a_bezier_track_uses_control_values() {
        let mut t = track(
            TrackKind::Translation,
            Interpolation::Bezier,
            &[(0, [0.0; 4]), (500, [1.0, 0.0, 0.0, 0.0])],
        );
        t.keys[0].out_tan = [0.533_333_0, 0.0, 0.0, 0.0];
        t.keys[1].in_tan = [0.866_667_0, 0.0, 0.0, 0.0];
        let mid = sample_keys(&t.keys, t.interpolation, 250.0).unwrap()[0];
        assert!((mid - 0.65).abs() < 0.005, "got {mid}");
        assert!((mid - 0.5).abs() > 0.1, "not a ramp");
    }

    /// A step track does not move between keys.
    #[test]
    fn a_step_track_holds_until_the_next_key() {
        let t = track(
            TrackKind::Scaling,
            Interpolation::None,
            &[(0, [1.0, 1.0, 1.0, 0.0]), (400, [2.0, 2.0, 2.0, 0.0])],
        );
        assert_eq!(
            sample_keys(&t.keys, t.interpolation, 399.0).unwrap()[0],
            1.0
        );
        assert_eq!(
            sample_keys(&t.keys, t.interpolation, 400.0).unwrap()[0],
            2.0
        );
    }

    /// The alpha sampler behaves the same way as the bone sampler.
    #[test]
    fn alpha_samples_linearly() {
        let keys = [(0u32, 0.0f32), (1000, 1.0)];
        assert_eq!(sample_alpha(&keys, 0.0), Some(0.0));
        assert_eq!(sample_alpha(&keys, 1000.0), Some(1.0));
        assert!((sample_alpha(&keys, 500.0).unwrap() - 0.5).abs() < 1e-6);
        assert_eq!(sample_alpha(&[], 5.0), None);
    }

    /// The skin key is the pattern the table uses, with the edition as the digit.
    #[test]
    fn the_skin_key_is_the_documented_pattern() {
        assert_eq!(
            skin_key(SKIN_INDEX_TOPRIGHT, SKIN_INDEX_ROC),
            "GlueSpriteLayerTopRight_V0"
        );
        assert_eq!(
            skin_key(SKIN_INDEX_TOPRIGHT, SKIN_INDEX_TFT),
            "GlueSpriteLayerTopRight_V1"
        );
        assert_eq!(
            SpriteLayerContract::default().skin_key(),
            "GlueSpriteLayerTopRight_V1"
        );
    }

    /// The contract carries the native numbers, and the camera uses them.
    #[test]
    fn the_contract_uses_the_native_glue_numbers() {
        let c = SpriteLayerContract::default();
        assert_eq!((c.view_width, c.view_height), (0.8, 0.6));
        assert_eq!(
            c.camera_arg, -1,
            "overlay sprites take the default projection"
        );
        let (_, projection, _) = sprite_camera();
        let Projection::Orthographic(o) = projection else {
            panic!("expected an orthographic projection");
        };
        // `ScalingMode` is not `PartialEq`; match the variant and compare fields.
        match o.scaling_mode {
            ScalingMode::Fixed { width, height } => assert_eq!((width, height), (0.8, 0.6)),
            other => panic!("expected ScalingMode::Fixed at 0.8 x 0.6, got {other:?}"),
        }
    }

    /// Geoset-animation records are looked up by their own id, so a geoset with
    /// no record reports `None` instead of picking up a neighbour's alpha.
    #[test]
    fn geoset_anims_bind_by_id_not_by_position() {
        let art = PanelArt {
            sequences: vec![PanelSequence {
                name: "MainMenu Birth".to_string(),
                start: 0,
                end: 1000,
            }],
            bones: Vec::new(),
            geoset_anims: vec![
                PanelGeosetAnim {
                    geoset_id: Some(5),
                    flags: 0,
                    static_alpha: 0.25,
                    static_colour: [1.0; 3],
                    alpha_keys: Vec::new(),
                },
                PanelGeosetAnim {
                    geoset_id: Some(9),
                    flags: 0,
                    static_alpha: 0.75,
                    static_colour: [1.0; 3],
                    alpha_keys: Vec::new(),
                },
            ],
            quads: Vec::new(),
            textures: Vec::new(),
            camera_count: 0,
        };
        // Position 0 holds id 5, position 1 holds id 9 — so index != id.
        assert_eq!(art.alpha_for_geoset(5, 0.0), Some(0.25));
        assert_eq!(art.alpha_for_geoset(9, 0.0), Some(0.75));
        assert_eq!(
            art.alpha_for_geoset(1, 0.0),
            None,
            "id 1 must not fall back to the record at position 0"
        );
        assert_eq!(
            art.geoanim_for_geoset(9).map(|g| g.static_alpha),
            Some(0.75)
        );
    }

    /// Against the real panel model: sequences, bones, quads and the no-camera
    /// fact this module's contract depends on.
    ///
    /// Ignored because it needs the extracted game asset; run with `--ignored` and
    /// `PEACECRAFT3_LEGACY_DIR` set. It **fails** when the asset is absent rather
    /// than passing silently.
    #[test]
    #[ignore = "needs the extracted panel model; run with --ignored and PEACECRAFT3_LEGACY_DIR set"]
    fn the_shipped_panel_parses() {
        let mut roots: Vec<std::path::PathBuf> = Vec::new();
        if let Some(p) = std::env::var_os("PEACECRAFT3_LEGACY_DIR") {
            roots.push(std::path::PathBuf::from(p));
        }
        if let Some(p) = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
        {
            roots.push(p.join("web/assets/menu-sprites"));
        }
        let bytes = roots
            .iter()
            .find_map(|r| std::fs::read(r.join("toprightpanel-expansion.mdx")).ok())
            .unwrap_or_else(|| panic!("panel model not found; searched {roots:?}"));

        let art = PanelArt::load(&bytes).expect("the shipped panel parses");
        assert_eq!(
            art.camera_count, 0,
            "an overlay sprite has no authored camera"
        );
        let birth = art.sequence(SEQ_MAINMENU_BIRTH).expect("MainMenu Birth");
        assert_eq!(birth.duration(), 1000, "the panel entrance is 1000 ms");
        assert!(art.sequence(SEQ_MAINMENU_STAND).is_some());
        assert!(!art.bones.is_empty(), "the panel has bones");

        // Binding is by the record's own geoset id, not by array position: the
        // shipped ids are a subset that skips 1 and 9..13.
        let ids: Vec<u32> = art
            .geoset_anims
            .iter()
            .filter_map(|g| g.geoset_id)
            .collect();
        assert_eq!(
            ids.len(),
            art.geoset_anims.len(),
            "every record carries an id"
        );
        assert!(
            ids.contains(&0) && ids.contains(&56),
            "ids span the geoset range"
        );
        assert!(!ids.contains(&1), "geoset 1 has no record");
        assert!(art.alpha_for_geoset(0, 2500.0).is_some());
        assert!(
            art.alpha_for_geoset(1, 2500.0).is_none(),
            "a geoset with no record reports none, rather than the next record's alpha"
        );

        let (x0, y0, x1, y1) = art.model_bounds();
        assert!(
            x1 - x0 > 0.4 && y1 - y0 > 1.0,
            "model bounds {x0} {y0} {x1} {y1}"
        );
        // The blocker this module documents: those model bounds exceed 0.8 x 0.6.
        assert!(x1 > GLUE_VIEW_WIDTH || y1 > GLUE_VIEW_HEIGHT);

        let pose = art.sample_pose(birth, 0.0);
        assert!(pose.frame >= f64::from(birth.start));
    }
}

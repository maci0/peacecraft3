//! The selected main menu logo, its authored quad and entrance.
//!
//! [`LogoData::parse`] reads the quad, sequences and alpha curve from the
//! selected model. Measured fixture expectations remain test-only; the fixed
//! served names below are transport compatibility aliases.
//!
//! The caller explicitly selects the skin's `MainMenuLogo_V0` or
//! `MainMenuLogo_V1` binding. Its model supplies the texture, quad and opacity;
//! `WarCraftIIILogo` in the current MainMenu FDF supplies the anchor. The caller
//! retains its existing model-quad projection policy.

use bevy::prelude::*;

use pcx_mdl::NO_ID;

/// The model the expansion ships for the menu logo.
pub const LOGO_MODEL: &str = r"UI\Glues\MainMenu\WarCraftIIILogo_exp\WarCraftIIILogo_exp.mdx";
pub const LOGO_FRAME: &str = "WarCraftIIILogo";
/// The texture that model names in its own `TEXS`.
pub const LOGO_TEXTURE: &str = r"ReplaceableTextures\WorldEditUI\WarcraftIIIFTLogo.blp";

/// The served name of the model, for a browser.
pub const LOGO_MODEL_SERVED: &str = "menu-exp/warcraftiiilogo_exp.mdx";
/// The served name of the texture, for a browser.
pub const LOGO_TEXTURE_SERVED: &str = "menu-exp/warcraftiiiftlogo.blp";

/// The quad bounds measured from the shipped model: `minX, minY, maxX, maxY`.
///
/// Expected values for the fixture test only. The renderer uses
/// [`LogoData::quad`], which is parsed.
#[cfg(test)]
pub const LOGO_QUAD_MEASURED: (f32, f32, f32, f32) = (-0.112_09, -0.058_2, 0.111_46, 0.053_3);

/// The textures and sequences the model is expected to carry, for the fixture
/// test.
#[cfg(test)]
pub const LOGO_BIRTH_MS_EXPECTED: f64 = 500.0;

/// One cubic-Bézier key, as the model stores it.
///
/// On disk a key is `frame(u32)`, `value(f32)`, then two tangents. Measured, the
/// first tangent is the key's **in** tangent and the second its **out** tangent
/// — the shipped model's first key reads `in 0.0, out 0.5333330035`, which is
/// what a first key holding zero looks like. The segment between two keys is the
/// cubic whose control values are
/// `(k0.value, k0.out_tan, k1.in_tan, k1.value)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BezierKey {
    /// Frame, in milliseconds.
    pub frame: u32,
    /// The key's value.
    pub value: f32,
    /// Tangent into this key.
    pub in_tan: f32,
    /// Tangent out of this key.
    pub out_tan: f32,
}

/// A parsed alpha curve, or the reason it could not be read.
#[derive(Debug, Clone, PartialEq)]
pub enum AlphaCurve {
    /// A cubic-Bézier track, as the logo carries.
    Bezier(Vec<BezierKey>),
    /// The track uses an interpolation this module does not implement, or is
    /// absent. Reported, never guessed at.
    Unsupported(String),
}

/// Why a model could not be read as the logo.
#[derive(Debug, Clone, PartialEq)]
pub enum LogoProblem {
    /// `Mdlx::parse` failed.
    Parse(String),
    /// The model has no `SEQS`, so there is no entrance to play.
    NoSequences,
    /// The model has no `Birth` sequence.
    NoBirth,
    /// The model has no geoset, so there is no quad.
    NoGeoset,
    /// The model's first texture has an empty path, so there is nothing to draw.
    NoTexture,
    /// No material carried a `KMTA` block at all.
    NoAlphaTrack,
    /// A `KMTA` block exists but this module will not use it. The reason is the
    /// block's own — an unimplemented interpolation, a global-sequence clock, a
    /// length that does not match its key count, a non-finite value, or keys out
    /// of order. Kept verbatim rather than flattened into `NoAlphaTrack`, which
    /// would describe a different problem.
    AlphaUnsupported(String),
}

impl std::fmt::Display for LogoProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "the model did not parse: {e}"),
            Self::NoSequences => write!(f, "the model carries no sequences"),
            Self::NoBirth => write!(f, "the model has no Birth sequence"),
            Self::NoGeoset => write!(f, "the model has no geoset"),
            Self::NoTexture => write!(f, "the model's first texture has an empty path"),
            Self::NoAlphaTrack => write!(f, "no material carries an alpha track"),
            Self::AlphaUnsupported(reason) => write!(f, "the alpha track is unusable: {reason}"),
        }
    }
}

/// One `SEQS` entry, as parsed.
#[derive(Debug, Clone, PartialEq)]
pub struct LogoSequence {
    /// Sequence name, e.g. `Birth`.
    pub name: String,
    /// First frame, milliseconds.
    pub start: u32,
    /// Last frame, milliseconds.
    pub end: u32,
}

impl LogoSequence {
    /// Length in milliseconds.
    #[must_use]
    #[cfg(test)]
    pub fn duration(&self) -> u32 {
        self.end.saturating_sub(self.start)
    }
}

/// Everything the entrance needs, read from the model's own bytes.
#[derive(Debug, Clone, PartialEq)]
pub struct LogoData {
    /// The model's sequences, in file order.
    pub sequences: Vec<LogoSequence>,
    /// `minX, minY, maxX, maxY` over every geoset `VRTX`.
    pub quad: (f32, f32, f32, f32),
    /// True when every vertex shares one `z`, i.e. the art is screen-space.
    pub planar: bool,
    /// The path of the model's first texture.
    pub texture: String,
    /// The alpha track that drives the entrance.
    pub alpha: AlphaCurve,
}

impl LogoData {
    /// Reads the logo out of the model's bytes.
    ///
    /// # Errors
    ///
    /// Returns [`LogoProblem`] when the model will not parse or lacks the pieces
    /// the entrance needs; a missing piece is reported rather than defaulted.
    pub fn parse(bytes: &[u8]) -> Result<Self, LogoProblem> {
        let m = pcx_mdl::Mdlx::parse(bytes).map_err(|e| LogoProblem::Parse(e.to_string()))?;

        let sequences: Vec<LogoSequence> = m
            .sequences(bytes)
            .map_err(|e| LogoProblem::Parse(e.to_string()))?
            .into_iter()
            .map(|s| LogoSequence {
                name: s.name,
                start: s.interval_start,
                end: s.interval_end,
            })
            .collect();
        if sequences.is_empty() {
            return Err(LogoProblem::NoSequences);
        }
        if !sequences.iter().any(|s| s.name == "Birth") {
            return Err(LogoProblem::NoBirth);
        }

        // The quad, from the `VRTX` array of the first geoset that has one.
        let geosets = m
            .geosets(bytes)
            .map_err(|e| LogoProblem::Parse(e.to_string()))?;
        let verts = geosets
            .iter()
            .find_map(|g| vrtx_of(&g.data).filter(|v| !v.is_empty()))
            .ok_or(LogoProblem::NoGeoset)?;
        let quad = bounds_of(&verts);
        let planar = verts.iter().all(|v| v[2] == verts[0][2]);

        let texture = m
            .textures(bytes)
            .map_err(|e| LogoProblem::Parse(e.to_string()))?
            .first()
            .map(|t| t.path.clone())
            .filter(|p| !p.is_empty())
            .ok_or(LogoProblem::NoTexture)?;

        // The alpha track. A material with no `KMTA` is silent; a material whose
        // `KMTA` exists but cannot be used speaks, and that reason is carried out
        // rather than being folded into "there was no alpha track" — which would
        // report a problem the model does not have.
        let materials = m
            .materials(bytes)
            .map_err(|e| LogoProblem::Parse(e.to_string()))?;
        let mut alpha = None;
        let mut reason = None;
        for mat in &materials {
            match alpha_curve_of(&mat.data) {
                Some(AlphaCurve::Bezier(keys)) => {
                    alpha = Some(AlphaCurve::Bezier(keys));
                    break;
                }
                Some(AlphaCurve::Unsupported(r)) => {
                    reason.get_or_insert(r);
                }
                None => continue,
            }
        }
        let alpha = match (alpha, reason) {
            (Some(c), _) => c,
            (None, Some(r)) => return Err(LogoProblem::AlphaUnsupported(r)),
            (None, None) => return Err(LogoProblem::NoAlphaTrack),
        };

        Ok(Self {
            sequences,
            quad,
            planar,
            texture,
            alpha,
        })
    }

    /// The `Birth` sequence, if the model has one.
    #[must_use]
    pub fn birth(&self) -> Option<&LogoSequence> {
        self.sequences.iter().find(|s| s.name == "Birth")
    }

    /// The quad's width in the model's own units.
    #[must_use]
    pub fn quad_width(&self) -> f32 {
        self.quad.2 - self.quad.0
    }

    /// The quad's height in the model's own units.
    #[must_use]
    pub fn quad_height(&self) -> f32 {
        self.quad.3 - self.quad.1
    }

    /// The artwork's aspect ratio.
    #[must_use]
    #[cfg(test)]
    pub fn quad_aspect(&self) -> f32 {
        if self.quad_height() == 0.0 {
            1.0
        } else {
            self.quad_width() / self.quad_height()
        }
    }

    /// The sprite's alpha `ms` into the entrance, or `None` when the model's
    /// curve could not be read.
    ///
    /// Only the `Birth` segment is played: the keys after it belong to
    /// `Birth Alternate` and `Death`, and following the raw track past `Birth`
    /// would fade the logo back out. At and past the end of `Birth` the value is
    /// the segment's end value, which is the `Stand` the entrance holds.
    #[must_use]
    pub fn alpha_at_ms(&self, ms: f64) -> Option<f32> {
        self.sequence_alpha("Birth", ms)
    }

    pub fn sequence_alpha(&self, name: &str, elapsed_ms: f64) -> Option<f32> {
        if !elapsed_ms.is_finite() {
            return None;
        }
        let sequence = self
            .sequences
            .iter()
            .find(|sequence| sequence.name == name)?;
        sequence.end.checked_sub(sequence.start)?;
        match &self.alpha {
            AlphaCurve::Bezier(keys) => {
                let first = keys.iter().position(|key| key.frame == sequence.start)?;
                sampled_alpha(
                    keys.get(first..)?,
                    sequence.start,
                    sequence.end,
                    f64::from(sequence.start) + elapsed_ms,
                )
            }
            AlphaCurve::Unsupported(_) => None,
        }
    }
}

/// Evaluates the cubic Bézier segment spanning `start..end` at `ms`.
///
/// The control values are the two keys' values and the outgoing/incoming
/// tangents, and the parameter is normalised over the segment. This is the
/// Bézier the format stores, not a linear ramp and not the Hermite form the
/// crate's bone sampler uses — see the module note in `menu-panel.md`.
#[must_use]
pub fn sampled_alpha(keys: &[BezierKey], start: u32, end: u32, ms: f64) -> Option<f32> {
    let k0 = keys.first()?;
    let k1 = keys.get(1)?;
    // The segment must be the one the sequence names; otherwise reading it would
    // be applying a curve to a window it does not cover.
    if k0.frame != start || k1.frame != end {
        return None;
    }
    if ms <= f64::from(k0.frame) {
        return Some(k0.value);
    }
    if ms >= f64::from(k1.frame) {
        return Some(k1.value);
    }
    let span = f64::from(k1.frame) - f64::from(k0.frame);
    if span == 0.0 {
        return Some(k0.value);
    }
    let t = ((ms - f64::from(k0.frame)) / span) as f32;
    Some(bezier(k0.value, k0.out_tan, k1.in_tan, k1.value, t))
}

/// The cubic Bézier through control values `p0, p1, p2, p3` at `t`.
#[must_use]
pub fn bezier(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    let u = 1.0 - t;
    u * u * u * p0 + 3.0 * u * u * t * p1 + 3.0 * u * t * t * p2 + t * t * t * p3
}

/// Reads the `VRTX` array out of a geoset's entry bytes.
fn vrtx_of(data: &[u8]) -> Option<Vec<[f32; 3]>> {
    let at = data.windows(4).position(|w| w == b"VRTX")?;
    let count = u32::from_le_bytes(data.get(at + 4..at + 8)?.try_into().ok()?) as usize;
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let base = at + 8 + i * 12;
        let b = data.get(base..base + 12)?;
        out.push([
            f32::from_le_bytes(b[0..4].try_into().ok()?),
            f32::from_le_bytes(b[4..8].try_into().ok()?),
            f32::from_le_bytes(b[8..12].try_into().ok()?),
        ]);
    }
    Some(out)
}

/// `minX, minY, maxX, maxY` over a vertex list.
fn bounds_of(verts: &[[f32; 3]]) -> (f32, f32, f32, f32) {
    let mut b = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for v in verts {
        b.0 = b.0.min(v[0]);
        b.1 = b.1.min(v[1]);
        b.2 = b.2.max(v[0]);
        b.3 = b.3.max(v[1]);
    }
    b
}

/// Reads the `KMTA` alpha track out of a material's entry bytes.
///
/// Returns `None` when the material carries no `KMTA` at all — silence, not a
/// problem — and `Some(AlphaCurve::Unsupported(reason))` when a block is present
/// but this module will not use it. The reason is the block's own and is kept
/// verbatim.
///
/// Keys are `frame(u32)`, `value(f32)`, then `in_tan(f32)`, `out_tan(f32)` per
/// key. Before anything is allocated the declared count is checked against the
/// bytes that remain, so a corrupt count cannot ask for a huge allocation; then
/// each key is required to be finite and its frame strictly greater than the one
/// before, because an unsorted or duplicated frame would silently reorder or
/// collapse the curve.
fn alpha_curve_of(data: &[u8]) -> Option<AlphaCurve> {
    let at = data.windows(4).position(|w| w == b"KMTA")?;
    let read_u32 = |o: usize| -> Option<u32> {
        data.get(at + o..at + o + 4)
            .and_then(|b| b.try_into().ok())
            .map(u32::from_le_bytes)
    };
    let read_f32 = |o: usize| -> Option<f32> {
        data.get(at + o..at + o + 4)
            .and_then(|b| b.try_into().ok())
            .map(f32::from_le_bytes)
    };
    let unsupported = |m: String| Some(AlphaCurve::Unsupported(m));

    // The block header: magic, count, interpolation, global sequence id. The
    // global sequence id is at +12; `NO_ID` means the track is driven by local
    // frames, which is what a `Birth` clock needs.
    let (Some(count), Some(interp), Some(global)) = (read_u32(4), read_u32(8), read_u32(12)) else {
        return unsupported("KMTA header truncated".to_string());
    };
    if interp != 3 {
        return unsupported(format!("interpolation {interp} is not cubic Bezier"));
    }
    if global != NO_ID {
        return unsupported(format!(
            "global sequence {global} drives the alpha, so Birth has no local clock"
        ));
    }
    if count < 2 {
        return unsupported(format!("KMTA declares {count} keys"));
    }
    // Every key is four `f32`-sized fields. Check the declared count against the
    // bytes actually present *before* reserving, so a bad count is a report and
    // not an allocation.
    let Some(body) = data.len().checked_sub(at + 16) else {
        return unsupported("KMTA header runs past the record".to_string());
    };
    let Some(needed) = (count as usize).checked_mul(16) else {
        return unsupported(format!("KMTA count {count} overflows"));
    };
    if needed > body {
        return unsupported(format!(
            "KMTA declares {count} keys ({needed} bytes) but {body} remain"
        ));
    }

    let mut keys = Vec::with_capacity(count as usize);
    let mut cursor = 16;
    for _ in 0..count {
        let (Some(frame), Some(value), Some(in_tan), Some(out_tan)) = (
            read_u32(cursor),
            read_f32(cursor + 4),
            read_f32(cursor + 8),
            read_f32(cursor + 12),
        ) else {
            return unsupported("KMTA key truncated".to_string());
        };
        if !value.is_finite() || !in_tan.is_finite() || !out_tan.is_finite() {
            return unsupported(format!("KMTA key at frame {frame} is not finite"));
        }
        if let Some(prev) = keys.last().map(|k: &BezierKey| k.frame)
            && frame <= prev
        {
            return unsupported(format!("KMTA frames out of order: {prev} then {frame}"));
        }
        keys.push(BezierKey {
            frame,
            value,
            in_tan,
            out_tan,
        });
        cursor += 16;
    }
    Some(AlphaCurve::Bezier(keys))
}

/// The selected model's parsed data and the texture its TEXS record names.
#[derive(Debug, Clone)]
pub struct LogoAssets {
    pub model_path: String,
    pub data: LogoData,
    pub texture: Vec<u8>,
}

/// Resolve the explicit edition's skin binding, then use its binary transport.
pub fn logo_model_path(
    skin: &crate::menu_skin::SkinTable,
    tft: bool,
) -> Result<String, crate::menu_skin::SkinError> {
    let key = if tft {
        "MainMenuLogo_V1"
    } else {
        "MainMenuLogo_V0"
    };
    let path = skin.unique_file(key)?.file;
    Ok(match path.get(path.len().saturating_sub(4)..) {
        Some(ext) if ext.eq_ignore_ascii_case(".mdl") => {
            format!("{}.mdx", &path[..path.len() - 4])
        }
        _ => path,
    })
}

/// Read the current FDF's anchor without supplying an authored coordinate.
pub fn logo_anchor(fdf: &crate::fdf::Fdf) -> Result<(f32, f32), crate::fdf::FdfError> {
    use crate::fdf::{FdfError, Point};
    let name = LOGO_FRAME;
    let frame = fdf
        .frame(name)
        .ok_or_else(|| FdfError::UnknownFrame(name.into()))?;
    match frame.props.set_points.as_slice() {
        [point]
            if point.point == Point::TopLeft
                && point.relative == "MainMenuFrame"
                && point.relative_point == Point::TopLeft
                && point.x.is_finite()
                && point.y.is_finite() =>
        {
            Ok((point.x, point.y))
        }
        points => Err(FdfError::Unsupported {
            frame: name.into(),
            property: "logo SetPoint",
            value: format!("{points:?}"),
        }),
    }
}

fn load_logo_with(
    skin: &crate::menu_skin::SkinTable,
    tft: bool,
    mut read: impl FnMut(&str) -> Option<Vec<u8>>,
) -> Result<LogoAssets, String> {
    let model_path = logo_model_path(skin, tft).map_err(|e| e.to_string())?;
    let model = read(&model_path).ok_or_else(|| format!("missing logo model {model_path:?}"))?;
    let data = LogoData::parse(&model).map_err(|e| format!("logo model {model_path:?}: {e}"))?;
    let texture = read(&data.texture).ok_or_else(|| {
        format!(
            "logo model {model_path:?}: missing texture {:?}",
            data.texture
        )
    })?;
    Ok(LogoAssets {
        model_path,
        data,
        texture,
    })
}

/// Load canonical selected paths first; old served aliases are compatibility only.
pub fn load_logo(
    desktop: Option<&std::path::Path>,
    skin: &crate::menu_skin::SkinTable,
    tft: bool,
) -> Result<LogoAssets, String> {
    load_logo_with(skin, tft, |path| {
        crate::mainmenu::art_bytes(desktop, path).or_else(|| {
            let alias = if path.eq_ignore_ascii_case(LOGO_MODEL) {
                Some(LOGO_MODEL_SERVED)
            } else if path.eq_ignore_ascii_case(LOGO_TEXTURE) {
                Some(LOGO_TEXTURE_SERVED)
            } else {
                None
            };
            alias.and_then(crate::mainmenu::from_served)
        })
    })
}

#[must_use]
/// Marker for the logo sprite.
#[derive(Component, Debug)]
pub struct MenuLogo;

/// How far into the entrance the logo is.
///
/// Milliseconds as `f64`: a frame shorter than a millisecond must not be
/// truncated, or a fast machine would finish the 500 ms fade early and a slow
/// one late, differently each run.
#[derive(Resource, Debug, Default)]
pub struct LogoEntrance {
    /// Milliseconds since the sprite was shown.
    pub elapsed_ms: f64,
    /// The parsed art, once loaded.
    pub data: Option<LogoData>,
}

/// Advances the entrance and writes the sprite's alpha from the parsed curve.
///
/// Placement is not this system's business: the caller positions the sprite. Any
/// `MenuLogo` entity present gets the tint.
pub fn advance_logo(
    time: Res<Time<Real>>,
    mut entrance: ResMut<LogoEntrance>,
    panels: Option<Res<crate::menu_sprites::MenuPanels>>,
    mut sprites: Query<&mut ImageNode, With<MenuLogo>>,
) {
    // Read what the clock needs before touching it: holding a borrow of
    // `entrance.data` across the mutation below would not compile, and taking
    // the frame delta first keeps the two reads and one write in order.
    let Some(birth) = entrance
        .data
        .as_ref()
        .and_then(|d| d.birth())
        .and_then(|s| s.end.checked_sub(s.start).map(f64::from))
    else {
        return;
    };
    if entrance.elapsed_ms < birth {
        entrance.elapsed_ms += time.delta().as_secs_f64() * 1000.0;
    }
    let Some(alpha) = entrance.data.as_ref().and_then(|data| {
        match panels.as_ref().and_then(|panels| panels.logo_animation()) {
            Some((sequence, elapsed)) => {
                data.sequence_alpha(sequence, elapsed.as_secs_f64() * 1000.0)
            }
            None => data.alpha_at_ms(entrance.elapsed_ms),
        }
    }) else {
        return;
    };
    for mut node in &mut sprites {
        node.color = Color::srgba(1.0, 1.0, 1.0, alpha);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequence_alpha_uses_named_exit_keys_and_elapsed_time() {
        let key = |frame, value| BezierKey {
            frame,
            value,
            in_tan: value,
            out_tan: value,
        };
        let data = LogoData {
            sequences: vec![
                LogoSequence {
                    name: "Birth".into(),
                    start: 10,
                    end: 110,
                },
                LogoSequence {
                    name: "Death".into(),
                    start: 500,
                    end: 800,
                },
            ],
            quad: (0.0, 0.0, 1.0, 1.0),
            planar: true,
            texture: "synthetic.blp".into(),
            alpha: AlphaCurve::Bezier(vec![
                key(10, 0.0),
                key(110, 1.0),
                key(500, 1.0),
                key(800, 0.0),
            ]),
        };
        assert_eq!(data.alpha_at_ms(50.0), Some(0.5));
        assert_eq!(data.alpha_at_ms(500.0), Some(1.0));
        assert_eq!(data.sequence_alpha("Death", 0.0), Some(1.0));
        assert_eq!(data.sequence_alpha("Death", 150.0), Some(0.5));
        assert_eq!(data.sequence_alpha("Death", 400.0), Some(0.0));
        assert_eq!(data.sequence_alpha("Absent", 0.0), None);
        assert_eq!(data.sequence_alpha("Death", f64::NAN), None);
        assert_eq!(data.sequence_alpha("Death", f64::INFINITY), None);
        let mut malformed = data;
        malformed.sequences[1].end = 499;
        assert_eq!(malformed.sequence_alpha("Death", 0.0), None);
    }

    /// The two entry keys exactly as the shipped model stores them.
    fn shipped_entry_keys() -> Vec<BezierKey> {
        vec![
            BezierKey {
                frame: 0,
                value: 0.0,
                in_tan: 0.0,
                out_tan: 0.533_333,
            },
            BezierKey {
                frame: 500,
                value: 1.0,
                in_tan: 0.866_667,
                out_tan: 1.222_22,
            },
        ]
    }

    /// The Bézier segment, at its ends and its middle.
    ///
    /// The middle is the whole point: a linear ramp gives `0.5`, the Hermite form
    /// gives `0.458`, and the model's own control values give `0.65`. A test
    /// that only checked the ends would pass against both wrong readings.
    #[test]
    fn the_entrance_is_the_models_bezier_not_a_ramp() {
        let keys = shipped_entry_keys();
        assert_eq!(sampled_alpha(&keys, 0, 500, 0.0), Some(0.0));
        assert_eq!(sampled_alpha(&keys, 0, 500, 500.0), Some(1.0));
        let mid = sampled_alpha(&keys, 0, 500, 250.0).expect("midpoint");
        assert!(
            (mid - 0.65).abs() < 0.005,
            "midpoint {mid}, expected the control-value Bezier's 0.65"
        );
        assert!(
            (mid - 0.5).abs() > 0.1,
            "distinguishable from a linear ramp"
        );
        assert!((mid - 0.458).abs() > 0.1, "and from the Hermite reading");
    }

    /// The curve is read over the sequence it names, and only that window.
    #[test]
    fn the_segment_must_match_the_sequence() {
        let keys = shipped_entry_keys();
        assert_eq!(sampled_alpha(&keys, 0, 400, 200.0), None, "wrong window");
        assert_eq!(sampled_alpha(&[keys[0]], 0, 500, 200.0), None, "one key");
        assert_eq!(sampled_alpha(&[], 0, 500, 200.0), None, "no keys");
    }

    /// Builds a `KMTA` block with the given header and raw key bytes.
    fn kmta(count: u32, interp: u32, global: u32, keys: &[u8]) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(b"KMTA");
        b.extend_from_slice(&count.to_le_bytes());
        b.extend_from_slice(&interp.to_le_bytes());
        b.extend_from_slice(&global.to_le_bytes());
        b.extend_from_slice(keys);
        b
    }

    /// One key as the model stores it.
    fn key_bytes(frame: u32, value: f32, in_tan: f32, out_tan: f32) -> [u8; 16] {
        let mut k = [0u8; 16];
        k[0..4].copy_from_slice(&frame.to_le_bytes());
        k[4..8].copy_from_slice(&value.to_le_bytes());
        k[8..12].copy_from_slice(&in_tan.to_le_bytes());
        k[12..16].copy_from_slice(&out_tan.to_le_bytes());
        k
    }

    /// A material with no `KMTA` is silent, and an unimplemented interpolation is
    /// reported — the two are different outcomes and stay different.
    #[test]
    fn silence_and_a_reason_are_distinguished() {
        assert_eq!(alpha_curve_of(b"no tracks here"), None, "silence");
        assert_eq!(
            alpha_curve_of(&kmta(2, 1, NO_ID, &[0u8; 32])),
            Some(AlphaCurve::Unsupported(
                "interpolation 1 is not cubic Bezier".to_string()
            ))
        );
    }

    /// A global-sequence clock cannot serve as the local `Birth` clock.
    #[test]
    fn a_global_sequence_clock_is_reported() {
        assert_eq!(
            alpha_curve_of(&kmta(2, 3, 4, &[0u8; 32])),
            Some(AlphaCurve::Unsupported(
                "global sequence 4 drives the alpha, so Birth has no local clock".to_string()
            ))
        );
    }

    /// A declared count larger than the bytes can hold is reported, not reserved.
    #[test]
    fn an_oversized_count_is_reported_before_allocating() {
        // Claims 4 million keys but carries two.
        let got = alpha_curve_of(&kmta(4_000_000, 3, NO_ID, &[0u8; 32]));
        assert!(
            matches!(&got, Some(AlphaCurve::Unsupported(r)) if r.contains("remain")),
            "got {got:?}"
        );
    }

    /// Non-finite values and out-of-order frames are rejected.
    #[test]
    fn non_finite_and_unsorted_keys_are_rejected() {
        let mut keys = Vec::new();
        keys.extend_from_slice(&key_bytes(0, 0.0, 0.0, 0.5));
        keys.extend_from_slice(&key_bytes(500, f32::NAN, 0.8, 1.0));
        assert!(
            matches!(alpha_curve_of(&kmta(2, 3, NO_ID, &keys)), Some(AlphaCurve::Unsupported(r)) if r.contains("not finite")),
            "a NaN key must be reported"
        );

        let mut keys = Vec::new();
        keys.extend_from_slice(&key_bytes(500, 0.0, 0.0, 0.5));
        keys.extend_from_slice(&key_bytes(500, 1.0, 0.8, 1.0));
        assert!(
            matches!(alpha_curve_of(&kmta(2, 3, NO_ID, &keys)), Some(AlphaCurve::Unsupported(r)) if r.contains("out of order")),
            "a duplicate frame must be reported"
        );
    }

    /// A Bézier `KMTA` block parses to exactly the shipped tangents.
    #[test]
    fn a_bezier_kmta_block_parses_its_keys() {
        let mut keys = Vec::new();
        for k in shipped_entry_keys() {
            keys.extend_from_slice(&key_bytes(k.frame, k.value, k.in_tan, k.out_tan));
        }
        assert_eq!(
            alpha_curve_of(&kmta(2, 3, NO_ID, &keys)),
            Some(AlphaCurve::Bezier(shipped_entry_keys()))
        );
    }

    /// The clock keeps sub-millisecond frames.
    ///
    /// A hundredth-of-a-millisecond step is below what a `u32` of milliseconds
    /// can represent; accumulating it must not round to nothing.
    #[test]
    fn the_clock_preserves_sub_millisecond_steps() {
        let mut e = LogoEntrance::default();
        for _ in 0..500 {
            e.elapsed_ms += 0.01;
        }
        assert!(e.elapsed_ms > 4.9, "got {}", e.elapsed_ms);
    }

    #[test]
    fn logo_paths_and_anchor_come_from_the_selected_inputs() {
        let skin = crate::menu_skin::SkinTable::parse(
            "[Default]\nMainMenuLogo_V0=Custom\\First.MDL\nMainMenuLogo_V1=Custom\\Second.mdx\n",
        )
        .unwrap();
        assert_eq!(logo_model_path(&skin, false).unwrap(), r"Custom\First.mdx");
        assert_eq!(logo_model_path(&skin, true).unwrap(), r"Custom\Second.mdx");
        let fdf = crate::fdf::Fdf::parse("Frame \"SPRITE\" \"WarCraftIIILogo\" {\nSetPoint TOPLEFT,\"MainMenuFrame\",TOPLEFT,0.217,-0.063,\n}\n").unwrap();
        assert_eq!(logo_anchor(&fdf).unwrap(), (0.217, -0.063));
        let duplicate = crate::menu_skin::SkinTable::parse(
            "[Default]\nMainMenuLogo_V0=a.mdl\nMainMenuLogo_V0=b.mdl\n",
        )
        .unwrap();
        assert!(logo_model_path(&duplicate, false).is_err());
        assert!(logo_model_path(&duplicate, true).is_err());
        let wrong = crate::fdf::Fdf::parse("Frame \"SPRITE\" \"WarCraftIIILogo\" {\nSetPoint TOPRIGHT,\"MainMenuFrame\",TOPLEFT,0.2,0.1,\n}\n").unwrap();
        assert!(logo_anchor(&wrong).is_err());
    }

    #[test]
    fn logo_model_failures_keep_the_selected_path() {
        let skin =
            crate::menu_skin::SkinTable::parse("[Default]\nMainMenuLogo_V0=custom.mdl\n").unwrap();
        let mut reads = Vec::new();
        let failure = load_logo_with(&skin, false, |path| {
            reads.push(path.to_string());
            Some(b"invalid model".to_vec())
        })
        .unwrap_err();
        assert_eq!(reads, ["custom.mdx"]);
        assert!(failure.contains("custom.mdx"));
        assert!(failure.contains("model did not parse"));
    }

    /// `bounds_of` over the measured quad returns the measured bounds.
    #[test]
    fn bounds_are_min_and_max_not_first_and_last() {
        let v = vec![
            [-0.112_09, 0.053_3, 0.0],
            [-0.112_09, -0.058_2, 0.0],
            [0.111_46, 0.053_3, 0.0],
            [0.111_46, -0.058_2, 0.0],
        ];
        let b = bounds_of(&v);
        assert!((b.0 - -0.112_09).abs() < 1e-6);
        assert!((b.1 - -0.058_2).abs() < 1e-6);
        assert!((b.2 - 0.111_46).abs() < 1e-6);
        assert!((b.3 - 0.053_3).abs() < 1e-6);
    }

    /// Against the real model: the quad, the sequences and the texture this
    /// module parses.
    ///
    /// Ignored because it needs a local Warcraft III install — the fixture is a
    /// shipped asset and this repository does not commit those. Run it with
    /// `cargo test -p peacecraft3 -- --ignored`, setting
    /// `PEACECRAFT3_LEGACY_DIR` if the install is not at the repository path.
    ///
    /// It **fails** when the assets are absent rather than returning green: a
    /// skipped check that reports success is worse than no check.
    #[test]
    #[ignore = "needs a local Warcraft III install; run with --ignored and PEACECRAFT3_LEGACY_DIR set"]
    fn the_shipped_model_parses_to_the_measured_values() {
        let mut roots: Vec<std::path::PathBuf> = Vec::new();
        if let Some(p) = std::env::var_os("PEACECRAFT3_LEGACY_DIR") {
            roots.push(std::path::PathBuf::from(p));
        }
        if let Some(p) = std::env::var_os("PEACECRAFT3_INSTALL") {
            roots.push(std::path::PathBuf::from(p));
        }
        if let Some(p) = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
        {
            roots.push(p.join("wc3/game/app"));
        }
        let assets = roots
            .iter()
            .find_map(|r| {
                let bytes =
                    crate::mainmenu::art_bytes(Some(r.as_path()), crate::mainmenu::SKIN_TABLE)?;
                let text = std::str::from_utf8(&bytes).ok()?;
                let skin = crate::menu_skin::SkinTable::parse(text).ok()?;
                load_logo(Some(r.as_path()), &skin, true).ok()
            })
            .unwrap_or_else(|| panic!("no Warcraft III install found; searched {roots:?}"));

        let data = assets.data;

        assert_eq!(data.texture, LOGO_TEXTURE);
        assert!(data.planar, "every vertex shares one z");
        let (a, b, c, d) = LOGO_QUAD_MEASURED;
        assert!((data.quad.0 - a).abs() < 1e-5, "minX {:?}", data.quad);
        assert!((data.quad.1 - b).abs() < 1e-5, "minY {:?}", data.quad);
        assert!((data.quad.2 - c).abs() < 1e-5, "maxX {:?}", data.quad);
        assert!((data.quad.3 - d).abs() < 1e-5, "maxY {:?}", data.quad);
        assert!((data.quad_aspect() - 2.0049).abs() < 0.001);

        let birth = data.birth().expect("Birth");
        assert_eq!(birth.start, 0);
        assert_eq!(f64::from(birth.duration()), LOGO_BIRTH_MS_EXPECTED);
        assert!(
            data.sequences
                .iter()
                .any(|s| s.name == "Stand" && s.duration() == 55_000)
        );
        assert!(
            data.sequences
                .iter()
                .any(|s| s.name == "Death" && s.duration() == 500)
        );

        let AlphaCurve::Bezier(keys) = &data.alpha else {
            panic!("expected a Bezier alpha curve, got {:?}", data.alpha);
        };
        assert_eq!(keys[0], shipped_entry_keys()[0]);
        assert_eq!(keys[1], shipped_entry_keys()[1]);
        assert!((data.alpha_at_ms(250.0).expect("midpoint") - 0.65).abs() < 0.005);
        // Past Birth the entrance holds rather than following the raw track.
        assert_eq!(data.alpha_at_ms(1333.0), Some(1.0));
        assert_eq!(data.sequence_alpha("Death", 0.0), Some(1.0));
        let death = data
            .sequences
            .iter()
            .find(|sequence| sequence.name == "Death")
            .unwrap();
        assert_eq!(
            data.sequence_alpha("Death", f64::from(death.duration())),
            Some(0.0)
        );
    }
}

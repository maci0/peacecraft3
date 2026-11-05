//! Texture Quality policy derived from the Legacy executable's Options
//! write routine (`0x538440..0x53882f`, `Warcraft III.exe` SHA-256
//! `3f2ed012…b959d59a`, VERSIONINFO 1.29.2.9231).
//!
//! That routine writes `texquality` unchanged, then derives two further
//! preference words from the same ordinal `q`:
//!
//! * `miplevel` (numeric id `0x0a`): `q == 0` writes `1`; every other
//!   choice writes `0`. Decoded from the branch chain at `0x53865c`.
//! * `texcolordepth` (numeric id `0x0b`): `q == 0, 1, 2` write `0x10`;
//!   `q == 3` writes `0x20`. Decoded from the branch chain at `0x538681`.
//!
//! The word magnitudes (`1` vs `0`, `0x10` vs `0x20`) are not interpreted
//! here as a BLP mip index or a bit depth; see the loader in this crate,
//! which treats the first as an index into the authored mip chain and the
//! second as the per-pixel byte width to slice. Neither mapping is an
//! anisotropy/sampler setting.
//!
//! The ordinal domain is the authored popup's. A value outside it refuses
//! rather than clamping or assuming a default.

/// The four authored Texture Quality choices, by ordinal.
pub const TEXTURE_QUALITY_COUNT: usize = 4;

/// Native-mapped texture policy for one ordinal.
///
/// `mip_level` is the index into the authored BLP mip chain to decode as
/// the texture base (0 = top level), derived from `miplevel`.
/// `color_depth_bytes` is the per-pixel byte width to retain from the
/// decoded mip, derived from `texcolordepth` (`0x10` -> 4, `0x20` -> 8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureQuality {
    /// Index into the authored mip chain; 0 is the top level.
    pub mip_level: usize,
    /// Retain this many bytes per pixel from the decoded mip.
    pub color_depth_bytes: usize,
    /// The `texcolordepth` preference word this quality was derived from.
    pub texcolordepth_word: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PolicyError {
    /// The ordinal is outside the authored four choices.
    OrdinalOutOfRange {
        /// The supplied ordinal.
        ordinal: u32,
    },
}

impl std::fmt::Display for PolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OrdinalOutOfRange { ordinal } => {
                write!(f, "texture quality ordinal {ordinal} is outside the authored {TEXTURE_QUALITY_COUNT} choices")
            }
        }
    }
}

impl std::error::Error for PolicyError {}

/// Map an authored `texquality` ordinal to its native-derived mip/depth
/// pair. Both derived words come from the branch chains in the write
/// routine; this is deliberately not a table lookup for the words
/// themselves, so a change in the native branches changes the words.
pub fn texture_quality_for_ordinal(ordinal: u32) -> Result<TextureQuality, PolicyError> {
    if ordinal as usize >= TEXTURE_QUALITY_COUNT {
        return Err(PolicyError::OrdinalOutOfRange { ordinal });
    }
    // miplevel (0x53865c): first choice writes 1, the rest write 0.
    let mip_word: u32 = if ordinal == 0 { 1 } else { 0 };
    // texcolordepth (0x538681): first three choices write 0x10, the last
    // writes 0x20.
    let depth_word: u32 = if ordinal == 3 { 0x20 } else { 0x10 };
    Ok(TextureQuality {
        mip_level: mip_word as usize,
        color_depth_bytes: match depth_word {
            0x10 => 4,
            0x20 => 8,
            other => {
                // Unreachable given the branch above; treat as a hard
                // refusal rather than guessing a width.
                return Err(PolicyError::OrdinalOutOfRange { ordinal: other });
            }
        },
        texcolordepth_word: depth_word,
    })
}

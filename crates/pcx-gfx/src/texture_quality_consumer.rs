//! Texture Quality consumer for the menu layer: ordinal + branch windows +
//! authored BLP bytes -> the single quality-named mip as a Bevy image, or a
//! retained-texture refusal. The menu layer (owner of the FDF popup domain
//! and of the selected-client image) supplies inputs; this module never
//! reads the client image itself and never owns UI state. See
//! `texture_quality` for the policy derivation and the depth evidence note
//! `docs/re-notes/texture-quality-depth16-evidence.md`: the 16-bit channel
//! format is unresolved, so the 2-byte depth refuses and the caller keeps
//! showing the previous texture.

use crate::texture_quality::{PolicyError, TextureQuality, derive_texture_quality};
use crate::{LoadedTexture, one_mip, viewer::rgba_image};
use bevy::image::Image;
use bevy::render::render_resource::TextureFormat;

/// Why a quality replacement was refused. Every variant retains the current
/// texture; none carries a replacement image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The two branch windows are not the reviewed opcode shapes.
    MalformedBranch,
    /// The ordinal is at or beyond the authored popup domain.
    OrdinalOutOfRange { ordinal: u32, domain: usize },
    /// The derived `texcolordepth` word is not a validated bit depth (16/32).
    UnsupportedDepth { word: u32 },
    /// The 16-bit channel layout (RGB565 / ARGB1555 / ARGB4444 / ...) is
    /// not established by the shipped bytes, so the selected 2-byte depth
    /// has no RGBA8 store. Retains the previous texture.
    Depth16Unresolved { word: u32 },
    /// The selected authored mip level is absent, invalid, undecodable, or
    /// exceeds the caller's byte budget.
    MipUnavailable,
}

impl From<PolicyError> for Refusal {
    fn from(error: PolicyError) -> Self {
        match error {
            PolicyError::MalformedBranch => Self::MalformedBranch,
            PolicyError::OrdinalOutOfRange { ordinal, domain } => {
                Self::OrdinalOutOfRange { ordinal, domain }
            }
            PolicyError::UnsupportedDepth { word } => Self::UnsupportedDepth { word },
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MalformedBranch => {
                write!(f, "texture quality branch window is malformed")
            }
            Self::OrdinalOutOfRange { ordinal, domain } => {
                write!(
                    f,
                    "texture quality ordinal {ordinal} is outside the authored {domain} choices"
                )
            }
            Self::UnsupportedDepth { word } => {
                write!(f, "texcolordepth word {word} is not a validated bit depth")
            }
            Self::Depth16Unresolved { word } => write!(
                f,
                "texture quality depth word {word} (16-bit colour) has no established channel format; previous texture retained"
            ),
            Self::MipUnavailable => {
                write!(
                    f,
                    "texture quality selected mip is unavailable; previous texture retained"
                )
            }
        }
    }
}

impl std::error::Error for Refusal {}

/// A successful replacement: the quality-selected mip as a Bevy image.
#[derive(Debug, Clone)]
pub struct Replacement {
    /// The decoded quality-selected level (single image, no mip chain).
    pub texture: LoadedTexture,
    /// The GPU image built from `texture` via the existing viewer upload.
    pub image: Image,
}

impl Replacement {
    /// The validated byte budget check Bevy uploaders already use.
    pub fn bytes(&self) -> Option<usize> {
        self.texture.rgba_bytes()
    }
}

/// Apply `ordinal` (an authored `TextureQualityMenu` choice index) to one
/// BLP's bytes. `domain` is the caller-measured popup length; `miplevel_branch`
/// and `texcolordepth_branch` are the windows read from the selected-client
/// image. On any refusal the returned error is the user-visible state and
/// the caller retains its current texture — no partial image is produced.
///
/// A refusal is never silently coerced: depth 16 (2 bytes/pixel) refuses as
/// [`Refusal::Depth16Unresolved`] instead of inventing a pixel format.
pub fn replacement(
    bytes: &[u8],
    source: &str,
    miplevel_branch: &[u8],
    texcolordepth_branch: &[u8],
    ordinal: u32,
    domain: usize,
    max_rgba_bytes: usize,
) -> Result<Replacement, Refusal> {
    let policy = derive_texture_quality(miplevel_branch, texcolordepth_branch, ordinal, domain)
        .map_err(Refusal::from)?;
    if policy.color_depth_bytes == 2 {
        return Err(Refusal::Depth16Unresolved {
            word: policy.texcolordepth_word,
        });
    }
    let texture =
        one_mip(bytes, source, &policy, max_rgba_bytes).map_err(|_| Refusal::MipUnavailable)?;
    let mut image = rgba_image(&texture);
    image.texture_descriptor.format = TextureFormat::Rgba8Unorm;
    Ok(Replacement { texture, image })
}

/// Inputs for [`retain_on_refusal`], grouped so the call stays readable.
pub struct Request<'a> {
    pub bytes: &'a [u8],
    pub source: &'a str,
    pub miplevel_branch: &'a [u8],
    pub texcolordepth_branch: &'a [u8],
    pub ordinal: u32,
    pub domain: usize,
    pub max_rgba_bytes: usize,
}

/// Retain-on-refusal application against a current texture.
///
/// Called by the integrator with the current `LoadedTexture`; on `Err` the
/// current value is returned unchanged and the refusal is the user-visible
/// notice. This is the exact retain-old-texture contract: the function
/// never mutates or drops the current texture on refusal.
pub fn retain_on_refusal(
    current: LoadedTexture,
    request: Request<'_>,
) -> Result<Replacement, (LoadedTexture, Refusal)> {
    replacement(
        request.bytes,
        request.source,
        request.miplevel_branch,
        request.texcolordepth_branch,
        request.ordinal,
        request.domain,
        request.max_rgba_bytes,
    )
    .map_err(|refusal| (current, refusal))
}

/// Derive the policy alone, for status text that needs the resolved words
/// without loading any pixels.
pub fn policy(
    miplevel_branch: &[u8],
    texcolordepth_branch: &[u8],
    ordinal: u32,
    domain: usize,
) -> Result<TextureQuality, Refusal> {
    derive_texture_quality(miplevel_branch, texcolordepth_branch, ordinal, domain)
        .map_err(Refusal::from)
}

/// Loader entry point for the menu layer: forwards to [`retain_on_refusal`]
/// with the existing [`Request`] shape. `domain` is the caller-measured
/// popup length from parsed source assets; `miplevel_branch` and
/// `texcolordepth_branch` are the windows read from the selected-client
/// image. Callers retain the old texture on `Err`.
#[allow(clippy::too_many_arguments)]
pub fn load_from_request(
    current: LoadedTexture,
    request: Request<'_>,
) -> Result<Replacement, (LoadedTexture, Refusal)> {
    retain_on_refusal(current, request)
}

/// Alias kept for the menu hook name used across worktrees.
pub type QualityRequest<'a> = Request<'a>;

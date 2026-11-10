//! Menu-layer adapter for Texture Quality: measures the authored popup
//! domain from the parsed Options FDF and forwards the selection to the
//! existing `pcx_gfx` loader hook. This module owns no policy, no branch
//! parser, no default and no client-image reader: it calls
//! `pcx_play::texture_quality_policy::TextureQualityWindows::read` on the
//! selected client image (checked opcodes/targets/bounds via the private PE
//! helpers) and passes the validated borrowed windows to the consumer. On a
//! 16-bit (2-byte depth) selection the gfx loader refuses with
//! `Refusal::Depth16Unresolved` and the current texture is retained.
//!
//! Shared-menu hook (item 10, not wired here): on a TextureQualityMenu
//! selection, item 10 reads the selected client image (native:
//! `menu_lighting::with_client_image`), measures `domain` with
//! [`popup_domain`], then calls [`apply_from_image`] with the current texture
//! and either swaps in `Replacement::image` or keeps the current texture and
//! shows the returned refusal. `apply_from_image` itself is not called from
//! any screen system in this change; the `allow(dead_code)` below covers the
//! hook until the genuine caller arrives, and that non-wiring is reported
//! explicitly rather than claimed as wired.

#![allow(dead_code)]

use pcx_gfx::texture_quality_consumer::{Refusal, Replacement, load_from_request};
use pcx_play::texture_quality_policy::TextureQualityWindows;

/// Caller-supplied inputs for [`apply_from_image`]. `domain` is measured with
/// [`popup_domain`]; `image` is the selected client image bytes; `source` is
/// the BLP's source label; `max_rgba_bytes` is the caller's byte budget.
pub(crate) struct Inputs<'a> {
    pub(crate) bytes: &'a [u8],
    pub(crate) source: &'a str,
    pub(crate) image: &'a [u8],
    pub(crate) origin: pcx_play::scripts::AssetIdentity,
    pub(crate) ordinal: u32,
    pub(crate) domain: usize,
    pub(crate) max_rgba_bytes: usize,
}

/// Authored `TextureQualityMenu` choice count from the parsed Options FDF
/// (`Widget::Popup` choices), not a copied constant.
pub(crate) fn popup_domain(fdf: &crate::fdf::Fdf) -> Result<usize, String> {
    let widget = crate::menu_screens::widget(fdf, "TextureQualityMenu")
        .map_err(|e| format!("TextureQualityMenu: {e}"))?
        .ok_or_else(|| "TextureQualityMenu: unknown frame".to_string())?;
    let crate::menu_screens::Widget::Popup { choices } = widget else {
        return Err("TextureQualityMenu: not a popup menu".into());
    };
    Ok(choices.len())
}

/// Apply the ordinal via the gfx loader; on refusal the current texture
/// is returned unchanged alongside the user-visible refusal. Branch windows
/// come from the validated `TextureQualityWindows` API, never from
/// caller-supplied lengths or asset values.
pub(crate) fn apply_from_image(
    current: pcx_gfx::LoadedTexture,
    inputs: Inputs<'_>,
) -> Result<Replacement, (pcx_gfx::LoadedTexture, Refusal)> {
    let windows = TextureQualityWindows::read(inputs.image, inputs.origin)
        .map_err(|_e| (current.clone(), Refusal::MalformedBranch))?;
    load_from_request(
        current,
        pcx_gfx::texture_quality_consumer::Request {
            bytes: inputs.bytes,
            source: inputs.source,
            miplevel_branch: windows.miplevel_branch,
            texcolordepth_branch: windows.texcolordepth_branch,
            ordinal: inputs.ordinal,
            domain: inputs.domain,
            max_rgba_bytes: inputs.max_rgba_bytes,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options_fdf(items: &str) -> crate::fdf::Fdf {
        crate::fdf::Fdf::parse(&format!(
            "Frame \"CHOICES\" \"TextureQualityItems\" {{\n{items}\n}}\n\
                 Frame \"POPUPMENU\" \"TextureQualityMenu\" {{\n\
                 PopupMenuFrame \"TextureQualityItems\",\n}}\n"
        ))
        .unwrap()
    }

    #[test]
    fn popup_domain_is_measured_from_the_fdf() {
        let fdf = options_fdf("MenuItem \"LOW\", 1,\nMenuItem \"HIGH\", 1,");
        assert_eq!(popup_domain(&fdf).unwrap(), 2);
        let empty = options_fdf("");
        assert_eq!(popup_domain(&empty).unwrap(), 0);
    }

    #[test]
    fn unreviewed_image_refuses_and_retains_the_current_texture() {
        // The adapter refuses any image whose digest is not the reviewed
        // Legacy image, which a synthetic nonstock fixture can never satisfy;
        // the current texture is retained unchanged.
        let current = pcx_gfx::LoadedTexture {
            width: 1,
            height: 1,
            rgba: vec![1, 2, 3, 4],
            mipmaps: Vec::new(),
            source: "previous.blp".into(),
        };
        let (kept, refusal) = apply_from_image(
            current.clone(),
            Inputs {
                bytes: &[],
                source: "q.blp",
                image: b"MZ synthetic",
                origin: pcx_play::scripts::AssetIdentity {
                    source: "synthetic".into(),
                    edition: "fixture".into(),
                    version: "fixture".into(),
                },
                ordinal: 0,
                domain: 4,
                max_rgba_bytes: 64,
            },
        )
        .unwrap_err();
        assert_eq!(refusal, Refusal::MalformedBranch);
        assert_eq!(kept.source, current.source);
        assert_eq!(kept.rgba, current.rgba);
    }

    #[test]
    #[ignore = "requires PCX_WAR3_EXE naming the reviewed Legacy Warcraft III.exe"]
    fn selected_original_windows_drive_depth16_refusal() {
        // Reviewed-image gate through the narrow API + adapter: ordinal 0 is
        // the 16-bit depth on the reviewed image, so the consumer must refuse
        // with Depth16Unresolved and retain the current texture.
        let image = std::fs::read(std::env::var("PCX_WAR3_EXE").expect("PCX_WAR3_EXE")).unwrap();
        let current = pcx_gfx::LoadedTexture {
            width: 1,
            height: 1,
            rgba: vec![1, 2, 3, 4],
            mipmaps: Vec::new(),
            source: "previous.blp".into(),
        };
        let (kept, refusal) = apply_from_image(
            current.clone(),
            Inputs {
                bytes: &[],
                source: "q.blp",
                image: &image,
                origin: pcx_play::scripts::AssetIdentity {
                    source: std::env::var("PCX_WAR3_EXE").unwrap(),
                    edition: "Legacy target".into(),
                    version: "1.29.2.9231".into(),
                },
                ordinal: 0,
                domain: 4,
                max_rgba_bytes: 64,
            },
        )
        .unwrap_err();
        assert_eq!(kept.source, current.source);
        assert!(
            matches!(refusal, Refusal::Depth16Unresolved { .. }),
            "{refusal:?}"
        );
    }
}

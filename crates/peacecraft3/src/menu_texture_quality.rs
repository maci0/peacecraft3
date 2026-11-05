//! Menu-layer adapter for Texture Quality: measures the authored popup
//! domain from the parsed Options FDF and forwards the selection to the
//! existing `pcx_gfx` loader hook. This module owns no policy, no branch
//! parser, no default and no client-image reader: the caller supplies the
//! two branch windows read from the selected client image and the BLP
//! bytes. On a 16-bit (2-byte depth) selection the gfx loader refuses with
//! `Refusal::Depth16Unresolved` and the current texture is retained.
//!
//! Shared-menu hook (item 10, not wired here): on a TextureQualityMenu
//! selection, item 10 reads the selected client image (native:
//! `menu_lighting::with_client_image`), extracts the two windows with
//! `pcx_play`'s PE section map, measures `domain` with
//! [`popup_domain`], then calls [`apply`] with the current texture and
//! either swaps in `Replacement::image` or keeps the current texture and
//! shows the returned refusal. `apply` itself is not called from any
//! screen system in this change.
//!
//! Missing public API for item 10: `PcplaySections` is private to
//! `pcx-play` (`pub(crate) mod pe_image` in `crates/pcx-play/src/lib.rs`),
//! so the peacecraft3 crate cannot call
//! `PeSections::parse(&bytes)?.va_bytes(va, len)` to obtain the windows.
//! Until that helper is exported, item 10 must supply the windows from a
//! crate that can see them; this adapter takes them as explicit inputs.
//!
//! The adapter is not yet called by any screen system (item 10 owns the
//! wiring), so the allow below covers the hook until then.

#![allow(dead_code)]

use pcx_gfx::texture_quality_consumer::{Refusal, Replacement, load_from_request};

/// Caller-supplied inputs for [`apply`]. `domain` is measured with
/// [`popup_domain`]; the branch windows come from the selected client
/// image; `max_rgba_bytes` is the caller's byte budget.
pub(crate) struct Inputs<'a> {
    pub(crate) bytes: &'a [u8],
    pub(crate) source: &'a str,
    pub(crate) miplevel_branch: &'a [u8],
    pub(crate) texcolordepth_branch: &'a [u8],
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
/// is returned unchanged alongside the user-visible refusal.
pub(crate) fn apply(
    current: pcx_gfx::LoadedTexture,
    inputs: Inputs<'_>,
) -> Result<Replacement, (pcx_gfx::LoadedTexture, Refusal)> {
    load_from_request(
        current,
        pcx_gfx::texture_quality_consumer::Request {
            bytes: inputs.bytes,
            source: inputs.source,
            miplevel_branch: inputs.miplevel_branch,
            texcolordepth_branch: inputs.texcolordepth_branch,
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
        crate::fdf::Fdf::parse(
            &format!(
                "Frame \"CHOICES\" \"TextureQualityItems\" {{\n{items}\n}}\n\
                 Frame \"POPUPMENU\" \"TextureQualityMenu\" {{\n\
                 PopupMenuFrame \"TextureQualityItems\",\n}}\n"
            ),
        )
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
    fn depth16_selection_retains_the_current_texture() {
        // Synthetic nonstock windows (same shape family as the gfx tests):
        // miplevel special 7 at q=0; depth default 32, special 16 at
        // cumulative sub 3, so q=3 derives the 16-bit depth that refuses.
        let miplevel = vec![
            0x33, 0xc9, 0x8b, 0xc7, 0x2b, 0xc1, 0x74, 0x0e, 0x83, 0xe8, 0x01, 0x74, 0x05, 0x83,
            0xe8, 0x01, 0x75, 0x09, 0x33, 0xc9, 0xeb, 0x05, 0xb9, 7, 0, 0, 0, 0x51, 0x6a, 0x0a,
            0x8b, 0xcb, 0xe8, 0, 0, 0, 0,
        ];
        let texcolor = vec![
            0xb8, 32, 0, 0, 0, 0x83, 0xef, 0x00, 0x74, 0x0f, 0x83, 0xef, 0x01, 0x74, 0x0a, 0x83,
            0xef, 0x02, 0x75, 0x05, 0xb8, 16, 0, 0, 0, 0x50, 0x6a, 0x0b, 0x8b, 0xcb, 0xe8, 0, 0,
            0, 0,
        ];
        let current = pcx_gfx::LoadedTexture {
            width: 1,
            height: 1,
            rgba: vec![1, 2, 3, 4],
            mipmaps: Vec::new(),
            source: "previous.blp".into(),
        };
        let (kept, refusal) = apply(
            current.clone(),
            Inputs {
                bytes: &[],
                source: "q.blp",
                miplevel_branch: &miplevel,
                texcolordepth_branch: &texcolor,
                ordinal: 3,
                domain: 4,
                max_rgba_bytes: 64,
            },
        )
        .unwrap_err();
        assert_eq!(refusal, Refusal::Depth16Unresolved { word: 16 });
        assert_eq!(kept.source, current.source);
        assert_eq!(kept.rgba, current.rgba);
    }
}

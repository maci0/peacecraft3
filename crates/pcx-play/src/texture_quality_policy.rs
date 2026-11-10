//! Narrow reviewed-image policy for Texture Quality branch windows.
//!
//! Validates the selected client image against [`menu_lighting::REVIEWED_IMAGE`],
//! extracts the two branch windows (miplevel at `0x53865c`, texcolordepth at
//! `0x538681`) through the private PE helpers, and returns borrowed windows
//! with source context. The whole PE parser is not exported; callers receive
//! only the validated windows or an error. Window lengths are the complete
//! branch sequences the `pcx-gfx` decoder consumes (measured from the reviewed
//! image gate), not caller-supplied guesses.

use crate::{
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::{Sha256Digest, sha256},
    pe_image::PeSections,
    scripts::AssetIdentity,
};

pub const MIPLEVEL_VA: u32 = 0x53865c;
pub const TEXCOLORDEPTH_VA: u32 = 0x538681;
pub const MIPLEVEL_LEN: usize = 0x20;
pub const TEXCOLORDEPTH_LEN: usize = 0x1e;

/// Validated branch windows borrowed from the reviewed image, with provenance.
#[derive(Debug, Clone)]
pub struct TextureQualityWindows<'a> {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    pub miplevel_branch: &'a [u8],
    pub texcolordepth_branch: &'a [u8],
}

#[derive(Debug)]
pub struct Error {
    source: String,
    reason: &'static str,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "texture quality policy {}: {}", self.source, self.reason)
    }
}

impl std::error::Error for Error {}

impl<'a> TextureQualityWindows<'a> {
    /// Validate identity, bound the image, and extract both windows.
    pub fn read(image: &'a [u8], origin: AssetIdentity) -> Result<Self, Error> {
        let fail = |reason| Error {
            source: origin.source.clone(),
            reason,
        };
        if image.len() > 32 * 1024 * 1024 {
            return Err(fail("client image exceeds budget"));
        }
        if [&origin.source, &origin.edition, &origin.version]
            .iter()
            .any(|value| value.trim().is_empty())
        {
            return Err(fail("missing source identity"));
        }
        let digest = sha256(image);
        if digest != REVIEWED_IMAGE {
            return Err(fail(
                "unsupported client image; no texture quality values substituted",
            ));
        }
        let pe = PeSections::parse(image).map_err(|_| fail("invalid PE image"))?;
        let miplevel_branch = pe
            .va_bytes(MIPLEVEL_VA, MIPLEVEL_LEN)
            .ok_or_else(|| fail("missing miplevel branch window"))?;
        let texcolordepth_branch = pe
            .va_bytes(TEXCOLORDEPTH_VA, TEXCOLORDEPTH_LEN)
            .ok_or_else(|| fail("missing texcolordepth branch window"))?;
        Ok(Self {
            origin,
            image_sha256: digest,
            miplevel_branch,
            texcolordepth_branch,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pe_image::tests::{IMAGE_BASE, synthetic_image};

    fn origin() -> AssetIdentity {
        AssetIdentity {
            source: "synthetic-texture-quality".into(),
            edition: "fixture".into(),
            version: "fixture".into(),
        }
    }

    fn image_with_windows() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        // Nonstock branch shapes (same family as pcx-gfx tests): mip special 7,
        // depth default 32, special 16 at cumulative sub 3.
        // Truncated to the exact production window lengths (0x20 / 0x1e),
        // which the decoder consumes as complete branch sequences.
        let miplevel = vec![
            0x33, 0xc9, 0x8b, 0xc7, 0x2b, 0xc1, 0x74, 0x0e, 0x83, 0xe8, 0x01, 0x74, 0x05, 0x83,
            0xe8, 0x01, 0x75, 0x09, 0x33, 0xc9, 0xeb, 0x05, 0xb9, 7, 0, 0, 0, 0x51, 0x6a, 0x0a,
            0x8b, 0xcb,
        ];
        assert_eq!(miplevel.len(), MIPLEVEL_LEN);
        let texcolor = vec![
            0xb8, 32, 0, 0, 0, 0x83, 0xef, 0x00, 0x74, 0x0f, 0x83, 0xef, 0x01, 0x74, 0x0a, 0x83,
            0xef, 0x02, 0x75, 0x05, 0xb8, 16, 0, 0, 0, 0x50, 0x6a, 0x0b, 0x8b, 0xcb,
        ];
        assert_eq!(texcolor.len(), TEXCOLORDEPTH_LEN);
        // Place both windows in one .text section at image-relative offsets so
        // IMAGE_BASE + rva == VA. Section raw bytes are the window bytes padded
        // to cover both.
        let text_rva = MIPLEVEL_VA - IMAGE_BASE;
        let span = (TEXCOLORDEPTH_VA - MIPLEVEL_VA) as usize + TEXCOLORDEPTH_LEN;
        let mut section = vec![0u8; span];
        section[..miplevel.len()].copy_from_slice(&miplevel);
        let depth_at = (TEXCOLORDEPTH_VA - MIPLEVEL_VA) as usize;
        section[depth_at..depth_at + texcolor.len()].copy_from_slice(&texcolor);
        assert_eq!(section.len(), span);
        let _ = text_rva;
        let bytes = synthetic_image(&[(&crate::pe_image::TEXT, text_rva, &section)]);
        (bytes, miplevel, texcolor)
    }

    #[test]
    fn nonstock_fixture_windows_are_extracted_by_va() {
        let (bytes, miplevel, texcolor) = image_with_windows();
        // The fixture is nonstock, so its digest cannot equal REVIEWED_IMAGE;
        // the gate below therefore refuses here, proving identity is checked
        // before extraction. The extraction path itself is covered by the
        // original-image gate.
        assert!(TextureQualityWindows::read(&bytes, origin()).is_err());
        // Sanity: the synthetic section does hold the bytes at the right VAs.
        let pe = PeSections::parse(&bytes).unwrap();
        assert_eq!(
            pe.va_bytes(MIPLEVEL_VA, MIPLEVEL_LEN).unwrap(),
            &miplevel[..]
        );
        assert_eq!(
            pe.va_bytes(TEXCOLORDEPTH_VA, TEXCOLORDEPTH_LEN).unwrap(),
            &texcolor[..]
        );
        assert!(pe.va_bytes(MIPLEVEL_VA - 1, 1).is_none());
        let mut bad = bytes;
        bad[0] ^= 1;
        assert!(TextureQualityWindows::read(&bad, origin()).is_err());
        assert!(TextureQualityWindows::read(b"MZ", origin()).is_err());
    }

    #[test]
    #[ignore = "requires PCX_WAR3_EXE naming the reviewed Legacy Warcraft III.exe"]
    fn selected_original_image_extracts_reviewed_branch_windows() {
        let image = std::fs::read(std::env::var("PCX_WAR3_EXE").expect("PCX_WAR3_EXE")).unwrap();
        let mut source = origin();
        source.source = std::env::var("PCX_WAR3_EXE").unwrap();
        let windows = TextureQualityWindows::read(&image, source).unwrap();
        assert_eq!(windows.image_sha256, sha256(&image));
        assert_eq!(windows.miplevel_branch.len(), MIPLEVEL_LEN);
        assert_eq!(windows.texcolordepth_branch.len(), TEXCOLORDEPTH_LEN);
        // Opcodes, not policy words (pcx-gfx derives the words).
        assert_eq!(&windows.miplevel_branch[..2], &[0x33, 0xc9]);
        assert_eq!(windows.texcolordepth_branch[0], 0xb8);
    }
}

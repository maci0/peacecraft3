//! Selection image placement from the reviewed, locally supplied Legacy client.
use crate::{
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    scripts::AssetIdentity,
};

const IMAGE_BUDGET: usize = 32 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct Parameters {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    pub z_offset: f32,
}

impl Parameters {
    pub fn read(image: &[u8], origin: AssetIdentity) -> Result<Self, String> {
        let fail = |reason: &str| format!("selection circle {}: {reason}", origin.source);
        if image.len() > IMAGE_BUDGET {
            return Err(fail("client image exceeds budget"));
        }
        if [&origin.source, &origin.edition, &origin.version]
            .into_iter()
            .any(|value| value.trim().is_empty())
        {
            return Err(fail("missing source identity"));
        }
        let digest = sha256(image);
        if digest != REVIEWED_IMAGE {
            return Err(fail("unsupported client image; no placement substituted"));
        }
        let z_offset = decode(image).map_err(|reason| fail(&reason))?;
        Ok(Self {
            origin,
            image_sha256: digest,
            z_offset,
        })
    }
}

fn decode(image: &[u8]) -> Result<f32, String> {
    let pe = Pe::parse(
        image,
        RuleInputBudget {
            image_bytes: IMAGE_BUDGET,
            profile_bytes: 128,
            sections: 96,
        },
    )
    .map_err(|e| e.to_string())?;
    // Reviewed instruction locations identify the selection image type and its Z table.
    let kind = pe.read_from(0x7033cb, true).map_err(|e| e.to_string())?;
    let kind = match kind.get(..2) {
        Some([0x6a, kind]) => u32::from(*kind),
        _ => return Err("missing selection image type instruction".into()),
    };
    let instruction = pe.read_from(0x73d093, true).map_err(|e| e.to_string())?;
    let table = match instruction.get(..9) {
        Some([0xf3, 0x0f, 0x10, 0x04, 0x85, a, b, c, d]) => u32::from_le_bytes([*a, *b, *c, *d]),
        _ => return Err("missing terrain image Z table instruction".into()),
    };
    let address = table
        .checked_add(kind * 4)
        .ok_or("selection image Z table address overflow")?;
    let bytes = pe.read_from(address, false).map_err(|e| e.to_string())?;
    let word = bytes.get(..4).ok_or("truncated selection image Z offset")?;
    let value = f32::from_le_bytes(word.try_into().unwrap());
    if !value.is_finite() || value < 0.0 {
        return Err("invalid selection image Z offset".into());
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn origin() -> AssetIdentity {
        AssetIdentity {
            source: "synthetic selection client".into(),
            edition: "fixture".into(),
            version: "fixture".into(),
        }
    }

    fn fixture(kind: u8, value: f32) -> Vec<u8> {
        let mut image = vec![0; 0x3200];
        image[..2].copy_from_slice(b"MZ");
        image[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        image[0x80..0x84].copy_from_slice(b"PE\0\0");
        image[0x84..0x86].copy_from_slice(&0x14cu16.to_le_bytes());
        image[0x86..0x88].copy_from_slice(&3u16.to_le_bytes());
        image[0x94..0x96].copy_from_slice(&64u16.to_le_bytes());
        image[0x98..0x9a].copy_from_slice(&0x10bu16.to_le_bytes());
        image[0xb4..0xb8].copy_from_slice(&0x400000u32.to_le_bytes());
        image[0xd0..0xd4].copy_from_slice(&0xc00000u32.to_le_bytes());
        for (index, (va, flags)) in [
            (0x703000u32, 0x60000020u32),
            (0x73d000, 0x60000020),
            (0xead000, 0x40000040),
        ]
        .into_iter()
        .enumerate()
        {
            let section = 0xd8 + index * 40;
            for (offset, value) in [
                (8, 0x1000),
                (12, va - 0x400000),
                (16, 0x1000),
                (20, 0x200 + index as u32 * 0x1000),
                (36, flags),
            ] {
                image[section + offset..section + offset + 4].copy_from_slice(&value.to_le_bytes());
            }
        }
        image[0x5cb..0x5cd].copy_from_slice(&[0x6a, kind]);
        image[0x1293..0x1298].copy_from_slice(&[0xf3, 0x0f, 0x10, 0x04, 0x85]);
        image[0x1298..0x129c].copy_from_slice(&0xead100u32.to_le_bytes());
        let at = 0x2300 + usize::from(kind) * 4;
        image[at..at + 4].copy_from_slice(&value.to_le_bytes());
        image
    }

    #[test]
    fn type_and_table_operands_supply_offset_without_substitutes() {
        for (kind, offset) in [(2, 17.25), (4, 0.375), (0, 0.0)] {
            assert_eq!(decode(&fixture(kind, offset)).unwrap(), offset);
        }
        for offset in [f32::NAN, f32::INFINITY, -0.5] {
            assert!(decode(&fixture(2, offset)).is_err());
        }
    }

    #[test]
    fn malformed_and_unreviewed_images_refuse() {
        let image = fixture(2, 4.25);
        assert!(Parameters::read(&image, origin()).is_err());
        for at in [0, 0x5cb, 0x1293] {
            let mut changed = image.clone();
            changed[at] ^= 0xff;
            assert!(decode(&changed).is_err());
        }
        assert!(decode(&image[..0x2300]).is_err());
        let mut changed = image;
        changed[0x1298..0x129c].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(decode(&changed).is_err());
    }

    #[test]
    #[ignore = "requires explicitly supplied reviewed Legacy client image"]
    fn original_selection_offset_comes_from_selected_image() {
        let path = std::env::var("PCX_SELECTION_CLIENT_IMAGE").expect("PCX_SELECTION_CLIENT_IMAGE");
        let image = std::fs::read(&path).unwrap();
        let mut identity = origin();
        identity.source = path;
        identity.edition = "Legacy target".into();
        identity.version = "1.29.2.9231".into();
        let parameters = Parameters::read(&image, identity).unwrap();
        assert_eq!(parameters.image_sha256, sha256(&image));
        assert!(parameters.z_offset > 0.0);
        let mut changed = image;
        changed[0x2300] ^= 1;
        assert!(Parameters::read(&changed, parameters.origin).is_err());
    }
}

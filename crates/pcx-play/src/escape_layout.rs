//! Escape modal anchor chain from the selected, reviewed client image.
use crate::{
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    scripts::AssetIdentity,
};

#[derive(Clone, Debug)]
pub struct Anchor {
    pub point: u8,
    pub relative: u8,
    pub offset: [f32; 2],
}
#[derive(Clone, Debug)]
pub struct Rules {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    pub viewport: [f32; 2],
    pub root_extent: [f32; 2],
    pub root: Anchor,
    pub panel: Anchor,
    pub root_frame: String,
    pub backdrop_frame: String,
}
fn fraction(point: u8) -> Result<[f32; 2], String> {
    if point > 8 {
        return Err(format!("unsupported frame anchor {point}"));
    }
    Ok([f32::from(point % 3) / 2., 1. - f32::from(point / 3) / 2.])
}
fn positive(extent: [f32; 2]) -> bool {
    extent.iter().all(|v| v.is_finite() && *v > 0.)
}
impl Rules {
    pub fn position(&self, panel_extent: [f32; 2]) -> Result<[f32; 2], String> {
        if !positive(self.viewport) || !positive(self.root_extent) || !positive(panel_extent) {
            return Err("escape layout: invalid extent".into());
        }
        let root_point = fraction(self.root.point)?;
        let root_relative = fraction(self.root.relative)?;
        let panel_point = fraction(self.panel.point)?;
        let panel_relative = fraction(self.panel.relative)?;
        let result = std::array::from_fn(|axis| {
            root_relative[axis] * self.viewport[axis] + self.root.offset[axis]
                - root_point[axis] * self.root_extent[axis]
                + panel_relative[axis] * self.root_extent[axis]
                + self.panel.offset[axis]
                - panel_point[axis] * panel_extent[axis]
        });
        if result.iter().any(|v: &f32| !v.is_finite()) {
            return Err("escape layout: non-finite anchor composition".into());
        }
        Ok(result)
    }
    pub fn read(image: &[u8], origin: AssetIdentity) -> Result<Self, String> {
        let fail = |error: &str| format!("escape layout {}: {error}", origin.source);
        if image.len() > 32 << 20 {
            return Err(fail("client image exceeds byte budget"));
        }
        let digest = sha256(image);
        if digest != REVIEWED_IMAGE
            || [&origin.source, &origin.edition, &origin.version]
                .iter()
                .any(|s| s.trim().is_empty())
        {
            return Err(fail("unsupported client identity"));
        }
        let pe = Pe::parse(
            image,
            RuleInputBudget {
                image_bytes: 32 << 20,
                profile_bytes: 64 << 10,
                sections: 96,
            },
        )
        .map_err(|e| fail(&e.to_string()))?;
        Self::decode(origin.clone(), digest, |va, n, code| {
            pe.read_from(va, code)
                .map_err(|e| e.to_string())?
                .get(..n)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| format!("truncated record at {va:#x}"))
        })
        .map_err(|e| fail(&e))
    }
    fn decode(
        origin: AssetIdentity,
        image_sha256: Sha256Digest,
        mut read: impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
    ) -> Result<Self, String> {
        let mut operand = |va, prefix: &[u8], size| {
            let bytes = read(va, prefix.len() + size, true)?;
            if bytes.get(..prefix.len()) != Some(prefix) || bytes.len() != prefix.len() + size {
                return Err(format!("unsupported instruction at {va:#x}"));
            }
            Ok(bytes[prefix.len()..].to_vec())
        };
        fn word(bytes: Vec<u8>) -> Result<u32, String> {
            Ok(u32::from_le_bytes(
                bytes.try_into().map_err(|_| "truncated operand")?,
            ))
        }
        for (site, target) in [
            (0x61d706u32, 0x873a10u32),
            (0x61d71c, 0x873b40),
            (0x61d732, 0x873830),
            (0x61d750, 0x8958b0),
            (0x61d788, 0x873a10),
            (0x61d799, 0x8958b0),
        ] {
            let displacement = word(operand(site, &[0xe8], 4)?)?;
            if site.wrapping_add(5).wrapping_add(displacement) != target {
                return Err(format!("unsupported anchor call at {site:#x}"));
            }
        }
        for site in [0x61d6d8, 0x61d75d] {
            if operand(site, &[0x6a], 1)? != [1] {
                return Err(format!("unsupported anchor mode at {site:#x}"));
            }
        }
        let viewport_pointers = [
            word(operand(0xab2a9f, &[0xf3, 0x0f, 0x59, 0x05], 4)?)?,
            word(operand(0xab2ab7, &[0xf3, 0x0f, 0x59, 0x05], 4)?)?,
        ];
        let root_extent = [
            f32::from_bits(word(operand(0x61d715, &[0xc7, 0x04, 0x24], 4)?)?),
            f32::from_bits(word(operand(0x61d72b, &[0xc7, 0x04, 0x24], 4)?)?),
        ];
        let root = Anchor {
            point: operand(0x61d704, &[0x6a], 1)?[0],
            relative: operand(0x61d701, &[0x6a], 1)?[0],
            offset: [
                f32::from_bits(word(operand(0x61d6fa, &[0xc7, 0x04, 0x24], 4)?)?),
                f32::from_bits(word(operand(0x61d6f0, &[0xc7, 0x44, 0x24, 0x04], 4)?)?),
            ],
        };
        let panel = Anchor {
            point: operand(0x61d786, &[0x6a], 1)?[0],
            relative: operand(0x61d783, &[0x6a], 1)?[0],
            offset: [
                f32::from_bits(word(operand(0x61d776, &[0xc7, 0x04, 0x24], 4)?)?),
                f32::from_bits(word(operand(0x61d76c, &[0xc7, 0x44, 0x24, 0x04], 4)?)?),
            ],
        };
        let backdrop_pointer = word(operand(0x61d74b, &[0x68], 4)?)?;
        let root_pointer = word(operand(0x61d794, &[0x68], 4)?)?;
        let viewport = [
            f32::from_bits(word(read(viewport_pointers[0], 4, false)?)?),
            f32::from_bits(word(read(viewport_pointers[1], 4, false)?)?),
        ];
        let mut string = |pointer: u32| {
            let mut result = Vec::new();
            for offset in 0..4096 {
                let address = pointer
                    .checked_add(offset)
                    .ok_or("string address overflow")?;
                let bytes = read(address, 1, false)?;
                let byte = *bytes.first().ok_or("truncated frame name")?;
                if byte == 0 {
                    if result.is_empty() {
                        return Err("empty frame name".into());
                    }
                    return String::from_utf8(result).map_err(|e| e.to_string());
                }
                result.push(byte);
            }
            Err("frame name exceeds byte budget".into())
        };
        let rules = Self {
            origin,
            image_sha256,
            viewport,
            root_extent,
            root,
            panel,
            root_frame: string(root_pointer)?,
            backdrop_frame: string(backdrop_pointer)?,
        };
        rules.position(rules.root_extent)?;
        Ok(rules)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Rules {
        Rules {
            origin: AssetIdentity {
                source: "synthetic".into(),
                edition: "fixture".into(),
                version: "1".into(),
            },
            image_sha256: sha256(b"fixture"),
            viewport: [12., 10.],
            root_extent: [6., 4.],
            root: Anchor {
                point: 0,
                relative: 2,
                offset: [-7., -1.],
            },
            panel: Anchor {
                point: 8,
                relative: 4,
                offset: [0.25, 0.75],
            },
            root_frame: "Root".into(),
            backdrop_frame: "Backdrop".into(),
        }
    }
    #[test]
    fn composes_distinct_root_and_backdrop_anchors() {
        let rules = fixture();
        assert_eq!(rules.position([2., 1.]).unwrap(), [6.25, 7.75]);
    }
    #[test]
    fn rejects_invalid_extents_offsets_and_anchors() {
        let mut rules = fixture();
        assert!(rules.position([f32::NAN, 1.]).is_err());
        rules.panel.point = 9;
        assert!(rules.position([1., 1.]).is_err());
        rules.panel.point = 4;
        rules.root.offset[0] = f32::INFINITY;
        assert!(rules.position([1., 1.]).is_err());
    }
    #[test]
    fn decoder_reads_nonstock_values_and_rejects_malformed_records() {
        use std::collections::BTreeMap;
        let mut bytes = BTreeMap::new();
        let mut put = |address: u32, prefix: &[u8], operand: &[u8]| {
            for (offset, byte) in prefix.iter().chain(operand).copied().enumerate() {
                bytes.insert(address + offset as u32, byte);
            }
        };
        for (site, target) in [
            (0x61d706u32, 0x873a10u32),
            (0x61d71c, 0x873b40),
            (0x61d732, 0x873830),
            (0x61d750, 0x8958b0),
            (0x61d788, 0x873a10),
            (0x61d799, 0x8958b0),
        ] {
            put(site, &[0xe8], &target.wrapping_sub(site + 5).to_le_bytes());
        }
        for site in [0x61d6d8, 0x61d75d] {
            put(site, &[0x6a], &[1]);
        }
        for (site, value) in [(0x61d704, 0), (0x61d701, 2), (0x61d786, 8), (0x61d783, 4)] {
            put(site, &[0x6a], &[value]);
        }
        for (site, value) in [
            (0x61d715, 6f32),
            (0x61d72b, 4.),
            (0x61d6fa, -7.),
            (0x61d776, 0.25),
        ] {
            put(site, &[0xc7, 0x04, 0x24], &value.to_le_bytes());
        }
        for (site, value) in [(0x61d6f0, -1f32), (0x61d76c, 0.75)] {
            put(site, &[0xc7, 0x44, 0x24, 0x04], &value.to_le_bytes());
        }
        for (site, pointer, value) in [(0xab2a9f, 0x1000u32, 12f32), (0xab2ab7, 0x1004, 10.)] {
            put(site, &[0xf3, 0x0f, 0x59, 0x05], &pointer.to_le_bytes());
            put(pointer, &[], &value.to_le_bytes());
        }
        for (site, pointer, name) in [
            (0x61d74b, 0x2000u32, "OtherBackdrop\0"),
            (0x61d794, 0x3000, "OtherRoot\0"),
        ] {
            put(site, &[0x68], &pointer.to_le_bytes());
            put(pointer, &[], name.as_bytes());
        }
        let decode = |bytes: &BTreeMap<u32, u8>| {
            let identity = fixture();
            Rules::decode(identity.origin, identity.image_sha256, |address, n, _| {
                (0..n)
                    .map(|offset| {
                        bytes
                            .get(&(address + offset as u32))
                            .copied()
                            .ok_or_else(|| "missing fixture byte".into())
                    })
                    .collect()
            })
        };
        let rules = decode(&bytes).unwrap();
        assert_eq!(rules.viewport, [12., 10.]);
        assert_eq!(rules.root_frame, "OtherRoot");
        assert_eq!(rules.backdrop_frame, "OtherBackdrop");
        assert_eq!(rules.position([2., 1.]).unwrap(), [6.25, 7.75]);
        let mut invalid = bytes.clone();
        invalid.insert(0x61d787, 9);
        assert!(decode(&invalid).is_err());
        for (offset, byte) in f32::NAN.to_le_bytes().into_iter().enumerate() {
            invalid.insert(0x1000 + offset as u32, byte);
        }
        invalid.insert(0x61d787, 8);
        assert!(decode(&invalid).is_err());
        bytes.remove(&0x61d706);
        assert!(decode(&bytes).is_err());
    }
    #[test]
    fn refuses_unqualified_instructions() {
        let rules = fixture();
        assert!(Rules::decode(rules.origin, rules.image_sha256, |_, n, _| Ok(vec![0; n])).is_err());
    }
    #[test]
    #[ignore = "requires PCX_ESCAPE_CLIENT_IMAGE selected original executable"]
    fn selected_original_anchor_chain() {
        let path = std::env::var("PCX_ESCAPE_CLIENT_IMAGE").unwrap();
        let image = std::fs::read(&path).unwrap();
        let rules = Rules::read(
            &image,
            AssetIdentity {
                source: path,
                edition: "selected Legacy".into(),
                version: "1.29.2.9231".into(),
            },
        )
        .unwrap();
        assert!(
            rules
                .position([0.31, 0.27])
                .unwrap()
                .iter()
                .all(|v| v.is_finite())
        );
        eprintln!("{rules:?}");
    }
}

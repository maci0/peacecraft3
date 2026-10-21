//! Selected-client startup passive alliances; explicit script writes take precedence.
use crate::{
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    scripts::AssetIdentity,
};

#[derive(Clone, Debug)]
pub struct Rules {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    neutral: [i32; 2],
    active: [i32; 2],
    pub(crate) kind: i32,
}

impl Rules {
    pub fn reviewed_legacy(image: &[u8], origin: AssetIdentity) -> Result<Self, String> {
        if image.len() > 32 << 20
            || sha256(image) != crate::menu_lighting::REVIEWED_IMAGE
            || [&origin.source, &origin.edition, &origin.version]
                .iter()
                .any(|s| s.trim().is_empty())
        {
            return Err("startup alliances: unsupported client identity".into());
        }
        let pe = Pe::parse(
            image,
            RuleInputBudget {
                image_bytes: 32 << 20,
                profile_bytes: 64 << 10,
                sections: 96,
            },
        )
        .map_err(|e| e.to_string())?;
        Self::decode(origin, sha256(image), |va, n| {
            pe.read_from(va, true)
                .map_err(|e| e.to_string())?
                .get(..n)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| "truncated startup alliance instructions".into())
        })
    }

    fn decode(
        origin: AssetIdentity,
        image_sha256: Sha256Digest,
        mut read: impl FnMut(u32, usize) -> Result<Vec<u8>, String>,
    ) -> Result<Self, String> {
        let neutral = read(0x47e82c, 10)?;
        let active = read(0x47e8da, 9)?;
        if neutral.len() != 10
            || neutral[..2] != [0x83, 0xf8]
            || neutral[3..7] != [0x74, 5, 0x83, 0xf8]
            || neutral[8..] != [0x75, 0x4a]
            || active.len() != 9
            || active[..6] != [0x85, 0xc0, 0x74, 5, 0x83, 0xf8]
            || active[7..] != [0x75, 0x6a]
        {
            return Err("unsupported startup controller branches".into());
        }
        let mut kind = None;
        for (va, prefix) in [
            (0x47e923, vec![]),
            (0x47e933, vec![0x8b, 0xc8]),
            (0x47e9d2, vec![0x8b, 0xc8]),
            (0x47e9e7, vec![]),
        ] {
            let bytes = read(va, 7 + prefix.len())?;
            if bytes.len() != 7 + prefix.len()
                || bytes[0] != 0x6a
                || bytes[2..2 + prefix.len()] != prefix
                || bytes[2 + prefix.len()] != 0xe8
            {
                return Err("unsupported startup alliance call".into());
            }
            let delta = i32::from_le_bytes(
                bytes[3 + prefix.len()..]
                    .try_into()
                    .map_err(|_| "call displacement")?,
            );
            if i64::from(va) + bytes.len() as i64 + i64::from(delta) != 0x4c1f70 {
                return Err("startup alliance setter differs".into());
            }
            let value = i32::from(bytes[1] as i8);
            if kind.is_some_and(|k| k != value) {
                return Err("startup alliance kinds differ".into());
            }
            kind = Some(value);
        }
        let neutral = [i32::from(neutral[2] as i8), i32::from(neutral[7] as i8)];
        let active = [0, i32::from(active[6] as i8)]; // `test eax,eax; je` admits zero.
        if neutral[0] == neutral[1]
            || active[0] == active[1]
            || neutral.iter().any(|v| active.contains(v))
        {
            return Err("ambiguous startup controller classes".into());
        }
        Ok(Self {
            origin,
            image_sha256,
            neutral,
            active,
            kind: kind.ok_or("alliance kind absent")?,
        })
    }

    pub(crate) fn allied(&self, a: i32, b: i32) -> bool {
        (self.neutral.contains(&a) && (self.neutral.contains(&b) || self.active.contains(&b)))
            || (self.active.contains(&a) && self.neutral.contains(&b))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub(crate) fn fixture(neutral: [u8; 2], active: u8, kind: u8) -> Result<Rules, String> {
        Rules::decode(
            AssetIdentity {
                source: "synthetic:alliances".into(),
                edition: "synthetic".into(),
                version: "1".into(),
            },
            sha256(&[]),
            |va, n| {
                let bytes = match va {
                    0x47e82c => vec![
                        0x83, 0xf8, neutral[0], 0x74, 5, 0x83, 0xf8, neutral[1], 0x75, 0x4a,
                    ],
                    0x47e8da => vec![0x85, 0xc0, 0x74, 5, 0x83, 0xf8, active, 0x75, 0x6a],
                    _ => {
                        let mut b = vec![0x6a, kind];
                        if matches!(va, 0x47e933 | 0x47e9d2) {
                            b.extend([0x8b, 0xc8]);
                        }
                        b.push(0xe8);
                        b.extend(
                            (0x4c1f70_i64 - i64::from(va) - b.len() as i64 - 4).to_le_bytes()[..4]
                                .iter(),
                        );
                        b
                    }
                };
                if bytes.len() != n {
                    return Err("fixture length".into());
                }
                Ok(bytes)
            },
        )
    }
    #[test]
    fn classes_and_alliance_kind_follow_source_operands() {
        for (neutral, active, kind) in [([2, 3], 1, 0), ([7, 9], 5, 11)] {
            let r = fixture(neutral, active, kind).unwrap();
            assert_eq!(r.kind, i32::from(kind));
            for n in neutral {
                assert!(r.allied(i32::from(n), i32::from(active)));
                assert!(r.allied(0, i32::from(n)));
                assert!(r.allied(i32::from(neutral[0]), i32::from(n)));
            }
            assert!(!r.allied(0, i32::from(active)));
            assert!(!r.allied(i32::from(neutral[0]), 99));
        }
        assert!(fixture([2, 2], 1, 0).is_err());
        assert!(fixture([0, 2], 1, 0).is_err());
    }
    #[test]
    fn missing_and_unrecognized_instruction_windows_refuse() {
        let origin = AssetIdentity {
            source: "synthetic:bad".into(),
            edition: "synthetic".into(),
            version: "1".into(),
        };
        assert!(Rules::decode(origin.clone(), sha256(&[]), |_, _| Ok(Vec::new())).is_err());
        assert!(Rules::decode(origin, sha256(&[]), |_, n| Ok(vec![0; n])).is_err());
    }
    #[test]
    #[ignore = "requires explicit PCX_ALLIANCE_IMAGE selected original executable"]
    fn local_original_startup_alliance_branches() {
        let path = std::env::var("PCX_ALLIANCE_IMAGE").expect("explicit original executable");
        let image = std::fs::read(&path).unwrap();
        let rules = Rules::reviewed_legacy(
            &image,
            AssetIdentity {
                source: path,
                edition: "Legacy".into(),
                version: "1.29.2.9231".into(),
            },
        )
        .unwrap();
        assert!(rules.allied(0, 3));
        assert!(rules.allied(3, 0));
        assert!(!rules.allied(0, 1));
    }
}

//! Selected-client startup alliances; explicit script writes take precedence.
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
    pub(crate) kind: i8,
    pub(crate) shared_control_kind: i8,
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
        let shared_control = pe
            .read_from(0x47cea6, true)
            .map_err(|e| e.to_string())?
            .get(..10)
            .ok_or("truncated startup shared-control instructions")?;
        Self::decode(origin, sha256(image), shared_control, |va, n| {
            let source = pe
                .read_from(va, true)
                .map_err(|e| e.to_string())?
                .get(..n)
                .ok_or_else(|| "truncated startup alliance instructions".to_string())?;
            let mut window = [0; 10];
            window[..n].copy_from_slice(source);
            Ok(window)
        })
    }

    fn decode(
        origin: AssetIdentity,
        image_sha256: Sha256Digest,
        shared_control: &[u8],
        mut read: impl FnMut(u32, usize) -> Result<[u8; 10], String>,
    ) -> Result<Self, String> {
        let neutral = read(0x47e82c, 10)?;
        let active = read(0x47e8da, 9)?;
        if neutral[..2] != [0x83, 0xf8]
            || neutral[3..7] != [0x74, 5, 0x83, 0xf8]
            || neutral[8..] != [0x75, 0x4a]
            || active[..6] != [0x85, 0xc0, 0x74, 5, 0x83, 0xf8]
            || active[7..9] != [0x75, 0x6a]
        {
            return Err("unsupported startup controller branches".into());
        }
        if shared_control.len() != 10
            || shared_control[..2] != [0x53, 0x6a]
            || shared_control[3..6] != [0x8b, 0xce, 0xe8]
        {
            return Err("unsupported startup shared-control call".into());
        }
        let delta = i32::from_le_bytes(
            shared_control[6..10]
                .try_into()
                .map_err(|_| "shared-control call displacement")?,
        );
        if 0x47cea6_i64 + 10 + i64::from(delta) != 0x4c1f70 {
            return Err("startup shared-control setter differs".into());
        }
        let shared_control_kind = shared_control[2] as i8;
        let mut kind = None;
        for (va, prefix) in [
            (0x47e923, &[][..]),
            (0x47e933, &[0x8b, 0xc8][..]),
            (0x47e9d2, &[0x8b, 0xc8][..]),
            (0x47e9e7, &[][..]),
        ] {
            let bytes = read(va, 7 + prefix.len())?;
            if bytes[0] != 0x6a
                || bytes[2..2 + prefix.len()] != *prefix
                || bytes[2 + prefix.len()] != 0xe8
            {
                return Err("unsupported startup alliance call".into());
            }
            let delta = i32::from_le_bytes(
                bytes[3 + prefix.len()..7 + prefix.len()]
                    .try_into()
                    .map_err(|_| "call displacement")?,
            );
            if i64::from(va) + (7 + prefix.len()) as i64 + i64::from(delta) != 0x4c1f70 {
                return Err("startup alliance setter differs".into());
            }
            let value = bytes[1] as i8;
            if kind.is_some_and(|k| k != value) {
                return Err("startup alliance kinds differ".into());
            }
            kind = Some(value);
        }
        let neutral = [i32::from(neutral[2] as i8), i32::from(neutral[7] as i8)];
        let active = [0, i32::from(active[6] as i8)]; // `test eax,eax; je` admits zero.
        let kind = kind.ok_or("alliance kind absent")?;
        if neutral[0] == neutral[1]
            || active[0] == active[1]
            || neutral.iter().any(|v| active.contains(v))
            || kind == shared_control_kind
        {
            return Err("ambiguous startup controller classes".into());
        }
        Ok(Self {
            origin,
            image_sha256,
            neutral,
            active,
            kind,
            shared_control_kind,
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
        let mut shared_control = [
            0x53,
            0x6a,
            kind.wrapping_add(1),
            0x8b,
            0xce,
            0xe8,
            0,
            0,
            0,
            0,
        ];
        shared_control[6..].copy_from_slice(&(0x4c1f70_i32 - 0x47cea6 - 10).to_le_bytes());
        Rules::decode(
            AssetIdentity {
                source: "synthetic:alliances".into(),
                edition: "synthetic".into(),
                version: "1".into(),
            },
            sha256(&[]),
            &shared_control,
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
                let mut window = [0; 10];
                window[..n].copy_from_slice(&bytes);
                Ok(window)
            },
        )
    }
    #[test]
    fn classes_and_alliance_kind_follow_source_operands() {
        for (neutral, active, kind) in [([2, 3], 1, 0), ([7, 9], 5, 11)] {
            let r = fixture(neutral, active, kind).unwrap();
            assert_eq!(r.kind, kind as i8);
            assert_eq!(r.shared_control_kind, kind.wrapping_add(1) as i8);
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
        assert!(Rules::decode(origin.clone(), sha256(&[]), &[], |_, _| Ok([0; 10])).is_err());
        assert!(Rules::decode(origin, sha256(&[]), &[0; 10], |_, _| Ok([0; 10])).is_err());
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

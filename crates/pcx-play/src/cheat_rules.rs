//! Selected-client new-session cheat flags and query masks. Cheat mutation is not admitted.
use crate::{
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    scripts::AssetIdentity,
};

#[derive(Clone, Debug)]
pub struct Rules {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    flags: u32,
    no_defeat: u32,
    no_victory: u32,
}
impl Rules {
    pub fn reviewed_legacy(image: &[u8], origin: AssetIdentity) -> Result<Self, String> {
        let fail = |reason: &str| format!("cheat query rules {}: {reason}", origin.source);
        if image.len() > 32 << 20
            || sha256(image) != crate::menu_lighting::REVIEWED_IMAGE
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
        let window = |va, len| -> Result<&[u8], String> {
            pe.read_from(va, true)
                .map_err(|e| fail(&e.to_string()))?
                .get(..len)
                .ok_or_else(|| fail("truncated instruction window"))
        };
        let (flags, no_defeat, no_victory) = operands(
            window(0x605f02, 7)?,
            window(0x4994b0, 34)?,
            window(0x4994e0, 34)?,
        )
        .map_err(fail)?;
        Ok(Self {
            origin,
            image_sha256: sha256(image),
            flags,
            no_defeat,
            no_victory,
        })
    }
    pub(crate) fn no_defeat(&self) -> bool {
        self.flags & self.no_defeat != 0
    }
    pub(crate) fn no_victory(&self) -> bool {
        self.flags & self.no_victory != 0
    }
}

fn operands(
    initial: &[u8],
    defeat: &[u8],
    victory: &[u8],
) -> Result<(u32, u32, u32), &'static str> {
    if initial.len() != 7 || initial[..2] != [0xc7, 0x41] {
        return Err("unsupported initial flags instruction");
    }
    let mask = |code: &[u8]| -> Result<u32, &'static str> {
        if code.len() != 34
            || code[0] != 0x6a
            || code[2] != 0xe8
            || code[7..12] != [0x83, 0xc4, 0x04, 0x8b, 0x40]
            || code[13..21] != [0x8b, 0x00, 0x85, 0xc0, 0x74, 0x0c, 0xf6, 0x40]
            || code[21] != initial[2]
            || code[23..]
                != [
                    0x74, 0x06, 0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3, 0x33, 0xc0, 0xc3,
                ]
            || code[22] == 0
        {
            return Err("unsupported cheat query instruction window");
        }
        Ok(u32::from(code[22]))
    };
    let a = mask(defeat)?;
    let b = mask(victory)?;
    if defeat[1] != victory[1] || defeat[12] != victory[12] || a & b != 0 {
        return Err("inconsistent cheat query state or overlapping masks");
    }
    Ok((u32::from_le_bytes(initial[3..].try_into().unwrap()), a, b))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub(crate) fn fixture(flags: u32, a: u8, b: u8) -> Rules {
        let mut initial = vec![0xc7, 0x41, 0x18];
        initial.extend(flags.to_le_bytes());
        let getter = |mask| {
            vec![
                0x6a, 0x19, 0xe8, 0, 0, 0, 0, 0x83, 0xc4, 0x04, 0x8b, 0x40, 0x20, 0x8b, 0, 0x85,
                0xc0, 0x74, 0x0c, 0xf6, 0x40, 0x18, mask, 0x74, 0x06, 0xb8, 1, 0, 0, 0, 0xc3, 0x33,
                0xc0, 0xc3,
            ]
        };
        let (flags, no_defeat, no_victory) = operands(&initial, &getter(a), &getter(b)).unwrap();
        Rules {
            origin: AssetIdentity {
                source: "synthetic:cheat-image".into(),
                edition: "synthetic".into(),
                version: "fixture".into(),
            },
            image_sha256: sha256(&initial),
            flags,
            no_defeat,
            no_victory,
        }
    }
    #[test]
    fn cheat_query_operands_follow_source_flags_and_masks() {
        for (flags, a, b, defeat, victory) in [
            (0, 2, 8, false, false),
            (2, 2, 8, true, false),
            (8, 2, 8, false, true),
            (10, 2, 8, true, true),
            (0x20, 0x10, 0x20, false, true),
        ] {
            let rules = fixture(flags, a, b);
            assert_eq!(rules.no_defeat(), defeat);
            assert_eq!(rules.no_victory(), victory);
        }
    }
    #[test]
    fn cheat_query_windows_refuse_truncation_and_wrong_shapes() {
        let initial = [0xc7, 0x41, 4, 0, 0, 0, 0];
        let mut query = vec![
            0x6a, 0x19, 0xe8, 0, 0, 0, 0, 0x83, 0xc4, 4, 0x8b, 0x40, 12, 0x8b, 0, 0x85, 0xc0, 0x74,
            12, 0xf6, 0x40, 4, 2, 0x74, 6, 0xb8, 1, 0, 0, 0, 0xc3, 0x33, 0xc0, 0xc3,
        ];
        let mut other = query.clone();
        other[22] = 8;
        for len in 0..initial.len() {
            assert!(operands(&initial[..len], &query, &other).is_err())
        }
        for len in 0..query.len() {
            assert!(operands(&initial, &query[..len], &other).is_err());
            assert!(operands(&initial, &other, &query[..len]).is_err())
        }
        for at in [0, 2, 7, 13, 19, 21, 23, 25, 30, 33] {
            query[at] ^= 1;
            assert!(operands(&initial, &query, &other).is_err());
            query[at] ^= 1;
        }
        other[22] = 2;
        assert!(operands(&initial, &query, &other).is_err());
    }
    #[test]
    fn cheat_query_unknown_image_keeps_source_context() {
        let error = Rules::reviewed_legacy(&[], fixture(0, 2, 8).origin).unwrap_err();
        assert!(error.contains("synthetic:cheat-image") && error.contains("identity"));
    }
    #[test]
    #[ignore = "requires PCX_CHEAT_CLIENT pointing to the locally supplied reviewed client"]
    fn local_original_cheat_query_rules_are_new_session_values() {
        let path = std::env::var("PCX_CHEAT_CLIENT").unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let rules = Rules::reviewed_legacy(
            &bytes,
            AssetIdentity {
                source: path,
                edition: "selected legacy client".into(),
                version: "1.29.2.9231".into(),
            },
        )
        .unwrap();
        assert!(!rules.no_defeat());
        assert!(!rules.no_victory());
    }
}

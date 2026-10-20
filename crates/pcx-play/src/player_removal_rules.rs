//! Selected-client result/slot transitions and player-event numbering.
use crate::{
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    scripts::AssetIdentity,
};

#[derive(Clone, Debug)]
pub struct Rules {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    pub(crate) playing: i32,
    pub(crate) left: i32,
    pub(crate) victory: i32,
    pub(crate) defeat: i32,
    pub(crate) tie: i32,
    pub(crate) neutral: i32,
    pub(crate) defeat_event: i32,
    pub(crate) victory_event: i32,
}
impl Rules {
    pub fn reviewed_legacy(image: &[u8], origin: AssetIdentity) -> Result<Self, String> {
        let context = format!("player removal rules {}", origin.source);
        let fail = |why: &str| format!("{context}: {why}");
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
        Self::decode(origin, sha256(image), |va, len| {
            pe.read_from(va, true)
                .map_err(|e| e.to_string())?
                .get(..len)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| "truncated removal instruction window".into())
        })
        .map_err(|e| fail(&e))
    }
    fn decode(
        origin: AssetIdentity,
        image_sha256: Sha256Digest,
        mut read: impl FnMut(u32, usize) -> Result<Vec<u8>, String>,
    ) -> Result<Self, String> {
        fn operand(
            read: &mut impl FnMut(u32, usize) -> Result<Vec<u8>, String>,
            va: u32,
            prefix: &[u8],
            len: usize,
        ) -> Result<Vec<u8>, String> {
            let code = read(va, prefix.len() + len)?;
            if code.len() != prefix.len() + len || code[..prefix.len()] != *prefix {
                return Err(format!("unsupported player-removal instruction at {va:#x}"));
            }
            Ok(code[prefix.len()..].to_vec())
        }
        fn word(b: &[u8]) -> Result<i32, String> {
            b.try_into()
                .map(i32::from_le_bytes)
                .map_err(|_| "truncated removal operand".into())
        }
        let slot_cmp = operand(&mut read, 0x4bfe40, &[0x83, 0xbb], 5)?;
        let slot_write = operand(&mut read, 0x4bfeab, &[0xc7, 0x83], 8)?;
        let slot_get = operand(&mut read, 0x494994, &[0x8b, 0x80], 4)?;
        if slot_cmp[..4] != slot_write[..4] || slot_get != slot_cmp[..4] {
            return Err("inconsistent native slot-state fields".into());
        }
        let playing = i32::from(slot_cmp[4] as i8);
        let left = word(&slot_write[4..])?;
        let neutral = i32::from(operand(&mut read, 0x4bfe92, &[0x83, 0xf8], 1)?[0] as i8);
        let victory = i32::from(operand(&mut read, 0x4bfebe, &[0x83, 0xee], 1)?[0] as i8);
        let defeat = victory
            .checked_add(i32::from(
                operand(&mut read, 0x4bfec3, &[0x83, 0xee], 1)?[0] as i8,
            ))
            .ok_or("result selector overflow")?;
        let tie = defeat
            .checked_add(i32::from(
                operand(&mut read, 0x4bfec8, &[0x83, 0xee], 1)?[0] as i8,
            ))
            .ok_or("result selector overflow")?;
        // The native slot write is skipped by TEST ESI,ESI / JE: zero is an
        // instruction semantic, not a copied player-result enumerand.
        if read(0x4bfea7, 4)? != [0x85, 0xf6, 0x74, 0x0a] || victory != 0 {
            return Err("unsupported victory slot-preservation branch".into());
        }
        let direct_victory = i32::from(operand(&mut read, 0x4bff28, &[0x83, 0xee], 1)?[0] as i8);
        let direct_defeat = direct_victory
            .checked_add(i32::from(
                operand(&mut read, 0x4bff2d, &[0x83, 0xee], 1)?[0] as i8,
            ))
            .ok_or("direct result selector overflow")?;
        if (direct_victory, direct_defeat) != (victory, defeat) {
            return Err("inconsistent direct/result-dispatch selectors".into());
        }
        let base = word(&operand(&mut read, 0x4add28, &[0x81, 0xc6], 4)?)?;
        let defeat_event = word(&operand(&mut read, 0x4b9477, &[0x68], 4)?)?
            .checked_sub(base)
            .ok_or("event selector overflow")?;
        let victory_event = word(&operand(&mut read, 0x4bcc57, &[0x68], 4)?)?
            .checked_sub(base)
            .ok_or("event selector overflow")?;
        if playing == left
            || defeat_event == victory_event
            || [victory, defeat, tie, neutral]
                .iter()
                .enumerate()
                .any(|(i, v)| [victory, defeat, tie, neutral][..i].contains(v))
        {
            return Err("overlapping removal source states".into());
        }
        Ok(Self {
            origin,
            image_sha256,
            playing,
            left,
            victory,
            defeat,
            tie,
            neutral,
            defeat_event,
            victory_event,
        })
    }
}
#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::collections::BTreeMap;
    fn windows(playing: u8, left: i32, step: u8, base: i32) -> BTreeMap<u32, Vec<u8>> {
        let mut cmp = vec![0x83, 0xbb];
        cmp.extend(0x1234u32.to_le_bytes());
        cmp.push(playing);
        let mut write = vec![0xc7, 0x83];
        write.extend(0x1234u32.to_le_bytes());
        write.extend(left.to_le_bytes());
        let mut get = vec![0x8b, 0x80];
        get.extend(0x1234u32.to_le_bytes());
        let imm = |prefix: &[u8], v: i32| {
            let mut b = prefix.to_vec();
            b.extend(v.to_le_bytes());
            b
        };
        BTreeMap::from([
            (0x4bfe40, cmp),
            (0x4bfeab, write),
            (0x494994, get),
            (0x4bfe92, vec![0x83, 0xf8, 99]),
            (0x4bfebe, vec![0x83, 0xee, 0]),
            (0x4bfec3, vec![0x83, 0xee, step]),
            (0x4bfec8, vec![0x83, 0xee, 3]),
            (0x4bfea7, vec![0x85, 0xf6, 0x74, 0x0a]),
            (0x4bff28, vec![0x83, 0xee, 0]),
            (0x4bff2d, vec![0x83, 0xee, step]),
            (0x4add28, imm(&[0x81, 0xc6], base)),
            (0x4b9477, imm(&[0x68], base + 41)),
            (0x4bcc57, imm(&[0x68], base - 7)),
        ])
    }
    fn decode(map: &BTreeMap<u32, Vec<u8>>) -> Result<Rules, String> {
        Rules::decode(
            AssetIdentity {
                source: "synthetic:removal".into(),
                edition: "synthetic".into(),
                version: "fixture".into(),
            },
            sha256(&[]),
            |va, n| {
                map.get(&va)
                    .and_then(|b| b.get(..n))
                    .map(<[u8]>::to_vec)
                    .ok_or_else(|| "missing fixture window".into())
            },
        )
    }
    pub(crate) fn fixture() -> Rules {
        decode(&windows(17, 23, 7, 700)).unwrap()
    }
    #[test]
    fn transitions_and_event_namespace_follow_source_operands() {
        for (playing, left, step, base) in [(17, 23, 7, 700), (51, 71, 11, 9000)] {
            let r = decode(&windows(playing, left, step, base)).unwrap();
            assert_eq!(
                (r.playing, r.left, r.defeat, r.tie),
                (
                    i32::from(playing),
                    left,
                    i32::from(step),
                    i32::from(step) + 3
                )
            );
            assert_eq!(
                (r.victory, r.neutral, r.defeat_event, r.victory_event),
                (0, 99, 41, -7)
            );
        }
    }
    #[test]
    fn malformed_field_branch_and_truncated_operands_refuse() {
        for va in [0x4bfe40, 0x4bfeab, 0x494994, 0x4bfea7, 0x4add28] {
            let mut map = windows(17, 23, 7, 700);
            map.get_mut(&va).unwrap()[0] ^= 1;
            assert!(decode(&map).is_err());
            let mut map = windows(17, 23, 7, 700);
            map.get_mut(&va).unwrap().pop();
            assert!(decode(&map).is_err());
        }
        assert!(decode(&windows(17, 17, 7, 700)).is_err());
    }
    #[test]
    #[ignore = "requires explicit PEACECRAFT3_PLAYER_REMOVAL_IMAGE and PEACECRAFT3_PLAYER_REMOVAL_COMMON qualified original sources"]
    fn local_original_removal_operands_are_qualified() {
        let p = std::env::var_os("PEACECRAFT3_PLAYER_REMOVAL_IMAGE").expect("explicit local image");
        let b = std::fs::read(&p).unwrap();
        let r = Rules::reviewed_legacy(
            &b,
            AssetIdentity {
                source: std::path::PathBuf::from(p).display().to_string(),
                edition: "selected local legacy".into(),
                version: "PE VERSIONINFO 1.29.2.9231".into(),
            },
        )
        .unwrap();
        assert_eq!(r.image_sha256, sha256(&b));
        assert_ne!(r.playing, r.left);
        let common = std::env::var_os("PEACECRAFT3_PLAYER_REMOVAL_COMMON")
            .expect("explicit selected common.j");
        let script = pcx_script::parse_script(&std::fs::read_to_string(common).unwrap()).unwrap();
        for (name, conversion, expected) in [
            (
                "PLAYER_GAME_RESULT_VICTORY",
                "ConvertPlayerGameResult",
                r.victory,
            ),
            (
                "PLAYER_GAME_RESULT_DEFEAT",
                "ConvertPlayerGameResult",
                r.defeat,
            ),
            ("PLAYER_GAME_RESULT_TIE", "ConvertPlayerGameResult", r.tie),
            (
                "PLAYER_GAME_RESULT_NEUTRAL",
                "ConvertPlayerGameResult",
                r.neutral,
            ),
            (
                "PLAYER_SLOT_STATE_PLAYING",
                "ConvertPlayerSlotState",
                r.playing,
            ),
            ("PLAYER_SLOT_STATE_LEFT", "ConvertPlayerSlotState", r.left),
            ("EVENT_PLAYER_DEFEAT", "ConvertPlayerEvent", r.defeat_event),
            (
                "EVENT_PLAYER_VICTORY",
                "ConvertPlayerEvent",
                r.victory_event,
            ),
        ] {
            use pcx_script::ast::{Expr, Literal};
            let globals = script.globals();
            let matches: Vec<_> = globals.iter().filter(|g| g.name.as_str() == name).collect();
            let [global] = matches.as_slice() else {
                panic!("one selected global {name}")
            };
            assert!(global.is_constant && !global.ty.array);
            let Some(Expr::Call { callee, args, .. }) = &global.init else {
                panic!("conversion initializer {name}")
            };
            assert!(matches!(callee.as_ref(),Expr::Ident(n,_) if n.as_str()==conversion));
            let [Expr::Literal(Literal::Int(value), _)] = args.as_slice() else {
                panic!("selected literal enumerand {name}")
            };
            assert_eq!(pcx_script::Value::wrap32(value.value), expected, "{name}");
        }
    }
}

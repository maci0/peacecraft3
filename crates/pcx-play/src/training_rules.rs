//! Source queue bounds and per-entry cancellation refunds.
use crate::{
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    script_numeric::NumericPolicy,
    scripts::AssetIdentity,
};
use pcx_script::{RealOps, Value, ast::BinOp};

#[derive(Clone, Debug)]
pub struct Rules {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    pub queue_slots: usize,
    pub profile_sources: Vec<String>,
    refund_rate: f32,
}

impl Rules {
    pub fn read(
        image: &[u8],
        origin: AssetIdentity,
        profiles: &[(&str, &[u8])],
        budget: RuleInputBudget,
    ) -> Result<Self, String> {
        let result = (|| {
            if image.len() > budget.image_bytes || sha256(image) != REVIEWED_IMAGE {
                return Err("unsupported client identity or image budget".into());
            }
            let pe = Pe::parse(image, budget).map_err(|e| e.to_string())?;
            Self::decode(
                origin.clone(),
                REVIEWED_IMAGE,
                profiles,
                budget.profile_bytes,
                |va, len, code| {
                    pe.read_from(va, code)
                        .map_err(|e| e.to_string())?
                        .get(..len)
                        .map(<[u8]>::to_vec)
                        .ok_or_else(|| format!("truncated training role at {va:#x}"))
                },
            )
        })();
        result.map_err(|error| format!("{}: training rules: {error}", origin.source))
    }

    fn decode(
        origin: AssetIdentity,
        image_sha256: Sha256Digest,
        profiles: &[(&str, &[u8])],
        profile_bytes: usize,
        mut read: impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
    ) -> Result<Self, String> {
        fn immediate(
            read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
            address: u32,
            opcode: &[u8],
            width: usize,
        ) -> Result<Vec<u8>, String> {
            let data = read(address, opcode.len() + width, true)?;
            if data.len() != opcode.len() + width || !data.starts_with(opcode) {
                return Err(format!("unsupported training instruction at {address:#x}"));
            }
            Ok(data[opcode.len()..].to_vec())
        }
        fn word(data: Vec<u8>) -> Result<u32, String> {
            Ok(u32::from_le_bytes(
                data.try_into().map_err(|_| "training word width")?,
            ))
        }
        if [&origin.source, &origin.edition, &origin.version]
            .iter()
            .any(|s| s.is_empty())
        {
            return Err("missing training source identity".into());
        }
        let slots = word(immediate(&mut read, 0xb56fbf, &[0xbb], 4)?)?;
        let other = word(immediate(&mut read, 0xb573bd, &[0xbb], 4)?)?;
        let pending = immediate(&mut read, 0x6324d1, &[0x83, 0xfb], 1)?[0];
        let allocated = immediate(&mut read, 0x63241f, &[0x6a], 1)?[0];
        if slots == 0
            || slots > u32::BITS
            || other != slots
            || pending & 0x80 != 0
            || pending != allocated
            || u32::from(pending).checked_add(1) != Some(slots)
        {
            return Err("training backend and pending UI bounds differ".into());
        }
        for (address, target) in [
            (0xb5724b_u32, 0x9a1620_u32),
            (0xb57258, 0x99ffe0),
            (0xb5725e, 0x9a07b0),
            (0xb5726a, 0x4b5820),
            (0xb57280, 0x9a1620),
            (0xb5728d, 0x99ffe0),
            (0xb57293, 0x9a07b0),
            (0xb5729f, 0x4b5860),
        ] {
            let delta = word(immediate(&mut read, address, &[0xe8], 4)?)?;
            if address.wrapping_add(5).wrapping_add(delta) != target {
                return Err("training refund numeric/credit path differs".into());
            }
        }
        let pointer = word(immediate(&mut read, 0xb571b1, &[0x68], 4)?)?;
        if read(pointer, b"TrainRefundRate\0".len(), false)? != b"TrainRefundRate\0" {
            return Err("training refund profile key differs".into());
        }
        let ([rate], profile_sources) =
            pcx_objects::editor::misc::misc_scalars(profiles, ["TrainRefundRate"], profile_bytes)?;
        Ok(Self {
            origin,
            image_sha256,
            queue_slots: usize::try_from(slots).map_err(|_| "training slot width")?,
            profile_sources,
            refund_rate: rate.ok_or("missing Misc/TrainRefundRate")?,
        })
    }

    pub fn refund(&self, numeric: &NumericPolicy, cost: i32) -> Result<i32, String> {
        if numeric.image_sha256() != &self.image_sha256 || cost < 0 {
            return Err("training refund source identity or cost differs".into());
        }
        let Value::Real(value) =
            numeric.binary(BinOp::Mul, numeric.integer(cost)?, self.refund_rate)?
        else {
            return Err("training refund product is not real".into());
        };
        if !value.is_finite()
            || f64::from(value) < f64::from(i32::MIN)
            || f64::from(value) > f64::from(i32::MAX)
        {
            return Err("training refund integer class is unsupported".into());
        }
        Ok(value.trunc() as i32)
    }
}

#[cfg(any(test, feature = "test-fixtures"))]
impl Rules {
    pub fn fixture(queue_slots: usize, refund_rate: f32) -> Self {
        Self {
            origin: AssetIdentity {
                source: "synthetic training".into(),
                edition: "fixture".into(),
                version: "nonstock".into(),
            },
            image_sha256: [0x42; 32],
            queue_slots,
            profile_sources: vec!["synthetic".into()],
            refund_rate,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn instructions(slots: u32) -> BTreeMap<u32, Vec<u8>> {
        let mut data = BTreeMap::new();
        for address in [0xb56fbf, 0xb573bd] {
            data.insert(address, [vec![0xbb], slots.to_le_bytes().to_vec()].concat());
        }
        data.insert(0x6324d1, vec![0x83, 0xfb, (slots - 1) as u8]);
        data.insert(0x63241f, vec![0x6a, (slots - 1) as u8]);
        for (address, target) in [
            (0xb5724b_u32, 0x9a1620_u32),
            (0xb57258, 0x99ffe0),
            (0xb5725e, 0x9a07b0),
            (0xb5726a, 0x4b5820),
            (0xb57280, 0x9a1620),
            (0xb5728d, 0x99ffe0),
            (0xb57293, 0x9a07b0),
            (0xb5729f, 0x4b5860),
        ] {
            data.insert(
                address,
                [
                    vec![0xe8],
                    target.wrapping_sub(address + 5).to_le_bytes().to_vec(),
                ]
                .concat(),
            );
        }
        data.insert(
            0xb571b1,
            [vec![0x68], 0x1234_u32.to_le_bytes().to_vec()].concat(),
        );
        data.insert(0x1234, b"TrainRefundRate\0".to_vec());
        data
    }

    fn decode(data: &BTreeMap<u32, Vec<u8>>, profiles: &[(&str, &[u8])]) -> Result<Rules, String> {
        Rules::decode(
            Rules::fixture(1, 0.).origin,
            [0x42; 32],
            profiles,
            4096,
            |address, length, _| {
                data.get(&address)
                    .and_then(|v| v.get(..length))
                    .map(<[u8]>::to_vec)
                    .ok_or("missing test window".into())
            },
        )
    }

    #[test]
    fn source_bounds_and_profile_overrides_drive_rules() {
        let rules = decode(
            &instructions(5),
            &[
                ("base", b"[Misc]\nTrainRefundRate=0.75\n"),
                ("map", b"[Misc]\nTrainRefundRate=0.37\n"),
            ],
        )
        .unwrap();
        assert_eq!(rules.queue_slots, 5);
        assert_eq!(rules.refund_rate, 0.37);
        assert!(decode(&instructions(5), &[("missing", b"[Misc]\n")]).is_err());
        assert!(
            decode(
                &instructions(5),
                &[("bad", b"[Misc]\nTrainRefundRate=NaN\n")]
            )
            .is_err()
        );
    }

    #[test]
    fn inconsistent_or_truncated_source_roles_refuse() {
        let profiles: &[(&str, &[u8])] = &[("base", b"[Misc]\nTrainRefundRate=0.5\n")];
        let source = instructions(5);
        for address in source.keys() {
            let mut mutant = source.clone();
            mutant.get_mut(address).unwrap().pop();
            assert!(decode(&mutant, profiles).is_err(), "{address:#x}");
        }
        for address in [0xb56fbf, 0xb573bd, 0x6324d1, 0x63241f, 0xb57258, 0x1234] {
            let mut mutant = source.clone();
            *mutant.get_mut(&address).unwrap().last_mut().unwrap() ^= 1;
            assert!(decode(&mutant, profiles).is_err(), "{address:#x}");
        }
    }
    #[test]
    fn refunds_truncate_each_entry_and_require_matching_numeric_source() {
        let (image, binding, budget) = crate::script_numeric::tests::fixture();
        let numeric = NumericPolicy::read(&image, binding, budget, 64).unwrap();
        let mut rules = Rules::fixture(5, 0.37);
        assert!(rules.refund(&numeric, 235).is_err());
        rules.image_sha256 = *numeric.image_sha256();
        assert_eq!(rules.refund(&numeric, 235).unwrap(), 86);
        assert_eq!(rules.refund(&numeric, 1).unwrap(), 0);
        assert!(rules.refund(&numeric, -1).is_err());
        rules.refund_rate = f32::INFINITY;
        assert!(rules.refund(&numeric, 235).is_err());
    }

    #[test]
    #[ignore = "requires PCX_WAR3_EXE and PCX_TRAINING_MISC original inputs"]
    fn reviewed_original_image_and_profile_decode() {
        let image = std::fs::read(std::env::var("PCX_WAR3_EXE").unwrap()).unwrap();
        let profile = std::fs::read(std::env::var("PCX_TRAINING_MISC").unwrap()).unwrap();
        let rules = Rules::read(
            &image,
            AssetIdentity {
                source: "explicit reviewed client".into(),
                edition: "classic".into(),
                version: "selected".into(),
            },
            &[("explicit original MiscGame", &profile)],
            RuleInputBudget {
                image_bytes: 32 << 20,
                profile_bytes: 8 << 20,
                sections: 96,
            },
        )
        .unwrap();
        assert_eq!(rules.image_sha256, REVIEWED_IMAGE);
        assert_eq!(rules.queue_slots, 7);
        assert_eq!(rules.refund_rate, 1.);
    }
}

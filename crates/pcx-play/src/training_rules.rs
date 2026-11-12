//! Source queue bounds and per-entry cancellation refunds.
use crate::{
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    script_numeric::NumericPolicy,
    scripts::AssetIdentity,
};
use pcx_script::{RealOps, Value, ast::BinOp};

#[derive(Clone, Copy)]
pub(crate) enum RefundKind {
    Training,
    Research,
    StructureUpgrade,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct UpgradePolicy {
    pub refund_rate: f32,
    pub relative_cost: bool,
    pub damage_penalty: bool,
}

#[derive(Clone, Debug)]
pub struct Rules {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    pub queue_slots: usize,
    pub(crate) ai_production: Result<crate::campaign_production_rules::Rules, String>,
    pub profile_sources: Vec<String>,
    refund_rate: f32,
    research_refund_rate: Option<f32>,
    upgrade: Result<UpgradePolicy, String>,
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
        result
            .map(|mut rules| {
                rules.ai_production = crate::campaign_production_rules::Rules::read(image, budget);
                rules
            })
            .map_err(|error| format!("{}: training rules: {error}", origin.source))
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
            (0xb570e0_u32, 0x430620_u32),
            (0xb570f8, 0x6df7e0),
            (0xb57136, 0x6e0a30),
            (0xb57102, 0x9a1620),
            (0xb5710f, 0x99ffe0),
            (0xb57115, 0x9a07b0),
            (0xb57121, 0x4b5820),
            (0xb57140, 0x9a1620),
            (0xb5714d, 0x99ffe0),
            (0xb57153, 0x9a07b0),
            (0xb5715f, 0x4b5860),
            (0xb5724b, 0x9a1620),
            (0xb57258, 0x99ffe0),
            (0xb5725e, 0x9a07b0),
            (0xb5726a, 0x4b5820),
            (0xb57280, 0x9a1620),
            (0xb5728d, 0x99ffe0),
            (0xb57293, 0x9a07b0),
            (0xb5729f, 0x4b5860),
            (0xb63e0a, 0x430620),
            (0xb63e1b, 0x4304c0),
            (0xb635d2, 0x4304c0),
            (0xb63b6a, 0x4304c0),
            (0xb635af, 0x6939c0),
            (0xb635bc, 0x6971a0),
            (0xb635e1, 0x6939c0),
            (0xb635ec, 0x6971a0),
            (0xb63622, 0x4b5820),
            (0xb63633, 0x4b5860),
            (0xb63b4b, 0x6939c0),
            (0xb63b56, 0x6971a0),
            (0xb63b79, 0x6939c0),
            (0xb63b84, 0x6971a0),
            (0xb63ba0, 0x9a1620),
            (0xb63bad, 0x99ffe0),
            (0xb63bb3, 0x9a07b0),
            (0xb63bbf, 0x4b5820),
            (0xb63bd5, 0x9a1620),
            (0xb63be2, 0x99ffe0),
            (0xb63be8, 0x9a07b0),
            (0xb63bf4, 0x4b5860),
        ] {
            let delta = word(immediate(&mut read, address, &[0xe8], 4)?)?;
            if address.wrapping_add(5).wrapping_add(delta) != target {
                return Err("training refund numeric/credit path differs".into());
            }
        }
        for (address, key) in [
            (0xb571b1, "TrainRefundRate"),
            (0xb570d2, "ResearchRefundRate"),
            (0xb63dfd, "UpgradeRefundRate"),
            (0xb63e11, "UpgradeDamageRefundPenalty"),
            (0xb635c5, "RelativeUpgradeCost"),
            (0xb63b5d, "RelativeUpgradeCost"),
        ] {
            let pointer = word(immediate(&mut read, address, &[0x68], 4)?)?;
            let terminated = format!("{key}\0");
            if read(pointer, terminated.len(), false)? != terminated.as_bytes() {
                return Err(format!("{key} refund profile key differs"));
            }
        }
        let ([rate, research_rate, upgrade_rate, relative, damage], profile_sources) =
            pcx_objects::editor::misc::misc_scalars(
                profiles,
                [
                    "TrainRefundRate",
                    "ResearchRefundRate",
                    "UpgradeRefundRate",
                    "RelativeUpgradeCost",
                    "UpgradeDamageRefundPenalty",
                ],
                profile_bytes,
            )?;
        let boolean = |key: &str, value: Option<f32>| match value {
            Some(0.) => Ok(false),
            Some(1.) => Ok(true),
            Some(_) => Err(format!("Misc/{key} boolean class is unsupported")),
            None => Err(format!("missing Misc/{key}")),
        };
        let upgrade = (|| {
            Ok(UpgradePolicy {
                refund_rate: upgrade_rate.ok_or("missing Misc/UpgradeRefundRate")?,
                relative_cost: boolean("RelativeUpgradeCost", relative)?,
                damage_penalty: boolean("UpgradeDamageRefundPenalty", damage)?,
            })
        })();
        Ok(Self {
            origin,
            image_sha256,
            ai_production: Err("AI production not admitted".into()),
            queue_slots: usize::try_from(slots).map_err(|_| "training slot width")?,
            profile_sources,
            refund_rate: rate.ok_or("missing Misc/TrainRefundRate")?,
            research_refund_rate: research_rate,
            upgrade,
        })
    }

    pub fn refund(&self, numeric: &NumericPolicy, cost: i32) -> Result<i32, String> {
        self.refund_at_rate(numeric, cost, self.refund_rate)
    }

    pub fn research_refund(&self, numeric: &NumericPolicy, cost: i32) -> Result<i32, String> {
        self.refund_at_rate(
            numeric,
            cost,
            self.research_refund_rate
                .ok_or("missing Misc/ResearchRefundRate")?,
        )
    }

    pub(crate) fn upgrade_policy(&self) -> Result<UpgradePolicy, String> {
        let policy = *self.upgrade.as_ref().map_err(Clone::clone)?;
        if policy.damage_penalty {
            return Err("upgrade damage refund penalty is not qualified".into());
        }
        Ok(policy)
    }

    pub(crate) fn upgrade_costs(
        &self,
        original: Option<[i32; 2]>,
        target: [i32; 2],
    ) -> Result<[i32; 2], String> {
        let policy = self.upgrade_policy()?;
        let mut costs = target;
        if !policy.relative_cost {
            let original = original.ok_or("original upgrade cost not admitted")?;
            for (cost, original) in costs.iter_mut().zip(original) {
                *cost = cost.checked_sub(original).ok_or("upgrade cost overflow")?;
            }
        }
        for cost in costs {
            if cost < 0 {
                return Err("negative upgrade cost is not qualified".into());
            }
        }
        Ok(costs)
    }

    pub(crate) fn refund_for(
        &self,
        numeric: &NumericPolicy,
        cost: i32,
        kind: RefundKind,
    ) -> Result<i32, String> {
        match kind {
            RefundKind::Training => self.refund(numeric, cost),
            RefundKind::Research => self.research_refund(numeric, cost),
            RefundKind::StructureUpgrade => {
                self.refund_at_rate(numeric, cost, self.upgrade_policy()?.refund_rate)
            }
        }
    }

    fn refund_at_rate(&self, numeric: &NumericPolicy, cost: i32, rate: f32) -> Result<i32, String> {
        if numeric.image_sha256() != &self.image_sha256 || cost < 0 {
            return Err("training refund source identity or cost differs".into());
        }
        let Value::Real(value) = numeric.binary(BinOp::Mul, numeric.integer(cost)?, rate)? else {
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
        Self::fixture_with_refunds(queue_slots, refund_rate, Some(refund_rate))
    }

    pub(crate) fn fixture_with_refunds(
        queue_slots: usize,
        refund_rate: f32,
        research_refund_rate: Option<f32>,
    ) -> Self {
        Self {
            origin: AssetIdentity {
                source: "synthetic training".into(),
                edition: "fixture".into(),
                version: "nonstock".into(),
            },
            image_sha256: [0x42; 32],
            queue_slots,
            ai_production: Err("synthetic AI production not admitted".into()),
            profile_sources: vec!["synthetic".into()],
            refund_rate,
            research_refund_rate,
            upgrade: Err("synthetic upgrade policy not admitted".into()),
        }
    }

    pub(crate) fn with_upgrade_fixture(mut self, policy: Option<UpgradePolicy>) -> Self {
        self.upgrade = policy.ok_or_else(|| "synthetic upgrade policy not admitted".into());
        self
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
            (0xb570e0_u32, 0x430620_u32),
            (0xb570f8, 0x6df7e0),
            (0xb57136, 0x6e0a30),
            (0xb57102, 0x9a1620),
            (0xb5710f, 0x99ffe0),
            (0xb57115, 0x9a07b0),
            (0xb57121, 0x4b5820),
            (0xb57140, 0x9a1620),
            (0xb5714d, 0x99ffe0),
            (0xb57153, 0x9a07b0),
            (0xb5715f, 0x4b5860),
            (0xb5724b, 0x9a1620),
            (0xb57258, 0x99ffe0),
            (0xb5725e, 0x9a07b0),
            (0xb5726a, 0x4b5820),
            (0xb57280, 0x9a1620),
            (0xb5728d, 0x99ffe0),
            (0xb57293, 0x9a07b0),
            (0xb5729f, 0x4b5860),
            (0xb63e0a, 0x430620),
            (0xb63e1b, 0x4304c0),
            (0xb635d2, 0x4304c0),
            (0xb63b6a, 0x4304c0),
            (0xb635af, 0x6939c0),
            (0xb635bc, 0x6971a0),
            (0xb635e1, 0x6939c0),
            (0xb635ec, 0x6971a0),
            (0xb63622, 0x4b5820),
            (0xb63633, 0x4b5860),
            (0xb63b4b, 0x6939c0),
            (0xb63b56, 0x6971a0),
            (0xb63b79, 0x6939c0),
            (0xb63b84, 0x6971a0),
            (0xb63ba0, 0x9a1620),
            (0xb63bad, 0x99ffe0),
            (0xb63bb3, 0x9a07b0),
            (0xb63bbf, 0x4b5820),
            (0xb63bd5, 0x9a1620),
            (0xb63be2, 0x99ffe0),
            (0xb63be8, 0x9a07b0),
            (0xb63bf4, 0x4b5860),
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
        data.insert(
            0xb570d2,
            [vec![0x68], 0x2345_u32.to_le_bytes().to_vec()].concat(),
        );
        data.insert(0x2345, b"ResearchRefundRate\0".to_vec());
        for (address, pointer, key) in [
            (0xb63dfd, 0x3456_u32, "UpgradeRefundRate"),
            (0xb63e11, 0x4567, "UpgradeDamageRefundPenalty"),
            (0xb635c5, 0x5678, "RelativeUpgradeCost"),
            (0xb63b5d, 0x5678, "RelativeUpgradeCost"),
        ] {
            data.insert(
                address,
                [vec![0x68], pointer.to_le_bytes().to_vec()].concat(),
            );
            data.insert(pointer, format!("{key}\0").into_bytes());
        }
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
                (
                    "base",
                    b"[Misc]\nTrainRefundRate=0.75\nResearchRefundRate=0.91\n",
                ),
                (
                    "map",
                    b"[Misc]\nTrainRefundRate=0.37\nResearchRefundRate=0.61\n",
                ),
            ],
        )
        .unwrap();
        assert_eq!(rules.queue_slots, 5);
        assert_eq!(rules.refund_rate, 0.37);
        assert_eq!(rules.research_refund_rate, Some(0.61));
        let missing = decode(
            &instructions(5),
            &[("train-only", b"[Misc]\nTrainRefundRate=0.4\n")],
        )
        .unwrap();
        assert_eq!(missing.research_refund_rate, None);
        assert!(
            decode(
                &instructions(5),
                &[(
                    "bad",
                    b"[Misc]\nTrainRefundRate=0.4\nResearchRefundRate=NaN\n"
                )]
            )
            .is_err()
        );
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
        for address in [
            0xb56fbf, 0xb573bd, 0x6324d1, 0x63241f, 0xb57258, 0x1234, 0xb570d2, 0x2345, 0xb5710f,
        ] {
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
        rules.research_refund_rate = Some(0.61);
        assert_eq!(rules.research_refund(&numeric, 235).unwrap(), 143);
        rules.research_refund_rate = None;
        assert!(
            rules
                .research_refund(&numeric, 235)
                .unwrap_err()
                .contains("ResearchRefundRate")
        );
        assert_eq!(rules.refund(&numeric, 235).unwrap(), 86);
        rules.refund_rate = f32::INFINITY;
        assert!(rules.refund(&numeric, 235).is_err());
    }

    #[test]
    fn upgrade_cost_modes_and_rates_follow_ordered_profiles_without_training_fallback() {
        let source = instructions(5);
        let base = b"[Misc]\nTrainRefundRate=.37\nResearchRefundRate=.61\nUpgradeRefundRate=.83\nRelativeUpgradeCost=0\nUpgradeDamageRefundPenalty=0\n";
        let rules = decode(&source, &[("base", base)]).unwrap();
        assert_eq!(
            rules.upgrade_costs(Some([31, 7]), [74, 23]).unwrap(),
            [43, 16]
        );
        assert!(
            rules
                .upgrade_costs(None, [74, 23])
                .unwrap_err()
                .contains("original")
        );
        let overridden = decode(
            &source,
            &[
                ("base", base),
                (
                    "map",
                    b"[Misc]\nUpgradeRefundRate=.23\nRelativeUpgradeCost=1\n",
                ),
            ],
        )
        .unwrap();
        assert_eq!(overridden.upgrade_costs(None, [74, 23]).unwrap(), [74, 23]);
        assert_eq!(overridden.upgrade_policy().unwrap().refund_rate, 0.23);
        assert_eq!(overridden.refund_rate, 0.37);
        assert_eq!(overridden.research_refund_rate, Some(0.61));
        assert!(
            rules
                .upgrade_costs(Some([75, 7]), [74, 23])
                .unwrap_err()
                .contains("negative")
        );
        assert!(
            rules
                .upgrade_costs(Some([i32::MIN, 0]), [i32::MAX, 0])
                .unwrap_err()
                .contains("overflow")
        );
        let missing = decode(&source, &[("missing", b"[Misc]\nTrainRefundRate=.37\n")]).unwrap();
        assert!(
            missing
                .upgrade_costs(Some([31, 7]), [74, 23])
                .unwrap_err()
                .contains("UpgradeRefundRate")
        );
        for (key, value) in [
            ("RelativeUpgradeCost", "2"),
            ("UpgradeDamageRefundPenalty", "2"),
        ] {
            let map = format!("[Misc]\n{key}={value}\n");
            let rules = decode(&source, &[("base", base), ("bad", map.as_bytes())]).unwrap();
            assert!(rules.upgrade_costs(Some([31, 7]), [74, 23]).is_err());
        }
        let damaged = decode(
            &source,
            &[
                ("base", base),
                ("map", b"[Misc]\nUpgradeDamageRefundPenalty=1\n"),
            ],
        )
        .unwrap();
        assert!(
            damaged
                .upgrade_costs(Some([31, 7]), [74, 23])
                .unwrap_err()
                .contains("damage")
        );
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
        assert!(rules.research_refund_rate.is_some());
        assert!(rules.upgrade_costs(Some([0; 2]), [0; 2]).is_ok());
    }
}

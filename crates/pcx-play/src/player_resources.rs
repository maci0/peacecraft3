//! Selected client state-counter initialization and money assignment bounds.
use crate::neutral_player::{RuleInputBudget, Sha256Digest, sha256};
use crate::neutral_rule_input::Pe;
use crate::scripts::AssetIdentity;

#[derive(Clone, Debug)]
pub struct Rules {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    pub profile_sources: Vec<String>,
    pub(crate) initial: i32,
    pub(crate) initialized_states: i32,
    pub(crate) money_states: [i32; 2],
    pub(crate) money_scale: i32,
    pub(crate) money_maximum: i32,
    pub(crate) food_limit: i32,
}
fn word(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}
fn call(bytes: &[u8], va: u32, at: usize) -> Option<u32> {
    (bytes.get(at) == Some(&0xe8)).then_some(())?;
    Some(
        va.wrapping_add(at as u32)
            .wrapping_add(5)
            .wrapping_add(word(bytes, at + 1)?),
    )
}
fn shape(bytes: &[u8], parts: &[(usize, &[u8])]) -> bool {
    parts
        .iter()
        .all(|(at, part)| bytes.get(*at..at + part.len()) == Some(*part))
}
impl Rules {
    /// Profiles have the same ordered stock/map provenance as other `[Misc]` rules.
    pub fn discover(
        image: &[u8],
        origin: AssetIdentity,
        profiles: &[(&str, &[u8])],
        budget: RuleInputBudget,
    ) -> Result<Self, String> {
        let fail = |s: &str| format!("{}: player resources: {s}", origin.source);
        if image.len() > budget.image_bytes
            || [&origin.source, &origin.edition, &origin.version]
                .iter()
                .any(|s| s.trim().is_empty())
        {
            return Err(fail("input identity or image budget"));
        }
        let pe = Pe::parse(image, budget).map_err(|e| fail(&e.to_string()))?;
        let entry = |name: &[u8], signature: &[u8]| -> Result<u32, String> {
            let mut found = Vec::new();
            for (_, code) in pe.code_ranges() {
                for bytes in code.windows(20) {
                    if bytes[0] != 0x68
                        || bytes[5] != 0x68
                        || bytes[10] != 0x68
                        || bytes[15] != 0xe8
                    {
                        continue;
                    }
                    let string = |at| pe.read_from(word(bytes, at).unwrap_or(0), false).ok();
                    if !string(6).is_some_and(|s| s.starts_with(name))
                        || !string(1).is_some_and(|s| s.starts_with(signature))
                    {
                        continue;
                    }
                    let target = word(bytes, 11).ok_or_else(|| fail("native registration"))?;
                    let code = pe
                        .read_from(target, true)
                        .map_err(|e| fail(&e.to_string()))?;
                    if code.starts_with(&[0x55, 0x8b, 0xec]) && !found.contains(&target) {
                        found.push(target);
                    }
                }
            }
            match found.as_slice() {
                [target] => Ok(*target),
                _ => Err(fail("missing or ambiguous native implementation")),
            }
        };
        let get_va = entry(b"GetPlayerState\0", b"(Hplayer;Hplayerstate;)I\0")?;
        let set_va = entry(b"SetPlayerState\0", b"(Hplayer;Hplayerstate;I)V\0")?;
        let get = pe
            .read_from(get_va, true)
            .map_err(|e| fail(&e.to_string()))?;
        let set = pe
            .read_from(set_va, true)
            .map_err(|e| fail(&e.to_string()))?;
        if !shape(
            get,
            &[
                (0, &[0x55, 0x8b, 0xec, 0xff, 0x75, 8, 0xe8]),
                (11, &[0x83, 0xc4, 4, 0x85, 0xc0, 0x75, 2, 0x5d, 0xc3]),
                (20, &[0x56, 0x8b, 0x75, 12, 0x83, 0xfe]),
                (34, &[0x8d, 0x4e, 4, 0xc1, 0xe1, 4, 0x57, 3, 0xc8, 0xe8]),
                (48, &[0x8d, 0x4e]),
                (51, &[0x99, 0xbf]),
                (57, &[0x3b, 0xf9, 0x1b, 0xc9, 0x83, 0xe1]),
                (64, &[0x83, 0xc1]),
                (67, &[0xf7, 0xf9]),
            ],
        ) || !shape(
            set,
            &[
                (0, &[0x55, 0x8b, 0xec, 0x56, 0xff, 0x75, 8, 0xe8]),
                (12, &[0x8b, 0xf0, 0x83, 0xc4, 4, 0x85, 0xf6, 0x74]),
                (70, &[0xc3, 0x8b, 0x45, 16, 0x8b, 0xce, 0x50, 0x52, 0xe8]),
            ],
        ) || call(get, get_va, 6) != call(set, set_va, 7)
        {
            return Err(fail("unsupported player-state wrapper shapes"));
        }
        let helper_va = call(set, set_va, 78).ok_or_else(|| fail("state setter target"))?;
        let helper = pe
            .read_from(helper_va, true)
            .map_err(|e| fail(&e.to_string()))?;
        if !shape(
            helper,
            &[
                (0, &[0x55, 0x8b, 0xec, 0x56, 0x8b, 0xf1, 0xba]),
                (11, &[0x8b, 0x4d, 8, 0x8d, 0x41]),
                (17, &[0x3b, 0xd0, 0x8d, 0x41]),
                (22, &[0x1b, 0xd2, 0x83, 0xe2]),
                (27, &[0x83, 0xc2]),
                (30, &[0x0f, 0xaf, 0x55, 12, 0x83, 0xf8]),
                (37, &[0x77, 0x13, 0x81, 0xfa]),
                (45, &[0x76, 11, 0xc1, 0xfa, 0x1f, 0xf7, 0xd2, 0x81, 0xe2]),
                (58, &[0x83, 0xc1, 4, 0xc1, 0xe1, 4, 0x52, 3, 0xce, 0xe8]),
            ],
        ) {
            return Err(fail("unsupported money clamp"));
        }
        let first = -i32::from(*get.get(50).ok_or_else(|| fail("money state range"))? as i8);
        let count = word(get, 53).ok_or_else(|| fail("money state count"))?;
        let scale = i32::from(*get.get(66).ok_or_else(|| fail("money scale"))? as i8);
        let maximum = word(helper, 41).ok_or_else(|| fail("money maximum"))? as i32;
        if count != 1
            || first != -i32::from(helper[16] as i8)
            || helper[36] != count as u8
            || helper[29] as i8 as i32 != scale
            || helper[26] != get[63]
            || word(helper, 54) != Some(maximum as u32)
            || scale <= 0
            || maximum < 0
        {
            return Err(fail("inconsistent source money operands"));
        }
        let mut initializers = Vec::new();
        for (code_va, code) in pe.code_ranges() {
            for (offset, b) in code.windows(79).enumerate() {
                if !shape(
                    b,
                    &[
                        (0, &[0xbe]),
                        (5, &[0x85, 0xdb, 0x74]),
                        (9, &[0x0f, 0x28, 5]),
                        (
                            16,
                            &[
                                0x8d, 0x45, 0xd0, 0x8b, 0x4d, 0xcc, 0x6a, 0, 0x50, 0x0f, 0x11,
                                0x45, 0xd0, 0x53, 0x0f, 0x57, 0xc0,
                            ],
                        ),
                        (62, &[0xe8]),
                        (67, &[0x83, 0xc6]),
                        (70, &[0x47, 0x81, 0xfe]),
                    ],
                ) {
                    continue;
                }
                let base = word(b, 1).unwrap_or(0);
                let stride = u32::from(b[69]);
                let end = word(b, 73).unwrap_or(0);
                if base
                    != u32::from(get[36])
                        .checked_shl(u32::from(get[39]))
                        .unwrap_or(0)
                    || stride != 1u32.checked_shl(u32::from(get[39])).unwrap_or(0)
                    || end <= base
                    || (end - base) / stride <= first as u32 + count
                {
                    continue;
                }
                let binder_va = call(b, code_va + offset as u32, 62)
                    .ok_or_else(|| fail("state schema binding"))?;
                let binder = pe
                    .read_from(binder_va, true)
                    .map_err(|e| fail(&e.to_string()))?;
                if !shape(
                    binder,
                    &[
                        (
                            19,
                            &[
                                0x56, 0xff, 0x75, 0x14, 0x8b, 0x75, 0x10, 0x8b, 0xcf, 0x56, 0xe8,
                            ],
                        ),
                        (34, &[0xff, 0x76]),
                        (37, &[0x8b, 0x75, 8, 0x8b, 0xcf, 0x6a, 1, 0x56, 0xe8]),
                    ],
                ) {
                    continue;
                }
                let template = pe
                    .read_from(word(b, 12).unwrap_or(0), false)
                    .map_err(|e| fail(&e.to_string()))?;
                let value_get_va =
                    call(get, get_va, 43).ok_or_else(|| fail("counter getter target"))?;
                let value_get = pe
                    .read_from(value_get_va, true)
                    .map_err(|e| fail(&e.to_string()))?;
                let value_set_va =
                    call(helper, helper_va, 67).ok_or_else(|| fail("counter setter target"))?;
                let value_set = pe
                    .read_from(value_set_va, true)
                    .map_err(|e| fail(&e.to_string()))?;
                if !shape(
                    value_get,
                    &[
                        (0, &[0xff, 0x71, 12, 0xff, 0x71, 8, 0xe8]),
                        (31, &[0x8b, 0x40]),
                        (34, &[0xc3]),
                    ],
                ) || !shape(
                    value_set,
                    &[
                        (37, &[0x8b, 0x4a]),
                        (40, &[0x8b, 0x45, 8, 0x6a, 0, 0x51, 0x8b, 0xca, 0x89, 0x42]),
                        (51, &[0xe8]),
                    ],
                ) || value_get[33] != value_set[39]
                    || value_get[33] != value_set[50]
                {
                    return Err(fail("counter value field relationship"));
                }
                let tags = [
                    word(template, 0).ok_or_else(|| fail("state template kind"))?,
                    word(template, 4).ok_or_else(|| fail("state template class"))?,
                ];
                let mut constructors = Vec::new();
                for (_, code) in pe.code_ranges() {
                    for factory in code.windows(56) {
                        if shape(
                            factory,
                            &[
                                (0, &[0xc7, 7]),
                                (6, &[0xc7, 0x47]),
                                (
                                    13,
                                    &[
                                        0xeb, 5, 0x8b, 6, 0x89, 0x43, 0x14, 0xff, 0x43, 0x18, 0x8d,
                                        0x46, 4, 0x8b, 0x4d, 8, 0x89, 0x48, 4, 0x8b, 0x4d, 12,
                                        0x5f, 0xc7, 0x40, 12,
                                    ],
                                ),
                                (43, &[0xc7, 0x40, 16]),
                                (50, &[0x89, 0x48, 8, 0xff, 0x43, 0x1c]),
                            ],
                        ) && factory[8] == value_get[33]
                            && word(factory, 39) == Some(tags[0])
                            && word(factory, 46) == Some(tags[1])
                        {
                            constructors.push(
                                word(factory, 9).ok_or_else(|| fail("counter initializer value"))?
                                    as i32,
                            );
                        }
                    }
                }
                let [initial] = constructors.as_slice() else {
                    return Err(fail("missing or ambiguous counter constructor"));
                };
                initializers.push((*initial, (end - base) / stride));
            }
        }
        let [(initial, initialized_states)] = initializers.as_slice() else {
            return Err(fail("missing or ambiguous state initialization"));
        };
        let ([food_limit], profile_sources) =
            pcx_objects::editor::misc::misc_scalars(profiles, ["FoodLimit"], budget.profile_bytes)?;
        let food_limit = food_limit
            .filter(|v| *v >= 0.0 && *v < i32::MAX as f32 && v.fract() == 0.0)
            .ok_or_else(|| fail("missing integral Misc/FoodLimit"))?
            as i32;
        Ok(Self {
            origin,
            image_sha256: sha256(image),
            profile_sources,
            initial: *initial,
            initialized_states: *initialized_states as i32,
            money_states: [first, first + count as i32],
            money_scale: scale,
            money_maximum: maximum,
            food_limit,
        })
    }
    pub(crate) fn money(&self, value: i32) -> i32 {
        let raw = value.wrapping_mul(self.money_scale);
        let raw = if raw as u32 > self.money_maximum as u32 {
            if raw < 0 { 0 } else { self.money_maximum }
        } else {
            raw
        };
        raw / self.money_scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires PCX_PLAYER_RESOURCE_IMAGE selecting a locally supplied client executable"]
    fn supplied_image_discovers_state_defaults_and_money_bounds() {
        let path = std::env::var("PCX_PLAYER_RESOURCE_IMAGE").expect("PCX_PLAYER_RESOURCE_IMAGE");
        let bytes = std::fs::read(&path).unwrap();
        let rules = Rules::discover(
            &bytes,
            AssetIdentity {
                source: path,
                edition: "explicit local image".into(),
                version: "caller-selected; identity retained".into(),
            },
            &[("synthetic food profile", b"[Misc]\nFoodLimit=73\n")],
            RuleInputBudget {
                image_bytes: 32 << 20,
                profile_bytes: 1 << 20,
                sections: 32,
            },
        )
        .unwrap();
        assert_eq!(rules.food_limit, 73);
        assert!(rules.money_scale > 0);
        eprintln!(
            "initial={} money_states={:?} scale={} cap={} sha256={:02x?}",
            rules.initial,
            rules.money_states,
            rules.money_scale,
            rules.money_maximum,
            rules.image_sha256
        );
    }
}

#[cfg(test)]
pub(crate) mod fixture {
    use super::*;
    pub fn image() -> (Vec<u8>, AssetIdentity, RuleInputBudget) {
        let (mut image, binding, budget) = crate::order_registry::tests::fixture([19, 37]);
        image[0x200..].fill(0);
        let va = |raw: usize| 0x301000u32 + (raw - 0x200) as u32;
        let mut put = |raw: usize, bytes: &[u8]| {
            image[raw..raw + bytes.len()].copy_from_slice(bytes);
        };
        let mut get = vec![0u8; 97];
        for (at, b) in [
            (0, &[0x55, 0x8b, 0xec, 0xff, 0x75, 8, 0xe8][..]),
            (11, &[0x83, 0xc4, 4, 0x85, 0xc0, 0x75, 2, 0x5d, 0xc3]),
            (20, &[0x56, 0x8b, 0x75, 12, 0x83, 0xfe, 26]),
            (34, &[0x8d, 0x4e, 4, 0xc1, 0xe1, 4, 0x57, 3, 0xc8, 0xe8]),
            (48, &[0x8d, 0x4e, 0xff, 0x99, 0xbf]),
            (
                57,
                &[
                    0x3b, 0xf9, 0x1b, 0xc9, 0x83, 0xe1, 0xfb, 0x83, 0xc1, 6, 0xf7, 0xf9,
                ],
            ),
        ] {
            get[at..at + b.len()].copy_from_slice(b);
        }
        get[53..57].copy_from_slice(&1u32.to_le_bytes());
        get[7..11].copy_from_slice(&va(0x750).wrapping_sub(va(0x280) + 11).to_le_bytes());
        get[44..48].copy_from_slice(&va(0x480).wrapping_sub(va(0x280) + 48).to_le_bytes());
        put(0x280, &get);
        let mut set = vec![0u8; 86];
        for (at, b) in [
            (0, &[0x55, 0x8b, 0xec, 0x56, 0xff, 0x75, 8, 0xe8][..]),
            (12, &[0x8b, 0xf0, 0x83, 0xc4, 4, 0x85, 0xf6, 0x74]),
            (70, &[0xc3, 0x8b, 0x45, 16, 0x8b, 0xce, 0x50, 0x52, 0xe8]),
        ] {
            set[at..at + b.len()].copy_from_slice(b);
        }
        set[8..12].copy_from_slice(&va(0x750).wrapping_sub(va(0x300) + 12).to_le_bytes());
        set[79..83].copy_from_slice(&va(0x380).wrapping_sub(va(0x300) + 83).to_le_bytes());
        put(0x300, &set);
        let mut helper = vec![0u8; 77];
        for (at, b) in [
            (0, &[0x55, 0x8b, 0xec, 0x56, 0x8b, 0xf1, 0xba][..]),
            (
                11,
                &[
                    0x8b, 0x4d, 8, 0x8d, 0x41, 0xff, 0x3b, 0xd0, 0x8d, 0x41, 0xff, 0x1b, 0xd2,
                    0x83, 0xe2, 0xfb, 0x83, 0xc2, 6, 0x0f, 0xaf, 0x55, 12, 0x83, 0xf8, 1, 0x77,
                    0x13, 0x81, 0xfa,
                ],
            ),
            (45, &[0x76, 11, 0xc1, 0xfa, 0x1f, 0xf7, 0xd2, 0x81, 0xe2]),
            (58, &[0x83, 0xc1, 4, 0xc1, 0xe1, 4, 0x52, 3, 0xce, 0xe8]),
        ] {
            helper[at..at + b.len()].copy_from_slice(b);
        }
        helper[41..45].copy_from_slice(&600i32.to_le_bytes());
        helper[54..58].copy_from_slice(&600i32.to_le_bytes());
        helper[68..72].copy_from_slice(&va(0x4c0).wrapping_sub(va(0x380) + 72).to_le_bytes());
        put(0x380, &helper);
        let mut schema = vec![0; 79];
        for (at, b) in [
            (0, &[0xbe][..]),
            (5, &[0x85, 0xdb, 0x74]),
            (9, &[0x0f, 0x28, 5]),
            (
                16,
                &[
                    0x8d, 0x45, 0xd0, 0x8b, 0x4d, 0xcc, 0x6a, 0, 0x50, 0x0f, 0x11, 0x45, 0xd0,
                    0x53, 0x0f, 0x57, 0xc0,
                ],
            ),
            (62, &[0xe8]),
            (67, &[0x83, 0xc6, 16, 0x47, 0x81, 0xfe]),
        ] {
            schema[at..at + b.len()].copy_from_slice(b);
        }
        schema[1..5].copy_from_slice(&64u32.to_le_bytes());
        schema[12..16].copy_from_slice(&va(0x780).to_le_bytes());
        schema[73..77].copy_from_slice(&464u32.to_le_bytes());
        schema[63..67].copy_from_slice(&va(0x500).wrapping_sub(va(0x400) + 67).to_le_bytes());
        put(0x400, &schema);
        put(0x780, &0x12345678u32.to_le_bytes());
        put(0x784, &0x23456789u32.to_le_bytes());
        put(0x788, &6005i32.to_le_bytes());
        let mut binder = vec![0; 56];
        binder[19..30].copy_from_slice(&[
            0x56, 0xff, 0x75, 0x14, 0x8b, 0x75, 0x10, 0x8b, 0xcf, 0x56, 0xe8,
        ]);
        binder[34..47].copy_from_slice(&[
            0xff, 0x76, 8, 0x8b, 0x75, 8, 0x8b, 0xcf, 0x6a, 1, 0x56, 0xe8, 0,
        ]);
        put(0x500, &binder);
        let mut read_counter = vec![0; 35];
        read_counter[..7].copy_from_slice(&[0xff, 0x71, 12, 0xff, 0x71, 8, 0xe8]);
        read_counter[31..35].copy_from_slice(&[0x8b, 0x40, 0x48, 0xc3]);
        put(0x480, &read_counter);
        let mut write_counter = vec![0; 56];
        write_counter[37..40].copy_from_slice(&[0x8b, 0x4a, 0x48]);
        write_counter[40..52].copy_from_slice(&[
            0x8b, 0x45, 8, 0x6a, 0, 0x51, 0x8b, 0xca, 0x89, 0x42, 0x48, 0xe8,
        ]);
        put(0x4c0, &write_counter);
        let mut factory = vec![0; 56];
        factory[..2].copy_from_slice(&[0xc7, 7]);
        factory[6..9].copy_from_slice(&[0xc7, 0x47, 0x48]);
        factory[9..13].copy_from_slice(&18i32.to_le_bytes());
        factory[13..39].copy_from_slice(&[
            0xeb, 5, 0x8b, 6, 0x89, 0x43, 0x14, 0xff, 0x43, 0x18, 0x8d, 0x46, 4, 0x8b, 0x4d, 8,
            0x89, 0x48, 4, 0x8b, 0x4d, 12, 0x5f, 0xc7, 0x40, 12,
        ]);
        factory[39..43].copy_from_slice(&0x12345678u32.to_le_bytes());
        factory[43..46].copy_from_slice(&[0xc7, 0x40, 16]);
        factory[46..50].copy_from_slice(&0x23456789u32.to_le_bytes());
        factory[50..56].copy_from_slice(&[0x89, 0x48, 8, 0xff, 0x43, 0x1c]);
        put(0x550, &factory);

        for (at, name, sig, target) in [(0x200, 0x600, 0x640, 0x280), (0x214, 0x620, 0x660, 0x300)]
        {
            let mut reg = vec![0x68];
            reg.extend_from_slice(&va(sig).to_le_bytes());
            reg.push(0x68);
            reg.extend_from_slice(&va(name).to_le_bytes());
            reg.push(0x68);
            reg.extend_from_slice(&va(target).to_le_bytes());
            reg.extend_from_slice(&[0xe8, 0, 0, 0, 0]);
            put(at, &reg);
        }
        put(0x600, b"GetPlayerState\0");
        put(0x620, b"SetPlayerState\0");
        put(0x640, b"(Hplayer;Hplayerstate;)I\0");
        put(0x660, b"(Hplayer;Hplayerstate;I)V\0");
        (image, binding.origin, budget)
    }
    pub fn rules() -> Rules {
        let (image, origin, budget) = image();
        Rules::discover(
            &image,
            origin,
            &[("synthetic misc", b"[Misc]\nFoodLimit=73\n")],
            budget,
        )
        .unwrap()
    }
    #[test]
    fn changed_source_operands_drive_defaults_bounds_and_refusals() {
        let rules = rules();
        assert_eq!(rules.initial, 18);
        assert_eq!(rules.money(117), 100);
        assert_eq!(rules.money(-3), 0);
        assert_eq!(rules.money(37), 37);
        let (image, origin, budget) = image();
        for at in [0x280, 0x300, 0x380, 0x400, 0x200, 0x550, 0x4a1] {
            let mut broken = image.clone();
            broken[at] ^= 1;
            assert!(
                Rules::discover(
                    &broken,
                    origin.clone(),
                    &[("synthetic", b"[Misc]\nFoodLimit=73")],
                    budget
                )
                .is_err()
            );
        }
        assert!(
            Rules::discover(
                &image,
                origin,
                &[("synthetic", b"[Misc]\nFoodLimit=-1")],
                budget
            )
            .is_err()
        );
    }
}

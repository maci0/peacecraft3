//! Selected client state-counter initialization and money assignment bounds.
use crate::neutral_player::{RuleInputBudget, Sha256Digest, sha256};
use crate::neutral_rule_input::Pe;
use crate::scripts::AssetIdentity;
#[path = "player_upkeep.rs"]
mod upkeep;
pub use upkeep::UpkeepLabel;

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
    pub(crate) gathered_states: [i32; 2],
    pub(crate) gathered_scale: i32,
    upkeep: Result<upkeep::Upkeep, String>,
    food_limit: Option<i32>,
    food_defaults: Vec<i32>,
    pub(crate) food_marker_default: i32,
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
/// Follow both authored food descriptors to the common integer cell reader.
fn food_marker_default(pe: &Pe<'_>) -> Result<i32, String> {
    let mut readers = Vec::new();
    for key in [b"fmade\0".as_slice(), b"fused\0"] {
        let mut descriptors = Vec::new();
        for (_, code) in pe.code_ranges() {
            for b in code.windows(18) {
                if shape(
                    b,
                    &[(0, &[0x68]), (5, &[0x8d, 0x81]), (11, &[0x50, 0x57, 0xe8])],
                ) && pe
                    .read_from(word(b, 1).unwrap_or(0), false)
                    .is_ok_and(|s| s.starts_with(key))
                {
                    descriptors.push(word(b, 7).ok_or("food descriptor offset")?);
                }
            }
        }
        let [descriptor] = descriptors.as_slice() else {
            return Err("missing or ambiguous food descriptor".into());
        };
        let mut targets = Vec::new();
        for (va, code) in pe.code_ranges() {
            for (at, b) in code.windows(22).enumerate() {
                if shape(
                    b,
                    &[
                        (0, &[0x55, 0x8b, 0xec, 0x8d, 0x81]),
                        (9, &[0x50, 0xff, 0x75, 8, 0xe8]),
                        (18, &[0x5d, 0xc2, 4, 0]),
                    ],
                ) && word(b, 5) == Some(*descriptor)
                {
                    targets.push(call(b, va + at as u32, 13).ok_or("food descriptor reader")?);
                }
            }
        }
        let [target] = targets.as_slice() else {
            return Err("missing or ambiguous food descriptor reader".into());
        };
        readers.push(*target);
    }
    if readers[0] != readers[1] {
        return Err("food descriptors use different integer readers".into());
    }
    let va = readers[0];
    let reader = pe.read_from(va, true).map_err(|e| e.to_string())?;
    let mut targets = Vec::new();
    for (at, b) in reader
        .get(..192)
        .ok_or("food reader bounds")?
        .windows(21)
        .enumerate()
    {
        if shape(
            b,
            &[
                (0, &[0xff, 0x33, 0xff, 0x37, 0xe8]),
                (
                    9,
                    &[
                        0x83, 0xc4, 8, 0x5f, 0x5e, 0x5b, 0x8b, 0xe5, 0x5d, 0xc2, 8, 0,
                    ],
                ),
            ],
        ) {
            targets.push(call(b, va + at as u32, 4).ok_or("food cell reader")?);
        }
    }
    let [target] = targets.as_slice() else {
        return Err("missing or ambiguous food integer cell reader".into());
    };
    let cell = pe.read_from(*target, true).map_err(|e| e.to_string())?;
    if !shape(
        cell,
        &[
            (0, &[0x55, 0x8b, 0xec]),
            (39, &[0x5f, 0x33, 0xc0, 0x5e, 0x5d, 0xc3]),
            (
                59,
                &[
                    0x85, 0xc0, 0x74, 0xe8, 0x8a, 8, 0x80, 0xf9, 0x49, 0x75, 7, 0x8b, 0x40, 4,
                    0x5f, 0x5e, 0x5d, 0xc3,
                ],
            ),
            (
                77,
                &[
                    0x80, 0xf9, 0x46, 0x75, 0xd5, 0xf3, 0x0f, 0x2c, 0x40, 4, 0x5f, 0x5e, 0x5d, 0xc3,
                ],
            ),
        ],
    ) {
        return Err("unsupported food integer cell conversion".into());
    }
    // The checked XOR EAX,EAX instruction produces zero, independently of data.
    Ok(0)
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
        let food_marker_default = food_marker_default(&pe).map_err(|e| fail(&e))?;
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
                    if (if name == b"VersionGet\0" {
                        code.first() == Some(&0xa1) && code.get(5) == Some(&0xc3)
                    } else {
                        code.starts_with(&[0x55, 0x8b, 0xec])
                    }) && !found.contains(&target)
                    {
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
                (69, &[0x8d, 0x4e]),
                (72, &[0x3b, 0xcf]),
                (74, &[0x77]),
                (76, &[0x8d, 0x4e]),
                (79, &[0x3b, 0xf9]),
                (81, &[0x1b, 0xc9]),
                (83, &[0x33, 0xd2]),
                (85, &[0x83, 0xe1]),
                (88, &[0x83, 0xc1]),
                (91, &[0xf7, 0xf1]),
            ],
        ) || !shape(
            set,
            &[
                (0, &[0x55, 0x8b, 0xec, 0x56, 0xff, 0x75, 8, 0xe8]),
                (12, &[0x8b, 0xf0, 0x83, 0xc4, 4, 0x85, 0xf6, 0x74]),
                (21, &[0x8b, 0x55, 12]),
                (24, &[0x83, 0xfa]),
                (27, &[0x73]),
                (29, &[0x8d, 0x4a]),
                (32, &[0x83, 0xf9]),
                (35, &[0x77]),
                (37, &[0x8d, 0x42]),
                (40, &[0xb9]),
                (45, &[0x3b, 0xc8]),
                (47, &[0x8b, 0xce]),
                (49, &[0x1b, 0xc0]),
                (51, &[0x83, 0xe0]),
                (54, &[0x83, 0xc0]),
                (57, &[0x0f, 0xaf, 0x45, 16]),
                (61, &[0x50]),
                (62, &[0x52]),
                (63, &[0xe8]),
                (68, &[0x5e, 0x5d, 0xc3]),
                (71, &[0x8b, 0x45, 16]),
                (74, &[0x8b, 0xce]),
                (76, &[0x50]),
                (77, &[0x52]),
                (78, &[0xe8]),
                (83, &[0x5e, 0x5d, 0xc3]),
            ],
        ) || call(get, get_va, 6) != call(set, set_va, 7)
            || call(set, set_va, 63) != call(set, set_va, 78)
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
        let mask = i32::from(*get.get(63).ok_or_else(|| fail("money scale mask"))? as i8);
        let maximum = word(helper, 41).ok_or_else(|| fail("money maximum"))? as i32;
        let helper_seed = word(helper, 7).ok_or_else(|| fail("state clamp seed"))?;
        let helper_mask = i32::from(*helper.get(26).ok_or_else(|| fail("state clamp mask"))? as i8);
        let helper_scale =
            i32::from(*helper.get(29).ok_or_else(|| fail("state clamp scale"))? as i8);
        let gathered_first =
            -i32::from(*set.get(31).ok_or_else(|| fail("gathered state range"))? as i8);
        let gathered_offset = i32::from(*set.get(34).ok_or_else(|| fail("gathered state count"))?);
        let gathered_seed = word(set, 41).ok_or_else(|| fail("gathered scale seed"))?;
        let gathered_set_mask =
            i32::from(*set.get(53).ok_or_else(|| fail("gathered setter mask"))? as i8);
        let gathered_set_scale =
            i32::from(*set.get(56).ok_or_else(|| fail("gathered setter scale"))? as i8);
        let gathered_get_first =
            -i32::from(*get.get(71).ok_or_else(|| fail("gathered getter range"))? as i8);
        let gathered_get_offset = word(get, 53).ok_or_else(|| fail("gathered getter count"))?;
        let gathered_get_mask =
            i32::from(*get.get(87).ok_or_else(|| fail("gathered getter mask"))? as i8);
        let gathered_get_scale =
            i32::from(*get.get(90).ok_or_else(|| fail("gathered getter scale"))? as i8);
        if count != 1
            || first != -i32::from(helper[16] as i8)
            || first != -i32::from(helper[21] as i8)
            || helper_seed != 1
            || helper[36] != count as u8
            || helper_scale != scale
            || helper_mask != mask
            || helper_mask.checked_add(helper_scale) != Some(1)
            || scale.checked_add(mask) != Some(1)
            || word(helper, 54) != Some(maximum as u32)
            || scale <= 0
            || maximum < 0
            || set[26] != get[26]
            || gathered_first <= 0
            || gathered_first != gathered_get_first
            || gathered_offset != 1
            || gathered_get_offset != 1
            || gathered_seed != 1
            || set[31] != set[39]
            || get[71] != get[78]
            || set[53] != get[87]
            || gathered_set_mask.checked_add(gathered_set_scale) != Some(1)
            || gathered_get_mask.checked_add(gathered_get_scale) != Some(1)
            || gathered_set_scale != gathered_get_scale
            || gathered_set_scale <= 0
            || gathered_first <= first + count as i32
        {
            return Err(fail("inconsistent source player-state operands"));
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
        if gathered_first + gathered_offset >= *initialized_states as i32 {
            return Err(fail("gathered state outside initialized range"));
        }
        let version_va = entry(b"VersionGet\0", b"()Hversion;\0")?;
        let version = pe
            .read_from(version_va, true)
            .map_err(|e| fail(&e.to_string()))?;
        let version_global = word(version, 1).ok_or_else(|| fail("version selector"))?;
        let mut ceilings = Vec::new();
        for (_, code) in pe.code_ranges() {
            for b in code.windows(155) {
                if !shape(
                    b,
                    &[
                        (0, &[0xa1]),
                        (5, &[0x83, 0xf8, 0xff, 0x0f, 0x85]),
                        (14, &[0x56, 0xe8]),
                        (34, &[0x68]),
                        (39, &[0x68]),
                        (44, &[0x8b, 0xce, 0xe8]),
                        (51, &[0x85, 0xc0, 0x74, 0x46, 0x6a, 0, 0x68]),
                        (62, &[0x68]),
                        (67, &[0x8b, 0xce, 0xe8]),
                        (74, &[0xa3]),
                        (79, &[0x83, 0xf8]),
                        (82, &[0x7d, 0x11, 0xc7, 5]),
                        (94, &[0xb8]),
                        (99, &[0x5e, 0xc3, 0x3d]),
                        (106, &[0x7e, 0x2d, 0xc7, 5]),
                        (118, &[0xb8]),
                        (123, &[0x5e, 0xc3, 0x33, 0xc9, 0x83, 0x3d]),
                        (134, &[0x0f, 0x46, 0x0d]),
                        (141, &[0x8b, 4, 0x8d]),
                        (148, &[0xa3]),
                        (153, &[0x5e, 0xc3]),
                    ],
                ) {
                    continue;
                }
                let key = word(b, 35).unwrap_or(0);
                let section = word(b, 40).unwrap_or(0);
                let cache = word(b, 1);
                if word(b, 58) != Some(key)
                    || word(b, 63) != Some(section)
                    || !pe
                        .read_from(key, false)
                        .is_ok_and(|s| s.starts_with(b"FoodCeiling\0"))
                    || !pe
                        .read_from(section, false)
                        .is_ok_and(|s| s.starts_with(b"Misc\0"))
                    || [75, 86, 110, 149].iter().any(|at| word(b, *at) != cache)
                    || word(b, 129) != Some(version_global)
                    || word(b, 137) != Some(version_global)
                {
                    continue;
                }
                let minimum = i32::from(b[81] as i8);
                let maximum = word(b, 102).ok_or_else(|| fail("ceiling maximum"))? as i32;
                if minimum > maximum
                    || word(b, 90) != Some(minimum as u32)
                    || word(b, 95) != Some(minimum as u32)
                    || word(b, 114) != Some(maximum as u32)
                    || word(b, 119) != Some(maximum as u32)
                {
                    return Err(fail("inconsistent ceiling clamp"));
                }
                let count = i32::from(b[133] as i8)
                    .checked_add(1)
                    .and_then(|n| usize::try_from(n).ok())
                    .filter(|n| *n > 0 && *n <= budget.profile_bytes / 4)
                    .ok_or_else(|| fail("ceiling table budget"))?;
                let table = pe
                    .read_from(word(b, 144).unwrap_or(0), false)
                    .map_err(|e| fail(&e.to_string()))?;
                let defaults = (0..count)
                    .map(|i| {
                        word(table, i * 4)
                            .map(|n| n as i32)
                            .ok_or_else(|| fail("ceiling table bounds"))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                ceilings.push((minimum, maximum, defaults));
            }
        }
        let [(minimum, ceiling_maximum, food_defaults)] = ceilings.as_slice() else {
            return Err(fail("missing or ambiguous selected food ceiling"));
        };
        let ([food_limit], profile_sources) = pcx_objects::editor::misc::misc_scalars(
            profiles,
            ["FoodCeiling"],
            budget.profile_bytes,
        )?;
        let food_limit = food_limit
            .map(|value| {
                if value.fract() != 0.0 || value < i32::MIN as f32 || value >= i32::MAX as f32 {
                    return Err(fail("nonintegral Misc/FoodCeiling"));
                }
                Ok((value as i32).clamp(*minimum, *ceiling_maximum))
            })
            .transpose()?;
        let upkeep = upkeep::Upkeep::discover(
            &pe,
            version_global,
            profiles,
            budget.profile_bytes,
            (
                i32::from(get[36] as i8),
                1u32.checked_shl(u32::from(get[39]))
                    .ok_or_else(|| fail("state stride"))?,
                call(get, get_va, 43).ok_or_else(|| fail("counter getter target"))?,
            ),
        )
        .map_err(|e| fail(&e));
        Ok(Self {
            upkeep,
            origin,
            image_sha256: sha256(image),
            profile_sources,
            initial: *initial,
            initialized_states: *initialized_states as i32,
            money_states: [first, first + count as i32],
            money_scale: scale,
            money_maximum: maximum,
            gathered_states: [gathered_first, gathered_first + gathered_offset],
            gathered_scale: gathered_get_scale,
            food_limit,
            food_defaults: food_defaults.clone(),
            food_marker_default,
        })
    }
    pub(crate) fn food_ceiling(&self, version: Option<i32>) -> Option<i32> {
        if let Some(value) = self.food_limit {
            return Some(value);
        }
        let version = usize::try_from(version?)
            .ok()
            .filter(|index| *index < self.food_defaults.len())
            .unwrap_or(0);
        self.food_defaults.get(version).copied()
    }
    pub(crate) fn upkeep_label(
        &self,
        food: i32,
        version: Option<i32>,
    ) -> Result<UpkeepLabel, String> {
        self.upkeep
            .as_ref()
            .map_err(Clone::clone)?
            .label(food, version)
    }
    pub(crate) fn upkeep_food_state(&self) -> Option<i32> {
        self.upkeep.as_ref().ok().map(|u| u.food_state)
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
    pub(crate) fn gathered(&self, value: i32) -> i32 {
        self.gathered_value(value.wrapping_mul(self.gathered_scale))
    }
    pub(crate) fn gathered_value(&self, raw: i32) -> i32 {
        (raw as u32 / self.gathered_scale as u32) as i32
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
            &[("synthetic food profile", b"[Misc]\nFoodCeiling=73\n")],
            RuleInputBudget {
                image_bytes: 32 << 20,
                profile_bytes: 1 << 20,
                sections: 32,
            },
        )
        .unwrap();
        assert_eq!(rules.food_ceiling(None), Some(73));
        assert!(rules.money_scale > 0);
        assert_eq!(rules.gathered_states, [15, 16]);
        assert_eq!(rules.gathered_scale, 10);
        assert_eq!(rules.gathered_value(18), 1);
        eprintln!(
            "initial={} money_states={:?} scale={} cap={} gathered={:?} gathered_scale={} sha256={:02x?}",
            rules.initial,
            rules.money_states,
            rules.money_scale,
            rules.money_maximum,
            rules.gathered_states,
            rules.gathered_scale,
            rules.image_sha256
        );
    }
}

#[cfg(test)]
pub(crate) mod fixture {
    use super::*;
    pub fn image() -> (Vec<u8>, AssetIdentity, RuleInputBudget) {
        let (mut image, binding, budget) = crate::order_registry::tests::fixture([19, 37]);
        image.resize(0x1000, 0);
        for at in [0x100, 0x108] {
            image[at..at + 4].copy_from_slice(&0xe00u32.to_le_bytes());
        }
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
            (
                69,
                &[
                    0x8d, 0x4e, 0xf1, 0x3b, 0xcf, 0x77, 0x11, 0x8d, 0x4e, 0xf1, 0x3b, 0xf9, 0x1b,
                    0xc9, 0x33, 0xd2, 0x83, 0xe1, 0xf7, 0x83, 0xc1, 0x0a, 0xf7, 0xf1, 0x5f, 0x5e,
                    0x5d, 0xc3,
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
            (12, &[0x8b, 0xf0, 0x83, 0xc4, 4, 0x85, 0xf6, 0x74, 0x3e]),
            (
                21,
                &[
                    0x8b, 0x55, 12, 0x83, 0xfa, 0x1a, 0x73, 0x36, 0x8d, 0x4a, 0xf1, 0x83, 0xf9, 1,
                    0x77, 0x22, 0x8d, 0x42, 0xf1, 0xb9, 1, 0, 0, 0, 0x3b, 0xc8, 0x8b, 0xce, 0x1b,
                    0xc0, 0x83, 0xe0, 0xf7, 0x83, 0xc0, 0x0a, 0x0f, 0xaf, 0x45, 16, 0x50, 0x52,
                    0xe8, 0, 0, 0, 0, 0x5e, 0x5d, 0xc3, 0x8b, 0x45, 16, 0x8b, 0xce, 0x50, 0x52,
                    0xe8, 0, 0, 0, 0, 0x5e, 0x5d, 0xc3,
                ],
            ),
        ] {
            set[at..at + b.len()].copy_from_slice(b);
        }
        set[8..12].copy_from_slice(&va(0x750).wrapping_sub(va(0x300) + 12).to_le_bytes());
        set[64..68].copy_from_slice(&va(0x380).wrapping_sub(va(0x300) + 68).to_le_bytes());
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
        helper[7..11].copy_from_slice(&1u32.to_le_bytes());
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

        for (at, name, sig, target) in [
            (0x200, 0x600, 0x640, 0x280),
            (0x214, 0x620, 0x660, 0x300),
            (0x228, 0xa40, 0xa60, 0x800),
        ] {
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
        put(0xa00, b"FoodCeiling\0");
        put(0xa20, b"Misc\0");
        put(0xa40, b"VersionGet\0");
        put(0xa60, b"()Hversion;\0");
        let mut version = vec![0xa1];
        version.extend_from_slice(&va(0xa94).to_le_bytes());
        version.push(0xc3);
        put(0x800, &version);
        let mut ceiling = vec![0; 155];
        for (at, bytes) in [
            (0, &[0xa1][..]),
            (5, &[0x83, 0xf8, 0xff, 0x0f, 0x85]),
            (14, &[0x56, 0xe8]),
            (34, &[0x68]),
            (39, &[0x68]),
            (44, &[0x8b, 0xce, 0xe8]),
            (51, &[0x85, 0xc0, 0x74, 0x46, 0x6a, 0, 0x68]),
            (62, &[0x68]),
            (67, &[0x8b, 0xce, 0xe8]),
            (74, &[0xa3]),
            (79, &[0x83, 0xf8, 2]),
            (82, &[0x7d, 0x11, 0xc7, 5]),
            (94, &[0xb8]),
            (99, &[0x5e, 0xc3, 0x3d]),
            (106, &[0x7e, 0x2d, 0xc7, 5]),
            (118, &[0xb8]),
            (123, &[0x5e, 0xc3, 0x33, 0xc9, 0x83, 0x3d]),
            (134, &[0x0f, 0x46, 0x0d]),
            (141, &[0x8b, 4, 0x8d]),
            (148, &[0xa3]),
            (153, &[0x5e, 0xc3]),
        ] {
            ceiling[at..at + bytes.len()].copy_from_slice(bytes);
        }
        for at in [1, 75, 86, 110, 149] {
            ceiling[at..at + 4].copy_from_slice(&va(0xa90).to_le_bytes());
        }
        for at in [35, 58] {
            ceiling[at..at + 4].copy_from_slice(&va(0xa00).to_le_bytes());
        }
        for at in [40, 63] {
            ceiling[at..at + 4].copy_from_slice(&va(0xa20).to_le_bytes());
        }
        for at in [90, 95] {
            ceiling[at..at + 4].copy_from_slice(&2u32.to_le_bytes());
        }
        for at in [102, 114, 119] {
            ceiling[at..at + 4].copy_from_slice(&91u32.to_le_bytes());
        }
        for at in [129, 137] {
            ceiling[at..at + 4].copy_from_slice(&va(0xa94).to_le_bytes());
        }
        ceiling[133] = 1;
        ceiling[144..148].copy_from_slice(&va(0xa80).to_le_bytes());
        put(0x880, &ceiling);
        put(0xa80, &61i32.to_le_bytes());
        put(0xa84, &87i32.to_le_bytes());
        put(0xb00, b"fmade\0");
        put(0xb10, b"fused\0");
        for (key, at, wrapper, descriptor) in
            [(0xb00, 0xb20, 0xb60, 0x40u32), (0xb10, 0xb40, 0xb80, 0x64)]
        {
            let mut init = vec![0x68];
            init.extend_from_slice(&va(key).to_le_bytes());
            init.extend_from_slice(&[0x8d, 0x81]);
            init.extend_from_slice(&descriptor.to_le_bytes());
            init.extend_from_slice(&[0x50, 0x57, 0xe8, 0, 0, 0, 0]);
            put(at, &init);
            let mut read = vec![0x55, 0x8b, 0xec, 0x8d, 0x81];
            read.extend_from_slice(&descriptor.to_le_bytes());
            read.extend_from_slice(&[0x50, 0xff, 0x75, 8, 0xe8]);
            read.extend_from_slice(&va(0xba0).wrapping_sub(va(wrapper) + 18).to_le_bytes());
            read.extend_from_slice(&[0x5d, 0xc2, 4, 0]);
            put(wrapper, &read);
        }
        let mut reader = vec![0; 192];
        reader[171..176].copy_from_slice(&[0xff, 0x33, 0xff, 0x37, 0xe8]);
        reader[176..180].copy_from_slice(&va(0xc80).wrapping_sub(va(0xba0) + 180).to_le_bytes());
        reader[180..192].copy_from_slice(&[
            0x83, 0xc4, 8, 0x5f, 0x5e, 0x5b, 0x8b, 0xe5, 0x5d, 0xc2, 8, 0,
        ]);
        put(0xba0, &reader);
        let mut cell = vec![0; 91];
        for (at, bytes) in [
            (0, &[0x55, 0x8b, 0xec][..]),
            (39, &[0x5f, 0x33, 0xc0, 0x5e, 0x5d, 0xc3]),
            (
                59,
                &[
                    0x85, 0xc0, 0x74, 0xe8, 0x8a, 8, 0x80, 0xf9, 0x49, 0x75, 7, 0x8b, 0x40, 4,
                    0x5f, 0x5e, 0x5d, 0xc3,
                ],
            ),
            (
                77,
                &[
                    0x80, 0xf9, 0x46, 0x75, 0xd5, 0xf3, 0x0f, 0x2c, 0x40, 4, 0x5f, 0x5e, 0x5d, 0xc3,
                ],
            ),
        ] {
            cell[at..at + bytes.len()].copy_from_slice(bytes);
        }
        put(0xc80, &cell);
        (
            image,
            binding.origin,
            RuleInputBudget {
                image_bytes: 8192,
                ..budget
            },
        )
    }
    pub fn rules_with_upkeep() -> Rules {
        let (_, origin, _) = image();
        let (mut image, budget) = super::upkeep::tests::fixture();
        image[0x1202..0x1206].copy_from_slice(&0x90u32.to_le_bytes());
        Rules::discover(
            &image,
            origin,
            &[(
                "synthetic upkeep",
                b"[Misc]\nFoodCeiling=73\nSyntheticUsage=20,30\n",
            )],
            budget,
        )
        .unwrap()
    }
    pub fn rules() -> Rules {
        let (image, origin, budget) = image();
        Rules::discover(
            &image,
            origin,
            &[("synthetic misc", b"[Misc]\nFoodCeiling=73\n")],
            budget,
        )
        .unwrap()
    }
    pub fn rules_with_gathered(first: u8, scale: u8, initial: i32) -> Rules {
        let (mut image, origin, budget) = image();
        for at in [0x280 + 71, 0x280 + 78, 0x300 + 31, 0x300 + 39] {
            image[at] = first.wrapping_neg();
        }
        for at in [0x280 + 90, 0x300 + 56] {
            image[at] = scale;
        }
        for at in [0x280 + 87, 0x300 + 53] {
            image[at] = 1u8.wrapping_sub(scale);
        }
        image[0x550 + 9..0x550 + 13].copy_from_slice(&initial.to_le_bytes());
        Rules::discover(
            &image,
            origin,
            &[("synthetic misc", b"[Misc]\nFoodCeiling=73\n")],
            budget,
        )
        .unwrap()
    }
    pub fn rules_without_food_override() -> Rules {
        let (image, origin, budget) = image();
        Rules::discover(
            &image,
            origin,
            &[("synthetic profile without food", b"[Misc]\nUnrelated=4")],
            budget,
        )
        .unwrap()
    }
    #[test]
    fn missing_profile_ceiling_uses_the_selected_version_and_override_clamps_are_source_derived() {
        let rules = rules_without_food_override();
        assert_eq!(rules.food_ceiling(None), None);
        assert_eq!(rules.food_ceiling(Some(0)), Some(61));
        assert_eq!(rules.food_ceiling(Some(1)), Some(87));
        let (image, origin, budget) = image();
        for (text, expected) in [
            (b"[Misc]\nFoodCeiling=-9".as_slice(), 2),
            (b"[Misc]\nFoodCeiling=107".as_slice(), 91),
        ] {
            let rules = Rules::discover(
                &image,
                origin.clone(),
                &[("authored override", text)],
                budget,
            )
            .unwrap();
            assert_eq!(rules.food_ceiling(None), Some(expected));
        }
        let mut corrupt = image.clone();
        corrupt[0x880 + 129] ^= 1;
        assert!(Rules::discover(&corrupt, origin, &[("empty", b"[Misc]")], budget).is_err());
    }
    #[test]
    fn changed_source_operands_drive_defaults_bounds_and_refusals() {
        let rules = rules();
        assert_eq!(rules.initial, 18);
        assert_eq!(rules.food_marker_default, 0);
        assert_eq!(rules.money(117), 100);
        assert_eq!(rules.money(-3), 0);
        assert_eq!(rules.money(37), 37);
        let (image, origin, budget) = image();
        assert_eq!(rules.gathered_states, [15, 16]);
        assert_eq!(rules.gathered_scale, 10);
        assert_eq!(rules.gathered(117), 117);
        assert_eq!(rules.gathered_value(18), 1);
        assert_eq!(rules.gathered_value(-10), 429_496_728);
        for at in [
            0x280,
            0x300,
            0x380,
            0x400,
            0x200,
            0x550,
            0x4a1,
            0xb47,
            0xca8,
            0x2c7,
            0x2da,
            0x31f,
            0x338,
            0x387,
            0x39a,
            0x39d,
            0x280 + 87,
            0x300 + 53,
        ] {
            let mut broken = image.clone();
            broken[at] ^= 1;
            assert!(
                Rules::discover(
                    &broken,
                    origin.clone(),
                    &[("synthetic", b"[Misc]\nFoodCeiling=73")],
                    budget
                )
                .is_err()
            );
        }
        let mut outside = image.clone();
        for at in [0x280 + 71, 0x280 + 78, 0x300 + 31, 0x300 + 39] {
            outside[at] = 24u8.wrapping_neg();
        }
        assert!(
            Rules::discover(
                &outside,
                origin.clone(),
                &[("synthetic", b"[Misc]\nFoodCeiling=73")],
                budget
            )
            .is_err()
        );
        assert!(
            Rules::discover(
                &image,
                origin,
                &[("synthetic", b"[Misc]\nFoodCeiling=1.5")],
                budget
            )
            .is_err()
        );
    }
}

#[cfg(test)]
mod normal_profile_tests {
    use super::*;
    #[test]
    #[ignore = "requires PCX_PLAYER_RESOURCE_IMAGE and PCX_PLAYER_RESOURCE_PROFILE selecting the actual normal stock profile"]
    fn supplied_normal_profile_uses_selected_version_food_ceiling() {
        let image_path =
            std::env::var("PCX_PLAYER_RESOURCE_IMAGE").expect("PCX_PLAYER_RESOURCE_IMAGE");
        let profile_path =
            std::env::var("PCX_PLAYER_RESOURCE_PROFILE").expect("PCX_PLAYER_RESOURCE_PROFILE");
        let image = std::fs::read(&image_path).unwrap();
        let profile = std::fs::read(&profile_path).unwrap();
        let map_path = std::env::var("PCX_PLAYER_RESOURCE_MAP_PROFILE").ok();
        let map = map_path.as_ref().map(std::fs::read).transpose().unwrap();
        let mut profiles = vec![(profile_path.as_str(), profile.as_slice())];
        if let (Some(path), Some(bytes)) = (&map_path, &map) {
            profiles.push((path.as_str(), bytes.as_slice()));
        }
        let rules = Rules::discover(
            &image,
            AssetIdentity {
                source: image_path,
                edition: "explicit normal source profile".into(),
                version: "caller selected; digest retained".into(),
            },
            &profiles,
            RuleInputBudget {
                image_bytes: 32 << 20,
                profile_bytes: 8 << 20,
                sections: 96,
            },
        )
        .unwrap();
        // An absent override is the shipped profile regression; no synthetic ceiling supplies it.
        assert!(rules.food_limit.is_none());
        assert!(rules.food_ceiling(None).is_none());
        for (index, expected) in rules.food_defaults.iter().enumerate() {
            assert_eq!(rules.food_ceiling(Some(index as i32)), Some(*expected));
        }
        eprintln!(
            "normal profile={} profile_sha256={:02x?} source_food_defaults={:?} image_sha256={:02x?}",
            profile_path,
            sha256(&profile),
            rules.food_defaults,
            rules.image_sha256
        );
    }
}

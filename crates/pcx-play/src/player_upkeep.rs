//! Bounded source-code admission for the participant resource-bar upkeep label.
use super::{call, shape, word};
use crate::neutral_rule_input::Pe;
use crate::pe_image::windows_with_byte;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpkeepLabel {
    pub primary_key: String,
    pub fallback: Option<(String, String)>,
    /// Native destination capacity, including its terminator.
    pub capacity: usize,
}
#[derive(Clone, Debug)]
pub(super) struct Upkeep {
    thresholds: Option<Vec<i32>>,
    defaults: Vec<Vec<i32>>,
    pub food_state: i32,
    keys: Vec<String>,
    first_rank: i32,
    custom_offset: i32,
    custom_format: String,
    suffix_format: String,
    key_capacity: usize,
    suffix_capacity: usize,
    capacity: usize,
}
fn code<'a>(pe: &Pe<'a>, va: u32, len: usize) -> Result<&'a [u8], String> {
    pe.read_from(va, true)
        .map_err(|e| e.to_string())?
        .get(..len)
        .ok_or_else(|| "upkeep code bounds".into())
}
fn string(pe: &Pe<'_>, va: u32) -> Result<String, String> {
    let bytes = pe.read_from(va, false).map_err(|e| e.to_string())?;
    let end = bytes
        .iter()
        .take(4096)
        .position(|b| *b == 0)
        .ok_or("upkeep string bounds")?;
    let text = std::str::from_utf8(&bytes[..end]).map_err(|e| e.to_string())?;
    if text.is_empty() {
        return Err("empty upkeep string".into());
    }
    Ok(text.into())
}
fn operand(b: &[u8], at: usize) -> Result<u32, String> {
    word(b, at).ok_or_else(|| "upkeep operand bounds".into())
}
fn target(b: &[u8], va: u32, at: usize) -> Result<u32, String> {
    call(b, va, at).ok_or_else(|| "upkeep call shape".into())
}
fn decimal(format: &str, value: i32, capacity: usize) -> Result<String, String> {
    let (prefix, suffix) = format
        .split_once("%d")
        .ok_or("unsupported upkeep integer format")?;
    if prefix.contains('%') || suffix.contains('%') {
        return Err("unsupported upkeep format conversion".into());
    }
    let text = format!("{prefix}{value}{suffix}");
    if text.len() >= capacity {
        return Err("upkeep formatted value exceeds source capacity".into());
    }
    Ok(text)
}
impl Upkeep {
    pub(super) fn discover(
        pe: &Pe<'_>,
        version_global: u32,
        profiles: &[(&str, &[u8])],
        budget: usize,
        state_address: (i32, u32, u32),
    ) -> Result<Self, String> {
        // Bind the updater's text member to the source constructor's named frame.
        let mut members = Vec::new();
        for (_, bytes) in pe.code_ranges() {
            for (_, b) in windows_with_byte(bytes, 0, 0x68, 30) {
                if shape(b, &[(0, &[0x68]), (5, &[0xe8]), (24, &[0x89, 0x86])])
                    && string(pe, operand(b, 1)?).is_ok_and(|s| s == "ResourceBarUpkeepText")
                {
                    members.push(operand(b, 26)?);
                }
            }
        }
        let [member] = members.as_slice() else {
            return Err("missing or ambiguous upkeep text constructor".into());
        };
        let mut updaters = Vec::new();
        for (va, bytes) in pe.code_ranges() {
            for (at, b) in windows_with_byte(bytes, 0, 0x55, 49) {
                if shape(
                    b,
                    &[
                        (0, &[0x55, 0x8b, 0xec, 0x83, 0xec]),
                        (17, &[0x6a]),
                        (19, &[0x8d, 0x45]),
                        (22, &[0x8b, 0xf1, 0x50, 0xe8]),
                        (30, &[0x50, 0x8b, 0xce, 0xe8]),
                        (38, &[0x8b, 0x8e]),
                        (44, &[0x8d, 0x45]),
                    ],
                ) && operand(b, 40)? == *member
                {
                    updaters.push((va + at as u32, b));
                }
            }
        }
        let [(va, update)] = updaters.as_slice() else {
            return Err("missing or ambiguous upkeep text updater".into());
        };
        let capacity = usize::from(update[18]);
        let tier_va = target(update, *va, 25)?;
        let formatter_va = target(update, *va, 33)?;
        let tier = code(pe, tier_va, 121)?;
        if !shape(
            tier,
            &[
                (60, &[0x8b, 0xce, 0xe8]),
                (71, &[0x33, 0xff, 0x33, 0xf6, 0x56, 0xe8]),
                (81, &[0x83, 0xc4, 4, 0x85, 0xc0, 0x74]),
                (88, &[0x56, 0xe8]),
                (94, &[0x83, 0xc4, 4, 0x3b, 0xc7, 0x7d]),
                (101, &[0x46, 0x56, 0xe8]),
                (108, &[0x83, 0xc4, 4, 0x85, 0xc0, 0x75]),
                (115, &[0x5f, 0x8d, 0x46]),
                (119, &[0x5e, 0xc3]),
            ],
        ) {
            return Err("unsupported upkeep tier comparison".into());
        }
        let getter_va = target(tier, tier_va, 62)?;
        let getter = code(pe, getter_va, 11)?;
        if !shape(getter, &[(0, &[0x81, 0xc1]), (6, &[0xe9])])
            || getter_va.wrapping_add(11).wrapping_add(operand(getter, 7)?) != state_address.2
        {
            return Err("unsupported upkeep food reader".into());
        }
        // GetPlayerState's checked integer-state address is player+(state+4)*16.
        let offset = operand(getter, 2)?;
        if state_address.1 == 0 || !offset.is_multiple_of(state_address.1) {
            return Err("unaligned upkeep state reader".into());
        }
        let food_state = i32::try_from(offset / state_address.1)
            .map_err(|_| "upkeep state range")?
            - state_address.0;
        let threshold_va = target(tier, tier_va, 76)?;
        if [89, 103]
            .into_iter()
            .any(|at| call(tier, tier_va, at) != Some(threshold_va))
        {
            return Err("inconsistent upkeep threshold consumer".into());
        }
        let t = code(pe, threshold_va, 371)?;
        if !shape(
            t,
            &[
                (0, &[0x55, 0x8b, 0xec, 0x53, 0x8b, 0x5d, 8, 0x83, 0xfb]),
                (10, &[0x72, 5, 0x33, 0xc0, 0x5b, 0x5d, 0xc3]),
                (50, &[0x68]),
                (55, &[0x68]),
                (60, &[0x8b, 0xcf, 0xe8]),
                (71, &[0x56, 0x33, 0xf6]),
                (80, &[0x56, 0x68]),
                (86, &[0x68]),
                (91, &[0x8b, 0xcf, 0xe8]),
                (105, &[0x46, 0x83, 0xfe]),
                (109, &[0x7c]),
                (123, &[0x8b, 0x15]),
                (129, &[0x33, 0xc0, 0x83, 0xfa]),
                (134, &[0x5f, 0x0f, 0x46, 0xc2, 0x8d, 4, 0x80, 0x8b, 4, 0xc5]),
            ],
        ) {
            return Err("unsupported upkeep threshold accessor".into());
        }
        let width = usize::from(t[9]);
        if width == 0
            || width != usize::from(t[108])
            || width > budget / 4
            || operand(t, 125)? != version_global
        {
            return Err("upkeep threshold count or edition relation".into());
        }
        let key = string(pe, operand(t, 51)?)?;
        if string(pe, operand(t, 56)?)? != "Misc"
            || operand(t, 51)? != operand(t, 82)?
            || operand(t, 56)? != operand(t, 87)?
        {
            return Err("inconsistent upkeep profile binding".into());
        }
        let profile_reader_va = target(t, threshold_va, 93)?;
        let profile_reader = code(pe, profile_reader_va, 77)?;
        if !shape(profile_reader, &[(32, &[0xff, 0x75, 0x10]), (40, &[0xe8])]) {
            return Err("unsupported upkeep indexed profile reader".into());
        }
        let integer = code(pe, target(profile_reader, profile_reader_va, 40)?, 32)?;
        if !shape(
            integer,
            &[
                (0, &[0x55, 0x8b, 0xec, 0x51]),
                (10, &[0xc7, 0x45, 0xfc]),
                (17, &[0x50]),
            ],
        ) {
            return Err("unsupported upkeep missing integer initializer".into());
        }
        let missing = operand(integer, 13)? as i32;
        let thresholds =
            pcx_objects::editor::misc::misc_integer_list(profiles, &key, width, missing, budget)?;
        let bank_count = usize::from(t[133])
            .checked_add(1)
            .filter(|n| *n <= budget / 4)
            .ok_or("upkeep edition table budget")?;
        let table_va = operand(t, 144)?;
        // LEA eax,[eax+eax*4] followed by scaled x8 gives source bank stride.
        // Check every subsequent authored load against that same contiguous table.
        let loads: Vec<_> = t
            .windows(7)
            .filter(|b| b.starts_with(&[0x8b, 4, 0xc5]) || b.starts_with(&[0x8b, 0x0c, 0xcd]))
            .collect();
        if loads.len() != width {
            return Err("upkeep default load count".into());
        }
        for (index, load) in loads.iter().enumerate() {
            if operand(load, 3)?
                != table_va
                    .checked_add((index * 4) as u32)
                    .ok_or("upkeep table offset")?
            {
                return Err("inconsistent upkeep default loads".into());
            }
        }
        let stride = ((1usize << (t[140] >> 6)) + 1) * (1usize << (t[143] >> 6));
        if width * 4 != stride {
            return Err("unsupported upkeep bank stride".into());
        }
        if bank_count
            .checked_mul(stride)
            .is_none_or(|total| total > budget)
        {
            return Err("upkeep table output budget".into());
        }
        let table = pe.read_from(table_va, false).map_err(|e| e.to_string())?;
        let defaults = (0..bank_count)
            .map(|bank| {
                (0..width)
                    .map(|i| operand(table, bank * stride + i * 4).map(|n| n as i32))
                    .collect()
            })
            .collect::<Result<Vec<Vec<_>>, String>>()?;
        let f = code(pe, formatter_va, 405)?;
        if !shape(
            f,
            &[
                (172, &[0x8b, 0x45, 8, 0x83, 0xf8]),
                (178, &[0x77]),
                (180, &[0xff, 0x24, 0x85]),
                (295, &[0x8d, 0x78]),
                (298, &[0x57, 0x68]),
                (310, &[0x68]),
                (347, &[0x68]),
                (357, &[0x57, 0x68]),
                (366, &[0x6a]),
            ],
        ) {
            return Err("unsupported upkeep label formatter".into());
        }
        let count = usize::from(f[177]) + 1;
        if count > budget / 4 {
            return Err("upkeep label table budget".into());
        }
        let switches = pe
            .read_from(operand(f, 183)?, false)
            .map_err(|e| e.to_string())?;
        let mut keys = Vec::new();
        for i in 0..count {
            let branch = code(pe, operand(switches, i * 4)?, 14)?;
            if !shape(
                branch,
                &[(0, &[0xff, 0x75, 0x10, 0x53, 0x68]), (9, &[0xe8])],
            ) {
                return Err("unsupported upkeep label switch".into());
            }
            keys.push(string(pe, operand(branch, 5)?)?);
        }
        if keys.last() != Some(&string(pe, operand(f, 348)?)?) {
            return Err("inconsistent upkeep fallback key".into());
        }
        Ok(Self {
            thresholds,
            defaults,
            food_state,
            keys,
            first_rank: i32::from(tier[118] as i8),
            custom_offset: i32::from(f[297] as i8),
            custom_format: string(pe, operand(f, 300)?)?,
            suffix_format: string(pe, operand(f, 359)?)?,
            key_capacity: operand(f, 311)? as usize,
            suffix_capacity: usize::from(f[367]),
            capacity,
        })
    }
    pub(super) fn label(&self, food: i32, version: Option<i32>) -> Result<UpkeepLabel, String> {
        let thresholds = if let Some(values) = &self.thresholds {
            values
        } else {
            let version = version.ok_or("upkeep requires the selected startup version")?;
            let bank = usize::try_from(version)
                .ok()
                .filter(|i| *i < self.defaults.len())
                .unwrap_or(0);
            &self.defaults[bank]
        };
        let crossed = thresholds
            .iter()
            .take_while(|threshold| **threshold != 0 && **threshold < food)
            .count();
        let rank = i32::try_from(crossed)
            .ok()
            .and_then(|v| v.checked_add(self.first_rank))
            .ok_or("upkeep rank overflow")?;
        if let Some(key) = usize::try_from(rank).ok().and_then(|i| self.keys.get(i)) {
            return Ok(UpkeepLabel {
                primary_key: key.clone(),
                fallback: None,
                capacity: self.capacity,
            });
        }
        let number = rank
            .checked_add(self.custom_offset)
            .ok_or("upkeep label index overflow")?;
        Ok(UpkeepLabel {
            primary_key: decimal(&self.custom_format, number, self.key_capacity)?,
            fallback: Some((
                self.keys.last().ok_or("empty upkeep labels")?.clone(),
                decimal(&self.suffix_format, number, self.suffix_capacity)?,
            )),
            capacity: self.capacity,
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{neutral_player::RuleInputBudget, scripts::AssetIdentity};
    pub(crate) fn fixture() -> (Vec<u8>, RuleInputBudget) {
        let (mut image, _, budget) = crate::player_resources::fixture::image();
        image.resize(0x2400, 0);
        image[0xd0..0xd4].copy_from_slice(&0x4000u32.to_le_bytes());
        for at in [0x100, 0x108] {
            image[at..at + 4].copy_from_slice(&0x2200u32.to_le_bytes());
        }
        let va = |raw: usize| 0x301000 + (raw - 0x200) as u32;
        fn put(image: &mut [u8], at: usize, parts: &[(usize, &[u8])]) {
            for (offset, bytes) in parts {
                image[at + offset..at + offset + bytes.len()].copy_from_slice(bytes);
            }
        }
        fn word_at(image: &mut [u8], at: usize, value: u32) {
            image[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
        let mut call_at = |at: usize, to: usize| {
            image[at] = 0xe8;
            word_at(&mut image, at + 1, va(to).wrapping_sub(va(at) + 5));
        };
        for (at, to) in [
            (0x1059, 0x1100),
            (0x1061, 0x1600),
            (0x113e, 0x1200),
            (0x114c, 0x1300),
            (0x1159, 0x1300),
            (0x1167, 0x1300),
            (0x135d, 0x1500),
            (0x1528, 0x1580),
        ] {
            call_at(at, to);
        }
        put(
            &mut image,
            0x1000,
            &[(0, &[0x68]), (5, &[0xe8]), (24, &[0x89, 0x86])],
        );
        word_at(&mut image, 0x1001, va(0x1a00));
        word_at(&mut image, 0x101a, 0x234);
        put(
            &mut image,
            0x1040,
            &[
                (0, &[0x55, 0x8b, 0xec, 0x83, 0xec]),
                (17, &[0x6a, 0x55]),
                (19, &[0x8d, 0x45, 0xbc, 0x8b, 0xf1, 0x50]),
                (30, &[0x50, 0x8b, 0xce]),
                (38, &[0x8b, 0x8e]),
                (44, &[0x8d, 0x45]),
            ],
        );
        word_at(&mut image, 0x1068, 0x234);
        put(
            &mut image,
            0x1100,
            &[
                (60, &[0x8b, 0xce]),
                (71, &[0x33, 0xff, 0x33, 0xf6, 0x56]),
                (81, &[0x83, 0xc4, 4, 0x85, 0xc0, 0x74]),
                (88, &[0x56]),
                (94, &[0x83, 0xc4, 4, 0x3b, 0xc7, 0x7d]),
                (101, &[0x46, 0x56]),
                (108, &[0x83, 0xc4, 4, 0x85, 0xc0, 0x75]),
                (115, &[0x5f, 0x8d, 0x46, 1, 0x5e, 0xc3]),
            ],
        );
        put(&mut image, 0x1200, &[(0, &[0x81, 0xc1]), (6, &[0xe9])]);
        word_at(&mut image, 0x1202, 0x70);
        word_at(&mut image, 0x1207, va(0x480).wrapping_sub(va(0x1200) + 11));
        put(
            &mut image,
            0x1300,
            &[
                (0, &[0x55, 0x8b, 0xec, 0x53, 0x8b, 0x5d, 8, 0x83, 0xfb, 10]),
                (10, &[0x72, 5, 0x33, 0xc0, 0x5b, 0x5d, 0xc3]),
                (50, &[0x68]),
                (55, &[0x68]),
                (60, &[0x8b, 0xcf, 0xe8]),
                (71, &[0x56, 0x33, 0xf6]),
                (80, &[0x56, 0x68]),
                (86, &[0x68]),
                (91, &[0x8b, 0xcf]),
                (105, &[0x46, 0x83, 0xfe, 10, 0x7c]),
                (123, &[0x8b, 0x15]),
                (
                    129,
                    &[
                        0x33, 0xc0, 0x83, 0xfa, 1, 0x5f, 0x0f, 0x46, 0xc2, 0x8d, 4, 0x80, 0x8b, 4,
                        0xc5,
                    ],
                ),
            ],
        );
        for at in [51, 82] {
            word_at(&mut image, 0x1300 + at, va(0x1a30));
        }
        for at in [56, 87] {
            word_at(&mut image, 0x1300 + at, va(0x1a50));
        }
        word_at(&mut image, 0x137d, va(0xa94));
        word_at(&mut image, 0x1390, va(0x1b00));
        for index in 1..10 {
            let at = 0x1394 + index * 10;
            put(&mut image, at, &[(0, &[0x8b, 4, 0xc5])]);
            word_at(&mut image, at + 3, va(0x1b00) + index as u32 * 4);
        }
        put(&mut image, 0x1500, &[(32, &[0xff, 0x75, 0x10])]);
        put(
            &mut image,
            0x1580,
            &[
                (0, &[0x55, 0x8b, 0xec, 0x51]),
                (10, &[0xc7, 0x45, 0xfc]),
                (17, &[0x50]),
            ],
        );
        put(
            &mut image,
            0x1600,
            &[
                (172, &[0x8b, 0x45, 8, 0x83, 0xf8, 3, 0x77]),
                (180, &[0xff, 0x24, 0x85]),
                (295, &[0x8d, 0x78, 0xfe, 0x57, 0x68]),
                (310, &[0x68]),
                (347, &[0x68]),
                (357, &[0x57, 0x68]),
                (366, &[0x6a, 21]),
            ],
        );
        word_at(&mut image, 0x1600 + 183, va(0x1900));
        word_at(&mut image, 0x1600 + 300, va(0x1ac0));
        word_at(&mut image, 0x1600 + 311, 71);
        word_at(&mut image, 0x1600 + 348, va(0x1aa0));
        word_at(&mut image, 0x1600 + 359, va(0x1ae0));
        for (index, key) in [0x1a60, 0x1a60, 0x1a80, 0x1aa0].into_iter().enumerate() {
            let branch = 0x1800 + index * 20;
            word_at(&mut image, 0x1900 + index * 4, va(branch));
            put(
                &mut image,
                branch,
                &[(0, &[0xff, 0x75, 0x10, 0x53, 0x68]), (9, &[0xe8])],
            );
            word_at(&mut image, branch + 5, va(key));
        }
        for (at, text) in [
            (0x1a00, b"ResourceBarUpkeepText\0".as_slice()),
            (0x1a30, b"SyntheticUsage\0"),
            (0x1a50, b"Misc\0"),
            (0x1a60, b"NONE_SYNTHETIC\0"),
            (0x1a80, b"LOW_SYNTHETIC\0"),
            (0x1aa0, b"HIGH_SYNTHETIC\0"),
            (0x1ac0, b"CUSTOM_%d\0"),
            (0x1ae0, b" (%d)\0"),
        ] {
            put(&mut image, at, &[(0, text)]);
        }
        for (bank, thresholds) in [
            [7, 19, 0, 0, 0, 0, 0, 0, 0, 0],
            [11, 29, 0, 0, 0, 0, 0, 0, 0, 0],
        ]
        .iter()
        .enumerate()
        {
            for (index, value) in thresholds.iter().enumerate() {
                word_at(&mut image, 0x1b00 + bank * 40 + index * 4, *value);
            }
        }
        (
            image,
            RuleInputBudget {
                image_bytes: 16384,
                ..budget
            },
        )
    }
    #[test]
    fn source_thresholds_edition_boundaries_and_override_padding_drive_labels() {
        let (image, budget) = fixture();
        let pe = Pe::parse(&image, budget).unwrap();
        let global = 0x301000 + (0xa94 - 0x200);
        let upkeep =
            Upkeep::discover(&pe, global, &[], budget.profile_bytes, (4, 16, 0x301280)).unwrap();
        assert_eq!(upkeep.food_state, 3);
        assert_eq!(
            upkeep.label(7, Some(0)).unwrap().primary_key,
            "NONE_SYNTHETIC"
        );
        assert_eq!(
            upkeep.label(8, Some(0)).unwrap().primary_key,
            "LOW_SYNTHETIC"
        );
        assert_eq!(
            upkeep.label(19, Some(0)).unwrap().primary_key,
            "LOW_SYNTHETIC"
        );
        assert_eq!(
            upkeep.label(20, Some(0)).unwrap().primary_key,
            "HIGH_SYNTHETIC"
        );
        assert_eq!(
            upkeep.label(8, Some(1)).unwrap().primary_key,
            "NONE_SYNTHETIC"
        );
        assert!(upkeep.label(8, None).is_err());
        let profiles = [
            ("stock", b"[Misc]\nSyntheticUsage=4,8,12,16\n".as_slice()),
            ("map", b"[Misc]\nSyntheticUsage=2\n"),
        ];
        let upkeep = Upkeep::discover(
            &pe,
            global,
            &profiles,
            budget.profile_bytes,
            (4, 16, 0x301280),
        )
        .unwrap();
        assert_eq!(
            upkeep.label(300, None).unwrap().primary_key,
            "LOW_SYNTHETIC"
        );
        let profiles = [("map", b"[Misc]\nSyntheticUsage=2,3,4,5\n".as_slice())];
        let upkeep = Upkeep::discover(
            &pe,
            global,
            &profiles,
            budget.profile_bytes,
            (4, 16, 0x301280),
        )
        .unwrap();
        let label = upkeep.label(6, None).unwrap();
        assert_eq!(label.primary_key, "CUSTOM_3");
        assert_eq!(
            label.fallback,
            Some(("HIGH_SYNTHETIC".into(), " (3)".into()))
        );
        assert_eq!(label.capacity, 0x55);
        assert!(
            Upkeep::discover(&pe, global, &[], budget.profile_bytes, (4, 16, 0x301284)).is_err()
        );
        assert!(
            Upkeep::discover(
                &pe,
                global + 4,
                &[],
                budget.profile_bytes,
                (4, 16, 0x301280)
            )
            .is_err()
        );
    }
    #[test]
    #[ignore = "requires PCX_PLAYER_RESOURCE_IMAGE selecting a locally supplied client executable"]
    fn supplied_image_binds_upkeep_to_food_version_and_localization_operands() {
        let path = std::env::var("PCX_PLAYER_RESOURCE_IMAGE").expect("explicit local image");
        let bytes = std::fs::read(&path).unwrap();
        let rules = crate::player_resources::Rules::discover(
            &bytes,
            AssetIdentity {
                source: path,
                edition: "explicit local source".into(),
                version: "explicit selected image; digest retained".into(),
            },
            &[],
            RuleInputBudget {
                image_bytes: 32 << 20,
                profile_bytes: 1 << 20,
                sections: 32,
            },
        )
        .unwrap();
        let upkeep = rules.upkeep.as_ref().unwrap();
        for version in 0..upkeep.defaults.len() {
            for threshold in &upkeep.defaults[version] {
                for food in [
                    threshold.saturating_sub(1),
                    *threshold,
                    threshold.saturating_add(1),
                ] {
                    let label = upkeep.label(food, Some(version as i32)).unwrap();
                    assert!(!label.primary_key.is_empty());
                }
            }
        }
        assert!(upkeep.label(0, None).is_err());
    }
}

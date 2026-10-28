//! Control-group constants from the reviewed, locally supplied Legacy client.
//!
//! Every value is decoded from instruction operands or from the tables those
//! instructions index: the digit-to-group switch, the selection cap, the
//! double-press window, and the `[Errors]` keys behind the three refusals. The
//! locations and their evidence are in `docs/re-notes/input-control-groups.md`.
use crate::{
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    scripts::AssetIdentity,
};

const IMAGE_BUDGET: usize = 32 * 1024 * 1024;
/// Control groups are bound to the ten digit keys.
pub const GROUP_COUNT: usize = 10;
/// Windows virtual-key code of the digit `0`; `1`..`9` follow it.
const VK_DIGIT_ZERO: u32 = 0x30;
/// Longest `[Errors]` key name accepted from the image.
const MAX_KEY_NAME: usize = 64;

// Reviewed instruction locations in the key handler `0x5fc700`.
const KEY_BIAS: u32 = 0x5fc75f;
const KEY_LIMIT: u32 = 0x5fc768;
const KEY_SWITCH: u32 = 0x5fc773;
const TAP_COMPARE: u32 = 0x5fcc36;
// Reviewed cap sites: control-group define dispatch, the local group mirror and
// the group select helper.
const CAP_DEFINE: u32 = 0x6cc90b;
const CAP_MIRROR: u32 = 0x4f4fe0;
const CAP_SELECT: u32 = 0x6005eb;
// Reviewed error-code computations: selection empty, merged list empty, and
// filtered list empty.
const CODE_SELECTION_EMPTY: u32 = 0x5fc891;
const CODE_MERGED_EMPTY: u32 = 0x5fcbb4;
const CODE_FILTERED_EMPTY: u32 = 0x5fc911;
// Reviewed error-name switch `0x5f0840`.
const NAME_BIAS: u32 = 0x5f0848;
const NAME_LIMIT: u32 = 0x5f084b;
const NAME_SWITCH: u32 = 0x5f0856;

/// `[Errors]` keys of `Units\CommandStrings.txt` the image selects for each refusal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ErrorKeys {
    /// Units were selected but none of them is controlled by the local player.
    pub invalid: String,
    /// Nothing was selected, or the merge produced no unit.
    pub empty: String,
    /// Only objects that are not units were selected.
    pub non_unit: String,
}

#[derive(Clone, Debug)]
pub struct ControlGroupRules {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    /// Group number the image binds to each digit key, indexed by digit `0`..`9`.
    pub digit_groups: [u8; GROUP_COUNT],
    /// Most units a define keeps and a select restores.
    pub selection_cap: usize,
    /// Seconds within which a second press of the same digit centres the camera.
    pub double_press_seconds: f32,
    pub error_keys: ErrorKeys,
}

impl ControlGroupRules {
    pub fn read(image: &[u8], origin: AssetIdentity) -> Result<Self, String> {
        let fail = |reason: &str| format!("control groups {}: {reason}", origin.source);
        if image.len() > IMAGE_BUDGET {
            return Err(fail("client image exceeds budget"));
        }
        if [&origin.source, &origin.edition, &origin.version]
            .into_iter()
            .any(|value| value.trim().is_empty())
        {
            return Err(fail("missing source identity"));
        }
        let digest = sha256(image);
        if digest != REVIEWED_IMAGE {
            return Err(fail("unsupported client image; no rules substituted"));
        }
        let pe = Pe::parse(
            image,
            RuleInputBudget {
                image_bytes: IMAGE_BUDGET,
                profile_bytes: 128,
                sections: 96,
            },
        )
        .map_err(|e| fail(&e.to_string()))?;
        let read = |va, code| pe.read_from(va, code).map_err(|e| e.to_string());
        let decoded = decode(&read).map_err(|reason| fail(&reason))?;
        Ok(Self {
            origin,
            image_sha256: digest,
            digit_groups: decoded.digit_groups,
            selection_cap: decoded.selection_cap,
            double_press_seconds: decoded.double_press_seconds,
            error_keys: decoded.error_keys,
        })
    }
}

/// Decoded values without image identity.
#[derive(Debug)]
struct Decoded {
    digit_groups: [u8; GROUP_COUNT],
    selection_cap: usize,
    double_press_seconds: f32,
    error_keys: ErrorKeys,
}

/// Address of a table entry; an operand that wraps the address space is refused.
fn at(base: u32, offset: u32) -> Result<u32, String> {
    base.checked_add(offset)
        .ok_or_else(|| "table address overflows".into())
}

fn decode<'a>(read: &impl Fn(u32, bool) -> Result<&'a [u8], String>) -> Result<Decoded, String> {
    let code = |va: u32, len: usize| -> Result<&'a [u8], String> {
        read(va, true)?
            .get(..len)
            .ok_or_else(|| format!("truncated instruction window at {va:#x}"))
    };
    let data = |va: u32, len: usize| -> Result<&'a [u8], String> {
        read(va, false)?
            .get(..len)
            .ok_or_else(|| format!("truncated data window at {va:#x}"))
    };
    let word = |va: u32| -> Result<u32, String> {
        Ok(u32::from_le_bytes(data(va, 4)?.try_into().unwrap()))
    };
    Ok(Decoded {
        digit_groups: digit_groups(&code, &data)?,
        selection_cap: selection_cap(&code)?,
        double_press_seconds: double_press(&code, &data)?,
        error_keys: error_keys(&code, read, &word)?,
    })
}

fn digit_groups<'a>(
    code: &impl Fn(u32, usize) -> Result<&'a [u8], String>,
    data: &impl Fn(u32, usize) -> Result<&'a [u8], String>,
) -> Result<[u8; GROUP_COUNT], String> {
    let &[0x83, 0xc0, bias] = code(KEY_BIAS, 3)? else {
        return Err("missing virtual-key bias instruction".into());
    };
    let bias = u32::from(bias.wrapping_neg());
    let &[0x3d, l0, l1, l2, l3] = code(KEY_LIMIT, 5)? else {
        return Err("missing virtual-key limit instruction".into());
    };
    let limit = u32::from_le_bytes([l0, l1, l2, l3]);
    let &[
        0x0f,
        0xb6,
        0x80,
        a0,
        a1,
        a2,
        a3,
        0xff,
        0x24,
        0x85,
        b0,
        b1,
        b2,
        b3,
    ] = code(KEY_SWITCH, 14)?
    else {
        return Err("missing virtual-key switch instructions".into());
    };
    let indexes = u32::from_le_bytes([a0, a1, a2, a3]);
    let targets = u32::from_le_bytes([b0, b1, b2, b3]);
    let mut groups = [0u8; GROUP_COUNT];
    for (digit, group) in (0u32..).zip(groups.iter_mut()) {
        let index = (VK_DIGIT_ZERO + digit)
            .checked_sub(bias)
            .filter(|index| *index <= limit)
            .ok_or("digit key lies outside the virtual-key switch")?;
        let case = data(at(indexes, index)?, 1)?[0];
        let target = u32::from_le_bytes(
            data(at(targets, 4 * u32::from(case))?, 4)?
                .try_into()
                .unwrap(),
        );
        let &[0xc7, 0x45, _, g0, g1, g2, g3] = code(target, 7)? else {
            return Err(format!("digit {digit} case does not store a group number"));
        };
        *group = u8::try_from(u32::from_le_bytes([g0, g1, g2, g3]))
            .ok()
            .filter(|group| usize::from(*group) < GROUP_COUNT)
            .ok_or_else(|| format!("digit {digit} binds a group outside 0..{GROUP_COUNT}"))?;
    }
    let mut seen = [false; GROUP_COUNT];
    for group in groups {
        if std::mem::replace(&mut seen[usize::from(group)], true) {
            return Err("two digit keys bind the same control group".into());
        }
    }
    Ok(groups)
}

fn selection_cap<'a>(
    code: &impl Fn(u32, usize) -> Result<&'a [u8], String>,
) -> Result<usize, String> {
    let &[0xbb, d0, d1, d2, d3, 0x39, 0x1f, 0x0f, 0x42, 0x1f] = code(CAP_DEFINE, 10)? else {
        return Err("missing define cap instructions".into());
    };
    let &[0x83, 0xfe, mirror, 0x73, _] = code(CAP_MIRROR, 5)? else {
        return Err("missing mirror cap instructions".into());
    };
    let &[0xb8, s0, s1, s2, s3] = code(CAP_SELECT, 5)? else {
        return Err("missing select cap instruction".into());
    };
    let define = u32::from_le_bytes([d0, d1, d2, d3]);
    if define != u32::from(mirror) || define != u32::from_le_bytes([s0, s1, s2, s3]) {
        return Err("cap sites disagree".into());
    }
    usize::try_from(define)
        .ok()
        .filter(|cap| *cap > 0)
        .ok_or_else(|| "selection cap is zero".into())
}

fn double_press<'a>(
    code: &impl Fn(u32, usize) -> Result<&'a [u8], String>,
    data: &impl Fn(u32, usize) -> Result<&'a [u8], String>,
) -> Result<f32, String> {
    let &[
        0x3b,
        0x5e,
        digit_field,
        0x75,
        _,
        0xf3,
        0x0f,
        0x10,
        0x46,
        time_field,
        0xf3,
        0x0f,
        0x58,
        0x05,
        w0,
        w1,
        w2,
        w3,
        0x0f,
        0x2f,
        0xc1,
        0x76,
        _,
    ] = code(TAP_COMPARE, 23)?
    else {
        return Err("missing double-press compare instructions".into());
    };
    if digit_field == time_field {
        return Err("double-press digit and time share one field".into());
    }
    let window = f32::from_le_bytes(
        data(u32::from_le_bytes([w0, w1, w2, w3]), 4)?
            .try_into()
            .unwrap(),
    );
    if window.is_finite() && window > 0.0 {
        Ok(window)
    } else {
        Err("double-press window is not a positive finite number".into())
    }
}

fn error_keys<'a>(
    code: &impl Fn(u32, usize) -> Result<&'a [u8], String>,
    read: &impl Fn(u32, bool) -> Result<&'a [u8], String>,
    word: &impl Fn(u32) -> Result<u32, String>,
) -> Result<ErrorKeys, String> {
    // Empty: `5 + (non-unit object selected)`. The other two sites subtract one
    // when a unit was refused, so the base is the empty code in all three.
    let &[0xf7, 0xd8, 0x1b, 0xc0, 0xf7, 0xd8, 0x83, 0xc0, empty] = code(CODE_SELECTION_EMPTY, 9)?
    else {
        return Err("missing selection-empty error code".into());
    };
    let &[
        0xf7,
        0xd9,
        0x6a,
        0x00,
        0x1b,
        0xc9,
        0x6a,
        0x00,
        0x83,
        0xc1,
        merged,
    ] = code(CODE_MERGED_EMPTY, 11)?
    else {
        return Err("missing merged-empty error code".into());
    };
    let &[
        0xf7,
        0xdb,
        0x6a,
        0x00,
        0x1b,
        0xdb,
        0x6a,
        0x00,
        0x83,
        0xc3,
        filtered,
    ] = code(CODE_FILTERED_EMPTY, 11)?
    else {
        return Err("missing filtered-empty error code".into());
    };
    if empty != merged || empty != filtered {
        return Err("error code bases disagree".into());
    }
    let &[0x83, 0xc0, bias] = code(NAME_BIAS, 3)? else {
        return Err("missing error name bias".into());
    };
    let bias = u32::from(bias.wrapping_neg());
    let &[0x3d, l0, l1, l2, l3] = code(NAME_LIMIT, 5)? else {
        return Err("missing error name limit".into());
    };
    let limit = u32::from_le_bytes([l0, l1, l2, l3]);
    let &[0xff, 0x24, 0x85, t0, t1, t2, t3] = code(NAME_SWITCH, 7)? else {
        return Err("missing error name switch".into());
    };
    let table = u32::from_le_bytes([t0, t1, t2, t3]);
    let name = |number: u8| -> Result<String, String> {
        let index = u32::from(number)
            .checked_sub(bias)
            .filter(|index| *index <= limit)
            .ok_or("error code lies outside the name switch")?;
        let target = word(at(table, 4 * index)?)?;
        let &[0xb9, s0, s1, s2, s3] = code(target, 5)? else {
            return Err(format!("error code {number} does not load a name"));
        };
        let text = read(u32::from_le_bytes([s0, s1, s2, s3]), false)?;
        let text = &text[..text.len().min(MAX_KEY_NAME)];
        let end = text
            .iter()
            .position(|byte| *byte == 0)
            .ok_or("error key name is not terminated")?;
        std::str::from_utf8(&text[..end])
            .ok()
            .filter(|name| !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric()))
            .map(str::to_owned)
            .ok_or_else(|| format!("error code {number} names an invalid key"))
    };
    Ok(ErrorKeys {
        invalid: name(empty.checked_sub(1).ok_or("error base is zero")?)?,
        empty: name(empty)?,
        non_unit: name(empty.checked_add(1).ok_or("error base overflows")?)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// Synthetic address space: instruction and data bytes by start address.
    struct Space(BTreeMap<u32, Vec<u8>>);

    impl Space {
        fn read(&self, va: u32, _code: bool) -> Result<&[u8], String> {
            self.0
                .range(..=va)
                .next_back()
                .and_then(|(start, bytes)| bytes.get((va - start) as usize..))
                .filter(|rest| !rest.is_empty())
                .ok_or_else(|| format!("unmapped {va:#x}"))
        }
        fn put(&mut self, va: u32, bytes: &[u8]) {
            self.0.insert(va, bytes.to_vec());
        }
    }

    const INDEXES: u32 = 0x700000;
    const TARGETS: u32 = 0x700400;
    const CASES: u32 = 0x710000;
    const WINDOW: u32 = 0x720000;
    const NAME_TABLE: u32 = 0x730000;
    const NAME_CASES: u32 = 0x731000;
    const NAMES: u32 = 0x732000;

    /// `groups[d]` is the group the synthetic image binds to digit `d`.
    fn space(groups: [u8; 10], cap: u32, window: f32, base: u8) -> Space {
        let mut space = Space(BTreeMap::new());
        space.put(KEY_BIAS, &[0x83, 0xc0, 0xee]);
        space.put(KEY_LIMIT, &[0x3d, 0xcb, 0, 0, 0]);
        let mut switch = vec![0x0f, 0xb6, 0x80];
        switch.extend(INDEXES.to_le_bytes());
        switch.extend([0xff, 0x24, 0x85]);
        switch.extend(TARGETS.to_le_bytes());
        space.put(KEY_SWITCH, &switch);
        // Case numbers: digit d (virtual key 0x30 + d, index 0x1e + d) uses case d.
        let mut indexes = vec![9u8; 0xcc];
        let mut targets = Vec::new();
        for (digit, group) in groups.iter().enumerate() {
            indexes[0x1e + digit] = digit as u8;
            let at = CASES + 16 * digit as u32;
            targets.extend(at.to_le_bytes());
            let mut body = vec![0xc7, 0x45, 0xf0];
            body.extend(u32::from(*group).to_le_bytes());
            space.put(at, &body);
        }
        space.put(INDEXES, &indexes);
        space.put(TARGETS, &targets);
        space.put(
            CAP_DEFINE,
            &[
                &[0xbb][..],
                &cap.to_le_bytes(),
                &[0x39, 0x1f, 0x0f, 0x42, 0x1f],
            ]
            .concat(),
        );
        space.put(CAP_MIRROR, &[0x83, 0xfe, cap as u8, 0x73, 0x17]);
        space.put(CAP_SELECT, &[&[0xb8][..], &cap.to_le_bytes()].concat());
        let mut tap = vec![
            0x3b, 0x5e, 0x0c, 0x75, 0x40, 0xf3, 0x0f, 0x10, 0x46, 0x10, 0xf3, 0x0f, 0x58, 0x05,
        ];
        tap.extend(WINDOW.to_le_bytes());
        tap.extend([0x0f, 0x2f, 0xc1, 0x76, 0x2e]);
        space.put(TAP_COMPARE, &tap);
        space.put(WINDOW, &window.to_le_bytes());
        space.put(
            CODE_SELECTION_EMPTY,
            &[0xf7, 0xd8, 0x1b, 0xc0, 0xf7, 0xd8, 0x83, 0xc0, base],
        );
        space.put(
            CODE_MERGED_EMPTY,
            &[0xf7, 0xd9, 0x6a, 0, 0x1b, 0xc9, 0x6a, 0, 0x83, 0xc1, base],
        );
        space.put(
            CODE_FILTERED_EMPTY,
            &[0xf7, 0xdb, 0x6a, 0, 0x1b, 0xdb, 0x6a, 0, 0x83, 0xc3, base],
        );
        space.put(NAME_BIAS, &[0x83, 0xc0, 0xfd]);
        space.put(NAME_LIMIT, &[0x3d, 0xd8, 0, 0, 0]);
        let mut switch = vec![0xff, 0x24, 0x85];
        switch.extend(NAME_TABLE.to_le_bytes());
        space.put(NAME_SWITCH, &switch);
        let mut table = Vec::new();
        let mut names = Vec::new();
        for (index, name) in ["Alpha", "Bravo", "Charlie", "Delta", "Echo"]
            .into_iter()
            .enumerate()
        {
            let case = NAME_CASES + 16 * index as u32;
            let text = NAMES + 16 * index as u32;
            table.extend(case.to_le_bytes());
            space.put(case, &[&[0xb9][..], &text.to_le_bytes()].concat());
            let mut bytes = name.as_bytes().to_vec();
            bytes.push(0);
            names.push((text, bytes));
        }
        space.put(NAME_TABLE, &table);
        for (text, bytes) in names {
            space.put(text, &bytes);
        }
        space
    }

    const SYNTHETIC: [u8; 10] = [3, 0, 1, 2, 9, 8, 7, 6, 5, 4];

    fn decoded(space: &Space) -> Result<Decoded, String> {
        decode(&|va, code| space.read(va, code))
    }

    #[test]
    fn values_come_from_operands_and_tables() {
        // Name numbers 4, 5 and 6 are table indexes 1, 2 and 3 after the bias of 3.
        let rules = decoded(&space(SYNTHETIC, 7, 0.75, 5)).unwrap();
        assert_eq!(rules.digit_groups, SYNTHETIC);
        assert_eq!(rules.selection_cap, 7);
        assert_eq!(rules.double_press_seconds, 0.75);
        assert_eq!(
            rules.error_keys,
            ErrorKeys {
                invalid: "Bravo".into(),
                empty: "Charlie".into(),
                non_unit: "Delta".into(),
            }
        );
    }

    #[test]
    fn cap_sites_that_disagree_refuse() {
        let mut broken = space(SYNTHETIC, 7, 0.75, 5);
        broken.put(CAP_MIRROR, &[0x83, 0xfe, 6, 0x73, 0x17]);
        assert_eq!(decoded(&broken).unwrap_err(), "cap sites disagree");
    }

    #[test]
    fn repeated_groups_refuse() {
        let mut groups = SYNTHETIC;
        groups[1] = groups[0];
        assert_eq!(
            decoded(&space(groups, 7, 0.75, 5)).unwrap_err(),
            "two digit keys bind the same control group"
        );
        groups[1] = 10;
        assert!(
            decoded(&space(groups, 7, 0.75, 5))
                .unwrap_err()
                .contains("outside")
        );
    }

    #[test]
    fn unusable_windows_refuse() {
        for window in [0.0, -0.5, f32::NAN, f32::INFINITY] {
            assert_eq!(
                decoded(&space(SYNTHETIC, 7, window, 5)).unwrap_err(),
                "double-press window is not a positive finite number"
            );
        }
    }

    #[test]
    fn error_bases_that_disagree_or_leave_the_table_refuse() {
        let mut broken = space(SYNTHETIC, 7, 0.75, 5);
        broken.put(
            CODE_FILTERED_EMPTY,
            &[0xf7, 0xdb, 0x6a, 0, 0x1b, 0xdb, 0x6a, 0, 0x83, 0xc3, 6],
        );
        assert_eq!(decoded(&broken).unwrap_err(), "error code bases disagree");
        // Base 8 puts the non-unit code at 9, past the five synthetic names.
        assert!(decoded(&space(SYNTHETIC, 7, 0.75, 8)).is_err());
    }

    #[test]
    fn other_images_refuse_before_decoding() {
        let origin = AssetIdentity {
            source: "synthetic".into(),
            edition: "fixture".into(),
            version: "fixture".into(),
        };
        let error = ControlGroupRules::read(&[0; 16], origin).unwrap_err();
        assert!(error.contains("unsupported client image"), "{error}");
    }

    /// Needs the Legacy 1.29.2.9231 `Warcraft III.exe` named by `PCX_WC3_INSTALL`.
    #[test]
    #[ignore = "requires PCX_WC3_INSTALL with the reviewed Legacy Warcraft III.exe"]
    fn reviewed_client_yields_the_documented_rules() {
        let install = std::env::var("PCX_WC3_INSTALL").expect("PCX_WC3_INSTALL");
        let path = std::path::Path::new(&install).join("Warcraft III.exe");
        let image = std::fs::read(&path).unwrap();
        let rules = ControlGroupRules::read(
            &image,
            AssetIdentity {
                source: path.display().to_string(),
                edition: "selected client".into(),
                version: "reviewed Legacy 1.29.2.9231".into(),
            },
        )
        .unwrap();
        assert_eq!(rules.digit_groups, [9, 0, 1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(rules.selection_cap, 12);
        assert_eq!(rules.double_press_seconds, 0.5);
        assert_eq!(rules.error_keys.invalid, "Invalidcontrolgroup");
        assert_eq!(rules.error_keys.empty, "Emptycontrolgroup");
        assert_eq!(rules.error_keys.non_unit, "Nonunitincontrolgroup");
    }
}

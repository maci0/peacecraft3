//! Ordinary autocast OFF capability bound to named selected-client ability classes.
use crate::{
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    pe_image::occurrences,
    scripts::AssetIdentity,
};

/// What the shared ON handler does beyond the state bit, decoded from the class.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Enable {
    /// Abilities of the same unit with an equal nonzero group are switched OFF by this ON.
    pub group: u32,
    /// Game seconds from the ON until the first automatic-cast evaluation.
    pub first_tick: f32,
}
#[derive(Clone, Debug)]
pub struct Class {
    pub code: [u8; 4],
    pub on: i32,
    pub off: i32,
    /// `None` when the class's ON handler, update hook or activation differs from the decoded shape.
    pub enable: Option<Enable>,
}
#[derive(Clone, Debug)]
pub struct Rules {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    pub classes: Vec<Class>,
    /// Every class sharing the ON handler whose group is nonzero: `(code, group)`.
    pub groups: Vec<([u8; 4], u32)>,
    /// Class code of the ability `UnitIgnoreAlarm` looks up. `None` when the source was not read
    /// from the image, so the natives refuse.
    pub alarm_code: Option<[u8; 4]>,
}
/// Native class names whose getter, class ABI and OFF handler are read from the selected client.
const CLASS_NAMES: [&str; 17] = [
    "CAbilitySlow",
    "CAbilityInnerFire",
    "CAbilityHeal",
    "CAbilityRaiseDead",
    "CAbilityAutoDispelMagic",
    "CAbilityWeb",
    "CAbilityFaerieFire",
    "CAbilityRenew",
    "CAbilityBloodlust",
    "CAbilityFrostArmorAuto",
    "CAbilityRestoration",
    "CAbilityManaShield",
    "CAbilityImmolation",
    "CAbilityRepair",
    "CAbilityBlackArrow",
    "CAbilityCurse",
    "CAbilityManaBattery",
];
const SCANS_PER_CLASS: usize = 3;
/// One data scan for the vtables that share an ON handler.
const FAMILY_SCANS: usize = 1;
fn word(b: &[u8], at: usize) -> Result<u32, String> {
    b.get(at..at + 4)
        .and_then(|s| s.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| "autocast operand truncated".into())
}
fn shape(b: &[u8], at: usize, s: &[u8]) -> Result<(), String> {
    if b.get(at..at + s.len()) == Some(s) {
        Ok(())
    } else {
        Err(format!("autocast instruction differs at {at}"))
    }
}
fn unique(mut found: impl Iterator<Item = u32>) -> Result<u32, String> {
    let first = found.next().ok_or("autocast source anchor missing")?;
    if found.next().is_some() {
        return Err("autocast source anchor ambiguous".into());
    }
    Ok(first)
}
impl Rules {
    pub fn read(image: &[u8], origin: AssetIdentity) -> Result<Self, String> {
        if image.len() > 32 << 20 || sha256(image) != crate::menu_lighting::REVIEWED_IMAGE {
            return Err("autocast selected client differs".into());
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
        let mut read = |va, n, code| {
            pe.read_from(va, code)
                .map_err(|e| e.to_string())?
                .get(..n)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| "autocast mapped window truncated".into())
        };
        let alarm_code = Self::alarm_code(&mut read)?;
        let mut rules = Self::decode(
            origin,
            sha256(image),
            pe.code_ranges().collect(),
            pe.initialized_ranges().collect(),
            // Each class scans the sections three times (name, getter, name slot); sections do not overlap.
            (CLASS_NAMES.len() * SCANS_PER_CLASS + FAMILY_SCANS) * image.len(),
            read,
        )?;
        rules.alarm_code = Some(alarm_code);
        Ok(rules)
    }
    /// The class code `UnitIgnoreAlarm` (`0x4af2d0`) and `UnitIgnoreAlarmToggled` (`0x4af330`)
    /// pass to the unit's ability lookup (`0x6d6570`), read from the getter both call
    /// (`0xb167f0`), after checking that each sets, clears or reads state bit `0x80`.
    fn alarm_code(
        read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
    ) -> Result<[u8; 4], String> {
        let mut code = None;
        for lookup in [ALARM_SET_LOOKUP, ALARM_QUERY_LOOKUP] {
            let b = read(lookup, 13, true)?;
            shape(&b, 0, &[0x6a, 1, 0x6a, 1, 0x6a, 0, 0x6a, 0, 0xe8])?;
            let getter = lookup.wrapping_add(13).wrapping_add(word(&b, 9)?);
            let g = read(getter, 6, true)?;
            shape(&g, 0, &[0xb8])?;
            shape(&g, 5, &[0xc3])?;
            let found = word(&g, 1)?.to_be_bytes();
            if !found.iter().all(u8::is_ascii_alphanumeric)
                || code.replace(found).is_some_and(|old| old != found)
            {
                return Err("autocast alarm class code differs between its natives".into());
            }
        }
        for (va, bytes) in [
            (ALARM_SET_BIT, &[0x81, 0x48, 0x20, 0x80, 0, 0, 0][..]),
            (ALARM_CLEAR_BIT, &[0x81, 0x60, 0x20, 0x7f, 0xff, 0xff, 0xff]),
            (
                ALARM_READ_BIT,
                &[0x8b, 0x40, 0x20, 0xc1, 0xe8, 0x07, 0x83, 0xe0, 0x01],
            ),
        ] {
            shape(&read(va, bytes.len(), true)?, 0, bytes)?;
        }
        code.ok_or_else(|| "autocast alarm class code missing".into())
    }
    fn decode(
        origin: AssetIdentity,
        image_sha256: Sha256Digest,
        code: Vec<(u32, &[u8])>,
        data: Vec<(u32, &[u8])>,
        mut work: usize,
        mut read: impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
    ) -> Result<Self, String> {
        let mut find = |ranges: &[(u32, &[u8])], needle: &[u8]| -> Result<u32, String> {
            let mut matches = Vec::new();
            for &(va, b) in ranges {
                work = work
                    .checked_sub(b.len())
                    .ok_or("autocast scan work exhausted")?;
                for i in occurrences(b, needle) {
                    if matches.len() == 2 {
                        return Err("autocast source anchor ambiguous".into());
                    }
                    matches.push(
                        va.checked_add(i.try_into().map_err(|_| "autocast offset overflow")?)
                            .ok_or("autocast address overflow")?,
                    )
                }
            }
            unique(matches.into_iter())
        };
        let mut classes = Vec::new();
        let mut shared: Vec<EnableDecode> = Vec::new();
        for name in CLASS_NAMES {
            let name_va = find(&data, format!("{name}\0").as_bytes())?;
            let mut getter = vec![0xb8];
            getter.extend(name_va.to_le_bytes());
            getter.push(0xc3);
            let getter_va = find(&code, &getter)?;
            let name_slot = find(&data, &getter_va.to_le_bytes())?;
            // Reviewed x86 class ABI: name slot, code slot and immediate on/off virtuals.
            let table = name_slot
                .checked_sub(0x58)
                .ok_or("autocast vtable underflow")?;
            let vtable = read(table, 0x350, false)?;
            let mut constant = |slot: usize| -> Result<u32, String> {
                let f = word(&vtable, slot)?;
                let b = read(f, 6, true)?;
                shape(&b, 0, &[0xb8])?;
                shape(&b, 5, &[0xc3])?;
                word(&b, 1)
            };
            let class = Class {
                code: constant(0x1c)?.to_be_bytes(),
                on: constant(0x310)? as i32,
                off: constant(0x320)? as i32,
                enable: None,
            };
            if !class.code.iter().all(u8::is_ascii_alphanumeric)
                || class.on == 0
                || class.off == 0
                || class.on == class.off
            {
                return Err("autocast class/order relationship invalid".into());
            }
            let wrapper = read(word(&vtable, 0x33c)?, 11, true)?;
            shape(&wrapper, 0, &[0x8b, 0x01, 0xff, 0x90])?;
            shape(&wrapper, 8, &[0xc2, 0x04, 0x00])?;
            let off_slot =
                usize::try_from(word(&wrapper, 4)?).map_err(|_| "autocast off slot overflow")?;
            if off_slot % 4 != 0 || off_slot + 4 > vtable.len() {
                return Err("autocast off slot outside class".into());
            }
            let off = read(word(&vtable, off_slot)?, 142, true)?;
            shape(&off, 0, &[0x56, 0x8b, 0xf1, 0x57, 0x8b, 0x7e])?;
            shape(&off, 47, &[0xf6, 0x46])?;
            shape(&off, 51, &[0x74])?;
            shape(&off, 53, &[0x81, 0x66])?;
            if usize::from(off[52]) + 53 != 139 {
                return Err("autocast repeated OFF branch differs".into());
            }
            let field = off[49];
            let mask = off[50];
            if mask == 0
                || !mask.is_power_of_two()
                || off[55] != field
                || word(&off, 56)? != !u32::from(mask)
            {
                return Err("autocast OFF does not clear its tested state bit".into());
            }
            shape(&off, 60, &[0x8b, 0xce, 0x8b, 0x06, 0xff, 0x90])?;
            shape(&off, 139, &[0x5f, 0x5e, 0xc3])?;
            if classes
                .iter()
                .any(|c: &Class| c.code == class.code || c.on == class.on || c.off == class.off)
            {
                return Err("autocast class/order collision".into());
            }
            // An ON handler that differs from the decoded shape leaves ON refused for this class.
            let enable = Self::decode_enable(&vtable, table, field, mask, &mut read).ok();
            classes.push(Class {
                enable: enable.map(|e| e.enable),
                ..class
            });
            if let Some(e) = enable
                && !shared.iter().any(|s| s.handler == e.handler)
            {
                shared.push(e);
            }
        }
        let mut groups: Vec<([u8; 4], u32)> = Vec::new();
        for shared in shared {
            let needle = shared.handler.to_le_bytes();
            for &(va, bytes) in &data {
                work = work
                    .checked_sub(bytes.len())
                    .ok_or("autocast scan work exhausted")?;
                for i in occurrences(bytes, &needle) {
                    let slot = va
                        .checked_add(i.try_into().map_err(|_| "autocast offset overflow")?)
                        .ok_or("autocast address overflow")?;
                    if slot % 4 != 0 {
                        continue;
                    }
                    let Some(table) = slot.checked_sub(shared.on_slot) else {
                        continue;
                    };
                    let Some((code, group)) = Self::family_member(table, shared, &mut read)? else {
                        continue;
                    };
                    match groups.iter().find(|(c, _)| *c == code) {
                        Some(&(_, known)) if known != group => {
                            return Err("autocast class code has two groups".into());
                        }
                        Some(_) => {}
                        None if group != 0 => groups.push((code, group)),
                        None => {}
                    }
                }
            }
        }
        Ok(Self {
            origin,
            image_sha256,
            classes,
            groups,
            alarm_code: None,
        })
    }
    /// Decodes the shared ON handler, its update hook and the timer the activation arms.
    fn decode_enable(
        vtable: &[u8],
        table: u32,
        field: u8,
        mask: u8,
        read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
    ) -> Result<EnableDecode, String> {
        let wrapper = read(word(vtable, ON_WRAPPER_SLOT)?, 11, true)?;
        shape(&wrapper, 0, &[0x8b, 0x01, 0xff, 0x90])?;
        shape(&wrapper, 8, &[0xc2, 0x04, 0x00])?;
        let on_slot = slot_index(word(&wrapper, 4)?, vtable.len())?;
        let handler = word(vtable, on_slot)?;
        let on = read(handler, ON_HANDLER_BYTES, true)?;
        // test [this+state], bit; or [this+state], bit; call [group]; tail jmp [hook].
        shape(&on, 47, &[0xf6, 0x46, field, mask])?;
        shape(&on, 113, &[0x81, 0x4e, field, mask, 0, 0, 0])?;
        shape(&on, 79, &[0xff, 0x90])?;
        shape(&on, 133, &[0xff, 0xa0])?;
        let group_slot = slot_index(word(&on, 81)?, vtable.len())?;
        let hook_slot = slot_index(word(&on, 135)?, vtable.len())?;
        let group = group_of(&read(word(vtable, group_slot)?, 6, true)?)?;
        // The hook jumps to the update routine, which arms the activation virtual.
        let hook_va = word(vtable, hook_slot)?;
        let hook = read(hook_va, 5, true)?;
        shape(&hook, 0, &[0xe9])?;
        let update_va = hook_va.wrapping_add(5).wrapping_add(word(&hook, 1)?);
        let update = read(update_va, 61, true)?;
        shape(&update, 0, &[0x83, 0x79, 0x3c, 0x00])?;
        shape(&update, 16, &[0xa8])?;
        if update[17] & mask == 0 {
            return Err("autocast update does not test the ON state bit".into());
        }
        shape(&update, 46, &[0x81, 0x49, field])?;
        shape(&update, 53, &[0x8b, 0x01, 0xff, 0x90])?;
        let activate_slot =
            usize::try_from(word(&update, 57)?).map_err(|_| "autocast activation slot overflow")?;
        if activate_slot % 4 != 0 {
            return Err("autocast activation slot unaligned".into());
        }
        let activate_va = word(
            &read(
                table
                    .checked_add(activate_slot as u32)
                    .ok_or("autocast activation slot overflow")?,
                4,
                false,
            )?,
            0,
        )?;
        let activate = read(activate_va, 24, true)?;
        shape(&activate, 0, &[0xf7, 0x41, field])?;
        shape(&activate, 9, &[0x6a, 0, 0x6a, 0, 0x51, 0x68])?;
        shape(&activate, 19, &[0x68])?;
        let delay = f32::from_bits(word(&read(word(&activate, 20)?, 4, false)?, 0)?);
        if !delay.is_finite() || delay <= 0. {
            return Err("autocast first tick delay invalid".into());
        }
        Ok(EnableDecode {
            enable: Enable {
                group,
                first_tick: delay,
            },
            handler,
            on_slot: on_slot as u32,
            group_slot,
        })
    }
    /// The code and group of a class vtable that shares `shared`'s ON handler, or `None` when
    /// the candidate is not a class table.
    fn family_member(
        table: u32,
        shared: EnableDecode,
        read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
    ) -> Result<Option<([u8; 4], u32)>, String> {
        let Some(code) = slot_function(read, table, CODE_SLOT, 6) else {
            return Ok(None);
        };
        if shape(&code, 0, &[0xb8]).is_err() || shape(&code, 5, &[0xc3]).is_err() {
            return Ok(None);
        }
        let code = word(&code, 1)?.to_be_bytes();
        if !code.iter().all(u8::is_ascii_alphanumeric) {
            return Ok(None);
        }
        let group = slot_function(read, table, shared.group_slot as u32, 6)
            .ok_or("autocast group slot unreadable")?;
        Ok(Some((code, group_of(&group)?)))
    }
}
/// The first `len` code bytes of the function a class table's `slot` points to.
fn slot_function(
    read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
    table: u32,
    slot: u32,
    len: usize,
) -> Option<Vec<u8>> {
    let target = word(&read(table.checked_add(slot)?, 4, false).ok()?, 0).ok()?;
    read(target, len, true).ok()
}
/// Decoded ON behaviour shared by a family of class vtables.
#[derive(Clone, Copy)]
struct EnableDecode {
    enable: Enable,
    handler: u32,
    on_slot: u32,
    group_slot: usize,
}
/// Where `UnitIgnoreAlarm` and `UnitIgnoreAlarmToggled` push the lookup arguments and call the
/// class-code getter, and the instructions that set, clear and read the ignore bit.
const ALARM_SET_LOOKUP: u32 = 0x4af2ea;
const ALARM_QUERY_LOOKUP: u32 = 0x4af34a;
const ALARM_SET_BIT: u32 = 0x4af309;
const ALARM_CLEAR_BIT: u32 = 0x4af318;
const ALARM_READ_BIT: u32 = 0x4af363;
/// Class-code getter slot of the reviewed class ABI.
const CODE_SLOT: u32 = 0x1c;
/// Wrapper slot that forwards to the ON handler (the OFF wrapper is at 0x33c).
const ON_WRAPPER_SLOT: usize = 0x338;
/// Bytes of the ON handler read: through its tail `jmp [hook]`.
const ON_HANDLER_BYTES: usize = 139;
/// `mov eax, imm32; ret` or `xor eax, eax; ret`.
fn group_of(b: &[u8]) -> Result<u32, String> {
    if b.get(..3) == Some(&[0x33, 0xc0, 0xc3]) {
        return Ok(0);
    }
    shape(b, 0, &[0xb8])?;
    shape(b, 5, &[0xc3])?;
    word(b, 1)
}
fn slot_index(operand: u32, vtable_len: usize) -> Result<usize, String> {
    let slot = usize::try_from(operand).map_err(|_| "autocast slot overflow")?;
    if slot % 4 != 0 || slot + 4 > vtable_len {
        return Err("autocast slot outside class".into());
    }
    Ok(slot)
}

#[cfg(test)]
#[path = "autocast_rules_tests.rs"]
mod tests;

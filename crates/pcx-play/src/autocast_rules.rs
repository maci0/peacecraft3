//! Ordinary autocast OFF capability bound to named selected-client ability classes.
use crate::{
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    pe_image::occurrences,
    scripts::AssetIdentity,
};

#[derive(Clone, Debug)]
pub struct Class {
    pub code: [u8; 4],
    pub on: i32,
    pub off: i32,
}
#[derive(Clone, Debug)]
pub struct Rules {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    pub classes: Vec<Class>,
}
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
        Self::decode(
            origin,
            sha256(image),
            pe.code_ranges().collect(),
            pe.initialized_ranges().collect(),
            128 << 20,
            |va, n, code| {
                pe.read_from(va, code)
                    .map_err(|e| e.to_string())?
                    .get(..n)
                    .map(<[u8]>::to_vec)
                    .ok_or_else(|| "autocast mapped window truncated".into())
            },
        )
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
        for name in ["CAbilitySlow", "CAbilityInnerFire", "CAbilityHeal"] {
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
            classes.push(class);
        }
        Ok(Self {
            origin,
            image_sha256,
            classes,
        })
    }
}

#[cfg(test)]
#[path = "autocast_rules_tests.rs"]
mod tests;

//! Root and unroot orders of the selected client's `CAbilityRoot` class.
use crate::{
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    pe_image::occurrences,
    scripts::AssetIdentity,
};

const CLASS_NAME: &str = "CAbilityRoot";
/// Bytes of the vtable read: slots through `0x320` plus the validity slots.
const VTABLE_BYTES: usize = 0x350;
/// Class ABI slots (reviewed 1.29.2.9231): name `0x58`, code `0x1c`, root `0x310`, unroot `0x320`,
/// root and unroot order validity `0x308` and `0x30c`.
const NAME_SLOT: usize = 0x58;
const CODE_SLOT: usize = 0x1c;
const ROOT_ORDER_SLOT: usize = 0x310;
const UNROOT_ORDER_SLOT: usize = 0x320;
const ROOT_VALIDITY_SLOT: usize = 0x308;
const UNROOT_VALIDITY_SLOT: usize = 0x30c;
const VALIDITY_BYTES: usize = 0x40;
/// Offsets of the order compare and state-bit mask inside the two validity functions.
const UNROOT_COMPARE_AT: usize = 6;
const UNROOT_MASK_AT: usize = 0x3c;
const ROOT_COMPARE_AT: usize = 0x15;
const ROOT_MASK_AT: usize = 0x3a;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Class {
    pub code: [u8; 4],
    /// The point order `root`. As an immediate order the class refuses it.
    pub root: i32,
    pub unroot: i32,
    /// Unit state bit tested by both validity functions: set while rooted.
    pub rooted_state_bit: u32,
}
#[derive(Clone, Debug)]
pub struct Rules {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    pub class: Class,
}
fn word(b: &[u8], at: usize) -> Result<u32, String> {
    b.get(at..at + 4)
        .and_then(|s| s.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| "root operand truncated".into())
}
fn shape(b: &[u8], at: usize, s: &[u8]) -> Result<(), String> {
    if b.get(at..at + s.len()) == Some(s) {
        Ok(())
    } else {
        Err(format!("root instruction differs at {at}"))
    }
}
fn unique(mut found: impl Iterator<Item = u32>) -> Result<u32, String> {
    let first = found.next().ok_or("root source anchor missing")?;
    if found.next().is_some() {
        return Err("root source anchor ambiguous".into());
    }
    Ok(first)
}
impl Rules {
    pub fn read(image: &[u8], origin: AssetIdentity) -> Result<Self, String> {
        if image.len() > 32 << 20 || sha256(image) != crate::menu_lighting::REVIEWED_IMAGE {
            return Err("root selected client differs".into());
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
            // Name, getter and name slot are each one scan of the sections.
            3 * image.len(),
            |va, n, code| {
                pe.read_from(va, code)
                    .map_err(|e| e.to_string())?
                    .get(..n)
                    .map(<[u8]>::to_vec)
                    .ok_or_else(|| "root mapped window truncated".into())
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
                    .ok_or("root scan work exhausted")?;
                for i in occurrences(b, needle) {
                    if matches.len() == 2 {
                        return Err("root source anchor ambiguous".into());
                    }
                    matches.push(
                        va.checked_add(i.try_into().map_err(|_| "root offset overflow")?)
                            .ok_or("root address overflow")?,
                    )
                }
            }
            unique(matches.into_iter())
        };
        let name_va = find(&data, format!("{CLASS_NAME}\0").as_bytes())?;
        let mut getter = vec![0xb8];
        getter.extend(name_va.to_le_bytes());
        getter.push(0xc3);
        let getter_va = find(&code, &getter)?;
        let name_slot = find(&data, &getter_va.to_le_bytes())?;
        let table = name_slot
            .checked_sub(NAME_SLOT as u32)
            .ok_or("root vtable underflow")?;
        let vtable = read(table, VTABLE_BYTES, false)?;
        let mut constant = |slot: usize| -> Result<u32, String> {
            let f = word(&vtable, slot)?;
            let b = read(f, 6, true)?;
            shape(&b, 0, &[0xb8])?;
            shape(&b, 5, &[0xc3])?;
            word(&b, 1)
        };
        let code_word = constant(CODE_SLOT)?.to_be_bytes();
        let root = constant(ROOT_ORDER_SLOT)? as i32;
        let unroot = constant(UNROOT_ORDER_SLOT)? as i32;
        if !code_word.iter().all(u8::is_ascii_alphanumeric)
            || root == 0
            || unroot == 0
            || root == unroot
        {
            return Err("root class/order relationship invalid".into());
        }
        // Validity functions: `push ebp; mov ebp, esp; cmp [ebp+8], order` then, later, the
        // test of one unit state bit. Both must test the same bit.
        let unroot_check = read(word(&vtable, UNROOT_VALIDITY_SLOT)?, VALIDITY_BYTES, true)?;
        shape(&unroot_check, 0, &[0x55, 0x8b, 0xec, 0x81, 0x7d, 0x08])?;
        shape(&unroot_check, UNROOT_MASK_AT - 1, &[0x25])?;
        let root_check = read(word(&vtable, ROOT_VALIDITY_SLOT)?, VALIDITY_BYTES, true)?;
        shape(&root_check, ROOT_COMPARE_AT - 3, &[0x81, 0x7d, 0x08])?;
        shape(&root_check, ROOT_MASK_AT - 1, &[0x25])?;
        if word(&unroot_check, UNROOT_COMPARE_AT)? != unroot as u32
            || word(&root_check, ROOT_COMPARE_AT)? != root as u32
        {
            return Err("root validity compares a different order".into());
        }
        let rooted_state_bit = word(&unroot_check, UNROOT_MASK_AT)?;
        if !rooted_state_bit.is_power_of_two()
            || word(&root_check, ROOT_MASK_AT)? != rooted_state_bit
        {
            return Err("root validity tests different state bits".into());
        }
        Ok(Self {
            origin,
            image_sha256,
            class: Class {
                code: code_word,
                root,
                unroot,
                rooted_state_bit,
            },
        })
    }
}

#[cfg(test)]
#[path = "unit_root_rules_tests.rs"]
mod tests;

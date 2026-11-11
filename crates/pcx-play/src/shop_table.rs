//! Per-shop stock table capacity and default slot count, derived from a selected game
//! executable. One table per stock kind: items (`SetItemTypeSlots`, `AddItemToStock`) and
//! units (`SetUnitTypeSlots`, `AddUnitToStock`).
//!
//! The slot setter is registered under its name in `.rdata`, with the implementation pushed
//! right after the name and a call to the registrar. The implementation calls one helper that
//! stores the slot count. That helper clamps the count with an unsigned compare against an
//! immediate (`mov ecx, C; cmp [ebp+8], ecx; cmovb ecx, [ebp+8]`), so a negative count
//! becomes `C`. The same helper bounds the cell index with `cmp ecx, L; jae`, where `L` is
//! the number of cells the shop object holds.
//!
//! The default count is what a shop holds before any setter ran. The add native's handler
//! calls a stub that returns the ability class code of the shop component and a helper that
//! compares the count field (`cmp [reg+disp32], reg`). A second stub with the same code sits
//! in the component's vtable. The vtable function that stores an immediate into that
//! `disp32` field is the component's initializer, and the immediate is the default.
use crate::item_class_table::{Image, ItemClassError};
use crate::scripts::AssetIdentity;
use std::collections::BTreeSet;
use thiserror::Error;

const PUSH_IMM32: u8 = 0x68;
const CALL: u8 = 0xe8;
/// `mov ecx, imm32`, `cmp ecx, [ebp+8]`, `push edi`, `cmovb ecx, [ebp+8]`. The push is part of
/// this build's code; a different build fails the unique-match check instead.
const CLAMP: [u8; 13] = [
    0xb9, 0, 0, 0, 0, 0x39, 0x4d, 0x08, 0x57, 0x0f, 0x42, 0x4d, 0x08,
];
/// `cmp ecx, imm8` then `jae rel8` or `jae rel32`.
const CMP_ECX: [u8; 2] = [0x83, 0xf9];
const JAE_SHORT: u8 = 0x73;
const JAE_NEAR: [u8; 2] = [0x0f, 0x83];
const STUB: [u8; 3] = [0x33, 0xc0, 0xc3];
/// `mov eax, imm32; ret`: the stub that returns a component class code.
const MOV_EAX: u8 = 0xb8;
const RET: u8 = 0xc3;
const CLASS_STUB_LEN: usize = 6;
/// `cmp [ebx+disp32], edi`: the add helper's compare against the stored slot count.
const CMP_COUNT: [u8; 2] = [0x39, 0xbb];
/// `mov dword [reg+disp32], imm32` with a plain base register (no SIB byte).
const MOV_MEM_IMM: u8 = 0xc7;
const MODRM_MOD10_FIRST: u8 = 0x80;
const MODRM_MOD10_LAST: u8 = 0x87;
const MODRM_SIB: u8 = 0x84;
const MOV_MEM_IMM_LEN: usize = 10;
const POINTER_LEN: usize = 4;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ShopTableError {
    #[error("executable image is malformed: {0}")]
    Image(#[from] ItemClassError),
    #[error("{0} name is not in .rdata exactly once")]
    Name(&'static str),
    #[error("{0} is not registered exactly once")]
    Registration(&'static str),
    #[error("{0} implementation has no unique clamp helper")]
    Helper(&'static str),
    #[error("{0} clamp helper has no unique cell-length check")]
    Length(&'static str),
    #[error("{kind} cell count {entries} is below the slot ceiling {ceiling}")]
    Ceiling {
        kind: &'static str,
        entries: u32,
        ceiling: u32,
    },
    #[error("{0} handler has no unique class stub and slot count compare")]
    AddHandler(&'static str),
    #[error("{0} component class stub is not in .rdata exactly once")]
    Vtable(&'static str),
    #[error("{0} component vtable has no unique slot count initializer")]
    Default(&'static str),
}

/// One stock kind's table shape, read from the executable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlotTable {
    /// Largest slot count the engine stores. Larger and negative requests store this.
    pub slot_ceiling: u32,
    /// Cells per shop. Removal scans all of them.
    pub entries: u32,
    /// Slot count a shop holds before any setter ran.
    pub default_slots: u32,
}

/// Executable-derived capacity of one shop's item and unit tables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShopTable {
    /// Selected source the executable came from (edition, version, source path).
    pub origin: AssetIdentity,
    /// SHA-256 of the exact executable bytes the capacity was read from.
    pub image_sha256: [u8; 32],
    pub item: SlotTable,
    pub unit: SlotTable,
}

/// Offsets of every occurrence of `needle` in `haystack`.
fn occurrences(haystack: &[u8], needle: &[u8]) -> Vec<usize> {
    haystack
        .windows(needle.len())
        .enumerate()
        .filter(|(_, w)| *w == needle)
        .map(|(at, _)| at)
        .collect()
}

fn only<T>(mut hits: Vec<T>) -> Option<T> {
    (hits.len() == 1).then(|| hits.remove(0))
}

fn le32(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

/// Derives the shop tables from an executable image and the identity of its source.
pub fn derive(exe: &[u8], origin: AssetIdentity) -> Result<ShopTable, ShopTableError> {
    let image = Image::parse(exe)?;
    Ok(ShopTable {
        origin,
        image_sha256: crate::neutral_player::sha256(exe),
        item: slot_table(&image, "item", "SetItemTypeSlots", "AddItemToStock")?,
        unit: slot_table(&image, "unit", "SetUnitTypeSlots", "AddUnitToStock")?,
    })
}

/// Text offset of the non-stub implementation registered under `name`.
fn registered_impl(image: &Image<'_>, name: &'static str) -> Result<usize, ShopTableError> {
    let mut needle = vec![0];
    needle.extend_from_slice(name.as_bytes());
    needle.push(0);
    let at = only(occurrences(image.rdata.bytes, &needle)).ok_or(ShopTableError::Name(name))? + 1;
    let name_va = image
        .image_base
        .checked_add(image.rdata.rva)
        .and_then(|v| v.checked_add(u32::try_from(at).ok()?))
        .ok_or(ShopTableError::Name(name))?;
    let text_base = image
        .image_base
        .checked_add(image.text.rva)
        .ok_or(ShopTableError::Registration(name))?;
    let code = image.text.bytes;
    // `push name; push impl; call registrar`, with the name pushed first. The name is also
    // registered with `xor eax, eax; ret` stubs. Those return 0 for every call and are not the
    // native, so only the non-stub registration is the implementation.
    let implementations: Vec<usize> = code
        .windows(11)
        .filter(|w| {
            w[0] == PUSH_IMM32
                && w[1..5] == name_va.to_le_bytes()
                && w[5] == PUSH_IMM32
                && w[10] == CALL
        })
        .filter_map(|w| u32::from_le_bytes([w[6], w[7], w[8], w[9]]).checked_sub(text_base))
        .map(|o| o as usize)
        .filter(|&o| code.get(o..o + STUB.len()).is_some_and(|b| b != STUB))
        .collect();
    only(implementations).ok_or(ShopTableError::Registration(name))
}

fn slot_table(
    image: &Image<'_>,
    kind: &'static str,
    set_name: &'static str,
    add_name: &'static str,
) -> Result<SlotTable, ShopTableError> {
    let code = image.text.bytes;
    let set_impl = registered_impl(image, set_name)?;
    let helpers: Vec<usize> = image
        .callees(set_impl)
        .into_iter()
        .filter(|&c| clamp_at(&code[c..image.function_end(c)]).is_some())
        .collect();
    let helper = only(helpers).ok_or(ShopTableError::Helper(set_name))?;
    let body = &code[helper..image.function_end(helper)];
    let clamp = clamp_at(body).ok_or(ShopTableError::Helper(set_name))?;
    let slot_ceiling = le32(body, clamp + 1).ok_or(ShopTableError::Helper(set_name))?;
    let length = only(
        body.windows(5)
            .enumerate()
            .filter(|(_, w)| w[..2] == CMP_ECX && (w[3] == JAE_SHORT || w[3..5] == JAE_NEAR))
            .map(|(at, w)| (at, w[2]))
            .collect(),
    )
    .ok_or(ShopTableError::Length(set_name))?;
    let entries = u32::from(length.1);
    if slot_ceiling > entries {
        return Err(ShopTableError::Ceiling {
            kind,
            entries,
            ceiling: slot_ceiling,
        });
    }
    Ok(SlotTable {
        slot_ceiling,
        entries,
        default_slots: default_slots(image, add_name)?,
    })
}

/// Slot count a shop component holds after its initializer, read through the add native.
fn default_slots(image: &Image<'_>, add_name: &'static str) -> Result<u32, ShopTableError> {
    let code = image.text.bytes;
    let callees = image.callees(registered_impl(image, add_name)?);
    let class_code = only(
        callees
            .iter()
            .filter_map(|&c| class_stub(&code[c..]))
            .collect(),
    )
    .ok_or(ShopTableError::AddHandler(add_name))?;
    let count_fields: Vec<u32> = callees
        .iter()
        .flat_map(|&c| {
            code[c..image.function_end(c)]
                .windows(CMP_COUNT.len() + POINTER_LEN)
                .filter(|w| w[..CMP_COUNT.len()] == CMP_COUNT)
                .filter_map(|w| le32(w, CMP_COUNT.len()))
                .collect::<Vec<_>>()
        })
        .collect();
    let count_field = only(count_fields).ok_or(ShopTableError::AddHandler(add_name))?;

    // The component's vtable holds a second copy of the class stub.
    let text_base = image
        .image_base
        .checked_add(image.text.rva)
        .ok_or(ShopTableError::Vtable(add_name))?;
    let rdata = image.rdata.bytes;
    let text_end = text_base.saturating_add(u32::try_from(code.len()).unwrap_or(u32::MAX));
    let target = |slot: usize| -> Option<usize> {
        let va = le32(rdata, slot * POINTER_LEN)?;
        (text_base..text_end)
            .contains(&va)
            .then(|| (va - text_base) as usize)
    };
    let stubs: BTreeSet<usize> = (0..code.len())
        .filter(|&at| class_stub(&code[at..]) == Some(class_code))
        .collect();
    let slots: Vec<usize> = (0..rdata.len() / POINTER_LEN)
        .filter(|&slot| target(slot).is_some_and(|t| stubs.contains(&t)))
        .collect();
    let slot = only(slots).ok_or(ShopTableError::Vtable(add_name))?;
    let mut at = slot;
    while at > 0 && target(at - 1).is_some() {
        at -= 1;
    }
    let mut stores = BTreeSet::new();
    while let Some(function) = target(at) {
        let body = &code[function..image.function_end(function)];
        for (i, w) in body.windows(MOV_MEM_IMM_LEN).enumerate() {
            if w[0] == MOV_MEM_IMM
                && (MODRM_MOD10_FIRST..=MODRM_MOD10_LAST).contains(&w[1])
                && w[1] != MODRM_SIB
                && le32(w, 2) == Some(count_field)
            {
                stores.insert((function + i, le32(w, 6)));
            }
        }
        at += 1;
    }
    match only(stores.into_iter().collect()) {
        Some((_, Some(default))) => Ok(default),
        _ => Err(ShopTableError::Default(add_name)),
    }
}

/// The class code a `mov eax, imm32; ret` stub at the start of `code` returns.
fn class_stub(code: &[u8]) -> Option<u32> {
    if code.first() == Some(&MOV_EAX) && code.get(CLASS_STUB_LEN - 1) == Some(&RET) {
        le32(code, 1)
    } else {
        None
    }
}

/// Offset of the single clamp sequence in `body`, if exactly one exists.
fn clamp_at(body: &[u8]) -> Option<usize> {
    let hits: Vec<usize> = body
        .windows(CLAMP.len())
        .enumerate()
        .filter(|(_, w)| w[0] == CLAMP[0] && w[5..] == CLAMP[5..] && w[2..5] == [0, 0, 0])
        .map(|(at, _)| at)
        .collect();
    only(hits)
}

#[cfg(test)]
#[path = "shop_table_tests.rs"]
mod tests;

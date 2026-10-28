//! Per-shop item table capacity derived from a selected game executable.
//!
//! `SetItemTypeSlots` is registered under its name in `.rdata`, with the implementation
//! pushed right after the name and a call to the registrar. The implementation calls one
//! helper that stores the slot count. That helper clamps the count with an unsigned compare
//! against an immediate (`mov ecx, C; cmp [ebp+8], ecx; cmovb ecx, [ebp+8]`), so a negative
//! count becomes `C`. The same helper bounds the item cell index with `cmp ecx, L; jae`, where
//! `L` is the number of cells the shop object holds.
use crate::item_class_table::{Image, ItemClassError};
use crate::scripts::AssetIdentity;
use thiserror::Error;

const NAME: &[u8] = b"\0SetItemTypeSlots\0";
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

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ShopTableError {
    #[error("executable image is malformed: {0}")]
    Image(#[from] ItemClassError),
    #[error("SetItemTypeSlots name is not in .rdata exactly once")]
    Name,
    #[error("SetItemTypeSlots is not registered exactly once")]
    Registration,
    #[error("SetItemTypeSlots implementation has no unique clamp helper")]
    Helper,
    #[error("clamp helper has no unique cell-length check")]
    Length,
    #[error("cell count {entries} is below the slot ceiling {ceiling}")]
    Ceiling { entries: u32, ceiling: u32 },
}

/// Executable-derived capacity of one shop's item table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShopTable {
    /// Selected source the executable came from (edition, version, source path).
    pub origin: AssetIdentity,
    /// SHA-256 of the exact executable bytes the capacity was read from.
    pub image_sha256: [u8; 32],
    /// Largest slot count the engine stores. Larger and negative requests store this.
    pub slot_ceiling: u32,
    /// Item cells per shop. Removal scans all of them.
    pub entries: u32,
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

/// Derives the shop table capacity from an executable image and the identity of its source.
pub fn derive(exe: &[u8], origin: AssetIdentity) -> Result<ShopTable, ShopTableError> {
    let image = Image::parse(exe)?;
    let name = only(occurrences(image.rdata.bytes, NAME)).ok_or(ShopTableError::Name)? + 1;
    let name_va = image
        .image_base
        .checked_add(image.rdata.rva)
        .and_then(|v| v.checked_add(u32::try_from(name).ok()?))
        .ok_or(ShopTableError::Name)?;
    // `push name; push impl; call registrar`, with the name pushed first.
    let mut registrations = Vec::new();
    for w in image.text.bytes.windows(11) {
        if w[0] == PUSH_IMM32
            && w[1..5] == name_va.to_le_bytes()
            && w[5] == PUSH_IMM32
            && w[10] == CALL
        {
            registrations.push(u32::from_le_bytes([w[6], w[7], w[8], w[9]]));
        }
    }
    let text_base = image
        .image_base
        .checked_add(image.text.rva)
        .ok_or(ShopTableError::Registration)?;
    let code = image.text.bytes;
    // The name is also registered with `xor eax, eax; ret` stubs. Those return 0 for every
    // call and are not the native, so only the non-stub registration is the implementation.
    let implementations: Vec<usize> = registrations
        .into_iter()
        .filter_map(|va| va.checked_sub(text_base).map(|o| o as usize))
        .filter(|&o| code.get(o..o + STUB.len()).is_some_and(|b| b != STUB))
        .collect();
    let impl_at = only(implementations).ok_or(ShopTableError::Registration)?;
    let helpers: Vec<usize> = image
        .callees(impl_at)
        .into_iter()
        .filter(|&c| clamp_at(&code[c..image.function_end(c)]).is_some())
        .collect();
    let helper = only(helpers).ok_or(ShopTableError::Helper)?;
    let body = &code[helper..image.function_end(helper)];
    let clamp = clamp_at(body).ok_or(ShopTableError::Helper)?;
    let slot_ceiling = u32::from_le_bytes([
        body[clamp + 1],
        body[clamp + 2],
        body[clamp + 3],
        body[clamp + 4],
    ]);
    let length = only(
        body.windows(5)
            .enumerate()
            .filter(|(_, w)| w[..2] == CMP_ECX && (w[3] == JAE_SHORT || w[3..5] == JAE_NEAR))
            .map(|(at, w)| (at, w[2]))
            .collect(),
    )
    .ok_or(ShopTableError::Length)?;
    let entries = u32::from(length.1);
    if slot_ceiling > entries {
        return Err(ShopTableError::Ceiling {
            entries,
            ceiling: slot_ceiling,
        });
    }
    Ok(ShopTable {
        origin,
        image_sha256: crate::neutral_player::sha256(exe),
        slot_ceiling,
        entries,
    })
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

//! Item class ordinals derived from a selected game executable. The item data loader reads
//! each row's `class` text into a compare chain: one case-insensitive equality per class
//! name, each returning its ordinal. The chain is found by a deterministic call-graph walk
//! from the `units\ItemData` path. Names and ordinals come from its code, not from a list
//! in this crate. The table keeps the image identity it was derived from.
use crate::scripts::AssetIdentity;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// The item data path the loader opens. A `.rdata` occurrence anchors the chain.
const ITEM_DATA_PATH: &[u8] = b"units\\ItemData";
const PROLOGUE: [u8; 3] = [0x55, 0x8b, 0xec];
const PUSH_IMM32: u8 = 0x68;
const CALL: u8 = 0xe8;
/// `push name; push esi; call compare; add esp, 8`.
const PUSH_ESI_CALL: [u8; 2] = [0x56, 0xe8];
const ADD_ESP_8: [u8; 3] = [0x83, 0xc4, 0x08];
const ENTRY_LEN: usize = 14;
const ORDINAL_WINDOW: usize = 12;
const MAX_NAME_LEN: usize = 64;
const NULL_WINDOW: usize = 0x20;
const MAX_SECTIONS: usize = 96;
const PE_OFFSET_AT: usize = 0x3c;
const COFF_LEN: usize = 20;
const PE32_MAGIC: u16 = 0x10b;
const SECTION_LEN: usize = 40;
const TEXT_NAME: &[u8; 8] = b".text\0\0\0";
const RDATA_NAME: &[u8; 8] = b".rdata\0\0";

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ItemClassError {
    #[error("executable image is malformed: {0}")]
    Image(&'static str),
    #[error("item data path is not referenced from .rdata exactly once")]
    Anchor,
    #[error("item data path reference is ambiguous")]
    AmbiguousAnchor,
    #[error("no unique class compare chain under the item data loader")]
    Chain,
    #[error("compare entry at text offset {0:#x} has no ordinal form")]
    OrdinalForm(usize),
    #[error("class default is not derivable from the parser entry")]
    Default,
}

/// Class text to ordinal, as the selected executable compares it, with the identity of
/// the image it was derived from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemClassTable {
    /// Selected source the executable came from (edition, version, source path).
    pub origin: AssetIdentity,
    /// SHA-256 of the exact executable bytes the table was read from.
    pub image_sha256: [u8; 32],
    /// Names as stored in the executable (lowercase ASCII).
    pub ordinals: BTreeMap<String, i32>,
    /// Ordinal for null, absent or unmatched class text.
    pub default: i32,
}

struct Section<'a> {
    rva: u32,
    bytes: &'a [u8],
}

struct Image<'a> {
    image_base: u32,
    text: Section<'a>,
    rdata: Section<'a>,
}

fn bytes_at(data: &[u8], at: usize, len: usize) -> Option<&[u8]> {
    data.get(at..at.checked_add(len)?)
}

fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes_at(data, at, 2)?.try_into().ok()?))
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes_at(data, at, 4)?.try_into().ok()?))
}

fn i32_at(data: &[u8], at: usize) -> Option<i32> {
    Some(i32::from_le_bytes(bytes_at(data, at, 4)?.try_into().ok()?))
}

impl<'a> Image<'a> {
    fn parse(data: &'a [u8]) -> Result<Self, ItemClassError> {
        let bad = ItemClassError::Image;
        let pe = u32_at(data, PE_OFFSET_AT).ok_or(bad("no PE header offset"))? as usize;
        if bytes_at(data, pe, 4) != Some(b"PE\0\0") {
            return Err(bad("missing PE signature"));
        }
        let coff = pe + 4;
        let count = usize::from(u16_at(data, coff + 2).ok_or(bad("no section count"))?);
        let opt_len = usize::from(u16_at(data, coff + 16).ok_or(bad("no optional header size"))?);
        let opt = coff + COFF_LEN;
        if u16_at(data, opt) != Some(PE32_MAGIC) {
            return Err(bad("not a PE32 image"));
        }
        let image_base = u32_at(data, opt + 28).ok_or(bad("no image base"))?;
        if count > MAX_SECTIONS {
            return Err(bad("too many sections"));
        }
        let table = opt.checked_add(opt_len).ok_or(bad("optional header overflows"))?;
        let mut text = None;
        let mut rdata = None;
        for i in 0..count {
            let at = table
                .checked_add(SECTION_LEN * i)
                .ok_or(bad("section table overflows"))?;
            let header = bytes_at(data, at, SECTION_LEN).ok_or(bad("truncated section table"))?;
            let virtual_size = u32_at(header, 8).ok_or(bad("section size"))?;
            let rva = u32_at(header, 12).ok_or(bad("section address"))?;
            let raw_size = u32_at(header, 16).ok_or(bad("section data size"))?;
            let raw_at = u32_at(header, 20).ok_or(bad("section data offset"))? as usize;
            let mapped = virtual_size.max(raw_size);
            image_base
                .checked_add(rva)
                .and_then(|v| v.checked_add(mapped))
                .ok_or(bad("section address overflows"))?;
            let bytes = bytes_at(data, raw_at, raw_size as usize)
                .ok_or(bad("section data outside image"))?;
            let section = Section { rva, bytes };
            let slot = match &header[..8] {
                name if name == TEXT_NAME => &mut text,
                name if name == RDATA_NAME => &mut rdata,
                _ => continue,
            };
            if slot.replace(section).is_some() {
                return Err(bad("duplicate section"));
            }
        }
        Ok(Self {
            image_base,
            text: text.ok_or(bad("no .text section"))?,
            rdata: rdata.ok_or(bad("no .rdata section"))?,
        })
    }

    /// NUL-terminated lowercase ASCII name at `va` inside `.rdata` only.
    fn name_at(&self, va: u32) -> Option<&'a [u8]> {
        let rel = va
            .checked_sub(self.image_base.checked_add(self.rdata.rva)?)?
            as usize;
        let tail = self.rdata.bytes.get(rel..)?;
        let len = tail.iter().take(MAX_NAME_LEN + 1).position(|&b| b == 0)?;
        let name = &tail[..len];
        (!name.is_empty() && name.iter().all(u8::is_ascii_lowercase)).then_some(name)
    }

    /// Start of the function containing `offset`: the last prologue at or before it.
    fn function_start(&self, offset: usize) -> Option<usize> {
        self.text.bytes.get(..=offset)?.windows(3).rposition(|w| w == PROLOGUE)
    }

    /// Offset of the next prologue after `start`, or the end of `.text`.
    fn function_end(&self, start: usize) -> usize {
        let code = self.text.bytes;
        code.get(start + 3..)
            .and_then(|rest| rest.windows(3).position(|w| w == PROLOGUE))
            .map_or(code.len(), |p| start + 3 + p)
    }

    /// Sorted, distinct `.text` offsets of direct calls inside the function at `start`.
    fn callees(&self, start: usize) -> Vec<usize> {
        let code = self.text.bytes;
        let end = self.function_end(start);
        let mut out = BTreeSet::new();
        for j in start..end {
            if code.get(j) != Some(&CALL) {
                continue;
            }
            let Some(rel) = i32_at(code, j + 1) else { continue };
            let target = j as i64 + 5 + i64::from(rel);
            if let Some(t) = usize::try_from(target).ok().filter(|&t| t < code.len()) {
                out.insert(t);
            }
        }
        out.into_iter().collect()
    }
}

/// One compare entry: `push name; push esi; call compare; add esp, 8` at `offset`.
struct Entry {
    offset: usize,
    name: String,
    target: usize,
}

fn chain_entries(image: &Image<'_>, start: usize) -> Vec<Entry> {
    let code = image.text.bytes;
    let end = image.function_end(start);
    let mut entries = Vec::new();
    for j in start..end {
        if j + ENTRY_LEN > end
            || code[j] != PUSH_IMM32
            || code.get(j + 5..j + 7) != Some(&PUSH_ESI_CALL[..])
            || code.get(j + 11..j + 14) != Some(&ADD_ESP_8[..])
        {
            continue;
        }
        let Some(va) = u32_at(code, j + 1) else { continue };
        let Some(name) = image.name_at(va) else { continue };
        let Some(rel) = i32_at(code, j + 7) else { continue };
        let Some(target) = usize::try_from(j as i64 + 11 + i64::from(rel)).ok() else {
            continue;
        };
        entries.push(Entry {
            offset: j,
            name: String::from_utf8_lossy(name).into_owned(),
            target,
        });
    }
    entries
}

/// The ordinal the compare entry returns on equality, read from the code after its call.
fn entry_ordinal(code: &[u8], entry: usize) -> Option<i32> {
    let after = code.get(entry + ENTRY_LEN..entry + ENTRY_LEN + ORDINAL_WINDOW)?;
    // `test eax, eax; jne skip; mov eax, n; pop esi; pop ebp; ret`. The equal path falls
    // through to `pop esi; pop ebp; ret` with eax zero, which is ordinal zero.
    if after[..2] == [0x85, 0xc0] && after[2] == 0x75 {
        let fallthrough = &after[4..];
        if fallthrough[0] == 0xb8 && fallthrough[5..8] == [0x5e, 0x5d, 0xc3] {
            return i32::try_from(u32_at(fallthrough, 1)?).ok();
        }
        if fallthrough[..3] == [0x5e, 0x5d, 0xc3] {
            return Some(0);
        }
    }
    // `neg eax; sbb eax, eax; neg eax; add eax, n`: zero on equality, then n.
    if after[..8] == [0xf7, 0xd8, 0x1b, 0xc0, 0xf7, 0xd8, 0x83, 0xc0] {
        return Some(i32::from(after[8]));
    }
    None
}

/// Null branch of the parser: `test esi, esi; jne skip; lea eax, [esi + n]`.
fn null_default(code: &[u8], start: usize) -> Option<i32> {
    code.get(start..start + NULL_WINDOW)?
        .windows(7)
        .find(|w| w[..2] == [0x85, 0xf6] && w[2] == 0x75 && w[4..6] == [0x8d, 0x46])
        .map(|w| i32::from(w[6]))
}

/// Derives the class table from an executable image and the identity of its source.
pub fn derive(exe: &[u8], origin: AssetIdentity) -> Result<ItemClassTable, ItemClassError> {
    let image = Image::parse(exe)?;
    let needle = [&[0u8][..], ITEM_DATA_PATH, &[0u8][..]].concat();
    let mut anchors = image
        .rdata
        .bytes
        .windows(needle.len())
        .enumerate()
        .filter(|(_, w)| *w == needle.as_slice());
    let anchor = anchors.next().ok_or(ItemClassError::Anchor)?.0;
    if anchors.next().is_some() {
        return Err(ItemClassError::AmbiguousAnchor);
    }
    let path_va = image
        .image_base
        .checked_add(image.rdata.rva)
        .and_then(|v| v.checked_add(u32::try_from(anchor + 1).ok()?))
        .ok_or(ItemClassError::Image("item data path address overflows"))?;
    let mut references = image
        .text
        .bytes
        .windows(5)
        .enumerate()
        .filter(|(_, w)| w[0] == PUSH_IMM32 && w[1..] == path_va.to_le_bytes());
    let reference = references.next().ok_or(ItemClassError::Anchor)?.0;
    if references.next().is_some() {
        return Err(ItemClassError::AmbiguousAnchor);
    }
    let loader = image.function_start(reference).ok_or(ItemClassError::Chain)?;
    // The chain is in a function the loader calls, directly or through one row builder.
    let mut candidates = BTreeSet::new();
    for callee in image.callees(loader) {
        candidates.insert(callee);
        candidates.extend(image.callees(callee));
    }
    let mut chains = candidates
        .into_iter()
        .map(|start| (start, chain_entries(&image, start)))
        .filter(|(_, entries)| entries.len() >= 2);
    let (start, entries) = chains.next().ok_or(ItemClassError::Chain)?;
    if chains.next().is_some() {
        return Err(ItemClassError::Chain);
    }
    let code = image.text.bytes;
    let compare = entries[0].target;
    if entries.iter().any(|e| e.target != compare) {
        return Err(ItemClassError::Chain);
    }
    let mut ordinals = BTreeMap::new();
    for entry in &entries {
        let ordinal = entry_ordinal(code, entry.offset)
            .ok_or(ItemClassError::OrdinalForm(entry.offset))?;
        if ordinals.insert(entry.name.clone(), ordinal).is_some() {
            return Err(ItemClassError::Chain);
        }
    }
    let default = null_default(code, start).ok_or(ItemClassError::Default)?;
    Ok(ItemClassTable {
        origin,
        image_sha256: Sha256::digest(exe).into(),
        ordinals,
        default,
    })
}

#[cfg(test)]
#[path = "item_class_table_tests.rs"]
mod tests;

//! Native fallback ordinal of `GetDefaultDifficulty`, read from the selected client image.
//!
//! The name and signature strings are searched only in `.rdata` raw data, and the
//! registration only in `.text` raw data. Every registration names the same signature and
//! handler shape. A handler is either the fallback shape, whose immediate is the ordinal,
//! or the known constant-zero stub (`xor eax, eax; ret`). Exactly one fallback is required.
use thiserror::Error;

const NAME: &[u8] = b"GetDefaultDifficulty\0";
const SIGNATURE: &[u8] = b"()Hgamedifficulty;\0";
const RDATA: &[u8; 8] = b".rdata\0\0";
const TEXT: &[u8; 8] = b".text\0\0\0";
const PUSH_IMM32: u8 = 0x68;
const CALL_REL32: u8 = 0xE8;
const TEST_EAX_EAX: [u8; 2] = [0x85, 0xC0];
const JNE_REL8: u8 = 0x75;
/// Bytes between the `jne` and its target: the fallback `mov` and `ret`.
const JNE_SKIPS_FALLBACK: u8 = 6;
const MOV_EAX_IMM32: u8 = 0xB8;
const RET: u8 = 0xC3;
const STUB: [u8; 3] = [0x33, 0xC0, RET];
/// Registrar call site, primary handler, and stub handler each span this many bytes.
const HANDLER_LEN: usize = 15;
const PE_SIGNATURE: &[u8; 4] = b"PE\0\0";
const PE32_MAGIC: u16 = 0x10B;
const PE_HEADER_OFFSET: usize = 0x3C;
const COFF_HEADER_LEN: usize = 20;
const OPTIONAL_HEADER_MIN_LEN: usize = 96;
const SECTION_HEADER_LEN: usize = 40;
const MAX_SECTIONS: usize = 96;
const SCN_MEM_EXECUTE: u32 = 0x2000_0000;
const SCN_MEM_READ: u32 = 0x4000_0000;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FallbackError {
    #[error("selected image is not a 32-bit PE image")]
    NotPe,
    #[error("selected image section table is invalid: {0}")]
    Sections(&'static str),
    #[error("GetDefaultDifficulty name is not present exactly once in .rdata")]
    Name,
    #[error("GetDefaultDifficulty registrations do not name exactly one fallback handler")]
    Registration,
}

#[derive(Clone, Copy)]
struct Section {
    name: [u8; 8],
    characteristics: u32,
    rva: u32,
    virtual_size: u32,
    raw_size: u32,
    raw_pointer: u32,
}

impl Section {
    fn span(&self) -> u32 {
        self.virtual_size.max(self.raw_size)
    }
}

struct Pe<'a> {
    image: &'a [u8],
    base: u32,
    sections: Vec<Section>,
}

impl<'a> Pe<'a> {
    fn parse(image: &'a [u8]) -> Result<Self, FallbackError> {
        let pe = le32(image, PE_HEADER_OFFSET).ok_or(FallbackError::NotPe)? as usize;
        if slice(image, pe, 4) != Some(PE_SIGNATURE) {
            return Err(FallbackError::NotPe);
        }
        let coff = pe + 4;
        let count = usize::from(le16(image, coff + 2).ok_or(FallbackError::Sections("count"))?);
        if count == 0 || count > MAX_SECTIONS {
            return Err(FallbackError::Sections("section count"));
        }
        let optional_len =
            usize::from(le16(image, coff + 16).ok_or(FallbackError::Sections("optional"))?);
        if optional_len < OPTIONAL_HEADER_MIN_LEN {
            return Err(FallbackError::Sections("optional header length"));
        }
        let optional = coff + COFF_HEADER_LEN;
        if le16(image, optional) != Some(PE32_MAGIC) {
            return Err(FallbackError::NotPe);
        }
        let base = le32(image, optional + 28).ok_or(FallbackError::NotPe)?;
        let table = optional
            .checked_add(optional_len)
            .ok_or(FallbackError::Sections("optional header overflows"))?;
        let table_len = count * SECTION_HEADER_LEN;
        let headers = slice(image, table, table_len)
            .ok_or(FallbackError::Sections("section table outside image"))?;
        let mut sections = Vec::new();
        sections
            .try_reserve_exact(count)
            .map_err(|_| FallbackError::Sections("allocation"))?;
        for header in headers.chunks_exact(SECTION_HEADER_LEN) {
            let virtual_size = le32(header, 8).ok_or(FallbackError::Sections("size"))?;
            let rva = le32(header, 12).ok_or(FallbackError::Sections("address"))?;
            let raw_size = le32(header, 16).ok_or(FallbackError::Sections("raw size"))?;
            let raw_pointer = le32(header, 20).ok_or(FallbackError::Sections("raw pointer"))?;
            let section = Section {
                name: header[..8]
                    .try_into()
                    .map_err(|_| FallbackError::Sections("name"))?,
                characteristics: le32(header, 36).ok_or(FallbackError::Sections("flags"))?,
                rva,
                virtual_size,
                raw_size,
                raw_pointer,
            };
            if raw_size > 0 {
                let end = raw_pointer
                    .checked_add(raw_size)
                    .ok_or(FallbackError::Sections("raw range overflows"))?;
                if end as usize > image.len() {
                    return Err(FallbackError::Sections("raw data outside image"));
                }
            }
            base.checked_add(rva)
                .and_then(|va| va.checked_add(section.span()))
                .ok_or(FallbackError::Sections("address range overflows"))?;
            sections.push(section);
        }
        no_overlap(
            sections
                .iter()
                .filter(|s| s.raw_size > 0)
                .map(|s| (s.raw_pointer, s.raw_size)),
        )?;
        no_overlap(
            sections
                .iter()
                .filter(|s| s.span() > 0)
                .map(|s| (s.rva, s.span())),
        )?;
        Ok(Self {
            image,
            base,
            sections,
        })
    }

    /// The single section named `name`, with `flag` set and `forbidden` clear.
    fn section(&self, name: &[u8; 8], flag: u32, forbidden: u32) -> Result<Section, FallbackError> {
        let mut matches = self.sections.iter().filter(|s| &s.name == name);
        let section = matches
            .next()
            .copied()
            .ok_or(FallbackError::Sections("required section missing"))?;
        if matches.next().is_some() {
            return Err(FallbackError::Sections("required section repeated"));
        }
        if section.characteristics & flag == 0 || section.characteristics & forbidden != 0 {
            return Err(FallbackError::Sections("required section flags"));
        }
        Ok(section)
    }

    /// Raw data of a validated section; empty when the section has no raw data.
    fn raw(&self, section: &Section) -> &'a [u8] {
        slice(
            self.image,
            section.raw_pointer as usize,
            section.raw_size as usize,
        )
        .unwrap_or(&[])
    }

    fn va_of_raw(&self, section: &Section, offset: usize) -> Option<u32> {
        self.base
            .checked_add(section.rva)?
            .checked_add(u32::try_from(offset).ok()?)
    }

    /// Bytes at `va` inside the raw data of `section`, if the whole range lies within it.
    fn bytes_at(&self, section: &Section, va: u32, len: usize) -> Option<&'a [u8]> {
        let within = va.checked_sub(self.base.checked_add(section.rva)?)? as usize;
        let raw = self.raw(section);
        slice(raw, within, len)
    }
}

/// Byte ranges `(start, length)` must not overlap. Ranges are checked after sorting.
fn no_overlap(ranges: impl Iterator<Item = (u32, u32)>) -> Result<(), FallbackError> {
    let mut sorted: Vec<(u64, u64)> = ranges.map(|(s, l)| (u64::from(s), u64::from(l))).collect();
    sorted.sort_unstable();
    for pair in sorted.windows(2) {
        if pair[0].0 + pair[0].1 > pair[1].0 {
            return Err(FallbackError::Sections("overlapping sections"));
        }
    }
    Ok(())
}

/// The ordinal the native returns when its singleton lookup fails.
pub fn native_default_fallback(image: &[u8]) -> Result<i32, FallbackError> {
    let pe = Pe::parse(image)?;
    let rdata = pe.section(RDATA, SCN_MEM_READ, SCN_MEM_EXECUTE)?;
    let text = pe.section(TEXT, SCN_MEM_EXECUTE, 0)?;
    let name_at = unique_offset(pe.raw(&rdata), NAME).ok_or(FallbackError::Name)?;
    let name_va = pe.va_of_raw(&rdata, name_at).ok_or(FallbackError::Name)?;
    let name_push = [&[PUSH_IMM32][..], &name_va.to_le_bytes()].concat();
    let code = pe.raw(&text);
    let mut fallback = None;
    let mut stubs = 0usize;
    for at in find_all(code, &name_push) {
        // A name push that is not a complete registration refuses the whole image.
        if at < 5 || at + 11 > code.len() {
            return Err(FallbackError::Registration);
        }
        let sig_va = le32(code, at - 4).ok_or(FallbackError::Registration)?;
        let handler_va = le32(code, at + 6).ok_or(FallbackError::Registration)?;
        let shaped =
            code[at - 5] == PUSH_IMM32 && code[at + 5] == PUSH_IMM32 && code[at + 10] == CALL_REL32;
        let signature_ok = pe.bytes_at(&rdata, sig_va, SIGNATURE.len()) == Some(SIGNATURE);
        if !shaped || !signature_ok {
            return Err(FallbackError::Registration);
        }
        let handler = pe
            .bytes_at(&text, handler_va, HANDLER_LEN)
            .ok_or(FallbackError::Registration)?;
        if handler[..STUB.len()] == STUB {
            stubs += 1;
            continue;
        }
        let imm = fallback_immediate(handler).ok_or(FallbackError::Registration)?;
        if fallback.replace(imm).is_some() {
            return Err(FallbackError::Registration);
        }
    }
    if stubs > 1 {
        return Err(FallbackError::Registration);
    }
    fallback.ok_or(FallbackError::Registration)
}

/// `call; test eax, eax; jne +6; mov eax, imm32; ret`, the shape of the primary handler.
fn fallback_immediate(handler: &[u8]) -> Option<i32> {
    let shape_ok = handler.first() == Some(&CALL_REL32)
        && handler.get(5..7) == Some(&TEST_EAX_EAX[..])
        && handler.get(7) == Some(&JNE_REL8)
        && handler.get(8) == Some(&JNE_SKIPS_FALLBACK)
        && handler.get(9) == Some(&MOV_EAX_IMM32)
        && handler.get(14) == Some(&RET);
    if !shape_ok {
        return None;
    }
    Some(i32::from_le_bytes(handler.get(10..14)?.try_into().ok()?))
}

fn unique_offset(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    let mut found = find_all(haystack, needle);
    let first = found.next()?;
    found.next().is_none().then_some(first)
}

fn find_all<'a>(haystack: &'a [u8], needle: &'a [u8]) -> impl Iterator<Item = usize> + 'a {
    haystack
        .windows(needle.len())
        .enumerate()
        .filter_map(move |(at, window)| (window == needle).then_some(at))
}

fn slice(data: &[u8], at: usize, len: usize) -> Option<&[u8]> {
    data.get(at..at.checked_add(len)?)
}

fn le16(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(slice(data, at, 2)?.try_into().ok()?))
}

fn le32(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(slice(data, at, 4)?.try_into().ok()?))
}

#[cfg(test)]
#[path = "default_difficulty_image_tests.rs"]
mod tests;

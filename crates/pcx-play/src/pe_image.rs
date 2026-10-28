//! Read-only PE32 section map of a selected executable: section bytes by name and
//! bounded reads at virtual addresses. Nothing is executed or relocated.

pub(crate) const PE_OFFSET_AT: usize = 0x3c;
const COFF_LEN: usize = 20;
pub(crate) const PE32_MAGIC: u16 = 0x10b;
pub(crate) const SECTION_LEN: usize = 40;
const MAX_SECTIONS: usize = 96;

/// Section name, NUL-padded to eight bytes as stored in the section header.
pub type SectionName = [u8; 8];

pub const TEXT: &SectionName = b".text\0\0\0";
pub const RDATA: &SectionName = b".rdata\0\0";

pub struct Section<'a> {
    pub rva: u32,
    pub bytes: &'a [u8],
}

pub struct PeSections<'a> {
    pub image_base: u32,
    sections: Vec<(SectionName, Section<'a>)>,
}

pub fn bytes_at(data: &[u8], at: usize, len: usize) -> Option<&[u8]> {
    data.get(at..at.checked_add(len)?)
}

/// Bytes tested per step by [`find_byte`]. Wide enough for the compiler to compare a block
/// with a few vector operations instead of a branch per byte.
const SCAN_BLOCK: usize = 32;

fn find_byte(haystack: &[u8], byte: u8) -> Option<usize> {
    let (blocks, remainder) = haystack.as_chunks::<SCAN_BLOCK>();
    let mut start = 0;
    for block in blocks {
        if block.iter().fold(false, |found, &b| found | (b == byte)) {
            return block.iter().position(|&b| b == byte).map(|at| start + at);
        }
        start += SCAN_BLOCK;
    }
    remainder
        .iter()
        .position(|&b| b == byte)
        .map(|at| start + at)
}

/// Offsets and contents of the `len`-byte windows of `haystack` whose byte at `index` is `byte`,
/// ascending. The same items as `haystack.windows(len).enumerate()` filtered on that byte, found
/// without testing every offset. `index` must be less than `len`.
pub(crate) fn windows_with_byte(
    haystack: &[u8],
    index: usize,
    byte: u8,
    len: usize,
) -> impl Iterator<Item = (usize, &[u8])> {
    debug_assert!(index < len);
    let mut from = 0;
    std::iter::from_fn(move || {
        loop {
            let start = from + find_byte(haystack.get(index + from..)?, byte)?;
            from = start + 1;
            if let Some(window) = haystack.get(start..start.checked_add(len)?) {
                return Some((start, window));
            }
        }
    })
}

/// Offsets where `needle` occurs in `haystack`, ascending, overlapping occurrences included.
/// The same offsets as `haystack.windows(needle.len())` filtered by equality, without a
/// comparison call per offset. An empty needle has no occurrences.
pub(crate) fn occurrences<'a>(
    haystack: &'a [u8],
    needle: &'a [u8],
) -> impl Iterator<Item = usize> + 'a {
    let first = needle.first().copied();
    // An empty needle never reaches the search; the window length only needs to be positive.
    windows_with_byte(haystack, 0, first.unwrap_or(0), needle.len().max(1))
        .filter(move |(_, window)| first.is_some() && *window == needle)
        .map(|(at, _)| at)
}

pub fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes_at(data, at, 2)?.try_into().ok()?))
}

pub fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes_at(data, at, 4)?.try_into().ok()?))
}

pub fn i32_at(data: &[u8], at: usize) -> Option<i32> {
    Some(i32::from_le_bytes(bytes_at(data, at, 4)?.try_into().ok()?))
}

impl<'a> PeSections<'a> {
    /// Sections whose raw bytes lie inside `data`. Duplicate names are not resolved.
    pub fn parse(data: &'a [u8]) -> Result<Self, &'static str> {
        let pe = u32_at(data, PE_OFFSET_AT).ok_or("no PE header offset")? as usize;
        if bytes_at(data, pe, 4) != Some(b"PE\0\0") {
            return Err("missing PE signature");
        }
        let coff = pe + 4;
        let count = usize::from(u16_at(data, coff + 2).ok_or("no section count")?);
        let opt_len = usize::from(u16_at(data, coff + 16).ok_or("no optional header size")?);
        let opt = coff + COFF_LEN;
        if u16_at(data, opt) != Some(PE32_MAGIC) {
            return Err("not a PE32 image");
        }
        let image_base = u32_at(data, opt + 28).ok_or("no image base")?;
        if count > MAX_SECTIONS {
            return Err("too many sections");
        }
        let table = opt
            .checked_add(opt_len)
            .ok_or("optional header overflows")?;
        let mut sections = Vec::with_capacity(count);
        for i in 0..count {
            let at = table
                .checked_add(SECTION_LEN * i)
                .ok_or("section table overflows")?;
            let header = bytes_at(data, at, SECTION_LEN).ok_or("truncated section table")?;
            let virtual_size = u32_at(header, 8).ok_or("section size")?;
            let rva = u32_at(header, 12).ok_or("section address")?;
            let raw_size = u32_at(header, 16).ok_or("section data size")?;
            let raw_at = u32_at(header, 20).ok_or("section data offset")? as usize;
            image_base
                .checked_add(rva)
                .and_then(|v| v.checked_add(virtual_size.max(raw_size)))
                .ok_or("section address overflows")?;
            let bytes =
                bytes_at(data, raw_at, raw_size as usize).ok_or("section data outside image")?;
            let name: SectionName = header[..8].try_into().map_err(|_| "section name")?;
            sections.push((name, Section { rva, bytes }));
        }
        Ok(Self {
            image_base,
            sections,
        })
    }

    /// The only section named `name`: `Ok(None)` when absent, `Err` when repeated. The
    /// repeat check runs here, not in `parse`, so unrelated duplicate names stay accepted.
    pub fn section(&self, name: &SectionName) -> Result<Option<&Section<'a>>, &'static str> {
        let mut matches = self
            .sections
            .iter()
            .filter(|(n, _)| n == name)
            .map(|(_, s)| s);
        let first = matches.next();
        if matches.next().is_some() {
            return Err("duplicate section");
        }
        Ok(first)
    }

    /// Virtual addresses of every occurrence of `needle` inside a section's raw data.
    pub fn find_va(&self, needle: &[u8]) -> Vec<u32> {
        let mut found = Vec::new();
        for (_, section) in &self.sections {
            let Some(base) = self.image_base.checked_add(section.rva) else {
                continue;
            };
            found.extend(
                occurrences(section.bytes, needle).filter_map(|at| base.checked_add(at as u32)),
            );
        }
        found
    }

    /// Bytes at a virtual address inside one section's raw data, or `None` when the
    /// range is absent or spans outside that data.
    pub fn va_bytes(&self, va: u32, len: usize) -> Option<&'a [u8]> {
        self.sections.iter().find_map(|(_, section)| {
            let start = va.checked_sub(self.image_base.checked_add(section.rva)?)? as usize;
            bytes_at(section.bytes, start, len)
        })
    }
}

#[cfg(test)]
#[path = "pe_image_tests.rs"]
pub(crate) mod tests;

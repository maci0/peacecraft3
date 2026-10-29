//! Verified local profile + complete PE input. No I/O or measured target operands.
use super::neutral_player::{
    NeutralError, RuleInputBudget, RuleProblem, Sha256Digest, VerifiedNeutralRule, sha256,
};
use crate::pe_image::windows_with_byte;
use crate::scripts::AssetIdentity;
/// First byte `decode_header` requires of its window (`cmp eax, imm8`).
pub(super) const HEADER_FIRST_BYTE: u8 = 0x83;
fn bad(p: RuleProblem) -> NeutralError {
    NeutralError::Rule(p)
}
fn bytes(d: &[u8], at: usize, n: usize) -> Result<&[u8], NeutralError> {
    d.get(at..at.checked_add(n).ok_or_else(|| bad(RuleProblem::Range))?)
        .ok_or_else(|| bad(RuleProblem::Range))
}

#[cfg(test)]
mod section_storage_tests {
    use super::*;

    fn image(count: u16) -> Vec<u8> {
        let mut data = vec![0; 0x1000 + usize::from(count) * 16];
        data[..2].copy_from_slice(b"MZ");
        data[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        data[0x80..0x84].copy_from_slice(b"PE\0\0");
        data[0x84..0x86].copy_from_slice(&0x14cu16.to_le_bytes());
        data[0x86..0x88].copy_from_slice(&count.to_le_bytes());
        data[0x94..0x96].copy_from_slice(&0x60u16.to_le_bytes());
        data[0x98..0x9a].copy_from_slice(&0x10bu16.to_le_bytes());
        data[0xb4..0xb8].copy_from_slice(&0x300000u32.to_le_bytes());
        data[0xd0..0xd4].copy_from_slice(&0x40000u32.to_le_bytes());
        for i in 0..usize::from(count) {
            let header = 0xf8 + i * 40;
            let rva = u32::from(count) - i as u32;
            let raw = 0x1000 + i * 16;
            data[header + 8..header + 12].copy_from_slice(&16u32.to_le_bytes());
            data[header + 12..header + 16].copy_from_slice(&(rva * 0x1000).to_le_bytes());
            data[header + 16..header + 20].copy_from_slice(&16u32.to_le_bytes());
            data[header + 20..header + 24].copy_from_slice(&(raw as u32).to_le_bytes());
            data[header + 36..header + 40].copy_from_slice(&0x60000060u32.to_le_bytes());
            data[raw] = i as u8;
        }
        data
    }

    #[test]
    fn section_storage_preserves_order_and_validation_above_inline_capacity() {
        let budget = RuleInputBudget {
            image_bytes: 1 << 20,
            profile_bytes: 1 << 10,
            sections: 32,
        };
        for count in [16, 17] {
            let data = image(count);
            let pe = Pe::parse(&data, budget).unwrap();
            assert_eq!(pe.code_ranges().count(), usize::from(count));
            assert_eq!(pe.read_from(0x301000, true).unwrap()[0], (count - 1) as u8);
            let mut overlap = data;
            let first = overlap[0xf8 + 12..0xf8 + 16].to_vec();
            overlap[0xf8 + 40 + 12..0xf8 + 40 + 16].copy_from_slice(&first);
            assert!(Pe::parse(&overlap, budget).is_err());
        }
    }

    #[test]
    fn zero_filled_range_must_fit_in_a_readable_virtual_tail() {
        let mut data = image(1);
        data[0xf8 + 8..0xf8 + 12].copy_from_slice(&32_u32.to_le_bytes());
        let budget = RuleInputBudget {
            image_bytes: 1 << 20,
            profile_bytes: 1 << 10,
            sections: 32,
        };
        let pe = Pe::parse(&data, budget).unwrap();
        assert!(pe.zero_filled(0x301010, 16));
        assert!(!pe.zero_filled(0x30100f, 2));
        assert!(!pe.zero_filled(0x30101f, 2));
        assert!(!pe.zero_filled(0x301010, 0));
        assert!(!pe.zero_filled(u32::MAX, 2));
    }
}
fn word(d: &[u8], at: usize) -> Result<u16, NeutralError> {
    Ok(u16::from_le_bytes(bytes(d, at, 2)?.try_into().unwrap()))
}
fn dword(d: &[u8], at: usize) -> Result<u32, NeutralError> {
    Ok(u32::from_le_bytes(bytes(d, at, 4)?.try_into().unwrap()))
}
struct Reader<'a> {
    d: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], NeutralError> {
        let b = bytes(self.d, self.at, n)?;
        self.at += n;
        Ok(b)
    }
    fn u32(&mut self) -> Result<u32, NeutralError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, NeutralError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn hash(&mut self) -> Result<Sha256Digest, NeutralError> {
        Ok(self.take(32)?.try_into().unwrap())
    }
    fn text(&mut self) -> Result<String, NeutralError> {
        let n = usize::try_from(self.u32()?).map_err(|_| bad(RuleProblem::ProfileFormat))?;
        let s = std::str::from_utf8(self.take(n)?).map_err(|_| bad(RuleProblem::ProfileFormat))?;
        if s.is_empty() {
            return Err(bad(RuleProblem::ProfileFormat));
        }
        Ok(s.into())
    }
    fn window(&mut self) -> Result<Window, NeutralError> {
        Ok(Window {
            va: self.u32()?,
            len: self.u32()?,
            digest: self.hash()?,
        })
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Window {
    pub va: u32,
    pub len: u32,
    pub digest: Sha256Digest,
}
#[derive(Clone, Copy, Default)]
struct Section {
    rva: u32,
    mapped: u32,
    raw: usize,
    raw_size: u32,
    flags: u32,
}
pub(crate) struct Pe<'a> {
    d: &'a [u8],
    base: u32,
    inline_sections: [Section; 16],
    heap_sections: Vec<Section>,
    section_count: usize,
}
impl<'a> Pe<'a> {
    fn sections(&self) -> &[Section] {
        if self.section_count <= self.inline_sections.len() {
            &self.inline_sections[..self.section_count]
        } else {
            &self.heap_sections
        }
    }

    pub(crate) fn parse(d: &'a [u8], budget: RuleInputBudget) -> Result<Self, NeutralError> {
        if bytes(d, 0, 2)? != b"MZ" {
            return Err(bad(RuleProblem::PeFormat));
        }
        let pe = usize::try_from(dword(d, 0x3c)?).map_err(|_| bad(RuleProblem::PeFormat))?;
        let coff = pe
            .checked_add(4)
            .ok_or_else(|| bad(RuleProblem::PeFormat))?;
        bytes(d, coff, 20)?;
        if bytes(d, pe, 4)? != b"PE\0\0" {
            return Err(bad(RuleProblem::PeFormat));
        }
        let count = usize::from(word(d, coff + 2)?);
        if count == 0 || count > budget.sections {
            return Err(bad(RuleProblem::Budget));
        }
        let opt = coff
            .checked_add(20)
            .ok_or_else(|| bad(RuleProblem::PeFormat))?;
        let opt_size = usize::from(word(d, coff + 16)?);
        if opt_size < 64 {
            return Err(bad(RuleProblem::PeFormat));
        }
        bytes(d, opt, opt_size)?;
        let machine = word(d, coff)?;
        let optional_magic = word(d, opt)?;
        if !matches!(optional_magic, 0x10b | 0x20b) || (optional_magic == 0x20b && opt_size < 112) {
            return Err(bad(RuleProblem::PeFormat));
        }
        if machine != 0x14c || optional_magic != 0x10b {
            return Err(bad(RuleProblem::UnsupportedArchitecture {
                machine,
                optional_magic,
            }));
        }
        let base = dword(d, opt + 28)?;
        let image_size = dword(d, opt + 56)?;
        base.checked_add(image_size)
            .ok_or_else(|| bad(RuleProblem::PeFormat))?;
        let start = opt
            .checked_add(opt_size)
            .ok_or_else(|| bad(RuleProblem::PeFormat))?;
        let table = bytes(
            d,
            start,
            count
                .checked_mul(40)
                .ok_or_else(|| bad(RuleProblem::PeFormat))?,
        )?;
        let mut inline_sections = [Section::default(); 16];
        let mut heap_sections = Vec::new();
        if count > inline_sections.len() {
            heap_sections
                .try_reserve_exact(count)
                .map_err(|_| bad(RuleProblem::Budget))?;
        }
        for (index, h) in table.as_chunks::<40>().0.iter().enumerate() {
            let virtual_size = dword(h, 8)?;
            let rva = dword(h, 12)?;
            let raw_size = dword(h, 16)?;
            let raw = usize::try_from(dword(h, 20)?).map_err(|_| bad(RuleProblem::PeFormat))?;
            let mapped = virtual_size.max(raw_size);
            if rva.checked_add(mapped).is_none_or(|end| end > image_size) {
                return Err(bad(RuleProblem::PeFormat));
            }
            bytes(d, raw, raw_size as usize)?;
            let section = Section {
                rva,
                mapped,
                raw,
                raw_size,
                flags: dword(h, 36)?,
            };
            if count <= inline_sections.len() {
                inline_sections[index] = section;
            } else {
                heap_sections.push(section);
            }
        }
        let sections = if count <= inline_sections.len() {
            &mut inline_sections[..count]
        } else {
            heap_sections.as_mut_slice()
        };
        sections.sort_by_key(|s| s.rva);
        if sections
            .windows(2)
            .any(|w| w[0].rva + w[0].mapped > w[1].rva)
        {
            return Err(bad(RuleProblem::PeFormat));
        }
        for (i, a) in sections.iter().enumerate() {
            for b in &sections[i + 1..] {
                if a.raw_size != 0
                    && b.raw_size != 0
                    && a.raw < b.raw + b.raw_size as usize
                    && b.raw < a.raw + a.raw_size as usize
                {
                    return Err(bad(RuleProblem::PeFormat));
                }
            }
        }
        Ok(Self {
            d,
            base,
            inline_sections,
            heap_sections,
            section_count: count,
        })
    }
    pub(crate) fn window(&self, w: &Window, code: bool) -> Result<&'a [u8], NeutralError> {
        if w.len == 0 {
            return Err(bad(RuleProblem::Range));
        }
        let data = self
            .read_from(w.va, code)?
            .get(..w.len as usize)
            .ok_or_else(|| bad(RuleProblem::Range))?;
        if sha256(data) != w.digest {
            return Err(bad(RuleProblem::RangeIdentity));
        }
        Ok(data)
    }
    pub(crate) fn code_ranges(&self) -> impl Iterator<Item = (u32, &'a [u8])> + '_ {
        self.sections()
            .iter()
            .filter(|s| Self::initialized(s, true) && s.raw_size != 0)
            .map(|s| {
                (
                    self.base + s.rva,
                    &self.d[s.raw..s.raw + s.raw_size as usize],
                )
            })
    }
    /// Initialized readable bytes, excluding virtual padding; callers bound their scans.
    pub(crate) fn initialized_ranges(&self) -> impl Iterator<Item = (u32, &'a [u8])> + '_ {
        self.sections()
            .iter()
            .filter(|s| Self::initialized(s, false) && s.raw_size != 0)
            .map(|s| {
                (
                    self.base + s.rva,
                    &self.d[s.raw..s.raw + s.raw_size as usize],
                )
            })
    }
    fn initialized(s: &Section, code: bool) -> bool {
        s.flags & 0x4000_0000 != 0
            && s.flags & 0x80 == 0
            && (!code || s.flags & 0x2000_0020 == 0x2000_0020)
            && s.flags & 0x60 != 0
    }
    /// Whether the whole range is zero-filled by the PE loader rather than stored in the file.
    pub(crate) fn zero_filled(&self, va: u32, len: u32) -> bool {
        let Some(rva) = va.checked_sub(self.base) else {
            return false;
        };
        let Some(end) = rva.checked_add(len).filter(|_| len != 0) else {
            return false;
        };
        self.sections().iter().any(|section| {
            rva >= section.rva + section.raw_size
                && end <= section.rva + section.mapped
                && section.flags & 0x4000_0000 != 0
        })
    }
    /// The remaining initialized bytes in one section, never its virtual padding.
    pub(crate) fn read_from(&self, va: u32, code: bool) -> Result<&'a [u8], NeutralError> {
        let rva = va
            .checked_sub(self.base)
            .ok_or_else(|| bad(RuleProblem::Range))?;
        let mut found = self
            .sections()
            .iter()
            .filter(|s| rva >= s.rva && rva < s.rva + s.mapped);
        let s = found.next().ok_or_else(|| bad(RuleProblem::Range))?;
        if found.next().is_some() || !Self::initialized(s, code) {
            return Err(bad(RuleProblem::Range));
        }
        let within = rva - s.rva;
        if within >= s.raw_size {
            return Err(bad(RuleProblem::Range));
        }
        bytes(
            self.d,
            s.raw + within as usize,
            (s.raw_size - within) as usize,
        )
    }
}
pub(super) fn import(
    image: &[u8],
    profile: &[u8],
    approval: &Sha256Digest,
    budget: RuleInputBudget,
    native_name: &'static str,
) -> Result<VerifiedNeutralRule, NeutralError> {
    if image.len() > budget.image_bytes || profile.len() > budget.profile_bytes {
        return Err(bad(RuleProblem::Budget));
    }
    if sha256(profile) != *approval {
        return Err(bad(RuleProblem::UnapprovedProfile));
    }
    let mut r = Reader { d: profile, at: 0 };
    if r.take(8)? != b"PCXNPR\0\x01" {
        return Err(bad(RuleProblem::ProfileFormat));
    }
    let size = r.u64()?;
    let image_sha256 = r.hash()?;
    if u64::try_from(image.len()).ok() != Some(size) || sha256(image) != image_sha256 {
        return Err(bad(RuleProblem::ImageIdentity));
    }
    let origin = AssetIdentity {
        edition: r.text()?,
        version: r.text()?,
        source: r.text()?,
    };
    let registration = r.window()?;
    let name = r.window()?;
    let signature = r.window()?;
    let getter = r.window()?;
    let header = r.window()?;
    let operands = [r.u32()?, r.u32()?, r.u32()?];
    if r.at != profile.len() {
        return Err(bad(RuleProblem::ProfileFormat));
    }
    let pe = Pe::parse(image, budget)?;
    let reg = pe.window(&registration, true)?;
    let name_data = pe.window(&name, false)?;
    let sig = pe.window(&signature, false)?;
    let getter_data = pe.window(&getter, true)?;
    let header_data = pe.window(&header, true)?;
    if name_data.len() != native_name.len() + 1
        || name_data.last() != Some(&0)
        || &name_data[..native_name.len()] != native_name.as_bytes()
        || sig != b"()I\0"
    {
        return Err(bad(RuleProblem::Binding));
    }
    for (i, (offset, expected)) in operands
        .iter()
        .zip([name.va, signature.va, getter.va])
        .enumerate()
    {
        let at = *offset as usize;
        let opcode = at
            .checked_sub(1)
            .and_then(|p| reg.get(p))
            .copied()
            .ok_or_else(|| bad(RuleProblem::Binding))?;
        if opcode != 0x68 && !(0xb8..=0xbf).contains(&opcode) {
            return Err(bad(RuleProblem::Binding));
        }
        if dword(reg, at)? != expected || operands[..i].iter().any(|old| old.abs_diff(*offset) < 5)
        {
            return Err(bad(RuleProblem::Binding));
        }
    }
    let (threshold, below, at_or_above) = decode_getter(getter_data)?;
    let (min_tag, max_tag) = decode_header(header_data)?;
    Ok(VerifiedNeutralRule {
        native_name,
        origin,
        image_sha256,
        profile_sha256: *approval,
        threshold,
        below,
        at_or_above,
        min_tag,
        max_tag,
    })
}
pub(super) fn decode_getter(d: &[u8]) -> Result<(u32, i32, i32), NeutralError> {
    let mut r = Reader { d, at: 0 };
    let mut expect = |b: &[u8]| -> Result<(), NeutralError> {
        if r.take(b.len())? != b {
            return Err(bad(RuleProblem::GetterShape));
        }
        Ok(())
    };
    expect(&[0xa1])?;
    r.take(4)?;
    if r.take(2)? != [0x8b, 0x48] {
        return Err(bad(RuleProblem::GetterShape));
    }
    r.take(1)?;
    if r.take(2)? != [0x8d, 0x49] {
        return Err(bad(RuleProblem::GetterShape));
    }
    r.take(1)?;
    if r.take(1)? != [0xe8] {
        return Err(bad(RuleProblem::GetterShape));
    }
    r.take(4)?;
    if r.take(2)? != [0x50, 0xe8] {
        return Err(bad(RuleProblem::GetterShape));
    }
    r.take(4)?;
    if r.take(2)? != [0x83, 0xc4]
        || r.take(1)?[0] as usize != std::mem::size_of::<u32>()
        || r.take(1)? != [0x3d]
    {
        return Err(bad(RuleProblem::GetterShape));
    }
    let threshold = r.u32()?;
    if r.take(4)? != [0x1b, 0xc0, 0x83, 0xe0] {
        return Err(bad(RuleProblem::GetterShape));
    }
    let mask = i32::from(i8::from_le_bytes([r.take(1)?[0]])) as u32;
    if r.take(2)? != [0x83, 0xc0] {
        return Err(bad(RuleProblem::GetterShape));
    }
    let add = i32::from(i8::from_le_bytes([r.take(1)?[0]])) as u32;
    if r.take(1)? != [0xc3] {
        return Err(bad(RuleProblem::GetterShape));
    }
    Ok((threshold, mask.wrapping_add(add) as i32, add as i32))
}
pub(super) fn decode_header(d: &[u8]) -> Result<(u32, u32), NeutralError> {
    let mut found = None;
    for (i, b) in windows_with_byte(d, 0, HEADER_FIRST_BYTE, 36) {
        if b[..2] != [0x83, 0xf8]
            || b[3] != 0x76
            || b[5..8] != [0x33, 0xc0, 0xe9]
            || b[12..14] != [0x83, 0xf8]
            || b[15] != 0x72
            || b[17..20] != [0x53, 0x8b, 0xce]
            || b[20] != 0xe8
            || b[25..27] != [0x8d, 0x43]
            || b[28..31] != [0x8b, 0xce, 0x50]
            || b[31] != 0xe8
        {
            continue;
        }
        let max = i32::from(i8::from_le_bytes([b[2]]));
        let min = i32::from(i8::from_le_bytes([b[14]]));
        let offset = i32::from(i8::from_le_bytes([b[27]]));
        let target1 = i as i64 + 25 + i64::from(i32::from_le_bytes(b[21..25].try_into().unwrap()));
        let target2 = i as i64 + 36 + i64::from(i32::from_le_bytes(b[32..36].try_into().unwrap()));
        // Two contiguous DWORD metadata fields and both scalar calls; no invented default.
        if min < 0
            || max < min
            || offset as usize != std::mem::size_of::<u32>()
            || 5 + i32::from(i8::from_le_bytes([b[4]])) != 12
            || 17 + i32::from(i8::from_le_bytes([b[16]])) != 36
            || target1 != target2
        {
            continue;
        }
        if found.replace((min as u32, max as u32)).is_some() {
            return Err(bad(RuleProblem::HeaderShape));
        }
    }
    found.ok_or_else(|| bad(RuleProblem::HeaderShape))
}

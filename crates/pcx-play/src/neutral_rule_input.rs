//! Verified local profile + complete PE input. No I/O or measured target operands.
use super::neutral_player::{
    NeutralError, RuleInputBudget, RuleProblem, Sha256Digest, VerifiedNeutralRule, sha256,
};
use crate::scripts::AssetIdentity;
fn bad(p: RuleProblem) -> NeutralError {
    NeutralError::Rule(p)
}
fn bytes(d: &[u8], at: usize, n: usize) -> Result<&[u8], NeutralError> {
    d.get(at..at.checked_add(n).ok_or_else(|| bad(RuleProblem::Range))?)
        .ok_or_else(|| bad(RuleProblem::Range))
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
    sections: Vec<Section>,
}
impl<'a> Pe<'a> {
    pub(crate) fn parse(d: &'a [u8], budget: RuleInputBudget) -> Result<Self, NeutralError> {
        if bytes(d, 0, 2)? != b"MZ" {
            return Err(bad(RuleProblem::PeFormat));
        }
        let pe = usize::try_from(dword(d, 0x3c)?).map_err(|_| bad(RuleProblem::PeFormat))?;
        let coff = pe
            .checked_add(4)
            .ok_or_else(|| bad(RuleProblem::PeFormat))?;
        bytes(d, coff, 20)?;
        if bytes(d, pe, 4)? != b"PE\0\0" || word(d, coff)? != 0x14c {
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
        if opt_size < 64 || word(d, opt)? != 0x10b {
            return Err(bad(RuleProblem::PeFormat));
        }
        bytes(d, opt, opt_size)?;
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
        let mut sections = Vec::new();
        sections
            .try_reserve_exact(count)
            .map_err(|_| bad(RuleProblem::Budget))?;
        for h in table.as_chunks::<40>().0 {
            let virtual_size = dword(h, 8)?;
            let rva = dword(h, 12)?;
            let raw_size = dword(h, 16)?;
            let raw = usize::try_from(dword(h, 20)?).map_err(|_| bad(RuleProblem::PeFormat))?;
            let mapped = virtual_size.max(raw_size);
            if rva.checked_add(mapped).is_none_or(|end| end > image_size) {
                return Err(bad(RuleProblem::PeFormat));
            }
            bytes(d, raw, raw_size as usize)?;
            sections.push(Section {
                rva,
                mapped,
                raw,
                raw_size,
                flags: dword(h, 36)?,
            });
        }
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
        Ok(Self { d, base, sections })
    }
    pub(crate) fn window(&self, w: &Window, code: bool) -> Result<&'a [u8], NeutralError> {
        if w.len == 0 {
            return Err(bad(RuleProblem::Range));
        }
        let rva =
            w.va.checked_sub(self.base)
                .ok_or_else(|| bad(RuleProblem::Range))?;
        let end = rva
            .checked_add(w.len)
            .ok_or_else(|| bad(RuleProblem::Range))?;
        let mut found = self
            .sections
            .iter()
            .filter(|s| rva >= s.rva && end <= s.rva + s.mapped);
        let s = found.next().ok_or_else(|| bad(RuleProblem::Range))?;
        if found.next().is_some()
            || s.flags & 0x4000_0000 == 0
            || s.flags & 0x80 != 0
            || (code && (s.flags & 0x2000_0020 != 0x2000_0020))
            || s.flags & 0x60 == 0
        {
            return Err(bad(RuleProblem::Range));
        }
        let within = rva - s.rva;
        if within.checked_add(w.len).is_none_or(|e| e > s.raw_size) {
            return Err(bad(RuleProblem::Range));
        }
        let data = bytes(self.d, s.raw + within as usize, w.len as usize)?;
        if sha256(data) != w.digest {
            return Err(bad(RuleProblem::RangeIdentity));
        }
        Ok(data)
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
fn decode_getter(d: &[u8]) -> Result<(u32, i32, i32), NeutralError> {
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
fn decode_header(d: &[u8]) -> Result<(u32, u32), NeutralError> {
    let mut found = None;
    for (i, b) in d.windows(36).enumerate() {
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

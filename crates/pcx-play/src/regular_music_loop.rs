//! Static loop capability for the selected client and the built-in Miles backend.
//! Numeric modes are deliberately independent of regular/thematic role names.
use crate::{
    client_module::{ImportDemand, ImportLimits},
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::sha256,
    pe_image::{PeSections, u32_at},
};

type Read<'a> = dyn Fn(u32, usize) -> Result<Vec<u8>, String> + 'a;
const BACKEND_HASH: [u8; 32] = [
    0xcc, 0x14, 0x2b, 0x9d, 0x8b, 0x52, 0x23, 0xe2, 0x72, 0x0c, 0x64, 0x40, 0xcb, 0x7a, 0x12, 0x4c,
    0x0a, 0x80, 0xd2, 0xfb, 0x04, 0xec, 0xf5, 0x9a, 0xd7, 0x33, 0x1d, 0xfd, 0x6e, 0x3c, 0xb5, 0x1f,
];
const EXPORT: &[u8] = b"_AIL_set_stream_loop_count@8\0";

fn bytes(r: &Read<'_>, at: u32, len: usize) -> Result<Vec<u8>, String> {
    let b = r(at, len)?;
    if b.len() != len {
        return Err(format!("truncated music loop window {at:#x}"));
    }
    Ok(b)
}
fn check(r: &Read<'_>, at: u32, expected: &[u8]) -> Result<(), String> {
    if bytes(r, at, expected.len())? != expected {
        return Err(format!("music loop instruction differs at {at:#x}"));
    }
    Ok(())
}
fn operand(r: &Read<'_>, at: u32, prefix: &[u8], len: usize) -> Result<Vec<u8>, String> {
    check(r, at, prefix)?;
    bytes(
        r,
        at.checked_add(prefix.len() as u32)
            .ok_or("loop address overflow")?,
        len,
    )
}
fn imm(r: &Read<'_>, at: u32, prefix: &[u8]) -> Result<u32, String> {
    Ok(u32::from_le_bytes(
        operand(r, at, prefix, 4)?.try_into().unwrap(),
    ))
}
fn byte(r: &Read<'_>, at: u32, prefix: &[u8]) -> Result<u8, String> {
    Ok(operand(r, at, prefix, 1)?[0])
}
fn branch(r: &Read<'_>, at: u32, opcode: &[u8], target: u32) -> Result<(), String> {
    let len = if opcode == [0xe8] || opcode.len() == 2 {
        4
    } else {
        1
    };
    let b = operand(r, at, opcode, len)?;
    let delta = if len == 1 {
        i64::from(b[0] as i8)
    } else {
        i64::from(i32::from_le_bytes(b.try_into().unwrap()))
    };
    if i64::from(at) + opcode.len() as i64 + len as i64 + delta != i64::from(target) {
        return Err(format!("music loop branch differs at {at:#x}"));
    }
    Ok(())
}
fn reader<'a>(pe: &'a PeSections<'a>) -> impl Fn(u32, usize) -> Result<Vec<u8>, String> + 'a {
    move |at, len| {
        pe.va_bytes(at, len)
            .map(<[u8]>::to_vec)
            .ok_or_else(|| format!("music loop source range {at:#x}"))
    }
}
fn main_image(image: &[u8]) -> Result<PeSections<'_>, String> {
    if image.len() > 32 << 20 || sha256(image) != REVIEWED_IMAGE {
        return Err("unsupported music loop client identity".into());
    }
    PeSections::parse(image).map_err(str::to_owned)
}

#[derive(Clone, Debug)]
pub struct Predicate {
    pub image_sha256: [u8; 32],
    pub playlist_count_global: u32,
    always_mode: i32,
    counted_mode: i32,
    count: usize,
}
impl Predicate {
    pub fn read(image: &[u8]) -> Result<Self, String> {
        let pe = main_image(image)?;
        Self::decode(&reader(&pe))
    }
    fn decode(r: &Read<'_>) -> Result<Self, String> {
        check(r, 0x5d8d56, &[0x8b, 0x45, 0x0c, 0x56])?;
        let always_mode = i32::from(byte(r, 0x5d8d5a, &[0x83, 0xf8])? as i8);
        branch(r, 0x5d8d5d, &[0x74], 0x5d8d70)?;
        check(r, 0x5d8d5f, &[0x85, 0xc0])?;
        branch(r, 0x5d8d61, &[0x75], 0x5d8d6c)?;
        let count_global = imm(r, 0x5d8d63, &[0x83, 0x3d])?;

        let count = usize::try_from(i32::from(byte(r, 0x5d8d69, &[])? as i8))
            .map_err(|_| "negative music loop playlist comparison")?;
        branch(r, 0x5d8d6a, &[0x74], 0x5d8d70)?;
        check(r, 0x5d8d6c, &[0x33, 0xf6])?;
        branch(r, 0x5d8d6e, &[0xeb], 0x5d8d75)?;
        if imm(r, 0x5d8d70, &[0xbe])? == 0 {
            return Err("music loop predicate true is zero".into());
        }
        let looping = imm(r, 0x5d8d7d, &[0xb9])?;
        check(r, 0x5d8d82, &[0x85, 0xf6, 0x6a, 0xff])?;
        let once = imm(r, 0x5d8d86, &[0xb8])?;
        check(r, 0x5d8d8b, &[0x0f, 0x45, 0xc1, 0x50])?;
        branch(r, 0x5d8d94, &[0xe8], 0x79ee30)?;
        check(r, 0x79ee4b, &[0x8b, 0x5d, 0x10])?;
        let preserve = byte(r, 0x79ee52, &[0x83, 0xe3])?;
        check(r, 0x79ee5f, &[0x0f, 0x44, 0x5d, 0x10])?;
        let one = byte(r, 0x79eedb, &[0x6a])?;
        check(r, 0x79eef0, &[0x53, 0x57, 0xff, 0x75, 0x08])?;
        branch(r, 0x79eef5, &[0xe8], 0x7c6000)?;
        check(r, 0x7c6030, &[0x8b, 0x4d, 0x10])?;
        let mask = byte(r, 0x7c6033, &[0xf6, 0xc1])?;
        // Only MOV instructions intervene between TEST and CMOVNE, preserving ZF.
        for (at, code) in [
            (0x7c6036, &[0xf3, 0x0f, 0x10, 0x45, 0x1c][..]),
            (0x7c6041, &[0x8b, 0x45, 0x14]),
            (0x7c604a, &[0x8b, 0x45, 0x18]),
            (0x7c6053, &[0x8b, 0x45, 0x24]),
            (0x7c605c, &[0x8b, 0x45, 0x20]),
            (0x7c607e, &[0x8b, 0xce]),
        ] {
            check(r, at, code)?;
        }
        for (at, code) in [
            (0x7c603b, &[0x89, 0x86][..]),
            (0x7c6044, &[0x89, 0x86]),
            (0x7c604d, &[0x89, 0x86]),
            (0x7c6056, &[0x89, 0x86]),
            (0x7c605f, &[0x89, 0x8e]),
            (0x7c606a, &[0x89, 0x86]),
            (0x7c6076, &[0xf3, 0x0f, 0x11, 0x86]),
        ] {
            imm(r, at, code)?;
        }
        let zero = imm(r, 0x7c6065, &[0xb9])?;
        check(r, 0x7c6070, &[0x8b, 0x45, 0x28, 0x0f, 0x45, 0xc1])?;
        let field = imm(r, 0x7c6080, &[0x89, 0x86])?;
        if field != imm(r, 0x7c7d53, &[0xff, 0xb6])?
            || mask == 0
            || mask & preserve != mask
            || looping & u32::from(mask) == 0
            || once & u32::from(mask) != 0
            || zero != 0
            || one != 1
        {
            return Err("music loop flags/count relationship differs".into());
        }
        Ok(Self {
            image_sha256: REVIEWED_IMAGE,
            playlist_count_global: count_global,
            always_mode,
            counted_mode: 0,
            count,
        })
    }
    pub fn should_loop(&self, mode: i32, count: usize) -> Result<bool, String> {
        if count == 0 || count > i32::MAX as usize {
            return Err("music playlist count outside native domain".into());
        }
        Ok(mode == self.always_mode || (mode == self.counted_mode && count == self.count))
    }
}

/// The caller selects the interpreted implementation; disk initialization proves no live dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackendMode {
    Builtin,
    Alternate,
}
#[derive(Clone, Debug)]
pub struct LoopRules {
    pub image_sha256: [u8; 32],
    pub backend_sha256: [u8; 32],
    predicate: Predicate,
}
impl LoopRules {
    pub fn read(main: &[u8], backend: &[u8]) -> Result<Self, String> {
        let pe = main_image(main)?;
        if backend.len() > 4 << 20 || sha256(backend) != BACKEND_HASH {
            return Err("unsupported music loop backend identity".into());
        }
        let dll = PeSections::parse(backend)?;
        let r = reader(&pe);
        let iat = imm(&r, 0x7c7d5f, &[0xff, 0x15])?;
        let demand = ImportDemand::for_iat(main, iat, ImportLimits::default())?;
        if !demand.module_name.eq_ignore_ascii_case("mss32.dll")
            || demand.import_name.as_bytes() != &EXPORT[..EXPORT.len() - 1]
        {
            return Err("music loop import owner differs".into());
        }
        let entry = export_entry(backend, &dll)?;
        Self::decode(&r, &reader(&dll), entry)
    }
    fn decode(main: &Read<'_>, dll: &Read<'_>, entry: u32) -> Result<Self, String> {
        let predicate = Predicate::decode(main)?;
        // Export RVA comes from the table, not the IAT's unbound disk contents.
        if entry != 0x2110b340 {
            return Err("music loop export entry differs".into());
        }
        check(
            dll,
            entry + 0x13,
            &[0x8b, 0x75, 0x0c, 0x57, 0x8b, 0x7d, 0x08],
        )?;
        let dispatch = imm(dll, entry + 0x57, &[0xa1])?;
        check(dll, entry + 0x5c, &[0x85, 0xc0])?;
        branch(dll, entry + 0x5e, &[0x74], entry + 0x7a)?;
        check(dll, entry + 0x7a, &[0x56, 0x57])?;
        branch(dll, entry + 0x7c, &[0xe8], 0x2111e050)?;
        check(dll, 0x2111e053, &[0x8b, 0x45, 0x08, 0x85, 0xc0])?;
        branch(dll, 0x2111e058, &[0x74], 0x2111e07b)?;
        let buffered = imm(dll, 0x2111e05a, &[0x8b, 0x88])?;
        check(dll, 0x2111e060, &[0x85, 0xc9, 0x8b, 0x4d, 0x0c])?;
        branch(dll, 0x2111e065, &[0x74], 0x2111e075)?;
        check(dll, 0x2111e067, &[0x8b, 0x50, 0x0c, 0x51, 0x52])?;
        branch(dll, 0x2111e06c, &[0xe8], 0x21105370)?;
        let field = imm(dll, 0x2111e075, &[0x89, 0x88])?;
        if buffered != imm(dll, 0x2111c939, &[0x89, 0xbe])? {
            return Err("buffered stream field differs".into());
        }
        if dispatch != imm(dll, 0x211053c7, &[0xa1])? {
            return Err("loop dispatch globals differ".into());
        }
        check(dll, 0x21105383, &[0x8b, 0x75, 0x0c, 0x57, 0x8b, 0x7d, 0x08])?;
        check(dll, 0x211053cc, &[0x85, 0xc0])?;
        branch(dll, 0x211053ce, &[0x74], 0x211053ea)?;
        check(dll, 0x211053ea, &[0x56, 0x57])?;
        branch(dll, 0x211053ec, &[0xe8], 0x21114860)?;
        check(dll, 0x21114863, &[0x8b, 0x45, 0x08, 0x85, 0xc0])?;
        branch(dll, 0x21114868, &[0x74], 0x21114870)?;
        check(dll, 0x2111486a, &[0x8b, 0x4d, 0x0c])?;
        let sample = byte(dll, 0x2111486d, &[0x89, 0x48])?;
        let end = imm(dll, 0x2111d9fc, &[0x8b, 0x86])?;
        let cursor = imm(dll, 0x2111da02, &[0x3b, 0x86])?;
        branch(dll, 0x2111da08, &[0x0f, 0x85], 0x2111daf6)?;
        if field != imm(dll, 0x2111da0e, &[0x8b, 0x8e])? {
            return Err("stream EOF count differs".into());
        }
        let one = imm(dll, 0x2111da14, &[0xb8])?;
        check(dll, 0x2111da19, &[0x3b, 0xc8])?;
        branch(dll, 0x2111da1b, &[0x75], 0x2111da2f)?;
        check(dll, 0x2111da1d, &[0x5f, 0x89, 0x86])?;
        check(
            dll,
            0x2111da24,
            &[
                0x8b, 0x45, 0xfc, 0x5e, 0x5b, 0x8b, 0xe5, 0x5d, 0xc2, 0x04, 0x00,
            ],
        )?;
        for (load, store, skip, target) in [
            (0x2111daa8, 0x2111dab3, 0x2111dab0, 0x2111dab9),
            (0x2111d997, 0x2111d9a2, 0x2111d99f, 0x2111d9a8),
        ] {
            if field != imm(dll, load, &[0x8b, 0x86])? || field != imm(dll, store, &[0x89, 0x86])? {
                return Err("stream decrement field differs".into());
            }
            check(dll, load + 6, &[0x85, 0xc0])?;
            branch(dll, skip, &[0x74], target)?;
            check(dll, store - 1, &[0x48])?;
        }
        if cursor != imm(dll, 0x2111dad0, &[0xc7, 0x86])?
            || imm(dll, 0x2111dad6, &[])? != 0
            || cursor == end
        {
            return Err("stream rewind cursor differs".into());
        }
        check(dll, 0x2111dac2, &[0x6a, 0x00, 0x51, 0x52])?;
        if imm(dll, 0x2111dada, &[0xff, 0x15])? != imm(dll, 0x2111d9e0, &[0xff, 0x15])? {
            return Err("stream rewind call differs".into());
        }
        if sample != byte(dll, 0x211125da, &[0x83, 0x7e])?
            || u32::from(byte(dll, 0x211125dd, &[])?) != one
            || one != 1
        {
            return Err("sample completion count differs".into());
        }
        branch(dll, 0x211125de, &[0x74], 0x21112620)?;
        check(dll, 0x211125e0, &[0x8b, 0x4e])?;
        byte(dll, 0x211125e2, &[])?;
        check(dll, 0x211125e3, &[0x8b, 0x56, 0x40, 0x89, 0x4c, 0x96, 0x1c])?;
        if sample != byte(dll, 0x21112613, &[0x8b, 0x46])?
            || sample != byte(dll, 0x2111261b, &[0x89, 0x46])?
        {
            return Err("sample decrement field differs".into());
        }
        check(dll, 0x21112616, &[0x85, 0xc0])?;
        branch(dll, 0x21112618, &[0x74], 0x21112668)?;
        check(dll, 0x2111261a, &[0x48])?;
        branch(dll, 0x2111261e, &[0xeb], 0x21112668)?;
        Ok(Self {
            image_sha256: REVIEWED_IMAGE,
            backend_sha256: BACKEND_HASH,
            predicate,
        })
    }
    pub fn predicate(&self) -> &Predicate {
        &self.predicate
    }
    pub fn builtin_looping(
        &self,
        mode: i32,
        count: usize,
        backend: BackendMode,
    ) -> Result<bool, String> {
        if backend != BackendMode::Builtin {
            return Err("alternate music backend loop behavior is unqualified".into());
        }
        self.predicate.should_loop(mode, count)
    }
}

fn export_entry(image: &[u8], pe: &PeSections<'_>) -> Result<u32, String> {
    let header = u32_at(image, 0x3c).ok_or("loop backend PE header")? as usize;
    let optional = header
        .checked_add(24)
        .ok_or("loop backend header overflow")?;
    let directory = u32_at(
        image,
        optional
            .checked_add(96)
            .ok_or("loop export header overflow")?,
    )
    .ok_or("loop backend export directory")?;
    let size = u32_at(
        image,
        optional
            .checked_add(100)
            .ok_or("loop export header overflow")?,
    )
    .ok_or("loop backend export size")?;
    let end = directory
        .checked_add(size)
        .ok_or("loop export range overflow")?;
    let va = |rva: u32| {
        pe.image_base
            .checked_add(rva)
            .ok_or("loop export address overflow")
    };
    let r = reader(pe);
    let d = bytes(&r, va(directory)?, 40)?;
    let functions = u32_at(&d, 20).unwrap();
    let names = u32_at(&d, 24).unwrap();
    if names > 65536 || functions > 65536 {
        return Err("loop export work bound".into());
    }
    let table = |at: usize, count: u32, width: usize| {
        bytes(&r, va(u32_at(&d, at).unwrap())?, count as usize * width)
    };
    let f = table(28, functions, 4)?;
    let n = table(32, names, 4)?;
    let o = table(36, names, 2)?;
    let mut found = None;
    for i in 0..names as usize {
        let name = bytes(&r, va(u32_at(&n, i * 4).unwrap())?, EXPORT.len())?;
        if name != EXPORT {
            continue;
        }
        let ordinal = u16::from_le_bytes(o[i * 2..i * 2 + 2].try_into().unwrap()) as usize;
        let target = u32_at(&f, ordinal * 4).ok_or("loop export ordinal outside table")?;
        if (directory..end).contains(&target) || found.is_some() {
            return Err("forwarded/ambiguous loop export".into());
        }
        found = Some(va(target)?);
    }
    found.ok_or_else(|| "loop export absent".into())
}

#[cfg(test)]
impl Predicate {
    pub(crate) fn fixture() -> Self {
        Self {
            image_sha256: REVIEWED_IMAGE,
            playlist_count_global: 0x2000,
            always_mode: 2,
            counted_mode: 0,
            count: 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    type Memory = BTreeMap<u32, u8>;
    fn put(m: &mut Memory, at: u32, b: &[u8]) {
        for (i, b) in b.iter().enumerate() {
            m.insert(at + i as u32, *b);
        }
    }
    fn word(m: &mut Memory, at: u32, p: &[u8], n: u32) {
        put(m, at, p);
        put(m, at + p.len() as u32, &n.to_le_bytes());
    }
    fn jump(m: &mut Memory, at: u32, p: &[u8], target: u32) {
        put(m, at, p);
        let len = if p == [0xe8] || p.len() == 2 { 4 } else { 1 };
        let delta = i64::from(target) - i64::from(at) - p.len() as i64 - len as i64;
        put(m, at + p.len() as u32, &(delta as i32).to_le_bytes()[..len]);
    }
    fn read(m: &Memory, at: u32, len: usize) -> Result<Vec<u8>, String> {
        (0..len)
            .map(|i| {
                m.get(&(at + i as u32))
                    .copied()
                    .ok_or_else(|| format!("fixture range {at:#x}"))
            })
            .collect()
    }
    fn fixture() -> (Memory, Memory) {
        let mut m = Memory::new();
        let mut d = Memory::new();
        for (at, code) in [
            (0x7c6036, &[0xf3, 0x0f, 0x10, 0x45, 0x1c][..]),
            (0x7c6041, &[0x8b, 0x45, 0x14]),
            (0x7c604a, &[0x8b, 0x45, 0x18]),
            (0x7c6053, &[0x8b, 0x45, 0x24]),
            (0x7c605c, &[0x8b, 0x45, 0x20]),
            (0x7c607e, &[0x8b, 0xce]),
        ] {
            put(&mut m, at, code);
        }
        for (at, code) in [
            (0x7c603b, &[0x89, 0x86][..]),
            (0x7c6044, &[0x89, 0x86]),
            (0x7c604d, &[0x89, 0x86]),
            (0x7c6056, &[0x89, 0x86]),
            (0x7c605f, &[0x89, 0x8e]),
            (0x7c606a, &[0x89, 0x86]),
            (0x7c6076, &[0xf3, 0x0f, 0x11, 0x86]),
        ] {
            word(&mut m, at, code, 0x888);
        }

        put(
            &mut d,
            0x2110b353,
            &[0x8b, 0x75, 0x0c, 0x57, 0x8b, 0x7d, 0x08],
        );
        put(
            &mut d,
            0x21105383,
            &[0x8b, 0x75, 0x0c, 0x57, 0x8b, 0x7d, 0x08],
        );
        for (at, b) in [
            (0x5d8d56, &[0x8b, 0x45, 0x0c, 0x56][..]),
            (0x5d8d5a, &[0x83, 0xf8, 7]),
            (0x5d8d5f, &[0x85, 0xc0]),
            (0x5d8d69, &[3]),
            (0x5d8d6c, &[0x33, 0xf6]),
            (0x5d8d82, &[0x85, 0xf6, 0x6a, 0xff]),
            (0x5d8d8b, &[0x0f, 0x45, 0xc1, 0x50]),
            (0x79ee4b, &[0x8b, 0x5d, 0x10]),
            (0x79ee52, &[0x83, 0xe3, 0xf7]),
            (0x79ee5f, &[0x0f, 0x44, 0x5d, 0x10]),
            (0x79eedb, &[0x6a, 1]),
            (0x79eef0, &[0x53, 0x57, 0xff, 0x75, 0x08]),
            (0x7c6030, &[0x8b, 0x4d, 0x10]),
            (0x7c6033, &[0xf6, 0xc1, 4]),
            (0x7c6070, &[0x8b, 0x45, 0x28, 0x0f, 0x45, 0xc1]),
        ] {
            put(&mut m, at, b);
        }
        for (at, p, n) in [
            (0x5d8d63, &[0x83, 0x3d][..], 0x2000),
            (0x5d8d70, &[0xbe], 1),
            (0x5d8d7d, &[0xb9], 0x44),
            (0x5d8d86, &[0xb8], 0x80),
            (0x7c6065, &[0xb9], 0),
            (0x7c6080, &[0x89, 0x86], 0x380),
            (0x7c7d53, &[0xff, 0xb6], 0x380),
        ] {
            word(&mut m, at, p, n);
        }
        for (at, p, t) in [
            (0x5d8d5d, &[0x74][..], 0x5d8d70),
            (0x5d8d61, &[0x75], 0x5d8d6c),
            (0x5d8d6a, &[0x74], 0x5d8d70),
            (0x5d8d6e, &[0xeb], 0x5d8d75),
            (0x5d8d94, &[0xe8], 0x79ee30),
            (0x79eef5, &[0xe8], 0x7c6000),
        ] {
            jump(&mut m, at, p, t);
        }
        for (at, b) in [
            (0x2110b39c, &[0x85, 0xc0][..]),
            (0x2110b3ba, &[0x56, 0x57]),
            (0x2111e053, &[0x8b, 0x45, 8, 0x85, 0xc0]),
            (0x2111e060, &[0x85, 0xc9, 0x8b, 0x4d, 0x0c]),
            (0x2111e067, &[0x8b, 0x50, 0x0c, 0x51, 0x52]),
            (0x211053cc, &[0x85, 0xc0]),
            (0x211053ea, &[0x56, 0x57]),
            (0x21114863, &[0x8b, 0x45, 8, 0x85, 0xc0]),
            (0x2111486a, &[0x8b, 0x4d, 0x0c]),
            (0x2111486d, &[0x89, 0x48, 0x60]),
            (0x2111da19, &[0x3b, 0xc8]),
            (0x2111da1d, &[0x5f, 0x89, 0x86]),
            (
                0x2111da24,
                &[0x8b, 0x45, 0xfc, 0x5e, 0x5b, 0x8b, 0xe5, 0x5d, 0xc2, 4, 0],
            ),
            (0x2111daae, &[0x85, 0xc0]),
            (0x2111dab2, &[0x48]),
            (0x2111d99d, &[0x85, 0xc0]),
            (0x2111d9a1, &[0x48]),
            (0x2111dac2, &[0x6a, 0, 0x51, 0x52]),
            (0x211125da, &[0x83, 0x7e, 0x60, 1]),
            (0x211125e0, &[0x8b, 0x4e, 0x64]),
            (0x211125e3, &[0x8b, 0x56, 0x40, 0x89, 0x4c, 0x96, 0x1c]),
            (0x21112613, &[0x8b, 0x46, 0x60]),
            (0x21112616, &[0x85, 0xc0]),
            (0x2111261a, &[0x48]),
            (0x2111261b, &[0x89, 0x46, 0x60]),
        ] {
            put(&mut d, at, b);
        }
        for (at, p, n) in [
            (0x2110b397, &[0xa1][..], 0x3000),
            (0x211053c7, &[0xa1], 0x3000),
            (0x2111e05a, &[0x8b, 0x88], 0x180),
            (0x2111c939, &[0x89, 0xbe], 0x180),
            (0x2111e075, &[0x89, 0x88], 0x190),
            (0x2111da0e, &[0x8b, 0x8e], 0x190),
            (0x2111daa8, &[0x8b, 0x86], 0x190),
            (0x2111dab3, &[0x89, 0x86], 0x190),
            (0x2111d997, &[0x8b, 0x86], 0x190),
            (0x2111d9a2, &[0x89, 0x86], 0x190),
            (0x2111d9fc, &[0x8b, 0x86], 0x194),
            (0x2111da02, &[0x3b, 0x86], 0x198),
            (0x2111da14, &[0xb8], 1),
            (0x2111dad0, &[0xc7, 0x86], 0x198),
            (0x2111dad6, &[], 0),
            (0x2111dada, &[0xff, 0x15], 0x4000),
            (0x2111d9e0, &[0xff, 0x15], 0x4000),
        ] {
            word(&mut d, at, p, n);
        }
        for (at, p, t) in [
            (0x2110b39e, &[0x74][..], 0x2110b3ba),
            (0x2110b3bc, &[0xe8], 0x2111e050),
            (0x2111e058, &[0x74], 0x2111e07b),
            (0x2111e065, &[0x74], 0x2111e075),
            (0x2111e06c, &[0xe8], 0x21105370),
            (0x211053ce, &[0x74], 0x211053ea),
            (0x211053ec, &[0xe8], 0x21114860),
            (0x21114868, &[0x74], 0x21114870),
            (0x2111da08, &[0x0f, 0x85], 0x2111daf6),
            (0x2111da1b, &[0x75], 0x2111da2f),
            (0x2111dab0, &[0x74], 0x2111dab9),
            (0x2111d99f, &[0x74], 0x2111d9a8),
            (0x211125de, &[0x74], 0x21112620),
            (0x21112618, &[0x74], 0x21112668),
            (0x2111261e, &[0xeb], 0x21112668),
        ] {
            jump(&mut d, at, p, t);
        }
        (m, d)
    }
    fn decode(m: &Memory, d: &Memory) -> Result<LoopRules, String> {
        LoopRules::decode(&|at, n| read(m, at, n), &|at, n| read(d, at, n), 0x2110b340)
    }
    #[test]
    fn regular_loop_nonstock_operands_and_relocated_fields() {
        let (m, d) = fixture();
        let rules = decode(&m, &d).unwrap();
        assert_eq!(rules.image_sha256, rules.predicate().image_sha256);
        assert_eq!(rules.backend_sha256, BACKEND_HASH);
        assert_eq!(rules.predicate().playlist_count_global, 0x2000);
        for (mode, count, expected) in [
            (7, 4, true),
            (0, 3, true),
            (0, 1, false),
            (2, 1, false),
            (-1, 3, false),
        ] {
            assert_eq!(
                rules
                    .builtin_looping(mode, count, BackendMode::Builtin)
                    .unwrap(),
                expected
            );
        }
        assert!(rules.builtin_looping(7, 1, BackendMode::Alternate).is_err());
        assert!(rules.predicate().should_loop(0, 0).is_err());
        assert!(rules.predicate().should_loop(0, usize::MAX).is_err());
        assert!(Predicate::fixture().should_loop(2, 9).unwrap());
    }
    #[test]
    fn regular_loop_rejects_malformed_relationships_and_truncation() {
        let (m, d) = fixture();
        for at in [
            0x5d8d5d, 0x5d8d61, 0x5d8d6a, 0x5d8d8b, 0x79eef5, 0x7c6073, 0x7c6082,
        ] {
            let mut changed = m.clone();
            *changed.get_mut(&at).unwrap() ^= 1;
            assert!(decode(&changed, &d).is_err(), "main {at:#x}");
        }
        for at in [
            0x2110b39e, 0x2110b3bd, 0x2111e065, 0x2111e077, 0x2111c93b, 0x211053c8, 0x2111486f,
            0x2111da10, 0x2111dab0, 0x2111dab5, 0x2111dad2, 0x2111dad6, 0x211125dd, 0x211125de,
            0x21112618, 0x2111261d,
        ] {
            let mut changed = d.clone();
            *changed.get_mut(&at).unwrap() ^= 1;
            assert!(decode(&m, &changed).is_err(), "backend {at:#x}");
        }
        for at in d.keys() {
            let mut changed = d.clone();
            changed.remove(at);
            assert!(decode(&m, &changed).is_err(), "missing {at:#x}");
        }
        for at in m.keys() {
            let mut changed = m.clone();
            changed.remove(at);
            assert!(decode(&changed, &d).is_err(), "missing {at:#x}");
        }
        assert!(Predicate::read(b"missing").is_err());
        assert!(LoopRules::read(b"missing", b"missing").is_err());
    }
    #[test]
    fn regular_loop_export_uses_relocated_tables_and_rejects_bad_ranges() {
        use crate::pe_image::{RDATA, tests::synthetic_image};
        let mut section = vec![0u8; 512];
        for (at, value) in [
            (20, 1u32),
            (24, 1),
            (28, 0x1080),
            (32, 0x1090),
            (36, 0x10a0),
            (0x80, 0x2000),
            (0x90, 0x10b0),
        ] {
            section[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
        section[0xb0..0xb0 + EXPORT.len()].copy_from_slice(EXPORT);
        let mut image = synthetic_image(&[(RDATA, 0x1000, &section)]);
        image[0x98 + 96..0x98 + 100].copy_from_slice(&0x1000u32.to_le_bytes());
        image[0x98 + 100..0x98 + 104].copy_from_slice(&0x200u32.to_le_bytes());
        for base in [0x400000u32, 0x30000000] {
            image[0x98 + 28..0x98 + 32].copy_from_slice(&base.to_le_bytes());
            assert_eq!(
                export_entry(&image, &PeSections::parse(&image).unwrap()).unwrap(),
                base + 0x2000
            );
        }
        for (offset, value) in [
            (0x400 + 0x80, 0x1100u32),
            (0x400 + 24, 65537),
            (0x400 + 36, u32::MAX),
        ] {
            let mut bad = image.clone();
            bad[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            assert!(export_entry(&bad, &PeSections::parse(&bad).unwrap()).is_err());
        }
        for len in [0, 80, 256, 1024] {
            let bad = &image[..len];
            if let Ok(pe) = PeSections::parse(bad) {
                assert!(export_entry(bad, &pe).is_err());
            }
        }
    }
    #[test]
    #[ignore = "requires explicit PCX_WAR3_EXE and PCX_MUSIC_BACKEND selected local images"]
    fn selected_regular_loop_source_gate() {
        use std::io::Read as _;
        let load = |key, limit| {
            let mut b = Vec::new();
            std::fs::File::open(std::env::var_os(key).expect("explicit selected image"))
                .unwrap()
                .take(limit + 1)
                .read_to_end(&mut b)
                .unwrap();
            assert!(b.len() as u64 <= limit);
            b
        };
        let main = load("PCX_WAR3_EXE", 32 << 20);
        let dll = load("PCX_MUSIC_BACKEND", 4 << 20);
        let rules = LoopRules::read(&main, &dll).unwrap();
        assert!(rules.builtin_looping(2, 2, BackendMode::Builtin).unwrap());
        assert!(rules.builtin_looping(0, 1, BackendMode::Builtin).unwrap());
        assert!(!rules.builtin_looping(0, 2, BackendMode::Builtin).unwrap());
        assert!(rules.builtin_looping(2, 1, BackendMode::Alternate).is_err());
        let mut bad = dll.clone();
        bad[0] ^= 1;
        assert!(LoopRules::read(&main, &bad).is_err());
    }
}

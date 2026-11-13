//! Selected initial-C StringCase and safe native SubString byte operations.
use crate::{
    camera_margins::SourceWindow,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    scripts::AssetIdentity,
};
use pcx_script::ScriptString;

#[derive(Clone, Debug)]
pub struct Binding {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    pub case: SourceWindow,
    pub copy: SourceWindow,
    pub upper_jump: SourceWindow,
    pub lower_jump: SourceWindow,
    pub upper: SourceWindow,
    pub lower: SourceWindow,
    pub substring: SourceWindow,
    pub slice: SourceWindow,
    pub entry: SourceWindow,
    pub startup: SourceWindow,
    pub crt_init: SourceWindow,
    pub callbacks: SourceWindow,
    pub walker: SourceWindow,
    pub locale_init: SourceWindow,
    pub locale_store: SourceWindow,
}
#[derive(Debug)]
pub struct Policy {
    binding: Binding,
    cap: usize,
    upper: [u8; 256],
    lower: [u8; 256],
}
fn at(bytes: &[u8], offset: usize, wanted: &[u8]) -> bool {
    bytes.get(offset..offset.saturating_add(wanted.len())) == Some(wanted)
}
fn word(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset.checked_add(4)?)?.try_into().ok()?,
    ))
}
fn target(bytes: &[u8], va: u32, offset: usize, opcode: u8) -> Option<u32> {
    (bytes.get(offset) == Some(&opcode)).then_some(())?;
    Some(
        va.wrapping_add(offset as u32)
            .wrapping_add(5)
            .wrapping_add(word(bytes, offset + 1)?),
    )
}
fn mapping(bytes: &[u8]) -> Option<[u8; 256]> {
    if !at(bytes, 0, &[0x8b, 0xff, 0x55, 0x8b, 0xec, 0xa1])
        || !at(bytes, 0xa, &[0x85, 0xc0, 0x75, 0x3b])
        || !at(bytes, 0x30, &[0x8a, 0x11, 0x80, 0xfa])
        || !at(bytes, 0x35, &[0x7c, 0x0a, 0x80, 0xfa])
        || !at(bytes, 0x3a, &[0x7f, 0x05, 0x80])
        || !at(
            bytes,
            0x3f,
            &[0x88, 0x11, 0x41, 0x80, 0x39, 0, 0x75, 0xe9, 0x5d, 0xc3],
        )
    {
        return None;
    }
    let low = *bytes.get(0x34)? as i8;
    let high = *bytes.get(0x39)? as i8;
    if low > high {
        return None;
    }
    let operation = *bytes.get(0x3d)?;
    if !matches!(operation, 0xc2 | 0xea) {
        return None;
    }
    let delta = *bytes.get(0x3e)?;
    Some(std::array::from_fn(|index| {
        let byte = index as u8;
        if (low..=high).contains(&(byte as i8)) {
            if operation == 0xc2 {
                byte.wrapping_add(delta)
            } else {
                byte.wrapping_sub(delta)
            }
        } else {
            byte
        }
    }))
}
impl Policy {
    pub fn read(image: &[u8], binding: Binding, budget: RuleInputBudget) -> Result<Self, String> {
        let fail = |why: &str| format!("script strings {}: {why}", binding.origin.source);
        let windows = [
            &binding.case,
            &binding.copy,
            &binding.upper_jump,
            &binding.lower_jump,
            &binding.upper,
            &binding.lower,
            &binding.substring,
            &binding.slice,
            &binding.entry,
            &binding.startup,
            &binding.crt_init,
            &binding.callbacks,
            &binding.walker,
            &binding.locale_init,
            &binding.locale_store,
        ];
        let selected_bytes = windows.iter().map(|w| u64::from(w.len)).sum::<u64>();
        if image.len() > budget.image_bytes || selected_bytes > budget.profile_bytes as u64 {
            return Err(fail("input/selection budget exceeded"));
        }
        if [
            &binding.origin.source,
            &binding.origin.edition,
            &binding.origin.version,
        ]
        .iter()
        .any(|s| s.trim().is_empty())
            || sha256(image) != binding.image_sha256
        {
            return Err(fail(
                "missing identity or complete-image fingerprint mismatch",
            ));
        }
        for (i, left) in windows.iter().enumerate() {
            let end = left
                .va
                .checked_add(left.len)
                .ok_or_else(|| fail("source range overflow"))?;
            if windows[i + 1..].iter().any(|right| {
                right
                    .va
                    .checked_add(right.len)
                    .is_none_or(|end_right| left.va < end_right && right.va < end)
            }) {
                return Err(fail("overlapping source roles"));
            }
        }
        let pe = Pe::parse(image, budget).map_err(|e| fail(&e.to_string()))?;
        let read = |va: u32, len: usize, code| {
            pe.read_from(va, code)
                .map_err(|e| fail(&e.to_string()))?
                .get(..len)
                .ok_or_else(|| fail("truncated source role"))
        };
        for window in windows {
            pe.window(window, true).map_err(|e| fail(&e.to_string()))?;
        }
        let bytes = |w: &SourceWindow| read(w.va, w.len as usize, true);
        let c = bytes(&binding.case)?;
        let copy = bytes(&binding.copy)?;
        let upper = bytes(&binding.upper)?;
        let lower = bytes(&binding.lower)?;
        let sub = bytes(&binding.substring)?;
        let slice = bytes(&binding.slice)?;
        let shape = || -> Option<(usize, [u8; 256], [u8; 256], u32)> {
            if !at(c, 0, &[0x55, 0x8b, 0xec, 0x81, 0xec])
                || !at(c, 0x62, &[0x68])
                || !at(c, 0x67, &[0x8d, 0x85])
                || !at(c, 0x6d, &[0x56, 0x50])
                || target(c, binding.case.va, 0x6f, 0xe8) != Some(binding.copy.va)
                || target(c, binding.case.va, 0x84, 0xe8) != Some(binding.upper_jump.va)
                || target(c, binding.case.va, 0x8b, 0xe8) != Some(binding.lower_jump.va)
                || target(
                    bytes(&binding.upper_jump).ok()?,
                    binding.upper_jump.va,
                    0,
                    0xe9,
                ) != Some(binding.upper.va)
                || target(
                    bytes(&binding.lower_jump).ok()?,
                    binding.lower_jump.va,
                    0,
                    0xe9,
                ) != Some(binding.lower.va)
                || !at(copy, 0x49, &[0x4e, 0x3b, 0xfe, 0x0f, 0x42, 0xf7])
                || !at(copy, 0x5a, &[0xc6, 0x04, 0x1e, 0])
                || !at(
                    c,
                    0x59,
                    &[0x85, 0xf6, 0x74, 0x4a, 0x80, 0x3e, 0, 0x74, 0x45],
                )
                || !at(c, 0x7d, &[0x83, 0x7d, 0x0c, 0, 0x50, 0x74, 7])
            {
                return None;
            }
            let cap = word(c, 0x63)? as usize;
            let stack = word(c, 5)? as usize;
            if !at(c, 0x10, &[0x89, 0x45])
                || (c[0x12] as i8) >= 0
                || cap == 0
                || cap.checked_add(-(c[0x12] as i8 as i32) as usize)? > stack
                || word(c, 0x69)? as i32 != i32::try_from(stack).ok()?.checked_neg()?
            {
                return None;
            }
            let up = mapping(upper)?;
            let down = mapping(lower)?;
            let flag = word(upper, 6)?;
            if word(lower, 6)? != flag {
                return None;
            }
            let raw = target(c, binding.case.va, 0x31, 0xe8)?;
            let intern = target(c, binding.case.va, 0xc0, 0xe8)?;
            if target(sub, binding.substring.va, 0x33, 0xe8) != Some(binding.slice.va)
                || target(sub, binding.substring.va, 0x42, 0xe8) != Some(raw)
                || target(sub, binding.substring.va, 0x64, 0xe8) != Some(intern)
                || !at(
                    slice,
                    0x58,
                    &[0x66, 0x8b, 0x5d, 0x0c, 0x66, 0x3b, 0xde, 0x7f, 0x7e],
                )
                || !at(
                    slice,
                    0x61,
                    &[
                        0x66, 0x39, 0x75, 0x10, 0x0f, 0xb7, 0x45, 0x10, 0x0f, 0xb7, 0xd6, 0x0f,
                        0x4c, 0xd0, 0x0f, 0xbf, 0xcb, 0x0f, 0xb7, 0xc2, 0x03, 0xcf, 0x0f, 0xbf,
                        0xf0,
                    ],
                )
                || !at(slice, 0x81, &[0x8a, 0x1c, 0x3e, 0xc6, 0x04, 0x3e, 0])
                || !at(slice, 0x9d, &[0x51, 0x8d, 0x4d, 0xe4])
                || !at(slice, 0xb8, &[0x88, 0x1c, 0x3e, 0xe8])
            {
                return None;
            }
            Some((cap, up, down, flag))
        };
        let (cap, upper_map, lower_map, flag) = shape().ok_or_else(|| {
            fail("unsupported native copy/case/signed-word slice encoding or call join")
        })?;
        if cap > budget.profile_bytes {
            return Err(fail("native copy capacity exceeds application budget"));
        }
        if !pe.zero_filled(flag, 4) && word(read(flag, 4, false)?, 0) != Some(0) {
            return Err(fail("initial CRT locale flag is not zero"));
        }
        let entry = bytes(&binding.entry)?;
        let startup = bytes(&binding.startup)?;
        let init = bytes(&binding.crt_init)?;
        let callbacks = bytes(&binding.callbacks)?;
        let walker = bytes(&binding.walker)?;
        let locale_init = bytes(&binding.locale_init)?;
        let store = bytes(&binding.locale_store)?;
        let pe_at = word(image, 0x3c).ok_or_else(|| fail("PE header"))? as usize;
        let entry_va =
            word(image, pe_at + 40).and_then(|rva| word(image, pe_at + 52)?.checked_add(rva));
        if entry_va != Some(binding.entry.va)
            || target(entry, binding.entry.va, 5, 0xe9) != Some(binding.startup.va)
            || target(startup, binding.startup.va, 0xe, 0xe8) != Some(binding.crt_init.va)
            || target(init, binding.crt_init.va, 0x22, 0xe8) != Some(binding.callbacks.va)
            || !at(callbacks, 0, &[0x68])
            || !at(callbacks, 5, &[0x68])
            || target(callbacks, binding.callbacks.va, 10, 0xe8) != Some(binding.walker.va)
            || !at(walker, 0x21, &[0x8b, 0x1e, 0x85, 0xdb])
            || !at(walker, 0x2f, &[0xff, 0xd3])
            || !at(walker, 0x35, &[0x83, 0xc6, 8])
            || !at(locale_init, 0, &[0x68])
            || !at(locale_init, 5, &[0xb9])
            || target(locale_init, binding.locale_init.va, 10, 0xe8)
                != Some(binding.locale_store.va)
            || !at(store, 0x1f, &[0x8b, 0x55, 8, 0x46, 0x89, 0x11])
        {
            return Err(fail("unsupported initial C startup join"));
        }
        let slot = word(locale_init, 6).ok_or_else(|| fail("initial locale slot"))?;
        if !pe.zero_filled(slot, 4) && word(read(slot, 4, false)?, 0) != Some(0) {
            return Err(fail("initial locale slot is not zero"));
        }
        let begin = word(callbacks, 6).ok_or_else(|| fail("callback begin"))?;
        let end = word(callbacks, 1).ok_or_else(|| fail("callback end"))?;
        let span = end
            .checked_sub(begin)
            .ok_or_else(|| fail("callback range"))? as usize;
        if span == 0
            || !span.is_multiple_of(8)
            || selected_bytes + span as u64 + 0xb4 + 512 + 8 > budget.profile_bytes as u64
        {
            return Err(fail("initial locale data budget"));
        }
        let pairs = read(begin, span, false)?;
        if word(pairs, 0) != Some(binding.locale_init.va) {
            return Err(fail("initial locale callback is not first"));
        }
        let locale = read(
            word(locale_init, 1).ok_or_else(|| fail("locale address"))?,
            0xb4,
            false,
        )?;
        if word(locale, 8) != Some(0) || word(locale, 0xa8) != Some(0) {
            return Err(fail("non-C locale/codepage profile is not qualified"));
        }
        for (offset, mapping) in [(0x94, &lower_map), (0x98, &upper_map)] {
            let table = read(
                word(locale, offset).ok_or_else(|| fail("locale byte table"))?,
                mapping.len(),
                false,
            )?;
            if table != mapping {
                return Err(fail(
                    "initial C byte table disagrees with the encoded fast branch",
                ));
            }
        }
        Ok(Self {
            binding,
            cap,
            upper: upper_map,
            lower: lower_map,
        })
    }
    pub fn binding(&self) -> &Binding {
        &self.binding
    }
    pub fn case(&self, bytes: &[u8], upper: bool) -> Result<ScriptString, String> {
        let bytes = pcx_script::text::native_c_bytes(bytes);
        let len = bytes.len().min(self.cap - 1);
        let mut output = Vec::new();
        output
            .try_reserve_exact(len)
            .map_err(|_| "StringCase byte allocation failed")?;
        let table = if upper { &self.upper } else { &self.lower };
        output.extend(bytes[..len].iter().map(|b| table[usize::from(*b)]));
        output.truncate(pcx_script::text::native_c_bytes(&output).len());
        Ok(output.into())
    }
    pub fn substring(
        &self,
        bytes: &[u8],
        start: i32,
        end: i32,
    ) -> Result<Option<ScriptString>, String> {
        let bytes = pcx_script::text::native_c_bytes(bytes);
        if bytes.is_empty() {
            return Ok(None);
        }
        // The admitted 66/movsx operands select signed x86 words, including length.
        let start = start as i16;
        let length = bytes.len() as i16;
        if start > length {
            return Ok(None);
        }
        let end = (end as i16).min(length);
        if start < 0 || end < 0 {
            return Err(format!(
                "SubString {}: unsafe effective signed-word pointer offsets start={start}, end={end}, length={length}",
                self.binding.origin.source
            ));
        }
        let start = start as usize;
        let end = if end < start as i16 {
            bytes.len()
        } else {
            end as usize
        };
        let bytes = bytes
            .get(start..end)
            .ok_or("SubString selected offsets exceed the actual byte buffer")?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(bytes.len())
            .map_err(|_| "SubString byte allocation failed")?;
        output.extend_from_slice(bytes);
        Ok(Some(output.into()))
    }
}

#[cfg(test)]
#[path = "script_string_rules_tests.rs"]
mod tests;

//! Raw byte StringHash qualified from the selected client image.
use crate::camera_margins::SourceWindow;
use crate::neutral_player::{RuleInputBudget, Sha256Digest, sha256};
use crate::neutral_rule_input::Pe;
use crate::scripts::AssetIdentity;

#[derive(Clone, Debug)]
pub struct Binding {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    pub handler: SourceWindow,
    pub raw: SourceWindow,
    pub normalize: SourceWindow,
    pub core: SourceWindow,
    pub tails: SourceWindow,
}
#[cfg(test)]
#[path = "script_hash_tests.rs"]
pub(crate) mod tests;
#[derive(Debug, thiserror::Error)]
#[error("script hash {source_context}: {problem}")]
pub struct Error {
    pub source_context: String,
    pub problem: Problem,
}
#[derive(Debug, thiserror::Error)]
pub enum Problem {
    #[error("input or selection budget exceeded")]
    Budget,
    #[error("missing identity or incorrect complete-image fingerprint")]
    Identity,
    #[error("invalid selected image window: {0}")]
    Image(#[source] crate::neutral_player::NeutralError),
    #[error("unsupported selected StringHash encoding or call relationship")]
    Shape,
    #[error("normalized byte allocation failed")]
    Capacity,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Shift {
    left: bool,
    bits: u32,
}
#[derive(Debug)]
pub struct Policy {
    binding: Binding,
    limit: usize,
    lower_start: u8,
    lower_end: u8,
    delta: u8,
    slash: u8,
    replacement: u8,
    golden: u32,
    seed: u32,
    shifts: [Shift; 9],
}
fn word(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}
fn at(bytes: &[u8], offset: usize, wanted: &[u8]) -> bool {
    bytes.get(offset..offset.saturating_add(wanted.len())) == Some(wanted)
}
fn relative(bytes: &[u8], base: u32, offset: usize, opcode: u8) -> Option<u32> {
    (bytes.get(offset) == Some(&opcode)).then_some(())?;
    Some(
        base.wrapping_add(offset as u32)
            .wrapping_add(5)
            .wrapping_add(word(bytes, offset + 1)?),
    )
}
const CHUNK_SHIFTS: [usize; 9] = [0xa7, 0xb8, 0xc3, 0xcc, 0xd9, 0xe4, 0xed, 0xfa, 0x105];
const FINAL_SHIFTS: [usize; 9] = [
    0x197, 0x1a2, 0x1ad, 0x1b8, 0x1c3, 0x1ce, 0x1d9, 0x1e4, 0x1eb,
];
fn shifts(bytes: &[u8], offsets: [usize; 9], registers: [u8; 9]) -> Option<[Shift; 9]> {
    let mut shifts = [Shift {
        left: false,
        bits: 0,
    }; 9];
    for ((out, offset), register) in shifts.iter_mut().zip(offsets).zip(registers) {
        let code = bytes.get(offset..offset + 3)?;
        if code[0] != 0xc1 || (code[1] & 0xc7) != (0xc0 | register) {
            return None;
        }
        out.left = match (code[1] >> 3) & 7 {
            4 => true,
            5 => false,
            _ => return None,
        };
        out.bits = u32::from(code[2] & 31);
    }
    Some(shifts)
}
fn mix_roles(core: &[u8]) -> bool {
    for (index, offset) in FINAL_SHIFTS.into_iter().enumerate() {
        let (prefix, suffix): (&[u8], &[u8]) = match index {
            7 => (&[0x2b, 0xcf, 0x2b, 0xcb, 0x8b, 0xd3], &[0x33, 0xca]),
            8 => (&[0x2b, 0xf9], &[0x2b, 0xfb, 0x33, 0xf9]),
            _ => match index % 3 {
                0 => (&[0x2b, 0xdf, 0x8b, 0xc7, 0x2b, 0xd9], &[0x33, 0xd8]),
                1 => (&[0x2b, 0xcf, 0x2b, 0xcb, 0x8b, 0xc3], &[0x33, 0xc8]),
                _ => (&[0x2b, 0xf9, 0x8b, 0xc1, 0x2b, 0xfb], &[0x33, 0xf8]),
            },
        };
        if !at(core, offset - prefix.len(), prefix) || !at(core, offset + 3, suffix) {
            return false;
        }
    }
    for (index, offset) in CHUNK_SHIFTS.into_iter().enumerate() {
        let (prefix, suffix): (&[u8], &[u8]) = match index {
            0 => (
                &[0x8b, 0xc7],
                &[0x2b, 0xd7, 0x2b, 0xd1, 0x2b, 0xcf, 0x03, 0xda, 0x33, 0xd8],
            ),
            1 => (&[0x2b, 0xcb, 0x8b, 0xc3], &[0x33, 0xc8]),
            3 | 6 => (&[0x2b, 0xdf, 0x8b, 0xc7], &[0x2b, 0xd9, 0x33, 0xd8]),
            4 => (&[0x2b, 0xcf, 0x2b, 0xcb, 0x8b, 0xc3], &[0x33, 0xc8]),
            7 => (&[0x2b, 0xcf, 0x8b, 0xc3, 0x2b, 0xcb], &[0x33, 0xc8]),
            _ => (&[0x2b, 0xf9, 0x8b, 0xc1, 0x2b, 0xfb], &[0x33, 0xf8]),
        };
        if !at(core, offset - prefix.len(), prefix) || !at(core, offset + 3, suffix) {
            return false;
        }
    }
    true
}
impl Policy {
    pub fn read(image: &[u8], binding: Binding, budget: RuleInputBudget) -> Result<Self, Error> {
        let fail = |problem| Error {
            source_context: binding.origin.source.clone(),
            problem,
        };
        let windows = [
            (&binding.handler, true),
            (&binding.raw, true),
            (&binding.normalize, true),
            (&binding.core, true),
            (&binding.tails, false),
        ];
        if image.len() > budget.image_bytes
            || windows.iter().map(|(w, _)| u64::from(w.len)).sum::<u64>()
                > budget.profile_bytes as u64
        {
            return Err(fail(Problem::Budget));
        }
        if [
            &binding.origin.edition,
            &binding.origin.version,
            &binding.origin.source,
        ]
        .iter()
        .any(|s| s.trim().is_empty())
            || sha256(image) != binding.image_sha256
        {
            return Err(fail(Problem::Identity));
        }
        for (index, (left, _)) in windows.iter().enumerate() {
            let end = left
                .va
                .checked_add(left.len)
                .ok_or_else(|| fail(Problem::Shape))?;
            if windows[index + 1..].iter().any(|(right, _)| {
                right
                    .va
                    .checked_add(right.len)
                    .is_none_or(|r| left.va < r && right.va < end)
            }) {
                return Err(fail(Problem::Shape));
            }
        }
        let pe = Pe::parse(image, budget).map_err(|e| fail(Problem::Image(e)))?;
        let open = |w, code| pe.window(w, code).map_err(|e| fail(Problem::Image(e)));
        let handler = open(&binding.handler, true)?;
        let raw = open(&binding.raw, true)?;
        let normalize = open(&binding.normalize, true)?;
        let core = open(&binding.core, true)?;
        let tails = open(&binding.tails, false)?;
        let decoded = (|| {
            if handler.len() != 33
                || raw.len() != 14
                || normalize.len() != 127
                || core.len() != 0x1fb
                || !at(handler, 0, &[0x55, 0x8b, 0xec, 0x8b, 0x4d, 8])
                || relative(handler, binding.handler.va, 6, 0xe8) != Some(binding.raw.va)
                || !at(
                    handler,
                    0xb,
                    &[0x85, 0xc0, 0x74, 0x0e, 0x80, 0x38, 0, 0x74, 9],
                )
                || relative(handler, binding.handler.va, 0x18, 0xe9) != Some(binding.normalize.va)
                || !at(handler, 0x1d, &[0x33, 0xc0, 0x5d, 0xc3])
                || !at(raw, 0, &[0x8b, 0x41])
                || !at(raw, 3, &[0x85, 0xc0, 0x74, 4, 0x8b, 0x40])
                || !at(raw, 10, &[0xc3, 0x33, 0xc0, 0xc3])
                || !at(normalize, 0, &[0x55, 0x8b, 0xec, 0x81, 0xec])
                || !at(normalize, 0x10, &[0x89, 0x45, 0xfc])
                || !at(normalize, 0x17, &[0x33, 0xf6])
                || !at(normalize, 0x1d, &[0x8d, 0xbd])
                || !at(normalize, 0x28, &[0x8d, 0x5e])
                || !at(normalize, 0x33, &[0x81, 0xfe])
                || !at(normalize, 0x3b, &[0x8d, 0x48])
                || !at(normalize, 0x3e, &[0x83, 0xf9])
                || !at(normalize, 0x43, &[0x83, 0xe8])
                || !at(normalize, 0x48, &[0x83, 0xf8])
                || !at(normalize, 0x4b, &[0x0f, 0x44, 0xc3])
                || !at(normalize, 0x5a, &[0x6a])
                || relative(normalize, binding.normalize.va, 0x67, 0xe8) != Some(binding.core.va)
                || !at(
                    core,
                    0,
                    &[0x55, 0x8b, 0xec, 0x83, 0xec, 8, 0x8b, 0x55, 0x0c, 0xb9],
                )
                || !at(core, 0x14, &[0x8b, 0xd9])
                || !at(core, 0x1b, &[0x8b, 0x7d, 0x10])
                || !at(core, 0x1e, &[0x83, 0xfa])
                || !at(core, 0x10a, &[0x83, 0xc6])
                || !at(core, 0x110, &[0x83, 0xe8])
                || !at(core, 0x126, &[0x48, 0x03, 0xfa, 0x83, 0xf8])
                || !at(core, 0x12e, &[0xff, 0x24, 0x85])
                || word(core, 0x131) != Some(binding.tails.va)
                || !at(
                    core,
                    0x1f2,
                    &[0x8b, 0xc7, 0x5f, 0x5e, 0x5b, 0x8b, 0xe5, 0x5d, 0xc3],
                )
            {
                return None;
            }
            let block = *core.get(0x20)?;
            let tail_count = usize::from(*core.get(0x12b)?) + 1;
            if usize::from(block) != 3 * std::mem::size_of::<u32>()
                || *core.get(0x10c)? != block
                || *core.get(0x112)? != block
                || tail_count + 1 != usize::from(block)
                || tails.len() != tail_count * 4
            {
                return None;
            }
            for (index, entry) in tails.as_chunks::<4>().0.iter().enumerate() {
                let target = u32::from_le_bytes(*entry).checked_sub(binding.core.va)? as usize;
                let valid = if index == 0 {
                    at(core, target, &[0x0f, 0xb6, 6])
                } else {
                    at(core, target, &[0x0f, 0xb6, 0x46, index as u8])
                };
                if !valid {
                    return None;
                }
                let mut end = target + if index == 0 { 3 } else { 4 };
                let shift = (index % 4 + usize::from(index >= 8)) * 8;
                if shift != 0 {
                    if !at(core, end, &[0xc1, 0xe0, shift as u8]) {
                        return None;
                    }
                    end += 3;
                }
                let destination = [0xd8, 0xc8, 0xf8][index / 4];
                if !at(core, end, &[0x03, destination]) {
                    return None;
                }
                let next = if index == 0 {
                    0x191
                } else {
                    word(tails, (index - 1) * 4)?.checked_sub(binding.core.va)? as usize
                };
                if end + 2 != next {
                    return None;
                }
            }
            let chunk = shifts(core, CHUNK_SHIFTS, [0, 0, 0, 0, 0, 0, 0, 0, 0])?;
            let final_mix = shifts(core, FINAL_SHIFTS, [0, 0, 0, 0, 0, 0, 0, 2, 1])?;
            if chunk != final_mix || !mix_roles(core) {
                return None;
            }
            let displacement = word(normalize, 0x1f)? as i32;
            let capacity = i32::from(normalize[0x12] as i8).checked_sub(displacement)?;
            let limit = usize::try_from(word(normalize, 0x35)?).ok()?;
            if word(normalize, 5)? != displacement.unsigned_abs()
                || usize::try_from(capacity).ok()?.checked_sub(1)? < limit
                || limit > budget.profile_bytes
            {
                return None;
            }
            let lower_start = u8::try_from(-i32::from(normalize[0x3d] as i8)).ok()?;
            let lower_end = lower_start.checked_add(normalize[0x40])?;
            Some((
                limit,
                lower_start,
                lower_end,
                normalize[0x45],
                normalize[0x4a],
                normalize[0x2a],
                word(core, 0xa)?,
                i32::from(normalize[0x5b] as i8) as u32,
                chunk,
            ))
        })()
        .ok_or_else(|| fail(Problem::Shape))?;
        let (limit, lower_start, lower_end, delta, slash, replacement, golden, seed, shifts) =
            decoded;
        Ok(Self {
            binding,
            limit,
            lower_start,
            lower_end,
            delta,
            slash,
            replacement,
            golden,
            seed,
            shifts,
        })
    }
    pub fn origin(&self) -> &AssetIdentity {
        &self.binding.origin
    }
    pub fn image_sha256(&self) -> &Sha256Digest {
        &self.binding.image_sha256
    }
    pub fn hash(&self, bytes: &[u8]) -> Result<i32, Error> {
        let bytes = pcx_script::text::native_c_bytes(bytes);
        if bytes.is_empty() {
            return Ok(0);
        }
        let bytes = &bytes[..bytes.len().min(self.limit)];
        let mut normalized = Vec::new();
        normalized
            .try_reserve_exact(bytes.len())
            .map_err(|_| Error {
                source_context: self.binding.origin.source.clone(),
                problem: Problem::Capacity,
            })?;
        normalized.extend(bytes.iter().map(|&byte| {
            if (self.lower_start..=self.lower_end).contains(&byte) {
                byte.wrapping_sub(self.delta)
            } else if byte == self.slash {
                self.replacement
            } else {
                byte
            }
        }));
        let mut state = [self.golden, self.golden, self.seed];
        let (chunks, tail) = normalized.as_chunks::<{ 3 * std::mem::size_of::<u32>() }>();
        for chunk in chunks {
            for (lane, word) in state.iter_mut().zip(chunk.as_chunks::<4>().0) {
                *lane = lane.wrapping_add(u32::from_le_bytes(*word));
            }
            self.mix(&mut state);
        }
        state[2] = state[2].wrapping_add(normalized.len() as u32);
        for (index, &byte) in tail.iter().enumerate() {
            let lane = index / 4;
            let shift = (index % 4 + usize::from(lane == 2)) * 8;
            state[lane] = state[lane].wrapping_add(u32::from(byte) << shift);
        }
        self.mix(&mut state);
        Ok(state[2] as i32)
    }
    fn mix(&self, state: &mut [u32; 3]) {
        for (index, shift) in self.shifts.iter().enumerate() {
            let target = index % 3;
            let other = (index + 1) % 3;
            let source = (index + 2) % 3;
            let value = state[source];
            let value = if shift.left {
                value.wrapping_shl(shift.bits)
            } else {
                value.wrapping_shr(shift.bits)
            };
            state[target] = state[target]
                .wrapping_sub(state[other])
                .wrapping_sub(state[source])
                ^ value;
        }
    }
}

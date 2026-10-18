//! Source-bound parameters for the selected script random image.
//!
//! The caller names each reviewed role. A window digest identifies those bytes
//! in one complete local image; it does not establish the role. `read` accepts
//! the window only when its straight-line encoding and direct-call targets
//! match the other windows bound to those roles.
//!
//! Seed lanes are the reviewed unsigned packing of one `i32` argument. Draw
//! lanes contribute their negative index adjustment, positive scale, and
//! rotation. The referenced table is the initialized dword span at the one
//! displacement named by every draw lane. No client address or measured table
//! is embedded here, and this reader does not supply Human01's initial state.
use crate::camera_margins::SourceWindow;
use crate::neutral_player::{NeutralError, RuleInputBudget, Sha256Digest, sha256};
use crate::neutral_rule_input::Pe;
use crate::scripts::AssetIdentity;

/// Reviewed windows in one complete local PE. Addresses belong to that image.
#[derive(Clone, Debug)]
pub struct Binding {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    pub seed: SourceWindow,
    pub draw: SourceWindow,
    pub integer: SourceWindow,
    pub real: SourceWindow,
    pub set_seed: SourceWindow,
    pub secondary: SourceWindow,
    pub real_constant_init: SourceWindow,
    pub math_sub: SourceWindow,
    pub math_add: SourceWindow,
    pub math_mul: SourceWindow,
    pub math_lzcnt: SourceWindow,
    pub math_i32_to_real: SourceWindow,
}

#[derive(Debug, thiserror::Error)]
#[error("random profile {source_context}: {problem}")]
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
    Image(#[source] NeutralError),
    #[error("selected windows overlap or are misaligned")]
    Ambiguous,
    #[error("selected seed packing does not match the reviewed relation")]
    Seed,
    #[error("selected draw does not match the reviewed index and rotation shape")]
    Draw,
    #[error("selected integer wrapper does not call the bound draw")]
    Integer,
    #[error("selected real wrapper does not match the reviewed mask, base or calls")]
    Real,
    #[error("selected seed entry does not call the bound seed and draw")]
    SetSeed,
    #[error("selected secondary initializer does not match the reviewed count")]
    Secondary,
    #[error("selected real initializer does not call the bound conversion")]
    Initializer,
    #[error("selected arithmetic helper does not match its reviewed role")]
    Math,
}

/// Parameters derived from the bound image. The image bytes are not retained.
#[derive(Clone, Debug)]
pub struct Policy {
    origin: AssetIdentity,
    image_sha256: Sha256Digest,
    pub(crate) key: Sha256Digest,
    pub(crate) periods: [u32; 4],
    pub(crate) moduli: [u32; 4],
    pub(crate) packing_shift: u32,
    pub(crate) decrements: [u32; 4],
    pub(crate) rotations: [u32; 4],
    pub(crate) table: Vec<u32>,
    pub(crate) real_mask: u32,
    pub(crate) real_base: u32,
    pub(crate) real_offset: i32,
    pub(crate) real_tolerance: u32,
    pub(crate) secondary_count: usize,
}

struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
    base: u32,
}

impl<'a> Cursor<'a> {
    fn done(self) -> bool {
        self.at == self.bytes.len()
    }
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.at.checked_add(n)?;
        let out = self.bytes.get(self.at..end)?;
        self.at = end;
        Some(out)
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
    fn i8(&mut self) -> Option<i8> {
        Some(i8::from_le_bytes([self.u8()?]))
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn expect(&mut self, prefix: &[u8]) -> Option<()> {
        self.bytes.get(self.at..)?.starts_with(prefix).then(|| {
            self.at += prefix.len();
        })
    }
    fn va(&self) -> u32 {
        self.base.wrapping_add(self.at as u32)
    }
    fn rel32(&mut self) -> Option<u32> {
        let next = self.va().wrapping_add(4);
        let rel = i32::from_le_bytes(self.take(4)?.try_into().ok()?);
        Some(next.wrapping_add(rel as u32))
    }
    fn branch8(&mut self, opcode: u8) -> Option<u32> {
        self.expect(&[opcode])?;
        let next = self.va().wrapping_add(1);
        Some(next.wrapping_add(self.i8()? as u32))
    }
    fn call(&mut self) -> Option<u32> {
        self.expect(&[0xe8])?;
        self.rel32()
    }
}

fn charge(selected: &mut usize, bytes: usize, limit: usize) -> bool {
    match selected.checked_add(bytes) {
        Some(next) if next <= limit => {
            *selected = next;
            true
        }
        _ => false,
    }
}

fn overlap(windows: &[&SourceWindow]) -> bool {
    let mut spans = Vec::new();
    for window in windows {
        let Some(end) = window.va.checked_add(window.len) else {
            return true;
        };
        spans.push((window.va, end));
    }
    spans.sort_by_key(|span| span.0);
    spans.windows(2).any(|pair| pair[0].1 > pair[1].0)
}

// For a fixed remainder the reciprocal error grows with the quotient. Checking
// its largest u32 input therefore establishes the entire residue class.
fn division(multiplier: u64, shift: u32, divisor: u32) -> bool {
    if divisor == 0 || divisor > 64 || shift >= 128 {
        return false;
    }
    let denominator = 1u128 << shift;
    if u128::from(multiplier) != denominator.div_ceil(u128::from(divisor)) {
        return false;
    }
    (0..divisor).all(|remainder| {
        let largest = u32::MAX - (u32::MAX - remainder) % divisor;
        (u128::from(largest) * u128::from(multiplier)) / denominator
            == u128::from(largest / divisor)
    })
}

fn seed_shape(bytes: &[u8], base: u32) -> Option<([u32; 4], u32)> {
    let mut c = Cursor { bytes, at: 0, base };
    c.expect(&[0x55, 0x8b, 0xec, 0x53, 0x56, 0x57, 0x8b, 0x7d, 8, 0xb8])?;
    let m0 = c.u32()?;
    c.expect(&[0xf7, 0xe7, 0x8b, 0xf7, 0x8b, 0xd9, 0xc1, 0xea])?;
    let q0 = u32::from(c.u8()?);
    c.expect(&[0x8b, 0xcf, 0x6b, 0xc2])?;
    let d0 = u32::try_from(c.i8()?).ok()?;
    c.expect(&[0x89, 0x3b, 0x2b, 0xf0, 0xb8])?;
    let m1 = c.u32()?;
    c.expect(&[0xf7, 0xe7, 0xc1, 0xe6])?;
    let p0 = u32::from(c.u8()?);
    c.expect(&[0xc1, 0xea])?;
    let q1 = u32::from(c.u8()?);
    c.expect(&[0x6b, 0xc2])?;
    let d1 = u32::try_from(c.i8()?).ok()?;
    c.expect(&[0x2b, 0xc8, 0xb8])?;
    let m2 = c.u32()?;
    c.expect(&[0xf7, 0xe7, 0x8b, 0xc7, 0xc1, 0xe1])?;
    let p1 = u32::from(c.u8()?);
    c.expect(&[
        0x2b, 0xc2, 0x0b, 0xf1, 0xd1, 0xe8, 0x8b, 0xcf, 0x03, 0xc2, 0xc1, 0xe8,
    ])?;
    let q2 = u32::from(c.u8()?);
    c.expect(&[0x6b, 0xc0])?;
    let d2 = u32::try_from(c.i8()?).ok()?;
    c.expect(&[0x2b, 0xc8, 0xb8])?;
    let m3 = c.u32()?;
    c.expect(&[0xf7, 0xe7, 0xc1, 0xe1])?;
    let p2 = u32::from(c.u8()?);
    c.expect(&[0xc1, 0xea])?;
    let q3 = u32::from(c.u8()?);
    c.expect(&[0x0b, 0xf1, 0x8b, 0xc2, 0xc1, 0xe0])?;
    let coefficient_shift = u32::from(c.u8()?);
    c.expect(&[0x03, 0xc7, 0x03, 0xc2, 0xc1, 0xe0])?;
    let p3 = u32::from(c.u8()?);
    c.expect(&[
        0x0b, 0xf0, 0x5f, 0x89, 0x73, 4, 0x5e, 0x5b, 0x5d, 0xc2, 4, 0,
    ])?;
    if !c.done() || [q0, q1, q2, q3, coefficient_shift].iter().any(|v| *v >= 32) {
        return None;
    }
    let packing_shift = p0 % 8;
    if !(2..8).contains(&packing_shift) || !(24..32).contains(&p3) {
        return None;
    }
    let coefficient = (1u64 << coefficient_shift) + 1;
    let capacity = 1u32 << (32 - p3);
    let d3 = (1..=capacity.min(64)).find(|d| {
        division(u64::from(m3), 32 + q3, *d)
            && (coefficient + u64::from(*d)).is_multiple_of(u64::from(capacity))
    })?;
    if !division(u64::from(m0), 32 + q0, d0)
        || !division(u64::from(m1), 32 + q1, d1)
        || !division((1u64 << 32) + u64::from(m2), 33 + q2, d2)
    {
        return None;
    }
    let mut moduli = [0; 4];
    for (shift, divisor) in [p0, p1, p2, p3].into_iter().zip([d0, d1, d2, d3]) {
        if shift >= 32 || shift % 8 != packing_shift || divisor > (1 << (8 - packing_shift)) {
            return None;
        }
        let lane = usize::try_from(shift / 8).ok()?;
        if moduli[lane] != 0 {
            return None;
        }
        moduli[lane] = divisor;
    }
    moduli
        .iter()
        .all(|d| *d != 0)
        .then_some((moduli, packing_shift))
}

fn negative_byte(cursor: &mut Cursor<'_>, modrm: u8) -> Option<u32> {
    cursor.expect(&[0x83, modrm])?;
    let value = cursor.i8()?;
    (value < 0).then_some(u32::from(value.unsigned_abs()))
}

fn scale_disp(cursor: &mut Cursor<'_>, opcode: u8, modrm: u8) -> Option<u32> {
    cursor.expect(&[opcode, modrm])?;
    let disp = cursor.u32()?;
    (disp != 0).then_some(disp)
}

fn rot(cursor: &mut Cursor<'_>, reg: u8) -> Option<u32> {
    if cursor.expect(&[0xd1, 0xc0 + reg]).is_some() {
        return Some(1);
    }
    cursor.expect(&[0xc1, 0xc0 + reg])?;
    let count = cursor.u8()?;
    (count != 0 && count < 32).then_some(u32::from(count))
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Lanes {
    table: u32,
    periods: [u32; 4],
    decrements: [u32; 4],
    rotations: [u32; 4],
}
fn lanes(c: &mut Cursor<'_>) -> Option<Lanes> {
    c.expect(&[0x8b, 0xcb, 0xc1, 0xe9, 24, 0x8b, 0xc3, 0xc1, 0xe8, 16])?;
    let top_delta = negative_byte(c, 0xc1)?;
    c.expect(&[0x0f, 0xb6, 0xd0])?;
    let upper_delta = negative_byte(c, 0xc2)?;
    c.expect(&[0x85, 0xc9])?;
    let top_period = scale_disp(c, 0x8d, 0x81)?;
    c.expect(&[0x0f, 0x49, 0xc1, 0x89, 0x45])?;
    let temporary = c.u8()?;
    let upper_period = scale_disp(c, 0x8d, 0xba)?;
    c.expect(&[0x8b, 0xc3, 0xc1, 0xe8, 8, 0x0f, 0xb6, 0xc8])?;
    let lower_delta = negative_byte(c, 0xc1)?;
    c.expect(&[
        0x0f, 0xb6, 0xc3, 0x8b, 0x5d, temporary, 0x85, 0xd2, 0x0f, 0x49, 0xfa,
    ])?;
    let bottom_delta = negative_byte(c, 0xc0)?;
    c.expect(&[0x85, 0xc9])?;
    let lower_period = scale_disp(c, 0x8d, 0xb1)?;
    c.expect(&[0x0f, 0x49, 0xf1, 0x85, 0xc0])?;
    let bottom_period = scale_disp(c, 0x8d, 0x90)?;
    c.expect(&[0x0f, 0x49, 0xd0])?;
    let table = scale_disp(c, 0x8b, 0x87)?;
    let upper_rotation = rot(c, 0)?;
    let lower_table = scale_disp(c, 0x8b, 0x8e)?;
    let lower_rotation = rot(c, 1)?;
    c.expect(&[0x33, 0xc8])?;
    let top_table = scale_disp(c, 0x8b, 0x83)?;
    c.expect(&[0xc1, 0xe3, 8, 0x0b, 0xdf])?;
    let top_rotation = rot(c, 0)?;
    if c.expect(&[0x33, 0xc8, 0xc1, 0xe3, 8]).is_none() {
        c.expect(&[0xc1, 0xe3, 8, 0x33, 0xc8])?;
    }
    (table == lower_table && table == top_table).then_some(Lanes {
        table,
        periods: [bottom_period, lower_period, upper_period, top_period],
        decrements: [bottom_delta, lower_delta, upper_delta, top_delta],
        rotations: [0, lower_rotation, upper_rotation, top_rotation],
    })
}
fn draw_shape(bytes: &[u8], base: u32) -> Option<Lanes> {
    let mut c = Cursor { bytes, at: 0, base };
    c.expect(&[
        0x55, 0x8b, 0xec, 0x51, 0x53, 0x8b, 0x5d, 8, 0x56, 0x57, 0x8b, 0x5b, 4,
    ])?;
    let lane = lanes(&mut c)?;
    c.expect(&[0x8b, 0x45, 8, 0x0b, 0xde])?;
    let table = scale_disp(&mut c, 0x33, 0x8a)?;
    c.expect(&[
        0xc1, 0xe3, 8, 0x5f, 0x03, 0x08, 0x0b, 0xda, 0x89, 0x58, 4, 0x5e, 0x89, 0x08, 0x8b, 0xc1,
        0x5b, 0x8b, 0xe5, 0x5d, 0xc3,
    ])?;
    (c.done() && table == lane.table).then_some(lane)
}

fn integer_shape(bytes: &[u8], base: u32) -> Option<(u32, u32)> {
    let mut cursor = Cursor { bytes, at: 0, base };
    cursor.expect(&[
        0x55, 0x8b, 0xec, 0x8b, 0x45, 0x0c, 0x56, 0x8b, 0x75, 0x08, 0x3b, 0xf0, 0x75, 0x05, 0x8b,
        0xc6, 0x5e, 0x5d, 0xc3, 0x57, 0x3b, 0xf0, 0x7e, 0x06, 0x8b, 0xfe, 0x2b, 0xf8, 0xeb, 0x04,
        0x8b, 0xf8, 0x2b, 0xfe, 0xff, 0x35,
    ])?;
    let state = cursor.u32()?;
    let draw = cursor.call()?;
    cursor.expect(&[
        0x83, 0xc4, 0x04, 0x8d, 0x4f, 0x01, 0xf7, 0xe1, 0x5f, 0x8d, 0x04, 0x16, 0x5e, 0x5d, 0xc3,
    ])?;
    cursor.done().then_some((state, draw))
}

fn real_shape(bytes: &[u8], base: u32) -> Option<(u32, u32, u32, u32, u32, [u32; 6])> {
    let mut c = Cursor { bytes, at: 0, base };
    c.expect(&[
        0x55, 0x8b, 0xec, 0x83, 0xec, 20, 0x56, 0x8b, 0x75, 8, 0x8d, 0x45, 0xf8, 0x57, 0x8b, 0x7d,
        12, 0x57, 0x56, 0x50,
    ])?;
    let first_sub = c.call()?;
    c.expect(&[0xf3, 0x0f, 0x10, 0x05])?;
    let tolerance = c.u32()?;
    c.expect(&[
        0x83, 0xc4, 12, 0x8b, 0x08, 0x81, 0xe1, 0xff, 0xff, 0xff, 0x7f, 0x89, 0x4d, 0xfc, 0x0f,
        0x2f, 0x45, 0xfc, 0x76,
    ])?;
    let next = c.va().wrapping_add(1);
    let ordinary = next.wrapping_add(c.i8()? as u32);
    c.expect(&[0x8b, 0x06, 0x5f, 0x5e, 0x8b, 0xe5, 0x5d, 0xc3])?;
    if ordinary != c.va() {
        return None;
    }
    c.expect(&[0xf3, 0x0f, 0x10, 0x06, 0x0f, 0x2f, 0x07])?;
    let otherwise = c.branch8(0x76)?;
    c.expect(&[0x57, 0x56, 0x8d, 0x45, 0xf8])?;
    let join = c.branch8(0xeb)?;
    if c.va() != otherwise {
        return None;
    }
    c.expect(&[0x56, 0x57, 0x8d, 0x45, 0xfc])?;
    if c.va() != join {
        return None;
    }
    c.expect(&[0x50])?;
    let span_sub = c.call()?;
    c.expect(&[0x83, 0xc4, 12, 0x8b, 0x00, 0xff, 0x35])?;
    let state = c.u32()?;
    c.expect(&[0x89, 0x45, 0xf4])?;
    let draw = c.call()?;
    c.expect(&[0x25])?;
    let mask = c.u32()?;
    c.expect(&[0x0d])?;
    let fraction_base = c.u32()?;
    c.expect(&[0x89, 0x45, 0xfc, 0x8d, 0x45, 0xfc, 0x68])?;
    let slot = c.u32()?;
    c.expect(&[0x50, 0x8d, 0x45, 0xf8, 0x50])?;
    let offset_add = c.call()?;
    c.expect(&[
        0x8d, 0x45, 0xf8, 0x50, 0x8d, 0x45, 0xf4, 0x50, 0x8d, 0x45, 0xf0, 0x50,
    ])?;
    let mul = c.call()?;
    c.expect(&[0x50, 0x8d, 0x45, 0xec, 0x56, 0x50])?;
    let result_add = c.call()?;
    c.expect(&[
        0x83, 0xc4, 40, 0x8b, 0x00, 0x5f, 0x5e, 0x8b, 0xe5, 0x5d, 0xc3,
    ])?;
    c.done().then_some((
        mask,
        fraction_base,
        slot,
        tolerance,
        state,
        [first_sub, span_sub, draw, offset_add, mul, result_add],
    ))
}

fn set_seed_shape(bytes: &[u8], base: u32) -> Option<(u32, u32, u32, u32)> {
    let mut cursor = Cursor { bytes, at: 0, base };
    cursor.expect(&[0x55, 0x8b, 0xec, 0x56, 0x8b, 0x35])?;
    let state = cursor.u32()?;
    cursor.expect(&[0x8b, 0xce, 0xff, 0x75, 0x08])?;
    let seed = cursor.call()?;
    cursor.expect(&[0x56])?;
    let draw = cursor.call()?;
    cursor.expect(&[0x83, 0xc4, 0x04, 0x5e, 0x89, 0x45, 0x08, 0x5d, 0xe9])?;
    let secondary = cursor.rel32()?;
    cursor.done().then_some((state, seed, draw, secondary))
}

fn secondary_shape(bytes: &[u8], base: u32, seed: u32, expected: &Lanes) -> Option<usize> {
    let mut c = Cursor { bytes, at: 0, base };
    c.expect(&[0x55, 0x8b, 0xec, 0x83, 0xec, 20, 0x56, 0x6a])?;
    c.u8()?;
    c.call()?;
    c.expect(&[
        0x83, 0xc4, 4, 0x8b, 0x40, 12, 0x8b, 0x70, 12, 0x85, 0xf6, 0x75,
    ])?;
    let next = c.va().wrapping_add(1);
    let present = next.wrapping_add(c.i8()? as u32);
    c.expect(&[0x6a])?;
    c.u8()?;
    c.call()?;
    c.expect(&[0x83, 0xc4, 4, 0x5e, 0x8b, 0xe5, 0x5d, 0xc3])?;
    if c.va() != present {
        return None;
    }
    c.expect(&[0xff, 0x75, 8, 0x8d, 0x4d, 0xec])?;
    if c.call()? != seed {
        return None;
    }
    c.expect(&[0x83, 0xc6, 4, 0xc7, 0x45, 0xf8])?;
    let count = usize::try_from(c.u32()?).ok()?;
    c.expect(&[0x53, 0x89, 0x75, 0xfc, 0x57])?;
    let loop_start = c.va();
    c.expect(&[0x8b, 0x5d, 0xf0])?;
    let local = lanes(&mut c)?;
    let table = scale_disp(&mut c, 0x33, 0x8a)?;
    c.expect(&[
        0x0b, 0xde, 0x03, 0x4d, 0xec, 0xc1, 0xe3, 8, 0x0b, 0xda, 0x89, 0x4d, 0xec, 0x89, 0x5d,
        0xf0, 0x8b, 0x5d, 0xfc, 0x51, 0x8b, 0xcb,
    ])?;
    if c.call()? != seed {
        return None;
    }
    c.expect(&[
        0x83, 0xc3, 8, 0x83, 0x6d, 0xf8, 1, 0x89, 0x5d, 0xfc, 0x0f, 0x85,
    ])?;
    if c.rel32()? != loop_start {
        return None;
    }
    c.expect(&[0x5f, 0x5b, 0x5e, 0x8b, 0xe5, 0x5d, 0xc3])?;
    (c.done() && count != 0 && table == expected.table && &local == expected).then_some(count)
}

fn initializer_shape(bytes: &[u8], base: u32) -> Option<(i32, u32, u32)> {
    let mut cursor = Cursor { bytes, at: 0, base };
    cursor.expect(&[0x6a])?;
    let offset = i32::from(cursor.i8()?);
    cursor.expect(&[0x68])?;
    let slot = cursor.u32()?;
    let target = cursor.call()?;
    cursor.expect(&[0x83, 0xc4, 0x08, 0xc3])?;
    (cursor.done() && offset != 0 && slot != 0).then_some((offset, slot, target))
}

fn has(bytes: &[u8], prefix: &[u8]) -> bool {
    bytes.windows(prefix.len()).any(|window| window == prefix)
}

fn stores_result(bytes: &[u8]) -> bool {
    bytes.first() == Some(&0x55)
        && bytes.ends_with(&[0xc3])
        && (has(bytes, &[0x89, 0x01]) || has(bytes, &[0x89, 0x10]) || has(bytes, &[0x89, 0x08]))
}

fn i32_shape(bytes: &[u8], base: u32) -> Option<u32> {
    let mut cursor = Cursor { bytes, at: 0, base };
    cursor.expect(&[
        0x55, 0x8b, 0xec, 0x8b, 0x45, 0x0c, 0x85, 0xc0, 0x75, 0x0b, 0x8b, 0x45, 0x08, 0xc7, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x5d, 0xc3, 0x56, 0x57, 0x8b, 0xf8, 0x99, 0x8b, 0xf0, 0x81, 0xe7,
        0x00, 0x00, 0x00, 0x80, 0x33, 0xf2, 0x2b, 0xf2, 0x8b, 0xce,
    ])?;
    let scan = cursor.call()?;
    bytes
        .ends_with(&[0x89, 0x08, 0x5e, 0x5d, 0xc3])
        .then_some(scan)
}

pub(crate) fn arithmetic_shapes(
    sub: &[u8],
    add: &[u8],
    mul: &[u8],
    scan: &[u8],
    convert: &[u8],
    convert_va: u32,
    scan_va: u32,
) -> bool {
    !(!stores_result(sub)
        || !sub.starts_with(&[0x55, 0x8b, 0xec, 0x8b, 0x45, 16, 0x8b, 0x4d, 12])
        || !has(sub, &[0x35, 0, 0, 0, 0x80])
        || !stores_result(add)
        || !add.starts_with(&[
            0x55, 0x8b, 0xec, 0x8b, 0x45, 12, 0x53, 0x56, 0x57, 0x8b, 0x08,
        ])
        || !has(add, &[0x03, 0xf6])
        || !has(add, &[0x03, 0xd2])
        || !stores_result(mul)
        || !has(mul, &[0xf7, 0xe7])
        || !scan.starts_with(&[0x55, 0x8b, 0xec, 0x51, 0x89, 0x4d, 0xfc, 0x0f, 0xbd])
        || !scan.ends_with(&[0xc3])
        || i32_shape(convert, convert_va) != Some(scan_va))
}

fn read_table(
    pe: &Pe<'_>,
    displacement: u32,
    length: usize,
    selected: &mut usize,
    limit: usize,
) -> Result<Vec<u32>, Problem> {
    let bytes = pe
        .read_from(displacement, false)
        .map_err(|_| Problem::Draw)?;
    let bytes = bytes.get(..length).ok_or(Problem::Draw)?;
    if !charge(selected, length, limit) {
        return Err(Problem::Budget);
    }
    let mut table = Vec::new();
    table
        .try_reserve_exact(bytes.len() / 4)
        .map_err(|_| Problem::Budget)?;
    for chunk in bytes.as_chunks::<4>().0 {
        table.push(u32::from_le_bytes(*chunk));
    }
    Ok(table)
}

impl Policy {
    /// Charge every selected window before hashing the image or allocating.
    /// A direct call is accepted only when it lands on the bound role.
    pub fn read(image: &[u8], binding: Binding, budget: RuleInputBudget) -> Result<Self, Error> {
        let fail = |problem| Error {
            source_context: binding.origin.source.clone(),
            problem,
        };
        if image.len() > budget.image_bytes || budget.profile_bytes == 0 || budget.sections == 0 {
            return Err(fail(Problem::Budget));
        }
        let selected_windows = [
            &binding.seed,
            &binding.draw,
            &binding.integer,
            &binding.real,
            &binding.set_seed,
            &binding.secondary,
            &binding.real_constant_init,
            &binding.math_sub,
            &binding.math_add,
            &binding.math_mul,
            &binding.math_lzcnt,
            &binding.math_i32_to_real,
        ];
        let mut selected = 0usize;
        for window in selected_windows {
            if window.len == 0 {
                return Err(fail(Problem::Budget));
            }
            if !charge(&mut selected, window.len as usize, budget.profile_bytes) {
                return Err(fail(Problem::Budget));
            }
        }
        if [
            &binding.origin.edition,
            &binding.origin.version,
            &binding.origin.source,
        ]
        .iter()
        .any(|text| text.trim().is_empty())
            || sha256(image) != binding.image_sha256
        {
            return Err(fail(Problem::Identity));
        }
        if overlap(&selected_windows) {
            return Err(fail(Problem::Ambiguous));
        }
        let pe = Pe::parse(image, budget).map_err(|error| fail(Problem::Image(error)))?;
        let open = |window: &SourceWindow| {
            pe.window(window, true)
                .map_err(|error| fail(Problem::Image(error)))
        };
        let seed = open(&binding.seed)?;
        let draw = open(&binding.draw)?;
        let integer = open(&binding.integer)?;
        let real = open(&binding.real)?;
        let set_seed = open(&binding.set_seed)?;
        let secondary = open(&binding.secondary)?;
        let initializer = open(&binding.real_constant_init)?;
        let sub = open(&binding.math_sub)?;
        let add = open(&binding.math_add)?;
        let mul = open(&binding.math_mul)?;
        let scan = open(&binding.math_lzcnt)?;
        let convert = open(&binding.math_i32_to_real)?;

        let (moduli, packing_shift) =
            seed_shape(seed, binding.seed.va).ok_or_else(|| fail(Problem::Seed))?;
        let lane = draw_shape(draw, binding.draw.va).ok_or_else(|| fail(Problem::Draw))?;
        for (i, modulus) in moduli.iter().enumerate() {
            if modulus.checked_shl(packing_shift) != Some(lane.periods[i])
                || lane.periods[i] > 256
                || lane.decrements[i] == 0
                || lane.decrements[i] > lane.periods[i]
                || lane.decrements[i] % 4 != 0
            {
                return Err(fail(Problem::Ambiguous));
            }
        }
        let (integer_state, integer_draw) =
            integer_shape(integer, binding.integer.va).ok_or_else(|| fail(Problem::Integer))?;
        if integer_draw != binding.draw.va {
            return Err(fail(Problem::Integer));
        }
        let (real_mask, real_base, real_slot, tolerance_address, real_state, real_calls) =
            real_shape(real, binding.real.va).ok_or_else(|| fail(Problem::Real))?;
        let real_tolerance_bytes = pe
            .read_from(tolerance_address, false)
            .map_err(|e| fail(Problem::Image(e)))?;
        let real_tolerance = u32::from_le_bytes(
            real_tolerance_bytes
                .get(..4)
                .ok_or_else(|| fail(Problem::Real))?
                .try_into()
                .unwrap(),
        );
        if !charge(&mut selected, 4, budget.profile_bytes)
            || !f32::from_bits(real_tolerance).is_finite()
            || real_tolerance & 0x8000_0000 != 0
            || real_mask & !0x7f_ffff != 0
            || super::real::from_f32(f32::from_bits(real_base)).is_err()
        {
            return Err(fail(Problem::Real));
        }
        let (real_offset, init_slot, init_target) =
            initializer_shape(initializer, binding.real_constant_init.va)
                .ok_or_else(|| fail(Problem::Initializer))?;
        if real_calls
            != [
                binding.math_sub.va,
                binding.math_sub.va,
                binding.draw.va,
                binding.math_add.va,
                binding.math_mul.va,
                binding.math_add.va,
            ]
            || real_slot != init_slot
            || init_target != binding.math_i32_to_real.va
        {
            return Err(fail(Problem::Real));
        }
        let (seed_state, seed_target, draw_target, secondary_target) =
            set_seed_shape(set_seed, binding.set_seed.va).ok_or_else(|| fail(Problem::SetSeed))?;
        if seed_target != binding.seed.va
            || draw_target != binding.draw.va
            || secondary_target != binding.secondary.va
            || seed_state != integer_state
            || seed_state != real_state
        {
            return Err(fail(Problem::SetSeed));
        }
        let secondary_count =
            secondary_shape(secondary, binding.secondary.va, binding.seed.va, &lane)
                .ok_or_else(|| fail(Problem::Secondary))?;
        if !arithmetic_shapes(
            sub,
            add,
            mul,
            scan,
            convert,
            binding.math_i32_to_real.va,
            binding.math_lzcnt.va,
        ) {
            return Err(fail(Problem::Math));
        }
        let length = usize::try_from(*lane.periods.iter().max().unwrap())
            .map_err(|_| fail(Problem::Budget))?;
        let table = read_table(&pe, lane.table, length, &mut selected, budget.profile_bytes)
            .map_err(fail)?;
        let mut fingerprint = Vec::new();
        fingerprint
            .try_reserve_exact(32 + selected_windows.len() * 40)
            .map_err(|_| fail(Problem::Budget))?;
        fingerprint.extend(binding.image_sha256);
        for window in selected_windows {
            fingerprint.extend(window.va.to_le_bytes());
            fingerprint.extend(window.len.to_le_bytes());
            fingerprint.extend(window.digest);
        }
        Ok(Self {
            key: sha256(&fingerprint),
            origin: binding.origin,
            image_sha256: binding.image_sha256,
            periods: lane.periods,
            moduli,
            packing_shift,
            decrements: lane.decrements,
            rotations: lane.rotations,
            table,
            real_mask,
            real_base,
            real_offset,
            real_tolerance,
            secondary_count,
        })
    }

    pub fn origin(&self) -> &AssetIdentity {
        &self.origin
    }
    pub fn image_sha256(&self) -> &Sha256Digest {
        &self.image_sha256
    }
}

#[cfg(test)]
pub(crate) fn fixture() -> Policy {
    Policy {
        origin: AssetIdentity {
            edition: "synthetic".into(),
            version: "fixture".into(),
            source: "self-authored RNG".into(),
        },
        image_sha256: [0; 32],
        key: [1; 32],
        periods: [40, 24, 56, 120],
        moduli: [5, 3, 7, 15],
        packing_shift: 3,
        decrements: [4, 8, 12, 16],
        rotations: [0, 7, 11, 1],
        table: (0..30).map(|i| i * 17 + 0x1357_9bdf).collect(),
        real_mask: 0x003f_ffff,
        real_base: 1.0f32.to_bits(),
        real_offset: -1,
        real_tolerance: 0.001f32.to_bits(),
        secondary_count: 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Self-authored x86 encoding with moduli 5/3/7/15 and 3-bit byte packing.
    fn seed() -> Vec<u8> {
        let mut b = Vec::new();
        let hex = |text: &str| {
            text.split_whitespace()
                .map(|s| u8::from_str_radix(s, 16).unwrap())
                .collect::<Vec<_>>()
        };
        b.extend(hex("55 8b ec 53 56 57 8b 7d 08 b8"));
        b.extend(3435973837u32.to_le_bytes());
        b.extend(hex(
            "f7 e7 8b f7 8b d9 c1 ea 02 8b cf 6b c2 05 89 3b 2b f0 b8",
        ));
        b.extend(2863311531u32.to_le_bytes());
        b.extend(hex("f7 e7 c1 e6 0b c1 ea 01 6b c2 03 2b c8 b8"));
        b.extend(613566757u32.to_le_bytes());
        b.extend(hex(
            "f7 e7 8b c7 c1 e1 03 2b c2 0b f1 d1 e8 8b cf 03 c2 c1 e8 02 6b c0 07 2b c8 b8",
        ));
        b.extend(2290649225u32.to_le_bytes());
        b.extend(hex("f7 e7 c1 e1 13 c1 ea 03 0b f1 8b c2 c1 e0 04 03 c7 03 c2 c1 e0 1b 0b f0 5f 89 73 04 5e 5b 5d c2 04 00"));
        b
    }

    #[test]
    fn packing_is_derived_and_malformed_shifts_never_panic() {
        let bytes = seed();
        assert_eq!(seed_shape(&bytes, 0x1000), Some(([3, 5, 7, 15], 3)));
        for end in 0..bytes.len() {
            assert!(seed_shape(&bytes[..end], 0x1000).is_none());
        }
        for at in 0..bytes.len() {
            for value in [0, 1, 31, 32, 127, 255] {
                let mut changed = bytes.clone();
                changed[at] = value;
                let _ = seed_shape(&changed, 0x1000);
            }
        }
        let mut extra = bytes;
        extra.push(0);
        assert!(seed_shape(&extra, 0x1000).is_none());
    }

    #[test]
    fn reciprocal_must_cover_entire_unsigned_domain() {
        assert!(division(3435973837, 34, 5));
        assert!(!division(3435973838, 34, 5));
        assert!(!division(1, 0, 0));
        assert!(!division(1, 128, 1));
    }
}

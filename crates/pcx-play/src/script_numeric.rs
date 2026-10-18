//! Selected script literal and real-operator semantics, with image-derived data.
//! Unqualified number classes and literal spellings refuse explicitly.
use crate::camera_margins::SourceWindow;
use crate::neutral_player::{NeutralError, RuleInputBudget, Sha256Digest, sha256};
use crate::neutral_rule_input::Pe;
use crate::script_random::{profile::arithmetic_shapes, real};
use crate::scripts::AssetIdentity;
use pcx_script::{RealOps, Value, ast::BinOp};

#[derive(Clone, Debug)]
pub struct Binding {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    pub literal: SourceWindow,
    pub division: SourceWindow,
    pub reciprocal: SourceWindow,
    pub reciprocal_table: SourceWindow,
    pub equality: SourceWindow,
    pub equality_constant: SourceWindow,
    pub inequality: SourceWindow,
    pub math_sub: SourceWindow,
    pub math_add: SourceWindow,
    pub math_mul: SourceWindow,
    pub math_lzcnt: SourceWindow,
    pub math_i32_to_real: SourceWindow,
}

#[derive(Debug, thiserror::Error)]
#[error("script numeric {source_context}: {problem}")]
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
    #[error("unsupported selected numeric encoding or call relationship")]
    Shape,
}

#[derive(Debug)]
pub struct NumericPolicy {
    binding: Binding,
    table: Vec<u32>,
    index_shift: u32,
    equality_threshold: f32,
    literal_bytes: usize,
}

fn word(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}
fn call(bytes: &[u8], base: u32, at: usize) -> Option<u32> {
    (bytes.get(at) == Some(&0xe8)).then_some(())?;
    Some(
        base.wrapping_add(at as u32)
            .wrapping_add(5)
            .wrapping_add(word(bytes, at + 1)?),
    )
}
fn at(bytes: &[u8], offset: usize, expected: &[u8]) -> bool {
    bytes.get(offset..offset.saturating_add(expected.len())) == Some(expected)
}

impl NumericPolicy {
    /// Bind reviewed roles in one complete image. `literal_bytes` bounds each conversion.
    pub fn read(
        image: &[u8],
        binding: Binding,
        budget: RuleInputBudget,
        literal_bytes: usize,
    ) -> Result<Self, Error> {
        let fail = |problem| Error {
            source_context: binding.origin.source.clone(),
            problem,
        };
        let windows = [
            &binding.literal,
            &binding.division,
            &binding.reciprocal,
            &binding.reciprocal_table,
            &binding.equality,
            &binding.equality_constant,
            &binding.inequality,
            &binding.math_sub,
            &binding.math_add,
            &binding.math_mul,
            &binding.math_lzcnt,
            &binding.math_i32_to_real,
        ];
        if literal_bytes == 0
            || image.len() > budget.image_bytes
            || windows.iter().map(|w| u64::from(w.len)).sum::<u64>() > budget.profile_bytes as u64
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
        for (index, left) in windows.iter().enumerate() {
            let Some(end) = left.va.checked_add(left.len) else {
                return Err(fail(Problem::Shape));
            };
            if windows[index + 1..].iter().any(|right| {
                right
                    .va
                    .checked_add(right.len)
                    .is_none_or(|right_end| left.va < right_end && right.va < end)
            }) {
                return Err(fail(Problem::Shape));
            }
        }
        let pe = Pe::parse(image, budget).map_err(|e| fail(Problem::Image(e)))?;
        let open = |window: &SourceWindow, code| {
            pe.window(window, code).map_err(|e| fail(Problem::Image(e)))
        };
        let literal = open(&binding.literal, true)?;
        let division = open(&binding.division, true)?;
        let reciprocal = open(&binding.reciprocal, true)?;
        let equality = open(&binding.equality, true)?;
        let inequality = open(&binding.inequality, true)?;
        let constant = open(&binding.equality_constant, false)?;
        let table = open(&binding.reciprocal_table, false)?;
        if !arithmetic_shapes(
            open(&binding.math_sub, true)?,
            open(&binding.math_add, true)?,
            open(&binding.math_mul, true)?,
            open(&binding.math_lzcnt, true)?,
            open(&binding.math_i32_to_real, true)?,
            binding.math_i32_to_real.va,
            binding.math_lzcnt.va,
        ) || !literal_shape(literal, &binding)
            || division.len() != 57
            || !at(
                division,
                0,
                &[0x55, 0x8b, 0xec, 0x8b, 0x4d, 0x10, 0x56, 0x8b, 0x75, 0x0c],
            )
            || !at(division, 10, &[0x8b, 0x06, 0x3b, 0x01, 0x74, 0x1d])
            || call(division, binding.division.va, 0x15) != Some(binding.reciprocal.va)
            || call(division, binding.division.va, 0x20) != Some(binding.math_mul.va)
            || !at(division, 0x31, &[0xc7, 0x00, 0, 0, 0x80, 0x3f, 0x5d, 0xc3])
            || equality.len() != 53
            || call(equality, binding.equality.va, 14) != Some(binding.math_sub.va)
            || !at(equality, 19, &[0xf3, 0x0f, 0x10, 0x05])
            || word(equality, 23) != Some(binding.equality_constant.va)
            || !at(equality, 32, &[0x25, 0xff, 0xff, 0xff, 0x7f])
            || !at(equality, 42, &[0x0f, 0x2f, 0x45, 0xfc, 0x0f, 0x97, 0xc0])
            || inequality.len() != 162
            || !at(inequality, 0, &[0x0f, 0xb6, 0x47, 0x01])
            || !at(inequality, 0x21, &[0x3b, 0x44, 0xcb, 0x70])
            || !at(inequality, 0x80, &[0x0f, 0x95, 0xc2])
            || constant.len() != 4
        {
            return Err(fail(Problem::Shape));
        }
        let (index_shift, table_va) =
            reciprocal_shape(reciprocal).ok_or_else(|| fail(Problem::Shape))?;
        let entries = (1usize << (23 - index_shift)) + 1;
        if table_va != binding.reciprocal_table.va || table.len() != entries * 4 {
            return Err(fail(Problem::Shape));
        }
        let equality_threshold = f32::from_bits(word(constant, 0).unwrap());
        if !equality_threshold.is_finite() || equality_threshold < 0.0 {
            return Err(fail(Problem::Shape));
        }
        let mut values = Vec::new();
        values
            .try_reserve_exact(entries)
            .map_err(|_| fail(Problem::Budget))?;
        values.extend(
            table
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| u32::from_le_bytes(*c)),
        );
        Ok(Self {
            binding,
            table: values,
            index_shift,
            equality_threshold,
            literal_bytes,
        })
    }

    pub fn origin(&self) -> &AssetIdentity {
        &self.binding.origin
    }
    pub fn image_sha256(&self) -> &Sha256Digest {
        &self.binding.image_sha256
    }

    fn division(&self, left: u32, right: u32) -> Result<u32, real::MathError> {
        if left == right {
            return Ok(1.0f32.to_bits());
        }
        let fraction = right & 0x7f_ffff;
        let index = (fraction >> self.index_shift) as usize;
        let repeated = fraction.wrapping_shl(32 - self.index_shift);
        let mut interpolation = repeated;
        interpolation |= interpolation >> self.index_shift;
        interpolation = repeated | (interpolation >> self.index_shift);
        let difference = self.table[index].wrapping_sub(self.table[index + 1]);
        let high = ((u64::from(interpolation) * u64::from(difference)) >> 32) as u32;
        let magnitude = self.table[index]
            .wrapping_sub(high)
            .wrapping_sub(right & 0x7f80_0000)
            .wrapping_add((2 * 127 - 1) << 23);
        let reciprocal = if (magnitude.wrapping_sub(1 << 23) as i32) < 0 {
            0
        } else {
            magnitude | (right & (1 << 31))
        };
        real::mul(left, reciprocal)
    }

    fn runtime_error(&self, why: impl std::fmt::Display) -> String {
        format!("script numeric {}: {why}", self.binding.origin.source)
    }
}

fn literal_shape(bytes: &[u8], binding: &Binding) -> bool {
    bytes.len() == 238
        && at(
            bytes,
            0,
            &[0x55, 0x8b, 0xec, 0x83, 0xec, 0x18, 0x53, 0x56, 0x57],
        )
        && at(bytes, 0x24, &[0x80, 0xf9, b'.', 0x74])
        && at(
            bytes,
            0x2d,
            &[
                0x8d, 0x1c, 0x9b, 0x0f, 0xbe, 0xc9, 0x8d, 0x5b, 0xe8, 0x8d, 0x1c, 0x59,
            ],
        )
        && at(bytes, 0x76, &[0x8d, 0x1c, 0x9b, 0x8d, 0x3c, 0xbf])
        && [
            (0x43, binding.math_i32_to_real.va),
            (0x98, binding.math_i32_to_real.va),
            (0xa8, binding.math_i32_to_real.va),
            (0xb5, binding.division.va),
            (0xc3, binding.math_i32_to_real.va),
            (0xd0, binding.math_add.va),
        ]
        .iter()
        .all(|(at, target)| call(bytes, binding.literal.va, *at) == Some(*target))
        && bytes.ends_with(&[0x8b, 0xe5, 0x5d, 0xc3])
}

fn reciprocal_shape(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() != 103
        || !at(bytes, 0, &[0x55, 0x8b, 0xec, 0x8b, 0x45, 0x0c, 0x56, 0x57])
        || !at(bytes, 0x19, &[0xc1, 0xea])
        || !at(bytes, 0x24, &[0x8b, 0x34, 0x95])
        || !at(bytes, 0x2f, &[0x2b, 0x0c, 0x95])
        || !at(bytes, 0x36, &[0xf7, 0xe1])
        || !at(bytes, 0x4c, &[0x81, 0xc6])
        || word(bytes, 0x4e)? != (2 * 127 - 1) << 23
        || !bytes.ends_with(&[0x89, 0x08, 0x5e, 0x5d, 0xc3])
    {
        return None;
    }
    let shift = u32::from(bytes[0x1b]);
    if !(11..=23).contains(&shift)
        || u32::from(bytes[0x16]) != 32 - shift
        || u32::from(bytes[0x1e]) != shift
        || u32::from(bytes[0x23]) != shift
    {
        return None;
    }
    let table = word(bytes, 0x27)?;
    (word(bytes, 0x32)? == table.checked_add(4)?).then_some((shift, table))
}

impl RealOps for NumericPolicy {
    fn literal(&self, spelling: &str) -> Result<f32, String> {
        if spelling.len() > self.literal_bytes {
            return Err(self.runtime_error("over-budget real literal"));
        }
        let (whole, fractional) = spelling
            .split_once('.')
            .ok_or_else(|| self.runtime_error("unsupported real literal spelling"))?;
        if (whole.is_empty() && fractional.is_empty())
            || !whole
                .bytes()
                .chain(fractional.bytes())
                .all(|b| b.is_ascii_digit())
        {
            return Err(self.runtime_error("unsupported or over-budget real literal"));
        }
        let accumulate = |s: &str| {
            s.bytes().fold(0i32, |n, b| {
                n.wrapping_mul(10).wrapping_add(i32::from(b - b'0'))
            })
        };
        let denominator = fractional.bytes().fold(1i32, |n, _| n.wrapping_mul(10));
        let result = (|| {
            let fractional = self.division(
                real::from_i32(accumulate(fractional))?,
                real::from_i32(denominator)?,
            )?;
            real::add(real::from_i32(accumulate(whole))?, fractional)
        })()
        .map_err(|why: real::MathError| self.runtime_error(why))?;
        Ok(f32::from_bits(result))
    }
    fn integer(&self, value: i32) -> Result<f32, String> {
        real::from_i32(value)
            .map(f32::from_bits)
            .map_err(|why| self.runtime_error(why))
    }
    fn binary(&self, op: BinOp, left: f32, right: f32) -> Result<Value, String> {
        let checked = |value| real::from_f32(value).map_err(|why| self.runtime_error(why));
        let (left_bits, right_bits) = (checked(left)?, checked(right)?);
        let result = match op {
            BinOp::Add => real::add(left_bits, right_bits),
            BinOp::Sub => real::sub(left_bits, right_bits),
            BinOp::Mul => real::mul(left_bits, right_bits),
            BinOp::Div => {
                // The VM rejects zero before entering the shared division helper.
                if right == 0.0 {
                    return Err(self.runtime_error("real division by zero"));
                }
                self.division(left_bits, right_bits)
            }
            BinOp::Eq => {
                return real::sub(left_bits, right_bits)
                    .map(|bits| {
                        Value::Bool(f32::from_bits(real::abs(bits)) < self.equality_threshold)
                    })
                    .map_err(|why| self.runtime_error(why));
            }
            BinOp::Ne => return Ok(Value::Bool(left_bits != right_bits)),
            BinOp::Lt => return Ok(Value::Bool(left < right)),
            BinOp::Le => return Ok(Value::Bool(left <= right)),
            BinOp::Gt => return Ok(Value::Bool(left > right)),
            BinOp::Ge => return Ok(Value::Bool(left >= right)),
            _ => return Err(self.runtime_error("unsupported real operator")),
        };
        result
            .map(|bits| Value::Real(f32::from_bits(bits)))
            .map_err(|why| self.runtime_error(why))
    }
}

#[cfg(test)]
#[path = "script_numeric_tests.rs"]
mod tests;

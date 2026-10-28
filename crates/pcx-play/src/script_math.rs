//! `SquareRoot`, `Atan2`, `Sin` and `Cos` as the reviewed client's software routines.
//! Code windows are fingerprinted; every coefficient, angle and loop bound is
//! read from the image through the addresses and immediates that code uses.
//! Runtime-initialized zero/one come from their reviewed static initializers.
use crate::camera_margins::SourceWindow;
use crate::neutral_player::{RuleInputBudget, Sha256Digest, sha256};
use crate::neutral_rule_input::Pe;
use crate::script_numeric::{Error, NumericPolicy, Problem};
use crate::script_random::real::{self, MathError};

const SIGN: u32 = 1 << 31;

#[derive(Clone, Debug)]
pub struct Binding {
    pub image_sha256: Sha256Digest,
    pub square_root_native: SourceWindow,
    pub arctangent2_native: SourceWindow,
    pub square_root: SourceWindow,
    pub integer_root: SourceWindow,
    pub arctangent2: SourceWindow,
    pub arctangent: SourceWindow,
    pub zero_initializer: SourceWindow,
    pub one_initializer: SourceWindow,
    /// Absent leaves `Sin` and `Cos` unsupported.
    pub trig: Option<TrigBinding>,
}

/// The table-driven sine and cosine routines and their native wrappers.
#[derive(Clone, Debug)]
pub struct TrigBinding {
    pub sine_native: SourceWindow,
    pub cosine_native: SourceWindow,
    pub sine: SourceWindow,
    pub cosine: SourceWindow,
}

/// Entries of the quarter-wave table: one per index `0..=1024`, so that index `i` and `i + 1`
/// bound each interpolation step.
const TABLE_ENTRIES: usize = 1025;
/// Interpolation fraction bits and table index bits of the fixed-point angle.
const FRACTION_BITS: u32 = 8;
const INDEX_MASK: i32 = 0x3ff;
const QUADRANT_SHIFT: u32 = 18;
const SINGLE_BIAS_SHIFT: u32 = 31 << 23;

/// Angle scale (fixed-point units per radian, as a real) and the quarter-wave table.
#[derive(Clone, Debug)]
struct Trig {
    scale: u32,
    table: Vec<u32>,
}

#[derive(Clone, Debug)]
pub struct ScriptMath {
    zero: u32,
    one: u32,
    /// Reduction threshold, reduction slope and offset, then the rational
    /// numerator/denominator coefficients, in the order the routine reads them.
    reduce: [u32; 3],
    rational: [u32; 3],
    pi_sixth: u32,
    half_pi: u32,
    pi: u32,
    root_bias: u32,
    root_scale: u32,
    odd_mantissa: u32,
    root_steps: u32,
    trig: Option<Trig>,
}

fn word(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}

fn call(bytes: &[u8], base: u32, at: usize) -> Option<u32> {
    (bytes.get(at) == Some(&0xe8)).then_some(())?;
    Some(
        base.wrapping_add(at as u32 + 5)
            .wrapping_add(word(bytes, at + 1)?),
    )
}

fn negative_nonzero(bits: u32) -> bool {
    bits.wrapping_add(bits) & ((bits as i32 >> 31) as u32) != 0
}

impl ScriptMath {
    pub(crate) fn read(
        image: &[u8],
        binding: &Binding,
        numeric: &NumericPolicy,
        budget: RuleInputBudget,
    ) -> Result<Self, Error> {
        let fail = |problem| Error {
            source_context: numeric.origin().source.clone(),
            problem,
        };
        if binding.image_sha256 != *numeric.image_sha256() || sha256(image) != binding.image_sha256
        {
            return Err(fail(Problem::Identity));
        }
        let pe = Pe::parse(image, budget).map_err(|e| fail(Problem::Image(e)))?;
        let open = |w: &SourceWindow| pe.window(w, true).map_err(|e| fail(Problem::Image(e)));
        let datum = |va: u32| -> Result<u32, Error> {
            let bytes = pe
                .read_from(va, false)
                .map_err(|e| fail(Problem::Image(e)))?;
            let value = word(bytes, 0).ok_or_else(|| fail(Problem::Shape))?;
            real::from_f32(f32::from_bits(value)).map_err(|_| fail(Problem::Shape))
        };
        let shape = |ok: bool| {
            if ok {
                Ok(())
            } else {
                Err(fail(Problem::Shape))
            }
        };
        let initializer = |w: &SourceWindow| {
            numeric
                .integer_initializer(open(w)?, w.va)
                .ok_or_else(|| fail(Problem::Shape))
        };
        let (zero_va, zero) = initializer(&binding.zero_initializer)?;
        let (one_va, one) = initializer(&binding.one_initializer)?;
        let (zero, one) = (zero.to_bits(), one.to_bits());

        let root_native = open(&binding.square_root_native)?;
        let atan2_native = open(&binding.arctangent2_native)?;
        let root = open(&binding.square_root)?;
        let integer_root = open(&binding.integer_root)?;
        let atan2 = open(&binding.arctangent2)?;
        let atan = open(&binding.arctangent)?;
        let threshold_va = numeric.equality_constant_va();
        shape(
            [12, 54, 85]
                .iter()
                .all(|&at| word(root_native, at) == Some(zero_va))
                && word(root_native, 27) == Some(threshold_va)
                && call(root_native, binding.square_root_native.va, 68)
                    == Some(binding.square_root.va)
                && [5, 49, 93]
                    .iter()
                    .all(|&at| word(atan2_native, at) == Some(zero_va))
                && [25, 69]
                    .iter()
                    .all(|&at| word(atan2_native, at) == Some(threshold_va))
                && call(atan2_native, binding.arctangent2_native.va, 111)
                    == Some(binding.arctangent2.va)
                && call(root, binding.square_root.va, 72) == Some(binding.integer_root.va)
                && root.get(0x4d) == Some(&0x2d)
                && root.get(0x54..0x56) == Some(&[0x69, 0xc0][..])
                && root.get(0x6f..0x71) == Some(&[0x81, 0xe1][..])
                && integer_root.get(0x0d..0x0f) == Some(&[0x8d, 0x7e][..])
                && call(atan2, binding.arctangent2.va, 76) == Some(binding.arctangent.va)
                && [32, 115, 233]
                    .iter()
                    .all(|&at| word(atan, at) == Some(one_va)),
        )?;
        let at = |bytes: &[u8], offset| word(bytes, offset).ok_or_else(|| fail(Problem::Shape));
        let half_pi_va = at(atan, 378)?;
        shape(at(atan2, 89)? == half_pi_va)?;
        Ok(Self {
            zero,
            one,
            reduce: [
                datum(at(atan, 72)?)?,
                datum(at(atan, 100)?)?,
                datum(at(atan, 131)?)?,
            ],
            rational: [
                datum(at(atan, 218)?)?,
                datum(at(atan, 253)?)?,
                datum(at(atan, 268)?)?,
            ],
            pi_sixth: datum(at(atan, 348)?)?,
            half_pi: datum(half_pi_va)?,
            pi: datum(at(atan2, 116)?)?,
            root_bias: at(root, 0x4e)?,
            root_scale: at(root, 0x56)?,
            odd_mantissa: at(root, 0x71)?,
            root_steps: u32::from(integer_root[0x0f]),
            trig: binding
                .trig
                .as_ref()
                .map(|trig| Self::read_trig(&pe, trig, numeric.multiplication_va(), &fail))
                .transpose()?,
        })
    }

    /// The sine and cosine routines share one shape: scale the angle by a constant through the
    /// shared multiply, truncate to a fixed-point integer, interpolate a quarter-wave table by
    /// quadrant and convert back. Cosine differs from sine by one added quadrant.
    fn read_trig(
        pe: &Pe,
        binding: &TrigBinding,
        multiplication: u32,
        fail: &impl Fn(Problem) -> Error,
    ) -> Result<Trig, Error> {
        let open = |w: &SourceWindow| pe.window(w, true).map_err(|e| fail(Problem::Image(e)));
        let shape = |ok: bool| {
            if ok {
                Ok(())
            } else {
                Err(fail(Problem::Shape))
            }
        };
        let (sine_native, cosine_native) =
            (open(&binding.sine_native)?, open(&binding.cosine_native)?);
        let (sine, cosine) = (open(&binding.sine)?, open(&binding.cosine)?);
        // Offsets of the immediates inside each routine; the cosine routine is one byte longer
        // after its added quadrant, which shifts the table operands by one.
        let operands = |bytes: &[u8], shift: usize| {
            Some([
                word(bytes, 0x88 + shift)?,
                word(bytes, 0x8f + shift)?,
                word(bytes, 0xa4 + shift)?,
                word(bytes, 0xab + shift)?,
            ])
        };
        let (Some([sr0, sr1, sf0, sf1]), Some([cr0, cr1, cf0, cf1])) =
            (operands(sine, 0), operands(cosine, 1))
        else {
            return Err(fail(Problem::Shape));
        };
        let table_va = sf1;
        shape(
            call(sine_native, binding.sine_native.va, 11) == Some(binding.sine.va)
                && call(cosine_native, binding.cosine_native.va, 11) == Some(binding.cosine.va)
                && call(sine, binding.sine.va, 0x13) == Some(multiplication)
                && call(cosine, binding.cosine.va, 0x13) == Some(multiplication)
                && sine.get(7) == Some(&0x68)
                && word(sine, 8) == word(cosine, 8)
                && [sr0, sr1, sf0, sf1] == [cr0, cr1, cf0, cf1]
                && sr0 == table_va.wrapping_add(4 * (TABLE_ENTRIES as u32 - 1))
                && sr1 == sr0.wrapping_sub(4)
                && sf0 == table_va.wrapping_add(4),
        )?;
        let words = |va: u32, count: usize| -> Result<Vec<u32>, Error> {
            let bytes = pe
                .read_from(va, false)
                .map_err(|e| fail(Problem::Image(e)))?;
            (0..count)
                .map(|i| word(bytes, 4 * i).ok_or_else(|| fail(Problem::Shape)))
                .collect()
        };
        let scale = words(word(sine, 8).ok_or_else(|| fail(Problem::Shape))?, 1)?[0];
        real::from_f32(f32::from_bits(scale)).map_err(|_| fail(Problem::Shape))?;
        Ok(Trig {
            scale,
            table: words(table_va, TABLE_ENTRIES)?,
        })
    }

    fn integer_root(&self, mut value: u32) -> u32 {
        let (mut remainder, mut root) = (0u32, 0u32);
        for _ in 0..self.root_steps {
            remainder = (remainder << 2) | (value >> 30);
            value <<= 2;
            root = root.wrapping_add(root);
            if root < remainder {
                remainder = remainder.wrapping_sub(root).wrapping_sub(1);
                root = root.wrapping_add(2);
            }
        }
        root >> 1
    }

    pub(crate) fn square_root(&self, x: u32) -> Result<u32, MathError> {
        if x.wrapping_add(x) & !((x as i32 >> 31) as u32) == 0 {
            return Ok(0);
        }
        let exponent = ((x >> 23) & 0xff) as i32 - 127;
        let mantissa = x & 0x7f_ffff;
        let root = self.integer_root(((mantissa | 0xff80_0000) << 8) | (mantissa >> 15));
        let factor = (root
            .wrapping_sub(self.root_bias)
            .wrapping_mul(self.root_scale)
            >> 8)
            | 127 << 23;
        let odd = ((exponent as u32) << 31) as i32 >> 31;
        let half = (odd + exponent) / 2;
        let scale = (((half + 127) as u32) << 23) | (odd as u32 & self.odd_mantissa);
        real::mul(scale, factor)
    }

    fn arctangent(&self, numeric: &NumericPolicy, a: u32) -> Result<u32, MathError> {
        let [threshold, slope, offset] = self.reduce;
        let [c_denominator, c_numerator, c_linear] = self.rational;
        let mut t = real::abs(a);
        let inverted = f32::from_bits(t) > f32::from_bits(self.one);
        if inverted {
            t = numeric.reciprocal(t);
        }
        let shifted = f32::from_bits(t) > f32::from_bits(threshold);
        if shifted {
            let denominator = real::add(self.one, real::mul(slope, t)?)?;
            t = numeric.division(real::add(t, offset)?, denominator)?;
        }
        let square = real::mul(t, t)?;
        let denominator = real::add(self.one, real::mul(c_denominator, square)?)?;
        let numerator = real::mul(t, real::add(c_linear, real::mul(c_numerator, square)?)?)?;
        let mut result = numeric.division(numerator, denominator)?;
        if shifted {
            result = real::add(result, self.pi_sixth)?;
        }
        if inverted {
            result = real::sub(self.half_pi, result)?;
        }
        Ok(if negative_nonzero(a) {
            result ^ SIGN
        } else {
            result
        })
    }

    fn arctangent2(&self, numeric: &NumericPolicy, y: u32, x: u32) -> Result<u32, MathError> {
        let mut result = if x & 0x7f80_0000 == 0 {
            self.half_pi
        } else {
            self.arctangent(numeric, real::abs(numeric.division(y, x)?))?
        };
        if negative_nonzero(x) {
            result = real::sub(self.pi, result)?;
        }
        Ok(if negative_nonzero(y) {
            result ^ SIGN
        } else {
            result
        })
    }

    fn near_zero(&self, numeric: &NumericPolicy, value: u32) -> Result<bool, MathError> {
        let difference = real::abs(real::sub(value, self.zero)?);
        Ok(numeric.equality_threshold() > f32::from_bits(difference))
    }

    /// The native `Sin`: the table routine with no input filtering.
    pub fn sine_native(&self, x: f32) -> Result<f32, MathError> {
        self.trig(x, 0)
    }

    /// The native `Cos`: the sine routine one quadrant on.
    pub fn cosine_native(&self, x: f32) -> Result<f32, MathError> {
        self.trig(x, 1)
    }

    fn trig(&self, x: f32, quadrant_offset: i32) -> Result<f32, MathError> {
        let trig = self.trig.as_ref().ok_or(MathError::Unsupported)?;
        let angle = real::mul(real::from_f32(x)?, trig.scale)?;
        Ok(f32::from_bits(table_trig(
            &trig.table,
            angle,
            quadrant_offset,
        )))
    }

    /// The native wrapper: near-zero or negative input yields the zero constant.
    pub fn square_root_native(&self, numeric: &NumericPolicy, x: f32) -> Result<f32, MathError> {
        let x = real::from_f32(x)?;
        if self.near_zero(numeric, x)? || f32::from_bits(self.zero) > f32::from_bits(x) {
            return Ok(f32::from_bits(self.zero));
        }
        self.square_root(x).map(f32::from_bits)
    }

    /// The native wrapper: both coordinates near zero yield the zero constant.
    pub fn arctangent2_native(
        &self,
        numeric: &NumericPolicy,
        y: f32,
        x: f32,
    ) -> Result<f32, MathError> {
        let (y, x) = (real::from_f32(y)?, real::from_f32(x)?);
        if self.near_zero(numeric, x)? && self.near_zero(numeric, y)? {
            return Ok(f32::from_bits(self.zero));
        }
        self.arctangent2(numeric, y, x).map(f32::from_bits)
    }
}

/// Truncate the scaled angle to a fixed-point integer, interpolate the quarter-wave table for
/// its quadrant and rebuild the real, discarding low bits as the routine does.
fn table_trig(table: &[u32], angle: u32, quadrant_offset: i32) -> u32 {
    let exponent = (angle >> 23) & 0xff;
    let units: i32 = if exponent < 0x7f {
        0
    } else {
        let mantissa = (angle & 0x7f_ffff) | 0x80_0000;
        let shift = 0x96 - exponent as i32;
        // x86 shifts mask the count to five bits.
        let magnitude = if shift >= 0 {
            mantissa >> (shift & 31)
        } else {
            mantissa << ((-shift) & 31)
        };
        let sign = (angle as i32) >> 31;
        ((magnitude as i32) ^ sign).wrapping_sub(sign)
    };
    let fraction = (units & 0xff) as u32;
    let replicated = fraction | fraction << 8 | fraction << 16 | fraction << 24;
    let quadrant = ((units >> QUADRANT_SHIFT).wrapping_add(quadrant_offset) & 3) as u32;
    let index = ((units >> FRACTION_BITS) & INDEX_MASK) as usize;
    let high = |difference: u32| ((u64::from(difference) * u64::from(replicated)) >> 32) as u32;
    let value = if quadrant & 1 == 1 {
        let upper = table[TABLE_ENTRIES - 1 - index];
        let lower = table[TABLE_ENTRIES - 2 - index];
        let step = high(upper.wrapping_sub(lower));
        step.wrapping_neg().wrapping_add(upper)
    } else {
        let lower = table[index];
        let step = high(table[index + 1].wrapping_sub(lower));
        step.wrapping_add(lower)
    };
    let negate = -(((quadrant >> 1) & 1) as i32);
    let signed = ((value as i32) ^ negate).wrapping_sub(negate);
    let mut bits = 0u32;
    if signed != 0 {
        let sign = (signed as u32) & SIGN;
        let magnitude = ((signed ^ (signed >> 31)).wrapping_sub(signed >> 31)) as u32;
        let leading = magnitude.leading_zeros();
        let msb = 31 - leading;
        let move_by = leading as i32 - 8;
        let mantissa = if move_by >= 0 {
            magnitude << (move_by & 31)
        } else {
            ((magnitude as i32) >> ((-move_by) & 31)) as u32
        };
        bits = ((msb + 127) << 23) | (mantissa & 0x7f_ffff) | sign;
    }
    if bits & 0x7f80_0000 != 0 {
        bits = bits.wrapping_sub(SINGLE_BIAS_SHIFT);
    }
    bits
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The routine scales the 8-bit interpolation fraction by 1/255 (a replicated byte), so it
    /// reaches the next table entry one step early; the error stays under this bound.
    const TOLERANCE: f64 = 3e-5;

    /// A nonstock quarter-wave table sampled at 1024 steps per quadrant.
    fn table() -> Vec<u32> {
        (0..TABLE_ENTRIES)
            .map(|i| {
                let angle = i as f64 * std::f64::consts::FRAC_PI_2 / 1024.0;
                (angle.sin() * f64::from(i32::MAX)).round() as u32
            })
            .collect()
    }

    fn scale() -> u32 {
        (((1u32 << 20) as f64 / std::f64::consts::TAU) as f32).to_bits()
    }

    fn eval(table: &[u32], x: f32, quadrant_offset: i32) -> f32 {
        let angle = real::mul(real::from_f32(x).unwrap(), scale()).unwrap();
        f32::from_bits(table_trig(table, angle, quadrant_offset))
    }

    #[test]
    fn sine_and_cosine_follow_the_table_across_quadrants() {
        let table = table();
        for step in -40..=40 {
            let x = step as f32 * 0.37;
            let (sin, cos) = (eval(&table, x, 0), eval(&table, x, 1));
            assert!(
                (f64::from(sin) - f64::from(x).sin()).abs() < TOLERANCE,
                "sin {x}: {sin}"
            );
            assert!(
                (f64::from(cos) - f64::from(x).cos()).abs() < TOLERANCE,
                "cos {x}: {cos}"
            );
        }
    }

    #[test]
    fn zero_symmetry_and_huge_angles_hold() {
        let table = table();
        assert_eq!(eval(&table, 0.0, 0), 0.0);
        for x in [0.1f32, 1.0, 2.5, 4.0, 6.0] {
            // Negative angles wrap through the table, so the pair is odd and even only to the
            // routine's own accuracy.
            assert!(f64::from(eval(&table, -x, 0) + eval(&table, x, 0)).abs() < TOLERANCE);
            assert!(f64::from(eval(&table, -x, 1) - eval(&table, x, 1)).abs() < TOLERANCE);
        }
        // Counts above the word size wrap like the routine's shifts and stay finite.
        for x in [1.0e9f32, -1.0e9, 3.0e38] {
            assert!(eval(&table, x, 0).is_finite());
            assert!(eval(&table, x, 1).abs() <= 1.0);
        }
    }
}

//! `SquareRoot` and `Atan2` as the reviewed client's software routines.
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

//! Integer arithmetic of the reviewed normal-real helpers, not host rounding.
//! Masks and bias describe IEEE binary32, independently of the game image.

const SIGN: u32 = 1 << 31;
const FRACTION: u32 = (1 << 23) - 1;
const HIDDEN: u32 = 1 << 23;
const EXPONENT: u32 = 0xff << 23;
const BIAS: u32 = 127 << 23;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("selected real arithmetic requires finite normal operands or zero")]
pub enum MathError {
    Unsupported,
}

fn checked(bits: u32) -> Result<u32, MathError> {
    let exponent = bits & EXPONENT;
    if exponent == EXPONENT || (exponent == 0 && bits & FRACTION != 0) {
        Err(MathError::Unsupported)
    } else {
        Ok(bits)
    }
}

pub fn from_f32(value: f32) -> Result<u32, MathError> {
    checked(value.to_bits())
}

pub fn from_i32(value: i32) -> Result<u32, MathError> {
    if value == 0 {
        return Ok(0);
    }
    let magnitude = value.unsigned_abs();
    let exponent = 31 - magnitude.leading_zeros();
    let mantissa = if exponent > 23 {
        magnitude >> (exponent - 23)
    } else {
        magnitude << (23 - exponent)
    };
    Ok((if value < 0 { SIGN } else { 0 }) | ((exponent + 127) << 23) | (mantissa & FRACTION))
}

pub fn abs(bits: u32) -> u32 {
    bits & !SIGN
}

pub fn below_or_equal(left: u32, right: u32) -> bool {
    f32::from_bits(left) <= f32::from_bits(right)
}

pub fn sub(left: u32, right: u32) -> Result<u32, MathError> {
    add(left, right ^ SIGN)
}

pub fn add(left: u32, right: u32) -> Result<u32, MathError> {
    checked(left)?;
    checked(right)?;
    let le = (left & EXPONENT) >> 23;
    let re = (right & EXPONENT) >> 23;
    if le == 0 {
        return Ok(right);
    }
    if re == 0 {
        return Ok(left);
    }
    let difference = re as i32 - le as i32;
    if difference >= 23 {
        return Ok(right);
    }
    if difference <= -23 {
        return Ok(left);
    }
    let signed = |bits: u32| {
        let mantissa = (((bits & FRACTION) | HIDDEN) << 1) as i32;
        if bits & SIGN == 0 {
            mantissa
        } else {
            -mantissa
        }
    };
    let mut lm = signed(left);
    let mut rm = signed(right);
    let exponent = if difference > 0 {
        lm >>= difference;
        re
    } else {
        rm >>= -difference;
        le
    };
    let sum = lm + rm;
    if sum == 0 {
        return Ok(0);
    }
    let magnitude = sum.unsigned_abs();
    let shift = 31 - magnitude.leading_zeros() as i32 - 23;
    let mantissa = if shift >= 0 {
        magnitude >> shift
    } else {
        magnitude << -shift
    };
    let encoded_exponent = ((exponent as i32 + shift - 1) as u32).wrapping_shl(23);
    checked((if sum < 0 { SIGN } else { 0 }) | encoded_exponent | (mantissa & FRACTION))
}

pub fn mul(left: u32, right: u32) -> Result<u32, MathError> {
    checked(left)?;
    checked(right)?;
    let le = left & EXPONENT;
    let re = right & EXPONENT;
    if le == 0 || re == 0 {
        return Ok(0);
    }
    let sign = (left ^ right) & SIGN;
    let lm = left & FRACTION;
    let rm = right & FRACTION;
    let exponent = le.wrapping_sub(BIAS).wrapping_add(re);
    if (exponent.wrapping_sub(HIDDEN) as i32) < 0 {
        return Ok(0);
    }
    let bits = if lm == 0 || rm == 0 {
        exponent | lm | rm | sign
    } else {
        let product = u64::from((lm | HIDDEN) << 8) * u64::from((rm | HIDDEN) << 8);
        let high = (product >> 32) as u32;
        let normalization = high >> 31;
        let mantissa = (high >> normalization >> 7) & FRACTION;
        exponent.wrapping_add(normalization << 23) | mantissa | sign
    };
    checked(bits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversion_truncates_instead_of_rounding_large_integers() {
        assert_eq!(from_i32(i32::MAX), Ok(2_147_483_520.0_f32.to_bits()));
        assert_eq!(from_i32(i32::MIN), Ok((-2_147_483_648.0_f32).to_bits()));
        assert_eq!(from_i32(0), Ok(0));
    }

    #[test]
    fn add_uses_the_selected_precision_cutoff() {
        assert_eq!(
            add(1.0_f32.to_bits(), (-2.0_f32.powi(-23)).to_bits()),
            Ok(1.0_f32.to_bits())
        );
        assert_eq!(
            sub(1.25_f32.to_bits(), 0.5_f32.to_bits()),
            Ok(0.75_f32.to_bits())
        );
        assert_eq!(add(1.0_f32.to_bits(), (-1.0_f32).to_bits()), Ok(0));
    }

    #[test]
    fn multiply_handles_power_and_fraction_paths_and_signed_zero() {
        assert_eq!(
            mul(2.0_f32.to_bits(), (-1.5_f32).to_bits()),
            Ok((-3.0_f32).to_bits())
        );
        assert_eq!(
            mul(1.5_f32.to_bits(), 1.5_f32.to_bits()),
            Ok(2.25_f32.to_bits())
        );
        assert_eq!(mul((-0.0_f32).to_bits(), 1.0_f32.to_bits()), Ok(0));
    }

    #[test]
    fn unqualified_number_classes_refuse() {
        for bits in [1, SIGN | 1, f32::INFINITY.to_bits(), f32::NAN.to_bits()] {
            assert_eq!(from_f32(f32::from_bits(bits)), Err(MathError::Unsupported));
            assert_eq!(add(1.0_f32.to_bits(), bits), Err(MathError::Unsupported));
            assert_eq!(mul(bits, 1.0_f32.to_bits()), Err(MathError::Unsupported));
        }
    }
}

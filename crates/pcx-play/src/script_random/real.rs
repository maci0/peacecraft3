//! Integer arithmetic of the reviewed normal-real helpers, not host rounding.
//! Masks and bias describe IEEE binary32, independently of the game image.

const SIGN: u32 = 1 << 31;
const FRACTION: u32 = (1 << 23) - 1;
const HIDDEN: u32 = 1 << 23;
const EXPONENT: u32 = 0xff << 23;
const BIAS: u32 = 127 << 23;
/// Immediate the reviewed reciprocal adds to its table value (`0x9a1d4c`).
const RECIPROCAL_BIAS: u32 = 0x7e80_0000;

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

/// Integer part of a normal real, truncated toward zero (the reviewed conversion at `0x9a07b0`).
/// Magnitudes below one convert to zero.
pub fn truncate(bits: u32) -> Result<i32, MathError> {
    checked(bits)?;
    let exponent = (bits & EXPONENT) >> 23;
    if exponent < 127 {
        return Ok(0);
    }
    let mantissa = (bits & FRACTION) | HIDDEN;
    let shift = 150_i32 - exponent as i32;
    let magnitude = if shift >= 0 {
        mantissa.checked_shr(shift as u32).unwrap_or(0)
    } else {
        mantissa.checked_shl(shift.unsigned_abs()).unwrap_or(0)
    };
    Ok(if bits & SIGN == 0 {
        magnitude as i32
    } else {
        (magnitude as i32).wrapping_neg()
    })
}

/// Bits of the reviewed reciprocal (`0x9a1d00`): the table entry for the top ten mantissa bits,
/// interpolated by the low thirteen. `table` holds the image's 1025 entries.
pub fn reciprocal(bits: u32, table: &[u32]) -> Result<u32, MathError> {
    checked(bits)?;
    let index = ((bits & FRACTION) >> 13) as usize;
    let (Some(&entry), Some(&next)) = (table.get(index), table.get(index + 1)) else {
        return Err(MathError::Unsupported);
    };
    let low = (bits & FRACTION) << 19;
    let fraction = (((low >> 13) | low) >> 13) | low;
    let step = ((u64::from(entry.wrapping_sub(next)) * u64::from(fraction)) >> 32) as u32;
    let value = entry
        .wrapping_sub(step)
        .wrapping_sub(bits & EXPONENT)
        .wrapping_add(RECIPROCAL_BIAS);
    let signed = (bits & SIGN) | value;
    let keep = !(((value.wrapping_sub(HIDDEN)) as i32) >> 31) as u32;
    Ok(keep & signed)
}

/// `left / right` as the reviewed divide (`0x9a02d0`): one for equal bit patterns, otherwise
/// `left` times the table reciprocal of `right`.
pub fn divide(left: u32, right: u32, table: &[u32]) -> Result<u32, MathError> {
    if left == right {
        return Ok(1.0_f32.to_bits());
    }
    mul(left, reciprocal(right, table)?)
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
    fn position_delta_cancellation_preserves_selected_precision_loss() {
        // Synthetic operands: a position update must not simplify base + (target - base).
        let base = -8192.25_f32;
        let target = 8.0 + 3.0 * 2.0_f32.powi(-12);
        let delta = sub(target.to_bits(), base.to_bits()).unwrap();
        assert_eq!(add(base.to_bits(), delta), Ok(8.0_f32.to_bits()));
        assert_ne!(8.0_f32.to_bits(), target.to_bits());
        assert_eq!(base + (target - base), 8.0 + 2.0_f32.powi(-10));

        let target = -target;
        let delta = sub(target.to_bits(), base.to_bits()).unwrap();
        assert_eq!(
            add(base.to_bits(), delta),
            Ok((-8.0 - 2.0_f32.powi(-10)).to_bits())
        );
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
    fn truncate_drops_the_fraction_toward_zero() {
        for (value, expected) in [
            (0.0_f32, 0),
            (0.99, 0),
            (1.0, 1),
            (6.99, 6),
            (7.5, 7),
            (-3.2, -3),
            (-0.5, 0),
            (123_456.78, 123_456),
        ] {
            assert_eq!(truncate(value.to_bits()), Ok(expected), "{value}");
        }
        assert_eq!(truncate(f32::NAN.to_bits()), Err(MathError::Unsupported));
    }

    /// The reciprocal of `1 + i / 1024` in the routine's layout: the mantissa of twice the
    /// reciprocal, minus one.
    fn ideal_table() -> Vec<u32> {
        (0..1025)
            .map(|i| {
                ((2.0 / (1.0 + f64::from(i) / 1024.0) - 1.0) * f64::from(1_u32 << 23)).round()
                    as u32
            })
            .collect()
    }

    #[test]
    fn divide_follows_the_table_reciprocal_and_returns_one_for_equal_operands() {
        let table = ideal_table();
        for (numerator, denominator) in [
            (38.0_f32, 10_000.0_f32),
            (1.0, 3.0),
            (7.0, 0.3),
            (-5.0, 8.0),
        ] {
            let got =
                f32::from_bits(divide(numerator.to_bits(), denominator.to_bits(), &table).unwrap());
            let expected = numerator / denominator;
            assert!(
                ((got - expected) / expected).abs() < 3e-6,
                "{numerator} / {denominator} = {got}, expected {expected}"
            );
        }
        assert_eq!(
            divide(10_000.0_f32.to_bits(), 10_000.0_f32.to_bits(), &table),
            Ok(1.0_f32.to_bits())
        );
        // A power of two divides exactly through the first table entry.
        assert_eq!(
            divide(3.0_f32.to_bits(), 4.0_f32.to_bits(), &table),
            Ok(0.75_f32.to_bits())
        );
        // A table that is too short, or an operand outside the normal range, refuses.
        assert_eq!(
            reciprocal(10_000.0_f32.to_bits(), &table[..4]),
            Err(MathError::Unsupported)
        );
        assert_eq!(
            reciprocal(f32::INFINITY.to_bits(), &table),
            Err(MathError::Unsupported)
        );
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

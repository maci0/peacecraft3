//! Unit-data heading properties and their native angle conversions.
use super::{call, decimal, shape, word};
use crate::{script_numeric::NumericPolicy, script_random::real};

#[derive(Clone, Debug)]
pub struct Rules {
    pub turn_column: String,
    pub window_column: String,
    pub selector_column: String,
    pub to_radians: f32,
    pub to_degrees: f32,
    pub minimum_turn: f32,
    pub timed_instant_limit: f32,
    pub zero: f32,
    pub selector_mask: u32,
    pub direction: super::direction::Rules,
}

impl Rules {
    pub(crate) fn read(
        read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
        numeric: &NumericPolicy,
    ) -> Result<Self, String> {
        let mut column = |site, offset: u32, getter| -> Result<String, String> {
            let code = read(site, 21, true)?;
            shape(&code, 0, &[0x68])?;
            let destination = if offset <= i8::MAX as u32 {
                shape(
                    &code,
                    5,
                    &[0x8d, 0x41, offset as u8, 0x50, 0x8d, 0x41, 0x10, 0x50],
                )?;
                13
            } else {
                shape(&code, 5, &[0x8d, 0x81])?;
                if word(&code, 7)? != offset {
                    return Err("heading profile column offset differs".into());
                }
                shape(&code, 11, &[0x50, 0x8d, 0x41, 0x10, 0x50])?;
                16
            };
            call(&code, site, destination, 0x6ac590)?;
            let get = read(getter, 21, true)?;
            if offset <= i8::MAX as u32 {
                shape(&get, 3, &[0x8d, 0x41, offset as u8, 0x50])?;
                call(&get, getter, 13, 0x6b0200)?;
            } else {
                shape(&get, 3, &[0x8d, 0x81])?;
                if word(&get, 5)? != offset {
                    return Err("heading profile getter offset differs".into());
                }
                call(&get, getter, 16, 0x6b0200)?;
            }
            let text = read(word(&code, 1)?, 32, false)?;
            let end = text
                .iter()
                .position(|b| *b == 0)
                .ok_or("heading profile column unterminated")?;
            if end == 0 || !text[..end].iter().all(u8::is_ascii_alphanumeric) {
                return Err("heading profile column invalid".into());
            }
            String::from_utf8(text[..end].to_vec()).map_err(|e| e.to_string())
        };
        let turn_column = column(0x6b3cd0, 0x74, 0x6b8cf0)?;
        let window_column = column(0x6b3ce8, 0x80, 0x6b7dc0)?;
        // The selector getter is integral, unlike the two real-valued properties.
        let registration = read(0x6b3d03, 21, true)?;
        shape(&registration, 0, &[0x68])?;
        shape(
            &registration,
            5,
            &[0x8d, 0x81, 0x8c, 0, 0, 0, 0x50, 0x8d, 0x41, 0x10, 0x50],
        )?;
        call(&registration, 0x6b3d03, 16, 0x6ac590)?;
        let text = read(word(&registration, 1)?, 32, false)?;
        let end = text
            .iter()
            .position(|b| *b == 0)
            .ok_or("heading selector column unterminated")?;
        if end == 0 || !text[..end].iter().all(u8::is_ascii_alphanumeric) {
            return Err("heading selector column invalid".into());
        }
        let selector_column = String::from_utf8(text[..end].to_vec()).map_err(|e| e.to_string())?;
        let selector = read(0x6b7c40, 18, true)?;
        shape(&selector, 3, &[0x8d, 0x81, 0x8c, 0, 0, 0])?;
        call(&selector, 0x6b7c40, 13, 0x6afae0)?;

        let radians = read(0x6887e5, 15, true)?;
        shape(&radians, 0, &[0x68])?;
        call(&radians, 0x6887e5, 10, numeric.multiplication_va())?;
        let setter = read(0x4ab40e, 19, true)?;
        shape(&setter, 0, &[0x68])?;
        call(&setter, 0x4ab40e, 14, numeric.multiplication_va())?;
        if word(&radians, 1)? != word(&setter, 1)? {
            return Err("heading authored and script angle scales differ".into());
        }
        let timed_instant_limit =
            Self::read_timed_instant_limit(read, word(&setter, 1)?, numeric.multiplication_va())?;
        let degrees = read(0x4976a5, 15, true)?;
        shape(&degrees, 0, &[0x68])?;
        call(&degrees, 0x4976a5, 10, numeric.multiplication_va())?;
        let minimum = read(0x800a0b, 18, true)?;
        shape(&minimum, 0, &[0x0f, 0x2f, 0x05])?;
        shape(&minimum, 7, &[0xa1])?;
        if word(&minimum, 3)? != word(&minimum, 8)? {
            return Err("heading turn minimum comparison differs".into());
        }
        let (zero_va, zero) = numeric
            .integer_initializer(&read(0x401300, 16, true)?, 0x401300)
            .ok_or("heading zero initializer invalid")?;
        let typed = read(0x95d220, 98, true)?;
        shape(&typed, 0x5d, &[0xa1])?;
        if word(&typed, 0x5e)? != zero_va {
            return Err("heading typed marker default differs".into());
        }
        let index = read(0x8b0920, 3, true)?;
        shape(&index, 0, &[0x83, 0xe7])?;
        let mut value = |va| -> Result<f32, String> {
            let v = f32::from_bits(word(&read(va, 4, false)?, 0)?);
            if !v.is_finite() || v <= zero {
                return Err("heading profile scalar invalid".into());
            }
            Ok(v)
        };
        Ok(Self {
            turn_column,
            window_column,
            selector_column,
            to_radians: value(word(&radians, 1)?)?,
            to_degrees: value(word(&degrees, 1)?)?,
            minimum_turn: value(word(&minimum, 3)?)?,
            timed_instant_limit,
            zero,
            selector_mask: u32::from(index[2]),
            direction: super::direction::Rules::read(read, numeric)?,
        })
    }

    fn read_timed_instant_limit(
        read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
        radians_operand: u32,
        multiplication: u32,
    ) -> Result<f32, String> {
        let branch = read(0x4ab443, 20, true)?;
        shape(&branch, 0, &[0xf3, 0x0f, 0x10, 0x05])?;
        shape(
            &branch,
            8,
            &[
                0x83, 0xec, 0x14, 0x56, 0x8b, 0x75, 0x10, 0x0f, 0x2f, 0x06, 0x72, 0x45,
            ],
        )?;
        let short = read(0x4ab474, 35, true)?;
        shape(&short, 0, &[0x68])?;
        shape(
            &short,
            5,
            &[0xff, 0x75, 0x0c, 0x8b, 0xf0, 0x8d, 0x45, 0xf4, 0x50],
        )?;
        call(&short, 0x4ab474, 14, multiplication)?;
        shape(
            &short,
            19,
            &[
                0x83, 0xc4, 0x0c, 0x8d, 0x45, 0xf4, 0x8b, 0xce, 0x6a, 0, 0x50,
            ],
        )?;
        let direct = read(0x4ab424, 13, true)?;
        shape(
            &direct,
            0,
            &[0x8d, 0x45, 0xfc, 0x8b, 0xce, 0x6a, 0, 0x50, 0xe8],
        )?;
        let setter = 0x4ab431_u32.wrapping_add(word(&direct, 9)?);
        call(&short, 0x4ab474, 30, setter)?;
        if word(&short, 1)? != radians_operand {
            return Err("timed and direct facing angle scales differ".into());
        }
        let value = f32::from_bits(word(&read(word(&branch, 4)?, 4, false)?, 0)?);
        if !value.is_finite() {
            return Err("timed facing duration threshold is not finite".into());
        }
        Ok(value)
    }

    pub(crate) fn authored(&self, numeric: &NumericPolicy, cell: &str) -> Result<f32, String> {
        match cell.trim() {
            "" | "-" | "_" => Ok(self.zero),
            text => decimal(text.as_bytes(), numeric),
        }
    }

    pub(crate) fn radians(&self, degrees: f32) -> Result<f32, String> {
        real::mul(
            real::from_f32(degrees).map_err(|e| e.to_string())?,
            self.to_radians.to_bits(),
        )
        .map(f32::from_bits)
        .map_err(|e| e.to_string())
    }

    pub(crate) fn degrees(&self, radians: f32) -> Result<f32, String> {
        real::mul(
            real::from_f32(radians).map_err(|e| e.to_string())?,
            self.to_degrees.to_bits(),
        )
        .map(f32::from_bits)
        .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timed_fixture(limit: f32) -> Vec<(u32, Vec<u8>)> {
        let relative = |bytes: &mut Vec<u8>, base: u32, target: u32| {
            let at = bytes.len() as u32;
            bytes.push(0xe8);
            bytes.extend(target.wrapping_sub(base + at + 5).to_le_bytes());
        };
        let mut branch = vec![0xf3, 0x0f, 0x10, 0x05];
        branch.extend(0x82000_u32.to_le_bytes());
        branch.extend([
            0x83, 0xec, 0x14, 0x56, 0x8b, 0x75, 0x10, 0x0f, 0x2f, 0x06, 0x72, 0x45,
        ]);
        let mut short = vec![0x68];
        short.extend(0x83000_u32.to_le_bytes());
        short.extend([0xff, 0x75, 0x0c, 0x8b, 0xf0, 0x8d, 0x45, 0xf4, 0x50]);
        relative(&mut short, 0x4ab474, 0x90100);
        short.extend([
            0x83, 0xc4, 0x0c, 0x8d, 0x45, 0xf4, 0x8b, 0xce, 0x6a, 0, 0x50,
        ]);
        relative(&mut short, 0x4ab474, 0x90200);
        let mut direct = vec![0x8d, 0x45, 0xfc, 0x8b, 0xce, 0x6a, 0, 0x50];
        relative(&mut direct, 0x4ab424, 0x90200);
        vec![
            (0x4ab443, branch),
            (0x4ab474, short),
            (0x4ab424, direct),
            (0x82000, limit.to_le_bytes().to_vec()),
        ]
    }

    fn timed_decode(memory: &[(u32, Vec<u8>)]) -> Result<f32, String> {
        Rules::read_timed_instant_limit(
            &mut |va, n, code| {
                let bytes = memory
                    .iter()
                    .find(|(at, _)| *at == va)
                    .ok_or("unprovided timed-facing fixture window")?;
                if bytes.1.len() != n || code != (va != 0x82000) {
                    return Err("timed-facing fixture window differs".into());
                }
                Ok(bytes.1.clone())
            },
            0x83000,
            0x90100,
        )
    }

    #[test]
    fn timed_facing_threshold_follows_relocated_source_and_rejects_branch_or_setter_drift() {
        for limit in [-0.03125_f32, 0.21875, 0.6875] {
            assert_eq!(
                timed_decode(&timed_fixture(limit)).unwrap().to_bits(),
                limit.to_bits()
            );
        }
        for (address, at) in [
            (0x4ab443, 18),
            (0x4ab474, 1),
            (0x4ab474, 15),
            (0x4ab474, 28),
            (0x4ab474, 31),
            (0x4ab424, 6),
        ] {
            let mut memory = timed_fixture(0.21875);
            memory.iter_mut().find(|(va, _)| *va == address).unwrap().1[at] ^= 1;
            assert!(timed_decode(&memory).is_err(), "{address:x}+{at}");
        }
        assert!(timed_decode(&timed_fixture(f32::NAN)).is_err());
        let mut truncated = timed_fixture(0.21875);
        truncated.last_mut().unwrap().1.pop();
        assert!(timed_decode(&truncated).is_err());
    }

    #[test]
    fn nonstock_heading_profile_keeps_marker_default_distinct_from_numeric_zero() {
        let (image, binding, budget) = crate::script_numeric::tests::fixture();
        let numeric = NumericPolicy::read(&image, binding, budget, 64).unwrap();
        let rules = Rules {
            turn_column: "syntheticTurn".into(),
            window_column: "syntheticWindow".into(),
            selector_column: "syntheticSelector".into(),
            to_radians: 0.125,
            to_degrees: 8.,
            minimum_turn: 0.25,
            timed_instant_limit: 0.21875,
            zero: 7.,
            selector_mask: 3,
            direction: super::super::direction::tests::nonstock(),
        };
        for marker in ["", "-", "_", "  _  "] {
            assert_eq!(rules.authored(&numeric, marker).unwrap(), 7.);
        }
        assert_eq!(rules.authored(&numeric, "0").unwrap(), 0.);
        assert_eq!(rules.authored(&numeric, "37").unwrap(), 37.);
        assert_eq!(rules.radians(42.).unwrap(), 5.25);
        assert_eq!(rules.degrees(5.25).unwrap(), 42.);
        for malformed in ["garbage", "--", "__", "1_0", "1e3", "NaN", "Inf"] {
            assert!(rules.authored(&numeric, malformed).is_err(), "{malformed}");
        }
        assert!(rules.radians(f32::NAN).is_err());
        assert!(rules.degrees(f32::INFINITY).is_err());
    }
}

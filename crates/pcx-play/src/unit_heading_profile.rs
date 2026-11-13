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
            zero,
            selector_mask: u32::from(index[2]),
            direction: super::direction::Rules::read(read, numeric)?,
        })
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

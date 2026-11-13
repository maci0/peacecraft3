//! Logical stationary turning. The physics queue and public orientation are separate phases.
use crate::{script_numeric::NumericPolicy, script_random::real};

#[path = "unit_heading_direction.rs"]
pub mod direction;
#[path = "unit_heading_power.rs"]
pub mod power;
#[path = "unit_heading_profile.rs"]
pub mod profile;
#[path = "unit_heading_timed.rs"]
pub mod timed;
#[path = "unit_heading_visual.rs"]
pub mod visual;

#[derive(Clone, Debug)]
pub struct Rules {
    pub period: f32,
    pub window: f32,
    pub tolerance: f32,
    pub half_turn: f32,
    pub full_turn: f32,
    pub visual: Option<visual::Rules>,
    pub profile: Option<profile::Rules>,
    pub timed: Option<timed::Rules>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Step {
    pub logical: f32,
    /// Read before this frame's turn, so reaching the window does not finish in the same frame.
    pub arrived: bool,
}

fn word(bytes: &[u8], at: usize) -> Result<u32, String> {
    bytes
        .get(at..at.saturating_add(4))
        .and_then(|b| b.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| "heading truncated operand".into())
}
fn shape(bytes: &[u8], at: usize, expected: &[u8]) -> Result<(), String> {
    if bytes.get(at..at.saturating_add(expected.len())) == Some(expected) {
        Ok(())
    } else {
        Err("heading unsupported instruction".into())
    }
}
fn call(bytes: &[u8], base: u32, at: usize, target: u32) -> Result<(), String> {
    shape(bytes, at, &[0xe8])?;
    if base
        .wrapping_add(at as u32 + 5)
        .wrapping_add(word(bytes, at + 1)?)
        != target
    {
        return Err("heading unsupported call relationship".into());
    }
    Ok(())
}

/// The bounded ordinary-decimal branch of the native string-to-real initializer.
/// Unlike script literals, it converts all digits together before scaling.
fn decimal(text: &[u8], numeric: &NumericPolicy) -> Result<f32, String> {
    let mut digits = 0_i32;
    let mut fractional = None;
    let mut significant = 0;
    let mut count = 0;
    for &byte in text {
        match byte {
            b'.' if fractional.is_none() => fractional = Some(0),
            b'0'..=b'9' => {
                count += 1;
                significant += usize::from(digits != 0 || byte != b'0');
                if significant > 9 {
                    return Err("heading initializer exceeds qualified significant digits".into());
                }
                digits = digits
                    .checked_mul(10)
                    .and_then(|v| v.checked_add(i32::from(byte - b'0')))
                    .ok_or("heading initializer integer overflow")?;
                if let Some(n) = &mut fractional {
                    *n += 1;
                }
            }
            _ => return Err("heading unsupported decimal initializer".into()),
        }
    }
    if count == 0 || text.len() > 16 {
        return Err("heading empty or over-budget initializer".into());
    }
    let result = (|| {
        let mut exponent = fractional.unwrap_or(0);
        let mut factor = real::from_i32(10)?;
        let mut scale = real::from_i32(1)?;
        while exponent != 0 {
            if exponent & 1 != 0 {
                scale = real::mul(scale, factor)?;
            }
            factor = real::mul(factor, factor)?;
            exponent >>= 1;
        }
        numeric.division(real::from_i32(digits)?, scale)
    })()
    .map_err(|e: real::MathError| format!("heading decimal: {e}"))?;
    Ok(f32::from_bits(result))
}

impl Rules {
    pub(crate) fn cast_ready(&self, current: f32, goal: f32) -> Result<bool, real::MathError> {
        let delta = self.delta(current, goal)?.abs();
        Ok(delta <= self.window
            || f32::from_bits(real::sub(delta.to_bits(), self.window.to_bits())?).abs()
                < self.tolerance)
    }
    pub(crate) fn normalize(
        &self,
        numeric: &NumericPolicy,
        angle: f32,
    ) -> Result<f32, real::MathError> {
        let angle = if angle.abs() >= self.full_turn {
            let value = f32::from_bits(
                numeric.positive_remainder(angle.abs().to_bits(), self.full_turn.to_bits())?,
            );
            value.copysign(angle)
        } else {
            angle
        };
        if angle < 0.0 {
            real::add(angle.to_bits(), self.full_turn.to_bits()).map(f32::from_bits)
        } else {
            Ok(angle)
        }
    }

    fn delta(&self, current: f32, goal: f32) -> Result<f32, real::MathError> {
        // Native subtraction truncates signed intermediates, so reversing operands changes bits.
        let reverse = f32::from_bits(real::sub(current.to_bits(), goal.to_bits())?);
        if reverse.abs() > self.half_turn {
            Ok(f32::from_bits(real::sub(
                self.full_turn.copysign(reverse).to_bits(),
                reverse.to_bits(),
            )?))
        } else {
            Ok(-reverse)
        }
    }

    pub(super) fn read(
        read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
        numeric: &NumericPolicy,
    ) -> Result<Self, String> {
        let remainder = read(0x9a1a90, 0x142, true)?;
        if !numeric.positive_remainder_shape(&remainder, 0x9a1a90) {
            return Err("heading remainder arithmetic or table relationship differs".into());
        }
        let timer = read(0x4ee0ce, 21, true)?;
        shape(&timer, 0, &[0x68])?;
        shape(&timer, 5, &[0x68])?;
        shape(&timer, 10, &[0x81, 0xc1, 0xa8, 2, 0, 0])?;
        call(&timer, 0x4ee0ce, 16, 0x7fbe90)?;
        let init = read(0x40d030, 19, true)?;
        shape(&init, 0, &[0x68])?;
        shape(&init, 5, &[0x68])?;
        shape(&init, 15, &[0x83, 0xc4, 8, 0xc3])?;
        call(&init, 0x40d030, 10, 0x9a1760)?;
        if word(&timer, 6)? != word(&init, 6)? {
            return Err("heading physics timer initializer differs".into());
        }
        let text = read(word(&init, 1)?, 17, false)?;
        let end = text
            .iter()
            .position(|b| *b == 0)
            .ok_or("heading unterminated period")?;
        let period = decimal(&text[..end], numeric)?;

        let arrival = read(0x8aec50, 8, true)?;
        shape(&arrival, 0, &[0xf3, 0x0f, 0x10, 0x05])?;
        let tolerance = read(0x8aebf5, 8, true)?;
        shape(&tolerance, 0, &[0xf3, 0x0f, 0x10, 0x05])?;
        let arc = read(0x8a19ea, 14, true)?;
        shape(&arc, 0, &[0x0f, 0x2f, 0x05])?;
        shape(&arc, 7, &[0x76, 0x5d, 0xa1])?;
        Self::read_normalization(read, numeric, word(&arc, 10)?)?;
        let step = read(0x8b0169, 20, true)?;
        shape(&step, 0, &[0x8b, 0x86, 0xb8, 0, 0, 0])?;
        shape(&step, 14, &[0x0f, 0x2f, 0xc1, 0x0f, 0x47, 0xc1])?;
        let value = |read: &mut dyn FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
                     va|
         -> Result<f32, String> {
            let v = f32::from_bits(word(&read(va, 4, false)?, 0)?);
            if !v.is_finite() || v <= 0.0 {
                return Err("heading nonpositive or nonfinite source scalar".into());
            }
            Ok(v)
        };
        let rules = Self {
            period,
            window: value(read, word(&arrival, 4)?)?,
            tolerance: value(read, word(&tolerance, 4)?)?,
            half_turn: value(read, word(&arc, 3)?)?,
            full_turn: value(read, word(&arc, 10)?)?,
            visual: None,
            profile: None,
            timed: None,
        };
        if !rules.period.is_finite()
            || rules.period <= 0.0
            || rules.tolerance >= rules.window
            || rules.window >= rules.half_turn
            || rules.half_turn >= rules.full_turn
        {
            return Err("heading inconsistent source ranges".into());
        }
        Ok(rules)
    }

    fn read_normalization(
        read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
        numeric: &NumericPolicy,
        full_turn: u32,
    ) -> Result<(), String> {
        call(&read(0x8b14b9, 5, true)?, 0x8b14b9, 0, 0x8bc3d0)?;
        let setter = read(0x8bc3dd, 16, true)?;
        call(&setter, 0x8bc3dd, 0, 0x8aead0)?;
        shape(
            &setter,
            5,
            &[0x83, 0xc4, 8, 0x8b, 0, 0x89, 0x86, 0x8c, 0, 0, 0],
        )?;
        let wrap = read(0x8aeaec, 27, true)?;
        shape(&wrap, 0, &[0x0f, 0x2f, 0x05])?;
        shape(&wrap, 7, &[0x72, 0x1b, 0x68])?;
        shape(&wrap, 14, &[0x8d, 0x45, 0x0c, 0x50, 0x8d, 0x45, 0xfc, 0x50])?;
        call(&wrap, 0x8aeaec, 22, 0x9a1a90)?;
        let negative = read(0x8aeb10, 32, true)?;
        shape(&negative, 0, &[0xf3, 0x0f, 0x10, 0x05])?;
        shape(&negative, 8, &[0x0f, 0x2f, 0x45, 0x0c, 0x76, 0x20, 0x68])?;
        shape(
            &negative,
            19,
            &[0x8d, 0x45, 0x0c, 0x50, 0x8d, 0x45, 0xfc, 0x50],
        )?;
        call(&negative, 0x8aeb10, 27, numeric.addition_va())?;
        let (zero, value) = numeric
            .integer_initializer(&read(0x401300, 16, true)?, 0x401300)
            .ok_or("heading normalization zero initializer differs")?;
        if word(&wrap, 3)? != full_turn
            || word(&wrap, 10)? != full_turn
            || word(&negative, 15)? != full_turn
            || word(&negative, 4)? != zero
            || value != 0.0
        {
            return Err("heading normalization operands differ".into());
        }
        Ok(())
    }

    /// One stationary logical step. Callers must supply the authoritative physics frame.
    /// This does not advance game time, public facing, walking or script timed turns.
    pub fn stationary_step(
        &self,
        numeric: &NumericPolicy,
        logical: f32,
        goal: f32,
        speed: f32,
    ) -> Result<Step, String> {
        if !logical.is_finite()
            || !goal.is_finite()
            || !speed.is_finite()
            || !(0.0..self.full_turn).contains(&logical)
            || !(0.0..self.full_turn).contains(&goal)
            || speed < 0.0
        {
            return Err(
                "heading requires normalized finite angles and nonnegative turn speed".into(),
            );
        }
        let result = (|| {
            let mut delta = self.delta(logical, goal)?;
            if delta.abs() < self.tolerance {
                delta = 0.0;
            }
            if delta.abs() <= self.window {
                return Ok(Step {
                    logical,
                    arrived: true,
                });
            }
            let increment = delta.abs().min(speed).copysign(delta);
            let logical = self.normalize(
                numeric,
                f32::from_bits(real::add(logical.to_bits(), increment.to_bits())?),
            )?;
            Ok(Step {
                logical,
                arrived: false,
            })
        })();
        result.map_err(|e: real::MathError| format!("heading logical step: {e}"))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    type Memory = Vec<(u32, Vec<u8>)>;

    fn numeric() -> NumericPolicy {
        let (image, binding, budget) = crate::script_numeric::tests::fixture();
        NumericPolicy::read(&image, binding, budget, 64).unwrap()
    }

    fn fixture() -> Memory {
        let (_, binding, _) = crate::script_numeric::tests::fixture();
        let mut remainder = vec![0; 0x142];
        remainder[0x33..0x37].copy_from_slice(&binding.reciprocal_table.va.to_le_bytes());
        remainder[0x3e..0x42].copy_from_slice(&(binding.reciprocal_table.va + 4).to_le_bytes());
        remainder[0x8b..0x8e].copy_from_slice(&[0x83, 0xe9, 0x7f]);
        remainder[0x95..0x98].copy_from_slice(&[0x83, 0xf9, 0x17]);
        remainder[0x9a..0xa6]
            .copy_from_slice(&[0x83, 0xc1, 8, 0xb8, 0, 0, 0, 0x80, 0xd3, 0xf8, 0x23, 0xc2]);
        for (at, target) in [
            (0x79, binding.math_mul.va),
            (0xb2, binding.math_sub.va),
            (0xcf, binding.math_mul.va),
            (0xfb, binding.math_sub.va),
        ] {
            remainder[at] = 0xe8;
            remainder[at + 1..at + 5]
                .copy_from_slice(&target.wrapping_sub(0x9a1a90 + at as u32 + 5).to_le_bytes());
        }
        let mut memory = vec![];
        let mut put = |va, bytes: Vec<u8>| memory.push((va, bytes));
        put(0x9a1a90, remainder);
        let relative = |base: u32, target: u32| target.wrapping_sub(base + 5).to_le_bytes();
        let mut timer = vec![0x68];
        timer.extend(0x99_u32.to_le_bytes());
        timer.push(0x68);
        timer.extend(0x70200_u32.to_le_bytes());
        timer.extend([0x81, 0xc1, 0xa8, 2, 0, 0, 0xe8]);
        timer.extend(relative(0x4ee0de, 0x7fbe90));
        put(0x4ee0ce, timer);
        let mut init = vec![0x68];
        init.extend(0x70300_u32.to_le_bytes());
        init.push(0x68);
        init.extend(0x70200_u32.to_le_bytes());
        init.push(0xe8);
        init.extend(relative(0x40d03a, 0x9a1760));
        init.extend([0x83, 0xc4, 8, 0xc3]);
        put(0x40d030, init);
        put(0x70300, b"1\0xxxxxxxxxxxxxxx".to_vec());
        for (site, operand) in [(0x8aec50, 0x70400_u32), (0x8aebf5, 0x70410)] {
            let mut instruction = vec![0xf3, 0x0f, 0x10, 0x05];
            instruction.extend(operand.to_le_bytes());
            put(site, instruction);
        }
        let mut arc = vec![0x0f, 0x2f, 0x05];
        arc.extend(0x70420_u32.to_le_bytes());
        arc.extend([0x76, 0x5d, 0xa1]);
        arc.extend(0x70430_u32.to_le_bytes());
        put(0x8a19ea, arc);
        let mut step = vec![0x8b, 0x86, 0xb8, 0, 0, 0];
        step.extend([0x90; 8]);
        step.extend([0x0f, 0x2f, 0xc1, 0x0f, 0x47, 0xc1]);
        put(0x8b0169, step);
        for (va, value) in [
            (0x70400, 0.25_f32),
            (0x70410, 0.03125),
            (0x70420, 6.),
            (0x70430, 12.),
        ] {
            put(va, value.to_bits().to_le_bytes().to_vec());
        }
        let mut setter = vec![0xe8];
        setter.extend(relative(0x8b14b9, 0x8bc3d0));
        put(0x8b14b9, setter);
        let mut normalize = vec![0xe8];
        normalize.extend(relative(0x8bc3dd, 0x8aead0));
        normalize.extend([0x83, 0xc4, 8, 0x8b, 0, 0x89, 0x86, 0x8c, 0, 0, 0]);
        put(0x8bc3dd, normalize);
        let mut wrap = vec![0x0f, 0x2f, 0x05];
        wrap.extend(0x70430_u32.to_le_bytes());
        wrap.extend([0x72, 0x1b, 0x68]);
        wrap.extend(0x70430_u32.to_le_bytes());
        wrap.extend([0x8d, 0x45, 0x0c, 0x50, 0x8d, 0x45, 0xfc, 0x50, 0xe8]);
        wrap.extend(relative(0x8aeb02, 0x9a1a90));
        put(0x8aeaec, wrap);
        let mut negative = vec![0xf3, 0x0f, 0x10, 0x05];
        negative.extend(0x70500_u32.to_le_bytes());
        negative.extend([0x0f, 0x2f, 0x45, 0x0c, 0x76, 0x20, 0x68]);
        negative.extend(0x70430_u32.to_le_bytes());
        negative.extend([0x8d, 0x45, 0x0c, 0x50, 0x8d, 0x45, 0xfc, 0x50, 0xe8]);
        negative.extend(relative(0x8aeb2b, binding.math_add.va));
        put(0x8aeb10, negative);
        let mut zero = vec![0x6a, 0, 0x68];
        zero.extend(0x70500_u32.to_le_bytes());
        zero.push(0xe8);
        zero.extend(relative(0x401307, binding.math_i32_to_real.va));
        zero.extend([0x83, 0xc4, 8, 0xc3]);
        put(0x401300, zero);
        memory
    }

    fn decode(memory: &Memory) -> Result<Rules, String> {
        Rules::read(
            &mut |va, n, _| {
                memory
                    .iter()
                    .find_map(|(base, bytes)| {
                        va.checked_sub(*base)
                            .and_then(|at| bytes.get(at as usize..at as usize + n))
                            .map(<[u8]>::to_vec)
                    })
                    .ok_or_else(|| "heading synthetic mapping".into())
            },
            &numeric(),
        )
    }

    #[test]
    fn relocated_heading_operands_and_nonstock_ranges_are_source_bound() {
        let rules = decode(&fixture()).unwrap();
        assert_eq!(rules.period, 1.);
        assert_eq!(rules.window, 0.25);
        assert_eq!(rules.tolerance, 0.03125);
        assert_eq!(rules.half_turn, 6.);
        assert_eq!(rules.full_turn, 12.);
        let mut drift = fixture();
        drift.iter_mut().find(|(va, _)| *va == 0x70400).unwrap().1 =
            0.75_f32.to_bits().to_le_bytes().to_vec();
        assert_eq!(decode(&drift).unwrap().window, 0.75);
        for (va, at) in [
            (0x4ee0ce, 6),
            (0x40d030, 11),
            (0x8b0169, 2),
            (0x8a19ea, 8),
            (0x9a1a90, 0x33),
            (0x9a1a90, 0x9a),
            (0x9a1a90, 0xfc),
        ] {
            let mut drift = fixture();
            drift.iter_mut().find(|(base, _)| *base == va).unwrap().1[at] ^= 1;
            assert!(decode(&drift).is_err(), "{va:x}+{at}");
        }
    }

    #[test]
    fn stationary_arrival_is_checked_before_turn_and_uses_the_short_arc() {
        let rules = decode(&fixture()).unwrap();
        let first = rules.stationary_step(&numeric(), 0., 1., 0.75).unwrap();
        assert_eq!(
            first,
            Step {
                logical: 0.75,
                arrived: false
            }
        );
        assert_eq!(
            rules
                .stationary_step(&numeric(), first.logical, 1., 0.75)
                .unwrap(),
            Step {
                logical: 0.75,
                arrived: true
            }
        );
        assert_eq!(
            rules.stationary_step(&numeric(), 11.5, 0.5, 0.75).unwrap(),
            Step {
                // The synthetic reciprocal table deliberately differs from exact division.
                logical: 6.375,
                arrived: false
            }
        );
        assert_eq!(
            rules.stationary_step(&numeric(), 0.5, 11.5, 0.75).unwrap(),
            Step {
                logical: 11.75,
                arrived: false
            }
        );
        assert_eq!(
            rules.stationary_step(&numeric(), 0., 1., 0.).unwrap(),
            Step {
                logical: 0.,
                arrived: false
            }
        );
        for (from, goal, speed) in [
            (f32::NAN, 1., 1.),
            (0., 12., 1.),
            (-1., 1., 1.),
            (0., 1., -1.),
        ] {
            assert!(
                rules
                    .stationary_step(&numeric(), from, goal, speed)
                    .is_err()
            );
        }
    }

    #[test]
    fn nonstock_normalization_reuses_fractional_reciprocal_and_refuses_unsupported_numbers() {
        let rules = decode(&fixture()).unwrap();
        let numeric = numeric();
        assert_eq!(rules.normalize(&numeric, 12.25).unwrap(), 6.375);
        assert_eq!(rules.normalize(&numeric, -0.25).unwrap(), 11.75);
        assert_eq!(rules.normalize(&numeric, -12.25).unwrap(), 5.625);
        assert_eq!(
            rules.normalize(&numeric, -0.).unwrap().to_bits(),
            (-0_f32).to_bits()
        );
        assert_eq!(
            numeric
                .positive_remainder(0_f32.to_bits(), 12_f32.to_bits())
                .unwrap(),
            0
        );
        for (left, right) in [
            (-1., 12.),
            (1., 0.),
            (1., -2.),
            (f32::NAN, 12.),
            (1., f32::INFINITY),
        ] {
            assert!(
                numeric
                    .positive_remainder(left.to_bits(), right.to_bits())
                    .is_err()
            );
        }
    }

    #[test]
    fn negative_normalization_binds_relocated_operands_setter_and_arithmetic_calls() {
        assert_eq!(
            decode(&fixture())
                .unwrap()
                .normalize(&numeric(), -0.5)
                .unwrap(),
            11.5
        );
        for (va, at) in [
            (0x8b14b9, 1),
            (0x8bc3dd, 1),
            (0x8bc3dd, 12),
            (0x8aeaec, 3),
            (0x8aeaec, 7),
            (0x8aeaec, 10),
            (0x8aeaec, 23),
            (0x8aeb10, 4),
            (0x8aeb10, 12),
            (0x8aeb10, 15),
            (0x8aeb10, 28),
            (0x401300, 1),
            (0x401300, 3),
            (0x401300, 8),
        ] {
            let mut drift = fixture();
            drift.iter_mut().find(|(base, _)| *base == va).unwrap().1[at] ^= 1;
            assert!(decode(&drift).is_err(), "{va:x}+{at}");
        }
    }

    #[test]
    fn unsupported_heading_initializers_and_ranges_refuse() {
        for text in [b"".as_slice(), b"-1", b"1e2", b"1.2.3", b"1234567890", b"."] {
            assert!(decimal(text, &numeric()).is_err());
        }
        for (va, value) in [
            (0x70400, 6.0_f32),
            (0x70410, 0.5),
            (0x70420, 13.),
            (0x70430, f32::NAN),
        ] {
            let mut memory = fixture();
            memory.iter_mut().find(|(base, _)| *base == va).unwrap().1 =
                value.to_bits().to_le_bytes().to_vec();
            assert!(decode(&memory).is_err());
        }
    }

    pub(crate) fn selected_numeric_image() -> (
        Vec<u8>,
        crate::scripts::AssetIdentity,
        NumericPolicy,
        crate::neutral_player::RuleInputBudget,
    ) {
        use crate::{
            camera_margins::SourceWindow,
            neutral_player::{RuleInputBudget, sha256},
            neutral_rule_input::Pe,
        };
        use std::io::Read;
        let budget = RuleInputBudget {
            image_bytes: 32 << 20,
            profile_bytes: 64 << 10,
            sections: 96,
        };
        let mut image = vec![];
        std::fs::File::open(std::env::var_os("PCX_WAR3_EXE").expect("PCX_WAR3_EXE"))
            .unwrap()
            .take(budget.image_bytes as u64 + 1)
            .read_to_end(&mut image)
            .unwrap();
        assert_eq!(sha256(&image), crate::menu_lighting::REVIEWED_IMAGE);
        let pe = Pe::parse(&image, budget).unwrap();
        let window = |va, len, code| SourceWindow {
            va,
            len,
            digest: sha256(&pe.read_from(va, code).unwrap()[..len as usize]),
        };
        let origin = crate::scripts::AssetIdentity {
            source: "explicit local heading client".into(),
            edition: "classic".into(),
            version: "1.29.2.9231".into(),
        };
        let numeric = NumericPolicy::read(
            &image,
            crate::script_numeric::Binding {
                origin: origin.clone(),
                image_sha256: sha256(&image),
                literal: window(0x8df660, 238, true),
                division: window(0x9a02d0, 57, true),
                reciprocal: window(0x9a1d00, 103, true),
                reciprocal_table: window(0xed6cf0, 4100, false),
                equality: window(0x8d8d10, 53, true),
                equality_constant: window(0x10db054, 4, false),
                inequality: window(0x8d7e32, 162, true),
                math_sub: window(0x9a00b0, 264, true),
                math_add: window(0x9a01c0, 262, true),
                math_mul: crate::campaign_client::numeric_multiplication(&image).unwrap(),
                math_lzcnt: window(0x9a0330, 29, true),
                math_i32_to_real: window(0x9a1620, 92, true),
            },
            budget,
            4096,
        )
        .unwrap()
        .with_math(
            &image,
            &crate::script_math::Binding {
                image_sha256: sha256(&image),
                square_root_native: window(0x4ac1a0, 94, true),
                arctangent2_native: window(0x489fd0, 126, true),
                square_root: window(0x9a20a0, 164, true),
                integer_root: window(0x9a2150, 61, true),
                arctangent2: window(0x9a0cf0, 161, true),
                arctangent: window(0x9a0da0, 423, true),
                zero_initializer: window(0x401300, 16, true),
                one_initializer: window(0x401460, 16, true),
                trig: Some(crate::script_math::TrigBinding {
                    sine_native: window(0x4ac180, 26, true),
                    cosine_native: window(0x48afc0, 26, true),
                    sine: window(0x9a1da0, 288, true),
                    cosine: window(0x9a0fe0, 289, true),
                }),
            },
            budget,
        )
        .unwrap()
        .with_timer_queries(
            &image,
            &crate::script_numeric::timer_query::Binding {
                image_sha256: sha256(&image),
                segment_initializer: window(0x40d210, 16, true),
                zero_initializer: window(0x401300, 16, true),
                constructor: window(0x4fe380, 0xd1, true),
                start: window(0x7fb820, 0x118, true),
                schedule: window(0x7fb940, 0x5c, true),
                remainder: window(0x9a1a90, 0x142, true),
                remaining: window(0x7fc0b0, 0x104, true),
                full_timeout: window(0x7fbad0, 0xbe, true),
                active_elapsed: window(0x7fbb90, 0x34, true),
                inactive_elapsed: window(0x4fe470, 0x6f, true),
                pause: window(0x4fe640, 0x46, true),
                resume: window(0x4fe6b0, 0x38, true),
                rearm: window(0x4fe5d0, 0x6a, true),
                stop: window(0x7fbbd0, 0x3c, true),
                dispatch: window(0x4fdf90, 0x4a, true),
                expiry_dispatch: window(0x7fc390, 0x16b, true),
            },
            budget,
        )
        .unwrap()
        .with_native_game_clock(&image, budget)
        .unwrap();
        (image, origin, numeric, budget)
    }

    #[test]
    #[ignore = "requires PCX_WAR3_EXE pointing to the reviewed local client"]
    fn selected_heading_source_and_stationary_steps_bind() {
        use crate::neutral_player::RuleInputBudget;
        let (image, origin, numeric, budget) = selected_numeric_image();
        let registry = crate::campaign_client::order_registry(&image, origin.clone()).unwrap();
        let rules = crate::campaign_client::unit_activity(&image, origin, &registry)
            .unwrap()
            .with_heading(&image, &numeric, budget)
            .unwrap();
        let heading = rules.heading_rules().unwrap();
        assert!(heading.period < heading.window);
        let first = heading
            .stationary_step(&numeric, 0., heading.half_turn, heading.window)
            .unwrap();
        assert!(!first.arrived);
        assert_eq!(first.logical.to_bits(), heading.window.to_bits());
        assert!(
            rules
                .clone()
                .with_heading(
                    &image,
                    &numeric,
                    RuleInputBudget {
                        profile_bytes: 1,
                        ..budget
                    }
                )
                .is_err()
        );
        let mut corrupt = image.clone();
        corrupt[0] ^= 1;
        assert!(rules.with_heading(&corrupt, &numeric, budget).is_err());
    }
}

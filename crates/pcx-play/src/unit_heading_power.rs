//! The selected orientation curve's nonnegative real power on a unit ratio.
use super::{call, shape, word};
use crate::{script_numeric::NumericPolicy, script_random::real};

#[derive(Clone, Debug)]
pub struct Power {
    one: u32,
    negative_one: u32,
    thresholds: [u32; 2],
    scales: [u32; 2],
    offsets: [u32; 2],
    rational: [u32; 2],
    log_factors: [u32; 2],
    exp_base: u32,
    exp_coefficients: [u32; 4],
}

impl Power {
    pub fn read(
        read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
        numeric: &NumericPolicy,
    ) -> Result<Self, String> {
        for (base, at, target) in [
            (0x8b0a63, 0, 0x9a1be0),
            (0x9a1be0, 0x82, 0x9a1960),
            (0x9a1be0, 0x97, 0x9a14c0),
            (0x9a1960, 0x3c, 0x9a0350),
        ] {
            call(&read(base, at + 5, true)?, base, at, target)?;
        }
        let mut operand = |site, prefix: &[u8]| -> Result<u32, String> {
            let bytes = read(site, prefix.len() + 4, true)?;
            shape(&bytes, 0, prefix)?;
            let address = word(&bytes, prefix.len())?;
            let value = word(&read(address, 4, false)?, 0)?;
            real::from_f32(f32::from_bits(value)).map_err(|e| e.to_string())?;
            Ok(value)
        };
        let thresholds = [
            operand(0x9a198e, &[0x0f, 0x2f, 0x05])?,
            operand(0x9a19a6, &[0x0f, 0x2f, 0x05])?,
        ];
        let scales = [operand(0x9a19b4, &[0x68])?, operand(0x9a19ce, &[0x68])?];
        let offsets = [operand(0x9a19af, &[0x68])?, operand(0x9a19c9, &[0x68])?];
        let rational = [operand(0x9a03bc, &[0x68])?, operand(0x9a0403, &[0x68])?];
        let log_factors = [operand(0x9a1a49, &[0x68])?, operand(0x9a1a6d, &[0x68])?];
        let exp_base = operand(0x9a05bb, &[0x68])?;
        let exp_coefficients = [
            operand(0x9a04e5, &[0x68])?,
            operand(0x9a04eb, &[0x68])?,
            operand(0x9a04f1, &[0x68])?,
            operand(0x9a04fd, &[0x68])?,
        ];
        let mut initializer = |base, use_site| -> Result<u32, String> {
            let init = read(base, 16, true)?;
            let (destination, value) = numeric
                .integer_initializer(&init, base)
                .ok_or("heading power initializer")?;
            let used = read(use_site, 5, true)?;
            shape(&used, 0, &[0x68])?;
            if word(&used, 1)? != destination {
                return Err("heading power initializer destination differs".into());
            }
            Ok(value.to_bits())
        };
        let one = initializer(0x401460, 0x9a035e)?;
        let negative_one = initializer(0x4017c0, 0x9a036a)?;
        Ok(Self {
            one,
            negative_one,
            thresholds,
            scales,
            offsets,
            rational,
            log_factors,
            exp_base,
            exp_coefficients,
        })
    }

    fn log_kernel(&self, numeric: &NumericPolicy, x: u32) -> Result<u32, real::MathError> {
        let t = numeric.division(real::add(x, self.negative_one)?, real::add(x, self.one)?)?;
        let square = real::mul(t, t)?;
        let numerator = real::mul(
            t,
            real::add(self.one, real::mul(self.rational[0], square)?)?,
        )?;
        // Doubling a normal binary32 value changes its exponent by one.
        let numerator = numerator.wrapping_add(if numerator & 0x7f80_0000 != 0 {
            1 << 23
        } else {
            0
        });
        numeric.division(
            numerator,
            real::add(self.one, real::mul(self.rational[1], square)?)?,
        )
    }

    fn logarithm(&self, numeric: &NumericPolicy, x: u32) -> Result<u32, real::MathError> {
        let exponent = ((x >> 23) & 0xff) as i32 - 127;
        let mantissa = x & 0x7f_ffff | 127 << 23;
        let value = if f32::from_bits(mantissa) >= f32::from_bits(self.thresholds[0]) {
            self.log_kernel(numeric, mantissa)?
        } else {
            let index = usize::from(f32::from_bits(mantissa) < f32::from_bits(self.thresholds[1]));
            real::add(
                self.log_kernel(numeric, real::mul(mantissa, self.scales[index])?)?,
                self.offsets[index],
            )?
        };
        real::mul(
            self.log_factors[1],
            real::add(
                real::from_i32(exponent)?,
                real::mul(value, self.log_factors[0])?,
            )?,
        )
    }

    fn integer_power(mut x: u32, mut exponent: u32) -> Result<u32, real::MathError> {
        let mut result = 1_f32.to_bits();
        while exponent != 0 {
            if exponent & 1 != 0 {
                result = real::mul(result, x)?;
            }
            x = real::mul(x, x)?;
            exponent >>= 1;
        }
        Ok(result)
    }

    fn positive_exp(&self, x: u32) -> Result<u32, real::MathError> {
        let scaled = x.wrapping_add(if x & 0x7f80_0000 != 0 { 2 << 23 } else { 0 });
        let exponent = ((scaled >> 23) & 0xff) as i32 - 127;
        if exponent >= 23 {
            return Err(real::MathError::Unsupported);
        }
        let integral = if exponent < 0 {
            0
        } else {
            scaled & (u32::MAX << (23 - exponent))
        };
        let count = f32::from_bits(integral) as u32;
        let fraction = real::sub(scaled, integral)?;
        let remainder = if fraction & 0x7f80_0000 != 0 {
            fraction.wrapping_sub(2 << 23)
        } else {
            0
        };
        real::from_f32(f32::from_bits(remainder))?;
        let mut polynomial = self.exp_coefficients[3];
        for coefficient in self.exp_coefficients[..3]
            .iter()
            .rev()
            .chain([&self.one, &self.one])
        {
            polynomial = real::add(*coefficient, real::mul(remainder, polynomial)?)?;
        }
        real::mul(polynomial, Self::integer_power(self.exp_base, count)?)
    }

    /// Finite normal ratio and nonnegative finite exponent; unsupported ranges refuse.
    pub fn ratio(&self, numeric: &NumericPolicy, ratio: f32, exponent: f32) -> Result<f32, String> {
        if !(0.0..=1.0).contains(&ratio)
            || !exponent.is_finite()
            || exponent < 0.0
            || exponent >= i32::MAX as f32
        {
            return Err("heading power requires a unit ratio and nonnegative exponent".into());
        }
        let result = (|| {
            let base = real::from_f32(ratio)?;
            let power = real::from_f32(exponent)?;
            if exponent.fract() == 0.0 {
                return Self::integer_power(base, exponent as u32);
            }
            if base & 0x7f80_0000 == 0 {
                return Ok(0);
            }
            let log = real::mul(power, self.logarithm(numeric, base)?)?;
            if f32::from_bits(log) < 0.0 {
                Ok(numeric.reciprocal(self.positive_exp(log ^ (1 << 31))?))
            } else {
                self.positive_exp(log)
            }
        })()
        .map_err(|e: real::MathError| format!("heading power: {e}"))?;
        real::from_f32(f32::from_bits(result)).map_err(|e| format!("heading power: {e}"))?;
        Ok(f32::from_bits(result))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    type Memory = Vec<(u32, Vec<u8>)>;
    fn fixture() -> (Memory, NumericPolicy) {
        let (image, binding, budget) = crate::script_numeric::tests::fixture();
        let converter = binding.math_i32_to_real.va;
        let numeric = NumericPolicy::read(&image, binding, budget, 64).unwrap();
        let mut memory: Memory = [
            (0x8b0a63, 5),
            (0x9a1be0, 0xa0),
            (0x9a1960, 0x120),
            (0x9a0350, 0x100),
            (0x9a0460, 0x180),
        ]
        .into_iter()
        .map(|(va, n)| (va, vec![0x90; n]))
        .collect();
        let put = |memory: &mut Memory, va, bytes: &[u8]| {
            if let Some((base, current)) = memory.iter_mut().find(|(base, current)| {
                va >= *base && (va - base) as usize + bytes.len() <= current.len()
            }) {
                let at = (va - *base) as usize;
                current[at..at + bytes.len()].copy_from_slice(bytes);
            } else {
                memory.push((va, bytes.to_vec()));
            }
        };
        for (base, at, target) in [
            (0x8b0a63_u32, 0, 0x9a1be0_u32),
            (0x9a1be0, 0x82, 0x9a1960),
            (0x9a1be0, 0x97, 0x9a14c0),
            (0x9a1960, 0x3c, 0x9a0350),
        ] {
            let mut bytes = vec![0xe8];
            bytes.extend(target.wrapping_sub(base + at + 5).to_le_bytes());
            put(&mut memory, base + at, &bytes);
        }
        for (i, (site, prefix, value)) in [
            (0x9a198e, &[0x0f, 0x2f, 0x05][..], 1.75_f32),
            (0x9a19a6, &[0x0f, 0x2f, 0x05][..], 1.25),
            (0x9a19b4, &[0x68][..], 1.5),
            (0x9a19ce, &[0x68][..], 1.75),
            (0x9a19af, &[0x68][..], -0.5),
            (0x9a19c9, &[0x68][..], -0.75),
            (0x9a03bc, &[0x68][..], 0.3125),
            (0x9a0403, &[0x68][..], -0.125),
            (0x9a1a49, &[0x68][..], 1.25),
            (0x9a1a6d, &[0x68][..], 0.625),
            (0x9a05bb, &[0x68][..], 1.5),
            (0x9a04e5, &[0x68][..], 0.25),
            (0x9a04eb, &[0x68][..], 0.125),
            (0x9a04f1, &[0x68][..], 0.0625),
            (0x9a04fd, &[0x68][..], 0.03125),
        ]
        .into_iter()
        .enumerate()
        {
            let address = 0x93000 + i as u32 * 16;
            let mut instruction = prefix.to_vec();
            instruction.extend(address.to_le_bytes());
            put(&mut memory, site, &instruction);
            put(&mut memory, address, &value.to_bits().to_le_bytes());
        }
        for (base, site, address, value) in [
            (0x401460_u32, 0x9a035e, 0x96000_u32, 1_i8),
            (0x4017c0, 0x9a036a, 0x96010, -1),
        ] {
            let mut init = vec![0x6a, value as u8, 0x68];
            init.extend(address.to_le_bytes());
            init.push(0xe8);
            init.extend(converter.wrapping_sub(base + 12).to_le_bytes());
            init.extend([0x83, 0xc4, 8, 0xc3]);
            put(&mut memory, base, &init);
            let mut operand = vec![0x68];
            operand.extend(address.to_le_bytes());
            put(&mut memory, site, &operand);
        }
        (memory, numeric)
    }

    fn decode(memory: &Memory, numeric: &NumericPolicy) -> Result<Power, String> {
        Power::read(
            &mut |va, n, _| {
                memory
                    .iter()
                    .find_map(|(base, data)| {
                        va.checked_sub(*base)
                            .and_then(|at| data.get(at as usize..at as usize + n))
                            .map(<[u8]>::to_vec)
                    })
                    .ok_or_else(|| "power synthetic source mapping".into())
            },
            numeric,
        )
    }

    pub(crate) fn nonstock_policy() -> (Power, NumericPolicy) {
        let (memory, numeric) = fixture();
        (decode(&memory, &numeric).unwrap(), numeric)
    }

    #[test]
    fn nonstock_power_coefficients_follow_relocated_source_operands() {
        let (mut memory, numeric) = fixture();
        let power = decode(&memory, &numeric).unwrap();
        assert_eq!(power.thresholds.map(f32::from_bits), [1.75, 1.25]);
        assert_eq!(power.rational.map(f32::from_bits), [0.3125, -0.125]);
        assert_eq!(
            power.exp_coefficients.map(f32::from_bits),
            [0.25, 0.125, 0.0625, 0.03125]
        );
        let operand = memory.iter_mut().find(|(va, _)| *va == 0x93000).unwrap();
        operand.1 = 2.5_f32.to_bits().to_le_bytes().to_vec();
        assert_eq!(
            f32::from_bits(decode(&memory, &numeric).unwrap().thresholds[0]),
            2.5
        );
        assert_eq!(power.ratio(&numeric, 0.75, 2.).unwrap(), 0.5625);
        assert_eq!(power.ratio(&numeric, 0., 0.).unwrap(), 1.);
    }

    #[test]
    fn changed_power_calls_initializers_and_unsupported_numbers_refuse() {
        for (base, at) in [(0x8b0a63, 1), (0x9a1be0, 0x83), (0x401460, 3)] {
            let (mut memory, numeric) = fixture();
            memory.iter_mut().find(|(va, _)| *va == base).unwrap().1[at] ^= 1;
            assert!(decode(&memory, &numeric).is_err());
        }
        let (memory, numeric) = fixture();
        let power = decode(&memory, &numeric).unwrap();
        for (ratio, exponent) in [
            (f32::NAN, 1.),
            (1.5, 1.),
            (0.5, -1.),
            (0.5, f32::INFINITY),
            (0.5, i32::MAX as f32),
            (f32::from_bits(1), 0.5),
        ] {
            assert!(power.ratio(&numeric, ratio, exponent).is_err());
        }
    }

    #[test]
    #[ignore = "requires PCX_WAR3_EXE and PCX_HEADING_POWER_WITNESS from a local original-client probe"]
    fn selected_heading_power_matches_retained_native_binary32_witness() {
        use std::io::Read;
        let (image, _, numeric, budget) = super::super::tests::selected_numeric_image();
        let pe = crate::neutral_rule_input::Pe::parse(&image, budget).unwrap();
        let power = Power::read(
            &mut |va, n, code| {
                pe.read_from(va, code)
                    .map_err(|e| e.to_string())?
                    .get(..n)
                    .map(<[u8]>::to_vec)
                    .ok_or_else(|| "heading power truncated witness source".into())
            },
            &numeric,
        )
        .unwrap();
        let mut bytes = Vec::new();
        std::fs::File::open(
            std::env::var_os("PCX_HEADING_POWER_WITNESS").expect("PCX_HEADING_POWER_WITNESS"),
        )
        .unwrap()
        .take((64 << 10) + 1)
        .read_to_end(&mut bytes)
        .unwrap();
        assert!(bytes.len() <= 64 << 10);
        let witness: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let hash: String = crate::neutral_player::sha256(&image)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(witness["image_sha256"], hash);
        assert_eq!(witness["roc"], true);
        let cases = witness["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 65);
        for (index, case) in cases.iter().enumerate() {
            assert_eq!(case["index"], index);
            let bits = |field| u32::try_from(case[field].as_u64().unwrap()).unwrap();
            let actual = power
                .ratio(
                    &numeric,
                    f32::from_bits(bits("base")),
                    f32::from_bits(bits("exponent")),
                )
                .unwrap();
            assert_eq!(
                actual.to_bits(),
                bits("result"),
                "native power witness {index}"
            );
        }
    }
}

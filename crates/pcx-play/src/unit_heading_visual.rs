//! The native orientation phase follows logical movement and its completion callbacks.
use super::{Rules as HeadingRules, call, decimal, power::Power, shape, word};
use crate::{script_numeric::NumericPolicy, script_random::real};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct State {
    pub logical: f32,
    pub angle: f32,
    pub velocity: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Curve {
    pub acceleration: f32,
    pub maximum: f32,
    pub ramp: f32,
    pub power: f32,
}

#[derive(Clone, Debug)]
pub struct Rules {
    pub curves: Vec<Curve>,
    pub tolerance: f32,
    zero: f32,
    one: f32,
    power: Power,
}

#[derive(Clone, Copy)]
enum Operand {
    Address(u32),
    Slot(u8),
    Number(f32),
    Saved,
}

/// Interpret only the reviewed straight-line decimal initializer's stack/register forms.
fn curves(
    code: &[u8],
    base: u32,
    table: u32,
    count: usize,
    read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
    numeric: &NumericPolicy,
) -> Result<Vec<Curve>, String> {
    let mut stack = Vec::new();
    let mut registers = [Operand::Saved; 8];
    let mut slots = BTreeMap::new();
    let mut output = BTreeMap::new();
    let mut at = 0;
    let number = |operand: Operand, slots: &BTreeMap<u8, f32>| -> Result<f32, String> {
        match operand {
            Operand::Number(v) => Ok(v),
            Operand::Slot(slot) => slots
                .get(&slot)
                .copied()
                .ok_or_else(|| "orientation uninitialized slot".into()),
            _ => Err("orientation unsupported initializer operand".into()),
        }
    };
    while at < code.len() {
        let next = code.get(at..).ok_or("orientation initializer range")?;
        match next[0] {
            0x50..=0x57 => {
                stack.push(registers[usize::from(next[0] - 0x50)]);
                at += 1;
            }
            0x58..=0x5f => {
                registers[usize::from(next[0] - 0x58)] =
                    stack.pop().ok_or("orientation stack underflow")?;
                at += 1;
            }
            0x68 => {
                stack.push(Operand::Address(word(code, at + 1)?));
                at += 5;
            }
            0x8d => {
                shape(code, at, &[0x8d, 0x45])?;
                registers[0] =
                    Operand::Slot(*code.get(at + 2).ok_or("orientation slot truncated")?);
                at += 3;
            }
            0x8b => {
                let mode = *code.get(at + 1).ok_or("orientation move truncated")?;
                if [0xec, 0xe5].contains(&mode) {
                    at += 2;
                    continue;
                }
                match mode {
                    0xd8 => registers[3] = registers[0],
                    0xf0 => registers[6] = registers[0],
                    0xf8 => registers[7] = registers[0],
                    0x08 => registers[1] = Operand::Number(number(registers[0], &slots)?),
                    0x03 => registers[0] = Operand::Number(number(registers[3], &slots)?),
                    0x06 => registers[0] = Operand::Number(number(registers[6], &slots)?),
                    0x07 => registers[0] = Operand::Number(number(registers[7], &slots)?),
                    _ => return Err("orientation unsupported register move".into()),
                }
                at += 2;
            }
            0x83 => {
                let mode = *code
                    .get(at + 1)
                    .ok_or("orientation stack adjustment truncated")?;
                let amount = usize::from(
                    *code
                        .get(at + 2)
                        .ok_or("orientation stack adjustment truncated")?,
                );
                match mode {
                    0xec if amount == 16 => {}
                    0xc4 if amount % 4 == 0 => {
                        let keep = stack
                            .len()
                            .checked_sub(amount / 4)
                            .ok_or("orientation stack release")?;
                        stack.truncate(keep);
                    }
                    _ => return Err("orientation unsupported stack adjustment".into()),
                }
                at += 3;
            }
            0xe8 => {
                call(code, base, at, 0x9a1760)?;
                let args = stack
                    .get(
                        stack
                            .len()
                            .checked_sub(2)
                            .ok_or("orientation call arguments")?..,
                    )
                    .ok_or("orientation call arguments")?;
                let [Operand::Address(string), Operand::Slot(slot)] = args else {
                    return Err("orientation decimal call operands differ".into());
                };
                let bytes = read(*string, 17, false)?;
                let end = bytes
                    .iter()
                    .position(|b| *b == 0)
                    .ok_or("orientation unterminated decimal")?;
                slots.insert(*slot, decimal(&bytes[..end], numeric)?);
                registers[0] = Operand::Slot(*slot);
                at += 5;
            }
            0x89 | 0xa3 => {
                let (offset, register, len) = if next[0] == 0x89 {
                    shape(code, at, &[0x89, 0x0d])?;
                    (at + 2, 1, 6)
                } else {
                    (at + 1, 0, 5)
                };
                let address = word(code, offset)?;
                let index = address
                    .checked_sub(table)
                    .ok_or("orientation destination precedes table")?
                    as usize;
                if !index.is_multiple_of(4)
                    || index / 4 >= count * 4
                    || output
                        .insert(index / 4, number(registers[register], &slots)?)
                        .is_some()
                {
                    return Err("orientation duplicate or invalid destination".into());
                }
                at += len;
            }
            0xc3 if at + 1 == code.len() => {
                at += 1;
            }
            _ => {
                return Err(format!(
                    "orientation unsupported initializer at {:#x}",
                    base + at as u32
                ));
            }
        }
        if stack.len() > 32 || slots.len() > 4 {
            return Err("orientation initializer work bound".into());
        }
    }
    if output.len() != count * 4 {
        return Err("orientation incomplete curve table".into());
    }
    (0..count)
        .map(|index| {
            let value = |field| {
                output
                    .get(&(index * 4 + field))
                    .copied()
                    .ok_or("orientation curve cell missing")
            };
            Ok(Curve {
                acceleration: value(0)?,
                maximum: value(1)?,
                ramp: value(2)?,
                power: value(3)?,
            })
        })
        .collect::<Result<_, &str>>()
        .map_err(str::to_owned)
}

impl Rules {
    pub fn read(
        read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
        numeric: &NumericPolicy,
    ) -> Result<Self, String> {
        let index = read(0x8b0920, 20, true)?;
        shape(&index, 0, &[0x83, 0xe7])?;
        shape(&index, 10, &[0xc1, 0xe7, 4, 0x50, 0x81, 0xc7])?;
        let count = usize::from(index[2]) + 1;
        if count > 256 {
            return Err("orientation curve count exceeds selector".into());
        }
        let table = word(&index, 16)?;
        let consumer = read(0x8b094d, 8, true)?;
        shape(&consumer, 0, &[0xf3, 0x0f, 0x10, 0x05])?;
        let init = read(0x4109d0, 19, true)?;
        shape(&init, 0, &[0x68])?;
        shape(&init, 5, &[0x68])?;
        call(&init, 0x4109d0, 10, 0x9a1760)?;
        if word(&init, 6)? != word(&consumer, 4)? {
            return Err("orientation tolerance initializer differs".into());
        }
        let text = read(word(&init, 1)?, 17, false)?;
        let end = text
            .iter()
            .position(|b| *b == 0)
            .ok_or("orientation tolerance unterminated")?;
        let tolerance = decimal(&text[..end], numeric)?;
        let mut scalar = |base, use_site, prefix: &[u8]| -> Result<f32, String> {
            let (destination, value) = numeric
                .integer_initializer(&read(base, 16, true)?, base)
                .ok_or("orientation scalar initializer")?;
            let use_bytes = read(use_site, prefix.len() + 4, true)?;
            shape(&use_bytes, 0, prefix)?;
            if word(&use_bytes, prefix.len())? != destination {
                return Err("orientation scalar destination differs".into());
            }
            Ok(value)
        };
        let zero = scalar(0x401300, 0x8b0939, &[0xf3, 0x0f, 0x10, 0x05])?;
        let one = scalar(0x401460, 0x8b0a1a, &[0xf3, 0x0f, 0x10, 0x0d])?;
        let code = read(0x4109f0, 0x5d8, true)?;
        let curves = curves(&code, 0x4109f0, table, count, read, numeric)?;
        if one <= zero
            || !tolerance.is_finite()
            || tolerance <= zero
            || curves.iter().any(|c| {
                c.acceleration < zero || c.maximum < zero || c.ramp <= zero || c.power < zero
            })
        {
            return Err("orientation source curve range invalid".into());
        }
        Ok(Self {
            curves,
            tolerance,
            zero,
            one,
            power: Power::read(read, numeric)?,
        })
    }

    pub fn step(
        &self,
        heading: &HeadingRules,
        numeric: &NumericPolicy,
        state: State,
        index: usize,
    ) -> Result<State, String> {
        let curve = self
            .curves
            .get(index)
            .ok_or("orientation selector not admitted")?;
        if ![state.angle, state.logical, state.velocity]
            .iter()
            .all(|v| v.is_finite())
            || !(self.zero..heading.full_turn).contains(&state.angle)
            || !(self.zero..heading.full_turn).contains(&state.logical)
        {
            return Err("orientation state is not normalized and finite".into());
        }
        let result = (|| {
            let delta = heading.delta(state.angle, state.logical)?;
            if delta.abs() < self.tolerance {
                return Ok(State {
                    angle: state.logical,
                    velocity: self.zero,
                    ..state
                });
            }
            let accelerated = f32::from_bits(real::add(
                state.velocity.to_bits(),
                curve.acceleration.copysign(delta).to_bits(),
            )?);
            let mut maximum = delta.abs().min(curve.maximum);
            let ratio = if delta.abs() >= curve.ramp {
                self.one
            } else {
                f32::from_bits(numeric.division(delta.abs().to_bits(), curve.ramp.to_bits())?)
            }
            .clamp(self.zero, self.one);
            if ratio != self.one {
                let power = self
                    .power
                    .ratio(numeric, ratio, curve.power)
                    .map_err(|_| real::MathError::Unsupported)?;
                maximum = f32::from_bits(real::mul(maximum.to_bits(), power.to_bits())?);
            }
            let magnitude = accelerated.abs().min(maximum);
            let velocity = if accelerated < self.zero {
                -magnitude
            } else {
                magnitude
            };
            let angle = heading.normalize(
                numeric,
                f32::from_bits(real::add(state.angle.to_bits(), velocity.to_bits())?),
            )?;
            Ok(State {
                angle,
                velocity,
                ..state
            })
        })();
        result.map_err(|e: real::MathError| format!("orientation step: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires PCX_WAR3_EXE and PCX_HEADING_VISUAL_WITNESS from private read-only native observations"]
    fn selected_orientation_step_matches_stable_native_state_transitions() {
        use std::io::Read;
        let (image, origin, numeric, budget) = super::super::tests::selected_numeric_image();
        let registry = crate::campaign_client::order_registry(&image, origin.clone()).unwrap();
        let activity = crate::campaign_client::unit_activity(&image, origin, &registry)
            .unwrap()
            .with_heading(&image, &numeric, budget)
            .unwrap();
        let heading = activity.heading_rules().unwrap();
        let visual = heading.visual.as_ref().unwrap();
        let mut bytes = Vec::new();
        std::fs::File::open(
            std::env::var_os("PCX_HEADING_VISUAL_WITNESS").expect("PCX_HEADING_VISUAL_WITNESS"),
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
        let cases = witness["cases"].as_array().unwrap();
        assert!(!cases.is_empty());
        assert_eq!(witness["counts"]["candidate_transitions"], cases.len());
        for case in cases {
            let bits = |field| u32::try_from(case[field].as_u64().unwrap()).unwrap();
            assert_eq!(case["cycle_before"], case["cycle_after"]);
            let elapsed =
                f32::from_bits(real::sub(bits("clock_after"), bits("clock_before")).unwrap());
            assert!(
                elapsed >= 0. && elapsed < heading.period,
                "more than one frame could fit in the observation interval"
            );
            let before_selector = (bits("flags_before") >> 8) & (visual.curves.len() as u32 - 1);
            let after_selector = (bits("flags_after") >> 8) & (visual.curves.len() as u32 - 1);
            assert_eq!(before_selector, after_selector);
            let state = State {
                logical: f32::from_bits(bits("logical")),
                angle: f32::from_bits(bits("angle")),
                velocity: f32::from_bits(bits("velocity")),
            };
            let actual = visual
                .step(heading, &numeric, state, after_selector as usize)
                .unwrap();
            assert_eq!(
                actual.angle.to_bits(),
                bits("expected_angle"),
                "native orientation angle at sample {}",
                case["ordinal"]
            );
            assert_eq!(
                actual.velocity.to_bits(),
                bits("expected_velocity"),
                "native angular velocity at sample {}",
                case["ordinal"]
            );
        }
        println!(
            "Compared {} stable native orientation transitions; source exclusions {}",
            cases.len(),
            witness["counts"]
        );
    }

    fn constructor() -> (Vec<u8>, Vec<(u32, Vec<u8>)>) {
        let mut code = vec![0x55, 0x8b, 0xec, 0x83, 0xec, 16, 0x53, 0x56, 0x57];
        let mut memory = Vec::new();
        for curve in 0..2_u32 {
            for (field, slot, register) in [
                (3, 0xfc, 0xd8),
                (2, 0xf8, 0xf8),
                (1, 0xf4, 0xf0),
                (0, 0xf0, 0x08),
            ] {
                let address = 0x61000_u32 + (curve * 4 + field) * 32;
                let mut text = format!("{}\0", curve * 4 + field + 1).into_bytes();
                text.resize(17, b'x');
                memory.push((address, text));
                code.extend([0x8d, 0x45, slot, 0x68]);
                code.extend(address.to_le_bytes());
                code.extend([0x50, 0xe8]);
                let call_at = 0x21000 + code.len() as u32 - 1;
                code.extend(0x9a1760_u32.wrapping_sub(call_at + 5).to_le_bytes());
                code.extend([0x8b, register]);
            }
            code.extend([0x89, 0x0d]);
            code.extend((0x52000 + curve * 16).to_le_bytes());
            for (field, register) in [(1, 0x06), (2, 0x07), (3, 0x03)] {
                code.extend([0x8b, register, 0xa3]);
                code.extend((0x52000 + curve * 16 + field * 4).to_le_bytes());
            }
            code.extend([0x83, 0xc4, 32]);
        }
        code.extend([0x5f, 0x5e, 0x5b, 0x8b, 0xe5, 0x5d, 0xc3]);
        (code, memory)
    }

    #[test]
    fn nonstock_orientation_constructor_tracks_slots_destinations_and_rejects_drift() {
        let (_, numeric) = super::super::power::tests::nonstock_policy();
        let (code, memory) = constructor();
        let decode = |code: &[u8]| {
            curves(
                code,
                0x21000,
                0x52000,
                2,
                &mut |va, n, _| {
                    memory
                        .iter()
                        .find(|(base, _)| *base == va)
                        .and_then(|(_, bytes)| bytes.get(..n))
                        .map(<[u8]>::to_vec)
                        .ok_or_else(|| "orientation fixture string".into())
                },
                &numeric,
            )
        };
        let result = decode(&code).unwrap();
        assert_eq!(result.len(), 2);
        // The synthetic numeric policy deliberately carries a nonstock reciprocal table.
        for (index, curve) in result.iter().enumerate() {
            for (field, actual) in [curve.acceleration, curve.maximum, curve.ramp, curve.power]
                .into_iter()
                .enumerate()
            {
                let text = (index * 4 + field + 1).to_string();
                assert_eq!(
                    actual.to_bits(),
                    decimal(text.as_bytes(), &numeric).unwrap().to_bits()
                );
            }
        }
        let mut changed = code.clone();
        changed[9] = 0xcc;
        assert!(decode(&changed).is_err());
        let mut changed = code.clone();
        let at = changed.windows(2).position(|b| b == [0x89, 0x0d]).unwrap();
        changed[at + 2..at + 6].copy_from_slice(&0x52004_u32.to_le_bytes());
        assert!(decode(&changed).is_err(), "duplicate table destination");
        assert!(decode(&code[..code.len() - 20]).is_err());
    }

    #[test]
    fn nonstock_orientation_retains_inertia_wraps_and_snaps_without_mutating_on_error() {
        let (power, numeric) = super::super::power::tests::nonstock_policy();
        let visual = Rules {
            curves: vec![Curve {
                acceleration: 0.125,
                maximum: 0.25,
                ramp: 0.5,
                power: 0.,
            }],
            tolerance: 0.03125,
            zero: 0.,
            one: 1.,
            power,
        };
        let heading = HeadingRules {
            period: 1.,
            window: 0.25,
            tolerance: 0.03125,
            half_turn: 6.,
            full_turn: 12.,
            visual: None,
        };
        let state = State {
            logical: 1.,
            angle: 0.,
            velocity: 0.,
        };
        let first = visual.step(&heading, &numeric, state, 0).unwrap();
        assert_eq!(
            first,
            State {
                logical: 1.,
                angle: 0.125,
                velocity: 0.125
            }
        );
        assert_eq!(
            visual.step(&heading, &numeric, first, 0).unwrap(),
            State {
                logical: 1.,
                angle: 0.375,
                velocity: 0.25
            }
        );
        let reverse = State {
            logical: 11.,
            ..first
        };
        assert_eq!(
            visual
                .step(&heading, &numeric, reverse, 0)
                .unwrap()
                .velocity
                .to_bits(),
            0_f32.to_bits()
        );
        let wrapped = visual
            .step(
                &heading,
                &numeric,
                State {
                    logical: 11.,
                    angle: 0.,
                    velocity: 0.,
                },
                0,
            )
            .unwrap();
        assert_eq!(
            wrapped,
            State {
                logical: 11.,
                angle: 11.875,
                velocity: -0.125
            }
        );
        let near = State {
            logical: 1.,
            angle: 0.984375,
            velocity: 0.25,
        };
        assert_eq!(
            visual.step(&heading, &numeric, near, 0).unwrap(),
            State {
                logical: 1.,
                angle: 1.,
                velocity: 0.
            }
        );
        assert!(visual.step(&heading, &numeric, state, 1).is_err());
        assert_eq!(
            state,
            State {
                logical: 1.,
                angle: 0.,
                velocity: 0.
            }
        );
    }

    #[test]
    #[ignore = "requires PCX_WAR3_EXE pointing to the reviewed local client"]
    fn selected_orientation_initializers_and_all_selector_entries_bind() {
        let (image, origin, numeric, budget) = super::super::tests::selected_numeric_image();
        let pe = crate::neutral_rule_input::Pe::parse(&image, budget).unwrap();
        let visual = Rules::read(
            &mut |va, n, code| {
                pe.read_from(va, code)
                    .map_err(|e| e.to_string())?
                    .get(..n)
                    .map(<[u8]>::to_vec)
                    .ok_or_else(|| "orientation source window".into())
            },
            &numeric,
        )
        .unwrap();
        let registry = crate::campaign_client::order_registry(&image, origin.clone()).unwrap();
        let heading = crate::campaign_client::unit_activity(&image, origin, &registry)
            .unwrap()
            .with_heading(&image, &numeric, budget)
            .unwrap();
        let heading = heading.heading_rules().unwrap();
        assert_eq!(
            visual.curves.len(),
            usize::from(pe.read_from(0x8b0920, true).unwrap()[2]) + 1
        );
        for index in 0..visual.curves.len() {
            let state = State {
                logical: heading.half_turn,
                angle: 0.,
                velocity: 0.,
            };
            let next = visual.step(heading, &numeric, state, index).unwrap();
            assert!(next.angle.is_finite() && next.velocity.is_finite());
            assert!(next.angle > 0. && next.angle < heading.full_turn);
        }
    }
}

//! Campaign flee-policy operands decoded from the reviewed client image.
use crate::{
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LifeBand {
    pub minimum: f32,
    pub maximum: f32,
}

#[derive(Clone, Debug)]
pub struct FleePolicy {
    pub image_sha256: Sha256Digest,
    pub units_flee_default: bool,
    pub context_units_flee_mask: u32,
    pub minimum_move_speed: f32,
    pub life_bands: [LifeBand; 6],
    pub ordinary_life_cutoff: f32,
    pub default_distance: f32,
    pub range_margin: f32,
    pub near_zero_distance: f32,
    pub route_move_order: u32,
}

#[derive(Debug, thiserror::Error)]
#[error("campaign flee policy: {0}")]
pub struct Error(&'static str);

const WINDOWS: [(u32, usize, bool); 26] = [
    (0xab7665, 16, true),
    (0xabf9e2, 10, true),
    (0xba9581, 7, true),
    (0xba95b3, 27, true),
    (0xba9693, 39, true),
    (0xba96bf, 39, true),
    (0xba96eb, 39, true),
    (0xba9717, 39, true),
    (0xba9743, 39, true),
    (0xba976f, 36, true),
    (0xba9837, 13, true),
    (0x401350, 19, true),
    (0x4013f0, 19, true),
    (0x401490, 19, true),
    (0x401570, 19, true),
    (0x4015f0, 19, true),
    (0x401620, 19, true),
    (0x401680, 19, true),
    (0x418200, 19, true),
    (0xb67eb8, 5, true),
    (0xb67e06, 5, true),
    (0xb67ede, 9, true),
    (0xb67ebf, 19, true),
    (0xb67dac, 28, true),
    (0xb67f2c, 19, true),
    (0x10db124, 4, false),
];

fn at(bytes: &[u8], offset: usize, expected: &[u8]) -> Result<(), Error> {
    (bytes.get(offset..offset + expected.len()) == Some(expected))
        .then_some(())
        .ok_or(Error("reviewed instruction relationship changed"))
}

fn word(bytes: &[u8], offset: usize) -> Result<u32, Error> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .ok_or(Error("truncated instruction operand"))?
            .try_into()
            .map_err(|_| Error("truncated instruction operand"))?,
    ))
}

fn call(bytes: &[u8], va: u32, offset: usize) -> Result<u32, Error> {
    at(bytes, offset, &[0xe8])?;
    Ok(va
        .wrapping_add(offset as u32 + 5)
        .wrapping_add(word(bytes, offset + 1)?))
}

fn budgeted_windows(budget: RuleInputBudget) -> Result<(), Error> {
    let mut total = 0usize;
    for (_, len, _) in WINDOWS {
        total = total
            .checked_add(len)
            .ok_or(Error("window budget exceeded"))?;
        if total > budget.profile_bytes {
            return Err(Error("window budget exceeded"));
        }
    }
    Ok(())
}

fn initializer(bytes: &[u8], va: u32) -> Result<(u32, f32), Error> {
    let (value, after) = match bytes.first() {
        Some(0x6a) => (
            i32::from(*bytes.get(1).ok_or(Error("truncated initializer operand"))? as i8),
            2,
        ),
        Some(0x68) => (word(bytes, 1)? as i32, 5),
        _ => return Err(Error("initializer push instruction changed")),
    };
    at(bytes, after, &[0x68])?;
    let global = word(bytes, after + 1)?;
    if call(bytes, va, after + 5)? != 0x9a1620 {
        return Err(Error("initializer conversion helper changed"));
    }
    at(bytes, after + 10, &[0x83, 0xc4, 0x08, 0xc3])?;
    if value < 0 {
        return Err(Error("initializer is a negative life or distance"));
    }
    Ok((global, value as f32))
}

fn band(bytes: &[u8], va: u32) -> Result<(u32, f32), Error> {
    at(bytes, 0, &[0x0f, 0x2f, 0x0d])?;
    let global = word(bytes, 3)?;
    at(bytes, 7, &[0x72])?;
    at(bytes, 9, &[0x8d, 0x45, 0xe0])?;
    let (upper, call_at) = match bytes.get(12).copied() {
        Some(0x68) => (word(bytes, 13)? as i32, 18),
        Some(0x6a) => (
            i32::from(*bytes.get(13).ok_or(Error("truncated band upper bound"))? as i8),
            15,
        ),
        _ => return Err(Error("band upper-bound push instruction changed")),
    };
    at(bytes, call_at - 1, &[0x50])?;
    if call(bytes, va, call_at)? != 0x9a1620 {
        return Err(Error("band conversion helper changed"));
    }
    at(
        bytes,
        call_at + 5,
        &[
            0xf3, 0x0f, 0x10, 0x4d, 0x08, 0x83, 0xc4, 0x08, 0xf3, 0x0f, 0x10, 0x00, 0x0f, 0x2f,
            0xc1,
        ],
    )?;
    if upper < 0 {
        return Err(Error("band upper bound is negative"));
    }
    Ok((global, upper as f32))
}

fn checked_band(minimum: f32, maximum: f32) -> Result<LifeBand, Error> {
    if !minimum.is_finite() || !maximum.is_finite() || maximum <= minimum {
        return Err(Error("life band bounds are invalid"));
    }
    Ok(LifeBand { minimum, maximum })
}

impl FleePolicy {
    pub fn read(image: &[u8], budget: RuleInputBudget) -> Result<Self, Error> {
        if image.len() > budget.image_bytes {
            return Err(Error("image budget exceeded"));
        }
        budgeted_windows(budget)?;
        if sha256(image) != REVIEWED_IMAGE {
            return Err(Error("unsupported client image"));
        }
        let pe = Pe::parse(image, budget).map_err(|_| Error("invalid selected PE"))?;
        let mut windows = Vec::with_capacity(WINDOWS.len());
        for (va, len, code) in WINDOWS {
            windows.push(
                pe.read_from(va, code)
                    .map_err(|_| Error("missing selected instruction window"))?
                    .get(..len)
                    .ok_or(Error("short selected instruction window"))?,
            );
        }
        Self::decode(&windows, REVIEWED_IMAGE)
    }

    fn decode(w: &[&[u8]], image_sha256: Sha256Digest) -> Result<Self, Error> {
        let [
            setter,
            constructor,
            decision_bit,
            decision_minimum,
            band600,
            band500,
            band400,
            band300,
            band200,
            band100,
            cutoff,
            init100,
            init_cutoff,
            init200,
            init300,
            init400,
            init500,
            init600,
            init_distance,
            distance_a,
            distance_b,
            distance_c,
            margin,
            epsilon_reference,
            order,
            epsilon_data,
        ] = w
        else {
            return Err(Error("incomplete flee-policy windows"));
        };

        at(setter, 0, &[0x83, 0x88])?;
        let context_offset = word(setter, 2)?;
        let context_mask = u32::from(*setter.get(6).ok_or(Error("missing flee-context mask"))?);
        at(setter, 7, &[0x5d, 0xc3])?;
        at(setter, 9, &[0x83, 0xa0])?;
        if word(setter, 11)? != context_offset
            || context_mask.count_ones() != 1
            || context_mask > u32::from(u8::MAX)
        {
            return Err(Error("flee-context flag offset or mask is invalid"));
        }
        at(setter, 15, &[!context_mask as u8])?;
        at(constructor, 0, &[0xc7, 0x86])?;
        if word(constructor, 2)? != context_offset {
            return Err(Error("flee-context constructor offset differs"));
        }
        let constructor_default = word(constructor, 6)?;
        at(decision_bit, 0, &[0xf6, 0x86])?;
        if word(decision_bit, 2)? != context_offset {
            return Err(Error("flee-context decision offset differs"));
        }
        at(decision_bit, 6, &[context_mask as u8])?;

        at(
            decision_minimum,
            0,
            &[0x8b, 0x4d, 0xec, 0x8d, 0x45, 0xe0, 0x50],
        )?;
        if call(decision_minimum, WINDOWS[3].0, 7)? != 0xb68160 {
            return Err(Error("minimum-speed scalar helper changed"));
        }
        at(decision_minimum, 12, &[0xf3, 0x0f, 0x10, 0x05])?;
        let minimum_global = word(decision_minimum, 16)?;
        at(decision_minimum, 20, &[0x0f, 0x2f, 0x00, 0x0f, 0x87])?;

        let (band600_global, band600_maximum) = band(band600, WINDOWS[4].0)?;
        let (band500_global, band500_maximum) = band(band500, WINDOWS[5].0)?;
        let (band400_global, band400_maximum) = band(band400, WINDOWS[6].0)?;
        let (band300_global, band300_maximum) = band(band300, WINDOWS[7].0)?;
        let (band200_global, band200_maximum) = band(band200, WINDOWS[8].0)?;
        let (band100_global, band100_maximum) = band(band100, WINDOWS[9].0)?;
        let (init600_global, band600_minimum) = initializer(init600, WINDOWS[17].0)?;
        let (init500_global, band500_minimum) = initializer(init500, WINDOWS[16].0)?;
        let (init400_global, band400_minimum) = initializer(init400, WINDOWS[15].0)?;
        let (init300_global, band300_minimum) = initializer(init300, WINDOWS[14].0)?;
        let (init200_global, band200_minimum) = initializer(init200, WINDOWS[13].0)?;
        let (init100_global, band100_minimum) = initializer(init100, WINDOWS[11].0)?;
        if band600_global != init600_global
            || band500_global != init500_global
            || band400_global != init400_global
            || band300_global != init300_global
            || band200_global != init200_global
            || band100_global != init100_global
            || band100_global != minimum_global
        {
            return Err(Error("life band initializer or minimum global differs"));
        }

        at(cutoff, 0, &[0xf3, 0x0f, 0x10, 0x05])?;
        let cutoff_global = word(cutoff, 4)?;
        at(cutoff, 8, &[0x0f, 0x2f, 0xc1, 0x76])?;
        let (init_cutoff_global, ordinary_life_cutoff) = initializer(init_cutoff, WINDOWS[12].0)?;
        if init_cutoff_global != cutoff_global || ordinary_life_cutoff <= 0.0 {
            return Err(Error("ordinary life cutoff initializer differs"));
        }

        at(distance_a, 0, &[0x68])?;
        at(distance_b, 0, &[0x68])?;
        at(distance_c, 0, &[0x8d, 0x45, 0xfc, 0x50, 0x68])?;
        let distance_global = word(distance_a, 1)?;
        if word(distance_b, 1)? != distance_global || word(distance_c, 5)? != distance_global {
            return Err(Error("route default-distance references differ"));
        }
        let (init_distance_global, default_distance) = initializer(init_distance, WINDOWS[18].0)?;
        if init_distance_global != distance_global || default_distance <= 0.0 {
            return Err(Error("route default-distance initializer differs"));
        }

        at(margin, 0, &[0x68])?;
        let margin_global = word(margin, 1)?;
        if margin_global != minimum_global {
            return Err(Error("route margin global differs from minimum speed"));
        }
        at(margin, 5, &[0x53, 0x50])?;
        if call(margin, WINDOWS[22].0, 7)? != 0xb20440 {
            return Err(Error("route margin score helper changed"));
        }
        at(margin, 12, &[0x50, 0x8d, 0x45, 0xdc, 0x50])?;

        at(epsilon_reference, 0, &[0xf3, 0x0f, 0x10, 0x05])?;
        if word(epsilon_reference, 4)? != WINDOWS[25].0 {
            return Err(Error("route epsilon reference changed"));
        }
        at(
            epsilon_reference,
            8,
            &[
                0x83, 0xc4, 0x0c, 0x8b, 0x00, 0x25, 0xff, 0xff, 0xff, 0x7f, 0x89, 0x45, 0xf4, 0x0f,
                0x2f, 0x45, 0xf4, 0x5f, 0x76, 0x7b,
            ],
        )?;
        let near_zero_distance = f32::from_bits(word(epsilon_data, 0)?);
        if !near_zero_distance.is_finite() || near_zero_distance <= 0.0 {
            return Err(Error("route epsilon is invalid"));
        }

        at(order, 0, &[0xf6, 0x83, 0x54, 0x04, 0x00, 0x00, 0x10])?;
        at(order, 7, &[0xbe])?;
        at(order, 12, &[0x74, 0x05, 0xbe])?;
        let route_move_order = word(order, 8)?;
        let alternate_order = word(order, 15)?;
        if route_move_order == 0 || alternate_order == 0 || route_move_order == alternate_order {
            return Err(Error("route order operands are invalid"));
        }

        let mut life_bands = [
            checked_band(band600_minimum, band600_maximum)?,
            checked_band(band500_minimum, band500_maximum)?,
            checked_band(band400_minimum, band400_maximum)?,
            checked_band(band300_minimum, band300_maximum)?,
            checked_band(band200_minimum, band200_maximum)?,
            checked_band(band100_minimum, band100_maximum)?,
        ];
        life_bands.sort_by(|a, b| a.minimum.total_cmp(&b.minimum));
        if life_bands
            .windows(2)
            .any(|pair| pair[0].minimum >= pair[1].minimum)
        {
            return Err(Error("life band lower bounds overlap"));
        }
        let minimum_move_speed = band100_minimum;
        if minimum_move_speed <= 0.0 || life_bands[0].minimum != minimum_move_speed {
            return Err(Error("minimum speed is not the lowest life band"));
        }

        Ok(Self {
            image_sha256,
            units_flee_default: constructor_default & context_mask != 0,
            context_units_flee_mask: context_mask,
            minimum_move_speed,
            life_bands,
            ordinary_life_cutoff,
            default_distance,
            range_margin: minimum_move_speed,
            near_zero_distance,
            route_move_order,
        })
    }

    #[cfg(any(test, feature = "test-fixtures"))]
    pub fn fixture() -> Self {
        let source = synthetic_windows();
        Self::decode(
            &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            [0x42; 32],
        )
        .expect("synthetic flee-policy instructions")
    }
}

#[cfg(any(test, feature = "test-fixtures"))]
fn synthetic_windows() -> Vec<Vec<u8>> {
    fn put(w: &mut [Vec<u8>], i: usize, at: usize, data: &[u8]) {
        w[i][at..at + data.len()].copy_from_slice(data);
    }
    fn dword(w: &mut [Vec<u8>], i: usize, at: usize, data: u32) {
        put(w, i, at, &data.to_le_bytes());
    }
    fn call(w: &mut [Vec<u8>], i: usize, at: usize, target: u32) {
        put(w, i, at, &[0xe8]);
        dword(
            w,
            i,
            at + 1,
            target.wrapping_sub(WINDOWS[i].0 + at as u32 + 5),
        );
    }
    fn initializer(w: &mut [Vec<u8>], i: usize, value: u32, global: u32, imm8: bool) {
        if imm8 {
            put(w, i, 0, &[0x6a, value as u8]);
            put(w, i, 2, &[0x68]);
            dword(w, i, 3, global);
            call(w, i, 7, 0x9a1620);
            put(w, i, 12, &[0x83, 0xc4, 0x08, 0xc3]);
        } else {
            put(w, i, 0, &[0x68]);
            dword(w, i, 1, value);
            put(w, i, 5, &[0x68]);
            dword(w, i, 6, global);
            call(w, i, 10, 0x9a1620);
            put(w, i, 15, &[0x83, 0xc4, 0x08, 0xc3]);
        }
    }
    fn band(w: &mut [Vec<u8>], i: usize, global: u32, maximum: u32, short: bool) {
        put(w, i, 0, &[0x0f, 0x2f, 0x0d]);
        dword(w, i, 3, global);
        if short {
            put(
                w,
                i,
                7,
                &[0x72, 0x20, 0x8d, 0x45, 0xe0, 0x6a, maximum as u8],
            );
            put(w, i, 14, &[0x50]);
            call(w, i, 15, 0x9a1620);
            put(
                w,
                i,
                20,
                &[
                    0xf3, 0x0f, 0x10, 0x4d, 0x08, 0x83, 0xc4, 0x08, 0xf3, 0x0f, 0x10, 0x00, 0x0f,
                    0x2f, 0xc1,
                ],
            );
        } else {
            put(w, i, 7, &[0x72, 0x23, 0x8d, 0x45, 0xe0, 0x68]);
            dword(w, i, 13, maximum);
            put(w, i, 17, &[0x50]);
            call(w, i, 18, 0x9a1620);
            put(
                w,
                i,
                23,
                &[
                    0xf3, 0x0f, 0x10, 0x4d, 0x08, 0x83, 0xc4, 0x08, 0xf3, 0x0f, 0x10, 0x00, 0x0f,
                    0x2f, 0xc1,
                ],
            );
        }
    }

    let mut w = WINDOWS
        .iter()
        .map(|(_, len, _)| vec![0; *len])
        .collect::<Vec<_>>();
    let context_offset = 0x2d0;
    let context_mask = 0x40;
    let minimum = 117;
    let minimum_global = 0x112d000;
    let cutoff_global = 0x112d004;
    let distance_global = 0x11b5000;

    put(&mut w, 0, 0, &[0x83, 0x88]);
    dword(&mut w, 0, 2, context_offset);
    put(&mut w, 0, 6, &[context_mask as u8]);
    put(&mut w, 0, 7, &[0x5d, 0xc3]);
    put(&mut w, 0, 9, &[0x83, 0xa0]);
    dword(&mut w, 0, 11, context_offset);
    put(&mut w, 0, 15, &[!context_mask as u8]);
    put(&mut w, 1, 0, &[0xc7, 0x86]);
    dword(&mut w, 1, 2, context_offset);
    dword(&mut w, 1, 6, context_mask);
    put(&mut w, 2, 0, &[0xf6, 0x86]);
    dword(&mut w, 2, 2, context_offset);
    put(&mut w, 2, 6, &[context_mask as u8]);
    put(&mut w, 3, 0, &[0x8b, 0x4d, 0xec, 0x8d, 0x45, 0xe0, 0x50]);
    call(&mut w, 3, 7, 0xb68160);
    put(&mut w, 3, 12, &[0xf3, 0x0f, 0x10, 0x05]);
    dword(&mut w, 3, 16, minimum_global);
    put(&mut w, 3, 20, &[0x0f, 0x2f, 0x00, 0x0f, 0x87, 0x1b, 0xff]);

    band(&mut w, 4, 0x112d010, 693, false);
    band(&mut w, 5, 0x112d014, 593, false);
    band(&mut w, 6, 0x112d018, 493, false);
    band(&mut w, 7, 0x112d01c, 393, false);
    band(&mut w, 8, 0x112d020, 293, false);
    band(&mut w, 9, minimum_global, 123, true);

    put(&mut w, 10, 0, &[0xf3, 0x0f, 0x10, 0x05]);
    dword(&mut w, 10, 4, cutoff_global);
    put(&mut w, 10, 8, &[0x0f, 0x2f, 0xc1, 0x76, 0x23]);
    initializer(&mut w, 11, minimum, minimum_global, true);
    initializer(&mut w, 12, 173, cutoff_global, false);
    initializer(&mut w, 13, 283, 0x112d020, false);
    initializer(&mut w, 14, 383, 0x112d01c, false);
    initializer(&mut w, 15, 483, 0x112d018, false);
    initializer(&mut w, 16, 583, 0x112d014, false);
    initializer(&mut w, 17, 683, 0x112d010, false);
    initializer(&mut w, 18, 347, distance_global, false);

    put(&mut w, 19, 0, &[0x68]);
    dword(&mut w, 19, 1, distance_global);
    put(&mut w, 20, 0, &[0x68]);
    dword(&mut w, 20, 1, distance_global);
    put(&mut w, 21, 0, &[0x8d, 0x45, 0xfc, 0x50, 0x68]);
    dword(&mut w, 21, 5, distance_global);

    put(&mut w, 22, 0, &[0x68]);
    dword(&mut w, 22, 1, minimum_global);
    put(&mut w, 22, 5, &[0x53, 0x50]);
    call(&mut w, 22, 7, 0xb20440);
    put(&mut w, 22, 12, &[0x50, 0x8d, 0x45, 0xdc, 0x50]);

    put(&mut w, 23, 0, &[0xf3, 0x0f, 0x10, 0x05]);
    dword(&mut w, 23, 4, WINDOWS[25].0);
    put(
        &mut w,
        23,
        8,
        &[
            0x83, 0xc4, 0x0c, 0x8b, 0x00, 0x25, 0xff, 0xff, 0xff, 0x7f, 0x89, 0x45, 0xf4, 0x0f,
            0x2f, 0x45, 0xf4, 0x5f, 0x76, 0x7b,
        ],
    );

    put(&mut w, 24, 0, &[0xf6, 0x83, 0x54, 0x04, 0x00, 0x00, 0x10]);
    put(&mut w, 24, 7, &[0xbe]);
    dword(&mut w, 24, 8, 0x000d0042);
    put(&mut w, 24, 12, &[0x74, 0x05, 0xbe]);
    dword(&mut w, 24, 15, 0x000d0043);
    dword(&mut w, 25, 0, 0.00000025_f32.to_bits());

    w
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_budget_is_exact() {
        let total = WINDOWS.iter().map(|(_, len, _)| len).sum();
        let budget = RuleInputBudget {
            image_bytes: 1,
            profile_bytes: total,
            sections: 1,
        };
        assert!(budgeted_windows(budget).is_ok());
        assert!(
            budgeted_windows(RuleInputBudget {
                profile_bytes: total - 1,
                ..budget
            })
            .is_err()
        );
    }

    #[test]
    fn changed_source_image_refuses_before_parsing() {
        let budget = RuleInputBudget {
            image_bytes: 1024,
            profile_bytes: 1024,
            sections: 1,
        };
        assert!(FleePolicy::read(b"MZunsupported", budget).is_err());
    }

    #[test]
    fn synthetic_instruction_operands_drive_the_policy() {
        let policy = FleePolicy::fixture();
        assert_eq!(policy.image_sha256, [0x42; 32]);
        assert!(policy.units_flee_default);
        assert_eq!(policy.context_units_flee_mask, 0x40);
        assert_eq!(policy.minimum_move_speed, 117.0);
        assert_eq!(policy.range_margin, policy.minimum_move_speed);
        assert_eq!(policy.ordinary_life_cutoff, 173.0);
        assert_eq!(policy.default_distance, 347.0);
        assert_eq!(policy.near_zero_distance, 0.00000025);
        assert_eq!(policy.route_move_order, 0x000d0042);
        assert_eq!(
            policy.life_bands,
            [
                LifeBand {
                    minimum: 117.0,
                    maximum: 123.0
                },
                LifeBand {
                    minimum: 283.0,
                    maximum: 293.0
                },
                LifeBand {
                    minimum: 383.0,
                    maximum: 393.0
                },
                LifeBand {
                    minimum: 483.0,
                    maximum: 493.0
                },
                LifeBand {
                    minimum: 583.0,
                    maximum: 593.0
                },
                LifeBand {
                    minimum: 683.0,
                    maximum: 693.0
                },
            ]
        );

        let mut source = synthetic_windows();
        source[4][13..17].copy_from_slice(&699_u32.to_le_bytes());
        assert_eq!(
            FleePolicy::decode(
                &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                [0; 32]
            )
            .expect("changed band upper bound")
            .life_bands[5]
                .maximum,
            699.0
        );

        source = synthetic_windows();
        source[1][6..10].copy_from_slice(&0_u32.to_le_bytes());
        assert!(
            !FleePolicy::decode(
                &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                [0; 32]
            )
            .expect("changed constructor default")
            .units_flee_default
        );
    }

    #[test]
    fn malformed_or_short_relationships_are_refused() {
        let refuses = |source: Vec<Vec<u8>>| {
            assert!(
                FleePolicy::decode(
                    &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                    [0; 32]
                )
                .is_err()
            );
        };

        let mut source = synthetic_windows();
        source[0].pop();
        refuses(source);

        let mut source = synthetic_windows();
        source[1][2] ^= 1;
        refuses(source);

        let mut source = synthetic_windows();
        source[2][6] ^= 1;
        refuses(source);

        let mut source = synthetic_windows();
        source[0][15] ^= 1;
        refuses(source);

        let mut source = synthetic_windows();
        source[0][6] = 0x50;
        source[2][6] = 0x50;
        source[0][15] = !0x50_u8;
        refuses(source);

        let mut source = synthetic_windows();
        source[22][1..5].copy_from_slice(&0x112d004_u32.to_le_bytes());
        refuses(source);

        let mut source = synthetic_windows();
        source[17][6..10].copy_from_slice(&0x112d024_u32.to_le_bytes());
        refuses(source);

        let mut source = synthetic_windows();
        source[9][13] = 117;
        refuses(source);

        let mut source = synthetic_windows();
        source[12][6..10].copy_from_slice(&0x112d008_u32.to_le_bytes());
        refuses(source);

        let mut source = synthetic_windows();
        source[19][1..5].copy_from_slice(&0x11b5004_u32.to_le_bytes());
        refuses(source);

        let mut source = synthetic_windows();
        source[25].copy_from_slice(&f32::NAN.to_le_bytes());
        refuses(source);

        let mut source = synthetic_windows();
        source[24][15..19].copy_from_slice(&0x000d0042_u32.to_le_bytes());
        refuses(source);

        let mut source = synthetic_windows();
        source[24][8..12].copy_from_slice(&0_u32.to_le_bytes());
        refuses(source);
    }

    #[test]
    #[ignore = "requires PCX_WAR3_EXE pointing to the reviewed local client"]
    fn reviewed_original_image_decodes() {
        let image = std::fs::read(std::env::var_os("PCX_WAR3_EXE").expect("PCX_WAR3_EXE required"))
            .expect("read reviewed client");
        let policy = FleePolicy::read(
            &image,
            RuleInputBudget {
                image_bytes: 32 << 20,
                profile_bytes: WINDOWS.iter().map(|(_, len, _)| len).sum(),
                sections: 96,
            },
        )
        .expect("selected flee-policy operands");
        assert_eq!(policy.image_sha256, REVIEWED_IMAGE);
        assert!(!policy.units_flee_default);
        assert_eq!(policy.context_units_flee_mask, 0x10);
        assert_eq!(policy.minimum_move_speed, 100.0);
        assert_eq!(policy.range_margin, policy.minimum_move_speed);
        assert_eq!(policy.ordinary_life_cutoff, 150.0);
        assert_eq!(policy.default_distance, 300.0);
        assert_eq!(policy.near_zero_distance, f32::from_bits(0x3456_bf95));
        assert_eq!(policy.route_move_order, 0x000d000f);
        assert_eq!(
            policy.life_bands,
            [
                LifeBand {
                    minimum: 100.0,
                    maximum: 110.0
                },
                LifeBand {
                    minimum: 200.0,
                    maximum: 210.0
                },
                LifeBand {
                    minimum: 300.0,
                    maximum: 310.0
                },
                LifeBand {
                    minimum: 400.0,
                    maximum: 410.0
                },
                LifeBand {
                    minimum: 500.0,
                    maximum: 510.0
                },
                LifeBand {
                    minimum: 600.0,
                    maximum: 610.0
                },
            ]
        );
    }
}

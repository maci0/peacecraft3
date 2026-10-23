//! Campaign flee-policy operands decoded from the reviewed client image.
use crate::{
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    order_registry::Registry,
    script_numeric::NumericPolicy,
    script_random::real,
    unit_activity::Rules,
};

/// A life band `[minimum, maximum]` with an **inclusive** upper bound. The
/// source `comiss` + `jae` to the shared true label selects a band when the
/// value is `>= minimum` and `>= maximum` is treated as still in-bounds at the
/// exact maximum (`jae` = jump if above-or-equal). Consumers must therefore
/// classify `value <= maximum` as belonging to the band; an exclusive upper
/// comparison would misclassify the boundary value.
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

#[derive(Debug, thiserror::Error)]
pub enum RouteError {
    #[error("campaign flee route: numeric source image differs from flee policy")]
    Identity,
    #[error("campaign flee route: selected numeric math is absent")]
    MissingMath,
    #[error("campaign flee route: {0}")]
    Input(&'static str),
    #[error("campaign flee route: zero-direction orientation is not qualified")]
    ZeroDirection,
    #[error("campaign flee route: selected real arithmetic: {0}")]
    Math(#[from] real::MathError),
}

const WINDOWS: [(u32, usize, bool); 26] = [
    (0xab7665, 16, true),
    (0xabf9e2, 10, true),
    (0xba9581, 7, true),
    (0xba95b3, 27, true),
    (0xba9693, 45, true),
    (0xba96bf, 45, true),
    (0xba96eb, 45, true),
    (0xba9717, 45, true),
    (0xba9743, 45, true),
    (0xba976f, 42, true),
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
    // The upper comparison is `comiss xmm1, xmm0` followed by an inclusive
    // `jae` (`0f 83 rel32`) to the shared true label. Validate the encoding and
    // that every band's true branch joins the same label.
    at(bytes, call_at + 20, &[0x0f, 0x83])?;
    let upper_target = va
        .wrapping_add(call_at as u32 + 26)
        .wrapping_add(word(bytes, call_at + 22)?);
    if upper_target != 0xba9844 {
        return Err(Error(
            "band upper true branch does not join the shared label",
        ));
    }
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
    /// Compute only the ordinary nonzero flee destination; no order is issued.
    pub fn ordinary_destination(
        &self,
        numeric: &NumericPolicy,
        actor: [f32; 2],
        threat: [f32; 2],
        target_range: f32,
    ) -> Result<[f32; 2], RouteError> {
        if self.image_sha256 != *numeric.image_sha256() {
            return Err(RouteError::Identity);
        }
        let math = numeric.math().ok_or(RouteError::MissingMath)?;
        if !self.range_margin.is_finite() || self.range_margin < 0.0 {
            return Err(RouteError::Input("invalid source route margin"));
        }
        if !self.near_zero_distance.is_finite() || self.near_zero_distance <= 0.0 {
            return Err(RouteError::Input("invalid source route epsilon"));
        }
        if !target_range.is_finite() || target_range < 0.0 {
            return Err(RouteError::Input("invalid initial target range"));
        }
        let actor = [real::from_f32(actor[0])?, real::from_f32(actor[1])?];
        let threat = [real::from_f32(threat[0])?, real::from_f32(threat[1])?];
        let range = real::from_f32(target_range)?;
        let margin = real::from_f32(self.range_margin)?;
        let dz = real::sub(0, 0)?;
        let dy = real::sub(actor[1], threat[1])?;
        let dx = real::sub(actor[0], threat[0])?;
        let square_z = real::mul(dz, dz)?;
        let square_y = real::mul(dy, dy)?;
        let square_x = real::mul(dx, dx)?;
        let sum = real::add(square_x, square_y)?;
        let sum = real::add(sum, square_z)?;
        let length = math.square_root(sum)?;
        if f32::from_bits(real::abs(length)) < self.near_zero_distance {
            return Err(RouteError::ZeroDirection);
        }
        let distance = real::add(range, margin)?;
        let factor = numeric.division(distance, length)?;
        Ok([
            f32::from_bits(real::add(actor[0], real::mul(dx, factor)?)?),
            f32::from_bits(real::add(actor[1], real::mul(dy, factor)?)?),
        ])
    }

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
        let context_mask_byte = *setter.get(6).ok_or(Error("missing flee-context mask"))?;
        if context_mask_byte & 0x80 != 0 {
            return Err(Error("flee-context flag mask is a negative imm8"));
        }
        let context_mask = u32::from(context_mask_byte);
        at(setter, 7, &[0x5d, 0xc3])?;
        at(setter, 9, &[0x83, 0xa0])?;
        if word(setter, 11)? != context_offset || context_mask.count_ones() != 1 {
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
        let route_move_order = word(order, 15)?;
        let alternate_order = word(order, 8)?;
        if route_move_order == 0 || alternate_order == 0 || route_move_order == alternate_order {
            return Err(Error("route order operands are invalid"));
        }

        let life_bands = [
            checked_band(band600_minimum, band600_maximum)?,
            checked_band(band500_minimum, band500_maximum)?,
            checked_band(band400_minimum, band400_maximum)?,
            checked_band(band300_minimum, band300_maximum)?,
            checked_band(band200_minimum, band200_maximum)?,
            checked_band(band100_minimum, band100_maximum)?,
        ];
        if life_bands
            .windows(2)
            .any(|pair| pair[0].minimum <= pair[1].minimum || pair[0].maximum <= pair[1].maximum)
        {
            return Err(Error("life band order differs from the native decision"));
        }
        let minimum_move_speed = band100_minimum;
        if minimum_move_speed <= 0.0 || life_bands[5].minimum != minimum_move_speed {
            return Err(Error("minimum speed is not the final native life band"));
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

    pub fn conditional_life_bands(&self) -> &[LifeBand] {
        &self.life_bands[..4]
    }

    pub fn unconditional_life_bands(&self) -> &[LifeBand] {
        &self.life_bands[4..]
    }

    pub fn validate_route_move_order(
        &self,
        registry: &Registry,
        activities: &Rules,
    ) -> Result<(), Error> {
        let route = i32::try_from(self.route_move_order)
            .map_err(|_| Error("route movement order is not a signed identifier"))?;
        if activities.movement != route || registry.name(route).is_none() {
            return Err(Error("route order is not the registered movement identity"));
        }
        Ok(())
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
    fn jae(w: &mut [Vec<u8>], i: usize, at: usize, target: u32) {
        put(w, i, at, &[0x0f, 0x83]);
        dword(
            w,
            i,
            at + 2,
            target.wrapping_sub(WINDOWS[i].0 + at as u32 + 6),
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
            jae(w, i, 35, 0xba9844);
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
            jae(w, i, 38, 0xba9844);
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
pub(crate) mod tests {
    use super::*;
    use crate::{camera_margins::SourceWindow, script_math, script_numeric};

    pub(crate) fn route_fixture() -> (FleePolicy, NumericPolicy) {
        let (mut image, mut numeric_binding, budget) = script_numeric::tests::fixture();
        let base = 0x301000u32;
        fn put(image: &mut [u8], base: u32, at: usize, bytes: &[u8]) -> SourceWindow {
            image[0x200 + at..0x200 + at + bytes.len()].copy_from_slice(bytes);
            SourceWindow {
                va: base + at as u32,
                len: bytes.len() as u32,
                digest: sha256(bytes),
            }
        }
        fn call(bytes: &mut [u8], base: u32, at: usize, target: u32) {
            bytes[at] = 0xe8;
            bytes[at + 1..at + 5]
                .copy_from_slice(&target.wrapping_sub(base + at as u32 + 5).to_le_bytes());
        }
        fn initializer(
            image: &mut [u8],
            base: u32,
            at: usize,
            value: u32,
            convert: u32,
        ) -> SourceWindow {
            let mut bytes = vec![0x68];
            bytes.extend(value.to_le_bytes());
            bytes.push(0x68);
            bytes.extend((base + at as u32 + 0x100).to_le_bytes());
            bytes.extend([0; 5]);
            call(&mut bytes, base + at as u32, 10, convert);
            bytes.extend([0x83, 0xc4, 8, 0xc3]);
            put(image, base, at, &bytes)
        }
        let zero = initializer(
            &mut image,
            base,
            0xc60,
            0,
            numeric_binding.math_i32_to_real.va,
        );
        let one = initializer(
            &mut image,
            base,
            0xc80,
            1,
            numeric_binding.math_i32_to_real.va,
        );
        let mut root_native = vec![0x90; 94];
        for at in [12, 54, 85] {
            root_native[at..at + 4].copy_from_slice(&(base + 0xd60).to_le_bytes());
        }
        root_native[27..31].copy_from_slice(&numeric_binding.equality_constant.va.to_le_bytes());
        call(&mut root_native, base + 0x800, 68, base + 0x900);
        let square_root_native = put(&mut image, base, 0x800, &root_native);
        let mut atan2_native = vec![0x90; 126];
        for at in [5, 49, 93] {
            atan2_native[at..at + 4].copy_from_slice(&(base + 0xd60).to_le_bytes());
        }
        for at in [25, 69] {
            atan2_native[at..at + 4]
                .copy_from_slice(&numeric_binding.equality_constant.va.to_le_bytes());
        }
        call(&mut atan2_native, base + 0x880, 111, base + 0xa00);
        let arctangent2_native = put(&mut image, base, 0x880, &atan2_native);
        let mut root = vec![0x90; 164];
        call(&mut root, base + 0x900, 72, base + 0x9b0);
        root[0x4d] = 0x2d;
        root[0x4e..0x52].copy_from_slice(&46340u32.to_le_bytes());
        root[0x54..0x56].copy_from_slice(&[0x69, 0xc0]);
        root[0x56..0x5a].copy_from_slice(&46340u32.to_le_bytes());
        root[0x6f..0x71].copy_from_slice(&[0x81, 0xe1]);
        root[0x71..0x75].copy_from_slice(&0x0035_0000u32.to_le_bytes());
        let square_root = put(&mut image, base, 0x900, &root);
        let mut integer_root = vec![0x90; 61];
        integer_root[0x0d..0x10].copy_from_slice(&[0x8d, 0x7e, 16]);
        let integer_root = put(&mut image, base, 0x9b0, &integer_root);
        let mut atan2 = vec![0x90; 161];
        call(&mut atan2, base + 0xa00, 76, base + 0xab0);
        atan2[89..93].copy_from_slice(&(base + 0xd20).to_le_bytes());
        atan2[116..120].copy_from_slice(&(base + 0xd24).to_le_bytes());
        let arctangent2 = put(&mut image, base, 0xa00, &atan2);
        let mut atan = vec![0x90; 423];
        for at in [32, 115, 233] {
            atan[at..at + 4].copy_from_slice(&(base + 0xd80).to_le_bytes());
        }
        for (at, datum) in [
            (72, 0xd00),
            (100, 0xd04),
            (131, 0xd08),
            (218, 0xd0c),
            (253, 0xd10),
            (268, 0xd14),
            (348, 0xd18),
            (378, 0xd20),
        ] {
            atan[at..at + 4].copy_from_slice(&(base + datum).to_le_bytes());
        }
        let arctangent = put(&mut image, base, 0xab0, &atan);
        for (at, value) in [
            (0xd00, 0.5f32),
            (0xd04, 1.0),
            (0xd08, 0.25),
            (0xd0c, 0.125),
            (0xd10, 0.75),
            (0xd14, 1.5),
            (0xd18, 0.5),
            (0xd20, 1.5),
            (0xd24, 3.0),
        ] {
            image[0x200 + at..0x204 + at].copy_from_slice(&value.to_le_bytes());
        }
        numeric_binding.image_sha256 = sha256(&image);
        let math_binding = script_math::Binding {
            image_sha256: numeric_binding.image_sha256,
            square_root_native,
            arctangent2_native,
            square_root,
            integer_root,
            arctangent2,
            arctangent,
            zero_initializer: zero,
            one_initializer: one,
        };
        let numeric = NumericPolicy::read(&image, numeric_binding, budget, 64)
            .unwrap()
            .with_math(&image, &math_binding, budget)
            .unwrap();
        let mut windows = synthetic_windows();
        windows[11][1] = 2;
        let policy = FleePolicy::decode(
            &windows.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            *numeric.image_sha256(),
        )
        .unwrap();
        (policy, numeric)
    }

    #[test]
    fn ordinary_route_moves_away_and_translates_with_the_actor() {
        let (policy, numeric) = route_fixture();
        assert_eq!(
            policy
                .ordinary_destination(&numeric, [5.0, 9.0], [3.0, 9.0], 0.0)
                .unwrap(),
            [7.0, 9.0]
        );
        assert_eq!(
            policy
                .ordinary_destination(&numeric, [105.0, 29.0], [103.0, 29.0], 0.0)
                .unwrap(),
            [107.0, 29.0]
        );
    }

    #[test]
    fn ordinary_route_uses_both_axes_and_source_margin() {
        let (mut policy, numeric) = route_fixture();
        let base = policy
            .ordinary_destination(&numeric, [8.0, 9.0], [5.0, 5.0], 3.0)
            .unwrap();
        assert!(base[0] > 8.0 && base[1] > 9.0);
        let farther = policy
            .ordinary_destination(&numeric, [8.0, 9.0], [5.0, 5.0], 4.0)
            .unwrap();
        assert!(farther[0] > base[0] && farther[1] > base[1]);
        policy.range_margin = 3.0;
        assert_eq!(
            policy
                .ordinary_destination(&numeric, [8.0, 9.0], [5.0, 5.0], 3.0)
                .unwrap(),
            farther
        );
    }

    #[test]
    fn route_refuses_unqualified_inputs_before_arithmetic() {
        let (mut policy, numeric) = route_fixture();
        assert!(matches!(
            policy.ordinary_destination(&numeric, [0.0; 2], [0.0; 2], 0.0),
            Err(RouteError::ZeroDirection)
        ));
        policy.near_zero_distance = 2.0;
        assert!(matches!(
            policy.ordinary_destination(&numeric, [1.0, 0.0], [0.0; 2], 1.0),
            Err(RouteError::ZeroDirection)
        ));
        policy.near_zero_distance = 0.00000025;
        for value in [f32::NAN, f32::INFINITY, f32::from_bits(1)] {
            assert!(
                policy
                    .ordinary_destination(&numeric, [value, 0.0], [0.0; 2], 1.0)
                    .is_err()
            );
        }
        assert!(
            policy
                .ordinary_destination(&numeric, [1.0, 0.0], [0.0; 2], -1.0)
                .is_err()
        );
        policy.range_margin = f32::NAN;
        assert!(matches!(
            policy.ordinary_destination(&numeric, [1.0, 0.0], [0.0; 2], 1.0),
            Err(RouteError::Input(_))
        ));
        policy.range_margin = 2.0;
        policy.near_zero_distance = 0.0;
        assert!(matches!(
            policy.ordinary_destination(&numeric, [1.0, 0.0], [0.0; 2], 1.0),
            Err(RouteError::Input(_))
        ));
        policy.near_zero_distance = 0.00000025;
        policy.image_sha256[0] ^= 1;
        assert!(matches!(
            policy.ordinary_destination(&numeric, [1.0, 0.0], [0.0; 2], 1.0),
            Err(RouteError::Identity)
        ));
        let (image, binding, budget) = script_numeric::tests::fixture();
        let numeric_without_math = NumericPolicy::read(&image, binding, budget, 64).unwrap();
        policy.image_sha256 = *numeric_without_math.image_sha256();
        assert!(matches!(
            policy.ordinary_destination(&numeric_without_math, [1.0, 0.0], [0.0; 2], 1.0),
            Err(RouteError::MissingMath)
        ));
    }

    #[test]
    fn route_calls_internal_root_not_the_native_near_zero_wrapper() {
        let (policy, numeric) = route_fixture();
        let math = numeric.math().unwrap();
        assert_eq!(
            math.square_root_native(&numeric, 0.125 * 0.125).unwrap(),
            0.0
        );
        assert!(
            policy
                .ordinary_destination(&numeric, [0.125, 0.0], [0.0; 2], 0.0)
                .is_ok()
        );
    }

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
        assert_eq!(policy.route_move_order, 0x000d0043);
        assert_eq!(
            policy.life_bands,
            [
                LifeBand {
                    minimum: 683.0,
                    maximum: 693.0
                },
                LifeBand {
                    minimum: 583.0,
                    maximum: 593.0
                },
                LifeBand {
                    minimum: 483.0,
                    maximum: 493.0
                },
                LifeBand {
                    minimum: 383.0,
                    maximum: 393.0
                },
                LifeBand {
                    minimum: 283.0,
                    maximum: 293.0
                },
                LifeBand {
                    minimum: 117.0,
                    maximum: 123.0
                },
            ]
        );
        assert_eq!(
            policy.conditional_life_bands(),
            [
                LifeBand {
                    minimum: 683.0,
                    maximum: 693.0
                },
                LifeBand {
                    minimum: 583.0,
                    maximum: 593.0
                },
                LifeBand {
                    minimum: 483.0,
                    maximum: 493.0
                },
                LifeBand {
                    minimum: 383.0,
                    maximum: 393.0
                },
            ]
        );
        assert_eq!(
            policy.unconditional_life_bands(),
            [
                LifeBand {
                    minimum: 283.0,
                    maximum: 293.0
                },
                LifeBand {
                    minimum: 117.0,
                    maximum: 123.0
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
            .life_bands[0]
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
    fn band_lower_branch_skips_to_next_boundary_and_true_joins_shared_label() {
        let source = synthetic_windows();
        let fall_through = 0xba9798;
        for i in 4..=9 {
            let w = &source[i];
            assert_eq!(w[7], 0x72, "band {i}: expected lower jb opcode");
            let next = if i < 9 {
                WINDOWS[i + 1].0
            } else {
                fall_through
            };
            let expected_rel = (i64::from(next) - i64::from(WINDOWS[i].0) - 9) as i8;
            assert_eq!(
                w[8] as i8, expected_rel,
                "band {i}: lower branch must skip to the next boundary"
            );
            let jae_at = if i == 9 { 35 } else { 38 };
            assert_eq!(&w[jae_at..jae_at + 2], &[0x0f, 0x83]);
            let disp = u32::from_le_bytes(
                w[jae_at + 2..jae_at + 6]
                    .try_into()
                    .expect("jae displacement"),
            );
            let target = WINDOWS[i]
                .0
                .wrapping_add(jae_at as u32 + 6)
                .wrapping_add(disp);
            assert_eq!(
                target, 0xba9844,
                "band {i}: true branch must join the shared label"
            );
        }
    }

    #[test]
    fn band_upper_branch_mutants_refuse() {
        for (index, mutate) in [
            (4usize, 38usize),
            (5, 38),
            (6, 38),
            (7, 38),
            (8, 38),
            (9, 35),
        ] {
            // JAE opcode flip
            let mut source = synthetic_windows();
            source[index][mutate] ^= 1;
            assert!(
                FleePolicy::decode(
                    &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                    [0; 32],
                )
                .is_err(),
                "band {index}: JAE opcode flip should refuse"
            );
            // JAE displacement break (target no longer shared)
            let mut source = synthetic_windows();
            source[index][mutate + 2] ^= 0x01;
            assert!(
                FleePolicy::decode(
                    &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                    [0; 32],
                )
                .is_err(),
                "band {index}: JAE displacement break should refuse"
            );
        }
    }

    #[test]
    fn route_order_must_join_the_registered_movement_identity() {
        use crate::{camera_margins::SourceWindow, scripts::AssetIdentity};

        let policy = FleePolicy::fixture();
        let movement = i32::try_from(policy.route_move_order).expect("synthetic route identifier");
        let (image, binding, budget) =
            crate::order_registry::tests::fixture([movement, movement + 1]);
        let registry = Registry::read(&image, binding, budget, 4, 32).expect("synthetic registry");
        let window = SourceWindow {
            va: 0,
            len: 0,
            digest: [0; 32],
        };
        let mut activities = Rules {
            binding: crate::unit_activity::Binding {
                origin: AssetIdentity {
                    edition: "synthetic".into(),
                    version: "test".into(),
                    source: "synthetic:flee".into(),
                },
                image_sha256: [0; 32],
                hold: window,
                stop: window,
                movement: window,
                patrol: window,
            },
            hold: movement - 3,
            stop: movement - 2,
            movement,
            patrol: movement - 1,
            follow: None,
            move_speed: None,
        };

        assert!(
            policy
                .validate_route_move_order(&registry, &activities)
                .is_ok()
        );
        activities.movement = movement + 2;
        assert!(
            policy
                .validate_route_move_order(&registry, &activities)
                .is_err()
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
        source[0][6] = 0x80;
        source[0][15] = !0x80_u8;
        source[2][6] = 0x80;
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
        assert_eq!(
            policy.life_bands,
            [
                LifeBand {
                    minimum: 600.0,
                    maximum: 610.0
                },
                LifeBand {
                    minimum: 500.0,
                    maximum: 510.0
                },
                LifeBand {
                    minimum: 400.0,
                    maximum: 410.0
                },
                LifeBand {
                    minimum: 300.0,
                    maximum: 310.0
                },
                LifeBand {
                    minimum: 200.0,
                    maximum: 210.0
                },
                LifeBand {
                    minimum: 100.0,
                    maximum: 110.0
                },
            ]
        );
        assert_eq!(policy.conditional_life_bands().len(), 4);
        assert_eq!(policy.unconditional_life_bands().len(), 2);
        let origin = crate::scripts::AssetIdentity {
            edition: "Legacy".into(),
            version: "1.29.2.9231".into(),
            source: "reviewed client".into(),
        };
        let registry =
            crate::campaign_client::order_registry(&image, origin.clone()).expect("order registry");
        let activities = crate::campaign_client::unit_activity(&image, origin, &registry)
            .expect("activity rules");
        policy
            .validate_route_move_order(&registry, &activities)
            .expect("flee route order joins the movement identity");
    }

    #[test]
    #[ignore = "requires PCX_WAR3_EXE pointing to the reviewed local client"]
    fn reviewed_route_joins_selected_numeric_and_math() {
        let image = std::fs::read(std::env::var_os("PCX_WAR3_EXE").expect("PCX_WAR3_EXE required"))
            .expect("read reviewed client");
        let budget = RuleInputBudget {
            image_bytes: 32 << 20,
            profile_bytes: 64 << 10,
            sections: 96,
        };
        let policy = FleePolicy::read(&image, budget).expect("selected flee policy");
        let pe = Pe::parse(&image, budget).expect("selected image sections");
        let window = |va, len, code| {
            let bytes = &pe.read_from(va, code).unwrap()[..len as usize];
            SourceWindow {
                va,
                len,
                digest: sha256(bytes),
            }
        };
        let numeric = NumericPolicy::read(
            &image,
            script_numeric::Binding {
                origin: crate::scripts::AssetIdentity {
                    edition: "reviewed".into(),
                    version: "selected".into(),
                    source: "PCX_WAR3_EXE".into(),
                },
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
                math_mul: window(0x99ffe0, 195, true),
                math_lzcnt: window(0x9a0330, 29, true),
                math_i32_to_real: window(0x9a1620, 92, true),
            },
            budget,
            4096,
        )
        .expect("selected numeric policy")
        .with_math(
            &image,
            &script_math::Binding {
                image_sha256: sha256(&image),
                square_root_native: window(0x4ac1a0, 94, true),
                arctangent2_native: window(0x489fd0, 126, true),
                square_root: window(0x9a20a0, 164, true),
                integer_root: window(0x9a2150, 61, true),
                arctangent2: window(0x9a0cf0, 161, true),
                arctangent: window(0x9a0da0, 423, true),
                zero_initializer: window(0x401300, 16, true),
                one_initializer: window(0x401460, 16, true),
            },
            budget,
        )
        .expect("selected math policy");
        let destination = policy
            .ordinary_destination(&numeric, [8.0, 9.0], [5.0, 5.0], 30.0)
            .expect("selected nonzero flee route");
        assert!(destination[0] > 8.0 && destination[1] > 9.0);
    }
}

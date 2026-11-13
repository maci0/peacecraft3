//! Static facing relations submitted by SetUnitFacingTimed.
use super::{Rules as HeadingRules, call, decimal, shape, word};
use crate::{script_numeric::NumericPolicy, script_random::real};

#[derive(Clone, Debug)]
pub struct Rules {
    pub period: f32,
    pub tolerance: f32,
    pub offset: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Target {
    pub point: [f32; 2],
    pub speed: f32,
}

impl Rules {
    pub(crate) fn read(
        read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
        numeric: &NumericPolicy,
    ) -> Result<Self, String> {
        let code = read(0x4ab4b2, 0xdd, true)?;
        shape(&code, 0, &[0x68])?;
        if word(&code, 1)? != word(&read(0x4ab40e, 5, true)?, 1)? {
            return Err("timed facing degree operand differs".into());
        }
        call(&code, 0x4ab4b2, 12, numeric.multiplication_va())?;
        shape(&code, 17, &[0x8b, 0x87, 0x6c, 2, 0, 0])?;
        shape(&code, 30, &[0x8d, 0x8f, 0x6c, 2, 0, 0, 0xff, 0x50, 0x1c])?;
        let wrapper = read(0x6759e6, 10, true)?;
        shape(&wrapper, 0, &[0xc7, 0x86, 0x6c, 2, 0, 0])?;
        if word(
            &read(
                word(&wrapper, 6)?
                    .checked_add(0x1c)
                    .ok_or("timed getter overflow")?,
                4,
                false,
            )?,
            0,
        )? != 0x7fe970
        {
            return Err("timed facing visual getter differs".into());
        }
        shape(&read(0x7fe99e, 6, true)?, 0, &[0x8b, 0x88, 0xc8, 0, 0, 0])?;
        call(&code, 0x4ab4b2, 0x33, 0x8a19b0)?;
        shape(&code, 0x3b, &[0x68])?;
        call(&code, 0x4ab4b2, 0x41, 0x9a1760)?;
        call(&code, 0x4ab4b2, 0x4c, numeric.division_va())?;
        call(&code, 0x4ab4b2, 0x8a, numeric.division_va())?;
        shape(&code, 0x51, &[0x68])?;
        shape(&code, 0x8f, &[0x68])?;
        let zero = word(&read(0x8aeb10, 8, true)?, 4)?;
        if word(&code, 0x52)? != zero || word(&code, 0x90)? != zero {
            return Err("timed facing numerical zero differs".into());
        }
        for at in [0x63, 0xab] {
            shape(&code, at, &[0xf3, 0x0f, 0x10, 0x05])?;
        }
        if word(&code, 0x67)? != word(&code, 0xaf)? {
            return Err("timed facing tolerance operands differ".into());
        }
        shape(&code, 0x7c, &[0x77, 0x5f])?;
        shape(&code, 0xc4, &[0x77, 0x17, 0x6a, 0, 0x6a, 0])?;
        call(&code, 0x4ab4b2, 0xd8, 0x800140)?;
        let text = read(word(&code, 0x3c)?, 17, false)?;
        let end = text
            .iter()
            .position(|b| *b == 0)
            .ok_or("timed facing period unterminated")?;
        let period = decimal(&text[..end], numeric)?;
        let point = read(0x8001b0, 0xa2, true)?;
        for (at, target) in [
            (0, 0x9a1da0),
            (10, 0x9a0fe0),
            (28, numeric.multiplication_va()),
            (48, numeric.multiplication_va()),
            (75, numeric.addition_va()),
            (97, numeric.addition_va()),
            (121, 0x8adbf0),
            (131, 0x8ad880),
            (141, 0x8a9490),
            (150, 0x8add40),
            (157, 0x8abd40),
        ] {
            call(&point, 0x8001b0, at, target)?;
        }
        shape(&point, 22, &[0x68])?;
        shape(&point, 42, &[0x68])?;
        if word(&point, 23)? != word(&point, 43)? {
            return Err("timed facing point scale operands differ".into());
        }
        shape(
            &read(0x8abed7, 9, true)?,
            0,
            &[0x8b, 0x87, 0xfc, 0, 0, 0, 0x89, 0x46, 0x74],
        )?;
        shape(
            &read(0x8abee0, 14, true)?,
            0,
            &[
                0x8b, 0x87, 0, 1, 0, 0, 0x53, 0x56, 0x89, 0x86, 0x80, 0, 0, 0,
            ],
        )?;
        let tolerance = f32::from_bits(word(&read(word(&code, 0x67)?, 4, false)?, 0)?);
        let offset = f32::from_bits(word(&read(word(&point, 23)?, 4, false)?, 0)?);
        if [period, tolerance, offset]
            .iter()
            .any(|x| !x.is_finite() || *x <= 0.)
        {
            return Err("timed facing source scalar invalid".into());
        }
        Ok(Self {
            period,
            tolerance,
            offset,
        })
    }

    pub(crate) fn prepare(
        &self,
        heading: &HeadingRules,
        numeric: &NumericPolicy,
        position: [f32; 2],
        visual: f32,
        angle: f32,
        duration: f32,
    ) -> Result<Option<Target>, String> {
        let calculate = || {
            let frames = numeric.division(real::from_f32(duration)?, self.period.to_bits())?;
            if f32::from_bits(frames).abs() < self.tolerance {
                return Ok(None);
            }
            let delta = heading.delta(visual, angle)?;
            let speed = real::abs(numeric.division(delta.to_bits(), frames)?);
            if f32::from_bits(speed).abs() < self.tolerance {
                return Ok(None);
            }
            let math = numeric.math().ok_or(real::MathError::Unsupported)?;
            let components = [math.cosine_native(angle)?, math.sine_native(angle)?];
            let mut point = position;
            for (p, component) in point.iter_mut().zip(components) {
                *p = f32::from_bits(real::add(
                    real::from_f32(*p)?,
                    real::mul(self.offset.to_bits(), component.to_bits())?,
                )?);
            }
            Ok(Some(Target {
                point,
                speed: f32::from_bits(speed),
            }))
        };
        calculate().map_err(|e: real::MathError| format!("timed facing: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    type Memory = Vec<(u32, Vec<u8>)>;
    fn fixture() -> (Memory, NumericPolicy) {
        let (image, binding, budget) = crate::script_numeric::tests::fixture();
        let numeric = NumericPolicy::read(&image, binding, budget, 64).unwrap();
        let mut code = vec![0; 0xdd];
        let mut put = |at: usize, bytes: &[u8]| code[at..at + bytes.len()].copy_from_slice(bytes);
        put(0, &[0x68]);
        put(1, &0x70200_u32.to_le_bytes());
        put(17, &[0x8b, 0x87, 0x6c, 2, 0, 0]);
        put(30, &[0x8d, 0x8f, 0x6c, 2, 0, 0, 0xff, 0x50, 0x1c]);
        put(0x3b, &[0x68]);
        put(0x3c, &0x70100_u32.to_le_bytes());
        for at in [0x51, 0x8f] {
            put(at, &[0x68]);
            put(at + 1, &0x70500_u32.to_le_bytes());
        }
        for at in [0x63, 0xab] {
            put(at, &[0xf3, 0x0f, 0x10, 0x05]);
            put(at + 4, &0x70600_u32.to_le_bytes());
        }
        put(0x7c, &[0x77, 0x5f]);
        put(0xc4, &[0x77, 0x17, 0x6a, 0, 0x6a, 0]);
        for (at, target) in [
            (12, numeric.multiplication_va()),
            (0x33, 0x8a19b0),
            (0x41, 0x9a1760),
            (0x4c, numeric.division_va()),
            (0x8a, numeric.division_va()),
            (0xd8, 0x800140),
        ] {
            put(at, &[0xe8]);
            put(
                at + 1,
                &target.wrapping_sub(0x4ab4b2 + at as u32 + 5).to_le_bytes(),
            );
        }
        let mut point = vec![0; 0xa2];
        for at in [22, 42] {
            point[at] = 0x68;
            point[at + 1..at + 5].copy_from_slice(&0x70700_u32.to_le_bytes());
        }
        for (at, target) in [
            (0, 0x9a1da0_u32),
            (10, 0x9a0fe0),
            (28, numeric.multiplication_va()),
            (48, numeric.multiplication_va()),
            (75, numeric.addition_va()),
            (97, numeric.addition_va()),
            (121, 0x8adbf0),
            (131, 0x8ad880),
            (141, 0x8a9490),
            (150, 0x8add40),
            (157, 0x8abd40),
        ] {
            point[at] = 0xe8;
            point[at + 1..at + 5]
                .copy_from_slice(&target.wrapping_sub(0x8001b0 + at as u32 + 5).to_le_bytes());
        }
        let memory = vec![
            (0x4ab4b2, code),
            (0x8001b0, point),
            (
                0x4ab40e,
                [vec![0x68], 0x70200_u32.to_le_bytes().to_vec()].concat(),
            ),
            (
                0x6759e6,
                [
                    vec![0xc7, 0x86, 0x6c, 2, 0, 0],
                    0x70300_u32.to_le_bytes().to_vec(),
                ]
                .concat(),
            ),
            (0x7031c, 0x7fe970_u32.to_le_bytes().to_vec()),
            (0x7fe99e, vec![0x8b, 0x88, 0xc8, 0, 0, 0]),
            (
                0x8aeb10,
                [
                    vec![0xf3, 0x0f, 0x10, 0x05],
                    0x70500_u32.to_le_bytes().to_vec(),
                ]
                .concat(),
            ),
            (0x70100, b"1\0xxxxxxxxxxxxxxx".to_vec()),
            (0x70600, 0.0625_f32.to_bits().to_le_bytes().to_vec()),
            (0x70700, 0.375_f32.to_bits().to_le_bytes().to_vec()),
            (0x8abed7, vec![0x8b, 0x87, 0xfc, 0, 0, 0, 0x89, 0x46, 0x74]),
            (
                0x8abee0,
                vec![
                    0x8b, 0x87, 0, 1, 0, 0, 0x53, 0x56, 0x89, 0x86, 0x80, 0, 0, 0,
                ],
            ),
        ];
        (memory, numeric)
    }
    fn decode(memory: &Memory, numeric: &NumericPolicy) -> Result<Rules, String> {
        Rules::read(
            &mut |va, n, _| {
                memory
                    .iter()
                    .find_map(|(base, b)| {
                        va.checked_sub(*base)
                            .and_then(|at| b.get(at as usize..at as usize + n))
                            .map(<[u8]>::to_vec)
                    })
                    .ok_or("timed fixture mapping".into())
            },
            numeric,
        )
    }
    #[test]
    fn timed_controller_scalars_follow_nonstock_operands_and_reject_handoff_or_getter_drift() {
        let (mut memory, numeric) = fixture();
        let rules = decode(&memory, &numeric).unwrap();
        assert_eq!(
            (rules.period, rules.offset, rules.tolerance),
            (1., 0.375, 0.0625)
        );
        memory.iter_mut().find(|(va, _)| *va == 0x70700).unwrap().1 =
            0.75_f32.to_bits().to_le_bytes().to_vec();
        assert_eq!(decode(&memory, &numeric).unwrap().offset, 0.75);
        for (va, at) in [
            (0x4ab4b2, 1),
            (0x4ab4b2, 38),
            (0x4ab4b2, 0x4d),
            (0x4ab4b2, 0x67),
            (0x4ab4b2, 0xc4),
            (0x6759e6, 6),
            (0x7031c, 0),
            (0x7fe99e, 2),
            (0x8001b0, 1),
            (0x8001b0, 43),
            (0x8001b0, 158),
            (0x8abed7, 8),
            (0x8abee0, 10),
        ] {
            let (mut memory, numeric) = fixture();
            memory.iter_mut().find(|(base, _)| *base == va).unwrap().1[at] ^= 1;
            assert!(decode(&memory, &numeric).is_err(), "{va:x}+{at}");
        }
        for va in [0x70600, 0x70700] {
            for value in [0., -1., f32::NAN, f32::INFINITY] {
                let (mut memory, numeric) = fixture();
                memory.iter_mut().find(|(base, _)| *base == va).unwrap().1 =
                    value.to_bits().to_le_bytes().to_vec();
                assert!(decode(&memory, &numeric).is_err());
            }
        }
    }
}

#[cfg(test)]
mod witness_tests {
    use super::*;

    #[test]
    #[ignore = "requires PCX_WAR3_EXE and PCX_HEADING_TIMED_WITNESS; selected map namespace and four retained native controls"]
    fn selected_timed_controller_replays_all_four_stationary_native_controls() {
        use crate::campaign_data_set::{CampaignRecipe, ImageRules};
        use pcx_objects::editor::{
            EditorReadLimits, SlkId,
            defaults::{DefaultTables, Family, Identity},
        };
        let path = std::env::var("PCX_HEADING_TIMED_WITNESS")
            .expect("explicit private timed-facing witness");
        let witness: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(witness["total_samples"], 1024);
        assert_eq!(witness["complete"], true);
        let (image, mut origin, numeric, budget) = super::super::tests::selected_numeric_image();
        let digest = numeric
            .image_sha256()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        assert_eq!(witness["image_sha256"], digest);
        let registry = crate::campaign_client::order_registry(&image, origin.clone()).unwrap();
        let activity = crate::campaign_client::unit_activity(&image, origin.clone(), &registry)
            .unwrap()
            .with_heading(&image, &numeric, budget)
            .unwrap();
        let heading = activity.heading_rules().unwrap();
        let timed = heading.timed.as_ref().unwrap();
        let profile = heading.profile.as_ref().unwrap();
        let visual = heading.visual.as_ref().unwrap();
        origin.source = std::env::var("PCX_WAR3_EXE").unwrap();
        let rules = ImageRules::read(&image, origin.clone()).unwrap();
        let recipe = CampaignRecipe::pin(
            std::path::Path::new(&origin.source).parent().unwrap(),
            pcx_assets::GameMode::default(),
            rules,
            &image,
            None,
            "VERSION_REIGN_OF_CHAOS",
        )
        .unwrap();
        let map = std::fs::read(witness["map"].as_str().unwrap()).unwrap();
        let hash = crate::neutral_player::sha256(&map)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        assert_eq!(witness["map_sha256"], hash);
        let reader = recipe
            .open_map(
                &map,
                "retained timed-facing control",
                pcx_mpq::checked::ReadBudget {
                    max_archive_bytes: 64 << 20,
                    max_archive_table_bytes: 8 << 20,
                    max_raw_packed_bytes: 16 << 20,
                    max_decoded_bytes: 16 << 20,
                },
            )
            .unwrap();
        let data = reader
            .resolver
            .resolve_bounded(SlkId::UnitData.path(), 4 << 20)
            .unwrap();
        let tables = DefaultTables::from_bytes(
            Identity {
                install: origin.source.clone(),
                edition_version: origin.version.clone(),
            },
            &[(SlkId::UnitData.path(), data.bytes.as_slice())],
            &[],
            EditorReadLimits {
                max_listed_paths: 1,
                max_asset_bytes: 4 << 20,
                max_aggregate_input_bytes: 8 << 20,
            },
        )
        .unwrap();
        for arm in witness["arms"].as_array().unwrap() {
            let name = arm["arm"].as_str().unwrap();
            let kind = (arm["metadata"]["kind"].as_i64().unwrap() as i32).to_be_bytes();
            let row = tables
                .object(Family::Unit, std::str::from_utf8(&kind).unwrap())
                .unwrap();
            let selector = (row
                .cell(SlkId::UnitData, &profile.selector_column)
                .unwrap()
                .text
                .parse::<u32>()
                .unwrap()
                & profile.selector_mask) as usize;
            let turn = profile
                .authored(
                    &numeric,
                    row.cell(SlkId::UnitData, &profile.turn_column)
                        .unwrap()
                        .text,
                )
                .unwrap();
            let window = profile
                .radians(
                    profile
                        .authored(
                            &numeric,
                            row.cell(SlkId::UnitData, &profile.window_column)
                                .unwrap()
                                .text,
                        )
                        .unwrap(),
                )
                .unwrap();
            assert_eq!(
                arm["metadata"]["turn"].as_u64().unwrap(),
                u64::from(turn.to_bits())
            );
            assert_eq!(
                arm["metadata"]["prop"].as_u64().unwrap(),
                u64::from(window.to_bits())
            );
            let position = if name == "translated" {
                [2048., -2048.]
            } else {
                [0., 0.]
            };
            let mut state = super::super::visual::State {
                logical: 0.,
                angle: 0.,
                velocity: 0.,
            };
            let mut target = timed
                .prepare(
                    heading,
                    &numeric,
                    position,
                    state.angle,
                    profile.radians(135.).unwrap(),
                    1.,
                )
                .unwrap();
            let mut next = heading.period;
            while next <= 1. {
                next = f32::from_bits(real::add(next.to_bits(), heading.period.to_bits()).unwrap());
            }
            for sample in arm["samples"].as_array().unwrap() {
                let step = sample["step"].as_u64().unwrap();
                let until = 1. + step as f32 / 64.;
                while next <= until {
                    if let Some(active) = target {
                        let vector = [
                            f32::from_bits(
                                real::sub(active.point[0].to_bits(), position[0].to_bits())
                                    .unwrap(),
                            ),
                            f32::from_bits(
                                real::sub(active.point[1].to_bits(), position[1].to_bits())
                                    .unwrap(),
                            ),
                        ];
                        let goal = profile
                            .direction
                            .angle(&numeric, vector, heading.full_turn, profile.zero)
                            .unwrap();
                        let result = heading
                            .stationary_step(&numeric, state.logical, goal, active.speed)
                            .unwrap();
                        state.logical = result.logical;
                        if result.arrived {
                            target = None;
                        }
                    }
                    state = visual.step(heading, &numeric, state, selector).unwrap();
                    next = f32::from_bits(
                        real::add(next.to_bits(), heading.period.to_bits()).unwrap(),
                    );
                }
                if step == 16 {
                    if name == "replacement" {
                        target = timed
                            .prepare(
                                heading,
                                &numeric,
                                position,
                                state.angle,
                                profile.radians(270.).unwrap(),
                                0.75,
                            )
                            .unwrap();
                    }
                    if name == "interruption" {
                        state.logical = heading
                            .normalize(&numeric, profile.radians(45.).unwrap())
                            .unwrap();
                    }
                }
                if step == 32 && name == "interruption" {
                    target = None;
                }
                assert_eq!(
                    profile.degrees(state.angle).unwrap().to_bits(),
                    sample["face_bits"].as_u64().unwrap() as u32,
                    "{name} step {step} logical={:?}",
                    state.logical
                );
            }
            assert!(target.is_none(), "{name} relation did not retire");
        }
    }
}

use super::*;

pub(crate) fn terrain(size: [u32; 2], origin: [f32; 2], flags: &[u8]) -> Terrain {
    assert_eq!(flags.len(), (size[0] * size[1]) as usize);
    let mut bytes = b"W3E!".to_vec();
    bytes.extend(11u32.to_le_bytes());
    bytes.push(b'L');
    bytes.extend(0u32.to_le_bytes());
    bytes.extend(1u32.to_le_bytes());
    bytes.extend(b"T001");
    bytes.extend(0u32.to_le_bytes());
    for n in size {
        bytes.extend(n.to_le_bytes());
    }
    for n in origin {
        bytes.extend(n.to_le_bytes());
    }
    for flag in flags {
        bytes.extend([0, 0, 0, 0, *flag, 0, 0]);
    }
    Terrain::from_bytes(bytes).unwrap()
}

pub(crate) fn identity() -> AssetIdentity {
    AssetIdentity {
        source: "synthetic terrain".into(),
        edition: "fixture".into(),
        version: "fixture".into(),
    }
}

fn band() -> InitialBlight {
    let terrain = terrain([4, 2], [0.0, 0.0], &[0, 0x20, 0, 0, 0, 0x20, 0, 0]);
    InitialBlight::read(&terrain, 40.0, identity(), 8).unwrap()
}

#[test]
fn blight_reads_only_the_format_bit_and_preserves_source_identity() {
    for flags in 0..16 {
        let terrain = terrain([1, 1], [-80.0, 120.0], &[flags << 4]);
        assert_eq!(terrain.corners()[0].is_blighted(), flags & 2 != 0);
        let blight = InitialBlight::read(&terrain, 13.0, identity(), 1).unwrap();
        assert_eq!(blight.origin, identity());
        assert_eq!(blight.is_blighted([-80.0, 120.0]).unwrap(), flags & 2 != 0);
        assert!(!blight.is_blighted([-100.0, 120.0]).unwrap());
    }
}

#[test]
fn packed_corner_flags_cross_byte_boundaries() {
    let flags = [0, 0x20, 0, 0x20, 0, 0, 0x20, 0, 0x20, 0x20, 0];
    let ground = terrain([11, 1], [-10.0, 20.0], &flags);
    let blight = InitialBlight::read(&ground, 7.0, identity(), flags.len()).unwrap();
    for (index, flag) in flags.into_iter().enumerate() {
        assert_eq!(
            blight
                .is_blighted([-10.0 + index as f32 * 7.0, 20.0])
                .unwrap(),
            flag & 0x20 != 0
        );
    }
}

#[test]
fn segment_integration_solves_crossings_and_composes_in_both_directions() {
    let blight = band();
    for (start, end) in [([0.0, 0.0], [120.0, 0.0]), ([120.0, 0.0], [0.0, 0.0])] {
        let whole = blight.seconds(start, end, 12.0, &mut 10).unwrap();
        assert!((whole - 4.0).abs() < 1e-12);
        let middle = [60.0, 0.0];
        let parts = blight.seconds(start, middle, 6.0, &mut 10).unwrap()
            + blight.seconds(middle, end, 6.0, &mut 10).unwrap();
        assert!((parts - whole).abs() < 1e-12);
    }
    let outside = blight
        .seconds([-80.0, 0.0], [200.0, 0.0], 28.0, &mut 10)
        .unwrap();
    assert!((outside - 4.0).abs() < 1e-12);
    assert_eq!(
        blight
            .seconds([40.0, 0.0], [40.0, 0.0], 7.0, &mut 1)
            .unwrap(),
        7.0
    );
    assert_eq!(
        blight
            .seconds([0.0, 0.0], [120.0, 0.0], 0.0, &mut 0)
            .unwrap(),
        0.0
    );
}

#[test]
fn diagonals_and_boundary_starts_do_not_double_count_cells() {
    let terrain = terrain([3, 3], [0.0, 0.0], &[0, 0, 0, 0, 0x20, 0, 0, 0, 0]);
    let blight = InitialBlight::read(&terrain, 10.0, identity(), 9).unwrap();
    assert_eq!(
        blight
            .seconds([0.0, 0.0], [20.0, 20.0], 8.0, &mut 3)
            .unwrap(),
        4.0
    );
    assert_eq!(
        blight
            .seconds([15.0, 15.0], [5.0, 5.0], 4.0, &mut 1)
            .unwrap(),
        4.0
    );
    assert_eq!(
        blight
            .seconds([5.0, 5.0], [15.0, 15.0], 4.0, &mut 1)
            .unwrap(),
        4.0
    );
}

#[test]
fn malformed_inputs_and_work_exhaustion_refuse() {
    assert!(GrowthRules::reviewed_legacy(&[]).is_err());
    let terrain = terrain([1, 1], [0.0, 0.0], &[0x20]);
    assert!(InitialBlight::read(&terrain, 10.0, identity(), 0).is_err());
    for spacing in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert!(InitialBlight::read(&terrain, spacing, identity(), 1).is_err());
    }
    let mut foreign = identity();
    foreign.source.clear();
    assert!(InitialBlight::read(&terrain, 10.0, foreign, 1).is_err());
    let blight = band();
    assert!(blight.is_blighted([f32::NAN, 0.0]).is_err());
    assert!(
        blight
            .seconds([0.0; 2], [120.0, 0.0], 12.0, &mut 1)
            .unwrap_err()
            .contains("work exhausted")
    );
    assert!(blight.seconds([0.0; 2], [0.0; 2], -1.0, &mut 10).is_err());
}

#[test]
fn growth_uses_discrete_steps_and_spatial_rectangles() {
    let spec = GrowthSpec {
        spacing: 10.0,
        fine_spacing: 2.5,
        interval: 0.25,
        increment: 5.0,
        target: 40.0,
        tolerance: 0.01,
    };
    let growth = Growth::new([15.0, 15.0], [0.0; 2], 4.0, spec).unwrap();
    assert!(!growth.is_blighted_at([15.0, 15.0], 3.9).unwrap());
    assert!(growth.is_blighted_at([15.0, 15.0], 4.0).unwrap());
    assert!(growth.is_blighted_at([25.0, 15.0], 4.0).unwrap());
    assert!(!growth.is_blighted_at([5.0, 15.0], 4.0).unwrap());
    assert!(!growth.is_blighted_at([45.0, 15.0], 4.5).unwrap());
    assert!(growth.mask_at([45.0, 15.0], 4.5, true).unwrap());
    assert!(growth.is_blighted_at([15.0, 45.0], 4.5).unwrap());
    assert!(growth.is_blighted_at([45.0, 15.0], 5.0).unwrap());
    assert!(growth.is_blighted_at([15.0, 15.0], f64::NAN).is_err());
    let negative = Growth::new([-15.0, -5.0], [-30.0, -20.0], 4.0, spec).unwrap();
    assert!(negative.is_blighted_at([-25.0, -5.0], 4.0).unwrap());
    assert!(!negative.is_blighted_at([-5.0, -5.0], 4.0).unwrap());
    let shifted = Growth::new([15.0; 2], [3.0, 7.0], 4.0, spec).unwrap();
    assert!(shifted.is_blighted_at([9.0, 12.0], 4.0).unwrap());
    assert!(!shifted.is_blighted_at([29.0, 12.0], 4.0).unwrap());
    let circle = Growth::new([55.0; 2], [0.0; 2], 0.0, spec).unwrap();
    assert!(circle.is_blighted_at([75.0, 25.0], 1.0).unwrap());
    assert!(!circle.is_blighted_at([85.0, 25.0], 1.0).unwrap());
    assert!(circle.is_blighted_at([85.0, 35.0], 1.0).unwrap());
    assert!(!circle.is_blighted_at([95.0, 35.0], 1.0).unwrap());
    assert!(
        Growth::new(
            [0.0; 2],
            [0.0; 2],
            0.0,
            GrowthSpec {
                spacing: 0.0,
                ..spec
            }
        )
        .is_err()
    );
}

#[test]
fn growth_state_integrates_the_clock_and_refuses_overlap_or_removal() {
    use std::num::NonZeroU64;
    let base_terrain = terrain([5, 5], [0.0; 2], &[0; 25]);
    let mut state = InitialBlight::read(&base_terrain, 10.0, identity(), 25).unwrap();
    let unit = |n| HandleId::new(NonZeroU64::new(n).unwrap());
    let growth = Growth::new(
        [15.0; 2],
        [0.0; 2],
        0.0,
        GrowthSpec {
            spacing: 10.0,
            fine_spacing: 2.5,
            interval: 0.25,
            increment: 5.0,
            target: 40.0,
            tolerance: 0.01,
        },
    )
    .unwrap();
    state.add_growth(unit(1), unit(11), growth.clone()).unwrap();
    assert!(state.owner_mask_at([15.0; 2], 0.0, Some(unit(11))).unwrap());
    assert!(!state.owner_mask_at([15.0; 2], 0.0, Some(unit(12))).unwrap());
    assert!(!state.is_blighted_at([45.0, 15.0], 0.5).unwrap());
    assert!(state.is_blighted_at([45.0, 15.0], 1.0).unwrap());
    assert_eq!(
        state
            .seconds_at([45.0, 15.0], [45.0, 15.0], 0.0, 2.0, &mut 20)
            .unwrap(),
        1.0
    );
    state.add_growth(unit(2), unit(12), growth).unwrap();
    assert!(
        state
            .is_blighted_at([15.0; 2], 0.0)
            .unwrap_err()
            .contains("overlap")
    );
    state.remove_growth(unit(1));
    assert!(
        state
            .is_blighted_at([15.0; 2], 0.0)
            .unwrap_err()
            .contains("removed")
    );
}

#[test]
fn removal_effect_refuses_only_placement_in_its_reached_area() {
    let map = terrain([9, 9], [0.0; 2], &[0; 81]);
    let mut state = InitialBlight::read(&map, 10.0, identity(), 81).unwrap();
    let unit = HandleId::new(std::num::NonZeroU64::new(7).unwrap());
    let effect = Growth::new(
        [45.0; 2],
        [0.0; 2],
        2.0,
        GrowthSpec {
            spacing: 10.0,
            fine_spacing: 2.5,
            interval: 0.25,
            increment: 5.0,
            target: 20.0,
            tolerance: 0.01,
        },
    )
    .unwrap();
    state.add_unqualified_removal(unit, unit, effect).unwrap();
    assert!(!state.is_blighted_for_placement([45.0; 2], 1.0).unwrap());
    assert!(!state.is_blighted_for_placement([5.0; 2], 3.0).unwrap());
    assert!(!state.is_blighted_at([45.0; 2], 3.0).unwrap());
    assert!(
        state
            .is_blighted_for_placement([45.0; 2], 3.0)
            .unwrap_err()
            .contains("HandleId(7)")
    );
}

#[test]
fn script_patch_is_immediate_and_does_not_grow() {
    let patch = Growth::fixed([15.0; 2], [0.0; 2], 10.0, 2.5, 3.0, 5.0).unwrap();
    assert!(!patch.is_blighted_at([15.0; 2], 2.9).unwrap());
    assert!(patch.is_blighted_at([15.0; 2], 3.0).unwrap());
    assert!(!patch.is_blighted_at([85.0, 15.0], 3.0).unwrap());
    assert!(!patch.is_blighted_at([85.0, 15.0], 999.0).unwrap());
    let base_terrain = terrain([5, 5], [0.0; 2], &[0; 25]);
    let mut state = InitialBlight::read(&base_terrain, 10.0, identity(), 25).unwrap();
    let owner = HandleId::new(std::num::NonZeroU64::new(91).unwrap());
    state.add_script_patch(owner, patch.clone()).unwrap();
    state.add_script_patch(owner, patch).unwrap();
    assert_eq!(
        state
            .seconds_at([15.0; 2], [15.0; 2], 2.0, 5.0, &mut 10)
            .unwrap(),
        2.0
    );
    assert_eq!(
        state
            .seconds_at([0.0, 15.0], [40.0, 15.0], 3.0, 11.0, &mut 100)
            .unwrap(),
        4.0
    );
    let terrain = terrain([9, 9], [3.0, 7.0], &[0; 81]);
    let mut shifted = InitialBlight::read(&terrain, 10.0, identity(), 81).unwrap();
    shifted
        .add_script_patch(
            owner,
            Growth::fixed([15.0; 2], [3.0, 7.0], 10.0, 2.5, 0.0, 5.0).unwrap(),
        )
        .unwrap();
    assert_eq!(
        shifted
            .seconds_at([3.0, 12.0], [33.0, 12.0], 0.0, 12.0, &mut 100)
            .unwrap(),
        8.0
    );
}

#[test]
#[ignore = "requires the local original-client blight probe, selected ability table and executable"]
fn growth_matches_recorded_original_samples() {
    use std::{env, fs, path::PathBuf};

    let probe = PathBuf::from(env::var_os("PCX_BLIGHT_PROBE").expect("PCX_BLIGHT_PROBE"));
    let script = fs::read_to_string(probe.join("probe3.j")).unwrap();
    let between = |line: &str, marker: &str, end: char| {
        line.split_once(marker)
            .unwrap()
            .1
            .split(end)
            .next()
            .unwrap()
            .trim()
            .parse::<f32>()
            .unwrap()
    };
    let timer = script
        .lines()
        .find(|line| line.contains("function PCXO_FastTick)") && line.contains("TimerStart"))
        .unwrap();
    let period = between(timer, "TimerStart(CreateTimer(),", ',');
    let distance = script
        .lines()
        .find(|line| line.contains("set d = I2R(i) *"))
        .unwrap();
    let distance = between(distance, "set d = I2R(i) *", '\n');
    let table =
        pcx_slq::Table::parse(&fs::read(env::var_os("PCX_BLIGHT_ABILITY_SLK").unwrap()).unwrap())
            .unwrap();
    let alias = env::var("PCX_BLIGHT_ABILITY_ID").unwrap();
    let row = table
        .data_rows()
        .find(|row| row.get(1) == Some(alias.as_str()))
        .unwrap();
    let number = |name| row.by_name(name).unwrap().parse::<f32>().unwrap();
    let image = fs::read(env::var_os("PCX_WAR3_EXE").unwrap()).unwrap();
    let rules = GrowthRules::reviewed_legacy(&image).unwrap();
    let terrain =
        Terrain::from_bytes(fs::read(env::var_os("PCX_BLIGHT_TERRAIN").unwrap()).unwrap()).unwrap();
    let origin = terrain.origin();
    let site = env::var("PCX_BLIGHT_SITE").unwrap();
    let mut site = site.split(',').map(|part| part.parse::<f32>().unwrap());
    let center = [site.next().unwrap(), site.next().unwrap()];
    assert!(site.next().is_none());
    let growth = Growth::new(
        center,
        [origin.0, origin.1],
        0.0,
        GrowthSpec {
            spacing: rules.spacing,
            fine_spacing: rules.fine_spacing,
            interval: number("Dur1").min(rules.interval_cap),
            increment: number("DataA1"),
            target: number("Area1"),
            tolerance: rules.tolerance,
        },
    )
    .unwrap();
    let directory =
        probe.join("run/prefix/drive_c/users/wine/Documents/Warcraft III/CustomMapData");
    let mut files: Vec<_> = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter_map(|path| {
            let name = path.file_stem()?.to_str()?;
            Some((name.strip_prefix("PCXF-")?.parse::<usize>().ok()?, path))
        })
        .collect();
    files.sort_unstable_by_key(|(index, _)| *index);
    let mut samples = 0_usize;
    for (_, path) in files {
        let contents = fs::read_to_string(path).unwrap();
        for token in contents
            .split_whitespace()
            .filter(|token| token.starts_with('t') && token.contains(':'))
        {
            let (_, bits) = token.split_once(':').unwrap();
            let bits = bits.trim_end_matches('"');
            assert_eq!(bits.len(), 44);
            let now = samples as f64 * f64::from(period);
            for (index, expected) in bits.bytes().enumerate() {
                let d = (index / 4) as f32 * distance;
                let point = match index % 4 {
                    0 => [center[0] + d, center[1]],
                    1 => [center[0] - d, center[1]],
                    2 => [center[0], center[1] + d],
                    _ => [center[0], center[1] - d],
                };
                assert_eq!(
                    growth.is_blighted_at(point, now).unwrap(),
                    expected == b'1',
                    "sample={samples} point={point:?}"
                );
            }
            samples += 1;
        }
    }
    assert_eq!(samples, 600);
}

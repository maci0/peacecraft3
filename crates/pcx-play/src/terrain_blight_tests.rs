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

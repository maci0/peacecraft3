use super::*;

pub(crate) fn fixture(exponent: u32, initial: f32) -> (Vec<u8>, Binding, RuleInputBudget) {
    let (mut bytes, old, mut budget) = crate::terrain_pathing::tests::fixture(exponent, &[8]);
    let address = |offset: usize| 0x301000 + (offset as u32 - 0x200);
    let mut put = |offset: usize, code: &[u8]| {
        bytes[offset..offset + code.len()].copy_from_slice(code);
        SourceWindow {
            va: address(offset),
            len: code.len() as u32,
            digest: sha256(code),
        }
    };
    let initial_position = put(
        0x300,
        &[&[0xa1], &address(0x500).to_le_bytes()[..]].concat(),
    );
    let small_radius = put(
        0x310,
        &[&[0xf3, 0x0f, 0x10, 0x05], &address(0x504).to_le_bytes()[..]].concat(),
    );
    let large_radius = put(
        0x320,
        &[&[0xf3, 0x0f, 0x10, 0x05], &address(0x508).to_le_bytes()[..]].concat(),
    );
    let initializer = |value, target: u32| {
        [
            &[0x6a, value, 0x68][..],
            &target.to_le_bytes(),
            &[0xe8, 0, 0, 0, 0, 0x83, 0xc4, 8, 0xc3],
        ]
        .concat()
    };
    let one_initializer = put(0x340, &initializer(1, address(0x510)));
    let two_initializer = put(0x360, &initializer(2, address(0x514)));
    let ground_mask = put(0x380, &[0xb9, 8, 0, 0, 0]);
    for (offset, value) in [(0x500, initial), (0x504, 0.5_f32), (0x508, 1.5)] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    budget.profile_bytes = 1024;
    let binding = Binding {
        origin: old.origin,
        image_sha256: sha256(&bytes),
        normalization: old.normalization,
        initial_position,
        small_radius,
        large_radius,
        one_initializer,
        two_initializer,
        ground_mask,
    };
    (bytes, binding, budget)
}

pub(crate) fn rules(exponent: u32, initial: f32) -> Rules {
    let (bytes, binding, budget) = fixture(exponent, initial);
    Rules::read(&bytes, binding, budget).unwrap()
}

pub(crate) fn placement(source: Source, queries: usize) -> Placement {
    Placement::new(
        Grid {
            source,
            origin: [0.0; 2],
            pathing: Pathing {
                version: 0,
                width: 32,
                height: 32,
                cells: vec![0; 1024],
            },
            domains: vec![5; 1024],
        },
        rules(2, -8192.25),
        BTreeMap::new(),
        Budget {
            cells: 1024,
            footprint_cells: 64,
            queries,
        },
    )
    .unwrap()
}

#[test]
fn operands_not_selected_image_values_drive_normalization_and_geometry() {
    for exponent in [1, 3, 5] {
        let r = rules(exponent, -8192.25);
        assert_eq!(r.cell_size(), (1u32 << exponent) as f32);
        assert_eq!(r.extents(0.49), [0, 0]);
        assert_eq!(r.extents(0.5), [1, 0]);
        assert_eq!(r.extents(1.0), [1, 1]);
        assert_eq!(r.extents(1.5), [2, 1]);
        assert!(r.shift(1.0_f32.to_bits(), -127).is_err());
    }
    let r = rules(2, -8192.25);
    let request = [32.0005, 32.0];
    assert_ne!(r.normalize(request, [0.0; 2]).unwrap()[0], request[0]);
    let r = rules(2, 0.0);
    assert_eq!(r.normalize(request, [0.0; 2]).unwrap(), request);
    let (mut bytes, mut binding, budget) = fixture(2, 0.0);
    bytes[0x341] = 3;
    bytes[0x361] = 6;
    bytes[0x504..0x508].copy_from_slice(&1.5_f32.to_le_bytes());
    bytes[0x508..0x50c].copy_from_slice(&4.5_f32.to_le_bytes());
    binding.one_initializer.digest = sha256(&bytes[0x340..0x350]);
    binding.two_initializer.digest = sha256(&bytes[0x360..0x370]);
    binding.image_sha256 = sha256(&bytes);
    let r = Rules::read(&bytes, binding, budget).unwrap();
    assert_eq!(r.extents(3.0), [3, 3]);
}

#[test]
fn malformed_source_windows_and_budgets_are_refused() {
    for mutation in 0..9 {
        let (mut bytes, mut binding, mut budget) = fixture(2, -8192.25);
        match mutation {
            0 => bytes[0x500] ^= 1,
            1 => budget.image_bytes = 1,
            2 => budget.profile_bytes = 1,
            3 => binding.origin.version.clear(),
            4 => binding.initial_position.va = u32::MAX,
            5 => {
                bytes[0x380] = 0xb8;
                binding.ground_mask.digest = sha256(&bytes[0x380..0x385]);
                binding.image_sha256 = sha256(&bytes);
            }
            6 => {
                bytes[0x340] = 0x90;
                binding.one_initializer.digest = sha256(&bytes[0x340..0x350]);
                binding.image_sha256 = sha256(&bytes);
            }
            7 => {
                bytes[0x504..0x508].copy_from_slice(&2.0_f32.to_le_bytes());
                binding.image_sha256 = sha256(&bytes);
            }
            _ => {
                bytes[0x500..0x504].copy_from_slice(&f32::NAN.to_le_bytes());
                binding.image_sha256 = sha256(&bytes);
            }
        }
        assert!(
            Rules::read(&bytes, binding, budget).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn occupancy_and_query_refusal_are_transactional_and_load_independent() {
    let source = Source {
        install: "synthetic".into(),
        edition: "fixture".into(),
        map: "world".into(),
    };
    let mut p = placement(source.clone(), 1);
    let first = p.prepare([32.0, 32.0], Some(2.0), "_", 0.0).unwrap();
    assert_eq!(first.position, [32.0, 32.0]);
    p.commit(first);
    let before = p.occupied.clone();
    assert!(
        p.prepare([32.0, 32.0], Some(2.0), "_", 0.0)
            .unwrap_err()
            .0
            .contains("budget")
    );
    assert_eq!(p.occupied, before);
    let run = || {
        let mut p = placement(source.clone(), 100);
        let mut positions = Vec::new();
        for _ in 0..4 {
            let next = p.prepare([32.0, 32.0], Some(2.0), "_", 0.0).unwrap();
            positions.push(next.position.map(f32::to_bits));
            p.commit(next);
        }
        positions
    };
    assert_eq!(run(), run());
    assert_ne!(run()[0], run()[1]);
    assert!(p.prepare([-1.0, 32.0], None, "_", 0.0).is_err());
    assert!(p.prepare([32.0, 32.0], Some(20.0), "_", 0.0).is_err());
    assert!(
        p.prepare([32.0, 32.0], Some(f32::MIN_POSITIVE), "_", 0.0)
            .is_err()
    );
    assert!(
        p.prepare([32.0, 32.0], Some(f32::from_bits(1)), "_", 0.0)
            .is_err()
    );
}

#[test]
fn footprint_rotation_and_missing_texture_refusal_use_the_same_collision_cache() {
    let source = Source {
        install: "synthetic".into(),
        edition: "fixture".into(),
        map: "world".into(),
    };
    let mut p = placement(source, 1);
    assert!(p.prepare([32.0, 32.0], None, "synthetic.tga", 0.0).is_err());
    assert!(p.occupied.iter().all(|&count| count == 0));
    p.footprints.insert(
        "synthetic.tga".into(),
        Footprint {
            origin: AssetIdentity {
                edition: "synthetic".into(),
                version: "fixture".into(),
                source: "synthetic:footprint".into(),
            },
            cells: vec![[0, 0]],
        },
    );
    assert!(
        p.prepare([32.0, 32.0], None, "synthetic.tga", 45.0)
            .is_err()
    );
    assert!(
        p.prepare([32.0, 32.0], None, "synthetic.tga", f32::MAX)
            .is_err()
    );
    let building = p
        .prepare([32.0, 32.0], None, "synthetic.tga", 90.0)
        .unwrap();
    p.commit(building);
    assert_eq!(p.grid.pathing.cell(7, 8), Some(p.rules.mask));
    assert_eq!(p.grid.pathing.cell(8, 8), Some(0));
    assert!(
        p.prepare([28.0, 32.0], Some(0.0), "_", 0.0)
            .unwrap_err()
            .0
            .contains("budget")
    );
}

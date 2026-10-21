use super::*;
fn id(n: u64) -> HandleId {
    HandleId::new(std::num::NonZeroU64::new(n).unwrap())
}

#[test]
fn live_motion_excludes_only_its_own_stamp_and_releases_the_previous_cells() {
    let mut p = placement(
        Source {
            install: "synthetic".into(),
            edition: "test".into(),
            map: "motion".into(),
        },
        10000,
    );
    let first = p.prepare([32., 32.], Some(2.), "_", 0.).unwrap();
    p.commit(id(1), first);
    let second = p.prepare([48., 32.], Some(2.), "_", 0.).unwrap();
    p.commit(id(2), second);
    assert!(p.prepare_motion(id(1), [32., 32.], 2.).unwrap().is_some());
    assert!(p.prepare_motion(id(1), [48., 32.], 2.).unwrap().is_none());
    let moved = p.prepare_motion(id(1), [64., 32.], 2.).unwrap().unwrap();
    p.commit(id(1), moved);
    assert_eq!(
        p.prepare([32., 32.], Some(2.), "_", 0.).unwrap().position,
        [32., 32.]
    );
    assert!(p.prepare_motion(id(1), [48., 32.], 2.).unwrap().is_none());
    assert!(p.prepare_motion(id(3), [32., 32.], 2.).is_err());
}

#[test]
fn overlapping_dynamic_footprints_preserve_other_owners_and_static_pathing() {
    let mut p = placement(
        Source {
            install: "synthetic".into(),
            edition: "test".into(),
            map: "footprints".into(),
        },
        10000,
    )
    .with_footprint_loader(|_| {
        Ok(Footprint {
            origin: AssetIdentity {
                source: "synthetic".into(),
                edition: "test".into(),
                version: "test".into(),
            },
            cells: vec![[0, 0], [0, 0]],
        })
    });
    let a = p.prepare([32., 32.], None, "fixture.tga", 0.).unwrap();
    p.commit(id(1), a);
    let b = p.prepare([32., 32.], None, "fixture.tga", 0.).unwrap();
    p.commit(id(2), b);
    let old = 8 * 32 + 8;
    assert_eq!(p.footprint_occupied[old], 2);
    let a = p.prepare([64., 32.], None, "fixture.tga", 0.).unwrap();
    p.commit(id(1), a);
    assert!(p.blocked(old, None));
    let b = p.prepare([80., 32.], None, "fixture.tga", 0.).unwrap();
    p.commit(id(2), b);
    assert!(!p.blocked(old, None));
    p.grid.pathing.cells[old] |= p.rules.mask;
    let a = p.prepare([32., 32.], None, "fixture.tga", 0.).unwrap();
    p.commit(id(1), a);
    let a = p.prepare([64., 32.], None, "fixture.tga", 0.).unwrap();
    p.commit(id(1), a);
    assert!(p.blocked(old, None));
}

#[test]
fn routes_avoid_authored_walls_and_refuse_work_without_changing_occupancy() {
    let mut p = placement(
        Source {
            install: "synthetic".into(),
            edition: "test".into(),
            map: "route".into(),
        },
        10000,
    );
    let a = p.prepare([16., 16.], Some(0.), "_", 0.).unwrap();
    p.commit(id(1), a);
    for y in 0..32 {
        if y != 20 {
            p.grid.pathing.cells[y * 32 + 10] = p.rules.mask;
        }
    }
    let before = p.occupied.clone();
    assert!(p.route(id(1), [16., 16.], [64., 16.], 0., &mut 1).is_err());
    assert_eq!(p.occupied, before);
    let path = p
        .route(id(1), [16., 16.], [64., 16.], 0., &mut 10000)
        .unwrap();
    assert!(path.len() > 1);
    assert_eq!(path.last(), Some(&[64., 16.]));
    let mut previous = [16., 16.];
    for point in path {
        assert!(
            p.segment_clear(id(1), previous, point, 0., &mut 1000)
                .unwrap()
        );
        previous = point;
    }
    assert_eq!(p.occupied, before);
}

#[test]
fn lazy_footprints_cache_success_and_refuse_oversized_data_before_occupancy_changes() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let source = Source {
        install: "synthetic".into(),
        edition: "fixture".into(),
        map: "world".into(),
    };
    let hits = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&hits);
    let mut p = placement(source, 100).with_footprint_loader(move |path| {
        count.fetch_add(1, Ordering::SeqCst);
        Ok(Footprint {
            origin: AssetIdentity {
                source: "synthetic texture".into(),
                edition: "fixture".into(),
                version: "fixture".into(),
            },
            cells: vec![[0, 0]; if path == "too-big.tga" { 65 } else { 1 }],
        })
    });
    let before = p.grid.pathing.cells.clone();
    assert!(p.prepare([32.0, 32.0], None, "too-big.tga", 0.0).is_err());
    assert!(!p.footprints.contains_key("too-big.tga"));
    assert_eq!(p.grid.pathing.cells, before);
    assert!(p.occupied.iter().all(|x| *x == 0));
    let prepared = p.prepare([32.0, 32.0], None, "ok.tga", 0.0).unwrap();
    assert_eq!(p.grid.pathing.cells, before);
    p.commit(id(1), prepared);
    p.prepare([40.0, 32.0], None, "ok.tga", 0.0).unwrap();
    assert_eq!(hits.load(Ordering::SeqCst), 2);
    assert!(p.blocked(8 * 32 + 8, None));
}

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
    p.commit(id(1), first);
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
        for n in 1..=4 {
            let next = p.prepare([32.0, 32.0], Some(2.0), "_", 0.0).unwrap();
            positions.push(next.position.map(f32::to_bits));
            p.commit(id(n), next);
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
fn large_creation_radius_uses_the_full_source_classified_footprint() {
    let source = Source {
        install: "synthetic".into(),
        edition: "fixture".into(),
        map: "large radius".into(),
    };
    let mut p = placement(source.clone(), 100);
    // This fixture has four-unit cells and its largest band begins at radius six.
    let first = p.prepare([32.0, 32.0], Some(6.0), "_", 0.0).unwrap();
    assert_eq!(first.position, [32.0, 32.0]);
    assert_eq!(first.cells.len(), 16);
    assert!(first.cells.contains(&p.index(6, 6).unwrap()));
    assert!(first.cells.contains(&p.index(9, 9).unwrap()));
    p.commit(id(1), first);
    let second = p.prepare([32.0, 32.0], Some(6.0), "_", 0.0).unwrap();
    assert_ne!(second.position, [32.0, 32.0]);
    assert!(second.cells.iter().all(|&cell| p.occupied[cell] == 0));

    let mut obstructed = placement(source, 1);
    let corner = obstructed.index(6, 6).unwrap();
    obstructed.grid.pathing.cells[corner] = obstructed.rules.ground_mask();
    // The outer corner blocks this large footprint although the centre is free.
    let before = obstructed.occupied.clone();
    assert!(
        obstructed
            .prepare([32.0, 32.0], Some(6.0), "_", 0.0)
            .is_err()
    );
    assert_eq!(obstructed.occupied, before);
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
    p.commit(id(1), building);
    assert!(p.blocked(8 * 32 + 7, None));
    assert_eq!(p.grid.pathing.cell(8, 8), Some(0));
    assert!(
        p.prepare([28.0, 32.0], Some(0.0), "_", 0.0)
            .unwrap_err()
            .0
            .contains("budget")
    );
}

#[test]
fn hidden_stamps_follow_motion_without_blocking_others_and_reveal_only_in_free_space() {
    let mut p = placement(
        Source {
            install: "synthetic".into(),
            edition: "test".into(),
            map: "visibility".into(),
        },
        10000,
    );
    let a = p.prepare([32., 32.], Some(2.), "_", 0.).unwrap();
    p.commit(id(1), a);
    p.hide(id(1)).unwrap();
    p.hide(id(1)).unwrap();
    let b = p.prepare([32., 32.], Some(2.), "_", 0.).unwrap();
    assert_eq!(b.position, [32., 32.]);
    p.commit(id(2), b);
    let hidden = p.prepare_motion(id(1), [32., 32.], 2.).unwrap().unwrap();
    p.commit(id(1), hidden);
    let revealed = p
        .prepare_show(id(1), [32., 32.], Some(2.), "_", 0.)
        .unwrap();
    assert_ne!(revealed.position, [32., 32.]);
    p.commit(id(1), revealed);
    assert!(p.prepare_motion(id(1), [32., 32.], 2.).unwrap().is_none());
    assert!(p.hide(id(7)).is_err());
}

#[test]
fn failed_reveal_preserves_hidden_occupancy_and_never_erases_static_ground() {
    let mut p = placement(
        Source {
            install: "synthetic".into(),
            edition: "test".into(),
            map: "visibility".into(),
        },
        10000,
    );
    let a = p.prepare([32., 32.], Some(2.), "_", 0.).unwrap();
    p.commit(id(1), a);
    p.hide(id(1)).unwrap();
    for cell in &mut p.grid.pathing.cells {
        *cell |= p.rules.mask;
    }
    let occupied = p.occupied.clone();
    let ground = p.grid.pathing.cells.clone();
    assert!(
        p.prepare_show(id(1), [32., 32.], Some(2.), "_", 0.)
            .is_err()
    );
    assert_eq!(p.occupied, occupied);
    assert_eq!(p.grid.pathing.cells, ground);
    assert!(!p.stamps[0].1.active);
}

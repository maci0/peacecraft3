use super::*;
fn id(n: u64) -> HandleId {
    HandleId::new(std::num::NonZeroU64::new(n).unwrap())
}

#[derive(Default)]
struct SourceLedger {
    loads: usize,
    staged: BTreeSet<String>,
    committed: BTreeSet<String>,
}

struct TrackingLoader(std::sync::Arc<std::sync::Mutex<SourceLedger>>);

impl TrackingLoader {
    fn new() -> (Self, std::sync::Arc<std::sync::Mutex<SourceLedger>>) {
        let ledger = std::sync::Arc::new(std::sync::Mutex::new(SourceLedger::default()));
        (Self(std::sync::Arc::clone(&ledger)), ledger)
    }
}

impl FootprintLoader for TrackingLoader {
    fn load(&mut self, path: &str) -> Result<Footprint, String> {
        let mut ledger = self.0.lock().unwrap();
        ledger.loads += 1;
        if !ledger.staged.contains(path) && !ledger.committed.contains(path) {
            ledger.staged.insert(path.into());
        }
        Ok(Footprint {
            origin: AssetIdentity {
                source: format!("synthetic source at {path}"),
                edition: "test".into(),
                version: "test".into(),
            },
            cells: vec![[0, 0]],
        })
    }

    fn commit(&mut self, paths: &[String]) {
        let mut ledger = self.0.lock().unwrap();
        for path in paths {
            ledger.staged.remove(path);
            ledger.committed.insert(path.clone());
        }
    }

    fn rollback(&mut self, paths: &[String]) {
        let mut ledger = self.0.lock().unwrap();
        for path in paths {
            ledger.staged.remove(path);
        }
    }
}

fn script_base() -> Pathing {
    Pathing {
        version: 0,
        width: 32,
        height: 32,
        cells: vec![0; 1024],
    }
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
fn movement_stamp_rectangle_uses_radius_class_and_transposes_axes() {
    for (radius, lower, upper) in [
        (1.0, [10, 8], [11, 9]),
        (2.0, [9, 7], [11, 9]),
        (4.0, [9, 7], [12, 10]),
        (6.0, [8, 6], [12, 10]),
    ] {
        let mut p = placement(
            Source {
                install: "synthetic".into(),
                edition: "test".into(),
                map: "movement stamp".into(),
            },
            10000,
        );
        let stamp = p.prepare([32., 40.], Some(radius), "_", 0.).unwrap();
        p.commit(id(1), stamp);
        let rect = p.movement_stamp_rectangle(id(1)).unwrap();
        assert_eq!((rect.lower, rect.upper), (lower, upper));
        assert!(p.movement_stamp_rectangle(id(2)).is_err());
    }
}

#[cfg(feature = "test-fixtures")]
#[test]
fn reach_cell_excludes_only_requested_ground_stamps() {
    let mut p = placement(
        Source {
            install: "synthetic".into(),
            edition: "test".into(),
            map: "reach".into(),
        },
        10000,
    );
    let first = p.prepare([32., 32.], Some(2.), "_", 0.).unwrap();
    p.commit(id(1), first);
    let second = p.prepare([48., 32.], Some(2.), "_", 0.).unwrap();
    p.commit(id(2), second);
    assert_eq!(p.diagnostic_cell_excluding([32., 32.], &[]).unwrap().1, 1);
    assert_eq!(
        p.diagnostic_cell_excluding([32., 32.], &[id(1)]).unwrap().1,
        0
    );
    assert_eq!(
        p.diagnostic_cell_excluding([48., 32.], &[id(1)]).unwrap().1,
        1
    );
    let rectangle = p.diagnostic_unit_rectangle(id(1)).unwrap();
    assert!(rectangle[0][0] < rectangle[1][0] && rectangle[0][1] < rectangle[1][1]);
    assert!(p.diagnostic_unit_rectangle(id(3)).is_err());
}

#[test]
fn stationary_layers_swap_live_and_dead_footprints_without_erasing_terrain_or_each_other() {
    let mut base = Pathing {
        version: 0,
        width: 32,
        height: 32,
        cells: vec![0; 1024],
    };
    base.cells[100] = 0x01;
    base.cells[200] = 0x01;
    let mut p = placement(
        Source {
            install: "synthetic".into(),
            edition: "test".into(),
            map: "stationary".into(),
        },
        1000,
    )
    .with_stationary(
        base,
        vec![
            StationaryStamp {
                key: StationaryKey::Placed(1),
                live: vec![100, 10],
                dead: vec![200],
                mask: 0x04,
                active: true,
            },
            StationaryStamp {
                key: StationaryKey::Placed(2),
                live: vec![100],
                dead: vec![],
                mask: 0x08,
                active: true,
            },
            StationaryStamp {
                key: StationaryKey::Placed(3),
                live: vec![200],
                dead: vec![100],
                mask: 0x10,
                active: false,
            },
        ],
    )
    .unwrap();
    assert_eq!(p.grid.pathing.cells[100], 0x1d);
    assert_eq!(p.grid.pathing.cells[10], 0x04);
    assert_eq!(p.grid.pathing.cells[200], 0x01);
    assert!(p.set_placed_active(1, false));
    assert_eq!(p.grid.pathing.cells[100], 0x19);
    assert_eq!(p.grid.pathing.cells[10], 0x00);
    assert_eq!(p.grid.pathing.cells[200], 0x05);
    assert!(p.set_placed_active(2, false));
    assert_eq!(p.grid.pathing.cells[100], 0x11);
    assert!(p.set_placed_active(3, true));
    assert_eq!(p.grid.pathing.cells[100], 0x01);
    assert_eq!(p.grid.pathing.cells[200], 0x15);
    assert!(!p.set_placed_active(99, true));
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
fn blocked_goals_stop_searching_when_the_nearest_cell_is_proven() {
    for layout in 0..3 {
        let mut p = placement(
            Source {
                install: "synthetic".into(),
                edition: "test".into(),
                map: "blocked goal".into(),
            },
            10000,
        );
        let start = [16.0, 16.0];
        let target = [64.0, 64.0];
        let unit = p.prepare(start, Some(0.0), "_", 0.0).unwrap();
        p.commit(id(1), unit);
        let goal = p.cell(target).unwrap();
        let mut block = |dx, dy| {
            let at = p
                .index(i64::from(goal[0] + dx), i64::from(goal[1] + dy))
                .unwrap();
            p.grid.pathing.cells[at] |= p.rules.mask;
        };
        block(0, 0);
        if layout == 1 {
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                block(dx, dy);
            }
        }
        if layout == 2 {
            for dy in -2..=2 {
                for dx in -2..=2 {
                    block(dx, dy);
                }
            }
        }
        let before = p.occupied.clone();
        let limit = p.grid.pathing.cells.len();
        let mut small = limit;
        let path = p.route(id(1), start, target, 0.0, &mut small).unwrap();
        assert!(
            small > 0,
            "layout {layout}: nearest goal found but the search spent its whole budget"
        );
        let mut large = 10000;
        assert_eq!(
            p.route(id(1), start, target, 0.0, &mut large).unwrap(),
            path
        );
        assert_eq!(
            limit - small,
            10000 - large,
            "work must depend on the search, not its limit"
        );
        let last = p.cell(*path.last().unwrap()).unwrap();
        let delta = [last[0] - goal[0], last[1] - goal[1]].map(i32::abs);
        if layout == 1 {
            assert_eq!(delta, [1, 1]);
        } else if layout == 2 {
            assert_eq!(delta[0].max(delta[1]), 3);
            assert_eq!(delta[0].min(delta[1]), 0);
        } else {
            assert_eq!(delta[0] + delta[1], 1);
        }
        let mut from = start;
        for to in path {
            assert!(p.segment_clear(id(1), from, to, 0.0, &mut 1000).unwrap());
            from = to;
        }
        assert_eq!(p.occupied, before);
    }
}

#[test]
fn blocked_goal_with_an_isolated_nearest_pocket_has_bounded_work() {
    let mut p = placement(
        Source {
            install: "synthetic".into(),
            edition: "test".into(),
            map: "isolated pocket".into(),
        },
        10000,
    );
    let start = [16., 16.];
    let target = [64., 64.];
    let stamp = p.prepare(start, Some(0.), "_", 0.).unwrap();
    p.commit(id(1), stamp);
    for y in 14..=18 {
        for x in 14..=18 {
            p.grid.pathing.cells[y * 32 + x] = p.rules.mask;
        }
    }
    p.grid.pathing.cells[16 * 32 + 15] = 0;
    let before = p.occupied.clone();
    let mut work = 10000;
    let path = p.route(id(1), start, target, 0., &mut work).unwrap();
    eprintln!(
        "isolated-pocket work={} endpoint={:?}",
        10000 - work,
        path.last()
    );
    assert!(
        10000 - work < 1024,
        "isolated pocket caused a map-wide search"
    );
    assert_eq!(
        path.last(),
        Some(&[66., 54.]),
        "nearest reachable endpoint changed"
    );
    let last = p.cell(*path.last().unwrap()).unwrap();
    assert_eq!((last[0] - 16).abs().max((last[1] - 16).abs()), 3);
    let mut from = start;
    for to in path {
        assert!(p.segment_clear(id(1), from, to, 0., &mut 1000).unwrap());
        from = to;
    }
    assert_eq!(p.occupied, before);
}

#[test]
fn pocket_proof_preserves_seed_edges_and_diagonal_corner_blocking() {
    let mut p = placement(
        Source {
            install: "synthetic".into(),
            edition: "test".into(),
            map: "seed edges".into(),
        },
        10000,
    );
    let stamp = p.prepare([16., 16.], Some(0.), "_", 0.).unwrap();
    p.commit(id(1), stamp);
    p.grid.pathing.cells.fill(p.rules.mask);
    p.grid.pathing.cells[4 * 32 + 5] = 0;
    assert!(
        p.isolated_route_component(id(1), [5, 4], [4, 4], 0., &mut 1000)
            .unwrap()
            .is_none(),
        "a blocked start centre must still retain its seeded outgoing edge"
    );
    p.grid.pathing.cells[4 * 32 + 5] = p.rules.mask;
    p.grid.pathing.cells[5 * 32 + 5] = 0;
    assert_eq!(
        p.isolated_route_component(id(1), [5, 5], [4, 4], 0., &mut 1000)
            .unwrap(),
        Some(BTreeSet::from([[5, 5]])),
        "diagonal corners cannot connect the pocket"
    );
    assert!(
        p.isolated_route_component(id(1), [5, 5], [4, 4], 0., &mut 0)
            .unwrap()
            .is_none(),
        "an unfinished proof must not prune"
    );
}

#[test]
fn a_multi_cell_isolated_target_pocket_does_not_force_map_wide_search() {
    for (half_width, limit) in [(4, 1000), (7, 2048)] {
        let mut p = placement(
            Source {
                install: "synthetic".into(),
                edition: "test".into(),
                map: "multi-cell goal pocket".into(),
            },
            10000,
        );
        let start = [16., 16.];
        let target = [64., 64.];
        let stamp = p.prepare(start, Some(0.), "_", 0.).unwrap();
        p.commit(id(1), stamp);
        for y in 16 - half_width..=16 + half_width {
            for x in 16 - half_width..=16 + half_width {
                if x == 16 - half_width
                    || x == 16 + half_width
                    || y == 16 - half_width
                    || y == 16 + half_width
                    || (x == 16 && y == 16)
                {
                    p.grid.pathing.cells[y * 32 + x] = p.rules.mask;
                }
            }
        }
        let before = p.occupied.clone();
        let mut work = limit;
        let path = p.route(id(1), start, target, 0., &mut work).unwrap();
        assert!(
            work > 0,
            "pocket half-width {half_width} exhausted {limit} work"
        );
        let mut ample = 100000;
        assert_eq!(p.route(id(1), start, target, 0., &mut ample).unwrap(), path);
        assert_eq!(
            limit - work,
            100000 - ample,
            "work depends on the map, not the allowance"
        );
        let end = p.cell(*path.last().unwrap()).unwrap();
        let offset = end.map(|axis| (axis - 16).abs());
        assert_eq!(
            offset[0].max(offset[1]),
            i32::try_from(half_width + 1).unwrap(),
            "the endpoint must reach the nearest outside edge of the closed ring"
        );
        assert_eq!(offset[0].min(offset[1]), 0);
        let mut from = start;
        for to in path {
            assert!(p.segment_clear(id(1), from, to, 0., &mut 1000).unwrap());
            from = to;
        }
        assert_eq!(p.occupied, before);
    }
}

#[test]
fn script_creation_refuses_a_missing_base_before_resolving_or_loading_sources() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let loads = Arc::new(AtomicUsize::new(0));
    let hits = Arc::clone(&loads);
    let mut p = placement(
        Source {
            install: "synthetic".into(),
            edition: "test".into(),
            map: "missing base".into(),
        },
        100,
    )
    .with_footprint_loader(move |path| {
        hits.fetch_add(1, Ordering::SeqCst);
        Ok(Footprint {
            origin: AssetIdentity {
                source: "synthetic texture".into(),
                edition: "test".into(),
                version: "test".into(),
            },
            cells: vec![[0, 0]; if path == "live.tga" { 1 } else { 2 }],
        })
    })
    .with_script_paths(|_, dead| Ok(if dead { "dead.tga" } else { "live.tga" }.into()));
    let error = p
        .prepare_script_destructable(7, [32., 32.], 0., false)
        .unwrap_err();
    assert!(error.0.contains("base pathing is missing"), "{error}");
    assert_eq!(loads.load(Ordering::SeqCst), 0);
    assert!(p.stationary.is_empty());
    assert!(p.stationary_cells.is_empty());
}

#[test]
fn unit_source_commit_survives_a_refused_canonical_alias_script_layer() {
    let (loader, ledger) = TrackingLoader::new();
    let mut p = placement(
        Source {
            install: "synthetic".into(),
            edition: "test".into(),
            map: "committed unit source".into(),
        },
        100,
    )
    .with_stationary(script_base(), Vec::new())
    .unwrap()
    .with_staged_footprint_loader(loader)
    .with_script_paths(|_, dead| Ok(if dead { "foo.tga" } else { "FOO.TGA" }.into()));
    p.prepare([32., 32.], None, "Foo.tga", 0.).unwrap();
    {
        let ledger = ledger.lock().unwrap();
        assert_eq!(ledger.loads, 1);
        assert!(ledger.staged.is_empty());
        assert_eq!(
            ledger.committed.iter().cloned().collect::<Vec<_>>(),
            ["foo.tga"]
        );
    }
    assert_eq!(p.footprints.len(), 1);
    assert!(p.footprints.contains_key("foo.tga"));

    p.stationary_cells_used = p.budget.stationary_cells;
    let error = p
        .prepare_script_destructable(7, [32., 32.], 0., false)
        .unwrap_err();
    assert!(error.0.contains("stationary cell budget"), "{error}");
    {
        let ledger = ledger.lock().unwrap();
        assert_eq!(ledger.loads, 1);
        assert!(ledger.staged.is_empty());
        assert!(ledger.committed.contains("foo.tga"));
    }
    assert_eq!(p.footprints.len(), 1);
    p.prepare([40., 32.], None, "Foo.tga", 0.).unwrap();
    assert_eq!(ledger.lock().unwrap().loads, 1);
}

#[test]
fn script_layer_case_and_slash_aliases_share_one_canonical_source() {
    let (loader, ledger) = TrackingLoader::new();
    let mut p = placement(
        Source {
            install: "synthetic".into(),
            edition: "test".into(),
            map: "canonical script aliases".into(),
        },
        100,
    )
    .with_stationary(script_base(), Vec::new())
    .unwrap()
    .with_staged_footprint_loader(loader)
    .with_script_paths(|_, dead| {
        Ok(if dead {
            "Textures\\Live.TGA"
        } else {
            "textures/live.tga"
        }
        .into())
    });
    p.prepare_script_destructable(7, [32., 32.], 0., false)
        .unwrap();
    let ledger = ledger.lock().unwrap();
    assert_eq!(ledger.loads, 1);
    assert!(ledger.staged.is_empty());
    assert_eq!(
        ledger.committed.iter().cloned().collect::<Vec<_>>(),
        ["textures/live.tga"]
    );
    drop(ledger);
    assert_eq!(p.footprints.len(), 1);
    assert!(p.footprints.contains_key("textures/live.tga"));
}

#[test]
fn initial_footprint_map_is_canonical_and_refuses_alias_duplicates() {
    let grid = || Grid {
        source: Source {
            install: "synthetic".into(),
            edition: "test".into(),
            map: "canonical initial footprints".into(),
        },
        origin: [0.0; 2],
        pathing: script_base(),
        domains: vec![5; 1024],
    };
    let budget = || Budget {
        cells: 1024,
        footprint_cells: 64,
        stationary_cells: 1 << 20,
        stationary_work: 1 << 20,
        queries: 100,
    };
    let footprint = || Footprint {
        origin: AssetIdentity {
            source: "synthetic source".into(),
            edition: "test".into(),
            version: "test".into(),
        },
        cells: vec![[0, 0]],
    };
    let p = Placement::new(
        grid(),
        rules(2, -8192.25),
        BTreeMap::from([("Textures\\Live.TGA".into(), footprint())]),
        budget(),
    )
    .unwrap();
    assert_eq!(p.footprints.len(), 1);
    assert!(p.footprints.contains_key("textures/live.tga"));
    let error = Placement::new(
        grid(),
        rules(2, -8192.25),
        BTreeMap::from([
            ("Textures\\Live.TGA".into(), footprint()),
            ("textures/live.tga".into(), footprint()),
        ]),
        budget(),
    )
    .err()
    .unwrap();
    assert!(
        error.0.contains("duplicate footprint path alias"),
        "{error}"
    );
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
    let fly_mask = put(0x3a0, &[0xb9, 4, 0, 0, 0]);
    let float_mask = put(0x3b0, &[0xb9, 16, 0, 0, 0]);
    let amphibious_mask = put(0x3c0, &[0xb9, 32, 0, 0, 0]);
    let terrain_xy = put(
        0x3d0,
        &[&[0xf3, 0x0f, 0x59, 0x05][..], &address(0x520).to_le_bytes()].concat(),
    );
    let terrain_height = put(
        0x3e0,
        &[
            &[0xf3, 0x0f, 0x58, 0x05][..],
            &address(0x524).to_le_bytes(),
            &[0xc1, 0xe6, 0x0e, 0xf3, 0x0f, 0x59, 0x05],
            &address(0x528).to_le_bytes(),
        ]
        .concat(),
    );
    let mut channels = [0; 48];
    channels[0..4].copy_from_slice(&[0xf6, 0x41, 0x02, 0x02]);
    channels[9..13].copy_from_slice(&[0x80, 0x0c, 0x03, 0x40]);
    channels[16..20].copy_from_slice(&[0x80, 0x79, 0x01, 0x00]);
    channels[25..29].copy_from_slice(&[0x80, 0x0c, 0x03, 0x04]);
    channels[36..39].copy_from_slice(&[0xf6, 0x01, 0x08]);
    channels[44..48].copy_from_slice(&[0x80, 0x0c, 0x01, 0x10]);
    let texture_channels = put(0x420, &channels);
    let texture_build_mask = put(0x470, &[0xf6, 0x45, 0x18, 0x50]);
    for (offset, value) in [
        (0x520, 20_f32),
        (0x524, 123_f32),
        (0x528, 8_f32),
        (0x500, initial),
        (0x504, 0.5_f32),
        (0x508, 1.5),
    ] {
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
        fly_mask,
        float_mask,
        amphibious_mask,
        terrain_xy,
        terrain_height,
        texture_channels,
        texture_build_mask,
    };
    (bytes, binding, budget)
}

#[test]
fn texture_channel_operands_control_construction_flags() {
    let policy = rules(2, 0.0).texture_policy();
    assert_eq!(policy.flags([2, 7, 8, 0]), 0x54);
    assert_eq!(policy.flags([1, 0, 4, 255]), 0);
    assert!(policy.checks_construction([2, 0, 0, 0]));
    assert!(policy.checks_construction([0, 0, 8, 0]));
    assert!(!policy.checks_construction([0, 1, 0, 255]));
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
            stationary_cells: 1 << 20,
            stationary_work: 1 << 20,
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
    for mutation in 0..12 {
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
            9 => {
                bytes[0x3a0] = 0xb8;
                binding.fly_mask.digest = sha256(&bytes[0x3a0..0x3a5]);
                binding.image_sha256 = sha256(&bytes);
            }
            10 => {
                bytes[0x430] = 0x90;
                binding.texture_channels.digest = sha256(&bytes[0x420..0x450]);
                binding.image_sha256 = sha256(&bytes);
            }
            11 => {
                bytes[0x473] = 0;
                binding.texture_build_mask.digest = sha256(&bytes[0x470..0x474]);
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

#[test]
fn attack_region_routes_around_a_blocked_contact_point_without_target_occupancy() {
    let mut p = placement(
        Source {
            install: "synthetic".into(),
            edition: "test".into(),
            map: "attack-region".into(),
        },
        10000,
    );
    for (unit, point) in [
        (id(1), [16.0, 16.0]),
        (id(2), [112.0, 16.0]),
        (id(3), [44.0, 16.0]),
    ] {
        let prepared = p.prepare(point, Some(2.0), "_", 0.0).unwrap();
        p.commit(unit, prepared);
    }
    let before = p.occupied.clone();
    let approach = approach_point([16.0, 16.0], [112.0, 16.0], 68.0);
    assert!(
        p.prepare_motion(id(1), approach, 2.0).unwrap().is_none(),
        "the straight contact point is blocked"
    );
    let mut work = 2000;
    let route = p
        .route_within(id(1), [16.0, 16.0], [112.0, 16.0], 2.0, 68.0, &mut work)
        .unwrap();
    let end = *route.last().unwrap();
    assert_ne!(end, [112.0, 16.0]);
    assert!((end[0] - 112.0).hypot(end[1] - 16.0) <= 68.0, "{route:?}");
    assert!(
        route.iter().any(|p| p[1] != 16.0),
        "the search finds another approach angle"
    );
    let mut previous = [16.0, 16.0];
    for point in route {
        assert!(
            p.segment_clear(id(1), previous, point, 2.0, &mut 1000)
                .unwrap()
        );
        previous = point;
    }
    assert_eq!(p.occupied, before);
    assert!(
        p.route_within(id(1), [16.0, 16.0], [112.0, 16.0], 2.0, f32::NAN, &mut 2000)
            .is_err()
    );
}

#[test]
fn fly_placement_ignores_ground_state_but_respects_the_fly_mask() {
    let mut p = placement(
        Source {
            install: "synthetic".into(),
            edition: "test".into(),
            map: "fly".into(),
        },
        10000,
    );
    let air = p
        .prepare_class([32., 32.], Some(2.), "_", 0., Movement::Air)
        .unwrap();
    p.commit(id(1), air);
    let cell = 8 * 32 + 8;
    assert_eq!(p.occupied[cell], 0);
    assert_eq!(p.footprint_occupied[cell], 0);
    let ground = p.prepare([32., 32.], Some(2.), "_", 0.).unwrap();
    p.commit(id(2), ground);
    p.grid.pathing.cells[cell] |= p.rules.ground_mask();
    p.grid.domains[cell] = 9;
    assert!(
        p.prepare_motion(id(1), [32., 32.], 2.).unwrap().is_some(),
        "air ignores the ground mask, ground occupancy and terrain domains"
    );
    p.grid.pathing.cells[cell] |= p.rules.fly_mask();
    assert!(
        p.prepare_motion(id(1), [32., 32.], 2.).unwrap().is_none(),
        "the selected fly mask still refuses air motion"
    );
}

#[test]
fn fly_creation_refuses_a_map_with_no_unmasked_cell() {
    let mut p = placement(
        Source {
            install: "synthetic".into(),
            edition: "test".into(),
            map: "fly-wall".into(),
        },
        10000,
    );
    let fly = p.rules.fly_mask();
    for cell in &mut p.grid.pathing.cells {
        *cell |= fly;
    }
    assert!(
        p.prepare_class([32., 32.], Some(2.), "_", 0., Movement::Air)
            .unwrap_err()
            .0
            .contains("no qualified placement")
    );
}

#[test]
fn water_classes_retain_shoreline_masks_and_separate_collision_layers() {
    let mut p = placement(
        Source {
            install: "synthetic".into(),
            edition: "test".into(),
            map: "shore".into(),
        },
        10000,
    );
    let ground = p.rules.ground_mask();
    let water = p.rules.float_mask();
    let amphibious = p.rules.amphibious_mask();
    for y in 0..32 {
        for x in 0..32 {
            p.grid.pathing.cells[y * 32 + x] = if x < 16 { water } else { ground };
        }
    }
    let boat = p
        .prepare_class([80., 32.], Some(2.), "_", 0., Movement::Float)
        .unwrap();
    p.commit(id(1), boat);
    assert!(p.prepare_motion(id(1), [24., 32.], 2.).unwrap().is_none());
    assert!(p.prepare_motion(id(1), [88., 32.], 2.).unwrap().is_some());
    let amphibian = p
        .prepare_class([24., 32.], Some(2.), "_", 0., Movement::Amphibious)
        .unwrap();
    p.commit(id(2), amphibian);
    let route = p
        .route_within(id(2), [24., 32.], [100., 32.], 2., 0., &mut 10000)
        .unwrap();
    assert_eq!(route.last(), Some(&[100., 32.]));
    assert!(
        route
            .iter()
            .all(|at| p.prepare_motion(id(2), *at, 2.).unwrap().is_some())
    );
    assert!(
        p.prepare_motion(id(2), [80., 32.], 2.).unwrap().is_none(),
        "amphibian collides with boat"
    );
    let cell = 8 * 32 + 25;
    p.grid.pathing.cells[cell] = ground | water;
    assert!(p.prepare_motion(id(2), [100., 32.], 2.).unwrap().is_none());
    p.grid.pathing.cells[cell] = amphibious;
    assert!(p.prepare_motion(id(2), [100., 32.], 2.).unwrap().is_none());
    p.grid.pathing.cells[cell] = 0;
    p.hide(id(1)).unwrap();
    assert!(p.prepare_motion(id(2), [80., 32.], 2.).unwrap().is_some());
    let shown = p
        .prepare_show_class(id(1), [80., 32.], Some(2.), "_", 0., Movement::Float)
        .unwrap();
    p.commit(id(1), shown);
    assert!(p.prepare_motion(id(2), [80., 32.], 2.).unwrap().is_none());
    // A bridge may host a land and water mover at the same XY without cross-layer collision.
    for y in 7..=9 {
        for x in 19..=21 {
            p.grid.pathing.cells[y * 32 + x] = 0;
        }
    }
    let walker = p.prepare([80., 32.], Some(2.), "_", 0.).unwrap();
    assert_eq!(walker.position, [80., 32.]);
    p.commit(id(3), walker);
    assert_eq!(p.occupied[8 * 32 + 20], 1);
    assert_eq!(p.water_occupied[8 * 32 + 20], 1);
    let moved = p
        .prepare_relocation_class(id(1), [92., 32.], Some(2.), "_", 0., Movement::Float)
        .unwrap();
    p.commit(id(1), moved);
    assert_eq!(p.water_occupied[8 * 32 + 20], 0);
    assert_eq!(p.occupied[8 * 32 + 20], 1);
}

#[test]
fn movement_and_surface_operands_are_nonstock_and_malformed_inputs_refuse() {
    let (mut bytes, mut binding, budget) = fixture(2, -8192.25);
    let decoded = Rules::read(&bytes, binding.clone(), budget).unwrap();
    assert_eq!(
        [
            decoded.ground_mask(),
            decoded.fly_mask(),
            decoded.float_mask(),
            decoded.amphibious_mask()
        ],
        [8, 4, 16, 32]
    );
    assert_eq!(decoded.terrain_encoding(), [20., 123., 8.]);
    for value in [0, 3, 4, 8, 32] {
        bytes[0x3b1..0x3b5].copy_from_slice(&u32::to_le_bytes(value));
        binding.float_mask.digest = sha256(&bytes[0x3b0..0x3b5]);
        binding.image_sha256 = sha256(&bytes);
        assert!(Rules::read(&bytes, binding.clone(), budget).is_err());
    }
    for value in [0., -1., f32::NAN, f32::INFINITY] {
        let (mut bytes, mut binding, budget) = fixture(2, -8192.25);
        bytes[0x528..0x52c].copy_from_slice(&f32::to_le_bytes(value));
        binding.image_sha256 = sha256(&bytes);
        assert!(Rules::read(&bytes, binding, budget).is_err());
    }
}

#[test]
fn construction_uses_full_authored_footprint_without_relocating_blocked_cells() {
    let source = Source {
        install: "synthetic".into(),
        edition: "test".into(),
        map: "build".into(),
    };
    let mut p = placement(source, 100);
    p.footprints.insert(
        "build.tga".into(),
        Footprint {
            origin: AssetIdentity {
                edition: "synthetic".into(),
                version: "test".into(),
                source: "build.tga".into(),
            },
            cells: vec![[-1, -1], [0, -1], [-1, 0], [0, 0]],
        },
    );
    assert_eq!(
        p.construction_position(None, [33., 35.], 0., "build.tga", 8, 0)
            .unwrap(),
        Some([32., 32.])
    );
    let cell = 7 * 32 + 7;
    p.grid.pathing.cells[cell] = 8;
    assert_eq!(
        p.construction_position(None, [33., 35.], 0., "build.tga", 8, 0)
            .unwrap(),
        None
    );
    p.grid.pathing.cells[cell] = 0;
    p.footprint_occupied[cell] = 1;
    assert_eq!(
        p.construction_position(None, [33., 35.], 0., "build.tga", 8, 0)
            .unwrap(),
        None
    );
    assert_eq!(
        p.construction_position(None, [0., 0.], 0., "build.tga", 8, 0)
            .unwrap(),
        None
    );
    assert!(
        p.construction_position(None, [f32::NAN, 0.], 0., "build.tga", 8, 0)
            .is_err()
    );
}

#[cfg(feature = "test-fixtures")]
#[test]
fn diagnostic_exclusions_restore_occupancy_after_success_and_error() {
    let mut p = placement(
        Source {
            install: "synthetic".into(),
            edition: "test".into(),
            map: "exclusions".into(),
        },
        100,
    );
    p.footprints.insert(
        "build.tga".into(),
        Footprint {
            origin: AssetIdentity {
                edition: "synthetic".into(),
                version: "test".into(),
                source: "build.tga".into(),
            },
            cells: vec![[0, 0]],
        },
    );
    let stamp = p.prepare([32., 32.], Some(2.), "_", 0.).unwrap();
    p.commit(id(3), stamp);
    let before = p.occupied.clone();
    assert!(
        p.diagnostic_construction_position([32., 32.], 0., "build.tga", 8, 0)
            .unwrap()
            .is_none()
    );
    assert!(
        p.diagnostic_construction_position_excluding([32., 32.], 0., "build.tga", 8, 0, &[id(3)])
            .unwrap()
            .is_some()
    );
    assert_eq!(p.occupied, before);
    assert!(
        p.diagnostic_construction_position_excluding(
            [f32::NAN, 32.],
            0.,
            "build.tga",
            8,
            0,
            &[id(3)]
        )
        .is_err()
    );
    assert_eq!(p.occupied, before);
    assert!(
        p.diagnostic_construction_position_excluding([32., 32.], 0., "build.tga", 8, 0, &[id(4)])
            .is_err()
    );
    let first = &p.stamps[0].1;
    let overlapping = Prepared {
        position: first.position,
        cells: first.cells.clone(),
        footprint: vec![],
        face: first.face,
        active: true,
        movement: first.movement,
    };
    p.commit(id(4), overlapping);
    let before = p.occupied.clone();
    assert!(
        p.diagnostic_construction_position_excluding(
            [32., 32.],
            0.,
            "build.tga",
            8,
            0,
            &[id(3), id(3)]
        )
        .unwrap()
        .is_none()
    );
    assert!(
        p.diagnostic_construction_position_excluding(
            [32., 32.],
            0.,
            "build.tga",
            8,
            0,
            &[id(3), id(4)]
        )
        .unwrap()
        .is_some()
    );
    assert_eq!(p.occupied, before);
    p.hide(id(4)).unwrap();
    assert!(
        p.diagnostic_construction_position_excluding(
            [32., 32.],
            0.,
            "build.tga",
            8,
            0,
            &[id(3), id(4)]
        )
        .unwrap()
        .is_some()
    );
    let cell = 8 * 32 + 8;
    p.footprint_occupied[cell] = 1;
    assert!(
        p.diagnostic_construction_position_excluding(
            [32., 32.],
            0.,
            "build.tga",
            8,
            0,
            &[id(3), id(4)]
        )
        .unwrap()
        .is_none()
    );
}

#[test]
fn construction_uses_its_own_rotated_footprint() {
    let mut p = placement(
        Source {
            install: "fixture".into(),
            edition: "test".into(),
            map: "map".into(),
        },
        100,
    );
    let origin = AssetIdentity {
        source: "fixture texture".into(),
        edition: "test".into(),
        version: "test".into(),
    };
    p.footprints.insert(
        "build.tga".into(),
        Footprint {
            origin: origin.clone(),
            cells: vec![[0, 0]],
        },
    );
    p.building_footprints.insert(
        "build.tga".into(),
        Footprint {
            origin,
            cells: vec![[-2, 0]],
        },
    );
    p.footprint_occupied[8 * 32 + 6] = 1;
    assert_eq!(
        p.construction_position(None, [32., 32.], 0., "build.tga", 8, 0)
            .unwrap(),
        None
    );
    assert_eq!(
        p.construction_position(None, [32., 32.], 90., "build.tga", 8, 0)
            .unwrap(),
        Some([32., 32.])
    );
    p.footprint_occupied[6 * 32 + 7] = 1;
    assert_eq!(
        p.construction_position(None, [32., 32.], 90., "build.tga", 8, 0)
            .unwrap(),
        None
    );
}

#[test]
fn bounded_route_outcome_preserves_paths_and_distinguishes_search_exhaustion() {
    let mut p = placement(
        Source {
            install: "synthetic".into(),
            edition: "test".into(),
            map: "route termination".into(),
        },
        10000,
    );
    let start = [16., 16.];
    let target = [112., 16.];
    let stamp = p.prepare(start, Some(0.), "_", 0.).unwrap();
    p.commit(id(1), stamp);
    for y in 0..27 {
        p.grid.pathing.cells[y * 32 + 16] |= p.rules.mask;
    }
    let mut small = 256;
    let partial = p
        .route_outcome(id(1), start, target, 0., 0., &mut small)
        .unwrap();
    assert!(
        matches!(partial, RouteOutcome::PartialBudget(_)),
        "{partial:?}"
    );
    assert_eq!(small, 0);
    for _ in 0..3 {
        assert_eq!(
            p.route_outcome(id(1), start, target, 0., 0., &mut 256)
                .unwrap(),
            partial,
            "restarting a bounded search makes no accumulated search progress"
        );
    }
    let RouteOutcome::PartialBudget(path) = partial else {
        unreachable!()
    };
    let mut compatible_work = 256;
    assert_eq!(
        p.route_within(id(1), start, target, 0., 0., &mut compatible_work)
            .unwrap(),
        path
    );
    assert_eq!(compatible_work, 0);
    let reached = p
        .route_outcome(id(1), start, target, 0., 0., &mut 100000)
        .unwrap();
    assert!(matches!(reached, RouteOutcome::Reached(_)), "{reached:?}");
    for y in 27..32 {
        p.grid.pathing.cells[y * 32 + 16] |= p.rules.mask;
    }
    let blocked = p
        .route_outcome(id(1), start, target, 0., 0., &mut 100000)
        .unwrap();
    assert!(matches!(blocked, RouteOutcome::Blocked(_)), "{blocked:?}");
}

#[test]
fn optional_streaming_probe_charges_near_obstruction_without_complete_segment_budget() {
    let mut p = placement(
        Source {
            install: "synthetic".into(),
            edition: "test".into(),
            map: "streamed route probe".into(),
        },
        10000,
    );
    let start = [16., 16.];
    let target = [112., 16.];
    let stamp = p.prepare(start, Some(0.), "_", 0.).unwrap();
    p.commit(id(1), stamp);
    p.grid.pathing.cells[4 * 32 + 4] |= p.rules.mask;
    let mut mandatory = 1;
    assert!(
        p.segment_clear(id(1), start, target, 0., &mut mandatory)
            .unwrap_err()
            .work_exhausted()
    );
    assert_eq!(
        mandatory, 1,
        "mandatory preflight retains its existing contract"
    );
    let mut optional = 1;
    assert!(
        !p.segment_clear_streaming(id(1), start, target, 0., &mut optional)
            .unwrap()
    );
    assert_eq!(
        optional, 0,
        "the first blocked sample is actually checked and charged"
    );
    for blocked in [true, false] {
        if !blocked {
            p.grid.pathing.cells[4 * 32 + 4] &= !p.rules.mask;
        }
        let mut complete = 1000;
        let mut streaming = 1000;
        assert_eq!(
            p.clear_fraction(id(1), start, target, 0., &mut complete)
                .unwrap(),
            p.clear_fraction_checked(id(1), start, target, 0., &mut streaming, false)
                .unwrap(),
        );
        assert_eq!(complete, streaming, "identical samples cost identical work");
    }
}

#[test]
fn a_standing_footprint_keeps_its_creation_facing_across_hide_and_reveal() {
    let mut p = placement(
        Source {
            install: "synthetic".into(),
            edition: "test".into(),
            map: "footprint facing".into(),
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
            cells: vec![[0, 0], [1, 0]],
        })
    });
    // A new footprint at a non-quarter facing is still refused.
    assert!(
        p.prepare([32., 32.], None, "fixture.tga", 41.)
            .unwrap_err()
            .0
            .contains("quarter-turn")
    );
    let a = p.prepare([32., 32.], None, "fixture.tga", 270.).unwrap();
    let cells = a.footprint.clone();
    p.commit(id(1), a);
    p.hide(id(1)).unwrap();
    // The model has turned to 41 degrees meanwhile; the footprint does not rotate with it.
    let shown = p
        .prepare_show(id(1), [32., 32.], None, "fixture.tga", 41.)
        .unwrap();
    assert_eq!(shown.footprint, cells);
}

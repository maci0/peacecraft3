//! Actual map import consumer checks; no original asset bytes or native policy.
#[path = "world_grid/fixtures.rs"]
mod fixtures;
use fixtures::*;
use pcx_map::{GridAlignmentError, GridDimensions, Map};
use pcx_sim::world::{ImportError, ImportedWorld, RecordProblem};

#[test]
fn missing_terrain_is_an_exact_member_error_before_catalog_resolution() {
    let bytes = map_bytes(None, &pathing_bytes(4, 6));
    let map = Map::parse(&bytes).unwrap();
    let catalog = catalog();
    let error = ImportedWorld::from_map_aligned(&map, source(), &catalog, &[]).unwrap_err();
    assert!(
        matches!(error, ImportError::Member { map, member: "war3map.w3e", error: pcx_map::Error::Extraction(pcx_mpq::Error::NotFound(_)) } if map == source().map)
    );
    assert_eq!(catalog.1.get(), 0);
}

#[test]
fn malformed_terrain_retains_decoder_error_and_member_context() {
    let bytes = map_bytes(Some(b"bad!"), &pathing_bytes(4, 6));
    let map = Map::parse(&bytes).unwrap();
    let catalog = catalog();
    let error = ImportedWorld::from_map_aligned(&map, source(), &catalog, &[]).unwrap_err();
    assert!(
        matches!(error, ImportError::Member { map, member: "war3map.w3e", error: pcx_map::Error::BadTerrainMagic(_) } if map == source().map)
    );
    assert_eq!(catalog.1.get(), 0);
}

#[test]
fn invalid_provenance_is_checked_before_reading_missing_terrain() {
    let bytes = map_bytes(None, &pathing_bytes(4, 6));
    let map = Map::parse(&bytes).unwrap();
    let catalog = catalog();
    for missing in [0, 1, 2] {
        let mut source = source();
        match missing {
            0 => source.install.clear(),
            1 => source.edition.clear(),
            _ => source.map.clear(),
        }
        assert!(matches!(
            ImportedWorld::from_map_aligned(&map, source, &catalog, &[]),
            Err(ImportError::MissingSource)
        ));
    }
    assert_eq!(catalog.1.get(), 0);
}

#[test]
fn mismatch_errors_keep_all_raw_dimensions_and_supplied_source() {
    for (pw, ph, expected) in [
        (
            5,
            6,
            GridAlignmentError::NonDivisibleWidth {
                terrain: 2,
                pathing: 5,
            },
        ),
        (
            4,
            7,
            GridAlignmentError::NonDivisibleHeight {
                terrain: 3,
                pathing: 7,
            },
        ),
        (
            4,
            9,
            GridAlignmentError::ScaleMismatch {
                width_scale: 2,
                height_scale: 3,
            },
        ),
    ] {
        let bytes = map_bytes(Some(&terrain_bytes(2, 3)), &pathing_bytes(pw, ph));
        let map = Map::parse(&bytes).unwrap();
        let catalog = catalog();
        let error = ImportedWorld::from_map_aligned(&map, source(), &catalog, &[]).unwrap_err();
        match error {
            ImportError::GridAlignment {
                source_identity,
                terrain,
                pathing,
                error,
            } => {
                assert_eq!(source_identity, source());
                assert_eq!(
                    terrain,
                    GridDimensions {
                        width: 2,
                        height: 3
                    }
                );
                assert_eq!(
                    pathing,
                    GridDimensions {
                        width: pw,
                        height: ph
                    }
                );
                assert_eq!(error, expected);
            }
            other => panic!("unexpected error: {other}"),
        }
        assert_eq!(catalog.1.get(), 0);
    }
}

#[test]
fn same_map_join_derives_different_ratios_and_preserves_every_raw_record() {
    for scale in [1, 3, 5] {
        let bytes = map_bytes(
            Some(&terrain_bytes(2, 3)),
            &pathing_bytes(2 * scale, 3 * scale),
        );
        let map = Map::parse(&bytes).unwrap();
        let catalog = catalog();
        let (world, alignment) =
            ImportedWorld::from_map_aligned(&map, source(), &catalog, &[*b"STAT"]).unwrap();
        let raw = ImportedWorld::from_map(&map, source(), &catalog, &[*b"STAT"]).unwrap();
        assert_eq!(world, raw);
        assert_eq!(world.source, source());
        assert_eq!(alignment.uniform_scale, scale);
        assert_eq!(
            alignment.terrain_corner_count,
            map.terrain().unwrap().corner_count() as u64
        );
        assert_eq!(
            alignment.pathing_cell_count,
            world.pathing.cells.len() as u64
        );
        let authored = map.units().unwrap();
        for (ordinal, record) in world.records.iter().enumerate() {
            assert_eq!(record.ordinal, ordinal);
            assert_eq!(record.placement, authored.instances[ordinal]);
        }
        assert_eq!(world.records[0].owner, Some(-2));
        assert_eq!(world.records[0].placement.creation_number, 42);
        assert_eq!(
            world.records[1].definition,
            Err(pcx_sim::world::ResolutionProblem::MissingRawcode(
                "MISS".into()
            ))
        );
        assert!(matches!(
            world.records[1].problems[0],
            RecordProblem::Definition(_)
        ));
    }
}

#[test]
fn opted_route_keeps_required_other_member_refusals() {
    let terrain = terrain_bytes(2, 3);
    let pathing = pathing_bytes(4, 6);
    let bytes = build_synthetic_map(
        "SyntheticMissingUnits",
        &[
            ("war3map.w3e", &terrain),
            ("war3map.wpm", &pathing),
            ("war3map.w3i", &info().canonical_bytes()),
        ],
    );
    let map = Map::parse(&bytes).unwrap();
    let catalog = catalog();
    assert!(matches!(
        ImportedWorld::from_map_aligned(&map, source(), &catalog, &[]),
        Err(ImportError::Member {
            member: "war3mapUnits.doo",
            ..
        })
    ));
    assert_eq!(catalog.1.get(), 0);
}

#[test]
fn raw_from_map_and_from_authored_keep_their_explicit_unaligned_contract() {
    let bytes = map_bytes(None, &pathing_bytes(4, 6));
    let map = Map::parse(&bytes).unwrap();
    let catalog = catalog();
    let raw = ImportedWorld::from_map(&map, source(), &catalog, &[]).unwrap();
    let authored = ImportedWorld::from_authored(
        source(),
        map.info().unwrap(),
        map.pathing().unwrap(),
        &map.units().unwrap(),
        &catalog,
        &[],
    )
    .unwrap();
    assert_eq!(raw, authored);
}

#[test]
fn malformed_pathing_and_zero_dimensions_refuse_before_catalog() {
    let terrain = terrain_bytes(2, 3);
    let mut pathing = pathing_bytes(4, 6);
    pathing.pop();
    let bytes = map_bytes(Some(&terrain), &pathing);
    let map = Map::parse(&bytes).unwrap();
    let catalog = catalog();
    assert!(matches!(
        ImportedWorld::from_map_aligned(&map, source(), &catalog, &[]),
        Err(ImportError::Member {
            member: "war3map.wpm",
            error: pcx_map::Error::Pathing(pcx_map::pathing::PathingError::Size {
                width: 4,
                height: 6,
                bytes: 23
            }),
            ..
        })
    ));
    for (tw, th, pw, ph, expected) in [
        (0, 3, 4, 6, GridAlignmentError::ZeroTerrainWidth),
        (2, 0, 4, 6, GridAlignmentError::ZeroTerrainHeight),
        (2, 3, 0, 6, GridAlignmentError::ZeroPathingWidth),
        (2, 3, 4, 0, GridAlignmentError::ZeroPathingHeight),
    ] {
        let bytes = map_bytes(Some(&terrain_bytes(tw, th)), &pathing_bytes(pw, ph));
        let map = Map::parse(&bytes).unwrap();
        assert!(
            matches!(ImportedWorld::from_map_aligned(&map, source(), &catalog, &[]), Err(ImportError::GridAlignment { error, .. }) if error == expected)
        );
    }
    assert_eq!(catalog.1.get(), 0);
}

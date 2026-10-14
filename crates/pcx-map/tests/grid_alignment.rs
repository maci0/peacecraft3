//! Synthetic dimensions exercise the public API; original fixtures are opt-in.
use pcx_map::grid_alignment::terrain_pathing_alignment;
use pcx_map::{
    GridAlignment, GridAlignmentError, GridDimensions, PathingGridDimensions, TerrainGridDimensions,
};

#[test]
fn alignment_uses_dimensions_from_synthetic_w3e_and_wpm_records() {
    let (width, height, scale) = (3u32, 5u32, 3u32);
    let mut terrain = pcx_map::TERRAIN_MAGIC.to_vec();
    terrain.extend(pcx_map::TERRAIN_VERSION.to_le_bytes());
    terrain.push(0); // Synthetic default slot, with empty slot tables.
    for word in [0, 0, 0, width + 1, height + 1] {
        terrain.extend(word.to_le_bytes());
    }
    terrain.extend([0; 8]); // Synthetic origin floats.
    terrain.resize(
        terrain.len() + ((width + 1) * (height + 1)) as usize * pcx_map::CORNER_RECORD,
        0,
    );
    let terrain = pcx_map::Terrain::from_bytes(terrain).unwrap();
    let mut pathing = b"MP3W".to_vec();
    for word in [0, width * scale, height * scale] {
        pathing.extend(word.to_le_bytes());
    }
    pathing.resize(pathing.len() + (width * height * scale * scale) as usize, 0);
    let pathing = pcx_map::Pathing::parse(&pathing).unwrap();
    let aligned = terrain_pathing_alignment(
        terrain.width(),
        terrain.height(),
        pathing.width,
        pathing.height,
    )
    .unwrap();
    assert_eq!(aligned.terrain.tiles, GridDimensions { width, height });
    assert_eq!(aligned.terrain_corner_count, terrain.corner_count() as u64);
    assert_eq!(aligned.pathing_cell_count, pathing.cells.len() as u64);
    assert_eq!(aligned.uniform_scale, scale);
}

#[test]
fn authored_dimensions_and_products_are_preserved_at_distinct_scales() {
    // Synthetic non-square grids, including a product wider than u32.
    for (width, height, scale) in [(3, 5, 1), (7, 2, 3), (2, 11, 5), (70_000, 80_000, 2)] {
        let aligned =
            GridAlignment::from_dimensions(width, height, width * scale, height * scale).unwrap();
        let terrain: TerrainGridDimensions = aligned.terrain;
        let pathing: PathingGridDimensions = aligned.pathing;
        assert_eq!(terrain.tiles, GridDimensions { width, height });
        assert_eq!(
            terrain.corners,
            GridDimensions {
                width: width + 1,
                height: height + 1
            }
        );
        assert_eq!(
            pathing.raster,
            GridDimensions {
                width: width * scale,
                height: height * scale
            }
        );
        assert_eq!(
            (
                aligned.width_scale,
                aligned.height_scale,
                aligned.uniform_scale
            ),
            (scale, scale, scale)
        );
        assert_eq!(
            aligned.terrain_tile_count,
            u64::from(width) * u64::from(height)
        );
        assert_eq!(
            aligned.terrain_corner_count,
            u64::from(width + 1) * u64::from(height + 1)
        );
        assert_eq!(
            aligned.pathing_cell_count,
            u64::from(width * scale) * u64::from(height * scale)
        );
        assert_eq!(
            terrain_pathing_alignment(width, height, width * scale, height * scale),
            Ok(aligned)
        );
    }
}

#[test]
fn every_refusal_is_public_and_retains_its_dimensions() {
    use GridAlignmentError::*;
    for (dimensions, expected) in [
        ((0, 1, 1, 1), ZeroTerrainWidth),
        ((1, 0, 1, 1), ZeroTerrainHeight),
        ((1, 1, 0, 1), ZeroPathingWidth),
        ((1, 1, 1, 0), ZeroPathingHeight),
        (
            (3, 5, 8, 15),
            NonDivisibleWidth {
                terrain: 3,
                pathing: 8,
            },
        ),
        (
            (3, 5, 9, 14),
            NonDivisibleHeight {
                terrain: 5,
                pathing: 14,
            },
        ),
        (
            (3, 5, 9, 10),
            ScaleMismatch {
                width_scale: 3,
                height_scale: 2,
            },
        ),
        (
            (3, 5, 6, 15),
            ScaleMismatch {
                width_scale: 2,
                height_scale: 3,
            },
        ),
        ((u32::MAX, 1, u32::MAX, 1), Overflow),
        ((1, u32::MAX, 1, u32::MAX), Overflow),
    ] {
        let (tw, th, pw, ph) = dimensions;
        assert_eq!(
            GridAlignment::from_dimensions(tw, th, pw, ph),
            Err(expected.clone())
        );
        assert_eq!(terrain_pathing_alignment(tw, th, pw, ph), Err(expected));
    }
}

#[test]
fn largest_representable_corner_grid_has_exact_u64_products() {
    let extent = u32::MAX - 1;
    let aligned = terrain_pathing_alignment(extent, extent, extent, extent).unwrap();
    assert_eq!(
        aligned.terrain.corners,
        GridDimensions {
            width: u32::MAX,
            height: u32::MAX
        }
    );
    assert_eq!(aligned.terrain_corner_count, u64::from(u32::MAX).pow(2));
    assert_eq!(aligned.pathing_cell_count, u64::from(extent).pow(2));
}

// Fail closed for every error, retaining the local map/member context without
// printing error payloads that can contain original bytes or extracted text.
fn require_member<T, E>(result: Result<T, E>, path: &std::path::Path, member: &str) -> T {
    result.unwrap_or_else(|_| {
        panic!(
            "{path:?}: {member}: missing, malformed, or unsupported; cannot check grid alignment"
        )
    })
}

#[test]
fn original_check_refuses_missing_and_corrupt_members_with_context() {
    type MemberFailure = (&'static str, fn() -> pcx_map::Error);
    let failures: [MemberFailure; 4] = [
        ("war3map.w3e", || pcx_map::Error::TerrainTruncated {
            have: 0,
            need: 0,
        }),
        ("war3map.w3e", || {
            pcx_map::Error::UnsupportedTerrainVersion(99)
        }),
        ("war3map.wpm", || {
            pcx_map::Error::Extraction(pcx_mpq::Error::NotFound("war3map.wpm".into()))
        }),
        ("war3map.wpm", || {
            pcx_map::Error::Pathing(pcx_map::PathingError::Truncated)
        }),
    ];
    for (member, error) in failures {
        let panic = std::panic::catch_unwind(|| {
            require_member::<(), _>(Err(error()), std::path::Path::new("synthetic.w3m"), member);
        })
        .unwrap_err();
        let message = panic.downcast_ref::<String>().unwrap();
        assert!(message.contains("synthetic.w3m"));
        assert!(message.contains(member));
        assert!(message.contains("cannot check grid alignment"));
    }
}

#[test]
#[ignore = "requires PEACECRAFT3_FORMAT_INSTALL with a local Maps tree; checks loose maps only, not install archives"]
fn original_map_grids_are_checked_inputs() {
    let install =
        std::env::var_os("PEACECRAFT3_FORMAT_INSTALL").expect("set PEACECRAFT3_FORMAT_INSTALL");
    let mut pending = vec![std::path::PathBuf::from(&install).join("Maps")];
    let mut checked = 0usize;
    let mut observed_scales = std::collections::BTreeMap::<u32, usize>::new();
    while let Some(directory) = pending.pop() {
        let entries =
            std::fs::read_dir(&directory).unwrap_or_else(|error| panic!("{directory:?}: {error}"));
        for entry in entries {
            let entry = entry.unwrap_or_else(|error| panic!("{directory:?}: {error}"));
            let path = entry.path();
            let kind = entry
                .file_type()
                .unwrap_or_else(|error| panic!("{path:?}: {error}"));
            if kind.is_dir() {
                pending.push(path);
            } else if kind.is_file()
                && path.extension().is_some_and(|extension| {
                    extension.eq_ignore_ascii_case("w3m") || extension.eq_ignore_ascii_case("w3x")
                })
            {
                let data = std::fs::read(&path).unwrap_or_else(|error| panic!("{path:?}: {error}"));
                let map = require_member(pcx_map::Map::parse(&data), &path, "map container");
                let terrain = require_member(map.terrain(), &path, "war3map.w3e");
                let pathing = require_member(map.pathing(), &path, "war3map.wpm");
                let aligned = terrain_pathing_alignment(
                    terrain.width(),
                    terrain.height(),
                    pathing.width,
                    pathing.height,
                )
                .unwrap_or_else(|error| panic!("{path:?}: war3map.w3e / war3map.wpm: {error}"));
                *observed_scales.entry(aligned.uniform_scale).or_default() += 1;
                checked += 1;
            }
        }
    }
    assert!(
        checked > 0,
        "no loose map grid pairs checked under supplied Maps tree"
    );
    eprintln!("checked loose map grid pairs={checked}; observed scales={observed_scales:?}");
}

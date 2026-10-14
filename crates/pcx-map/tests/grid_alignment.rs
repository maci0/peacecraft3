#[path = "../src/grid_alignment.rs"]
mod grid_alignment;

use grid_alignment::{GridAlignment, GridAlignmentError};

#[test]
fn valid_uniform_scale() {
    let aligned = GridAlignment::from_dimensions(64, 64, 256, 256).unwrap();
    assert_eq!(aligned.uniform_scale, 4);
    assert_eq!(aligned.width_scale, 4);
    assert_eq!(aligned.height_scale, 4);
    assert_eq!(aligned.terrain.tiles.width, 64);
    assert_eq!(aligned.terrain.tiles.height, 64);
    assert_eq!(aligned.terrain.corners.width, 65);
    assert_eq!(aligned.terrain.corners.height, 65);
    assert_eq!(aligned.pathing.raster.width, 256);
    assert_eq!(aligned.pathing.raster.height, 256);
    assert_eq!(aligned.terrain_tile_count, 4096);
    assert_eq!(aligned.terrain_corner_count, 4225);
    assert_eq!(aligned.pathing_cell_count, 65536);
}

#[test]
fn free_helper_works() {
    assert!(grid_alignment::terrain_pathing_alignment(2, 2, 6, 6).is_ok());
}

#[test]
fn non_divisible_width_rejected() {
    assert!(matches!(
        GridAlignment::from_dimensions(64, 64, 255, 256),
        Err(GridAlignmentError::NonDivisibleWidth {
            terrain: 64,
            pathing: 255
        })
    ));
}

#[test]
fn non_divisible_height_rejected() {
    assert!(matches!(
        GridAlignment::from_dimensions(64, 64, 256, 255),
        Err(GridAlignmentError::NonDivisibleHeight {
            terrain: 64,
            pathing: 255
        })
    ));
}

#[test]
fn scale_mismatch_rejected() {
    assert!(matches!(
        GridAlignment::from_dimensions(64, 64, 256, 128),
        Err(GridAlignmentError::ScaleMismatch {
            width_scale: 4,
            height_scale: 2
        })
    ));
}

#[test]
fn zero_dimensions_rejected() {
    assert!(matches!(
        GridAlignment::from_dimensions(0, 1, 1, 1),
        Err(GridAlignmentError::ZeroTerrainWidth)
    ));
    assert!(matches!(
        GridAlignment::from_dimensions(1, 0, 1, 1),
        Err(GridAlignmentError::ZeroTerrainHeight)
    ));
    assert!(matches!(
        GridAlignment::from_dimensions(1, 1, 0, 1),
        Err(GridAlignmentError::ZeroPathingWidth)
    ));
    assert!(matches!(
        GridAlignment::from_dimensions(1, 1, 1, 0),
        Err(GridAlignmentError::ZeroPathingHeight)
    ));
}

#[test]
fn corner_overflow_rejected() {
    assert!(matches!(
        GridAlignment::from_dimensions(u32::MAX, 1, u32::MAX, 1),
        Err(GridAlignmentError::Overflow)
    ));
}

#[test]
fn anisotropic_equal_ratio_required() {
    assert!(GridAlignment::from_dimensions(2, 3, 4, 6).is_ok());
}

#[test]
#[ignore = "requires PEACECRAFT3_FORMAT_INSTALL pointing at a local original install"]
fn original_map_grids_are_checked_inputs() {
    use pcx_map::Map;
    use std::collections::BTreeMap;

    let install = std::env::var("PEACECRAFT3_FORMAT_INSTALL")
        .expect("PEACECRAFT3_FORMAT_INSTALL must point at a local original install");
    let mut files = 0usize;
    let mut records_without_grids = 0usize;
    let mut checked = 0usize;
    let mut observed_scales: BTreeMap<u32, usize> = BTreeMap::new();

    let mut pending = vec![std::path::PathBuf::from(&install).join("Maps")];
    while let Some(directory) = pending.pop() {
        if !directory.exists() {
            continue;
        }
        let entries = match std::fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) => panic!("{directory:?}: {error}"),
        };
        for entry in entries {
            let entry = entry.unwrap_or_else(|error| panic!("{directory:?}: {error}"));
            let file_type = entry
                .file_type()
                .unwrap_or_else(|error| panic!("{:?}: {error}", entry.path()));
            let path = entry.path();
            if file_type.is_dir() {
                pending.push(path);
                continue;
            }
            let lower = path.to_string_lossy().to_ascii_lowercase();
            if !(lower.ends_with(".w3m") || lower.ends_with(".w3x")) {
                continue;
            }
            files += 1;
            let data = match std::fs::read(&path) {
                Ok(data) => data,
                Err(error) => {
                    panic!("{path:?}: read error: {error}");
                }
            };
            let map = match Map::parse(&data) {
                Ok(map) => map,
                Err(error) => {
                    panic!("{path:?}: parse error: {error}");
                }
            };
            let terrain = map.terrain();
            let pathing = map.pathing();
            match (terrain, pathing) {
                (Ok(terrain), Ok(pathing)) => {
                    match grid_alignment::GridAlignment::from_dimensions(
                        terrain.width(),
                        terrain.height(),
                        pathing.width,
                        pathing.height,
                    ) {
                        Ok(aligned) => {
                            *observed_scales.entry(aligned.uniform_scale).or_insert(0) += 1;
                            checked += 1;
                        }
                        Err(error) => {
                            panic!("{path:?}: unexpected grid relationship: {error}");
                        }
                    }
                }
                (Err(_), Err(_)) => {
                    records_without_grids += 1;
                }
                (result, _) => {
                    let error = result
                        .err()
                        .map(|error| error.to_string())
                        .unwrap_or_default();
                    panic!("{path:?}: inconsistent grid availability: {error}");
                }
            }
        }
    }

    assert!(files > 0, "no map files found under supplied install");
    for (scale, count) in &observed_scales {
        eprintln!("observed terrain/pathing scale {scale}: {count} maps");
    }
    eprintln!(
        "checked supplied maps={files} readable_maps={files} parsed_maps={files} grid_records={checked} records_without_grids={records_without_grids}"
    );
    assert!(checked > 0, "no terrain/pathing grid records were checked");
}

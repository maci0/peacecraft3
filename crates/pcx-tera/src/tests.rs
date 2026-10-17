//! Tests: a hand-built 2×2 fixture for exact values, install-guarded real-map
//! tests, and an `#[ignore]`d sweep over every shipped terrain.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;

use crate::{TerrainMesh, TileCatalog, corner_world_scaled};

use pcx_map::{Map, Terrain, shipped_maps};

use super::{TILE_XY, Z_PER_INDEX, Z_REFERENCE_INDEX};

fn workspace() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn install_dir() -> Option<PathBuf> {
    let p = workspace().join("wc3/game/app");
    p.is_dir().then_some(p)
}

fn legacy_dir() -> Option<PathBuf> {
    let p = std::env::var_os("PEACECRAFT3_LEGACY_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            let p = workspace()
                .join("../../Faugus/battle-net/drive_c/Program Files (x86)/Warcraft III (Legacy)");
            p.is_dir().then_some(p)
        })?;
    p.is_dir().then_some(p)
}

/// A hand-built `war3map.w3e` for a 2×2-tile terrain (3×3 corners), with
/// heights chosen so the world `z` lands on whole units:
/// `z = (height_index - 8192) * 0.25`.
fn fixture_terrain() -> Terrain {
    let mut b = Vec::new();
    b.extend_from_slice(b"W3E!");
    b.extend_from_slice(&11u32.to_le_bytes()); // version
    b.push(5); // default slot id
    b.extend_from_slice(&0u32.to_le_bytes()); // flag word
    b.extend_from_slice(&2u32.to_le_bytes()); // N ground slots
    b.extend_from_slice(b"AAAA");
    b.extend_from_slice(b"BBBB");
    b.extend_from_slice(&1u32.to_le_bytes()); // M secondary slots
    b.extend_from_slice(b"CCCC");
    b.extend_from_slice(&3u32.to_le_bytes()); // width + 1
    b.extend_from_slice(&3u32.to_le_bytes()); // height + 1
    b.extend_from_slice(&0.0f32.to_le_bytes()); // origin x
    b.extend_from_slice(&0.0f32.to_le_bytes()); // origin y

    // (height_index, position_word, slot_nibble, corner_flags) row-major, 3×3.
    #[rustfmt::skip]
    let corners: &[(u16, u16, u8, u8)] = &[
        (8192, 0x4000, 0, 0), (8196, 0, 1, 0), (8192, 0, 0, 0),
        (8188, 0, 0, 0),      (8192, 0, 1, 0), (8196, 0, 0, 0),
        (8192, 0, 0, 0),      (8188, 0, 1, 0), (8192, 0, 0, 0),
    ];
    for &(h, pos, slot, flags) in corners {
        b.extend_from_slice(&h.to_le_bytes());
        b.extend_from_slice(&pos.to_le_bytes());
        b.push((flags << 4) | slot);
        b.push(0);
        b.push(0);
    }
    Terrain::from_bytes(b).unwrap()
}

#[test]
fn water_preserves_original_corner_bytes_and_provenance() {
    let original = fixture_terrain();
    let mut bytes = original.raw().to_vec();
    let body = bytes.len() - original.corner_count() * pcx_map::CORNER_RECORD;
    // Synthetic reserved bits and opaque fields must survive without meanings.
    bytes[body..body + pcx_map::CORNER_RECORD]
        .copy_from_slice(&[0xff, 0xff, 0xab, 0xcd, 0xe1, 0x82, 0x94]);
    let terrain = Terrain::from_bytes(bytes).unwrap();
    let water = crate::WaterData::new(&terrain, "synthetic edition", "fixture.w3m").unwrap();
    let corner = water.raw_corner(0, 0).unwrap();
    assert_eq!(corner.byte_offset, body);
    assert_eq!(corner.record, &[0xff, 0xff, 0xab, 0xcd, 0xe1, 0x82, 0x94]);
    assert_eq!(corner.corner.position_word, 0xcdab);
    assert_eq!(corner.corner.corner_flags, 0xe);
    assert_eq!(corner.provenance, water.provenance());
    assert_eq!(corner.provenance.terrain_version, terrain.version());
    assert_eq!(corner.provenance.map_source, "fixture.w3m");
    assert_eq!(corner.provenance.edition, "synthetic edition");
    assert_eq!(
        water.raw_corner(1, 1).unwrap().byte_offset,
        body + 4 * pcx_map::CORNER_RECORD
    );
    assert_eq!(
        water.raw_corner(2, 2).unwrap().byte_offset,
        body + 8 * pcx_map::CORNER_RECORD
    );
    assert!(water.raw_corner(3, 0).is_none());
    assert!(water.raw_corner(0, u32::MAX).is_none());
}

fn water_slk(id: &str, height: &str) -> String {
    format!(
        "ID;PWXL;N;E\r\nC;Y1;X1;K\"waterID\"\r\nC;X2;K\"height\"\r\nC;Y2;X1;K\"{id}\"\r\nC;X2;K\"{height}\"\r\nE\r\n"
    )
}

fn water_tile(tileset: u8, corners: &[(u16, u16, u8)]) -> Terrain {
    let mut bytes = pcx_map::TERRAIN_MAGIC.to_vec();
    bytes.extend(pcx_map::TERRAIN_VERSION.to_le_bytes());
    bytes.push(tileset);
    bytes.extend(0u32.to_le_bytes());
    bytes.extend(1u32.to_le_bytes());
    bytes.extend_from_slice(b"Ldrt");
    bytes.extend(0u32.to_le_bytes());
    bytes.extend(2u32.to_le_bytes());
    bytes.extend(2u32.to_le_bytes());
    bytes.extend(0f32.to_le_bytes());
    bytes.extend(0f32.to_le_bytes());
    for &(ground, water, flags) in corners {
        bytes.extend(ground.to_le_bytes());
        bytes.extend(water.to_le_bytes());
        bytes.push(flags << 4);
        bytes.push(0);
        bytes.push(0);
    }
    Terrain::from_bytes(bytes).unwrap()
}

fn water_budget() -> crate::WaterBudget {
    crate::WaterBudget {
        max_vertices: 64,
        max_indices: 64,
    }
}

#[test]
fn dry_water_mesh_is_empty_and_out_of_range_is_none() {
    let terrain = water_tile(b'L', &[(8192, 8192, 0); 4]);
    let water = crate::WaterData::new(&terrain, "synthetic edition", "fixture.w3m").unwrap();
    let bytes = water_slk("LSha", "0.5");
    let table = pcx_slq::Table::parse(bytes.as_bytes()).unwrap();
    let profile = crate::WaterProfile::open(
        &table,
        "synthetic Water.slk",
        crate::WaterSelection::TilesetSha,
        &terrain,
    )
    .unwrap();
    assert_eq!(profile.water_id(), "LSha");
    assert_eq!(profile.height_offset(), 0.5 * TILE_XY);
    assert_eq!(water.water_present(0, 0), Some(false));
    assert_eq!(water.water_level(3, 0, &profile).unwrap(), None);
    let mesh = water.mesh(&profile, &water_budget()).unwrap();
    assert_eq!(mesh.wet_tiles, 0);
    assert!(mesh.positions.is_empty());
    assert!(matches!(
        crate::WaterData::new(&terrain, " ", "fixture.w3m"),
        Err(crate::MissingWaterProvenance("edition"))
    ));
    assert!(matches!(
        crate::WaterData::new(&terrain, "synthetic edition", ""),
        Err(crate::MissingWaterProvenance("map source"))
    ));
    assert!(
        crate::Unsupported::WaterLevel
            .to_string()
            .contains("Water.slk")
    );
    assert!(
        crate::Unsupported::WaterGeometry
            .to_string()
            .contains("WaterData::mesh")
    );
}

#[test]
fn wet_nonflat_levels_use_the_masked_second_word_and_selected_offset() {
    let words = [8192u16, 8196, 8192 | 0x4000, 8192 | 0x8000];
    let terrain = water_tile(
        b'L',
        &[
            (8192, words[0], 0b0100),
            (8192, words[1], 0),
            (8192, words[2], 0),
            (8192, words[3], 0),
        ],
    );
    let water = crate::WaterData::new(&terrain, "synthetic edition", "fixture.w3m").unwrap();
    let bytes = water_slk("Pool", "0.25");
    let table = pcx_slq::Table::parse(bytes.as_bytes()).unwrap();
    let profile = crate::WaterProfile::open(
        &table,
        "synthetic Water.slk",
        crate::WaterSelection::Explicit("Pool"),
        &terrain,
    )
    .unwrap();
    let offset = profile.height_offset();
    for (index, word) in words.into_iter().enumerate() {
        let level = water
            .water_level(index as u32 % 2, index as u32 / 2, &profile)
            .unwrap()
            .unwrap();
        assert_eq!(level, crate::world_z(word & 0x3FFF) + offset);
    }
    let mesh = water.mesh(&profile, &water_budget()).unwrap();
    assert_eq!(mesh.wet_tiles, 1);
    assert_eq!(mesh.indices, vec![0, 1, 3, 0, 3, 2]);
    assert_eq!(mesh.positions[1][2], crate::world_z(8196) + offset);
    assert_ne!(mesh.positions[0][2], mesh.positions[1][2]);
}

#[test]
fn malformed_water_profile_and_short_budget_are_errors() {
    let terrain = water_tile(b'L', &[(8192, 8192, 0b0100); 4]);
    let water = crate::WaterData::new(&terrain, "synthetic edition", "fixture.w3m").unwrap();
    let missing = water_slk("Other", "0");
    let table = pcx_slq::Table::parse(missing.as_bytes()).unwrap();
    assert!(matches!(
        crate::WaterProfile::open(
            &table,
            "synthetic Water.slk",
            crate::WaterSelection::Explicit("LSha"),
            &terrain,
        ),
        Err(crate::WaterError::MissingRow { .. })
    ));
    let bad = water_slk("LSha", "nope");
    let table = pcx_slq::Table::parse(bad.as_bytes()).unwrap();
    assert!(matches!(
        crate::WaterProfile::open(
            &table,
            "synthetic Water.slk",
            crate::WaterSelection::TilesetSha,
            &terrain,
        ),
        Err(crate::WaterError::MissingField {
            field: "height",
            ..
        })
    ));
    let bytes = water_slk("LSha", "0");
    let table = pcx_slq::Table::parse(bytes.as_bytes()).unwrap();
    let profile = crate::WaterProfile::open(
        &table,
        "synthetic Water.slk",
        crate::WaterSelection::TilesetSha,
        &terrain,
    )
    .unwrap();
    assert!(matches!(
        water.mesh(
            &profile,
            &crate::WaterBudget {
                max_vertices: 3,
                max_indices: 64,
            },
        ),
        Err(crate::WaterError::Budget {
            what: "vertices",
            needed: 4,
            ..
        })
    ));
    assert!(matches!(
        water.mesh(
            &profile,
            &crate::WaterBudget {
                max_indices: 5,
                ..water_budget()
            },
        ),
        Err(crate::WaterError::Budget {
            what: "indices",
            needed: 6,
            limit: 5,
        })
    ));
    let unnamed = water_tile(0, &[(8192, 8192, 0); 4]);
    assert!(matches!(
        crate::WaterProfile::open(
            &table,
            "synthetic Water.slk",
            crate::WaterSelection::TilesetSha,
            &unnamed,
        ),
        Err(crate::WaterError::TilesetByte { byte: 0 })
    ));
}

#[test]
fn water_profiles_refuse_duplicate_rows_and_nonfinite_height() {
    let terrain = water_tile(b'L', &[(8192, 8192, 0b0100); 4]);
    let duplicate = water_slk("LSha", "0");
    let duplicate = format!(
        "{}C;Y3;X1;K\"LSha\"\r\nC;X2;K0\r\nE\r\n",
        duplicate.strip_suffix("E\r\n").unwrap(),
    );
    let table = pcx_slq::Table::parse(duplicate.as_bytes()).unwrap();
    assert!(matches!(
        crate::WaterProfile::open(&table, "duplicate.slk", crate::WaterSelection::TilesetSha, &terrain),
        Err(crate::WaterError::Ambiguous { table, .. }) if table == "duplicate.slk"
    ));
    for height in ["NaN", "inf", "3.4028235e38"] {
        let bytes = water_slk("LSha", height);
        let table = pcx_slq::Table::parse(bytes.as_bytes()).unwrap();
        assert!(matches!(
            crate::WaterProfile::open(
                &table,
                "invalid.slk",
                crate::WaterSelection::TilesetSha,
                &terrain
            ),
            Err(crate::WaterError::MissingField {
                field: "height",
                ..
            })
        ));
    }
}

#[test]
fn wall_geometry_uses_only_the_callers_coordinates_and_threshold() {
    let edge = [[10.0, -4.0, 3.0], [13.0, 0.0, 7.0]];
    let wall = crate::cliff_wall(edge, 3.0).unwrap().unwrap();
    assert_eq!(
        wall.positions,
        [
            [10.0, -4.0, 3.0],
            [13.0, 0.0, 3.0],
            [13.0, 0.0, 7.0],
            [10.0, -4.0, 7.0],
        ]
    );
    assert_eq!(wall.normal, [0.8, -0.6, 0.0]);
    assert_eq!(crate::cliff_wall(edge, 4.0).unwrap(), None);
    assert_eq!(crate::cliff_wall(edge, 5.0).unwrap(), None);
    assert!(crate::cliff_wall(edge, 0.0).unwrap().is_some());
    let reversed = crate::cliff_wall([edge[1], edge[0]], 3.0).unwrap().unwrap();
    assert_eq!(reversed.normal, [-0.8, 0.6, 0.0]);
    for quad in [wall, reversed] {
        for tri in crate::WallQuad::TRIANGLE_INDICES.as_chunks::<3>().0 {
            let a = quad.positions[tri[0] as usize];
            let b = quad.positions[tri[1] as usize];
            let c = quad.positions[tri[2] as usize];
            let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let normal = [
                ab[1] * ac[2] - ab[2] * ac[1],
                ab[2] * ac[0] - ab[0] * ac[2],
                ab[0] * ac[1] - ab[1] * ac[0],
            ];
            let dot = normal
                .iter()
                .zip(quad.normal)
                .map(|(a, b)| a * b)
                .sum::<f32>();
            assert!(dot > 0.0, "triangle winding must match the face normal");
        }
    }
}

#[test]
fn wall_tile_perimeter_has_no_diagonal_or_implicit_cliff_rule() {
    let corners = [
        [2.0, 3.0, 1.0],
        [9.0, 3.0, 5.0],
        [2.0, 11.0, 1.0],
        [9.0, 11.0, 5.0],
    ];
    let walls = crate::cliff_walls(corners, 2.0).unwrap();
    assert_eq!(walls.len(), 2);
    assert_eq!(walls[0].normal, [0.0, -1.0, 0.0]);
    assert_eq!(walls[1].normal, [0.0, 1.0, 0.0]);
    assert!(crate::cliff_walls(corners, 4.0).unwrap().is_empty());
    let profile = crate::TileProfile::of(&fixture_terrain(), 0, 0).unwrap();
    assert_eq!(
        crate::cliff_class(&profile),
        Err(crate::Unsupported::CliffRule)
    );
    assert_eq!(
        crate::cliff::cliff_threshold(),
        Err(crate::Unsupported::CliffThreshold)
    );
}

#[test]
fn wall_rejects_invalid_geometry_and_skips_flat_edges() {
    use crate::WallGeometryError::{CoincidentEndpoints, InvalidThreshold, NonFiniteGeometry};
    let edge = [[0.0, 0.0, 0.0], [2.0, 3.0, 4.0]];
    for threshold in [-1.0, f32::NAN, f32::INFINITY] {
        assert_eq!(crate::cliff_wall(edge, threshold), Err(InvalidThreshold));
    }
    assert_eq!(crate::cliff_wall([[0.0; 3]; 2], 0.0), Ok(None));
    assert_eq!(
        crate::cliff_wall([[0.0; 3], [0.0, 0.0, 1.0]], 0.0),
        Err(CoincidentEndpoints)
    );
    assert_eq!(
        crate::cliff_wall([[f32::NAN; 3], edge[1]], 0.0),
        Err(NonFiniteGeometry)
    );
    assert_eq!(
        crate::cliff_wall([[f32::MAX; 3], [-f32::MAX; 3]], 0.0),
        Err(NonFiniteGeometry)
    );
}

#[test]
fn fixture_terrain_is_exactly_the_shipped_framing() {
    let t = fixture_terrain();
    assert_eq!((t.width(), t.height()), (2, 2));
    assert_eq!(t.corner_count(), 9);
    assert_eq!(t.slots(), &[*b"AAAA", *b"BBBB"]);
    assert_eq!(t.secondary_slots(), &[*b"CCCC"]);
    assert_eq!(t.origin(), (0.0, 0.0));
}

#[test]
fn fixture_mesh_positions_and_flags_are_exact() {
    let t = fixture_terrain();
    let m = TerrainMesh::build(&t);

    assert_eq!(m.vertex_count(), 9, "(w+1)(h+1)");
    assert_eq!(m.triangle_count(), 8, "2*w*h");
    assert_eq!(m.indices.len(), 24);

    // Hand-computed world positions. x=origin_x+cx*128, y=+cy*128,
    // z=(h-8192)*0.25.
    let z = |h: u16| (h as f32 - 8192.0) * 0.25;
    assert_eq!(
        m.positions,
        vec![
            [0.0, 0.0, z(8192)],     // (0,0)
            [128.0, 0.0, z(8196)],   // (1,0)
            [256.0, 0.0, z(8192)],   // (2,0)
            [0.0, 128.0, z(8188)],   // (0,1)
            [128.0, 128.0, z(8192)], // (1,1)
            [256.0, 128.0, z(8196)], // (2,1)
            [0.0, 256.0, z(8192)],   // (0,2)
            [128.0, 256.0, z(8188)], // (1,2)
            [256.0, 256.0, z(8192)], // (2,2)
        ]
    );
    assert_eq!(m.positions[1][2], 1.0);
    assert_eq!(m.positions[3][2], -1.0);

    // UVs are grid-space: u = cx, v = cy.
    assert_eq!(m.uvs[0], [0.0, 0.0]);
    assert_eq!(m.uvs[4], [1.0, 1.0]);
    assert_eq!(m.uvs[8], [2.0, 2.0]);

    // Slot nibble = cx % 2, carried raw.
    assert_eq!(m.slot_per_vertex, vec![0, 1, 0, 0, 1, 0, 0, 1, 0]);

    // Corner 0 keeps position_word bit 14 as a preserved flag.
    let t = fixture_terrain();
    assert!(t.corner(0, 0).unwrap().position_flag());
    assert!(!t.corner(1, 0).unwrap().position_flag());

    // Triangle winding: two triangles per tile, (a,b,d) and (a,d,c).
    assert_eq!(
        m.indices,
        vec![
            0, 1, 4, 0, 4, 3, // tile (0,0)
            1, 2, 5, 1, 5, 4, // tile (1,0)
            3, 4, 7, 3, 7, 6, // tile (0,1)
            4, 5, 8, 4, 8, 7, // tile (1,1)
        ]
    );
}

#[test]
fn fixture_normals_point_up() {
    let t = fixture_terrain();
    let m = TerrainMesh::build(&t);
    // Every vertex normal has a positive z component on this gentle bump.
    for n in &m.normals {
        assert!(n[2] > 0.0, "normal {n:?} not up");
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        assert!((len - 1.0).abs() < 1e-5, "normal not unit: {len}");
    }
}

#[test]
fn fixture_mesh_is_closed_manifold() {
    let t = fixture_terrain();
    let m = TerrainMesh::build(&t);
    let r = m.manifold();
    // 2×2 tiles: boundary is the 2w+2h = 8 perimeter edges, each used once;
    // the 8 interior edges (grid + diagonals) each used twice; none more.
    assert_eq!(r.max_shared, 2);
    assert_eq!(r.boundary_edges, 2 * 2 + 2 * 2);
    assert_eq!(r.interior_edges, 8);
    assert!(r.oriented);
    assert!(r.is_clean());
}

#[test]
fn build_scaled_overrides_the_defaults() {
    let t = fixture_terrain();
    let m = TerrainMesh::build_scaled(&t, 100.0, 1.0, 0.0);
    // xy 100/tile, z 1/index, reference index 0.
    assert_eq!(m.positions[2], [200.0, 0.0, 8192.0]);
    let w = corner_world_scaled(&t, 1, 0, 100.0, 1.0, 0.0).unwrap();
    assert_eq!(w, [100.0, 0.0, 8196.0]);
    // The default constants are what the shipped bytes measure.
    let d = corner_world_default(&t, 1, 0);
    assert_eq!(d, [128.0, 0.0, 1.0]);
}

fn corner_world_default(t: &Terrain, cx: u32, cy: u32) -> [f32; 3] {
    corner_world_scaled(t, cx, cy, TILE_XY, Z_PER_INDEX, Z_REFERENCE_INDEX).unwrap()
}

// ---- real-map evidence ----------------------------------------------------

/// Bilinear height-index interpolation at fractional grid (gx, gy).
fn interp_h(t: &Terrain, gx: f32, gy: f32) -> f32 {
    let w = t.width();
    let h = t.height();
    let cx = (gx.floor() as i64).clamp(0, (w - 1) as i64);
    let cy = (gy.floor() as i64).clamp(0, (h - 1) as i64);
    let fx = (gx - cx as f32).clamp(0.0, 1.0);
    let fy = (gy - cy as f32).clamp(0.0, 1.0);
    let c00 = t.corner(cx as u32, cy as u32).unwrap().height_index as f32;
    let c10 = t.corner(cx as u32 + 1, cy as u32).unwrap().height_index as f32;
    let c01 = t.corner(cx as u32, cy as u32 + 1).unwrap().height_index as f32;
    let c11 = t.corner(cx as u32 + 1, cy as u32 + 1).unwrap().height_index as f32;
    let a = c00 * (1.0 - fx) + c10 * fx;
    let b = c01 * (1.0 - fx) + c11 * fx;
    a * (1.0 - fy) + b * fy
}

type FitsSummary = (Vec<(f64, f64, usize)>, bool);

fn all_fits(m: &Map) -> Option<FitsSummary> {
    let t = m.terrain().ok()?;
    let d = m.doodads().ok()?;
    let (ox, oy) = t.origin();
    let w = t.width() as f32;
    let h = t.height() as f32;

    let in_grid = d.instances().iter().all(|inst| {
        let gx = (inst.position[0] - ox) / TILE_XY;
        let gy = (inst.position[1] - oy) / TILE_XY;
        gx >= -0.5 && gy >= -0.5 && gx <= w + 0.5 && gy <= h + 0.5
    });

    let mut by_type: BTreeMap<[u8; 4], Vec<(f32, f32)>> = BTreeMap::new();
    for inst in d.instances() {
        let gx = (inst.position[0] - ox) / TILE_XY;
        let gy = (inst.position[1] - oy) / TILE_XY;
        if (0.0..=w).contains(&gx) && (0.0..=h).contains(&gy) {
            by_type
                .entry(inst.type_id)
                .or_default()
                .push((interp_h(&t, gx, gy), inst.position[2]));
        }
    }

    let mut fits = Vec::new();
    for pts in by_type.values() {
        if pts.len() < 8 {
            continue;
        }
        let n = pts.len() as f64;
        let mx = pts.iter().map(|p| p.0 as f64).sum::<f64>() / n;
        let my = pts.iter().map(|p| p.1 as f64).sum::<f64>() / n;
        let (mut sxx, mut sxy) = (0.0, 0.0);
        for (x, y) in pts {
            sxx += (*x as f64 - mx) * (*x as f64 - mx);
            sxy += (*x as f64 - mx) * (*y as f64 - my);
        }
        if sxx == 0.0 {
            continue;
        }
        let slope = sxy / sxx;
        let intercept = my - slope * mx;
        fits.push((slope, intercept, pts.len()));
    }
    Some((fits, in_grid))
}

/// Open a shipped map from an install, leaking the bytes so the returned
/// `Map` can borrow them for the test's duration.
fn shipped(dir: &PathBuf, path: &str) -> Option<Map<'static>> {
    let mut set = pcx_mpq::MpqSet::open_wc3(dir).ok()?;
    let bytes: &'static [u8] = Box::leak(set.extract(path).ok()?.into_boxed_slice());
    Map::parse(bytes).ok()
}

/// The measured constants, asserted against real geometry on named maps.
///
/// `Z_PER_INDEX` is pinned by the least-squares slope of doodad world `z`
/// against interpolated corner height indices (`0.25` on every named map).
/// `Z_REFERENCE_INDEX` (→ intercept `-0.25*8192 = -2048`) is pinned by a
/// *ground-sitting* doodad type whose pivot is at ground level. `TILE_XY` is
/// pinned by whether every doodad lands inside its own tile grid.
#[test]
fn scale_is_measured_on_shipped_maps() {
    let named_128c = [
        r"(4)Adrenaline.w3m",
        r"(1)TheDeathSheep.w3m",
        r"Maps\Campaign\Undead08.w3m",
        r"Maps\Campaign\Orc02.w3m",
    ];
    let Some(dir) = install_dir() else {
        eprintln!("1.28c install absent; skipping scale evidence");
        return;
    };
    let mut slope_proved = 0usize;
    let mut grid_proved = 0usize;
    let mut reference_proved = 0usize;
    for path in named_128c {
        let Some(m) = shipped(&dir, path) else {
            continue;
        };
        let Some((fits, in_grid)) = all_fits(&m) else {
            continue;
        };
        // Z_PER_INDEX: some type on this map fits slope 0.25.
        if fits
            .iter()
            .any(|(s, _, n)| *n >= 8 && (s - 0.25).abs() < 5e-3)
        {
            slope_proved += 1;
        }
        // TILE_XY: every doodad is inside the 128/tile grid.
        if in_grid {
            grid_proved += 1;
        }
        // Z_REFERENCE_INDEX: a well-populated type whose model sits on the
        // ground (pivot at ground level) has intercept -2048. The most
        // numerous such type is ordinary terrain foliage.
        let ground = fits
            .iter()
            .filter(|(s, _, n)| *n >= 8 && (s - 0.25).abs() < 5e-3)
            .max_by_key(|(_, _, n)| *n);
        if let Some((_, intercept, n)) = ground
            && (intercept + 2048.0).abs() < 40.0
        {
            reference_proved += 1;
            eprintln!("{path}: ground type n={n} intercept={intercept:.3}");
        }
    }
    assert!(
        slope_proved >= 3,
        "slope 0.25 proved on only {slope_proved}"
    );
    assert!(
        grid_proved >= 3,
        "128/tile grid proved on only {grid_proved}"
    );
    assert!(
        reference_proved >= 1,
        "index-8192 reference proved on no named map"
    );
}

/// Human campaign first mission (the M3 gate map): exact vertex/triangle
/// counts, a clean mesh, and world z spanning exactly what its height indices
/// measure.
#[test]
fn human01_mesh_matches_its_terrain() {
    let Some(dir) = legacy_dir().or_else(install_dir) else {
        eprintln!("no install with Human01; skipping");
        return;
    };
    let Some(m) = shipped(&dir, r"Maps\Campaign\Human01.w3m").or_else(|| {
        // Fall back to any human01 spelling.
        shipped(&dir, r"maps\campaign\human01.w3m")
    }) else {
        eprintln!("Human01 not present; skipping");
        return;
    };
    let t = m.terrain().unwrap();
    let mesh = TerrainMesh::build(&t);

    assert_eq!(mesh.vertex_count(), 96 * 96 + 96 + 96 + 1);
    assert_eq!(
        mesh.vertex_count(),
        (t.width() as usize + 1) * (t.height() as usize + 1)
    );
    assert_eq!(mesh.triangle_count(), 2 * 96 * 96);
    assert!(mesh.manifold().is_clean());

    // World z bounds equal the terrain's index bounds run through the scale.
    let (lo, hi) = t.height_index_range().unwrap();
    let zs: Vec<f32> = mesh.positions.iter().map(|p| p[2]).collect();
    let zmin = zs.iter().cloned().fold(f32::INFINITY, f32::min);
    let zmax = zs.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    assert!((zmin - (lo as f32 - Z_REFERENCE_INDEX) * Z_PER_INDEX).abs() < 1e-3);
    assert!((zmax - (hi as f32 - Z_REFERENCE_INDEX) * Z_PER_INDEX).abs() < 1e-3);
}

/// Every ground slot fourcc used by shipped maps resolves to a BLP that
/// exists in the archive. The single known exception on 1.28c is the cliff
/// accent `CAc2`, which lives in the cliff table, not `Terrain.slk`.
#[test]
fn tile_catalog_resolves_shipped_ground_slots() {
    for (label, dir) in [("1.28c", install_dir()), ("1.29b", legacy_dir())] {
        let Some(dir) = dir else { continue };
        let mut set = pcx_mpq::MpqSet::open_wc3(&dir).unwrap();
        let slk = set.extract("TerrainArt\\Terrain.slk").unwrap();
        let catalog = TileCatalog::from_terrain_slk(&slk).unwrap();
        let files: std::collections::BTreeSet<String> = set
            .file_list()
            .unwrap()
            .into_iter()
            .map(|f| f.to_ascii_lowercase())
            .collect();

        let maps = shipped_maps(&mut set).unwrap();
        let mut unresolved: std::collections::BTreeSet<[u8; 4]> = Default::default();
        let mut missing_files: Vec<String> = Vec::new();
        for path in &maps {
            if !path.to_ascii_lowercase().ends_with(".w3m") {
                continue;
            }
            let raw = set.extract(path).unwrap();
            let bytes: &'static [u8] = Box::leak(raw.into_boxed_slice());
            let m = match Map::parse(bytes) {
                Ok(m) => m,
                Err(_) => continue,
            };
            let t = match m.terrain() {
                Ok(t) => t,
                Err(_) => continue,
            };
            for slot in t.slots() {
                match catalog.blp_path(*slot) {
                    None => {
                        unresolved.insert(*slot);
                    }
                    Some(p) => {
                        if !files.contains(&p.to_ascii_lowercase()) {
                            missing_files.push(format!("{} -> {p}", String::from_utf8_lossy(slot)));
                        }
                    }
                }
            }
        }
        assert!(
            missing_files.is_empty(),
            "{label}: resolved paths missing from archive: {missing_files:?}"
        );
        let names: Vec<String> = unresolved
            .iter()
            .map(|s| String::from_utf8_lossy(s).to_string())
            .collect();
        if label == "1.29b" {
            assert!(unresolved.is_empty(), "1.29b: unresolved slots {names:?}");
        } else {
            // Only the known cliff accent may be unresolved on 1.28c.
            assert_eq!(names, ["CAc2"], "1.28c: unexpected unresolved slots");
        }
    }
}

// ---- full sweep -----------------------------------------------------------

/// Sweep every shipped terrain on both builds: build the mesh, assert closed
/// topology and vertex counts, tally a census, and record failures to
/// `/tmp/opencode/tera-sweep.txt`. Ignored by default because it opens every
/// install.
#[test]
#[ignore = "opens every shipped install; run explicitly"]
fn sweep_every_shipped_terrain() {
    let mut report = String::new();
    let mut failures: Vec<String> = Vec::new();
    let mut total_maps = 0usize;
    let mut total_meshes = 0usize;
    let mut total_tris = 0usize;
    let mut total_verts = 0usize;
    let mut dims: std::collections::BTreeSet<(u32, u32)> = Default::default();
    let mut z_min = f32::INFINITY;
    let mut z_max = f32::NEG_INFINITY;
    let mut clean = 0usize;

    for (label, dir) in [("1.28c", install_dir()), ("1.29b", legacy_dir())] {
        let Some(dir) = dir else {
            let _ = writeln!(report, "{label}: install not present, skipped");
            continue;
        };
        let mut set = match pcx_mpq::MpqSet::open_wc3(&dir) {
            Ok(s) => s,
            Err(e) => {
                failures.push(format!("{label}: open failed: {e}"));
                continue;
            }
        };
        let maps = match shipped_maps(&mut set) {
            Ok(m) => m,
            Err(e) => {
                failures.push(format!("{label}: listfile failed: {e}"));
                continue;
            }
        };
        let mut here_meshes = 0usize;
        for path in &maps {
            if !path.to_ascii_lowercase().ends_with(".w3m") {
                continue;
            }
            total_maps += 1;
            let raw = match set.extract(path) {
                Ok(b) => b,
                Err(e) => {
                    failures.push(format!("{label} {path}: extract: {e}"));
                    continue;
                }
            };
            let bytes: &'static [u8] = Box::leak(raw.into_boxed_slice());
            let m = match Map::parse(bytes) {
                Ok(m) => m,
                Err(e) => {
                    failures.push(format!("{label} {path}: header: {e}"));
                    continue;
                }
            };
            let t = match m.terrain() {
                Ok(t) => t,
                Err(e) => {
                    failures.push(format!("{label} {path}: terrain: {e}"));
                    continue;
                }
            };
            let mesh = TerrainMesh::build(&t);

            let want_v = (t.width() as usize + 1) * (t.height() as usize + 1);
            if mesh.vertex_count() != want_v {
                failures.push(format!("{label} {path}: vertex count {want_v}"));
            }
            if mesh.triangle_count() != 2 * t.width() as usize * t.height() as usize {
                failures.push(format!("{label} {path}: triangle count"));
            }
            let r = mesh.manifold();
            if !r.is_clean() {
                failures.push(format!("{label} {path}: not a clean manifold: {r}"));
            } else {
                clean += 1;
            }
            for p in &mesh.positions {
                z_min = z_min.min(p[2]);
                z_max = z_max.max(p[2]);
            }
            here_meshes += 1;
            total_meshes += 1;
            total_verts += mesh.vertex_count();
            total_tris += mesh.triangle_count();
            dims.insert((t.width(), t.height()));
        }
        let _ = writeln!(report, "{label}: {here_meshes} meshes built");
    }

    let _ = writeln!(report, "total: {total_maps} maps, {total_meshes} meshes");
    let _ = writeln!(
        report,
        "verts {total_verts}, triangles {total_tris}, clean-manifold {clean}/{total_meshes}"
    );
    let _ = writeln!(report, "distinct sizes: {}", dims.len());
    let _ = writeln!(report, "world z range across all maps: [{z_min}, {z_max}]");
    let _ = writeln!(report, "failures: {}", failures.len());
    for f in &failures {
        let _ = writeln!(report, "  {f}");
    }

    let out = std::path::Path::new("/tmp/opencode/tera-sweep.txt");
    if let Some(dir) = out.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::fs::write(out, &report).expect("write sweep report");
    println!("{report}");
    assert!(
        failures.is_empty(),
        "{} failures, see {out:?}",
        failures.len()
    );
}

fn selector_terrain(slots: &[[u8; 4]], corners: &[(u8, u8)]) -> Terrain {
    assert_eq!(corners.len(), 4);
    let mut bytes = pcx_map::TERRAIN_MAGIC.to_vec();
    bytes.extend(pcx_map::TERRAIN_VERSION.to_le_bytes());
    bytes.push(0);
    bytes.extend(0u32.to_le_bytes());
    bytes.extend((slots.len() as u32).to_le_bytes());
    for slot in slots {
        bytes.extend_from_slice(slot);
    }
    bytes.extend(0u32.to_le_bytes());
    bytes.extend(2u32.to_le_bytes());
    bytes.extend(2u32.to_le_bytes());
    bytes.extend(0.0f32.to_le_bytes());
    bytes.extend(0.0f32.to_le_bytes());
    for &(nibble, byte5) in corners {
        bytes.extend(8192u16.to_le_bytes());
        bytes.extend(0u16.to_le_bytes());
        bytes.push(nibble & 0x0F);
        bytes.push(byte5);
        bytes.push(0);
    }
    Terrain::from_bytes(bytes).unwrap()
}

#[test]
fn ground_variation_keeps_the_low_five_bits() {
    let terrain = selector_terrain(&[*b"Ldrt"], &[(0, 0xE3), (0, 0), (0, 0), (0, 0)]);
    let corner = terrain.corner(0, 0).unwrap();
    assert_eq!(crate::ground_variation(corner), 3);
    assert_eq!(crate::ground_variation(terrain.corner(1, 0).unwrap()), 0);
}

#[test]
fn reference_layer_follows_the_sheet_flag() {
    assert_eq!(crate::reference_layer(true, 0), 16);
    assert_eq!(crate::reference_layer(true, 15), 31);
    assert_eq!(crate::reference_layer(true, 16), 15);
    assert_eq!(crate::reference_layer(true, 17), 0);
    assert_eq!(crate::reference_layer(true, 31), 0);
    assert_eq!(crate::reference_layer(false, 0), 0);
    assert_eq!(crate::reference_layer(false, 1), 15);
    assert_eq!(crate::reference_layer(false, 31), 15);
}

#[test]
fn one_slot_uses_the_bottom_left_variation_and_leaves_the_rest_empty() {
    let terrain = selector_terrain(&[*b"Ldrt"], &[(0, 0), (0, 5), (0, 9), (0, 9)]);
    let selectors = crate::tile_texture_selectors(&terrain, &[false], 0, 0).unwrap();
    assert_eq!(
        selectors,
        [
            0,
            crate::EMPTY_SELECTOR,
            crate::EMPTY_SELECTOR,
            crate::EMPTY_SELECTOR
        ]
    );
}

#[test]
fn lowest_slot_uses_the_bottom_left_variation_even_when_that_corner_differs() {
    let terrain = selector_terrain(&[*b"AAAA", *b"BBBB"], &[(1, 1), (0, 5), (1, 9), (0, 9)]);
    let plain = crate::tile_texture_selectors(&terrain, &[false, true], 0, 0).unwrap();
    assert_eq!(plain[0], 15 << 5);
    let extended_lowest = crate::tile_texture_selectors(&terrain, &[true, false], 0, 0).unwrap();
    assert_eq!(extended_lowest[0], 17 << 5);
}

#[test]
fn later_slots_use_the_pinned_corner_mask_order() {
    let terrain = selector_terrain(&[*b"AAAA", *b"BBBB"], &[(1, 0), (0, 0), (1, 0), (0, 0)]);
    let selectors = crate::tile_texture_selectors(&terrain, &[false, false], 0, 0).unwrap();
    assert_eq!(
        selectors,
        [
            0,
            1 + (0b1010 << 5),
            crate::EMPTY_SELECTOR,
            crate::EMPTY_SELECTOR
        ]
    );

    let bottom_right = selector_terrain(&[*b"AAAA", *b"BBBB"], &[(0, 0), (1, 0), (0, 0), (0, 0)]);
    assert_eq!(
        crate::tile_texture_selectors(&bottom_right, &[false, false], 0, 0).unwrap()[1],
        1 + (1 << 5)
    );
    let top_right = selector_terrain(&[*b"AAAA", *b"BBBB"], &[(0, 0), (0, 0), (0, 0), (1, 0)]);
    assert_eq!(
        crate::tile_texture_selectors(&top_right, &[false, false], 0, 0).unwrap()[1],
        1 + (1 << 7)
    );
    let top_left = selector_terrain(&[*b"AAAA", *b"BBBB"], &[(0, 0), (0, 0), (1, 0), (0, 0)]);
    assert_eq!(
        crate::tile_texture_selectors(&top_left, &[false, false], 0, 0).unwrap()[1],
        1 + (1 << 8)
    );
}

#[test]
fn a_nibble_outside_the_slot_list_is_not_clamped() {
    let terrain = selector_terrain(&[*b"Ldrt"], &[(1, 0), (0, 0), (0, 0), (0, 0)]);
    assert_eq!(
        crate::tile_texture_selectors(&terrain, &[false], 0, 0),
        Err(crate::GroundSelectError::SlotRange {
            nibble: 1,
            slot_count: 1
        })
    );
    assert_eq!(
        crate::tile_texture_selectors(&terrain, &[], 0, 0),
        Err(crate::GroundSelectError::ExtendedLen {
            slots: 1,
            extended: 0
        })
    );
    assert!(matches!(
        crate::tile_texture_selectors(&terrain, &[false], 1, 0),
        Err(crate::GroundSelectError::TileRange { tx: 1, ty: 0 })
    ));
    assert_eq!(
        crate::GroundTiles::of(&terrain).blend(&terrain, 0, 0),
        Err(crate::Unsupported::BlendWeights)
    );
}

//! Clip the selection square to the same tile triangles as `pcx_gfx::ground`.
use bevy::{asset::RenderAssetUsages, mesh::PrimitiveTopology, prelude::*};
use pcx_map::Terrain;
use pcx_tera::world::{TILE_XY, corner_world};

/// Rendering-work limit per circle, not an authored size or gameplay rule.
const MAX_TILES: u64 = 256;
/// Two tile triangles, each clipped by four square edges, at most five fan triangles.
const MAX_VERTICES_PER_TILE: usize = 2 * 5 * 3;

pub(super) fn project(
    terrain: &Terrain,
    transform: &Transform,
    z_offset: f32,
) -> Result<Mesh, String> {
    let center = transform.translation;
    let size = transform.scale.x;
    if !transform.is_finite() || size <= 0.0 || !z_offset.is_finite() || z_offset < 0.0 {
        return Err("circle footprint must have finite positive size".into());
    }
    let [cx, cy] = [center.x, -center.z];
    let min = Vec2::new(cx - size / 2.0, cy - size / 2.0);
    let max = Vec2::new(cx + size / 2.0, cy + size / 2.0);
    if !min.is_finite() || !max.is_finite() || min.x >= max.x || min.y >= max.y {
        return Err("circle footprint cannot be represented in world coordinates".into());
    }
    let (ox, oy) = terrain.origin();
    if !ox.is_finite() || !oy.is_finite() {
        return Err("terrain origin is not finite".into());
    }
    let start_x = ((min.x - ox) / TILE_XY)
        .floor()
        .clamp(0.0, terrain.width() as f32) as u32;
    let start_y = ((min.y - oy) / TILE_XY)
        .floor()
        .clamp(0.0, terrain.height() as f32) as u32;
    let end_x = ((max.x - ox) / TILE_XY)
        .ceil()
        .clamp(0.0, terrain.width() as f32) as u32;
    let end_y = ((max.y - oy) / TILE_XY)
        .ceil()
        .clamp(0.0, terrain.height() as f32) as u32;
    let tiles = u64::from(end_x - start_x) * u64::from(end_y - start_y);
    if tiles == 0 || tiles > MAX_TILES {
        return Err(format!(
            "circle footprint covers {tiles} terrain tiles; supported range is 1..={MAX_TILES}"
        ));
    }
    let capacity = tiles as usize * MAX_VERTICES_PER_TILE;
    let mut positions = Vec::with_capacity(capacity);
    let mut normals = Vec::with_capacity(capacity);
    let mut uvs = Vec::with_capacity(capacity);
    for ty in start_y..end_y {
        for tx in start_x..end_x {
            let corners = [(tx, ty), (tx + 1, ty), (tx, ty + 1), (tx + 1, ty + 1)]
                .map(|(x, y)| corner_world(terrain, x, y).map(Vec3::from_array));
            let [Some(a), Some(b), Some(c), Some(d)] = corners else {
                return Err(format!("missing terrain corner at tile {tx},{ty}"));
            };
            if ![a, b, c, d].into_iter().all(Vec3::is_finite) {
                return Err(format!("non-finite terrain corner at tile {tx},{ty}"));
            }
            // Ground uses indices [00,10,11] and [00,11,01], including non-coplanar tiles.
            for triangle in [[a, b, d], [a, d, c]] {
                let mut polygon = triangle.to_vec();
                for (axis, bound, greater) in [
                    (0, min.x, true),
                    (0, max.x, false),
                    (1, min.y, true),
                    (1, max.y, false),
                ] {
                    polygon = clip(&polygon, axis, bound, greater);
                }
                for index in 1..polygon.len().saturating_sub(1) {
                    let world = [polygon[0], polygon[index], polygon[index + 1]];
                    let local = world.map(|p| {
                        Vec3::new(
                            (p.x - cx) / size,
                            p.z + z_offset - center.y,
                            -(p.y - cy) / size,
                        )
                    });
                    let Some(normal) = (local[1] - local[0])
                        .cross(local[2] - local[0])
                        .try_normalize()
                    else {
                        continue;
                    };
                    for (p, position) in world.into_iter().zip(local) {
                        positions.push(position.to_array());
                        normals.push(normal.to_array());
                        // Preserve Plane3d UV orientation: V increases along Bevy +Z (world -Y).
                        uvs.push([(p.x - min.x) / size, (max.y - p.y) / size]);
                    }
                }
            }
        }
    }
    if positions.is_empty() {
        return Err("circle footprint has no non-degenerate terrain triangles".into());
    }
    Ok(Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs))
}

fn clip(polygon: &[Vec3], axis: usize, bound: f32, greater: bool) -> Vec<Vec3> {
    let mut output = Vec::with_capacity(polygon.len() + 1);
    let Some(mut previous) = polygon.last().copied() else {
        return output;
    };
    let inside = |p: Vec3| {
        if greater {
            p[axis] >= bound
        } else {
            p[axis] <= bound
        }
    };
    for &current in polygon {
        if inside(previous) != inside(current) {
            let fraction = (bound - previous[axis]) / (current[axis] - previous[axis]);
            let mut intersection = previous.lerp(current, fraction);
            intersection[axis] = bound;
            output.push(intersection);
        }
        if inside(current) {
            output.push(current);
        }
        previous = current;
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;

    fn terrain(width: u32, height: u32, heights: &[u16]) -> Terrain {
        assert_eq!(heights.len(), ((width + 1) * (height + 1)) as usize);
        let mut raw = b"W3E!".to_vec();
        raw.extend(11u32.to_le_bytes());
        raw.push(0);
        raw.extend(0u32.to_le_bytes());
        raw.extend(1u32.to_le_bytes());
        raw.extend(b"Ldrt");
        raw.extend(1u32.to_le_bytes());
        raw.extend(b"CLdi");
        raw.extend((width + 1).to_le_bytes());
        raw.extend((height + 1).to_le_bytes());
        raw.extend(32f32.to_le_bytes());
        raw.extend((-48f32).to_le_bytes());
        for height in heights {
            raw.extend(height.to_le_bytes());
            raw.extend([0; 5]);
        }
        Terrain::from_bytes(raw).unwrap()
    }

    fn triangle_height(terrain: &Terrain, x: f32, y: f32) -> f32 {
        let (ox, oy) = terrain.origin();
        let fx = (x - ox) / TILE_XY;
        let fy = (y - oy) / TILE_XY;
        let tx = (fx.floor() as u32).min(terrain.width() - 1);
        let ty = (fy.floor() as u32).min(terrain.height() - 1);
        let [dx, dy] = [fx - tx as f32, fy - ty as f32];
        let [a, b, c, d] = [(tx, ty), (tx + 1, ty), (tx, ty + 1), (tx + 1, ty + 1)]
            .map(|(x, y)| corner_world(terrain, x, y).unwrap()[2]);
        if dx >= dy {
            a * (1.0 - dx) + b * (dx - dy) + d * dy
        } else {
            a * (1.0 - dy) + d * dx + c * (dy - dx)
        }
    }

    #[test]
    fn clipped_triangles_match_non_coplanar_rendered_ground_and_keep_footprint_uvs() {
        let terrain = terrain(
            2,
            2,
            &[8192, 8352, 8224, 8256, 8192, 8288, 8320, 8224, 8384],
        );
        let transform =
            Transform::from_xyz(176.0, 7.0, -64.0).with_scale(Vec3::new(176.0, 1.0, 176.0));
        let mesh = project(&terrain, &transform, 0.0).unwrap();
        let positions = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .unwrap()
            .as_float3()
            .unwrap();
        let VertexAttributeValues::Float32x2(uvs) = mesh.attribute(Mesh::ATTRIBUTE_UV_0).unwrap()
        else {
            panic!("UV format");
        };
        let mut area = 0.0;
        for triangle in positions.as_chunks::<3>().0 {
            let world = triangle.map(|p| transform.transform_point(Vec3::from_array(p)));
            for p in world
                .into_iter()
                .chain([world.into_iter().sum::<Vec3>() / 3.0])
            {
                assert!(
                    (p.y - triangle_height(&terrain, p.x, -p.z)).abs() < 0.001,
                    "vertex/centroid {p:?} must lie on its rendered terrain triangle"
                );
            }
            area += (world[1] - world[0]).cross(world[2] - world[0]).y / 2.0;
        }
        assert!(
            (area - transform.scale.x.powi(2)).abs() < 0.01,
            "no overlapping triangles or holes: {area}"
        );
        let mut minimum = Vec2::ONE;
        let mut maximum = Vec2::ZERO;
        for (position, uv) in positions.iter().zip(uvs) {
            let p = Vec3::from_array(*position);
            let uv = Vec2::from_array(*uv);
            assert!((uv - Vec2::new(p.x + 0.5, p.z + 0.5)).length() < 0.00001);
            minimum = minimum.min(uv);
            maximum = maximum.max(uv);
        }
        assert_eq!(minimum, Vec2::ZERO);
        assert_eq!(maximum, Vec2::ONE);
        // Bilinear sampling differs on this tile; matching it would put geometry off the mesh.
        assert_ne!(
            triangle_height(&terrain, 96.0, 16.0),
            pcx_tera::world::sample_world_z(&terrain, 96.0, 16.0).unwrap()
        );
    }

    #[test]
    fn map_edge_clips_geometry_without_stretching_the_source_image() {
        let terrain = terrain(1, 1, &[8192; 4]);
        let transform =
            Transform::from_xyz(32.0, 0.0, 48.0).with_scale(Vec3::new(128.0, 1.0, 128.0));
        let mesh = project(&terrain, &transform, 0.0).unwrap();
        let positions = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .unwrap()
            .as_float3()
            .unwrap();
        let VertexAttributeValues::Float32x2(uvs) = mesh.attribute(Mesh::ATTRIBUTE_UV_0).unwrap()
        else {
            panic!("UV format");
        };
        for (position, uv) in positions.iter().zip(uvs) {
            assert!(position[0] >= 0.0 && position[2] <= 0.0);
            assert!(uv[0] >= 0.5 && uv[1] <= 0.5);
            assert_eq!(position[1], 0.0);
        }
    }

    #[test]
    fn admitted_offset_changes_height_without_changing_footprint_or_uvs() {
        let terrain = terrain(1, 1, &[8192, 8256, 8288, 8224]);
        let transform =
            Transform::from_xyz(96.0, 15.0, -16.0).with_scale(Vec3::new(72.0, 1.0, 72.0));
        let base = project(&terrain, &transform, 0.0).unwrap();
        for offset in [0.375, 17.25] {
            let raised = project(&terrain, &transform, offset).unwrap();
            assert_eq!(
                base.attribute(Mesh::ATTRIBUTE_UV_0),
                raised.attribute(Mesh::ATTRIBUTE_UV_0)
            );
            let Some(VertexAttributeValues::Float32x3(a)) =
                base.attribute(Mesh::ATTRIBUTE_POSITION)
            else {
                panic!("positions")
            };
            let Some(VertexAttributeValues::Float32x3(b)) =
                raised.attribute(Mesh::ATTRIBUTE_POSITION)
            else {
                panic!("positions")
            };
            for (a, b) in a.iter().zip(b) {
                assert_eq!([a[0], a[2]], [b[0], b[2]]);
                assert!((b[1] - a[1] - offset).abs() < 0.0001);
            }
        }
        for offset in [f32::NAN, f32::INFINITY, -0.5] {
            assert!(project(&terrain, &transform, offset).is_err());
        }
    }

    #[test]
    fn invalid_sizes_outside_footprints_and_tile_budget_refuse_before_geometry() {
        let terrain = terrain(17, 17, &[8192; 18 * 18]);
        for size in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert!(
                project(
                    &terrain,
                    &Transform::default().with_scale(Vec3::new(size, 1.0, size)),
                    0.0,
                )
                .is_err()
            );
        }
        let bounded =
            Transform::from_xyz(1056.0, 0.0, -976.0).with_scale(Vec3::new(2048.0, 1.0, 2048.0));
        let mesh = project(&terrain, &bounded, 0.0).unwrap();
        assert_eq!(mesh.count_vertices(), MAX_TILES as usize * 2 * 3);
        let outside = Transform::from_xyz(-1024.0, 0.0, 1024.0).with_scale(Vec3::splat(32.0));
        assert!(
            project(&terrain, &outside, 0.0)
                .unwrap_err()
                .contains("covers 0 terrain tiles")
        );
        let huge =
            Transform::from_xyz(1024.0, 0.0, -1024.0).with_scale(Vec3::new(4096.0, 1.0, 4096.0));
        assert!(
            project(&terrain, &huge, 0.0)
                .unwrap_err()
                .contains("covers 289 terrain tiles")
        );
        let tiny = Transform::from_xyz(1024.0, 0.0, -1024.0).with_scale(Vec3::new(
            f32::MIN_POSITIVE,
            1.0,
            f32::MIN_POSITIVE,
        ));
        assert!(project(&terrain, &tiny, 0.0).is_err());
    }
}

use pcx_gfx::viewer::{terrain_plane_mesh, terrain_tile_path_from_slk};

fn synth_slk() -> String {
    let mut text = String::from("ID;PWXL;N;E\n");
    text.push_str("C;X1;Y1;K\"tileID\"\n");
    text.push_str("C;X2;Y1;K\"dir\"\n");
    text.push_str("C;X3;Y1;K\"file\"\n");
    text.push_str("C;X1;Y2;K\"Aaaa\"\n");
    text.push_str("C;X2;Y2;K\"Synthetic\\FirstSet\"\n");
    text.push_str("C;X3;Y2;K\"early\"\n");
    text.push_str("C;X1;Y3;K\"Zzzz\"\n");
    text.push_str("C;X2;Y3;K\"Synthetic\\ChosenSet\"\n");
    text.push_str("C;X3;Y3;K\"chosen\"\n");
    text.push_str("E\n");
    text.replace('\n', "\r\n")
}

#[test]
fn terrain_tile_path_is_derived_from_terrain_slk() {
    let text = synth_slk();
    let path = terrain_tile_path_from_slk(text.as_bytes(), *b"Zzzz").expect("path");
    assert_eq!(path, "Synthetic\\ChosenSet\\chosen.blp");
}

#[test]
fn terrain_tile_path_rejects_missing_columns() {
    let text = "ID;PWXL;N;E\nE\n".replace('\n', "\r\n");
    assert!(terrain_tile_path_from_slk(text.as_bytes(), *b"Zzzz").is_err());
}

#[test]
fn terrain_plane_mesh_has_expected_geometry() {
    use bevy::mesh::{Indices, PrimitiveTopology};
    let mesh =
        terrain_plane_mesh([-7.0, -11.0, 13.0], [17.0, 19.0, 23.0], "synthetic.mdx").unwrap();
    assert_eq!(
        mesh.attribute(bevy::mesh::Mesh::ATTRIBUTE_POSITION)
            .unwrap()
            .as_float3()
            .unwrap(),
        &[
            [-7.0, 19.0, 13.0],
            [17.0, 19.0, 13.0],
            [17.0, -11.0, 13.0],
            [-7.0, -11.0, 13.0]
        ]
    );
    let positions = mesh
        .attribute(bevy::mesh::Mesh::ATTRIBUTE_POSITION)
        .unwrap()
        .as_float3()
        .unwrap();
    let normals = mesh
        .attribute(bevy::mesh::Mesh::ATTRIBUTE_NORMAL)
        .unwrap()
        .as_float3()
        .unwrap();
    let indices = mesh.indices().unwrap().iter().collect::<Vec<_>>();
    let (triangles, remainder) = indices.as_chunks::<3>();
    assert!(remainder.is_empty());
    for triangle in triangles {
        let a = bevy::math::Vec3::from_array(positions[triangle[0]]);
        let b = bevy::math::Vec3::from_array(positions[triangle[1]]);
        let c = bevy::math::Vec3::from_array(positions[triangle[2]]);
        let facing = (b - a).cross(c - a);
        for index in triangle {
            assert!(
                facing.dot(bevy::math::Vec3::from_array(normals[*index])) > 0.0,
                "preview triangle winding must agree with its vertex normals"
            );
        }
    }
    assert_eq!(mesh.primitive_topology(), PrimitiveTopology::TriangleList);
    assert_eq!(
        mesh.attribute(bevy::mesh::Mesh::ATTRIBUTE_POSITION)
            .unwrap()
            .len(),
        4
    );
    assert_eq!(
        mesh.attribute(bevy::mesh::Mesh::ATTRIBUTE_UV_0)
            .unwrap()
            .len(),
        4
    );
    match mesh.indices().unwrap() {
        Indices::U16(v) => assert_eq!(v.len(), 6),
        _ => panic!("expected u16 indices"),
    }
}

#[test]
fn requested_missing_install_returns_typed_error_before_window_creation() {
    let install = std::path::Path::new(".scratch/viewer-terrain-tests/synthetic-absent-install");
    assert!(!install.exists());
    let error = pcx_gfx::viewer::run_with_terrain(install, "synthetic.mdx", Some(*b"Zzzz"))
        .expect_err("missing requested terrain must stop before opening a window");
    assert!(matches!(error, pcx_gfx::Error::Terrain(error)
        if error.tile_id == *b"Zzzz"
        && matches!(error.kind, pcx_gfx::viewer::TerrainTileErrorKind::Read(_))));
}

#[test]
fn terrain_preview_refuses_nonfinite_inverted_and_zero_area_bounds() {
    for axis in 0..3 {
        for value in [f32::NAN, f32::NEG_INFINITY, f32::INFINITY] {
            for lower in [true, false] {
                let mut lo = [0.0; 3];
                let mut hi = [1.0; 3];
                if lower {
                    lo[axis] = value;
                } else {
                    hi[axis] = value;
                }
                assert!(matches!(terrain_plane_mesh(lo, hi, "bounds.mdx"),
                    Err(pcx_gfx::Error::TerrainBounds { source_name, axis: bad_axis, .. })
                    if source_name == "bounds.mdx" && bad_axis == axis));
            }
        }
        let mut lo = [0.0; 3];
        lo[axis] = 2.0;
        assert!(matches!(terrain_plane_mesh(lo, [1.0; 3], "inverted.mdx"),
            Err(pcx_gfx::Error::TerrainBounds { axis: bad_axis, .. }) if bad_axis == axis));
    }
    for axis in 0..2 {
        let lo = [0.0; 3];
        let mut hi = [1.0; 3];
        hi[axis] = 0.0;
        assert!(terrain_plane_mesh(lo, hi, "degenerate.mdx").is_err());
    }
    assert!(terrain_plane_mesh([0.0; 3], [1.0, 1.0, 0.0], "flat.mdx").is_ok());
}

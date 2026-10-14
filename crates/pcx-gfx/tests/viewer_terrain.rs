use pcx_gfx::viewer::{terrain_plane_mesh, terrain_tile_path_from_slk};

fn synth_slk() -> String {
    let mut text = String::from("ID;PWXL;N;E\n");
    text.push_str("C;X1;Y1;K\"tileID\"\n");
    text.push_str("C;X2;Y1;K\"dir\"\n");
    text.push_str("C;X3;Y1;K\"file\"\n");
    text.push_str("C;X1;Y2;K\"Aaaa\"\n");
    text.push_str("C;X2;Y2;K\"TerrainArt\\Ashenvale\"\n");
    text.push_str("C;X3;Y2;K\"Ashen_DirtGrass\"\n");
    text.push_str("C;X1;Y3;K\"Ldrt\"\n");
    text.push_str("C;X2;Y3;K\"TerrainArt\\Lordaeron\"\n");
    text.push_str("C;X3;Y3;K\"Lords_Grass\"\n");
    text.push_str("E\n");
    text.replace('\n', "\r\n")
}

#[test]
fn terrain_tile_path_is_derived_from_terrain_slk() {
    let text = synth_slk();
    let path = terrain_tile_path_from_slk(text.as_bytes()).expect("path");
    assert_eq!(path, "TerrainArt\\Ashenvale\\Ashen_DirtGrass.blp");
}

#[test]
fn terrain_tile_path_rejects_missing_columns() {
    let text = "ID;PWXL;N;E\nE\n".replace('\n', "\r\n");
    assert!(terrain_tile_path_from_slk(text.as_bytes()).is_err());
}

#[test]
fn terrain_plane_mesh_has_expected_geometry() {
    use bevy::mesh::{Indices, PrimitiveTopology};
    let mesh = terrain_plane_mesh();
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

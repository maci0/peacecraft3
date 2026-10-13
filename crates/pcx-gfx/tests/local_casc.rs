//! Explicit named-path import gate; original content stays in memory.
use bevy::mesh::Mesh;

#[test]
#[ignore = "requires PEACECRAFT3_REFORGED_DIR and PEACECRAFT3_REFORGED_ASSET"]
fn a_named_casc_model_uses_the_existing_graphics_importer() {
    let install =
        std::env::var_os("PEACECRAFT3_REFORGED_DIR").expect("an original CASC install is required");
    let path = std::env::var("PEACECRAFT3_REFORGED_ASSET")
        .expect("an explicit stored TVFS path is required; no module/HD default is guessed");
    let storage = pcx_casc::storage::Storage::open(
        std::path::Path::new(&install),
        pcx_casc::blte::Limits {
            encoded_bytes: 64 * 1024 * 1024,
            decoded_bytes: 256 * 1024 * 1024,
        },
    )
    .expect("checked local storage");
    let encoding = storage.load_encoding().expect("verified encoding manifest");
    let resolver = storage
        .load_tvfs(&encoding)
        .expect("verified TVFS manifests");
    let asset = storage
        .read_path(&resolver, &path)
        .expect("named asset identity and content hashes");
    let model = pcx_gfx::load_model(&asset.file.decoded.bytes, &path)
        .unwrap_or_else(|error| panic!("{path}: graphics import failed: {error}"));
    assert!(
        !model.meshes.is_empty(),
        "the named model must provide geometry"
    );
    let mut vertices = 0;
    for rest in &model.meshes {
        let mesh = pcx_gfx::to_bevy_mesh(rest);
        let positions = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .expect("render mesh positions");
        assert_eq!(positions.len(), rest.positions.len());
        assert!(!rest.positions.is_empty());
        vertices += rest.positions.len();
    }
    println!(
        "source={path} version={} bytes={} ckey={} ekey={} geosets={} vertices={vertices}",
        storage.build.info.source.version,
        asset.file.decoded.bytes.len(),
        pcx_casc::hex(&asset.resolved.ckey),
        pcx_casc::hex(&asset.resolved.ekey),
        model.meshes.len()
    );
}

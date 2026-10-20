//! Explicit original-install gate: the install identity and the geoset index bounds.
use pcx_mdl::Mdlx;
use pcx_mpq::MpqSet;

#[test]
#[ignore = "requires PEACECRAFT3_FORMAT_INSTALL pointing to a local original install"]
fn original_geoset_index_bounds_and_footman_presence() {
    let install = std::env::var_os("PEACECRAFT3_FORMAT_INSTALL")
        .expect("set PEACECRAFT3_FORMAT_INSTALL to the original install");
    let mut archives = MpqSet::open_wc3(&install).expect("open original install");
    let paths = archives.file_list().expect("read original listfiles");
    assert!(
        paths
            .iter()
            .any(|p| p.eq_ignore_ascii_case("units\\human\\footman\\footman.mdx")),
        "install lacks the Footman model; not the expected Warcraft III archive set"
    );
    let mut models = 0usize;
    for path in paths
        .iter()
        .filter(|path| path.to_ascii_lowercase().ends_with(".mdx"))
    {
        let data = archives
            .extract(path)
            .unwrap_or_else(|error| panic!("{path}: {error}"));
        let model = Mdlx::parse(&data).unwrap_or_else(|error| panic!("{path}: {error}"));
        let Ok(geosets) = model.geosets(&data) else {
            continue;
        };
        models += 1;
        for (index, source) in geosets.iter().enumerate() {
            let mesh = model
                .mesh(&data, source)
                .unwrap_or_else(|error| panic!("{path} geoset {index}: {error}"));
            let vertices = mesh.vertices.count as usize;
            assert!(
                mesh.triangles
                    .raw
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .all(|c| usize::from(u16::from_le_bytes([c[0], c[1]])) < vertices),
                "{path} geoset {index}: triangle index outside its vertex array"
            );
        }
    }
    assert!(models > 0, "no original models read");
}

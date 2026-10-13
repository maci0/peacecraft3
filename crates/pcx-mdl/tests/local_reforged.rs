//! Explicit CASC fixture; no original model bytes are stored in the repository.
use pcx_casc::{blte::Limits, storage::Storage};
use pcx_mdl::{Error, Mdlx, tag};

#[test]
#[ignore = "requires PEACECRAFT3_REFORGED_DIR and PEACECRAFT3_REFORGED_ASSET from the authored TVFS namespace"]
fn original_casc_geosets_preserve_v1800_lod_and_classic_geometry() {
    let install = std::env::var_os("PEACECRAFT3_REFORGED_DIR")
        .expect("set PEACECRAFT3_REFORGED_DIR to the original CASC install");
    let path = std::env::var("PEACECRAFT3_REFORGED_ASSET")
        .expect("set PEACECRAFT3_REFORGED_ASSET to the authored qualified TVFS path");
    let storage = Storage::open(
        std::path::Path::new(&install),
        Limits {
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
        .expect("verified named original content");
    let bytes = &asset.file.decoded.bytes;
    assert_eq!(asset.file.decoded.content_key, asset.resolved.ckey);
    assert_eq!(bytes.len() as u64, asset.resolved.decoded_size);
    let model = Mdlx::parse(bytes).expect("original MDLX container");
    assert_eq!(
        model.version, 1800,
        "this fixture must exercise the newer trailer"
    );
    let geosets = model.geosets(bytes).expect("inclusive GEOS framing");
    assert!(!geosets.is_empty());
    assert_eq!(
        geosets.iter().map(|g| g.size as usize).sum::<usize>(),
        model.payload(bytes, tag::GEOS).unwrap().len()
    );
    let mut vertices = 0;
    for (i, geoset) in geosets.iter().enumerate() {
        let mesh = model
            .mesh(bytes, geoset)
            .unwrap_or_else(|error| panic!("{path} geoset {i}: {error}"));
        let lod = mesh.lod.as_ref().expect("authored v1800 LoD field");
        assert_eq!(lod.version, model.version);
        let raw = &geoset.data[lod.offset..lod.offset + 84];
        assert_eq!(lod.level, i32::from_le_bytes(raw[..4].try_into().unwrap()));
        assert_eq!(lod.name.as_slice(), &raw[4..]);
        assert!(mesh.vertices.count > 0);
        assert_eq!(mesh.vertices.count, mesh.normals.count);
        assert_eq!(mesh.vertices.count, mesh.tex_coords.count);
        assert_eq!(mesh.vertices.count as usize, mesh.vertex_groups.len());
        assert_eq!(mesh.matrix_span_u32s, mesh.matrix_indices.len());
        assert_eq!(mesh.seq_bounds.len(), model.sequences(bytes).unwrap().len());
        vertices += mesh.vertices.count as usize;
        println!(
            "geoset={i} vertices={} lod_version={} lod_rel={:#x} lod_file={:#x} name_bytes={} seq_bounds={}",
            mesh.vertices.count,
            lod.version,
            lod.offset,
            model.chunk(tag::GEOS).unwrap().payload_offset + geoset.offset + 4 + lod.offset,
            lod.name.len(),
            mesh.seq_bounds.len()
        );
    }
    let textures = model
        .textures(bytes)
        .expect("TEXS records use the verified common layout");
    assert!(!textures.is_empty());
    assert_eq!(
        textures.len() * pcx_mdl::TEXTURE_SIZE,
        model.payload(bytes, tag::TEXS).unwrap().len()
    );
    assert!(matches!(
        model.materials(bytes),
        Err(Error::UnsupportedVersionedChunk {
            version: 1800,
            chunk: "MTLS"
        })
    ));
    println!(
        "source={path} build={} mdx_version={} bytes={} ckey={} ekey={} geosets={} vertices={vertices} textures={} materials=typed-refusal TANG/SKIN=absent",
        storage.build.info.source.version,
        model.version,
        bytes.len(),
        pcx_casc::hex(&asset.resolved.ckey),
        pcx_casc::hex(&asset.resolved.ekey),
        geosets.len(),
        textures.len()
    );
}

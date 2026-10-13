//! Synthetic records discriminate the versioned trailer without original bytes.
use pcx_mdl::{Error, Geoset, Mdlx};

fn word(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend(value.to_le_bytes());
}

fn floats(bytes: &mut Vec<u8>, values: &[f32]) {
    for value in values {
        bytes.extend(value.to_le_bytes());
    }
}

fn block(bytes: &mut Vec<u8>, tag: &[u8; 4], count: u32, body: &[u8]) {
    bytes.extend(tag);
    word(bytes, count);
    bytes.extend(body);
}

fn fixture(version: u32, extension: Option<&[u8; 4]>) -> (Mdlx, Geoset, usize) {
    let mut data = Vec::new();
    let mut vector = Vec::new();
    floats(&mut vector, &[1.0, 2.0, 3.0]);
    block(&mut data, b"VRTX", 1, &vector);
    block(&mut data, b"NRMS", 1, &vector);
    block(&mut data, b"PTYP", 1, &4u32.to_le_bytes());
    block(&mut data, b"PCNT", 1, &3u32.to_le_bytes());
    block(&mut data, b"PVTX", 3, &[0; 6]);
    block(&mut data, b"GNDX", 1, &[0]);
    block(&mut data, b"MTGC", 1, &1u32.to_le_bytes());
    block(&mut data, b"MATS", 1, &9u32.to_le_bytes());
    let trailer = data.len();
    for value in [7, 11, 4] {
        word(&mut data, value);
    }
    if version == 1800 {
        data.extend((-7i32).to_le_bytes());
        // Non-zero, non-UTF8 and not NUL-terminated: neither string scanning
        // nor skipping zero padding can locate the following bounds correctly.
        let mut name = [0xa5; 80];
        name[..4].copy_from_slice(b"UVAS");
        data.extend(name);
    }
    floats(&mut data, &[6.5, -3.0, -2.0, -1.0, 8.0, 9.0, 10.0]);
    word(&mut data, 2);
    floats(&mut data, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0]);
    floats(&mut data, &[8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0]);
    if let Some(tag) = extension {
        if tag == b"SKIN" {
            block(&mut data, tag, 8, &[0; 8]);
        } else {
            block(&mut data, tag, 1, &[0; 16]);
        }
    }
    block(&mut data, b"UVAS", 1, &[]);
    let mut uv = Vec::new();
    floats(&mut uv, &[0.25, 0.75]);
    block(&mut data, b"UVBS", 1, &uv);
    let geoset = Geoset {
        offset: 0,
        size: data.len() as u32 + 4,
        data,
    };
    (
        Mdlx {
            version,
            chunks: Vec::new(),
        },
        geoset,
        trailer,
    )
}

#[test]
fn v1800_lod_precedes_bounds_and_preserves_all_name_bytes() {
    let (model, geoset, trailer) = fixture(1800, None);
    let mesh = model.mesh(&[], &geoset).unwrap();
    let lod = mesh.lod.as_ref().unwrap();
    assert_eq!(lod.version, model.version);
    assert_eq!(lod.offset, trailer + 12);
    assert_eq!(lod.level, -7);
    assert_eq!(&lod.name[..4], b"UVAS");
    assert!(lod.name[4..].iter().all(|byte| *byte == 0xa5));
    assert_eq!(mesh.material_id, 7);
    assert_eq!(mesh.selection_group, 11);
    assert_eq!(mesh.unselectable, 4);
    assert_eq!(mesh.bounds_radius, 6.5);
    assert_eq!(mesh.minimum_extent, [-3.0, -2.0, -1.0]);
    assert_eq!(mesh.maximum_extent, [8.0, 9.0, 10.0]);
    assert_eq!(
        mesh.seq_bounds,
        vec![
            [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0],
            [8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0]
        ]
    );
    assert_eq!(mesh.tex_coords.get(0), Some([0.25, 0.75]));
    assert_eq!(mesh.matrix_indices, vec![9]);

    // This exact fixture fails the old layout; an unconditional classic walk
    // or a scan to the embedded UVAS cannot satisfy the assertions above.
    let old = Mdlx {
        version: 800,
        chunks: Vec::new(),
    };
    assert!(old.mesh(&[], &geoset).is_err());
}

#[test]
fn classic_trailer_has_no_lod_insert_and_the_same_fields() {
    let (model, geoset, _) = fixture(800, None);
    let classic = model.mesh(&[], &geoset).unwrap();
    let (new_model, new_geoset, _) = fixture(1800, None);
    let mut newer = new_model.mesh(&[], &new_geoset).unwrap();
    assert!(classic.lod.is_none());
    newer.lod = None;
    assert_eq!(classic, newer);
}

#[test]
fn truncated_lod_is_a_typed_failure() {
    let (model, mut geoset, trailer) = fixture(1800, None);
    geoset.data.truncate(trailer + 12 + 83);
    assert!(
        matches!(model.mesh(&[], &geoset), Err(Error::Truncated { what: "MATS LoD", at, need: 84, have: 83 }) if at == trailer + 12)
    );
}

#[test]
fn hd_streams_are_explicit_refusals_and_classic_errors_stay_classic() {
    for tag in [b"TANG", b"SKIN"] {
        let (model, geoset, trailer) = fixture(1800, Some(tag));
        assert!(
            matches!(model.mesh(&[], &geoset), Err(Error::UnsupportedGeosetFeature { version: 1800, at, tag: got }) if got == *tag && at == trailer + 84 + 44 + 2 * 28)
        );
        let (classic, geoset, _) = fixture(800, Some(tag));
        assert!(
            matches!(classic.mesh(&[], &geoset), Err(Error::UnknownGeosetTag(got)) if got == *tag)
        );
    }
}

#[test]
fn unknown_mesh_versions_and_new_materials_are_not_assumed_classic() {
    let (mut model, geoset, _) = fixture(1800, None);
    model.version = 1900;
    assert!(matches!(
        model.mesh(&[], &geoset),
        Err(Error::UnsupportedVersion(1900))
    ));
    let mut bytes = b"MDLXVERS".to_vec();
    word(&mut bytes, 4);
    word(&mut bytes, 1800);
    block(&mut bytes, b"MTLS", 0, &[]);
    let model = Mdlx::parse(&bytes).unwrap();
    assert!(matches!(
        model.materials(&bytes),
        Err(Error::UnsupportedVersionedChunk {
            version: 1800,
            chunk: "MTLS"
        })
    ));
}

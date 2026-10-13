//! Discriminating stored-HD fixtures contain synthetic words only.
use pcx_mdl::{Error, Geoset, Mdlx};

fn word(bytes: &mut Vec<u8>, word: u32) {
    bytes.extend(word.to_le_bytes());
}
fn block(bytes: &mut Vec<u8>, tag: &[u8; 4], count: u32, body: &[u8]) {
    bytes.extend(tag);
    word(bytes, count);
    bytes.extend(body);
}

fn geoset() -> (Geoset, usize, usize) {
    let mut data = Vec::new();
    block(&mut data, b"VRTX", 1, &[0; 12]);
    block(&mut data, b"NRMS", 1, &[0; 12]);
    block(&mut data, b"PTYP", 1, &4u32.to_le_bytes());
    block(&mut data, b"PCNT", 1, &3u32.to_le_bytes());
    block(&mut data, b"PVTX", 3, &[0; 6]);
    block(&mut data, b"GNDX", 0, &[]);
    block(&mut data, b"MTGC", 0, &[]);
    block(&mut data, b"MATS", 0, &[]);
    data.extend([0; 128]);
    let tangent = data.len();
    let mut raw = Vec::new();
    for bits in [0x8000_0000, 0x7fc0_1234, 0x3f80_0000, 0xbf80_0000] {
        word(&mut raw, bits);
    }
    block(&mut data, b"TANG", 1, &raw);
    let skin = data.len();
    let mut raw = Vec::new();
    // Wider than u8 and deliberately not normalized/clamped to 255.
    for value in [513u16, 1027, 4099, 65535, 7, 1024, 32768, 65535] {
        raw.extend(value.to_le_bytes());
    }
    block(&mut data, b"SKIN", 8, &raw);
    block(&mut data, b"UVAS", 2, &[]);
    block(&mut data, b"UVBS", 1, &[0; 8]);
    block(&mut data, b"UVBS", 1, &[0; 8]);
    (
        Geoset {
            offset: 0,
            size: data.len() as u32 + 4,
            data,
        },
        tangent,
        skin,
    )
}

fn container_model(version: u32) -> Mdlx {
    Mdlx {
        version,
        chunks: Vec::new(),
    }
}

#[test]
fn v1800_words_and_tangent_bits_are_preserved_and_classic_import_still_refuses() {
    let (geoset, tangent, skin) = geoset();
    let model = container_model(1800);
    assert!(
        matches!(model.mesh(&[],&geoset),Err(Error::UnsupportedGeosetFeature {version:1800,at,tag}) if at==tangent && &tag==b"TANG")
    );
    let streams = model.geoset_streams_v1800(&geoset).unwrap();
    assert_eq!(streams.vertex_count, 1);
    assert_eq!(streams.version, model.version);
    let tangents = streams.tangents.unwrap();
    assert_eq!(tangents.offset, tangent);
    assert_eq!(tangents.count, 1);
    assert_eq!(
        tangents.bits(0),
        Some([0x8000_0000, 0x7fc0_1234, 0x3f80_0000, 0xbf80_0000])
    );
    assert!(tangents.bits(1).is_none());
    assert!(tangents.bits(usize::MAX).is_none());
    let weights = streams.skin.unwrap();
    assert_eq!(weights.offset, skin);
    assert_eq!(weights.word_count, 8);
    assert_eq!(weights.record_count(), 1);
    let record = weights.get(0).unwrap();
    assert_eq!(record.joint_words, [513, 1027, 4099, 65535]);
    assert_eq!(record.weight_words, [7, 1024, 32768, 65535]);
    assert!(weights.get(1).is_none());
    assert!(weights.get(usize::MAX).is_none());
    let mut reconstructed = Vec::new();
    for b in streams.blocks {
        reconstructed.extend(b.tag);
        word(&mut reconstructed, b.count);
        reconstructed.extend(b.body);
        reconstructed.extend(b.trailer);
    }
    assert_eq!(reconstructed, geoset.data);
}

#[test]
fn narrow_skin_and_wrong_counts_do_not_resynchronize_into_uvs() {
    let (source, _, skin) = geoset();
    let model = container_model(1800);
    let mut narrow = source.clone();
    narrow.data.drain(skin + 8 + 8..skin + 8 + 16);
    assert!(model.geoset_streams_v1800(&narrow).is_err());
    for count in [4u32, 7, 9, u32::MAX] {
        let mut bad = source.clone();
        bad.data[skin + 4..skin + 8].copy_from_slice(&count.to_le_bytes());
        assert!(model.geoset_streams_v1800(&bad).is_err());
    }
    for version in [800, 900, 1000, 1200, 1400, 1900] {
        assert!(
            matches!(container_model(version).geoset_streams_v1800(&source),Err(Error::UnsupportedVersion(got)) if got==version)
        );
    }
}

#[test]
fn truncated_and_duplicate_streams_are_typed_errors() {
    let (mut geoset, tangent, _) = geoset();
    let model = container_model(1800);
    let repeated = geoset.data[tangent..tangent + 24].to_vec();
    geoset.data.extend(repeated);
    assert!(
        matches!(model.geoset_streams_v1800(&geoset),Err(Error::DuplicateGeosetTag {tag,..}) if &tag==b"TANG")
    );
    geoset.data.truncate(tangent + 8 + 15);
    assert!(matches!(
        model.geoset_streams_v1800(&geoset),
        Err(Error::Truncated { .. })
    ));
}

#[test]
fn stored_container_retains_opaque_chunks_without_changing_strict_parser() {
    let (geoset, _, _) = geoset();
    let mut bytes = b"MDLXVERS".to_vec();
    word(&mut bytes, 4);
    word(&mut bytes, 1800);
    let mut payload = Vec::new();
    word(&mut payload, geoset.size);
    payload.extend(&geoset.data);
    block(&mut bytes, b"GEOS", payload.len() as u32, &payload);
    block(&mut bytes, b"FAFX", 3, &[1, 2, 3]);
    assert!(matches!(Mdlx::parse(&bytes),Err(Error::UnknownTag(tag)) if &tag==b"FAFX"));
    let stored = Mdlx::parse_stored_v1800(&bytes).unwrap();
    assert_eq!(stored.payload(&bytes, b"FAFX"), Some([1, 2, 3].as_slice()));
    let geos = stored.geosets(&bytes).unwrap();
    assert_eq!(geos[0], geoset);
    assert!(stored.geoset_streams_v1800(&geos[0]).is_ok());
    assert!(stored.mesh(&bytes, &geos[0]).is_err());
    let mut truncated = bytes.clone();
    truncated.pop();
    assert!(matches!(
        Mdlx::parse_stored_v1800(&truncated),
        Err(Error::ChunkOverruns { .. })
    ));
    bytes[12..16].copy_from_slice(&900u32.to_le_bytes());
    assert!(matches!(
        Mdlx::parse_stored_v1800(&bytes),
        Err(Error::UnsupportedVersion(900))
    ));
}

#[test]
fn stored_container_rejects_duplicate_matching_vers() {
    let mut bytes = b"MDLX".to_vec();
    for version in [1800u32, 1800] {
        block(&mut bytes, b"VERS", 4, &version.to_le_bytes());
    }
    assert!(matches!(
        Mdlx::parse_stored_v1800(&bytes),
        Err(Error::UnsupportedVersionedChunk {
            version: 1800,
            chunk: "duplicate VERS"
        })
    ));
    // The stricter stored-accessor policy does not change the classic API.
    assert_eq!(Mdlx::parse(&bytes).unwrap().version, 1800);
}

#[test]
fn stored_container_rejects_duplicate_conflicting_vers() {
    let mut bytes = b"MDLX".to_vec();
    for version in [1800u32, 900] {
        block(&mut bytes, b"VERS", 4, &version.to_le_bytes());
    }
    assert!(matches!(
        Mdlx::parse_stored_v1800(&bytes),
        Err(Error::UnsupportedVersionedChunk {
            version: 1800,
            chunk: "duplicate VERS"
        })
    ));
    assert_eq!(Mdlx::parse(&bytes).unwrap().version, 1800);
}

#[test]
fn stored_container_rejects_short_vers_payload() {
    for size in 0..4usize {
        let mut bytes = b"MDLX".to_vec();
        block(
            &mut bytes,
            b"VERS",
            size as u32,
            &1800u32.to_le_bytes()[..size],
        );
        assert!(matches!(
            Mdlx::parse_stored_v1800(&bytes),
            Err(Error::RecordShape { chunk: "VERS", at: 4, size: got, measured: 4 }) if got == size
        ));
    }
}

#[test]
fn stored_container_rejects_oversized_vers_payload() {
    for size in [5usize, 8] {
        let mut payload = 1800u32.to_le_bytes().to_vec();
        payload.resize(size, 0);
        let mut bytes = b"MDLX".to_vec();
        block(&mut bytes, b"VERS", size as u32, &payload);
        assert!(matches!(
            Mdlx::parse_stored_v1800(&bytes),
            Err(Error::RecordShape { chunk: "VERS", at: 4, size: got, measured: 4 }) if got == size
        ));
        assert_eq!(Mdlx::parse(&bytes).unwrap().version, 1800);
    }
}

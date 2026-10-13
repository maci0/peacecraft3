//! Synthetic raw records; no original game bytes are embedded.
use pcx_mdl::{Chunk, Error, Mdlx};

fn chunk(bytes: &mut Vec<u8>, tag: &[u8; 4], payload: &[u8]) {
    bytes.extend(tag);
    bytes.extend(u32::try_from(payload.len()).unwrap().to_le_bytes());
    bytes.extend(payload);
}

fn container() -> Vec<u8> {
    let mut bytes = b"MDLX".to_vec();
    chunk(&mut bytes, b"VERS", &1800u32.to_le_bytes());
    bytes
}

#[test]
fn fafx_preserves_non_utf8_nuls_trailing_bytes_and_borrowed_ranges() {
    let mut bytes = container();
    chunk(&mut bytes, b"TEST", &[9, 8, 7]);
    let chunk_offset = bytes.len();
    let mut raw = [0xff; 680];
    for (index, byte) in raw.iter_mut().enumerate() {
        *byte = (index % 256) as u8;
    }
    // A NUL does not shorten either field, and neither field requires UTF-8.
    raw[0] = 0xff;
    raw[1] = 0;
    raw[79] = 0xfe;
    raw[80] = 0xff;
    raw[81] = 0;
    raw[339] = 0xfe;
    // The second record deliberately has no NUL terminator.
    raw[340..].fill(0xff);
    chunk(&mut bytes, b"FAFX", &raw);
    chunk(&mut bytes, b"TAIL", &[1, 2]);
    let model = Mdlx::parse_stored_v1800(&bytes).unwrap();
    let stream = model.fafx_stream_v1800(&bytes).unwrap().unwrap();
    assert_eq!(stream.version, 1800);
    assert_eq!(stream.offset, chunk_offset);
    assert_eq!(stream.payload_offset, chunk_offset + 8);
    assert_eq!(stream.record_count(), 2);
    assert_eq!(stream.data(), raw);
    assert_eq!(stream.data().as_ptr(), bytes[chunk_offset + 8..].as_ptr());
    for index in 0..2 {
        let record = stream.get(index).unwrap();
        let offset = chunk_offset + 8 + index * 340;
        assert_eq!(record.offset, offset);
        assert_eq!(record.name.as_slice(), &bytes[offset..offset + 80]);
        assert_eq!(record.path.as_slice(), &bytes[offset + 80..offset + 340]);
        assert_eq!(record.name.as_ptr(), bytes[offset..].as_ptr());
        assert_eq!(record.path.as_ptr(), bytes[offset + 80..].as_ptr());
    }
    assert!(stream.get(2).is_none());
    assert!(stream.get(usize::MAX).is_none());
}

#[test]
fn fafx_absent_and_present_empty_are_distinct() {
    let mut bytes = container();
    let model = Mdlx::parse_stored_v1800(&bytes).unwrap();
    assert!(model.fafx_stream_v1800(&bytes).unwrap().is_none());
    chunk(&mut bytes, b"FAFX", &[]);
    let model = Mdlx::parse_stored_v1800(&bytes).unwrap();
    let stream = model.fafx_stream_v1800(&bytes).unwrap().unwrap();
    assert_eq!(stream.record_count(), 0);
    assert!(stream.data().is_empty());
    assert!(stream.get(0).is_none());
}

#[test]
fn fafx_rejects_short_long_and_non_340_remainders() {
    for size in [1, 4, 79, 80, 259, 260, 339, 341, 344, 679, 681] {
        let mut bytes = container();
        chunk(&mut bytes, b"FAFX", &vec![0xa5; size]);
        let model = Mdlx::parse_stored_v1800(&bytes).unwrap();
        assert!(matches!(
            model.fafx_stream_v1800(&bytes),
            Err(Error::RaggedRecords { chunk: "FAFX", size: got, stride: 340 }) if got == size
        ));
    }
}

#[test]
fn fafx_rejects_a_count_prefix_instead_of_shifting_raw_fields() {
    let mut bytes = container();
    let mut raw = 1u32.to_le_bytes().to_vec();
    raw.extend([0xa5; 340]);
    chunk(&mut bytes, b"FAFX", &raw);
    let model = Mdlx::parse_stored_v1800(&bytes).unwrap();
    assert!(matches!(
        model.fafx_stream_v1800(&bytes),
        Err(Error::RaggedRecords {
            chunk: "FAFX",
            size: 344,
            stride: 340
        })
    ));
}

#[test]
fn fafx_rejects_matching_conflicting_and_empty_duplicate_chunks() {
    for second in [vec![0xa5; 340], vec![0x5a; 340], Vec::new()] {
        let mut bytes = container();
        chunk(&mut bytes, b"FAFX", &[0xa5; 340]);
        chunk(&mut bytes, b"FAFX", &second);
        let model = Mdlx::parse_stored_v1800(&bytes).unwrap();
        assert!(matches!(
            model.fafx_stream_v1800(&bytes),
            Err(Error::UnsupportedVersionedChunk {
                version: 1800,
                chunk: "duplicate FAFX"
            })
        ));
    }
}

#[test]
fn fafx_refuses_other_versions_even_without_a_chunk() {
    for version in [800, 900, 1000, 1200, 1400, 1801] {
        let model = Mdlx {
            version,
            chunks: Vec::new(),
        };
        assert!(matches!(
            model.fafx_stream_v1800(&[]),
            Err(Error::UnsupportedVersion(got)) if got == version
        ));
    }
}

#[test]
fn fafx_truncated_input_and_offset_overflow_are_typed_errors() {
    let mut bytes = container();
    chunk(&mut bytes, b"FAFX", &[0xa5; 340]);
    let model = Mdlx::parse_stored_v1800(&bytes).unwrap();
    assert!(matches!(
        model.fafx_stream_v1800(&bytes[..bytes.len() - 1]),
        Err(Error::Truncated {
            what: "FAFX payload",
            need: 340,
            have: 339,
            ..
        })
    ));
    assert!(matches!(
        Mdlx::parse_stored_v1800(&bytes[..bytes.len() - 1]),
        Err(Error::ChunkOverruns {
            size: 340,
            left: 339,
            ..
        })
    ));
    let malformed = Mdlx {
        version: 1800,
        chunks: vec![Chunk {
            tag: *b"FAFX",
            offset: 0,
            payload_offset: usize::MAX,
            size: 340,
        }],
    };
    assert!(matches!(
        malformed.fafx_stream_v1800(&[]),
        Err(Error::Truncated {
            what: "FAFX payload",
            have: 0,
            ..
        })
    ));
}

#[test]
fn fafx_does_not_relax_classic_strict_container_parsing() {
    let mut bytes = container();
    chunk(&mut bytes, b"FAFX", &[0xa5; 340]);
    assert!(matches!(Mdlx::parse(&bytes), Err(Error::UnknownTag(tag)) if &tag == b"FAFX"));
    let model = Mdlx::parse_stored_v1800(&bytes).unwrap();
    assert_eq!(
        model
            .fafx_stream_v1800(&bytes)
            .unwrap()
            .unwrap()
            .record_count(),
        1
    );
}

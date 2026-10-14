//! Synthetic framing only; no rendering, normalization or native behavior claim.
use pcx_mdl::{Error, Geoset, Mdlx, SkinStream1800, TangentStream1800};

fn words32(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|v| v.to_le_bytes()).collect()
}
fn words16(words: &[u16]) -> Vec<u8> {
    words.iter().flat_map(|v| v.to_le_bytes()).collect()
}
fn block(data: &mut Vec<u8>, tag: &[u8; 4], count: u32, body: &[u8]) -> usize {
    let at = data.len();
    data.extend(tag);
    data.extend(count.to_le_bytes());
    data.extend(body);
    at
}
fn model() -> Mdlx {
    Mdlx {
        version: 1800,
        chunks: Vec::new(),
    }
}

#[test]
fn tangent_access_honors_header_count_even_with_trailing_complete_records() {
    let raw = words32(&[
        0x8000_0000,
        0x7fa0_1234,
        1,
        0xff80_0000,
        0x7fc0_9876,
        0x3f80_0000,
        0xbf80_0000,
        0,
    ]);
    for count in [0, 1, 2, u32::MAX] {
        let stream = TangentStream1800 {
            offset: 53,
            count,
            data: &raw,
        };
        for index in [0, 1, 2, usize::MAX] {
            let expected = index < count as usize && index < 2;
            assert_eq!(
                stream.bits(index).is_some(),
                expected,
                "count={count} index={index}"
            );
        }
        assert_eq!(stream.offset, 53);
        assert_eq!(stream.data, raw);
    }
    let stream = TangentStream1800 {
        offset: 53,
        count: 1,
        data: &raw,
    };
    assert_eq!(
        stream.bits(0),
        Some([0x8000_0000, 0x7fa0_1234, 1, 0xff80_0000])
    );
}

#[test]
fn skin_access_honors_authored_word_count_without_trailing_record_invention() {
    let raw = words16(&[
        513, 1027, 4099, 65535, 7, 1024, 32768, 65535, 3, 2, 1, 0, 65535, 0, 256, 513,
    ]);
    for word_count in [0, 1, 7, 8, 9, 15, 16, u32::MAX] {
        let stream = SkinStream1800 {
            offset: 117,
            word_count,
            data: &raw,
        };
        let expected_count = (word_count as usize / 8).min(2);
        assert_eq!(stream.record_count(), expected_count);
        for index in [0, 1, 2, usize::MAX] {
            assert_eq!(
                stream.get(index).is_some(),
                index < expected_count,
                "word_count={word_count} index={index}"
            );
        }
        assert_eq!(stream.offset, 117);
        assert_eq!(stream.data, raw);
    }
    let stream = SkinStream1800 {
        offset: 117,
        word_count: 8,
        data: &raw,
    };
    let record = stream.get(0).unwrap();
    assert_eq!(record.joint_words, [513, 1027, 4099, 65535]);
    assert_eq!(record.weight_words, [7, 1024, 32768, 65535]);
}

#[test]
fn header_counts_do_not_allow_incomplete_source_records_or_panics() {
    let raw = [0u8; 32];
    for bytes in 0..=raw.len() {
        let tangent = TangentStream1800 {
            offset: 0,
            count: u32::MAX,
            data: &raw[..bytes],
        };
        let skin = SkinStream1800 {
            offset: 0,
            word_count: u32::MAX,
            data: &raw[..bytes],
        };
        assert_eq!(skin.record_count(), bytes / 16);
        for index in [0, 1, 2, usize::MAX] {
            let complete = index < bytes / 16;
            assert_eq!(tangent.bits(index).is_some(), complete);
            assert_eq!(skin.get(index).is_some(), complete);
        }
    }
}

#[test]
fn parsed_normals_vertices_tangents_and_skin_retain_order_bits_and_borrowed_spans() {
    let vertices = words32(&[
        0x8000_0000,
        0x7fa0_1234,
        1,
        0x7fc0_5555,
        0x7f80_0000,
        0xff80_0000,
    ]);
    let normals = words32(&[0x7fa0_4321, 0x8000_0000, 0x7fc0_9876, 0, 1, 0xbf80_0000]);
    let tangents = words32(&[
        0x8000_0000,
        0x7fa0_1234,
        1,
        0xff80_0000,
        0x7fc0_9876,
        0x3f80_0000,
        0xbf80_0000,
        0,
    ]);
    let skin = words16(&[
        513, 1027, 4099, 65535, 7, 1024, 32768, 65535, 3, 2, 1, 0, 65535, 0, 256, 513,
    ]);
    let mut data = Vec::new();
    let v = block(&mut data, b"VRTX", 2, &vertices);
    let n = block(&mut data, b"NRMS", 2, &normals);
    block(&mut data, b"MATS", 0, &[]);
    data.extend([0; 128]);
    let t = block(&mut data, b"TANG", 2, &tangents);
    let s = block(&mut data, b"SKIN", 16, &skin);
    block(&mut data, b"UVAS", 1, &[]);
    block(&mut data, b"UVBS", 2, &[0; 16]);
    let geoset = Geoset {
        offset: 89,
        size: data.len() as u32 + 4,
        data,
    };
    let streams = model().geoset_streams_v1800(&geoset).unwrap();
    for (tag, offset, raw) in [
        (b"VRTX", v, &vertices),
        (b"NRMS", n, &normals),
        (b"TANG", t, &tangents),
        (b"SKIN", s, &skin),
    ] {
        let block = streams.blocks.iter().find(|b| &b.tag == tag).unwrap();
        assert_eq!(block.offset, offset);
        assert_eq!(block.body, raw);
        assert_eq!(block.body.as_ptr(), geoset.data[offset + 8..].as_ptr());
    }
    let tangent = streams.tangents.as_ref().unwrap();
    assert_eq!(tangent.offset, t);
    assert_eq!(
        tangent.bits(1),
        Some([0x7fc0_9876, 0x3f80_0000, 0xbf80_0000, 0])
    );
    assert_eq!(tangent.data.as_ptr(), geoset.data[t + 8..].as_ptr());
    let weights = streams.skin.as_ref().unwrap();
    assert_eq!(weights.offset, s);
    assert_eq!(weights.get(1).unwrap().joint_words, [3, 2, 1, 0]);
    assert_eq!(weights.get(1).unwrap().weight_words, [65535, 0, 256, 513]);
    assert_eq!(weights.data.as_ptr(), geoset.data[s + 8..].as_ptr());
    let mut reconstructed = Vec::new();
    for b in &streams.blocks {
        reconstructed.extend(b.tag);
        reconstructed.extend(b.count.to_le_bytes());
        reconstructed.extend(b.body);
        reconstructed.extend(b.trailer);
    }
    assert_eq!(reconstructed, geoset.data);
    assert!(matches!(model().mesh(&[], &geoset),
        Err(Error::UnsupportedGeosetFeature { version: 1800, at, tag }) if at == t && &tag == b"TANG"));
}

#[test]
fn skin_only_stream_still_has_typed_strict_mesh_refusal() {
    let mut data = Vec::new();
    block(&mut data, b"VRTX", 1, &[0; 12]);
    block(&mut data, b"NRMS", 1, &[0; 12]);
    let s = block(&mut data, b"SKIN", 8, &[0; 16]);
    let geoset = Geoset {
        offset: 0,
        size: data.len() as u32 + 4,
        data,
    };
    assert!(model().geoset_streams_v1800(&geoset).is_ok());
    assert!(matches!(model().mesh(&[], &geoset),
        Err(Error::UnsupportedGeosetFeature { version: 1800, at, tag }) if at == s && &tag == b"SKIN"));
}

use pcx_mdl::{Error, Mdlx};

fn record(size: u32, id: u32) -> Vec<u8> {
    let mut bytes = vec![0; 96];
    bytes[..4].copy_from_slice(&size.to_le_bytes());
    bytes[84..88].copy_from_slice(&id.to_le_bytes());
    bytes[88..92].copy_from_slice(&(-1_i32).to_le_bytes());
    bytes
}

fn trailer(bytes: &mut Vec<u8>) {
    bytes.extend_from_slice(&(-1_i32).to_le_bytes());
    bytes.extend_from_slice(&(-1_i32).to_le_bytes());
}

fn container(payload: &[u8]) -> Vec<u8> {
    let mut bytes = b"MDLXVERS".to_vec();
    bytes.extend_from_slice(&4_u32.to_le_bytes());
    bytes.extend_from_slice(&800_u32.to_le_bytes());
    bytes.extend_from_slice(b"BONE");
    bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    bytes.extend_from_slice(payload);
    bytes
}

#[test]
fn every_undersized_header_is_rejected_even_with_following_bytes_available() {
    for size in 0..96 {
        let mut payload = record(size, 7);
        trailer(&mut payload);
        payload.extend_from_slice(&record(96, 9));
        trailer(&mut payload);
        let bytes = container(&payload);
        let model = Mdlx::parse(&bytes).unwrap();
        assert!(matches!(
            model.bones(&bytes),
            Err(Error::BoneRecordOverruns { at: 0, size: declared, .. }) if declared == size as usize
        ));
    }
}

#[test]
fn minimal_complete_records_retain_ids_and_trailers() {
    let mut payload = record(96, 7);
    trailer(&mut payload);
    payload.extend_from_slice(&record(96, 9));
    trailer(&mut payload);
    let bytes = container(&payload);
    let bones = Mdlx::parse(&bytes).unwrap().bones(&bytes).unwrap();
    assert_eq!(
        bones.iter().map(|b| b.object_id).collect::<Vec<_>>(),
        [7, 9]
    );
    for (i, bone) in bones.iter().enumerate() {
        assert_eq!(bone.offset, i * 104);
        assert_eq!(bone.span, 104);
        assert_eq!(bone.parent_id, -1);
        assert_eq!(bone.geoset_id, -1);
        assert_eq!(bone.geoset_anim_id, -1);
        assert!(bone.tracks.is_empty());
    }
}

#[test]
fn huge_record_span_after_a_valid_record_is_rejected() {
    let mut payload = record(96, 7);
    trailer(&mut payload);
    payload.extend_from_slice(&record(u32::MAX - 8, 9));
    trailer(&mut payload);
    let bytes = container(&payload);
    assert!(matches!(
        Mdlx::parse(&bytes).unwrap().bones(&bytes),
        Err(Error::BoneRecordOverruns { at: 104, .. })
    ));
}

#[test]
fn track_header_and_body_span_overflow_is_rejected_before_slicing() {
    let mut payload = record(112, 7);
    payload.extend_from_slice(b"KGTR");
    payload.extend_from_slice(&0x0fff_ffff_u32.to_le_bytes());
    payload.extend_from_slice(&1_u32.to_le_bytes());
    payload.extend_from_slice(&u32::MAX.to_le_bytes());
    trailer(&mut payload);
    let bytes = container(&payload);
    assert!(matches!(
        Mdlx::parse(&bytes).unwrap().bones(&bytes),
        Err(Error::BoneTrackOverruns { at: 96, .. })
    ));
}

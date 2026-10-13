//! Synthetic primary-convention joins; these do not assert native selection.
use pcx_mdl::{Error, Mdlx, PrimaryJointRefusal1800 as Refusal, SkinRecord1800, tag};

fn chunk(bytes: &mut Vec<u8>, tag: &[u8; 4], body: &[u8]) {
    bytes.extend(tag);
    bytes.extend(u32::try_from(body.len()).unwrap().to_le_bytes());
    bytes.extend(body);
}
fn block(bytes: &mut Vec<u8>, tag: &[u8; 4], count: u32, body: &[u8]) {
    bytes.extend(tag);
    bytes.extend(count.to_le_bytes());
    bytes.extend(body);
}
fn matrix_block(bytes: &mut Vec<u8>, palette: &[u32]) {
    let raw: Vec<_> = palette.iter().flat_map(|id| id.to_le_bytes()).collect();
    block(bytes, b"MATS", palette.len() as u32, &raw);
    bytes.extend([0; 128]);
}
fn bone_body(ids: &[u32]) -> Vec<u8> {
    let mut bones = Vec::new();
    for id in ids {
        bones.extend(96u32.to_le_bytes());
        bones.extend([0; 80]);
        bones.extend(id.to_le_bytes());
        bones.extend((-1i32).to_le_bytes());
        bones.extend(0x100u32.to_le_bytes());
        bones.extend([0xff; 8]);
    }
    bones
}
fn pose_bits(index: u32) -> [u32; 12] {
    let mut bits = [0x7fc0_1234; 12];
    bits[0] = 0x8000_0000;
    bits[1] = index;
    bits[11] = 0xff80_0000;
    bits
}
fn fixture(
    palette: Option<&[u32]>,
    bones: &[u32],
    poses: Option<u32>,
    joints: Option<[u16; 4]>,
    weights: [u16; 4],
) -> Vec<u8> {
    let mut bytes = b"MDLX".to_vec();
    chunk(&mut bytes, b"VERS", &1800u32.to_le_bytes());
    let mut geoset = Vec::new();
    block(&mut geoset, b"VRTX", 1, &[0; 12]);
    if let Some(palette) = palette {
        matrix_block(&mut geoset, palette);
    }
    if let Some(joints) = joints {
        let skin: Vec<_> = joints
            .into_iter()
            .chain(weights)
            .flat_map(u16::to_le_bytes)
            .collect();
        block(&mut geoset, b"SKIN", 8, &skin);
    }
    block(&mut geoset, b"UVAS", 0, &[]);
    let mut inclusive = (geoset.len() as u32 + 4).to_le_bytes().to_vec();
    inclusive.extend(geoset);
    chunk(&mut bytes, tag::GEOS, &inclusive);
    chunk(&mut bytes, tag::BONE, &bone_body(bones));
    if let Some(count) = poses {
        let mut body = count.to_le_bytes().to_vec();
        for index in 0..count {
            body.extend(pose_bits(index).into_iter().flat_map(u32::to_le_bytes));
        }
        chunk(&mut bytes, b"BPOS", &body);
    }
    bytes
}
fn default_fixture() -> Vec<u8> {
    fixture(
        Some(&[21, 10]),
        &[10, 21],
        Some(22),
        Some([0, 1, 0, 1]),
        [0, 65535, 7, 1024],
    )
}

#[test]
fn nonidentity_mats_joins_sparse_object_ids_and_bpos_rows_without_rebase() {
    // Both source BONE orders must yield the same object ID / BPOS row.
    for bones in [&[10, 21][..], &[21, 10][..]] {
        let bytes = fixture(
            Some(&[21, 10]),
            bones,
            Some(22),
            Some([0, 1, 0, 1]),
            [0, 65535, 7, 1024],
        );
        let model = Mdlx::parse_stored_v1800(&bytes).unwrap();
        let geoset = model.geosets(&bytes).unwrap().remove(0);
        let streams = model.geoset_streams_v1800(&geoset).unwrap();
        let skin = streams.skin.unwrap();
        let base = model.chunk(tag::GEOS).unwrap().payload_offset + geoset.offset + 4;
        let palette = model
            .matrix_palette_v1800(&bytes, &geoset)
            .unwrap()
            .unwrap();
        assert_eq!(palette.geoset_offset, base - 4);
        assert_eq!(palette.count, 2);
        assert_eq!(palette.record_count(), 2);
        assert_eq!(
            palette.data().as_ptr(),
            bytes[palette.payload_offset..].as_ptr()
        );
        assert!(palette.get(2).is_none());
        assert!(palette.get(usize::MAX).is_none());
        let poses = model.bind_pose_stream_v1800(&bytes).unwrap().unwrap();
        let join = model.primary_joint_join_v1800(&bytes, &geoset).unwrap();
        for (lane, object_id) in [21u32, 10, 21, 10].into_iter().enumerate() {
            let result = join.get(0, lane).unwrap();
            let joint = usize::from(skin.get(0).unwrap().joint_words[lane]);
            let entry = palette.get(joint).unwrap();
            assert_eq!(result.version, 1800);
            assert_eq!(result.object_id, object_id);
            assert_eq!(result.bind_pose.index, object_id as usize);
            assert_eq!(result.bind_pose.bits(), pose_bits(object_id));
            assert_eq!(
                result.bind_pose.offset,
                poses.payload_offset + 4 + object_id as usize * 48
            );
            assert_eq!(
                result.bind_pose.data.as_ptr(),
                bytes[result.bind_pose.offset..].as_ptr()
            );
            assert_eq!(result.joint_offset, base + skin.offset + 8 + lane * 2);
            assert_eq!(result.weight_offset, result.joint_offset + 8);
            assert_eq!(result.raw_weight, skin.get(0).unwrap().weight_words[lane]);
            assert_eq!(result.mats_offset, entry.offset);
            assert_eq!(entry.data.as_ptr(), bytes[entry.offset..].as_ptr());
            assert_eq!(
                result.raw_joint.to_le_bytes(),
                bytes[result.joint_offset..result.joint_offset + 2]
            );
            assert_eq!(
                result.raw_weight.to_le_bytes(),
                bytes[result.weight_offset..result.weight_offset + 2]
            );
            assert_eq!(
                object_id.to_le_bytes(),
                bytes[result.bone_object_id_offset..result.bone_object_id_offset + 4]
            );
            assert_eq!(object_id.to_le_bytes(), *entry.data);
        }
        assert!(matches!(
            join.get(usize::MAX, 0),
            Err(Error::PrimaryJointRefusal {
                reason: Refusal::InvalidVertex { record_count: 1 },
                ..
            })
        ));
        assert!(matches!(
            join.get(0, 4),
            Err(Error::PrimaryJointRefusal {
                reason: Refusal::InvalidLane,
                ..
            })
        ));
    }
}

#[test]
fn integer_weight_diagnostics_preserve_zeros_duplicates_and_full_u16_range() {
    let record = SkinRecord1800 {
        joint_words: [0, 1, 0, 1],
        weight_words: [0, 65535, 7, 1024],
    };
    let before = record.clone();
    let summary = record.weight_diagnostics();
    assert_eq!(summary.sum, 66566);
    assert_eq!(summary.nonzero_lanes, 3);
    assert_eq!(summary.max_word, 65535);
    assert_eq!(record, before);
    let maximum = SkinRecord1800 {
        joint_words: [0; 4],
        weight_words: [65535; 4],
    }
    .weight_diagnostics();
    assert_eq!(maximum.sum, 262140);
    assert_eq!(maximum.nonzero_lanes, 4);
    let zero = SkinRecord1800 {
        joint_words: [0; 4],
        weight_words: [0; 4],
    }
    .weight_diagnostics();
    assert_eq!(zero.sum, 0);
    assert_eq!(zero.nonzero_lanes, 0);
    assert_eq!(zero.max_word, 0);
}

#[test]
fn bpos_rows_borrow_exact_bits_and_missing_empty_are_distinct() {
    let bytes = default_fixture();
    let model = Mdlx::parse_stored_v1800(&bytes).unwrap();
    let poses = model.bind_pose_stream_v1800(&bytes).unwrap().unwrap();
    assert_eq!(poses.version, 1800);
    assert_eq!(poses.count, 22);
    assert_eq!(poses.record_count(), 22);
    assert_eq!(poses.data(), model.payload(&bytes, b"BPOS").unwrap());
    assert_eq!(poses.offset + 8, poses.payload_offset);
    for index in 0..22 {
        let row = poses.get(index).unwrap();
        assert_eq!(row.index, index);
        assert_eq!(row.bits(), pose_bits(index as u32));
        assert_eq!(row.data.as_slice(), &bytes[row.offset..row.offset + 48]);
        assert_eq!(row.data.as_ptr(), bytes[row.offset..].as_ptr());
    }
    assert!(poses.get(22).is_none());
    assert!(poses.get(usize::MAX).is_none());
    for count in [None, Some(0)] {
        let bytes = fixture(None, &[], count, None, [0; 4]);
        let model = Mdlx::parse_stored_v1800(&bytes).unwrap();
        let poses = model.bind_pose_stream_v1800(&bytes).unwrap();
        assert_eq!(poses.is_some(), count.is_some());
        if let Some(poses) = poses {
            assert_eq!(poses.record_count(), 0);
            assert_eq!(poses.data(), 0u32.to_le_bytes());
            assert!(poses.get(0).is_none());
        }
    }
}

#[test]
fn malformed_or_duplicate_bpos_refuses_without_hiding_raw_mats() {
    let base = fixture(Some(&[3]), &[3], None, Some([0; 4]), [0; 4]);
    for body in [
        Vec::new(),
        vec![0; 3],
        1u32.to_le_bytes().to_vec(),
        u32::MAX.to_le_bytes().to_vec(),
        vec![0; 5],
        vec![0; 52],
    ] {
        let mut bytes = base.clone();
        chunk(&mut bytes, b"BPOS", &body);
        let model = Mdlx::parse_stored_v1800(&bytes).unwrap();
        assert!(matches!(
            model.bind_pose_stream_v1800(&bytes),
            Err(Error::Truncated { .. } | Error::RecordShape { chunk: "BPOS", .. })
        ));
        let geoset = model.geosets(&bytes).unwrap().remove(0);
        assert_eq!(
            model
                .matrix_palette_v1800(&bytes, &geoset)
                .unwrap()
                .unwrap()
                .get(0)
                .unwrap()
                .object_id,
            3
        );
    }
    let mut bytes = default_fixture();
    chunk(&mut bytes, b"BPOS", &0u32.to_le_bytes());
    let model = Mdlx::parse_stored_v1800(&bytes).unwrap();
    assert!(matches!(
        model.bind_pose_stream_v1800(&bytes),
        Err(Error::UnsupportedVersionedChunk {
            chunk: "duplicate BPOS",
            ..
        })
    ));
    let bytes = default_fixture();
    let model = Mdlx::parse_stored_v1800(&bytes).unwrap();
    assert!(matches!(
        model.bind_pose_stream_v1800(&bytes[..bytes.len() - 1]),
        Err(Error::Truncated {
            what: "BPOS payload",
            ..
        })
    ));
}

#[test]
fn out_of_palette_including_zero_weight_refuses_direct_id_fallback_contextually() {
    for weight in [0, 65535] {
        let bytes = fixture(
            Some(&[21]),
            &[21],
            Some(22),
            Some([21, 0, 0, 0]),
            [weight, 0, 0, 0],
        );
        let model = Mdlx::parse_stored_v1800(&bytes).unwrap();
        let geoset = model.geosets(&bytes).unwrap().remove(0);
        let skin = model.geoset_streams_v1800(&geoset).unwrap().skin.unwrap();
        let at =
            model.chunk(tag::GEOS).unwrap().payload_offset + geoset.offset + 4 + skin.offset + 8;
        assert_eq!(skin.get(0).unwrap().joint_words[0], 21);
        assert!(
            matches!(model.primary_joint_join_v1800(&bytes, &geoset).unwrap().get(0, 0), Err(Error::PrimaryJointRefusal { vertex: 0, lane: 0, at: got, reason: Refusal::JointOutsidePalette { joint: 21, palette_count: 1 }, .. }) if got == at)
        );
        assert_eq!(
            model
                .bind_pose_stream_v1800(&bytes)
                .unwrap()
                .unwrap()
                .get(21)
                .unwrap()
                .index,
            21
        );
    }
}

#[test]
fn nonbone_and_ambiguous_bone_objects_refuse_with_lane_context() {
    for (bones, ambiguous) in [(&[2][..], false), (&[3, 3][..], true)] {
        let mut bytes = fixture(Some(&[3]), bones, Some(4), Some([0; 4]), [7; 4]);
        let helper = bone_body(&[3]);
        chunk(&mut bytes, tag::HELP, &helper[..96]);
        let model = Mdlx::parse_stored_v1800(&bytes).unwrap();
        assert_eq!(model.nodes(&bytes, tag::HELP).unwrap()[0].object_id, 3);
        let geoset = model.geosets(&bytes).unwrap().remove(0);
        let refusal = model
            .primary_joint_join_v1800(&bytes, &geoset)
            .unwrap()
            .get(0, 2)
            .unwrap_err();
        assert!(
            matches!(
                refusal,
                Error::PrimaryJointRefusal {
                    vertex: 0,
                    lane: 2,
                    reason: Refusal::AmbiguousBoneObject { object_id: 3 },
                    ..
                }
            ) == ambiguous
        );
        if !ambiguous {
            assert!(matches!(
                refusal,
                Error::PrimaryJointRefusal {
                    vertex: 0,
                    lane: 2,
                    reason: Refusal::ObjectNotInBoneInventory { object_id: 3 },
                    ..
                }
            ));
        }
        assert_eq!(
            model
                .matrix_palette_v1800(&bytes, &geoset)
                .unwrap()
                .unwrap()
                .get(0)
                .unwrap()
                .object_id,
            3
        );
    }
}

#[test]
fn absent_streams_and_missing_pose_rows_refuse_without_defaults() {
    for (palette, poses, joints, expected) in [
        (None, Some(4), Some([0; 4]), Refusal::MissingPalette),
        (
            Some(&[3][..]),
            None,
            Some([0; 4]),
            Refusal::MissingBindPoses,
        ),
        (
            Some(&[3][..]),
            Some(3),
            Some([0; 4]),
            Refusal::MissingBindPoseRow {
                object_id: 3,
                record_count: 3,
            },
        ),
        (Some(&[3][..]), Some(4), None, Refusal::MissingSkin),
    ] {
        let bytes = fixture(palette, &[3], poses, joints, [0; 4]);
        let model = Mdlx::parse_stored_v1800(&bytes).unwrap();
        let geoset = model.geosets(&bytes).unwrap().remove(0);
        assert!(
            matches!(model.primary_joint_join_v1800(&bytes, &geoset).unwrap().get(0, 0), Err(Error::PrimaryJointRefusal { reason, .. }) if reason == expected)
        );
    }
}

#[test]
fn version_duplicate_chunks_and_mismatched_geoset_sources_are_typed() {
    let bytes = default_fixture();
    let mut model = Mdlx::parse_stored_v1800(&bytes).unwrap();
    let geoset = model.geosets(&bytes).unwrap().remove(0);
    for version in [800, 900, 1000, 1400, 1801] {
        model.version = version;
        assert!(
            matches!(model.bind_pose_stream_v1800(&bytes), Err(Error::UnsupportedVersion(got)) if got == version)
        );
        assert!(
            matches!(model.matrix_palette_v1800(&bytes, &geoset), Err(Error::UnsupportedVersion(got)) if got == version)
        );
        assert!(
            matches!(model.primary_joint_join_v1800(&bytes, &geoset), Err(Error::UnsupportedVersion(got)) if got == version)
        );
    }
    model.version = 1800;
    let mut other = geoset.clone();
    other.data[8] ^= 1;
    assert!(matches!(
        model.matrix_palette_v1800(&bytes, &other),
        Err(Error::UnsupportedVersionedChunk {
            chunk: "GEOS source mismatch",
            ..
        })
    ));
    other.offset = usize::MAX;
    assert!(matches!(
        model.primary_joint_join_v1800(&bytes, &other),
        Err(Error::Truncated { .. })
    ));
    let mut duplicate = bytes.clone();
    chunk(&mut duplicate, tag::BONE, &bone_body(&[3]));
    let model = Mdlx::parse_stored_v1800(&duplicate).unwrap();
    assert!(matches!(
        model.primary_joint_join_v1800(&duplicate, &geoset),
        Err(Error::UnsupportedVersionedChunk {
            chunk: "duplicate BONE",
            ..
        })
    ));
    let mut duplicate = bytes.clone();
    chunk(
        &mut duplicate,
        tag::GEOS,
        model.payload(&bytes, tag::GEOS).unwrap(),
    );
    let model = Mdlx::parse_stored_v1800(&duplicate).unwrap();
    assert!(matches!(
        model.matrix_palette_v1800(&duplicate, &geoset),
        Err(Error::UnsupportedVersionedChunk {
            chunk: "duplicate GEOS",
            ..
        })
    ));
}

#[test]
fn duplicate_mats_refuses_ambiguity_and_unchecked_offsets_do_not_panic() {
    let mut bytes = b"MDLX".to_vec();
    chunk(&mut bytes, b"VERS", &1800u32.to_le_bytes());
    let mut g = Vec::new();
    block(&mut g, b"VRTX", 1, &[0; 12]);
    matrix_block(&mut g, &[3]);
    matrix_block(&mut g, &[4]);
    let mut raw = (g.len() as u32 + 4).to_le_bytes().to_vec();
    raw.extend(g);
    chunk(&mut bytes, tag::GEOS, &raw);
    let model = Mdlx::parse_stored_v1800(&bytes).unwrap();
    let geoset = model.geosets(&bytes).unwrap().remove(0);
    assert!(model.geoset_streams_v1800(&geoset).is_ok());
    assert!(
        matches!(model.matrix_palette_v1800(&bytes, &geoset), Err(Error::DuplicateGeosetTag { tag, .. }) if &tag == b"MATS")
    );
    let mut model = Mdlx::parse_stored_v1800(&default_fixture()).unwrap();
    let chunk = model
        .chunks
        .iter_mut()
        .find(|chunk| &chunk.tag == b"BPOS")
        .unwrap();
    chunk.payload_offset = usize::MAX;
    assert!(matches!(
        model.bind_pose_stream_v1800(&[]),
        Err(Error::Truncated { .. })
    ));
}

#[test]
fn undersized_bone_record_cannot_borrow_fields_from_the_next_node() {
    let mut bytes = fixture(Some(&[3]), &[], Some(4), Some([0; 4]), [0; 4]);
    let model = Mdlx::parse_stored_v1800(&bytes).unwrap();
    let empty_bone = model.chunk(tag::BONE).unwrap().offset;
    // Replace the empty trailing BONE/BPOS region while preserving GEOS.
    bytes.truncate(empty_bone);
    let mut raw = vec![0; 8];
    raw.extend(bone_body(&[3]));
    chunk(&mut bytes, tag::BONE, &raw);
    let model = Mdlx::parse_stored_v1800(&bytes).unwrap();
    let geoset = model.geosets(&bytes).unwrap().remove(0);
    assert!(matches!(
        model.primary_joint_join_v1800(&bytes, &geoset),
        Err(Error::RecordShape {
            chunk: "BONE",
            size: 8,
            measured: 104,
            ..
        })
    ));
    assert_eq!(
        model
            .matrix_palette_v1800(&bytes, &geoset)
            .unwrap()
            .unwrap()
            .get(0)
            .unwrap()
            .object_id,
        3
    );
}

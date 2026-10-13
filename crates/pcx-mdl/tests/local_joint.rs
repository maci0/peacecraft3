//! Keyed originals prove the checked source convention, not native skinning.
use pcx_casc::{blte::Limits, storage::Storage};
use pcx_mdl::{Error, Mdlx, PrimaryJointRefusal1800, tag};
use std::collections::BTreeMap;

fn check_fixture(env: &str, expected_path: &str, ckey: &str, ekey: &str, altar: bool) {
    let install =
        std::env::var_os("PEACECRAFT3_REFORGED_DIR").expect("explicit original CASC install");
    let path = std::env::var(env).expect("explicit discovered configured HD path");
    assert_eq!(path, expected_path);
    let storage = Storage::open(
        std::path::Path::new(&install),
        Limits {
            encoded_bytes: 64 * 1024 * 1024,
            decoded_bytes: 256 * 1024 * 1024,
        },
    )
    .unwrap();
    let encoding = storage.load_encoding().unwrap();
    let tvfs = storage.load_tvfs(&encoding).unwrap();
    let asset = storage.read_path(&tvfs, &path).unwrap();
    assert_eq!(asset.file.decoded.content_key, asset.resolved.ckey);
    assert_eq!(pcx_casc::hex(&asset.resolved.ckey), ckey);
    assert_eq!(pcx_casc::hex(&asset.resolved.ekey), ekey);
    let bytes = &asset.file.decoded.bytes;
    assert_eq!(bytes.len() as u64, asset.resolved.decoded_size);
    let model = Mdlx::parse_stored_v1800(bytes).unwrap();
    let bones = model.bones(bytes).unwrap();
    let bone_ids: BTreeMap<_, _> = bones.iter().map(|bone| (bone.object_id, bone)).collect();
    assert_eq!(
        bone_ids.len(),
        bones.len(),
        "fixture BONE IDs must be unambiguous"
    );
    let poses = model.bind_pose_stream_v1800(bytes).unwrap().unwrap();
    assert_eq!(
        poses.data().as_ptr(),
        bytes[poses.payload_offset..].as_ptr()
    );
    assert_eq!(poses.data().len(), 4 + poses.record_count() * 48);
    for index in 0..poses.record_count() {
        let row = poses.get(index).unwrap();
        assert_eq!(row.offset, poses.payload_offset + 4 + index * 48);
        assert_eq!(row.data.as_ptr(), bytes[row.offset..].as_ptr());
        assert_eq!(row.data.as_slice(), &bytes[row.offset..row.offset + 48]);
        for (i, bits) in row.bits().into_iter().enumerate() {
            assert_eq!(
                bits.to_le_bytes(),
                bytes[row.offset + i * 4..row.offset + i * 4 + 4]
            );
        }
    }
    let geosets = model.geosets(bytes).unwrap();
    let geos_base = model.chunk(tag::GEOS).unwrap().payload_offset;
    let bone_base = model.chunk(tag::BONE).unwrap().payload_offset;
    let mut vertices = 0;
    let mut joined = 0;
    let mut refused_nonbone = 0;
    let mut sums = BTreeMap::<u32, usize>::new();
    for (index, geoset) in geosets.iter().enumerate() {
        let base = geos_base + geoset.offset + 4;
        let streams = model.geoset_streams_v1800(geoset).unwrap();
        let skin = streams.skin.unwrap();
        let block = streams
            .blocks
            .iter()
            .find(|block| &block.tag == b"MATS")
            .unwrap();
        let palette = model.matrix_palette_v1800(bytes, geoset).unwrap().unwrap();
        assert_eq!(palette.offset, base + block.offset);
        assert_eq!(palette.payload_offset, palette.offset + 8);
        assert_eq!(palette.geoset_offset, base - 4);
        assert_eq!(palette.data(), block.body);
        assert_eq!(
            palette.data().as_ptr(),
            bytes[palette.payload_offset..].as_ptr()
        );
        for entry_index in 0..palette.record_count() {
            let entry = palette.get(entry_index).unwrap();
            assert_eq!(entry.offset, palette.payload_offset + entry_index * 4);
            assert_eq!(entry.data.as_ptr(), bytes[entry.offset..].as_ptr());
            assert_eq!(entry.object_id.to_le_bytes(), *entry.data);
        }
        let join = model.primary_joint_join_v1800(bytes, geoset).unwrap();
        for vertex in 0..skin.record_count() {
            let raw = skin.get(vertex).unwrap();
            let diagnostics = raw.weight_diagnostics();
            assert_eq!(
                diagnostics.sum,
                raw.weight_words
                    .iter()
                    .map(|word| u32::from(*word))
                    .sum::<u32>()
            );
            assert_eq!(
                diagnostics.nonzero_lanes as usize,
                raw.weight_words.iter().filter(|word| **word != 0).count()
            );
            assert_eq!(
                diagnostics.max_word,
                *raw.weight_words.iter().max().unwrap()
            );
            *sums.entry(diagnostics.sum).or_default() += 1;
            for lane in 0..4 {
                let joint_offset = base + skin.offset + 8 + vertex * 16 + lane * 2;
                assert_eq!(
                    raw.joint_words[lane].to_le_bytes(),
                    bytes[joint_offset..joint_offset + 2]
                );
                assert_eq!(
                    raw.weight_words[lane].to_le_bytes(),
                    bytes[joint_offset + 8..joint_offset + 10]
                );
                let selected = palette.get(usize::from(raw.joint_words[lane])).unwrap();
                if let Some(bone) = bone_ids.get(&selected.object_id) {
                    let result = join.get(vertex, lane).unwrap();
                    assert_eq!(result.version, 1800);
                    assert_eq!(result.raw_joint, raw.joint_words[lane]);
                    assert_eq!(result.raw_weight, raw.weight_words[lane]);
                    assert_eq!(result.object_id, selected.object_id);
                    assert_eq!(result.joint_offset, joint_offset);
                    assert_eq!(result.weight_offset, joint_offset + 8);
                    assert_eq!(result.mats_offset, selected.offset);
                    assert_eq!(result.bone_object_id_offset, bone_base + bone.offset + 84);
                    assert_eq!(
                        result.object_id.to_le_bytes(),
                        bytes[result.bone_object_id_offset..result.bone_object_id_offset + 4]
                    );
                    assert_eq!(result.bind_pose.index, selected.object_id as usize);
                    assert_eq!(
                        result.bind_pose,
                        poses.get(selected.object_id as usize).unwrap()
                    );
                    assert_eq!(
                        result.bind_pose.data.as_ptr(),
                        bytes[result.bind_pose.offset..].as_ptr()
                    );
                    joined += 1;
                } else {
                    assert!(
                        matches!(join.get(vertex, lane), Err(Error::PrimaryJointRefusal { at, vertex: v, lane: l, reason: PrimaryJointRefusal1800::ObjectNotInBoneInventory { object_id }, .. }) if at == joint_offset && v == vertex && l == lane && object_id == selected.object_id)
                    );
                    refused_nonbone += 1;
                }
            }
        }
        vertices += skin.record_count();
        if altar && index == 3 {
            let result = join.get(0, 0).unwrap();
            assert_eq!(palette.record_count(), 1);
            assert_eq!(result.raw_joint, 0);
            assert_eq!(result.raw_weight, 255);
            assert_eq!(result.object_id, 3);
            assert_ne!(u32::from(result.raw_joint), result.object_id);
            assert_eq!(result.mats_offset, 0xdabe8);
            assert_eq!(result.joint_offset, 0xdacbc);
            assert_eq!(result.weight_offset, 0xdacc4);
            assert_eq!(result.bone_object_id_offset, 0xdcc88);
            assert_eq!(result.bind_pose.index, 3);
            assert_eq!(result.bind_pose.offset, 0xdea68);
            assert_eq!(result.bind_pose.data.len(), 48);
            println!(
                "Altar witness: geoset=3 vertex=0 lane=0 raw_joint=0 MATS_object_id=3 BONE_object_id_file=0xdcc88 BPOS_row=3 BPOS_span=[0xdea68,0xdea98) convention=primary native=unverified"
            );
        }
        assert!(
            matches!(model.mesh(bytes, geoset), Err(Error::UnsupportedGeosetFeature { version: 1800, tag, .. }) if &tag == b"TANG" || &tag == b"SKIN")
        );
    }
    if altar {
        assert_eq!(bones.len(), 4);
        assert_eq!(poses.record_count(), 22);
        assert_eq!(geosets.len(), 4);
        assert!(bone_ids.contains_key(&3));
    } else {
        assert_eq!(geosets.len(), 36);
        assert_eq!(vertices, 40142);
        assert_eq!(joined, 160568);
        assert_eq!(refused_nonbone, 0);
    }
    assert_eq!(joined + refused_nonbone, vertices * 4);
    println!(
        "source={path} build={} ckey={ckey} ekey={ekey} version={} BONE={} BPOS={} geosets={} vertices={vertices} joined_lanes={joined} refused_nonbone={refused_nonbone} integer_weight_sums={sums:?} convention=primary local-import-policy native/render=unverified",
        storage.build.info.source.version,
        model.version,
        bones.len(),
        poses.record_count(),
        geosets.len()
    );
}

#[test]
#[ignore = "requires PEACECRAFT3_REFORGED_DIR and PEACECRAFT3_REFORGED_ALTAR_ASSET for the keyed configured HD Altar portrait"]
fn original_casc_altar_primary_joint_join_confirms_bone3_and_bpos3() {
    check_fixture(
        "PEACECRAFT3_REFORGED_ALTAR_ASSET",
        "war3.w3mod:_hd.w3mod:buildings/human/altarofkings/altarofkings_portrait.mdx",
        "751c8db8dbeced3e7619165e9251aefc",
        "c6f84b1b7f75d22e73f603be03328717",
        true,
    );
}

#[test]
#[ignore = "requires PEACECRAFT3_REFORGED_DIR and PEACECRAFT3_REFORGED_HD_ASSET for the keyed configured HD Footman"]
fn original_casc_footman_primary_joint_join_preserves_every_lane_and_raw_row() {
    check_fixture(
        "PEACECRAFT3_REFORGED_HD_ASSET",
        "war3.w3mod:_hd.w3mod:units/human/footman/footman.mdx",
        "6fcd268161babbd73f4601060be39df5",
        "55ddc4ab832e5eadc526d91d40cdd9dd",
        false,
    );
}

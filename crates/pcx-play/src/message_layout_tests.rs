use super::*;
use std::collections::BTreeMap;

fn origin() -> AssetIdentity {
    AssetIdentity {
        source: "synthetic client".into(),
        edition: "fixture".into(),
        version: "fixture".into(),
    }
}

pub(crate) fn fixture(unit_width: f32) -> BTreeMap<u32, Vec<u8>> {
    let mut records = BTreeMap::new();
    for (va, target) in [(0x5e7d2e_u32, 0x873750_u32), (0x5f4900, 0x873a10)] {
        records.insert(
            va,
            [
                vec![0xe8],
                target.wrapping_sub(va + 5).to_le_bytes().to_vec(),
            ]
            .concat(),
        );
    }
    for (va, pointer) in [
        (0x5e7ca5_u32, 0x100_u32),
        (0x5e8395, 0x120),
        (0x5e85ac, 0x140),
    ] {
        records.insert(va, [vec![0x68], pointer.to_le_bytes().to_vec()].concat());
    }
    records.insert(0x100, b"SimpleConsole\0".to_vec());
    records.insert(0x120, b"WorldFrameMessage\0".to_vec());
    records.insert(0x140, b"WorldFrameUnitMessage\0".to_vec());
    for (va, prefix, value) in [
        (0x5e7cbd, &[0x89, 0xb7][..], 0x468_u32),
        (0x5e7d28, &[0x8b, 0x8f][..], 0x468),
        (0x5e841f, &[0xff, 0xb7][..], 0x468),
        (0x5e838f, &[0x89, 0x87][..], 0x424),
        (0x5e85a6, &[0x89, 0x87][..], 0x428),
        (0x5e8636, &[0xff, 0xb7][..], 0x424),
        (0x5f4874, &[0x8b, 0x8e][..], 0x424),
        (0x5f48f8, &[0xff, 0xb6][..], 0x424),
    ] {
        records.insert(va, [prefix, &value.to_le_bytes()].concat());
    }
    for (va, value) in [
        (0x5e841d_u32, 7_u8),
        (0x5e8425, 7),
        (0x5e8634, 1),
        (0x5e863c, 7),
        (0x5f48f6, 1),
        (0x5f48fe, 7),
    ] {
        records.insert(va, vec![0x6a, value]);
    }
    for (va, prefix, value) in [
        (0x5e83e4, &[0xc7, 0x04, 0x24][..], 0.38_f32),
        (0x5e83f7, &[0xc7, 0x04, 0x24][..], 0.05),
        (0x5e8416, &[0xc7, 0x04, 0x24][..], 0.),
        (0x5e840e, &[0xc7, 0x44, 0x24, 0x04][..], 0.18),
        (0x5e85fb, &[0xc7, 0x04, 0x24][..], unit_width),
        (0x5e860e, &[0xc7, 0x04, 0x24][..], 0.25),
        (0x5e862d, &[0xc7, 0x04, 0x24][..], -0.098125),
        (0x5e8625, &[0xc7, 0x44, 0x24, 0x04][..], 0.0005),
    ] {
        records.insert(va, [prefix, &value.to_le_bytes()].concat());
    }
    for (va, prefix, pointer, value) in [
        (0xab297d, &[0xf3, 0x0f, 0x5e, 0x05][..], 0x200_u32, 0.8_f32),
        (0xab29a0, &[0xf3, 0x0f, 0x5e, 0x05][..], 0x204, 0.6),
        (0x5f48c0, &[0xf3, 0x0f, 0x10, 0x05][..], 0x200, 0.8),
        (0x5f4887, &[0xf3, 0x0f, 0x58, 0x15][..], 0x208, 0.0005),
        (0x5f48a5, &[0xf3, 0x0f, 0x5c, 0x2d][..], 0x20c, 0.098125),
        (0x5f4894, &[0xf3, 0x0f, 0x10, 0x05][..], 0x210, 0.452),
    ] {
        records.insert(va, [prefix, &pointer.to_le_bytes()].concat());
        records.insert(pointer, value.to_le_bytes().to_vec());
    }
    records
}

fn decode(records: &BTreeMap<u32, Vec<u8>>) -> Result<Rules, String> {
    Rules::decode(origin(), [0; 32], |va, len, code| {
        assert_eq!(code, va >= 0x400000);
        records
            .range(..=va)
            .next_back()
            .and_then(|(&start, bytes)| bytes.get((va - start) as usize..))
            .and_then(|bytes| bytes.get(..len))
            .map(<[u8]>::to_vec)
            .ok_or_else(|| format!("missing fixture operand at {va:#x}"))
    })
}

pub(crate) fn rules(unit_width: f32) -> Rules {
    decode(&fixture(unit_width)).expect("synthetic message layout")
}

fn near(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 0.000001,
        "{actual} != {expected}"
    );
}

#[test]
fn script_origin_uses_the_source_frame_inset() {
    let rules = decode(&fixture(0.5)).unwrap();
    assert_eq!(rules.origin, origin());
    assert_eq!(rules.console_key, "SimpleConsole");
    assert_eq!(rules.message_key, "WorldFrameMessage");
    assert_eq!(rules.viewport, [0.8, 0.6]);
    let origin = rules.placement(0., 0.).unwrap();
    near(origin.left, 0.051875);
    near(origin.bottom, 0.2305);
    near(origin.width, 0.5);
    near(origin.height, 0.25);
    near(origin.viewport[0], 0.8);
    near(origin.viewport[1], 0.6);
}

#[test]
fn script_axes_scale_across_the_source_message_area() {
    let rules = decode(&fixture(0.5)).unwrap();
    let right = rules.placement(1., 0.).unwrap();
    near(right.left, 0.36);
    near(right.bottom, 0.2305);
    let top = rules.placement(0., 1.).unwrap();
    near(top.left, 0.051875);
    near(top.bottom, 0.452);
    near(rules.placement(2., 0.).unwrap().left, 0.668125);
    near(rules.placement(-1., -1.).unwrap().left, 0.051875);
    near(rules.placement(-1., -1.).unwrap().bottom, 0.2305);
    assert!(rules.placement(f32::NAN, 0.).is_err());
    assert!(rules.placement(0., f32::INFINITY).is_err());
}

#[test]
fn changing_a_measured_field_changes_the_placement() {
    let mut records = fixture(0.5);
    records.get_mut(&0x5e83e4).unwrap()[3..].copy_from_slice(&0.5_f32.to_le_bytes());
    let rules = decode(&records).unwrap();
    near(rules.placement(1., 0.).unwrap().left, 0.3);
    records.get_mut(&0x5e862d).unwrap()[3..].copy_from_slice(&(-0.05_f32).to_le_bytes());
    records.get_mut(&0x5f48a5).unwrap()[4..].copy_from_slice(&0x20c_u32.to_le_bytes());
    records.insert(0x20c, 0.05_f32.to_le_bytes().to_vec());
    let rules = decode(&records).unwrap();
    near(rules.placement(0., 0.).unwrap().left, 0.1);
}

#[test]
fn viewport_and_travel_constants_come_from_the_image() {
    let mut records = fixture(0.5);
    records.insert(0x200, 1.6_f32.to_le_bytes().to_vec());
    records.insert(0x204, 0.9_f32.to_le_bytes().to_vec());
    let rules = decode(&records).unwrap();
    assert_eq!(rules.viewport, [1.6, 0.9]);
    let moved = rules.placement(1., 1.).unwrap();
    near(moved.left, 1.16);
    near(moved.bottom, 0.452);
}

#[test]
fn frame_keys_anchor_pairs_and_calls_must_match() {
    for (address, byte) in [
        (0x120_u32, b'X'),
        (0x140, b'X'),
        (0x100, b'X'),
        (0x5e841d, 8),
        (0x5e8425, 8),
        (0x5e8634, 8),
        (0x5e863c, 1),
        (0x5f48f6, 8),
        (0x5f48fe, 1),
        (0x5e841f, 0x69),
        (0x5e8636, 0x42),
        (0x5f4874, 0x42),
    ] {
        let mut records = fixture(0.5);
        let record = records.get_mut(&address).unwrap();
        let last = record.len() - 1;
        record[last] = byte;
        assert!(decode(&records).is_err(), "accepted change at {address:#x}");
    }
    for address in [0x5e7d2e, 0x5f4900] {
        let mut records = fixture(0.5);
        records.get_mut(&address).unwrap()[1] ^= 1;
        assert!(
            decode(&records).is_err(),
            "accepted call change at {address:#x}"
        );
    }
    for address in [0x5e7cbd, 0x5e7d28, 0x5e838f, 0x5e85a6, 0x5f48f8] {
        let mut records = fixture(0.5);
        let record = records.get_mut(&address).unwrap();
        record[2] ^= 0x10;
        assert!(
            decode(&records).is_err(),
            "accepted field change at {address:#x}"
        );
    }
}

#[test]
fn nonfinite_empty_and_degenerate_geometry_refuses() {
    for (address, value) in [
        (0x5e83e4, f32::NAN),
        (0x5e83e4, 0.),
        (0x5e83e4, -0.38),
        (0x5e83f7, f32::INFINITY),
        (0x5e85fb, 0.),
        (0x5e860e, -0.25),
        (0x200, 0.),
        (0x204, -0.6),
        (0x208, -0.0005),
        (0x20c, 0.2),
        (0x210, 0.2),
    ] {
        let mut records = fixture(0.5);
        let record = records.get_mut(&address).unwrap();
        let tail = record.len() - 4;
        record[tail..].copy_from_slice(&value.to_le_bytes());
        assert!(
            decode(&records).is_err(),
            "accepted {value} at {address:#x}"
        );
    }
    for text in [b"\0".as_slice(), b"\xff\0", b"bad\nkey\0", b"unterminated"] {
        let mut records = fixture(0.5);
        records.insert(0x120, text.to_vec());
        assert!(decode(&records).is_err());
    }
}

#[test]
fn truncated_records_refuse() {
    for address in fixture(0.5).keys() {
        let mut records = fixture(0.5);
        records.get_mut(address).unwrap().pop();
        assert!(decode(&records).is_err(), "accepted truncated {address:#x}");
    }
}

#[test]
fn unreviewed_client_refuses_before_decoding() {
    assert!(Rules::reviewed_legacy(&[0; 64], origin()).is_err());
}

#[test]
#[ignore = "requires PCX_WAR3_EXE pointing at the reviewed local client"]
fn selected_original_message_frames_scale_with_position() {
    let path = std::env::var_os("PCX_WAR3_EXE").expect("set PCX_WAR3_EXE");
    let path = std::path::Path::new(&path);
    let image = std::fs::read(path).unwrap();
    let rules = Rules::reviewed_legacy(
        &image,
        AssetIdentity {
            source: path.display().to_string(),
            edition: "Legacy".into(),
            version: "1.29.2.9231".into(),
        },
    )
    .unwrap();
    assert_eq!(rules.image_sha256, sha256(&image));
    let origin = rules.placement(0., 0.).unwrap();
    let far = rules.placement(1., 1.).unwrap();
    assert!((origin.left - 0.051875).abs() < 1e-5);
    assert!((origin.bottom - 0.2305).abs() < 1e-5);
    assert_eq!(origin.width, 0.5);
    assert_eq!(origin.height, 0.25);
    assert!((far.left - 0.36).abs() < 1e-5);
    assert!((far.bottom - 0.452).abs() < 1e-5);
    assert!(far.left > origin.left && far.bottom > origin.bottom);
    assert_eq!(origin.width, far.width);
    assert_eq!(origin.height, far.height);
}

use super::*;
use std::collections::BTreeMap;

fn origin() -> AssetIdentity {
    AssetIdentity {
        source: "synthetic client".into(),
        edition: "fixture".into(),
        version: "fixture".into(),
    }
}

fn fixture() -> BTreeMap<u32, Vec<u8>> {
    let mut records = BTreeMap::new();
    for (va, target) in [
        (0x5e805d_u32, 0x61d130_u32),
        (0x61d24d, 0x874900),
        (0x5eafb6, 0x635390),
        (0x5eafd5, 0x5d3c80),
        (0x5d3caf, 0x5d3e30),
        (0x5d3e60, 0x8750b0),
    ] {
        records.insert(
            va,
            [
                vec![0xe8],
                target.wrapping_sub(va + 5).to_le_bytes().to_vec(),
            ]
            .concat(),
        );
    }
    records.insert(
        0x5e8074,
        [vec![0x89, 0x87], 0x88_u32.to_le_bytes().to_vec()].concat(),
    );
    records.insert(
        0x5eafab,
        [vec![0x8b, 0x8b], 0x88_u32.to_le_bytes().to_vec()].concat(),
    );
    records.insert(0x887ce5, vec![0x83, 0xe0, 4]);
    records.insert(0x8750db, vec![0x6a, 4]);
    records.insert(0x875141, vec![0x6a, 4]);
    records.insert(
        0x5e7ca5,
        [vec![0x68], 0x300_u32.to_le_bytes().to_vec()].concat(),
    );
    records.insert(0x300, b"FixtureConsole\0".to_vec());
    for (va, prefix, value) in [
        (0x61d1c4, &[0x6a][..], 2),
        (0x61d1c6, &[0x6a][..], 3),
        (0x61d357, &[0x83, 0xff][..], 3),
        (0x61d364, &[0x83, 0xfe][..], 2),
        (0x61d207, &[0x6a][..], 8),
        (0x61d20a, &[0x6a][..], 8),
    ] {
        records.insert(va, [prefix, &[value]].concat());
    }
    for (va, prefix, value) in [
        (0x61d1d6, &[0xc7, 0x04, 0x24][..], 0.20_f32),
        (0x61d1e5, &[0xc7, 0x04, 0x24][..], 0.15),
        (0x61d200, &[0xc7, 0x04, 0x24][..], -0.02),
        (0x61d1f8, &[0xc7, 0x44, 0x24, 0x04][..], 0.01),
        (0x61d2e1, &[0xc7, 0x04, 0x24][..], 0.04),
        (0x61d2f0, &[0xc7, 0x04, 0x24][..], 0.03),
        (0x88c7e2, &[0xc7, 0x86, 0x40, 1, 0, 0][..], 0.01),
        (0x88c7ec, &[0xc7, 0x86, 0x44, 1, 0, 0][..], 0.02),
        (0x88c7f6, &[0xc7, 0x86, 0x48, 1, 0, 0][..], 0.03),
        (0x88c800, &[0xc7, 0x86, 0x4c, 1, 0, 0][..], 0.01),
    ] {
        records.insert(va, [prefix, &value.to_le_bytes()].concat());
    }
    for (va, prefix, pointer, value) in [
        (0x640ea8, &[0xf3, 0x0f, 0x10, 5][..], 0x100_u32, 1.2_f32),
        (0x640e96, &[0xf3, 0x0f, 0x10, 0x1d][..], 0x200, 0.9),
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

fn near(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 0.000001,
        "{actual} != {expected}"
    );
}

#[test]
fn source_dimensions_drive_the_grid_and_rows_start_at_the_top() {
    let rules = decode(&fixture()).unwrap();
    assert_eq!((rules.columns, rules.rows), (2, 3));
    assert_eq!(rules.origin, origin());
    assert_eq!(rules.console_key, "FixtureConsole");
    assert!(rules.icon_repeat);
    let first = rules.slot(0, 0).unwrap();
    near(first.x, 0.99);
    near(first.y, 0.11);
    near(first.width, 0.04);
    near(first.height, 0.03);
    let last = rules.slot(1, 2).unwrap();
    near(last.x, 1.07);
    near(last.y, 0.03);
    assert!(rules.slot(2, 0).is_err());
    assert!(rules.slot(0, 3).is_err());
}

#[test]
fn texture_flags_and_both_setter_paths_come_from_the_image() {
    let mut records = fixture();
    records.get_mut(&0x8750db).unwrap()[1] = 0;
    assert!(decode(&records).unwrap_err().contains("sampler relation"));
    records.get_mut(&0x875141).unwrap()[1] = 0;
    assert!(!decode(&records).unwrap().icon_repeat);
    records.get_mut(&0x5eafab).unwrap()[2..].copy_from_slice(&0x90_u32.to_le_bytes());
    assert!(decode(&records).unwrap_err().contains("another UI field"));
}

#[test]
fn changing_an_operand_changes_the_result_without_a_copied_geometry_fallback() {
    let mut records = fixture();
    records.get_mut(&0x61d1d6).unwrap()[3..].copy_from_slice(&0.4_f32.to_le_bytes());
    let rules = decode(&records).unwrap();
    near(rules.slot(0, 0).unwrap().x, 0.79);
    near(rules.slot(1, 0).unwrap().x, 0.97);
}

#[test]
fn unsupported_instructions_and_truncated_inputs_refuse() {
    for address in fixture().keys() {
        let mut records = fixture();
        records.get_mut(address).unwrap().pop();
        assert!(decode(&records).is_err(), "accepted truncated {address:#x}");
    }
    let mut records = fixture();
    records.get_mut(&0x61d2e1).unwrap()[0] ^= 1;
    assert!(decode(&records).unwrap_err().contains("instruction"));
}

#[test]
fn grid_and_anchor_relations_must_match() {
    for address in [0x61d1c4, 0x61d1c6, 0x61d357, 0x61d364, 0x61d207, 0x61d20a] {
        let mut records = fixture();
        *records.get_mut(&address).unwrap().last_mut().unwrap() = 0;
        assert!(decode(&records).is_err(), "accepted mismatch {address:#x}");
    }
}

#[test]
fn nonfinite_nonpositive_and_oversized_buttons_refuse() {
    for value in [f32::NAN, f32::INFINITY, -0.02, 0., 0.5] {
        let mut records = fixture();
        records.get_mut(&0x61d2e1).unwrap()[3..].copy_from_slice(&value.to_le_bytes());
        assert!(decode(&records).is_err(), "accepted button width {value}");
    }
    for value in [f32::NAN, f32::INFINITY, -0.01, 0.4] {
        let mut records = fixture();
        records.get_mut(&0x88c7e2).unwrap()[6..].copy_from_slice(&value.to_le_bytes());
        assert!(decode(&records).is_err(), "accepted inset {value}");
    }
}

#[test]
fn unreviewed_client_refuses_before_decoding() {
    assert!(Rules::reviewed_legacy(&[0; 64], origin()).is_err());
}

#[test]
fn parent_string_needs_a_terminated_nonempty_valid_key() {
    for text in [b"\0".as_slice(), b"\xff\0", b"bad\nkey\0", b"unterminated"] {
        let mut records = fixture();
        records.insert(0x300, text.to_vec());
        assert!(decode(&records).is_err());
    }
}

#[test]
#[ignore = "requires PCX_WAR3_EXE pointing at the reviewed local client"]
fn selected_original_command_geometry_has_ordered_slots() {
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
    let first = rules.slot(0, 0).unwrap();
    let last = rules.slot(rules.columns - 1, rules.rows - 1).unwrap();
    assert!(first.x < last.x && first.y > last.y);
    assert!(first.x >= 0. && last.x + last.width <= rules.viewport[0]);
    assert!(last.y >= 0. && first.y + first.height <= rules.viewport[1]);
}

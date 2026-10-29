use super::*;
use crate::message_layout::tests::fixture as layout_fixture;
use std::collections::BTreeMap;

fn origin() -> AssetIdentity {
    AssetIdentity {
        source: "synthetic client".into(),
        edition: "fixture".into(),
        version: "fixture".into(),
    }
}

/// An edit applied to one fixture record.
type Change = fn(&mut Vec<u8>);

fn code(prefix: &[u8], operand: u32) -> Vec<u8> {
    [prefix, &operand.to_le_bytes()].concat()
}

fn fixture() -> BTreeMap<u32, Vec<u8>> {
    let mut records = layout_fixture(0.5);
    for (va, target) in [
        (POST_CLEAR_CALL, CLASS_CLEAR_ROUTINE),
        (POST_UNKNOWN_CLEAR_CALL, CLASS_CLEAR_ROUTINE),
        (UI_SHADOW_CALL, CLASS_SHADOW_ROUTINE),
    ] {
        records.insert(va, code(&[0xe8], target.wrapping_sub(va + 5)));
    }
    for va in [POST_METHOD_SLOT, POST_UNKNOWN_METHOD_SLOT] {
        records.insert(va, vec![0xff, 0x50, POST_SLOT_OFFSET as u8]);
    }
    for (va, prefix) in [
        (POST_FRAME_FIELD, &[0x8b, 0x8e][..]),
        (UI_SHADOW_FRAME, &[0x8b, 0x8f][..]),
    ] {
        records.insert(va, code(prefix, UI_ERROR_FRAME_FIELD));
    }
    records.insert(CLASS_VTABLE_STORE, code(&[0xc7, 0x06], 0x300));
    records.insert(
        0x300 + POST_SLOT_OFFSET,
        CLASS_POST_METHOD.to_le_bytes().to_vec(),
    );
    records.insert(POST_COLOUR, code(&[0xc7, 0x45, 0x0c], 0xfffe_d312));
    for va in [POST_HOLD, POST_UNKNOWN_HOLD] {
        records.insert(va, code(&[0xc7, 0x04, 0x24], 10.0_f32.to_bits()));
    }
    records.insert(
        CLASS_FADE,
        code(&fade_prefix(FIELD_FADE), 3.0_f32.to_bits()),
    );
    records.insert(
        CLASS_JUSTIFICATION,
        code(&fade_prefix(FIELD_JUSTIFICATION), JUSTIFY_LEFT),
    );
    records.insert(CLASS_BOTTOM_UP, code(&fade_prefix(FIELD_BOTTOM_UP), 1));
    records.insert(UI_FONT_FLAGS, vec![0x6a, 1]);
    records.insert(UI_FONT_KEY, code(&[0x68], 0x160));
    records.insert(0x160, b"MessageFont\0".to_vec());
    records.insert(
        UI_SHADOW_X,
        code(&[0xc7, 0x85, 0xe0, 0xfe, 0xff, 0xff], 0.001_f32.to_bits()),
    );
    records.insert(
        UI_SHADOW_Y,
        code(
            &[0xc7, 0x85, 0xe4, 0xfe, 0xff, 0xff],
            (-0.001_f32).to_bits(),
        ),
    );
    records.insert(UI_SHADOW_COLOUR, code(&[0xc7, 0x00], 0xff00_0000));
    records
}

fn decode(records: &BTreeMap<u32, Vec<u8>>) -> Result<ErrorLineRules, String> {
    ErrorLineRules::decode(origin(), [0; 32], |va, len, is_code| {
        assert_eq!(is_code, va >= 0x400000);
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
        (actual - expected).abs() < 0.000_001,
        "{actual} != {expected}"
    );
}

#[test]
fn values_come_from_the_posting_routine_and_the_frame_class() {
    let rules = decode(&fixture()).unwrap();
    assert_eq!(rules.colour, [0xfe, 0xd3, 0x12, 0xff]);
    assert_eq!(rules.hold_seconds, 10.0);
    assert_eq!(rules.fade_seconds, 3.0);
    assert_eq!(rules.height_key, "WorldFrameMessage");
    assert_eq!(rules.font_key, "MessageFont");
    assert_eq!(rules.shadow.colour, [0, 0, 0, 0xff]);
    near(rules.shadow.offset[0], 0.001);
    near(rules.shadow.offset[1], -0.001);
}

#[test]
fn the_frame_sits_above_the_console_centre() {
    let frame = decode(&fixture()).unwrap().frame;
    near(frame.left, 0.21);
    near(frame.bottom, 0.18);
    near(frame.width, 0.38);
    near(frame.height, 0.05);
    assert_eq!(frame.viewport, [0.8, 0.6]);
}

#[test]
fn opacity_holds_then_falls_linearly_then_ends() {
    let rules = decode(&fixture()).unwrap();
    assert_eq!(rules.opacity(0.0), Some(1.0));
    assert_eq!(rules.opacity(10.0), Some(1.0));
    near(rules.opacity(11.5).unwrap(), 0.5);
    near(rules.opacity(12.99).unwrap(), 0.01 / 3.0);
    assert_eq!(rules.opacity(13.0), None);
    assert_eq!(rules.opacity(-0.1), None);
    assert_eq!(rules.opacity(f32::NAN), None);
}

#[test]
fn changing_a_measured_operand_changes_the_rules() {
    let mut records = fixture();
    records.insert(POST_COLOUR, code(&[0xc7, 0x45, 0x0c], 0x8011_2233));
    for va in [POST_HOLD, POST_UNKNOWN_HOLD] {
        records.insert(va, code(&[0xc7, 0x04, 0x24], 4.0_f32.to_bits()));
    }
    records.insert(
        CLASS_FADE,
        code(&fade_prefix(FIELD_FADE), 1.0_f32.to_bits()),
    );
    let rules = decode(&records).unwrap();
    assert_eq!(rules.colour, [0x11, 0x22, 0x33, 0x80]);
    assert_eq!(rules.hold_seconds, 4.0);
    assert_eq!(rules.fade_seconds, 1.0);
    near(rules.opacity(4.5).unwrap(), 0.5);
}

#[test]
fn a_changed_instruction_refuses() {
    let cases: [(u32, Change); 12] = [
        (POST_CLEAR_CALL, |b| b[1] ^= 1),
        (UI_SHADOW_CALL, |b| b[1] ^= 1),
        (POST_METHOD_SLOT, |b| b[2] = 0x74),
        (POST_UNKNOWN_METHOD_SLOT, |b| b[2] = 0x74),
        (POST_FRAME_FIELD, |b| b[2] ^= 0x10),
        (UI_SHADOW_FRAME, |b| b[0] = 0x8d),
        (CLASS_VTABLE_STORE, |b| b[2] ^= 1),
        (CLASS_JUSTIFICATION, |b| b[6] = 2),
        (CLASS_BOTTOM_UP, |b| b[6] = 0),
        (UI_FONT_FLAGS, |b| b[1] = 0),
        (POST_UNKNOWN_HOLD, |b| b[5] ^= 1),
        (CLASS_FADE, |b| b[6..].copy_from_slice(&0_f32.to_le_bytes())),
    ];
    for (va, change) in cases {
        let mut records = fixture();
        change(records.get_mut(&va).unwrap());
        assert!(decode(&records).is_err(), "accepted a change at {va:#x}");
    }
}

#[test]
fn a_method_slot_that_is_not_the_reviewed_routine_refuses() {
    let mut records = fixture();
    records.insert(
        0x300 + POST_SLOT_OFFSET,
        0x88b511_u32.to_le_bytes().to_vec(),
    );
    let error = decode(&records).unwrap_err();
    assert!(error.contains("post method"), "{error}");
}

#[test]
fn a_font_key_must_be_text() {
    for key in [b"\0".to_vec(), b"Mess\x01age\0".to_vec(), vec![b'a'; 300]] {
        let mut records = fixture();
        records.insert(0x160, key);
        assert!(decode(&records).is_err());
    }
}

#[test]
fn other_images_refuse_before_decoding() {
    let error = ErrorLineRules::reviewed_legacy(&[0; 16], origin()).unwrap_err();
    assert!(error.contains("unsupported client image"), "{error}");
}

/// Needs the Legacy 1.29.2.9231 `Warcraft III.exe` named by `PCX_WC3_INSTALL`.
#[test]
#[ignore = "requires PCX_WC3_INSTALL with the reviewed Legacy Warcraft III.exe"]
fn reviewed_client_yields_the_documented_rules() {
    let install = std::env::var("PCX_WC3_INSTALL").expect("PCX_WC3_INSTALL");
    let path = std::path::Path::new(&install).join("Warcraft III.exe");
    let image = std::fs::read(&path).unwrap();
    let rules = ErrorLineRules::reviewed_legacy(
        &image,
        AssetIdentity {
            source: path.display().to_string(),
            edition: "selected client".into(),
            version: "reviewed Legacy 1.29.2.9231".into(),
        },
    )
    .unwrap();
    assert_eq!(rules.colour, [0xfe, 0xd3, 0x12, 0xff]);
    assert_eq!((rules.hold_seconds, rules.fade_seconds), (10.0, 3.0));
    assert_eq!(rules.height_key, "WorldFrameMessage");
    assert_eq!(rules.font_key, "MessageFont");
    near(rules.frame.left, 0.21);
    near(rules.frame.bottom, 0.18);
}

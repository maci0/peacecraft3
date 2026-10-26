use super::*;
use std::collections::BTreeMap;

fn fixture() -> BTreeMap<u32, u8> {
    let mut data = BTreeMap::new();
    let mut put = |va: u32, prefix: &[u8], value: u32| {
        for (offset, byte) in prefix
            .iter()
            .copied()
            .chain(value.to_le_bytes())
            .enumerate()
        {
            data.insert(va + offset as u32, byte);
        }
    };
    for (va, target) in [
        (0x5e7b68_u32, 0x873a10_u32),
        (0x5e7ba0, 0x873a10),
        (0x5f3ddb, 0x873a10),
        (0x652c8e, 0x62aa80),
    ] {
        put(va, &[0xe8], target.wrapping_sub(va + 5));
    }
    let stack = &[0xc7, 0x04, 0x24][..];
    let y = &[0xc7, 0x44, 0x24, 0x04][..];
    for (va, prefix, value) in [
        (0x5e7b50, stack, 0.),
        (0x5e7b48, y, 2.),
        (0x5e7b86, stack, 0.),
        (0x5e7b7c, y, -3.),
        (0x5f3d8b, stack, -4.),
        (0x5f3d81, y, -5.),
        (0x6332ab, stack, 1.),
        (0x6332a3, y, 2.),
        (0x63329b, &[0xc7, 0x44, 0x24, 0x08], 3.),
        (0x633293, &[0xc7, 0x44, 0x24, 0x0c], 4.),
        (0x6332bf, stack, 7.),
        (0x6332d5, stack, 2.),
        (0x652bf0, stack, -2.),
        (0x652be8, y, -3.),
        (0x652c27, stack, 0.),
        (0x652c1f, y, -2.),
        (0x652c6d, stack, 0.),
        (0x652c63, y, 1.),
        (0x6579a8, stack, 2.),
        (0x6579d8, stack, -3.),
        (0x6579f8, stack, -2.),
        (0x657907, stack, -4.),
    ] {
        put(va, prefix, f32::to_bits(value));
    }
    put(0x62aa88, &[0xbf], 8);
    put(0x629671, &[0xc7, 0x46, 0x60], 0);
    put(0x62aaad, &[0xf3, 0x0f, 0x10, 0x15], 0x100);
    put(0x62aaa3, &[0xf3, 0x0f, 0x10, 0x15], 0x110);
    put(0x100, &[], 4_f32.to_bits());
    put(0x110, &[], 10_f32.to_bits());
    put(
        0x87a764,
        &[0xc7, 0x86, 0x4c, 0x02, 0x00, 0x00],
        0_f32.to_bits(),
    );
    put(0x639794, &[0xc7, 0x00], 0xff123456);
    put(0x639776, &[0xc7, 0x45, 0xe0], 0.3_f32.to_bits());
    put(0x63977d, &[0xc7, 0x45, 0xe4], (-0.4_f32).to_bits());
    put(0x6395d1, &[0x68], 0x200);
    put(0x6395fc, &[0x68], 0x220);
    for (va, bytes) in [
        (0x897a82, &[0x8b, 0x86, 0x20, 0x01, 0x00, 0x00][..]),
        (0x897a88, &[0x89, 0x83, 0x48, 0x02, 0x00, 0x00]),
        (0x897aa1, &[0x8b, 0x86, 0x24, 0x01, 0x00, 0x00]),
        (0x897aa7, &[0x89, 0x83, 0x4c, 0x02, 0x00, 0x00]),
        (0x87ae25, &[0xf3, 0x0f, 0x10, 0x87, 0x4c, 0x02, 0x00, 0x00]),
        (0x87ae3b, &[0xf3, 0x0f, 0x10, 0x97, 0x48, 0x02, 0x00, 0x00]),
    ] {
        for (i, b) in bytes.iter().copied().enumerate() {
            data.insert(va + i as u32, b);
        }
    }
    for (va, text) in [
        (0xe841a0, "SyntheticSection"),
        (0x200, "SyntheticHeight"),
        (0x220, "SyntheticFont"),
    ] {
        for (offset, byte) in text.bytes().chain([0]).enumerate() {
            data.insert(va + offset as u32, byte);
        }
    }
    data
}
fn decode(data: &BTreeMap<u32, u8>) -> Result<Rules, String> {
    Rules::decode(
        AssetIdentity {
            source: "synthetic".into(),
            edition: "test".into(),
            version: "1".into(),
        },
        sha256(b"synthetic"),
        |va, len, _| {
            (0..len)
                .map(|i| {
                    data.get(&(va + i as u32))
                        .copied()
                        .ok_or_else(|| format!("missing {va:#x}"))
                })
                .collect()
        },
    )
}
#[test]
fn geometry_uses_operands_and_measured_columns() {
    let rules = decode(&fixture()).unwrap();
    assert_eq!(rules.font_key, "SyntheticFont");
    assert!(!rules.creation_icons);
    assert_eq!(rules.backdrop_tile_size(None, 13.).unwrap(), 13.);
    assert_eq!(rules.backdrop_tile_size(Some(7.), 13.).unwrap(), 7.);
    assert_eq!(rules.row_shadow, [0x12, 0x34, 0x56, 0xff]);
    let p = rules.placement([120., 90.], 40., 2, None).unwrap();
    assert_eq!(
        p.root,
        Rect {
            x: 76.,
            y: 56.,
            width: 40.,
            height: 26.
        }
    );
    assert_eq!(
        p.list,
        Rect {
            x: 76.,
            y: 57.,
            width: 40.,
            height: 23.
        }
    );
    assert_eq!(
        rules.row(p.list, 0).unwrap(),
        Rect {
            x: 78.,
            y: 72.,
            width: 36.,
            height: 7.
        }
    );
    assert_eq!(rules.name_span(36., None).unwrap(), [2., 34.]);
    assert_eq!(rules.name_span(36., Some(9.)).unwrap(), [2., 20.]);
    assert!(rules.name_span(10., Some(50.)).is_err());
    assert!(rules.row(p.list, 7).is_err());
}
#[test]
fn malformed_operand_and_nonfinite_source_refuse() {
    let mut data = fixture();
    data.insert(0x5f3ddb, 0x90);
    assert!(decode(&data).unwrap_err().contains("instruction"));
    let mut data = fixture();
    for (i, byte) in f32::NAN.to_bits().to_le_bytes().into_iter().enumerate() {
        data.insert(0x6332bf + 3 + i as u32, byte);
    }
    assert!(decode(&data).unwrap_err().contains("nonfinite"));
}

#[test]
#[ignore = "requires PCX_WAR3_EXE pointing at the reviewed local client"]
fn leaderboard_selected_client_rules_are_read_from_original_bytes() {
    let path = std::env::var_os("PCX_WAR3_EXE").expect("PCX_WAR3_EXE");
    let bytes = std::fs::read(path).unwrap();
    let rules = Rules::reviewed_legacy(
        &bytes,
        AssetIdentity {
            source: "explicit local client qualification".into(),
            edition: "legacy".into(),
            version: "1.29.2.9231".into(),
        },
    )
    .unwrap();
    assert!(!rules.font_key.is_empty());
    assert!(rules.row_height > 0.);
    assert!(rules.backdrop_tile_size(None, 0.037).unwrap() > 0.);
}

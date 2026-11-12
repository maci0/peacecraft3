use super::*;
use std::collections::BTreeMap;

fn windows() -> BTreeMap<u32, Vec<u8>> {
    let mut plain = vec![0; 24];
    plain[..8].copy_from_slice(&[0x55, 0x8b, 0xec, 0x8b, 0x4d, 8, 0x6a, 37]);
    let mut extended = vec![0; 25];
    extended[..9].copy_from_slice(&[0x55, 0x8b, 0xec, 0xff, 0x75, 12, 0x8b, 0x4d, 8]);
    let mut setter = vec![0; 34];
    setter[..4].copy_from_slice(&[0x55, 0x8b, 0xec, 0xa1]);
    setter[8..18].copy_from_slice(&[0x85, 0xc0, 0x74, 0x14, 0x8b, 0x4d, 8, 0x83, 0xf9, 0xf9]);
    setter[18..24].copy_from_slice(&[0x74, 12, 0x51, 0xff, 0x70, 8]);
    for (bytes, base, at, target) in [
        (&mut plain, 0x4a4200u32, 8, 0x600000u32),
        (&mut extended, 0x4a4220, 9, 0x600000),
        (&mut setter, 0x5da9f0, 24, 0x7a00e0),
    ] {
        relative(bytes, base, at, target);
    }
    relative(&mut plain, 0x4a4200, 14, 0x5d9170);
    relative(&mut extended, 0x4a4220, 15, 0x5d9170);
    BTreeMap::from([(0x4a4200, plain), (0x4a4220, extended), (0x5da9f0, setter)])
}
fn relative(bytes: &mut [u8], base: u32, at: usize, target: u32) {
    bytes[at] = 0xe8;
    bytes[at + 1..at + 5].copy_from_slice(&target.wrapping_sub(base + at as u32 + 5).to_le_bytes());
}
fn decode(windows: &BTreeMap<u32, Vec<u8>>) -> Result<Rules, String> {
    Rules::decode([0x39; 32], |va, _| {
        windows.get(&va).cloned().ok_or("missing window".into())
    })
}
pub(crate) fn rules() -> Rules {
    decode(&windows()).unwrap()
}

#[test]
fn thematic_nonstock_source_operands_control_default_and_sentinel() {
    let rules = rules();
    assert_eq!(rules.image_sha256, [0x39; 32]);
    assert_eq!(
        rules.initial_position().unwrap(),
        Some(Duration::from_millis(37))
    );
    assert_eq!(rules.position(-7).unwrap(), None);
    assert_eq!(
        rules.position(9580).unwrap(),
        Some(Duration::from_millis(9580))
    );
    assert!(rules.position(-1).is_err());
    assert!(rules.position(i32::MIN).is_err());
}

#[test]
fn thematic_policy_refuses_broken_calls_windows_and_unknown_images() {
    for (va, at) in [(0x4a4200, 0), (0x4a4220, 15), (0x5da9f0, 18)] {
        let mut windows = windows();
        windows.get_mut(&va).unwrap()[at] ^= 1;
        assert!(decode(&windows).is_err());
    }
    let mut windows = windows();
    windows.get_mut(&0x5da9f0).unwrap().pop();
    assert!(decode(&windows).is_err());
    assert!(Rules::read(b"unknown client").is_err());
}

#[test]
#[ignore = "requires PCX_THEMATIC_IMAGE selecting the local Legacy 1.29.2.9231 executable"]
fn thematic_selected_original_image_qualifies_operands() {
    let path = std::env::var_os("PCX_THEMATIC_IMAGE").expect("selected image");
    let image = std::fs::read(path).unwrap();
    let rules = Rules::read(&image).unwrap();
    assert_eq!(rules.image_sha256, REVIEWED_IMAGE);
    assert!(rules.initial_position().unwrap().is_some());
    assert!(rules.position(rules.ignored_position).unwrap().is_none());
    println!(
        "thematic image operands admitted, initial={} ignored={}",
        rules.initial_position, rules.ignored_position
    );
}

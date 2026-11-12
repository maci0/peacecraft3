use super::*;
use std::collections::BTreeMap;
type Windows = BTreeMap<u32, (bool, Vec<u8>)>;
fn bytes(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|v| u8::from_str_radix(v, 16).unwrap())
        .collect()
}
fn put(b: &mut [u8], at: usize, word: u32) {
    b[at..at + 4].copy_from_slice(&word.to_le_bytes());
}
fn relative(b: &mut [u8], base: u32, at: usize, target: u32) {
    b[at] = 0xe8;
    put(
        b,
        at + 1,
        target.wrapping_sub(base.wrapping_add(at as u32 + 5)),
    );
}
fn windows() -> Windows {
    let mut w = BTreeMap::new();
    let mut reg = bytes("68 00 00 00 00 68 00 00 00 00 68 00 00 00 00 e8 00 00 00 00");
    put(&mut reg, 1, 0x500100);
    put(&mut reg, 6, 0x500120);
    put(&mut reg, 11, 0x600000);
    relative(&mut reg, 0xab69e4, 15, 0x8ccc80);
    w.insert(0xab69e4, (true, reg));
    w.insert(0x500100, (false, b"()I\0".to_vec()));
    w.insert(0x500120, (false, b"MeleeDifficulty\0".to_vec()));
    let mut q = bytes(
        "8b 0d 00 00 00 00 85 c9 75 0d 6a 57 e8 00 00 00 00 83 c4 04 33 c0 c3 8b 41 54 23 41 50 83 f8 ff 75 04 33 c0 eb 0e 8d 49 50 e8 00 00 00 00 8b 0d 00 00 00 00 f7 80 00 00 00 00 00 00 00 00 74 06 b8 00 00 00 00 c3 8b 41 54 83 c1 50 23 01 83 f8 ff 75 04 33 c0 eb 05 e8 00 00 00 00 8b 80 00 00 00 00 25 00 00 00 00 f7 d8 1b c0 f7 d8 83 c0 0d c3",
    );
    put(&mut q, 2, 0x700100);
    put(&mut q, 48, 0x700100);
    put(&mut q, 54, 0x456);
    put(&mut q, 94, 0x456);
    put(&mut q, 58, 0x40);
    put(&mut q, 99, 0x140);
    put(&mut q, 65, 7);
    relative(&mut q, 0x600000, 41, 0x7fc7e0);
    relative(&mut q, 0x600000, 87, 0x7fc7e0);
    w.insert(0x600000, (true, q));
    let mut owner = bytes("a1 00 00 00 00");
    put(&mut owner, 1, 0x700100);
    w.insert(0xab49c0, (true, owner));
    w.insert(0xab49d6, (true, bytes("8d 48 50")));
    let mut writer = bytes(
        "8b 45 0c 83 e8 05 74 11 83 e8 04 75 16 81 8e 00 00 00 00 00 00 00 00 eb 0a 81 8e 00 00 00 00 00 00 00 00",
    );
    put(&mut writer, 15, 0x456);
    put(&mut writer, 27, 0x456);
    put(&mut writer, 19, 0x100);
    put(&mut writer, 31, 0x40);
    w.insert(0xabd3ed, (true, writer));
    let mut initial = bytes("c7 87 00 00 00 00 00 00 00 00");
    put(&mut initial, 2, 0x456);
    put(&mut initial, 6, 0x2000);
    w.insert(0xab98df, (true, initial));
    let mut vt = bytes("c7 07 00 00 00 00");
    put(&mut vt, 2, 0x500300);
    w.insert(0xab950c, (true, vt));
    w.insert(0x50031c, (false, 0x600300u32.to_le_bytes().to_vec()));
    w.insert(0x600300, (true, bytes("b8 44 43 42 41 c3")));
    w.insert(0xabcc74, (true, bytes("68 44 43 42 41")));
    let mut wrapper = bytes("55 8b ec 56 ff 75 08 8b f1 0f b6 46 30 50 e8 00 00 00 00 83 c4 08");
    relative(&mut wrapper, 0x4c0ed0, 14, 0xabd3a0);
    w.insert(0x4c0ed0, (true, wrapper));
    let mut startup = bytes("ff b1 88 05 00 00 e8 00 00 00 00");
    relative(&mut startup, 0x47e546, 6, 0x4c0ed0);
    w.insert(0x47e546, (true, startup));
    w.insert(0x490ab2, (true, bytes("8b 80 88 05 00 00")));
    w.insert(0x4bdf8c, (true, bytes("c7 86 88 05 00 00 0b 00 00 00")));
    w.insert(0x51dfd8, (true, bytes("0f b6 46 09 89 82 88 05 00 00")));
    w.insert(0x51e693, (true, bytes("c6 46 09 0b")));
    w
}
fn decode(w: &Windows, budget: usize) -> Result<Rules, String> {
    Rules::decode([0x53; 32], budget, |va, n, code| {
        let (actual, b) = w.get(&va).ok_or("missing synthetic window")?;
        if *actual != code || b.len() != n {
            return Err("synthetic window differs".into());
        }
        Ok(b.clone())
    })
}
pub(super) fn rules() -> Rules {
    decode(&windows(), 4096).unwrap()
}
#[test]
fn nonstock_creation_levels_flags_and_results_drive_owned_inputs() {
    let r = rules();
    assert_eq!(r.levels, [5, 11, 9]);
    let input = Input::new(r.clone(), &[(3, 5), (7, 11), (9, 9)]).unwrap();
    assert_eq!(input.image_sha256, [0x53; 32]);
    for (player, expected) in [(3, 7), (7, 13), (9, 14)] {
        assert_eq!(input.for_player(player).unwrap(), expected);
    }
    assert!(input.for_player(4).is_err());
    assert!(Input::new(r.clone(), &[(3, 10)]).is_err());
    assert!(Input::new(r.clone(), &[(3, 5), (3, 9)]).is_err());
    assert!(Input::new(r.clone(), &[(-1, 5)]).is_err());
    let fresh = r.fresh_input([2, 8]).unwrap();
    assert_eq!(fresh.for_player(8).unwrap(), 13);
}
#[test]
fn malformed_source_joins_and_all_truncated_windows_refuse() {
    let w = windows();
    let total = w.values().map(|(_, b)| b.len()).sum();
    assert!(decode(&w, total).is_ok());
    assert!(decode(&w, total - 1).is_err());
    for (va, at) in [
        (0xab69e4, 1),
        (0x500120, 15),
        (0x600000, 2),
        (0x600000, 28),
        (0x600000, 41),
        (0x600000, 54),
        (0x600000, 58),
        (0x600000, 62),
        (0x600000, 99),
        (0xabd3ed, 7),
        (0xabd3ed, 19),
        (0xab98df, 2),
        (0xab950c, 2),
        (0x600300, 1),
        (0x4c0ed0, 15),
        (0x47e546, 2),
        (0x490ab2, 2),
        (0x4bdf8c, 6),
        (0x51dfd8, 3),
        (0x51e693, 3),
    ] {
        let mut changed = w.clone();
        changed.get_mut(&va).unwrap().1[at] ^= 1;
        assert!(decode(&changed, 4096).is_err(), "{va:x}+{at}");
    }
    for (&va, (_, b)) in &w {
        for n in 0..b.len() {
            let mut changed = w.clone();
            changed.get_mut(&va).unwrap().1.truncate(n);
            assert!(decode(&changed, 4096).is_err(), "{va:x} length {n}");
        }
    }
}
#[test]
#[ignore = "requires PCX_WAR3_EXE selected Legacy client"]
fn selected_melee_difficulty_image_qualifies_creation_and_lobby_joins() {
    let image = std::fs::read(std::env::var_os("PCX_WAR3_EXE").unwrap()).unwrap();
    let r = Rules::read(&image).unwrap();
    assert_eq!(r.image_sha256, REVIEWED_IMAGE);
    let defaults = r.clone().fresh_input([0, 11]).unwrap();
    assert_eq!(
        defaults.for_player(11).unwrap(),
        r.result(r.levels[1]).unwrap()
    );
}

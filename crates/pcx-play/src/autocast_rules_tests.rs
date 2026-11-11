use super::*;
fn identity() -> AssetIdentity {
    AssetIdentity {
        source: "synthetic-autocast".into(),
        edition: "nonstock".into(),
        version: "relocated".into(),
    }
}
type Memory = Vec<(u32, Vec<u8>, bool)>;
type Drift = (&'static str, fn(&mut Memory));
const FIELD: u8 = 0x24;
/// Shared ON handler, update hook, update routine, activation and its delay float.
const SHARED_HANDLER: u32 = 0x20000;
const SHARED_HOOK: u32 = 0x20100;
const SHARED_UPDATE: u32 = 0x20200;
const SHARED_ACTIVATE: u32 = 0x20300;
const SHARED_DELAY: u32 = 0x21000;
/// Tables of vtables that share the ON handler but are not in `CLASS_NAMES`.
const EXTRA_TABLES: u32 = 0x60000;
const DELAY: f32 = 0.0625;
const GROUP_SLOT: usize = 0x1b8;
const HOOK_SLOT: usize = 0x348;
const ACTIVATE_SLOT: usize = 0x434;
const TABLE_BYTES: usize = 0x440;
fn constant(value: u32) -> Vec<u8> {
    let mut b = vec![0xb8];
    b.extend(value.to_le_bytes());
    b.push(0xc3);
    b
}
fn group_function(group: u32) -> Vec<u8> {
    if group == 0 {
        vec![0x33, 0xc0, 0xc3, 0x90, 0x90, 0x90]
    } else {
        constant(group)
    }
}
fn on_handler(mask: u8) -> Vec<u8> {
    let mut b = vec![0x90; 139];
    b[47..51].copy_from_slice(&[0xf6, 0x46, FIELD, mask]);
    b[79..81].copy_from_slice(&[0xff, 0x90]);
    b[81..85].copy_from_slice(&(GROUP_SLOT as u32).to_le_bytes());
    b[113..120].copy_from_slice(&[0x81, 0x4e, FIELD, mask, 0, 0, 0]);
    b[133..135].copy_from_slice(&[0xff, 0xa0]);
    b[135..139].copy_from_slice(&(HOOK_SLOT as u32).to_le_bytes());
    b
}
fn update_routine(mask: u8) -> Vec<u8> {
    let mut b = vec![0x90; 61];
    b[..4].copy_from_slice(&[0x83, 0x79, 0x3c, 0x00]);
    b[16..18].copy_from_slice(&[0xa8, mask | 0x02]);
    b[46..49].copy_from_slice(&[0x81, 0x49, FIELD]);
    b[53..57].copy_from_slice(&[0x8b, 0x01, 0xff, 0x90]);
    b[57..61].copy_from_slice(&(ACTIVATE_SLOT as u32).to_le_bytes());
    b
}
fn activation(shift: u32) -> Vec<u8> {
    let mut b = vec![0x90; 24];
    b[..3].copy_from_slice(&[0xf7, 0x41, FIELD]);
    b[9..15].copy_from_slice(&[0x6a, 0, 0x6a, 0, 0x51, 0x68]);
    b[19] = 0x68;
    b[20..24].copy_from_slice(&(shift + SHARED_DELAY).to_le_bytes());
    b
}
/// Group of class `i`: 0, 1 or 2.
fn group_of_class(i: usize) -> u32 {
    (i % 3) as u32
}
fn class_table(shift: u32, group_fn: u32, code_fn: u32) -> Vec<u8> {
    let mut v = vec![0; TABLE_BYTES];
    for (slot, target) in [
        (0x1c, code_fn),
        (GROUP_SLOT, group_fn),
        (0x340, shift + SHARED_HANDLER),
        (HOOK_SLOT, shift + SHARED_HOOK),
        (ACTIVATE_SLOT, shift + SHARED_ACTIVATE),
    ] {
        v[slot..slot + 4].copy_from_slice(&target.to_le_bytes())
    }
    v
}
fn fixture(shift: u32, mask: u8) -> Vec<(u32, Vec<u8>, bool)> {
    let mut memory = vec![
        (shift + SHARED_HANDLER, on_handler(mask), true),
        (
            shift + SHARED_HOOK,
            [
                &[0xe9][..],
                &SHARED_UPDATE.wrapping_sub(SHARED_HOOK + 5).to_le_bytes(),
            ]
            .concat(),
            true,
        ),
        (shift + SHARED_UPDATE, update_routine(mask), true),
        (shift + SHARED_ACTIVATE, activation(shift), true),
        (shift + SHARED_DELAY, DELAY.to_le_bytes().to_vec(), false),
    ];
    for (i, name) in CLASS_NAMES.iter().enumerate() {
        let n = shift + 0x30000 + i as u32 * 0x1000;
        let f = shift + 0x10000 + i as u32 * 0x1000;
        let table = shift + 0x40000 + i as u32 * 0x1000;
        memory.push((n, format!("{name}\0").into_bytes(), false));
        memory.push((f, constant(n), true));
        memory.push((
            f + 0x10,
            constant(u32::from_be_bytes([
                b'X',
                b'0' + (i / 10) as u8,
                b'0' + (i % 10) as u8,
                b'1',
            ])),
            true,
        ));
        memory.push((f + 0x20, constant(7000 + i as u32 * 2), true));
        memory.push((f + 0x30, constant(7001 + i as u32 * 2), true));
        memory.push((f + 0x50, group_function(group_of_class(i)), true));
        let mut v = class_table(shift, f + 0x50, f + 0x10);
        for (slot, target) in [
            (0x58, f),
            (0x310, f + 0x20),
            (0x320, f + 0x30),
            (0x338, f + 0x60),
            (0x33c, f + 0x40),
            (0x344, f + 0x100),
        ] {
            v[slot..slot + 4].copy_from_slice(&target.to_le_bytes())
        }
        memory.push((table, v, false));
        memory.push((
            f + 0x40,
            vec![0x8b, 0x01, 0xff, 0x90, 0x44, 0x03, 0, 0, 0xc2, 4, 0],
            true,
        ));
        memory.push((
            f + 0x60,
            vec![0x8b, 0x01, 0xff, 0x90, 0x40, 0x03, 0, 0, 0xc2, 4, 0],
            true,
        ));
        let mut b = vec![0x90; 142];
        b[..6].copy_from_slice(&[0x56, 0x8b, 0xf1, 0x57, 0x8b, 0x7e]);
        b[47..56].copy_from_slice(&[0xf6, 0x46, FIELD, mask, 0x74, 86, 0x81, 0x66, FIELD]);
        b[56..60].copy_from_slice(&(!u32::from(mask)).to_le_bytes());
        b[60..66].copy_from_slice(&[0x8b, 0xce, 0x8b, 0x06, 0xff, 0x90]);
        b[139..].copy_from_slice(&[0x5f, 0x5e, 0xc3]);
        memory.push((f + 0x100, b, true));
    }
    memory
}
/// Adds a vtable sharing the ON handler under `code` with `group` (a class outside `CLASS_NAMES`).
fn add_family_member(
    memory: &mut Vec<(u32, Vec<u8>, bool)>,
    shift: u32,
    code: [u8; 4],
    group: u32,
) {
    let at = shift + EXTRA_TABLES + memory.len() as u32 * 0x1000;
    memory.push((at, constant(u32::from_be_bytes(code)), true));
    memory.push((at + 0x10, group_function(group), true));
    memory.push((at + 0x1000, class_table(shift, at + 0x10, at), false));
}
fn decode(memory: &[(u32, Vec<u8>, bool)]) -> Result<Rules, String> {
    decode_with_work(memory, 128 << 20)
}
fn decode_with_work(memory: &[(u32, Vec<u8>, bool)], work: usize) -> Result<Rules, String> {
    Rules::decode(
        identity(),
        [17; 32],
        memory
            .iter()
            .filter(|r| r.2)
            .map(|r| (r.0, r.1.as_slice()))
            .collect(),
        memory
            .iter()
            .filter(|r| !r.2)
            .map(|r| (r.0, r.1.as_slice()))
            .collect(),
        work,
        |va, n, code| {
            memory
                .iter()
                .find(|r| va >= r.0 && va - r.0 < r.1.len() as u32 && (!code || r.2))
                .and_then(|r| r.1.get((va - r.0) as usize..(va - r.0) as usize + n))
                .map(<[u8]>::to_vec)
                .ok_or_else(|| "fixture mapping".into())
        },
    )
}
#[test]
fn relocated_nonstock_classes_orders_and_state_bit_are_derived() {
    for shift in [0, 0x80000] {
        let rules = decode(&fixture(shift, 0x40)).unwrap();
        assert_eq!(rules.classes[0].code, *b"X001");
        assert_eq!(rules.classes[1].on, 7002);
        assert_eq!(rules.classes[2].off, 7005);
        assert_eq!(rules.classes[4].code, *b"X041");
        assert_eq!(rules.classes[4].on, 7008);
        assert_eq!(rules.classes[4].off, 7009);
    }
}
#[test]
fn on_handler_group_and_first_tick_are_derived_per_class() {
    for shift in [0, 0x80000] {
        let rules = decode(&fixture(shift, 0x40)).unwrap();
        for (i, class) in rules.classes.iter().enumerate() {
            let enable = class.enable.expect("ON decoded");
            assert_eq!(enable.group, group_of_class(i));
            assert_eq!(enable.first_tick, DELAY);
        }
    }
}
#[test]
fn family_groups_list_every_nonzero_table_sharing_the_on_handler() {
    let mut memory = fixture(0, 0x40);
    add_family_member(&mut memory, 0, *b"Z001", 2);
    add_family_member(&mut memory, 0, *b"Z002", 0);
    let rules = decode(&memory).unwrap();
    // Classes 1, 2, 4, 5, ... carry group 1 or 2; Z001 is outside `CLASS_NAMES`; Z002 has no group.
    assert!(rules.groups.contains(&(*b"Z001", 2)));
    assert!(rules.groups.iter().all(|(code, _)| code != b"Z002"));
    assert_eq!(
        rules.groups.len(),
        CLASS_NAMES.len() - CLASS_NAMES.len().div_ceil(3) + 1
    );
    assert!(rules.groups.contains(&(*b"X011", 1)));
}
#[test]
fn conflicting_group_for_one_code_refuses() {
    let mut memory = fixture(0, 0x40);
    add_family_member(&mut memory, 0, *b"Z001", 1);
    add_family_member(&mut memory, 0, *b"Z001", 2);
    assert!(decode(&memory).unwrap_err().contains("code has two groups"));
}
#[test]
fn unreadable_group_function_of_a_family_member_refuses() {
    let mut memory = fixture(0, 0x40);
    add_family_member(&mut memory, 0, *b"Z001", 1);
    let group_fn = memory[memory.len() - 2].0;
    memory.retain(|r| r.0 != group_fn);
    assert!(decode(&memory).is_err());
}
#[test]
fn drifted_on_handler_leaves_enable_undecoded_but_keeps_off() {
    let mask = 0x40;
    let drifts: [Drift; 4] = [
        ("handler sets another bit", |m| m[0].1[116] ^= 0x01),
        ("hook is not a jump", |m| m[1].1[0] = 0x90),
        ("update does not test the bit", |m| m[2].1[17] = 0x02),
        ("delay is not positive", |m| {
            m[4].1.copy_from_slice(&0f32.to_le_bytes())
        }),
    ];
    for (what, drift) in drifts {
        let mut memory = fixture(0, mask);
        drift(&mut memory);
        let rules = decode(&memory).unwrap();
        assert!(
            rules.classes.iter().all(|c| c.enable.is_none()),
            "{what}: ON decoded"
        );
        assert_eq!(rules.classes.len(), CLASS_NAMES.len(), "{what}");
    }
}
#[test]
fn ambiguous_or_malformed_source_refuses() {
    let mut m = fixture(0, 0x40);
    let dupe = m[5].clone();
    m.push((dupe.0 + 0x100000, dupe.1, dupe.2));
    assert!(decode(&m).is_err());
    for mask in [0, 3] {
        assert!(decode(&fixture(0, mask)).is_err());
    }
    let mut m = fixture(0, 0x40);
    m.last_mut().unwrap().1[56] ^= 1;
    assert!(decode(&m).is_err());
}
#[test]
#[ignore = "requires PCX_WAR3_EXE pointing to the reviewed local client"]
fn selected_original_autocast_classes_bind() {
    let image = std::fs::read(std::env::var_os("PCX_WAR3_EXE").expect("PCX_WAR3_EXE")).unwrap();
    let rules = Rules::read(
        &image,
        AssetIdentity {
            source: "explicit selected Legacy image".into(),
            edition: "classic".into(),
            version: "1.29.2.9231".into(),
        },
    )
    .unwrap();
    assert_eq!(rules.classes.len(), CLASS_NAMES.len());
    assert!(rules.classes.iter().all(|c| c.on != c.off));
    let codes: Vec<[u8; 4]> = rules.classes.iter().map(|c| c.code).collect();
    for code in [
        b"Aweb", b"Afae", b"Aren", b"Ablo", b"AUfu", b"Arst", b"ANms", b"AEim", b"Arep", b"ANba",
        b"Acrs",
    ] {
        assert!(codes.contains(code), "missing autocast class {code:?}");
    }
    let class = |code: &[u8; 4]| rules.classes.iter().find(|c| c.code == *code).unwrap();
    // Inner Fire, Web, Raise Dead, Black Arrow and Heal share the decoded ON handler;
    // Mana Shield and Immolation override the update hook and stay undecoded.
    for code in [b"Ainf", b"Aweb", b"Arai", b"ANba", b"Ahea"] {
        assert!(class(code).enable.is_some(), "no ON decode for {code:?}");
    }
    for code in [b"ANms", b"AEim"] {
        assert!(
            class(code).enable.is_none(),
            "unexpected ON decode {code:?}"
        );
    }
    assert_eq!(class(b"Ainf").enable.unwrap().group, 1);
    assert_eq!(class(b"ANba").enable.unwrap().group, 2);
    assert_eq!(class(b"Ainf").enable.unwrap().first_tick, 0.02);
    for pair in [(*b"Ahea", 1), (*b"Aslo", 1), (*b"AHfa", 2), (*b"ANba", 2)] {
        assert!(rules.groups.contains(&pair), "missing group {pair:?}");
    }
    println!(
        "autocast families with a nonzero group: {}",
        rules.groups.len()
    );
}

#[test]
fn bounded_source_scan_refuses_before_unbounded_search() {
    let memory = fixture(0, 0x20);
    assert!(
        decode_with_work(&memory, 1)
            .unwrap_err()
            .contains("scan work exhausted")
    );
    let mut changed = memory;
    changed.last_mut().unwrap().1[52] = 0;
    assert!(decode(&changed).is_err());
}

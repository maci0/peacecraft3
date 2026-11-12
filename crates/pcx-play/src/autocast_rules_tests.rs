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
const EXTRA_TABLES: u32 = 0x380000;
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
        let n = shift + 0x330000 + 0x14 + i as u32 * 0x1000;
        let f = shift + 0x300000 + 0x28 + i as u32 * 0x1000;
        let table = shift + 0x350000 + i as u32 * 0x1000;
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
fn executable_getter_pointer_coincidence_keeps_true_data_ambiguity_strict() {
    let mut memory = fixture(0, 0x40);
    let name = memory
        .iter()
        .find(|entry| !entry.2 && entry.1 == format!("{}\0", CLASS_NAMES[0]).as_bytes())
        .unwrap()
        .0;
    let getter = memory
        .iter()
        .find(|entry| entry.2 && entry.1 == constant(name))
        .unwrap()
        .0;
    let pointer = getter.to_le_bytes().to_vec();
    memory.push((0x700000, pointer.clone(), true));
    assert_eq!(decode(&memory).unwrap().classes.len(), CLASS_NAMES.len());
    memory.push((0x710000, pointer, false));
    assert!(
        decode(&memory)
            .unwrap_err()
            .contains("source anchor ambiguous")
    );
}
fn timing_fixture(shift: u32, retry: i8, minimum: i8) -> Memory {
    let table = shift + 0x30000;
    let mut memory = Vec::new();
    for (slot, init_va, value) in [(0x470, 0x4015c0u32, retry), (0x474, 0x401460, minimum)] {
        let getter_va = shift + 0x40000 + slot * 0x20;
        let global_va = shift + 0x50000 + slot;
        memory.push((table + slot, getter_va.to_le_bytes().to_vec(), false));
        memory.push((
            getter_va,
            [
                &[0x55, 0x8b, 0xec, 0x8b, 0x45, 0x08, 0x8b, 0x0d][..],
                &global_va.to_le_bytes(),
                &[0x89, 0x08, 0x5d, 0xc2, 0x04, 0x00],
            ]
            .concat(),
            true,
        ));
        memory.push((
            init_va,
            [
                &[0x6a, value as u8, 0x68][..],
                &global_va.to_le_bytes(),
                &[0xe8],
                &0x9a1620u32.wrapping_sub(init_va + 12).to_le_bytes(),
                &[0x83, 0xc4, 0x08, 0xc3],
            ]
            .concat(),
            true,
        ));
    }
    memory
}

fn read_timing(memory: &Memory, shift: u32) -> Result<TickTiming, String> {
    TickTiming::read(shift + 0x30000, &mut |va, n, code| {
        memory
            .iter()
            .find(|(at, b, executable)| {
                va >= *at && va - *at < b.len() as u32 && code == *executable
            })
            .and_then(|(at, b, _)| b.get((va - *at) as usize..(va - *at) as usize + n))
            .map(<[u8]>::to_vec)
            .ok_or_else(|| "synthetic timing window missing".into())
    })
}

#[test]
fn tick_delays_follow_relocated_getters_and_nonstock_bootstrap_operands() {
    for shift in [0, 0x100000] {
        let timing = read_timing(&timing_fixture(shift, 11, 7), shift).unwrap();
        assert_eq!(timing.retry, 11.);
        assert_eq!(timing.minimum_after_cast, 7.);
        assert_eq!(
            read_timing(&timing_fixture(shift, 17, 9), shift)
                .unwrap()
                .retry,
            17.
        );
    }
}

#[test]
fn class_tick_timing_preserves_independent_on_and_off_admission() {
    let mut memory = fixture(0, 0x40);
    let mut timing = timing_fixture(0, 11, 7);
    timing[0].0 = 0x350000 + 0x470;
    timing[3].0 = 0x350000 + 0x474;
    memory.extend(timing);
    let rules = decode(&memory).unwrap();
    assert_eq!(
        rules.classes[0].tick,
        Some(TickTiming {
            retry: 11.,
            minimum_after_cast: 7.
        })
    );
    assert!(rules.classes[0].enable.is_some());
    assert!(rules.classes[1..].iter().all(|c| c.tick.is_none()));
    let initializer = memory.iter_mut().find(|r| r.0 == 0x4015c0).unwrap();
    initializer.1[3] ^= 1;
    let rules = decode(&memory).unwrap();
    assert!(rules.classes[0].tick.is_none());
    assert!(rules.classes[0].enable.is_some());
    assert_ne!(rules.classes[0].on, rules.classes[0].off);
}

#[test]
fn tick_delay_drift_or_invalid_initialization_refuses_without_stock_fallback() {
    for mutate in [
        (|m: &mut Memory| m[1].1[6] = 0x90) as fn(&mut Memory),
        |m| m[2].1[3] ^= 1,
        |m| m[2].1[8] ^= 1,
        |m| m[2].1[1] = 0,
        |m| m[2].1[1] = 0xff,
        |m| {
            m[0].1.truncate(2);
        },
    ] {
        let mut memory = timing_fixture(0, 11, 7);
        mutate(&mut memory);
        assert!(read_timing(&memory, 0).is_err());
    }
}

fn mana_fixture(shift: u32, floor: i8) -> Memory {
    let table = shift + 0x60000;
    let helper = shift + 0x70000;
    let global = shift + 0x80000;
    let mut config = vec![0x90; 0x61];
    config[..6].copy_from_slice(&[0x8b, 0x45, 0x18, 0xff, 0x75, 0x20]);
    config[0x14..0x1c].copy_from_slice(&[0x8b, 0x10, 0x8b, 0x45, 0x1c, 0x89, 0x55, 0xe0]);
    config[0x4d..0x55].copy_from_slice(&[0x8d, 0x45, 0xd0, 0x50, 0x56, 0xff, 0x75, 0x08]);
    config[0x5c] = 0xe8;
    config[0x5d..].copy_from_slice(&0x801c50u32.wrapping_sub(helper + 0x91).to_le_bytes());
    let mut init = vec![0x90; 40];
    init[..12].copy_from_slice(&[0x8b, 0x86, 0xc0, 0x01, 0, 0, 0x8d, 0x8e, 0xc0, 0x01, 0, 0]);
    init[12..15].copy_from_slice(&[0x6a, 0, 0x68]);
    init[19] = 0x68;
    init[20..24].copy_from_slice(&global.to_le_bytes());
    init[24] = 0x68;
    init[25..29].copy_from_slice(&global.to_le_bytes());
    init[37..].copy_from_slice(&[0xff, 0x50, 0x18]);
    vec![
        (
            0x675835,
            [&[0xc7, 0x86, 0xc0, 0x01, 0, 0][..], &table.to_le_bytes()].concat(),
            true,
        ),
        (table + 0x18, helper.to_le_bytes().to_vec(), false),
        (helper + 0x30, config, true),
        (0x670a10, init, true),
        (
            0x8c3ee9,
            vec![
                0x8b, 0x46, 0x10, 0x89, 0x45, 0xf8, 0x8b, 0x46, 0x14, 0x89, 0x45, 0xfc, 0x8d, 0x97,
                0x80, 0, 0, 0, 0x8b, 0x01, 0x8b, 0x49, 0x04, 0x89, 0x02, 0x89, 0x4a, 0x04,
            ],
            true,
        ),
        (
            0x8c47d0,
            vec![
                0xf3, 0x0f, 0x10, 0x83, 0x80, 0, 0, 0, 0x8d, 0xb3, 0x80, 0, 0, 0,
            ],
            true,
        ),
        (0x8c4817, vec![0xf3, 0x0f, 0x10, 0x06], true),
        (
            0x401300,
            [
                &[0x6a, floor as u8, 0x68][..],
                &global.to_le_bytes(),
                &[0xe8],
                &0x9a1620u32.wrapping_sub(0x401300 + 12).to_le_bytes(),
                &[0x83, 0xc4, 0x08, 0xc3],
            ]
            .concat(),
            true,
        ),
    ]
}

fn read_mana_minimum(memory: &Memory) -> Result<f32, String> {
    Rules::mana_minimum(&mut |va, n, code| {
        memory
            .iter()
            .find(|(at, b, executable)| {
                va >= *at && va - *at < b.len() as u32 && code == *executable
            })
            .and_then(|(at, b, _)| b.get((va - *at) as usize..(va - *at) as usize + n))
            .map(<[u8]>::to_vec)
            .ok_or_else(|| "synthetic mana window missing".into())
    })
}

#[test]
fn mana_minimum_follows_proxy_descriptor_and_nonstock_bootstrap() {
    for shift in [0, 0x100000] {
        for floor in [-3, 0, 4, 9] {
            assert_eq!(
                read_mana_minimum(&mana_fixture(shift, floor)).unwrap(),
                f32::from(floor)
            );
        }
    }
}

#[test]
fn mana_minimum_rejects_changed_component_descriptor_or_clamp_roles() {
    for mutate in [
        (|m: &mut Memory| m[0].1[2] = 0xa0) as fn(&mut Memory),
        |m| m[2].1[2] = 0x14,
        |m| m[2].1[0x1b] = 0xe4,
        |m| m[2].1[0x5d] ^= 1,
        |m| m[3].1[20] ^= 1,
        |m| m[4].1[2] = 0x14,
        |m| m[5].1[4] = 0x84,
        |m| {
            m[7].1.truncate(8);
        },
    ] {
        let mut memory = mana_fixture(0, 4);
        mutate(&mut memory);
        assert!(read_mana_minimum(&memory).is_err());
    }
}

#[test]
#[ignore = "requires PCX_WAR3_EXE pointing to the reviewed local client"]
fn selected_original_autocast_classes_bind() {
    use std::io::Read;
    const IMAGE_BUDGET: usize = 32 << 20;
    let mut image = Vec::new();
    std::fs::File::open(std::env::var_os("PCX_WAR3_EXE").expect("PCX_WAR3_EXE"))
        .unwrap()
        .take(IMAGE_BUDGET as u64 + 1)
        .read_to_end(&mut image)
        .unwrap();
    assert!(
        image.len() <= IMAGE_BUDGET,
        "selected image exceeds test budget"
    );
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
        b"Acrs", b"Ambt", b"Anhe", b"Asps", b"Afak",
    ] {
        assert!(codes.contains(code), "missing autocast class {code:?}");
    }
    let class = |code: &[u8; 4]| rules.classes.iter().find(|c| c.code == *code).unwrap();
    // Inner Fire, Web, Raise Dead, Black Arrow and Heal share the decoded ON handler;
    // Mana Shield and Immolation override the update hook and stay undecoded.
    for code in [
        b"Ainf", b"Aweb", b"Arai", b"ANba", b"Ahea", b"Ambt", b"Anhe", b"Asps", b"Afak",
    ] {
        assert!(class(code).enable.is_some(), "no ON decode for {code:?}");
    }
    for code in [b"ANms", b"AEim"] {
        assert!(
            class(code).enable.is_none(),
            "unexpected ON decode {code:?}"
        );
    }
    assert_eq!(rules.alarm_code, Some(*b"Aalr"));
    assert!(rules.mana_minimum.is_some_and(f32::is_finite));
    let clock = rules.clock.expect("selected simple timer clock");
    assert!(
        clock
            .deadline(0., class(b"Ahea").tick.unwrap().retry)
            .unwrap()
            > 0.
    );
    assert_eq!(class(b"Ainf").enable.unwrap().group, 1);
    assert_eq!(class(b"Ambt").enable.unwrap().group, 1);
    assert_eq!(class(b"ANba").enable.unwrap().group, 2);
    assert_eq!(
        class(b"Ainf").enable.unwrap().first_tick,
        class(b"Ahea").enable.unwrap().first_tick
    );
    assert_eq!(class(b"Anhe").enable, class(b"Ahea").enable);
    assert_eq!(class(b"Asps").enable, class(b"Ahea").enable);
    let orb = class(b"Afak").enable.unwrap();
    assert_eq!(orb.group, class(b"ANba").enable.unwrap().group);
    assert_eq!(orb.first_tick, class(b"Ahea").enable.unwrap().first_tick);
    for code in [b"Asps", b"Afak"] {
        assert_ne!(class(code).on, class(b"Ahea").on);
        assert_ne!(class(code).off, class(b"Ahea").off);
    }

    let timing = class(b"Ahea").tick.expect("selected Heal tick timing");
    assert!(timing.retry > timing.minimum_after_cast);
    assert!(timing.minimum_after_cast > 0.);
    for code in [b"Aslo", b"Anhe", b"Asps", b"Afak"] {
        assert_eq!(class(code).tick, Some(timing));
    }
    assert_ne!(class(b"Anhe").on, class(b"Ahea").on);
    assert_ne!(class(b"Anhe").off, class(b"Ahea").off);
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

fn alarm_memory(second_code: u32) -> Vec<(u32, Vec<u8>)> {
    let lookup = |va: u32, getter: u32| {
        let mut b = vec![0x6a, 1, 0x6a, 1, 0x6a, 0, 0x6a, 0, 0xe8];
        b.extend(getter.wrapping_sub(va + 13).to_le_bytes());
        (va, b)
    };
    vec![
        lookup(ALARM_SET_LOOKUP, 0x9000),
        lookup(ALARM_QUERY_LOOKUP, 0x9010),
        (0x9000, constant(u32::from_be_bytes(*b"X0al"))),
        (0x9010, constant(second_code)),
        (ALARM_SET_BIT, vec![0x81, 0x48, 0x20, 0x80, 0, 0, 0]),
        (
            ALARM_CLEAR_BIT,
            vec![0x81, 0x60, 0x20, 0x7f, 0xff, 0xff, 0xff],
        ),
        (
            ALARM_READ_BIT,
            vec![0x8b, 0x40, 0x20, 0xc1, 0xe8, 0x07, 0x83, 0xe0, 0x01],
        ),
    ]
}
fn read_alarm(memory: &[(u32, Vec<u8>)]) -> Result<[u8; 4], String> {
    Rules::alarm_code(&mut |va, n, _| {
        memory
            .iter()
            .find(|(base, b)| va >= *base && (va - base) as usize + n <= b.len())
            .map(|(base, b)| b[(va - base) as usize..][..n].to_vec())
            .ok_or_else(|| "fixture mapping".into())
    })
}
#[test]
fn alarm_code_is_read_from_the_getter_both_natives_call() {
    assert_eq!(
        read_alarm(&alarm_memory(u32::from_be_bytes(*b"X0al"))),
        Ok(*b"X0al")
    );
}
#[test]
fn alarm_code_refuses_differing_getters_and_changed_bit_operations() {
    assert!(read_alarm(&alarm_memory(u32::from_be_bytes(*b"X0bl"))).is_err());
    assert!(read_alarm(&alarm_memory(0x0000_0001)).is_err());
    for at in [ALARM_SET_BIT, ALARM_CLEAR_BIT, ALARM_READ_BIT] {
        let mut memory = alarm_memory(u32::from_be_bytes(*b"X0al"));
        let (_, bytes) = memory.iter_mut().find(|(va, _)| *va == at).unwrap();
        bytes[3] ^= 0x10;
        assert!(read_alarm(&memory).is_err());
    }
    let mut memory = alarm_memory(u32::from_be_bytes(*b"X0al"));
    memory.truncate(3);
    assert!(read_alarm(&memory).is_err());
}

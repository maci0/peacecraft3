use super::*;
use crate::camera_input::edge;

const CODE_VA: u32 = 0x5d1000;
const CODE_SIZE: u32 = 0x80000;
const DATA_VA: u32 = 0xe90000;
const DATA_SIZE: u32 = 0x5000;
const RAW: usize = 0x400;
const NAMES: u32 = 0xe90610;
const TINTS: u32 = 0xe94210;
const GROUP_JUMPS: u32 = 0x6168b8;
const GROUP_INDEX: u32 = 0x6168d0;
const SCROLL_SUB_JUMPS: u32 = 0x6168e4;
const SCROLL_PROLOGUE: u32 = 0x6166ef;
const MASK_JUMPS: u32 = 0x5dfe7c;
const SCROLL_DEFAULT: u32 = 0x5dfe32;
/// Case bodies live here, apart from the fixed sites.
const CASES: u32 = 0x616a30;
const TAIL: u32 = 0x616a00;
const SCROLL_TAIL: u32 = 0x616a10;

const NAME_LIST: [&str; TAG_COUNT] = [
    "Normal",
    "Select",
    "Target",
    "TargetSelect",
    "InvalidTarget",
    "HoldItem",
    "Scroll",
    "Left",
    "Right",
    "Up",
    "Down",
];

/// The original's group of each mode: normal, three hover, target, four armed, signal.
const GROUPS: [u8; 18] = [0, 1, 1, 1, 2, 3, 3, 3, 3, 4, 5, 5, 5, 5, 5, 5, 5, 5];
/// Tag lists the synthetic cases pass, by mode.
fn tags_of(mode: usize) -> Vec<u8> {
    match mode {
        0 => vec![0],
        1..=3 => vec![1],
        4 => vec![2],
        5..=8 => vec![3],
        9 => vec![5],
        10 => vec![6, 7],
        11 => vec![6, 8],
        12 => vec![6, 9],
        13 => vec![6, 10],
        14 => vec![6, 9, 7],
        15 => vec![6, 9, 8],
        16 => vec![6, 10, 7],
        _ => vec![6, 10, 8],
    }
}

fn put(image: &mut [u8], va: u32, bytes: &[u8]) -> u32 {
    let at = if va >= DATA_VA {
        RAW + CODE_SIZE as usize + (va - DATA_VA) as usize
    } else {
        RAW + (va - CODE_VA) as usize
    };
    image[at..at + bytes.len()].copy_from_slice(bytes);
    va + bytes.len() as u32
}

fn call_sink(image: &mut [u8], va: u32) -> u32 {
    let rel = SPRITE_SET_TAGS.wrapping_sub(va + 5);
    put(image, va, &[&[0xe8][..], &rel.to_le_bytes()].concat())
}

fn jump_short(image: &mut [u8], va: u32, to: u32) -> u32 {
    put(image, va, &[0xeb, (to as i64 - va as i64 - 2) as i8 as u8])
}

fn store(image: &mut [u8], va: u32, disp: i8, value: u32) -> u32 {
    put(
        image,
        va,
        &[&[0xc7, 0x45, disp as u8][..], &value.to_le_bytes()].concat(),
    )
}

/// A PE with the instruction forms the rules read at the reviewed addresses.
fn image() -> Vec<u8> {
    let mut image = vec![0; RAW + (CODE_SIZE + DATA_SIZE) as usize];
    image[..2].copy_from_slice(b"MZ");
    image[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
    image[0x80..0x84].copy_from_slice(b"PE\0\0");
    image[0x84..0x86].copy_from_slice(&0x14cu16.to_le_bytes());
    image[0x86..0x88].copy_from_slice(&2u16.to_le_bytes());
    image[0x94..0x96].copy_from_slice(&64u16.to_le_bytes());
    image[0x98..0x9a].copy_from_slice(&0x10bu16.to_le_bytes());
    image[0xb4..0xb8].copy_from_slice(&0x400000u32.to_le_bytes());
    image[0xd0..0xd4].copy_from_slice(&0xc00000u32.to_le_bytes());
    for (index, (va, size, raw, flags)) in [
        (CODE_VA, CODE_SIZE, RAW as u32, 0x6000_0020u32),
        (DATA_VA, DATA_SIZE, RAW as u32 + CODE_SIZE, 0x4000_0040),
    ]
    .into_iter()
    .enumerate()
    {
        let section = 0xd8 + index * 40;
        for (offset, value) in [
            (8, size),
            (12, va - 0x400000),
            (16, size),
            (20, raw),
            (36, flags),
        ] {
            image[section + offset..section + offset + 4].copy_from_slice(&value.to_le_bytes());
        }
    }
    // Tag registration: push index, push name, push [table], call.
    let mut name_at = NAMES;
    let mut at = TAG_REGISTRATION.start;
    for (index, name) in NAME_LIST.iter().enumerate() {
        put(&mut image, name_at, name.as_bytes());
        let entry = [
            &[0x6a, index as u8, 0x68][..],
            &name_at.to_le_bytes(),
            &[0xff, 0x35, 0xac, 0xd0, 0x13, 0x01],
        ]
        .concat();
        at = put(&mut image, at, &entry);
        at = call_sink(&mut image, at);
        name_at += 16;
    }
    // Mode setter: range check, group index and jump tables.
    put(&mut image, MODE_LIMIT_COMPARE, &[0x83, 0xfb, 0x11]);
    put(
        &mut image,
        GROUP_INDEX_LOAD,
        &[&[0x0f, 0xb6, 0x83][..], &GROUP_INDEX.to_le_bytes()].concat(),
    );
    put(
        &mut image,
        GROUP_JUMP,
        &[&[0xff, 0x24, 0x85][..], &GROUP_JUMPS.to_le_bytes()].concat(),
    );
    put(&mut image, GROUP_INDEX, &GROUPS);
    // Single-tag cases share a tail: push 0, push 1, lea eax, push eax, push esi, call.
    let mut tail = put(
        &mut image,
        TAIL,
        &[0x6a, 0x00, 0x6a, 0x01, 0x8d, 0x45, 0xec, 0x50, 0x56],
    );
    call_sink(&mut image, tail);
    tail = TAIL;
    let mut case_at = CASES;
    let mut jumps = [0u32; 6];
    for (group, jump) in jumps.iter_mut().enumerate().take(5) {
        *jump = case_at;
        let mode = GROUPS
            .iter()
            .position(|&g| usize::from(g) == group)
            .unwrap();
        let tag = u32::from(tags_of(mode)[0]);
        if group == 0 {
            // mov ecx, edi; call elsewhere; store; jump to the shared tail.
            case_at = put(
                &mut image,
                case_at,
                &[0x8b, 0xcf, 0xe8, 0x10, 0x00, 0x00, 0x00],
            );
        }
        case_at = store(&mut image, case_at, -0x14, tag);
        case_at = jump_short(&mut image, case_at, tail);
    }
    // Hold item has its own call sequence in the original; here it shares the tail.
    jumps[5] = SCROLL_PROLOGUE;
    for (index, jump) in jumps.iter().enumerate() {
        put(
            &mut image,
            GROUP_JUMPS + 4 * index as u32,
            &jump.to_le_bytes(),
        );
    }
    // Scroll group: base tag, count 2, range check, table jump.
    let mut at = store(&mut image, SCROLL_PROLOGUE, -0x10, 6);
    at = put(&mut image, at, &[0xb8, 2, 0, 0, 0]);
    at = put(&mut image, at, &[0x8d, 0x4b, 0xf6]);
    at = put(&mut image, at, &[0x83, 0xf9, 0x07]);
    at = put(&mut image, at, &[0x0f, 0x87, 0, 0, 0, 0]);
    put(
        &mut image,
        at,
        &[&[0xff, 0x24, 0x8d][..], &SCROLL_SUB_JUMPS.to_le_bytes()].concat(),
    );
    let mut tail_at = put(
        &mut image,
        SCROLL_TAIL,
        &[0x6a, 0, 0x50, 0x8d, 0x45, 0xf0, 0x50, 0x56],
    );
    call_sink(&mut image, tail_at);
    tail_at = SCROLL_TAIL;
    for sub in 0..8usize {
        let tags = tags_of(10 + sub);
        put(
            &mut image,
            SCROLL_SUB_JUMPS + 4 * sub as u32,
            &case_at.to_le_bytes(),
        );
        if tags.len() == 2 {
            case_at = store(&mut image, case_at, -0xc, u32::from(tags[1]));
            case_at = jump_short(&mut image, case_at, tail_at);
        } else {
            case_at = put(&mut image, case_at, &[0x6a, 0x00, 0xb8, 3, 0, 0, 0]);
            case_at = store(&mut image, case_at, -0xc, u32::from(tags[1]));
            case_at = put(&mut image, case_at, &[0x50, 0x8d, 0x45, 0xf0]);
            case_at = store(&mut image, case_at, -8, u32::from(tags[2]));
            case_at = put(&mut image, case_at, &[0x50, 0x56]);
            case_at = call_sink(&mut image, case_at);
        }
    }
    // Tints: white, yellow, red, green for hover; the same for armed; then the rest white.
    let palette: [[u8; 3]; 4] = [[255, 255, 255], [255, 255, 0], [255, 0, 0], [0, 255, 0]];
    for mode in 0..18usize {
        let tint = match mode {
            0..=7 => palette[mode % 4],
            8 => [255, 10, 32],
            _ => [255, 255, 255],
        };
        put(&mut image, TINTS + 3 * mode as u32, &tint);
    }
    for (site, (modrm, offset)) in TINT_LOADS
        .into_iter()
        .zip([(0x8c, 0), (0x84, 1), (0x84, 2)])
    {
        put(
            &mut image,
            site,
            &[
                &[0x0f, 0xb6, modrm, 0x52][..],
                &(TINTS + offset).to_le_bytes(),
            ]
            .concat(),
        );
    }
    // Pointer-state pushes.
    for (site, mode) in [
        (HOVER_NONE, 0),
        (HOVER_ALLY, 3),
        (HOVER_NEUTRAL, 1),
        (HOVER_ENEMY, 2),
        (HOVER_FLAGGED, 1),
        (TARGET_PLAIN, 4),
        (TARGET_NEUTRAL, 5),
        (TARGET_ENEMY, 6),
        (TARGET_ALLY, 7),
    ] {
        put(&mut image, site, &[0x6a, mode]);
    }
    put(&mut image, ITEM_ICON_SLOT, &[0x6a, 0x15]);
    // Edge masks: 1 up, 2 down, 4 left, 8 right and the corner sums; 3 and 7 share a default.
    put(&mut image, SCROLL_MASK_BOUND, &[0x83, 0xfb, 0x09]);
    put(
        &mut image,
        SCROLL_MASK_JUMP,
        &[&[0xff, 0x24, 0x9d][..], &MASK_JUMPS.to_le_bytes()].concat(),
    );
    put(&mut image, SCROLL_DEFAULT, &[0x8b, 0xcf]);
    let mut at: u32 = 0x5dfdfc;
    for (mask, mode) in [
        (1u32, 12u32),
        (2, 13),
        (4, 10),
        (5, 14),
        (6, 16),
        (8, 11),
        (9, 15),
        (10, 17),
    ] {
        put(&mut image, MASK_JUMPS + 4 * (mask - 1), &at.to_le_bytes());
        at = put(&mut image, at, &[&[0xbe][..], &mode.to_le_bytes()].concat());
        at = jump_short(&mut image, at, SCROLL_DEFAULT);
    }
    for mask in [3u32, 7] {
        put(
            &mut image,
            MASK_JUMPS + 4 * (mask - 1),
            &SCROLL_DEFAULT.to_le_bytes(),
        );
    }
    image
}

fn origin() -> AssetIdentity {
    AssetIdentity {
        source: "synthetic".into(),
        edition: "fixture".into(),
        version: "fixture".into(),
    }
}

fn rules() -> CursorRules {
    decode(&image(), origin()).unwrap()
}

#[test]
fn sequences_are_the_registered_tag_names_each_mode_passes() {
    let rules = rules();
    let name = |pointer| rules.look(pointer).sequence.to_string();
    assert_eq!(name(Pointer::Normal), "Normal");
    assert_eq!(name(Pointer::Hover(Relation::Enemy)), "Select");
    assert_eq!(name(Pointer::Target(None)), "Target");
    assert_eq!(
        name(Pointer::Target(Some(Relation::Friend))),
        "TargetSelect"
    );
    assert_eq!(
        name(Pointer::Scroll(edge::UP | edge::LEFT)),
        "Scroll Up Left"
    );
    assert_eq!(
        name(Pointer::Scroll(edge::DOWN | edge::RIGHT)),
        "Scroll Down Right"
    );
    assert_eq!(name(Pointer::Scroll(edge::RIGHT)), "Scroll Right");
}

#[test]
fn tints_follow_the_relation_of_the_hovered_unit() {
    assert_eq!(rules().item_icon_slot, 21);
    let rules = rules();
    let tint = |pointer| rules.look(pointer).tint;
    assert_eq!(tint(Pointer::Normal), [255, 255, 255]);
    assert_eq!(tint(Pointer::Hover(Relation::Friend)), [0, 255, 0]);
    assert_eq!(tint(Pointer::Hover(Relation::Neutral)), [255, 255, 0]);
    assert_eq!(tint(Pointer::Hover(Relation::Enemy)), [255, 0, 0]);
    assert_eq!(tint(Pointer::Target(None)), [255, 255, 255]);
    assert_eq!(tint(Pointer::Target(Some(Relation::Enemy))), [255, 0, 0]);
}

#[test]
fn an_edge_mask_without_a_scroll_cursor_keeps_the_normal_cursor() {
    let rules = rules();
    for mask in [0, 3, 7, 11, 15, 200] {
        assert_eq!(rules.look(Pointer::Scroll(mask)).mode, 0, "mask {mask}");
    }
}

#[test]
fn every_scroll_mode_is_named_by_distinct_tags() {
    let rules = rules();
    let mut names = Vec::new();
    for mask in [1, 2, 4, 5, 6, 8, 9, 10] {
        names.push(rules.look(Pointer::Scroll(mask)).sequence.to_string());
    }
    names.sort();
    names.dedup();
    assert_eq!(names.len(), 8);
    assert!(rules.modes().any(|(_, name)| name == "Scroll Down Left"));
}

#[test]
fn a_changed_instruction_form_is_refused_instead_of_guessed() {
    let mut broken = image();
    put(&mut broken, GROUP_JUMP, &[0xff, 0x24, 0x8d]);
    let error = decode(&broken, origin()).unwrap_err();
    assert!(error.contains("unexpected instruction"), "{error}");

    let mut broken = image();
    // The tail loses its sprite call: the case runs off into zeroed code.
    put(&mut broken, TAIL + 9, &[0x90; 5]);
    let error = decode(&broken, origin()).unwrap_err();
    assert!(error.contains("mode 0"), "{error}");
}

#[test]
fn a_tag_that_is_not_registered_is_refused() {
    let mut broken = image();
    // Tag 10 is registered as index 11 instead.
    let mut tag_ten = None;
    let body = TAG_REGISTRATION.start;
    for offset in 0..(TAG_REGISTRATION.end - body) {
        let at = RAW + (body + offset - CODE_VA) as usize;
        if broken[at..at + 2] == [0x6a, 0x0a] {
            tag_ten = Some(at);
        }
    }
    let at = tag_ten.expect("tag 10 registration");
    broken[at + 1] = 0x0b;
    let error = decode(&broken, origin()).unwrap_err();
    assert!(error.contains("tag 10 is not registered"), "{error}");
}

#[test]
fn other_images_refuse_before_decoding() {
    let error = CursorRules::read(&[0; 16], origin()).unwrap_err();
    assert!(error.contains("unsupported client image"), "{error}");
}

/// Needs the Legacy 1.29.2.9231 `Warcraft III.exe` named by `PCX_WC3_INSTALL`.
#[test]
#[ignore = "requires PCX_WC3_INSTALL with the reviewed Legacy Warcraft III.exe"]
fn reviewed_client_yields_the_documented_cursor_rules() {
    let install = std::env::var("PCX_WC3_INSTALL").expect("PCX_WC3_INSTALL");
    let path = std::path::Path::new(&install).join("Warcraft III.exe");
    let rules = CursorRules::read(&std::fs::read(&path).unwrap(), origin()).unwrap();
    let look = |pointer| {
        let look = rules.look(pointer);
        (look.mode, look.sequence.to_string(), look.tint)
    };
    assert_eq!(rules.item_icon_slot, 21);
    assert_eq!(look(Pointer::Normal), (0, "Normal".into(), [255, 255, 255]));
    assert_eq!(
        look(Pointer::Hover(Relation::Friend)),
        (3, "Select".into(), [0, 255, 0])
    );
    assert_eq!(
        look(Pointer::Hover(Relation::Neutral)),
        (1, "Select".into(), [255, 255, 0])
    );
    assert_eq!(
        look(Pointer::Hover(Relation::Enemy)),
        (2, "Select".into(), [255, 0, 0])
    );
    assert_eq!(
        look(Pointer::Target(None)),
        (4, "Target".into(), [255, 255, 255])
    );
    assert_eq!(
        look(Pointer::Target(Some(Relation::Enemy))),
        (6, "TargetSelect".into(), [255, 0, 0])
    );
    assert_eq!(
        look(Pointer::Scroll(edge::UP | edge::LEFT)).1,
        "Scroll Up Left"
    );
    let mut names: Vec<_> = rules.modes().map(|(_, name)| name).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), 13);
    assert_eq!(rules.modes().count(), 18);
}

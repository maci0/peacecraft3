use super::*;
use crate::pe_image::{PE_OFFSET_AT, PE32_MAGIC, SECTION_LEN};

const IMAGE_BASE: u32 = 0x40_0000;
const TEXT_RVA: u32 = 0x1000;
const FILE_ALIGN: usize = 0x200;
const SECTION_RVA_STEP: u32 = 0x1000;
const TEXT_RAW: usize = 0x400;
const SECTIONS_AT: usize = 0x58 + 0xe0;
const TEXT_HDR: usize = SECTIONS_AT;
const RDATA_HDR: usize = SECTIONS_AT + SECTION_LEN;
const PATH: &[u8] = b"units\\ItemData";
const PATH_NUL: &[u8] = b"\0units\\ItemData\0";
const NAMES: [&str; 3] = ["permanent", "charged", "miscellanous"];

/// Compare tails: `permanent` falls through to ret (0), `charged` moves 1, and
/// `miscellanous` uses the neg/sbb form with 6.
const TAILS: [&[u8]; 3] = [
    &[0x85, 0xc0, 0x75, 0x09, 0x5e, 0x5d, 0xc3, 0, 0, 0, 0, 0],
    &[0x85, 0xc0, 0x75, 0x09, 0xb8, 1, 0, 0, 0, 0x5e, 0x5d, 0xc3],
    &[
        0xf7, 0xd8, 0x1b, 0xc0, 0xf7, 0xd8, 0x83, 0xc0, 0x06, 0x5e, 0x5d, 0xc3,
    ],
];

/// What the synthetic image contains beyond the well-formed 1.29b-shaped chain.
#[derive(Default)]
struct Shape<'a> {
    /// Omit the item data path from `.rdata` and push another name in its place.
    path_missing: bool,
    /// Bytes appended to `.rdata` (for an ambiguous anchor).
    extra_rdata: &'a [u8],
    /// Bytes appended to `.text` after the chain (for a spoofed or second reference).
    extra_text: Vec<u8>,
    /// Replace the entries' compare tails; `None` uses `TAILS`.
    tails: Option<&'a [&'a [u8]]>,
}

fn call(at: usize, target: usize) -> [u8; 5] {
    let rel = target as i64 - (at as i64 + 5);
    [
        0xe8,
        rel as u8,
        (rel >> 8) as u8,
        (rel >> 16) as u8,
        (rel >> 24) as u8,
    ]
}

fn origin() -> AssetIdentity {
    AssetIdentity {
        edition: "synthetic edition".into(),
        version: "1.29.2.9231".into(),
        source: "synthetic image".into(),
    }
}

fn rdata_offset(rdata: &[u8], name: &[u8]) -> usize {
    rdata
        .windows(name.len() + 1)
        .position(|w| &w[..name.len()] == name && w[name.len()] == 0)
        .expect("name in rdata")
}

/// Builds a PE with `.text` and `.rdata`. Code: loader (push path; call row), row builder
/// (call parser), parser (null branch, then one compare entry per name).
fn image(shape: &Shape<'_>) -> Vec<u8> {
    let mut rdata = if shape.path_missing {
        vec![0]
    } else {
        [&[0u8][..], PATH, &[0u8][..]].concat()
    };
    for name in NAMES {
        rdata.extend_from_slice(name.as_bytes());
        rdata.push(0);
    }
    rdata.extend_from_slice(shape.extra_rdata);
    let rdata_va = IMAGE_BASE + TEXT_RVA + SECTION_RVA_STEP;
    let name_va = |name: &[u8]| rdata_va + rdata_offset(&rdata, name) as u32;
    let path_va = if shape.path_missing {
        name_va(NAMES[0].as_bytes())
    } else {
        name_va(PATH)
    };
    let tails = shape.tails.unwrap_or(&TAILS);
    let mut code = vec![0xc3u8; 0x100];
    let (loader, row, parser, compare) = (0x00usize, 0x20usize, 0x40usize, 0xc0usize);
    code[loader..loader + 3].copy_from_slice(&PROLOGUE);
    code[loader + 3] = PUSH_IMM32;
    code[loader + 4..loader + 8].copy_from_slice(&path_va.to_le_bytes());
    code[loader + 8..loader + 13].copy_from_slice(&call(loader + 8, row));
    code[loader + 13] = 0xc3;
    code[row..row + 3].copy_from_slice(&PROLOGUE);
    code[row + 3..row + 8].copy_from_slice(&call(row + 3, parser));
    code[row + 8] = 0xc3;
    let mut p = parser;
    code[p..p + 3].copy_from_slice(&PROLOGUE);
    p += 3;
    code[p..p + 8].copy_from_slice(&[0x56, 0x8b, 0x75, 0x08, 0x85, 0xf6, 0x75, 0x03]);
    p += 8;
    code[p..p + 6].copy_from_slice(&[0x8d, 0x46, 0x07, 0x5e, 0x5d, 0xc3]);
    p += 6;
    for (name, tail) in NAMES.iter().zip(tails) {
        code[p] = PUSH_IMM32;
        code[p + 1..p + 5].copy_from_slice(&name_va(name.as_bytes()).to_le_bytes());
        code[p + 5] = 0x56;
        code[p + 6..p + 11].copy_from_slice(&call(p + 6, compare));
        code[p + 11..p + 14].copy_from_slice(&ADD_ESP_8);
        code[p + 14..p + 14 + tail.len()].copy_from_slice(tail);
        p += 14 + tail.len();
    }
    code.extend_from_slice(&shape.extra_text);
    assemble(&code, &rdata)
}

fn assemble(code: &[u8], rdata: &[u8]) -> Vec<u8> {
    let text_len = code.len().next_multiple_of(FILE_ALIGN);
    let rdata_raw = TEXT_RAW + text_len;
    let rdata_len = rdata.len().next_multiple_of(FILE_ALIGN);
    let mut out = vec![0u8; rdata_raw + rdata_len];
    out[0..2].copy_from_slice(b"MZ");
    out[PE_OFFSET_AT..PE_OFFSET_AT + 4].copy_from_slice(&0x40u32.to_le_bytes());
    out[0x40..0x44].copy_from_slice(b"PE\0\0");
    out[0x44 + 2..0x44 + 4].copy_from_slice(&2u16.to_le_bytes());
    out[0x44 + 16..0x44 + 18].copy_from_slice(&0xe0u16.to_le_bytes());
    let opt = 0x58;
    out[opt..opt + 2].copy_from_slice(&PE32_MAGIC.to_le_bytes());
    out[opt + 28..opt + 32].copy_from_slice(&IMAGE_BASE.to_le_bytes());
    let sections = [
        (TEXT_HDR, *TEXT, TEXT_RVA, code, text_len, TEXT_RAW),
        (
            RDATA_HDR,
            *RDATA,
            TEXT_RVA + SECTION_RVA_STEP,
            rdata,
            rdata_len,
            rdata_raw,
        ),
    ];
    for (at, name, va, body, len, raw) in sections {
        out[at..at + 8].copy_from_slice(&name);
        out[at + 8..at + 12].copy_from_slice(&(body.len() as u32).to_le_bytes());
        out[at + 12..at + 16].copy_from_slice(&va.to_le_bytes());
        out[at + 16..at + 20].copy_from_slice(&(len as u32).to_le_bytes());
        out[at + 20..at + 24].copy_from_slice(&(raw as u32).to_le_bytes());
        out[raw..raw + body.len()].copy_from_slice(body);
    }
    out
}

fn set_u32(exe: &mut [u8], at: usize, value: u32) {
    exe[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

#[test]
fn derives_names_and_ordinals_and_keeps_the_image_identity() {
    let exe = image(&Shape::default());
    let table = derive(&exe, origin()).unwrap();
    assert_eq!(table.default, 7);
    assert_eq!(table.origin, origin());
    assert_eq!(table.image_sha256, <[u8; 32]>::from(Sha256::digest(&exe)));
    assert_eq!(
        table.ordinals,
        BTreeMap::from([
            ("charged".to_owned(), 1),
            ("miscellanous".to_owned(), 6),
            ("permanent".to_owned(), 0),
        ])
    );
    assert_eq!(
        derive(&exe, origin()).unwrap(),
        table,
        "derivation is deterministic"
    );
}

#[test]
fn refuses_non_pe_and_truncated_input() {
    assert!(matches!(
        derive(b"not an image", origin()),
        Err(ItemClassError::Image(_))
    ));
    let exe = image(&Shape::default());
    assert!(matches!(
        derive(&exe[..0x200], origin()),
        Err(ItemClassError::Image(_))
    ));
    assert!(matches!(
        derive(&exe[..0x30], origin()),
        Err(ItemClassError::Image(_))
    ));
}

#[test]
fn refuses_section_offsets_and_addresses_that_overflow_or_leave_the_image() {
    let exe = image(&Shape::default());
    let mut raw_beyond = exe.clone();
    set_u32(&mut raw_beyond, RDATA_HDR + 20, u32::MAX - 0x10);
    assert!(matches!(
        derive(&raw_beyond, origin()),
        Err(ItemClassError::Image(_))
    ));
    let mut rva_overflow = exe.clone();
    set_u32(&mut rva_overflow, TEXT_HDR + 12, 0xffff_f000);
    assert!(matches!(
        derive(&rva_overflow, origin()),
        Err(ItemClassError::Image(_))
    ));
    let mut too_many = exe.clone();
    too_many[0x44 + 2..0x44 + 4].copy_from_slice(&200u16.to_le_bytes());
    assert!(matches!(
        derive(&too_many, origin()),
        Err(ItemClassError::Image(_))
    ));
    let mut not_pe32 = exe.clone();
    not_pe32[0x58..0x5a].copy_from_slice(&0x20bu16.to_le_bytes());
    assert!(matches!(
        derive(&not_pe32, origin()),
        Err(ItemClassError::Image(_))
    ));
    let mut duplicate = exe.clone();
    duplicate[RDATA_HDR..RDATA_HDR + 8].copy_from_slice(TEXT);
    assert!(matches!(
        derive(&duplicate, origin()),
        Err(ItemClassError::Image(_))
    ));
}

#[test]
fn anchor_must_be_a_single_rdata_occurrence() {
    let missing = image(&Shape {
        path_missing: true,
        ..Shape::default()
    });
    assert_eq!(derive(&missing, origin()), Err(ItemClassError::Anchor));
    let twice = image(&Shape {
        extra_rdata: PATH_NUL,
        ..Shape::default()
    });
    assert_eq!(
        derive(&twice, origin()),
        Err(ItemClassError::AmbiguousAnchor)
    );
}

#[test]
fn a_spoofed_path_outside_rdata_is_not_an_anchor() {
    let spoof = [PATH, &[0u8][..]].concat();
    let exe = image(&Shape {
        path_missing: true,
        extra_text: spoof,
        ..Shape::default()
    });
    assert_eq!(derive(&exe, origin()), Err(ItemClassError::Anchor));
}

#[test]
fn a_second_code_reference_to_the_path_is_ambiguous() {
    let rdata_va = IMAGE_BASE + TEXT_RVA + SECTION_RVA_STEP;
    let path_va = rdata_va + 1;
    let mut second = vec![PUSH_IMM32];
    second.extend_from_slice(&path_va.to_le_bytes());
    let exe = image(&Shape {
        extra_text: second,
        ..Shape::default()
    });
    assert_eq!(derive(&exe, origin()), Err(ItemClassError::AmbiguousAnchor));
}

#[test]
fn refuses_a_chain_that_is_missing_or_has_an_unrecognized_compare_form() {
    let no_entries = image(&Shape {
        tails: Some(&[]),
        ..Shape::default()
    });
    assert_eq!(derive(&no_entries, origin()), Err(ItemClassError::Chain));
    let unknown = image(&Shape {
        tails: Some(&[TAILS[0], &[0x90; 12], TAILS[2]]),
        ..Shape::default()
    });
    assert!(matches!(
        derive(&unknown, origin()),
        Err(ItemClassError::OrdinalForm(_))
    ));
}

/// Original-input gate: `PCX_WAR3_EXE` names a local executable whose version resource is
/// 1.29.2.9231. Unset, it skips, and the synthetic tests carry the logic.
#[test]
fn original_executable_yields_the_chain_and_its_identity() {
    let Some(path) = std::env::var_os("PCX_WAR3_EXE") else {
        return;
    };
    let exe = std::fs::read(&path).expect("readable executable");
    assert!(
        exe.windows(b"1.29.2.9231".len())
            .any(|w| w == b"1.29.2.9231"),
        "version resource is 1.29.2.9231"
    );
    let origin = AssetIdentity {
        edition: "explicit gate selection".into(),
        version: "1.29.2.9231".into(),
        source: path.to_string_lossy().into_owned(),
    };
    let table = derive(&exe, origin).expect("chain in original executable");
    assert_eq!(table.default, 7);
    assert_eq!(
        table.ordinals.into_iter().collect::<Vec<_>>(),
        vec![
            ("artifact".to_owned(), 3),
            ("campaign".to_owned(), 5),
            ("charged".to_owned(), 1),
            ("miscellanous".to_owned(), 6),
            ("permanent".to_owned(), 0),
            ("powerup".to_owned(), 2),
            ("purchasable".to_owned(), 4),
        ]
    );
}

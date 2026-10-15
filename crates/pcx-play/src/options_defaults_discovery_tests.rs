use super::*;

const CODE_RAW: usize = 0x200;
const CODE_LEN: usize = 0x800;
const GETTER_RAW: usize = CODE_RAW + 0x40;
const DATA_RAW: usize = 0x1000;
const DATA_LEN: usize = 0x1000;

#[derive(Clone)]
struct Fixture {
    bytes: Vec<u8>,
    base: u32,
    origin: AssetIdentity,
    budget: DiscoveryBudget,
}
fn put(d: &mut [u8], at: usize, value: u32) {
    d[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn code(count: u8, shift: u8, field: u32) -> Vec<u8> {
    // Synthetic operands; the opcode/control-flow profile is the supported format.
    let mut b = vec![
        0x55, 0x8b, 0xec, 0x53, 0x56, 0x8b, 0x75, 8, 0x8b, 0xd9, 0x83, 0xfe, count, 0x7c, 8, 0x5e,
        0x33, 0xc0, 0x5b, 0x5d, 0xc2, 12, 0, 0x57, 0x68,
    ];
    b.extend_from_slice(&0x12345678u32.to_le_bytes());
    b.extend_from_slice(&[
        0x56, 0xe8, 7, 0, 0, 0, 0x85, 0xc0, 0x74, 5, 0x8d, 0x78, 0x18, 0xeb, 0x14, 0x8b, 0xc6,
        0x8b, 0xcb, 0xc1, 0xe0, shift, 0x8b, 0xb8,
    ]);
    b.extend_from_slice(&field.to_le_bytes());
    b.extend_from_slice(&[
        0x57, 0x56, 0xe8, 11, 0, 0, 0, 0xff, 0x75, 0x10, 0x57, 0xff, 0x75, 12, 0xe8, 17, 0, 0, 0,
        0x83, 0xc4, 12, 0xb8, 1, 0, 0, 0, 0x5f, 0x5e, 0x5b, 0x5d, 0xc2, 12, 0,
    ]);
    assert_eq!(b.len(), GETTER_BYTES);
    b
}
fn table(f: &mut Fixture, raw: usize, strings: usize, count: u8, shift: u8) {
    let stride = 1usize << shift;
    for index in 0..usize::from(count) {
        let row = raw + index * stride;
        let name = format!("option.synthetic.{index}");
        let value = format!("raw-{index}-text");
        let name_at = strings + index * 64;
        let value_at = name_at + 32;
        put(
            &mut f.bytes,
            row,
            f.base + 0x3000 + (name_at - DATA_RAW) as u32,
        );
        put(&mut f.bytes, row + 4, index as u32 + 73);
        put(&mut f.bytes, row + 8, index as u32 + 91);
        put(
            &mut f.bytes,
            row + stride - 4,
            f.base + 0x3000 + (value_at - DATA_RAW) as u32,
        );
        f.bytes[name_at..name_at + name.len()].copy_from_slice(name.as_bytes());
        f.bytes[value_at..value_at + value.len()].copy_from_slice(value.as_bytes());
    }
}
fn fixture(base: u32, count: u8, shift: u8) -> Fixture {
    let mut f = Fixture {
        bytes: vec![0; DATA_RAW + DATA_LEN],
        base,
        origin: AssetIdentity {
            edition: "synthetic".into(),
            version: "profile-fixture".into(),
            source: "synthetic:automatic-options".into(),
        },
        budget: DiscoveryBudget {
            image: RuleInputBudget {
                image_bytes: 0x4000,
                profile_bytes: 0x4000,
                sections: 2,
            },
            string_bytes: 256,
            candidate_limit: 8,
            work_bytes: 0x10000,
        },
    };
    f.bytes[..2].copy_from_slice(b"MZ");
    f.bytes[0x80..0x84].copy_from_slice(b"PE\0\0");
    for (at, value) in [(0x84, 0x14cu16), (0x86, 2), (0x94, 0x60), (0x98, 0x10b)] {
        f.bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
    }
    for (at, value) in [
        (0x3c, 0x80),
        (0xb4, base),
        (0xd0, 0x5000),
        (0x100, CODE_LEN as u32),
        (0x104, 0x1000),
        (0x108, CODE_LEN as u32),
        (0x10c, CODE_RAW as u32),
        (0x11c, 0x60000020),
        (0x128, DATA_LEN as u32),
        (0x12c, 0x3000),
        (0x130, DATA_LEN as u32),
        (0x134, DATA_RAW as u32),
        (0x144, 0x40000040),
    ] {
        put(&mut f.bytes, at, value);
    }
    let b = code(count, shift, base + 0x3000 + (1u32 << shift) - 4);
    f.bytes[GETTER_RAW..GETTER_RAW + b.len()].copy_from_slice(&b);
    table(&mut f, DATA_RAW, DATA_RAW + 0x400, count, shift);
    f
}
fn discover(f: &Fixture) -> Result<Defaults<'_>, Error> {
    Defaults::discover(&f.bytes, f.origin.clone(), f.budget)
}
fn assert_none(f: &Fixture) {
    assert!(matches!(
        discover(f).unwrap_err().problem,
        Problem::DiscoveryNone
    ));
}
fn assert_budget(f: &Fixture) {
    assert!(matches!(discover(f).unwrap_err().problem, Problem::Budget));
}

#[test]
fn discovers_complete_relocated_profiles_and_preserves_raw_source() {
    for (base, count, shift) in [(0x300000, 3, 4), (0x900000, 5, 5)] {
        let f = fixture(base, count, shift);
        let d = discover(&f).unwrap();
        assert_eq!(d.index_limit(), u32::from(count));
        assert_eq!(d.entries().len(), usize::from(count));
        assert_eq!(d.binding().image_sha256, sha256(&f.bytes));
        assert_eq!(d.binding().origin.source, f.origin.source);
        assert_eq!(d.binding().getter.va, base + 0x1040);
        assert_eq!(d.binding().rows[0].row.va, base + 0x3000);
        assert_eq!(d.get("option.synthetic.1").unwrap().fallback, b"raw-1-text");
        assert_eq!(d.entries()[0].raw.as_ptr(), f.bytes[DATA_RAW..].as_ptr());
        assert!(d.get("unselected.synthetic").is_none());
        let manual = Defaults::read(&f.bytes, d.binding().clone(), f.budget.image).unwrap();
        assert_eq!(manual.entries()[1].fallback, d.entries()[1].fallback);
    }
}

#[test]
fn relocates_code_table_sections_and_raw_offsets_independently() {
    let mut f = fixture(0x500000, 3, 4);
    let b = code(3, 4, f.base + 0x600c);
    f.bytes[GETTER_RAW..GETTER_RAW + GETTER_BYTES].fill(0);
    f.bytes[CODE_RAW + 0x200..CODE_RAW + 0x200 + GETTER_BYTES].copy_from_slice(&b);
    for index in 0..3 {
        for field in [0, 12] {
            let at = DATA_RAW + index * 16 + field;
            let old = u32::from_le_bytes(f.bytes[at..at + 4].try_into().unwrap());
            put(&mut f.bytes, at, old + 0x3000);
        }
    }
    put(&mut f.bytes, 0x104, 0x2000);
    put(&mut f.bytes, 0x12c, 0x6000);
    put(&mut f.bytes, 0xd0, 0x8000);
    f.bytes.resize(0x3000, 0);
    f.bytes.copy_within(DATA_RAW..DATA_RAW + DATA_LEN, 0x2000);
    put(&mut f.bytes, 0x134, 0x2000);
    let d = discover(&f).unwrap();
    assert_eq!(d.binding().getter.va, f.base + 0x2200);
    assert_eq!(d.binding().rows[0].row.va, f.base + 0x6000);
    assert_eq!(d.entries()[0].raw.as_ptr(), f.bytes[0x2000..].as_ptr());
}

#[test]
fn rejects_equal_and_distinct_valid_collisions() {
    for distinct in [false, true] {
        let mut f = fixture(0x400000, 3, 4);
        let table_offset = if distinct { 0x200 } else { 0 };
        if distinct {
            table(&mut f, DATA_RAW + 0x200, DATA_RAW + 0x800, 3, 4);
        }
        let second = code(3, 4, f.base + 0x300c + table_offset);
        f.bytes[GETTER_RAW + 0x100..GETTER_RAW + 0x100 + GETTER_BYTES].copy_from_slice(&second);
        assert!(matches!(
            discover(&f).unwrap_err().problem,
            Problem::DiscoveryAmbiguous
        ));
    }
}

#[test]
fn invalid_decoys_do_not_set_address_priority_but_still_consume_candidate_budget() {
    for first_invalid in [false, true] {
        let mut f = fixture(0x400000, 3, 4);
        let invalid = code(3, 4, f.base + 0x900c);
        if first_invalid {
            f.bytes
                .copy_within(GETTER_RAW..GETTER_RAW + GETTER_BYTES, GETTER_RAW + 0x100);
            f.bytes[GETTER_RAW..GETTER_RAW + GETTER_BYTES].copy_from_slice(&invalid);
        } else {
            f.bytes[GETTER_RAW + 0x100..GETTER_RAW + 0x100 + GETTER_BYTES]
                .copy_from_slice(&invalid);
        }
        assert_eq!(discover(&f).unwrap().entries().len(), 3);
        f.budget.candidate_limit = 1;
        assert_budget(&f);
    }
}

#[test]
fn complete_shape_and_initialized_code_are_required() {
    let original = fixture(0x400000, 3, 4);
    for at in [0, 13, 14, 37, 43, 51, 88] {
        let mut f = original.clone();
        f.bytes[GETTER_RAW + at] ^= 0x80;
        assert_none(&f);
    }
    for flags in [0x40000040, 0x20000020, 0x60000080] {
        let mut f = original.clone();
        put(&mut f.bytes, 0x11c, flags);
        assert_none(&f);
    }
    let mut f = original.clone();
    f.bytes
        .copy_within(GETTER_RAW..GETTER_RAW + GETTER_BYTES, DATA_RAW + 0xc00);
    f.bytes[GETTER_RAW..GETTER_RAW + GETTER_BYTES].fill(0);
    assert_none(&f);
    f.bytes[CODE_RAW + CODE_LEN - GETTER_BYTES + 1..CODE_RAW + CODE_LEN]
        .copy_from_slice(&original.bytes[GETTER_RAW..GETTER_RAW + GETTER_BYTES - 1]);
    assert_none(&f);
}

#[test]
fn bounds_and_malformed_rows_are_refused_without_partial_admission() {
    let original = fixture(0x400000, 3, 4);
    for value in [0, 0x80, 0xff] {
        let mut f = original.clone();
        f.bytes[GETTER_RAW + 12] = value;
        assert_none(&f);
    }
    for field in [0, 12] {
        let mut f = original.clone();
        put(&mut f.bytes, DATA_RAW + 2 * 16 + field, 0);
        assert_none(&f);
    }
    let mut f = original.clone();
    let name = u32::from_le_bytes(f.bytes[DATA_RAW..DATA_RAW + 4].try_into().unwrap());
    put(&mut f.bytes, DATA_RAW + 16, name);
    assert_none(&f);
    let mut f = original.clone();
    f.bytes[DATA_RAW + 0x400] = 0;
    assert_none(&f);
    let mut f = original.clone();
    f.bytes[DATA_RAW + 0x400] = 0xff;
    assert_none(&f);
    let mut f = original.clone();
    put(&mut f.bytes, 0x128, (DATA_LEN + 0x400) as u32);
    put(&mut f.bytes, DATA_RAW, f.base + 0x4000);
    assert_none(&f);
    let mut f = original.clone();
    let b = code(3, 4, f.base + 0x4000 - 4);
    f.bytes[GETTER_RAW..GETTER_RAW + GETTER_BYTES].copy_from_slice(&b);
    assert_none(&f);
    let mut f = original.clone();
    f.bytes.truncate(DATA_RAW + DATA_LEN - 1);
    assert!(matches!(
        discover(&f).unwrap_err().problem,
        Problem::Image(_)
    ));
    let mut f = original;
    f.bytes[GETTER_RAW + 50] = 31;
    assert_none(&f);
    let b = code(3, 31, 0xfffffff0);
    f.bytes[GETTER_RAW..GETTER_RAW + GETTER_BYTES].copy_from_slice(&b);
    assert_budget(&f);
}

#[test]
fn empty_and_non_utf8_fallbacks_are_raw_and_section_end_nul_is_valid() {
    let mut f = fixture(0x400000, 3, 4);
    put(&mut f.bytes, DATA_RAW + 12, f.base + 0x3fff);
    f.bytes[DATA_RAW + 0x400 + 64 + 32] = 0xff;
    f.bytes[DATA_RAW + 0x400 + 64 + 33] = 0;
    let d = discover(&f).unwrap();
    assert!(d.entries()[0].fallback.is_empty());
    assert_eq!(d.entries()[1].fallback, [0xff]);
    assert_eq!(d.binding().rows[0].fallback.len, 1);
}

#[test]
fn exact_budget_edges_and_late_exhaustion_are_global() {
    let mut f = fixture(0x400000, 3, 4);
    let d = discover(&f).unwrap();
    let source_bytes = GETTER_BYTES
        + d.binding()
            .rows
            .iter()
            .map(|r| (r.row.len + r.name.len + r.fallback.len) as usize)
            .sum::<usize>();
    let max_string = d
        .binding()
        .rows
        .iter()
        .flat_map(|r| [r.name.len, r.fallback.len])
        .max()
        .unwrap() as usize;
    f.budget.image.image_bytes = f.bytes.len();
    f.budget.image.profile_bytes = source_bytes;
    f.budget.string_bytes = max_string;
    f.budget.candidate_limit = 1;
    f.budget.work_bytes = CODE_LEN + source_bytes - GETTER_BYTES;
    assert!(discover(&f).is_ok());
    let exact = f.clone();
    f.budget.image.image_bytes -= 1;
    assert_budget(&f);
    f = exact.clone();
    f.budget.image.profile_bytes -= 1;
    assert_budget(&f);
    f = exact.clone();
    f.budget.string_bytes -= 1;
    assert_budget(&f);
    f = exact.clone();
    f.budget.work_bytes -= 1;
    assert_budget(&f);
    f = exact.clone();
    f.budget.candidate_limit = 0;
    assert_budget(&f);
    f = exact;
    let later = code(3, 4, f.base + 0x900c);
    f.bytes[GETTER_RAW + 0x100..GETTER_RAW + 0x100 + GETTER_BYTES].copy_from_slice(&later);
    f.budget.candidate_limit = 2;
    // The valid candidate has already consumed the exact work allowance.
    assert_budget(&f);
}

#[test]
fn unterminated_strings_hit_global_limits_and_bad_origin_is_not_invented() {
    let mut f = fixture(0x400000, 3, 4);
    f.bytes[DATA_RAW + 0x400..DATA_RAW + 0x400 + 256].fill(b'x');
    assert_budget(&f);
    let mut f = fixture(0x400000, 3, 4);
    f.origin.version.clear();
    assert!(matches!(
        discover(&f).unwrap_err().problem,
        Problem::Identity
    ));
}

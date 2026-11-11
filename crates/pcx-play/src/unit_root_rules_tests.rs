use super::*;

fn identity() -> AssetIdentity {
    AssetIdentity {
        source: "synthetic-root".into(),
        edition: "nonstock".into(),
        version: "relocated".into(),
    }
}
const ROOT: u32 = 7000;
const UNROOT: u32 = 7001;
const BIT: u32 = 1 << 20;
fn constant(value: u32) -> Vec<u8> {
    let mut b = vec![0xb8];
    b.extend(value.to_le_bytes());
    b.push(0xc3);
    b
}
fn fixture(shift: u32) -> Vec<(u32, Vec<u8>, bool)> {
    let n = shift + 0x30000;
    let f = shift + 0x10000;
    let table = shift + 0x40000;
    let mut memory = vec![(n, format!("{CLASS_NAME}\0").into_bytes(), false)];
    memory.push((f, constant(n), true));
    memory.push((f + 0x10, constant(u32::from_be_bytes(*b"X001")), true));
    memory.push((f + 0x20, constant(ROOT), true));
    memory.push((f + 0x30, constant(UNROOT), true));
    let mut v = vec![0; VTABLE_BYTES];
    for (slot, target) in [
        (NAME_SLOT, f),
        (CODE_SLOT, f + 0x10),
        (ROOT_ORDER_SLOT, f + 0x20),
        (UNROOT_ORDER_SLOT, f + 0x30),
        (ROOT_VALIDITY_SLOT, f + 0x100),
        (UNROOT_VALIDITY_SLOT, f + 0x200),
    ] {
        v[slot..slot + 4].copy_from_slice(&target.to_le_bytes());
    }
    memory.push((table, v, false));
    let mut root_check = vec![0x90; VALIDITY_BYTES];
    root_check[ROOT_COMPARE_AT - 3..ROOT_COMPARE_AT].copy_from_slice(&[0x81, 0x7d, 0x08]);
    root_check[ROOT_COMPARE_AT..ROOT_COMPARE_AT + 4].copy_from_slice(&ROOT.to_le_bytes());
    root_check[ROOT_MASK_AT - 1] = 0x25;
    root_check[ROOT_MASK_AT..ROOT_MASK_AT + 4].copy_from_slice(&BIT.to_le_bytes());
    memory.push((f + 0x100, root_check, true));
    let mut unroot_check = vec![0x90; VALIDITY_BYTES];
    unroot_check[..6].copy_from_slice(&[0x55, 0x8b, 0xec, 0x81, 0x7d, 0x08]);
    unroot_check[UNROOT_COMPARE_AT..UNROOT_COMPARE_AT + 4].copy_from_slice(&UNROOT.to_le_bytes());
    unroot_check[UNROOT_MASK_AT - 1] = 0x25;
    unroot_check[UNROOT_MASK_AT..UNROOT_MASK_AT + 4].copy_from_slice(&BIT.to_le_bytes());
    memory.push((f + 0x200, unroot_check, true));
    memory
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
fn decode(memory: &[(u32, Vec<u8>, bool)]) -> Result<Rules, String> {
    decode_with_work(memory, 128 << 20)
}
fn index_of(memory: &[(u32, Vec<u8>, bool)], at: u32) -> usize {
    memory
        .iter()
        .position(|r| r.0 == at)
        .expect("fixture region")
}

#[test]
fn relocated_class_orders_and_state_bit_are_derived() {
    for shift in [0, 0x80000] {
        let rules = decode(&fixture(shift)).unwrap();
        assert_eq!(rules.class.code, *b"X001");
        assert_eq!(rules.class.root, ROOT as i32);
        assert_eq!(rules.class.unroot, UNROOT as i32);
        assert_eq!(rules.class.rooted_state_bit, BIT);
    }
}

#[test]
fn ambiguous_malformed_or_inconsistent_source_refuses() {
    let m = fixture(0);
    let mut dupe = m.clone();
    let name = m[0].clone();
    dupe.push((name.0 + 0x100000, name.1, name.2));
    assert!(decode(&dupe).is_err(), "duplicate class name");
    assert!(decode_with_work(&m, 1).is_err(), "exhausted scan budget");

    let mut bad = fixture(0);
    let at = index_of(&bad, 0x10000 + 0x20);
    bad[at].1[0] = 0x90;
    assert!(decode(&bad).is_err(), "root order getter is not a constant");

    let mut bad = fixture(0);
    let at = index_of(&bad, 0x10000 + 0x200);
    bad[at].1[UNROOT_COMPARE_AT] ^= 1;
    assert!(
        decode(&bad).is_err(),
        "unroot validity compares another order"
    );

    let mut bad = fixture(0);
    let at = index_of(&bad, 0x10000 + 0x100);
    bad[at].1[ROOT_MASK_AT] ^= 1;
    assert!(
        decode(&bad).is_err(),
        "validity functions test different bits"
    );

    let mut bad = fixture(0);
    let at = index_of(&bad, 0x10000 + 0x200);
    bad[at].1[UNROOT_MASK_AT - 1] = 0x90;
    assert!(decode(&bad).is_err(), "state bit test missing");

    let mut bad = fixture(0);
    let at = index_of(&bad, 0x10000 + 0x30);
    bad[at].1[1..5].copy_from_slice(&ROOT.to_le_bytes());
    assert!(decode(&bad).is_err(), "root and unroot orders coincide");
}

#[test]
#[ignore = "requires PCX_WAR3_EXE pointing to the reviewed local client"]
fn selected_original_root_class_binds() {
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
    assert_eq!(rules.class.code, *b"Aroo");
    assert_ne!(rules.class.root, rules.class.unroot);
    assert_eq!(rules.class.rooted_state_bit, 0x0800_0000);
}

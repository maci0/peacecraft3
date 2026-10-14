use pcx_trig::TriggerArity;

#[path = "../src/wtg_v7.rs"]
mod wtg_v7;

fn arity() -> TriggerArity {
    let mut a = TriggerArity::new();
    a.insert(b"DoNothing", 0).unwrap();
    a
}

fn base() -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(b"WTG!");
    v.extend_from_slice(&7u32.to_le_bytes());
    // one category
    v.extend_from_slice(&1u32.to_le_bytes());
    v.extend_from_slice(&5u32.to_le_bytes());
    v.extend_from_slice(b"C\0");
    v.extend_from_slice(&0u32.to_le_bytes()); // is_comment
    v.extend_from_slice(&1u32.to_le_bytes()); // is_expandable
    v.extend_from_slice(&0u32.to_le_bytes()); // parent
    // unknown root
    v.extend_from_slice(&1u32.to_le_bytes());
    // one variable
    v.extend_from_slice(&1u32.to_le_bytes());
    v.extend_from_slice(b"v\0");
    v.extend_from_slice(b"integer\0");
    v.extend_from_slice(&1u32.to_le_bytes()); // unknown
    v.extend_from_slice(&1u32.to_le_bytes()); // is_array
    v.extend_from_slice(&3u32.to_le_bytes()); // array_size
    v.extend_from_slice(&0u32.to_le_bytes()); // is_initialized
    v.extend_from_slice(b"\0"); // initial value
    // one trigger
    v.extend_from_slice(&1u32.to_le_bytes());
    v.extend_from_slice(b"T\0");
    v.extend_from_slice(b"\0"); // description
    v.extend_from_slice(&0u32.to_le_bytes()); // is_comment
    v.extend_from_slice(&0u32.to_le_bytes()); // id
    v.extend_from_slice(&1u32.to_le_bytes()); // enabled
    v.extend_from_slice(&0u32.to_le_bytes()); // custom
    v.extend_from_slice(&0u32.to_le_bytes()); // initially off
    v.extend_from_slice(&0u32.to_le_bytes()); // run on init
    v.extend_from_slice(&5u32.to_le_bytes()); // parent id
    v.extend_from_slice(&1u32.to_le_bytes()); // eca count
    // one ECA (action, top-level: no group)
    v.extend_from_slice(&2u32.to_le_bytes()); // kind action
    v.extend_from_slice(b"DoNothing\0");
    v.extend_from_slice(&1u32.to_le_bytes()); // enabled
    v.extend_from_slice(&0u32.to_le_bytes()); // child_branch
    v.extend_from_slice(&0u32.to_le_bytes()); // child count
    v
}

#[test]
fn parses_and_round_trips() {
    let b = base();
    let w = wtg_v7::WtgV7::parse(&b, &arity()).unwrap();
    assert_eq!(w.version(), 7);
    assert_eq!(w.categories().len(), 1);
    assert_eq!(w.variables()[0].array_size, 3);
    assert_eq!(w.triggers()[0].ecas.len(), 1);
    assert_eq!(w.canonical_bytes(), b);
}

#[test]
fn rejects_bad_version() {
    let mut b = base();
    b[4..8].copy_from_slice(&4u32.to_le_bytes());
    assert!(matches!(
        wtg_v7::WtgV7::parse(&b, &arity()),
        Err(wtg_v7::Error::BadVersion { .. })
    ));
}

#[test]
fn preserves_child_eca() {
    // append a child to the single ECA instead of flat
    let mut b = base();
    // remove trailing child count 0 already at end? rebuild quickly by replacing last 4 bytes
    // last 4 bytes is child count 0 -> set to 1 and append child ECA
    let n = b.len();
    b[n - 4..n].copy_from_slice(&1u32.to_le_bytes());
    // child: kind action, group 1, name DoNothing, enabled 1, no params, branch 0, 0 children
    b.extend_from_slice(&2u32.to_le_bytes()); // kind
    b.extend_from_slice(&1u32.to_le_bytes()); // group
    b.extend_from_slice(b"DoNothing\0");
    b.extend_from_slice(&1u32.to_le_bytes()); // enabled
    b.extend_from_slice(&0u32.to_le_bytes()); // branch
    b.extend_from_slice(&0u32.to_le_bytes()); // child count
    // fix original parent child_count already set to 1 above; trailing root still has trigger. Parse:
    let w = wtg_v7::WtgV7::parse(&b, &arity()).unwrap();
    assert_eq!(w.triggers()[0].ecas[0].child_ecas.len(), 1);
    assert_eq!(w.triggers()[0].ecas[0].child_ecas[0].group, Some(1));
    assert_eq!(w.canonical_bytes(), b);
}

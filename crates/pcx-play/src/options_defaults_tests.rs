use super::*;

fn put(d: &mut [u8], at: usize, v: u32) {
    d[at..at + 4].copy_from_slice(&v.to_le_bytes());
}
fn fixture(base: u32, limit: u8) -> (Vec<u8>, Binding, RuleInputBudget) {
    let mut d = vec![0; 0x800];
    d[..2].copy_from_slice(b"MZ");
    d[0x80..0x84].copy_from_slice(b"PE\0\0");
    for (at, v) in [(0x84, 0x14cu16), (0x86, 2), (0x94, 0x60), (0x98, 0x10b)] {
        d[at..at + 2].copy_from_slice(&v.to_le_bytes());
    }
    for (at, v) in [
        (0x3c, 0x80),
        (0xb4, base),
        (0xd0, 0x3000),
        (0x100, 0x200),
        (0x104, 0x1000),
        (0x108, 0x200),
        (0x10c, 0x200),
        (0x11c, 0x60000020),
        (0x128, 0x400),
        (0x12c, 0x2000),
        (0x130, 0x400),
        (0x134, 0x400),
        (0x144, 0x40000040),
    ] {
        put(&mut d, at, v);
    }
    // Synthetic call destinations and operands; no game payload in the fixture.
    let mut code = vec![
        0x55, 0x8b, 0xec, 0x53, 0x56, 0x8b, 0x75, 8, 0x8b, 0xd9, 0x83, 0xfe, limit, 0x7c, 8, 0x5e,
        0x33, 0xc0, 0x5b, 0x5d, 0xc2, 12, 0, 0x57, 0x68,
    ];
    code.extend_from_slice(&(base + 0x2800).to_le_bytes());
    code.extend_from_slice(&[
        0x56, 0xe8, 1, 0, 0, 0, 0x85, 0xc0, 0x74, 5, 0x8d, 0x78, 0x18, 0xeb, 0x14, 0x8b, 0xc6,
        0x8b, 0xcb, 0xc1, 0xe0, 4, 0x8b, 0xb8,
    ]);
    code.extend_from_slice(&(base + 0x200c).to_le_bytes());
    code.extend_from_slice(&[
        0x57, 0x56, 0xe8, 2, 0, 0, 0, 0xff, 0x75, 0x10, 0x57, 0xff, 0x75, 12, 0xe8, 3, 0, 0, 0,
        0x83, 0xc4, 12, 0xb8, 1, 0, 0, 0, 0x5f, 0x5e, 0x5b, 0x5d, 0xc2, 12, 0,
    ]);
    d[0x220..0x220 + code.len()].copy_from_slice(&code);
    let mut rows = Vec::new();
    for (index, offset, name, fallback) in [
        (4u32, 0x600, "synthetic.size", b"173".as_slice()),
        (1, 0x650, "synthetic.sound", b"not a number".as_slice()),
    ] {
        let at = 0x400 + index as usize * 16;
        let name_va = base + 0x2000 + (offset - 0x400) as u32;
        let value_offset = offset + name.len() + 1;
        let value_va = name_va + name.len() as u32 + 1;
        put(&mut d, at, name_va);
        put(&mut d, at + 4, index + 39);
        put(&mut d, at + 8, index + 71);
        put(&mut d, at + 12, value_va);
        d[offset..offset + name.len()].copy_from_slice(name.as_bytes());
        d[value_offset..value_offset + fallback.len()].copy_from_slice(fallback);
        rows.push(RowBinding {
            index,
            row: SourceWindow {
                va: base + 0x2000 + index * 16,
                len: 16,
                digest: sha256(&d[at..at + 16]),
            },
            name: SourceWindow {
                va: name_va,
                len: name.len() as u32 + 1,
                digest: sha256(&d[offset..offset + name.len() + 1]),
            },
            fallback: SourceWindow {
                va: value_va,
                len: fallback.len() as u32 + 1,
                digest: sha256(&d[value_offset..value_offset + fallback.len() + 1]),
            },
        });
    }
    let binding = Binding {
        origin: AssetIdentity {
            edition: "synthetic".into(),
            version: "fixture".into(),
            source: "synthetic:preferences.exe".into(),
        },
        image_sha256: sha256(&d),
        getter: SourceWindow {
            va: base + 0x1020,
            len: code.len() as u32,
            digest: sha256(&code),
        },
        rows,
    };
    (
        d,
        binding,
        RuleInputBudget {
            image_bytes: 4096,
            profile_bytes: 1024,
            sections: 2,
        },
    )
}
fn offset(d: &[u8], va: u32) -> usize {
    let base = u32::from_le_bytes(d[0xb4..0xb8].try_into().unwrap());
    match va - base {
        n if n < 0x2000 => (n - 0x1000 + 0x200) as usize,
        n => (n - 0x2000 + 0x400) as usize,
    }
}
fn refresh(d: &[u8], b: &mut Binding) {
    b.image_sha256 = sha256(d);
    for w in std::iter::once(&mut b.getter).chain(
        b.rows
            .iter_mut()
            .flat_map(|r| [&mut r.row, &mut r.name, &mut r.fallback]),
    ) {
        let at = offset(d, w.va);
        w.digest = sha256(&d[at..at + w.len as usize]);
    }
}
#[test]
fn source_operands_and_strings_survive_relocation_and_partial_selection() {
    for (base, limit) in [(0x300000, 7), (0x700000, 19)] {
        let (d, b, n) = fixture(base, limit);
        let input = Defaults::read(&d, b, n).unwrap();
        assert_eq!(input.index_limit(), u32::from(limit));
        assert_eq!(
            input.entries().iter().map(|e| e.index).collect::<Vec<_>>(),
            [4, 1]
        );
        assert_eq!(input.get("synthetic.size").unwrap().fallback, b"173");
        assert_eq!(
            input.get("synthetic.sound").unwrap().fallback,
            b"not a number"
        );
        assert!(input.get("reswidth").is_none());
        assert_eq!(
            input.get("synthetic.size").unwrap().raw.as_ptr(),
            d[0x440..].as_ptr()
        );
        assert_eq!(input.binding().origin.source, "synthetic:preferences.exe");
    }
}
#[test]
fn control_flow_and_addressing_mutations_are_refused() {
    let (d, b, n) = fixture(0x300000, 7);
    for i in [
        0, 10, 13, 14, 36, 37, 42, 43, 48, 50, 51, 55, 57, 64, 71, 79, 88,
    ] {
        let mut changed = d.clone();
        let mut selection = b.clone();
        changed[0x220 + i] ^= 0x80;
        refresh(&changed, &mut selection);
        assert!(
            Defaults::read(&changed, selection, n).is_err(),
            "mutant {i}"
        );
    }
    for limit in [0, 0x80, 0xff] {
        let mut changed = d.clone();
        let mut selection = b.clone();
        changed[0x220 + 12] = limit;
        refresh(&changed, &mut selection);
        assert!(matches!(
            Defaults::read(&changed, selection, n).unwrap_err().problem,
            Problem::Getter
        ));
    }
}
#[test]
fn caller_limits_precede_selection_allocation() {
    let (d, b, mut n) = fixture(0x300000, 7);
    let total = b.getter.len as usize
        + b.rows
            .iter()
            .map(|r| (r.row.len + r.name.len + r.fallback.len) as usize)
            .sum::<usize>();
    n.profile_bytes = total;
    assert!(Defaults::read(&d, b.clone(), n).is_ok());
    n.profile_bytes -= 1;
    assert!(matches!(
        Defaults::read(&d, b.clone(), n).unwrap_err().problem,
        Problem::Budget
    ));
    n.profile_bytes = usize::MAX;
    n.image_bytes = d.len() - 1;
    assert!(matches!(
        Defaults::read(&d, b, n).unwrap_err().problem,
        Problem::Budget
    ));
}
#[test]
fn wrong_image_range_identity_and_bss_are_refused() {
    let (d, b, n) = fixture(0x300000, 7);
    let mut wrong = b.clone();
    wrong.image_sha256[0] ^= 1;
    assert!(matches!(
        Defaults::read(&d, wrong, n).unwrap_err().problem,
        Problem::Identity
    ));
    let mut wrong = b.clone();
    wrong.rows[0].row.digest[0] ^= 1;
    assert!(matches!(
        Defaults::read(&d, wrong, n).unwrap_err().problem,
        Problem::Image(_)
    ));
    let mut wrong = b.clone();
    wrong.origin.version.clear();
    assert!(matches!(
        Defaults::read(&d, wrong, n).unwrap_err().problem,
        Problem::Identity
    ));
    let mut data = d.clone();
    put(&mut data, 0x130, 0x100);
    let mut wrong = b;
    refresh(&data, &mut wrong);
    assert!(matches!(
        Defaults::read(&data, wrong, n).unwrap_err().problem,
        Problem::Image(_)
    ));
}
#[test]
fn empty_duplicate_and_out_of_bound_selections_refuse() {
    let (d, b, n) = fixture(0x300000, 7);
    let mut wrong = b.clone();
    wrong.rows.clear();
    assert!(Defaults::read(&d, wrong, n).is_err());
    let mut wrong = b.clone();
    wrong.rows.push(wrong.rows[0].clone());
    assert!(Defaults::read(&d, wrong, n).is_err());
    for index in [7, u32::MAX] {
        let mut wrong = b.clone();
        wrong.rows[0].index = index;
        assert!(matches!(
            Defaults::read(&d, wrong, n).unwrap_err().problem,
            Problem::Row { .. }
        ));
    }
    let mut wrong = b;
    wrong.rows[1].row.va += 16;
    assert!(Defaults::read(&d, wrong, n).is_err());
}
#[test]
fn direct_pointer_binding_and_exact_nul_framing_are_required() {
    let (d, b, n) = fixture(0x300000, 7);
    let mut wrong = b.clone();
    wrong.rows[0].fallback = b.rows[1].fallback;
    assert!(Defaults::read(&d, wrong, n).is_err());
    for at in [0x600, 0x60e, 0x612] {
        let mut changed = d.clone();
        let mut selection = b.clone();
        changed[at] = if at == 0x600 { 0 } else { b'X' };
        refresh(&changed, &mut selection);
        assert!(
            Defaults::read(&changed, selection, n).is_err(),
            "offset {at:x}"
        );
    }
    let mut changed = d;
    let mut selection = b;
    put(&mut changed, 0x44c, 0);
    refresh(&changed, &mut selection);
    assert!(matches!(
        Defaults::read(&changed, selection, n).unwrap_err().problem,
        Problem::NullFallback { index: 4 }
    ));
}
#[test]
fn duplicate_names_refuse_and_non_numeric_values_stay_raw() {
    let (mut d, mut b, n) = fixture(0x300000, 7);
    let va = b.rows[0].name.va;
    put(&mut d, 0x410, va);
    b.rows[1].name = b.rows[0].name;
    refresh(&d, &mut b);
    assert!(Defaults::read(&d, b, n).is_err());
    let (mut d, mut b, n) = fixture(0x300000, 7);
    let at = offset(&d, b.rows[0].fallback.va);
    d[at] = 0xff;
    refresh(&d, &mut b);
    assert_eq!(
        Defaults::read(&d, b, n).unwrap().entries()[0].fallback,
        [0xff, b'7', b'3']
    );
}

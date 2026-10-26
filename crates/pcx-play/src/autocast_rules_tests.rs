use super::*;
fn identity() -> AssetIdentity {
    AssetIdentity {
        source: "synthetic-autocast".into(),
        edition: "nonstock".into(),
        version: "relocated".into(),
    }
}
fn fixture(shift: u32, mask: u8) -> Vec<(u32, Vec<u8>, bool)> {
    let mut memory = Vec::new();
    for (i, name) in ["CAbilitySlow", "CAbilityInnerFire", "CAbilityHeal"]
        .iter()
        .enumerate()
    {
        let n = shift + 0x30000 + i as u32 * 0x1000;
        let f = shift + 0x10000 + i as u32 * 0x1000;
        let table = shift + 0x40000 + i as u32 * 0x1000;
        memory.push((n, format!("{name}\0").into_bytes(), false));
        let constant = |value: u32| {
            let mut b = vec![0xb8];
            b.extend(value.to_le_bytes());
            b.push(0xc3);
            b
        };
        memory.push((f, constant(n), true));
        memory.push((
            f + 0x10,
            constant(u32::from_be_bytes([b'X', b'0', b'0' + i as u8, b'1'])),
            true,
        ));
        memory.push((f + 0x20, constant(7000 + i as u32 * 2), true));
        memory.push((f + 0x30, constant(7001 + i as u32 * 2), true));
        let mut v = vec![0; 0x350];
        for (slot, target) in [
            (0x58, f),
            (0x1c, f + 0x10),
            (0x310, f + 0x20),
            (0x320, f + 0x30),
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
        let mut b = vec![0x90; 142];
        b[..6].copy_from_slice(&[0x56, 0x8b, 0xf1, 0x57, 0x8b, 0x7e]);
        b[47..56].copy_from_slice(&[0xf6, 0x46, 0x24, mask, 0x74, 86, 0x81, 0x66, 0x24]);
        b[56..60].copy_from_slice(&(!u32::from(mask)).to_le_bytes());
        b[60..66].copy_from_slice(&[0x8b, 0xce, 0x8b, 0x06, 0xff, 0x90]);
        b[139..].copy_from_slice(&[0x5f, 0x5e, 0xc3]);
        memory.push((f + 0x100, b, true));
    }
    memory
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
    }
}
#[test]
fn ambiguous_or_malformed_source_refuses() {
    let mut m = fixture(0, 0x40);
    let dupe = m[0].clone();
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
    assert_eq!(rules.classes.len(), 3);
    assert!(rules.classes.iter().all(|c| c.on != c.off));
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

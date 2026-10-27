use super::*;
fn identity() -> AssetIdentity {
    AssetIdentity {
        source: "synthetic-morph".into(),
        edition: "nonstock".into(),
        version: "relocated".into(),
    }
}
fn fixture(shift: u32, _mask: u8) -> Vec<(u32, Vec<u8>, bool)> {
    let mut memory = Vec::new();
    for (i, name) in ["CAbilityRavenForm"].iter().enumerate() {
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
            (0x80, f + 0x200),
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
        let mut init = vec![0x90; 0x70];
        init[0x44..0x4b].copy_from_slice(&[0xa8, 0x40, 0x74, 0x07, 0x81, 0x4f, 0x20]);
        init[0x4f..0x56].copy_from_slice(&[0xa8, 0x08, 0x74, 0x07, 0x81, 0x4f, 0x20]);
        memory.push((f + 0x200, init, true));
        memory.push((
            f + 0x40,
            vec![0x8b, 0x01, 0xff, 0x90, 0x44, 0x03, 0, 0, 0xc2, 4, 0],
            true,
        ));
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
fn relocated_nonstock_class_and_orders_are_derived() {
    for shift in [0, 0x80000] {
        let rules = decode(&fixture(shift, 0x40)).unwrap();
        assert_eq!(rules.classes[0].code, *b"X001");
        assert_eq!(rules.classes[0].on, 7000);
        assert_eq!(rules.classes[0].off, 7001);
        assert_eq!(rules.classes[0].uninterruptible, 0x08);
        assert_eq!(rules.classes[0].immediate_takeoff, 0x40);
    }
}
#[test]
fn ambiguous_or_malformed_source_refuses() {
    let mut m = fixture(0, 0x40);
    let dupe = m[0].clone();
    m.push((dupe.0 + 0x100000, dupe.1, dupe.2));
    assert!(decode(&m).is_err());
    let mut m = fixture(0, 0x40);
    m[2].1[0] = 0x90;
    assert!(decode(&m).is_err());
    assert!(decode_with_work(&fixture(0, 0x40), 1).is_err());
}
#[test]
#[ignore = "requires PCX_WAR3_EXE pointing to the reviewed local client"]
fn selected_original_raven_class_binds() {
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
    assert!(rules.flight.is_ok(), "{:?}", rules.flight);
    assert_eq!(rules.classes.len(), 1);
    assert_eq!(rules.classes[0].code, *b"Arav");
    assert_ne!(rules.classes[0].on, rules.classes[0].off);
}

#[test]
fn ranked_nonstock_operands_and_malformed_or_other_modes() {
    let class = Class {
        code: *b"X001",
        on: 7,
        off: 8,
        uninterruptible: 8,
        immediate_takeoff: 64,
    };
    let cells = |key: &str| {
        match key {
            "DataA" => "u001",
            "UnitID" => "u002",
            "DataB" => "72",
            "Cost" => "17",
            "Cast" => "0.25",
            "Dur" => "0.75",
            "DataC" => "2.5",
            "DataD" => "1.25",
            _ => unreachable!(),
        }
        .to_owned()
    };
    let spec = Spec::read(&class, |k, _| Ok(cells(k))).unwrap();
    assert_eq!(spec.normal, *b"u001");
    assert_eq!(spec.alternate, *b"u002");
    assert_eq!(
        (
            spec.mana_cost,
            spec.cast,
            spec.duration,
            spec.altitude_duration,
            spec.landing_delay
        ),
        (17., 0.25, 0.75, 2.5, 1.25)
    );
    for (key, invalid) in [
        ("DataB", "0"),
        ("DataB", "-1"),
        ("DataA", "_"),
        ("UnitID", "u001"),
        ("Dur", "NaN"),
        ("DataD", "-0.5"),
        ("DataC", ""),
        ("Cost", "oops"),
    ] {
        assert!(
            Spec::read(&class, |k, _| Ok(if k == key {
                invalid.to_owned()
            } else {
                cells(k)
            }))
            .is_err(),
            "{key}: {invalid}"
        );
    }
}

fn flight_fixture(shift: u32, minimum: f32) -> Vec<(u32, Vec<u8>, bool)> {
    let mut m = Vec::new();
    let set = shift + 0x1000u32;
    let get = shift + 0x2000;
    let helper = shift + 0x3000;
    let trajectory = shift + 0x4000;
    let mut registration = Vec::new();
    for (i, (name, sig, handler)) in [
        ("SetUnitFlyHeight", "(Hunit;RR)V", set),
        ("GetUnitFlyHeight", "(Hunit;)R", get),
    ]
    .into_iter()
    .enumerate()
    {
        let n = shift + 0x8000 + i as u32 * 0x100;
        let sig_va = n + 0x40;
        m.push((n, format!("{name}\0").into_bytes(), false));
        m.push((sig_va, format!("{sig}\0").into_bytes(), false));
        for v in [sig_va, n, handler] {
            registration.push(0x68);
            registration.extend(v.to_le_bytes());
        }
        registration.extend([0xe8, 0, 0, 0, 0]);
    }
    m.push((shift + 0x7000, registration, true));
    let mut s = vec![0x90; 76];
    let mut g = vec![0x90; 33];
    let mut h = vec![0x90; 0x80];
    let mut t = vec![0x90; 0x278];
    fn put(b: &mut [u8], at: usize, bytes: &[u8]) {
        b[at..at + bytes.len()].copy_from_slice(bytes);
    }
    fn call(b: &mut [u8], va: u32, at: usize, target: u32) {
        b[at] = 0xe8;
        put(
            b,
            at + 1,
            &target.wrapping_sub(va + at as u32 + 5).to_le_bytes(),
        );
    }
    put(&mut s, 0, &[0x55, 0x8b, 0xec]);
    put(&mut g, 0, &[0x55, 0x8b, 0xec]);
    call(&mut s, set, 7, shift + 0x6000);
    call(&mut g, get, 6, shift + 0x6000);
    put(&mut s, 0xf, &[0x85, 0xc0, 0x74]);
    put(&mut s, 0x1b, &[0xf3, 0x0f, 0x10, 0x01, 0x0f, 0x2f, 0x05]);
    put(&mut s, 0x22, &(shift + 0x9000).to_le_bytes());
    put(&mut g, 0x13, &(shift + 0x9000).to_le_bytes());
    put(&mut s, 0x26, &[0x76, 0x10]);
    s[0x38] = 0x68;
    put(&mut s, 0x39, &(shift + 0x9100).to_le_bytes());
    call(&mut s, set, 0x2f, helper);
    call(&mut s, set, 0x43, helper);
    put(&mut g, 0xe, &[0x85, 0xc0, 0x75]);
    put(&mut g, 0x19, &[0x8b, 0x80, 0x14, 0x04, 0, 0, 0x5d, 0xc3]);
    put(&mut h, 0x41, &[0x8b, 0x87, 0x10, 0x04, 0, 0]);
    put(&mut h, 0x4d, &[0x6a, 0x01, 0x56, 0xff, 0x10]);
    call(&mut h, helper, 0x75, trajectory);
    put(
        &mut t,
        0x1a1,
        &[0xf3, 0x0f, 0x10, 0x45, 0xec, 0x0f, 0x2e, 0x05],
    );
    put(&mut t, 0x1a9, &(shift + 0x9000).to_le_bytes());
    put(&mut t, 0x1ad, &[0x9f, 0xf6, 0xc4, 0x44, 0x0f, 0x8b]);
    for at in [0x1c7, 0x1d4, 0x1eb] {
        call(&mut t, trajectory, at, shift + 0x6000);
    }
    put(&mut t, 0x24e, &[0x8d, 0x45, 0x0c, 0x8b, 0xcf, 0x50, 0xe8]);
    m.extend([
        (set, s, true),
        (get, g, true),
        (helper, h, true),
        (trajectory, t, true),
        (shift + 0x9100, minimum.to_le_bytes().to_vec(), false),
    ]);
    m
}
fn flight_decode(memory: &[(u32, Vec<u8>, bool)]) -> Result<FlightRules, String> {
    FlightRules::decode(
        &memory
            .iter()
            .filter(|r| r.2)
            .map(|r| (r.0, r.1.as_slice()))
            .collect::<Vec<_>>(),
        &memory
            .iter()
            .filter(|r| !r.2)
            .map(|r| (r.0, r.1.as_slice()))
            .collect::<Vec<_>>(),
        &mut |va, n, code| {
            memory
                .iter()
                .find(|r| va >= r.0 && va - r.0 < r.1.len() as u32 && (!code || r.2))
                .and_then(|r| r.1.get((va - r.0) as usize..(va - r.0) as usize + n))
                .map(<[u8]>::to_vec)
                .ok_or_else(|| "fixture flight mapping".into())
        },
    )
}
#[test]
fn relocated_height_operand_and_changed_helper_refusal() {
    for shift in [0, 0x120000] {
        let memory = flight_fixture(shift, 0.03125);
        assert_eq!(flight_decode(&memory).unwrap().minimum_target, 0.03125);
    }
    for minimum in [f32::NAN, -1., 0.] {
        assert!(flight_decode(&flight_fixture(0, minimum)).is_err());
    }
    let mut memory = flight_fixture(0, 0.03125);
    memory.iter_mut().find(|r| r.0 == 0x3000).unwrap().1[0x4d] = 0x90;
    assert!(flight_decode(&memory).is_err());
    let mut memory = flight_fixture(0, 0.03125);
    let duplicate = memory[4].clone();
    memory.push((duplicate.0 + 0x10000, duplicate.1, duplicate.2));
    assert!(flight_decode(&memory).is_err());
}

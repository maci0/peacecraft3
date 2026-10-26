use super::*;
use std::collections::BTreeMap;

fn identity() -> AssetIdentity {
    AssetIdentity {
        edition: "synthetic".into(),
        version: "relocated".into(),
        source: "fixture".into(),
    }
}
fn fixture() -> BTreeMap<u32, Vec<u8>> {
    let mut m = BTreeMap::new();
    let mov = |value: u32| {
        let mut b = vec![0xb8];
        b.extend(value.to_le_bytes());
        b.push(0xc3);
        b
    };
    let alias = u32::from_be_bytes(*b"Zmir");
    let buff = u32::from_be_bytes(*b"Zbuf");
    let rel = |b: &mut Vec<u8>, base: u32, at: usize, target: u32| {
        b[at] = 0xe8;
        b[at + 1..at + 5].copy_from_slice(&target.wrapping_sub(base + at as u32 + 5).to_le_bytes());
    };
    let mut registration = vec![0; 25];
    registration[0] = 0x68;
    registration[10..12].copy_from_slice(&[0x50, 0x68]);
    registration[12..16].copy_from_slice(&alias.to_le_bytes());
    registration[21..].copy_from_slice(&[0x83, 0xc4, 0x0c, 0xc3]);
    rel(&mut registration, 0xc2e4b0, 5, 0xb4b180);
    rel(&mut registration, 0xc2e4b0, 16, 0x7f6360);
    m.insert(0xc2e4b0, registration);
    m.insert(0xc2c930, mov(alias));
    m.insert(0xc2c920, mov(0x1800));
    m.insert(0x1800, b"CAbilityMirrorImage\0".to_vec());
    let mut ctor = vec![0xc7, 0x06];
    ctor.extend(0x2000u32.to_le_bytes());
    m.insert(0xc2d39e, ctor);
    let mut table = vec![0; 0x3cc];
    for (slot, role) in [
        (0x0c, 0xc2cf00u32),
        (0x1c, 0xc2c930),
        (0x58, 0xc2c920),
        (0x310, 0xc2cc60),
        (0x3c8, 0xc2ccc0),
    ] {
        table[slot..slot + 4].copy_from_slice(&role.to_le_bytes());
    }
    m.insert(0x2000, table);
    let mut handler = vec![0; 23];
    handler[..3].copy_from_slice(&[0x81, 0x78, 8]);
    handler[3..7].copy_from_slice(&91u32.to_le_bytes());
    handler[7..9].copy_from_slice(&[0x74, 9]);
    rel(&mut handler, 0xc2cf06, 18, 0xc2e5a0);
    m.insert(0xc2cf06, handler);
    let mut scheduled = vec![0; 30];
    rel(&mut scheduled, 0xc2cebc, 0, 0x46ef80);
    scheduled[10] = 0x68;
    scheduled[11..15].copy_from_slice(&91u32.to_le_bytes());
    rel(&mut scheduled, 0xc2cebc, 25, 0x7fbe90);
    m.insert(0xc2cebc, scheduled);
    m.insert(0xc2cc60, mov(789));
    m.insert(0xc2c0c0, mov(buff));
    let mut b = vec![0; 25];
    b[11] = 0x68;
    b[12..16].copy_from_slice(&buff.to_le_bytes());
    m.insert(0xc2e4f0, b);
    let mut registration = vec![0; 20];
    for at in [0, 5, 10] {
        registration[at] = 0x68;
    }
    registration[1..5].copy_from_slice(&0x5100u32.to_le_bytes());
    registration[6..10].copy_from_slice(&0x5200u32.to_le_bytes());
    registration[11..15].copy_from_slice(&0x5300u32.to_le_bytes());
    rel(&mut registration, 0x49dc85, 15, 0x8ccc80);
    m.insert(0x49dc85, registration);
    m.insert(0x5100, b"(Hunit;)B\0".to_vec());
    m.insert(0x5200, b"IsUnitIllusion\0".to_vec());
    let mut query = vec![0; 34];
    rel(&mut query, 0x5300, 6, 0x483940);
    query[20..22].copy_from_slice(&[0x8b, 0x80]);
    query[22..26].copy_from_slice(&0x288u32.to_le_bytes());
    query[26..].copy_from_slice(&[0xc1, 0xe8, 11, 0x83, 0xe0, 1, 0x5d, 0xc3]);
    m.insert(0x5300, query);
    let mut creation = vec![0; 20];
    creation[0] = 0x68;
    creation[1..5].copy_from_slice(&0x200u32.to_le_bytes());
    rel(&mut creation, 0xc2dc67, 15, 0x687ca0);
    m.insert(0xc2dc67, creation);
    let mut mark = vec![
        0xf7, 0xc3, 0, 0, 0, 0, 0x74, 10, 0x81, 0x8e, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    mark[2..6].copy_from_slice(&0x200u32.to_le_bytes());
    mark[10..14].copy_from_slice(&0x288u32.to_le_bytes());
    mark[14..18].copy_from_slice(&(1u32 << 11).to_le_bytes());
    m.insert(0x687eb9, mark);
    let mut random = vec![0; 31];
    random[0] = 0x68;
    random[1..5].copy_from_slice(&0x3000u32.to_le_bytes());
    random[5..14].copy_from_slice(&[0x6a, 0, 0x41, 0x51, 0x6a, 1, 0x6a, 2, 0x50]);
    rel(&mut random, 0xc2e673, 14, 0x69c5a0);
    rel(&mut random, 0xc2e673, 20, 0x9a07b0);
    m.insert(0xc2e673, random);
    let mut initializer = vec![
        0x6a, 1, 0x68, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x83, 0xc4, 8, 0xc3,
    ];
    initializer[3..7].copy_from_slice(&0x3000u32.to_le_bytes());
    rel(&mut initializer, 0x401460, 7, 0x9a1620);
    m.insert(0x401460, initializer);
    let mut count = vec![0; 19];
    count[0] = 0xb8;
    count[1..5].copy_from_slice(&7u32.to_le_bytes());
    count[5..7].copy_from_slice(&[0x3b, 0xf0]);
    count[16..].copy_from_slice(&[0x0f, 0x4f, 0xf0]);
    m.insert(0xc2e6e6, count);
    let mut circle = vec![0; 43];
    circle[0] = 0x68;
    circle[1..5].copy_from_slice(&0x4000u32.to_le_bytes());
    rel(&mut circle, 0xc2e266, 25, 0x99ffe0);
    rel(&mut circle, 0xc2e266, 38, 0x9a02d0);
    m.insert(0xc2e266, circle);
    m.insert(0x4000, 5.25f32.to_bits().to_le_bytes().to_vec());
    m
}
fn decode(m: &BTreeMap<u32, Vec<u8>>, budget: usize) -> Result<Rules, String> {
    Rules::decode(identity(), [8; 32], budget, |va, len, _| {
        m.get(&va)
            .filter(|b| b.len() == len)
            .cloned()
            .ok_or_else(|| "missing fixture window".into())
    })
}
#[test]
fn relocated_operands_and_nonstock_values_are_derived() {
    let r = decode(&fixture(), 4096).unwrap();
    assert_eq!(r.ability, *b"Zmir");
    assert_eq!(r.buff, *b"Zbuf");
    assert_eq!(r.order, 789);
    assert_eq!(r.image_limit, 7);
    assert_eq!(r.random_channel, 2);
    assert_eq!(r.circle_radians, 5.25);
}
#[test]
fn malformed_relationships_and_budget_refuse() {
    let m = fixture();
    assert!(decode(&m, 20).is_err());
    for (va, offset) in [
        (0xc2e4b0, 12),
        (0xc2d39e, 0),
        (0xc2e673, 10),
        (0xc2e6e6, 16),
        (0xc2e266, 25),
        (0xc2cf06, 3),
        (0xc2cebc, 11),
        (0x687eb9, 14),
    ] {
        let mut bad = m.clone();
        bad.get_mut(&va).unwrap()[offset] ^= 1;
        assert!(decode(&bad, 4096).is_err(), "{va:x}");
    }
    let mut bad = m;
    bad.get_mut(&0x401460).unwrap()[1] = 2;
    assert!(decode(&bad, 4096).is_err());
}
#[test]
#[ignore = "requires locally supplied selected Legacy executable"]
fn selected_source_mirror_image_rules() {
    let path = std::env::var("PCX_MIRROR_CLIENT").expect("PCX_MIRROR_CLIENT");
    let image = std::fs::read(path).unwrap();
    let r = Rules::read(&image, identity()).unwrap();
    assert_ne!(r.ability, r.buff);
    assert!(r.image_limit > 0);
    assert!(r.circle_radians.is_finite());
}

#[test]
fn optional_admission_preserves_unrelated_hero_rules_and_checks_random_source() {
    let hero = crate::hero_skills::tests::rules();
    assert!(hero.mirror_image.is_err());
    let preserved = hero
        .clone()
        .with_mirror_image(Err("unsupported optional source".into()));
    assert_eq!(preserved.initial_level, hero.initial_level);
    assert_eq!(preserved.slots, hero.slots);
    assert!(
        preserved
            .mirror_image
            .unwrap_err()
            .contains("unsupported optional source")
    );
    let rules = decode(&fixture(), 4096).unwrap();
    assert!(
        hero.clone()
            .with_mirror_image(Ok(rules.clone()))
            .mirror_image
            .is_err()
    );
    let random = crate::script_random::profile::fixture();
    assert!(rules.clone().with_random(&random).is_err());
    let mut matched = rules;
    matched.image_sha256 = *random.image_sha256();
    assert!(matched.clone().with_random(&random).is_ok());
    matched.random_channel = random.secondary_count;
    assert!(matched.with_random(&random).is_err());
}

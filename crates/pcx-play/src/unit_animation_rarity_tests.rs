use super::*;
#[test]
#[ignore = "requires PCX_RARITY_CLIENT selected original executable"]
fn selected_rarity_reader() {
    let p = std::env::var("PCX_RARITY_CLIENT").unwrap();
    let b = std::fs::read(p).unwrap();
    let rules = Rules::read(
        &b,
        AssetIdentity {
            source: "selected original".into(),
            edition: "Legacy classic".into(),
            version: "1.29.2.9231".into(),
        },
    )
    .unwrap();
    assert!(!rules.tags.is_empty());
}

#[test]
#[ignore = "requires PCX_RARITY_CLIENT, PCX_RARITY_MODEL and PCX_RARITY_COMMON local original inputs"]
fn selected_model_rarity_relationships() {
    let image = std::fs::read(std::env::var("PCX_RARITY_CLIENT").unwrap()).unwrap();
    let rules = Rules::read(
        &image,
        AssetIdentity {
            source: "selected original executable".into(),
            edition: "Legacy classic".into(),
            version: "1.29.2.9231".into(),
        },
    )
    .unwrap();
    let bytes = std::fs::read(std::env::var("PCX_RARITY_MODEL").unwrap()).unwrap();
    let model = crate::unit_model::Model::read(
        AssetIdentity {
            source: "War3.mpq; War3Local member absent".into(),
            edition: "Legacy classic".into(),
            version: "1.29.2.9231".into(),
        },
        "Units/Orc/HeroBladeMaster/HeroBladeMaster.mdx".into(),
        bytes.into(),
        16 << 20,
    )
    .unwrap();
    let common = std::fs::read_to_string(std::env::var("PCX_RARITY_COMMON").unwrap()).unwrap();
    let ordinal = |name: &str| {
        let line = common
            .lines()
            .find(|line| line.contains(name) && line.contains("ConvertRarityControl("))
            .unwrap();
        line.split("ConvertRarityControl(")
            .nth(1)
            .unwrap()
            .split(')')
            .next()
            .unwrap()
            .trim()
            .parse::<i32>()
            .unwrap()
    };
    let frequent = rules
        .candidates(&model, "stand", &[], ordinal("RARITY_FREQUENT"))
        .unwrap();
    let rare = rules
        .candidates(&model, "stand", &[], ordinal("RARITY_RARE"))
        .unwrap();
    assert!(!frequent.is_empty());
    let all = model
        .sequences
        .iter()
        .filter(|s| {
            s.name.eq_ignore_ascii_case("stand")
                || s.name.to_ascii_lowercase().starts_with("stand -")
                || s.name.eq_ignore_ascii_case("stand cinematic")
        })
        .map(pcx_mdl::Sequence::rarity)
        .collect::<Vec<_>>();
    let max = all.iter().copied().reduce(f32::max).unwrap();
    let min = all.iter().copied().fold(rules.rare_initial, f32::min);
    assert!(frequent.iter().all(|i| model.sequences[*i].rarity() == max));
    assert_eq!(rare.is_empty(), !all.contains(&min));
    assert!(rare.iter().all(|i| model.sequences[*i].rarity() == min));
}

fn origin() -> AssetIdentity {
    AssetIdentity {
        source: "self-authored".into(),
        edition: "nonstock".into(),
        version: "fixture".into(),
    }
}
fn fixture(delta: u32) -> BTreeMap<u32, Vec<u8>> {
    let mut m = BTreeMap::new();
    fn rel(b: &mut [u8], va: u32, i: usize, target: u32) {
        b[i] = 0xe8;
        b[i + 1..i + 5].copy_from_slice(&target.wrapping_sub(va + i as u32 + 5).to_le_bytes());
    }
    let mut reg = |va: u32, name: &str, handler: u32, ptr: u32| {
        let mut b = vec![0; 20];
        for i in [0, 5, 10] {
            b[i] = 0x68;
        }
        b[6..10].copy_from_slice(&(ptr + delta).to_le_bytes());
        b[11..15].copy_from_slice(&handler.to_le_bytes());
        rel(&mut b, va, 15, 0x1500);
        m.insert(va, b);
        m.insert(ptr + delta, format!("{name}\0").into_bytes());
    };
    reg(0x49d632, "SetUnitAnimationWithRarity", 0x4ab110, 0x2000);
    reg(0x49b868, "ConvertRarityControl", 0x48ad10, 0x2100);
    m.insert(0x48ad10, vec![0x55, 0x8b, 0xec, 0x8b, 0x45, 8, 0x5d, 0xc3]);
    let mut b = vec![0x55, 0x8b, 0xec, 0x83, 0x6d, 0x10, 7, 0xb8];
    b.extend(0x40u32.to_le_bytes());
    b.extend([0x75, 5, 0xb8]);
    b.extend(0x80u32.to_le_bytes());
    b.extend([0x89, 0x45, 0x10, 0x5d, 0xe9]);
    b.extend(0x4aafc0u32.wrapping_sub(0x4ab110 + 28).to_le_bytes());
    m.insert(0x4ab110, b);
    let mut b = vec![
        0xff, 0x75, 0x10, 0x57, 0x53, 0xff, 0x75, 0xe4, 0xe8, 0, 0, 0, 0,
    ];
    b.resize(21, 0);
    rel(&mut b, 0x4ab0cb, 8, 0x966c10);
    m.insert(0x4ab0cb, b);
    let mut b = vec![
        0x8b, 0x45, 0x14, 0xa8, 0xc0, 0x74, 0, 0x8b, 0x4b, 0x30, 0x83, 0xe0, 0x40, 0x56, 0x50,
        0xe8, 0, 0, 0, 0,
    ];
    rel(&mut b, 0x966da4, 15, 0x982ff0);
    m.insert(0x966da4, b);
    let mut b = vec![0; 0x72];
    b[..16].copy_from_slice(&[
        0x8b, 0x5d, 8, 0x85, 0xdb, 0x74, 5, 0x0f, 0x57, 0xc9, 0xeb, 8, 0xf3, 0x0f, 0x10, 0x0d,
    ]);
    b[16..20].copy_from_slice(&(0x2200 + delta).to_le_bytes());
    b[0x58..0x69].copy_from_slice(&[
        0x85, 0xdb, 0x74, 5, 0x0f, 0x2f, 0xc8, 0xeb, 3, 0x0f, 0x2f, 0xc1, 0x77, 3, 0x0f, 0x28, 0xc8,
    ]);
    m.insert(0x983013, b);
    m.insert(0x2200 + delta, 0.75f32.to_le_bytes().to_vec());
    m.insert(
        0x8f9918,
        vec![0xe8, 0, 0, 0, 0, 0x8b, 0xcb, 0xd9, 0x5e, 0x0c],
    );
    let mut b = vec![0; 18];
    b[..6].copy_from_slice(&[0x8b, 0x41, 0x10, 0x89, 0x42, 0x10]);
    m.insert(0xcc2cd2, b);
    m.insert(
        0x982bea,
        vec![0xf3, 0x0f, 0x10, 0x45, 0xd0, 0xf3, 0x0f, 0x11, 0x47, 0x18],
    );
    let mut b = vec![0x68];
    b.extend(0xabcdu32.to_le_bytes());
    b.push(0xb9);
    b.extend((0x2300 + delta).to_le_bytes());
    b.extend([0xe8, 0, 0, 0, 0, 0xc3]);
    rel(&mut b, 0x4140d0, 10, 0x9892a0);
    m.insert(0x4140d0, b);
    let mut b = vec![0; 33];
    b[0] = 0x68;
    b[1..5].copy_from_slice(&(0x2300 + delta).to_le_bytes());
    b[5] = 0xb9;
    rel(&mut b, 0x9839ea, 28, 0x99ff10);
    m.insert(0x9839ea, b);
    let mut b = vec![0; 0x50];
    rel(&mut b, 0x99ff10, 24, 0x480dd0);
    b[0x20] = 0x25;
    b[0x21..0x25].copy_from_slice(&0x3fffffu32.to_le_bytes());
    b[0x25] = 0x0d;
    b[0x26..0x2a].copy_from_slice(&2f32.to_bits().to_le_bytes());
    b[0x32..0x36].copy_from_slice(&[0xf3, 0x0f, 0x5c, 5]);
    b[0x36..0x3a].copy_from_slice(&(0x2400 + delta).to_le_bytes());
    m.insert(0x99ff10, b);
    m.insert(0x2400 + delta, 2f32.to_le_bytes().to_vec());
    let mut b = vec![0x81, 0xf9];
    b.extend(31u32.to_le_bytes());
    m.insert(0xcc20cf, b);
    let mut b = vec![0x52, 0x50, 0xe8, 0, 0, 0, 0];
    rel(&mut b, 0x964021, 2, 0x983940);
    m.insert(0x964021, b);
    let mut b = vec![0; 19];
    b[..9].copy_from_slice(&[0x0f, 0x2e, 0x45, 0x0c, 0x9f, 0xf6, 0xc4, 0x44, 0x7a]);
    m.insert(0x9830d0, b);
    m.insert(0x4ac670, vec![0x6a, 16]);
    let mut b = vec![0x68];
    b.extend(256u32.to_le_bytes());
    m.insert(0x9774fd, b);
    m.insert(0x4ac682, vec![0x6a, 13, 0, 0, 0, 0, 0, 0]);
    for (va, ptr, text) in [
        (0x5d5dd8, 0x2500, "motionTags"),
        (0x4ac66b, 0x2600, " "),
        (0x6dafe0, 0x2700, " ,;"),
    ] {
        let mut b = vec![0x68];
        b.extend((ptr + delta).to_le_bytes());
        m.insert(va, b);
        m.insert(ptr + delta, format!("{text}\0").into_bytes());
    }
    let mut b = Vec::new();
    for (n, (name, value)) in [("still", 13i32), ("wave", 19), ("cinema", i32::MIN + 3)]
        .into_iter()
        .enumerate()
    {
        b.push(0x68);
        b.extend(value.to_le_bytes());
        b.push(0x68);
        let ptr = 0x3000 + delta + n as u32 * 32;
        b.extend(ptr.to_le_bytes());
        m.insert(ptr, format!("{name}\0").into_bytes());
        b.extend([0xff, 0x35]);
        b.extend(0x4000u32.to_le_bytes());
        let i = b.len();
        b.resize(i + 5, 0);
        rel(&mut b, 0x6db0e0, i, 0x977540);
    }
    b.push(0xc3);
    b.resize(0x4b0, 0xcc);
    m.insert(0x6db0e0, b);
    m
}
fn read_fixture(m: &BTreeMap<u32, Vec<u8>>, budget: usize) -> Result<Rules, String> {
    Rules::decode(
        [3; 32],
        origin(),
        |va, n| {
            m.iter()
                .find_map(|(base, b)| {
                    va.checked_sub(*base)
                        .and_then(|i| b.get(i as usize..i as usize + n))
                        .map(<[u8]>::to_vec)
                })
                .ok_or_else(|| format!("fixture range {va:x}"))
        },
        budget,
    )
}
pub(crate) fn rules() -> Rules {
    read_fixture(&fixture(0), 64 << 10).unwrap()
}
pub(crate) fn model(rows: &[(&str, f32)]) -> crate::unit_model::Model {
    let mut m = (*crate::unit_model::tests::model()).clone();
    m.sequences = rows
        .iter()
        .enumerate()
        .map(|(i, (name, weight))| pcx_mdl::Sequence {
            name: (*name).into(),
            interval_start: i as u32 * 100,
            interval_end: i as u32 * 100 + 90,
            move_speed: 0.,
            non_looping: false,
            trailing: [weight.to_bits(), 0, 0, 0, 0, 0, 0],
        })
        .collect();
    m
}
#[test]
fn relocated_nonstock_source_operands_and_malformed_budget_are_distinct() {
    for delta in [0, 0x60000] {
        let r = read_fixture(&fixture(delta), 64 << 10).unwrap();
        assert_eq!((r.rare, r.seed, r.fallback), (7, 0xabcd, 13));
        assert_eq!(r.rare_initial, 0.75);
        assert_eq!(r.props, "motionTags");
        assert_eq!(r.tags["cinema"], i32::MIN + 3);
    }
    assert!(
        read_fixture(&fixture(0), 20)
            .unwrap_err()
            .contains("budget")
    );
    let mut m = fixture(0);
    m.get_mut(&0x966da4).unwrap()[12] ^= 1;
    assert!(read_fixture(&m, 64 << 10).is_err());
    m = fixture(0);
    m.insert(0x2200, f32::NAN.to_le_bytes().to_vec());
    assert!(read_fixture(&m, 64 << 10).is_err());
}
#[test]
fn source_extrema_tags_signed_category_and_swap_filter_preserve_authored_relationships() {
    let r = rules();
    let m = model(&[
        ("still 2", 0.2),
        ("still wave", 0.9),
        ("still cinema", 0.6),
        ("still 4", 0.4),
    ]);
    assert_eq!(r.candidates(&m, "still", &[], 0).unwrap(), [2]);
    assert_eq!(r.candidates(&m, "still", &[], 7).unwrap(), [0]);
    assert_eq!(r.candidates(&m, "still", &["wave".into()], 0).unwrap(), [1]);
    let rows = vec![("still", 0.1); 32];
    assert!(
        r.candidates(&model(&rows), "still", &[], 0)
            .unwrap_err()
            .contains("budget")
    );
    let m = model(&[("still", 0.8), ("still 2", 0.9)]);
    assert!(r.candidates(&m, "still", &[], 7).unwrap().is_empty());
    for n in [f32::NAN, f32::INFINITY, -0.1] {
        assert!(
            r.candidates(&model(&[("still", n)]), "still", &[], 0)
                .is_err()
        );
    }
}
#[test]
fn independent_weighted_draws_are_repeatable_include_zero_sum_and_refuse_transactionally() {
    let policy = Arc::new(crate::script_random::profile::fixture());
    let mut r = rules();
    r.image_sha256 = *policy.image_sha256();
    let mut a = State::new(r, policy).unwrap();
    let mut b = a.clone();
    for weights in [&[0., 0.][..], &[0.25, 0.75, 0.5][..], &[2.][..]] {
        assert_eq!(a.choose(weights).unwrap(), b.choose(weights).unwrap());
    }
    assert_eq!(a.choose(&[0., 0., 0.]).unwrap(), 2);
    b.choose(&[0., 0., 0.]).unwrap();
    for invalid in [&[][..], &[f32::NAN][..], &[f32::MAX, f32::MAX][..]] {
        assert!(a.choose(invalid).is_err());
    }
    assert_eq!(a.choose(&[1., 2.]).unwrap(), b.choose(&[1., 2.]).unwrap());
}

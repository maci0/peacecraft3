use super::*;
use std::collections::BTreeMap;

fn numeric() -> NumericPolicy {
    let (image, binding, budget) = crate::script_numeric::tests::fixture();
    NumericPolicy::read(&image, binding, budget, 64).unwrap()
}
fn fixture(numeric: &NumericPolicy, scale: i8, weight: i32) -> BTreeMap<u32, Vec<u8>> {
    let first = 0x280_u32;
    let full = 0x220_u32;
    let current = 0x228_u32;
    let baseline = 0x230_u32;
    let mut data = BTreeMap::new();
    let mut put = |address, length, values: &[(usize, &[u8])]| {
        let mut bytes = vec![0; length];
        for &(at, value) in values {
            bytes[at..at + value.len()].copy_from_slice(value);
        }
        data.insert(address, bytes);
    };
    put(
        0xabe270,
        27,
        &[
            (0, &[0x8b, 0x81]),
            (2, &(first + 4).to_le_bytes()),
            (6, &[0x81, 0xc1]),
            (8, &first.to_le_bytes()),
        ],
    );
    put(
        0xabf930,
        36,
        &[
            (2, &(first + 4).to_le_bytes()),
            (8, &first.to_le_bytes()),
            (28, &[0x83, 0x88]),
            (30, &full.to_le_bytes()),
            (34, &[4, 0xc3]),
        ],
    );
    put(
        0xabb238,
        18,
        &[(0, &[0x83, 0xa3]), (2, &full.to_le_bytes()), (6, &[!4])],
    );
    put(
        0xab3fc0,
        15,
        &[
            (5, &[0x8b, 0x80]),
            (7, &full.to_le_bytes()),
            (11, &[0x83, 0xe0, 4, 0xc3]),
        ],
    );
    put(
        0xac9b30,
        22,
        &[
            (0, &[0x01, 0xb3]),
            (2, &current.to_le_bytes()),
            (6, &[0x85, 0xf6, 0x7e, 0x0c, 0x8b, 0x83]),
            (12, &current.to_le_bytes()),
            (16, &[0x89, 0x83]),
            (18, &baseline.to_le_bytes()),
        ],
    );
    put(
        0xab3e1f,
        7,
        &[(0, &[0x8b, 0x80]), (2, &current.to_le_bytes())],
    );
    put(
        0xad5a4c,
        20,
        &[
            (0, &[0xc7, 0x83]),
            (2, &baseline.to_le_bytes()),
            (10, &[0xc7, 0x83]),
            (12, &current.to_le_bytes()),
        ],
    );
    put(
        0xad2831,
        42,
        &[
            (0, &[0x8b, 0x86]),
            (2, &baseline.to_le_bytes()),
            (6, &[0x2b, 0x86]),
            (8, &current.to_le_bytes()),
            (12, &[0x69, 0xc0]),
            (14, &weight.to_le_bytes()),
        ],
    );
    let (image, binding, _) = crate::script_numeric::tests::fixture();
    assert_eq!(numeric.image_sha256(), &sha256(&image));
    let initializer = |address: u32, destination: u32, value: i8| {
        let mut bytes = vec![0x6a, value as u8, 0x68];
        bytes.extend(destination.to_le_bytes());
        bytes.push(0xe8);
        bytes.extend(
            binding
                .math_i32_to_real
                .va
                .wrapping_sub(address + 12)
                .to_le_bytes(),
        );
        bytes.extend([0x83, 0xc4, 8, 0xc3]);
        bytes
    };
    // Nonstock scalar, counter, mask, alias and priority inputs.
    let one = initializer(0x401460, 0x12300, 1);
    let percent = initializer(0x401350, 0x12400, scale);
    put(
        0xad281a,
        23,
        &[(0, &[0xa1]), (1, &0x12300_u32.to_le_bytes())],
    );
    put(
        0xad28d0,
        18,
        &[
            (0, &[0x8b, 0x4f, 0x34, 0xc1, 0xe9, 0x18, 0x83, 0xe9]),
            (8, b"X"),
            (9, &[0x83, 0xf9]),
            (11, &[2]),
        ],
    );
    put(
        0xad2a6f,
        112,
        &[
            (4, &[0x68]),
            (5, &0x12400_u32.to_le_bytes()),
            (42, &[0x68]),
            (43, &0x12400_u32.to_le_bytes()),
            (80, &[0x68]),
            (81, &0x12400_u32.to_le_bytes()),
        ],
    );
    put(
        0xad2987,
        68,
        &[
            (8, &[0xff, 0x90]),
            (10, &0x12c_u32.to_le_bytes()),
            (44, &[0xff, 0x90]),
            (46, &0x130_u32.to_le_bytes()),
        ],
    );
    put(
        0xad2ae1,
        30,
        &[
            (0, &[0x83, 0x7d, 0x08, 0, 0x74, 0x4c]),
            (18, &[0x0f, 0x47, 0x45, 0xd0]),
        ],
    );
    put(
        0xad0857,
        112,
        &[
            (4, &[0x8b, 0xbf]),
            (10, &[0xff, 0x90, 0xec, 0, 0, 0, 0x3b, 0xc7]),
            (32, &u32::from_be_bytes(*b"q123").to_le_bytes()),
            (78, &u32::from_be_bytes(*b"q123").to_le_bytes()),
            (47, &[0xf7, 0xd8, 0x1b, 0xc0, 0x83, 0xc0]),
            (53, &[3]),
            (59, &[0x33, 0xc0]),
            (93, &[0xf7, 0xd8, 0x1b, 0xc0, 0x83, 0xc0]),
            (105, &[0xc7, 0x45, 0xec]),
            (99, &[9]),
            (108, &7_u32.to_le_bytes()),
        ],
    );
    put(
        0xac40f0,
        77,
        &[
            (5, &(first + 4).to_le_bytes()),
            (11, &first.to_le_bytes()),
            (37, &[0xf6, 0x80]),
            (39, &full.to_le_bytes()),
            (43, &[4]),
            (62, &[0xb8, 1, 0, 0, 0, 0x5d, 0xc2, 0x10, 0]),
        ],
    );
    data.insert(0x401460, one);
    data.insert(0x401350, percent);
    for offset in [42, 88] {
        let base = 0xad0857_u32;
        let bytes = data.get_mut(&base).unwrap();
        bytes[offset] = 0xe8;
        bytes[offset + 1..offset + 5].copy_from_slice(
            &0x696b50_u32
                .wrapping_sub(base + offset as u32 + 5)
                .to_le_bytes(),
        );
    }
    let mut consumer = vec![0; 20];
    consumer[..20].copy_from_slice(&[
        0x8b, 0x06, 0x8b, 0xce, 0x6a, 4, 0x6a, 8, 0xff, 0xb7, 0x70, 1, 0, 0, 0xff, 0x90, 0xfc, 0,
        0, 0,
    ]);
    data.insert(0xad07ce, consumer);
    data.insert(0xe9c600, 0x13000_u32.to_le_bytes().to_vec());
    let mut wrapper = vec![0; 83];
    wrapper[63..78].copy_from_slice(&[
        0x0b, 0x45, 0x0c, 0x8b, 0xce, 0x6a, 0, 0xff, 0x75, 0x10, 0x50, 0x57, 0xff, 0x75, 8,
    ]);
    wrapper[78] = 0xe8;
    wrapper[79..83].copy_from_slice(&0x14000_u32.wrapping_sub(0x13000 + 83).to_le_bytes());
    data.insert(0x13000, wrapper);
    let mut query = vec![0; 228];
    query[0x93] = 0xb8;
    query[0x94..0x98].copy_from_slice(&8_u32.to_le_bytes());
    query[0x98..0x9c].copy_from_slice(&[0x84, 0xd8, 0x75, 0x3f]);
    query[0x9c..0xa7].copy_from_slice(&[0x8d, 0x45, 0xf8, 0xc7, 0x45, 0xf8, 0, 0, 0, 0, 0x50]);
    query[0xb2..0xb8].copy_from_slice(&[0xff, 0x90, 0xb8, 0, 0, 0]);
    query[0xd6] = 0xe8;
    query[0xdb..0xe4].copy_from_slice(&[0x5b, 0x5f, 0x5e, 0x8b, 0xe5, 0x5d, 0xc2, 0x14, 0]);
    data.insert(0x14000, query);
    data
}
fn decode(
    data: &BTreeMap<u32, Vec<u8>>,
    numeric: &NumericPolicy,
    budget: usize,
) -> Result<Rules, String> {
    Rules::decode(
        *numeric.image_sha256(),
        numeric,
        budget,
        |address, length, _code| {
            data.get(&address)
                .and_then(|bytes| bytes.get(..length))
                .map(<[u8]>::to_vec)
                .ok_or("missing synthetic assault role".into())
        },
    )
}

#[test]
fn source_operands_and_nonstock_readiness_are_not_defaults() {
    let numeric = numeric();
    let rules = decode(&fixture(&numeric, 80, 37), &numeric, 4096).unwrap();
    assert_eq!(rules.full_mask, 4);
    assert_eq!(rules.casualty_weight, 37);
    assert_eq!(rules.percentage, 80.);
    assert_eq!(rules.gold_mine, *b"q123");
    assert_eq!(rules.owner_tiers, [[3, 2], [9, 8]]);
    assert_eq!(rules.ordinary_readiness(&numeric, 0, &[]).unwrap(), 80);
    assert_eq!(
        rules
            .ordinary_readiness(&numeric, 1, &[(*b"u001", 19., 19.)])
            .unwrap(),
        80
    );
    // The existing synthetic reciprocal table deliberately uses a nonstock constant mantissa.
    assert_eq!(
        rules
            .ordinary_readiness(&numeric, 1, &[(*b"u001", 3., 19.)])
            .unwrap(),
        20
    );
    assert_eq!(
        rules
            .ordinary_readiness(&numeric, 2, &[(*b"u001", 19., 19.)])
            .unwrap(),
        50
    );
    assert!(
        rules
            .ordinary_readiness(&numeric, 1, &[(*b"X001", 19., 19.)])
            .is_err()
    );
    assert!(
        rules
            .ordinary_readiness(&numeric, 0, &[(*b"u001", 19., 19.)])
            .is_err()
    );
    assert!(
        rules
            .ordinary_readiness(&numeric, 1, &[(*b"u001", f32::NAN, 19.)])
            .is_err()
    );
}

#[test]
fn broken_counter_mask_scalar_and_budget_relationships_refuse() {
    let numeric = numeric();
    let source = fixture(&numeric, 80, 37);
    assert!(decode(&source, &numeric, 20).is_err());
    for (address, offset) in [
        (0xac9b30, 12),
        (0xabb238, 6),
        (0xad2a6f, 6),
        (0xad0857, 78),
        (0xac40f0, 43),
    ] {
        let mut changed = source.clone();
        changed.get_mut(&address).unwrap()[offset] ^= 1;
        assert!(
            decode(&changed, &numeric, 4096).is_err(),
            "{address:x}+{offset}"
        );
    }
    for (address, bytes) in &source {
        let mut changed = source.clone();
        changed.get_mut(address).unwrap().truncate(bytes.len() - 1);
        assert!(
            decode(&changed, &numeric, 4096).is_err(),
            "short {address:x}"
        );
    }
}

#[test]
#[ignore = "requires PCX_WAR3_EXE from selected local install"]
fn selected_original_assault_operands() {
    use crate::{camera_margins::SourceWindow, script_numeric::Binding, scripts::AssetIdentity};
    let image = std::fs::read(std::env::var("PCX_WAR3_EXE").expect("PCX_WAR3_EXE")).unwrap();
    let budget = RuleInputBudget {
        image_bytes: 32 << 20,
        profile_bytes: 64 << 10,
        sections: 96,
    };
    let pe = Pe::parse(&image, budget).unwrap();
    let window = |va, len, code| SourceWindow {
        va,
        len,
        digest: sha256(&pe.read_from(va, code).unwrap()[..len as usize]),
    };
    let numeric = NumericPolicy::read(
        &image,
        Binding {
            origin: AssetIdentity {
                edition: "selected local".into(),
                version: "reviewed client".into(),
                source: "PCX_WAR3_EXE".into(),
            },
            image_sha256: sha256(&image),
            literal: window(0x8df660, 238, true),
            division: window(0x9a02d0, 57, true),
            reciprocal: window(0x9a1d00, 103, true),
            reciprocal_table: window(0xed6cf0, 4100, false),
            equality: window(0x8d8d10, 53, true),
            equality_constant: window(0x10db054, 4, false),
            inequality: window(0x8d7e32, 162, true),
            math_sub: window(0x9a00b0, 264, true),
            math_add: window(0x9a01c0, 262, true),
            math_mul: window(0x99ffe0, 195, true),
            math_lzcnt: window(0x9a0330, 29, true),
            math_i32_to_real: window(0x9a1620, 92, true),
        },
        budget,
        4096,
    )
    .unwrap();
    let rules = Rules::read(&image, &numeric, budget).unwrap();
    assert_eq!(rules.image_sha256, *numeric.image_sha256());
    assert!(rules.casualty_weight > 0);
    assert!(rules.percentage > 0.);
    assert!(
        rules
            .ordinary_readiness(&numeric, 1, &[(*b"u001", 100., 100.)])
            .unwrap()
            > 0
    );
}

#[test]
fn strategic_fog_bypass_follows_relocated_helpers_and_nonstock_mode_mask() {
    let numeric = numeric();
    let mut data = fixture(&numeric, 3, 17);
    let mut wrapper = data.remove(&0x13000).unwrap();
    let query = data.remove(&0x14000).unwrap();
    wrapper[79..83].copy_from_slice(&0x17000_u32.wrapping_sub(0x16000 + 83).to_le_bytes());
    data.insert(0x16000, wrapper);
    data.insert(0x17000, query);
    data.insert(0xe9c600, 0x16000_u32.to_le_bytes().to_vec());
    assert!(decode(&data, &numeric, 4096).is_ok());
    data.get_mut(&0xad07ce).unwrap()[7] = 2;
    assert!(
        decode(&data, &numeric, 4096)
            .unwrap_err()
            .contains("bypass mode")
    );
    data.get_mut(&0xad07ce).unwrap()[7] = 8;
    data.get_mut(&0x17000).unwrap()[0x9b] -= 1;
    assert!(decode(&data, &numeric, 4096).is_err());
}

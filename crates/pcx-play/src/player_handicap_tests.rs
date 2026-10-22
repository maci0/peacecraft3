use super::*;

pub(crate) fn fixture() -> (
    Vec<u8>,
    Binding,
    crate::script_numeric::Binding,
    RuleInputBudget,
) {
    let (mut image, mut numeric, mut budget) = crate::script_numeric::tests::fixture();
    let loc = |offset: u32| 0x301000 + offset;
    let mut setter = vec![0; 97];
    for (at, data) in [
        (0, &[0x55, 0x8b, 0xec, 0x51, 0xff, 0x75, 8, 0xe8][..]),
        (
            12,
            &[
                0x8b, 0xd0, 0x83, 0xc4, 4, 0x85, 0xd2, 0x74, 0x48, 0x8b, 0x4d, 12, 0xf3, 0x0f,
                0x10, 5,
            ],
        ),
        (
            32,
            &[
                0x8b, 9, 0x89, 0x4d, 0xfc, 0xf3, 0x0f, 0x10, 0x4d, 0xfc, 0x0f, 0x2f, 0xc1, 0x76, 7,
                0xb8,
            ],
        ),
        (52, &[0xeb, 0x12, 0x0f, 0x2f, 0x0d]),
        (61, &[0xb8]),
        (
            66,
            &[0x8d, 0x4d, 0xfc, 0x0f, 0x46, 0xc1, 0x8b, 0, 0x8d, 0x8a],
        ),
        (
            80,
            &[
                0x89, 0x45, 0xfc, 0x8d, 0x55, 0xfc, 0x8b, 1, 0x6a, 1, 0x52, 0xff, 0x10, 0x8b, 0xe5,
                0x5d, 0xc3,
            ],
        ),
    ] {
        setter[at..at + data.len()].copy_from_slice(data);
    }
    for (at, value) in [
        (8, loc(0x1800).wrapping_sub(loc(0x1200) + 12)),
        (28, loc(0x1500)),
        (48, loc(0x1500)),
        (57, loc(0x1580)),
        (62, loc(0x1580)),
        (76, 0x180),
    ] {
        setter[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    let mut getter = vec![0; 33];
    for (at, data) in [
        (0, &[0x55, 0x8b, 0xec, 0xff, 0x75, 8, 0xe8][..]),
        (11, &[0x83, 0xc4, 4, 0x85, 0xc0, 0x75, 7, 0xa1]),
        (23, &[0x5d, 0xc3, 0x8b, 0x80]),
        (31, &[0x5d, 0xc3]),
    ] {
        getter[at..at + data.len()].copy_from_slice(data);
    }
    for (at, value) in [
        (7, loc(0x1800).wrapping_sub(loc(0x1300) + 11)),
        (19, loc(0x1584)),
        (27, 0x184),
    ] {
        getter[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    let init = |value: u8, at: u32, dest: u32| {
        [
            vec![0x6a, value, 0x68],
            dest.to_le_bytes().to_vec(),
            vec![0xe8],
            numeric
                .math_i32_to_real
                .va
                .wrapping_sub(at + 12)
                .to_le_bytes()
                .to_vec(),
            vec![0x83, 0xc4, 8, 0xc3],
        ]
        .concat()
    };
    let mut put = |offset: usize, data: &[u8]| {
        image[0x200 + offset..0x200 + offset + data.len()].copy_from_slice(data);
        SourceWindow {
            va: loc(offset as u32),
            len: data.len() as u32,
            digest: sha256(data),
        }
    };
    let setter = put(0x1200, &setter);
    let getter = put(0x1300, &getter);
    let maximum_initializer = put(0x1400, &init(37, loc(0x1400), loc(0x1580)));
    let null_initializer = put(0x1420, &init(3, loc(0x1420), loc(0x1584)));
    put(0x1500, &0.375_f32.to_le_bytes());
    numeric.image_sha256 = sha256(&image);
    budget.profile_bytes = 8192;
    let binding = Binding {
        origin: numeric.origin.clone(),
        image_sha256: sha256(&image),
        setter,
        getter,
        maximum_initializer,
        null_initializer,
    };
    (image, binding, numeric, budget)
}
pub(crate) fn rules() -> (Rules, NumericPolicy) {
    let (image, binding, numeric, budget) = fixture();
    let numeric = NumericPolicy::read(&image, numeric, budget, 32).unwrap();
    (
        Rules::read(&image, binding, &numeric, budget).unwrap(),
        numeric,
    )
}
fn refresh(image: &[u8], binding: &mut Binding, numeric: &mut crate::script_numeric::Binding) {
    binding.image_sha256 = sha256(image);
    numeric.image_sha256 = sha256(image);
    for w in [
        &mut binding.setter,
        &mut binding.getter,
        &mut binding.maximum_initializer,
        &mut binding.null_initializer,
    ] {
        let at = 0x200 + (w.va - 0x301000) as usize;
        w.digest = sha256(&image[at..at + w.len as usize]);
    }
}
#[test]
fn source_operands_drive_clamping_null_result_and_nonfinite_comparisons() {
    let (r, _) = rules();
    for (input, expected) in [
        (-1.0, 0.375_f32),
        (0.0, 0.375),
        (1.5, 1.5),
        (40.0, 37.0),
        (f32::INFINITY, 37.0),
        (f32::NEG_INFINITY, 0.375),
    ] {
        assert_eq!(r.clamp(input).to_bits(), expected.to_bits());
    }
    for bits in [0x7fc0_0123, 0xffc0_0234, 0x7f80_0001] {
        assert_eq!(r.clamp(f32::from_bits(bits)).to_bits(), bits);
    }
    assert_eq!(r.null_result(), 3.0);
    let (mut image, mut binding, mut numeric, budget) = fixture();
    image[0x1700..0x1704].copy_from_slice(&2.25_f32.to_le_bytes());
    image[0x1601] = 17;
    image[0x1621] = 7;
    refresh(&image, &mut binding, &mut numeric);
    let numeric = NumericPolicy::read(&image, numeric, budget, 32).unwrap();
    let r = Rules::read(&image, binding, &numeric, budget).unwrap();
    assert_eq!(r.clamp(0.0), 2.25);
    assert_eq!(r.clamp(30.0), 17.0);
    assert_eq!(r.null_result(), 7.0);
}
#[test]
fn changed_shapes_calls_destinations_limits_and_sources_refuse() {
    for change in 0..10 {
        let (mut image, mut binding, mut numeric, mut budget) = fixture();
        match change {
            0 => image[0x1400 + 45] ^= 1,
            1 => image[0x1600 + 8] ^= 1,
            2 => image[0x1600 + 3] ^= 4,
            3 => image[0x1500 + 27] ^= 4,
            4 => image[0x1400 + 48] ^= 4,
            5 => image[0x1700..0x1704].copy_from_slice(&f32::NAN.to_le_bytes()),
            6 => image[0x1601] = 0,
            7 => binding.origin.source.clear(),
            8 => budget.profile_bytes = 1,
            9 => budget.sections = 0,
            _ => unreachable!(),
        }
        refresh(&image, &mut binding, &mut numeric);
        let numeric = NumericPolicy::read(
            &image,
            numeric,
            RuleInputBudget {
                sections: 1,
                ..budget_with_profile(budget)
            },
            32,
        )
        .unwrap();
        assert!(
            Rules::read(&image, binding, &numeric, budget).is_err(),
            "mutant {change}"
        );
    }
}
fn budget_with_profile(budget: RuleInputBudget) -> RuleInputBudget {
    RuleInputBudget {
        profile_bytes: 8192,
        ..budget
    }
}
#[test]
fn mismatched_fingerprint_window_digest_and_numeric_model_refuse() {
    let (image, binding, numeric, budget) = fixture();
    let n = NumericPolicy::read(&image, numeric, budget, 32).unwrap();
    let mut bad = binding.clone();
    bad.image_sha256[0] ^= 1;
    assert!(Rules::read(&image, bad, &n, budget).is_err());
    let mut bad = binding.clone();
    bad.getter.digest[0] ^= 1;
    assert!(Rules::read(&image, bad, &n, budget).is_err());
    let (mut other, mut other_binding, mut other_numeric, budget) = fixture();
    other[0x1700..0x1704].copy_from_slice(&1.25_f32.to_le_bytes());
    refresh(&other, &mut other_binding, &mut other_numeric);
    let other_numeric = NumericPolicy::read(&other, other_numeric, budget, 32).unwrap();
    assert!(Rules::read(&image, binding, &other_numeric, budget).is_err());
}

#[test]
fn wide_and_signed_integer_initializers_follow_the_selected_conversion() {
    let (mut image, mut binding, mut numeric, budget) = fixture();
    let destination = u32::from_le_bytes(image[0x1603..0x1607].try_into().unwrap());
    let wide = [
        vec![0x68],
        1500_i32.to_le_bytes().to_vec(),
        vec![0x68],
        destination.to_le_bytes().to_vec(),
        vec![0xe8],
        numeric
            .math_i32_to_real
            .va
            .wrapping_sub(binding.maximum_initializer.va + 15)
            .to_le_bytes()
            .to_vec(),
        vec![0x83, 0xc4, 8, 0xc3],
    ]
    .concat();
    image[0x1600..0x1600 + wide.len()].copy_from_slice(&wide);
    binding.maximum_initializer.len = wide.len() as u32;
    refresh(&image, &mut binding, &mut numeric);
    let n = NumericPolicy::read(&image, numeric, budget, 32).unwrap();
    let r = Rules::read(&image, binding, &n, budget).unwrap();
    assert_eq!(r.clamp(2000.0), 1500.0);
    let (mut image, mut binding, mut numeric, budget) = fixture();
    image[0x1601] = (-2_i8) as u8;
    image[0x1700..0x1704].copy_from_slice(&(-7.5_f32).to_le_bytes());
    refresh(&image, &mut binding, &mut numeric);
    let n = NumericPolicy::read(&image, numeric, budget, 32).unwrap();
    let r = Rules::read(&image, binding, &n, budget).unwrap();
    assert_eq!(r.clamp(0.0), -2.0);
    assert_eq!(r.clamp(-10.0), -7.5);
}

pub(crate) fn health_fixture() -> (
    Vec<u8>,
    HealthBinding,
    crate::script_numeric::Binding,
    RuleInputBudget,
) {
    let (mut image, mut numeric, mut budget) = crate::script_numeric::tests::fixture();
    let loc = |offset: u32| 0x301000 + offset;
    let field = loc(0x1a10);
    let mut setter = vec![0; 192];
    for (at, data) in [
        (
            0,
            &[
                0x55, 0x8b, 0xec, 0x83, 0xec, 0x0c, 0x56, 0xff, 0x75, 0x08, 0xe8, 0xc1, 0x91, 0xfd,
                0xff,
            ][..],
        ),
        (
            15,
            &[
                0x8b, 0xf0, 0x83, 0xc4, 0x04, 0x85, 0xf6, 0x0f, 0x84, 0x9f, 0x00, 0x00, 0x00, 0x8b,
                0x8e, 0x5c, 0x03, 0x00, 0x00, 0x8d, 0x45, 0xfc, 0x57, 0x8b, 0x7d, 0x0c, 0x57, 0x50,
                0x8d, 0x45, 0xf4, 0x89, 0x4d, 0xfc, 0x50, 0xe8, 0xd9, 0x6a, 0x4f, 0x00, 0xf3, 0x0f,
                0x10, 0x05, 0x54, 0xb0, 0x0d, 0x01, 0x83, 0xc4, 0x0c,
            ],
        ),
        (
            66,
            &[
                0x8b, 0x00, 0x25, 0xff, 0xff, 0xff, 0x7f, 0x89, 0x45, 0xfc, 0x0f, 0x2f, 0x45, 0xfc,
                0x77, 0x68,
            ],
        ),
        (
            82,
            &[
                0x8b, 0x07, 0x6a, 0x1f, 0x6a, 0x06, 0x89, 0x45, 0xf8, 0x0f, 0xb6, 0x46, 0x30, 0x6a,
                0x04, 0x50, 0x6a, 0x18, 0x8d, 0x45, 0xf8, 0x50, 0x68, 0x70, 0x9f, 0x48, 0x00, 0xe8,
                0xee, 0x53, 0x1e, 0x00, 0x8b, 0x07, 0x83, 0xc4, 0x1c,
            ],
        ),
        (
            119,
            &[
                0xf3, 0x0f, 0x10, 0x05, 0x58, 0xb0, 0x0d, 0x01, 0x89, 0x45, 0xfc, 0xf3, 0x0f, 0x10,
                0x4d, 0xfc, 0x0f, 0x2f, 0xc1, 0x76, 0x07, 0xb8, 0x58, 0xb0, 0x0d, 0x01, 0xeb, 0x12,
            ],
        ),
        (
            147,
            &[
                0x0f, 0x2f, 0x0d, 0x54, 0xd8, 0x12, 0x01, 0xb8, 0x54, 0xd8, 0x12, 0x01, 0x8d, 0x4d,
                0xfc, 0x0f, 0x46, 0xc1,
            ],
        ),
        (
            165,
            &[
                0x8b, 0x00, 0x8d, 0x8e, 0x58, 0x03, 0x00, 0x00, 0x89, 0x45, 0xfc, 0x8d, 0x55, 0xfc,
                0x8b, 0x01, 0x6a, 0x01, 0x52, 0xff, 0x10, 0x5f, 0x5e, 0x8b, 0xe5, 0x5d, 0xc3,
            ],
        ),
    ] {
        setter[at..at + data.len()].copy_from_slice(data);
    }
    for (at, value) in [
        (11, loc(0x1900).wrapping_sub(loc(0x1800) + 15)),
        (59, loc(0x1a00)),
        (141, loc(0x1a04)),
        (155, field),
    ] {
        setter[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    let mut getter = vec![0; 33];
    for (at, data) in [
        (
            0,
            &[
                0x55, 0x8b, 0xec, 0xff, 0x75, 0x08, 0xe8, 0xd5, 0xe1, 0xfe, 0xff, 0x83, 0xc4, 0x04,
                0x85, 0xc0,
            ][..],
        ),
        (16, &[0x75, 0x07, 0xa1, 0x10, 0xd8, 0x12, 0x01, 0x5d, 0xc3]),
        (25, &[0x8b, 0x80, 0x5c, 0x03, 0x00, 0x00, 0x5d]),
        (32, &[0xc3]),
    ] {
        getter[at..at + data.len()].copy_from_slice(data);
    }
    for (at, value) in [
        (7, loc(0x1800).wrapping_sub(loc(0x1900) + 11)),
        (19, field.wrapping_add(4)),
    ] {
        getter[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    let init = |value: u8, at: u32, dest: u32| {
        [
            vec![0x6a, value, 0x68],
            dest.to_le_bytes().to_vec(),
            vec![0xe8],
            numeric
                .math_i32_to_real
                .va
                .wrapping_sub(at + 12)
                .to_le_bytes()
                .to_vec(),
            vec![0x83, 0xc4, 8, 0xc3],
        ]
        .concat()
    };
    let mut put = |offset: usize, data: &[u8]| {
        image[0x200 + offset..0x200 + offset + data.len()].copy_from_slice(data);
        SourceWindow {
            va: loc(offset as u32),
            len: data.len() as u32,
            digest: sha256(data),
        }
    };
    let setter = put(0x1800, &setter);
    let getter = put(0x1900, &getter);
    let maximum_initializer = put(0x1940, &init(100, loc(0x1940), field));
    let null_initializer = put(0x1950, &init(1, loc(0x1950), field.wrapping_add(4)));
    put(0x1a00, &0.001_f32.to_le_bytes());
    put(0x1a04, &0.01_f32.to_le_bytes());
    numeric.image_sha256 = sha256(&image);
    budget.profile_bytes = 8192;
    let binding = HealthBinding {
        origin: numeric.origin.clone(),
        image_sha256: sha256(&image),
        setter,
        getter,
        maximum_initializer,
        null_initializer,
    };
    (image, binding, numeric, budget)
}

pub(crate) fn health_rules() -> (HealthRules, NumericPolicy) {
    let (image, binding, numeric, budget) = health_fixture();
    let numeric = NumericPolicy::read(&image, numeric, budget, 32).unwrap();
    (
        HealthRules::read(&image, binding, &numeric, budget).unwrap(),
        numeric,
    )
}

#[test]
fn health_source_operands_drive_limits_null_result_and_threshold() {
    let (r, _) = health_rules();
    assert_eq!(r.clamp(-1.0), 0.01);
    assert_eq!(r.clamp(0.6), 0.6);
    assert_eq!(r.clamp(250.0), 100.0);
    assert_eq!(r.null_result(), 1.0);
    assert!(!r.changes(1.0, 1.0));
    assert!(!r.changes(1.0, 1.000_5));
    assert!(r.changes(1.0, 1.001));
    assert!(r.changes(1.0, 0.998));
}

#[test]
fn health_changed_limits_and_shape_refuse() {
    for change in 0..5 {
        let (mut image, mut binding, mut numeric, budget) = health_fixture();
        match change {
            0 => image[0x1c00..0x1c04].copy_from_slice(&0.0_f32.to_le_bytes()),
            1 => image[0x1c04..0x1c08].copy_from_slice(&200.0_f32.to_le_bytes()),
            2 => image[0x1a00 + 66] ^= 1,
            3 => image[0x1b42] = 37,
            4 => binding.origin.source.clear(),
            _ => unreachable!(),
        }
        binding.image_sha256 = sha256(&image);
        numeric.image_sha256 = sha256(&image);
        for w in [
            &mut binding.setter,
            &mut binding.getter,
            &mut binding.maximum_initializer,
            &mut binding.null_initializer,
        ] {
            let at = 0x200 + (w.va - 0x301000) as usize;
            w.digest = sha256(&image[at..at + w.len as usize]);
        }
        let numeric = NumericPolicy::read(&image, numeric, budget, 32).unwrap();
        assert!(
            HealthRules::read(&image, binding, &numeric, budget).is_err(),
            "mutant {change}"
        );
    }
}

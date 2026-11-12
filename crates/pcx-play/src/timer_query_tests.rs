use super::*;

pub(crate) fn fixture(
    segment: i8,
    minimum: f32,
) -> (Vec<u8>, super::super::Binding, Binding, RuleInputBudget) {
    let (mut image, mut numeric, budget) = super::super::tests::fixture();
    let mut put = |offset: usize, len: usize, pieces: &[(usize, &[u8])], calls: &[(usize, u32)]| {
        let va = 0x301000 + offset as u32;
        let mut bytes = vec![0x90; len];
        for &(at, piece) in pieces {
            bytes[at..at + piece.len()].copy_from_slice(piece);
        }
        for &(at, target) in calls {
            bytes[at] = 0xe8;
            bytes[at + 1..at + 5]
                .copy_from_slice(&target.wrapping_sub(va + at as u32 + 5).to_le_bytes());
        }
        image[0x200 + offset..0x200 + offset + len].copy_from_slice(&bytes);
        SourceWindow {
            va,
            len: len as u32,
            digest: sha256(&bytes),
        }
    };
    let segment_va = 0x302d00u32;
    let zero_va = segment_va + 4;
    let min_va = segment_va + 8;
    let initializer = |value: i8, dest: u32| {
        let mut b = vec![0x6a, value as u8, 0x68];
        b.extend(dest.to_le_bytes());
        b.extend([0x90; 5]);
        b.extend([0x83, 0xc4, 8, 0xc3]);
        b
    };
    let segment_initializer = put(
        0x800,
        16,
        &[(0, &initializer(segment, segment_va))],
        &[(7, numeric.math_i32_to_real.va)],
    );
    let zero_initializer = put(
        0x820,
        16,
        &[(0, &initializer(0, zero_va))],
        &[(7, numeric.math_i32_to_real.va)],
    );
    let constructor = put(
        0x850,
        0xd1,
        &[
            (0xa8, &zero_va.to_le_bytes()),
            (0xb7, &zero_va.to_le_bytes()),
        ],
        &[],
    );
    let schedule_va = 0x301a80;
    let remainder_va = 0x301b00;
    let start = put(
        0x950,
        0x118,
        &[
            (0x89, &segment_va.to_le_bytes()),
            (0x9c, &segment_va.to_le_bytes()),
            (0xd9, &segment_va.to_le_bytes()),
            (0x99, &[0xf7, 0xf9]),
            (0xa3, &[0x66, 0x89, 0x43, 0x14]),
        ],
        &[
            (0xab, remainder_va),
            (0xdf, schedule_va),
            (0xf3, schedule_va),
        ],
    );
    let schedule = put(
        0xa80,
        0x5c,
        &[
            (7, &[0xf3, 0x0f, 0x10, 5]),
            (0xb, &min_va.to_le_bytes()),
            (0x1e, &min_va.to_le_bytes()),
        ],
        &[(0x37, numeric.math_add.va)],
    );
    let remainder = put(
        0xb00,
        0x142,
        &[
            (0x33, &numeric.reciprocal_table.va.to_le_bytes()),
            (0x3e, &(numeric.reciprocal_table.va + 4).to_le_bytes()),
        ],
        &[
            (0x79, numeric.math_mul.va),
            (0xb2, numeric.math_sub.va),
            (0xcf, numeric.math_mul.va),
        ],
    );
    let remaining = put(
        0xc50,
        0x104,
        &[
            (0x54, &zero_va.to_le_bytes()),
            (0x66, &segment_va.to_le_bytes()),
        ],
        &[
            (0x6f, numeric.math_i32_to_real.va),
            (0x7c, numeric.math_mul.va),
            (0x8a, numeric.math_add.va),
            (0xa9, numeric.math_sub.va),
            (0xba, numeric.math_add.va),
            (0xda, numeric.math_add.va),
        ],
    );
    let full_timeout = put(
        0xd60,
        0xbe,
        &[(0x6e, &segment_va.to_le_bytes())],
        &[
            (0x77, numeric.math_i32_to_real.va),
            (0x84, numeric.math_mul.va),
            (0x91, numeric.math_add.va),
        ],
    );
    let active_elapsed = put(
        0xe30,
        0x34,
        &[(0x11, &[0xff, 0x50, 0x18]), (0x1d, &[0xff, 0x50, 0x10])],
        &[(0x25, numeric.math_sub.va)],
    );
    let inactive_elapsed = put(
        0xe70,
        0x6f,
        &[(0x43, &[0x8b, 0x46, 0x50]), (0x4c, &[0x8b, 0x46, 0x48])],
        &[(0x5e, numeric.math_sub.va)],
    );
    put(0x1d08, 4, &[(0, &minimum.to_le_bytes())], &[]);
    numeric.image_sha256 = sha256(&image);
    let binding = Binding {
        image_sha256: numeric.image_sha256,
        segment_initializer,
        zero_initializer,
        constructor,
        start,
        schedule,
        remainder,
        remaining,
        full_timeout,
        active_elapsed,
        inactive_elapsed,
    };
    (image, numeric, binding, budget)
}

pub(crate) fn policy(segment: i8, minimum: f32) -> NumericPolicy {
    let (image, n, b, budget) = fixture(segment, minimum);
    NumericPolicy::read(&image, n, budget, 64)
        .unwrap()
        .with_timer_queries(&image, &b, budget)
        .unwrap()
}

#[test]
fn timer_query_rules_derive_nonstock_segment_minimum_and_refuse_bad_input() {
    let p = policy(8, 0.125);
    let rules = p.timer_queries().unwrap();
    let e = rules.begin(&p, 1.0, 16.0, 2).unwrap();
    assert_eq!(e.expiry, 17.125);
    assert_eq!(rules.remaining(&p, e, 9.0).unwrap(), 8.0);
    assert_eq!(rules.elapsed(&p, e, 8.0).unwrap(), 8.0);
    assert!(rules.begin(&p, 1.0, 16.0, 1).is_err());
    for t in [
        f32::NAN,
        f32::INFINITY,
        -1.0,
        f32::from_bits(1),
        i32::MAX as f32,
    ] {
        assert!(rules.begin(&p, 1.0, t, usize::MAX).is_err());
    }
    assert!(rules.begin(&p, f32::INFINITY, 1.0, 10).is_err());
    for (segment, min) in [(0, 0.125), (8, -0.5), (8, f32::NAN), (8, 8.0)] {
        let (image, n, b, budget) = fixture(segment, min);
        assert!(
            NumericPolicy::read(&image, n, budget, 64)
                .unwrap()
                .with_timer_queries(&image, &b, budget)
                .is_err()
        );
    }
    let (image, n, mut b, budget) = fixture(8, 0.125);
    b.image_sha256[0] ^= 1;
    assert!(
        NumericPolicy::read(&image, n, budget, 64)
            .unwrap()
            .with_timer_queries(&image, &b, budget)
            .is_err()
    );
    let (image, n, b, budget) = fixture(8, 0.125);
    let tiny = RuleInputBudget {
        profile_bytes: 1,
        ..budget
    };
    assert!(
        NumericPolicy::read(&image, n, budget, 64)
            .unwrap()
            .with_timer_queries(&image, &b, tiny)
            .is_err()
    );
    let (mut image, mut n, mut b, budget) = fixture(8, 0.125);
    image[0x200 + (b.start.va - 0x301000) as usize + 0x99] ^= 1;
    b.start.digest = sha256(
        &image[0x200 + (b.start.va - 0x301000) as usize
            ..0x200 + (b.start.va - 0x301000) as usize + b.start.len as usize],
    );
    n.image_sha256 = sha256(&image);
    b.image_sha256 = n.image_sha256;
    assert!(
        NumericPolicy::read(&image, n, budget, 64)
            .unwrap()
            .with_timer_queries(&image, &b, budget)
            .is_err()
    );
}

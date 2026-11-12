use super::*;

type Memory = Vec<(u32, Vec<u8>, bool)>;

fn fixture(shift: u32) -> Memory {
    let minimum = shift + 0x10000u32;
    let epsilon = shift + 0x10010;
    let cycle = shift + 0x10020;
    let origin = shift + 0x10030;
    let mut schedule = vec![0x90; 0x85];
    schedule[3..6].copy_from_slice(&[0x8b, 0x4f, 0x48]);
    schedule[0x10..0x20].copy_from_slice(&[
        0xf3, 0x0f, 0x10, 0x45, 0x18, 0x0f, 0x2f, 0x45, 0xfc, 0x0f, 0x93, 0xc1, 0x85, 0xc9, 0x74,
        0x17,
    ]);
    schedule[0x20] = 0x68;
    schedule[0x21..0x25].copy_from_slice(&epsilon.to_le_bytes());
    schedule[0x37..0x3b].copy_from_slice(&[0xf3, 0x0f, 0x10, 0x05]);
    schedule[0x3b..0x3f].copy_from_slice(&minimum.to_le_bytes());
    schedule[0x4c..0x4f].copy_from_slice(&[0x0f, 0x47, 0x05]);
    schedule[0x4f..0x53].copy_from_slice(&minimum.to_le_bytes());
    schedule[0x60..0x63].copy_from_slice(&[0x8d, 0x47, 0x40]);
    for (at, target) in [(0x2d, 0x9a00b0u32), (0x68, 0x9a01c0), (0x80, 0x7fbe00)] {
        schedule[at] = 0xe8;
        schedule[at + 1..at + 5]
            .copy_from_slice(&target.wrapping_sub(0x7fb771 + at as u32 + 5).to_le_bytes());
    }
    let mut manager = vec![0x90; 28];
    manager[0] = 0x68;
    manager[1..5].copy_from_slice(&cycle.to_le_bytes());
    manager[7] = 0xe8;
    manager[8..12].copy_from_slice(&0x7f3390u32.wrapping_sub(0x7f73cc + 12).to_le_bytes());
    manager[25..].copy_from_slice(&[0x89, 0x47, 0x44]);
    let ctor = [
        &[0xa1][..],
        &origin.to_le_bytes(),
        &[
            0x89, 0x47, 0x40, 0x8b, 0x45, 0x08, 0xc7, 0x47, 0x44, 0, 0, 0, 0, 0x8b, 0, 0x89, 0x47,
            0x48,
        ],
    ]
    .concat();
    let bootstrap = |at: u32, global: u32, value: i32| {
        [
            &[0x68][..],
            &value.to_le_bytes(),
            &[0x68],
            &global.to_le_bytes(),
            &[0xe8],
            &0x9a1620u32.wrapping_sub(at + 15).to_le_bytes(),
            &[0x83, 0xc4, 0x08, 0xc3],
        ]
        .concat()
    };
    vec![
        (0xe6e240, 0x7fb700u32.to_le_bytes().to_vec(), false),
        (
            0x7fb75b,
            vec![
                0x83, 0x7d, 0x18, 0, 0x74, 0x09, 0x83, 0x4e, 0x10, 0x04, 0x8b, 0x7f, 0x48, 0xeb,
                0x03, 0x8b, 0x7f, 0x44,
            ],
            true,
        ),
        (0x7fb771, schedule, true),
        (0x7f73cc, manager, true),
        (0x7f3407, ctor, true),
        (0x401300, bootstrap(0x401300, origin, 0), true),
        (0x401570, bootstrap(0x401570, cycle, 257), true),
        (minimum, 0.03125f32.to_le_bytes().to_vec(), false),
        (epsilon, 0.0625f32.to_le_bytes().to_vec(), false),
    ]
}

fn decode(memory: &Memory) -> Result<Rules, String> {
    Rules::read(&mut |va, n, code| {
        memory
            .iter()
            .find(|(at, b, executable)| {
                va >= *at && va - *at < b.len() as u32 && code == *executable
            })
            .and_then(|(at, b, _)| b.get((va - *at) as usize..(va - *at) as usize + n))
            .map(<[u8]>::to_vec)
            .ok_or_else(|| "synthetic simple timer window missing".into())
    })
}

#[test]
fn simple_timer_uses_nonstock_source_clamps_and_relocated_scalar_operands() {
    for shift in [0, 0x100000] {
        let rules = decode(&fixture(shift)).unwrap();
        assert_eq!(rules.cycle, 257.);
        assert_eq!(rules.deadline(5., 0.).unwrap(), 5.03125);
        assert_eq!(rules.deadline(5., -7.).unwrap(), 5.03125);
        assert_eq!(rules.deadline(5., 11.75).unwrap(), 16.75);
        assert_eq!(rules.deadline(0., 257.).unwrap(), 256.9375);
        assert_eq!(rules.deadline(0., 999.).unwrap(), 256.9375);
    }
}

#[test]
fn simple_timer_refuses_unqualified_clock_domains_wrap_and_numeric_states() {
    let rules = decode(&fixture(0)).unwrap();
    for (now, delay) in [
        (0.1, 1.),
        (f64::NAN, 1.),
        (f64::INFINITY, 1.),
        (-1., 1.),
        (257., 1.),
        (256., 2.),
        (0., f32::NAN),
        (0., f32::INFINITY),
        (0., f32::from_bits(1)),
    ] {
        assert!(rules.deadline(now, delay).is_err(), "{now} {delay}");
    }
}

#[test]
fn simple_timer_refuses_changed_virtual_queue_clamp_or_arithmetic_roles() {
    for mutate in [
        (|m: &mut Memory| m[0].1[0] ^= 1) as fn(&mut Memory),
        |m| m[1].1[17] = 0x48,
        |m| m[2].1[0x1f] ^= 1,
        |m| m[2].1[0x4f] ^= 1,
        |m| m[2].1[0x69] ^= 1,
        |m| m[3].1[27] = 0x48,
        |m| m[4].1[22] = 0x44,
        |m| m[6].1[1..5].copy_from_slice(&0i32.to_le_bytes()),
        |m| m[7].1.copy_from_slice(&(-1f32).to_le_bytes()),
    ] {
        let mut memory = fixture(0);
        mutate(&mut memory);
        assert!(decode(&memory).is_err());
    }
}

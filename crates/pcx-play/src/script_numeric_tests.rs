use super::*;
use crate::playercolor_host::{EnumHost, Limits, ObjectHost};
use crate::scripts::{MapScripts, ScriptAsset};
use pcx_script::{Host, RealOps};
use std::sync::Arc;

// Self-authored encodings and arbitrary table values; no original image bytes.
pub(crate) fn fixture() -> (Vec<u8>, Binding, RuleInputBudget) {
    let mut image = vec![0; 0x2000];
    image[..2].copy_from_slice(b"MZ");
    image[0x80..0x84].copy_from_slice(b"PE\0\0");
    for (at, value) in [
        (0x3c, 0x80u32),
        (0xb4, 0x300000),
        (0xd0, 0x3000),
        (0x100, 0x1e00),
        (0x104, 0x1000),
        (0x108, 0x1e00),
        (0x10c, 0x200),
        (0x11c, 0x60000020),
    ] {
        image[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    for (at, value) in [(0x84, 0x14cu16), (0x86, 1), (0x94, 0x60), (0x98, 0x10b)] {
        image[at..at + 2].copy_from_slice(&value.to_le_bytes());
    }
    let base = 0x301000u32;
    let loc = |offset| base + offset;
    let mut put = |offset: usize, bytes: &[u8]| {
        image[0x200 + offset..0x200 + offset + bytes.len()].copy_from_slice(bytes);
        SourceWindow {
            va: loc(offset as u32),
            len: bytes.len() as u32,
            digest: sha256(bytes),
        }
    };
    let mut sub = vec![0x55, 0x8b, 0xec, 0x8b, 0x45, 16, 0x8b, 0x4d, 12];
    sub.extend([0x35, 0, 0, 0, 0x80, 0x89, 0x01, 0xc3]);
    let math_sub = put(0, &sub);
    let math_add = put(
        0x40,
        &[
            0x55, 0x8b, 0xec, 0x8b, 0x45, 12, 0x53, 0x56, 0x57, 0x8b, 0x08, 0x03, 0xf6, 0x03, 0xd2,
            0x89, 0x01, 0xc3,
        ],
    );
    let math_mul = put(0x80, &[0x55, 0xf7, 0xe7, 0x89, 0x01, 0xc3]);
    let math_lzcnt = put(
        0xc0,
        &[
            0x55, 0x8b, 0xec, 0x51, 0x89, 0x4d, 0xfc, 0x0f, 0xbd, 0xc1, 0xc3,
        ],
    );
    let patch_call = |bytes: &mut [u8], base: u32, at: usize, target: u32| {
        bytes[at] = 0xe8;
        bytes[at + 1..at + 5]
            .copy_from_slice(&target.wrapping_sub(base + at as u32 + 5).to_le_bytes());
    };
    let mut convert = vec![
        0x55, 0x8b, 0xec, 0x8b, 0x45, 0x0c, 0x85, 0xc0, 0x75, 0x0b, 0x8b, 0x45, 0x08, 0xc7, 0x00,
        0, 0, 0, 0, 0x5d, 0xc3, 0x56, 0x57, 0x8b, 0xf8, 0x99, 0x8b, 0xf0, 0x81, 0xe7, 0, 0, 0,
        0x80, 0x33, 0xf2, 0x2b, 0xf2, 0x8b, 0xce,
    ];
    let at = convert.len();
    convert.extend([0; 5]);
    patch_call(&mut convert, loc(0x100), at, math_lzcnt.va);
    convert.extend([0x89, 0x08, 0x5e, 0x5d, 0xc3]);
    let math_i32_to_real = put(0x100, &convert);
    let table_offset = 0x700;
    let mut reciprocal = vec![0x90; 103];
    for (at, bytes) in [
        (0, &[0x55, 0x8b, 0xec, 0x8b, 0x45, 0x0c, 0x56, 0x57][..]),
        (0x19, &[0xc1, 0xea]),
        (0x24, &[0x8b, 0x34, 0x95]),
        (0x2f, &[0x2b, 0x0c, 0x95]),
        (0x36, &[0xf7, 0xe1]),
        (0x4c, &[0x81, 0xc6]),
        (98, &[0x89, 0x08, 0x5e, 0x5d, 0xc3]),
    ] {
        reciprocal[at..at + bytes.len()].copy_from_slice(bytes);
    }
    reciprocal[0x16] = 9;
    for at in [0x1b, 0x1e, 0x23] {
        reciprocal[at] = 23;
    }
    reciprocal[0x27..0x2b].copy_from_slice(&loc(table_offset).to_le_bytes());
    reciprocal[0x32..0x36].copy_from_slice(&(loc(table_offset) + 4).to_le_bytes());
    reciprocal[0x4e..0x52].copy_from_slice(&((2u32 * 127 - 1) << 23).to_le_bytes());
    let reciprocal = put(0x180, &reciprocal);
    let mut division = vec![0x90; 57];
    division[..16].copy_from_slice(&[
        0x55, 0x8b, 0xec, 0x8b, 0x4d, 0x10, 0x56, 0x8b, 0x75, 0x0c, 0x8b, 0x06, 0x3b, 0x01, 0x74,
        0x1d,
    ]);
    patch_call(&mut division, loc(0x200), 0x15, reciprocal.va);
    patch_call(&mut division, loc(0x200), 0x20, math_mul.va);
    division[0x31..].copy_from_slice(&[0xc7, 0x00, 0, 0, 0x80, 0x3f, 0x5d, 0xc3]);
    let division = put(0x200, &division);
    let mut literal = vec![0x90; 238];
    for (at, bytes) in [
        (
            0,
            &[0x55, 0x8b, 0xec, 0x83, 0xec, 0x18, 0x53, 0x56, 0x57][..],
        ),
        (0x24, &[0x80, 0xf9, b'.', 0x74]),
        (
            0x2d,
            &[
                0x8d, 0x1c, 0x9b, 0x0f, 0xbe, 0xc9, 0x8d, 0x5b, 0xe8, 0x8d, 0x1c, 0x59,
            ],
        ),
        (0x76, &[0x8d, 0x1c, 0x9b, 0x8d, 0x3c, 0xbf]),
        (234, &[0x8b, 0xe5, 0x5d, 0xc3]),
    ] {
        literal[at..at + bytes.len()].copy_from_slice(bytes);
    }
    for (at, target) in [
        (0x43, math_i32_to_real.va),
        (0x98, math_i32_to_real.va),
        (0xa8, math_i32_to_real.va),
        (0xb5, division.va),
        (0xc3, math_i32_to_real.va),
        (0xd0, math_add.va),
    ] {
        patch_call(&mut literal, loc(0x280), at, target);
    }
    let literal = put(0x280, &literal);
    let equality_constant = put(0x780, &0.25f32.to_le_bytes());
    let mut equality = vec![0x90; 53];
    patch_call(&mut equality, loc(0x400), 14, math_sub.va);
    equality[19..23].copy_from_slice(&[0xf3, 0x0f, 0x10, 0x05]);
    equality[23..27].copy_from_slice(&equality_constant.va.to_le_bytes());
    equality[32..37].copy_from_slice(&[0x25, 0xff, 0xff, 0xff, 0x7f]);
    equality[42..49].copy_from_slice(&[0x0f, 0x2f, 0x45, 0xfc, 0x0f, 0x97, 0xc0]);
    let equality = put(0x400, &equality);
    let mut inequality = vec![0x90; 162];
    inequality[..4].copy_from_slice(&[0x0f, 0xb6, 0x47, 1]);
    inequality[0x21..0x25].copy_from_slice(&[0x3b, 0x44, 0xcb, 0x70]);
    inequality[0x80..0x83].copy_from_slice(&[0x0f, 0x95, 0xc2]);
    let inequality = put(0x480, &inequality);
    let reciprocal_table = put(table_offset as usize, &[0, 0, 0x80, 0, 0, 0, 0x80, 0]);
    let binding = Binding {
        origin: AssetIdentity {
            edition: "synthetic".into(),
            version: "fixture".into(),
            source: "fixture:numeric".into(),
        },
        image_sha256: sha256(&image),
        literal,
        division,
        reciprocal,
        reciprocal_table,
        equality,
        equality_constant,
        inequality,
        math_sub,
        math_add,
        math_mul,
        math_lzcnt,
        math_i32_to_real,
    };
    (
        image,
        binding,
        RuleInputBudget {
            image_bytes: 8192,
            profile_bytes: 4096,
            sections: 2,
        },
    )
}
fn policy() -> NumericPolicy {
    let (image, binding, budget) = fixture();
    NumericPolicy::read(&image, binding, budget, 64).unwrap()
}

#[test]
fn literals_and_equality_use_the_bound_data_and_preserve_distinct_operators() {
    let p = policy();
    assert_eq!(p.literal("12.0").unwrap(), 12.0);
    assert_eq!(p.literal("1.5").unwrap(), 1.625); // Synthetic reciprocal table intentionally differs.
    assert_eq!(p.binary(BinOp::Eq, 1.0, 1.125).unwrap(), Value::Bool(true));
    assert_eq!(p.binary(BinOp::Ne, 1.0, 1.125).unwrap(), Value::Bool(true));
    assert_eq!(p.binary(BinOp::Eq, 0.0, 0.25).unwrap(), Value::Bool(false));
    assert_eq!(p.binary(BinOp::Ne, 0.0, -0.0).unwrap(), Value::Bool(true));
    for zero in [0.0, -0.0] {
        assert!(p.binary(BinOp::Div, 0.0, zero).is_err());
        assert!(p.binary(BinOp::Div, 1.0, zero).is_err());
    }
    assert_eq!(
        p.integer(i32::MAX).unwrap().to_bits(),
        real::from_i32(i32::MAX).unwrap()
    );
    for text in [
        "1e3",
        "1.2.3",
        "-1.0",
        ".",
        "1.00000000000000000000000000000000000000000000000000000000000000000",
    ] {
        assert!(p.literal(text).is_err(), "{text}");
    }
    for value in [f32::INFINITY, f32::NAN, f32::from_bits(1)] {
        assert!(p.binary(BinOp::Add, value, 1.0).is_err());
    }
    let (mut image, mut binding, budget) = fixture();
    image[0x980..0x984].copy_from_slice(&0.0625f32.to_le_bytes());
    binding.image_sha256 = sha256(&image);
    binding.equality_constant.digest = sha256(&image[0x980..0x984]);
    assert_eq!(
        NumericPolicy::read(&image, binding, budget, 64)
            .unwrap()
            .binary(BinOp::Eq, 1.0, 1.125)
            .unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn image_roles_targets_lengths_and_budgets_refuse_mutants() {
    for mutation in 0..12 {
        let (mut image, mut binding, mut budget) = fixture();
        match mutation {
            0 => binding.origin.version.clear(),
            1 => binding.image_sha256[0] ^= 1,
            2 => budget.image_bytes = 1,
            3 => budget.profile_bytes = 1,
            4 => binding.literal.digest[0] ^= 1,
            5 => binding.reciprocal_table.len -= 4,
            6 => binding.equality_constant.va = binding.literal.va,
            7 => {
                image[0x200 + 0x280 + 0xb6] ^= 1;
                binding.literal.digest = sha256(&image[0x480..0x480 + 238]);
                binding.image_sha256 = sha256(&image);
            }
            8 => {
                image[0x200 + 0x180 + 0x1b] = 0;
                binding.reciprocal.digest = sha256(&image[0x380..0x380 + 103]);
                binding.image_sha256 = sha256(&image);
            }
            9 => {
                image[0x200 + 0x400 + 23] ^= 1;
                binding.equality.digest = sha256(&image[0x600..0x600 + 53]);
                binding.image_sha256 = sha256(&image);
            }
            10 => binding.math_sub.va = binding.math_add.va,
            _ => binding.literal.len = u32::MAX,
        }
        assert!(
            NumericPolicy::read(&image, binding, budget, 64).is_err(),
            "mutant {mutation}"
        );
    }
}

#[test]
fn object_authority_survives_both_localization_wrappers_and_vm_calls() {
    let source = b"type agent extends handle\ntype timer extends agent\nnative CreateTimer takes nothing returns timer\nnative DestroyTimer takes timer t returns nothing\nnative I2R takes integer i returns real\nnative Hold takes nothing returns real\n";
    let map=b"function converted takes nothing returns real\nreturn 1.5 + I2R(1)\nendfunction\nfunction pair takes real a, real b returns boolean\nreturn a == b and a != b\nendfunction\nfunction parked takes nothing returns boolean\nreturn (1.0 == Hold()) and (1.0 != Hold())\nendfunction\n";
    let scripts = Arc::new(
        MapScripts::compile(
            AssetIdentity {
                edition: "synthetic".into(),
                version: "fixture".into(),
                source: "fixture:script".into(),
            },
            ScriptAsset {
                label: "common",
                bytes: source,
            },
            ScriptAsset {
                label: "blizzard",
                bytes: b"// fixture",
            },
            ScriptAsset {
                label: "map",
                bytes: map,
            },
            None,
        )
        .unwrap(),
    );
    let host = || {
        ObjectHost::checked(
            EnumHost::checked_selected(
                scripts.shared_program(),
                "fixture:script",
                &[],
                Limits {
                    max_handles: 4,
                    admission_steps: 4,
                    expression_depth: 16,
                    expression_work: 32,
                },
            )
            .unwrap(),
        )
        .unwrap()
        .with_script_numeric(policy())
    };
    let mut borrowed = scripts.vm(host());
    assert_eq!(borrowed.call("converted", &[]).unwrap(), Value::Real(2.625));
    assert_eq!(
        borrowed
            .call("pair", &[Value::Real(1.0), Value::Real(1.125)])
            .unwrap(),
        Value::Bool(true)
    );
    let mut owned = Arc::clone(&scripts).owned_vm(host());
    assert_eq!(owned.call("converted", &[]).unwrap(), Value::Real(2.625));
    assert!(owned.host().real_ops().is_some());
    struct Park(ObjectHost);
    impl Host for Park {
        fn real_ops(&self) -> Option<&dyn RealOps> {
            self.0.real_ops()
        }
        fn suspend(&mut self, name: &str, _: &[Value]) -> bool {
            name == "Hold"
        }
        fn native(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
            self.0.native(name, args)
        }
    }
    let mut parked = Arc::clone(&scripts).owned_vm(Park(host()));
    assert_eq!(
        parked.invoke("parked", &[]).unwrap(),
        pcx_script::Exec::Suspended
    );
    assert_eq!(
        parked.resume(Value::Real(1.125)).unwrap(),
        pcx_script::Exec::Suspended
    );
    assert_eq!(
        parked.resume(Value::Real(1.125)).unwrap(),
        pcx_script::Exec::Finished(Value::Bool(true))
    );
}

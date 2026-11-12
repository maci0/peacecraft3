use super::*;
use crate::script_numeric::NumericPolicy;
use pcx_script::{Host, RealOps, Value, Vm};

fn put(image: &mut [u8], offset: usize, bytes: &[u8]) -> SourceWindow {
    image[0x200 + offset..0x200 + offset + bytes.len()].copy_from_slice(bytes);
    SourceWindow {
        va: 0x301000 + offset as u32,
        len: bytes.len() as u32,
        digest: sha256(bytes),
    }
}
fn patch_call(bytes: &mut [u8], base: u32, offset: usize, target: u32) {
    bytes[offset] = 0xe8;
    bytes[offset + 1..offset + 5]
        .copy_from_slice(&target.wrapping_sub(base + offset as u32 + 5).to_le_bytes());
}
// Arbitrary profile values and hand-authored instruction skeletons, not image copies.
fn fixture() -> (Vec<u8>, NumericBinding, Binding, RuleInputBudget) {
    let (mut image, mut numeric, budget) = crate::script_numeric::tests::fixture();
    let va = |offset: u32| 0x301000u32 + offset;
    let mut body = vec![0x90; 0x2b0];
    for (offset, bytes) in [
        (0, &[0x55, 0x8b, 0xec, 0x8b, 0x55, 8][..]),
        (0x18, &[0x85, 0xc8, 0x74, 4, 0xc6, 3, b'-']),
        (0x2f, &[0x81, 0xe1, 0, 0, 0x80, 0x7f, 0x81, 0xf9]),
        (0x46, &[0x0f, 0x2f, 5]),
        (0x51, &[0xc7, 3, b'i', b'n', b'f', 0]),
        (0x6b, &[0xc7, 0x45, 0xf4]),
        (0xa8, &[0x68]),
        (0xc3, b"0000"),
        (0xd8, &[0xbe]),
        (0xdd, &[0xc6, 3, b'.']),
        (0x110, &[0xbb]),
        (0x16d, &[0xb9]),
        (0x177, &[0x8d, 0x51, 5]),
        (0x17f, &[0x8d, 0x72, (-9i8) as u8]),
        (0x1f1, &[0x68]),
        (0x25a, &[0x83, 0xfb, 1]),
        (0x260, &[0xb8]),
        (0x269, &[0xc1, 0xfa, 2]),
        (0x273, &[0x8d, 0x0c, 0xb6, 0x03, 0xc9]),
        (0x27f, &[0x83, 0xfb, 1]),
        (0x287, &[0xb9]),
        (0x299, &[0x68]),
    ] {
        body[offset..offset + bytes.len()].copy_from_slice(bytes);
    }
    for (offset, value) in [
        (0x37, (127 + 31) << 23),
        (0x49, va(0x1c94)),
        (0x6e, 0.25f32.to_bits()),
        (0xa9, va(0x1ca8)),
        (0xd9, 1),
        (0x111, 4),
        (0x16e, 5),
        (0x1f2, va(0x1c90)),
        (0x261, ((1u64 << 34) / 10 + 1) as u32),
        (0x288, 1),
        (0x29a, va(0x1c98)),
    ] {
        body[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    for (offset, target) in [
        (0x7b, numeric.math_mul.va),
        (0xa2, va(0x1c30)),
        (0x1c8, numeric.math_sub.va),
        (0x1de, numeric.math_i32_to_real.va),
        (0x1ec, numeric.math_mul.va),
        (0x1fe, numeric.math_add.va),
        (0x208, va(0x1b90)),
    ] {
        patch_call(&mut body, va(0x1800), offset, target);
    }
    let body = put(&mut image, 0x1800, &body);
    let make_native = |wide: bool| {
        let mut bytes = vec![0x90; if wide { 106 } else { 104 }];
        bytes[..5].copy_from_slice(&[0x55, 0x8b, 0xec, 0x81, 0xec]);
        bytes[5..9].copy_from_slice(&100u32.to_le_bytes());
        bytes[0x10..0x13].copy_from_slice(&[0x89, 0x45, 0xfc]);
        let lea = if wide { 0x19 } else { 0x16 };
        bytes[lea..lea + 2].copy_from_slice(&[0x8d, 0x8d]);
        bytes[lea + 2..lea + 6].copy_from_slice(&(-100i32).to_le_bytes());
        if wide {
            bytes[0x13..0x16].copy_from_slice(&[0xff, 0x75, 0x10]);
            bytes[0x1f..0x22].copy_from_slice(&[0xff, 0x75, 0x0c]);
        } else {
            bytes[0x1c..0x20].copy_from_slice(&[0x6a, 2, 0x6a, 0]);
        }
        patch_call(
            &mut bytes,
            va(if wide { 0x1b20 } else { 0x1ab0 }),
            if wide { 0x24 } else { 0x22 },
            body.va,
        );
        bytes
    };
    let r2s = put(&mut image, 0x1ab0, &make_native(false));
    let r2sw = put(&mut image, 0x1b20, &make_native(true));
    let mut floor = vec![0x90; 152];
    floor[..9].copy_from_slice(&[0x55, 0x8b, 0xec, 0x8b, 0x45, 0x0c, 0x56, 0x8b, 0x10]);
    floor[0x11..0x14].copy_from_slice(&[0x83, 0xee, 0x7f]);
    floor[0x35..0x38].copy_from_slice(&[0x83, 0xfe, 0x17]);
    let floor = put(&mut image, 0x1b90, &floor);
    let mut truncate = vec![0x90; 80];
    truncate[..9].copy_from_slice(&[0x55, 0x8b, 0xec, 0x8b, 0x45, 8, 0x56, 0x8b, 0x30]);
    truncate[0x11..0x14].copy_from_slice(&[0x83, 0xf8, 0x7f]);
    let truncate = put(&mut image, 0x1c30, &truncate);
    let rounding = put(&mut image, 0x1c90, &0.25f32.to_le_bytes());
    let large_limit = put(&mut image, 0x1c94, &f32::MAX.to_le_bytes());
    let format = put(&mut image, 0x1c98, b"%*d.%0*d\0");
    let integer_format = put(&mut image, 0x1ca8, b"%d\0");
    numeric.image_sha256 = sha256(&image);
    let binding = Binding {
        image_sha256: numeric.image_sha256,
        r2s,
        r2sw,
        body,
        floor,
        truncate,
        rounding,
        large_limit,
        format,
        integer_format,
    };
    (image, numeric, binding, budget)
}
fn policy() -> NumericPolicy {
    let (image, numeric, format, budget) = fixture();
    NumericPolicy::read(&image, numeric, budget, 64)
        .unwrap()
        .with_formatting(&image, &format, budget)
        .unwrap()
}
struct NumericHost(NumericPolicy);
impl Host for NumericHost {
    fn native(&mut self, name: &str, _: &[Value]) -> Result<Value, String> {
        Err(format!("unsupported {name}"))
    }
    fn real_ops(&self) -> Option<&dyn RealOps> {
        Some(&self.0)
    }
}
fn program() -> pcx_script::Program {
    pcx_script::resolve(&pcx_script::parse("native R2S takes real value returns string\nnative R2SW takes real value, integer width, integer precision returns string\nfunction Default takes real value returns string\nreturn R2S(value)\nendfunction\nfunction Width takes real value, integer width, integer precision returns string\nreturn R2SW(value,width,precision)\nendfunction\n").unwrap()).unwrap()
}
#[test]
fn real_formatter_vm_uses_nonstock_defaults_rounding_caps_and_signed_padding() {
    let program = program();
    let mut vm = Vm::new(&program, NumericHost(policy()));
    assert_eq!(
        vm.call("Default", &[Value::Real(0.125)]).unwrap(),
        Value::Str("0.12".into())
    );
    for (precision, expected) in [(-1, "0.5"), (9, "0.50000"), (0, "0.0")] {
        assert_eq!(
            vm.call(
                "Width",
                &[Value::Real(0.5), Value::Int(0), Value::Int(precision)]
            )
            .unwrap(),
            Value::Str(expected.into())
        );
    }
    assert_eq!(
        vm.call("Width", &[Value::Real(-0.5), Value::Int(8), Value::Int(3)])
            .unwrap(),
        Value::Str("-    0.500".into())
    );
    assert_eq!(
        vm.call("Default", &[Value::Real(-0.0)]).unwrap(),
        Value::Str("0.00".into())
    );
}
#[test]
fn real_formatter_large_class_keeps_precision_ignores_width_and_derives_scaling() {
    let p = policy();
    assert_eq!(
        p.real_to_string(2147483648.0, Some((i32::MAX, 6))).unwrap(),
        "5368709120.000000"
    );
    assert_eq!(p.real_to_string(f32::MAX, None).unwrap(), "inf");
    assert_eq!(p.real_to_string(f32::INFINITY, None).unwrap(), "inf");
    assert_eq!(p.real_to_string(f32::NEG_INFINITY, None).unwrap(), "-inf");
    assert_eq!(p.real_to_string(f32::from_bits(1), None).unwrap(), "0.00");
    assert_eq!(
        p.real_to_string(f32::from_bits(0x80000001), None).unwrap(),
        "-0.00"
    );
    assert!(p.real_to_string(f32::NAN, None).is_err());
}
#[test]
fn real_formatter_missing_source_capacity_and_lifecycle_errors_remain_explicit() {
    let (image, numeric, format, budget) = fixture();
    let bare = NumericPolicy::read(&image, numeric, budget, 64).unwrap();
    assert!(
        bare.real_to_string(0.5, None)
            .unwrap_err()
            .contains("not admitted")
    );
    let program = program();
    let mut vm = Vm::new(
        &program,
        NumericHost(bare.with_formatting(&image, &format, budget).unwrap()),
    );
    let error = vm
        .call("Width", &[Value::Real(0.5), Value::Int(128), Value::Int(3)])
        .unwrap_err();
    assert!(error.message.contains("selected native buffer"));
    assert_eq!(
        vm.call("Default", &[Value::Real(0.5)]).unwrap(),
        Value::Str("0.50".into())
    );
}
#[test]
fn real_formatter_profile_refuses_wrong_digest_call_target_and_operand_relationship() {
    let (image, numeric, format, budget) = fixture();
    for offset in [0x1800 + 0x17f, 0x1800 + 0x1ec, 0x1ab0 + 0x22] {
        let mut changed = image.clone();
        changed[0x200 + offset] ^= 1;
        let mut n = numeric.clone();
        n.image_sha256 = sha256(&changed);
        let mut f = format.clone();
        f.image_sha256 = n.image_sha256;
        for window in [&mut f.body, &mut f.r2s] {
            let start = 0x200 + (window.va - 0x301000) as usize;
            window.digest = sha256(&changed[start..start + window.len as usize]);
        }
        let policy = NumericPolicy::read(&changed, n, budget, 64).unwrap();
        assert!(
            policy.with_formatting(&changed, &f, budget).is_err(),
            "offset{offset:x}"
        );
    }
    let mut wrong = format.clone();
    wrong.body.digest = [0; 32];
    assert!(
        NumericPolicy::read(&image, numeric, budget, 64)
            .unwrap()
            .with_formatting(&image, &wrong, budget)
            .is_err()
    );
}

#[test]
#[ignore = "requires PCX_WAR3_EXE and PCX_FORMAT_CAPTURE_STATUS from the bounded local original-client probe"]
fn real_formatter_selected_image_matches_original_capture_through_vm() {
    use crate::scripts::AssetIdentity;
    let image = std::fs::read(std::env::var_os("PCX_WAR3_EXE").expect("selected image")).unwrap();
    let status = std::fs::read(
        std::env::var_os("PCX_FORMAT_CAPTURE_STATUS").expect("original probe receipt"),
    )
    .unwrap();
    let status: serde_json::Value = serde_json::from_slice(&status).unwrap();
    assert_eq!(status["complete"], true);
    assert_eq!(status["result"], "captured");
    assert!(status.get("cleanup_error").is_none());
    assert_eq!(status["isolation"]["verified"], true);
    let digest = sha256(&image);
    let text = digest
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    assert_eq!(status["source_executable_sha256"], text);
    assert_eq!(digest, crate::menu_lighting::REVIEWED_IMAGE);
    let raw = std::fs::read(status["trace"].as_str().unwrap()).unwrap();
    assert_eq!(
        status["trace_raw_sha256"],
        sha256(&raw)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    let records = std::fs::read(status["results"].as_str().unwrap()).unwrap();
    assert!(records.len() <= 512 * 1024);
    let records: serde_json::Value = serde_json::from_slice(&records).unwrap();
    let rows = records.as_array().unwrap();
    assert!(!rows.is_empty() && rows.len() <= 256);
    assert_eq!(status["records"].as_u64().unwrap(), rows.len() as u64 + 2);
    let budget = RuleInputBudget {
        image_bytes: 32 << 20,
        profile_bytes: 64 << 10,
        sections: 96,
    };
    let pe = Pe::parse(&image, budget).unwrap();
    let window = |va, len, code| {
        let bytes = &pe.read_from(va, code).unwrap()[..len as usize];
        SourceWindow {
            va,
            len,
            digest: sha256(bytes),
        }
    };
    let numeric = NumericBinding {
        origin: AssetIdentity {
            edition: "selected local Legacy".into(),
            version: "1.29.2.9231".into(),
            source: "explicit original formatter capture".into(),
        },
        image_sha256: digest,
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
    };
    let format = Binding {
        image_sha256: digest,
        r2s: window(0x4a4b10, 104, true),
        r2sw: window(0x4a4b80, 106, true),
        body: window(0x9a0870, 0x2b0, true),
        floor: window(0x9a1520, 152, true),
        truncate: window(0x9a07b0, 80, true),
        rounding: window(0x10db0a8, 4, false),
        large_limit: window(0x10db134, 4, false),
        format: window(0xed9f30, 9, false),
        integer_format: window(0xe6ddd8, 3, false),
    };
    let policy = NumericPolicy::read(&image, numeric, budget, 4096)
        .unwrap()
        .with_formatting(&image, &format, budget)
        .unwrap();
    let mut source = String::from(
        "native I2R takes integer value returns real\nnative R2S takes real value returns string\nnative R2SW takes real value, integer width, integer precision returns string\n",
    );
    for (i, row) in rows.iter().enumerate() {
        assert_eq!(row["index"].as_u64().unwrap(), i as u64);
        let expression = row["expression"].as_str().unwrap();
        assert!(
            expression.len() <= 160
                && expression
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_(),.-*".contains(&b))
        );
        source.push_str(&format!(
            "function Case{i} takes nothing returns string\nreturn {expression}\nendfunction\n"
        ));
    }
    let program = pcx_script::resolve(&pcx_script::parse(&source).unwrap()).unwrap();
    let mut vm = Vm::new(&program, NumericHost(policy));
    for (i, row) in rows.iter().enumerate() {
        assert_eq!(
            vm.call(&format!("Case{i}"), &[]).unwrap(),
            Value::Str(row["output"].as_str().unwrap().into()),
            "{} {}",
            row["label"],
            row["expression"]
        );
    }
    eprintln!(
        "{} original captured formatter expressions matched through the VM",
        rows.len()
    );
}

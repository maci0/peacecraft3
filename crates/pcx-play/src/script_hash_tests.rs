use super::*;
use pcx_script::{Host, Value, Vm};

fn put(image: &mut [u8], offset: usize, bytes: &[u8]) -> SourceWindow {
    image[0x200 + offset..0x200 + offset + bytes.len()].copy_from_slice(bytes);
    SourceWindow {
        va: 0x301000 + offset as u32,
        len: bytes.len() as u32,
        digest: sha256(bytes),
    }
}
fn branch(bytes: &mut [u8], base: u32, offset: usize, opcode: u8, target: u32) {
    bytes[offset] = opcode;
    bytes[offset + 1..offset + 5]
        .copy_from_slice(&target.wrapping_sub(base + offset as u32 + 5).to_le_bytes());
}
// Self-authored instruction skeletons with deliberately nonstock operands.
fn fixture() -> (Vec<u8>, Binding, RuleInputBudget) {
    let mut image = vec![0; 0x1200];
    image[..2].copy_from_slice(b"MZ");
    image[0x80..0x84].copy_from_slice(b"PE\0\0");
    for (at, value) in [
        (0x3c, 0x80u32),
        (0xb4, 0x300000),
        (0xd0, 0x3000),
        (0x100, 0x1000),
        (0x104, 0x1000),
        (0x108, 0x1000),
        (0x10c, 0x200),
        (0x11c, 0x60000020),
    ] {
        image[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    for (at, value) in [(0x84, 0x14cu16), (0x86, 1), (0x94, 0x60), (0x98, 0x10b)] {
        image[at..at + 2].copy_from_slice(&value.to_le_bytes());
    }
    let mut handler = vec![0x90; 33];
    handler[..6].copy_from_slice(&[0x55, 0x8b, 0xec, 0x8b, 0x4d, 8]);
    branch(&mut handler, 0x301000, 6, 0xe8, 0x301040);
    handler[0xb..0x14].copy_from_slice(&[0x85, 0xc0, 0x74, 0x0e, 0x80, 0x38, 0, 0x74, 9]);
    branch(&mut handler, 0x301000, 0x18, 0xe9, 0x301080);
    handler[0x1d..].copy_from_slice(&[0x33, 0xc0, 0x5d, 0xc3]);
    let handler = put(&mut image, 0, &handler);
    let raw = put(
        &mut image,
        0x40,
        &[
            0x8b, 0x41, 4, 0x85, 0xc0, 0x74, 4, 0x8b, 0x40, 8, 0xc3, 0x33, 0xc0, 0xc3,
        ],
    );
    let mut norm = vec![0x90; 127];
    for (at, bytes) in [
        (0, &[0x55, 0x8b, 0xec, 0x81, 0xec][..]),
        (0x10, &[0x89, 0x45, 0xfc]),
        (0x17, &[0x33, 0xf6]),
        (0x1d, &[0x8d, 0xbd]),
        (0x28, &[0x8d, 0x5e, b'!']),
        (0x33, &[0x81, 0xfe]),
        (0x3b, &[0x8d, 0x48, (-120i8) as u8]),
        (0x3e, &[0x83, 0xf9, 2]),
        (0x43, &[0x83, 0xe8, 1]),
        (0x48, &[0x83, 0xf8, b'?']),
        (0x4b, &[0x0f, 0x44, 0xc3]),
        (0x5a, &[0x6a, (-3i8) as u8]),
    ] {
        norm[at..at + bytes.len()].copy_from_slice(bytes);
    }
    norm[5..9].copy_from_slice(&68u32.to_le_bytes());
    norm[0x1f..0x23].copy_from_slice(&(-68i32).to_le_bytes());
    norm[0x35..0x39].copy_from_slice(&31u32.to_le_bytes());
    branch(&mut norm, 0x301080, 0x67, 0xe8, 0x301180);
    let normalize = put(&mut image, 0x80, &norm);
    let mut core = vec![0x90; 0x1fb];
    for (at, bytes) in [
        (
            0,
            &[0x55, 0x8b, 0xec, 0x83, 0xec, 8, 0x8b, 0x55, 0x0c, 0xb9][..],
        ),
        (0x14, &[0x8b, 0xd9]),
        (0x1b, &[0x8b, 0x7d, 0x10]),
        (0x1e, &[0x83, 0xfa, 12]),
        (0x10a, &[0x83, 0xc6, 12]),
        (0x110, &[0x83, 0xe8, 12]),
        (0x126, &[0x48, 0x03, 0xfa, 0x83, 0xf8, 10]),
        (0x12e, &[0xff, 0x24, 0x85]),
        (
            0x1f2,
            &[0x8b, 0xc7, 0x5f, 0x5e, 0x5b, 0x8b, 0xe5, 0x5d, 0xc3],
        ),
    ] {
        core[at..at + bytes.len()].copy_from_slice(bytes);
    }
    core[0xa..0xe].copy_from_slice(&0x12345678u32.to_le_bytes());
    core[0x131..0x135].copy_from_slice(&0x301400u32.to_le_bytes());
    for (i, (chunk, final_mix)) in CHUNK_SHIFTS.into_iter().zip(FINAL_SHIFTS).enumerate() {
        let operation = if i % 2 == 0 { 0xe8 } else { 0xe0 };
        let count = (i + 1) as u8;
        core[chunk..chunk + 3].copy_from_slice(&[0xc1, operation, count]);
        core[final_mix..final_mix + 3].copy_from_slice(&[
            0xc1,
            operation
                | if i == 7 {
                    2
                } else if i == 8 {
                    1
                } else {
                    0
                },
            count,
        ]);
    }
    for (i, offset) in FINAL_SHIFTS.into_iter().enumerate() {
        let (prefix, suffix): (&[u8], &[u8]) = match i {
            7 => (&[0x2b, 0xcf, 0x2b, 0xcb, 0x8b, 0xd3], &[0x33, 0xca]),
            8 => (&[0x2b, 0xf9], &[0x2b, 0xfb, 0x33, 0xf9]),
            _ => match i % 3 {
                0 => (&[0x2b, 0xdf, 0x8b, 0xc7, 0x2b, 0xd9], &[0x33, 0xd8]),
                1 => (&[0x2b, 0xcf, 0x2b, 0xcb, 0x8b, 0xc3], &[0x33, 0xc8]),
                _ => (&[0x2b, 0xf9, 0x8b, 0xc1, 0x2b, 0xfb], &[0x33, 0xf8]),
            },
        };
        core[offset - prefix.len()..offset].copy_from_slice(prefix);
        core[offset + 3..offset + 3 + suffix.len()].copy_from_slice(suffix);
    }
    for (i, offset) in CHUNK_SHIFTS.into_iter().enumerate() {
        let (prefix, suffix): (&[u8], &[u8]) = match i {
            0 => (
                &[0x8b, 0xc7],
                &[0x2b, 0xd7, 0x2b, 0xd1, 0x2b, 0xcf, 0x03, 0xda, 0x33, 0xd8],
            ),
            1 => (&[0x2b, 0xcb, 0x8b, 0xc3], &[0x33, 0xc8]),
            3 | 6 => (&[0x2b, 0xdf, 0x8b, 0xc7], &[0x2b, 0xd9, 0x33, 0xd8]),
            4 => (&[0x2b, 0xcf, 0x2b, 0xcb, 0x8b, 0xc3], &[0x33, 0xc8]),
            7 => (&[0x2b, 0xcf, 0x8b, 0xc3, 0x2b, 0xcb], &[0x33, 0xc8]),
            _ => (&[0x2b, 0xf9, 0x8b, 0xc1, 0x2b, 0xfb], &[0x33, 0xf8]),
        };
        core[offset - prefix.len()..offset].copy_from_slice(prefix);
        core[offset + 3..offset + 3 + suffix.len()].copy_from_slice(suffix);
    }
    let mut targets = Vec::new();
    let mut end = 0x191;
    for i in 0..11 {
        let shift = (i % 4 + usize::from(i >= 8)) * 8;
        let mut instructions = if i == 0 {
            vec![0x0f, 0xb6, 6]
        } else {
            vec![0x0f, 0xb6, 0x46, i as u8]
        };
        if shift != 0 {
            instructions.extend([0xc1, 0xe0, shift as u8]);
        }
        instructions.extend([0x03, [0xd8, 0xc8, 0xf8][i / 4]]);
        end -= instructions.len();
        core[end..end + instructions.len()].copy_from_slice(&instructions);
        targets.extend((0x301180 + end as u32).to_le_bytes());
    }
    let core = put(&mut image, 0x180, &core);
    let tails = put(&mut image, 0x400, &targets);
    let binding = Binding {
        origin: AssetIdentity {
            edition: "synthetic".into(),
            version: "hash-v1".into(),
            source: "arbitrary hash profile".into(),
        },
        image_sha256: sha256(&image),
        handler,
        raw,
        normalize,
        core,
        tails,
    };
    (
        image,
        binding,
        RuleInputBudget {
            image_bytes: 0x2000,
            profile_bytes: 2048,
            sections: 2,
        },
    )
}
pub(crate) fn policy() -> Policy {
    let (image, binding, budget) = fixture();
    Policy::read(&image, binding, budget).unwrap()
}

#[test]
fn string_hash_nonstock_policy_uses_source_cap_case_mapping_seed_and_mix() {
    let p = policy();
    assert_eq!(p.hash(b"").unwrap(), 0);
    assert_eq!(p.hash(b"abc\0ignored").unwrap(), p.hash(b"abc").unwrap());
    assert_eq!(p.hash(b"x?").unwrap(), p.hash(b"w!").unwrap());
    assert_ne!(p.hash(b"abc").unwrap(), p.hash(b"ABC").unwrap());
    assert_eq!(p.hash(&[b'a'; 32]).unwrap(), p.hash(&[b'a'; 31]).unwrap());
    assert_ne!(p.hash(&[b'a'; 30]).unwrap(), p.hash(&[b'a'; 31]).unwrap());
    assert_ne!(p.hash(&[0xc3]).unwrap(), p.hash(&[0xa9]).unwrap());
    let cut = format!("{}é", "a".repeat(30));
    let mut prefix = vec![b'a'; 30];
    prefix.push(0xc3);
    assert_eq!(p.hash(cut.as_bytes()).unwrap(), p.hash(&prefix).unwrap());
    assert_eq!(p.origin().source, "arbitrary hash profile");
    // Independent fixed result for arbitrary operands, including the reserved low tail byte.
    assert_eq!(p.hash(b"abc").unwrap(), -1890785412);
}
#[test]
fn string_hash_policy_refuses_identity_budget_bad_join_and_bad_mix() {
    let (mut image, mut b, budget) = fixture();
    let digest = b.image_sha256;
    b.image_sha256 = [0; 32];
    assert!(matches!(
        Policy::read(&image, b.clone(), budget).unwrap_err().problem,
        Problem::Identity
    ));
    b.image_sha256 = digest;
    assert!(matches!(
        Policy::read(
            &image,
            b.clone(),
            RuleInputBudget {
                profile_bytes: 1,
                ..budget
            }
        )
        .unwrap_err()
        .problem,
        Problem::Budget
    ));
    image[0x200 + 0x80 + 0x68] ^= 1;
    b.image_sha256 = sha256(&image);
    b.normalize.digest = sha256(&image[0x280..0x280 + 127]);
    assert!(matches!(
        Policy::read(&image, b, budget).unwrap_err().problem,
        Problem::Shape
    ));
    let (mut image, mut b, budget) = fixture();
    image[0x200 + 0x180 + FINAL_SHIFTS[0] + 2] ^= 1;
    b.image_sha256 = sha256(&image);
    b.core.digest = sha256(&image[0x380..0x380 + 0x1fb]);
    assert!(matches!(
        Policy::read(&image, b, budget).unwrap_err().problem,
        Problem::Shape
    ));
}

#[test]
fn string_hash_policy_refuses_changed_mix_registers_and_tail_arithmetic() {
    for offset in [FINAL_SHIFTS[0] - 6, CHUNK_SHIFTS[0] + 3, 0x18f] {
        let (mut image, mut binding, budget) = fixture();
        image[0x200 + 0x180 + offset] ^= 1;
        binding.image_sha256 = sha256(&image);
        binding.core.digest = sha256(&image[0x380..0x380 + 0x1fb]);
        assert!(
            matches!(
                Policy::read(&image, binding, budget).unwrap_err().problem,
                Problem::Shape
            ),
            "offset {offset:x}"
        );
    }
}

#[test]
fn string_hash_vm_recovers_after_missing_policy_without_resolving_a_table() {
    let world = crate::neutral_player::native_string_world_fixture(None);
    let program = world.context().scripts.shared_program();
    let mut vm = Vm::new(&program, world);
    let arg = Value::Str("TRIGSTR_7".into());
    assert!(
        vm.call("hash", std::slice::from_ref(&arg))
            .unwrap_err()
            .message
            .contains("StringHash source policy is not admitted")
    );
    vm.host()
        .inner_mut()
        .inner_mut()
        .inner_mut()
        .inner_mut()
        .inner_mut()
        .inner_mut()
        .script_hash = Some(policy());
    assert_eq!(
        vm.call("hash", &[arg]).unwrap(),
        Value::Int(policy().hash(b"TRIGSTR_7").unwrap())
    );
    assert_eq!(vm.call("hash", &[Value::Null]).unwrap(), Value::Int(0));
}

#[test]
#[ignore = "requires selected PCX_WAR3_EXE and PCX_STRING_CAPTURE_STATUS local native receipt"]
fn string_hash_selected_image_matches_38_original_expressions_through_vm() {
    use crate::playercolor_host::{EnumHost, Limits, ObjectHost};
    use std::io::Read;
    fn bounded(path: impl AsRef<std::path::Path>, limit: u64) -> Vec<u8> {
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .unwrap()
            .take(limit + 1)
            .read_to_end(&mut bytes)
            .unwrap();
        assert!(bytes.len() as u64 <= limit);
        bytes
    }
    let image = bounded(
        std::env::var_os("PCX_WAR3_EXE").expect("selected image"),
        32 << 20,
    );
    let status: serde_json::Value = serde_json::from_slice(&bounded(
        std::env::var_os("PCX_STRING_CAPTURE_STATUS").expect("native receipt"),
        64 << 10,
    ))
    .unwrap();
    assert_eq!(status["complete"], true);
    assert_eq!(status["result"], "captured");
    assert!(status.get("cleanup_error").is_none());
    assert_eq!(status["isolation"]["verified"], true);
    let hex = |bytes: &[u8]| {
        sha256(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    assert_eq!(status["source_executable_sha256"], hex(&image));
    assert_eq!(sha256(&image), crate::menu_lighting::REVIEWED_IMAGE);
    let raw = bounded(status["trace"].as_str().unwrap(), 512 << 10);
    assert_eq!(status["trace_raw_sha256"], hex(&raw));
    let rows: serde_json::Value =
        serde_json::from_slice(&bounded(status["results"].as_str().unwrap(), 512 << 10)).unwrap();
    let rows = rows.as_array().unwrap();
    assert_eq!(status["records"].as_u64().unwrap(), rows.len() as u64 + 2);
    assert!(rows.len() <= 256);
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
    let binding = Binding {
        origin: AssetIdentity {
            edition: "local Legacy".into(),
            version: "1.29.2.9231".into(),
            source: "explicit native string matrix".into(),
        },
        image_sha256: sha256(&image),
        handler: window(0x4ac5a0, 33, true),
        raw: window(0x469010, 14, true),
        normalize: window(0x433890, 127, true),
        core: window(0x4347f0, 0x1fb, true),
        tails: window(0x4349ec, 44, false),
    };
    let policy = Policy::read(&image, binding, budget).unwrap();
    let mut source = String::from(
        "type agent extends handle\ntype timer extends agent\nnative CreateTimer takes nothing returns timer\nnative DestroyTimer takes timer t returns nothing\nnative StringHash takes string value returns integer\nfunction PCXRepeat takes string text, integer count returns string\nlocal string result = \"\"\nloop\nexitwhen count <= 0\nset result = result + text\nset count = count - 1\nendloop\nreturn result\nendfunction\n",
    );
    let selected: Vec<_> = rows
        .iter()
        .filter(|r| r["label"].as_str().unwrap().starts_with("hash_"))
        .collect();
    assert_eq!(selected.len(), 38);
    for (i, row) in selected.iter().enumerate() {
        let expression = row["expression"].as_str().unwrap();
        assert!(expression.len() <= 512 && !expression.contains(['\n', '\r']));
        source.push_str(&format!("function Case{i} takes nothing returns integer\nreturn StringHash({expression})\nendfunction\n"));
    }
    let program =
        std::sync::Arc::new(pcx_script::resolve(&pcx_script::parse(&source).unwrap()).unwrap());
    let enums = EnumHost::checked_selected(
        program.clone(),
        "native hash matrix",
        &[],
        Limits {
            max_handles: 8,
            admission_steps: 100,
            expression_depth: 64,
            expression_work: 10000,
        },
    )
    .unwrap();
    let object = ObjectHost::checked(enums).unwrap().with_script_hash(policy);
    let mut vm = Vm::new(&program, object);
    for (i, row) in selected.iter().enumerate() {
        assert_eq!(
            vm.call(&format!("Case{i}"), &[]).unwrap(),
            Value::Int(row["hash"].as_i64().unwrap() as i32),
            "{} {}",
            row["label"],
            row["expression"]
        );
    }
    eprintln!("38 original captured StringHash expressions matched through VM/ObjectHost");
}

impl Policy {
    pub(crate) fn synthetic_fixture() -> Self {
        policy()
    }
}
#[test]
fn string_hash_real_selected_world_and_borrowed_owned_localization_forward_raw_once() {
    let expected = policy().hash(b"TRIGSTR_7").unwrap();
    let world = || {
        crate::neutral_player::native_hash_world_fixture(
            Some(b"STRING 7\r\n{\r\nother text\r\n}\r\n"),
            Some(policy()),
        )
    };
    let plain = crate::neutral_player::native_string_world_fixture(None);
    assert!(
        plain
            .string_hash(b"TRIGSTR_7")
            .unwrap_err()
            .contains("StringHash source policy is not admitted")
    );
    let selected = world();
    assert_eq!(selected.string_hash(b"TRIGSTR_7").unwrap(), expected);
    let scripts = selected.context().scripts.clone();
    let mut borrowed = scripts.vm(selected);
    assert_eq!(
        borrowed
            .call("hash", &[Value::Str("TRIGSTR_7".into())])
            .unwrap(),
        Value::Int(expected)
    );
    let selected = world();
    let scripts = selected.context().scripts.clone();
    let mut owned = scripts.owned_vm(selected);
    assert_eq!(
        owned
            .call("hash", &[Value::Str("TRIGSTR_7".into())])
            .unwrap(),
        Value::Int(expected)
    );
}

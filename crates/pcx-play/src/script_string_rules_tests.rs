use super::*;
use crate::playercolor_host::{EnumHost, Limits, ObjectHost};
use pcx_script::{Host, Value};
use std::sync::Arc;
fn origin() -> AssetIdentity {
    AssetIdentity {
        source: "fixture:byte-strings".into(),
        edition: "synthetic".into(),
        version: "nonstock".into(),
    }
}
fn budget() -> RuleInputBudget {
    RuleInputBudget {
        image_bytes: 64 << 10,
        profile_bytes: 16 << 10,
        sections: 8,
    }
}
fn offset(va: u32) -> usize {
    (va - 0x301000) as usize + 0x200
}
fn put(image: &mut [u8], va: u32, bytes: &[u8]) {
    let at = offset(va);
    image[at..at + bytes.len()].copy_from_slice(bytes);
}
fn call(image: &mut [u8], va: u32, target: u32, op: u8) {
    put(image, va, &[op]);
    put(image, va + 1, &target.wrapping_sub(va + 5).to_le_bytes());
}
fn binding(w: [SourceWindow; 15], hash: Sha256Digest) -> Binding {
    let [
        case,
        copy,
        upper_jump,
        lower_jump,
        upper,
        lower,
        substring,
        slice,
        entry,
        startup,
        crt_init,
        callbacks,
        walker,
        locale_init,
        locale_store,
    ] = w;
    Binding {
        origin: origin(),
        image_sha256: hash,
        case,
        copy,
        upper_jump,
        lower_jump,
        upper,
        lower,
        substring,
        slice,
        entry,
        startup,
        crt_init,
        callbacks,
        walker,
        locale_init,
        locale_store,
    }
}
fn windows(b: &mut Binding) -> [&mut SourceWindow; 15] {
    [
        &mut b.case,
        &mut b.copy,
        &mut b.upper_jump,
        &mut b.lower_jump,
        &mut b.upper,
        &mut b.lower,
        &mut b.substring,
        &mut b.slice,
        &mut b.entry,
        &mut b.startup,
        &mut b.crt_init,
        &mut b.callbacks,
        &mut b.walker,
        &mut b.locale_init,
        &mut b.locale_store,
    ]
}
fn seal(image: &[u8], b: &mut Binding) {
    b.image_sha256 = sha256(image);
    for w in windows(b) {
        w.digest = sha256(&image[offset(w.va)..offset(w.va) + w.len as usize]);
    }
}
fn fixture() -> (Vec<u8>, Binding) {
    let mut image = vec![0; 0x3200];
    image[..2].copy_from_slice(b"MZ");
    image[0x80..0x84].copy_from_slice(b"PE\0\0");
    for (at, value) in [
        (0x3c, 0x80u32),
        (0xb4, 0x300000),
        (0xd0, 0x6000),
        (0x100, 0x4000),
        (0x104, 0x1000),
        (0x108, 0x3000),
        (0x10c, 0x200),
        (0x11c, 0x60000020),
    ] {
        image[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    for (at, value) in [(0x84, 0x14cu16), (0x86, 1), (0x94, 0x60), (0x98, 0x10b)] {
        image[at..at + 2].copy_from_slice(&value.to_le_bytes());
    }
    let sizes = [
        0xd5, 0x65, 5, 5, 0x5d, 0x5d, 0x85, 0x125, 10, 0x19, 0x39, 0x12, 0x83, 0x12, 0x2f,
    ];
    let mut b = binding(
        std::array::from_fn(|i| SourceWindow {
            va: 0x301000 + i as u32 * 0x200,
            len: sizes[i],
            digest: [0; 32],
        }),
        [0; 32],
    );
    let c = b.case.va;
    put(&mut image, c, &[0x55, 0x8b, 0xec, 0x81, 0xec]);
    put(&mut image, c + 5, &0x104u32.to_le_bytes());
    put(&mut image, c + 0x10, &[0x89, 0x45, 0xfc]);
    put(&mut image, c + 0x62, &[0x68]);
    put(&mut image, c + 0x63, &4u32.to_le_bytes());
    put(&mut image, c + 0x67, &[0x8d, 0x85]);
    put(&mut image, c + 0x69, &(-0x104i32).to_le_bytes());
    put(&mut image, c + 0x6d, &[0x56, 0x50]);
    for (at, target) in [
        (0x31, 0x301f00),
        (0x6f, b.copy.va),
        (0x84, b.upper_jump.va),
        (0x8b, b.lower_jump.va),
        (0xc0, 0x301f20),
    ] {
        call(&mut image, c + at, target, 0xe8);
    }
    put(
        &mut image,
        c + 0x59,
        &[0x85, 0xf6, 0x74, 0x4a, 0x80, 0x3e, 0, 0x74, 0x45],
    );
    put(&mut image, c + 0x7d, &[0x83, 0x7d, 0x0c, 0, 0x50, 0x74, 7]);
    put(
        &mut image,
        b.copy.va + 0x49,
        &[0x4e, 0x3b, 0xfe, 0x0f, 0x42, 0xf7],
    );
    put(&mut image, b.copy.va + 0x5a, &[0xc6, 0x04, 0x1e, 0]);
    call(&mut image, b.upper_jump.va, b.upper.va, 0xe9);
    call(&mut image, b.lower_jump.va, b.lower.va, 0xe9);
    for (va, low, high, op, delta, table) in [
        (b.upper.va, b'j', b'm', 0xea, 5u8, 0x303300),
        (b.lower.va, b'C', b'E', 0xc2, 7u8, 0x303200),
    ] {
        put(&mut image, va, &[0x8b, 0xff, 0x55, 0x8b, 0xec, 0xa1]);
        put(&mut image, va + 6, &0x304000u32.to_le_bytes());
        put(&mut image, va + 0xa, &[0x85, 0xc0, 0x75, 0x3b]);
        put(
            &mut image,
            va + 0x30,
            &[
                0x8a, 0x11, 0x80, 0xfa, low, 0x7c, 0x0a, 0x80, 0xfa, high, 0x7f, 0x05, 0x80, op,
                delta, 0x88, 0x11, 0x41, 0x80, 0x39, 0, 0x75, 0xe9, 0x5d, 0xc3,
            ],
        );
        let map: [u8; 256] = std::array::from_fn(|i| {
            let byte = i as u8;
            if (low..=high).contains(&byte) {
                if op == 0xc2 {
                    byte.wrapping_add(delta)
                } else {
                    byte.wrapping_sub(delta)
                }
            } else {
                byte
            }
        });
        put(&mut image, table, &map);
    }
    let sub = b.substring.va;
    for (at, target) in [(0x33, b.slice.va), (0x42, 0x301f00), (0x64, 0x301f20)] {
        call(&mut image, sub + at, target, 0xe8);
    }
    let s = b.slice.va;
    put(
        &mut image,
        s + 0x58,
        &[0x66, 0x8b, 0x5d, 0x0c, 0x66, 0x3b, 0xde, 0x7f, 0x7e],
    );
    put(
        &mut image,
        s + 0x61,
        &[
            0x66, 0x39, 0x75, 0x10, 0x0f, 0xb7, 0x45, 0x10, 0x0f, 0xb7, 0xd6, 0x0f, 0x4c, 0xd0,
            0x0f, 0xbf, 0xcb, 0x0f, 0xb7, 0xc2, 0x03, 0xcf, 0x0f, 0xbf, 0xf0,
        ],
    );
    put(
        &mut image,
        s + 0x81,
        &[0x8a, 0x1c, 0x3e, 0xc6, 0x04, 0x3e, 0],
    );
    put(&mut image, s + 0x9d, &[0x51, 0x8d, 0x4d, 0xe4]);
    put(&mut image, s + 0xb8, &[0x88, 0x1c, 0x3e, 0xe8]);
    image[0xa8..0xac].copy_from_slice(&(b.entry.va - 0x300000).to_le_bytes());
    call(&mut image, b.entry.va + 5, b.startup.va, 0xe9);
    call(&mut image, b.startup.va + 0xe, b.crt_init.va, 0xe8);
    call(&mut image, b.crt_init.va + 0x22, b.callbacks.va, 0xe8);
    put(&mut image, b.callbacks.va, &[0x68]);
    put(&mut image, b.callbacks.va + 1, &0x303508u32.to_le_bytes());
    put(&mut image, b.callbacks.va + 5, &[0x68]);
    put(&mut image, b.callbacks.va + 6, &0x303500u32.to_le_bytes());
    call(&mut image, b.callbacks.va + 10, b.walker.va, 0xe8);
    put(&mut image, b.walker.va + 0x21, &[0x8b, 0x1e, 0x85, 0xdb]);
    put(&mut image, b.walker.va + 0x2f, &[0xff, 0xd3]);
    put(&mut image, b.walker.va + 0x35, &[0x83, 0xc6, 8]);
    put(&mut image, 0x303500, &b.locale_init.va.to_le_bytes());
    put(&mut image, b.locale_init.va, &[0x68]);
    put(&mut image, b.locale_init.va + 1, &0x303000u32.to_le_bytes());
    put(&mut image, b.locale_init.va + 5, &[0xb9]);
    put(&mut image, b.locale_init.va + 6, &0x304010u32.to_le_bytes());
    call(&mut image, b.locale_init.va + 10, b.locale_store.va, 0xe8);
    put(
        &mut image,
        b.locale_store.va + 0x1f,
        &[0x8b, 0x55, 8, 0x46, 0x89, 0x11],
    );
    put(&mut image, 0x303094, &0x303200u32.to_le_bytes());
    put(&mut image, 0x303098, &0x303300u32.to_le_bytes());
    seal(&image, &mut b);
    (image, b)
}
fn policy() -> Policy {
    let (image, b) = fixture();
    Policy::read(&image, b, budget()).unwrap()
}
const BASE: &str = "type agent extends handle\ntype timer extends agent\nnative CreateTimer takes nothing returns timer\nnative DestroyTimer takes timer t returns nothing\nnative StringCase takes string text, boolean upper returns string\nnative SubString takes string text, integer start, integer finish returns string\nnative StringLength takes string text returns integer\n";
fn object(program: Arc<pcx_script::Program>) -> ObjectHost {
    ObjectHost::checked(
        EnumHost::checked_selected(
            program,
            "fixture:byte-strings",
            &[],
            Limits {
                max_handles: 8,
                admission_steps: 100,
                expression_depth: 64,
                expression_work: 20000,
            },
        )
        .unwrap(),
    )
    .unwrap()
}
#[test]
fn string_rules_nonstock_case_operands_cap_and_raw_signed_word_branches() {
    let p = policy();
    assert_eq!(
        p.case("jmé".as_bytes(), true).unwrap().as_bytes(),
        b"eh\xc3"
    );
    assert_eq!(p.case(b"CDEz", false).unwrap(), "JKL");
    for (start, end, expected) in [
        (3, 1, Some("def")),
        (1, 1, Some("")),
        (6, 8, Some("")),
        (7, -1, None),
        (65536, 65538, Some("ab")),
        (-65536, -65534, Some("ab")),
        (32767, -1, None),
    ] {
        assert_eq!(
            p.substring(b"abcdef", start, end)
                .unwrap()
                .as_ref()
                .map(|s| s.as_utf8().unwrap()),
            expected
        );
    }
    assert_eq!(
        p.substring("é".as_bytes(), 0, 1)
            .unwrap()
            .unwrap()
            .as_bytes(),
        b"\xc3"
    );
    for (start, end) in [(-1, 2), (0, -1), (-32768, 0)] {
        assert!(
            p.substring(b"abcdef", start, end)
                .unwrap_err()
                .contains("unsafe effective signed-word pointer offsets")
        );
    }
    assert_eq!(p.substring(&vec![b'x'; 32768], 0, 1).unwrap(), None);
    assert_eq!(p.substring(&vec![b'x'; 32768], 32769, 0).unwrap(), None);
    assert_eq!(
        p.substring(&vec![b'x'; 65536], 0, 1).unwrap(),
        Some("".into())
    );
}
#[test]
fn string_rules_case_copy_and_intern_keep_allocated_empty_distinct_from_null() {
    let (mut image, mut b) = fixture();
    put(&mut image, b.case.va + 0x63, &1u32.to_le_bytes());
    seal(&image, &mut b);
    let p = Policy::read(&image, b.clone(), budget()).unwrap();
    assert_eq!(p.case(b"jmx", true).unwrap(), "");
    put(&mut image, b.case.va + 0x63, &4u32.to_le_bytes());
    put(&mut image, b.upper.va + 0x3e, b"j");
    put(&mut image, 0x303300 + u32::from(b'j'), &[0, 1, 2, 3]);
    seal(&image, &mut b);
    let p = Policy::read(&image, b, budget()).unwrap();
    assert_eq!(p.case(b"jmx", true).unwrap().as_bytes(), b"");
    let source = format!(
        "{BASE}function change takes nothing returns string\nreturn StringCase(\"jmx\",true)\nendfunction\n"
    );
    let program = Arc::new(pcx_script::resolve(&pcx_script::parse(&source).unwrap()).unwrap());
    let host = object(program.clone()).with_script_strings(p);
    let mut vm = pcx_script::Vm::new(&program, host);
    assert_eq!(vm.call("change", &[]).unwrap(), Value::Str("".into()));
}

#[test]
fn string_rules_refuse_bad_identity_cap_locale_tables_and_source_joins() {
    let (image, b) = fixture();
    let mut bad = b.clone();
    bad.image_sha256[0] ^= 1;
    assert!(
        Policy::read(&image, bad, budget())
            .unwrap_err()
            .contains("fingerprint")
    );
    for (va, bytes) in [
        (b.case.va + 0x63, 0u32.to_le_bytes().to_vec()),
        (0x303008, 1u32.to_le_bytes().to_vec()),
        (0x3030a8, 1u32.to_le_bytes().to_vec()),
        (0x303300, vec![1]),
        (b.slice.va + 0x5f, vec![0x77]),
        (b.locale_store.va + 0x23, vec![0x88]),
        (b.upper_jump.va + 1, vec![0xff]),
    ] {
        let mut image = image.clone();
        put(&mut image, va, &bytes);
        let mut binding = b.clone();
        seal(&image, &mut binding);
        assert!(Policy::read(&image, binding, budget()).is_err(), "{va:x}");
    }
    for (left, right) in [
        (b.case.va + 0x31, b.substring.va + 0x42),
        (b.case.va + 0xc0, b.substring.va + 0x64),
    ] {
        let mut image = image.clone();
        put(&mut image, left, &[0x90]);
        put(&mut image, right, &[0x90]);
        let mut binding = b.clone();
        seal(&image, &mut binding);
        assert!(Policy::read(&image, binding, budget()).is_err());
    }
    assert!(
        Policy::read(
            &image,
            b,
            RuleInputBudget {
                profile_bytes: 1,
                ..budget()
            }
        )
        .unwrap_err()
        .contains("budget")
    );
}
#[test]
fn string_rules_real_vm_keeps_raw_substrings_and_one_level_case_source_boundaries() {
    let scripts=crate::scripts::MapScripts::compile(origin(),crate::scripts::ScriptAsset{label:"fixture:common",bytes:BASE.as_bytes()},crate::scripts::ScriptAsset{label:"fixture:BJ",bytes:b"// fixture"},crate::scripts::ScriptAsset{label:"fixture:map",bytes:b"function cut takes string text, integer first, integer last returns string\nreturn SubString(text,first,last)\nendfunction\nfunction change takes string text returns string\nreturn StringCase(text,true)\nendfunction\n"},Some(crate::scripts::ScriptAsset{label:"fixture:WTS",bytes:b"STRING 7\r\n{\r\njmx\r\n}\r\nSTRING 8\r\n{\r\nTRIGSTR_7\r\n}\r\n"})).unwrap();
    let scripts = Arc::new(scripts);
    let mut vm = scripts
        .clone()
        .owned_vm(object(scripts.shared_program()).with_script_strings(policy()));
    vm.init_globals().unwrap();
    assert_eq!(
        vm.call("change", &[Value::Str("TRIGSTR_7".into())])
            .unwrap(),
        Value::Str("ehx".into())
    );
    assert_eq!(
        vm.call("change", &[Value::Str("TRIGSTR_8".into())])
            .unwrap(),
        Value::Str("TRI".into())
    );
    assert_eq!(
        vm.call("change", &[Value::Str("TRIGSTR_9".into())])
            .unwrap(),
        Value::Null
    );
    assert_eq!(
        vm.call(
            "cut",
            &[Value::Str("TRIGSTR_7".into()), Value::Int(0), Value::Int(8)]
        )
        .unwrap(),
        Value::Str("TRIGSTR_".into())
    );
    assert!(
        vm.call(
            "cut",
            &[Value::Str("abcdef".into()), Value::Int(0), Value::Int(-1)]
        )
        .unwrap_err()
        .message
        .contains("unsafe effective")
    );
    assert_eq!(
        vm.call(
            "cut",
            &[Value::Str("é".into()), Value::Int(0), Value::Int(1)]
        )
        .unwrap(),
        Value::Str(vec![0xc3].into())
    );
    let mut borrowed = scripts.vm(object(scripts.shared_program()).with_script_strings(policy()));
    assert_eq!(
        borrowed
            .call("change", &[Value::Str("TRIGSTR_7".into())])
            .unwrap(),
        Value::Str("ehx".into())
    );
}

fn local_bytes(path: impl AsRef<std::path::Path>, limit: u64) -> Vec<u8> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .unwrap()
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .unwrap();
    assert!(bytes.len() as u64 <= limit);
    bytes
}
#[test]
#[ignore = "requires selected PCX_WAR3_EXE and PCX_STRING_CAPTURE_STATUS original receipt"]
fn string_rules_selected_image_matches_39_bounded_original_case_substring_observations_through_vm()
{
    let image = local_bytes(
        std::env::var_os("PCX_WAR3_EXE").expect("selected image"),
        32 << 20,
    );
    let status: serde_json::Value = serde_json::from_slice(&local_bytes(
        std::env::var_os("PCX_STRING_CAPTURE_STATUS").expect("native receipt"),
        64 << 10,
    ))
    .unwrap();
    let hex = |bytes: &[u8]| {
        sha256(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    assert_eq!(sha256(&image), crate::menu_lighting::REVIEWED_IMAGE);
    assert_eq!(status["source_executable_sha256"], hex(&image));
    assert_eq!(status["complete"], true);
    assert_eq!(status["result"], "captured");
    assert_eq!(status["isolation"]["verified"], true);
    assert!(status.get("cleanup_error").is_none());
    let raw = local_bytes(status["trace"].as_str().unwrap(), 512 << 10);
    assert_eq!(status["trace_raw_sha256"], hex(&raw));
    let rows: serde_json::Value =
        serde_json::from_slice(&local_bytes(status["results"].as_str().unwrap(), 512 << 10))
            .unwrap();
    let rows = rows.as_array().unwrap();
    assert!(rows.len() <= 256);
    assert_eq!(status["records"].as_u64().unwrap(), rows.len() as u64 + 2);
    let manifest: serde_json::Value = serde_json::from_slice(&local_bytes(
        status["manifest"].as_str().unwrap(),
        128 << 10,
    ))
    .unwrap();
    let trace_text = std::str::from_utf8(&raw).unwrap();
    let nonce = manifest["nonce"].as_str().unwrap();
    assert!(!nonce.is_empty() && nonce.bytes().all(|b| b.is_ascii_digit()));
    assert_eq!(
        trace_text
            .lines()
            .filter(|line| line.contains("PCX_STRING "))
            .count(),
        rows.len() + 2
    );
    assert_eq!(
        manifest["arms"][0]["cases"].as_array().unwrap().len(),
        rows.len()
    );
    for boundary in ["begin", "end"] {
        assert_eq!(
            trace_text
                .matches(&format!(
                    "PCX_STRING {nonce} matrix {boundary} cases={}",
                    rows.len()
                ))
                .count(),
            1
        );
    }
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(row["index"].as_u64().unwrap(), index as u64);
        let bit = |name| u8::from(row[name].as_bool().unwrap());
        let marker = format!(
            "PCX_STRING {nonce} matrix case={} label={} hash={} length={} null={} empty={} eq={}",
            row["index"].as_u64().unwrap(),
            row["label"].as_str().unwrap(),
            row["hash"].as_i64().unwrap(),
            row["length"].as_u64().unwrap(),
            bit("null"),
            bit("empty"),
            bit("equal_expected")
        );
        assert_eq!(trace_text.matches(&marker).count(), 1, "{marker}");
        let authored = &manifest["arms"][0]["cases"][row["index"].as_u64().unwrap() as usize];
        assert_eq!(authored["expression"], row["expression"]);
        assert_eq!(authored["expected"], row["expected"]);
    }
    let source_map = local_bytes(manifest["source"]["path"].as_str().unwrap(), 32 << 20);
    assert_eq!(manifest["source"]["sha256"], hex(&source_map));
    let selected_origin = AssetIdentity {
        source: "explicit native Case/SubString matrix".into(),
        edition: "local Legacy".into(),
        version: "1.29.2.9231".into(),
    };
    let conditions = &manifest["conditions"];
    let wts_id = conditions["wts_id"].as_u64().unwrap();
    let wts_body = conditions["wts_body"].as_str().unwrap();
    assert!(wts_body.len() <= 4096);
    let mut source = String::from(
        "native StringHash takes string text returns integer\nfunction PCXRepeat takes string text, integer count returns string\nlocal string result = \"\"\nloop\nexitwhen count <= 0\nset result = result + text\nset count = count - 1\nendloop\nreturn result\nendfunction\n",
    );
    let selected: Vec<_> = rows
        .iter()
        .filter(|row| {
            let label = row["label"].as_str().unwrap();
            (label.starts_with("substring_") && !label.starts_with("substring_len"))
                || label.starts_with("case_")
        })
        .collect();
    assert_eq!(selected.len(), 39);
    for (i, row) in selected.iter().enumerate() {
        let expression = row["expression"].as_str().unwrap();
        let expected = row["expected"].as_str().unwrap();
        assert!(
            expression.len() <= 512
                && expected.len() <= 512
                && !expression.contains(['\r', '\n'])
                && !expected.contains(['\r', '\n'])
        );
        source.push_str(&format!("function Raw{i} takes nothing returns string\nreturn {expression}\nendfunction\nfunction Hash{i} takes nothing returns integer\nreturn StringHash({expression})\nendfunction\nfunction Len{i} takes nothing returns integer\nreturn StringLength({expression})\nendfunction\nfunction Eq{i} takes nothing returns boolean\nreturn ({expression}) == ({expected})\nendfunction\n"));
    }
    let wts = format!("STRING {wts_id}\r\n{{\r\n{wts_body}\r\n}}\r\n");
    let scripts = Arc::new(
        crate::scripts::MapScripts::compile(
            origin(),
            crate::scripts::ScriptAsset {
                label: "selected:common-contracts",
                bytes: BASE.as_bytes(),
            },
            crate::scripts::ScriptAsset {
                label: "fixture:BJ",
                bytes: b"// fixture",
            },
            crate::scripts::ScriptAsset {
                label: "captured:expressions",
                bytes: source.as_bytes(),
            },
            Some(crate::scripts::ScriptAsset {
                label: "selected:probe-WTS",
                bytes: wts.as_bytes(),
            }),
        )
        .unwrap(),
    );
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
    let hash = crate::playercolor_host::ScriptHashPolicy::read(
        &image,
        crate::playercolor_host::ScriptHashBinding {
            origin: selected_origin.clone(),
            image_sha256: sha256(&image),
            handler: window(0x4ac5a0, 33, true),
            raw: window(0x469010, 14, true),
            normalize: window(0x433890, 127, true),
            core: window(0x4347f0, 0x1fb, true),
            tails: window(0x4349ec, 44, false),
        },
        budget,
    )
    .unwrap();
    let policy = crate::campaign_client::string_rules(&image, selected_origin).unwrap();
    let host = object(scripts.shared_program())
        .with_script_strings(policy)
        .with_script_hash(hash);
    let mut vm = scripts.clone().owned_vm(host);
    vm.init_globals().unwrap();
    for (i, row) in selected.iter().enumerate() {
        let value = vm.call(&format!("Raw{i}"), &[]).unwrap();
        let label = row["label"].as_str().unwrap();
        assert_eq!(
            value == Value::Null,
            row["null"].as_bool().unwrap(),
            "{label}"
        );
        assert_eq!(
            value == Value::Str("".into()),
            row["empty"].as_bool().unwrap(),
            "{label}"
        );
        assert_eq!(
            vm.call(&format!("Hash{i}"), &[]).unwrap(),
            Value::Int(row["hash"].as_i64().unwrap() as i32),
            "{label}"
        );
        // This is a fresh native length demand, not a claim about a returned token's raw length.
        assert_eq!(
            vm.call(&format!("Len{i}"), &[]).unwrap(),
            Value::Int(row["length"].as_i64().unwrap() as i32),
            "{label}"
        );
        assert_eq!(
            vm.call(&format!("Eq{i}"), &[]).unwrap(),
            Value::Bool(row["equal_expected"].as_bool().unwrap()),
            "{label}"
        );
        if label == "substring_utf8lead" {
            assert_eq!(value, Value::Str(vec![0xc3].into()));
        }
        if label == "substring_utf8tail" {
            assert_eq!(value, Value::Str(vec![0xa9].into()));
        }
        if label == "case_split_utf8" {
            let Value::Str(bytes) = value else {
                panic!("allocated case bytes")
            };
            assert!(bytes.as_utf8().is_err());
            assert_eq!(bytes.as_bytes().last(), Some(&0xc3));
        }
    }
    eprintln!(
        "39 bounded original Case/SubString observations matched through VM/ObjectHost; four unqualified large producers excluded"
    );
}

impl Policy {
    pub(crate) fn synthetic_fixture() -> Self {
        policy()
    }
}
#[test]
fn string_rules_selected_neutral_decorators_and_borrowed_owned_sessions_forward() {
    let world = || {
        crate::neutral_player::native_case_world_fixture(
            Some(b"STRING 7\r\n{\r\njmx\r\n}\r\n"),
            policy(),
        )
    };
    let selected = world();
    assert_eq!(selected.string_case(b"jm", true).unwrap(), "eh");
    assert_eq!(
        selected
            .substring(&[0xff, 0xc3], 0, 1)
            .unwrap()
            .unwrap()
            .as_bytes(),
        &[0xff]
    );
    let scripts = selected.context().scripts.clone();
    let mut borrowed = scripts.vm(selected);
    assert_eq!(
        borrowed
            .call("change", &[Value::Str("TRIGSTR_7".into())])
            .unwrap(),
        Value::Str("ehx".into())
    );
    let mut owned = scripts.clone().owned_vm(world());
    owned.init_globals().unwrap();
    assert_eq!(
        owned
            .call("cut", &[Value::Str(vec![0xff, 0xc3].into())])
            .unwrap(),
        Value::Str(vec![0xff].into())
    );
}

#[test]
fn string_rules_native_bytes_survive_registered_callback_park_and_checked_consumer_refusal() {
    struct Carry {
        inner: ObjectHost,
        callback: Option<pcx_script::value::FunctionId>,
    }
    impl Host for Carry {
        fn native(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
            if let ("Register", [Value::Code(id)]) = (name, args) {
                self.callback = Some(*id);
                Ok(Value::Null)
            } else {
                self.inner.native(name, args)
            }
        }
        fn string_case(&self, bytes: &[u8], upper: bool) -> Result<ScriptString, String> {
            self.inner.string_case(bytes, upper)
        }
        fn substring(
            &self,
            bytes: &[u8],
            first: i32,
            last: i32,
        ) -> Result<Option<ScriptString>, String> {
            self.inner.substring(bytes, first, last)
        }
        // Fixture suspension exercises VM retention; it does not model native sleep timing.
        fn suspend(&mut self, name: &str, _: &[Value]) -> bool {
            name == "Pause"
        }
        fn park_thread(&mut self) -> Result<u64, String> {
            Ok(1)
        }
        fn resume_thread(&mut self, _: u64) -> Result<(), String> {
            Ok(())
        }
        fn leave_thread(&mut self) -> Result<(), String> {
            Ok(())
        }
    }
    let source = format!(
        "{BASE}native Register takes code cb returns nothing\nnative Pause takes nothing returns nothing\nnative SetCineFilterTexture takes string path returns nothing\nglobals\nstring value=null\nstring array pending\nendglobals\nfunction callback takes nothing returns nothing\nset pending[1]=SubString(\"é\",0,1)\ncall Pause()\nset value=pending[1]+SubString(\"é\",1,2)\nendfunction\nfunction seed takes nothing returns nothing\ncall Register(function callback)\nendfunction\nfunction consume takes nothing returns nothing\ncall SetCineFilterTexture(pending[1])\nendfunction\nfunction recover takes nothing returns nothing\ncall SetCineFilterTexture(value)\nendfunction\n"
    );
    let program = Arc::new(pcx_script::resolve(&pcx_script::parse(&source).unwrap()).unwrap());
    let mut host = object(program.clone()).with_script_strings(policy());
    host.admit_presentation(64).unwrap();
    let mut vm = pcx_script::Vm::new(
        &program,
        Carry {
            inner: host,
            callback: None,
        },
    );
    vm.init_globals().unwrap();
    vm.call("seed", &[]).unwrap();
    let callback = vm.host().callback.unwrap();
    vm.run_thread(&[callback]).unwrap();
    assert_eq!(vm.global_at("pending", 1), Value::Str(vec![0xc3].into()));
    assert_eq!(vm.parked_threads().collect::<Vec<_>>(), [1]);
    assert!(
        vm.call("consume", &[])
            .unwrap_err()
            .message
            .contains("UTF-8")
    );
    assert_eq!(vm.parked_threads().collect::<Vec<_>>(), [1]);
    vm.resume_thread(1).unwrap();
    assert_eq!(vm.global("value"), Value::Str("é".into()));
    vm.call("recover", &[]).unwrap();
    assert_eq!(vm.parked_threads().count(), 0);
}

#[test]
fn string_rules_case_distinguishes_missing_wts_source_entry_and_bad_body_and_recovers() {
    for (index, wts) in [
        None,
        Some(&b"STRING 8\r\n{\r\njmx\r\n}\r\n"[..]),
        Some(&b"STRING 7\r\n{\r\n\xff\r\n}\r\n"[..]),
    ]
    .into_iter()
    .enumerate()
    {
        let world = crate::neutral_player::native_case_world_fixture(wts, policy());
        let program = world.context().scripts.shared_program();
        let mut vm = pcx_script::Vm::new(&program, world);
        let result = vm.call("change", &[Value::Str("TRIGSTR_7".into())]);
        match index {
            0 => assert!(
                result
                    .unwrap_err()
                    .message
                    .contains("source was not supplied")
            ),
            1 => assert_eq!(result.unwrap(), Value::Null),
            2 => assert!(result.unwrap_err().message.contains("entry 7 is not UTF-8")),
            _ => unreachable!(),
        }
        assert_eq!(
            vm.call("cut", &[Value::Str("TRIGSTR_7".into())]).unwrap(),
            Value::Str("T".into())
        );
        assert_eq!(
            vm.call("change", &[Value::Str("jm".into())]).unwrap(),
            Value::Str("eh".into())
        );
        assert_eq!(vm.call("change", &[Value::Null]).unwrap(), Value::Null);
    }
}

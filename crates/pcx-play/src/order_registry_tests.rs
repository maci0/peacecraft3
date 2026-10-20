use super::*;
use crate::playercolor_host::{EnumHost, Limits, ObjectHost};
use pcx_script::{Host, Prelude, Value};
use std::sync::Arc;

pub(crate) fn fixture(ids: [i32; 2]) -> (Vec<u8>, Binding, RuleInputBudget) {
    let mut image = vec![0; 0x800];
    for (at, value) in [
        (0x3c, 0x80u32),
        (0xb4, 0x300000),
        (0xd0, 0x2000),
        (0x100, 0x600),
        (0x104, 0x1000),
        (0x108, 0x600),
        (0x10c, 0x200),
        (0x11c, 0x60000020),
    ] {
        image[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    image[..2].copy_from_slice(b"MZ");
    image[0x80..0x84].copy_from_slice(b"PE\0\0");
    for (at, value) in [(0x84, 0x14cu16), (0x86, 1), (0x94, 0x60), (0x98, 0x10b)] {
        image[at..at + 2].copy_from_slice(&value.to_le_bytes());
    }
    let va = 0x301020;
    let register = 0x301100;
    let mut code = vec![0x56, 0x8b, 0x35, 0, 0, 0x40, 0, 0x8b, 0xce];
    for (at, (name, id)) in [
        (0x301200u32, ids[0]),
        (0x301300, ids[1]),
        (0x301200, ids[0]),
    ]
    .into_iter()
    .enumerate()
    {
        code.push(0x68);
        code.extend_from_slice(&name.to_le_bytes());
        code.push(0x68);
        code.extend_from_slice(&id.to_le_bytes());
        if at != 0 {
            code.extend_from_slice(&[0x8b, 0xce]);
        }
        code.push(0xe8);
        let next = va + code.len() as u32 + 4;
        code.extend_from_slice(&(register as i32 - next as i32).to_le_bytes());
    }
    code.extend_from_slice(&[0x5e, 0xc3]);
    image[0x220..0x220 + code.len()].copy_from_slice(&code);
    image[0x300] = 0xc3;
    image[0x400..0x406].copy_from_slice(b"alpha\0");
    image[0x500..0x505].copy_from_slice(b"beta\0");
    let mut bootstrap = vec![0x68];
    bootstrap.extend_from_slice(&0x301400u32.to_le_bytes());
    bootstrap.extend_from_slice(&[0xff, 0x35]);
    bootstrap.extend_from_slice(&0x301350u32.to_le_bytes());
    for target in [0x301100i32, va as i32] {
        bootstrap.push(0xe8);
        let next = 0x301080 + bootstrap.len() as i32 + 4;
        bootstrap.extend_from_slice(&(target - next).to_le_bytes());
    }
    image[0x280..0x295].copy_from_slice(&bootstrap);
    image[0x550..0x554].copy_from_slice(&(ids[0] + 1200).to_le_bytes());
    image[0x600..0x606].copy_from_slice(b"gamma\0");
    let binding = Binding {
        origin: AssetIdentity {
            edition: "synthetic".into(),
            version: "test".into(),
            source: "synthetic:orders".into(),
        },
        image_sha256: sha256(&image),
        initializer: SourceWindow {
            va,
            len: code.len() as u32,
            digest: sha256(&code),
        },
        bootstrap: SourceWindow {
            va: 0x301080,
            len: 21,
            digest: sha256(&bootstrap),
        },
        direct_register: register,
        register,
    };
    (
        image,
        binding,
        RuleInputBudget {
            image_bytes: 4096,
            profile_bytes: 1024,
            sections: 2,
        },
    )
}

fn seal(image: &[u8], binding: &mut Binding) {
    binding.image_sha256 = sha256(image);
    binding.bootstrap.digest = sha256(&image[0x280..0x280 + binding.bootstrap.len as usize]);
    binding.initializer.digest = sha256(&image[0x220..0x220 + binding.initializer.len as usize]);
}

fn registry() -> Arc<Registry> {
    let (image, binding, budget) = fixture([431, 744]);
    Arc::new(Registry::read(&image, binding, budget, 4, 32).unwrap())
}

#[test]
fn identifiers_are_decoded_and_identical_duplicate_registration_is_allowed() {
    for ids in [[431, 744], [19, 6], [-83, 999]] {
        let (image, binding, budget) = fixture(ids);
        let r = Registry::read(&image, binding, budget, 4, 32).unwrap();
        assert_eq!(r.id("ALPHA"), Some(ids[0]));
        assert_eq!(r.id("alpha\0ignored"), Some(ids[0]));
        assert_eq!(r.id("beta"), Some(ids[1]));
        assert_eq!(r.id("gamma"), Some(ids[0] + 1200));
        assert_eq!(r.name(ids[1]), Some("beta"));
        assert_eq!(r.id("not_in_this_source"), None);
        assert_eq!(r.name(123456), None);
        assert_eq!(r.binding().origin.source, "synthetic:orders");
    }
}

#[test]
fn malformed_records_strings_and_conflicting_names_refuse_after_rebinding() {
    for (offset, value) in [
        (0x220, 0x90),
        (0x280, 0x90),
        (0x280 + 12, 0x90),
        (0x28a, 0xff),
        (0x220 + 20, 0x90),
        (0x220 + 24 + 13, 0x90),
        (0x400, 0xff),
        (0x400, 0),
    ] {
        let (mut image, mut binding, budget) = fixture([431, 744]);
        image[offset] = value;
        seal(&image, &mut binding);
        assert!(
            Registry::read(&image, binding, budget, 4, 32).is_err(),
            "offset {offset:#x}"
        );
    }
    let (image, binding, budget) = fixture([431, 431]);
    assert!(Registry::read(&image, binding, budget, 4, 32).is_err());
    let (mut image, mut binding, budget) = fixture([431, 744]);
    image[0x400..0x420].fill(b'x');
    seal(&image, &mut binding);
    assert!(Registry::read(&image, binding, budget, 4, 32).is_err());
}

#[test]
fn fingerprint_section_and_work_limits_are_enforced() {
    let (image, binding, budget) = fixture([431, 744]);
    for (count, text) in [(0, 32), (2, 32), (3, 32), (3, 0), (3, 5), (3, 2048)] {
        assert!(Registry::read(&image, binding.clone(), budget, count, text).is_err());
    }
    let mut bad = binding.clone();
    bad.register += 1;
    assert!(Registry::read(&image, bad, budget, 4, 32).is_err());
    let mut bad = binding.clone();
    bad.image_sha256[0] ^= 1;
    assert!(Registry::read(&image, bad, budget, 4, 32).is_err());
    let mut bad = image.clone();
    bad[0x11c..0x120].copy_from_slice(&0x60000080u32.to_le_bytes());
    let mut b = binding.clone();
    seal(&bad, &mut b);
    assert!(Registry::read(&bad, b, budget, 4, 32).is_err());
    for len in 0..binding.initializer.len {
        let mut b = binding.clone();
        b.initializer.len = len;
        seal(&image, &mut b);
        assert!(Registry::read(&image, b, budget, 4, 32).is_err());
    }
    for len in 0..binding.bootstrap.len {
        let mut b = binding.clone();
        b.bootstrap.len = len;
        seal(&image, &mut b);
        assert!(Registry::read(&image, b, budget, 4, 32).is_err());
    }
}

const COMMON: &str = "type agent extends handle\ntype timer extends agent\ntype playercolor extends handle\nconstant native ConvertPlayerColor takes integer i returns playercolor\nnative CreateTimer takes nothing returns timer\nnative DestroyTimer takes timer t returns nothing\nconstant native OrderId takes string text returns integer\nconstant native OrderId2String takes integer id returns string\nglobals\nconstant playercolor COLOR = ConvertPlayerColor(37)\nendglobals\n";
fn host(common: &str) -> ObjectHost {
    host_sources(common, "")
}
fn host_sources(common: &str, foreign: &str) -> ObjectHost {
    let c = pcx_script::parse_script(common).unwrap();
    let map = pcx_script::parse_script("").unwrap();
    let foreign = pcx_script::parse_script(foreign).unwrap();
    let p = Arc::new(
        pcx_script::resolve_with(
            &map,
            &[
                Prelude::new("synthetic:common", &c),
                Prelude::new("synthetic:foreign", &foreign),
            ],
        )
        .unwrap(),
    );
    let inner = EnumHost::checked_selected(
        p,
        "synthetic:orders",
        &["ConvertPlayerColor"],
        Limits {
            max_handles: 8,
            admission_steps: 100,
            expression_depth: 16,
            expression_work: 100,
        },
    )
    .unwrap();
    ObjectHost::checked(inner).unwrap()
}

#[test]
fn host_lookup_requires_admission_and_preserves_null_and_missing_results() {
    let (image, binding, budget) = fixture([-1, 744]);
    let mut reserved = host(COMMON)
        .with_order_registry(Arc::new(
            Registry::read(&image, binding, budget, 4, 32).unwrap(),
        ))
        .unwrap();
    assert_eq!(
        reserved
            .native("OrderId", &[Value::Str("alpha".into())])
            .unwrap(),
        Value::Int(0)
    );
    assert!(
        host(COMMON)
            .native("OrderId", &[Value::Str("alpha".into())])
            .is_err()
    );
    let mut h = host(COMMON).with_order_registry(registry()).unwrap();
    assert_eq!(
        h.native("OrderId", &[Value::Str("ALPHA".into())]).unwrap(),
        Value::Int(431)
    );
    assert_eq!(
        h.native("OrderId", &[Value::Str("missing".into())])
            .unwrap(),
        Value::Int(0)
    );
    assert_eq!(h.native("OrderId", &[Value::Null]).unwrap(), Value::Int(0));
    assert_eq!(
        h.native("OrderId2String", &[Value::Int(744)]).unwrap(),
        Value::Str("beta".into())
    );
    assert_eq!(
        h.native("OrderId2String", &[Value::Int(0)]).unwrap(),
        Value::Null
    );
    for name in ["OrderId", "OrderId2String"] {
        assert!(h.native(name, &[]).is_err());
        assert!(h.native(name, &[Value::Bool(true)]).is_err());
    }
    assert!(
        h.native(
            "IssueImmediateOrder",
            &[Value::Null, Value::Str("alpha".into())]
        )
        .is_err()
    );
    assert!(h.with_order_registry(registry()).is_err());
}

#[test]
fn lookup_contracts_cannot_be_omitted_or_changed() {
    let inverse = "constant native OrderId2String takes integer id returns string\n";
    assert!(
        host_sources(&COMMON.replace(inverse, ""), inverse)
            .with_order_registry(registry())
            .is_err()
    );
    for common in [
        COMMON.replace("constant native OrderId takes", "native OrderId takes"),
        COMMON.replace(
            "constant native OrderId2String takes integer id returns string\n",
            "",
        ),
    ] {
        assert!(host(&common).with_order_registry(registry()).is_err());
    }
}

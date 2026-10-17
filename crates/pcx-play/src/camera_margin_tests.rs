use super::*;
use crate::playercolor_host::{EnumHost, Limits, ObjectHost};
use pcx_script::{Host, Prelude, Value};
use std::sync::Arc;
fn origin(source: &str) -> AssetIdentity {
    AssetIdentity {
        edition: "synthetic".into(),
        version: "test".into(),
        source: source.into(),
    }
}
fn input(scale: f32) -> (Vec<u8>, CameraMarginBinding, CameraMarginBudget) {
    let mut image = vec![0; 0x400];
    for (at, value) in [
        (0x3c, 0x80u32),
        (0xb4, 0x300000),
        (0xd0, 0x2000),
        (0x100, 0x200),
        (0x104, 0x1000),
        (0x108, 0x200),
        (0x10c, 0x200),
        (0x11c, 0x40000040),
    ] {
        image[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    image[..2].copy_from_slice(b"MZ");
    image[0x80..0x84].copy_from_slice(b"PE\0\0");
    for (at, value) in [(0x84, 0x14cu16), (0x86, 1), (0x94, 0x60), (0x98, 0x10b)] {
        image[at..at + 2].copy_from_slice(&value.to_le_bytes());
    }
    let mut write = |at: usize, bytes: &[u8]| {
        image[at..at + bytes.len()].copy_from_slice(bytes);
        SourceWindow {
            va: 0x301000 + (at - 0x200) as u32,
            len: bytes.len() as u32,
            digest: sha256(bytes),
        }
    };
    let section = write(0x200, b"SyntheticMargins\0");
    // Deliberately shuffled names/order and scale, unrelated to measured game values.
    let keys = [
        write(0x220, b"z\0"),
        write(0x230, b"a\0"),
        write(0x240, b"q\0"),
        write(0x250, b"b\0"),
    ];
    let scale = write(0x260, &scale.to_le_bytes());
    let binding = CameraMarginBinding {
        origin: origin("synthetic:PE"),
        image_sha256: sha256(&image),
        section,
        keys,
        scale,
    };
    (
        image,
        binding,
        CameraMarginBudget {
            image: RuleInputBudget {
                image_bytes: 4096,
                profile_bytes: 200,
                sections: 2,
            },
            config_bytes: 4096,
        },
    )
}
const CONFIG: &[u8] = b"[Unrelated]\nz=500\n[SyntheticMargins]\na=-7\nz=19\nb=3\nq=0\n";
fn margins() -> CameraMargins {
    let (image, binding, budget) = input(2.5);
    CameraMargins::read(&image, binding, CONFIG, origin("synthetic:config"), budget).unwrap()
}
#[test]
fn selected_names_order_integer_conversion_and_scale_are_read_from_sources() {
    for scale in [0.125, 2.5, 37.0] {
        let (image, binding, budget) = input(scale);
        let m = CameraMargins::read(&image, binding, CONFIG, origin("synthetic:config"), budget)
            .unwrap();
        assert_eq!(*m.values(), [19.0 * scale, -7.0 * scale, 0.0, 3.0 * scale]);
        assert_eq!(m.config_sha256(), sha256(CONFIG));
        assert_eq!(m.binding().image_sha256, sha256(&image));
        assert_eq!(m.config_origin().source, "synthetic:config");
    }
}
#[test]
fn strict_selected_section_refuses_ambiguity_missing_and_malformed_values() {
    for text in [
        "",
        "[SyntheticMargins]\nz=1\na=2\nq=3\n",
        "[SyntheticMargins]\nz=1\na=2\nq=3\nb=4\nz=5",
        "[SyntheticMargins]\nz=1\na=2\nq=3\nb=4\n[SYNTHETICMARGINS]",
        "[SyntheticMargins]\nz=1\na=2\nq=3\nb=4.5",
        "[SyntheticMargins]\nz=2147483648\na=2\nq=3\nb=4",
        "[SyntheticMargins]\nz=1\na=2\nq=3\nb=4\nbroken",
    ] {
        let (image, binding, budget) = input(2.5);
        let e = CameraMargins::read(
            &image,
            binding,
            text.as_bytes(),
            origin("synthetic:bad"),
            budget,
        )
        .unwrap_err();
        assert!(e.to_string().contains("synthetic:bad"));
    }
    let (image, binding, budget) = input(2.5);
    let m = CameraMargins::read(
        &image,
        binding,
        "\u{feff}[ syntheticmargins ]\r\nZ=1 // note\na=2\nq=3\nb=4\n; comment".as_bytes(),
        origin("synthetic:config"),
        budget,
    )
    .unwrap();
    assert_eq!(*m.values(), [2.5, 5.0, 7.5, 10.0]);
}
#[test]
fn image_fingerprints_initialized_ranges_and_source_identity_are_checked() {
    let (mut image, mut binding, budget) = input(2.5);
    image[0x300] ^= 1;
    assert!(matches!(
        CameraMargins::read(&image, binding.clone(), CONFIG, origin("cfg"), budget)
            .unwrap_err()
            .problem,
        Problem::Identity
    ));
    binding.image_sha256 = sha256(&image);
    binding.scale.digest[0] ^= 1;
    assert!(matches!(
        CameraMargins::read(&image, binding.clone(), CONFIG, origin("cfg"), budget)
            .unwrap_err()
            .problem,
        Problem::Image(_)
    ));
    binding.scale.digest[0] ^= 1;
    image[0x100..0x104].copy_from_slice(&0x300u32.to_le_bytes());
    binding.image_sha256 = sha256(&image);
    binding.scale.va = 0x301250;
    assert!(CameraMargins::read(&image, binding, CONFIG, origin("cfg"), budget).is_err());
    let (image, binding, budget) = input(2.5);
    assert!(CameraMargins::read(&image, binding, CONFIG, origin(""), budget).is_err());
}
#[test]
fn budgets_are_checked_before_pe_parsing_and_input_processing() {
    for which in 0..4 {
        let (image, binding, mut budget) = input(2.5);
        match which {
            0 => budget.config_bytes = CONFIG.len() - 1,
            1 => budget.image.image_bytes = image.len() - 1,
            2 => budget.image.profile_bytes = 1,
            _ => budget.image.sections = 0,
        }
        assert!(CameraMargins::read(&image, binding, CONFIG, origin("cfg"), budget).is_err());
    }
}
#[test]
fn bad_source_selections_nonfinite_scale_and_overflow_refuse() {
    for scale in [f32::NAN, f32::INFINITY, -1.0, 0.0, f32::MAX] {
        let (image, binding, budget) = input(scale);
        assert!(CameraMargins::read(&image, binding, CONFIG, origin("cfg"), budget).is_err());
    }
    let (image, mut binding, budget) = input(2.5);
    binding.keys[1] = binding.keys[0];
    assert!(CameraMargins::read(&image, binding, CONFIG, origin("cfg"), budget).is_err());
    let (image, mut binding, budget) = input(2.5);
    binding.scale.len = 3;
    binding.scale.digest = sha256(&image[0x260..0x263]);
    assert!(CameraMargins::read(&image, binding, CONFIG, origin("cfg"), budget).is_err());
}
const COMMON: &str = "type agent extends handle\ntype timer extends agent\ntype playercolor extends handle\nconstant native ConvertPlayerColor takes integer i returns playercolor\nnative CreateTimer takes nothing returns timer\nnative DestroyTimer takes timer t returns nothing\nnative GetCameraMargin takes integer i returns real\nnative SetCameraBounds takes real a, real b, real c, real d, real e, real f, real g, real h returns nothing\nglobals\nconstant playercolor C = ConvertPlayerColor(37)\nendglobals\n";
fn host(common: &str, map: &str) -> ObjectHost {
    let common = pcx_script::parse_script(common).unwrap();
    let map = pcx_script::parse_script(map).unwrap();
    let p = Arc::new(
        pcx_script::resolve_with(&map, &[Prelude::new("synthetic:common", &common)]).unwrap(),
    );
    let e = EnumHost::checked_selected(
        p,
        "synthetic:camera",
        &["ConvertPlayerColor"],
        Limits {
            max_handles: 4,
            admission_steps: 100,
            expression_depth: 16,
            expression_work: 100,
        },
    )
    .unwrap();
    ObjectHost::checked(e).unwrap()
}
#[test]
fn camera_requires_explicit_admission_and_bounds_replace_atomically() {
    let mut disabled = host(COMMON, "");
    assert!(
        disabled
            .native("GetCameraMargin", &[Value::Int(0)])
            .is_err()
    );
    let mut h = disabled.with_camera(margins()).unwrap();
    assert!(h.camera_state().unwrap().bounds.is_none());
    for (i, value) in margins().values().iter().enumerate() {
        assert_eq!(
            h.native("GetCameraMargin", &[Value::Int(i as i32)])
                .unwrap(),
            Value::Real(*value)
        );
    }
    for args in [
        vec![Value::Int(-1)],
        vec![Value::Int(4)],
        vec![Value::Real(0.0)],
        vec![],
    ] {
        assert!(h.native("GetCameraMargin", &args).is_err());
    }
    let bounds = [-0.0, 1.25, 2.5, -3.75, 4.0, 5.25, 6.5, 7.75];
    h.native("SetCameraBounds", &bounds.map(Value::Real))
        .unwrap();
    let before = h.camera_state().unwrap().bounds.unwrap().map(f32::to_bits);
    assert_eq!(before, bounds.map(f32::to_bits));
    let mut bad = bounds.map(Value::Real);
    bad[7] = Value::Real(f32::NAN);
    assert!(h.native("SetCameraBounds", &bad).is_err());
    assert!(h.native("SetCameraBounds", &bad[..7]).is_err());
    assert_eq!(
        h.camera_state().unwrap().bounds.unwrap().map(f32::to_bits),
        before
    );
    assert!(h.with_camera(margins()).is_err());
}
#[test]
fn camera_native_signatures_flags_and_script_overrides_are_checked() {
    for common in [
        COMMON.replace("native GetCameraMargin", "constant native GetCameraMargin"),
        COMMON.replace("integer i returns real", "integer i returns integer"),
        COMMON.replace("real h returns nothing", "real h returns real"),
    ] {
        assert!(host(&common, "").with_camera(margins()).is_err());
    }
    assert!(
        host(
            COMMON,
            "function GetCameraMargin takes integer i returns real\nreturn 0.0\nendfunction\n"
        )
        .with_camera(margins())
        .is_err()
    );
}
#[test]
fn owned_vm_uses_the_derived_values_and_retains_the_authored_bounds() {
    let mut vm = host(COMMON, "function main takes nothing returns nothing\ncall SetCameraBounds(GetCameraMargin(0),GetCameraMargin(1),GetCameraMargin(2),GetCameraMargin(3),-1.0,2.0,3.0,4.0)\nendfunction\n").with_camera(margins()).unwrap().into_vm();
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    assert_eq!(
        vm.host().camera_state().unwrap().bounds.unwrap(),
        [47.5, -17.5, 0.0, 7.5, -1.0, 2.0, 3.0, 4.0]
    );
}

const BOUND_NAMES: [&str; 4] = [
    "GetCameraBoundMinX",
    "GetCameraBoundMinY",
    "GetCameraBoundMaxX",
    "GetCameraBoundMaxY",
];
fn bound_common() -> String {
    format!(
        "{COMMON}{}",
        BOUND_NAMES
            .map(|name| format!("constant native {name} takes nothing returns real\n"))
            .join("")
    )
}
#[test]
fn bound_getters_require_admission_assignment_and_keep_failed_bounds() {
    let common = bound_common();
    assert!(host(&common, "").with_camera_bound_getters().is_err());
    let mut h = host(&common, "").with_camera(margins()).unwrap();
    for name in BOUND_NAMES {
        assert!(h.native(name, &[]).is_err());
    }
    let mut h = h.with_camera_bound_getters().unwrap();
    for name in BOUND_NAMES {
        assert!(h.native(name, &[]).is_err());
    }
    // Every extreme belongs to a different corner; paired endpoints are insufficient.
    let assigned = [
        -900.25, 500.5, 100.125, 1000.75, -1200.5, -300.25, 800.75, -50.125,
    ];
    h.native("SetCameraBounds", &assigned.map(Value::Real))
        .unwrap();
    let check = |h: &mut ObjectHost| {
        for (name, expected) in BOUND_NAMES
            .into_iter()
            .zip([-1200.5, -300.25, 800.75, 1000.75])
        {
            assert_eq!(h.native(name, &[]).unwrap(), Value::Real(expected));
            assert!(h.native(name, &[Value::Int(0)]).is_err());
        }
    };
    check(&mut h);
    for index in 0..8 {
        for value in [
            Value::Real(f32::NAN),
            Value::Real(f32::INFINITY),
            Value::Real(f32::NEG_INFINITY),
            Value::Bool(false),
        ] {
            let mut bad = assigned.map(Value::Real);
            bad[index] = value;
            assert!(h.native("SetCameraBounds", &bad).is_err());
            check(&mut h);
            assert_eq!(
                h.camera_state().unwrap().bounds.unwrap().map(f32::to_bits),
                assigned.map(f32::to_bits)
            );
        }
    }
    let reordered = [
        800.75, -50.125, -1200.5, -300.25, 100.125, 1000.75, -900.25, 500.5,
    ];
    h.native("SetCameraBounds", &reordered.map(Value::Real))
        .unwrap();
    check(&mut h);
    assert!(h.with_camera_bound_getters().is_err());
}
#[test]
fn bound_getter_contracts_flags_sources_and_overrides_are_checked() {
    let common = bound_common();
    for name in BOUND_NAMES {
        for bad in [
            common.replace(
                &format!("constant native {name}"),
                &format!("native {name}"),
            ),
            common.replace(
                &format!("{name} takes nothing returns real"),
                &format!("{name} takes integer x returns real"),
            ),
            common.replace(
                &format!("{name} takes nothing returns real"),
                &format!("{name} takes nothing returns integer"),
            ),
            common.replace(
                &format!("constant native {name} takes nothing returns real\n"),
                "",
            ),
        ] {
            assert!(
                host(&bad, "")
                    .with_camera(margins())
                    .unwrap()
                    .with_camera_bound_getters()
                    .is_err()
            );
        }
        let map = format!("function {name} takes nothing returns real\nreturn 1.0\nendfunction\n");
        assert!(
            host(&common, &map)
                .with_camera(margins())
                .unwrap()
                .with_camera_bound_getters()
                .is_err()
        );
    }
    let common_ast = pcx_script::parse_script(COMMON).unwrap();
    let foreign = pcx_script::parse_script(&common[COMMON.len()..]).unwrap();
    let map = pcx_script::parse_script("").unwrap();
    let program = Arc::new(
        pcx_script::resolve_with(
            &map,
            &[
                Prelude::new("synthetic:common", &common_ast),
                Prelude::new("synthetic:foreign", &foreign),
            ],
        )
        .unwrap(),
    );
    let enums = EnumHost::checked_selected(
        program,
        "synthetic:camera",
        &["ConvertPlayerColor"],
        Limits {
            max_handles: 4,
            admission_steps: 100,
            expression_depth: 16,
            expression_work: 100,
        },
    )
    .unwrap();
    assert!(
        ObjectHost::checked(enums)
            .unwrap()
            .with_camera(margins())
            .unwrap()
            .with_camera_bound_getters()
            .is_err()
    );
}
#[test]
fn owned_vm_derives_all_four_bounds_without_margin_adjustment_or_allocation() {
    let common = bound_common();
    let map = "globals\nreal a = 0.0\nreal b = 0.0\nreal c = 0.0\nreal d = 0.0\nendglobals\nfunction main takes nothing returns nothing\ncall SetCameraBounds(-1,2,3,-4,5,6,-7,-8)\nset a = GetCameraBoundMinX()\nset b = GetCameraBoundMinY()\nset c = GetCameraBoundMaxX()\nset d = GetCameraBoundMaxY()\nendfunction\n";
    let mut vm = host(&common, map)
        .with_camera(margins())
        .unwrap()
        .with_camera_bound_getters()
        .unwrap()
        .into_vm();
    vm.init_globals().unwrap();
    let before = vm.host().inner().handles();
    vm.call("main", &[]).unwrap();
    for (name, value) in [("a", -7.0), ("b", -8.0), ("c", 5.0), ("d", 6.0)] {
        assert_eq!(vm.global(name), Value::Real(value));
    }
    assert_eq!(vm.host().inner().handles(), before);
    assert_eq!(
        vm.host().camera_state().unwrap().bounds.unwrap(),
        [-1.0, 2.0, 3.0, -4.0, 5.0, 6.0, -7.0, -8.0]
    );
}

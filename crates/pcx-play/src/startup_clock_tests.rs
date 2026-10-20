use super::*;
use crate::playercolor_host::{EnumHost, Limits, ObjectHost};
use pcx_script::{Host, Prelude, Value};
use std::sync::Arc;
fn origin() -> AssetIdentity {
    AssetIdentity {
        edition: "synthetic".into(),
        version: "test".into(),
        source: "synthetic:clock-PE".into(),
    }
}
fn fixture(hour: i8) -> (Vec<u8>, Binding, RuleInputBudget) {
    let mut image = vec![0; 0x400];
    for (at, value) in [
        (0x3c, 0x80u32),
        (0xb4, 0x300000),
        (0xd0, 0x2000),
        (0x100, 0x200),
        (0x104, 0x1000),
        (0x108, 0x200),
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
    // Entirely synthetic destinations/call displacements, not copied game code.
    let code = [
        0x6a, hour as u8, 0x68, 0x70, 0x30, 0x30, 0, 0xe8, 0x10, 0, 0, 0, 0x83, 0xc4, 8, 0xc3,
    ];
    image[0x220..0x230].copy_from_slice(&code);
    let mut scale = code;
    scale[1] = 3;
    image[0x230..0x240].copy_from_slice(&scale);
    let binding = Binding {
        origin: origin(),
        image_sha256: sha256(&image),
        initializer: SourceWindow {
            va: 0x301020,
            len: 16,
            digest: sha256(&code),
        },
        scale_initializer: SourceWindow {
            va: 0x301030,
            len: 16,
            digest: sha256(&scale),
        },
    };
    (
        image,
        binding,
        RuleInputBudget {
            image_bytes: 4096,
            profile_bytes: 64,
            sections: 2,
        },
    )
}
pub(crate) fn input(hour: i8) -> StartupClock {
    let (d, b, n) = fixture(hour);
    StartupClock::read(&d, b, n).unwrap()
}
const COMMON: &str = "type agent extends handle\ntype timer extends agent\ntype fgamestate extends handle\ntype playercolor extends handle\nconstant native ConvertFGameState takes integer i returns fgamestate\nconstant native ConvertPlayerColor takes integer i returns playercolor\nnative CreateTimer takes nothing returns timer\nnative DestroyTimer takes timer t returns nothing\nconstant native GetFloatGameState takes fgamestate s returns real\nglobals\nconstant fgamestate CLOCK = ConvertFGameState(11 + 26)\nconstant fgamestate OTHER = ConvertFGameState(-5)\nconstant playercolor C = ConvertPlayerColor(37)\nendglobals\n";
fn host(common: &str) -> ObjectHost {
    let c = pcx_script::parse_script(common).unwrap();
    let map = pcx_script::parse_script("").unwrap();
    let p =
        Arc::new(pcx_script::resolve_with(&map, &[Prelude::new("synthetic:common", &c)]).unwrap());
    let inner = EnumHost::checked_selected(
        p,
        "synthetic:clock",
        &["ConvertFGameState", "ConvertPlayerColor"],
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
fn admitted(hour: i8) -> ObjectHost {
    host(COMMON)
        .with_startup_clock(input(hour), "CLOCK", 100, 16, 100)
        .unwrap()
}
#[test]
fn initial_hour_comes_from_checked_code_not_a_runtime_address_or_fixed_value() {
    for hour in [i8::MIN, -3, 0, 7, 29, i8::MAX] {
        let c = input(hour);
        assert_eq!(c.hour().to_bits(), (hour as f32).to_bits());
        assert_eq!(c.scale(), 3.0);
        assert_eq!(c.binding().origin.source, "synthetic:clock-PE");
        let mut h = admitted(hour);
        let id = h.native("ConvertFGameState", &[Value::Int(37)]).unwrap();
        assert_eq!(
            h.native("GetFloatGameState", &[id])
                .unwrap()
                .as_real()
                .unwrap()
                .to_bits(),
            c.hour().to_bits()
        );
        assert_eq!(h.startup_clock_state().unwrap().selector, 37);
    }
}
#[test]
fn wrong_arguments_families_and_selectors_refuse_without_mutation() {
    let mut h = admitted(9);
    let wrong = h.native("ConvertPlayerColor", &[Value::Int(37)]).unwrap();
    let other = h.native("ConvertFGameState", &[Value::Int(-5)]).unwrap();
    for args in [
        vec![],
        vec![Value::Null],
        vec![Value::Int(37)],
        vec![wrong],
        vec![other],
    ] {
        assert!(h.native("GetFloatGameState", &args).is_err());
    }
    assert_eq!(h.startup_clock_state().unwrap().input.hour(), 9.0);
    assert!(h.native("SetFloatGameState", &[]).is_err());
    assert!(host(COMMON).native("GetFloatGameState", &[]).is_err());
}
#[test]
fn admission_requires_source_global_contract_and_explicit_work() {
    for global in ["", "missing", "C"] {
        assert!(
            host(COMMON)
                .with_startup_clock(input(3), global, 100, 16, 100)
                .is_err()
        );
    }
    for (n, d, w) in [(0, 16, 100), (100, 0, 100), (100, 16, 0)] {
        assert!(
            host(COMMON)
                .with_startup_clock(input(3), "CLOCK", n, d, w)
                .is_err()
        );
    }
    for bad in [
        COMMON.replace(
            "constant native GetFloatGameState",
            "native GetFloatGameState",
        ),
        COMMON.replace(
            "takes fgamestate s returns real",
            "takes integer s returns real",
        ),
    ] {
        assert!(
            host(&bad)
                .with_startup_clock(input(3), "CLOCK", 100, 16, 100)
                .is_err()
        );
    }
    assert!(
        admitted(3)
            .with_startup_clock(input(4), "CLOCK", 100, 16, 100)
            .is_err()
    );
}
#[test]
fn source_and_window_identity_permissions_and_bss_are_checked() {
    for case in 0..6 {
        let (mut d, mut b, n) = fixture(5);
        match case {
            0 => d[0x300] ^= 1,
            1 => b.initializer.digest[0] ^= 1,
            2 => {
                d[0x11c..0x120].copy_from_slice(&0x40000040u32.to_le_bytes());
                b.image_sha256 = sha256(&d);
            }
            3 => {
                d[0x100..0x104].copy_from_slice(&0x300u32.to_le_bytes());
                b.image_sha256 = sha256(&d);
                b.initializer.va = 0x301220;
            }
            4 => b.origin.version.clear(),
            _ => b.initializer.va = u32::MAX,
        }
        assert!(StartupClock::read(&d, b, n).is_err(), "case {case}");
    }
}
#[test]
fn every_initializer_instruction_and_length_is_validated() {
    for at in [0, 2, 7, 12, 13, 14, 15] {
        let (mut d, mut b, n) = fixture(5);
        d[0x220 + at] ^= 1;
        b.image_sha256 = sha256(&d);
        b.initializer.digest = sha256(&d[0x220..0x230]);
        assert!(matches!(
            StartupClock::read(&d, b, n).unwrap_err().problem,
            Problem::Initializer
        ));
    }
    for len in [1u32, 15, 17, 0x100] {
        let (d, mut b, n) = fixture(5);
        b.initializer.len = len;
        b.initializer.digest = sha256(&d[0x220..0x220 + len as usize]);
        assert!(StartupClock::read(&d, b, n).is_err());
    }
}
#[test]
fn source_budgets_precede_parsing_and_fingerprinting() {
    for case in 0..3 {
        let (d, b, mut n) = fixture(2);
        match case {
            0 => n.image_bytes = d.len() - 1,
            1 => n.profile_bytes = 15,
            _ => n.sections = 0,
        };
        assert!(StartupClock::read(&d, b, n).is_err());
    }
}
#[test]
fn retained_vm_uses_same_arena_and_source_selected_clock_constant() {
    let c = pcx_script::parse_script(COMMON).unwrap();
    let map=pcx_script::parse_script("function ReadClock takes nothing returns real\nreturn GetFloatGameState(CLOCK)\nendfunction\n").unwrap();
    let p =
        Arc::new(pcx_script::resolve_with(&map, &[Prelude::new("synthetic:common", &c)]).unwrap());
    let inner = EnumHost::checked_selected(
        p,
        "synthetic:clock",
        &["ConvertFGameState", "ConvertPlayerColor"],
        Limits {
            max_handles: 8,
            admission_steps: 100,
            expression_depth: 16,
            expression_work: 100,
        },
    )
    .unwrap();
    let mut vm = ObjectHost::checked(inner)
        .unwrap()
        .with_startup_clock(input(19), "CLOCK", 100, 16, 100)
        .unwrap()
        .into_vm();
    vm.init_globals().unwrap();
    assert_eq!(vm.call("ReadClock", &[]).unwrap().as_real(), Some(19.0));
}

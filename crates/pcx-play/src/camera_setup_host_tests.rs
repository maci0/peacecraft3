use super::super::super::{EnumHost, Failure, Limits, ObjectHost};
use pcx_script::{HandleId, Host, Prelude, Value};
use std::num::NonZeroU64;
use std::sync::Arc;

const DECLARATIONS: &str = "type agent extends handle
type timer extends agent
type camerafield extends handle
type camerasetup extends handle
type playercolor extends handle
constant native ConvertCameraField takes integer i returns camerafield
constant native ConvertPlayerColor takes integer value returns playercolor
native CreateTimer takes nothing returns timer
native DestroyTimer takes timer which returns nothing
native CreateCameraSetup takes nothing returns camerasetup
native CameraSetupSetField takes camerasetup whichSetup, camerafield whichField, real value, real duration returns nothing
native CameraSetupGetField takes camerasetup whichSetup, camerafield whichField returns real
native CameraSetupSetDestPosition takes camerasetup whichSetup, real x, real y, real duration returns nothing
native CameraSetupGetDestPositionX takes camerasetup whichSetup returns real
native CameraSetupGetDestPositionY takes camerasetup whichSetup returns real
globals
constant camerafield FIELD_NEAR = ConvertCameraField(4 + 5)
constant camerafield FIELD_OTHER = ConvertCameraField(-3)
constant playercolor COLOR = ConvertPlayerColor(19)
endglobals
";

fn host_from(declarations: &str, script: &str, max_handles: usize) -> ObjectHost {
    let common = pcx_script::parse_script(declarations).unwrap();
    let map = pcx_script::parse_script(script).unwrap();
    let program = Arc::new(
        pcx_script::resolve_with(&map, &[Prelude::new("synthetic:camera-contract", &common)])
            .unwrap(),
    );
    let inner = EnumHost::checked_selected(
        program,
        "synthetic:selected-map",
        &["ConvertPlayerColor", "ConvertCameraField"],
        Limits {
            max_handles,
            admission_steps: 200,
            expression_depth: 16,
            expression_work: 200,
        },
    )
    .unwrap();
    ObjectHost::checked(inner).unwrap()
}

fn host() -> ObjectHost {
    host_from(DECLARATIONS, "", 16)
        .with_camera_setups(2)
        .unwrap()
}

fn field(host: &mut ObjectHost, value: i32) -> Value {
    host.native("ConvertCameraField", &[Value::Int(value)])
        .unwrap()
}

fn create(host: &mut ObjectHost) -> Value {
    host.native("CreateCameraSetup", &[]).unwrap()
}

#[test]
fn direct_handle_setup_keeps_executed_field_and_destination_bits() {
    let mut host = host();
    let timer = host.native("CreateTimer", &[]).unwrap();
    let near = field(&mut host, 9);
    let other = field(&mut host, -3);
    let setup = create(&mut host);
    assert_ne!(setup, timer);
    assert_ne!(setup, near);
    let Value::Handle(setup_id) = setup.clone() else {
        panic!("setup handle");
    };
    assert_eq!(host.inner().kind(setup_id).unwrap(), "camerasetup");
    let Value::Handle(near_id) = near.clone() else {
        panic!("field handle");
    };
    assert!(host.camera_setup_field(setup_id, near_id).is_err());
    assert!(host.camera_setup_destination(setup_id).is_err());
    let value = f32::from_bits(0xbf80_0000);
    let duration = f32::from_bits(0x8000_0000);
    host.native(
        "CameraSetupSetField",
        &[
            setup.clone(),
            near.clone(),
            Value::Real(value),
            Value::Real(duration),
        ],
    )
    .unwrap();
    host.native(
        "CameraSetupSetDestPosition",
        &[
            setup.clone(),
            Value::Real(-2.5),
            Value::Real(3.25),
            Value::Real(duration),
        ],
    )
    .unwrap();
    assert_eq!(
        host.native("CameraSetupGetField", &[setup.clone(), near.clone()])
            .unwrap(),
        Value::Real(value)
    );
    assert_eq!(
        host.camera_setup_field(setup_id, near_id).unwrap(),
        (value, duration)
    );
    assert_eq!(
        (
            host.native("CameraSetupGetDestPositionX", std::slice::from_ref(&setup))
                .unwrap(),
            host.native("CameraSetupGetDestPositionY", std::slice::from_ref(&setup))
                .unwrap(),
        ),
        (Value::Real(-2.5), Value::Real(3.25))
    );
    assert_eq!(
        host.camera_setup_destination(setup_id)
            .unwrap()
            .duration
            .to_bits(),
        duration.to_bits()
    );
    host.native(
        "CameraSetupSetField",
        &[
            setup.clone(),
            near.clone(),
            Value::Real(1.5),
            Value::Real(2.0),
        ],
    )
    .unwrap();
    assert_eq!(
        host.camera_setup_field(setup_id, near_id).unwrap(),
        (1.5, 2.0)
    );
    let before = host.camera_setup_field(setup_id, near_id).unwrap();
    for bad in [f32::NAN, f32::INFINITY] {
        assert!(
            host.native(
                "CameraSetupSetField",
                &[
                    setup.clone(),
                    near.clone(),
                    Value::Real(bad),
                    Value::Real(1.0)
                ],
            )
            .is_err()
        );
        assert!(
            host.native(
                "CameraSetupSetDestPosition",
                &[
                    setup.clone(),
                    Value::Real(bad),
                    Value::Real(1.0),
                    Value::Real(1.0)
                ],
            )
            .is_err()
        );
    }
    assert_eq!(host.camera_setup_field(setup_id, near_id).unwrap(), before);
    assert_eq!(
        host.camera_setup_destination(setup_id).unwrap().x.to_bits(),
        (-2.5f32).to_bits()
    );
    assert!(
        host.native("CameraSetupGetField", &[setup.clone(), other.clone()])
            .unwrap_err()
            .contains("field is unset")
    );
    host.native(
        "CameraSetupSetField",
        &[setup.clone(), other, Value::Real(4.0), Value::Real(5.0)],
    )
    .unwrap();
    let limited = host_from(DECLARATIONS, "", 16)
        .with_camera_setups(1)
        .unwrap();
    let mut limited = limited;
    let one = create(&mut limited);
    let first = field(&mut limited, 9);
    let second = field(&mut limited, -3);
    limited
        .native(
            "CameraSetupSetField",
            &[one.clone(), first, Value::Real(1.0), Value::Real(2.0)],
        )
        .unwrap();
    assert!(
        limited
            .native(
                "CameraSetupSetField",
                &[one, second, Value::Real(3.0), Value::Real(4.0)],
            )
            .unwrap_err()
            .contains("capacity")
    );
    let mut tight = {
        let declarations = DECLARATIONS
            .replace(
                "constant camerafield FIELD_OTHER = ConvertCameraField(-3)\n",
                "",
            )
            .replace("constant playercolor COLOR = ConvertPlayerColor(19)\n", "")
            .replace(
                "constant native ConvertPlayerColor takes integer value returns playercolor\n",
                "",
            );
        let common = pcx_script::parse_script(&declarations).unwrap();
        let map = pcx_script::parse_script("").unwrap();
        let program = Arc::new(
            pcx_script::resolve_with(&map, &[Prelude::new("synthetic:camera-contract", &common)])
                .unwrap(),
        );
        let inner = EnumHost::checked_selected(
            program,
            "synthetic:selected-map",
            &["ConvertCameraField"],
            Limits {
                max_handles: 2,
                admission_steps: 200,
                expression_depth: 16,
                expression_work: 200,
            },
        )
        .unwrap();
        ObjectHost::checked(inner)
            .unwrap()
            .with_camera_setups(1)
            .unwrap()
    };
    let _field = field(&mut tight, 9);
    assert!(tight.native("CreateCameraSetup", &[]).is_ok());
    assert!(
        tight
            .native("CreateCameraSetup", &[])
            .unwrap_err()
            .contains("capacity")
    );
    let Value::Handle(timer_id) = timer.clone() else {
        panic!("timer");
    };
    assert!(host.camera_setup_field(timer_id, near_id).is_err());
    for (name, args) in [
        ("CameraSetupGetField", vec![timer.clone(), near.clone()]),
        ("DestroyTimer", vec![setup.clone()]),
        (
            "CameraSetupSetField",
            vec![
                setup.clone(),
                Value::Int(9),
                Value::Real(1.0),
                Value::Real(1.0),
            ],
        ),
        (
            "CameraSetupGetField",
            vec![
                Value::Handle(HandleId::new(NonZeroU64::new(90).unwrap())),
                near.clone(),
            ],
        ),
        (
            "CameraSetupGetDestPositionX",
            vec![Value::Handle(HandleId::new(NonZeroU64::new(91).unwrap()))],
        ),
    ] {
        let error = host.native(name, &args).unwrap_err();
        assert!(
            error.contains("another family")
                || error.contains("unknown handle")
                || error.contains("raw values"),
            "{name}: {error}"
        );
    }
    assert!(
        host.native("CameraSetupGetDestPositionLoc", &[setup])
            .unwrap_err()
            .contains("unsupported")
    );
}

#[test]
fn retargeted_handle_families_are_refused_and_agent_ancestry_still_passes() {
    let agent = DECLARATIONS.replace(
        "type camerasetup extends handle",
        "type camerasetup extends agent",
    );
    let mut admitted = host_from(&agent, "", 8).with_camera_setups(1).unwrap();
    let setup = create(&mut admitted);
    let Value::Handle(setup_id) = setup else {
        panic!("setup handle");
    };
    assert_eq!(admitted.inner().kind(setup_id).unwrap(), "camerasetup");
    let retargeted = DECLARATIONS
        .replace(
            "type camerasetup extends handle\n",
            "type unit extends handle\n",
        )
        .replace("camerasetup", "unit");
    assert!(
        host_from(&retargeted, "", 8).with_camera_setups(1).is_err(),
        "constructor and queries retargeted to unit must be refused"
    );
    let wrong_field = DECLARATIONS
        .replace(
            "type camerafield extends handle\n",
            "type player extends handle\n",
        )
        .replace("camerafield", "player");
    assert!(
        host_from(&wrong_field, "", 8)
            .with_camera_setups(1)
            .is_err(),
        "ConvertCameraField retargeted to player must be refused"
    );
}

#[test]
fn agent_parent_is_optional_and_other_namespace_sources_are_refused() {
    let agent = DECLARATIONS.replace(
        "type camerasetup extends handle",
        "type camerasetup extends agent",
    );
    host_from(&agent, "", 8).with_camera_setups(1).unwrap();
    let mismatched = DECLARATIONS.replace(
        "camerafield whichField, real value",
        "playercolor whichField, real value",
    );
    assert!(host_from(&mismatched, "", 8).with_camera_setups(1).is_err());
    let common = pcx_script::parse_script(
        "type camerafield extends handle
constant native ConvertCameraField takes integer i returns camerafield
globals
constant camerafield FIELD_NEAR = ConvertCameraField(4 + 5)
endglobals
",
    )
    .unwrap();
    let other = pcx_script::parse_script(
        "type agent extends handle
type timer extends agent
type playercolor extends handle
type camerasetup extends handle
constant native ConvertPlayerColor takes integer value returns playercolor
native CreateTimer takes nothing returns timer
native DestroyTimer takes timer which returns nothing
native CreateCameraSetup takes nothing returns camerasetup
native CameraSetupSetField takes camerasetup whichSetup, camerafield whichField, real value, real duration returns nothing
native CameraSetupGetField takes camerasetup whichSetup, camerafield whichField returns real
native CameraSetupSetDestPosition takes camerasetup whichSetup, real x, real y, real duration returns nothing
native CameraSetupGetDestPositionX takes camerasetup whichSetup returns real
native CameraSetupGetDestPositionY takes camerasetup whichSetup returns real
globals
constant playercolor COLOR = ConvertPlayerColor(19)
endglobals
",
    )
    .unwrap();
    let map = pcx_script::parse_script("").unwrap();
    let program = Arc::new(
        pcx_script::resolve_with(
            &map,
            &[
                Prelude::new("synthetic:field-namespace", &common),
                Prelude::new("synthetic:setup-namespace", &other),
            ],
        )
        .unwrap(),
    );
    let inner = EnumHost::checked_selected(
        program,
        "synthetic:selected-map",
        &["ConvertPlayerColor", "ConvertCameraField"],
        Limits {
            max_handles: 8,
            admission_steps: 200,
            expression_depth: 16,
            expression_work: 200,
        },
    )
    .unwrap();
    assert!(
        ObjectHost::checked(inner)
            .unwrap()
            .with_camera_setups(1)
            .is_err()
    );
}

#[test]
fn owned_vm_main_reads_the_admitted_field_without_a_default_destination() {
    let mut vm = host_from(
        DECLARATIONS,
        "globals
camerasetup Setup = null
endglobals
function main takes nothing returns nothing
set Setup = CreateCameraSetup()
call CameraSetupSetField(Setup, FIELD_NEAR, 1.25, 0.5)
endfunction
",
        16,
    )
    .with_camera_setups(1)
    .unwrap()
    .into_vm();
    vm.init_globals().unwrap();
    let Value::Handle(field_id) = vm.global("FIELD_NEAR") else {
        panic!("field global");
    };
    vm.call("main", &[]).unwrap();
    let Value::Handle(setup_id) = vm.global("Setup") else {
        panic!("setup global");
    };
    assert_eq!(
        vm.host().camera_setup_field(setup_id, field_id).unwrap(),
        (1.25, 0.5)
    );
    assert!(vm.host().camera_setup_destination(setup_id).is_err());
}

#[test]
fn owned_vm_reads_a_written_field_and_refuses_the_other_setup() {
    let mut vm = host_from(
        DECLARATIONS,
        "globals
camerasetup First = null
camerasetup Second = null
real Read = 0.
endglobals
function main takes nothing returns nothing
set First = CreateCameraSetup()
set Second = CreateCameraSetup()
call CameraSetupSetField(First, FIELD_NEAR, 1.25, 0.5)
set Read = CameraSetupGetField(First, FIELD_NEAR)
endfunction
function other takes nothing returns real
return CameraSetupGetField(Second, FIELD_NEAR)
endfunction
",
        16,
    )
    .with_camera_setups(1)
    .unwrap()
    .into_vm();
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    let Value::Handle(first) = vm.global("First") else {
        panic!("first setup");
    };
    let Value::Handle(second) = vm.global("Second") else {
        panic!("second setup");
    };
    assert_ne!(first, second);
    assert_eq!(vm.global("Read"), Value::Real(1.25));
    assert!(
        vm.call("other", &[])
            .unwrap_err()
            .to_string()
            .contains("field is unset")
    );
}

#[test]
fn absent_getters_stay_unsupported_and_mismatched_ones_are_refused() {
    let without = DECLARATIONS
        .replace(
            "native CameraSetupGetField takes camerasetup whichSetup, camerafield whichField returns real\n",
            "",
        )
        .replace(
            "native CameraSetupGetDestPositionX takes camerasetup whichSetup returns real\n",
            "",
        )
        .replace(
            "native CameraSetupGetDestPositionY takes camerasetup whichSetup returns real\n",
            "",
        );
    let mut admitted = host_from(&without, "", 8).with_camera_setups(1).unwrap();
    let setup = create(&mut admitted);
    let near = field(&mut admitted, 9);
    admitted
        .native(
            "CameraSetupSetField",
            &[
                setup.clone(),
                near.clone(),
                Value::Real(1.0),
                Value::Real(2.0),
            ],
        )
        .unwrap();
    for (name, args) in [
        ("CameraSetupGetField", vec![setup.clone(), near]),
        ("CameraSetupGetDestPositionX", vec![setup.clone()]),
        ("CameraSetupGetDestPositionY", vec![setup]),
    ] {
        assert!(
            admitted
                .native(name, &args)
                .unwrap_err()
                .contains("unsupported"),
            "{name}"
        );
    }
    for replacement in [
        "constant native CameraSetupGetField takes camerasetup whichSetup, camerafield whichField returns real\n",
        "native CameraSetupGetField takes integer whichSetup, camerafield whichField returns real\n",
        "native CameraSetupGetField takes camerasetup whichSetup, camerafield whichField returns nothing\n",
        "native CameraSetupGetDestPositionX takes camerasetup whichSetup, real extra returns real\n",
    ] {
        let declarations = if replacement.contains("GetField") {
            DECLARATIONS.replace(
                "native CameraSetupGetField takes camerasetup whichSetup, camerafield whichField returns real\n",
                replacement,
            )
        } else {
            DECLARATIONS.replace(
                "native CameraSetupGetDestPositionX takes camerasetup whichSetup returns real\n",
                replacement,
            )
        };
        assert!(
            host_from(&declarations, "", 8)
                .with_camera_setups(1)
                .is_err(),
            "{replacement}"
        );
    }
    let selected = pcx_script::parse_script(
        "type camerafield extends handle
type camerasetup extends handle
constant native ConvertCameraField takes integer i returns camerafield
native CreateCameraSetup takes nothing returns camerasetup
native CameraSetupSetField takes camerasetup whichSetup, camerafield whichField, real value, real duration returns nothing
native CameraSetupSetDestPosition takes camerasetup whichSetup, real x, real y, real duration returns nothing
globals
constant camerafield FIELD_NEAR = ConvertCameraField(4 + 5)
endglobals
",
    )
    .unwrap();
    let foreign_getter = pcx_script::parse_script(
        "native CameraSetupGetField takes camerasetup whichSetup, camerafield whichField returns real
",
    )
    .unwrap();
    let map = pcx_script::parse_script("").unwrap();
    let program = Arc::new(
        pcx_script::resolve_with(
            &map,
            &[
                Prelude::new("synthetic:setup-namespace", &selected),
                Prelude::new("synthetic:getter-namespace", &foreign_getter),
            ],
        )
        .unwrap(),
    );
    let inner = EnumHost::checked_selected(
        program,
        "synthetic:selected-map",
        &["ConvertCameraField"],
        Limits {
            max_handles: 8,
            admission_steps: 200,
            expression_depth: 16,
            expression_work: 200,
        },
    )
    .unwrap();
    let error = ObjectHost::checked(inner)
        .unwrap()
        .with_camera_setups(1)
        .unwrap_err();
    assert_eq!(error.native, "CameraSetupGetField");
    assert_eq!(error.failure, Failure::Source);
}

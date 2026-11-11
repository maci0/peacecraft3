use super::*;

const MAP: &str = "globals\nplayer P = null\nunit U = null\nendglobals\nfunction setup takes nothing returns nothing\nset P = Player(4)\nset U = CreateUnit(P, 'hfoo', 1.5, -2.25, 90.0)\nendfunction\n";

fn open_extra(extra: &str) -> VmHost {
    let common = script_common_with("widget", true, extra);
    let ctx = context(300, &common, MAP, "unit-catalog");
    let (catalog, hits, seen) = counting();
    let shared: Arc<dyn ObjectCatalog + Send + Sync> = catalog;
    let host = object(&ctx, 16)
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_unit_catalog(Arc::clone(&shared), source(), limits(4))
        .unwrap();
    VmHost {
        vm: Arc::clone(&ctx.scripts).owned_vm(host),
        hits,
        seen,
        catalog: shared,
    }
}

const RESERVE: &str = "native SetReservedLocalHeroButtons takes integer reserved returns nothing\n";

#[test]
fn reserved_local_hero_buttons_stores_the_integer_unclamped() {
    let mut s = open_extra(RESERVE);
    s.vm.init_globals().unwrap();
    let host = &mut s.vm.host().inner;
    assert_eq!(host.reserved_local_hero_buttons(), None);
    for value in [2, 0, -3, i32::MAX] {
        assert_eq!(
            host.native("SetReservedLocalHeroButtons", &[Value::Int(value)])
                .unwrap(),
            Value::Null
        );
        assert_eq!(host.reserved_local_hero_buttons(), Some(value));
    }
}

#[test]
fn reserved_local_hero_buttons_refuses_other_argument_shapes_and_keeps_the_value() {
    let mut s = open_extra(RESERVE);
    s.vm.init_globals().unwrap();
    let host = &mut s.vm.host().inner;
    host.native("SetReservedLocalHeroButtons", &[Value::Int(2)])
        .unwrap();
    for args in [
        &[][..],
        &[Value::Real(1.)][..],
        &[Value::Int(1), Value::Int(1)][..],
    ] {
        assert!(host.native("SetReservedLocalHeroButtons", args).is_err());
    }
    assert_eq!(host.reserved_local_hero_buttons(), Some(2));
}

#[test]
fn reserved_local_hero_buttons_refuses_a_declaration_with_another_signature() {
    let common = script_common_with(
        "widget",
        true,
        "native SetReservedLocalHeroButtons takes real reserved returns nothing\n",
    );
    let ctx = context(300, &common, MAP, "unit-catalog");
    let (catalog, ..) = counting();
    let shared: Arc<dyn ObjectCatalog + Send + Sync> = catalog;
    let admitted = object(&ctx, 16)
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_unit_catalog(shared, source(), limits(4));
    assert!(admitted.is_err());
}

#[test]
fn a_program_without_the_declaration_leaves_the_native_unsupported() {
    let mut s = open_extra("");
    s.vm.init_globals().unwrap();
    let host = &mut s.vm.host().inner;
    let error = host
        .native("SetReservedLocalHeroButtons", &[Value::Int(2)])
        .unwrap_err();
    assert!(error.contains("unsupported"), "{error}");
    assert_eq!(host.reserved_local_hero_buttons(), None);
}

const ALT_ICON: &str = "native SetAltMinimapIcon takes string iconPath returns nothing\n";

#[test]
fn alt_minimap_icon_keeps_the_last_accepted_path() {
    let mut s = open_extra(ALT_ICON);
    s.vm.init_globals().unwrap();
    let host = &mut s.vm.host().inner;
    assert_eq!(host.alt_minimap_icon(), None);
    host.native("SetAltMinimapIcon", &[Value::Str("UI\\Minimap\\A".into())])
        .unwrap();
    assert_eq!(host.alt_minimap_icon(), Some("UI\\Minimap\\A"));
    host.native("SetAltMinimapIcon", &[Value::Str("UI\\Minimap\\B".into())])
        .unwrap();
    assert_eq!(host.alt_minimap_icon(), Some("UI\\Minimap\\B"));
}

#[test]
fn alt_minimap_icon_ignores_a_null_or_empty_path_instead_of_clearing() {
    let mut s = open_extra(ALT_ICON);
    s.vm.init_globals().unwrap();
    let host = &mut s.vm.host().inner;
    host.native("SetAltMinimapIcon", &[Value::Null]).unwrap();
    host.native("SetAltMinimapIcon", &[Value::Str("".into())])
        .unwrap();
    assert_eq!(host.alt_minimap_icon(), None);
    host.native("SetAltMinimapIcon", &[Value::Str("x".into())])
        .unwrap();
    host.native("SetAltMinimapIcon", &[Value::Null]).unwrap();
    host.native("SetAltMinimapIcon", &[Value::Str("".into())])
        .unwrap();
    assert_eq!(host.alt_minimap_icon(), Some("x"));
}

#[test]
fn alt_minimap_icon_refuses_trigger_strings_and_bad_arguments_without_a_write() {
    let mut s = open_extra(ALT_ICON);
    s.vm.init_globals().unwrap();
    let host = &mut s.vm.host().inner;
    host.native("SetAltMinimapIcon", &[Value::Str("keep".into())])
        .unwrap();
    let trigger = host
        .native("SetAltMinimapIcon", &[Value::Str("trigstr_12".into())])
        .unwrap_err();
    assert!(trigger.contains("map string table"), "{trigger}");
    for args in [&[][..], &[Value::Int(1)][..]] {
        assert!(host.native("SetAltMinimapIcon", args).is_err());
    }
    assert_eq!(host.alt_minimap_icon(), Some("keep"));
}

const CAMERA: &str = "type camerafield extends handle
type camerasetup extends handle
constant native ConvertCameraField takes integer i returns camerafield
native CreateCameraSetup takes nothing returns camerasetup
native CameraSetupSetField takes camerasetup s, camerafield f, real v, real d returns nothing
native CameraSetupSetDestPosition takes camerasetup s, real x, real y, real d returns nothing
native PanCameraToTimed takes real x, real y, real d returns nothing
native SetCameraQuickPosition takes real x, real y returns nothing
constant native GetCameraTargetPositionX takes nothing returns real
native RemoveUnit takes unit u returns nothing
native TimerStart takes timer t, real timeout, boolean periodic, code callback returns nothing
native TimerGetElapsed takes timer t returns real
native TimerGetRemaining takes timer t returns real
native TimerGetTimeout takes timer t returns real
native GetExpiredTimer takes nothing returns timer
native SetCameraTargetController takes unit whichUnit, real xoffset, real yoffset, boolean inheritOrientation returns nothing
";

const CAMERA_FIELDS: &str = "globals
constant camerafield CAMERA_FIELD_TARGET_DISTANCE = ConvertCameraField(0)
constant camerafield CAMERA_FIELD_FARZ = ConvertCameraField(1)
constant camerafield CAMERA_FIELD_ANGLE_OF_ATTACK = ConvertCameraField(2)
constant camerafield CAMERA_FIELD_FIELD_OF_VIEW = ConvertCameraField(3)
constant camerafield CAMERA_FIELD_ROTATION = ConvertCameraField(5)
constant camerafield CAMERA_FIELD_ZOFFSET = ConvertCameraField(6)
";

fn open_camera() -> VmHost {
    let common = script_common_with("widget", true, CAMERA).replacen("globals\n", CAMERA_FIELDS, 1);
    let ctx = context(300, &common, MAP, "unit-catalog");
    let (catalog, hits, seen) = counting();
    let shared: Arc<dyn ObjectCatalog + Send + Sync> = catalog;
    let enums = EnumHost::checked_selected(
        ctx.scripts.shared_program(),
        &ctx.map_digest.label,
        &[
            "ConvertPlacement",
            "ConvertPlayerColor",
            "ConvertRacePref",
            "ConvertMapControl",
            "ConvertCameraField",
        ],
        Limits {
            max_handles: 32,
            admission_steps: 1000,
            expression_depth: 16,
            expression_work: 1000,
        },
    )
    .unwrap();
    let mut host = ObjectHost::checked(enums)
        .unwrap()
        .with_map_config(crate::playercolor_host::MapConfigBudget {
            text_bytes: 128,
            start_locations: 2,
        })
        .unwrap()
        .with_timer_runtime(100.0)
        .unwrap()
        .with_camera_setups(8)
        .unwrap()
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_unit_catalog(Arc::clone(&shared), source(), limits(4))
        .unwrap();
    host.admit_camera_view(&pcx_objects::editor::camera::GameCamera {
        angle_of_attack: 304.0,
        field_of_view: 70.0,
        rotation: 90.0,
        distance: 1650.0,
        far_z: 5000.0,
        height: 0.0,
        forward_rate: 3000.0,
        strafe_rate: 3000.0,
    })
    .unwrap();
    VmHost {
        vm: Arc::clone(&ctx.scripts).owned_vm(host),
        hits,
        seen,
        catalog: shared,
    }
}

#[test]
fn camera_target_controller_stores_the_unit_offsets_and_orientation_flag() {
    let mut s = open_camera();
    s.vm.init_globals().unwrap();
    s.vm.call("setup", &[]).unwrap();
    let unit = s.vm.global("U");
    let Value::Handle(id) = unit else {
        panic!("unit")
    };
    let host = &mut s.vm.host().inner;
    assert_eq!(host.camera_target_controller(), None);
    let call = |host: &mut ObjectHost, unit: Value, x: f32, y: f32, inherit: bool| {
        host.native(
            "SetCameraTargetController",
            &[unit, Value::Real(x), Value::Real(y), Value::Bool(inherit)],
        )
    };
    assert_eq!(
        call(host, unit.clone(), 10., -5., true).unwrap(),
        Value::Null
    );
    let first = host.camera_target_controller().unwrap();
    assert_eq!(
        (first.unit, first.offset, first.inherit_orientation),
        (id, [10., -5.], true)
    );
    call(host, unit.clone(), 0., 0., false).unwrap();
    let second = host.camera_target_controller().unwrap();
    assert_eq!(
        (second.offset, second.inherit_orientation),
        ([0., 0.], false)
    );
    // A null unit ends the call with no write.
    call(host, Value::Null, 3., 3., true).unwrap();
    assert_eq!(host.camera_target_controller(), Some(second));
}

#[test]
fn camera_target_controller_ignores_a_removed_unit_and_refuses_bad_calls() {
    let mut s = open_camera();
    s.vm.init_globals().unwrap();
    s.vm.call("setup", &[]).unwrap();
    let unit = s.vm.global("U");
    let host = &mut s.vm.host().inner;
    host.native("RemoveUnit", std::slice::from_ref(&unit))
        .unwrap();
    host.native(
        "SetCameraTargetController",
        &[unit, Value::Real(1.), Value::Real(1.), Value::Bool(true)],
    )
    .unwrap();
    assert_eq!(host.camera_target_controller(), None);
    let wrong_family = host
        .native(
            "SetCameraTargetController",
            &[
                Value::Int(0),
                Value::Real(1.),
                Value::Real(1.),
                Value::Bool(true),
            ],
        )
        .unwrap_err();
    assert!(wrong_family.contains("another family"), "{wrong_family}");
    assert!(
        host.native("SetCameraTargetController", &[Value::Null])
            .is_err()
    );
}

#[test]
fn a_set_controller_makes_target_reads_and_pans_refuse_instead_of_guessing() {
    let mut s = open_camera();
    s.vm.init_globals().unwrap();
    s.vm.call("setup", &[]).unwrap();
    let unit = s.vm.global("U");
    let host = &mut s.vm.host().inner;
    host.native(
        "PanCameraToTimed",
        &[Value::Real(5.), Value::Real(6.), Value::Real(0.)],
    )
    .unwrap();
    assert_eq!(
        host.native("GetCameraTargetPositionX", &[]).unwrap(),
        Value::Real(5.)
    );
    host.native(
        "SetCameraTargetController",
        &[unit, Value::Real(0.), Value::Real(0.), Value::Bool(false)],
    )
    .unwrap();
    for (name, args) in [
        ("GetCameraTargetPositionX", vec![]),
        (
            "PanCameraToTimed",
            vec![Value::Real(1.), Value::Real(1.), Value::Real(0.)],
        ),
        (
            "SetCameraQuickPosition",
            vec![Value::Real(1.), Value::Real(1.)],
        ),
    ] {
        let error = host.native(name, &args).unwrap_err();
        assert!(error.contains("target controller unit is set"), "{error}");
    }
}

use super::{COMMON, budget, context, id, slots_rule};
use crate::map_admission::ParsedMapContext;
use crate::playercolor_host::{EnumHost, Limits, ObjectHost, StartupSettings};
use pcx_script::{HandleId, Host, Value};
use std::sync::Arc;
const SETTINGS: &str = "type gamespeed extends handle
constant native ConvertGameSpeed takes integer i returns gamespeed
constant native ConvertOtherSpeed takes integer i returns gamespeed
constant native GetGameSpeed takes nothing returns gamespeed
native SetGameSpeed takes gamespeed s returns nothing
native IsFogEnabled takes nothing returns boolean
native IsFogMaskEnabled takes nothing returns boolean
native FogEnable takes boolean b returns nothing
native FogMaskEnable takes boolean b returns nothing
globals
constant gamespeed Low = ConvertGameSpeed(-11)
constant gamespeed High = ConvertGameSpeed(43)
constant gamespeed Other = ConvertOtherSpeed(101)
endglobals
";
fn common() -> String {
    format!("{COMMON}\n{SETTINGS}")
}
fn object(c: &str, map: &str, selected: bool) -> (ObjectHost, Arc<ParsedMapContext>) {
    let ctx = context(301, c, map, "synthetic:settings");
    let mut names = vec![
        "ConvertPlacement",
        "ConvertPlayerColor",
        "ConvertRacePref",
        "ConvertMapControl",
        "ConvertOtherSpeed",
    ];
    if selected {
        names.push("ConvertGameSpeed");
    }
    let inner = EnumHost::checked_selected(
        ctx.scripts.shared_program(),
        "synthetic:settings",
        &names,
        Limits {
            max_handles: 32,
            admission_steps: 1000,
            expression_depth: 16,
            expression_work: 1000,
        },
    )
    .unwrap();
    let h = ObjectHost::checked(inner)
        .unwrap()
        .with_players(ctx.clone(), slots_rule(301, -3, 9), budget())
        .unwrap();
    (h, ctx)
}
fn seed(h: &mut ObjectHost) -> StartupSettings {
    StartupSettings {
        speed: id(&h.native("ConvertGameSpeed", &[Value::Int(-11)]).unwrap()),
        fog_enabled: false,
        fog_mask_enabled: true,
    }
}
fn read(h: &mut ObjectHost) -> (Value, Value, Value) {
    (
        h.native("GetGameSpeed", &[]).unwrap(),
        h.native("IsFogEnabled", &[]).unwrap(),
        h.native("IsFogMaskEnabled", &[]).unwrap(),
    )
}
#[test]
fn owned_vm_saves_toggles_and_restores_explicit_settings_without_new_handles() {
    let map = "globals
gamespeed Saved = null
boolean Fog = true
boolean Mask = false
endglobals
function save takes nothing returns nothing
set Saved = GetGameSpeed()
set Fog = IsFogEnabled()
set Mask = IsFogMaskEnabled()
call FogEnable(true)
call FogMaskEnable(false)
endfunction
function restore takes nothing returns nothing
call FogEnable(Fog)
call FogMaskEnable(Mask)
endfunction
";
    let (mut h, ctx) = object(&common(), map, true);
    let s = seed(&mut h);
    h.admit_startup_settings(&ctx, s).unwrap();
    for (native, value, fog, mask) in [
        ("FogEnable", true, true, true),
        ("FogMaskEnable", false, true, false),
        ("FogEnable", false, false, false),
        ("FogMaskEnable", true, false, true),
    ] {
        h.native(native, &[Value::Bool(value)]).unwrap();
        assert_eq!(
            read(&mut h),
            (Value::Handle(s.speed), Value::Bool(fog), Value::Bool(mask))
        );
    }
    let mut vm = h.into_vm();
    vm.init_globals().unwrap();
    let handles = vm.host().inner().handles();
    vm.call("save", &[]).unwrap();
    assert_eq!(vm.global("Saved"), Value::Handle(s.speed));
    assert_eq!(vm.global("Fog"), Value::Bool(false));
    assert_eq!(vm.global("Mask"), Value::Bool(true));
    assert_eq!(
        read(vm.host()),
        (
            Value::Handle(s.speed),
            Value::Bool(true),
            Value::Bool(false)
        )
    );
    vm.call("restore", &[]).unwrap();
    assert_eq!(
        read(vm.host()),
        (
            Value::Handle(s.speed),
            Value::Bool(false),
            Value::Bool(true)
        )
    );
    assert_eq!(vm.host().inner().handles(), handles);
}
#[test]
fn invalid_writes_and_failed_replacement_preserve_settings() {
    let (mut h, ctx) = object(&common(), "// map", true);
    let s = seed(&mut h);
    for name in [
        "GetGameSpeed",
        "IsFogEnabled",
        "IsFogMaskEnabled",
        "FogEnable",
        "FogMaskEnable",
    ] {
        assert!(h.native(name, &[]).unwrap_err().contains("unsupported"));
    }
    h.admit_startup_settings(&ctx, s).unwrap();
    let before = read(&mut h);
    let timer = h.native("CreateTimer", &[]).unwrap();
    let other = h.native("ConvertOtherSpeed", &[Value::Int(101)]).unwrap();
    let unknown = HandleId::new(std::num::NonZeroU64::new(u64::MAX).unwrap());
    let handles = h.inner().handles();
    for name in ["FogEnable", "FogMaskEnable"] {
        for args in [
            vec![],
            vec![Value::Null],
            vec![Value::Int(0)],
            vec![timer.clone()],
            vec![Value::Bool(true), Value::Bool(false)],
        ] {
            assert!(h.native(name, &args).is_err());
            assert_eq!(read(&mut h), before);
        }
    }
    for name in ["GetGameSpeed", "IsFogEnabled", "IsFogMaskEnabled"] {
        assert!(h.native(name, &[Value::Null]).is_err());
    }
    for speed in [id(&timer), id(&other), unknown] {
        assert!(
            h.admit_startup_settings(&ctx, StartupSettings { speed, ..s })
                .is_err()
        );
        assert_eq!(read(&mut h), before);
    }
    let wrong = context(301, &common(), "// map", "synthetic:settings");
    assert!(h.admit_startup_settings(&wrong, s).is_err());
    assert_eq!(read(&mut h), before);
    assert_eq!(
        h.native("SetGameSpeed", &[Value::Handle(s.speed)]).unwrap(),
        Value::Null
    );
    assert_eq!(read(&mut h), before);
    assert_eq!(h.inner().handles(), handles);
    let high = id(&h.native("ConvertGameSpeed", &[Value::Int(43)]).unwrap());
    h.admit_startup_settings(
        &ctx,
        StartupSettings {
            speed: high,
            fog_enabled: true,
            fog_mask_enabled: false,
        },
    )
    .unwrap();
    assert_eq!(
        read(&mut h),
        (Value::Handle(high), Value::Bool(true), Value::Bool(false))
    );
    h.invalidate_startup_settings();
    assert!(
        h.native("GetGameSpeed", &[])
            .unwrap_err()
            .contains("unsupported")
    );
    h.admit_startup_settings(&ctx, s).unwrap();
    assert_eq!(read(&mut h), before);
}
#[test]
fn settings_need_selected_conversion_matching_context_and_loaded_native_contracts() {
    let c = common();
    let (mut h, ctx) = object(&c, "// map", false);
    let speed = id(&h.native("ConvertOtherSpeed", &[Value::Int(101)]).unwrap());
    assert!(
        h.admit_startup_settings(
            &ctx,
            StartupSettings {
                speed,
                fog_enabled: false,
                fog_mask_enabled: false
            }
        )
        .is_err()
    );
    let (mut h, ctx) = object(&c, "// map", true);
    let s = seed(&mut h);
    let mut bare = ObjectHost::checked(super::enums(&ctx, 32)).unwrap();
    assert!(bare.admit_startup_settings(&ctx, s).is_err());
    let get = "constant native GetGameSpeed takes nothing returns gamespeed";
    let fog = "native FogEnable takes boolean b returns nothing";
    for c in [
        c.replace(get, ""),
        c.replace(get, &get.replacen("constant ", "", 1)),
        c.replace(
            "IsFogEnabled takes nothing returns boolean",
            "IsFogEnabled takes nothing returns integer",
        ),
        c.replace(
            fog,
            "constant native FogEnable takes boolean b returns nothing",
        ),
        c.replace("FogEnable takes boolean b", "FogEnable takes integer b"),
        c.replace(
            "type gamespeed extends handle",
            "type gamespeed extends agent",
        ) + "\n",
    ] {
        let (mut h, ctx) = object(&c, "// map", true);
        let s = seed(&mut h);
        // An extra authored ancestry edge is valid; native contract changes are not.
        if c.contains("type gamespeed extends agent") {
            h.admit_startup_settings(&ctx, s).unwrap();
        } else {
            assert!(h.admit_startup_settings(&ctx, s).is_err());
        }
    }
    let declaration = "native IsFogEnabled takes nothing returns boolean";
    let moved = common().replace(declaration, "");
    for (c, map) in [
        (moved, declaration.to_string()),
        (common(), declaration.to_string()),
        (
            common(),
            "function IsFogEnabled takes nothing returns boolean\nreturn true\nendfunction\n"
                .into(),
        ),
        (
            common().replace("type gamespeed extends handle", ""),
            "type gamespeed extends handle\n".into(),
        ),
    ] {
        let (mut h, ctx) = object(&c, &map, true);
        let s = seed(&mut h);
        assert!(h.admit_startup_settings(&ctx, s).is_err());
    }
}

#[test]
fn current_speed_roundtrip_is_idempotent_and_other_requests_refuse_atomically() {
    let map = "function keep takes nothing returns nothing\ncall SetGameSpeed(GetGameSpeed())\nendfunction\n";
    let (mut h, ctx) = object(&common(), map, true);
    let s = seed(&mut h);
    h.admit_startup_settings(&ctx, s).unwrap();
    let other = h.native("ConvertGameSpeed", &[Value::Int(43)]).unwrap();
    let foreign = h.native("ConvertOtherSpeed", &[Value::Int(101)]).unwrap();
    let timer = h.native("CreateTimer", &[]).unwrap();
    let before = read(&mut h);
    let handles = h.inner().handles();
    for args in [
        vec![],
        vec![Value::Null],
        vec![Value::Int(-11)],
        vec![other.clone()],
        vec![foreign],
        vec![timer],
        vec![Value::Handle(HandleId::new(
            std::num::NonZeroU64::new(u64::MAX).unwrap(),
        ))],
        vec![Value::Handle(s.speed), Value::Handle(s.speed)],
    ] {
        assert!(h.native("SetGameSpeed", &args).is_err());
        assert_eq!(read(&mut h), before);
    }
    assert_eq!(h.inner().handles(), handles);
    let mut vm = h.into_vm();
    vm.init_globals().unwrap();
    let handles = vm.host().inner().handles();
    vm.call("keep", &[]).unwrap();
    assert_eq!(read(vm.host()), before);
    assert_eq!(vm.host().inner().handles(), handles);
    let high = id(&other);
    vm.host()
        .admit_startup_settings(&ctx, StartupSettings { speed: high, ..s })
        .unwrap();
    vm.call("keep", &[]).unwrap();
    assert_eq!(vm.host().native("GetGameSpeed", &[]).unwrap(), other);
    assert!(
        vm.host()
            .native("SetGameSpeed", &[Value::Handle(s.speed)])
            .is_err()
    );
    vm.host().invalidate_startup_settings();
    assert!(
        vm.host()
            .native("SetGameSpeed", &[Value::Handle(high)])
            .is_err()
    );
}
#[test]
fn optional_speed_setter_requires_its_own_source_and_contract() {
    let declaration = "native SetGameSpeed takes gamespeed s returns nothing";
    let c = common();
    let absent = c.replace(declaration, "");
    let (mut h, ctx) = object(&absent, "// map", true);
    let s = seed(&mut h);
    h.admit_startup_settings(&ctx, s).unwrap();
    assert!(h.native("SetGameSpeed", &[Value::Handle(s.speed)]).is_err());
    for (c, map) in [
        (absent.clone(), declaration.to_owned()),
        (c.clone(), declaration.to_owned()),
        (
            c.clone(),
            "function SetGameSpeed takes gamespeed s returns nothing\nendfunction\n".to_owned(),
        ),
        (
            c.replace(
                declaration,
                "constant native SetGameSpeed takes gamespeed s returns nothing",
            ),
            "// map".to_owned(),
        ),
        (
            c.replace("SetGameSpeed takes gamespeed", "SetGameSpeed takes integer"),
            "// map".to_owned(),
        ),
        (
            c.replace(
                declaration,
                "native SetGameSpeed takes gamespeed s returns integer",
            ),
            "// map".to_owned(),
        ),
    ] {
        let (mut h, ctx) = object(&c, &map, true);
        let s = seed(&mut h);
        assert!(h.admit_startup_settings(&ctx, s).is_err());
    }
}

#[path = "startup_version_tests.rs"]
mod version;

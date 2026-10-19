use super::*;
use crate::playercolor_host::MapConfigBudget;
fn host(c: &str, map: &str) -> (ObjectHost, Arc<ParsedMapContext>) {
    let (h, ctx) = object(c, map, true);
    (
        h.with_map_config(MapConfigBudget {
            text_bytes: 0,
            start_locations: 0,
        })
        .unwrap(),
        ctx,
    )
}
#[test]
fn cinematic_speed_lock_changes_speed_request_behavior_without_changing_speed() {
    let script = "function enter takes nothing returns nothing
call SetMapFlag(MAP_LOCK_SPEED, true)
call SetGameSpeed(High)
endfunction
function leave takes nothing returns nothing
call SetMapFlag(MAP_LOCK_SPEED, false)
endfunction
";
    let (mut h, ctx) = host(&common(), script);
    let s = seed(&mut h);
    h.admit_startup_settings(&ctx, s).unwrap();
    let mut vm = h.into_vm();
    vm.init_globals().unwrap();
    let flag = vm.global("MAP_LOCK_SPEED");
    let high = vm.global("High");
    let before = read(vm.host());
    let handles = vm.host().inner().handles();
    assert!(
        vm.host()
            .native("IsMapFlagSet", std::slice::from_ref(&flag))
            .is_err()
    );
    assert!(
        vm.host()
            .native("SetGameSpeed", std::slice::from_ref(&high))
            .is_err()
    );
    vm.call("enter", &[]).unwrap();
    assert_eq!(
        vm.host()
            .native("IsMapFlagSet", std::slice::from_ref(&flag))
            .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(read(vm.host()), before);
    vm.call("leave", &[]).unwrap();
    assert_eq!(
        vm.host().native("IsMapFlagSet", &[flag]).unwrap(),
        Value::Bool(false)
    );
    assert!(vm.host().native("SetGameSpeed", &[high]).is_err());
    assert_eq!(read(vm.host()), before);
    assert_eq!(vm.host().inner().handles(), handles);
}
#[test]
fn explicit_lock_can_ignore_speed_requests_without_inventing_current_settings() {
    let (h, _) = host(&common(), "// map");
    let mut vm = h.into_vm();
    vm.init_globals().unwrap();
    let flag = vm.global("MAP_LOCK_SPEED");
    let high = vm.global("High");
    vm.host()
        .native("SetMapFlag", &[flag.clone(), Value::Bool(true)])
        .unwrap();
    assert_eq!(
        vm.host()
            .native("SetGameSpeed", std::slice::from_ref(&high))
            .unwrap(),
        Value::Null
    );
    assert!(vm.host().native("GetGameSpeed", &[]).is_err());
    vm.host()
        .native("SetMapFlag", &[flag, Value::Bool(false)])
        .unwrap();
    assert!(vm.host().native("SetGameSpeed", &[high]).is_err());
    assert!(vm.host().native("GetGameSpeed", &[]).is_err());
}
#[test]
fn invalid_flags_arguments_and_locked_speed_inputs_preserve_state() {
    let (h, _) = host(&common(), "// map");
    let mut vm = h.into_vm();
    vm.init_globals().unwrap();
    let flag = vm.global("MAP_LOCK_SPEED");
    let other = vm.global("OTHER_FLAG");
    let speed = vm.global("High");
    let foreign = vm.global("Other");
    let timer = vm.host().native("CreateTimer", &[]).unwrap();
    vm.host()
        .native("SetMapFlag", &[flag.clone(), Value::Bool(true)])
        .unwrap();
    let handles = vm.host().inner().handles();
    for (name, args) in [
        ("SetMapFlag", vec![]),
        ("SetMapFlag", vec![Value::Null, Value::Bool(false)]),
        ("SetMapFlag", vec![flag.clone(), Value::Int(0)]),
        (
            "SetMapFlag",
            vec![flag.clone(), Value::Bool(false), Value::Null],
        ),
        ("SetMapFlag", vec![other.clone(), Value::Bool(false)]),
        ("SetMapFlag", vec![speed, Value::Bool(false)]),
        ("IsMapFlagSet", vec![]),
        ("IsMapFlagSet", vec![other]),
        ("IsMapFlagSet", vec![flag, Value::Null]),
        ("SetGameSpeed", vec![Value::Null]),
        ("SetGameSpeed", vec![foreign]),
        ("SetGameSpeed", vec![timer]),
    ] {
        assert!(vm.host().native(name, &args).is_err(), "{name}");
        assert_eq!(vm.host().map_config().unwrap().speed_locked, Some(true));
    }
    assert_eq!(vm.host().inner().handles(), handles);
}
#[test]
fn speed_lock_needs_source_declarations_and_the_source_global() {
    let c = common();
    let setter = "native SetMapFlag takes mapflag flag, boolean value returns nothing";
    let getter = "native IsMapFlagSet takes mapflag flag returns boolean";
    let global = "constant mapflag MAP_LOCK_SPEED = ConvertMapFlag(7*31)";
    for (c, map, native) in [
        (c.replace(setter, ""), "// map".to_owned(), "SetMapFlag"),
        (c.replace(setter, ""), setter.to_owned(), "SetMapFlag"),
        (c.clone(), setter.to_owned(), "SetMapFlag"),
        (
            c.replace("SetMapFlag takes mapflag", "SetMapFlag takes integer"),
            "// map".to_owned(),
            "SetMapFlag",
        ),
        (
            c.replace(setter, &format!("constant {setter}")),
            "// map".to_owned(),
            "SetMapFlag",
        ),
        (c.replace(getter, ""), "// map".to_owned(), "IsMapFlagSet"),
        (c.replace(getter, ""), getter.to_owned(), "IsMapFlagSet"),
        (
            c.replace(getter, &format!("constant {getter}")),
            "// map".to_owned(),
            "IsMapFlagSet",
        ),
        (
            c.replace(global, ""),
            format!("globals\n{global}\nendglobals"),
            "SetMapFlag",
        ),
        (c.replace(global, ""), "// map".to_owned(), "SetMapFlag"),
        (
            c.replace("type mapflag extends handle", ""),
            "type mapflag extends handle".to_owned(),
            "SetMapFlag",
        ),
    ] {
        let (h, _) = host(&c, &map);
        let mut vm = h.into_vm();
        vm.init_globals().unwrap();
        let flag = vm
            .host()
            .native("ConvertMapFlag", &[Value::Int(7 * 31)])
            .unwrap();
        let args = if native == "SetMapFlag" {
            vec![flag, Value::Bool(true)]
        } else {
            vec![flag]
        };
        assert!(vm.host().native(native, &args).is_err(), "{native}");
        assert_eq!(vm.host().map_config().unwrap().speed_locked, None);
    }
}

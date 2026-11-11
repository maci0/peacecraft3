use super::waygate::open;
use super::*;

const NATIVES: &str = "native GetUnitUserData takes unit whichUnit returns integer
native SetUnitUserData takes unit whichUnit, integer data returns nothing
native RemoveUnit takes unit whichUnit returns nothing
";

const MAP: &str = r#"
globals
unit A = null
unit B = null
endglobals
function setup takes nothing returns nothing
set A = CreateUnit(Player(4), 'nwgt', 0.0, 0.0, 0.0)
set B = CreateUnit(Player(4), 'hfoo', 0.0, 0.0, 0.0)
endfunction
"#;

fn setup() -> (VmHost, Value, Value) {
    let mut s = open(NATIVES, MAP);
    s.vm.init_globals().unwrap();
    s.vm.call("setup", &[]).unwrap();
    let a = s.vm.global("A");
    let b = s.vm.global("B");
    (s, a, b)
}

#[test]
fn user_data_is_kept_per_unit_and_overwritten_by_the_last_write() {
    let (mut s, a, b) = setup();
    let host = &mut s.vm.host().inner;
    host.native("SetUnitUserData", &[a.clone(), Value::Int(-7)])
        .unwrap();
    host.native("SetUnitUserData", &[b.clone(), Value::Int(i32::MAX)])
        .unwrap();
    assert_eq!(
        host.native("GetUnitUserData", std::slice::from_ref(&a))
            .unwrap(),
        Value::Int(-7)
    );
    host.native("SetUnitUserData", &[a.clone(), Value::Int(0)])
        .unwrap();
    assert_eq!(
        host.native("GetUnitUserData", std::slice::from_ref(&a))
            .unwrap(),
        Value::Int(0)
    );
    assert_eq!(
        host.native("GetUnitUserData", std::slice::from_ref(&b))
            .unwrap(),
        Value::Int(i32::MAX)
    );
}

#[test]
fn an_unwritten_value_refuses_and_a_null_unit_reads_zero() {
    let (mut s, a, _) = setup();
    let host = &mut s.vm.host().inner;
    let error = host
        .native("GetUnitUserData", std::slice::from_ref(&a))
        .unwrap_err();
    assert!(
        error.contains("unit user data was never written"),
        "{error}"
    );
    assert_eq!(
        host.native("GetUnitUserData", &[Value::Null]).unwrap(),
        Value::Int(0)
    );
    assert_eq!(
        host.native("SetUnitUserData", &[Value::Null, Value::Int(5)])
            .unwrap(),
        Value::Null
    );
}

#[test]
fn a_removed_unit_reads_zero_and_ignores_writes() {
    let (mut s, a, _) = setup();
    let host = &mut s.vm.host().inner;
    host.native("SetUnitUserData", &[a.clone(), Value::Int(9)])
        .unwrap();
    host.native("RemoveUnit", std::slice::from_ref(&a)).unwrap();
    assert_eq!(
        host.native("GetUnitUserData", std::slice::from_ref(&a))
            .unwrap(),
        Value::Int(0)
    );
    host.native("SetUnitUserData", &[a.clone(), Value::Int(1)])
        .unwrap();
    assert_eq!(
        host.native("GetUnitUserData", std::slice::from_ref(&a))
            .unwrap(),
        Value::Int(0)
    );
}

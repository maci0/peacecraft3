use super::{EXTRAS, common_source, enumerated, source, trigger_limits, unit_limits};
use crate::playercolor_host::{Failure, FilterBudget, ObjectHost, UnitInRangeRegistration};
use pcx_script::{HandleId, Host, Value};
use std::sync::{Arc, atomic::AtomicUsize};

const REGISTER: &str = "TriggerRegisterUnitInRange";
const DECLARATION: &str =
    "native TriggerRegisterUnitInRange takes trigger t, unit u, real r, boolexpr f returns event\n";
const EXTRA: &str = "\
type boolexpr extends agent
type filterfunc extends boolexpr
native Filter takes code predicate returns filterfunc
";
const EMPTY: &str = "function empty takes nothing returns nothing\nendfunction\n";
const MAP: &str = "\
globals
trigger T=null
unit U=null
filterfunc F=null
event A=null
event B=null
integer hits=0
endglobals
function pred takes nothing returns boolean
set hits=hits+1
return false
endfunction
function main takes nothing returns nothing
set T=CreateTrigger()
set U=CreateUnit(Player(4),'hfoo',1.0,2.0,3.0)
set F=Filter(function pred)
set A=TriggerRegisterUnitInRange(T,U,4.0,F)
set B=TriggerRegisterUnitInRange(T,U,-1.0,null)
endfunction
";

fn common() -> String {
    common_source(&format!("{EXTRAS}{EXTRA}{DECLARATION}"))
}

fn objects(host: ObjectHost) -> ObjectHost {
    host.with_unit_catalog(
        Arc::new(super::Counting {
            hits: Arc::new(AtomicUsize::new(0)),
        }),
        source(),
        unit_limits(),
    )
    .unwrap()
    .with_trigger_admission(trigger_limits(4, 0))
    .unwrap()
    .with_filters(FilterBudget { max_records: 2 })
    .unwrap()
}

fn id(value: &Value) -> HandleId {
    let Value::Handle(id) = value else {
        panic!("identity")
    };
    *id
}

#[test]
fn registration_retains_the_call_without_running_the_filter_or_moving_the_unit() {
    let (ctx, host) = enumerated(&common(), MAP, 64);
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(objects(host).with_unit_in_range(2).unwrap());
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    assert_eq!(vm.global("hits"), Value::Int(0));
    let (trigger, unit, filter, first, second) = (
        id(&vm.global("T")),
        id(&vm.global("U")),
        id(&vm.global("F")),
        id(&vm.global("A")),
        id(&vm.global("B")),
    );
    let host = &vm.host().inner;
    assert_eq!(
        host.unit_in_range_registration(first).unwrap(),
        UnitInRangeRegistration {
            trigger,
            unit,
            range: 4.0,
            filter: Some(filter),
        }
    );
    assert_eq!(
        host.unit_in_range_registration(second).unwrap(),
        UnitInRangeRegistration {
            trigger,
            unit,
            range: -1.0,
            filter: None,
        }
    );
    let placed = host.executed_unit(unit).unwrap();
    assert_eq!((placed.x, placed.y, placed.face), (1.0, 2.0, 3.0));
    assert_eq!(host.retained_unit_in_range().unwrap(), 2);
    assert_eq!(host.inner().kind(first).unwrap(), "event");
    assert!(host.trigger_enabled(trigger).unwrap().is_none());
}

fn make_unit(host: &mut ObjectHost, player: i32) -> Value {
    let owner = host.native("Player", &[Value::Int(player)]).unwrap();
    host.native(
        "CreateUnit",
        &[
            owner,
            Value::Int(i32::from_be_bytes(*b"hfoo")),
            Value::Real(1.0),
            Value::Real(2.0),
            Value::Real(3.0),
        ],
    )
    .unwrap()
}

#[test]
fn a_bad_range_or_family_does_not_consume_a_record() {
    let (_, host) = enumerated(&common(), EMPTY, 64);
    let mut host = objects(host).with_unit_in_range(1).unwrap();
    let trigger = host.native("CreateTrigger", &[]).unwrap();
    let unit = make_unit(&mut host, 4);
    let before = host.inner().handles();
    for bad in [
        vec![],
        vec![
            trigger.clone(),
            unit.clone(),
            Value::Real(f32::NAN),
            Value::Null,
        ],
        vec![
            trigger.clone(),
            unit.clone(),
            Value::Real(f32::INFINITY),
            Value::Null,
        ],
        vec![unit.clone(), trigger.clone(), Value::Real(1.0), Value::Null],
        vec![
            trigger.clone(),
            trigger.clone(),
            Value::Real(1.0),
            Value::Null,
        ],
    ] {
        assert!(host.native(REGISTER, &bad).is_err());
        assert_eq!(host.inner().handles(), before);
        assert_eq!(host.retained_unit_in_range().unwrap(), 0);
    }
    let event = host
        .native(REGISTER, &[trigger, unit, Value::Real(4.0), Value::Null])
        .unwrap();
    let other = make_unit(&mut host, 0);
    let other_trigger = host.native("CreateTrigger", &[]).unwrap();
    let occupied = host.inner().handles();
    assert!(
        host.native(
            REGISTER,
            &[other_trigger, other, Value::Real(1.0), Value::Null],
        )
        .unwrap_err()
        .contains("unit-in-range registration")
    );
    assert_eq!(host.inner().handles(), occupied);
    assert_eq!(host.retained_unit_in_range().unwrap(), 1);
    assert!(host.unit_in_range_registration(id(&event)).is_ok());
}

#[test]
fn omission_and_a_foreign_declaration_stay_unsupported() {
    let (_, mut host) = enumerated(&common(), EMPTY, 64);
    host = objects(host);
    let trigger = host.native("CreateTrigger", &[]).unwrap();
    assert!(
        host.native(
            REGISTER,
            &[trigger, Value::Null, Value::Real(1.0), Value::Null],
        )
        .unwrap_err()
        .contains("unsupported")
    );
    assert!(host.retained_unit_in_range().is_err());
    let (_, host) = enumerated(
        &common().replace(DECLARATION, ""),
        &format!("{DECLARATION}{EMPTY}"),
        64,
    );
    let host = objects(host);
    assert!(matches!(
        host.with_unit_in_range(1),
        Err(error) if error.failure == Failure::Source
    ));
}

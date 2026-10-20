use super::*;
use pcx_script::HandleId;
type Vm = pcx_script::OwnedVm<crate::scripts::OwnedLocalizedHost<ObjectHost>>;
const SCENE: &str = r#"
globals
unit A=null
unit B=null
unit C=null
trigger T=null
integer HITS=0
boolean OK=true
endglobals
function enter takes nothing returns boolean
set HITS=HITS+1
set OK=OK and GetFilterUnit()==B and GetTriggerUnit()==B and GetTriggeringTrigger()==null and GetTriggerEventId()==null and GetEnteringUnit()==null
return true
endfunction
function condition takes nothing returns boolean
set HITS=HITS+10
set OK=OK and GetFilterUnit()==B and GetTriggerUnit()==B and GetTriggeringTrigger()==T and GetTriggerEventId()==TEST_RANGE and GetEnteringUnit()==B
return true
endfunction
function action takes nothing returns nothing
set HITS=HITS+100
set OK=OK and GetFilterUnit()==B and GetTriggerUnit()==B and GetTriggeringTrigger()==T and GetTriggerEventId()==TEST_RANGE and GetEnteringUnit()==B
endfunction
function setup takes nothing returns nothing
set A=CreateUnit(Player(4),'U001',16.0,16.0,0.0)
set B=CreateUnit(Player(4),'U001',48.0,16.0,0.0)
set C=CreateUnit(Player(4),'U001',80.0,16.0,0.0)
set T=CreateTrigger()
call TriggerRegisterUnitInRange(T,A,32.0,Filter(function enter))
call TriggerAddCondition(T,Condition(function condition))
call TriggerAddAction(T,function action)
endfunction
"#;
fn declarations() -> String {
    format!(
        "{}\nconstant native GetTriggerUnit takes nothing returns unit\nconstant native GetEnteringUnit takes nothing returns unit\nnative TriggerRegisterUnitInRange takes trigger t, unit u, real r, boolexpr f returns event\nglobals\nconstant unitevent TEST_RANGE=ConvertUnitEvent(-47)\nendglobals\n",
        enumeration::TRIGGERS
    )
}
fn open(pairs: usize) -> Vm {
    open_source(pairs, SCENE)
}
fn open_source(pairs: usize, source: &str) -> Vm {
    let mut h = position_host_program(100, 16, source, &declarations())
        .with_unit_in_range(8)
        .unwrap();
    h.admit_unit_range_runtime("TEST_RANGE", pairs).unwrap();
    let ctx = Arc::clone(h.player_context().unwrap());
    let mut v = Arc::clone(&ctx.scripts).owned_vm(h);
    v.init_globals().unwrap();
    v.call("setup", &[]).unwrap();
    v
}
fn handle(v: &Vm, name: &str) -> HandleId {
    let Value::Handle(id) = v.global(name) else {
        panic!("unit handle")
    };
    id
}
fn place(v: &mut Vm, name: &str, point: [f32; 2]) {
    let id = handle(v, name);
    v.host().inner.range_test_position(id, Some(point));
}
#[test]
fn deferred_entry_has_owned_filter_and_trigger_contexts_without_new_identities() {
    let mut v = open(8);
    let before = v.host().inner.inner().handles();
    assert_eq!(v.global("HITS"), Value::Int(0));
    let mut entries = v.host().inner.poll_unit_ranges(&mut 1000).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(v.global("HITS"), Value::Int(0));
    let filter = v
        .host()
        .inner
        .begin_unit_range_entry(entries.pop().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(
        filter,
        v.host()
            .inner
            .player_context()
            .unwrap()
            .scripts
            .program()
            .function_id("enter")
            .unwrap()
    );
    assert_eq!(v.call("enter", &[]).unwrap(), Value::Bool(true));
    let delivery = v.host().inner.begin_unit_range_trigger().unwrap().unwrap();
    assert_eq!(delivery.trigger, handle(&v, "T"));
    assert_eq!((delivery.conditions.len(), delivery.actions.len()), (1, 1));
    assert_eq!(v.call("condition", &[]).unwrap(), Value::Bool(true));
    v.call("action", &[]).unwrap();
    v.host().inner.finish_unit_range_entry();
    assert_eq!(v.global("HITS"), Value::Int(111));
    assert_eq!(v.global("OK"), Value::Bool(true));
    assert_eq!(v.host().inner.inner().handles(), before);
    for name in [
        "GetTriggerUnit",
        "GetEnteringUnit",
        "GetFilterUnit",
        "GetTriggeringTrigger",
        "GetTriggerEventId",
    ] {
        assert_eq!(v.host().inner.native(name, &[]).unwrap(), Value::Null);
    }
    assert!(
        v.host()
            .inner
            .poll_unit_ranges(&mut 1000)
            .unwrap()
            .is_empty()
    );
}
#[test]
fn final_positions_coalesce_and_only_outside_to_inside_edges_reenter() {
    let mut v = open(8);
    assert_eq!(v.host().inner.poll_unit_ranges(&mut 1000).unwrap().len(), 1);
    place(&mut v, "B", [90., 16.]);
    place(&mut v, "B", [48., 16.]);
    assert!(
        v.host()
            .inner
            .poll_unit_ranges(&mut 1000)
            .unwrap()
            .is_empty()
    );
    place(&mut v, "B", [90., 16.]);
    assert!(
        v.host()
            .inner
            .poll_unit_ranges(&mut 1000)
            .unwrap()
            .is_empty()
    );
    place(&mut v, "B", [48., 16.]);
    assert_eq!(v.host().inner.poll_unit_ranges(&mut 1000).unwrap().len(), 1);
    place(&mut v, "C", [49., 16.]);
    assert_eq!(v.host().inner.poll_unit_ranges(&mut 1000).unwrap().len(), 1);
    place(&mut v, "C", [51., 16.]);
    assert!(
        v.host()
            .inner
            .poll_unit_ranges(&mut 1000)
            .unwrap()
            .is_empty()
    );
    place(&mut v, "C", [49., 16.]);
    assert_eq!(v.host().inner.poll_unit_ranges(&mut 1000).unwrap().len(), 1);
}
#[test]
fn query_and_pair_limits_do_not_commit_partial_membership() {
    let mut v = open(1);
    assert!(v.host().inner.poll_unit_ranges(&mut 1).is_err());
    place(&mut v, "C", [49., 16.]);
    assert!(v.host().inner.poll_unit_ranges(&mut 1000).is_err());
    place(&mut v, "C", [80., 16.]);
    assert_eq!(v.host().inner.poll_unit_ranges(&mut 1000).unwrap().len(), 1);
}
#[test]
fn stale_and_foreign_entries_refuse_even_when_handle_numbers_match() {
    let mut a = open(8);
    let mut b = open(8);
    let e = a
        .host()
        .inner
        .poll_unit_ranges(&mut 1000)
        .unwrap()
        .pop()
        .unwrap();
    assert!(b.host().inner.begin_unit_range_entry(e).is_err());
    place(&mut a, "B", [90., 16.]);
    a.host().inner.poll_unit_ranges(&mut 1000).unwrap();
    place(&mut a, "B", [48., 16.]);
    let e = a
        .host()
        .inner
        .poll_unit_ranges(&mut 1000)
        .unwrap()
        .pop()
        .unwrap();
    a.host().inner.poll_unit_ranges(&mut 1000).unwrap();
    assert!(a.host().inner.begin_unit_range_entry(e).is_err());
}
#[test]
fn destroyed_or_disabled_trigger_keeps_filter_context_but_does_not_deliver_actions() {
    for operation in ["DestroyTrigger", "DisableTrigger"] {
        let mut v = open(8);
        let e = v
            .host()
            .inner
            .poll_unit_ranges(&mut 1000)
            .unwrap()
            .pop()
            .unwrap();
        let t = v.global("T");
        v.host().inner.native(operation, &[t]).unwrap();
        v.host().inner.begin_unit_range_entry(e).unwrap();
        assert_eq!(v.call("enter", &[]).unwrap(), Value::Bool(true));
        assert!(v.host().inner.begin_unit_range_trigger().unwrap().is_none());
        v.host().inner.finish_unit_range_entry();
        assert_eq!(v.global("HITS"), Value::Int(1));
        assert_eq!(v.global("OK"), Value::Bool(true));
    }
}
#[test]
fn missing_accepted_position_and_negative_geometry_refuse_without_request_fallback() {
    let mut v = open(8);
    let id = handle(&v, "B");
    v.host().inner.range_test_position(id, None);
    assert!(
        v.host()
            .inner
            .poll_unit_ranges(&mut 1000)
            .unwrap_err()
            .to_string()
            .contains("accepted range position")
    );
    let mut v = open(8);
    let t = v.global("T");
    let a = v.global("A");
    v.host()
        .inner
        .native(
            "TriggerRegisterUnitInRange",
            &[t, a, Value::Real(-1.), Value::Null],
        )
        .unwrap();
    assert!(
        v.host()
            .inner
            .poll_unit_ranges(&mut 1000)
            .unwrap_err()
            .to_string()
            .contains("negative range geometry")
    );
}

#[test]
fn a_timer_expiry_must_finish_before_a_range_poll() {
    let mut v = open(8);
    let timer = v.host().inner.native("CreateTimer", &[]).unwrap();
    v.host()
        .inner
        .native(
            "TimerStart",
            &[timer, Value::Real(0.5), Value::Bool(false), Value::Null],
        )
        .unwrap();
    let expiry = v
        .host()
        .inner
        .next_timer_expiry(0.5, &mut 1)
        .unwrap()
        .unwrap();
    assert!(
        v.host()
            .inner
            .poll_unit_ranges(&mut 1000)
            .unwrap_err()
            .to_string()
            .contains("completed script boundary")
    );
    v.host().inner.finish_timer_expiry(expiry).unwrap();
    assert_eq!(v.host().inner.poll_unit_ranges(&mut 1000).unwrap().len(), 1);
}

#[test]
fn nested_rectangle_filter_restores_the_range_candidate_and_keeps_trigger_unit() {
    let source=SCENE.replace("set HITS=HITS+100", "set HITS=HITS+100\ncall GroupEnumUnitsInRect(CreateGroup(),Rect(0.0,0.0,100.0,100.0),Filter(function inner))\nset OK=OK and GetFilterUnit()==B")+r#"
function inner takes nothing returns boolean
set OK=OK and GetFilterUnit()!=null and GetTriggerUnit()==B and GetTriggeringTrigger()==T and GetTriggerEventId()==TEST_RANGE and GetEnteringUnit()==B
return false
endfunction
"#;
    let mut v = open_source(8, &source);
    let entry = v
        .host()
        .inner
        .poll_unit_ranges(&mut 1000)
        .unwrap()
        .pop()
        .unwrap();
    v.host().inner.begin_unit_range_entry(entry).unwrap();
    v.call("enter", &[]).unwrap();
    v.host().inner.begin_unit_range_trigger().unwrap();
    v.call("action", &[]).unwrap();
    assert!(v.host().inner.poll_unit_ranges(&mut 1000).is_err());
    v.host().inner.finish_unit_range_entry();
    assert_eq!(v.global("OK"), Value::Bool(true));
}

#[test]
fn a_wrong_event_global_refuses_without_installing_a_partial_runtime() {
    let mut h = position_host_program(100, 16, SCENE, &declarations())
        .with_unit_in_range(8)
        .unwrap();
    let before = h.inner().handles();
    assert!(h.admit_unit_range_runtime("TEST_TIMER", 8).is_err());
    assert!(!h.unit_range_runtime_enabled());
    assert_eq!(h.inner().handles(), before);
    h.admit_unit_range_runtime("TEST_RANGE", 8).unwrap();
    assert!(h.admit_unit_range_runtime("TEST_RANGE", 8).is_err());
}

#[test]
fn optional_entering_unit_getter_is_checked_and_omission_stays_unsupported() {
    let map = "function main takes nothing returns nothing\nendfunction\n";
    let getter = "constant native GetEnteringUnit takes nothing returns unit\n";
    let mut h = position_host_program(100, 16, map, &declarations().replace(getter, ""))
        .with_unit_in_range(8)
        .unwrap();
    h.admit_unit_range_runtime("TEST_RANGE", 8).unwrap();
    assert!(h.native("GetEnteringUnit", &[]).is_err());
    for replacement in [
        "native GetEnteringUnit takes nothing returns unit\n",
        "constant native GetEnteringUnit takes nothing returns real\n",
    ] {
        let mut h =
            position_host_program(100, 16, map, &declarations().replace(getter, replacement))
                .with_unit_in_range(8)
                .unwrap();
        assert!(h.admit_unit_range_runtime("TEST_RANGE", 8).is_err());
        assert!(!h.unit_range_runtime_enabled());
    }
}

#[test]
fn a_filter_named_by_a_unit_in_range_registration_refuses_destruction_and_stays_live() {
    let source = SCENE
        .replace("unit A=null", "boolexpr FL=null\nunit A=null")
        .replace(
            "call TriggerRegisterUnitInRange(T,A,32.0,Filter(function enter))",
            "set FL=Filter(function enter)\ncall TriggerRegisterUnitInRange(T,A,32.0,FL)",
        )
        + "function kill takes nothing returns nothing\ncall DestroyBoolExpr(FL)\nendfunction\n";
    let declarations = format!(
        "{}native DestroyBoolExpr takes boolexpr e returns nothing\n",
        declarations()
    );
    let mut h = position_host_program(100, 16, &source, &declarations)
        .with_unit_in_range(8)
        .unwrap();
    h.admit_unit_range_runtime("TEST_RANGE", 4).unwrap();
    let ctx = Arc::clone(h.player_context().unwrap());
    let mut v = Arc::clone(&ctx.scripts).owned_vm(h);
    v.init_globals().unwrap();
    v.call("setup", &[]).unwrap();
    let Value::Handle(filter) = v.global("FL") else {
        panic!("FL is a filter handle")
    };
    assert!(
        v.call("kill", &[]).is_err(),
        "a named filter refuses destruction"
    );
    assert!(
        v.host().inner.filter_callback(filter).is_ok(),
        "the filter stays live"
    );
}

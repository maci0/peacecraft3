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
fn range_subscription_retains_filter_until_trigger_destruction() {
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
    let pending = v
        .host()
        .inner
        .poll_unit_ranges(&mut 1000)
        .unwrap()
        .pop()
        .unwrap();
    let trigger = v.global("T");
    v.host().inner.native("DestroyTrigger", &[trigger]).unwrap();
    v.call("kill", &[]).unwrap();
    assert!(v.host().inner.filter_callback(filter).is_err());
    assert!(
        v.host()
            .inner
            .begin_unit_range_entry(pending)
            .unwrap()
            .is_some()
    );
    assert_eq!(v.call("enter", &[]).unwrap(), Value::Bool(true));
    assert!(v.host().inner.begin_unit_range_trigger().unwrap().is_none());
    v.host().inner.finish_unit_range_entry();
    let handles = v.host().inner.inner().handles();
    assert!(
        v.host()
            .inner
            .poll_unit_ranges(&mut 1000)
            .unwrap()
            .is_empty()
    );
    assert_eq!(v.host().inner.retained_unit_in_range().unwrap(), 1);
    assert_eq!(v.host().inner.inner().handles(), handles);
    assert_eq!(v.global("HITS"), Value::Int(1));
}

#[test]
fn published_range_entries_use_live_condition_traversal_and_current_actions() {
    use super::super::trigger_admission::{
        CONDITION_MUTATION_CASES, CONDITION_MUTATION_FUNCTIONS,
        CONDITION_SESSION_NEUTRAL_DECLARATIONS, publish_condition_session,
    };
    let declarations = format!(
        "{}\n{}\nnative TriggerRemoveCondition takes trigger t, triggercondition c returns nothing\nnative TriggerClearConditions takes trigger t returns nothing\nnative TriggerClearActions takes trigger t returns nothing\n",
        declarations(),
        CONDITION_SESSION_NEUTRAL_DECLARATIONS
    );
    for allowed in [true, false] {
        for &(setup, trace, actions) in CONDITION_MUTATION_CASES {
            let functions = CONDITION_MUTATION_FUNCTIONS.replace(
                "trigger T = null",
                "trigger T = null\nunit A = null\nunit B = null",
            );
            let map = format!(
                "{functions}\nfunction gate takes nothing returns boolean\nreturn {allowed} and GetFilterUnit() == B and GetTriggeringTrigger() == null\nendfunction\nfunction main takes nothing returns nothing\nset A = CreateUnit(Player(4), 'U001', 16.0, 16.0, 0.0)\nset B = CreateUnit(Player(4), 'U001', 48.0, 16.0, 0.0)\nset T = CreateTrigger()\ncall TriggerAddAction(T, function action)\n{setup}\ncall TriggerRegisterUnitInRange(T, A, 32.0, Filter(function gate))\nendfunction\n"
            );
            let mut host = position_host_program(100, 16, &map, &declarations)
                .with_unit_in_range(8)
                .unwrap();
            host.admit_unit_range_runtime("TEST_RANGE", 8).unwrap();
            let ctx = Arc::clone(host.player_context().unwrap());
            let mut session = publish_condition_session(ctx, host);
            session.advance_unit_ranges(512).unwrap();
            assert_eq!(
                session.global("trace"),
                Some(Value::Int(if allowed { trace } else { 0 })),
                "{setup}"
            );
            assert_eq!(
                session.global("actions"),
                Some(Value::Int(if allowed { actions } else { 0 })),
                "{setup}"
            );
            let host = session.host().unwrap().objects_mut();
            assert_eq!(
                host.native("GetTriggeringTrigger", &[]).unwrap(),
                Value::Null
            );
            assert_eq!(host.native("GetFilterUnit", &[]).unwrap(), Value::Null);
        }
    }
}

#[test]
fn published_region_entries_use_live_condition_traversal_and_current_actions() {
    use super::super::trigger_admission::{
        CONDITION_MUTATION_CASES, CONDITION_MUTATION_FUNCTIONS,
        CONDITION_SESSION_NEUTRAL_DECLARATIONS, publish_condition_session,
    };
    let declarations = format!(
        "{}\n{}\nnative TriggerRemoveCondition takes trigger t, triggercondition c returns nothing\nnative TriggerClearConditions takes trigger t returns nothing\nnative TriggerClearActions takes trigger t returns nothing\ntype region extends agent\nnative CreateRegion takes nothing returns region\nnative RegionAddRect takes region g, rect r returns nothing\nnative TriggerRegisterEnterRegion takes trigger t, region g, boolexpr f returns event\n",
        declarations(),
        CONDITION_SESSION_NEUTRAL_DECLARATIONS
    );
    for &(setup, trace, actions) in CONDITION_MUTATION_CASES {
        let functions = CONDITION_MUTATION_FUNCTIONS
            .replace("trigger T = null", "trigger T = null\nunit B = null");
        let map = format!(
            "{functions}\nfunction gate takes nothing returns boolean\nreturn GetFilterUnit() == B and GetTriggeringTrigger() == null and GetTriggerUnit() == null and GetTriggerEventId() == null\nendfunction\nfunction main takes nothing returns nothing\nlocal region g = CreateRegion()\nset B = CreateUnit(Player(4), 'U001', 48.0, 16.0, 0.0)\nset T = CreateTrigger()\ncall TriggerAddAction(T, function action)\n{setup}\ncall RegionAddRect(g, Rect(72.0, 8.0, 88.0, 24.0))\ncall TriggerRegisterEnterRegion(T, g, Filter(function gate))\nendfunction\n"
        );
        let host = position_host_program(100, 16, &map, &declarations)
            .with_regions(crate::playercolor_host::RegionBudget {
                max_regions: 1,
                max_rect_additions: 1,
                max_events: 1,
            })
            .unwrap();
        let ctx = Arc::clone(host.player_context().unwrap());
        let mut session = publish_condition_session(ctx, host);
        session.advance_unit_ranges(512).unwrap();
        assert_eq!(session.global("trace"), Some(Value::Int(0)));
        let Value::Handle(unit) = session.global("B").unwrap() else {
            panic!("unit")
        };
        session
            .host()
            .unwrap()
            .objects_mut()
            .range_test_position(unit, Some([80., 16.]));
        session.advance_unit_ranges(512).unwrap();
        assert_eq!(session.global("trace"), Some(Value::Int(trace)), "{setup}");
        assert_eq!(
            session.global("actions"),
            Some(Value::Int(actions)),
            "{setup}"
        );
        let host = session.host().unwrap().objects_mut();
        assert_eq!(
            host.native("GetTriggeringTrigger", &[]).unwrap(),
            Value::Null
        );
        assert_eq!(host.native("GetFilterUnit", &[]).unwrap(), Value::Null);
    }
}

#[test]
fn published_region_actions_use_source_event_ids_and_restore_context() {
    use super::super::trigger_admission::{
        CONDITION_SESSION_NEUTRAL_DECLARATIONS, publish_condition_session,
    };
    let declarations = format!(
        "{}\n{}\ntype region extends agent\nnative CreateRegion takes nothing returns region\nnative RegionAddRect takes region g, rect r returns nothing\nnative TriggerRegisterEnterRegion takes trigger t, region g, boolexpr f returns event\nnative TriggerRegisterLeaveRegion takes trigger t, region g, boolexpr f returns event\nglobals\nconstant gameevent EVENT_GAME_ENTER_REGION = ConvertGameEvent(-83)\nconstant gameevent EVENT_GAME_LEAVE_REGION = ConvertGameEvent(-89)\nendglobals\n",
        declarations(),
        CONDITION_SESSION_NEUTRAL_DECLARATIONS
    );
    let map = r#"
globals
unit B = null
trigger ENTER = null
trigger LEAVE = null
integer HITS = 0
boolean OK = true
endglobals
function enter takes nothing returns nothing
set HITS = HITS + 1
set OK = OK and GetTriggerEventId() == EVENT_GAME_ENTER_REGION and GetTriggeringTrigger() == ENTER and GetTriggerUnit() == B and GetEnteringUnit() == B
endfunction
function leave takes nothing returns nothing
set HITS = HITS + 10
set OK = OK and GetTriggerEventId() == EVENT_GAME_LEAVE_REGION and GetTriggeringTrigger() == LEAVE and GetTriggerUnit() == B
endfunction
function main takes nothing returns nothing
local region g = CreateRegion()
set B = CreateUnit(Player(4), 'U001', 48.0, 16.0, 0.0)
set ENTER = CreateTrigger()
set LEAVE = CreateTrigger()
call TriggerAddAction(ENTER, function enter)
call TriggerAddAction(LEAVE, function leave)
call RegionAddRect(g, Rect(72.0, 8.0, 88.0, 24.0))
call TriggerRegisterEnterRegion(ENTER, g, null)
call TriggerRegisterLeaveRegion(LEAVE, g, null)
endfunction
"#;
    let budget = crate::playercolor_host::RegionBudget {
        max_regions: 1,
        max_rect_additions: 1,
        max_events: 2,
    };
    let host = position_host_program(100, 16, map, &declarations)
        .with_regions(budget)
        .unwrap();
    let ctx = Arc::clone(host.player_context().unwrap());
    let mut session = publish_condition_session(ctx, host);
    session.advance_unit_ranges(512).unwrap();
    let Value::Handle(unit) = session.global("B").unwrap() else {
        panic!("unit")
    };
    for (position, hits) in [([80., 16.], 1), ([48., 16.], 11)] {
        session
            .host()
            .unwrap()
            .objects_mut()
            .range_test_position(unit, Some(position));
        session.advance_unit_ranges(512).unwrap();
        assert_eq!(session.global("HITS"), Some(Value::Int(hits)));
        assert_eq!(session.global("OK"), Some(Value::Bool(true)));
        let host = session.host().unwrap().objects_mut();
        for getter in [
            "GetTriggerEventId",
            "GetTriggeringTrigger",
            "GetTriggerUnit",
        ] {
            assert_eq!(host.native(getter, &[]).unwrap(), Value::Null, "{getter}");
        }
    }
}

#[test]
fn published_region_mutations_filter_inline_and_defer_every_crossing() {
    use super::super::trigger_admission::{
        CONDITION_SESSION_NEUTRAL_DECLARATIONS, publish_condition_session,
    };
    let declarations = format!(
        "{}\n{}\n{}\ntype region extends agent\nnative CreateRegion takes nothing returns region\nnative RegionAddRect takes region g, rect r returns nothing\nnative RegionClearRect takes region g, rect r returns nothing\nnative TriggerRegisterEnterRegion takes trigger t, region g, boolexpr f returns event\nnative TriggerRegisterLeaveRegion takes trigger t, region g, boolexpr f returns event\nnative SetUnitPosition takes unit u, real x, real y returns nothing\nnative RemoveUnit takes unit u returns nothing\nnative ShowUnit takes unit u, boolean b returns nothing\nglobals\nconstant gameevent EVENT_GAME_ENTER_REGION=ConvertGameEvent(-83)\nconstant gameevent EVENT_GAME_LEAVE_REGION=ConvertGameEvent(-89)\nendglobals\n",
        declarations(),
        CONDITION_SESSION_NEUTRAL_DECLARATIONS,
        super::activity::ORDERS,
    );
    for (mutation, filters, trace, allowed) in [
        (
            "call SetUnitPosition(A,80.0,16.0)\ncall SetUnitPosition(A,16.0,16.0) // retry",
            2,
            12,
            true,
        ),
        (
            "call SetUnitPosition(A,80.0,16.0)\ncall SetUnitPosition(A,16.0,16.0)",
            2,
            12,
            true,
        ),
        (
            "set C=CreateUnit(Player(4),'U001',80.0,16.0,0.0)",
            1,
            1,
            true,
        ),
        (
            "set C=CreateUnit(Player(4),'U001',80.0,16.0,0.0)\ncall RemoveUnit(C)",
            1,
            1,
            true,
        ),
        (
            "call SetUnitPosition(A,80.0,16.0)\ncall RegionClearRect(G,R)",
            1,
            1,
            true,
        ),
        (
            "call RegionAddRect(G,Rect(8.0,8.0,24.0,24.0))\ncall SetUnitPosition(A,48.0,16.0)",
            1,
            2,
            true,
        ),
        (
            "call RegionClearRect(G,R)\ncall SetUnitPosition(A,80.0,16.0)",
            0,
            0,
            true,
        ),
        (
            "call SetUnitPosition(A,80.0,16.0)\ncall ShowUnit(A,false)",
            1,
            1,
            true,
        ),
        ("call SetUnitPosition(A,80.0,16.0)", 1, 0, false),
        (
            "set NEST=true\ncall SetUnitPosition(A,80.0,16.0)",
            2,
            11,
            true,
        ),
    ] {
        let map = format!(
            r#"
globals
unit A=null
unit B=null
unit C=null
region G=null
rect R=null
trigger IN=null
trigger OUT=null
integer FILTERS=0
integer CONDITIONS=0
integer TRACE=0
boolean OK=true
boolean NEST=false
boolean QUEUED=false
endglobals
function gate takes nothing returns boolean
local unit saved=GetFilterUnit()
set FILTERS=FILTERS+1
if QUEUED then
set OK=OK and TRACE==12 and CONDITIONS==2
endif
if NEST then
set NEST=false
call SetUnitPosition(B,80.0,16.0)
set OK=OK and GetFilterUnit()==saved
endif
set OK=OK and GetFilterUnit()!=null and GetTriggerUnit()==null and GetTriggeringTrigger()==null and GetTriggerEventId()==null
return {allowed}
endfunction
function late takes nothing returns boolean
set CONDITIONS=CONDITIONS+1
return true
endfunction
function enter takes nothing returns nothing
set TRACE=TRACE*10+1
set OK=OK and GetTriggeringTrigger()==IN and GetTriggerEventId()==EVENT_GAME_ENTER_REGION and GetTriggerUnit()!=null
endfunction
function leave takes nothing returns nothing
set TRACE=TRACE*10+2
set OK=OK and GetTriggeringTrigger()==OUT and GetTriggerEventId()==EVENT_GAME_LEAVE_REGION and GetTriggerUnit()!=null
endfunction
function mutate takes nothing returns nothing
{mutation}
set OK=OK and FILTERS=={filters} and CONDITIONS==0 and TRACE==0
endfunction
function main takes nothing returns nothing
set A=CreateUnit(Player(4),'U001',16.0,16.0,0.0)
set B=CreateUnit(Player(4),'U001',112.0,16.0,0.0)
set G=CreateRegion()
set R=Rect(72.0,8.0,88.0,24.0)
call RegionAddRect(G,R)
set IN=CreateTrigger()
set OUT=CreateTrigger()
call TriggerAddAction(IN,function enter)
call TriggerAddAction(OUT,function leave)
call TriggerAddCondition(IN,Condition(function late))
call TriggerAddCondition(OUT,Condition(function late))
call TriggerRegisterEnterRegion(IN,G,Filter(function gate))
call TriggerRegisterLeaveRegion(OUT,G,Filter(function gate))
call mutate()
set NEST=true
set QUEUED=true
endfunction
"#
        );
        let mut host = position_host_program(100, 16, &map, &declarations)
            .with_regions(crate::playercolor_host::RegionBudget {
                max_regions: 1,
                max_rect_additions: 4,
                max_events: 2,
            })
            .unwrap();
        let (registry, rules) = super::activity::rules();
        host = host.with_order_registry(registry).unwrap();
        host.admit_unit_activity(rules, 65536).unwrap();
        let ctx = Arc::clone(host.player_context().unwrap());
        let mut session = publish_condition_session(ctx, host);
        assert_eq!(
            session.global("FILTERS"),
            Some(Value::Int(filters)),
            "{mutation}"
        );
        assert_eq!(
            session.global("CONDITIONS"),
            Some(Value::Int(0)),
            "{mutation}"
        );
        assert_eq!(session.global("OK"), Some(Value::Bool(true)), "{mutation}");
        session.advance_queued_events(512).unwrap();
        assert_eq!(
            session.global("TRACE"),
            Some(Value::Int(trace)),
            "{mutation}"
        );
        assert_eq!(
            session.global("CONDITIONS"),
            Some(Value::Int(if trace == 12 || trace == 11 {
                2
            } else {
                i32::from(trace != 0)
            })),
            "{mutation}"
        );
        assert_eq!(session.global("OK"), Some(Value::Bool(true)), "{mutation}");
        session.advance_unit_ranges(512).unwrap();
        assert_eq!(
            session.global("TRACE"),
            Some(Value::Int(trace)),
            "{mutation}"
        );
        assert_eq!(
            session
                .host()
                .unwrap()
                .objects_mut()
                .native("GetFilterUnit", &[])
                .unwrap(),
            Value::Null
        );
        if trace == 12 {
            let unit = session.global("A").unwrap();
            session
                .host()
                .unwrap()
                .objects_mut()
                .native(
                    "SetUnitPosition",
                    &[unit, Value::Real(80.0), Value::Real(16.0)],
                )
                .unwrap();
            let other = session.global("B").unwrap();
            session
                .host()
                .unwrap()
                .objects_mut()
                .native(
                    "SetUnitPosition",
                    &[other, Value::Real(80.0), Value::Real(16.0)],
                )
                .unwrap();
            assert_eq!(session.global("FILTERS"), Some(Value::Int(2)));
            let retry = mutation.contains("// retry");
            if retry {
                assert!(session.advance_queued_events(2).is_err());
                assert_eq!(session.global("FILTERS"), Some(Value::Int(3)));
                assert_eq!(session.global("TRACE"), Some(Value::Int(12)));
            }
            session.advance_queued_events(512).unwrap();
            assert_eq!(
                session.global("FILTERS"),
                Some(Value::Int(if retry { 3 } else { 4 }))
            );
            assert_eq!(
                session.global("TRACE"),
                Some(Value::Int(if retry { 121 } else { 1211 }))
            );
            assert_eq!(session.global("OK"), Some(Value::Bool(true)));
            assert_eq!(
                session
                    .host()
                    .unwrap()
                    .objects_mut()
                    .native("GetFilterUnit", &[])
                    .unwrap(),
                Value::Null
            );
        }
    }
}

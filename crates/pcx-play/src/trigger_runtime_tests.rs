use super::{EnumHost, Failure, Limits, ObjectHost, TriggerBudget, TriggerRuntimeInput};
use pcx_script::{Host, Prelude, Value};
use std::sync::Arc;
#[path = "game_event_tests.rs"]
mod game_events;

const COMMON: &str = "\
type agent extends handle
type timer extends agent
type trigger extends agent
type event extends agent
type widget extends agent
type unit extends widget
type eventid extends handle
type unitevent extends eventid
type gameevent extends eventid
constant native ConvertUnitEvent takes integer i returns unitevent
constant native ConvertGameEvent takes integer i returns gameevent
native CreateTimer takes nothing returns timer
native DestroyTimer takes timer t returns nothing
native TimerStart takes timer t, real timeout, boolean periodic, code handler returns nothing
native TimerGetElapsed takes timer t returns real
native TimerGetRemaining takes timer t returns real
native TimerGetTimeout takes timer t returns real
native GetExpiredTimer takes nothing returns timer
native CreateTrigger takes nothing returns trigger
native DestroyTrigger takes trigger t returns nothing
native EnableTrigger takes trigger t returns nothing
native DisableTrigger takes trigger t returns nothing
native IsTriggerEnabled takes trigger t returns boolean
native TriggerRegisterUnitEvent takes trigger t, unit u, unitevent e returns event
native TriggerRegisterTimerExpireEvent takes trigger t, timer which returns event
native TriggerRegisterTimerEvent takes trigger t, real timeout, boolean periodic returns event
constant native GetTriggeringTrigger takes nothing returns trigger
constant native GetTriggerEventId takes nothing returns eventid
globals
constant unitevent U = ConvertUnitEvent(63)
constant gameevent TEST_TIMER = ConvertGameEvent(13 + 2)
endglobals
";
fn input(max_timed_events: usize) -> TriggerRuntimeInput {
    TriggerRuntimeInput {
        initial_enabled: true,
        timer_event_global: "TEST_TIMER".into(),
        max_timed_events,
        max_parked_threads: 4,
    }
}
fn host(common: &str, capacity: usize) -> ObjectHost {
    host_map(common, "", capacity, 0)
}
fn host_map(common: &str, map: &str, capacity: usize, max_actions: usize) -> ObjectHost {
    let common = pcx_script::parse_script(common).unwrap();
    let map = pcx_script::parse_script(map).unwrap();
    let program =
        pcx_script::resolve_with(&map, &[Prelude::new("synthetic:trigger-common", &common)])
            .unwrap();
    ObjectHost::checked(
        EnumHost::checked_selected(
            Arc::new(program),
            "synthetic timed event",
            &["ConvertUnitEvent", "ConvertGameEvent"],
            Limits {
                max_handles: capacity,
                admission_steps: 1000,
                expression_depth: 64,
                expression_work: 4096,
            },
        )
        .unwrap(),
    )
    .unwrap()
    .with_trigger_admission(TriggerBudget {
        max_triggers: 8,
        max_unit_events: 0,
        max_actions,
    })
    .unwrap()
    .with_timer_events(8)
    .unwrap()
    .with_timer_runtime(20.0)
    .unwrap()
}
fn register(
    host: &mut ObjectHost,
    trigger: &Value,
    timeout: f32,
    periodic: bool,
) -> Result<Value, String> {
    host.native(
        "TriggerRegisterTimerEvent",
        &[trigger.clone(), Value::Real(timeout), Value::Bool(periodic)],
    )
}

#[test]
fn direct_trigger_callback_declarations_are_checked_before_runtime_admission() {
    for declaration in [
        "native TriggerEvaluate takes trigger t returns real\n",
        "constant native TriggerEvaluate takes trigger t returns boolean\n",
        "native TriggerExecute takes integer t returns nothing\n",
    ] {
        let common = format!("{COMMON}{declaration}");
        assert!(host(&common, 32).with_trigger_runtime(input(2)).is_err());
    }
}

#[test]
fn automatic_event_shares_clock_preserves_period_and_has_no_script_timer() {
    let mut h = host(COMMON, 32).with_trigger_runtime(input(2)).unwrap();
    let trigger = h.native("CreateTrigger", &[]).unwrap();
    assert_eq!(
        h.native("IsTriggerEnabled", std::slice::from_ref(&trigger))
            .unwrap(),
        Value::Bool(true)
    );
    let event = register(&mut h, &trigger, 0.5, true).unwrap();
    let Value::Handle(id) = event else {
        panic!("event")
    };
    assert_eq!(h.inner().kind(id).unwrap(), "event");
    let handles = h.inner().handles();
    assert_eq!(h.live_timers(), 0);
    assert!(h.next_timer_expiry(21.0, &mut 0).is_err());
    assert_eq!(h.timer_game_time(), Some(20.0));
    h.native("DisableTrigger", std::slice::from_ref(&trigger))
        .unwrap();
    let expiry = h.next_timer_expiry(20.75, &mut 1).unwrap().unwrap();
    assert!(expiry.triggers().is_empty());
    assert_eq!(h.native("GetExpiredTimer", &[]).unwrap(), Value::Null);
    h.finish_timer_expiry(expiry).unwrap();
    h.native("EnableTrigger", std::slice::from_ref(&trigger))
        .unwrap();
    let expiry = h.next_timer_expiry(21.0, &mut 1).unwrap().unwrap();
    assert_eq!(h.timer_game_time(), Some(21.0));
    assert!(h.begin_timer_trigger(&expiry, 0).unwrap());
    assert_eq!(h.native("GetTriggeringTrigger", &[]).unwrap(), trigger);
    let Value::Handle(event_id) = h.native("GetTriggerEventId", &[]).unwrap() else {
        panic!("event ID")
    };
    assert_eq!(h.inner().symbolic_value(event_id, "gameevent").unwrap(), 15);
    h.native("DestroyTrigger", &[trigger]).unwrap();
    assert_eq!(
        h.native("GetTriggeringTrigger", &[]).unwrap(),
        Value::Handle(expiry.triggers()[0].trigger)
    );
    h.finish_timer_expiry(expiry).unwrap();
    assert_eq!(h.native("GetTriggerEventId", &[]).unwrap(), Value::Null);
    assert!(h.next_timer_expiry(100.0, &mut 0).unwrap().is_none());
    assert_eq!(h.inner().handles(), handles);
}

#[test]
fn registration_refusals_are_atomic_and_default_is_explicit() {
    let mut i = input(1);
    i.initial_enabled = false;
    let mut h = host(COMMON, 32).with_trigger_runtime(i).unwrap();
    let t = h.native("CreateTrigger", &[]).unwrap();
    assert_eq!(
        h.native("IsTriggerEnabled", std::slice::from_ref(&t))
            .unwrap(),
        Value::Bool(false)
    );
    let before = h.inner().handles();
    for n in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert!(register(&mut h, &t, n, true).is_err());
    }
    let timer = h.native("CreateTimer", &[]).unwrap();
    assert!(
        register(&mut h, &timer, 1.0, false)
            .unwrap_err()
            .contains("TriggerRegisterTimerEvent at")
    );
    assert!(
        h.native("DestroyTrigger", std::slice::from_ref(&timer))
            .unwrap_err()
            .contains("DestroyTrigger at")
    );
    assert_eq!(h.inner().handles(), before + 1);
    register(&mut h, &t, 1.0, false).unwrap();
    let before = h.inner().handles();
    assert!(
        register(&mut h, &t, 2.0, false)
            .unwrap_err()
            .contains("timed trigger registration")
    );
    assert_eq!(h.inner().handles(), before);
    h.native("DestroyTrigger", std::slice::from_ref(&t))
        .unwrap();
    assert!(register(&mut h, &t, 1.0, false).is_err());
    assert!(h.native("EnableTrigger", &[t]).is_err());
    let mut h = host(COMMON, 2).with_trigger_runtime(input(1)).unwrap();
    let t = h.native("CreateTrigger", &[]).unwrap();
    assert!(
        register(&mut h, &t, 1.0, false)
            .unwrap_err()
            .contains("identity capacity")
    );
    assert!(h.next_timer_expiry(30.0, &mut 0).unwrap().is_none());
}

#[test]
fn admission_checks_selected_source_global_types_contracts_and_lifecycle() {
    for (from, to) in [
        (
            "native DestroyTrigger takes trigger t returns nothing",
            "native DestroyTrigger takes timer t returns nothing",
        ),
        (
            "constant native GetTriggerEventId",
            "native GetTriggerEventId",
        ),
        (
            "type gameevent extends eventid",
            "type gameevent extends handle",
        ),
    ] {
        assert!(
            host(&COMMON.replace(from, to), 32)
                .with_trigger_runtime(input(2))
                .is_err()
        );
    }
    let mut i = input(2);
    i.timer_event_global = "U".into();
    assert!(host(COMMON, 32).with_trigger_runtime(i).is_err());
    assert!(
        host(COMMON, 32)
            .with_trigger_runtime(input(usize::MAX))
            .is_err()
    );
    assert!(
        host(COMMON, 32)
            .with_trigger_runtime(input(2))
            .unwrap()
            .with_trigger_runtime(input(2))
            .is_err()
    );
    let mut h = host(COMMON, 32);
    let t = h.native("CreateTrigger", &[]).unwrap();
    assert!(register(&mut h, &t, 1.0, false).is_err());
    assert!(
        matches!(h.with_trigger_runtime(input(2)), Err(e) if matches!(e.failure, Failure::TriggerState(_)))
    );
}

const SLEEP_COMMON: &str = "native TriggerAddAction takes trigger t, code f returns triggeraction
native TriggerExecute takes trigger t returns nothing
native TriggerEvaluate takes trigger t returns boolean
native TriggerSleepAction takes real r returns nothing
native Mark takes integer i returns nothing
";
const SLEEP_MAP: &str = "globals
trigger T = null
integer g = 0
endglobals
function act takes nothing returns nothing
    set g = g + 1
    call TriggerSleepAction(2.5)
    if GetTriggeringTrigger() == T then
        set g = g + 10
    endif
endfunction
function second takes nothing returns nothing
    set g = g + 100
endfunction
function main takes nothing returns nothing
    set T = CreateTrigger()
    call TriggerAddAction(T, function act)
    call TriggerAddAction(T, function second)
    call TriggerExecute(T)
    set g = g + 1000
endfunction
";

#[test]
fn sleeping_actions_park_their_own_thread_and_resume_with_their_trigger() {
    let common = format!("{COMMON}type triggeraction extends handle\n{SLEEP_COMMON}");
    let program = {
        let mut h = host_map(&common, SLEEP_MAP, 32, 8)
            .with_trigger_runtime(input(2))
            .unwrap();
        h.admit_thread_sleep(1).unwrap();
        assert!(h.admit_thread_sleep(1).is_err());
        h
    };
    let shared = Arc::clone(&program.inner().program);
    let mut vm = pcx_script::OwnedVm::new_owned(shared, program);
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    // The caller continued while the action thread slept; its tail has not run.
    assert_eq!(vm.global("g"), Value::Int(1001));
    assert_eq!(
        vm.host().native("GetTriggeringTrigger", &[]).unwrap(),
        Value::Null
    );
    assert_eq!(vm.host().parked_thread_count(), 1);
    assert_eq!(vm.host().next_thread_wake(22.0).unwrap(), None);
    assert_eq!(vm.host().timer_game_time(), Some(20.0));
    let ticket = vm.host().next_thread_wake(30.0).unwrap().unwrap();
    assert_eq!(vm.host().timer_game_time(), Some(22.5));
    vm.resume_thread(ticket).unwrap();
    assert_eq!(vm.global("g"), Value::Int(1111));
    assert_eq!(vm.host().parked_thread_count(), 0);
    assert_eq!(
        vm.host().native("GetTriggeringTrigger", &[]).unwrap(),
        Value::Null
    );
    // A second park beyond the admitted limit refuses, as do invalid durations.
    vm.call("main", &[]).unwrap();
    let error = vm.call("main", &[]).unwrap_err().message;
    assert!(error.contains("parked threads"), "{error}");
    assert!(
        vm.host()
            .native("TriggerSleepAction", &[Value::Real(f32::NAN)])
            .is_err()
    );
}

#[test]
fn sleeping_actions_retain_expired_timer_through_nested_calls_and_repark() {
    let common = format!("{COMMON}type triggeraction extends handle\n{SLEEP_COMMON}");
    let source = "\
globals
timer clock = CreateTimer()
trigger sleeper = CreateTrigger()
trigger nested = CreateTrigger()
boolean before = false
boolean after = false
boolean inner = false
boolean again = false
integer starts = 0
integer tails = 0
real elapsed = 0.0
endglobals
function nested_action takes nothing returns nothing
set inner = GetExpiredTimer() == clock
endfunction
function act takes nothing returns nothing
local timer retained = GetExpiredTimer()
set starts = starts + 1
set before = retained == clock
call TriggerSleepAction(2.5)
set after = GetExpiredTimer() == retained
set elapsed = TimerGetElapsed(retained)
call TriggerExecute(nested)
call TriggerSleepAction(1.0)
set again = GetExpiredTimer() == retained
endfunction
function tail takes nothing returns nothing
set tails = tails + 1
endfunction
function begin takes nothing returns nothing
call TriggerExecute(sleeper)
endfunction
function main takes nothing returns nothing
call TriggerAddAction(sleeper,function act)
call TriggerAddAction(sleeper,function tail)
call TriggerAddAction(nested,function nested_action)
call TimerStart(clock,1.0,false,function begin)
endfunction
";
    let mut host = host_map(&common, source, 64, 4)
        .with_trigger_runtime(input(4))
        .unwrap();
    host.admit_thread_sleep(4).unwrap();
    let program = Arc::clone(&host.inner().program);
    let mut vm = pcx_script::OwnedVm::new_owned(Arc::clone(&program), host);
    vm.call("main", &[]).unwrap();
    let expiry = vm.host().next_timer_expiry(21.0, &mut 1).unwrap().unwrap();
    let callback = &program.functions()[expiry.callback.unwrap().index()].name;
    vm.call(callback, &[]).unwrap();
    vm.host().finish_timer_expiry(expiry).unwrap();
    assert_eq!(vm.global("before"), Value::Bool(true));
    assert!(vm.host().native("GetExpiredTimer", &[]).is_err());

    let ticket = vm.host().next_thread_wake(30.0).unwrap().unwrap();
    vm.resume_thread(ticket).unwrap();
    assert_eq!(vm.global("after"), Value::Bool(true));
    assert_eq!(vm.global("inner"), Value::Bool(true));
    assert_eq!(vm.global("elapsed"), Value::Real(1.0));
    assert_eq!(vm.global("tails"), Value::Int(0));
    assert_eq!(vm.host().timer_game_time(), Some(23.5));
    assert!(vm.host().native("GetExpiredTimer", &[]).is_err());

    let ticket = vm.host().next_thread_wake(30.0).unwrap().unwrap();
    vm.resume_thread(ticket).unwrap();
    assert_eq!(vm.global("again"), Value::Bool(true));
    assert_eq!(vm.global("starts"), Value::Int(1));
    assert_eq!(vm.global("tails"), Value::Int(1));
    assert_eq!(vm.host().timer_game_time(), Some(24.5));
    assert_eq!(vm.host().parked_thread_count(), 0);
    assert!(vm.host().native("GetExpiredTimer", &[]).is_err());
}

#[test]
fn sleeping_timed_event_keeps_null_expiry_without_masking_its_caller() {
    let common = format!("{COMMON}type triggeraction extends handle\n{SLEEP_COMMON}");
    let source = "\
globals
trigger sleeper = CreateTrigger()
timer outer = CreateTimer()
boolean before = false
boolean after = false
endglobals
function act takes nothing returns nothing
set before = GetExpiredTimer() == null
call TriggerSleepAction(2.5)
set after = GetExpiredTimer() == null
endfunction
function main takes nothing returns nothing
call TriggerAddAction(sleeper,function act)
call TriggerRegisterTimerEvent(sleeper,1.0,false)
endfunction
";
    let mut host = host_map(&common, source, 64, 2)
        .with_trigger_runtime(input(4))
        .unwrap();
    host.admit_thread_sleep(4).unwrap();
    let program = Arc::clone(&host.inner().program);
    let mut vm = pcx_script::OwnedVm::new_owned(program, host);
    vm.call("main", &[]).unwrap();
    let expiry = vm.host().next_timer_expiry(21.0, &mut 1).unwrap().unwrap();
    assert!(vm.host().begin_timer_trigger(&expiry, 0).unwrap());
    vm.run_thread(&expiry.triggers()[0].actions).unwrap();
    vm.host().finish_timer_expiry(expiry).unwrap();
    assert_eq!(vm.global("before"), Value::Bool(true));
    let ticket = vm.host().next_thread_wake(30.0).unwrap().unwrap();

    let outer = vm.global("outer");
    vm.host()
        .native(
            "TimerStart",
            &[
                outer.clone(),
                Value::Real(0.5),
                Value::Bool(false),
                Value::Null,
            ],
        )
        .unwrap();
    let expiry = vm.host().next_timer_expiry(24.0, &mut 1).unwrap().unwrap();
    assert_eq!(vm.host().native("GetExpiredTimer", &[]).unwrap(), outer);
    vm.resume_thread(ticket).unwrap();
    assert_eq!(vm.global("after"), Value::Bool(true));
    assert_eq!(vm.host().timer_game_time(), Some(24.0));
    assert_eq!(vm.host().native("GetExpiredTimer", &[]).unwrap(), outer);
    vm.host().finish_timer_expiry(expiry).unwrap();
    assert!(vm.host().native("GetExpiredTimer", &[]).is_err());
}

#[test]
fn resumed_expired_timer_context_unwinds_on_script_failure() {
    let common = format!("{COMMON}type triggeraction extends handle\n{SLEEP_COMMON}");
    let source = "\
globals
timer clock = CreateTimer()
trigger sleeper = CreateTrigger()
boolean retained = false
endglobals
function act takes nothing returns nothing
call TriggerSleepAction(2.5)
set retained = GetExpiredTimer() == clock
call TimerStart(null,1.0,false,null)
endfunction
function begin takes nothing returns nothing
call TriggerExecute(sleeper)
endfunction
function main takes nothing returns nothing
call TriggerAddAction(sleeper,function act)
call TimerStart(clock,1.0,false,function begin)
endfunction
";
    let mut host = host_map(&common, source, 64, 2)
        .with_trigger_runtime(input(4))
        .unwrap();
    host.admit_thread_sleep(4).unwrap();
    let program = Arc::clone(&host.inner().program);
    let mut vm = pcx_script::OwnedVm::new_owned(Arc::clone(&program), host);
    vm.call("main", &[]).unwrap();
    let expiry = vm.host().next_timer_expiry(21.0, &mut 1).unwrap().unwrap();
    let callback = &program.functions()[expiry.callback.unwrap().index()].name;
    vm.call(callback, &[]).unwrap();
    vm.host().finish_timer_expiry(expiry).unwrap();
    let ticket = vm.host().next_thread_wake(30.0).unwrap().unwrap();
    assert!(vm.resume_thread(ticket).is_err());
    assert_eq!(vm.global("retained"), Value::Bool(true));
    assert_eq!(vm.host().parked_thread_count(), 0);
    assert!(vm.host().native("GetExpiredTimer", &[]).is_err());
    assert_eq!(
        vm.host().native("GetTriggeringTrigger", &[]).unwrap(),
        Value::Null
    );
}

#[test]
fn variable_event_queues_when_the_real_changes_across_the_limit() {
    let common = format!(
        "{COMMON}\ntype limitop extends handle\nconstant native ConvertLimitOp takes integer i returns limitop\nglobals\nconstant limitop LESS_THAN=ConvertLimitOp(0)\nconstant limitop LESS_THAN_OR_EQUAL=ConvertLimitOp(1)\nconstant limitop EQUAL=ConvertLimitOp(2)\nconstant limitop GREATER_THAN_OR_EQUAL=ConvertLimitOp(3)\nconstant limitop GREATER_THAN=ConvertLimitOp(4)\nconstant limitop NOT_EQUAL=ConvertLimitOp(5)\nendglobals\n"
    );
    let map = "globals\nreal udg_ArthasDenials=0\nendglobals\n";
    let common = pcx_script::parse_script(&common).unwrap();
    let map = pcx_script::parse_script(map).unwrap();
    let program =
        pcx_script::resolve_with(&map, &[Prelude::new("synthetic:trigger-common", &common)])
            .unwrap();
    let mut h = ObjectHost::checked(
        EnumHost::checked_selected(
            Arc::new(program),
            "synthetic variable event",
            &["ConvertUnitEvent", "ConvertGameEvent", "ConvertLimitOp"],
            Limits {
                max_handles: 32,
                admission_steps: 1000,
                expression_depth: 64,
                expression_work: 4096,
            },
        )
        .unwrap(),
    )
    .unwrap()
    .with_trigger_admission(TriggerBudget {
        max_triggers: 8,
        max_unit_events: 0,
        max_actions: 4,
    })
    .unwrap()
    .with_timer_events(8)
    .unwrap()
    .with_timer_runtime(20.0)
    .unwrap()
    .with_trigger_runtime(input(2))
    .unwrap();
    let trigger = h.native("CreateTrigger", &[]).unwrap();
    let op = h.native("ConvertLimitOp", &[Value::Int(3)]).unwrap();
    let event = h
        .native(
            "TriggerRegisterVariableEvent",
            &[
                trigger.clone(),
                Value::Str("udg_ArthasDenials".into()),
                op,
                Value::Real(21.0),
            ],
        )
        .unwrap();
    let Value::Handle(event_id) = event else {
        panic!("event")
    };
    let Value::Handle(trigger_id) = trigger else {
        panic!("trigger")
    };
    let mut work = 8;
    h.poll_variable_events(&[(event_id, Value::Real(0.0))], &mut work)
        .unwrap();
    assert!(h.take_fired_event().is_none());
    h.poll_variable_events(&[(event_id, Value::Real(21.0))], &mut work)
        .unwrap();
    assert_eq!(h.take_fired_event().unwrap().trigger, trigger_id);
    h.poll_variable_events(&[(event_id, Value::Real(22.0))], &mut work)
        .unwrap();
    assert_eq!(h.take_fired_event().unwrap().trigger, trigger_id);
    h.poll_variable_events(&[(event_id, Value::Real(22.0))], &mut work)
        .unwrap();
    assert!(h.take_fired_event().is_none());
}

#[test]
fn condition_edits_change_subsequent_owned_vm_evaluations() {
    let common = format!(
        "{COMMON}\n\
type boolexpr extends agent
 type conditionfunc extends boolexpr
 type triggercondition extends agent
 native Condition takes code f returns conditionfunc
 native TriggerAddCondition takes trigger t, boolexpr c returns triggercondition
 native TriggerRemoveCondition takes trigger t, triggercondition c returns nothing
 native TriggerClearConditions takes trigger t returns nothing
 native TriggerEvaluate takes trigger t returns boolean
"
    );
    let map = "\
function predicate takes nothing returns boolean
return false
endfunction
function main takes nothing returns boolean
local trigger t = CreateTrigger()
local boolexpr c = Condition(function predicate)
local triggercondition r = TriggerAddCondition(t, c)
if TriggerEvaluate(t) then
return false
endif
call TriggerRemoveCondition(t, r)
if not TriggerEvaluate(t) then
return false
endif
set r = TriggerAddCondition(t, c)
call TriggerClearConditions(t)
return TriggerEvaluate(t)
endfunction
";
    let h = host_map(&common, map, 32, 0)
        .with_conditions(super::ConditionBudget { max_records: 1 })
        .unwrap()
        .with_trigger_conditions(1)
        .unwrap()
        .with_trigger_runtime(input(0))
        .unwrap();
    let program = Arc::clone(&h.inner().program);
    let mut vm = pcx_script::Vm::new(&program, h);
    vm.init_globals().unwrap();
    assert_eq!(vm.call("main", &[]).unwrap(), Value::Bool(true));
    let coverage = vm.host().native_coverage();
    for name in ["TriggerRemoveCondition", "TriggerClearConditions"] {
        assert!(coverage.supported.iter().any(|entry| entry == name));
        assert!(!coverage.unsupported.iter().any(|entry| entry == name));
    }
}

const ACTION_EDIT_COMMON: &str = "\
type triggeraction extends handle
native TriggerAddAction takes trigger t, code f returns triggeraction
native TriggerRemoveAction takes trigger t, triggeraction a returns nothing
native TriggerClearActions takes trigger t returns nothing
native TriggerExecute takes trigger t returns nothing
";
const ACTION_EDIT_MAP: &str = "\
globals
trigger T = null
trigger other = null
triggeraction A = null
triggeraction B = null
triggeraction C = null
integer trace = 0
endglobals
function first takes nothing returns nothing
set trace = trace * 10 + 1
endfunction
function second takes nothing returns nothing
set trace = trace * 10 + 2
endfunction
function third takes nothing returns nothing
set trace = trace * 10 + 3
endfunction
function prepare takes nothing returns nothing
set T = CreateTrigger()
set other = CreateTrigger()
set A = TriggerAddAction(T, function first)
set B = TriggerAddAction(T, function second)
set C = TriggerAddAction(other, function third)
endfunction
function main takes nothing returns nothing
call prepare()
call TriggerRemoveAction(T, A)
call TriggerExecute(T)
call TriggerExecute(other)
set A = TriggerAddAction(T, function first)
call TriggerExecute(T)
call TriggerClearActions(T)
call TriggerClearActions(T)
call TriggerExecute(T)
call TriggerExecute(other)
endfunction
";

#[test]
fn action_removal_and_clear_change_later_owned_vm_execution() {
    let common = format!("{COMMON}{ACTION_EDIT_COMMON}");
    let h = host_map(&common, ACTION_EDIT_MAP, 32, 3)
        .with_trigger_runtime(input(2))
        .unwrap();
    let program = Arc::clone(&h.inner().program);
    let mut vm = pcx_script::OwnedVm::new_owned(program, h);
    vm.call("main", &[]).unwrap();
    assert_eq!(vm.global("trace"), Value::Int(23213));
    let coverage = vm.host().native_coverage();
    for name in ["TriggerRemoveAction", "TriggerClearActions"] {
        assert!(coverage.supported.iter().any(|entry| entry == name));
        assert!(!coverage.unsupported.iter().any(|entry| entry == name));
    }

    let Value::Handle(a) = vm.global("A") else {
        panic!("action")
    };
    let Value::Handle(c) = vm.global("C") else {
        panic!("action")
    };
    assert!(a.token() > c.token());
    assert!(vm.host().trigger_action(a).is_err());
    assert!(vm.host().trigger_action(c).is_ok());
}

#[test]
fn removed_actions_are_absent_from_future_timed_delivery() {
    let common = format!("{COMMON}{ACTION_EDIT_COMMON}");
    let h = host_map(&common, ACTION_EDIT_MAP, 32, 3)
        .with_trigger_runtime(input(2))
        .unwrap();
    let program = Arc::clone(&h.inner().program);
    let mut vm = pcx_script::OwnedVm::new_owned(program, h);
    vm.call("prepare", &[]).unwrap();
    let t = vm.global("T");
    let a = vm.global("A");
    vm.host()
        .native("TriggerRemoveAction", &[t.clone(), a])
        .unwrap();
    register(vm.host(), &t, 1.0, true).unwrap();
    let expiry = vm.host().next_timer_expiry(21.0, &mut 1).unwrap().unwrap();
    assert_eq!(expiry.triggers()[0].actions.len(), 1);
    assert!(vm.host().begin_timer_trigger(&expiry, 0).unwrap());
    vm.run_thread(&expiry.triggers()[0].actions).unwrap();
    vm.host().finish_timer_expiry(expiry).unwrap();
    assert_eq!(vm.global("trace"), Value::Int(2));
    vm.host()
        .native("TriggerClearActions", std::slice::from_ref(&t))
        .unwrap();
    let expiry = vm.host().next_timer_expiry(22.0, &mut 1).unwrap().unwrap();
    assert!(expiry.triggers()[0].actions.is_empty());
    vm.host().finish_timer_expiry(expiry).unwrap();
    assert_eq!(vm.global("trace"), Value::Int(2));
}

#[test]
fn invalid_action_removal_preserves_registrations_and_capacity() {
    let common = format!("{COMMON}{ACTION_EDIT_COMMON}");
    let h = host_map(&common, ACTION_EDIT_MAP, 32, 3)
        .with_trigger_runtime(input(2))
        .unwrap();
    let program = Arc::clone(&h.inner().program);
    let mut vm = pcx_script::OwnedVm::new_owned(program, h);
    vm.call("prepare", &[]).unwrap();
    let t = vm.global("T");
    let other = vm.global("other");
    let a = vm.global("A");
    let before = vm.host().inner().handles();
    for (name, args) in [
        ("TriggerRemoveAction", vec![]),
        ("TriggerRemoveAction", vec![other, a.clone()]),
        ("TriggerRemoveAction", vec![t.clone(), t.clone()]),
        ("TriggerRemoveAction", vec![t.clone(), Value::Null]),
        ("TriggerClearActions", vec![Value::Null]),
        ("TriggerClearActions", vec![a.clone()]),
        ("TriggerClearActions", vec![t.clone(), a.clone()]),
    ] {
        let error = vm.host().native(name, &args).unwrap_err();
        assert!(error.contains(name), "{error}");
        assert_eq!(vm.host().inner().handles(), before);
    }
    let callback = vm.host().inner().program.function_id("first").unwrap();
    assert!(
        vm.host()
            .native("TriggerAddAction", &[t.clone(), Value::Code(callback)])
            .unwrap_err()
            .contains("trigger action policy exceeded")
    );
    let Value::Handle(a_id) = a else {
        panic!("action")
    };
    assert!(vm.host().trigger_action(a_id).is_ok());
    vm.host()
        .native("TriggerRemoveAction", &[t.clone(), a.clone()])
        .unwrap();
    assert!(
        vm.host()
            .native("TriggerRemoveAction", &[t, a])
            .unwrap_err()
            .contains("identity")
    );
    let dead = vm.global("other");
    vm.host()
        .native("DestroyTrigger", std::slice::from_ref(&dead))
        .unwrap();
    assert!(vm.host().native("TriggerClearActions", &[dead]).is_err());
}

#[test]
fn trigger_destruction_releases_registrations_but_preserves_dispatched_tail() {
    let common = format!(
        "{COMMON}{ACTION_EDIT_COMMON}\n\
type boolexpr extends agent
type conditionfunc extends boolexpr
type filterfunc extends boolexpr
type triggercondition extends agent
native Condition takes code f returns conditionfunc
native Filter takes code f returns filterfunc
native DestroyBoolExpr takes boolexpr c returns nothing
native TriggerAddCondition takes trigger t, boolexpr c returns triggercondition
"
    );
    let map = "\
globals
trigger T = null
trigger other = null
boolexpr C = null
triggercondition old_condition = null
triggeraction old_action = null
triggeraction old_second = null
integer trace = 0
endglobals
function predicate takes nothing returns boolean
return true
endfunction
function first takes nothing returns nothing
call DestroyTrigger(T)
set trace = trace * 10 + 1
endfunction
function second takes nothing returns nothing
set trace = trace * 10 + 2
endfunction
function third takes nothing returns nothing
set trace = trace * 10 + 3
endfunction
function main takes nothing returns nothing
set T = CreateTrigger()
set other = CreateTrigger()
set C = Condition(function predicate)
set old_condition = TriggerAddCondition(T, C)
set old_action = TriggerAddAction(T, function first)
set old_second = TriggerAddAction(T, function second)
call TriggerExecute(T)
call DestroyBoolExpr(C)
set C = Condition(function predicate)
call TriggerAddCondition(other, C)
call TriggerAddAction(other, function third)
call TriggerAddAction(other, function third)
call TriggerExecute(other)
endfunction
";
    let h = host_map(&common, map, 32, 2)
        .with_filters(super::FilterBudget { max_records: 0 })
        .unwrap()
        .with_conditions(super::ConditionBudget { max_records: 2 })
        .unwrap()
        .with_trigger_conditions(1)
        .unwrap()
        .with_trigger_runtime(input(0))
        .unwrap();
    let program = Arc::clone(&h.inner().program);
    let mut vm = pcx_script::OwnedVm::new_owned(program, h);
    vm.call("main", &[]).unwrap();
    assert_eq!(vm.global("trace"), Value::Int(1233));
    assert_eq!(vm.host().retained_trigger_conditions().unwrap(), 1);
    let Value::Handle(action) = vm.global("old_action") else {
        panic!("action")
    };
    let Value::Handle(condition) = vm.global("old_condition") else {
        panic!("condition")
    };
    assert!(vm.host().trigger_action(action).is_err());
    let Value::Handle(second) = vm.global("old_second") else {
        panic!("action")
    };
    assert!(vm.host().trigger_action(second).is_err());
    assert!(vm.host().trigger_condition_registration(condition).is_err());
}

#[test]
fn trigger_destruction_detaches_each_event_filter_only_from_its_owner() {
    let common = format!(
        "{COMMON}\n\
type player extends agent
type region extends agent
type playerunitevent extends eventid
type boolexpr extends agent
type filterfunc extends boolexpr
native Filter takes code f returns filterfunc
native DestroyBoolExpr takes boolexpr f returns nothing
"
    );
    let map = "function predicate takes nothing returns boolean\nreturn true\nendfunction\n";
    let mut h = host_map(&common, map, 32, 0)
        .with_filters(super::FilterBudget { max_records: 1 })
        .unwrap()
        .with_trigger_runtime(input(0))
        .unwrap();
    let program = Arc::clone(&h.inner().program);
    let first = h.native("CreateTrigger", &[]).unwrap();
    let second = h.native("CreateTrigger", &[]).unwrap();
    let filter = h
        .native(
            "Filter",
            &[Value::Code(program.function_id("predicate").unwrap())],
        )
        .unwrap();
    let Value::Handle(f) = filter else {
        panic!("filter")
    };
    let player = h.inner.reserve_identity("player").unwrap();
    let unit = h.inner.reserve_identity("unit").unwrap();
    let region = h.inner.reserve_identity("region").unwrap();
    let event = h.inner.reserve_identity("playerunitevent").unwrap();
    let mut subscriptions = Vec::new();
    // The family constructors have separate contract tests; these fixtures isolate shared ownership.
    for owner in [&first, &second] {
        let Value::Handle(trigger) = owner else {
            panic!("trigger")
        };
        for entry in [
            super::Entry::RegisteredPlayerUnitEvent(super::PlayerUnitEventRegistration {
                trigger: *trigger,
                player,
                playerunitevent: event,
                enumerand: 0,
                filter: Some(f),
            }),
            super::Entry::RegisteredUnitInRange(super::UnitInRangeRegistration {
                trigger: *trigger,
                unit,
                range: 1.0,
                filter: Some(f),
            }),
            super::Entry::RegisteredRegionEvent(super::RegionEventRegistration {
                trigger: *trigger,
                region,
                filter: Some(f),
                kind: super::RegionEventKind::Enter,
            }),
        ] {
            subscriptions.push(
                h.inner
                    .allocate(entry, "synthetic event ownership")
                    .unwrap(),
            );
        }
    }
    let before = h.inner().handles();
    h.native("DestroyTrigger", &[first]).unwrap();
    assert!(
        h.native("DestroyBoolExpr", std::slice::from_ref(&filter))
            .is_err()
    );
    for (index, id) in subscriptions.iter().enumerate() {
        let retained = match h.inner.entry(*id).unwrap() {
            super::Entry::RegisteredPlayerUnitEvent(r) => r.filter,
            super::Entry::RegisteredUnitInRange(r) => r.filter,
            super::Entry::RegisteredRegionEvent(r) => r.filter,
            _ => panic!("subscription identity changed"),
        };
        assert_eq!(retained, if index < 3 { None } else { Some(f) });
    }
    h.native("DestroyTrigger", &[second]).unwrap();
    h.native("DestroyBoolExpr", &[filter]).unwrap();
    assert_eq!(h.inner().handles(), before);
}

const CONDITION_EDIT_COMMON: &str = "
type boolexpr extends agent
type conditionfunc extends boolexpr
type triggercondition extends agent
native Condition takes code f returns conditionfunc
native TriggerAddCondition takes trigger t, boolexpr c returns triggercondition
native TriggerRemoveCondition takes trigger t, triggercondition c returns nothing
native TriggerClearConditions takes trigger t returns nothing
native TriggerEvaluate takes trigger t returns boolean
";

#[test]
fn explicit_condition_traversal_matches_selected_runtime_mutation_probes() {
    let common = format!("{COMMON}{CONDITION_EDIT_COMMON}");
    let functions = r"
globals
trigger T = null
triggercondition pending = null
triggercondition current = null
integer trace = 0
boolean nested = false
boolean nestedResult = false
endglobals
function yes takes nothing returns boolean
set trace = trace * 10 + 7
return true
endfunction
function no takes nothing returns boolean
set trace = trace * 10 + 5
return false
endfunction
function clear takes nothing returns boolean
set trace = trace * 10 + 4
call TriggerClearConditions(T)
return true
endfunction
function removeNext takes nothing returns boolean
set trace = trace * 10 + 6
call TriggerRemoveCondition(T, pending)
return true
endfunction
function removeSelf takes nothing returns boolean
set trace = trace * 10 + 8
call TriggerRemoveCondition(T, current)
return true
endfunction
function append takes nothing returns boolean
set trace = trace * 10 + 9
call TriggerAddCondition(T, Condition(function yes))
return true
endfunction
function clearAppend takes nothing returns boolean
set trace = trace * 10 + 3
call TriggerClearConditions(T)
call TriggerAddCondition(T, Condition(function yes))
return true
endfunction
function reenter takes nothing returns boolean
set trace = trace * 10 + 1
if not nested then
set nested = true
set nestedResult = TriggerEvaluate(T)
set nested = false
endif
return true
endfunction
";
    for (setup, result, trace) in [
        ("", true, 0),
        (
            "call TriggerAddCondition(T, Condition(function yes))",
            true,
            7,
        ),
        (
            "call TriggerAddCondition(T, Condition(function no))\ncall TriggerAddCondition(T, Condition(function yes))",
            false,
            57,
        ),
        (
            "call TriggerAddCondition(T, Condition(function clear))\ncall TriggerAddCondition(T, Condition(function no))",
            false,
            4,
        ),
        (
            "call TriggerAddCondition(T, Condition(function removeNext))\nset pending = TriggerAddCondition(T, Condition(function no))",
            false,
            6,
        ),
        (
            "set current = TriggerAddCondition(T, Condition(function removeSelf))\ncall TriggerAddCondition(T, Condition(function yes))",
            true,
            87,
        ),
        (
            "set current = TriggerAddCondition(T, Condition(function removeSelf))",
            true,
            8,
        ),
        (
            "call TriggerAddCondition(T, Condition(function append))\ncall TriggerAddCondition(T, Condition(function no))",
            false,
            957,
        ),
        (
            "call TriggerAddCondition(T, Condition(function append))",
            true,
            9,
        ),
        (
            "call TriggerAddCondition(T, Condition(function clearAppend))\ncall TriggerAddCondition(T, Condition(function no))",
            false,
            3,
        ),
        (
            "call TriggerAddCondition(T, Condition(function removeNext))\nset pending = TriggerAddCondition(T, Condition(function no))\ncall TriggerAddCondition(T, Condition(function yes))",
            false,
            6,
        ),
        (
            "call TriggerAddCondition(T, Condition(function removeNext))\ncall TriggerAddCondition(T, Condition(function yes))\nset pending = TriggerAddCondition(T, Condition(function no))",
            true,
            67,
        ),
        (
            "call TriggerAddCondition(T, Condition(function reenter))\ncall TriggerAddCondition(T, Condition(function yes))",
            true,
            1177,
        ),
    ] {
        let map = format!(
            "{functions}\nfunction main takes nothing returns boolean\nset T = CreateTrigger()\n{setup}\nreturn TriggerEvaluate(T)\nendfunction\n"
        );
        let h = host_map(&common, &map, 128, 0)
            .with_conditions(super::ConditionBudget { max_records: 16 })
            .unwrap()
            .with_trigger_conditions(16)
            .unwrap()
            .with_trigger_runtime(input(0))
            .unwrap();
        let program = Arc::clone(&h.inner().program);
        let mut vm = pcx_script::OwnedVm::new_owned(program, h);
        vm.init_globals().unwrap();
        assert_eq!(
            vm.call("main", &[]).unwrap(),
            Value::Bool(result),
            "{setup}"
        );
        assert_eq!(vm.global("trace"), Value::Int(trace), "{setup}");
        if trace == 1177 {
            assert_eq!(vm.global("nestedResult"), Value::Bool(true));
        }
        assert_eq!(
            vm.host().native("GetTriggeringTrigger", &[]).unwrap(),
            Value::Null
        );
    }
}

#[test]
fn growing_condition_traversal_hits_registration_budget_and_unwinds_context() {
    let common = format!("{COMMON}{CONDITION_EDIT_COMMON}");
    let map = "globals
trigger T = null
boolexpr C = null
integer hits = 0
endglobals
function grow takes nothing returns boolean
set hits = hits + 1
call TriggerAddCondition(T, C)
return true
endfunction
function main takes nothing returns boolean
set T = CreateTrigger()
set C = Condition(function grow)
call TriggerAddCondition(T, C)
call TriggerAddCondition(T, C)
return TriggerEvaluate(T)
endfunction
function recover takes nothing returns boolean
call TriggerClearConditions(T)
return TriggerEvaluate(T)
endfunction
";
    let h = host_map(&common, map, 32, 0)
        .with_conditions(super::ConditionBudget { max_records: 1 })
        .unwrap()
        .with_trigger_conditions(4)
        .unwrap()
        .with_trigger_runtime(input(0))
        .unwrap();
    let program = Arc::clone(&h.inner().program);
    let mut vm = pcx_script::OwnedVm::new_owned(program, h);
    vm.init_globals().unwrap();
    let error = vm.call("main", &[]).unwrap_err();
    assert!(
        error.message.contains("trigger condition registration"),
        "{error:?}"
    );
    assert_eq!(vm.global("hits"), Value::Int(3));
    assert_eq!(vm.host().retained_trigger_conditions().unwrap(), 4);
    assert_eq!(
        vm.host().native("GetTriggeringTrigger", &[]).unwrap(),
        Value::Null
    );
    assert_eq!(vm.call("recover", &[]).unwrap(), Value::Bool(true));
    assert_eq!(vm.host().retained_trigger_conditions().unwrap(), 0);
}

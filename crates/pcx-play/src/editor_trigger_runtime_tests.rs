//! Runtime differential for the explicit editor-trigger path (in-crate: the trigger
//! host modules are crate-private).
//! The lowered graph and a
//! hand-written script with the same behavior run on the same timer and trigger host
//! (the object arena the trigger tests use), with the same clock and budget. Timer
//! delivery, condition refusal and initialized globals must produce the same observable
//! outcome: which callbacks were delivered, what each condition returned, and the final
//! globals.
//!
//! Evidence scope: a synthetic TriggerData and a synthetic Blizzard.j stub. The
//! compiled script is hand-written from the same behavior, not taken from a shipped map.
//! This covers the host contracts of the trigger runtime; it does not cover the native game.

use super::{ConditionBudget, EnumHost, Limits, ObjectHost, TriggerBudget, TriggerRuntimeInput};
use crate::editor_triggers::compile_editor_triggers;
use crate::editor_triggers::editor_fixture::{
    eca, empty_wct, nested, param, trigger, variable, wtg_with_variables,
};
use crate::scripts::{AssetIdentity, MapScripts, ScriptAsset};
use pcx_script::Value;
use pcx_trig::catalog::Catalog;

const CATALOG: &str = "\
[TriggerTypes]
integer=0,1,1,WESTRING_integer
[TriggerTypeDefaults]
integer=0
[TriggerParams]
RealHalf=0,real,0.5,WESTRING_HALF
Num1=0,integer,1,WESTRING_ONE
Num10=0,integer,10,WESTRING_TEN
Num100=0,integer,100,WESTRING_HUNDRED
Five=0,integer,5,WESTRING_FIVE
Six=0,integer,6,WESTRING_SIX
One=0,integer,1,WESTRING_ONEEE
OperatorAdd=0,ArithmeticOperator,\"+\",WESTRING_ADD
OperatorEqual=0,ComparisonOperator,\"==\",WESTRING_EQ
[TriggerEvents]
TriggerRegisterTimerEventSingle=0,real
[TriggerConditions]
OperatorCompareInteger=0,integer,ComparisonOperator,integer
[TriggerActions]
SetVariable=0,AnyGlobal,Null
[TriggerCalls]
OperatorInt=0,1,integer,integer,ArithmeticOperator,integer
";

/// The timer and trigger natives the trigger host admits, with the signatures and
/// sources of the shipped common.j. `TriggerRegisterTimerEventSingle` is a Blizzard.j
/// function; it is stubbed here as the one-shot form of the native.
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
native TriggerAddAction takes trigger t, code c returns triggeraction
native TriggerAddCondition takes trigger t, boolexpr c returns triggercondition
native Condition takes code func returns conditionfunc
type triggeraction extends handle
type triggercondition extends agent
type boolexpr extends agent
type conditionfunc extends boolexpr
type code extends handle
native Player takes integer n returns player
type player extends handle
";

const BLIZZARD: &str = "\
function TriggerRegisterTimerEventSingle takes trigger t, real timeout returns event
    return TriggerRegisterTimerEvent(t, timeout, false)
endfunction
";

/// Count = 5. "Start" adds 1. "Gated" is refused by a false condition (Count == 1), so its
/// +100 must never run. "Open" runs on Count == 6 (after Start) and adds 10. Final Count: 16.
fn graph() -> Vec<u8> {
    let start = trigger(
        "Start",
        true,
        &[
            eca(
                0,
                "TriggerRegisterTimerEventSingle",
                vec![param(0, "RealHalf", None)],
            ),
            eca(
                2,
                "SetVariable",
                vec![param(1, "Count", None), operator("Count", "Num1")],
            ),
        ],
    );
    let gated = trigger(
        "Gated",
        true,
        &[
            eca(
                0,
                "TriggerRegisterTimerEventSingle",
                vec![param(0, "RealHalf", None)],
            ),
            eca(
                1,
                "OperatorCompareInteger",
                vec![
                    param(1, "Count", None),
                    param(0, "OperatorEqual", None),
                    param(0, "One", None),
                ],
            ),
            eca(
                2,
                "SetVariable",
                vec![param(1, "Count", None), param(0, "Num100", None)],
            ),
        ],
    );
    let open = trigger(
        "Open",
        true,
        &[
            eca(
                0,
                "TriggerRegisterTimerEventSingle",
                vec![param(0, "RealHalf", None)],
            ),
            eca(
                1,
                "OperatorCompareInteger",
                vec![
                    param(1, "Count", None),
                    param(0, "OperatorEqual", None),
                    param(0, "Six", None),
                ],
            ),
            eca(
                2,
                "SetVariable",
                vec![param(1, "Count", None), operator("Count", "Num10")],
            ),
        ],
    );
    wtg_with_variables(
        &[variable("Count", "integer", Some("5"))],
        &[start, gated, open],
    )
}

fn operator(variable: &str, amount: &str) -> Vec<u8> {
    nested(
        "OperatorInt",
        vec![
            param(1, variable, None),
            param(0, "OperatorAdd", None),
            param(0, amount, None),
        ],
    )
}

/// The compiled-script equivalent of `graph()`, written the way the editor writes it.
const HAND_WRITTEN: &str = "globals
integer udg_Count
trigger gg_trg_Start
trigger gg_trg_Gated
trigger gg_trg_Open
endglobals

function InitGlobals takes nothing returns nothing
    set udg_Count = 5
endfunction

function Trig_Start_Actions takes nothing returns nothing
    set udg_Count = (udg_Count + 1)
endfunction

function Trig_Gated_Conditions takes nothing returns boolean
    if (not (udg_Count == 1)) then
        return false
    endif
    return true
endfunction

function Trig_Gated_Actions takes nothing returns nothing
    set udg_Count = 100
endfunction

function Trig_Open_Conditions takes nothing returns boolean
    if (not (udg_Count == 6)) then
        return false
    endif
    return true
endfunction

function Trig_Open_Actions takes nothing returns nothing
    set udg_Count = (udg_Count + 10)
endfunction

function InitTrig_Start takes nothing returns nothing
    set gg_trg_Start = CreateTrigger()
    call TriggerRegisterTimerEventSingle(gg_trg_Start, 0.5)
    call TriggerAddAction(gg_trg_Start, function Trig_Start_Actions)
endfunction

function InitTrig_Gated takes nothing returns nothing
    set gg_trg_Gated = CreateTrigger()
    call TriggerRegisterTimerEventSingle(gg_trg_Gated, 0.5)
    call TriggerAddCondition(gg_trg_Gated, Condition(function Trig_Gated_Conditions))
    call TriggerAddAction(gg_trg_Gated, function Trig_Gated_Actions)
endfunction

function InitTrig_Open takes nothing returns nothing
    set gg_trg_Open = CreateTrigger()
    call TriggerRegisterTimerEventSingle(gg_trg_Open, 0.5)
    call TriggerAddCondition(gg_trg_Open, Condition(function Trig_Open_Conditions))
    call TriggerAddAction(gg_trg_Open, function Trig_Open_Actions)
endfunction

function InitCustomTriggers takes nothing returns nothing
    call InitTrig_Start()
    call InitTrig_Gated()
    call InitTrig_Open()
endfunction

function RunInitializationTriggers takes nothing returns nothing
endfunction
";

fn identity() -> AssetIdentity {
    AssetIdentity {
        edition: "synthetic".into(),
        version: "fixture".into(),
        source: "memory".into(),
    }
}

fn compile_scripts(map_source: Option<&str>, lowered: bool) -> MapScripts {
    let catalog =
        Catalog::parse("synthetic-TriggerData.txt", "synthetic", "fixture", CATALOG).unwrap();
    let common = ScriptAsset {
        label: "common.j",
        bytes: COMMON.as_bytes(),
    };
    let blizzard = ScriptAsset {
        label: "Blizzard.j",
        bytes: BLIZZARD.as_bytes(),
    };
    if lowered {
        compile_editor_triggers(
            identity(),
            common,
            blizzard,
            &graph(),
            &empty_wct(3),
            &catalog,
        )
        .unwrap()
    } else {
        MapScripts::compile(
            identity(),
            common,
            blizzard,
            ScriptAsset {
                label: "war3map.j",
                bytes: map_source.unwrap().as_bytes(),
            },
            None,
        )
        .unwrap()
    }
}

/// The trigger runtime admission the existing trigger fixtures use.
fn host(scripts: &MapScripts) -> ObjectHost {
    let program = scripts.shared_program();
    ObjectHost::checked(
        EnumHost::checked_selected(
            program,
            "synthetic timed event",
            &["ConvertUnitEvent", "ConvertGameEvent"],
            Limits {
                max_handles: 64,
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
        max_actions: 8,
    })
    .unwrap()
    .with_timer_events(8)
    .unwrap()
    .with_timer_runtime(20.0)
    .unwrap()
    .with_conditions(ConditionBudget { max_records: 8 })
    .unwrap()
    .with_trigger_conditions(8)
    .unwrap()
    .with_trigger_runtime(TriggerRuntimeInput {
        initial_enabled: true,
        timer_event_global: "TEST_TIMER".into(),
        max_timed_events: 8,
        max_parked_threads: 4,
    })
    .unwrap()
}

/// Startup as `main` runs it, then one timer expiry delivered by the host, with each
/// delivered condition and action run by the session in the same VM.
fn drive(scripts: &MapScripts) -> (Vec<String>, Value) {
    let names: Vec<String> = scripts
        .program()
        .functions()
        .iter()
        .map(|f| f.name.clone())
        .collect();
    let mut vm = scripts.vm(host(scripts));
    vm.init_globals().unwrap();
    vm.call("InitGlobals", &[]).unwrap();
    vm.call("InitCustomTriggers", &[]).unwrap();
    vm.call("RunInitializationTriggers", &[]).unwrap();
    let mut log = Vec::new();
    // The host prepares one firing per call, in deadline order, until none is due.
    while let Some(expiry) = vm.host().inner.next_timer_expiry(21.0, &mut 8).unwrap() {
        for delivery in expiry.triggers() {
            let mut passed = true;
            for condition in &delivery.conditions {
                let name = &names[condition.index()];
                let result = vm.call(name, &[]).unwrap();
                log.push(format!("condition {name} => {result:?}"));
                passed &= result == Value::Bool(true);
            }
            for action in &delivery.actions {
                let name = &names[action.index()];
                if passed {
                    vm.call(name, &[]).unwrap();
                    log.push(format!("ran {name}"));
                } else {
                    log.push(format!("refused {name}"));
                }
            }
        }
        vm.host().inner.finish_timer_expiry(expiry).unwrap();
    }
    let count = vm.global("udg_Count");
    (log, count)
}

#[test]
fn timer_callbacks_condition_refusal_and_initialized_globals_match_the_hand_written_script() {
    let lowered = compile_scripts(None, true);
    let hand = compile_scripts(Some(HAND_WRITTEN), false);
    let (lowered_log, lowered_count) = drive(&lowered);
    let (hand_log, hand_count) = drive(&hand);
    assert_eq!(lowered_log, hand_log);
    assert_eq!(lowered_count, hand_count);
    assert_eq!(lowered_count, Value::Int(16), "{lowered_log:?}");
    assert!(
        lowered_log
            .iter()
            .any(|l| l == "refused Trig_Gated_Actions"),
        "{lowered_log:?}"
    );
    assert!(
        lowered_log.iter().any(|l| l == "ran Trig_Start_Actions"),
        "{lowered_log:?}"
    );
    assert!(
        lowered_log.iter().any(|l| l == "ran Trig_Open_Actions"),
        "{lowered_log:?}"
    );
}

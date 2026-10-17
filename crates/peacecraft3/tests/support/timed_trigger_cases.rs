//! Timed triggers through the actual factory and published session.
use super::script_timer_cases::{deliver_timed_trigger, timer_app};
use super::*;
use pcx_script::Host;

pub(super) const COMMON: &str = "\
type gameevent extends eventid
type triggeraction extends handle
type triggercondition extends agent
type conditionfunc extends boolexpr
constant native ConvertGameEvent takes integer i returns gameevent
constant native GetTriggeringTrigger takes nothing returns trigger
constant native GetTriggerEventId takes nothing returns eventid
native DestroyTrigger takes trigger t returns nothing
native EnableTrigger takes trigger t returns nothing
native DisableTrigger takes trigger t returns nothing
native IsTriggerEnabled takes trigger t returns boolean
native TriggerRegisterTimerEvent takes trigger t, real timeout, boolean periodic returns event
native TriggerAddAction takes trigger t, code callback returns triggeraction
native Condition takes code callback returns conditionfunc
native TriggerAddCondition takes trigger t, boolexpr callback returns triggercondition
globals
constant gameevent TEST_TIMER_EVENT = ConvertGameEvent(29)
endglobals
";
const SCRIPT: &str = "\
globals
trigger t = CreateTrigger()
timer direct = CreateTimer()
integer seen = 0
boolean context_ok = true
boolean cleared = false
endglobals
function check_context takes nothing returns nothing
set context_ok = context_ok and GetTriggeringTrigger() == t and GetTriggerEventId() == TEST_TIMER_EVENT and GetExpiredTimer() == null
endfunction
function first takes nothing returns boolean
call check_context()
set seen = seen * 10 + 1
return false
endfunction
function second takes nothing returns boolean
call check_context()
set seen = seen * 10 + 2
return true
endfunction
function action takes nothing returns nothing
call check_context()
set seen = seen * 10 + 3
call DestroyTrigger(t)
endfunction
function tail takes nothing returns nothing
call check_context()
set seen = seen * 10 + 4
endfunction
function direct_handler takes nothing returns nothing
set cleared = GetTriggeringTrigger() == null and GetTriggerEventId() == null and GetExpiredTimer() == direct
endfunction
function config takes nothing returns nothing
endfunction
function main takes nothing returns nothing
call TriggerAddCondition(t, Condition(function first))
call TriggerAddCondition(t, Condition(function second))
call TriggerAddAction(t, function action)
call TriggerAddAction(t, function tail)
call TriggerRegisterTimerEvent(t, 0.5, true)
call TimerStart(direct, 0.75, false, function direct_handler)
endfunction
";

#[test]
fn timed_trigger_evaluates_every_condition_and_preserves_action_tail_context() {
    for (source, expected) in [
        (SCRIPT.to_string(), 1212),
        (SCRIPT.replace("return false", "return true"), 1234),
    ] {
        let mut app = timer_app();
        deliver_timed_trigger(&mut app, &source, 1);
        assert!(
            app.world()
                .resource::<CampaignStartup>()
                .last_error
                .is_none(),
            "{:?}",
            app.world().resource::<CampaignStartup>().last_error
        );
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let handles = session.host().unwrap().objects().inner().handles();
        assert!(session.advance_timers(11.0, 0).is_err());
        assert_eq!(session.global("seen"), Some(Value::Int(0)));
        session.advance_timers(11.0, 8).unwrap();
        assert_eq!(session.global("seen"), Some(Value::Int(expected)));
        assert_eq!(session.global("context_ok"), Some(Value::Bool(true)));
        assert_eq!(session.global("cleared"), Some(Value::Bool(true)));
        assert_eq!(session.host().unwrap().objects().inner().handles(), handles);
        assert_eq!(
            session
                .host()
                .unwrap()
                .objects_mut()
                .native("GetTriggeringTrigger", &[])
                .unwrap(),
            Value::Null
        );
    }
}

#[test]
fn timed_trigger_disabled_phase_and_destroy_before_firing_are_retained() {
    let source = "\
globals
trigger t = CreateTrigger()
trigger dead = CreateTrigger()
timer enable = CreateTimer()
integer hits = 0
real at = 0.0
endglobals
function fire takes nothing returns nothing
set hits = hits + 1
call DestroyTrigger(GetTriggeringTrigger())
endfunction
function allow takes nothing returns nothing
call EnableTrigger(t)
endfunction
function config takes nothing returns nothing
endfunction
function main takes nothing returns nothing
call TriggerAddAction(t,function fire)
call TriggerAddAction(dead,function fire)
call TriggerRegisterTimerEvent(t,0.25,true)
call TriggerRegisterTimerEvent(dead,0.25,true)
call DisableTrigger(t)
call DestroyTrigger(dead)
call TimerStart(enable,0.625,false,function allow)
endfunction
";
    let mut app = timer_app();
    deliver_timed_trigger(&mut app, source, 1);
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_none()
    );
    let mut s = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    assert_eq!(s.advance_timers(10.625, 3).unwrap(), 3);
    assert_eq!(s.global("hits"), Some(Value::Int(0)));
    assert_eq!(s.advance_timers(10.75, 1).unwrap(), 1);
    assert_eq!(s.global("hits"), Some(Value::Int(1)));
    assert_eq!(s.advance_timers(100.0, 0).unwrap(), 0);
}

#[test]
fn timer_subscription_runs_actions_then_restores_context_for_direct_handler() {
    let source = SCRIPT
        .replace("call TriggerAddCondition(t, Condition(function first))", "")
        .replace(
            "call TriggerAddCondition(t, Condition(function second))",
            "",
        )
        .replace("GetExpiredTimer() == null", "GetExpiredTimer() == direct")
        .replace(
            "call TriggerRegisterTimerEvent(t, 0.5, true)",
            "call TriggerRegisterTimerExpireEvent(t, direct)",
        );
    let mut app = timer_app();
    deliver_timed_trigger(&mut app, &source, 1);
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_none()
    );
    let mut s = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    assert_eq!(s.advance_timers(11.0, 1).unwrap(), 1);
    assert_eq!(s.global("seen"), Some(Value::Int(34)));
    assert_eq!(s.global("context_ok"), Some(Value::Bool(true)));
    assert_eq!(s.global("cleared"), Some(Value::Bool(true)));
}

#[test]
fn timed_callback_failure_cleans_context_and_candidate_replacement_keeps_previous_state() {
    let good = SCRIPT.replace("return false", "return true");
    let mut app = timer_app();
    deliver_timed_trigger(&mut app, &good, 1);
    let witness = app
        .world()
        .resource::<MapScriptSession<MapStartupHost>>()
        .session_witness()
        .unwrap();
    deliver_timed_trigger(&mut app, &good.replace("t, 0.5", "t, 0.0"), 2);
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_some()
    );
    assert!(witness.upgrade().is_some());
    assert_eq!(
        app.world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .advance_timers(10.5, 1)
            .unwrap(),
        1
    );
    deliver_timed_trigger(
        &mut app,
        &good.replace(
            "set seen = seen * 10 + 1",
            "set seen = seen * 10 + 1\ncall TimerStart(null,1.0,false,null)",
        ),
        3,
    );
    assert!(witness.upgrade().is_none());
    let mut s = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    assert!(s.advance_timers(10.5, 1).is_err());
    assert_eq!(s.global("seen"), Some(Value::Int(1)));
    assert_eq!(
        s.host()
            .unwrap()
            .objects_mut()
            .native("GetTriggeringTrigger", &[])
            .unwrap(),
        Value::Null
    );
    assert_eq!(
        s.host()
            .unwrap()
            .objects_mut()
            .native("GetTriggerEventId", &[])
            .unwrap(),
        Value::Null
    );
    assert!(
        s.host()
            .unwrap()
            .objects_mut()
            .native("GetExpiredTimer", &[])
            .is_err()
    );
    s.advance_timers(10.75, 1).unwrap();
    assert_eq!(s.global("cleared"), Some(Value::Bool(true)));
    s.clear();
    assert!(s.host().is_none());
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn browser_timed_trigger_gate() -> u32 {
    timed_trigger_evaluates_every_condition_and_preserves_action_tail_context();
    timed_trigger_disabled_phase_and_destroy_before_firing_are_retained();
    timer_subscription_runs_actions_then_restores_context_for_direct_handler();
    timed_callback_failure_cleans_context_and_candidate_replacement_keeps_previous_state();
    4
}

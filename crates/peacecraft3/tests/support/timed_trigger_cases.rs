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
native TriggerEvaluate takes trigger t returns boolean
native TriggerExecute takes trigger t returns nothing
globals
constant gameevent TEST_TIMER_EVENT = ConvertGameEvent(29)
endglobals
";

#[test]
fn direct_initialization_triggers_execute_in_the_owning_vm_and_restore_nested_context() {
    let source = "\
globals
trigger a = CreateTrigger()
trigger b = CreateTrigger()
integer seen = 0
boolean contexts = true
endglobals
function first takes nothing returns boolean
set seen = seen * 10 + 1
set contexts = contexts and GetTriggeringTrigger() == a and GetTriggerEventId() == null
return false
endfunction
function second takes nothing returns boolean
set seen = seen * 10 + 2
return true
endfunction
function inner takes nothing returns nothing
set seen = seen * 10 + 3
set contexts = contexts and GetTriggeringTrigger() == b and GetTriggerEventId() == null
endfunction
function outer takes nothing returns nothing
set seen = seen * 10 + 4
call TriggerExecute(b)
set seen = seen * 10 + 5
set contexts = contexts and GetTriggeringTrigger() == a
endfunction
function config takes nothing returns nothing
endfunction
function main takes nothing returns nothing
call TriggerAddCondition(a, Condition(function first))
call TriggerAddCondition(a, Condition(function second))
call TriggerAddAction(a, function outer)
call TriggerAddAction(b, function inner)
call DisableTrigger(a)
if TriggerEvaluate(a) then
set seen = -1
endif
call TriggerExecute(a)
set contexts = contexts and GetTriggeringTrigger() == null and GetTriggerEventId() == null
endfunction
";
    let mut app = timer_app();
    deliver_timed_trigger(&mut app, source, 1);
    let session = app.world().resource::<MapScriptSession<MapStartupHost>>();
    assert_eq!(session.global("seen"), Some(Value::Int(12435)));
    assert_eq!(session.global("contexts"), Some(Value::Bool(true)));
}

#[test]
fn direct_trigger_callback_failure_unwinds_context_without_repeating_prior_effects() {
    let source = "\
globals
trigger t = CreateTrigger()
integer seen = 0
endglobals
function fails takes nothing returns nothing
set seen = seen + 1
call SetSoundPitch(null, 1.0)
endfunction
function run takes nothing returns nothing
call TriggerExecute(t)
endfunction
function config takes nothing returns nothing
endfunction
function main takes nothing returns nothing
call TriggerAddAction(t, function fails)
endfunction
";
    let mut app = timer_app();
    deliver_timed_trigger(&mut app, source, 1);
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let id = session
        .context()
        .unwrap()
        .scripts
        .program()
        .function_id("run")
        .unwrap();
    let ticket = session.callback_ticket(id).unwrap();
    assert!(session.call_callback(&ticket, &[]).is_err());
    assert_eq!(session.global("seen"), Some(Value::Int(1)));
    let host = session.host().unwrap();
    assert_eq!(
        host.native("GetTriggeringTrigger", &[]).unwrap(),
        Value::Null
    );
    assert_eq!(host.native("GetTriggerEventId", &[]).unwrap(), Value::Null);
}
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

#[test]
fn timer_delivery_cursor_keeps_condition_snapshot_and_skips_retired_subscription() {
    let source = "\
globals
trigger first = CreateTrigger()
trigger retired = CreateTrigger()
timer clock = CreateTimer()
integer seen = 0
boolean context_ok = true
endglobals
function added takes nothing returns boolean
set seen = seen * 10 + 9
return true
endfunction
function reject takes nothing returns boolean
set context_ok = context_ok and GetTriggeringTrigger() == first and GetExpiredTimer() == clock
set seen = seen * 10 + 1
call TriggerAddCondition(first,Condition(function added))
call DestroyTrigger(retired)
return false
endfunction
function accept takes nothing returns boolean
set context_ok = context_ok and GetTriggeringTrigger() == first and GetExpiredTimer() == clock
set seen = seen * 10 + 2
return true
endfunction
function unwanted takes nothing returns nothing
set seen = -1
endfunction
function handler takes nothing returns nothing
set context_ok = context_ok and GetTriggeringTrigger() == null and GetTriggerEventId() == null and GetExpiredTimer() == clock
set seen = seen * 10 + 3
call DestroyTimer(clock)
endfunction
function config takes nothing returns nothing
endfunction
function main takes nothing returns nothing
call TriggerAddCondition(first,Condition(function reject))
call TriggerAddCondition(first,Condition(function accept))
call TriggerAddAction(first,function unwanted)
call TriggerAddAction(retired,function unwanted)
call TriggerRegisterTimerExpireEvent(first,clock)
call TriggerRegisterTimerExpireEvent(retired,clock)
call TimerStart(clock,0.5,false,function handler)
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
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    assert_eq!(session.advance_timers(10.5, 1).unwrap(), 1);
    assert_eq!(session.global("seen"), Some(Value::Int(123)));
    assert_eq!(session.global("context_ok"), Some(Value::Bool(true)));
    assert_eq!(session.advance_timers(20.0, 0).unwrap(), 0);
}

#[test]
fn timer_delivery_cursor_failure_retires_expiry_before_next_callback() {
    let source = "\
globals
trigger subscribed = CreateTrigger()
timer first = CreateTimer()
timer next = CreateTimer()
integer seen = 0
boolean context_ok = false
endglobals
function fail takes nothing returns boolean
set seen = 7
call TimerStart(null,1.0,false,null)
return true
endfunction
function unwanted takes nothing returns nothing
set seen = -1
endfunction
function recovered takes nothing returns nothing
set context_ok = GetTriggeringTrigger() == null and GetTriggerEventId() == null and GetExpiredTimer() == next
set seen = seen * 10 + 9
endfunction
function config takes nothing returns nothing
endfunction
function main takes nothing returns nothing
call TriggerAddCondition(subscribed,Condition(function fail))
call TriggerAddAction(subscribed,function unwanted)
call TriggerRegisterTimerExpireEvent(subscribed,first)
call TimerStart(first,0.5,false,function unwanted)
call TimerStart(next,0.75,false,function recovered)
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
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    assert!(session.advance_timers(10.5, 1).is_err());
    assert_eq!(session.global("seen"), Some(Value::Int(7)));
    let objects = session.host().unwrap().objects_mut();
    assert_eq!(
        objects.native("GetTriggeringTrigger", &[]).unwrap(),
        Value::Null
    );
    assert_eq!(
        objects.native("GetTriggerEventId", &[]).unwrap(),
        Value::Null
    );
    assert!(objects.native("GetExpiredTimer", &[]).is_err());
    assert_eq!(session.advance_timers(10.75, 1).unwrap(), 1);
    assert_eq!(session.global("seen"), Some(Value::Int(79)));
    assert_eq!(session.global("context_ok"), Some(Value::Bool(true)));
    assert_eq!(session.advance_timers(20.0, 0).unwrap(), 0);
}

fn assert_subscription_handler(first: &str, second: &str, destroyed: bool, expected: i32) {
    let source = format!(
        "\
globals
trigger first = CreateTrigger()
trigger second = CreateTrigger()
timer clock = CreateTimer()
timer next = CreateTimer()
integer seen = 0
boolean context_ok = true
endglobals
function check_handler takes nothing returns nothing
set context_ok = context_ok and GetTriggeringTrigger() == null and GetTriggerEventId() == null and GetExpiredTimer() == {identity}
call DestroyTimer(clock)
endfunction
function a takes nothing returns nothing
call check_handler()
set seen = seen * 10 + 3
endfunction
function b takes nothing returns nothing
call check_handler()
set seen = seen * 10 + 4
endfunction
function c takes nothing returns nothing
call check_handler()
set seen = seen * 10 + 5
endfunction
function first_action takes nothing returns nothing
set context_ok = context_ok and GetTriggeringTrigger() == first and GetExpiredTimer() == clock
set seen = seen * 10 + 1
{first}
endfunction
function second_action takes nothing returns nothing
set context_ok = context_ok and GetTriggeringTrigger() == second and GetExpiredTimer() == {identity}
set seen = seen * 10 + 2
{second}
endfunction
function following takes nothing returns nothing
set context_ok = context_ok and GetTriggeringTrigger() == null and GetExpiredTimer() == next
set seen = seen * 10 + 6
endfunction
function config takes nothing returns nothing
endfunction
function main takes nothing returns nothing
call TriggerAddAction(first,function first_action)
call TriggerAddAction(second,function second_action)
call TriggerRegisterTimerExpireEvent(first,clock)
call TriggerRegisterTimerExpireEvent(second,clock)
call TimerStart(clock,0.5,false,function a)
call TimerStart(next,0.75,false,function following)
endfunction
",
        identity = if destroyed { "null" } else { "clock" }
    );
    let mut app = timer_app();
    deliver_timed_trigger(&mut app, &source, 1);
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_none()
    );
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    assert_eq!(session.advance_timers(10.5, 1).unwrap(), 1);
    assert_eq!(session.global("seen"), Some(Value::Int(expected)));
    assert_eq!(session.global("context_ok"), Some(Value::Bool(true)));
    assert_eq!(session.advance_timers(10.75, 1).unwrap(), 1);
    assert_eq!(session.global("seen"), Some(Value::Int(expected * 10 + 6)));
    assert_eq!(session.global("context_ok"), Some(Value::Bool(true)));
}

#[test]
fn timer_subscription_replacement_selects_current_direct_handler() {
    assert_subscription_handler(
        "call TimerStart(clock,2.0,false,function b)",
        "",
        false,
        124,
    );
}

#[test]
fn timer_subscription_null_replacement_omits_direct_handler() {
    assert_subscription_handler("call TimerStart(clock,2.0,false,null)", "", false, 12);
}

#[test]
fn timer_subscriptions_select_final_replacement_after_all_actions() {
    assert_subscription_handler(
        "call TimerStart(clock,2.0,false,function b)",
        "call TimerStart(clock,3.0,false,function c)",
        false,
        125,
    );
}

#[test]
fn timer_subscription_destroy_retains_handler_with_null_expired_identity() {
    assert_subscription_handler("call DestroyTimer(clock)", "", true, 123);
}

#[test]
fn timer_subscription_replace_then_destroy_retains_latest_handler() {
    assert_subscription_handler(
        "call TimerStart(clock,2.0,false,function b)\ncall DestroyTimer(clock)",
        "",
        true,
        124,
    );
}

#[test]
fn timer_subscription_null_then_destroy_omits_direct_handler() {
    assert_subscription_handler(
        "call TimerStart(clock,2.0,false,null)\ncall DestroyTimer(clock)",
        "",
        true,
        12,
    );
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn browser_timed_trigger_gate() -> u32 {
    timed_trigger_evaluates_every_condition_and_preserves_action_tail_context();
    timed_trigger_disabled_phase_and_destroy_before_firing_are_retained();
    timer_subscription_runs_actions_then_restores_context_for_direct_handler();
    timed_callback_failure_cleans_context_and_candidate_replacement_keeps_previous_state();
    timer_delivery_cursor_keeps_condition_snapshot_and_skips_retired_subscription();
    timer_delivery_cursor_failure_retires_expiry_before_next_callback();
    timer_subscription_replacement_selects_current_direct_handler();
    timer_subscription_null_replacement_omits_direct_handler();
    timer_subscriptions_select_final_replacement_after_all_actions();
    timer_subscription_destroy_retains_handler_with_null_expired_identity();
    timer_subscription_replace_then_destroy_retains_latest_handler();
    timer_subscription_null_then_destroy_omits_direct_handler();
    12
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn browser_direct_trigger_gate() -> u32 {
    direct_initialization_triggers_execute_in_the_owning_vm_and_restore_nested_context();
    direct_trigger_callback_failure_unwinds_context_without_repeating_prior_effects();
    2
}

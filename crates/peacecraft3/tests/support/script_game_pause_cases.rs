use super::*;
use peacecraft3::campaign_runtime::CampaignRuntime;
use std::time::Duration;

const PAUSING_TIMER: &str = "\
globals
timer t = CreateTimer()
integer marker = 0
boolean afterPause = false
endglobals
function fire takes nothing returns nothing
set marker = marker + 1
if marker == 1 then
call PauseGame(true)
set afterPause = true
endif
endfunction
function resume takes nothing returns nothing
call PauseGame(false)
endfunction
function config takes nothing returns nothing
endfunction
function main takes nothing returns nothing
call TimerStart(t, 0.25, true, function fire)
endfunction
";

fn resume(session: &mut MapScriptSession<MapStartupHost>) {
    let function = session
        .context()
        .unwrap()
        .scripts
        .program()
        .function_id("resume")
        .unwrap();
    session.call_function(function, &[]).unwrap();
}

#[test]
pub(super) fn executed_pause_finishes_thread_and_holds_future_timer_delivery() {
    let mut app = timer_app();
    deliver_timer(&mut app, PAUSING_TIMER, 1);
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    assert_eq!(session.advance_timers(30.0, 8).unwrap(), 1);
    assert_eq!(session.global("afterPause"), Some(Value::Bool(true)));
    assert_eq!(
        session.host().unwrap().objects().timer_game_time(),
        Some(10.25)
    );
    assert_eq!(session.advance_timers(30.0, 0).unwrap(), 0);
    assert_eq!(session.global("marker"), Some(Value::Int(1)));
    for until in [f64::NAN, f64::INFINITY, 10.0] {
        assert!(session.advance_timers(until, 8).is_err());
    }
    assert_eq!(
        session.host().unwrap().objects().timer_game_time(),
        Some(10.25)
    );
    resume(&mut session);
    assert_eq!(session.advance_timers(10.5, 8).unwrap(), 1);
    assert_eq!(session.global("marker"), Some(Value::Int(2)));
}

#[test]
pub(super) fn runtime_rebases_a_mid_tick_pause_and_discards_paused_wall_time() {
    let mut app = timer_app();
    deliver_timer(&mut app, PAUSING_TIMER, 1);
    let mut runtime = CampaignRuntime::new(Duration::from_secs(1), 8, 8);
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    runtime.advance(&mut session, Duration::ZERO);
    runtime.advance(&mut session, Duration::from_secs(10));
    assert!(runtime.last_error.is_none(), "{:?}", runtime.last_error);
    assert_eq!(runtime.tick(), 1);
    assert_eq!(runtime.pending(), Duration::ZERO);
    runtime.advance(&mut session, Duration::from_secs(100));
    assert_eq!(
        session.host().unwrap().objects().timer_game_time(),
        Some(10.25)
    );
    resume(&mut session);
    runtime.advance(&mut session, Duration::from_millis(500));
    assert_eq!(runtime.tick(), 1);
    runtime.advance(&mut session, Duration::from_millis(500));
    assert!(runtime.last_error.is_none(), "{:?}", runtime.last_error);
    assert_eq!(runtime.tick(), 2);
    assert_eq!(
        session.host().unwrap().objects().timer_game_time(),
        Some(11.25)
    );
    assert_eq!(session.global("marker"), Some(Value::Int(5)));
}

#[test]
pub(super) fn queued_dialog_resume_runs_while_the_simulation_clock_is_paused() {
    let mut app = timer_app();
    let common = format!(
        "{}\n{}",
        super::super::timed_trigger_cases::COMMON,
        "\
constant native GetLocalPlayer takes nothing returns player
type dialog extends agent
type button extends agent
type dialogevent extends eventid
constant native ConvertDialogEvent takes integer n returns dialogevent
native DialogCreate takes nothing returns dialog
native DialogAddButton takes dialog d, string text, integer key returns button
native DialogDisplay takes player p, dialog d, boolean visible returns nothing
native TriggerRegisterDialogEvent takes trigger t, dialog d returns event
globals
constant dialogevent EVENT_DIALOG_CLICK = ConvertDialogEvent(73)
endglobals
"
    );
    let script = "\
globals
player p = Player(0)
dialog d = DialogCreate()
button b = DialogAddButton(d, \"Resume fixture\", 82)
trigger listener = CreateTrigger()
integer marker = 0
endglobals
function clicked takes nothing returns nothing
call PauseGame(false)
set marker = marker + 1
endfunction
function config takes nothing returns nothing
endfunction
function main takes nothing returns nothing
call TriggerRegisterDialogEvent(listener, d)
call TriggerAddAction(listener, function clicked)
call PauseGame(true)
call DialogDisplay(p, d, true)
endfunction
";
    deliver_timer_with(&mut app, script, 1, &common, |inputs| {
        inputs.post_config.local_player = Some(0);
        inputs
            .enumeration
            .names
            .extend(["ConvertGameEvent".into(), "ConvertDialogEvent".into()]);
        inputs.enumeration.limits.max_handles = 256;
        inputs.trigger_runtime = Some(pcx_play::playercolor_host::TriggerRuntimeInput {
            initial_enabled: true,
            timer_event_global: "TEST_TIMER_EVENT".into(),
            max_timed_events: 8,
            max_parked_threads: 8,
        });
        inputs.budgets.dialogs = Some(pcx_play::playercolor_host::DialogBudget {
            max_dialogs: 2,
            max_buttons: 4,
            text_bytes: 256,
            max_events: 4,
            max_quit_requests: 2,
        });
    });
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_none(),
        "{:?}",
        app.world().resource::<CampaignStartup>().last_error
    );
    let mut runtime = CampaignRuntime::new(Duration::from_millis(125), 8, 8);
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    runtime.advance(&mut session, Duration::ZERO);
    runtime.advance(&mut session, Duration::from_secs(100));
    let Value::Handle(player) = session.global("p").unwrap() else {
        panic!("fixture player")
    };
    let Value::Handle(button) = session.global("b").unwrap() else {
        panic!("fixture button")
    };
    assert_eq!(
        session
            .host()
            .unwrap()
            .objects_mut()
            .click_dialog_button(player, button)
            .unwrap(),
        1
    );
    runtime.advance(&mut session, Duration::from_secs(100));
    assert!(runtime.last_error.is_none(), "{:?}", runtime.last_error);
    assert_eq!(session.global("marker"), Some(Value::Int(1)));
    assert_eq!(session.host().unwrap().objects().game_paused(), Some(false));
    assert_eq!(
        session.host().unwrap().objects().timer_game_time(),
        Some(10.0)
    );
    assert_eq!(runtime.pending(), Duration::ZERO);
    runtime.advance(&mut session, Duration::from_millis(125));
    assert_eq!(
        session.host().unwrap().objects().timer_game_time(),
        Some(10.125)
    );
}

#[test]
pub(super) fn paused_session_preserves_parked_threads_until_game_time_resumes() {
    let mut app = timer_app();
    let common = format!(
        "{}\nnative TriggerSleepAction takes real seconds returns nothing\n",
        super::super::timed_trigger_cases::COMMON
    );
    let script = "\
globals
trigger sleeper = CreateTrigger()
timer pauser = CreateTimer()
integer marker = 0
endglobals
function sleepAction takes nothing returns nothing
set marker = 1
call TriggerSleepAction(0.25)
set marker = 2
endfunction
function pauseAction takes nothing returns nothing
call PauseGame(true)
endfunction
function resume takes nothing returns nothing
call PauseGame(false)
endfunction
function config takes nothing returns nothing
endfunction
function main takes nothing returns nothing
call TriggerAddAction(sleeper, function sleepAction)
call TriggerRegisterTimerEvent(sleeper, 0.125, false)
call TimerStart(pauser, 0.25, false, function pauseAction)
endfunction
";
    deliver_timer_with(&mut app, script, 1, &common, |inputs| {
        inputs.enumeration.names.push("ConvertGameEvent".into());
        inputs.enumeration.limits.max_handles = 256;
        inputs.trigger_runtime = Some(pcx_play::playercolor_host::TriggerRuntimeInput {
            initial_enabled: true,
            timer_event_global: "TEST_TIMER_EVENT".into(),
            max_timed_events: 8,
            max_parked_threads: 8,
        });
    });
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
    assert_eq!(session.advance_timers(30.0, 8).unwrap(), 2);
    assert_eq!(session.global("marker"), Some(Value::Int(1)));
    assert_eq!(session.host().unwrap().objects().parked_thread_count(), 1);
    assert_eq!(session.advance_timers(30.0, 0).unwrap(), 0);
    assert_eq!(
        session.host().unwrap().objects().timer_game_time(),
        Some(10.25)
    );
    resume(&mut session);
    assert_eq!(session.advance_timers(10.375, 8).unwrap(), 1);
    assert_eq!(session.global("marker"), Some(Value::Int(2)));
    assert_eq!(session.host().unwrap().objects().parked_thread_count(), 0);
}

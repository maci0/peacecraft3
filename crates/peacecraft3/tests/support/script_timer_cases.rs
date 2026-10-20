//! Timer delivery through the production factory and its one published VM.
use super::*;
use peacecraft3::map_script_session::TimerSessionError;
use std::collections::BTreeMap;

pub(super) const TIMER_COMMON: &str = "\
native PauseGame takes boolean paused returns nothing
native TimerStart takes timer t, real timeout, boolean periodic, code handler returns nothing
native TimerGetElapsed takes timer t returns real
native TimerGetRemaining takes timer t returns real
native TimerGetTimeout takes timer t returns real
native GetExpiredTimer takes nothing returns timer
";
const RESTART: &str = "\
globals
timer t = CreateTimer()
timer retained = t
integer marker = 0
real elapsed = -1.0
real remaining = -1.0
boolean same = false
endglobals
function fire takes nothing returns nothing
set marker = marker + 1
set same = GetExpiredTimer() == retained
set elapsed = TimerGetElapsed(retained)
set remaining = TimerGetRemaining(retained)
if marker == 1 then
call TimerStart(retained, 0.125, false, function fire)
else
call DestroyTimer(retained)
endif
endfunction
function config takes nothing returns nothing
endfunction
function main takes nothing returns nothing
call TimerStart(t, 0.5, true, function fire)
endfunction
";

pub(super) fn timer_app() -> App {
    let mut app = App::new();
    peacecraft3::map_requests::install(&mut app);
    peacecraft3::campaign_startup::install(&mut app);
    app
}
fn deliver_timer(app: &mut App, script: &str, revision: u64) {
    deliver_timer_mode(app, script, revision, false);
}

#[test]
fn campaign_runtime_catches_up_without_dropping_load_debt() {
    use peacecraft3::campaign_runtime::CampaignRuntime;
    use std::time::Duration;
    let script = "\
globals
timer t = CreateTimer()
integer marker = 0
real elapsed = -1.0
endglobals
function fire takes nothing returns nothing
set marker = marker + 1
set elapsed = TimerGetElapsed(t)
endfunction
function config takes nothing returns nothing
endfunction
function main takes nothing returns nothing
call TimerStart(t, 0.125, true, function fire)
endfunction
";
    let run = |chunks: &[u64]| {
        let mut app = timer_app();
        deliver_timer(&mut app, script, 1);
        let mut runtime = CampaignRuntime::new(Duration::from_millis(125), 2, 8);
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        runtime.advance(&mut session, Duration::from_secs(50));
        assert_eq!(runtime.tick(), 0, "loading time is excluded");
        for chunk in chunks {
            runtime.advance(&mut session, Duration::from_millis(*chunk));
            assert!(runtime.last_error.is_none(), "{:?}", runtime.last_error);
        }
        while !runtime.pending().is_zero() {
            runtime.advance(&mut session, Duration::ZERO);
            assert!(runtime.last_error.is_none());
        }
        (
            runtime.tick(),
            session.global("marker"),
            session.global("elapsed"),
            session
                .host()
                .unwrap()
                .objects()
                .timer_game_time()
                .unwrap()
                .to_bits(),
        )
    };
    let expected = run(&[125; 8]);
    assert_eq!(expected.0, 8);
    assert_eq!(expected.1, Some(Value::Int(8)));
    assert_eq!(run(&[1000]), expected);
    assert_eq!(run(&[0, 13, 112, 7, 618, 250]), expected);
}

#[test]
fn campaign_runtime_pause_replacement_and_failed_callback_do_not_replay_effects() {
    use peacecraft3::campaign_runtime::CampaignRuntime;
    use std::time::Duration;
    let mut app = timer_app();
    deliver_timer(&mut app, RESTART, 1);
    let mut runtime = CampaignRuntime::new(Duration::from_millis(125), 1, 8);
    {
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        runtime.advance(&mut session, Duration::ZERO);
        runtime.advance(&mut session, Duration::from_secs(1));
        assert_eq!(runtime.tick(), 1);
        assert_eq!(runtime.pending(), Duration::from_millis(875));
        runtime.paused = true;
        runtime.advance(&mut session, Duration::from_secs(10));
        assert_eq!(runtime.tick(), 1);
        assert_eq!(runtime.pending(), Duration::from_millis(875));
    }
    // A declared native with an invalid handle fails after the callback's first effect.
    let failing = RESTART.replace(
        "set marker = marker + 1",
        "set marker = marker + 1\ncall SetSoundPitch(null, 1.0)",
    );
    deliver_timer(&mut app, &failing, 2);
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    runtime.paused = false;
    runtime.advance(&mut session, Duration::from_secs(10));
    assert_eq!(runtime.tick(), 0);
    assert_eq!(runtime.pending(), Duration::ZERO);
    runtime.advance(&mut session, Duration::from_secs(1));
    runtime.advance(&mut session, Duration::ZERO);
    runtime.advance(&mut session, Duration::ZERO);
    runtime.advance(&mut session, Duration::ZERO);
    assert!(runtime.last_error.is_some());
    assert_eq!(session.global("marker"), Some(Value::Int(1)));
    runtime.advance(&mut session, Duration::from_secs(10));
    assert_eq!(session.global("marker"), Some(Value::Int(1)));
}
#[test]
fn campaign_runtime_application_system_advances_the_published_host() {
    use peacecraft3::campaign_runtime::{self, CampaignRuntime};
    use std::time::Duration;
    let mut app = timer_app();
    app.init_resource::<Time<Real>>();
    campaign_runtime::install(
        &mut app,
        CampaignRuntime::new(Duration::from_millis(125), 2, 8),
    );
    deliver_timer(&mut app, RESTART, 1);
    // The next frame's delta includes startup work done after Time<Real> was sampled.
    app.world_mut()
        .resource_mut::<Time<Real>>()
        .advance_by(Duration::from_secs(10_000));
    app.update();
    assert_eq!(app.world().resource::<CampaignRuntime>().tick(), 0);
    assert!(
        app.world()
            .resource::<CampaignRuntime>()
            .pending()
            .is_zero()
    );
    app.world_mut()
        .resource_mut::<Time<Real>>()
        .advance_by(Duration::from_secs(1));
    app.update();
    assert_eq!(app.world().resource::<CampaignRuntime>().tick(), 2);
    assert_eq!(
        app.world().resource::<CampaignRuntime>().pending(),
        Duration::from_millis(750)
    );
    for _ in 0..3 {
        app.world_mut()
            .resource_mut::<Time<Real>>()
            .advance_by(Duration::ZERO);
        app.update();
    }
    let runtime = app.world().resource::<CampaignRuntime>();
    assert_eq!(runtime.tick(), 8);
    assert!(runtime.pending().is_zero());
    assert!(runtime.last_error.is_none());
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    assert_eq!(session.global("marker"), Some(Value::Int(2)));
    assert_eq!(
        session.host().unwrap().objects().timer_game_time(),
        Some(11.0)
    );
}

pub(super) fn deliver_timed_trigger(app: &mut App, script: &str, revision: u64) {
    deliver_timer_mode(app, script, revision, true);
}
fn deliver_timer_mode(app: &mut App, script: &str, revision: u64, timed: bool) {
    let common = if timed {
        super::timed_trigger_cases::COMMON
    } else {
        ""
    };
    deliver_timer_with(app, script, revision, common, |inputs| {
        if timed {
            inputs.trigger_runtime = Some(pcx_play::playercolor_host::TriggerRuntimeInput {
                initial_enabled: true,
                timer_event_global: "TEST_TIMER_EVENT".into(),
                max_timed_events: 16,
                max_parked_threads: 16,
            });
            inputs.enumeration.names.push("ConvertGameEvent".into());
            inputs.enumeration.limits.max_handles = 256;
            inputs.budgets.triggers.max_triggers = 16;
            inputs.budgets.triggers.max_actions = 32;
            inputs.budgets.conditions =
                Some(pcx_play::playercolor_host::ConditionBudget { max_records: 16 });
            inputs.budgets.trigger_conditions = Some(16);
        }
    });
}
fn deliver_timer_with(
    app: &mut App,
    script: &str,
    revision: u64,
    common: &str,
    configure: impl FnOnce(&mut MapStartupInputs),
) {
    deliver_timer_with_assets(app, script, revision, common, BTreeMap::new(), configure);
}

/// `assets` are extra selected-source files; the bindings retain the resolver that reads them.
fn deliver_timer_with_assets(
    app: &mut App,
    script: &str,
    revision: u64,
    common: &str,
    assets: BTreeMap<String, crate::binding::Reply>,
    configure: impl FnOnce(&mut MapStartupInputs),
) {
    let mut map = fixture(script);
    map.common.extend_from_slice(TIMER_COMMON.as_bytes());
    map.common.extend_from_slice(common.as_bytes());
    map.rebuild();
    let asset_count = assets.len();
    let resolver = Arc::new(map.resolver_with_assets(assets));
    // The selected-source extras are listed by the same resolver, so the listing bound grows.
    let mut limits = map.limits();
    limits.reader.max_listed_paths += asset_count;
    let mut bound = bind_map(&resolver, map.input(revision), limits).unwrap();
    bound.resolver = Some(Arc::clone(&resolver));
    let mut inputs = host_inputs(Arc::clone(&bound.catalog.0));
    inputs.timer_time = Some(10.0);
    configure(&mut inputs);
    let bytes = Arc::clone(&bound.catalog_map_bytes);
    app.insert_resource(MapRequests {
        bindings: Some(bound),
        ..Default::default()
    });
    app.insert_resource(CampaignHostSupply {
        revision,
        inputs: Some(inputs),
    });
    app.world_mut().write_message(MapRequest {
        label: "synthetic timer map".into(),
        map_member: pcx_assets::AssetPath::new("selected map.w3m").unwrap(),
        identity: map.map_id.clone(),
        binding_revision: revision,
        payload: MapPayload::Shared(bytes),
    });
    app.update();
    assert!(
        app.world()
            .resource::<MapRequests>()
            .last_outcome
            .as_ref()
            .unwrap()
            .result
            .is_ok(),
        "{:?}",
        app.world()
            .resource::<MapRequests>()
            .last_outcome
            .as_ref()
            .unwrap()
            .result
    );
}

#[test]
fn published_timer_callbacks_restart_and_destroy_in_the_existing_vm() {
    let mut app = timer_app();
    deliver_timer(&mut app, RESTART, 1);
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_none()
    );
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    assert_eq!(session.global("t"), session.global("retained"));
    let count = session.host().unwrap().objects().inner().handles();
    assert!(matches!(
        session.advance_timers(12.0, 1),
        Err(TimerSessionError::Timer(_))
    ));
    assert_eq!(session.global("marker"), Some(Value::Int(1)));
    assert_eq!(session.global("same"), Some(Value::Bool(true)));
    assert_eq!(session.global("elapsed"), Some(Value::Real(0.5)));
    assert_eq!(session.global("remaining"), Some(Value::Real(0.0)));
    assert_eq!(
        session.host().unwrap().objects().timer_game_time(),
        Some(10.5)
    );
    assert_eq!(session.advance_timers(12.0, 1).unwrap(), 1);
    assert_eq!(session.global("marker"), Some(Value::Int(2)));
    assert_eq!(session.global("elapsed"), Some(Value::Real(0.125)));
    assert_eq!(session.host().unwrap().objects().live_timers(), 0);
    assert_eq!(session.host().unwrap().objects().inner().handles(), count);
    assert_eq!(session.advance_timers(20.0, 0).unwrap(), 0);
}

#[test]
fn failed_replacement_keeps_timers_success_and_clear_retire_them() {
    let mut app = timer_app();
    deliver_timer(&mut app, RESTART, 1);
    let witness = app
        .world()
        .resource::<MapScriptSession<MapStartupHost>>()
        .session_witness()
        .unwrap();
    deliver_timer(
        &mut app,
        &RESTART.replace("call TimerStart(t, 0.5", "call TimerStart(null, 0.5"),
        2,
    );
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_some()
    );
    assert!(witness.upgrade().is_some());
    {
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        assert_eq!(session.advance_timers(10.5, 1).unwrap(), 1);
        assert_eq!(session.global("marker"), Some(Value::Int(1)));
    }
    deliver_timer(&mut app, RESTART, 3);
    assert!(witness.upgrade().is_none());
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    assert_eq!(session.global("marker"), Some(Value::Int(0)));
    assert_eq!(
        session.host().unwrap().objects().timer_game_time(),
        Some(10.0)
    );
    let witness = session.session_witness().unwrap();
    session.clear();
    assert!(witness.upgrade().is_none());
    assert!(matches!(
        session.advance_timers(11.0, 1),
        Err(TimerSessionError::Session(ScriptSessionError::NoSession))
    ));
}

#[test]
fn callback_failure_clears_expired_context_without_replaying_one_shot() {
    let script = "\
globals
timer t = CreateTimer()
integer marker = 0
endglobals
function fire takes nothing returns nothing
set marker = marker + 1
call TimerStart(null, 1.0, false, null)
endfunction
function config takes nothing returns nothing
endfunction
function main takes nothing returns nothing
call TimerStart(t, 0.5, false, function fire)
endfunction
";
    let mut app = timer_app();
    deliver_timer(&mut app, script, 1);
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    assert!(matches!(
        session.advance_timers(11.0, 1),
        Err(TimerSessionError::Session(ScriptSessionError::Execution(_)))
    ));
    assert_eq!(session.global("marker"), Some(Value::Int(1)));
    let config = session
        .context()
        .unwrap()
        .scripts
        .program()
        .function_id("config")
        .unwrap();
    session.call_function(config, &[]).unwrap();
    use pcx_script::Host;
    assert!(
        session
            .host()
            .unwrap()
            .objects_mut()
            .native("GetExpiredTimer", &[])
            .is_err()
    );
    assert_eq!(session.advance_timers(11.0, 1).unwrap(), 0);
    assert_eq!(session.global("marker"), Some(Value::Int(1)));
}

#[test]
fn registered_expiry_refuses_before_clock_or_handler_mutation() {
    let script = RESTART.replace("call TimerStart(t, 0.5, true, function fire)", "call TriggerRegisterTimerExpireEvent(CreateTrigger(), t)\ncall TimerStart(t, 0.5, true, function fire)");
    let mut app = timer_app();
    deliver_timer(&mut app, &script, 1);
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_none()
    );
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let error = session.advance_timers(11.0, 1).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("expiry-trigger execution is unsupported")
    );
    assert_eq!(session.global("marker"), Some(Value::Int(0)));
    assert_eq!(
        session.host().unwrap().objects().timer_game_time(),
        Some(10.0)
    );
    assert!(session.advance_timers(11.0, 1).is_err());
}

#[test]
fn destroyed_expired_timer_start_keeps_its_own_run() {
    let script = "\
globals
timer expired = CreateTimer()
timer next = null
integer marker = 0
real elapsed = -1.0
real remaining = -1.0
endglobals
function fire takes nothing returns nothing
set marker = marker + 1
call DestroyTimer(expired)
set next = CreateTimer()
call TimerStart(next, 1.0, false, null)
set elapsed = TimerGetElapsed(next)
set remaining = TimerGetRemaining(next)
endfunction
function config takes nothing returns nothing
endfunction
function main takes nothing returns nothing
call TimerStart(expired, 0.5, false, function fire)
endfunction
";
    let mut app = timer_app();
    deliver_timer(&mut app, script, 1);
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    assert_eq!(session.advance_timers(11.0, 1).unwrap(), 1);
    assert_eq!(session.global("marker"), Some(Value::Int(1)));
    assert_eq!(session.global("elapsed"), Some(Value::Real(0.0)));
    assert_eq!(session.global("remaining"), Some(Value::Real(1.0)));
    assert_eq!(session.advance_timers(12.0, 1).unwrap(), 1);
    assert_eq!(session.global("marker"), Some(Value::Int(1)));
    let next = session.global("next").unwrap();
    use pcx_script::Host;
    let objects = session.host().unwrap().objects_mut();
    assert_eq!(
        objects.native("TimerGetElapsed", std::slice::from_ref(&next)),
        Ok(Value::Real(1.0))
    );
    assert_eq!(
        objects.native("TimerGetRemaining", std::slice::from_ref(&next)),
        Ok(Value::Real(0.0))
    );
}

#[test]
fn periodic_catch_up_spends_one_budget_per_callback_and_destruction_stops_it() {
    let script = RESTART.replace("if marker == 1 then\ncall TimerStart(retained, 0.125, false, function fire)\nelse\ncall DestroyTimer(retained)\nendif", "if marker == 3 then\ncall DestroyTimer(retained)\nendif");
    let mut app = timer_app();
    deliver_timer(&mut app, &script, 1);
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    assert!(session.advance_timers(15.0, 2).is_err());
    assert_eq!(session.global("marker"), Some(Value::Int(2)));
    assert_eq!(
        session.host().unwrap().objects().timer_game_time(),
        Some(11.0)
    );
    assert_eq!(session.advance_timers(15.0, 1).unwrap(), 1);
    assert_eq!(session.global("marker"), Some(Value::Int(3)));
    assert_eq!(session.global("elapsed"), Some(Value::Real(0.5)));
    assert_eq!(session.advance_timers(30.0, 0).unwrap(), 0);
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn browser_timer_callback_lifetime_gate() -> u32 {
    published_timer_callbacks_restart_and_destroy_in_the_existing_vm();
    failed_replacement_keeps_timers_success_and_clear_retire_them();
    callback_failure_clears_expired_context_without_replaying_one_shot();
    registered_expiry_refuses_before_clock_or_handler_mutation();
    destroyed_expired_timer_start_keeps_its_own_run();
    periodic_catch_up_spends_one_budget_per_callback_and_destruction_stops_it();
    6
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn browser_campaign_runtime_gate() -> u32 {
    campaign_runtime_catches_up_without_dropping_load_debt();
    campaign_runtime_pause_replacement_and_failed_callback_do_not_replay_effects();
    campaign_runtime_application_system_advances_the_published_host();
    game_pause::executed_pause_finishes_thread_and_holds_future_timer_delivery();
    game_pause::runtime_rebases_a_mid_tick_pause_and_discards_paused_wall_time();
    game_pause::queued_dialog_resume_runs_while_the_simulation_clock_is_paused();
    game_pause::paused_session_preserves_parked_threads_until_game_time_resumes();
    game_exit::normal_quit_button_in_paused_state_ends_the_session_in_the_delivering_update();
    game_exit::second_end_game_past_the_queue_bound_fails_the_callback_and_keeps_the_first_request(
    );
    game_exit::ended_session_stays_ended_without_runtime_errors_across_later_updates();
    game_exit::replacement_session_starts_without_inherited_exit_and_keeps_running();
    11
}

#[path = "script_game_pause_cases.rs"]
mod game_pause;

#[path = "script_game_exit_cases.rs"]
mod game_exit;

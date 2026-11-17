//! Cross-program listeners through the published map session, never AI function IDs.
use super::*;
use pcx_play::map_startup_host::MapStartupError;
use pcx_script::Host;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

fn publish(
    native: bool,
    ai_body: &str,
    observer: &str,
) -> (MapScriptSession<MapStartupHost>, Arc<AtomicUsize>) {
    publish_with_observers(native, ai_body, observer, true)
}

fn publish_with_observers(
    native: bool,
    ai_body: &str,
    observer: &str,
    watch: bool,
) -> (MapScriptSession<MapStartupHost>, Arc<AtomicUsize>) {
    let source = format!(
        r#"
globals
trigger watched=CreateTrigger()
trigger other=CreateTrigger()
trigger queued=CreateTrigger()
timer anchor=CreateTimer()
integer calls=0
integer queuedCalls=0
integer trace=0
integer oldTail=0
boolean accepts=true
boolean conditionFails=false
integer firstSeen=0
integer secondSeen=0
real before=0.0
real after=0.0
endglobals
function queuedAction takes nothing returns nothing
set queuedCalls=queuedCalls+1
endfunction
function nested takes nothing returns nothing
set trace=trace*10+4
endfunction
function accepted takes nothing returns boolean
set trace=trace*10+1
if conditionFails then
return 1/0==0
endif
return accepts
endfunction
function failConditions takes nothing returns nothing
set conditionFails=true
endfunction
function reject takes nothing returns nothing
set accepts=false
endfunction
function secondCondition takes nothing returns boolean
set trace=trace*10+2
return true
endfunction
function observe takes nothing returns nothing
local integer value=GetPlayerState(Player(0),PLAYER_STATE_RESOURCE_LUMBER)
set before=TimerGetElapsed(anchor)
set calls=calls+1
if calls==1 then
set firstSeen=value
else
set secondSeen=value
endif
set trace=trace*10+3
{observer}
call TriggerExecute(other)
set trace=trace*10+5
set after=TimerGetElapsed(anchor)
endfunction
function tail takes nothing returns nothing
set trace=trace*10+6
endfunction
function config takes nothing returns nothing
call SetPlayers(2)
endfunction
function main takes nothing returns nothing
if {watch} then
call TriggerRegisterPlayerStateEvent(watched,Player(0),PLAYER_STATE_RESOURCE_GOLD,EQUAL,7.0)
call TriggerRegisterPlayerStateEvent(watched,Player(0),PLAYER_STATE_RESOURCE_GOLD,EQUAL,9.0)
endif
call TriggerAddCondition(watched,Condition(function accepted))
call TriggerAddCondition(watched,Condition(function secondCondition))
call TriggerAddAction(watched,function observe)
call TriggerAddAction(watched,function tail)
call TriggerAddAction(other,function nested)
if {watch} then
call TriggerRegisterPlayerStateEvent(queued,Player(1),PLAYER_STATE_RESOURCE_GOLD,EQUAL,42.0)
endif
call TriggerAddAction(queued,function queuedAction)
call TimerStart(anchor,10.0,false,null)
endfunction
function parkOld takes nothing returns nothing
call TriggerSleepAction(0.5)
set oldTail=73
endfunction
function startAfterPark takes nothing returns nothing
call TriggerSleepAction(0.0317)
call StartCampaignAI(Player(0),"observed.ai")
endfunction
function startOther takes nothing returns nothing
call StartCampaignAI(Player(1),"observed.ai")
endfunction
function start takes nothing returns nothing
call StartCampaignAI(Player(0),"observed.ai")
endfunction
"#
    );
    let mut map = fixture(&source);
    map.common
        .extend_from_slice(script_timer_cases::TIMER_COMMON.as_bytes());
    map.common
        .extend_from_slice(super::super::super::timed_trigger_cases::COMMON.as_bytes());
    map.common.extend_from_slice(b"native StartCampaignAI takes player p, string path returns nothing\nconstant native GetPlayerState takes player p, playerstate state returns integer\nnative TriggerSleepAction takes real delay returns nothing\nnative TriggerWaitForSound takes sound s, real offset returns nothing\nglobals\nconstant playerstate PLAYER_STATE_RESOURCE_GOLD=ConvertPlayerState(1)\nconstant playerstate PLAYER_STATE_RESOURCE_LUMBER=ConvertPlayerState(2)\nconstant playerstate PLAYER_STATE_RESOURCE_FOOD_USED=ConvertPlayerState(5)\nconstant playerstate PLAYER_STATE_RESOURCE_FOOD_CAP=ConvertPlayerState(4)\nconstant playerstate PLAYER_STATE_FOOD_CAP_CEILING=ConvertPlayerState(6)\nconstant playerstate PLAYER_STATE_GOLD_GATHERED=ConvertPlayerState(15)\nconstant playerstate PLAYER_STATE_LUMBER_GATHERED=ConvertPlayerState(16)\n");
    map.common.extend_from_slice(b"endglobals\n");
    map.rebuild();
    let reader = map.resolver();
    let bound = bind_map(&reader, map.input(1), map.limits()).unwrap();
    let mut inputs = host_inputs(Arc::clone(&bound.catalog.0));
    inputs.resources = Some(pcx_play::player_resources::Rules::synthetic_fixture());
    // The foreign-program case initializes a second global store in the same host.
    inputs.budgets.triggers.max_triggers *= 2;
    inputs.enumeration.names.push("ConvertGameEvent".into());
    inputs.timer_time = Some(0.);
    inputs.budgets.conditions =
        Some(pcx_play::playercolor_host::ConditionBudget { max_records: 16 });
    inputs.budgets.trigger_conditions = Some(16);
    inputs.trigger_runtime = Some(pcx_play::playercolor_host::TriggerRuntimeInput {
        initial_enabled: true,
        timer_event_global: "TEST_TIMER_EVENT".into(),
        max_timed_events: 16,
        max_parked_threads: 8,
    });
    inputs.numeric = Some(super::numeric(true));
    let mut app = App::new();
    peacecraft3::map_requests::install(&mut app);
    let bytes = Arc::clone(&bound.catalog_map_bytes);
    app.insert_resource(MapRequests {
        bindings: Some(bound),
        ..Default::default()
    });
    app.world_mut().write_message(MapRequest {
        label: "synthetic observed AI map".into(),
        map_member: pcx_assets::AssetPath::new("selected map.w3m").unwrap(),
        identity: map.map_id,
        binding_revision: 1,
        payload: MapPayload::Shared(bytes),
    });
    app.update();
    let context = app
        .world()
        .resource::<MapRequests>()
        .admission
        .shared_context()
        .unwrap();
    let ai = Arc::new(
        pcx_play::scripts::MapScripts::compile(
            AssetIdentity {
                edition: "synthetic".into(),
                version: "ai-map-bridge".into(),
                source: "observed.ai".into(),
            },
            ScriptAsset {
                label: "fixture:common.j",
                bytes: &map.common,
            },
            ScriptAsset {
                label: "fixture:common.ai",
                bytes: AI_COMMON.as_bytes(),
            },
            ScriptAsset {
                label: "observed.ai",
                bytes: ai_body.as_bytes(),
            },
            None,
        )
        .unwrap(),
    );
    let entries =
        ["config", "main"].map(|name| context.scripts.program().function_id(name).unwrap());
    let loads = Arc::new(AtomicUsize::new(0));
    let loaded = Arc::clone(&loads);
    let mut session = MapScriptSession::default();
    session
        .replace(
            context,
            move |ctx| {
                let mut host =
                    pcx_play::map_startup_host::assemble(ctx, inputs)?.with_ai_loader(move |_| {
                        loaded.fetch_add(1, Ordering::SeqCst);
                        Ok(Arc::clone(&ai))
                    });
                if native {
                    host.objects_mut()
                        .select_native_counter_platform()
                        .map_err(MapStartupError::from)?;
                }
                Ok::<_, MapStartupError>(host)
            },
            &entries,
        )
        .unwrap();
    (session, loads)
}
fn start(
    session: &mut MapScriptSession<MapStartupHost>,
) -> Result<Value, peacecraft3::map_script_session::ScriptSessionError> {
    let id = session
        .context()
        .unwrap()
        .scripts
        .program()
        .function_id("start")
        .unwrap();
    session.call_function(id, &[])
}
fn bank(session: &mut MapScriptSession<MapStartupHost>, player: i32, state: &str) -> Value {
    let state = session.global(state).unwrap();
    let host = session.host().unwrap().objects_mut();
    let player = host.native("Player", &[Value::Int(player)]).unwrap();
    host.native("GetPlayerState", &[player, state]).unwrap()
}
const AI: &str = r#"
function main takes nothing returns nothing
local player p=Player(GetAiPlayer())
local integer seen=0
call SetPlayerState(p,PLAYER_STATE_RESOURCE_LUMBER,11)
call SetPlayerState(p,PLAYER_STATE_RESOURCE_GOLD,7)
set seen=GetPlayerState(p,PLAYER_STATE_RESOURCE_LUMBER)
call SetPlayerState(p,PLAYER_STATE_RESOURCE_LUMBER,22)
call SetPlayerState(Player(1),PLAYER_STATE_RESOURCE_GOLD,seen)
call Sleep(0.0317)
call SetPlayerState(p,PLAYER_STATE_RESOURCE_LUMBER,31)
call SetPlayerState(p,PLAYER_STATE_RESOURCE_GOLD,9)
set seen=GetPlayerState(p,PLAYER_STATE_RESOURCE_LUMBER)
call SetPlayerState(p,PLAYER_STATE_RESOURCE_LUMBER,42)
call SetPlayerState(Player(1),PLAYER_STATE_RESOURCE_LUMBER,seen)
call Sleep(0.0317)
call SetPlayerState(Player(1),PLAYER_STATE_RESOURCE_GOLD,99)
endfunction
"#;

#[test]
#[ignore = "requires explicit PCX_WAR3_EXE selected executable"]
fn ai_map_bridge_immediate_reread_nested_callbacks_and_repark_keep_game_scope() {
    for native in [false, true] {
        let (mut session, loads) = publish(
            native,
            AI,
            "call SetPlayerState(Player(0),PLAYER_STATE_RESOURCE_LUMBER,value+1)",
        );
        start(&mut session).unwrap();
        assert_eq!(loads.load(Ordering::SeqCst), 1);
        assert_eq!(session.global("calls"), Some(Value::Int(1)));
        assert_eq!(session.global("trace"), Some(Value::Int(123456)));
        assert_eq!(session.global("firstSeen"), Some(Value::Int(11)));
        assert_eq!(
            bank(&mut session, 1, "PLAYER_STATE_RESOURCE_GOLD"),
            Value::Int(12)
        );
        assert_eq!(
            bank(&mut session, 0, "PLAYER_STATE_RESOURCE_LUMBER"),
            Value::Int(22)
        );
        if native {
            session
                .advance_native_frame(
                    Some(pcx_play::native_game_clock::FrameInput {
                        game_elapsed: Duration::from_millis(125),
                        real_elapsed_ms: 125,
                    }),
                    128,
                )
                .unwrap();
        } else {
            session.advance_timers(0.125, 128).unwrap();
        }
        assert_eq!(session.global("calls"), Some(Value::Int(2)));
        assert_eq!(session.global("secondSeen"), Some(Value::Int(31)));
        assert_eq!(
            bank(&mut session, 1, "PLAYER_STATE_RESOURCE_LUMBER"),
            Value::Int(32)
        );
        assert_eq!(
            bank(&mut session, 1, "PLAYER_STATE_RESOURCE_GOLD"),
            Value::Int(99)
        );
        assert_eq!(
            bank(&mut session, 0, "PLAYER_STATE_RESOURCE_LUMBER"),
            Value::Int(42)
        );
        assert_eq!(session.global("before"), session.global("after"));
        let Some(Value::Real(time)) = session.global("before") else {
            panic!("source timer query");
        };
        assert!(time > 0. && time < 0.125, "{native}: {time}");
        assert_eq!(
            session
                .host()
                .unwrap()
                .native("GetTriggeringTrigger", &[])
                .unwrap(),
            Value::Null
        );
        assert_eq!(session.host().unwrap().objects().parked_thread_count(), 0);
    }
}

#[test]
#[ignore = "requires explicit PCX_WAR3_EXE selected executable"]
fn ai_map_bridge_wake_error_preserves_original_map_failure_and_blocks_all_retries() {
    use peacecraft3::map_script_session::TimerSessionError;
    for native in [false, true] {
        let observer = "if calls==2 then\nset trace=1/0\nendif\ncall SetPlayerState(Player(0),PLAYER_STATE_RESOURCE_LUMBER,value+1)";
        let (mut session, _) = publish(native, AI, observer);
        let parked = session
            .context()
            .unwrap()
            .scripts
            .program()
            .function_id("parkOld")
            .unwrap();
        session.run_thread(&[parked]).unwrap();
        assert_eq!(session.host().unwrap().objects().parked_thread_count(), 1);
        start(&mut session).unwrap();
        let error = if native {
            session
                .advance_native_frame(
                    Some(pcx_play::native_game_clock::FrameInput {
                        game_elapsed: Duration::from_millis(125),
                        real_elapsed_ms: 125,
                    }),
                    128,
                )
                .unwrap_err()
        } else {
            session.advance_timers(0.125, 128).unwrap_err()
        };
        let TimerSessionError::AiBridge(fault) = error else {
            panic!("owned AI map callback failure: {error}");
        };
        assert!(fault.contains("division by zero"), "{fault}");
        assert!(fault.contains("war3map.j:"), "{fault}");
        assert_eq!(session.global("calls"), Some(Value::Int(2)));
        assert_eq!(
            bank(&mut session, 0, "PLAYER_STATE_RESOURCE_GOLD"),
            Value::Int(9)
        );
        assert_eq!(
            bank(&mut session, 0, "PLAYER_STATE_RESOURCE_LUMBER"),
            Value::Int(31)
        );
        assert_eq!(
            bank(&mut session, 1, "PLAYER_STATE_RESOURCE_GOLD"),
            Value::Int(12)
        );
        assert_eq!(
            bank(&mut session, 1, "PLAYER_STATE_RESOURCE_LUMBER"),
            Value::Int(0)
        );
        assert_eq!(session.host().unwrap().objects().parked_thread_count(), 1);
        assert_eq!(session.global("oldTail"), Some(Value::Int(0)));
        let before = format!(
            "{:?}",
            session.host().unwrap().objects().test_native_clock_state()
        );
        let now = session.host().unwrap().objects().timer_game_time();
        for retry in 0..3 {
            let error = match retry {
                0 => session
                    .advance_native_frame(
                        Some(pcx_play::native_game_clock::FrameInput {
                            game_elapsed: Duration::from_secs(1),
                            real_elapsed_ms: 1000,
                        }),
                        128,
                    )
                    .unwrap_err(),
                1 => session.advance_native_frame(None, 128).unwrap_err(),
                _ => pcx_play::campaign_host::advance_paced_tick(
                    &mut session,
                    Duration::from_secs(1),
                    1.,
                    128,
                )
                .unwrap_err(),
            };
            let TimerSessionError::AiBridge(repeated) = error else {
                panic!("stored callback failure: {error}");
            };
            assert!(Arc::ptr_eq(&fault, &repeated));
            assert_eq!(session.host().unwrap().objects().timer_game_time(), now);
            assert_eq!(
                format!(
                    "{:?}",
                    session.host().unwrap().objects().test_native_clock_state()
                ),
                before
            );
            assert_eq!(session.global("calls"), Some(Value::Int(2)));
        }
        assert!(!session.native_pacing_can_resume(&TimerSessionError::WakeBudget));
        let budget = pcx_script::checkpoint::Budget {
            bytes: 1 << 20,
            items: 4096,
            depth: 32,
        };
        let mut out = pcx_script::checkpoint::Writer::new(budget).unwrap();
        assert!(
            session
                .host()
                .unwrap()
                .write_ai_checkpoint(&mut out, budget)
                .unwrap_err()
                .contains("faulted")
        );
        assert_eq!(trigger_after_fault(&mut session), Value::Null);
    }
}
fn trigger_after_fault(session: &mut MapScriptSession<MapStartupHost>) -> Value {
    session
        .host()
        .unwrap()
        .objects_mut()
        .native("GetTriggeringTrigger", &[])
        .unwrap()
}

#[test]
#[ignore = "requires explicit PCX_WAR3_EXE selected executable"]
fn ai_map_bridge_initializer_and_direct_host_refuse_before_watched_write() {
    const INITIALIZER: &str = "globals\ninteger INIT=initial()\nendglobals\nfunction initial takes nothing returns integer\ncall SetPlayerState(Player(0),PLAYER_STATE_RESOURCE_GOLD,7)\nreturn 1\nendfunction\nfunction main takes nothing returns nothing\nendfunction\n";
    for native in [false, true] {
        let (mut session, loads) = publish(
            native,
            INITIALIZER,
            "call SetPlayerState(Player(0),PLAYER_STATE_RESOURCE_LUMBER,value+1)",
        );
        let error = start(&mut session).unwrap_err().to_string();
        assert!(error.contains("initializer or direct host"), "{error}");
        assert_eq!(loads.load(Ordering::SeqCst), 1);
        assert_eq!(session.global("calls"), Some(Value::Int(0)));
        assert_eq!(
            bank(&mut session, 0, "PLAYER_STATE_RESOURCE_GOLD"),
            Value::Int(0)
        );
        let (mut direct, loads) = publish(
            native,
            AI,
            "call SetPlayerState(Player(0),PLAYER_STATE_RESOURCE_LUMBER,value+1)",
        );
        let player = direct
            .host()
            .unwrap()
            .native("Player", &[Value::Int(0)])
            .unwrap();
        let error = direct
            .host()
            .unwrap()
            .native(
                "StartCampaignAI",
                &[player, Value::Str("observed.ai".into())],
            )
            .unwrap_err();
        assert!(error.contains("initializer or direct host"), "{error}");
        assert_eq!(loads.load(Ordering::SeqCst), 1);
        assert_eq!(direct.global("calls"), Some(Value::Int(0)));
        assert_eq!(
            bank(&mut direct, 0, "PLAYER_STATE_RESOURCE_GOLD"),
            Value::Int(0)
        );
    }
}

#[test]
#[ignore = "requires explicit PCX_WAR3_EXE selected executable"]
fn ai_map_bridge_sleep_and_reentrant_start_refuse_before_their_effects() {
    for native in [false, true] {
        for observer in [
            "call TriggerSleepAction(1.0)",
            "call TriggerWaitForSound(null,0.0)",
            "call StartCampaignAI(Player(1),\"observed.ai\")",
        ] {
            let (mut session, loads) = publish(native, AI, observer);
            let error = start(&mut session).unwrap_err().to_string();
            assert!(error.contains("not qualified"), "{error}");
            assert_eq!(loads.load(Ordering::SeqCst), 1);
            assert_eq!(session.global("calls"), Some(Value::Int(1)));
            assert_eq!(
                bank(&mut session, 0, "PLAYER_STATE_RESOURCE_GOLD"),
                Value::Int(7)
            );
            assert_eq!(
                bank(&mut session, 0, "PLAYER_STATE_RESOURCE_LUMBER"),
                Value::Int(11)
            );
            assert_eq!(
                bank(&mut session, 1, "PLAYER_STATE_RESOURCE_GOLD"),
                Value::Int(0)
            );
            assert_eq!(session.host().unwrap().objects().parked_thread_count(), 0);
            assert_eq!(
                session.host().unwrap().objects().timer_game_time(),
                Some(0.)
            );
        }
    }
}

#[test]
#[ignore = "requires explicit PCX_WAR3_EXE selected executable"]
fn ai_map_bridge_false_conditions_and_disabled_observers_return_without_actions() {
    for native in [false, true] {
        for disabled in [false, true] {
            let (mut session, _) = publish(
                native,
                AI,
                "call SetPlayerState(Player(0),PLAYER_STATE_RESOURCE_LUMBER,value+1)",
            );
            if disabled {
                let watched = session.global("watched").unwrap();
                session
                    .host()
                    .unwrap()
                    .native("DisableTrigger", &[watched])
                    .unwrap();
            } else {
                let rejected = session
                    .context()
                    .unwrap()
                    .scripts
                    .program()
                    .function_id("reject")
                    .unwrap();
                session.call_function(rejected, &[]).unwrap();
            }
            start(&mut session).unwrap();
            assert_eq!(
                bank(&mut session, 1, "PLAYER_STATE_RESOURCE_GOLD"),
                Value::Int(11)
            );
            assert_eq!(session.global("calls"), Some(Value::Int(0)));
            if native {
                session
                    .advance_native_frame(
                        Some(pcx_play::native_game_clock::FrameInput {
                            game_elapsed: Duration::from_millis(125),
                            real_elapsed_ms: 125,
                        }),
                        128,
                    )
                    .unwrap();
            } else {
                session.advance_timers(0.125, 128).unwrap();
            }
            assert_eq!(
                bank(&mut session, 1, "PLAYER_STATE_RESOURCE_LUMBER"),
                Value::Int(31)
            );
            assert_eq!(
                bank(&mut session, 1, "PLAYER_STATE_RESOURCE_GOLD"),
                Value::Int(99)
            );
            assert_eq!(session.global("calls"), Some(Value::Int(0)));
            assert_eq!(
                session.global("trace"),
                Some(Value::Int(if disabled { 0 } else { 11 }))
            );
        }
    }
}

#[test]
#[ignore = "requires explicit PCX_WAR3_EXE selected executable"]
fn ai_map_bridge_initial_failure_gates_pure_vm_entries_and_retains_primary_cause() {
    use peacecraft3::map_script_session::TimerSessionError;
    for native in [false, true] {
        let (mut session, loads) = publish(native, AI, "set trace=1/0");
        let program = session.context().unwrap().scripts.shared_program();
        let parked = program.function_id("parkOld").unwrap();
        let nested = program.function_id("nested").unwrap();
        let rejected = program.function_id("reject").unwrap();
        let callback = session.callback_ticket(nested).unwrap();
        session.run_thread(&[parked]).unwrap();
        let original = start(&mut session).unwrap_err().to_string();
        assert!(
            original.contains("division by zero") && original.contains("war3map.j:"),
            "{original}"
        );
        let snapshot_globals = |session: &MapScriptSession<MapStartupHost>| {
            program
                .globals()
                .iter()
                .map(|global| (global.name.clone(), session.global(&global.name)))
                .collect::<Vec<_>>()
        };
        let globals = snapshot_globals(&session);
        assert_eq!(session.host().unwrap().objects().parked_thread_count(), 1);
        assert_eq!(
            bank(&mut session, 0, "PLAYER_STATE_RESOURCE_GOLD"),
            Value::Int(7)
        );
        assert_eq!(
            bank(&mut session, 0, "PLAYER_STATE_RESOURCE_LUMBER"),
            Value::Int(11)
        );
        assert_eq!(
            bank(&mut session, 1, "PLAYER_STATE_RESOURCE_GOLD"),
            Value::Int(0)
        );
        let world = session.host().unwrap().capture_world_snapshot();
        let handles = session.host().unwrap().objects().inner().handles();
        let clock = format!(
            "{:?}",
            session.host().unwrap().objects().test_native_clock_state()
        );
        let mut retained = None;
        for entry in 0..4 {
            let error = match entry {
                0 => session.call_function(rejected, &[]).unwrap_err(),
                1 => session.call_callback(&callback, &[]).unwrap_err(),
                2 => session.run_thread(&[nested]).unwrap_err(),
                _ => session.resume_thread(1).unwrap_err(),
            };
            let error = error.to_string();
            assert!(error.contains(&original), "{error}");
            assert_eq!(snapshot_globals(&session), globals);
            assert_eq!(session.host().unwrap().capture_world_snapshot(), world);
            assert_eq!(session.host().unwrap().objects().inner().handles(), handles);
            assert_eq!(session.host().unwrap().objects().parked_thread_count(), 1);
            assert_eq!(
                format!(
                    "{:?}",
                    session.host().unwrap().objects().test_native_clock_state()
                ),
                clock
            );
        }
        for entry in 0..4 {
            let error = match entry {
                0 => session
                    .advance_native_frame(
                        Some(pcx_play::native_game_clock::FrameInput {
                            game_elapsed: Duration::from_secs(1),
                            real_elapsed_ms: 1000,
                        }),
                        128,
                    )
                    .unwrap_err(),
                1 => session.advance_native_frame(None, 128).unwrap_err(),
                2 => session.advance_timers(1., 128).unwrap_err(),
                _ => pcx_play::campaign_host::advance_paced_tick(
                    &mut session,
                    Duration::from_secs(1),
                    1.,
                    128,
                )
                .unwrap_err(),
            };
            let TimerSessionError::AiBridge(fault) = error else {
                panic!("retained initial failure: {error}");
            };
            assert!(fault.contains(&original), "{fault}");
            if let Some(previous) = &retained {
                assert!(Arc::ptr_eq(previous, &fault));
            } else {
                retained = Some(fault);
            }
            assert_eq!(snapshot_globals(&session), globals);
            assert_eq!(session.host().unwrap().capture_world_snapshot(), world);
            assert_eq!(session.host().unwrap().objects().inner().handles(), handles);
            assert_eq!(session.host().unwrap().objects().parked_thread_count(), 1);
            assert_eq!(
                format!(
                    "{:?}",
                    session.host().unwrap().objects().test_native_clock_state()
                ),
                clock
            );
        }
        assert_eq!(loads.load(Ordering::SeqCst), 1);
        assert_eq!(
            bank(&mut session, 0, "PLAYER_STATE_RESOURCE_GOLD"),
            Value::Int(7)
        );
        assert_eq!(
            bank(&mut session, 0, "PLAYER_STATE_RESOURCE_LUMBER"),
            Value::Int(11)
        );
    }
}

#[test]
#[ignore = "requires explicit PCX_WAR3_EXE selected executable"]
fn ai_map_bridge_ordinary_error_without_capture_preserves_other_ai_and_old_park() {
    for (native, wake) in [(false, false), (false, true), (true, false), (true, true)] {
        let delay = if wake { "call Sleep(0.0317)" } else { "" };
        let ordinary_error = format!(
            "function main takes nothing returns nothing\nlocal integer x=0\nif GetAiPlayer()==0 then\n{delay}\nset x=GetUnitGoldCost('xxxx')\nelse\ncall Sleep(0.0634)\ncall SetPlayerState(Player(1),PLAYER_STATE_RESOURCE_LUMBER,55)\nendif\nendfunction\n"
        );
        let (mut session, loads) = publish(
            native,
            &ordinary_error,
            "call SetPlayerState(Player(0),PLAYER_STATE_RESOURCE_LUMBER,value+1)",
        );
        let program = session.context().unwrap().scripts.shared_program();
        let old = program.function_id("parkOld").unwrap();
        session.run_thread(&[old]).unwrap();
        session
            .call_function(program.function_id("startOther").unwrap(), &[])
            .unwrap();
        let error = if wake {
            start(&mut session).unwrap();
            let error = if native {
                session
                    .advance_native_frame(
                        Some(pcx_play::native_game_clock::FrameInput {
                            game_elapsed: Duration::from_millis(125),
                            real_elapsed_ms: 125,
                        }),
                        128,
                    )
                    .unwrap_err()
            } else {
                session.advance_timers(0.125, 128).unwrap_err()
            };
            assert!(
                matches!(
                    error,
                    peacecraft3::map_script_session::TimerSessionError::Ai(_)
                ),
                "{error}"
            );
            error.to_string()
        } else {
            start(&mut session).unwrap_err().to_string()
        };
        assert!(error.contains("GetUnitGoldCost"), "{error}");
        session
            .call_function(program.function_id("nested").unwrap(), &[])
            .unwrap();
        assert_eq!(session.global("trace"), Some(Value::Int(4)));
        if native {
            session
                .advance_native_frame(
                    (!wake).then_some(pcx_play::native_game_clock::FrameInput {
                        game_elapsed: Duration::from_millis(125),
                        real_elapsed_ms: 125,
                    }),
                    128,
                )
                .unwrap();
        } else {
            session.advance_timers(0.125, 128).unwrap();
        }
        assert_eq!(
            bank(&mut session, 1, "PLAYER_STATE_RESOURCE_LUMBER"),
            Value::Int(55)
        );
        assert_eq!(
            bank(&mut session, 0, "PLAYER_STATE_RESOURCE_GOLD"),
            Value::Int(0)
        );
        assert_eq!(session.global("calls"), Some(Value::Int(0)));
        assert_eq!(session.global("oldTail"), Some(Value::Int(0)));
        assert_eq!(session.host().unwrap().objects().parked_thread_count(), 1);
        assert_eq!(loads.load(Ordering::SeqCst), 2);
    }
}

#[test]
#[ignore = "requires explicit PCX_WAR3_EXE selected executable"]
fn ai_map_bridge_fault_preserves_unrelated_queued_event_and_parked_context() {
    for native in [false, true] {
        let (mut session, _) = publish(native, AI, "set trace=1/0");
        let old = session
            .context()
            .unwrap()
            .scripts
            .program()
            .function_id("parkOld")
            .unwrap();
        session.run_thread(&[old]).unwrap();
        let queued = session.global("queued").unwrap();
        let state = session.global("PLAYER_STATE_RESOURCE_GOLD").unwrap();
        let player = session
            .host()
            .unwrap()
            .objects_mut()
            .native("Player", &[Value::Int(1)])
            .unwrap();
        session
            .host()
            .unwrap()
            .objects_mut()
            .native("SetPlayerState", &[player.clone(), state, Value::Int(42)])
            .unwrap();
        assert_eq!(session.global("queuedCalls"), Some(Value::Int(0)));
        let error = start(&mut session).unwrap_err().to_string();
        assert!(error.contains("division by zero"), "{error}");
        let event = session
            .host()
            .unwrap()
            .objects_mut()
            .take_fired_event()
            .expect("unrelated queued event survives");
        assert_eq!(Value::Handle(event.trigger), queued);
        assert_eq!(event.subject.player.map(Value::Handle), Some(player));
        assert!(
            session
                .host()
                .unwrap()
                .objects_mut()
                .take_fired_event()
                .is_none()
        );
        assert_eq!(session.host().unwrap().objects().parked_thread_count(), 1);
        assert_eq!(session.global("oldTail"), Some(Value::Int(0)));
        assert_eq!(session.global("queuedCalls"), Some(Value::Int(0)));
        assert_eq!(trigger_after_fault(&mut session), Value::Null);
    }
}

#[test]
#[ignore = "requires explicit PCX_WAR3_EXE selected executable"]
fn ai_map_bridge_condition_errors_retain_source_and_committed_write_at_initial_and_wake() {
    use peacecraft3::map_script_session::TimerSessionError;
    for (native, wake) in [(false, false), (false, true), (true, false), (true, true)] {
        let (mut session, _) = publish(
            native,
            AI,
            "call SetPlayerState(Player(0),PLAYER_STATE_RESOURCE_LUMBER,value+1)",
        );
        if wake {
            start(&mut session).unwrap();
        }
        let fail = session
            .context()
            .unwrap()
            .scripts
            .program()
            .function_id("failConditions")
            .unwrap();
        session.call_function(fail, &[]).unwrap();
        let primary = if wake {
            let error = if native {
                session
                    .advance_native_frame(
                        Some(pcx_play::native_game_clock::FrameInput {
                            game_elapsed: Duration::from_millis(125),
                            real_elapsed_ms: 125,
                        }),
                        128,
                    )
                    .unwrap_err()
            } else {
                session.advance_timers(0.125, 128).unwrap_err()
            };
            let TimerSessionError::AiBridge(error) = error else {
                panic!("{error}")
            };
            error.to_string()
        } else {
            start(&mut session).unwrap_err().to_string()
        };
        assert!(
            primary.contains("division by zero") && primary.contains("war3map.j:"),
            "{primary}"
        );
        assert_eq!(session.global("calls"), Some(Value::Int(i32::from(wake))));
        assert_eq!(
            bank(&mut session, 0, "PLAYER_STATE_RESOURCE_GOLD"),
            Value::Int(if wake { 9 } else { 7 })
        );
        assert_eq!(
            bank(&mut session, 0, "PLAYER_STATE_RESOURCE_LUMBER"),
            Value::Int(if wake { 31 } else { 11 })
        );
        let fault = session.advance_timers(1., 128).unwrap_err();
        let TimerSessionError::AiBridge(fault) = fault else {
            panic!("{fault}")
        };
        assert!(fault.contains(&primary), "{fault}");
        let TimerSessionError::AiBridge(repeated) = session.advance_timers(1., 128).unwrap_err()
        else {
            panic!("retained failure")
        };
        assert!(Arc::ptr_eq(&fault, &repeated));
        assert_eq!(trigger_after_fault(&mut session), Value::Null);
    }
}

#[test]
#[ignore = "requires explicit PCX_WAR3_EXE selected executable"]
fn ai_map_bridge_capture_error_at_wake_returns_the_retained_fault_arc() {
    use peacecraft3::map_script_session::TimerSessionError;
    const INVALID_SETTER: &str = "function main takes nothing returns nothing\ncall Sleep(0.0317)\ncall SetPlayerState(Player(0),null,7)\ncall SetPlayerState(Player(1),PLAYER_STATE_RESOURCE_LUMBER,99)\nendfunction\n";
    for native in [false, true] {
        let (mut session, loads) = publish(native, INVALID_SETTER, "");
        start(&mut session).unwrap();
        let error = if native {
            session
                .advance_native_frame(
                    Some(pcx_play::native_game_clock::FrameInput {
                        game_elapsed: Duration::from_millis(125),
                        real_elapsed_ms: 125,
                    }),
                    128,
                )
                .unwrap_err()
        } else {
            session.advance_timers(0.125, 128).unwrap_err()
        };
        let TimerSessionError::AiBridge(fault) = error else {
            panic!("capture failure must retain its identity: {error}")
        };
        assert!(
            fault.contains("SetPlayerState") && fault.contains("observed.ai:"),
            "{fault}"
        );
        let world = session.host().unwrap().capture_world_snapshot();
        let handles = session.host().unwrap().objects().inner().handles();
        let clock = format!(
            "{:?}",
            session.host().unwrap().objects().test_native_clock_state()
        );
        for entry in 0..3 {
            let error = match entry {
                0 => session
                    .advance_native_frame(
                        Some(pcx_play::native_game_clock::FrameInput {
                            game_elapsed: Duration::from_secs(1),
                            real_elapsed_ms: 1000,
                        }),
                        128,
                    )
                    .unwrap_err(),
                1 => session.advance_native_frame(None, 128).unwrap_err(),
                _ => session.advance_timers(1., 128).unwrap_err(),
            };
            let TimerSessionError::AiBridge(repeated) = error else {
                panic!("retained capture failure: {error}")
            };
            assert!(Arc::ptr_eq(&fault, &repeated));
            assert_eq!(session.host().unwrap().capture_world_snapshot(), world);
            assert_eq!(session.host().unwrap().objects().inner().handles(), handles);
            assert_eq!(
                format!(
                    "{:?}",
                    session.host().unwrap().objects().test_native_clock_state()
                ),
                clock
            );
        }
        assert_eq!(session.global("calls"), Some(Value::Int(0)));
        assert_eq!(
            bank(&mut session, 1, "PLAYER_STATE_RESOURCE_LUMBER"),
            Value::Int(0)
        );
        assert_eq!(loads.load(Ordering::SeqCst), 1);
        assert_eq!(trigger_after_fault(&mut session), Value::Null);
    }
}

struct BorrowedMapHost<'a>(&'a mut MapStartupHost);
impl Host for BorrowedMapHost<'_> {
    fn native(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
        self.0.native(name, args)
    }
    fn begin_script_call(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Result<Option<pcx_script::ScriptCall>, String> {
        self.0.begin_script_call(name, args)
    }
    fn begin_initialized_script_call(
        &mut self,
        name: &str,
        args: &[Value],
        owner: pcx_script::eval::InitializedVmCall<'_>,
    ) -> Result<Option<pcx_script::ScriptCall>, String> {
        self.0.begin_initialized_script_call(name, args, owner)
    }
}

fn initialize_ai_from_nested_global(host: &mut MapStartupHost) -> Result<Value, pcx_script::Error> {
    let syntax = pcx_script::parse_script("type player extends handle\nnative Player takes integer p returns player\nnative StartCampaignAI takes player p, string path returns nothing\nglobals\ninteger started=initial()\nendglobals\nfunction launch takes nothing returns nothing\ncall StartCampaignAI(Player(0),\"observed.ai\")\nendfunction\nfunction initial takes nothing returns integer\ncall launch()\nreturn 1\nendfunction\n").unwrap();
    let program = pcx_script::resolve(&syntax).unwrap();
    let mut vm = pcx_script::Vm::new(&program, BorrowedMapHost(host));
    vm.init_globals()?;
    Ok(vm.global("started"))
}

#[test]
#[ignore = "requires explicit PCX_WAR3_EXE selected executable"]
fn ai_map_bridge_plain_host_and_nested_map_initializer_cannot_acquire_observer_owner() {
    for (native, watch) in [(false, false), (false, true), (true, false), (true, true)] {
        let (mut session, loads) = publish_with_observers(native, AI, "", watch);
        let player = session
            .host()
            .unwrap()
            .native("Player", &[Value::Int(0)])
            .unwrap();
        let direct = session.host().unwrap().begin_script_call(
            "StartCampaignAI",
            &[player, Value::Str("observed.ai".into())],
        );
        if watch {
            assert!(direct.unwrap_err().contains("owning map VM"));
        } else {
            assert!(direct.unwrap().is_none());
        }
        assert_eq!(loads.load(Ordering::SeqCst), 0);
        let result = initialize_ai_from_nested_global(session.host().unwrap());
        if watch {
            assert!(result.unwrap_err().to_string().contains("owning map VM"));
        } else {
            assert_eq!(result.unwrap(), Value::Int(1));
        }
        assert_eq!(loads.load(Ordering::SeqCst), usize::from(!watch));
        assert_eq!(
            bank(&mut session, 0, "PLAYER_STATE_RESOURCE_GOLD"),
            Value::Int(if watch { 0 } else { 7 })
        );
        assert_eq!(session.global("calls"), Some(Value::Int(0)));
    }
}

#[test]
#[ignore = "requires explicit PCX_WAR3_EXE selected executable"]
fn ai_map_bridge_later_registration_keeps_initialized_owner_and_refuses_unowned_wakes() {
    use peacecraft3::map_script_session::TimerSessionError;
    for native in [false, true] {
        for origin in 0..3 {
            let (mut session, loads) = publish_with_observers(
                native,
                AI,
                "call SetPlayerState(Player(0),PLAYER_STATE_RESOURCE_LUMBER,value+1)",
                false,
            );
            match origin {
                0 => {
                    start(&mut session).unwrap();
                }
                1 => {
                    let host = session.host().unwrap();
                    let player = host.native("Player", &[Value::Int(0)]).unwrap();
                    host.native(
                        "StartCampaignAI",
                        &[player, Value::Str("observed.ai".into())],
                    )
                    .unwrap();
                }
                _ => {
                    initialize_ai_from_nested_global(session.host().unwrap()).unwrap();
                }
            }
            assert_eq!(loads.load(Ordering::SeqCst), 1);
            assert_eq!(session.global("calls"), Some(Value::Int(0)));
            let watched = session.global("watched").unwrap();
            let state = session.global("PLAYER_STATE_RESOURCE_GOLD").unwrap();
            let host = session.host().unwrap();
            let player = host.native("Player", &[Value::Int(0)]).unwrap();
            let op = host.native("ConvertLimitOp", &[Value::Int(2)]).unwrap();
            host.native(
                "TriggerRegisterPlayerStateEvent",
                &[watched, player, state, op, Value::Real(9.)],
            )
            .unwrap();
            let result = if native {
                session.advance_native_frame(
                    Some(pcx_play::native_game_clock::FrameInput {
                        game_elapsed: Duration::from_millis(125),
                        real_elapsed_ms: 125,
                    }),
                    128,
                )
            } else {
                session.advance_timers(0.125, 128)
            };
            if origin == 0 {
                result.unwrap();
                assert_eq!(session.global("calls"), Some(Value::Int(1)));
                assert_eq!(session.global("firstSeen"), Some(Value::Int(31)));
                assert_eq!(
                    bank(&mut session, 1, "PLAYER_STATE_RESOURCE_LUMBER"),
                    Value::Int(32)
                );
            } else {
                let error = result.unwrap_err();
                assert!(matches!(error, TimerSessionError::Ai(_)), "{error}");
                assert!(
                    error.to_string().contains("initializer or direct host"),
                    "{error}"
                );
                assert_eq!(session.global("calls"), Some(Value::Int(0)));
                assert_eq!(
                    bank(&mut session, 0, "PLAYER_STATE_RESOURCE_GOLD"),
                    Value::Int(7)
                );
                assert_eq!(
                    bank(&mut session, 0, "PLAYER_STATE_RESOURCE_LUMBER"),
                    Value::Int(22)
                );
            }
        }
    }
}

#[test]
#[ignore = "requires explicit PCX_WAR3_EXE selected executable"]
fn ai_map_bridge_ordinary_map_thread_can_start_observers_after_resuming_a_park() {
    for native in [false, true] {
        let (mut session, loads) = publish(
            native,
            AI,
            "call SetPlayerState(Player(0),PLAYER_STATE_RESOURCE_LUMBER,value+1)",
        );
        let entry = session
            .context()
            .unwrap()
            .scripts
            .program()
            .function_id("startAfterPark")
            .unwrap();
        session.run_thread(&[entry]).unwrap();
        assert_eq!(loads.load(Ordering::SeqCst), 0);
        assert_eq!(session.host().unwrap().objects().parked_thread_count(), 1);
        if native {
            for _ in 0..4 {
                session
                    .advance_native_frame(
                        Some(pcx_play::native_game_clock::FrameInput {
                            game_elapsed: Duration::from_millis(125),
                            real_elapsed_ms: 125,
                        }),
                        128,
                    )
                    .unwrap();
            }
        } else {
            session.advance_timers(0.5, 128).unwrap();
        }
        assert_eq!(loads.load(Ordering::SeqCst), 1);
        assert_eq!(session.global("calls"), Some(Value::Int(2)));
        assert_eq!(session.global("firstSeen"), Some(Value::Int(11)));
        assert_eq!(session.global("secondSeen"), Some(Value::Int(31)));
        assert_eq!(
            bank(&mut session, 1, "PLAYER_STATE_RESOURCE_LUMBER"),
            Value::Int(32)
        );
        assert_eq!(session.host().unwrap().objects().parked_thread_count(), 0);
        assert_eq!(trigger_after_fault(&mut session), Value::Null);
    }
}

#[test]
#[ignore = "requires explicit PCX_WAR3_EXE selected executable"]
fn ai_map_bridge_foreign_initialized_vm_cannot_own_map_callback_ids() {
    for (native, watch) in [(false, false), (false, true), (true, false), (true, true)] {
        let (mut session, loads) = publish_with_observers(native, AI, "", watch);
        let program = session.context().unwrap().scripts.program().clone();
        assert!(!std::ptr::eq(
            &program,
            session.context().unwrap().scripts.program()
        ));
        let result = pcx_script::Vm::new(&program, BorrowedMapHost(session.host().unwrap()))
            .call("start", &[]);
        if watch {
            assert!(result.unwrap_err().to_string().contains("owning map VM"));
        } else {
            result.unwrap();
        }
        assert_eq!(loads.load(Ordering::SeqCst), usize::from(!watch));
        assert_eq!(session.global("calls"), Some(Value::Int(0)));
        assert_eq!(
            bank(&mut session, 0, "PLAYER_STATE_RESOURCE_GOLD"),
            Value::Int(if watch { 0 } else { 7 })
        );
    }
}

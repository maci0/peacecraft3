//! Selected arithmetic through the actual published factory and application tick caller.
use super::*;
use pcx_play::{native_game_clock as clock, script_numeric::NumericPolicy};
use peacecraft3::campaign_runtime::CampaignRuntime;
use std::time::Duration;

fn numeric(nonstock: bool) -> NumericPolicy {
    use std::io::Read;
    let path = std::env::var("PCX_WAR3_EXE").expect("explicit selected executable");
    let file = std::fs::File::open(&path).unwrap();
    assert!(file.metadata().unwrap().len() <= 32 << 20);
    let mut image = Vec::new();
    file.take((32 << 20) + 1).read_to_end(&mut image).unwrap();
    let word = |at| u32::from_le_bytes(image.get(at..at + 4).unwrap().try_into().unwrap());
    let half = |at| u16::from_le_bytes(image.get(at..at + 2).unwrap().try_into().unwrap());
    let nt = word(0x3c) as usize;
    assert_eq!(&image[..2], b"MZ");
    assert_eq!(image.get(nt..nt + 4).unwrap(), b"PE\0\0");
    let count = half(nt + 6) as usize;
    assert!(count <= 96);
    let headers = nt + 24 + half(nt + 20) as usize;
    let base = word(nt + 52);
    let text = |va: u32| {
        let rva = va.checked_sub(base).unwrap();
        let sections: Vec<_> = (0..count)
            .filter_map(|i| {
                let h = headers + i * 40;
                let start = word(h + 12);
                let len = word(h + 16);
                (rva >= start && rva - start < len)
                    .then(|| (word(h + 20) + rva - start, len - (rva - start)))
            })
            .collect();
        let [(at, len)] = sections.as_slice() else {
            panic!("one mapped source string")
        };
        let bytes = image
            .get(*at as usize..*at as usize + (*len as usize).min(256))
            .unwrap();
        let end = bytes.iter().position(|b| *b == 0).unwrap();
        std::str::from_utf8(&bytes[..end]).unwrap().to_owned()
    };
    let profile = format!(
        "[{}]\n{}=17\n{}=19\n{}=23\n{}=29\n",
        text(0xe6eb40),
        text(0xe6eb38),
        text(0xe6eb50),
        text(0xe6eb58),
        text(0xe6eb5c)
    );
    let origin = pcx_play::scripts::AssetIdentity {
        source: path,
        edition: "Legacy".into(),
        version: "1.29.2.9231".into(),
    };
    let numeric = pcx_play::campaign_client::read(
        &image,
        origin.clone(),
        profile.as_bytes(),
        pcx_play::scripts::AssetIdentity {
            source: "synthetic camera diagnostic".into(),
            ..origin
        },
        pcx_objects::editor::hero::Progression {
            max_level: 17,
            ability_level_skip: 2,
            sources: vec!["synthetic progression diagnostic".into()],
        },
    )
    .unwrap()
    .numeric;
    if nonstock {
        numeric.with_native_game_clock_fixture(clock::Rules {
            simple: pcx_play::autocast_rules::clock::Rules {
                origin: 0.,
                cycle: 16.,
                minimum: 0.03125,
                epsilon: 0.0625,
            },
            numerator: 1,
            denominator: 8,
            step: 0.125f32.to_bits(),
            wrap_tolerance: 0.00001f32.to_bits(),
            frame: clock::frame::Rules {
                unsigned_correction: [0f64.to_bits(), 4294967296f64.to_bits()],
                scale: 0.001f32.to_bits(),
                chunk: 8f32.to_bits(),
            },
            sleep: clock::sleep::Rules {
                script: clock::Domain::Real,
                ai: clock::Domain::Game,
            },
        })
    } else {
        numeric
    }
}

fn publish(app: &mut App, script: &str, revision: u64, nonstock: bool) {
    deliver_timer_with(app, script, revision, "", |inputs| {
        inputs.timer_time = Some(0.);
        inputs.numeric = Some(numeric(nonstock));
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
    assert_eq!(
        session.host().unwrap().objects().native_game_point(),
        Some(clock::Point { cycle: 0, local: 0 })
    );
}

const LONG: &str = "\
globals
timer t=CreateTimer()
integer fired=0
real held=-1.0
endglobals
function done takes nothing returns nothing
set fired=fired+1
set held=TimerGetRemaining(t)
endfunction
function config takes nothing returns nothing
endfunction
function main takes nothing returns nothing
call TimerStart(t,40.0,false,function done)
endfunction
";

#[test]
#[ignore = "requires explicit PCX_WAR3_EXE selected executable"]
fn native_app_ticks_cross_two_nonstock_wraps_and_deliver_one_extended_expiry() {
    let run = |step: Duration, ticks: usize| {
        let mut app = timer_app();
        publish(&mut app, LONG, 1, true);
        let mut runtime = CampaignRuntime::new(step, 2, 8);
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        assert_eq!(runtime.advance_ticks(&mut session, 0).unwrap(), 0);
        for _ in 0..ticks {
            assert_eq!(runtime.advance_ticks(&mut session, 1).unwrap(), 1);
        }
        let point = session
            .host()
            .unwrap()
            .objects()
            .native_game_point()
            .unwrap();
        (
            point,
            session.global("fired"),
            session.global("held"),
            session.host().unwrap().objects().timer_game_time(),
        )
    };
    let expected = run(Duration::from_millis(125), 324);
    assert_eq!(
        expected.0,
        clock::Point {
            cycle: 2,
            local: 8.5f32.to_bits()
        }
    );
    assert_eq!(expected.1, Some(Value::Int(1)));
    assert_eq!(expected.2, Some(Value::Real(0.)));
    assert_eq!(expected.3, Some(40.5));
    assert_eq!(run(Duration::from_millis(37), 1095), expected);
}

#[test]
#[ignore = "requires explicit PCX_WAR3_EXE selected executable"]
fn native_app_driver_domains_refuse_mixing_and_session_replacement_resets_clock() {
    let mut app = timer_app();
    publish(&mut app, LONG, 1, true);
    {
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        assert_eq!(
            session.advance_timers(0., 8).unwrap(),
            0,
            "zero noop leaves fresh domain"
        );
        assert_eq!(session.advance_timers(0.1, 8).unwrap(), 0);
        let before = session.host().unwrap().objects().timer_game_time();
        assert!(
            pcx_play::campaign_host::advance_paced_tick(
                &mut session,
                Duration::from_millis(125),
                0.125,
                8
            )
            .is_err()
        );
        assert_eq!(session.host().unwrap().objects().timer_game_time(), before);
        assert_eq!(
            session.advance_timers(40., 8).unwrap(),
            1,
            "legacy startup request preserved"
        );
        assert_eq!(session.global("fired"), Some(Value::Int(1)));
    }
    publish(&mut app, LONG, 2, true);
    let mut runtime = CampaignRuntime::new(Duration::from_millis(125), 2, 8);
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    runtime.advance_ticks(&mut session, 0).unwrap();
    runtime.advance_ticks(&mut session, 1).unwrap();
    let before = session.host().unwrap().objects().native_game_point();
    assert!(session.advance_timers(0.5, 8).is_err());
    assert_eq!(
        session.host().unwrap().objects().native_game_point(),
        before
    );
    assert_eq!(
        session.host().unwrap().objects().timer_game_time(),
        Some(0.125)
    );
    assert_eq!(session.global("fired"), Some(Value::Int(0)));
}

#[test]
#[ignore = "requires explicit PCX_WAR3_EXE selected executable"]
fn native_selected_app_ticks_use_repeated_source_add_across_actual_cycle() {
    let mut app = timer_app();
    let script = LONG.replace("40.0", "2000.0");
    let script = format!(
        "{script}\nfunction sample takes nothing returns nothing\nset held=TimerGetRemaining(t)\nendfunction\n"
    );
    publish(&mut app, &script, 1, false);
    let mut runtime = CampaignRuntime::new(Duration::from_millis(125), 2, 8);
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    runtime.advance_ticks(&mut session, 0).unwrap();
    for _ in 0..2405 {
        runtime.advance_ticks(&mut session, 1).unwrap();
    }
    let point = session
        .host()
        .unwrap()
        .objects()
        .native_game_point()
        .unwrap();
    assert_eq!(point.cycle, 1);
    assert!(f32::from_bits(point.local) > 0. && f32::from_bits(point.local) < 1.);
    assert_eq!(session.global("fired"), Some(Value::Int(0)));
    let function = session
        .context()
        .unwrap()
        .scripts
        .program()
        .function_id("sample")
        .unwrap();
    session.call_function(function, &[]).unwrap();
    let Some(Value::Real(remaining)) = session.global("held") else {
        panic!("remaining")
    };
    assert!(remaining > 1699. && remaining < 1701., "{remaining}");
}

#[test]
#[ignore = "requires explicit PCX_WAR3_EXE selected executable"]
fn native_app_pause_preserves_pending_outer_step_and_pacing_credit() {
    let script = LONG.replace("40.0", "0.2").replace(
        "set fired=fired+1",
        "set fired=fired+1\ncall PauseGame(true)",
    );
    let script = format!(
        "{script}\nfunction unpause takes nothing returns nothing\ncall PauseGame(false)\nendfunction\n"
    );
    let mut app = timer_app();
    publish(&mut app, &script, 1, true);
    let mut runtime = CampaignRuntime::new(Duration::from_secs(1), 2, 8);
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    runtime.advance_ticks(&mut session, 0).unwrap();
    assert_eq!(runtime.advance_ticks(&mut session, 1).unwrap(), 1);
    assert_eq!(session.global("fired"), Some(Value::Int(1)));
    assert_eq!(runtime.advance_ticks(&mut session, 1).unwrap(), 0);
    let function = session
        .context()
        .unwrap()
        .scripts
        .program()
        .function_id("unpause")
        .unwrap();
    session.call_function(function, &[]).unwrap();
    assert_eq!(runtime.advance_ticks(&mut session, 1).unwrap(), 1);
    assert_eq!(
        session.host().unwrap().objects().native_game_point(),
        Some(clock::Point {
            cycle: 0,
            local: 2f32.to_bits()
        })
    );
    assert_eq!(
        session.host().unwrap().objects().timer_game_time(),
        Some(2.)
    );
    assert_eq!(session.global("fired"), Some(Value::Int(1)));
}

#[test]
#[ignore = "requires explicit PCX_WAR3_EXE selected executable"]
fn native_published_budget_refusal_retains_request_and_retries_without_extra_time() {
    let script = LONG.replace("40.0", "0.2");
    let mut app = timer_app();
    publish(&mut app, &script, 1, true);
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    assert!(
        pcx_play::campaign_host::advance_paced_tick(&mut session, Duration::from_secs(1), 1., 0)
            .is_err()
    );
    assert_eq!(session.global("fired"), Some(Value::Int(0)));
    assert_eq!(
        pcx_play::campaign_host::advance_paced_tick(&mut session, Duration::ZERO, 1., 8).unwrap(),
        1
    );
    assert_eq!(session.global("fired"), Some(Value::Int(1)));
    assert_eq!(
        session.host().unwrap().objects().native_game_point(),
        Some(clock::Point {
            cycle: 0,
            local: 1f32.to_bits()
        })
    );
}

#[test]
#[ignore = "requires explicit PCX_WAR3_EXE selected executable"]
fn native_timed_event_uses_extended_minimum_and_exact_callback_cursor() {
    let script = LONG.replace(
        "call TimerStart(t,40.0,false,function done)",
        "call TriggerAddAction(g,function event)\ncall TriggerRegisterTimerEvent(g,0.0,false)",
    );
    let script = script.replace(
        "timer t=CreateTimer()",
        "timer t=CreateTimer()\ntrigger g=CreateTrigger()",
    );
    let script=script.replace("function config takes nothing returns nothing","function event takes nothing returns nothing\ncall TimerStart(t,0.25,false,function done)\nendfunction\nfunction config takes nothing returns nothing");
    let script = format!(
        "{script}\nfunction sample takes nothing returns nothing\nset held=TimerGetRemaining(t)\nendfunction\n"
    );
    let publish = |app: &mut App, revision| {
        deliver_timer_with(
            app,
            &script,
            revision,
            super::super::timed_trigger_cases::COMMON,
            |inputs| {
                inputs.timer_time = Some(0.);
                inputs.numeric = Some(numeric(true));
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
            },
        );
    };
    let mut app = timer_app();
    publish(&mut app, 1);
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_none(),
        "{:?}",
        app.world().resource::<CampaignStartup>().last_error
    );
    let mut runtime = CampaignRuntime::new(Duration::from_millis(125), 2, 8);
    {
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        runtime.advance_ticks(&mut session, 0).unwrap();
        runtime.advance_ticks(&mut session, 1).unwrap();
        let function = session
            .context()
            .unwrap()
            .scripts
            .program()
            .function_id("sample")
            .unwrap();
        session.call_function(function, &[]).unwrap();
        let Some(Value::Real(remaining)) = session.global("held") else {
            panic!("remaining")
        };
        assert!((0.125..0.126).contains(&remaining), "{remaining}");
        runtime.advance_ticks(&mut session, 1).unwrap();
        assert_eq!(session.global("fired"), Some(Value::Int(0)));
        runtime.advance_ticks(&mut session, 1).unwrap();
        assert_eq!(session.global("fired"), Some(Value::Int(1)));
    }
    publish(&mut app, 2);
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    assert_eq!(
        session.advance_timers(0., 8).unwrap(),
        1,
        "same-time due work chooses Legacy"
    );
    assert!(
        pcx_play::campaign_host::advance_paced_tick(&mut session, Duration::ZERO, 0., 8).is_err()
    );
    assert_eq!(session.advance_timers(0.25, 8).unwrap(), 1);
    assert_eq!(session.global("fired"), Some(Value::Int(1)));
}

#[test]
#[ignore = "requires explicit PCX_WAR3_EXE selected executable"]
fn native_callback_error_restores_outer_cursor_and_runtime_does_not_replay() {
    let script = LONG.replace("40.0", "0.2").replace(
        "set fired=fired+1",
        "set fired=fired+1\ncall SetSoundPitch(null,1.0)",
    );
    let mut app = timer_app();
    publish(&mut app, &script, 1, true);
    let mut runtime = CampaignRuntime::new(Duration::from_secs(1), 2, 8);
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    runtime.advance_ticks(&mut session, 0).unwrap();
    assert!(runtime.advance_ticks(&mut session, 1).is_err());
    assert_eq!(runtime.tick(), 0);
    assert_eq!(session.global("fired"), Some(Value::Int(1)));
    assert_eq!(session.global("held"), Some(Value::Real(-1.)));
    assert_eq!(
        session.host().unwrap().objects().native_game_point(),
        Some(clock::Point {
            cycle: 0,
            local: 0.25f32.to_bits()
        })
    );
    assert!(runtime.advance_ticks(&mut session, 1).is_err());
    assert_eq!(session.global("fired"), Some(Value::Int(1)));
}

#[test]
#[ignore = "requires explicit PCX_WAR3_EXE selected executable"]
fn explicit_initial_caller_clocks_preserve_legacy_factory_behavior() {
    for initial in [20., 0.1, -1., -0.] {
        let mut app = timer_app();
        deliver_timer_with(&mut app, LONG, 1, "", |inputs| {
            inputs.timer_time = Some(initial);
            inputs.numeric = Some(numeric(false));
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
        assert_eq!(
            session
                .host()
                .unwrap()
                .objects()
                .timer_game_time()
                .unwrap()
                .to_bits(),
            initial.to_bits()
        );
        assert_eq!(session.host().unwrap().objects().native_game_point(), None);
        assert!(
            pcx_play::campaign_host::advance_paced_tick(
                &mut session,
                Duration::from_millis(125),
                initial + 0.125,
                8
            )
            .is_err()
        );
        assert_eq!(
            session
                .host()
                .unwrap()
                .objects()
                .timer_game_time()
                .unwrap()
                .to_bits(),
            initial.to_bits()
        );
        assert_eq!(session.host().unwrap().objects().native_game_point(), None);
        assert_eq!(session.global("fired"), Some(Value::Int(0)));
        assert_eq!(session.advance_timers(initial + 40., 8).unwrap(), 1);
        assert_eq!(session.global("fired"), Some(Value::Int(1)));
    }
    let mut app = timer_app();
    publish(&mut app, LONG, 1, false);
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    session.advance_timers(-0., 8).unwrap();
    assert_eq!(
        session
            .host()
            .unwrap()
            .objects()
            .timer_game_time()
            .unwrap()
            .to_bits(),
        (-0f64).to_bits()
    );
    assert!(
        pcx_play::campaign_host::advance_paced_tick(&mut session, Duration::ZERO, 0., 8).is_err()
    );
    assert_eq!(session.advance_timers(40., 8).unwrap(), 1);
}

#[test]
#[ignore = "requires explicit PCX_WAR3_EXE selected executable"]
fn native_unbound_map_wake_preserves_ticket_and_prior_timer_progress() {
    let common = format!(
        "{}\nnative TriggerSleepAction takes real seconds returns nothing\n",
        super::super::timed_trigger_cases::COMMON
    );
    let script = "\
globals
trigger sleeper=CreateTrigger()
timer t=CreateTimer()
integer trace=0
endglobals
function afterSleep takes nothing returns nothing
set trace=trace*10+4
endfunction
function beforeSleep takes nothing returns nothing
set trace=trace*10+2
endfunction
function sleepAction takes nothing returns nothing
set trace=trace*10+1
call TriggerSleepAction(0.125)
set trace=trace*10+3
call TimerStart(t,0.1,false,function afterSleep)
endfunction
function config takes nothing returns nothing
endfunction
function main takes nothing returns nothing
call TriggerAddAction(sleeper,function sleepAction)
call TriggerRegisterTimerEvent(sleeper,0.0625,false)
call TimerStart(t,0.125,false,function beforeSleep)
endfunction
";
    for native in [true, false] {
        let mut app = timer_app();
        deliver_timer_with(&mut app, script, 1, &common, |inputs| {
            inputs.timer_time = Some(0.);
            inputs.numeric = Some(numeric(true));
            inputs.enumeration.names.push("ConvertGameEvent".into());
            inputs.enumeration.limits.max_handles = 256;
            inputs.trigger_runtime = Some(pcx_play::playercolor_host::TriggerRuntimeInput {
                initial_enabled: true,
                timer_event_global: "TEST_TIMER_EVENT".into(),
                max_timed_events: 8,
                max_parked_threads: 8,
            });
        });
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        if native {
            for elapsed in [Duration::from_millis(250), Duration::ZERO] {
                let error =
                    pcx_play::campaign_host::advance_paced_tick(&mut session, elapsed, 0.25, 8)
                        .unwrap_err();
                assert!(
                    error.to_string().contains("native real-queue entry"),
                    "{error}"
                );
                assert_eq!(session.global("trace"), Some(Value::Int(12)));
                let objects = session.host().unwrap().objects();
                assert_eq!(objects.parked_thread_count(), 1);
                assert_eq!(objects.timer_game_time(), Some(0.125));
                assert_eq!(
                    objects.native_game_point(),
                    Some(clock::Point {
                        cycle: 0,
                        local: 0.25f32.to_bits()
                    })
                );
            }
        } else {
            assert_eq!(session.advance_timers(0.5, 8).unwrap(), 4);
            assert_eq!(session.global("trace"), Some(Value::Int(1234)));
            assert_eq!(session.host().unwrap().objects().parked_thread_count(), 0);
        }
    }
}

#[test]
#[ignore = "requires explicit PCX_WAR3_EXE selected executable"]
fn native_unbound_ai_wake_preserves_ticket_and_prior_timer_progress() {
    use pcx_play::map_startup_host::MapStartupError;
    use pcx_script::Host;

    let mut app = timer_app();
    let common = "\
native StartCampaignAI takes player p, string path returns nothing
constant native GetPlayerState takes player p, playerstate state returns integer
globals
constant playerstate PLAYER_STATE_RESOURCE_GOLD=ConvertPlayerState(1)
constant playerstate PLAYER_STATE_RESOURCE_LUMBER=ConvertPlayerState(2)
constant playerstate PLAYER_STATE_RESOURCE_FOOD_USED=ConvertPlayerState(5)
constant playerstate PLAYER_STATE_RESOURCE_FOOD_CAP=ConvertPlayerState(4)
constant playerstate PLAYER_STATE_FOOD_CAP_CEILING=ConvertPlayerState(6)
constant playerstate PLAYER_STATE_GOLD_GATHERED=ConvertPlayerState(15)
constant playerstate PLAYER_STATE_LUMBER_GATHERED=ConvertPlayerState(16)
endglobals
";
    let script = "\
globals
timer t=CreateTimer()
integer seen=0
endglobals
function tick takes nothing returns nothing
set seen=seen+1
endfunction
function config takes nothing returns nothing
call SetPlayers(1)
endfunction
function main takes nothing returns nothing
call TimerStart(t,0.125,false,function tick)
endfunction
function start takes nothing returns nothing
call StartCampaignAI(Player(0),\"idle.ai\")
endfunction
";
    deliver_timer_with(&mut app, script, 1, common, |inputs| {
        inputs.timer_time = Some(0.);
        inputs.numeric = Some(numeric(true));
    });
    let catalog = Arc::clone(
        &app.world()
            .resource::<MapRequests>()
            .bindings
            .as_ref()
            .unwrap()
            .catalog
            .0,
    );
    let mut inputs = host_inputs(catalog);
    inputs.timer_time = Some(0.);
    inputs.numeric = Some(numeric(true));
    inputs.resources = Some(pcx_play::player_resources::Rules::synthetic_fixture());
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let context = Arc::clone(session.context().unwrap());
    let common = context
        .scripts
        .source(&context.scripts.digests[0].label)
        .unwrap();
    let ai = Arc::new(
        pcx_play::scripts::MapScripts::compile(
            AssetIdentity {
                edition: "synthetic".into(),
                version: "native-wake-guard".into(),
                source: "idle.ai".into(),
            },
            ScriptAsset { label: "fixture:common.j", bytes: common.as_bytes() },
            ScriptAsset { label: "fixture:common.ai", bytes: super::super::AI_COMMON.as_bytes() },
            ScriptAsset {
                label: "idle.ai",
                bytes: b"function main takes nothing returns nothing\ncall SetPlayerState(Player(GetAiPlayer()),ConvertPlayerState(1),17)\ncall Sleep(0.25)\ncall SetPlayerState(Player(GetAiPlayer()),ConvertPlayerState(1),34)\nendfunction\n",
            },
            None,
        )
        .unwrap(),
    );
    let entries = ["config", "main", "start"]
        .map(|name| context.scripts.program().function_id(name).unwrap());
    session
        .replace(
            context,
            move |ctx| {
                Ok::<_, MapStartupError>(
                    pcx_play::map_startup_host::assemble(ctx, inputs)?
                        .with_ai_loader(move |_| Ok(Arc::clone(&ai))),
                )
            },
            &entries,
        )
        .unwrap();
    let gold = session
        .host()
        .unwrap()
        .native("ConvertPlayerState", &[Value::Int(1)])
        .unwrap();
    let player = session
        .host()
        .unwrap()
        .native("Player", &[Value::Int(0)])
        .unwrap();
    for elapsed in [Duration::from_millis(500), Duration::ZERO] {
        let error =
            pcx_play::campaign_host::advance_paced_tick(&mut session, elapsed, 0.5, 8).unwrap_err();
        assert!(
            error.to_string().contains("native game-queue entry"),
            "{error}"
        );
        assert_eq!(session.global("seen"), Some(Value::Int(1)));
        let host = session.host().unwrap();
        assert_eq!(
            host.native("GetPlayerState", &[player.clone(), gold.clone()])
                .unwrap(),
            Value::Int(17)
        );
        assert_eq!(host.objects().timer_game_time(), Some(0.125));
        assert_eq!(
            host.objects().native_game_point(),
            Some(clock::Point {
                cycle: 0,
                local: 0.25f32.to_bits()
            })
        );
    }
}
fn publish_map_sleeper(app: &mut App, script: &str) {
    let common = format!(
        "{}\nnative TriggerSleepAction takes real seconds returns nothing\n",
        super::super::timed_trigger_cases::COMMON
    );
    deliver_timer_with(app, script, 1, &common, |inputs| {
        inputs.timer_time = Some(0.);
        inputs.numeric = Some(numeric(true));
        inputs.enumeration.names.push("ConvertGameEvent".into());
        inputs.enumeration.limits.max_handles = 256;
        inputs.trigger_runtime = Some(pcx_play::playercolor_host::TriggerRuntimeInput {
            initial_enabled: true,
            timer_event_global: "TEST_TIMER_EVENT".into(),
            max_timed_events: 16,
            max_parked_threads: 16,
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
}

const PAIRED_SLEEP: &str = "\
globals
timer sleeper=CreateTimer()
trigger nested=CreateTrigger()
timer observer=CreateTimer()
timer after=CreateTimer()
integer trace=0
integer retained=0
boolean context=true
real remaining=-1.0
endglobals
function afterWake takes nothing returns nothing
set trace=trace*10+4
endfunction
function observe takes nothing returns nothing
set trace=trace*10+2
endfunction
function sleepAction takes nothing returns nothing
local integer saved=73
set trace=trace*10+1
call TriggerSleepAction(0.125)
set retained=saved
set context=context and GetExpiredTimer()==sleeper
set trace=trace*10+3
call TimerStart(after,0.25,false,function afterWake)
set remaining=TimerGetRemaining(after)
endfunction
function beginSleep takes nothing returns nothing
call TriggerExecute(nested)
endfunction
function config takes nothing returns nothing
endfunction
function main takes nothing returns nothing
call TriggerAddAction(nested,function sleepAction)
call TimerStart(sleeper,0.0625,false,function beginSleep)
call TimerStart(observer,0.125,false,function observe)
endfunction
";

#[test]
#[ignore = "requires explicit PCX_WAR3_EXE selected executable"]
fn paired_real_map_wake_starts_timer_at_completed_game_cursor() {
    let mut app = timer_app();
    publish_map_sleeper(&mut app, PAIRED_SLEEP);
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    session
        .advance_native_frame(
            Some(clock::FrameInput {
                game_elapsed: Duration::from_millis(500),
                real_elapsed_ms: 250,
            }),
            16,
        )
        .unwrap();
    assert_eq!(session.global("trace"), Some(Value::Int(123)));
    assert_eq!(session.global("retained"), Some(Value::Int(73)));
    assert_eq!(session.global("context"), Some(Value::Bool(true)));
    assert_eq!(session.global("remaining"), Some(Value::Real(0.25)));
    assert_eq!(session.host().unwrap().objects().parked_thread_count(), 0);
    assert_eq!(
        session
            .host()
            .unwrap()
            .objects()
            .native_queue_point(clock::Domain::Game),
        Some(clock::Point {
            cycle: 0,
            local: 0.5f32.to_bits()
        })
    );
    assert_eq!(
        session
            .host()
            .unwrap()
            .objects()
            .native_queue_point(clock::Domain::Real),
        Some(clock::Point {
            cycle: 0,
            local: 0.25f32.to_bits()
        })
    );
    for (elapsed, expected) in [(125, 123), (125, 1234)] {
        session
            .advance_native_frame(
                Some(clock::FrameInput {
                    game_elapsed: Duration::from_millis(elapsed),
                    real_elapsed_ms: 0,
                }),
                16,
            )
            .unwrap();
        assert_eq!(session.global("trace"), Some(Value::Int(expected)));
    }
}

#[test]
#[ignore = "requires explicit PCX_WAR3_EXE selected executable"]
fn paired_real_map_wake_budget_retry_preserves_frame_and_ticket() {
    let mut app = timer_app();
    publish_map_sleeper(&mut app, PAIRED_SLEEP);
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let error = session
        .advance_native_frame(
            Some(clock::FrameInput {
                game_elapsed: Duration::from_millis(500),
                real_elapsed_ms: 250,
            }),
            2,
        )
        .unwrap_err();
    assert!(error.to_string().contains("wake budget"), "{error}");
    assert_eq!(session.global("trace"), Some(Value::Int(12)));
    assert_eq!(session.host().unwrap().objects().parked_thread_count(), 1);
    // The diagnostic view reports the retained outer horizon, not completed real time.
    assert_eq!(
        session
            .host()
            .unwrap()
            .objects()
            .native_queue_point(clock::Domain::Real),
        Some(clock::Point {
            cycle: 0,
            local: 0.25f32.to_bits()
        })
    );
    session.advance_native_frame(None, 16).unwrap();
    assert_eq!(session.global("trace"), Some(Value::Int(123)));
    assert_eq!(session.global("retained"), Some(Value::Int(73)));
    assert_eq!(session.global("context"), Some(Value::Bool(true)));
    assert_eq!(session.host().unwrap().objects().parked_thread_count(), 0);
    assert_eq!(
        session
            .host()
            .unwrap()
            .objects()
            .native_queue_point(clock::Domain::Game),
        Some(clock::Point {
            cycle: 0,
            local: 0.5f32.to_bits()
        })
    );
    assert_eq!(
        session
            .host()
            .unwrap()
            .objects()
            .native_queue_point(clock::Domain::Real),
        Some(clock::Point {
            cycle: 0,
            local: 0.25f32.to_bits()
        })
    );
    session.advance_native_frame(None, 16).unwrap();
    assert_eq!(
        session.global("trace"),
        Some(Value::Int(123)),
        "drained frame never repeats wake"
    );
}

#[test]
#[ignore = "requires explicit PCX_WAR3_EXE selected executable"]
fn paired_real_repark_keeps_selected_origin_across_nonstock_wrap() {
    let script = PAIRED_SLEEP.replace(
        "call TriggerSleepAction(0.125)",
        "call TriggerSleepAction(17.0)\nset saved=saved+8\ncall TriggerSleepAction(0.25)",
    );
    let mut app = timer_app();
    publish_map_sleeper(&mut app, &script);
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    session
        .advance_native_frame(
            Some(clock::FrameInput {
                game_elapsed: Duration::from_millis(500),
                real_elapsed_ms: 17000,
            }),
            32,
        )
        .unwrap();
    assert_eq!(session.global("trace"), Some(Value::Int(12)));
    assert_eq!(session.host().unwrap().objects().parked_thread_count(), 1);
    assert_eq!(
        session
            .host()
            .unwrap()
            .objects()
            .native_queue_point(clock::Domain::Real),
        Some(clock::Point {
            cycle: 1,
            local: 1f32.to_bits()
        })
    );
    session
        .advance_native_frame(
            Some(clock::FrameInput {
                game_elapsed: Duration::ZERO,
                real_elapsed_ms: 250,
            }),
            32,
        )
        .unwrap();
    assert_eq!(session.global("trace"), Some(Value::Int(123)));
    assert_eq!(session.global("retained"), Some(Value::Int(81)));
    assert_eq!(session.global("context"), Some(Value::Bool(true)));
    assert_eq!(session.global("remaining"), Some(Value::Real(0.25)));
    assert_eq!(
        session
            .host()
            .unwrap()
            .objects()
            .native_queue_point(clock::Domain::Game),
        Some(clock::Point {
            cycle: 0,
            local: 0.5f32.to_bits()
        })
    );
    assert_eq!(
        session
            .host()
            .unwrap()
            .objects()
            .native_queue_point(clock::Domain::Real),
        Some(clock::Point {
            cycle: 1,
            local: 1.25f32.to_bits()
        })
    );
}

#[test]
#[ignore = "requires explicit PCX_WAR3_EXE selected executable"]
fn paired_real_sleep_consumes_extended_internal_entries_before_resuming() {
    let script = PAIRED_SLEEP
        .replace("TriggerSleepAction(0.125)", "TriggerSleepAction(240.0317)")
        .replace(
            "set trace=trace*10+3",
            "set trace=trace*10+3\ncall TriggerSleepAction(0.0625)\nset trace=trace*10+5",
        );
    let mut app = timer_app();
    publish_map_sleeper(&mut app, &script);
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    for (game_ms, real_ms, trace, parked) in [
        (500, 240000, 12, 1),
        (0, 64, 123, 1),
        (0, 64, 1235, 0),
        (250, 0, 12354, 0),
    ] {
        session
            .advance_native_frame(
                Some(clock::FrameInput {
                    game_elapsed: Duration::from_millis(game_ms),
                    real_elapsed_ms: real_ms,
                }),
                16,
            )
            .unwrap();
        assert_eq!(session.global("trace"), Some(Value::Int(trace)));
        assert_eq!(
            session.host().unwrap().objects().parked_thread_count(),
            parked
        );
    }
    assert_eq!(session.global("retained"), Some(Value::Int(73)));
    assert_eq!(session.global("context"), Some(Value::Bool(true)));
}

#[test]
#[ignore = "requires explicit PCX_WAR3_EXE selected executable"]
fn paired_real_sleep_unpauses_without_advancing_the_game_cursor() {
    let script = "\
globals
trigger action=CreateTrigger()
timer starter=CreateTimer()
timer after=CreateTimer()
integer trace=0
real remaining=-1.0
endglobals
function done takes nothing returns nothing
set trace=3
endfunction
function wait takes nothing returns nothing
set trace=1
call PauseGame(true)
call TriggerSleepAction(0.125)
set trace=2
call PauseGame(false)
call TimerStart(after,0.0625,false,function done)
set remaining=TimerGetRemaining(after)
endfunction
function begin takes nothing returns nothing
call TriggerExecute(action)
endfunction
function config takes nothing returns nothing
endfunction
function main takes nothing returns nothing
call TriggerAddAction(action,function wait)
call TimerStart(starter,0.0625,false,function begin)
endfunction
";
    let mut app = timer_app();
    publish_map_sleeper(&mut app, script);
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    session
        .advance_native_frame(
            Some(clock::FrameInput {
                game_elapsed: Duration::from_millis(125),
                real_elapsed_ms: 0,
            }),
            16,
        )
        .unwrap();
    assert_eq!(session.global("trace"), Some(Value::Int(1)));
    assert_eq!(session.host().unwrap().objects().game_paused(), Some(true));
    session
        .advance_native_frame(
            Some(clock::FrameInput {
                game_elapsed: Duration::ZERO,
                real_elapsed_ms: 250,
            }),
            16,
        )
        .unwrap();
    assert_eq!(session.global("trace"), Some(Value::Int(2)));
    assert_eq!(session.global("remaining"), Some(Value::Real(0.0625)));
    assert_eq!(session.host().unwrap().objects().game_paused(), Some(false));
    assert_eq!(
        session.host().unwrap().objects().native_game_point(),
        Some(clock::Point {
            cycle: 0,
            local: 0.125f32.to_bits()
        })
    );
    session
        .advance_native_frame(
            Some(clock::FrameInput {
                game_elapsed: Duration::from_millis(125),
                real_elapsed_ms: 0,
            }),
            16,
        )
        .unwrap();
    assert_eq!(session.global("trace"), Some(Value::Int(3)));
}

#[test]
#[ignore = "requires explicit PCX_WAR3_EXE selected executable"]
fn paired_fresh_sleep_admission_failure_preserves_legacy_choice_and_tickets() {
    let script = "\
globals
trigger action=CreateTrigger()
timer t=CreateTimer()
integer trace=0
endglobals
function done takes nothing returns nothing
set trace=trace*10+3
endfunction
function wait takes nothing returns nothing
set trace=1
call TriggerSleepAction(-241.0)
set trace=2
endfunction
function config takes nothing returns nothing
endfunction
function main takes nothing returns nothing
call TriggerAddAction(action,function wait)
call TriggerExecute(action)
call TimerStart(t,0.125,false,function done)
endfunction
";
    let mut app = timer_app();
    publish_map_sleeper(&mut app, script);
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    assert_eq!(session.global("trace"), Some(Value::Int(1)));
    assert_eq!(session.host().unwrap().objects().parked_thread_count(), 1);
    let error = session
        .advance_native_frame(
            Some(clock::FrameInput {
                game_elapsed: Duration::from_millis(125),
                real_elapsed_ms: 1,
            }),
            16,
        )
        .unwrap_err();
    assert!(
        error.to_string().contains("negative segmented sleep"),
        "{error}"
    );
    let objects = session.host().unwrap().objects();
    assert_eq!(objects.timer_game_time(), Some(0.));
    assert_eq!(
        objects.native_game_point(),
        Some(clock::Point { cycle: 0, local: 0 })
    );
    assert_eq!(
        objects.native_queue_point(clock::Domain::Real),
        Some(clock::Point { cycle: 0, local: 0 })
    );
    assert_eq!(objects.parked_thread_count(), 1);
    session.advance_timers(0.25, 16).unwrap();
    assert_eq!(session.global("trace"), Some(Value::Int(23)));
    assert_eq!(session.host().unwrap().objects().parked_thread_count(), 0);
}

fn publish_ai_sleeper(app: &mut App, script: &str, ai_source: &str) {
    use pcx_play::map_startup_host::MapStartupError;
    let common = "\
native StartCampaignAI takes player p, string path returns nothing
native PauseCompAI takes player p, boolean paused returns nothing
constant native GetPlayerState takes player p, playerstate state returns integer
globals
constant playerstate PLAYER_STATE_RESOURCE_GOLD=ConvertPlayerState(1)
constant playerstate PLAYER_STATE_RESOURCE_LUMBER=ConvertPlayerState(2)
constant playerstate PLAYER_STATE_RESOURCE_FOOD_USED=ConvertPlayerState(5)
constant playerstate PLAYER_STATE_RESOURCE_FOOD_CAP=ConvertPlayerState(4)
constant playerstate PLAYER_STATE_FOOD_CAP_CEILING=ConvertPlayerState(6)
constant playerstate PLAYER_STATE_GOLD_GATHERED=ConvertPlayerState(15)
constant playerstate PLAYER_STATE_LUMBER_GATHERED=ConvertPlayerState(16)
endglobals
";
    deliver_timer_with(app, script, 1, common, |inputs| {
        inputs.timer_time = Some(0.);
        inputs.numeric = Some(numeric(true));
    });
    let catalog = Arc::clone(
        &app.world()
            .resource::<MapRequests>()
            .bindings
            .as_ref()
            .unwrap()
            .catalog
            .0,
    );
    let mut inputs = host_inputs(catalog);
    inputs.timer_time = Some(0.);
    inputs.numeric = Some(numeric(true));
    inputs.resources = Some(pcx_play::player_resources::Rules::synthetic_fixture());
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let context = Arc::clone(session.context().unwrap());
    let common = context
        .scripts
        .source(&context.scripts.digests[0].label)
        .unwrap();
    let ai_common = format!(
        "{}native BreakWake takes nothing returns nothing\n",
        super::super::AI_COMMON
    );
    let ai = Arc::new(
        pcx_play::scripts::MapScripts::compile(
            AssetIdentity {
                edition: "synthetic".into(),
                version: "paired-ai-sleep".into(),
                source: "idle.ai".into(),
            },
            ScriptAsset {
                label: "fixture:common.j",
                bytes: common.as_bytes(),
            },
            ScriptAsset {
                label: "fixture:common.ai",
                bytes: ai_common.as_bytes(),
            },
            ScriptAsset {
                label: "idle.ai",
                bytes: ai_source.as_bytes(),
            },
            None,
        )
        .unwrap(),
    );
    let entries = ["config", "main", "start"]
        .map(|name| context.scripts.program().function_id(name).unwrap());
    session
        .replace(
            context,
            move |ctx| {
                Ok::<_, MapStartupError>(
                    pcx_play::map_startup_host::assemble(ctx, inputs)?
                        .with_ai_pause_rules(Ok(pcx_play::campaign_ai_pause_policy::fixture(1)))
                        .with_ai_loader(move |_| Ok(Arc::clone(&ai))),
                )
            },
            &entries,
        )
        .unwrap();
}

const AI_BANK_OBSERVERS: &str = "\
globals
timer first=CreateTimer()
timer middle=CreateTimer()
timer last=CreateTimer()
integer first0=0
integer first1=0
integer middle0=0
integer middle1=0
integer last0=0
integer last1=0
endglobals
function firstBank takes nothing returns nothing
set first0=GetPlayerState(Player(0),PLAYER_STATE_RESOURCE_GOLD)
set first1=GetPlayerState(Player(1),PLAYER_STATE_RESOURCE_GOLD)
endfunction
function middleBank takes nothing returns nothing
set middle0=GetPlayerState(Player(0),PLAYER_STATE_RESOURCE_GOLD)
set middle1=GetPlayerState(Player(1),PLAYER_STATE_RESOURCE_GOLD)
endfunction
function lastBank takes nothing returns nothing
set last0=GetPlayerState(Player(0),PLAYER_STATE_RESOURCE_GOLD)
set last1=GetPlayerState(Player(1),PLAYER_STATE_RESOURCE_GOLD)
endfunction
function config takes nothing returns nothing
call SetPlayers(2)
endfunction
function main takes nothing returns nothing
call TimerStart(first,0.125,false,function firstBank)
call TimerStart(middle,0.375,false,function middleBank)
call TimerStart(last,0.625,false,function lastBank)
endfunction
function start takes nothing returns nothing
call StartCampaignAI(Player(0),\"idle.ai\")
call StartCampaignAI(Player(1),\"idle.ai\")
endfunction
";

const AI_RETAINED_LOCALS: &str = "\
function main takes nothing returns nothing
local integer retained=73+GetAiPlayer()
call SetPlayerState(Player(GetAiPlayer()),ConvertPlayerState(1),17+GetAiPlayer())
call Sleep(0.25)
call SetPlayerState(Player(GetAiPlayer()),ConvertPlayerState(1),retained)
call Sleep(0.25)
call SetPlayerState(Player(GetAiPlayer()),ConvertPlayerState(1),retained+100)
endfunction
";

fn assert_ai_bank_observers(session: &MapScriptSession<MapStartupHost>) {
    for (name, expected) in [
        ("first0", 17),
        ("first1", 18),
        ("middle0", 73),
        ("middle1", 74),
        ("last0", 173),
        ("last1", 174),
    ] {
        assert_eq!(session.global(name), Some(Value::Int(expected)), "{name}");
    }
}

#[test]
#[ignore = "requires explicit PCX_WAR3_EXE selected executable"]
fn paired_game_ai_wakes_between_map_timers_and_retains_each_players_locals() {
    let mut app = timer_app();
    publish_ai_sleeper(&mut app, AI_BANK_OBSERVERS, AI_RETAINED_LOCALS);
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    session
        .advance_native_frame(
            Some(clock::FrameInput {
                game_elapsed: Duration::from_millis(750),
                real_elapsed_ms: 0,
            }),
            16,
        )
        .unwrap();
    assert_ai_bank_observers(&session);
    assert_eq!(
        session
            .host()
            .unwrap()
            .objects()
            .native_queue_point(clock::Domain::Game),
        Some(clock::Point {
            cycle: 0,
            local: 0.75f32.to_bits()
        })
    );
    assert_eq!(
        session
            .host()
            .unwrap()
            .objects()
            .native_queue_point(clock::Domain::Real),
        Some(clock::Point { cycle: 0, local: 0 })
    );
    session.advance_native_frame(None, 16).unwrap();
    assert_ai_bank_observers(&session);
}

#[test]
#[ignore = "requires explicit PCX_WAR3_EXE selected executable"]
fn paired_game_ai_budget_retry_retains_tickets_without_replaying_map_or_ai() {
    let mut app = timer_app();
    publish_ai_sleeper(&mut app, AI_BANK_OBSERVERS, AI_RETAINED_LOCALS);
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let error = session
        .advance_native_frame(
            Some(clock::FrameInput {
                game_elapsed: Duration::from_millis(750),
                real_elapsed_ms: 0,
            }),
            1,
        )
        .unwrap_err();
    assert!(error.to_string().contains("wake budget"), "{error}");
    assert_eq!(session.global("first0"), Some(Value::Int(17)));
    assert_eq!(session.global("first1"), Some(Value::Int(18)));
    assert_eq!(session.global("middle0"), Some(Value::Int(0)));
    session.advance_native_frame(None, 16).unwrap();
    assert_ai_bank_observers(&session);
    session.advance_native_frame(None, 16).unwrap();
    assert_ai_bank_observers(&session);
}

#[test]
#[ignore = "requires explicit PCX_WAR3_EXE selected executable"]
fn paired_game_ai_native_repark_accepts_zero_and_small_negative_delays() {
    use pcx_script::Host;
    let mut app = timer_app();
    publish_ai_sleeper(
        &mut app,
        AI_BANK_OBSERVERS,
        "\
function main takes nothing returns nothing
local integer retained=73+GetAiPlayer()
call Sleep(0.25)
call SetPlayerState(Player(GetAiPlayer()),ConvertPlayerState(1),retained)
call Sleep(0.0)
call SetPlayerState(Player(GetAiPlayer()),ConvertPlayerState(1),retained+100)
call Sleep(-0.25)
call SetPlayerState(Player(GetAiPlayer()),ConvertPlayerState(1),retained+200)
endfunction
",
    );
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    session
        .advance_native_frame(
            Some(clock::FrameInput {
                game_elapsed: Duration::from_millis(500),
                real_elapsed_ms: 0,
            }),
            16,
        )
        .unwrap();
    assert_eq!(session.global("middle0"), Some(Value::Int(273)));
    assert_eq!(session.global("middle1"), Some(Value::Int(274)));
    let host = session.host().unwrap();
    let gold = host.native("ConvertPlayerState", &[Value::Int(1)]).unwrap();
    for player in 0..2 {
        let handle = host.native("Player", &[Value::Int(player)]).unwrap();
        assert_eq!(
            host.native("GetPlayerState", &[handle, gold.clone()])
                .unwrap(),
            Value::Int(273 + player)
        );
    }
}

#[test]
#[ignore = "requires explicit PCX_WAR3_EXE selected executable"]
fn paired_game_ai_nested_pause_retries_keep_the_original_vm_ticket() {
    use pcx_script::Host;
    let mut app = timer_app();
    publish_ai_sleeper(&mut app, AI_BANK_OBSERVERS, AI_RETAINED_LOCALS);
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let host = session.host().unwrap();
    let player = host.native("Player", &[Value::Int(0)]).unwrap();
    let gold = host.native("ConvertPlayerState", &[Value::Int(1)]).unwrap();
    for _ in 0..2 {
        host.native("PauseCompAI", &[player.clone(), Value::Bool(true)])
            .unwrap();
    }
    for (milliseconds, expected, resume) in [
        (500, 17, true),
        (750, 17, true),
        (875, 17, false),
        (125, 73, false),
        (250, 173, false),
    ] {
        session
            .advance_native_frame(
                Some(clock::FrameInput {
                    game_elapsed: Duration::from_millis(milliseconds),
                    real_elapsed_ms: 0,
                }),
                32,
            )
            .unwrap();
        let host = session.host().unwrap();
        assert_eq!(
            host.native("GetPlayerState", &[player.clone(), gold.clone()])
                .unwrap(),
            Value::Int(expected)
        );
        if resume {
            host.native("PauseCompAI", &[player.clone(), Value::Bool(false)])
                .unwrap();
        }
    }
    let host = session.host().unwrap();
    let other = host.native("Player", &[Value::Int(1)]).unwrap();
    assert_eq!(
        host.native("GetPlayerState", &[other, gold]).unwrap(),
        Value::Int(174)
    );
}

#[test]
#[ignore = "requires explicit PCX_WAR3_EXE selected executable"]
fn paired_game_ai_wake_refusal_returns_world_and_preserves_other_players_ticket() {
    use pcx_script::Host;
    let mut app = timer_app();
    publish_ai_sleeper(
        &mut app,
        AI_BANK_OBSERVERS,
        "\
function main takes nothing returns nothing
call SetPlayerState(Player(GetAiPlayer()),ConvertPlayerState(1),17+GetAiPlayer())
call Sleep(0.25)
call SetPlayerState(Player(GetAiPlayer()),ConvertPlayerState(1),73+GetAiPlayer())
if GetAiPlayer()==0 then
call BreakWake()
endif
call Sleep(0.25)
call SetPlayerState(Player(GetAiPlayer()),ConvertPlayerState(1),173+GetAiPlayer())
endfunction
",
    );
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let error = session
        .advance_native_frame(
            Some(clock::FrameInput {
                game_elapsed: Duration::from_millis(750),
                real_elapsed_ms: 0,
            }),
            16,
        )
        .unwrap_err();
    assert!(error.to_string().contains("BreakWake"), "{error}");
    let host = session.host().unwrap();
    let gold = host.native("ConvertPlayerState", &[Value::Int(1)]).unwrap();
    let player = host.native("Player", &[Value::Int(0)]).unwrap();
    assert_eq!(
        host.native("GetPlayerState", &[player, gold]).unwrap(),
        Value::Int(73)
    );
    session.advance_native_frame(None, 16).unwrap();
    for (name, expected) in [
        ("first0", 17),
        ("first1", 18),
        ("middle0", 73),
        ("middle1", 74),
        ("last0", 73),
        ("last1", 174),
    ] {
        assert_eq!(session.global(name), Some(Value::Int(expected)), "{name}");
    }
    session.advance_native_frame(None, 16).unwrap();
    assert_eq!(session.global("last1"), Some(Value::Int(174)));
}

#[test]
#[ignore = "requires explicit PCX_WAR3_EXE selected executable"]
fn paired_game_ai_wrap_rebases_raw_sleep_and_repark_without_early_resume() {
    use pcx_script::Host;
    let mut app = timer_app();
    publish_ai_sleeper(
        &mut app,
        AI_BANK_OBSERVERS,
        "\
function main takes nothing returns nothing
local integer retained=73+GetAiPlayer()
call SetPlayerState(Player(GetAiPlayer()),ConvertPlayerState(1),17+GetAiPlayer())
call Sleep(40.25)
call SetPlayerState(Player(GetAiPlayer()),ConvertPlayerState(1),retained)
call Sleep(0.0317)
call SetPlayerState(Player(GetAiPlayer()),ConvertPlayerState(1),retained+100)
endfunction
",
    );
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    for (millis, expected) in [(40_000, 17), (500, 173)] {
        session
            .advance_native_frame(
                Some(clock::FrameInput {
                    game_elapsed: Duration::from_millis(millis),
                    real_elapsed_ms: 0,
                }),
                16,
            )
            .unwrap();
        let host = session.host().unwrap();
        let gold = host.native("ConvertPlayerState", &[Value::Int(1)]).unwrap();
        for player in 0..2 {
            let handle = host.native("Player", &[Value::Int(player)]).unwrap();
            assert_eq!(
                host.native("GetPlayerState", &[handle, gold.clone()])
                    .unwrap(),
                Value::Int(expected + player)
            );
        }
    }
    assert_eq!(
        session
            .host()
            .unwrap()
            .objects()
            .native_queue_point(clock::Domain::Game),
        Some(clock::Point {
            cycle: 2,
            local: 8.5f32.to_bits()
        })
    );
    assert_eq!(
        session
            .host()
            .unwrap()
            .objects()
            .native_queue_point(clock::Domain::Real),
        Some(clock::Point { cycle: 0, local: 0 })
    );
}

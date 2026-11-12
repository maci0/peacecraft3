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

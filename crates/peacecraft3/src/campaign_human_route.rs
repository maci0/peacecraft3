//! Local-original Human01 route through public commands and the production clock.
//! Selectors identify authored script objects; geometry, group membership,
//! quest state and player results are read from the unchanged running map.

use bevy::prelude::*;
use pcx_play::{
    map_startup_host::MapStartupHost,
    player_command_log::{PlayerCommand, StartIdentity},
    playercolor_host::{ObjectHost, VisionCapability, VisionState},
    standard_command::Kind,
};
use pcx_script::{HandleId, Host, Value};
use peacecraft3::{
    campaign_runtime::CampaignRuntime, campaign_startup::CampaignSeed,
    map_script_session::MapScriptSession,
};
use std::{path::Path, time::Duration};

#[test]
#[ignore = "requires unchanged Human01 via PCX_NATIVE_CHAPTER_* selecting roc, plus fresh isolated XDG_DATA_HOME under .scratch"]
fn selected_human01_route_reaches_authored_victory() {
    let scratch = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.scratch")
        .canonicalize()
        .expect("local test artifacts directory");
    let profile = std::env::var_os("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .expect("set XDG_DATA_HOME to a fresh isolated test profile");
    let profile = profile
        .canonicalize()
        .expect("create the isolated profile directory");
    assert!(
        profile.starts_with(scratch),
        "test profile must stay under .scratch"
    );
    let metadata = profile.metadata().expect("isolated profile metadata");
    assert!(metadata.is_dir(), "test profile must be a directory");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            metadata.permissions().mode() & 0o777,
            0o700,
            "test profile must be private"
        );
    }
    assert!(
        !profile.join("peacecraft3/campaign-cache.bin").exists(),
        "use a fresh campaign profile"
    );
    println!(
        "Human01 initial profile: path={} cache=absent",
        profile.display()
    );
    let map = std::env::var("PCX_NATIVE_CHAPTER_MAP").expect("select the original Human01 chapter");
    assert!(
        map.rsplit(['\\', '/'])
            .next()
            .unwrap()
            .eq_ignore_ascii_case("Human01.w3m")
    );
    assert_eq!(
        std::env::var("PCX_NATIVE_CHAPTER_ADDON").as_deref(),
        Ok("roc")
    );
    let mut app = crate::native_campaign::tests::started_app();
    let identity = {
        let session = app.world().resource::<MapScriptSession<MapStartupHost>>();
        StartIdentity::from_context(
            app.world()
                .resource::<CampaignSeed>()
                .prepared
                .expect("prepared seed"),
            session.context().expect("admitted context"),
        )
    };
    println!("Human01 start identity: {identity:?}");
    app.world_mut()
        .resource_mut::<crate::campaign_input::CampaignInput>()
        .commands
        .begin(identity);
    // Application test pacing and work bounds, not authored game durations.
    let mut runtime = CampaignRuntime::new(Duration::from_secs_f64(1.0 / 64.0), 64, 4096);
    {
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        assert_eq!(runtime.advance_ticks(&mut session, 0).unwrap(), 0);
    }
    let mut entered = false;
    let mut encounter_ready = false;
    let mut target = None;
    let mut healing = None;
    let mut support = false;
    let mut support_moved = false;
    let mut recorded = 0;
    let mut saw_victory_group = false;
    let mut saw_rescuables = false;
    let mut rescue_witnessed = false;
    let mut last = String::new();
    let mut outcome_waits = 0;
    for batch in 0..900 {
        let state = observe(&mut app, target);
        last = format!(
            "tick={} clock={} hero_position={:?} main_quest_completed={} {state:?}",
            runtime.tick(),
            state.clock,
            state.hero_position,
            state.quest_completed
        );
        if batch % 16 == 0 {
            println!("Human01 checkpoint: {last}");
            diagnostics(&mut app);
        }
        entered |= state.entry_witness;
        if !encounter_ready && state.orc_scene_complete {
            assert!(
                entered,
                "encounter completed without witnessing the authored entrance: {last}"
            );
            encounter_ready = true;
        }
        saw_rescuables |= state.rescue_members > 0;
        if !rescue_witnessed && state.rescue_complete {
            assert!(
                saw_rescuables,
                "rescue must have a nonempty source-group witness"
            );
            rescue_witnessed = true;
            println!(
                "Human01 authored rescue witnessed: {last}; surviving_group={} owned_survivors={}",
                state.rescue_survivors, state.rescued_survivors
            );
        }
        saw_victory_group |= state.group_members > 0;
        assert!(state.hero_life > 0.0, "Arthas died: {last}");
        if state.control && state.fighters_alive == 0 && state.group_alive > 0 {
            diagnostics(&mut app);
            panic!("all mobile combat allies died: {last}");
        }
        // Route policy: withdraw below half health; this is not an authored game rule.
        if encounter_ready && !support && state.hero_life < state.hero_max_life * 0.5 {
            support = true;
            println!("Human01 hero switches to support: {last}");
        }
        if state.victory && state.quest_items_complete && state.group_alive == 0 {
            assert!(
                saw_victory_group,
                "an empty group cannot qualify the victory"
            );
            if state.exit_requested {
                let input = app
                    .world()
                    .resource::<crate::campaign_input::CampaignInput>();
                assert!(input.last_error.is_none(), "{:?}", input.last_error);
                assert!(!input.commands.entries().is_empty());
                println!(
                    "Human01 authored victory: {last}; accepted_commands={}",
                    input.commands.entries().len()
                );
                continue_campaign(&mut app, runtime);
                return;
            }
            outcome_waits += 1;
            assert!(
                outcome_waits <= 64,
                "victory dialog did not become ready: {last}"
            );
            continue_victory(&mut app);
        } else if !state.control {
            skip_cinematic(&mut app);
            target = None;
        } else {
            let casting = healing.is_some_and(|cast| healing_pending(&mut app, cast));
            if healing.is_some() && !casting {
                healing = None;
                support_moved = false;
            }
            let bounds = if !rescue_witnessed {
                state.rescue_region
            } else if encounter_ready {
                state.boss_region
            } else {
                state.entrance
            };
            target = state.visible_enemy;
            let changed = if let Some(enemy) = target {
                order(
                    &mut app,
                    Kind::Attack,
                    center(bounds),
                    Some(enemy),
                    support || casting,
                )
            } else if support {
                order(&mut app, Kind::Move, center(bounds), None, true)
            } else {
                let changed = order(
                    &mut app,
                    Kind::Move,
                    state.hero_position,
                    Some(state.hero),
                    true,
                );
                if !casting {
                    if state.followers_ready {
                        move_hero(&mut app, center(bounds));
                    } else {
                        hold_hero(&mut app);
                    }
                }
                changed
            };
            if changed {
                println!(
                    "Human01 squad intent: {last}; target={target:?} rescue_witnessed={rescue_witnessed}"
                );
                diagnostics(&mut app);
            }
        }
        if state.control
            && support
            && healing.is_none()
            && (!support_moved || (state.hero_target.is_some() && !state.hero_moving))
        {
            move_hero(&mut app, center(state.entrance));
            support_moved = true;
        }
        if state.control && healing.is_none() {
            healing = heal_ally(&mut app);
        }
        {
            let input = app
                .world()
                .resource::<crate::campaign_input::CampaignInput>();
            for entry in &input.commands.entries()[recorded..] {
                println!("Human01 accepted command: {entry:?}");
            }
            recorded = input.commands.entries().len();
        }
        let result = {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            runtime.advance_ticks(&mut session, 64)
        };
        if let Err(error) = result {
            let failed = observe(&mut app, target);
            println!(
                "Human01 execution failure: completed_tick={} actual_host_clock={} {failed:?}",
                runtime.tick(),
                failed.clock
            );
            diagnostics(&mut app);
            panic!(
                "{error}; last_completed_observation={last}; failing tick may have partial effects"
            );
        }
    }
    let input = app
        .world()
        .resource::<crate::campaign_input::CampaignInput>();
    panic!(
        "Human01 exhausted the tick budget: {last}; entered={entered} encounter_ready={encounter_ready} accepted_commands={} last_command_error={:?}",
        input.commands.entries().len(),
        input.last_error
    );
}

#[cfg(feature = "test-fixtures")]
#[test]
#[ignore = "requires unchanged Human02 via PCX_NATIVE_CHAPTER_*, and an isolated clone of a legitimately produced campaign profile"]
fn selected_human02_continuation_public_training() {
    let (mut app, mut runtime) = human02_controlled();
    train_public_unit(&mut app, &mut runtime);
}

#[cfg(feature = "test-fixtures")]
pub(super) fn human02_controlled() -> (App, CampaignRuntime) {
    let profile = std::path::PathBuf::from(
        std::env::var_os("XDG_DATA_HOME").expect("isolated campaign profile"),
    )
    .canonicalize()
    .unwrap();
    let scratch = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.scratch")
        .canonicalize()
        .unwrap();
    assert!(profile.starts_with(scratch));
    assert!(
        profile.join("peacecraft3/campaign-cache.bin").is_file(),
        "Human02 requires earned carryover"
    );
    use sha2::Digest;
    let expected =
        std::env::var("PCX_NATIVE_CAMPAIGN_CACHE_SHA256").expect("pin the earned input cache");
    assert_eq!(expected.len(), 64);
    assert!(expected.bytes().all(|b| b.is_ascii_hexdigit()));
    let bytes = std::fs::read(profile.join("peacecraft3/campaign-cache.bin")).unwrap();
    assert_eq!(
        sha2::Sha256::digest(&bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
        expected.to_ascii_lowercase()
    );
    println!(
        "Human02 earned input cache: sha256={expected} bytes={}",
        bytes.len()
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            profile.metadata().unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
    assert!(
        std::env::var("PCX_NATIVE_CHAPTER_MAP")
            .unwrap()
            .rsplit(['\\', '/'])
            .next()
            .unwrap()
            .eq_ignore_ascii_case("Human02.w3m")
    );
    let mut app = crate::native_campaign::tests::started_app();
    let identity = StartIdentity::from_context(
        app.world().resource::<CampaignSeed>().prepared.unwrap(),
        app.world()
            .resource::<MapScriptSession<MapStartupHost>>()
            .context()
            .unwrap(),
    );
    println!("Human02 start identity: {identity:?}");
    let camera = app.world_mut().spawn_empty().id();
    app.insert_resource(peacecraft3::campaign_camera::CampaignCamera::active_for_test(camera))
        .init_resource::<crate::campaign_hud::CampaignHud>()
        .init_resource::<crate::campaign_inventory::CampaignInventory>()
        .init_resource::<crate::campaign_script_dialogs::CampaignScriptDialogs>()
        .init_resource::<crate::campaign_command_card::CommandCard>();
    let mut runtime = CampaignRuntime::new(Duration::from_secs_f64(1.0 / 64.0), 64, 4096);
    {
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let hero = handle(&session, "udg_Arthas");
        assert_eq!(handle(&session, "bj_lastLoadedUnit"), hero);
        let objects = session.host().unwrap().objects();
        assert!(
            objects.executed_unit(hero).unwrap().cached,
            "default hero is not carryover"
        );
        assert_eq!(runtime.advance_ticks(&mut session, 0).unwrap(), 0);
    }
    for _ in 0..120 {
        let controlled = {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let objects = session.host().unwrap().objects();
            objects.presentation().is_some_and(|p| {
                p.control_enabled == Some(true) && p.interface_visible == Some(true)
            })
        };
        if controlled {
            {
                let mut session = app
                    .world_mut()
                    .resource_mut::<MapScriptSession<MapStartupHost>>();
                let objects = session.host().unwrap().objects();
                let local = objects.local_player_settings().unwrap();
                for state in [
                    "PLAYER_STATE_RESOURCE_GOLD",
                    "PLAYER_STATE_RESOURCE_LUMBER",
                    "PLAYER_STATE_RESOURCE_FOOD_CAP",
                    "PLAYER_STATE_RESOURCE_FOOD_USED",
                ] {
                    println!(
                        "Human02 economy: clock={:?} state={state} value={:?}",
                        objects.timer_game_time(),
                        objects
                            .player_resource_value(local.identity, state)
                            .unwrap()
                    );
                }
                for (id, unit) in objects
                    .executed_units()
                    .filter(|(_, unit)| unit.owner == local.identity)
                {
                    println!(
                        "Human02 local unit: id={id:?} rawcode={:?} position={:?} shown={} vitals={:?} training={:?}",
                        unit.definition.rawcode,
                        unit.position,
                        unit.shown,
                        unit.vitals,
                        pcx_play::training_command::types(&unit.definition)
                    );
                }
                println!(
                    "Human02 authored control reached at tick={} clock={:?}; mission victory not yet reached",
                    runtime.tick(),
                    objects.timer_game_time()
                );
            }
            return (app, runtime);
        }
        skip_cinematic(&mut app);
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        runtime.advance_ticks(&mut session, 64).unwrap();
    }
    panic!("Human02 did not restore control within bounded canonical work");
}

#[cfg(feature = "test-fixtures")]
pub(super) fn human02_workers_ready(app: &mut App, runtime: &mut CampaignRuntime) {
    let gold_names = ["udg_Gold01", "udg_Gold02", "udg_Gold03"];
    let initial_gold = {
        let session = app.world().resource::<MapScriptSession<MapStartupHost>>();
        gold_names.map(|name| session.global(name).expect("source gold worker"))
    };
    let mut handed_over = false;
    for _ in 0..120 {
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let current = gold_names.map(|name| session.global(name).expect("source gold worker"));
        let objects = session.host().unwrap().objects();
        handed_over = current.iter().zip(&initial_gold).all(|(current, initial)| {
                current != initial && matches!(current, Value::Handle(id) if objects.executed_unit(*id).is_ok_and(|unit| unit.vitals.is_some_and(|v| v.life > 0.)))
            }) && objects.presentation().is_some_and(|p| p.control_enabled == Some(true));
        if handed_over {
            println!(
                "Human02 source worker handover witnessed: tick={} clock={:?} original={initial_gold:?} current={current:?}",
                runtime.tick(),
                objects.timer_game_time()
            );
            break;
        }
        runtime.advance_ticks(&mut session, 64).unwrap();
    }
    if !handed_over {
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let current = gold_names.map(|name| session.global(name));
        panic!(
            "Human02 worker handover exhausted canonical work: tick={} clock={:?} current={current:?}",
            runtime.tick(),
            session.host().unwrap().objects().timer_game_time()
        );
    }
}

#[cfg(feature = "test-fixtures")]
#[test]
#[ignore = "requires unchanged Human02 and private earned carryover via PCX_NATIVE_CHAPTER_*"]
fn selected_human02_route_reaches_authored_base() {
    let _ = human02_base();
}

#[cfg(feature = "test-fixtures")]
fn human02_base() -> (App, CampaignRuntime) {
    use crate::campaign_command_card::Kind as CardKind;
    let (mut app, mut runtime) = human02_controlled();
    human02_workers_ready(&mut app, &mut runtime);
    let (barracks, farm, footman, farms_needed, footmen_needed) = {
        let session = app.world().resource::<MapScriptSession<MapStartupHost>>();
        (
            human02_source_kind(
                session,
                "Trig_BarracksConstructed_Conditions",
                "GetConstructedStructure",
            ),
            human02_source_kind(
                session,
                "Trig___FarmCompletes_Conditions",
                "GetConstructedStructure",
            ),
            human02_source_kind(
                session,
                "Trig___FootmanPopsOut_Conditions",
                "GetTrainedUnit",
            ),
            human02_source_count(
                session,
                "Trig_FarmsConstructed_Conditions",
                "udg_NumberOfFarms",
            ),
            human02_source_count(
                session,
                "Trig_FootmenTrained_Conditions",
                "udg_FootmenTrained",
            ),
        )
    };
    println!(
        "Human02 source base requirements: barracks={:?} farm={:?} footman={:?} farms={farms_needed} footmen={footmen_needed}",
        barracks.to_be_bytes(),
        farm.to_be_bytes(),
        footman.to_be_bytes()
    );
    let mut pending = None;
    for batch in 0..900 {
        let (flags, farms, footmen, controlled, complete, clock) = {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let flags = [
                "udg_BaseRequirement01",
                "udg_BaseRequirement02",
                "udg_BaseRequirement03",
            ]
            .map(|name| {
                let Some(Value::Bool(value)) = session.global(name) else {
                    panic!("source flag {name}")
                };
                value
            });
            let Some(Value::Int(farms)) = session.global("udg_NumberOfFarms") else {
                panic!("source farm count")
            };
            let Some(Value::Int(footmen)) = session.global("udg_FootmenTrained") else {
                panic!("source trained count")
            };
            let complete = session.global("udg_VictoryCondition01") == Some(Value::Bool(true));
            let objects = session.host().unwrap().objects();
            (
                flags,
                farms,
                footmen,
                objects
                    .presentation()
                    .is_some_and(|p| p.control_enabled == Some(true)),
                complete,
                objects.timer_game_time(),
            )
        };
        if batch % 16 == 0 || complete {
            println!(
                "Human02 base checkpoint: tick={} clock={clock:?} flags={flags:?} farms={farms}/{farms_needed} footmen={footmen}/{footmen_needed} pending={pending:?}",
                runtime.tick()
            );
            human02_pending_diagnostics(&mut app, pending);
        }
        if complete {
            assert!(flags.into_iter().all(|flag| flag));
            println!(
                "Human02 actual authored base quest completed; accepted commands={:?}",
                app.world()
                    .resource::<crate::campaign_input::CampaignInput>()
                    .commands
                    .entries()
            );
            return (app, runtime);
        }
        if controlled {
            if let Some(worker) = pending {
                let mut session = app
                    .world_mut()
                    .resource_mut::<MapScriptSession<MapStartupHost>>();
                let objects = session.host().unwrap().objects();
                if objects.unit_construction_order(worker).unwrap().is_none() {
                    pending = None;
                }
            }
            let needed = if !flags[0] {
                Some(barracks)
            } else if !flags[1] && farms < farms_needed {
                Some(farm)
            } else {
                None
            };
            if let Some(kind) = needed.filter(|_| pending.is_none()) {
                let candidate = {
                    let mut session = app
                        .world_mut()
                        .resource_mut::<MapScriptSession<MapStartupHost>>();
                    let objects = session.host().unwrap().objects_mut();
                    let local = objects.local_player_settings().unwrap().identity;
                    let completed = kind == barracks
                        && objects.executed_units().any(|(id, unit)| {
                            unit.owner == local
                                && unit.definition.rawcode == kind.to_be_bytes()
                                && unit.vitals.is_some_and(|v| v.life > 0.)
                                && objects.unit_construction(id).unwrap().is_none()
                        });
                    if completed {
                        None
                    } else {
                        let worker = objects
                            .executed_units()
                            .filter(|(_, unit)| unit.owner == local && unit.shown)
                            .find_map(|(id, _)| {
                                objects
                                    .unit_build_options(id)
                                    .unwrap()
                                    .iter()
                                    .any(|option| {
                                        option.kind == kind && option.unavailable.is_none()
                                    })
                                    .then_some(id)
                            });
                        worker.and_then(|worker| {
                            human02_build_point(objects, worker, kind).map(|point| (worker, point))
                        })
                    }
                };
                if let Some((worker, point)) = candidate {
                    human02_select(&mut app, worker);
                    human02_button(&mut app, worker, CardKind::BuildMenu);
                    human02_button(&mut app, worker, CardKind::Build(kind));
                    let accepted = app.world_mut().resource_scope(
                        |world, mut input: Mut<crate::campaign_input::CampaignInput>| {
                            let mut session =
                                world.resource_mut::<MapScriptSession<MapStartupHost>>();
                            let witness = session.session_witness().unwrap();
                            crate::campaign_input::place_build(
                                session.host().unwrap(),
                                &mut input,
                                Some(&witness),
                                point,
                            )
                            .unwrap()
                        },
                    );
                    assert!(
                        accepted,
                        "source-ready building placement refused: worker={worker:?} kind={:?} point={point:?}",
                        kind.to_be_bytes()
                    );
                    pending = Some(worker);
                    println!(
                        "Human02 build accepted: tick={} worker={worker:?} kind={:?} point={point:?}",
                        runtime.tick(),
                        kind.to_be_bytes()
                    );
                }
            } else if flags[0] && flags[1] && !flags[2] && footmen < footmen_needed {
                let producer = {
                    let mut session = app
                        .world_mut()
                        .resource_mut::<MapScriptSession<MapStartupHost>>();
                    let objects = session.host().unwrap().objects();
                    let local = objects.local_player_settings().unwrap().identity;
                    objects.executed_units().find_map(|(id, unit)| {
                        (unit.owner == local
                            && unit.definition.rawcode == barracks.to_be_bytes()
                            && objects.unit_construction(id).unwrap().is_none()
                            && objects.unit_training_queue(id).unwrap().is_empty())
                        .then_some(id)
                    })
                };
                if let Some(producer) = producer {
                    human02_select(&mut app, producer);
                    human02_button(&mut app, producer, CardKind::Train(footman));
                }
            }
        } else {
            skip_cinematic(&mut app);
        }
        let result = {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            runtime.advance_ticks(&mut session, 64)
        };
        if let Err(error) = result {
            println!(
                "Human02 base execution stopped: completed_tick={} error={error}",
                runtime.tick()
            );
            human02_pending_diagnostics(&mut app, pending);
            diagnostics(&mut app);
            panic!("{error}; failing tick may have partial effects");
        }
    }
    human02_pending_diagnostics(&mut app, pending);
    panic!(
        "Human02 base quest exhausted bounded canonical work at tick={}",
        runtime.tick()
    );
}

#[cfg(feature = "test-fixtures")]
fn human02_pending_diagnostics(app: &mut App, pending: Option<HandleId>) {
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let objects = session.host().unwrap().objects_mut();
    println!(
        "Human02 pending snapshot: clock={:?} worker={pending:?}",
        objects.timer_game_time()
    );
    let Some(worker) = pending else { return };
    let current_order = objects.native("GetUnitCurrentOrder", &[Value::Handle(worker)]);
    let order = objects.unit_construction_order(worker);
    let state = objects.executed_unit(worker).map(|unit| {
        let activity = unit.activity.as_ref().map(|activity| {
            let (points, next) = activity.waypoints();
            (
                activity.destination(),
                (points.len(), next, points.get(next).copied()),
                activity.moving(),
            )
        });
        (
            unit.position,
            unit.owner,
            unit.vitals.map(|vitals| (vitals.life, vitals.life > 0.)),
            unit.paused_since,
            activity,
            unit.combat
                .as_ref()
                .map(|combat| (combat.target, combat.attack_destination)),
        )
    });
    println!(
        "Human02 pending worker: id={worker:?} state={state:?} current_order={current_order:?} construction={order:?} guard={:?}",
        objects.guard_state().and_then(|state| state.get(worker))
    );
    if let Ok(Some(order)) = order
        && let Some(site) = order.site
    {
        println!(
            "Human02 pending site: id={site:?} vitals={:?} construction={:?}",
            objects.executed_unit(site).map(|unit| unit.vitals),
            objects.unit_construction(site)
        );
    }
}

#[cfg(feature = "test-fixtures")]
fn human02_select(app: &mut App, unit: HandleId) {
    app.world_mut().resource_scope(
        |world, mut input: Mut<crate::campaign_input::CampaignInput>| {
            let mut session = world.resource_mut::<MapScriptSession<MapStartupHost>>();
            let objects = session.host().unwrap().objects_mut();
            let record = objects.executed_unit(unit).unwrap();
            assert_eq!(
                Some(objects.player_settings(record.owner).unwrap().number),
                objects.local_player_number()
            );
            assert!(record.shown && record.vitals.is_some_and(|v| v.life > 0.));
            if objects.selection() != [unit] {
                objects.select_units(&[unit], true);
                input
                    .commands
                    .record(
                        objects.timer_game_time(),
                        PlayerCommand::Select {
                            units: vec![unit],
                            replace: true,
                        },
                    )
                    .unwrap();
            }
        },
    );
}

#[cfg(feature = "test-fixtures")]
fn human02_button(app: &mut App, unit: HandleId, kind: crate::campaign_command_card::Kind) {
    use bevy::ui_widgets::{Activate, Button};
    let witness = app
        .world()
        .resource::<MapScriptSession<MapStartupHost>>()
        .session_witness()
        .unwrap();
    app.world_mut()
        .resource_mut::<crate::button_actions::ButtonActions>()
        .entities
        .clear();
    let button = app
        .world_mut()
        .spawn((
            Button,
            crate::campaign_command_card::ButtonOrder {
                unit,
                kind,
                session: witness,
                order: None,
            },
        ))
        .id();
    app.world_mut().trigger(Activate { entity: button });
    crate::campaign_input::run_command_button(app);
    app.world_mut().despawn(button);
    assert!(
        app.world()
            .resource::<crate::campaign_input::CampaignInput>()
            .last_error
            .is_none(),
        "{:?}",
        app.world()
            .resource::<crate::campaign_input::CampaignInput>()
            .last_error
    );
}

#[cfg(feature = "test-fixtures")]
fn human02_build_point(objects: &mut ObjectHost, worker: HandleId, kind: i32) -> Option<[f32; 2]> {
    let origin = objects.executed_unit(worker).unwrap().position.unwrap();
    let radius = collision(objects, worker);
    assert!(radius > 0.);
    // TAS placement-search bound; spacing follows the admitted worker collision radius.
    for y in -4..=4 {
        for x in -4..=4 {
            let point = [
                origin[0] + x as f32 * radius * 4.,
                origin[1] + y as f32 * radius * 4.,
            ];
            if let pcx_play::playercolor_host::ConstructionPreview::Allowed { position } = objects
                .preview_unit_construction(worker, kind, point)
                .unwrap()
            {
                return Some(position);
            }
        }
    }
    None
}

#[cfg(feature = "test-fixtures")]
fn human02_comparison(
    body: &[pcx_script::ast::Stmt],
) -> (&pcx_script::ast::Expr, &pcx_script::ast::Literal) {
    use pcx_script::ast::{BinOp, Expr, Literal, Stmt, UnOp};
    fn ungroup(mut value: &Expr) -> &Expr {
        while let Expr::Group(inner, _) = value {
            value = inner;
        }
        value
    }
    let Some(Stmt::If { branches, .. }) = body.first() else {
        panic!("source condition guard")
    };
    assert_eq!(branches.len(), 1);
    let Expr::Unary {
        op: UnOp::Not,
        operand,
        ..
    } = ungroup(&branches[0].cond)
    else {
        panic!("source negated guard")
    };
    assert!(matches!(
        branches[0].body.first(),
        Some(Stmt::Return(
            Some(Expr::Literal(Literal::Bool(false), _)),
            _
        ))
    ));
    let Expr::Binary {
        op: BinOp::Eq,
        lhs,
        rhs,
        ..
    } = ungroup(operand)
    else {
        panic!("source equality guard")
    };
    let Expr::Literal(value, _) = ungroup(rhs) else {
        panic!("source comparison literal")
    };
    (ungroup(lhs), value)
}

#[cfg(feature = "test-fixtures")]
fn human02_source_kind(
    session: &MapScriptSession<MapStartupHost>,
    function: &str,
    response: &str,
) -> i32 {
    use pcx_script::ast::{Expr, Literal};
    let (lhs, rhs) = human02_comparison(
        session
            .context()
            .unwrap()
            .scripts
            .program()
            .function(function)
            .unwrap()
            .body
            .as_deref()
            .unwrap(),
    );
    let Expr::Call { callee, args, .. } = lhs else {
        panic!("source unit type comparison")
    };
    assert!(matches!(callee.as_ref(), Expr::Ident(name, _) if name.name == "GetUnitTypeId"));
    let [Expr::Call { callee, args, .. }] = args.as_slice() else {
        panic!("source event response")
    };
    assert!(
        args.is_empty() && matches!(callee.as_ref(), Expr::Ident(name, _) if name.name == response)
    );
    let Literal::Raw(raw) = rhs else {
        panic!("source unit rawcode")
    };
    let bytes: [u8; 4] = raw
        .text
        .as_bytes()
        .try_into()
        .expect("four-byte source rawcode");
    assert!(bytes.iter().all(u8::is_ascii_graphic));
    i32::from_be_bytes(bytes)
}

#[cfg(feature = "test-fixtures")]
fn human02_source_count(
    session: &MapScriptSession<MapStartupHost>,
    function: &str,
    counter: &str,
) -> i32 {
    use pcx_script::ast::{Expr, Literal};
    let (lhs, rhs) = human02_comparison(
        session
            .context()
            .unwrap()
            .scripts
            .program()
            .function(function)
            .unwrap()
            .body
            .as_deref()
            .unwrap(),
    );
    assert!(matches!(lhs, Expr::Ident(name, _) if name.name == counter));
    let Literal::Int(value) = rhs else {
        panic!("source count literal")
    };
    let value = i32::try_from(value.value).unwrap();
    assert!(value > 0);
    value
}

#[cfg(feature = "test-fixtures")]
#[test]
fn source_quest_comparison_reads_a_nonstock_literal() {
    let script = pcx_script::parse("function Q takes nothing returns boolean\nif not (count == 13) then\nreturn false\nendif\nreturn true\nendfunction\n").unwrap();
    let pcx_script::ast::Item::Function(function) = &script.items[0] else {
        panic!()
    };
    let (lhs, rhs) = human02_comparison(function.body.as_deref().unwrap());
    assert!(matches!(lhs, pcx_script::ast::Expr::Ident(name, _) if name.name == "count"));
    assert!(matches!(rhs, pcx_script::ast::Literal::Int(value) if value.value == 13));
}

#[cfg(feature = "test-fixtures")]
fn train_public_unit(app: &mut App, runtime: &mut CampaignRuntime) {
    use crate::campaign_command_card::{ButtonOrder, Kind as CardKind};
    use bevy::ui_widgets::{Activate, Button};
    let (producer, kind, owner, before_gold, before_units, witness, clock) = {
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let witness = session.session_witness().unwrap();
        let objects = session.host().unwrap().objects_mut();
        let owner = objects.local_player_settings().unwrap().identity;
        let (producer, kind) = objects
            .executed_units()
            .filter(|(_, u)| u.owner == owner && u.shown)
            .find_map(|(id, unit)| {
                pcx_play::training_command::types(&unit.definition)
                    .unwrap()
                    .first()
                    .copied()
                    .map(|kind| (id, i32::from_be_bytes(kind)))
            })
            .expect("locally controlled source producer with Trains");
        let before_units = objects
            .executed_units()
            .map(|(id, _)| id)
            .collect::<Vec<_>>();
        let before_gold = objects
            .player_resource_value(owner, "PLAYER_STATE_RESOURCE_GOLD")
            .unwrap()
            .unwrap();
        objects.select_units(&[producer], true);
        assert_eq!(objects.selection(), &[producer]);
        (
            producer,
            kind,
            owner,
            before_gold,
            before_units,
            witness,
            objects.timer_game_time(),
        )
    };
    app.world_mut()
        .resource_mut::<crate::campaign_input::CampaignInput>()
        .commands
        .record(
            clock,
            PlayerCommand::Select {
                units: vec![producer],
                replace: true,
            },
        )
        .unwrap();
    let button = app
        .world_mut()
        .spawn((
            Button,
            ButtonOrder {
                unit: producer,
                kind: CardKind::Train(kind),
                session: witness,
                order: None,
            },
        ))
        .id();
    app.world_mut().trigger(Activate { entity: button });
    crate::campaign_input::run_command_button(app);
    assert!(
        app.world()
            .resource::<crate::campaign_input::CampaignInput>()
            .last_error
            .is_none()
    );
    for entry in app
        .world()
        .resource::<crate::campaign_input::CampaignInput>()
        .commands
        .entries()
    {
        println!("Human02 accepted command: {entry:?}");
    }
    let deadline = {
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let objects = session.host().unwrap().objects();
        let queue = objects.unit_training_queue(producer).unwrap();
        assert!(
            queue.iter().any(|(queued, _)| *queued == kind),
            "public button did not enqueue training"
        );
        assert!(
            objects
                .player_resource_value(owner, "PLAYER_STATE_RESOURCE_GOLD")
                .unwrap()
                .unwrap()
                < before_gold
        );
        println!(
            "Human02 accepted training: producer={producer:?} kind={:?} queue={queue:?}",
            kind.to_be_bytes()
        );
        queue
            .first()
            .and_then(|(_, finish)| *finish)
            .expect("active training deadline")
    };
    let mut seen_units = before_units;
    for _ in 0..120 {
        let result = {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            runtime.advance_ticks(&mut session, 64)
        };
        if let Err(error) = result {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let objects = session.host().unwrap().objects_mut();
            println!(
                "Human02 execution failure: completed_tick={} actual_host_clock={:?} queue={:?} error={error}",
                runtime.tick(),
                objects.timer_game_time(),
                objects.unit_training_queue(producer)
            );
            let units: Vec<_> = objects
                .executed_units()
                .filter(|(_, unit)| unit.owner == owner)
                .map(|(id, _)| id)
                .collect();
            for id in units {
                let order = objects.native("GetUnitCurrentOrder", &[Value::Handle(id)]);
                let unit = objects.executed_unit(id).unwrap();
                let destination = unit.activity.as_ref().and_then(|a| a.destination());
                let path = unit.activity.as_ref().map(|a| {
                    let (points, next) = a.waypoints();
                    (points.len(), next, points.get(next).copied())
                });
                println!(
                    "Human02 partial unit: id={id:?} rawcode={:?} position={:?} order={order:?} destination={destination:?} path={path:?}",
                    unit.definition.rawcode, unit.position
                );
            }
            panic!("{error}; failing tick may have partial effects");
        }
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let objects = session.host().unwrap().objects();
        let source = objects.executed_unit(producer).unwrap();
        assert_eq!(source.owner, owner, "training producer ownership changed");
        assert!(
            source.vitals.is_some_and(|v| v.life > 0.0),
            "training producer died"
        );
        if objects.unit_training_queue(producer).unwrap().is_empty() {
            assert!(
                objects.timer_game_time().unwrap() >= deadline,
                "queue ended before its accepted completion deadline"
            );
            let new_units: Vec<_> = objects
                .executed_units()
                .filter(|(id, unit)| {
                    !seen_units.contains(id)
                        && unit.owner == owner
                        && unit.definition.rawcode == kind.to_be_bytes()
                })
                .collect();
            assert_eq!(
                new_units.len(),
                1,
                "ambiguous or missing unit in the training completion projection"
            );
            let (id, unit) = new_units[0];
            println!(
                "Human02 actual public training finished: tick={} clock={:?} unit={id:?} rawcode={:?} deadline={deadline}; construction/economy route still pending",
                runtime.tick(),
                objects.timer_game_time(),
                unit.definition.rawcode
            );
            for state in [
                "PLAYER_STATE_RESOURCE_GOLD",
                "PLAYER_STATE_RESOURCE_LUMBER",
                "PLAYER_STATE_RESOURCE_FOOD_CAP",
                "PLAYER_STATE_RESOURCE_FOOD_USED",
            ] {
                println!(
                    "Human02 completion economy: state={state} value={:?}",
                    objects.player_resource_value(owner, state).unwrap()
                );
            }
            return;
        }
        seen_units = objects.executed_units().map(|(id, _)| id).collect();
    }
    panic!("public training did not complete within bounded canonical work");
}

fn continue_campaign(app: &mut App, mut runtime: CampaignRuntime) {
    use peacecraft3::{campaign_runtime, campaign_score};
    let previous = app
        .world()
        .resource::<MapScriptSession<MapStartupHost>>()
        .context()
        .unwrap()
        .map_sha256;
    let (progress, levels) = {
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let hero = handle(&session, "udg_Arthas");
        let unit = session
            .host()
            .unwrap()
            .objects()
            .executed_unit(hero)
            .unwrap();
        let abilities = unit.abilities.as_ref().expect("admitted hero abilities");
        (
            abilities.hero.clone(),
            abilities.levels().collect::<Vec<_>>(),
        )
    };
    assert!(
        !app.world()
            .contains_resource::<peacecraft3::campaign_runtime::CampaignTransition>()
    );
    // Keep application updates from adding wall-clock simulation ticks.
    runtime.paused = true;
    campaign_runtime::install(app, runtime);
    campaign_score::install(app);
    app.update();
    let request = {
        let mut score = app
            .world_mut()
            .resource_mut::<campaign_score::ScoreScreenState>();
        assert!(matches!(
            score.0,
            Some(campaign_score::ScoreScreen::Refused {
                local_result: Some(campaign_runtime::LocalGameResult::Victory),
                ..
            })
        ));
        assert_eq!(
            campaign_score::press(&mut score, "ScoreScreenOkButton"),
            Some(campaign_score::ActionOutcome::Continue)
        );
        score.continuation().expect("authored score continuation")
    };
    let next = request.next_level.clone();
    app.world_mut().write_message(request);
    for _ in 0..128 {
        app.update();
        let transition = app
            .world()
            .resource::<campaign_runtime::CampaignTransition>();
        assert!(
            transition.last_error.is_none(),
            "{:?}",
            transition.last_error
        );
        let startup = app
            .world()
            .resource::<peacecraft3::campaign_startup::CampaignStartup>();
        assert!(
            startup.last_error.is_none(),
            "next chapter startup: {:?}",
            startup.last_error
        );
        let seed = app.world().resource::<CampaignSeed>().prepared.unwrap();
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let Some(context) = session.context() else {
            continue;
        };
        if context.map_sha256 == previous {
            continue;
        }
        let identity = StartIdentity::from_context(seed, context);
        assert!(
            identity.sources.map.source == next,
            "{identity:?}; next={next}"
        );
        let restored = handle(&session, "bj_lastLoadedUnit");
        assert_eq!(
            handle(&session, "udg_Arthas"),
            restored,
            "source must use restored Arthas"
        );
        let unit = session
            .host()
            .unwrap()
            .objects()
            .executed_unit(restored)
            .unwrap();
        let abilities = unit.abilities.as_ref().expect("restored hero abilities");
        assert_eq!(abilities.hero, progress);
        assert_eq!(abilities.levels().collect::<Vec<_>>(), levels);
        println!(
            "Human02 actual carryover: identity={identity:?} hero={restored:?} rawcode={:?} vitals={:?} hero_state={:?}",
            unit.definition.rawcode, unit.vitals, abilities.hero
        );
        return;
    }
    panic!("authored next map did not publish within bounded application updates: {next}");
}

fn handle(session: &MapScriptSession<MapStartupHost>, name: &str) -> HandleId {
    match session.global(name) {
        Some(Value::Handle(id)) => id,
        other => panic!("{name}: expected an authored live handle, got {other:?}"),
    }
}

fn members(objects: &mut ObjectHost, group: HandleId) -> Vec<HandleId> {
    let units: Vec<_> = objects.executed_units().map(|(id, _)| id).collect();
    units
        .into_iter()
        .filter(|id| {
            match objects
                .native("IsUnitInGroup", &[Value::Handle(*id), Value::Handle(group)])
                .unwrap()
            {
                Value::Bool(member) => member,
                other => panic!("IsUnitInGroup returned {other:?}"),
            }
        })
        .collect()
}

fn center(bounds: [f32; 4]) -> [f32; 2] {
    [(bounds[0] + bounds[2]) * 0.5, (bounds[1] + bounds[3]) * 0.5]
}

fn inside([x, y]: [f32; 2], bounds: [f32; 4]) -> bool {
    (bounds[0]..=bounds[2]).contains(&x) && (bounds[1]..=bounds[3]).contains(&y)
}

#[derive(Debug)]
struct Observation {
    clock: f64,
    hero: HandleId,
    hero_position: [f32; 2],
    followers_ready: bool,
    rescue_region: [f32; 4],
    rescue_members: usize,
    rescue_survivors: usize,
    rescued_survivors: usize,
    rescue_complete: bool,
    hero_life: f32,
    hero_max_life: f32,
    hero_moving: bool,
    hero_target: Option<HandleId>,
    fighters_alive: usize,
    control: bool,
    entrance: [f32; 4],
    entry_witness: bool,
    orc_scene_complete: bool,
    boss_region: [f32; 4],
    group_members: usize,
    group_alive: usize,
    visible_enemy: Option<HandleId>,
    quest_completed: bool,
    quest_items_complete: bool,
    victory: bool,
    exit_requested: bool,
}

fn observe(app: &mut App, retained: Option<HandleId>) -> Observation {
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let hero = handle(&session, "udg_Arthas");
    let group = handle(&session, "udg_WarlordAndCrewGroup");
    let rescue_region = handle(&session, "gg_rct_RescueFootmen01");
    let rescue_group = handle(&session, "udg_RescuableFootmen");
    let rescue_trigger = handle(&session, "gg_trg_Rescue_Footmen");
    let entrance = handle(&session, "gg_rct_TriggerEndCinematic_02");
    let other_entrance = handle(&session, "gg_rct_TriggerEndCinematic_01");
    let orc_scene_complete =
        ["udg_EndSkipped", "udg_OrcQuestGiven"]
            .iter()
            .any(|name| match session.global(name) {
                Some(Value::Bool(value)) => value,
                other => panic!("{name}: expected authored boolean, got {other:?}"),
            });
    let boss_region = handle(&session, "gg_rct_EndBoss");
    let quest = session.global("udg_QuestVillage");
    let expected_result = handle(&session, "PLAYER_GAME_RESULT_VICTORY");
    let authored_next = session.global("bj_changeLevelMapName");
    let objects = session.host().expect("published host").objects_mut();
    let group = members(objects, group);
    let rescuable = members(objects, rescue_group);
    let squad = army(objects);
    let handle_from_hero = hero;
    let hero = objects.executed_unit(hero).unwrap();
    let local = objects.local_player_settings().expect("local player");
    let living: Vec<_> = group
        .iter()
        .copied()
        .filter(|id| {
            !objects
                .executed_unit(*id)
                .unwrap()
                .combat
                .as_ref()
                .expect("admitted combat")
                .dead
        })
        .collect();
    let hero_position = hero.position.expect("admitted hero position");
    let acquire = hero
        .combat
        .as_ref()
        .and_then(|c| c.weapon)
        .expect("hero weapon")
        .acquire;
    assert!(
        acquire.is_finite() && acquire > 0.0,
        "positive admitted acquisition range required"
    );
    let VisionCapability::Ready { fog, mask } = objects.vision_capability() else {
        panic!("admitted tactical vision")
    };
    let threats: Vec<_> = objects
        .executed_units()
        .filter_map(|(id, unit)| {
            let combat = unit.combat.as_ref()?;
            let weapon = combat.weapon?;
            let point = unit.position?;
            if !unit.shown
                || !objects.is_enemy(handle_from_hero, id)
                || objects
                    .vision_presented(local.identity, point, fog, mask)
                    .unwrap()
                    != Some(VisionState::Visible)
            {
                return None;
            }
            let attacking = combat.target.is_some_and(|target| squad.contains(&target));
            let distance = distance_between(hero_position, point);
            let near_attacked_ally =
                combat
                    .target
                    .filter(|id| squad.contains(id))
                    .is_some_and(|id| {
                        let ally = objects.executed_unit(id).unwrap();
                        let ally_acquire =
                            ally.combat.as_ref().and_then(|c| c.weapon).unwrap().acquire;
                        distance_between(point, ally.position.unwrap())
                            <= ally_acquire + weapon.range
                    });
            (distance <= acquire || near_attacked_ally).then_some((
                id,
                attacking,
                distance,
                unit.vitals.expect("enemy vitals").life,
            ))
        })
        .collect();
    let visible_enemy = retained
        .filter(|id| threats.iter().any(|entry| entry.0 == *id))
        .or_else(|| {
            threats
                .iter()
                .min_by(|a, b| {
                    b.1.cmp(&a.1)
                        .then_with(|| a.2.total_cmp(&b.2))
                        .then_with(|| a.3.total_cmp(&b.3))
                        .then_with(|| a.0.cmp(&b.0))
                })
                .map(|entry| entry.0)
        });
    let followers_ready = squad
        .iter()
        .filter(|id| **id != handle_from_hero)
        .all(|id| {
            let unit = objects.executed_unit(*id).unwrap();
            let activity = unit.activity.as_ref().unwrap();
            activity.followed_unit() == Some(handle_from_hero)
                && activity.follow_range().is_some_and(|range| {
                    let gap = distance_between(hero_position, unit.position.unwrap());
                    // TAS cohesion envelope uses the admitted tactical range, not a native follow leash.
                    gap <= range
                        || (!activity.moving()
                            && activity.waypoints().0.is_empty()
                            && gap <= acquire)
                })
        });
    let rescue_survivors = rescuable
        .iter()
        .filter(|id| {
            objects
                .executed_unit(**id)
                .unwrap()
                .vitals
                .is_some_and(|v| v.life > 0.0)
        })
        .count();
    let rescued_survivors = rescuable
        .iter()
        .filter(|id| {
            let unit = objects.executed_unit(**id).unwrap();
            unit.vitals.is_some_and(|v| v.life > 0.0) && unit.owner == local.identity
        })
        .count();
    let rescue_complete = objects.trigger_enabled(rescue_trigger).unwrap() == Some(false)
        && rescue_survivors == rescued_survivors;
    let (quest_completed, quest_items_complete) = match quest {
        Some(Value::Handle(id)) => {
            let quest = pcx_play::quest_view::project_quest(
                objects.quest_session().expect("quest session"),
                id,
            )
            .unwrap();
            (
                quest.completed,
                !quest.items.is_empty() && quest.items.iter().all(|item| item.completed),
            )
        }
        Some(Value::Null) => (false, false),
        other => panic!("udg_QuestVillage: {other:?}"),
    };
    Observation {
        clock: objects.timer_game_time().expect("admitted campaign clock"),
        hero: handle_from_hero,
        hero_position,
        followers_ready,
        rescue_region: objects.rectangle_bounds(rescue_region).unwrap(),
        rescue_members: rescuable.len(),
        rescue_survivors,
        rescued_survivors,
        rescue_complete,
        hero_life: hero.vitals.expect("admitted hero vitals").life,
        hero_max_life: hero.vitals.unwrap().max_life,
        hero_moving: hero.activity.as_ref().is_some_and(|a| a.moving()),
        hero_target: hero.combat.as_ref().and_then(|c| c.target),
        fighters_alive: squad.iter().filter(|id| **id != handle_from_hero).count(),
        control: objects.presentation().is_some_and(|state| {
            state.control_enabled == Some(true) && state.interface_visible == Some(true)
        }),
        entrance: objects.rectangle_bounds(entrance).unwrap(),
        entry_witness: objects.executed_units().any(|(_, unit)| {
            unit.owner == local.identity
                && unit.shown
                && unit.position.is_some_and(|point| {
                    [entrance, other_entrance]
                        .iter()
                        .any(|id| inside(point, objects.rectangle_bounds(*id).unwrap()))
                })
        }),
        orc_scene_complete,
        boss_region: objects.rectangle_bounds(boss_region).unwrap(),
        group_members: group.len(),
        group_alive: living.len(),
        visible_enemy,
        quest_completed,
        quest_items_complete,
        victory: objects
            .player_removal_state()
            .and_then(|state| state.get(local.identity))
            .is_some_and(|record| record.game_result == expected_result),
        exit_requested: {
            let requests = objects.game_exit_requests();
            if !requests.is_empty() {
                let Some(Value::Str(next)) = &authored_next else {
                    panic!("exit without authored next map: {authored_next:?}; {requests:?}")
                };
                assert!(!next.is_empty());
                assert!(
                    requests
                        .iter()
                        .all(|r| r.next_level.as_ref() == Some(next) && r.score_screen)
                );
                println!("Human01 authored exit: target={next:?} requests={requests:?}");
            }
            !requests.is_empty()
        },
    }
}

fn collision(objects: &mut ObjectHost, unit: HandleId) -> f32 {
    let Value::Real(radius) = objects
        .native("BlzGetUnitCollisionSize", &[Value::Handle(unit)])
        .expect("admitted collision query")
    else {
        panic!("collision query must return real");
    };
    assert!(
        radius.is_finite() && radius >= 0.0,
        "invalid collision radius {radius}"
    );
    radius
}

fn distance_between(a: [f32; 2], b: [f32; 2]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

fn army(objects: &ObjectHost) -> Vec<HandleId> {
    let local = objects
        .local_player_settings()
        .expect("local player")
        .identity;
    objects
        .executed_units()
        .filter(|(id, unit)| {
            unit.owner == local
                && unit.shown
                && unit.combat.as_ref().is_some_and(|c| c.weapon.is_some())
                && unit.vitals.is_some_and(|v| v.life > 0.0)
                && objects
                    .standard_command_order(*id, Kind::Move)
                    .unwrap()
                    .is_some()
                && objects
                    .standard_command_order(*id, Kind::Attack)
                    .unwrap()
                    .is_some()
        })
        .map(|(id, _)| id)
        .collect()
}

fn order(
    app: &mut App,
    kind: Kind,
    point: [f32; 2],
    target: Option<HandleId>,
    exclude_hero: bool,
) -> bool {
    order_with_units(app, kind, point, target, exclude_hero, |objects, _| {
        army(objects)
    })
}

fn order_with_units(
    app: &mut App,
    kind: Kind,
    point: [f32; 2],
    target: Option<HandleId>,
    exclude_hero: bool,
    select: impl Fn(&ObjectHost, HandleId) -> Vec<HandleId>,
) -> bool {
    app.world_mut().resource_scope(|world, mut input: Mut<crate::campaign_input::CampaignInput>| {
        let mut session = world.resource_mut::<MapScriptSession<MapStartupHost>>();
        let hero = handle(&session, "udg_Arthas");
        let objects = session.host().expect("published host").objects_mut();
        let local = objects.local_player_settings().unwrap().identity;
        let units: Vec<_> = select(objects, hero).into_iter().filter(|id| {
            if exclude_hero && *id == hero { return false; }
            let unit = objects.executed_unit(*id).unwrap();
            match (kind, target) {
                (Kind::Attack, Some(target)) => unit.combat.as_ref().unwrap().target != Some(target),
                (Kind::Move, Some(target)) => unit.activity.as_ref().unwrap().followed_unit() != Some(target),
                (Kind::Move, None) => unit.activity.as_ref().unwrap().destination() != Some(point),
                _ => true,
            }
        }).collect();
        if units.is_empty() { return false; }
        let VisionCapability::Ready { fog, mask } = objects.vision_capability() else { panic!("admitted squad vision") };
        for &id in &units {
            let unit = objects.executed_unit(id).unwrap();
            assert_eq!(objects.vision_presented(local, unit.position.unwrap(), fog, mask).unwrap(), Some(VisionState::Visible), "local squad unit must be visible");
            println!("Human01 squad order: id={id:?} rawcode={:?} owner={:?} position={:?} kind={kind:?} point={point:?} target={target:?}", unit.definition.rawcode, unit.owner, unit.position);
        }
        let expected = units.len();
        objects.select_units(&units, true);
        input.commands.record(objects.timer_game_time(), PlayerCommand::Select { units, replace: true }).unwrap();
        let before = input.commands.entries().len();
        crate::campaign_input::standard_target(objects, &mut input, kind, point, target).unwrap();
        assert!(input.last_error.is_none(), "{:?}", input.last_error);
        assert_eq!(input.commands.entries().len() - before, expected, "every selected unit must accept its intent");
        true
    })
}

fn hold_hero(app: &mut App) {
    app.world_mut().resource_scope(
        |world, mut input: Mut<crate::campaign_input::CampaignInput>| {
            let mut session = world.resource_mut::<MapScriptSession<MapStartupHost>>();
            let hero = handle(&session, "udg_Arthas");
            let witness = session.session_witness().unwrap();
            let host = session.host().unwrap();
            if host
                .objects()
                .executed_unit(hero)
                .unwrap()
                .activity
                .as_ref()
                .unwrap()
                .held()
            {
                return;
            }
            host.objects_mut().select_units(&[hero], true);
            input
                .commands
                .record(
                    host.objects().timer_game_time(),
                    PlayerCommand::Select {
                        units: vec![hero],
                        replace: true,
                    },
                )
                .unwrap();
            let before = input.commands.entries().len();
            crate::campaign_input::activate_standard(host, &mut input, hero, Kind::Hold, witness)
                .unwrap();
            assert!(input.last_error.is_none(), "{:?}", input.last_error);
            assert_eq!(
                input.commands.entries().len(),
                before + 1,
                "hero hold must be accepted"
            );
        },
    );
}

fn move_hero(app: &mut App, point: [f32; 2]) {
    app.world_mut().resource_scope(
        |world, mut input: Mut<crate::campaign_input::CampaignInput>| {
            let mut session = world.resource_mut::<MapScriptSession<MapStartupHost>>();
            let hero = handle(&session, "udg_Arthas");
            let objects = session.host().unwrap().objects_mut();
            let activity = objects
                .executed_unit(hero)
                .unwrap()
                .activity
                .as_ref()
                .unwrap();
            if activity.moving() && activity.destination() == Some(point) {
                return;
            }
            objects.select_units(&[hero], true);
            input
                .commands
                .record(
                    objects.timer_game_time(),
                    PlayerCommand::Select {
                        units: vec![hero],
                        replace: true,
                    },
                )
                .unwrap();
            let before = input.commands.entries().len();
            crate::campaign_input::standard_target(objects, &mut input, Kind::Move, point, None)
                .unwrap();
            assert!(input.last_error.is_none(), "{:?}", input.last_error);
            assert_eq!(
                input.commands.entries().len(),
                before + 1,
                "hero retreat must be accepted"
            );
        },
    );
}

fn healing_pending(app: &mut App, (target, mana): (HandleId, f32)) -> bool {
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let hero = handle(&session, "udg_Arthas");
    let objects = session.host().unwrap().objects();
    let target_alive = objects
        .executed_unit(target)
        .is_ok_and(|u| u.vitals.is_some_and(|v| v.life > 0.0));
    let readiness = objects
        .holy_light_readiness(hero)
        .unwrap()
        .expect("admitted learned Holy Light");
    // The canonical cast pays mana and starts cooldown together when its effect commits.
    target_alive
        && readiness.cooldown_remaining == 0.0
        && readiness.mana.is_some_and(|now| now >= mana)
}

fn heal_ally(app: &mut App) -> Option<(HandleId, f32)> {
    heal_with_units(app, |objects, _| army(objects))
}

fn heal_with_units(
    app: &mut App,
    select: impl Fn(&ObjectHost, HandleId) -> Vec<HandleId>,
) -> Option<(HandleId, f32)> {
    app.world_mut().resource_scope(|world, mut input: Mut<crate::campaign_input::CampaignInput>| {
        let mut session = world.resource_mut::<MapScriptSession<MapStartupHost>>();
        let hero = handle(&session, "udg_Arthas");
        let objects = session.host().unwrap().objects_mut();
        let command = objects.holy_light_command(hero).unwrap().expect("source-learned Holy Light must be admitted");
        let ready = objects.holy_light_readiness(hero).unwrap().expect("learned spell readiness");
        if !ready.ready() { return None; }
        let local = objects.local_player_settings().unwrap().identity;
        let VisionCapability::Ready { fog, mask } = objects.vision_capability() else { panic!("admitted spell target vision") };
        let range = objects.holy_light_cast_range(hero).unwrap().expect("admitted spell range");
        let hero_position = objects.executed_unit(hero).unwrap().position.unwrap();
        let squad = select(objects, hero);
        let target = objects.executed_units().filter(|(id, u)| {
            *id != hero && squad.contains(id) && u.owner == local && u.shown
                && u.vitals.is_some_and(|v| v.life > 0.0 && v.life < v.max_life * 0.5)
                && u.position.is_some_and(|point| distance_between(hero_position, point) <= range && objects.vision_presented(local, point, fog, mask).unwrap() == Some(VisionState::Visible))
        }).min_by(|(a, ua), (b, ub)| {
            let a_life = ua.vitals.unwrap(); let b_life = ub.vitals.unwrap();
            (a_life.life / a_life.max_life).total_cmp(&(b_life.life / b_life.max_life)).then_with(|| a.cmp(b))
        }).map(|(id, _)| id)?;
        let Value::Str(order) = objects.native("OrderId2String", &[Value::Int(command.order)]).unwrap() else { panic!("admitted spell order name") };
        objects.select_units(&[hero], true);
        input.commands.record(objects.timer_game_time(), PlayerCommand::Select { units: vec![hero], replace: true }).unwrap();
        // Same playable target-order native used by campaign_input's armed Holy Light command.
        let accepted = objects.native("IssueTargetOrderById", &[Value::Handle(hero), Value::Int(command.order), Value::Handle(target)]).unwrap();
        match accepted {
            Value::Bool(true) => {
                input.commands.record(objects.timer_game_time(), PlayerCommand::Target { unit: hero, order, target }).unwrap();
                println!("Human01 Holy Light accepted: clock={:?} target={target:?} readiness={ready:?}", objects.timer_game_time());
                Some((target, ready.mana.unwrap()))
            }
            Value::Bool(false) => { println!("Human01 Holy Light target refused by admitted rules: {target:?}"); None }
            other => panic!("target order returned {other:?}"),
        }
    })
}

fn skip_cinematic(app: &mut App) {
    app.world_mut().resource_scope(
        |world, mut input: Mut<crate::campaign_input::CampaignInput>| {
            let mut session = world.resource_mut::<MapScriptSession<MapStartupHost>>();
            let objects = session.host().expect("published host").objects_mut();
            let name = "EVENT_PLAYER_END_CINEMATIC";
            objects.fire_local_player_event(name).unwrap();
            input
                .commands
                .record(
                    objects.timer_game_time(),
                    PlayerCommand::LocalEvent { name: name.into() },
                )
                .unwrap();
        },
    );
}

fn continue_victory(app: &mut App) {
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let objects = session.host().expect("published host").objects_mut();
    let local = objects.local_player_settings().expect("local player");
    let player = local.identity;
    let number = local.number;
    let Value::Str(label) = objects
        .native(
            "GetLocalizedString",
            &[Value::Str("GAMEOVER_CONTINUE".into())],
        )
        .unwrap()
    else {
        panic!("victory Continue label is not admitted")
    };
    let buttons: Vec<_> = objects
        .dialog_state()
        .expect("admitted dialogs")
        .dialogs()
        .filter(|dialog| dialog.visible_to(number))
        .flat_map(|dialog| &dialog.buttons)
        .filter(|button| button.text == label)
        .map(|button| button.id)
        .collect();
    assert!(
        buttons.len() <= 1,
        "ambiguous source-labelled Continue button"
    );
    if let Some(button) = buttons.first() {
        objects.click_dialog_button(player, *button).unwrap();
        println!(
            "Human01 accepted Continue: clock={:?} button={button:?}",
            objects.timer_game_time()
        );
    } else {
        println!(
            "Human01 awaiting Continue: clock={:?} paused={:?} player={number} label={label:?} dialogs={:?}",
            objects.timer_game_time(),
            objects.game_paused(),
            objects
                .dialog_state()
                .unwrap()
                .dialogs()
                .collect::<Vec<_>>()
        );
    }
}

fn diagnostics(app: &mut App) {
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let hero = handle(&session, "udg_Arthas");
    let objects = session.host().expect("published host").objects_mut();
    let local = objects
        .local_player_settings()
        .expect("local player")
        .identity;
    println!(
        "Human01 source ranges: acquire={:?} holy_cast={:?}",
        objects
            .executed_unit(hero)
            .unwrap()
            .combat
            .as_ref()
            .and_then(|c| c.weapon)
            .map(|w| w.acquire),
        objects.holy_light_cast_range(hero).unwrap()
    );
    let hero_radius = collision(objects, hero);
    let hero_position = objects.executed_unit(hero).unwrap().position.unwrap();
    let units: Vec<_> = objects
        .executed_units()
        .filter(|(_, unit)| unit.owner == local)
        .map(|(id, _)| id)
        .collect();
    let random = format!("{:?}", objects.script_random_state());
    println!(
        "Human01 RNG checkpoint: fnv64={:016x}",
        pcx_script::fnv1a64(random.as_bytes())
    );
    for id in units {
        let current_order = objects
            .native("GetUnitCurrentOrder", &[Value::Handle(id)])
            .unwrap();
        let radius = collision(objects, id);
        let unit = objects.executed_unit(id).unwrap();
        let destination = unit.activity.as_ref().and_then(|a| a.destination());
        let follow = unit
            .activity
            .as_ref()
            .map(|a| (a.followed_unit(), a.follow_range()));
        println!(
            "Human01 follow checkpoint: unit={id:?} follow={follow:?} distance={} hero_radius={hero_radius} unit_radius={radius}",
            distance_between(hero_position, unit.position.unwrap())
        );
        let path = unit.activity.as_ref().map(|a| {
            let (points, next) = a.waypoints();
            (points.len(), next, points.get(next).copied())
        });
        if let Some(target) = unit.combat.as_ref().and_then(|c| c.target) {
            println!(
                "Human01 combat target identity: id={target:?} unit={:?}",
                objects.executed_unit(target).ok().map(|u| (
                    u.definition.rawcode,
                    u.owner,
                    u.position,
                    u.vitals
                ))
            );
        }
        println!(
            "Human01 unit checkpoint: unit={id:?} position={:?} vitals={:?} order={current_order:?} destination={destination:?} path={path:?} target={:?} attack_destination={:?} guard={:?}",
            unit.position,
            unit.vitals,
            unit.combat.as_ref().and_then(|c| c.target),
            unit.combat.as_ref().and_then(|c| c.attack_destination),
            objects.guard_state().and_then(|state| state.get(id))
        );
    }
}

#[cfg(feature = "test-fixtures")]
fn human02_military(objects: &ObjectHost, hero: HandleId, footman: i32) -> Vec<HandleId> {
    army(objects)
        .into_iter()
        .filter(|id| {
            *id == hero
                || objects.executed_unit(*id).unwrap().definition.rawcode == footman.to_be_bytes()
        })
        .collect()
}

#[cfg(feature = "test-fixtures")]
#[test]
#[ignore = "requires unchanged Human02 and private earned carryover via PCX_NATIVE_CHAPTER_*"]
fn selected_human02_route_reaches_authored_victory() {
    let (mut app, mut runtime) = human02_base();
    let footman = {
        let session = app.world().resource::<MapScriptSession<MapStartupHost>>();
        human02_source_kind(
            session,
            "Trig___FootmanPopsOut_Conditions",
            "GetTrainedUnit",
        )
    };
    let mut retained = None;
    let mut cast = None;
    let mut withdrawing = false;
    let mut retreat_goal = None;
    let mut last_threat = None;
    let mut journal_cursor = app
        .world()
        .resource::<crate::campaign_input::CampaignInput>()
        .commands
        .entries()
        .len();
    for batch in 0..900 {
        let (alive, clock, outro, victory, living_footmen) = {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let hero = handle(&session, "udg_Arthas");
            let expected = handle(&session, "PLAYER_GAME_RESULT_VICTORY");
            let outro = human02_authored_outro(&mut session);
            let objects = session.host().unwrap().objects();
            let local = objects.local_player_settings().unwrap().identity;
            (
                objects
                    .executed_unit(hero)
                    .is_ok_and(|u| u.vitals.is_some_and(|v| v.life > 0.)),
                objects.timer_game_time(),
                outro,
                objects
                    .player_removal_state()
                    .and_then(|state| state.get(local))
                    .is_some_and(|record| record.game_result == expected),
                human02_living_owned(objects, footman).len(),
            )
        };
        if victory || outro || !alive || living_footmen == 0 {
            human02_outcome_snapshot(&mut app, &runtime, footman);
        }
        if victory {
            assert!(
                outro,
                "player victory without earned authored Human02 outcome"
            );
            return;
        }
        if outro {
            skip_cinematic(&mut app);
            human02_advance_military(&mut app, &mut runtime, footman);
            continue;
        }
        if !alive || living_footmen == 0 {
            human02_military_diagnostics(&mut app, footman);
            panic!(
                "Human02 combat loss at tick {} clock {clock:?}: hero_alive={alive} living_footmen={living_footmen} withdrawing={withdrawing} goal={retreat_goal:?}",
                runtime.tick()
            );
        }
        let (hero, point, target, followers_ready, controlled, victory) = {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let hero = handle(&session, "udg_Arthas");
            let boss = handle(&session, "udg_Blademaster");
            let region = handle(&session, "gg_rct_BlademasterOrigin");
            let flags = [
                "udg_VictoryCondition01",
                "udg_VictoryCondition02",
                "udg_BlademasterHasBeenSlain",
            ]
            .map(|name| session.global(name) == Some(Value::Bool(true)));
            let expected = handle(&session, "PLAYER_GAME_RESULT_VICTORY");
            let objects = session.host().unwrap().objects();
            let local = objects.local_player_settings().unwrap();
            let result = objects
                .player_removal_state()
                .and_then(|state| state.get(local.identity));
            let victory = result.is_some_and(|record| record.game_result == expected);
            if victory {
                assert!(flags.into_iter().all(|flag| flag));
            }
            let squad = human02_military(objects, hero, footman);
            let unit = objects.executed_unit(hero).unwrap();
            let position = unit.position.unwrap();
            let acquire = unit.combat.as_ref().and_then(|c| c.weapon).unwrap().acquire;
            assert!(acquire.is_finite() && acquire > 0.);
            let VisionCapability::Ready { fog, mask } = objects.vision_capability() else {
                panic!("admitted tactical vision")
            };
            let threats: Vec<_> = objects
                .executed_units()
                .filter_map(|(id, enemy)| {
                    let point = enemy.position?;
                    if !enemy.shown
                        || !enemy.vitals.is_some_and(|v| v.life > 0.)
                        || !objects.is_enemy(hero, id)
                        || objects
                            .vision_presented(local.identity, point, fog, mask)
                            .unwrap()
                            != Some(VisionState::Visible)
                    {
                        return None;
                    }
                    let attacking = enemy
                        .combat
                        .as_ref()
                        .and_then(|c| c.target)
                        .is_some_and(|id| squad.contains(&id));
                    let gap = distance_between(position, point);
                    (gap <= acquire || attacking).then_some((
                        id,
                        attacking,
                        gap,
                        enemy.vitals.unwrap().life,
                    ))
                })
                .collect();
            let target = retained
                .filter(|id| threats.iter().any(|t| t.0 == *id))
                .or_else(|| {
                    threats
                        .iter()
                        .min_by(|a, b| {
                            b.1.cmp(&a.1)
                                .then_with(|| a.3.total_cmp(&b.3))
                                .then_with(|| a.2.total_cmp(&b.2))
                                .then_with(|| a.0.cmp(&b.0))
                        })
                        .map(|entry| entry.0)
                });
            if let Some(target) = target {
                last_threat = objects.executed_unit(target).unwrap().position;
            }
            let followers: Vec<_> = squad
                .iter()
                .filter(|id| **id != hero)
                .map(|id| objects.executed_unit(*id).unwrap().position.unwrap())
                .collect();
            let vitals = unit.vitals.unwrap();
            let support = human02_support_plan(
                vitals.life,
                vitals.max_life,
                withdrawing,
                position,
                &followers,
                last_threat,
            )
            .expect("finite live squad support inputs");
            if support.withdrawing {
                let moving = unit.activity.as_ref().is_some_and(|a| a.moving());
                let safe_goal = retreat_goal
                    .zip(last_threat)
                    .is_some_and(|(goal, threat)| human02_behind(position, goal, threat));
                if !withdrawing || !moving || !safe_goal {
                    retreat_goal = support.destination;
                }
            } else {
                retreat_goal = None;
            }
            withdrawing = support.withdrawing;
            let ready = squad.iter().filter(|id| **id != hero).all(|id| {
                let follower = objects.executed_unit(*id).unwrap();
                let activity = follower.activity.as_ref().unwrap();
                activity.follow_range().is_some_and(|range| {
                    human02_cohesive(
                        activity.followed_unit() == Some(hero),
                        distance_between(position, follower.position.unwrap()),
                        range,
                        acquire,
                    )
                })
            });
            if batch % 16 == 0 || victory {
                println!(
                    "Human02 military checkpoint: tick={} clock={:?} flags={flags:?} squad={squad:?} hero={:?} boss={:?} target={target:?} followers_ready={ready} withdrawing={withdrawing} retreat={retreat_goal:?} result={:?}",
                    runtime.tick(),
                    objects.timer_game_time(),
                    unit.vitals,
                    objects.executed_unit(boss).map(|u| (u.position, u.vitals)),
                    result
                );
            }
            (
                hero,
                center(objects.rectangle_bounds(region).unwrap()),
                target,
                ready,
                objects
                    .presentation()
                    .is_some_and(|p| p.control_enabled == Some(true)),
                victory,
            )
        };
        if batch % 16 == 0 || victory {
            human02_military_diagnostics(&mut app, footman);
        }
        if victory {
            return;
        }
        if controlled {
            let pending = cast.is_some_and(|value| healing_pending(&mut app, value));
            if !pending {
                cast = human02_heal(&mut app, footman);
            }
            let protect_cast = pending || cast.is_some();
            if let Some(target) = target {
                human02_military_order(
                    &mut app,
                    footman,
                    Kind::Attack,
                    point,
                    Some(target),
                    protect_cast || withdrawing,
                );
            } else {
                human02_military_order(&mut app, footman, Kind::Move, point, Some(hero), true);
                if !protect_cast && !withdrawing {
                    if followers_ready {
                        move_hero(&mut app, point);
                    } else {
                        hold_hero(&mut app);
                    }
                }
            }
            if withdrawing && !protect_cast {
                if let Some(goal) = retreat_goal {
                    move_hero(&mut app, goal);
                } else {
                    hold_hero(&mut app);
                }
            }
        } else {
            skip_cinematic(&mut app);
        }
        let entries = app
            .world()
            .resource::<crate::campaign_input::CampaignInput>()
            .commands
            .entries();
        if entries.len() != journal_cursor {
            println!(
                "Human02 military accepted commands: target={target:?} followers_ready={followers_ready} withdrawing={withdrawing} retreat={retreat_goal:?} commands={:?}",
                &entries[journal_cursor..]
            );
            journal_cursor = entries.len();
        }
        retained = target;
        human02_advance_military(&mut app, &mut runtime, footman);
    }
    diagnostics(&mut app);
    panic!(
        "Human02 military exhausted bounded canonical work at tick {}",
        runtime.tick()
    );
}

#[cfg(feature = "test-fixtures")]
fn human02_military_order(
    app: &mut App,
    footman: i32,
    kind: Kind,
    point: [f32; 2],
    target: Option<HandleId>,
    exclude_hero: bool,
) -> bool {
    order_with_units(app, kind, point, target, exclude_hero, |objects, hero| {
        human02_military(objects, hero, footman)
    })
}

#[cfg(feature = "test-fixtures")]
fn human02_heal(app: &mut App, footman: i32) -> Option<(HandleId, f32)> {
    heal_with_units(app, |objects, hero| {
        human02_military(objects, hero, footman)
    })
}

#[cfg(feature = "test-fixtures")]
#[test]
#[ignore = "requires unchanged Human02 and private earned carryover via PCX_NATIVE_CHAPTER_*"]
fn selected_human02_scene_diagnostic_normal() {
    human02_scene_diagnostic(false);
}

#[cfg(feature = "test-fixtures")]
#[test]
#[ignore = "requires unchanged Human02 and private earned carryover via PCX_NATIVE_CHAPTER_*"]
fn selected_human02_scene_diagnostic_cancel() {
    human02_scene_diagnostic(true);
}

#[cfg(feature = "test-fixtures")]
fn human02_scene_diagnostic(cancel: bool) {
    let (mut app, mut runtime) = human02_controlled();
    human02_workers_ready(&mut app, &mut runtime);
    let initial_game_pause = {
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let trigger = handle(&session, "gg_trg_Blademaster_Cinematic");
        let execute = session
            .context()
            .unwrap()
            .scripts
            .program()
            .function_id("ConditionalTriggerExecute")
            .unwrap();
        // Explicit diagnostic stimulus exercises the authored scene, not mission completion.
        session
            .call_function(execute, &[Value::Handle(trigger)])
            .unwrap();
        session.host().unwrap().objects().game_paused()
    };
    let mut image_witness = None;
    let mut cancel_sent = false;
    let mut previous_caster = None;
    for tick in 0..16384 {
        let (skippable, done) = {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let caster = session.global("udg_TempBlademaster");
            let image = session.global("udg_TempMirrorImage");
            let capture = handle(&session, "gg_trg_____SetMirrorImage");
            let cancel_trigger = handle(&session, "gg_trg_Blademaster_Cancel");
            let hero = handle(&session, "udg_Arthas");
            let quest = session.global("udg_BlademasterQuestRecieved") == Some(Value::Bool(true));
            let in_scene = session.global("udg_InCinematic");
            let objects = session.host().unwrap().objects_mut();
            if previous_caster != caster || tick % 64 == 0 {
                let caster_state = match caster {
                    Some(Value::Handle(id)) => {
                        let order = objects.native("GetUnitCurrentOrder", &[Value::Handle(id)]);
                        Some((
                            id,
                            objects
                                .executed_unit(id)
                                .map(|u| (u.definition.rawcode, u.owner, u.vitals, &u.abilities)),
                            order,
                        ))
                    }
                    _ => None,
                };
                println!(
                    "Human02 scene diagnostic: cancel={cancel} tick={} clock={:?} caster={caster_state:?} image={image:?} capture_enabled={:?} in_scene={in_scene:?}",
                    runtime.tick(),
                    objects.timer_game_time(),
                    objects.trigger_enabled(capture)
                );
                previous_caster = caster.clone();
            }
            if image_witness.is_none()
                && let Some(Value::Handle(id)) = image
            {
                let summoned = objects
                    .executed_unit(id)
                    .expect("captured summoned unit must exist");
                assert!(
                    summoned.illusion.is_some() && summoned.vitals.is_some_and(|v| v.life > 0.)
                );
                let Some(Value::Handle(caster)) = caster else {
                    panic!("image without source caster")
                };
                assert_ne!(id, caster);
                assert_eq!(summoned.owner, objects.executed_unit(caster).unwrap().owner);
                assert_eq!(objects.trigger_enabled(capture).unwrap(), Some(false));
                println!(
                    "Human02 positive source GetSummonedUnit capture: id={id:?} rawcode={:?} owner={:?} vitals={:?}",
                    summoned.definition.rawcode, summoned.owner, summoned.vitals
                );
                image_witness = Some(id);
            }
            let done = quest
                && in_scene == Some(Value::Bool(false))
                && objects
                    .presentation()
                    .is_some_and(|p| p.control_enabled == Some(true));
            if done {
                assert!(
                    image_witness.is_some(),
                    "scene returned without a live Mirror Image witness"
                );
                assert_eq!(objects.game_paused(), initial_game_pause);
                assert!(objects.executed_unit(hero).unwrap().paused_since.is_none());
            }
            (
                objects.trigger_enabled(cancel_trigger).unwrap() == Some(true),
                done,
            )
        };
        assert!(
            app.world()
                .resource::<peacecraft3::campaign_startup::CampaignStartup>()
                .last_error
                .is_none()
        );
        assert!(
            app.world()
                .resource::<crate::campaign_input::CampaignInput>()
                .last_error
                .is_none()
        );
        if done {
            assert!(!cancel || cancel_sent);
            println!(
                "Human02 diagnostic scene completed: cancel={cancel} image={image_witness:?} tick={}; not mission victory",
                runtime.tick()
            );
            return;
        }
        if cancel && !cancel_sent && skippable && image_witness.is_some() {
            skip_cinematic(&mut app);
            cancel_sent = true;
        }
        let result = {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            runtime.advance_ticks(&mut session, 1)
        };
        if let Err(error) = result {
            {
                let mut session = app
                    .world_mut()
                    .resource_mut::<MapScriptSession<MapStartupHost>>();
                let caster = session.global("udg_TempBlademaster");
                let image = session.global("udg_TempMirrorImage");
                let capture = handle(&session, "gg_trg_____SetMirrorImage");
                let objects = session.host().unwrap().objects_mut();
                println!(
                    "Human02 scene failure globals: clock={:?} caster={caster:?} image={image:?} capture_enabled={:?}",
                    objects.timer_game_time(),
                    objects.trigger_enabled(capture)
                );
                if let Some(Value::Handle(id)) = caster {
                    let order = objects.native("GetUnitCurrentOrder", &[Value::Handle(id)]);
                    println!(
                        "Human02 scene failure caster: id={id:?} order={order:?} unit={:?}",
                        objects.executed_unit(id).map(|u| (
                            u.definition.rawcode,
                            u.owner,
                            u.vitals,
                            &u.abilities
                        ))
                    );
                }
            }
            println!(
                "Human02 diagnostic scene stopped: cancel={cancel} completed_tick={} image={image_witness:?} error={error}",
                runtime.tick()
            );
            diagnostics(&mut app);
            panic!("{error}; failing tick may have partial effects");
        }
    }
    panic!(
        "Human02 diagnostic scene exhausted bounded canonical work: cancel={cancel} tick={} image={image_witness:?}",
        runtime.tick()
    );
}

#[cfg(feature = "test-fixtures")]
fn human02_cohesive(following: bool, gap: f32, follow_range: f32, acquire: f32) -> bool {
    // TAS travel policy uses the admitted tactical envelope even while followers move.
    following
        && gap.is_finite()
        && gap >= 0.0
        && follow_range.is_finite()
        && follow_range > 0.0
        && acquire.is_finite()
        && acquire > 0.0
        && (gap <= follow_range || gap <= acquire)
}

#[cfg(feature = "test-fixtures")]
#[test]
fn human02_cohesion_allows_active_follow_inside_tactical_range() {
    assert!(human02_cohesive(true, 48.0, 23.0, 67.0));
    assert!(!human02_cohesive(true, 68.0, 23.0, 67.0));
    assert!(!human02_cohesive(false, 12.0, 23.0, 67.0));
    for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(!human02_cohesive(true, invalid, 23.0, 67.0));
        assert!(!human02_cohesive(true, 12.0, invalid, 67.0));
        assert!(!human02_cohesive(true, 12.0, 23.0, invalid));
    }
    assert!(!human02_cohesive(true, -1.0, 23.0, 67.0));
    assert!(!human02_cohesive(true, 12.0, 0.0, 67.0));
    assert!(!human02_cohesive(true, 12.0, 23.0, 0.0));
}

#[cfg(feature = "test-fixtures")]
fn human02_military_diagnostics(app: &mut App, footman: i32) {
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let hero = handle(&session, "udg_Arthas");
    let objects = session.host().unwrap().objects_mut();
    let position = objects.executed_unit(hero).unwrap().position.unwrap();
    let mut squad = human02_military(objects, hero, footman);
    if !squad.contains(&hero) {
        squad.insert(0, hero);
    }
    for id in squad {
        let order = objects.native("GetUnitCurrentOrder", &[Value::Handle(id)]);
        let unit = objects.executed_unit(id).unwrap();
        let activity = unit.activity.as_ref().map(|activity| {
            let (points, next) = activity.waypoints();
            (
                activity.followed_unit(),
                activity.follow_range(),
                activity.destination(),
                activity.moving(),
                points.len(),
                next,
                points.get(next).copied(),
            )
        });
        println!(
            "Human02 squad checkpoint: id={id:?} position={:?} gap={} order={order:?} activity={activity:?} target={:?}",
            unit.position,
            distance_between(position, unit.position.unwrap()),
            unit.combat.as_ref().and_then(|c| c.target)
        );
    }
}

#[cfg(feature = "test-fixtures")]
#[derive(Debug, PartialEq)]
struct Human02SupportPlan {
    withdrawing: bool,
    destination: Option<[f32; 2]>,
}

#[cfg(feature = "test-fixtures")]
fn human02_behind(hero: [f32; 2], point: [f32; 2], threat: [f32; 2]) -> bool {
    let axis = [
        f64::from(threat[0]) - f64::from(hero[0]),
        f64::from(threat[1]) - f64::from(hero[1]),
    ];
    axis != [0.0, 0.0]
        && (f64::from(point[0]) - f64::from(hero[0])) * axis[0]
            + (f64::from(point[1]) - f64::from(hero[1])) * axis[1]
            <= 0.0
}

#[cfg(feature = "test-fixtures")]
fn human02_support_plan(
    life: f32,
    maximum: f32,
    withdrawing: bool,
    hero: [f32; 2],
    followers: &[[f32; 2]],
    threat: Option<[f32; 2]>,
) -> Option<Human02SupportPlan> {
    if !life.is_finite()
        || !maximum.is_finite()
        || life < 0.0
        || maximum <= 0.0
        || !hero.into_iter().all(f32::is_finite)
        || !followers.iter().flatten().all(|v| v.is_finite())
        || threat.is_some_and(|p| !p.into_iter().all(f32::is_finite))
    {
        return None;
    }
    // TAS strategy: withdraw at half life and remain in support until fully recovered.
    let withdrawing = if withdrawing {
        life < maximum
    } else {
        life <= maximum * 0.5
    };
    let destination = if withdrawing && !followers.is_empty() {
        let center = std::array::from_fn(|axis| {
            (followers.iter().map(|p| f64::from(p[axis])).sum::<f64>() / followers.len() as f64)
                as f32
        });
        threat.and_then(|threat| {
            if human02_behind(hero, center, threat) {
                return Some(center);
            }
            followers
                .iter()
                .copied()
                .filter(|p| human02_behind(hero, *p, threat))
                .max_by(|a, b| {
                    distance_between(*a, threat)
                        .total_cmp(&distance_between(*b, threat))
                        .then_with(|| a[0].total_cmp(&b[0]))
                        .then_with(|| a[1].total_cmp(&b[1]))
                })
        })
    } else {
        None
    };
    Some(Human02SupportPlan {
        withdrawing,
        destination,
    })
}

#[cfg(feature = "test-fixtures")]
#[test]
fn human02_support_uses_live_backline_and_persistent_health_policy() {
    let squad = [[4.0, 7.0], [8.0, 9.0]];
    let plan =
        human02_support_plan(19.0, 50.0, false, [12.0, 8.0], &squad, Some([40.0, 8.0])).unwrap();
    assert_eq!(
        plan,
        Human02SupportPlan {
            withdrawing: true,
            destination: Some([6.0, 8.0])
        }
    );
    assert_eq!(
        human02_support_plan(
            19.0,
            50.0,
            false,
            [0.0, 0.0],
            &[[-4.0, 3.0], [20.0, 1.0]],
            Some([40.0, 0.0])
        )
        .unwrap()
        .destination,
        Some([-4.0, 3.0])
    );
    assert!(
        human02_support_plan(39.0, 50.0, true, [12.0, 8.0], &squad, Some([40.0, 8.0]))
            .unwrap()
            .withdrawing
    );
    assert!(
        !human02_support_plan(50.0, 50.0, true, [12.0, 8.0], &squad, Some([40.0, 8.0]))
            .unwrap()
            .withdrawing
    );
    assert_eq!(
        human02_support_plan(19.0, 50.0, false, [0.0, 8.0], &squad, Some([40.0, 8.0]))
            .unwrap()
            .destination,
        None
    );
    assert_eq!(
        human02_support_plan(19.0, 50.0, false, [12.0, 8.0], &[], Some([40.0, 8.0]))
            .unwrap()
            .destination,
        None
    );
    assert_eq!(
        human02_support_plan(19.0, 50.0, false, [12.0, 8.0], &squad, None)
            .unwrap()
            .destination,
        None
    );
    assert!(
        human02_support_plan(
            f32::NAN,
            50.0,
            false,
            [12.0, 8.0],
            &squad,
            Some([40.0, 8.0])
        )
        .is_none()
    );
}

#[cfg(feature = "test-fixtures")]
fn human02_living_owned(objects: &ObjectHost, kind: i32) -> Vec<HandleId> {
    let local = objects.local_player_settings().unwrap().identity;
    objects
        .executed_units()
        .filter(|(_, unit)| {
            unit.owner == local
                && unit.definition.rawcode == kind.to_be_bytes()
                && unit.vitals.is_some_and(|v| v.life > 0.0)
        })
        .map(|(id, _)| id)
        .collect()
}

#[cfg(feature = "test-fixtures")]
fn human02_authored_outro(session: &mut MapScriptSession<MapStartupHost>) -> bool {
    let boss = handle(session, "udg_Blademaster");
    [
        "udg_VictoryCondition01",
        "udg_VictoryCondition02",
        "udg_BlademasterHasBeenSlain",
        "udg_GameOver",
    ]
    .into_iter()
    .all(|name| session.global(name) == Some(Value::Bool(true)))
        && session
            .host()
            .unwrap()
            .objects()
            .executed_unit(boss)
            .is_ok_and(|unit| unit.vitals.is_some_and(|v| v.life <= 0.0))
}

#[cfg(feature = "test-fixtures")]
fn human02_outcome_snapshot(app: &mut App, runtime: &CampaignRuntime, footman: i32) {
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let flags = [
        "udg_VictoryCondition01",
        "udg_VictoryCondition02",
        "udg_BlademasterHasBeenSlain",
        "udg_GameOver",
    ]
    .map(|name| session.global(name));
    let hero = handle(&session, "udg_Arthas");
    let boss = handle(&session, "udg_Blademaster");
    let objects = session.host().unwrap().objects();
    let local = objects.local_player_settings().unwrap().identity;
    let units: Vec<_> = objects
        .executed_units()
        .filter(|(id, unit)| {
            *id == hero || (unit.owner == local && unit.definition.rawcode == footman.to_be_bytes())
        })
        .map(|(id, unit)| (id, unit.owner, unit.vitals, unit.shown, unit.paused_since))
        .collect();
    println!(
        "Human02 outcome snapshot: tick={} clock={:?} flags={flags:?} boss={:?} result={:?} control={:?} units={units:?}",
        runtime.tick(),
        objects.timer_game_time(),
        objects
            .executed_unit(boss)
            .map(|unit| (unit.position, unit.vitals, unit.shown)),
        objects
            .player_removal_state()
            .and_then(|state| state.get(local)),
        objects.presentation().map(|p| p.control_enabled)
    );
}

#[cfg(feature = "test-fixtures")]
fn human02_advance_military(app: &mut App, runtime: &mut CampaignRuntime, footman: i32) {
    let result = {
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        runtime.advance_ticks(&mut session, 64)
    };
    if let Err(error) = result {
        human02_outcome_snapshot(app, runtime, footman);
        diagnostics(app);
        panic!(
            "Human02 military stopped at completed tick {}: {error}; failing tick may have partial effects",
            runtime.tick()
        );
    }
}

#[cfg(feature = "test-fixtures")]
#[test]
#[ignore = "requires unchanged Human02 and private earned carryover via PCX_NATIVE_CHAPTER_*"]
fn selected_human02_outro_diagnostic_normal() {
    human02_outro_diagnostic(false);
}

#[cfg(feature = "test-fixtures")]
#[test]
#[ignore = "requires unchanged Human02 and private earned carryover via PCX_NATIVE_CHAPTER_*"]
fn selected_human02_outro_diagnostic_cancel() {
    human02_outro_diagnostic(true);
}

#[cfg(feature = "test-fixtures")]
fn human02_outro_diagnostic(cancel: bool) {
    let (mut app, mut runtime) = human02_controlled();
    human02_workers_ready(&mut app, &mut runtime);
    let (worker, kind) = {
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let worker = handle(&session, "udg_Gold01");
        let trigger = handle(&session, "gg_trg_Victory_Queue");
        let execute = session
            .context()
            .unwrap()
            .scripts
            .program()
            .function_id("ConditionalTriggerExecute")
            .unwrap();
        let objects = session.host().unwrap().objects();
        let unit = objects.executed_unit(worker).unwrap();
        assert!(unit.shown && unit.vitals.is_some_and(|v| v.life > 0.0));
        assert_eq!(
            unit.owner,
            objects.local_player_settings().unwrap().identity
        );
        let kind = i32::from_be_bytes(unit.definition.rawcode);
        // Explicit diagnostic scene stimulus does not earn the mission's boss/base flags.
        session
            .call_function(execute, &[Value::Handle(trigger)])
            .unwrap();
        (worker, kind)
    };
    let mut hidden_witness = false;
    let mut cancel_sent = false;
    for _ in 0..16384 {
        let (victory, skippable) = {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let expected = handle(&session, "PLAYER_GAME_RESULT_VICTORY");
            let cancel_trigger = handle(&session, "gg_trg_Victory_Cinematic_Cancel");
            assert!(
                !human02_authored_outro(&mut session),
                "diagnostic must not fabricate earned mission flags"
            );
            let objects = session.host().unwrap().objects();
            let local = objects.local_player_settings().unwrap().identity;
            let unit = objects.executed_unit(worker).unwrap();
            if !unit.shown {
                assert!(human02_living_owned(objects, kind).contains(&worker));
                assert_eq!(unit.owner, local);
                if !hidden_witness {
                    println!(
                        "Human02 diagnostic hidden-live worker witness: id={worker:?} tick={} clock={:?} life={:?} pause={:?}",
                        runtime.tick(),
                        objects.timer_game_time(),
                        unit.vitals,
                        unit.paused_since
                    );
                }
                hidden_witness = true;
            }
            (
                objects
                    .player_removal_state()
                    .and_then(|state| state.get(local))
                    .is_some_and(|record| record.game_result == expected),
                objects.trigger_enabled(cancel_trigger).unwrap() == Some(true),
            )
        };
        assert!(
            app.world()
                .resource::<peacecraft3::campaign_startup::CampaignStartup>()
                .last_error
                .is_none()
        );
        assert!(
            app.world()
                .resource::<crate::campaign_input::CampaignInput>()
                .last_error
                .is_none()
        );
        if victory {
            assert!(hidden_witness && (!cancel || cancel_sent));
            println!(
                "Human02 diagnostic outro completed: cancel={cancel} tick={}; direct scene stimulus, NOT mission victory proof",
                runtime.tick()
            );
            return;
        }
        if cancel && !cancel_sent && skippable && hidden_witness {
            skip_cinematic(&mut app);
            cancel_sent = true;
        }
        let result = {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            runtime.advance_ticks(&mut session, 1)
        };
        if let Err(error) = result {
            human02_outcome_snapshot(&mut app, &runtime, kind);
            panic!(
                "Human02 diagnostic outro stopped at completed tick {}: {error}; failing tick may have partial effects",
                runtime.tick()
            );
        }
    }
    panic!(
        "Human02 diagnostic outro exhausted bounded canonical work: cancel={cancel} tick={}",
        runtime.tick()
    );
}

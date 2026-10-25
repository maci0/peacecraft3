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
    let mut issued = false;
    let mut target = None;
    let mut saw_victory_group = false;
    let mut last = String::new();
    for batch in 0..900 {
        let state = observe(&mut app);
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
            issued = false;
        }
        saw_victory_group |= state.group_members > 0;
        assert!(state.hero_life > 0.0, "Arthas died: {last}");
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
                return;
            }
            continue_victory(&mut app);
        } else if !state.control {
            skip_cinematic(&mut app);
            issued = false;
            target = None;
        } else {
            let next_target = if encounter_ready {
                state.visible_enemy
            } else {
                None
            };
            if !issued || next_target != target {
                let bounds = if encounter_ready {
                    state.boss_region
                } else {
                    state.entrance
                };
                order(
                    &mut app,
                    if next_target.is_some() {
                        Kind::Attack
                    } else {
                        Kind::Move
                    },
                    center(bounds),
                    next_target,
                );
                issued = true;
                target = next_target;
                println!(
                    "Human01 route order: {last}; entered={entered} encounter_ready={encounter_ready} target={target:?}"
                );
                diagnostics(&mut app);
            }
        }
        let result = {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            runtime.advance_ticks(&mut session, 64)
        };
        if let Err(error) = result {
            let failed = observe(&mut app);
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
    hero_position: [f32; 2],
    hero_life: f32,
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

fn observe(app: &mut App) -> Observation {
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let hero = handle(&session, "udg_Arthas");
    let group = handle(&session, "udg_WarlordAndCrewGroup");
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
    let objects = session.host().expect("published host").objects_mut();
    let group = members(objects, group);
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
    let visible_enemy = living.iter().copied().find(|id| {
        let unit = objects.executed_unit(*id).unwrap();
        if !unit.shown {
            return false;
        }
        let point = unit.position.expect("admitted unit position");
        match objects.vision_capability() {
            VisionCapability::Ready { fog, mask } => {
                objects
                    .vision_presented(local.identity, point, fog, mask)
                    .unwrap()
                    == Some(VisionState::Visible)
            }
            other => panic!("route target requires admitted vision: {other:?}"),
        }
    });
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
        hero_position: hero.position.expect("admitted hero position"),
        hero_life: hero.vitals.expect("admitted hero vitals").life,
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
        exit_requested: !objects.game_exit_requests().is_empty() && objects.next_level().is_some(),
    }
}

fn order(app: &mut App, kind: Kind, point: [f32; 2], target: Option<HandleId>) {
    app.world_mut().resource_scope(
        |world, mut input: Mut<crate::campaign_input::CampaignInput>| {
            let mut session = world.resource_mut::<MapScriptSession<MapStartupHost>>();
            let labels: Vec<_> = ["udg_Arthas", "udg_StartingFootman01", "udg_StartingFootman02", "udg_Footman05", "udg_Footman06"]
                .into_iter().map(|name| (handle(&session, name), name)).collect();
            let objects = session.host().expect("published host").objects_mut();
            let local = objects.local_player_settings().expect("local player").identity;
            let mut units = Vec::new();
            for (id, unit) in objects.executed_units().filter(|(_, unit)| unit.owner == local) {
                let reason = if !unit.shown {
                    Some("hidden")
                } else if unit.combat.as_ref().expect("admitted combat").dead {
                    Some("dead")
                } else if objects.standard_command_order(id, Kind::Move).unwrap().is_none() {
                    Some("no mobile command")
                } else if objects.standard_command_order(id, Kind::Attack).unwrap().is_none() {
                    Some("no attack command")
                } else {
                    let point = unit.position.expect("admitted army position");
                    match objects.vision_capability() {
                        VisionCapability::Ready { fog, mask } => {
                            (objects.vision_presented(local, point, fog, mask).unwrap() != Some(VisionState::Visible)).then_some("not visible")
                        }
                        other => panic!("army selection requires admitted vision: {other:?}"),
                    }
                };
                let label = labels.iter().find(|(handle, _)| *handle == id).map(|(_, name)| *name);
                println!("Human01 army inventory: unit={id:?} source_global={label:?} rawcode={:?} position={:?} excluded={reason:?}", unit.definition.rawcode, unit.position);
                if reason.is_none() {
                    units.push(id);
                }
            }
            assert!(!units.is_empty(), "no eligible local mobile army");
            println!("Human01 eligible army: clock={:?} units={units:?} point={point:?} target={target:?}", objects.timer_game_time());
            let expected_orders = units.len();
            objects.select_units(&units, true);
            input
                .commands
                .record(
                    objects.timer_game_time(),
                    PlayerCommand::Select {
                        units,
                        replace: true,
                    },
                )
                .unwrap();
            let before = input.commands.entries().len();
            crate::campaign_input::standard_target(
                objects,
                &mut input,
                kind,
                point,
                target,
            )
            .unwrap();
            assert!(input.last_error.is_none(), "{:?}", input.last_error);
            assert_eq!(input.commands.entries().len() - before, expected_orders, "every selected unit must accept its order");
        },
    );
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
    assert_eq!(
        buttons.len(),
        1,
        "one source-labelled visible Continue button"
    );
    objects.click_dialog_button(player, buttons[0]).unwrap();
}

fn diagnostics(app: &mut App) {
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let objects = session.host().expect("published host").objects_mut();
    let local = objects
        .local_player_settings()
        .expect("local player")
        .identity;
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
        let unit = objects.executed_unit(id).unwrap();
        let destination = unit.activity.as_ref().and_then(|a| a.destination());
        let path = unit.activity.as_ref().map(|a| {
            let (points, next) = a.waypoints();
            (points.len(), next, points.get(next).copied())
        });
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

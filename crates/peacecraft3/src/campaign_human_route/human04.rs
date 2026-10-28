//! Local-original Human04 route through public commands and the production clock.
//!
//! The mission opens with a town hall, farm and barracks. The player trains footmen at the
//! barracks, gathers the party at the town, then attack-moves the party to Kel'Thuzad; his
//! death runs the map's own ending and victory. The gather size and the order of steps are a
//! player policy, not authored rules. The party never casts Holy Light: the campaign AI's
//! target scorer refuses any candidate that ever carried an effect (spell art attaches one),
//! which is an open refusal, not a rule of the map; every unit, rectangle, trigger state and the victory
//! result are read from the unchanged running map.

use super::human03::{controlled, march_with};
use super::{
    army, continue_campaign, continue_victory, diagnostics, handle, human02_build_point,
    human02_button, human02_select, skip_cinematic,
};
use bevy::prelude::*;
use pcx_play::{
    map_startup_host::MapStartupHost, playercolor_host::ObjectHost, standard_command::Kind,
};
use pcx_script::{HandleId, Value};
use peacecraft3::map_script_session::MapScriptSession;

/// Simulation ticks between policy decisions (1 s at the 64 Hz campaign clock).
const DECISION_TICKS: usize = 64;
/// Application test work bound; not an authored game duration.
const DEFAULT_DECISIONS: usize = 3000;
/// Policy: trained footmen at the town before the party marches.
const GATHER_FOOTMEN: usize = 8;
/// Authored units whose rawcodes name the party's combat kinds besides the heroes.
const PARTY_KINDS: [&str; 3] = [
    "gg_unit_hfoo_0016",
    "gg_unit_hmpr_0070",
    "gg_unit_hmtm_0055",
];
const FOOTMAN_SAMPLE: &str = "gg_unit_hfoo_0016";
const RALLY: &str = "gg_rct_ArthasMove";
const KEL_THUZAD: &str = "udg_KelThuzad";
const QUESTS: [&str; 2] = ["udg_QuestAndorhal", "udg_QuestNecromancer"];
/// The authored tutorial trigger whose build order names the supply building.
const FARM_TRIGGER: &str = "Trig_PeasantFarm_Actions";
/// Policy: gold on hand before a blocked trainer asks a worker for a farm.
const FARM_GOLD: i32 = 200;
/// Set to also run the score-screen Continue into the next chapter after victory.
const CONTINUE_ENV: &str = "PCX_HUMAN04_CONTINUE";

struct Observation {
    clock: f64,
    control: bool,
    arthas_alive: bool,
    jaina_alive: bool,
    party: usize,
    footmen: usize,
    gold: i32,
    game_over: bool,
    kel_thuzad_dead: bool,
    /// Completion of the Andorhal and Kel'Thuzad quests.
    quests: [Option<bool>; 2],
    victory: bool,
    exit_requested: bool,
}

#[test]
#[ignore = "requires unchanged Human04 via PCX_NATIVE_CHAPTER_*, and a pinned private clone of the earned Human03 profile"]
fn selected_human04_route_reaches_authored_victory() {
    let (mut app, mut runtime) = controlled("Human04");
    let decisions = std::env::var("PCX_HUMAN04_DECISIONS").map_or(DEFAULT_DECISIONS, |value| {
        value.parse().expect("whole count")
    });
    let footman = kind_of(&mut app, FOOTMAN_SAMPLE);
    let kinds = PARTY_KINDS.map(|name| kind_of(&mut app, name));
    let mut marching = false;
    let farm = source_build_kind(&mut app, FARM_TRIGGER);
    let mut farming: Option<HandleId> = None;
    let mut last = String::new();
    let mut outcome_waits = 0;
    super::sim_bench::counters("human04-start");
    for batch in 0..decisions {
        let state = observe(&mut app, footman);
        last = format!(
            "tick={} clock={:.2} control={} party={} footmen={} gold={} marching={marching} game_over={} kel_thuzad_dead={} quests={:?} victory={}",
            runtime.tick(),
            state.clock,
            state.control,
            state.party,
            state.footmen,
            state.gold,
            state.game_over,
            state.kel_thuzad_dead,
            state.quests,
            state.victory
        );
        if batch % 8 == 0 {
            println!("Human04 checkpoint: {last}");
        }
        if batch % 32 == 0 {
            diagnostics(&mut app);
        }
        if state.victory {
            if state.exit_requested {
                println!("Human04 authored victory: {last}");
                assert!(state.game_over && state.kel_thuzad_dead, "{last}");
                if std::env::var_os(CONTINUE_ENV).is_some() {
                    continue_campaign(&mut app, runtime);
                }
                return;
            }
            outcome_waits += 1;
            assert!(
                outcome_waits <= 64,
                "victory dialog did not become ready: {last}"
            );
            continue_victory(&mut app);
        } else if !state.arthas_alive || !state.jaina_alive {
            diagnostics(&mut app);
            panic!("a hero died: {last}");
        } else if !state.control {
            skip_cinematic(&mut app);
        } else {
            if let Some(worker) = farming
                && !has_construction_order(&mut app, worker)
            {
                farming = None;
            }
            match train(&mut app, footman) {
                Training::Queued => {}
                Training::Declined(error) => {
                    if batch % 16 == 0 {
                        println!("Human04 training declined: {error}");
                    }
                }
                // Gold was available but nothing queued: the supply cap blocks it.
                Training::Silent if state.gold >= FARM_GOLD && farming.is_none() => {
                    farming = build_farm(&mut app, farm);
                    println!("Human04 supply blocked; farm worker {farming:?}: {last}");
                }
                Training::Silent => {}
            }
            marching |= state.footmen >= GATHER_FOOTMEN;
            let party = move |objects: &ObjectHost, id: HandleId| is_party(objects, id, &kinds);
            if marching {
                let target = target_unit(&mut app, KEL_THUZAD);
                march_with(&mut app, Kind::Attack, [0.0; 2], Some(target), party);
            } else {
                let point = rally_point(&mut app);
                march_with(&mut app, Kind::Attack, point, None, party);
            }
        }
        let result = {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            runtime.advance_ticks(&mut session, DECISION_TICKS)
        };
        if let Err(error) = result {
            diagnostics(&mut app);
            panic!("{error}; last_completed_observation={last}");
        }
    }
    if super::sim_bench::enabled() {
        super::sim_bench::finish("human04-end", &mut app, &last);
        return;
    }
    panic!("Human04 exhausted the decision budget: {last}");
}

pub(super) fn kind_of(app: &mut App, authored: &str) -> [u8; 4] {
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let unit = handle(&session, authored);
    session
        .host()
        .unwrap()
        .objects()
        .executed_unit(unit)
        .unwrap()
        .definition
        .rawcode
}

pub(super) fn target_unit(app: &mut App, name: &str) -> HandleId {
    handle(
        app.world().resource::<MapScriptSession<MapStartupHost>>(),
        name,
    )
}

fn rally_point(app: &mut App) -> [f32; 2] {
    super::human03::center(super::human03::region(app, RALLY))
}

/// Heroes and the authored combat kinds, never the workers.
pub(super) fn is_party(objects: &ObjectHost, id: HandleId, kinds: &[[u8; 4]]) -> bool {
    let unit = objects.executed_unit(id).unwrap();
    unit.abilities.as_ref().is_some_and(|a| a.hero.is_some())
        || kinds.contains(&unit.definition.rawcode)
}

fn observe(app: &mut App, footman: [u8; 4]) -> Observation {
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let arthas = handle(&session, "udg_Arthas");
    let jaina = handle(&session, "udg_Jaina");
    let kel_thuzad = handle(&session, KEL_THUZAD);
    let expected = handle(&session, "PLAYER_GAME_RESULT_VICTORY");
    let game_over = matches!(session.global("udg_GAMEOVER"), Some(Value::Bool(true)));
    let authored_next = session.global("bj_changeLevelMapName");
    let quest_globals = QUESTS.map(|name| session.global(name));
    let objects = session.host().expect("published host").objects_mut();
    let local = objects
        .local_player_settings()
        .expect("local player")
        .identity;
    let alive = |objects: &ObjectHost, id| {
        objects
            .executed_unit(id)
            .is_ok_and(|u| u.vitals.is_some_and(|v| v.life > 0.0))
    };
    let exit_requested = {
        let requests = objects.game_exit_requests();
        if !requests.is_empty() {
            let Some(Value::Str(next)) = &authored_next else {
                panic!("exit without authored next map: {authored_next:?}; {requests:?}")
            };
            assert!(
                requests
                    .iter()
                    .all(|r| r.next_level.as_ref() == Some(next) && r.score_screen)
            );
            println!("Human04 authored exit: target={next:?} requests={requests:?}");
        }
        !requests.is_empty()
    };
    Observation {
        clock: objects.timer_game_time().expect("admitted campaign clock"),
        control: objects.presentation().is_some_and(|state| {
            state.control_enabled == Some(true) && state.interface_visible == Some(true)
        }),
        arthas_alive: alive(objects, arthas),
        jaina_alive: alive(objects, jaina),
        party: army(objects).len(),
        footmen: objects
            .executed_units()
            .filter(|(id, u)| {
                u.owner == local && u.definition.rawcode == footman && alive(objects, *id)
            })
            .count(),
        gold: objects
            .player_resource_value(local, "PLAYER_STATE_RESOURCE_GOLD")
            .ok()
            .flatten()
            .unwrap_or(0),
        game_over,
        kel_thuzad_dead: !alive(objects, kel_thuzad),
        quests: quest_globals.map(|global| match global {
            Some(Value::Handle(id)) => Some(
                pcx_play::quest_view::project_quest(
                    objects.quest_session().expect("quest session"),
                    id,
                )
                .expect("authored quest")
                .completed,
            ),
            _ => None,
        }),
        victory: objects
            .player_removal_state()
            .and_then(|state| state.get(local))
            .is_some_and(|record| record.game_result == expected),
        exit_requested,
    }
}

pub(super) enum Training {
    Queued,
    Declined(String),
    /// The press was accepted but nothing entered the queue.
    Silent,
}

/// One public Train button press at the barracks.
pub(super) fn train(app: &mut App, footman: [u8; 4]) -> Training {
    use crate::campaign_command_card::{ButtonOrder, Kind as CardKind};
    use bevy::ui_widgets::{Activate, Button};
    let found = {
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let witness = session.session_witness().unwrap();
        let objects = session.host().unwrap().objects_mut();
        let owner = objects.local_player_settings().unwrap().identity;
        objects
            .executed_units()
            .filter(|(_, u)| u.owner == owner && u.shown)
            .find_map(|(id, unit)| {
                pcx_play::training_command::types(&unit.definition)
                    .ok()?
                    .contains(&footman)
                    .then_some(id)
            })
            .map(|producer| (producer, witness))
    };
    let Some((producer, witness)) = found else {
        return Training::Declined("no producer".into());
    };
    let queued = |app: &mut App| {
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        session
            .host()
            .unwrap()
            .objects()
            .unit_training_queue(producer)
            .map_or(0, |queue| queue.len())
    };
    let before = queued(app);
    human02_select(app, producer);
    app.world_mut()
        .resource_mut::<crate::button_actions::ButtonActions>()
        .entities
        .clear();
    let button = app
        .world_mut()
        .spawn((
            Button,
            ButtonOrder {
                unit: producer,
                kind: CardKind::Train(i32::from_be_bytes(footman)),
                session: witness,
                order: None,
            },
        ))
        .id();
    app.world_mut().trigger(Activate { entity: button });
    crate::campaign_input::run_command_button(app);
    app.world_mut().despawn(button);
    let error = app
        .world_mut()
        .resource_mut::<crate::campaign_input::CampaignInput>()
        .last_error
        .take();
    match error {
        Some(error) => Training::Declined(error),
        None if queued(app) > before => Training::Queued,
        None => Training::Silent,
    }
}

fn has_construction_order(app: &mut App, worker: HandleId) -> bool {
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    session
        .host()
        .unwrap()
        .objects()
        .unit_construction_order(worker)
        .is_ok_and(|order| order.is_some())
}

/// The rawcode the authored `function` passes to `IssueBuildOrderByIdLocBJ`.
fn source_build_kind(app: &mut App, function: &str) -> i32 {
    use pcx_script::ast::{Expr, Literal, Stmt};
    let session = app.world().resource::<MapScriptSession<MapStartupHost>>();
    let body = session
        .context()
        .unwrap()
        .scripts
        .program()
        .function(function)
        .unwrap()
        .body
        .as_deref()
        .unwrap();
    body.iter()
        .find_map(|statement| {
            let Stmt::Call { callee, args, .. } = statement else {
                return None;
            };
            if callee.name != "IssueBuildOrderByIdLocBJ" {
                return None;
            }
            let Some(Expr::Literal(Literal::Raw(raw), _)) = args.get(1) else {
                return None;
            };
            Some(i32::from_be_bytes(raw.text.as_bytes().try_into().ok()?))
        })
        .expect("the authored build order names its building")
}

/// Asks the first worker that can build `kind` to place it near itself.
fn build_farm(app: &mut App, kind: i32) -> Option<HandleId> {
    use crate::campaign_command_card::Kind as CardKind;
    let (worker, point) = {
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let objects = session.host().unwrap().objects_mut();
        let local = objects.local_player_settings().unwrap().identity;
        let worker = objects
            .executed_units()
            .filter(|(_, unit)| unit.owner == local && unit.shown)
            .find_map(|(id, _)| {
                objects
                    .unit_build_options(id)
                    .ok()?
                    .iter()
                    .any(|option| option.kind == kind && option.unavailable.is_none())
                    .then_some(id)
            })?;
        let point = human02_build_point(objects, worker, kind)?;
        (worker, point)
    };
    human02_select(app, worker);
    human02_button(app, worker, CardKind::BuildMenu);
    human02_button(app, worker, CardKind::Build(kind));
    let accepted = app.world_mut().resource_scope(
        |world, mut input: Mut<crate::campaign_input::CampaignInput>| {
            let mut session = world.resource_mut::<MapScriptSession<MapStartupHost>>();
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
    accepted.then_some(worker)
}

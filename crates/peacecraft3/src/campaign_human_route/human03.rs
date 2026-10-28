//! Local-original Human03 route through public commands and the production clock.
//!
//! The mission has no economy: the player marches the party (Arthas, Jaina and
//! the starting footmen) west and kills the Grain Warehouse. `ROUTE` is a player
//! policy, not an authored rule; every rectangle, unit, trigger state and the
//! victory result are read from the unchanged running map.

use super::{
    army, collision, continue_campaign, continue_victory, diagnostics, handle, heal_ally,
    healing_pending, skip_cinematic,
};
use bevy::prelude::*;
use pcx_play::{
    map_startup_host::MapStartupHost,
    player_command_log::{PlayerCommand, StartIdentity},
    standard_command::Kind,
};
use pcx_script::{HandleId, Value};
use peacecraft3::{
    campaign_runtime::CampaignRuntime, campaign_startup::CampaignSeed,
    map_script_session::MapScriptSession,
};
use std::{path::Path, time::Duration};

/// Simulation ticks between policy decisions (1 s at the 64 Hz campaign clock).
const DECISION_TICKS: usize = 64;
/// Application test work bound; not an authored game duration.
const DEFAULT_DECISIONS: usize = 2400;
/// Authored region globals the policy visits in order, then the warehouse.
const ROUTE: &[&str] = &[
    "gg_rct_VillagerConcernedArea",
    "gg_rct_Bandit",
    "gg_rct_VillagerActivateFountainQuest",
    "gg_rct_MurlocArea",
    "gg_rct_DefendZone",
    "gg_rct_HolyLightHintArea",
    "gg_rct_GrainArea",
    "gg_rct_VillageRaiseSkeletonStart",
    "gg_rct_PriestsStartArea",
    "gg_rct_MortarTeamArea",
    "gg_rct_Granary03Area",
];
const WAREHOUSE: &str = "gg_unit_ngwr_0074";
const QUESTS: [&str; 3] = [
    "udg_QuestInvestigateVillages",
    "udg_QuestDestroyGranary",
    "udg_QuestFountain",
];
/// Set to also run the score-screen Continue into the next chapter after victory.
const CONTINUE_ENV: &str = "PCX_HUMAN03_CONTINUE";

#[test]
#[ignore = "requires unchanged Human03 via PCX_NATIVE_CHAPTER_*, and a pinned private clone of the earned Human02 profile"]
fn selected_human03_route_reaches_authored_victory() {
    let (mut app, mut runtime) = controlled("Human03");
    let decisions = std::env::var("PCX_HUMAN03_DECISIONS").map_or(DEFAULT_DECISIONS, |value| {
        value.parse().expect("whole count")
    });
    let mut waypoint = 0;
    let mut cast = None;
    let mut recorded = 0;
    let mut last = String::new();
    let mut outcome_waits = 0;
    for batch in 0..decisions {
        let state = observe(&mut app, waypoint);
        last = format!(
            "commands={recorded} tick={} clock={:.2} control={} arthas={:?} life={} army={} waypoint={} game_over={} warehouse_dead={} quests={:?} victory={}",
            runtime.tick(),
            state.clock,
            state.control,
            state.arthas_position,
            state.arthas_life,
            state.army,
            state.waypoint,
            state.game_over,
            state.warehouse_dead,
            state.quests,
            state.victory
        );
        if batch % 8 == 0 {
            println!("Human03 checkpoint: {last}");
        }
        if batch % 32 == 0 {
            diagnostics(&mut app);
        }
        if state.victory {
            if state.exit_requested {
                println!("Human03 authored victory: {last}");
                assert!(
                    state.game_over,
                    "victory without the authored GameOver flag: {last}"
                );
                assert!(
                    state.warehouse_dead,
                    "victory without killing the Grain Warehouse: {last}"
                );
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
            if let Some(&name) = ROUTE.get(waypoint) {
                let bounds = region(&mut app, name);
                if touches(state.arthas_position, bounds, state.arthas_radius) {
                    println!("Human03 reached {name}: {last}");
                    waypoint += 1;
                } else {
                    march(&mut app, Kind::Attack, center(bounds), None);
                }
            } else {
                let warehouse = target_unit(&mut app, WAREHOUSE);
                march(
                    &mut app,
                    Kind::Attack,
                    state.arthas_position,
                    Some(warehouse),
                );
            }
            if !cast.is_some_and(|value| healing_pending(&mut app, value)) {
                cast = heal_ally(&mut app);
            }
        }
        {
            let input = app
                .world()
                .resource::<crate::campaign_input::CampaignInput>();
            assert!(input.last_error.is_none(), "{:?}; {last}", input.last_error);
            recorded = input.commands.entries().len();
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
    panic!("Human03 exhausted the decision budget: {last}");
}

/// The hero's collision circle overlaps the rectangle.
pub(super) fn touches([x, y]: [f32; 2], [x0, y0, x1, y1]: [f32; 4], radius: f32) -> bool {
    (x - x.clamp(x0, x1)).hypot(y - y.clamp(y0, y1)) <= radius
}

pub(super) fn center([x0, y0, x1, y1]: [f32; 4]) -> [f32; 2] {
    [(x0 + x1) * 0.5, (y0 + y1) * 0.5]
}

pub(super) fn region(app: &mut App, name: &str) -> [f32; 4] {
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let id = handle(&session, name);
    session
        .host()
        .unwrap()
        .objects()
        .rectangle_bounds(id)
        .unwrap()
}

fn target_unit(app: &mut App, name: &str) -> HandleId {
    let session = app.world().resource::<MapScriptSession<MapStartupHost>>();
    handle(session, name)
}

struct Observation {
    clock: f64,
    control: bool,
    arthas_position: [f32; 2],
    arthas_life: f32,
    arthas_radius: f32,
    arthas_alive: bool,
    jaina_alive: bool,
    army: usize,
    waypoint: usize,
    game_over: bool,
    warehouse_dead: bool,
    /// Completion of the main, grain and fountain quests, in that order.
    quests: [Option<bool>; 3],
    victory: bool,
    exit_requested: bool,
}

fn observe(app: &mut App, waypoint: usize) -> Observation {
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let arthas = handle(&session, "udg_Arthas");
    let jaina = handle(&session, "udg_Jaina");
    let warehouse = handle(&session, WAREHOUSE);
    let expected = handle(&session, "PLAYER_GAME_RESULT_VICTORY");
    let game_over = matches!(session.global("udg_GameOver"), Some(Value::Bool(true)));
    let authored_next = session.global("bj_changeLevelMapName");
    let quest_globals = QUESTS.map(|name| session.global(name));
    let objects = session.host().expect("published host").objects_mut();
    let local = objects
        .local_player_settings()
        .expect("local player")
        .identity;
    let alive = |objects: &pcx_play::playercolor_host::ObjectHost, id| {
        objects
            .executed_unit(id)
            .is_ok_and(|u| u.vitals.is_some_and(|v| v.life > 0.0))
    };
    let hero = objects.executed_unit(arthas).unwrap();
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
            println!("Human03 authored exit: target={next:?} requests={requests:?}");
        }
        !requests.is_empty()
    };
    Observation {
        clock: objects.timer_game_time().expect("admitted campaign clock"),
        control: objects.presentation().is_some_and(|state| {
            state.control_enabled == Some(true) && state.interface_visible == Some(true)
        }),
        arthas_position: hero.position.expect("admitted hero position"),
        arthas_life: hero.vitals.expect("admitted hero vitals").life,
        arthas_radius: collision(objects, arthas),
        arthas_alive: alive(objects, arthas),
        jaina_alive: alive(objects, jaina),
        army: army(objects).len(),
        waypoint,
        game_over,
        warehouse_dead: !alive(objects, warehouse),
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

/// Selects the party members not already carrying this intent and issues one
/// public attack command (ground attack-move, or a target when given).
pub(super) fn march(app: &mut App, kind: Kind, point: [f32; 2], target: Option<HandleId>) -> bool {
    march_with(app, kind, point, target, |_, _| true)
}

/// [`march`] for the party members `include` accepts.
pub(super) fn march_with(
    app: &mut App,
    kind: Kind,
    point: [f32; 2],
    target: Option<HandleId>,
    include: impl Fn(&pcx_play::playercolor_host::ObjectHost, HandleId) -> bool,
) -> bool {
    app.world_mut().resource_scope(
        |world, mut input: Mut<crate::campaign_input::CampaignInput>| {
            let mut session = world.resource_mut::<MapScriptSession<MapStartupHost>>();
            let objects = session.host().expect("published host").objects_mut();
            let units: Vec<_> = army(objects)
                .into_iter()
                .filter(|id| {
                    if !include(objects, *id) {
                        return false;
                    }
                    let combat = objects.executed_unit(*id).unwrap().combat.as_ref().unwrap();
                    match target {
                        Some(target) => combat.target != Some(target),
                        None => combat.attack_destination != Some(point),
                    }
                })
                .collect();
            if units.is_empty() {
                return false;
            }
            let expected = units.len();
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
            crate::campaign_input::standard_target(objects, &mut input, kind, point, target)
                .unwrap();
            assert!(input.last_error.is_none(), "{:?}", input.last_error);
            let accepted = input.commands.entries().len() - before;
            if accepted != expected {
                // The game may refuse an order (for example against a unit that just died).
                let victim = target.and_then(|id| objects.executed_unit(id).ok());
                println!(
                    "Human03 orders accepted {accepted} of {expected}: kind={kind:?} target_vitals={:?} invulnerable={:?}",
                    victim.and_then(|u| u.vitals),
                    victim.map(|u| u.invulnerable)
                );
            }
            true
        },
    )
}

/// Starts the selected Human03 with the pinned earned profile and runs the
/// authored opening until the player has control.
pub(super) fn controlled(chapter: &str) -> (App, CampaignRuntime) {
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
    use sha2::Digest;
    let expected =
        std::env::var("PCX_NATIVE_CAMPAIGN_CACHE_SHA256").expect("pin the earned input cache");
    assert_eq!(expected.len(), 64);
    let bytes = std::fs::read(profile.join("peacecraft3/campaign-cache.bin"))
        .expect("the chapter requires earned carryover");
    assert_eq!(
        sha2::Sha256::digest(&bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
        expected.to_ascii_lowercase()
    );
    println!(
        "{chapter} earned input cache: sha256={expected} bytes={}",
        bytes.len()
    );
    assert!(
        std::env::var("PCX_NATIVE_CHAPTER_MAP")
            .unwrap()
            .rsplit(['\\', '/'])
            .next()
            .unwrap()
            .eq_ignore_ascii_case(&format!("{chapter}.w3m"))
    );
    let mut app = crate::native_campaign::tests::started_app();
    let identity = StartIdentity::from_context(
        app.world().resource::<CampaignSeed>().prepared.unwrap(),
        app.world()
            .resource::<MapScriptSession<MapStartupHost>>()
            .context()
            .unwrap(),
    );
    println!("{chapter} start identity: {identity:?}");
    app.world_mut()
        .resource_mut::<crate::campaign_input::CampaignInput>()
        .commands
        .begin(identity);
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
        assert!(
            session
                .host()
                .unwrap()
                .objects()
                .executed_unit(hero)
                .unwrap()
                .cached,
            "default hero is not carryover"
        );
        assert_eq!(runtime.advance_ticks(&mut session, 0).unwrap(), 0);
    }
    for _ in 0..240 {
        let controlled = {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            session
                .host()
                .unwrap()
                .objects()
                .presentation()
                .is_some_and(|p| {
                    p.control_enabled == Some(true) && p.interface_visible == Some(true)
                })
        };
        if controlled {
            println!(
                "{chapter} authored control reached at tick={}",
                runtime.tick()
            );
            return (app, runtime);
        }
        skip_cinematic(&mut app);
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        runtime.advance_ticks(&mut session, DECISION_TICKS).unwrap();
    }
    panic!("{chapter} did not restore control within bounded canonical work");
}

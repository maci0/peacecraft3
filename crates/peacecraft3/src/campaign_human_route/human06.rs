//! Human06, The Culling. The player policy skips the intro, then attack-moves
//! the living army at the nearest living zombie (`nzom`). Victory is the map's own result:
//! `SetNextLevelBJ("Maps\\Campaign\\Human06Interlude.w3m")` and
//! `CustomVictoryBJ` with the dialog and scores flags set.

use super::human03::march;
use super::skip_cinematic;
use bevy::prelude::*;
use pcx_play::{
    map_startup_host::MapStartupHost, player_command_log::StartIdentity, standard_command::Kind,
};
use pcx_script::{HandleId, Value};
use peacecraft3::{
    campaign_runtime::CampaignRuntime, campaign_startup::CampaignSeed,
    map_script_session::MapScriptSession,
};
use std::{path::Path, time::Duration};

const DECISION_TICKS: usize = 64;
const DEFAULT_DECISIONS: usize = 2400;
const NEXT: &str = "Maps\\Campaign\\Human06Interlude.w3m";

#[test]
#[ignore = "requires unchanged Human06 via PCX_NATIVE_CHAPTER_*, and a pinned private clone of the earned Human05 profile"]
fn selected_human06_route_reaches_authored_victory() {
    let (mut app, mut runtime) = started("Human06");
    let decisions = std::env::var("PCX_HUMAN06_DECISIONS").map_or(DEFAULT_DECISIONS, |value| {
        value.parse().expect("whole count")
    });
    let mut last = String::new();
    let mut outcome_waits = 0;
    for batch in 0..decisions {
        let state = observe(&mut app);
        last = format!(
            "tick={} clock={:.2} control={} intro={:?} army={} hostiles={} zombies={} zombie_range={:?} denials={:?} conversions={:?} game_over={:?} victory={} exit={}",
            runtime.tick(),
            state.clock,
            state.control,
            state.intro_cancelled,
            state.army,
            state.hostiles,
            state.zombies,
            state.zombie_range,
            state.denials,
            state.conversions,
            state.game_over,
            state.victory,
            state.exit
        );
        if batch % 16 == 0 {
            println!("Human06 checkpoint: {last}");
        }
        if state.exit {
            println!("Human06 authored victory: {last}");
            assert!(state.victory, "{last}");
            assert_eq!(state.game_over, Some(Value::Bool(true)), "{last}");
            return;
        }
        if state.game_over == Some(Value::Bool(true)) && !state.victory {
            panic!("Human06 defeat: {last}");
        }
        if !state.control {
            skip_cinematic(&mut app);
        } else if let Some(point) = state.threat {
            march(&mut app, Kind::Attack, point, None);
        }
        let result = {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            runtime.advance_ticks(&mut session, DECISION_TICKS)
        };
        if let Err(error) = result {
            println!("Human06 advance failed: {error}; last_completed_observation={last}");
            panic!("{error}; last_completed_observation={last}");
        }
        if state.victory {
            outcome_waits += 1;
            assert!(
                outcome_waits <= 64,
                "victory dialog did not become ready: {last}"
            );
        }
    }
    panic!("Human06 exhausted the decision budget: {last}");
}

fn started(chapter: &str) -> (App, CampaignRuntime) {
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
        assert_eq!(runtime.advance_ticks(&mut session, 0).unwrap(), 0);
    }
    (app, runtime)
}

struct Observation {
    clock: f64,
    control: bool,
    intro_cancelled: Option<Value>,
    army: usize,
    hostiles: usize,
    zombies: usize,
    zombie_range: Option<f32>,
    denials: Option<Value>,
    conversions: Option<Value>,
    threat: Option<[f32; 2]>,
    game_over: Option<Value>,
    victory: bool,
    exit: bool,
}

fn observe(app: &mut App) -> Observation {
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let expected = super::handle(&session, "PLAYER_GAME_RESULT_VICTORY");
    let intro_cancelled = session.global("udg_IntroCancel");
    let game_over = session.global("udg_GameOver");
    let denials = session.global("udg_ArthasDenials");
    let conversions = session.global("udg_MalganisConversions");
    let authored_next = session.global("bj_changeLevelMapName");
    let arthas = match session.global("udg_ArthasVariable") {
        Some(Value::Handle(id)) => Some(id),
        _ => None,
    };
    let objects = session.host().expect("published host").objects_mut();
    let local = objects.local_player_settings().expect("local player");
    let army = super::army(objects);
    let origin = arthas
        .and_then(|id| {
            objects
                .executed_unit(id)
                .ok()
                .and_then(|unit| unit.position)
        })
        .or_else(|| {
            army.first().and_then(|id| {
                objects
                    .executed_unit(*id)
                    .ok()
                    .and_then(|unit| unit.position)
            })
        });
    let start = local.start_location.and_then(|index| {
        objects
            .map_config()
            .and_then(|config| config.start_locations.iter().find(|row| row.index == index))
            .map(|row| [row.x, row.y])
    });
    // The culling is around the player's start. A centroid of every immovable
    // unit sat thousands away from those zombies.
    let home = start.or(origin);
    let mut zombies = 0usize;
    let mut zombie_range = None;
    let hostiles: Vec<(u8, HandleId, [f32; 2])> = objects
        .executed_units()
        .filter_map(|(id, unit)| {
            if unit.owner == local.identity || !unit.shown {
                return None;
            }
            let life = unit.vitals?;
            if life.life <= 0.0 {
                return None;
            }
            let point = unit.position?;
            let zombie = unit.definition.rawcode == *b"nzom";
            let range = home.map(|home| super::distance_between(home, point));
            if zombie {
                zombies += 1;
                if range.is_some_and(|range| zombie_range.is_none_or(|best| range < best)) {
                    zombie_range = range;
                }
            }
            // Street zombies sit outside a tight town radius. 8000 reaches them
            // without marching at the undead base. Other armed units are left
            // alone; that wave wiped the army before any denial.
            if zombie && range.is_some_and(|range| range <= 3000.0) {
                Some((0, id, point))
            } else {
                None
            }
        })
        .collect();
    let threat = home.and_then(|home| {
        hostiles
            .iter()
            .min_by(|a, b| {
                a.0.cmp(&b.0)
                    .then_with(|| {
                        super::distance_between(home, a.2)
                            .total_cmp(&super::distance_between(home, b.2))
                    })
                    .then_with(|| a.2[0].total_cmp(&b.2[0]).then(a.2[1].total_cmp(&b.2[1])))
            })
            .map(|(_, _, point)| *point)
    });
    let exit = {
        let requests = objects.game_exit_requests();
        if !requests.is_empty() {
            assert!(
                requests.iter().all(|request| {
                    request.next_level.as_deref() == Some(NEXT) && request.score_screen
                }),
                "authored exit is ChangeLevel({NEXT:?}, true): {requests:?} next={authored_next:?}"
            );
            println!("Human06 authored exit: requests={requests:?}");
        }
        !requests.is_empty()
    };
    Observation {
        clock: objects.timer_game_time().expect("admitted campaign clock"),
        control: objects.presentation().is_some_and(|state| {
            state.control_enabled == Some(true) && state.interface_visible == Some(true)
        }),
        intro_cancelled,
        army: army.len(),
        hostiles: hostiles.len(),
        zombies,
        zombie_range,
        denials,
        conversions,
        threat,
        game_over,
        victory: objects
            .player_removal_state()
            .and_then(|state| state.get(local.identity))
            .is_some_and(|record| record.game_result == expected),
        exit,
    }
}

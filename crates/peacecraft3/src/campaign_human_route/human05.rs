//! Local-original Human05 route through public commands and the production clock.
//!
//! The mission is a 30-minute defense of Hearthglen: the map's own timer expiry (or the loss of
//! every enemy structure) runs its ending and victory. The policy keeps training footmen at the
//! player's trainer and holds the party at the town keep; it is a player policy, not an authored
//! rule. Units, positions and the victory result are read from the unchanged running map. The
//! party never casts spells (see `campaign-human04.md`, the open AI target scorer refusal).

use super::human03::{controlled, march_with};
use super::human04::{Training, is_party, kind_of, train};
use super::{continue_campaign, continue_victory, diagnostics, handle, skip_cinematic};
use bevy::prelude::*;
use pcx_play::{
    map_startup_host::MapStartupHost, playercolor_host::ObjectHost, standard_command::Kind,
};
use pcx_script::{HandleId, Value};
use peacecraft3::map_script_session::MapScriptSession;

/// Simulation ticks between policy decisions (1 s at the 64 Hz campaign clock).
const DECISION_TICKS: usize = 64;
/// Application test work bound; not an authored game duration.
const DEFAULT_DECISIONS: usize = 2400;
/// Authored units whose rawcodes name the party's combat kinds besides the heroes.
const PARTY_KINDS: [&str; 3] = [
    "gg_unit_hfoo_0024",
    "gg_unit_hrif_0026",
    "gg_unit_hmpr_0028",
];
const FOOTMAN_SAMPLE: &str = "gg_unit_hfoo_0024";
/// The town keep the party holds around.
const KEEP: &str = "gg_unit_hkee_0043";
/// Set to also run the score-screen Continue into the next chapter after victory.
const CONTINUE_ENV: &str = "PCX_HUMAN05_CONTINUE";

struct Observation {
    clock: f64,
    control: bool,
    arthas_alive: bool,
    jaina_alive: bool,
    party: usize,
    footmen: usize,
    gold: i32,
    game_over: bool,
    intro_cancelled: Option<Value>,
    victory: bool,
    exit_requested: bool,
}

#[test]
#[ignore = "requires unchanged Human05 via PCX_NATIVE_CHAPTER_*, and a pinned private clone of the earned Human04 profile"]
fn selected_human05_route_reaches_authored_victory() {
    let (mut app, mut runtime) = controlled("Human05");
    let decisions = std::env::var("PCX_HUMAN05_DECISIONS").map_or(DEFAULT_DECISIONS, |value| {
        value.parse().expect("whole count")
    });
    let footman = kind_of(&mut app, FOOTMAN_SAMPLE);
    let kinds = PARTY_KINDS.map(|name| kind_of(&mut app, name));
    let mut last = String::new();
    let mut outcome_waits = 0;
    for batch in 0..decisions {
        let state = observe(&mut app, footman);
        last = format!(
            "tick={} clock={:.2} control={} party={} footmen={} gold={} game_over={} intro_cancelled={:?} victory={}",
            runtime.tick(),
            state.clock,
            state.control,
            state.party,
            state.footmen,
            state.gold,
            state.game_over,
            state.intro_cancelled,
            state.victory
        );
        if batch % 16 == 0 {
            println!("Human05 checkpoint: {last}");
        }
        if batch % 64 == 0 {
            diagnostics(&mut app);
        }
        if state.victory {
            if state.exit_requested {
                println!("Human05 authored victory: {last}");
                assert!(state.game_over, "{last}");
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
            if let Training::Declined(error) = train(&mut app, footman)
                && batch % 32 == 0
            {
                println!("Human05 training declined: {error}");
            }
            let point = keep_position(&mut app);
            let party = move |objects: &ObjectHost, id: HandleId| is_party(objects, id, &kinds);
            march_with(&mut app, Kind::Attack, point, None, party);
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
    panic!("Human05 exhausted the decision budget: {last}");
}

fn keep_position(app: &mut App) -> [f32; 2] {
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let keep = handle(&session, KEEP);
    session
        .host()
        .unwrap()
        .objects()
        .executed_unit(keep)
        .unwrap()
        .position
        .expect("the keep's accepted position")
}

fn observe(app: &mut App, footman: [u8; 4]) -> Observation {
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let arthas = handle(&session, "udg_Arthas");
    let jaina = handle(&session, "udg_Jaina");
    let expected = handle(&session, "PLAYER_GAME_RESULT_VICTORY");
    let game_over = matches!(session.global("udg_GAMEOVER"), Some(Value::Bool(true)));
    let authored_next = session.global("bj_changeLevelMapName");
    let intro_cancelled = session.global("udg_IntroCancelled");
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
            println!("Human05 authored exit: target={next:?} requests={requests:?}");
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
        party: super::army(objects).len(),
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
        intro_cancelled,
        victory: objects
            .player_removal_state()
            .and_then(|state| state.get(local))
            .is_some_and(|record| record.game_result == expected),
        exit_requested,
    }
}

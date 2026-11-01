//! Human05 interlude: a cinematic. Ending it is the authored escape
//! (`TriggerRegisterPlayerEventEndCinematic`), which runs `VICTORY`:
//! `SetNextLevelBJ("Maps\\Campaign\\Human06.w3m")` and
//! `CustomVictoryBJ(udg_ThePlayer, false, false)`.

use super::{diagnostics, skip_cinematic};
use bevy::prelude::*;
use pcx_play::{map_startup_host::MapStartupHost, player_command_log::StartIdentity};
use pcx_script::Value;
use peacecraft3::{
    campaign_runtime::CampaignRuntime, campaign_startup::CampaignSeed,
    map_script_session::MapScriptSession,
};
use std::{path::Path, time::Duration};

const DECISION_TICKS: usize = 64;
const NEXT: &str = "Maps\\Campaign\\Human06.w3m";

#[test]
#[ignore = "requires unchanged Human05Interlude via PCX_NATIVE_CHAPTER_*, and a pinned private clone of the earned Human05 profile"]
fn selected_human05interlude_route_reaches_authored_victory() {
    let (mut app, mut runtime) = started();
    let mut last = String::new();
    for batch in 0..64 {
        let state = observe(&mut app);
        last = format!(
            "tick={} clock={:.2} control={} escape={:?} game_over={:?} victory={} exit={}",
            runtime.tick(),
            state.clock,
            state.control,
            state.escape,
            state.game_over,
            state.victory,
            state.exit
        );
        if batch % 4 == 0 {
            println!("Human05Interlude checkpoint: {last}");
        }
        if state.exit {
            println!("Human05Interlude authored victory: {last}");
            assert!(state.victory, "{last}");
            return;
        }
        if !state.control {
            skip_cinematic(&mut app);
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
    panic!("Human05Interlude exhausted the decision budget: {last}");
}

fn started() -> (App, CampaignRuntime) {
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
        .expect("the interlude requires the earned Human05 cache");
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
            .eq_ignore_ascii_case("Human05Interlude.w3m")
    );
    let mut app = crate::native_campaign::tests::started_app();
    let identity = StartIdentity::from_context(
        app.world().resource::<CampaignSeed>().prepared.unwrap(),
        app.world()
            .resource::<MapScriptSession<MapStartupHost>>()
            .context()
            .unwrap(),
    );
    println!("Human05Interlude start identity: {identity:?}");
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
    escape: Option<Value>,
    game_over: Option<Value>,
    victory: bool,
    exit: bool,
}

fn observe(app: &mut App) -> Observation {
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let expected = super::handle(&session, "PLAYER_GAME_RESULT_VICTORY");
    let escape = session.global("udg_Escape");
    let game_over = session.global("udg_GameOver");
    let authored_next = session.global("bj_changeLevelMapName");
    let objects = session.host().expect("published host").objects_mut();
    let local = objects.local_player_settings().expect("local player");
    let exit = {
        let requests = objects.game_exit_requests();
        if !requests.is_empty() {
            assert!(
                requests.iter().all(|request| {
                    request.next_level.as_deref() == Some(NEXT) && !request.score_screen
                }),
                "authored exit is ChangeLevel({NEXT:?}, false): {requests:?} next={authored_next:?}"
            );
            println!("Human05Interlude authored exit: requests={requests:?}");
        }
        !requests.is_empty()
    };
    Observation {
        clock: objects.timer_game_time().expect("admitted campaign clock"),
        control: objects.presentation().is_some_and(|state| {
            state.control_enabled == Some(true) && state.interface_visible == Some(true)
        }),
        escape,
        game_over,
        victory: objects
            .player_removal_state()
            .and_then(|state| state.get(local.identity))
            .is_some_and(|record| record.game_result == expected),
        exit,
    }
}

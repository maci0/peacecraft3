use super::*;
use peacecraft3::campaign_runtime::CampaignRuntime;
use std::time::Duration;

#[test]
#[ignore = "requires PCX_NATIVE_CHAPTER_INSTALL, MAP=Maps\\Campaign\\Human01.w3m, ADDON=roc; private display/audio/profile isolation"]
fn selected_human01_counter_frontier() {
    chapter_counter_horizon("Human01.w3m");
}

#[test]
#[ignore = "requires PCX_NATIVE_CHAPTER_INSTALL, MAP=Maps\\Campaign\\Undead02.w3m, ADDON=roc; private display/audio/profile isolation"]
fn selected_undead02_counter_frontier() {
    chapter_counter_horizon("Undead02.w3m");
}

fn chapter_counter_horizon(expected: &str) {
    let (install, map, expansion) = selected_chapter_env();
    assert!(!expansion, "explicitly select the RoC source namespace");
    assert!(
        map.rsplit(['\\', '/'])
            .next()
            .unwrap()
            .eq_ignore_ascii_case(expected)
    );
    let mut app = chapter_app_with_startup(
        install,
        expansion,
        peacecraft3::campaign_startup::install_native_counter,
    );
    let recipe = enqueue_selected_chapter(&mut app, &map);
    for _ in 0..6 {
        app.update();
    }
    assert_published_chapter(&mut app, &map, recipe, true);
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let context = Arc::clone(session.context().unwrap());
    assert!(session.host().unwrap().objects().native_counter_platform());
    println!("Counter chapter={map} context={:?}", context.sources.map);
    // Diagnostic input cadence and horizon; source Counter rules retain all game timing.
    let input = Duration::from_millis(25);
    let mut runtime = CampaignRuntime::new(input, 8, 65536);
    let mut cinematic = false;
    let mut controlled = false;
    for frame in 0..12000 {
        runtime.advance(&mut session, input);
        let objects = session.host().unwrap().objects();
        assert!(
            runtime.last_error.is_none(),
            "Counter frontier: chapter={map} frame={frame} input={input:?} clock={:?} context={:?} error={:?}",
            objects.native_game_point(),
            context.sources.map,
            runtime.last_error
        );
        let state = objects.presentation().unwrap();
        cinematic |= state.control_enabled == Some(false);
        controlled |= cinematic
            && state.control_enabled == Some(true)
            && state.interface_visible == Some(true);
        if state.control_enabled == Some(false) && frame % 10 == 0 {
            session
                .host()
                .unwrap()
                .objects_mut()
                .fire_local_player_event("EVENT_PLAYER_END_CINEMATIC")
                .unwrap();
        }
        assert!(
            requested_exit(&mut session).is_none(),
            "unexpected early Counter exit: chapter={map} frame={frame} clock={:?} context={:?}",
            session.host().unwrap().objects().native_game_point(),
            context.sources.map,
        );
    }
    assert!(
        controlled,
        "Counter chapter must restore player control within the diagnostic horizon"
    );
}

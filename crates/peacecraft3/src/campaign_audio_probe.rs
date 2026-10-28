//! Local-original probe: which campaign sounds reach Bevy audio entities.
//! Needs the unchanged Human01 chapter selected through `PCX_NATIVE_CHAPTER_*` and
//! a private `XDG_DATA_HOME` under `.scratch`. No audio plugin or device is created:
//! "reaches audio" means an `AudioPlayer` entity with its decodable source asset.

use crate::menu_audio::CampaignMusic;
use bevy::prelude::*;
use pcx_play::{
    map_startup_host::MapStartupHost,
    player_command_log::{PlayerCommand, StartIdentity},
};
use peacecraft3::{
    campaign_runtime::CampaignRuntime,
    campaign_startup::CampaignSeed,
    map_script_session::MapScriptSession,
    script_audio::{ScriptAudioReport, ScriptVoice},
};
use std::collections::BTreeSet;
use std::time::Duration;

/// Probe pacing, not an authored duration.
const BATCHES: u32 = 240;
const TICKS_PER_BATCH: usize = 64;
const SKIP_AFTER_BATCH: u32 = 120;

fn begin(app: &mut App) -> CampaignRuntime {
    let identity = {
        let session = app.world().resource::<MapScriptSession<MapStartupHost>>();
        StartIdentity::from_context(
            app.world().resource::<CampaignSeed>().prepared.unwrap(),
            session.context().unwrap(),
        )
    };
    app.world_mut()
        .resource_mut::<crate::campaign_input::CampaignInput>()
        .commands
        .begin(identity);
    let mut runtime = CampaignRuntime::new(Duration::from_secs_f64(1.0 / 64.0), 64, 4096);
    {
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        runtime.advance_ticks(&mut session, 0).unwrap();
    }
    runtime
}

fn advance(app: &mut App, runtime: &mut CampaignRuntime) {
    {
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        runtime
            .advance_ticks(&mut session, TICKS_PER_BATCH)
            .unwrap();
    }
    app.update();
}

fn skip_cinematic(app: &mut App) {
    app.world_mut().resource_scope(
        |world, mut input: Mut<crate::campaign_input::CampaignInput>| {
            let mut session = world.resource_mut::<MapScriptSession<MapStartupHost>>();
            let objects = session.host().unwrap().objects_mut();
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

/// The `[Human]` `Music_V0` entries of the selected skin table, read through the
/// session's own resolver.
fn skin_playlist(app: &App, section: &str, key: &str) -> Vec<String> {
    let requests = app
        .world()
        .resource::<peacecraft3::map_requests::MapRequests>();
    let receipts = &requests.bindings.as_ref().unwrap().receipts;
    let reader = app
        .world()
        .resource::<peacecraft3::browser_configuration::SelectedMapSources>()
        .for_receipts(receipts)
        .expect("published reader");
    let text = reader
        .resolve_bounded("UI\\war3skins.txt", 1 << 20)
        .unwrap()
        .bytes;
    let skin = crate::menu_skin::SkinTable::parse(std::str::from_utf8(&text).unwrap()).unwrap();
    let section = skin
        .sections()
        .iter()
        .find(|s| s.name == section)
        .expect("race section");
    let (_, value) = section
        .entries
        .iter()
        .find(|(entry, _)| entry == key)
        .expect("playlist key");
    value.split(';').map(str::to_owned).collect()
}

#[test]
#[ignore = "requires unchanged Human01 via PCX_NATIVE_CHAPTER_* selecting roc and a private XDG_DATA_HOME under .scratch"]
fn human01_opening_audio_reaches_entities() {
    let mut app = crate::native_campaign::tests::started_app();
    peacecraft3::script_audio::install(&mut app);
    crate::campaign_audio::install(&mut app);
    app.init_resource::<crate::menu_settings::Backend>();
    let mut runtime = begin(&mut app);
    let mut last_refusal = None;
    let mut voices_seen = BTreeSet::new();
    let mut music_seen: Vec<String> = Vec::new();
    let mut thematic_seen = None;
    for batch in 0..BATCHES {
        advance(&mut app, &mut runtime);
        if batch == SKIP_AFTER_BATCH {
            skip_cinematic(&mut app);
        }
        let world = app.world_mut();
        let mut voices = world.query::<&ScriptVoice>();
        for voice in voices.iter(world) {
            voices_seen.insert(voice.member.clone());
        }
        let mut tracks = world.query::<&CampaignMusic>();
        let heard: Vec<String> = tracks.iter(world).map(|t| t.member.clone()).collect();
        assert!(heard.len() <= 1, "one music track at a time: {heard:?}");
        if let Some(member) = heard.first()
            && music_seen.last() != Some(member)
        {
            println!("batch {batch}: music starts {member}");
            music_seen.push(member.clone());
        }
        let mut session = world.resource_mut::<MapScriptSession<MapStartupHost>>();
        let objects = session.host().unwrap().objects();
        thematic_seen = thematic_seen.or(objects.thematic_music().map(str::to_owned));
        let report = world.resource::<ScriptAudioReport>();
        if report.last_refusal != last_refusal {
            println!("batch {batch}: refusal: {:?}", report.last_refusal);
            last_refusal = report.last_refusal.clone();
        }
    }
    let report = app.world().resource::<ScriptAudioReport>();
    println!(
        "final: played={} refusals={} admission_error={:?} partial={:?}",
        report.played, report.refusal_count, report.admission_error, report.last_partial
    );
    for member in &voices_seen {
        println!("voice member: {member}");
    }
    let audio = app
        .world()
        .resource::<crate::campaign_audio::CampaignAudio>();
    assert!(
        audio.last_error.is_none(),
        "campaign music error: {:?}",
        audio.last_error
    );
    let thematic = thematic_seen.expect("the opening requests thematic music");
    assert_eq!(
        music_seen.first(),
        Some(&thematic),
        "thematic music plays first: {music_seen:?}"
    );
    let playlist = skin_playlist(&app, "Human", "Music_V0");
    assert!(
        music_seen.len() > 1 && playlist.contains(&music_seen[1]),
        "map playlist resumes after the thematic file: {music_seen:?} vs {playlist:?}"
    );
}

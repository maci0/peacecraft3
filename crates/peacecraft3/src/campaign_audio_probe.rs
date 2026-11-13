//! Local-original probe: which campaign sounds reach Bevy audio entities.
//! Needs the unchanged Human01 chapter selected through `PCX_NATIVE_CHAPTER_*` and
//! a private `XDG_DATA_HOME` under `.scratch`. "Reaches audio" means an `AudioPlayer`
//! entity with its source asset. Only the null-sink test adds the audio plugin, and
//! only after `assert_null_audio_environment` shows no device outside `.scratch` is reachable.

use crate::campaign_audio::GameSound;
use crate::menu_audio::CampaignMusic;
use bevy::audio::AudioSinkPlayback;
use bevy::prelude::*;
use pcx_play::{
    map_startup_host::MapStartupHost,
    player_command_log::{PlayerCommand, StartIdentity},
    standard_command::Kind,
};
use pcx_script::{HandleId, Value};
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
/// Batches the null-sink probe waits for a music sink.
const SINK_BATCHES: u32 = 8;
/// Distance of the probe's move order from the hero, in map units.
const MOVE_OFFSET: f32 = 256.0;

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
    crate::campaign_ack::install(&mut app);
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
    let hero = select_hero(&mut app);
    app.update();
    assert_eq!(playing_sounds(&mut app), ["ArthasWhat"]);
    move_hero_near(&mut app, hero);
    app.update();
    assert_eq!(
        playing_sounds(&mut app),
        ["ArthasWhat", "ArthasYes"],
        "the order speaks on its own channel beside the selection"
    );
    let audio = app
        .world()
        .resource::<crate::campaign_audio::CampaignAudio>();
    assert!(
        audio.last_error.is_none(),
        "campaign audio error after the unit voices: {:?}",
        audio.last_error
    );
}

/// Selects Arthas through the same command log the pointer path writes.
fn select_hero(app: &mut App) -> HandleId {
    app.world_mut().resource_scope(
        |world, mut input: Mut<crate::campaign_input::CampaignInput>| {
            let mut session = world.resource_mut::<MapScriptSession<MapStartupHost>>();
            let hero = match session.global("udg_Arthas") {
                Some(Value::Handle(id)) => id,
                other => panic!("udg_Arthas: expected a live handle, got {other:?}"),
            };
            let objects = session.host().unwrap().objects_mut();
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
            hero
        },
    )
}

fn move_hero_near(app: &mut App, hero: HandleId) {
    app.world_mut().resource_scope(
        |world, mut input: Mut<crate::campaign_input::CampaignInput>| {
            let mut session = world.resource_mut::<MapScriptSession<MapStartupHost>>();
            let objects = session.host().unwrap().objects_mut();
            let [x, y] = objects.executed_unit(hero).unwrap().position.unwrap();
            crate::campaign_input::standard_target(
                objects,
                &mut input,
                Kind::Move,
                [x + MOVE_OFFSET, y],
                None,
            )
            .unwrap();
        },
    );
}

fn playing_sounds(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    let mut sounds = world.query::<&GameSound>();
    let mut names: Vec<String> = sounds.iter(world).map(|sound| sound.name.clone()).collect();
    names.sort();
    names
}

/// Refuses to open an audio device unless the process cannot reach the desktop: no
/// display, a PulseAudio address that does not exist and an ALSA configuration that
/// routes the default device to the null plugin. Everything stays under `.scratch`.
fn assert_null_audio_environment() {
    for name in ["DISPLAY", "WAYLAND_DISPLAY"] {
        assert!(std::env::var_os(name).is_none(), "{name} must be unset");
    }
    let scratch = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.scratch")
        .canonicalize()
        .expect("local test artifacts directory");
    let alsa = std::path::PathBuf::from(
        std::env::var_os("ALSA_CONFIG_PATH").expect("ALSA_CONFIG_PATH must select a null device"),
    )
    .canonicalize()
    .expect("ALSA configuration file");
    assert!(
        alsa.starts_with(&scratch),
        "ALSA configuration outside .scratch"
    );
    let config = std::fs::read_to_string(&alsa).unwrap();
    assert_eq!(
        config.split_whitespace().collect::<Vec<_>>(),
        "pcm.!default { type null } ctl.!default { type null }"
            .split_whitespace()
            .collect::<Vec<_>>(),
        "the ALSA configuration must route only the default device to null"
    );
    let pulse = std::env::var("PULSE_SERVER").expect("PULSE_SERVER must name an absent socket");
    let socket = std::path::Path::new(
        pulse
            .strip_prefix("unix:")
            .expect("PULSE_SERVER must be a unix socket path"),
    );
    assert!(
        socket.starts_with(&scratch) && !socket.exists(),
        "PULSE_SERVER must name an absent socket under .scratch"
    );
}

#[test]
#[ignore = "requires unchanged Human01 via PCX_NATIVE_CHAPTER_*, a private XDG_DATA_HOME and the null-audio environment of .scratch/runtest.sh"]
fn human01_music_reaches_a_null_audio_sink() {
    check_human01_music_sink(false);
}

#[test]
#[ignore = "requires unchanged Human01 via PCX_NATIVE_CHAPTER_* and verified private null audio"]
fn human01_thematic_seek_reaches_a_null_audio_sink() {
    check_human01_music_sink(true);
}

fn check_human01_music_sink(check_seek: bool) {
    assert_null_audio_environment();
    let mut app = crate::native_campaign::tests::started_app();
    app.add_plugins((
        bevy::asset::AssetPlugin::default(),
        bevy::audio::AudioPlugin::default(),
    ));
    peacecraft3::script_audio::install(&mut app);
    crate::campaign_audio::install(&mut app);
    app.init_resource::<crate::menu_settings::Backend>();
    let mut runtime = begin(&mut app);
    let mut checked = false;
    for _ in 0..SINK_BATCHES {
        advance(&mut app, &mut runtime);
        app.update();
        let world = app.world_mut();
        let mut tracks = world.query::<(&CampaignMusic, Option<&AudioSink>)>();
        if let Some((track, sink)) = tracks.iter(world).next() {
            let sink = sink.expect("the music track has a sink on the null device");
            assert!(
                !sink.is_paused() && !sink.empty(),
                "{} is not playing",
                track.member
            );
            let wanted = world
                .resource::<crate::menu_settings::Backend>()
                .audio
                .effective_music();
            assert_eq!(sink.volume(), bevy::audio::Volume::Linear(wanted));
            println!("sink playing {}", track.member);
            checked = true;
            break;
        }
    }
    assert!(
        checked,
        "no music track began within {SINK_BATCHES} batches"
    );
    if check_seek {
        use pcx_script::Host;
        app.world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .host()
            .unwrap()
            .objects_mut()
            .native("SetThematicMusicPlayPosition", &[Value::Int(37)])
            .unwrap();
        app.update();
        let world = app.world_mut();
        let mut sinks = world.query_filtered::<&AudioSink, With<CampaignMusic>>();
        assert!(sinks.single(world).unwrap().position() >= Duration::from_millis(37));
        assert!(
            world
                .resource::<crate::campaign_audio::CampaignAudio>()
                .last_error
                .is_none()
        );
    }
}

#[test]
#[ignore = "requires unchanged Human01 via PCX_NATIVE_CHAPTER_* and isolated display/null audio"]
fn human01_thematic_ex_seek_restart_and_retirement_reach_audio_entities() {
    use pcx_script::Host;
    let mut app = crate::native_campaign::tests::started_app();
    crate::campaign_audio::install(&mut app);
    app.init_resource::<crate::menu_settings::Backend>();
    let mut runtime = begin(&mut app);
    for _ in 0..SINK_BATCHES {
        advance(&mut app, &mut runtime);
        let world = app.world_mut();
        if world.query::<&CampaignMusic>().iter(world).next().is_some() {
            break;
        }
    }
    let (original, path) = {
        let world = app.world_mut();
        let mut tracks = world.query::<(Entity, &CampaignMusic)>();
        let (entity, track) = tracks.single(world).unwrap();
        (entity, track.member.clone())
    };
    let call = |app: &mut App, name, args: &[Value]| {
        app.world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .host()
            .unwrap()
            .objects_mut()
            .native(name, args)
            .unwrap();
    };
    call(
        &mut app,
        "PlayThematicMusicEx",
        &[Value::Str(path.clone()), Value::Int(9580)],
    );
    app.update();
    app.update();
    let restarted = {
        let world = app.world_mut();
        let mut tracks = world.query::<(Entity, &CampaignMusic, &PlaybackSettings)>();
        let (entity, track, settings) = tracks.single(world).unwrap();
        assert_eq!(track.member, path);
        assert_ne!(entity, original);
        assert_eq!(settings.start_position, Some(Duration::from_millis(9580)));
        entity
    };
    call(
        &mut app,
        "SetThematicMusicPlayPosition",
        &[Value::Int(2401)],
    );
    app.update();
    assert_eq!(
        app.world()
            .get::<PlaybackSettings>(restarted)
            .unwrap()
            .start_position,
        Some(Duration::from_millis(2401))
    );
    call(&mut app, "PlayThematicMusic", &[Value::Str(path)]);
    let initial = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>()
        .host()
        .unwrap()
        .objects()
        .thematic_music_request()
        .unwrap()
        .position;
    app.update();
    app.update();
    assert!(app.world().get_entity(restarted).is_err());
    let world = app.world_mut();
    assert_eq!(
        world
            .query_filtered::<&PlaybackSettings, With<CampaignMusic>>()
            .single(world)
            .unwrap()
            .start_position,
        initial
    );
    call(&mut app, "EndThematicMusic", &[]);
    app.update();
    app.insert_resource(MapScriptSession::<MapStartupHost>::default());
    app.update();
    let world = app.world_mut();
    assert_eq!(world.query::<&CampaignMusic>().iter(world).count(), 0);
    assert!(
        world
            .resource::<crate::campaign_audio::CampaignAudio>()
            .last_error
            .is_none()
    );
}

pub(crate) fn selected_regular_volume_rules() -> pcx_play::playercolor_host::RegularMusicVolumeRules
{
    let image_path = std::env::var_os("PCX_WAR3_EXE").expect("explicit selected client");
    let backend_path = std::env::var_os("PCX_MUSIC_BACKEND").expect("explicit local backend");
    let read = |path: &std::ffi::OsStr, limit: u64| {
        use std::io::Read;
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .unwrap()
            .take(limit + 1)
            .read_to_end(&mut bytes)
            .unwrap();
        assert!(bytes.len() as u64 <= limit);
        bytes
    };
    pcx_play::playercolor_host::RegularMusicVolumeRules::read(
        &read(&image_path, 32 << 20),
        &read(&backend_path, 4 << 20),
        pcx_play::scripts::AssetIdentity {
            source: backend_path.to_string_lossy().into_owned(),
            edition: "Legacy".into(),
            version: "3.0.0.0".into(),
        },
    )
    .unwrap()
}

#[test]
#[ignore = "requires unchanged Human01 via PCX_NATIVE_CHAPTER_*, pinned local music backend and verified private null audio"]
fn human01_regular_volume_updates_live_sink_future_track_and_preserves_thematic() {
    use crate::menu_audio::RegularMusicGain;
    use bevy::ecs::system::RunSystemOnce;
    use pcx_script::Host;
    assert_null_audio_environment();
    let mut app = crate::native_campaign::tests::started_app();
    app.add_plugins((
        bevy::asset::AssetPlugin::default(),
        bevy::audio::AudioPlugin::default(),
    ));
    crate::campaign_audio::install(&mut app);
    app.init_resource::<crate::menu_settings::Backend>();
    let mut runtime = begin(&mut app);
    for _ in 0..SINK_BATCHES {
        advance(&mut app, &mut runtime);
        let world = app.world_mut();
        if world.query::<&AudioSink>().iter(world).next().is_some() {
            break;
        }
    }
    let call = |app: &mut App, name, args: &[Value]| {
        app.world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .host()
            .unwrap()
            .objects_mut()
            .native(name, args)
            .unwrap();
    };
    let theme_path = {
        let world = app.world_mut();
        let (track, sink, regular) = world
            .query::<(&CampaignMusic, &AudioSink, Option<&RegularMusicGain>)>()
            .single(world)
            .unwrap();
        assert!(regular.is_none());
        assert!(!sink.is_paused() && !sink.empty());
        track.member.clone()
    };
    call(&mut app, "SetMusicVolume", &[Value::Int(0)]);
    app.update();
    app.world_mut()
        .run_system_once(crate::menu_audio::apply_user_volume)
        .unwrap();
    {
        let world = app.world_mut();
        let (sink, regular) = world
            .query_filtered::<(&AudioSink, Option<&RegularMusicGain>), With<CampaignMusic>>()
            .single(world)
            .unwrap();
        assert!(regular.is_none());
        assert_eq!(sink.volume(), bevy::audio::Volume::Linear(1.0));
    }
    call(&mut app, "EndThematicMusic", &[]);
    call(
        &mut app,
        "SetMapMusic",
        &[
            Value::Str("Music".into()),
            Value::Bool(false),
            Value::Int(0),
        ],
    );
    for _ in 0..SINK_BATCHES {
        app.update();
    }
    app.world_mut()
        .run_system_once(crate::menu_audio::apply_user_volume)
        .unwrap();
    let (first, maximum) = {
        let world = app.world_mut();
        let (entity, sink, gain) = world
            .query::<(Entity, &AudioSink, &RegularMusicGain)>()
            .single(world)
            .unwrap();
        assert_eq!(gain.request.level, 0);
        assert_eq!(sink.volume(), bevy::audio::Volume::Linear(0.0));
        assert!(!sink.is_paused() && !sink.empty());
        (entity, gain.request.rules.maximum_level())
    };
    call(
        &mut app,
        "SetMusicVolume",
        &[Value::Int(i32::try_from(maximum).unwrap())],
    );
    app.world_mut()
        .resource_mut::<crate::menu_settings::Backend>()
        .audio
        .music_gain = 0.5;
    app.update();
    app.world_mut()
        .run_system_once(crate::menu_audio::apply_user_volume)
        .unwrap();
    {
        let sink = app.world().get::<AudioSink>(first).unwrap();
        let gain = app.world().get::<RegularMusicGain>(first).unwrap();
        let expected = gain.request.rules.gain(maximum, gain.group, 0.5).unwrap();
        assert_eq!(gain.request.level, maximum);
        assert_eq!(sink.volume(), bevy::audio::Volume::Linear(expected));
        assert_ne!(expected, 0.5);
    }
    call(&mut app, "SetMusicVolume", &[Value::Int(0)]);
    call(&mut app, "ClearMapMusic", &[]);
    app.update();
    call(
        &mut app,
        "SetMapMusic",
        &[
            Value::Str("Music".into()),
            Value::Bool(false),
            Value::Int(0),
        ],
    );
    for _ in 0..SINK_BATCHES {
        app.update();
    }
    app.world_mut()
        .run_system_once(crate::menu_audio::apply_user_volume)
        .unwrap();
    {
        let world = app.world_mut();
        let (entity, sink, gain) = world
            .query::<(Entity, &AudioSink, &RegularMusicGain)>()
            .single(world)
            .unwrap();
        assert_ne!(entity, first);
        assert_eq!(gain.request.level, 0);
        assert_eq!(sink.volume(), bevy::audio::Volume::Linear(0.0));
        assert!(!sink.is_paused() && !sink.empty());
    }
    call(&mut app, "PlayThematicMusic", &[Value::Str(theme_path)]);
    for _ in 0..SINK_BATCHES {
        app.update();
    }
    app.world_mut()
        .run_system_once(crate::menu_audio::apply_user_volume)
        .unwrap();
    {
        let world = app.world_mut();
        let (sink, regular) = world
            .query_filtered::<(&AudioSink, Option<&RegularMusicGain>), With<CampaignMusic>>()
            .single(world)
            .unwrap();
        assert!(regular.is_none());
        assert_eq!(sink.volume(), bevy::audio::Volume::Linear(0.5));
    }
    app.insert_resource(MapScriptSession::<MapStartupHost>::default());
    app.update();
    let world = app.world_mut();
    assert_eq!(world.query::<&CampaignMusic>().iter(world).count(), 0);
    assert!(
        world
            .resource::<crate::campaign_audio::CampaignAudio>()
            .last_error
            .is_none()
    );
}

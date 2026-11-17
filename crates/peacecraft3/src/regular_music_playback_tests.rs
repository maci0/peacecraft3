use super::*;
use crate::regular_music_epoch::{Epoch, SourceFrame};
use bevy::ecs::system::RunSystemOnce;
use pcx_play::map_startup_host::MapStartupHost;
use pcx_play::playercolor_host::{RegularMusicOperation as Operation, RegularMusicRules};
use peacecraft3::map_script_session::MapScriptSession;
use std::sync::Arc;

fn source_frame(app: &App) -> SourceFrame {
    app.world()
        .resource::<crate::native_campaign::Chapters>()
        .audio_sources(app.world().resource::<crate::launch_gui::Launcher>())
        .unwrap()
        .unwrap()
        .0
}

fn compare_random(app: &App, expected: &pcx_play::playercolor_host::AudioRandom) {
    let mut actual = app
        .world()
        .resource::<Epoch>()
        .playback
        .as_ref()
        .unwrap()
        .controller
        .random
        .clone();
    let mut expected = expected.clone();
    for _ in 0..12 {
        assert_eq!(
            actual.choose(1_000_003, &mut 100).unwrap(),
            expected.choose(1_000_003, &mut 100).unwrap()
        );
    }
}

fn acknowledge_hero(app: &mut App, expected: &mut pcx_play::playercolor_host::AudioRandom) {
    crate::campaign_audio_probe::begin(app);
    let hero = crate::campaign_audio_probe::select_hero(app);
    let name = {
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let record = session
            .host()
            .unwrap()
            .objects()
            .executed_unit(hero)
            .unwrap();
        format!(
            "{}What",
            crate::campaign_ack::unit_sound(&record.definition)
                .unwrap()
                .unwrap()
        )
    };
    app.world_mut()
        .run_system_once(crate::campaign_ack::unit_voices)
        .unwrap();
    let row = app
        .world_mut()
        .resource_mut::<crate::campaign_audio::CampaignAudio>()
        .bank
        .as_mut()
        .unwrap()
        .row("UI\\SoundInfo\\UnitAckSounds.slk", &name)
        .unwrap();
    expected
        .choose_non_repeating(&row.record, row.members.len(), &mut 1024)
        .unwrap();
    if row.flags.iter().any(|flag| flag == "RANDOMPITCH") {
        expected
            .pitch(row.pitch, row.pitch_variance, true, &mut 1024)
            .unwrap();
    }
    let world = app.world_mut();
    assert!(
        world
            .query::<&crate::campaign_audio::GameSound>()
            .iter(world)
            .any(|sound| sound.name == name),
        "actual acknowledgement did not publish {name}"
    );
    compare_random(app, expected);
}

fn await_empty(app: &mut App, entity: Entity) {
    app.world().get::<AudioSink>(entity).unwrap().stop();
    for _ in 0..512 {
        app.world_mut().run_schedule(PostUpdate);
        if app.world().get::<AudioSink>(entity).unwrap().empty() {
            return;
        }
        std::thread::yield_now();
    }
    panic!("null backend did not acknowledge stop within 512 audio updates");
}

#[test]
#[ignore = "requires selected client/backend, unchanged Human01 and verified private null audio"]
fn native_menu_single_and_duplicate_or_distinct_eof_use_one_draw_and_keep_ack_during_replacement() {
    let f = fixture();
    for kind in ["single", "duplicate", "distinct"] {
        let mut app = crate::native_campaign::tests::started_audio_app();
        assert!(!crate::menu_audio::menu_paused(app.world()));
        let frame = source_frame(&app);
        let reader = app
            .world()
            .resource::<crate::native_campaign::Chapters>()
            .audio_sources(app.world().resource::<crate::launch_gui::Launcher>())
            .unwrap()
            .unwrap()
            .1;
        let asset = reader
            .resolve_bounded("UI\\war3skins.txt", 1 << 20)
            .unwrap();
        let skin =
            crate::menu_skin::SkinTable::parse(std::str::from_utf8(&asset.bytes).unwrap()).unwrap();
        let key = f
            .rules
            .edition_key(f.rules.menu_start().0, frame.edition)
            .unwrap();
        let first = f
            .rules
            .tracks(&skin.unique_file(&key).unwrap().file)
            .unwrap()
            .remove(0);
        let second = if kind == "distinct" {
            skin.sections()
                .iter()
                .flat_map(|section| &section.entries)
                .flat_map(|(_, value)| f.rules.tracks(value).unwrap_or_default())
                .find(|path| path != &first && path.to_ascii_lowercase().ends_with(".mp3"))
                .expect("selected skin names another distinct music member")
        } else {
            first.clone()
        };
        let list = if kind == "single" {
            first.clone()
        } else {
            format!("{first};{second}")
        };
        let skin =
            crate::menu_skin::SkinTable::parse(&format!("[Default]\n{key}={list}\n")).unwrap();
        let mut expected = app
            .world()
            .resource::<Epoch>()
            .playback
            .as_ref()
            .unwrap()
            .controller
            .random
            .clone();
        expected
            .choose(if kind == "single" { 1 } else { 2 }, &mut 100)
            .unwrap();
        app.world_mut()
            .resource_scope(|world, mut epoch: Mut<Epoch>| {
                epoch
                    .menu_choice(
                        &skin,
                        &|path, limit| {
                            let asset = reader
                                .resolve_bounded(path, limit)
                                .map_err(|error| error.to_string())?;
                            let (_, origin) = pcx_play::campaign_assets::identity(
                                &asset,
                                &format!("version enumerand {}", frame.edition),
                                16 << 10,
                            )?;
                            Ok((asset.bytes, origin))
                        },
                        |_, source, _, looping| {
                            assert_eq!(looping, kind == "single");
                            let entity = if looping {
                                let handle = world
                                    .resource_mut::<Assets<LoopSource>>()
                                    .add(LoopSource(source));
                                world
                                    .spawn((
                                        AudioPlayer(handle),
                                        PlaybackSettings::ONCE,
                                        crate::menu_audio::MenuMusic,
                                        AuthoredVolume(1.0),
                                    ))
                                    .id()
                            } else {
                                let handle =
                                    world.resource_mut::<Assets<AudioSource>>().add(source);
                                world
                                    .spawn((
                                        AudioPlayer(handle),
                                        PlaybackSettings::ONCE,
                                        crate::menu_audio::MenuMusic,
                                        AuthoredVolume(1.0),
                                    ))
                                    .id()
                            };
                            Ok(entity)
                        },
                    )
                    .unwrap();
            });
        compare_random(&app, &expected);
        app.world_mut().run_schedule(PostUpdate);
        let (serial, entity, path) = {
            let menu = app.world().resource::<Epoch>().menu.as_ref().unwrap();
            let voice = menu.controller.regular().unwrap();
            (
                voice.serial,
                menu.voices[&voice.serial].entity,
                voice.path.clone(),
            )
        };
        assert!(app.world().get::<AudioSink>(entity).is_some());
        if kind == "single" {
            app.world_mut()
                .resource_scope(|world, mut epoch: Mut<Epoch>| epoch.drive_menu(world, 10))
                .unwrap();
            assert!(app.world().get::<AudioPlayer<LoopSource>>(entity).is_some());
            acknowledge_hero(&mut app, &mut expected);
            continue;
        }
        await_empty(&mut app, entity);
        app.world_mut()
            .resource_scope(|world, mut epoch: Mut<Epoch>| epoch.drive_menu(world, 10))
            .unwrap();
        assert!(
            app.world()
                .resource::<Epoch>()
                .menu
                .as_ref()
                .unwrap()
                .controller
                .regular()
                .unwrap()
                .ended
        );
        let due = 10 + f.rules.next_interval_ms();
        app.world_mut()
            .resource_scope(|world, mut epoch: Mut<Epoch>| epoch.drive_menu(world, due - 1))
            .unwrap();
        assert_eq!(
            app.world()
                .resource::<Epoch>()
                .menu
                .as_ref()
                .unwrap()
                .controller
                .regular()
                .unwrap()
                .serial,
            serial
        );
        let result = app
            .world_mut()
            .resource_scope(|world, mut epoch: Mut<Epoch>| epoch.drive_menu(world, due));
        assert!(result.unwrap_err().contains("pending or unavailable"));
        assert!(app.world().get::<AudioSink>(entity).is_some());
        assert!(
            app.world()
                .resource::<Epoch>()
                .menu
                .as_ref()
                .unwrap()
                .has_pending()
        );
        acknowledge_hero(&mut app, &mut expected);
        app.world_mut().run_schedule(PostUpdate);
        app.world_mut()
            .resource_scope(|world, mut epoch: Mut<Epoch>| epoch.drive_menu(world, due))
            .unwrap();
        let menu = app.world().resource::<Epoch>().menu.as_ref().unwrap();
        let voice = menu.controller.regular().unwrap();
        assert_ne!(voice.serial, serial);
        assert_eq!(voice.path == path, kind == "duplicate");
        assert!(!voice.looping && !menu.has_pending());
        assert!(app.world().get_entity(entity).is_err());
        let promoted = menu.voices[&voice.serial].entity;
        assert!(
            app.world()
                .get::<crate::menu_audio::MenuMusic>(promoted)
                .is_some()
        );
        assert!(app.world().get::<RegularMusicGain>(promoted).is_none());
        assert_eq!(app.world().get::<AuthoredVolume>(promoted).unwrap().0, 1.0);
        compare_random(&app, &expected);
        let mut retained = menu.controller.random.clone();
        assert_eq!(
            retained.choose(1_000_003, &mut 100).unwrap(),
            expected.clone().choose(1_000_003, &mut 100).unwrap()
        );
        app.world_mut()
            .resource_scope(|world, mut epoch: Mut<Epoch>| {
                epoch.observe(
                    world,
                    Some(Ok(SourceFrame {
                        revision: frame.revision + 1,
                        ..frame.clone()
                    })),
                    crate::music_wall::sample().unwrap(),
                )
            })
            .unwrap();
        assert!(app.world().resource::<Epoch>().menu.is_none());
        assert!(app.world().get_entity(promoted).is_err());
        compare_random(&app, &expected);
    }
}

fn drain_native_commands(app: &mut App) {
    for _ in 0..512 {
        let queue_empty = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .host()
            .unwrap()
            .objects()
            .regular_music_commands()
            .expect("normal startup must configure the journal")
            .is_empty();
        let epoch = app.world().resource::<Epoch>();
        let p = epoch.playback.as_ref().unwrap();
        assert!(p.fault.is_none(), "music consumer fault: {:?}", p.fault);
        if queue_empty && !p.has_pending() {
            return;
        }
        app.update();
    }
    panic!(
        "music command prefix did not drain in 512 application updates: {:?}",
        app.world().resource::<Epoch>().last_error
    );
}

fn native(app: &mut App, name: &str, args: &[pcx_script::Value]) {
    call_native(app, name, args);
    drain_native_commands(app);
}

fn call_native(app: &mut App, name: &str, args: &[pcx_script::Value]) {
    use pcx_script::Host;
    app.world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>()
        .host()
        .unwrap()
        .native(name, args)
        .unwrap();
}

fn drive_at(app: &mut App, now: u32) {
    let result = app
        .world_mut()
        .resource_scope(|world, mut epoch: Mut<Epoch>| epoch.drive(world, now));
    if let Err(error) = result {
        assert!(error.contains("pending or unavailable"), "{error}");
    }
    app.world_mut()
        .run_system_once(crate::menu_audio::apply_user_volume)
        .unwrap();
    app.world_mut().run_schedule(PostUpdate);
}

fn native_at(app: &mut App, name: &str, args: &[pcx_script::Value], now: u32) {
    call_native(app, name, args);
    for _ in 0..512 {
        drive_at(app, now);
        let empty = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .host()
            .unwrap()
            .objects()
            .regular_music_commands()
            .unwrap()
            .is_empty();
        if empty
            && !app
                .world()
                .resource::<Epoch>()
                .playback
                .as_ref()
                .unwrap()
                .has_pending()
        {
            return;
        }
    }
    panic!("manual-wall command prefix did not drain in 512 audio updates");
}

fn finish_group_transition(app: &mut App) {
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let host = session.host().unwrap();
    let policy = host.sound_playback().unwrap().group_policy();
    let elapsed = policy.duration() + policy.period();
    // The manual music-wall fixture advances this separate existing REAL-audio clock directly.
    host.objects_mut().advance_volume_group_clock(elapsed);
    eprintln!("controlled volume-group elapsed_ns={}", elapsed.as_nanos());
}

#[test]
#[ignore = "requires selected client/backend, unchanged Human01 and verified private null audio"]
fn native_music_normal_startup_consumes_ordered_host_commands_into_retained_sinks() {
    use pcx_script::Value;
    let f = fixture();
    let mut app = crate::native_campaign::tests::started_audio_app();
    app.world_mut()
        .resource_mut::<crate::menu_settings::Backend>()
        .set_test_preferences(false);
    app.world_mut()
        .run_system_once(crate::menu_audio::load)
        .unwrap();
    drain_native_commands(&mut app);
    let frame = source_frame(&app);
    let reader = app
        .world()
        .resource::<crate::native_campaign::Chapters>()
        .audio_sources(app.world().resource::<crate::launch_gui::Launcher>())
        .unwrap()
        .unwrap()
        .1;
    let bytes = reader
        .resolve_bounded("UI\\war3skins.txt", 1 << 20)
        .unwrap()
        .bytes;
    let skin = crate::menu_skin::SkinTable::parse(std::str::from_utf8(&bytes).unwrap()).unwrap();
    let key = f
        .rules
        .edition_key(f.rules.menu_start().0, frame.edition)
        .unwrap();
    let member = f
        .rules
        .tracks(&skin.unique_file(&key).unwrap().file)
        .unwrap()
        .remove(0);
    native(&mut app, "EndThematicMusic", &[]);
    native(
        &mut app,
        "PlayMusicEx",
        &[
            Value::Str(member.clone().into()),
            Value::Int(0),
            Value::Int(0),
        ],
    );
    let p = app.world().resource::<Epoch>().playback.as_ref().unwrap();
    let serial = p.controller.regular().unwrap().serial;
    let entity = p.voices[&serial].entity;
    assert!(p.controller.regular().unwrap().looping);
    native(&mut app, "StopMusic", &[Value::Bool(false)]);
    assert!(app.world().get::<AudioSink>(entity).unwrap().is_paused());
    native(&mut app, "SetMusicPlayPosition", &[Value::Int(333)]);
    assert!(
        app.world()
            .get::<AudioSink>(entity)
            .unwrap()
            .position()
            .abs_diff(Duration::from_millis(333))
            <= Duration::from_millis(1)
    );
    native(&mut app, "ResumeMusic", &[]);
    assert!(!app.world().get::<AudioSink>(entity).unwrap().is_paused());
    assert!(!app.world().get::<PlaybackSettings>(entity).unwrap().paused);
    native(
        &mut app,
        "PlayThematicMusicEx",
        &[Value::Str(member.clone().into()), Value::Int(12)],
    );
    assert!(
        !app.world().get::<AudioSink>(entity).unwrap().is_paused(),
        "theme arms a source fade, not an immediate pause"
    );
    assert!(
        app.world()
            .resource::<Epoch>()
            .playback
            .as_ref()
            .unwrap()
            .controller
            .thematic()
            .is_some()
    );
    let theme = {
        let p = app.world().resource::<Epoch>().playback.as_ref().unwrap();
        let voice = p.controller.thematic().unwrap();
        p.voices[&voice.serial].entity
    };
    // Hold physical thematic EOF while the real consumer receives deterministic wall samples.
    app.world().get::<AudioSink>(theme).unwrap().pause();
    app.world_mut()
        .get_mut::<PlaybackSettings>(theme)
        .unwrap()
        .paused = true;
    assert!(
        app.world()
            .get::<crate::menu_audio::MenuMusic>(entity)
            .is_none()
    );
    assert!(app.world().get::<CampaignMusic>(entity).is_some());
    let first = crate::music_wall::sample()
        .unwrap()
        .milliseconds
        .wrapping_add(f.rules.fade_interval_ms());
    drive_at(&mut app, first);
    let endpoint = first.wrapping_add(f.rules.default_fade_ms());
    drive_at(&mut app, endpoint);
    let p = app.world().resource::<Epoch>().playback.as_ref().unwrap();
    assert!(p.controller.regular().unwrap().playing);
    assert_eq!(p.controller.effective_level(), 0);
    assert!(!app.world().get::<AudioSink>(entity).unwrap().is_paused());
    let now = endpoint.wrapping_add(f.rules.fade_interval_ms());
    drive_at(&mut app, now);
    let p = app.world().resource::<Epoch>().playback.as_ref().unwrap();
    assert_eq!(p.voices[&serial].entity, entity);
    assert!(!p.controller.regular().unwrap().playing);
    assert!(app.world().get::<PlaybackSettings>(entity).unwrap().paused);
    assert!(app.world().get::<AudioSink>(entity).unwrap().is_paused());
    let cursor = app.world().get::<AudioSink>(entity).unwrap().position();
    native_at(&mut app, "EndThematicMusic", &[], now);
    assert!(!app.world().get::<AudioSink>(entity).unwrap().is_paused());
    assert!(app.world().get::<AudioSink>(entity).unwrap().position() >= cursor);
    assert_eq!(
        app.world()
            .resource::<Epoch>()
            .playback
            .as_ref()
            .unwrap()
            .voices[&serial]
            .entity,
        entity
    );
    let level = f.gain.request.rules.maximum_level() / 2;
    native_at(&mut app, "SetMusicVolume", &[Value::Int(level as i32)], now);
    assert_eq!(
        app.world()
            .resource::<Epoch>()
            .playback
            .as_ref()
            .unwrap()
            .controller
            .target_level(),
        level
    );
    assert_eq!(
        app.world()
            .get::<RegularMusicGain>(entity)
            .unwrap()
            .request
            .level,
        level
    );
    let group = app
        .world()
        .resource::<MapScriptSession<MapStartupHost>>()
        .global("SOUND_VOLUMEGROUP_MUSIC")
        .expect("authored music volume group");
    let previous_group = app.world().get::<RegularMusicGain>(entity).unwrap().group;
    native_at(
        &mut app,
        "VolumeGroupSetVolume",
        &[group.clone(), Value::Real(0.375)],
        now,
    );
    assert_eq!(
        app.world().get::<RegularMusicGain>(entity).unwrap().group,
        previous_group
    );
    finish_group_transition(&mut app);
    drive_at(&mut app, now);
    assert_eq!(
        app.world().get::<RegularMusicGain>(entity).unwrap().group,
        0.375
    );
    {
        use pcx_script::Host;
        app.world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .host()
            .unwrap()
            .native(
                "PlayMusicEx",
                &[
                    Value::Str(member.clone().into()),
                    Value::Int(123),
                    Value::Int(0),
                ],
            )
            .unwrap();
    }
    let sequence = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>()
        .host()
        .unwrap()
        .objects()
        .regular_music_commands()
        .unwrap()
        .back()
        .unwrap()
        .sequence;
    for _ in 0..512 {
        drive_at(&mut app, now);
        if app
            .world()
            .resource::<Epoch>()
            .playback
            .as_ref()
            .unwrap()
            .pending
            .as_ref()
            .is_some_and(|pending| pending.sequence == Some(sequence))
        {
            break;
        }
    }
    assert_eq!(
        app.world()
            .resource::<Epoch>()
            .playback
            .as_ref()
            .unwrap()
            .pending
            .as_ref()
            .unwrap()
            .sequence,
        Some(sequence)
    );
    {
        use pcx_script::Host;
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let objects = session.host().unwrap().objects_mut();
        assert_eq!(
            objects
                .regular_music_commands()
                .unwrap()
                .front()
                .unwrap()
                .sequence,
            sequence,
            "staged source must remain unacknowledged"
        );
        objects
            .native("VolumeGroupSetVolume", &[group, Value::Real(0.625)])
            .unwrap();
        assert_eq!(
            objects
                .volume_group_channel_gain(f.gain.request.rules.channel())
                .unwrap(),
            0.375
        );
    }
    app.world_mut()
        .resource_mut::<crate::menu_settings::Backend>()
        .audio
        .music_gain = 0.25;
    finish_group_transition(&mut app);
    assert!(
        app.world()
            .resource::<Epoch>()
            .playback
            .as_ref()
            .unwrap()
            .has_pending()
    );
    for _ in 0..512 {
        drive_at(&mut app, now);
        if !app
            .world()
            .resource::<Epoch>()
            .playback
            .as_ref()
            .unwrap()
            .has_pending()
        {
            break;
        }
    }
    let p = app.world().resource::<Epoch>().playback.as_ref().unwrap();
    let promoted = p.voices[&p.controller.regular().unwrap().serial].entity;
    let gain = app.world().get::<RegularMusicGain>(promoted).unwrap();
    assert_eq!(gain.group, 0.625);
    assert_eq!(gain.request.level, p.controller.effective_level());
    let user = app
        .world()
        .resource::<crate::menu_settings::Backend>()
        .audio
        .effective_music();
    let expected = Volume::Linear(gain.with_user_gain(user).unwrap());
    assert_eq!(
        app.world()
            .get::<PlaybackSettings>(promoted)
            .unwrap()
            .volume,
        expected
    );
    assert_eq!(
        app.world().get::<AudioSink>(promoted).unwrap().volume(),
        expected
    );
    let random = p.controller.random.clone();
    await_empty(&mut app, promoted);
    call_native(
        &mut app,
        "PlayMusicEx",
        &[Value::Str(member.into()), Value::Int(1), Value::Int(0)],
    );
    let sequence = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>()
        .host()
        .unwrap()
        .objects()
        .regular_music_commands()
        .unwrap()
        .front()
        .unwrap()
        .sequence;
    for _ in 0..2 {
        let error = app
            .world_mut()
            .resource_scope(|world, mut epoch: Mut<Epoch>| epoch.drive(world, now))
            .unwrap_err();
        assert!(error.contains("infinite music decoder ended unexpectedly"));
        assert_eq!(
            app.world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>()
                .host()
                .unwrap()
                .objects()
                .regular_music_commands()
                .unwrap()
                .front()
                .unwrap()
                .sequence,
            sequence
        );
        compare_random(&app, &random);
    }
}

#[test]
#[ignore = "requires selected client/backend, unchanged Human01 and verified private null audio"]
fn native_music_source_boundaries_retire_on_same_content_revision_and_recheck_absence_and_error() {
    let f = fixture();
    let mut app = crate::native_campaign::tests::audio_fixture_app();
    let frame = source_frame(&app);
    let wall = crate::music_wall::sample().unwrap();
    assert!(
        app.world()
            .resource::<Epoch>()
            .rules_for(&frame.bundle)
            .is_some()
    );
    for boundary in ["revision", "none", "error"] {
        app.world_mut()
            .resource_scope(|world, mut epoch: Mut<Epoch>| {
                epoch.observe(world, Some(Ok(frame.clone())), wall).unwrap();
                let playback = epoch.playback.as_mut().unwrap();
                let mut next = playback.controller.clone();
                let effects = next
                    .apply(
                        Operation::Play {
                            name: "pending".into(),
                            position_ms: 0,
                            fade_ms: 0,
                        },
                        Some(vec!["a.wav".into(), "b.wav".into()]),
                        0,
                        &mut 100,
                    )
                    .unwrap();
                playback
                    .stage(
                        world,
                        next,
                        effects,
                        None,
                        Inputs {
                            gain: &f.gain,
                            loops: Some(&f.loops),
                            user: 1.0,
                            read: &|_, _| Ok((f.bytes.clone(), f.origin.clone())),
                        },
                    )
                    .unwrap();
                let staged = playback
                    .pending
                    .as_ref()
                    .unwrap()
                    .staged
                    .values()
                    .next()
                    .unwrap()
                    .entity;
                let mut expected = playback.controller.random.clone();
                let input = match boundary {
                    "revision" => Some(Ok(SourceFrame {
                        revision: frame.revision + 1,
                        ..frame.clone()
                    })),
                    "none" => None,
                    "error" => Some(Err("source getter failed at unchanged revision".into())),
                    _ => unreachable!(),
                };
                let result = epoch.observe(world, input, wall);
                assert_eq!(result.is_ok(), boundary == "revision");
                assert_eq!(
                    epoch.rules_for(&frame.bundle).is_some(),
                    boundary == "revision"
                );
                let playback = epoch.playback.as_mut().unwrap();
                assert!(playback.pending.is_none() && playback.voices.is_empty());
                assert!(world.get_entity(staged).is_err());
                assert_eq!(
                    playback
                        .controller
                        .random
                        .choose(1_000_003, &mut 100)
                        .unwrap(),
                    expected.choose(1_000_003, &mut 100).unwrap()
                );
            });
    }
    drop(app);
    let mut app = crate::native_campaign::tests::started_audio_app();
    drain_native_commands(&mut app);
    native(&mut app, "EndThematicMusic", &[]);
    native(&mut app, "StopMusic", &[pcx_script::Value::Bool(false)]);
    let frame = source_frame(&app);
    let reader = app
        .world()
        .resource::<crate::native_campaign::Chapters>()
        .audio_sources(app.world().resource::<crate::launch_gui::Launcher>())
        .unwrap()
        .unwrap()
        .1;
    let skin = reader
        .resolve_bounded("UI\\war3skins.txt", 1 << 20)
        .unwrap();
    let skin =
        crate::menu_skin::SkinTable::parse(std::str::from_utf8(&skin.bytes).unwrap()).unwrap();
    let key = f
        .rules
        .edition_key(f.rules.menu_start().0, frame.edition)
        .unwrap();
    let members = f
        .rules
        .tracks(&skin.unique_file(&key).unwrap().file)
        .unwrap();
    let mut expected = app
        .world()
        .resource::<Epoch>()
        .playback
        .as_ref()
        .unwrap()
        .controller
        .random
        .clone();
    expected.choose(1, &mut 100).unwrap();
    call_native(
        &mut app,
        "PlayMusicEx",
        &[
            pcx_script::Value::Str(members[0].clone().into()),
            pcx_script::Value::Int(5),
            pcx_script::Value::Int(0),
        ],
    );
    let sequence = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>()
        .host()
        .unwrap()
        .objects()
        .regular_music_commands()
        .unwrap()
        .back()
        .unwrap()
        .sequence;
    for _ in 0..512 {
        app.update();
        if app
            .world()
            .resource::<Epoch>()
            .playback
            .as_ref()
            .unwrap()
            .pending
            .as_ref()
            .is_some_and(|pending| pending.sequence == Some(sequence))
        {
            break;
        }
    }
    assert_eq!(
        app.world()
            .resource::<Epoch>()
            .playback
            .as_ref()
            .unwrap()
            .pending
            .as_ref()
            .unwrap()
            .sequence,
        Some(sequence)
    );
    compare_random(&app, &expected);
    app.world_mut()
        .resource_scope(|world, mut epoch: Mut<Epoch>| {
            let wall = crate::music_wall::sample().unwrap();
            assert!(epoch.observe(world, None, wall).is_err());
            epoch.observe(world, Some(Ok(frame.clone())), wall).unwrap();
        });
    for _ in 0..2 {
        let error = app
            .world_mut()
            .resource_scope(|world, mut epoch: Mut<Epoch>| {
                epoch.drive(world, crate::music_wall::sample().unwrap().milliseconds)
            })
            .unwrap_err();
        assert!(error.contains("retired audio epoch"));
        assert_eq!(
            app.world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>()
                .host()
                .unwrap()
                .objects()
                .regular_music_commands()
                .unwrap()
                .front()
                .unwrap()
                .sequence,
            sequence
        );
        compare_random(&app, &expected);
    }
    crate::campaign_audio_probe::begin(&mut app);
    crate::campaign_audio_probe::select_hero(&mut app);
    app.world_mut()
        .run_system_once(crate::campaign_ack::unit_voices)
        .unwrap();
    assert!(
        app.world()
            .resource::<crate::campaign_audio::CampaignAudio>()
            .last_error
            .as_deref()
            .unwrap()
            .contains("retired audio epoch")
    );
    let world = app.world_mut();
    assert_eq!(
        world
            .query::<&crate::campaign_audio::GameSound>()
            .iter(world)
            .count(),
        0
    );
    compare_random(&app, &expected);
    app.world_mut()
        .resource_mut::<crate::menu_settings::Backend>()
        .set_test_preferences(false);
    if f.rules.menu_start().2 < 0 {
        expected.choose(members.len(), &mut 100).unwrap();
    }
    app.world_mut()
        .run_system_once(crate::menu_audio::load)
        .unwrap();
    assert!(
        app.world().resource::<Epoch>().menu.is_some(),
        "current-epoch menu remains admitted after old producers retire"
    );
    compare_random(&app, &expected);
}

#[test]
#[ignore = "requires selected client/backend inputs and verified private null audio"]
fn retained_music_processed_eof_reopens_drained_sink_without_random_draw_and_keeps_next() {
    let f = fixture();
    let mut app = app();
    let mut p = Playback::new(
        RegularMusicController::new(Arc::clone(&f.rules), 77),
        [1; 32],
    );
    stage(
        &f,
        &mut p,
        &mut app,
        Operation::Play {
            name: "set".into(),
            position_ms: 333,
            fade_ms: 0,
        },
        0,
        1,
    );
    finish(&mut p, &mut app, 1);
    let old = p.controller.regular().unwrap().clone();
    assert!(!old.looping);
    let old_entity = p.voices[&old.serial].entity;
    // Force the backend's drained state; this does not measure the original EOF dispatch race.
    await_empty(&mut app, old_entity);
    assert!(app.world().get::<AudioSink>(old_entity).unwrap().empty());
    assert_eq!(
        p.completed(app.world_mut()),
        Some((MusicRole::Regular, old.serial))
    );
    let mut next = p.controller.clone();
    let effects = next
        .finished(MusicRole::Regular, old.serial, 7, &mut 100, None)
        .unwrap();
    p.stage(
        app.world_mut(),
        next,
        effects,
        None,
        Inputs {
            gain: &f.gain,
            loops: Some(&f.loops),
            user: 1.0,
            read: &|_, _| unreachable!(),
        },
    )
    .unwrap();
    assert_eq!(p.finish(app.world_mut()).unwrap(), Progress::Accepted(None));
    assert!(p.completed(app.world_mut()).is_none());
    let mut expected = p.controller.random.clone();
    stage(&f, &mut p, &mut app, Operation::Resume, 8, 2);
    finish(&mut p, &mut app, 2);
    let reopened = p.controller.regular().unwrap();
    assert_ne!(reopened.serial, old.serial);
    assert_eq!(reopened.path, old.path);
    assert_eq!(reopened.start_ms, 0);
    assert!(app.world().get_entity(old_entity).is_err());
    let entity = p.voices[&reopened.serial].entity;
    assert!(!app.world().get::<AudioSink>(entity).unwrap().empty());
    assert!(!app.world().get::<AudioSink>(entity).unwrap().is_paused());
    assert_eq!(
        p.controller.random.choose(1_000_003, &mut 100).unwrap(),
        expected.choose(1_000_003, &mut 100).unwrap()
    );
    let mut next = p.controller.clone();
    let effects = next.pump(7 + f.rules.next_interval_ms(), &mut 100).unwrap();
    assert_ne!(next.regular().unwrap().path, old.path);
    p.stage(
        app.world_mut(),
        next,
        effects,
        Some(3),
        Inputs {
            gain: &f.gain,
            loops: Some(&f.loops),
            user: 1.0,
            read: &|_, _| Ok((f.bytes.clone(), f.origin.clone())),
        },
    )
    .unwrap();
    finish(&mut p, &mut app, 3);
    assert!(app.world().get_entity(entity).is_err());

    let mut next = p.controller.clone();
    let effects = next
        .apply(
            Operation::Play {
                name: "single".into(),
                position_ms: 0,
                fade_ms: 0,
            },
            Some(vec!["single.wav".into()]),
            100,
            &mut 100,
        )
        .unwrap();
    p.stage(
        app.world_mut(),
        next,
        effects,
        Some(4),
        Inputs {
            gain: &f.gain,
            loops: Some(&f.loops),
            user: 1.0,
            read: &|_, _| Ok((f.bytes.clone(), f.origin.clone())),
        },
    )
    .unwrap();
    finish(&mut p, &mut app, 4);
    let looping = p.controller.regular().unwrap();
    assert!(looping.looping);
    assert!(
        app.world()
            .get::<AudioPlayer<LoopSource>>(p.voices[&looping.serial].entity)
            .is_some()
    );
    assert!(matches!(
        app.world()
            .get::<PlaybackSettings>(p.voices[&looping.serial].entity)
            .unwrap()
            .mode,
        bevy::audio::PlaybackMode::Once
    ));
    assert!(p.completed(app.world_mut()).is_none());
}

#[test]
#[ignore = "requires selected client/backend inputs and verified private null audio"]
fn retained_music_missing_audio_device_stays_explicitly_pending_without_acceptance() {
    let f = fixture();
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()));
    install_sources(&mut app);
    app.init_resource::<Assets<AudioSource>>();
    let mut p = Playback::new(
        RegularMusicController::new(Arc::clone(&f.rules), 77),
        [1; 32],
    );
    stage(
        &f,
        &mut p,
        &mut app,
        Operation::Play {
            name: "missing-device".into(),
            position_ms: 0,
            fade_ms: 0,
        },
        0,
        1,
    );
    for _ in 0..3 {
        app.update();
        assert_eq!(
            p.finish(app.world_mut()).unwrap(),
            Progress::AwaitingBackend { sequence: Some(1) }
        );
        assert!(p.waiting_for_backend && p.has_pending());
        assert!(p.controller.regular().is_none());
    }
}

#[test]
#[ignore = "requires selected client/backend, unchanged Human01 and verified private null audio"]
fn native_menu_and_unit_voice_callers_share_pending_music_draws_across_commit_failure_and_revision()
{
    let f = fixture();
    for outcome in ["commit", "failure", "revision"] {
        // Preferences deliberately arrive after source publication, while a decoder is staged.
        let mut app = crate::native_campaign::tests::started_audio_app();
        let frame = source_frame(&app);
        let mut expected = app
            .world()
            .resource::<Epoch>()
            .playback
            .as_ref()
            .unwrap()
            .controller
            .random
            .clone();
        app.world_mut()
            .resource_scope(|world, mut epoch: Mut<Epoch>| {
                let p = epoch.playback.as_mut().unwrap();
                p.retire(world);
                let mut next = p.controller.clone();
                let effects = next
                    .apply(
                        Operation::Play {
                            name: "pending".into(),
                            position_ms: 0,
                            fade_ms: 0,
                        },
                        Some(vec!["a.wav".into(), "b.wav".into()]),
                        0,
                        &mut 100,
                    )
                    .unwrap();
                p.stage(
                    world,
                    next,
                    effects,
                    None,
                    Inputs {
                        gain: &f.gain,
                        loops: Some(&f.loops),
                        user: 1.0,
                        read: &|_, _| Ok((f.bytes.clone(), f.origin.clone())),
                    },
                )
                .unwrap();
            });
        expected.choose(2, &mut 100).unwrap();
        let reader = app
            .world()
            .resource::<crate::native_campaign::Chapters>()
            .audio_sources(app.world().resource::<crate::launch_gui::Launcher>())
            .unwrap()
            .unwrap()
            .1;
        let skin = reader
            .resolve_bounded("UI\\war3skins.txt", 1 << 20)
            .unwrap();
        let skin =
            crate::menu_skin::SkinTable::parse(std::str::from_utf8(&skin.bytes).unwrap()).unwrap();
        let (name, _, index, _, _) = f.rules.menu_start();
        let key = f.rules.edition_key(name, frame.edition).unwrap();
        let members = f
            .rules
            .tracks(&skin.unique_file(&key).unwrap().file)
            .unwrap();
        if index < 0 {
            expected.choose(members.len(), &mut 100).unwrap();
        }
        app.world_mut()
            .resource_mut::<crate::menu_settings::Backend>()
            .set_test_preferences(false);
        app.world_mut()
            .run_system_once(crate::menu_audio::load)
            .unwrap();
        let world = app.world_mut();
        assert_eq!(
            world
                .query_filtered::<Entity, With<crate::menu_audio::MenuMusic>>()
                .iter(world)
                .count(),
            1,
            "menu publication: {}",
            world.resource::<crate::menu_screens::Navigation>().notice
        );
        compare_random(&app, &expected);
        acknowledge_hero(&mut app, &mut expected);
        match outcome {
            "commit" => {
                for _ in 0..8 {
                    let result = app
                        .world_mut()
                        .resource_scope(|world, mut epoch: Mut<Epoch>| {
                            epoch.playback.as_mut().unwrap().finish(world)
                        })
                        .unwrap();
                    if result == Progress::Accepted(None) {
                        break;
                    }
                    app.update();
                }
                assert!(
                    !app.world()
                        .resource::<Epoch>()
                        .playback
                        .as_ref()
                        .unwrap()
                        .has_pending()
                );
            }
            "failure" => app
                .world_mut()
                .resource_scope(|world, mut epoch: Mut<Epoch>| {
                    let p = epoch.playback.as_mut().unwrap();
                    let entity = p
                        .pending
                        .as_ref()
                        .unwrap()
                        .staged
                        .values()
                        .next()
                        .unwrap()
                        .entity;
                    world.despawn(entity);
                    assert!(p.finish(world).is_err());
                    assert!(p.finish(world).is_err());
                }),
            "revision" => app
                .world_mut()
                .resource_scope(|world, mut epoch: Mut<Epoch>| {
                    epoch
                        .observe(
                            world,
                            Some(Ok(SourceFrame {
                                revision: frame.revision + 1,
                                ..frame.clone()
                            })),
                            crate::music_wall::sample().unwrap(),
                        )
                        .unwrap();
                    assert!(!epoch.playback.as_ref().unwrap().has_pending());
                }),
            _ => unreachable!(),
        }
        compare_random(&app, &expected);
    }
}

struct Fixture {
    rules: Arc<RegularMusicRules>,
    gain: RegularMusicGain,
    loops: MusicLoopRules,
    origin: AssetIdentity,
    bytes: Vec<u8>,
}

fn fixture() -> Fixture {
    crate::campaign_audio_probe::assert_null_audio_environment();
    let path = std::env::var("PCX_WAR3_EXE").expect("PCX_WAR3_EXE");
    use std::io::Read;
    let mut bytes = Vec::new();
    let limit = pcx_play::client_module::CLIENT_IMAGE_BYTES;
    std::fs::File::open(&path)
        .unwrap()
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .unwrap();
    assert!(bytes.len() <= limit);
    let rules = Arc::new(
        RegularMusicRules::read(
            &bytes,
            pcx_play::campaign_inputs::client_identity(&bytes, &path),
        )
        .unwrap(),
    );
    let volume = Arc::new(crate::campaign_audio_probe::selected_regular_volume_rules());
    let backend_path = std::env::var("PCX_MUSIC_BACKEND").unwrap();
    let mut backend = Vec::new();
    std::fs::File::open(backend_path)
        .unwrap()
        .take((4 << 20) + 1)
        .read_to_end(&mut backend)
        .unwrap();
    assert!(backend.len() <= 4 << 20);
    let loops = MusicLoopRules::read(&bytes, &backend).unwrap();
    assert_eq!(rules.image_sha256, volume.image_sha256);
    let gain = RegularMusicGain {
        request: pcx_play::playercolor_host::RegularMusicVolumeRequest {
            level: volume.initial_level(),
            rules: volume,
            revision: 0,
        },
        group: 1.0,
    };
    Fixture {
        rules,
        gain,
        loops,
        origin: AssetIdentity {
            source: "synthetic silent PCM".into(),
            edition: "fixture".into(),
            version: "fixture".into(),
        },
        bytes: wave(),
    }
}

fn wave() -> Vec<u8> {
    let rate = 8000u32;
    let data = vec![0u8; rate as usize * 8 * 2];
    let mut wave = Vec::new();
    wave.extend_from_slice(b"RIFF");
    wave.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    wave.extend_from_slice(b"WAVEfmt ");
    wave.extend_from_slice(&16u32.to_le_bytes());
    wave.extend_from_slice(&1u16.to_le_bytes());
    wave.extend_from_slice(&1u16.to_le_bytes());
    wave.extend_from_slice(&rate.to_le_bytes());
    wave.extend_from_slice(&(rate * 2).to_le_bytes());
    wave.extend_from_slice(&2u16.to_le_bytes());
    wave.extend_from_slice(&16u16.to_le_bytes());
    wave.extend_from_slice(b"data");
    wave.extend_from_slice(&(data.len() as u32).to_le_bytes());
    wave.extend(data);
    wave
}

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        bevy::audio::AudioPlugin::default(),
    ));
    install_sources(&mut app);
    app
}

fn stage(f: &Fixture, p: &mut Playback, app: &mut App, op: Operation, at: u32, sequence: u64) {
    let resolved = if let Operation::Play { name, .. } = &op {
        Some(vec![format!("{name}-a.wav"), format!("{name}-b.wav")])
    } else {
        None
    };
    let mut next = p.controller.clone();
    let effects = next.apply(op, resolved, at, &mut 1000).unwrap();
    p.stage(
        app.world_mut(),
        next,
        effects,
        Some(sequence),
        Inputs {
            gain: &f.gain,
            loops: Some(&f.loops),
            user: 0.75,
            read: &|_, limit| {
                assert!(f.bytes.len() as u64 <= limit);
                Ok((f.bytes.clone(), f.origin.clone()))
            },
        },
    )
    .unwrap();
}

fn finish(p: &mut Playback, app: &mut App, sequence: u64) {
    for _ in 0..8 {
        if p.finish(app.world_mut()).unwrap() == Progress::Accepted(Some(sequence)) {
            return;
        }
        app.update();
    }
    panic!("null music sink did not become ready within eight application updates");
}

fn pump(f: &Fixture, p: &mut Playback, app: &mut App, at: u32) {
    let mut next = p.controller.clone();
    let effects = next.pump(at, &mut 1000).unwrap();
    p.stage(
        app.world_mut(),
        next,
        effects,
        None,
        Inputs {
            gain: &f.gain,
            loops: Some(&f.loops),
            user: 0.75,
            read: &|_, limit| {
                assert!(f.bytes.len() as u64 <= limit);
                Ok((f.bytes.clone(), f.origin.clone()))
            },
        },
    )
    .unwrap();
    assert_eq!(p.finish(app.world_mut()).unwrap(), Progress::Accepted(None));
}

#[test]
#[ignore = "requires selected client/backend inputs and verified private null audio"]
fn retained_music_sink_preserves_cursor_and_applies_source_fade_and_theme_handoff() {
    let f = fixture();
    let mut app = app();
    let mut p = Playback::new(
        RegularMusicController::new(Arc::clone(&f.rules), 77),
        [1; 32],
    );
    stage(
        &f,
        &mut p,
        &mut app,
        Operation::Play {
            name: "first".into(),
            position_ms: 12,
            fade_ms: 0,
        },
        0,
        1,
    );
    assert_eq!(
        p.finish(app.world_mut()).unwrap(),
        Progress::AwaitingBackend { sequence: Some(1) }
    );
    finish(&mut p, &mut app, 1);
    assert!(!p.waiting_for_backend);
    let serial = p.controller.regular().unwrap().serial;
    let entity = p.voices[&serial].entity;
    assert!(!app.world().get::<PlaybackSettings>(entity).unwrap().paused);
    assert!(!app.world().get::<AudioSink>(entity).unwrap().is_paused());
    stage(&f, &mut p, &mut app, Operation::Stop { fade: false }, 0, 2);
    finish(&mut p, &mut app, 2);
    assert!(app.world().get::<AudioSink>(entity).unwrap().is_paused());
    stage(&f, &mut p, &mut app, Operation::Seek(333), 0, 3);
    finish(&mut p, &mut app, 3);
    let position = app.world().get::<AudioSink>(entity).unwrap().position();
    assert!(position.abs_diff(Duration::from_millis(333)) <= Duration::from_secs_f64(1.0 / 8000.0));
    stage(
        &f,
        &mut p,
        &mut app,
        Operation::Theme {
            path: "theme.wav".into(),
            position_ms: 20,
        },
        0,
        4,
    );
    finish(&mut p, &mut app, 4);
    assert_eq!(p.voices[&serial].entity, entity);
    assert!(app.world().get::<AudioSink>(entity).unwrap().is_paused());
    stage(&f, &mut p, &mut app, Operation::EndTheme, 0, 5);
    finish(&mut p, &mut app, 5);
    assert_eq!(p.voices[&serial].entity, entity);
    assert!(!app.world().get::<AudioSink>(entity).unwrap().is_paused());
    assert_eq!(p.voices.len(), 1);
    stage(&f, &mut p, &mut app, Operation::Stop { fade: true }, 0, 6);
    finish(&mut p, &mut app, 6);
    let interval = f.rules.fade_interval_ms();
    pump(&f, &mut p, &mut app, interval);
    pump(&f, &mut p, &mut app, interval + f.rules.default_fade_ms());
    assert!(p.controller.regular().unwrap().playing);
    assert_eq!(p.controller.effective_level(), 0);
    let expected = f.gain.request.rules.gain(0, f.gain.group, 0.75).unwrap();
    assert_eq!(
        app.world().get::<AudioSink>(entity).unwrap().volume(),
        Volume::Linear(expected)
    );
    pump(
        &f,
        &mut p,
        &mut app,
        interval * 2 + f.rules.default_fade_ms(),
    );
    assert!(app.world().get::<AudioSink>(entity).unwrap().is_paused());
    assert_eq!(p.voices[&serial].entity, entity);
    p.retire(app.world_mut());
    assert!(app.world().get_entity(entity).is_err());
    for phase in ["pending", "live"] {
        p.source(app.world_mut(), Some([2; 32]));
        let mut next = p.controller.clone();
        let effects = next
            .apply(
                Operation::Play {
                    name: "loop".into(),
                    position_ms: 0,
                    fade_ms: 0,
                },
                Some(vec!["loop.wav".into()]),
                0,
                &mut 100,
            )
            .unwrap();
        p.stage(
            app.world_mut(),
            next,
            effects,
            Some(11),
            Inputs {
                gain: &f.gain,
                loops: Some(&f.loops),
                user: 1.0,
                read: &|_, _| Ok((f.bytes.clone(), f.origin.clone())),
            },
        )
        .unwrap();
        let entity = if phase == "live" {
            finish(&mut p, &mut app, 11);
            p.voices[&p.controller.regular().unwrap().serial].entity
        } else {
            p.pending
                .as_ref()
                .unwrap()
                .staged
                .values()
                .next()
                .unwrap()
                .entity
        };
        let handle = app
            .world()
            .get::<AudioPlayer<LoopSource>>(entity)
            .unwrap()
            .0
            .clone();
        app.world_mut()
            .resource_mut::<Assets<LoopSource>>()
            .remove(handle.id());
        assert!(
            p.finish(app.world_mut())
                .unwrap_err()
                .contains("source disappeared")
        );
        assert!(p.fault.is_some());
        if phase == "live" {
            assert!(app.world().get::<AudioSink>(entity).is_some());
        }
        p.source(app.world_mut(), None);
        assert!(p.fault.is_none());
    }
}

#[test]
#[ignore = "requires selected client/backend inputs and verified private null audio"]
fn retained_music_decoder_and_async_asset_failures_keep_old_voice_and_do_not_replay() {
    let f = fixture();
    let mut app = app();
    let mut p = Playback::new(
        RegularMusicController::new(Arc::clone(&f.rules), 77),
        [1; 32],
    );
    stage(
        &f,
        &mut p,
        &mut app,
        Operation::Play {
            name: "first".into(),
            position_ms: 0,
            fade_ms: 0,
        },
        0,
        1,
    );
    finish(&mut p, &mut app, 1);
    let serial = p.controller.regular().unwrap().serial;
    let entity = p.voices[&serial].entity;
    let old_random = p.controller.random.clone();
    let mut next = p.controller.clone();
    let effects = next
        .apply(
            Operation::Play {
                name: "bad".into(),
                position_ms: 0,
                fade_ms: 0,
            },
            Some(vec!["bad-a".into(), "bad-b".into()]),
            0,
            &mut 100,
        )
        .unwrap();
    assert!(
        p.stage(
            app.world_mut(),
            next,
            effects,
            Some(2),
            Inputs {
                gain: &f.gain,
                loops: Some(&f.loops),
                user: 0.75,
                read: &|_, limit| {
                    assert!(limit >= 12);
                    Ok((vec![0; 12], f.origin.clone()))
                },
            }
        )
        .is_err()
    );
    assert_eq!(
        p.controller.random.clone().choose(31, &mut 100).unwrap(),
        old_random.clone().choose(31, &mut 100).unwrap()
    );
    assert_eq!(p.voices[&serial].entity, entity);
    assert!(p.pending.is_none());
    stage(
        &f,
        &mut p,
        &mut app,
        Operation::Play {
            name: "replacement".into(),
            position_ms: 0,
            fade_ms: 0,
        },
        0,
        2,
    );
    let staged = p
        .pending
        .as_ref()
        .unwrap()
        .staged
        .values()
        .next()
        .unwrap()
        .entity;
    let handle = app.world().get::<AudioPlayer>(staged).unwrap().0.clone();
    app.world_mut()
        .resource_mut::<Assets<AudioSource>>()
        .remove(handle.id());
    assert!(
        p.finish(app.world_mut())
            .unwrap_err()
            .contains("source disappeared")
    );
    assert_eq!(p.voices[&serial].entity, entity);
    assert!(app.world().get::<AudioSink>(entity).is_some());
    assert_eq!(p.controller.regular().unwrap().serial, serial);
    let fault = p.fault.clone();
    assert!(p.finish(app.world_mut()).is_err());
    assert_eq!(p.fault, fault);
    assert!(p.pending.is_none());
    p.source(app.world_mut(), None);
    assert!(p.fault.is_none());
    assert!(p.voices.is_empty());
    assert!(app.world().get_entity(entity).is_err());
}

#[test]
#[ignore = "requires selected client/backend inputs and verified private null audio"]
fn retained_music_pending_commit_failure_and_retirement_preserve_intervening_audio_draws() {
    let f = fixture();
    for outcome in ["commit", "failure", "retirement"] {
        let mut app = app();
        let mut p = Playback::new(
            RegularMusicController::new(Arc::clone(&f.rules), 77),
            [1; 32],
        );
        let mut expected = p.controller.random.clone();
        expected.choose(2, &mut 100).unwrap();
        stage(
            &f,
            &mut p,
            &mut app,
            Operation::Play {
                name: "first".into(),
                position_ms: 0,
                fade_ms: 0,
            },
            0,
            1,
        );
        assert_eq!(
            p.controller.random.choose(5, &mut 100).unwrap(),
            expected.choose(5, &mut 100).unwrap()
        );
        assert_eq!(
            p.controller
                .random
                .choose_non_repeating("ack", 3, &mut 100)
                .unwrap(),
            expected.choose_non_repeating("ack", 3, &mut 100).unwrap()
        );
        match outcome {
            "commit" => finish(&mut p, &mut app, 1),
            "failure" => {
                let entity = p
                    .pending
                    .as_ref()
                    .unwrap()
                    .staged
                    .values()
                    .next()
                    .unwrap()
                    .entity;
                app.world_mut().despawn(entity);
                assert!(p.finish(app.world_mut()).is_err());
                assert!(p.finish(app.world_mut()).is_err());
            }
            "retirement" => p.source(app.world_mut(), None),
            _ => unreachable!(),
        }
        assert_eq!(
            p.controller.random.clone().choose(31, &mut 100).unwrap(),
            expected.clone().choose(31, &mut 100).unwrap(),
            "{outcome}"
        );
        p.source(app.world_mut(), Some([2; 32]));
        assert_eq!(
            p.controller.random.choose(31, &mut 100).unwrap(),
            expected.choose(31, &mut 100).unwrap(),
            "{outcome} after new generation"
        );
        assert!(p.pending.is_none());
        assert!(p.voices.is_empty());
    }
}

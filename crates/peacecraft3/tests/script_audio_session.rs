//! Registered playback against a host published by the production factory.
use peacecraft3::map_requests;
#[path = "support/map_admission_cases.rs"]
mod admission;
#[path = "support/map_binding_cases.rs"]
mod binding;
#[path = "support/browser_archive_source_cases.rs"]
mod browser_archive_source_cases;
#[path = "support/browser_bounded_cases.rs"]
mod browser_bounded_cases;
#[path = "support/browser_configuration_cases.rs"]
mod browser_configuration_cases;
#[path = "support/browser_configuration_json_cases.rs"]
mod browser_configuration_json_cases;
#[path = "support/browser_mpq_fixtures.rs"]
mod browser_mpq_fixtures;
#[path = "support/browser_read_cases.rs"]
mod browser_read_cases;
#[path = "../src/button_actions.rs"]
mod button_actions;
#[path = "support/script_session_cases.rs"]
mod consumer_cases;
#[path = "support/map_fixtures.rs"]
mod fixtures;
#[path = "support/mpq_extraction_cases.rs"]
mod mpq_extraction_cases;
#[cfg(not(target_arch = "wasm32"))]
#[path = "support/mpq_file_cases.rs"]
mod mpq_file_cases;
#[path = "support/mpq_extraction_fixtures.rs"]
mod mpq_fixtures;
#[path = "support/script_session_tests.rs"]
mod script_session_tests;
#[path = "support/source_budget_cases.rs"]
mod source_budget_cases;
#[path = "../src/map_source_controls.rs"]
mod source_controls;
#[path = "support/callback_ticket_cases.rs"]
mod ticket_cases;

#[cfg(not(target_arch = "wasm32"))]
mod published_audio {
    include!("support/campaign_startup_cases.rs");

    use pcx_play::file_sounds::{Budget as SoundBudget, SoundInput};
    use pcx_play::map_startup_host;
    use pcx_script::Host;
    use peacecraft3::script_audio::{ScriptAudioLimits, ScriptAudioReport, ScriptVoice};

    const SOUND_SCRIPT: &str = "\
globals
integer marker = 1
sound voice = null
sound looped = null
endglobals
function config takes nothing returns nothing
endfunction
function main takes nothing returns nothing
set voice = CreateSound(\"Media\\\\tone.wav\", false, false, false, 0, 0, \"\")
set looped = CreateSound(\"Media\\\\tone.wav\", true, false, false, 0, 0, \"\")
call StartSound(voice)
call StartSound(looped)
endfunction
";

    fn tone() -> Vec<u8> {
        let mut format = vec![];
        format.extend(1u16.to_le_bytes());
        format.extend(1u16.to_le_bytes());
        format.extend(8_000u32.to_le_bytes());
        format.extend(16_000u32.to_le_bytes());
        format.extend(2u16.to_le_bytes());
        format.extend(16u16.to_le_bytes());
        let mut bytes = b"RIFF".to_vec();
        let chunks = {
            let mut fmt = b"fmt ".to_vec();
            fmt.extend(16u32.to_le_bytes());
            fmt.extend(&format);
            let mut data = b"data".to_vec();
            data.extend(4u32.to_le_bytes());
            data.extend([0x10, 0x20, 0x30, 0x40]);
            [fmt, data]
        };
        let size = 4 + chunks.iter().map(|chunk| chunk.len()).sum::<usize>();
        bytes.extend(u32::try_from(size).unwrap().to_le_bytes());
        bytes.extend(b"WAVE");
        for chunk in &chunks {
            bytes.extend(chunk);
        }
        bytes
    }

    fn sound_bank(pcm: &[u8]) -> file_sounds::Assets {
        file_sounds::Assets::read(
            &[SoundInput {
                member: r"Media\tone.wav",
                asset: ScriptAsset {
                    label: "synthetic:pcm",
                    bytes: pcm,
                },
                origin: AssetIdentity {
                    edition: "synthetic".into(),
                    version: "test".into(),
                    source: "synthetic:archive".into(),
                },
            }],
            SoundBudget {
                resources: 1,
                resource_bytes: pcm.len(),
                total_bytes: pcm.len(),
                metadata_bytes: 256,
            },
        )
        .expect("pcm")
    }

    fn app(limits: ScriptAudioLimits) -> App {
        let mut app = App::new();
        peacecraft3::map_requests::install(&mut app);
        peacecraft3::campaign_startup::install(&mut app);
        peacecraft3::script_audio::install(&mut app);
        app.insert_resource(limits);
        app
    }

    fn deliver_sound(
        app: &mut App,
        script: &str,
        pcm: &[u8],
        revision: u64,
    ) -> Arc<dyn pcx_sim::world::ObjectCatalog + Send + Sync> {
        let map = fixture(script);
        let resolver = map.resolver();
        let bound = bind_map(&resolver, map.input(revision), map.limits()).expect("bind");
        let catalog = Arc::clone(&bound.catalog.0);
        let bytes = bound.catalog_map_bytes.clone();
        let identity = map.map_id.clone();
        let mut inputs = host_inputs(Arc::clone(&catalog));
        inputs.file_sounds = sound_bank(pcm);
        app.insert_resource(MapRequests {
            bindings: Some(bound),
            ..Default::default()
        });
        app.insert_resource(CampaignHostSupply {
            revision,
            inputs: Some(inputs),
        });
        app.world_mut().write_message(MapRequest {
            label: "chosen map label".into(),
            map_member: pcx_assets::AssetPath::new("selected map.w3m").expect("member"),
            identity,
            binding_revision: revision,
            payload: MapPayload::Shared(bytes),
        });
        app.update();
        catalog
    }

    fn voices(app: &mut App) -> Vec<(Entity, bool)> {
        let world = app.world_mut();
        let mut query = world.query::<(Entity, &ScriptVoice)>();
        query
            .iter(world)
            .map(|(entity, voice)| (entity, voice.looping))
            .collect()
    }

    #[derive(Component)]
    struct Bystander;

    #[derive(Debug)]
    struct Hold;

    impl std::fmt::Display for Hold {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("candidate host refused")
        }
    }

    impl std::error::Error for Hold {}

    #[test]
    fn published_factory_host_plays_reuses_and_does_not_replay() {
        let pcm = tone();
        let mut app = app(ScriptAudioLimits {
            max_staged_bytes: pcm.len(),
            max_live_bytes: pcm.len(),
        });
        deliver_sound(&mut app, SOUND_SCRIPT, &pcm, 7);
        assert!(
            app.world()
                .resource::<peacecraft3::campaign_startup::CampaignStartup>()
                .last_error
                .is_none()
        );
        let rows = voices(&mut app);
        assert_eq!(rows.len(), 2);
        assert_eq!(app.world().resource::<ScriptAudioReport>().played, 2);
        assert_eq!(
            app.world().resource::<Assets<AudioSource>>().iter().count(),
            1
        );
        assert!(
            app.world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>()
                .host()
                .unwrap()
                .objects_mut()
                .pending_sound_starts()
                .is_empty()
        );
        let played = app.world().resource::<ScriptAudioReport>().played;
        app.update();
        assert_eq!(app.world().resource::<ScriptAudioReport>().played, played);
        assert_eq!(voices(&mut app).len(), 2);
    }

    #[test]
    fn same_context_replace_retires_only_the_previous_voices() {
        let pcm = tone();
        let mut app = app(ScriptAudioLimits {
            max_staged_bytes: pcm.len(),
            max_live_bytes: pcm.len(),
        });
        let catalog = deliver_sound(&mut app, SOUND_SCRIPT, &pcm, 3);
        let previous: Vec<_> = voices(&mut app)
            .into_iter()
            .map(|(entity, _)| entity)
            .collect();
        assert_eq!(previous.len(), 2);
        let context = Arc::clone(
            app.world()
                .resource::<MapScriptSession<MapStartupHost>>()
                .context()
                .unwrap(),
        );
        let config = context.scripts.program().function_id("config").unwrap();
        let main = context.scripts.program().function_id("main").unwrap();
        let mut inputs = host_inputs(catalog);
        inputs.file_sounds = sound_bank(&pcm);
        app.world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .replace(
                Arc::clone(&context),
                move |context| map_startup_host::assemble(context, inputs),
                &[config, main],
            )
            .unwrap();
        app.update();
        let rows = voices(&mut app);
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|(entity, _)| !previous.contains(entity)));
        let witness = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .session_witness()
            .unwrap();
        let world = app.world_mut();
        let mut query = world.query::<&ScriptVoice>();
        assert!(
            query
                .iter(world)
                .all(|voice| std::sync::Weak::ptr_eq(&voice.session, &witness))
        );
    }

    #[test]
    fn failed_replace_keeps_the_published_voices_and_queue() {
        let pcm = tone();
        let mut app = app(ScriptAudioLimits {
            max_staged_bytes: pcm.len(),
            max_live_bytes: pcm.len(),
        });
        deliver_sound(&mut app, SOUND_SCRIPT, &pcm, 4);
        let before = voices(&mut app);
        let context = Arc::clone(
            app.world()
                .resource::<MapScriptSession<MapStartupHost>>()
                .context()
                .unwrap(),
        );
        let voice = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .global("voice")
            .unwrap();
        app.world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .host()
            .unwrap()
            .objects_mut()
            .native("StartSound", &[voice])
            .unwrap();
        let pending = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .host()
            .unwrap()
            .objects_mut()
            .pending_sound_starts()
            .to_vec();
        assert_eq!(pending.len(), 1);
        let error = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .replace(context, |_| Err(Hold), &[])
            .unwrap_err();
        assert!(
            error.to_string().contains("candidate host refused"),
            "{error}"
        );
        assert_eq!(
            app.world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>()
                .host()
                .unwrap()
                .objects_mut()
                .pending_sound_starts(),
            pending.as_slice()
        );
        assert_eq!(voices(&mut app), before);
        let played = app.world().resource::<ScriptAudioReport>().played;
        app.update();
        assert_eq!(app.world().resource::<ScriptAudioReport>().played, played);
        assert_eq!(voices(&mut app), before);
        assert!(
            app.world()
                .resource::<ScriptAudioReport>()
                .last_refusal
                .as_deref()
                .unwrap()
                .contains("repeat")
        );
        assert!(
            app.world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>()
                .host()
                .unwrap()
                .objects_mut()
                .pending_sound_starts()
                .is_empty()
        );
    }

    #[test]
    fn clear_drops_script_voices_and_leaves_other_entities() {
        let pcm = tone();
        let mut app = app(ScriptAudioLimits {
            max_staged_bytes: pcm.len(),
            max_live_bytes: pcm.len(),
        });
        deliver_sound(&mut app, SOUND_SCRIPT, &pcm, 5);
        app.world_mut().spawn(Bystander);
        app.world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .clear();
        app.update();
        assert!(voices(&mut app).is_empty());
        let world = app.world_mut();
        let mut query = world.query::<&Bystander>();
        assert_eq!(query.iter(world).count(), 1);
    }

    #[test]
    fn finished_oneshot_is_reclaimed_and_the_looping_voice_stays() {
        let pcm = tone();
        let mut app = app(ScriptAudioLimits {
            max_staged_bytes: pcm.len(),
            max_live_bytes: pcm.len(),
        });
        deliver_sound(&mut app, SOUND_SCRIPT, &pcm, 6);
        let oneshots: Vec<_> = voices(&mut app)
            .into_iter()
            .filter(|(_, looping)| !looping)
            .map(|(entity, _)| entity)
            .collect();
        assert_eq!(oneshots.len(), 1);
        app.world_mut()
            .entity_mut(oneshots[0])
            .remove::<AudioPlayer<AudioSource>>();
        app.update();
        let rows = voices(&mut app);
        assert_eq!(rows.len(), 1);
        assert!(rows[0].1);
        assert_eq!(
            app.world().resource::<Assets<AudioSource>>().iter().count(),
            1
        );
    }

    #[test]
    fn staged_limit_leaves_the_factory_queue_unpublished() {
        let pcm = tone();
        let mut app = app(ScriptAudioLimits {
            max_staged_bytes: 0,
            max_live_bytes: pcm.len(),
        });
        deliver_sound(&mut app, SOUND_SCRIPT, &pcm, 8);
        assert!(voices(&mut app).is_empty());
        assert_eq!(app.world().resource::<ScriptAudioReport>().played, 0);
        let pending = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .host()
            .unwrap()
            .objects_mut()
            .pending_sound_starts()
            .len();
        assert_eq!(pending, 2);
        let error = app
            .world()
            .resource::<ScriptAudioReport>()
            .admission_error
            .as_deref()
            .unwrap();
        assert!(error.contains("staging limit"), "{error}");
    }
    #[test]
    fn missing_limits_only_report_when_a_start_is_queued() {
        let pcm = tone();
        let mut app = app(ScriptAudioLimits {
            max_staged_bytes: 0,
            max_live_bytes: 0,
        });
        app.world_mut().remove_resource::<ScriptAudioLimits>();
        let silent = SOUND_SCRIPT
            .replace("call StartSound(voice)", "")
            .replace("call StartSound(looped)", "");
        deliver_sound(&mut app, &silent, &pcm, 7);
        assert!(
            app.world()
                .resource::<ScriptAudioReport>()
                .admission_error
                .is_none()
        );
        let sound = app
            .world()
            .resource::<MapScriptSession<MapStartupHost>>()
            .global("voice")
            .unwrap();
        app.world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .host()
            .unwrap()
            .objects_mut()
            .native("StartSound", &[sound])
            .unwrap();
        app.update();
        assert!(
            app.world()
                .resource::<ScriptAudioReport>()
                .admission_error
                .as_deref()
                .unwrap()
                .contains("not supplied")
        );
        assert_eq!(
            app.world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>()
                .host()
                .unwrap()
                .objects_mut()
                .pending_sound_starts()
                .len(),
            1
        );
        assert!(voices(&mut app).is_empty());
    }

    #[test]
    fn repeated_active_start_is_reported_until_its_playback_behavior_is_admitted() {
        let mut app = app(ScriptAudioLimits {
            max_staged_bytes: 1024,
            max_live_bytes: 1024,
        });
        let pcm = tone();
        let script = SOUND_SCRIPT.replace(
            "call StartSound(looped)",
            "call StartSound(looped)\ncall StartSound(looped)",
        );
        deliver_sound(&mut app, &script, &pcm, 7);
        assert_eq!(voices(&mut app).len(), 2);
        assert_eq!(app.world().resource::<ScriptAudioReport>().refusal_count, 1);
        let looping = {
            let mut query = app.world_mut().query::<&ScriptVoice>();
            query
                .iter(app.world())
                .find(|voice| voice.looping)
                .unwrap()
                .handle
        };
        app.world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .host()
            .unwrap()
            .objects_mut()
            .native("StartSound", &[pcx_script::Value::Handle(looping)])
            .unwrap();
        app.update();
        assert_eq!(
            voices(&mut app).len(),
            2,
            "an unmeasured repeat must not create an extra looping voice"
        );
        let report = app.world().resource::<ScriptAudioReport>();
        assert_eq!(report.refusal_count, 2);
        assert!(
            report
                .last_refusal
                .as_deref()
                .is_some_and(|reason| reason.contains("repeat"))
        );
    }

    #[test]
    fn a_finished_handle_can_start_again_in_the_same_reclaim_pass() {
        let pcm = tone();
        let mut app = app(ScriptAudioLimits {
            max_staged_bytes: pcm.len(),
            max_live_bytes: pcm.len(),
        });
        deliver_sound(&mut app, SOUND_SCRIPT, &pcm, 7);
        let (previous, handle) = {
            let mut query = app.world_mut().query::<(Entity, &ScriptVoice)>();
            query
                .iter(app.world())
                .find(|(_, voice)| !voice.looping)
                .map(|(entity, voice)| (entity, voice.handle))
                .unwrap()
        };
        app.world_mut().entity_mut(previous).remove::<AudioPlayer>();
        app.world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .host()
            .unwrap()
            .objects_mut()
            .native("StartSound", &[pcx_script::Value::Handle(handle)])
            .unwrap();
        app.update();
        assert!(app.world().get_entity(previous).is_err());
        assert_eq!(voices(&mut app).len(), 2);
        assert_eq!(app.world().resource::<ScriptAudioReport>().played, 3);
        assert_eq!(app.world().resource::<Assets<AudioSource>>().len(), 1);
    }
}

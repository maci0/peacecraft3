use super::*;
use std::cell::RefCell;
use wasm_bindgen::prelude::wasm_bindgen;

thread_local! {
    static DEVICE_APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

#[wasm_bindgen]
pub fn browser_script_audio_device_prepare() -> String {
    DEVICE_APP.with_borrow_mut(|slot| {
        assert!(slot.is_none(), "stop the previous audio fixture first");
        let ceiling = tone().len().max(
            include_bytes!("../../../pcx-play/tests/fixtures/synthetic-mp3/mono-xing.mp3").len(),
        );
        let mut app = app(ScriptAudioLimits {
            max_staged_bytes: ceiling,
            max_live_bytes: ceiling,
        });
        app.add_plugins((
            MinimalPlugins,
            bevy::asset::AssetPlugin::default(),
            bevy::audio::AudioPlugin::default(),
        ));
        app.update();
        let observed = observe(&mut app);
        *slot = Some(app);
        observed
    })
}

#[wasm_bindgen]
pub fn browser_script_audio_device_start(mp3: bool) -> String {
    DEVICE_APP.with_borrow_mut(|slot| {
        let app = slot.as_mut().expect("prepare before the user gesture");
        let bytes: &[u8] = if mp3 {
            include_bytes!("../../../pcx-play/tests/fixtures/synthetic-mp3/mono-xing.mp3")
        } else {
            &tone()
        };
        let member = if mp3 {
            r"Media\tone.mp3"
        } else {
            r"Media\tone.wav"
        };
        let sounds = file_sounds::Assets::read_encoded(
            &[SoundInput {
                member,
                asset: ScriptAsset {
                    label: "synthetic:device",
                    bytes,
                },
                origin: AssetIdentity {
                    edition: "synthetic".into(),
                    version: "test".into(),
                    source: "synthetic:device".into(),
                },
            }],
            SoundBudget {
                resources: 1,
                resource_bytes: bytes.len(),
                total_bytes: bytes.len(),
                metadata_bytes: 256,
            },
            file_sounds::Mp3Budget {
                max_input_allocation_bytes: bytes.len() + pcx_play::mp3_frame::DECODER_RING_BYTES,
                max_decoder_steps: 512,
                max_packets: 128,
                max_frames: 200_000,
                max_samples: 400_000,
            },
        )
        .expect("synthetic encoded sound");
        let script = if mp3 {
            SOUND_SCRIPT.replace("tone.wav", "tone.mp3")
        } else {
            SOUND_SCRIPT.into()
        };
        deliver_sound_bank(app, &script, sounds, 1);
        observe(app)
    })
}

fn observe(app: &mut App) -> String {
    let world = app.world_mut();
    let mut query = world.query::<(&ScriptVoice, Option<&AudioSink>)>();
    let voices: Vec<_> = query.iter(world).collect();
    let count = voices.len();
    let looping_sinks = voices
        .iter()
        .filter(|(voice, sink)| voice.looping && sink.is_some())
        .count();
    let encoded_mp3 = voices.iter().filter(|(voice, _)| voice.encoded_mp3).count();
    let queued = world
        .resource_mut::<MapScriptSession<MapStartupHost>>()
        .host()
        .map(|host| host.objects_mut().pending_sound_starts().len());
    let report = world.resource::<ScriptAudioReport>();
    serde_json::json!({
        "voices": count,
        "looping_sinks": looping_sinks,
        "encoded_mp3": encoded_mp3,
        "source_count": world.resource::<Assets<AudioSource>>().len(),
        "queued": queued,
        "played": report.played,
        "refusals": report.refusal_count,
        "admission_error": report.admission_error,
        "startup_error": world.resource::<CampaignStartup>().last_error.as_ref().map(ToString::to_string),
    }).to_string()
}

#[wasm_bindgen]
pub fn browser_script_audio_device_update() -> String {
    DEVICE_APP.with_borrow_mut(|slot| {
        let app = slot.as_mut().expect("start the audio fixture first");
        app.update();
        observe(app)
    })
}

#[wasm_bindgen]
pub fn browser_script_audio_device_stop() -> String {
    DEVICE_APP.with_borrow_mut(|slot| {
        let mut app = slot.take().expect("start the audio fixture first");
        app.world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .clear();
        app.update();
        observe(&mut app)
    })
}

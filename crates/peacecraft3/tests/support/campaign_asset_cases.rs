//! Executed lighting arguments → retained resolver → published Host.
use super::*;
use crate::binding::Reply;
use std::collections::BTreeMap;

const SCRIPT: &str = r#"
globals
string folder = "Scene/"
endglobals
function config takes nothing returns nothing
endfunction
function main takes nothing returns nothing
call SetDayNightModels(folder + "Terrain.MDL", folder + "Units.mdx")
endfunction
function swap takes nothing returns nothing
call SetDayNightModels(folder + "Units.mdx", folder + "Terrain.MDL")
endfunction
function missing takes nothing returns nothing
call SetDayNightModels(folder + "Terrain.MDL", folder + "Missing.mdl")
endfunction
function fresh takes nothing returns nothing
call SetDayNightModels(folder + "Later.MDL", folder + "Units.mdx")
endfunction
function choose takes string terrain returns nothing
call SetDayNightModels(terrain, folder + "Units.mdx")
endfunction
"#;

fn model(colour: f32) -> Vec<u8> {
    fn chunk(out: &mut Vec<u8>, tag: &[u8; 4], bytes: &[u8]) {
        out.extend(tag);
        out.extend((bytes.len() as u32).to_le_bytes());
        out.extend(bytes);
    }
    let mut bytes = b"MDLX".to_vec();
    chunk(&mut bytes, b"VERS", &800u32.to_le_bytes());
    let mut light = vec![0; 144];
    light[..4].copy_from_slice(&144u32.to_le_bytes());
    light[4..8].copy_from_slice(&96u32.to_le_bytes());
    light[8..13].copy_from_slice(b"light");
    light[92..96].copy_from_slice(&(-1i32).to_le_bytes());
    light[100..104].copy_from_slice(&1u32.to_le_bytes());
    light[112..116].copy_from_slice(&colour.to_le_bytes());
    light[124..128].copy_from_slice(&1f32.to_le_bytes());
    chunk(&mut bytes, b"LITE", &light);
    chunk(&mut bytes, b"PIVT", &[0; 12]);
    bytes
}

fn models() -> BTreeMap<String, Reply> {
    BTreeMap::from([
        ("Scene/Terrain.mdx".into(), Reply::Bytes(model(0.125))),
        ("Scene/Units.mdx".into(), Reply::Bytes(model(0.875))),
        ("Scene/Later.mdx".into(), Reply::Bytes(model(0.5))),
    ])
}

fn app() -> App {
    let mut app = App::new();
    peacecraft3::map_requests::install(&mut app);
    peacecraft3::campaign_startup::install(&mut app);
    app
}

fn deliver(
    app: &mut App,
    script: &str,
    assets: BTreeMap<String, Reply>,
    revision: u64,
) -> Arc<std::sync::atomic::AtomicUsize> {
    let f = fixture(script);
    let asset_count = assets.len();
    let resolver = Arc::new(f.resolver_with_assets(assets));
    let mut requests = MapRequests::default();
    let mut sources = SelectedMapSources::default();
    let mut limits = f.limits();
    limits.reader.max_listed_paths += asset_count;
    let outcome = requests.replace_selected(&mut sources, resolver, f.input(revision), limits);
    assert!(outcome.result.is_ok(), "{:?}", outcome.result);
    let inputs = host_inputs(requests.bindings.as_ref().unwrap().catalog.0.clone());
    assert!(inputs.day_night.models().is_empty());
    app.insert_resource(requests)
        .insert_resource(sources)
        .insert_resource(CampaignHostSupply {
            revision,
            inputs: Some(inputs),
        });
    app.update();
    f.catalog_reads
}

fn selected(app: &mut App) -> [f32; 2] {
    app.world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>()
        .host()
        .unwrap()
        .objects()
        .day_night_state()
        .unwrap()
        .selected()
        .unwrap()
        .map(|model| model.lights()[0].colour[0])
}

fn call(app: &mut App, name: &str) -> Result<Value, ScriptSessionError> {
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let id = session
        .context()
        .unwrap()
        .scripts
        .program()
        .function_id(name)
        .unwrap();
    session.call_function(id, &[])
}

#[test]
pub fn executed_paths_load_lighting_with_source_provenance_and_no_hand_built_bank() {
    let mut app = app();
    let reads = deliver(&mut app, SCRIPT, models(), 1);
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_none()
    );
    assert_eq!(selected(&mut app), [0.125, 0.875]);
    {
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let models = session
            .host()
            .unwrap()
            .objects()
            .day_night_state()
            .unwrap()
            .selected()
            .unwrap();
        assert_eq!(models[0].script_path(), "Scene/Terrain.MDL");
        assert_eq!(models[0].bytes(), model(0.125));
        assert_eq!(models[0].origin().version, "catalog supplied version");
        let source: serde_json::Value = serde_json::from_str(&models[0].origin().source).unwrap();
        assert_eq!(source["source"]["id"], "catalog");
        assert!(
            source["locator"]
                .as_str()
                .unwrap()
                .ends_with("Scene/Terrain.mdx")
        );
    }
    let before = reads.load(std::sync::atomic::Ordering::SeqCst);
    call(&mut app, "swap").unwrap();
    assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), before);
    assert_eq!(selected(&mut app), [0.875, 0.125]);
    assert!(call(&mut app, "missing").is_err());
    assert_eq!(selected(&mut app), [0.875, 0.125]);
}

#[test]
pub fn missing_malformed_and_oversized_replacements_keep_the_prior_session_reader() {
    let mut app = app();
    deliver(&mut app, SCRIPT, models(), 1);
    let witness = app
        .world()
        .resource::<MapScriptSession<MapStartupHost>>()
        .session_witness()
        .unwrap();
    for (index, reply) in [
        Reply::Missing,
        Reply::Bytes(b"not a model".to_vec()),
        Reply::Bytes(vec![0; (2 << 20) + 1]),
    ]
    .into_iter()
    .enumerate()
    {
        let mut next = models();
        next.insert("Scene/Terrain.mdx".into(), Reply::Bytes(model(1.5)));
        next.insert("Scene/Units.mdx".into(), reply);
        next.insert("Scene/Later.mdx".into(), Reply::Bytes(model(1.25)));
        deliver(&mut app, SCRIPT, next, index as u64 + 2);
        assert!(
            app.world()
                .resource::<CampaignStartup>()
                .last_error
                .is_some()
        );
        assert!(std::sync::Weak::ptr_eq(
            &witness,
            &app.world()
                .resource::<MapScriptSession<MapStartupHost>>()
                .session_witness()
                .unwrap()
        ));
        call(&mut app, "fresh").unwrap();
        assert_eq!(
            selected(&mut app),
            [0.5, 0.875],
            "old VM keeps its original reader after refusal"
        );
    }
}

#[test]
pub fn repeated_model_paths_share_one_binding_without_duplicate_asset_failure() {
    let mut app = app();
    let script = SCRIPT.replace("folder + \"Units.mdx\"", "folder + \"Terrain.MDL\"");
    deliver(&mut app, &script, models(), 1);
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_none()
    );
    assert_eq!(selected(&mut app), [0.125, 0.125]);
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    assert_eq!(
        session
            .host()
            .unwrap()
            .objects()
            .day_night_state()
            .unwrap()
            .assets()
            .models()
            .len(),
        1
    );
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn browser_campaign_asset_gate() -> u32 {
    executed_paths_load_lighting_with_source_provenance_and_no_hand_built_bank();
    missing_malformed_and_oversized_replacements_keep_the_prior_session_reader();
    repeated_model_paths_share_one_binding_without_duplicate_asset_failure();
    cumulative_model_load_work_is_bounded_without_charging_cached_selection();
    4
}

#[test]
pub fn cumulative_model_load_work_is_bounded_without_charging_cached_selection() {
    let mut assets = models();
    for index in 0..64 {
        assets.insert(
            format!("Scene/Other{index}.mdx"),
            Reply::Bytes(model(index as f32)),
        );
    }
    let mut app = app();
    let reads = deliver(&mut app, SCRIPT, assets, 1);
    for index in 0..64 {
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let id = session
            .context()
            .unwrap()
            .scripts
            .program()
            .function_id("choose")
            .unwrap();
        let before = reads.load(std::sync::atomic::Ordering::SeqCst);
        let result = session.call_function(id, &[Value::Str(format!("Scene/Other{index}.mdl"))]);
        if index < 63 {
            result.unwrap();
            let after = reads.load(std::sync::atomic::Ordering::SeqCst);
            assert_eq!(after, before + 2);
            session
                .call_function(id, &[Value::Str(format!("Scene/Other{index}.mdl"))])
                .unwrap();
            assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), after);
        } else {
            assert!(
                result
                    .err()
                    .unwrap()
                    .to_string()
                    .contains("session source-load budget exhausted")
            );
            assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), before);
        }
    }
    assert_eq!(selected(&mut app), [62.0, 0.875]);
}

const SOUND_SCRIPT: &str = r#"
globals
string folder = "Media/"
sound first = null
sound encoded = null
endglobals
function config takes nothing returns nothing
endfunction
function main takes nothing returns nothing
set first = CreateSound(folder + "first.audio", true, false, true, -7, 91, "Authored")
set encoded = CreateSound(folder + "encoded.data", false, true, false, 3, 5, "Other")
call StartSound(first)
call StartSound(encoded)
endfunction
function repeat takes nothing returns sound
return CreateSound(folder + "first.audio", false, false, false, 0, 0, "")
endfunction
function fresh takes nothing returns sound
return CreateSound(folder + "later.audio", false, false, false, 0, 0, "")
endfunction
function absent takes nothing returns sound
return CreateSound(folder + "missing.audio", false, false, false, 0, 0, "")
endfunction
function choose_sound takes string path returns sound
return CreateSound(path, false, false, false, 0, 0, "")
endfunction
"#;

fn waveform(sample: i16) -> Vec<u8> {
    let mut bytes = b"RIFF".to_vec();
    bytes.extend(38u32.to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16u32.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(8000u32.to_le_bytes());
    bytes.extend(16000u32.to_le_bytes());
    bytes.extend(2u16.to_le_bytes());
    bytes.extend(16u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend(2u32.to_le_bytes());
    bytes.extend(sample.to_le_bytes());
    bytes
}

fn sounds() -> BTreeMap<String, Reply> {
    BTreeMap::from([
        ("Media/first.audio".into(), Reply::Bytes(waveform(7))),
        (
            "Media/encoded.data".into(),
            Reply::Bytes(
                include_bytes!("../../../pcx-play/tests/fixtures/synthetic-mp3/mono-xing.mp3")
                    .to_vec(),
            ),
        ),
        ("Media/later.audio".into(), Reply::Bytes(waveform(9))),
        ("Media/missing.audio".into(), Reply::Missing),
    ])
}

#[test]
pub fn executed_sound_files_append_pcm_and_mp3_with_original_identity_and_cached_reads() {
    let mut app = app();
    let reads = deliver(&mut app, SOUND_SCRIPT, sounds(), 1);
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_none()
    );
    let first = app
        .world()
        .resource::<MapScriptSession<MapStartupHost>>()
        .global("first")
        .unwrap();
    let encoded = app
        .world()
        .resource::<MapScriptSession<MapStartupHost>>()
        .global("encoded")
        .unwrap();
    let Value::Handle(first_id) = first else {
        panic!("PCM")
    };
    let Value::Handle(mp3_id) = encoded else {
        panic!("MP3")
    };
    {
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let host = session.host().unwrap().objects();
        let pcm = host.file_sound_state(first_id).unwrap();
        assert_eq!(pcm.resource.bytes(), waveform(7));
        assert!(pcm.looping && pcm.stop_when_out_of_range);
        assert_eq!(
            (pcm.fade_in_rate, pcm.fade_out_rate, pcm.eax_setting),
            (-7, 91, "Authored")
        );
        let origin: serde_json::Value =
            serde_json::from_str(&pcm.resource.origin().source).unwrap();
        assert_eq!(origin["source"]["id"], "catalog");
        assert!(
            origin["locator"]
                .as_str()
                .unwrap()
                .ends_with("Media/first.audio")
        );
        let mp3 = host.file_mp3_state(mp3_id).unwrap();
        assert_eq!(
            mp3.resource.bytes(),
            include_bytes!("../../../pcx-play/tests/fixtures/synthetic-mp3/mono-xing.mp3")
        );
        assert_eq!(mp3.resource.origin().version, "catalog supplied version");
    }
    let before = reads.load(std::sync::atomic::Ordering::SeqCst);
    let repeated = call(&mut app, "repeat").unwrap();
    assert_ne!(repeated, first);
    assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), before);
    call(&mut app, "fresh").unwrap();
    assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), before + 1);
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let host = session.host().unwrap().objects_mut();
    assert_eq!(
        host.file_sound_state(first_id).unwrap().resource.bytes(),
        waveform(7)
    );
    assert_eq!(
        host.file_mp3_state(mp3_id).unwrap().resource.bytes(),
        include_bytes!("../../../pcx-play/tests/fixtures/synthetic-mp3/mono-xing.mp3")
    );
    assert_eq!(host.take_sound_starts().len(), 2);
}

#[test]
pub fn failed_sound_replacement_keeps_the_prior_vm_resources_and_reader() {
    let mut app = app();
    deliver(&mut app, SOUND_SCRIPT, sounds(), 1);
    let witness = app
        .world()
        .resource::<MapScriptSession<MapStartupHost>>()
        .session_witness()
        .unwrap();
    for (index, reply) in [
        Reply::Missing,
        Reply::Bytes(b"not audio".to_vec()),
        Reply::Bytes(vec![0; (16 << 20) + 1]),
    ]
    .into_iter()
    .enumerate()
    {
        let mut next = sounds();
        next.insert("Media/encoded.data".into(), reply);
        next.insert("Media/later.audio".into(), Reply::Bytes(waveform(100)));
        deliver(&mut app, SOUND_SCRIPT, next, index as u64 + 2);
        assert!(
            app.world()
                .resource::<CampaignStartup>()
                .last_error
                .is_some()
        );
        assert!(std::sync::Weak::ptr_eq(
            &witness,
            &app.world()
                .resource::<MapScriptSession<MapStartupHost>>()
                .session_witness()
                .unwrap()
        ));
        let Value::Handle(id) = call(&mut app, "fresh").unwrap() else {
            panic!("sound")
        };
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        assert_eq!(
            session
                .host()
                .unwrap()
                .objects()
                .file_sound_state(id)
                .unwrap()
                .resource
                .bytes(),
            waveform(9)
        );
    }
}

#[test]
pub fn failed_sound_source_work_is_bounded_and_cached_files_do_not_charge_it() {
    let mut app = app();
    let reads = deliver(&mut app, SOUND_SCRIPT, sounds(), 1);
    let before = reads.load(std::sync::atomic::Ordering::SeqCst);
    call(&mut app, "repeat").unwrap();
    assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), before);
    for _ in 0..126 {
        assert!(call(&mut app, "absent").is_err());
    }
    assert_eq!(
        reads.load(std::sync::atomic::Ordering::SeqCst),
        before + 126
    );
    let error = call(&mut app, "absent").expect_err("source work cap");
    assert!(
        error
            .to_string()
            .contains("session source-load budget exhausted")
    );
    assert_eq!(
        reads.load(std::sync::atomic::Ordering::SeqCst),
        before + 126
    );
    assert!(call(&mut app, "repeat").is_ok());
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn browser_campaign_file_sound_gate() -> u32 {
    executed_sound_files_append_pcm_and_mp3_with_original_identity_and_cached_reads();
    failed_sound_replacement_keeps_the_prior_vm_resources_and_reader();
    failed_sound_source_work_is_bounded_and_cached_files_do_not_charge_it();
    sound_source_path_limit_refuses_before_reading_or_copying_the_path_into_an_error();
    4
}

#[test]
pub fn sound_source_path_limit_refuses_before_reading_or_copying_the_path_into_an_error() {
    let mut app = app();
    let reads = deliver(&mut app, SOUND_SCRIPT, sounds(), 1);
    let before = reads.load(std::sync::atomic::Ordering::SeqCst);
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let id = session
        .context()
        .unwrap()
        .scripts
        .program()
        .function_id("choose_sound")
        .unwrap();
    let error = session
        .call_function(id, &[Value::Str("x".repeat((16 << 10) + 1))])
        .expect_err("source path cap");
    let message = error.to_string();
    assert!(message.contains("source path/identity/count budget exceeded"));
    assert!(message.len() < 2048);
    assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), before);
}

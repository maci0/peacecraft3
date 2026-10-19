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

fn parameter_table(dialogue: bool) -> Vec<u8> {
    let rows = if dialogue {
        vec![
            vec![
                "SoundLabel",
                "Volume",
                "Flags",
                "version",
                "DirectoryBase",
                "FileNames",
            ],
            vec!["Speech", "97", "OPAQUE", "4", "Dialogue/", "line.data"],
        ]
    } else {
        vec![
            vec![
                "SoundName",
                "Volume",
                "Pitch",
                "PitchVariance",
                "Priority",
                "Channel",
                "MinDistance",
                "MaxDistance",
                "DistanceCutoff",
                "Flags",
                "EAXFlags",
            ],
            vec![
                "Voice", "83", "0.75", "0.125", "-2", "3", "5", "9", "11", "OPAQUE", "Profile",
            ],
            vec![
                "Other",
                "17",
                "1.25",
                "0",
                "7",
                "9",
                "3",
                "13",
                "19",
                "OTHER",
                "Different",
            ],
        ]
    };
    let mut text = String::from("ID;PWXL;N;E\r\n");
    for (y, row) in rows.iter().enumerate() {
        for (x, value) in row.iter().enumerate() {
            text.push_str(&format!("C;Y{};X{};K\"{}\"\r\n", y + 1, x + 1, value));
        }
    }
    text.push_str("E\r\n");
    text.into_bytes()
}

fn parameter_assets() -> BTreeMap<String, Reply> {
    let mut assets = sounds();
    assets.insert(
        "UI/SoundInfo/Arbitrary.slk".into(),
        Reply::Bytes(parameter_table(false)),
    );
    assets.insert(
        "UI/SoundInfo/Speech.slk".into(),
        Reply::Bytes(parameter_table(true)),
    );
    assets.insert(
        "UI/SoundInfo/Unrelated.slk".into(),
        Reply::Bytes(b"ID;PWXL;N;E\r\nC;Y1;X1;K\"SoundLabel\"\r\nC;Y1;X2;K\"AnimSoundEvent\"\r\nC;Y2;X1;K\"Voice\"\r\nC;Y2;X2;K\"Event\"\r\nE\r\n".to_vec()),
    );
    assets
}

fn parameter_script() -> String {
    SOUND_SCRIPT
        .replace(
            "call StartSound(first)",
            "call SetSoundParamsFromLabel(first, folder + \"Voice\")\ncall StartSound(first)",
        )
        .replace("folder + \"Voice\"", "\"Vo\" + \"ice\"")
        .replace(
            "call StartSound(encoded)",
            "call SetSoundParamsFromLabel(encoded, \"Speech\")\ncall StartSound(encoded)",
        )
        + r#"
function change_parameters takes nothing returns nothing
call SetSoundParamsFromLabel(encoded,"Other")
endfunction
function missing_parameters takes nothing returns nothing
call SetSoundParamsFromLabel(first,"Missing")
endfunction
"#
}

fn assignment(app: &mut App, global: &str) -> (String, i32) {
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let Value::Handle(id) = session.global(global).unwrap() else {
        panic!("sound")
    };
    let row = session
        .host()
        .unwrap()
        .objects()
        .sound_parameter_assignment(id)
        .unwrap()
        .unwrap();
    match row.parameters {
        pcx_play::sound_parameters::ParameterValues::Full(p) => (p.label.into(), p.values.volume),
        pcx_play::sound_parameters::ParameterValues::Dialogue(p) => {
            (p.label.into(), p.values.volume)
        }
    }
}

#[test]
pub fn executed_parameter_labels_use_selected_tables_and_keep_old_assignments() {
    let mut app = app();
    let reads = deliver(&mut app, &parameter_script(), parameter_assets(), 1);
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_none(),
        "{:?}",
        app.world().resource::<CampaignStartup>().last_error
    );
    assert_eq!(assignment(&mut app, "first"), ("Voice".into(), 83));
    assert_eq!(assignment(&mut app, "encoded"), ("Speech".into(), 97));
    let before = reads.load(std::sync::atomic::Ordering::SeqCst);
    call(&mut app, "change_parameters").unwrap();
    call(&mut app, "change_parameters").unwrap();
    assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), before);
    assert_eq!(assignment(&mut app, "first"), ("Voice".into(), 83));
    assert_eq!(assignment(&mut app, "encoded"), ("Other".into(), 17));
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    assert_eq!(
        session
            .host()
            .unwrap()
            .objects()
            .sound_parameter_banks()
            .unwrap()
            .sources()
            .count(),
        2
    );
}

#[test]
pub fn failed_parameter_replacement_preserves_prior_assignments() {
    let mut app = app();
    deliver(&mut app, &parameter_script(), parameter_assets(), 1);
    let witness = app
        .world()
        .resource::<MapScriptSession<MapStartupHost>>()
        .session_witness()
        .unwrap();
    for (i, reply) in [
        Reply::Missing,
        Reply::Bytes(b"not a table".to_vec()),
        Reply::Bytes(parameter_table(false)),
    ]
    .into_iter()
    .enumerate()
    {
        let mut assets = parameter_assets();
        if i == 2 {
            assets.insert("UI/SoundInfo/Duplicate.slk".into(), reply);
        } else {
            assets.insert("UI/SoundInfo/Arbitrary.slk".into(), reply);
        }
        deliver(&mut app, &parameter_script(), assets, i as u64 + 2);
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
        call(&mut app, "change_parameters").unwrap();
        assert_eq!(assignment(&mut app, "first"), ("Voice".into(), 83));
        assert_eq!(assignment(&mut app, "encoded"), ("Other".into(), 17));
    }
}

#[test]
pub fn parameter_source_work_is_bounded_without_rereading_tables() {
    let mut app = app();
    let reads = deliver(&mut app, &parameter_script(), parameter_assets(), 1);
    let before = reads.load(std::sync::atomic::Ordering::SeqCst);
    for _ in 0..126 {
        assert!(call(&mut app, "missing_parameters").is_err());
    }
    assert!(
        call(&mut app, "missing_parameters")
            .expect_err("work cap")
            .to_string()
            .contains("source work budget exhausted")
    );
    assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), before);
    assert_eq!(assignment(&mut app, "first"), ("Voice".into(), 83));
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn browser_campaign_parameter_gate() -> u32 {
    executed_parameter_labels_use_selected_tables_and_keep_old_assignments();
    failed_parameter_replacement_preserves_prior_assignments();
    parameter_source_work_is_bounded_without_rereading_tables();
    failed_parameter_index_is_not_read_repeatedly_on_the_published_host();
    4
}

#[test]
pub fn failed_parameter_index_is_not_read_repeatedly_on_the_published_host() {
    let mut app = app();
    let script = SOUND_SCRIPT.to_owned()
        + "\nfunction late_parameters takes nothing returns nothing\ncall SetSoundParamsFromLabel(first,\"Voice\")\nendfunction\n";
    let mut assets = parameter_assets();
    assets.insert("UI/SoundInfo/Arbitrary.slk".into(), Reply::Missing);
    let reads = deliver(&mut app, &script, assets, 1);
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_none()
    );
    let before = reads.load(std::sync::atomic::Ordering::SeqCst);
    assert!(call(&mut app, "late_parameters").is_err());
    let after = reads.load(std::sync::atomic::Ordering::SeqCst);
    assert_eq!(after, before + 1);
    assert!(call(&mut app, "late_parameters").is_err());
    assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), after);
}

fn label_assets() -> BTreeMap<String, Reply> {
    let mut assets = parameter_assets();
    let table = String::from_utf8(parameter_table(false)).unwrap();
    let mut table = table.strip_suffix("E\r\n").unwrap().to_owned();
    table.push_str("C;Y1;X12;K\"DirectoryBase\"\r\nC;Y1;X13;K\"FileNames\"\r\nC;Y2;X12;K\"Media\"\r\nC;Y2;X13;K\"voice.wav\"\r\nC;Y3;X12;K\"Media\"\r\nC;Y3;X13;K\"other.wav\"\r\nE\r\n");
    assets.insert(
        "UI/SoundInfo/Arbitrary.slk".into(),
        Reply::Bytes(table.into_bytes()),
    );
    assets.insert(
        "UI/SoundInfo/Speech.slk".into(),
        Reply::Bytes(
            String::from_utf8(parameter_table(true))
                .unwrap()
                .replace("Dialogue/", "Dialogue")
                .into_bytes(),
        ),
    );
    assets.insert("UI/SoundInfo/OtherFormat.slk".into(), Reply::Bytes(
        b"ID;PWXL;N;E\r\nC;Y1;X1;K\"SoundLabel\"\r\nC;Y1;X2;K\"MIDIFileName\"\r\nC;Y1;X3;K\"Volume\"\r\nC;Y2;X1;K\"Speech\"\r\nC;Y2;X2;K\"other.mid\"\r\nC;Y2;X3;K\"17\"\r\nE\r\n".to_vec()));
    assets.insert("Media/voice.wav".into(), Reply::Bytes(waveform(23)));
    assets.insert("Media/other.wav".into(), Reply::Bytes(waveform(37)));
    assets.insert("Dialogue/line.data".into(), Reply::Bytes(waveform(41)));
    assets
}
const LABEL_SCRIPT: &str = r#"
globals
sound first=null
sound encoded=null
endglobals
function config takes nothing returns nothing
endfunction
function main takes nothing returns nothing
set first=CreateSoundFromLabel("Vo"+"ice",true,false,true,-7,91)
set encoded=CreateMIDISound("Speech",3,5)
call StartSound(first)
call StartSound(encoded)
endfunction
function apply_label_parameters takes nothing returns nothing
call SetSoundParamsFromLabel(first,"Voice")
endfunction
function repeat_label takes nothing returns sound
return CreateSoundFromLabel("Voice",false,false,false,0,0)
endfunction
function later_label takes nothing returns sound
return CreateSoundFromLabel("Other",false,false,false,0,0)
endfunction
function missing_label takes nothing returns sound
return CreateSoundFromLabel("Missing",false,false,false,0,0)
endfunction
"#;
fn labelled(app: &mut App, global: &str) -> (String, Vec<u8>, i32, i32) {
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let Value::Handle(id) = session.global(global).unwrap() else {
        panic!("sound handle")
    };
    let state = session.host().unwrap().objects().sound_state(id).unwrap();
    (
        state.resource.label().into(),
        state.resource.bytes().to_vec(),
        state.fade_in_rate,
        state.fade_out_rate,
    )
}
#[test]
pub fn executed_label_constructors_share_sources_and_keep_existing_sounds() {
    let mut app = app();
    let reads = deliver(&mut app, LABEL_SCRIPT, label_assets(), 1);
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_none(),
        "{:?}",
        app.world().resource::<CampaignStartup>().last_error
    );
    assert_eq!(
        labelled(&mut app, "first"),
        ("Voice".into(), waveform(23), -7, 91)
    );
    assert_eq!(
        labelled(&mut app, "encoded"),
        ("Speech".into(), waveform(41), 3, 5)
    );
    let indexed_reads = reads.load(std::sync::atomic::Ordering::SeqCst);
    call(&mut app, "apply_label_parameters").unwrap();
    assert_eq!(
        reads.load(std::sync::atomic::Ordering::SeqCst),
        indexed_reads,
        "parameter assignment reuses the constructor's source index"
    );
    assert_eq!(assignment(&mut app, "first"), ("Voice".into(), 83));
    let before = reads.load(std::sync::atomic::Ordering::SeqCst);
    let a = call(&mut app, "repeat_label").unwrap();
    let b = call(&mut app, "repeat_label").unwrap();
    assert_ne!(a, b);
    assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), before);
    call(&mut app, "later_label").unwrap();
    let after = reads.load(std::sync::atomic::Ordering::SeqCst);
    assert_eq!(after, before + 1);
    call(&mut app, "later_label").unwrap();
    assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), after);
    assert_eq!(labelled(&mut app, "first").1, waveform(23));
}
#[test]
pub fn failed_label_source_replacements_preserve_the_old_reader_and_sounds() {
    let mut app = app();
    deliver(&mut app, LABEL_SCRIPT, label_assets(), 1);
    let witness = app
        .world()
        .resource::<MapScriptSession<MapStartupHost>>()
        .session_witness()
        .unwrap();
    for (i, reply) in [
        Reply::Missing,
        Reply::Bytes(b"broken wave".to_vec()),
        Reply::Bytes(vec![0; (64 << 20) + 1]),
    ]
    .into_iter()
    .enumerate()
    {
        let mut assets = label_assets();
        assets.insert("Media/voice.wav".into(), reply);
        assets.insert("Media/other.wav".into(), Reply::Bytes(waveform(99)));
        deliver(&mut app, LABEL_SCRIPT, assets, i as u64 + 2);
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
        let Value::Handle(id) = call(&mut app, "later_label").unwrap() else {
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
                .sound_state(id)
                .unwrap()
                .resource
                .bytes(),
            waveform(37)
        );
    }
    assert_eq!(labelled(&mut app, "first").1, waveform(23));
}
#[test]
pub fn unsupported_label_variants_refuse_before_reading_and_work_is_bounded() {
    let mut app = app();
    let mut assets = label_assets();
    let Reply::Bytes(table) = assets.get_mut("UI/SoundInfo/Arbitrary.slk").unwrap() else {
        panic!("table")
    };
    *table = String::from_utf8(table.clone())
        .unwrap()
        .replace("K\"other.wav\"", "K\"other.wav,extra.wav\"")
        .into_bytes();
    let reads = deliver(&mut app, LABEL_SCRIPT, assets, 1);
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_none()
    );
    let before = reads.load(std::sync::atomic::Ordering::SeqCst);
    assert!(
        call(&mut app, "later_label")
            .unwrap_err()
            .to_string()
            .contains("multiple files")
    );
    for _ in 0..125 {
        assert!(call(&mut app, "missing_label").is_err());
    }
    assert!(
        call(&mut app, "missing_label")
            .unwrap_err()
            .to_string()
            .contains("source work budget exhausted")
    );
    call(&mut app, "repeat_label").unwrap();
    assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), before);
    assert_eq!(labelled(&mut app, "encoded").1, waveform(41));
}
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn browser_campaign_label_sound_gate() -> u32 {
    executed_label_constructors_share_sources_and_keep_existing_sounds();
    failed_label_source_replacements_preserve_the_old_reader_and_sounds();
    unsupported_label_variants_refuse_before_reading_and_work_is_bounded();
    3
}

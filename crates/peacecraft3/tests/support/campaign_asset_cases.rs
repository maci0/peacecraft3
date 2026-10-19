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

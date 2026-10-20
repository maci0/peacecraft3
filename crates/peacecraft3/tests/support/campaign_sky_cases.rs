//! Executed `SetSkyModel` state through the production map request and startup path.
//! The dome itself is drawn in the application's campaign actor module, which these
//! integration tests cannot reach. They check the published session state that the
//! dome reads.
use super::*;
use std::collections::BTreeMap;

const SKY_PATH: &str = r"Environment\Sky\LordaeronSummerSky\LordaeronSummerSky.mdl";

fn sky_script(call: &str) -> String {
    format!(
        "globals\ninteger marker = 1\nendglobals\n\
         function config takes nothing returns nothing\nendfunction\n\
         function main takes nothing returns nothing\n{call}\nendfunction\n"
    )
}

fn app() -> App {
    let mut app = App::new();
    peacecraft3::map_requests::install(&mut app);
    peacecraft3::campaign_startup::install(&mut app);
    app
}

fn deliver(app: &mut App, script: &str, revision: u64) {
    let f = fixture(script);
    let resolver = Arc::new(f.resolver_with_assets(BTreeMap::new()));
    let mut requests = MapRequests::default();
    let mut sources = SelectedMapSources::default();
    let outcome = requests.replace_selected(&mut sources, resolver, f.input(revision), f.limits());
    assert!(outcome.result.is_ok(), "{:?}", outcome.result);
    let inputs = host_inputs(requests.bindings.as_ref().unwrap().catalog.0.clone());
    app.insert_resource(requests)
        .insert_resource(sources)
        .insert_resource(CampaignHostSupply {
            revision,
            inputs: Some(inputs),
        });
    app.update();
}

fn published_sky(app: &mut App) -> Option<Option<String>> {
    app.world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>()
        .host()
        .expect("published host")
        .objects()
        .environment()
        .expect("environment admitted")
        .sky_model
        .clone()
}

#[test]
pub fn executed_sky_path_is_published_with_its_source_spelling() {
    let mut app = app();
    deliver(
        &mut app,
        &sky_script(&format!(
            "call SetSkyModel(\"{}\")",
            SKY_PATH.replace('\\', "\\\\")
        )),
        1,
    );
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_none()
    );
    assert_eq!(published_sky(&mut app), Some(Some(SKY_PATH.to_owned())));
}

#[test]
pub fn explicit_null_is_published_distinct_from_an_unset_sky() {
    let mut app = app();
    deliver(&mut app, &sky_script("call SetSkyModel(null)"), 1);
    assert_eq!(published_sky(&mut app), Some(None));
    let mut app = app();
    deliver(&mut app, &sky_script(""), 1);
    assert_eq!(published_sky(&mut app), None);
}

#[test]
pub fn replacement_session_publishes_its_own_sky_state() {
    let mut app = app();
    deliver(
        &mut app,
        &sky_script(&format!(
            "call SetSkyModel(\"{}\")",
            SKY_PATH.replace('\\', "\\\\")
        )),
        1,
    );
    assert_eq!(published_sky(&mut app), Some(Some(SKY_PATH.to_owned())));
    deliver(&mut app, &sky_script("call SetSkyModel(null)"), 2);
    assert_eq!(published_sky(&mut app), Some(None));
}

#[test]
pub fn refused_replacement_keeps_the_published_sky_state() {
    let mut app = app();
    deliver(
        &mut app,
        &sky_script(&format!(
            "call SetSkyModel(\"{}\")",
            SKY_PATH.replace('\\', "\\\\")
        )),
        1,
    );
    deliver(
        &mut app,
        &sky_script("call SetSkyModel(null)\ncall SetAllItemTypeSlots(1)"),
        2,
    );
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_some()
    );
    assert_eq!(published_sky(&mut app), Some(Some(SKY_PATH.to_owned())));
}

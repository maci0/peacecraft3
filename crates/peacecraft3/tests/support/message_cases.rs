//! Executed text messages on the production startup and replacement path.
use super::*;
const EXTRA: &str = "
constant native GetLocalPlayer takes nothing returns player
native DisplayTimedTextToPlayer takes player p, real x, real y, real duration, string text returns nothing
native ClearTextMessages takes nothing returns nothing
";
const MAP: &str = "
function config takes nothing returns nothing
endfunction
function main takes nothing returns nothing
call DisplayTimedTextToPlayer(Player(2),0,0,4,\"old\")
call DisplayTimedTextToPlayer(Player(1),0,0,4,\"other\")
call ClearTextMessages()
call DisplayTimedTextToPlayer(GetLocalPlayer(),0,0,2,\"live\")
endfunction
function clear takes nothing returns nothing
call ClearTextMessages()
endfunction
";
fn deliver(app: &mut App, revision: u64, extra: &str) {
    let mut f = fixture(MAP);
    f.common.extend_from_slice(extra.as_bytes());
    f.common
        .extend_from_slice(super::script_timer_cases::TIMER_COMMON.as_bytes());
    f.rebuild();
    let resolver = f.resolver();
    let bound = bind_map(&resolver, f.input(revision), f.limits()).unwrap();
    let mut inputs = host_inputs(Arc::clone(&bound.catalog.0));
    inputs.post_config.local_player = Some(2);
    inputs.timer_time = Some(0.0);
    let bytes = Arc::clone(&bound.catalog_map_bytes);
    app.insert_resource(MapRequests {
        bindings: Some(bound),
        ..default()
    })
    .insert_resource(CampaignHostSupply {
        revision,
        inputs: Some(inputs),
    });
    app.world_mut().write_message(MapRequest {
        label: "synthetic message map".into(),
        map_member: pcx_assets::AssetPath::new("selected map.w3m").unwrap(),
        identity: f.map_id.clone(),
        binding_revision: revision,
        payload: MapPayload::Shared(bytes),
    });
    app.update();
}
fn app() -> App {
    let mut app = App::new();
    peacecraft3::map_requests::install(&mut app);
    peacecraft3::campaign_startup::install(&mut app);
    app
}
#[test]
pub fn messages_and_clear_execute_on_the_published_factory_host() {
    let mut app = app();
    deliver(&mut app, 1, EXTRA);
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_none(),
        "{:?}",
        app.world().resource::<CampaignStartup>().last_error
    );
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let context = session.context().unwrap().clone();
    let host = session.host().unwrap().objects();
    let state = host.messages().unwrap();
    assert_eq!(state.revision(), 3);
    assert_eq!(state.visible(0.).next().unwrap().text, "live");
    assert_eq!(state.visible(2.).count(), 0);
    let clear = context.scripts.program().function_id("clear").unwrap();
    session.call_function(clear, &[]).unwrap();
    assert_eq!(
        session
            .host()
            .unwrap()
            .objects()
            .messages()
            .unwrap()
            .visible(0.)
            .count(),
        0
    );
}
#[test]
pub fn invalid_message_contract_preserves_the_live_published_messages() {
    let mut app = app();
    deliver(&mut app, 1, EXTRA);
    let witness = app
        .world()
        .resource::<MapScriptSession<MapStartupHost>>()
        .session_witness()
        .unwrap();
    deliver(
        &mut app,
        2,
        &EXTRA.replace(
            "native ClearTextMessages takes nothing",
            "constant native ClearTextMessages takes nothing",
        ),
    );
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_some()
    );
    assert!(witness.upgrade().is_some());
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    assert_eq!(
        session
            .host()
            .unwrap()
            .objects()
            .messages()
            .unwrap()
            .visible(0.)
            .next()
            .unwrap()
            .text,
        "live"
    );
}
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn browser_campaign_messages_gate() -> u32 {
    messages_and_clear_execute_on_the_published_factory_host();
    invalid_message_contract_preserves_the_live_published_messages();
    2
}

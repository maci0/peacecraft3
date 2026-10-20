//! No-pass capability belongs to the renderer, not to an inferred game option.
use super::*;
const EXTRA: &str = "native EnableOcclusion takes boolean flag returns nothing\n";
fn app(capability: bool) -> App {
    let mut app = App::new();
    peacecraft3::map_requests::install(&mut app);
    peacecraft3::campaign_startup::install(&mut app);
    if capability {
        app.insert_resource(peacecraft3::map_ground::NoOcclusionPass);
    }
    app
}
fn deliver(app: &mut App, revision: u64, enable: bool) {
    let mut f = fixture(&format!(
        "globals\ninteger after=0\nendglobals\nfunction config takes nothing returns nothing\nendfunction\nfunction main takes nothing returns nothing\ncall EnableOcclusion({enable})\nset after=1\nendfunction"
    ));
    f.common.extend_from_slice(EXTRA.as_bytes());
    f.rebuild();
    let resolver = f.resolver();
    let bound = bind_map(&resolver, f.input(revision), f.limits()).unwrap();
    let inputs = host_inputs(Arc::clone(&bound.catalog.0));
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
        label: "synthetic no-occlusion-pass map".into(),
        map_member: pcx_assets::AssetPath::new("selected map.w3m").unwrap(),
        identity: f.map_id.clone(),
        binding_revision: revision,
        payload: MapPayload::Shared(bytes),
    });
    app.update();
}
#[test]
pub fn renderer_capability_admits_the_executed_occlusion_flag() {
    for (capability, enable, accepted) in [
        (false, false, false),
        (true, false, true),
        (true, true, true),
    ] {
        let mut app = app(capability);
        deliver(&mut app, 1, enable);
        assert_eq!(
            app.world()
                .resource::<CampaignStartup>()
                .last_error
                .is_none(),
            accepted
        );
        let session = app.world().resource::<MapScriptSession<MapStartupHost>>();
        assert_eq!(session.global("after"), accepted.then_some(Value::Int(1)));
    }
}
#[test]
pub fn refused_occlusion_candidate_keeps_the_previous_published_session() {
    let mut app = app(true);
    deliver(&mut app, 1, false);
    let witness = app
        .world()
        .resource::<MapScriptSession<MapStartupHost>>()
        .session_witness()
        .unwrap();
    // Without the renderer capability the candidate refuses the native.
    app.world_mut()
        .remove_resource::<peacecraft3::map_ground::NoOcclusionPass>();
    deliver(&mut app, 2, true);
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_some()
    );
    assert!(witness.upgrade().is_some());
    let session = app.world().resource::<MapScriptSession<MapStartupHost>>();
    assert_eq!(session.global("after"), Some(Value::Int(1)));
}
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn browser_campaign_occlusion_gate() -> u32 {
    renderer_capability_admits_the_executed_occlusion_flag();
    refused_occlusion_candidate_keeps_the_previous_published_session();
    2
}

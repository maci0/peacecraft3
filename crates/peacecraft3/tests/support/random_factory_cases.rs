//! Source-bound RNG through delivered-map publication and the production factory.
use super::*;
use pcx_play::map_startup_host::RandomInput;
use pcx_play::script_random::{profile::Policy, state};
use pcx_script::Host;
#[path = "../../../pcx-play/tests/support/random_local.rs"]
mod local;

const EXTRA: &str = "native GetRandomInt takes integer low, integer high returns integer\nnative GetRandomReal takes real low, real high returns real\nnative SetRandomSeed takes integer seed returns nothing\n";
const MAP: &str = "globals\ninteger marker = 0\nreal sample = 0.0\nendglobals\nfunction config takes nothing returns nothing\ncall SetRandomSeed(-29)\nendfunction\nfunction main takes nothing returns nothing\nset marker = GetRandomInt(-20, 30)\nset sample = GetRandomReal(-1.0, 2.0)\nendfunction\n";

fn deliver_random(app: &mut App, revision: u64, script: &str, random: Option<RandomInput>) {
    let mut map = fixture(script);
    map.common.extend_from_slice(EXTRA.as_bytes());
    map.rebuild();
    let resolver = map.resolver();
    let bound = bind_map(&resolver, map.input(revision), map.limits()).unwrap();
    let mut inputs = host_inputs(Arc::clone(&bound.catalog.0));
    inputs.random = random;
    let bytes = Arc::clone(&bound.catalog_map_bytes);
    app.insert_resource(MapRequests {
        bindings: Some(bound),
        ..Default::default()
    });
    app.insert_resource(CampaignHostSupply {
        revision,
        inputs: Some(inputs),
    });
    app.world_mut().write_message(MapRequest {
        label: "synthetic explicit-seed map".into(),
        map_member: pcx_assets::AssetPath::new("selected map.w3m").unwrap(),
        identity: map.map_id.clone(),
        binding_revision: revision,
        payload: MapPayload::Shared(bytes),
    });
    app.update();
    assert!(
        app.world()
            .resource::<MapRequests>()
            .last_outcome
            .as_ref()
            .unwrap()
            .result
            .is_ok()
    );
}

fn factory_gate(policy: Arc<Policy>) {
    let mut app = App::new();
    peacecraft3::map_requests::install(&mut app);
    peacecraft3::campaign_startup::install(&mut app);
    let input = || {
        Some(RandomInput {
            policy: Arc::clone(&policy),
            max_secondary: 256,
        })
    };
    deliver_random(&mut app, 1, MAP, input());
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_none()
    );
    let mut control = Some(state::seeded(&policy, -29, 256).unwrap());
    let expected_int = state::integer(&policy, &mut control, -20, 30).unwrap();
    let expected_real = state::real(&policy, &mut control, -1.0, 2.0).unwrap();
    assert_eq!(session_marker(&mut app), Value::Int(expected_int));
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    assert_eq!(
        session.global("sample").unwrap(),
        Value::Real(expected_real)
    );
    assert_eq!(
        session.host().unwrap().objects().script_random_state(),
        control.as_ref()
    );
    let previous = session.session_witness().unwrap();
    let old_word = control.as_ref().unwrap().primary_word();

    let unseeded = MAP.replace("call SetRandomSeed(-29)", "");
    deliver_random(&mut app, 2, &unseeded, input());
    let error = app
        .world()
        .resource::<CampaignStartup>()
        .last_error
        .as_ref()
        .unwrap()
        .to_string();
    assert!(error.contains("explicit seed"), "{error}");
    assert!(previous.upgrade().is_some());
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    assert_eq!(
        session
            .host()
            .unwrap()
            .objects()
            .script_random_state()
            .unwrap()
            .primary_word(),
        old_word
    );

    deliver_random(&mut app, 3, MAP, None);
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_some()
    );
    assert!(previous.upgrade().is_some());
    deliver_random(&mut app, 4, MAP, input());
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_none()
    );
    assert!(previous.upgrade().is_none());
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    assert_eq!(
        session.host().unwrap().objects().script_random_state(),
        control.as_ref()
    );
    let actual = session
        .host()
        .unwrap()
        .native("GetRandomInt", &[Value::Int(0), Value::Int(100)])
        .unwrap();
    assert_eq!(
        actual,
        Value::Int(state::integer(&policy, &mut control, 0, 100).unwrap())
    );
}

#[test]
#[ignore = "requires explicit qualified local image and twelve-role binding"]
fn explicit_source_rng_uses_factory_and_replacement_lifetime() {
    factory_gate(Arc::new(local::local_policy()));
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn browser_script_random_factory_gate(image: &[u8], selection: &[u8]) -> u32 {
    factory_gate(Arc::new(local::policy_from_inputs(image, selection)));
    4
}

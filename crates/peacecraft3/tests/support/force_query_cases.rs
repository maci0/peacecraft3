//! Membership queries through the production factory and published session.
use super::*;
const QUERY: &str = "constant native IsPlayerInForce takes player p, force f returns boolean\n";
const MAP: &str = "
globals
player a=Player(0)
player b=Player(1)
force f=CreateForce()
boolean before=false
boolean member=false
boolean other=true
endglobals
function config takes nothing returns nothing
endfunction
function main takes nothing returns nothing
set before=IsPlayerInForce(a,f)
call ForceAddPlayer(f,a)
set member=IsPlayerInForce(Player(0),f)
set other=IsPlayerInForce(b,f)
endfunction
";
fn deliver(app: &mut App, revision: u64, query: &str, foreign: bool) {
    let mut map = fixture(&if foreign {
        format!("{query}{MAP}")
    } else {
        MAP.into()
    });
    if !foreign {
        map.common.extend_from_slice(query.as_bytes());
    }
    map.rebuild();
    let resolver = map.resolver();
    let bound = bind_map(&resolver, map.input(revision), map.limits()).unwrap();
    let mut inputs = host_inputs(Arc::clone(&bound.catalog.0));
    inputs.budgets.forces = ForceBudget {
        max_records: 1,
        max_memberships: 1,
    };
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
        label: "synthetic force query".into(),
        map_member: pcx_assets::AssetPath::new("selected map.w3m").unwrap(),
        identity: map.map_id.clone(),
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
pub fn published_force_queries_use_executed_memberships_at_capacity() {
    let mut app = app();
    deliver(&mut app, 1, QUERY, false);
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
    assert_eq!(session.global("before"), Some(Value::Bool(false)));
    assert_eq!(session.global("member"), Some(Value::Bool(true)));
    assert_eq!(session.global("other"), Some(Value::Bool(false)));
    let Value::Handle(a) = session.global("a").unwrap() else {
        panic!("player");
    };
    let Value::Handle(f) = session.global("f").unwrap() else {
        panic!("force");
    };
    assert_eq!(
        session.host().unwrap().objects().force_members(f).unwrap(),
        &[a]
    );
}
#[test]
pub fn refused_query_contracts_keep_the_previous_published_force() {
    let mut app = app();
    deliver(&mut app, 1, QUERY, false);
    let witness = app
        .world()
        .resource::<MapScriptSession<MapStartupHost>>()
        .session_witness()
        .unwrap();
    for (revision, query, foreign) in [
        (2, QUERY.replace("boolean", "integer"), false),
        (3, QUERY.into(), true),
    ] {
        deliver(&mut app, revision, &query, foreign);
        assert!(
            app.world()
                .resource::<CampaignStartup>()
                .last_error
                .is_some()
        );
        assert!(witness.upgrade().is_some());
        let session = app.world().resource::<MapScriptSession<MapStartupHost>>();
        assert_eq!(session.global("member"), Some(Value::Bool(true)));
    }
}
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn browser_force_query_gate() -> u32 {
    published_force_queries_use_executed_memberships_at_capacity();
    refused_query_contracts_keep_the_previous_published_force();
    2
}

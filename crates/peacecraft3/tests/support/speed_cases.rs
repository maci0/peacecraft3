//! A current-speed request must not invent a rate or admit a different one.
use super::*;
use pcx_play::map_startup_host::SettingsObservation;
pub(super) const EXTRA: &str = "type mapflag extends handle
constant native ConvertMapFlag takes integer i returns mapflag
native SetMapFlag takes mapflag flag, boolean value returns nothing
native IsMapFlagSet takes mapflag flag returns boolean
type gamespeed extends handle
constant native ConvertGameSpeed takes integer i returns gamespeed
constant native GetGameSpeed takes nothing returns gamespeed
native SetGameSpeed takes gamespeed speed returns nothing
native IsFogEnabled takes nothing returns boolean
native IsFogMaskEnabled takes nothing returns boolean
native FogEnable takes boolean enabled returns nothing
native FogMaskEnable takes boolean enabled returns nothing
globals
constant mapflag MAP_LOCK_SPEED = ConvertMapFlag(17*19)
constant gamespeed CURRENT_SPEED = ConvertGameSpeed(-17)
constant gamespeed OTHER_SPEED = ConvertGameSpeed(53)
endglobals
";
fn deliver(app: &mut App, revision: u64, current: bool, lock: Option<bool>) {
    let lock = lock.map_or_else(String::new, |lock| {
        format!("call SetMapFlag(MAP_LOCK_SPEED, {lock})\n")
    });
    let speed = if current {
        "GetGameSpeed()"
    } else {
        "OTHER_SPEED"
    };
    let mut f = fixture(&format!(
        "globals\ninteger after=0\ngamespeed after_speed=null\nendglobals\nfunction config takes nothing returns nothing\nendfunction\nfunction main takes nothing returns nothing\n{lock}call SetGameSpeed({speed})\nset after_speed=GetGameSpeed()\nset after=1\nendfunction"
    ));
    f.common.extend_from_slice(EXTRA.as_bytes());
    f.rebuild();
    let resolver = f.resolver();
    let bound = bind_map(&resolver, f.input(revision), f.limits()).unwrap();
    let mut inputs = host_inputs(Arc::clone(&bound.catalog.0));
    inputs
        .enumeration
        .names
        .extend(["ConvertGameSpeed".into(), "ConvertMapFlag".into()]);
    inputs.post_config.settings = Some(SettingsObservation {
        speed_global: "CURRENT_SPEED".into(),
        fog_enabled: true,
        fog_mask_enabled: false,
    });
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
        label: "synthetic current-speed request".into(),
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
pub fn current_speed_request_publishes_with_the_retained_setting() {
    let mut app = app();
    deliver(&mut app, 1, true, None);
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_none()
    );
    let session = app.world().resource::<MapScriptSession<MapStartupHost>>();
    assert_eq!(session.global("after"), Some(Value::Int(1)));
    assert!(matches!(
        session.global("CURRENT_SPEED"),
        Some(Value::Handle(_))
    ));
}
#[test]
pub fn different_speed_refusal_preserves_the_published_session() {
    let mut app = app();
    deliver(&mut app, 1, true, None);
    let witness = app
        .world()
        .resource::<MapScriptSession<MapStartupHost>>()
        .session_witness()
        .unwrap();
    deliver(&mut app, 2, false, None);
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_some()
    );
    assert!(witness.upgrade().is_some());
    assert_eq!(
        app.world()
            .resource::<MapScriptSession<MapStartupHost>>()
            .global("after"),
        Some(Value::Int(1))
    );
}
#[test]
pub fn explicit_speed_lock_ignores_a_different_speed_and_unlock_restores_refusal() {
    let mut app = app();
    deliver(&mut app, 1, false, Some(true));
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_none()
    );
    let session = app.world().resource::<MapScriptSession<MapStartupHost>>();
    assert_eq!(session.global("after"), Some(Value::Int(1)));
    assert_eq!(
        session.global("after_speed"),
        session.global("CURRENT_SPEED")
    );
    let witness = session.session_witness().unwrap();
    deliver(&mut app, 2, false, Some(false));
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_some()
    );
    assert!(witness.upgrade().is_some());
    assert_eq!(
        app.world()
            .resource::<MapScriptSession<MapStartupHost>>()
            .global("after"),
        Some(Value::Int(1))
    );
}
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn browser_campaign_speed_gate() -> u32 {
    current_speed_request_publishes_with_the_retained_setting();
    different_speed_refusal_preserves_the_published_session();
    explicit_speed_lock_ignores_a_different_speed_and_unlock_restores_refusal();
    3
}

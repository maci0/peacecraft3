//! Actual factory/session → camera consumer, including failed/replaced sessions.
use super::*;
use peacecraft3::campaign_camera::{CampaignCamera, CampaignSceneCamera};
use peacecraft3::map_ground::GroundPublication;

const CAMERA_COMMON: &str = "
constant native GetLocalPlayer takes nothing returns player
native CameraSetupApplyForceDuration takes camerasetup which, boolean pan, real duration returns nothing
globals
constant camerafield CAMERA_FIELD_TARGET_DISTANCE = ConvertCameraField(11)
constant camerafield CAMERA_FIELD_FARZ = ConvertCameraField(14)
constant camerafield CAMERA_FIELD_ANGLE_OF_ATTACK = ConvertCameraField(17)
constant camerafield CAMERA_FIELD_FIELD_OF_VIEW = ConvertCameraField(20)
constant camerafield CAMERA_FIELD_ROLL = ConvertCameraField(23)
constant camerafield CAMERA_FIELD_ROTATION = ConvertCameraField(26)
constant camerafield CAMERA_FIELD_ZOFFSET = ConvertCameraField(29)
endglobals
";
const SCRIPT: &str = "
globals
camerasetup setup = CreateCameraSetup()
endglobals
function config takes nothing returns nothing
endfunction
function main takes nothing returns nothing
call CameraSetupSetField(setup, CAMERA_FIELD_TARGET_DISTANCE, 80.0, 0.0)
call CameraSetupSetField(setup, CAMERA_FIELD_FARZ, 900.0, 0.0)
call CameraSetupSetField(setup, CAMERA_FIELD_ANGLE_OF_ATTACK, 330.0, 0.0)
call CameraSetupSetField(setup, CAMERA_FIELD_FIELD_OF_VIEW, 65.0, 0.0)
call CameraSetupSetField(setup, CAMERA_FIELD_ROLL, 0.0, 0.0)
call CameraSetupSetField(setup, CAMERA_FIELD_ROTATION, 90.0, 0.0)
call CameraSetupSetField(setup, CAMERA_FIELD_ZOFFSET, 7.0, 0.0)
call CameraSetupSetDestPosition(setup, 11.0, 23.0, 0.0)
call CameraSetupApplyForceDuration(setup, true, 0.0)
endfunction
function pan takes nothing returns nothing
call CameraSetupSetDestPosition(setup, 17.0, 29.0, 0.0)
call CameraSetupApplyForceDuration(setup, true, 0.0)
endfunction
";

fn app() -> App {
    let mut app = App::new();
    peacecraft3::map_requests::install(&mut app);
    peacecraft3::campaign_startup::install(&mut app);
    app.init_resource::<GroundPublication>();
    app.init_resource::<ButtonInput<KeyCode>>();
    app.insert_resource(peacecraft3::campaign_runtime::CampaignRuntime::new(
        std::time::Duration::from_millis(10),
        2,
        8,
    ));
    peacecraft3::campaign_camera::install(&mut app, 1, 4096);
    app
}

fn deliver_camera(app: &mut App, script: &str, config: &[u8], revision: u64) {
    deliver_camera_for_player(app, script, config, revision, None);
}

fn deliver_camera_for_player(
    app: &mut App,
    script: &str,
    config: &[u8],
    revision: u64,
    local_player: Option<i32>,
) {
    let mut f = fixture(script);
    f.common = format!("{COMMON}{CAMERA_COMMON}").into_bytes();
    f.rebuild();
    let resolver = Arc::new(f.resolver_with_assets(std::collections::BTreeMap::from([(
        "UI\\MiscData.txt".into(),
        crate::binding::Reply::Bytes(config.to_vec()),
    )])));
    let mut requests = MapRequests::default();
    let mut sources = SelectedMapSources::default();
    let mut limits = f.limits();
    limits.reader.max_listed_paths += 1; // The selected camera config is one additional member.
    let outcome = requests.replace_selected(&mut sources, resolver, f.input(revision), limits);
    assert!(outcome.result.is_ok(), "{:?}", outcome.result);
    let mut inputs = host_inputs(Arc::clone(&requests.bindings.as_ref().unwrap().catalog.0));
    inputs.budgets.camera_setup_fields = 7;
    inputs.post_config.local_player = local_player;
    let request = requests.selected_map_request().unwrap();
    app.insert_resource(requests)
        .insert_resource(sources)
        .insert_resource(CampaignHostSupply {
            revision,
            inputs: Some(inputs),
        });
    app.world_mut().write_message(request);
    app.update();
    let context = app
        .world()
        .resource::<MapScriptSession<MapStartupHost>>()
        .context()
        .unwrap()
        .clone();
    app.world_mut().resource_mut::<GroundPublication>().context = Some(context);
    app.update();
}

#[test]
pub fn published_camera_uses_executed_fields_and_preserves_session_ownership() {
    let mut app = app();
    deliver_camera(&mut app, SCRIPT, b"[Camera]\nNearZ=3,9", 1);
    let view = app.world().resource::<CampaignCamera>();
    assert!(view.last_error.is_none(), "{:?}", view.last_error);
    let entity = view.entity.unwrap();
    assert!(view.active());
    assert_eq!(
        app.world()
            .get::<bevy::camera::visibility::RenderLayers>(entity),
        Some(&bevy::camera::visibility::RenderLayers::layer(1))
    );
    assert_eq!(
        view.clip_receipt.as_ref().unwrap().provenance.byte_length,
        b"[Camera]\nNearZ=3,9".len() as u64
    );
    let Projection::Perspective(projection) = app.world().get::<Projection>(entity).unwrap() else {
        panic!("perspective")
    };
    assert_eq!((projection.near, projection.far), (9.0, 900.0));
    let previous = *app.world().get::<Transform>(entity).unwrap();
    let function = app
        .world()
        .resource::<MapScriptSession<MapStartupHost>>()
        .context()
        .unwrap()
        .scripts
        .program()
        .function_id("pan")
        .unwrap();
    app.world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>()
        .call_function(function, &[])
        .unwrap();
    app.update();
    assert_ne!(*app.world().get::<Transform>(entity).unwrap(), previous);
    assert_eq!(
        app.world().resource::<CampaignCamera>().entity,
        Some(entity)
    );

    app.world_mut()
        .resource_mut::<peacecraft3::campaign_runtime::CampaignRuntime>()
        .paused = true;
    app.update();
    assert!(
        app.world()
            .resource::<peacecraft3::campaign_runtime::CampaignRuntime>()
            .paused,
        "view does not erase an explicit simulation pause"
    );
    app.world_mut()
        .resource_mut::<peacecraft3::campaign_runtime::CampaignRuntime>()
        .paused = false;

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    app.update();
    assert!(!app.world().resource::<CampaignCamera>().active());
    assert!(!app.world().get::<Camera>(entity).unwrap().is_active);
    assert!(
        app.world()
            .resource::<peacecraft3::campaign_runtime::CampaignRuntime>()
            .view_paused
    );
    app.update();
    assert!(
        !app.world().resource::<CampaignCamera>().active(),
        "held Escape does not toggle again"
    );

    deliver_camera(
        &mut app,
        &SCRIPT.replace("true, 0.0)", "true, 2.0)"),
        b"[Camera]\nNearZ=5,15",
        2,
    );
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_some()
    );
    assert_eq!(
        app.world().resource::<CampaignCamera>().entity,
        Some(entity)
    );
    deliver_camera(&mut app, SCRIPT, b"[Camera]\nNearZ=5,15", 3);
    let next = app.world().resource::<CampaignCamera>().entity.unwrap();
    assert!(
        !app.world()
            .resource::<peacecraft3::campaign_runtime::CampaignRuntime>()
            .view_paused
    );
    assert_ne!(next, entity);
    assert!(app.world().get::<CampaignSceneCamera>(entity).is_none());
    let Projection::Perspective(projection) = app.world().get::<Projection>(next).unwrap() else {
        panic!("perspective")
    };
    assert_eq!(projection.near, 15.0);
    app.world_mut().resource_mut::<GroundPublication>().context = None;
    app.update();
    assert!(!app.world().resource::<CampaignCamera>().active());
    assert!(!app.world().get::<Camera>(next).unwrap().is_active);
    let context = app
        .world()
        .resource::<MapScriptSession<MapStartupHost>>()
        .context()
        .cloned();
    app.world_mut().resource_mut::<GroundPublication>().context = context;
    app.update();
    assert!(app.world().resource::<CampaignCamera>().active());
    deliver_camera(&mut app, SCRIPT, b"[Camera]\nNearZ=5,NaN", 4);
    assert!(app.world().resource::<CampaignCamera>().entity.is_none());
    assert!(app.world().get::<CampaignSceneCamera>(next).is_none());
}

#[test]
pub fn invalid_source_clip_does_not_create_a_default_camera() {
    let mut app = app();
    deliver_camera(&mut app, SCRIPT, b"[Camera]\nNearZ=3,NaN", 1);
    let view = app.world().resource::<CampaignCamera>();
    assert!(view.entity.is_none());
    assert!(view.last_error.as_ref().unwrap().contains("NearZ[1]"));
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn browser_campaign_camera_gate() -> u32 {
    published_camera_uses_executed_fields_and_preserves_session_ownership();
    invalid_source_clip_does_not_create_a_default_camera();
    selected_local_player_controls_the_published_camera_branch();
    3
}

#[test]
pub fn selected_local_player_controls_the_published_camera_branch() {
    let script = SCRIPT.replace(
        "call CameraSetupApplyForceDuration(setup, true, 0.0)",
        "if GetLocalPlayer() == Player(4) then\ncall CameraSetupApplyForceDuration(setup, true, 0.0)\nendif",
    );
    for (number, active) in [(4, true), (5, false)] {
        let mut app = app();
        deliver_camera_for_player(&mut app, &script, b"[Camera]\nNearZ=3,9", 1, Some(number));
        assert!(
            app.world()
                .resource::<CampaignStartup>()
                .last_error
                .is_none()
        );
        assert_eq!(app.world().resource::<CampaignCamera>().active(), active);
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let host = session.host().unwrap();
        let local = pcx_script::Host::native(host, "GetLocalPlayer", &[]).unwrap();
        assert_eq!(
            pcx_script::Host::native(host, "GetPlayerId", &[local]).unwrap(),
            Value::Int(number)
        );
    }
}

use super::*;
use crate::ground_fixtures;
use bevy::asset::{AssetApp, AssetPlugin};
use pcx_gfx::{GroundMaterial, GroundMemory};
use peacecraft3::{
    campaign_boundary::BoundaryPublication,
    map_ground::{GroundPublication, GroundSupply},
};
fn scene(profile: Option<&[u8]>) -> App {
    let script = "function config takes nothing returns nothing\nendfunction\nfunction main takes nothing returns nothing\ncall EnableWorldFogBoundary(true)\nendfunction\nfunction disable takes nothing returns nothing\ncall EnableWorldFogBoundary(false)\nendfunction\nfunction enable takes nothing returns nothing\ncall EnableWorldFogBoundary(true)\nendfunction\n";
    let mut f = fixture(script);
    f.common
        .extend_from_slice(super::speed_cases::EXTRA.as_bytes());
    f.common
        .extend_from_slice(b"native EnableWorldFogBoundary takes boolean flag returns nothing\n");
    let terrain = &mut f
        .files
        .iter_mut()
        .find(|(n, _)| *n == "war3map.w3e")
        .unwrap()
        .1;
    terrain[13..17].copy_from_slice(&1u32.to_le_bytes());
    terrain.splice(17..17, *b"Qgrd");
    // First source corner's map-edge bit; no screenshot-fit region.
    let at = terrain.len() - 4 * pcx_map::CORNER_RECORD;
    let word = u16::from_le_bytes(terrain[at + 2..at + 4].try_into().unwrap()) | 0x4000;
    terrain[at + 2..at + 4].copy_from_slice(&word.to_le_bytes());
    f.rebuild();
    let mut assets = ground_fixtures::assets();
    if let Some(p) = profile {
        assets.insert(
            "UI\\MiscData.txt".into(),
            crate::binding::Reply::Bytes(p.to_vec()),
        );
    }
    let extra_paths = assets.len();
    let resolver = Arc::new(f.resolver_with_assets(assets));
    let mut requests = MapRequests::default();
    let mut sources = peacecraft3::browser_configuration::SelectedMapSources::default();
    let mut limits = f.limits();
    limits.reader.max_listed_paths += extra_paths;
    assert!(
        requests
            .replace_selected(&mut sources, resolver, f.input(1), limits)
            .result
            .is_ok()
    );
    let mut inputs = host_inputs(Arc::clone(&requests.bindings.as_ref().unwrap().catalog.0));
    inputs
        .enumeration
        .names
        .extend(["ConvertGameSpeed".into(), "ConvertMapFlag".into()]);
    inputs.post_config.settings = Some(pcx_play::map_startup_host::SettingsObservation {
        speed_global: "CURRENT_SPEED".into(),
        fog_enabled: false,
        fog_mask_enabled: false,
    });
    let request = requests.selected_map_request().unwrap();
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Image>()
        .init_asset::<Shader>()
        .init_resource::<Assets<Mesh>>();
    peacecraft3::map_requests::install(&mut app);
    peacecraft3::campaign_startup::install(&mut app);
    peacecraft3::map_ground::install(&mut app);
    app.insert_resource(requests)
        .insert_resource(sources)
        .insert_resource(CampaignHostSupply {
            revision: 1,
            inputs: Some(inputs),
        })
        .insert_resource(GroundSupply {
            revision: 1,
            read_limit: Some(4096),
            memory: Some(GroundMemory {
                retained_bytes: 64,
                transient_bytes: 128,
            }),
        });
    app.world_mut().write_message(request);
    app.update();
    app.update();
    app
}
fn call(app: &mut App, name: &str) {
    {
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
        session.call_function(id, &[]).unwrap();
    }
    app.update();
}
#[test]
pub fn executed_boundary_toggle_mounts_and_removes_the_source_mask() {
    let mut app = scene(Some(b"[FogOfWar]\nBoundaryTerrain=127,11,29,43\n"));
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_none()
    );
    let state = app.world().resource::<BoundaryPublication>();
    assert!(state.last_error.is_none(), "{:?}", state.last_error);
    assert!(state.profile_receipt.is_some());
    let entity = app.world().resource::<GroundPublication>().entity.unwrap();
    assert!(
        app.world()
            .get::<MeshMaterial3d<GroundMaterial>>(entity)
            .is_some()
    );
    let images = app.world().resource::<Assets<Image>>().len();
    app.update();
    assert_eq!(app.world().resource::<Assets<Image>>().len(), images);
    call(&mut app, "disable");
    assert!(
        app.world()
            .resource::<BoundaryPublication>()
            .profile_receipt
            .is_none()
    );
    assert_eq!(app.world().resource::<Assets<Image>>().len() + 1, images);
    call(&mut app, "enable");
    assert!(
        app.world()
            .resource::<BoundaryPublication>()
            .last_error
            .is_none()
    );
    assert_eq!(app.world().resource::<Assets<Image>>().len(), images);
}
#[test]
pub fn missing_boundary_style_refuses_without_a_color_fallback() {
    let mut app = scene(None);
    assert!(
        app.world()
            .resource::<BoundaryPublication>()
            .last_error
            .is_some()
    );
    assert!(
        app.world()
            .resource::<BoundaryPublication>()
            .profile_receipt
            .is_none()
    );
    call(&mut app, "disable");
    assert!(
        app.world()
            .resource::<BoundaryPublication>()
            .last_error
            .is_none()
    );
}
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn browser_campaign_fog_boundary_gate() -> u32 {
    executed_boundary_toggle_mounts_and_removes_the_source_mask();
    missing_boundary_style_refuses_without_a_color_fallback();
    2
}

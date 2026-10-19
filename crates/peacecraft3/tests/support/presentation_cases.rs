//! Published VM, retained source reader, actual filter nodes and input consumers.
use super::script_camera_cases::{CAMERA_COMMON, SCRIPT, app as camera_app};
use super::*;
use bevy::{
    asset::{AssetApp, AssetPlugin},
    ecs::system::RunSystemOnce,
};
use peacecraft3::{
    campaign_camera::CampaignCamera,
    campaign_presentation::{CampaignHud, CampaignPresentation, FilterMaterial},
    map_ground::GroundPublication,
};

const EXTRA: &str = "
type blendmode extends handle
type texmapflags extends handle
constant native ConvertBlendMode takes integer n returns blendmode
constant native ConvertTexMapFlags takes integer n returns texmapflags
native EnableUserUI takes boolean enabled returns nothing
native SetCineFilterTexture takes string path returns nothing
native SetCineFilterBlendMode takes blendmode b returns nothing
native SetCineFilterTexMapFlags takes texmapflags f returns nothing
native SetCineFilterStartUV takes real a, real b, real c, real d returns nothing
native SetCineFilterEndUV takes real a, real b, real c, real d returns nothing
native SetCineFilterStartColor takes integer r, integer g, integer b, integer a returns nothing
native SetCineFilterEndColor takes integer r, integer g, integer b, integer a returns nothing
native SetCineFilterDuration takes real duration returns nothing
native DisplayCineFilter takes boolean show returns nothing
globals
constant blendmode BLEND_MODE_BLEND=ConvertBlendMode(97)
constant texmapflags TEXMAP_FLAG_NONE=ConvertTexMapFlags(53)
endglobals
";
const SETUP: &str = "
call EnableUserUI(false)
call SetCineFilterTexture(\"filter.tga\")
call SetCineFilterBlendMode(BLEND_MODE_BLEND)
call SetCineFilterTexMapFlags(TEXMAP_FLAG_NONE)
call SetCineFilterStartUV(0,0,1,1)
call SetCineFilterEndUV(0,0,1,1)
call SetCineFilterStartColor(13,29,47,63)
call SetCineFilterEndColor(71,89,107,127)
call SetCineFilterDuration(0)
call DisplayCineFilter(true)
";
const CALLBACKS: &str = "
function hide takes nothing returns nothing
call DisplayCineFilter(false)
call EnableUserUI(true)
endfunction
function bad takes nothing returns nothing
call SetCineFilterTexture(\"missing.tga\")
call DisplayCineFilter(true)
endfunction
function show takes nothing returns nothing
call SetCineFilterTexture(\"filter.tga\")
call DisplayCineFilter(true)
endfunction
";
fn app() -> App {
    let mut app = camera_app();
    app.add_plugins((bevy::app::TaskPoolPlugin::default(), AssetPlugin::default()))
        .init_asset::<Image>()
        .init_asset::<Shader>();
    peacecraft3::campaign_presentation::install(&mut app);
    crate::button_actions::install(&mut app);
    app
}
enum Art {
    Tga,
    Blp,
    MalformedTga,
    UnreadableTga,
}
fn deliver(app: &mut App, revision: u64, fade: bool) {
    deliver_art(app, revision, fade, Art::Tga);
}
fn deliver_art(app: &mut App, revision: u64, fade: bool, art: Art) {
    let script = format!(
        "{}{}",
        SCRIPT.replace(
            "call CameraSetupApplyForceDuration(setup, true, 0.0)\nendfunction",
            &format!(
                "call CameraSetupApplyForceDuration(setup, true, 0.0)\n{}endfunction",
                if fade { SETUP } else { "" }
            )
        ),
        CALLBACKS
    );
    let mut f = fixture(&script);
    f.common = format!("{COMMON}{CAMERA_COMMON}{EXTRA}").into_bytes();
    f.rebuild();
    let mut tga = vec![0u8; 18];
    tga[2] = 2;
    tga[12..14].copy_from_slice(&1u16.to_le_bytes());
    tga[14..16].copy_from_slice(&1u16.to_le_bytes());
    tga[16] = 32;
    tga[17] = 0x28;
    tga.extend([29, 47, 71, 127]);
    let mut blp = vec![0u8; 0x49c + 1];
    blp[..4].copy_from_slice(b"BLP1");
    blp[4..8].copy_from_slice(&1u32.to_le_bytes());
    blp[12..16].copy_from_slice(&1u32.to_le_bytes());
    blp[16..20].copy_from_slice(&1u32.to_le_bytes());
    blp[28..32].copy_from_slice(&0x49cu32.to_le_bytes());
    blp[92..96].copy_from_slice(&1u32.to_le_bytes());
    blp[0x9c..0xa0].copy_from_slice(&[29, 47, 71, 255]);
    let mut assets = std::collections::BTreeMap::from([(
        "UI\\MiscData.txt".into(),
        crate::binding::Reply::Bytes(b"[Camera]\nNearZ=3,9".to_vec()),
    )]);
    match art {
        Art::Tga => {
            assets.insert("filter.tga".into(), crate::binding::Reply::Bytes(tga));
        }
        Art::Blp => {
            assets.insert("filter.blp".into(), crate::binding::Reply::Bytes(blp));
        }
        Art::MalformedTga => {
            assets.insert("filter.tga".into(), crate::binding::Reply::Bytes(vec![0]));
            assets.insert("filter.blp".into(), crate::binding::Reply::Bytes(blp));
        }
        Art::UnreadableTga => {
            assets.insert("filter.tga".into(), crate::binding::Reply::Missing);
            assets.insert("filter.blp".into(), crate::binding::Reply::Bytes(blp));
        }
    }
    let extra_paths = assets.len();
    let resolver = Arc::new(f.resolver_with_assets(assets));
    let mut requests = MapRequests::default();
    let mut sources = SelectedMapSources::default();
    let mut limits = f.limits();
    limits.reader.max_listed_paths += extra_paths;
    assert!(
        requests
            .replace_selected(&mut sources, resolver, f.input(revision), limits)
            .result
            .is_ok()
    );
    let mut inputs = host_inputs(Arc::clone(&requests.bindings.as_ref().unwrap().catalog.0));
    inputs.budgets.camera_setup_fields = 7;
    inputs
        .enumeration
        .names
        .extend(["ConvertBlendMode".into(), "ConvertTexMapFlags".into()]);
    let request = requests.selected_map_request().unwrap();
    app.insert_resource(requests)
        .insert_resource(sources)
        .insert_resource(CampaignHostSupply {
            revision,
            inputs: Some(inputs),
        });
    app.world_mut().write_message(request);
    app.update();
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_none(),
        "{:?}",
        app.world().resource::<CampaignStartup>().last_error
    );
    let context = app
        .world()
        .resource::<MapScriptSession<MapStartupHost>>()
        .context()
        .unwrap()
        .clone();
    app.world_mut().resource_mut::<GroundPublication>().context = Some(context);
    app.update();
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
pub fn published_filter_uses_its_source_and_blocks_actions_without_hiding_hud_or_pausing_time() {
    let mut app = app();
    let hud = app.world_mut().spawn((Node::default(), CampaignHud)).id();
    deliver(&mut app, 1, true);
    let presentation = app.world().resource::<CampaignPresentation>();
    assert!(presentation.input_blocked());
    assert!(
        presentation.last_error.is_none(),
        "{:?}",
        presentation.last_error
    );
    assert_eq!(
        presentation
            .texture_receipt
            .as_ref()
            .unwrap()
            .provenance
            .byte_length,
        22
    );
    assert!(app.world().resource::<CampaignCamera>().active());
    assert!(
        !app.world()
            .resource::<peacecraft3::campaign_runtime::CampaignRuntime>()
            .paused
    );
    let camera = app.world().get::<UiTargetCamera>(hud).unwrap().0;
    assert!(app.world().get::<Camera>(camera).unwrap().is_active);
    assert_ne!(
        app.world().get::<Visibility>(hud),
        Some(&Visibility::Hidden)
    );
    assert!(
        !app.world_mut()
            .run_system_once(crate::button_actions::input_allowed)
            .unwrap()
    );
    let button = app.world_mut().spawn(bevy::ui_widgets::Button).id();
    app.world_mut()
        .trigger(bevy::ui_widgets::Activate { entity: button });
    app.world_mut().flush();
    assert!(
        app.world()
            .resource::<crate::button_actions::ButtonActions>()
            .entities
            .is_empty()
    );
    let images = app.world().resource::<Assets<Image>>();
    assert_eq!(images.len(), 1);
    let image = images.iter().next().unwrap().1;
    assert_eq!(image.data.as_ref().unwrap(), &[71, 47, 29, 127]);
    assert_eq!(
        image.texture_descriptor.format,
        bevy::render::render_resource::TextureFormat::Rgba8Unorm
    );
    let overlay = app
        .world_mut()
        .query_filtered::<Entity, With<MaterialNode<FilterMaterial>>>()
        .single(app.world())
        .unwrap();
    assert_eq!(
        app.world().get::<Visibility>(overlay),
        Some(&Visibility::Visible)
    );
    let material = app
        .world()
        .get::<MaterialNode<FilterMaterial>>(overlay)
        .unwrap()
        .0
        .id();
    call(&mut app, "bad");
    assert!(
        app.world()
            .resource::<CampaignPresentation>()
            .last_error
            .as_ref()
            .unwrap()
            .contains("missing.tga")
    );
    assert_eq!(
        app.world()
            .get::<MaterialNode<FilterMaterial>>(overlay)
            .unwrap()
            .0
            .id(),
        material
    );
    assert_eq!(
        app.world().get::<Visibility>(overlay),
        Some(&Visibility::Visible)
    );
    call(&mut app, "hide");
    assert!(
        app.world_mut()
            .run_system_once(crate::button_actions::input_allowed)
            .unwrap()
    );
    assert!(
        !app.world()
            .resource::<CampaignPresentation>()
            .input_blocked()
    );
    assert_eq!(
        app.world().get::<Visibility>(overlay),
        Some(&Visibility::Hidden)
    );
    app.world_mut()
        .trigger(bevy::ui_widgets::Activate { entity: button });
    app.world_mut().flush();
    assert_eq!(
        app.world()
            .resource::<crate::button_actions::ButtonActions>()
            .entities,
        [button]
    );
    call(&mut app, "show");
    assert_eq!(app.world().resource::<Assets<Image>>().len(), 1);
    deliver(&mut app, 2, false);
    assert!(
        !app.world()
            .resource::<CampaignPresentation>()
            .input_blocked()
    );
    assert_eq!(app.world().resource::<Assets<Image>>().len(), 0);
    assert_eq!(
        app.world().get::<Visibility>(overlay),
        Some(&Visibility::Hidden)
    );
    app.world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>()
        .clear();
    app.update();
    assert!(!app.world().get::<Camera>(camera).unwrap().is_active);
}
#[test]
pub fn missing_tga_resolves_the_existing_blp_from_the_same_published_reader() {
    let mut app = app();
    deliver_art(&mut app, 1, true, Art::Blp);
    let state = app.world().resource::<CampaignPresentation>();
    assert!(state.last_error.is_none(), "{:?}", state.last_error);
    assert_eq!(
        state.texture_receipt.as_ref().unwrap().path.as_str(),
        "filter.blp"
    );
    let images = app.world().resource::<Assets<Image>>();
    assert_eq!(images.len(), 1);
    assert_eq!(
        images.iter().next().unwrap().1.data.as_ref().unwrap(),
        &[71, 47, 29, 255]
    );
}
#[test]
pub fn invalid_or_unreadable_exact_texture_does_not_fall_back_to_blp() {
    for art in [Art::MalformedTga, Art::UnreadableTga] {
        let mut app = app();
        deliver_art(&mut app, 1, true, art);
        let state = app.world().resource::<CampaignPresentation>();
        assert!(state.last_error.is_some());
        assert!(state.texture_receipt.is_none());
        assert_eq!(app.world().resource::<Assets<Image>>().len(), 0);
    }
}
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn browser_campaign_presentation_gate() -> u32 {
    published_filter_uses_its_source_and_blocks_actions_without_hiding_hud_or_pausing_time();
    missing_tga_resolves_the_existing_blp_from_the_same_published_reader();
    invalid_or_unreadable_exact_texture_does_not_fall_back_to_blp();
    3
}

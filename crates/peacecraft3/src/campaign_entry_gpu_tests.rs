//! Explicit local-asset diagnostic of the production UI and renderer.
use bevy::{
    asset::AssetPlugin,
    camera::RenderTarget,
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    prelude::*,
    render::{
        RenderPlugin,
        render_resource::{PollType, TextureFormat, TextureUsages},
        renderer::RenderDevice,
        settings::{Backends, RenderCreation, WgpuSettings},
        view::screenshot::{Screenshot, ScreenshotCaptured},
    },
    ui::{InteractionDisabled, UiSystems},
    ui_widgets::{Activate, Button},
    window::{ExitCondition, PrimaryWindow},
    winit::WinitPlugin,
};
use pcx_play::map_startup_host::MapStartupHost;
use peacecraft3::{
    campaign_camera::CampaignCamera, campaign_startup::CampaignStartup,
    map_script_session::MapScriptSession,
};
use std::{path::Path, time::Duration};

#[derive(Resource)]
struct Target(Handle<Image>);

#[derive(Resource, Default)]
struct Capture(Option<Image>);

fn offscreen(target: Res<Target>, mut cameras: Query<&mut RenderTarget>) {
    for mut camera in &mut cameras {
        if matches!(*camera, RenderTarget::Window(_)) {
            *camera = RenderTarget::Image(target.0.clone().into());
        }
    }
}

fn step(app: &mut App) {
    app.update();
    app.world()
        .resource::<RenderDevice>()
        .wgpu_device()
        .poll(PollType::Wait {
            submission_index: None,
            timeout: Some(Duration::from_secs(20)),
        })
        .expect("GPU completion");
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_none(),
        "startup: {:?}",
        app.world().resource::<CampaignStartup>().last_error
    );
}

fn until(app: &mut App, label: &str, ready: impl Fn(&mut World) -> bool) {
    for _ in 0..600 {
        step(app);
        if ready(app.world_mut()) {
            return;
        }
    }
    panic!("{label} did not complete in 600 updates");
}

fn escape(app: &mut App, state: ButtonState) {
    let window = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .unwrap();
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::Escape,
        logical_key: Key::Escape,
        state,
        repeat: false,
        window,
        text: None,
    });
}

fn activate(app: &mut App, entity: Entity) {
    assert!(app.world().entity(entity).contains::<Button>());
    assert!(!app.world().entity(entity).contains::<InteractionDisabled>());
    app.world_mut().trigger(Activate { entity });
}

fn screen(app: &mut App, expected: crate::menu_screens::Screen) {
    until(app, "menu navigation", |world| {
        let navigation = world.resource::<crate::menu_screens::Navigation>();
        navigation.screen == expected
            && navigation.requested.is_none()
            && !navigation.transition_active
    });
}

fn frame_button(app: &mut App, frame: &str) -> Entity {
    app.world_mut()
        .query::<(Entity, &crate::glue_menu::GlueButton)>()
        .iter(app.world())
        .find(|(_, b)| b.frame == frame)
        .expect(frame)
        .0
}

fn capture(app: &mut App, output: &Path) {
    assert!(!output.exists(), "capture must be new");
    for _ in 0..12 {
        step(app);
    }
    let target = app.world().resource::<Target>().0.clone();
    app.world_mut().resource_mut::<Capture>().0 = None;
    app.world_mut().spawn(Screenshot::image(target)).observe(
        |event: On<ScreenshotCaptured>, mut capture: ResMut<Capture>| {
            capture.0 = Some(event.image.clone());
        },
    );
    until(app, "GPU screenshot", |w| {
        w.resource::<Capture>().0.is_some()
    });
    let image = app.world_mut().resource_mut::<Capture>().0.take().unwrap();
    image.try_into_dynamic().unwrap().save(output).unwrap();
}

#[test]
#[ignore = "requires explicit original install/edition/map, Vulkan and a new PCX_CAMPAIGN_CAPTURE_DIR"]
fn mounted_menu_enters_the_rendered_campaign() {
    let install = std::env::var("PCX_NATIVE_CHAPTER_INSTALL").expect("PCX_NATIVE_CHAPTER_INSTALL");
    let map = std::env::var("PCX_NATIVE_CHAPTER_MAP").expect("PCX_NATIVE_CHAPTER_MAP");
    let expansion = match std::env::var("PCX_NATIVE_CHAPTER_ADDON").as_deref() {
        Ok("roc") => false,
        Ok("tft") => true,
        _ => panic!("select roc or tft explicitly"),
    };
    let output = std::path::PathBuf::from(
        std::env::var_os("PCX_CAMPAIGN_CAPTURE_DIR").expect("PCX_CAMPAIGN_CAPTURE_DIR"),
    );
    assert!(!output.exists(), "output directory must be new");
    std::fs::create_dir_all(&output).unwrap();
    // Diagnostic viewport and work bounds, not source layout or timing values.
    let [width, height] = [1280, 960];
    let plugins = DefaultPlugins
        .set(WindowPlugin {
            primary_window: Some(Window {
                resolution: (width, height).into(),
                ..default()
            }),
            exit_condition: ExitCondition::DontExit,
            ..default()
        })
        .set(AssetPlugin {
            file_path: concat!(env!("CARGO_MANIFEST_DIR"), "/assets").into(),
            ..default()
        })
        .set(RenderPlugin {
            synchronous_pipeline_compilation: true,
            render_creation: RenderCreation::Automatic(Box::new(WgpuSettings {
                backends: Some(Backends::VULKAN),
                ..default()
            })),
            ..default()
        })
        .disable::<WinitPlugin>()
        .disable::<bevy::audio::AudioPlugin>()
        .disable::<bevy::gilrs::GilrsPlugin>();
    let mut app = crate::launch_gui::app_with_plugins(plugins, |_| {});
    app.init_asset::<AudioSource>();
    app.insert_resource(crate::launch_gui::Launcher::test_source(
        install.clone(),
        expansion,
    ))
    .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        Duration::from_secs_f64(1.0 / 64.0),
    ))
    .init_resource::<Capture>();
    let mut image = Image::new_target_texture(width, height, TextureFormat::Rgba8UnormSrgb, None);
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let target = app.world_mut().resource_mut::<Assets<Image>>().add(image);
    app.insert_resource(Target(target)).add_systems(
        PostUpdate,
        offscreen
            .before(UiSystems::Layout)
            .before(bevy::camera::CameraUpdateSystems),
    );
    app.finish();
    app.cleanup();
    step(&mut app);
    let mut text = app.world_mut().query::<(Entity, &Text)>();
    let mut button = text
        .iter(app.world())
        .find(|(_, t)| t.0 == "Main menu")
        .expect("mounted launcher main-menu action")
        .0;
    while !app.world().entity(button).contains::<Button>() {
        button = app
            .world()
            .entity(button)
            .get::<ChildOf>()
            .unwrap()
            .parent();
    }
    activate(&mut app, button);
    escape(&mut app, ButtonState::Pressed);
    step(&mut app);
    escape(&mut app, ButtonState::Released);
    screen(&mut app, crate::menu_screens::Screen::Main);
    capture(&mut app, &output.join("main-menu.png"));
    let button = frame_button(&mut app, "SinglePlayerButton");
    activate(&mut app, button);
    screen(&mut app, crate::menu_screens::Screen::SinglePlayer);
    let button = frame_button(&mut app, "CampaignButton");
    activate(&mut app, button);
    screen(&mut app, crate::menu_screens::Screen::Campaign);
    let chapters = app.world().resource::<crate::native_campaign::Chapters>();
    assert!(chapters.error.is_none(), "{:?}", chapters.error);
    let (campaign, mission) = chapters
        .campaigns
        .iter()
        .enumerate()
        .find_map(|(c, entry)| {
            entry
                .missions
                .iter()
                .position(|m| m.map.eq_ignore_ascii_case(&map))
                .map(|m| (c, m))
        })
        .expect("map in selected source index");
    assert!(
        chapters.campaigns[campaign].default_open,
        "campaign must be source-unlocked"
    );
    let button = app
        .world_mut()
        .query::<(Entity, &crate::native_campaign::CampaignButton)>()
        .iter(app.world())
        .find(|(_, b)| b.0 == campaign)
        .unwrap()
        .0;
    activate(&mut app, button);
    until(&mut app, "chapter panel", |w| {
        w.resource::<crate::native_campaign::CampaignMenuState>()
            .selected_campaign
            == Some(campaign)
    });
    assert!(
        app.world()
            .resource::<MapScriptSession<MapStartupHost>>()
            .context()
            .is_none()
    );
    capture(&mut app, &output.join("chapter-selection.png"));
    let button = app
        .world_mut()
        .query::<(Entity, &crate::native_campaign::MissionButton)>()
        .iter(app.world())
        .find(|(_, b)| b.campaign == campaign && b.mission == mission)
        .unwrap()
        .0;
    activate(&mut app, button);
    until(&mut app, "campaign startup", |w| {
        w.resource::<CampaignCamera>().active()
            && w.resource::<MapScriptSession<MapStartupHost>>()
                .context()
                .is_some()
    });
    capture(&mut app, &output.join("campaign-entry.png"));
    escape(&mut app, ButtonState::Pressed);
    step(&mut app);
    escape(&mut app, ButtonState::Released);
    until(&mut app, "campaign control", |w| {
        w.resource_mut::<MapScriptSession<MapStartupHost>>()
            .host()
            .is_some_and(|h| {
                h.objects().presentation().is_some_and(|p| {
                    p.control_enabled == Some(true) && p.interface_visible == Some(true)
                })
            })
    });
    assert!(
        app.world()
            .resource::<crate::campaign_actors::UnitActors>()
            .last_error
            .is_none()
    );
    assert!(
        app.world()
            .resource::<peacecraft3::map_ground::GroundPublication>()
            .last_error
            .is_none()
    );
    capture(&mut app, &output.join("campaign-control.png"));
    std::fs::write(output.join("scope.json"), serde_json::to_vec_pretty(&serde_json::json!({
        "install": install, "map": map, "expansion": expansion,
        "scope": "Production app offscreen Vulkan; mounted widget activation and synthetic keyboard messages; no physical input, audio playback or original paired-frame parity claim"
    })).unwrap()).unwrap();
}

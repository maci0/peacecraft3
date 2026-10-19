//! Actual render graph, synthetic art only. Original-client parity is separate.
use super::*;
use bevy::{
    asset::AssetPlugin,
    camera::RenderTarget,
    render::{
        RenderPlugin,
        gpu_readback::{Readback, ReadbackComplete},
        render_resource::{PollType, TextureUsages},
        renderer::RenderDevice,
        settings::{Backends, RenderCreation, WgpuSettings},
    },
    window::ExitCondition,
    winit::WinitPlugin,
};
use std::time::Duration;
#[derive(Resource, Default)]
struct Capture(Vec<u8>);
fn render_app() -> App {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..default()
            })
            .set(AssetPlugin::default())
            .set(RenderPlugin {
                synchronous_pipeline_compilation: true,
                render_creation: RenderCreation::Automatic(Box::new(WgpuSettings {
                    backends: Some(Backends::VULKAN),
                    ..default()
                })),
                ..default()
            })
            .disable::<WinitPlugin>()
            .disable::<bevy::log::LogPlugin>()
            .disable::<bevy::audio::AudioPlugin>()
            .disable::<bevy::gilrs::GilrsPlugin>(),
    );
    bevy::asset::embedded_asset!(&mut app, "campaign_filter.wgsl");
    app.add_plugins(UiMaterialPlugin::<FilterMaterial>::default())
        .init_resource::<Capture>()
        .init_resource::<CampaignPresentation>()
        .add_systems(
            PostUpdate,
            interface_visibility
                .before(bevy::ui::UiSystems::Prepare)
                .before(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate),
        );
    app.finish();
    app.cleanup();
    app
}
#[test]
#[ignore = "requires a Vulkan adapter"]
fn synthetic_filter_alpha_reaches_the_gpu_and_scales_with_the_view() {
    let mut app = render_app();
    let mut art = pcx_gfx::viewer::rgba_image(&pcx_gfx::LoadedTexture {
        source: "synthetic".into(),
        width: 1,
        height: 1,
        rgba: vec![128, 64, 32, 128],
    });
    art.texture_descriptor.format = TextureFormat::Rgba8Unorm;
    let art = app.world_mut().resource_mut::<Assets<Image>>().add(art);
    let material = app
        .world_mut()
        .resource_mut::<Assets<FilterMaterial>>()
        .add(FilterMaterial {
            texture: art,
            uv: Vec4::new(1., 1., 0., 0.),
            color: Vec4::new(0.5, 1., 0.5, 0.5),
        });
    let mut previous: Option<(Entity, Entity)> = None;
    for (width, height) in [(64, 32), (128, 64), (3840, 2160)] {
        if let Some((scene, ui)) = previous.take() {
            app.world_mut().despawn(scene);
            app.world_mut().despawn(ui);
        }
        let mut image =
            Image::new_target_texture(width, height, TextureFormat::Rgba8UnormSrgb, None);
        image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
        let target = app.world_mut().resource_mut::<Assets<Image>>().add(image);
        let scene = app
            .world_mut()
            .spawn((
                Camera3d::default(),
                Camera {
                    clear_color: ClearColorConfig::Custom(Color::srgb(0.2, 0.4, 0.6)),
                    ..default()
                },
                bevy::core_pipeline::tonemapping::Tonemapping::None,
                bevy::core_pipeline::tonemapping::DebandDither::Disabled,
                Msaa::Off,
                RenderTarget::Image(target.clone().into()),
                RenderLayers::none(),
            ))
            .id();
        let camera = app
            .world_mut()
            .spawn((
                Camera2d,
                Camera {
                    order: 1,
                    clear_color: ClearColorConfig::None,
                    ..default()
                },
                Msaa::Off,
                RenderTarget::Image(target.clone().into()),
            ))
            .id();
        let overlay = app
            .world_mut()
            .spawn((
                fullscreen(),
                UiTargetCamera(camera),
                MaterialNode(material.clone()),
            ))
            .id();
        app.world_mut().resource_mut::<Capture>().0.clear();
        let readback = app
            .world_mut()
            .spawn(Readback::texture(target))
            .observe(
                |event: On<ReadbackComplete>, mut capture: ResMut<Capture>| {
                    capture.0.clone_from(&event.data)
                },
            )
            .id();
        for _ in 0..32 {
            app.update();
            app.world()
                .resource::<RenderDevice>()
                .wgpu_device()
                .poll(PollType::Wait {
                    submission_index: None,
                    timeout: Some(Duration::from_secs(10)),
                })
                .unwrap();
        }
        let data = &app.world().resource::<Capture>().0;
        assert!(!data.is_empty());
        assert!(data.len().is_multiple_of(height as usize));
        let stride = data.len() / height as usize;
        for (x, y) in [
            (1, 1),
            (width - 2, 1),
            (width / 2, height / 2),
            (1, height - 2),
            (width - 2, height - 2),
        ] {
            let at = y as usize * stride + x as usize * 4;
            let alpha = (128. / 255.) * 0.5;
            for (channel, (foreground, background)) in [
                (128. / 255. * 0.5, 0.2),
                (64. / 255., 0.4),
                (32. / 255. * 0.5, 0.6),
            ]
            .into_iter()
            .enumerate()
            {
                let foreground = Color::srgba(foreground, foreground, foreground, 1.)
                    .to_linear()
                    .red;
                let background = Color::srgba(background, background, background, 1.)
                    .to_linear()
                    .red;
                let blended = foreground * alpha + background * (1. - alpha);
                let encoded = bevy::color::Srgba::from(bevy::color::LinearRgba::new(
                    blended, blended, blended, 1.,
                ))
                .red;
                let expected = (encoded * 255.0).round() as u8;
                assert!(
                    data[at + channel].abs_diff(expected) <= 2,
                    "{width}x{height}: channel {channel}: {} vs {expected}",
                    data[at + channel]
                );
            }
        }
        app.world_mut().despawn(readback);
        app.world_mut().despawn(overlay);
        previous = Some((scene, camera));
    }
}

#[test]
#[ignore = "requires a Vulkan adapter; synthetic ordinary-HUD visibility through 4K"]
fn interface_hides_explicitly_visible_children_and_restores_the_source_layout() {
    let mut app = render_app();
    for (width, height) in [(64, 32), (128, 64), (3840, 2160)] {
        let mut image =
            Image::new_target_texture(width, height, TextureFormat::Rgba8UnormSrgb, None);
        image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
        let target = app.world_mut().resource_mut::<Assets<Image>>().add(image);
        let camera = app
            .world_mut()
            .spawn((
                Camera2d,
                Camera {
                    clear_color: ClearColorConfig::Custom(Color::BLACK),
                    ..default()
                },
                RenderTarget::Image(target.clone().into()),
                Msaa::Off,
            ))
            .id();
        let root = app
            .world_mut()
            .spawn((
                Node {
                    width: percent(100),
                    height: percent(100),
                    display: Display::Grid,
                    ..default()
                },
                CampaignInterface,
                UiTargetCamera(camera),
            ))
            .id();
        let child = app
            .world_mut()
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(50),
                    height: percent(100),
                    ..default()
                },
                BackgroundColor(Color::srgb(1., 0., 0.)),
                Visibility::Visible,
                ChildOf(root),
            ))
            .id();
        let text_layer = app
            .world_mut()
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: percent(50),
                    width: percent(50),
                    height: percent(100),
                    ..default()
                },
                BackgroundColor(Color::srgb(0., 1., 0.)),
                UiTargetCamera(camera),
            ))
            .id();
        let readback = app
            .world_mut()
            .spawn(Readback::texture(target.clone()))
            .observe(
                |event: On<ReadbackComplete>, mut capture: ResMut<Capture>| {
                    capture.0.clone_from(&event.data);
                },
            )
            .id();
        for hide in [false, true, false] {
            app.world_mut()
                .resource_mut::<CampaignPresentation>()
                .hide_interface = hide;
            app.world_mut().resource_mut::<Capture>().0.clear();
            for _ in 0..32 {
                app.update();
                app.world()
                    .resource::<RenderDevice>()
                    .wgpu_device()
                    .poll(PollType::Wait {
                        submission_index: None,
                        timeout: Some(Duration::from_secs(10)),
                    })
                    .unwrap();
            }
            assert_eq!(
                app.world().get::<Visibility>(child),
                Some(&Visibility::Visible)
            );
            assert_eq!(
                app.world().get::<Node>(root).unwrap().display,
                if hide { Display::None } else { Display::Grid }
            );
            let data = &app.world().resource::<Capture>().0;
            assert!(!data.is_empty());
            assert!(data.len().is_multiple_of(height as usize));
            let stride = data.len() / height as usize;
            for (x, expected) in [
                (width / 4, if hide { [0, 0, 0] } else { [255, 0, 0] }),
                (width * 3 / 4, [0, 255, 0]),
            ] {
                let at = (height / 2) as usize * stride + x as usize * 4;
                assert_eq!(&data[at..at + 3], &expected, "{width}x{height} hide={hide}");
            }
            println!(
                "ordinary HUD suppression extent={width}x{height} hidden={hide}; separate cinematic layer preserved"
            );
        }
        for entity in [root, text_layer, readback, camera] {
            app.world_mut().despawn(entity);
        }
        app.world_mut()
            .resource_mut::<Assets<Image>>()
            .remove(target.id());
    }
}

use super::*;
use bevy::{
    asset::AssetPlugin,
    render::{
        RenderPlugin,
        gpu_readback::{Readback, ReadbackComplete},
        render_resource::PollType,
        settings::{Backends, RenderCreation, WgpuSettings},
    },
    window::ExitCondition,
    winit::WinitPlugin,
};
use std::time::Duration;
#[derive(Resource, Default)]
struct Capture(Vec<u8>);
fn app() -> App {
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
            .disable::<bevy::audio::AudioPlugin>(),
    );
    install(&mut app);
    app.init_resource::<Capture>();
    app.finish();
    app.cleanup();
    app
}
fn steps(app: &mut App) {
    for _ in 0..24 {
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
}
#[test]
#[ignore = "requires an isolated Vulkan adapter; actual supersampling and display/UI targets"]
fn world_scales_but_display_ui_coordinates_and_lifecycle_do_not() {
    let mut app = app();
    let mut image = Image::new_target_texture(64, 32, TextureFormat::Rgba8UnormSrgb, None);
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let output = app.world_mut().resource_mut::<Assets<Image>>().add(image);
    let target = RenderTarget::Image(ImageRenderTarget {
        handle: output.clone(),
        scale_factor: 2.,
    });
    let scene = app
        .world_mut()
        .spawn((
            CampaignSceneCamera,
            Camera3d::default(),
            Camera {
                clear_color: ClearColorConfig::Custom(Color::srgb(0.25, 0.5, 0.75)),
                ..default()
            },
            RenderLayers::none(),
            target.clone(),
            bevy::core_pipeline::tonemapping::Tonemapping::None,
            Msaa::Off,
        ))
        .id();
    let sky = app
        .world_mut()
        .spawn((
            SkyCamera,
            Camera3d::default(),
            Camera {
                order: -1,
                ..default()
            },
            RenderLayers::none(),
            target.clone(),
            Msaa::Off,
        ))
        .id();
    let ui = app
        .world_mut()
        .spawn((
            Camera2d,
            Camera {
                order: 1,
                clear_color: ClearColorConfig::None,
                ..default()
            },
            target,
            RenderLayers::none(),
        ))
        .id();
    let readback = app
        .world_mut()
        .spawn(Readback::texture(output.clone()))
        .observe(
            |event: On<ReadbackComplete>, mut capture: ResMut<Capture>| {
                capture.0.clone_from(&event.data)
            },
        )
        .id();
    steps(&mut app);
    let before = app
        .world()
        .get::<Camera>(scene)
        .unwrap()
        .logical_viewport_size()
        .unwrap();
    let original_ray = app
        .world()
        .get::<Camera>(scene)
        .unwrap()
        .viewport_to_world(
            app.world().get::<GlobalTransform>(scene).unwrap(),
            Vec2::new(9., 7.),
        )
        .unwrap();
    for factor in 1..=4 {
        app.world_mut().resource_mut::<InternalResolution>().factor = factor;
        steps(&mut app);
        let world = app.world();
        let camera = world.get::<Camera>(scene).unwrap();
        assert_eq!(camera.logical_viewport_size().unwrap(), before);
        let ray = camera
            .viewport_to_world(
                world.get::<GlobalTransform>(scene).unwrap(),
                Vec2::new(9., 7.),
            )
            .unwrap();
        assert!(ray.direction.abs_diff_eq(*original_ray.direction, 0.00001));
        let scene_target = world
            .get::<RenderTarget>(scene)
            .unwrap()
            .as_image()
            .unwrap();
        assert_eq!(
            world
                .resource::<Assets<Image>>()
                .get(scene_target)
                .unwrap()
                .size(),
            UVec2::new(64 * factor, 32 * factor)
        );
        assert_eq!(
            world.get::<RenderTarget>(ui).unwrap().as_image().unwrap(),
            &output
        );
        if factor > 1 {
            assert_eq!(
                world.get::<RenderTarget>(sky).unwrap().as_image().unwrap(),
                scene_target
            );
        }
        let pixels = &world.resource::<Capture>().0;
        assert!(!pixels.is_empty());
        let at = (pixels.len() / 32) * 16 + 32 * 4;
        for (i, expected) in [64u8, 128, 191].into_iter().enumerate() {
            assert!(
                pixels[at + i].abs_diff(expected) <= 3,
                "factor {factor}: {:?}",
                &pixels[at..at + 4]
            );
        }
    }
    let owned = app
        .world()
        .resource::<InternalResolution>()
        .pass
        .as_ref()
        .unwrap()
        .image
        .clone();
    app.world_mut()
        .resource_mut::<Assets<Image>>()
        .get_mut(&output)
        .unwrap()
        .resize(Extent3d {
            width: 80,
            height: 40,
            depth_or_array_layers: 1,
        });
    steps(&mut app);
    assert!(
        app.world()
            .resource::<Assets<Image>>()
            .get(&owned)
            .is_none()
    );
    let pass = app
        .world()
        .resource::<InternalResolution>()
        .pass
        .as_ref()
        .unwrap();
    assert_eq!(pass.size, UVec2::new(320, 160));
    let owned = pass.image.clone();
    app.world_mut().get_mut::<Camera>(scene).unwrap().is_active = false;
    steps(&mut app);
    assert!(app.world().resource::<InternalResolution>().pass.is_none());
    assert!(
        app.world()
            .resource::<Assets<Image>>()
            .get(&owned)
            .is_none()
    );
    assert_eq!(
        app.world()
            .get::<RenderTarget>(scene)
            .unwrap()
            .as_image()
            .unwrap(),
        &output
    );
    app.world_mut().despawn(readback);
}

#[test]
#[ignore = "requires an isolated Vulkan adapter; full NxN shader averaging"]
fn every_subpixel_contributes_to_the_downsample() {
    let mut app = app();
    for factor in 2..=4 {
        let size = 8 * factor;
        let mut data = Vec::new();
        for y in 0..size {
            for x in 0..size {
                data.extend_from_slice(if x % factor == 0 && y % factor == 0 {
                    &[255, 255, 255, 255]
                } else {
                    &[0, 0, 0, 255]
                });
            }
        }
        let source = Image::new(
            Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            data,
            TextureFormat::Rgba8UnormSrgb,
            default(),
        );
        let source = app.world_mut().resource_mut::<Assets<Image>>().add(source);
        let material = app
            .world_mut()
            .resource_mut::<Assets<Downsample>>()
            .add(Downsample {
                image: source,
                factor,
            });
        let mut output = Image::new_target_texture(8, 8, TextureFormat::Rgba8UnormSrgb, None);
        output.texture_descriptor.usage |= TextureUsages::COPY_SRC;
        let output = app.world_mut().resource_mut::<Assets<Image>>().add(output);
        let camera = app
            .world_mut()
            .spawn((
                Camera2d,
                RenderTarget::Image(output.clone().into()),
                RenderLayers::none(),
            ))
            .id();
        let root = app
            .world_mut()
            .spawn((
                Node {
                    width: percent(100),
                    height: percent(100),
                    ..default()
                },
                UiTargetCamera(camera),
                MaterialNode(material),
            ))
            .id();
        app.world_mut().resource_mut::<Capture>().0.clear();
        let readback = app
            .world_mut()
            .spawn(Readback::texture(output))
            .observe(|e: On<ReadbackComplete>, mut capture: ResMut<Capture>| {
                capture.0.clone_from(&e.data)
            })
            .id();
        steps(&mut app);
        let expected = (Color::linear_rgb(1. / (factor * factor) as f32, 0., 0.)
            .to_srgba()
            .red
            * 255.)
            .round() as u8;
        let data = &app.world().resource::<Capture>().0;
        assert!(!data.is_empty());
        let stride = data.len() / 8;
        for y in 0..8 {
            for x in 0..8 {
                let at = y * stride + x * 4;
                assert!(
                    data[at].abs_diff(expected) <= 2,
                    "factor {factor}: {} vs {expected}",
                    data[at]
                );
            }
        }
        for entity in [root, camera, readback] {
            app.world_mut().despawn(entity);
        }
    }
}

#[derive(Resource)]
struct Output(Handle<Image>);
#[test]
#[ignore = "requires isolated Vulkan; production launcher layout and activation"]
fn launcher_renders_and_activates_the_scale_without_original_menu_controls() {
    let plugins = DefaultPlugins
        .set(WindowPlugin {
            primary_window: None,
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
        .disable::<bevy::log::LogPlugin>();
    let mut app = crate::launch_gui::app_with_plugins(plugins, |_| {});
    app.init_asset::<AudioSource>().init_resource::<Capture>();
    let mut image = Image::new_target_texture(880, 1000, TextureFormat::Rgba8UnormSrgb, None);
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let image = app.world_mut().resource_mut::<Assets<Image>>().add(image);
    app.insert_resource(Output(image.clone())).add_systems(
        PostUpdate,
        (|output: Res<Output>, mut cameras: Query<&mut RenderTarget>| {
            for mut target in &mut cameras {
                if matches!(*target, RenderTarget::Window(_)) {
                    *target = RenderTarget::Image(output.0.clone().into());
                }
            }
        })
        .before(CameraUpdateSystems),
    );
    app.finish();
    app.cleanup();
    let readback = app
        .world_mut()
        .spawn(Readback::texture(image.clone()))
        .observe(|e: On<ReadbackComplete>, mut capture: ResMut<Capture>| {
            capture.0.clone_from(&e.data)
        })
        .id();
    for (width, height) in [(480, 1000), (880, 1000), (1280, 1000)] {
        app.world_mut()
            .resource_mut::<Assets<Image>>()
            .get_mut(&image)
            .unwrap()
            .resize(Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            });
        steps(&mut app);
        let labels: Vec<_> = app
            .world_mut()
            .query::<(Entity, &Text, &ComputedNode)>()
            .iter(app.world())
            .filter(|(_, t, _)| matches!(t.0.as_str(), "1×" | "2×" | "3×" | "4×"))
            .map(|(e, t, n)| (e, t.0.clone(), n.size()))
            .collect();
        assert_eq!(labels.len(), 4);
        for (_, _, size) in &labels {
            assert!(size.min_element() > 0.);
            assert!(size.x < width as f32);
        }
        let mut button = labels.iter().find(|(_, t, _)| t == "2×").unwrap().0;
        while !app
            .world()
            .entity(button)
            .contains::<bevy::ui_widgets::Button>()
        {
            button = app.world().get::<ChildOf>(button).unwrap().parent();
        }
        app.world_mut()
            .trigger(bevy::ui_widgets::Activate { entity: button });
        steps(&mut app);
        assert_eq!(app.world().resource::<InternalResolution>().factor, 2);
        assert!(app.world().resource::<InternalResolution>().pass.is_none());
        let data = &app.world().resource::<Capture>().0;
        let stride = data.len() / height as usize;
        assert!(stride >= width as usize * 4);
        if let Ok(directory) = std::env::var("PCX_INTERNAL_RESOLUTION_CAPTURE") {
            let pixels: Vec<u8> = data
                .chunks(stride)
                .flat_map(|row| row[..width as usize * 4].iter().copied())
                .collect();
            let output = Image::new(
                Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                pixels,
                TextureFormat::Rgba8UnormSrgb,
                default(),
            );
            output
                .try_into_dynamic()
                .unwrap()
                .save(std::path::Path::new(&directory).join(format!("launcher-{width}.png")))
                .unwrap();
        }
        app.world_mut()
            .entity_mut(button)
            .remove::<bevy::ui::Pressed>();
    }
    app.world_mut().despawn(readback);
}

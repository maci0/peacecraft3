//! Draws each pointer state of the Human cursor side by side on an offscreen Vulkan target
//! and writes a binary PPM to `PCX_CURSOR_CAPTURE`. No window is opened.
use super::*;
use bevy::{
    asset::AssetPlugin,
    camera::RenderTarget,
    pbr::MaterialPlugin,
    render::{
        RenderPlugin,
        gpu_readback::{Readback, ReadbackComplete},
        render_resource::{PollType, TextureFormat, TextureUsages},
        renderer::RenderDevice,
        settings::{Backends, RenderCreation, WgpuSettings},
    },
    window::ExitCondition,
    winit::WinitPlugin,
};
use pcx_assets::{Addon, AssetMode, Behaviour, Build, GameMode, OverrideDirectory, Resolver};
use pcx_play::camera_input::edge;

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 960;
const FRAMES: usize = 40;
const READ_AT: usize = 30;

#[derive(Resource, Default)]
struct Shot(Vec<u8>);

fn hover(relation: Relation) -> Option<Hovered> {
    Some(Hovered {
        relation,
        selected: false,
    })
}

#[test]
#[ignore = "requires PCX_WC3_INSTALL, PCX_CURSOR_ASSETS, PCX_CURSOR_CAPTURE and a Vulkan adapter"]
fn draw_every_pointer_state_offscreen() {
    let out = std::env::var_os("PCX_CURSOR_CAPTURE").expect("PCX_CURSOR_CAPTURE");
    let assets = std::env::var_os("PCX_CURSOR_ASSETS").expect("PCX_CURSOR_ASSETS");
    let mode = GameMode {
        build: Build::V1_29b,
        addon: Addon::Tft,
        assets: AssetMode::Vanilla,
        behaviour: Behaviour::Vanilla,
    };
    let reader = Resolver::new(
        mode,
        Some(
            OverrideDirectory::open(std::path::Path::new(&assets), "cursor-capture", None)
                .unwrap(),
        ),
        Vec::new(),
    )
    .unwrap();
    let install = std::env::var("PCX_WC3_INSTALL").expect("PCX_WC3_INSTALL");
    let exe = std::path::Path::new(&install).join("Warcraft III.exe");
    let rules = CursorRules::read(
        &std::fs::read(&exe).unwrap(),
        AssetIdentity {
            source: exe.display().to_string(),
            edition: "Legacy target".into(),
            version: "1.29.2.9231".into(),
        },
    )
    .unwrap();
    let path = model_path(&reader, Some("RACE_PREF_HUMAN".into())).unwrap();
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
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
            .disable::<bevy::log::LogPlugin>()
            .disable::<bevy::audio::AudioPlugin>(),
    )
    .add_plugins(crate::menu_composition::MenuCompositionPlugin)
    .add_plugins(crate::mainmenu_scene::ParticleMaterialPlugin)
    .add_plugins(MaterialPlugin::<MenuMaterial>::default())
    .init_resource::<Shot>()
    .add_systems(Update, campaign_score_model::animate_backgrounds);
    app.finish();
    app.cleanup();
    let armed = |armed, hover| Facts {
        armed,
        hover,
        ..default()
    };
    let states: Vec<Facts> = vec![
        Facts::default(),
        armed(Armed::Nothing, hover(Relation::Friend)),
        armed(Armed::Nothing, hover(Relation::Neutral)),
        armed(Armed::Nothing, hover(Relation::Enemy)),
        armed(Armed::Target, None),
        armed(Armed::Attack, hover(Relation::Friend)),
        armed(Armed::Attack, hover(Relation::Enemy)),
        Facts {
            edges: edge::UP | edge::LEFT,
            ..default()
        },
        Facts {
            edges: edge::DOWN | edge::RIGHT,
            ..default()
        },
    ];
    let spacing = FRAME_H * 0.12;
    let first = -spacing * (states.len() as f32 - 1.0) / 2.0;
    for (i, state) in states.iter().enumerate() {
        let Prepared { plan, sequences, .. } = prepare(&reader, &path, &rules).unwrap();
        let root = app
            .world_mut()
            .spawn((
                Transform::from_xyz(first + spacing * i as f32, 0.0, 0.0),
                Visibility::default(),
            ))
            .id();
        let world = app.world_mut();
        let mut meshes = world.remove_resource::<Assets<Mesh>>().unwrap();
        let mut materials = world.remove_resource::<Assets<MenuMaterial>>().unwrap();
        let mut images = world.remove_resource::<Assets<Image>>().unwrap();
        let mut commands = world.commands();
        campaign_score_model::spawn_backgrounds_on(
            &mut commands,
            crate::mainmenu_scene::GeosetAssets {
                meshes: &mut meshes,
                materials: &mut materials,
                images: &mut images,
            },
            root,
            vec![plan],
            CURSOR_LAYER,
        )
        .unwrap();
        world.flush();
        let mut query = world.query::<(&ChildOf, &mut ScoreBackgroundState)>();
        let mut found = query
            .iter_mut(world)
            .find(|(parent, _)| parent.parent() == root)
            .unwrap()
            .1;
        let look = rules.look(pointer(*state).0);
        apply(look, &sequences, &mut found, &mut materials).unwrap();
        world.insert_resource(meshes);
        world.insert_resource(materials);
        world.insert_resource(images);
    }
    let mut image = Image::new_target_texture(WIDTH, HEIGHT, TextureFormat::Rgba8UnormSrgb, None);
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let target = app.world_mut().resource_mut::<Assets<Image>>().add(image);
    app.world_mut().spawn((
        Camera3d::default(),
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical {
                viewport_height: FRAME_H,
            },
            near: -DEPTH_RANGE,
            far: DEPTH_RANGE,
            ..OrthographicProjection::default_3d()
        }),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb(0.25, 0.45, 0.2)),
            ..default()
        },
        RenderTarget::Image(target.clone().into()),
        Tonemapping::None,
        DebandDither::Disabled,
        RenderLayers::layer(CURSOR_LAYER),
    ));
    for frame in 0..FRAMES {
        if frame == READ_AT {
            app.world_mut()
                .spawn(Readback::texture(target.clone()))
                .observe(|event: On<ReadbackComplete>, mut shot: ResMut<Shot>| {
                    shot.0.clone_from(&event.data);
                });
        }
        app.update();
        app.world()
            .resource::<RenderDevice>()
            .wgpu_device()
            .poll(PollType::Wait {
                submission_index: None,
                timeout: Some(std::time::Duration::from_secs(10)),
            })
            .unwrap();
    }
    let data = &app.world().resource::<Shot>().0;
    let (width, height) = (WIDTH as usize, HEIGHT as usize);
    assert!(
        !data.is_empty() && data.len().is_multiple_of(height),
        "{}",
        data.len()
    );
    let stride = data.len() / height;
    // Each cursor's mean colour over the pixels that differ from the background.
    let pixel = |x: usize, y: usize| -> [f32; 3] {
        let at = y * stride + x * 4;
        [data[at], data[at + 1], data[at + 2]].map(f32::from)
    };
    let background = pixel(0, 0);
    let mean = |i: usize| -> ([f32; 3], usize) {
        let centre = (WIDTH as f32 / 2.0 + (first + spacing * i as f32) / FRAME_H * HEIGHT as f32)
            as usize;
        let (mut sum, mut count) = ([0.0f32; 3], 0usize);
        for y in HEIGHT as usize / 2 - 60..HEIGHT as usize / 2 + 60 {
            for x in centre - 45..centre + 45 {
                let colour = pixel(x, y);
                if colour.iter().zip(background).any(|(c, b)| (c - b).abs() > 40.0) {
                    count += 1;
                    for channel in 0..3 {
                        sum[channel] += colour[channel];
                    }
                }
            }
        }
        (sum.map(|total| total / count.max(1) as f32), count)
    };
    for i in 0..states.len() {
        assert!(mean(i).1 > 150, "cursor {i} drew {} pixels", mean(i).1);
    }
    let [normal, friend, neutral, enemy, plain, attack_friend, attack_enemy] =
        [0, 1, 2, 3, 4, 5, 6].map(|i| mean(i).0);
    assert!(friend[1] > friend[0] + 60.0 && friend[1] > friend[2] + 60.0, "{friend:?}");
    assert!(neutral[0] > neutral[2] + 60.0 && neutral[1] > neutral[2] + 60.0, "{neutral:?}");
    assert!(enemy[0] > enemy[1] + 60.0 && enemy[0] > enemy[2] + 60.0, "{enemy:?}");
    assert!(normal[2] >= normal[0] && normal[1] > normal[0], "grey-blue {normal:?}");
    assert!(plain[2] > plain[0] + 40.0, "cyan target {plain:?}");
    assert!(attack_friend[1] > attack_friend[0] + 60.0, "{attack_friend:?}");
    assert!(attack_enemy[0] > attack_enemy[1] + 60.0, "{attack_enemy:?}");
    let mut ppm = format!("P6\n{WIDTH} {HEIGHT}\n255\n").into_bytes();
    for row in data.chunks(stride).take(height) {
        for pixel in row[..width * 4].chunks(4) {
            ppm.extend_from_slice(&pixel[..3]);
        }
    }
    std::fs::write(out, ppm).unwrap();
}

//! Synthetic render-graph check; original-client colour-space parity is separate.
use crate::{
    mainmenu_scene::apply_geoset_colour,
    menu_material::{MenuMaterial, checked_source_mode},
};
use bevy::{
    asset::AssetPlugin,
    camera::{Hdr, RenderTarget, ScalingMode},
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    pbr::MaterialPlugin,
    prelude::*,
    render::{
        RenderPlugin,
        gpu_readback::{Readback, ReadbackComplete},
        render_resource::{PollType, TextureFormat, TextureUsages},
        renderer::{RenderAdapterInfo, RenderDevice},
        settings::{Backends, RenderCreation, WgpuSettings},
    },
    window::ExitCondition,
    winit::WinitPlugin,
};
use pcx_gfx::{
    mdx_opacity::OpacityPlan,
    mdx_runtime::{GeosetInput, RuntimeInput, RuntimePlan},
};
use std::time::Duration;

fn plan(animated: bool) -> OpacityPlan {
    let mut data = Vec::new();
    for word in [
        0u32,
        0,
        u32::from_le_bytes(*b"LAYS"),
        1,
        28,
        0,
        0,
        0,
        pcx_mdl::NO_ID,
        0,
        1f32.to_bits(),
    ] {
        data.extend(word.to_le_bytes());
    }
    let mut raw = Vec::new();
    for (frame, colour) in [(0u32, [1.0f32, 0.0, 0.0]), (100, [0.0, 0.0, 1.0])] {
        raw.extend(frame.to_le_bytes());
        for value in colour {
            raw.extend(value.to_le_bytes());
        }
    }
    let runtime = RuntimePlan::checked(RuntimeInput {
        bones: vec![],
        helpers: vec![],
        pivots: pcx_mdl::Pivots(vec![]),
        geosets: (0..2)
            .map(|_| GeosetInput {
                vertex_count: 0,
                material_id: 0,
                matrix_joint_ids: vec![],
            })
            .collect(),
        geoset_animations: vec![pcx_mdl::GeosetAnim {
            offset: 0,
            len: 60,
            flags: 0x2,
            geoset_id: 1,
            static_alpha: 1.0,
            static_colour: [0.25, 0.5, 0.75],
            tracks: if animated {
                vec![pcx_mdl::AnimTrack {
                    magic: *b"KGAC",
                    count: 2,
                    interpolation: pcx_mdl::Interpolation::Linear,
                    global_seq_id: pcx_mdl::NO_ID,
                    raw,
                }]
            } else {
                vec![]
            },
            trailing: 0,
        }],
        materials: vec![pcx_mdl::Material {
            offset: 0,
            len: data.len() as u32 + 4,
            data,
        }],
        textures: vec![pcx_mdl::Texture {
            slot: 0,
            path: "synthetic".into(),
            flags: 0,
        }],
        texture_animations: vec![],
        global_sequences: vec![],
    })
    .unwrap();
    OpacityPlan::checked(&runtime, [0, 100]).unwrap()
}

#[derive(Resource, Default)]
struct Capture {
    completed: usize,
    last: Vec<u8>,
}

fn half(bytes: &[u8]) -> f32 {
    let bits = u16::from_le_bytes(bytes.try_into().unwrap());
    let sign = if bits & 0x8000 == 0 { 1.0 } else { -1.0 };
    let exponent = i32::from((bits >> 10) & 31);
    let mantissa = f32::from(bits & 1023);
    match exponent {
        0 => sign * mantissa * 2_f32.powi(-24),
        31 => {
            if mantissa == 0.0 {
                sign * f32::INFINITY
            } else {
                f32::NAN
            }
        }
        _ => sign * (1.0 + mantissa / 1024.0) * 2_f32.powi(exponent - 15),
    }
}

#[test]
#[ignore = "requires a local Vulkan render adapter; synthetic GPU readback"]
fn authored_geoset_colour_reaches_the_menu_material_render_graph() {
    const WIDTH: u32 = 128;
    const HEIGHT: u32 = 32;
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
            .disable::<bevy::audio::AudioPlugin>()
            .disable::<bevy::gilrs::GilrsPlugin>(),
    )
    .add_plugins(MaterialPlugin::<MenuMaterial>::default())
    .init_resource::<Capture>();
    app.finish();
    app.cleanup();
    println!(
        "colour-readback adapter={:?}",
        **app.world().resource::<RenderAdapterInfo>()
    );
    let mut image = Image::new_target_texture(WIDTH, HEIGHT, TextureFormat::Rgba16Float, None);
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let target = app.world_mut().resource_mut::<Assets<Image>>().add(image);
    app.world_mut().spawn((
        Camera3d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            ..default()
        },
        RenderTarget::Image(target.clone().into()),
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::Fixed {
                width: 4.0,
                height: 1.0,
            },
            near: 0.1,
            far: 20.0,
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_xyz(0.0, 0.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
        Hdr,
        Tonemapping::None,
        DebandDither::Disabled,
        Msaa::Off,
    ));
    let quad = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(Rectangle::new(0.75, 0.75));
    let static_plan = plan(false);
    let animated_plan = plan(true);
    for (column, (plan, geoset, frame)) in [
        (&static_plan, 1, 0),
        (&animated_plan, 1, 0),
        (&animated_plan, 1, 100),
        (&static_plan, 0, 0),
    ]
    .into_iter()
    .enumerate()
    {
        let mut materials = app.world_mut().resource_mut::<Assets<MenuMaterial>>();
        let handle = materials.add(
            checked_source_mode(
                StandardMaterial {
                    base_color: Color::srgba(0.1, 0.2, 0.3, 0.7),
                    unlit: true,
                    fog_enabled: false,
                    cull_mode: None,
                    alpha_mode: AlphaMode::Opaque,
                    ..default()
                },
                0,
            )
            .unwrap(),
        );
        apply_geoset_colour(
            plan,
            &[(geoset, Entity::PLACEHOLDER, handle.clone())],
            frame,
            0,
            &mut materials,
        )
        .unwrap();
        assert_eq!(materials.get(&handle).unwrap().base.base_color.alpha(), 0.7);
        app.world_mut().spawn((
            Mesh3d(quad.clone()),
            MeshMaterial3d(handle),
            Transform::from_xyz(column as f32 - 1.5, 0.0, 0.0),
        ));
    }
    for frame in 0..48 {
        if frame == 24 {
            app.world_mut()
                .spawn(Readback::texture(target.clone()))
                .observe(
                    |event: On<ReadbackComplete>, mut capture: ResMut<Capture>| {
                        assert!(event.data.len() <= (WIDTH as usize * 8 + 256) * HEIGHT as usize);
                        capture.completed += 1;
                        capture.last.clone_from(&event.data);
                    },
                );
        }
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
    let capture = app.world().resource::<Capture>();
    assert!(
        capture.completed > 0,
        "no readback within deterministic frame cap"
    );
    assert!(capture.last.len().is_multiple_of(HEIGHT as usize));
    let stride = capture.last.len() / HEIGHT as usize;
    assert!(stride >= WIDTH as usize * 8);
    let expected = [
        [0.25, 0.5, 0.75],
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 0.0],
        [1.0; 3],
    ];
    let untinted_control = Color::srgb(0.25, 0.5, 0.75).to_linear();
    let mut checks = 0;
    for (column, rgb) in expected.into_iter().enumerate() {
        let linear = Color::srgb(rgb[0], rgb[1], rgb[2]).to_linear();
        for dy in [15, 16, 17] {
            for dx in [15, 16, 17] {
                let offset = dy * stride + (column * 32 + dx) * 8;
                for (channel, expected) in [linear.red, linear.green, linear.blue]
                    .into_iter()
                    .enumerate()
                {
                    let at = offset + channel * 2;
                    let actual = half(&capture.last[at..at + 2]);
                    assert!(
                        actual.is_finite() && (actual - expected).abs() < 0.002,
                        "column={column} channel={channel} expected={expected} actual={actual}"
                    );
                    if column == 3 {
                        let tinted = [
                            untinted_control.red,
                            untinted_control.green,
                            untinted_control.blue,
                        ][channel];
                        assert!(
                            (actual - tinted).abs() > 0.1,
                            "untinted control must differ from static tint"
                        );
                    }
                    checks += 1;
                }
            }
        }
    }
    println!(
        "colour-readback frames=48 completed={} pixels=36 rgb_checks={checks} target=Rgba16Float",
        capture.completed
    );
}

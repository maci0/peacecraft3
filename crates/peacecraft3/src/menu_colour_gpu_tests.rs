//! Synthetic render-graph check; original-client colour-space parity is separate.
use crate::{
    mainmenu_scene::apply_geoset_colour,
    menu_material::{MenuMaterial, NO_DEPTH_SET, NO_DEPTH_TEST, checked_source_mode},
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

const READBACK_WIDTH: u32 = 128;
const READBACK_HEIGHT: u32 = 32;
const QUAD_EXTENT: f32 = 0.75;

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

struct GpuReadback {
    bytes: Vec<u8>,
    stride: usize,
    completed: usize,
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

fn vulkan_menu_app(label: &str) -> App {
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
        "{label} adapter={:?}",
        **app.world().resource::<RenderAdapterInfo>()
    );
    app
}

fn spawn_readback_camera(app: &mut App) -> Handle<Image> {
    let mut image = Image::new_target_texture(
        READBACK_WIDTH,
        READBACK_HEIGHT,
        TextureFormat::Rgba16Float,
        None,
    );
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
    target
}

fn add_menu_material(
    materials: &mut Assets<MenuMaterial>,
    base_color: Color,
    alpha_mode: AlphaMode,
    filter_mode: u32,
    shading_flags: u32,
) -> Handle<MenuMaterial> {
    materials.add(
        checked_source_mode(
            StandardMaterial {
                base_color,
                unlit: true,
                fog_enabled: false,
                cull_mode: None,
                alpha_mode,
                ..default()
            },
            filter_mode,
            shading_flags,
        )
        .unwrap(),
    )
}

fn drive_readback(app: &mut App, target: Handle<Image>) -> GpuReadback {
    for frame in 0..48 {
        if frame == 24 {
            app.world_mut()
                .spawn(Readback::texture(target.clone()))
                .observe(
                    |event: On<ReadbackComplete>, mut capture: ResMut<Capture>| {
                        assert!(
                            event.data.len()
                                <= (READBACK_WIDTH as usize * 8 + 256) * READBACK_HEIGHT as usize
                        );
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
    assert!(capture.last.len().is_multiple_of(READBACK_HEIGHT as usize));
    let stride = capture.last.len() / READBACK_HEIGHT as usize;
    assert!(stride >= READBACK_WIDTH as usize * 8);
    GpuReadback {
        bytes: capture.last.clone(),
        stride,
        completed: capture.completed,
    }
}

fn channel_at(
    bytes: &[u8],
    stride: usize,
    column: usize,
    dx: usize,
    dy: usize,
    channel: usize,
) -> f32 {
    let column_px = READBACK_WIDTH as usize / 4;
    let offset = dy * stride + (column * column_px + dx) * 8 + channel * 2;
    half(&bytes[offset..offset + 2])
}

fn spawn_menu_quad(
    app: &mut App,
    mesh: &Handle<Mesh>,
    place: (usize, f32),
    source: (Color, AlphaMode, u32, u32),
) {
    let (column, z) = place;
    let (color, alpha_mode, filter_mode, shading_flags) = source;
    let handle = {
        let mut materials = app.world_mut().resource_mut::<Assets<MenuMaterial>>();
        add_menu_material(
            &mut materials,
            color,
            alpha_mode,
            filter_mode,
            shading_flags,
        )
    };
    app.world_mut().spawn((
        Mesh3d(mesh.clone()),
        MeshMaterial3d(handle),
        Transform::from_xyz(column as f32 - 1.5, 0.0, z),
    ));
}

#[test]
#[ignore = "requires a local Vulkan render adapter; synthetic GPU readback"]
fn authored_geoset_colour_reaches_the_menu_material_render_graph() {
    let mut app = vulkan_menu_app("colour-readback");
    let target = spawn_readback_camera(&mut app);
    let quad = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(Rectangle::new(QUAD_EXTENT, QUAD_EXTENT));
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
        let handle = {
            let mut materials = app.world_mut().resource_mut::<Assets<MenuMaterial>>();
            let handle = add_menu_material(
                &mut materials,
                Color::srgba(0.1, 0.2, 0.3, 0.7),
                AlphaMode::Opaque,
                0,
                0,
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
            handle
        };
        app.world_mut().spawn((
            Mesh3d(quad.clone()),
            MeshMaterial3d(handle),
            Transform::from_xyz(column as f32 - 1.5, 0.0, 0.0),
        ));
    }
    let readback = drive_readback(&mut app, target);
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
                for (channel, expected) in [linear.red, linear.green, linear.blue]
                    .into_iter()
                    .enumerate()
                {
                    let actual =
                        channel_at(&readback.bytes, readback.stride, column, dx, dy, channel);
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
        readback.completed
    );
}

#[test]
#[ignore = "requires a local Vulkan render adapter; synthetic GPU readback"]
fn depth_flags_change_overlapping_opaque_and_blend_visibility() {
    // Near red is opaque and spawned before far transparent blue.
    // Columns are control, farther 0x40, nearer 0x80, and nearer 0x40 only.
    // Column 1 is blue from the far quad's Always compare. The descriptor
    // keeps that blend write off. Column 3 is blue when the nearer 0x40 quad
    // leaves the cleared depth; a surviving opaque write leaves it red.
    let columns = [
        (0, 0),
        (0, NO_DEPTH_TEST),
        (NO_DEPTH_SET, 0),
        (NO_DEPTH_TEST, 0),
    ];
    let mut app = vulkan_menu_app("depth-readback");
    let target = spawn_readback_camera(&mut app);
    let quad = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(Rectangle::new(QUAD_EXTENT, QUAD_EXTENT));
    let near = Color::linear_rgb(1.0, 0.0, 0.0);
    let far = Color::linear_rgb(0.0, 0.0, 1.0);
    for (column, (near_flags, _)) in columns.iter().copied().enumerate() {
        spawn_menu_quad(
            &mut app,
            &quad,
            (column, 1.0),
            (near, AlphaMode::Opaque, 0, near_flags),
        );
    }
    for (column, (_, far_flags)) in columns.iter().copied().enumerate() {
        spawn_menu_quad(
            &mut app,
            &quad,
            (column, -1.0),
            (far, AlphaMode::Blend, 2, far_flags),
        );
    }
    let readback = drive_readback(&mut app, target);
    let expected = [
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
    ];
    let mut checks = 0;
    for (column, rgb) in expected.into_iter().enumerate() {
        for dy in [15, 16, 17] {
            for dx in [15, 16, 17] {
                for (channel, expected) in rgb.into_iter().enumerate() {
                    let actual =
                        channel_at(&readback.bytes, readback.stride, column, dx, dy, channel);
                    assert!(
                        actual.is_finite() && (actual - expected).abs() < 0.002,
                        "column={column} channel={channel} expected={expected} actual={actual}"
                    );
                    checks += 1;
                }
            }
        }
    }
    println!(
        "depth-readback frames=48 completed={} pixels=36 rgb_checks={checks} target=Rgba16Float",
        readback.completed
    );
}

#[test]
#[ignore = "requires a local Vulkan adapter; synthetic HUD texture readback"]
fn authored_hud_texture_ranges_and_alpha_reach_the_ui_render_graph() {
    use crate::hud_assets::{HudBudgets, HudInput, HudMarkup, NamedBytes, RootRole, SourceId};
    use crate::hud_render::{HudAnchor, HudMount, HudSubmit};

    let mut app = vulkan_menu_app("HUD texture ranges");
    crate::hud_render::install(&mut app);
    let target = spawn_readback_camera(&mut app);
    let camera = app
        .world_mut()
        .query_filtered::<Entity, With<Camera3d>>()
        .single(app.world())
        .unwrap();
    let parent = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            UiTargetCamera(camera),
        ))
        .id();
    app.world_mut().resource_mut::<HudAnchor>().parent = Some(parent);
    let mut fdf = String::from("Frame \"SIMPLEFRAME\" \"Console\" {\n");
    for (index, uv) in ["0,1,0,1", "1,0,0,1", "0,1,1,0", "1,0,1,0"]
        .iter()
        .enumerate()
    {
        fdf.push_str(&format!(
            "Texture {{\n File \"checker.tga\",\n Width 0.25,\n Height 1,\n TexCoord {uv},\n AlphaMode \"ALPHAKEY\",\n Anchor TOPLEFT,{},0,\n }}\n",
            index as f32 * 0.25,
        ));
    }
    fdf.push('}');
    let mut image = vec![0u8; 18];
    image[2] = 2;
    image[12..14].copy_from_slice(&2u16.to_le_bytes());
    image[14..16].copy_from_slice(&2u16.to_le_bytes());
    image[16] = 32;
    image[17] = 0x28;
    for bgra in [
        [0, 0, 255, 0],
        [0, 255, 0, 255],
        [255, 0, 0, 255],
        [255, 255, 255, 255],
    ] {
        image.extend(bgra);
    }
    app.world_mut().write_message(HudSubmit {
        input: HudInput {
            source: SourceId {
                identity: "synthetic-gpu".into(),
            },
            fdf_texts: vec![fdf],
            skin_text: String::new(),
            race_sections: vec![],
            roots: vec![RootRole {
                role: "console".into(),
                frame: "Console".into(),
            }],
            viewport: (1.0, 1.0),
            global_strings: String::new(),
            decorate_suffixes: vec![],
            images: vec![NamedBytes {
                path: "checker.tga".into(),
                bytes: image,
            }],
            fonts: vec![],
            budgets: HudBudgets {
                max_input_bytes: 4096,
                max_image_input_bytes: 64,
                max_font_bytes: 0,
                max_font_aggregate_bytes: 0,
                max_image_decoded_bytes: 16,
                max_decoded_bytes: 16,
                max_retained_bytes: 16,
                max_transient_bytes: 4112,
            },
            markup: HudMarkup::Plain,
            source_member: "synthetic.fdf".into(),
            receipt_path: String::new(),
            binding_revision: 0,
        },
    });
    let readback = drive_readback(&mut app, target);
    assert_eq!(app.world().resource::<HudMount>().generation, 1);
    let colours = [
        [0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [1.0, 1.0, 1.0],
    ];
    for column in 0..4 {
        for (pixel, (dx, dy)) in [(8, 8), (24, 8), (8, 24), (24, 24)].into_iter().enumerate() {
            let reflected = pixel ^ column;
            for (channel, expected) in colours[reflected].iter().enumerate() {
                let actual = channel_at(&readback.bytes, readback.stride, column, dx, dy, channel);
                assert!(
                    (actual - expected).abs() < 0.08,
                    "column={column} pixel={pixel} channel={channel}: {actual}"
                );
            }
        }
    }
    println!(
        "HUD texture checks=48 completed_readbacks={}",
        readback.completed
    );
}

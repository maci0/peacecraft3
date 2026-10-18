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
    let _ = bevy::log::tracing_subscriber::fmt()
        .with_max_level(bevy::log::Level::INFO)
        .with_test_writer()
        .try_init();
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
            .disable::<bevy::audio::AudioPlugin>()
            .disable::<bevy::gilrs::GilrsPlugin>(),
    )
    .add_plugins(crate::menu_composition::MenuCompositionPlugin)
    .add_plugins(crate::mainmenu_scene::ParticleMaterialPlugin)
    .add_plugins(MaterialPlugin::<MenuMaterial>::default())
    .init_resource::<crate::button_actions::ButtonActions>()
    .init_resource::<Capture>();
    crate::glue_menu::install(&mut app);
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
    let pixel_bytes = app
        .world()
        .resource::<Assets<Image>>()
        .get(&target)
        .unwrap()
        .texture_descriptor
        .format
        .block_copy_size(None)
        .unwrap() as usize;
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
    assert!(stride >= READBACK_WIDTH as usize * pixel_bytes);
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
    let untinted_control = Color::linear_rgb(0.25, 0.5, 0.75).to_linear();
    let mut checks = 0;
    for (column, rgb) in expected.into_iter().enumerate() {
        let linear = Color::linear_rgb(rgb[0], rgb[1], rgb[2]).to_linear();
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

#[test]
#[ignore = "requires a local Vulkan adapter; synthetic styled tooltip readback"]
fn authored_tooltip_spans_reach_the_owners_ui_render_target() {
    use crate::hud_assets::{HudMarkup, RelativeBox, sample_hud};
    use crate::hud_render::{HudAnchor, HudMount, HudSourceAttachment, HudSubmit};
    use crate::tooltip::{TooltipRecord, TooltipSupply, TooltipView};

    let mut input = sample_hud(&["Default"]);
    input.fdf_texts = vec![
        r#"
Frame "SIMPLEFRAME" "Console" {
    Frame "TEXT" "Owner" {
        Width 1,
        Height 1,
        SetPoint TOPLEFT, "Console", TOPLEFT, 0, 0,
        FrameFont "MasterFont", 0.3, "",
        FontColor 1 1 1 1,
        FontHighlightColor 1 1 1 1,
        FontDisabledColor 0.5 0.5 0.5 1,
        FontShadowColor 0 0 0 0,
        FontShadowOffset 0 0,
        FontFlags "",
        FontJustificationH JUSTIFYLEFT,
        FontJustificationV JUSTIFYTOP,
        Text "",
    }
}
"#
        .into(),
    ];
    input.roots.truncate(1);
    input.viewport = (1.0, 1.0);
    input.images.clear();
    input.markup = HudMarkup::Adapter(crate::tooltip::hud_markup_adapter);
    let style = crate::hud_assets::resolve(&input)
        .unwrap()
        .frames
        .into_iter()
        .find(|frame| frame.name == "Owner")
        .unwrap()
        .text
        .unwrap();
    let budget = input.budgets.max_input_bytes;
    let mut app = vulkan_menu_app("Tooltip spans");
    crate::hud_render::install(&mut app);
    crate::tooltip::install(&mut app);
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
    app.world_mut().write_message(HudSubmit { input });
    app.update();
    assert!(app.world().resource::<HudMount>().error.is_none());
    let owner = app
        .world_mut()
        .query::<(Entity, &HudSourceAttachment)>()
        .iter(app.world())
        .find(|(_, source)| source.frame == "Owner")
        .map(|(entity, _)| entity)
        .unwrap();
    let attachment = app
        .world()
        .get::<HudSourceAttachment>(owner)
        .unwrap()
        .clone();
    let text = app.world().get::<Children>(owner).unwrap()[0];
    let bevy::text::FontSource::Handle(font) = &app.world().get::<TextFont>(text).unwrap().font
    else {
        panic!("expected resolved font handle")
    };
    let font = font.clone();
    app.insert_resource(TooltipSupply {
        field: "Tip".into(),
        record: TooltipRecord {
            source_label: attachment.source_label,
            source_member: attachment.source_member,
            frame: attachment.frame,
            receipt_path: attachment.receipt_path,
            binding_revision: attachment.binding_revision,
            field: "Tip".into(),
            text: "|cffff0000MM|r|n|cff00ff00MM|r".into(),
        },
        font,
        style,
        placement: Some(RelativeBox {
            left: 0.0,
            top: 0.0,
            width: 100.0,
            height: 100.0,
        }),
        max_input_bytes: budget,
    });
    app.insert_resource(bevy::input_focus::InputFocus::from_entity(owner));
    app.insert_resource(bevy::input_focus::InputFocusVisible(true));
    let readback = drive_readback(&mut app, target);
    let view = app.world().resource::<TooltipView>().0;
    assert_eq!(
        app.world().get::<ChildOf>(view).unwrap().parent(),
        app.world().resource::<HudMount>().root.unwrap()
    );
    let mut red = Vec::new();
    let mut green = Vec::new();
    for y in 0..READBACK_HEIGHT as usize {
        for x in 0..READBACK_WIDTH as usize {
            let offset = y * readback.stride + x * 8;
            let r = half(&readback.bytes[offset..offset + 2]);
            let g = half(&readback.bytes[offset + 2..offset + 4]);
            let b = half(&readback.bytes[offset + 4..offset + 6]);
            if r > 0.2 && g < 0.05 && b < 0.05 {
                red.push(y);
            }
            if g > 0.2 && r < 0.05 && b < 0.05 {
                green.push(y);
            }
        }
    }
    assert!(red.len() > 3, "red glyph pixels={}", red.len());
    assert!(green.len() > 3, "green glyph pixels={}", green.len());
    assert!(
        red.iter().max().unwrap() < green.iter().min().unwrap(),
        "newline did not separate the colour runs"
    );
    println!(
        "tooltip readback red_pixels={} green_pixels={} completed={}",
        red.len(),
        green.len(),
        readback.completed
    );
}

// Synthetic channel/alpha values; no game content or screenshot measurements.
const COMPOSITION_BG: [u8; 4] = [32, 96, 160, 255];
const COMPOSITION_PANEL: [u8; 3] = [196, 84, 156];
const COMPOSITION_TINT: [f32; 4] = [0.625, 0.375, 0.75, 0.7];
const COMPOSITION_PARTICLE: [u8; 4] = [64, 192, 96, 128];
const COMPOSITION_PARTICLE_TINT: [f32; 4] = [0.25, 0.75, 0.5, 0.5];
const COMPOSITION_UI: [u8; 4] = [212, 64, 172, 128];
const COMPOSITION_UI_TINT: [f32; 4] = [0.75, 0.5, 0.25, 0.5];

fn composition_readback(raw: bool) -> GpuReadback {
    use crate::{mainmenu_scene::ParticleMaterial, menu_composition};
    use bevy::camera::visibility::RenderLayers;
    let mut app = vulkan_menu_app(if raw {
        "menu working channels"
    } else {
        "linear control"
    });
    let mut output = Image::new_target_texture(
        READBACK_WIDTH,
        READBACK_HEIGHT,
        TextureFormat::Rgba8UnormSrgb,
        None,
    );
    output.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let target = app.world_mut().resource_mut::<Assets<Image>>().add(output);
    for (order, layer) in [(0, 1), (1, 2)] {
        let mut camera = app.world_mut().spawn((
            Camera3d::default(),
            Camera {
                order,
                clear_color: if order == 0 {
                    ClearColorConfig::Custom(Color::BLACK)
                } else {
                    ClearColorConfig::None
                },
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
            Tonemapping::None,
            DebandDither::Disabled,
            Msaa::Off,
            RenderLayers::layer(layer),
        ));
        if raw {
            camera.insert(menu_composition::MenuColourCamera);
        }
    }
    let camera = {
        let mut entity = app.world_mut().spawn((
            Camera2d,
            Camera {
                order: 2,
                clear_color: ClearColorConfig::None,
                ..default()
            },
            RenderTarget::Image(target.clone().into()),
            Msaa::Off,
        ));
        if raw {
            entity.insert(menu_composition::MenuColourCamera);
        }
        entity.id()
    };
    let image = |world: &mut World, rgba: [u8; 4]| {
        let texture = pcx_gfx::LoadedTexture {
            source: "synthetic.tga".into(),
            width: 1,
            height: 1,
            rgba: rgba.to_vec(),
        };
        let image = if raw {
            menu_composition::image(&texture)
        } else {
            pcx_gfx::viewer::rgba_image(&texture)
        };
        world.resource_mut::<Assets<Image>>().add(image)
    };
    let colour = |rgba: [f32; 4]| {
        if raw {
            menu_composition::colour(rgba)
        } else {
            Color::srgba(rgba[0], rgba[1], rgba[2], rgba[3])
        }
    };
    let background = image(app.world_mut(), COMPOSITION_BG);
    let panel_mesh = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(Rectangle::new(0.75, 1.0));
    let background_mesh = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(Rectangle::new(4.0, 1.0));
    let material = |world: &mut World, texture, tint, alpha_mode, mode| {
        world.resource_mut::<Assets<MenuMaterial>>().add(
            checked_source_mode(
                StandardMaterial {
                    base_color_texture: Some(texture),
                    base_color: colour(tint),
                    alpha_mode,
                    unlit: true,
                    fog_enabled: false,
                    cull_mode: None,
                    ..default()
                },
                mode,
                0,
            )
            .unwrap(),
        )
    };
    let background = material(app.world_mut(), background, [1.0; 4], AlphaMode::Opaque, 0);
    app.world_mut().spawn((
        Mesh3d(background_mesh),
        MeshMaterial3d(background),
        RenderLayers::layer(1),
    ));
    for (column, alpha) in [0, 64, 128, 255].into_iter().enumerate() {
        let texture = image(
            app.world_mut(),
            [
                COMPOSITION_PANEL[0],
                COMPOSITION_PANEL[1],
                COMPOSITION_PANEL[2],
                alpha,
            ],
        );
        let handle = material(
            app.world_mut(),
            texture,
            COMPOSITION_TINT,
            AlphaMode::Blend,
            2,
        );
        app.world_mut().spawn((
            Mesh3d(panel_mesh.clone()),
            MeshMaterial3d(handle),
            Transform::from_xyz(column as f32 - 1.5, 0.0, 0.0),
            RenderLayers::layer(2),
        ));
    }
    let particle_image = image(app.world_mut(), COMPOSITION_PARTICLE);
    let particle = app
        .world_mut()
        .resource_mut::<Assets<ParticleMaterial>>()
        .add(ParticleMaterial {
            base: StandardMaterial {
                base_color: colour(COMPOSITION_PARTICLE_TINT),
                base_color_texture: Some(particle_image),
                alpha_mode: AlphaMode::Add,
                unlit: true,
                fog_enabled: false,
                cull_mode: None,
                ..default()
            },
            ..default()
        });
    let mesh = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(Rectangle::new(4.0, 6.0 / READBACK_HEIGHT as f32));
    app.world_mut().spawn((
        Mesh3d(mesh),
        MeshMaterial3d(particle),
        Transform::from_xyz(0.0, 0.5 - 9.0 / READBACK_HEIGHT as f32, 0.2),
        RenderLayers::layer(2),
    ));
    let ui_image = image(app.world_mut(), COMPOSITION_UI);
    app.world_mut().spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(12.0),
            width: Val::Px(READBACK_WIDTH as f32),
            height: Val::Px(6.0),
            ..default()
        },
        UiTargetCamera(camera),
        ImageNode::new(ui_image.clone())
            .with_mode(bevy::ui::widget::NodeImageMode::Stretch)
            .with_color(colour(COMPOSITION_UI_TINT)),
    ));
    crate::glue_menu::composition_test_nodes(
        app.world_mut(),
        camera,
        ui_image,
        READBACK_WIDTH as f32,
        18.0,
        6.0,
    );
    drive_readback(&mut app, target)
}

fn composition_expected(raw: bool, column: usize, row: usize) -> [u8; 3] {
    let decode = |value: f32| {
        if raw {
            value
        } else {
            Color::srgb(value, 0.0, 0.0).to_linear().red
        }
    };
    let encode = |value: f32| {
        if raw {
            value
        } else {
            Color::linear_rgb(value, 0.0, 0.0).to_srgba().red
        }
    };
    let mut destination = [COMPOSITION_BG[0], COMPOSITION_BG[1], COMPOSITION_BG[2]];
    let mut blend = |source: [u8; 4], tint: [f32; 4], add| {
        let alpha = f32::from(source[3]) / 255.0 * tint[3];
        for channel in 0..3 {
            let src = decode(f32::from(source[channel]) / 255.0) * decode(tint[channel]);
            let dst = decode(f32::from(destination[channel]) / 255.0);
            let mixed = src * alpha + dst * if add { 1.0 } else { 1.0 - alpha };
            destination[channel] = (encode(mixed).clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    };
    blend(
        [
            COMPOSITION_PANEL[0],
            COMPOSITION_PANEL[1],
            COMPOSITION_PANEL[2],
            [0, 64, 128, 255][column],
        ],
        COMPOSITION_TINT,
        false,
    );
    match row {
        0 => (),
        1 => blend(COMPOSITION_PARTICLE, COMPOSITION_PARTICLE_TINT, true),
        2 => blend(COMPOSITION_UI, COMPOSITION_UI_TINT, false),
        3 => blend(COMPOSITION_UI, [1.0; 4], false),
        4 => blend(COMPOSITION_UI, [1.0; 4], true),
        _ => unreachable!(),
    }
    destination
}

#[test]
#[ignore = "requires a local Vulkan adapter; synthetic legacy and linear framebuffer readbacks"]
fn menu_geometry_particles_and_ui_share_the_same_working_channels() {
    let mut differences = 0;
    for raw in [false, true] {
        let readback = composition_readback(raw);
        for column in 0..4 {
            for row in 0..5 {
                let expected = composition_expected(raw, column, row);
                let other = composition_expected(!raw, column, row);
                for channel in 0..3 {
                    let offset = (row * 6 + 3) * readback.stride + (column * 32 + 16) * 4 + channel;
                    let actual = readback.bytes[offset];
                    assert!(
                        actual.abs_diff(expected[channel]) <= 2,
                        "raw={raw} column={column} row={row} channel={channel}: {actual} != {}",
                        expected[channel]
                    );
                    differences += usize::from(actual.abs_diff(other[channel]) > 2);
                }
            }
        }
    }
    assert!(
        differences > 0,
        "linear and encoded working-space controls must differ"
    );
    println!(
        "composition readbacks=2 rgb_checks=120 different_working_channel_checks={differences}"
    );
}

#[test]
#[ignore = "requires a local Vulkan render adapter; synthetic GPU readback"]
fn menu_fog_uses_camera_depth_for_geometry_and_particles() {
    use crate::mainmenu_scene::ParticleMaterial;
    use bevy::pbr::{DistanceFog, FogFalloff};
    let mut app = vulkan_menu_app("camera-depth-fog");
    let target = spawn_readback_camera(&mut app);
    let camera_transform =
        Transform::from_xyz(7.0, 5.0, 9.0).looking_at(Vec3::new(1.0, 2.0, 1.0), Vec3::Y);
    let camera = app
        .world_mut()
        .query_filtered::<Entity, With<Camera3d>>()
        .single(app.world())
        .unwrap();
    let source = [0.2, 0.4, 0.7, 0.6];
    let fog = [0.8, 0.1, 0.3, 0.5];
    app.world_mut().entity_mut(camera).insert((
        camera_transform,
        DistanceFog {
            color: Color::linear_rgba(fog[0], fog[1], fog[2], fog[3]),
            falloff: FogFalloff::Linear {
                start: 2.0,
                end: 6.0,
            },
            directional_light_color: Color::NONE,
            directional_light_exponent: 0.0,
        },
    ));
    let quad = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(Rectangle::new(QUAD_EXTENT, QUAD_EXTENT));
    for column in 0..4 {
        let base = StandardMaterial {
            base_color: Color::linear_rgba(source[0], source[1], source[2], source[3]),
            unlit: true,
            fog_enabled: column != 3,
            cull_mode: None,
            alpha_mode: if column == 1 || column == 2 {
                AlphaMode::Add
            } else {
                AlphaMode::Opaque
            },
            ..default()
        };
        let transform = Transform {
            translation: camera_transform.transform_point(Vec3::new(
                column as f32 - 1.5,
                0.0,
                -4.0,
            )),
            rotation: camera_transform.rotation,
            ..default()
        };
        if column == 2 {
            let material = app
                .world_mut()
                .resource_mut::<Assets<ParticleMaterial>>()
                .add(ParticleMaterial {
                    base,
                    extension: default(),
                });
            app.world_mut()
                .spawn((Mesh3d(quad.clone()), MeshMaterial3d(material), transform));
        } else {
            let material = app
                .world_mut()
                .resource_mut::<Assets<MenuMaterial>>()
                .add(checked_source_mode(base, if column == 1 { 4 } else { 0 }, 0).unwrap());
            app.world_mut()
                .spawn((Mesh3d(quad.clone()), MeshMaterial3d(material), transform));
        }
    }
    let readback = drive_readback(&mut app, target);
    let mut radial_discriminators = 0;
    for column in 0..4 {
        for dx in [14, 18] {
            for dy in [15, 16] {
                let x = -2.0 + (column * 32 + dx) as f32 / 32.0 + 0.5 / 32.0;
                let y = 0.5 - (dy as f32 + 0.5) / 32.0;
                let radial_fog = ((16.0 + x * x + y * y).sqrt() - 2.0) / 4.0 * fog[3];
                for channel in 0..3 {
                    let fog_channel = if column == 2 { 0.0 } else { fog[channel] };
                    let blend = |amount: f32| {
                        let value = source[channel] * (1.0 - amount) + fog_channel * amount;
                        if column == 1 || column == 2 {
                            value * source[3]
                        } else {
                            value
                        }
                    };
                    let expected = if column == 3 {
                        source[channel]
                    } else {
                        blend(0.25)
                    };
                    let actual =
                        channel_at(&readback.bytes, readback.stride, column, dx, dy, channel);
                    assert!(
                        (actual - expected).abs() < 0.003,
                        "column={column} sample={dx},{dy} channel={channel}: {actual} != {expected}"
                    );
                    if column != 3 && (blend(radial_fog) - expected).abs() > 0.006 {
                        assert!((actual - blend(radial_fog)).abs() > 0.003);
                        radial_discriminators += 1;
                    }
                }
            }
        }
    }
    assert!(radial_discriminators >= 4);
    println!("camera_depth_fog rgb_checks=48 radial_discriminators={radial_discriminators}");
}

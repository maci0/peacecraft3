//! Cell-edge and minification readback for the admitted ground sheet.
//! The ignored test needs a local Vulkan adapter. The other tests do not.

use std::time::Duration;

use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler};
use bevy::mesh::VertexAttributeValues;
use bevy::prelude::*;
use bevy::render::render_resource::{
    TextureDataOrder, TextureDimension, TextureFormat, TextureViewDimension,
};
use pcx_gfx::ground::{GROUND_SELECTORS, GroundError, spawn_admitted_ground};
use pcx_gfx::{GroundMemory, SelectedGroundBlp, decode_ground_slots};
use pcx_map::Terrain;
use pcx_tera::GroundSelectError;

const READBACK_WIDTH: u32 = 128;
const READBACK_HEIGHT: u32 = 64;

fn one_tile(slots: &[[u8; 4]], nibbles: [u8; 4], byte5: u8) -> Terrain {
    let mut bytes = pcx_map::TERRAIN_MAGIC.to_vec();
    bytes.extend(pcx_map::TERRAIN_VERSION.to_le_bytes());
    bytes.push(0);
    bytes.extend(0u32.to_le_bytes());
    bytes.extend((slots.len() as u32).to_le_bytes());
    for slot in slots {
        bytes.extend_from_slice(slot);
    }
    bytes.extend(0u32.to_le_bytes());
    bytes.extend(2u32.to_le_bytes());
    bytes.extend(2u32.to_le_bytes());
    bytes.extend(0.0f32.to_le_bytes());
    bytes.extend(0.0f32.to_le_bytes());
    for (index, nibble) in nibbles.iter().enumerate() {
        bytes.extend(8192u16.to_le_bytes());
        bytes.extend(0u16.to_le_bytes());
        bytes.push(*nibble);
        bytes.push(if index == 0 { byte5 } else { 0 });
        bytes.push(0);
    }
    Terrain::from_bytes(bytes).unwrap()
}

fn direct_blp(width: u32, height: u32, indices: Option<&[u8]>) -> Vec<u8> {
    let payload_at = 0x9C + 256 * 4;
    let payload_len = indices.map(|texels| texels.len()).unwrap_or(0);
    let mut bytes = vec![
        0u8;
        if indices.is_some() {
            payload_at + payload_len
        } else {
            0x9C
        }
    ];
    bytes[0..4].copy_from_slice(b"BLP1");
    bytes[4..8].copy_from_slice(&1u32.to_le_bytes());
    bytes[0x0C..0x10].copy_from_slice(&width.to_le_bytes());
    bytes[0x10..0x14].copy_from_slice(&height.to_le_bytes());
    bytes[0x1C..0x20].copy_from_slice(&(payload_at as u32).to_le_bytes());
    let declared = indices.map(|texels| texels.len()).unwrap_or(1024);
    bytes[0x5C..0x60].copy_from_slice(&(declared as u32).to_le_bytes());
    if let Some(indices) = indices {
        let mut palette = [[0u8; 4]; 256];
        palette[0] = [0, 255, 0, 255];
        palette[1] = [0, 0, 255, 255];
        palette[2] = [255, 0, 0, 255];
        for (index, entry) in palette.iter().enumerate() {
            let at = 0x9C + index * 4;
            bytes[at..at + 4].copy_from_slice(entry);
        }
        bytes[payload_at..].copy_from_slice(indices);
    }
    bytes
}

fn variation_sheet() -> Vec<u8> {
    let mut indices = vec![0u8; 32 * 32];
    for y in 0..8 {
        for x in 8..16 {
            indices[y * 32 + x] = 2;
        }
    }
    for y in 3..5 {
        for x in 3..5 {
            indices[y * 32 + x] = 1;
        }
    }
    direct_blp(32, 32, Some(&indices))
}

fn slot<'a>(fourcc: [u8; 4], source: &'a str, bytes: &'a [u8]) -> SelectedGroundBlp<'a> {
    SelectedGroundBlp {
        fourcc,
        source,
        bytes,
    }
}

fn memory(retained_bytes: usize, transient_bytes: usize) -> GroundMemory {
    GroundMemory {
        retained_bytes,
        transient_bytes,
    }
}

fn empty_assets() -> (Assets<Mesh>, Assets<Image>, Assets<pcx_gfx::GroundMaterial>) {
    (Assets::default(), Assets::default(), Assets::default())
}

fn assert_unpublished(
    images: &Assets<Image>,
    meshes: &Assets<Mesh>,
    materials: &Assets<pcx_gfx::GroundMaterial>,
) {
    assert!(images.iter().next().is_none());
    assert!(meshes.iter().next().is_none());
    assert!(materials.iter().next().is_none());
}

#[test]
fn ground_texture_budget_rejects_before_decode() {
    let bytes = direct_blp(32, 32, None);
    assert!(pcx_blp::Header::parse(&bytes).is_ok());
    let error = decode_ground_slots(
        &[slot(*b"Ldrt", "synthetic/truncated.blp", &bytes)],
        memory(5439, 8192),
    )
    .unwrap_err();
    assert!(
        matches!(
            error,
            GroundError::RetainedBytes {
                needed: 5440,
                limit: 5439,
                ..
            }
        ),
        "{error:?}"
    );
    let transient = decode_ground_slots(
        &[slot(*b"Ldrt", "synthetic/truncated.blp", &bytes)],
        memory(5440, 8191),
    )
    .unwrap_err();
    assert!(
        matches!(
            transient,
            GroundError::TransientBytes {
                needed: 8192,
                limit: 8191,
                ..
            }
        ),
        "{transient:?}"
    );

    let terrain = one_tile(&[*b"Ldrt"], [0, 0, 0, 0], 0);
    let (mut meshes, mut images, mut materials) = empty_assets();
    let spawned = spawn_admitted_ground(
        &terrain,
        &[slot(*b"Ldrt", "synthetic/truncated.blp", &bytes)],
        memory(5439, 8192),
        None,
        &mut meshes,
        &mut images,
        &mut materials,
    )
    .unwrap_err();
    assert!(matches!(
        spawned,
        GroundError::RetainedBytes { needed: 5440, .. }
    ));
    assert_unpublished(&images, &meshes, &materials);

    let (mut meshes, mut images, mut materials) = empty_assets();
    let missing = spawn_admitted_ground(
        &terrain,
        &[],
        memory(5440, 8192),
        None,
        &mut meshes,
        &mut images,
        &mut materials,
    )
    .unwrap_err();
    assert_eq!(missing, GroundError::MissingSlots { expected: 1 });
    assert_unpublished(&images, &meshes, &materials);
}

#[test]
fn admitted_ground_keeps_selector_order_and_publishes_atomically() {
    let sheet = {
        let indices = vec![0u8; 4 * 4];
        direct_blp(4, 4, Some(&indices))
    };
    let terrain = one_tile(&[*b"AAAA", *b"BBBB"], [1, 0, 1, 0], 0);
    let (mut meshes, mut images, mut materials) = empty_assets();
    let draw = spawn_admitted_ground(
        &terrain,
        &[
            slot(*b"AAAA", "synthetic/a.blp", &sheet),
            slot(*b"BBBB", "synthetic/b.blp", &sheet),
        ],
        memory(128, 128),
        None,
        &mut meshes,
        &mut images,
        &mut materials,
    )
    .unwrap();
    let mesh = meshes.get(draw.mesh.as_ref().unwrap()).unwrap();
    let VertexAttributeValues::Uint32x4(selectors) = mesh.attribute(GROUND_SELECTORS).unwrap()
    else {
        panic!("selector attribute");
    };
    assert_eq!(selectors.len(), 4);
    assert!(
        selectors
            .iter()
            .all(|value| *value == [0, 1 + (0b1010 << 5), 17, 17])
    );
    let material = materials.get(draw.material.as_ref().unwrap()).unwrap();
    assert_eq!(material.layer_base()[0], 0);
    assert_eq!(material.layer_base()[1], 16);
    assert!(!draw.is_empty());

    let bad = one_tile(&[*b"Ldrt"], [1, 0, 0, 0], 0);
    let (mut meshes, mut images, mut materials) = empty_assets();
    let error = spawn_admitted_ground(
        &bad,
        &[slot(*b"Ldrt", "synthetic/a.blp", &sheet)],
        memory(64, 128),
        None,
        &mut meshes,
        &mut images,
        &mut materials,
    )
    .unwrap_err();
    assert_eq!(
        error,
        GroundError::Select(GroundSelectError::SlotRange {
            nibble: 1,
            slot_count: 1
        })
    );
    assert_unpublished(&images, &meshes, &materials);

    let short = direct_blp(4, 3, None);
    let (mut meshes, mut images, mut materials) = empty_assets();
    let error = spawn_admitted_ground(
        &one_tile(&[*b"AAAA", *b"BBBB"], [0, 0, 0, 0], 0),
        &[
            slot(*b"AAAA", "synthetic/narrow.blp", &sheet),
            slot(*b"BBBB", "synthetic/short.blp", &short),
        ],
        memory(10_000, 10_000),
        None,
        &mut meshes,
        &mut images,
        &mut materials,
    )
    .unwrap_err();
    assert!(
        matches!(
            error,
            GroundError::SheetGrid {
                width: 4,
                height: 3,
                ..
            }
        ),
        "{error:?}"
    );
    assert_unpublished(&images, &meshes, &materials);

    let bigger = direct_blp(8, 8, Some(&[0u8; 8 * 8]));
    let (mut meshes, mut images, mut materials) = empty_assets();
    let error = spawn_admitted_ground(
        &one_tile(&[*b"AAAA", *b"BBBB"], [0, 0, 0, 0], 0),
        &[
            slot(*b"AAAA", "synthetic/narrow.blp", &sheet),
            slot(*b"BBBB", "synthetic/bigger.blp", &bigger),
        ],
        memory(10_000, 10_000),
        None,
        &mut meshes,
        &mut images,
        &mut materials,
    )
    .unwrap_err();
    assert!(
        matches!(
            error,
            GroundError::MixedTileExtent {
                expected: 1,
                actual: 2,
                ..
            }
        ),
        "{error:?}"
    );
    assert_unpublished(&images, &meshes, &materials);
}

#[test]
fn admitted_sheet_is_a_clamped_linear_mip_array() {
    let bytes = variation_sheet();
    let terrain = one_tile(&[*b"Ldrt"], [0, 0, 0, 0], 0);
    let (mut meshes, mut images, mut materials) = empty_assets();
    let draw = spawn_admitted_ground(
        &terrain,
        &[slot(*b"Ldrt", "synthetic/variation.blp", &bytes)],
        memory(5440, 8192),
        None,
        &mut meshes,
        &mut images,
        &mut materials,
    )
    .unwrap();
    let mesh = meshes.get(draw.mesh.as_ref().unwrap()).unwrap();
    let VertexAttributeValues::Float32x3(positions) =
        mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap()
    else {
        panic!("positions");
    };
    assert_eq!(
        positions,
        &[
            [0.0, 0.0, 0.0],
            [128.0, 0.0, 0.0],
            [0.0, 0.0, -128.0],
            [128.0, 0.0, -128.0],
        ]
    );
    let edge_a = [
        positions[1][0] - positions[0][0],
        positions[1][1] - positions[0][1],
        positions[1][2] - positions[0][2],
    ];
    let edge_b = [
        positions[3][0] - positions[0][0],
        positions[3][1] - positions[0][1],
        positions[3][2] - positions[0][2],
    ];
    let normal = [
        edge_a[1] * edge_b[2] - edge_a[2] * edge_b[1],
        edge_a[2] * edge_b[0] - edge_a[0] * edge_b[2],
        edge_a[0] * edge_b[1] - edge_a[1] * edge_b[0],
    ];
    assert!(normal[1] > 0.0 && normal[0].abs() < 1.0e-3 && normal[2].abs() < 1.0e-3);
    let VertexAttributeValues::Float32x2(uvs) = mesh.attribute(Mesh::ATTRIBUTE_UV_0).unwrap()
    else {
        panic!("uvs");
    };
    assert_eq!(uvs, &[[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]]);
    match mesh.indices().unwrap() {
        bevy::mesh::Indices::U32(indices) => assert_eq!(indices, &[0, 1, 3, 0, 3, 2]),
        other => panic!("{other:?}"),
    }
    let material = materials.get(draw.material.as_ref().unwrap()).unwrap();
    let image = images.get(&material.texture).unwrap();
    assert_eq!(image.texture_descriptor.format, TextureFormat::Rgba8Unorm);
    assert_eq!(image.texture_descriptor.dimension, TextureDimension::D2);
    assert_eq!(image.texture_descriptor.mip_level_count, 4);
    assert_eq!(image.texture_descriptor.size.width, 8);
    assert_eq!(image.texture_descriptor.size.height, 8);
    assert_eq!(image.texture_descriptor.size.depth_or_array_layers, 16);
    assert_eq!(image.data_order, TextureDataOrder::LayerMajor);
    let view = image.texture_view_descriptor.as_ref().unwrap();
    assert_eq!(view.dimension, Some(TextureViewDimension::D2Array));
    let ImageSampler::Descriptor(sampler) = &image.sampler else {
        panic!("sampler stayed at the nearest default");
    };
    assert_eq!(sampler.mag_filter, ImageFilterMode::Linear);
    assert_eq!(sampler.min_filter, ImageFilterMode::Linear);
    assert_eq!(sampler.mipmap_filter, ImageFilterMode::Linear);
    assert_eq!(sampler.address_mode_u, ImageAddressMode::ClampToEdge);
    assert_eq!(sampler.address_mode_v, ImageAddressMode::ClampToEdge);
    assert_eq!(sampler.address_mode_w, ImageAddressMode::ClampToEdge);
    let data = image.data.as_ref().unwrap();
    assert_eq!(&data[108..112], &[255, 0, 0, 255], "cell center stays red");
    let level3 = (8 * 8 + 4 * 4 + 2 * 2) * 4;
    assert_eq!(&data[level3..level3 + 4], &[16, 239, 0, 255]);
    let layer1 = (8 * 8 + 4 * 4 + 2 * 2 + 1) * 4;
    assert_eq!(&data[layer1..layer1 + 4], &[0, 0, 255, 255]);
    assert!(!draw.is_empty());
}

#[test]
#[ignore = "requires a local Vulkan render adapter; synthetic GPU readback"]
fn ground_edge_and_minification_stay_inside_the_variation_cell() {
    use bevy::asset::AssetPlugin;
    use bevy::camera::{Hdr, RenderTarget, ScalingMode};
    use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
    use bevy::render::RenderPlugin;
    use bevy::render::gpu_readback::{Readback, ReadbackComplete};
    use bevy::render::render_resource::{PollType, TextureUsages};
    use bevy::render::renderer::{RenderAdapterInfo, RenderDevice};
    use bevy::render::settings::{Backends, RenderCreation, WgpuSettings};
    use bevy::window::ExitCondition;
    use bevy::winit::WinitPlugin;

    #[derive(Resource, Default)]
    struct Capture {
        completed: usize,
        last: Vec<u8>,
    }

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..default()
            })
            .set(AssetPlugin {
                file_path: env!("CARGO_MANIFEST_DIR").into(),
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
    .add_plugins(pcx_gfx::GroundMaterialPlugin)
    .init_resource::<Capture>();
    app.finish();
    app.cleanup();
    println!(
        "ground-readback adapter={:?}",
        **app.world().resource::<RenderAdapterInfo>()
    );

    let bytes = variation_sheet();
    let terrain = one_tile(&[*b"Ldrt"], [0, 0, 0, 0], 0);
    let mut meshes = app.world_mut().remove_resource::<Assets<Mesh>>().unwrap();
    let mut images = app.world_mut().remove_resource::<Assets<Image>>().unwrap();
    let mut materials = app
        .world_mut()
        .remove_resource::<Assets<pcx_gfx::GroundMaterial>>()
        .unwrap();
    let draw = spawn_admitted_ground(
        &terrain,
        &[slot(*b"Ldrt", "synthetic/variation.blp", &bytes)],
        memory(5440, 8192),
        Some(&app.world().resource::<RenderDevice>().limits()),
        &mut meshes,
        &mut images,
        &mut materials,
    )
    .unwrap();
    let material = draw.material.clone().unwrap();
    let selectors = {
        let mesh = meshes.get(draw.mesh.as_ref().unwrap()).unwrap();
        let VertexAttributeValues::Uint32x4(values) = mesh.attribute(GROUND_SELECTORS).unwrap()
        else {
            panic!("selectors");
        };
        values[0]
    };
    // Ortho is 4 by 2 over 128 by 64, so one pixel is 0.03125 world units.
    // Image row 0 is world +Y. The large quad keeps implicit LOD 0: its center
    // is the red texels and a pixel near u=1 stays on the green edge. The
    // second quad is one pixel, UVs span the whole cell, and its center is off
    // the shared diagonal so that pixel is covered.
    let wide = meshes.add(full_uv_quad([-1.0, 0.0, 0.0], [0.9, 0.9], selectors));
    let pixel = meshes.add(full_uv_quad(
        [1.019625, -0.015625, 0.0],
        [0.015625, 0.015625],
        selectors,
    ));
    app.world_mut().insert_resource(meshes);
    app.world_mut().insert_resource(images);
    app.world_mut().insert_resource(materials);

    let mut target_image = Image::new_target_texture(
        READBACK_WIDTH,
        READBACK_HEIGHT,
        TextureFormat::Rgba16Float,
        None,
    );
    target_image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let target = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .add(target_image);
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
                height: 2.0,
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
    app.world_mut()
        .spawn((Mesh3d(wide), MeshMaterial3d(material.clone())));
    app.world_mut()
        .spawn((Mesh3d(pixel), MeshMaterial3d(material)));

    for frame in 0..48 {
        if frame == 24 {
            app.world_mut()
                .spawn(Readback::texture(target.clone()))
                .observe(
                    |event: On<ReadbackComplete>, mut capture: ResMut<Capture>| {
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
    assert!(capture.completed > 0, "no readback within the frame cap");
    let stride = capture.last.len() / READBACK_HEIGHT as usize;
    assert!(stride >= READBACK_WIDTH as usize * 8);
    let center = sample(&capture.last, stride, 33, 32);
    let edge = sample(&capture.last, stride, 59, 32);
    let minified = sample(&capture.last, stride, 96, 32);
    assert!(
        center[0] > 0.9 && center[1] < 0.05 && center[2] < 0.05 && center[3] > 0.9,
        "{center:?}"
    );
    assert!(
        edge[0] < 0.02 && edge[1] > 0.98 && edge[2] < 0.02 && edge[3] > 0.9,
        "{edge:?}"
    );
    let tolerance = 2.0 / 255.0;
    assert!(
        (minified[0] - 16.0 / 255.0).abs() < tolerance,
        "{minified:?}"
    );
    assert!(
        (minified[1] - 239.0 / 255.0).abs() < tolerance,
        "{minified:?}"
    );
    assert!(
        minified[2].abs() < tolerance && minified[3] > 0.9,
        "{minified:?}"
    );
    assert!((edge[1] - minified[1]).abs() > 0.04);
}

fn full_uv_quad(center: [f32; 3], half: [f32; 2], selectors: [u32; 4]) -> Mesh {
    let [x, y, z] = center;
    let [hx, hy] = half;
    Mesh::new(
        bevy::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![
            [x - hx, y - hy, z],
            [x + hx, y - hy, z],
            [x - hx, y + hy, z],
            [x + hx, y + hy, z],
        ],
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_UV_0,
        vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]],
    )
    .with_inserted_attribute(GROUND_SELECTORS, vec![selectors; 4])
    .with_inserted_indices(bevy::mesh::Indices::U32(vec![0, 1, 3, 0, 3, 2]))
}

fn sample(bytes: &[u8], stride: usize, x: usize, y: usize) -> [f32; 4] {
    let offset = y * stride + x * 8;
    [
        half(&bytes[offset..offset + 2]),
        half(&bytes[offset + 2..offset + 4]),
        half(&bytes[offset + 4..offset + 6]),
        half(&bytes[offset + 6..offset + 8]),
    ]
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
fn nonfinite_ground_corners_refuse_before_publishing_assets() {
    let bytes = variation_sheet();
    for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut raw = one_tile(&[*b"Ldrt"], [0, 0, 0, 0], 0).raw().to_vec();
        raw[33..37].copy_from_slice(&invalid.to_le_bytes());
        let terrain = Terrain::from_bytes(raw).unwrap();
        let (mut meshes, mut images, mut materials) = empty_assets();
        let result = spawn_admitted_ground(
            &terrain,
            &[slot(*b"Ldrt", "synthetic/finite.blp", &bytes)],
            memory(5440, 8192),
            None,
            &mut meshes,
            &mut images,
            &mut materials,
        );
        assert!(
            result.is_err(),
            "nonfinite origin was published: {invalid:?}"
        );
        assert_unpublished(&images, &meshes, &materials);
    }
}

#[test]
fn device_extent_and_array_limits_refuse_before_decode_or_publication() {
    use bevy::render::render_resource::WgpuLimits;
    let bytes = direct_blp(32, 32, None);
    let terrain = one_tile(&[*b"Ldrt"], [0, 0, 0, 0], 0);
    for limits in [
        WgpuLimits {
            max_texture_dimension_2d: 7,
            ..Default::default()
        },
        WgpuLimits {
            max_texture_array_layers: 15,
            ..Default::default()
        },
    ] {
        let (mut meshes, mut images, mut materials) = empty_assets();
        let error = spawn_admitted_ground(
            &terrain,
            &[slot(*b"Ldrt", "synthetic/device-bound.blp", &bytes)],
            memory(5440, 8192),
            Some(&limits),
            &mut meshes,
            &mut images,
            &mut materials,
        )
        .unwrap_err();
        assert!(
            matches!(error, GroundError::DeviceExtent { .. }),
            "{error:?}"
        );
        assert_unpublished(&images, &meshes, &materials);
    }
    let (mut meshes, mut images, mut materials) = empty_assets();
    assert_eq!(
        spawn_admitted_ground(
            &one_tile(&[], [0, 0, 0, 0], 0),
            &[],
            memory(0, 0),
            None,
            &mut meshes,
            &mut images,
            &mut materials,
        )
        .unwrap_err(),
        GroundError::EmptySlots
    );
    assert_unpublished(&images, &meshes, &materials);
}

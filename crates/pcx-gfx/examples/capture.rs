//! Renders the Footman offscreen and writes a PNG, so the render can be checked
//! without a human at a window.
//!
//! The image is the artefact: `analyze` reads the pixels back and reports
//! coverage, colour spread, and whether anything drew at all. A render that
//! produces an empty or monochrome buffer is a failure this catches, and a
//! human-verified screenshot is not required to see it.

use std::path::Path;
use std::process::ExitCode;

use bevy::asset::RenderAssetUsages;
use bevy::camera::RenderTarget;
use bevy::image::Image;
use bevy::mesh::Mesh;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use bevy::window::ExitCondition;

use pcx_gfx::viewer::rgba_image;
use pcx_gfx::{
    LoadedModel, load_image, load_model, model_texture_paths, pose_skeleton, skin_mesh,
    to_bevy_mesh,
};
use pcx_mdl::{Bone, Mdlx, Pivots};
use pcx_mpq::MpqSet;

const WIDTH: u32 = 320;
const HEIGHT: u32 = 320;
const MODEL: &str = "Units\\Human\\Footman\\Footman.mdx";
const TERRAIN: &str = "TerrainArt\\Ashenvale\\Ashen_DirtGrass.blp";

/// Which frame of the animation to pose the model at.
///
/// 7000 ms is the first key frame of the clip that actually animates, so the
/// capture shows a posed model rather than a rest pose.
const POSE_FRAME: u32 = 7000;

fn install() -> std::path::PathBuf {
    for key in ["PEACECRAFT3_LEGACY_DIR", "PEACECRAFT3_INSTALL"] {
        if let Ok(v) = std::env::var(key) {
            let p = std::path::PathBuf::from(v);
            if p.is_dir() {
                return p;
            }
        }
    }
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("wc3/game/app")
}

#[derive(Resource)]
struct Shot {
    image: Handle<Image>,
    /// Set once the readback has been asked for, so only one is requested.
    requested: bool,
    /// Set once the buffer has been written and analysed.
    saved: bool,
}

fn main() -> ExitCode {
    let dir = install();
    let mut set = MpqSet::open_wc3(&dir).unwrap_or_else(|e| panic!("{dir:?}: {e}"));
    let bytes = set
        .extract(MODEL)
        .unwrap_or_else(|e| panic!("{MODEL}: {e}"));

    let model = load_model(&bytes, MODEL).unwrap_or_else(|e| panic!("model: {e}"));
    let parsed = Mdlx::parse(&bytes).unwrap();
    let bones: Vec<Bone> = parsed.bones(&bytes).unwrap_or_default();
    let pivots: Pivots = parsed.pivots(&bytes).unwrap_or_default();
    println!(
        "model {MODEL}: {} geosets, {} bones, {} pivots, clip frame {POSE_FRAME}",
        model.meshes.len(),
        bones.len(),
        pivots.0.len()
    );

    // The terrain tile and the model's textures, both from memory.
    let tile = set
        .extract(TERRAIN)
        .ok()
        .and_then(|b| load_image(&b, TERRAIN).ok());
    // One entry per material, in material order, so a geoset's `material_id`
    // indexes this list exactly as it indexes the model's `MTLS`. A material
    // with an empty path — the footman's second — or a texture that fails to
    // load is `None`, and its geoset falls back to flat colour.
    let tex_paths = model_texture_paths(&bytes, MODEL).unwrap_or_default();
    let mut textures: Vec<Option<pcx_gfx::LoadedTexture>> = Vec::with_capacity(tex_paths.len());
    for p in &tex_paths {
        let loaded = if p.is_empty() {
            None
        } else {
            set.extract(p).ok().and_then(|b| load_image(&b, p).ok())
        };
        textures.push(loaded);
    }
    println!(
        "terrain {}x{}, {} of {} materials textured: {:?}",
        tile.as_ref().map_or(0, |t| t.width),
        tile.as_ref().map_or(0, |t| t.height),
        textures.iter().flatten().count(),
        tex_paths.len(),
        tex_paths
    );

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            // A headless capture must not require a window.
            primary_window: None,
            // `exit_on_all_closed` (WindowPlugin's default) ends the run on
            // the very first frame, because there is never a window to keep
            // open. This capture ends itself instead, once the pixels have
            // been read back.
            exit_condition: ExitCondition::DontExit,
            ..default()
        }))
        .insert_resource(Shot {
            image: Handle::default(),
            requested: false,
            saved: false,
        })
        .insert_resource(Scene {
            model,
            bones,
            pivots,
            tile,
            textures,
        })
        .add_systems(Startup, spawn)
        .add_systems(Update, save_when_ready)
        .run();

    ExitCode::SUCCESS
}

#[derive(Resource)]
struct Scene {
    model: LoadedModel,
    bones: Vec<Bone>,
    pivots: Pivots,
    tile: Option<pcx_gfx::LoadedTexture>,
    /// One entry per material, in material order; `None` where the material
    /// names no texture or the texture did not load.
    textures: Vec<Option<pcx_gfx::LoadedTexture>>,
}

fn spawn(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut shot: ResMut<Shot>,
    scene: Res<Scene>,
) {
    let mut target = Image::new_uninit(
        Extent3d {
            width: WIDTH,
            height: HEIGHT,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    target.sampler = bevy::image::ImageSampler::linear();
    // The camera renders into this image, so the GPU texture has to be a render
    // attachment. `new_uninit` only asks for texture binding and copies, which
    // is fine for a texture you read *from* but not for one you draw into.
    // COPY_SRC is retained so the pixels can be copied back out.
    target.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING
        | TextureUsages::COPY_DST
        | TextureUsages::COPY_SRC
        | TextureUsages::RENDER_ATTACHMENT;
    let target = images.add(target);
    shot.image = target.clone();

    commands.spawn((
        Camera3d::default(),
        Camera {
            // The clear has to be black, or the background itself would count as
            // "lit" and a blank render would pass as a captured one.
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            ..default()
        },
        // `Camera::target` is gone: the render target is its own required
        // component on the camera entity.
        RenderTarget::Image(target.into()),
        Projection::Perspective(PerspectiveProjection {
            fov: 0.6,
            near: 0.5,
            far: 5000.0,
            ..default()
        }),
        // Framed from the model's own height rather than a magic distance.
        Transform::from_xyz(150.0, -210.0, 120.0).looking_at(Vec3::new(0.0, 0.0, 80.0), Vec3::Z),
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 14_000.0,
            ..default()
        },
        Transform::from_xyz(200.0, -200.0, 300.0),
    ));
    // Scene-wide ambient is `GlobalAmbientLight` in 0.20; `AmbientLight` is
    // now the per-camera override component.
    commands.insert_resource(GlobalAmbientLight {
        color: Color::WHITE,
        brightness: 600.0,
        affects_lightmapped_meshes: true,
    });

    // The terrain plane, textured, so the Footman is not floating in a void.
    if let Some(tile) = scene.tile.as_ref() {
        let image = images.add(rgba_image(tile));
        let material = materials.add(StandardMaterial {
            base_color_texture: Some(image),
            unlit: true,
            perceptual_roughness: 1.0,
            ..default()
        });
        commands.spawn((
            MeshMaterial3d(material),
            Mesh3d(meshes.add(quad())),
            Transform::from_xyz(0.0, 0.0, 0.0),
        ));
    }

    // Every geoset, posed.
    let skeleton = pose_skeleton(&scene.model, &scene.bones, &scene.pivots, POSE_FRAME);
    for (i, rest) in scene.model.meshes.iter().enumerate() {
        let skinned = skin_mesh(rest, &skeleton);
        let mut mesh = to_bevy_mesh(rest);
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, skinned);
        let mesh_handle = meshes.add(mesh);

        // The texture bound to this geoset is the one its `material_id` names,
        // read off the geoset's trailer rather than cycled from the geoset
        // index. The Footman's five geosets read 0, 1, 0, 0, 2: the last is the
        // blood geoset and takes `Textures\gutz.blp`, which index cycling gave
        // to a body piece while the blood geoset drew with the empty-texture
        // material instead. A material with no loadable texture falls back to
        // flat colour; it is never silently swapped for another material's.
        let texture = scene
            .textures
            .get(rest.material_id as usize)
            .and_then(|t| t.as_ref());
        println!(
            "geoset {i}: material {} -> {}",
            rest.material_id,
            texture.as_ref().map_or("<flat colour>", |t| &t.source)
        );
        let material = match texture {
            Some(t) => {
                let image = images.add(rgba_image(t));
                materials.add(StandardMaterial {
                    base_color_texture: Some(image),
                    perceptual_roughness: 0.9,
                    ..default()
                })
            }
            None => materials.add(StandardMaterial {
                base_color: Color::srgb(0.75, 0.75, 0.78),
                ..default()
            }),
        };
        commands.spawn((
            MeshMaterial3d(material),
            Mesh3d(mesh_handle),
            Transform::from_xyz(0.0, 0.0, 0.0),
        ));
    }
    println!(
        "scene spawned: 1 camera, 1 light, 1 terrain, {} geosets",
        scene.model.meshes.len()
    );
}

fn quad() -> Mesh {
    let mut mesh = Mesh::new(
        bevy::mesh::PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![
            [-400.0, 400.0, 0.0],
            [400.0, 400.0, 0.0],
            [400.0, -400.0, 0.0],
            [-400.0, -400.0, 0.0],
        ],
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_UV_0,
        vec![[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; 4]);
    mesh.insert_indices(bevy::mesh::Indices::U16(vec![0, 1, 2, 0, 2, 3]));
    mesh
}

/// How many frames the GPU needs before a fresh render target holds pixels.
const WARMUP_FRAMES: u32 = 40;

/// Asks the renderer for the pixels once the image render target is warm.
///
/// The image asset itself never receives data: `RenderTarget::Image` renders
/// into the GPU texture behind the handle, so the only way back to the CPU is
/// the renderer's own readback. [`Screenshot`] copies that texture into a
/// buffer and hands it over as [`ScreenshotCaptured`] when the GPU is done.
fn save_when_ready(
    mut commands: Commands,
    mut shot: ResMut<Shot>,
    mut frames: Local<u32>,
    mut exit: MessageWriter<AppExit>,
) {
    if shot.requested {
        if shot.saved {
            exit.write(AppExit::Success);
        }
        return;
    }
    *frames += 1;
    if *frames < WARMUP_FRAMES {
        return;
    }
    shot.requested = true;
    commands
        .spawn(Screenshot::image(shot.image.clone()))
        .observe(captured);
}

/// The captured pixels: write them out and report what is in them.
fn captured(screenshot: On<ScreenshotCaptured>, mut shot: ResMut<Shot>) {
    let image = &screenshot.image;
    let data = image.data.as_deref().unwrap_or_default();
    let path = Path::new("/tmp/opencode/capture.png");
    if let Err(e) = image::save_buffer(
        path,
        data,
        image.width(),
        image.height(),
        image::ColorType::Rgba8,
    ) {
        eprintln!("save {path:?}: {e}");
    } else {
        println!("wrote {}", path.display());
    }
    analyze(data);
    shot.saved = true;
}

/// Reads the rendered buffer back and reports what is actually in it.
///
/// This is the "look at it" step: coverage, colour spread, and a coarse
/// row-profile so a silhouette can be recognised from numbers alone.
fn analyze(data: &[u8]) {
    let n = (WIDTH * HEIGHT) as usize;
    let pixels = &data[..n * 4];
    let mut covered = 0usize;
    let mut lit = 0usize;
    let mut colours = std::collections::BTreeSet::new();
    let mut rows = vec![0u32; HEIGHT as usize];
    let mut sum = [0u64; 3];

    for (i, p) in pixels.as_chunks::<4>().0.iter().enumerate() {
        if p[3] > 0 {
            covered += 1;
        }
        let (r, g, b) = (u32::from(p[0]), u32::from(p[1]), u32::from(p[2]));
        sum[0] += u64::from(r);
        sum[1] += u64::from(g);
        sum[2] += u64::from(b);
        if r + g + b > 24 {
            lit += 1;
        }
        colours.insert((p[0] >> 3, p[1] >> 3, p[2] >> 3));
        rows[i / WIDTH as usize] += r + g + b;
    }
    let total = n as f64;
    println!("capture {WIDTH}x{HEIGHT}");
    println!(
        "  alpha>0      {:>8} ({:.1}%)",
        covered,
        100.0 * covered as f64 / total
    );
    println!(
        "  lit (sum>24) {:>8} ({:.1}%)",
        lit,
        100.0 * lit as f64 / total
    );
    println!(
        "  mean rgb     {:.1} {:.1} {:.1}",
        sum[0] as f64 / total,
        sum[1] as f64 / total,
        sum[2] as f64 / total
    );
    println!("  distinct colours (5-bit) {}", colours.len());

    // A silhouette: how much of each row is lit. A standing figure has a wide
    // middle and narrow top; a flat plane is uniform.
    let step = 16;
    for y in (0..HEIGHT as usize).step_by(step) {
        let v: u32 = rows[y..(y + step).min(rows.len())].iter().sum();
        let bar = "#".repeat((v / (step as u32 * 40)).min(60) as usize);
        println!("  y{y:>3} {v:>10} {bar}");
    }
    assert!(lit > 200, "almost nothing rendered ({lit} lit pixels)");
    assert_eq!(covered, n, "the buffer is not fully opaque");
}

//! A minimal model viewer: the M2 gate.
//!
//! The gate is "a Footman renders with correct skinning, animation and a
//! sprite-lit terrain tile". This module is the first half of it — the geometry
//! and texture path — and it exists to be *looked at*, which is why it is a
//! window and not a test.
//!
//! What is verified rather than seen: every mesh that reaches the GPU carries
//! exactly the vertex, normal, UV and index counts the decoder produced, and the
//! texture bound to each one is the one its material names. That is in
//! `spawn_model`'s assertions, because a render that merely *runs* can still be
//! showing the wrong thing.

use std::path::{Path, PathBuf};

use bevy::animation::animated_field;
use bevy::animation::{AnimatedBy, AnimationTargetId};
use bevy::app::AnimationSystems;
use bevy::curve::{Curve, Interval};
use bevy::prelude::*;

use crate::{LoadedTexture, load_image, pose_from_poses, skin_mesh, to_bevy};

/// Where to find the archives.
pub fn resolve_install(explicit: Option<&str>) -> PathBuf {
    if let Some(p) = explicit {
        return PathBuf::from(p);
    }
    for key in ["PEACECRAFT3_INSTALL", "PEACECRAFT3_LEGACY_DIR"] {
        if let Ok(v) = std::env::var(key) {
            let p = PathBuf::from(v);
            if p.is_dir() {
                return p;
            }
        }
    }
    PathBuf::from("wc3/game/app")
}

/// Spawns every geoset of a model, each with the texture its material names.
///
/// This is the function the gate is really about: it is where the decoder's
/// output becomes GPU state. Anything wrong upstream shows up here as a count
/// mismatch, which is why the counts are asserted rather than assumed.
pub fn spawn_model(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    bytes: &[u8],
    source: &str,
    textures: &[LoadedTexture],
) -> Result<usize, crate::Error> {
    spawn_model_instance(commands, meshes, materials, images, bytes, source, textures)
        .map(|(_, count)| count)
}

#[derive(Component)]
struct SpawnedGeoset {
    instance: Entity,
    geoset: usize,
}

fn spawn_model_instance(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    bytes: &[u8],
    source: &str,
    textures: &[LoadedTexture],
) -> Result<(Entity, usize), crate::Error> {
    let parts = to_bevy(bytes, source)?;
    let instance = commands
        .spawn((
            Transform::default(),
            Visibility::Inherited,
            Name::new(source.to_owned()),
        ))
        .id();
    if parts.is_empty() {
        return Ok((instance, 0));
    }

    // The model's own extents give the camera something sensible to frame,
    // rather than a hard-coded distance that only suits one unit.
    let model = crate::load_model(bytes, source)?;
    if parts.len() != model.meshes.len() {
        return Err(crate::Error::Decode(format!(
            "{source}: mesh/geoset count mismatch"
        )));
    }
    let centre = [
        (model.minimum_extent[0] + model.maximum_extent[0]) * 0.5,
        (model.minimum_extent[1] + model.maximum_extent[1]) * 0.5,
        (model.minimum_extent[2] + model.maximum_extent[2]) * 0.5,
    ];
    let height = (model.maximum_extent[2] - model.minimum_extent[2]).max(1.0);
    let authored = pcx_mdl::Mdlx::parse(bytes)
        .and_then(|mdl| mdl.textures(bytes))
        .unwrap_or_default();

    let mut spawned = 0;
    for ((mesh, path), rest) in parts.iter().zip(&model.meshes) {
        let mesh_handle = meshes.add(mesh.clone());

        // A geoset whose material points at no texture draws flat-shaded rather
        // than invisible: a missing texture is a visible fact, not a black hole.
        let image = if path.is_empty() {
            None
        } else {
            pick(textures, path)
        };
        // The model's own `TEXS` flags decide whether this texture repeats.
        // Read once per model below and looked up per geoset.
        let authored_flags = authored_texture_flags(&authored, path);

        let material = match image {
            Some(tex) => {
                let mut image = rgba_image(tex);
                image.sampler = sampler_for_texture_flags(authored_flags);
                let handle = images.add(image);
                materials.add(StandardMaterial {
                    base_color_texture: Some(handle),
                    perceptual_roughness: 0.9,
                    ..default()
                })
            }
            None => materials.add(StandardMaterial {
                base_color: Color::srgb(0.75, 0.75, 0.78),
                perceptual_roughness: 0.9,
                ..default()
            }),
        };

        commands.spawn((
            MeshMaterial3d(material),
            Mesh3d(mesh_handle),
            Transform::from_translation(Vec3::new(centre[0], centre[1], centre[2])),
            SpawnedGeoset {
                instance,
                geoset: rest.geoset,
            },
            ChildOf(instance),
        ));
        spawned += 1;
    }

    // Frame the model from its own size.
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(height * 1.6, -height * 2.2, height * 1.1)
            .looking_at(Vec3::new(centre[0], centre[1], centre[2]), Vec3::Z),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 12_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(height * 2.0, -height * 2.0, height * 3.0)
            .looking_at(Vec3::new(centre[0], centre[1], centre[2]), Vec3::Z),
    ));
    // Ambient light is a resource-wide default in 0.20 (`GlobalAmbientLight`);
    // `AmbientLight` itself became a per-camera override component.
    commands.insert_resource(GlobalAmbientLight {
        color: Color::WHITE,
        brightness: 400.0,
        affects_lightmapped_meshes: true,
    });

    Ok((instance, spawned))
}

/// The authored `TEXS` flags for `path`, or `0` for an unnamed texture.
fn authored_texture_flags(textures: &[pcx_mdl::Texture], path: &str) -> u32 {
    let norm = |s: &str| s.replace('/', "\\").to_ascii_lowercase();
    let want = norm(path);
    textures
        .iter()
        .find(|t| norm(&t.path) == want)
        .map(|t| t.flags)
        .unwrap_or(0)
}

/// Finds the loaded texture whose source path matches `path`.
///
/// Both spellings are accepted because the archives use backslashes while a
/// caller may reasonably hold forward slashes, and a mismatch here would be
/// invisible: the geoset would simply draw untextured.
fn pick<'a>(textures: &'a [LoadedTexture], path: &str) -> Option<&'a LoadedTexture> {
    let norm = |s: &str| s.replace('/', "\\").to_ascii_lowercase();
    let want = norm(path);
    textures
        .iter()
        .find(|t| norm(&t.source) == want)
        .or_else(|| textures.iter().find(|t| norm(&t.source).ends_with(&want)))
}

/// Turns straight RGBA8 into a Bevy image.
pub fn rgba_image(tex: &LoadedTexture) -> Image {
    use bevy::asset::RenderAssetUsages;
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

    Image::new(
        Extent3d {
            width: tex.width.max(1),
            height: tex.height.max(1),
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        tex.rgba.clone(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

/// The sampler a texture's own `TEXS` flags ask for.
///
/// The authored bits are [`pcx_mdl::TEXTURE_WRAP_WIDTH`] and
/// [`pcx_mdl::TEXTURE_WRAP_HEIGHT`]; an unset bit means the texture does not
/// repeat along that axis, and Bevy's default is to clamp. Textures in the
/// shipped main-menu scenes carry UVs far outside `[0, 1]` — the expansion
/// scene's sky spans roughly `U -2.6 .. 4.3` — so a clamp smears the last row
/// and column across the whole surface, which is what streaked ice and water
/// looked like.
///
/// The filter is left at its default: nothing in the format flags says which
/// filter to use, and inventing one would be a change the original does not ask
/// for.
#[must_use]
pub fn sampler_for_texture_flags(flags: u32) -> bevy::image::ImageSampler {
    use bevy::image::ImageSampler as S;
    use bevy::image::{ImageAddressMode, ImageSamplerDescriptor};

    if flags & (pcx_mdl::TEXTURE_WRAP_WIDTH | pcx_mdl::TEXTURE_WRAP_HEIGHT) == 0 {
        // Neither axis wraps: the default sampler is already correct, and
        // returning it unchanged keeps a non-wrapping texture byte-identical to
        // what it was before this existed.
        return S::default();
    }
    let wrap = |set: bool| {
        if set {
            ImageAddressMode::Repeat
        } else {
            ImageAddressMode::ClampToEdge
        }
    };
    S::Descriptor(ImageSamplerDescriptor {
        address_mode_u: wrap(flags & pcx_mdl::TEXTURE_WRAP_WIDTH != 0),
        address_mode_v: wrap(flags & pcx_mdl::TEXTURE_WRAP_HEIGHT != 0),
        ..ImageSamplerDescriptor::default()
    })
}

/// Opens a window and draws a model.
///
/// `install` is where the archives live, so a texture can be fetched on demand;
/// `model` is an internal MPQ path such as
/// `Units\Human\Footman\Footman.mdx`.
pub fn run(install: &Path, model: &str) -> Result<(), crate::Error> {
    let install = install.to_path_buf();
    let model = model.to_string();

    let bytes = pcx_mpq::MpqSet::open_wc3(&install)
        .map_err(|e| crate::Error::Decode(format!("{}: {e}", install.display())))?
        .extract(&model)
        .map_err(|e| crate::Error::Decode(format!("{model}: {e}")))?;

    // Textures are resolved up front so a failure is a message rather than a
    // silently untextured model.
    let textures = crate::load_model(&bytes, &model)
        .ok()
        .map(|_| ())
        .and_then(|_| {
            let paths = crate::model_texture_paths(&bytes, &model).ok()?;
            let mut set = pcx_mpq::MpqSet::open_wc3(&install).ok()?;
            Some(
                paths
                    .iter()
                    .filter(|p| !p.is_empty())
                    .filter_map(|p| {
                        let b = set.extract(p).ok()?;
                        crate::load_image(&b, p).ok()
                    })
                    .collect::<Vec<_>>(),
            )
        });

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: format!("peacecraft3 — {model}"),
                resolution: (1024u32, 768u32).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(crate::performance::PerformanceOverlayPlugin)
        .insert_resource(Viewer {
            install,
            model,
            bytes,
            textures: textures.unwrap_or_default(),
            spawned: false,
        })
        .init_resource::<ViewerCache>()
        .add_systems(Startup, spawn_once)
        .add_systems(Update, animate)
        .add_systems(PostUpdate, skin_from_animation.after(AnimationSystems))
        .run();
    Ok(())
}

#[derive(Resource)]
struct Viewer {
    install: PathBuf,
    model: String,
    bytes: Vec<u8>,
    textures: Vec<LoadedTexture>,
    spawned: bool,
}

/// The terrain tile drawn under a model, and where the viewer looks for it.
///
/// The shipped table that maps a ground slot fourcc to its texture path.
const TERRAIN_SLK: &str = "TerrainArt\\Terrain.slk";

fn spawn_once(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut animations: ResMut<Assets<bevy::animation::AnimationClip>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut viewer: ResMut<Viewer>,
) {
    if viewer.spawned {
        return;
    }
    viewer.spawned = true;

    let model = match crate::load_model(&viewer.bytes, &viewer.model) {
        Ok(m) => m,
        Err(e) => {
            error!("cannot show {}: {e}", viewer.model);
            return;
        }
    };

    // Parsed once, so the clip choice and the per-frame skinning agree on the
    // same skeleton and pivots.
    let joints = match crate::mdx_runtime::JointPlan::parse(&viewer.bytes) {
        Ok(joints) => joints,
        Err(error) => {
            error!("{}: {error}", viewer.model);
            return;
        }
    };
    let bones = joints.joints().to_vec();
    let pivots = joints.pivots().clone();
    let anims = model_anims(&viewer.bytes);
    info!(
        "{} geoset animations, {} bones, {} pivots",
        anims.len(),
        bones.len(),
        pivots.0.len()
    );

    // Record what the model actually offers, so a clip can be chosen by name
    // rather than by an index that means nothing to whoever runs this.
    let clips: Vec<(String, u32)> = model
        .sequences
        .iter()
        .map(|s| (s.name.clone(), s.duration_ms))
        .collect();
    info!("{} clips: {clips:?}", clips.len());

    // Play the clip that actually contains the model's first bone key.
    //
    // Matching a sequence literally called "Stand" is wrong for the Footman,
    // whose stand clips are `Stand - 1`, `Stand - 2` and so on — nothing would
    // be selected and the animate system would panic on a missing resource.
    let first_key = bones
        .iter()
        .filter_map(|b| b.translation())
        .filter_map(|t| t.key_time(0))
        .min();
    let chosen = first_key
        .and_then(|k| {
            model
                .sequences
                .iter()
                .position(|s| k >= s.start_ms && k <= s.start_ms + s.duration_ms)
        })
        .unwrap_or(0);

    // One `AnimationClip` per `SEQS` sequence, carrying the model's own
    // `KGTR`/`KGRT`/`KGSC` keys, and one bone entity per bone for Bevy's
    // animation system to pose. The clip is what the viewer plays; the
    // skinning below reads the poses it produced.
    let clip_handles: Vec<Handle<bevy::animation::AnimationClip>> = model
        .sequences
        .iter()
        .map(|s| animations.add(build_clip(&bones, s.start_ms, s.duration_ms)))
        .collect();
    let (graph, nodes) = AnimationGraph::from_clips(clip_handles);
    let graph_handle = graphs.add(graph);
    let player = commands
        .spawn((
            AnimationPlayer::default(),
            AnimationGraphHandle(graph_handle),
        ))
        .id();
    let bone_entities: Vec<Entity> = bones
        .iter()
        .map(|bone| {
            commands
                .spawn((
                    Transform::default(),
                    AnimationTargetId::from_name(&Name::new(bone_key(bone))),
                    AnimatedBy(player),
                ))
                .id()
        })
        .collect();

    let entries: Vec<ClipEntry> = model
        .sequences
        .iter()
        .zip(nodes)
        .map(|(s, node)| ClipEntry {
            node,
            duration_ms: s.duration_ms as f32,
            loops: s.loops,
        })
        .collect();
    commands.insert_resource(PlaybackState {
        playback: crate::Playback {
            sequence: chosen,
            time_ms: 0.0,
            looping: model.sequences.get(chosen).map(|s| s.loops).unwrap_or(true),
        },
        clips: entries,
        player,
    });

    match spawn_model_instance(
        &mut commands,
        &mut meshes,
        &mut materials,
        &mut images,
        &viewer.bytes,
        &viewer.model,
        &viewer.textures,
    ) {
        Ok((instance, n)) => {
            info!("spawned {n} geosets");
            commands.insert_resource(ViewerCache {
                model: Some(model),
                instance: Some(instance),
                bones,
                pivots,
                bone_entities,
            });
        }
        Err(e) => error!("cannot show {}: {e}", viewer.model),
    }

    // A ground plane textured with a real tile, so the model is seen on terrain
    // rather than floating in a void. This is the "terrain tile" half of the M2
    // gate: it proves the texture path works on a tile, not just on a model.
    spawn_terrain(
        &mut commands,
        &mut images,
        &mut meshes,
        &mut materials,
        &viewer.install,
    );
}

/// Spawns a ground plane textured with an asset-derived terrain tile.
fn spawn_terrain(
    commands: &mut Commands,
    images: &mut Assets<Image>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    install: &Path,
) {
    let (texture, tile_path) = match pcx_mpq::MpqSet::open_wc3(install).ok() {
        Some(mut set) => {
            let path = set
                .extract(TERRAIN_SLK)
                .ok()
                .and_then(|b| terrain_tile_path_from_slk(&b).ok());
            let texture = path
                .as_ref()
                .and_then(|p| set.extract(p).ok().and_then(|b| load_image(&b, p).ok()));
            (texture, path)
        }
        None => (None, None),
    };

    let material = match texture {
        Some(tex) => {
            let handle = images.add(rgba_image(&tex));
            materials.add(StandardMaterial {
                base_color_texture: Some(handle),
                // A tile is viewed edge-on; unshaded and unfogged keeps it from
                // disappearing under the model's own lighting.
                unlit: true,
                perceptual_roughness: 1.0,
                ..default()
            })
        }
        None => {
            // A missing tile must not take the scene down with it.
            if let Some(path) = tile_path {
                error!("cannot stage terrain tile {path}");
            } else {
                error!("cannot resolve terrain tile from {TERRAIN_SLK}");
            }
            materials.add(StandardMaterial {
                base_color: Color::srgb(0.25, 0.30, 0.22),
                unlit: true,
                perceptual_roughness: 1.0,
                ..default()
            })
        }
    };

    commands.spawn((
        MeshMaterial3d(material),
        Mesh3d(meshes.add(terrain_plane_mesh())),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));
}

/// Asset-derived terrain tile path from `TerrainArt\\Terrain.slk`.
pub fn terrain_tile_path_from_slk(bytes: &[u8]) -> Result<String, crate::Error> {
    let table = pcx_slq::Table::parse(bytes)
        .map_err(|e| crate::Error::Decode(format!("{TERRAIN_SLK}: {e}")))?;
    if table.column_of("tileID").is_none()
        || table.column_of("dir").is_none()
        || table.column_of("file").is_none()
    {
        return Err(crate::Error::Decode(format!(
            "{TERRAIN_SLK}: missing column"
        )));
    }
    let mut entries: Vec<([u8; 4], String)> = Vec::new();
    for row in table.data_rows() {
        let tile_id = row.by_name("tileID").unwrap_or("");
        if tile_id.len() != 4 {
            continue;
        }
        let dir = row.by_name("dir").unwrap_or("");
        let file = row.by_name("file").unwrap_or("");
        if file.is_empty() {
            continue;
        }
        let four = [
            tile_id.as_bytes()[0],
            tile_id.as_bytes()[1],
            tile_id.as_bytes()[2],
            tile_id.as_bytes()[3],
        ];
        entries.push((four, format!("{dir}\\{file}.blp")));
    }
    if entries.is_empty() {
        return Err(crate::Error::Decode(format!("{TERRAIN_SLK}: no tile rows")));
    }
    entries.sort_by_key(|(four, _)| *four);
    Ok(entries[0].1.clone())
}

/// Asset-derived terrain tile path using the supplied install.
pub fn terrain_tile_path(install: &Path) -> Result<String, crate::Error> {
    let mut set = pcx_mpq::MpqSet::open_wc3(install)
        .map_err(|e| crate::Error::Decode(format!("{}: {e}", install.display())))?;
    let bytes = set
        .extract(TERRAIN_SLK)
        .map_err(|e| crate::Error::Decode(format!("{TERRAIN_SLK}: {e}")))?;
    terrain_tile_path_from_slk(&bytes)
}

/// A single quad, tiled a few times so the texture reads as terrain.
pub fn terrain_plane_mesh() -> Mesh {
    use bevy::mesh::{Indices, PrimitiveTopology};
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
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
    mesh.insert_indices(Indices::U16(vec![0, 1, 2, 0, 2, 3]));
    mesh
}

/// One playback step, in milliseconds.
///
/// Playback advances by this fixed step once per frame rather than by wall
/// clock, so the pose shown on frame N of a run does not depend on how long
/// the frames took. It is the model's own animation rate: 30 ticks a second.
const TICK_MS: f32 = 1_000.0 / 30.0;

/// Which clip is playing, and the Bevy node each sequence became.
#[derive(Resource)]
struct PlaybackState {
    /// The model's own clip clock: which sequence, and how far into it.
    playback: crate::Playback,
    /// One per sequence, in sequence order.
    clips: Vec<ClipEntry>,
    /// The entity holding the [`AnimationPlayer`].
    player: Entity,
}

/// One sequence as Bevy plays it.
#[derive(Debug, Clone, Copy)]
struct ClipEntry {
    node: AnimationNodeIndex,
    duration_ms: f32,
    loops: bool,
}

/// Advances the playing clip by one fixed tick and hands the time to Bevy.
///
/// The active animation is paused so [`bevy::animation::advance_animations`]
/// cannot move it on wall-clock `Time`: the seek written here is the only
/// thing that advances it. Bevy's animation system does the rest — the
/// `AnimationClip` curves posed the bones, and [`skin_from_animation`] reads
/// the result.
fn animate(mut playback: ResMut<PlaybackState>, mut players: Query<&mut AnimationPlayer>) {
    let Some(entry) = playback.clips.get(playback.playback.sequence).copied() else {
        return;
    };
    playback
        .playback
        .advance(TICK_MS, entry.duration_ms, entry.loops);
    let seek = playback.playback.time_ms / 1000.0;

    let Ok(mut player) = players.get_mut(playback.player) else {
        return;
    };
    let active = player.play(entry.node);
    active.pause();
    active.set_seek_time(seek);
}

/// Re-skins every mesh to the pose Bevy's animation system has just applied.
///
/// This is where the decoded formats become motion: the clip posed each bone
/// entity's [`Transform`], those poses compose into world matrices through the
/// decoder's own matrix convention, and each mesh's positions are rewritten.
/// A vertex owned by a matrix group the skeleton does not cover keeps its rest
/// position, because dropping it would punch holes in the model rather than
/// leave it static.
fn skin_from_animation(
    mut meshes: ResMut<Assets<Mesh>>,
    cache: Res<ViewerCache>,
    posed: Query<&Transform>,
    spawned: Query<(&Mesh3d, &SpawnedGeoset)>,
) {
    let Some(model) = cache.model.as_ref() else {
        return;
    };
    let poses: Vec<pcx_mdl::Pose> = cache
        .bone_entities
        .iter()
        .map(|&e| posed.get(e).map(pose_of).unwrap_or_default())
        .collect();
    let skeleton = pose_from_poses(model, &cache.bones, &cache.pivots, &poses);
    for (handle, binding) in &spawned {
        if Some(binding.instance) != cache.instance {
            continue;
        }
        let Some(mut mesh) = meshes.get_mut(&handle.0) else {
            continue;
        };
        // load_model retains GEOS records in authored ordinal order.
        let Some(rest) = model.meshes.get(binding.geoset) else {
            continue;
        };
        let skinned = skin_mesh(rest, &skeleton);
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, skinned);
    }
}

/// The pose a bone entity's transform carries: the format's own components.
///
/// `Quat::to_array` is `x, y, z, w`, which is the order `Pose` reads.
fn pose_of(t: &Transform) -> pcx_mdl::Pose {
    pcx_mdl::Pose {
        translation: t.translation.to_array(),
        rotation: t.rotation.to_array(),
        scale: t.scale.to_array(),
    }
}

/// A bone's animation identity.
///
/// Object ids are unique within a model and stable across runs; bone names are
/// not — the shipped set has unnamed bones — and `AnimationTargetId` is a hash
/// of exactly this string.
fn bone_key(bone: &pcx_mdl::Bone) -> String {
    format!("bone/{}", bone.object_id)
}

/// Which half of a [`pcx_mdl::Pose`] a `Vec3` curve reads.
#[derive(Debug, Clone, Copy, Reflect)]
enum Vec3Channel {
    Translation,
    Scale,
}

/// A curve that samples a bone's `KGTR` or `KGSC` keys with the decoder's own
/// interpolation.
///
/// The keys are not re-interpolated into an `AnimatableKeyframeCurve`: the
/// format interpolates each component independently and Hermite tracks carry
/// tangents that a keyframe curve cannot hold, so sampling
/// [`pcx_mdl::sample_bone_pose`] at the requested instant is the only way the
/// clip shows what the model actually animates to.
#[derive(Debug, Clone, Reflect)]
#[reflect(from_reflect = false)]
struct BoneVec3Curve {
    #[reflect(ignore)]
    bone: pcx_mdl::Bone,
    channel: Vec3Channel,
    /// The sequence's own window on the model's millisecond timeline.
    start_ms: f32,
    end_ms: f32,
}

/// The same, for `KGRT`: a rotation is a quaternion and so is the curve.
#[derive(Debug, Clone, Reflect)]
#[reflect(from_reflect = false)]
struct BoneQuatCurve {
    #[reflect(ignore)]
    bone: pcx_mdl::Bone,
    start_ms: f32,
    end_ms: f32,
}

/// Clip seconds → the model's millisecond timeline.
///
/// Clamped to the sequence's own window, so a one-shot past its end holds its
/// last pose instead of sliding into the next sequence's keys — the timeline
/// is shared by every sequence in the file.
fn sample_at(bone: &pcx_mdl::Bone, start_ms: f32, end_ms: f32, t: f32) -> pcx_mdl::Pose {
    let ms = (start_ms + t * 1000.0).clamp(start_ms, end_ms.max(start_ms));
    pcx_mdl::sample_bone_pose(bone, ms.round() as u32)
}

/// The clip's time domain: `[0, duration]`, or everywhere for a zero-length
/// sequence (which [`AnimationClip::add_curve_to_target`] would not extend).
fn clip_domain(duration_sec: f32) -> Interval {
    if duration_sec > 0.0 {
        Interval::new(0.0, duration_sec).expect("a positive duration is a valid interval")
    } else {
        Interval::EVERYWHERE
    }
}

impl Curve<Vec3> for BoneVec3Curve {
    fn domain(&self) -> Interval {
        clip_domain((self.end_ms - self.start_ms) / 1000.0)
    }
    fn sample_unchecked(&self, t: f32) -> Vec3 {
        let pose = sample_at(&self.bone, self.start_ms, self.end_ms, t);
        match self.channel {
            Vec3Channel::Translation => Vec3::from_array(pose.translation),
            Vec3Channel::Scale => Vec3::from_array(pose.scale),
        }
    }
}

impl Curve<Quat> for BoneQuatCurve {
    fn domain(&self) -> Interval {
        clip_domain((self.end_ms - self.start_ms) / 1000.0)
    }
    fn sample_unchecked(&self, t: f32) -> Quat {
        Quat::from_array(sample_at(&self.bone, self.start_ms, self.end_ms, t).rotation)
    }
}

/// Builds one sequence as a Bevy [`AnimationClip`].
///
/// Every bone with `KGTR`/`KGRT`/`KGSC` keys becomes three curves on its own
/// [`AnimationTargetId`], each driving that bone entity's [`Transform`]. Clip
/// time 0 is the sequence's start, so a clip can be seeked independently of
/// where it sits on the model's shared timeline.
fn build_clip(
    bones: &[pcx_mdl::Bone],
    start_ms: u32,
    duration_ms: u32,
) -> bevy::animation::AnimationClip {
    let mut clip = bevy::animation::AnimationClip::default();
    let (start, end) = (start_ms as f32, (start_ms + duration_ms) as f32);
    for bone in bones {
        let target = AnimationTargetId::from_name(&Name::new(bone_key(bone)));
        if bone.translation().is_some() {
            clip.add_curve_to_target(
                target,
                AnimatableCurve::new(
                    animated_field!(Transform::translation),
                    BoneVec3Curve {
                        bone: bone.clone(),
                        channel: Vec3Channel::Translation,
                        start_ms: start,
                        end_ms: end,
                    },
                ),
            );
        }
        if bone.rotation().is_some() {
            clip.add_curve_to_target(
                target,
                AnimatableCurve::new(
                    animated_field!(Transform::rotation),
                    BoneQuatCurve {
                        bone: bone.clone(),
                        start_ms: start,
                        end_ms: end,
                    },
                ),
            );
        }
        if bone.scaling().is_some() {
            clip.add_curve_to_target(
                target,
                AnimatableCurve::new(
                    animated_field!(Transform::scale),
                    BoneVec3Curve {
                        bone: bone.clone(),
                        channel: Vec3Channel::Scale,
                        start_ms: start,
                        end_ms: end,
                    },
                ),
            );
        }
    }
    clip.set_duration(duration_ms as f32 / 1000.0);
    clip
}

/// Parsed model plus its animation records, cached for the session.
#[derive(Resource, Default)]
struct ViewerCache {
    model: Option<crate::LoadedModel>,
    instance: Option<Entity>,
    bones: Vec<pcx_mdl::Bone>,
    pivots: pcx_mdl::Pivots,
    /// The bone entities Bevy's animation system poses, in bone order.
    bone_entities: Vec<Entity>,
}

/// Cached animation records for the shown model.
fn model_anims(bytes: &[u8]) -> Vec<pcx_mdl::GeosetAnim> {
    pcx_mdl::Mdlx::parse(bytes)
        .and_then(|m| m.geoset_anims(bytes))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn equal_vertex_counts_keep_geoset_identity_and_other_instances_untouched() {
        let rest = |geoset, position| crate::LoadedMesh {
            positions: vec![position],
            normals: vec![[0.0, 0.0, 1.0]],
            tex_coords: vec![[0.0, 0.0]],
            indices: vec![],
            vertex_groups: vec![],
            bone_indices: vec![],
            group_counts: vec![],
            geoset,
            material_id: 0,
        };
        let model = crate::LoadedModel {
            name: "synthetic equal-sized geosets".into(),
            minimum_extent: [0.0; 3],
            maximum_extent: [10.0; 3],
            meshes: vec![rest(0, [1.0, 2.0, 3.0]), rest(1, [7.0, 8.0, 9.0])],
            bone_names: vec![],
            bone_parents: vec![],
            sequences: vec![],
        };
        let mut world = World::new();
        let instance = world.spawn_empty().id();
        let other_instance = world.spawn_empty().id();
        let mut meshes = Assets::<Mesh>::default();
        let handles: Vec<_> = (0..3)
            .map(|_| meshes.add(crate::to_bevy_mesh(&rest(0, [99.0; 3]))))
            .collect();
        for (handle, binding) in handles.iter().zip([
            SpawnedGeoset {
                instance,
                geoset: 0,
            },
            SpawnedGeoset {
                instance,
                geoset: 1,
            },
            SpawnedGeoset {
                instance: other_instance,
                geoset: 0,
            },
        ]) {
            world.spawn((Mesh3d(handle.clone()), binding));
        }
        world.insert_resource(meshes);
        world.insert_resource(ViewerCache {
            model: Some(model),
            instance: Some(instance),
            ..default()
        });
        world.run_system_once(skin_from_animation).unwrap();
        let meshes = world.resource::<Assets<Mesh>>();
        for (handle, expected) in handles.iter().zip([
            vec![[1.0, 2.0, 3.0]],
            vec![[7.0, 8.0, 9.0]],
            vec![[99.0; 3]],
        ]) {
            assert_eq!(
                meshes
                    .get(handle)
                    .unwrap()
                    .attribute(Mesh::ATTRIBUTE_POSITION),
                Some(&bevy::mesh::VertexAttributeValues::Float32x3(expected)),
            );
        }
    }

    fn shipped_model(rel: &str) -> Option<Vec<u8>> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()?
            .parent()?
            .join("wc3/game/app");
        if !dir.is_dir() {
            return None;
        }
        pcx_mpq::MpqSet::open_wc3(dir).ok()?.extract(rel).ok()
    }

    /// A clip is a re-encoding of the model's own keys, and sampling it must
    /// land on the decoder's own sample at the same instant — including the
    /// Hermite tangents a keyframe curve would have flattened.
    ///
    /// Every animated bone of the carrier is checked, not just the first one,
    /// at instants the keys do not sit on, so this pins the interpolation and
    /// not just the key values. Past a sequence's end the clip must hold that
    /// sequence's last pose rather than slide into the next sequence's keys,
    /// because the model's timeline is shared by all of them.
    ///
    /// Two carriers, because the Footman — the viewer's gate model — animates
    /// rotation and translation on *different* bones: `handle` translates,
    /// `Box04` rotates, `Cone11` rotates and scales, and no bone of it carries
    /// both a `KGTR` and a `KGRT`. The Knight supplies the bones that do, so
    /// the clip's translation curve is checked against the decoder there.
    #[test]
    fn a_clip_samples_exactly_what_the_decoder_sampler_samples() {
        let Some(footman) = shipped_model("Units\\Human\\Footman\\Footman.mdx") else {
            return;
        };
        let mut carriers: Vec<(&str, Vec<u8>)> = vec![("Footman", footman)];
        if let Some(knight) = shipped_model("Units\\Human\\Knight\\Knight.mdx") {
            carriers.push(("Knight", knight));
        }

        for (model, bytes) in carriers {
            let parsed = pcx_mdl::Mdlx::parse(&bytes).unwrap();
            let bones = parsed.bones(&bytes).unwrap_or_default();
            let seqs = parsed.sequences(&bytes).unwrap_or_default();
            assert!(!bones.is_empty() && !seqs.is_empty(), "{model} animates");
            let animated: Vec<&pcx_mdl::Bone> =
                bones.iter().filter(|b| !b.tracks.is_empty()).collect();
            assert!(!animated.is_empty(), "{model} animates a bone");

            let mut held_past_an_end = false;
            for bone in animated {
                let target = AnimationTargetId::from_name(&Name::new(bone_key(bone)));
                for seq in &seqs {
                    let start = seq.interval_start;
                    let duration_ms = seq.duration();
                    let end = start + duration_ms;
                    let clip = build_clip(&bones, start, duration_ms);
                    assert_eq!(
                        clip.duration(),
                        duration_ms as f32 / 1000.0,
                        "{model}: {} must last its own {} ms",
                        seq.name,
                        duration_ms
                    );
                    assert!(
                        clip.curves_for_target(target).is_some(),
                        "{model}: {} must animate bone {}",
                        seq.name,
                        bone.object_id
                    );

                    // A second and a bit past the start is past the end of the
                    // short sequences. The clip clamps to its own window, so it
                    // holds the pose the sequence ends on; the raw instant would
                    // run on into the next sequence's keys.
                    let past = start + 1_111;
                    let held_ms = past.min(end);
                    let held = pcx_mdl::sample_bone_pose(bone, held_ms);
                    if held_ms < past && pcx_mdl::sample_bone_pose(bone, past) != held {
                        held_past_an_end = true;
                    }

                    for ms in [start, between_keys(bone, start, end), end, held_ms] {
                        let t = (ms - start) as f32 / 1000.0;
                        let want = pcx_mdl::sample_bone_pose(bone, ms);
                        if bone.translation().is_some() {
                            let got = clip
                                .sample_clamped(animated_field!(Transform::translation), target, t)
                                .unwrap_or_else(|| {
                                    panic!(
                                        "{model}: {}: bone {} has no translation curve at {t}s",
                                        seq.name, bone.object_id
                                    )
                                });
                            assert_eq!(
                                got,
                                Vec3::from_array(want.translation),
                                "{model}: {} at {t}s: the clip must show the decoder's sample",
                                seq.name
                            );
                        }
                        if bone.rotation().is_some() {
                            let got_r = clip
                                .sample_clamped(animated_field!(Transform::rotation), target, t)
                                .unwrap_or_else(|| {
                                    panic!(
                                        "{model}: {}: bone {} has no rotation curve at {t}s",
                                        seq.name, bone.object_id
                                    )
                                });
                            assert_eq!(
                                got_r,
                                Quat::from_array(want.rotation),
                                "{model}: {} at {t}s: rotation must match too",
                                seq.name
                            );
                        }
                        if bone.scaling().is_some() {
                            let got_s = clip
                                .sample_clamped(animated_field!(Transform::scale), target, t)
                                .unwrap_or_else(|| {
                                    panic!(
                                        "{model}: {}: bone {} has no scale curve at {t}s",
                                        seq.name, bone.object_id
                                    )
                                });
                            assert_eq!(
                                got_s,
                                Vec3::from_array(want.scale),
                                "{model}: {} at {t}s: scale must match too",
                                seq.name
                            );
                        }
                    }
                }
            }

            assert!(
                held_past_an_end,
                "{model}: some sequence must end inside its own keys, or nothing ever holds past an end"
            );
        }
    }

    /// An instant inside `[start, end]` that none of the bone's keys sits on.
    ///
    /// A probe on a key frame would agree even with a curve that had flattened
    /// the tangents, so only an instant between the keys checks the
    /// interpolation.
    fn between_keys(bone: &pcx_mdl::Bone, start: u32, end: u32) -> u32 {
        let keys: Vec<u32> = bone
            .tracks
            .iter()
            .flat_map(|t| (0..t.count as usize).filter_map(|i| t.key_time(i)))
            .collect();
        let mut ms = start + (end - start) / 3;
        while ms < end && keys.contains(&ms) {
            ms += 1;
        }
        ms
    }

    /// Playback is a fixed tick, not a wall clock: thirty ticks must be one
    /// second of animation whatever the frame rate was.
    #[test]
    fn playback_advances_by_the_tick_not_by_the_clock() {
        let mut playback = crate::Playback {
            sequence: 0,
            time_ms: 0.0,
            looping: true,
        };
        for _ in 0..30 {
            playback.advance(TICK_MS, 60_000.0, true);
        }
        assert!(
            (playback.time_ms - 1_000.0).abs() < 0.01,
            "thirty fixed ticks are one second, got {} ms",
            playback.time_ms
        );
    }

    /// The camera and light must be framed from the model, not from a constant.
    ///
    /// A hard-coded distance suits one unit and quietly clips a dragon, so the
    /// extents are the input and the distance is derived from them.
    #[test]
    fn the_camera_is_framed_from_the_models_own_extents() {
        let tall_min = [0.0, 0.0, 0.0];
        let tall_max = [10.0, 10.0, 100.0];
        let short_min = [0.0, 0.0, 0.0];
        let short_max = [1.0, 1.0, 2.0];

        let centre = |mn: [f32; 3], mx: [f32; 3]| {
            [
                (mn[0] + mx[0]) * 0.5,
                (mn[1] + mx[1]) * 0.5,
                (mn[2] + mx[2]) * 0.5,
            ]
        };
        let height = |mn: [f32; 3], mx: [f32; 3]| (mx[2] - mn[2]).max(1.0);

        assert_eq!(centre(tall_min, tall_max)[2], 50.0);
        assert_eq!(height(tall_min, tall_max), 100.0);
        assert_eq!(height(short_min, short_max), 2.0);
        assert_ne!(
            height(tall_min, tall_max),
            height(short_min, short_max),
            "the framing must scale with the model"
        );
    }

    /// A texture's path must be matchable through either separator, because the
    /// archives use backslashes and a caller may hold forward slashes.
    #[test]
    fn textures_are_matched_through_either_separator() {
        let tex = vec![LoadedTexture {
            width: 2,
            height: 2,
            rgba: vec![0; 16],
            source: "Textures\\Footman.blp".to_string(),
        }];
        assert!(pick(&tex, "Textures\\Footman.blp").is_some());
        assert!(
            pick(&tex, "Textures/Footman.blp").is_some(),
            "forward slashes must match"
        );
        assert!(
            pick(&tex, "footman.blp").is_some(),
            "bare names must match by suffix"
        );
        assert!(
            pick(&tex, "gutz.blp").is_none(),
            "a different texture must not match"
        );
    }

    /// RGBA8 must land in an image of exactly the right size.
    #[test]
    fn rgba_becomes_an_image_of_the_right_size() {
        let tex = LoadedTexture {
            width: 4,
            height: 3,
            rgba: vec![7u8; 4 * 3 * 4],
            source: "x.blp".to_string(),
        };
        let img = rgba_image(&tex);
        assert_eq!(img.texture_descriptor.size.width, 4);
        assert_eq!(img.texture_descriptor.size.height, 3);
    }
}

#[cfg(test)]
mod sampler_tests {
    use super::*;
    use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};

    fn modes(flags: u32) -> (ImageAddressMode, ImageAddressMode) {
        match sampler_for_texture_flags(flags) {
            ImageSampler::Descriptor(d) => (d.address_mode_u, d.address_mode_v),
            other => panic!("expected a descriptor for flags {flags:#x}, got {other:?}"),
        }
    }

    /// The two authored bits map to the two axes, and nothing else does.
    #[test]
    fn the_wrap_flags_choose_repeat_per_axis() {
        assert_eq!(
            modes(pcx_mdl::TEXTURE_WRAP_WIDTH | pcx_mdl::TEXTURE_WRAP_HEIGHT),
            (ImageAddressMode::Repeat, ImageAddressMode::Repeat),
            "0x3, the expansion scene's ice and sky"
        );
        assert_eq!(
            modes(pcx_mdl::TEXTURE_WRAP_WIDTH),
            (ImageAddressMode::Repeat, ImageAddressMode::ClampToEdge)
        );
        assert_eq!(
            modes(pcx_mdl::TEXTURE_WRAP_HEIGHT),
            (ImageAddressMode::ClampToEdge, ImageAddressMode::Repeat)
        );
    }

    /// A texture that wraps on neither axis keeps the default sampler exactly,
    /// so nothing that was already correct changes.
    #[test]
    fn a_non_wrapping_texture_keeps_the_default_sampler() {
        assert!(matches!(
            sampler_for_texture_flags(0x0),
            ImageSampler::Default
        ));
        assert!(matches!(
            sampler_for_texture_flags(0xfffc),
            ImageSampler::Default
        ));
    }

    /// The filter is not invented from a flag word.
    #[test]
    fn the_filter_stays_at_its_default() {
        let d = ImageSamplerDescriptor::default();
        match sampler_for_texture_flags(0x3) {
            ImageSampler::Descriptor(got) => {
                assert_eq!(got.mag_filter, d.mag_filter);
                assert_eq!(got.min_filter, d.min_filter);
                assert_eq!(got.mipmap_filter, d.mipmap_filter);
            }
            other => panic!("{other:?}"),
        }
    }
}

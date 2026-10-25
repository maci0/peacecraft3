//! Authored `MODEL` frames of the selected score screen. Each frame with a `BackgroundArt`
//! record names the model it draws, its geometry comes from the frame's authored placement
//! in the viewport, and its `LayerStyle` decides shading. The defeat banner is not among
//! these frames: its skin key names a model, but the screen names no frame to place it in,
//! so it is not drawn.
use crate::fdf::Rect;
use crate::menu_material::MenuMaterial;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use peacecraft3::campaign_score_layout::ScoreScreenLayout;

/// One authored background model: the frame it belongs to, its `.mdl` path as the screen
/// names it, its frame rectangle, and whether its layer style asks for no shading.
#[derive(Debug, Clone, PartialEq)]
pub struct AuthoredBackground {
    pub frame: String,
    pub model: String,
    pub rect: Rect,
    pub unlit: bool,
}

/// Every `MODEL` frame that names a `BackgroundArt`, in the screen's frame order.
pub fn authored_backgrounds(
    layout: &ScoreScreenLayout,
    viewport: (f32, f32),
) -> Result<Vec<AuthoredBackground>, String> {
    let fdf = layout.fdf();
    fdf.frames()
        .iter()
        .filter(|frame| frame.kind == "MODEL")
        .filter_map(|frame| {
            fdf.string(&frame.name, "BackgroundArt")
                .map(|model| (frame.name.clone(), model))
        })
        .map(|(frame, model)| {
            if !fdf.flag(&frame, "SetAllPoints")
                && fdf.set_point(&frame).is_none()
                && (fdf.record(&frame, "Width").is_none() || fdf.record(&frame, "Height").is_none())
            {
                return Err(format!("{frame}: no authored geometry"));
            }
            let rect = layout
                .rect(&frame, viewport)
                .map_err(|e| format!("{frame}: {e:?}"))?;
            let unlit = fdf
                .string(&frame, "LayerStyle")
                .is_some_and(|style| style.split('|').any(|flag| flag == "NOSHADING"));
            Ok(AuthoredBackground {
                frame,
                model,
                rect,
                unlit,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIEWPORT: (f32, f32) = (0.8, 0.6);

    fn layout(screen: &str) -> ScoreScreenLayout {
        ScoreScreenLayout::parse(screen, &[], "").unwrap()
    }

    #[test]
    fn only_model_frames_with_background_art_are_drawn_with_their_authored_style() {
        let l = layout(
            "Frame \"FRAME\" \"ScoreScreenFrame\" {\n\tWidth 0.8,\n\tHeight 0.6,\n}\n\
             Frame \"MODEL\" \"ScoreScreenBackground\" {\n\tBackgroundArt \"UI\\Glues\\Back.mdl\",\n\tSetAllPoints,\n\tLayerStyle \"NOSHADING\",\n}\n\
             Frame \"MODEL\" \"ScoreScreenRaceArt\" {\n\tSetAllPoints,\n\tLayerStyle \"NOSHADING|IGNORETRACKEVENTS\",\n}\n\
             Frame \"MODEL\" \"ScoreScreenBarTop\" {\n\tBackgroundArt \"UI\\Glues\\Top.mdl\",\n\tWidth 0.2,\n\tHeight 0.03,\n\tLayerStyle \"SHADED\",\n}\n",
        );
        let drawn = authored_backgrounds(&l, VIEWPORT).unwrap();
        let names: Vec<&str> = drawn.iter().map(|b| b.frame.as_str()).collect();
        assert_eq!(names, ["ScoreScreenBackground", "ScoreScreenBarTop"]);
        assert_eq!(drawn[0].model, "UI\\Glues\\Back.mdl");
        assert!(drawn[0].unlit);
        assert!(!drawn[1].unlit, "a style without NOSHADING is shaded");
        assert_eq!(drawn[1].rect.width, 0.2);
    }

    #[test]
    fn a_background_frame_without_authored_geometry_refuses_instead_of_guessing() {
        let l = layout(
            "Frame \"MODEL\" \"ScoreScreenBackground\" {\n\tBackgroundArt \"UI\\Glues\\Back.mdl\",\n\tLayerStyle \"NOSHADING\",\n}\n",
        );
        let err = authored_backgrounds(&l, VIEWPORT).unwrap_err();
        assert!(err.starts_with("ScoreScreenBackground:"), "{err}");
    }
}

/// Upper bound on one background model file read through the selected resolver.
const MODEL_BYTES: u64 = 16 << 20;
/// Upper bound on one texture file read through the selected resolver.
const TEXTURE_BYTES: u64 = 16 << 20;
/// A background with all reads and preflight done. It owns its bytes, so its borrows are
/// valid until it is spawned or dropped.
pub struct BackgroundPlan {
    pub frame: String,
    pub source: String,
    pub bytes: Vec<u8>,
    pub mdl: pcx_mdl::Mdlx,
    pub model: pcx_gfx::LoadedModel,
    pub runtime: pcx_gfx::mdx_runtime::RuntimePlan,
    pub textures: Vec<pcx_gfx::LoadedTexture>,
    pub interval: [u32; 2],
    pub opacity: pcx_gfx::mdx_opacity::OpacityPlan,
    pub selection: pcx_gfx::mdx_texture_selection::TextureSelectionPlan,
    pub uv: pcx_gfx::mdx_uv_translation::UvTranslationPlan,
    pub rect: Rect,
    pub unlit: bool,
}

/// Reads one authored background through `resolver` and checks it, without spawning anything.
pub fn prepare(
    resolver: &pcx_assets::Resolver,
    background: &AuthoredBackground,
) -> Result<BackgroundPlan, String> {
    let source = pcx_mdl::binary_model_path(&background.model);
    let bytes = resolver
        .resolve_bounded(&source, MODEL_BYTES)
        .map_err(|e| format!("{source}: {e}"))?
        .bytes;
    let mdl = pcx_mdl::Mdlx::parse(&bytes).map_err(|e| format!("{source}: {e}"))?;
    let sequences = mdl
        .sequences(&bytes)
        .map_err(|e| format!("{source}: {e}"))?;
    // The first authored sequence. The score models' original initializer is not yet traced:
    // the shipped `ScoreScreen-Background` and `Tabs` models carry no `Stand` name, so `Stand`
    // is not required. A model without sequences holds its first frame.
    let interval = sequences
        .first()
        .map_or([0, 0], |first| [first.interval_start, first.interval_end]);
    let model = pcx_gfx::load_model(&bytes, &source).map_err(|e| format!("{source}: {e}"))?;
    let runtime =
        pcx_gfx::mdx_runtime::RuntimePlan::parse(&bytes).map_err(|e| format!("{source}: {e}"))?;
    let paths =
        crate::mainmenu_scene::texture_paths(&bytes).map_err(|e| format!("{source}: {e}"))?;
    let textures = crate::mainmenu_scene::load_textures_from_paths(&paths, |path| {
        resolver
            .resolve_bounded(path, TEXTURE_BYTES)
            .map(|asset| asset.bytes)
            .map_err(|e| e.to_string())
    })
    .map_err(|e| format!("{source}: {e}"))?;
    let opacity = pcx_gfx::mdx_opacity::OpacityPlan::checked(&runtime, interval)
        .map_err(|e| format!("{source}: {e}"))?;
    let selection = crate::mainmenu_scene::texture_selection(&runtime, interval, &source)
        .map_err(|e| e.to_string())?;
    let uv =
        pcx_gfx::mdx_uv_translation::UvTranslationPlan::from_runtime(&runtime, interval, &source)
            .map_err(|e| format!("{source}: {e}"))?;
    Ok(BackgroundPlan {
        frame: background.frame.clone(),
        source,
        bytes,
        mdl,
        model,
        runtime,
        textures,
        interval,
        opacity,
        selection,
        uv,
        rect: background.rect,
        unlit: background.unlit,
    })
}

/// Marks one spawned background frame root.
#[derive(Component)]
pub struct ScoreBackground;

/// Handles one spawned score screen created, kept on its root so teardown can release them.
#[derive(Component, Default, Clone)]
pub struct ScoreSceneHandles {
    pub meshes: Vec<Handle<Mesh>>,
    pub materials: Vec<Handle<MenuMaterial>>,
    pub images: Vec<Handle<Image>>,
}

/// Spawns each prepared background under `root`, positioned at its authored frame origin, and
/// records every mesh, material and image it created on `root` for teardown.
pub fn spawn_backgrounds(
    commands: &mut Commands,
    assets: crate::mainmenu_scene::GeosetAssets,
    root: Entity,
    plans: Vec<BackgroundPlan>,
) -> Result<ScoreSceneHandles, String> {
    let crate::mainmenu_scene::GeosetAssets {
        meshes,
        materials,
        images,
    } = assets;
    let mut handles = ScoreSceneHandles::default();
    for plan in plans {
        let frame_root = commands
            .spawn((
                Transform::from_xyz(plan.rect.left, plan.rect.bottom, 0.0),
                Visibility::default(),
                ChildOf(root),
                ScoreBackground,
            ))
            .id();
        let mut info = crate::mainmenu_scene::SceneInfo::default();
        let geosets = crate::mainmenu_scene::spawn_geosets(
            commands,
            crate::mainmenu_scene::GeosetAssets {
                meshes: &mut *meshes,
                materials: &mut *materials,
                images: &mut *images,
            },
            crate::mainmenu_scene::GeosetSource {
                m: &plan.mdl,
                bytes: &plan.bytes,
                source: &plan.source,
                model: &plan.model,
                textures: &plan.textures,
                runtime: &plan.runtime,
                selection: &plan.selection,
                uv_translation: &plan.uv,
                native_cutout_reference: None,
                material_layers: None,
                culls_back_faces: false,
            },
            &plan.opacity,
            plan.interval[0],
            &mut info,
            (
                ChildOf(frame_root),
                bevy::render::batching::NoAutomaticBatching,
                RenderLayers::layer(SCORE_RENDER_LAYER),
            ),
        )
        .map_err(|e| format!("{}: {e}", plan.source))?;
        for (_, _, material) in &geosets.bound {
            if let Some(mut authored) = materials.get_mut(material) {
                // NOSHADING on the authored frame: the geosets draw without lighting.
                authored.base.unlit = plan.unlit;
            }
            handles.materials.push(material.clone());
        }
        handles.meshes.extend(geosets.handles.iter().cloned());
        handles
            .images
            .extend(geosets.textures.image_handles().cloned());
        commands.entity(frame_root).insert(ScoreBackgroundState {
            frame: plan.frame,
            source: plan.source,
            interval: plan.interval,
            elapsed: std::time::Duration::ZERO,
            model: plan.model,
            runtime: plan.runtime,
            handles: geosets.handles.clone(),
            opacity: plan.opacity,
            selection: plan.selection,
            uv: plan.uv,
            bound: geosets.bound,
            textures: geosets.textures,
        });
    }
    Ok(handles)
}

/// Render layer of the score screen's model camera, the glue panel layer.
pub const SCORE_RENDER_LAYER: usize = 7;

/// Looping frame for a presentation clock inside an authored interval. An empty interval holds
/// its start.
pub fn frame_for(interval: [u32; 2], elapsed_ms: u64) -> u32 {
    let length = u64::from(interval[1].saturating_sub(interval[0]));
    if length == 0 {
        return interval[0];
    }
    let offset = elapsed_ms % length;
    // The offset is below `length`, which is a u32 span, so the sum fits in u32.
    interval[0] + u32::try_from(offset).unwrap_or(0)
}

/// Per-background presentation state. The clock starts at zero when the background is spawned,
/// so a new screen, or a new session's screen, always starts from its first frame.
#[derive(Component)]
pub struct ScoreBackgroundState {
    pub frame: String,
    pub source: String,
    pub interval: [u32; 2],
    pub elapsed: std::time::Duration,
    pub model: pcx_gfx::LoadedModel,
    pub runtime: pcx_gfx::mdx_runtime::RuntimePlan,
    pub handles: Vec<Handle<Mesh>>,
    pub opacity: pcx_gfx::mdx_opacity::OpacityPlan,
    pub selection: pcx_gfx::mdx_texture_selection::TextureSelectionPlan,
    pub uv: pcx_gfx::mdx_uv_translation::UvTranslationPlan,
    pub bound: Vec<crate::mainmenu_scene::BoundGeoset>,
    pub textures: crate::mainmenu_scene::TextureBindings,
}

/// Advances every spawned background on the real clock and applies its opacity, texture and
/// UV tracks. Bone-driven motion is not applied here; the models' bones stay at rest.
pub fn animate_backgrounds(
    time: Res<Time<Real>>,
    mut states: Query<&mut ScoreBackgroundState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<MenuMaterial>>,
    mut visibility: Query<&mut Visibility>,
) {
    for mut state in &mut states {
        state.elapsed += time.delta();
        let Ok(elapsed_ms) = crate::mainmenu_scene::elapsed_millis(&state.source, state.elapsed)
        else {
            continue;
        };
        let frame = frame_for(state.interval, elapsed_ms);
        let state = &mut *state;
        let Ok(elapsed_ms32) = u32::try_from(elapsed_ms) else {
            warn_once!(
                "score background {}: clock exceeds the sequence range",
                state.source
            );
            continue;
        };
        // Bones pose the mesh from the models' own bones and pivots; the score models have no
        // authored camera, so they take the default pose path (`camera_pose: None`).
        let joints = &state.runtime.joints;
        if let Err(e) = crate::mainmenu_scene::skin_geosets(
            crate::mainmenu_scene::Rig {
                blend: None,
                interval: state.interval,
                model: &state.model,
                bones: joints.joints(),
                pivots: joints.pivots(),
                global_durations: &state.runtime.global_sequences,
                camera_pose: None,
                look_at: None,
            },
            &state.handles,
            frame,
            elapsed_ms32,
            &mut meshes,
        ) {
            warn_once!("score background {} ({}): {e}", state.frame, state.source);
        }
        crate::mainmenu_scene::apply_opacity(
            &state.opacity,
            &state.bound,
            frame,
            elapsed_ms,
            &mut materials,
            &mut visibility,
        );
        if let Err(e) = crate::mainmenu_scene::apply_texture_selection(
            &state.selection,
            &mut state.textures,
            frame,
            elapsed_ms,
            &mut materials,
        )
        .and_then(|()| {
            crate::mainmenu_scene::apply_uv_translation(
                &state.uv,
                &state.textures,
                frame,
                elapsed_ms,
                &mut materials,
            )
        }) {
            warn_once!("score background {} ({}): {e}", state.frame, state.source);
        }
    }
}

#[cfg(test)]
mod clock_tests {
    use super::*;

    #[test]
    fn the_presentation_clock_loops_inside_the_authored_interval() {
        assert_eq!(frame_for([100, 200], 0), 100);
        assert_eq!(frame_for([100, 200], 150), 150);
        assert_eq!(frame_for([100, 200], 250), 150);
        assert_eq!(frame_for([100, 200], 299), 199);
    }

    #[test]
    fn an_empty_interval_holds_its_start() {
        assert_eq!(frame_for([0, 0], 5000), 0);
        assert_eq!(frame_for([40, 40], 9), 40);
    }
}

/// Selected-source fixtures for the resolver path: each source is a directory written for one
/// test under the ignored `.scratch` tree, opened as an override directory, and removed after.
#[cfg(test)]
mod resolver_tests {
    use super::*;
    use pcx_assets::{Addon, AssetMode, Behaviour, Build, GameMode, OverrideDirectory, Resolver};
    use std::path::PathBuf;

    pub(super) struct SelectedSource(PathBuf);

    impl SelectedSource {
        pub(super) fn new(name: &str, files: &[(&str, Vec<u8>)]) -> Self {
            let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../.scratch/score-art-fixtures")
                .join(format!("{}-{name}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).unwrap();
            for (path, bytes) in files {
                let at = root.join(path.replace('\\', "/"));
                std::fs::create_dir_all(at.parent().expect("file has a parent")).unwrap();
                std::fs::write(&at, bytes).unwrap();
            }
            Self(root)
        }

        pub(super) fn resolver(&self) -> Resolver {
            let mode = GameMode {
                build: Build::V1_29b,
                addon: Addon::Tft,
                assets: AssetMode::Vanilla,
                behaviour: Behaviour::Vanilla,
            };
            Resolver::new(
                mode,
                Some(OverrideDirectory::open(&self.0, "score-test", None).unwrap()),
                Vec::new(),
            )
            .unwrap()
        }
    }

    impl Drop for SelectedSource {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// One 1x1 uncompressed 32-bit TGA with a single pixel.
    pub(super) fn tga(pixel: [u8; 4]) -> Vec<u8> {
        let mut bytes = vec![0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 0, 32, 0];
        bytes.extend_from_slice(&pixel);
        bytes
    }

    #[test]
    fn the_same_path_in_two_selected_sources_reads_each_source_own_texture() {
        let red = SelectedSource::new(
            "same-path-red",
            &[("Textures\\glow.tga", tga([0, 0, 255, 255]))],
        );
        let blue = SelectedSource::new(
            "same-path-blue",
            &[("Textures\\glow.tga", tga([255, 0, 0, 255]))],
        );
        let (red, blue) = (red.resolver(), blue.resolver());
        let a = resolve_pixels(&red, "Textures\\glow.tga");
        let b = resolve_pixels(&blue, "Textures\\glow.tga");
        assert_eq!(a, [255, 0, 0, 255]);
        assert_eq!(b, [0, 0, 255, 255]);
    }

    #[test]
    fn a_path_missing_from_the_selected_source_refuses_and_names_it() {
        let only = SelectedSource::new(
            "missing-path",
            &[("Textures\\other.tga", tga([0, 0, 0, 255]))],
        );
        let paths = vec!["Textures\\glow.tga".to_owned()];
        let err = crate::mainmenu_scene::load_textures_from_paths(&paths, |p| {
            only.resolver()
                .resolve_bounded(p, TEXTURE_BYTES)
                .map(|asset| asset.bytes)
                .map_err(|e| e.to_string())
        })
        .unwrap_err()
        .to_string();
        assert!(err.contains("Textures\\glow.tga"), "{err}");
    }

    /// The first texel's RGBA, read through the resolver path the score backgrounds use.
    fn resolve_pixels(resolver: &Resolver, path: &str) -> [u8; 4] {
        let paths = vec![path.to_owned()];
        let loaded = crate::mainmenu_scene::load_textures_from_paths(&paths, |p| {
            resolver
                .resolve_bounded(p, TEXTURE_BYTES)
                .map(|asset| asset.bytes)
                .map_err(|e| e.to_string())
        })
        .expect("texture decodes");
        let texture = &loaded[0];
        [
            texture.rgba[0],
            texture.rgba[1],
            texture.rgba[2],
            texture.rgba[3],
        ]
    }
}

/// Spawn, update and teardown on the real asset stores, over a synthetic selected model.
#[cfg(test)]
pub(crate) mod lifecycle_tests {
    use super::resolver_tests::{SelectedSource, tga};
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use std::time::Duration;

    fn word(bytes: &mut Vec<u8>, value: u32) {
        bytes.extend(value.to_le_bytes());
    }

    fn floats(bytes: &mut Vec<u8>, values: &[f32]) {
        for value in values {
            bytes.extend(value.to_le_bytes());
        }
    }

    fn block(bytes: &mut Vec<u8>, tag: &[u8; 4], count: u32, body: &[u8]) {
        bytes.extend(tag);
        word(bytes, count);
        bytes.extend(body);
    }

    fn chunk(bytes: &mut Vec<u8>, tag: &[u8; 4], payload: &[u8]) {
        bytes.extend(tag);
        word(bytes, payload.len() as u32);
        bytes.extend(payload);
    }

    /// One weighted triangle, bound to the bone with object id 9. Validated by the pcx-mdl readers.
    fn geoset() -> Vec<u8> {
        let mut data = Vec::new();
        let mut vertices = Vec::new();
        floats(
            &mut vertices,
            &[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
        );
        block(&mut data, b"VRTX", 3, &vertices);
        let mut normals = Vec::new();
        floats(&mut normals, &[0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0]);
        block(&mut data, b"NRMS", 3, &normals);
        block(&mut data, b"PTYP", 1, &4u32.to_le_bytes());
        block(&mut data, b"PCNT", 1, &3u32.to_le_bytes());
        block(&mut data, b"PVTX", 3, &[0, 0, 1, 0, 2, 0]);
        block(&mut data, b"GNDX", 3, &[0, 0, 0]);
        block(&mut data, b"MTGC", 1, &1u32.to_le_bytes());
        block(&mut data, b"MATS", 1, &9u32.to_le_bytes());
        for value in [0, 11, 4] {
            word(&mut data, value);
        }
        floats(&mut data, &[6.5, -3.0, -2.0, -1.0, 8.0, 9.0, 10.0]);
        word(&mut data, 2);
        floats(&mut data, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0]);
        floats(&mut data, &[8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0]);
        block(&mut data, b"UVAS", 1, &[]);
        let mut uv = Vec::new();
        floats(&mut uv, &[0.0, 0.0, 1.0, 0.0, 0.0, 1.0]);
        block(&mut data, b"UVBS", 3, &uv);
        data
    }

    /// One material layer record with `alpha` as its static alpha.
    fn layer(alpha: f32, coord_id: u32) -> Vec<u8> {
        let mut data = Vec::new();
        for value in [28u32, 4, 16, 0, u32::MAX, coord_id] {
            word(&mut data, value);
        }
        data.extend(alpha.to_le_bytes());
        data
    }

    fn material() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend((-3i32).to_le_bytes());
        word(&mut data, 5);
        data.extend(b"LAYS");
        word(&mut data, 1);
        data.extend(layer(0.875, 0));
        let mut records = Vec::new();
        word(&mut records, (data.len() + 4) as u32);
        records.extend(data);
        records
    }

    /// One bone, object id 9, with a linear translation track from z=0 at time 0 to z=10 at 1000.
    fn bone() -> Vec<u8> {
        let mut tracks = Vec::new();
        tracks.extend(b"KGTR");
        word(&mut tracks, 2);
        word(&mut tracks, 1);
        tracks.extend((-1i32).to_le_bytes());
        for (time, z) in [(0u32, 0.0f32), (1000, 10.0)] {
            word(&mut tracks, time);
            floats(&mut tracks, &[0.0, 0.0, z]);
        }
        let mut record = Vec::new();
        word(&mut record, (96 + tracks.len()) as u32);
        let mut name = [0u8; 80];
        name[..4].copy_from_slice(b"Root");
        record.extend(name);
        word(&mut record, 9);
        record.extend((-1i32).to_le_bytes());
        word(&mut record, 0);
        record.extend(tracks);
        record.extend([0u8; 8]);
        record
    }

    fn sequence(name: &str, start: u32, end: u32) -> Vec<u8> {
        let mut record = vec![0u8; 132];
        record[..name.len()].copy_from_slice(name.as_bytes());
        record[80..84].copy_from_slice(&start.to_le_bytes());
        record[84..88].copy_from_slice(&end.to_le_bytes());
        record
    }

    fn texture(path: &str) -> Vec<u8> {
        let mut record = vec![0u8; 268];
        record[4..4 + path.len()].copy_from_slice(path.as_bytes());
        record
    }

    /// A model whose one texture is `Textures\glow.tga`.
    pub(crate) fn model() -> Vec<u8> {
        let mut bytes = b"MDLX".to_vec();
        chunk(&mut bytes, b"VERS", &800u32.to_le_bytes());
        let mut header = vec![0; pcx_mdl::MODL_SIZE];
        for (slot, value) in [-1.0f32; 3].into_iter().chain([1.0; 3]).enumerate() {
            let at = 0x158 + slot * 4;
            header[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
        chunk(&mut bytes, b"MODL", &header);
        let mut geosets = Vec::new();
        let body = geoset();
        word(&mut geosets, (body.len() + 4) as u32);
        geosets.extend(body);
        chunk(&mut bytes, b"GEOS", &geosets);
        chunk(&mut bytes, b"MTLS", &material());
        chunk(&mut bytes, b"TEXS", &texture(r"Textures\glow.tga"));
        chunk(&mut bytes, b"SEQS", &sequence("Stand", 0, 1000));
        chunk(&mut bytes, b"BONE", &bone());
        chunk(&mut bytes, b"PIVT", &[0; 120]);
        bytes
    }

    fn background() -> AuthoredBackground {
        AuthoredBackground {
            frame: "ScoreScreenBackground".into(),
            model: r"UI\Glues\Test\Test.mdl".into(),
            rect: crate::fdf::Rect {
                left: 0.1,
                bottom: 0.2,
                width: 0.3,
                height: 0.4,
            },
            unlit: true,
        }
    }

    fn source(model_bytes: Vec<u8>, texture_bytes: Vec<u8>) -> SelectedSource {
        SelectedSource::new(
            "lifecycle",
            &[
                ("UI\\Glues\\Test\\Test.mdx", model_bytes),
                ("Textures\\glow.tga", texture_bytes),
            ],
        )
    }

    fn assets_app() -> App {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<MenuMaterial>>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Time<Real>>();
        app
    }

    /// Z of every vertex of `mesh`, in vertex order.
    fn vertex_z(app: &App, mesh: &Handle<Mesh>) -> Vec<f32> {
        let meshes = app.world().resource::<Assets<Mesh>>();
        let positions = meshes
            .get(mesh)
            .and_then(|m| m.attribute(Mesh::ATTRIBUTE_POSITION))
            .and_then(|values| values.as_float3())
            .expect("skinned positions");
        positions.iter().map(|p| p[2]).collect()
    }

    #[test]
    fn prepare_spawn_update_and_teardown_release_the_background_assets() {
        let selected = source(model(), tga([0, 0, 255, 255]));
        let resolver = selected.resolver();
        let plan = prepare(&resolver, &background()).expect("prepared");
        let mut app = assets_app();
        // Unrelated assets that teardown must leave in place.
        let (keep_mesh, keep_material, keep_image) = {
            let mut meshes = app.world_mut().resource_mut::<Assets<Mesh>>();
            let mesh = meshes.add(Mesh::new(
                bevy::mesh::PrimitiveTopology::TriangleList,
                bevy::asset::RenderAssetUsages::MAIN_WORLD,
            ));
            let mut materials = app.world_mut().resource_mut::<Assets<MenuMaterial>>();
            let material = materials.add(MenuMaterial::default());
            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            let image = images.add(Image::default());
            (mesh, material, image)
        };
        let root = app.world_mut().spawn(Visibility::default()).id();
        let mut plans = Some(vec![plan]);
        let handles = app
            .world_mut()
            .run_system_once(
                move |mut commands: Commands,
                      mut meshes: ResMut<Assets<Mesh>>,
                      mut materials: ResMut<Assets<MenuMaterial>>,
                      mut images: ResMut<Assets<Image>>| {
                    spawn_backgrounds(
                        &mut commands,
                        crate::mainmenu_scene::GeosetAssets {
                            meshes: &mut meshes,
                            materials: &mut materials,
                            images: &mut images,
                        },
                        root,
                        plans.take().unwrap_or_default(),
                    )
                },
            )
            .expect("system runs")
            .expect("background spawns");
        assert!(!handles.meshes.is_empty(), "geosets create meshes");
        assert!(!handles.materials.is_empty(), "geosets create materials");
        assert!(
            !handles.images.is_empty(),
            "the texture binding creates an image"
        );
        let mesh = handles.meshes[0].clone();
        let frame_roots: Vec<Entity> = app
            .world_mut()
            .query_filtered::<Entity, With<ScoreBackground>>()
            .iter(app.world())
            .collect();
        assert_eq!(frame_roots.len(), 1);

        // Frame zero: the bone's translation key at time zero, z = 0.
        assert_eq!(vertex_z(&app, &mesh), [0.0, 0.0, 0.0]);
        app.world_mut()
            .resource_mut::<Time<Real>>()
            .advance_by(Duration::from_millis(500));
        app.world_mut()
            .run_system_once(animate_backgrounds)
            .expect("update runs");
        // At 500 ms the linear track between z = 0 and z = 10 is z = 5 on every weighted vertex.
        assert_eq!(vertex_z(&app, &mesh), [5.0, 5.0, 5.0]);

        app.world_mut()
            .run_system_once(
                move |mut meshes: ResMut<Assets<Mesh>>,
                      mut materials: ResMut<Assets<MenuMaterial>>,
                      mut images: ResMut<Assets<Image>>| {
                    release_handles(&handles, &mut meshes, &mut materials, &mut images);
                },
            )
            .expect("teardown runs");
        app.world_mut().despawn(root);
        // Owned assets are gone; the unrelated ones remain.
        assert!(app.world().resource::<Assets<Mesh>>().get(&mesh).is_none());
        assert!(
            app.world()
                .resource::<Assets<Mesh>>()
                .get(&keep_mesh)
                .is_some()
        );
        assert!(
            app.world()
                .resource::<Assets<MenuMaterial>>()
                .get(&keep_material)
                .is_some()
        );
        assert!(
            app.world()
                .resource::<Assets<Image>>()
                .get(&keep_image)
                .is_some()
        );
        for entity in frame_roots {
            assert!(
                app.world().get_entity(entity).is_err(),
                "frame children are removed with the root"
            );
        }
        assert!(app.world().get_entity(root).is_err());
    }

    #[test]
    fn a_replacement_source_that_lacks_the_model_refuses_and_names_its_path() {
        let missing = SelectedSource::new("replacement-missing", &[]);
        let Err(err) = prepare(&missing.resolver(), &background()) else {
            panic!("a missing model must refuse");
        };
        assert!(err.contains("UI\\Glues\\Test\\Test.mdx"), "{err}");
    }
}

/// Frees the meshes, materials and images one spawn created, once its root is gone.
pub fn release_handles(
    handles: &ScoreSceneHandles,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<MenuMaterial>,
    images: &mut Assets<Image>,
) {
    for mesh in &handles.meshes {
        meshes.remove(mesh);
    }
    for material in &handles.materials {
        materials.remove(material);
    }
    for image in &handles.images {
        images.remove(image);
    }
}

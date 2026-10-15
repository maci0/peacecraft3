//! The original `MainMenu3d` animated background, from the shipped model.
//!
//! Everything here reads the model the game ships at
//! `UI\Glues\MainMenu\MainMenu3d\MainMenu3d.mdx` — its own geometry, its own
//! camera, its own lights, its own texture list and its own `Stand` sequence.
//! Nothing is substituted: a piece this module cannot read is reported as a
//! gap, never replaced with an invented effect.
//!
//! ## What the model actually is (measured)
//!
//! ```text
//! chunks   BONE CAMS EVTS GEOA GEOS GLBS HELP LITE MODL MTLS PIVT PRE2 SEQS TEXS TXAN VERS
//! GEOS     33 geosets, 9,307 vertices
//! MTLS     31 materials
//! TEXS     33 textures, one `Stand` clip
//! SEQS     Stand 0..16667 (16,667 ms, looping), Stand 19833..36500 (loop),
//!          Stand Alternate 36933..53600 (loop), Morph Alternate 67700..69767
//! ```
//!
//! The entry clip is the first `Stand`, `0..16667` ms, looping — that is what
//! [`SceneInfo::duration_ms`] reports and what the advance system plays.
//!
//! ## Known fidelity gaps, stated rather than papered over
//!
//! * **`PRE2` particle emitters are not drawn.** The chunk is decoded to its
//!   properties by `pcx-mdl`, but their native rendering is not implemented.
//!   No stand-in particle effect is created.
//! * **First-layer `TXAN` translation only.** Checked KTAT tracks drive absolute
//!   material UV transforms; rotation, scale and multiple UV sets refuse.
//!   XY projection follows retained renderer references, not native runtime
//!   proof; see `menu-texture-animation.md` for the source/evidence boundary.
//!   GEOA/KMTA opacity is applied by authored geoset/layer IDs and clocks.
//!   Joint tracks also use authored `GLBS` durations on a separate elapsed clock.
//!   First-layer KMTF texture selection binds authored TEXS images and samplers
//!   at spawn and advances on the selected local clip or independent elapsed clock.
//! * **One texture layer per material.** Each of the scene's 31 materials
//!   carries exactly one `LAYS` layer, so this costs nothing here, but a model
//!   with a multi-layer material (the format allows several, drawn over each
//!   other) would only have its first drawn. The counts are asserted in the
//!   tests, so a model that changes that fails rather than silently loses a
//!   layer.
//! * **Back-face culling is off, because which flags bit means `TwoSided` is
//!   not established here.** The scene's layers carry `0x00`, `0x10`, `0x21`,
//!   `0x90` and `0xA1`; none of those is proven to be the two-sided bit, and
//!   guessing would make a geoset disappear entirely rather than merely shade
//!   differently. Nothing is culled until that bit is measured.
//! * **`unlit` and the blend are the layer's own**, read from its `filterMode`
//!   and `shadingFlags` words. Three of the format's seven modes are not
//!   reproduced exactly, and each is reported in [`SceneInfo::blend_gaps`] with
//!   the reason rather than being silently approximated:
//!   * `filterMode 1` (`Transparent`) is an **alpha test** — opaque or gone,
//!     never partly there. Bevy's `AlphaMode::Mask(t)` is the equivalent and `t`
//!     is a game constant this workspace has not measured, so the layer draws
//!     blended: a hard edge becomes a soft one.
//!   * `filterMode 3` (`Additive`, `ONE/ONE`) differs from Bevy's
//!     `AlphaMode::Add`, which premultiplies by alpha (`SRC_ALPHA/ONE`). The two
//!     agree only at alpha 1. `filterMode 4` *is* Bevy's `Add` exactly, so only
//!     `3` is flagged.
//!   * `filterMode 6` (`Modulate2x`) has no equivalent and draws opaque.
//!
//!   `filterMode 5` (`Modulate`) maps to `AlphaMode::Multiply`, which Bevy does
//!   have (`BLEND_MULTIPLY`); no material in this scene uses it.
//! * **Animated light volumes are placed statically.** `LITE` records carry
//!   node tracks this module does not sample, so each light sits where its
//!   record's default puts it.
//! * Bevy's own camera is used rather than a hand-rolled matrix, so the
//!   projection is Bevy's; the eye, target, stored FOV and clip planes come from
//!   the model. Native FOV-axis/projection equivalence is not established here.

use bevy::prelude::*;

/// The model this module renders.
pub const SCENE_ART: &str = r"UI\Glues\MainMenu\MainMenu3d\MainMenu3d.mdx";

/// The served name of the model itself.
///
/// The scene needs its geometry, not only its textures, and a browser cannot
/// read the install; `web/loader.js` registers the extracted model under this
/// name and the launcher reads it back with
/// `mainmenu::art_bytes_named(install, SCENE_ART, MODEL_SERVED_NAME)`.
pub const MODEL_SERVED_NAME: &str = "menu3d/mainmenu3d.mdx";

/// The subdirectory the scene's textures are served under in a browser.
///
/// A browser cannot read the install, so the 33 textures the model names are
/// extracted next to the page and registered under
/// `menu3d/<lowercased file name>`. [`texture_served_name_in`] is the single place
/// that mapping is defined, and `web/loader.js` mirrors it.
pub const TEXTURE_DIR: &str = "menu3d";

/// The expansion scene and its browser namespace, separate from RoC's textures.
pub fn scene_source(expansion: bool) -> (&'static str, &'static str, &'static str) {
    if expansion {
        (
            r"UI\Glues\MainMenu\MainMenu3D_Exp\MainMenu3D_Exp.mdx",
            "menu3d-exp/mainmenu3d_exp.mdx",
            "menu3d-exp",
        )
    } else {
        (SCENE_ART, MODEL_SERVED_NAME, TEXTURE_DIR)
    }
}

/// The selected background model belongs to the supplied skin table. The
/// `scene_source` tuple remains a compatibility transport namespace only.
pub fn skin_scene_source(
    skin: &crate::menu_skin::SkinTable,
    expansion: bool,
) -> Result<String, crate::menu_skin::SkinError> {
    let file = skin
        .unique_file(&format!(
            "GlueSpriteLayerBackground_V{}",
            u32::from(expansion)
        ))?
        .file;
    Ok(
        match file.len().checked_sub(4).and_then(|at| file.get(at..)) {
            Some(ext) if ext.eq_ignore_ascii_case(".mdl") => {
                format!("{}.mdx", &file[..file.len() - 4])
            }
            _ => file,
        },
    )
}

/// What one material layer's flags mean for a Bevy [`StandardMaterial`].
///
/// Read from the layer's own words, whose positions are measured:
///
/// ```text
/// LAYS+12  word 12   the blend the layer draws with (0..6; called `shader_id`
///                    in `pcx-mdl`, which names it `filterMode` in the format)
/// LAYS+16  word 16   a bitmask (called `blend_mode` in `pcx-mdl`, `shadingFlags`
///                    in the format)
/// LAYS+32  f32       static alpha
/// ```
///
/// The two words are named the other way round in `pcx-mdl` than in the format.
/// The values decide it: the `+12` word only ever reads 0..4 here, which is the
/// `filterMode` range, while `+16` reads `0x10`, `0x21`, `0x90`, `0xA1` — a
/// bitmask, so `shadingFlags`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LayerFlags {
    /// True when `shadingFlags` bit 0 is set: the original draws this unlit.
    pub unshaded: bool,
    /// The blending the layer asks for.
    pub alpha_mode: AlphaMode,
    /// The layer's static alpha. `1.0` when the layer has none.
    pub alpha: f32,
    /// Where [`LayerFlags::alpha_mode`] is not the original's blend exactly.
    pub blend_gap: Option<BlendGap>,
}

/// A blend the format asks for that `StandardMaterial` cannot reproduce exactly.
///
/// Each variant names what is *different*, because "not exact" on its own is not
/// actionable and the three cases fail differently on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendGap {
    /// `filterMode 1`, the format's `Transparent`: the original **alpha-tests**
    /// — a fragment is opaque or gone, never partly there. Bevy's equivalent is
    /// `AlphaMode::Mask(t)`, and `t` is a game constant this workspace has not
    /// measured, so the layer draws *blended* instead. A hard edge becomes a soft
    /// one; nothing disappears, but the cutout look is not reproduced.
    AlphaTestThresholdUnproven,
    /// `filterMode 3`, additive with `ONE/ONE`: the source colour is added
    /// unmodified. Bevy's `AlphaMode::Add` premultiplies first
    /// (`src_color *= src_alpha`, `src_alpha = 0`, giving
    /// `src_alpha * src_color + dst_color`), so the two agree only where the
    /// layer's alpha is 1. Below that Bevy adds less than the original does.
    /// `filterMode 4` is *exactly* Bevy's `Add`, which is why only `3` is
    /// flagged.
    AdditiveIsPremultiplied,
    /// `filterMode 6`, modulate 2x: the destination is multiplied by twice the
    /// source. There is no such mode, so it draws opaque.
    Modulate2x,
}

impl BlendGap {
    /// A one-line statement of what differs, for a log or a report.
    #[must_use]
    pub fn reason(self) -> &'static str {
        match self {
            BlendGap::AlphaTestThresholdUnproven => {
                "filterMode 1 alpha-tests; the threshold is unmeasured, so this draws blended"
            }
            BlendGap::AdditiveIsPremultiplied => {
                "filterMode 3 is ONE/ONE; Bevy's Add premultiplies by alpha"
            }
            BlendGap::Modulate2x => "filterMode 6 modulate-2x has no equivalent; draws opaque",
        }
    }
}

impl LayerFlags {
    /// The material's base colour: a texture's own colours are modulated by the
    /// static alpha, which is where the scene's `0.3` and `0.55` layers get
    /// their transparency.
    fn base_color(&self) -> Color {
        Color::srgba(1.0, 1.0, 1.0, self.alpha.clamp(0.0, 1.0))
    }
}

/// Maps a format `filterMode` word onto Bevy's blending, and says where the two
/// differ.
///
/// The format's modes and their D3D blend states:
///
/// ```text
/// 0  None          opaque
/// 1  Transparent   alpha test -- opaque or discarded, no partial coverage
/// 2  Blend         SRC_ALPHA / ONE_MINUS_SRC_ALPHA
/// 3  Additive      ONE / ONE
/// 4  AdditiveAlpha SRC_ALPHA / ONE
/// 5  Modulate      DST_COLOR / ZERO          (multiply)
/// 6  Modulate2x    DST_COLOR / SRC_COLOR      (multiply, then add both)
/// ```
///
/// Against Bevy's `AlphaMode`, verified in this tree: `Blend` is the same as
/// mode 2; `Multiply` maps to `BLEND_MULTIPLY`
/// (`bevy_pbr/src/pbr_material.rs`) and is the same as mode 5; and `Add` is
/// **mode 4**, not mode 3 — `bevy_pbr/src/render/pbr_functions.wesl` premultiplies
/// `src_color` by `src_alpha` and zeroes `src_alpha`, which is `SRC_ALPHA / ONE`.
/// Mode 3 therefore gets [`BlendGap::AdditiveIsPremultiplied`] and mode 6 has no
/// equivalent at all.
fn alpha_mode_of(filter_mode: u32) -> (AlphaMode, Option<BlendGap>) {
    match filter_mode {
        0 => (AlphaMode::Opaque, None),
        // The original cuts out; the threshold is not measured here, so this is
        // blended and flagged rather than given an invented threshold.
        1 => (AlphaMode::Blend, Some(BlendGap::AlphaTestThresholdUnproven)),
        2 => (AlphaMode::Blend, None),
        3 => (AlphaMode::Add, Some(BlendGap::AdditiveIsPremultiplied)),
        4 => (AlphaMode::Add, None),
        5 => (AlphaMode::Multiply, None),
        _ => (AlphaMode::Opaque, Some(BlendGap::Modulate2x)),
    }
}

/// One material's layer flags, from the material's own record.
///
/// A material with no `LAYS` layer is unlit and opaque, which is what the
/// format says an empty record means.
#[must_use]
pub fn layer_flags(materials: &[pcx_mdl::Material], material_id: usize) -> LayerFlags {
    let Some(mat) = materials.get(material_id) else {
        return LayerFlags {
            alpha: 1.0,
            ..default()
        };
    };
    let Some(layer) = mat.tex_layers().into_iter().next() else {
        return LayerFlags {
            alpha: 1.0,
            ..default()
        };
    };
    let (alpha_mode, blend_gap) = alpha_mode_of(layer.filter_mode);
    LayerFlags {
        unshaded: layer.shading_flags & 0x01 != 0,
        alpha_mode,
        alpha: layer_static_alpha(mat).unwrap_or(1.0),
        blend_gap,
    }
}

/// The layer's static alpha, read at its true offset.
///
/// The checked decoder reads the alpha after the coordinate-id word in each
/// inclusive layer record. This renderer currently uses the first layer only;
/// Additional layers still need drawing support. First-layer and GEOA alpha
/// tracks are sampled by the scene's checked opacity plan.
#[must_use]
pub fn layer_static_alpha(mat: &pcx_mdl::Material) -> Option<f32> {
    Some(mat.checked_layers().ok()?.first()?.properties.static_alpha)
}

/// The served name for one of the scene's own texture paths.
///
/// Lower-cased so the page and this function cannot disagree about case; the
/// archives spell the same file `Wetrock1.blp` and `wetrock1.blp` in different
/// places and `browser::normalize` preserves case.
#[must_use]
#[cfg(test)]
pub fn texture_served_name(wc3_path: &str) -> String {
    texture_served_name_in(wc3_path, TEXTURE_DIR)
}

pub fn texture_served_name_in(wc3_path: &str, directory: &str) -> String {
    let file = wc3_path
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(wc3_path)
        .to_ascii_lowercase();
    format!("{directory}/{file}")
}

/// The texture paths the model names in its own `TEXS` chunk.
pub fn texture_paths(bytes: &[u8]) -> Result<Vec<String>, SceneError> {
    let m = pcx_mdl::Mdlx::parse(bytes).map_err(|e| SceneError::Decode(e.to_string()))?;
    let texs = m
        .textures(bytes)
        .map_err(|e| SceneError::Decode(e.to_string()))?;
    Ok(texs
        .iter()
        .map(|t| t.path.clone())
        .filter(|p| !p.is_empty())
        .collect())
}

/// The scene's own texture list.
///
/// This is the exact list `web/loader.js` registers under `menu3d/`; keeping it
/// derived from the model rather than hand-copied is what stops the two drifting.
#[cfg(test)]
pub fn texture_served_names(bytes: &[u8]) -> Vec<String> {
    texture_served_names_in(bytes, TEXTURE_DIR)
}

#[cfg(any(test, target_arch = "wasm32"))]
pub fn texture_served_names_in(bytes: &[u8], directory: &str) -> Vec<String> {
    texture_paths(bytes)
        .unwrap_or_default()
        .iter()
        .map(|p| texture_served_name_in(p, directory))
        .collect()
}

/// Loads and decodes every texture the model names.
///
/// Desktop first, browser store second — the same two sources the rest of the
/// menu art uses. A texture that cannot be read or decoded is skipped here;
/// required first-layer images are admitted by [`spawn_geosets`], which returns
/// a contextual error before spawning that model if any dependency is absent.
#[cfg(test)]
pub fn load_textures(
    desktop: Option<&std::path::Path>,
    bytes: &[u8],
) -> Vec<pcx_gfx::LoadedTexture> {
    load_textures_in(desktop, bytes, TEXTURE_DIR)
}

pub fn load_textures_in(
    desktop: Option<&std::path::Path>,
    bytes: &[u8],
    directory: &str,
) -> Vec<pcx_gfx::LoadedTexture> {
    let mut out = Vec::new();
    for path in texture_paths(bytes).unwrap_or_default() {
        let served = texture_served_name_in(&path, directory);
        let Some(raw) = crate::mainmenu::art_bytes_named(desktop, &path, &served) else {
            continue;
        };
        if let Ok(tex) = pcx_gfx::load_image(&raw, &path) {
            out.push(tex);
        }
    }
    out
}

/// What [`spawn`] managed to put in the scene, and what it could not.
#[derive(Debug, Clone, Default)]
pub struct SceneInfo {
    /// Geosets spawned.
    pub geosets: usize,
    /// Bones the skeleton is built from.
    pub bones: usize,
    /// HELP nodes included in the checked transform hierarchy.
    pub helpers: usize,
    /// Lights added from the model's `LITE` chunk.
    pub lights: usize,
    /// Cameras found in the model; the first is used.
    pub cameras: usize,
    /// The clip being played.
    pub sequence: String,
    /// That clip's length in milliseconds.
    pub duration_ms: u32,
    /// Retained diagnostic field. Required first-layer image failures now return
    /// a contextual error before this model's geosets are spawned.
    pub missing_textures: Vec<String>,
    /// One entry per geoset whose blend is not the original's exactly, with the
    /// reason. Empty means every material blended as the format asks.
    pub blend_gaps: Vec<BlendGap>,
    /// Bone tracks that are driven by a `GLBS` global sequence rather than the
    /// `Stand` clip. These are the tracks [`tracked_pose`] exists for.
    pub global_sequence_tracks: usize,
}

/// Why the background could not be built.
#[derive(Debug, thiserror::Error)]
pub enum SceneError {
    /// The model did not parse.
    #[error("scene model: {0}")]
    Decode(String),
    #[error("{source_name}: {error}")]
    TextureSelection {
        source_name: String,
        #[source]
        error: pcx_gfx::mdx_texture_selection::Error,
    },
    #[error("{source_name}: {context}: {reason}")]
    TextureImage {
        source_name: String,
        context: String,
        reason: &'static str,
    },
    #[error(transparent)]
    UvTranslation(#[from] pcx_gfx::mdx_uv_translation::Error),
    #[error("{source_name}: elapsed milliseconds exceed u64 clock")]
    Clock { source_name: String },
}

/// The live background: the skeleton being posed and the meshes it drives.
#[derive(Resource, Default)]
pub struct MainMenuScene {
    /// The decoded model, whose `meshes` are the rest positions.
    model: Option<pcx_gfx::LoadedModel>,
    /// Checked BONE records followed by HELP transforms, with authored IDs.
    bones: Vec<pcx_mdl::Bone>,
    /// The model's pivot table.
    pivots: Option<pcx_mdl::Pivots>,
    /// One Bevy mesh per geoset, parallel to `model.meshes`.
    handles: Vec<Handle<Mesh>>,
    opacity: Option<pcx_gfx::mdx_opacity::OpacityPlan>,
    texture_selection: Option<pcx_gfx::mdx_texture_selection::TextureSelectionPlan>,
    texture_bindings: TextureBindings,
    uv_translation: Option<pcx_gfx::mdx_uv_translation::UvTranslationPlan>,
    opacity_materials: Vec<(usize, Entity, Handle<StandardMaterial>)>,
    /// The clip's first frame.
    interval_start: u32,
    /// The clip's length in milliseconds.
    duration_ms: u32,
    /// Position within the clip.
    time_ms: f32,
    /// Monotonic elapsed time, which is what global sequences run on.
    elapsed: std::time::Duration,
    /// `GLBS` durations, indexed by global-sequence id.
    global_durations: Vec<u32>,
}

/// Build the background from the model's bytes.
///
/// `textures` must contain every rendered first-layer static/KMTF image. Missing,
/// ambiguous or unsupported dependencies refuse before geosets are spawned.
///
/// The camera and lights come from the model. The camera is given `order: -1`
/// so it draws **before** the UI camera; see the module docs for the caller's
/// UI camera configuration.
pub fn spawn(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    source: &str,
    bytes: &[u8],
    textures: &[pcx_gfx::LoadedTexture],
) -> Result<SceneInfo, SceneError> {
    let m = pcx_mdl::Mdlx::parse(bytes).map_err(|e| SceneError::Decode(e.to_string()))?;
    let model =
        pcx_gfx::load_model(bytes, source).map_err(|e| SceneError::Decode(e.to_string()))?;
    let runtime = pcx_gfx::mdx_runtime::RuntimePlan::parse(bytes)
        .map_err(|e| SceneError::Decode(e.to_string()))?;
    let joints = &runtime.joints;
    let bones = joints.joints().to_vec();
    let pivots = Some(joints.pivots().clone());

    // The entry clip: the *first* looping `Stand`, which is the 0..16667 one.
    let sequences = m
        .sequences(bytes)
        .map_err(|e| SceneError::Decode(e.to_string()))?;
    let clip = sequences
        .iter()
        .find(|s| s.name == "Stand" && !s.non_looping)
        .or_else(|| sequences.iter().find(|s| s.name == "Stand"))
        .or_else(|| sequences.first());
    let interval = clip.map_or([0, 0], |clip| [clip.interval_start, clip.interval_end]);
    let opacity = pcx_gfx::mdx_opacity::OpacityPlan::checked(&runtime, interval)
        .map_err(|e| SceneError::Decode(format!("{source}: {e}")))?;

    let texture_selection = texture_selection(&runtime, interval, source)?;
    let uv_translation =
        pcx_gfx::mdx_uv_translation::UvTranslationPlan::from_runtime(&runtime, interval, source)?;
    let mut info = SceneInfo {
        bones: joints
            .kinds()
            .iter()
            .filter(|&&kind| kind == pcx_gfx::mdx_runtime::JointKind::Bone)
            .count(),
        helpers: joints
            .kinds()
            .iter()
            .filter(|&&kind| kind == pcx_gfx::mdx_runtime::JointKind::Helper)
            .count(),
        global_sequence_tracks: bones
            .iter()
            .flat_map(|b| b.tracks.iter())
            .filter(|t| t.global_sequence_id >= 0)
            .count(),
        sequence: clip.map(|s| s.name.clone()).unwrap_or_default(),
        duration_ms: clip.map(|s| s.duration()).unwrap_or(0),
        ..default()
    };

    let Geosets {
        handles,
        bound: opacity_materials,
        textures: texture_bindings,
    } = spawn_geosets(
        commands,
        GeosetAssets {
            meshes,
            materials,
            images,
        },
        GeosetSource {
            m: &m,
            bytes,
            source,
            model: &model,
            textures,
            runtime: &runtime,
            selection: &texture_selection,
            uv_translation: &uv_translation,
        },
        &opacity,
        interval[0],
        &mut info,
        MainMenuSceneEntity,
    )?;

    // The model's own camera.
    let cameras = m.cameras(bytes).unwrap_or_default();
    info.cameras = cameras.len();
    if let Some(cam) = cameras.first() {
        commands.spawn((
            Camera3d::default(),
            Projection::Perspective(PerspectiveProjection {
                fov: if cam.field_of_view > 0.0 {
                    cam.field_of_view
                } else {
                    std::f32::consts::FRAC_PI_4
                },
                near: cam.near.max(0.1),
                far: cam.far.max(1000.0),
                ..default()
            }),
            // The model is Z-up, which is the convention the rest of the
            // workspace renders in, so `up` is `Z`.
            Transform::from_translation(Vec3::from(cam.eye))
                .looking_at(Vec3::from(cam.target), Vec3::Z),
            Camera {
                // Draw the world before the UI camera, which the launcher owns.
                order: -1,
                clear_color: ClearColorConfig::Custom(Color::srgb(0.0, 0.0, 0.0)),
                ..default()
            },
            MainMenuSceneCamera,
            MainMenuSceneEntity,
        ));
    }

    // The model's own lights.
    let lights = m.lights(bytes).unwrap_or_default();
    let mut ambient = Vec::new();
    for (i, l) in lights.iter().enumerate() {
        let colour = Color::srgb(
            l.colour[0].clamp(0.0, 1.0),
            l.colour[1].clamp(0.0, 1.0),
            l.colour[2].clamp(0.0, 1.0),
        );
        match l.kind {
            pcx_mdl::LightKind::Ambient => ambient.push((colour, l.ambient_intensity)),
            pcx_mdl::LightKind::Directional => {
                // A directional light in this format has no position; its
                // direction is the node's own transform. Without sampling the
                // node tracks it is placed looking down the world -Y, which is
                // the scene's default orientation.
                commands.spawn((
                    DirectionalLight {
                        color: colour,
                        illuminance: (l.intensity.max(0.0) * 20_000.0).max(1.0),
                        ..default()
                    },
                    Transform::from_xyz(0.0, 0.0, 5000.0).looking_at(Vec3::ZERO, Vec3::Z),
                    Name::new(format!("menu3d-light-{i}")),
                    MainMenuSceneEntity,
                ));
                info.lights += 1;
            }
            pcx_mdl::LightKind::Omnidirectional => {
                // The light's position is its **node pivot**, which is where the
                // model's own rest pose puts it. Placing these all at the origin
                // — which an earlier version did — stacks five point lights in
                // one spot and blows the scene out. The light's animated node
                // position is not sampled (its `KeyTrack`s have no public
                // sampler here), so a light that moves sits still; that is
                // recorded rather than approximated.
                let pos = pivots
                    .as_ref()
                    .and_then(|p| p.position(l.object_id))
                    .unwrap_or([0.0; 3]);
                commands.spawn((
                    PointLight {
                        color: colour,
                        // WC3's intensity is its own unit, not lumens, so this
                        // is a scale factor and an approximation: the shipped
                        // values are 40..175 and Bevy wants thousands of lumens
                        // for a room-sized light. `×200` puts them at 8,000..
                        // 35,000 lm. The original's exact photometric behaviour
                        // is not reproduced.
                        intensity: (l.intensity.max(0.0) * 200.0).max(1.0),
                        // `attenuation_end` (200 in every shipped record) is the
                        // radius in the model's own units.
                        range: l.attenuation_end.max(1.0),
                        ..default()
                    },
                    Transform::from_translation(Vec3::from(pos)),
                    Name::new(format!("menu3d-light-{i}")),
                    MainMenuSceneEntity,
                ));
                info.lights += 1;
            }
        }
    }
    let ambient_total: f32 = ambient.iter().map(|(_, i)| i.max(0.0)).sum();
    commands.insert_resource(GlobalAmbientLight {
        color: Color::WHITE,
        brightness: if ambient_total > 0.0 {
            ambient_total * 2000.0
        } else {
            // A glue scene with no ambient record still needs the unlit parts
            // to be visible; this is a floor, and `lights` above says whether
            // the model supplied anything.
            600.0
        },
        affects_lightmapped_meshes: true,
    });

    commands.insert_resource(MainMenuScene {
        model: Some(model),
        bones,
        pivots,
        handles,
        opacity: Some(opacity),
        opacity_materials,
        texture_selection: Some(texture_selection),
        texture_bindings,
        uv_translation: Some(uv_translation),
        interval_start: clip.map(|s| s.interval_start).unwrap_or(0),
        duration_ms: clip.map(|s| s.duration()).unwrap_or(0),
        time_ms: 0.0,
        elapsed: std::time::Duration::ZERO,
        global_durations: runtime.global_sequences,
    });
    info!(bones = info.bones, helpers = info.helpers, sequence = %info.sequence, global_sequence_tracks = info.global_sequence_tracks, "Original menu animation source");
    Ok(info)
}

/// The asset stores a model's geosets are added to.
pub struct GeosetAssets<'a> {
    pub meshes: &'a mut Assets<Mesh>,
    pub materials: &'a mut Assets<StandardMaterial>,
    pub images: &'a mut Assets<Image>,
}

/// One parsed model and the decoded textures its geosets bind.
pub struct GeosetSource<'a> {
    pub m: &'a pcx_mdl::Mdlx,
    pub bytes: &'a [u8],
    pub source: &'a str,
    pub model: &'a pcx_gfx::LoadedModel,
    pub textures: &'a [pcx_gfx::LoadedTexture],
    pub runtime: &'a pcx_gfx::mdx_runtime::RuntimePlan,
    pub selection: &'a pcx_gfx::mdx_texture_selection::TextureSelectionPlan,
    pub uv_translation: &'a pcx_gfx::mdx_uv_translation::UvTranslationPlan,
}

/// One geoset's authored id, its entity and its material.
pub type BoundGeoset = (usize, Entity, Handle<StandardMaterial>);

/// The meshes [`spawn_geosets`] created, parallel to the model's meshes, and the
/// material each geoset's opacity is written to.
pub struct Geosets {
    pub handles: Vec<Handle<Mesh>>,
    pub bound: Vec<BoundGeoset>,
    pub textures: TextureBindings,
}

/// Spawns one entity per geoset, with the material its own `material_id` and
/// layer flags describe and its opacity at `frame`. `bundle` is added to each.
pub fn spawn_geosets(
    commands: &mut Commands,
    assets: GeosetAssets,
    source: GeosetSource,
    opacity: &pcx_gfx::mdx_opacity::OpacityPlan,
    frame: u32,
    info: &mut SceneInfo,
    bundle: impl Bundle + Clone,
) -> Result<Geosets, SceneError> {
    let GeosetAssets {
        meshes,
        materials,
        images,
    } = assets;
    let GeosetSource {
        m,
        bytes,
        source,
        model,
        textures,
        runtime,
        selection,
        uv_translation,
    } = source;
    let required = required_texture_images(source, runtime, textures)?;
    let mdl_materials = m
        .materials(bytes)
        .map_err(|e| SceneError::Decode(format!("{source}: {e}")))?;
    if model.meshes.len() != runtime.geosets.len() {
        return Err(SceneError::Decode(format!(
            "{source}: mesh/GEOS count mismatch"
        )));
    }
    // Preflight every initial selection and opacity before any asset/entity write.
    let mut initial = Vec::with_capacity(model.meshes.len());
    for mesh in &model.meshes {
        let fail =
            |reason| texture_failure(source, mesh.geoset, mesh.material_id as usize, None, reason);
        let binding = runtime
            .geosets
            .get(mesh.geoset)
            .ok_or_else(|| fail("mesh has no checked GEOS binding"))?;
        if binding.geoset_id != mesh.geoset || binding.material_id != mesh.material_id as usize {
            return Err(fail("mesh/checked GEOS material mismatch"));
        }
        let id = selection
            .sample(mesh.geoset, frame, 0)
            .ok_or_else(|| fail("missing initial TEXS sample"))?;
        if !required.contains_key(&id) {
            return Err(fail("initial sample has no admitted TEXS image"));
        }
        let alpha = opacity
            .sample(mesh.geoset, frame, 0)
            .ok_or_else(|| fail("missing initial opacity binding"))?;
        let uv = sampled_uv_transform(
            uv_translation,
            source,
            mesh.geoset,
            mesh.material_id as usize,
            frame,
            0,
        )?;
        initial.push((id, alpha, uv));
    }
    let mut texture_bindings = cache_texture_images(source, runtime, &required, images);
    let mut handles = Vec::with_capacity(model.meshes.len());
    let mut bound = Vec::with_capacity(model.meshes.len());
    for (mesh, (texture_id, alpha, uv_transform)) in model.meshes.iter().zip(initial) {
        let id = mesh.material_id as usize;
        let flags = layer_flags(&mdl_materials, id);
        if let Some(gap) = flags.blend_gap {
            warn!(
                material_id = id,
                reason = gap.reason(),
                "Original menu material rendering gap"
            );
            info.blend_gaps.push(gap);
        }
        let image = texture_bindings.images[&texture_id].clone();
        let material = materials.add(StandardMaterial {
            base_color: flags.base_color().with_alpha(alpha),
            base_color_texture: Some(image),
            uv_transform,
            // Glue art is scenery, not a lit character, so a high roughness
            // keeps it from picking up a sheen the original has not got.
            perceptual_roughness: 1.0,
            // `Unshaded` in the layer's shading flags.
            unlit: flags.unshaded,
            // `filterMode`.
            alpha_mode: flags.alpha_mode,
            // Culling is left off on purpose: the bit that means `TwoSided` in
            // this format is not established by measurement here, and guessing
            // wrong makes a geoset vanish rather than merely look different.
            cull_mode: None,
            ..default()
        });
        handles.push(meshes.add(pcx_gfx::to_bevy_mesh(mesh)));
        let entity = commands
            .spawn((
                Mesh3d(handles.last().unwrap().clone()),
                MeshMaterial3d(material.clone()),
                if alpha <= 0.0 {
                    Visibility::Hidden
                } else {
                    Visibility::Inherited
                },
                bundle.clone(),
            ))
            .id();
        texture_bindings.materials.push(TextureBinding {
            geoset: mesh.geoset,
            material_id: id,
            material: material.clone(),
            last_texture: texture_id,
        });
        bound.push((mesh.geoset, entity, material));
        info.geosets += 1;
    }
    Ok(Geosets {
        handles,
        bound,
        textures: texture_bindings,
    })
}

/// Writes each bound geoset's opacity at `frame`, hiding the ones at zero.
pub fn apply_opacity<F: bevy::ecs::query::QueryFilter>(
    opacity: &pcx_gfx::mdx_opacity::OpacityPlan,
    bound: &[BoundGeoset],
    frame: u32,
    elapsed_ms: u64,
    materials: &mut Assets<StandardMaterial>,
    visibility: &mut Query<&mut Visibility, F>,
) {
    for (geoset, entity, handle) in bound {
        let Some(alpha) = opacity.sample(*geoset, frame, elapsed_ms) else {
            continue;
        };
        if let Some(mut material) = materials.get_mut(handle) {
            material.base_color = material.base_color.with_alpha(alpha);
        }
        if let Ok(mut visibility) = visibility.get_mut(*entity) {
            *visibility = if alpha <= 0.0 {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            };
        }
    }
}

/// A model's rest meshes and the joints that pose them.
pub struct Rig<'a> {
    pub model: &'a pcx_gfx::LoadedModel,
    pub bones: &'a [pcx_mdl::Bone],
    pub pivots: &'a pcx_mdl::Pivots,
    /// `GLBS` durations, indexed by global-sequence id.
    pub global_durations: &'a [u32],
}

/// Poses the skeleton at `frame` and rewrites every geoset's positions.
///
/// The poses compose through the decoder's own matrix convention
/// ([`pcx_gfx::pose_from_poses`] and [`pcx_gfx::skin_mesh`]); global-sequence
/// tracks run on `elapsed_ms` (see [`tracked_pose`]).
pub fn skin_geosets(
    rig: Rig,
    handles: &[Handle<Mesh>],
    frame: u32,
    elapsed_ms: u32,
    meshes: &mut Assets<Mesh>,
) {
    let poses: Vec<pcx_mdl::Pose> = rig
        .bones
        .iter()
        .map(|b| tracked_pose(b, frame, elapsed_ms, rig.global_durations))
        .collect();
    let skeleton = pcx_gfx::pose_from_poses(rig.model, rig.bones, rig.pivots, &poses);
    for (handle, rest) in handles.iter().zip(&rig.model.meshes) {
        let Some(mut mesh) = meshes.get_mut(handle) else {
            continue;
        };
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            pcx_gfx::skin_mesh(rest, &skeleton),
        );
    }
}

/// Marker for the camera this module owns, so a caller can find and remove it.
#[derive(Component)]
pub struct MainMenuSceneCamera;

/// All entities owned by a scene, so changing expansion removes the old scene.
#[derive(Component, Clone)]
pub struct MainMenuSceneEntity;

/// A material's last selected TEXS record; paths do not identify sampler state.
struct TextureBinding {
    geoset: usize,
    material_id: usize,
    material: Handle<StandardMaterial>,
    last_texture: u32,
}

#[derive(Default)]
pub struct TextureBindings {
    source: String,
    images: std::collections::BTreeMap<u32, Handle<Image>>,
    materials: Vec<TextureBinding>,
}

pub fn texture_selection(
    runtime: &pcx_gfx::mdx_runtime::RuntimePlan,
    interval: [u32; 2],
    source: &str,
) -> Result<pcx_gfx::mdx_texture_selection::TextureSelectionPlan, SceneError> {
    pcx_gfx::mdx_texture_selection::TextureSelectionPlan::checked(runtime, interval).map_err(
        |error| SceneError::TextureSelection {
            source_name: source.into(),
            error,
        },
    )
}

pub fn elapsed_millis(source: &str, elapsed: std::time::Duration) -> Result<u64, SceneError> {
    u64::try_from(elapsed.as_millis()).map_err(|_| SceneError::Clock {
        source_name: source.into(),
    })
}

/// Advance the independent clock only after its full millisecond range is checked.
pub fn advance_elapsed(
    source: &str,
    elapsed: &mut std::time::Duration,
    delta: std::time::Duration,
) -> Result<u64, SceneError> {
    let next = elapsed
        .checked_add(delta)
        .ok_or_else(|| SceneError::Clock {
            source_name: source.into(),
        })?;
    let millis = elapsed_millis(source, next)?;
    *elapsed = next;
    Ok(millis)
}

fn texture_failure(
    source: &str,
    geoset: usize,
    material: usize,
    texture: Option<(u32, &str)>,
    reason: &'static str,
) -> SceneError {
    let mut context = format!("geoset {geoset} material {material} layer 0");
    if let Some((id, path)) = texture {
        context.push_str(&format!(" TEXS {id} {path:?}"));
    }
    SceneError::TextureImage {
        source_name: source.into(),
        context,
        reason,
    }
}

/// Admit every first-layer static/KMTF edge before allocating images or entities.
fn required_texture_images<'a>(
    source: &str,
    runtime: &pcx_gfx::mdx_runtime::RuntimePlan,
    textures: &'a [pcx_gfx::LoadedTexture],
) -> Result<std::collections::BTreeMap<u32, &'a pcx_gfx::LoadedTexture>, SceneError> {
    let normalize = |path: &str| path.replace('\\', "/").to_ascii_lowercase();
    let mut required = std::collections::BTreeMap::new();
    for binding in &runtime.geosets {
        let fail = |texture, reason| {
            texture_failure(
                source,
                binding.geoset_id,
                binding.material_id,
                texture,
                reason,
            )
        };
        let layer = runtime
            .materials
            .get(binding.material_id)
            .and_then(|material| material.layers.first())
            .ok_or_else(|| fail(None, "missing checked first layer"))?;
        let ids = std::iter::once(layer.properties.texture_id).chain(
            layer
                .texture_tracks
                .iter()
                .flat_map(|track| track.keys.iter().map(|&(_, id)| id)),
        );
        for id in ids {
            if required.contains_key(&id) {
                continue;
            }
            let authored = runtime
                .textures
                .get(id as usize)
                .ok_or_else(|| fail(Some((id, "")), "absent TEXS record"))?;
            let fail = |reason| fail(Some((id, authored.path.as_str())), reason);
            if authored.path.is_empty() {
                return Err(fail("empty texture path; no replacement source supplied"));
            }
            if authored.flags & !(pcx_mdl::TEXTURE_WRAP_WIDTH | pcx_mdl::TEXTURE_WRAP_HEIGHT) != 0 {
                return Err(fail("unsupported TEXS sampler flags"));
            }
            let path = normalize(&authored.path);
            let mut found: Option<&pcx_gfx::LoadedTexture> = None;
            for image in textures
                .iter()
                .filter(|image| normalize(&image.source) == path)
            {
                let length = usize::try_from(image.width)
                    .ok()
                    .and_then(|width| usize::try_from(image.height).ok()?.checked_mul(width))
                    .and_then(|pixels| pixels.checked_mul(4));
                if image.width == 0 || image.height == 0 || length != Some(image.rgba.len()) {
                    return Err(fail("invalid decoded RGBA dimensions or length"));
                }
                if found.is_some_and(|previous| {
                    previous.width != image.width
                        || previous.height != image.height
                        || previous.rgba != image.rgba
                }) {
                    return Err(fail("unequal decoded images for canonical path"));
                }
                found = Some(image);
            }
            required.insert(
                id,
                found.ok_or_else(|| fail("missing decoded canonical image"))?,
            );
        }
    }
    Ok(required)
}

/// Admission proved equal canonical pixels; share backing only for equal sampler flags.
fn cache_texture_images(
    source: &str,
    runtime: &pcx_gfx::mdx_runtime::RuntimePlan,
    required: &std::collections::BTreeMap<u32, &pcx_gfx::LoadedTexture>,
    images: &mut Assets<Image>,
) -> TextureBindings {
    let mut shared = std::collections::BTreeMap::new();
    TextureBindings {
        source: source.into(),
        images: required
            .iter()
            .map(|(&id, texture)| {
                let authored = &runtime.textures[id as usize];
                let key = (
                    authored.path.replace('\\', "/").to_ascii_lowercase(),
                    authored.flags,
                );
                let handle = shared.entry(key).or_insert_with(|| {
                    let mut image = pcx_gfx::viewer::rgba_image(texture);
                    image.sampler = pcx_gfx::viewer::sampler_for_texture_flags(authored.flags);
                    images.add(image)
                });
                (id, handle.clone())
            })
            .collect(),
        materials: Vec::new(),
    }
}

/// Validate all targets before any write; unchanged selections do not dirty assets.
pub fn apply_texture_selection(
    selection: &pcx_gfx::mdx_texture_selection::TextureSelectionPlan,
    bindings: &mut TextureBindings,
    frame: u32,
    elapsed_ms: u64,
    materials: &mut Assets<StandardMaterial>,
) -> Result<(), SceneError> {
    let mut changes = Vec::new();
    for (index, binding) in bindings.materials.iter().enumerate() {
        let fail = |texture, reason| {
            texture_failure(
                &bindings.source,
                binding.geoset,
                binding.material_id,
                texture,
                reason,
            )
        };
        let id = selection
            .sample(binding.geoset, frame, elapsed_ms)
            .ok_or_else(|| fail(None, "missing sampled TEXS binding"))?;
        let image = bindings
            .images
            .get(&id)
            .ok_or_else(|| fail(Some((id, "")), "missing cached TEXS image"))?;
        if materials.get(&binding.material).is_none() {
            return Err(fail(Some((id, "")), "missing bound material"));
        }
        if id != binding.last_texture {
            changes.push((index, id, image.clone()));
        }
    }
    for (index, id, image) in changes {
        let binding = &mut bindings.materials[index];
        // Every target was validated above; exclusive Assets access prevents removal.
        // Distinct authored TEXS IDs may share the same backing image.
        if materials
            .get(&binding.material)
            .expect("validated texture material")
            .base_color_texture
            .as_ref()
            != Some(&image)
        {
            materials
                .get_mut(&binding.material)
                .expect("validated texture material")
                .base_color_texture = Some(image);
        }
        binding.last_texture = id;
    }
    Ok(())
}

/// Convert a checked sample to an absolute transform of the authored UVs.
fn sampled_uv_transform(
    plan: &pcx_gfx::mdx_uv_translation::UvTranslationPlan,
    source: &str,
    geoset: usize,
    material: usize,
    frame: u32,
    elapsed_ms: u64,
) -> Result<bevy::math::Affine2, SceneError> {
    let sample = plan.sample(geoset, frame, elapsed_ms)?.ok_or_else(|| {
        pcx_gfx::mdx_uv_translation::Error {
            context: format!("{source} geoset {geoset} material {material} layer 0"),
            reason: "missing UV translation sample".into(),
        }
    })?;
    Ok(bevy::math::Affine2::from_translation(Vec2::from_array(
        sample.offset,
    )))
}

/// Validate every target before writing; resampling avoids a per-frame change list.
pub fn apply_uv_translation(
    plan: &pcx_gfx::mdx_uv_translation::UvTranslationPlan,
    bindings: &TextureBindings,
    frame: u32,
    elapsed_ms: u64,
    materials: &mut Assets<StandardMaterial>,
) -> Result<(), SceneError> {
    for binding in &bindings.materials {
        sampled_uv_transform(
            plan,
            &bindings.source,
            binding.geoset,
            binding.material_id,
            frame,
            elapsed_ms,
        )?;
        if materials.get(&binding.material).is_none() {
            return Err(pcx_gfx::mdx_uv_translation::Error {
                context: format!(
                    "{} geoset {} material {} layer 0",
                    bindings.source, binding.geoset, binding.material_id
                ),
                reason: "missing bound UV material".into(),
            }
            .into());
        }
    }
    for binding in &bindings.materials {
        let transform = sampled_uv_transform(
            plan,
            &bindings.source,
            binding.geoset,
            binding.material_id,
            frame,
            elapsed_ms,
        )
        .expect("validated UV sample");
        // Exclusive asset access and pure sampling preserve the validation above.
        if materials
            .get(&binding.material)
            .expect("validated UV material")
            .uv_transform
            != transform
        {
            materials
                .get_mut(&binding.material)
                .expect("validated UV material")
                .uv_transform = transform;
        }
    }
    Ok(())
}

/// One bone's pose with global-sequence tracks sampled on their **own** clock.
///
/// A global sequence does not follow the model's `Stand` clip; it runs on its
/// own timeline and continues across clip wraps.
///
/// A bone whose tracks are all local still goes straight to the crate's sampler,
/// so nothing about the ordinary path changes. A bone with any global track has
/// each track sampled at its own time; the time is the **elapsed** clock modulo
/// that sequence's duration, not the clip clock, because a global sequence keeps
/// running across a clip wrap.
///
/// The crate's own interpolation is reused rather than reimplemented: each track
/// is sampled through the borrowed `sample_bone_track`, so Hermite and Bézier
/// easing are still the decoder's without copying owned track bytes.
#[must_use]
pub fn tracked_pose(
    bone: &pcx_mdl::Bone,
    sequence_frame: u32,
    elapsed_ms: u32,
    durations: &[u32],
) -> pcx_mdl::Pose {
    if bone.tracks.iter().all(|t| t.global_sequence_id < 0) {
        return pcx_mdl::sample_bone_pose(bone, sequence_frame);
    }
    let mut out = pcx_mdl::Pose::default();
    for track in &bone.tracks {
        let frame = match u32::try_from(track.global_sequence_id) {
            Ok(id) => match durations.get(id as usize).copied().unwrap_or(0) {
                // A global sequence of length zero cannot be sampled; the clip
                // clock is the only sane fallback and is recorded as such.
                0 => sequence_frame,
                d => elapsed_ms % d,
            },
            Err(_) => sequence_frame,
        };
        let pose = pcx_mdl::sample_bone_track(track, frame);
        match track.kind {
            pcx_mdl::TrackKind::Translation => out.translation = pose.translation,
            pcx_mdl::TrackKind::Rotation => out.rotation = pose.rotation,
            pcx_mdl::TrackKind::Scaling => out.scale = pose.scale,
        }
    }
    out
}

/// Advances the background's clip and re-skins every geoset.
///
/// This is the whole animation path: the model's `Stand` tracks are sampled at
/// the clip clock, the poses compose into a skeleton through the decoder's own
/// matrix convention, and each geoset's positions are rewritten. It is the same
/// composition `pcx-gfx`'s viewer uses — [`pcx_gfx::pose_from_poses`] and
/// [`pcx_gfx::skin_mesh`] — driven by a clock this module owns rather than by
/// Bevy's animation graph, because the background is a single looping clip and
/// nothing waits on `Stand Alternate` or `Morph Alternate`.
pub fn advance(
    time: Res<Time>,
    mut scene: ResMut<MainMenuScene>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut visibility: Query<&mut Visibility, With<MainMenuSceneEntity>>,
) {
    if scene.model.is_none() {
        return;
    }
    // An absent or zero-length local clip holds its source frame; global tracks
    // still use the independent elapsed clock below.
    if scene.duration_ms == 0 {
        scene.time_ms = 0.0;
    } else {
        let dt_ms = time.delta_secs() * 1000.0;
        scene.time_ms = (scene.time_ms + dt_ms) % scene.duration_ms as f32;
    }
    // The elapsed clock does *not* wrap with the clip: a global sequence keeps
    // running across a clip wrap, which is the whole point of it being global.
    let frame = scene.interval_start + scene.time_ms as u32;
    let elapsed_result = {
        let MainMenuScene {
            elapsed,
            texture_bindings,
            ..
        } = &mut *scene;
        advance_elapsed(&texture_bindings.source, elapsed, time.delta())
    };
    let elapsed_ms = match elapsed_result {
        Ok(elapsed) => elapsed,
        Err(error) => {
            error!("{error}");
            return;
        }
    };
    if let Some(uv) = &scene.uv_translation
        && let Err(error) = apply_uv_translation(
            uv,
            &scene.texture_bindings,
            frame,
            elapsed_ms,
            &mut materials,
        )
    {
        error!("{error}");
        return;
    }
    {
        let MainMenuScene {
            texture_selection,
            texture_bindings,
            ..
        } = &mut *scene;
        if let Some(selection) = texture_selection
            && let Err(error) = apply_texture_selection(
                selection,
                texture_bindings,
                frame,
                elapsed_ms,
                &mut materials,
            )
        {
            error!("{error}");
            return;
        }
    }
    if let Some(opacity) = &scene.opacity {
        apply_opacity(
            opacity,
            &scene.opacity_materials,
            frame,
            elapsed_ms,
            &mut materials,
            &mut visibility,
        );
    }
    let (Some(model), Some(pivots)) = (scene.model.as_ref(), scene.pivots.as_ref()) else {
        return;
    };
    skin_geosets(
        Rig {
            model,
            bones: &scene.bones,
            pivots,
            global_durations: &scene.global_durations,
        },
        &scene.handles,
        frame,
        elapsed_ms as u32,
        &mut meshes,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alpha_update_follows_geoset_id_and_hides_zero_opacity_without_touching_other_materials() {
        use bevy::ecs::system::RunSystemOnce;
        use pcx_gfx::mdx_runtime::{GeosetInput, RuntimeInput, RuntimePlan};
        let mut data = Vec::new();
        for word in [
            0u32,
            0,
            u32::from_le_bytes(*b"LAYS"),
            1,
            28,
            2,
            0,
            0,
            pcx_mdl::NO_ID,
            0,
            1f32.to_bits(),
        ] {
            data.extend(word.to_le_bytes());
        }
        let track = pcx_mdl::AnimTrack {
            magic: *b"KGAO",
            count: 2,
            interpolation: pcx_mdl::Interpolation::Linear,
            global_seq_id: pcx_mdl::NO_ID,
            raw: [(0u32, 0f32), (100, 1f32)]
                .into_iter()
                .flat_map(|(frame, alpha)| {
                    frame.to_le_bytes().into_iter().chain(alpha.to_le_bytes())
                })
                .collect(),
        };
        let plan = RuntimePlan::checked(RuntimeInput {
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
                flags: 0,
                geoset_id: 1,
                static_alpha: 0.25,
                static_colour: [1.; 3],
                tracks: vec![track],
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
        let opacity = pcx_gfx::mdx_opacity::OpacityPlan::checked(&plan, [0, 100]).unwrap();
        let mut world = World::new();
        let mut materials = Assets::<StandardMaterial>::default();
        let unaffected = materials.add(StandardMaterial::default());
        let animated = materials.add(StandardMaterial::default());
        let unrelated = materials.add(StandardMaterial {
            base_color: Color::WHITE.with_alpha(0.7),
            ..default()
        });
        let first = world
            .spawn((MainMenuSceneEntity, Visibility::Inherited))
            .id();
        let second = world.spawn((MainMenuSceneEntity, Visibility::Hidden)).id();
        world.insert_resource(materials);
        world.insert_resource(Assets::<Mesh>::default());
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_millis(50));
        world.insert_resource(time);
        world.insert_resource(MainMenuScene {
            model: Some(pcx_gfx::LoadedModel {
                name: String::new(),
                minimum_extent: [0.; 3],
                maximum_extent: [0.; 3],
                meshes: vec![],
                bone_names: vec![],
                bone_parents: vec![],
                sequences: vec![],
            }),
            opacity: Some(opacity),
            opacity_materials: vec![
                (0, first, unaffected.clone()),
                (1, second, animated.clone()),
            ],
            duration_ms: 100,
            ..default()
        });
        world.run_system_once(advance).unwrap();
        let materials = world.resource::<Assets<StandardMaterial>>();
        assert_eq!(materials.get(&unaffected).unwrap().base_color.alpha(), 1.0);
        assert_eq!(materials.get(&animated).unwrap().base_color.alpha(), 0.5);
        assert_eq!(materials.get(&unrelated).unwrap().base_color.alpha(), 0.7);
        assert_eq!(
            world.get::<Visibility>(second),
            Some(&Visibility::Inherited)
        );
        world.run_system_once(advance).unwrap();
        assert_eq!(world.get::<Visibility>(second), Some(&Visibility::Hidden));
        assert_eq!(world.get::<Visibility>(first), Some(&Visibility::Inherited));
    }

    #[test]
    #[ignore = "requires PEACECRAFT3_MDX_INSTALL with both original menu models"]
    fn original_menu_import_keeps_authored_helper_parents() {
        let install = std::path::PathBuf::from(
            std::env::var_os("PEACECRAFT3_MDX_INSTALL")
                .expect("explicit original install required"),
        );
        for expansion in [false, true] {
            let source = scene_source(expansion).0;
            let bytes = crate::mainmenu::art_bytes(Some(&install), source)
                .unwrap_or_else(|| panic!("{}: missing {source}", install.display()));
            let parsed = pcx_mdl::Mdlx::parse(&bytes).unwrap();
            let authored_bones = parsed.bones(&bytes).unwrap();
            let authored_helpers = parsed.nodes(&bytes, pcx_mdl::tag::HELP).unwrap();
            assert!(!authored_helpers.is_empty(), "fixture must exercise HELP");
            let textures = load_textures_in(Some(&install), &bytes, scene_source(expansion).2);
            let mut world = World::new();
            let mut meshes = Assets::<Mesh>::default();
            let mut materials = Assets::<StandardMaterial>::default();
            let mut images = Assets::<Image>::default();
            let info = spawn(
                &mut world.commands(),
                &mut meshes,
                &mut materials,
                &mut images,
                source,
                &bytes,
                &textures,
            )
            .unwrap();
            world.flush();
            let scene = world.resource::<MainMenuScene>();
            assert_eq!(info.bones, authored_bones.len());
            assert_eq!(info.helpers, authored_helpers.len());
            assert_eq!(scene.bones.len(), info.bones + info.helpers);
            let helper_ids: std::collections::HashSet<_> = authored_helpers
                .iter()
                .map(|helper| helper.object_id)
                .collect();
            assert!(
                authored_bones
                    .iter()
                    .any(|bone| helper_ids.contains(&(bone.parent_id as u32)))
            );
            for helper in &authored_helpers {
                let retained = scene
                    .bones
                    .iter()
                    .find(|joint| joint.object_id == helper.object_id)
                    .expect("HELP must participate in the actual scene hierarchy");
                assert_eq!(retained.parent_id, helper.parent_id);
                assert_eq!(retained.name, helper.name);
                assert_eq!(retained.tracks.len(), helper.tracks.len());
            }
        }
    }

    #[test]
    fn expansion_selects_its_shipped_scene_and_separate_texture_namespace() {
        let (path, served, directory) = scene_source(true);
        assert_eq!(path, r"UI\Glues\MainMenu\MainMenu3D_Exp\MainMenu3D_Exp.mdx");
        assert_eq!(served, "menu3d-exp/mainmenu3d_exp.mdx");
        assert_eq!(
            texture_served_name_in(r"Textures\SKY2.blp", directory),
            "menu3d-exp/sky2.blp"
        );
        assert_eq!(
            scene_source(false),
            (SCENE_ART, MODEL_SERVED_NAME, TEXTURE_DIR)
        );
        assert_ne!(
            texture_served_name_in(r"Textures\SKY2.blp", directory),
            texture_served_name(r"Textures\SKY2.blp")
        );
    }

    #[test]
    fn global_clock_keeps_fractional_milliseconds_across_frames() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_nanos(16_666_667));
        world.insert_resource(time);
        world.insert_resource(Assets::<Mesh>::default());
        world.insert_resource(Assets::<StandardMaterial>::default());
        world.insert_resource(MainMenuScene {
            model: Some(pcx_gfx::LoadedModel {
                name: String::new(),
                minimum_extent: [0.0; 3],
                maximum_extent: [0.0; 3],
                meshes: Vec::new(),
                bone_names: Vec::new(),
                bone_parents: Vec::new(),
                sequences: Vec::new(),
            }),
            duration_ms: 16_667,
            ..default()
        });
        for _ in 0..60 {
            world.run_system_once(advance).unwrap();
        }
        assert_eq!(world.resource::<MainMenuScene>().elapsed.as_millis(), 1_000);
    }

    fn scene_bytes() -> Option<Vec<u8>> {
        let ws = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()?
            .parent()?
            .join("wc3/game/app");
        if !ws.is_dir() {
            return None;
        }
        crate::mainmenu::art_bytes(Some(&ws), SCENE_ART)
    }

    /// The served name is derived, so it is the one thing the page and Rust must
    /// agree on exactly. Case is folded for that reason.
    #[test]
    fn the_served_name_is_lowercased_and_flat() {
        assert_eq!(
            texture_served_name(r"ui\Glues\MainMenu\MainMenu3d\Wetrock1.blp"),
            "menu3d/wetrock1.blp"
        );
        assert_eq!(
            texture_served_name("Textures/Dirt2c.blp"),
            "menu3d/dirt2c.blp"
        );
    }

    /// The scene is the one the FDF names, and its texture list comes from the
    /// model rather than a hand-copy.
    #[test]
    fn the_scene_names_its_own_textures() {
        let Some(bytes) = scene_bytes() else { return };
        let names = texture_served_names(&bytes);
        assert_eq!(names.len(), 33, "the model's TEXS count");
        assert!(
            names
                .iter()
                .all(|n| n.starts_with("menu3d/") && n.ends_with(".blp")),
            "every name is served under menu3d/"
        );
        // No duplicates: a repeated basename would make two geosets share one
        // served name and one of them would draw with the wrong texture.
        let mut sorted = names.clone();
        sorted.sort();
        let before = sorted.len();
        sorted.dedup();
        assert_eq!(sorted.len(), before, "served names are unique");
    }

    /// The clip the background plays is the first looping `Stand`, and it is the
    /// 16,667 ms one the model declares — not a hand-picked duration.
    #[test]
    fn the_entry_clip_is_the_first_looping_stand() {
        let Some(bytes) = scene_bytes() else { return };
        let m = pcx_mdl::Mdlx::parse(&bytes).expect("parses");
        let seqs = m.sequences(&bytes).expect("sequences");
        let stand = seqs
            .iter()
            .find(|s| s.name == "Stand" && !s.non_looping)
            .expect("a looping Stand");
        assert_eq!(stand.interval_start, 0);
        assert_eq!(stand.duration(), 16_667);
    }

    /// Every texture the model names must be one the loader can serve, and the
    /// geometry must be the size the earlier measurement established. This is
    /// the gate that stops "the background renders" being claimed while most of
    /// it is flat-shaded.
    #[test]
    fn every_named_texture_is_readable_and_decodes() {
        let ws = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("wc3/game/app");
        if !ws.is_dir() {
            return;
        }
        let bytes = crate::mainmenu::art_bytes(Some(&ws), SCENE_ART).expect("the scene is present");
        let textures = load_textures(Some(&ws), &bytes);
        assert_eq!(
            textures.len(),
            33,
            "all 33 textures read and decode; got {}",
            textures.len()
        );
    }

    /// The scene's geometry, as measured: 33 geosets and 9,307 vertices.
    #[test]
    fn the_scene_geometry_is_the_measured_size() {
        let Some(bytes) = scene_bytes() else { return };
        let model = pcx_gfx::load_model(&bytes, SCENE_ART).expect("loads");
        assert_eq!(model.meshes.len(), 33, "geosets");
        let verts: usize = model.meshes.iter().map(|m| m.positions.len()).sum();
        assert_eq!(verts, 9_307, "vertices");
    }

    /// The scene's layers really are one per material, which is the assumption
    /// the single-material path rests on. If a model change puts two layers on a
    /// material, this fails rather than the second layer vanishing.
    #[test]
    fn every_scene_material_carries_exactly_one_layer() {
        let Some(bytes) = scene_bytes() else { return };
        let m = pcx_mdl::Mdlx::parse(&bytes).expect("parses");
        let materials = m.materials(&bytes).expect("materials");
        assert_eq!(materials.len(), 31, "the scene's material count");
        for (i, mat) in materials.iter().enumerate() {
            assert_eq!(
                mat.tex_layers().len(),
                1,
                "material {i} has more than one layer, so this path loses one"
            );
        }
    }

    /// The flags the materials actually carry, as measured. These are the
    /// numbers the material construction is built on, so they are pinned rather
    /// than described.
    #[test]
    fn the_layer_flags_are_the_measured_ones() {
        let Some(bytes) = scene_bytes() else { return };
        let m = pcx_mdl::Mdlx::parse(&bytes).expect("parses");
        let materials = m.materials(&bytes).expect("materials");
        let all: Vec<_> = (0..materials.len())
            .map(|i| layer_flags(&materials, i))
            .collect();

        let opaque = all
            .iter()
            .filter(|f| f.alpha_mode == AlphaMode::Opaque)
            .count();
        let blend = all
            .iter()
            .filter(|f| f.alpha_mode == AlphaMode::Blend)
            .count();
        let add = all
            .iter()
            .filter(|f| f.alpha_mode == AlphaMode::Add)
            .count();
        // filterMode 0:18, 1:6, 2:4, 3:1, 4:2 across the 31 layers.
        assert_eq!(opaque, 18, "filterMode 0");
        assert_eq!(blend, 10, "filterMode 1 and 2");
        assert_eq!(add, 3, "filterMode 3 and 4");
        assert_eq!(
            all.iter().filter(|f| f.unshaded).count(),
            4,
            "shadingFlags bit 0"
        );
        // filterMode 1 (x6) alpha-tests and filterMode 3 (x1) is ONE/ONE;
        // filterMode 2 (x4) and 4 (x2) are exact.
        let gaps: Vec<BlendGap> = all.iter().filter_map(|f| f.blend_gap).collect();
        assert_eq!(
            gaps.len(),
            7,
            "six alpha-test plus one non-premultiplied additive"
        );
        assert_eq!(
            gaps.iter()
                .filter(|g| **g == BlendGap::AlphaTestThresholdUnproven)
                .count(),
            6
        );
        assert_eq!(
            gaps.iter()
                .filter(|g| **g == BlendGap::AdditiveIsPremultiplied)
                .count(),
            1
        );

        // The alpha the scene really stores, and the reason the crate's field
        // cannot be used: `TexLayer::static_alpha` reads the `coordId` word.
        let mut alphas: Vec<String> = all.iter().map(|f| format!("{:.2}", f.alpha)).collect();
        alphas.sort();
        alphas.dedup();
        assert_eq!(
            alphas,
            ["0.30", "0.55", "1.00"],
            "the scene's static alphas"
        );
    }

    /// The scene uses the checked decoder's alpha, not the coordinate-id word.
    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen::prelude::wasm_bindgen]
    pub fn menu_sampler_synthetic_check() -> u32 {
        super::texture_selection_tests::same_path_distinct_texs_keep_samplers();
        1
    }

    #[test]
    fn checked_layer_alpha_reaches_scene_materials() {
        let Some(bytes) = scene_bytes() else { return };
        let m = pcx_mdl::Mdlx::parse(&bytes).expect("parses");
        let materials = m.materials(&bytes).expect("materials");
        for mat in &materials {
            let layer = mat.checked_layers().expect("valid layers").remove(0);
            assert_eq!(layer_static_alpha(mat), Some(layer.properties.static_alpha));
            assert!(layer.properties.static_alpha > 0.0);
        }
    }

    /// Every `filterMode` word's mapping, pinned. The three gaps are asserted
    /// as gaps: a change that silently drops one would make the scene look
    /// closer to right while being further from the original.
    #[test]
    fn the_blend_mapping_is_the_vendored_one() {
        assert_eq!(alpha_mode_of(0), (AlphaMode::Opaque, None));
        assert_eq!(
            alpha_mode_of(1),
            (AlphaMode::Blend, Some(BlendGap::AlphaTestThresholdUnproven))
        );
        assert_eq!(alpha_mode_of(2), (AlphaMode::Blend, None));
        assert_eq!(
            alpha_mode_of(3),
            (AlphaMode::Add, Some(BlendGap::AdditiveIsPremultiplied))
        );
        // Mode 4 is `SRC_ALPHA / ONE`, which is exactly what Bevy's `Add` does.
        assert_eq!(alpha_mode_of(4), (AlphaMode::Add, None));
        assert_eq!(alpha_mode_of(5), (AlphaMode::Multiply, None));
        assert_eq!(
            alpha_mode_of(6),
            (AlphaMode::Opaque, Some(BlendGap::Modulate2x))
        );
    }

    /// The camera and lights are the model's, not defaults.
    #[test]
    fn the_scene_supplies_its_own_camera_and_lights() {
        let Some(bytes) = scene_bytes() else { return };
        let m = pcx_mdl::Mdlx::parse(&bytes).expect("parses");
        let cameras = m.cameras(&bytes).expect("cameras");
        assert_eq!(cameras.len(), 1, "the model's one camera");
        assert!(cameras[0].far > cameras[0].near, "a usable clip range");
        let lights = m.lights(&bytes).expect("lights");
        assert!(!lights.is_empty(), "the model carries its own lights");
    }
}

#[cfg(test)]
mod global_seq_tests {
    use super::*;

    #[test]
    fn borrowed_tracks_preserve_clock_fallbacks_and_duplicate_order_bit_for_bit() {
        use pcx_mdl::{Bone, BoneTrack, Interpolation, Pose, TrackKind};
        let mut bone = Bone {
            offset: 0,
            span: 0,
            name: String::new(),
            object_id: 0,
            parent_id: -1,
            flags: 0,
            tracks: vec![],
            geoset_id: -1,
            geoset_anim_id: -1,
        };
        fn previous(bone: &Bone, sequence: u32, elapsed: u32, durations: &[u32]) -> Pose {
            if bone.tracks.iter().all(|t| t.global_sequence_id < 0) {
                return pcx_mdl::sample_bone_pose(bone, sequence);
            }
            let mut pose = Pose::default();
            for track in &bone.tracks {
                let frame = match u32::try_from(track.global_sequence_id) {
                    Ok(id) => match durations.get(id as usize).copied().unwrap_or(0) {
                        0 => sequence,
                        d => elapsed % d,
                    },
                    Err(_) => sequence,
                };
                let one = Bone {
                    tracks: vec![track.clone()],
                    ..bone.clone()
                };
                let sampled = pcx_mdl::sample_bone_pose(&one, frame);
                match track.kind {
                    TrackKind::Translation => pose.translation = sampled.translation,
                    TrackKind::Rotation => pose.rotation = sampled.rotation,
                    TrackKind::Scaling => pose.scale = sampled.scale,
                }
            }
            pose
        }
        fn bits(pose: Pose) -> [u32; 10] {
            let mut bits = [0; 10];
            for (out, value) in bits.iter_mut().zip(
                pose.translation
                    .into_iter()
                    .chain(pose.rotation)
                    .chain(pose.scale),
            ) {
                *out = value.to_bits();
            }
            bits
        }
        for interpolation in [
            Interpolation::None,
            Interpolation::Linear,
            Interpolation::Hermite,
            Interpolation::Bezier,
        ] {
            bone.tracks.clear();
            for (kind, global) in [
                (TrackKind::Translation, -1),
                (TrackKind::Rotation, 0),
                (TrackKind::Scaling, 1),
                (TrackKind::Translation, 3),
            ] {
                let width = if kind == TrackKind::Rotation { 4 } else { 3 };
                let tangents = matches!(
                    interpolation,
                    Interpolation::Hermite | Interpolation::Bezier
                );
                let mut raw = vec![];
                for (frame, value) in [(0_u32, 2_f32), (100, 6.)] {
                    raw.extend(frame.to_le_bytes());
                    for base in [value, -3., 9.]
                        .into_iter()
                        .take(if tangents { 3 } else { 1 })
                    {
                        for component in 0..width {
                            raw.extend((base + component as f32).to_le_bytes());
                        }
                    }
                }
                bone.tracks.push(BoneTrack {
                    kind,
                    count: 2,
                    interpolation,
                    global_sequence_id: global,
                    offset: 0,
                    stride: 4 + width * 4 * if tangents { 3 } else { 1 },
                    raw,
                });
            }
            for durations in [&[][..], &[100, 0][..], &[100, 75, 90, 80][..]] {
                for (sequence, elapsed) in [(0, 0), (50, 125), (100, 200), (101, u32::MAX)] {
                    assert_eq!(
                        bits(tracked_pose(&bone, sequence, elapsed, durations)),
                        bits(previous(&bone, sequence, elapsed, durations))
                    );
                }
            }
            for t in &mut bone.tracks {
                t.global_sequence_id = -1;
            }
            bone.tracks[3].raw[4..8].copy_from_slice(&20_f32.to_le_bytes());
            assert_eq!(tracked_pose(&bone, 0, 125, &[100]).translation[0], 2.);
            bone.tracks[1].global_sequence_id = 0;
            assert_eq!(tracked_pose(&bone, 0, 125, &[100]).translation[0], 20.);
        }
    }

    fn scene() -> Option<(Vec<u8>, pcx_mdl::Mdlx)> {
        let ws = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()?
            .parent()?
            .join("wc3/game/app");
        if !ws.is_dir() {
            return None;
        }
        let bytes = crate::mainmenu::art_bytes(Some(&ws), SCENE_ART)?;
        let m = pcx_mdl::Mdlx::parse(&bytes).ok()?;
        Some((bytes, m))
    }

    /// The number that makes the bug real: more than half the scene's bone
    /// tracks are global, so sampling them on the clip clock is not an edge case.
    #[test]
    fn most_of_the_scenes_tracks_are_global() {
        let Some((bytes, m)) = scene() else { return };
        let bones = m.bones(&bytes).expect("bones");
        let total: usize = bones.iter().map(|b| b.tracks.len()).sum();
        let global = bones
            .iter()
            .flat_map(|b| b.tracks.iter())
            .filter(|t| t.global_sequence_id >= 0)
            .count();
        assert_eq!(total, 482, "bone tracks");
        assert_eq!(global, 258, "global-sequence tracks");
        assert!(global * 2 > total, "they are the majority");
    }

    /// `GLBS` is a plain duration array here, and its length is exactly one more
    /// than the largest id the tracks use.
    #[test]
    fn the_glbs_durations_cover_every_id_used() {
        let Some((bytes, m)) = scene() else { return };
        let durations = pcx_gfx::mdx_runtime::RuntimePlan::parse(&bytes)
            .expect("checked runtime plan")
            .global_sequences;
        assert_eq!(durations.len(), 37, "148 bytes / 4");
        let bones = m.bones(&bytes).expect("bones");
        let max_id = bones
            .iter()
            .flat_map(|b| b.tracks.iter())
            .map(|t| t.global_sequence_id)
            .max()
            .expect("a global id");
        assert_eq!(max_id, 36, "the largest id");
        assert!(durations[max_id as usize] > 0, "and it has a duration");
    }

    /// The lights, pinned: the values that made the blow-out possible.
    ///
    /// An earlier version scaled `intensity` by 200,000 and put every light at
    /// the origin, which drove five point lights to tens of millions of lumens
    /// in one spot — the white, blown-out planes the browser showed. The scale
    /// is now 200 and each light takes its own node pivot.
    #[test]
    fn the_lights_are_the_models_own_and_not_blown_out() {
        let Some((bytes, m)) = scene() else { return };
        let lights = m.lights(&bytes).expect("lights");
        let pivots = m.pivots(&bytes).expect("pivots");
        assert_eq!(lights.len(), 5, "the scene's five lights");
        for l in &lights {
            assert_eq!(l.kind, pcx_mdl::LightKind::Omnidirectional);
            assert!(
                (1.0..=200.0).contains(&l.intensity),
                "{}: intensity {} is the shipped range",
                l.name,
                l.intensity
            );
            // The scale this module applies must keep a light inside the range a
            // scene light can plausibly be, not millions.
            let scaled = l.intensity.max(0.0) * 200.0;
            assert!(scaled <= 40_000.0, "{}: {scaled} lumens", l.name);
            // And every light has a real place to stand, not the origin.
            let p = pivots.position(l.object_id).expect("a pivot");
            assert!(p != [0.0, 0.0, 0.0], "{}: placed at the origin", l.name);
        }
        // Ambient is zero in every record, so the module's floor is what lights
        // the unlit pieces; that floor is not zero.
        assert!(lights.iter().all(|l| l.ambient_intensity == 0.0));
    }
}

#[cfg(test)]
mod texture_selection_tests {
    use super::*;
    use bevy::asset::{AssetApp, AssetEvent, AssetPlugin};
    use bevy::ecs::{message::Messages, world::CommandQueue};
    use bevy::image::{ImageAddressMode, ImageSampler};
    use pcx_gfx::mdx_runtime::{GeosetInput, RuntimeInput, RuntimePlan};
    use pcx_mdl::{Interpolation, Material, NO_ID, Pivots, Texture, TextureFrameTrack};

    fn track(global: u32, keys: &[(i32, u32)]) -> TextureFrameTrack {
        TextureFrameTrack {
            interpolation: Interpolation::None,
            global_seq_id: global,
            keys: keys.to_vec(),
        }
    }
    fn material(base: u32, tracks: &[TextureFrameTrack]) -> Material {
        let mut layer = Vec::new();
        for word in [2_u32, 1, base, NO_ID, 0, 0.25_f32.to_bits()] {
            layer.extend(word.to_le_bytes());
        }
        for track in tracks {
            layer.extend(b"KMTF");
            for word in [
                track.keys.len() as u32,
                track.interpolation as u32,
                track.global_seq_id,
            ] {
                layer.extend(word.to_le_bytes());
            }
            for &(frame, id) in &track.keys {
                layer.extend(frame.to_le_bytes());
                layer.extend(id.to_le_bytes());
            }
        }
        let mut data = vec![0; 8];
        data.extend(b"LAYS");
        data.extend(1_u32.to_le_bytes());
        data.extend((layer.len() as u32 + 4).to_le_bytes());
        data.extend(layer);
        Material {
            offset: 0,
            len: data.len() as u32 + 4,
            data,
        }
    }
    struct Fixture {
        runtime: RuntimePlan,
        bytes: Vec<u8>,
        model: pcx_gfx::LoadedModel,
        textures: Vec<pcx_gfx::LoadedTexture>,
    }
    fn fixture(tracks: &[TextureFrameTrack]) -> Fixture {
        let materials = vec![material(1, tracks), material(7, &[])];
        let mut bytes = b"MDLX".to_vec();
        bytes.extend(b"VERS");
        bytes.extend(4_u32.to_le_bytes());
        bytes.extend(800_u32.to_le_bytes());
        bytes.extend(b"MTLS");
        bytes.extend(materials.iter().map(|m| m.len).sum::<u32>().to_le_bytes());
        for m in &materials {
            bytes.extend(m.len.to_le_bytes());
            bytes.extend(&m.data);
        }
        let textures: Vec<_> = (0..9)
            .map(|id| Texture {
                slot: 0,
                path: match id {
                    1 => "Static.blp",
                    2 | 5 => "Synthetic\\Shared.blp",
                    7 => "Other.blp",
                    _ => "Unused.blp",
                }
                .into(),
                flags: if id == 5 { 3 } else { 0 },
            })
            .collect();
        let mut runtime = RuntimePlan::checked(RuntimeInput {
            bones: vec![],
            helpers: vec![],
            pivots: Pivots::default(),
            geosets: (0..2)
                .map(|id| GeosetInput {
                    vertex_count: 3,
                    material_id: id,
                    matrix_joint_ids: vec![],
                })
                .collect(),
            geoset_animations: vec![],
            materials,
            textures,
            texture_animations: vec![],
            global_sequences: vec![13],
        })
        .unwrap();
        // Each synthetic loaded mesh below declares one authored UV set.
        runtime.uv_set_counts = vec![Some(1); runtime.geosets.len()];
        let model = pcx_gfx::LoadedModel {
            name: "synthetic".into(),
            minimum_extent: [0.; 3],
            maximum_extent: [1.; 3],
            bone_names: vec![],
            bone_parents: vec![],
            sequences: vec![],
            meshes: (0..2)
                .map(|id| pcx_gfx::LoadedMesh {
                    positions: vec![[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]],
                    normals: vec![[0., 0., 1.]; 3],
                    tex_coords: vec![[0., 0.], [1., 0.], [0., 1.]],
                    indices: vec![0, 1, 2],
                    vertex_groups: vec![0; 3],
                    bone_indices: vec![],
                    group_counts: vec![],
                    geoset: id,
                    material_id: id as u32,
                })
                .collect(),
        };
        let textures = [
            ("Static.blp", [255, 0, 0, 255]),
            ("synthetic/shared.blp", [0, 255, 0, 255]),
            ("Other.blp", [0, 0, 255, 255]),
        ]
        .into_iter()
        .map(|(source, rgba)| pcx_gfx::LoadedTexture {
            width: 1,
            height: 1,
            rgba: rgba.to_vec(),
            source: source.into(),
        })
        .collect();
        Fixture {
            runtime,
            bytes,
            model,
            textures,
        }
    }
    struct Spawned {
        world: World,
        meshes: Assets<Mesh>,
        materials: Assets<StandardMaterial>,
        images: Assets<Image>,
        result: Result<Geosets, SceneError>,
    }
    fn spawn_parts(
        source: &str,
        runtime: &RuntimePlan,
        bytes: &[u8],
        model: &pcx_gfx::LoadedModel,
        textures: &[pcx_gfx::LoadedTexture],
        interval: [u32; 2],
    ) -> Spawned {
        let selection = texture_selection(runtime, interval, "synthetic").unwrap();
        let uv_translation =
            pcx_gfx::mdx_uv_translation::UvTranslationPlan::from_runtime(runtime, interval, source)
                .unwrap();
        let opacity = pcx_gfx::mdx_opacity::OpacityPlan::checked(runtime, interval).unwrap();
        let m = pcx_mdl::Mdlx::parse(bytes).unwrap();
        let mut world = World::new();
        let mut queue = CommandQueue::default();
        let mut meshes = Assets::default();
        let mut materials = Assets::default();
        let mut images = Assets::default();
        let result = spawn_geosets(
            &mut Commands::new(&mut queue, &world),
            GeosetAssets {
                meshes: &mut meshes,
                materials: &mut materials,
                images: &mut images,
            },
            GeosetSource {
                m: &m,
                bytes,
                source,
                model,
                textures,
                runtime,
                selection: &selection,
                uv_translation: &uv_translation,
            },
            &opacity,
            interval[0],
            &mut SceneInfo::default(),
            Transform::default(),
        );
        queue.apply(&mut world);
        Spawned {
            world,
            meshes,
            materials,
            images,
            result,
        }
    }
    fn rendered_entities(world: &mut World) -> usize {
        world
            .query::<(&Mesh3d, &MeshMaterial3d<StandardMaterial>)>()
            .iter(world)
            .count()
    }
    fn spawn(f: &Fixture) -> Spawned {
        spawn_parts(
            "synthetic",
            &f.runtime,
            &f.bytes,
            &f.model,
            &f.textures,
            [100, 117],
        )
    }
    fn material_events(app: &mut App) -> Vec<AssetEvent<StandardMaterial>> {
        app.update();
        app.world_mut()
            .resource_mut::<Messages<AssetEvent<StandardMaterial>>>()
            .drain()
            .collect()
    }
    fn event_app(materials: Assets<StandardMaterial>) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<StandardMaterial>();
        app.insert_resource(materials);
        material_events(&mut app);
        app
    }

    #[test]
    pub(super) fn same_path_distinct_texs_keep_samplers() {
        let f = fixture(&[track(0, &[(0, 2), (7, 5)])]);
        let mut spawned = spawn(&f);
        let geosets = spawned.result.unwrap();
        assert_ne!(geosets.textures.images[&2], geosets.textures.images[&5]);
        assert_eq!(
            geosets.textures.materials[0].last_texture, 2,
            "spawn incorrectly bound static TEXS 1"
        );
        assert_eq!(
            spawned
                .materials
                .get(&geosets.bound[0].2)
                .unwrap()
                .base_color_texture
                .as_ref(),
            Some(&geosets.textures.images[&2])
        );
        let a = spawned.images.get(&geosets.textures.images[&2]).unwrap();
        let b = spawned.images.get(&geosets.textures.images[&5]).unwrap();
        assert_eq!(a.data, b.data);
        assert!(matches!(a.sampler, ImageSampler::Default));
        let ImageSampler::Descriptor(s) = &b.sampler else {
            panic!("lost selected TEXS sampler")
        };
        assert_eq!(s.address_mode_u, ImageAddressMode::Repeat);
        assert_eq!(s.address_mode_v, ImageAddressMode::Repeat);
        assert_eq!(rendered_entities(&mut spawned.world), 2);
        assert_eq!(spawned.meshes.len(), 2);
    }

    #[test]
    fn same_canonical_path_and_flags_share_backing_without_merging_texs_ids() {
        let mut f = fixture(&[track(0, &[(0, 2), (7, 5)])]);
        f.runtime.textures[5].flags = f.runtime.textures[2].flags;
        f.runtime.textures[5].path = "SYNTHETIC/SHARED.BLP".into();
        let spawned = spawn(&f);
        let mut geosets = spawned.result.unwrap();
        let mut app = event_app(spawned.materials);
        assert!(geosets.textures.images.contains_key(&2));
        assert!(geosets.textures.images.contains_key(&5));
        assert_eq!(geosets.textures.images.len(), 4);
        assert_eq!(
            spawned.images.len(),
            3,
            "identical sampler/image allocated twice"
        );
        assert_eq!(geosets.textures.images[&2], geosets.textures.images[&5]);
        assert_eq!(geosets.textures.materials[0].last_texture, 2);
        let selection = texture_selection(&f.runtime, [100, 117], "synthetic").unwrap();
        apply_texture_selection(
            &selection,
            &mut geosets.textures,
            103,
            7,
            &mut app.world_mut().resource_mut(),
        )
        .unwrap();
        assert!(
            material_events(&mut app)
                .iter()
                .all(|event| !matches!(event, AssetEvent::Modified { .. })),
            "equal backing image dirtied a material"
        );
        assert_eq!(
            geosets.textures.materials[0].last_texture, 5,
            "backing-image equality collapsed authored selection identity"
        );
        assert_eq!(
            app.world()
                .resource::<Assets<StandardMaterial>>()
                .get(&geosets.bound[0].2)
                .unwrap()
                .base_color_texture
                .as_ref(),
            Some(&geosets.textures.images[&5])
        );
    }

    #[test]
    fn global_clock_and_actual_asset_events_preserve_other_material_fields() {
        let f = fixture(&[track(0, &[(0, 2), (7, 5)])]);
        let spawned = spawn(&f);
        let mut geosets = spawned.result.unwrap();
        let selection = texture_selection(&f.runtime, [100, 117], "synthetic").unwrap();
        let handle = geosets.bound[0].2.clone();
        let other = geosets.bound[1].2.clone();
        let initial = spawned.materials.get(&handle).unwrap().clone();
        let other_image = spawned
            .materials
            .get(&other)
            .unwrap()
            .base_color_texture
            .clone();
        let mut app = event_app(spawned.materials);
        for (elapsed, id, changed) in [
            (6, 2, false),
            (7, 5, true),
            (12, 5, false),
            (13, 2, true),
            (20, 5, true),
            (20, 5, false),
        ] {
            apply_texture_selection(
                &selection,
                &mut geosets.textures,
                103,
                elapsed,
                &mut app.world_mut().resource_mut(),
            )
            .unwrap();
            let events = material_events(&mut app);
            let modified: Vec<_> = events
                .iter()
                .filter(|e| matches!(e, AssetEvent::Modified { .. }))
                .collect();
            assert_eq!(modified.len(), usize::from(changed), "elapsed={elapsed}");
            if changed {
                assert!(modified[0].is_modified(handle.id()));
            }
            let assets = app.world().resource::<Assets<StandardMaterial>>();
            let current = assets.get(&handle).unwrap();
            assert_eq!(
                current.base_color_texture.as_ref(),
                Some(&geosets.textures.images[&id])
            );
            assert_eq!(current.base_color, initial.base_color);
            assert_eq!(current.alpha_mode, initial.alpha_mode);
            assert_eq!(current.unlit, initial.unlit);
            assert_eq!(current.perceptual_roughness, initial.perceptual_roughness);
            assert_eq!(current.cull_mode, initial.cull_mode);
            assert_eq!(assets.get(&other).unwrap().base_color_texture, other_image);
        }
        let expected = selection.sample(0, 103, u64::MAX).unwrap();
        apply_texture_selection(
            &selection,
            &mut geosets.textures,
            103,
            u64::MAX,
            &mut app.world_mut().resource_mut(),
        )
        .unwrap();
        assert_eq!(geosets.textures.materials[0].last_texture, expected);
        assert_ne!(
            selection.sample(0, 103, u64::MAX),
            selection.sample(0, 103, u64::MAX as u32 as u64),
            "synthetic long clock must discriminate truncation"
        );
    }

    #[test]
    fn every_dependency_refuses_before_any_spawn_or_asset_allocation() {
        let base = fixture(&[track(0, &[(0, 2), (7, 5)])]);
        for failure in 0..7 {
            let mut f = fixture(&[track(0, &[(0, 2), (7, 5)])]);
            match failure {
                0 => {
                    f.textures.retain(|t| t.source != "synthetic/shared.blp");
                }
                1 => {
                    f.runtime.textures[5].path = "MissingFuture.blp".into();
                }
                2 => {
                    f.runtime.textures[5].path.clear();
                }
                3 => {
                    f.runtime.textures[5].flags = 4;
                }
                4 => {
                    f.textures[1].width = 0;
                }
                5 => {
                    f.textures[1].rgba.pop();
                }
                6 => {
                    let mut bad = f.textures[1].clone();
                    bad.source = "SYNTHETIC\\SHARED.BLP".into();
                    bad.rgba[0] ^= 1;
                    f.textures.push(bad);
                }
                _ => unreachable!(),
            }
            let mut spawned = spawn(&f);
            let error = spawned.result.err().expect("bad dependency admitted");
            assert!(matches!(error, SceneError::TextureImage { .. }), "{error}");
            assert_eq!(rendered_entities(&mut spawned.world), 0);
            assert!(spawned.materials.is_empty());
            assert!(spawned.meshes.is_empty());
            assert!(spawned.images.is_empty());
        }
        let mut equal = base.textures.clone();
        equal.push(equal[1].clone());
        assert!(
            spawn_parts(
                "synthetic",
                &base.runtime,
                &base.bytes,
                &base.model,
                &equal,
                [100, 117]
            )
            .result
            .is_ok()
        );
    }

    #[test]
    fn failed_update_is_atomic_and_selector_errors_keep_source_context() {
        let f = fixture(&[track(0, &[(0, 2), (7, 5)])]);
        let spawned = spawn(&f);
        let mut g = spawned.result.unwrap();
        let mut materials = spawned.materials;
        let selection = texture_selection(&f.runtime, [100, 117], "synthetic").unwrap();
        let first = g.bound[0].2.clone();
        let before = materials.get(&first).unwrap().base_color_texture.clone();
        materials.remove(&g.bound[1].2);
        let mut app = event_app(materials);
        assert!(
            apply_texture_selection(
                &selection,
                &mut g.textures,
                103,
                7,
                &mut app.world_mut().resource_mut()
            )
            .is_err()
        );
        assert_eq!(
            app.world()
                .resource::<Assets<StandardMaterial>>()
                .get(&first)
                .unwrap()
                .base_color_texture,
            before
        );
        assert_eq!(g.textures.materials[0].last_texture, 2);
        assert!(
            material_events(&mut app)
                .iter()
                .all(|e| !matches!(e, AssetEvent::Modified { .. }))
        );
        let bad = fixture(&[track(0, &[(1, 2), (1, 5)])]);
        let error = texture_selection(&bad.runtime, [100, 117], "source-qualified-model")
            .err()
            .unwrap();
        let SceneError::TextureSelection { source_name, error } = error else {
            panic!("lost typed selector failure")
        };
        assert_eq!(source_name, "source-qualified-model");
        assert!(error.reason.contains("duplicate"));
        assert_eq!(
            elapsed_millis("clock", std::time::Duration::from_millis(u64::MAX)).unwrap(),
            u64::MAX
        );
        assert!(matches!(
            elapsed_millis("clock", std::time::Duration::MAX),
            Err(SceneError::Clock { .. })
        ));
        let mut elapsed = std::time::Duration::from_millis(u64::MAX);
        let before = elapsed;
        assert!(
            advance_elapsed("clock", &mut elapsed, std::time::Duration::from_millis(1)).is_err()
        );
        assert_eq!(elapsed, before, "refused clock changed elapsed");
    }

    #[test]
    fn local_selection_uses_selected_clip_and_static_when_no_keys_apply() {
        let f = fixture(&[track(NO_ID, &[(3, 2), (107, 5), (120, 2)])]);
        let spawned = spawn(&f);
        let g = spawned.result.unwrap();
        assert_eq!(g.textures.materials[0].last_texture, 5);
        let selection = texture_selection(&f.runtime, [200, 217], "synthetic").unwrap();
        assert_eq!(selection.sample(0, 205, u64::MAX), Some(1));
    }
    #[test]
    fn background_system_uses_elapsed_across_the_local_clip_wrap() {
        use bevy::ecs::system::RunSystemOnce;
        let f = fixture(&[track(0, &[(0, 2), (7, 5)])]);
        let mut spawned = spawn(&f);
        let geosets = spawned.result.unwrap();
        let selection = texture_selection(&f.runtime, [100, 117], "background").unwrap();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_millis(20));
        spawned.world.insert_resource(time);
        spawned.world.insert_resource(spawned.meshes);
        spawned.world.insert_resource(spawned.materials);
        spawned.world.insert_resource(MainMenuScene {
            model: Some(f.model),
            texture_selection: Some(selection),
            texture_bindings: geosets.textures,
            interval_start: 100,
            duration_ms: 17,
            ..default()
        });
        spawned.world.run_system_once(advance).unwrap();
        let scene = spawned.world.resource::<MainMenuScene>();
        assert_eq!(scene.elapsed.as_millis(), 20);
        assert_eq!(scene.time_ms, 3.0);
        assert_eq!(scene.texture_bindings.materials[0].last_texture, 5);
        assert_eq!(
            spawned
                .world
                .resource::<Assets<StandardMaterial>>()
                .get(&geosets.bound[0].2)
                .unwrap()
                .base_color_texture
                .as_ref(),
            Some(&scene.texture_bindings.images[&5]),
        );
    }

    #[test]
    fn absent_or_zero_length_local_clip_keeps_global_textures_advancing() {
        use bevy::ecs::system::RunSystemOnce;
        for interval_start in [0, 100] {
            let f = fixture(&[track(0, &[(0, 2), (7, 5)])]);
            let mut spawned = spawn_parts(
                "background",
                &f.runtime,
                &f.bytes,
                &f.model,
                &f.textures,
                [interval_start, interval_start],
            );
            let geosets = spawned.result.unwrap();
            let selection =
                texture_selection(&f.runtime, [interval_start, interval_start], "background")
                    .unwrap();
            let mut time = Time::<()>::default();
            time.advance_by(std::time::Duration::from_millis(7));
            spawned.world.insert_resource(time);
            spawned.world.insert_resource(spawned.meshes);
            spawned.world.insert_resource(spawned.materials);
            spawned.world.insert_resource(MainMenuScene {
                model: Some(f.model),
                texture_selection: Some(selection),
                texture_bindings: geosets.textures,
                interval_start,
                duration_ms: 0,
                ..default()
            });
            spawned.world.run_system_once(advance).unwrap();
            let scene = spawned.world.resource::<MainMenuScene>();
            assert_eq!(scene.elapsed.as_millis(), 7);
            assert_eq!(scene.time_ms, 0.0);
            assert_eq!(scene.interval_start + scene.time_ms as u32, interval_start);
            assert_eq!(scene.texture_bindings.materials[0].last_texture, 5);
            assert_eq!(
                spawned
                    .world
                    .resource::<Assets<StandardMaterial>>()
                    .get(&geosets.bound[0].2)
                    .unwrap()
                    .base_color_texture
                    .as_ref(),
                Some(&scene.texture_bindings.images[&5]),
            );
        }
    }

    #[test]
    fn panel_phase_changes_keep_the_independent_global_clock() {
        let mut f = fixture(&[track(NO_ID, &[(105, 2), (205, 5)])]);
        f.runtime.materials[1].layers[0].texture_tracks = vec![track(0, &[(0, 2), (7, 5)])];
        let birth = texture_selection(&f.runtime, [100, 110], "panel").unwrap();
        let stand = texture_selection(&f.runtime, [200, 217], "panel").unwrap();
        let timeline = crate::menu_sprites::Timeline {
            birth: [100, 110],
            stand: [200, 217],
            stand_loops: true,
        };
        let spawned = spawn_parts(
            "synthetic",
            &f.runtime,
            &f.bytes,
            &f.model,
            &f.textures,
            [100, 110],
        );
        let mut g = spawned.result.unwrap();
        let mut materials = spawned.materials;
        let mut elapsed = std::time::Duration::ZERO;
        assert_eq!(
            advance_elapsed("panel", &mut elapsed, std::time::Duration::from_millis(6)).unwrap(),
            6
        );
        let selected = timeline.texture_selection(elapsed.as_secs_f64() * 1000., &birth, &stand);
        apply_texture_selection(selected, &mut g.textures, 106, 6, &mut materials).unwrap();
        assert_eq!(g.textures.materials[0].last_texture, 2);
        let ms =
            advance_elapsed("panel", &mut elapsed, std::time::Duration::from_millis(11)).unwrap();
        let selected = timeline.texture_selection(elapsed.as_secs_f64() * 1000., &birth, &stand);
        apply_texture_selection(selected, &mut g.textures, 207, ms, &mut materials).unwrap();
        assert_eq!(
            g.textures.materials[0].last_texture, 5,
            "Stand did not choose its local keys"
        );
        assert_eq!(
            g.textures.materials[1].last_texture, 2,
            "global clock reset at Birth/Stand"
        );
        assert_ne!(
            selected.sample(1, 207, ms),
            selected.sample(1, 207, ms - 10)
        );
    }

    fn uv_track(global: u32, keys: &[(u32, [f32; 3])]) -> pcx_mdl::KeyTrack {
        let raw = keys
            .iter()
            .flat_map(|&(frame, value)| {
                frame
                    .to_le_bytes()
                    .into_iter()
                    .chain(value.into_iter().flat_map(f32::to_le_bytes))
            })
            .collect();
        pcx_mdl::KeyTrack {
            magic: *b"KTAT",
            count: keys.len() as u32,
            interpolation: Interpolation::Linear,
            global_seq_id: global,
            width: 3,
            stride: 16,
            raw,
        }
    }

    fn uv_fixture() -> Fixture {
        let mut f = fixture(&[track(0, &[(0, 2), (7, 5)])]);
        f.runtime.texture_animations = vec![pcx_mdl::TextureAnim {
            offset: 0,
            len: 4,
            tracks: vec![uv_track(0, &[(0, [2., 4., 99.]), (12, [14., 28., -99.])])],
        }];
        f.runtime.materials[0].layers[0].properties.texture_anim_id = 0;
        f
    }

    #[test]
    fn uv_spawner_refuses_a_missing_initial_sample_before_model_writes() {
        let f = uv_fixture();
        let mut empty = fixture(&[]).runtime;
        empty.geosets.clear();
        empty.uv_set_counts.clear();
        let uv = pcx_gfx::mdx_uv_translation::UvTranslationPlan::from_runtime(
            &empty,
            [100, 117],
            "empty-plan",
        )
        .unwrap();
        let selection = texture_selection(&f.runtime, [100, 117], "synthetic-uv").unwrap();
        let opacity = pcx_gfx::mdx_opacity::OpacityPlan::checked(&f.runtime, [100, 117]).unwrap();
        let model = pcx_mdl::Mdlx::parse(&f.bytes).unwrap();
        let mut world = World::new();
        let mut queue = CommandQueue::default();
        let mut meshes = Assets::default();
        let mut materials = Assets::default();
        let mut images = Assets::default();
        let error = spawn_geosets(
            &mut Commands::new(&mut queue, &world),
            GeosetAssets {
                meshes: &mut meshes,
                materials: &mut materials,
                images: &mut images,
            },
            GeosetSource {
                m: &model,
                bytes: &f.bytes,
                source: "initial-uv-source",
                model: &f.model,
                textures: &f.textures,
                runtime: &f.runtime,
                selection: &selection,
                uv_translation: &uv,
            },
            &opacity,
            100,
            &mut SceneInfo::default(),
            Transform::default(),
        )
        .err()
        .expect("missing initial UV sample admitted");
        queue.apply(&mut world);
        assert!(
            error
                .to_string()
                .contains("initial-uv-source geoset 0 material 0 layer 0")
        );
        assert_eq!(rendered_entities(&mut world), 0);
        assert!(meshes.is_empty());
        assert!(materials.is_empty());
        assert!(images.is_empty());
    }

    #[test]
    fn uv_binding_is_absolute_changed_only_and_preserves_other_material_state() {
        let f = uv_fixture();
        let spawned = spawn(&f);
        let mut geosets = spawned.result.unwrap();
        let handle = geosets.bound[0].2.clone();
        let other = geosets.bound[1].2.clone();
        let initial = spawned.materials.get(&handle).unwrap().clone();
        assert_eq!(initial.uv_transform.translation, Vec2::new(2., 4.));
        assert_eq!(initial.uv_transform.matrix2, bevy::math::Mat2::IDENTITY);
        let sampler = spawned
            .images
            .get(&geosets.textures.images[&2])
            .unwrap()
            .sampler
            .clone();
        let uv = pcx_gfx::mdx_uv_translation::UvTranslationPlan::from_runtime(
            &f.runtime,
            [100, 117],
            "synthetic-uv",
        )
        .unwrap();
        let mut app = event_app(spawned.materials);
        for (elapsed, expected, changed) in [
            (0, Vec2::new(2., 4.), false),
            (6, Vec2::new(8., 16.), true),
            (6, Vec2::new(8., 16.), false),
            (12, Vec2::new(14., 28.), true),
            (13, Vec2::new(2., 4.), true),
            (13, Vec2::new(2., 4.), false),
        ] {
            apply_uv_translation(
                &uv,
                &geosets.textures,
                103,
                elapsed,
                &mut app.world_mut().resource_mut(),
            )
            .unwrap();
            let events = material_events(&mut app);
            let modified: Vec<_> = events
                .iter()
                .filter(|event| matches!(event, AssetEvent::Modified { .. }))
                .collect();
            assert_eq!(modified.len(), usize::from(changed), "elapsed={elapsed}");
            if changed {
                assert!(modified[0].is_modified(handle.id()));
            }
            let materials = app.world().resource::<Assets<StandardMaterial>>();
            let actual = materials.get(&handle).unwrap();
            assert_eq!(actual.uv_transform.translation, expected);
            assert_eq!(
                actual.uv_transform.transform_point2(Vec2::new(0.25, 0.75)),
                Vec2::new(0.25, 0.75) + expected
            );
            assert_eq!(actual.uv_transform.matrix2, bevy::math::Mat2::IDENTITY);
            assert_eq!(actual.base_color_texture, initial.base_color_texture);
            assert_eq!(actual.base_color, initial.base_color);
            assert_eq!(actual.alpha_mode, initial.alpha_mode);
            assert_eq!(actual.unlit, initial.unlit);
            assert_eq!(actual.cull_mode, initial.cull_mode);
            assert_eq!(actual.perceptual_roughness, initial.perceptual_roughness);
            assert_eq!(
                materials.get(&other).unwrap().uv_transform,
                bevy::math::Affine2::IDENTITY
            );
            assert_eq!(geosets.textures.materials[0].last_texture, 2);
        }
        let texture = texture_selection(&f.runtime, [100, 117], "synthetic-uv").unwrap();
        apply_texture_selection(
            &texture,
            &mut geosets.textures,
            103,
            7,
            &mut app.world_mut().resource_mut(),
        )
        .unwrap();
        let current = app
            .world()
            .resource::<Assets<StandardMaterial>>()
            .get(&handle)
            .unwrap();
        assert_eq!(geosets.textures.materials[0].last_texture, 5);
        assert_eq!(current.uv_transform.translation, Vec2::new(2., 4.));
        assert_eq!(
            spawned
                .images
                .get(&geosets.textures.images[&2])
                .unwrap()
                .sampler,
            sampler
        );
    }

    #[test]
    fn uv_binding_failure_is_atomic_and_recovers_when_the_target_returns() {
        let f = uv_fixture();
        let spawned = spawn(&f);
        let geosets = spawned.result.unwrap();
        let first = geosets.bound[0].2.clone();
        let second = geosets.bound[1].2.clone();
        let uv = pcx_gfx::mdx_uv_translation::UvTranslationPlan::from_runtime(
            &f.runtime,
            [100, 117],
            "synthetic-uv",
        )
        .unwrap();
        let mut materials = spawned.materials;
        let replacement = materials.remove(&second).unwrap();
        let before = materials.get(&first).unwrap().uv_transform;
        let mut app = event_app(materials);
        let error = apply_uv_translation(
            &uv,
            &geosets.textures,
            103,
            6,
            &mut app.world_mut().resource_mut(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("geoset 1 material 1 layer 0"));
        assert_eq!(
            app.world()
                .resource::<Assets<StandardMaterial>>()
                .get(&first)
                .unwrap()
                .uv_transform,
            before
        );
        assert!(
            material_events(&mut app)
                .iter()
                .all(|event| !matches!(event, AssetEvent::Modified { .. }))
        );
        app.world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .insert(second.id(), replacement)
            .unwrap();
        material_events(&mut app);
        apply_uv_translation(
            &uv,
            &geosets.textures,
            103,
            6,
            &mut app.world_mut().resource_mut(),
        )
        .unwrap();
        assert_eq!(
            app.world()
                .resource::<Assets<StandardMaterial>>()
                .get(&first)
                .unwrap()
                .uv_transform
                .translation,
            Vec2::new(8., 16.)
        );
    }

    #[test]
    fn uv_sample_failure_refuses_before_any_material_write() {
        let mut f = uv_fixture();
        f.runtime.texture_animations.push(pcx_mdl::TextureAnim {
            offset: 0,
            len: 4,
            tracks: vec![uv_track(0, &[(0, [f32::MAX; 3]), (12, [-f32::MAX; 3])])],
        });
        f.runtime.materials[1].layers[0].properties.texture_anim_id = 1;
        let spawned = spawn(&f);
        let geosets = spawned.result.unwrap();
        let first = geosets.bound[0].2.clone();
        let before = spawned.materials.get(&first).unwrap().uv_transform;
        let uv = pcx_gfx::mdx_uv_translation::UvTranslationPlan::from_runtime(
            &f.runtime,
            [100, 117],
            "synthetic-overflow",
        )
        .unwrap();
        let mut app = event_app(spawned.materials);
        let error = apply_uv_translation(
            &uv,
            &geosets.textures,
            103,
            6,
            &mut app.world_mut().resource_mut(),
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("synthetic-overflow geoset 1 material 1 layer 0 TXAN 1")
        );
        assert_eq!(
            app.world()
                .resource::<Assets<StandardMaterial>>()
                .get(&first)
                .unwrap()
                .uv_transform,
            before
        );
        assert!(
            material_events(&mut app)
                .iter()
                .all(|event| !matches!(event, AssetEvent::Modified { .. }))
        );
    }

    #[test]
    fn uv_panel_phase_selection_preserves_the_continuous_global_clock() {
        let mut f = uv_fixture();
        f.runtime.texture_animations[0].tracks = vec![uv_track(
            NO_ID,
            &[(105, [2., 4., 0.]), (205, [8., 16., 0.])],
        )];
        f.runtime.texture_animations.push(pcx_mdl::TextureAnim {
            offset: 0,
            len: 4,
            tracks: vec![uv_track(0, &[(0, [2., 4., 0.]), (12, [14., 28., 0.])])],
        });
        f.runtime.materials[1].layers[0].properties.texture_anim_id = 1;
        let birth = pcx_gfx::mdx_uv_translation::UvTranslationPlan::from_runtime(
            &f.runtime,
            [100, 110],
            "panel",
        )
        .unwrap();
        let stand = pcx_gfx::mdx_uv_translation::UvTranslationPlan::from_runtime(
            &f.runtime,
            [200, 217],
            "panel",
        )
        .unwrap();
        let timeline = crate::menu_sprites::Timeline {
            birth: [100, 110],
            stand: [200, 217],
            stand_loops: true,
        };
        let spawned = spawn_parts(
            "panel",
            &f.runtime,
            &f.bytes,
            &f.model,
            &f.textures,
            [100, 110],
        );
        let geosets = spawned.result.unwrap();
        let mut materials = spawned.materials;
        let mut elapsed = std::time::Duration::ZERO;
        let ms =
            advance_elapsed("panel", &mut elapsed, std::time::Duration::from_millis(6)).unwrap();
        apply_uv_translation(
            timeline.uv_translation(elapsed.as_secs_f64() * 1000., &birth, &stand),
            &geosets.textures,
            106,
            ms,
            &mut materials,
        )
        .unwrap();
        assert_eq!(
            materials
                .get(&geosets.bound[0].2)
                .unwrap()
                .uv_transform
                .translation,
            Vec2::new(2., 4.)
        );
        let ms =
            advance_elapsed("panel", &mut elapsed, std::time::Duration::from_millis(11)).unwrap();
        apply_uv_translation(
            timeline.uv_translation(elapsed.as_secs_f64() * 1000., &birth, &stand),
            &geosets.textures,
            207,
            ms,
            &mut materials,
        )
        .unwrap();
        assert_eq!(
            materials
                .get(&geosets.bound[0].2)
                .unwrap()
                .uv_transform
                .translation,
            Vec2::new(8., 16.)
        );
        assert_eq!(
            materials
                .get(&geosets.bound[1].2)
                .unwrap()
                .uv_transform
                .translation,
            Vec2::new(6., 12.)
        );
    }

    #[test]
    fn background_uv_system_uses_elapsed_across_the_local_clip_wrap() {
        use bevy::ecs::system::RunSystemOnce;
        let f = uv_fixture();
        let mut spawned = spawn(&f);
        let geosets = spawned.result.unwrap();
        let uv_translation = pcx_gfx::mdx_uv_translation::UvTranslationPlan::from_runtime(
            &f.runtime,
            [100, 117],
            "background",
        )
        .unwrap();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_millis(20));
        spawned.world.insert_resource(time);
        spawned.world.insert_resource(spawned.meshes);
        spawned.world.insert_resource(spawned.materials);
        spawned.world.insert_resource(MainMenuScene {
            model: Some(f.model),
            uv_translation: Some(uv_translation),
            texture_bindings: geosets.textures,
            interval_start: 100,
            duration_ms: 17,
            ..default()
        });
        spawned.world.run_system_once(advance).unwrap();
        let scene = spawned.world.resource::<MainMenuScene>();
        assert_eq!(scene.elapsed.as_millis(), 20);
        assert_eq!(scene.time_ms, 3.0);
        assert_eq!(
            spawned
                .world
                .resource::<Assets<StandardMaterial>>()
                .get(&geosets.bound[0].2)
                .unwrap()
                .uv_transform
                .translation,
            Vec2::new(9., 18.)
        );
    }

    #[test]
    #[ignore = "requires explicit retained fixture manifest with source-qualified local members"]
    fn retained_first_layer_handles_follow_authored_global_boundaries() {
        let manifest = std::env::var("PEACECRAFT3_TEXTURE_BINDING_FIXTURE")
            .expect("set explicit fixture manifest");
        let rows = std::fs::read_to_string(manifest).unwrap();
        let rows: Vec<_> = rows
            .lines()
            .map(|line| line.split('\t').collect::<Vec<_>>())
            .collect();
        let norm = |path: &str| path.replace('\\', "/").to_ascii_lowercase();
        let mut animated_channels = 0;
        for model_row in rows.iter().filter(|r| r[0] == "M") {
            let source = model_row[1];
            let raw = std::fs::read(model_row[2]).unwrap();
            let runtime = RuntimePlan::parse(&raw).unwrap();
            let original_materials = runtime.materials.clone();
            let model = pcx_gfx::load_model(&raw, source).unwrap();
            let mdl = pcx_mdl::Mdlx::parse(&raw).unwrap();
            let clips = mdl.sequences(&raw).unwrap();
            let clip = clips
                .iter()
                .find(|c| c.name == "Stand" && !c.non_looping)
                .or_else(|| clips.first())
                .expect("model has a source clip");
            let interval = [clip.interval_start, clip.interval_end];
            let selection = texture_selection(&runtime, interval, source).unwrap();
            let ids: std::collections::BTreeSet<_> = runtime
                .geosets
                .iter()
                .flat_map(|g| {
                    let l = &runtime.materials[g.material_id].layers[0];
                    std::iter::once(l.properties.texture_id).chain(
                        l.texture_tracks
                            .iter()
                            .flat_map(|t| t.keys.iter().map(|&(_, id)| id)),
                    )
                })
                .collect();
            let textures: Vec<_> = ids
                .iter()
                .map(|&id| {
                    let path = &runtime.textures[id as usize].path;
                    let row = rows
                        .iter()
                        .find(|r| r[0] == "T" && norm(r[1]) == norm(path))
                        .expect("manifest has required canonical texture");
                    pcx_gfx::load_image(&std::fs::read(row[2]).unwrap(), path).unwrap()
                })
                .collect();
            let mut spawned = spawn_parts(source, &runtime, &raw, &model, &textures, interval);
            let mut geosets = spawned.result.unwrap();
            let mut materials = spawned.materials;
            assert_eq!(rendered_entities(&mut spawned.world), model.meshes.len());
            for binding in &runtime.geosets {
                let layer = &runtime.materials[binding.material_id].layers[0];
                for track in &layer.texture_tracks {
                    if track.global_seq_id == NO_ID || track.keys.is_empty() {
                        continue;
                    }
                    animated_channels += 1;
                    let duration =
                        u64::from(runtime.global_sequences[track.global_seq_id as usize]);
                    let mut ticks = std::collections::BTreeSet::from([
                        0,
                        duration - 1,
                        duration,
                        duration + 1,
                        u64::MAX,
                    ]);
                    for &(key, _) in &track.keys {
                        let key = u64::try_from(key).unwrap();
                        ticks.insert(key);
                        ticks.insert(key.saturating_sub(1));
                        ticks.insert(key + 1);
                        ticks.insert(duration + key);
                    }
                    for elapsed in ticks {
                        apply_texture_selection(
                            &selection,
                            &mut geosets.textures,
                            interval[0],
                            elapsed,
                            &mut materials,
                        )
                        .unwrap();
                        let at = (elapsed % duration) as i64;
                        let expected = track
                            .keys
                            .iter()
                            .rev()
                            .find(|&&(time, _)| i64::from(time) <= at)
                            .or_else(|| track.keys.first())
                            .unwrap()
                            .1;
                        let bound = geosets
                            .textures
                            .materials
                            .iter()
                            .find(|b| b.geoset == binding.geoset_id)
                            .unwrap();
                        assert_eq!(
                            bound.last_texture, expected,
                            "{source} GEOS {} elapsed {elapsed}",
                            binding.geoset_id
                        );
                        assert_eq!(
                            materials
                                .get(&bound.material)
                                .unwrap()
                                .base_color_texture
                                .as_ref(),
                            Some(&geosets.textures.images[&expected])
                        );
                    }
                    println!(
                        "source={source} geoset={} material={} source_keys={} global_duration={duration} initial_static_id={} actual_first_id={}",
                        binding.geoset_id,
                        binding.material_id,
                        track.keys.len(),
                        layer.properties.texture_id,
                        selection.sample(binding.geoset_id, interval[0], 0).unwrap()
                    );
                }
            }
            assert_eq!(
                runtime.materials, original_materials,
                "source tracks mutated"
            );
            println!(
                "source={source} geosets={} required_texs_ids={}",
                model.meshes.len(),
                ids.len()
            );
        }
        assert!(
            animated_channels > 0,
            "fixture did not exercise an authored global selection channel"
        );
    }
}

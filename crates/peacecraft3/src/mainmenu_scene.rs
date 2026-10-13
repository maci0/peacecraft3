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
//! * **`TXAN` texture animation is not applied.** Its three-component track to
//!   UV-matrix mapping remains unproved; see `menu-texture-animation.md`.
//!   GEOA/KMTA opacity is applied by authored geoset/layer IDs and clocks.
//!   Joint tracks also use authored `GLBS` durations on a separate elapsed clock.
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
/// `menu3d/<lowercased file name>`. [`texture_served_name`] is the single place
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
    fn base_color(&self, has_texture: bool) -> Color {
        if has_texture {
            Color::srgba(1.0, 1.0, 1.0, self.alpha.clamp(0.0, 1.0))
        } else {
            Color::srgba(0.5, 0.5, 0.5, self.alpha.clamp(0.0, 1.0))
        }
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
/// menu art uses. A texture that cannot be read or decoded is skipped and its
/// path is reported by [`spawn`] as a missing texture, so a gap shows up as a
/// flat-shaded geoset and a name in the list rather than as silence.
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
    /// Texture paths no loaded image matched. Every one is a geoset drawing
    /// flat-shaded, so the list is the exact size of the gap.
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
/// `textures` are the decoded images to bind; a geoset whose material names a
/// path no image matches draws flat-shaded and the path lands in
/// [`SceneInfo::missing_textures`].
///
/// The camera and lights come from the model. The camera is given `order: -1`
/// so it draws **before** the UI camera; see the module docs and [`layer`] for
/// what the caller has to change on the UI side.
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
        .map_err(|e| SceneError::Decode(e.to_string()))?;

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

    // One material per geoset, from that geoset's own `material_id`, and from
    // the flags the layer actually carries.
    let paths = pcx_gfx::model_texture_paths(bytes, source).unwrap_or_default();
    let mdl_materials = m.materials(bytes).unwrap_or_default();
    // The model's own texture flags, by path, so each geoset's texture repeats
    // exactly as authored rather than as this module decides.
    let authored: Vec<(String, u32)> = m
        .textures(bytes)
        .unwrap_or_default()
        .into_iter()
        .map(|t| (t.path, t.flags))
        .collect();
    let mut handles = Vec::with_capacity(model.meshes.len());
    let mut opacity_materials = Vec::with_capacity(model.meshes.len());
    for mesh in &model.meshes {
        let id = mesh.material_id as usize;
        let path = paths.get(id).cloned().unwrap_or_default();
        let flags = layer_flags(&mdl_materials, id);
        if let Some(gap) = flags.blend_gap {
            warn!(
                material_id = id,
                reason = gap.reason(),
                "Original menu material rendering gap"
            );
            info.blend_gaps.push(gap);
        }
        let texture = pick(textures, &path);
        // The texture's own `TEXS` flags decide whether it repeats. This scene's
        // UVs run far outside `[0, 1]` — its sky spans about `U -2.6 .. 4.3` —
        // so a clamp smears one row and column across the surface, which is what
        // the streaked ice and water were. `rgba_image` is left alone; the
        // sampler is set on the image it returns, so a caller that does not know
        // about flags keeps the default it had.
        let handle = texture.map(|t| {
            let mut image = pcx_gfx::viewer::rgba_image(t);
            image.sampler =
                pcx_gfx::viewer::sampler_for_texture_flags(texture_flags(&authored, &path));
            images.add(image)
        });
        if texture.is_none() && !path.is_empty() {
            info.missing_textures.push(path.clone());
        }
        let alpha = opacity
            .sample(mesh.geoset as usize, interval[0], 0)
            .ok_or_else(|| {
                SceneError::Decode(format!(
                    "missing opacity binding for geoset {}",
                    mesh.geoset
                ))
            })?;
        let material = materials.add(StandardMaterial {
            base_color: flags.base_color(texture.is_some()).with_alpha(alpha),
            base_color_texture: handle,
            // The glue scene is scenery, not a lit character, so a high
            // roughness keeps it from picking up a sheen the original has not
            // got.
            perceptual_roughness: 1.0,
            // `Unshaded` in the layer's shading flags. Four of the scene's 31
            // layers carry it (`0x21` and `0xA1`), and those are the cloud and
            // sky pieces that the original draws without lighting.
            unlit: flags.unshaded,
            // `filterMode`. Eighteen layers are opaque, ten blend, three add.
            alpha_mode: flags.alpha_mode,
            // Culling is left off on purpose: the bit that means `TwoSided` in
            // this format is not established by measurement here (the scene's
            // flags are `0x00`, `0x10`, `0x21`, `0x90`, `0xA1` and none of them
            // is proven to be it), and guessing wrong makes a geoset vanish
            // rather than merely look different. See the module docs.
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
                MainMenuSceneEntity,
            ))
            .id();
        opacity_materials.push((mesh.geoset as usize, entity, material));
        info.geosets += 1;
    }

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
        interval_start: clip.map(|s| s.interval_start).unwrap_or(0),
        duration_ms: clip.map(|s| s.duration()).unwrap_or(0),
        time_ms: 0.0,
        elapsed: std::time::Duration::ZERO,
        global_durations: global_sequence_durations(bytes),
    });
    info!(bones = info.bones, helpers = info.helpers, sequence = %info.sequence, global_sequence_tracks = info.global_sequence_tracks, "Original menu animation source");
    Ok(info)
}

/// Marker for the camera this module owns, so a caller can find and remove it.
#[derive(Component)]
pub struct MainMenuSceneCamera;

/// All entities owned by a scene, so changing expansion removes the old scene.
#[derive(Component)]
pub struct MainMenuSceneEntity;

/// The authored `TEXS` flags for a texture path, or `0` when the model does not
/// name that path.
///
/// Paths are compared with either separator and without case, the same rule
/// [`pick`] uses, so a lookup here cannot disagree with which image was chosen.
fn texture_flags(authored: &[(String, u32)], path: &str) -> u32 {
    let norm = |s: &str| s.replace('/', "\\").to_ascii_lowercase();
    let want = norm(path);
    authored
        .iter()
        .find(|(p, _)| norm(p) == want)
        .map(|(_, f)| *f)
        .unwrap_or(0)
}

/// Finds the loaded texture whose source path matches `path`.
///
/// Both separators are accepted because the archives use backslashes while a
/// caller may hold forward slashes, and the comparison is case-insensitive
/// because the same file is spelled both ways in the archives.
fn pick<'a>(
    textures: &'a [pcx_gfx::LoadedTexture],
    path: &str,
) -> Option<&'a pcx_gfx::LoadedTexture> {
    if path.is_empty() {
        return None;
    }
    let norm = |s: &str| s.replace('/', "\\").to_ascii_lowercase();
    let want = norm(path);
    textures
        .iter()
        .find(|t| norm(&t.source) == want)
        .or_else(|| textures.iter().find(|t| norm(&t.source).ends_with(&want)))
}

/// The `GLBS` global-sequence durations, in milliseconds, indexed by id.
///
/// The chunk is a **plain array of `u32` durations with no count word**:
/// `MainMenu3d`'s is 148 bytes, which is exactly 37 words, and the largest
/// global-sequence id any of its 482 bone tracks references is **36** — 37
/// values, indexed from zero. `pcx-mdl`'s own note says the id "resolves against
/// the `GLBS` count", but this model stores no count and reading its first word
/// as one gives 144,967, which is not a sequence count.
///
/// This is read here rather than through the crate because the crate exposes no
/// accessor at all — see [`tracked_pose`] for why that matters.
#[must_use]
pub fn global_sequence_durations(bytes: &[u8]) -> Vec<u32> {
    let mut i = 4usize;
    while i + 8 <= bytes.len() {
        let tag = &bytes[i..i + 4];
        let size =
            u32::from_le_bytes([bytes[i + 4], bytes[i + 5], bytes[i + 6], bytes[i + 7]]) as usize;
        if tag == b"GLBS" {
            let body = &bytes[i + 8..(i + 8 + size).min(bytes.len())];
            return body
                .chunks_exact(4)
                .map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
                .collect();
        }
        i += 8 + size;
    }
    Vec::new()
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
/// is sampled through `sample_bone_pose` on a copy of the bone carrying only
/// that track, so Hermite and Bézier easing are still the decoder's.
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
        let one = pcx_mdl::Bone {
            tracks: vec![track.clone()],
            ..bone.clone()
        };
        let pose = pcx_mdl::sample_bone_pose(&one, frame);
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
    if scene.duration_ms == 0 || scene.model.is_none() {
        return;
    }
    // Wrap at the clip's own length so the loop is seamless.
    let dt_ms = time.delta_secs() * 1000.0;
    scene.time_ms = (scene.time_ms + dt_ms) % scene.duration_ms as f32;
    // The elapsed clock does *not* wrap with the clip: a global sequence keeps
    // running across a clip wrap, which is the whole point of it being global.
    scene.elapsed += time.delta();
    let frame = scene.interval_start + scene.time_ms as u32;
    if let Some(opacity) = &scene.opacity {
        for (geoset, entity, handle) in &scene.opacity_materials {
            if let Some(alpha) = opacity.sample(*geoset, frame, scene.elapsed.as_millis() as u64) {
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
    }

    let Some(model) = scene.model.as_ref() else {
        return;
    };
    let Some(pivots) = scene.pivots.as_ref() else {
        return;
    };
    let poses: Vec<pcx_mdl::Pose> = scene
        .bones
        .iter()
        .map(|b| {
            tracked_pose(
                b,
                frame,
                scene.elapsed.as_millis() as u32,
                &scene.global_durations,
            )
        })
        .collect();
    let skeleton = pcx_gfx::pose_from_poses(model, &scene.bones, pivots, &poses);

    for (handle, rest) in scene.handles.iter().zip(&model.meshes) {
        let Some(mut mesh) = meshes.get_mut(handle) else {
            continue;
        };
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            pcx_gfx::skin_mesh(rest, &skeleton),
        );
    }
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
                &[],
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
        let durations = global_sequence_durations(&bytes);
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

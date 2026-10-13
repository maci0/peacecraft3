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
//!   framing by `pcx-mdl` (its 171-byte prelude is deliberately kept as raw
//!   bytes because the field meanings are not proven), so there is nothing
//!   correct to render from it. No stand-in particle effect is created.
//! * **`TXAN` texture animation and `GLBS` global sequences are not applied.**
//!   A material that animates its UVs or its alpha draws at its base state.
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
//!   projection is Bevy's; the eye, target, vertical FOV and clip planes are
//!   the model's own.

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
    // `shader_id` is the format's `filterMode`; `blend_mode` is its
    // `shadingFlags`. See `LayerFlags`' doc comment for why the values settle it.
    let (alpha_mode, blend_gap) = alpha_mode_of(layer.shader_id);
    LayerFlags {
        unshaded: layer.blend_mode & 0x01 != 0,
        alpha_mode,
        alpha: layer_static_alpha(mat).unwrap_or(1.0),
        blend_gap,
    }
}

/// The layer's static alpha, read at its true offset.
///
/// `pcx_mdl::TexLayer::static_alpha` reads `LAYS+28`, which is the layer's
/// `coordId` word — the format has a `coordId` between `textureAnimId` and the
/// alpha, and this workspace's record omits it — so the crate's field reads `0`
/// for every shipped layer. The real value is at `LAYS+32`.
///
/// Measured for `MainMenu3d`, the 31 layers carry `0.3`, `0.55` or `1.0`, so
/// taking the crate's field would have faded eight materials to invisible. The
/// logo's single layer reads `1.0` here and `0` there.
#[must_use]
pub fn layer_static_alpha(mat: &pcx_mdl::Material) -> Option<f32> {
    let at = *mat.layers().first()?;
    let b = mat.data.get(at + 32..at + 36)?;
    Some(f32::from_le_bytes(b.try_into().ok()?))
}

/// The served name for one of the scene's own texture paths.
///
/// Lower-cased so the page and this function cannot disagree about case; the
/// archives spell the same file `Wetrock1.blp` and `wetrock1.blp` in different
/// places and `browser::normalize` preserves case.
#[must_use]
pub fn texture_served_name(wc3_path: &str) -> String {
    let file = wc3_path
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(wc3_path)
        .to_ascii_lowercase();
    format!("{TEXTURE_DIR}/{file}")
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
pub fn texture_served_names(bytes: &[u8]) -> Vec<String> {
    texture_paths(bytes)
        .unwrap_or_default()
        .iter()
        .map(|p| texture_served_name(p))
        .collect()
}

/// The scene model's own bytes, from the install or the browser store.
///
/// This is the one call the launcher needs to get the background's geometry; it
/// resolves the desktop archive first and the page's registered copy second,
/// exactly like the rest of the menu art.
#[must_use]
pub fn load_model_bytes(desktop: Option<&std::path::Path>) -> Option<Vec<u8>> {
    crate::mainmenu::art_bytes_named(desktop, SCENE_ART, MODEL_SERVED_NAME)
}

/// Loads and decodes every texture the model names.
///
/// Desktop first, browser store second — the same two sources the rest of the
/// menu art uses. A texture that cannot be read or decoded is skipped and its
/// path is reported by [`spawn`] as a missing texture, so a gap shows up as a
/// flat-shaded geoset and a name in the list rather than as silence.
pub fn load_textures(
    desktop: Option<&std::path::Path>,
    bytes: &[u8],
) -> Vec<pcx_gfx::LoadedTexture> {
    let mut out = Vec::new();
    for path in texture_paths(bytes).unwrap_or_default() {
        let served = texture_served_name(&path);
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
    /// Bone records in model order.
    bones: Vec<pcx_mdl::Bone>,
    /// The model's pivot table.
    pivots: Option<pcx_mdl::Pivots>,
    /// One Bevy mesh per geoset, parallel to `model.meshes`.
    handles: Vec<Handle<Mesh>>,
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
    /// What was built, for the caller to report.
    pub info: SceneInfo,
}

impl MainMenuScene {
    /// The clip clock, in milliseconds into the `Stand` sequence.
    #[must_use]
    pub fn time_ms(&self) -> f32 {
        self.time_ms
    }
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
    bytes: &[u8],
    textures: &[pcx_gfx::LoadedTexture],
) -> Result<SceneInfo, SceneError> {
    let m = pcx_mdl::Mdlx::parse(bytes).map_err(|e| SceneError::Decode(e.to_string()))?;
    let model =
        pcx_gfx::load_model(bytes, SCENE_ART).map_err(|e| SceneError::Decode(e.to_string()))?;
    let bones = m
        .bones(bytes)
        .map_err(|e| SceneError::Decode(e.to_string()))?;
    let pivots = m.pivots(bytes).ok();

    // The entry clip: the *first* looping `Stand`, which is the 0..16667 one.
    let sequences = m
        .sequences(bytes)
        .map_err(|e| SceneError::Decode(e.to_string()))?;
    let clip = sequences
        .iter()
        .find(|s| s.name == "Stand" && !s.non_looping)
        .or_else(|| sequences.iter().find(|s| s.name == "Stand"))
        .or_else(|| sequences.first());

    let mut info = SceneInfo {
        bones: bones.len(),
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
    let paths = pcx_gfx::model_texture_paths(bytes, SCENE_ART).unwrap_or_default();
    let mdl_materials = m.materials(bytes).unwrap_or_default();
    let mut handles = Vec::with_capacity(model.meshes.len());
    for mesh in &model.meshes {
        let id = mesh.material_id as usize;
        let path = paths.get(id).cloned().unwrap_or_default();
        let flags = layer_flags(&mdl_materials, id);
        if let Some(gap) = flags.blend_gap {
            info.blend_gaps.push(gap);
        }
        let texture = pick(textures, &path);
        let handle = texture.map(|t| images.add(pcx_gfx::viewer::rgba_image(t)));
        if texture.is_none() && !path.is_empty() {
            info.missing_textures.push(path.clone());
        }
        let material = materials.add(StandardMaterial {
            base_color: flags.base_color(texture.is_some()),
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
        commands.spawn((
            Mesh3d(handles.last().unwrap().clone()),
            MeshMaterial3d(material),
        ));
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
        interval_start: clip.map(|s| s.interval_start).unwrap_or(0),
        duration_ms: clip.map(|s| s.duration()).unwrap_or(0),
        time_ms: 0.0,
        elapsed: std::time::Duration::ZERO,
        global_durations: global_sequence_durations(bytes),
        info: info.clone(),
    });
    Ok(info)
}

/// Marker for the camera this module owns, so a caller can find and remove it.
#[derive(Component)]
pub struct MainMenuSceneCamera;

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
    fn global_clock_keeps_fractional_milliseconds_across_frames() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_nanos(16_666_667));
        world.insert_resource(time);
        world.insert_resource(Assets::<Mesh>::default());
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

    /// The offset the crate reads is not the alpha, and this pins the
    /// difference so the workaround is not quietly dropped as redundant.
    #[test]
    fn the_crates_layer_alpha_is_the_coord_word_not_the_alpha() {
        let Some(bytes) = scene_bytes() else { return };
        let m = pcx_mdl::Mdlx::parse(&bytes).expect("parses");
        let materials = m.materials(&bytes).expect("materials");
        let mut differences = 0;
        for mat in &materials {
            let layer = mat.tex_layers().into_iter().next().expect("a layer");
            let crate_field = layer.static_alpha;
            let real = layer_static_alpha(mat).expect("a static alpha");
            // `coordId` is 0 in every layer here, so the crate reads 0 and the
            // real value is 0.3, 0.55 or 1.0.
            assert_eq!(crate_field, 0.0, "the crate reads the coord word");
            assert!(real > 0.0, "the real alpha is non-zero");
            differences += 1;
        }
        assert_eq!(
            differences, 31,
            "every layer differs, so the workaround matters"
        );
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

/// The change the UI camera needs so the background is visible behind it.
///
/// Bevy draws cameras in `order`; this module's camera is `-1`, so the launcher's
/// `Camera2d` draws on top. For the background to show through, that camera must
/// **not** clear the colour buffer:
///
/// ```ignore
/// Camera { order: 0, clear_color: ClearColorConfig::None, ..default() }
/// ```
///
/// [`layer_ui_camera`] applies exactly that to a camera a caller hands it, so
/// the change is one call rather than something to remember.
pub fn layer_ui_camera(camera: &mut Camera) {
    camera.order = 0;
    camera.clear_color = ClearColorConfig::None;
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

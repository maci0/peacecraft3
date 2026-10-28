//! The console's time-of-day clock model, drawn at the top centre of the HUD.
//!
//! The model comes from the race skin's `TimeOfDayIndicator` key. No FDF frame
//! places it: its geometry is authored directly in the 0.8 x 0.6 UI frame (the
//! header extent centres on x 0.401, y 0.575), so an orthographic camera over
//! that extent reproduces its placement. The first sequence spans one day and is
//! sampled at the host's `day_fraction`, the convention the day/night model uses.
//! Placement, the sequence choice and unlit-by-authored-flags drawing are
//! recreation choices: see `docs/re-notes/campaign-hud-top-bar.md`.
use crate::{campaign_actors, mainmenu_scene as model_scene, menu_material::MenuMaterial};
use bevy::{
    camera::{RenderTarget, ScalingMode, visibility::RenderLayers},
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    ecs::system::SystemParam,
    prelude::*,
    render::render_resource::TextureFormat,
};
use pcx_play::map_startup_host::MapStartupHost;
use peacecraft3::{
    browser_configuration::SelectedMapSources,
    campaign_camera::{CampaignCamera, CampaignCameraSet},
    campaign_presentation::{CampaignHud, CampaignInterface, CampaignPresentationSet},
    map_binding::AssetReceipt,
    map_requests::MapRequests,
    map_script_session::MapScriptSession,
};
use std::sync::{Arc, Weak};

// Application composition and allocation limits, unrelated to authored model values.
const LAYER: usize = 10;
const SKIN_KEY: &str = "TimeOfDayIndicator";
const MODEL_BYTES: u64 = 16 << 20;
const IMAGE_BYTES: usize = 16 << 20;
const TEXTURE_BYTES: u64 = 4 << 20;
const VERTICES: usize = 100_000;
const TARGET_EDGE: f32 = 2048.;
const TARGET_PIXELS: u32 = 1 << 20;
const HUD_Z: i32 = 10;

#[derive(Resource, Default)]
pub(crate) struct CampaignTimeIndicator {
    witness: Option<Weak<()>>,
    attempted: bool,
    scene: Option<Scene>,
    shown: bool,
    pub last_error: Option<String>,
}
#[derive(Component)]
struct IndicatorImage;
/// The model's header extent in UI frame units: x/y place it, z orders its layers.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Extent {
    left: f32,
    bottom: f32,
    width: f32,
    height: f32,
    near: f32,
    far: f32,
}
struct Plan {
    source: String,
    bytes: Vec<u8>,
    runtime: pcx_gfx::mdx_runtime::RuntimePlan,
    model: pcx_gfx::LoadedModel,
    textures: Vec<pcx_gfx::LoadedTexture>,
    layers: Vec<(usize, usize)>,
    interval: [u32; 2],
    opacity: pcx_gfx::mdx_opacity::OpacityPlan,
    selection: pcx_gfx::mdx_texture_selection::TextureSelectionPlan,
    uv: pcx_gfx::mdx_uv_translation::UvTranslationPlan,
    extent: Extent,
    receipts: Vec<AssetReceipt>,
}
struct Scene {
    root: Entity,
    image_node: Entity,
    camera: Entity,
    target: Handle<Image>,
    size: UVec2,
    elapsed: std::time::Duration,
    geosets: model_scene::Geosets,
    plan: Plan,
}
#[derive(SystemParam)]
struct Inputs<'w> {
    sources: Res<'w, SelectedMapSources>,
    requests: Res<'w, MapRequests>,
    camera: Res<'w, CampaignCamera>,
}
#[derive(SystemParam)]
struct DrawAssets<'w> {
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<MenuMaterial>>,
    images: ResMut<'w, Assets<Image>>,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<CampaignTimeIndicator>()
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<MenuMaterial>>()
        .init_resource::<Assets<Image>>()
        .add_systems(
            Update,
            project
                .after(CampaignCameraSet)
                .before(CampaignPresentationSet),
        )
        .add_systems(PostUpdate, resize.after(bevy::ui::UiSystems::PostLayout));
}

/// Frame inside the first sequence for a day fraction in `0..1`, the same
/// mapping the day/night model's sun uses (`pcx_play::day_night`).
fn frame_at(interval: [u32; 2], fraction: f32) -> Result<u32, String> {
    if !fraction.is_finite() || !(0.0..=1.0).contains(&fraction) {
        return Err("time-of-day fraction outside 0..1".into());
    }
    let [start, end] = interval;
    let span = end
        .checked_sub(start)
        .filter(|span| *span > 0)
        .ok_or("time indicator: empty day sequence")?;
    Ok((start + (f64::from(span) * f64::from(fraction)) as u32).min(end))
}

fn extent(min: [f32; 3], max: [f32; 3]) -> Result<Extent, String> {
    let valid = min.iter().chain(&max).all(|value| value.is_finite())
        && (0..3).all(|axis| max[axis] > min[axis])
        && min[0] >= 0.
        && min[1] >= 0.
        && max[0] <= crate::mainmenu::FRAME_W + f32::EPSILON;
    if !valid {
        return Err("time indicator: model extent is not a placeable UI rectangle".into());
    }
    Ok(Extent {
        left: min[0],
        bottom: min[1],
        width: max[0] - min[0],
        height: max[1] - min[1],
        near: min[2],
        far: max[2],
    })
}
fn frame_node(extent: Extent) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: percent(extent.left / crate::mainmenu::FRAME_W * 100.),
        bottom: percent(extent.bottom / crate::mainmenu::FRAME_H * 100.),
        width: percent(extent.width / crate::mainmenu::FRAME_W * 100.),
        height: percent(extent.height / crate::mainmenu::FRAME_H * 100.),
        ..default()
    }
}

fn project(
    mut commands: Commands,
    mut state: ResMut<CampaignTimeIndicator>,
    time: Res<Time>,
    inputs: Inputs,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
    mut assets: DrawAssets,
    mut visibility: Query<&mut Visibility>,
) {
    let witness = session.session_witness();
    let same = match (&state.witness, &witness) {
        (Some(a), Some(b)) => Weak::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    };
    if !same {
        if let Some(scene) = state.scene.take() {
            retire(&mut commands, scene, &mut assets);
        }
        *state = CampaignTimeIndicator {
            witness,
            ..default()
        };
    }
    state.shown = false;
    if !inputs.camera.active() {
        return;
    }
    // The host is borrowed mutably; take owned values before the context check.
    let Some(host) = session.host() else {
        return;
    };
    let objects = host.objects();
    let race = (state.scene.is_none() && !state.attempted).then(|| {
        objects
            .local_race_preference_constant()
            .map_err(|e| e.to_string())
    });
    // Before the clock is admitted there is no time to show: draw nothing.
    let Some(fraction) = objects.day_fraction() else {
        return;
    };
    if state.scene.is_none() && !state.attempted {
        let Some(bindings) = inputs.requests.bindings.as_ref() else {
            return;
        };
        let Some(context) = session.context() else {
            return;
        };
        if !inputs
            .requests
            .admission
            .shared_context()
            .as_ref()
            .is_some_and(|published| Arc::ptr_eq(published, context))
            || !inputs
                .requests
                .published_receipts
                .as_ref()
                .is_some_and(|published| Arc::ptr_eq(published, &bindings.receipts))
        {
            return;
        }
        let Some(reader) = inputs.sources.for_receipts(&bindings.receipts) else {
            return;
        };
        state.attempted = true;
        let race = race.unwrap_or(Ok(None));
        match prepare(&reader, race).and_then(|plan| spawn(&mut commands, &mut assets, plan)) {
            Ok(scene) => state.scene = Some(scene),
            Err(error) => state.last_error = Some(error),
        }
    }
    let Some(mut scene) = state.scene.take() else {
        return;
    };
    scene.elapsed += time.delta();
    match advance(&mut scene, fraction, &mut assets, &mut visibility) {
        Ok(()) => {
            state.shown = true;
            state.scene = Some(scene);
        }
        Err(error) => {
            state.last_error = Some(error);
            retire(&mut commands, scene, &mut assets);
        }
    }
}

fn prepare(
    reader: &pcx_assets::Resolver,
    race: Result<Option<String>, String>,
) -> Result<Plan, String> {
    use crate::campaign_hud::upper_buttons::{read, skin_file, text};
    let mut receipts = Vec::new();
    let skin = crate::menu_skin::SkinTable::parse(&text(reader, super::SKIN, &mut receipts)?)
        .map_err(|e| e.to_string())?;
    let sections = super::race_sections(&skin, race?)?;
    let source = pcx_mdl::binary_model_path(&skin_file(&skin, &sections, SKIN_KEY)?);
    let bytes = read(reader, &source, MODEL_BYTES, &mut receipts)?;
    let mdl = pcx_mdl::Mdlx::parse(&bytes).map_err(|e| format!("{source}: {e}"))?;
    let day = mdl
        .sequences(&bytes)
        .map_err(|e| format!("{source}: {e}"))?
        .into_iter()
        .next()
        .ok_or_else(|| format!("{source}: no day sequence"))?;
    let interval = [day.interval_start, day.interval_end];
    frame_at(interval, 0.)?;
    let mut model = pcx_gfx::load_model(&bytes, &source).map_err(|e| e.to_string())?;
    let mut runtime =
        pcx_gfx::mdx_runtime::RuntimePlan::parse(&bytes).map_err(|e| e.to_string())?;
    let extent = extent(model.minimum_extent, model.maximum_extent)?;
    let (_, layers) = campaign_actors::expand_layers(&mut model, &mut runtime, VERTICES)?;
    let required: std::collections::BTreeSet<_> = runtime
        .geosets
        .iter()
        .flat_map(|g| {
            let l = &runtime.materials[g.material_id].layers[0];
            std::iter::once(l.properties.texture_id).chain(
                l.texture_tracks
                    .iter()
                    .flat_map(|t| t.keys.iter().map(|(_, id)| *id)),
            )
        })
        .collect();
    let mut textures = Vec::new();
    let mut remaining = IMAGE_BYTES;
    for (id, texture) in runtime.textures.iter().enumerate() {
        if !required.contains(&(id as u32)) {
            continue;
        }
        if texture.path.is_empty() {
            return Err(format!("{source}: replaceable texture {id} has no source"));
        }
        let bytes = read(reader, &texture.path, TEXTURE_BYTES, &mut receipts)?;
        let decoded = pcx_gfx::load_image_bounded(&bytes, &texture.path, remaining)
            .map_err(|e| e.to_string())?;
        remaining = remaining
            .checked_sub(
                decoded
                    .rgba_bytes()
                    .ok_or("time indicator: invalid image")?,
            )
            .ok_or("time indicator: image budget")?;
        textures.push(decoded);
    }
    let opacity = pcx_gfx::mdx_opacity::OpacityPlan::checked(&runtime, interval)
        .map_err(|e| e.to_string())?;
    let selection =
        model_scene::texture_selection(&runtime, interval, &source).map_err(|e| e.to_string())?;
    let uv =
        pcx_gfx::mdx_uv_translation::UvTranslationPlan::from_runtime(&runtime, interval, &source)
            .map_err(|e| e.to_string())?;
    Ok(Plan {
        source,
        bytes,
        runtime,
        model,
        textures,
        layers,
        interval,
        opacity,
        selection,
        uv,
        extent,
        receipts,
    })
}

fn spawn(
    commands: &mut Commands,
    assets: &mut DrawAssets,
    mut plan: Plan,
) -> Result<Scene, String> {
    let mdl = pcx_mdl::Mdlx::parse(&plan.bytes).map_err(|e| e.to_string())?;
    let source = model_scene::GeosetSource {
        m: &mdl,
        bytes: &plan.bytes,
        source: &plan.source,
        model: &plan.model,
        textures: &plan.textures,
        runtime: &plan.runtime,
        selection: &plan.selection,
        uv_translation: &plan.uv,
        native_cutout_reference: None,
        material_layers: Some(&plan.layers),
        culls_back_faces: false,
    };
    model_scene::preflight_geosets(&source, &plan.opacity, plan.interval[0])
        .map_err(|e| e.to_string())?;
    let root = commands
        .spawn((Transform::default(), Visibility::Inherited))
        .id();
    let geosets = model_scene::spawn_geosets(
        commands,
        model_scene::GeosetAssets {
            meshes: &mut assets.meshes,
            materials: &mut assets.materials,
            images: &mut assets.images,
        },
        source,
        &plan.opacity,
        plan.interval[0],
        &mut model_scene::SceneInfo::default(),
        (
            ChildOf(root),
            bevy::render::batching::NoAutomaticBatching,
            RenderLayers::layer(LAYER),
        ),
    )
    .map_err(|e| e.to_string())?;
    plan.textures.clear();
    let target = assets.images.add(Image::new_target_texture(
        1,
        1,
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    let extent = plan.extent;
    // Looking down -Z from z 0: depth runs from -far to -near of the model's own extent.
    let camera = commands
        .spawn((
            Camera3d::default(),
            crate::menu_composition::MenuColourCamera,
            Camera {
                order: -1,
                is_active: false,
                clear_color: ClearColorConfig::Custom(Color::NONE),
                ..default()
            },
            RenderTarget::Image(target.clone().into()),
            Projection::Orthographic(OrthographicProjection {
                scaling_mode: ScalingMode::Fixed {
                    width: extent.width,
                    height: extent.height,
                },
                near: -extent.far,
                far: -extent.near,
                ..OrthographicProjection::default_3d()
            }),
            Transform::from_xyz(
                extent.left + extent.width / 2.,
                extent.bottom + extent.height / 2.,
                0.,
            ),
            Tonemapping::None,
            DebandDither::Disabled,
            RenderLayers::layer(LAYER),
            ChildOf(root),
        ))
        .id();
    let image_node = commands
        .spawn((
            frame_node(extent),
            ImageNode::new(target.clone()).with_mode(bevy::ui::widget::NodeImageMode::Stretch),
            IndicatorImage,
            GlobalZIndex(HUD_Z),
            CampaignHud,
            CampaignInterface,
            Pickable::IGNORE,
        ))
        .id();
    Ok(Scene {
        root,
        image_node,
        camera,
        target,
        size: UVec2::ONE,
        elapsed: std::time::Duration::ZERO,
        geosets,
        plan,
    })
}

fn advance(
    scene: &mut Scene,
    fraction: f32,
    assets: &mut DrawAssets,
    visibility: &mut Query<&mut Visibility>,
) -> Result<(), String> {
    let elapsed = model_scene::elapsed_millis(&scene.plan.source, scene.elapsed)
        .map_err(|e| e.to_string())?;
    let frame = frame_at(scene.plan.interval, fraction)?;
    model_scene::skin_geosets(
        model_scene::Rig {
            blend: None,
            interval: scene.plan.interval,
            model: &scene.plan.model,
            bones: scene.plan.runtime.joints.joints(),
            pivots: scene.plan.runtime.joints.pivots(),
            global_durations: &scene.plan.runtime.global_sequences,
            camera_pose: None,
            look_at: None,
        },
        &scene.geosets.handles,
        frame,
        u32::try_from(elapsed).map_err(|_| "time indicator: global clock overflow")?,
        &mut assets.meshes,
    )
    .map_err(|e| e.to_string())?;
    model_scene::apply_opacity(
        &scene.plan.opacity,
        &scene.geosets.bound,
        frame,
        elapsed,
        &mut assets.materials,
        visibility,
    );
    model_scene::apply_geoset_colour(
        &scene.plan.opacity,
        &scene.geosets.bound,
        frame,
        elapsed,
        &mut assets.materials,
    )
    .map_err(|e| e.to_string())?;
    model_scene::apply_texture_selection(
        &scene.plan.selection,
        &mut scene.geosets.textures,
        frame,
        elapsed,
        &mut assets.materials,
    )
    .map_err(|e| e.to_string())?;
    model_scene::apply_uv_translation(
        &scene.plan.uv,
        &scene.geosets.textures,
        frame,
        elapsed,
        &mut assets.materials,
    )
    .map_err(|e| e.to_string())
}

fn retire(commands: &mut Commands, scene: Scene, assets: &mut DrawAssets) {
    commands.entity(scene.image_node).try_despawn();
    commands.entity(scene.root).try_despawn();
    for handle in scene.geosets.handles {
        assets.meshes.remove(handle.id());
    }
    for (_, _, handle) in scene.geosets.bound {
        assets.materials.remove(handle.id());
    }
    for handle in scene.geosets.textures.image_handles() {
        assets.images.remove(handle.id());
    }
    assets.images.remove(scene.target.id());
}

/// Sizes the render target to the node's pixels and runs the camera only while
/// the node is laid out (a hidden interface lays out to zero).
fn resize(
    mut state: ResMut<CampaignTimeIndicator>,
    nodes: Query<&ComputedNode, With<IndicatorImage>>,
    mut targets: ResMut<Assets<Image>>,
    mut cameras: Query<&mut Camera>,
) {
    let shown = state.shown;
    let Some(scene) = state.scene.as_mut() else {
        return;
    };
    let (Ok(node), Ok(mut camera)) = (nodes.get(scene.image_node), cameras.get_mut(scene.camera))
    else {
        return;
    };
    camera.is_active = false;
    if !shown {
        return;
    }
    let size = match target_size(node.size()) {
        Ok(Some(size)) => size,
        Ok(None) => return,
        Err(error) => {
            state.last_error = Some(error.into());
            return;
        }
    };
    if size != scene.size {
        let Some(mut image) = targets.get_mut(&scene.target) else {
            state.last_error = Some("time indicator: render target removed".into());
            return;
        };
        *image = Image::new_target_texture(size.x, size.y, TextureFormat::Rgba8UnormSrgb, None);
        scene.size = size;
    }
    camera.is_active = true;
}

fn target_size(size: Vec2) -> Result<Option<UVec2>, &'static str> {
    if !size.is_finite() {
        return Err("time indicator: nonfinite render target");
    }
    if size.min_element() <= 0. {
        return Ok(None);
    }
    if size.max_element() > TARGET_EDGE {
        return Err("time indicator: render target dimension budget");
    }
    let size = size.ceil().as_uvec2();
    if size.x.checked_mul(size.y).is_none_or(|n| n > TARGET_PIXELS) {
        return Err("time indicator: render target pixel budget");
    }
    Ok(Some(size))
}

impl CampaignTimeIndicator {
    pub(crate) fn receipt_count(&self) -> usize {
        self.scene.as_ref().map_or(0, |s| s.plan.receipts.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_fraction_maps_linearly_into_the_first_sequence() {
        let interval = [0, 60_000];
        assert_eq!(frame_at(interval, 0.).unwrap(), 0);
        assert_eq!(frame_at(interval, 0.25).unwrap(), 15_000);
        assert_eq!(frame_at(interval, 0.75).unwrap(), 45_000);
        assert_eq!(frame_at(interval, 1.).unwrap(), 60_000);
        assert_eq!(frame_at([67_500, 127_500], 0.5).unwrap(), 97_500);
    }

    #[test]
    fn frame_refuses_bad_fractions_and_empty_sequences() {
        assert!(frame_at([0, 60_000], f32::NAN).is_err());
        assert!(frame_at([0, 60_000], -0.1).is_err());
        assert!(frame_at([0, 60_000], 1.1).is_err());
        assert!(frame_at([5, 5], 0.5).is_err());
        assert!(frame_at([9, 5], 0.5).is_err());
    }

    #[test]
    fn extent_places_the_model_in_the_ui_frame() {
        let e = extent([0.337, 0.536, 0.3736], [0.465, 0.615, 0.677]).unwrap();
        assert!((e.left - 0.337).abs() < 1e-6 && (e.bottom - 0.536).abs() < 1e-6);
        assert!((e.width - 0.128).abs() < 1e-6 && (e.height - 0.079).abs() < 1e-6);
        assert_eq!((e.near, e.far), (0.3736, 0.677));
        let node = frame_node(e);
        assert_eq!(node.left, percent(0.337 / crate::mainmenu::FRAME_W * 100.));
        assert_eq!(
            node.bottom,
            percent(0.536 / crate::mainmenu::FRAME_H * 100.)
        );
    }

    #[test]
    fn extent_refuses_degenerate_or_offscreen_models() {
        assert!(extent([0.4, 0.5, 0.], [0.4, 0.6, 1.]).is_err());
        assert!(extent([0.4, 0.5, 0.], [0.5, 0.6, f32::NAN]).is_err());
        assert!(extent([-0.1, 0.5, 0.], [0.5, 0.6, 1.]).is_err());
        assert!(extent([0.4, 0.5, 0.], [0.9, 0.6, 1.]).is_err());
    }

    #[test]
    fn target_size_is_bounded_and_skips_hidden_nodes() {
        assert_eq!(target_size(Vec2::ZERO), Ok(None));
        assert_eq!(
            target_size(Vec2::new(160.4, 98.)),
            Ok(Some(UVec2::new(161, 98)))
        );
        assert!(target_size(Vec2::new(4096., 10.)).is_err());
        assert!(target_size(Vec2::new(f32::NAN, 10.)).is_err());
        assert!(target_size(Vec2::new(2048., 2048.)).is_err());
    }
}

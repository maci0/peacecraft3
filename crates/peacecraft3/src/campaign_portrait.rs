//! A receipt-bound portrait scene using the shared MDX renderer and authored CAMS.
use crate::{
    campaign_actors::{self, TeamPalette},
    mainmenu_scene as model_scene,
    menu_material::MenuMaterial,
};
use bevy::{
    camera::{RenderTarget, visibility::RenderLayers},
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    ecs::system::SystemParam,
    prelude::*,
    render::render_resource::TextureFormat,
};
use pcx_play::{
    map_startup_host::MapStartupHost, playercolor_host::ExecutedUnit, unit_panel_layout::Portrait,
};
use peacecraft3::{
    browser_configuration::SelectedMapSources,
    campaign_camera::CampaignCamera,
    campaign_presentation::{CampaignInterface, CampaignPresentationSet},
    map_binding::{AssetReceipt, receipt},
    map_requests::MapRequests,
    map_script_session::MapScriptSession,
};
use std::{
    collections::BTreeSet,
    sync::{Arc, Weak},
};

// Application composition and allocation limits, unrelated to authored model values.
const LAYER: usize = 9;
const IMAGE_BYTES: usize = 64 << 20;
const VERTICES: usize = 100_000;
const TARGET_PIXELS: u32 = 1 << 20;
const TARGET_EDGE: f32 = 2048.;
#[derive(Resource, Default)]
pub(crate) struct CampaignPortrait {
    witness: Option<Weak<()>>,
    parameters: Option<pcx_play::menu_lighting::Parameters>,
    palette: Option<TeamPalette>,
    scene: Option<Scene>,
    failed: Option<Key>,
    shown: bool,
    pub last_error: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Key {
    unit: pcx_script::HandleId,
    model: String,
    color: i32,
}
#[derive(Component)]
struct PortraitImage;
struct Plan {
    animation: pcx_play::unit_model::Animation,
    source: Arc<pcx_play::unit_model::Model>,
    runtime: pcx_gfx::mdx_runtime::RuntimePlan,
    model: pcx_gfx::LoadedModel,
    textures: Vec<pcx_gfx::LoadedTexture>,
    layers: Vec<(usize, usize)>,
    interval: [u32; 2],
    opacity: pcx_gfx::mdx_opacity::OpacityPlan,
    selection: pcx_gfx::mdx_texture_selection::TextureSelectionPlan,
    uv: pcx_gfx::mdx_uv_translation::UvTranslationPlan,
    camera: pcx_mdl::Camera,
    camera_plan: pcx_gfx::mdx_camera::CameraPlan,
    lighting: crate::menu_lighting::Lighting,
    tint: [f32; 3],
    receipts: Vec<AssetReceipt>,
}
struct Scene {
    key: Key,
    parent: Entity,
    root: Entity,
    image_node: Entity,
    camera_entity: Entity,
    target: Handle<Image>,
    size: UVec2,
    elapsed: std::time::Duration,
    geosets: model_scene::Geosets,
    plan: Plan,
}
#[derive(SystemParam)]
struct DrawAssets<'w> {
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<MenuMaterial>>,
    images: ResMut<'w, Assets<Image>>,
}
#[derive(SystemParam)]
struct Inputs<'w> {
    sources: Res<'w, SelectedMapSources>,
    requests: Res<'w, MapRequests>,
    camera: Res<'w, CampaignCamera>,
    panel: Res<'w, crate::campaign_unit_panel::UnitPanel>,
    launcher: Option<Res<'w, crate::launch_gui::Launcher>>,
}
#[derive(SystemParam)]
struct Queries<'w, 's> {
    visibility: Query<'w, 's, &'static mut Visibility>,
    transforms: Query<'w, 's, &'static mut Transform>,
    cameras: Query<'w, 's, &'static mut Camera>,
}
pub(crate) fn install(app: &mut App) {
    app.init_resource::<CampaignPortrait>()
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<MenuMaterial>>()
        .init_resource::<Assets<Image>>()
        .add_systems(
            Update,
            project
                .after(crate::campaign_unit_panel::UnitPanelSet)
                .before(CampaignPresentationSet),
        )
        .add_systems(PostUpdate, resize.after(bevy::ui::UiSystems::PostLayout));
}
fn project(
    mut commands: Commands,
    mut state: ResMut<CampaignPortrait>,
    time: Res<Time>,
    inputs: Inputs,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
    mut assets: DrawAssets,
    mut queries: Queries,
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
        *state = CampaignPortrait {
            witness,
            ..default()
        };
    }
    state.shown = inputs.camera.active();
    if !state.shown {
        if let Some(scene) = &state.scene {
            commands.entity(scene.image_node).insert(Visibility::Hidden);
            if let Ok(mut camera) = queries.cameras.get_mut(scene.camera_entity) {
                camera.is_active = false;
            }
        }
        return;
    }
    let Some((rules, viewport, parent)) = inputs.panel.portrait_binding() else {
        return;
    };
    let Some(bindings) = inputs.requests.bindings.as_ref().filter(|b| {
        inputs
            .requests
            .published_receipts
            .as_ref()
            .is_some_and(|r| Arc::ptr_eq(r, &b.receipts))
            && inputs
                .requests
                .admission
                .shared_context()
                .as_ref()
                .zip(session.context())
                .is_some_and(|(a, b)| Arc::ptr_eq(a, b))
    }) else {
        return;
    };
    let Some(reader) = inputs.sources.for_receipts(&bindings.receipts) else {
        return;
    };
    let Some(host) = session.host() else {
        return;
    };
    let objects = host.objects();
    if objects
        .presentation()
        .is_some_and(|p| p.interface_visible == Some(false))
    {
        state.shown = false;
        if let Some(scene) = &state.scene {
            commands.entity(scene.image_node).insert(Visibility::Hidden);
            if let Ok(mut camera) = queries.cameras.get_mut(scene.camera_entity) {
                camera.is_active = false;
            }
        }
        return;
    }
    let desired = (|| {
        let [id] = objects.selection() else {
            return Ok(None);
        };
        let unit = objects.executed_unit(*id).map_err(|e| e.to_string())?;
        if !unit.shown || unit.combat.as_ref().is_some_and(|c| c.dead) {
            return Ok(None);
        }
        let animation = unit
            .animation
            .as_ref()
            .ok_or("portrait: unit model unavailable")?;
        Ok::<_, String>(Some((
            Key {
                unit: *id,
                model: animation.model.member.clone(),
                color: campaign_actors::team_color(objects, unit)?,
            },
            unit,
        )))
    })();
    let (key, unit) = match desired {
        Ok(Some(desired)) => desired,
        Ok(None) => {
            if let Some(scene) = state.scene.take() {
                retire(&mut commands, scene, &mut assets);
            }
            state.failed = None;
            state.last_error = None;
            return;
        }
        Err(error) => {
            state.last_error = Some(error);
            if let Some(scene) = state.scene.take() {
                retire(&mut commands, scene, &mut assets);
            }
            return;
        }
    };
    if state
        .scene
        .as_ref()
        .is_some_and(|s| s.key != key || s.parent != parent)
    {
        let scene = state.scene.take().unwrap();
        retire(&mut commands, scene, &mut assets);
    }
    if state.scene.is_none() && state.failed.as_ref() != Some(&key) {
        let prepared = (|| {
            if state.parameters.is_none() {
                let install = inputs
                    .launcher
                    .as_deref()
                    .and_then(crate::launch_gui::selected_install);
                let parameters = crate::menu_lighting::parameters(install.as_deref())?;
                if parameters.image_sha256 != rules.image_sha256 {
                    return Err("portrait: source image differs from layout".into());
                }
                state.parameters = Some(parameters);
            }
            if state.palette.is_none() {
                let asset = reader
                    .resolve_bounded(r"UI\war3skins.txt", 1 << 20)
                    .map_err(|e| e.to_string())?;
                state.palette = Some(TeamPalette::read(&asset.bytes)?);
            }
            let plan = prepare(
                &reader,
                unit,
                &rules.portrait,
                key.color,
                state.palette.as_ref().unwrap(),
                state.parameters.as_ref().unwrap(),
            )?;
            spawn(
                &mut commands,
                &mut assets,
                plan,
                key.clone(),
                parent,
                viewport,
                &rules.portrait,
                state.parameters.as_ref().unwrap().cutout_reference,
            )
        })();
        match prepared {
            Ok(scene) => {
                state.scene = Some(scene);
                state.failed = None;
                state.last_error = None;
            }
            Err(error) => {
                state.failed = Some(key.clone());
                state.last_error = Some(error);
            }
        }
    }
    let Some(mut scene) = state.scene.take() else {
        return;
    };
    commands
        .entity(scene.image_node)
        .insert(Visibility::Inherited);
    scene.elapsed += time.delta();
    if let Err(error) = advance(
        &mut scene,
        &mut assets,
        &mut queries.visibility,
        &mut queries.transforms,
    ) {
        state.last_error = Some(error);
        state.failed = Some(key);
        retire(&mut commands, scene, &mut assets);
    } else {
        state.scene = Some(scene);
    }
}
fn prepare(
    reader: &pcx_assets::Resolver,
    unit: &ExecutedUnit,
    layout: &Portrait,
    color: i32,
    palette: &TeamPalette,
    parameters: &pcx_play::menu_lighting::Parameters,
) -> Result<Plan, String> {
    let animation = unit
        .animation
        .as_ref()
        .ok_or("portrait: unit model unavailable")?;
    let preferred = layout.member(&animation.model.member)?;
    let mut receipts = Vec::new();
    let source = match reader.resolve_bounded(&preferred, 16 << 20) {
        Ok(asset) => {
            receipts.push(receipt(&asset));
            Arc::new(pcx_play::unit_model::Model::read(
                animation.model.origin.clone(),
                preferred,
                Arc::from(asset.bytes),
                16 << 20,
            )?)
        }
        Err(pcx_assets::Error::MissingAsset { .. }) => animation.model.clone(),
        Err(error) => return Err(error.to_string()),
    };
    let mdl = pcx_mdl::Mdlx::parse(&source.bytes).map_err(|e| e.to_string())?;
    // shortcut: idle variants follow source order, expand when acknowledgement audio drives portrait speech and rarity selection.
    let mut idle = pcx_play::unit_model::Animation::new(source.clone(), 0.)?;
    idle.select("portrait", 0., None)?;
    let sample = idle
        .sample(0.)?
        .ok_or("portrait: model has no authored animation")?;
    let interval = sample.interval;
    let camera = mdl
        .cameras(&source.bytes)
        .map_err(|e| e.to_string())?
        .into_iter()
        .next()
        .ok_or("portrait: model has no authored camera")?;
    let mut model =
        pcx_gfx::load_model(&source.bytes, &source.member).map_err(|e| e.to_string())?;
    let mut runtime =
        pcx_gfx::mdx_runtime::RuntimePlan::parse(&source.bytes).map_err(|e| e.to_string())?;
    let (_, layers) = campaign_actors::expand_layers(&mut model, &mut runtime, VERTICES)?;
    let camera_plan =
        pcx_gfx::mdx_camera::CameraPlan::checked(&camera, interval, &runtime.global_sequences)
            .map_err(str::to_owned)?;
    pose(camera_plan.sample(interval[0], 0).map_err(str::to_owned)?)?;
    model_scene::MenuCameraProjection::checked(
        camera.field_of_view,
        [1., 1.],
        camera.near,
        camera.far,
    )
    .map_err(str::to_owned)?;
    let required: BTreeSet<_> = runtime
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
    for (id, texture) in runtime.textures.iter_mut().enumerate() {
        if !required.contains(&(id as u32)) {
            continue;
        }
        if texture.path.is_empty() {
            texture.path = palette.path(reader.paths(), texture.slot, color)?;
        }
        let asset = reader
            .resolve_bounded(&texture.path, 16 << 20)
            .map_err(|e| e.to_string())?;
        let decoded = pcx_gfx::load_mipmapped_image_bounded(&asset.bytes, &texture.path, remaining)
            .map_err(|e| e.to_string())?;
        remaining = remaining
            .checked_sub(decoded.rgba_bytes().ok_or("portrait: invalid mip chain")?)
            .ok_or("portrait: image budget")?;
        receipts.push(receipt(&asset));
        textures.push(decoded);
    }
    let opacity = pcx_gfx::mdx_opacity::OpacityPlan::checked(&runtime, interval)
        .map_err(|e| e.to_string())?;
    let selection = model_scene::texture_selection(&runtime, interval, &source.member)
        .map_err(|e| e.to_string())?;
    let uv = pcx_gfx::mdx_uv_translation::UvTranslationPlan::from_runtime(
        &runtime,
        interval,
        &source.member,
    )
    .map_err(|e| e.to_string())?;
    let lighting = crate::menu_lighting::Lighting::checked(
        &source.member,
        &mdl,
        &source.bytes,
        &runtime,
        interval,
        parameters.clone(),
    )?
    .with_directional(layout.light);
    let tint = [
        campaign_actors::scalar(&unit.definition, "red")?,
        campaign_actors::scalar(&unit.definition, "green")?,
        campaign_actors::scalar(&unit.definition, "blue")?,
    ]
    .map(|v| v * parameters.channel_scale);
    if tint.iter().any(|v| !(0.0..=1.0).contains(v)) {
        return Err("portrait: unsupported source tint".into());
    }
    Ok(Plan {
        animation: idle,
        source,
        runtime,
        model,
        textures,
        layers,
        interval,
        opacity,
        selection,
        uv,
        camera,
        camera_plan,
        lighting,
        tint,
        receipts,
    })
}
fn pose(pose: pcx_gfx::mdx_camera::Pose) -> Result<Transform, String> {
    let mut transform =
        model_scene::source_camera_pose(pose.eye, pose.target).map_err(str::to_owned)?;
    transform.rotation *= Quat::from_rotation_z(pose.roll);
    Ok(transform)
}
#[expect(
    clippy::too_many_arguments,
    reason = "one spawn combines the accepted scene and console bindings"
)]
fn spawn(
    commands: &mut Commands,
    assets: &mut DrawAssets,
    mut plan: Plan,
    key: Key,
    parent: Entity,
    viewport: [f32; 2],
    layout: &Portrait,
    cutout: f32,
) -> Result<Scene, String> {
    let mdl = pcx_mdl::Mdlx::parse(&plan.source.bytes).map_err(|e| e.to_string())?;
    let source = model_scene::GeosetSource {
        m: &mdl,
        bytes: &plan.source.bytes,
        source: &plan.source.member,
        model: &plan.model,
        textures: &plan.textures,
        runtime: &plan.runtime,
        selection: &plan.selection,
        uv_translation: &plan.uv,
        native_cutout_reference: Some(cutout),
        material_layers: Some(&plan.layers),
        culls_back_faces: true,
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
    plan.lighting.bind(&geosets.bound, &assets.materials);
    plan.textures.clear();
    let target = assets.images.add(Image::new_target_texture(
        1,
        1,
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    let projection = model_scene::MenuCameraProjection::checked(
        plan.camera.field_of_view,
        [1., 1.],
        plan.camera.near,
        plan.camera.far,
    )
    .map_err(str::to_owned)?;
    let camera_entity = commands
        .spawn((
            Camera3d::default(),
            Camera {
                order: -1,
                is_active: false,
                // The isolated scene starts with an opaque empty framebuffer.
                clear_color: ClearColorConfig::Custom(Color::BLACK),
                ..default()
            },
            RenderTarget::Image(target.clone().into()),
            Projection::custom(projection),
            pose(
                plan.camera_plan
                    .sample(plan.interval[0], 0)
                    .map_err(str::to_owned)?,
            )?,
            Tonemapping::None,
            DebandDither::Disabled,
            RenderLayers::layer(LAYER),
            ChildOf(root),
        ))
        .id();
    let rect = layout.rect(viewport[0] / viewport[1])?;
    let image_node = commands
        .spawn((
            frame(rect, viewport),
            ImageNode::new(target.clone()).with_mode(bevy::ui::widget::NodeImageMode::Stretch),
            PortraitImage,
            CampaignInterface,
            Pickable::IGNORE,
            ZIndex(-1),
            ChildOf(parent),
        ))
        .id();
    Ok(Scene {
        key,
        parent,
        root,
        image_node,
        camera_entity,
        target,
        size: UVec2::ONE,
        elapsed: std::time::Duration::ZERO,
        geosets,
        plan,
    })
}
fn frame(rect: pcx_play::inventory_layout::Rect, viewport: [f32; 2]) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: percent(rect.x / viewport[0] * 100.),
        bottom: percent(rect.y / viewport[1] * 100.),
        width: percent(rect.width / viewport[0] * 100.),
        height: percent(rect.height / viewport[1] * 100.),
        ..default()
    }
}
fn advance(
    scene: &mut Scene,
    assets: &mut DrawAssets,
    visibility: &mut Query<&mut Visibility>,
    transforms: &mut Query<&mut Transform>,
) -> Result<(), String> {
    let elapsed = model_scene::elapsed_millis(&scene.plan.source.member, scene.elapsed)
        .map_err(|e| e.to_string())?;
    let frame = scene
        .plan
        .animation
        .sample(scene.elapsed.as_secs_f64())?
        .ok_or("portrait: selected animation disappeared")?
        .frame;
    let transform = pose(
        scene
            .plan
            .camera_plan
            .sample(frame, elapsed)
            .map_err(str::to_owned)?,
    )?;
    if let Ok(mut current) = transforms.get_mut(scene.camera_entity) {
        *current = transform;
    }
    let billboard = pcx_gfx::mdx_billboard::CameraPosePlan::checked(
        &scene.plan.runtime.joints,
        transform.rotation,
    )
    .map_err(str::to_owned)?;
    let skeleton = model_scene::skin_geosets(
        model_scene::Rig {
            blend: None,
            interval: scene.plan.interval,
            model: &scene.plan.model,
            bones: scene.plan.runtime.joints.joints(),
            pivots: scene.plan.runtime.joints.pivots(),
            global_durations: &scene.plan.runtime.global_sequences,
            camera_pose: Some(&billboard),
            look_at: None,
        },
        &scene.geosets.handles,
        frame,
        u32::try_from(elapsed).map_err(|_| "portrait: global clock overflow")?,
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
    .map_err(|e| e.to_string())?;
    for (_, _, handle) in &scene.geosets.bound {
        if let Some(mut material) = assets.materials.get_mut(handle) {
            let c = material.base.base_color.to_linear();
            material.base.base_color = Color::linear_rgba(
                c.red * scene.plan.tint[0],
                c.green * scene.plan.tint[1],
                c.blue * scene.plan.tint[2],
                c.alpha,
            );
        }
    }
    scene.plan.lighting.apply(
        &scene.plan.model,
        &skeleton,
        (&scene.geosets.handles, &scene.geosets.bound),
        (frame, elapsed),
        (&mut assets.meshes, &mut assets.materials),
    )
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
fn resize(
    mut state: ResMut<CampaignPortrait>,
    panel: Res<crate::campaign_unit_panel::UnitPanel>,
    nodes: Query<&ComputedNode>,
    mut targets: ResMut<Assets<Image>>,
    mut cameras: Query<(&mut Camera, &mut Projection)>,
    mut frames: Query<&mut Node, With<PortraitImage>>,
) {
    let shown = state.shown;
    let Some(scene) = state.scene.as_mut() else {
        return;
    };
    let Some((rules, viewport, _)) = panel.portrait_binding() else {
        return;
    };
    let Ok(node) = nodes.get(scene.image_node) else {
        return;
    };
    let Ok((mut camera, mut projection)) = cameras.get_mut(scene.camera_entity) else {
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
    camera.is_active = true;
    let result = (|| {
        let parent = nodes
            .get(scene.parent)
            .map_err(|_| "portrait: console parent removed")?
            .size();
        let aspect = parent.x / parent.y;
        if let Ok(mut frame) = frames.get_mut(scene.image_node) {
            frame.set_if_neq(self::frame(rules.portrait.rect(aspect)?, viewport));
        }
        if size != scene.size {
            *targets
                .get_mut(&scene.target)
                .ok_or("portrait: render target removed")? =
                Image::new_target_texture(size.x, size.y, TextureFormat::Rgba8UnormSrgb, None);
            *projection = Projection::custom(
                model_scene::MenuCameraProjection::checked(
                    scene.plan.camera.field_of_view,
                    [size.x as f32, size.y as f32],
                    scene.plan.camera.near,
                    scene.plan.camera.far,
                )
                .map_err(str::to_owned)?,
            );
            scene.size = size;
        }
        Ok::<_, String>(())
    })();
    if let Err(error) = result {
        camera.is_active = false;
        state.last_error = Some(error);
    } else {
        state.last_error = None;
    }
}

fn target_size(size: Vec2) -> Result<Option<UVec2>, &'static str> {
    if !size.is_finite() {
        return Err("portrait: nonfinite render target");
    }
    if size.min_element() <= 0. {
        return Ok(None);
    }
    if size.max_element() > TARGET_EDGE {
        return Err("portrait: render target dimension budget");
    }
    let size = size.ceil().as_uvec2();
    if size.x.checked_mul(size.y).is_none_or(|n| n > TARGET_PIXELS) {
        return Err("portrait: render target pixel budget");
    }
    Ok(Some(size))
}

impl CampaignPortrait {
    pub(crate) fn receipt_count(&self) -> usize {
        self.scene.as_ref().map_or(0, |s| s.plan.receipts.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn portrait_frame_preserves_source_coordinates() {
        let node = frame(
            pcx_play::inventory_layout::Rect {
                x: 0.2,
                y: 0.04,
                width: 0.12,
                height: 0.1,
            },
            [1., 2.],
        );
        assert_eq!(node.left, percent(20));
        assert_eq!(node.bottom, percent(2));
        assert_eq!(node.width, percent(12));
        assert_eq!(node.height, percent(5));
        assert_eq!(node.position_type, PositionType::Absolute);
    }
    #[test]
    fn render_targets_are_bounded_before_float_conversion_or_allocation() {
        assert_eq!(target_size(Vec2::ZERO).unwrap(), None);
        assert_eq!(
            target_size(Vec2::new(200.1, 300.2)).unwrap(),
            Some(UVec2::new(201, 301))
        );
        for size in [
            Vec2::splat(f32::NAN),
            Vec2::splat(f32::INFINITY),
            Vec2::splat(2048.),
            Vec2::new(1., 4096.),
        ] {
            assert!(target_size(size).is_err());
        }
    }
    #[test]
    fn authored_camera_pose_keeps_z_up_and_roll_without_fitting_model_bounds() {
        let p = pcx_gfx::mdx_camera::Pose {
            eye: [7., 4., 12.],
            target: [1., 2., 6.],
            roll: 0.3,
        };
        let t = pose(p).unwrap();
        assert_eq!(t.translation, Vec3::from(p.eye));
        let forward = (Vec3::from(p.target) - t.translation).normalize();
        assert!((t.forward().dot(forward) - 1.).abs() < 1e-5);
        assert!(pose(pcx_gfx::mdx_camera::Pose { target: p.eye, ..p }).is_err());
    }
    #[cfg(all(feature = "test-fixtures", not(target_arch = "wasm32")))]
    #[test]
    #[ignore = "requires selected PCX_NATIVE_CHAPTER_* inputs"]
    fn selected_chapter_portrait_loads_source_camera_animates_and_releases_its_assets() {
        use bevy::ecs::system::RunSystemOnce;
        #[derive(Resource)]
        struct Fixture(Option<Scene>);
        let mut app = crate::native_campaign::tests::started_app();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Assets<MenuMaterial>>();
        let reader = {
            let requests = app.world().resource::<MapRequests>();
            app.world()
                .resource::<SelectedMapSources>()
                .for_receipts(&requests.bindings.as_ref().unwrap().receipts)
                .unwrap()
        };
        let install =
            std::path::PathBuf::from(std::env::var("PCX_NATIVE_CHAPTER_INSTALL").unwrap());
        let rules = crate::menu_lighting::with_client_image(Some(&install), |source, bytes| {
            pcx_play::unit_panel_layout::Rules::reviewed_legacy(
                bytes,
                pcx_play::scripts::AssetIdentity {
                    source: source.into(),
                    edition: "selected test".into(),
                    version: "Legacy".into(),
                },
            )
        })
        .unwrap();
        let parameters = crate::menu_lighting::parameters(Some(&install)).unwrap();
        let palette = TeamPalette::read(
            &reader
                .resolve_bounded(r"UI\war3skins.txt", 1 << 20)
                .unwrap()
                .bytes,
        )
        .unwrap();
        let (plan, key) = {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let objects = session.host().unwrap().objects();
            let (_, footman) = objects
                .executed_units()
                .find(|(_, u)| u.definition.rawcode == *b"hfoo")
                .unwrap();
            let footman_plan = prepare(
                &reader,
                footman,
                &rules.portrait,
                campaign_actors::team_color(objects, footman).unwrap(),
                &palette,
                &parameters,
            )
            .unwrap();
            assert!(!footman_plan.runtime.joints.joints().is_empty());
            assert_eq!(
                footman_plan.lighting.count(),
                1,
                "the scene supplies light when the model has none"
            );
            let (id, unit) = objects
                .executed_units()
                .find(|(_, u)| u.definition.rawcode == *b"Hart")
                .unwrap();
            let color = campaign_actors::team_color(objects, unit).unwrap();
            let expected = rules
                .portrait
                .member(&unit.animation.as_ref().unwrap().model.member)
                .unwrap();
            let plan =
                prepare(&reader, unit, &rules.portrait, color, &palette, &parameters).unwrap();
            assert_eq!(plan.source.member, expected);
            assert!(plan.lighting.count() > 0);
            assert!(!plan.receipts.is_empty());
            (
                plan,
                Key {
                    unit: id,
                    model: unit.animation.as_ref().unwrap().model.member.clone(),
                    color,
                },
            )
        };
        let keep = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(Image::default());
        let parent = app.world_mut().spawn(Node::default()).id();
        let mut plan = Some(plan);
        let mut key = Some(key);
        let layout = rules.portrait;
        app.world_mut()
            .run_system_once(move |mut commands: Commands, mut assets: DrawAssets| {
                let scene = spawn(
                    &mut commands,
                    &mut assets,
                    plan.take().unwrap(),
                    key.take().unwrap(),
                    parent,
                    [1., 1.],
                    &layout,
                    parameters.cutout_reference,
                )
                .unwrap();
                commands.insert_resource(Fixture(Some(scene)));
            })
            .unwrap();
        let (root, camera, image) = {
            let scene = app.world().resource::<Fixture>().0.as_ref().unwrap();
            (scene.root, scene.camera_entity, scene.image_node)
        };
        app.world_mut()
            .run_system_once(
                |mut fixture: ResMut<Fixture>,
                 mut assets: DrawAssets,
                 mut visibility: Query<&mut Visibility>,
                 mut transforms: Query<&mut Transform>| {
                    let scene = fixture.0.as_mut().unwrap();
                    scene.elapsed = std::time::Duration::from_millis(300);
                    advance(scene, &mut assets, &mut visibility, &mut transforms).unwrap();
                },
            )
            .unwrap();
        assert!(app.world().get::<Camera>(camera).is_some());
        app.world_mut()
            .run_system_once(
                |mut commands: Commands, mut fixture: ResMut<Fixture>, mut assets: DrawAssets| {
                    retire(&mut commands, fixture.0.take().unwrap(), &mut assets)
                },
            )
            .unwrap();
        for entity in [root, camera, image] {
            assert!(app.world().get_entity(entity).is_err());
        }
        assert!(app.world().resource::<Assets<Mesh>>().is_empty());
        assert!(app.world().resource::<Assets<MenuMaterial>>().is_empty());
        assert!(app.world().resource::<Assets<Image>>().get(&keep).is_some());
        assert_eq!(app.world().resource::<Assets<Image>>().len(), 1);
    }
}

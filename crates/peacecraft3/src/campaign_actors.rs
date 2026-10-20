//! Render the published session's executed unit handles and their selected source clips.
use crate::{mainmenu_scene as model_scene, menu_material::MenuMaterial};
use bevy::{camera::visibility::RenderLayers, ecs::system::SystemParam, prelude::*};
use pcx_assets::Resolver;
use pcx_play::{map_startup_host::MapStartupHost, playercolor_host::ExecutedUnit};
use pcx_script::HandleId;

#[path = "campaign_item_model.rs"]
mod item_model;
#[path = "campaign_sky.rs"]
mod sky;
use pcx_sim::world::{Definition, FieldValue};
use peacecraft3::{
    browser_configuration::SelectedMapSources,
    campaign_camera::{CampaignCameraSet, CampaignSceneCamera},
    map_requests::MapRequests,
    map_script_session::MapScriptSession,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Weak},
};

#[derive(Component)]
struct UnitActor(HandleId);

struct Actor {
    root: Entity,
    model: pcx_gfx::LoadedModel,
    runtime: pcx_gfx::mdx_runtime::RuntimePlan,
    geosets: model_scene::Geosets,
    interval: [u32; 2],
    opacity: pcx_gfx::mdx_opacity::OpacityPlan,
    selection: pcx_gfx::mdx_texture_selection::TextureSelectionPlan,
    uv: pcx_gfx::mdx_uv_translation::UvTranslationPlan,
    tint: [f32; 3],
    scale: f32,
    height: f32,
    color: i32,
    image_bytes: usize,
    vertices: usize,
    /// Geosets whose authored material is lit, captured before sun lighting.
    lit: Vec<bool>,
    world_transform: Transform,
}

#[derive(Resource, Default)]
pub(crate) struct UnitActors {
    session: Option<Weak<()>>,
    reader: Option<Arc<Resolver>>,
    actors: BTreeMap<HandleId, Actor>,
    item_animations: BTreeMap<HandleId, pcx_play::unit_model::Animation>,
    item_models: BTreeMap<String, Arc<pcx_play::unit_model::Model>>,
    item_source_bytes: usize,
    failed: BTreeMap<HandleId, Option<HandleId>>,
    image_bytes: usize,
    vertices: usize,
    parameters: Option<pcx_play::menu_lighting::Parameters>,
    parameters_attempted: bool,
    palette: Option<TeamPalette>,
    sky: sky::Sky,
    pub last_error: Option<String>,
}

impl UnitActors {
    pub(crate) fn item_hit(&self, ray: Ray3d, far: f32, session: &Weak<()>) -> Option<HandleId> {
        if !self
            .session
            .as_ref()
            .is_some_and(|current| Weak::ptr_eq(current, session))
        {
            return None;
        }
        self.item_animations
            .keys()
            .filter_map(|id| {
                let actor = self.actors.get(id)?;
                let inverse = actor.world_transform.to_matrix().inverse();
                let distance = bounds_hit(
                    inverse.transform_point3(ray.origin),
                    inverse.transform_vector3(*ray.direction),
                    Vec3::from_array(actor.model.minimum_extent),
                    Vec3::from_array(actor.model.maximum_extent),
                    far,
                )?;
                Some((*id, distance))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id)
    }
}

fn bounds_hit(
    origin: Vec3,
    direction: Vec3,
    minimum: Vec3,
    maximum: Vec3,
    far: f32,
) -> Option<f32> {
    if !origin.is_finite()
        || !direction.is_finite()
        || !minimum.is_finite()
        || !maximum.is_finite()
        || !far.is_finite()
        || far <= 0.0
        || minimum.cmpge(maximum).any()
    {
        return None;
    }
    let (mut near, mut end) = (0.0_f32, far);
    for axis in 0..3 {
        if direction[axis] == 0.0 {
            if origin[axis] < minimum[axis] || origin[axis] > maximum[axis] {
                return None;
            }
        } else {
            let a = (minimum[axis] - origin[axis]) / direction[axis];
            let b = (maximum[axis] - origin[axis]) / direction[axis];
            near = near.max(a.min(b));
            end = end.min(a.max(b));
            if near > end {
                return None;
            }
        }
    }
    Some(near)
}

#[derive(SystemParam)]
struct DrawAssets<'w> {
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<MenuMaterial>>,
    images: ResMut<'w, Assets<Image>>,
}

#[derive(SystemParam)]
struct ActorQueries<'w, 's> {
    visibility: Query<'w, 's, &'static mut Visibility>,
    transforms: Query<'w, 's, &'static mut Transform, Without<CampaignSceneCamera>>,
    actors: Query<'w, 's, (Entity, &'static UnitActor)>,
    cameras: Query<'w, 's, Entity, With<CampaignSceneCamera>>,
    scene: sky::SceneCameras<'w, 's>,
}

fn transform_for(
    unit: &ExecutedUnit,
    terrain: &pcx_map::Terrain,
    scale: f32,
    height: f32,
) -> Result<Transform, String> {
    let [x, y] = unit.position.ok_or("unit has no accepted world position")?;
    let z = pcx_tera::world::sample_world_z(terrain, x, y)
        .ok_or("unit position is outside admitted terrain")?
        + height;
    let transform = Transform::from_translation(Vec3::new(x, z, -y))
        .with_rotation(
            Quat::from_rotation_y(unit.face.to_radians())
                * Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
        )
        .with_scale(Vec3::splat(scale));
    if !transform.is_finite() {
        return Err("nonfinite unit actor transform".into());
    }
    Ok(transform)
}

fn team_color(
    objects: &pcx_play::playercolor_host::ObjectHost,
    unit: &ExecutedUnit,
) -> Result<i32, String> {
    if let Some(color) = unit.team_color {
        return objects
            .inner()
            .player_color(color)
            .map_err(|e| e.to_string());
    }
    let value = scalar(&unit.definition, "teamColor")?;
    if value == -1. {
        unit.creation_color
            .map(|color| {
                objects
                    .inner()
                    .player_color(color)
                    .map_err(|e| e.to_string())
            })
            .unwrap_or(Ok(unit.executed_owner))
    } else if value >= 0. && value.fract() == 0. && value < (i32::MAX as f32) {
        Ok(value as i32)
    } else {
        Err("unsupported unit team-color source".into())
    }
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<UnitActors>()
        .add_systems(Update, project.after(CampaignCameraSet));
}

pub(crate) fn scalar(unit: &Definition, column: &str) -> Result<f32, String> {
    use pcx_objects::editor::{FieldKind, FieldSource, SlkId};
    let mut matches = unit.fields.iter().filter(|(_, f)| {
        f.metadata
            .as_ref()
            .is_some_and(|m| matches!(&m.source,FieldSource::Column {column:c,..} if c==column))
    });
    let (id, field) = matches
        .next()
        .ok_or_else(|| format!("missing unit field {column}"))?;
    if matches.next().is_some() {
        return Err(format!("ambiguous unit field {column}"));
    }
    let metadata = field.metadata.as_ref().expect("selected metadata");
    if metadata.from != SlkId::UnitMetaData
        || metadata.fourcc != *id
        || metadata.index != -1
        || !(matches!(
            metadata.kind,
            FieldKind::Real | FieldKind::Unreal | FieldKind::Int
        ) || (column == "teamColor"
            && metadata.kind == FieldKind::Token
            && metadata.declared == "teamColor"))
    {
        return Err(format!("unsupported unit field {column}"));
    }
    let number = match &field.value {
        Ok(FieldValue::Cell(s)) => s.parse::<f32>().map_err(|e| e.to_string())?,
        Ok(FieldValue::Override(pcx_objects::Value::Int(v))) if metadata.kind == FieldKind::Int => {
            *v as f32
        }
        Ok(FieldValue::Override(pcx_objects::Value::Real(v)))
            if metadata.kind == FieldKind::Real =>
        {
            *v
        }
        Ok(FieldValue::Override(pcx_objects::Value::Unreal(v)))
            if metadata.kind == FieldKind::Unreal =>
        {
            *v
        }
        Ok(FieldValue::Override(pcx_objects::Value::Str(v)))
            if metadata.kind == FieldKind::Token =>
        {
            std::str::from_utf8(v)
                .map_err(|e| e.to_string())?
                .parse::<i32>()
                .map_err(|e| e.to_string())? as f32
        }
        _ => return Err(format!("unsupported unit field {column} value")),
    };
    if !number.is_finite() {
        return Err(format!("nonfinite unit field {column}"));
    }
    Ok(number)
}

#[derive(Clone)]
struct TeamPalette {
    prefixes: [String; 2],
    count: i32,
}
impl TeamPalette {
    fn read(bytes: &[u8]) -> Result<Self, String> {
        let skin = crate::menu_skin::SkinTable::parse(
            std::str::from_utf8(bytes).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let field = |key| {
            skin.unique_file(key)
                .map(|v| v.file)
                .map_err(|e| e.to_string())
        };
        let count = field("TeamColors")?
            .parse::<i32>()
            .map_err(|e| e.to_string())?;
        if count <= 0 {
            return Err("invalid TeamColors count".into());
        }
        let normalize = |key| -> Result<String, String> {
            pcx_assets::AssetPath::new(&field(key)?)
                .map(|path| path.as_str().to_owned())
                .map_err(|e| e.to_string())
        };
        Ok(Self {
            prefixes: [normalize("TeamColor")?, normalize("TeamGlow")?],
            count,
        })
    }
    fn path<'a>(
        &self,
        paths: impl Iterator<Item = &'a pcx_assets::AssetPath>,
        slot: u32,
        index: i32,
    ) -> Result<String, String> {
        let prefix = match slot {
            1 => &self.prefixes[0],
            2 => &self.prefixes[1],
            _ => return Err(format!("unsupported unit replacement slot {slot}")),
        };
        if !(0..self.count).contains(&index) {
            return Err(format!(
                "replacement colour {index} exceeds source TeamColors"
            ));
        }
        let mut found = paths.filter(|path| {
            path.as_str()
                .strip_prefix(prefix)
                .and_then(|s| s.strip_suffix(".blp"))
                .and_then(|s| s.parse::<i32>().ok())
                == Some(index)
        });
        let path = found
            .next()
            .ok_or_else(|| format!("missing palette {prefix} index {index}"))?;
        if found.next().is_some() {
            return Err(format!("ambiguous palette {prefix} index {index}"));
        }
        Ok(path.as_str().to_owned())
    }
}

// Each source layer gets a draw while keeping the source geometry, GEOA and clocks.
fn expand_layers(
    model: &mut pcx_gfx::LoadedModel,
    runtime: &mut pcx_gfx::mdx_runtime::RuntimePlan,
    maximum_vertices: usize,
) -> Result<(usize, Vec<(usize, usize)>), String> {
    let mut vertices = 0usize;
    for mesh in &model.meshes {
        let layers = runtime
            .materials
            .get(mesh.material_id as usize)
            .ok_or("missing actor material")?
            .layers
            .len();
        if layers == 0 {
            return Err("actor material has no layers".into());
        }
        vertices = mesh
            .positions
            .len()
            .checked_mul(layers)
            .and_then(|n| vertices.checked_add(n))
            .ok_or("actor layer vertex overflow")?;
    }
    if vertices > maximum_vertices {
        return Err("actor layer vertex budget exceeded".into());
    }
    let mut materials = Vec::new();
    let mut source_layers = Vec::new();
    let starts: Vec<_> = runtime
        .materials
        .iter()
        .enumerate()
        .map(|(material_id, material)| {
            let start = materials.len();
            for (layer_id, layer) in material.layers.iter().enumerate() {
                source_layers.push((material_id, layer_id));
                materials.push(pcx_gfx::mdx_runtime::MaterialBinding {
                    priority_plane: material.priority_plane,
                    flags: material.flags,
                    layers: vec![layer.clone()],
                });
            }
            start
        })
        .collect();
    let mut meshes = Vec::new();
    let mut geosets = Vec::new();
    let mut animations = Vec::new();
    let mut uv_sets = Vec::new();
    for mesh in &model.meshes {
        let binding = runtime
            .geosets
            .get(mesh.geoset)
            .ok_or("missing actor geoset")?;
        if binding.material_id != mesh.material_id as usize {
            return Err("actor material mismatch".into());
        }
        for layer in 0..runtime.materials[binding.material_id].layers.len() {
            let id = meshes.len();
            let material = starts[binding.material_id] + layer;
            let mut draw = mesh.clone();
            draw.geoset = id;
            draw.material_id = material as u32;
            let mut bound = binding.clone();
            bound.geoset_id = id;
            bound.material_id = material;
            bound.animation_records.clear();
            for &index in &binding.animation_records {
                let mut animation = runtime
                    .geoset_animations
                    .get(index)
                    .ok_or("missing actor GEOA")?
                    .clone();
                animation.geoset_id = id as u32;
                bound.animation_records.push(animations.len());
                animations.push(animation);
            }
            meshes.push(draw);
            geosets.push(bound);
            uv_sets.push(
                *runtime
                    .uv_set_counts
                    .get(mesh.geoset)
                    .ok_or("missing actor UVAS")?,
            );
        }
    }
    let mut unbound = Vec::new();
    for &index in &runtime.unbound_geoset_animations {
        unbound.push(animations.len());
        animations.push(
            runtime
                .geoset_animations
                .get(index)
                .ok_or("missing unbound actor GEOA")?
                .clone(),
        );
    }
    model.meshes = meshes;
    runtime.materials = materials;
    runtime.geosets = geosets;
    runtime.geoset_animations = animations;
    runtime.unbound_geoset_animations = unbound;
    runtime.uv_set_counts = uv_sets;
    Ok((vertices, source_layers))
}

#[derive(Clone, Copy)]
enum Visual<'a> {
    Unit(&'a ExecutedUnit),
    Item(
        &'a pcx_play::playercolor_host::ExecutedItem,
        &'a pcx_play::unit_model::Animation,
    ),
    /// The sky dome centred on the scene camera eye.
    Sky(&'a pcx_play::unit_model::Animation, Vec3),
}
impl<'a> Visual<'a> {
    fn animation(self) -> Result<&'a pcx_play::unit_model::Animation, String> {
        match self {
            Self::Unit(unit) => unit
                .animation
                .as_ref()
                .ok_or_else(|| "unit model is unavailable".into()),
            Self::Item(_, animation) | Self::Sky(animation, _) => Ok(animation),
        }
    }
    fn face(self) -> f32 {
        match self {
            Self::Unit(unit) => unit.face,
            Self::Item(..) | Self::Sky(..) => 0.0,
        }
    }
    fn layer(self) -> usize {
        match self {
            Self::Sky(..) => sky::RENDER_LAYER,
            Self::Unit(_) | Self::Item(..) => peacecraft3::campaign_camera::RENDER_LAYER,
        }
    }
    fn transform(
        self,
        terrain: &pcx_map::Terrain,
        scale: f32,
        height: f32,
    ) -> Result<Transform, String> {
        match self {
            Self::Sky(_, eye) => {
                Ok(Transform::from_translation(eye).with_scale(Vec3::splat(scale)))
            }
            Self::Unit(unit) => transform_for(unit, terrain, scale, height),
            Self::Item(item, _) => {
                let z = pcx_tera::world::sample_world_z(terrain, item.x, item.y)
                    .ok_or("item outside admitted terrain")?;
                // shortcut: model origin is grounded without a facing override; qualify native item orientation before claiming parity.
                Ok(Transform::from_translation(Vec3::new(item.x, z, -item.y))
                    .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))
                    .with_scale(Vec3::splat(scale)))
            }
        }
    }
    fn properties(self, channel_scale: f32) -> Result<(f32, f32, [f32; 3]), String> {
        match self {
            Self::Unit(unit) => Ok((
                scalar(&unit.definition, "modelScale")?,
                scalar(&unit.definition, "moveHeight")?,
                [
                    scalar(&unit.definition, "red")?,
                    scalar(&unit.definition, "green")?,
                    scalar(&unit.definition, "blue")?,
                ]
                .map(|v| v * channel_scale),
            )),
            Self::Item(item, _) => item_model::properties(&item.definition, channel_scale)
                .map(|(scale, tint)| (scale, 0.0, tint)),
            // The dome is authored in world units and carries no script tint, so no channel scale applies.
            Self::Sky(..) => Ok((sky::MODEL_SCALE, 0.0, [1.0; 3])),
        }
    }
}

struct ActorSource<'a> {
    reader: &'a Resolver,
    palette: &'a TeamPalette,
    visual: Visual<'a>,
    terrain: &'a pcx_map::Terrain,
    id: Option<HandleId>,
    color: i32,
    now: f64,
    channel_scale: f32,
    cutout: f32,
    available_images: usize,
    available_vertices: usize,
}
#[derive(SystemParam)]
struct ActorSources<'w> {
    requests: Res<'w, MapRequests>,
    sources: Res<'w, SelectedMapSources>,
    launcher: Res<'w, crate::launch_gui::Launcher>,
}

impl Actor {
    fn spawn(
        commands: &mut Commands,
        assets: &mut DrawAssets,
        input: ActorSource,
    ) -> Result<(Self, usize, usize), String> {
        let ActorSource {
            reader,
            palette,
            visual,
            terrain,
            id,
            color,
            now,
            channel_scale,
            cutout,
            available_images,
            available_vertices,
        } = input;
        let animation = visual.animation()?;
        let source = &animation.model;
        let bytes = &source.bytes;
        let mdl = pcx_mdl::Mdlx::parse(bytes).map_err(|e| e.to_string())?;
        let mut model = pcx_gfx::load_model(bytes, &source.member).map_err(|e| e.to_string())?;
        let mut runtime =
            pcx_gfx::mdx_runtime::RuntimePlan::parse(bytes).map_err(|e| e.to_string())?;
        if matches!(visual, Visual::Item(..))
            && (0..3).any(|axis| model.minimum_extent[axis] >= model.maximum_extent[axis])
        {
            return Err(format!(
                "{}: unsupported degenerate item picking bounds",
                source.member
            ));
        }
        let (vertices, source_layers) =
            expand_layers(&mut model, &mut runtime, available_vertices)?;
        let mut textures = Vec::new();
        let mut remaining = available_images;
        let required: BTreeSet<_> = runtime
            .geosets
            .iter()
            .filter_map(|geoset| {
                runtime
                    .materials
                    .get(geoset.material_id)
                    .and_then(|m| m.layers.first())
            })
            .flat_map(|layer| {
                std::iter::once(layer.properties.texture_id).chain(
                    layer
                        .texture_tracks
                        .iter()
                        .flat_map(|t| t.keys.iter().map(|&(_, id)| id)),
                )
            })
            .collect();
        for (index, texture) in runtime.textures.iter_mut().enumerate() {
            if !required.contains(&(index as u32)) {
                continue;
            }
            if texture.path.is_empty() {
                if matches!(visual, Visual::Item(..)) {
                    return Err(format!(
                        "item has unsupported replaceable texture slot {}",
                        texture.slot
                    ));
                }
                // Empty slots 1/2 use the indexed team-color/glow families; full paths come from the selected inventory.
                texture.path = match texture.slot {
                    1 | 2 => palette.path(reader.paths(), texture.slot, color)?,
                    slot => return Err(format!("unsupported empty unit texture slot {slot}")),
                };
            }
            let raw = reader
                .resolve_bounded(&texture.path, 16 << 20)
                .map_err(|e| e.to_string())?;
            let decoded =
                pcx_gfx::load_mipmapped_image_bounded(&raw.bytes, &texture.path, remaining)
                    .map_err(|e| e.to_string())?;
            remaining = remaining
                .checked_sub(decoded.rgba_bytes().ok_or("invalid unit image mip chain")?)
                .ok_or("actor image budget exceeded")?;
            textures.push(decoded);
        }
        let sample = animation
            .sample(now)?
            .unwrap_or(pcx_play::unit_model::Sample {
                interval: [0, 0],
                frame: 0,
                elapsed_ms: 0,
            });
        let interval = sample.interval;
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
        let (scale, height, tint) = visual.properties(channel_scale)?;
        if !scale.is_finite() || scale <= 0.0 {
            return Err("nonpositive actor model scale".into());
        }
        let initial_transform = visual.transform(terrain, scale, height)?;
        // Bevy culls by screen winding and corrects only shader front facing, so a mirrored
        // root would cull its visible side. Mirrors are refused; no mirror policy is inferred.
        // Written so that NaN fails the test.
        let determinant = initial_transform.to_matrix().determinant();
        if !(determinant.is_finite() && determinant > 0.0) {
            return Err("unsupported nonfinite or mirrored actor transform".into());
        }
        let source_input = model_scene::GeosetSource {
            m: &mdl,
            bytes,
            source: &source.member,
            model: &model,
            textures: &textures,
            runtime: &runtime,
            selection: &selection,
            uv_translation: &uv,
            native_cutout_reference: Some(cutout),
            material_layers: Some(&source_layers),
            culls_back_faces: true,
        };
        model_scene::preflight_geosets(&source_input, &opacity, sample.frame)
            .map_err(|e| e.to_string())?;
        let mut root = commands.spawn((initial_transform, Visibility::default()));
        if let Some(id) = id {
            root.insert(UnitActor(id));
        }
        let root = root.id();
        let mut info = model_scene::SceneInfo::default();
        let geosets = model_scene::spawn_geosets(
            commands,
            model_scene::GeosetAssets {
                meshes: &mut assets.meshes,
                materials: &mut assets.materials,
                images: &mut assets.images,
            },
            source_input,
            &opacity,
            sample.frame,
            &mut info,
            (
                ChildOf(root),
                bevy::render::batching::NoAutomaticBatching,
                RenderLayers::layer(visual.layer()),
            ),
        )
        .map_err(|e| e.to_string())?;
        let lit = geosets
            .bound
            .iter()
            .map(|(_, _, handle)| assets.materials.get(handle).is_some_and(|m| !m.base.unlit))
            .collect();
        Ok((
            Self {
                root,
                model,
                runtime,
                geosets,
                interval,
                opacity,
                selection,
                uv,
                tint,
                scale,
                height,
                color,
                image_bytes: available_images - remaining,
                vertices,
                lit,
                world_transform: initial_transform,
            },
            available_images - remaining,
            vertices,
        ))
    }
    fn update(
        &mut self,
        visual: Visual,
        now: f64,
        assets: &mut DrawAssets,
        (visibility, transforms): (
            &mut Query<&mut Visibility>,
            &mut Query<&mut Transform, Without<CampaignSceneCamera>>,
        ),
        terrain: &pcx_map::Terrain,
        light: Option<(
            &pcx_play::day_night::Sun,
            &pcx_play::menu_lighting::Parameters,
        )>,
    ) -> Result<(), String> {
        let animation = visual.animation()?;
        let sample = animation
            .sample(now)?
            .unwrap_or(pcx_play::unit_model::Sample {
                interval: [0, 0],
                frame: 0,
                elapsed_ms: 0,
            });
        if self.interval != sample.interval {
            let opacity =
                pcx_gfx::mdx_opacity::OpacityPlan::checked(&self.runtime, sample.interval)
                    .map_err(|e| e.to_string())?;
            let selection = model_scene::texture_selection(
                &self.runtime,
                sample.interval,
                &animation.model.member,
            )
            .map_err(|e| e.to_string())?;
            let uv = pcx_gfx::mdx_uv_translation::UvTranslationPlan::from_runtime(
                &self.runtime,
                sample.interval,
                &animation.model.member,
            )
            .map_err(|e| e.to_string())?;
            self.opacity = opacity;
            self.selection = selection;
            self.uv = uv;
            self.interval = sample.interval;
        }
        self.world_transform = visual.transform(terrain, self.scale, self.height)?;
        if let Ok(mut transform) = transforms.get_mut(self.root) {
            *transform = self.world_transform;
        }
        let skeleton = model_scene::skin_geosets(
            model_scene::Rig {
                interval: self.interval,
                model: &self.model,
                bones: self.runtime.joints.joints(),
                pivots: self.runtime.joints.pivots(),
                global_durations: &self.runtime.global_sequences,
                camera_pose: None,
            },
            &self.geosets.handles,
            sample.frame,
            u32::try_from(sample.elapsed_ms).map_err(|_| "unit global animation clock overflow")?,
            &mut assets.meshes,
        )
        .map_err(|e| e.to_string())?;
        model_scene::apply_opacity(
            &self.opacity,
            &self.geosets.bound,
            sample.frame,
            sample.elapsed_ms,
            &mut assets.materials,
            visibility,
        );
        model_scene::apply_geoset_colour(
            &self.opacity,
            &self.geosets.bound,
            sample.frame,
            sample.elapsed_ms,
            &mut assets.materials,
        )
        .map_err(|e| e.to_string())?;
        model_scene::apply_texture_selection(
            &self.selection,
            &mut self.geosets.textures,
            sample.frame,
            sample.elapsed_ms,
            &mut assets.materials,
        )
        .map_err(|e| e.to_string())?;
        model_scene::apply_uv_translation(
            &self.uv,
            &self.geosets.textures,
            sample.frame,
            sample.elapsed_ms,
            &mut assets.materials,
        )
        .map_err(|e| e.to_string())?;
        for (_, _, handle) in &self.geosets.bound {
            if let Some(material) = assets.materials.get_mut(handle).as_deref_mut() {
                let c = material.base.base_color.to_linear();
                material.base.base_color = Color::linear_rgba(
                    c.red * self.tint[0],
                    c.green * self.tint[1],
                    c.blue * self.tint[2],
                    c.alpha,
                );
            }
        }
        if let Some((sun, parameters)) = light {
            // The model faces +X at zero facing: undo the unit's facing around Z.
            let (sin, cos) = visual.face().to_radians().sin_cos();
            let [x, y, z] = sun.direction;
            let direction = Vec3::new(x * cos + y * sin, -x * sin + y * cos, z);
            crate::menu_lighting::apply_sun(
                &animation.model.member,
                &self.model,
                &skeleton,
                (&self.geosets.handles, &self.geosets.bound, &self.lit),
                (direction, sun, parameters),
                (&mut assets.meshes, &mut assets.materials),
            )?;
        }
        Ok(())
    }
}

fn project(
    mut commands: Commands,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
    sources: ActorSources,
    mut state: ResMut<UnitActors>,
    mut assets: DrawAssets,
    mut queries: ActorQueries,
) {
    let witness = session.session_witness();
    if !matches!((&state.session,&witness),(Some(a),Some(b)) if Weak::ptr_eq(a,b)) {
        for (entity, actor) in &queries.actors {
            trace!(unit=?actor.0,"retire campaign unit actor");
            commands.entity(entity).despawn();
        }
        sky::release(&mut commands, &mut state, &mut queries.scene);
        *state = UnitActors {
            session: witness,
            ..default()
        };
    }
    let Some(context) = session.context().cloned() else {
        return;
    };
    if state.reader.is_none()
        && sources
            .requests
            .admission
            .shared_context()
            .is_some_and(|admitted| Arc::ptr_eq(&admitted, &context))
        && let Some(bindings) = sources.requests.bindings.as_ref()
    {
        state.reader = sources.sources.for_receipts(&bindings.receipts);
    }
    let Some(reader) = state.reader.clone() else {
        return;
    };
    let Some(host) = session.host() else {
        return;
    };
    let objects = host.objects();
    let Some(now) = objects.timer_game_time() else {
        return;
    };
    if !state.parameters_attempted {
        state.parameters_attempted = true;
        let install = crate::launch_gui::selected_install(&sources.launcher);
        match crate::menu_lighting::parameters(install.as_deref()) {
            Ok(v) => state.parameters = Some(v),
            Err(e) => state.last_error = Some(e),
        }
        match reader
            .resolve_bounded("UI/war3skins.txt", 1 << 20)
            .map_err(|e| e.to_string())
            .and_then(|asset| TeamPalette::read(&asset.bytes))
        {
            Ok(palette) => state.palette = Some(palette),
            Err(e) => state.last_error = Some(format!("UI/war3skins.txt: {e}")),
        }
    }
    let Some(palette) = state.palette.clone() else {
        return;
    };
    let Some(parameters) = state.parameters.clone() else {
        return;
    };
    // Units take the selected unit day/night light at the current time of day.
    let sun = match (
        objects.day_night_state().and_then(|s| s.selected()),
        objects.day_fraction(),
    ) {
        (Some([_, unit_light]), Some(fraction)) => match unit_light.sun(fraction) {
            Ok(sun) => Some(sun),
            Err(error) => {
                state.last_error = Some(error);
                None
            }
        },
        // Lit geosets keep Bevy's shading without source sun: this path is not PBR fidelity.
        (None, _) => {
            state.last_error = Some("unit day/night light state is not selected".into());
            None
        }
        (Some(_), None) => {
            state.last_error = Some("unit day fraction is unavailable".into());
            None
        }
    };
    let (channel_scale, cutout) = (parameters.channel_scale, parameters.cutout_reference);
    let live: BTreeSet<_> = objects
        .executed_units()
        .map(|(id, _)| id)
        .chain(
            objects
                .executed_items()
                .filter(|(_, item)| item.held_by.is_none())
                .map(|(id, _)| id),
        )
        .collect();
    let retired: Vec<_> = state
        .actors
        .keys()
        .copied()
        .filter(|id| !live.contains(id))
        .collect();
    for id in retired {
        retire(&mut commands, &mut state, id);
    }
    for (id, unit) in objects.executed_units() {
        if let Some(actor) = state.actors.get(&id)
            && let Ok(mut visibility) = queries.visibility.get_mut(actor.root)
        {
            *visibility = if unit.shown {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
        if !unit.shown {
            continue;
        }
        if unit.animation.is_none() || state.failed.get(&id) == Some(&unit.team_color) {
            continue;
        }
        let color = match team_color(objects, unit) {
            Ok(v) => v,
            Err(e) => {
                state.failed.insert(id, unit.team_color);
                state.last_error = Some(e);
                continue;
            }
        };
        if state
            .actors
            .get(&id)
            .is_some_and(|actor| actor.color != color)
        {
            retire(&mut commands, &mut state, id);
        }
        if !state.actors.contains_key(&id) {
            let result = Actor::spawn(
                &mut commands,
                &mut assets,
                ActorSource {
                    reader: &reader,
                    palette: &palette,
                    visual: Visual::Unit(unit),
                    terrain: &context.terrain,
                    id: Some(id),
                    color,
                    now,
                    channel_scale,
                    cutout,
                    available_images: (512usize << 20).saturating_sub(state.image_bytes),
                    available_vertices: 2_000_000usize.saturating_sub(state.vertices),
                },
            );
            match result {
                Ok((actor, bytes, vertices)) => {
                    state.image_bytes += bytes;
                    state.vertices += vertices;
                    state.actors.insert(id, actor);
                }
                Err(e) => {
                    state.failed.insert(id, unit.team_color);
                    state.last_error = Some(format!("unit {id:?}: {e}"));
                    continue;
                }
            }
        }
        if let Some(actor) = state.actors.get_mut(&id)
            && let Err(e) = actor.update(
                Visual::Unit(unit),
                now,
                &mut assets,
                (&mut queries.visibility, &mut queries.transforms),
                &context.terrain,
                sun.as_ref().map(|s| (s, &parameters)),
            )
        {
            state.last_error = Some(format!("unit {id:?}: {e}"));
        }
    }
    for (id, item) in objects
        .executed_items()
        .filter(|(_, item)| item.held_by.is_none())
    {
        if state.failed.contains_key(&id) {
            continue;
        }
        if !state.item_animations.contains_key(&id) {
            let UnitActors {
                item_models,
                item_source_bytes,
                ..
            } = &mut *state;
            match item_model::load(
                &reader,
                &item.definition,
                &context.sources.map.edition,
                now,
                item_models,
                item_source_bytes,
            ) {
                Ok(animation) => {
                    state.item_animations.insert(id, animation);
                }
                Err(error) => {
                    state.failed.insert(id, None);
                    state.last_error = Some(format!("item {id:?}: {error}"));
                    continue;
                }
            }
        }
        let animation = state.item_animations[&id].clone();
        let visual = Visual::Item(item, &animation);
        if !state.actors.contains_key(&id) {
            let result = Actor::spawn(
                &mut commands,
                &mut assets,
                ActorSource {
                    reader: &reader,
                    palette: &palette,
                    visual,
                    terrain: &context.terrain,
                    id: Some(id),
                    // Items do not carry a team-color override; reject replaceable team slots below.
                    color: 0,
                    now,
                    channel_scale,
                    cutout,
                    available_images: (512usize << 20).saturating_sub(state.image_bytes),
                    available_vertices: 2_000_000usize.saturating_sub(state.vertices),
                },
            );
            match result {
                Ok((actor, bytes, vertices)) => {
                    state.image_bytes += bytes;
                    state.vertices += vertices;
                    state.actors.insert(id, actor);
                }
                Err(error) => {
                    state.failed.insert(id, None);
                    state.last_error = Some(format!("item {id:?}: {error}"));
                    continue;
                }
            }
        }
        if let Some(actor) = state.actors.get_mut(&id)
            && let Err(error) = actor.update(
                visual,
                now,
                &mut assets,
                (&mut queries.visibility, &mut queries.transforms),
                &context.terrain,
                sun.as_ref().map(|s| (s, &parameters)),
            )
        {
            state.last_error = Some(format!("item {id:?}: {error}"));
        }
    }
    sky::project(
        &mut commands,
        &mut state,
        &mut assets,
        &mut queries,
        sky::Input {
            script: objects
                .environment()
                .and_then(|e| e.sky_model.clone())
                .flatten(),
            reader: &reader,
            edition: &context.sources.map.edition,
            palette: &palette,
            terrain: &context.terrain,
            now,
            channel_scale,
            cutout,
        },
    );
    let mut owners = Vec::new();
    let mut orders = bevy::render::sync_world::MainEntityHashMap::default();
    for actor in state.actors.values() {
        let owner = actor.root.into();
        owners.push(owner);
        for (ordinal, entity, _) in &actor.geosets.bound {
            let binding = &actor.runtime.geosets[*ordinal];
            orders.insert(
                (*entity).into(),
                crate::menu_particle_order::SourceOrder {
                    owner,
                    priority_plane: actor.runtime.materials[binding.material_id].priority_plane,
                    kind: crate::menu_particle_order::SourceKind::Geometry,
                    ordinal: *ordinal as u32,
                    sort_far_z: false,
                },
            );
        }
    }
    let orders = Arc::new(orders);
    let owners: Arc<[_]> = owners.into();
    for entity in &queries.cameras {
        commands
            .entity(entity)
            .insert(crate::menu_particle_order::SourceOrderView {
                owners: Arc::clone(&owners),
                source: "executed campaign actors".into(),
                orders: Arc::clone(&orders),
                max_phase_items: 65536,
            });
    }
}

fn retire(commands: &mut Commands, state: &mut UnitActors, id: HandleId) {
    state.item_animations.remove(&id);
    if let Some(actor) = state.actors.remove(&id) {
        commands.entity(actor.root).despawn();
        state.image_bytes -= actor.image_bytes;
        state.vertices -= actor.vertices;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pcx_objects::editor::{FieldDef, FieldKind, FieldSource, ObjectKind, SlkId};
    fn unit() -> ExecutedUnit {
        let field = FieldDef {
            fourcc: *b"S001",
            from: SlkId::UnitMetaData,
            field: "modelScale".into(),
            slk: "UnitUI".into(),
            index: -1,
            declared: "unreal".into(),
            kind: FieldKind::Unreal,
            category: "art".into(),
            source: FieldSource::Column {
                slk: SlkId::UnitUi,
                column: "modelScale".into(),
            },
            measured: true,
        };
        let id = HandleId::new(std::num::NonZeroU64::new(1).unwrap());
        ExecutedUnit {
            definition: Definition {
                rawcode: *b"U001",
                base_rawcode: "U001".into(),
                kind: ObjectKind::Unit,
                fields: BTreeMap::from([(
                    *b"S001",
                    pcx_sim::world::ObjectField {
                        metadata: Some(field),
                        value: Ok(FieldValue::Cell("1.25".into())),
                    },
                )]),
                modifications: Vec::new(),
            },
            executed_owner: 0,
            owner: id,
            team_color: None,
            creation_color: None,
            source: pcx_sim::world::Source {
                install: "synthetic".into(),
                edition: "test".into(),
                map: "fixture".into(),
            },
            x: 9999.,
            y: 9999.,
            position: Some([32., 64.]),
            face: 90.,
            acquire_range: None,
            requested_hero_level: None,
            requested_show_eye_candy: None,
            unit_states: Vec::new(),
            invulnerable: None,
            admitted_order: None,
            abilities: None,
            animation: None,
            activity: None,
            shown: true,
            vitals: None,
            combat: None,
            permanent_strength: 0,
        }
    }
    fn terrain() -> pcx_map::Terrain {
        let mut bytes = b"W3E!".to_vec();
        bytes.extend(11u32.to_le_bytes());
        bytes.push(b'L');
        bytes.extend(1u32.to_le_bytes());
        bytes.extend(1u32.to_le_bytes());
        bytes.extend(b"Ldrt");
        bytes.extend(1u32.to_le_bytes());
        bytes.extend(b"CLdi");
        bytes.extend(2u32.to_le_bytes());
        bytes.extend(2u32.to_le_bytes());
        bytes.extend(0f32.to_le_bytes());
        bytes.extend(0f32.to_le_bytes());
        for _ in 0..4 {
            bytes.extend(8232u16.to_le_bytes());
            bytes.extend(8192u16.to_le_bytes());
            bytes.extend([0, 0, 0]);
        }
        pcx_map::Terrain::from_bytes(bytes).unwrap()
    }
    #[test]
    fn source_model_bounds_pick_only_forward_finite_intersections() {
        let min = Vec3::new(-2.0, -3.0, -4.0);
        let max = Vec3::new(2.0, 3.0, 4.0);
        assert_eq!(
            bounds_hit(Vec3::new(-10.0, 0.0, 0.0), Vec3::X, min, max, 100.0),
            Some(8.0)
        );
        assert_eq!(
            bounds_hit(Vec3::new(-10.0, 0.0, 0.0), Vec3::NEG_X, min, max, 100.0),
            None
        );
        assert_eq!(
            bounds_hit(Vec3::new(-10.0, 5.0, 0.0), Vec3::X, min, max, 100.0),
            None
        );
        assert_eq!(
            bounds_hit(Vec3::new(-10.0, 0.0, 0.0), Vec3::X, min, max, 7.0),
            None
        );
        assert_eq!(bounds_hit(Vec3::ZERO, Vec3::X, min, max, 100.0), Some(0.0));
        for far in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert_eq!(bounds_hit(Vec3::ZERO, Vec3::X, min, max, far), None);
        }
        assert_eq!(bounds_hit(Vec3::ZERO, Vec3::X, min, min, 100.0), None);
        let state = UnitActors::default();
        assert_eq!(
            state.item_hit(
                Ray3d::new(Vec3::ZERO, Dir3::X),
                100.0,
                &Arc::downgrade(&Arc::new(()))
            ),
            None
        );
    }

    #[test]
    fn every_material_layer_keeps_its_texture_opacity_uv_and_geoset_binding() {
        let mut bytes = b"MDLXVERS".to_vec();
        bytes.extend(4u32.to_le_bytes());
        bytes.extend(800u32.to_le_bytes());
        let mut runtime = pcx_gfx::mdx_runtime::RuntimePlan::parse(&bytes).unwrap();
        let layer = |index, filter, alpha| pcx_mdl::MaterialLayer {
            offset: 0,
            size: 28,
            properties: pcx_mdl::TexLayer {
                index,
                shader_id: filter,
                blend_mode: 0,
                filter_mode: filter,
                shading_flags: 0,
                coord_id: 0,
                texture_id: index,
                texture_anim_id: pcx_mdl::NO_ID,
                static_alpha: alpha,
            },
            alpha_tracks: Vec::new(),
            texture_tracks: Vec::new(),
        };
        runtime
            .materials
            .push(pcx_gfx::mdx_runtime::MaterialBinding {
                priority_plane: -3,
                flags: 0,
                layers: vec![layer(0, 0, 0.5), layer(1, 2, 0.8)],
            });
        runtime.geosets.push(pcx_gfx::mdx_runtime::GeosetBinding {
            geoset_id: 0,
            vertex_count: 1,
            material_id: 0,
            animation_records: vec![0],
            matrix_joint_ids: Vec::new(),
        });
        runtime.geoset_animations.push(pcx_mdl::GeosetAnim {
            offset: 0,
            len: 28,
            flags: 0,
            geoset_id: 0,
            static_alpha: 0.5,
            static_colour: [1.; 3],
            tracks: Vec::new(),
            trailing: 0,
        });
        runtime.uv_set_counts.push(Some(1));
        for slot in 0..2 {
            runtime.textures.push(pcx_mdl::Texture {
                slot,
                path: format!("fixture-{slot}.blp"),
                flags: 0,
            });
        }
        let mut model = pcx_gfx::LoadedModel {
            name: "synthetic layers".into(),
            minimum_extent: [0.; 3],
            maximum_extent: [1.; 3],
            meshes: vec![pcx_gfx::LoadedMesh {
                positions: vec![[1., 2., 3.]],
                normals: vec![[0., 0., 1.]],
                tex_coords: vec![[0.25, 0.75]],
                indices: Vec::new(),
                vertex_groups: Vec::new(),
                bone_indices: Vec::new(),
                group_counts: Vec::new(),
                geoset: 0,
                material_id: 0,
            }],
            bone_names: Vec::new(),
            bone_parents: Vec::new(),
            sequences: Vec::new(),
        };
        assert!(expand_layers(&mut model, &mut runtime, 1).is_err());
        assert_eq!(model.meshes.len(), 1);
        assert_eq!(runtime.geosets.len(), 1);
        assert_eq!(
            expand_layers(&mut model, &mut runtime, 2).unwrap(),
            (2, vec![(0, 0), (0, 1)])
        );
        assert_eq!(model.meshes.len(), 2);
        for i in 0..2 {
            assert_eq!(model.meshes[i].positions, vec![[1., 2., 3.]]);
            assert_eq!(model.meshes[i].tex_coords, vec![[0.25, 0.75]]);
            assert_eq!(runtime.geosets[i].geoset_id, i);
            assert_eq!(runtime.geosets[i].animation_records, vec![i]);
            assert_eq!(runtime.geoset_animations[i].geoset_id, i as u32);
            assert_eq!(runtime.materials[i].priority_plane, -3);
            assert_eq!(
                runtime.materials[i].layers[0].properties.texture_id,
                i as u32
            );
        }
        let alpha = pcx_gfx::mdx_opacity::OpacityPlan::checked(&runtime, [0, 1000]).unwrap();
        assert_eq!(alpha.sample(0, 0, 0), Some(0.25));
        assert_eq!(alpha.sample(1, 0, 0), Some(0.4));
        let textures =
            pcx_gfx::mdx_texture_selection::TextureSelectionPlan::checked(&runtime, [0, 1000])
                .unwrap();
        assert_eq!(textures.sample(0, 0, 0), Some(0));
        assert_eq!(textures.sample(1, 0, 0), Some(1));
    }
    #[test]
    fn actor_uses_accepted_position_terrain_height_and_source_axis_rotation() {
        let mut unit = unit();
        let terrain = terrain();
        let transform = transform_for(&unit, &terrain, 2., 7.).unwrap();
        assert_eq!(transform.translation, Vec3::new(32., 17., -64.));
        assert_eq!(transform.scale, Vec3::splat(2.));
        assert!((transform.rotation * Vec3::X - Vec3::NEG_Z).length() < 0.00001);
        unit.position = None;
        assert!(transform_for(&unit, &terrain, 2., 7.).is_err());
    }
    #[test]
    fn actor_scalar_retains_map_overrides_and_rejects_bad_metadata_or_values() {
        let mut unit = unit();
        assert_eq!(scalar(&unit.definition, "modelScale").unwrap(), 1.25);
        unit.definition.fields.get_mut(b"S001").unwrap().value =
            Ok(FieldValue::Override(pcx_objects::Value::Unreal(2.75)));
        assert_eq!(scalar(&unit.definition, "modelScale").unwrap(), 2.75);
        unit.definition.fields.get_mut(b"S001").unwrap().value = Ok(FieldValue::Cell("NaN".into()));
        assert!(scalar(&unit.definition, "modelScale").is_err());
        unit.definition
            .fields
            .get_mut(b"S001")
            .unwrap()
            .metadata
            .as_mut()
            .unwrap()
            .index = 0;
        assert!(scalar(&unit.definition, "modelScale").is_err());
    }
    #[test]
    fn declared_team_color_enum_keeps_its_string_wire_override() {
        let mut unit = unit();
        let field = unit.definition.fields.get_mut(b"S001").unwrap();
        let metadata = field.metadata.as_mut().unwrap();
        metadata.kind = FieldKind::Token;
        metadata.declared = "teamColor".into();
        metadata.source = FieldSource::Column {
            slk: SlkId::UnitUi,
            column: "teamColor".into(),
        };
        field.value = Ok(FieldValue::Cell("-1".into()));
        assert_eq!(scalar(&unit.definition, "teamColor").unwrap(), -1.);
        unit.definition.fields.get_mut(b"S001").unwrap().value =
            Ok(FieldValue::Override(pcx_objects::Value::Str(b"7".to_vec())));
        assert_eq!(scalar(&unit.definition, "teamColor").unwrap(), 7.);
        unit.definition.fields.get_mut(b"S001").unwrap().value =
            Ok(FieldValue::Override(pcx_objects::Value::Int(7)));
        assert!(scalar(&unit.definition, "teamColor").is_err());
        let field = unit.definition.fields.get_mut(b"S001").unwrap();
        field.value = Ok(FieldValue::Cell("7".into()));
        field.metadata.as_mut().unwrap().declared = "unknownEnum".into();
        assert!(scalar(&unit.definition, "teamColor").is_err());
    }
    #[test]
    fn palette_prefix_and_count_come_from_the_selected_skin() {
        let source =
            b"[Default]\nTeamColors=4\nTeamColor=Selected/TeamColor\nTeamGlow=Selected/TeamGlow\n";
        let palette = TeamPalette::read(source).unwrap();
        let paths = [
            "Selected/TeamColor03.blp",
            "Selected/TeamGlow03.blp",
            "Other/TeamColor03.blp",
        ]
        .map(|s| pcx_assets::AssetPath::new(s).unwrap());
        assert_eq!(
            palette.path(paths.iter(), 1, 3).unwrap(),
            "selected/teamcolor03.blp"
        );
        assert_eq!(
            palette.path(paths.iter(), 2, 3).unwrap(),
            "selected/teamglow03.blp"
        );
        assert!(palette.path(paths.iter(), 1, 4).is_err());
        assert!(palette.path(paths.iter(), 1, -1).is_err());
        assert!(palette.path(paths.iter(), 0, 3).is_err());
        assert!(palette.path(paths.iter(), 1, 2).is_err());
        assert!(
            palette
                .path(paths.iter().chain(std::iter::once(&paths[0])), 1, 3)
                .is_err()
        );
        assert!(TeamPalette::read(b"[Default]\nTeamColors=1\nTeamColor=A\n").is_err());
        assert!(
            TeamPalette::read(b"[Default]\nTeamColors=1\nTeamColor=A\nTeamColor=B\nTeamGlow=C\n")
                .is_err()
        );
    }
}

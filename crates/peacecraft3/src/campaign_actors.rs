//! Render the published session's executed unit handles and their selected source clips.
use crate::{mainmenu_scene as model_scene, menu_material::MenuMaterial};
use bevy::{
    camera::{
        primitives::{Frustum, Sphere},
        visibility::RenderLayers,
    },
    ecs::system::SystemParam,
    prelude::*,
};
use pcx_assets::Resolver;
use pcx_play::{map_startup_host::MapStartupHost, playercolor_host::ExecutedUnit};
use pcx_script::HandleId;

#[path = "campaign_chain_lightning.rs"]
mod chain_lightning;
#[path = "campaign_destructables.rs"]
mod destructables;
#[path = "campaign_doodads.rs"]
mod doodads;
#[path = "campaign_item_model.rs"]
mod item_model;
#[path = "campaign_sky.rs"]
pub(crate) mod sky;
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
struct CampaignActor;

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
    channel_scale: f32,
    material_modes: Vec<(AlphaMode, crate::menu_material::MenuBlend)>,
    scale: f32,
    height: f32,
    color: i32,
    definition_revision: Option<u64>,
    image_bytes: usize,
    vertices: usize,
    /// Geosets whose authored material is lit, captured before sun lighting.
    lit: Vec<bool>,
    world_transform: Transform,
    particles: Option<model_scene::particles::ParticleScene>,
    particle_orders: Option<crate::menu_particle_order::SourceOrderView>,
    particle_clock: std::time::Duration,
    particle_started: f64,
    /// Inputs of the last full update, when it was cacheable.
    update_key: Option<UpdateKey>,
    key_scratch: UpdateKey,
    /// Linear tinted colour per geoset, rebuilt by each full update.
    tints: Vec<[f32; 4]>,
    /// Reused vertex colour buffer for sun lighting.
    colours: Vec<[f32; 4]>,
    /// A full update has run, so culling may skip later ones.
    posed: bool,
}

/// An actor is updated while its bounding sphere, inflated by this factor to cover animation
/// beyond the rest extents, touches the camera frustum.
const CULL_RADIUS_FACTOR: f32 = 2.0;

struct OrderCache {
    fingerprint: u64,
    owners: Arc<[bevy::render::sync_world::MainEntity]>,
    orders:
        Arc<bevy::render::sync_world::MainEntityHashMap<crate::menu_particle_order::SourceOrder>>,
}

#[derive(Resource, Default)]
pub(crate) struct UnitActors {
    session: Option<Weak<()>>,
    reader: Option<Arc<Resolver>>,
    actors: BTreeMap<HandleId, Actor>,
    item_animations: BTreeMap<HandleId, pcx_play::unit_model::Animation>,
    item_models: BTreeMap<String, Arc<pcx_play::unit_model::Model>>,
    item_source_bytes: usize,
    failed: BTreeMap<HandleId, Option<(i32, u64)>>,
    image_bytes: usize,
    vertices: usize,
    parameters: Option<Arc<pcx_play::menu_lighting::Parameters>>,
    parameters_attempted: bool,
    palette: Option<Arc<TeamPalette>>,
    water: Option<Result<WaterSurface, String>>,
    sky: sky::Sky,
    doodads: doodads::Doodads,
    /// Source order of the actor geometry and particles; rebuilt only when the actors change.
    order_view: Option<OrderCache>,
    destructables: destructables::Draw,
    pub last_error: Option<String>,
}

impl UnitActors {
    #[cfg(all(test, not(target_arch = "wasm32"), feature = "test-fixtures"))]
    pub(crate) fn rendered_colors(&self) -> BTreeMap<HandleId, i32> {
        self.actors
            .iter()
            .map(|(&id, actor)| (id, actor.color))
            .collect()
    }

    #[cfg(all(test, not(target_arch = "wasm32"), feature = "test-fixtures"))]
    pub(crate) fn rendered_destructables(&self) -> usize {
        self.destructables
            .instances
            .keys()
            .filter(|id| self.actors.contains_key(id))
            .count()
    }

    #[cfg(all(test, not(target_arch = "wasm32"), feature = "test-fixtures"))]
    pub(crate) fn item_probe(&self, id: HandleId) -> Option<(Entity, Ray3d, f32)> {
        let actor = self.actors.get(&id)?;
        let min = Vec3::from_array(actor.model.minimum_extent);
        let max = Vec3::from_array(actor.model.maximum_extent);
        let width = max.x - min.x;
        if !width.is_finite() || width <= 0. {
            return None;
        }
        let centre = (min + max) * 0.5;
        let origin =
            actor
                .world_transform
                .transform_point(Vec3::new(min.x - width, centre.y, centre.z));
        let end =
            actor
                .world_transform
                .transform_point(Vec3::new(max.x + width, centre.y, centre.z));
        let delta = end - origin;
        Some((
            actor.root,
            Ray3d::new(origin, Dir3::new(delta).ok()?),
            delta.length(),
        ))
    }

    pub(crate) fn item_hit(
        &self,
        ray: Ray3d,
        far: f32,
        session: &Weak<()>,
        objects: &pcx_play::playercolor_host::ObjectHost,
    ) -> Option<HandleId> {
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
                if objects.executed_item(*id).ok()?.visible() != Some(true) {
                    return None;
                }
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
    particle_materials: ResMut<'w, Assets<model_scene::ParticleMaterial>>,
}

#[derive(SystemParam)]
struct ActorQueries<'w, 's> {
    visibility: Query<'w, 's, &'static mut Visibility>,
    transforms: Query<'w, 's, &'static mut Transform, Without<CampaignSceneCamera>>,
    actors: Query<'w, 's, Entity, With<CampaignActor>>,
    cameras: Query<
        'w,
        's,
        (
            Entity,
            Option<&'static crate::menu_particle_order::SourceOrderView>,
        ),
        With<CampaignSceneCamera>,
    >,
    scene: sky::SceneCameras<'w, 's>,
}

#[derive(Clone, Copy)]
struct WaterSurface {
    mapping: pcx_tera::world::Mapping,
    offset: f32,
}

impl WaterSurface {
    fn read(
        reader: &Resolver,
        rules: &pcx_play::unit_placement::Rules,
        terrain: &pcx_map::Terrain,
    ) -> Result<Self, String> {
        let [tile, bias, factor] = rules.terrain_encoding();
        let mapping = pcx_tera::world::Mapping::new(tile, bias, factor)
            .ok_or("invalid admitted terrain encoding")?;
        let asset = reader
            .resolve_bounded("TerrainArt/Water.slk", 4 << 20)
            .map_err(|e| e.to_string())?;
        let table = pcx_slq::Table::parse(&asset.bytes)
            .map_err(|e| format!("TerrainArt/Water.slk: {e}"))?;
        let profile = pcx_tera::WaterProfile::open_scaled(
            &table,
            "TerrainArt/Water.slk",
            pcx_tera::WaterSelection::Tileset,
            terrain,
            tile,
        )
        .map_err(|e| e.to_string())?;
        Ok(Self {
            mapping,
            offset: profile.height_offset(),
        })
    }

    fn height(
        self,
        unit: &ExecutedUnit,
        terrain: &pcx_map::Terrain,
        edition: &str,
        source: &str,
    ) -> Result<f32, String> {
        let [x, y] = unit.position.ok_or("unit has no accepted world position")?;
        let ground = self
            .mapping
            .sample_ground(terrain, x, y)
            .ok_or("unit outside admitted terrain")?;
        let water = pcx_tera::WaterData::new(terrain, edition, source)
            .map_err(|e| e.to_string())?
            .sample_surface(x, y, self.mapping, self.offset)
            .map_err(|e| e.to_string())?;
        match unit.movement()? {
            pcx_play::unit_placement::Movement::Float => {
                water.ok_or_else(|| "floating unit has no authored water surface".into())
            }
            pcx_play::unit_placement::Movement::Amphibious => {
                Ok(water.map_or(ground, |z| ground.max(z)))
            }
            _ => Err("water surface requested for a non-water mover".into()),
        }
    }
}

fn unit_water_height(
    unit: &ExecutedUnit,
    objects: &pcx_play::playercolor_host::ObjectHost,
    reader: &Resolver,
    context: &pcx_play::map_admission::ParsedMapContext,
    cached: &mut Option<Result<WaterSurface, String>>,
) -> Result<Option<f32>, String> {
    if !matches!(
        unit.movement()?,
        pcx_play::unit_placement::Movement::Float | pcx_play::unit_placement::Movement::Amphibious
    ) {
        return Ok(None);
    }
    let surface = cached
        .get_or_insert_with(|| {
            let rules = objects
                .unit_placement_rules()
                .ok_or("water mover terrain encoding is absent")?;
            WaterSurface::read(reader, rules, &context.terrain)
        })
        .as_ref()
        .map_err(Clone::clone)?;
    surface
        .height(
            unit,
            &context.terrain,
            &context.sources.map.edition,
            &context.sources.map.source,
        )
        .map(Some)
}

fn transform_for(
    unit: &ExecutedUnit,
    terrain: &pcx_map::Terrain,
    scale: f32,
    height: f32,
    water: Option<f32>,
) -> Result<Transform, String> {
    let [x, y] = unit.position.ok_or("unit has no accepted world position")?;
    let z = water
        .or_else(|| pcx_tera::world::sample_world_z(terrain, x, y))
        .ok_or("unit position is outside admitted terrain")?
        + unit.flight_height.unwrap_or(height);
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

fn look_at_target<'a>(
    unit: &'a ExecutedUnit,
    objects: &pcx_play::playercolor_host::ObjectHost,
    reader: &Resolver,
    context: &pcx_play::map_admission::ParsedMapContext,
    water: &mut Option<Result<WaterSurface, String>>,
) -> Result<Option<(&'a str, Vec3)>, String> {
    let Some(look) = &unit.look_at else {
        return Ok(None);
    };
    let target = objects
        .executed_unit(look.target)
        .map_err(|e| e.to_string())?;
    let transform = transform_for(
        target,
        &context.terrain,
        1.0,
        scalar(&target.definition, "moveHeight")?,
        unit_water_height(target, objects, reader, context, water)?,
    )?;
    let [x, y, z] = look.offset;
    Ok(Some((
        &look.bone,
        transform.translation + Vec3::new(x, z, -y),
    )))
}

pub(crate) fn team_color(
    objects: &pcx_play::playercolor_host::ObjectHost,
    unit: &ExecutedUnit,
) -> Result<i32, String> {
    objects.display_team_color(unit, original_team_color(objects, unit)?)
}

pub(crate) fn original_team_color(
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
        .init_resource::<chain_lightning::Bolts>()
        .add_systems(
            Update,
            (project, chain_lightning::draw, advance_doodad_particles)
                .chain()
                .after(CampaignCameraSet),
        );
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
            FieldKind::Real | FieldKind::Unreal | FieldKind::Int | FieldKind::Bool
        ) || (column == "teamColor"
            && metadata.kind == FieldKind::Token
            && metadata.declared == "teamColor"))
    {
        return Err(format!("unsupported unit field {column}"));
    }
    if metadata.kind == FieldKind::Bool {
        return match &field.value {
            Ok(FieldValue::Cell(value)) if value.trim() == "0" => Ok(0.),
            Ok(FieldValue::Cell(value)) if value.trim() == "1" => Ok(1.),
            Ok(FieldValue::Override(pcx_objects::Value::Int(0))) => Ok(0.),
            Ok(FieldValue::Override(pcx_objects::Value::Int(1))) => Ok(1.),
            _ => Err(format!("invalid unit boolean {column}")),
        };
    }
    let number = match &field.value {
        Ok(FieldValue::Cell(s)) => s.parse::<f32>().map_err(|e| e.to_string())?,
        Ok(FieldValue::Override(pcx_objects::Value::Int(v))) if metadata.kind == FieldKind::Int => {
            *v as f32
        }
        // Object-data records choose real or unreal per override, not per metadata kind.
        Ok(FieldValue::Override(pcx_objects::Value::Real(v) | pcx_objects::Value::Unreal(v)))
            if matches!(metadata.kind, FieldKind::Real | FieldKind::Unreal) =>
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
pub(crate) struct TeamPalette {
    prefixes: [String; 2],
    count: i32,
}
impl TeamPalette {
    pub(crate) fn read(bytes: &[u8]) -> Result<Self, String> {
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
    pub(crate) fn path<'a>(
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
pub(crate) fn expand_layers(
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
    Unit(&'a ExecutedUnit, Option<(&'a str, Vec3)>, Option<f32>),
    Item(
        &'a pcx_play::playercolor_host::ExecutedItem,
        &'a pcx_play::unit_model::Animation,
    ),
    Doodad(
        &'a pcx_play::campaign_placement::PlacedDoodad,
        &'a pcx_play::campaign_placement::DoodadAppearance,
    ),
    Destructable(&'a destructables::Instance),
    /// The sky dome centred on the scene camera eye.
    Sky(&'a pcx_play::unit_model::Animation, Vec3),
}
impl<'a> Visual<'a> {
    fn definition_revision(self) -> Option<u64> {
        match self {
            Self::Unit(unit, ..) => Some(unit.definition_revision),
            _ => None,
        }
    }
    fn animation(self) -> Result<&'a pcx_play::unit_model::Animation, String> {
        match self {
            Self::Unit(unit, ..) => unit
                .animation
                .as_ref()
                .ok_or_else(|| "unit model is unavailable".into()),
            Self::Item(_, animation) | Self::Sky(animation, _) => Ok(animation),
            Self::Doodad(_, appearance) => Ok(&appearance.animation),
            Self::Destructable(instance) => Ok(&instance.animation),
        }
    }
    fn face(self) -> f32 {
        match self {
            Self::Unit(unit, ..) => unit.face,
            Self::Item(..) | Self::Sky(..) => 0.0,
            Self::Doodad(row, _) => row.source.angle.to_degrees(),
            Self::Destructable(instance) => instance.face,
        }
    }
    fn layer(self) -> usize {
        match self {
            Self::Sky(..) => sky::RENDER_LAYER,
            Self::Unit(..) | Self::Item(..) | Self::Doodad(..) | Self::Destructable(..) => {
                peacecraft3::campaign_camera::RENDER_LAYER
            }
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
            Self::Unit(unit, _, water) => transform_for(unit, terrain, scale, height, water),
            Self::Doodad(row, _) => doodads::transform(row),
            Self::Destructable(instance) => Ok(instance.pose),
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
            Self::Unit(unit, ..) => Ok((
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
            Self::Doodad(_, appearance) => Ok((1., 0., appearance.tint.map(|c| c * channel_scale))),
            Self::Destructable(instance) => {
                Ok((1., 0., instance.profile.tint.map(|c| c * channel_scale)))
            }
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
    color: i32,
    now: f64,
    channel_scale: f32,
    cutout: f32,
    particle_parameters: Option<&'a pcx_play::menu_lighting::Parameters>,
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
            color,
            now,
            channel_scale,
            cutout,
            particle_parameters,
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
        let mut required: BTreeSet<_> = runtime
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
        if particle_parameters.is_some() {
            for emitter in mdl.particles2(bytes).map_err(|e| e.to_string())? {
                required.insert(emitter.properties().texture_id);
            }
        }
        for (index, texture) in runtime.textures.iter_mut().enumerate() {
            if !required.contains(&(index as u32)) {
                continue;
            }
            if let Visual::Destructable(instance) = visual
                && let Some((slot, path)) = &instance.profile.replacement
                && texture.slot == *slot
            {
                texture.path.clone_from(path);
            }
            if texture.path.is_empty() {
                if matches!(
                    visual,
                    Visual::Item(..) | Visual::Doodad(..) | Visual::Destructable(..)
                ) {
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
        let particle_prepared = if let Some(parameters) = particle_parameters {
            let clip = animation.sequence.map(|i| {
                let sequence = &animation.model.sequences[i];
                (
                    pcx_gfx::mdx_particles::Clip {
                        interval: [sequence.interval_start, sequence.interval_end],
                        looping: !sequence.non_looping,
                    },
                    None,
                )
            });
            match model_scene::particles::prepare(
                &source.member,
                &mdl,
                bytes,
                &runtime,
                &textures,
                clip,
                Some(parameters),
            )
            .map_err(|e| e.to_string())?
            {
                model_scene::particles::Admission::Absent => None,
                model_scene::particles::Admission::Ready(prepared) => Some(prepared),
                model_scene::particles::Admission::Unsupported(error) => {
                    return Err(format!("{}: PRE2: {error}", source.member));
                }
            }
        } else {
            None
        };
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
        let visibility = match visual {
            Visual::Doodad(row, _) if !row.shown => Visibility::Hidden,
            _ => Visibility::default(),
        };
        let mut root = commands.spawn((initial_transform, visibility));
        if !matches!(visual, Visual::Sky(..)) {
            root.insert(CampaignActor);
        }
        let root = root.id();
        let mut info = model_scene::SceneInfo::default();
        let mut geosets = model_scene::spawn_geosets(
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
        let particle_clock =
            std::time::Duration::try_from_secs_f64(animation.started - animation.created)
                .map_err(|e| e.to_string())?;
        let (particles, particle_orders) = if let Some(prepared) = particle_prepared {
            let (mut particles, orders) = prepared.spawn(
                commands,
                model_scene::particles::ParticleAssets {
                    meshes: &mut assets.meshes,
                    materials: &mut assets.particle_materials,
                    images: &mut assets.images,
                },
                &mut geosets.textures,
                root,
                &geosets.bound,
                &runtime,
            );
            particles
                .start_at(particle_clock)
                .map_err(|e| e.to_string())?;
            particles.attach_to(commands, root, visual.layer());
            (Some(particles), Some(orders))
        } else {
            (None, None)
        };
        let lit = geosets
            .bound
            .iter()
            .map(|(_, _, handle)| assets.materials.get(handle).is_some_and(|m| !m.base.unlit))
            .collect();
        let material_modes = geosets
            .bound
            .iter()
            .map(|(_, _, handle)| {
                let material = assets
                    .materials
                    .get(handle)
                    .expect("spawned actor material");
                (material.base.alpha_mode, material.extension.clone())
            })
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
                channel_scale,
                material_modes,
                scale,
                height,
                color,
                definition_revision: visual.definition_revision(),
                image_bytes: available_images - remaining,
                vertices,
                lit,
                world_transform: initial_transform,
                particles,
                particle_orders,
                particle_clock,
                particle_started: animation.started,
                update_key: None,
                key_scratch: UpdateKey::default(),
                tints: Vec::new(),
                colours: Vec::new(),
                posed: false,
            },
            available_images - remaining,
            vertices,
        ))
    }
    /// True when the actor's inflated bounding sphere can touch the view; an actor outside it
    /// is not drawn this frame, so its pose and materials may stay stale until it returns.
    fn in_view(&self, view: &Frustum) -> bool {
        extent_in_view(
            view,
            self.model.minimum_extent,
            self.model.maximum_extent,
            &self.world_transform,
        )
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
        (light, view): (
            Option<(
                &pcx_play::day_night::Sun,
                &pcx_play::menu_lighting::Parameters,
            )>,
            Option<&Frustum>,
        ),
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
        if self.posed && view.is_some_and(|view| !self.in_view(view)) {
            return Ok(());
        }
        let blend = animation.blend_sample(now)?;
        let tint = match visual {
            Visual::Unit(unit, ..) => unit_tint(unit, self.tint, self.channel_scale),
            _ => [self.tint[0], self.tint[1], self.tint[2], 1.0],
        };
        if tint[3] < 1.0
            && self
                .material_modes
                .iter()
                .any(|(mode, _)| *mode == AlphaMode::Multiply)
        {
            return Err("scripted unit opacity on modulate layers is not qualified".into());
        }
        let elapsed_ms =
            u32::try_from(sample.elapsed_ms).map_err(|_| "unit global animation clock overflow")?;
        let look_at = match visual {
            Visual::Unit(_, Some((bone, target)), _) => Some((bone, target)),
            _ => None,
        };
        // The model faces +X at zero facing: undo the unit's facing around Z.
        let lighting =
            light
                .filter(|_| self.lit.iter().any(|lit| *lit))
                .map(|(sun, parameters)| {
                    let (sin, cos) = visual.face().to_radians().sin_cos();
                    let [x, y, z] = sun.direction;
                    (
                        Vec3::new(x * cos + y * sin, -x * sin + y * cos, z),
                        sun,
                        parameters,
                    )
                });
        // Every output below is a pure function of this key, so an unchanged key leaves the
        // meshes and materials exactly as the previous update wrote them.
        let previous = self.update_key.take();
        let mut key = std::mem::take(&mut self.key_scratch);
        key.fill(
            &sample,
            &self.runtime.global_sequences,
            tint,
            look_at.map(|(_, target)| (target, &self.world_transform)),
            lighting
                .as_ref()
                .map(|(direction, sun, _)| (*direction, **sun)),
        );
        if blend.is_none() && previous.as_ref() == Some(&key) {
            self.key_scratch = key;
            self.update_key = previous;
            return Ok(());
        }
        let skeleton = model_scene::skin_geosets(
            model_scene::Rig {
                blend,
                interval: self.interval,
                model: &self.model,
                bones: self.runtime.joints.joints(),
                pivots: self.runtime.joints.pivots(),
                global_durations: &self.runtime.global_sequences,
                camera_pose: None,
                look_at: look_at.map(|(bone, target)| {
                    (
                        bone,
                        self.world_transform
                            .compute_affine()
                            .inverse()
                            .transform_point3(target),
                    )
                }),
            },
            &self.geosets.handles,
            sample.frame,
            elapsed_ms,
            &mut assets.meshes,
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
        // Opacity, colour and tint compose into one value per geoset; a material is only
        // written when that value differs, so unchanged geosets raise no asset event.
        self.tints.clear();
        for (index, ((geoset, entity, handle), (mode, blend))) in self
            .geosets
            .bound
            .iter()
            .zip(&self.material_modes)
            .enumerate()
        {
            let current = assets
                .materials
                .get(handle)
                .ok_or_else(|| format!("geoset {geoset}: missing bound colour material"))?;
            let alpha = self
                .opacity
                .sample(*geoset, sample.frame, sample.elapsed_ms)
                .unwrap_or_else(|| current.base.base_color.alpha());
            let rgb = self
                .opacity
                .sample_material_colour(*geoset, sample.frame, sample.elapsed_ms)
                .map_err(|error| error.to_string())?;
            let colour = model_scene::checked_geoset_colour(*geoset, rgb, alpha)
                .map_err(|error| error.to_string())?;
            let c = colour.to_linear();
            let tinted = Color::linear_rgba(
                c.red * tint[0],
                c.green * tint[1],
                c.blue * tint[2],
                c.alpha * tint[3],
            );
            let (mode, blend) = blend.with_unit_opacity(*mode, tint[3]);
            self.tints.push(tinted.to_linear().to_f32_array());
            // Sun-lit geosets get their colour from the lighting pass below.
            let sunlit = lighting.is_some() && self.lit[index];
            if (!sunlit && current.base.base_color != tinted)
                || current.base.alpha_mode != mode
                || current.extension != blend
            {
                let mut material = assets.materials.get_mut(handle).expect("checked material");
                if !sunlit {
                    material.base.base_color = tinted;
                }
                material.base.alpha_mode = mode;
                material.extension = blend;
            }
            if let Ok(mut visible) = visibility.get_mut(*entity) {
                let wanted = if c.alpha * tint[3] <= 0.0 {
                    Visibility::Hidden
                } else {
                    Visibility::Inherited
                };
                if *visible != wanted {
                    *visible = wanted;
                }
            }
        }
        if let Some((direction, sun, parameters)) = lighting {
            crate::menu_lighting::apply_sun(
                &animation.model.member,
                &self.model,
                &skeleton,
                (&self.geosets.handles, &self.geosets.bound, &self.lit),
                &self.tints,
                (direction, sun, parameters),
                (&mut assets.meshes, &mut assets.materials, &mut self.colours),
            )?;
        }
        self.key_scratch = previous.unwrap_or_default();
        self.update_key = blend.is_none().then_some(key);
        self.posed = true;
        Ok(())
    }
}

/// True when the sphere around a model's extents, placed by `transform` and inflated by
/// [`CULL_RADIUS_FACTOR`], can touch the view. Extents with no usable size always count.
fn extent_in_view(
    view: &Frustum,
    minimum: [f32; 3],
    maximum: [f32; 3],
    transform: &Transform,
) -> bool {
    let (minimum, maximum) = (Vec3::from(minimum), Vec3::from(maximum));
    let half = (maximum - minimum) / 2.0;
    let scale = transform.scale.abs().max_element();
    let radius = half.length() * scale * CULL_RADIUS_FACTOR;
    if !(radius.is_finite() && radius > 0.0) {
        return true;
    }
    let center = transform.transform_point((minimum + maximum) / 2.0);
    view.intersects_sphere(
        &Sphere {
            center: center.into(),
            radius,
        },
        true,
    )
}

/// Inputs of one actor update beyond the unchanging model. Equal keys produce equal assets.
#[derive(Clone, Debug, Default, PartialEq)]
struct UpdateKey {
    interval: [u32; 2],
    frame: u32,
    /// Each global sequence's phase: tracks on it sample `elapsed_ms % duration`.
    phases: Vec<u32>,
    tint: [u32; 4],
    /// Look-at target and the world transform it is expressed through, as bits.
    look_at: Option<[u32; 13]>,
    /// Sun direction in model space and the sun, present only for lit geosets.
    light: Option<([u32; 3], pcx_play::day_night::Sun)>,
}

impl UpdateKey {
    fn fill(
        &mut self,
        sample: &pcx_play::unit_model::Sample,
        global_sequences: &[u32],
        tint: [f32; 4],
        look_at: Option<(Vec3, &Transform)>,
        light: Option<(Vec3, pcx_play::day_night::Sun)>,
    ) {
        self.interval = sample.interval;
        self.frame = sample.frame;
        self.phases.clear();
        self.phases.extend(global_sequences.iter().map(|&duration| {
            if duration == 0 {
                0
            } else {
                (sample.elapsed_ms % u64::from(duration)) as u32
            }
        }));
        self.tint = tint.map(f32::to_bits);
        self.look_at = look_at.map(|(target, transform)| {
            let [rx, ry, rz, rw] = transform.rotation.to_array();
            let [tx, ty, tz] = transform.translation.to_array();
            let [sx, sy, sz] = transform.scale.to_array();
            let [x, y, z] = target.to_array();
            [x, y, z, tx, ty, tz, rx, ry, rz, rw, sx, sy, sz].map(f32::to_bits)
        });
        self.light = light.map(|(direction, sun)| (direction.to_array().map(f32::to_bits), sun));
    }
}

fn unit_tint(unit: &ExecutedUnit, source: [f32; 3], channel_scale: f32) -> [f32; 4] {
    match unit.vertex_color {
        Some([r, g, b, a]) => [
            f32::from(r) * channel_scale,
            f32::from(g) * channel_scale,
            f32::from(b) * channel_scale,
            f32::from(a) / f32::from(u8::MAX),
        ],
        None => [source[0], source[1], source[2], 1.0],
    }
}

impl Actor {
    fn particle_step(
        &mut self,
        target: std::time::Duration,
        camera: &Transform,
        assets: &mut DrawAssets,
        visibility: &mut Query<&mut Visibility>,
    ) -> Result<(), String> {
        let Some(particles) = self.particles.as_mut() else {
            return Ok(());
        };
        let delta = target
            .checked_sub(self.particle_clock)
            .ok_or("doodad particle clock moved backward")?;
        for (entity, shown) in particles
            .advance(
                delta,
                target,
                (
                    camera,
                    self.world_transform.scale * (camera.rotation * Vec3::Z),
                ),
                &mut assets.meshes,
                &assets.particle_materials,
                |entity| visibility.contains(entity),
            )
            .map_err(|e| e.to_string())?
        {
            if let Ok(mut value) = visibility.get_mut(entity) {
                *value = if shown {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            }
        }
        self.particle_clock = target;
        Ok(())
    }

    fn advance_particles(
        &mut self,
        animation: &pcx_play::unit_model::Animation,
        now: f64,
        camera: &Transform,
        assets: &mut DrawAssets,
        visibility: &mut Query<&mut Visibility>,
        parameters: &pcx_play::menu_lighting::Parameters,
    ) -> Result<(), String> {
        if self.particles.is_none() {
            return Ok(());
        }
        if self.particle_started != animation.started {
            let at = std::time::Duration::try_from_secs_f64(animation.started - animation.created)
                .map_err(|e| e.to_string())?;
            self.particle_step(at, camera, assets, visibility)?;
            let sequence = animation
                .sequence
                .and_then(|i| animation.model.sequences.get(i))
                .ok_or("doodad particles need a selected clip")?;
            let model = pcx_mdl::Mdlx::parse(&animation.model.bytes).map_err(|e| e.to_string())?;
            let changed = self
                .particles
                .as_mut()
                .expect("present")
                .select_clip(
                    &model,
                    &animation.model.bytes,
                    &self.runtime,
                    pcx_gfx::mdx_particles::Clip {
                        interval: [sequence.interval_start, sequence.interval_end],
                        looping: !sequence.non_looping,
                    },
                    at,
                    parameters,
                )
                .map_err(|e| e.to_string())?;
            if !changed {
                return Ok(());
            }
            self.particle_started = animation.started;
        }
        let target = std::time::Duration::try_from_secs_f64(now - animation.created)
            .map_err(|e| e.to_string())?;
        self.particle_step(target, camera, assets, visibility)
    }
}

fn advance_doodad_particles(
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
    mut state: ResMut<UnitActors>,
    mut assets: DrawAssets,
    mut queries: ActorQueries,
) {
    if !matches!((&state.session, session.session_witness()), (Some(old), Some(new)) if Weak::ptr_eq(old, &new))
    {
        return;
    }
    let Some(host) = session.host() else {
        return;
    };
    let objects = host.objects();
    let (Some(now), Some(parameters)) = (objects.timer_game_time(), state.parameters.clone())
    else {
        return;
    };
    destructables::advance_particles(&mut state, &mut assets, &mut queries, now, &parameters);
    doodads::advance_particles(
        &mut state,
        &mut assets,
        &mut queries,
        objects.placed_doodads(),
        now,
        &parameters,
    );
}

#[derive(Clone, Copy)]
struct LocalViewer {
    identity: HandleId,
    fog: bool,
    mask: bool,
}

fn unit_publication_sight_gate(
    readiness: Result<(), &'static str>,
    admitted: bool,
) -> Result<bool, &'static str> {
    match readiness {
        Err("shared vision is not admitted") if !admitted => Ok(false),
        Err(reason) => Err(reason),
        Ok(()) => Ok(true),
    }
}

fn unit_publication_sight(
    objects: &pcx_play::playercolor_host::ObjectHost,
) -> Result<Option<LocalViewer>, String> {
    if !unit_publication_sight_gate(objects.vision_readiness(), objects.shared_vision_admitted())? {
        return Ok(None);
    }
    let Some(settings) = objects.startup_settings() else {
        return Err("startup settings are missing".into());
    };
    let Some(local) = objects.local_player_settings() else {
        return Err("local viewer is not admitted".into());
    };
    Ok(Some(LocalViewer {
        identity: local.identity,
        fog: settings.fog_enabled,
        mask: settings.fog_mask_enabled,
    }))
}

fn shown_unit_is_published(
    shown: bool,
    position: Option<[f32; 2]>,
    filter: bool,
    presented: impl FnOnce([f32; 2]) -> Result<Option<pcx_play::playercolor_host::VisionState>, String>,
) -> Result<bool, String> {
    if !shown || !filter {
        return Ok(shown);
    }
    let Some(position) = position else {
        return Ok(true);
    };
    match presented(position)? {
        Some(pcx_play::playercolor_host::VisionState::Visible) => Ok(true),
        Some(
            pcx_play::playercolor_host::VisionState::Fogged
            | pcx_play::playercolor_host::VisionState::Masked,
        )
        | None => Ok(false),
    }
}

fn project(
    mut commands: Commands,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
    sources: ActorSources,
    mut state: ResMut<UnitActors>,
    mut bolts: ResMut<chain_lightning::Bolts>,
    mut assets: DrawAssets,
    mut queries: ActorQueries,
) {
    let witness = session.session_witness();
    if !matches!((&state.session,&witness),(Some(a),Some(b)) if Weak::ptr_eq(a,b)) {
        for entity in &queries.actors {
            commands.entity(entity).try_despawn();
        }
        sky::release(&mut commands, &mut state, &mut queries.scene);
        *state = UnitActors {
            session: witness,
            ..default()
        };
        chain_lightning::release(&mut bolts);
    }
    let Some(context) = session.context().cloned() else {
        chain_lightning::release(&mut bolts);
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
    let objects = host.objects_mut();
    let Some(now) = objects.timer_game_time() else {
        return;
    };
    if !state.parameters_attempted {
        state.parameters_attempted = true;
        let install = crate::launch_gui::selected_install(&sources.launcher);
        match crate::menu_lighting::parameters(install.as_deref()) {
            Ok(v) => state.parameters = Some(Arc::new(v)),
            Err(e) => state.last_error = Some(e),
        }
        match reader
            .resolve_bounded("UI/war3skins.txt", 1 << 20)
            .map_err(|e| e.to_string())
            .and_then(|asset| TeamPalette::read(&asset.bytes))
        {
            Ok(palette) => state.palette = Some(Arc::new(palette)),
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
    // The camera was placed earlier this frame; the frustum component still holds the last one.
    let view = queries
        .scene
        .single()
        .ok()
        .map(|(_, transform, projection, ..)| {
            projection.compute_frustum(&GlobalTransform::from(*transform))
        });
    let sight = if objects.executed_units().next().is_none() {
        None
    } else {
        match unit_publication_sight(objects) {
            Ok(sight) => sight,
            Err(reason) => {
                state.last_error = Some(reason);
                return;
            }
        }
    };
    if let Some(viewer) = sight {
        for (_, unit) in objects.executed_units() {
            if !unit.shown {
                continue;
            }
            let Some(position) = unit.position else {
                continue;
            };
            if let Err(error) =
                objects.vision_presented(viewer.identity, position, viewer.fog, viewer.mask)
            {
                state.last_error = Some(error.to_string());
                return;
            }
        }
    }
    let live: BTreeSet<_> = objects
        .executed_units()
        .map(|(id, _)| id)
        .chain(
            objects
                .executed_items()
                .filter(|(_, item)| item.held_by.is_none())
                .map(|(id, _)| id),
        )
        .chain(objects.destructables_with_handles().map(|(id, _, _)| id))
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
        let decision = match sight {
            Some(viewer) => shown_unit_is_published(unit.shown, unit.position, true, |position| {
                objects
                    .vision_presented(viewer.identity, position, viewer.fog, viewer.mask)
                    .map_err(|error| error.to_string())
            }),
            None => shown_unit_is_published(unit.shown, unit.position, false, |_| Ok(None)),
        };
        let publish = match decision {
            Ok(publish) => publish,
            Err(error) => {
                state.last_error = Some(error);
                return;
            }
        };
        if let Some(actor) = state.actors.get(&id)
            && let Ok(mut visibility) = queries.visibility.get_mut(actor.root)
        {
            *visibility = if publish {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
        if !publish {
            continue;
        }
        if unit.animation.is_none() {
            continue;
        }
        let water = match unit_water_height(unit, objects, &reader, &context, &mut state.water) {
            Ok(height) => height,
            Err(error) => {
                state.last_error = Some(format!("unit {id:?}: {error}"));
                continue;
            }
        };
        let look_at = match look_at_target(unit, objects, &reader, &context, &mut state.water) {
            Ok(target) => target,
            Err(error) => {
                state.last_error = Some(format!("unit {id:?}: {error}"));
                continue;
            }
        };
        let color = match team_color(objects, unit) {
            Ok(v) => v,
            Err(e) => {
                state.failed.insert(id, None);
                state.last_error = Some(e);
                continue;
            }
        };
        if state.failed.get(&id) == Some(&Some((color, unit.definition_revision))) {
            continue;
        }
        if state.actors.get(&id).is_some_and(|actor| {
            actor.color != color || actor.definition_revision != Some(unit.definition_revision)
        }) {
            retire(&mut commands, &mut state, id);
        }
        if !state.actors.contains_key(&id) {
            let result = Actor::spawn(
                &mut commands,
                &mut assets,
                ActorSource {
                    reader: &reader,
                    palette: &palette,
                    visual: Visual::Unit(unit, look_at, water),
                    terrain: &context.terrain,
                    color,
                    now,
                    channel_scale,
                    cutout,
                    particle_parameters: None,
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
                    state
                        .failed
                        .insert(id, Some((color, unit.definition_revision)));
                    state.last_error = Some(format!("unit {id:?}: {e}"));
                    continue;
                }
            }
        }
        if let Some(actor) = state.actors.get_mut(&id)
            && let Err(e) = actor.update(
                Visual::Unit(unit, look_at, water),
                now,
                &mut assets,
                (&mut queries.visibility, &mut queries.transforms),
                &context.terrain,
                (sun.as_ref().map(|s| (s, &*parameters)), view.as_ref()),
            )
        {
            state.last_error = Some(format!("unit {id:?}: {e}"));
        }
    }
    let draw = doodads::Input {
        reader: &reader,
        terrain: &context.terrain,
        palette: &palette,
        parameters: &parameters,
        now,
        sun: sun.as_ref(),
        view: view.as_ref(),
    };
    project_ground_items(
        &mut commands,
        &mut state,
        &mut assets,
        &mut queries,
        &draw,
        &context.sources.map.edition,
        objects
            .executed_items()
            .filter(|(_, item)| item.held_by.is_none()),
    );
    destructables::project(
        &mut commands,
        &mut state,
        &mut assets,
        &mut queries,
        &draw,
        objects,
        &context.sources.map.edition,
    );
    doodads::project(
        &mut commands,
        &mut state,
        &mut assets,
        &mut queries,
        draw,
        objects.placed_doodads(),
    );
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
            day_fraction: objects.day_fraction(),
            parameters: &parameters,
        },
    );
    // Each actor's roots, particle orders and geoset count are fixed from spawn, so a
    // fingerprint of them identifies the previous frame's order tables.
    let fingerprint = {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::hash::DefaultHasher::new();
        for actor in state.actors.values().chain(state.doodads.actors.values()) {
            actor.root.hash(&mut hasher);
            actor
                .particle_orders
                .as_ref()
                .map(|particles| Arc::as_ptr(&particles.orders) as usize)
                .hash(&mut hasher);
            actor.geosets.bound.len().hash(&mut hasher);
        }
        hasher.finish()
    };
    if state
        .order_view
        .as_ref()
        .is_none_or(|cache| cache.fingerprint != fingerprint)
    {
        let mut owners = Vec::new();
        let mut orders = bevy::render::sync_world::MainEntityHashMap::default();
        for actor in state.actors.values().chain(state.doodads.actors.values()) {
            let owner = actor.root.into();
            owners.push(owner);
            if let Some(particles) = &actor.particle_orders {
                orders.extend(
                    particles
                        .orders
                        .iter()
                        .map(|(entity, order)| (*entity, *order)),
                );
            }
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
        state.order_view = Some(OrderCache {
            fingerprint,
            owners: owners.into(),
            orders: Arc::new(orders),
        });
    }
    let cache = state.order_view.as_ref().expect("order tables built");
    for (entity, current) in &queries.cameras {
        if current.is_some_and(|view| {
            Arc::ptr_eq(&view.owners, &cache.owners) && Arc::ptr_eq(&view.orders, &cache.orders)
        }) {
            continue;
        }
        commands
            .entity(entity)
            .insert(crate::menu_particle_order::SourceOrderView {
                owners: Arc::clone(&cache.owners),
                source: "executed campaign actors".into(),
                orders: Arc::clone(&cache.orders),
                max_phase_items: 65536,
            });
    }
    if let Err(error) = chain_lightning::queue(objects, &reader, &context.terrain, now, &mut bolts)
    {
        state.last_error = Some(format!("chain lightning: {error}"));
    }
}

fn project_ground_items<'a>(
    commands: &mut Commands,
    state: &mut UnitActors,
    assets: &mut DrawAssets,
    queries: &mut ActorQueries,
    draw: &doodads::Input<'_>,
    edition: &str,
    items: impl Iterator<Item = (HandleId, &'a pcx_play::playercolor_host::ExecutedItem)>,
) {
    let (channel_scale, cutout) = (
        draw.parameters.channel_scale,
        draw.parameters.cutout_reference,
    );
    for (id, item) in items {
        if let Some(actor) = state.actors.get(&id)
            && let Ok(mut visibility) = queries.visibility.get_mut(actor.root)
        {
            *visibility = if item.visible() == Some(true) {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
        if item.visible().is_none() {
            state.last_error = Some("item visibility rules missing".into());
            continue;
        }
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
                draw.reader,
                &item.definition,
                edition,
                draw.now,
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
        // Admit the animation before culling so hiding a newborn item does not delay its clock.
        if item.visible() != Some(true) {
            continue;
        }
        let animation = state.item_animations[&id].clone();
        let visual = Visual::Item(item, &animation);
        if !state.actors.contains_key(&id) {
            let result = Actor::spawn(
                commands,
                assets,
                ActorSource {
                    reader: draw.reader,
                    palette: draw.palette,
                    visual,
                    terrain: draw.terrain,
                    // Items do not carry a team-color override; reject replaceable team slots below.
                    color: 0,
                    now: draw.now,
                    channel_scale,
                    cutout,
                    particle_parameters: None,
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
                draw.now,
                assets,
                (&mut queries.visibility, &mut queries.transforms),
                draw.terrain,
                (draw.sun.map(|sun| (sun, draw.parameters)), draw.view),
            )
        {
            state.last_error = Some(format!("item {id:?}: {error}"));
        }
    }
}

fn retire(commands: &mut Commands, state: &mut UnitActors, id: HandleId) {
    state.item_animations.remove(&id);
    state.destructables.instances.remove(&id);
    if let Some(actor) = state.actors.remove(&id) {
        commands.entity(actor.root).try_despawn();
        state.image_bytes -= actor.image_bytes;
        state.vertices -= actor.vertices;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pcx_objects::editor::{FieldDef, FieldKind, FieldSource, ObjectKind, SlkId};
    #[test]
    fn extents_outside_the_inflated_view_are_culled_and_degenerate_ones_never() {
        let view = Projection::from(PerspectiveProjection::default())
            .compute_frustum(&GlobalTransform::default());
        let unit = ([-1.0; 3], [1.0; 3]);
        let at = |x: f32, z: f32| Transform::from_xyz(x, 0.0, z);
        // Straight ahead (-Z), and far to the side where even the doubled radius misses.
        assert!(extent_in_view(&view, unit.0, unit.1, &at(0.0, -5.0)));
        assert!(!extent_in_view(&view, unit.0, unit.1, &at(1000.0, -5.0)));
        // Behind the eye.
        assert!(!extent_in_view(&view, unit.0, unit.1, &at(0.0, 50.0)));
        // The doubled radius keeps a model whose centre is just off-screen.
        let edge = 5.0 * (PerspectiveProjection::default().fov / 2.0).tan() * 1.6;
        assert!(extent_in_view(&view, unit.0, unit.1, &at(edge, -5.0)));
        // Zero or nonfinite extents have no radius to test.
        assert!(extent_in_view(&view, [0.0; 3], [0.0; 3], &at(1000.0, 50.0)));
        assert!(extent_in_view(
            &view,
            [f32::NAN; 3],
            [1.0; 3],
            &at(1000.0, 50.0)
        ));
    }

    #[test]
    fn update_key_tracks_only_the_inputs_the_outputs_depend_on() {
        let sample = |frame, elapsed_ms| pcx_play::unit_model::Sample {
            interval: [0, 1000],
            frame,
            elapsed_ms,
        };
        let key = |sample: &pcx_play::unit_model::Sample, tint: [f32; 4]| {
            let mut key = UpdateKey::default();
            key.fill(sample, &[400, 0], tint, None, None);
            key
        };
        let base = key(&sample(10, 5000), [1.0; 4]);
        // Global tracks sample elapsed % duration: 5000 % 400 == 6200 % 400, and a zero-length
        // sequence is never sampled, so neither clock difference changes the outputs.
        assert_eq!(base, key(&sample(10, 6200), [1.0; 4]));
        assert_eq!(base, key(&sample(10, 5000 + 400 * 17), [1.0; 4]));
        assert_ne!(base, key(&sample(10, 5001), [1.0; 4]));
        assert_ne!(base, key(&sample(11, 5000), [1.0; 4]));
        assert_ne!(base, key(&sample(10, 5000), [1.0, 1.0, 1.0, 0.5]));
        let mut moved = UpdateKey::default();
        moved.fill(
            &sample(10, 5000),
            &[400, 0],
            [1.0; 4],
            Some((Vec3::X, &Transform::IDENTITY)),
            None,
        );
        assert_ne!(base, moved);
    }

    #[test]
    fn live_vertex_color_replaces_source_tint_and_preserves_authored_opacity_multiplier() {
        let mut unit = unit();
        let source = [0.17, 0.43, 0.79];
        assert_eq!(unit_tint(&unit, source, 0.01), [0.17, 0.43, 0.79, 1.0]);
        unit.vertex_color = Some([23, 67, 131, 51]);
        let tint = unit_tint(&unit, source, 0.01);
        for (actual, expected) in tint.into_iter().zip([0.23, 0.67, 1.31, 0.2]) {
            assert!((actual - expected).abs() < 0.00001);
        }
        unit.vertex_color = Some([0, 0, 0, 0]);
        assert_eq!(unit_tint(&unit, source, 0.01), [0.0; 4]);
        unit.vertex_color = Some([19, 37, 73, 255]);
        assert_eq!(unit_tint(&unit, source, 0.01)[3], 1.0);
    }

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
            illusion: None,
            construction: None,
            construction_revision: 0,
            construction_request: None,
            decay: None,
            collision_radius: Err("missing UnitBalance collision field"),
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
            vision_viewers: Vec::new(),
            team_color: None,
            vertex_color: None,
            definition_revision: 0,
            morph: None,
            flight_height: None,
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
            resource_amount: None,
            invulnerable: None,
            admitted_order: None,
            abilities: None,
            animation: None,
            look_at: None,
            summoned: false,
            cached: false,
            ai_harvest_flags: None,
            activity: None,
            shown: true,
            paused_since: None,
            sleep: None,
            vitals: None,
            combat: None,
            permanent_strength: 0,
            health_handicap: 1.0,
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
            bytes.extend([0, 0, 2]); // neutral elevation layer
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
        let transform = transform_for(&unit, &terrain, 2., 7., None).unwrap();
        assert_eq!(transform.translation, Vec3::new(32., 17., -64.));
        assert_eq!(transform.scale, Vec3::splat(2.));
        assert!((transform.rotation * Vec3::X - Vec3::NEG_Z).length() < 0.00001);
        unit.position = None;
        assert!(transform_for(&unit, &terrain, 2., 7., None).is_err());
    }
    #[test]
    fn water_mover_actor_uses_surface_and_authored_model_height() {
        let mut unit = unit();
        let mut field = unit.definition.fields[b"S001"].clone();
        let metadata = field.metadata.as_mut().unwrap();
        metadata.fourcc = *b"M001";
        metadata.kind = FieldKind::Token;
        metadata.source = FieldSource::Column {
            slk: SlkId::UnitData,
            column: "movetp".into(),
        };
        field.value = Ok(FieldValue::Cell("float".into()));
        unit.definition.fields.insert(*b"M001", field);
        let terrain = terrain();
        let surface = WaterSurface {
            mapping: pcx_tera::world::Mapping::new(128., 2048., 4.).unwrap(),
            offset: 25.,
        };
        // This fixture is dry: a floating model must refuse, rather than use the floor.
        assert!(
            surface
                .height(&unit, &terrain, "test", "synthetic")
                .is_err()
        );
        unit.definition.fields.get_mut(b"M001").unwrap().value =
            Ok(FieldValue::Cell("amph".into()));
        assert_eq!(
            surface
                .height(&unit, &terrain, "test", "synthetic")
                .unwrap(),
            10.
        );
        let transform = transform_for(&unit, &terrain, 2., 7., Some(25.)).unwrap();
        assert_eq!(transform.translation, Vec3::new(32., 32., -64.));
        unit.flight_height = Some(19.);
        let transform = transform_for(&unit, &terrain, 2., 7., Some(25.)).unwrap();
        assert_eq!(transform.translation, Vec3::new(32., 44., -64.));
        unit.flight_height = Some(f32::NAN);
        assert!(transform_for(&unit, &terrain, 2., 7., Some(25.)).is_err());
    }

    #[test]
    fn unit_visual_retains_definition_revision_without_changing_identity() {
        let mut unit = unit();
        assert_eq!(
            Visual::Unit(&unit, None, None).definition_revision(),
            Some(0)
        );
        unit.definition_revision = 7;
        assert_eq!(
            Visual::Unit(&unit, None, None).definition_revision(),
            Some(7)
        );
    }

    #[test]
    fn actor_scalar_retains_map_overrides_and_rejects_bad_metadata_or_values() {
        let mut unit = unit();
        assert_eq!(scalar(&unit.definition, "modelScale").unwrap(), 1.25);
        unit.definition.fields.get_mut(b"S001").unwrap().value =
            Ok(FieldValue::Override(pcx_objects::Value::Unreal(2.75)));
        assert_eq!(scalar(&unit.definition, "modelScale").unwrap(), 2.75);
        unit.definition
            .fields
            .get_mut(b"S001")
            .unwrap()
            .metadata
            .as_mut()
            .unwrap()
            .kind = FieldKind::Real;
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
    fn boolean_visual_fields_keep_source_cells_and_integer_wire_overrides() {
        let mut unit = unit();
        let field = unit.definition.fields.get_mut(b"S001").unwrap();
        let metadata = field.metadata.as_mut().unwrap();
        metadata.kind = FieldKind::Bool;
        metadata.declared = "bool".into();
        metadata.source = FieldSource::Column {
            slk: SlkId::UnitUi,
            column: "hideOnMinimap".into(),
        };
        for (value, expected) in [
            (FieldValue::Cell("0".into()), Some(0.)),
            (FieldValue::Cell("1".into()), Some(1.)),
            (FieldValue::Override(pcx_objects::Value::Int(1)), Some(1.)),
            (FieldValue::Override(pcx_objects::Value::Int(0)), Some(0.)),
            (FieldValue::Cell("1.0".into()), None),
            (FieldValue::Override(pcx_objects::Value::Int(2)), None),
            (FieldValue::Override(pcx_objects::Value::Real(1.)), None),
        ] {
            unit.definition.fields.get_mut(b"S001").unwrap().value = Ok(value);
            assert_eq!(scalar(&unit.definition, "hideOnMinimap").ok(), expected);
        }
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

#[cfg(all(test, not(target_arch = "wasm32"), feature = "test-fixtures"))]
#[path = "campaign_item_birth_tests.rs"]
mod item_birth_tests;

#[cfg(test)]
mod unit_sight {
    use super::shown_unit_is_published;
    use pcx_play::playercolor_host::VisionState;

    #[test]
    fn a_unit_without_a_position_is_not_an_extent_or_ledger_result() {
        let mut queried = false;
        let publish = shown_unit_is_published(true, None, true, |_| {
            queried = true;
            Ok(None)
        })
        .unwrap();
        assert!(publish);
        assert!(!queried);
    }

    #[test]
    fn fogged_masked_and_outside_points_hide_only_that_unit() {
        let position = Some([3.0, 4.0]);
        assert!(
            !shown_unit_is_published(true, position, true, |_| { Ok(Some(VisionState::Fogged)) })
                .unwrap()
        );
        assert!(
            !shown_unit_is_published(true, position, true, |_| { Ok(Some(VisionState::Masked)) })
                .unwrap()
        );
        assert!(!shown_unit_is_published(true, position, true, |_| Ok(None)).unwrap());
        assert!(
            shown_unit_is_published(true, position, true, |_| Ok(Some(VisionState::Visible)))
                .unwrap()
        );
    }

    #[test]
    fn an_unadmitted_ledger_and_a_hidden_unit_do_not_query() {
        let mut queried = false;
        assert!(
            shown_unit_is_published(true, Some([0.0, 0.0]), false, |_| {
                queried = true;
                Ok(None)
            })
            .unwrap()
        );
        assert!(
            !shown_unit_is_published(false, Some([0.0, 0.0]), true, |_| {
                queried = true;
                Ok(Some(VisionState::Visible))
            })
            .unwrap()
        );
        assert!(!queried);
    }

    #[test]
    fn a_presentation_error_is_preserved() {
        let error = shown_unit_is_published(true, Some([0.0, 0.0]), true, |_| {
            Err("viewer has no shared vision sample".into())
        })
        .unwrap_err();
        assert_eq!(error, "viewer has no shared vision sample");
    }

    #[test]
    fn a_script_hidden_unit_stays_hidden_when_fog_would_show_it() {
        let mut queried = false;
        let publish = shown_unit_is_published(false, Some([8.0, 9.0]), true, |_| {
            queried = true;
            Ok(Some(VisionState::Visible))
        })
        .unwrap();
        assert!(!publish);
        assert!(!queried);
    }

    #[test]
    fn a_ledger_error_does_not_remove_an_existing_actor_root() {
        use bevy::prelude::*;
        let mut app = App::new();
        let root = app.world_mut().spawn(Visibility::Inherited).id();
        let reason =
            super::unit_publication_sight_gate(Err("shared vision has not been sampled"), true)
                .unwrap_err();
        assert_eq!(reason, "shared vision has not been sampled");
        assert_eq!(
            *app.world().get::<Visibility>(root).unwrap(),
            Visibility::Inherited
        );
        let blocked = super::unit_publication_sight_gate(
            Err("shared vision needs admitted placement"),
            false,
        )
        .unwrap_err();
        assert_eq!(blocked, "shared vision needs admitted placement");
        assert!(app.world().get::<Visibility>(root).is_some());
        assert!(
            super::unit_publication_sight_gate(Err("shared vision is not admitted"), false).is_ok()
        );
    }
}

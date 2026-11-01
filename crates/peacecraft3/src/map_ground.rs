//! Admitted-map ground draw.
//!
//! [`install`] registers the ground material and a publisher that runs after
//! [`crate::map_requests::MapRequestSet::Deliver`]. The publisher reads the
//! admitted terrain only. It does not start a script VM, and parsed map data
//! is not a playable session.
//!
//! The source reader is [`crate::browser_configuration::SelectedMapSources`]
//! for the admitted binding receipts. Native `replace_selected` and a file drop
//! of a resolver-backed binding publish that pair. A binding that did not retain
//! its resolver, or a refused candidate, leaves the previous reader in place.
//! With no published reader the publisher records
//! [`MapGroundError::MissingSourceReader`] and does not draw.
//! The SLK read limit and retained/transient texture budgets come from
//! [`GroundSupply`]. [`install_with_budget`] supplies a fixed application policy
//! for each delivered binding. Neither entry point chooses archives or tilesets.

use std::sync::{Arc, Weak};

use bevy::prelude::*;
use bevy::render::{render_resource::WgpuLimits, renderer::RenderDevice};
use pcx_gfx::{
    GroundDraw, GroundError, GroundMaterial, GroundMaterialPlugin, GroundMemory, SelectedGroundBlp,
    spawn_admitted_ground,
};
use pcx_map::Terrain;
use pcx_play::map_admission::ParsedMapContext;
use pcx_tera::TileCatalog;

use crate::browser_configuration::SelectedMapSources;
use crate::map_binding::{AssetReceipt, BindingReceipts, receipt};
use crate::map_requests::{MapRequestSet, MapRequests};

/// Capability of this renderer: terrain is drawn without gameplay occluder fading.
/// Remove this admission when adding an active pass; script enabling must then
/// be connected to that pass rather than silently accepted here.
#[derive(Resource)]
pub struct NoOcclusionPass;

const TERRAIN_SLK: &str = "TerrainArt\\Terrain.slk";

struct AttemptKey {
    context: Weak<ParsedMapContext>,
    revision: u64,
    read_limit: Option<u64>,
    memory: Option<GroundMemory>,
}

/// Last admitted ground draw. `entity` is absent until a publication succeeds.
#[derive(Resource, Default)]
pub struct GroundPublication {
    attempted: Option<AttemptKey>,
    shadow: Option<Handle<Image>>,
    pub entity: Option<Entity>,
    /// Source context of the mounted draw; a refused replacement keeps it.
    pub context: Option<Arc<ParsedMapContext>>,
    /// Catalog and BLP winners for the mounted draw, excluding their contents.
    pub receipts: Vec<AssetReceipt>,
    pub last_error: Option<MapGroundError>,
    /// Optional source-shadow failure; the independently admitted ground stays drawable.
    pub shadow_error: Option<String>,
}

/// Caller budgets for the admitted binding revision. `Default` sets no limit.
#[derive(Resource, Default, PartialEq, Eq)]
pub struct GroundSupply {
    pub revision: u64,
    pub read_limit: Option<u64>,
    pub memory: Option<GroundMemory>,
}

/// Owned BLP bytes for one terrain slot, in slot order.
#[derive(Debug)]
pub struct SlotBlp {
    pub fourcc: [u8; 4],
    pub source: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum MapGroundError {
    #[error("admitted map has no published source reader for its binding receipts")]
    MissingSourceReader,
    #[error("binding revision {binding} has no ground read limit or texture budget")]
    MissingBudgets { binding: u64 },
    #[error("ground budgets are for binding revision {supply}; the admitted revision is {binding}")]
    InputRevision { supply: u64, binding: u64 },
    #[error("{path}: terrain catalog: {detail}")]
    Catalog { path: String, detail: String },
    #[error("terrain slot {fourcc:?} has no Terrain.slk row")]
    MissingCatalog { fourcc: [u8; 4] },
    #[error("{path}: {detail}")]
    Read { path: String, detail: String },
    #[error("ground slot list allocation failed")]
    Allocation,
    #[error(transparent)]
    Ground(#[from] GroundError),
}

/// Registers the ground material and the admitted-map publisher.
pub fn install(app: &mut App) {
    app.add_plugins(GroundMaterialPlugin)
        .insert_resource(NoOcclusionPass);
    register_publisher(app);
    crate::campaign_boundary::install(app);
    // Session-state publications that render alongside ground admission.
    crate::campaign_fog::install(app);
    crate::campaign_preload::install(app);
    crate::campaign_weather::install(app);
}

/// Install ground publication with application budgets that follow map delivery.
/// These bounds do not change the selected terrain or its authored values.
pub fn install_with_budget(app: &mut App, read_limit: u64, memory: GroundMemory) {
    install(app);
    app.add_systems(
        Update,
        (move |requests: Res<MapRequests>, mut supply: ResMut<GroundSupply>| {
            let Some(outcome) = &requests.last_outcome else {
                return;
            };
            let Some(bindings) = &requests.bindings else {
                return;
            };
            if outcome.result.is_ok() && outcome.binding_revision == Some(bindings.revision) {
                supply.set_if_neq(GroundSupply {
                    revision: bindings.revision,
                    read_limit: Some(read_limit),
                    memory: Some(memory),
                });
            }
        })
        .after(MapRequestSet::Deliver)
        .before(publish_admitted_ground),
    );
}

fn register_publisher(app: &mut App) {
    app.init_resource::<GroundPublication>()
        .init_resource::<WaterPublication>()
        .init_resource::<GroundSupply>()
        .init_resource::<Assets<StandardMaterial>>()
        .init_resource::<SelectedMapSources>()
        .add_systems(
            Update,
            (publish_admitted_ground, publish_admitted_water).after(MapRequestSet::Deliver),
        );
}

/// Last admitted water surface draw. Water quads come from the map's own
/// wet-tile flags and stored levels plus the tileset-selected `Water.slk`
/// row; the texture comes from that row's `texFile`. Dry maps publish no
/// entity rather than a stand-in plane. Shorelines are the wet-tile boundary.
/// Doodad bridge decks use the separately admitted [`BridgeDeckTable`]:
/// trajectories that cross a bridge keep the sampled ground/water surface and
/// report the deck gap through [`BridgeCrossing`] instead of clipping through
/// invented deck heights.
#[derive(Resource, Default)]
pub struct WaterPublication {
    attempted: Option<AttemptKey>,
    pub entity: Option<Entity>,
    pub context: Option<Arc<ParsedMapContext>>,
    pub receipts: Vec<AssetReceipt>,
    pub last_error: Option<MapGroundError>,
    /// Admitted doodad bridge-deck heights, keyed by doodad path. `None`
    /// keeps the prior surface-only behavior: trajectories sample ground/water
    /// and report the bridge gap separately rather than inventing a deck.
    pub decks: Option<BridgeDeckTable>,
}

/// Admitted bridge-deck height table. Entries come from a caller-supplied SLK
/// (for example the tileset-selected doodad table) through caller-supplied
/// column names; no doodad path or height is hard-coded here. Heights are
/// world units above the terrain origin, matching the ground-mesh convention.
#[derive(Clone, Debug, Default, PartialEq)]
#[allow(dead_code)]
pub struct BridgeDeckTable {
    entries: Vec<(String, f32)>,
}

impl BridgeDeckTable {
    /// Read deck heights from SLK `bytes` admitted at `path`. `path_column`
    /// names the doodad-path column and `height_column` names the deck-height
    /// column; both are resolved through the table's own row-1 headers. Rows
    /// with an empty path or a nonfinite height refuse with source context.
    /// `max_entries` bounds allocation before any insert.
    pub fn read(
        bytes: &[u8],
        path: &str,
        path_column: &str,
        height_column: &str,
        max_entries: usize,
    ) -> Result<Self, String> {
        let table = pcx_slq::Table::parse(bytes).map_err(|e| format!("{path}: deck table: {e}"))?;
        if table.column_of(path_column).is_none() {
            return Err(format!("{path}: deck table has no {path_column} column"));
        }
        if table.column_of(height_column).is_none() {
            return Err(format!("{path}: deck table has no {height_column} column"));
        }
        let mut entries = Vec::new();
        for row in table.rows().skip(1) {
            let Some(raw_path) = row.by_name(path_column) else {
                continue;
            };
            let raw_path = raw_path.trim().trim_matches('"');
            if raw_path.is_empty() {
                continue;
            }
            let Some(raw_height) = row.by_name(height_column) else {
                continue;
            };
            let raw_height = raw_height.trim().trim_matches('"');
            if raw_height.is_empty() {
                continue;
            }
            let height: f32 = raw_height
                .parse()
                .map_err(|_| format!("{path}: row {}: invalid {height_column}", row.index()))?;
            if !height.is_finite() {
                return Err(format!(
                    "{path}: row {}: nonfinite {height_column}",
                    row.index()
                ));
            }
            if entries.len() >= max_entries {
                return Err(format!("{path}: deck table exceeds {max_entries} entries"));
            }
            entries.push((raw_path.to_owned(), height));
        }
        Ok(Self { entries })
    }

    pub fn deck_height(&self, doodad_path: &str) -> Option<f32> {
        self.entries
            .iter()
            .find(|(path, _)| path.eq_ignore_ascii_case(doodad_path))
            .map(|(_, height)| *height)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// One movement-segment sample where a bridge deck may cover the surface.
/// `deck` is the admitted deck height when a bridge footprint covers the
/// point; `surface` is the sampled ground/water height. Callers keep the
/// surface for motion and record the gap separately.
#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(dead_code)]
pub struct BridgeCrossing {
    pub deck: Option<f32>,
    pub surface: f32,
}

impl BridgeCrossing {
    /// Admitted deck height wins for rendering; the gap stays observable so a
    /// trajectory never silently clips through an invented height.
    pub fn rendered_height(self) -> f32 {
        self.deck.unwrap_or(self.surface)
    }

    pub fn deck_gap(self) -> Option<f32> {
        self.deck.map(|deck| deck - self.surface)
    }
}

fn require_reader(
    sources: &SelectedMapSources,
    receipts: &Arc<BindingReceipts>,
) -> Result<Arc<pcx_assets::Resolver>, MapGroundError> {
    sources
        .for_receipts(receipts)
        .ok_or(MapGroundError::MissingSourceReader)
}

fn require_budget(
    supply: &GroundSupply,
    binding: u64,
) -> Result<(u64, GroundMemory), MapGroundError> {
    if supply.revision != binding {
        return Err(MapGroundError::InputRevision {
            supply: supply.revision,
            binding,
        });
    }
    match (supply.read_limit, supply.memory) {
        (Some(read_limit), Some(memory)) => Ok((read_limit, memory)),
        _ => Err(MapGroundError::MissingBudgets { binding }),
    }
}

type GroundAssets<'w> = (
    ResMut<'w, Assets<Mesh>>,
    ResMut<'w, Assets<Image>>,
    ResMut<'w, Assets<GroundMaterial>>,
);

pub(crate) fn publish_admitted_ground(
    mut commands: Commands,
    requests: Res<MapRequests>,
    sources: Res<SelectedMapSources>,
    supply: Res<GroundSupply>,
    mut publication: ResMut<GroundPublication>,
    device: Option<Res<RenderDevice>>,
    mut assets: GroundAssets<'_>,
) {
    let Some(outcome) = requests.last_outcome.as_ref() else {
        return;
    };
    if outcome.result.is_err() {
        return;
    }
    let Some(bindings) = requests.bindings.as_ref() else {
        return;
    };
    let Some(context) = requests.admission.shared_context() else {
        return;
    };
    if outcome.binding_revision != Some(bindings.revision)
        || outcome.identity.as_ref() != Some(&context.sources.map)
        || context.sources.map.edition != bindings.map_identity.edition
        || context.sources.map.version != bindings.map_identity.version
        || !requests
            .published_receipts
            .as_ref()
            .is_some_and(|receipts| Arc::ptr_eq(receipts, &bindings.receipts))
    {
        return;
    }
    let key = AttemptKey {
        context: Arc::downgrade(&context),
        revision: bindings.revision,
        read_limit: supply.read_limit,
        memory: supply.memory,
    };
    if publication.attempted.as_ref().is_some_and(|previous| {
        previous.revision == key.revision
            && Weak::ptr_eq(&previous.context, &key.context)
            && previous.read_limit == key.read_limit
            && previous.memory == key.memory
    }) {
        return;
    }
    let (read_limit, memory) = match require_budget(&supply, bindings.revision) {
        Ok(budget) => budget,
        Err(error) => {
            publication.attempted = None;
            publication.last_error = Some(error);
            return;
        }
    };
    let reader = match require_reader(&sources, &bindings.receipts) {
        Ok(reader) => reader,
        Err(error) => {
            publication.last_error = Some(error);
            return;
        }
    };
    let texture_limits = device.as_ref().map(|device| device.limits());
    let shadow = prepare_shadow(
        &context.terrain,
        reader.as_ref(),
        &bindings.receipts,
        read_limit,
        texture_limits.as_ref(),
    );
    let result = prepare_draw(
        &context.terrain,
        reader.as_ref(),
        read_limit,
        memory,
        texture_limits.as_ref(),
        (&mut assets.0, &mut assets.1, &mut assets.2),
    );
    publication.attempted = Some(key);
    match result {
        Ok((draw, receipts)) => {
            let entity = entity_from_draw(&mut commands, &draw);
            replace_entity(&mut commands, &mut publication, entity);
            publication.context = Some(context);
            publication.receipts = receipts;
            publication.last_error = None;
            if let Some(old) = publication.shadow.take() {
                assets.1.remove(old.id());
            }
            match shadow {
                Ok((image, bounds, color, source_receipt)) => {
                    if let Some(mut material) =
                        draw.material.as_ref().and_then(|h| assets.2.get_mut(h))
                    {
                        let image = assets.1.add(image);
                        material
                            .set_shadow(image.clone(), bounds, color)
                            .expect("preflighted shadow bounds");
                        publication.shadow = Some(image);
                        publication.receipts.push(source_receipt);
                    }
                    publication.shadow_error = None;
                }
                Err(error) => publication.shadow_error = Some(error.to_string()),
            }
        }
        Err(error) => {
            publication.last_error = Some(error);
        }
    }
}

// Nine Bevy system params (commands, requests, sources, supply, publication,
// device limits, mesh/image/material assets) exceed the default arity lint;
// grouping them would churn every system call site for no behavior change.
#[allow(clippy::too_many_arguments)]
fn publish_admitted_water(
    mut commands: Commands,
    requests: Res<MapRequests>,
    sources: Res<SelectedMapSources>,
    supply: Res<GroundSupply>,
    mut publication: ResMut<WaterPublication>,
    device: Option<Res<RenderDevice>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(outcome) = requests.last_outcome.as_ref() else {
        return;
    };
    if outcome.result.is_err() {
        return;
    }
    let Some(bindings) = requests.bindings.as_ref() else {
        return;
    };
    let Some(context) = requests.admission.shared_context() else {
        return;
    };
    if outcome.binding_revision != Some(bindings.revision)
        || outcome.identity.as_ref() != Some(&context.sources.map)
        || context.sources.map.edition != bindings.map_identity.edition
        || context.sources.map.version != bindings.map_identity.version
        || !requests
            .published_receipts
            .as_ref()
            .is_some_and(|receipts| Arc::ptr_eq(receipts, &bindings.receipts))
    {
        return;
    }
    let key = AttemptKey {
        context: Arc::downgrade(&context),
        revision: bindings.revision,
        read_limit: supply.read_limit,
        memory: supply.memory,
    };
    if publication.attempted.as_ref().is_some_and(|previous| {
        previous.revision == key.revision
            && Weak::ptr_eq(&previous.context, &key.context)
            && previous.read_limit == key.read_limit
            && previous.memory == key.memory
    }) {
        return;
    }
    let (read_limit, _) = match require_budget(&supply, bindings.revision) {
        Ok(budget) => budget,
        Err(error) => {
            publication.attempted = None;
            publication.last_error = Some(error);
            return;
        }
    };
    let reader = match require_reader(&sources, &bindings.receipts) {
        Ok(reader) => reader,
        Err(error) => {
            publication.last_error = Some(error);
            return;
        }
    };
    let texture_limits = device.as_ref().map(|device| device.limits());
    publication.attempted = Some(key);
    let draw = prepare_water(
        &context.terrain,
        reader.as_ref(),
        read_limit,
        texture_limits.as_ref(),
        &mut meshes,
        &mut images,
        &mut materials,
    );
    match draw {
        Ok(None) => {
            if let Some(previous) = publication.entity.take() {
                commands.entity(previous).despawn();
            }
            publication.context = Some(context);
            publication.receipts.clear();
            publication.last_error = None;
        }
        Ok(Some((mesh, material, receipts))) => {
            if let Some(previous) = publication.entity.take() {
                commands.entity(previous).despawn();
            }
            let entity = commands
                .spawn((
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    bevy::camera::visibility::RenderLayers::layer(
                        crate::campaign_camera::RENDER_LAYER,
                    ),
                ))
                .id();
            publication.entity = Some(entity);
            publication.context = Some(context);
            publication.receipts = receipts;
            publication.last_error = None;
        }
        Err(error) => {
            publication.last_error = Some(error);
        }
    }
}

/// Build the admitted water surface, if the map has any wet tile. Returns
/// `Ok(None)` for a dry map. The texture path comes from the selected
/// `Water.slk` row's `texFile`; its alpha channel is the blend, matching the
/// ground-sheet convention of authored alpha rather than numeric weights.
#[allow(clippy::too_many_arguments)]
/// Prepared water mesh, material and per-asset receipts for publication.
/// Factored out so the fallible preparation reads separately from the
/// infallible ECS publication below.
type PreparedWater = Option<(Handle<Mesh>, Handle<StandardMaterial>, Vec<AssetReceipt>)>;

fn prepare_water(
    terrain: &Terrain,
    reader: &pcx_assets::Resolver,
    read_limit: u64,
    device: Option<&WgpuLimits>,
    meshes: &mut Assets<Mesh>,
    images: &mut Assets<Image>,
    materials: &mut Assets<StandardMaterial>,
) -> Result<PreparedWater, MapGroundError> {
    use bevy::{
        asset::RenderAssetUsages,
        mesh::{Indices, PrimitiveTopology},
        render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    };
    use pcx_tera::{WaterBudget, WaterData, WaterProfile, WaterSelection};
    let read_error = |path: &str, detail: String| MapGroundError::Read {
        path: path.into(),
        detail,
    };
    let table_asset = reader
        .resolve_bounded("TerrainArt\\Water.slk", read_limit)
        .map_err(|e| read_error("TerrainArt\\Water.slk", e.to_string()))?;
    let table = pcx_slq::Table::parse(&table_asset.bytes)
        .map_err(|e| read_error("TerrainArt\\Water.slk", e.to_string()))?;
    let tile = pcx_tera::world::TILE_XY;
    let profile = WaterProfile::open_scaled(
        &table,
        "TerrainArt\\Water.slk",
        WaterSelection::Tileset,
        terrain,
        tile,
    )
    .map_err(|e| read_error("TerrainArt\\Water.slk", e.to_string()))?;
    let corners = terrain.corner_count();
    let tiles = usize::try_from(terrain.width())
        .ok()
        .and_then(|w| usize::try_from(terrain.height()).ok()?.checked_mul(w))
        .ok_or(MapGroundError::Allocation)?;
    let budget = WaterBudget {
        max_vertices: u64::try_from(corners).map_err(|_| MapGroundError::Allocation)? * 4,
        max_indices: u64::try_from(tiles).map_err(|_| MapGroundError::Allocation)? * 6,
    };
    let data = WaterData::new(terrain, "selected map", "war3map.w3e")
        .map_err(|e| read_error("war3map.w3e", e.to_string()))?;
    let mesh = data
        .mesh(&profile, &budget)
        .map_err(|e| read_error("war3map.w3e", e.to_string()))?;
    if mesh.wet_tiles == 0 {
        return Ok(None);
    }
    let tex_file = profile.field("texFile").ok_or_else(|| {
        read_error(
            "TerrainArt\\Water.slk",
            "selected row has no texFile".into(),
        )
    })?;
    let tex_file = tex_file.trim().trim_matches('"');
    if tex_file.is_empty() {
        return Err(read_error(
            "TerrainArt\\Water.slk",
            "selected row texFile is empty".into(),
        ));
    }
    let texture_asset = reader
        .resolve_bounded(tex_file, read_limit)
        .map_err(|e| read_error(tex_file, e.to_string()))?;
    let loaded = pcx_gfx::load_image_bounded(&texture_asset.bytes, tex_file, 8 << 20)
        .map_err(|e| read_error(tex_file, e.to_string()))?;
    if loaded.width == 0 || loaded.height == 0 {
        return Err(read_error(tex_file, "water texture is empty".into()));
    }
    if let Some(limits) = device
        && (loaded.width > limits.max_texture_dimension_2d
            || loaded.height > limits.max_texture_dimension_2d)
    {
        return Err(read_error(
            tex_file,
            "water texture exceeds device limits".into(),
        ));
    }
    let mut water_image = Image::new(
        Extent3d {
            width: loaded.width,
            height: loaded.height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        loaded.rgba,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    water_image.sampler = bevy::image::ImageSampler::linear();
    let texture = images.add(water_image);
    let material = materials.add(StandardMaterial {
        base_color_texture: Some(texture),
        base_color: Color::WHITE,
        alpha_mode: AlphaMode::Blend,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    let mut render_mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    // Terrain `(x east, y north, z up)` becomes Bevy `(x, z, -y)`, matching the
    // ground mesh convention so floating/amphibious actors sample the same
    // surface the renderer draws. Bridge decks use the admitted
    // `BridgeDeckTable`; see `WaterPublication`.
    let positions: Vec<[f32; 3]> = mesh
        .positions
        .iter()
        .map(|[x, y, z]| [*x, *z, -*y])
        .collect();
    render_mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    render_mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, mesh.uvs.clone());
    render_mesh.insert_indices(Indices::U32(mesh.indices.clone()));
    let handle = meshes.add(render_mesh);
    let mut receipts = Vec::new();
    receipts
        .try_reserve_exact(2)
        .map_err(|_| MapGroundError::Allocation)?;
    receipts.push(receipt(&table_asset));
    receipts.push(receipt(&texture_asset));
    Ok(Some((handle, material, receipts)))
}

fn prepare_shadow(
    terrain: &Terrain,
    reader: &pcx_assets::Resolver,
    receipts: &BindingReceipts,
    limit: u64,
    device: Option<&WgpuLimits>,
) -> Result<(Image, Vec4, [u8; 4], AssetReceipt), MapGroundError> {
    let read_error = |path: &str, detail: String| MapGroundError::Read {
        path: path.into(),
        detail,
    };
    let source = reader
        .resolve_bounded(receipts.map.path.as_str(), limit)
        .map_err(|e| read_error("war3map.shd", e.to_string()))?;
    if receipt(&source) != receipts.map {
        return Err(read_error(
            "war3map.shd",
            "map differs from admitted receipt".into(),
        ));
    }
    let map =
        pcx_map::Map::parse(&source.bytes).map_err(|e| read_error("war3map.shd", e.to_string()))?;
    let budget = usize::try_from(limit)
        .map_err(|_| read_error("war3map.shd", "read budget overflow".into()))?;
    let raw = map
        .extract_checked(
            receipts.map.path.as_str(),
            "war3map.shd",
            pcx_mpq::checked::ReadBudget {
                max_archive_bytes: budget,
                max_archive_table_bytes: budget,
                max_raw_packed_bytes: budget,
                max_decoded_bytes: budget,
            },
        )
        .map_err(|e| read_error("war3map.shd", e.to_string()))?
        .ok_or_else(|| read_error("war3map.shd", "missing authored shadow mask".into()))?;
    let mask = pcx_map::Shadows::from_bytes(raw, terrain.width(), terrain.height())
        .map_err(|e| read_error("war3map.shd", e.to_string()))?;
    let max_dimension = device.map_or(u32::MAX, |d| d.max_texture_dimension_2d);
    let image = shadow_image(&mask, max_dimension).map_err(|e| read_error("war3map.shd", e))?;
    let profile = reader
        .resolve_bounded("UI\\MiscData.txt", limit)
        .map_err(|e| read_error("UI\\MiscData.txt", e.to_string()))?;
    let color = pcx_objects::editor::fog_boundary::ShadowColor::read(&profile.bytes, budget)
        .map_err(|e| read_error("UI\\MiscData.txt", e))?
        .0;
    let (x, y) = terrain.origin();
    let [end_x, end_y, _] = pcx_tera::corner_world(terrain, terrain.width(), terrain.height())
        .ok_or_else(|| read_error("war3map.shd", "missing terrain extent".into()))?;
    let bounds = Vec4::new(x, y, end_x - x, end_y - y);
    if !bounds.is_finite() || bounds.z <= 0.0 || bounds.w <= 0.0 {
        return Err(read_error("war3map.shd", "invalid terrain extent".into()));
    }
    Ok((image, bounds, color, receipt(&profile)))
}

fn shadow_image(mask: &pcx_map::Shadows, max_dimension: u32) -> Result<Image, String> {
    use bevy::{
        asset::RenderAssetUsages,
        render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    };
    let width = u32::try_from(mask.columns()).map_err(|_| "shadow width overflow")?;
    let height = u32::try_from(mask.rows()).map_err(|_| "shadow height overflow")?;
    if width == 0 || height == 0 || width > max_dimension || height > max_dimension {
        return Err("shadow grid exceeds active texture limits or is empty".into());
    }
    // SHD rows increase in world Y; the shader samples in the same coordinate system.
    let mut image = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        mask.raw().to_vec(),
        TextureFormat::R8Unorm,
        RenderAssetUsages::default(),
    );
    image.sampler = bevy::image::ImageSampler::linear();
    Ok(image)
}

fn prepare_draw(
    terrain: &Terrain,
    reader: &pcx_assets::Resolver,
    read_limit: u64,
    memory: GroundMemory,
    texture_limits: Option<&WgpuLimits>,
    assets: (
        &mut Assets<Mesh>,
        &mut Assets<Image>,
        &mut Assets<GroundMaterial>,
    ),
) -> Result<(GroundDraw, Vec<AssetReceipt>), MapGroundError> {
    let (meshes, images, materials) = assets;
    let slk = reader
        .resolve_bounded(TERRAIN_SLK, read_limit)
        .map_err(|error| MapGroundError::Read {
            path: TERRAIN_SLK.to_owned(),
            detail: error.to_string(),
        })?;
    let catalog =
        TileCatalog::from_terrain_slk(&slk.bytes).map_err(|error| MapGroundError::Catalog {
            path: TERRAIN_SLK.to_owned(),
            detail: error.to_string(),
        })?;
    let mut receipts = Vec::new();
    receipts
        .try_reserve_exact(terrain.slots().len() + 1)
        .map_err(|_| MapGroundError::Allocation)?;
    receipts.push(receipt(&slk));
    let slots = load_slot_blps(terrain, &catalog, |path| {
        let asset = reader
            .resolve_bounded(path, read_limit)
            .map_err(|error| error.to_string())?;
        receipts.push(receipt(&asset));
        Ok(asset.bytes)
    })?;
    let draw = spawn_admitted(
        terrain,
        &slots,
        memory,
        texture_limits,
        meshes,
        images,
        materials,
    )?;
    Ok((draw, receipts))
}

fn entity_from_draw(commands: &mut Commands, draw: &GroundDraw) -> Option<Entity> {
    let mesh = draw.mesh.clone()?;
    let material = draw.material.clone()?;
    Some(
        commands
            .spawn((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                bevy::camera::visibility::RenderLayers::layer(crate::campaign_camera::RENDER_LAYER),
            ))
            .id(),
    )
}

fn replace_entity(
    commands: &mut Commands,
    publication: &mut GroundPublication,
    next: Option<Entity>,
) {
    if let Some(previous) = publication.entity.take() {
        commands.entity(previous).despawn();
    }
    publication.entity = next;
}

/// Resolve each terrain slot through the catalog and the caller's reader.
///
/// A missing catalog row is an error. Slot 0 is not substituted.
pub fn load_slot_blps(
    terrain: &Terrain,
    catalog: &TileCatalog,
    mut read: impl FnMut(&str) -> Result<Vec<u8>, String>,
) -> Result<Vec<SlotBlp>, MapGroundError> {
    if terrain.slots().len() > pcx_gfx::ground::MAX_GROUND_SLOTS {
        return Err(GroundError::TooManySlots {
            count: terrain.slots().len(),
        }
        .into());
    }
    let mut slots = Vec::new();
    slots
        .try_reserve_exact(terrain.slots().len())
        .map_err(|_| MapGroundError::Allocation)?;
    for fourcc in terrain.slots() {
        let source = catalog
            .blp_path(*fourcc)
            .ok_or(MapGroundError::MissingCatalog { fourcc: *fourcc })?;
        let bytes = read(&source).map_err(|detail| MapGroundError::Read {
            path: source.clone(),
            detail,
        })?;
        slots.push(SlotBlp {
            fourcc: *fourcc,
            source,
            bytes,
        });
    }
    Ok(slots)
}

/// Decode and publish the ground draw. `memory` is the caller's retained and
/// transient budget, separate from menu image budgets.
pub fn spawn_admitted(
    terrain: &Terrain,
    slots: &[SlotBlp],
    memory: GroundMemory,
    texture_limits: Option<&WgpuLimits>,
    meshes: &mut Assets<Mesh>,
    images: &mut Assets<Image>,
    materials: &mut Assets<GroundMaterial>,
) -> Result<GroundDraw, MapGroundError> {
    let mut borrowed = Vec::new();
    borrowed
        .try_reserve_exact(slots.len())
        .map_err(|_| MapGroundError::Allocation)?;
    for slot in slots {
        borrowed.push(SelectedGroundBlp {
            fourcc: slot.fourcc,
            source: &slot.source,
            bytes: &slot.bytes,
        });
    }
    Ok(spawn_admitted_ground(
        terrain,
        &borrowed,
        memory,
        texture_limits,
        meshes,
        images,
        materials,
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shadow_grid_keeps_source_orientation_values_and_device_limits() {
        let mut cells = vec![0; 32];
        cells[0] = 37;
        cells[31] = 191;
        let mask = pcx_map::Shadows::from_bytes(cells.clone(), 2, 1).unwrap();
        let image = shadow_image(&mask, 8).unwrap();
        assert_eq!(image.width(), 8);
        assert_eq!(image.height(), 4);
        assert_eq!(image.data.unwrap(), cells);
        assert!(shadow_image(&mask, 7).is_err());
        assert!(shadow_image(&pcx_map::Shadows::from_bytes(vec![], 0, 1).unwrap(), 8).is_err());
    }

    fn catalog() -> TileCatalog {
        let text = "ID;PWXL;N;E\r\nC;Y1;X1;K\"tileID\"\r\nC;X2;K\"dir\"\r\nC;X3;K\"file\"\r\nC;Y2;X1;K\"Ldrt\"\r\nC;X2;K\"TerrainArt\\LordaeronSummer\"\r\nC;X3;K\"LordaeronSummer\"\r\nE\r\n";
        TileCatalog::from_terrain_slk(text.as_bytes()).unwrap()
    }

    fn terrain(nibble: u8) -> Terrain {
        let mut bytes = pcx_map::TERRAIN_MAGIC.to_vec();
        bytes.extend(pcx_map::TERRAIN_VERSION.to_le_bytes());
        bytes.push(0);
        bytes.extend(0u32.to_le_bytes());
        bytes.extend(1u32.to_le_bytes());
        bytes.extend_from_slice(b"Ldrt");
        bytes.extend(0u32.to_le_bytes());
        bytes.extend(2u32.to_le_bytes());
        bytes.extend(2u32.to_le_bytes());
        bytes.extend(0.0f32.to_le_bytes());
        bytes.extend(0.0f32.to_le_bytes());
        for _ in 0..4 {
            bytes.extend(8192u16.to_le_bytes());
            bytes.extend(0u16.to_le_bytes());
            bytes.push(nibble);
            bytes.push(0);
            bytes.push(0);
        }
        Terrain::from_bytes(bytes).unwrap()
    }

    fn sheet() -> Vec<u8> {
        let payload_at = 0x9Cu32 + 256 * 4;
        let mut bytes = vec![0u8; payload_at as usize + 16];
        bytes[0..4].copy_from_slice(b"BLP1");
        bytes[4..8].copy_from_slice(&1u32.to_le_bytes());
        bytes[0x0C..0x10].copy_from_slice(&4u32.to_le_bytes());
        bytes[0x10..0x14].copy_from_slice(&4u32.to_le_bytes());
        bytes[0x1C..0x20].copy_from_slice(&payload_at.to_le_bytes());
        bytes[0x5C..0x60].copy_from_slice(&16u32.to_le_bytes());
        for index in 0..256 {
            let at = 0x9C + index * 4;
            let entry: [u8; 4] = if index == 0 {
                [0, 255, 0, 255]
            } else {
                [0, 0, 0, 255]
            };
            bytes[at..at + 4].copy_from_slice(&entry);
        }
        bytes
    }

    fn sample_receipts() -> Arc<BindingReceipts> {
        use crate::map_binding::AssetReceipt;
        use pcx_assets::{AssetPath, LayerKind, Provenance, SourceInfo};
        let source = SourceInfo {
            id: "fixture".into(),
            kind: LayerKind::Mpq,
            origin: "fixture-origin".into(),
            version: Some("fixture".into()),
        };
        let receipt = AssetReceipt {
            path: AssetPath::new("fixture.blp").unwrap(),
            provenance: Provenance {
                source,
                archive_priority: Some(1),
                locator: "fixture.blp".into(),
                byte_length: 1,
            },
            hash: 1,
        };
        Arc::new(BindingReceipts {
            map: receipt.clone(),
            common: receipt.clone(),
            blizzard: receipt,
            catalog: Vec::new(),
            missing_catalog: Vec::new(),
            units: None,
            items: None,
        })
    }

    #[test]
    fn the_hook_resolves_catalog_bytes_and_spawns_the_ground_draw() {
        let terrain = terrain(0);
        let slots = load_slot_blps(&terrain, &catalog(), |path| {
            assert_eq!(path, "TerrainArt\\LordaeronSummer\\LordaeronSummer.blp");
            Ok(sheet())
        })
        .unwrap();
        let mut meshes = Assets::default();
        let mut images = Assets::default();
        let mut materials = Assets::default();
        let draw = spawn_admitted(
            &terrain,
            &slots,
            GroundMemory {
                retained_bytes: 64,
                transient_bytes: 256,
            },
            None,
            &mut meshes,
            &mut images,
            &mut materials,
        )
        .unwrap();
        let mut world = World::new();
        let entity = entity_from_draw(&mut world.commands(), &draw).unwrap();
        world.flush();
        assert!(world.get::<Mesh3d>(entity).is_some());
        assert_eq!(
            world.get::<bevy::camera::visibility::RenderLayers>(entity),
            Some(&bevy::camera::visibility::RenderLayers::layer(
                crate::campaign_camera::RENDER_LAYER
            ))
        );
        assert!(
            world
                .get::<MeshMaterial3d<GroundMaterial>>(entity)
                .is_some()
        );
    }

    #[test]
    fn a_missing_catalog_row_or_failed_budget_publishes_nothing() {
        let missing = {
            let mut bytes = pcx_map::TERRAIN_MAGIC.to_vec();
            bytes.extend(pcx_map::TERRAIN_VERSION.to_le_bytes());
            bytes.push(0);
            bytes.extend(0u32.to_le_bytes());
            bytes.extend(1u32.to_le_bytes());
            bytes.extend_from_slice(b"ZZZZ");
            bytes.extend(0u32.to_le_bytes());
            bytes.extend(2u32.to_le_bytes());
            bytes.extend(2u32.to_le_bytes());
            bytes.extend(0.0f32.to_le_bytes());
            bytes.extend(0.0f32.to_le_bytes());
            for _ in 0..4 {
                bytes.extend(8192u16.to_le_bytes());
                bytes.extend(0u16.to_le_bytes());
                bytes.extend_from_slice(&[0, 0, 0]);
            }
            Terrain::from_bytes(bytes).unwrap()
        };
        let error = load_slot_blps(&missing, &catalog(), |_| Ok(sheet())).unwrap_err();
        assert_eq!(error, MapGroundError::MissingCatalog { fourcc: *b"ZZZZ" });

        let terrain = terrain(0);
        let slots = load_slot_blps(&terrain, &catalog(), |_| Ok(sheet())).unwrap();
        let mut meshes = Assets::default();
        let mut images = Assets::default();
        let mut materials = Assets::default();
        let error = spawn_admitted(
            &terrain,
            &slots,
            GroundMemory {
                retained_bytes: 63,
                transient_bytes: 256,
            },
            None,
            &mut meshes,
            &mut images,
            &mut materials,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            MapGroundError::Ground(GroundError::RetainedBytes {
                needed: 64,
                limit: 63,
                ..
            })
        ));
        assert!(images.iter().next().is_none());
        assert!(meshes.iter().next().is_none());
        assert!(materials.iter().next().is_none());
        assert!(
            entity_from_draw(
                &mut World::new().commands(),
                &GroundDraw {
                    mesh: None,
                    material: None
                }
            )
            .is_none()
        );
    }

    #[test]
    fn budgets_and_a_missing_reader_are_refused() {
        let error = require_budget(&GroundSupply::default(), 7).unwrap_err();
        assert_eq!(
            error,
            MapGroundError::InputRevision {
                supply: 0,
                binding: 7
            }
        );
        let partial = GroundSupply {
            revision: 7,
            read_limit: Some(32),
            memory: None,
        };
        assert_eq!(
            require_budget(&partial, 7).unwrap_err(),
            MapGroundError::MissingBudgets { binding: 7 }
        );
        let sources = SelectedMapSources::default();
        assert!(matches!(
            require_reader(&sources, &sample_receipts()),
            Err(MapGroundError::MissingSourceReader)
        ));
    }

    #[test]
    fn the_publisher_draws_nothing_without_an_admitted_map() {
        let mut app = App::new();
        crate::map_requests::install(&mut app);
        register_publisher(&mut app);
        app.init_resource::<Assets<Mesh>>();
        app.init_resource::<Assets<Image>>();
        app.init_resource::<Assets<GroundMaterial>>();
        app.update();
        let publication = app.world().resource::<GroundPublication>();
        assert!(publication.entity.is_none());
        assert!(publication.last_error.is_none());
    }
}

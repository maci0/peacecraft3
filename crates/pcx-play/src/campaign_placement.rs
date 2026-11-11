//! Selected-source creation collision cache on the existing executed identities.
use crate::{
    footprint_geometry::{clamp_cell, rotate_offset, stationary_anchor},
    map_admission::ParsedMapContext,
    playercolor_host::{
        DestructableCatalog, DestructableCreation, DestructableOrigin, DestructableState,
        DestructableStats, ObjectHost, PlacedDestructable, PlacedSource,
    },
    spatial_pool::{Geometry, PoolOwner, SpatialRules, Texture},
    unit_placement::{
        Budget, Footprint, FootprintLoader, Grid, NativeTexture, Placement, Rules, StationaryStamp,
        TexturePolicy,
    },
};
use pcx_assets::{AssetPath, Resolver};
use pcx_objects::editor::{
    FieldKind, FieldSource, Overlay, SlkId,
    defaults::{DefaultTables, Family, Unresolved},
};
use std::{collections::BTreeMap, sync::Arc};

#[path = "placed_doodads.rs"]
mod decorative;
pub(crate) use decorative::model_member;
pub use decorative::{DoodadAppearance, PlacedDoodad, RectangleSelection};

const CELLS: usize = 4 << 20;
const FOOTPRINT_CELLS: usize = 1 << 16;
/// Existing source-count operating limit, reused as the non-refundable
/// uncached resolve/decode attempt limit. Each attempt still obeys the 1 MiB
/// resolve cap and the footprint pixel cap, bounding total spent work.
const FOOTPRINT_SOURCES: usize = 256;
const STATIONARY_WORK: usize = 4 << 20;
const STATIONARY_CELLS: usize = 1 << 20;

/// Selected map members needed by placed objects: the `.doo` instances and the
/// object catalogs they resolve against.
#[derive(Clone)]
pub struct Catalog {
    objects: DestructableCatalog,
    doodads: Vec<pcx_map::Doodad>,
    terrain_doodads: Vec<pcx_map::TerrainDoodad>,
    destructables: Overlay,
    decorations: Overlay,
}

/// The path-resolution subset of the selected catalog, retained by the collision
/// cache for script-created destructables. It excludes the authored `.doo` rows.
#[derive(Clone)]
struct ScriptPaths {
    tables: Arc<DefaultTables>,
    destructables: Overlay,
    decorations: Overlay,
}

impl ScriptPaths {
    fn resolve(&self, objectid: i32, dead: bool) -> Result<String, String> {
        path(
            &self.tables,
            &self.destructables,
            &self.decorations,
            objectid.to_be_bytes(),
            dead,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum PlacedRole {
    Automatic(DestructableStats),
    ScriptOnly,
    Decoration,
}

/// Decodes `war3map.doo`, refusing a declared instance count over the stationary
/// budget before any record is decoded.
fn parse_doodads(doodad_bytes: Vec<u8>) -> Result<pcx_map::Doodads, String> {
    let count = doodad_bytes
        .get(12..16)
        .ok_or("truncated placement doodad count")?;
    if u32::from_le_bytes(count.try_into().unwrap()) > 100000 {
        return Err("placement stationary count budget".into());
    }
    pcx_map::Doodads::from_bytes(doodad_bytes).map_err(|e| e.to_string())
}

/// The map's destructable and doodad overlays. `war3map.w3b` carries destructables (base ids such
/// as `LTe3`, `BTtw`); `war3map.w3h` is the buff family (`BHwe`) and never supplies destructable
/// stats.
fn stationary_overlays(
    extract: impl Fn(&'static str) -> Result<Option<Vec<u8>>, String>,
) -> Result<(Overlay, Overlay), String> {
    let overlay = |member: &'static str| -> Result<Overlay, String> {
        extract(member)?
            .map(|raw| {
                pcx_objects::ObjectFile::parse(member, &raw)
                    .map(|f| Overlay::of(&f))
                    .map_err(|e| e.to_string())
            })
            .transpose()
            .map(Option::unwrap_or_default)
    };
    Ok((overlay("war3map.w3b")?, overlay("war3map.w3d")?))
}

impl Catalog {
    pub fn read(
        context: &ParsedMapContext,
        map_bytes: &[u8],
        tables: Arc<DefaultTables>,
    ) -> Result<Self, String> {
        let map = pcx_map::Map::parse(map_bytes).map_err(|e| e.to_string())?;
        let extract = |member: &'static str| {
            map.extract_checked(
                &context.map_digest.label,
                member,
                pcx_mpq::checked::ReadBudget {
                    max_archive_bytes: 64 << 20,
                    max_archive_table_bytes: 8 << 20,
                    max_raw_packed_bytes: 16 << 20,
                    max_decoded_bytes: 16 << 20,
                },
            )
            .map_err(|e| e.to_string())
        };
        let doodads = parse_doodads(extract("war3map.doo")?.ok_or("missing placement doodads")?)?;
        let (destructables, decorations) = stationary_overlays(extract)?;
        Ok(Self {
            objects: DestructableCatalog::new(tables, destructables.clone(), decorations.clone()),
            doodads: doodads.instances().to_vec(),
            terrain_doodads: doodads.terrain_doodads().to_vec(),
            destructables,
            decorations,
        })
    }

    pub fn objects(&self) -> &DestructableCatalog {
        &self.objects
    }

    pub fn doodads(&self) -> &[pcx_map::Doodad] {
        &self.doodads
    }

    /// Terrain (cliff) doodads from `war3map.doo`. They are not placed: the file stores
    /// only a type, a variation and a grid cell for each, with no height or orientation,
    /// and this crate has no derivation for either.
    pub fn terrain_doodads(&self) -> &[pcx_map::TerrainDoodad] {
        &self.terrain_doodads
    }

    fn role(&self, instance: &pcx_map::Doodad) -> Result<PlacedRole, String> {
        let objectid = i32::from_be_bytes(instance.type_id);
        let Some(stats) = self.objects.stats_if_destructable(objectid)? else {
            return Ok(PlacedRole::Decoration);
        };
        match instance.flags {
            0 | 3 | 4 | 7 => Ok(PlacedRole::ScriptOnly),
            2 | 6 => Ok(PlacedRole::Automatic(stats)),
            flags => {
                let name = std::str::from_utf8(&instance.type_id)
                    .map_err(|_| "non-UTF8 placed destructable identity")?;
                Err(format!(
                    "{name} creation {}: unqualified placed destructable flags {flags:#04x}; measured 0x00, 0x02, 0x03, 0x04, 0x06, 0x07",
                    instance.creation_number
                ))
            }
        }
    }

    /// Source life byte is the percentage of authored maximum HP, capped at
    /// 100. Four initially dead records carry byte zero. Only the measured
    /// automatic flags enter the arena; other measured flags leave creation to
    /// the map script, and unmeasured flags refuse.
    pub fn placed(&self) -> Result<Vec<PlacedDestructable>, String> {
        let mut rows = Vec::new();
        rows.try_reserve_exact(self.doodads.len())
            .map_err(|_| "placed destructable allocation")?;
        for instance in &self.doodads {
            let PlacedRole::Automatic(stats) = self.role(instance)? else {
                continue;
            };
            if instance.position.iter().any(|v| !v.is_finite())
                || !instance.angle.is_finite()
                || instance.scale.iter().any(|v| !v.is_finite())
            {
                return Err("nonfinite placed destructable pose".into());
            }
            let objectid = i32::from_be_bytes(instance.type_id);
            let variation = i32::try_from(instance.variation)
                .map_err(|_| "placed destructable variation range")?;
            let percent = f32::from(instance.life.min(100)) / 100.0;
            let dead = instance.life == 0;
            let life = if dead { 0.0 } else { stats.max_life * percent };
            rows.push(PlacedDestructable {
                creation: DestructableCreation {
                    origin: DestructableOrigin::Placed {
                        creation_number: instance.creation_number,
                    },
                    objectid,
                    x: instance.position[0],
                    y: instance.position[1],
                    z: Some(instance.position[2]),
                    face: instance.angle.to_degrees(),
                    scale: instance.scale[0],
                    variation,
                    dead,
                },
                state: DestructableState {
                    dead,
                    life: Some(life),
                    max_life: Some(stats.max_life),
                    ..DestructableState::default()
                },
                source: placed_source(instance),
            });
        }
        Ok(rows)
    }

    /// Authored destructable rows whose measured flags leave creation to the
    /// map script. They are retained as source records, not arena objects or
    /// initial collision layers.
    pub fn scripted_sources(&self) -> Result<Vec<PlacedSource>, String> {
        let mut rows = Vec::new();
        rows.try_reserve_exact(self.doodads.len())
            .map_err(|_| "scripted destructable allocation")?;
        for instance in &self.doodads {
            if matches!(self.role(instance)?, PlacedRole::ScriptOnly) {
                rows.push(placed_source(instance));
            }
        }
        Ok(rows)
    }

    fn path(&self, tables: &DefaultTables, id: [u8; 4], dead: bool) -> Result<String, String> {
        path(tables, &self.destructables, &self.decorations, id, dead)
    }

    fn script_paths(&self) -> ScriptPaths {
        ScriptPaths {
            tables: self.objects.tables(),
            destructables: self.destructables.clone(),
            decorations: self.decorations.clone(),
        }
    }
}

/// The session state the first unit creation supplies to the placement cache.
pub struct Live<'a> {
    pub objects: &'a ObjectHost,
    pub rules: &'a Rules,
    /// Present exactly when the session is tracked (`campaign_host::ai_possible`).
    pub spatial: Option<&'a SpatialRules>,
}

/// Called immediately before the first executed unit, after stationary creations.
pub fn assemble(
    context: &Arc<ParsedMapContext>,
    map_bytes: &[u8],
    reader: Arc<Resolver>,
    tables: &DefaultTables,
    catalog: &Catalog,
    live: Live<'_>,
) -> Result<Placement, String> {
    let Live {
        objects,
        rules,
        spatial,
    } = live;
    let map = pcx_map::Map::parse(map_bytes).map_err(|e| e.to_string())?;
    let extract = |member: &'static str| {
        map.extract_checked(
            &context.map_digest.label,
            member,
            pcx_mpq::checked::ReadBudget {
                max_archive_bytes: 64 << 20,
                max_archive_table_bytes: 8 << 20,
                max_raw_packed_bytes: 16 << 20,
                max_decoded_bytes: 16 << 20,
            },
        )
        .map_err(|e| e.to_string())
    };
    let terrain =
        pcx_map::Terrain::from_bytes(extract("war3map.w3e")?.ok_or("missing placement terrain")?)
            .map_err(|e| e.to_string())?;
    let (x, y) = terrain.origin();
    let domains = domains(&terrain, &context.world.pathing)?;
    let mut grid = Grid {
        source: context.world.source.clone(),
        origin: [x, y],
        pathing: context.world.pathing.clone(),
        domains,
    };
    let mut textures = Textures {
        reader,
        edition: context.sources.map.edition.clone(),
        policy: rules.texture_policy(),
        cache: BTreeMap::new(),
        staged: BTreeMap::new(),
        native: objects
            .region_registration_order_enabled()
            .then(|| Box::new(NativeTextures::default())),
        remaining_bytes: 16 << 20,
        remaining_cells: FOOTPRINT_CELLS,
        remaining_source_attempts: FOOTPRINT_SOURCES,
    };
    let mut pool = spatial_owner(objects, &grid, rules, spatial)?;
    if let Some(pool) = &mut pool {
        pool.retain_context(Arc::clone(context));
    }
    let mut work = STATIONARY_WORK;
    let mut decoration = 0u32;
    for instance in catalog.doodads() {
        if !matches!(catalog.role(instance)?, PlacedRole::Decoration) {
            continue;
        }
        let path = catalog.path(tables, instance.type_id, instance.life == 0)?;
        stamp(
            &mut grid,
            rules,
            &mut textures,
            &path,
            [instance.position[0], instance.position[1]],
            f64::from(instance.angle) / std::f64::consts::FRAC_PI_2,
            &mut work,
        )?;
        if let Some(pool) = &mut pool {
            let pixels = native_footprint_cells(
                &grid,
                rules,
                &textures,
                &path,
                [instance.position[0], instance.position[1]],
                f64::from(instance.angle) / std::f64::consts::FRAC_PI_2,
            )?;
            pool.decoration_pixels(Texture::Decoration(decoration), &pixels);
            decoration += 1;
        }
    }
    let mut layers = Vec::new();
    for (id, instance, state) in objects.destructables_with_handles() {
        let live_path = catalog.path(tables, instance.objectid.to_be_bytes(), false)?;
        let dead_path = catalog.path(tables, instance.objectid.to_be_bytes(), true)?;
        let live = footprint_cells(
            &grid,
            rules,
            &mut textures,
            &live_path,
            [instance.x, instance.y],
            f64::from(instance.face) / 90.0,
            &mut work,
        )?;
        let dead = footprint_cells(
            &grid,
            rules,
            &mut textures,
            &dead_path,
            [instance.x, instance.y],
            f64::from(instance.face) / 90.0,
            &mut work,
        )?;
        if let Some(pool) = &mut pool {
            let point = [instance.x, instance.y];
            let turns = f64::from(instance.face) / 90.0;
            let live_pixels =
                native_footprint_cells(&grid, rules, &textures, &live_path, point, turns)?;
            let dead_pixels =
                native_footprint_cells(&grid, rules, &textures, &dead_path, point, turns)?;
            pool.stationary_pixels(
                Texture::Destructable(id),
                live_pixels,
                dead_pixels,
                !state.dead,
            );
        }
        layers.push(StationaryStamp {
            key: id,
            live,
            dead,
            mask: rules.solid_mask(),
            active: !state.dead,
        });
    }
    if let Some(pool) = &mut pool {
        for addition in objects.region_registrations().map_err(|e| e.to_string())? {
            let (region, request) = addition.map_err(|e| e.to_string())?;
            pool.region_added(region, request.bounds);
        }
    }
    let base = grid.pathing.clone();
    let script_paths = catalog.script_paths();
    let mut placement = Placement::new(
        grid,
        rules.clone(),
        BTreeMap::new(),
        Budget {
            cells: CELLS,
            footprint_cells: FOOTPRINT_CELLS,
            stationary_cells: STATIONARY_CELLS,
            stationary_work: STATIONARY_WORK,
            queries: 1 << 16,
        },
    )
    .and_then(|p| p.with_stationary(base, layers))
    .map(|p| {
        p.with_staged_footprint_loader(TextureLoader { textures })
            .with_script_paths(move |id, dead| script_paths.resolve(id, dead))
    })
    .map_err(|e| e.to_string())?;
    if let Some(pool) = pool {
        placement.attach_spatial(pool);
    }
    Ok(placement)
}

fn native_footprint_cells(
    grid: &Grid,
    rules: &Rules,
    textures: &Textures,
    path: &str,
    point: [f32; 2],
    turns: f64,
) -> Result<crate::spatial_pool::TexturePixels, String> {
    if matches!(path, "" | "_" | "none") {
        return Ok(Vec::new());
    }
    let key = Textures::key(path)?;
    let texture = textures
        .native
        .as_ref()
        .and_then(|cache| cache.committed.get(&key))
        .ok_or_else(|| format!("native texture flags missing for {path}"))?;
    project_native_texture(grid, rules, texture, point, turns)
}

pub(crate) fn project_native_texture(
    grid: &Grid,
    rules: &Rules,
    texture: &NativeTexture,
    point: [f32; 2],
    turns: f64,
) -> Result<crate::spatial_pool::TexturePixels, String> {
    let dimensions = [u32::from(texture.height), u32::from(texture.width)];
    let quarter = turns.round_ties_even().rem_euclid(4.0) as u8;
    let anchor = anchor(grid, rules, point, dimensions, quarter)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(texture.flags.len())
        .map_err(|_| "native stationary footprint allocation")?;
    for (i, &flags) in texture.flags.iter().enumerate() {
        if flags == 0 {
            continue;
        }
        let offset = [
            (i / usize::from(texture.width)) as i32 - i32::from(texture.height) / 2,
            (i % usize::from(texture.width)) as i32 - i32::from(texture.width) / 2,
        ];
        let index = clamped_index(grid, anchor, rotate_offset(offset, dimensions, quarter));
        result.push((
            u32::try_from(index).map_err(|_| "native texture cell index overflow")?,
            flags,
        ));
    }
    Ok(result)
}

/// The anchor cell of a stationary footprint (`stationary_anchor`) on the selected grid.
fn anchor(
    grid: &Grid,
    rules: &Rules,
    point: [f32; 2],
    dimensions: [u32; 2],
    quarter: u8,
) -> Result<[i64; 2], String> {
    let size = rules.cell_size();
    let bounds = [
        grid.origin,
        [
            grid.origin[0] + grid.pathing.width as f32 * size,
            grid.origin[1] + grid.pathing.height as f32 * size,
        ],
    ];
    stationary_anchor(
        rules.footprint_geometry(),
        bounds,
        size,
        point,
        dimensions,
        quarter,
    )
}

/// The cell index of a texel offset from `anchor`. `0x804be0` clamps the texel position to
/// the map before converting it to a cell, so a texel outside the map takes a border cell.
fn clamped_index(grid: &Grid, anchor: [i64; 2], offset: [i32; 2]) -> usize {
    let [x, y] = clamp_cell(
        [
            anchor[0] + i64::from(offset[0]),
            anchor[1] + i64::from(offset[1]),
        ],
        grid.pathing.width,
        grid.pathing.height,
    );
    y as usize * grid.pathing.width as usize + x as usize
}

/// The chain owner for a tracked session, or `None` when region history was not enabled.
fn spatial_owner(
    objects: &ObjectHost,
    grid: &Grid,
    rules: &Rules,
    spatial: Option<&SpatialRules>,
) -> Result<Option<PoolOwner>, String> {
    if !objects.region_registration_order_enabled() {
        return Ok(None);
    }
    let spatial = spatial.ok_or("tracked session lacks the spatial chain operands")?;
    let start = objects
        .timer_game_time()
        .ok_or("tracked session lacks the game clock for the spatial cleanup")?;
    let interval = objects.spatial_cleanup_interval(spatial)?;
    let geometry = Geometry {
        origin: grid.origin,
        cell_size: rules.cell_size(),
        width: grid.pathing.width as usize,
        height: grid.pathing.height as usize,
    };
    PoolOwner::new(spatial.clone(), geometry, start as f32, interval)
        .map(Some)
        .map_err(|e| e.to_string())
}

fn placed_source(instance: &pcx_map::Doodad) -> PlacedSource {
    PlacedSource {
        creation_number: instance.creation_number,
        type_id: instance.type_id,
        variation: instance.variation,
        position: instance.position,
        angle: instance.angle,
        scale: instance.scale,
        flags: instance.flags,
        life: instance.life,
    }
}

fn domains(terrain: &pcx_map::Terrain, pathing: &pcx_map::Pathing) -> Result<Vec<u8>, String> {
    let alignment = pcx_map::grid_alignment::terrain_pathing_alignment(
        terrain.width(),
        terrain.height(),
        pathing.width,
        pathing.height,
    )
    .map_err(|e| e.to_string())?;
    let scale = alignment.width_scale;
    let count = usize::try_from(u64::from(pathing.width) * u64::from(pathing.height))
        .map_err(|_| "placement cell count")?;
    if scale != alignment.height_scale
        || scale == 0
        || scale % 2 != 0
        || count > CELLS
        || count != pathing.cells.len()
    {
        return Err("placement grid dimensions or budget".into());
    }
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| "placement domain allocation")?;
    for y in 0..pathing.height {
        for x in 0..pathing.width {
            let corner = terrain
                .corner((x + scale / 2) / scale, (y + scale / 2) / scale)
                .ok_or("placement terrain-domain corner")?;
            result.push(corner.byte6 & 0x0f);
        }
    }
    Ok(result)
}

fn path(
    tables: &DefaultTables,
    destructables: &Overlay,
    decorations: &Overlay,
    id: [u8; 4],
    dead: bool,
) -> Result<String, String> {
    let name = std::str::from_utf8(&id).map_err(|_| "non-UTF8 stationary identity")?;
    let mut found = None;
    for (family, slk, overlay) in [
        (Family::Destructable, SlkId::DestructableData, destructables),
        (Family::Doodad, SlkId::Doodads, decorations),
    ] {
        let custom = overlay.new_object(name);
        let (base, mods) = custom.unwrap_or((name, overlay.on(name)));
        if overlay.new_object(base).is_some() {
            return Err(format!("{name}: custom stationary base chain"));
        }
        let present = custom.is_some() || overlay.mods.contains_key(name);
        let object = match tables.object(family, base) {
            Ok(object) => object,
            Err(Unresolved::Object { .. }) if !present => continue,
            Err(error) => {
                return Err(format!("{name}: malformed stationary record: {error}"));
            }
        };
        if object.rawcode() != base {
            return Err(format!(
                "{name}: stationary lookup resolved case-insensitively to {}",
                object.rawcode()
            ));
        }
        if found.is_some() {
            return Err(format!("{name}: ambiguous stationary family"));
        }
        if mods.len() > 4096 {
            return Err(format!("{name}: stationary modification budget"));
        }
        let column = if family == Family::Destructable && dead {
            "pathTexDeath"
        } else {
            "pathTex"
        };
        let mut fields = tables.editor().fields().iter().filter(|f| f.from == family.metadata()
            && matches!(&f.source, FieldSource::Column { slk: s, column: c } if *s == slk && c == column));
        let metadata = fields
            .next()
            .ok_or_else(|| format!("{name}: missing {column} metadata"))?;
        if fields.next().is_some()
            || !matches!(metadata.index, -1 | 0)
            || !metadata.measured
            || metadata.kind != FieldKind::Token
        {
            return Err(format!("{name}: unsupported {column} metadata"));
        }
        let value = if let Some(edit) = mods.iter().rev().find(|m| m.field == metadata.fourcc) {
            if edit.selectors.is_some_and(|s| s != [0, 0])
                || edit.value_type.word() != metadata.kind.wire_word()
            {
                return Err(format!("{name}: unsupported {column} edit"));
            }
            let pcx_objects::Value::Str(raw) = &edit.value else {
                return Err(format!("{name}: nontext {column}"));
            };
            std::str::from_utf8(raw)
                .map_err(|_| format!("{name}: non-UTF8 {column}"))?
                .to_owned()
        } else {
            object
                .cell(slk, column)
                .map_err(|e| e.to_string())?
                .text
                .to_owned()
        };
        found = Some(value);
    }
    found.ok_or_else(|| format!("{name}: missing stationary catalog record"))
}

struct Staged {
    footprint: Arc<Footprint>,
    bytes: usize,
    cells: usize,
}

#[derive(Default)]
struct NativeTextures {
    committed: BTreeMap<String, NativeTexture>,
    staged: BTreeMap<String, NativeTexture>,
}

struct Decoded {
    footprint: Arc<Footprint>,
    native: Option<NativeTexture>,
    bytes: usize,
    cells: usize,
}

struct Textures {
    reader: Arc<Resolver>,
    edition: String,
    policy: TexturePolicy,
    cache: BTreeMap<String, Arc<Footprint>>,
    staged: BTreeMap<String, Staged>,
    native: Option<Box<NativeTextures>>,
    /// Retained-cache capacity. A rolled-back staged load returns its share.
    remaining_bytes: usize,
    /// Retained-cell capacity. A rolled-back staged load returns its share.
    remaining_cells: usize,
    /// Non-refundable uncached resolve/decode attempts. Cache and staged hits
    /// are free; rollback never returns an attempt.
    remaining_source_attempts: usize,
}

impl Textures {
    fn read(&mut self, path: &str) -> Result<Arc<Footprint>, String> {
        let key = Self::key(path)?;
        if let Some(cached) = self.cache.get(&key) {
            return Ok(Arc::clone(cached));
        }
        if self.native.is_some() && self.staged.contains_key(&key) {
            return Err("native texture read while the same source is staged".into());
        }
        let decoded = self.decode(path)?;
        if let (Some(native), Some(cache)) = (decoded.native, &mut self.native) {
            cache.committed.insert(key.clone(), native);
        }
        self.cache.insert(key, Arc::clone(&decoded.footprint));
        Ok(decoded.footprint)
    }

    fn stage(&mut self, path: &str) -> Result<Arc<Footprint>, String> {
        let key = Self::key(path)?;
        if let Some(cached) = self.cache.get(&key) {
            return Ok(Arc::clone(cached));
        }
        if let Some(staged) = self.staged.get(&key) {
            return Ok(Arc::clone(&staged.footprint));
        }
        let decoded = self.decode(path)?;
        if let (Some(native), Some(cache)) = (decoded.native, &mut self.native) {
            cache.staged.insert(key.clone(), native);
        }
        self.staged.insert(
            key,
            Staged {
                footprint: Arc::clone(&decoded.footprint),
                bytes: decoded.bytes,
                cells: decoded.cells,
            },
        );
        Ok(decoded.footprint)
    }

    fn commit(&mut self, paths: &[String]) {
        for path in paths {
            let Ok(key) = Self::key(path) else {
                continue;
            };
            if let Some(staged) = self.staged.remove(&key) {
                if let Some(cache) = &mut self.native
                    && let Some(native) = cache.staged.remove(&key)
                {
                    cache.committed.insert(key.clone(), native);
                }
                self.cache.insert(key, staged.footprint);
            }
        }
    }

    fn rollback(&mut self, paths: &[String]) {
        for path in paths {
            let Ok(key) = Self::key(path) else {
                continue;
            };
            if let Some(staged) = self.staged.remove(&key) {
                if let Some(cache) = &mut self.native {
                    cache.staged.remove(&key);
                }
                self.remaining_bytes = self.remaining_bytes.saturating_add(staged.bytes);
                self.remaining_cells = self.remaining_cells.saturating_add(staged.cells);
            }
        }
    }

    fn key(path: &str) -> Result<String, String> {
        if path.len() > 16 << 10 {
            return Err("footprint path budget".into());
        }
        Ok(AssetPath::new(path)
            .map_err(|e| e.to_string())?
            .as_str()
            .to_owned())
    }

    fn decode(&mut self, path: &str) -> Result<Decoded, String> {
        self.decode_kind(path, false)
    }

    fn decode_kind(&mut self, path: &str, building: bool) -> Result<Decoded, String> {
        if self.cache.len() + self.staged.len() >= FOOTPRINT_SOURCES {
            return Err("footprint source count budget".into());
        }
        let remaining = self
            .remaining_source_attempts
            .checked_sub(1)
            .ok_or("footprint source attempt budget")?;
        self.remaining_source_attempts = remaining;
        let asset = self
            .reader
            .resolve_bounded(path, self.remaining_bytes.min(1 << 20) as u64)
            .map_err(|e| e.to_string())?;
        let (_, origin) = crate::campaign_assets::identity(&asset, &self.edition, 16 << 10)?;
        let header = pcx_tga::Header::parse(&asset.bytes).map_err(|e| e.to_string())?;
        let count = usize::from(header.width) * usize::from(header.height);
        if count > FOOTPRINT_CELLS
            || header.width == 0
            || header.height == 0
            || (building && (header.width % 2 != 0 || header.height % 2 != 0))
        {
            return Err(format!(
                "{path}: footprint cell budget or unsupported empty/odd dimensions"
            ));
        }
        let rgba = pcx_tga::decode_with(&asset.bytes, &header).map_err(|e| e.to_string())?;
        let cells = if building {
            building_raster(header.width, header.height, &rgba, self.policy)?
        } else {
            raster(header.width, header.height, &rgba, self.policy)?
        };
        let retain_native = if let Some(cache) = &self.native {
            let key = Self::key(path)?;
            !cache.committed.contains_key(&key) && !cache.staged.contains_key(&key)
        } else {
            false
        };
        let native = if retain_native {
            let mut flags = Vec::new();
            flags
                .try_reserve_exact(count)
                .map_err(|_| "native texture flag allocation")?;
            flags.extend(
                rgba.as_chunks::<4>()
                    .0
                    .iter()
                    .map(|pixel| self.policy.flags(*pixel)),
            );
            Some(NativeTexture {
                origin: origin.clone(),
                width: header.width,
                height: header.height,
                flags,
            })
        } else {
            None
        };
        let cell_count = cells
            .len()
            .checked_add(native.as_ref().map_or(0, |native| native.flags.len()))
            .ok_or("footprint aggregate cell count overflow")?;
        let remaining_cells = self
            .remaining_cells
            .checked_sub(cell_count)
            .ok_or("footprint aggregate cell budget")?;
        let bytes = asset.bytes.len();
        let remaining_bytes = self
            .remaining_bytes
            .checked_sub(bytes)
            .ok_or("footprint source byte budget")?;
        self.remaining_cells = remaining_cells;
        self.remaining_bytes = remaining_bytes;
        Ok(Decoded {
            footprint: Arc::new(Footprint {
                origin,
                cells,
                dimensions: [u32::from(header.height), u32::from(header.width)],
            }),
            native,
            bytes,
            cells: cell_count,
        })
    }
}

struct TextureLoader {
    textures: Textures,
}

impl FootprintLoader for TextureLoader {
    fn load(&mut self, path: &str) -> Result<Footprint, String> {
        self.textures.stage(path).map(|f| f.as_ref().clone())
    }

    fn load_building(&mut self, path: &str) -> Result<Option<Footprint>, String> {
        if self.textures.native.is_some()
            && self.textures.staged.contains_key(&Textures::key(path)?)
        {
            return Err("native building texture read while the source is staged".into());
        }
        let decoded = self.textures.decode_kind(path, true)?;
        if let (Some(native), Some(cache)) = (decoded.native, &mut self.textures.native) {
            cache.committed.insert(Textures::key(path)?, native);
        }
        Ok(Some(decoded.footprint.as_ref().clone()))
    }

    fn native_texture(&self, path: &str) -> Option<&NativeTexture> {
        let key = Textures::key(path).ok()?;
        let cache = self.textures.native.as_ref()?;
        cache.committed.get(&key).or_else(|| cache.staged.get(&key))
    }

    fn commit(&mut self, paths: &[String]) {
        self.textures.commit(paths);
    }

    fn rollback(&mut self, paths: &[String]) {
        self.textures.rollback(paths);
    }
}

fn building_raster(
    width: u16,
    height: u16,
    rgba: &[u8],
    policy: TexturePolicy,
) -> Result<Vec<[i32; 2]>, String> {
    let count = usize::from(width) * usize::from(height);
    if width == 0 || height == 0 || count > FOOTPRINT_CELLS || rgba.len() != count * 4 {
        return Err("building footprint raster size".into());
    }
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| "building footprint allocation")?;
    for (i, pixel) in rgba.as_chunks::<4>().0.iter().enumerate() {
        if policy.checks_construction(*pixel) {
            result.push([
                (i / usize::from(width)) as i32 - i32::from(height) / 2,
                (i % usize::from(width)) as i32 - i32::from(width) / 2,
            ]);
        }
    }
    Ok(result)
}

/// Footprint cells of the texels whose red channel links the movement entries: the native
/// constructor tests one red bit (`0x4deb57`), whatever the other bits hold.
fn raster(
    width: u16,
    height: u16,
    rgba: &[u8],
    policy: TexturePolicy,
) -> Result<Vec<[i32; 2]>, String> {
    let count = usize::from(width) * usize::from(height);
    if width == 0 || height == 0 || count > FOOTPRINT_CELLS || rgba.len() != count * 4 {
        return Err("footprint raster size".into());
    }
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| "footprint cell allocation")?;
    for (i, pixel) in rgba.as_chunks::<4>().0.iter().enumerate() {
        if policy.blocks_movement(*pixel) {
            result.push([
                (i / usize::from(width)) as i32 - i32::from(height) / 2,
                (i % usize::from(width)) as i32 - i32::from(width) / 2,
            ]);
        }
    }
    Ok(result)
}

fn footprint_cells(
    grid: &Grid,
    rules: &Rules,
    textures: &mut Textures,
    path: &str,
    point: [f32; 2],
    turns: f64,
    work: &mut usize,
) -> Result<Vec<usize>, String> {
    if matches!(path, "" | "_" | "none") {
        return Ok(Vec::new());
    }
    if !turns.is_finite() || point.iter().any(|x| !x.is_finite()) {
        return Err("nonfinite stationary footprint pose".into());
    }
    let footprint = textures.read(path)?;
    *work = work
        .checked_sub(footprint.cells.len())
        .ok_or("stationary footprint work budget")?;
    // shortcut: nearest quarter-turn static raster, validated for the retained creation stream; qualify arbitrary-angle rasterization before widening fidelity claims.
    let quarter = turns.round_ties_even().rem_euclid(4.0) as u8;
    let anchor = anchor(grid, rules, point, footprint.dimensions, quarter)?;
    let mut cells = Vec::new();
    cells
        .try_reserve_exact(footprint.cells.len())
        .map_err(|_| "stationary cell allocation")?;
    for &offset in &footprint.cells {
        cells.push(clamped_index(
            grid,
            anchor,
            rotate_offset(offset, footprint.dimensions, quarter),
        ));
    }
    cells.sort_unstable();
    cells.dedup();
    Ok(cells)
}

fn stamp(
    grid: &mut Grid,
    rules: &Rules,
    textures: &mut Textures,
    path: &str,
    point: [f32; 2],
    turns: f64,
    work: &mut usize,
) -> Result<Vec<usize>, String> {
    let cells = footprint_cells(grid, rules, textures, path, point, turns, work)?;
    for &i in &cells {
        grid.pathing.cells[i] |= rules.solid_mask();
    }
    Ok(cells)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A version-1 plain object file with one custom record carrying one `unreal` modification.
    fn plain_object_file(original: &[u8; 4], custom: &[u8; 4], field: &[u8; 4]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend(1u32.to_le_bytes());
        bytes.extend(0u32.to_le_bytes());
        bytes.extend(1u32.to_le_bytes());
        bytes.extend(original);
        bytes.extend(custom);
        bytes.extend(1u32.to_le_bytes());
        bytes.extend(field);
        bytes.extend(2u32.to_le_bytes());
        bytes.extend(2.5f32.to_le_bytes());
        bytes.extend(0u32.to_le_bytes());
        bytes
    }

    #[test]
    fn destructables_come_from_w3b_and_never_from_the_buff_member() {
        let w3b = plain_object_file(b"LTe3", b"B000", b"bmas");
        let w3h = plain_object_file(b"BHwe", b"B001", b"fnam");
        let w3d = plain_object_file(b"LTb1", b"D000", b"dmas");
        let (destructables, decorations) = stationary_overlays(|member| {
            Ok(match member {
                "war3map.w3b" => Some(w3b.clone()),
                "war3map.w3h" => Some(w3h.clone()),
                "war3map.w3d" => Some(w3d.clone()),
                other => panic!("unexpected member {other}"),
            })
        })
        .unwrap();
        assert_eq!(destructables.new_object("B000").unwrap().0, "LTe3");
        assert!(destructables.new_object("B001").is_none());
        assert_eq!(decorations.new_object("D000").unwrap().0, "LTb1");
        let (none, _) = stationary_overlays(|_| Ok(None)).unwrap();
        assert!(none.mods.is_empty() && none.custom.is_empty());
        let error = stationary_overlays(|_| Ok(Some(w3b[..10].to_vec()))).unwrap_err();
        assert!(!error.is_empty());
    }

    #[test]
    fn domains_follow_the_selected_grid_ratio_and_corner_words() {
        let mut bytes = pcx_map::TERRAIN_MAGIC.to_vec();
        bytes.extend(pcx_map::TERRAIN_VERSION.to_le_bytes());
        bytes.push(0);
        for word in [0u32, 0, 0, 3, 3] {
            bytes.extend(word.to_le_bytes());
        }
        bytes.extend([0; 8]);
        for i in 1..=9 {
            let mut corner = vec![0; pcx_map::CORNER_RECORD];
            corner[6] = 0xa0 + i;
            bytes.extend(corner);
        }
        let terrain = pcx_map::Terrain::from_bytes(bytes).unwrap();
        let mut pathing = pcx_map::Pathing {
            version: 0,
            width: 8,
            height: 8,
            cells: vec![0; 64],
        };
        let result = domains(&terrain, &pathing).unwrap();
        assert_eq!(&result[..8], &[1, 1, 2, 2, 2, 2, 3, 3]);
        assert_eq!(result[2 * 8], 4);
        assert_eq!(result[6 * 8], 7);
        pathing.width = 7;
        assert!(domains(&terrain, &pathing).is_err());
    }

    #[test]
    fn stationary_paths_follow_metadata_stock_and_map_edits() {
        let table = |headers: &[&str], rows: &[&[&str]]| {
            let mut s = String::from("ID;PWXL;N;E\r\n");
            for (y, row) in std::iter::once(headers)
                .chain(rows.iter().copied())
                .enumerate()
            {
                for (x, value) in row.iter().enumerate() {
                    s.push_str(&format!("C;X{};Y{};K\"{}\"\r\n", x + 1, y + 1, value));
                }
            }
            s.push_str("E\r\n");
            s.into_bytes()
        };
        let data = table(
            &["DestructableID", "pathTex", "pathTexDeath"],
            &[&["S001", "live.tga", "dead.tga"]],
        );
        let metadata = |index: &str| {
            table(
                &["ID", "field", "slk", "type", "index", "category"],
                &[
                    &[
                        "Qpth",
                        "pathTex",
                        "DestructableData",
                        "string",
                        index,
                        "pathing",
                    ],
                    &[
                        "Qded",
                        "pathTexDeath",
                        "DestructableData",
                        "string",
                        index,
                        "pathing",
                    ],
                ],
            )
        };
        let load = |metadata: &[u8]| {
            DefaultTables::from_bytes(
                pcx_objects::editor::defaults::Identity {
                    install: "synthetic".into(),
                    edition_version: "synthetic".into(),
                },
                &[
                    (SlkId::DestructableData.path(), &data),
                    (SlkId::DestructableMetaData.path(), metadata),
                ],
                &[],
                pcx_objects::editor::EditorReadLimits {
                    max_listed_paths: 4,
                    max_asset_bytes: 4096,
                    max_aggregate_input_bytes: 8192,
                },
            )
            .unwrap()
        };
        let tables = load(&metadata("0"));
        let mut edits = Overlay::default();
        let empty = Overlay::default();
        assert_eq!(
            path(&tables, &edits, &empty, *b"S001", false).unwrap(),
            "live.tga"
        );
        assert_eq!(
            path(&load(&metadata("-1")), &edits, &empty, *b"S001", false).unwrap(),
            "live.tga"
        );
        assert!(path(&load(&metadata("1")), &edits, &empty, *b"S001", false).is_err());
        assert_eq!(
            path(&tables, &edits, &empty, *b"S001", true).unwrap(),
            "dead.tga"
        );
        edits.custom.insert(
            "S002".into(),
            (
                "S001".into(),
                vec![pcx_objects::Mod {
                    field: *b"Qpth",
                    value_type: pcx_objects::ValueType::Str,
                    selectors: Some([0, 0]),
                    value: pcx_objects::Value::Str(b"custom.tga".to_vec()),
                    end_word: 0,
                }],
            ),
        );
        assert_eq!(
            path(&tables, &edits, &empty, *b"S002", false).unwrap(),
            "custom.tga"
        );
        edits.custom.get_mut("S002").unwrap().1[0].selectors = Some([1, 0]);
        assert!(path(&tables, &edits, &empty, *b"S002", false).is_err());
        assert!(path(&tables, &edits, &empty, *b"S404", false).is_err());
    }

    fn slk_table(headers: &[&str], rows: &[&[&str]]) -> Vec<u8> {
        let mut text = String::from("ID;PWXL;N;E\r\n");
        for (y, row) in std::iter::once(headers)
            .chain(rows.iter().copied())
            .enumerate()
        {
            for (x, value) in row.iter().enumerate() {
                text.push_str(&format!("C;X{};Y{};K\"{}\"\r\n", x + 1, y + 1, value));
            }
        }
        text.push_str("E\r\n");
        text.into_bytes()
    }

    fn catalog_with_life(max_life: &str) -> Catalog {
        let data = slk_table(
            &[
                "DestructableID",
                "HP",
                "targType",
                "pathTex",
                "pathTexDeath",
            ],
            &[&["LTlt", max_life, "tree,ground", "live.tga", "dead.tga"]],
        );
        let decorations = slk_table(&["doodID"], &[&["LOsm"]]);
        let metadata = slk_table(
            &["ID", "field", "slk", "type", "index", "category"],
            &[
                &["Ahpv", "HP", "DestructableData", "int", "0", "stats"],
                &[
                    "Atgt",
                    "targType",
                    "DestructableData",
                    "string",
                    "0",
                    "combat",
                ],
                &[
                    "Qpth",
                    "pathTex",
                    "DestructableData",
                    "string",
                    "0",
                    "pathing",
                ],
                &[
                    "Qded",
                    "pathTexDeath",
                    "DestructableData",
                    "string",
                    "0",
                    "pathing",
                ],
            ],
        );
        let tables = DefaultTables::from_bytes(
            pcx_objects::editor::defaults::Identity {
                install: "synthetic".into(),
                edition_version: "synthetic".into(),
            },
            &[
                (SlkId::DestructableData.path(), &data),
                (SlkId::DestructableMetaData.path(), &metadata),
                (SlkId::Doodads.path(), &decorations),
            ],
            &[],
            pcx_objects::editor::EditorReadLimits {
                max_listed_paths: 4,
                max_asset_bytes: 4096,
                max_aggregate_input_bytes: 8192,
            },
        )
        .unwrap();
        Catalog {
            objects: DestructableCatalog::new(
                Arc::new(tables),
                Overlay::default(),
                Overlay::default(),
            ),
            doodads: Vec::new(),
            terrain_doodads: Vec::new(),
            destructables: Overlay::default(),
            decorations: Overlay::default(),
        }
    }

    #[test]
    fn script_paths_resolve_the_selected_live_dead_cells_and_map_edits() {
        let mut catalog = catalog_with_life("50");
        let id = i32::from_be_bytes(*b"LTlt");
        assert_eq!(
            catalog.script_paths().resolve(id, false).unwrap(),
            "live.tga"
        );
        assert_eq!(
            catalog.script_paths().resolve(id, true).unwrap(),
            "dead.tga"
        );
        let mut edits = Overlay::default();
        edits.custom.insert(
            "LTl2".into(),
            (
                "LTlt".into(),
                vec![pcx_objects::Mod {
                    field: *b"Qpth",
                    value_type: pcx_objects::ValueType::Str,
                    selectors: Some([0, 0]),
                    value: pcx_objects::Value::Str(b"custom.tga".to_vec()),
                    end_word: 0,
                }],
            ),
        );
        catalog.destructables = edits;
        let edited = i32::from_be_bytes(*b"LTl2");
        assert_eq!(
            catalog.script_paths().resolve(edited, false).unwrap(),
            "custom.tga"
        );
        assert_eq!(
            catalog.script_paths().resolve(edited, true).unwrap(),
            "dead.tga"
        );
    }

    /// Version-7 `war3map.doo`: one instance when `instance` is set, then the terrain
    /// section with one 16-byte record per `(grid_x, grid_y)`.
    fn doo(instance: bool, terrain: &[[u32; 2]]) -> Vec<u8> {
        let mut data = b"W3do".to_vec();
        for word in [7u32, 9, u32::from(instance)] {
            data.extend(word.to_le_bytes());
        }
        if instance {
            data.extend(*b"LOsm");
            data.extend(0u32.to_le_bytes());
            for value in [0.0f32, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0] {
                data.extend(value.to_le_bytes());
            }
            data.extend([2u8, 100u8]);
            data.extend(7u32.to_le_bytes());
        }
        data.extend(0u32.to_le_bytes());
        data.extend(u32::try_from(terrain.len()).unwrap().to_le_bytes());
        for [x, y] in terrain {
            data.extend(*b"LCc0");
            data.extend(0u32.to_le_bytes());
            data.extend(x.to_le_bytes());
            data.extend(y.to_le_bytes());
        }
        data
    }

    #[test]
    fn terrain_doodads_are_kept_without_refusing_the_doodad_file() {
        let parsed = parse_doodads(doo(true, &[[44, 116]])).unwrap();
        assert_eq!(parsed.instances().len(), 1);
        assert_eq!(
            parsed.terrain_doodads(),
            [pcx_map::TerrainDoodad {
                type_id: *b"LCc0",
                variation: 0,
                grid_x: 44,
                grid_y: 116,
            }]
        );
    }

    #[test]
    fn truncated_doodad_headers_are_refused() {
        for empty in [Vec::new(), b"W3do\x07\0\0\0\x09\0\0\0".to_vec()] {
            assert_eq!(
                parse_doodads(empty).unwrap_err(),
                "truncated placement doodad count"
            );
        }
        let data = doo(false, &[]);
        assert!(parse_doodads(data[..data.len() - 1].to_vec()).is_err());
    }

    #[test]
    fn truncated_terrain_record_is_refused_with_the_declared_counts() {
        let data = doo(false, &[[1, 2]]);
        let error = parse_doodads(data[..data.len() - 1].to_vec()).unwrap_err();
        assert!(error.contains("terrain doodads, which need"), "{error}");
    }

    #[test]
    fn declared_instance_count_over_the_budget_is_refused_before_decoding() {
        let mut data = b"W3do".to_vec();
        for word in [7u32, 9, 100001] {
            data.extend(word.to_le_bytes());
        }
        let error = parse_doodads(data).unwrap_err();
        assert_eq!(error, "placement stationary count budget");
    }

    fn doodad(creation_number: u32, life: u8) -> pcx_map::Doodad {
        doodad_with(creation_number, *b"LTlt", 2, life)
    }

    fn doodad_with(creation_number: u32, type_id: [u8; 4], flags: u8, life: u8) -> pcx_map::Doodad {
        pcx_map::Doodad {
            type_id,
            variation: 0,
            position: [100.0, 200.0, 0.0],
            angle: 0.0,
            scale: [1.0; 3],
            flags,
            life,
            creation_number,
        }
    }

    #[test]
    fn placed_life_is_the_authored_percent_of_the_catalog_maximum() {
        let mut catalog = catalog_with_life("50");
        catalog.doodads = vec![
            doodad(1, 0),
            doodad(2, 25),
            doodad(3, 100),
            doodad(4, 200),
            doodad(5, 255),
        ];
        let rows = catalog.placed().unwrap();
        assert_eq!(rows.len(), 5);
        for (row, life) in rows.iter().zip([0.0, 12.5, 50.0, 50.0, 50.0]) {
            assert_eq!(row.state.max_life, Some(50.0));
            assert_eq!(row.state.life, Some(life));
            assert_eq!(row.state.dead, life == 0.0);
            assert_eq!(row.creation.objectid, i32::from_be_bytes(*b"LTlt"));
            assert_eq!(row.creation.x, 100.0);
            assert_eq!(row.creation.y, 200.0);
        }
        assert_eq!(
            rows[0].creation.origin,
            DestructableOrigin::Placed { creation_number: 1 }
        );
        assert_eq!(
            rows[0].source,
            PlacedSource {
                creation_number: 1,
                type_id: *b"LTlt",
                variation: 0,
                position: [100.0, 200.0, 0.0],
                angle: 0.0,
                scale: [1.0; 3],
                flags: 2,
                life: 0,
            }
        );
        assert_eq!(
            rows[4].creation.origin,
            DestructableOrigin::Placed { creation_number: 5 }
        );
    }

    #[test]
    fn placed_life_refuses_unknown_or_invalid_records_before_producing_rows() {
        let mut catalog = catalog_with_life("0");
        catalog.doodads = vec![doodad(1, 100)];
        assert!(catalog.placed().is_err());
        let mut catalog = catalog_with_life("50");
        catalog.doodads = vec![pcx_map::Doodad {
            type_id: *b"LTlt",
            variation: 0,
            position: [f32::NAN, 0.0, 0.0],
            angle: 0.0,
            scale: [1.0; 3],
            flags: 2,
            life: 100,
            creation_number: 1,
        }];
        assert!(catalog.placed().is_err());
        let mut catalog = catalog_with_life("50");
        let mut row = doodad(1, 100);
        row.scale[2] = f32::INFINITY;
        catalog.doodads = vec![row];
        let error = catalog.placed().unwrap_err();
        assert!(
            error.contains("nonfinite placed destructable pose"),
            "{error}"
        );
    }

    #[test]
    fn placed_flags_select_automatic_scripted_and_decorative_rows() {
        let mut catalog = catalog_with_life("50");
        catalog.doodads = vec![
            doodad_with(1, *b"LTlt", 0, 100),
            doodad_with(2, *b"LTlt", 2, 100),
            doodad_with(3, *b"LTlt", 3, 100),
            doodad_with(4, *b"LTlt", 6, 100),
            doodad_with(5, *b"LOsm", 0, 100),
            doodad_with(6, *b"LTlt", 4, 0),
            doodad_with(7, *b"LTlt", 7, 100),
        ];
        let rows = catalog.placed().unwrap();
        assert_eq!(
            rows.iter()
                .map(|row| row.source.creation_number)
                .collect::<Vec<_>>(),
            [2, 4]
        );
        assert_eq!(
            rows.iter().map(|row| row.source.flags).collect::<Vec<_>>(),
            [2, 6]
        );
        let scripted = catalog.scripted_sources().unwrap();
        assert_eq!(
            scripted
                .iter()
                .map(|source| (source.creation_number, source.flags))
                .collect::<Vec<_>>(),
            [(1, 0), (3, 3), (6, 4), (7, 7)]
        );
        assert!(matches!(
            catalog.role(&catalog.doodads[4]).unwrap(),
            PlacedRole::Decoration
        ));
    }

    #[test]
    fn placed_flags_refuse_unmeasured_values_with_source_context() {
        let mut catalog = catalog_with_life("50");
        for flags in [1, 5, 8, u8::MAX] {
            catalog.doodads = vec![doodad_with(37, *b"LTlt", flags, 100)];
            let error = catalog.placed().unwrap_err();
            assert!(
                error.contains(&format!(
                    "LTlt creation 37: unqualified placed destructable flags {flags:#04x}"
                )),
                "{error}"
            );
            assert!(catalog.scripted_sources().is_err());
        }
    }

    struct TextureSource {
        info: pcx_assets::SourceInfo,
        files: BTreeMap<String, Vec<u8>>,
        reads: Arc<AtomicUsize>,
    }

    impl TextureSource {
        fn new(files: &[(&str, Vec<u8>)], reads: Arc<AtomicUsize>) -> Self {
            Self {
                info: pcx_assets::SourceInfo {
                    id: "synthetic-textures".into(),
                    kind: pcx_assets::LayerKind::Mpq,
                    origin: "synthetic-textures.mpq".into(),
                    version: Some("fixture".into()),
                },
                files: files
                    .iter()
                    .map(|(path, bytes)| ((*path).into(), bytes.clone()))
                    .collect(),
                reads,
            }
        }
    }

    impl pcx_assets::ContentSource for TextureSource {
        fn info(&self) -> &pcx_assets::SourceInfo {
            &self.info
        }

        fn paths(&self) -> Result<Vec<String>, pcx_assets::SourceError> {
            Ok(self.files.keys().cloned().collect())
        }

        fn read(
            &self,
            member: &str,
        ) -> Result<Option<pcx_assets::SourceContent>, pcx_assets::SourceError> {
            Ok(self
                .files
                .get(member)
                .map(|bytes| pcx_assets::SourceContent {
                    bytes: bytes.clone(),
                    declared_length: bytes.len() as u64,
                    locator: member.into(),
                }))
        }

        fn read_bounded(
            &self,
            member: &str,
            limit: u64,
        ) -> Result<Option<pcx_assets::SourceContent>, pcx_assets::SourceError> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            let Some(bytes) = self.files.get(member) else {
                return Ok(None);
            };
            if bytes.len() as u64 > limit {
                return Err(pcx_assets::SourceError::Limit {
                    requested: bytes.len() as u64,
                    limit,
                });
            }
            self.read(member)
        }
    }

    fn texture(width: u16, height: u16, pixels: &[[u8; 3]]) -> Vec<u8> {
        let mut bytes = vec![0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        bytes.extend(width.to_le_bytes());
        bytes.extend(height.to_le_bytes());
        bytes.extend([24, 0x20]);
        for [r, g, b] in pixels {
            bytes.extend([*b, *g, *r]);
        }
        bytes
    }

    fn textures_with_reads(files: &[(&str, Vec<u8>)]) -> (Textures, Arc<AtomicUsize>) {
        let reads = Arc::new(AtomicUsize::new(0));
        let textures = Textures {
            reader: Arc::new(
                Resolver::new(
                    pcx_assets::GameMode::default(),
                    None,
                    vec![pcx_assets::ArchiveLayer {
                        priority: 1,
                        source: Box::new(TextureSource::new(files, Arc::clone(&reads))),
                    }],
                )
                .unwrap(),
            ),
            edition: "synthetic".into(),
            policy: crate::unit_placement::tests::rules(2, 0.0).texture_policy(),
            cache: BTreeMap::new(),
            staged: BTreeMap::new(),
            native: None,
            remaining_bytes: 4096,
            remaining_cells: 8,
            remaining_source_attempts: FOOTPRINT_SOURCES,
        };
        (textures, reads)
    }

    #[test]
    fn native_texture_flags_follow_channels_and_staged_rollback() {
        let pixels = [[255, 0, 0], [0, 255, 0], [0, 0, 255], [0, 0, 0]];
        let (mut textures, _) = textures_with_reads(&[("flags.tga", texture(2, 2, &pixels))]);
        textures.native = Some(Box::new(NativeTextures::default()));
        textures.remaining_cells = 16;
        let before = textures.remaining_cells;
        let expected: Vec<_> = pixels
            .iter()
            .map(|&[r, g, b]| textures.policy.flags([r, g, b, 255]))
            .collect();
        textures.stage("flags.tga").unwrap();
        assert!(textures.native.as_ref().unwrap().committed.is_empty());
        assert_eq!(
            textures.native.as_ref().unwrap().staged["flags.tga"].flags,
            expected
        );
        assert!(textures.read("FLAGS.TGA").is_err());
        textures.rollback(&["flags.tga".into()]);
        assert_eq!(textures.remaining_cells, before);
        assert!(textures.staged.is_empty());
        assert!(textures.native.as_ref().unwrap().staged.is_empty());
        textures.stage("flags.tga").unwrap();
        textures.commit(&["flags.tga".into()]);
        let flags = &textures.native.as_ref().unwrap().committed["flags.tga"];
        assert_eq!([flags.width, flags.height], [2, 2]);
        assert_eq!(flags.flags, expected);
        let loader = TextureLoader { textures };
        assert_eq!(loader.native_texture("FLAGS.TGA").unwrap().flags, expected);
    }

    #[test]
    fn native_stationary_pixels_keep_channel_flags_and_rotate() {
        let pixels = [[255, 0, 0], [0, 255, 0], [0, 0, 255], [0, 0, 0]];
        let (mut textures, _) = textures_with_reads(&[("flags.tga", texture(2, 2, &pixels))]);
        textures.native = Some(Box::new(NativeTextures::default()));
        textures.remaining_cells = 16;
        textures.read("flags.tga").unwrap();
        let rules = crate::unit_placement::tests::rules(2, 0.0);
        let grid = Grid {
            source: pcx_sim::world::Source {
                install: "synthetic".into(),
                edition: "test".into(),
                map: "test".into(),
            },
            origin: [0., 0.],
            pathing: pcx_map::Pathing {
                version: 0,
                width: 4,
                height: 4,
                cells: vec![0; 16],
            },
            domains: vec![0; 16],
        };
        let flags: Vec<_> = pixels
            .iter()
            .map(|&[r, g, b]| textures.policy.flags([r, g, b, 255]))
            .collect();
        assert_eq!(
            native_footprint_cells(&grid, &rules, &textures, "flags.tga", [8., 8.], 0.).unwrap(),
            [(5, flags[0]), (9, flags[1]), (6, flags[2])]
        );
        assert_eq!(
            native_footprint_cells(&grid, &rules, &textures, "flags.tga", [8., 8.], 1.).unwrap(),
            [(6, flags[0]), (5, flags[1]), (10, flags[2])]
        );
        // Texels left of or below the map, the green one included, link to the border cell
        // (`0x804be0` clamps the position first), in texel order.
        assert_eq!(
            native_footprint_cells(&grid, &rules, &textures, "flags.tga", [0., 0.], 0.).unwrap(),
            [(0, flags[0]), (0, flags[1]), (0, flags[2])]
        );
        assert_eq!(
            native_footprint_cells(&grid, &rules, &textures, "flags.tga", [64., 64.], 0.).unwrap(),
            [(15, flags[0]), (15, flags[1]), (15, flags[2])]
        );
    }

    #[test]
    fn odd_dimensions_decode_for_stationary_footprints_and_stay_refused_for_buildings() {
        let pixels = [[255, 0, 0]; 9];
        let (mut textures, _) = textures_with_reads(&[("odd.tga", texture(3, 3, &pixels))]);
        textures.remaining_cells = 64;
        assert!(
            textures
                .decode_kind("odd.tga", true)
                .err()
                .unwrap()
                .contains("odd dimensions")
        );
        let grid = Grid {
            source: pcx_sim::world::Source {
                install: "synthetic".into(),
                edition: "test".into(),
                map: "test".into(),
            },
            origin: [0., 0.],
            pathing: pcx_map::Pathing {
                version: 0,
                width: 32,
                height: 32,
                cells: vec![0; 1024],
            },
            domains: vec![0; 1024],
        };
        let rules = crate::unit_placement::tests::rules(5, 0.0);
        let mut work = 64;
        // Without the decoded snap an odd footprint has no defined anchor.
        assert!(
            footprint_cells(
                &grid,
                &rules,
                &mut textures,
                "odd.tga",
                [240., 240.],
                0.,
                &mut work
            )
            .is_err()
        );
        let rules = rules
            .with_footprint_geometry(crate::footprint_geometry::tests::with_building_quarter(3));
        let cells = footprint_cells(
            &grid,
            &rules,
            &mut textures,
            "odd.tga",
            [240., 240.],
            0.,
            &mut work,
        )
        .unwrap();
        let expected: Vec<usize> = (6..=8)
            .flat_map(|y| (6..=8).map(move |x| y * 32 + x))
            .collect();
        assert_eq!(cells, expected);
        // An unaligned position snaps first (`0x6bd2a0`): 224 becomes 240 on the 3-wide axis.
        assert_eq!(
            footprint_cells(
                &grid,
                &rules,
                &mut textures,
                "odd.tga",
                [224., 240.],
                0.,
                &mut work
            )
            .unwrap(),
            expected
        );
    }

    #[test]
    fn script_destructable_registers_staged_native_pixels() {
        let image = texture(2, 2, &[[0, 255, 0], [0; 3], [0; 3], [0; 3]]);
        let (mut textures, _) = textures_with_reads(&[("live.tga", image)]);
        textures.native = Some(Box::new(NativeTextures::default()));
        textures.remaining_cells = 16;
        let source = pcx_sim::world::Source {
            install: "synthetic".into(),
            edition: "test".into(),
            map: "test".into(),
        };
        let mut placement = crate::unit_placement::tests::placement(source, 10000)
            .with_stationary(
                pcx_map::Pathing {
                    version: 0,
                    width: 32,
                    height: 32,
                    cells: vec![0; 1024],
                },
                vec![],
            )
            .unwrap()
            .with_staged_footprint_loader(TextureLoader { textures })
            .with_script_paths(|_, dead| Ok(if dead { "_" } else { "live.tga" }.into()));
        placement.attach_spatial(crate::spatial_pool::tests::pool());
        let prepared = placement
            .prepare_script_destructable(1, [32., 32.], 0., false)
            .unwrap();
        let id = pcx_script::HandleId::new(std::num::NonZeroU64::new(42).unwrap());
        placement.publish_script_destructable(id, prepared);
        let pool = placement.spatial().unwrap();
        assert!(pool.refusal().is_none(), "{:?}", pool.refusal());
        assert!(pool.chain([7, 7]).unwrap().next().is_some());
        assert!(placement.set_destructable_active(id, false));
        assert!(placement.spatial().unwrap().refusal().is_none());
    }

    #[test]
    fn staged_texture_loads_only_charge_the_retained_budget_when_committed() {
        let (mut textures, reads) = textures_with_reads(&[
            (
                "live.tga",
                texture(2, 2, &[[255; 3], [0; 3], [0; 3], [0; 3]]),
            ),
            (
                "dead.tga",
                texture(2, 2, &[[0; 3], [255; 3], [0; 3], [0; 3]]),
            ),
        ]);
        let bytes = textures.remaining_bytes;
        let cells = textures.remaining_cells;
        let live = textures.stage("live.tga").unwrap();
        assert_eq!(live.cells, vec![[-1, -1]]);
        assert_eq!(textures.staged.len(), 1);
        assert!(textures.cache.is_empty());
        assert_eq!(textures.remaining_bytes, bytes - 30);
        assert_eq!(textures.remaining_cells, cells - 1);
        assert_eq!(textures.remaining_source_attempts, FOOTPRINT_SOURCES - 1);
        assert_eq!(reads.load(Ordering::SeqCst), 1);
        assert!(Arc::ptr_eq(&live, &textures.stage("live.tga").unwrap()));
        assert_eq!(textures.staged.len(), 1);
        assert_eq!(textures.remaining_source_attempts, FOOTPRINT_SOURCES - 1);
        assert_eq!(reads.load(Ordering::SeqCst), 1);
        textures.rollback(&["live.tga".into()]);
        assert!(textures.staged.is_empty());
        assert!(textures.cache.is_empty());
        assert_eq!(textures.remaining_bytes, bytes);
        assert_eq!(textures.remaining_cells, cells);
        assert_eq!(textures.remaining_source_attempts, FOOTPRINT_SOURCES - 1);
        assert_eq!(reads.load(Ordering::SeqCst), 1);

        textures.stage("live.tga").unwrap();
        assert_eq!(textures.remaining_source_attempts, FOOTPRINT_SOURCES - 2);
        assert_eq!(reads.load(Ordering::SeqCst), 2);
        textures.commit(&["LIVE.TGA".into()]);
        assert!(textures.staged.is_empty());
        assert!(textures.cache.contains_key("live.tga"));
        assert_eq!(textures.remaining_cells, cells - 1);
        textures.stage("live.tga").unwrap();
        assert_eq!(textures.remaining_cells, cells - 1);
        assert_eq!(textures.remaining_source_attempts, FOOTPRINT_SOURCES - 2);
        assert_eq!(reads.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn canonical_path_aliases_share_one_staged_source_and_one_charge() {
        let (mut textures, reads) = textures_with_reads(&[(
            "textures/live.tga",
            texture(2, 2, &[[255; 3], [0; 3], [0; 3], [0; 3]]),
        )]);
        let bytes = textures.remaining_bytes;
        let cells = textures.remaining_cells;
        let first = textures.stage("Textures\\Live.TGA").unwrap();
        assert_eq!(reads.load(Ordering::SeqCst), 1);
        assert_eq!(textures.remaining_source_attempts, FOOTPRINT_SOURCES - 1);
        assert_eq!(textures.remaining_bytes, bytes - 30);
        assert_eq!(textures.remaining_cells, cells - 1);
        assert!(textures.staged.contains_key("textures/live.tga"));
        let second = textures.stage("textures/live.tga").unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(textures.staged.len(), 1);
        assert_eq!(reads.load(Ordering::SeqCst), 1);
        assert_eq!(textures.remaining_source_attempts, FOOTPRINT_SOURCES - 1);
        assert_eq!(textures.remaining_bytes, bytes - 30);
        assert_eq!(textures.remaining_cells, cells - 1);

        textures.commit(&["TEXTURES\\LIVE.TGA".into()]);
        assert!(textures.staged.is_empty());
        assert!(textures.cache.contains_key("textures/live.tga"));
        assert_eq!(textures.remaining_bytes, bytes - 30);
        assert_eq!(textures.remaining_cells, cells - 1);
        let cached = textures.stage("textures\\live.tga").unwrap();
        assert!(Arc::ptr_eq(&first, &cached));
        assert_eq!(reads.load(Ordering::SeqCst), 1);
        assert_eq!(textures.remaining_source_attempts, FOOTPRINT_SOURCES - 1);
        assert_eq!(textures.remaining_bytes, bytes - 30);
        assert_eq!(textures.remaining_cells, cells - 1);
    }

    #[test]
    fn failed_source_attempts_are_nonrefundable_and_exhaust_before_reads() {
        let (mut textures, reads) = textures_with_reads(&[
            ("bad-live.tga", vec![0; 17]),
            ("bad-dead.tga", vec![0; 17]),
            (
                "good.tga",
                texture(2, 2, &[[255; 3], [0; 3], [0; 3], [0; 3]]),
            ),
        ]);
        textures.remaining_source_attempts = 2;
        let bytes = textures.remaining_bytes;
        let cells = textures.remaining_cells;
        for path in ["bad-live.tga", "bad-dead.tga"] {
            assert!(textures.stage(path).is_err());
            assert!(textures.staged.is_empty());
            assert_eq!(textures.remaining_bytes, bytes);
            assert_eq!(textures.remaining_cells, cells);
        }
        assert_eq!(textures.remaining_source_attempts, 0);
        assert_eq!(reads.load(Ordering::SeqCst), 2);
        let error = textures.stage("good.tga").err().unwrap();
        assert!(error.contains("footprint source attempt budget"), "{error}");
        assert_eq!(reads.load(Ordering::SeqCst), 2);
        assert_eq!(textures.remaining_bytes, bytes);
        assert_eq!(textures.remaining_cells, cells);
        assert_eq!(textures.remaining_source_attempts, 0);
    }

    #[test]
    fn failed_attempt_preserves_capacity_while_rollback_keeps_it_spent() {
        let (mut textures, reads) = textures_with_reads(&[
            ("bad.tga", vec![0; 17]),
            (
                "good.tga",
                texture(2, 2, &[[255; 3], [0; 3], [0; 3], [0; 3]]),
            ),
        ]);
        textures.remaining_source_attempts = 2;
        let bytes = textures.remaining_bytes;
        let cells = textures.remaining_cells;
        assert!(textures.stage("bad.tga").is_err());
        assert_eq!(textures.remaining_source_attempts, 1);
        assert_eq!(reads.load(Ordering::SeqCst), 1);
        assert_eq!(textures.remaining_bytes, bytes);
        assert_eq!(textures.remaining_cells, cells);

        textures.stage("good.tga").unwrap();
        assert_eq!(textures.remaining_source_attempts, 0);
        assert_eq!(reads.load(Ordering::SeqCst), 2);
        assert!(textures.remaining_bytes < bytes);
        assert!(textures.remaining_cells < cells);
        textures.rollback(&["good.tga".into()]);
        assert_eq!(textures.remaining_bytes, bytes);
        assert_eq!(textures.remaining_cells, cells);
        assert_eq!(textures.remaining_source_attempts, 0);
        assert_eq!(reads.load(Ordering::SeqCst), 2);
        let error = textures.stage("good.tga").err().unwrap();
        assert!(error.contains("footprint source attempt budget"), "{error}");
        assert_eq!(reads.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn footprint_raster_follows_the_red_channel_bit_and_rejects_partial_data() {
        let policy = crate::unit_placement::tests::rules(2, 0.0).texture_policy();
        let mut rgba = vec![0; 16];
        rgba[0] = 255;
        rgba[12] = 255;
        assert_eq!(raster(2, 2, &rgba, policy).unwrap(), [[-1, -1], [0, 0]]);
        // The tested bit is 2 in this fixture: another bit does not link, a value that has it does.
        rgba[4] = 1;
        assert_eq!(raster(2, 2, &rgba, policy).unwrap(), [[-1, -1], [0, 0]]);
        rgba[8] = 0x4a;
        assert_eq!(
            raster(2, 2, &rgba, policy).unwrap(),
            [[-1, -1], [0, -1], [0, 0]]
        );
        assert!(raster(2, 2, &rgba[..12], policy).is_err());
        assert!(raster(0, 2, &[], policy).is_err());
    }

    #[test]
    fn building_raster_uses_decoded_channel_flags_and_preserves_axes() {
        let policy = crate::unit_placement::tests::rules(2, 0.0).texture_policy();
        let rgba = [2, 0, 0, 0, 0, 1, 0, 255, 0, 0, 8, 0, 0, 0, 0, 255];
        assert_eq!(
            building_raster(2, 2, &rgba, policy).unwrap(),
            [[-1, -1], [0, -1]]
        );
        assert!(building_raster(2, 2, &rgba[..12], policy).is_err());
    }
}

//! Selected-source creation collision cache on the existing executed identities.
use crate::{
    map_admission::ParsedMapContext,
    playercolor_host::{
        DestructableCatalog, DestructableCreation, DestructableOrigin, DestructableState,
        DestructableStats, ObjectHost, PlacedDestructable, PlacedSource,
    },
    unit_placement::{Budget, Footprint, Grid, Placement, Rules, StationaryKey, StationaryStamp},
};
use pcx_assets::{AssetPath, Resolver};
use pcx_objects::editor::{
    FieldKind, FieldSource, Overlay, SlkId,
    defaults::{DefaultTables, Family, Unresolved},
};
use std::{collections::BTreeMap, sync::Arc};

const CELLS: usize = 4 << 20;
const FOOTPRINT_CELLS: usize = 1 << 16;
const STATIONARY_WORK: usize = 4 << 20;
const STATIONARY_CELLS: usize = 1 << 20;

/// Selected map members needed by placed objects: the `.doo` instances and the
/// object catalogs they resolve against.
#[derive(Clone)]
pub struct Catalog {
    objects: DestructableCatalog,
    doodads: Vec<pcx_map::Doodad>,
    destructables: Overlay,
    decorations: Overlay,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum PlacedRole {
    Automatic(DestructableStats),
    ScriptOnly,
    Decoration,
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
        let doodad_bytes = extract("war3map.doo")?.ok_or("missing placement doodads")?;
        let count = doodad_bytes
            .get(12..16)
            .ok_or("truncated placement doodad count")?;
        if u32::from_le_bytes(count.try_into().unwrap()) > 100000 {
            return Err("placement stationary count budget".into());
        }
        let doodads = pcx_map::Doodads::from_bytes(doodad_bytes).map_err(|e| e.to_string())?;
        if doodads.instances().len() > 100000 || !doodads.terrain_doodads().is_empty() {
            return Err("placement stationary count or unsupported terrain doodads".into());
        }
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
        let destructables = overlay("war3map.w3b")?;
        let decorations = overlay("war3map.w3d")?;
        Ok(Self {
            objects: DestructableCatalog::new(tables, destructables.clone(), decorations.clone()),
            doodads: doodads.instances().to_vec(),
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
}

/// Called immediately before the first executed unit, after stationary creations.
pub fn assemble(
    context: &Arc<ParsedMapContext>,
    map_bytes: &[u8],
    reader: Arc<Resolver>,
    tables: &DefaultTables,
    catalog: &Catalog,
    objects: &ObjectHost,
    rules: &Rules,
) -> Result<Placement, String> {
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
        cache: BTreeMap::new(),
        remaining_bytes: 16 << 20,
        remaining_cells: FOOTPRINT_CELLS,
    };
    let mut work = STATIONARY_WORK;
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
    }
    let mut layers = Vec::new();
    for (id, instance, state) in objects.destructables_with_handles() {
        let live = catalog.path(tables, instance.objectid.to_be_bytes(), false)?;
        let dead = catalog.path(tables, instance.objectid.to_be_bytes(), true)?;
        let key = match instance.origin {
            DestructableOrigin::Placed { creation_number } => {
                StationaryKey::Placed(creation_number)
            }
            DestructableOrigin::Script => StationaryKey::Destructable(id),
        };
        let live = footprint_cells(
            &grid,
            rules,
            &mut textures,
            &live,
            [instance.x, instance.y],
            f64::from(instance.face) / 90.0,
            &mut work,
        )?;
        let dead = footprint_cells(
            &grid,
            rules,
            &mut textures,
            &dead,
            [instance.x, instance.y],
            f64::from(instance.face) / 90.0,
            &mut work,
        )?;
        layers.push(StationaryStamp {
            key,
            live,
            dead,
            mask: rules.ground_mask(),
            active: !state.dead,
        });
    }
    let base = grid.pathing.clone();
    Placement::new(
        grid,
        rules.clone(),
        BTreeMap::new(),
        Budget {
            cells: CELLS,
            footprint_cells: FOOTPRINT_CELLS,
            stationary_cells: STATIONARY_CELLS,
            queries: 1 << 16,
        },
    )
    .and_then(|p| p.with_stationary(base, layers))
    .map(|p| p.with_footprint_loader(move |path| textures.read(path).map(|f| f.as_ref().clone())))
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

struct Textures {
    reader: Arc<Resolver>,
    edition: String,
    cache: BTreeMap<String, Arc<Footprint>>,
    remaining_bytes: usize,
    remaining_cells: usize,
}

impl Textures {
    fn read(&mut self, path: &str) -> Result<Arc<Footprint>, String> {
        if path.len() > 16 << 10 {
            return Err("footprint path budget".into());
        }
        let key = AssetPath::new(path)
            .map_err(|e| e.to_string())?
            .as_str()
            .to_owned();
        if let Some(cached) = self.cache.get(&key) {
            return Ok(Arc::clone(cached));
        }
        if self.cache.len() >= 256 {
            return Err("footprint source count budget".into());
        }
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
            || header.width % 2 != 0
            || header.height % 2 != 0
        {
            return Err(format!(
                "{path}: footprint cell budget or unsupported empty/odd dimensions"
            ));
        }
        let rgba = pcx_tga::decode_with(&asset.bytes, &header).map_err(|e| e.to_string())?;
        let cells = raster(header.width, header.height, &rgba)?;
        let remaining_cells = self
            .remaining_cells
            .checked_sub(cells.len())
            .ok_or("footprint aggregate cell budget")?;
        self.remaining_bytes = self
            .remaining_bytes
            .checked_sub(asset.bytes.len())
            .ok_or("footprint source byte budget")?;
        self.remaining_cells = remaining_cells;
        let result = Arc::new(Footprint { origin, cells });
        self.cache.insert(key, Arc::clone(&result));
        Ok(result)
    }
}

fn raster(width: u16, height: u16, rgba: &[u8]) -> Result<Vec<[i32; 2]>, String> {
    let count = usize::from(width) * usize::from(height);
    if width == 0 || height == 0 || count > FOOTPRINT_CELLS || rgba.len() != count * 4 {
        return Err("footprint raster size".into());
    }
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| "footprint cell allocation")?;
    for (i, pixel) in rgba.as_chunks::<4>().0.iter().enumerate() {
        match pixel[0] {
            0 => {}
            255 => result.push([
                (i / usize::from(width)) as i32 - i32::from(height) / 2,
                (i % usize::from(width)) as i32 - i32::from(width) / 2,
            ]),
            _ => return Err("unsupported nonbinary footprint walk channel".into()),
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
    let cell: [f64; 2] = std::array::from_fn(|axis| {
        ((f64::from(point[axis]) - f64::from(grid.origin[axis])) / f64::from(rules.cell_size()))
            .round_ties_even()
    });
    if cell.iter().any(|v| v.abs() > f64::from(i32::MAX)) {
        return Err("stationary cell range".into());
    }
    // shortcut: nearest quarter-turn static raster, validated for the retained creation stream; qualify arbitrary-angle rasterization before widening fidelity claims.
    let quarter = turns.round_ties_even().rem_euclid(4.0) as u8;
    let mut cells = Vec::new();
    cells
        .try_reserve_exact(footprint.cells.len())
        .map_err(|_| "stationary cell allocation")?;
    for &[x, y] in &footprint.cells {
        let (x, y) = match quarter {
            0 => (i64::from(x), i64::from(y)),
            1 => (-i64::from(y) - 1, i64::from(x)),
            2 => (-i64::from(x) - 1, -i64::from(y) - 1),
            _ => (i64::from(y), -i64::from(x) - 1),
        };
        let (x, y) = (cell[0] as i64 + x, cell[1] as i64 + y);
        if x >= 0
            && y >= 0
            && x < i64::from(grid.pathing.width)
            && y < i64::from(grid.pathing.height)
        {
            cells.push(y as usize * grid.pathing.width as usize + x as usize);
        }
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
) -> Result<(), String> {
    let cells = footprint_cells(grid, rules, textures, path, point, turns, work)?;
    for i in cells {
        grid.pathing.cells[i] |= rules.ground_mask();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
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
            &["DestructableID", "HP", "targType"],
            &[&["LTlt", max_life, "tree,ground"]],
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
            destructables: Overlay::default(),
            decorations: Overlay::default(),
        }
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

    #[test]
    fn footprint_raster_uses_authored_pixels_and_rejects_partial_or_nonbinary_data() {
        let mut rgba = vec![0; 16];
        rgba[0] = 255;
        rgba[12] = 255;
        assert_eq!(raster(2, 2, &rgba).unwrap(), [[-1, -1], [0, 0]]);
        rgba[4] = 1;
        assert!(raster(2, 2, &rgba).is_err());
        assert!(raster(2, 2, &rgba[..12]).is_err());
        assert!(raster(0, 2, &[]).is_err());
    }
}

//! Water presence, level, and surface quads.
//!
//! Presence and the height word follow WarsmashModEngine `Corner.load` at
//! commit `f9e0aeed4be372d6016519d0e97b384aa873f374`: the water flag is
//! `textureAndFlags & 0b01000000`, and the height index is
//! `waterAndEdge & 0x3FFF`. This parser already stores that flag in the high
//! nibble of [`pcx_map::Corner::corner_flags`], so the flag constant is the
//! same mask shifted down by 4. The 14-bit index uses `world_z`
//! (the measured ground scale). Warsmash divides the same index by 512 and
//! later multiplies by 128; those are one scale, equal to `world_z`, and are
//! not a second conversion.
//!
//! The `height` column of the caller-selected `Water.slk` row is added in that
//! same Warsmash height unit. `SoftwareWaterAndGroundMesh` multiplies
//! `(waterHeight + heightOffset)` by 128, so the column becomes
//! `parsed_height * TILE_XY` in this world. The row is not a fixed Human01
//! record: the caller passes a `waterID`, or the W3E tileset byte
//! ([`pcx_map::Terrain::default_slot`]) plus the Warsmash suffix `Sha`.
//! A missing row or an unparsable `height` is an error. Texture frames,
//! colours, opacity, and the animation clock remain available as borrowed raw
//! cells; this module does not interpret them.
//!
//! This is a recreation of that loader. It is not a claim that a native
//! function was traced.

use pcx_map::{CORNER_RECORD, Terrain};
use pcx_slq::Table;

use crate::world::{TILE_XY, world_z};

/// Warsmash `textureAndFlags & 0b01000000`, after `pcx_map` stores
/// `(byte >> 4) & 0x0F` in [`pcx_map::Corner::corner_flags`].
const WATER_FLAG: u8 = 0b0100;
/// Warsmash `waterAndEdge & 0x3FFF`. Bit 14 is the map-edge flag
/// ([`pcx_map::Corner::position_flag`]) and is not part of the height index.
const WATER_INDEX_MASK: u16 = 0x3FFF;

/// Caller-declared original input identity, alongside its parsed format version.
/// Archive adjacency alone does not establish an edition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaterProvenance<'a> {
    pub edition: &'a str,
    /// Map path/member containing `war3map.w3e`.
    pub map_source: &'a str,
    pub terrain_version: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("water input requires explicit {0} provenance")]
pub struct MissingWaterProvenance(pub &'static str);

/// One original seven-byte record, without losing opaque or reserved bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawWaterCorner<'a> {
    /// Offset within the original `war3map.w3e` payload.
    pub byte_offset: usize,
    pub record: &'a [u8; CORNER_RECORD],
    /// The existing map parser's interpretation; not a water classification.
    pub corner: &'a pcx_map::Corner,
    pub provenance: WaterProvenance<'a>,
}

/// How the caller chooses one `Water.slk` data row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaterSelection<'a> {
    /// Match this `waterID` cell exactly.
    Explicit(&'a str),
    /// Warsmash `tileset + "Sha"` from the W3E tileset byte.
    TilesetSha,
}

/// Caps checked before water mesh storage is allocated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaterBudget {
    pub max_vertices: u64,
    pub max_indices: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WaterError {
    #[error("water profile source is empty")]
    EmptySource,
    #[error("water id is empty")]
    EmptyId,
    #[error("water id cannot be derived from tileset byte {byte}")]
    TilesetByte { byte: u8 },
    #[error("water profile row {water_id} is not in {table}")]
    MissingRow { water_id: String, table: String },
    #[error("water profile {water_id} matches more than one row in {table}")]
    Ambiguous { water_id: String, table: String },
    #[error("water profile {water_id} has no finite {field} in {table}")]
    MissingField {
        water_id: String,
        field: &'static str,
        table: String,
    },
    #[error("water {what} count {needed} exceeds caller budget {limit}")]
    Budget {
        what: &'static str,
        needed: u64,
        limit: u64,
    },
    #[error("water position is not finite")]
    NonFinite,
}

/// The selected `Water.slk` row. `height_offset` is in world units.
#[derive(Debug, Clone, Copy)]
pub struct WaterProfile<'a> {
    water_id: &'a str,
    row: u32,
    height_offset: f32,
    table: &'a Table,
    source: &'a str,
}

impl<'a> WaterProfile<'a> {
    /// Select one row from a parsed `Water.slk`. `source` is the caller-supplied
    /// archive member or path for that table.
    pub fn open(
        table: &'a Table,
        source: &'a str,
        selection: WaterSelection<'_>,
        terrain: &Terrain,
    ) -> Result<Self, WaterError> {
        if source.trim().is_empty() {
            return Err(WaterError::EmptySource);
        }
        let water_id = match selection {
            WaterSelection::Explicit(id) => {
                if id.is_empty() {
                    return Err(WaterError::EmptyId);
                }
                id.to_string()
            }
            WaterSelection::TilesetSha => {
                let byte = terrain.default_slot();
                if !byte.is_ascii_alphanumeric() {
                    return Err(WaterError::TilesetByte { byte });
                }
                format!("{}Sha", char::from(byte))
            }
        };
        let mut found = None;
        for row in table.data_rows() {
            if row.by_name("waterID") == Some(water_id.as_str()) {
                if found.is_some() {
                    return Err(WaterError::Ambiguous {
                        water_id,
                        table: source.to_string(),
                    });
                }
                found = Some(row.index());
            }
        }
        let Some(row) = found else {
            return Err(WaterError::MissingRow {
                water_id,
                table: source.to_string(),
            });
        };
        let id_cell = table
            .by_name(row, "waterID")
            .ok_or_else(|| WaterError::MissingRow {
                water_id: water_id.clone(),
                table: source.to_string(),
            })?;
        let height_text = table
            .by_name(row, "height")
            .ok_or_else(|| WaterError::MissingField {
                water_id: water_id.clone(),
                field: "height",
                table: source.to_string(),
            })?;
        let height: f32 = height_text.parse().map_err(|_| WaterError::MissingField {
            water_id: water_id.clone(),
            field: "height",
            table: source.to_string(),
        })?;
        let height_offset = height * TILE_XY;
        if !height.is_finite() || !height_offset.is_finite() {
            return Err(WaterError::MissingField {
                water_id: water_id.clone(),
                field: "height",
                table: source.to_string(),
            });
        }
        Ok(Self {
            water_id: id_cell,
            row,
            height_offset,
            table,
            source,
        })
    }

    #[must_use]
    pub fn water_id(&self) -> &'a str {
        self.water_id
    }

    #[must_use]
    pub fn height_offset(&self) -> f32 {
        self.height_offset
    }

    #[must_use]
    pub fn source(&self) -> &'a str {
        self.source
    }

    /// One raw cell from the selected row, when that header exists.
    #[must_use]
    pub fn field(&self, name: &str) -> Option<&'a str> {
        self.table.by_name(self.row, name)
    }

    /// Header/value pairs for the selected row, in column order.
    pub fn fields(&self) -> impl Iterator<Item = (&'a str, &'a str)> {
        let table = self.table;
        let row = self.row;
        table.row_cells(row).filter_map(move |(column, value)| {
            table.header(column).map(|header| (header, value.as_str()))
        })
    }
}

/// A provenance-bearing view of the inputs needed to build water.
#[derive(Debug, Clone, Copy)]
pub struct WaterData<'a> {
    terrain: &'a Terrain,
    provenance: WaterProvenance<'a>,
}

impl<'a> WaterData<'a> {
    pub fn new(
        terrain: &'a Terrain,
        edition: &'a str,
        map_source: &'a str,
    ) -> Result<Self, MissingWaterProvenance> {
        for (name, value) in [("edition", edition), ("map source", map_source)] {
            if value.trim().is_empty() {
                return Err(MissingWaterProvenance(name));
            }
        }
        Ok(Self {
            terrain,
            provenance: WaterProvenance {
                edition,
                map_source,
                terrain_version: terrain.version(),
            },
        })
    }

    #[must_use]
    pub fn provenance(&self) -> WaterProvenance<'a> {
        self.provenance
    }

    /// Original record for an in-range corner. Framing comes from the map
    /// parser's exact consumption of the seven-byte row-major corner table.
    #[must_use]
    pub fn raw_corner(&self, cx: u32, cy: u32) -> Option<RawWaterCorner<'a>> {
        let corner = self.terrain.corner(cx, cy)?;
        let stride = (self.terrain.width() as usize).checked_add(1)?;
        let ordinal = (cy as usize)
            .checked_mul(stride)?
            .checked_add(cx as usize)?;
        let body_len = self.terrain.corner_count().checked_mul(CORNER_RECORD)?;
        let body_offset = self.terrain.raw().len().checked_sub(body_len)?;
        let byte_offset = body_offset.checked_add(ordinal.checked_mul(CORNER_RECORD)?)?;
        let end = byte_offset.checked_add(CORNER_RECORD)?;
        let record = self.terrain.raw().get(byte_offset..end)?.try_into().ok()?;
        Some(RawWaterCorner {
            byte_offset,
            record,
            corner,
            provenance: self.provenance,
        })
    }

    /// Whether the Warsmash water flag is set. `None` is outside the corner grid.
    #[must_use]
    pub fn water_present(&self, cx: u32, cy: u32) -> Option<bool> {
        Some(self.terrain.corner(cx, cy)?.corner_flags & WATER_FLAG != 0)
    }

    /// Stored water level in world units, including the selected row's height
    /// offset. This is the second-word index, not a claim that the corner is
    /// wet. `Ok(None)` means outside the corner grid.
    pub fn water_level(
        &self,
        cx: u32,
        cy: u32,
        profile: &WaterProfile<'_>,
    ) -> Result<Option<f32>, WaterError> {
        let Some(corner) = self.terrain.corner(cx, cy) else {
            return Ok(None);
        };
        Ok(Some(level(corner, profile)?))
    }

    /// One quad per tile that has at least one water flag. Dry terrain is an
    /// empty mesh, not an error and not a stand-in plane. Each vertex of a wet
    /// tile uses that corner's stored water level.
    pub fn mesh(
        &self,
        profile: &WaterProfile<'_>,
        budget: &WaterBudget,
    ) -> Result<WaterMesh, WaterError> {
        let mut wet = 0u64;
        for ty in 0..self.terrain.height() {
            for tx in 0..self.terrain.width() {
                if tile_wet(self.terrain, tx, ty) {
                    wet = wet.checked_add(1).ok_or(WaterError::Budget {
                        what: "tiles",
                        needed: u64::MAX,
                        limit: budget.max_vertices,
                    })?;
                }
            }
        }
        let vertices = wet.checked_mul(4).ok_or(WaterError::Budget {
            what: "vertices",
            needed: u64::MAX,
            limit: budget.max_vertices,
        })?;
        let indices = wet.checked_mul(6).ok_or(WaterError::Budget {
            what: "indices",
            needed: u64::MAX,
            limit: budget.max_indices,
        })?;
        if vertices > budget.max_vertices {
            return Err(WaterError::Budget {
                what: "vertices",
                needed: vertices,
                limit: budget.max_vertices,
            });
        }
        if indices > budget.max_indices {
            return Err(WaterError::Budget {
                what: "indices",
                needed: indices,
                limit: budget.max_indices,
            });
        }
        let vertex_count = usize::try_from(vertices).map_err(|_| WaterError::Budget {
            what: "vertices",
            needed: vertices,
            limit: budget.max_vertices,
        })?;
        let index_count = usize::try_from(indices).map_err(|_| WaterError::Budget {
            what: "indices",
            needed: indices,
            limit: budget.max_indices,
        })?;
        let mut positions = Vec::new();
        let mut uvs = Vec::new();
        let mut mesh_indices = Vec::new();
        positions
            .try_reserve_exact(vertex_count)
            .map_err(|_| WaterError::Budget {
                what: "vertices",
                needed: vertices,
                limit: budget.max_vertices,
            })?;
        uvs.try_reserve_exact(vertex_count)
            .map_err(|_| WaterError::Budget {
                what: "vertices",
                needed: vertices,
                limit: budget.max_vertices,
            })?;
        mesh_indices
            .try_reserve_exact(index_count)
            .map_err(|_| WaterError::Budget {
                what: "indices",
                needed: indices,
                limit: budget.max_indices,
            })?;
        let (origin_x, origin_y) = self.terrain.origin();
        for ty in 0..self.terrain.height() {
            for tx in 0..self.terrain.width() {
                if !tile_wet(self.terrain, tx, ty) {
                    continue;
                }
                let base = u32::try_from(positions.len()).map_err(|_| WaterError::Budget {
                    what: "vertices",
                    needed: vertices,
                    limit: budget.max_vertices,
                })?;
                for (cx, cy) in [(tx, ty), (tx + 1, ty), (tx, ty + 1), (tx + 1, ty + 1)] {
                    let corner = self.terrain.corner(cx, cy).ok_or(WaterError::NonFinite)?;
                    let z = level(corner, profile)?;
                    let x = origin_x + cx as f32 * TILE_XY;
                    let y = origin_y + cy as f32 * TILE_XY;
                    if !x.is_finite() || !y.is_finite() {
                        return Err(WaterError::NonFinite);
                    }
                    positions.push([x, y, z]);
                    // Same grid uv as TerrainMesh: one repeat per tile.
                    uvs.push([cx as f32, cy as f32]);
                }
                // TerrainMesh winding `(a, b, d)`, `(a, d, c)` with
                // `a` bottom-left, `b` bottom-right, `c` top-left, `d` top-right.
                mesh_indices.extend([base, base + 1, base + 3, base, base + 3, base + 2]);
            }
        }
        let wet_tiles = u32::try_from(wet).map_err(|_| WaterError::Budget {
            what: "tiles",
            needed: wet,
            limit: budget.max_vertices,
        })?;
        Ok(WaterMesh {
            positions,
            uvs,
            indices: mesh_indices,
            wet_tiles,
        })
    }
}

fn tile_wet(terrain: &Terrain, tx: u32, ty: u32) -> bool {
    [(tx, ty), (tx + 1, ty), (tx, ty + 1), (tx + 1, ty + 1)]
        .into_iter()
        .any(|(cx, cy)| {
            terrain
                .corner(cx, cy)
                .is_some_and(|corner| corner.corner_flags & WATER_FLAG != 0)
        })
}

fn level(corner: &pcx_map::Corner, profile: &WaterProfile<'_>) -> Result<f32, WaterError> {
    let z = world_z(corner.position_word & WATER_INDEX_MASK) + profile.height_offset;
    if z.is_finite() {
        Ok(z)
    } else {
        Err(WaterError::NonFinite)
    }
}

/// Water quads in terrain-file order. Empty when no tile has the water flag.
#[derive(Debug, Clone, PartialEq)]
pub struct WaterMesh {
    pub positions: Vec<[f32; 3]>,
    /// Grid uv, `u = cx`, `v = cy`, matching the ground mesh convention.
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
    pub wet_tiles: u32,
}

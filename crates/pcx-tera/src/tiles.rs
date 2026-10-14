//! Join a ground slot fourcc to its shipped BLP, through
//! `TerrainArt\Terrain.slk`.
//!
//! The shipped table's `tileID` column is exactly the w3e slot fourcc, so this
//! is the join that turns a nibble into a texture. The table also carries
//! `cliffSet`, `dir`, `file`, `buildable`, `footprints`, `walkable`, `flyable`,
//! `blightPri`, `convertTo`, `InBeta` and `version`; this crate reads the
//! identity columns it can *verify* against an archive (`tileID`, `dir`, `file`)
//! plus `cliffSet`, which is the measured negative that refuses the cliff rule
//! (see [`crate::cliff`]: no shipped map selects a cliff-set tile).
//!
//! The rest of the columns are **not** interpreted. `walkable` in particular is
//! a per-*tile-type* flag, and the game's pathing is its own; see
//! [`crate::Unsupported::Walkability`].

use crate::Error;

/// Errors from resolving a slot fourcc to its shipped BLP.
#[derive(Debug, thiserror::Error)]
pub enum TilesetError {
    #[error("terrain slk: {0}")]
    Slk(#[from] pcx_slq::Error),
    #[error("terrain slk has no column {column:?}; the shipped table always does")]
    MissingColumn { column: &'static str },
}

/// One row of `TerrainArt\Terrain.slk` this crate resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileEntry {
    /// The `tileID` fourcc.
    pub fourcc: [u8; 4],
    /// Directory of the shipped texture, from the `dir` column.
    pub dir: String,
    /// File base name, from the `file` column; the texture is
    /// `<dir>\<file>.blp`.
    pub file: String,
    /// The `cliffSet` column, raw. `-1` on every tile a shipped map selects;
    /// see [`crate::cliff`] for what that refutes.
    pub cliff_set: i64,
}

impl TileEntry {
    /// The shipped archive path of this tile's texture.
    #[must_use]
    pub fn blp_path(&self) -> String {
        format!("{}\\{}.blp", self.dir, self.file)
    }

    /// `true` when the shipped table puts this tile in a cliff set.
    ///
    /// Never `true` for a slot any shipped map selects, which is the measured
    /// fact; the predicate exists so a sweep can count it.
    #[must_use]
    pub fn is_cliff_tile(&self) -> bool {
        self.cliff_set != -1
    }
}

/// A tileset catalogue: `tileID` fourcc → shipped texture.
///
/// Deterministic: the entries are kept in a [`std::collections::BTreeMap`], so
/// iteration order is the fourcc order, never a hash order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TileCatalog {
    entries: std::collections::BTreeMap<[u8; 4], TileEntry>,
}

impl TileCatalog {
    /// Parse a `TerrainArt\Terrain.slk`.
    ///
    /// # Errors
    ///
    /// [`crate::Error::Tileset`] when the table cannot be parsed or lacks one of
    /// the three identity columns, which the shipped table always has.
    pub fn from_terrain_slk(bytes: &[u8]) -> Result<Self, Error> {
        let table = pcx_slq::Table::parse(bytes)?;
        for column in ["tileID", "dir", "file"] {
            if table.column_of(column).is_none() {
                return Err(Error::Tileset(TilesetError::MissingColumn { column }));
            }
        }
        let mut entries = std::collections::BTreeMap::new();
        for row in table.data_rows() {
            let Some(four) = fourcc(row.by_name("tileID").unwrap_or("")) else {
                continue;
            };
            let file = row.by_name("file").unwrap_or("");
            if file.is_empty() {
                continue;
            }
            let entry = TileEntry {
                fourcc: four,
                dir: row.by_name("dir").unwrap_or("").to_string(),
                file: file.to_string(),
                cliff_set: row
                    .by_name("cliffSet")
                    .unwrap_or("-1")
                    .trim()
                    .parse::<i64>()
                    .unwrap_or(-1),
            };
            entries.insert(four, entry);
        }
        Ok(Self { entries })
    }

    /// The entry for a slot fourcc, or `None` when the shipped table does not
    /// name it — a cliff accent, or a fourcc outside the table.
    #[must_use]
    pub fn entry(&self, slot: [u8; 4]) -> Option<&TileEntry> {
        self.entries.get(&slot)
    }

    /// The shipped BLP path for a slot fourcc, or `None`.
    #[must_use]
    pub fn blp_path(&self, slot: [u8; 4]) -> Option<String> {
        self.entry(slot).map(TileEntry::blp_path)
    }

    /// The path for each of a terrain's ground slots, in file order.
    ///
    /// A `None` entry is a slot the shipped table does not name, reported
    /// rather than fabricated.
    #[must_use]
    pub fn resolve_terrain(&self, terrain: &pcx_map::Terrain) -> Vec<Option<String>> {
        terrain.slots().iter().map(|s| self.blp_path(*s)).collect()
    }

    /// Entries whose `cliffSet` is separate from `-1`.
    ///
    /// The count of these, and the fact that no shipped corner selects one, is
    /// what refuses the "slot change ⇒ cliff" rule.
    #[must_use]
    pub fn cliff_tiles(&self) -> Vec<&TileEntry> {
        self.entries
            .values()
            .filter(|e| e.is_cliff_tile())
            .collect()
    }

    /// Number of resolved tiles.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// `true` when the catalogue resolved nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// All entries, in fourcc order.
    pub fn entries(&self) -> impl Iterator<Item = &TileEntry> {
        self.entries.values()
    }
}

/// A four-character `tileID`. Longer or shorter values are not tile ids.
fn fourcc(s: &str) -> Option<[u8; 4]> {
    let b = s.as_bytes();
    (b.len() == 4).then(|| [b[0], b[1], b[2], b[3]])
}

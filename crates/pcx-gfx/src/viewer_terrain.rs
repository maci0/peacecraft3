//! Explicit terrain texture selection for the model viewer.
use std::path::Path;

const TERRAIN_SLK: &str = "TerrainArt\\Terrain.slk";

#[derive(Debug, thiserror::Error)]
#[error("{source_name}: tile {tile_id:?}: {kind}")]
pub struct TerrainTileError {
    pub source_name: String,
    pub tile_id: [u8; 4],
    #[source]
    pub kind: TerrainTileErrorKind,
}

#[derive(Debug, thiserror::Error)]
pub enum TerrainTileErrorKind {
    #[error("archive read: {0}")]
    Read(#[source] pcx_mpq::Error),
    #[error("table parse: {0}")]
    Parse(#[source] pcx_slq::Error),
    #[error("missing column {0:?}")]
    MissingColumn(&'static str),
    #[error("multiple columns named {0:?}")]
    AmbiguousColumn(&'static str),
    #[error("no matching tile row")]
    UnknownTile,
    #[error("multiple matching tile rows: {first}, {second}")]
    AmbiguousTile { first: u32, second: u32 },
    #[error("row {row}: missing or empty {column:?}")]
    MissingValue { row: u32, column: &'static str },
    #[error("table anomaly {kind:?} at row {row:?}, column {column:?}")]
    TableAnomaly {
        kind: pcx_slq::AnomalyKind,
        row: Option<u32>,
        column: Option<u32>,
    },
    #[error("texture decode: {0}")]
    Decode(#[source] Box<crate::Error>),
}

fn failure(source_name: &str, tile_id: [u8; 4], kind: TerrainTileErrorKind) -> TerrainTileError {
    TerrainTileError {
        source_name: source_name.to_owned(),
        tile_id,
        kind,
    }
}

/// Resolves exactly one caller-selected tileID using its authored dir/file.
pub fn terrain_tile_path_from_slk(
    bytes: &[u8],
    tile_id: [u8; 4],
) -> Result<String, TerrainTileError> {
    path_from_slk(bytes, tile_id, TERRAIN_SLK)
}

fn path_from_slk(
    bytes: &[u8],
    tile_id: [u8; 4],
    source_name: &str,
) -> Result<String, TerrainTileError> {
    let fail = |kind| failure(source_name, tile_id, kind);
    let table = pcx_slq::Table::parse(bytes).map_err(|e| fail(TerrainTileErrorKind::Parse(e)))?;
    for column in ["tileID", "dir", "file"] {
        let count = table.headers().filter(|(_, name)| *name == column).count();
        match count {
            0 => return Err(fail(TerrainTileErrorKind::MissingColumn(column))),
            1 => (),
            _ => return Err(fail(TerrainTileErrorKind::AmbiguousColumn(column))),
        }
    }
    if let Some(anomaly) = table.anomalies().iter().find(|a| {
        a.severity == pcx_slq::Severity::Error || a.kind == pcx_slq::AnomalyKind::StrayDataAfterEnd
    }) {
        return Err(fail(TerrainTileErrorKind::TableAnomaly {
            kind: anomaly.kind,
            row: anomaly.row,
            column: anomaly.column,
        }));
    }
    let mut rows = table.data_rows().filter(|row| {
        row.by_name("tileID")
            .is_some_and(|id| id.as_bytes() == tile_id)
    });
    let row = rows
        .next()
        .ok_or_else(|| fail(TerrainTileErrorKind::UnknownTile))?;
    if let Some(second) = rows.next() {
        return Err(fail(TerrainTileErrorKind::AmbiguousTile {
            first: row.index(),
            second: second.index(),
        }));
    }
    let value = |column| {
        row.by_name(column)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                fail(TerrainTileErrorKind::MissingValue {
                    row: row.index(),
                    column,
                })
            })
    };
    Ok(format!("{}\\{}.blp", value("dir")?, value("file")?))
}

/// Resolves a caller-selected tile from the supplied installation's table.
pub fn terrain_tile_path(install: &Path, tile_id: [u8; 4]) -> Result<String, TerrainTileError> {
    let source_name = format!("{}::{TERRAIN_SLK}", install.display());
    let mut set = pcx_mpq::MpqSet::open_wc3(install)
        .map_err(|e| failure(&source_name, tile_id, TerrainTileErrorKind::Read(e)))?;
    let bytes = set
        .extract(TERRAIN_SLK)
        .map_err(|e| failure(&source_name, tile_id, TerrainTileErrorKind::Read(e)))?;
    path_from_slk(&bytes, tile_id, &source_name)
}

pub(super) fn load_terrain_tile(
    install: &Path,
    tile_id: [u8; 4],
) -> Result<crate::LoadedTexture, TerrainTileError> {
    let install_name = install.display().to_string();
    let mut set = pcx_mpq::MpqSet::open_wc3(install)
        .map_err(|e| failure(&install_name, tile_id, TerrainTileErrorKind::Read(e)))?;
    load_from(&install_name, tile_id, |path| set.extract(path))
}

fn load_from(
    install_name: &str,
    tile_id: [u8; 4],
    mut extract: impl FnMut(&str) -> Result<Vec<u8>, pcx_mpq::Error>,
) -> Result<crate::LoadedTexture, TerrainTileError> {
    let table_source = format!("{install_name}::{TERRAIN_SLK}");
    let bytes = extract(TERRAIN_SLK)
        .map_err(|e| failure(&table_source, tile_id, TerrainTileErrorKind::Read(e)))?;
    let path = path_from_slk(&bytes, tile_id, &table_source)?;
    let texture_source = format!("{install_name}::{path}");
    let bytes = extract(&path)
        .map_err(|e| failure(&texture_source, tile_id, TerrainTileErrorKind::Read(e)))?;
    crate::load_image(&bytes, &path).map_err(|e| {
        failure(
            &texture_source,
            tile_id,
            TerrainTileErrorKind::Decode(Box::new(e)),
        )
    })
}

#[cfg(test)]
#[path = "viewer_terrain_tests.rs"]
mod tests;

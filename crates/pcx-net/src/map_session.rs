//! Map-admitted session start for native hosts and peers.
//!
//! The same install, executable, map bytes, and seed produce the same
//! [`World`] and [`SessionIdentity`] on every peer. Peers compare the identity
//! before play, so a peer that admitted a different map, executable, profile or
//! seed refuses to join.
//!
//! Every game value comes from the supplied bytes through the existing readers.
//! The one constant here is the W3E terrain tile edge, a format invariant.

use std::{collections::BTreeMap, path::Path};

use pcx_map::Map;
use pcx_mpq::checked::ReadBudget;
use pcx_objects::{
    ObjectFile,
    editor::{Editor, EditorReadLimits, FieldSource, ObjectKind, Overlay, SlkId},
};
use pcx_play::{campaign_client, scripts::AssetIdentity};
use pcx_sim::{
    FX_ONE, Fx, MovementProfile, World, admit_aligned_movement,
    world::{
        Definition, FieldValue, ImportError, ImportedWorld, MetadataCatalog, RecordProblem,
        ResolutionProblem, Source,
    },
};
use sha2::{Digest, Sha256};

use crate::{
    encode_world_payload,
    session::{COORDINATE_LIMIT, SessionIdentity},
};

/// World units per W3E terrain tile edge. This is a format invariant, not a map value.
pub const TERRAIN_TILE_WORLD_UNITS: Fx = 128;

/// Mobile units per session. A snapshot of this many units is at most 23 KiB, which fits the
/// host's 64 KiB write buffer with room for the header.
pub const MAX_SESSION_UNITS: usize = 1024;

const EXECUTABLE_NAME: &str = "Warcraft III.exe";
/// Placement rawcode of a start location. It has no unit definition, and the import keeps
/// it as a named missing rawcode (docs/re-notes/world-import.md).
const START_LOCATION_RAWCODE: &str = "sloc";
const MAP_READ_BUDGET: ReadBudget = ReadBudget {
    max_archive_bytes: 64 << 20,
    max_archive_table_bytes: 8 << 20,
    max_raw_packed_bytes: 16 << 20,
    max_decoded_bytes: 16 << 20,
};
const EDITOR_LIMITS: EditorReadLimits = EditorReadLimits {
    max_listed_paths: 0,
    max_asset_bytes: 8 << 20,
    max_aggregate_input_bytes: 64 << 20,
};

#[derive(Debug, thiserror::Error)]
pub enum AdmitError {
    #[error("install: {0}")]
    Install(#[from] pcx_mpq::Error),
    #[error("{path}: {source}")]
    Executable {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("placement client: {0}")]
    Client(String),
    #[error("map: {0}")]
    Map(String),
    #[error("catalog: {0}")]
    Catalog(String),
    #[error("world import: {0}")]
    Import(#[from] ImportError),
    #[error("movement admission: {0}")]
    Movement(String),
    #[error("unit record {ordinal}: {reason}")]
    Record { ordinal: usize, reason: String },
    #[error("more than {limit} mobile units")]
    TooManyUnits { limit: usize },
    #[error("coordinate {0} is outside the session range")]
    Coordinate(f32),
}

/// What admission kept and what it left out. Omitted records are counted, never hidden.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub units: usize,
    /// Records left out, by reason: `start-location`, `random-selection`, `non-unit` or
    /// `immobile`. Session peers agree on these counts through the identity.
    pub excluded: BTreeMap<&'static str, usize>,
}

#[derive(Debug)]
pub struct MapSession {
    pub world: World,
    pub seed: u64,
    pub identity: SessionIdentity,
    pub report: Report,
}

/// Admits `map_bytes` for play on the install at `install_dir`.
///
/// The executable must be the reviewed Legacy build; other builds fail in the
/// placement reader instead of guessing the movement mask.
pub fn admit(
    install_dir: &Path,
    map_bytes: &[u8],
    map_label: &str,
    seed: u64,
) -> Result<MapSession, AdmitError> {
    let mut install = pcx_mpq::Install::open(install_dir)?;
    let exe_path = install_dir.join(EXECUTABLE_NAME);
    let exe = std::fs::read(&exe_path).map_err(|source| AdmitError::Executable {
        path: exe_path.display().to_string(),
        source,
    })?;
    let exe_digest: [u8; 32] = Sha256::digest(&exe).into();
    let provenance = AssetIdentity {
        edition: install.version.to_string(),
        version: hex(&exe_digest),
        source: install_dir.display().to_string(),
    };
    let rules =
        campaign_client::placement_rules(&exe, provenance.clone()).map_err(AdmitError::Client)?;

    let editor = Editor::from_reader(install_dir, &[], EDITOR_LIMITS, |member| {
        match install.extract(member) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(pcx_mpq::Error::NotFound(_)) => Ok(None),
            Err(error) => Err(error),
        }
    })
    .map_err(|error| AdmitError::Catalog(error.to_string()))?;

    let map = Map::parse(map_bytes).map_err(|error| AdmitError::Map(error.to_string()))?;
    let overlay = |member: &'static str| -> Result<Overlay, AdmitError> {
        let raw = map
            .extract_checked(map_label, member, MAP_READ_BUDGET)
            .map_err(|error| AdmitError::Map(error.to_string()))?;
        let Some(raw) = raw else {
            return Ok(Overlay::default());
        };
        let file =
            ObjectFile::parse(member, &raw).map_err(|error| AdmitError::Map(error.to_string()))?;
        Ok(Overlay::of(&file))
    };
    let units = overlay("war3map.w3u")?;
    let items = overlay("war3map.w3t")?;
    let catalog = MetadataCatalog {
        editor: &editor,
        units: &units,
        items: &items,
    };
    let source = Source {
        install: install_dir.display().to_string(),
        edition: install.version.to_string(),
        map: map_label.to_string(),
    };
    let (imported, alignment) = ImportedWorld::from_map_aligned(&map, source, &catalog, &[])?;

    let terrain = map
        .terrain()
        .map_err(|error| AdmitError::Map(error.to_string()))?;
    let (origin_x, origin_y) = terrain.origin();
    let profile = MovementProfile {
        origin_x: fixed(origin_x)?,
        origin_y: fixed(origin_y)?,
        terrain_tile_fx: TERRAIN_TILE_WORLD_UNITS * FX_ONE,
        blocked_bits: rules.ground_mask(),
    };
    let movement = admit_aligned_movement(
        &imported.pathing,
        &alignment,
        profile,
        imported.pathing.cells.len(),
    )
    .map_err(|error| AdmitError::Movement(error.to_string()))?;

    let mut world = World::empty();
    let mut report = Report::default();
    for record in &imported.records {
        let fail = |reason: String| AdmitError::Record {
            ordinal: record.ordinal,
            reason,
        };
        if let Some(reason) = excluded_record(&record.problems) {
            *report.excluded.entry(reason).or_default() += 1;
            continue;
        }
        if !record.problems.is_empty() {
            return Err(fail(format!("{:?}", record.problems)));
        }
        let definition = match &record.definition {
            Ok(index) => imported
                .definitions
                .get(*index)
                .ok_or_else(|| fail("definition index outside the import".into()))?,
            Err(problem) => return Err(fail(format!("{problem:?}"))),
        };
        if definition.kind != ObjectKind::Unit {
            *report.excluded.entry("non-unit").or_default() += 1;
            continue;
        }
        match movement_type(definition) {
            None => return Err(fail("movement type field is absent".into())),
            Some("" | "_" | "none") => {
                *report.excluded.entry("immobile").or_default() += 1;
                continue;
            }
            Some(_) => {}
        }
        let owner = record
            .owner
            .and_then(|owner| u8::try_from(owner).ok())
            .ok_or_else(|| fail(format!("owner {:?} is outside 0..=255", record.owner)))?;
        if world.units.len() >= MAX_SESSION_UNITS {
            return Err(AdmitError::TooManyUnits {
                limit: MAX_SESSION_UNITS,
            });
        }
        let [x, y, _] = record.placement.position;
        world.spawn(owner, fixed(x)?, fixed(y)?);
    }
    report.units = world.units.len();
    world.movement = Some(movement);

    let mut hash = Sha256::new();
    hash.update(b"pcx-session-identity-v1\n");
    hash.update(exe_digest);
    hash.update(Sha256::digest(map_bytes));
    hash.update(profile.origin_x.to_le_bytes());
    hash.update(profile.origin_y.to_le_bytes());
    hash.update(profile.terrain_tile_fx.to_le_bytes());
    hash.update([profile.blocked_bits]);
    hash.update(imported.pathing.width.to_le_bytes());
    hash.update(imported.pathing.height.to_le_bytes());
    hash.update(&imported.pathing.cells);
    hash.update(seed.to_le_bytes());
    for (reason, count) in &report.excluded {
        hash.update(reason.as_bytes());
        hash.update((*count as u64).to_le_bytes());
    }
    hash.update(encode_world_payload(&world));
    let identity = SessionIdentity(hash.finalize().into());

    Ok(MapSession {
        world,
        seed,
        identity,
        report,
    })
}

/// Why a record is left out before unit checks, or `None` when it must be admitted or refused.
///
/// A start location is not a unit. A random-selection word is the only problem a record
/// carries when the game picks its unit at creation, and that choice is not imported, so the
/// record is omitted and counted rather than guessed.
fn excluded_record(problems: &[RecordProblem]) -> Option<&'static str> {
    if problems.iter().any(|problem| {
        matches!(problem, RecordProblem::Definition(ResolutionProblem::MissingRawcode(raw)) if raw == START_LOCATION_RAWCODE)
    }) {
        return Some("start-location");
    }
    (!problems.is_empty()
        && problems
            .iter()
            .all(|problem| matches!(problem, RecordProblem::UnverifiedSelection(_))))
    .then_some("random-selection")
}

/// The `movetp` text of a unit. Structures and other immobile records use `_`, `none`
/// or empty text, as `unit_position_host::geometry` reads them.
fn movement_type(definition: &Definition) -> Option<&str> {
    definition
        .fields
        .values()
        .find_map(|field| match (&field.metadata, &field.value) {
            (Some(meta), Ok(value))
                if matches!(&meta.source, FieldSource::Column { slk, column }
                    if *slk == SlkId::UnitData && column == "movetp") =>
            {
                match value {
                    FieldValue::Cell(text) => Some(text.as_str()),
                    FieldValue::Override(pcx_objects::Value::Str(text)) => {
                        std::str::from_utf8(text).ok()
                    }
                    _ => None,
                }
            }
            _ => None,
        })
}

/// Converts an authored world coordinate to fixed point, rounding to the nearest unit.
fn fixed(value: f32) -> Result<Fx, AdmitError> {
    let scaled = (f64::from(value) * f64::from(FX_ONE)).round();
    if !scaled.is_finite() || scaled.abs() > f64::from(COORDINATE_LIMIT) {
        return Err(AdmitError::Coordinate(value));
    }
    Ok(scaled as Fx)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinates_round_to_the_nearest_fixed_point_unit() {
        assert_eq!(fixed(0.0).unwrap(), 0);
        assert_eq!(fixed(1.0).unwrap(), FX_ONE);
        assert_eq!(fixed(-0.5).unwrap(), -FX_ONE / 2);
        assert_eq!(fixed(1.0 / 65536.0 * 0.6).unwrap(), 1);
        assert_eq!(fixed(-1280.0).unwrap(), -1280 * FX_ONE);
    }

    #[test]
    fn non_finite_and_out_of_range_coordinates_are_refused() {
        for bad in [f32::NAN, f32::INFINITY, 16_001.0, -16_001.0] {
            assert!(
                matches!(fixed(bad), Err(AdmitError::Coordinate(_))),
                "accepted {bad}"
            );
        }
        assert!(fixed(16_000.0).is_ok());
    }
}

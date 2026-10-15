//! Parsed same-map context. Admission does not execute scripts or create game state.
use crate::map_scripts_import::{self, MapScriptBudget, MapScriptImportError};
use crate::scripts::{AssetIdentity, MapScripts, ScriptAsset, SourceDigest};
use pcx_map::{
    GridAlignment, GridAlignmentError, GridDimensions, Map, MapInfo, Pathing, Terrain, Units,
};
use pcx_mpq::checked::{CheckedArchive, ReadBudget, ReadError};
use pcx_sim::world::{ImportError, ImportedWorld, ObjectCatalog, Source};

use std::sync::Arc;

pub struct IdentifiedScript<'a> {
    pub identity: &'a AssetIdentity,
    pub asset: ScriptAsset<'a>,
}
pub struct MapAdmissionInputs<'a, C> {
    pub map: &'a Map<'a>,
    pub map_label: &'a str,
    pub map_identity: &'a AssetIdentity,
    pub common: IdentifiedScript<'a>,
    pub blizzard: IdentifiedScript<'a>,
    /// Caller-declared provenance; the catalog retains its own field sources.
    pub catalog_identity: &'a AssetIdentity,
    pub catalog: &'a C,
    pub required_fields: &'a [[u8; 4]],
}
/// Explicit input policy, not a native admission or total-memory guarantee.
#[derive(Clone, Copy)]
pub struct MapAdmissionBudget {
    pub scripts: MapScriptBudget,
    pub terrain_bytes: usize,
    pub info_bytes: usize,
    pub pathing_bytes: usize,
    pub units_bytes: usize,
    pub aggregate_input_bytes: usize,
    pub required_fields: usize,
}
#[derive(Debug, thiserror::Error)]
pub enum MapAdmissionError {
    #[error("{input} identity requires nonempty {field}")]
    Identity {
        input: &'static str,
        field: &'static str,
    },
    #[error("{map}: requested {actual} field bindings, limit is {limit}")]
    FieldBindings {
        map: String,
        actual: usize,
        limit: usize,
    },
    #[error(transparent)]
    Input(#[from] MapScriptImportError),
    #[error("{map}: required member {member} is absent")]
    MissingMember { map: String, member: &'static str },
    #[error("{member}: {source}")]
    Read {
        member: &'static str,
        #[source]
        source: ReadError,
    },
    #[error("{map}: parsing {member}: {source}")]
    Parse {
        map: String,
        member: &'static str,
        #[source]
        source: pcx_map::Error,
    },
    #[error("{map}: terrain {terrain:?}, pathing {pathing:?}: {source}")]
    Alignment {
        map: String,
        terrain: GridDimensions,
        pathing: GridDimensions,
        #[source]
        source: GridAlignmentError,
    },
    #[error(transparent)]
    World(#[from] ImportError),
}
pub struct MapInputSources {
    pub map: AssetIdentity,
    pub common: AssetIdentity,
    pub blizzard: AssetIdentity,
    pub catalog: AssetIdentity,
}
/// Owned parsed data only. World records may still carry explicit problems.
pub struct ParsedMapContext {
    pub(crate) admission_witness: Option<crate::neutral_player::AdmissionWitness>,
    pub sources: MapInputSources,
    pub required_fields: Vec<[u8; 4]>,
    pub input_budget: MapAdmissionBudget,
    pub map_digest: SourceDigest,
    pub world_member_digests: Vec<SourceDigest>,
    pub terrain: Terrain,
    pub alignment: GridAlignment,
    pub world: ImportedWorld,
    pub scripts: Arc<MapScripts>,
}
fn identity(input: &'static str, identity: &AssetIdentity) -> Result<(), MapAdmissionError> {
    for (field, value) in [
        ("edition", &identity.edition),
        ("version", &identity.version),
        ("source", &identity.source),
    ] {
        if value.is_empty() {
            return Err(MapAdmissionError::Identity { input, field });
        }
    }
    Ok(())
}
/// All inputs are supplied by the caller; every world member comes from this Map.
/// Catalog resolution and script compilation finish before the context is returned.
pub fn prepare_map_context<C: ObjectCatalog>(
    input: MapAdmissionInputs<'_, C>,
    budget: MapAdmissionBudget,
) -> Result<ParsedMapContext, MapAdmissionError> {
    for (role, source) in [
        ("map", input.map_identity),
        ("common", input.common.identity),
        ("blizzard", input.blizzard.identity),
        ("catalog", input.catalog_identity),
    ] {
        identity(role, source)?;
    }
    if input.map_label.is_empty() {
        return Err(MapAdmissionError::Identity {
            input: "map",
            field: "label",
        });
    }
    if input.required_fields.len() > budget.required_fields {
        return Err(MapAdmissionError::FieldBindings {
            map: input.map_label.into(),
            actual: input.required_fields.len(),
            limit: budget.required_fields,
        });
    }
    let label = input.map_label;
    let prelude_bytes = input
        .common
        .asset
        .bytes
        .len()
        .checked_add(input.blizzard.asset.bytes.len())
        .ok_or_else(|| MapScriptImportError::AggregatePreludeExceeded {
            map: label.into(),
            declared_bytes: usize::MAX,
            max_bytes: budget.scripts.max_prelude_bytes,
        })?;
    map_scripts_import::check_bytes(
        label,
        "preludes",
        prelude_bytes as u64,
        budget.scripts.max_prelude_bytes,
    )?;
    let raw = input.map.archive();
    map_scripts_import::check_bytes(
        label,
        "embedded archive",
        raw.len() as u64,
        budget.scripts.max_archive_bytes,
    )?;
    map_scripts_import::aggregate(
        label,
        &[raw.len(), prelude_bytes],
        budget.aggregate_input_bytes,
    )?;
    map_scripts_import::aggregate(
        label,
        &[raw.len(), prelude_bytes],
        budget.scripts.max_aggregate_input_bytes,
    )?;
    let archive = CheckedArchive::parse(
        raw,
        label,
        ReadBudget {
            max_archive_bytes: budget.scripts.max_archive_bytes,
            max_archive_table_bytes: budget.scripts.max_archive_table_bytes,
            max_raw_packed_bytes: budget.scripts.max_raw_packed_bytes,
            max_decoded_bytes: budget.scripts.max_decoded_bytes,
        },
    )
    .map_err(|source| MapAdmissionError::Read {
        member: "embedded archive",
        source,
    })?;
    let members = [
        ("war3map.w3e", budget.terrain_bytes),
        ("war3map.wpm", budget.pathing_bytes),
        ("war3map.w3i", budget.info_bytes),
        ("war3mapUnits.doo", budget.units_bytes),
    ];
    let mut sizes = [0usize; 4];
    for (index, (name, limit)) in members.iter().copied().enumerate() {
        sizes[index] = archive
            .preflight_member(name, limit)
            .map_err(|source| MapAdmissionError::Read {
                member: name,
                source,
            })?
            .ok_or_else(|| MapAdmissionError::MissingMember {
                map: label.into(),
                member: name,
            })?;
    }
    let script_bytes = archive
        .preflight_member("war3map.j", budget.scripts.max_script_bytes)
        .map_err(|source| MapAdmissionError::Read {
            member: "war3map.j",
            source,
        })?
        .ok_or_else(|| MapScriptImportError::MissingScriptMember {
            map: label.into(),
            member: "war3map.j",
        })?;
    let wts_bytes = archive
        .preflight_member("war3map.wts", budget.scripts.max_wts_bytes)
        .map_err(|source| MapAdmissionError::Read {
            member: "war3map.wts",
            source,
        })?
        .unwrap_or(0);
    map_scripts_import::aggregate(
        label,
        &[raw.len(), prelude_bytes, script_bytes, wts_bytes],
        budget.scripts.max_aggregate_input_bytes,
    )?;
    map_scripts_import::aggregate(
        label,
        &[
            raw.len(),
            prelude_bytes,
            script_bytes,
            wts_bytes,
            sizes[0],
            sizes[1],
            sizes[2],
            sizes[3],
        ],
        budget.aggregate_input_bytes,
    )?;
    // Metadata for every required world/script member passed before extraction.
    let mut digests = Vec::new();
    let mut extract = |name: &'static str, limit: usize| -> Result<Vec<u8>, MapAdmissionError> {
        let bytes = archive
            .extract(name, limit)
            .map_err(|source| MapAdmissionError::Read {
                member: name,
                source,
            })?
            .ok_or_else(|| MapAdmissionError::MissingMember {
                map: label.into(),
                member: name,
            })?;
        digests.push(SourceDigest {
            label: format!("{label}:{name}"),
            hash: pcx_script::fnv1a64(&bytes),
        });
        Ok(bytes)
    };
    let parse_error = |member, source| MapAdmissionError::Parse {
        map: label.into(),
        member,
        source,
    };
    let terrain = Terrain::from_bytes(extract("war3map.w3e", budget.terrain_bytes)?)
        .map_err(|error| parse_error("war3map.w3e", error))?;
    let pathing = Pathing::parse(&extract("war3map.wpm", budget.pathing_bytes)?)
        .map_err(|error| parse_error("war3map.wpm", error.into()))?;
    let terrain_dimensions = GridDimensions {
        width: terrain.width(),
        height: terrain.height(),
    };
    let pathing_dimensions = GridDimensions {
        width: pathing.width,
        height: pathing.height,
    };
    let alignment = pcx_map::grid_alignment::terrain_pathing_alignment(
        terrain_dimensions.width,
        terrain_dimensions.height,
        pathing.width,
        pathing.height,
    )
    .map_err(|source| MapAdmissionError::Alignment {
        map: label.into(),
        terrain: terrain_dimensions,
        pathing: pathing_dimensions,
        source,
    })?;
    let info = MapInfo::from_bytes(extract("war3map.w3i", budget.info_bytes)?)
        .map_err(|error| parse_error("war3map.w3i", error))?;
    let units = Units::parse(&extract("war3mapUnits.doo", budget.units_bytes)?)
        .map_err(|error| parse_error("war3mapUnits.doo", error.into()))?;
    drop(archive);
    let scripts = map_scripts_import::import_map_scripts(
        input.map,
        label,
        input.map_identity.clone(),
        input.common.asset,
        input.blizzard.asset,
        budget.scripts,
    )?;
    let mut required_fields = Vec::new();
    required_fields
        .try_reserve_exact(input.required_fields.len())
        .map_err(|source| MapScriptImportError::Allocation {
            map: label.into(),
            region: "field bindings",
            source,
        })?;
    required_fields.extend_from_slice(input.required_fields);
    let world = ImportedWorld::from_authored(
        Source {
            install: input.map_identity.source.clone(),
            edition: input.map_identity.edition.clone(),
            map: label.into(),
        },
        info,
        pathing,
        &units,
        input.catalog,
        input.required_fields,
    )?;
    let scripts = Arc::new(scripts);
    let map_digest = SourceDigest {
        label: label.into(),
        hash: pcx_script::fnv1a64(input.map.raw()),
    };
    let admission_witness = crate::neutral_player::AdmissionWitness::new(
        input.map.raw(),
        world.info.raw(),
        input.map_identity.clone(),
        &map_digest,
        Arc::clone(&scripts),
    );
    Ok(ParsedMapContext {
        admission_witness: Some(admission_witness),
        required_fields,
        input_budget: budget,
        sources: MapInputSources {
            map: input.map_identity.clone(),
            common: input.common.identity.clone(),
            blizzard: input.blizzard.identity.clone(),
            catalog: input.catalog_identity.clone(),
        },
        map_digest,
        world_member_digests: digests,
        terrain,
        alignment,
        world,
        scripts,
    })
}

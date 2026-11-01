//! Parsed same-map context. Admission does not execute scripts or create game state.
use crate::map_scripts_import::{self, MapScriptBudget, MapScriptImportError};
use crate::scripts::{AssetIdentity, MapScripts, ScriptAsset, SourceDigest};
use pcx_map::{
    GridAlignment, GridAlignmentError, GridDimensions, Map, MapInfo, Pathing, Terrain, Units,
};
use pcx_mpq::checked::{CheckedArchive, ReadBudget, ReadError};
use pcx_sim::world::{ImportError, ImportedWorld, ObjectCatalog, Source};
use sha2::Digest;

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
    /// TriggerData of the same edition/version for the editor trigger path.
    /// `None` leaves `war3map.wtg` compilation refusing loudly.
    pub trigger_catalog: Option<std::sync::Arc<pcx_trig::catalog::Catalog>>,
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
    #[error("editor triggers in {map}: {detail}")]
    Editor { map: String, detail: String },
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
#[derive(Debug, Clone, PartialEq, Eq)]
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
    /// SHA-256 of the admitted map bytes. Use this for identity; `map_digest` is FNV-1a, a checksum only.
    pub map_sha256: [u8; 32],
    pub world_member_digests: Vec<SourceDigest>,
    pub terrain: Terrain,
    pub alignment: GridAlignment,
    pub world: ImportedWorld,
    pub scripts: Arc<MapScripts>,
    /// `war3map.ini` when the archive carries it, with script-selection provenance.
    pub ini: Option<pcx_map::Ini>,
    /// `war3map.wpmp` allied shared data, retained opaque; layout is unmeasured
    /// so gameplay use refuses loudly with the map label.
    pub wpmp: Option<pcx_map::Wpmp>,
    /// Which script member was compiled: `war3map.j` or `war3map.wtg`.
    pub script_source: String,
    /// Declared object tables from `war3map.w3i`; the join key into the
    /// object-editor overlays below.
    pub declared_tables: Vec<pcx_map::info::TableList>,
    /// Parsed object-edit overlays for all seven `W3?` families the archive
    /// carries, keyed by member name. Absent members stay absent.
    pub object_overlays: std::collections::BTreeMap<String, pcx_objects::editor::Overlay>,
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
#[cfg_attr(
    feature = "profile-cpu",
    tracing::instrument(skip_all, target = "pcx_cpu", name = "startup.map_admission")
)]
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
    // Optional map-compat members: init selection, shared data, strings and
    // both script paths. Presence is preflighted; absence is not an error.
    let _ini_size = archive
        .preflight_member("war3map.ini", budget.scripts.max_wts_bytes)
        .map_err(|source| MapAdmissionError::Read {
            member: "war3map.ini",
            source,
        })?
        .unwrap_or(0);
    let _wpmp_size = archive
        .preflight_member("war3map.wpmp", budget.scripts.max_wts_bytes)
        .map_err(|source| MapAdmissionError::Read {
            member: "war3map.wpmp",
            source,
        })?
        .unwrap_or(0);
    let j_size = archive
        .preflight_member("war3map.j", budget.scripts.max_script_bytes)
        .map_err(|source| MapAdmissionError::Read {
            member: "war3map.j",
            source,
        })?
        .unwrap_or(0);
    let wtg_size = archive
        .preflight_member("war3map.wtg", budget.scripts.max_script_bytes)
        .map_err(|source| MapAdmissionError::Read {
            member: "war3map.wtg",
            source,
        })?
        .unwrap_or(0);
    let wct_size = archive
        .preflight_member("war3map.wct", budget.scripts.max_script_bytes)
        .map_err(|source| MapAdmissionError::Read {
            member: "war3map.wct",
            source,
        })?
        .unwrap_or(0);
    let mut object_sizes: Vec<(&'static str, usize)> = Vec::new();
    for member in [
        "war3map.w3u",
        "war3map.w3t",
        "war3map.w3a",
        "war3map.w3b",
        "war3map.w3d",
        "war3map.w3h",
        "war3map.w3q",
    ] {
        let size = archive
            .preflight_member(member, budget.scripts.max_decoded_bytes)
            .map_err(|source| MapAdmissionError::Read { member, source })?
            .unwrap_or(0);
        if size > 0 {
            object_sizes.push((member, size));
        }
    }
    let present: Vec<&str> = [
        ("war3map.j", j_size),
        ("war3map.wtg", wtg_size),
        ("war3map.wct", wct_size),
    ]
    .into_iter()
    .filter(|(_, size)| *size > 0)
    .map(|(name, _)| name)
    .collect();
    let wts_bytes = archive
        .preflight_member("war3map.wts", budget.scripts.max_wts_bytes)
        .map_err(|source| MapAdmissionError::Read {
            member: "war3map.wts",
            source,
        })?
        .unwrap_or(0);
    map_scripts_import::aggregate(
        label,
        &[
            raw.len(),
            prelude_bytes,
            j_size,
            wts_bytes,
            wtg_size,
            wct_size,
        ],
        budget.scripts.max_aggregate_input_bytes,
    )?;
    map_scripts_import::aggregate(
        label,
        &[
            raw.len(),
            prelude_bytes,
            j_size,
            wts_bytes,
            wtg_size,
            wct_size,
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
    let declared_tables = info.table_lists().to_vec();
    let units = Units::parse(&extract("war3mapUnits.doo", budget.units_bytes)?)
        .map_err(|error| parse_error("war3mapUnits.doo", error.into()))?;
    let mut extract_optional =
        |name: &'static str, limit: usize| -> Result<Option<Vec<u8>>, MapAdmissionError> {
            let bytes = archive
                .extract(name, limit)
                .map_err(|source| MapAdmissionError::Read {
                    member: name,
                    source,
                })?;
            if let Some(bytes) = bytes {
                digests.push(SourceDigest {
                    label: format!("{label}:{name}"),
                    hash: pcx_script::fnv1a64(&bytes),
                });
                Ok(Some(bytes))
            } else {
                Ok(None)
            }
        };
    let ini = extract_optional("war3map.ini", budget.scripts.max_wts_bytes)?
        .map(|bytes| {
            pcx_map::Ini::from_bytes(bytes).map_err(|reason| MapAdmissionError::Parse {
                map: label.into(),
                member: "war3map.ini",
                source: pcx_map::Error::BadIni(reason),
            })
        })
        .transpose()?;
    let wpmp = extract_optional("war3map.wpmp", budget.scripts.max_wts_bytes)?
        .map(|bytes| {
            pcx_map::Wpmp::from_bytes(bytes).map_err(|reason| MapAdmissionError::Parse {
                map: label.into(),
                member: "war3map.wpmp",
                source: pcx_map::Error::BadWpmp(reason),
            })
        })
        .transpose()?;
    let mut object_overlays = std::collections::BTreeMap::new();
    for (member, _) in object_sizes {
        let bytes = extract_optional(member, budget.scripts.max_decoded_bytes)?
            .expect("preflighted object member is present");
        let file = pcx_objects::ObjectFile::parse(member, &bytes).map_err(|error| {
            MapAdmissionError::Parse {
                map: label.into(),
                member,
                source: pcx_map::Error::BadObjects(error.to_string()),
            }
        })?;
        object_overlays.insert(member.to_string(), pcx_objects::editor::Overlay::of(&file));
    }
    let selection = pcx_map::preferred_script(ini.as_ref(), &present).ok_or_else(|| {
        MapAdmissionError::MissingMember {
            map: label.into(),
            member: "war3map.j",
        }
    })?;
    let script_source = selection.to_string();
    drop(archive);
    let scripts =
        match selection {
            "war3map.j" => map_scripts_import::import_map_scripts(
                input.map,
                label,
                input.map_identity.clone(),
                input.common.asset,
                input.blizzard.asset,
                budget.scripts,
            )
            .map_err(MapAdmissionError::Input)?,
            _ => {
                let catalog = input.trigger_catalog.as_ref().ok_or_else(|| {
                    MapAdmissionError::MissingMember {
                        map: label.into(),
                        member: "war3map.wtg",
                    }
                })?;
                crate::editor_triggers::compile_editor_map_scripts_with_wts(
                    input.map,
                    label,
                    input.map_identity.clone(),
                    input.common.asset,
                    input.blizzard.asset,
                    catalog,
                    pcx_mpq::checked::ReadBudget {
                        max_archive_bytes: budget.scripts.max_archive_bytes,
                        max_archive_table_bytes: budget.scripts.max_archive_table_bytes,
                        max_raw_packed_bytes: budget.scripts.max_raw_packed_bytes,
                        max_decoded_bytes: budget.scripts.max_decoded_bytes,
                    },
                )
                .map_err(|error| MapAdmissionError::Editor {
                    map: label.into(),
                    detail: error.to_string(),
                })?
            }
        };
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
    let map_sha256 = sha2::Sha256::digest(input.map.raw()).into();
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
        map_sha256,
        world_member_digests: digests,
        terrain,
        alignment,
        world,
        scripts,
        ini,
        wpmp,
        script_source,
        declared_tables,
        object_overlays,
    })
}

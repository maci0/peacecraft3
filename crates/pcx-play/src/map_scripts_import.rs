//! Import authored map scripts with explicit input budgets and provenance.
//!
//! Borrowed header and allocation-layout checks precede table decoding; the archive
//! is not cloned. Selected members are checked before packed/decoded allocation. These
//! input limits do not bound total compiler, AST, decoder or runtime memory.
//! Prelude sources and asset identity are supplied by the caller; no archive,
//! locale, native Host or edition priority is inferred.

use crate::scripts::{AssetIdentity, MapScripts, ScriptAsset};
use pcx_map::Map;
use pcx_mpq::checked::{CheckedArchive, ReadBudget, ReadError};
use std::alloc::Layout;
use thiserror::Error;

const SCRIPT: &str = "war3map.j";
const WTS: &str = "war3map.wts";

/// Caller policy for input-sized allocations, not native game limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapScriptBudget {
    pub max_script_bytes: usize,
    pub max_wts_bytes: usize,
    pub max_archive_bytes: usize,
    /// Sum of serialized hash and block table bytes, checked before decoding.
    pub max_archive_table_bytes: usize,
    pub max_raw_packed_bytes: usize,
    pub max_decoded_bytes: usize,
    /// Sum of the two supplied prelude byte lengths.
    pub max_prelude_bytes: usize,
    /// Archive input bytes + decoded script/WTS bytes + prelude bytes. Table buffers
    /// and compiler/AST allocations are not included in this input accounting.
    pub max_aggregate_input_bytes: usize,
}

impl MapScriptBudget {
    /// Harness policy defaults. Callers may choose different explicit limits.
    pub const DEFAULT: Self = Self {
        max_script_bytes: 4 * 1024 * 1024,
        max_wts_bytes: 2 * 1024 * 1024,
        max_archive_bytes: 64 * 1024 * 1024,
        max_archive_table_bytes: 256 * 1024,
        max_raw_packed_bytes: 8 * 1024 * 1024,
        max_decoded_bytes: 4 * 1024 * 1024,
        max_prelude_bytes: 8 * 1024 * 1024,
        max_aggregate_input_bytes: 128 * 1024 * 1024,
    };
}
impl Default for MapScriptBudget {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Contextual typed import errors for map script extraction and compilation.
#[derive(Debug, Error)]
pub enum MapScriptImportError {
    #[error("asset identity field {field:?} must not be empty")]
    InvalidIdentity { field: &'static str },

    #[error("prelude asset {label:?} is empty")]
    EmptyPrelude { label: String },

    #[error("map MPQ archive in {map:?} failed to open: {source}")]
    MpqOpen {
        map: String,
        #[source]
        source: pcx_mpq::Error,
    },

    #[error("missing required script member {member:?} in map {map:?}")]
    MissingScriptMember { map: String, member: &'static str },

    #[error(
        "ambiguous localized script member {member:?} in map {map:?}: \
         {count} distinct locale/platform candidates found (expected 1)"
    )]
    AmbiguousLocalizedScript {
        map: String,
        member: &'static str,
        count: usize,
    },

    #[error(
        "input {member:?} in map {map:?} exceeds size budget: \
         declared {declared_bytes} bytes, limit is {max_bytes} bytes"
    )]
    BudgetExceeded {
        map: String,
        member: &'static str,
        declared_bytes: usize,
        max_bytes: usize,
    },

    #[error("archive table size {bytes} exceeds table budget {max_bytes} bytes in {map:?}")]
    ArchiveTableBudgetExceeded {
        map: String,
        bytes: usize,
        max_bytes: usize,
    },

    #[error(
        "aggregate prelude bytes {declared_bytes} exceed aggregate budget {max_bytes} in {map:?}"
    )]
    AggregatePreludeExceeded {
        map: String,
        declared_bytes: usize,
        max_bytes: usize,
    },

    #[error(
        "aggregate input bytes {declared_bytes} exceed aggregate input budget \
         {max_bytes} bytes in {map:?}; this budget does not cover full compiler-memory bounds"
    )]
    AggregateInputExceeded {
        map: String,
        declared_bytes: usize,
        max_bytes: usize,
    },

    #[error("failed extracting {member:?} from map {map:?}: {source}")]
    ExtractionFailed {
        map: String,
        member: &'static str,
        #[source]
        source: pcx_mpq::Error,
    },

    /// A present but malformed WTS member: the file exists in the archive but
    /// its bytes fail parsing/decryption/decompression. This is distinct from
    /// an absent optional WTS (which yields `None`).
    #[error("present WTS member {member:?} in map {map:?} is malformed: {reason}")]
    MalformedWts {
        map: String,
        member: &'static str,
        reason: String,
    },

    #[error("{region} in {map:?} cannot fit the platform allocation layout: {bytes} bytes")]
    InvalidLayout {
        map: String,
        region: &'static str,
        bytes: u64,
    },

    #[error("cannot reserve {region} in {map:?}: {source}")]
    Allocation {
        map: String,
        region: &'static str,
        #[source]
        source: std::collections::TryReserveError,
    },

    #[error("map script compilation failed for {map:?}: {reason}")]
    Compilation { map: String, reason: String },
}

fn layout<T>(
    map: &str,
    region: &'static str,
    count: usize,
    bytes: u64,
) -> Result<(), MapScriptImportError> {
    Layout::array::<T>(count)
        .map(|_| ())
        .map_err(|_| MapScriptImportError::InvalidLayout {
            map: map.into(),
            region,
            bytes,
        })
}

fn check_bytes(
    map: &str,
    member: &'static str,
    bytes: u64,
    max_bytes: usize,
) -> Result<usize, MapScriptImportError> {
    let declared_bytes =
        usize::try_from(bytes).map_err(|_| MapScriptImportError::InvalidLayout {
            map: map.into(),
            region: member,
            bytes,
        })?;
    if declared_bytes > max_bytes {
        return Err(MapScriptImportError::BudgetExceeded {
            map: map.into(),
            member,
            declared_bytes,
            max_bytes,
        });
    }
    layout::<u8>(map, member, declared_bytes, bytes)?;
    Ok(declared_bytes)
}

fn aggregate(map: &str, counts: &[usize], max_bytes: usize) -> Result<(), MapScriptImportError> {
    let declared_bytes = counts
        .iter()
        .try_fold(0usize, |sum, n| sum.checked_add(*n))
        .ok_or_else(|| MapScriptImportError::AggregateInputExceeded {
            map: map.into(),
            declared_bytes: usize::MAX,
            max_bytes,
        })?;
    if declared_bytes > max_bytes {
        return Err(MapScriptImportError::AggregateInputExceeded {
            map: map.into(),
            declared_bytes,
            max_bytes,
        });
    }
    layout::<u8>(
        map,
        "aggregate input",
        declared_bytes,
        declared_bytes as u64,
    )
}

fn member_error(map: &str, member: &'static str, source: pcx_mpq::Error) -> MapScriptImportError {
    if member == WTS {
        MapScriptImportError::MalformedWts {
            map: map.into(),
            member,
            reason: source.to_string(),
        }
    } else {
        MapScriptImportError::ExtractionFailed {
            map: map.into(),
            member,
            source,
        }
    }
}

fn read_error(error: ReadError) -> MapScriptImportError {
    match error {
        ReadError::MpqOpen { map, source } => MapScriptImportError::MpqOpen { map, source },
        ReadError::AmbiguousMember { map, member, count } => {
            MapScriptImportError::AmbiguousLocalizedScript { map, member, count }
        }
        ReadError::BudgetExceeded {
            map,
            member,
            declared_bytes,
            max_bytes,
        } => MapScriptImportError::BudgetExceeded {
            map,
            member,
            declared_bytes,
            max_bytes,
        },
        ReadError::ArchiveTableBudgetExceeded {
            map,
            bytes,
            max_bytes,
        } => MapScriptImportError::ArchiveTableBudgetExceeded {
            map,
            bytes,
            max_bytes,
        },
        ReadError::InvalidLayout { map, region, bytes } => {
            MapScriptImportError::InvalidLayout { map, region, bytes }
        }
        ReadError::ExtractionFailed {
            map,
            member,
            source,
        } => member_error(&map, member, source),
    }
}

/// Extract compiled `war3map.j` and optional WTS, then compile identified preludes.
/// Budget checks precede the corresponding input-sized allocations. Metadata/error
/// strings, decoder temporaries, parsed WTS tables and compiler/AST storage are not
/// a total-memory guarantee. Native execution and WTG lowering are separate gates.
pub fn import_map_scripts(
    map: &Map<'_>,
    map_label: &str,
    identity: AssetIdentity,
    common: ScriptAsset<'_>,
    blizzard: ScriptAsset<'_>,
    budget: MapScriptBudget,
) -> Result<MapScripts, MapScriptImportError> {
    for (field, value) in [
        ("edition", identity.edition.as_str()),
        ("version", identity.version.as_str()),
        ("source", identity.source.as_str()),
        ("map_label", map_label),
    ] {
        if value.is_empty() {
            return Err(MapScriptImportError::InvalidIdentity { field });
        }
    }
    for asset in [common, blizzard] {
        if asset.bytes.is_empty() {
            return Err(MapScriptImportError::EmptyPrelude {
                label: asset.label.into(),
            });
        }
    }
    let prelude_bytes = common
        .bytes
        .len()
        .checked_add(blizzard.bytes.len())
        .ok_or_else(|| MapScriptImportError::AggregatePreludeExceeded {
            map: map_label.into(),
            declared_bytes: usize::MAX,
            max_bytes: budget.max_prelude_bytes,
        })?;
    if prelude_bytes > budget.max_prelude_bytes {
        return Err(MapScriptImportError::AggregatePreludeExceeded {
            map: map_label.into(),
            declared_bytes: prelude_bytes,
            max_bytes: budget.max_prelude_bytes,
        });
    }
    layout::<u8>(map_label, "preludes", prelude_bytes, prelude_bytes as u64)?;
    let raw = map.archive();
    check_bytes(
        map_label,
        "embedded archive",
        raw.len() as u64,
        budget.max_archive_bytes,
    )?;
    aggregate(
        map_label,
        &[raw.len(), prelude_bytes],
        budget.max_aggregate_input_bytes,
    )?;
    let archive = CheckedArchive::parse(
        raw,
        map_label,
        ReadBudget {
            max_archive_bytes: budget.max_archive_bytes,
            max_archive_table_bytes: budget.max_archive_table_bytes,
            max_raw_packed_bytes: budget.max_raw_packed_bytes,
            max_decoded_bytes: budget.max_decoded_bytes,
        },
    )
    .map_err(read_error)?;
    let script_size = archive
        .preflight_member(SCRIPT, budget.max_script_bytes)
        .map_err(read_error)?
        .ok_or_else(|| MapScriptImportError::MissingScriptMember {
            map: map_label.into(),
            member: SCRIPT,
        })?;
    let wts_size = archive
        .preflight_member(WTS, budget.max_wts_bytes)
        .map_err(read_error)?;
    aggregate(
        map_label,
        &[raw.len(), prelude_bytes, script_size, wts_size.unwrap_or(0)],
        budget.max_aggregate_input_bytes,
    )?;
    let script_bytes = archive
        .extract(SCRIPT, budget.max_script_bytes)
        .map_err(read_error)?
        .expect("immutable preflighted script");
    let wts_bytes = archive
        .extract(WTS, budget.max_wts_bytes)
        .map_err(read_error)?;
    if let Some(bytes) = &wts_bytes {
        let table =
            pcx_trig::wts::Table::parse(bytes).map_err(|e| MapScriptImportError::MalformedWts {
                map: map_label.into(),
                member: WTS,
                reason: e.to_string(),
            })?;
        if !table.ids_are_unique() {
            return Err(MapScriptImportError::MalformedWts {
                map: map_label.into(),
                member: WTS,
                reason: "duplicate string ids".into(),
            });
        }
    }
    let script_label = format!("{map_label}:{SCRIPT}");
    let wts_label = format!("{map_label}:{WTS}");
    MapScripts::compile(
        identity,
        common,
        blizzard,
        ScriptAsset {
            label: &script_label,
            bytes: &script_bytes,
        },
        wts_bytes.as_ref().map(|bytes| ScriptAsset {
            label: &wts_label,
            bytes,
        }),
    )
    .map_err(|reason| MapScriptImportError::Compilation {
        map: map_label.into(),
        reason,
    })
}

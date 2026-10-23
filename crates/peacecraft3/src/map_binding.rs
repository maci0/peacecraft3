//! Bind one explicitly selected map through a caller-configured resolver.
use crate::map_requests::{BoundScript, MapBindings, bind_metadata};
use pcx_assets::{Provenance, ResolvedAsset, Resolver, SourceInfo};
use pcx_map::Map;
use pcx_mpq::checked::{CheckedArchive, ReadBudget};
use pcx_objects::editor::{EditorReadLimits, path::SlkId, report::Overlay};
use pcx_play::{map_admission::MapAdmissionBudget, scripts::AssetIdentity};
use std::{path::Path, sync::Arc};

pub struct IdentifiedMember<'a> {
    pub path: &'a str,
    pub label: &'a str,
    pub identity: &'a AssetIdentity,
    pub source: &'a SourceInfo,
}
pub struct BindingInputs<'a> {
    pub map: IdentifiedMember<'a>,
    pub common: IdentifiedMember<'a>,
    pub blizzard: IdentifiedMember<'a>,
    pub catalog_identity: &'a AssetIdentity,
    pub catalog_label: &'a str,
    pub catalog_sources: &'a [SourceInfo],
    pub required_catalog_tables: &'a [SlkId],
    pub required_fields: &'a [[u8; 4]],
    pub revision: u64,
}
#[derive(Clone, Copy)]
pub struct OverlayLimits {
    pub unit_bytes: usize,
    pub item_bytes: usize,
    pub aggregate_bytes: usize,
}
#[derive(Clone, Copy)]
pub struct BindingLimits {
    pub reader: EditorReadLimits,
    pub overlay: OverlayLimits,
    pub admission: MapAdmissionBudget,
    pub max_map_file_bytes: usize,
    pub max_resolver_input_bytes: usize,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetReceipt {
    pub path: pcx_assets::AssetPath,
    pub provenance: Provenance,
    pub hash: u64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayReceipt {
    pub member: &'static str,
    pub map_source: SourceInfo,
    pub map_locator: String,
    pub byte_length: usize,
    pub hash: u64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingReceipts {
    pub map: AssetReceipt,
    pub common: AssetReceipt,
    pub blizzard: AssetReceipt,
    pub catalog: Vec<AssetReceipt>,
    pub missing_catalog: Vec<String>,
    pub units: Option<OverlayReceipt>,
    pub items: Option<OverlayReceipt>,
}
#[derive(Debug, thiserror::Error)]
pub enum BindingError {
    #[error("{input}: caller identity/binding is empty")]
    Identity { input: &'static str },
    #[error("asset {member}: {source}")]
    Resolve {
        member: String,
        #[source]
        source: pcx_assets::Error,
    },
    #[error("asset {member}: caller source differs from resolver winner")]
    SourceMismatch {
        member: String,
        expected: Vec<SourceInfo>,
        actual: Box<SourceInfo>,
    },
    #[error("{member}: {actual} input bytes exceed caller limit {limit}")]
    Budget {
        member: String,
        actual: usize,
        limit: usize,
    },
    #[error("{member}: aggregate input length overflows usize")]
    Overflow { member: String },
    #[error("selected map {map}: {source}")]
    Container {
        map: String,
        #[source]
        source: pcx_map::Error,
    },
    #[error("selected map {map}, overlay {member}: {source}")]
    OverlayRead {
        map: String,
        member: &'static str,
        #[source]
        source: Box<pcx_mpq::checked::ReadError>,
    },
    #[error(
        "selected map {map}, overlay {member}: duplicate custom raw ID {rawcode:?} at records {first_record}/{second_record}"
    )]
    AmbiguousOverlay {
        map: String,
        member: &'static str,
        rawcode: [u8; 4],
        first_record: usize,
        second_record: usize,
    },
    #[error("selected map {map}, overlay {member}: {source}")]
    OverlayParse {
        map: String,
        member: &'static str,
        #[source]
        source: pcx_objects::Error,
    },
    #[error("{label}: {source}")]
    Editor {
        label: String,
        #[source]
        source: pcx_objects::Error,
    },
    #[error("catalog {label}: required caller table {member} is absent")]
    MissingCatalog { label: String, member: &'static str },
}
fn account(
    total: &mut usize,
    member: &str,
    bytes: usize,
    limit: usize,
) -> Result<(), BindingError> {
    let sum = total
        .checked_add(bytes)
        .ok_or_else(|| BindingError::Overflow {
            member: member.to_owned(),
        })?;
    if sum > limit {
        return Err(BindingError::Budget {
            member: member.to_owned(),
            actual: sum,
            limit,
        });
    }
    *total = sum;
    Ok(())
}
fn resolve(
    resolver: &Resolver,
    member: &str,
    limit: Option<usize>,
) -> Result<ResolvedAsset, BindingError> {
    let result = match limit {
        Some(limit) => resolver.resolve_bounded(member, limit as u64),
        None => resolver.resolve(member),
    };
    result.map_err(|source| BindingError::Resolve {
        member: member.to_owned(),
        source,
    })
}
/// Record the exact resolved member and source used by a presentation consumer.
pub fn receipt(asset: &ResolvedAsset) -> AssetReceipt {
    AssetReceipt {
        path: asset.path.clone(),
        provenance: asset.provenance.clone(),
        hash: pcx_mpq::fnv1a64(&asset.bytes),
    }
}
fn source(asset: &ResolvedAsset, expected: &[SourceInfo]) -> Result<(), BindingError> {
    if !expected.contains(&asset.provenance.source) {
        return Err(BindingError::SourceMismatch {
            member: asset.path.as_str().to_owned(),
            expected: expected.to_vec(),
            actual: Box::new(asset.provenance.source.clone()),
        });
    }
    Ok(())
}
fn identity(input: &'static str, value: &AssetIdentity, label: &str) -> Result<(), BindingError> {
    if label.trim().is_empty()
        || value.edition.trim().is_empty()
        || value.version.trim().is_empty()
        || value.source.trim().is_empty()
    {
        return Err(BindingError::Identity { input });
    }
    Ok(())
}
fn checked_overlay(
    view: &CheckedArchive<'_>,
    map: &str,
    member: &'static str,
    limit: usize,
) -> Result<Option<Vec<u8>>, BindingError> {
    view.extract(member, limit)
        .map_err(|source| BindingError::OverlayRead {
            map: map.to_owned(),
            member,
            source: Box::new(source),
        })
}
fn overlay(
    raw: Option<Vec<u8>>,
    member: &'static str,
    map: &IdentifiedMember<'_>,
    map_receipt: &AssetReceipt,
) -> Result<(Overlay, Option<OverlayReceipt>), BindingError> {
    match raw {
        None => Ok((Overlay::default(), None)),
        Some(bytes) => {
            let file = pcx_objects::ObjectFile::parse(member, &bytes).map_err(|source| {
                BindingError::OverlayParse {
                    map: map.label.to_owned(),
                    member,
                    source,
                }
            })?;
            let mut custom = std::collections::BTreeMap::new();
            for (ordinal, record) in file.custom().iter().enumerate() {
                if let Some(first) = custom.insert(record.modified, ordinal) {
                    return Err(BindingError::AmbiguousOverlay {
                        map: map.label.to_owned(),
                        member,
                        rawcode: record.modified,
                        first_record: first,
                        second_record: ordinal,
                    });
                }
            }
            let record = OverlayReceipt {
                member,
                map_source: map_receipt.provenance.source.clone(),
                map_locator: map_receipt.provenance.locator.clone(),
                byte_length: bytes.len(),
                hash: pcx_mpq::fnv1a64(&bytes),
            };
            Ok((Overlay::of(&file), Some(record)))
        }
    }
}
pub fn bind_map(
    resolver: &Resolver,
    input: BindingInputs<'_>,
    limits: BindingLimits,
) -> Result<MapBindings, BindingError> {
    bind_retained(resolver, None, input, limits, false)
}
/// Require source preallocation budgets as well as existing parser/input ceilings.
/// Sources without that read contract refuse; no unbounded fallback is performed.
pub fn bind_map_bounded(
    resolver: Arc<Resolver>,
    input: BindingInputs<'_>,
    limits: BindingLimits,
) -> Result<MapBindings, BindingError> {
    bind_retained(
        resolver.as_ref(),
        Some(Arc::clone(&resolver)),
        input,
        limits,
        true,
    )
}
fn bind_retained(
    resolver: &Resolver,
    retained: Option<Arc<Resolver>>,
    input: BindingInputs<'_>,
    limits: BindingLimits,
    bounded: bool,
) -> Result<MapBindings, BindingError> {
    for (role, member) in [
        ("map", &input.map),
        ("common", &input.common),
        ("blizzard", &input.blizzard),
    ] {
        identity(role, member.identity, member.label)?;
    }
    identity("catalog", input.catalog_identity, input.catalog_label)?;
    if input.catalog_sources.is_empty() {
        return Err(BindingError::Identity {
            input: "catalog sources",
        });
    }
    if input.required_fields.len() > limits.admission.required_fields {
        return Err(BindingError::Budget {
            member: "required field bindings".into(),
            actual: input.required_fields.len(),
            limit: limits.admission.required_fields,
        });
    }
    let mut total = 0;
    let map = resolve(
        resolver,
        input.map.path,
        bounded.then_some(
            limits
                .max_map_file_bytes
                .min(limits.max_resolver_input_bytes),
        ),
    )?;
    source(&map, std::slice::from_ref(input.map.source))?;
    if map.bytes.len() > limits.max_map_file_bytes {
        return Err(BindingError::Budget {
            member: input.map.path.to_owned(),
            actual: map.bytes.len(),
            limit: limits.max_map_file_bytes,
        });
    }
    account(
        &mut total,
        input.map.path,
        map.bytes.len(),
        limits.max_resolver_input_bytes,
    )?;
    let map_receipt = receipt(&map);
    let parsed = Map::parse(&map.bytes).map_err(|source| BindingError::Container {
        map: input.map.label.to_owned(),
        source,
    })?;
    let b = limits.admission.scripts;
    let view = CheckedArchive::parse(
        parsed.archive(),
        input.map.label,
        ReadBudget {
            max_archive_bytes: b.max_archive_bytes,
            max_archive_table_bytes: b.max_archive_table_bytes,
            max_raw_packed_bytes: b.max_raw_packed_bytes,
            max_decoded_bytes: b.max_decoded_bytes,
        },
    )
    .map_err(|source| BindingError::OverlayRead {
        map: input.map.label.to_owned(),
        member: "object archive",
        source: Box::new(source),
    })?;
    let mut overlay_total = 0;
    for (member, limit) in [
        ("war3map.w3u", limits.overlay.unit_bytes),
        ("war3map.w3t", limits.overlay.item_bytes),
    ] {
        let size = view
            .preflight_member(member, limit)
            .map_err(|source| BindingError::OverlayRead {
                map: input.map.label.to_owned(),
                member,
                source: Box::new(source),
            })?
            .unwrap_or(0);
        account(
            &mut overlay_total,
            member,
            size,
            limits.overlay.aggregate_bytes,
        )?;
    }
    // All object-member preflight and aggregate checks precede either extraction.
    account(
        &mut total,
        "object overlays",
        overlay_total,
        limits.max_resolver_input_bytes,
    )?;
    let unit_bytes = checked_overlay(
        &view,
        input.map.label,
        "war3map.w3u",
        limits.overlay.unit_bytes,
    )?;
    let item_bytes = checked_overlay(
        &view,
        input.map.label,
        "war3map.w3t",
        limits.overlay.item_bytes,
    )?;
    let (units, unit_receipt) = overlay(unit_bytes, "war3map.w3u", &input.map, &map_receipt)?;
    let (items, item_receipt) = overlay(item_bytes, "war3map.w3t", &input.map, &map_receipt)?;
    drop(view);
    drop(parsed);
    let common = resolve(
        resolver,
        input.common.path,
        bounded.then_some(
            b.max_prelude_bytes
                .min(limits.max_resolver_input_bytes.saturating_sub(total)),
        ),
    )?;
    source(&common, std::slice::from_ref(input.common.source))?;
    if common.bytes.len() > b.max_prelude_bytes {
        return Err(BindingError::Budget {
            member: input.common.path.to_owned(),
            actual: common.bytes.len(),
            limit: b.max_prelude_bytes,
        });
    }
    account(
        &mut total,
        input.common.path,
        common.bytes.len(),
        limits.max_resolver_input_bytes,
    )?;
    let blizzard = resolve(
        resolver,
        input.blizzard.path,
        bounded.then_some(
            b.max_prelude_bytes
                .saturating_sub(common.bytes.len())
                .min(limits.max_resolver_input_bytes.saturating_sub(total)),
        ),
    )?;
    source(&blizzard, std::slice::from_ref(input.blizzard.source))?;
    let prelude_bytes = common
        .bytes
        .len()
        .checked_add(blizzard.bytes.len())
        .ok_or_else(|| BindingError::Overflow {
            member: "preludes".into(),
        })?;
    if prelude_bytes > b.max_prelude_bytes {
        return Err(BindingError::Budget {
            member: "preludes".into(),
            actual: prelude_bytes,
            limit: b.max_prelude_bytes,
        });
    }
    account(
        &mut total,
        input.blizzard.path,
        blizzard.bytes.len(),
        limits.max_resolver_input_bytes,
    )?;
    let mut catalog = Vec::new();
    let mut catalog_bytes = 0usize;
    let mut missing = Vec::new();
    let count = resolver.paths().count();
    if count > limits.reader.max_listed_paths {
        return Err(BindingError::Budget {
            member: "resolver inventory".into(),
            actual: count,
            limit: limits.reader.max_listed_paths,
        });
    }
    let listed: Vec<_> = resolver
        .paths()
        .map(|path| path.as_str().to_owned())
        .collect();
    let bound = bind_metadata(
        Path::new(input.catalog_label),
        &listed,
        limits.reader,
        |member| {
            let limit = limits
                .reader
                .max_asset_bytes
                .min(
                    limits
                        .reader
                        .max_aggregate_input_bytes
                        .saturating_sub(catalog_bytes),
                )
                .min(limits.max_resolver_input_bytes.saturating_sub(total));
            let result = if bounded {
                resolver.resolve_bounded(member, limit as u64)
            } else {
                resolver.resolve(member)
            };
            let asset = match result {
                Ok(asset) => asset,
                Err(pcx_assets::Error::MissingAsset { .. }) => {
                    if input
                        .required_catalog_tables
                        .iter()
                        .any(|table| table.path() == member)
                    {
                        return Err(BindingError::MissingCatalog {
                            label: input.catalog_label.to_owned(),
                            member: input
                                .required_catalog_tables
                                .iter()
                                .find(|table| table.path() == member)
                                .expect("matched table")
                                .path(),
                        });
                    }
                    missing.push(member.to_owned());
                    return Ok(None);
                }
                Err(source) => {
                    return Err(BindingError::Resolve {
                        member: member.to_owned(),
                        source,
                    });
                }
            };
            source(&asset, input.catalog_sources)?;
            account(
                &mut total,
                member,
                asset.bytes.len(),
                limits.max_resolver_input_bytes,
            )?;
            if bounded {
                account(
                    &mut catalog_bytes,
                    member,
                    asset.bytes.len(),
                    limits.reader.max_aggregate_input_bytes,
                )?;
            }
            catalog.push(receipt(&asset));
            Ok(Some(asset.bytes))
        },
        units,
        items,
    )
    .map_err(|source| BindingError::Editor {
        label: input.catalog_label.to_owned(),
        source,
    })?;
    let receipts = Arc::new(BindingReceipts {
        map: map_receipt,
        common: receipt(&common),
        blizzard: receipt(&blizzard),
        catalog,
        missing_catalog: missing,
        units: unit_receipt,
        items: item_receipt,
    });
    Ok(MapBindings {
        map_identity: input.map.identity.clone(),
        map_label: input.map.label.to_owned(),
        catalog_map_bytes: Arc::from(map.bytes),
        receipts,
        resolver: retained,
        revision: input.revision,
        common: BoundScript {
            identity: input.common.identity.clone(),
            label: input.common.label.to_owned(),
            bytes: common.bytes,
        },
        blizzard: BoundScript {
            identity: input.blizzard.identity.clone(),
            label: input.blizzard.label.to_owned(),
            bytes: blizzard.bytes,
        },
        catalog_identity: input.catalog_identity.clone(),
        catalog_label: input.catalog_label.to_owned(),
        catalog_sources: input.catalog_sources.to_vec(),
        required_catalog_tables: input.required_catalog_tables.to_vec(),
        catalog: bound,
        required_fields: input.required_fields.to_vec(),
        budget: limits.admission,
        max_map_file_bytes: limits.max_map_file_bytes,
        limits,
    })
}

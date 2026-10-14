//! Explicit metadata only. The caller's UTF-8 ceiling precedes serde allocation.
use crate::browser_configuration::{Configuration, ConfigurationInbox, InboxError, Member};
use crate::map_binding::{BindingLimits, OverlayLimits};
use pcx_assets::{
    ArchiveLayer, AssetPath, GameMode, LayerKind, NamedMember, Resolver, SourceError, SourceInfo,
};
use pcx_mpq::ExtractionBudget;
use pcx_objects::editor::{EditorReadLimits, path::SlkId};
use pcx_play::{
    map_admission::MapAdmissionBudget, map_scripts_import::MapScriptBudget, scripts::AssetIdentity,
};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("configuration metadata has {actual} UTF-8 bytes, limit is {limit}")]
    Budget { actual: usize, limit: usize },
    #[error("configuration JSON syntax: {0}")]
    Syntax(#[source] serde_json::Error),
    #[error("configuration schema: {0}")]
    Schema(#[source] serde_json::Error),
    #[error("configuration {field}: {problem}")]
    Policy {
        field: String,
        problem: &'static str,
    },
    #[error("binding revision overflows u64: {0}")]
    Revision(#[source] std::num::ParseIntError),
    #[error("configuration mode: {0}")]
    Mode(#[source] pcx_assets::ModeError),
    #[error("configuration {field}: {source}")]
    Path {
        field: String,
        #[source]
        source: pcx_assets::PathError,
    },
    #[error("unsupported required catalog table {path:?}")]
    Table { path: String },
    #[error("configuration source {layer:?}: {source}")]
    Source {
        layer: String,
        #[source]
        source: SourceError,
    },
    #[error("configuration resolver: {0}")]
    Resolver(#[source] pcx_assets::Error),
    #[error("configuration queue: {0}")]
    Queue(#[source] InboxError),
}
fn policy(field: impl Into<String>, problem: &'static str) -> Error {
    Error::Policy {
        field: field.into(),
        problem,
    }
}
fn nonempty(field: impl Into<String>, value: &str) -> Result<(), Error> {
    if value.trim().is_empty() {
        Err(policy(field, "must be nonempty"))
    } else {
        Ok(())
    }
}
#[derive(Deserialize)]
#[serde(try_from = "String")]
enum Schema {
    V1,
}
impl TryFrom<String> for Schema {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value == "pcx-map-config-v1" {
            Ok(Self::V1)
        } else {
            Err("expected exact pcx-map-config-v1 string")
        }
    }
}
#[derive(Deserialize)]
#[serde(try_from = "String")]
enum Codec {
    NoHuffman,
}
impl TryFrom<String> for Codec {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value == "no-huffman" {
            Ok(Self::NoHuffman)
        } else {
            Err("expected exact no-huffman string")
        }
    }
}
#[derive(Deserialize)]
#[serde(try_from = "String")]
enum Kind {
    Mpq,
}
impl TryFrom<String> for Kind {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value == "mpq" {
            Ok(Self::Mpq)
        } else {
            Err("expected exact mpq string")
        }
    }
}
#[derive(Deserialize)]
#[serde(tag = "state", rename_all = "lowercase", deny_unknown_fields)]
enum Version {
    Unknown {},
    Known { value: String },
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Info {
    id: String,
    kind: Kind,
    origin: String,
    version: Version,
}
impl Info {
    fn source(&self) -> SourceInfo {
        SourceInfo {
            id: self.id.clone(),
            kind: match self.kind {
                Kind::Mpq => LayerKind::Mpq,
            },
            origin: self.origin.clone(),
            version: match &self.version {
                Version::Unknown {} => None,
                Version::Known { value } => Some(value.clone()),
            },
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Mapping {
    asset: String,
    member: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Extraction {
    max_raw_packed_bytes: usize,
    max_decoded_bytes: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Layer {
    archive_index: usize,
    committed_revision: u32,
    priority: u32,
    info: Info,
    members: Vec<Mapping>,
    extraction: Extraction,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Identity {
    source: String,
    edition: String,
    version: String,
}
impl Identity {
    fn validate(&self, field: &str) -> Result<(), Error> {
        for (name, value) in [
            ("source", &self.source),
            ("edition", &self.edition),
            ("version", &self.version),
        ] {
            nonempty(format!("{field}.{name}"), value)?;
        }
        Ok(())
    }
    fn owned(&self) -> AssetIdentity {
        AssetIdentity {
            source: self.source.clone(),
            edition: self.edition.clone(),
            version: self.version.clone(),
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Role {
    source_layer: String,
    path: String,
    label: String,
    identity: Identity,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    identity: Identity,
    label: String,
    source_layers: Vec<String>,
    required_catalog_tables: Vec<String>,
    required_fields: Vec<[u8; 4]>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reader {
    max_listed_paths: usize,
    max_asset_bytes: usize,
    max_aggregate_input_bytes: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Overlay {
    unit_bytes: usize,
    item_bytes: usize,
    aggregate_bytes: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Scripts {
    max_script_bytes: usize,
    max_wts_bytes: usize,
    max_archive_bytes: usize,
    max_archive_table_bytes: usize,
    max_raw_packed_bytes: usize,
    max_decoded_bytes: usize,
    max_prelude_bytes: usize,
    max_aggregate_input_bytes: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Admission {
    scripts: Scripts,
    terrain_bytes: usize,
    info_bytes: usize,
    pathing_bytes: usize,
    units_bytes: usize,
    aggregate_input_bytes: usize,
    required_fields: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Limits {
    reader: Reader,
    overlay: Overlay,
    admission: Admission,
    max_map_file_bytes: usize,
    max_resolver_input_bytes: usize,
}
impl Limits {
    fn owned(&self) -> BindingLimits {
        let s = &self.admission.scripts;
        let a = &self.admission;
        BindingLimits {
            reader: EditorReadLimits {
                max_listed_paths: self.reader.max_listed_paths,
                max_asset_bytes: self.reader.max_asset_bytes,
                max_aggregate_input_bytes: self.reader.max_aggregate_input_bytes,
            },
            overlay: OverlayLimits {
                unit_bytes: self.overlay.unit_bytes,
                item_bytes: self.overlay.item_bytes,
                aggregate_bytes: self.overlay.aggregate_bytes,
            },
            admission: MapAdmissionBudget {
                scripts: MapScriptBudget {
                    max_script_bytes: s.max_script_bytes,
                    max_wts_bytes: s.max_wts_bytes,
                    max_archive_bytes: s.max_archive_bytes,
                    max_archive_table_bytes: s.max_archive_table_bytes,
                    max_raw_packed_bytes: s.max_raw_packed_bytes,
                    max_decoded_bytes: s.max_decoded_bytes,
                    max_prelude_bytes: s.max_prelude_bytes,
                    max_aggregate_input_bytes: s.max_aggregate_input_bytes,
                },
                terrain_bytes: a.terrain_bytes,
                info_bytes: a.info_bytes,
                pathing_bytes: a.pathing_bytes,
                units_bytes: a.units_bytes,
                aggregate_input_bytes: a.aggregate_input_bytes,
                required_fields: a.required_fields,
            },
            max_map_file_bytes: self.max_map_file_bytes,
            max_resolver_input_bytes: self.max_resolver_input_bytes,
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Metadata {
    schema: Schema,
    codec: Codec,
    mode: String,
    binding_revision: String,
    sources: Vec<Layer>,
    map: Role,
    common: Role,
    blizzard: Role,
    catalog: Catalog,
    limits: Limits,
}
fn role(role: &Role, field: &str, sources: &BTreeMap<String, SourceInfo>) -> Result<Member, Error> {
    role.identity.validate(&format!("{field}.identity"))?;
    nonempty(format!("{field}.label"), &role.label)?;
    AssetPath::new(&role.path).map_err(|source| Error::Path {
        field: format!("{field}.path"),
        source,
    })?;
    let source = sources.get(&role.source_layer).ok_or_else(|| {
        policy(
            format!("{field}.source_layer"),
            "must name an explicit source layer",
        )
    })?;
    Ok(Member {
        path: role.path.clone(),
        label: role.label.clone(),
        identity: role.identity.owned(),
        source: source.clone(),
    })
}
/// Metadata/source factory boundary. All schema/semantic policy validation completes
/// before the first factory call; factory/resolver and inbox causes remain typed.
/// This does not include an already-performed wasm-bindgen String ingress copy.
pub fn decode_and_enqueue(
    text: &str,
    max_metadata_bytes: usize,
    inbox: &ConfigurationInbox,
    mut factory: impl FnMut(
        usize,
        u32,
        SourceInfo,
        Vec<NamedMember>,
        ExtractionBudget,
    ) -> Result<
        crate::browser_archive_source::CommittedArchiveSource,
        SourceError,
    >,
) -> Result<u32, Error> {
    if text.len() > max_metadata_bytes {
        return Err(Error::Budget {
            actual: text.len(),
            limit: max_metadata_bytes,
        });
    }
    let raw: Metadata = serde_json::from_str(text).map_err(|error| {
        if error.is_data() {
            Error::Schema(error)
        } else {
            Error::Syntax(error)
        }
    })?;
    let Schema::V1 = raw.schema;
    let Codec::NoHuffman = raw.codec;
    if raw.binding_revision.is_empty()
        || !raw.binding_revision.bytes().all(|b| b.is_ascii_digit())
        || (raw.binding_revision.len() > 1 && raw.binding_revision.starts_with('0'))
    {
        return Err(policy(
            "binding_revision",
            "must be canonical ASCII decimal u64 text",
        ));
    }
    let revision = raw
        .binding_revision
        .parse::<u64>()
        .map_err(Error::Revision)?;
    let mode: GameMode = raw.mode.parse().map_err(Error::Mode)?;
    if raw.sources.is_empty() {
        return Err(Error::Resolver(pcx_assets::Error::NoLayers));
    }
    let mut sources = BTreeMap::new();
    let mut priorities = BTreeMap::new();
    for layer in &raw.sources {
        nonempty("source.id", &layer.info.id)?;
        nonempty("source.origin", &layer.info.origin)?;
        if let Version::Known { value } = &layer.info.version {
            nonempty("source.version.value", value)?;
        }
        if sources
            .insert(layer.info.id.clone(), layer.info.source())
            .is_some()
        {
            return Err(Error::Resolver(pcx_assets::Error::AmbiguousLayer {
                layer: layer.info.id.clone(),
            }));
        }
        if let Some(previous) = priorities.insert(layer.priority, layer.info.id.clone()) {
            return Err(Error::Resolver(pcx_assets::Error::AmbiguousPriority {
                priority: layer.priority,
                layers: [previous, layer.info.id.clone()],
            }));
        }
        let mut names = BTreeMap::new();
        for mapping in &layer.members {
            let path = AssetPath::new(&mapping.asset).map_err(|source| Error::Path {
                field: format!("source {} member", layer.info.id),
                source,
            })?;
            if let Some(previous) = names.insert(path.clone(), mapping.asset.clone()) {
                return Err(Error::Resolver(pcx_assets::Error::AmbiguousPath {
                    layer: layer.info.id.clone(),
                    path,
                    candidates: vec![previous, mapping.asset.clone()],
                }));
            }
            if mapping.member.is_empty() || mapping.member.chars().any(char::is_control) {
                return Err(policy(
                    "source.member",
                    "must be a nonempty exact locator without control characters",
                ));
            }
        }
    }
    let map = role(&raw.map, "map", &sources)?;
    let common = role(&raw.common, "common", &sources)?;
    let blizzard = role(&raw.blizzard, "blizzard", &sources)?;
    raw.catalog.identity.validate("catalog.identity")?;
    nonempty("catalog.label", &raw.catalog.label)?;
    if raw.catalog.source_layers.is_empty() {
        return Err(policy(
            "catalog.source_layers",
            "must supply catalog provenance",
        ));
    }
    let mut catalog_sources = Vec::new();
    let mut ids = BTreeSet::new();
    for id in &raw.catalog.source_layers {
        if !ids.insert(id) {
            return Err(policy("catalog.source_layers", "duplicate source layer"));
        }
        catalog_sources.push(
            sources
                .get(id)
                .ok_or_else(|| policy("catalog.source_layers", "unknown source layer"))?
                .clone(),
        );
    }
    let mut tables = Vec::new();
    for path in &raw.catalog.required_catalog_tables {
        tables.push(
            *SlkId::ALL
                .iter()
                .find(|id| id.path() == path)
                .ok_or_else(|| Error::Table { path: path.clone() })?,
        );
    }
    if raw.catalog.required_fields.len() > raw.limits.admission.required_fields {
        return Err(policy(
            "catalog.required_fields",
            "count exceeds explicit admission field limit",
        ));
    }
    let mut layers = Vec::new();
    for layer in raw.sources {
        let info = layer.info.source();
        let id = info.id.clone();
        let names = layer
            .members
            .into_iter()
            .map(|m| NamedMember {
                asset: m.asset,
                member: m.member,
            })
            .collect();
        let source = factory(
            layer.archive_index,
            layer.committed_revision,
            info,
            names,
            ExtractionBudget {
                max_raw_packed_bytes: layer.extraction.max_raw_packed_bytes,
                max_decoded_bytes: layer.extraction.max_decoded_bytes,
            },
        )
        .map_err(|source| Error::Source { layer: id, source })?;
        layers.push(ArchiveLayer {
            priority: layer.priority,
            source: Box::new(source),
        });
    }
    let resolver = Resolver::new(mode, None, layers).map_err(Error::Resolver)?;
    inbox
        .enqueue(Configuration {
            resolver,
            map,
            common,
            blizzard,
            catalog_identity: raw.catalog.identity.owned(),
            catalog_label: raw.catalog.label,
            catalog_sources,
            required_catalog_tables: tables,
            required_fields: raw.catalog.required_fields,
            revision,
            limits: raw.limits.owned(),
        })
        .map_err(Error::Queue)
}

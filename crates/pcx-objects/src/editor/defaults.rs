//! Lossless stock tables, with source identity and typed lookup failures.
//!
//! These are shipped-byte defaults, not reconstructed native defaults. The
//! metadata's extra columns remain available verbatim. A scalar metadata join
//! does not establish the native meaning of repeat/data/applicability selectors.
//! Ability rows are keyed by their authored alias, not their shared code.

use std::collections::BTreeMap;
use std::path::Path;

use pcx_mpq::MpqSet;

use super::{
    Editor, EditorReadLimits, FieldDef, FieldKind, FieldSource, SlkId, meta::METADATA_SLK,
};
use crate::Fourcc;

/// Caller-supplied edition/version claim; archive fingerprints are independent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub install: String,
    pub edition_version: String,
}

/// Exact member selected through the supplied archive set's priority order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    pub member: String,
    pub archive: String,
    /// Content fingerprint, not a release identification or cryptographic hash.
    pub fnv1a64: u64,
}

/// Location in the authored input, including profile assignment occurrences.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Location {
    Slk {
        row: u32,
        column: String,
    },
    Profile {
        section: String,
        key: String,
        line: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance<'a> {
    pub identity: &'a Identity,
    pub asset: &'a Asset,
    pub location: Location,
}

/// Text remains verbatim; numeric interpretation requires an explicit type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell<'a> {
    pub text: &'a str,
    /// Malformed authored profile syntax remains visible and cannot be typed.
    pub parse_error: Option<&'a str>,
    pub provenance: Provenance<'a>,
}

/// Lexical values only: no game integer width, scaling, clamping or inheritance.
#[derive(Debug, Clone, PartialEq)]
pub enum Value<'a> {
    Integer(i64),
    Boolean(bool),
    Real(f64),
    IntegerList(Vec<i64>),
    Text(&'a str),
    /// Authored sentinel, preserved without converting it to a number.
    Marker(&'a str),
}

impl<'a> Cell<'a> {
    pub fn typed(&self, kind: FieldKind) -> Result<Value<'a>, Refusal> {
        let raw = self.text.trim();
        let invalid = || Refusal::MalformedValue {
            member: self.provenance.asset.member.clone(),
            location: self.provenance.location.clone(),
            declared: kind.to_string(),
            value: self.text.into(),
        };
        if self.parse_error.is_some() {
            return Err(invalid());
        }
        if matches!(raw, "-" | "_") {
            return Ok(Value::Marker(self.text));
        }
        Ok(match kind {
            FieldKind::Int | FieldKind::AttackBits | FieldKind::VersionFlags => {
                Value::Integer(raw.parse().map_err(|_| invalid())?)
            }
            FieldKind::Bool => Value::Boolean(match raw {
                "0" | "false" | "FALSE" => false,
                "1" | "true" | "TRUE" => true,
                _ => return Err(invalid()),
            }),
            FieldKind::Real | FieldKind::Unreal => {
                let value: f64 = raw.parse().map_err(|_| invalid())?;
                if !value.is_finite() {
                    return Err(invalid());
                }
                Value::Real(value)
            }
            FieldKind::IntList => Value::IntegerList(
                raw.split(',')
                    .map(|value| value.trim().parse().map_err(|_| invalid()))
                    .collect::<Result<_, _>>()?,
            ),
            FieldKind::Char | FieldKind::Token => Value::Text(self.text),
        })
    }
}

/// Separate from ObjectKind so existing importer exhaustiveness is unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Family {
    Unit,
    Item,
    Ability,
    Upgrade,
    Buff,
    Destructable,
    Doodad,
}

impl Family {
    pub const ALL: &'static [Self] = &[
        Self::Unit,
        Self::Item,
        Self::Ability,
        Self::Upgrade,
        Self::Buff,
        Self::Destructable,
        Self::Doodad,
    ];

    pub fn tables(self) -> &'static [SlkId] {
        match self {
            Self::Unit => &[
                SlkId::UnitData,
                SlkId::UnitUi,
                SlkId::UnitBalance,
                SlkId::UnitWeapons,
                SlkId::UnitAbilities,
            ],
            Self::Item => &[SlkId::ItemData],
            Self::Ability => &[SlkId::AbilityData],
            Self::Upgrade => &[SlkId::UpgradeData],
            Self::Buff => &[SlkId::AbilityBuffData],
            Self::Destructable => &[SlkId::DestructableData],
            Self::Doodad => &[SlkId::Doodads],
        }
    }

    pub fn metadata(self) -> SlkId {
        match self {
            Self::Unit | Self::Item => SlkId::UnitMetaData,
            Self::Ability => SlkId::AbilityMetaData,
            Self::Upgrade => SlkId::UpgradeMetaData,
            Self::Buff => SlkId::AbilityBuffMetaData,
            Self::Destructable => SlkId::DestructableMetaData,
            Self::Doodad => SlkId::DoodadMetaData,
        }
    }
}

/// Missing identities/files/records are visible instead of silently fabricated.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Unresolved {
    #[error("explicit install and edition/version provenance is required")]
    Identity,
    #[error("{member}: {detail}")]
    Asset { member: String, detail: String },
    #[error("{member} line {line}: {detail}")]
    ProfileLine {
        member: String,
        line: usize,
        /// Original line retained locally, including whitespace and malformed syntax.
        text: String,
        detail: String,
    },
    #[error("{member} row {row}: {detail}")]
    Row {
        member: String,
        row: u32,
        detail: String,
    },
    #[error("{family:?}: no shipped object {rawcode}")]
    Object { family: Family, rawcode: String },
}

/// No missing cell, unsupported selector or native behavior receives a default.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refusal {
    #[error("{member} {rawcode}: no authored cell {column}")]
    MissingCell {
        member: String,
        rawcode: String,
        column: String,
    },
    #[error("{member}: {declared} value {value:?} is malformed at {location:?}")]
    MalformedValue {
        member: String,
        location: Location,
        declared: String,
        /// Complete untrimmed authored text, owned without spare capacity.
        value: Box<str>,
    },
    #[error("no {family:?} metadata for field {fourcc:?}")]
    Metadata { family: Family, fourcc: Fourcc },
    #[error("field {fourcc:?}: native selector/applicability mapping is unverified: {detail}")]
    Mapping { fourcc: Fourcc, detail: String },
    #[error("{rawcode}: profile {key} has conflicting assignments in {members:?}")]
    AmbiguousProfile {
        rawcode: String,
        key: String,
        members: Vec<String>,
    },
    #[error("{rawcode}: profile {key} element {index} was not authored")]
    ProfileIndex {
        rawcode: String,
        key: String,
        index: usize,
    },
    #[error("{behavior:?}: no applicable native evidence supplied")]
    NativeEvidence { behavior: Behavior },
}

/// Input data alone does not establish these executable behaviors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Behavior {
    EngineDefaults,
    SelectorApplicability,
    TechTreeEvaluation,
    AvailabilityEvaluation,
    RandomUnitSelection,
    RandomItemSelection,
    EditorStatParity,
}

#[derive(Debug)]
struct Assignment {
    value: String,
    raw: String,
    line: usize,
    parse_error: Option<String>,
}

pub(crate) struct ParsedProfile {
    sections: Sections,
    unresolved: Vec<Unresolved>,
}

type Sections = BTreeMap<String, BTreeMap<String, Vec<Assignment>>>;

#[derive(Debug)]
struct Profile {
    member: String,
    families: Vec<Family>,
    sections: Sections,
}

/// One object keeps all contributing row identities, without copying its stats.
#[derive(Debug)]
struct Rows {
    spelling: String,
    tables: BTreeMap<SlkId, u32>,
}

/// Complete shipped rows and profile assignments, backed by the decoded Editor.
#[derive(Debug)]
pub struct DefaultTables {
    editor: Editor,
    identity: Identity,
    assets: BTreeMap<String, Asset>,
    profiles: Vec<Profile>,
    objects: BTreeMap<Family, BTreeMap<String, Rows>>,
    metadata_rows: BTreeMap<Fourcc, (SlkId, u32)>,
    unresolved: Vec<Unresolved>,
}

impl DefaultTables {
    pub fn open(install: &Path, edition_version: impl Into<String>) -> Result<Self, Unresolved> {
        let identity = Identity {
            install: install.display().to_string(),
            edition_version: edition_version.into(),
        };
        if identity.install.trim().is_empty() || identity.edition_version.trim().is_empty() {
            return Err(Unresolved::Identity);
        }
        let mut set = MpqSet::open_wc3(install).map_err(|error| Unresolved::Asset {
            member: identity.install.clone(),
            detail: error.to_string(),
        })?;
        Self::open_in(&mut set, install, identity)
    }

    pub fn open_in(
        set: &mut MpqSet,
        install: &Path,
        identity: Identity,
    ) -> Result<Self, Unresolved> {
        if identity.install.trim().is_empty() || identity.edition_version.trim().is_empty() {
            return Err(Unresolved::Identity);
        }
        let editor = Editor::open_in(set, install).map_err(|error| Unresolved::Asset {
            member: install.display().to_string(),
            detail: error.to_string(),
        })?;
        let listed = set.file_list().map_err(|error| Unresolved::Asset {
            member: "(listfile) union".into(),
            detail: error.to_string(),
        })?;
        let mut assets = BTreeMap::new();
        let mut unresolved = Vec::new();
        for id in SlkId::ALL {
            match read_asset(set, id.path()) {
                Ok((asset, _)) => {
                    assets.insert(id.path().to_string(), asset);
                }
                Err(issue) => unresolved.push(issue),
            }
        }
        let mut profiles = Vec::new();
        for member in listed {
            let families = profile_families(&member);
            if families.is_empty() {
                continue;
            }
            let (asset, bytes) = match read_asset(set, &member) {
                Ok(source) => source,
                Err(issue) => {
                    unresolved.push(issue);
                    continue;
                }
            };
            assets.insert(member.clone(), asset);
            match parse_profile(&bytes, &member) {
                Ok(parsed) => {
                    unresolved.extend(parsed.unresolved);
                    profiles.push(Profile {
                        member,
                        families,
                        sections: parsed.sections,
                    });
                }
                Err(issue) => unresolved.push(issue),
            }
        }
        Self::assemble(editor, identity, assets, profiles, unresolved)
    }

    /// Decode caller-supplied table and profile bytes.
    ///
    /// The editor reads only those bytes. Archive priority and listfile
    /// selection stay with the caller. Profile members use the same family
    /// names as an install read. Counts and aggregate bytes (including member
    /// names and copied provenance) are checked before parsing or copying inputs.
    #[cfg_attr(
        feature = "profile-cpu",
        tracing::instrument(skip_all, target = "pcx_cpu", name = "objects.parse_tables")
    )]
    pub fn from_bytes(
        identity: Identity,
        tables: &[(&str, &[u8])],
        profiles: &[(&str, &[u8])],
        limits: EditorReadLimits,
    ) -> Result<Self, Unresolved> {
        if identity.install.trim().is_empty() || identity.edition_version.trim().is_empty() {
            return Err(Unresolved::Identity);
        }
        let fail = || Unresolved::Asset {
            member: identity.install.clone(),
            detail: "supplied table/profile input budget exceeded".into(),
        };
        let count = tables.len().checked_add(profiles.len()).ok_or_else(fail)?;
        if count > limits.max_listed_paths {
            return Err(fail());
        }
        let mut total = identity
            .install
            .len()
            .checked_add(identity.edition_version.len())
            .ok_or_else(fail)?;
        for &(member, bytes) in tables.iter().chain(profiles) {
            if bytes.len() > limits.max_asset_bytes || member.len() > limits.max_asset_bytes {
                return Err(fail());
            }
            total = total
                .checked_add(bytes.len())
                .and_then(|n| n.checked_add(member.len()))
                .and_then(|n| n.checked_add(identity.install.len()))
                .ok_or_else(fail)?;
            if total > limits.max_aggregate_input_bytes {
                return Err(fail());
            }
        }
        if total > limits.max_aggregate_input_bytes {
            return Err(fail());
        }
        let mut supplied: BTreeMap<String, &[u8]> = BTreeMap::new();
        let mut listed = Vec::new();
        for &(member, bytes) in tables {
            if supplied.insert(member.to_string(), bytes).is_some() {
                return Err(Unresolved::Asset {
                    member: member.into(),
                    detail: "duplicate supplied member".into(),
                });
            }
            if !SlkId::ALL.iter().any(|id| id.path() == member) {
                return Err(Unresolved::Asset {
                    member: member.into(),
                    detail: "not a stock table path".into(),
                });
            }
            listed.push(member.to_string());
        }
        let editor = Editor::from_reader(
            Path::new(&identity.install),
            &listed,
            limits,
            |member| -> Result<Option<Vec<u8>>, std::io::Error> {
                Ok(supplied.get(member).map(|bytes| bytes.to_vec()))
            },
        )
        .map_err(|error| Unresolved::Asset {
            member: identity.install.clone(),
            detail: error.to_string(),
        })?;
        let mut assets = BTreeMap::new();
        for id in SlkId::ALL {
            let Some(bytes) = supplied.get(id.path()) else {
                continue;
            };
            if editor.table(*id).is_none() {
                continue;
            }
            assets.insert(
                id.path().to_string(),
                Asset {
                    member: id.path().into(),
                    archive: identity.install.clone(),
                    fnv1a64: pcx_mpq::fnv1a64(bytes),
                },
            );
        }
        let mut unresolved = Vec::new();
        let mut decoded = Vec::new();
        for &(member, bytes) in profiles {
            let families = profile_families(member);
            if families.is_empty() {
                return Err(Unresolved::Asset {
                    member: member.into(),
                    detail: "not a unit, item, ability, or upgrade profile".into(),
                });
            }
            if assets.contains_key(member) {
                return Err(Unresolved::Asset {
                    member: member.into(),
                    detail: "duplicate supplied member".into(),
                });
            }
            assets.insert(
                member.to_string(),
                Asset {
                    member: member.into(),
                    archive: identity.install.clone(),
                    fnv1a64: pcx_mpq::fnv1a64(bytes),
                },
            );
            let parsed = parse_profile(bytes, member)?;
            unresolved.extend(parsed.unresolved);
            decoded.push(Profile {
                member: member.to_string(),
                families,
                sections: parsed.sections,
            });
        }
        Self::assemble(editor, identity, assets, decoded, unresolved)
    }

    fn assemble(
        editor: Editor,
        identity: Identity,
        assets: BTreeMap<String, Asset>,
        profiles: Vec<Profile>,
        mut unresolved: Vec<Unresolved>,
    ) -> Result<Self, Unresolved> {
        if identity.install.trim().is_empty() || identity.edition_version.trim().is_empty() {
            return Err(Unresolved::Identity);
        }
        let mut objects = BTreeMap::new();
        for &family in Family::ALL {
            let mut records: BTreeMap<String, Rows> = BTreeMap::new();
            for &id in family.tables() {
                let Some(table) = editor.table(id) else {
                    continue;
                };
                if !assets.contains_key(id.path()) {
                    unresolved.push(Unresolved::Asset {
                        member: id.path().into(),
                        detail: "missing source fingerprint".into(),
                    });
                    continue;
                }
                if table
                    .anomalies()
                    .iter()
                    .any(|anomaly| anomaly.severity == pcx_slq::Severity::Error)
                {
                    unresolved.push(Unresolved::Asset {
                        member: id.path().into(),
                        detail: "SYLK has unrecoverable anomalies".into(),
                    });
                    continue;
                }
                // Object identities are format columns, independent of header order.
                // AbilityData's alias and code are distinct.
                let key = match id {
                    SlkId::AbilityData | SlkId::AbilityBuffData => Some("alias"),
                    SlkId::DestructableData => Some("DestructableID"),
                    SlkId::Doodads => Some("doodID"),
                    _ => id.id_column(),
                };
                let column = if id == SlkId::UnitWeapons {
                    match super::keys::weapon_identity_column(&editor.tables) {
                        Ok(column) => Some(column),
                        Err(detail) => {
                            unresolved.push(Unresolved::Row {
                                member: id.path().into(),
                                row: 1,
                                detail,
                            });
                            continue;
                        }
                    }
                } else {
                    key.and_then(|key| table.column_of(key))
                };
                let Some(column) = column else {
                    unresolved.push(Unresolved::Row {
                        member: id.path().into(),
                        row: 1,
                        detail: format!(
                            "missing identity header {key:?}; authored first header {:?}",
                            table.headers().next().map(|(_, name)| name)
                        ),
                    });
                    continue;
                };
                let mut seen = BTreeMap::new();
                for row in table.data_rows().filter(|row| !row.is_empty()) {
                    let Some(rawcode) = row.get(column) else {
                        unresolved.push(Unresolved::Row {
                            member: id.path().into(),
                            row: row.index(),
                            detail: "missing identity cell".into(),
                        });
                        continue;
                    };
                    if rawcode.len() != 4 || !rawcode.is_ascii() {
                        unresolved.push(Unresolved::Row {
                            member: id.path().into(),
                            row: row.index(),
                            detail: "identity is not a fourcc".into(),
                        });
                        continue;
                    }
                    let record = records
                        .entry(rawcode.to_ascii_lowercase())
                        .or_insert_with(|| Rows {
                            spelling: rawcode.to_string(),
                            tables: BTreeMap::new(),
                        });
                    let folded = rawcode.to_ascii_lowercase();
                    if let Some(previous) = seen.insert(folded.clone(), row.index()) {
                        record.tables.remove(&id);
                        unresolved.push(Unresolved::Row {
                            member: id.path().into(),
                            row: row.index(),
                            detail: format!("duplicate identity {rawcode}, also row {previous}"),
                        });
                    } else {
                        record.tables.insert(id, row.index());
                    }
                }
            }
            objects.insert(family, records);
        }
        let mut metadata_rows = BTreeMap::new();
        for &id in METADATA_SLK {
            let Some(table) = editor.table(id) else {
                continue;
            };
            for row in table.data_rows().filter(|row| !row.is_empty()) {
                let code = row.by_name("ID").unwrap_or("");
                let Ok(fourcc) = <Fourcc>::try_from(code.as_bytes()) else {
                    unresolved.push(Unresolved::Row {
                        member: id.path().into(),
                        row: row.index(),
                        detail: format!("metadata ID is not a fourcc: {code:?}"),
                    });
                    continue;
                };
                if metadata_rows.insert(fourcc, (id, row.index())).is_some() {
                    return Err(Unresolved::Row {
                        member: id.path().into(),
                        row: row.index(),
                        detail: "duplicate metadata fourcc".into(),
                    });
                }
            }
        }
        Ok(Self {
            editor,
            identity,
            assets,
            profiles,
            objects,
            metadata_rows,
            unresolved,
        })
    }

    pub fn editor(&self) -> &Editor {
        &self.editor
    }
    pub fn identity(&self) -> &Identity {
        &self.identity
    }
    pub fn assets(&self) -> &BTreeMap<String, Asset> {
        &self.assets
    }
    pub fn unresolved(&self) -> &[Unresolved] {
        &self.unresolved
    }

    pub fn rawcodes(&self, family: Family) -> impl Iterator<Item = &str> {
        self.objects
            .get(&family)
            .into_iter()
            .flat_map(|rows| rows.values())
            .filter(|rows| !rows.tables.is_empty())
            .map(|rows| rows.spelling.as_str())
    }

    pub fn object(&self, family: Family, rawcode: &str) -> Result<Object<'_>, Unresolved> {
        let rows = self
            .objects
            .get(&family)
            .and_then(|rows| rows.get(&rawcode.to_ascii_lowercase()))
            .filter(|rows| !rows.tables.is_empty())
            .ok_or_else(|| Unresolved::Object {
                family,
                rawcode: rawcode.into(),
            })?;
        Ok(Object {
            tables: self,
            family,
            rows,
        })
    }

    /// All authored metadata attributes, including applicability and selectors.
    pub fn metadata(&self, fourcc: Fourcc) -> Option<Metadata<'_>> {
        let &(id, row) = self.metadata_rows.get(&fourcc)?;
        Some(Metadata {
            tables: self,
            id,
            row,
        })
    }

    pub fn behavior(&self, behavior: Behavior) -> Refusal {
        Refusal::NativeEvidence { behavior }
    }

    fn cell(&self, id: SlkId, row: u32, column: &str) -> Option<Cell<'_>> {
        let table = self.editor.table(id)?;
        let text = table.by_name(row, column)?;
        let asset = self.assets.get(id.path())?;
        Some(Cell {
            text,
            parse_error: None,
            provenance: Provenance {
                identity: &self.identity,
                asset,
                location: Location::Slk {
                    row,
                    column: column.into(),
                },
            },
        })
    }
}

/// Extended metadata is read from the original row; FieldDef remains unchanged.
#[derive(Debug, Clone, Copy)]
pub struct Metadata<'a> {
    tables: &'a DefaultTables,
    id: SlkId,
    row: u32,
}

impl<'a> Metadata<'a> {
    pub fn attribute(self, column: &str) -> Option<Cell<'a>> {
        self.tables.cell(self.id, self.row, column)
    }
    pub fn attributes(self) -> Vec<Cell<'a>> {
        self.tables
            .editor
            .table(self.id)
            .into_iter()
            .flat_map(|table| table.headers())
            .filter_map(|(_, column)| self.attribute(column))
            .collect()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Object<'a> {
    tables: &'a DefaultTables,
    family: Family,
    rows: &'a Rows,
}

impl<'a> Object<'a> {
    pub fn rawcode(self) -> &'a str {
        &self.rows.spelling
    }
    pub fn family(self) -> Family {
        self.family
    }

    /// Includes cells not named by an object-editor metadata fourcc.
    pub fn cells(self) -> Vec<Cell<'a>> {
        self.rows
            .tables
            .iter()
            .flat_map(|(&id, &row)| {
                self.tables
                    .editor
                    .table(id)
                    .into_iter()
                    .flat_map(move |table| {
                        table
                            .headers()
                            .filter_map(move |(_, column)| self.tables.cell(id, row, column))
                    })
            })
            .collect()
    }

    pub fn cell(self, id: SlkId, column: &str) -> Result<Cell<'a>, Refusal> {
        self.rows
            .tables
            .get(&id)
            .and_then(|&row| self.tables.cell(id, row, column))
            .ok_or_else(|| Refusal::MissingCell {
                member: id.path().into(),
                rawcode: self.rawcode().into(),
                column: column.into(),
            })
    }

    /// Declared level count, without inferring effective levels or fallback.
    pub fn level_count(self) -> Result<Cell<'a>, Refusal> {
        let (id, column) = match self.family {
            Family::Ability => (SlkId::AbilityData, "levels"),
            Family::Upgrade => (SlkId::UpgradeData, "maxlevel"),
            _ => {
                return Err(Refusal::Mapping {
                    fourcc: *b"\0\0\0\0",
                    detail: "this family has no supported level-count column".into(),
                });
            }
        };
        self.cell(id, column)
    }

    /// Exact authored level column: preserves columns beyond the declared count.
    /// No native repeat/fallback semantics are assigned to a missing column.
    pub fn level_cell(self, stem: &str, level: u32) -> Result<Cell<'a>, Refusal> {
        self.cell(SlkId::AbilityData, &format!("{stem}{level}"))
    }

    /// Groups columns by their authored numeric suffix, including padded levels.
    pub fn level_columns(self) -> BTreeMap<u32, Vec<String>> {
        let mut levels: BTreeMap<u32, Vec<String>> = BTreeMap::new();
        if self.family != Family::Ability {
            return levels;
        }
        if let Some(table) = self.tables.editor.table(SlkId::AbilityData) {
            for (_, column) in table.headers() {
                let stem = column.trim_end_matches(|ch: char| ch.is_ascii_digit());
                if let Ok(level) = column[stem.len()..].parse::<u32>() {
                    levels.entry(level).or_default().push(column.to_string());
                }
            }
        }
        levels
    }

    /// Every profile assignment is kept, including conflicts and exact sources.
    pub fn profile(self, key: &str) -> Vec<Cell<'a>> {
        let rawcode = self.rawcode().to_ascii_lowercase();
        let key = key.to_ascii_lowercase();
        let mut cells = Vec::new();
        for profile in &self.tables.profiles {
            if !profile.families.contains(&self.family) {
                continue;
            }
            let Some(assignments) = profile
                .sections
                .get(&rawcode)
                .and_then(|keys| keys.get(&key))
            else {
                continue;
            };
            let Some(asset) = self.tables.assets.get(&profile.member) else {
                continue;
            };
            for assignment in assignments {
                cells.push(Cell {
                    text: &assignment.value,
                    parse_error: assignment.parse_error.as_deref(),
                    provenance: Provenance {
                        identity: &self.tables.identity,
                        asset,
                        location: Location::Profile {
                            section: rawcode.clone(),
                            key: key.clone(),
                            line: assignment.line,
                        },
                    },
                });
            }
        }
        cells
    }

    pub fn profile_keys(self) -> Vec<&'a str> {
        let rawcode = self.rawcode().to_ascii_lowercase();
        let mut keys: Vec<_> = self
            .tables
            .profiles
            .iter()
            .filter(|profile| profile.families.contains(&self.family))
            .filter_map(|profile| profile.sections.get(&rawcode))
            .flat_map(|keys| keys.keys().map(String::as_str))
            .collect();
        keys.sort_unstable();
        keys.dedup();
        keys
    }

    /// Editor categories (including tech) come from the runtime metadata.
    /// Applicability and graph evaluation remain separate from the byte join.
    pub fn fields_in_category(
        self,
        category: &str,
    ) -> Vec<(&'a FieldDef, Result<MappedField<'a>, Refusal>)> {
        self.tables
            .editor
            .fields()
            .iter()
            .filter(|def| {
                def.from == self.family.metadata() && def.category.eq_ignore_ascii_case(category)
            })
            .map(|def| (def, self.field(def.fourcc)))
            .collect()
    }

    /// Join only exact scalar metadata columns/profile slots. Repeated fields
    /// and specific-object applicability require separate native evidence.
    pub fn field(self, fourcc: Fourcc) -> Result<MappedField<'a>, Refusal> {
        let def = self
            .tables
            .editor
            .field(fourcc)
            .filter(|def| def.from == self.family.metadata())
            .ok_or(Refusal::Metadata {
                family: self.family,
                fourcc,
            })?;
        let metadata = self.tables.metadata(fourcc).ok_or(Refusal::Metadata {
            family: self.family,
            fourcc,
        })?;
        for selector in [
            "repeat",
            "data",
            "appendIndex",
            "useSpecific",
            "notSpecific",
        ] {
            if let Some(cell) = metadata.attribute(selector)
                && !matches!(cell.text.trim(), "" | "0" | "-" | "_")
            {
                return Err(Refusal::Mapping {
                    fourcc,
                    detail: format!("{selector}={}", cell.text),
                });
            }
        }
        let cell = match &def.source {
            FieldSource::Column { slk, column } => self.cell(*slk, column)?,
            FieldSource::Profile { key, index, .. } => {
                // Duplicate schema rows do not create extra comma slots.
                let slots = self
                    .tables
                    .editor
                    .fields()
                    .iter()
                    .filter_map(|other| match &other.source {
                        FieldSource::Profile {
                            key: other_key,
                            index,
                            ..
                        } if other.from == def.from && other_key == key => u32::try_from(*index)
                            .ok()
                            .and_then(|index| index.checked_add(1)),
                        _ => None,
                    })
                    .max()
                    .unwrap_or(1);
                self.profile_element(key, *index, slots)?
            }
            FieldSource::Unknown { slk, field } => {
                return Err(Refusal::Mapping {
                    fourcc,
                    detail: format!("unsupported source {slk}/{field}"),
                });
            }
        };
        let value = cell.typed(def.kind)?;
        Ok(MappedField {
            definition: def,
            metadata,
            cell,
            value,
        })
    }

    fn profile_element(self, key: &str, index: i32, slots: u32) -> Result<Cell<'a>, Refusal> {
        let candidates = self.profile(key);
        let Some(first) = candidates.first() else {
            return Err(Refusal::MissingCell {
                member: "Profile".into(),
                rawcode: self.rawcode().into(),
                column: key.into(),
            });
        };
        if candidates.iter().any(|cell| cell.text != first.text) {
            return Err(Refusal::AmbiguousProfile {
                rawcode: self.rawcode().into(),
                key: key.into(),
                members: candidates
                    .iter()
                    .map(|cell| cell.provenance.asset.member.clone())
                    .collect(),
            });
        }
        if first.parse_error.is_some() || index < 0 || slots <= 1 {
            return Ok(first.clone());
        }
        let Location::Profile { line, .. } = &first.provenance.location else {
            unreachable!()
        };
        let assignment = self
            .tables
            .profiles
            .iter()
            .find(|profile| profile.member == first.provenance.asset.member)
            .and_then(|profile| profile.sections.get(&self.rawcode().to_ascii_lowercase()))
            .and_then(|section| section.get(&key.to_ascii_lowercase()))
            .and_then(|assignments| {
                assignments
                    .iter()
                    .find(|assignment| assignment.line == *line)
            })
            .expect("profile cells retain their assignments");
        let elements = profile_elements(&assignment.raw).map_err(|detail| Refusal::Mapping {
            fourcc: *b"\0\0\0\0",
            detail,
        })?;
        let text = elements
            .get(index as usize)
            .copied()
            .ok_or_else(|| Refusal::ProfileIndex {
                rawcode: self.rawcode().into(),
                key: key.into(),
                index: index as usize,
            })?;
        Ok(Cell {
            text,
            parse_error: None,
            provenance: first.provenance.clone(),
        })
    }
}

#[derive(Debug)]
pub struct MappedField<'a> {
    pub definition: &'a FieldDef,
    pub metadata: Metadata<'a>,
    pub cell: Cell<'a>,
    pub value: Value<'a>,
}

fn read_asset(set: &mut MpqSet, member: &str) -> Result<(Asset, Vec<u8>), Unresolved> {
    let error = |detail: String| Unresolved::Asset {
        member: member.into(),
        detail,
    };
    let (index, _) = set
        .locate(member)
        .ok_or_else(|| error("not present in the supplied archive set".into()))?;
    let archive = set.members()[index].path().display().to_string();
    let bytes = set.extract(member).map_err(|err| error(err.to_string()))?;
    let asset = Asset {
        member: member.into(),
        archive,
        fnv1a64: pcx_mpq::fnv1a64(&bytes),
    };
    Ok((asset, bytes))
}

fn profile_families(member: &str) -> Vec<Family> {
    let path = member.replace('/', "\\").to_ascii_lowercase();
    let Some(name) = path.strip_prefix("units\\") else {
        return Vec::new();
    };
    if name.contains('\\') {
        return Vec::new();
    }
    if name.ends_with("unitfunc.txt") || name.ends_with("unitstrings.txt") {
        vec![Family::Unit]
    } else if matches!(name, "itemfunc.txt" | "itemstrings.txt") {
        vec![Family::Item]
    } else if name.ends_with("abilityfunc.txt") || name.ends_with("abilitystrings.txt") {
        vec![Family::Ability, Family::Buff]
    } else if name.ends_with("upgradefunc.txt") || name.ends_with("upgradestrings.txt") {
        vec![Family::Upgrade]
    } else {
        Vec::new()
    }
}

impl ParsedProfile {
    pub(crate) fn unresolved(&self) -> &[Unresolved] {
        &self.unresolved
    }

    pub(crate) fn has_section(&self, section: &str) -> bool {
        self.sections.contains_key(section)
    }

    pub(crate) fn keys(&self, section: &str) -> Option<Vec<&str>> {
        self.sections
            .get(section)
            .map(|keys| keys.keys().map(String::as_str).collect())
    }

    pub(crate) fn raw_fields(&self, section: &str, key: &str) -> Vec<(usize, &str)> {
        self.sections
            .get(section)
            .and_then(|keys| keys.get(key))
            .map(|items| {
                items
                    .iter()
                    .map(|item| (item.line, item.raw.as_str()))
                    .collect()
            })
            .unwrap_or_default()
    }
}

pub(crate) fn parse_profile(bytes: &[u8], member: &str) -> Result<ParsedProfile, Unresolved> {
    parse_profile_lines(bytes, member, false)
}

/// Config profiles allow inline comments outside quoted values; unit text profiles do not.
pub(crate) fn parse_config_profile(
    bytes: &[u8],
    member: &str,
) -> Result<ParsedProfile, Unresolved> {
    parse_profile_lines(bytes, member, true)
}

fn parse_profile_lines(
    bytes: &[u8],
    member: &str,
    inline_comments: bool,
) -> Result<ParsedProfile, Unresolved> {
    let error = |detail: String| Unresolved::Asset {
        member: member.into(),
        detail,
    };
    let text = std::str::from_utf8(bytes)
        .map_err(|err| error(format!("unsupported profile encoding: {err}")))?;
    let mut sections: Sections = BTreeMap::new();
    let mut unresolved = Vec::new();
    let mut current = None;
    for (line, raw) in text
        .strip_prefix('\u{feff}')
        .unwrap_or(text)
        .lines()
        .enumerate()
    {
        let line = line + 1;
        let authored_line = raw;
        let line_error = |detail: String| Unresolved::ProfileLine {
            member: member.into(),
            line,
            text: authored_line.into(),
            detail,
        };
        let raw = if inline_comments {
            let mut quoted = false;
            let comment = raw.char_indices().find_map(|(index, ch)| {
                if ch == '"' {
                    quoted = !quoted;
                }
                (!quoted && raw[index..].starts_with("//")).then_some(index)
            });
            &raw[..comment.unwrap_or(raw.len())]
        } else {
            raw
        };
        let raw = raw.trim();
        if raw.is_empty() || raw.starts_with("//") {
            continue;
        }
        if raw.starts_with('[') {
            current = match raw.strip_prefix('[').and_then(|raw| raw.strip_suffix(']')) {
                Some(section) if !section.is_empty() => Some(section.to_ascii_lowercase()),
                _ => {
                    unresolved.push(line_error("malformed section".into()));
                    None
                }
            };
            continue;
        }
        let Some((key, value)) = raw.split_once('=') else {
            unresolved.push(line_error("expected assignment".into()));
            continue;
        };
        let Some(section) = current.as_ref() else {
            unresolved.push(line_error("assignment without section".into()));
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        if key.is_empty() {
            unresolved.push(line_error("empty key".into()));
            continue;
        }
        let raw = value.trim();
        let (value, parse_error) = match profile_value(raw) {
            Ok(value) => (value, None),
            Err(detail) => {
                unresolved.push(line_error(detail.clone()));
                (raw, Some(detail))
            }
        };
        sections
            .entry(section.clone())
            .or_default()
            .entry(key)
            .or_default()
            .push(Assignment {
                value: value.into(),
                raw: raw.into(),
                line,
                parse_error,
            });
    }
    Ok(ParsedProfile {
        sections,
        unresolved,
    })
}

/// Quotes shield commas. Backslashes are literal Warcraft paths, not escapes.
pub(crate) fn profile_elements(raw: &str) -> Result<Vec<&str>, String> {
    profile_elements_iter(raw).collect()
}

/// Validate borrowed fields incrementally so callers can bound output growth.
pub(crate) fn profile_elements_iter(raw: &str) -> impl Iterator<Item = Result<&str, String>> {
    let mut tail = Some(raw);
    std::iter::from_fn(move || {
        let raw = tail.take()?;
        let mut quoted = false;
        for (index, ch) in raw.char_indices() {
            if ch == '"' {
                quoted = !quoted;
            } else if ch == ',' && !quoted {
                tail = Some(&raw[index + 1..]);
                return Some(profile_element(&raw[..index]));
            }
        }
        Some(if quoted {
            Err("unterminated profile quote".into())
        } else {
            profile_element(raw)
        })
    })
}

fn profile_value(raw: &str) -> Result<&str, String> {
    let mut fields = profile_elements_iter(raw);
    let first = fields
        .next()
        .expect("even an empty profile has one field")?;
    let mut multiple = false;
    for field in fields {
        field?;
        multiple = true;
    }
    Ok(if multiple { raw } else { first })
}

fn profile_element(raw: &str) -> Result<&str, String> {
    let raw = raw.trim();
    if let Some(inner) = raw.strip_prefix('"').and_then(|raw| raw.strip_suffix('"')) {
        if inner.contains('"') {
            return Err("unsupported embedded profile quote".into());
        }
        Ok(inner)
    } else if raw.contains('"') {
        Err("unsupported profile quote placement".into())
    } else {
        Ok(raw)
    }
}

#[cfg(test)]
mod tests;

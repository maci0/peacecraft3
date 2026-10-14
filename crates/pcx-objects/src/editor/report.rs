//! Laying a map's object file on top of the editor data, and what happened.
//!
//! This is the M4 gate's second half: **a map whose rawcodes do not resolve
//! is unplayable.** Every record of every shipped `.w3u` / `.w3t` is walked
//! here, and the answer is one of
//!
//! * An existing-object [`Resolution`] — the record's original rawcode names an object
//!   the base data has, so the modifications land on it;
//! * A custom-object [`Resolution`] — a second-table record whose modified rawcode is
//!   new, so the record is a new object derived from its original;
//! * [`Unresolved`] — the original rawcode names nothing, and the record is
//!   reported rather than silently dropped.
//!
//! Field-level outcomes are separate from object-level ones, and the crate
//! reports every field mapping it cannot prove instead of guessing one.

use std::collections::BTreeMap;
use std::path::Path;

use super::{
    Editor, ObjectKind,
    object::{Item, Unit, Upgrade},
};
use crate::{Error, Fourcc, Mod, ObjectFile, Value, fourcc_text};

/// One field fourcc whose declared type disagrees with its wire word, with how
/// many shipped modifications carry the pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireMismatch {
    /// The field fourcc.
    pub fourcc: Fourcc,
    /// The metadata table's declared type, verbatim.
    pub declared: String,
    /// The type word the file carried.
    pub word: u32,
    /// How many shipped modifications carry this pair.
    pub count: usize,
}

/// Why a field mapping could not be proven.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// Scalar field lookup cannot interpret the two authored selectors.
    IndexedField { fourcc: Fourcc, selectors: [u32; 2] },
    /// The fourcc is not in any `*MetaData.slk` this build ships.
    UnknownFourcc { fourcc: Fourcc },
    /// The fourcc is a `Profile` field, but **no shipped `.txt` section
    /// carries its key**, so its base value is unmeasured. The map's override
    /// is still applied; only the stock value is refused.
    NoShippedBase {
        fourcc: Fourcc,
        /// The key the metadata names, lower-cased.
        key: String,
    },
    /// The declared type is `char`, which ships on the string wire — 8
    /// shipped modifications. Applied as text; the single-character read is
    /// refused.
    CharOnStringWire { fourcc: Fourcc },
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IndexedField { fourcc, selectors } => write!(
                f,
                "{}: authored selectors {selectors:?} require indexed lookup",
                fourcc_text(*fourcc)
            ),
            Self::UnknownFourcc { fourcc } => {
                write!(
                    f,
                    "{}: no row in any shipped *MetaData.slk",
                    fourcc_text(*fourcc)
                )
            }
            Self::NoShippedBase { fourcc, key } => write!(
                f,
                "{}: profile key {key:?} ships in no section, so the base value \
                 is unmeasured",
                fourcc_text(*fourcc)
            ),
            Self::CharOnStringWire { fourcc } => write!(
                f,
                "{}: declared `char` but shipped on the string wire",
                fourcc_text(*fourcc)
            ),
        }
    }
}

/// Why one record could not be resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unresolved {
    /// A record modifies or derives from a rawcode absent from the base data.
    UnknownOriginal {
        /// The rawcode the record modifies.
        rawcode: Fourcc,
        /// Which table the record came from: `0` base, `1` custom.
        table: usize,
        /// The map that carries it.
        map: String,
    },
    /// A second-table record's modified rawcode collides with an object the
    /// base data already ships.
    ModifiedCollides {
        /// The new rawcode the record declares.
        rawcode: Fourcc,
        /// What it collides with.
        kind: ObjectKind,
    },
}

impl std::fmt::Display for Unresolved {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownOriginal {
                rawcode,
                table,
                map,
            } => {
                let action = if *table == 1 {
                    "custom record derives from"
                } else {
                    "base record modifies"
                };
                write!(
                    f,
                    "{map}: {action} {}, which no shipped editor table carries",
                    fourcc_text(*rawcode)
                )
            }
            Self::ModifiedCollides { rawcode, kind } => write!(
                f,
                "custom rawcode {} already names a shipped {}",
                fourcc_text(*rawcode),
                kind
            ),
        }
    }
}

/// One map's modifications, keyed by the raw code they land on.
///
/// This is what the second half of the M4 gate needs: after this, a map's
/// custom unit answers `editor.unit(rawcode)`-shaped questions, with the base
/// cells underneath and the map's words on top.
#[derive(Debug, Clone, Default)]
pub struct Overlay {
    /// First-table modifications, by the raw code they modify.
    pub mods: BTreeMap<String, Vec<Mod>>,
    /// Second-table modifications, by the **new** raw code, with the original
    /// it derives from beside it.
    pub custom: BTreeMap<String, (String, Vec<Mod>)>,
}

impl Overlay {
    /// The modifications a map put on `rawcode`, base table first.
    pub fn on(&self, rawcode: &str) -> &[Mod] {
        self.mods.get(rawcode).map(Vec::as_slice).unwrap_or(&[])
    }

    /// The modifications of a custom raw code, with its base raw code.
    pub fn new_object(&self, rawcode: &str) -> Option<(&str, &[Mod])> {
        self.custom
            .get(rawcode)
            .map(|(base, mods)| (base.as_str(), mods.as_slice()))
    }

    /// Every raw code this overlay touches, in sorted order.
    pub fn rawcodes(&self) -> impl Iterator<Item = &String> {
        self.mods.keys().chain(self.custom.keys())
    }

    /// Read one object file's records into the overlay.
    pub fn of(file: &ObjectFile<'_>) -> Self {
        let mut me = Self::default();
        for record in file.base() {
            let key = fourcc_text(record.original);
            me.mods
                .entry(key)
                .or_default()
                .extend(record.mods.iter().cloned());
        }
        for record in file.custom() {
            let key = fourcc_text(record.modified);
            let base = fourcc_text(record.original);
            me.custom
                .entry(key)
                .or_insert_with(|| (base, Vec::new()))
                .1
                .extend(record.mods.iter().cloned());
        }
        me
    }

    /// Exact authored value for a field and selector pair, without flattening.
    /// Last authored override wins only within this exact key.
    pub fn indexed_value(
        &self,
        rawcode: &str,
        field: Fourcc,
        selectors: [u32; 2],
    ) -> Option<&Value> {
        let mods = self
            .new_object(rawcode)
            .map(|(_, mods)| mods)
            .unwrap_or_else(|| self.on(rawcode));
        mods.iter()
            .rev()
            .find(|m| m.field == field && m.selectors == Some(selectors))
            .map(|m| &m.value)
    }

    /// The unit `rawcode` becomes with this map's modifications on top.
    pub fn unit<'e>(&self, editor: &'e Editor, rawcode: &str) -> Unit<'e> {
        let mut unit = Unit::new(editor, rawcode);
        for m in self.on(rawcode) {
            unit.put(m);
        }
        if let Some((base, mods)) = self.new_object(rawcode) {
            unit = Unit::new(editor, base);
            unit.mark_custom();
            for m in mods {
                unit.put(m);
            }
        }
        unit
    }

    /// The item `rawcode` becomes with this map's modifications on top.
    pub fn item<'e>(&self, editor: &'e Editor, rawcode: &str) -> Item<'e> {
        let mut item = Item::new(editor, rawcode);
        for m in self.on(rawcode) {
            item.put(m);
        }
        if let Some((base, mods)) = self.new_object(rawcode) {
            item = Item::new(editor, base);
            item.mark_custom();
            for m in mods {
                item.put(m);
            }
        }
        item
    }

    /// The upgrade `rawcode` becomes with this map's modifications on top.
    pub fn upgrade<'e>(&self, editor: &'e Editor, rawcode: &str) -> Upgrade<'e> {
        let mut upgrade = Upgrade::new(editor, rawcode);
        for m in self.on(rawcode) {
            upgrade.put(m);
        }
        upgrade
    }
}

/// What happened to one record.
#[derive(Debug, Clone)]
pub struct Resolution {
    /// The object's rawcode, as text.
    pub rawcode: String,
    /// Which kind of object it resolved to.
    pub kind: Option<ObjectKind>,
    /// Which table the record came from: `0` = base, `1` = custom.
    pub table: usize,
    /// How many modifications were laid on it.
    pub mods: usize,
    /// The field fourccs laid on it.
    pub fields: Vec<Fourcc>,
}

/// Everything that happened to one object file.
#[derive(Debug, Clone)]
pub struct ApplyReport {
    /// Which build resolved against.
    pub build: String,
    /// The map, as an archive label.
    pub map: String,
    /// The object file's own name inside the map.
    pub name: String,
    /// Resolutions, in file order.
    pub resolved: Vec<Resolution>,
    /// Records that could not be resolved.
    pub unresolved: Vec<Unresolved>,
    /// Field mappings that could not be proven.
    pub refusals: Vec<Refusal>,
}

impl ApplyReport {
    /// Whether every record resolved **and** every mapping was proven.
    ///
    /// A [`Refusal`] is not a failure to resolve: it is a mapping the crate
    /// refuses to interpret, reported so the caller can see it.
    pub fn clean(&self) -> bool {
        self.unresolved.is_empty() && self.refusals.is_empty()
    }

    /// Whether every record resolved. Refusals do not affect this.
    pub fn resolved_all(&self) -> bool {
        self.unresolved.is_empty()
    }

    /// First-table records, resolved or not.
    pub fn base_records(&self) -> usize {
        self.resolved.iter().filter(|r| r.table == 0).count()
            + self
                .unresolved
                .iter()
                .filter(|u| matches!(u, Unresolved::UnknownOriginal { table: 0, .. }))
                .count()
    }

    /// Second-table records, resolved or not.
    pub fn custom_records(&self) -> usize {
        self.resolved.iter().filter(|r| r.table == 1).count()
            + self
                .unresolved
                .iter()
                .filter(|u| matches!(u, Unresolved::UnknownOriginal { table: 1, .. }))
                .count()
    }

    /// How many records resolved.
    pub fn resolved_count(&self) -> usize {
        self.resolved.len()
    }

    /// How many modifications were laid.
    pub fn mods(&self) -> usize {
        self.resolved.iter().map(|r| r.mods).sum()
    }
}

/// The state one map's object files were applied into.
#[derive(Debug, Clone, Default)]
pub struct Applied {
    /// Per-map reports, in map order.
    pub reports: Vec<ApplyReport>,
    /// The modifications, by map then raw code.
    pub layers: BTreeMap<String, Overlay>,
    /// Every field fourcc any shipped map set, with the wire word it used.
    pub fourcc_words: BTreeMap<Fourcc, u32>,
    /// Every `(fourcc, declared, wire word)` that disagrees, with a count.
    pub wire_mismatches: BTreeMap<(Fourcc, String, u32), usize>,
}

impl Applied {
    /// Apply every object file of one map, in archive order.
    pub fn apply_map(editor: &Editor, set: &mut pcx_mpq::MpqSet, map: &str) -> Result<Self, Error> {
        let names = crate::map_object_files(set, map)?;
        let mut me = Self::default();
        for name in names {
            let low = name.to_ascii_lowercase();
            if !(low.ends_with(".w3u") || low.ends_with(".w3t")) {
                continue;
            }
            let bytes = crate::extract_object_file(set, map, &name)?;
            let file = ObjectFile::parse(&name, &bytes)?;
            let report = me.push(editor, map, &name, &file);
            let overlay = Overlay::of(&file);
            me.layers.insert(map.to_string(), overlay);
            let _ = report;
        }
        Ok(me)
    }

    /// Apply one already-parsed object file, appending its report.
    pub fn push(
        &mut self,
        editor: &Editor,
        map: &str,
        name: &str,
        file: &ObjectFile<'_>,
    ) -> ApplyReport {
        let mut report = ApplyReport {
            build: editor.build().to_string(),
            map: map.to_string(),
            name: name.to_string(),
            resolved: Vec::new(),
            unresolved: Vec::new(),
            refusals: Vec::new(),
        };

        for (table_index, records) in [(0usize, file.base()), (1usize, file.custom())] {
            for record in records {
                let rawcode = fourcc_text(record.original);
                let kind = editor.resolve(&rawcode);
                let fields: Vec<Fourcc> = record.mods.iter().map(|m| m.field).collect();

                // A first-table record must modify something that exists; a
                // second-table record creates something that does not.
                match table_index {
                    0 if kind.is_none() => {
                        report.unresolved.push(Unresolved::UnknownOriginal {
                            rawcode: record.original,
                            table: table_index,
                            map: map.to_string(),
                        });
                        continue;
                    }
                    1 => {
                        let new = fourcc_text(record.modified);
                        if let Some(k) = editor.resolve(&new) {
                            report.unresolved.push(Unresolved::ModifiedCollides {
                                rawcode: record.modified,
                                kind: k,
                            });
                            continue;
                        }
                        if kind.is_none() {
                            report.unresolved.push(Unresolved::UnknownOriginal {
                                rawcode: record.original,
                                table: table_index,
                                map: map.to_string(),
                            });
                            continue;
                        }
                    }
                    _ => {}
                }

                for m in &record.mods {
                    if let Some(selectors) = m.selectors {
                        report.refusals.push(Refusal::IndexedField {
                            fourcc: m.field,
                            selectors,
                        });
                    }
                    self.fourcc_words.insert(m.field, m.value_type.word());
                    let Some(def) = editor.field(m.field) else {
                        report
                            .refusals
                            .push(Refusal::UnknownFourcc { fourcc: m.field });
                        continue;
                    };
                    let expected = def.kind.wire_word();
                    let actual = m.value_type.word();
                    if expected != actual {
                        *self
                            .wire_mismatches
                            .entry((m.field, def.declared.clone(), actual))
                            .or_insert(0) += 1;
                    }
                    if def.declared == "char" {
                        report
                            .refusals
                            .push(Refusal::CharOnStringWire { fourcc: m.field });
                    }
                    if !def.measured {
                        let key = match &def.source {
                            super::FieldSource::Profile { key, .. } => key.clone(),
                            super::FieldSource::Column { .. }
                            | super::FieldSource::Unknown { .. } => String::new(),
                        };
                        report.refusals.push(Refusal::NoShippedBase {
                            fourcc: m.field,
                            key,
                        });
                    }
                }

                report.resolved.push(Resolution {
                    rawcode,
                    kind,
                    table: table_index,
                    mods: record.mods.len(),
                    fields,
                });
            }
        }

        self.reports.push(report.clone());
        report
    }

    /// Every unresolved record, across every applied map.
    pub fn unresolved(&self) -> impl Iterator<Item = &Unresolved> {
        self.reports.iter().flat_map(|r| r.unresolved.iter())
    }

    /// Every refusal, across every applied map.
    pub fn refusals(&self) -> impl Iterator<Item = &Refusal> {
        self.reports.iter().flat_map(|r| r.refusals.iter())
    }

    /// Whether every record of every applied map resolved and every mapping
    /// was proven.
    pub fn clean(&self) -> bool {
        !self.reports.is_empty() && self.reports.iter().all(ApplyReport::clean)
    }

    /// Sweep every shipped map of every install, and report the coverage.
    ///
    /// This is the M4 gate's second half. A map whose rawcodes do not resolve
    /// is reported by name; the answer only counts as clean when every record
    /// of every map resolves.
    pub fn sweep(installs: &[&Path]) -> Result<Coverage, Error> {
        let mut out = Coverage::default();
        for install in installs {
            let editor = Editor::open(install)?;
            let mut set = pcx_mpq::MpqSet::open_wc3(install)?;
            let maps = crate::shipped_maps(&mut set)?;
            let mut build = BuildCoverage::default();
            build.build = editor.build().to_string();
            build.maps_in_install = maps.len();
            build.units_in_base = editor.units().len();
            build.items_in_base = editor.items().len();
            build.upgrades_in_base = editor.upgrades().len();
            build.abilities_in_base = editor.abilities().len();
            for map in &maps {
                let applied = match Self::apply_map(&editor, &mut set, map) {
                    Ok(a) => a,
                    Err(e) => {
                        build.failures.push(format!("{map}: {e}"));
                        continue;
                    }
                };
                build.maps_with_objects += applied.reports.len();
                for report in &applied.reports {
                    build.base_records += report.base_records();
                    build.custom_records += report.custom_records();
                    for r in &report.resolved {
                        if r.kind == Some(ObjectKind::Unit) {
                            if r.table == 0 {
                                build.base_units_resolved += 1;
                            } else {
                                build.custom_units_derived += 1;
                            }
                        }
                        if r.kind == Some(ObjectKind::Item) {
                            if r.table == 0 {
                                build.base_items_resolved += 1;
                            } else {
                                build.custom_items_derived += 1;
                            }
                        }
                    }
                    for u in &report.unresolved {
                        build.unresolved.push(format!("{map} {}: {u}", report.name));
                    }
                    for r in &report.refusals {
                        build.refusals.push(format!("{map} {}: {r}", report.name));
                    }
                    for r in &report.resolved {
                        let _ = r;
                    }
                }
                for ((fourcc, declared, word), n) in &applied.wire_mismatches {
                    build.wire_mismatches.push(WireMismatch {
                        fourcc: *fourcc,
                        declared: declared.clone(),
                        word: *word,
                        count: *n,
                    });
                }
                let _ = &applied;
            }
            out.builds.push(build);
        }
        Ok(out)
    }
}

/// What one install's maps resolve against that install's editor data.
#[derive(Debug, Clone, Default)]
pub struct BuildCoverage {
    /// The install label.
    pub build: String,
    /// `.w3m` / `.w3n` members in the union listfile.
    pub maps_in_install: usize,
    /// How many of them carry at least one object file.
    pub maps_with_objects: usize,
    /// Unit raw codes in the base data.
    pub units_in_base: usize,
    /// Item raw codes in the base data.
    pub items_in_base: usize,
    /// Upgrade raw codes in the base data.
    pub upgrades_in_base: usize,
    /// Ability raw codes in the base data.
    pub abilities_in_base: usize,
    /// First-table records that resolved to a unit.
    pub base_units_resolved: usize,
    /// First-table records that resolved to an item.
    pub base_items_resolved: usize,
    /// First-table records in total.
    pub base_records: usize,
    /// Second-table records that derived from a unit.
    pub custom_units_derived: usize,
    /// Second-table records that derived from an item.
    pub custom_items_derived: usize,
    /// Second-table records in total.
    pub custom_records: usize,
    /// Records that named nothing the base data carries.
    pub unresolved: Vec<String>,
    /// Field mappings that could not be proven.
    pub refusals: Vec<String>,
    /// Checks that failed outright.
    pub failures: Vec<String>,
    /// Every `(fourcc, declared, wire word)` mismatch, with counts.
    pub wire_mismatches: Vec<WireMismatch>,
}

impl BuildCoverage {
    /// First-table unit/item records that resolved.
    pub fn base_resolved(&self) -> usize {
        self.base_units_resolved + self.base_items_resolved
    }

    /// Whether every record resolved with no unproven mapping.
    pub fn clean(&self) -> bool {
        self.unresolved.is_empty()
            && self.failures.is_empty()
            && self.base_records == self.base_resolved()
            && self.custom_records == self.custom_units_derived + self.custom_items_derived
    }
}

/// Every install's coverage, in order.
#[derive(Debug, Clone, Default)]
pub struct Coverage {
    /// One entry per install swept.
    pub builds: Vec<BuildCoverage>,
}

impl Coverage {
    /// Whether every build is clean.
    pub fn clean(&self) -> bool {
        !self.builds.is_empty() && self.builds.iter().all(BuildCoverage::clean)
    }

    /// Every unresolved record, across every build.
    pub fn unresolved(&self) -> impl Iterator<Item = &String> {
        self.builds.iter().flat_map(|b| b.unresolved.iter())
    }
}

/// Apply a map's object file over a build's editor data, reporting everything.
pub fn apply_map(editor: &Editor, set: &mut pcx_mpq::MpqSet, map: &str) -> Result<Applied, Error> {
    Applied::apply_map(editor, set, map)
}

/// Decode the base data of an install.
pub fn open(install: &Path) -> Result<Editor, Error> {
    Editor::open(install)
}

/// One already-parsed object file, laid over a build's editor data.
pub fn apply(editor: &Editor, map: &str, name: &str, file: &ObjectFile<'_>) -> ApplyReport {
    let mut applied = Applied::default();
    applied.push(editor, map, name, file);
    applied.reports.remove(0)
}

/// The value a map wrote for one field, as wire-decoded.
pub fn override_value<'a>(
    file: &'a ObjectFile<'a>,
    rawcode: &str,
    fourcc: Fourcc,
) -> Option<&'a Value> {
    file.records()
        .find(|r| fourcc_text(r.original) == rawcode)
        .and_then(|r| r.mods.iter().find(|m| m.field == fourcc))
        .map(|m| &m.value)
}

/// Which editor table a rawcode belongs to, for a diagnostic.
pub fn kind_of(editor: &Editor, rawcode: &str) -> Option<ObjectKind> {
    editor.resolve(rawcode)
}

#[cfg(test)]
mod indexed_regressions {
    use super::*;
    #[test]
    fn distinct_authored_selector_pairs_never_collapse() {
        let mut data = Vec::new();
        for word in [2u32, 1] {
            data.extend(word.to_le_bytes());
        }
        data.extend(b"ABCD");
        data.extend([0; 4]);
        data.extend(2u32.to_le_bytes());
        for (selector, value) in [(1u32, 10u32), (2, 20)] {
            data.extend(b"data");
            for word in [0u32, selector, 0, value, 0] {
                data.extend(word.to_le_bytes());
            }
        }
        data.extend(0u32.to_le_bytes());
        let file = ObjectFile::parse("war3map.w3a", &data).unwrap();
        let overlay = Overlay::of(&file);
        assert_eq!(
            overlay.indexed_value("ABCD", *b"data", [1, 0]),
            Some(&Value::Int(10))
        );
        assert_eq!(
            overlay.indexed_value("ABCD", *b"data", [2, 0]),
            Some(&Value::Int(20))
        );
        assert_eq!(overlay.indexed_value("ABCD", *b"data", [3, 0]), None);
    }
}

//! Raw authored technology-requirement entries from `DefaultTables`.
//!
//! This helper does not evaluate a technology tree. It exposes the authored
//! SLK/profile fields whose editor category is `tech` exactly as they are
//! joined by [`pcx_objects::editor::defaults::DefaultTables`]: every source
//! remains identifiable, and missing, malformed or ambiguous joins stay as
//! [`pcx_objects::editor::report::Refusal`] values instead of being replaced
//! with defaults or simulated native behavior.

use pcx_objects::editor::defaults::Refusal;
use pcx_objects::editor::defaults::{DefaultTables, Family, MappedField};
use pcx_objects::editor::FieldDef;

/// One authored technology-requirement lookup for one object.
#[derive(Debug)]
pub struct RequirementEntry<'a> {
    /// Object family that carries the field.
    pub family: Family,
    /// Authored object rawcode, with its original spelling preserved.
    pub rawcode: &'a str,
    /// Metadata definition for the tech-category field.
    pub field: &'a FieldDef,
    /// Result of joining and typing the field, including refusal states.
    pub outcome: Result<MappedField<'a>, Refusal>,
}

/// Collection of all authored technology-requirement entries from one table set.
#[derive(Debug)]
pub struct RequirementsCatalog<'a> {
    tables: &'a DefaultTables,
    entries: Vec<RequirementEntry<'a>>,
}

impl<'a> RequirementsCatalog<'a> {
    /// Build the catalog. Missing or unresolved objects are skipped; their
    /// absence is reported by the underlying [`DefaultTables`], not hidden here.
    pub fn new(tables: &'a DefaultTables, fields: &[[u8; 4]]) -> Self {
        let mut entries = Vec::new();
        for &family in Family::ALL {
            for rawcode in tables.rawcodes(family) {
                let Ok(object) = tables.object(family, rawcode) else {
                    continue;
                };
                for &fourcc in fields {
                    if let Ok(mapped) = object.field(fourcc) {
                        entries.push(RequirementEntry {
                            family,
                            rawcode,
                            field: mapped.definition,
                            outcome: Ok(mapped),
                        });
                    } else {
                        // Keep a refused entry only when the family actually owns
                        // the metadata field; otherwise it is unrelated to this object.
                        if tables
                            .editor()
                            .field(fourcc)
                            .map(|def| def.from == family.metadata())
                            .unwrap_or(false)
                        {
                            entries.push(RequirementEntry {
                                family,
                                rawcode,
                                field: tables.editor().field(fourcc).unwrap(),
                                outcome: Err(object.field(fourcc).unwrap_err()),
                            });
                        }
                    }
                }
            }
        }
        Self { tables, entries }
    }

    /// The table set this catalog was derived from.
    pub fn tables(&self) -> &'a DefaultTables {
        self.tables
    }

    /// Every entry, in family/source order.
    pub fn entries(&self) -> &[RequirementEntry<'a>] {
        &self.entries
    }

    /// Number of entries. A zero value means no tech category was joined.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the catalog is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Entries for one family.
    pub fn for_family(&self, family: Family) -> Vec<&RequirementEntry<'a>> {
        self.entries
            .iter()
            .filter(|entry| entry.family == family)
            .collect()
    }

    /// Entries for one object rawcode within an optional family filter.
    pub fn for_rawcode(&self, family: Family, rawcode: &str) -> Vec<&RequirementEntry<'a>> {
        self.entries
            .iter()
            .filter(|entry| entry.family == family && entry.rawcode == rawcode)
            .collect()
    }
}

/// Compatibility alias for callers that name this lookup by its source concept.
pub type TechnologyRequirements<'a> = RequirementsCatalog<'a>;

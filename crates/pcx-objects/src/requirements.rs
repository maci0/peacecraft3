//! Batched caller-selected authored fields from [`DefaultTables`].
//!
//! The caller chooses the field identifiers and their intended role. Neither
//! the identifiers nor an editor category establish native prerequisite rules.
//! Successful SLK/profile joins retain provenance; failed joins retain [`Refusal`].
//! Source-loading issues remain available through [`DefaultTables::unresolved`].

use crate::Fourcc;
use crate::editor::FieldDef;
use crate::editor::defaults::{DefaultTables, Family, MappedField, Refusal, Unresolved};

#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("no supplied metadata definition for requested field {fourcc:?}")]
    UnknownField { fourcc: Fourcc },
    #[error(transparent)]
    Object(#[from] Unresolved),
}

/// One caller-selected field lookup for one object.
#[derive(Debug)]
pub struct RequirementEntry<'a> {
    /// Object family that carries the field.
    pub family: Family,
    /// Authored object rawcode, with its original spelling preserved.
    pub rawcode: &'a str,
    /// Authored metadata definition for the selected field.
    pub field: &'a FieldDef,
    /// Result of joining and typing the field, including refusal states.
    pub outcome: Result<MappedField<'a>, Refusal>,
}

/// Selected field outcomes from one table set, without dependency evaluation.
#[derive(Debug)]
pub struct RequirementsCatalog<'a> {
    tables: &'a DefaultTables,
    entries: Vec<RequirementEntry<'a>>,
}

impl<'a> RequirementsCatalog<'a> {
    /// Validate every selected identifier before collecting outcomes. For each
    /// object, only definitions from its family's metadata are queried. Selection
    /// order and duplicates are preserved; absent source rows remain in
    /// [`DefaultTables::unresolved`], rather than receiving invented objects.
    pub fn new(tables: &'a DefaultTables, fields: &[Fourcc]) -> Result<Self, CatalogError> {
        let definitions = fields
            .iter()
            .map(|&fourcc| {
                tables
                    .editor()
                    .field(fourcc)
                    .ok_or(CatalogError::UnknownField { fourcc })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut entries = Vec::new();
        for &family in Family::ALL {
            for rawcode in tables.rawcodes(family) {
                let object = tables.object(family, rawcode)?;
                for &field in definitions
                    .iter()
                    .filter(|def| def.from == family.metadata())
                {
                    entries.push(RequirementEntry {
                        family,
                        rawcode,
                        field,
                        outcome: object.field(field.fourcc),
                    });
                }
            }
        }
        Ok(Self { tables, entries })
    }

    /// The table set this catalog was derived from.
    pub fn tables(&self) -> &'a DefaultTables {
        self.tables
    }

    /// Every entry, in family/rawcode order, then caller selection order.
    pub fn entries(&self) -> &[RequirementEntry<'a>] {
        &self.entries
    }

    /// Number of lookup outcomes, including refusals.
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

    /// Entries for one exact authored rawcode within a family.
    pub fn for_rawcode(&self, family: Family, rawcode: &str) -> Vec<&RequirementEntry<'a>> {
        self.entries
            .iter()
            .filter(|entry| entry.family == family && entry.rawcode == rawcode)
            .collect()
    }
}

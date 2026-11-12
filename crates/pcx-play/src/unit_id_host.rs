//! `UnitId(string)`: a unit-type name to its rawcode integer. Decoded from the reviewed client,
//! see `docs/re-notes/unit-id-native.md`.
//!
//! Handler `0x4af290` looks the string up in the name table of the script context
//! (`0x4770c0`, a case-insensitive ASCII compare, `_strnicmp` through `0xa77598`) and answers the
//! stored id, or zero for a null string or a name the table does not hold. The table is filled
//! once per unit definition by `0x69af80` through `0x47c5c0`: the first definition to register a
//! name owns it, a later one gets a counter-suffixed name instead.
//!
//! The registered text is the unit's `UnitUI` `name` column (`footman` for `hfoo`). That column
//! is the one the client's own `UnitId2String` shows for a stock unit; the slot the client reads
//! it from is not traced to its loader, so this stays recorded as inferred. Only the shipped
//! unit rows are enumerated: a name two rows share, which would depend on the registration
//! order, refuses.
use super::{Error, Failure, ObjectHost};
use pcx_objects::editor::SlkId;
use pcx_objects::editor::defaults::{DefaultTables, Family};
use pcx_script::Value;
use std::collections::BTreeMap;
use std::sync::Arc;

const NATIVE: &str = "UnitId";
const NAME_COLUMN: &str = "name";

/// What a lower-cased name stands for.
#[derive(Debug)]
enum Owner {
    Unit([u8; 4]),
    /// Two or more shipped rows carry the name.
    Shared,
}

#[derive(Debug)]
pub(super) struct UnitIds {
    tables: Arc<DefaultTables>,
    /// Built on the first lookup so admission does not read every unit row.
    names: Option<BTreeMap<String, Owner>>,
}

impl ObjectHost {
    /// Admit the shared raw unit-name table for declared UnitId and type enumerators.
    pub fn with_unit_id_lookup(mut self, tables: Arc<DefaultTables>) -> Result<Self, Error> {
        let mut admitted = false;
        for (name, parameters, result, constant) in [
            (NATIVE, &["string"][..], "integer", true),
            (
                "GroupEnumUnitsOfType",
                &["group", "string", "boolexpr"][..],
                "nothing",
                false,
            ),
            (
                "GroupEnumUnitsOfTypeCounted",
                &["group", "string", "boolexpr", "integer"][..],
                "nothing",
                false,
            ),
        ] {
            let natives = self.inner.program.natives();
            let Some(declaration) = natives.get(name) else {
                continue;
            };
            Self::contract(&self.inner, name, parameters, result, constant, None)?;
            if natives
                .get("CreateUnit")
                .is_some_and(|create| create.source != declaration.source)
            {
                return Err(self.inner.error(name, Failure::Source));
            }
            admitted = true;
        }
        if !admitted {
            return Ok(self);
        }
        self.unit_ids = Some(UnitIds {
            tables,
            names: None,
        });
        Ok(self)
    }

    fn unit_id_error(&self, why: impl Into<String>) -> Error {
        self.inner.error(NATIVE, Failure::UnitActivity(why.into()))
    }

    fn unit_name_table(&self) -> Result<BTreeMap<String, Owner>, Error> {
        let state = self
            .unit_ids
            .as_ref()
            .ok_or_else(|| self.unit_id_error("not admitted"))?;
        let mut names: BTreeMap<String, Owner> = BTreeMap::new();
        let mut named = false;
        for spelling in state.tables.rawcodes(Family::Unit) {
            let rawcode = <[u8; 4]>::try_from(spelling.as_bytes()).map_err(|_| {
                self.unit_id_error(format!("unit row {spelling:?} is not a rawcode"))
            })?;
            let object = state
                .tables
                .object(Family::Unit, spelling)
                .map_err(|why| self.unit_id_error(format!("unit row {spelling:?}: {why}")))?;
            // A unit with no `UnitUI` cell has no name to register (`0x47c5c0` ignores an empty
            // one); several shipped rows have none.
            let Ok(cell) = object.cell(SlkId::UnitUi, NAME_COLUMN) else {
                continue;
            };
            let name = cell.text.trim().to_ascii_lowercase();
            if name.is_empty() {
                continue;
            }
            named = true;
            names
                .entry(name)
                .and_modify(|owner| *owner = Owner::Shared)
                .or_insert(Owner::Unit(rawcode));
        }
        if !named {
            return Err(self.unit_id_error(
                "no unit row carries a UnitUI name; the selected tables lack that column",
            ));
        }
        Ok(names)
    }

    pub(super) fn unit_name_id(&mut self, text: &str) -> Result<Option<[u8; 4]>, Error> {
        if self.unit_ids.as_ref().is_some_and(|s| s.names.is_none()) {
            let table = self.unit_name_table()?;
            if let Some(state) = self.unit_ids.as_mut() {
                state.names = Some(table);
            }
        }
        let names = self
            .unit_ids
            .as_ref()
            .and_then(|state| state.names.as_ref())
            .ok_or_else(|| self.unit_id_error("name table is not built"))?;
        match names.get(&text.to_ascii_lowercase()) {
                None => Ok(None),
                Some(Owner::Unit(rawcode)) => Ok(Some(*rawcode)),
                Some(Owner::Shared) => Err(self.unit_id_error(format!(
                    "unit name {text:?} is shared by several unit rows; the registration order that picks the owner is not decoded"
                ))),
            }
    }

    pub(super) fn unit_id_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if name != NATIVE
            || self.unit_ids.is_none()
            || self.inner.program.natives().get(NATIVE).is_none()
        {
            return None;
        }
        Some((|| {
            let text = match args {
                [Value::Str(text)] => text,
                [Value::Null] => return Ok(Value::Int(0)),
                _ => return Err(self.inner.error(NATIVE, Failure::Signature)),
            };
            self.unit_name_id(text)
                .map(|id| Value::Int(id.map(i32::from_be_bytes).unwrap_or(0)))
        })())
    }
}

#[cfg(test)]
#[path = "unit_id_tests.rs"]
mod tests;

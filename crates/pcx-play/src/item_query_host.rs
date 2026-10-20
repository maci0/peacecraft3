//! `GetItemLevel` and `GetItemType` for executed items.
//!
//! The level is the item definition's `ilev` field, an authored cell or a typed override.
//! The type is the ordinal that the selected executable's class compare chain assigns to
//! the definition's `icla` text, returned as the program's `ConvertItemType` identity.
//! Class text that matches no name, and a missing class, take the chain's default, which
//! the engine also returns for an invalid item handle. A missing or consumed item answers
//! level 0, the zero value `GetItemTypeId` uses.

use super::ObjectHost;
use crate::item_class_table::ItemClassTable;
use super::super::{Error, Failure};
use pcx_objects::Value as FieldOverride;
use pcx_script::Value;
use pcx_sim::world::{FieldProblem, FieldValue};

const LEVEL: &str = "GetItemLevel";
const TYPE: &str = "GetItemType";
const CONVERT: &str = "ConvertItemType";
const LEVEL_FOURCC: [u8; 4] = *b"ilev";
const CLASS_FOURCC: [u8; 4] = *b"icla";

impl ObjectHost {
    /// Validates both declarations and that every class ordinal, plus the default, is an
    /// enumerand of the program's `ConvertItemType` conversion.
    pub fn with_item_queries(mut self, classes: ItemClassTable) -> Result<Self, Error> {
        if self.item_queries.is_some() {
            return Err(self
                .inner
                .error(LEVEL, Failure::ItemState("item queries already admitted")));
        }
        Self::contract(&self.inner, LEVEL, &["item"], "integer", false, None)?;
        Self::contract(&self.inner, TYPE, &["item"], "itemtype", false, None)?;
        let domain = self
            .inner
            .conversions()
            .find(|c| c.name == CONVERT && c.kind == "itemtype")
            .map(|c| c.domain.clone())
            .ok_or_else(|| self.inner.error(CONVERT, Failure::Signature))?;
        let ordinals = classes.ordinals.values().copied().chain([classes.default]);
        if ordinals.clone().any(|n| !domain.contains(&n)) {
            return Err(self
                .inner
                .error(TYPE, Failure::ItemState("class ordinal is not a program item type")));
        }
        self.item_queries = Some(classes);
        Ok(self)
    }

    pub(super) fn item_query_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if self.item_queries.is_none() || !matches!(name, LEVEL | TYPE) {
            return None;
        }
        Some(self.query_item(name, args))
    }

    fn query_item(&mut self, name: &str, args: &[Value]) -> Result<Value, Error> {
        let [Value::Handle(item)] = args else {
            return Err(self.inner.error(name, Failure::ItemState("expected one item handle")));
        };
        let live = self.live_item(*item)?;
        let classes = self.item_queries.as_ref().expect("dispatch requires admission");
        let default = classes.default;
        if name == LEVEL {
            let Some(record) = live else {
                return Ok(Value::Int(0));
            };
            return authored_level(record.definition.fields.get(&LEVEL_FOURCC).map(|f| &f.value))
                .map(Value::Int)
                .ok_or_else(|| self.inner.error(name, Failure::ItemState("authored item level is not derived")));
        }
        let ordinal = match live {
            None => default,
            Some(record) => {
                let text = class_text(record.definition.fields.get(&CLASS_FOURCC).map(|f| &f.value));
                match text {
                    Err(()) => {
                        return Err(self
                            .inner
                            .error(name, Failure::ItemState("authored item class is not derived")));
                    }
                    Ok(None) => default,
                    Ok(Some(text)) => classes
                        .ordinals
                        .iter()
                        .find(|(class, _)| class.eq_ignore_ascii_case(&text))
                        .map_or(default, |(_, n)| *n),
                }
            }
        };
        self.inner.convert_for(CONVERT, &[Value::Int(ordinal)])
    }
}

/// `ilev` as a level: a cell that parses as an integer, or a typed integer override.
fn authored_level(value: Option<&Result<FieldValue, FieldProblem>>) -> Option<i32> {
    match value? {
        Ok(FieldValue::Cell(text)) => text.trim().parse().ok(),
        Ok(FieldValue::Override(FieldOverride::Int(raw))) => i32::try_from(*raw).ok(),
        _ => None,
    }
}

/// `icla` text: `Ok(None)` when the class is absent, `Err` when it is present but not text.
fn class_text(value: Option<&Result<FieldValue, FieldProblem>>) -> Result<Option<String>, ()> {
    match value {
        None | Some(Err(FieldProblem::Missing)) => Ok(None),
        Some(Ok(FieldValue::Cell(text))) => Ok(Some(text.clone())),
        Some(Ok(FieldValue::Override(FieldOverride::Str(bytes)))) => {
            String::from_utf8(bytes.clone()).map(Some).map_err(|_| ())
        }
        Some(_) => Err(()),
    }
}

#[cfg(test)]
#[path = "item_query_tests.rs"]
mod tests;

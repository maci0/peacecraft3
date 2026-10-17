//! Source-catalog item definitions and executed creation arguments, not world state.
use super::super::{Entry, Error, Failure, below};
use super::ObjectHost;
use pcx_objects::editor::ObjectKind;
use pcx_script::{HandleId, Value};
use pcx_sim::world::{Definition, ObjectCatalog, ResolutionProblem, Source};
use std::sync::Arc;

const CREATE: &str = "CreateItem";

/// Caller retention policy, not original item limits or stats.
#[derive(Clone, Copy, Debug)]
pub struct ItemBudget {
    pub max_records: usize,
    pub max_retained_fields: usize,
    pub max_modifications: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExecutedItem {
    pub definition: Definition,
    pub source: Source,
    pub x: f32,
    pub y: f32,
}

pub(super) struct Admission {
    catalog: Arc<dyn ObjectCatalog + Send + Sync>,
    source: Source,
    budget: ItemBudget,
    records: usize,
}

impl ObjectHost {
    /// Opt into creation using the caller's selected catalog and map identity.
    /// Non-finite coordinates refuse as recreation policy, not native evidence.
    pub fn with_item_catalog(
        mut self,
        catalog: Arc<dyn ObjectCatalog + Send + Sync>,
        source: Source,
        budget: ItemBudget,
    ) -> Result<Self, Error> {
        let fail = |reason| self.inner.error(CREATE, reason);
        if self.items.is_some() {
            return Err(fail(Failure::ItemState("already admitted")));
        }
        if source.install.is_empty() || source.edition.is_empty() || source.map.is_empty() {
            return Err(fail(Failure::ItemState("source identity is incomplete")));
        }
        std::alloc::Layout::array::<ExecutedItem>(budget.max_records)
            .map_err(|_| fail(Failure::Capacity))?;
        if !below(&self.inner.program, "item", "widget") {
            return Err(fail(Failure::Signature));
        }
        Self::contract(
            &self.inner,
            CREATE,
            &["integer", "real", "real"],
            "item",
            false,
            Some("item"),
        )?;
        self.items = Some(Admission {
            catalog,
            source,
            budget,
            records: 0,
        });
        Ok(self)
    }

    pub fn executed_item(&self, id: HandleId) -> Result<&ExecutedItem, Error> {
        match self
            .inner
            .entry(id)
            .map_err(|e| self.inner.error(CREATE, e.failure))?
        {
            Entry::Item { record } => Ok(record),
            _ => Err(self.inner.error(CREATE, Failure::WrongFamily)),
        }
    }

    pub(super) fn item_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if name != CREATE || self.items.is_none() {
            return None;
        }
        Some(self.create_item(args))
    }

    fn create_item(&mut self, args: &[Value]) -> Result<Value, Error> {
        let fail = |reason| self.inner.error(CREATE, reason);
        let [Value::Int(rawcode), x, y] = args else {
            return Err(fail(Failure::ItemState("expected integer, real, real")));
        };
        let x = x
            .as_real()
            .filter(|v| v.is_finite())
            .ok_or_else(|| fail(Failure::ItemState("finite real coordinate required")))?;
        let y = y
            .as_real()
            .filter(|v| v.is_finite())
            .ok_or_else(|| fail(Failure::ItemState("finite real coordinate required")))?;
        let admitted = self.items.as_ref().expect("dispatch requires admission");
        if admitted.records >= admitted.budget.max_records {
            return Err(fail(Failure::Budget("item creation")));
        }
        if self.inner.handles() >= self.inner.max_handles {
            return Err(fail(Failure::Capacity));
        }
        let requested = rawcode.to_be_bytes();
        let catalog_error = |problem| {
            fail(Failure::ItemCatalog {
                requested,
                problem: Box::new(problem),
            })
        };
        let definition = admitted.catalog.resolve(requested).map_err(catalog_error)?;
        if definition.rawcode != requested {
            return Err(catalog_error(ResolutionProblem::RawcodeMismatch {
                requested,
                resolved: definition.rawcode,
            }));
        }
        if definition.kind != ObjectKind::Item {
            return Err(catalog_error(ResolutionProblem::WrongFamily {
                expected: ObjectKind::Item,
                actual: definition.kind,
            }));
        }
        if definition.fields.len() > admitted.budget.max_retained_fields {
            return Err(fail(Failure::Budget("item retained fields")));
        }
        if definition.modifications.len() > admitted.budget.max_modifications {
            return Err(fail(Failure::Budget("item modifications")));
        }
        let record = ExecutedItem {
            definition,
            source: admitted.source.clone(),
            x,
            y,
        };
        let id = self.inner.allocate(Entry::Item { record }, CREATE)?;
        self.items.as_mut().expect("admission retained").records += 1;
        Ok(Value::Handle(id))
    }
}

#[cfg(test)]
#[path = "item_host_tests.rs"]
mod tests;

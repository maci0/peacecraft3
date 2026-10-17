//! Executed `CreateItem` joined to the caller's already selected catalog.
//!
//! The catalog value is the same `Arc` the caller holds. This module does not
//! load SLK, overlays, or map files, and it does not place, own, stock, charge,
//! animate, pick up, or remove the item. Record budget and arena capacity are
//! checked before `resolve` and before the per-item `Source` clone. The integer
//! rawcode is `i32::to_be_bytes`. `base_rawcode` is stored as the catalog
//! returned it. A reserved identity of kind `item` is not a created item.
use super::super::{Entry, Error, Failure, below};
use super::ObjectHost;
use pcx_objects::editor::ObjectKind;
use pcx_script::span::Span;
use pcx_script::{HandleId, Value};
use pcx_sim::world::{Definition, ObjectCatalog, ResolutionProblem, Source};
use std::sync::Arc;

const NATIVE: &str = "CreateItem";
const KIND: &str = "item";

/// Caller bound on creation records. Not a world, stock, or rendering limit.
#[derive(Clone, Copy, Debug)]
pub struct ItemBudget {
    pub max_records: usize,
    /// Maximum `Definition.fields` retained from one successful resolve.
    pub max_retained_fields: usize,
    /// Maximum authored modifications retained from one successful resolve.
    pub max_modifications: usize,
}

/// Checked catalog result plus the executed coordinates.
///
/// `source` is the identity the caller already selected. Coordinates are the
/// call's finite reals. This is not ownership, life, charges, or a placed
/// world object.
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
    declaration_source: String,
    span: Span,
    max_records: usize,
    max_retained_fields: usize,
    max_modifications: usize,
    records: usize,
}

impl ObjectHost {
    /// Admit `CreateItem` against an already selected catalog and source.
    ///
    /// `catalog` must be the caller's existing arc. `source` must be an
    /// identity the caller already selected. Empty install, edition, or map
    /// is refused. `item` must extend `widget`. The native is non-constant
    /// `integer, real, real -> item` and is not inferred from another family.
    pub fn with_item_catalog(
        mut self,
        catalog: Arc<dyn ObjectCatalog + Send + Sync>,
        source: Source,
        budget: ItemBudget,
    ) -> Result<Self, Error> {
        let fail = |reason| self.inner.error(NATIVE, reason);
        if self.items.is_some() {
            return Err(fail(Failure::ItemState("already admitted")));
        }
        if source.install.is_empty() || source.edition.is_empty() || source.map.is_empty() {
            return Err(fail(Failure::ItemState("source identity is incomplete")));
        }
        std::alloc::Layout::array::<ExecutedItem>(budget.max_records)
            .map_err(|_| fail(Failure::Capacity))?;
        if !below(&self.inner.program, KIND, "widget") {
            return Err(fail(Failure::Signature));
        }
        Self::contract(
            &self.inner,
            NATIVE,
            &["integer", "real", "real"],
            KIND,
            false,
            Some(KIND),
        )?;
        let native = self
            .inner
            .program
            .natives()
            .get(NATIVE)
            .ok_or_else(|| fail(Failure::Signature))?;
        let mut declaration_source = String::new();
        declaration_source
            .try_reserve_exact(native.source.len())
            .map_err(|_| fail(Failure::Capacity))?;
        declaration_source.push_str(&native.source);
        self.items = Some(Admission {
            catalog,
            source,
            declaration_source,
            span: native.span,
            max_records: budget.max_records,
            max_retained_fields: budget.max_retained_fields,
            max_modifications: budget.max_modifications,
            records: 0,
        });
        Ok(self)
    }

    /// Same catalog allocation passed to [`Self::with_item_catalog`].
    pub fn item_catalog(&self) -> Result<Arc<dyn ObjectCatalog + Send + Sync>, Error> {
        Ok(Arc::clone(&self.admitted_items()?.catalog))
    }

    /// Declaration retained at admission. The span is the native's own span.
    pub fn item_declaration(&self) -> Result<(&str, Span), Error> {
        let admitted = self.admitted_items()?;
        Ok((&admitted.declaration_source, admitted.span))
    }

    /// Checked result for this item handle. Other families and reserved identities are refused.
    pub fn executed_item(&self, id: HandleId) -> Result<&ExecutedItem, Error> {
        match self.inner.entry(id)? {
            Entry::Item { record } => Ok(record),
            _ => Err(self.inner.error(NATIVE, Failure::WrongFamily)),
        }
    }

    pub(super) fn item_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        self.items.as_ref()?;
        (name == NATIVE).then(|| self.create_item(args))
    }

    fn admitted_items(&self) -> Result<&Admission, Error> {
        self.items
            .as_ref()
            .ok_or_else(|| self.inner.error(NATIVE, Failure::ItemState("not admitted")))
    }

    fn create_item(&mut self, args: &[Value]) -> Result<Value, Error> {
        let record = {
            let fail = |reason| self.inner.error(NATIVE, reason);
            let [Value::Int(itemid), x, y] = args else {
                return Err(fail(Failure::ItemState(
                    "arguments do not match the item native declaration",
                )));
            };
            let x = finite_real(x).map_err(fail)?;
            let y = finite_real(y).map_err(fail)?;
            let rawcode = itemid.to_be_bytes();
            let (records, max_records) = {
                let admitted = self.admitted_items()?;
                (admitted.records, admitted.max_records)
            };
            if records >= max_records {
                return Err(fail(Failure::Budget("item creation")));
            }
            if self.inner.handles() >= self.inner.max_handles {
                return Err(fail(Failure::Capacity));
            }
            let catalog = Arc::clone(&self.admitted_items()?.catalog);
            let (max_fields, max_modifications) = {
                let admitted = self.admitted_items()?;
                (admitted.max_retained_fields, admitted.max_modifications)
            };
            let definition = match catalog.resolve(rawcode) {
                Ok(definition) => definition,
                Err(problem) => {
                    return Err(fail(Failure::ItemCatalog {
                        requested: rawcode,
                        problem: Box::new(problem),
                    }));
                }
            };
            if definition.rawcode != rawcode {
                return Err(fail(Failure::ItemCatalog {
                    requested: rawcode,
                    problem: Box::new(ResolutionProblem::RawcodeMismatch {
                        requested: rawcode,
                        resolved: definition.rawcode,
                    }),
                }));
            }
            if definition.kind != ObjectKind::Item {
                return Err(fail(Failure::ItemCatalog {
                    requested: rawcode,
                    problem: Box::new(ResolutionProblem::WrongFamily {
                        expected: ObjectKind::Item,
                        actual: definition.kind,
                    }),
                }));
            }
            if definition.fields.len() > max_fields {
                return Err(fail(Failure::ItemState(
                    "resolved definition exceeds the retained-field budget",
                )));
            }
            if definition.modifications.len() > max_modifications {
                return Err(fail(Failure::ItemState(
                    "resolved definition exceeds the modification budget",
                )));
            }
            let source = self.admitted_items()?.source.clone();
            ExecutedItem {
                definition,
                source,
                x,
                y,
            }
        };
        let id = self.inner.allocate(Entry::Item { record }, NATIVE)?;
        let admitted = self
            .items
            .as_mut()
            .ok_or_else(|| self.inner.error(NATIVE, Failure::ItemState("not admitted")))?;
        admitted.records += 1;
        Ok(Value::Handle(id))
    }
}

fn finite_real(value: &Value) -> Result<f32, Failure> {
    match value {
        Value::Real(real) if real.is_finite() => Ok(*real),
        Value::Real(_) => Err(Failure::ItemState(
            "recreation policy: finite real required",
        )),
        _ => Err(Failure::ItemState(
            "arguments do not match the item native declaration",
        )),
    }
}

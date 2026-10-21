//! Canonical source-catalog item identities, ground positions and inventory ownership.
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
    pub held_by: Option<(HandleId, usize)>,
    pub invulnerable: Option<bool>,
    pub droppable: Option<bool>,
    pub drop_on_death: Option<bool>,
    /// Automatic powerup whose grant was applied and whose pickup delivery is pending.
    /// Queries still see the live item; pickup and held-slot paths refuse it.
    pub claimed: bool,
    /// Set once a claimed powerup has no queued pickup left. Removed from listings and queries.
    pub consumed: bool,
}

pub(super) struct Admission {
    catalog: Arc<dyn ObjectCatalog + Send + Sync>,
    source: Source,
    budget: ItemBudget,
    records: usize,
    inventory: Option<inventory::State>,
}

pub(super) struct CachedInventory(Vec<(usize, ExecutedItem, i32)>);

impl ObjectHost {
    pub(super) fn prepare_cached_inventory(
        &mut self,
        unit: &super::ExecutedUnit,
        saved: &[crate::game_cache::CachedItem],
    ) -> Result<CachedInventory, Error> {
        let fail = |detail: &str| {
            self.inner
                .error("RestoreUnit", Failure::GameCache(detail.into()))
        };
        let policy = self.inventory_policy_for(unit)?;
        if saved.len() > policy.slots || saved.iter().any(|item| item.slot >= policy.slots) {
            return Err(fail("cached inventory exceeds the current unit's slots"));
        }
        let mut records = Vec::new();
        records
            .try_reserve_exact(saved.len())
            .map_err(|_| fail("cached item allocation failed"))?;
        if !saved.is_empty() {
            let admitted = self
                .items
                .as_ref()
                .ok_or_else(|| fail("cached item catalog missing"))?;
            if admitted.source.install != unit.source.install
                || admitted.source.edition != unit.source.edition
            {
                return Err(fail(
                    "cached items require the unit's selected install/edition",
                ));
            }
            if admitted
                .records
                .checked_add(saved.len())
                .is_none_or(|n| n > admitted.budget.max_records)
            {
                return Err(fail("cached item record policy exceeded"));
            }
            for item in saved {
                let definition = self.item_definition(i32::from_be_bytes(item.rawcode))?;
                if self.definition_flag(&definition, "powerup")? {
                    return Err(fail("cached powerup cannot be restored as a held item"));
                }
                if definition.fields.len() > admitted.budget.max_retained_fields
                    || definition.modifications.len() > admitted.budget.max_modifications
                {
                    return Err(fail("cached item definition policy exceeded"));
                }
                records.push((
                    item.slot,
                    ExecutedItem {
                        definition,
                        source: admitted.source.clone(),
                        x: unit.x,
                        y: unit.y,
                        held_by: None,
                        invulnerable: item.flags[0],
                        droppable: item.flags[1],
                        drop_on_death: item.flags[2],
                        claimed: false,
                        consumed: false,
                    },
                    item.charges,
                ));
            }
        }
        self.preflight_cached_charges(saved.len())?;
        let handles = saved
            .len()
            .checked_add(1)
            .ok_or_else(|| fail("cached handle count overflow"))?;
        if self
            .inner
            .handles()
            .checked_add(handles)
            .is_none_or(|n| n > self.inner.max_handles)
        {
            return Err(fail("cached unit/item arena capacity exceeded"));
        }
        self.inner
            .arena
            .try_reserve(handles)
            .map_err(|_| self.inner.error("RestoreUnit", Failure::Capacity))?;
        Ok(CachedInventory(records))
    }

    pub(super) fn commit_cached_inventory(&mut self, unit: HandleId, prepared: CachedInventory) {
        let count = prepared.0.len();
        for (slot, mut record, charges) in prepared.0 {
            record.held_by = Some((unit, slot));
            let item = self
                .inner
                .allocate(Entry::Item { record }, "RestoreUnit")
                .expect("preflighted cached item identities");
            self.commit_cached_charges(item, charges);
        }
        if count != 0 {
            self.items
                .as_mut()
                .expect("preflighted item catalog")
                .records += count;
        }
    }
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
            inventory: None,
        });
        Ok(self)
    }

    pub fn executed_items(&self) -> impl Iterator<Item = (HandleId, &ExecutedItem)> {
        self.inner
            .arena
            .iter()
            .enumerate()
            .filter_map(|(at, entry)| match entry {
                Entry::Item { record } if !record.consumed => {
                    Some((self.inner.id(at, CREATE).ok()?, record))
                }
                _ => None,
            })
    }

    /// Canonical item lookup for every inventory and query path: a consumed powerup and a
    /// removed handle both read as absent, a non-item handle is a family error.
    pub(super) fn live_item(&self, item: HandleId) -> Result<Option<&ExecutedItem>, Error> {
        match self.inner.entry(item)? {
            Entry::Item { record } => Ok((!record.consumed).then_some(record)),
            Entry::RemovedItem => Ok(None),
            _ => Err(self.inner.error("item", Failure::WrongFamily)),
        }
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

    pub(crate) fn create_item(&mut self, args: &[Value]) -> Result<Value, Error> {
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
        Ok(Value::Handle(
            self.create_item_record(*rawcode, x, y, None)?,
        ))
    }

    /// The source definition of an item rawcode, checked against the admitted catalog. This
    /// reads only, so a caller can classify an item before any budget or handle is spent.
    pub(crate) fn item_definition(&self, rawcode: i32) -> Result<Definition, Error> {
        let fail = |reason| self.inner.error(CREATE, reason);
        let admitted = self
            .items
            .as_ref()
            .ok_or_else(|| fail(Failure::ItemState("item catalog not admitted")))?;
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
        Ok(definition)
    }

    /// Creates one item record, optionally already held by `held_by`. Every refusal (item budget,
    /// handle capacity, catalog, retained-field budget) is checked before the handle is allocated,
    /// so a refused creation leaves the arena, handle counter and record count unchanged.
    pub(crate) fn create_item_record(
        &mut self,
        rawcode: i32,
        x: f32,
        y: f32,
        held_by: Option<(HandleId, usize)>,
    ) -> Result<HandleId, Error> {
        let fail = |reason| self.inner.error(CREATE, reason);
        let admitted = self.items.as_ref().expect("dispatch requires admission");
        if admitted.records >= admitted.budget.max_records {
            return Err(fail(Failure::Budget("item creation")));
        }
        if self.inner.handles() >= self.inner.max_handles {
            return Err(fail(Failure::Capacity));
        }
        let definition = self.item_definition(rawcode)?;
        let admitted = self.items.as_ref().expect("dispatch requires admission");
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
            held_by,
            invulnerable: None,
            droppable: None,
            drop_on_death: None,
            claimed: false,
            consumed: false,
        };
        let id = self.inner.allocate(Entry::Item { record }, CREATE)?;
        self.items.as_mut().expect("admission retained").records += 1;
        Ok(id)
    }
}

#[cfg(test)]
#[path = "item_host_tests.rs"]
mod tests;

#[path = "inventory_host.rs"]
mod inventory;

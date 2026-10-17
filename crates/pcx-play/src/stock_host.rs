//! Executed shop slot configuration on the existing Host.
//!
//! `InitBlizzard` calls `InitNeutralBuildings`, which passes the script's
//! item and unit slot integers to `SetAllItemTypeSlots` and
//! `SetAllUnitTypeSlots`. Those integers are stored as written. Per-unit
//! `SetItemTypeSlots` and `SetUnitTypeSlots` records stay separate: a global
//! value does not resolve a unit record in this assignment-only profile.
//! Native default and ceiling behavior remains unmeasured.
//! Add/remove stock, restock timing, item categories, and marketplace
//! selection are not admitted.
use super::super::{Error, Failure, below};
use super::ObjectHost;
use pcx_script::{HandleId, Value};

const SET_ALL_ITEM: &str = "SetAllItemTypeSlots";
const SET_ALL_UNIT: &str = "SetAllUnitTypeSlots";
const SET_ITEM: &str = "SetItemTypeSlots";
const SET_UNIT: &str = "SetUnitTypeSlots";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SlotKind {
    Item,
    Unit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct UnitSlots {
    unit: HandleId,
    kind: SlotKind,
    slots: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct State {
    item_slots: Option<i32>,
    unit_slots: Option<i32>,
    units: Vec<UnitSlots>,
    max_unit_records: usize,
}

/// Bound on distinct per-unit slot records. Global assignments are separate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StockBudget {
    pub max_unit_records: usize,
}

fn slot_native(name: &str) -> bool {
    matches!(name, SET_ALL_ITEM | SET_ALL_UNIT | SET_ITEM | SET_UNIT)
}

impl ObjectHost {
    /// Admit the four common slot natives on this Host.
    ///
    /// Each declaration is non-constant, returns nothing, and shares
    /// `CreateTimer`'s source. A function of the same name, a second source,
    /// or a different parameter list refuses admission. `unit` must extend
    /// `handle`. The record bound is checked before any slot state is stored.
    /// A second call returns `StockState("already admitted")` and consumes
    /// this host, as the other `with_*` admissions do.
    pub fn with_stock(mut self, budget: StockBudget) -> Result<Self, Error> {
        let fail = |reason| self.inner.error(SET_ALL_ITEM, reason);
        if self.stock.is_some() {
            return Err(fail(Failure::StockState("already admitted")));
        }
        std::alloc::Layout::array::<UnitSlots>(budget.max_unit_records)
            .map_err(|_| fail(Failure::Capacity))?;
        if !below(&self.inner.program, "unit", "handle") {
            return Err(fail(Failure::Signature));
        }
        let anchor = self
            .inner
            .program
            .natives()
            .get("CreateTimer")
            .ok_or_else(|| fail(Failure::Signature))?
            .source
            .clone();
        for (name, parameters) in [
            (SET_ALL_ITEM, &["integer"][..]),
            (SET_ALL_UNIT, &["integer"][..]),
            (SET_ITEM, &["unit", "integer"][..]),
            (SET_UNIT, &["unit", "integer"][..]),
        ] {
            Self::contract(&self.inner, name, parameters, "nothing", false, None)?;
            if self
                .inner
                .program
                .natives()
                .get(name)
                .ok_or_else(|| fail(Failure::Signature))?
                .source
                != anchor
            {
                return Err(self.inner.error(name, Failure::Source));
            }
        }
        self.stock = Some(State {
            item_slots: None,
            unit_slots: None,
            units: Vec::new(),
            max_unit_records: budget.max_unit_records,
        });
        Ok(self)
    }

    /// Executed `SetAllItemTypeSlots` integer. `Ok(None)` is unset, not zero.
    pub fn all_item_type_slots(&self) -> Result<Option<i32>, Error> {
        Ok(self.admitted_stock(SET_ALL_ITEM)?.item_slots)
    }

    /// Executed `SetAllUnitTypeSlots` integer. `Ok(None)` is unset, not zero.
    pub fn all_unit_type_slots(&self) -> Result<Option<i32>, Error> {
        Ok(self.admitted_stock(SET_ALL_UNIT)?.unit_slots)
    }

    /// Explicit `SetItemTypeSlots` record for one live unit.
    ///
    /// A missing record is `Ok(None)`. The global item slot integer is not
    /// consulted. An id that is not a live unit is an error.
    pub fn unit_item_type_slots(&self, unit: HandleId) -> Result<Option<i32>, Error> {
        self.unit_slots(SET_ITEM, unit, SlotKind::Item)
    }

    /// Explicit `SetUnitTypeSlots` record for one live unit.
    ///
    /// A missing record is `Ok(None)`. The global unit slot integer is not
    /// consulted. An id that is not a live unit is an error.
    pub fn unit_type_slots(&self, unit: HandleId) -> Result<Option<i32>, Error> {
        self.unit_slots(SET_UNIT, unit, SlotKind::Unit)
    }

    /// Count of retained per-unit records. Global assignments are not included.
    pub fn stock_unit_records(&self) -> Result<usize, Error> {
        Ok(self.admitted_stock(SET_ITEM)?.units.len())
    }

    fn admitted_stock(&self, name: &str) -> Result<&State, Error> {
        self.stock
            .as_ref()
            .ok_or_else(|| self.inner.error(name, Failure::StockState("not admitted")))
    }

    fn unit_slots(&self, name: &str, unit: HandleId, kind: SlotKind) -> Result<Option<i32>, Error> {
        self.admitted_stock(name)?;
        self.stock_unit(&Value::Handle(unit), name)?;
        Ok(self
            .admitted_stock(name)?
            .units
            .iter()
            .find(|row| row.unit == unit && row.kind == kind)
            .map(|row| row.slots))
    }

    fn stock_unit(&self, value: &Value, name: &str) -> Result<HandleId, Error> {
        let Value::Handle(id) = value else {
            return Err(self.inner.error(name, Failure::StockState("arguments")));
        };
        let kind = self
            .inner
            .kind(*id)
            .map_err(|error| self.inner.error(name, error.failure))?;
        if !below(&self.inner.program, kind, "unit") {
            return Err(self.inner.error(name, Failure::WrongFamily));
        }
        Ok(*id)
    }

    pub(super) fn stock_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if self.stock.is_none() || !slot_native(name) {
            return None;
        }
        Some(match name {
            SET_ALL_ITEM => self.set_all_slots(name, args, true),
            SET_ALL_UNIT => self.set_all_slots(name, args, false),
            SET_ITEM => self.set_unit_slots(name, args, SlotKind::Item),
            SET_UNIT => self.set_unit_slots(name, args, SlotKind::Unit),
            _ => Err(self.inner.error(name, Failure::Signature)),
        })
    }

    fn set_all_slots(&mut self, name: &str, args: &[Value], item: bool) -> Result<Value, Error> {
        let [Value::Int(slots)] = args else {
            return Err(self.inner.error(name, Failure::StockState("arguments")));
        };
        let slots = *slots;
        if self.stock.is_none() {
            return Err(self.inner.error(name, Failure::StockState("not admitted")));
        }
        let state = self.stock.as_mut().expect("stock admitted");
        if item {
            state.item_slots = Some(slots);
        } else {
            state.unit_slots = Some(slots);
        }
        Ok(Value::Null)
    }

    fn set_unit_slots(
        &mut self,
        name: &str,
        args: &[Value],
        kind: SlotKind,
    ) -> Result<Value, Error> {
        let [unit, Value::Int(slots)] = args else {
            return Err(self.inner.error(name, Failure::StockState("arguments")));
        };
        let id = self.stock_unit(unit, name)?;
        let slots = *slots;
        if self.stock.is_none() {
            return Err(self.inner.error(name, Failure::StockState("not admitted")));
        }
        let failure = {
            let state = self.stock.as_mut().expect("stock admitted");
            if let Some(row) = state
                .units
                .iter_mut()
                .find(|row| row.unit == id && row.kind == kind)
            {
                row.slots = slots;
                None
            } else if state.units.len() >= state.max_unit_records {
                Some(Failure::Budget("stock unit slots"))
            } else if state.units.try_reserve(1).is_err() {
                Some(Failure::Capacity)
            } else {
                state.units.push(UnitSlots {
                    unit: id,
                    kind,
                    slots,
                });
                None
            }
        };
        match failure {
            None => Ok(Value::Null),
            Some(failure) => Err(self.inner.error(name, failure)),
        }
    }
}

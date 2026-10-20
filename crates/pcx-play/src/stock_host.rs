//! Executed shop stock on the existing Host.
//!
//! `InitBlizzard` calls `InitNeutralBuildings`, which passes the script's
//! item and unit slot integers to `SetAllItemTypeSlots` and
//! `SetAllUnitTypeSlots`. Those integers are stored as written. Per-unit
//! `SetItemTypeSlots` and `SetUnitTypeSlots` records stay separate: a global
//! value does not resolve a unit record in this assignment-only profile.
//!
//! A shop's item table has `ShopCapacity::cells` cells, read from the selected
//! executable (see `shop_table`). `SetItemTypeSlots` stores the count clamped to
//! `slot_ceiling` as unsigned, so a negative count stores the ceiling. Only the first
//! `count` cells take part in adds. Setting the count clears the stock of every cell at
//! or beyond it and keeps their item ids.
//!
//! `AddItemToStock` finds the item among the first `count` cells. If it is not there,
//! the victim is the cell with the greatest age, lowest index on a tie. A full table
//! evicts that cell. With count 0 the victim is cell 0. Every add ages all cells by one,
//! and the touched cell's age becomes 0. Eviction is not a refusal. A new stock record
//! past `StockBudget::max_records` refuses before any cell changes.
//!
//! `RemoveItemFromStock` clears the first cell, over all cells, whose item id matches.
//! With no match it does nothing, as the native remove does.
//!
//! Evidence is in `docs/re-notes/script-stock-slots.md`. The repeated-add stock write
//! and the `currentStock` flag are not fully read, so the stored integers are not a
//! measured stock state.
use super::super::{Error, Failure, below};
use super::ObjectHost;
use crate::shop_table::ShopTable;
use pcx_script::{HandleId, Value};
use std::sync::Arc;

const SET_ALL_ITEM: &str = "SetAllItemTypeSlots";
const SET_ALL_UNIT: &str = "SetAllUnitTypeSlots";
const SET_ITEM: &str = "SetItemTypeSlots";
const SET_UNIT: &str = "SetUnitTypeSlots";
const ADD_ITEM: &str = "AddItemToStock";
const REMOVE_ITEM: &str = "RemoveItemFromStock";

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

// Shop item table shape, with the selected executable identity it was read from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShopCapacity {
    table: Arc<ShopTable>,
}

impl ShopCapacity {
    pub fn new(table: ShopTable) -> Self {
        Self {
            table: Arc::new(table),
        }
    }

    pub fn table(&self) -> &ShopTable {
        &self.table
    }

    fn cells(&self) -> usize {
        self.table.entries as usize
    }

    fn slot_ceiling(&self) -> u32 {
        self.table.slot_ceiling
    }
}

/// Caller bound on retained stock records, and the shop shape the records live in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StockBudget {
    /// Bound on distinct retained slot and item stock records across all units.
    pub max_records: usize,
    pub shop: ShopCapacity,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Cell {
    item: Option<i32>,
    /// `(current, max)` integers from the add that last filled the cell.
    stock: Option<(i32, i32)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Shop {
    unit: HandleId,
    cells: Vec<Cell>,
    ages: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct State {
    item_slots: Option<i32>,
    unit_slots: Option<i32>,
    units: Vec<UnitSlots>,
    shops: Vec<Shop>,
    shop: ShopCapacity,
    max_records: usize,
}

/// Index of the cell to evict among `ages`: the greatest age as a signed 32-bit value, lowest
/// index on a tie. The native compare is `jle`, so an age that wrapped to negative is the
/// least recent. `ages` is never empty when this is called with a non-zero count.
fn oldest_cell(ages: &[u32]) -> usize {
    (1..ages.len()).fold(0, |oldest, i| {
        if (ages[i] as i32) > (ages[oldest] as i32) {
            i
        } else {
            oldest
        }
    })
}

/// Unsigned clamp of a stored slot count to the engine ceiling, as `cmovb` performs it.
fn effective_count(slots: i32, ceiling: u32) -> usize {
    (slots as u32).min(ceiling) as usize
}

impl State {
    fn records(&self) -> usize {
        self.units.len()
            + self
                .shops
                .iter()
                .flat_map(|s| &s.cells)
                .filter(|c| c.stock.is_some())
                .count()
    }

    fn item_slots_of(&self, unit: HandleId) -> Option<usize> {
        self.units
            .iter()
            .find(|row| row.unit == unit && row.kind == SlotKind::Item)
            .map(|row| effective_count(row.slots, self.shop.slot_ceiling()))
    }

    fn shop_index(&self, unit: HandleId) -> Option<usize> {
        self.shops.iter().position(|s| s.unit == unit)
    }

    fn set_item_slots(&mut self, unit: HandleId, slots: i32) {
        let count = effective_count(slots, self.shop.slot_ceiling());
        if let Some(at) = self.shop_index(unit) {
            for cell in self.shops[at].cells.iter_mut().skip(count) {
                cell.stock = None;
            }
        }
        match self
            .units
            .iter_mut()
            .find(|row| row.unit == unit && row.kind == SlotKind::Item)
        {
            Some(row) => row.slots = slots,
            None => self.units.push(UnitSlots {
                unit,
                kind: SlotKind::Item,
                slots,
            }),
        }
    }

    fn add_item(&mut self, unit: HandleId, item: i32, stock: (i32, i32)) -> Result<(), Failure> {
        let count = self
            .item_slots_of(unit)
            .ok_or(Failure::StockState("shop slot count is not assigned"))?;
        let at = match self.shop_index(unit) {
            Some(at) => at,
            None => {
                let mut cells = Vec::new();
                cells
                    .try_reserve_exact(self.shop.cells())
                    .map_err(|_| Failure::Capacity)?;
                cells.resize(self.shop.cells(), Cell::default());
                let mut ages = Vec::new();
                ages.try_reserve_exact(self.shop.cells())
                    .map_err(|_| Failure::Capacity)?;
                ages.resize(self.shop.cells(), 0);
                self.shops.push(Shop { unit, cells, ages });
                self.shops.len() - 1
            }
        };
        let shop = &self.shops[at];
        let victim = match (0..count).find(|&i| shop.cells[i].item == Some(item)) {
            Some(found) => found,
            None if count == 0 => 0,
            None => oldest_cell(&shop.ages[..count]),
        };
        if shop.cells[victim].stock.is_none() && self.records() >= self.max_records {
            return Err(Failure::Budget("stock item records"));
        }
        let shop = &mut self.shops[at];
        shop.cells[victim] = Cell {
            item: Some(item),
            stock: Some(stock),
        };
        for age in &mut shop.ages {
            *age = age.wrapping_add(1);
        }
        shop.ages[victim] = 0;
        Ok(())
    }

    fn remove_item(&mut self, unit: HandleId, item: i32) {
        let Some(at) = self.shop_index(unit) else {
            return;
        };
        if let Some(cell) = self.shops[at]
            .cells
            .iter_mut()
            .find(|c| c.item == Some(item))
        {
            *cell = Cell::default();
        }
    }
}

fn stock_native_name(name: &str) -> bool {
    matches!(
        name,
        SET_ALL_ITEM | SET_ALL_UNIT | SET_ITEM | SET_UNIT | ADD_ITEM | REMOVE_ITEM
    )
}

impl ObjectHost {
    /// Admit the four common slot natives and the two stock natives on this Host.
    ///
    /// Each declaration is non-constant, returns nothing, and shares
    /// `CreateTimer`'s source. A function of the same name, a second source,
    /// or a different parameter list refuses admission. `unit` must extend
    /// `handle`. The record bound and the shop shape are checked before any state is stored.
    /// A second call returns `StockState("already admitted")` and consumes
    /// this host, as the other `with_*` admissions do.
    pub fn with_stock(mut self, budget: StockBudget) -> Result<Self, Error> {
        let fail = |reason| self.inner.error(SET_ALL_ITEM, reason);
        if self.stock.is_some() {
            return Err(fail(Failure::StockState("already admitted")));
        }
        std::alloc::Layout::array::<UnitSlots>(budget.max_records)
            .map_err(|_| fail(Failure::Capacity))?;
        if budget.shop.cells() == 0 || budget.shop.slot_ceiling() as usize > budget.shop.cells() {
            return Err(fail(Failure::StockState("shop capacity")));
        }
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
            (ADD_ITEM, &["unit", "integer", "integer", "integer"][..]),
            (REMOVE_ITEM, &["unit", "integer"][..]),
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
            shops: Vec::new(),
            shop: budget.shop,
            max_records: budget.max_records,
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

    /// Explicit `SetItemTypeSlots` record for one live unit, as executed.
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

    /// Count of retained per-unit slot records. Global assignments are not included.
    pub fn stock_unit_records(&self) -> Result<usize, Error> {
        Ok(self.admitted_stock(SET_ITEM)?.units.len())
    }

    /// `(current, max)` integers that filled the cell holding one item in one unit's shop.
    ///
    /// A missing record is `Ok(None)`. An id that is not a live unit is an error.
    pub fn item_stock(&self, unit: HandleId, item: i32) -> Result<Option<(i32, i32)>, Error> {
        self.admitted_stock(ADD_ITEM)?;
        self.stock_unit(&Value::Handle(unit), ADD_ITEM)?;
        let state = self.admitted_stock(ADD_ITEM)?;
        Ok(state.shop_index(unit).and_then(|at| {
            state.shops[at]
                .cells
                .iter()
                .find(|c| c.item == Some(item))
                .and_then(|c| c.stock)
        }))
    }

    /// Count of retained item stock records across all shops.
    pub fn stock_item_records(&self) -> Result<usize, Error> {
        Ok(self
            .admitted_stock(ADD_ITEM)?
            .shops
            .iter()
            .flat_map(|s| &s.cells)
            .filter(|c| c.stock.is_some())
            .count())
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
        if self.stock.is_none() || !stock_native_name(name) {
            return None;
        }
        Some(match name {
            SET_ALL_ITEM => self.set_all_slots(name, args, true),
            SET_ALL_UNIT => self.set_all_slots(name, args, false),
            SET_ITEM => self.set_unit_slots(name, args, SlotKind::Item),
            SET_UNIT => self.set_unit_slots(name, args, SlotKind::Unit),
            ADD_ITEM => self.add_item_stock(name, args),
            REMOVE_ITEM => self.remove_item_stock(name, args),
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
            if kind == SlotKind::Item {
                let known = state
                    .units
                    .iter()
                    .any(|row| row.unit == id && row.kind == kind);
                if !known && state.records() >= state.max_records {
                    Some(Failure::Budget("stock unit slots"))
                } else if !known && state.units.try_reserve(1).is_err() {
                    Some(Failure::Capacity)
                } else {
                    state.set_item_slots(id, slots);
                    None
                }
            } else if let Some(row) = state
                .units
                .iter_mut()
                .find(|row| row.unit == id && row.kind == kind)
            {
                row.slots = slots;
                None
            } else if state.records() >= state.max_records {
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

    fn add_item_stock(&mut self, name: &str, args: &[Value]) -> Result<Value, Error> {
        let [unit, Value::Int(item), Value::Int(current), Value::Int(max)] = args else {
            return Err(self.inner.error(name, Failure::StockState("arguments")));
        };
        let unit = self.stock_unit(unit, name)?;
        let failure = self
            .stock
            .as_mut()
            .expect("stock admitted")
            .add_item(unit, *item, (*current, *max))
            .err();
        match failure {
            None => Ok(Value::Null),
            Some(failure) => Err(self.inner.error(name, failure)),
        }
    }

    fn remove_item_stock(&mut self, name: &str, args: &[Value]) -> Result<Value, Error> {
        let [unit, Value::Int(item)] = args else {
            return Err(self.inner.error(name, Failure::StockState("arguments")));
        };
        let unit = self.stock_unit(unit, name)?;
        self.stock
            .as_mut()
            .expect("stock admitted")
            .remove_item(unit, *item);
        Ok(Value::Null)
    }
}

#[cfg(test)]
#[path = "stock_host_cells_tests.rs"]
mod cell_tests;

const SELL: &str = "purchase";

/// A loaded stock cell that a sale may decrement, located once by [`ObjectHost::stock_sale_cell`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StockCell {
    shop_at: usize,
    cell: usize,
    current: i32,
}

impl StockCell {
    pub(crate) fn current(self) -> i32 {
        self.current
    }
}

impl ObjectHost {
    /// Loaded `currentStock` for one item in one shop. `None` when the item has no cell.
    pub(crate) fn stock_count(&self, shop: HandleId, item: i32) -> Result<Option<i32>, Error> {
        Ok(self.stock_sale_cell(shop, item)?.map(StockCell::current))
    }

    /// The cell that holds `item` in `shop`, with its loaded count. Reads only.
    pub(crate) fn stock_sale_cell(
        &self,
        shop: HandleId,
        item: i32,
    ) -> Result<Option<StockCell>, Error> {
        self.admitted_stock(ADD_ITEM)?;
        self.stock_unit(&Value::Handle(shop), SELL)?;
        let state = self.admitted_stock(ADD_ITEM)?;
        Ok(state.shop_index(shop).and_then(|at| {
            state.shops[at]
                .cells
                .iter()
                .enumerate()
                .find(|(_, c)| c.item == Some(item))
                .and_then(|(cell, c)| {
                    c.stock.map(|(current, _)| StockCell {
                        shop_at: at,
                        cell,
                        current,
                    })
                })
        }))
    }

    /// Lowers the loaded count of a located cell by one sale. The cell was located with a
    /// positive count in the same host state, and no write between the two reaches the stock
    /// table, so the indices are valid and the decrement cannot fail.
    pub(crate) fn apply_stock_sale(&mut self, cell: StockCell) {
        let state = self
            .stock
            .as_mut()
            .expect("located stock implies admitted stock");
        let count = &mut state.shops[cell.shop_at].cells[cell.cell]
            .stock
            .as_mut()
            .expect("located cell holds stock")
            .0;
        *count -= 1;
    }
}

//! `IsItemIdSellable` and `ChooseRandomItemEx` over the loaded item sale rules.
//!
//! `IsItemIdSellable` answers the `sellable` field of the item row. An id with no row
//! answers false, which is what the native's null lookup yields. A row whose sellable
//! value is absent refuses. `ChooseRandomItemEx` picks uniformly among rows whose class
//! ordinal and level match. The native's own candidate order and random draw are not
//! reproduced: candidates are in ascending id order and the draw is one script RNG draw.
//! The `ITEM_TYPE_UNKNOWN` (7) branch is unread, so it refuses rather than guess.
use super::super::{Error, Failure};
use super::{Entry, EventSubject, ObjectHost, SaleContext};
use crate::item_sales::ItemSaleTable;
use pcx_script::{HandleId, Value};

const SELLABLE: &str = "IsItemIdSellable";
const CHOOSE: &str = "ChooseRandomItemEx";
/// `ConvertItemType(7)`, the `ITEM_TYPE_UNKNOWN` value the native branches on.
const UNKNOWN_TYPE: i32 = 7;

impl ObjectHost {
    /// Admits both declarations against the loaded sale rules.
    pub fn with_item_sales(mut self, sales: ItemSaleTable) -> Result<Self, Error> {
        if self.item_sales.is_some() {
            return Err(self
                .inner
                .error(SELLABLE, Failure::ItemState("item sales already admitted")));
        }
        Self::contract(&self.inner, SELLABLE, &["integer"], "boolean", false, None)?;
        Self::contract(
            &self.inner,
            CHOOSE,
            &["itemtype", "integer"],
            "integer",
            false,
            None,
        )?;
        self.item_sales = Some(sales);
        Ok(self)
    }

    pub(super) fn item_sale_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if self.item_sales.is_none() || !matches!(name, SELLABLE | CHOOSE) {
            return None;
        }
        Some(match (name, args) {
            (SELLABLE, [Value::Int(id)]) => self.is_sellable(*id),
            (CHOOSE, [Value::Int(class), Value::Int(level)]) => {
                self.choose_random_item(*class, *level)
            }
            _ => Err(self
                .inner
                .error(name, Failure::ItemState("expected integer arguments"))),
        })
    }

    fn is_sellable(&self, id: i32) -> Result<Value, Error> {
        let sales = self
            .item_sales
            .as_ref()
            .expect("dispatch requires admission");
        match sales.row(id) {
            None => Ok(Value::Bool(false)),
            Some(row) => row.sellable.map(Value::Bool).ok_or_else(|| {
                self.inner.error(
                    SELLABLE,
                    Failure::ItemState("sellable value is not derived"),
                )
            }),
        }
    }

    fn choose_random_item(&mut self, class: i32, level: i32) -> Result<Value, Error> {
        if class == UNKNOWN_TYPE {
            return Err(self.inner.error(
                CHOOSE,
                Failure::ItemState("ITEM_TYPE_UNKNOWN branch is not reconstructed"),
            ));
        }
        let candidates: Vec<i32> = self
            .item_sales
            .as_ref()
            .expect("dispatch requires admission")
            .candidates(class, level)
            .collect();
        if candidates.is_empty() {
            return Ok(Value::Int(0));
        }
        let last = i32::try_from(candidates.len() - 1)
            .map_err(|_| self.inner.error(CHOOSE, Failure::Capacity))?;
        let pick = self.draw_random_integer(0, last, CHOOSE)?;
        let index =
            usize::try_from(pick).map_err(|_| self.inner.error(CHOOSE, Failure::Capacity))?;
        Ok(Value::Int(candidates[index]))
    }
}

/// Why a purchase did not happen. A refusal changes no gold, stock, inventory or event state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PurchaseRefusal {
    NotInStock,
    UnsupportedItem,
    InsufficientGold,
    BuyerUnavailable,
    InventoryDisabled,
    NoFreeSlot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PurchaseOutcome {
    Sold { item: HandleId },
    Refused(PurchaseRefusal),
}

impl ObjectHost {
    /// Sells one loaded stock item from `shop` to `buyer`.
    ///
    /// The first block only reads and reserves. A refusal there changes no gold, stock,
    /// inventory, handle, item-budget or queued event. Unsupported content (powerup or passive
    /// bonus) is classified from the item's source definition before any item is created.
    ///
    /// The second block creates the item with its holder already set, so the only fallible
    /// operation is creation, which checks every budget and capacity before it allocates. The
    /// gold debit, stock decrement and event queueing that follow are infallible: their rows,
    /// cells and queue capacity were located or reserved in the first block.
    ///
    /// Observable reconstruction: the native order handler is unread. The sold item is created
    /// at the shop's authored position and placed without the pickup route, so no pickup event
    /// fires for a sale. The sell event is queued after the state it reports is complete.
    pub fn purchase_stock_item(
        &mut self,
        shop: HandleId,
        buyer: HandleId,
        item: i32,
    ) -> Result<PurchaseOutcome, Error> {
        let name = "purchase";
        let Some(cell) = self
            .stock_sale_cell(shop, item)?
            .filter(|cell| cell.current() > 0)
        else {
            return Ok(PurchaseOutcome::Refused(PurchaseRefusal::NotInStock));
        };
        let price = self
            .item_sales
            .as_ref()
            .ok_or_else(|| {
                self.inner
                    .error(name, Failure::ItemState("item sales not admitted"))
            })?
            .row(item)
            .map(|row| row.gold)
            .ok_or_else(|| {
                self.inner
                    .error(name, Failure::ItemState("sold item has no sale row"))
            })?;
        let (seller, shop_x, shop_y) = {
            let record = self.executed_unit(shop)?;
            (record.owner, record.x, record.y)
        };
        let buyer_record = self.executed_unit(buyer)?;
        if buyer_record.combat.as_ref().is_some_and(|c| c.dead) {
            return Ok(PurchaseOutcome::Refused(PurchaseRefusal::BuyerUnavailable));
        }
        let owner = buyer_record.owner;
        let gold = self.owner_gold(owner)?.ok_or_else(|| {
            self.inner
                .error(name, Failure::PlayerState("gold not admitted"))
        })?;
        if gold < price {
            return Ok(PurchaseOutcome::Refused(PurchaseRefusal::InsufficientGold));
        }
        let policy = self.inventory_policy(buyer)?;
        if !policy.get_items {
            return Ok(PurchaseOutcome::Refused(PurchaseRefusal::InventoryDisabled));
        }
        let Some(slot) = (0..policy.slots).find(|&s| self.held_slot(buyer, s).is_none()) else {
            return Ok(PurchaseOutcome::Refused(PurchaseRefusal::NoFreeSlot));
        };
        let definition = self.item_definition(item)?;
        if self.definition_flag(&definition, "powerup")? {
            return Ok(PurchaseOutcome::Refused(PurchaseRefusal::UnsupportedItem));
        }
        let bonus = self.definition_passive(&definition)?;
        if bonus.strength != 0 || bonus.agility != 0 || bonus.armor != 0 {
            return Ok(PurchaseOutcome::Refused(PurchaseRefusal::UnsupportedItem));
        }
        let sell =
            self.source_constant_enumerand("EVENT_PLAYER_UNIT_SELL_ITEM", "playerunitevent")?;
        let mut triggers = Vec::new();
        for entry in &self.inner.arena {
            if let Entry::RegisteredPlayerUnitEvent(r) = entry
                && r.player == seller
                && r.enumerand == sell
            {
                triggers.push((r.trigger, r.filter, r.playerunitevent));
            }
        }
        let mut events = Vec::with_capacity(triggers.len());
        for (trigger, filter, event) in triggers {
            events.push((trigger, self.resolve_event_filter(filter)?, event));
        }
        let debit = self.prepare_gold_debit_reserving(owner, price, events.len())?;

        // The only fallible write: creation checks every budget before it allocates.
        let item_handle = self.create_item_record(item, shop_x, shop_y, Some((buyer, slot)))?;

        // Every row, cell and queue slot was located or reserved above.
        self.apply_gold_debit(debit)?;
        self.apply_stock_sale(cell);
        let subject = EventSubject {
            sale: Some(SaleContext {
                shop,
                item: item_handle,
            }),
            ..EventSubject::default()
        };
        for (trigger, filter, event) in events {
            self.push_reserved_event(trigger, filter, subject, Some(event));
        }
        Ok(PurchaseOutcome::Sold { item: item_handle })
    }
}

#[cfg(test)]
#[path = "item_sale_tests.rs"]
mod tests;

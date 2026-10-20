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
    /// Reads come first. Refusals cover stock, sale price, buyer liveness, owner gold, the
    /// inventory policy, a free slot, an item the held-bonus path would have to model
    /// (powerup or passive bonus), and an event queue with no room. After the checks the item
    /// is created at the shop's position, placed directly into the buyer's free slot, the
    /// owner's gold is lowered, the stock cell is decremented, and the sell event is queued.
    ///
    /// Observable reconstruction, not measured ordering: the native order handler is unread.
    /// The item is placed without the pickup route, so no pickup event fires for a sale.
    /// Passive and powerup items are refused as unsupported rather than placed without their
    /// effect. The only fallible write after creation is the placement, which is preceded by
    /// the same slot read, so a failure there rolls the created item back to removed.
    pub fn purchase_stock_item(
        &mut self,
        shop: HandleId,
        buyer: HandleId,
        item: i32,
    ) -> Result<PurchaseOutcome, Error> {
        let name = "purchase";
        match self.stock_count(shop, item)? {
            Some(current) if current > 0 => {}
            _ => return Ok(PurchaseOutcome::Refused(PurchaseRefusal::NotInStock)),
        }
        let price = self
            .item_sales
            .as_ref()
            .ok_or_else(|| self.inner.error(name, Failure::ItemState("item sales not admitted")))?
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
        let gold = self
            .owner_gold(owner)?
            .ok_or_else(|| self.inner.error(name, Failure::PlayerState("gold not admitted")))?;
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
        let sell = self.source_constant_enumerand("EVENT_PLAYER_UNIT_SELL_ITEM", "playerunitevent")?;
        let mut triggers = Vec::new();
        for entry in &self.inner.arena {
            if let Entry::RegisteredPlayerUnitEvent(r) = entry
                && r.player == seller
                && r.enumerand == sell
            {
                triggers.push((r.trigger, r.filter, r.playerunitevent));
            }
        }
        for &(_, filter, _) in &triggers {
            if let Some(filter) = filter {
                self.check_event_filter(filter)?;
            }
        }
        let item_handle = self.create_item_at(item, shop_x, shop_y)?;
        let refusal = match self.live_item(item_handle)? {
            Some(_) if self.item_flag(item_handle, "powerup")? => {
                Some(PurchaseRefusal::UnsupportedItem)
            }
            Some(_) => {
                let bonus = self.held_attributes(item_handle)?;
                (bonus.strength != 0 || bonus.agility != 0 || bonus.armor != 0)
                    .then_some(PurchaseRefusal::UnsupportedItem)
            }
            None => Some(PurchaseRefusal::UnsupportedItem),
        };
        if let Some(refusal) = refusal {
            self.remove_created_item(item_handle);
            return Ok(PurchaseOutcome::Refused(refusal));
        }
        // Writes. The slot read above is current because nothing else has run since.
        self.reserve_event_delivery(triggers.len())?;
        if let Entry::Item { record } = self.inner.entry_mut(item_handle)? {
            record.held_by = Some((buyer, slot));
        }
        self.debit_gold(owner, price)?;
        self.take_one_stock(shop, item)?;
        let subject = EventSubject {
            sale: Some(SaleContext {
                shop,
                item: item_handle,
            }),
            ..EventSubject::default()
        };
        for (trigger, filter, event) in triggers {
            self.queue_event(trigger, filter, subject, Some(event))?;
        }
        Ok(PurchaseOutcome::Sold {
            item: item_handle,
        })
    }

    fn check_event_filter(&self, id: HandleId) -> Result<(), Error> {
        match self.inner.entry(id)? {
            Entry::Condition { .. } => self.condition_callback(id).map(|_| ()),
            Entry::Filter { .. } => self.filter_callback(id).map(|_| ()),
            _ => Err(self.inner.error("purchase", Failure::WrongFamily)),
        }
    }

    fn create_item_at(&mut self, item: i32, x: f32, y: f32) -> Result<HandleId, Error> {
        match self.create_item(&[Value::Int(item), Value::Real(x), Value::Real(y)])? {
            Value::Handle(handle) => Ok(handle),
            _ => Err(self
                .inner
                .error("purchase", Failure::ItemState("created item is not a handle"))),
        }
    }

    /// Undoes a creation that has not been placed or announced.
    fn remove_created_item(&mut self, item: HandleId) {
        if let Ok(entry) = self.inner.entry_mut(item) {
            *entry = Entry::RemovedItem;
        }
    }
}

#[cfg(test)]
#[path = "item_sale_tests.rs"]
mod tests;

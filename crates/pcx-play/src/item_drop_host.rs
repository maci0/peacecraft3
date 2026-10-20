//! `SetItemDropID` records the drop origin of an executed item. `UnitDropItem` passes
//! `GetUnitTypeId(inUnit)` as the integer (Blizzard.j:10226). The declared script API
//! has no getter, so no script-visible query reads the value. Drop behavior that the
//! original engine derives from it is not implemented here.
use super::super::{Error, Failure};
use super::ObjectHost;
use pcx_script::{HandleId, Value};
use std::collections::BTreeMap;

const SET: &str = "SetItemDropID";

/// Drop origin per executed item. Handles are never reused, so an entry outlives its
/// item without ever attaching to another one.
pub(super) struct ItemDrops {
    max_records: usize,
    unit_ids: BTreeMap<HandleId, i32>,
}

impl ObjectHost {
    /// Validates `SetItemDropID` against the selected source. `max_records` caps the
    /// items that carry a drop origin, and it is the item budget's record count.
    pub fn with_item_drops(mut self, max_records: usize) -> Result<Self, Error> {
        if self.item_drops.is_some() {
            return Err(self
                .inner
                .error(SET, Failure::ItemState("item drops already admitted")));
        }
        Self::contract(
            &self.inner,
            SET,
            &["item", "integer"],
            "nothing",
            false,
            None,
        )?;
        self.item_drops = Some(ItemDrops {
            max_records,
            unit_ids: BTreeMap::new(),
        });
        Ok(self)
    }

    /// The executed integer last set for `item`, or `None` if it was never set.
    pub fn item_drop_id(&self, item: HandleId) -> Option<i32> {
        self.item_drops.as_ref()?.unit_ids.get(&item).copied()
    }

    pub(super) fn item_drop_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if name != SET || self.item_drops.is_none() {
            return None;
        }
        Some(self.set_item_drop_id(args))
    }

    fn set_item_drop_id(&mut self, args: &[Value]) -> Result<Value, Error> {
        let fail = |reason| self.inner.error(SET, reason);
        let [Value::Handle(item), Value::Int(unit_id)] = args else {
            return Err(fail(Failure::ItemState("expected item handle and integer")));
        };
        if self.live_item(*item)?.is_none() {
            return Err(fail(Failure::ItemState("absent item")));
        }
        let drops = self
            .item_drops
            .as_mut()
            .expect("dispatch requires admission");
        if !drops.unit_ids.contains_key(item) && drops.unit_ids.len() >= drops.max_records {
            return Err(fail(Failure::Budget("item drop origins")));
        }
        drops.unit_ids.insert(*item, *unit_id);
        Ok(Value::Null)
    }
}

#[cfg(test)]
#[path = "item_drop_tests.rs"]
mod tests;

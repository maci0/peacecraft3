//! `IsItemIdSellable` and `ChooseRandomItemEx` over the loaded item sale rules.
//!
//! `IsItemIdSellable` answers the `sellable` field of the item row. An id with no row
//! answers false, which is what the native's null lookup yields. A row whose sellable
//! value is absent refuses. `ChooseRandomItemEx` picks uniformly among rows whose class
//! ordinal and level match. The native's own candidate order and random draw are not
//! reproduced: candidates are in ascending id order and the draw is one script RNG draw.
//! The `ITEM_TYPE_UNKNOWN` (7) branch is unread, so it refuses rather than guess.
use super::super::{Error, Failure};
use super::ObjectHost;
use crate::item_sales::ItemSaleTable;
use pcx_script::Value;

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

#[cfg(test)]
#[path = "item_sale_tests.rs"]
mod tests;

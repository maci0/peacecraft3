//! Item sale rules read from loaded item metadata: gold cost, sellable flag, level and class.
//!
//! `ItemData` supplies `goldcost`, `sellable`, `Level` and `class` for every item row. The
//! class text is mapped with the same ordinal table that `GetItemType` uses. Rows and their
//! fields come from the loaded editor, not from a list in this crate. A field the metadata
//! does not carry for an item stays `None`, and the native that needs it refuses.
use crate::item_class_table::ItemClassTable;
use crate::scripts::AssetIdentity;
use pcx_objects::editor::{
    SlkId,
    defaults::{DefaultTables, Family},
};
use std::collections::BTreeMap;
use thiserror::Error;

const LEVEL_COLUMN: &str = "Level";
const CLASS_COLUMN: &str = "class";
const GOLD_COLUMN: &str = "goldcost";
const SELLABLE_COLUMN: &str = "sellable";

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ItemSaleError {
    #[error("item rawcode {0:?} is not a four-character identifier")]
    Rawcode(String),
    #[error("item {0:?} has no gold cost")]
    GoldCost(String),
    #[error("item {0:?} is unresolved: {1}")]
    Unresolved(String, String),
}

/// One item row. `None` marks a value the metadata does not carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemSale {
    pub level: Option<i32>,
    pub class_ordinal: i32,
    pub gold: i32,
    pub sellable: Option<bool>,
}

/// Sale rules for every item row of one loaded metadata set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemSaleTable {
    /// Edition, version and source of the loaded metadata.
    pub origin: AssetIdentity,
    /// Keyed by the big-endian rawcode, the id a script passes.
    rows: BTreeMap<i32, ItemSale>,
}

impl ItemSaleTable {
    /// Rows keyed by the id a script passes; a fixture or a derived table.
    pub fn new(origin: AssetIdentity, rows: BTreeMap<i32, ItemSale>) -> Self {
        Self { origin, rows }
    }

    pub fn row(&self, id: i32) -> Option<&ItemSale> {
        self.rows.get(&id)
    }

    /// Ids of rows with the class ordinal and level, in ascending id order.
    pub fn candidates(&self, class_ordinal: i32, level: i32) -> impl Iterator<Item = i32> + '_ {
        self.rows
            .iter()
            .filter(move |(_, row)| row.class_ordinal == class_ordinal && row.level == Some(level))
            .map(|(id, _)| *id)
    }
}

/// Derives the sale rows from loaded item metadata and the executable's class ordinals.
///
/// Columns are the `ItemData.slk` headers `goldcost`, `Level`, `class` and `sellable`, read
/// verbatim from the loaded table. A missing `goldcost` refuses the whole table, because a
/// purchase needs the price. A missing `sellable` or `Level` stays `None` for that row.
pub fn derive(
    tables: &DefaultTables,
    classes: &ItemClassTable,
    origin: AssetIdentity,
) -> Result<ItemSaleTable, ItemSaleError> {
    let mut rows = BTreeMap::new();
    for raw in tables.rawcodes(Family::Item) {
        let bytes: [u8; 4] = raw
            .as_bytes()
            .try_into()
            .map_err(|_| ItemSaleError::Rawcode(raw.to_owned()))?;
        let object = tables
            .object(Family::Item, raw)
            .map_err(|e| ItemSaleError::Unresolved(raw.to_owned(), e.to_string()))?;
        let text = |column: &str| {
            object
                .cell(SlkId::ItemData, column)
                .ok()
                .map(|cell| cell.text.trim())
        };
        let integer = |column: &str| {
            text(column)
                .and_then(|t| t.parse::<i64>().ok())
                .and_then(|v| i32::try_from(v).ok())
        };
        let gold = integer(GOLD_COLUMN).ok_or_else(|| ItemSaleError::GoldCost(raw.to_owned()))?;
        // Absent or unmatched class text takes the chain default, as `GetItemType` does.
        let class_ordinal = text(CLASS_COLUMN)
            .filter(|t| !t.is_empty())
            .and_then(|t| {
                classes
                    .ordinals
                    .iter()
                    .find(|(name, _)| name.eq_ignore_ascii_case(t))
                    .map(|(_, n)| *n)
            })
            .unwrap_or(classes.default);
        let sellable = text(SELLABLE_COLUMN).and_then(|t| match t {
            "1" => Some(true),
            "0" => Some(false),
            _ => None,
        });
        rows.insert(
            i32::from_be_bytes(bytes),
            ItemSale {
                level: integer(LEVEL_COLUMN),
                class_ordinal,
                gold,
                sellable,
            },
        );
    }
    Ok(ItemSaleTable::new(origin, rows))
}

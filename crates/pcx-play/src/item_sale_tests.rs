use super::*;
use crate::item_sales::{ItemSale, ItemSaleTable};
use crate::playercolor_host::{EnumHost, Limits};
use crate::scripts::AssetIdentity;
use pcx_script::{Host, Prelude};
use std::collections::BTreeMap;

const DECLARATION: &str = "\
type agent extends handle
type timer extends agent
native CreateTimer takes nothing returns timer
native DestroyTimer takes timer t returns nothing
type itemtype extends handle
native IsItemIdSellable takes integer itemId returns boolean
native ChooseRandomItemEx takes itemtype whichType, integer level returns integer
constant native ConvertItemType takes integer i returns itemtype
globals
constant itemtype ITEM_TYPE_PERMANENT = ConvertItemType(0)
constant itemtype ITEM_TYPE_CHARGED = ConvertItemType(1)
constant itemtype ITEM_TYPE_UNKNOWN = ConvertItemType(7)
endglobals
";

const SWORD: i32 = 0x73776f72; // 'swor'
const SHIELD: i32 = 0x73686c64; // 'shld'
const ORB: i32 = 0x6f726220; // 'orb '
const UNSOLD: i32 = 0x756e7361; // 'unsa'

fn origin() -> AssetIdentity {
    AssetIdentity {
        edition: "synthetic".into(),
        version: "synthetic".into(),
        source: "synthetic".into(),
    }
}

fn sale(class_ordinal: i32, level: Option<i32>, sellable: Option<bool>) -> ItemSale {
    ItemSale {
        level,
        class_ordinal,
        gold: 100,
        sellable,
    }
}

fn rows() -> BTreeMap<i32, ItemSale> {
    BTreeMap::from([
        (SWORD, sale(0, Some(3), Some(true))),
        (SHIELD, sale(0, Some(3), Some(false))),
        (ORB, sale(1, Some(3), Some(true))),
        (UNSOLD, sale(0, Some(3), None)),
    ])
}

fn host(table: ItemSaleTable) -> ObjectHost {
    let common = pcx_script::parse_script(DECLARATION).unwrap();
    let map = pcx_script::parse_script("").unwrap();
    let program = std::sync::Arc::new(
        pcx_script::resolve_with(&map, &[Prelude::new("synthetic:common", &common)]).unwrap(),
    );
    let enums = EnumHost::checked_selected(
        program,
        "synthetic:map",
        &["ConvertItemType"],
        Limits {
            max_handles: 16,
            admission_steps: 1024,
            expression_depth: 16,
            expression_work: 1024,
        },
    )
    .unwrap();
    ObjectHost::checked(enums)
        .unwrap()
        .with_item_sales(table)
        .unwrap()
}

fn table() -> ItemSaleTable {
    ItemSaleTable::new(origin(), rows())
}

#[test]
fn candidates_are_the_rows_of_one_class_and_level_in_id_order() {
    let table = table();
    assert_eq!(
        table.candidates(0, 3).collect::<Vec<_>>(),
        vec![SHIELD, SWORD, UNSOLD]
    );
    assert_eq!(table.candidates(1, 3).collect::<Vec<_>>(), vec![ORB]);
    assert_eq!(table.candidates(0, 4).count(), 0);
}

#[test]
fn sellable_answers_the_loaded_flag_and_refuses_an_absent_one() {
    let mut h = host(table());
    let sellable = |h: &mut ObjectHost, id: i32| h.native("IsItemIdSellable", &[Value::Int(id)]);
    assert_eq!(sellable(&mut h, SWORD).unwrap(), Value::Bool(true));
    assert_eq!(sellable(&mut h, SHIELD).unwrap(), Value::Bool(false));
    assert_eq!(sellable(&mut h, 0x7a7a7a7a).unwrap(), Value::Bool(false));
    let refused = sellable(&mut h, UNSOLD).unwrap_err();
    assert!(
        refused.contains("sellable value is not derived"),
        "{refused}"
    );
}

#[test]
fn choose_with_no_candidates_answers_zero_without_drawing() {
    let mut h = host(table());
    let choose = |h: &mut ObjectHost, class: i32, level: i32| {
        h.native(
            "ChooseRandomItemEx",
            &[Value::Int(class), Value::Int(level)],
        )
    };
    assert_eq!(choose(&mut h, 0, 9).unwrap(), Value::Int(0));
    assert_eq!(choose(&mut h, 1, 0).unwrap(), Value::Int(0));
}

#[test]
fn the_unknown_type_branch_refuses_instead_of_guessing() {
    let mut h = host(table());
    let refused = h
        .native("ChooseRandomItemEx", &[Value::Int(7), Value::Int(3)])
        .unwrap_err();
    assert!(refused.contains("ITEM_TYPE_UNKNOWN"), "{refused}");
}

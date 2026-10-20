use super::super::ItemBudget;
use super::super::Entry;
use super::*;
use crate::playercolor_host::{EnumHost, Limits};
use pcx_objects::editor::defaults::{DefaultTables, Identity};
use pcx_objects::editor::{EditorReadLimits, ObjectKind, Overlay, SlkId};
use pcx_script::{Host, Prelude};
use pcx_sim::world::{Definition, FieldValue, ObjectCatalog, ObjectField, ResolutionProblem, Source};
use std::collections::BTreeMap;
use std::sync::Arc;

const DECLARATION: &str = "\
native GetItemCharges takes item whichItem returns integer
native SetItemCharges takes item whichItem, integer charges returns nothing
";
const BASE: &str = "\
type agent extends handle
type timer extends agent
type widget extends agent
type item extends widget
native CreateTimer takes nothing returns timer
native DestroyTimer takes timer t returns nothing
native CreateItem takes integer typeId, real x, real y returns item
";

fn sylk(headers: &[&str], rows: &[&[&str]]) -> Vec<u8> {
    let mut text = String::from("ID;PWXL;N;E\r\n");
    for (y, row) in std::iter::once(headers)
        .chain(rows.iter().copied())
        .enumerate()
    {
        for (x, value) in row.iter().enumerate() {
            text.push_str(&format!("C;X{};Y{};K\"{}\"\r\n", x + 1, y + 1, value));
        }
    }
    text.push_str("E\r\n");
    text.into_bytes()
}

/// `AIh1` has level-1 `Cool1` 20; `AIh2` has 0. Both share the `AIhe` code.
fn rules() -> Rules {
    let ability = sylk(
        &["alias", "code", "levels", "Cool1"],
        &[&["AIh1", "AIhe", "1", "20"], &["AIh2", "AIhe", "1", "0"]],
    );
    let tables = Arc::new(
        DefaultTables::from_bytes(
            Identity {
                install: "synthetic item use host".into(),
                edition_version: "fixture".into(),
            },
            &[(SlkId::AbilityData.path(), &ability)],
            &[],
            EditorReadLimits {
                max_listed_paths: 4,
                max_asset_bytes: 65536,
                max_aggregate_input_bytes: 131072,
            },
        )
        .unwrap(),
    );
    Rules::new(tables, Overlay::default())
}

fn cell(text: &str) -> ObjectField {
    ObjectField {
        metadata: None,
        value: Ok(FieldValue::Cell(text.into())),
    }
}

/// `POT1`/`POT2`: two charges, one group `AIhe` (20 s). `WAND`: uncharged, no group.
/// `NEGA`: `ignoreCD`, which the host refuses until its native rule is verified. `BAD1`: a
/// `uses` cell that is not a count.
struct Catalog;
impl ObjectCatalog for Catalog {
    fn resolve(&self, rawcode: [u8; 4]) -> Result<Definition, ResolutionProblem> {
        let mut fields = BTreeMap::new();
        let potion = |fields: &mut BTreeMap<[u8; 4], ObjectField>, uses: &str| {
            fields.insert(*b"iuse", cell(uses));
            fields.insert(*b"iper", cell("1"));
            fields.insert(*b"iicd", cell("0"));
            fields.insert(*b"icid", cell("AIhe"));
            fields.insert(*b"iabi", cell("AIh1"));
        };
        match &rawcode {
            b"POT1" | b"POT2" => potion(&mut fields, "2"),
            b"NEGA" => {
                potion(&mut fields, "3");
                fields.insert(*b"iper", cell("0"));
                fields.insert(*b"iicd", cell("1"));
            }
            b"BAD1" => potion(&mut fields, "many"),
            b"WAND" => {
                fields.insert(*b"iuse", cell("-"));
                fields.insert(*b"iper", cell("0"));
                fields.insert(*b"iicd", cell("0"));
                fields.insert(*b"iabi", cell("_"));
            }
            _ => {}
        }
        Ok(Definition {
            rawcode,
            base_rawcode: "synthetic base".into(),
            kind: ObjectKind::Item,
            fields,
            modifications: Vec::new(),
        })
    }
}

fn source() -> Source {
    Source {
        install: "synthetic install".into(),
        edition: "synthetic edition/revision".into(),
        map: "synthetic map".into(),
    }
}

fn program() -> std::sync::Arc<pcx_script::Program> {
    let common = pcx_script::parse_script(&format!("{BASE}{DECLARATION}")).unwrap();
    let map = pcx_script::parse_script("").unwrap();
    std::sync::Arc::new(
        pcx_script::resolve_with(&map, &[Prelude::new("synthetic:common", &common)]).unwrap(),
    )
}

fn enums() -> EnumHost {
    EnumHost::checked_selected(
        program(),
        "synthetic:map",
        &[],
        Limits {
            max_handles: 16,
            admission_steps: 1024,
            expression_depth: 16,
            expression_work: 1024,
        },
    )
    .unwrap()
}

fn host(max_records: usize) -> ObjectHost {
    ObjectHost::checked(enums())
        .unwrap()
        .with_item_catalog(
            std::sync::Arc::new(Catalog),
            source(),
            ItemBudget {
                max_records: 8,
                max_retained_fields: 16,
                max_modifications: 8,
            },
        )
        .unwrap()
        .with_item_use(rules(), max_records, CooldownScope::SharedGroup)
        .unwrap()
}

fn item(h: &mut ObjectHost, raw: &[u8; 4]) -> HandleId {
    let Value::Handle(id) = h
        .native(
            "CreateItem",
            &[Value::Int(i32::from_be_bytes(*raw)), Value::Real(0.0), Value::Real(0.0)],
        )
        .unwrap()
    else {
        panic!("CreateItem returns a handle");
    };
    id
}

fn charges(h: &mut ObjectHost, id: HandleId) -> Value {
    h.native("GetItemCharges", &[Value::Handle(id)]).unwrap()
}

fn set(h: &mut ObjectHost, id: HandleId, count: i32) {
    h.native("SetItemCharges", &[Value::Handle(id), Value::Int(count)]).unwrap();
}

#[test]
fn admission_requires_items_and_the_declared_natives_once() {
    let base = ObjectHost::checked(enums()).unwrap();
    assert!(base.with_item_use(rules(), 1, CooldownScope::SharedGroup).is_err(), "items are required first");
    let h = host(4);
    assert!(h.item_use.is_some());
    assert!(host(4).with_item_use(rules(), 4, CooldownScope::SharedGroup).is_err(), "second admission refuses");
}

#[test]
fn get_reads_the_authored_count_and_set_replaces_it() {
    let mut h = host(4);
    let potion = item(&mut h, b"POT1");
    assert_eq!(charges(&mut h, potion), Value::Int(2));
    set(&mut h, potion, 7);
    assert_eq!(charges(&mut h, potion), Value::Int(7));
}

#[test]
fn null_item_reads_zero_and_writes_nothing_as_the_binary_does() {
    let mut h = host(4);
    assert_eq!(h.native("GetItemCharges", &[Value::Null]).unwrap(), Value::Int(0));
    assert_eq!(h.native("SetItemCharges", &[Value::Null, Value::Int(3)]).unwrap(), Value::Null);
    assert!(h.item_use.as_ref().unwrap().charges.is_empty());
}

#[test]
fn negative_write_stores_zero_as_the_binary_does() {
    let mut h = host(4);
    let potion = item(&mut h, b"POT1");
    set(&mut h, potion, -5);
    assert_eq!(charges(&mut h, potion), Value::Int(0));
}

#[test]
fn removed_item_reads_zero_and_ignores_writes() {
    let mut h = host(4);
    let potion = item(&mut h, b"POT1");
    *h.inner.entry_mut(potion).unwrap() = Entry::RemovedItem;
    assert_eq!(charges(&mut h, potion), Value::Int(0));
    set(&mut h, potion, 4);
    assert!(h.item_use.as_ref().unwrap().charges.is_empty());
}

#[test]
fn uncharged_items_read_zero_and_non_item_handles_refuse() {
    let mut h = host(4);
    let wand = item(&mut h, b"WAND");
    assert_eq!(charges(&mut h, wand), Value::Int(0));
    assert!(h.native("SetItemCharges", &[Value::Handle(wand), Value::Real(1.0)]).is_err());
    assert!(h.native("GetItemCharges", &[Value::Real(1.0)]).is_err());
}

#[test]
fn a_malformed_uses_cell_refuses_loudly_and_writes_nothing() {
    let mut h = host(4);
    let bad = item(&mut h, b"BAD1");
    assert!(h.native("GetItemCharges", &[Value::Handle(bad)]).is_err());
    assert!(h.item_use.as_ref().unwrap().charges.is_empty());
}

#[test]
fn the_charge_table_is_bounded_by_the_item_budget() {
    let mut h = host(1);
    let a = item(&mut h, b"POT1");
    let b = item(&mut h, b"POT2");
    set(&mut h, a, 1);
    assert!(h.native("SetItemCharges", &[Value::Handle(b), Value::Int(1)]).is_err());
    set(&mut h, a, 4);
    assert_eq!(charges(&mut h, a), Value::Int(4));
}

#[test]
fn an_unadmitted_host_does_not_dispatch_the_charge_natives() {
    let mut h = ObjectHost::checked(enums()).unwrap();
    assert!(h.native("GetItemCharges", &[Value::Null]).is_err());
}

#[test]
fn check_refuses_without_an_admitted_game_clock() {
    let mut h = host(4);
    let item = item(&mut h, b"POT1");
    let err = h.check_item_use(item, item).unwrap_err();
    assert!(err.to_string().contains("game clock"), "{err}");
    assert!(h.item_use.as_ref().unwrap().ready_at.is_empty());
}

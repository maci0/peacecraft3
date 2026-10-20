use super::*;
use crate::playercolor_host::{EnumHost, Limits};
use super::super::ItemBudget;
use pcx_script::{Host, Prelude};
use pcx_sim::world::{Definition, ObjectCatalog, ResolutionProblem, Source};
use pcx_objects::editor::ObjectKind;
use std::collections::BTreeMap;

const DECLARATION: &str = "native SetItemDropID takes item whichItem, integer unitId returns nothing\n";
const BASE: &str = "\
type agent extends handle
type timer extends agent
type widget extends agent
type item extends widget
native CreateTimer takes nothing returns timer
native DestroyTimer takes timer t returns nothing
native CreateItem takes integer typeId, real x, real y returns item
";

struct Catalog;
impl ObjectCatalog for Catalog {
    fn resolve(&self, rawcode: [u8; 4]) -> Result<Definition, ResolutionProblem> {
        Ok(Definition {
            rawcode,
            base_rawcode: "synthetic base".into(),
            kind: ObjectKind::Item,
            fields: BTreeMap::new(),
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

fn program(declaration: &str) -> std::sync::Arc<pcx_script::Program> {
    let common = pcx_script::parse_script(&format!("{BASE}{declaration}")).unwrap();
    let map = pcx_script::parse_script("").unwrap();
    std::sync::Arc::new(
        pcx_script::resolve_with(&map, &[Prelude::new("synthetic:common", &common)]).unwrap(),
    )
}

fn enums(declaration: &str) -> EnumHost {
    EnumHost::checked_selected(
        program(declaration),
        "synthetic:map",
        &[],
        Limits {
            max_handles: 8,
            admission_steps: 1024,
            expression_depth: 16,
            expression_work: 1024,
        },
    )
    .unwrap()
}

fn host(max_records: usize) -> ObjectHost {
    ObjectHost::checked(enums(DECLARATION))
        .unwrap()
        .with_item_catalog(
            std::sync::Arc::new(Catalog),
            source(),
            ItemBudget {
                max_records: 8,
                max_retained_fields: 8,
                max_modifications: 8,
            },
        )
        .unwrap()
        .with_item_drops(max_records)
        .unwrap()
}

fn item(h: &mut ObjectHost, x: f32) -> HandleId {
    let Value::Handle(id) = h
        .native(
            "CreateItem",
            &[Value::Int(i32::from_be_bytes(*b"ITM1")), Value::Real(x), Value::Real(0.0)],
        )
        .unwrap()
    else {
        panic!("CreateItem returns a handle");
    };
    id
}

#[test]
fn admission_requires_the_declared_signature_once() {
    let base = ObjectHost::checked(enums(DECLARATION)).unwrap();
    assert!(base.with_item_drops(1).is_ok());
    let wrong = ObjectHost::checked(enums(
        "native SetItemDropID takes item whichItem, real unitId returns nothing\n",
    ))
    .unwrap();
    assert!(wrong.with_item_drops(1).is_err());
    let missing = ObjectHost::checked(enums("")).unwrap();
    assert!(missing.with_item_drops(1).is_err());
    assert!(host(1).with_item_drops(1).is_err());
}

#[test]
fn set_records_the_executed_integer_for_each_item() {
    let mut h = host(4);
    let a = item(&mut h, 1.0);
    let b = item(&mut h, 2.0);
    assert_eq!(h.item_drop_id(a), None);
    h.native("SetItemDropID", &[Value::Handle(a), Value::Int(1234)]).unwrap();
    assert_eq!(h.item_drop_id(a), Some(1234));
    assert_eq!(h.item_drop_id(b), None);
    h.native("SetItemDropID", &[Value::Handle(a), Value::Int(-7)]).unwrap();
    assert_eq!(h.item_drop_id(a), Some(-7));
}

#[test]
fn refuses_a_non_item_handle_and_leaves_the_table_unchanged() {
    let mut h = host(4);
    let timer = h.native("CreateTimer", &[]).unwrap();
    assert!(h.native("SetItemDropID", &[timer, Value::Int(1)]).is_err());
    assert!(h.item_drops.as_ref().unwrap().unit_ids.is_empty());
}

#[test]
fn refuses_a_real_where_the_integer_is_declared() {
    let mut h = host(4);
    let a = item(&mut h, 1.0);
    assert!(h.native("SetItemDropID", &[Value::Handle(a), Value::Real(1.0)]).is_err());
    assert_eq!(h.item_drop_id(a), None);
}

#[test]
fn budget_bounds_items_with_drop_origins_but_not_resets() {
    let mut h = host(1);
    let a = item(&mut h, 1.0);
    let b = item(&mut h, 2.0);
    h.native("SetItemDropID", &[Value::Handle(a), Value::Int(1)]).unwrap();
    assert!(h.native("SetItemDropID", &[Value::Handle(b), Value::Int(2)]).is_err());
    assert_eq!(h.item_drop_id(b), None);
    h.native("SetItemDropID", &[Value::Handle(a), Value::Int(3)]).unwrap();
    assert_eq!(h.item_drop_id(a), Some(3));
}

#[test]
fn unadmitted_host_does_not_dispatch_the_native() {
    let mut h = ObjectHost::checked(enums(DECLARATION)).unwrap();
    assert!(h.native("SetItemDropID", &[Value::Null, Value::Int(1)]).is_err());
}

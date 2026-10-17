use super::{COMMON, context, object};
use crate::playercolor_host::{Failure, ItemBudget, ObjectHost};
use pcx_script::{Host, Value};
use pcx_sim::world::{
    Definition, FieldValue, ObjectCatalog, ObjectField, ResolutionProblem, Source,
};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

struct Counting {
    hits: Arc<AtomicUsize>,
    seen: Arc<Mutex<Vec<[u8; 4]>>>,
    item: [u8; 4],
    unit: [u8; 4],
    mismatch: [u8; 4],
    high: [u8; 4],
    high_base: String,
    fields: usize,
    modifications: usize,
}

fn definition(
    rawcode: [u8; 4],
    base_rawcode: String,
    kind: pcx_objects::editor::ObjectKind,
    fields: usize,
    modifications: usize,
) -> Definition {
    let mut retained = BTreeMap::new();
    for index in 0..fields {
        let key = (index as u32).to_be_bytes();
        retained.insert(
            key,
            ObjectField {
                metadata: None,
                value: Ok(FieldValue::Cell("fixture-cell".into())),
            },
        );
    }
    Definition {
        rawcode,
        base_rawcode,
        kind,
        fields: retained,
        modifications: std::iter::repeat_with(|| pcx_objects::Mod {
            field: *b"fxid",
            value_type: pcx_objects::ValueType::Int,
            selectors: None,
            value: pcx_objects::Value::Int(0),
            end_word: 0,
        })
        .take(modifications)
        .collect(),
    }
}

impl ObjectCatalog for Counting {
    fn resolve(&self, rawcode: [u8; 4]) -> Result<Definition, ResolutionProblem> {
        self.hits.fetch_add(1, Ordering::SeqCst);
        self.seen.lock().expect("seen").push(rawcode);
        if rawcode == self.item || rawcode == self.high {
            let base = if rawcode == self.high {
                self.high_base.clone()
            } else {
                pcx_objects::fourcc_text(rawcode)
            };
            Ok(definition(
                rawcode,
                base,
                pcx_objects::editor::ObjectKind::Item,
                self.fields,
                self.modifications,
            ))
        } else if rawcode == self.unit {
            Ok(definition(
                rawcode,
                pcx_objects::fourcc_text(rawcode),
                pcx_objects::editor::ObjectKind::Unit,
                0,
                0,
            ))
        } else if rawcode == self.mismatch {
            Ok(definition(
                *b"nope",
                pcx_objects::fourcc_text(*b"nope"),
                pcx_objects::editor::ObjectKind::Item,
                0,
                0,
            ))
        } else {
            Err(ResolutionProblem::MissingRawcode(pcx_objects::fourcc_text(
                rawcode,
            )))
        }
    }
}

fn script_common(item_parent: &str, declare_create: bool) -> String {
    let globals = COMMON.find("\nglobals\n").expect("globals");
    let mut common = COMMON.to_string();
    let native = if declare_create {
        "native CreateItem takes integer itemid, real x, real y returns item\n"
    } else {
        ""
    };
    common.insert_str(
        globals + 1,
        &format!("type widget extends agent\ntype item extends {item_parent}\n{native}"),
    );
    common
}

fn source() -> Source {
    Source {
        install: "selected-install".into(),
        edition: "selected-edition".into(),
        map: "item-catalog".into(),
    }
}

fn limits(max_records: usize) -> ItemBudget {
    ItemBudget {
        max_records,
        max_retained_fields: 4,
        max_modifications: 0,
    }
}

fn counting(
    fields: usize,
    modifications: usize,
) -> (Arc<Counting>, Arc<AtomicUsize>, Arc<Mutex<Vec<[u8; 4]>>>) {
    let hits = Arc::new(AtomicUsize::new(0));
    let seen = Arc::new(Mutex::new(Vec::new()));
    let catalog = Arc::new(Counting {
        hits: Arc::clone(&hits),
        seen: Arc::clone(&seen),
        item: *b"gold",
        unit: *b"hfoo",
        mismatch: *b"bad!",
        high: i32::from_be_bytes([0xff, 0x80, 0x7f, 0x01]).to_be_bytes(),
        high_base: "\\xff\\x80\\x7f\\x01".into(),
        fields,
        modifications,
    });
    (catalog, hits, seen)
}

struct VmHost {
    vm: pcx_script::OwnedVm<crate::scripts::OwnedLocalizedHost<ObjectHost>>,
    hits: Arc<AtomicUsize>,
    seen: Arc<Mutex<Vec<[u8; 4]>>>,
    catalog: Arc<dyn ObjectCatalog + Send + Sync>,
}

fn open(max_records: usize, max_handles: usize, map: &str) -> VmHost {
    open_shaped(max_records, max_handles, map, "widget", true, 1, 0)
}

fn open_shaped(
    max_records: usize,
    max_handles: usize,
    map: &str,
    item_parent: &str,
    declare_create: bool,
    fields: usize,
    modifications: usize,
) -> VmHost {
    let common = script_common(item_parent, declare_create);
    let ctx = context(300, &common, map, "item-catalog");
    let (catalog, hits, seen) = counting(fields, modifications);
    let shared: Arc<dyn ObjectCatalog + Send + Sync> = catalog;
    let host = object(&ctx, max_handles)
        .with_item_catalog(Arc::clone(&shared), source(), limits(max_records))
        .unwrap();
    assert!(Arc::ptr_eq(&host.item_catalog().unwrap(), &shared));
    let (declared, span) = host.item_declaration().unwrap();
    assert_eq!(declared, "synthetic/common.j");
    assert!(common[span.start as usize..span.end as usize].contains("CreateItem"));
    VmHost {
        vm: Arc::clone(&ctx.scripts).owned_vm(host),
        hits,
        seen,
        catalog: shared,
    }
}

const MAP: &str = "globals\nitem I = null\ntimer T = null\nendglobals\nfunction main takes nothing returns nothing\nset I = CreateItem('gold', 1.5, -2.25)\nendfunction\nfunction missing takes nothing returns item\nreturn CreateItem('zzzz', 0.0, 0.0)\nendfunction\nfunction unit takes nothing returns item\nreturn CreateItem('hfoo', 0.0, 0.0)\nendfunction\nfunction mismatched takes nothing returns item\nreturn CreateItem('bad!', 0.0, 0.0)\nendfunction\nfunction again takes nothing returns item\nreturn CreateItem('gold', 0.0, 0.0)\nendfunction\n";

#[test]
fn owned_vm_retains_catalog_result_and_typed_resolve_failures() {
    let mut session = open(4, 16, MAP);
    session.vm.init_globals().unwrap();
    session.vm.call("main", &[]).unwrap();
    assert_eq!(session.hits.load(Ordering::SeqCst), 1);
    let Value::Handle(item) = session.vm.global("I") else {
        panic!("item");
    };
    assert_eq!(session.vm.host().inner.inner().kind(item).unwrap(), "item");
    let executed = session.vm.host().inner.executed_item(item).unwrap().clone();
    assert_eq!(executed.definition.rawcode, *b"gold");
    assert_eq!(
        executed.definition.kind,
        pcx_objects::editor::ObjectKind::Item
    );
    assert_eq!(executed.definition.base_rawcode, "gold");
    assert_eq!(executed.source, source());
    assert_eq!(executed.x, 1.5);
    assert_eq!(executed.y, -2.25);
    assert_eq!(
        executed
            .definition
            .fields
            .values()
            .next()
            .and_then(|field| field.value.as_ref().ok()),
        Some(&FieldValue::Cell("fixture-cell".into()))
    );
    assert!(Arc::ptr_eq(
        &session.vm.host().inner.item_catalog().unwrap(),
        &session.catalog
    ));
    let handles = session.vm.host().inner.inner().handles();
    for (name, needle) in [
        (
            "missing",
            "item catalog [122, 122, 122, 122]: MissingRawcode(\"zzzz\")",
        ),
        ("unit", "WrongFamily { expected: Item, actual: Unit }"),
        (
            "mismatched",
            "RawcodeMismatch { requested: [98, 97, 100, 33], resolved: [110, 111, 112, 101] }",
        ),
    ] {
        let error = session.vm.call(name, &[]).unwrap_err().to_string();
        assert!(error.contains(needle), "{name}: {error}");
        assert!(error.contains("synthetic/common.j"), "{name}: {error}");
        assert!(error.contains("CreateItem"), "{name}: {error}");
    }
    assert_eq!(session.vm.host().inner.inner().handles(), handles);
    assert_eq!(session.hits.load(Ordering::SeqCst), 4);
}

#[test]
fn zero_and_full_record_budgets_do_not_resolve() {
    let mut empty = open(0, 8, MAP);
    empty.vm.init_globals().unwrap();
    let error = empty.vm.call("main", &[]).unwrap_err().to_string();
    assert!(error.contains("item creation policy exceeded"), "{error}");
    assert!(error.contains("synthetic/common.j"), "{error}");
    assert_eq!(empty.hits.load(Ordering::SeqCst), 0);
    assert!(empty.seen.lock().expect("seen").is_empty());

    let mut full = open(1, 8, MAP);
    full.vm.init_globals().unwrap();
    full.vm.call("main", &[]).unwrap();
    assert_eq!(full.hits.load(Ordering::SeqCst), 1);
    let error = full.vm.call("again", &[]).unwrap_err().to_string();
    assert!(error.contains("item creation policy exceeded"), "{error}");
    assert_eq!(full.hits.load(Ordering::SeqCst), 1);
    assert_eq!(full.seen.lock().expect("seen").as_slice(), &[*b"gold"]);
}

#[test]
fn high_byte_integer_resolves_unchanged_and_keeps_catalog_base_text() {
    let mut session = open(2, 8, MAP);
    session.vm.init_globals().unwrap();
    session.vm.call("main", &[]).unwrap();
    let id = i32::from_be_bytes([0xff, 0x80, 0x7f, 0x01]);
    let before = session.hits.load(Ordering::SeqCst);
    let created = session
        .vm
        .host()
        .native(
            "CreateItem",
            &[Value::Int(id), Value::Real(3.0), Value::Real(-4.0)],
        )
        .unwrap();
    let Value::Handle(item) = created else {
        panic!("high-byte item");
    };
    assert_eq!(session.hits.load(Ordering::SeqCst), before + 1);
    assert_eq!(
        session.seen.lock().expect("seen").last().copied(),
        Some(id.to_be_bytes())
    );
    let executed = session.vm.host().inner.executed_item(item).unwrap();
    assert_eq!(executed.definition.rawcode, id.to_be_bytes());
    assert_eq!(executed.definition.base_rawcode, "\\xff\\x80\\x7f\\x01");
    assert_eq!((executed.x, executed.y), (3.0, -4.0));
}

#[test]
fn signature_source_and_ancestry_refuse_before_resolve() {
    let shared: Arc<dyn ObjectCatalog + Send + Sync> = counting(0, 0).0;
    let missing = script_common("widget", false);
    let ctx = context(300, &missing, MAP, "item-catalog");
    let error = match object(&ctx, 4).with_item_catalog(Arc::clone(&shared), source(), limits(1)) {
        Err(error) => error,
        Ok(_) => panic!("missing CreateItem was admitted"),
    };
    assert_eq!(error.failure, Failure::Signature);
    assert_eq!(error.native, "CreateItem");

    let ancestry = script_common("agent", true);
    let ctx = context(300, &ancestry, MAP, "item-catalog");
    let error = match object(&ctx, 4).with_item_catalog(Arc::clone(&shared), source(), limits(1)) {
        Err(error) => error,
        Ok(_) => panic!("item ancestry was admitted"),
    };
    assert_eq!(error.failure, Failure::Signature);
    assert_eq!(error.declaration_source, "synthetic/common.j");
    assert_ne!(error.declaration_span, pcx_script::span::Span::EMPTY);

    let wrong = "native CreateItem takes integer itemid, real x, real y returns unit\n";
    let common = script_common("widget", false).replacen(
        "type widget extends agent\n",
        &format!("type widget extends agent\n{wrong}"),
        1,
    );
    let ctx = context(300, &common, MAP, "item-catalog");
    let error = match object(&ctx, 4).with_item_catalog(shared, source(), limits(1)) {
        Err(error) => error,
        Ok(_) => panic!("wrong return was admitted"),
    };
    assert_eq!(error.failure, Failure::Signature);
}

#[test]
fn unadmitted_create_item_is_unsupported_and_reserved_identity_is_not_created() {
    let common = script_common("widget", true);
    let ctx = context(
        300,
        &common,
        "function main takes nothing returns nothing\nendfunction\n",
        "item-catalog",
    );
    let host = object(&ctx, 8);
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host);
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    let error = vm.host().native("CreateItem", &[]).unwrap_err();
    assert!(error.contains("unsupported"), "{error}");

    let host = object(&ctx, 8)
        .with_item_catalog(counting(0, 0).0, source(), limits(1))
        .unwrap();
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host);
    let reserved = vm
        .host()
        .inner
        .inner_mut()
        .reserve_identity("item")
        .unwrap();
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    let created = vm
        .host()
        .native(
            "CreateItem",
            &[
                Value::Int(i32::from_be_bytes(*b"gold")),
                Value::Real(0.0),
                Value::Real(0.0),
            ],
        )
        .unwrap();
    let Value::Handle(item) = created else {
        panic!("created item");
    };
    assert_ne!(item, reserved);
    assert!(vm.host().inner.executed_item(item).is_ok());
    assert!(matches!(
        vm.host().inner.executed_item(reserved).unwrap_err().failure,
        Failure::WrongFamily
    ));
}

#[test]
fn non_finite_capacity_and_field_budgets_do_not_allocate() {
    let mut session = open(2, 8, MAP);
    session.vm.init_globals().unwrap();
    let before = session.vm.host().inner.inner().handles();
    let error = session
        .vm
        .host()
        .native(
            "CreateItem",
            &[
                Value::Int(i32::from_be_bytes(*b"gold")),
                Value::Real(f32::NAN),
                Value::Real(0.0),
            ],
        )
        .unwrap_err();
    assert!(error.contains("finite real required"), "{error}");
    assert_eq!(session.hits.load(Ordering::SeqCst), 0);
    assert_eq!(session.vm.host().inner.inner().handles(), before);

    let mut tight = open(1, 1, MAP);
    tight.vm.init_globals().unwrap();
    let error = tight.vm.call("main", &[]).unwrap_err().to_string();
    assert!(error.contains("capacity"), "{error}");
    assert_eq!(tight.hits.load(Ordering::SeqCst), 0);

    let over_fields = open_shaped(1, 8, MAP, "widget", true, 5, 0);
    assert!(over_fields.vm.call("main", &[]).is_err());
    let over_mods = open_shaped(1, 8, MAP, "widget", true, 0, 1);
    assert!(over_mods.vm.call("main", &[]).is_err());
}

#[test]
fn incomplete_source_and_repeat_admission_are_refused() {
    let shared: Arc<dyn ObjectCatalog + Send + Sync> = counting(0, 0).0;
    let common = script_common("widget", true);
    let ctx = context(300, &common, MAP, "item-catalog");
    let mut incomplete = source();
    incomplete.edition.clear();
    let error = match object(&ctx, 4).with_item_catalog(Arc::clone(&shared), incomplete, limits(1))
    {
        Err(error) => error,
        Ok(_) => panic!("incomplete source was admitted"),
    };
    assert!(matches!(error.failure, Failure::ItemState(_)));
    let admitted = object(&ctx, 4)
        .with_item_catalog(Arc::clone(&shared), source(), limits(1))
        .unwrap();
    let error = match admitted.with_item_catalog(shared, source(), limits(1)) {
        Err(error) => error,
        Ok(_) => panic!("second admission"),
    };
    assert_eq!(error.failure, Failure::ItemState("already admitted"));
}

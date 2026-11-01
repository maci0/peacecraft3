use super::*;
#[path = "widget_enumeration_tests.rs"]
mod widget_enumeration;
use crate::playercolor_host::{EnumHost, Limits};
use pcx_script::{Host, Prelude};
use pcx_sim::world::{FieldValue, ObjectField};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};

const COMMON: &str = "\
type agent extends handle
type widget extends agent
type item extends widget
type timer extends agent
type playercolor extends handle
constant native ConvertPlayerColor takes integer i returns playercolor
native CreateTimer takes nothing returns timer
native DestroyTimer takes timer t returns nothing
native CreateItem takes integer typeId, real x, real y returns item
globals
constant playercolor C = ConvertPlayerColor(9)
endglobals
";

struct Catalog(Arc<AtomicUsize>);
impl ObjectCatalog for Catalog {
    fn resolve(&self, rawcode: [u8; 4]) -> Result<Definition, ResolutionProblem> {
        self.0.fetch_add(1, Ordering::SeqCst);
        if rawcode == *b"MISS" {
            return Err(ResolutionProblem::MissingRawcode("MISS".into()));
        }
        Ok(Definition {
            rawcode: if rawcode == *b"MISM" {
                *b"NOPE"
            } else {
                rawcode
            },
            base_rawcode: "synthetic base".into(),
            kind: if rawcode == *b"UNIT" {
                ObjectKind::Unit
            } else {
                ObjectKind::Item
            },
            fields: BTreeMap::from([(
                *b"TEXT",
                ObjectField {
                    metadata: None,
                    value: Ok(FieldValue::Cell("authored synthetic field".into())),
                },
            )]),
            modifications: if rawcode == *b"MODS" {
                vec![pcx_objects::Mod {
                    field: *b"TEXT",
                    value_type: pcx_objects::ValueType::Str,
                    selectors: None,
                    value: pcx_objects::Value::Str(b"synthetic modification".to_vec()),
                    end_word: 0,
                }]
            } else {
                Vec::new()
            },
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
fn budget() -> ItemBudget {
    ItemBudget {
        max_records: 1,
        max_retained_fields: 1,
        max_modifications: 0,
    }
}
fn host(common: &str, max_handles: usize) -> ObjectHost {
    let common = pcx_script::parse_script(common).unwrap();
    let map = pcx_script::parse_script("").unwrap();
    let program = Arc::new(
        pcx_script::resolve_with(&map, &[Prelude::new("synthetic common", &common)]).unwrap(),
    );
    let inner = EnumHost::checked_selected(
        program,
        "synthetic item map",
        &["ConvertPlayerColor"],
        Limits {
            max_handles,
            admission_steps: 100,
            expression_depth: 8,
            expression_work: 100,
        },
    )
    .unwrap();
    ObjectHost::checked(inner).unwrap()
}
fn args(rawcode: &[u8; 4]) -> [Value; 3] {
    [
        Value::Int(i32::from_be_bytes(*rawcode)),
        Value::Int(-4),
        Value::Real(-0.0),
    ]
}
fn handle(value: Value) -> HandleId {
    let Value::Handle(id) = value else {
        panic!("handle")
    };
    id
}

#[test]
fn creation_requires_opt_in_and_shares_the_existing_timer_arena() {
    let hits = Arc::new(AtomicUsize::new(0));
    let catalog: Arc<dyn ObjectCatalog + Send + Sync> = Arc::new(Catalog(Arc::clone(&hits)));
    let mut host = host(COMMON, 8);
    assert!(host.native(CREATE, &args(b"ITEM")).is_err());
    let mut host = host.with_item_catalog(catalog, source(), budget()).unwrap();
    let before = handle(host.native("CreateTimer", &[]).unwrap());
    let item = handle(host.native(CREATE, &args(b"ITEM")).unwrap());
    let after = handle(host.native("CreateTimer", &[]).unwrap());
    assert_eq!(item.token(), before.token() + 1);
    assert_eq!(after.token(), item.token() + 1);
    assert_eq!(host.inner().kind(item).unwrap(), "item");
    let record = host.executed_item(item).unwrap();
    assert_eq!(record.definition.rawcode, *b"ITEM");
    assert_eq!(record.definition.kind, ObjectKind::Item);
    assert_eq!(record.source, source());
    assert_eq!(record.x, -4.0);
    assert_eq!(record.y.to_bits(), (-0.0f32).to_bits());
    assert_eq!(
        record.definition.fields[b"TEXT"].value,
        Ok(FieldValue::Cell("authored synthetic field".into()))
    );
    assert_eq!(
        host.executed_item(before).unwrap_err().failure,
        Failure::WrongFamily
    );
    let count = host.inner().handles();
    assert!(host.native("DestroyTimer", &[Value::Handle(item)]).is_err());
    assert_eq!(
        host.create_item(&args(b"ITEM")).unwrap_err().failure,
        Failure::Budget("item creation")
    );
    assert_eq!(host.inner().handles(), count);
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    host.native("DestroyTimer", &[Value::Handle(before)])
        .unwrap();
    host.native("DestroyTimer", &[Value::Handle(after)])
        .unwrap();
    assert_eq!(host.live_timers(), 0);
    assert!(host.executed_item(item).is_ok());
}

#[test]
fn catalog_failures_do_not_allocate_or_consume_creation_budget() {
    let hits = Arc::new(AtomicUsize::new(0));
    let mut host = host(COMMON, 8)
        .with_item_catalog(Arc::new(Catalog(hits)), source(), budget())
        .unwrap();
    let before = host.inner().handles();
    for rawcode in [b"UNIT", b"MISM", b"MISS"] {
        assert!(
            matches!(host.create_item(&args(rawcode)).unwrap_err().failure, Failure::ItemCatalog { requested, .. } if requested == *rawcode)
        );
        assert_eq!(host.inner().handles(), before);
    }
    assert!(host.native(CREATE, &args(b"ITEM")).is_ok());
}

#[test]
fn invalid_arguments_and_capacity_refuse_before_catalog_resolution() {
    for (max_handles, budget, arguments) in [
        (8, budget(), vec![]),
        (
            8,
            budget(),
            vec![Value::Int(0), Value::Real(f32::NAN), Value::Real(0.0)],
        ),
        (
            8,
            budget(),
            vec![Value::Int(0), Value::Real(0.0), Value::Real(f32::INFINITY)],
        ),
        (
            8,
            ItemBudget {
                max_records: 0,
                ..budget()
            },
            args(b"ITEM").to_vec(),
        ),
        (1, budget(), args(b"ITEM").to_vec()),
    ] {
        let hits = Arc::new(AtomicUsize::new(0));
        let mut host = host(COMMON, max_handles)
            .with_item_catalog(Arc::new(Catalog(Arc::clone(&hits))), source(), budget)
            .unwrap();
        if max_handles == 1 {
            host.native("CreateTimer", &[]).unwrap();
        }
        let count = host.inner().handles();
        assert!(host.native(CREATE, &arguments).is_err());
        assert_eq!(host.inner().handles(), count);
        assert_eq!(hits.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn declaration_source_layout_and_retained_fields_are_checked() {
    for common in [
        COMMON.replace("returns item", "returns timer"),
        COMMON.replace("item extends widget", "item extends handle"),
        COMMON.replace("native CreateItem", "constant native CreateItem"),
        COMMON.replace("integer typeId", "real typeId"),
    ] {
        let result = host(&common, 8).with_item_catalog(
            Arc::new(Catalog(Arc::new(AtomicUsize::new(0)))),
            source(),
            budget(),
        );
        assert_eq!(result.err().unwrap().failure, Failure::Signature);
    }
    let catalog: Arc<dyn ObjectCatalog + Send + Sync> =
        Arc::new(Catalog(Arc::new(AtomicUsize::new(0))));
    let mut missing = source();
    missing.map.clear();
    assert!(matches!(
        host(COMMON, 8)
            .with_item_catalog(Arc::clone(&catalog), missing, budget())
            .err()
            .unwrap()
            .failure,
        Failure::ItemState(_)
    ));
    assert_eq!(
        host(COMMON, 8)
            .with_item_catalog(
                Arc::clone(&catalog),
                source(),
                ItemBudget {
                    max_records: usize::MAX,
                    ..budget()
                }
            )
            .err()
            .unwrap()
            .failure,
        Failure::Capacity
    );
    let mut host = host(COMMON, 8)
        .with_item_catalog(
            Arc::clone(&catalog),
            source(),
            ItemBudget {
                max_retained_fields: 0,
                ..budget()
            },
        )
        .unwrap();
    let count = host.inner().handles();
    assert_eq!(
        host.create_item(&args(b"ITEM")).unwrap_err().failure,
        Failure::Budget("item retained fields")
    );
    assert_eq!(host.inner().handles(), count);
    assert!(matches!(
        host.with_item_catalog(catalog, source(), budget())
            .err()
            .unwrap()
            .failure,
        Failure::ItemState("already admitted")
    ));
}

#[test]
fn modification_retention_is_bounded_without_spending_a_creation_record() {
    let catalog: Arc<dyn ObjectCatalog + Send + Sync> =
        Arc::new(Catalog(Arc::new(AtomicUsize::new(0))));
    let mut limited = host(COMMON, 8)
        .with_item_catalog(Arc::clone(&catalog), source(), budget())
        .unwrap();
    let count = limited.inner().handles();
    assert_eq!(
        limited.create_item(&args(b"MODS")).unwrap_err().failure,
        Failure::Budget("item modifications")
    );
    assert_eq!(limited.inner().handles(), count);
    assert!(limited.create_item(&args(b"ITEM")).is_ok());
    let mut admitted = host(COMMON, 8)
        .with_item_catalog(
            Arc::clone(&catalog),
            source(),
            ItemBudget {
                max_modifications: 1,
                ..budget()
            },
        )
        .unwrap();
    let item = handle(admitted.create_item(&args(b"MODS")).unwrap());
    assert_eq!(
        admitted.executed_item(item).unwrap().definition,
        catalog.resolve(*b"MODS").unwrap()
    );
}

#[test]
fn visibility_contracts_and_precreation_admission_are_checked() {
    let good = "native SetItemVisible takes item i, boolean show returns nothing\nnative IsItemVisible takes item i returns boolean\n";
    for declarations in [
        good.replace("boolean show", "integer show"),
        good.replace("native IsItemVisible", "constant native IsItemVisible"),
        good.replace("returns boolean", "returns integer"),
    ] {
        let h = host(&format!("{COMMON}{declarations}"), 8)
            .with_item_catalog(
                Arc::new(Catalog(Arc::new(AtomicUsize::new(0)))),
                source(),
                budget(),
            )
            .unwrap();
        assert!(
            h.with_item_visibility(crate::item_visibility::Rules::fixture())
                .is_err()
        );
    }
    let mut h = host(&format!("{COMMON}{good}"), 8)
        .with_item_catalog(
            Arc::new(Catalog(Arc::new(AtomicUsize::new(0)))),
            source(),
            budget(),
        )
        .unwrap();
    let item = h.native(CREATE, &args(b"ITEM")).unwrap();
    assert_eq!(h.executed_item(handle(item)).unwrap().visible(), None);
    assert!(
        h.with_item_visibility(crate::item_visibility::Rules::fixture())
            .is_err()
    );
    let mut h = host(&format!("{COMMON}{good}"), 8)
        .with_item_catalog(
            Arc::new(Catalog(Arc::new(AtomicUsize::new(0)))),
            source(),
            budget(),
        )
        .unwrap()
        .with_item_visibility(crate::item_visibility::Rules::fixture())
        .unwrap();
    let item = h.native(CREATE, &args(b"ITEM")).unwrap();
    let timer = h.native("CreateTimer", &[]).unwrap();
    assert!(
        h.native("SetItemVisible", &[timer, Value::Bool(false)])
            .is_err()
    );
    assert!(
        h.native("SetItemVisible", &[item.clone(), Value::Int(0)])
            .is_err()
    );
    assert_eq!(
        h.native("IsItemVisible", &[item]).unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn set_item_position_moves_world_items_and_ignores_null_and_removed_ones() {
    let common =
        format!("{COMMON}native SetItemPosition takes item i, real x, real y returns nothing\n");
    let hits = Arc::new(AtomicUsize::new(0));
    let catalog: Arc<dyn ObjectCatalog + Send + Sync> = Arc::new(Catalog(hits));
    let mut host = host(&common, 8)
        .with_item_catalog(catalog, source(), budget())
        .unwrap();
    let item = handle(host.native(CREATE, &args(b"ITEM")).unwrap());
    let moved = |host: &mut ObjectHost, target: Value, x: f32, y: f32| {
        host.native("SetItemPosition", &[target, Value::Real(x), Value::Real(y)])
    };
    moved(&mut host, Value::Handle(item), 10.5, -7.25).unwrap();
    let record = host.executed_item(item).unwrap();
    assert_eq!((record.x, record.y), (10.5, -7.25));
    assert_eq!(
        moved(&mut host, Value::Null, 1.0, 2.0).unwrap(),
        Value::Null
    );
    let timer = host.native("CreateTimer", &[]).unwrap();
    assert!(moved(&mut host, timer, 1.0, 2.0).is_err());
    assert!(moved(&mut host, Value::Handle(item), f32::NAN, 2.0).is_err());
    assert_eq!(host.executed_item(item).unwrap().x, 10.5);
}

fn visible_host() -> ObjectHost {
    let common = format!(
        "{COMMON}native SetItemVisible takes item i, boolean show returns nothing\nnative IsItemVisible takes item i returns boolean\nnative SetItemPosition takes item i, real x, real y returns nothing\n"
    );
    host(&common, 16)
        .with_item_catalog(
            Arc::new(Catalog(Arc::new(AtomicUsize::new(0)))),
            source(),
            ItemBudget {
                max_records: 8,
                ..budget()
            },
        )
        .unwrap()
        .with_item_visibility(crate::item_visibility::Rules::fixture())
        .unwrap()
}

#[test]
fn carried_ground_and_consumed_visibility_truth_table() {
    let mut h = visible_host();
    let owner = handle(h.native("CreateTimer", &[]).unwrap());
    let stranger = handle(h.native("CreateTimer", &[]).unwrap());
    let ground = handle(h.native(CREATE, &args(b"ITEM")).unwrap());
    let base = h.executed_item(ground).unwrap().clone();
    assert_eq!(base.visible(), Some(true));
    let probe = |shown: Option<bool>, held_by: Option<(HandleId, usize)>, consumed: bool| {
        let mut record = base.clone();
        record.shown = shown;
        record.held_by = held_by;
        record.consumed = consumed;
        record.visible()
    };
    assert_eq!(probe(Some(true), None, false), Some(true));
    assert_eq!(probe(Some(false), None, false), Some(false));
    assert_eq!(probe(Some(true), Some((owner, 0)), false), Some(false));
    assert_eq!(probe(Some(true), Some((stranger, 3)), false), Some(false));
    assert_eq!(probe(Some(false), Some((owner, 0)), false), Some(false));
    assert_eq!(probe(Some(true), None, true), Some(false));
    assert_eq!(probe(Some(true), Some((owner, 0)), true), Some(false));
    let mut bare = host(COMMON, 8)
        .with_item_catalog(
            Arc::new(Catalog(Arc::new(AtomicUsize::new(0)))),
            source(),
            budget(),
        )
        .unwrap();
    let pending = handle(bare.native(CREATE, &args(b"ITEM")).unwrap());
    assert_eq!(bare.executed_item(pending).unwrap().visible(), None);
}

#[test]
fn carried_items_ignore_show_hide_and_refuse_reposition() {
    let mut h = visible_host();
    let holder = handle(h.native("CreateTimer", &[]).unwrap());
    let carried = h
        .create_item_record(i32::from_be_bytes(*b"ITEM"), 1.0, 2.0, Some((holder, 0)))
        .unwrap();
    let ground = handle(h.native(CREATE, &args(b"ITEM")).unwrap());
    assert_eq!(
        h.native("IsItemVisible", &[Value::Handle(carried)])
            .unwrap(),
        Value::Bool(false)
    );
    assert_eq!(
        h.native("IsItemVisible", &[Value::Handle(ground)]).unwrap(),
        Value::Bool(true)
    );
    h.native(
        "SetItemVisible",
        &[Value::Handle(carried), Value::Bool(false)],
    )
    .unwrap();
    assert_eq!(h.executed_item(carried).unwrap().shown, Some(true));
    assert_eq!(
        h.native("IsItemVisible", &[Value::Handle(carried)])
            .unwrap(),
        Value::Bool(false)
    );
    h.native(
        "SetItemVisible",
        &[Value::Handle(ground), Value::Bool(false)],
    )
    .unwrap();
    assert_eq!(
        h.native("IsItemVisible", &[Value::Handle(ground)]).unwrap(),
        Value::Bool(false)
    );
    h.native(
        "SetItemVisible",
        &[Value::Handle(ground), Value::Bool(true)],
    )
    .unwrap();
    assert_eq!(
        h.native("IsItemVisible", &[Value::Handle(ground)]).unwrap(),
        Value::Bool(true)
    );
    assert!(
        h.native(
            "SetItemPosition",
            &[Value::Handle(carried), Value::Real(5.0), Value::Real(6.0)]
        )
        .is_err()
    );
    let record = h.executed_item(carried).unwrap();
    assert_eq!((record.x, record.y), (1.0, 2.0));
}

#[test]
fn consumed_items_read_as_absent_while_retaining_their_record() {
    let mut h = visible_host();
    let item = handle(h.native(CREATE, &args(b"ITEM")).unwrap());
    let Entry::Item { record } = h.inner.entry_mut(item).unwrap() else {
        panic!("item entry")
    };
    record.consumed = true;
    assert!(h.executed_items().next().is_none());
    assert!(h.executed_item(item).unwrap().consumed);
    assert_eq!(
        h.native("IsItemVisible", &[Value::Handle(item)]).unwrap(),
        Value::Bool(false)
    );
    h.native("SetItemVisible", &[Value::Handle(item), Value::Bool(true)])
        .unwrap();
    assert_eq!(h.executed_item(item).unwrap().shown, Some(true));
    assert_eq!(
        h.native(
            "SetItemPosition",
            &[Value::Handle(item), Value::Real(9.0), Value::Real(9.0)]
        )
        .unwrap(),
        Value::Null
    );
    assert_eq!(h.executed_item(item).unwrap().x, -4.0);
}

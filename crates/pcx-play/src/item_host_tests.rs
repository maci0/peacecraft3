use super::*;
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

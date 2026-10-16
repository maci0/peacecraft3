use super::{COMMON, budget, context, object, slots_rule};
use crate::playercolor_host::{Failure, UnitBudget};
use pcx_objects::editor::ObjectKind;
use pcx_script::{Host, Value};
use pcx_sim::world::{
    Definition, FieldValue, ObjectCatalog, ObjectField, ResolutionProblem, Source,
};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

const PLACEMENT_OWNER: i16 = 7;

struct Counting {
    hits: Arc<AtomicUsize>,
    seen: Arc<Mutex<Vec<[u8; 4]>>>,
    unit: [u8; 4],
    item: [u8; 4],
    mismatch: [u8; 4],
    high: [u8; 4],
    high_base: String,
}

fn definition(
    rawcode: [u8; 4],
    base_rawcode: String,
    kind: ObjectKind,
    fields: BTreeMap<[u8; 4], ObjectField>,
) -> Definition {
    Definition {
        rawcode,
        base_rawcode,
        kind,
        fields,
        modifications: Vec::new(),
    }
}

fn retained_field() -> BTreeMap<[u8; 4], ObjectField> {
    let mut fields = BTreeMap::new();
    fields.insert(
        *b"fxid",
        ObjectField {
            metadata: None,
            value: Ok(FieldValue::Cell("fixture-cell".into())),
        },
    );
    fields
}

impl ObjectCatalog for Counting {
    fn resolve(&self, rawcode: [u8; 4]) -> Result<Definition, ResolutionProblem> {
        self.hits.fetch_add(1, Ordering::SeqCst);
        self.seen.lock().expect("seen").push(rawcode);
        if rawcode == self.unit {
            Ok(definition(
                rawcode,
                pcx_objects::fourcc_text(rawcode),
                ObjectKind::Unit,
                retained_field(),
            ))
        } else if rawcode == self.item {
            Ok(definition(
                rawcode,
                pcx_objects::fourcc_text(rawcode),
                ObjectKind::Item,
                BTreeMap::new(),
            ))
        } else if rawcode == self.mismatch {
            Ok(definition(
                *b"nope",
                pcx_objects::fourcc_text(*b"nope"),
                ObjectKind::Unit,
                BTreeMap::new(),
            ))
        } else if rawcode == self.high {
            Ok(definition(
                rawcode,
                self.high_base.clone(),
                ObjectKind::Unit,
                BTreeMap::new(),
            ))
        } else {
            Err(ResolutionProblem::MissingRawcode(pcx_objects::fourcc_text(
                rawcode,
            )))
        }
    }
}

fn script_common(unit_parent: &str, declare_create: bool) -> String {
    let globals = COMMON.find("\nglobals\n").expect("globals");
    let mut common = COMMON.to_string();
    let native = if declare_create {
        "native CreateUnit takes player id, integer unitid, real x, real y, real face returns unit\nnative SetUnitAcquireRange takes unit whichUnit, real newAcquireRange returns nothing\n"
    } else {
        ""
    };
    common.insert_str(
        globals + 1,
        &format!("type widget extends agent\ntype unit extends {unit_parent}\n{native}"),
    );
    common
}

fn source() -> Source {
    Source {
        install: "selected-install".into(),
        edition: "selected-edition".into(),
        map: "unit-catalog".into(),
    }
}

fn limits(max_records: usize) -> UnitBudget {
    UnitBudget {
        max_records,
        max_retained_fields: 4,
        max_modifications: 0,
    }
}

type SeenRawcodes = Arc<Mutex<Vec<[u8; 4]>>>;

fn counting() -> (Arc<Counting>, Arc<AtomicUsize>, SeenRawcodes) {
    let hits = Arc::new(AtomicUsize::new(0));
    let seen = Arc::new(Mutex::new(Vec::new()));
    let high = i32::from_be_bytes([0xff, 0x80, 0x7f, 0x01]).to_be_bytes();
    let catalog = Arc::new(Counting {
        hits: Arc::clone(&hits),
        seen: Arc::clone(&seen),
        unit: *b"hfoo",
        item: *b"item",
        mismatch: *b"bad!",
        high,
        high_base: "\\xff\\x80\\x7f\\x01".into(),
    });
    (catalog, hits, seen)
}

struct VmHost {
    vm: pcx_script::OwnedVm<
        crate::scripts::OwnedLocalizedHost<crate::playercolor_host::ObjectHost>,
    >,
    hits: Arc<AtomicUsize>,
    seen: Arc<Mutex<Vec<[u8; 4]>>>,
    catalog: Arc<dyn ObjectCatalog + Send + Sync>,
}

fn open(max_records: usize, max_handles: usize, map: &str) -> VmHost {
    open_with(max_records, max_handles, map, "widget", true)
}

fn open_with(
    max_records: usize,
    max_handles: usize,
    map: &str,
    unit_parent: &str,
    declare_create: bool,
) -> VmHost {
    let common = script_common(unit_parent, declare_create);
    let ctx = context(300, &common, map, "unit-catalog");
    assert!(ctx.world.records.is_empty());
    let (catalog, hits, seen) = counting();
    let shared: Arc<dyn ObjectCatalog + Send + Sync> = catalog;
    let host = object(&ctx, max_handles)
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_unit_catalog(Arc::clone(&shared), source(), limits(max_records))
        .unwrap();
    assert!(Arc::ptr_eq(&host.unit_catalog().unwrap(), &shared));
    let (declared, span) = host.unit_declaration().unwrap();
    assert_eq!(declared, "synthetic/common.j");
    assert!(common[span.start as usize..span.end as usize].contains("CreateUnit"));
    VmHost {
        vm: Arc::clone(&ctx.scripts).owned_vm(host),
        hits,
        seen,
        catalog: shared,
    }
}

const MAP: &str = "globals\nplayer P = null\nunit U = null\ntimer T = null\nendglobals\nfunction main takes nothing returns nothing\nset P = Player(4)\nset U = CreateUnit(P, 'hfoo', 1.5, -2.25, 90.0)\nendfunction\nfunction missing takes nothing returns unit\nreturn CreateUnit(P, 'zzzz', 0.0, 0.0, 0.0)\nendfunction\nfunction item takes nothing returns unit\nreturn CreateUnit(P, 'item', 0.0, 0.0, 0.0)\nendfunction\nfunction mismatched takes nothing returns unit\nreturn CreateUnit(P, 'bad!', 0.0, 0.0, 0.0)\nendfunction\nfunction wrong takes nothing returns unit\nset T = CreateTimer()\nreturn CreateUnit(T, 'hfoo', 0.0, 0.0, 0.0)\nendfunction\nfunction again takes nothing returns unit\nreturn CreateUnit(P, 'hfoo', 0.0, 0.0, 0.0)\nendfunction\nfunction zero takes nothing returns nothing\ncall SetUnitAcquireRange(U, 0.0)\nendfunction\nfunction negative takes nothing returns nothing\ncall SetUnitAcquireRange(U, -12.5)\nendfunction\nfunction wrong_range takes nothing returns nothing\nset T = CreateTimer()\ncall SetUnitAcquireRange(T, 4.0)\nendfunction\n";

#[test]
fn owned_vm_retains_catalog_result_and_typed_resolve_failures() {
    let mut session = open(4, 16, MAP);
    session.vm.init_globals().unwrap();
    session.vm.call("main", &[]).unwrap();
    assert_eq!(session.hits.load(Ordering::SeqCst), 1);
    let Value::Handle(player) = session.vm.global("P") else {
        panic!("player");
    };
    let Value::Handle(unit) = session.vm.global("U") else {
        panic!("unit");
    };
    assert_eq!(session.vm.host().inner.inner().kind(unit).unwrap(), "unit");
    assert_eq!(
        session
            .vm
            .host()
            .native("GetPlayerId", &[Value::Handle(player)])
            .unwrap(),
        Value::Int(4)
    );
    let executed = session.vm.host().inner.executed_unit(unit).unwrap().clone();
    assert_eq!(executed.definition.rawcode, *b"hfoo");
    assert_eq!(executed.definition.base_rawcode, "hfoo");
    assert_eq!(executed.executed_owner, 4);
    assert_ne!(executed.executed_owner, i32::from(PLACEMENT_OWNER));
    assert_eq!(executed.source, source());
    assert_eq!(
        executed
            .definition
            .fields
            .get(b"fxid")
            .and_then(|field| field.value.as_ref().ok()),
        Some(&FieldValue::Cell("fixture-cell".into()))
    );
    assert!(Arc::ptr_eq(
        &session.vm.host().inner.unit_catalog().unwrap(),
        &session.catalog
    ));
    let handles = session.vm.host().inner.inner().handles();
    for (name, needle) in [
        (
            "missing",
            "unit catalog [122, 122, 122, 122]: MissingRawcode(\"zzzz\")",
        ),
        ("item", "WrongFamily { expected: Unit, actual: Item }"),
        (
            "mismatched",
            "RawcodeMismatch { requested: [98, 97, 100, 33], resolved: [110, 111, 112, 101] }",
        ),
    ] {
        let error = session.vm.call(name, &[]).unwrap_err().to_string();
        assert!(error.contains(needle), "{name}: {error}");
        assert!(error.contains("synthetic/common.j"), "{name}: {error}");
        assert!(error.contains("CreateUnit"), "{name}: {error}");
    }
    assert_eq!(session.vm.host().inner.inner().handles(), handles);
    assert_eq!(session.hits.load(Ordering::SeqCst), 4);
}

#[test]
fn zero_and_full_record_budgets_do_not_resolve() {
    let mut empty = open(0, 8, MAP);
    empty.vm.init_globals().unwrap();
    let error = empty.vm.call("main", &[]).unwrap_err().to_string();
    assert!(error.contains("unit creation policy exceeded"), "{error}");
    assert!(error.contains("synthetic/common.j"), "{error}");
    assert_eq!(empty.hits.load(Ordering::SeqCst), 0);
    assert!(empty.seen.lock().expect("seen").is_empty());

    let mut full = open(1, 8, MAP);
    full.vm.init_globals().unwrap();
    full.vm.call("main", &[]).unwrap();
    assert_eq!(full.hits.load(Ordering::SeqCst), 1);
    let error = full.vm.call("again", &[]).unwrap_err().to_string();
    assert!(error.contains("unit creation policy exceeded"), "{error}");
    assert_eq!(full.hits.load(Ordering::SeqCst), 1);
    assert_eq!(full.seen.lock().expect("seen").as_slice(), &[*b"hfoo"]);
}

#[test]
fn high_byte_integer_resolves_unchanged_and_keeps_catalog_base_text() {
    let mut session = open(2, 8, MAP);
    session.vm.init_globals().unwrap();
    session.vm.call("main", &[]).unwrap();
    let Value::Handle(player) = session.vm.global("P") else {
        panic!("player");
    };
    let id = i32::from_be_bytes([0xff, 0x80, 0x7f, 0x01]);
    let before = session.hits.load(Ordering::SeqCst);
    let created = session
        .vm
        .host()
        .native(
            "CreateUnit",
            &[
                Value::Handle(player),
                Value::Int(id),
                Value::Real(3.0),
                Value::Real(4.0),
                Value::Real(5.0),
            ],
        )
        .unwrap();
    let Value::Handle(unit) = created else {
        panic!("high-byte unit");
    };
    assert_eq!(session.hits.load(Ordering::SeqCst), before + 1);
    assert_eq!(
        session.seen.lock().expect("seen").last().copied(),
        Some(id.to_be_bytes())
    );
    let executed = session.vm.host().inner.executed_unit(unit).unwrap();
    assert_eq!(executed.definition.rawcode, id.to_be_bytes());
    assert_eq!(executed.definition.base_rawcode, "\\xff\\x80\\x7f\\x01");
    assert_ne!(
        executed.definition.base_rawcode,
        pcx_objects::fourcc_text(id.to_be_bytes())
    );
}

#[test]
fn wrong_player_family_does_not_resolve() {
    let mut session = open(2, 8, MAP);
    session.vm.init_globals().unwrap();
    session.vm.call("main", &[]).unwrap();
    let hits = session.hits.load(Ordering::SeqCst);
    let error = session.vm.call("wrong", &[]).unwrap_err().to_string();
    assert!(
        error.contains("handle identifies another family"),
        "{error}"
    );
    assert!(error.contains("synthetic/common.j"), "{error}");
    assert_eq!(session.hits.load(Ordering::SeqCst), hits);
}

#[test]
fn non_finite_coordinate_does_not_resolve() {
    let mut session = open(2, 8, MAP);
    session.vm.init_globals().unwrap();
    session.vm.call("main", &[]).unwrap();
    let Value::Handle(player) = session.vm.global("P") else {
        panic!("player");
    };
    let hits = session.hits.load(Ordering::SeqCst);
    let error = session
        .vm
        .host()
        .native(
            "CreateUnit",
            &[
                Value::Handle(player),
                Value::Int(i32::from_be_bytes(*b"hfoo")),
                Value::Real(f32::INFINITY),
                Value::Real(0.0),
                Value::Real(0.0),
            ],
        )
        .unwrap_err();
    assert!(
        error.contains("recreation policy: finite real required"),
        "{error}"
    );
    assert!(error.contains("synthetic/common.j"), "{error}");
    assert_eq!(session.hits.load(Ordering::SeqCst), hits);
}

#[test]
fn arena_capacity_does_not_resolve() {
    let mut session = open(4, 5, MAP);
    session.vm.init_globals().unwrap();
    let error = session.vm.call("main", &[]).unwrap_err().to_string();
    assert!(
        error.contains("host identity capacity or allocation exceeded"),
        "{error}"
    );
    assert!(error.contains("synthetic/common.j"), "{error}");
    assert_eq!(session.hits.load(Ordering::SeqCst), 0);
    assert!(session.seen.lock().expect("seen").is_empty());
}

#[test]
fn readmission_keeps_the_declaration_on_the_error() {
    let common = script_common("widget", true);
    let ctx = context(300, &common, MAP, "unit-catalog");
    let (catalog, hits, _) = counting();
    let shared: Arc<dyn ObjectCatalog + Send + Sync> = catalog;
    let host = object(&ctx, 8)
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_unit_catalog(Arc::clone(&shared), source(), limits(1))
        .unwrap();
    let error = match host.with_unit_catalog(shared, source(), limits(1)) {
        Err(error) => error,
        Ok(_) => panic!("readmission was accepted"),
    };
    assert_eq!(error.failure, Failure::UnitState("already admitted"));
    assert_eq!(error.native, "CreateUnit");
    assert_eq!(error.declaration_source, "synthetic/common.j");
    assert_ne!(error.declaration_span, pcx_script::span::Span::EMPTY);
    assert_eq!(hits.load(Ordering::SeqCst), 0);
}

#[test]
fn signature_and_ancestry_fail_before_a_catalog_call() {
    let (catalog, hits, _) = counting();
    let shared: Arc<dyn ObjectCatalog + Send + Sync> = catalog;
    let missing = script_common("widget", false);
    let ctx = context(
        300,
        &missing,
        "function main takes nothing returns nothing\nendfunction\n",
        "unit-catalog",
    );
    let error = match object(&ctx, 4)
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_unit_catalog(Arc::clone(&shared), source(), limits(1))
    {
        Err(error) => error,
        Ok(_) => panic!("missing CreateUnit was admitted"),
    };
    assert_eq!(error.failure, Failure::Signature);
    assert_eq!(error.native, "CreateUnit");
    let ancestry = script_common("agent", true);
    let ctx = context(300, &ancestry, MAP, "unit-catalog");
    let error = match object(&ctx, 4)
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_unit_catalog(shared, source(), limits(1))
    {
        Err(error) => error,
        Ok(_) => panic!("unit ancestry was admitted"),
    };
    assert_eq!(error.failure, Failure::Signature);
    assert_eq!(error.declaration_source, "synthetic/common.j");
    assert_ne!(error.declaration_span, pcx_script::span::Span::EMPTY);
    assert_eq!(hits.load(Ordering::SeqCst), 0);
}

#[test]
fn unadmitted_create_unit_is_unsupported_on_the_owned_vm_host() {
    let common = script_common("widget", true);
    let ctx = context(
        300,
        &common,
        "function main takes nothing returns nothing\nendfunction\n",
        "unit-catalog",
    );
    let host = object(&ctx, 8)
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap();
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host);
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    let error = vm.host().native("CreateUnit", &[]).unwrap_err();
    assert!(error.contains("unsupported"), "{error}");
}

#[test]
fn owned_vm_set_acquire_range_keeps_exact_finite_real_and_prior_state() {
    let mut session = open(4, 16, MAP);
    session.vm.init_globals().unwrap();
    session.vm.call("main", &[]).unwrap();
    let Value::Handle(unit) = session.vm.global("U") else {
        panic!("unit");
    };
    let rawcode = session
        .vm
        .host()
        .inner
        .executed_unit(unit)
        .unwrap()
        .definition
        .rawcode;
    assert_eq!(
        session
            .vm
            .host()
            .inner
            .executed_unit(unit)
            .unwrap()
            .acquire_range,
        None
    );
    let hits = session.hits.load(Ordering::SeqCst);
    session.vm.call("zero", &[]).unwrap();
    assert_eq!(
        session
            .vm
            .host()
            .inner
            .executed_unit(unit)
            .unwrap()
            .acquire_range,
        Some(0.0)
    );
    assert_ne!(
        session
            .vm
            .host()
            .inner
            .executed_unit(unit)
            .unwrap()
            .acquire_range,
        None
    );
    let family = session.vm.call("wrong_range", &[]).unwrap_err().to_string();
    assert!(
        family.contains("handle identifies another family"),
        "{family}"
    );
    assert!(family.contains("synthetic/common.j"), "{family}");
    assert_eq!(
        session
            .vm
            .host()
            .inner
            .executed_unit(unit)
            .unwrap()
            .acquire_range,
        Some(0.0)
    );
    session.vm.call("negative", &[]).unwrap();
    assert_eq!(
        session
            .vm
            .host()
            .inner
            .executed_unit(unit)
            .unwrap()
            .acquire_range,
        Some(-12.5)
    );
    let Value::Handle(player) = session.vm.global("P") else {
        panic!("player");
    };
    let non_finite = session
        .vm
        .host()
        .native(
            "SetUnitAcquireRange",
            &[Value::Handle(unit), Value::Real(f32::NAN)],
        )
        .unwrap_err();
    assert!(
        non_finite.contains("recreation policy: finite real required"),
        "{non_finite}"
    );
    assert!(non_finite.contains("synthetic/common.j"), "{non_finite}");
    let unknown = session
        .vm
        .host()
        .native(
            "SetUnitAcquireRange",
            &[Value::Handle(player), Value::Real(8.0)],
        )
        .unwrap_err();
    assert!(
        unknown.contains("handle identifies another family"),
        "{unknown}"
    );
    let missing = session
        .vm
        .host()
        .native(
            "SetUnitAcquireRange",
            &[
                Value::Handle(pcx_script::HandleId::new(
                    std::num::NonZeroU64::new(9_999).unwrap(),
                )),
                Value::Real(8.0),
            ],
        )
        .unwrap_err();
    assert!(missing.contains("unknown handle identity"), "{missing}");
    assert_eq!(
        session
            .vm
            .host()
            .inner
            .executed_unit(unit)
            .unwrap()
            .acquire_range,
        Some(-12.5)
    );
    assert_eq!(
        session
            .vm
            .host()
            .inner
            .executed_unit(unit)
            .unwrap()
            .definition
            .rawcode,
        rawcode
    );
    assert_eq!(session.hits.load(Ordering::SeqCst), hits);
    let getter = session
        .vm
        .host()
        .native("GetUnitAcquireRange", &[Value::Handle(unit)])
        .unwrap_err();
    assert!(getter.contains("unsupported"), "{getter}");
}

#[test]
fn acquire_range_bad_signature_does_not_admit_or_resolve() {
    let common = script_common("widget", true).replace(
        "native SetUnitAcquireRange takes unit whichUnit, real newAcquireRange returns nothing\n",
        "native SetUnitAcquireRange takes integer whichUnit returns nothing\n",
    );
    let ctx = context(
        300,
        &common,
        "function main takes nothing returns nothing\nendfunction\n",
        "unit-catalog",
    );
    let (catalog, hits, _) = counting();
    let shared: Arc<dyn ObjectCatalog + Send + Sync> = catalog;
    let error = match object(&ctx, 4)
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_unit_catalog(shared, source(), limits(1))
    {
        Err(error) => error,
        Ok(_) => panic!("bad acquire-range signature was admitted"),
    };
    assert_eq!(error.failure, Failure::Signature);
    assert_eq!(error.native, "SetUnitAcquireRange");
    assert_eq!(error.declaration_source, "synthetic/common.j");
    assert_ne!(error.declaration_span, pcx_script::span::Span::EMPTY);
    assert_eq!(hits.load(Ordering::SeqCst), 0);
}

#[test]
fn acquire_range_from_another_source_is_refused() {
    let primary = "type agent extends handle\ntype widget extends agent\ntype unit extends widget\ntype player extends agent\ntype timer extends agent\ntype playercolor extends handle\nconstant native ConvertPlayerColor takes integer i returns playercolor\nnative CreateTimer takes nothing returns timer\nnative DestroyTimer takes timer t returns nothing\nnative CreateUnit takes player id, integer unitid, real x, real y, real face returns unit\nglobals\nconstant playercolor C = ConvertPlayerColor(1)\nendglobals\n";
    let other =
        "native SetUnitAcquireRange takes unit whichUnit, real newAcquireRange returns nothing\n";
    let map = pcx_script::parse_script("").unwrap();
    let primary = pcx_script::parse_script(primary).unwrap();
    let other = pcx_script::parse_script(other).unwrap();
    let program = Arc::new(
        pcx_script::resolve_with(
            &map,
            &[
                pcx_script::Prelude::new("synthetic/common.j", &primary),
                pcx_script::Prelude::new("synthetic/other.j", &other),
            ],
        )
        .unwrap(),
    );
    let inner = crate::playercolor_host::EnumHost::checked_selected(
        program,
        "split-source",
        &["ConvertPlayerColor"],
        crate::playercolor_host::Limits {
            max_handles: 4,
            admission_steps: 100,
            expression_depth: 16,
            expression_work: 100,
        },
    )
    .unwrap();
    let (catalog, hits, _) = counting();
    let shared: Arc<dyn ObjectCatalog + Send + Sync> = catalog;
    let error = match crate::playercolor_host::ObjectHost::checked(inner)
        .unwrap()
        .with_unit_catalog(shared, source(), limits(1))
    {
        Err(error) => error,
        Ok(_) => panic!("foreign acquire-range source was admitted"),
    };
    assert_eq!(error.failure, Failure::Source);
    assert_eq!(error.native, "SetUnitAcquireRange");
    assert_eq!(error.declaration_source, "synthetic/other.j");
    assert_ne!(error.declaration_span, pcx_script::span::Span::EMPTY);
    assert_eq!(hits.load(Ordering::SeqCst), 0);
}

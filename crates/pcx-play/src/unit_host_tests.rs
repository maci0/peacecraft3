use super::{COMMON, budget, context, object, slots_rule};
use crate::playercolor_host::{EnumHost, Failure, Limits, ObjectHost, UnitBudget, UnitStateBudget};
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
        "native CreateUnit takes player id, integer unitid, real x, real y, real face returns unit\nnative SetUnitAcquireRange takes unit whichUnit, real newAcquireRange returns nothing\nnative SetHeroLevel takes unit whichHero, integer level, boolean showEyeCandy returns nothing\n"
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

const MAP: &str = "globals\nplayer P = null\nunit U = null\ntimer T = null\nendglobals\nfunction main takes nothing returns nothing\nset P = Player(4)\nset U = CreateUnit(P, 'hfoo', 1.5, -2.25, 90.0)\nendfunction\nfunction missing takes nothing returns unit\nreturn CreateUnit(P, 'zzzz', 0.0, 0.0, 0.0)\nendfunction\nfunction item takes nothing returns unit\nreturn CreateUnit(P, 'item', 0.0, 0.0, 0.0)\nendfunction\nfunction mismatched takes nothing returns unit\nreturn CreateUnit(P, 'bad!', 0.0, 0.0, 0.0)\nendfunction\nfunction wrong takes nothing returns unit\nset T = CreateTimer()\nreturn CreateUnit(T, 'hfoo', 0.0, 0.0, 0.0)\nendfunction\nfunction again takes nothing returns unit\nreturn CreateUnit(P, 'hfoo', 0.0, 0.0, 0.0)\nendfunction\nfunction zero takes nothing returns nothing\ncall SetUnitAcquireRange(U, 0.0)\nendfunction\nfunction negative takes nothing returns nothing\ncall SetUnitAcquireRange(U, -12.5)\nendfunction\nfunction wrong_range takes nothing returns nothing\nset T = CreateTimer()\ncall SetUnitAcquireRange(T, 4.0)\nendfunction\nfunction level_zero takes nothing returns nothing\ncall SetHeroLevel(U, 0, false)\nendfunction\nfunction level_signed takes nothing returns nothing\ncall SetHeroLevel(U, -7, true)\nendfunction\nfunction level_foreign takes nothing returns nothing\nset T = CreateTimer()\ncall SetHeroLevel(T, 4, false)\nendfunction\n";

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

#[test]
fn owned_vm_set_hero_level_keeps_exact_request_on_the_admitted_unit() {
    let mut session = open(4, 16, MAP);
    session.vm.init_globals().unwrap();
    session.vm.call("main", &[]).unwrap();
    let Value::Handle(unit) = session.vm.global("U") else {
        panic!("unit");
    };
    let before = session.vm.host().inner.executed_unit(unit).unwrap().clone();
    assert_eq!(before.definition.rawcode, *b"hfoo");
    assert_eq!(before.requested_hero_level, None);
    assert_eq!(before.requested_show_eye_candy, None);
    assert_eq!(session.hits.load(Ordering::SeqCst), 1);
    session.vm.call("level_zero", &[]).unwrap();
    let zero = session.vm.host().inner.executed_unit(unit).unwrap().clone();
    assert_eq!(zero.requested_hero_level, Some(0));
    assert_eq!(zero.requested_show_eye_candy, Some(false));
    assert_ne!(zero.requested_hero_level, None);
    assert_eq!(zero.definition, before.definition);
    assert_eq!(zero.executed_owner, before.executed_owner);
    assert_eq!(zero.source, before.source);
    assert_eq!(zero.x, before.x);
    assert_eq!(zero.y, before.y);
    assert_eq!(zero.face, before.face);
    assert_eq!(zero.acquire_range, None);
    assert_eq!(session.hits.load(Ordering::SeqCst), 1);
    let foreign = session
        .vm
        .call("level_foreign", &[])
        .unwrap_err()
        .to_string();
    assert!(
        foreign.contains("handle identifies another family"),
        "{foreign}"
    );
    assert!(foreign.contains("synthetic/common.j"), "{foreign}");
    assert!(foreign.contains("SetHeroLevel"), "{foreign}");
    assert_eq!(
        session
            .vm
            .host()
            .inner
            .executed_unit(unit)
            .unwrap()
            .requested_hero_level,
        Some(0)
    );
    assert_eq!(
        session
            .vm
            .host()
            .inner
            .executed_unit(unit)
            .unwrap()
            .requested_show_eye_candy,
        Some(false)
    );
    let Value::Handle(player) = session.vm.global("P") else {
        panic!("player");
    };
    let player_target = session
        .vm
        .host()
        .native(
            "SetHeroLevel",
            &[Value::Handle(player), Value::Int(4), Value::Bool(false)],
        )
        .unwrap_err();
    assert!(
        player_target.contains("handle identifies another family"),
        "{player_target}"
    );
    let missing = session
        .vm
        .host()
        .native(
            "SetHeroLevel",
            &[
                Value::Handle(pcx_script::HandleId::new(
                    std::num::NonZeroU64::new(9_999).unwrap(),
                )),
                Value::Int(4),
                Value::Bool(true),
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
            .requested_hero_level,
        Some(0)
    );
    session.vm.call("level_signed", &[]).unwrap();
    let signed = session.vm.host().inner.executed_unit(unit).unwrap();
    assert_eq!(signed.requested_hero_level, Some(-7));
    assert_eq!(signed.requested_show_eye_candy, Some(true));
    assert_eq!(signed.definition.rawcode, *b"hfoo");
    assert_eq!(session.hits.load(Ordering::SeqCst), 1);
    let getter = session
        .vm
        .host()
        .native("GetHeroLevel", &[Value::Handle(unit)])
        .unwrap_err();
    assert!(getter.contains("unsupported"), "{getter}");
    assert_eq!(
        session
            .vm
            .host()
            .inner
            .executed_unit(unit)
            .unwrap()
            .requested_hero_level,
        Some(-7)
    );
}

#[test]
fn hero_level_bad_signature_does_not_admit_or_resolve() {
    let common = script_common("widget", true).replace(
        "native SetHeroLevel takes unit whichHero, integer level, boolean showEyeCandy returns nothing\n",
        "native SetHeroLevel takes integer level returns nothing\n",
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
        Ok(_) => panic!("bad hero-level signature was admitted"),
    };
    assert_eq!(error.failure, Failure::Signature);
    assert_eq!(error.native, "SetHeroLevel");
    assert_eq!(error.declaration_source, "synthetic/common.j");
    assert_ne!(error.declaration_span, pcx_script::span::Span::EMPTY);
    assert_eq!(hits.load(Ordering::SeqCst), 0);
}

#[test]
fn hero_level_from_another_source_is_refused() {
    let primary = "type agent extends handle\ntype widget extends agent\ntype unit extends widget\ntype player extends agent\ntype timer extends agent\ntype playercolor extends handle\nconstant native ConvertPlayerColor takes integer i returns playercolor\nnative CreateTimer takes nothing returns timer\nnative DestroyTimer takes timer t returns nothing\nnative CreateUnit takes player id, integer unitid, real x, real y, real face returns unit\nglobals\nconstant playercolor C = ConvertPlayerColor(1)\nendglobals\n";
    let other = "native SetHeroLevel takes unit whichHero, integer level, boolean showEyeCandy returns nothing\n";
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
        Ok(_) => panic!("foreign hero-level source was admitted"),
    };
    assert_eq!(error.failure, Failure::Source);
    assert_eq!(error.native, "SetHeroLevel");
    assert_eq!(error.declaration_source, "synthetic/other.j");
    assert_ne!(error.declaration_span, pcx_script::span::Span::EMPTY);
    assert_eq!(hits.load(Ordering::SeqCst), 0);
}

#[test]
fn unit_creation_still_admits_a_program_without_the_hero_setter() {
    let common = script_common("widget", true).replace(
        "native SetHeroLevel takes unit whichHero, integer level, boolean showEyeCandy returns nothing\n",
        "",
    );
    let ctx = context(
        300,
        &common,
        "function main takes nothing returns nothing\nendfunction\n",
        "unit-catalog",
    );
    let (catalog, hits, _) = counting();
    let shared: Arc<dyn ObjectCatalog + Send + Sync> = catalog;
    let mut host = object(&ctx, 4)
        .with_unit_catalog(shared, source(), limits(1))
        .unwrap();
    let error = host.native("SetHeroLevel", &[]).unwrap_err();
    assert!(error.contains("unsupported"), "{error}");
    assert_eq!(hits.load(Ordering::SeqCst), 0);
}

const STATE_CONVERSIONS: &[&str] = &[
    "ConvertPlacement",
    "ConvertPlayerColor",
    "ConvertRacePref",
    "ConvertMapControl",
    "ConvertUnitState",
];

const STATE_MAP: &str = "globals\nplayer P = null\nunit U = null\nunit V = null\ntimer T = null\nendglobals\nfunction main takes nothing returns nothing\nset P = Player(4)\nset U = CreateUnit(P, 'hfoo', 1.5, -2.25, 90.0)\nset V = CreateUnit(P, 'hfoo', 3.0, 4.0, 0.0)\nendfunction\nfunction zero takes nothing returns nothing\ncall SetUnitState(U, FIRST, 0)\nendfunction\nfunction finite takes nothing returns nothing\ncall SetUnitState(U, FIRST, -12.5)\nendfunction\nfunction other takes nothing returns nothing\ncall SetUnitState(U, SECOND, 4.5)\nendfunction\nfunction other_unit takes nothing returns nothing\ncall SetUnitState(V, FIRST, 1.0)\nendfunction\nfunction wrong takes nothing returns nothing\nset T = CreateTimer()\ncall SetUnitState(T, FIRST, 1.0)\nendfunction\n";

fn state_common(declare_state: bool) -> String {
    let globals = COMMON.find("\nglobals\n").expect("globals");
    let mut common = COMMON.to_string();
    let mut extra = String::from(
        "type widget extends agent\ntype unit extends widget\ntype unitstate extends handle\nnative CreateUnit takes player id, integer unitid, real x, real y, real face returns unit\nnative SetUnitAcquireRange takes unit whichUnit, real newAcquireRange returns nothing\nnative SetHeroLevel takes unit whichHero, integer level, boolean showEyeCandy returns nothing\n",
    );
    if declare_state {
        extra.push_str(
            "constant native ConvertUnitState takes integer i returns unitstate\nnative SetUnitState takes unit whichUnit, unitstate whichUnitState, real newVal returns nothing\nconstant native GetUnitState takes unit whichUnit, unitstate whichUnitState returns real\n",
        );
    }
    common.insert_str(globals + 1, &extra);
    if declare_state {
        common = common.replacen(
            "\nglobals\n",
            "\nglobals\nconstant unitstate FIRST = ConvertUnitState(0)\nconstant unitstate SECOND = ConvertUnitState(-4)\n",
            1,
        );
    }
    common
}

fn state_enums(ctx: &crate::map_admission::ParsedMapContext, names: &[&str]) -> EnumHost {
    EnumHost::checked_selected(
        ctx.scripts.shared_program(),
        &ctx.map_digest.label,
        names,
        Limits {
            max_handles: 32,
            admission_steps: 1000,
            expression_depth: 16,
            expression_work: 1000,
        },
    )
    .unwrap()
}

type StateSession = (
    pcx_script::OwnedVm<crate::scripts::OwnedLocalizedHost<ObjectHost>>,
    Arc<AtomicUsize>,
    Arc<dyn ObjectCatalog + Send + Sync>,
);

fn state_catalog_host(
    common: &str,
    map: &str,
    names: &[&str],
    max_state_records: usize,
) -> Result<StateSession, crate::playercolor_host::Error> {
    let ctx = context(300, common, map, "unit-catalog");
    let (catalog, hits, _) = counting();
    let shared: Arc<dyn ObjectCatalog + Send + Sync> = catalog;
    let host = ObjectHost::checked(state_enums(&ctx, names))
        .unwrap()
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_unit_catalog(Arc::clone(&shared), source(), limits(2))?
        .with_unit_states(UnitStateBudget {
            max_records: max_state_records,
        })?;
    assert!(Arc::ptr_eq(&host.unit_catalog().unwrap(), &shared));
    Ok((Arc::clone(&ctx.scripts).owned_vm(host), hits, shared))
}

fn assignment_bits(
    unit: &crate::playercolor_host::ExecutedUnit,
    state: pcx_script::HandleId,
) -> Option<u32> {
    unit.unit_states
        .iter()
        .find(|assignment| assignment.unitstate == state)
        .map(|assignment| assignment.value.to_bits())
}

#[test]
fn owned_vm_set_unit_state_retains_the_domain_handle_and_exact_finite_value() {
    let (mut vm, hits, _) =
        state_catalog_host(&state_common(true), STATE_MAP, STATE_CONVERSIONS, 1)
            .expect("unit state admission");
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    assert_eq!(hits.load(Ordering::SeqCst), 2);
    let Value::Handle(unit) = vm.global("U") else {
        panic!("unit");
    };
    let Value::Handle(other_unit) = vm.global("V") else {
        panic!("second unit");
    };
    let Value::Handle(first) = vm
        .host()
        .native("ConvertUnitState", &[Value::Int(0)])
        .unwrap()
    else {
        panic!("domain handle");
    };
    let Value::Handle(second) = vm
        .host()
        .native("ConvertUnitState", &[Value::Int(-4)])
        .unwrap()
    else {
        panic!("second domain handle");
    };
    assert_eq!(vm.global("FIRST"), Value::Handle(first));
    assert_eq!(
        vm.host()
            .inner
            .inner()
            .symbolic_value(first, "unitstate")
            .unwrap(),
        0
    );
    let created = vm.host().inner.executed_unit(unit).unwrap().clone();
    assert!(created.unit_states.is_empty());
    assert_eq!(created.unit_states.capacity(), 0);
    assert_eq!(created.acquire_range, None);
    assert_eq!(created.requested_hero_level, None);
    let outside = vm
        .host()
        .native("ConvertUnitState", &[Value::Int(1)])
        .unwrap_err();
    assert!(outside.contains("outside"), "{outside}");
    let ordinal = vm
        .host()
        .native(
            "SetUnitState",
            &[Value::Handle(unit), Value::Int(0), Value::Real(5.0)],
        )
        .unwrap_err();
    assert!(
        ordinal.contains("arguments do not match the unit native declaration"),
        "{ordinal}"
    );
    let unknown_state = vm
        .host()
        .native(
            "SetUnitState",
            &[
                Value::Handle(unit),
                Value::Handle(pcx_script::HandleId::new(
                    std::num::NonZeroU64::new(9_999).unwrap(),
                )),
                Value::Real(5.0),
            ],
        )
        .unwrap_err();
    assert!(
        unknown_state.contains("unknown handle identity"),
        "{unknown_state}"
    );
    let Value::Handle(player) = vm.global("P") else {
        panic!("player");
    };
    let wrong_state_family = vm
        .host()
        .native(
            "SetUnitState",
            &[
                Value::Handle(unit),
                Value::Handle(player),
                Value::Real(f32::NAN),
            ],
        )
        .unwrap_err();
    assert!(
        wrong_state_family.contains("handle identifies another family"),
        "{wrong_state_family}"
    );
    let untouched = vm.host().inner.executed_unit(unit).unwrap();
    assert!(untouched.unit_states.is_empty());
    assert_eq!(untouched.unit_states.capacity(), 0);
    assert_eq!(hits.load(Ordering::SeqCst), 2);

    vm.call("zero", &[]).unwrap();
    let zero = vm.host().inner.executed_unit(unit).unwrap().clone();
    assert_eq!(zero.unit_states.len(), 1);
    assert_eq!(assignment_bits(&zero, first), Some(0.0f32.to_bits()));
    assert_eq!(assignment_bits(&zero, second), None);
    assert_eq!(zero.definition, created.definition);
    assert_eq!(zero.x.to_bits(), created.x.to_bits());
    assert_eq!(zero.y.to_bits(), created.y.to_bits());
    assert_eq!(zero.face.to_bits(), created.face.to_bits());
    assert_eq!(zero.acquire_range, None);
    assert_eq!(zero.requested_hero_level, None);
    assert_eq!(zero.requested_show_eye_candy, None);
    assert!(
        vm.host()
            .inner
            .executed_unit(other_unit)
            .unwrap()
            .unit_states
            .is_empty()
    );
    let capacity = zero.unit_states.capacity();
    assert!(capacity >= 1);

    vm.call("finite", &[]).unwrap();
    let replaced = vm.host().inner.executed_unit(unit).unwrap().clone();
    assert_eq!(replaced.unit_states.len(), 1);
    assert_eq!(replaced.unit_states.capacity(), capacity);
    assert_eq!(
        assignment_bits(&replaced, first),
        Some((-12.5f32).to_bits())
    );
    assert_eq!(hits.load(Ordering::SeqCst), 2);

    let full = vm.call("other", &[]).unwrap_err().to_string();
    assert!(full.contains("unit state assignments"), "{full}");
    let other_full = vm.call("other_unit", &[]).unwrap_err().to_string();
    assert!(
        other_full.contains("unit state assignments"),
        "{other_full}"
    );
    let foreign = vm.call("wrong", &[]).unwrap_err().to_string();
    assert!(
        foreign.contains("handle identifies another family"),
        "{foreign}"
    );
    let non_finite = vm
        .host()
        .native(
            "SetUnitState",
            &[
                Value::Handle(unit),
                Value::Handle(first),
                Value::Real(f32::INFINITY),
            ],
        )
        .unwrap_err();
    assert!(non_finite.contains("finite real required"), "{non_finite}");
    let nan = vm
        .host()
        .native(
            "SetUnitState",
            &[
                Value::Handle(unit),
                Value::Handle(second),
                Value::Real(f32::NAN),
            ],
        )
        .unwrap_err();
    assert!(nan.contains("finite real required"), "{nan}");
    let kept = vm.host().inner.executed_unit(unit).unwrap().clone();
    assert_eq!(kept.unit_states.len(), 1);
    assert_eq!(kept.unit_states.capacity(), capacity);
    assert_eq!(assignment_bits(&kept, first), Some((-12.5f32).to_bits()));
    assert_eq!(
        vm.host()
            .inner
            .executed_unit(other_unit)
            .unwrap()
            .unit_states
            .len(),
        0
    );

    let exact = f32::from_bits(0x3f800001);
    vm.host()
        .native(
            "SetUnitState",
            &[Value::Handle(unit), Value::Handle(first), Value::Real(0.0)],
        )
        .unwrap();
    assert_eq!(
        assignment_bits(vm.host().inner.executed_unit(unit).unwrap(), first),
        Some(0.0f32.to_bits())
    );
    vm.host()
        .native(
            "SetUnitState",
            &[
                Value::Handle(unit),
                Value::Handle(first),
                Value::Real(exact),
            ],
        )
        .unwrap();
    assert_eq!(
        assignment_bits(vm.host().inner.executed_unit(unit).unwrap(), first),
        Some(exact.to_bits())
    );
    let wide = i32::MIN;
    vm.host()
        .native(
            "SetUnitState",
            &[Value::Handle(unit), Value::Handle(first), Value::Int(wide)],
        )
        .unwrap();
    let stored = vm.host().inner.executed_unit(unit).unwrap().clone();
    assert_eq!(stored.unit_states.len(), 1);
    assert_eq!(stored.unit_states.capacity(), capacity);
    assert_eq!(
        assignment_bits(&stored, first),
        Some((wide as f32).to_bits())
    );
    assert_eq!(stored.definition, created.definition);
    assert_eq!(hits.load(Ordering::SeqCst), 2);
    let getter = vm
        .host()
        .native("GetUnitState", &[Value::Handle(unit), Value::Handle(first)])
        .unwrap_err();
    assert!(getter.contains("unsupported"), "{getter}");
    assert_eq!(
        assignment_bits(vm.host().inner.executed_unit(unit).unwrap(), first),
        Some((wide as f32).to_bits())
    );
}

#[test]
fn unit_creation_without_the_state_setter_leaves_assignments_absent() {
    let mut session = open(4, 16, MAP);
    session.vm.init_globals().unwrap();
    session.vm.call("main", &[]).unwrap();
    let Value::Handle(unit) = session.vm.global("U") else {
        panic!("unit");
    };
    assert!(
        session
            .vm
            .host()
            .inner
            .executed_unit(unit)
            .unwrap()
            .unit_states
            .is_empty()
    );
    let unsupported = session
        .vm
        .host()
        .native(
            "SetUnitState",
            &[Value::Handle(unit), Value::Int(0), Value::Real(0.0)],
        )
        .unwrap_err();
    assert!(unsupported.contains("unsupported"), "{unsupported}");
    assert!(
        session
            .vm
            .host()
            .inner
            .executed_unit(unit)
            .unwrap()
            .unit_states
            .is_empty()
    );
    assert_eq!(session.hits.load(Ordering::SeqCst), 1);

    let ctx = context(
        300,
        &script_common("widget", true),
        "function main takes nothing returns nothing\nendfunction\n",
        "unit-catalog",
    );
    let (catalog, hits, _) = counting();
    let shared: Arc<dyn ObjectCatalog + Send + Sync> = catalog;
    let error = match object(&ctx, 4)
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_unit_catalog(shared, source(), limits(1))
        .unwrap()
        .with_unit_states(UnitStateBudget { max_records: 1 })
    {
        Err(error) => error,
        Ok(_) => panic!("omitted setter was admitted"),
    };
    assert_eq!(error.failure, Failure::Signature);
    assert_eq!(error.native, "SetUnitState");
    assert_eq!(hits.load(Ordering::SeqCst), 0);
}

#[test]
fn unit_state_bad_signature_does_not_admit_or_allocate() {
    for replacement in [
        "native SetUnitState takes integer level returns nothing\n",
        "constant native SetUnitState takes unit whichUnit, unitstate whichUnitState, real newVal returns nothing\n",
    ] {
        let common = state_common(true).replacen(
            "native SetUnitState takes unit whichUnit, unitstate whichUnitState, real newVal returns nothing\n",
            replacement,
            1,
        );
        let ctx = context(
            300,
            &common,
            "function main takes nothing returns nothing\nendfunction\n",
            "unit-catalog",
        );
        let (catalog, hits, _) = counting();
        let shared: Arc<dyn ObjectCatalog + Send + Sync> = catalog;
        let error = match ObjectHost::checked(state_enums(&ctx, STATE_CONVERSIONS))
            .unwrap()
            .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
            .unwrap()
            .with_unit_catalog(shared, source(), limits(1))
            .unwrap()
            .with_unit_states(UnitStateBudget { max_records: 4 })
        {
            Err(error) => error,
            Ok(_) => panic!("bad unit-state signature was admitted"),
        };
        assert_eq!(error.failure, Failure::Signature);
        assert_eq!(error.native, "SetUnitState");
        assert_eq!(error.declaration_source, "synthetic/common.j");
        assert_ne!(error.declaration_span, pcx_script::span::Span::EMPTY);
        assert_eq!(hits.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn unit_state_from_another_source_is_refused() {
    let primary = state_common(true).replacen(
        "native SetUnitState takes unit whichUnit, unitstate whichUnitState, real newVal returns nothing\n",
        "",
        1,
    );
    let other = "native SetUnitState takes unit whichUnit, unitstate whichUnitState, real newVal returns nothing\n";
    let map = pcx_script::parse_script("").unwrap();
    let primary = pcx_script::parse_script(&primary).unwrap();
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
    let inner = EnumHost::checked_selected(
        Arc::clone(&program),
        "split-source",
        STATE_CONVERSIONS,
        Limits {
            max_handles: 32,
            admission_steps: 1000,
            expression_depth: 16,
            expression_work: 1000,
        },
    )
    .unwrap();
    let (catalog, hits, _) = counting();
    let shared: Arc<dyn ObjectCatalog + Send + Sync> = catalog;
    let error = match ObjectHost::checked(inner)
        .unwrap()
        .with_unit_catalog(shared, source(), limits(1))
        .unwrap()
        .with_unit_states(UnitStateBudget { max_records: 1 })
    {
        Err(error) => error,
        Ok(_) => panic!("foreign unit-state source was admitted"),
    };
    assert_eq!(error.failure, Failure::Source);
    assert_eq!(error.native, "SetUnitState");
    assert_eq!(error.declaration_source, "synthetic/other.j");
    assert_ne!(error.declaration_span, pcx_script::span::Span::EMPTY);
    assert_eq!(hits.load(Ordering::SeqCst), 0);
}

#[test]
fn unit_state_requires_the_selected_convert_domain() {
    let names: Vec<&str> = STATE_CONVERSIONS
        .iter()
        .copied()
        .filter(|name| *name != "ConvertUnitState")
        .collect();
    let ctx = context(
        300,
        &state_common(true),
        "function main takes nothing returns nothing\nendfunction\n",
        "unit-catalog",
    );
    let (catalog, hits, _) = counting();
    let shared: Arc<dyn ObjectCatalog + Send + Sync> = catalog;
    let error = match ObjectHost::checked(state_enums(&ctx, &names))
        .unwrap()
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_unit_catalog(shared, source(), limits(1))
        .unwrap()
        .with_unit_states(UnitStateBudget { max_records: 1 })
    {
        Err(error) => error,
        Ok(_) => panic!("unselected unit-state domain was admitted"),
    };
    assert_eq!(error.failure, Failure::Signature);
    assert_eq!(error.native, "ConvertUnitState");
    assert_eq!(error.declaration_source, "synthetic/common.j");
    assert_ne!(error.declaration_span, pcx_script::span::Span::EMPTY);
    assert_eq!(hits.load(Ordering::SeqCst), 0);
}

#[test]
fn root_zero_state_budget_leaves_units_and_assignments_unchanged() {
    let (mut vm, hits, _) =
        state_catalog_host(&state_common(true), STATE_MAP, STATE_CONVERSIONS, 0).unwrap();
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    let Value::Handle(unit) = vm.global("U") else {
        panic!("unit")
    };
    let before = vm.host().inner.executed_unit(unit).unwrap().clone();
    let error = vm.call("zero", &[]).unwrap_err().to_string();
    assert!(error.contains("unit state assignments"), "{error}");
    assert_eq!(vm.host().inner.executed_unit(unit).unwrap(), &before);
    assert_eq!(hits.load(Ordering::SeqCst), 2);
}

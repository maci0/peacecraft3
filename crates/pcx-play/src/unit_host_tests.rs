use super::{COMMON, budget, context, object, slots_rule};
use crate::playercolor_host::{
    EnumHost, Failure, Limits, ObjectHost, TechLimitBudget, UnitBudget, UnitStateBudget,
};
use pcx_objects::editor::defaults::{DefaultTables, Identity};
use pcx_objects::editor::{FieldDef, FieldKind, FieldSource, ObjectKind, SlkId};
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
    script_common_with(unit_parent, declare_create, "")
}

fn script_common_with(unit_parent: &str, declare_create: bool, extra: &str) -> String {
    let globals = COMMON.find("\nglobals\n").expect("globals");
    let mut common = COMMON.to_string();
    let native = if declare_create {
        "native CreateUnit takes player id, integer unitid, real x, real y, real face returns unit\nnative SetUnitAcquireRange takes unit whichUnit, real newAcquireRange returns nothing\nnative SetHeroLevel takes unit whichHero, integer level, boolean showEyeCandy returns nothing\n"
    } else {
        ""
    };
    common.insert_str(
        globals + 1,
        &format!("type widget extends agent\ntype unit extends {unit_parent}\n{native}{extra}"),
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

fn open_identity(max_records: usize, max_handles: usize, map: &str) -> VmHost {
    open_identity_with(max_records, max_handles, map, false)
}

fn open_identity_with(max_records: usize, max_handles: usize, map: &str, constant: bool) -> VmHost {
    let prefix = if constant { "constant " } else { "" };
    let extra = format!(
        "{prefix}native GetUnitTypeId takes unit whichUnit returns integer\n{prefix}native GetOwningPlayer takes unit whichUnit returns player\n"
    );
    let common = script_common_with("widget", true, &extra);
    let ctx = context(300, &common, map, "unit-catalog");
    let (catalog, hits, seen) = counting();
    let shared: Arc<dyn ObjectCatalog + Send + Sync> = catalog;
    let host = object(&ctx, max_handles)
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_unit_catalog(Arc::clone(&shared), source(), limits(max_records))
        .unwrap();
    VmHost {
        vm: Arc::clone(&ctx.scripts).owned_vm(host),
        hits,
        seen,
        catalog: shared,
    }
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

const IDENTITY_MAP: &str = "globals\nplayer P = null\nunit U = null\ntimer T = null\ninteger type_id = 0\nplayer owner = null\nendglobals\nfunction main takes nothing returns nothing\nset P = Player(4)\nset U = CreateUnit(P, 'hfoo', 1.5, -2.25, 90.0)\nendfunction\nfunction identities takes nothing returns nothing\nset type_id = GetUnitTypeId(U)\nset owner = GetOwningPlayer(U)\nendfunction\nfunction identity_foreign takes nothing returns integer\nset T = CreateTimer()\nreturn GetUnitTypeId(T)\nendfunction\nfunction owner_foreign takes nothing returns player\nset T = CreateTimer()\nreturn GetOwningPlayer(T)\nendfunction\n";

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
fn owned_vm_returns_the_admitted_rawcode_and_original_owner_handle() {
    for constant in [false, true] {
        let mut session = open_identity_with(2, 16, IDENTITY_MAP, constant);
        session.vm.init_globals().unwrap();
        session.vm.call("main", &[]).unwrap();
        let Value::Handle(player) = session.vm.global("P") else {
            panic!("player");
        };
        let Value::Handle(unit) = session.vm.global("U") else {
            panic!("unit");
        };
        let handles = session.vm.host().inner.inner().handles();
        let hits = session.hits.load(Ordering::SeqCst);
        session.vm.call("identities", &[]).unwrap();
        assert_eq!(
            session.vm.global("type_id"),
            Value::Int(i32::from_be_bytes(*b"hfoo"))
        );
        assert_eq!(session.vm.global("owner"), Value::Handle(player));
        assert_eq!(session.vm.host().inner.player_identity(4), Some(player));
        let executed = session.vm.host().inner.executed_unit(unit).unwrap();
        assert_eq!(executed.owner, player);
        assert_eq!(executed.executed_owner, 4);
        assert_eq!(executed.requested_hero_level, None);
        assert!(executed.unit_states.is_empty());
        assert_eq!(session.vm.host().inner.inner().handles(), handles);
        assert_eq!(session.hits.load(Ordering::SeqCst), hits);
    }
}

#[test]
fn identity_getters_refuse_another_family_and_a_reserved_unit_token() {
    let mut session = open_identity(2, 16, IDENTITY_MAP);
    session.vm.init_globals().unwrap();
    session.vm.call("main", &[]).unwrap();
    let type_error = session
        .vm
        .call("identity_foreign", &[])
        .unwrap_err()
        .to_string();
    assert!(
        type_error.contains("handle identifies another family"),
        "{type_error}"
    );
    assert!(type_error.contains("GetUnitTypeId"), "{type_error}");
    let owner_error = session
        .vm
        .call("owner_foreign", &[])
        .unwrap_err()
        .to_string();
    assert!(
        owner_error.contains("handle identifies another family"),
        "{owner_error}"
    );
    assert!(owner_error.contains("GetOwningPlayer"), "{owner_error}");

    let extra = "native GetUnitTypeId takes unit whichUnit returns integer\nnative GetOwningPlayer takes unit whichUnit returns player\nnative SetUnitInvulnerable takes unit u, boolean flag returns nothing\nnative BlzIsUnitInvulnerable takes unit u returns boolean\n";
    let common = script_common_with("widget", true, extra);
    let ctx = context(300, &common, IDENTITY_MAP, "unit-catalog");
    let (catalog, _, _) = counting();
    let shared: Arc<dyn ObjectCatalog + Send + Sync> = catalog;
    let mut inner = EnumHost::checked_selected(
        ctx.scripts.shared_program(),
        &ctx.map_digest.label,
        &["ConvertPlayerColor"],
        Limits {
            max_handles: 16,
            admission_steps: 1000,
            expression_depth: 16,
            expression_work: 1000,
        },
    )
    .unwrap();
    let reserved = inner.reserve_identity("unit").unwrap();
    let mut host = ObjectHost::checked(inner)
        .unwrap()
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_unit_catalog(shared, source(), limits(1))
        .unwrap();
    let player = host.native("Player", &[Value::Int(4)]).unwrap();
    let unit = host
        .native(
            "CreateUnit",
            &[
                player.clone(),
                Value::Int(i32::from_be_bytes(*b"hfoo")),
                Value::Real(0.0),
                Value::Real(0.0),
                Value::Real(0.0),
            ],
        )
        .unwrap();
    assert_eq!(
        host.native("GetOwningPlayer", std::slice::from_ref(&unit))
            .unwrap(),
        player
    );
    for (name, args) in [
        (
            "SetUnitInvulnerable",
            vec![Value::Handle(reserved), Value::Bool(true)],
        ),
        ("BlzIsUnitInvulnerable", vec![Value::Handle(reserved)]),
    ] {
        let e = host.native(name, &args).unwrap_err();
        assert!(
            e.contains(name) && e.contains("handle identifies another family"),
            "{e}"
        );
    }
    let reserved_type = host
        .native("GetUnitTypeId", &[Value::Handle(reserved)])
        .unwrap_err();
    assert!(
        reserved_type.contains("handle identifies another family"),
        "{reserved_type}"
    );
    let reserved_owner = host
        .native("GetOwningPlayer", &[Value::Handle(reserved)])
        .unwrap_err();
    assert!(
        reserved_owner.contains("handle identifies another family"),
        "{reserved_owner}"
    );
}

#[test]
fn identity_getters_from_another_source_are_refused() {
    let primary = "type agent extends handle\ntype widget extends agent\ntype unit extends widget\ntype player extends agent\ntype timer extends agent\ntype playercolor extends handle\nconstant native ConvertPlayerColor takes integer i returns playercolor\nnative CreateTimer takes nothing returns timer\nnative DestroyTimer takes timer t returns nothing\nnative CreateUnit takes player id, integer unitid, real x, real y, real face returns unit\nglobals\nconstant playercolor C = ConvertPlayerColor(1)\nendglobals\n";
    for (name, other) in [
        (
            "GetUnitTypeId",
            "native GetUnitTypeId takes unit whichUnit returns integer\nnative GetOwningPlayer takes unit whichUnit returns player\n",
        ),
        (
            "BlzGetUnitCollisionSize",
            "native BlzGetUnitCollisionSize takes unit u returns real\n",
        ),
    ] {
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
            Ok(_) => panic!("foreign identity getters were admitted"),
        };
        assert_eq!(error.failure, Failure::Source);
        assert_eq!(error.native, name);
        assert_eq!(error.declaration_source, "synthetic/other.j");
        assert_eq!(hits.load(Ordering::SeqCst), 0);
    }
}
#[test]
fn identity_getters_validate_the_loaded_signature_before_catalog_use() {
    for (name, declaration) in [
        (
            "GetUnitTypeId",
            "constant native GetUnitTypeId takes timer t returns integer\n",
        ),
        (
            "GetUnitTypeId",
            "native GetUnitTypeId takes unit u returns real\n",
        ),
        (
            "GetOwningPlayer",
            "constant native GetOwningPlayer takes unit u, integer i returns player\n",
        ),
        (
            "GetOwningPlayer",
            "native GetOwningPlayer takes unit u returns integer\n",
        ),
        (
            "BlzGetUnitCollisionSize",
            "native BlzGetUnitCollisionSize takes timer u returns real\n",
        ),
        (
            "BlzGetUnitCollisionSize",
            "native BlzGetUnitCollisionSize takes unit u returns integer\n",
        ),
    ] {
        let common = script_common_with("widget", true, declaration);
        let ctx = context(300, &common, MAP, "unit-catalog");
        let (catalog, hits, _) = counting();
        let shared: Arc<dyn ObjectCatalog + Send + Sync> = catalog;
        let error = object(&ctx, 16)
            .with_unit_catalog(shared, source(), limits(1))
            .err()
            .expect("malformed getter refused");
        assert_eq!(error.failure, Failure::Signature, "{name}");
        assert_eq!(error.native, name);
        assert_eq!(error.declaration_source, "synthetic/common.j");
        assert_eq!(hits.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn omitted_identity_getters_stay_unsupported() {
    let mut session = open(2, 8, MAP);
    session.vm.init_globals().unwrap();
    session.vm.call("main", &[]).unwrap();
    let Value::Handle(unit) = session.vm.global("U") else {
        panic!("unit");
    };
    let missing = session
        .vm
        .host()
        .native("GetUnitTypeId", &[Value::Handle(unit)])
        .unwrap_err();
    assert!(missing.contains("unsupported"), "{missing}");
    let missing_owner = session
        .vm
        .host()
        .native("GetOwningPlayer", &[Value::Handle(unit)])
        .unwrap_err();
    assert!(missing_owner.contains("unsupported"), "{missing_owner}");
    let missing_collision = session
        .vm
        .host()
        .native("BlzGetUnitCollisionSize", &[Value::Handle(unit)])
        .unwrap_err();
    assert!(
        missing_collision.contains("unsupported"),
        "{missing_collision}"
    );
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

const ORDER_HEADERS: &[&str] = &[
    "alias",
    "code",
    "hero",
    "item",
    "checkDep",
    "levels",
    "reqLevel",
    "levelSkip",
    "Cool1",
    "Cost1",
];

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

fn order_tables(headers: &[&str], row: &[&str], amount: &str, maximum: &str) -> DefaultTables {
    let bytes = sylk(headers, &[row]);
    let upgrade = sylk(&["upgradeid", "maxlevel"], &[&["Qtec", maximum]]);
    let profile = format!("[Qord]\r\nOrder=engage\r\nUnorder=release\r\nRequires=Qtec\r\n{amount}");
    DefaultTables::from_bytes(
        Identity {
            install: "synthetic".into(),
            edition_version: "fixture edition/version".into(),
        },
        &[
            (SlkId::AbilityData.path(), bytes.as_slice()),
            (SlkId::UpgradeData.path(), upgrade.as_slice()),
        ],
        &[("Units\\QAbilityFunc.txt", profile.as_bytes())],
        pcx_objects::editor::EditorReadLimits {
            max_listed_paths: 3,
            max_asset_bytes: 64 << 10,
            max_aggregate_input_bytes: 128 << 10,
        },
    )
    .unwrap_or_else(|error| panic!("synthetic tables: {error}"))
}

fn ability_field(column: &str, text: &str) -> pcx_sim::world::ObjectField {
    pcx_sim::world::ObjectField {
        metadata: Some(FieldDef {
            fourcc: *b"uabi",
            from: SlkId::UnitMetaData,
            field: column.into(),
            slk: "UnitAbilities".into(),
            index: -1,
            declared: "abilityList".into(),
            kind: FieldKind::Token,
            category: "abil".into(),
            source: FieldSource::Column {
                slk: SlkId::UnitAbilities,
                column: column.into(),
            },
            measured: true,
        }),
        value: Ok(FieldValue::Cell(text.into())),
    }
}

struct OrderCatalog {
    hits: Arc<AtomicUsize>,
}

impl ObjectCatalog for OrderCatalog {
    fn resolve(&self, rawcode: [u8; 4]) -> Result<Definition, ResolutionProblem> {
        self.hits.fetch_add(1, Ordering::SeqCst);
        let list = match &rawcode {
            b"u001" => "Qord",
            b"u002" => "Zzzz, Qord",
            b"u003" => "qord",
            _ => {
                return Err(ResolutionProblem::MissingRawcode(pcx_objects::fourcc_text(
                    rawcode,
                )));
            }
        };
        let mut fields = BTreeMap::new();
        let mut ability = ability_field("abilList", list);
        if rawcode == *b"u002" {
            ability.value = Ok(FieldValue::Override(pcx_objects::Value::Str(
                list.as_bytes().to_vec(),
            )));
        }
        fields.insert(*b"uabi", ability);
        if rawcode == *b"u003" {
            fields.insert(*b"uhab", ability_field("heroAbilList", "Qord"));
        }
        Ok(definition(
            rawcode,
            pcx_objects::fourcc_text(rawcode),
            ObjectKind::Unit,
            fields,
        ))
    }
}

fn order_common() -> String {
    let mut common = script_common("widget", true);
    let globals = common.find("\nglobals\n").expect("globals");
    common.insert_str(
        globals + 1,
        "native IssueImmediateOrder takes unit whichUnit, string order returns boolean\n\
constant native SetPlayerTechResearched takes player whichPlayer, integer techid, integer setToLevel returns nothing\n",
    );
    common
}

type OrderVm = pcx_script::OwnedVm<crate::scripts::OwnedLocalizedHost<ObjectHost>>;

fn order_vm(tables: &DefaultTables, research: bool) -> (OrderVm, Arc<AtomicUsize>) {
    let common = order_common();
    let map = "globals\nplayer P = null\nunit A = null\nunit B = null\nunit C = null\nendglobals\n\
function main takes nothing returns nothing\n\
set P = Player(4)\n\
set A = CreateUnit(P, 'u001', 1.0, 2.0, 3.0)\n\
set B = CreateUnit(P, 'u002', 4.0, 5.0, 6.0)\n\
set C = CreateUnit(P, 'u003', 7.0, 8.0, 9.0)\n\
endfunction\n\
function issue takes unit target, string text returns boolean\n\
return IssueImmediateOrder(target, text)\n\
endfunction\n\
function research takes integer tech, integer level returns nothing\n\
call SetPlayerTechResearched(P, tech, level)\n\
endfunction\n";
    let ctx = context(300, &common, map, "unit-catalog");
    let hits = Arc::new(AtomicUsize::new(0));
    let shared: Arc<dyn ObjectCatalog + Send + Sync> = Arc::new(OrderCatalog {
        hits: Arc::clone(&hits),
    });
    let mut host = object(&ctx, 16)
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_unit_catalog(Arc::clone(&shared), source(), limits(3))
        .unwrap();
    if research {
        host = host
            .with_player_tech_researched(TechLimitBudget { max_records: 2 })
            .unwrap();
    }
    let host = host.with_defend_ability(tables, "Qord").unwrap();
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host);
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    (vm, hits)
}

fn order_bit(vm: &mut OrderVm, name: &str) -> Option<([u8; 4], bool)> {
    let Value::Handle(unit) = vm.global(name) else {
        panic!("unit");
    };
    vm.host().inner.executed_unit(unit).unwrap().admitted_order
}

fn issue(vm: &mut OrderVm, name: &str, text: &str) -> Result<Value, String> {
    let Value::Handle(unit) = vm.global(name) else {
        panic!("unit");
    };
    vm.call("issue", &[Value::Handle(unit), Value::Str(text.into())])
        .map_err(|error| error.to_string())
}

fn research(vm: &mut OrderVm, tech: [u8; 4], level: i32) {
    vm.call(
        "research",
        &[Value::Int(i32::from_be_bytes(tech)), Value::Int(level)],
    )
    .unwrap();
}

fn refused_order(headers: &[&str], row: &[&str], amount: &str) -> String {
    let tables = order_tables(headers, row, amount, "1");
    refused_tables(&tables)
}

fn refused_tables(tables: &DefaultTables) -> String {
    let common = order_common();
    let ctx = context(
        300,
        &common,
        "function main takes nothing returns nothing\nendfunction\n",
        "unit-catalog",
    );
    let shared: Arc<dyn ObjectCatalog + Send + Sync> = Arc::new(OrderCatalog {
        hits: Arc::new(AtomicUsize::new(0)),
    });
    object(&ctx, 8)
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_unit_catalog(shared, source(), limits(1))
        .unwrap()
        .with_defend_ability(tables, "Qord")
        .map(|_| String::new())
        .unwrap_or_else(|error| error.to_string())
}

#[test]
fn owned_vm_defend_tracks_selected_ability_and_owner_research() {
    let good = &["Qord", "Qcls", "0", "0", "1", "1", "0", "0", "0", "0"];
    let absent = order_tables(ORDER_HEADERS, good, "", "1");
    let (mut cold, _) = order_vm(&absent, false);
    assert_eq!(order_bit(&mut cold, "A"), Some((*b"Qcls", false)));
    assert_eq!(
        issue(&mut cold, "A", "release").unwrap(),
        Value::Bool(false)
    );
    assert_eq!(order_bit(&mut cold, "A"), Some((*b"Qcls", false)));
    let before_research = issue(&mut cold, "A", "engage").unwrap_err();
    assert!(before_research.contains("unsupported"), "{before_research}");
    assert_eq!(order_bit(&mut cold, "A"), Some((*b"Qcls", false)));

    let (mut vm, hits) = order_vm(&absent, true);
    assert_eq!(hits.load(Ordering::SeqCst), 3);
    assert_eq!(order_bit(&mut vm, "A"), Some((*b"Qcls", false)));
    assert_eq!(order_bit(&mut vm, "B"), Some((*b"Qcls", false)));
    assert_eq!(order_bit(&mut vm, "C"), None);
    assert_eq!(issue(&mut vm, "A", "release").unwrap(), Value::Bool(false));
    assert_eq!(order_bit(&mut vm, "A"), Some((*b"Qcls", false)));
    let unsupported = issue(&mut vm, "A", "engage").unwrap_err();
    assert!(unsupported.contains("unsupported"), "{unsupported}");
    let early_other = issue(&mut vm, "A", "other").unwrap_err();
    assert!(early_other.contains("unsupported"), "{early_other}");
    assert_eq!(order_bit(&mut vm, "A"), Some((*b"Qcls", false)));
    research(&mut vm, *b"Qoth", 1);
    let other_tech = issue(&mut vm, "A", "engage").unwrap_err();
    assert!(other_tech.contains("unsupported"), "{other_tech}");
    research(&mut vm, *b"Qtec", 0);
    let explicit_zero = issue(&mut vm, "A", "engage").unwrap_err();
    assert!(explicit_zero.contains("unsupported"), "{explicit_zero}");
    research(&mut vm, *b"Qtec", 2);
    let unobserved = issue(&mut vm, "A", "engage").unwrap_err();
    assert!(unobserved.contains("unsupported"), "{unobserved}");
    assert_eq!(order_bit(&mut vm, "A"), Some((*b"Qcls", false)));
    research(&mut vm, *b"Qtec", 1);
    assert_eq!(issue(&mut vm, "A", "engage").unwrap(), Value::Bool(true));
    assert_eq!(order_bit(&mut vm, "A"), Some((*b"Qcls", true)));
    assert_eq!(order_bit(&mut vm, "B"), Some((*b"Qcls", false)));
    assert_eq!(issue(&mut vm, "A", "engage").unwrap(), Value::Bool(false));
    assert_eq!(order_bit(&mut vm, "A"), Some((*b"Qcls", true)));
    assert_eq!(issue(&mut vm, "A", "release").unwrap(), Value::Bool(true));
    assert_eq!(order_bit(&mut vm, "A"), Some((*b"Qcls", false)));
    assert_eq!(issue(&mut vm, "A", "release").unwrap(), Value::Bool(false));
    assert_eq!(order_bit(&mut vm, "A"), Some((*b"Qcls", false)));
    assert!(
        issue(&mut vm, "A", "other")
            .unwrap_err()
            .contains("unsupported")
    );
    assert_eq!(order_bit(&mut vm, "A"), Some((*b"Qcls", false)));
    assert_eq!(issue(&mut vm, "B", "engage").unwrap(), Value::Bool(true));
    assert_eq!(order_bit(&mut vm, "A"), Some((*b"Qcls", false)));
    assert_eq!(order_bit(&mut vm, "B"), Some((*b"Qcls", true)));
    assert_eq!(issue(&mut vm, "B", "release").unwrap(), Value::Bool(true));
    assert_eq!(order_bit(&mut vm, "B"), Some((*b"Qcls", false)));
    let without = issue(&mut vm, "C", "engage").unwrap_err();
    assert!(without.contains("unsupported"), "{without}");
    let without_off = issue(&mut vm, "C", "release").unwrap_err();
    assert!(without_off.contains("unsupported"), "{without_off}");
    assert_eq!(order_bit(&mut vm, "C"), None);
    let bad_args = vm
        .host()
        .native("IssueImmediateOrder", &[Value::Int(1)])
        .unwrap_err();
    assert!(
        bad_args.contains("arguments do not match the unit native declaration"),
        "{bad_args}"
    );
    assert_eq!(order_bit(&mut vm, "A"), Some((*b"Qcls", false)));
    let Value::Handle(player) = vm.global("P") else {
        panic!("player");
    };
    let wrong_family = vm
        .host()
        .native(
            "IssueImmediateOrder",
            &[Value::Handle(player), Value::Str("engage".into())],
        )
        .unwrap_err();
    assert!(
        wrong_family.contains("handle identifies another family"),
        "{wrong_family}"
    );
    assert_eq!(order_bit(&mut vm, "A"), Some((*b"Qcls", false)));

    let blank = order_tables(ORDER_HEADERS, good, "Requiresamount=\r\n", "1");
    let (mut blank_vm, _) = order_vm(&blank, true);
    let blank_before = issue(&mut blank_vm, "A", "engage").unwrap_err();
    assert!(blank_before.contains("unsupported"), "{blank_before}");
    research(&mut blank_vm, *b"Qtec", 1);
    assert_eq!(
        issue(&mut blank_vm, "A", "engage").unwrap(),
        Value::Bool(true)
    );
    assert_eq!(order_bit(&mut blank_vm, "A"), Some((*b"Qcls", true)));

    let missing_cool = refused_order(
        &[
            "alias",
            "code",
            "hero",
            "item",
            "checkDep",
            "levels",
            "reqLevel",
            "levelSkip",
            "Cost1",
        ],
        &["Qord", "Qcls", "0", "0", "1", "1", "0", "0", "0"],
        "",
    );
    assert!(missing_cool.contains("missing Cool1"), "{missing_cool}");
    let blank_cool = refused_order(
        ORDER_HEADERS,
        &["Qord", "Qcls", "0", "0", "1", "1", "0", "0", "", "0"],
        "",
    );
    assert!(blank_cool.contains("blank Cool1"), "{blank_cool}");
    let marker_cool = refused_order(
        ORDER_HEADERS,
        &["Qord", "Qcls", "0", "0", "1", "1", "0", "0", "-", "0"],
        "",
    );
    assert!(marker_cool.contains("marker Cool1"), "{marker_cool}");
    let paid = refused_order(
        ORDER_HEADERS,
        &["Qord", "Qcls", "0", "0", "1", "1", "0", "0", "0", "4"],
        "",
    );
    assert!(paid.contains("Cost1 is not authored zero"), "{paid}");
    let amount = refused_order(ORDER_HEADERS, good, "Requiresamount=2\r\n");
    assert!(
        amount.contains("Requiresamount is not an absent or blank level"),
        "{amount}"
    );
}

#[test]
fn defend_requires_one_authored_upgrade_level_before_any_unit_creation() {
    let good = &["Qord", "Qcls", "0", "0", "1", "1", "0", "0", "0", "0"];
    for maximum in ["", "-", "0", "2"] {
        let tables = order_tables(ORDER_HEADERS, good, "", maximum);
        assert!(
            !refused_tables(&tables).is_empty(),
            "accepted maxlevel {maximum:?}"
        );
    }
    let tables = order_tables(ORDER_HEADERS, good, "", "1");
    let common = order_common();
    let ctx = context(
        300,
        &common,
        "function main takes nothing returns nothing\nendfunction\n",
        "unit-catalog",
    );
    let shared: Arc<dyn ObjectCatalog + Send + Sync> = Arc::new(OrderCatalog {
        hits: Arc::new(AtomicUsize::new(0)),
    });
    let mut host = object(&ctx, 16)
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_unit_catalog(shared, source(), limits(1))
        .unwrap();
    let player = host.native("Player", &[Value::Int(4)]).unwrap();
    host.native(
        "CreateUnit",
        &[
            player,
            Value::Int(i32::from_be_bytes(*b"u001")),
            Value::Real(0.0),
            Value::Real(0.0),
            Value::Real(0.0),
        ],
    )
    .unwrap();
    let Err(error) = host.with_defend_ability(&tables, "Qord") else {
        panic!("late admission succeeded");
    };
    assert!(
        error
            .to_string()
            .contains("admit Defend before creating units")
    );
}

struct GeometryCatalog {
    fields: BTreeMap<[u8; 4], ObjectField>,
    hits: Arc<AtomicUsize>,
}

impl ObjectCatalog for GeometryCatalog {
    fn resolve(&self, rawcode: [u8; 4]) -> Result<Definition, ResolutionProblem> {
        self.hits.fetch_add(1, Ordering::SeqCst);
        Ok(definition(
            rawcode,
            "synthetic-base".into(),
            ObjectKind::Unit,
            self.fields.clone(),
        ))
    }
}

fn geometry_field(value: Result<FieldValue, pcx_sim::world::FieldProblem>) -> ObjectField {
    ObjectField {
        metadata: Some(FieldDef {
            fourcc: *b"G001",
            from: SlkId::UnitMetaData,
            field: "collision".into(),
            slk: "UnitBalance".into(),
            index: -1,
            declared: "unreal".into(),
            kind: FieldKind::Unreal,
            category: "synthetic-geometry".into(),
            source: FieldSource::Column {
                slk: SlkId::UnitBalance,
                column: "collision".into(),
            },
            measured: true,
        }),
        value,
    }
}

fn geometry_vm(fields: BTreeMap<[u8; 4], ObjectField>, constant: bool) -> VmHost {
    let prefix = if constant { "constant " } else { "" };
    let common = script_common_with(
        "widget",
        true,
        &format!("{prefix}native BlzGetUnitCollisionSize takes unit u returns real\n"),
    );
    let map = format!(
        "{MAP}\nfunction collision takes unit u returns real\nreturn BlzGetUnitCollisionSize(u)\nendfunction\n"
    );
    let ctx = context(300, &common, &map, "unit-catalog");
    let hits = Arc::new(AtomicUsize::new(0));
    let catalog: Arc<dyn ObjectCatalog + Send + Sync> = Arc::new(GeometryCatalog {
        fields,
        hits: Arc::clone(&hits),
    });
    let host = object(&ctx, 16)
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_unit_catalog(Arc::clone(&catalog), source(), limits(1))
        .unwrap();
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host);
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    VmHost {
        vm,
        hits,
        seen: Arc::new(Mutex::new(Vec::new())),
        catalog,
    }
}

#[test]
fn collision_query_uses_the_retained_catalog_field_without_resolving_again() {
    for constant in [false, true] {
        for (kind, value, expected) in [
            (
                FieldKind::Unreal,
                FieldValue::Cell(" 27.125 ".into()),
                27.125,
            ),
            (
                FieldKind::Unreal,
                FieldValue::Override(pcx_objects::Value::Unreal(9.5)),
                9.5,
            ),
            (
                FieldKind::Real,
                FieldValue::Override(pcx_objects::Value::Real(0.0)),
                0.0,
            ),
        ] {
            let mut field = geometry_field(Ok(value));
            field.metadata.as_mut().unwrap().kind = kind;
            let mut session = geometry_vm(BTreeMap::from([(*b"G001", field)]), constant);
            let unit = session.vm.global("U");
            let before = session.vm.host().inner.inner().handles();
            assert_eq!(
                session
                    .vm
                    .call("collision", std::slice::from_ref(&unit))
                    .unwrap(),
                Value::Real(expected)
            );
            assert_eq!(
                session
                    .vm
                    .host()
                    .native("BlzGetUnitCollisionSize", std::slice::from_ref(&unit))
                    .unwrap(),
                Value::Real(expected)
            );
            assert_eq!(session.vm.host().inner.inner().handles(), before);
            assert_eq!(session.hits.load(Ordering::SeqCst), 1);
            let player = session.vm.global("P");
            let wrong = session
                .vm
                .host()
                .native("BlzGetUnitCollisionSize", &[player])
                .unwrap_err();
            assert!(wrong.contains("BlzGetUnitCollisionSize"), "{wrong}");
            assert!(
                session
                    .vm
                    .host()
                    .native("BlzGetUnitCollisionSize", &[Value::Null])
                    .is_err()
            );
        }
    }
}

#[test]
fn collision_query_refuses_missing_ambiguous_and_unusable_geometry() {
    let good = geometry_field(Ok(FieldValue::Cell("27.125".into())));
    let mut cases = vec![BTreeMap::new()];
    let mut duplicate = good.clone();
    duplicate.metadata.as_mut().unwrap().fourcc = *b"G002";
    cases.push(BTreeMap::from([
        (*b"G001", good.clone()),
        (*b"G002", duplicate),
    ]));
    for value in [
        Err(pcx_sim::world::FieldProblem::Missing),
        Err(pcx_sim::world::FieldProblem::Indexed([1, 2])),
        Ok(FieldValue::Cell("_".into())),
        Ok(FieldValue::Cell("-1".into())),
        Ok(FieldValue::Cell("NaN".into())),
        Ok(FieldValue::Cell("1e999".into())),
        Ok(FieldValue::Override(pcx_objects::Value::Int(8))),
        Ok(FieldValue::Override(pcx_objects::Value::Real(8.0))),
        Ok(FieldValue::Override(pcx_objects::Value::Unreal(
            f32::INFINITY,
        ))),
    ] {
        cases.push(BTreeMap::from([(*b"G001", geometry_field(value))]));
    }
    for mutation in 0..5 {
        let mut field = good.clone();
        let meta = field.metadata.as_mut().unwrap();
        match mutation {
            0 => meta.fourcc = *b"G002",
            1 => meta.from = SlkId::AbilityMetaData,
            2 => meta.kind = FieldKind::Int,
            3 => meta.index = 0,
            _ => {
                meta.source = FieldSource::Column {
                    slk: SlkId::UnitData,
                    column: "collision".into(),
                }
            }
        }
        cases.push(BTreeMap::from([(*b"G001", field)]));
    }
    for fields in cases {
        let mut session = geometry_vm(fields, false);
        let unit = session.vm.global("U");
        let error = session
            .vm
            .host()
            .native("BlzGetUnitCollisionSize", &[unit])
            .unwrap_err();
        assert!(error.contains("BlzGetUnitCollisionSize"), "{error}");
        assert!(error.contains("synthetic/common.j"), "{error}");
        assert_eq!(session.hits.load(Ordering::SeqCst), 1);
    }
}

fn position_host(queries: usize, memberships: usize) -> ObjectHost {
    position_host_program(
        queries,
        memberships,
        "function main takes nothing returns nothing\nendfunction\n",
        "",
    )
}

fn position_host_program(
    queries: usize,
    memberships: usize,
    map: &str,
    declarations: &str,
) -> ObjectHost {
    let mut extra = String::from(
        "type rect extends agent\ntype group extends agent\ntype boolexpr extends agent\ntype destructable extends widget\nnative CreateDestructable takes integer id, real x, real y, real face, real scale, integer variation returns destructable\n",
    );
    extra.push_str("type filterfunc extends boolexpr\ntype conditionfunc extends boolexpr\nnative Filter takes code f returns filterfunc\nnative Condition takes code f returns conditionfunc\nconstant native GetFilterUnit takes nothing returns unit\nconstant native GetOwningPlayer takes unit u returns player\n");
    extra.push_str("native ForGroup takes group g, code f returns nothing\nconstant native GetEnumUnit takes nothing returns unit\n");
    extra.push_str(declarations);
    for (name, args, result, constant) in [
        ("GetUnitX", "unit u", "real", true),
        ("GetUnitY", "unit u", "real", true),
        (
            "GroupEnumUnitsInRect",
            "group g, rect r, boolexpr b",
            "nothing",
            false,
        ),
        ("CreateGroup", "nothing", "group", false),
        ("DestroyGroup", "group g", "nothing", false),
        ("GroupClear", "group g", "nothing", false),
        ("FirstOfGroup", "group g", "unit", false),
        ("GroupAddUnit", "group g, unit u", "nothing", false),
        ("GroupRemoveUnit", "group g, unit u", "nothing", false),
        ("IsUnitInGroup", "unit u, group g", "boolean", true),
        ("Rect", "real a, real b, real c, real d", "rect", false),
        ("RemoveRect", "rect r", "nothing", false),
        (
            "SetRect",
            "rect r, real a, real b, real c, real d",
            "nothing",
            false,
        ),
        ("MoveRectTo", "rect r, real x, real y", "nothing", false),
        ("GetRectCenterX", "rect r", "real", false),
        ("GetRectCenterY", "rect r", "real", false),
        ("GetRectMinX", "rect r", "real", false),
        ("GetRectMinY", "rect r", "real", false),
        ("GetRectMaxX", "rect r", "real", false),
        ("GetRectMaxY", "rect r", "real", false),
    ] {
        extra.push_str(&format!(
            "{}native {name} takes {args} returns {result}\n",
            if constant { "constant " } else { "" }
        ));
    }
    let common = script_common_with("widget", true, &extra);
    let ctx = context(300, &common, map, "unit-catalog");
    let mut fields = BTreeMap::new();
    fields.insert(*b"G001", geometry_field(Ok(FieldValue::Cell("2".into()))));
    for (id, column, value) in [(*b"M001", "movetp", "foot"), (*b"P001", "pathTex", "_")] {
        let mut field = geometry_field(Ok(FieldValue::Cell(value.into())));
        let meta = field.metadata.as_mut().unwrap();
        meta.fourcc = id;
        meta.field = column.into();
        meta.kind = FieldKind::Token;
        meta.source = FieldSource::Column {
            slk: SlkId::UnitData,
            column: column.into(),
        };
        fields.insert(id, field);
    }
    let catalog: Arc<dyn ObjectCatalog + Send + Sync> = Arc::new(GeometryCatalog {
        fields,
        hits: Arc::new(AtomicUsize::new(0)),
    });
    let mut conversions = vec![
        "ConvertPlacement",
        "ConvertPlayerColor",
        "ConvertRacePref",
        "ConvertMapControl",
    ];
    for name in ["ConvertUnitEvent", "ConvertGameEvent"] {
        if ctx.scripts.shared_program().natives().get(name).is_some() {
            conversions.push(name);
        }
    }
    let inner = EnumHost::checked_selected(
        ctx.scripts.shared_program(),
        &ctx.map_digest.label,
        &conversions,
        Limits {
            max_handles: 256,
            admission_steps: 1000,
            expression_depth: 64,
            expression_work: 4096,
        },
    )
    .unwrap();
    let mut host = ObjectHost::checked(inner)
        .unwrap()
        .with_map_config(crate::playercolor_host::MapConfigBudget {
            text_bytes: 128,
            start_locations: 2,
        })
        .unwrap()
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_unit_catalog(catalog, source(), limits(8))
        .unwrap()
        .with_groups(memberships)
        .unwrap()
        .with_rectangles()
        .unwrap()
        .with_destructables(crate::playercolor_host::DestructableBudget { max_records: 2 })
        .unwrap();
    host = host
        .with_filters(crate::playercolor_host::FilterBudget { max_records: 16 })
        .unwrap()
        .with_conditions(crate::playercolor_host::ConditionBudget { max_records: 16 })
        .unwrap();
    if ctx
        .scripts
        .shared_program()
        .natives()
        .get("CreateTrigger")
        .is_some()
    {
        host = host
            .with_trigger_admission(crate::playercolor_host::TriggerBudget {
                max_triggers: 8,
                max_unit_events: 0,
                max_actions: 8,
            })
            .unwrap()
            .with_trigger_conditions(8)
            .unwrap()
            .with_timer_events(8)
            .unwrap()
            .with_timer_runtime(0.0)
            .unwrap()
            .with_trigger_runtime(crate::playercolor_host::TriggerRuntimeInput {
                initial_enabled: true,
                timer_event_global: "TEST_TIMER".into(),
                max_timed_events: 8,
            })
            .unwrap();
    }
    host.admit_unit_placement(crate::unit_placement::tests::placement(source(), queries))
        .unwrap();
    host
}

fn position_create(host: &mut ObjectHost, x: f32, y: f32) -> Result<Value, String> {
    let owner = host.native("Player", &[Value::Int(4)]).unwrap();
    host.native(
        "CreateUnit",
        &[
            owner,
            Value::Int(i32::from_be_bytes(*b"hfoo")),
            Value::Real(x),
            Value::Real(y),
            Value::Real(0.0),
        ],
    )
}

#[test]
fn accepted_positions_and_rectangle_members_use_the_executed_unit_handle() {
    let mut host = position_host(100, 8);
    let first = position_create(&mut host, 32.0005, 32.0).unwrap();
    let second = position_create(&mut host, 32.0005, 32.0).unwrap();
    let Value::Handle(id) = second else {
        panic!("unit");
    };
    let unit = host.executed_unit(id).unwrap();
    assert_eq!(unit.x, 32.0005);
    assert_ne!(unit.position, Some([unit.x, unit.y]));
    let [x, y] = unit.position.unwrap();
    assert_eq!(
        host.native("GetUnitX", std::slice::from_ref(&second))
            .unwrap(),
        Value::Real(x)
    );
    assert_eq!(
        host.native("GetUnitY", std::slice::from_ref(&second))
            .unwrap(),
        Value::Real(y)
    );
    let rect = host
        .native(
            "Rect",
            &[
                Value::Real(x - 0.125),
                Value::Real(y - 0.125),
                Value::Real(x + 0.125),
                Value::Real(y + 0.125),
            ],
        )
        .unwrap();
    let group = host.native("CreateGroup", &[]).unwrap();
    host.native(
        "GroupEnumUnitsInRect",
        &[group.clone(), rect.clone(), Value::Null],
    )
    .unwrap();
    assert_eq!(
        host.native("FirstOfGroup", std::slice::from_ref(&group))
            .unwrap(),
        second
    );
    assert_eq!(
        host.native("IsUnitInGroup", &[first, group.clone()])
            .unwrap(),
        Value::Bool(false)
    );
    let before = host.memberships();
    assert_eq!(
        host.native("GroupEnumUnitsInRect", &[group.clone(), rect, Value::Null])
            .unwrap(),
        Value::Null
    );
    assert_eq!(host.memberships(), before);
    assert!(
        host.admit_unit_placement(crate::unit_placement::tests::placement(source(), 100))
            .is_err()
    );
}

#[test]
fn failed_creation_and_rectangle_budget_leave_arena_and_members_unchanged() {
    let mut host = position_host(1, 1);
    let first = position_create(&mut host, 32.0, 32.0).unwrap();
    let handles = host.inner().handles();
    assert!(
        position_create(&mut host, 32.0, 32.0)
            .unwrap_err()
            .contains("query budget")
    );
    assert_eq!(host.inner().handles(), handles);
    assert!(
        host.native(
            "CreateDestructable",
            &[
                Value::Int(i32::from_be_bytes(*b"D001")),
                Value::Real(32.0),
                Value::Real(32.0),
                Value::Real(0.0),
                Value::Real(1.0),
                Value::Int(0)
            ]
        )
        .unwrap_err()
        .contains("stationary collision cache")
    );
    assert_eq!(host.inner().handles(), handles);
    let second = position_create(&mut host, 64.0, 64.0).unwrap();
    assert_ne!(first, second);
    let rect = host
        .native(
            "Rect",
            &[
                Value::Real(0.0),
                Value::Real(0.0),
                Value::Real(100.0),
                Value::Real(100.0),
            ],
        )
        .unwrap();
    let group = host.native("CreateGroup", &[]).unwrap();
    let handles = host.inner().handles();
    assert!(
        host.native(
            "GroupEnumUnitsInRect",
            &[group.clone(), rect.clone(), Value::Null]
        )
        .unwrap_err()
        .contains("group membership")
    );
    assert_eq!(host.memberships(), 0);
    assert_eq!(host.inner().handles(), handles);
    assert!(
        host.native(
            "GroupEnumUnitsInRect",
            &[group.clone(), rect, second.clone()]
        )
        .is_err()
    );
    assert_eq!(host.memberships(), 0);
    host.native("GroupAddUnit", &[group.clone(), first])
        .unwrap();
    assert_ne!(host.native("FirstOfGroup", &[group]).unwrap(), second);
}

#[path = "unit_enumeration_tests.rs"]
mod enumeration;

#[path = "unit_invulnerability_tests.rs"]
mod invulnerability;

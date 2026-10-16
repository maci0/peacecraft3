use super::super::{context, slots_rule};
use super::{COMMON, budget};
use crate::map_admission::ParsedMapContext;
use crate::playercolor_host::{EnumHost, Failure, Limits, ObjectHost, TriggerBudget, UnitBudget};
use pcx_objects::editor::ObjectKind;
use pcx_script::{Host, Value};
use pcx_sim::world::{Definition, ObjectCatalog, ResolutionProblem, Source};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

const EXTRAS: &str = "type widget extends agent\ntype unit extends widget\ntype event extends agent\ntype eventid extends handle\ntype unitevent extends eventid\ntype trigger extends agent\ntype triggeraction extends handle\nnative CreateUnit takes player id, integer unitid, real x, real y, real face returns unit\nnative CreateTrigger takes nothing returns trigger\nnative TriggerRegisterUnitEvent takes trigger whichTrigger, unit whichUnit, unitevent whichEvent returns event\nnative TriggerAddAction takes trigger whichTrigger, code actionFunc returns triggeraction\nnative EnableTrigger takes trigger whichTrigger returns nothing\nnative IsTriggerEnabled takes trigger whichTrigger returns boolean\nconstant native ConvertUnitEvent takes integer i returns unitevent\n";

const MAP: &str = "globals\nplayer P = null\nunit U = null\ntrigger G = null\nevent E = null\ntimer T = null\nendglobals\nfunction main takes nothing returns nothing\ncall prepare()\nset E = TriggerRegisterUnitEvent(G, U, SAMPLE)\nendfunction\nfunction prepare takes nothing returns nothing\nset P = Player(4)\nset U = CreateUnit(P, 'hfoo', 1.5, -2.25, 90.0)\nset T = CreateTimer()\nset G = CreateTrigger()\nendfunction\nfunction bare takes nothing returns trigger\nreturn CreateTrigger()\nendfunction\nfunction register_only takes nothing returns event\nreturn TriggerRegisterUnitEvent(G, U, SAMPLE)\nendfunction\nfunction wrong_unit takes nothing returns event\nreturn TriggerRegisterUnitEvent(G, T, SAMPLE)\nendfunction\nfunction wrong_event takes nothing returns event\nreturn TriggerRegisterUnitEvent(G, U, C)\nendfunction\nfunction action takes nothing returns nothing\nendfunction\nfunction add takes nothing returns triggeraction\nreturn TriggerAddAction(G, function action)\nendfunction\nfunction enable takes nothing returns nothing\ncall EnableTrigger(G)\nendfunction\nfunction enabled takes nothing returns boolean\nreturn IsTriggerEnabled(G)\nendfunction\n";

struct Counting {
    hits: Arc<AtomicUsize>,
}

impl ObjectCatalog for Counting {
    fn resolve(&self, rawcode: [u8; 4]) -> Result<Definition, ResolutionProblem> {
        self.hits.fetch_add(1, Ordering::SeqCst);
        if rawcode == *b"hfoo" {
            Ok(Definition {
                rawcode,
                base_rawcode: pcx_objects::fourcc_text(rawcode),
                kind: ObjectKind::Unit,
                fields: BTreeMap::new(),
                modifications: Vec::new(),
            })
        } else {
            Err(ResolutionProblem::MissingRawcode(pcx_objects::fourcc_text(
                rawcode,
            )))
        }
    }
}

fn common_source(extras: &str) -> String {
    let globals = COMMON.find("\nglobals\n").expect("globals");
    let mut common = COMMON.to_string();
    common.insert_str(globals + 1, extras);
    let end = common.find("\nendglobals\n").expect("endglobals");
    common.insert_str(end, "\nconstant unitevent SAMPLE = ConvertUnitEvent(3)\n");
    common
}

fn source() -> Source {
    Source {
        install: "selected-install".into(),
        edition: "selected-edition".into(),
        map: "trigger-admission".into(),
    }
}

fn unit_limits() -> UnitBudget {
    UnitBudget {
        max_records: 2,
        max_retained_fields: 0,
        max_modifications: 0,
    }
}

fn trigger_limits(max_triggers: usize, max_unit_events: usize) -> TriggerBudget {
    TriggerBudget {
        max_triggers,
        max_unit_events,
    }
}

fn enumerated(common: &str, map: &str, max_handles: usize) -> (Arc<ParsedMapContext>, ObjectHost) {
    let ctx = context(300, common, map, "trigger-admission");
    let program = ctx.scripts.shared_program();
    let inner = EnumHost::checked_selected(
        program,
        "trigger-admission",
        &[
            "ConvertPlacement",
            "ConvertPlayerColor",
            "ConvertRacePref",
            "ConvertMapControl",
            "ConvertUnitEvent",
        ],
        Limits {
            max_handles,
            admission_steps: 1000,
            expression_depth: 16,
            expression_work: 1000,
        },
    )
    .unwrap();
    let host = ObjectHost::checked(inner)
        .unwrap()
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap();
    (ctx, host)
}

fn admitted(
    max_handles: usize,
    triggers: usize,
    events: usize,
) -> (Arc<ParsedMapContext>, ObjectHost, Arc<AtomicUsize>) {
    let hits = Arc::new(AtomicUsize::new(0));
    let catalog = Arc::new(Counting {
        hits: Arc::clone(&hits),
    });
    let (ctx, host) = enumerated(&common_source(EXTRAS), MAP, max_handles);
    let host = host
        .with_unit_catalog(catalog, source(), unit_limits())
        .unwrap()
        .with_trigger_admission(trigger_limits(triggers, events))
        .unwrap();
    (ctx, host, hits)
}

#[test]
fn unadmitted_create_trigger_stays_unsupported() {
    let common = common_source(EXTRAS);
    let (ctx, host) = enumerated(&common, MAP, 16);
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host);
    vm.init_globals().unwrap();
    let error = vm.call("bare", &[]).unwrap_err().to_string();
    assert!(error.contains("unsupported"), "{error}");
}

#[test]
fn owned_vm_retains_live_trigger_loaded_unit_event_and_prior_state() {
    let (ctx, host, hits) = admitted(24, 2, 2);
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host);
    vm.init_globals().unwrap();
    let before = vm.host().inner.inner().handles();
    vm.call("main", &[]).unwrap();
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    let Value::Handle(trigger) = vm.global("G") else {
        panic!("trigger");
    };
    let Value::Handle(unit) = vm.global("U") else {
        panic!("unit");
    };
    let Value::Handle(event) = vm.global("E") else {
        panic!("event");
    };
    let Value::Handle(sample) = vm.global("SAMPLE") else {
        panic!("unitevent");
    };
    assert_eq!(vm.host().inner.inner().kind(trigger).unwrap(), "trigger");
    assert_eq!(vm.host().inner.inner().kind(event).unwrap(), "event");
    let registration = vm.host().inner.unit_event_registration(event).unwrap();
    assert_eq!(registration.trigger, trigger);
    assert_eq!(registration.unit, unit);
    assert_eq!(registration.unitevent, sample);
    assert_eq!(registration.enumerand, 3);
    assert_eq!(vm.host().inner.retained_triggers().unwrap(), 1);
    assert_eq!(vm.host().inner.retained_unit_events().unwrap(), 1);
    assert!(vm.host().inner.inner().handles() > before);
    let executed = vm.host().inner.executed_unit(unit).unwrap();
    assert_eq!(executed.definition.rawcode, *b"hfoo");
    assert_eq!(executed.acquire_range, None);
    let handles = vm.host().inner.inner().handles();
    let wrong_unit = vm.call("wrong_unit", &[]).unwrap_err().to_string();
    assert!(wrong_unit.contains("another family"), "{wrong_unit}");
    assert!(wrong_unit.contains("trigger-admission"), "{wrong_unit}");
    let wrong_event = vm.call("wrong_event", &[]).unwrap_err().to_string();
    assert!(wrong_event.contains("another family"), "{wrong_event}");
    let unknown = vm
        .host()
        .native(
            "TriggerRegisterUnitEvent",
            &[
                Value::Handle(trigger),
                Value::Handle(unit),
                Value::Handle(pcx_script::HandleId::new(
                    std::num::NonZeroU64::new(9_999).unwrap(),
                )),
            ],
        )
        .unwrap_err();
    assert!(unknown.contains("unknown handle identity"), "{unknown}");
    let extra_args = vm
        .host()
        .native("CreateTrigger", &[Value::Int(1)])
        .unwrap_err();
    assert!(
        extra_args.contains("arguments do not match the trigger native declaration"),
        "{extra_args}"
    );
    assert_eq!(vm.host().inner.inner().handles(), handles);
    assert_eq!(vm.host().inner.retained_triggers().unwrap(), 1);
    assert_eq!(vm.host().inner.retained_unit_events().unwrap(), 1);
    let action = vm.call("add", &[]).unwrap_err().to_string();
    assert!(action.contains("unsupported"), "{action}");
    let enable = vm.call("enable", &[]).unwrap_err().to_string();
    assert!(enable.contains("unsupported"), "{enable}");
    let enabled = vm.call("enabled", &[]).unwrap_err().to_string();
    assert!(enabled.contains("unsupported"), "{enabled}");
    assert_eq!(vm.host().inner.inner().handles(), handles);
}

#[test]
fn trigger_budgets_fail_before_allocation() {
    let (ctx, host, _) = admitted(24, 0, 0);
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host);
    vm.init_globals().unwrap();
    let handles = vm.host().inner.inner().handles();
    let error = vm.call("bare", &[]).unwrap_err().to_string();
    assert!(error.contains("trigger creation"), "{error}");
    assert_eq!(vm.host().inner.inner().handles(), handles);
    assert_eq!(vm.host().inner.retained_triggers().unwrap(), 0);
    let (ctx, host, _) = admitted(24, 1, 0);
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host);
    vm.init_globals().unwrap();
    vm.call("prepare", &[]).unwrap();
    let handles = vm.host().inner.inner().handles();
    let error = vm.call("register_only", &[]).unwrap_err().to_string();
    assert!(error.contains("unit event registration"), "{error}");
    assert_eq!(vm.host().inner.retained_triggers().unwrap(), 1);
    assert_eq!(vm.host().inner.retained_unit_events().unwrap(), 0);
    assert_eq!(vm.host().inner.inner().handles(), handles);
}

#[test]
fn trigger_and_event_refuse_a_full_shared_arena_without_retaining_records() {
    let (ctx, host, _) = admitted(24, 2, 2);
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host);
    vm.init_globals().unwrap();
    let initial = vm.host().inner.inner().handles();
    vm.call("prepare", &[]).unwrap();
    let prepared = vm.host().inner.inner().handles();

    let (ctx, host, _) = admitted(initial, 2, 2);
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host);
    vm.init_globals().unwrap();
    let error = vm.call("bare", &[]).unwrap_err().to_string();
    assert!(error.contains("capacity"), "{error}");
    assert_eq!(vm.host().inner.inner().handles(), initial);
    assert_eq!(vm.host().inner.retained_triggers().unwrap(), 0);

    let (ctx, host, _) = admitted(prepared, 2, 2);
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host);
    vm.init_globals().unwrap();
    vm.call("prepare", &[]).unwrap();
    let error = vm.call("register_only", &[]).unwrap_err().to_string();
    assert!(error.contains("capacity"), "{error}");
    assert_eq!(vm.host().inner.inner().handles(), prepared);
    assert_eq!(vm.host().inner.retained_triggers().unwrap(), 1);
    assert_eq!(vm.host().inner.retained_unit_events().unwrap(), 0);
}

#[test]
fn trigger_signature_and_source_fail_before_admission() {
    let bad = EXTRAS.replace(
        "native CreateTrigger takes nothing returns trigger\n",
        "native CreateTrigger takes integer i returns trigger\n",
    );
    let (_ctx, host) = enumerated(
        &common_source(&bad),
        "function main takes nothing returns nothing\nendfunction\n",
        16,
    );
    let error = match host.with_trigger_admission(trigger_limits(1, 1)) {
        Err(error) => error,
        Ok(_) => panic!("bad CreateTrigger signature was admitted"),
    };
    assert_eq!(error.failure, Failure::Signature);
    assert_eq!(error.native, "CreateTrigger");
    assert_eq!(error.declaration_source, "synthetic/common.j");
    assert_ne!(error.declaration_span, pcx_script::span::Span::EMPTY);

    let map = pcx_script::parse_script("").unwrap();
    let primary = pcx_script::parse_script(&common_source(
        "type widget extends agent\ntype unit extends widget\ntype event extends agent\ntype eventid extends handle\ntype unitevent extends eventid\ntype trigger extends agent\nnative CreateUnit takes player id, integer unitid, real x, real y, real face returns unit\nnative CreateTrigger takes nothing returns trigger\nconstant native ConvertUnitEvent takes integer i returns unitevent\n",
    ))
    .unwrap();
    let other = pcx_script::parse_script(
        "native TriggerRegisterUnitEvent takes trigger whichTrigger, unit whichUnit, unitevent whichEvent returns event\n",
    )
    .unwrap();
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
        program,
        "split-source",
        &[
            "ConvertPlacement",
            "ConvertPlayerColor",
            "ConvertRacePref",
            "ConvertMapControl",
            "ConvertUnitEvent",
        ],
        Limits {
            max_handles: 16,
            admission_steps: 1000,
            expression_depth: 16,
            expression_work: 1000,
        },
    )
    .unwrap();
    let error = match ObjectHost::checked(inner)
        .unwrap()
        .with_trigger_admission(trigger_limits(1, 1))
    {
        Err(error) => error,
        Ok(_) => panic!("foreign registration source was admitted"),
    };
    assert_eq!(error.failure, Failure::Source);
    assert_eq!(error.native, "TriggerRegisterUnitEvent");
    assert_eq!(error.declaration_source, "synthetic/other.j");
}

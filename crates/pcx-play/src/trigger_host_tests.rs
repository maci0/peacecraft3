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

const EXTRAS: &str = "type widget extends agent\ntype unit extends widget\ntype event extends agent\ntype eventid extends handle\ntype unitevent extends eventid\ntype trigger extends agent\ntype triggeraction extends handle\nnative CreateUnit takes player id, integer unitid, real x, real y, real face returns unit\nnative CreateTrigger takes nothing returns trigger\nnative TriggerRegisterUnitEvent takes trigger whichTrigger, unit whichUnit, unitevent whichEvent returns event\nnative TriggerAddAction takes trigger whichTrigger, code actionFunc returns triggeraction\nnative EnableTrigger takes trigger whichTrigger returns nothing\nnative DisableTrigger takes trigger whichTrigger returns nothing\nnative IsTriggerEnabled takes trigger whichTrigger returns boolean\nconstant native ConvertUnitEvent takes integer i returns unitevent\n";

const MAP: &str = "globals\nplayer P = null\nunit U = null\ntrigger G = null\nevent E = null\ntimer T = null\ninteger hits = 0\ntriggeraction A = null\ntriggeraction B = null\nendglobals\nfunction main takes nothing returns nothing\ncall prepare()\nset E = TriggerRegisterUnitEvent(G, U, SAMPLE)\nendfunction\nfunction prepare takes nothing returns nothing\nset P = Player(4)\nset U = CreateUnit(P, 'hfoo', 1.5, -2.25, 90.0)\nset T = CreateTimer()\nset G = CreateTrigger()\nendfunction\nfunction bare takes nothing returns trigger\nreturn CreateTrigger()\nendfunction\nfunction register_only takes nothing returns event\nreturn TriggerRegisterUnitEvent(G, U, SAMPLE)\nendfunction\nfunction wrong_unit takes nothing returns event\nreturn TriggerRegisterUnitEvent(G, T, SAMPLE)\nendfunction\nfunction wrong_event takes nothing returns event\nreturn TriggerRegisterUnitEvent(G, U, C)\nendfunction\nfunction action takes nothing returns nothing\nset hits = hits + 1\nendfunction\nfunction other takes nothing returns nothing\nendfunction\nfunction bad takes integer i returns nothing\nendfunction\nfunction add takes nothing returns triggeraction\nset A = TriggerAddAction(G, function action)\nreturn A\nendfunction\nfunction add_other takes nothing returns triggeraction\nset B = TriggerAddAction(G, function other)\nreturn B\nendfunction\nfunction add_bad takes nothing returns triggeraction\nreturn TriggerAddAction(G, function bad)\nendfunction\nfunction add_timer takes nothing returns triggeraction\nreturn TriggerAddAction(T, function action)\nendfunction\nfunction enable takes nothing returns nothing\ncall EnableTrigger(G)\nendfunction\nfunction enabled takes nothing returns boolean\nreturn IsTriggerEnabled(G)\nendfunction\nfunction disable takes nothing returns nothing\ncall DisableTrigger(G)\nendfunction\n";

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
        max_actions: 4,
    }
}

fn action_limits(max_triggers: usize, max_unit_events: usize, max_actions: usize) -> TriggerBudget {
    TriggerBudget {
        max_triggers,
        max_unit_events,
        max_actions,
    }
}

fn enum_input(common: &str, map: &str, max_handles: usize) -> (Arc<ParsedMapContext>, EnumHost) {
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
    (ctx, inner)
}

fn enumerated(common: &str, map: &str, max_handles: usize) -> (Arc<ParsedMapContext>, ObjectHost) {
    let (ctx, inner) = enum_input(common, map, max_handles);
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
    admitted_budget(max_handles, trigger_limits(triggers, events))
}

fn admitted_budget(
    max_handles: usize,
    limits: TriggerBudget,
) -> (Arc<ParsedMapContext>, ObjectHost, Arc<AtomicUsize>) {
    let hits = Arc::new(AtomicUsize::new(0));
    let catalog = Arc::new(Counting {
        hits: Arc::clone(&hits),
    });
    let (ctx, host) = enumerated(&common_source(EXTRAS), MAP, max_handles);
    let host = host
        .with_unit_catalog(catalog, source(), unit_limits())
        .unwrap()
        .with_trigger_admission(limits)
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
    assert_eq!(vm.host().inner.trigger_enabled(trigger).unwrap(), None);
    let enabled = vm.call("enabled", &[]).unwrap_err().to_string();
    assert!(enabled.contains("not assigned"), "{enabled}");
    vm.call("enable", &[]).unwrap();
    assert_eq!(vm.call("enabled", &[]).unwrap(), Value::Bool(true));
    vm.call("disable", &[]).unwrap();
    assert_eq!(vm.call("enabled", &[]).unwrap(), Value::Bool(false));
    assert_eq!(
        vm.host().inner.unit_event_registration(event).unwrap(),
        registration
    );
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

#[test]
fn owned_vm_registers_callback_without_running_it_until_an_explicit_call() {
    let (ctx, host, _) = admitted_budget(24, action_limits(2, 2, 2));
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host);
    vm.init_globals().unwrap();
    vm.call("prepare", &[]).unwrap();
    let Value::Handle(trigger) = vm.global("G") else {
        panic!("trigger");
    };
    let before = vm.host().inner.inner().handles();
    vm.call("add_other", &[]).unwrap();
    vm.call("add", &[]).unwrap();
    assert_eq!(vm.global("hits"), Value::Int(0));
    let Value::Handle(first) = vm.global("B") else {
        panic!("first registration");
    };
    let Value::Handle(second) = vm.global("A") else {
        panic!("second registration");
    };
    assert!(first.token() < second.token());
    let (other_record, action_record) = {
        let host = &vm.host().inner;
        let other = host.trigger_action(first).unwrap();
        let action = host.trigger_action(second).unwrap();
        (
            (
                other.function,
                other.source.to_string(),
                other.span,
                other.trigger,
            ),
            (
                action.function,
                action.source.to_string(),
                action.span,
                action.trigger,
            ),
        )
    };
    let program = ctx.scripts.shared_program();
    let other_fn = program.function("other").unwrap();
    let action_fn = program.function("action").unwrap();
    let other_id = program.function_id("other").unwrap();
    let action_id = program.function_id("action").unwrap();
    assert!(action_id.index() < other_id.index());
    assert_eq!(other_record.0, other_id);
    assert_eq!(action_record.0, action_id);
    assert_eq!(other_record.3, trigger);
    assert_eq!(action_record.3, trigger);
    assert_eq!(action_record.1, action_fn.source);
    assert_eq!(action_record.2, action_fn.span);
    assert_eq!(other_record.1, other_fn.source);
    assert_eq!(other_record.2, other_fn.span);
    assert_ne!(other_record.2, action_record.2);
    assert_eq!(
        vm.host().inner.inner().kind(first).unwrap(),
        "triggeraction"
    );
    assert_eq!(vm.host().inner.inner().handles(), before + 2);
    vm.call("action", &[]).unwrap();
    assert_eq!(vm.global("hits"), Value::Int(1));
    let handles = vm.host().inner.inner().handles();
    let wrong = vm.call("add_timer", &[]).unwrap_err().to_string();
    assert!(wrong.contains("another family"), "{wrong}");
    let bad = vm.call("add_bad", &[]).unwrap_err().to_string();
    assert!(
        bad.contains("callback takes nothing returns nothing"),
        "{bad}"
    );
    let missing = vm
        .host()
        .native(
            "TriggerAddAction",
            &[
                Value::Handle(trigger),
                Value::Code(pcx_script::value::FunctionId(u32::MAX)),
            ],
        )
        .unwrap_err();
    assert!(
        missing.contains("callback is not a loaded function"),
        "{missing}"
    );
    assert_eq!(vm.host().inner.inner().handles(), handles);
    assert_eq!(vm.global("hits"), Value::Int(1));
    vm.call("disable", &[]).unwrap();
    vm.call("enable", &[]).unwrap();
    assert_eq!(vm.call("enabled", &[]).unwrap(), Value::Bool(true));
    assert_eq!(vm.global("hits"), Value::Int(1));
    assert_eq!(
        vm.host().inner.trigger_action(first).unwrap().trigger,
        trigger
    );
    assert_eq!(vm.host().inner.inner().handles(), handles);
}

#[test]
fn trigger_action_budgets_and_full_arena_preserve_prior_records() {
    let (ctx, host, _) = admitted_budget(24, action_limits(2, 2, 0));
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host);
    vm.init_globals().unwrap();
    vm.call("prepare", &[]).unwrap();
    let handles = vm.host().inner.inner().handles();
    let error = vm.call("add", &[]).unwrap_err().to_string();
    assert!(error.contains("trigger action"), "{error}");
    assert_eq!(vm.global("hits"), Value::Int(0));
    assert_eq!(vm.global("A"), Value::Null);
    assert_eq!(vm.host().inner.inner().handles(), handles);

    let (ctx, host, _) = admitted_budget(24, action_limits(2, 2, 2));
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host);
    vm.init_globals().unwrap();
    vm.call("prepare", &[]).unwrap();
    let prepared = vm.host().inner.inner().handles();
    let (ctx, host, _) = admitted_budget(prepared, action_limits(2, 2, 2));
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host);
    vm.init_globals().unwrap();
    vm.call("prepare", &[]).unwrap();
    let error = vm.call("add", &[]).unwrap_err().to_string();
    assert!(error.contains("capacity"), "{error}");
    assert_eq!(vm.host().inner.inner().handles(), prepared);
    assert_eq!(vm.global("hits"), Value::Int(0));
    assert_eq!(vm.global("A"), Value::Null);
}

#[test]
fn a_program_without_action_declarations_keeps_trigger_creation() {
    let common = common_source(&EXTRAS.replace(
        "native TriggerAddAction takes trigger whichTrigger, code actionFunc returns triggeraction\n", "",
    ).replace("type triggeraction extends handle\n", ""));
    let map = "function main takes nothing returns trigger\nreturn CreateTrigger()\nendfunction\n";
    let (ctx, host) = enumerated(&common, map, 16);
    let host = host.with_trigger_admission(action_limits(1, 0, 0)).unwrap();
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host);
    let value = vm.call("main", &[]).unwrap();
    let Value::Handle(id) = value else {
        panic!("trigger")
    };
    assert_eq!(vm.host().inner.inner().kind(id).unwrap(), "trigger");
    let count = vm.host().inner.inner().handles();
    assert!(
        vm.host()
            .native("TriggerAddAction", &[value])
            .unwrap_err()
            .contains("unsupported")
    );
    assert_eq!(vm.host().inner.inner().handles(), count);
}

#[test]
fn trigger_action_signature_and_source_fail_before_admission() {
    let bad = EXTRAS.replace(
        "native TriggerAddAction takes trigger whichTrigger, code actionFunc returns triggeraction\n",
        "native TriggerAddAction takes trigger whichTrigger returns nothing\n",
    );
    let (_ctx, host) = enumerated(
        &common_source(&bad),
        "function main takes nothing returns nothing\nendfunction\n",
        16,
    );
    let error = match host.with_trigger_admission(action_limits(1, 1, 1)) {
        Err(error) => error,
        Ok(_) => panic!("bad TriggerAddAction signature was admitted"),
    };
    assert_eq!(error.failure, Failure::Signature);
    assert_eq!(error.native, "TriggerAddAction");
    assert_eq!(error.declaration_source, "synthetic/common.j");
    assert_ne!(error.declaration_span, pcx_script::span::Span::EMPTY);

    let map = pcx_script::parse_script("").unwrap();
    let primary = pcx_script::parse_script(&common_source(&EXTRAS.replace(
        "native TriggerAddAction takes trigger whichTrigger, code actionFunc returns triggeraction\n",
        "",
    )))
    .unwrap();
    let other = pcx_script::parse_script(
        "native TriggerAddAction takes trigger whichTrigger, code actionFunc returns triggeraction\n",
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
        "split-action",
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
        .with_trigger_admission(action_limits(1, 1, 1))
    {
        Err(error) => error,
        Ok(_) => panic!("foreign action source was admitted"),
    };
    assert_eq!(error.failure, Failure::Source);
    assert_eq!(error.native, "TriggerAddAction");
    assert_eq!(error.declaration_source, "synthetic/other.j");
}

#[path = "timer_event_tests.rs"]
mod timer_events;

#[path = "game_state_event_tests.rs"]
mod game_state_events;

#[path = "player_unit_event_tests.rs"]
mod player_unit_event_tests;

#[path = "player_event_tests.rs"]
mod player_event_tests;

#[path = "player_chat_event_tests.rs"]
mod player_chat_event_tests;

#[path = "player_state_event_tests.rs"]
mod player_state_event_tests;

#[path = "trigger_condition_tests.rs"]
mod trigger_conditions;

#[test]
fn trigger_state_is_explicit_per_identity_and_bad_calls_preserve_it() {
    let (ctx, mut inner) = enum_input(&common_source(EXTRAS), MAP, 24);
    let reserved = inner.reserve_identity("trigger").unwrap();
    let host = ObjectHost::checked(inner)
        .unwrap()
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_unit_catalog(
            Arc::new(Counting {
                hits: Arc::new(AtomicUsize::new(0)),
            }),
            source(),
            unit_limits(),
        )
        .unwrap()
        .with_trigger_admission(trigger_limits(2, 0))
        .unwrap();
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host);
    vm.init_globals().unwrap();
    vm.call("prepare", &[]).unwrap();
    let Value::Handle(first) = vm.global("G") else {
        panic!("trigger")
    };
    let Value::Handle(second) = vm.call("bare", &[]).unwrap() else {
        panic!("trigger")
    };
    let handles = vm.host().inner.inner().handles();
    assert_eq!(vm.host().inner.trigger_enabled(first).unwrap(), None);
    assert_eq!(vm.host().inner.trigger_enabled(second).unwrap(), None);
    vm.call("disable", &[]).unwrap();
    vm.call("disable", &[]).unwrap();
    assert_eq!(vm.call("enabled", &[]).unwrap(), Value::Bool(false));
    assert_eq!(vm.host().inner.trigger_enabled(second).unwrap(), None);
    vm.host()
        .native("EnableTrigger", &[Value::Handle(second)])
        .unwrap();
    assert_eq!(vm.host().inner.trigger_enabled(second).unwrap(), Some(true));
    let timer = vm.global("T");
    let unknown = Value::Handle(pcx_script::HandleId::new(
        std::num::NonZeroU64::new(9_999).unwrap(),
    ));
    for name in ["EnableTrigger", "DisableTrigger", "IsTriggerEnabled"] {
        for args in [
            vec![],
            vec![Value::Null],
            vec![timer.clone()],
            vec![unknown.clone()],
            vec![Value::Handle(reserved)],
            vec![Value::Handle(first), Value::Bool(true)],
        ] {
            let error = vm.host().native(name, &args).unwrap_err();
            assert!(error.contains(name), "{error}");
            assert_eq!(vm.host().inner.trigger_enabled(first).unwrap(), Some(false));
            assert_eq!(vm.host().inner.trigger_enabled(second).unwrap(), Some(true));
            assert_eq!(vm.host().inner.inner().handles(), handles);
        }
    }
    vm.call("enable", &[]).unwrap();
    vm.call("enable", &[]).unwrap();
    assert_eq!(vm.call("enabled", &[]).unwrap(), Value::Bool(true));
    assert_eq!(vm.global("hits"), Value::Int(0));
}

#[test]
fn trigger_state_contracts_are_selected_and_optional() {
    let map = "function main takes nothing returns trigger\nreturn CreateTrigger()\nendfunction\n";
    let mut without = EXTRAS.to_string();
    for (name, result) in [
        ("EnableTrigger", "nothing"),
        ("DisableTrigger", "nothing"),
        ("IsTriggerEnabled", "boolean"),
    ] {
        let declaration = format!("native {name} takes trigger whichTrigger returns {result}\n");
        for replacement in [
            format!("constant {declaration}"),
            declaration.replace("trigger whichTrigger", "integer whichTrigger"),
            declaration.replace(&format!("returns {result}"), "returns trigger"),
        ] {
            let extras = EXTRAS.replace(&declaration, &replacement);
            let (_, host) = enumerated(&common_source(&extras), map, 16);
            let error = match host.with_trigger_admission(trigger_limits(1, 0)) {
                Err(e) => e,
                Ok(_) => panic!("invalid {name} admitted"),
            };
            assert_eq!(error.native, name);
            assert_eq!(error.failure, Failure::Signature);
        }
        let extras = EXTRAS.replace(&declaration, "");
        let (_, host) = enumerated(&common_source(&extras), &format!("{declaration}{map}"), 16);
        let error = match host.with_trigger_admission(trigger_limits(1, 0)) {
            Err(e) => e,
            Ok(_) => panic!("foreign {name} admitted"),
        };
        assert_eq!(error.native, name);
        assert_eq!(error.failure, Failure::Source);
        without = without.replace(&declaration, "");
    }
    let (ctx, host) = enumerated(&common_source(&without), map, 16);
    let host = host.with_trigger_admission(trigger_limits(1, 0)).unwrap();
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host);
    let trigger = vm.call("main", &[]).unwrap();
    for name in ["EnableTrigger", "DisableTrigger", "IsTriggerEnabled"] {
        assert!(
            vm.host()
                .native(name, std::slice::from_ref(&trigger))
                .unwrap_err()
                .contains("unsupported")
        );
    }
}

#[path = "death_event_tests.rs"]
mod death_event_tests;

#[path = "range_event_tests.rs"]
mod range_event_tests;

#[test]
fn action_cleanup_admission_checks_optional_contracts_and_sources() {
    let map = "function main takes nothing returns nothing\nendfunction\n";
    let declarations = "\
native TriggerRemoveAction takes trigger t, triggeraction a returns nothing
native TriggerClearActions takes trigger t returns nothing
";
    let common = common_source(&format!("{EXTRAS}{declarations}"));
    for bad in [
        common.replace(
            "native TriggerClearActions",
            "constant native TriggerClearActions",
        ),
        common.replace(
            "triggeraction a returns nothing",
            "trigger a returns nothing",
        ),
        common.replace(
            "native TriggerClearActions takes trigger t returns nothing",
            "native TriggerClearActions takes trigger t returns integer",
        ),
    ] {
        let (_, h) = enumerated(&bad, map, 32);
        assert!(h.with_trigger_admission(action_limits(2, 0, 2)).is_err());
    }
    for declaration in declarations.lines() {
        let (_, h) = enumerated(
            &common.replace(declaration, ""),
            &format!("{declaration}\n{map}"),
            32,
        );
        let Err(error) = h.with_trigger_admission(action_limits(2, 0, 2)) else {
            panic!("foreign cleanup source admitted")
        };
        assert_eq!(error.failure, Failure::Source);
    }
    let common = common_source(EXTRAS);
    let (_, mut h) = enumerated(&common, map, 32);
    h = h.with_trigger_admission(action_limits(2, 0, 2)).unwrap();
    for name in ["TriggerRemoveAction", "TriggerClearActions"] {
        assert!(h.native(name, &[]).unwrap_err().contains("unsupported"));
    }
}

pub(super) const CONDITION_SESSION_NEUTRAL_DECLARATIONS: &str =
    "native GetPlayerNeutralPassive takes nothing returns integer
native GetPlayerNeutralAggressive takes nothing returns integer
native GetBJMaxPlayers takes nothing returns integer
native GetBJPlayerNeutralVictim takes nothing returns integer
native GetBJPlayerNeutralExtra takes nothing returns integer
";

pub(super) fn publish_condition_session(
    ctx: Arc<ParsedMapContext>,
    host: ObjectHost,
) -> crate::map_script_session::MapScriptSession<crate::map_startup_host::MapStartupHost> {
    use crate::map_script_session::MapScriptSession;
    use crate::map_startup_host::MapStartupHost;
    use crate::neutral_player::{SelectedMapNeutralHost, tests as neutral};
    let world = SelectedMapNeutralHost::try_new(ctx.clone(), neutral::slots_rule(301, -3, 9), host)
        .unwrap();
    let world =
        SelectedMapNeutralHost::try_new(ctx.clone(), neutral::extra_rule(301, -3, 9), world)
            .unwrap();
    let world =
        SelectedMapNeutralHost::try_new(ctx.clone(), neutral::victim_rule(301, -3, 9), world)
            .unwrap();
    let world = SelectedMapNeutralHost::try_new(
        ctx.clone(),
        neutral::bj_max_players_rule(301, -3, 9),
        world,
    )
    .unwrap();
    let world = SelectedMapNeutralHost::try_new(ctx.clone(), neutral::aggressive_rules(301), world)
        .unwrap();
    let world = SelectedMapNeutralHost::try_new(ctx.clone(), neutral::rules(), world).unwrap();
    let main = ctx.scripts.program().function_id("main").unwrap();
    let mut session = MapScriptSession::<MapStartupHost>::default();
    session
        .replace(
            ctx,
            |_| Ok::<_, std::convert::Infallible>(MapStartupHost::timer_session_fixture(world)),
            &[main],
        )
        .unwrap();
    session
}

pub(super) const CONDITION_MUTATION_FUNCTIONS: &str = r"
globals
trigger T = null
triggercondition pending = null
triggercondition current = null
boolean nested = false
integer trace = 0
integer actions = 0
endglobals
function yes takes nothing returns boolean
set trace = trace * 10 + 7
return true
endfunction
function no takes nothing returns boolean
set trace = trace * 10 + 5
return false
endfunction
function clear takes nothing returns boolean
set trace = trace * 10 + 4
call TriggerClearConditions(T)
return true
endfunction
function append takes nothing returns boolean
set trace = trace * 10 + 9
call TriggerAddCondition(T, Condition(function yes))
return true
endfunction
function removeNext takes nothing returns boolean
set trace = trace * 10 + 6
call TriggerRemoveCondition(T, pending)
return true
endfunction
function removeSelf takes nothing returns boolean
set trace = trace * 10 + 8
call TriggerRemoveCondition(T, current)
return true
endfunction
function clearAppend takes nothing returns boolean
set trace = trace * 10 + 3
call TriggerClearConditions(T)
call TriggerAddCondition(T, Condition(function yes))
return true
endfunction
function reenter takes nothing returns boolean
local boolean result
set trace = trace * 10 + 1
if not nested then
set nested = true
set result = TriggerEvaluate(T)
set nested = false
endif
return true
endfunction
function extraAction takes nothing returns nothing
set actions = actions + 10
endfunction
function clearActions takes nothing returns boolean
set trace = trace * 10 + 2
call TriggerClearActions(T)
return true
endfunction
function appendAction takes nothing returns boolean
set trace = trace * 10 + 3
call TriggerAddAction(T, function extraAction)
return true
endfunction
function action takes nothing returns nothing
set actions = actions + 1
endfunction
";

pub(super) const CONDITION_MUTATION_CASES: &[(&str, i32, i32)] = &[
    ("", 0, 1),
    ("call TriggerAddCondition(T, Condition(function yes))", 7, 1),
    (
        "call TriggerAddCondition(T, Condition(function no))\ncall TriggerAddCondition(T, Condition(function yes))",
        57,
        0,
    ),
    (
        "call TriggerAddCondition(T, Condition(function clear))\ncall TriggerAddCondition(T, Condition(function no))",
        4,
        0,
    ),
    (
        "call TriggerAddCondition(T, Condition(function append))\ncall TriggerAddCondition(T, Condition(function no))",
        957,
        0,
    ),
    (
        "call TriggerAddCondition(T, Condition(function append))",
        9,
        1,
    ),
    (
        "call TriggerAddCondition(T, Condition(function clearActions))",
        2,
        0,
    ),
    (
        "call TriggerAddCondition(T, Condition(function appendAction))",
        3,
        11,
    ),
    (
        "set current = TriggerAddCondition(T, Condition(function removeSelf))\ncall TriggerAddCondition(T, Condition(function yes))",
        87,
        1,
    ),
    (
        "call TriggerAddCondition(T, Condition(function removeNext))\nset pending = TriggerAddCondition(T, Condition(function no))\ncall TriggerAddCondition(T, Condition(function yes))",
        6,
        0,
    ),
    (
        "call TriggerAddCondition(T, Condition(function removeNext))\ncall TriggerAddCondition(T, Condition(function yes))\nset pending = TriggerAddCondition(T, Condition(function no))",
        67,
        1,
    ),
    (
        "call TriggerAddCondition(T, Condition(function clearAppend))\ncall TriggerAddCondition(T, Condition(function no))",
        3,
        0,
    ),
    (
        "call TriggerAddCondition(T, Condition(function reenter))\ncall TriggerAddCondition(T, Condition(function yes))",
        1177,
        1,
    ),
];

#[test]
fn published_timer_conditions_follow_live_registrations_and_select_actions_afterward() {
    use crate::playercolor_host::{ConditionBudget, TriggerRuntimeInput};

    let extras = format!(
        "{EXTRAS}{CONDITION_SESSION_NEUTRAL_DECLARATIONS}{}",
        r"
type boolexpr extends agent
type conditionfunc extends boolexpr
type triggercondition extends agent
type gameevent extends eventid
constant native ConvertGameEvent takes integer i returns gameevent
native Condition takes code f returns conditionfunc
native TriggerAddCondition takes trigger t, boolexpr c returns triggercondition
native TriggerClearConditions takes trigger t returns nothing
native TriggerRemoveCondition takes trigger t, triggercondition c returns nothing
native TriggerEvaluate takes trigger t returns boolean
native TriggerClearActions takes trigger t returns nothing
native DestroyTrigger takes trigger t returns nothing
native TriggerRegisterTimerExpireEvent takes trigger t, timer which returns event
native TriggerRegisterTimerEvent takes trigger t, real timeout, boolean periodic returns event
constant native GetTriggeringTrigger takes nothing returns trigger
constant native GetTriggerEventId takes nothing returns eventid
native TimerStart takes timer t, real timeout, boolean periodic, code handler returns nothing
native TimerGetElapsed takes timer t returns real
native TimerGetRemaining takes timer t returns real
native TimerGetTimeout takes timer t returns real
native GetExpiredTimer takes nothing returns timer
"
    );
    let common = common_source(&extras).replace(
        "\nendglobals\n",
        "\nconstant gameevent TEST_TIMER = ConvertGameEvent(15)\nendglobals\n",
    );
    let functions = CONDITION_MUTATION_FUNCTIONS;
    for &(setup, trace, actions) in CONDITION_MUTATION_CASES {
        let map = format!(
            "{functions}\nfunction main takes nothing returns nothing\nset T = CreateTrigger()\ncall TriggerAddAction(T, function action)\n{setup}\ncall TriggerRegisterTimerEvent(T, 0.03125, false)\nendfunction\n"
        );
        let ctx = context(300, &common, &map, "synthetic timer conditions");
        let inner = EnumHost::checked_selected(
            ctx.scripts.shared_program(),
            "synthetic timer conditions",
            &[
                "ConvertPlacement",
                "ConvertPlayerColor",
                "ConvertRacePref",
                "ConvertMapControl",
                "ConvertUnitEvent",
                "ConvertGameEvent",
            ],
            Limits {
                max_handles: 128,
                admission_steps: 1000,
                expression_depth: 64,
                expression_work: 4096,
            },
        )
        .unwrap();
        let host = ObjectHost::checked(inner)
            .unwrap()
            .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
            .unwrap()
            .with_trigger_admission(action_limits(8, 0, 8))
            .unwrap()
            .with_conditions(ConditionBudget { max_records: 16 })
            .unwrap()
            .with_trigger_conditions(16)
            .unwrap()
            .with_timer_events(8)
            .unwrap()
            .with_timer_runtime(0.)
            .unwrap()
            .with_trigger_runtime(TriggerRuntimeInput {
                initial_enabled: true,
                timer_event_global: "TEST_TIMER".into(),
                max_timed_events: 8,
                max_parked_threads: 0,
            })
            .unwrap();
        let mut session = publish_condition_session(ctx, host);
        assert_eq!(session.advance_timers(0.125, 8).unwrap(), 1, "{setup}");
        assert_eq!(session.global("trace"), Some(Value::Int(trace)), "{setup}");
        assert_eq!(
            session.global("actions"),
            Some(Value::Int(actions)),
            "{setup}"
        );
        assert_eq!(
            session
                .host()
                .unwrap()
                .objects_mut()
                .native("GetTriggeringTrigger", &[])
                .unwrap(),
            Value::Null
        );
    }
}

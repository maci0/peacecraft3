use super::super::destructable_creation::life_catalog;
use super::{
    Counting, EXTRAS, budget, common_source, enum_input, enumerated, slots_rule, source,
    trigger_limits, unit_limits,
};
use crate::map_admission::ParsedMapContext;
use crate::playercolor_host::{
    ConditionBudget, DeathEventRegistration, DestructableBudget, EnumHost, Failure, FilterBudget,
    Limits, ObjectHost, PlayerStateRequestBudget, TriggerRuntimeInput,
};
use crate::scripts::OwnedLocalizedHost;
use pcx_script::OwnedVm;
use pcx_script::{HandleId, Host, Value};
use std::sync::{Arc, atomic::AtomicUsize};

const REGISTER: &str = "TriggerRegisterDeathEvent";
const DECLARATION: &str =
    "native TriggerRegisterDeathEvent takes trigger t, widget w returns event\n";
const WIDGET_EVENT: &str = "\
type widgetevent extends eventid
constant native ConvertWidgetEvent takes integer i returns widgetevent
";
const EXTRA: &str = "\
type destructable extends widget
native CreateDestructable takes integer id, real x, real y, real f, real s, integer v returns destructable
native CreateDeadDestructableZ takes integer id, real x, real y, real z, real f, real s, integer v returns destructable
";
const EMPTY: &str = "function empty takes nothing returns nothing\nendfunction\n";
const MAP: &str = "\
globals
trigger T=null
unit U=null
destructable D=null
event A=null
event B=null
event C=null
integer hits=0
endglobals
function action takes nothing returns nothing
set hits=hits+1
endfunction
function main takes nothing returns nothing
set T=CreateTrigger()
call DisableTrigger(T)
call TriggerAddAction(T,function action)
set U=CreateUnit(Player(4),'hfoo',1.0,2.0,3.0)
set D=CreateDeadDestructableZ('Zxyz',4.0,5.0,6.0,7.0,1.0,2)
set A=TriggerRegisterDeathEvent(T,U)
set B=TriggerRegisterDeathEvent(T,D)
set C=TriggerRegisterDeathEvent(T,D)
endfunction
";
fn common() -> String {
    let mut common = common_source(&format!("{EXTRAS}{EXTRA}{WIDGET_EVENT}{DECLARATION}"));
    let end = common.find("\nendglobals\n").expect("endglobals");
    common.insert_str(
        end,
        "\nconstant widgetevent EVENT_WIDGET_DEATH=ConvertWidgetEvent(862)\n",
    );
    common
}
fn death_enum_input(
    common: &str,
    map: &str,
    max_handles: usize,
) -> (Arc<ParsedMapContext>, EnumHost) {
    let ctx = super::super::context(300, common, map, "trigger-admission");
    let inner = EnumHost::checked_selected(
        ctx.scripts.shared_program(),
        "trigger-admission",
        &[
            "ConvertPlacement",
            "ConvertPlayerColor",
            "ConvertRacePref",
            "ConvertMapControl",
            "ConvertUnitEvent",
            "ConvertWidgetEvent",
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
fn objects(h: ObjectHost) -> ObjectHost {
    h.with_unit_catalog(
        Arc::new(Counting {
            hits: Arc::new(AtomicUsize::new(0)),
        }),
        source(),
        unit_limits(),
    )
    .unwrap()
    .with_destructables(DestructableBudget { max_records: 4 })
    .unwrap()
    .with_trigger_admission(trigger_limits(4, 0))
    .unwrap()
}
fn host(common: &str, map: &str) -> (Arc<ParsedMapContext>, ObjectHost) {
    let (ctx, inner) = death_enum_input(common, map, 64);
    let h = ObjectHost::checked(inner)
        .unwrap()
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap();
    (ctx, objects(h))
}
fn id(v: &Value) -> HandleId {
    let Value::Handle(id) = v else {
        panic!("identity")
    };
    *id
}
fn inputs(h: &mut ObjectHost) -> [Value; 2] {
    [
        h.native("CreateTrigger", &[]).unwrap(),
        h.native(
            "CreateDestructable",
            &[
                Value::Int(i32::from_be_bytes(*b"Zxyz")),
                Value::Real(1.0),
                Value::Real(2.0),
                Value::Real(3.0),
                Value::Real(1.0),
                Value::Int(0),
            ],
        )
        .unwrap(),
    ]
}
const EVENT_GLOBAL: &str = "constant widgetevent EVENT_WIDGET_DEATH=ConvertWidgetEvent(862)";
const OTHER_EVENT_GLOBAL: &str = "constant widgetevent OTHER_WIDGET_EVENT=ConvertWidgetEvent(861)";

#[test]
fn optional_death_event_stays_unset_without_the_global() {
    let common = common().replace(EVENT_GLOBAL, OTHER_EVENT_GLOBAL);
    let (_, mut h) = host(&common, EMPTY);
    assert!(
        h.optional_widget_death_event("trigger-admission")
            .unwrap()
            .is_none()
    );
}

#[test]
fn death_event_derivation_refuses_a_missing_global() {
    let common = common().replace(EVENT_GLOBAL, OTHER_EVENT_GLOBAL);
    let (_, h) = host(&common, EMPTY);
    let error = match h.with_death_events(1) {
        Err(error) => error,
        Ok(_) => panic!("missing EVENT_WIDGET_DEATH was admitted"),
    };
    assert_eq!(error.native, "EVENT_WIDGET_DEATH");
    assert!(matches!(error.failure, Failure::TriggerState(_)));
}

#[test]
fn death_event_derivation_refuses_a_foreign_source() {
    let common = common().replace(EVENT_GLOBAL, OTHER_EVENT_GLOBAL);
    let map = format!("globals\n{EVENT_GLOBAL}\nendglobals\n{EMPTY}");
    let (_, h) = host(&common, &map);
    let error = match h.with_death_events(1) {
        Err(error) => error,
        Ok(_) => panic!("foreign EVENT_WIDGET_DEATH source was admitted"),
    };
    assert_eq!(error.native, "EVENT_WIDGET_DEATH");
    assert_eq!(error.failure, Failure::Source);
}

#[test]
fn death_event_derivation_refuses_a_nonstock_initializer() {
    let common = common()
        .replace(
            "type widgetevent extends eventid",
            "type widgetevent extends eventid\ntype gameevent extends eventid",
        )
        .replace(
            EVENT_GLOBAL,
            "constant widgetevent EVENT_WIDGET_DEATH=ConvertGameEvent(15)\nconstant widgetevent OTHER_WIDGET_EVENT=ConvertWidgetEvent(861)",
        );
    let map =
        format!("constant native ConvertGameEvent takes integer i returns gameevent\n{EMPTY}");
    let (_, h) = host(&common, &map);
    let error = match h.with_death_events(1) {
        Err(error) => error,
        Ok(_) => panic!("nonstock EVENT_WIDGET_DEATH initializer was admitted"),
    };
    assert_eq!(error.native, "EVENT_WIDGET_DEATH");
    assert!(matches!(error.failure, Failure::Initializer(_)));
}

#[test]
fn trigger_widget_responses_stay_unsupported_until_death_events_are_admitted() {
    let (_, mut h) = host(&common(), EMPTY);
    for name in ["GetTriggerWidget", "GetTriggerDestructable"] {
        let error = h.native(name, &[]).unwrap_err();
        assert!(error.contains("unsupported"), "{name}: {error}");
    }
}

#[test]
fn owned_vm_retains_units_and_destructables_without_running_actions_or_changing_state() {
    let (ctx, h) = host(&common(), MAP);
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(h.with_death_events(3).unwrap());
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    assert_eq!(vm.global("hits"), Value::Int(0));
    let (t, u, d, a, b, c) = (
        id(&vm.global("T")),
        id(&vm.global("U")),
        id(&vm.global("D")),
        id(&vm.global("A")),
        id(&vm.global("B")),
        id(&vm.global("C")),
    );
    assert_ne!(a, b);
    assert_ne!(b, c);
    let h = &vm.host().inner;
    for (event, widget) in [(a, u), (b, d), (c, d)] {
        assert_eq!(
            h.death_event_registration(event).unwrap(),
            DeathEventRegistration { trigger: t, widget }
        );
        assert_eq!(h.inner().kind(event).unwrap(), "event");
        assert_ne!(event, t);
        assert_ne!(event, widget);
    }
    assert!(h.destructable_creation(d).unwrap().dead);
    assert_eq!(h.retained_death_events().unwrap(), 3);
    assert_eq!(h.retained_unit_events().unwrap(), 0);
    assert_eq!(h.trigger_enabled(t).unwrap(), Some(false));
}
#[test]
fn bad_arguments_and_reserved_families_preserve_existing_subscriptions() {
    let (ctx, mut inner) = death_enum_input(&common(), EMPTY, 64);
    let reserved = ["widget", "unit", "destructable", "trigger"]
        .map(|kind| inner.reserve_identity(kind).unwrap());
    let h = ObjectHost::checked(inner)
        .unwrap()
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap();
    let mut h = objects(h).with_death_events(4).unwrap();
    let args = inputs(&mut h);
    let event = h.native(REGISTER, &args).unwrap();
    let record = h.death_event_registration(id(&event)).unwrap();
    let before = h.inner().handles();
    let unknown = Value::Handle(HandleId::new(std::num::NonZeroU64::new(9999).unwrap()));
    let mut invalid = vec![
        vec![],
        vec![Value::Null, args[1].clone()],
        vec![args[0].clone(), Value::Null],
        vec![args[1].clone(), args[0].clone()],
        vec![unknown.clone(), args[1].clone()],
        vec![args[0].clone(), unknown],
        vec![args[0].clone(), args[1].clone(), Value::Null],
        vec![args[0].clone(), event.clone()],
    ];
    for fake in reserved {
        invalid.push(vec![args[0].clone(), Value::Handle(fake)]);
        invalid.push(vec![Value::Handle(fake), args[1].clone()]);
    }
    for bad in invalid {
        assert!(h.native(REGISTER, &bad).is_err());
        assert_eq!(h.inner().handles(), before);
        assert_eq!(h.retained_death_events().unwrap(), 1);
        assert_eq!(h.death_event_registration(id(&event)).unwrap(), record);
    }
    assert!(h.death_event_registration(id(&args[0])).is_err());
    assert_eq!(h.trigger_enabled(id(&args[0])).unwrap(), None);
}
#[test]
fn separate_record_limit_and_shared_arena_exhaustion_do_not_partially_register() {
    for (limit, fill) in [(0, false), (1, false), (2, true)] {
        let (_, h) = host(&common(), EMPTY);
        let mut h = h.with_death_events(limit).unwrap();
        let args = inputs(&mut h);
        let first = if limit > 0 {
            Some(h.native(REGISTER, &args).unwrap())
        } else {
            None
        };
        if fill {
            while h.inner().handles() < 64 {
                h.native("CreateTimer", &[]).unwrap();
            }
        }
        let before = h.inner().handles();
        assert!(h.native(REGISTER, &args).is_err());
        assert_eq!(h.inner().handles(), before);
        assert_eq!(
            h.retained_death_events().unwrap(),
            usize::from(first.is_some())
        );
        if let Some(first) = first {
            assert_eq!(
                h.death_event_registration(id(&first)).unwrap().widget,
                id(&args[1])
            );
        }
    }
}
#[test]
fn opt_in_contract_source_ancestry_and_platform_limits_are_checked() {
    let (_, mut h) = host(&common(), EMPTY);
    let args = inputs(&mut h);
    assert!(
        h.native(REGISTER, &args)
            .unwrap_err()
            .contains("unsupported")
    );
    assert!(h.retained_death_events().is_err());
    assert!(
        h.with_death_events(1)
            .unwrap()
            .with_death_events(1)
            .is_err()
    );
    let (_, h) = host(&common(), EMPTY);
    assert!(matches!(h.with_death_events(usize::MAX),Err(e) if e.failure==Failure::Capacity));
    for src in [
        common().replace(DECLARATION, &format!("constant {DECLARATION}")),
        common().replace("widget w returns event", "unit w returns event"),
        common().replace("widget w returns event", "widget w returns trigger"),
        common().replace("type widget extends agent", "type widget extends handle"),
        common().replace(DECLARATION, ""),
    ] {
        let (_, h) = enumerated(&src, EMPTY, 64);
        let h = h.with_trigger_admission(trigger_limits(1, 0)).unwrap();
        assert!(h.with_death_events(1).is_err());
    }
    let (_, h) = host(
        &common().replace(DECLARATION, ""),
        &format!("{DECLARATION}{EMPTY}"),
    );
    assert!(matches!(h.with_death_events(1),Err(e) if e.failure==Failure::Source));
    let declaration = "type destructable extends widget\n";
    let (_, h) = host(
        &common().replace(declaration, ""),
        &format!("{declaration}{EMPTY}"),
    );
    assert!(matches!(h.with_death_events(1),Err(e) if e.failure==Failure::Source));
    let (_, inner) = enum_input(&common(), EMPTY, 64);
    let h = ObjectHost::checked(inner).unwrap();
    assert!(h.with_death_events(1).is_err());
}

const DEATH_EXTRAS: &str = "\
type destructable extends widget
type widgetevent extends eventid
type gameevent extends eventid
type triggeraction extends handle
native CreateDestructable takes integer id, real x, real y, real f, real s, integer v returns destructable
native CreateTrigger takes nothing returns trigger
native TriggerRegisterDeathEvent takes trigger t, widget w returns event
native TriggerAddAction takes trigger t, code actionFunc returns triggeraction
constant native ConvertWidgetEvent takes integer i returns widgetevent
constant native ConvertGameEvent takes integer i returns gameevent
constant native GetTriggeringTrigger takes nothing returns trigger
constant native GetTriggerEventId takes nothing returns eventid
constant native GetTriggerWidget takes nothing returns widget
constant native GetTriggerDestructable takes nothing returns destructable
native SetDestructableLife takes destructable d, real life returns nothing
native KillDestructable takes destructable d returns nothing
native DestructableRestoreLife takes destructable d, real life, boolean birth returns nothing
native GetDestructableLife takes destructable d returns real
native GetDestructableMaxLife takes destructable d returns real
native TimerStart takes timer t, real timeout, boolean periodic, code handler returns nothing
native TriggerRegisterTimerExpireEvent takes trigger t, timer which returns event
native TriggerRegisterTimerEvent takes trigger t, real timeout, boolean periodic returns event
native DestroyTrigger takes trigger t returns nothing
native TimerGetElapsed takes timer t returns real
native TimerGetRemaining takes timer t returns real
native TimerGetTimeout takes timer t returns real
native GetExpiredTimer takes nothing returns timer
type playerstate extends handle
type limitop extends eventid
type playerevent extends eventid
constant native ConvertPlayerState takes integer i returns playerstate
constant native ConvertLimitOp takes integer i returns limitop
constant native ConvertPlayerEvent takes integer i returns playerevent
constant native GetPlayerState takes player p, playerstate setting returns integer
native SetPlayerState takes player p, playerstate setting, integer value returns nothing
constant native GetEventPlayerState takes nothing returns playerstate
native TriggerRegisterPlayerStateEvent takes trigger owner, player p, playerstate state, limitop op, real limit returns event
native TriggerSleepAction takes real timeout returns nothing
type boolexpr extends agent
type conditionfunc extends boolexpr
type filterfunc extends boolexpr
type triggercondition extends agent
native Condition takes code f returns conditionfunc
native Filter takes code f returns filterfunc
native TriggerAddCondition takes trigger t, boolexpr c returns triggercondition
native TriggerEvaluate takes trigger t returns boolean
";

fn death_common() -> String {
    let mut common = common_source(&format!("{EXTRAS}{DEATH_EXTRAS}"));
    let end = common.find("\nendglobals\n").expect("endglobals");
    common.insert_str(
        end,
        "\nconstant widgetevent EVENT_WIDGET_DEATH=ConvertWidgetEvent(862)
constant gameevent EVENT_TIMER=ConvertGameEvent(15)
constant playerstate PLAYER_STATE_RESOURCE_GOLD=ConvertPlayerState(1)
constant playerstate PLAYER_STATE_RESOURCE_LUMBER=ConvertPlayerState(2)
constant playerstate PLAYER_STATE_RESOURCE_FOOD_USED=ConvertPlayerState(5)
constant playerstate PLAYER_STATE_RESOURCE_FOOD_CAP=ConvertPlayerState(4)
constant playerstate PLAYER_STATE_FOOD_CAP_CEILING=ConvertPlayerState(6)
constant playerstate PLAYER_STATE_GOLD_GATHERED=ConvertPlayerState(15)
constant playerstate PLAYER_STATE_LUMBER_GATHERED=ConvertPlayerState(16)
constant limitop LESS_THAN=ConvertLimitOp(0)
constant limitop LESS_THAN_OR_EQUAL=ConvertLimitOp(1)
constant limitop EQUAL=ConvertLimitOp(2)
constant limitop GREATER_THAN_OR_EQUAL=ConvertLimitOp(3)
constant limitop GREATER_THAN=ConvertLimitOp(4)
constant limitop NOT_EQUAL=ConvertLimitOp(5)
constant playerevent EVENT_PLAYER_STATE_LIMIT=ConvertPlayerEvent(47)
",
    );
    common
}

fn death_host(common: &str, map: &str) -> (Arc<ParsedMapContext>, ObjectHost) {
    let ctx = super::super::context(300, common, map, "synthetic:destructable-death");
    let enums = EnumHost::checked_selected(
        ctx.scripts.shared_program(),
        "synthetic:destructable-death",
        &[
            "ConvertPlacement",
            "ConvertPlayerColor",
            "ConvertRacePref",
            "ConvertMapControl",
            "ConvertUnitEvent",
            "ConvertWidgetEvent",
            "ConvertGameEvent",
            "ConvertPlayerState",
            "ConvertLimitOp",
            "ConvertPlayerEvent",
        ],
        Limits {
            max_handles: 256,
            admission_steps: 2000,
            expression_depth: 16,
            expression_work: 4000,
        },
    )
    .unwrap();
    let mut host = ObjectHost::checked(enums)
        .unwrap()
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_destructables(DestructableBudget { max_records: 4 })
        .unwrap()
        .with_player_state_requests(PlayerStateRequestBudget { max_records: 4 })
        .unwrap()
        .with_player_resources(crate::player_resources::fixture::rules())
        .unwrap()
        .with_filters(FilterBudget { max_records: 4 })
        .unwrap()
        .with_conditions(ConditionBudget { max_records: 4 })
        .unwrap();
    host.admit_destructable_catalog(life_catalog()).unwrap();
    let host = host
        .with_destructable_restore(crate::playercolor_host::RestoreLifePolicy::synthetic(
            0.0, 0.125, 0.75,
        ))
        .unwrap()
        .with_trigger_admission(trigger_limits(8, 0))
        .unwrap()
        .with_trigger_conditions(4)
        .unwrap()
        .with_timer_runtime(20.0)
        .unwrap()
        .with_death_events(8)
        .unwrap()
        .with_trigger_runtime(TriggerRuntimeInput {
            initial_enabled: true,
            timer_event_global: "EVENT_TIMER".into(),
            max_timed_events: 4,
            max_parked_threads: 4,
        })
        .unwrap()
        .with_player_state_events(4)
        .unwrap();
    (ctx, host)
}

const DEATH_MAP: &str = "\
globals
trigger T=null
destructable D=null
integer hits=0
real seenLife=-1.0
boolean event_ok=false
boolean widget_ok=false
boolean destructable_ok=false
endglobals
function action takes nothing returns nothing
 set hits=hits+1
 set seenLife=GetDestructableLife(D)
 set event_ok=GetTriggerEventId()==EVENT_WIDGET_DEATH
 set widget_ok=GetTriggerWidget()==D
 set destructable_ok=GetTriggerDestructable()==D
endfunction
function setup takes nothing returns nothing
 set T=CreateTrigger()
 call TriggerAddAction(T,function action)
 set D=CreateDestructable('LTlt',0.0,0.0,0.0,1.0,0)
 call TriggerRegisterDeathEvent(T,D)
endfunction
function kill takes nothing returns nothing
 call KillDestructable(D)
endfunction
function negative takes nothing returns nothing
 call SetDestructableLife(D,-5.0)
endfunction
function positive takes nothing returns nothing
 call SetDestructableLife(D,50.0)
endfunction
function restore takes nothing returns nothing
 call DestructableRestoreLife(D,GetDestructableMaxLife(D),false)
endfunction
function restore_amount takes real amount returns nothing
 call DestructableRestoreLife(D,amount,false)
endfunction
function widget_now takes nothing returns widget
 return GetTriggerWidget()
endfunction
";

fn vm_for(
    map: &str,
) -> (
    Arc<ParsedMapContext>,
    OwnedVm<OwnedLocalizedHost<ObjectHost>>,
) {
    let (ctx, host) = death_host(&death_common(), map);
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host);
    vm.init_globals().unwrap();
    (ctx, vm)
}

fn vm_for_sleep(
    map: &str,
) -> (
    Arc<ParsedMapContext>,
    OwnedVm<OwnedLocalizedHost<ObjectHost>>,
) {
    let (ctx, mut host) = death_host(&death_common(), map);
    host.admit_thread_sleep(1).unwrap();
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host);
    vm.init_globals().unwrap();
    (ctx, vm)
}

#[test]
fn a_death_callback_runs_before_the_life_setter_returns_and_restores_context() {
    let (_ctx, mut vm) = vm_for(DEATH_MAP);
    vm.call("setup", &[]).unwrap();
    assert_eq!(
        vm.global("hits"),
        Value::Int(0),
        "the positive setup is not a death"
    );
    vm.call("negative", &[]).unwrap();
    assert_eq!(vm.global("hits"), Value::Int(1));
    assert_eq!(vm.global("seenLife"), Value::Real(0.0));
    assert_eq!(vm.global("event_ok"), Value::Bool(true));
    assert_eq!(vm.global("widget_ok"), Value::Bool(true));
    assert_eq!(vm.global("destructable_ok"), Value::Bool(true));
    assert_eq!(vm.call("widget_now", &[]).unwrap(), Value::Null);
}

#[test]
fn already_dead_trees_do_not_fire_and_positive_life_rearms_the_next_death() {
    let (_ctx, mut vm) = vm_for(DEATH_MAP);
    vm.call("setup", &[]).unwrap();
    vm.call("kill", &[]).unwrap();
    assert_eq!(vm.global("hits"), Value::Int(1));
    vm.call("kill", &[]).unwrap();
    assert_eq!(vm.global("hits"), Value::Int(1));
    vm.call("positive", &[]).unwrap();
    assert_eq!(vm.global("hits"), Value::Int(1));
    vm.call("kill", &[]).unwrap();
    assert_eq!(vm.global("hits"), Value::Int(2));
    vm.call("restore", &[]).unwrap();
    assert_eq!(vm.global("hits"), Value::Int(2));
    vm.call("kill", &[]).unwrap();
    assert_eq!(vm.global("hits"), Value::Int(3));
}

#[test]
fn restoration_requires_the_life_argument_and_refuses_unqualified_forms_atomically() {
    let (_ctx, mut vm) = vm_for(DEATH_MAP);
    vm.call("setup", &[]).unwrap();
    vm.call("kill", &[]).unwrap();
    let tree = id(&vm.global("D"));
    let trigger = id(&vm.global("T"));
    let before = vm.host().inner.destructable_state(tree).unwrap();
    let maximum = before.max_life.unwrap();
    for args in [
        vec![Value::Handle(tree), Value::Bool(false)],
        vec![
            Value::Handle(tree),
            Value::Real(f32::NAN),
            Value::Bool(false),
        ],
        vec![Value::Handle(tree), Value::Real(maximum), Value::Bool(true)],
        vec![
            Value::Handle(trigger),
            Value::Real(maximum),
            Value::Bool(false),
        ],
    ] {
        assert!(
            vm.host()
                .inner
                .native("DestructableRestoreLife", &args)
                .is_err()
        );
        assert_eq!(vm.host().inner.destructable_state(tree).unwrap(), before);
        assert!(vm.host().inner.take_fired_event().is_none());
    }
    assert_eq!(
        vm.host()
            .inner
            .native(
                "DestructableRestoreLife",
                &[Value::Null, Value::Real(maximum), Value::Bool(false)],
            )
            .unwrap(),
        Value::Null
    );
    assert_eq!(vm.host().inner.destructable_state(tree).unwrap(), before);
    vm.call("restore", &[]).unwrap();
    let restored = vm.host().inner.destructable_state(tree).unwrap();
    assert_eq!(restored.life, Some(maximum));
    assert!(!restored.dead);
    assert_eq!(vm.global("hits"), Value::Int(1));
}

#[test]
fn source_threshold_and_floor_restore_dead_targets_without_duplicate_deaths() {
    let (_ctx, mut vm) = vm_for(DEATH_MAP);
    vm.call("setup", &[]).unwrap();
    let tree = id(&vm.global("D"));
    let life = |vm: &mut OwnedVm<OwnedLocalizedHost<ObjectHost>>| {
        vm.host().inner.destructable_state(tree).unwrap()
    };
    vm.call("restore_amount", &[Value::Real(25.0)]).unwrap();
    assert_eq!(life(&mut vm).life, Some(50.0));
    assert_eq!(vm.global("hits"), Value::Int(0));
    for (request, expected) in [
        (0.0, 50.0),
        (25.0, 25.0),
        (100.0, 50.0),
        (-50.0, 0.75),
        (0.0625, 50.0),
        (0.25, 0.75),
        (0.9, 0.9),
    ] {
        vm.call("kill", &[]).unwrap();
        let deaths = vm.global("hits");
        vm.call("restore_amount", &[Value::Real(request)]).unwrap();
        let state = life(&mut vm);
        assert_eq!(state.life, Some(expected), "request={request}");
        assert!(!state.dead);
        assert_eq!(vm.global("hits"), deaths);
    }
}

#[test]
fn engine_damage_queues_the_same_event_for_delivery_at_the_next_boundary() {
    let (_ctx, mut vm) = vm_for(DEATH_MAP);
    vm.call("setup", &[]).unwrap();
    let destructable = id(&vm.global("D"));
    let event = id(&vm.global("EVENT_WIDGET_DEATH"));
    let before = vm.host().inner.event_queue_capacity();
    assert_eq!(
        vm.host()
            .inner
            .damage_destructable(destructable, 50.0)
            .unwrap(),
        0.0
    );
    assert!(
        vm.host().inner.event_queue_capacity() > before,
        "the transition reserves room for every matching registration before committing"
    );
    assert_eq!(vm.global("hits"), Value::Int(0));
    let fired = vm.host().inner.take_fired_event().expect("death queued");
    assert!(vm.host().inner.begin_fired_event(&fired).unwrap().is_some());
    assert_eq!(
        vm.host().inner.native("GetTriggerEventId", &[]).unwrap(),
        Value::Handle(event)
    );
    assert_eq!(
        vm.host().inner.native("GetTriggerWidget", &[]).unwrap(),
        Value::Handle(destructable)
    );
    assert_eq!(
        vm.host()
            .inner
            .native("GetTriggerDestructable", &[])
            .unwrap(),
        Value::Handle(destructable)
    );
    vm.host().inner.finish_fired_event();
    assert!(vm.host().inner.take_fired_event().is_none());
}

#[test]
fn prepared_damage_reserves_and_mutates_only_at_commit() {
    let (_ctx, mut vm) = vm_for(DEATH_MAP);
    vm.call("setup", &[]).unwrap();
    let destructable = id(&vm.global("D"));
    let before = vm
        .host()
        .inner
        .native("GetDestructableLife", &[Value::Handle(destructable)])
        .unwrap();
    let plan = vm
        .host()
        .inner
        .prepare_destructable_damage(destructable, 50.0)
        .unwrap()
        .expect("damage plan");
    assert_eq!(plan.life(), 0.0);
    assert!(!plan.was_dead());
    assert_eq!(
        vm.host()
            .inner
            .native("GetDestructableLife", &[Value::Handle(destructable)])
            .unwrap(),
        before,
        "preparation leaves life untouched"
    );
    assert!(
        vm.host().inner.take_fired_event().is_none(),
        "preparation does not queue a death event"
    );
    vm.host().inner.commit_destructable_damage(plan);
    assert_eq!(
        vm.host()
            .inner
            .native("GetDestructableLife", &[Value::Handle(destructable)])
            .unwrap(),
        Value::Real(0.0)
    );
    assert!(vm.host().inner.take_fired_event().is_some());
}

const NESTED_MAP: &str = "\
globals
trigger T1=null
trigger T2=null
destructable A=null
destructable B=null
integer outer_hits=0
integer inner_hits=0
boolean outer_before=false
boolean outer_after=false
boolean inner_context=false
endglobals
function outer takes nothing returns nothing
 set outer_hits=outer_hits+1
 set outer_before=GetTriggerEventId()==EVENT_WIDGET_DEATH and GetTriggerWidget()==A and GetTriggerDestructable()==A
 call KillDestructable(B)
 set outer_after=GetTriggerEventId()==EVENT_WIDGET_DEATH and GetTriggerWidget()==A and GetTriggerDestructable()==A
endfunction
function inner takes nothing returns nothing
 set inner_hits=inner_hits+1
 set inner_context=GetTriggerEventId()==EVENT_WIDGET_DEATH and GetTriggerWidget()==B and GetTriggerDestructable()==B
endfunction
function setup takes nothing returns nothing
 set T1=CreateTrigger()
 set T2=CreateTrigger()
 call TriggerAddAction(T1,function outer)
 call TriggerAddAction(T2,function inner)
 set A=CreateDestructable('LTlt',0.0,0.0,0.0,1.0,0)
 set B=CreateDestructable('LTlt',2.0,0.0,0.0,1.0,0)
 call TriggerRegisterDeathEvent(T1,A)
 call TriggerRegisterDeathEvent(T2,B)
endfunction
function kill_outer takes nothing returns nothing
 call KillDestructable(A)
endfunction
function widget_now takes nothing returns widget
 return GetTriggerWidget()
endfunction
";

#[test]
fn a_callback_can_kill_another_registered_tree_without_stealing_its_context() {
    let (_ctx, mut vm) = vm_for(NESTED_MAP);
    vm.call("setup", &[]).unwrap();
    vm.call("kill_outer", &[]).unwrap();
    assert_eq!(vm.global("outer_hits"), Value::Int(1));
    assert_eq!(vm.global("inner_hits"), Value::Int(1));
    assert_eq!(vm.global("outer_before"), Value::Bool(true));
    assert_eq!(vm.global("inner_context"), Value::Bool(true));
    assert_eq!(
        vm.global("outer_after"),
        Value::Bool(true),
        "the outer event context returns after the nested death delivery"
    );
    assert_eq!(vm.call("widget_now", &[]).unwrap(), Value::Null);
}

const DEATH_TO_STATE_MAP: &str = "\
globals
trigger DTH=null
trigger STATE=null
player P=null
destructable TREE=null
integer death_hits=0
integer state_hits=0
boolean death_before=false
boolean death_after=false
boolean state_context=false
endglobals
function death_action takes nothing returns nothing
 set death_hits=death_hits+1
 set death_before=GetTriggerEventId()==EVENT_WIDGET_DEATH and GetTriggerWidget()==TREE and GetTriggerDestructable()==TREE
 call SetPlayerState(P, PLAYER_STATE_RESOURCE_GOLD, 10)
 set death_after=GetTriggerEventId()==EVENT_WIDGET_DEATH and GetTriggerWidget()==TREE and GetTriggerDestructable()==TREE
endfunction
function state_action takes nothing returns nothing
 set state_hits=state_hits+1
 set state_context=GetTriggerEventId()==EVENT_PLAYER_STATE_LIMIT and GetEventPlayerState()==PLAYER_STATE_RESOURCE_GOLD and GetTriggerWidget()==null
endfunction
function setup takes nothing returns nothing
 set DTH=CreateTrigger()
 set STATE=CreateTrigger()
 set P=Player(0)
 set TREE=CreateDestructable('LTlt',0.0,0.0,0.0,1.0,0)
 call TriggerAddAction(DTH,function death_action)
 call TriggerAddAction(STATE,function state_action)
 call TriggerRegisterDeathEvent(DTH,TREE)
 call TriggerRegisterPlayerStateEvent(STATE,P,PLAYER_STATE_RESOURCE_GOLD,EQUAL,10.0)
endfunction
function kill_tree takes nothing returns nothing
 call KillDestructable(TREE)
endfunction
function widget_now takes nothing returns widget
 return GetTriggerWidget()
endfunction
";

#[test]
fn a_death_callback_can_nest_a_player_state_callback_before_returning() {
    let (_ctx, mut vm) = vm_for(DEATH_TO_STATE_MAP);
    vm.call("setup", &[]).unwrap();
    vm.call("kill_tree", &[]).unwrap();
    assert_eq!(vm.global("death_hits"), Value::Int(1));
    assert_eq!(vm.global("state_hits"), Value::Int(1));
    assert_eq!(vm.global("death_before"), Value::Bool(true));
    assert_eq!(vm.global("state_context"), Value::Bool(true));
    assert_eq!(
        vm.global("death_after"),
        Value::Bool(true),
        "the outer death context returns after the nested state delivery"
    );
    assert_eq!(vm.call("widget_now", &[]).unwrap(), Value::Null);
}

const STATE_TO_DEATH_MAP: &str = "\
globals
trigger STATE=null
trigger DTH=null
player P=null
destructable TREE=null
integer state_hits=0
integer death_hits=0
boolean state_before=false
boolean state_after=false
boolean death_context=false
endglobals
function state_action takes nothing returns nothing
 set state_hits=state_hits+1
 set state_before=GetTriggerEventId()==EVENT_PLAYER_STATE_LIMIT and GetEventPlayerState()==PLAYER_STATE_RESOURCE_GOLD and GetTriggerWidget()==null
 call KillDestructable(TREE)
 set state_after=GetTriggerEventId()==EVENT_PLAYER_STATE_LIMIT and GetEventPlayerState()==PLAYER_STATE_RESOURCE_GOLD and GetTriggerWidget()==null
endfunction
function death_action takes nothing returns nothing
 set death_hits=death_hits+1
 set death_context=GetTriggerEventId()==EVENT_WIDGET_DEATH and GetTriggerWidget()==TREE and GetTriggerDestructable()==TREE
endfunction
function setup takes nothing returns nothing
 set STATE=CreateTrigger()
 set DTH=CreateTrigger()
 set P=Player(0)
 set TREE=CreateDestructable('LTlt',0.0,0.0,0.0,1.0,0)
 call TriggerAddAction(STATE,function state_action)
 call TriggerAddAction(DTH,function death_action)
 call TriggerRegisterPlayerStateEvent(STATE,P,PLAYER_STATE_RESOURCE_GOLD,LESS_THAN,50.0)
 call TriggerRegisterDeathEvent(DTH,TREE)
endfunction
function set_gold takes nothing returns nothing
 call SetPlayerState(P, PLAYER_STATE_RESOURCE_GOLD, 30)
endfunction
function widget_now takes nothing returns widget
 return GetTriggerWidget()
endfunction
";

#[test]
fn a_player_state_callback_can_nest_a_death_callback_before_returning() {
    let (_ctx, mut vm) = vm_for(STATE_TO_DEATH_MAP);
    vm.call("setup", &[]).unwrap();
    vm.call("set_gold", &[]).unwrap();
    assert_eq!(vm.global("state_hits"), Value::Int(1));
    assert_eq!(vm.global("death_hits"), Value::Int(1));
    assert_eq!(vm.global("state_before"), Value::Bool(true));
    assert_eq!(vm.global("death_context"), Value::Bool(true));
    assert_eq!(
        vm.global("state_after"),
        Value::Bool(true),
        "the outer state context returns after the nested death delivery"
    );
    assert_eq!(vm.call("widget_now", &[]).unwrap(), Value::Null);
}

const CONDITION_MAP: &str = "\
globals
trigger T=null
destructable D=null
conditionfunc C=null
integer conds=0
integer hits=0
endglobals
function predicate takes nothing returns boolean
 set conds=conds+1
 return false
endfunction
function action takes nothing returns nothing
 set hits=hits+1
endfunction
function setup takes nothing returns nothing
 set T=CreateTrigger()
 set C=Condition(function predicate)
 call TriggerAddCondition(T,C)
 call TriggerAddAction(T,function action)
 set D=CreateDestructable('LTlt',0.0,0.0,0.0,1.0,0)
 call TriggerRegisterDeathEvent(T,D)
endfunction
function kill_tree takes nothing returns nothing
 call KillDestructable(D)
endfunction
function widget_now takes nothing returns widget
 return GetTriggerWidget()
endfunction
";

#[test]
fn a_false_death_condition_skips_the_action_without_leaking_context_or_an_event() {
    let (_ctx, mut vm) = vm_for(CONDITION_MAP);
    vm.call("setup", &[]).unwrap();
    vm.call("kill_tree", &[]).unwrap();
    assert_eq!(vm.global("conds"), Value::Int(1));
    assert_eq!(vm.global("hits"), Value::Int(0));
    assert!(vm.host().inner.take_fired_event().is_none());
    assert_eq!(vm.call("widget_now", &[]).unwrap(), Value::Null);
}

const SLEEP_MAP: &str = "\
globals
trigger A=null
trigger B=null
destructable D=null
integer order=0
boolean first_before=false
boolean first_after=false
boolean second_context=false
endglobals
function first takes nothing returns nothing
 set first_before=GetTriggerEventId()==EVENT_WIDGET_DEATH and GetTriggerWidget()==D and GetTriggerDestructable()==D
 set order=order*10+1
 call TriggerSleepAction(2.0)
 set first_after=GetTriggerEventId()==EVENT_WIDGET_DEATH and GetTriggerWidget()==D and GetTriggerDestructable()==D
 set order=order*10+9
endfunction
function second takes nothing returns nothing
 set second_context=GetTriggerEventId()==EVENT_WIDGET_DEATH and GetTriggerWidget()==D and GetTriggerDestructable()==D
 set order=order*10+2
endfunction
function setup takes nothing returns nothing
 set A=CreateTrigger()
 set B=CreateTrigger()
 call TriggerAddAction(A,function first)
 call TriggerAddAction(B,function second)
 set D=CreateDestructable('LTlt',0.0,0.0,0.0,1.0,0)
 call TriggerRegisterDeathEvent(A,D)
 call TriggerRegisterDeathEvent(B,D)
endfunction
function kill_tree takes nothing returns nothing
 call KillDestructable(D)
endfunction
";

#[test]
fn a_sleeping_death_action_still_lets_the_other_registration_finish_before_return() {
    let (_ctx, mut vm) = vm_for_sleep(SLEEP_MAP);
    vm.call("setup", &[]).unwrap();
    vm.call("kill_tree", &[]).unwrap();
    assert_eq!(vm.global("order"), Value::Int(12));
    assert_eq!(vm.global("first_before"), Value::Bool(true));
    assert_eq!(vm.global("second_context"), Value::Bool(true));
    assert_eq!(vm.host().inner.parked_thread_count(), 1);
    let ticket = vm.host().inner.next_thread_wake(30.0).unwrap().unwrap();
    assert_eq!(vm.host().inner.timer_game_time(), Some(22.0));
    vm.resume_thread(ticket).unwrap();
    assert_eq!(vm.global("order"), Value::Int(129));
    assert_eq!(
        vm.global("first_after"),
        Value::Bool(true),
        "the sleeping callback resumes under its own event context"
    );
    assert_eq!(vm.host().inner.parked_thread_count(), 0);
    assert!(vm.host().inner.take_fired_event().is_none());
}

const ERROR_MAP: &str = "\
globals
trigger T=null
destructable D=null
integer hits=0
endglobals
function action takes nothing returns nothing
 set hits=hits+1
 call CreateUnit(Player(0), 'hfoo', 0.0, 0.0, 0.0)
endfunction
function setup takes nothing returns nothing
 set T=CreateTrigger()
 call TriggerAddAction(T,function action)
 set D=CreateDestructable('LTlt',0.0,0.0,0.0,1.0,0)
 call TriggerRegisterDeathEvent(T,D)
endfunction
function kill_tree takes nothing returns nothing
 call KillDestructable(D)
endfunction
function widget_now takes nothing returns widget
 return GetTriggerWidget()
endfunction
";

#[test]
fn an_action_error_aborts_the_synchronous_delivery_and_restores_context() {
    let (_ctx, mut vm) = vm_for(ERROR_MAP);
    vm.call("setup", &[]).unwrap();
    assert!(vm.call("kill_tree", &[]).is_err());
    assert_eq!(vm.global("hits"), Value::Int(1));
    assert_eq!(vm.call("widget_now", &[]).unwrap(), Value::Null);
    assert!(vm.host().inner.take_fired_event().is_none());
}

const NESTING_MAP: &str = "\
globals
trigger T=null
destructable D=null
integer hits=0
endglobals
function action takes nothing returns nothing
 set hits=hits+1
 call SetDestructableLife(D,10.0)
 call KillDestructable(D)
endfunction
function setup takes nothing returns nothing
 set T=CreateTrigger()
 call TriggerAddAction(T,function action)
 set D=CreateDestructable('LTlt',0.0,0.0,0.0,1.0,0)
 call TriggerRegisterDeathEvent(T,D)
endfunction
function kill_tree takes nothing returns nothing
 call KillDestructable(D)
endfunction
function widget_now takes nothing returns widget
 return GetTriggerWidget()
endfunction
";

#[test]
fn runaway_nested_deaths_stop_at_the_shared_call_depth_budget() {
    let (_ctx, mut vm) = vm_for(NESTING_MAP);
    vm.call("setup", &[]).unwrap();
    let error = vm.call("kill_tree", &[]).unwrap_err().to_string();
    assert!(error.contains("nested past 64 spawns"), "{error}");
    assert!(matches!(vm.global("hits"), Value::Int(n) if n > 0));
    assert_eq!(vm.call("widget_now", &[]).unwrap(), Value::Null);
    assert!(vm.host().inner.take_fired_event().is_none());
}

use super::{
    Counting, EXTRAS, budget, common_source, enum_input, enumerated, slots_rule, source,
    trigger_limits, unit_limits,
};
use crate::map_admission::ParsedMapContext;
use crate::playercolor_host::{
    ConditionBudget, DeathEventRegistration, DestructableBudget, EnumHost, Failure, Limits,
    ObjectHost,
};
use pcx_script::{HandleId, Host, Value};
use std::sync::{Arc, atomic::AtomicUsize};

const REGISTER: &str = "TriggerRegisterDeathEvent";
const DECLARATION: &str =
    "native TriggerRegisterDeathEvent takes trigger t, widget w returns event\n";
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
    common_source(&format!("{EXTRAS}{EXTRA}{DECLARATION}"))
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
    let (ctx, h) = enumerated(common, map, 64);
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
    let (ctx, mut inner) = enum_input(&common(), EMPTY, 64);
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

const SYNC_EXTRA: &str = "\
type gameevent extends eventid
type boolexpr extends agent
type conditionfunc extends boolexpr
type triggercondition extends agent
native TriggerRegisterTimerEvent takes trigger t, real timeout, boolean periodic returns event
native DestroyTrigger takes trigger t returns nothing
native TimerStart takes timer t, real timeout, boolean periodic, code callback returns nothing
native TimerGetElapsed takes timer t returns real
native TimerGetRemaining takes timer t returns real
native TimerGetTimeout takes timer t returns real
native GetExpiredTimer takes nothing returns timer
constant native ConvertGameEvent takes integer i returns gameevent
native TriggerAddCondition takes trigger t, boolexpr c returns triggercondition
native TriggerExecute takes trigger t returns nothing
native Condition takes code f returns conditionfunc
type playerstate extends handle
type limitop extends eventid
type playerevent extends eventid
constant native ConvertPlayerState takes integer i returns playerstate
constant native ConvertLimitOp takes integer i returns limitop
constant native ConvertPlayerEvent takes integer i returns playerevent
constant native GetPlayerState takes player p, playerstate setting returns integer
native SetPlayerState takes player p, playerstate setting, integer value returns nothing
native TriggerRegisterPlayerStateEvent takes trigger owner, player p, playerstate state, limitop op, real limit returns event
native KillDestructable takes destructable d returns nothing
native SetDestructableLife takes destructable d, real life returns nothing
native GetDestructableLife takes destructable d returns real
constant native GetTriggeringTrigger takes nothing returns trigger
constant native GetTriggerEventId takes nothing returns eventid
constant native GetTriggerWidget takes nothing returns widget
constant native GetTriggerDestructable takes nothing returns destructable
";
const SYNC_MAP: &str = "\
globals
trigger T = null
destructable D = null
event E = null
eventid GOT = null
widget W = null
destructable OUT = null
string trace = \"\"
boolean okay = true
real life_during = 1.0
endglobals
function condition takes nothing returns boolean
    set trace = trace + \"C\"
    return true
endfunction
function action takes nothing returns nothing
    set trace = trace + \"A\"
    set life_during = GetDestructableLife(D)
    set GOT = GetTriggerEventId()
    set W = GetTriggerWidget()
    set OUT = GetTriggerDestructable()
    set okay = okay and GetTriggeringTrigger() == T
endfunction
function setup takes nothing returns nothing
    set T = CreateTrigger()
    call TriggerAddCondition(T, Condition(function condition))
    call TriggerAddAction(T, function action)
    set D = CreateDestructable('Zxyz', 4.0, 5.0, 0.0, 1.0, 0)
    set E = TriggerRegisterDeathEvent(T, D)
endfunction
function hit takes nothing returns nothing
    call setup()
    call SetDestructableLife(D, 0.0)
    set trace = trace + \"N\"
    call SetDestructableLife(D, 50.0)
    set trace = trace + \"R\"
    call SetDestructableLife(D, 0.0)
    set trace = trace + \"N\"
endfunction
function hit_bad takes nothing returns nothing
    call setup()
    call SetDestructableLife(D, 0.0 / 0.0)
endfunction
";
fn sync_common() -> String {
    let mut common = common_source(&format!("{EXTRAS}{EXTRA}{SYNC_EXTRA}{DECLARATION}"));
    let globals = common.find("\nglobals\n").expect("globals") + "\nglobals\n".len();
    common.insert_str(
        globals,
        "constant gameevent EVT=ConvertGameEvent(4)\nconstant playerstate PLAYER_STATE_RESOURCE_GOLD=ConvertPlayerState(1)\nconstant playerstate PLAYER_STATE_RESOURCE_LUMBER=ConvertPlayerState(2)\nconstant playerstate PLAYER_STATE_RESOURCE_FOOD_USED=ConvertPlayerState(5)\nconstant playerstate PLAYER_STATE_RESOURCE_FOOD_CAP=ConvertPlayerState(4)\nconstant playerstate PLAYER_STATE_FOOD_CAP_CEILING=ConvertPlayerState(6)\nconstant playerstate PLAYER_STATE_GOLD_GATHERED=ConvertPlayerState(15)\nconstant playerstate PLAYER_STATE_LUMBER_GATHERED=ConvertPlayerState(16)\nconstant limitop LESS_THAN=ConvertLimitOp(1)\nconstant limitop LESS_THAN_OR_EQUAL=ConvertLimitOp(2)\nconstant limitop EQUAL=ConvertLimitOp(3)\nconstant limitop GREATER_THAN_OR_EQUAL=ConvertLimitOp(4)\nconstant limitop GREATER_THAN=ConvertLimitOp(5)\nconstant limitop NOT_EQUAL=ConvertLimitOp(6)\nconstant playerevent EVENT_PLAYER_STATE_LIMIT=ConvertPlayerEvent(47)\n",
    );
    common
}

fn slk(headers: &[&str], rows: &[&[&str]]) -> Vec<u8> {
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

fn sync_catalog() -> crate::playercolor_host::DestructableCatalog {
    let data = slk(
        &["DestructableID", "HP", "targType"],
        &[&["Zxyz", "50", "tree,ground"]],
    );
    let metadata = slk(
        &["ID", "field", "slk", "type", "index", "category"],
        &[
            &["Ahpv", "HP", "DestructableData", "int", "0", "stats"],
            &[
                "Atgt",
                "targType",
                "DestructableData",
                "string",
                "0",
                "combat",
            ],
        ],
    );
    let input = [
        (pcx_objects::editor::SlkId::DestructableData.path(), data),
        (
            pcx_objects::editor::SlkId::DestructableMetaData.path(),
            metadata,
        ),
    ];
    let bytes: Vec<(&str, &[u8])> = input
        .iter()
        .map(|(path, data)| (*path, data.as_slice()))
        .collect();
    let tables = pcx_objects::editor::defaults::DefaultTables::from_bytes(
        pcx_objects::editor::defaults::Identity {
            install: "synthetic".into(),
            edition_version: "synthetic".into(),
        },
        &bytes,
        &[],
        pcx_objects::editor::EditorReadLimits {
            max_listed_paths: 8,
            max_asset_bytes: 4096,
            max_aggregate_input_bytes: 16384,
        },
    )
    .unwrap();
    crate::playercolor_host::DestructableCatalog::new(
        Arc::new(tables),
        pcx_objects::editor::Overlay::default(),
        pcx_objects::editor::Overlay::default(),
    )
}

fn sync_host(common: &str, map: &str) -> (Arc<ParsedMapContext>, ObjectHost) {
    let ctx = super::super::super::context(300, common, map, "trigger-admission");
    let inner = EnumHost::checked_selected(
        ctx.scripts.shared_program(),
        "trigger-admission",
        &[
            "ConvertPlacement",
            "ConvertPlayerColor",
            "ConvertRacePref",
            "ConvertMapControl",
            "ConvertUnitEvent",
            "ConvertGameEvent",
            "ConvertPlayerState",
            "ConvertLimitOp",
            "ConvertPlayerEvent",
        ],
        Limits {
            max_handles: 64,
            admission_steps: 1000,
            expression_depth: 16,
            expression_work: 1000,
        },
    )
    .unwrap();
    let h = ObjectHost::checked(inner)
        .unwrap()
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap();
    let mut h = objects(h)
        .with_conditions(ConditionBudget { max_records: 2 })
        .unwrap()
        .with_trigger_conditions(4)
        .unwrap()
        .with_player_state_requests(crate::playercolor_host::PlayerStateRequestBudget {
            max_records: 4,
        })
        .unwrap()
        .with_player_resources(crate::player_resources::fixture::rules())
        .unwrap()
        .with_player_state_events(4)
        .unwrap();
    h.admit_destructable_catalog(sync_catalog()).unwrap();
    (ctx, h)
}
const CROSS_MAP: &str = "\
globals
trigger DTM = null
trigger DTP = null
trigger STK = null
trigger DTN = null
trigger X = null
trigger ST = null
destructable D = null
player P = null
string trace = \"\"
integer death_hits = 0
integer state_hits = 0
integer gold_after = -1
integer life_after = -1
endglobals
function death_mutate takes nothing returns nothing
    set trace = trace + \"D\"
    set death_hits = death_hits + 1
    call SetPlayerState(P, PLAYER_STATE_RESOURCE_GOLD, 30)
endfunction
function death_plain takes nothing returns nothing
    set trace = trace + \"D\"
    set death_hits = death_hits + 1
endfunction
function state_action takes nothing returns nothing
    set trace = trace + \"S\"
    set state_hits = state_hits + 1
endfunction
function state_kills takes nothing returns nothing
    set trace = trace + \"S\"
    set state_hits = state_hits + 1
    call SetDestructableLife(D, 0.0)
endfunction
function other_action takes nothing returns nothing
    set trace = trace + \"X\"
endfunction
function death_nested takes nothing returns nothing
    set trace = trace + \"D\"
    call TriggerExecute(X)
endfunction
function tree takes nothing returns nothing
    set D = CreateDestructable('Zxyz', 4.0, 5.0, 0.0, 1.0, 0)
    set P = Player(0)
endfunction
function hit_death_mutates_state takes nothing returns nothing
    call tree()
    set DTM = CreateTrigger()
    call TriggerAddAction(DTM, function death_mutate)
    call TriggerRegisterDeathEvent(DTM, D)
    set ST = CreateTrigger()
    call TriggerAddAction(ST, function state_action)
    call TriggerRegisterPlayerStateEvent(ST, P, PLAYER_STATE_RESOURCE_GOLD, LESS_THAN, 50.0)
    call SetDestructableLife(D, 0.0)
    set trace = trace + \"N\"
    set gold_after = GetPlayerState(P, PLAYER_STATE_RESOURCE_GOLD)
    set life_after = GetDestructableLife(D)
endfunction
function hit_death_inside_state takes nothing returns nothing
    call tree()
    set DTP = CreateTrigger()
    call TriggerAddAction(DTP, function death_plain)
    call TriggerRegisterDeathEvent(DTP, D)
    set STK = CreateTrigger()
    call TriggerAddAction(STK, function state_kills)
    call TriggerRegisterPlayerStateEvent(STK, P, PLAYER_STATE_RESOURCE_GOLD, LESS_THAN, 50.0)
    set trace = trace + \"M\"
    call SetPlayerState(P, PLAYER_STATE_RESOURCE_GOLD, 30)
    set trace = trace + \"N\"
    set life_after = GetDestructableLife(D)
endfunction
function hit_nested_trigger_execute takes nothing returns nothing
    call tree()
    set DTN = CreateTrigger()
    set X = CreateTrigger()
    call TriggerAddAction(DTN, function death_nested)
    call TriggerAddAction(X, function other_action)
    call TriggerRegisterDeathEvent(DTN, D)
    call SetDestructableLife(D, 0.0)
    set trace = trace + \"N\"
endfunction
";
fn cross_host(common: &str, map: &str) -> (Arc<ParsedMapContext>, ObjectHost) {
    let (ctx, h) = sync_host(common, map);
    let h = h
        .with_timer_runtime(20.0)
        .unwrap()
        .with_trigger_runtime(crate::playercolor_host::TriggerRuntimeInput {
            initial_enabled: true,
            timer_event_global: "EVT".into(),
            max_timed_events: 2,
            max_parked_threads: 0,
        })
        .unwrap()
        .with_death_events(2)
        .unwrap();
    (ctx, h)
}
#[test]
fn a_death_callback_can_mutate_player_state_before_the_native_returns() {
    let (ctx, h) = cross_host(&sync_common(), CROSS_MAP);
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(h);
    vm.init_globals().unwrap();
    vm.call("hit_death_mutates_state", &[]).unwrap();
    assert_eq!(vm.global("trace"), Value::Str("DSN".into()));
    assert_eq!(vm.global("death_hits"), Value::Int(1));
    assert_eq!(vm.global("state_hits"), Value::Int(1));
    assert_eq!(vm.global("gold_after"), Value::Int(30));
    assert_eq!(vm.global("life_after"), Value::Real(0.0));
    assert!(vm.host().inner.take_fired_event().is_none());
}
#[test]
fn a_player_state_callback_can_kill_a_destructable_before_it_returns() {
    let (ctx, h) = cross_host(&sync_common(), CROSS_MAP);
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(h);
    vm.init_globals().unwrap();
    vm.call("hit_death_inside_state", &[]).unwrap();
    assert_eq!(vm.global("trace"), Value::Str("MSDN".into()));
    assert_eq!(vm.global("death_hits"), Value::Int(1));
    assert_eq!(vm.global("state_hits"), Value::Int(1));
    assert_eq!(vm.global("life_after"), Value::Real(0.0));
    assert!(vm.host().inner.take_fired_event().is_none());
}
#[test]
fn a_death_callback_can_execute_another_trigger_before_the_native_returns() {
    let (ctx, h) = cross_host(&sync_common(), CROSS_MAP);
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(h);
    vm.init_globals().unwrap();
    vm.call("hit_nested_trigger_execute", &[]).unwrap();
    assert_eq!(vm.global("trace"), Value::Str("DXN".into()));
    assert!(vm.host().inner.take_fired_event().is_none());
}
#[test]
fn a_refused_life_write_leaves_the_tree_and_delivery_batch_unchanged() {
    let (ctx, h) = sync_host(&sync_common(), SYNC_MAP);
    let h = h
        .with_timer_runtime(20.0)
        .unwrap()
        .with_trigger_runtime(crate::playercolor_host::TriggerRuntimeInput {
            initial_enabled: true,
            timer_event_global: "EVT".into(),
            max_timed_events: 2,
            max_parked_threads: 0,
        })
        .unwrap()
        .with_death_events(2)
        .unwrap();
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(h);
    vm.init_globals().unwrap();
    assert!(vm.call("hit_bad", &[]).is_err());
    assert_eq!(vm.global("trace"), Value::Str(String::new()));
    assert!(vm.host().inner.take_fired_event().is_none());
    assert!(!vm.host().inner.synchronous_delivery_active());
    let d = id(&vm.global("D"));
    assert_eq!(
        vm.host().inner.destructable_state(d).unwrap().life,
        Some(50.0)
    );
}

#[test]
fn script_life_loss_delivers_callbacks_before_the_native_returns_and_rearms() {
    let (ctx, h) = sync_host(&sync_common(), SYNC_MAP);
    let h = h
        .with_timer_runtime(20.0)
        .unwrap()
        .with_trigger_runtime(crate::playercolor_host::TriggerRuntimeInput {
            initial_enabled: true,
            timer_event_global: "EVT".into(),
            max_timed_events: 2,
            max_parked_threads: 0,
        })
        .unwrap()
        .with_death_events(2)
        .unwrap();
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(h);
    vm.init_globals().unwrap();
    vm.call("hit", &[]).unwrap();
    assert_eq!(vm.global("trace"), Value::Str("CANRCAN".into()));
    assert_eq!(vm.global("okay"), Value::Bool(true));
    assert_eq!(vm.global("life_during"), Value::Real(0.0));
    let e = id(&vm.global("E"));
    assert_eq!(id(&vm.global("GOT")), e);
    assert_eq!(id(&vm.global("W")), id(&vm.global("D")));
    assert_eq!(id(&vm.global("OUT")), id(&vm.global("D")));
}

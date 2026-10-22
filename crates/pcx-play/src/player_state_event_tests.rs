use super::{EXTRAS, budget, common_source, context, slots_rule};
use crate::playercolor_host::{
    EnumHost, Limits, ObjectHost, PlayerStateRequestBudget, TriggerBudget, TriggerRuntimeInput,
};
use pcx_script::{HandleId, Host, Value};
use std::sync::Arc;

const PLAYER_STATE: &str = "type playerstate extends handle
type limitop extends eventid
type gameevent extends eventid
constant native ConvertPlayerState takes integer i returns playerstate
constant native ConvertLimitOp takes integer i returns limitop
constant native ConvertGameEvent takes integer i returns gameevent
constant native GetPlayerState takes player p, playerstate setting returns integer
native SetPlayerState takes player p, playerstate setting, integer value returns nothing
constant native GetEventPlayerState takes nothing returns playerstate
native TriggerRegisterPlayerStateEvent takes trigger owner, player p, playerstate state, limitop op, real limit returns event
native TriggerRegisterTimerEvent takes trigger owner, real timeout, boolean periodic returns event
native TriggerSleepAction takes real timeout returns nothing
native TimerStart takes timer t, real timeout, boolean periodic, code callback returns nothing
native TimerGetElapsed takes timer t returns real
native TimerGetRemaining takes timer t returns real
native TimerGetTimeout takes timer t returns real
native GetExpiredTimer takes nothing returns timer
native DestroyTrigger takes trigger whichTrigger returns nothing
constant native GetTriggeringTrigger takes nothing returns trigger
constant native GetTriggerEventId takes nothing returns eventid
";

fn common(less_than: i32) -> String {
    let mut common = common_source(EXTRAS);
    let globals = common.find("\nglobals\n").expect("globals") + 1;
    common.insert_str(globals, PLAYER_STATE);
    let end = common.find("\nendglobals\n").expect("endglobals");
    common.insert_str(
        end,
        &format!(
            "\nconstant playerstate PLAYER_STATE_RESOURCE_GOLD=ConvertPlayerState(1)
constant playerstate PLAYER_STATE_RESOURCE_LUMBER=ConvertPlayerState(2)
constant playerstate PLAYER_STATE_RESOURCE_FOOD_USED=ConvertPlayerState(5)
constant playerstate PLAYER_STATE_RESOURCE_FOOD_CAP=ConvertPlayerState(4)
constant playerstate PLAYER_STATE_FOOD_CAP_CEILING=ConvertPlayerState(6)
constant limitop LESS_THAN=ConvertLimitOp({less_than})
constant limitop LESS_THAN_OR_EQUAL=ConvertLimitOp(1)
constant limitop EQUAL=ConvertLimitOp(2)
constant limitop GREATER_THAN_OR_EQUAL=ConvertLimitOp(3)
constant limitop GREATER_THAN=ConvertLimitOp(4)
constant limitop NOT_EQUAL=ConvertLimitOp(5)
constant gameevent EVT=ConvertGameEvent(13 + 2)
"
        ),
    );
    common
}

fn base(common: &str) -> (Arc<crate::map_admission::ParsedMapContext>, ObjectHost) {
    base_map(common, "// synthetic map")
}

fn base_map(common: &str, map: &str) -> (Arc<crate::map_admission::ParsedMapContext>, ObjectHost) {
    let ctx = context(300, common, map, "synthetic:player-state-events");
    let names = [
        "ConvertPlacement",
        "ConvertPlayerColor",
        "ConvertRacePref",
        "ConvertMapControl",
        "ConvertUnitEvent",
        "ConvertPlayerState",
        "ConvertLimitOp",
        "ConvertGameEvent",
    ];
    let enums = EnumHost::checked_selected(
        ctx.scripts.shared_program(),
        "synthetic:player-state-events",
        &names,
        Limits {
            max_handles: 512,
            admission_steps: 4000,
            expression_depth: 16,
            expression_work: 4000,
        },
    )
    .unwrap();
    let host = ObjectHost::checked(enums)
        .unwrap()
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_player_state_requests(PlayerStateRequestBudget { max_records: 4 })
        .unwrap()
        .with_player_resources(crate::player_resources::fixture::rules())
        .unwrap()
        .with_trigger_admission(TriggerBudget {
            max_triggers: 64,
            max_unit_events: 0,
            max_actions: 8,
        })
        .unwrap()
        .with_timer_runtime(20.0)
        .unwrap()
        .with_trigger_runtime(TriggerRuntimeInput {
            initial_enabled: true,
            timer_event_global: "EVT".into(),
            max_timed_events: 4,
            max_parked_threads: 0,
        })
        .unwrap();
    (ctx, host)
}

fn host(common: &str, max_events: usize) -> ObjectHost {
    base(common).1.with_player_state_events(max_events).unwrap()
}

fn id(value: &Value) -> HandleId {
    let Value::Handle(id) = value else {
        panic!("handle")
    };
    *id
}

fn convert(host: &mut ObjectHost, name: &str, value: i32) -> Value {
    host.native(name, &[Value::Int(value)]).unwrap()
}

fn register(
    host: &mut ObjectHost,
    trigger: &Value,
    player: &Value,
    state: &Value,
    op: &Value,
    limit: f32,
) -> Result<Value, String> {
    host.native(
        "TriggerRegisterPlayerStateEvent",
        &[
            trigger.clone(),
            player.clone(),
            state.clone(),
            op.clone(),
            Value::Real(limit),
        ],
    )
}

#[test]
fn state_events_seed_at_registration_then_fire_only_on_change_while_the_comparison_holds() {
    let mut h = host(&common(0), 4);
    let trigger = h.native("CreateTrigger", &[]).unwrap();
    let player = h.native("Player", &[Value::Int(0)]).unwrap();
    let gold = convert(&mut h, "ConvertPlayerState", 1);
    let less = convert(&mut h, "ConvertLimitOp", 0);
    let event = register(&mut h, &trigger, &player, &gold, &less, 50.0).unwrap();
    assert_eq!(h.retained_player_state_events().unwrap(), 1);
    let record = h.player_state_event_registration(id(&event)).unwrap();
    assert_eq!(
        (
            record.trigger,
            record.player,
            record.state,
            record.state_value,
            record.limitop,
            record.limitop_value,
            record.limit
        ),
        (id(&trigger), id(&player), id(&gold), 1, id(&less), 0, 50.0)
    );
    assert_eq!(record.last, 3);

    let mut work = 16;
    assert_eq!(h.poll_player_state_events(&mut work).unwrap(), 0);
    assert_eq!(work, 15);

    h.native(
        "SetPlayerState",
        &[player.clone(), gold.clone(), Value::Int(30)],
    )
    .unwrap();
    let fired = h.take_fired_event().expect("player state event");
    assert_eq!(fired.trigger, id(&trigger));
    assert_eq!(fired.subject.player, Some(id(&player)));
    assert_eq!(fired.subject.player_state, Some(id(&gold)));
    assert!(h.begin_fired_event(&fired).unwrap().is_some());
    assert_eq!(
        h.native("GetEventPlayerState", &[]).unwrap(),
        Value::Handle(id(&gold))
    );
    h.finish_fired_event();
    assert_eq!(
        h.player_state_event_registration(id(&event)).unwrap().last,
        30
    );
    let mut work = 16;
    assert_eq!(h.poll_player_state_events(&mut work).unwrap(), 0);
    assert_eq!(work, 15);

    h.native(
        "SetPlayerState",
        &[player.clone(), gold.clone(), Value::Int(80)],
    )
    .unwrap();
    assert!(h.take_fired_event().is_none());
    let mut work = 16;
    assert_eq!(h.poll_player_state_events(&mut work).unwrap(), 0);
    assert_eq!(
        h.player_state_event_registration(id(&event)).unwrap().last,
        80
    );

    h.native(
        "SetPlayerState",
        &[player.clone(), gold.clone(), Value::Int(25)],
    )
    .unwrap();
    assert!(h.take_fired_event().is_some());
}

#[test]
fn registration_uses_the_selected_limitop_constants_rather_than_fixed_numbers() {
    let mut h = host(&common(-7), 4);
    let trigger = h.native("CreateTrigger", &[]).unwrap();
    let player = h.native("Player", &[Value::Int(0)]).unwrap();
    let gold = convert(&mut h, "ConvertPlayerState", 1);
    let less = convert(&mut h, "ConvertLimitOp", -7);
    register(&mut h, &trigger, &player, &gold, &less, 50.0).unwrap();
    h.native("SetPlayerState", &[player, gold, Value::Int(30)])
        .unwrap();
    assert!(h.take_fired_event().is_some());
}

#[test]
fn registration_budget_and_complete_limit_operation_source_are_required() {
    let (_, unadmitted) = base(&common(0));
    assert!(matches!(
        unadmitted.with_player_state_events(usize::MAX),
        Err(error) if error.failure == crate::playercolor_host::Failure::Capacity
    ));

    let mut h = host(&common(0), 1);
    let trigger = h.native("CreateTrigger", &[]).unwrap();
    let player = h.native("Player", &[Value::Int(0)]).unwrap();
    let gold = convert(&mut h, "ConvertPlayerState", 1);
    let less = convert(&mut h, "ConvertLimitOp", 0);
    register(&mut h, &trigger, &player, &gold, &less, 50.0).unwrap();
    assert!(register(&mut h, &trigger, &player, &gold, &less, 50.0).is_err());
    assert_eq!(h.retained_player_state_events().unwrap(), 1);

    let missing = common(0).replace("constant limitop EQUAL=ConvertLimitOp(2)\n", "");
    assert!(base(&missing).1.with_player_state_events(4).is_err());
}

#[test]
fn intermediate_qualifying_changes_are_committed_before_the_boundary() {
    let mut h = host(&common(0), 4);
    let trigger = h.native("CreateTrigger", &[]).unwrap();
    let player = h.native("Player", &[Value::Int(0)]).unwrap();
    let gold = convert(&mut h, "ConvertPlayerState", 1);
    let equal = convert(&mut h, "ConvertLimitOp", 2);
    let event = register(&mut h, &trigger, &player, &gold, &equal, 10.0).unwrap();

    h.native(
        "SetPlayerState",
        &[player.clone(), gold.clone(), Value::Int(10)],
    )
    .unwrap();
    h.native(
        "SetPlayerState",
        &[player.clone(), gold.clone(), Value::Int(0)],
    )
    .unwrap();

    assert_eq!(
        h.player_state_event_registration(id(&event)).unwrap().last,
        0
    );
    assert!(
        h.take_fired_event().is_some(),
        "the intermediate 10 qualifies"
    );
    assert!(h.take_fired_event().is_none());
    let mut work = 16;
    assert_eq!(h.poll_player_state_events(&mut work).unwrap(), 0);
    assert_eq!(work, 15);
}

#[test]
fn repeated_qualifying_writes_queue_each_observable_change() {
    let mut h = host(&common(0), 4);
    let trigger = h.native("CreateTrigger", &[]).unwrap();
    let player = h.native("Player", &[Value::Int(0)]).unwrap();
    let gold = convert(&mut h, "ConvertPlayerState", 1);
    let less = convert(&mut h, "ConvertLimitOp", 0);
    let event = register(&mut h, &trigger, &player, &gold, &less, 50.0).unwrap();

    for value in [10, 20, 20] {
        h.native(
            "SetPlayerState",
            &[player.clone(), gold.clone(), Value::Int(value)],
        )
        .unwrap();
    }

    assert_eq!(
        h.player_state_event_registration(id(&event)).unwrap().last,
        20
    );
    assert!(h.take_fired_event().is_some());
    assert!(h.take_fired_event().is_some());
    assert!(h.take_fired_event().is_none());
}

#[test]
fn same_effective_value_write_does_not_fire() {
    let mut h = host(&common(0), 4);
    let trigger = h.native("CreateTrigger", &[]).unwrap();
    let player = h.native("Player", &[Value::Int(0)]).unwrap();
    let gold = convert(&mut h, "ConvertPlayerState", 1);
    let equal = convert(&mut h, "ConvertLimitOp", 2);
    let event = register(&mut h, &trigger, &player, &gold, &equal, 10.0).unwrap();

    h.native(
        "SetPlayerState",
        &[player.clone(), gold.clone(), Value::Int(10)],
    )
    .unwrap();
    h.native("SetPlayerState", &[player, gold, Value::Int(10)])
        .unwrap();

    assert_eq!(
        h.player_state_event_registration(id(&event)).unwrap().last,
        10
    );
    assert!(h.take_fired_event().is_some());
    assert!(h.take_fired_event().is_none());
}

#[test]
fn gold_debit_uses_the_same_player_state_change_path() {
    let mut h = host(&common(0), 4);
    let trigger = h.native("CreateTrigger", &[]).unwrap();
    let player = h.native("Player", &[Value::Int(0)]).unwrap();
    let gold = convert(&mut h, "ConvertPlayerState", 1);
    let less = convert(&mut h, "ConvertLimitOp", 0);
    let event = register(&mut h, &trigger, &player, &gold, &less, 50.0).unwrap();

    let debit = h.prepare_gold_debit_reserving(id(&player), 3, 0).unwrap();
    h.apply_gold_debit(debit);

    assert_eq!(
        h.player_state_event_registration(id(&event)).unwrap().last,
        0
    );
    let fired = h.take_fired_event().expect("debit state event");
    assert_eq!(fired.subject.player, Some(id(&player)));
    assert_eq!(fired.subject.player_state, Some(id(&gold)));
    assert!(h.take_fired_event().is_none());
}

#[test]
fn a_gold_debit_reserves_pending_sale_events_and_its_own_state_event_together() {
    let mut h = host(&common(0), 4);
    let trigger = h.native("CreateTrigger", &[]).unwrap();
    let player = h.native("Player", &[Value::Int(0)]).unwrap();
    let gold = convert(&mut h, "ConvertPlayerState", 1);
    let less = convert(&mut h, "ConvertLimitOp", 0);
    register(&mut h, &trigger, &player, &gold, &less, 50.0).unwrap();

    let before = h.event_queue_capacity();
    let debit = h.prepare_gold_debit_reserving(id(&player), 3, 4).unwrap();
    let reserved = h.event_queue_capacity();
    assert!(
        reserved >= before + 5,
        "four pending events and one state event need one combined block"
    );
    for _ in 0..4 {
        h.push_reserved_event(
            id(&trigger),
            None,
            crate::playercolor_host::EventSubject::default(),
            None,
        );
    }
    h.apply_gold_debit(debit);
    assert_eq!(
        h.event_queue_capacity(),
        reserved,
        "both commits push into the combined reservation without growing it"
    );
    for _ in 0..5 {
        assert!(h.take_fired_event().is_some());
    }
    assert!(h.take_fired_event().is_none());
}

#[test]
fn a_gold_debit_reserves_pending_events_when_player_state_events_are_not_admitted() {
    let (_, mut h) = base(&common(0));
    let player = h.native("Player", &[Value::Int(0)]).unwrap();

    let before = h.event_queue_capacity();
    let debit = h.prepare_gold_debit_reserving(id(&player), 3, 4).unwrap();
    let reserved = h.event_queue_capacity();
    assert!(
        reserved >= before + 4,
        "four pending sale events need their own reservation even without state triggers"
    );
    for _ in 0..4 {
        h.push_reserved_event(
            id(&player),
            None,
            crate::playercolor_host::EventSubject::default(),
            None,
        );
    }
    h.apply_gold_debit(debit);
    assert_eq!(
        h.event_queue_capacity(),
        reserved,
        "the pending events push into the reservation without growing it"
    );
    assert_eq!(h.owner_gold(id(&player)).unwrap(), Some(0));
    for _ in 0..4 {
        assert!(h.take_fired_event().is_some());
    }
    assert!(h.take_fired_event().is_none());
}

#[test]
fn poll_work_is_charged_per_registration_not_per_arena_entry() {
    let mut h = host(&common(0), 4);
    for _ in 0..40 {
        h.native("CreateTrigger", &[]).unwrap();
    }
    let trigger = h.native("CreateTrigger", &[]).unwrap();
    let player = h.native("Player", &[Value::Int(0)]).unwrap();
    let gold = convert(&mut h, "ConvertPlayerState", 1);
    let less = convert(&mut h, "ConvertLimitOp", 0);
    register(&mut h, &trigger, &player, &gold, &less, 50.0).unwrap();

    let mut work = 0;
    assert!(matches!(
        h.poll_player_state_events(&mut work),
        Err(error) if error.failure == crate::playercolor_host::Failure::Budget("player state scan")
    ));
    let mut work = 1;
    assert_eq!(h.poll_player_state_events(&mut work).unwrap(), 0);
    assert_eq!(work, 0);
}

#[test]
fn callbacks_run_before_set_player_state_returns_without_a_tick() {
    let map = "globals
trigger T = null
player P = null
integer hits = 0
integer order = 0
endglobals
function action takes nothing returns nothing
    set hits = hits + 1
    set order = order * 10 + 1
endfunction
function main takes nothing returns nothing
    set T = CreateTrigger()
    set P = Player(0)
    call TriggerAddAction(T, function action)
    call TriggerRegisterPlayerStateEvent(T, P, PLAYER_STATE_RESOURCE_GOLD, EQUAL, 10.0)
    call SetPlayerState(P, PLAYER_STATE_RESOURCE_GOLD, 10)
    set order = order * 10 + 2
    call SetPlayerState(P, PLAYER_STATE_RESOURCE_GOLD, 0)
    set order = order * 10 + 3
endfunction
";
    let (ctx, host) = base_map(&common(0), map);
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host.with_player_state_events(4).unwrap());
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    assert_eq!(vm.global("hits"), Value::Int(1));
    assert_eq!(vm.global("order"), Value::Int(123));
    assert!(vm.host().inner.take_fired_event().is_none());
}

#[test]
fn every_matching_registration_runs_before_set_player_state_returns() {
    let map = "globals
trigger A = null
trigger B = null
player P = null
integer order = 0
endglobals
function first takes nothing returns nothing
    set order = order * 10 + 1
endfunction
function second takes nothing returns nothing
    set order = order * 10 + 2
endfunction
function main takes nothing returns nothing
    set A = CreateTrigger()
    set B = CreateTrigger()
    set P = Player(0)
    call TriggerAddAction(A, function first)
    call TriggerAddAction(B, function second)
    call TriggerRegisterPlayerStateEvent(A, P, PLAYER_STATE_RESOURCE_GOLD, LESS_THAN, 50.0)
    call TriggerRegisterPlayerStateEvent(B, P, PLAYER_STATE_RESOURCE_GOLD, LESS_THAN, 50.0)
    call SetPlayerState(P, PLAYER_STATE_RESOURCE_GOLD, 30)
    set order = order * 10 + 3
endfunction
";
    let (ctx, host) = base_map(&common(0), map);
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host.with_player_state_events(4).unwrap());
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    assert_eq!(vm.global("order"), Value::Int(123));
    assert!(vm.host().inner.take_fired_event().is_none());
}

const CONDITIONS: &str = "type boolexpr extends agent
type conditionfunc extends boolexpr
type filterfunc extends boolexpr
type triggercondition extends agent
native Condition takes code f returns conditionfunc
native Filter takes code f returns filterfunc
native TriggerAddCondition takes trigger t, boolexpr c returns triggercondition
native TriggerEvaluate takes trigger t returns boolean
";

#[test]
fn conditions_gate_synchronous_actions_before_set_player_state_returns() {
    let mut common = common(0);
    let at = common.find("type playerstate extends handle").unwrap();
    common.insert_str(at, CONDITIONS);
    let map = "globals
trigger T = null
player P = null
conditionfunc C = null
integer hits = 0
integer conds = 0
endglobals
function predicate takes nothing returns boolean
    set conds = conds + 1
    return hits == 0
endfunction
function action takes nothing returns nothing
    set hits = hits + 1
endfunction
function main takes nothing returns nothing
    set T = CreateTrigger()
    set P = Player(0)
    set C = Condition(function predicate)
    call TriggerAddCondition(T, C)
    call TriggerAddAction(T, function action)
    call TriggerRegisterPlayerStateEvent(T, P, PLAYER_STATE_RESOURCE_GOLD, LESS_THAN, 50.0)
    call SetPlayerState(P, PLAYER_STATE_RESOURCE_GOLD, 30)
    call SetPlayerState(P, PLAYER_STATE_RESOURCE_GOLD, 20)
endfunction
";
    let (ctx, host) = base_map(&common, map);
    let host = host
        .with_filters(crate::playercolor_host::FilterBudget { max_records: 4 })
        .unwrap()
        .with_conditions(crate::playercolor_host::ConditionBudget { max_records: 4 })
        .unwrap()
        .with_trigger_conditions(4)
        .unwrap()
        .with_player_state_events(4)
        .unwrap();
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host);
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    assert_eq!(vm.global("conds"), Value::Int(2));
    assert_eq!(vm.global("hits"), Value::Int(1));
    assert!(vm.host().inner.take_fired_event().is_none());
}

#[test]
fn a_condition_trigger_evaluate_returns_to_its_own_callback_not_the_event() {
    let mut common = common(0);
    let at = common.find("type playerstate extends handle").unwrap();
    common.insert_str(at, CONDITIONS);
    let map = "globals
trigger A = null
trigger B = null
player P = null
conditionfunc C = null
integer order = 0
endglobals
function nested_condition takes nothing returns boolean
    set order = order * 10 + 7
    return true
endfunction
function outer_condition takes nothing returns boolean
    local boolean accepted
    set order = order * 10 + 1
    set accepted = TriggerEvaluate(B)
    set order = order * 10 + 2
    return accepted
endfunction
function action takes nothing returns nothing
    set order = order * 10 + 3
endfunction
function main takes nothing returns nothing
    set A = CreateTrigger()
    set B = CreateTrigger()
    set P = Player(0)
    set C = Condition(function nested_condition)
    call TriggerAddCondition(B, C)
    call TriggerAddCondition(A, Condition(function outer_condition))
    call TriggerAddAction(A, function action)
    call TriggerRegisterPlayerStateEvent(A, P, PLAYER_STATE_RESOURCE_GOLD, EQUAL, 10.0)
    call SetPlayerState(P, PLAYER_STATE_RESOURCE_GOLD, 10)
    set order = order * 10 + 9
endfunction
";
    let (ctx, host) = base_map(&common, map);
    let host = host
        .with_filters(crate::playercolor_host::FilterBudget { max_records: 4 })
        .unwrap()
        .with_conditions(crate::playercolor_host::ConditionBudget { max_records: 4 })
        .unwrap()
        .with_trigger_conditions(4)
        .unwrap()
        .with_player_state_events(4)
        .unwrap();
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host);
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    assert_eq!(vm.global("order"), Value::Int(17239));
    assert!(vm.host().inner.take_fired_event().is_none());
}

#[test]
fn a_condition_trigger_execute_completion_does_not_end_the_event_early() {
    let mut common = common(0);
    let globals = common.find("\nglobals\n").expect("globals") + 1;
    common.insert_str(
        globals,
        "type triggeraction extends handle
native TriggerAddAction takes trigger t, code f returns triggeraction
native TriggerExecute takes trigger t returns nothing
",
    );
    let at = common.find("type playerstate extends handle").unwrap();
    common.insert_str(at, CONDITIONS);
    let map = "globals
trigger A = null
trigger B = null
player P = null
integer order = 0
endglobals
function nested_action takes nothing returns nothing
    set order = order * 10 + 7
endfunction
function outer_condition takes nothing returns boolean
    set order = order * 10 + 1
    call TriggerExecute(B)
    set order = order * 10 + 2
    return true
endfunction
function action takes nothing returns nothing
    set order = order * 10 + 3
endfunction
function main takes nothing returns nothing
    set A = CreateTrigger()
    set B = CreateTrigger()
    set P = Player(0)
    call TriggerAddAction(B, function nested_action)
    call TriggerAddCondition(A, Condition(function outer_condition))
    call TriggerAddAction(A, function action)
    call TriggerRegisterPlayerStateEvent(A, P, PLAYER_STATE_RESOURCE_GOLD, EQUAL, 10.0)
    call SetPlayerState(P, PLAYER_STATE_RESOURCE_GOLD, 10)
    set order = order * 10 + 9
endfunction
";
    let (ctx, host) = base_map(&common, map);
    let host = host
        .with_conditions(crate::playercolor_host::ConditionBudget { max_records: 4 })
        .unwrap()
        .with_trigger_conditions(4)
        .unwrap()
        .with_player_state_events(4)
        .unwrap();
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host);
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    assert_eq!(vm.global("order"), Value::Int(17239));
    assert!(vm.host().inner.take_fired_event().is_none());
}

#[test]
fn set_player_state_inside_trigger_execute_runs_before_the_setter_returns() {
    let mut common = common(0);
    let globals = common.find("\nglobals\n").expect("globals") + 1;
    common.insert_str(
        globals,
        "type triggeraction extends handle
native TriggerAddAction takes trigger t, code f returns triggeraction
native TriggerExecute takes trigger t returns nothing
",
    );
    let map = "globals
trigger A = null
trigger B = null
player P = null
integer hits = 0
endglobals
function nested takes nothing returns nothing
    call SetPlayerState(P, PLAYER_STATE_RESOURCE_GOLD, 30)
    set hits = hits * 10 + 2
endfunction
function observe takes nothing returns nothing
    set hits = hits * 10 + 1
endfunction
function main takes nothing returns nothing
    set A = CreateTrigger()
    set B = CreateTrigger()
    set P = Player(0)
    call TriggerAddAction(A, function nested)
    call TriggerAddAction(B, function observe)
    call TriggerRegisterPlayerStateEvent(B, P, PLAYER_STATE_RESOURCE_GOLD, LESS_THAN, 50.0)
    call TriggerExecute(A)
    set hits = hits * 10 + 3
endfunction
";
    let (ctx, host) = base_map(&common, map);
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host.with_player_state_events(4).unwrap());
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    assert_eq!(vm.global("hits"), Value::Int(123));
    assert!(vm.host().inner.take_fired_event().is_none());
}

#[test]
fn nested_trigger_execute_inside_a_synchronous_action_keeps_the_outer_action_in_order() {
    let mut common = common(0);
    let globals = common.find("\nglobals\n").expect("globals") + 1;
    common.insert_str(
        globals,
        "type triggeraction extends handle
native TriggerAddAction takes trigger t, code f returns triggeraction
native TriggerExecute takes trigger t returns nothing
",
    );
    let map = "globals
trigger A = null
trigger B = null
trigger C = null
player P = null
integer order = 0
endglobals
function inner takes nothing returns nothing
    set order = order * 10 + 3
endfunction
function outer takes nothing returns nothing
    set order = order * 10 + 1
    call TriggerExecute(B)
    set order = order * 10 + 2
endfunction
function second takes nothing returns nothing
    set order = order * 10 + 4
endfunction
function main takes nothing returns nothing
    set A = CreateTrigger()
    set B = CreateTrigger()
    set C = CreateTrigger()
    set P = Player(0)
    call TriggerAddAction(B, function inner)
    call TriggerAddAction(A, function outer)
    call TriggerAddAction(C, function second)
    call TriggerRegisterPlayerStateEvent(A, P, PLAYER_STATE_RESOURCE_GOLD, EQUAL, 10.0)
    call TriggerRegisterPlayerStateEvent(C, P, PLAYER_STATE_RESOURCE_GOLD, EQUAL, 10.0)
    call SetPlayerState(P, PLAYER_STATE_RESOURCE_GOLD, 10)
    set order = order * 10 + 9
endfunction
";
    let (ctx, host) = base_map(&common, map);
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host.with_player_state_events(4).unwrap());
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    assert_eq!(vm.global("order"), Value::Int(13249));
    assert!(vm.host().inner.take_fired_event().is_none());
}

#[test]
fn sleep_in_a_synchronous_action_lets_the_other_registration_finish_before_the_native_returns() {
    let common = common(0);
    let map = "globals
trigger A = null
trigger C = null
player P = null
integer order = 0
endglobals
function outer takes nothing returns nothing
    set order = order * 10 + 1
    call TriggerSleepAction(2.0)
    set order = order * 10 + 9
endfunction
function second takes nothing returns nothing
    set order = order * 10 + 2
endfunction
function main takes nothing returns nothing
    set A = CreateTrigger()
    set C = CreateTrigger()
    set P = Player(0)
    call TriggerAddAction(A, function outer)
    call TriggerAddAction(C, function second)
    call TriggerRegisterPlayerStateEvent(A, P, PLAYER_STATE_RESOURCE_GOLD, EQUAL, 10.0)
    call TriggerRegisterPlayerStateEvent(C, P, PLAYER_STATE_RESOURCE_GOLD, EQUAL, 10.0)
    call SetPlayerState(P, PLAYER_STATE_RESOURCE_GOLD, 10)
    set order = order * 10 + 3
endfunction
";
    let (ctx, host) = base_map(&common, map);
    let mut host = host.with_player_state_events(4).unwrap();
    host.admit_thread_sleep(1).unwrap();
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host);
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    assert_eq!(vm.global("order"), Value::Int(123));
    assert_eq!(vm.host().inner.parked_thread_count(), 1);
    let ticket = vm.host().inner.next_thread_wake(30.0).unwrap().unwrap();
    assert_eq!(vm.host().inner.timer_game_time(), Some(22.0));
    vm.resume_thread(ticket).unwrap();
    assert_eq!(vm.global("order"), Value::Int(1239));
    assert_eq!(vm.host().inner.parked_thread_count(), 0);
}

fn assert_native_nested_ordering(sleep: bool) {
    let mut common = common(0);
    let at = common.find("type playerstate extends handle").unwrap();
    common.insert_str(at, CONDITIONS);
    let at = common.find("\nglobals\n").unwrap();
    common.insert_str(
        at,
        "\nnative TriggerExecute takes trigger t returns nothing\nconstant native GetTriggerPlayer takes nothing returns player\n",
    );
    let map = format!(
        r#"globals
trigger A = null
trigger C = null
trigger N = null
player P = null
string trace = ""
boolean okay = true
boolean wait = {sleep}
endglobals
function nested_condition takes nothing returns boolean
    set okay = okay and GetTriggeringTrigger() == N
    set trace = trace + "N"
    return true
endfunction
function condition takes nothing returns boolean
    local boolean accepted
    set okay = okay and GetTriggeringTrigger() == A
    set trace = trace + "B"
    set accepted = TriggerEvaluate(N)
    set okay = okay and GetTriggeringTrigger() == A
    set trace = trace + "E"
    return accepted
endfunction
function action takes nothing returns nothing
    set okay = okay and GetTriggeringTrigger() == A and GetTriggerPlayer() == P
    set trace = trace + "A"
    if GetPlayerState(P, PLAYER_STATE_RESOURCE_GOLD) == 10 then
        set trace = trace + "W"
        if wait then
            call TriggerSleepAction(2.0)
        endif
        set okay = okay and GetTriggeringTrigger() == A and GetEventPlayerState() == PLAYER_STATE_RESOURCE_GOLD
        set trace = trace + "R"
        call SetPlayerState(P, PLAYER_STATE_RESOURCE_GOLD, 20)
        set okay = okay and GetTriggeringTrigger() == A
        set trace = trace + "Z"
    endif
    set trace = trace + "L"
endfunction
function caller takes nothing returns nothing
    set trace = trace + "C"
    call SetPlayerState(P, PLAYER_STATE_RESOURCE_GOLD, 10)
    set okay = okay and GetTriggeringTrigger() == C
    set trace = trace + "D"
endfunction
function after_state takes nothing returns trigger
    return GetTriggeringTrigger()
endfunction
function main takes nothing returns nothing
    set A = CreateTrigger()
    set C = CreateTrigger()
    set N = CreateTrigger()
    set P = Player(0)
    call TriggerAddCondition(N, Condition(function nested_condition))
    call TriggerAddCondition(A, Condition(function condition))
    call TriggerAddAction(A, function action)
    call TriggerAddAction(C, function caller)
    call TriggerRegisterPlayerStateEvent(A, P, PLAYER_STATE_RESOURCE_GOLD, GREATER_THAN_OR_EQUAL, 10.0)
    call TriggerExecute(C)
    set trace = trace + "F"
endfunction
"#
    );
    let (ctx, host) = base_map(&common, &map);
    let mut host = host
        .with_filters(crate::playercolor_host::FilterBudget { max_records: 4 })
        .unwrap()
        .with_conditions(crate::playercolor_host::ConditionBudget { max_records: 4 })
        .unwrap()
        .with_trigger_conditions(4)
        .unwrap()
        .with_player_state_events(4)
        .unwrap();
    host.admit_thread_sleep(2).unwrap();
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host);
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    if sleep {
        assert_eq!(vm.global("trace"), Value::Str("CBNEAWDF".into()));
        assert_eq!(vm.host().inner.parked_thread_count(), 1);
        let ticket = vm.host().inner.next_thread_wake(30.0).unwrap().unwrap();
        vm.resume_thread(ticket).unwrap();
        assert_eq!(vm.global("trace"), Value::Str("CBNEAWDFRBNEALZL".into()));
    } else {
        assert_eq!(vm.global("trace"), Value::Str("CBNEAWRBNEALZLDF".into()));
    }
    assert_eq!(vm.global("okay"), Value::Bool(true));
    assert_eq!(vm.host().inner.parked_thread_count(), 0);
    assert!(vm.host().inner.take_fired_event().is_none());
    assert_eq!(vm.call("after_state", &[]).unwrap(), Value::Null);
}

#[test]
fn nested_mutation_and_condition_evaluation_match_the_native_order() {
    assert_native_nested_ordering(false);
}

#[test]
fn sleeping_event_resumes_its_context_and_can_deliver_a_nested_mutation() {
    assert_native_nested_ordering(true);
}

use super::{COMMON, budget as player_budget, context, id, player, slots_rule};
use crate::playercolor_host::{
    EnumHost, Limits, MapConfigBudget, ObjectHost, PlayerRemovalBudget, TriggerBudget,
    TriggerRuntimeInput,
};
use pcx_script::{Host, Value, Vm};

const EMPTY_MAP: &str = "function unused takes nothing returns nothing\nendfunction\n";
const DECLARATIONS: &str = r#"
type playerslotstate extends handle
type playergameresult extends handle
type trigger extends agent
type triggeraction extends handle
type widget extends agent
type unit extends widget
type event extends agent
type eventid extends handle
type unitevent extends eventid
type gameevent extends eventid
type playerevent extends eventid
constant native GetLocalPlayer takes nothing returns player
constant native ConvertPlayerSlotState takes integer n returns playerslotstate
constant native ConvertPlayerGameResult takes integer n returns playergameresult
constant native ConvertPlayerEvent takes integer n returns playerevent
constant native ConvertUnitEvent takes integer n returns unitevent
constant native ConvertGameEvent takes integer n returns gameevent
native RemovePlayer takes player p, playergameresult result returns nothing
native GetPlayerSlotState takes player p returns playerslotstate
native CreateTrigger takes nothing returns trigger
native DestroyTrigger takes trigger t returns nothing
native TriggerAddAction takes trigger t, code action returns triggeraction
native TriggerRegisterPlayerEvent takes trigger t, player p, playerevent e returns event
native TriggerRegisterUnitEvent takes trigger t, unit u, unitevent e returns event
native TriggerRegisterTimerEvent takes trigger t, real timeout, boolean periodic returns event
constant native GetTriggeringTrigger takes nothing returns trigger
constant native GetTriggerEventId takes nothing returns eventid
constant native GetTriggerPlayer takes nothing returns player
native TimerStart takes timer t, real timeout, boolean periodic, code callback returns nothing
native TimerGetElapsed takes timer t returns real
native TimerGetRemaining takes timer t returns real
native TimerGetTimeout takes timer t returns real
native GetExpiredTimer takes nothing returns timer
globals
constant playerslotstate PLAYER_SLOT_STATE_EMPTY=ConvertPlayerSlotState(5)
constant playerslotstate PLAYER_SLOT_STATE_PLAYING=ConvertPlayerSlotState(17)
constant playerslotstate PLAYER_SLOT_STATE_LEFT=ConvertPlayerSlotState(23)
constant playergameresult PLAYER_GAME_RESULT_VICTORY=ConvertPlayerGameResult(0)
constant playergameresult PLAYER_GAME_RESULT_DEFEAT=ConvertPlayerGameResult(7)
constant playergameresult PLAYER_GAME_RESULT_TIE=ConvertPlayerGameResult(10)
constant playergameresult PLAYER_GAME_RESULT_NEUTRAL=ConvertPlayerGameResult(99)
constant playerevent EVENT_PLAYER_DEFEAT=ConvertPlayerEvent(41)
constant playerevent EVENT_PLAYER_VICTORY=ConvertPlayerEvent(-7)
constant playerevent EVENT_PLAYER_LEAVE=ConvertPlayerEvent(83)
constant unitevent EXAMPLE_UNIT=ConvertUnitEvent(702)
constant gameevent EXAMPLE_TIMER=ConvertGameEvent(703)
endglobals
"#;
fn budget() -> PlayerRemovalBudget {
    PlayerRemovalBudget {
        max_players: 4,
        max_event_deliveries: 8,
    }
}
fn unadmitted(declarations: &str, map: &str) -> ObjectHost {
    let ctx = context(
        301,
        &format!("{COMMON}{declarations}"),
        map,
        "synthetic:player-removal",
    );
    let enums = EnumHost::checked_selected(
        ctx.scripts.shared_program(),
        &ctx.map_digest.label,
        &[
            "ConvertPlacement",
            "ConvertPlayerColor",
            "ConvertRacePref",
            "ConvertMapControl",
            "ConvertPlayerSlotState",
            "ConvertPlayerGameResult",
            "ConvertPlayerEvent",
            "ConvertUnitEvent",
            "ConvertGameEvent",
        ],
        Limits {
            max_handles: 128,
            admission_steps: 8192,
            expression_depth: 32,
            expression_work: 8192,
        },
    )
    .unwrap();
    let mut h = ObjectHost::checked(enums)
        .unwrap()
        .with_map_config(MapConfigBudget {
            text_bytes: 128,
            start_locations: 2,
        })
        .unwrap()
        .with_players(ctx.clone(), slots_rule(301, -3, 9), player_budget())
        .unwrap()
        .with_timer_runtime(0.)
        .unwrap()
        .with_trigger_admission(TriggerBudget {
            max_triggers: 8,
            max_unit_events: 0,
            max_actions: 8,
        })
        .unwrap()
        .with_trigger_runtime(TriggerRuntimeInput {
            initial_enabled: true,
            timer_event_global: "EXAMPLE_TIMER".into(),
            max_timed_events: 0,
            max_parked_threads: 0,
        })
        .unwrap()
        .with_player_events(8)
        .unwrap();
    h.admit_local_player(&ctx, 0).unwrap();
    let p = player(&mut h, 0);
    let controller = h.native("ConvertMapControl", &[Value::Int(91)]).unwrap();
    h.native("SetPlayerController", &[p, controller]).unwrap();
    let playing = symbol(&mut h, "ConvertPlayerSlotState", 17);
    let empty = symbol(&mut h, "ConvertPlayerSlotState", 5);
    h.admit_startup_player_slot_numbers(
        &ctx,
        &[(0, id(&playing)), (1, id(&playing)), (2, id(&empty))],
    )
    .unwrap();
    h
}
fn host_with(budget: PlayerRemovalBudget) -> ObjectHost {
    unadmitted(DECLARATIONS, EMPTY_MAP)
        .with_player_removal(crate::player_removal_rules::tests::fixture(), budget)
        .unwrap()
}
fn symbol(h: &mut ObjectHost, convert: &str, value: i32) -> Value {
    h.native(convert, &[Value::Int(value)]).unwrap()
}
fn remove(h: &mut ObjectHost, p: Value, value: i32) {
    let r = symbol(h, "ConvertPlayerGameResult", value);
    h.native("RemovePlayer", &[p, r]).unwrap();
}
fn register(h: &mut ObjectHost, player: Value, event: i32) -> Value {
    let t = h.native("CreateTrigger", &[]).unwrap();
    let e = symbol(h, "ConvertPlayerEvent", event);
    h.native("TriggerRegisterPlayerEvent", &[t.clone(), player, e])
        .unwrap();
    t
}
#[test]
fn removal_retains_player_controller_and_first_result_without_reallocating_handles() {
    let mut h = host_with(budget());
    let p = player(&mut h, 0);
    let other = player(&mut h, 1);
    let left = symbol(&mut h, "ConvertPlayerSlotState", 23);
    let result = symbol(&mut h, "ConvertPlayerGameResult", 7);
    let controller = h
        .native("GetPlayerController", std::slice::from_ref(&p))
        .unwrap();
    let handles = h.inner().handles();
    h.native("RemovePlayer", &[p.clone(), result.clone()])
        .unwrap();
    assert_eq!(h.inner().handles(), handles);
    assert_eq!(h.native("Player", &[Value::Int(0)]).unwrap(), p);
    assert_eq!(
        h.native("GetPlayerController", std::slice::from_ref(&p))
            .unwrap(),
        controller
    );
    assert_eq!(
        h.native("GetPlayerSlotState", std::slice::from_ref(&p))
            .unwrap(),
        left
    );
    assert_eq!(
        h.player_removal_state()
            .unwrap()
            .get(id(&p))
            .unwrap()
            .game_result,
        id(&result)
    );
    assert!(h.player_removal_state().unwrap().get(id(&p)).unwrap().left);
    let other_slot = h
        .native("GetPlayerSlotState", std::slice::from_ref(&other))
        .unwrap();
    remove(&mut h, p.clone(), 0);
    assert_eq!(h.player_removal_state().unwrap().records().len(), 1);
    assert_eq!(
        h.player_removal_state()
            .unwrap()
            .get(id(&p))
            .unwrap()
            .game_result,
        id(&result)
    );
    assert_eq!(
        h.native("GetPlayerSlotState", &[other]).unwrap(),
        other_slot
    );
}
#[test]
fn victory_preserves_playing_while_tie_neutral_and_defeat_leave() {
    for (result, left) in [(0, false), (7, true), (10, true), (99, true)] {
        let mut h = host_with(budget());
        let p = player(&mut h, 0);
        let expected = symbol(&mut h, "ConvertPlayerSlotState", if left { 23 } else { 17 });
        remove(&mut h, p.clone(), result);
        assert_eq!(
            h.native("GetPlayerSlotState", std::slice::from_ref(&p))
                .unwrap(),
            expected
        );
        assert_eq!(
            h.player_removal_state().unwrap().get(id(&p)).unwrap().left,
            left
        );
        remove(&mut h, p.clone(), if left { 0 } else { 7 });
        assert_eq!(h.native("GetPlayerSlotState", &[p]).unwrap(), expected);
    }
}
#[test]
fn result_events_match_registered_player_and_keep_typed_response_context() {
    for (result, event) in [(0, -7), (7, 41), (10, 0), (99, 0)] {
        let mut h = host_with(budget());
        let p = player(&mut h, 0);
        let other = player(&mut h, 1);
        let defeated = register(&mut h, p.clone(), 41);
        let victorious = register(&mut h, p.clone(), -7);
        register(&mut h, p.clone(), 83);
        register(&mut h, other, 41);
        let expected = if event == 41 {
            Some(defeated)
        } else if event == -7 {
            Some(victorious)
        } else {
            None
        };
        remove(&mut h, p.clone(), result);
        if let Some(trigger) = expected {
            let fired = h.take_fired_event().expect("matching result registration");
            assert_eq!(Value::Handle(fired.trigger), trigger);
            h.begin_fired_event(&fired).unwrap().unwrap();
            assert_eq!(h.native("GetTriggerPlayer", &[]).unwrap(), p);
            let expected_event = symbol(&mut h, "ConvertPlayerEvent", event);
            assert_eq!(h.native("GetTriggerEventId", &[]).unwrap(), expected_event);
            h.finish_fired_event();
            assert_eq!(h.native("GetTriggerPlayer", &[]).unwrap(), Value::Null);
        }
        assert!(h.take_fired_event().is_none(), "no unrelated/leave event");
        remove(&mut h, p, result);
        assert!(h.take_fired_event().is_none(), "repeat does not fire twice");
    }
}
#[test]
fn record_and_callback_budgets_refuse_before_slot_or_result_publication() {
    for limited in [
        PlayerRemovalBudget {
            max_players: 0,
            max_event_deliveries: 8,
        },
        PlayerRemovalBudget {
            max_players: 4,
            max_event_deliveries: 0,
        },
    ] {
        let mut h = host_with(limited);
        let p = player(&mut h, 0);
        register(&mut h, p.clone(), 41);
        let r = symbol(&mut h, "ConvertPlayerGameResult", 7);
        let prior = h
            .native("GetPlayerSlotState", std::slice::from_ref(&p))
            .unwrap();
        assert!(h.native("RemovePlayer", &[p.clone(), r]).is_err());
        assert_eq!(
            h.native("GetPlayerSlotState", std::slice::from_ref(&p))
                .unwrap(),
            prior
        );
        assert!(h.player_removal_state().unwrap().records().is_empty());
        assert!(h.take_fired_event().is_none());
    }
}
#[test]
fn null_nonplaying_and_invalid_requests_preserve_state() {
    let mut h = host_with(budget());
    let p = player(&mut h, 0);
    let empty = player(&mut h, 2);
    let r = symbol(&mut h, "ConvertPlayerGameResult", 7);
    assert_eq!(
        h.native("RemovePlayer", &[Value::Null, r.clone()]).unwrap(),
        Value::Null
    );
    h.native("RemovePlayer", &[empty, r.clone()]).unwrap();
    for args in [
        vec![],
        vec![p.clone()],
        vec![Value::Int(0), r.clone()],
        vec![p.clone(), Value::Int(7)],
        vec![r.clone(), r.clone()],
        vec![p.clone(), p.clone()],
    ] {
        assert!(h.native("RemovePlayer", &args).is_err());
    }
    h.invalidate_startup_player_slots();
    assert!(h.native("RemovePlayer", &[p, r]).is_err());
    assert!(h.player_removal_state().unwrap().records().is_empty());
}
#[test]
fn vm_defeat_continues_after_remove_with_same_controller_and_player_identity() {
    let map = r#"globals
boolean continued=false
endglobals
function authored_defeat takes nothing returns nothing
local player p=Player(0)
call RemovePlayer(p,PLAYER_GAME_RESULT_DEFEAT)
if GetPlayerController(p)==M and GetPlayerSlotState(p)==PLAYER_SLOT_STATE_LEFT then
set continued=true
endif
endfunction
function check takes nothing returns boolean
return continued
endfunction
"#;
    let h = unadmitted(DECLARATIONS, map)
        .with_player_removal(crate::player_removal_rules::tests::fixture(), budget())
        .unwrap();
    let program = h.player_context().unwrap().scripts.shared_program();
    let mut vm = Vm::new(&program, h);
    vm.init_globals().unwrap();
    vm.call("authored_defeat", &[]).unwrap();
    assert_eq!(vm.call("check", &[]).unwrap(), Value::Bool(true));
    assert_eq!(vm.host().player_removal_state().unwrap().records().len(), 1);
}
#[test]
fn missing_admission_and_mismatched_selected_contracts_refuse() {
    let mut h = unadmitted(DECLARATIONS, EMPTY_MAP);
    let p = player(&mut h, 0);
    let r = symbol(&mut h, "ConvertPlayerGameResult", 7);
    assert!(h.native("RemovePlayer", &[p, r]).is_err());
    for decl in [
        DECLARATIONS.replace(
            "playergameresult result returns nothing",
            "integer result returns nothing",
        ),
        DECLARATIONS.replace(
            "native RemovePlayer takes",
            "constant native RemovePlayer takes",
        ),
        DECLARATIONS.replace("ConvertPlayerGameResult(7)", "ConvertPlayerGameResult(8)"),
    ] {
        assert!(
            unadmitted(&decl, EMPTY_MAP)
                .with_player_removal(crate::player_removal_rules::tests::fixture(), budget())
                .is_err()
        );
    }
}

#[test]
fn declaration_source_budget_and_optional_absence_are_checked() {
    let decl = DECLARATIONS.replace(
        "native RemovePlayer takes player p, playergameresult result returns nothing\n",
        "",
    );
    let h = unadmitted(&decl, EMPTY_MAP)
        .with_player_removal(crate::player_removal_rules::tests::fixture(), budget())
        .unwrap();
    assert!(h.player_removal_state().is_none());
    let map = format!(
        "native RemovePlayer takes player p, playergameresult result returns nothing\n{EMPTY_MAP}"
    );
    let error = unadmitted(&decl, &map)
        .with_player_removal(crate::player_removal_rules::tests::fixture(), budget())
        .err()
        .expect("cross-source RemovePlayer");
    assert!(matches!(
        error.failure,
        crate::playercolor_host::Failure::Source
    ));
    assert!(
        unadmitted(DECLARATIONS, EMPTY_MAP)
            .with_player_removal(
                crate::player_removal_rules::tests::fixture(),
                PlayerRemovalBudget {
                    max_players: usize::MAX,
                    max_event_deliveries: 1
                }
            )
            .is_err()
    );
}

use super::*;

const SCRIPT: &str = r#"
globals
unit U=null
integer Seen=0
integer Issued=0
integer Current=0
unit Response=null
integer CallbackMode=0
endglobals
function main takes nothing returns nothing
set U=CreateUnit(Player(4),'u001',16.,16.,0.)
endfunction
function observe takes nothing returns nothing
set Seen=Seen+1
set Issued=GetIssuedOrderId()
set Current=GetUnitCurrentOrder(U)
set Response=GetTriggerUnit()
if CallbackMode==1 then
call IssuePointOrder(U,"gamma",72.,16.)
elseif CallbackMode==2 then
call IssueImmediateOrder(U,"beta")
elseif CallbackMode==3 then
call SetUnitMoveSpeed(U,-1.)
elseif CallbackMode==4 then
call PauseUnit(U,true)
endif
endfunction
function listen takes nothing returns nothing
local trigger t=CreateTrigger()
call TriggerRegisterUnitEvent(t,U,EVENT_UNIT_ISSUED_ORDER)
call TriggerAddAction(t,function observe)
set t=CreateTrigger()
call TriggerRegisterPlayerUnitEvent(t,Player(4),EVENT_PLAYER_UNIT_ISSUED_ORDER,null)
call TriggerAddAction(t,function observe)
endfunction
function listen_player takes nothing returns nothing
local trigger t=CreateTrigger()
call TriggerRegisterPlayerUnitEvent(t,Player(4),EVENT_PLAYER_UNIT_ISSUED_ORDER,null)
call TriggerAddAction(t,function observe)
endfunction
function off takes nothing returns boolean
return IssueImmediateOrder(U,"delta")
endfunction
function on takes nothing returns boolean
return IssueImmediateOrder(U,"repair")
endfunction
function onpeer takes nothing returns boolean
return IssueImmediateOrder(U,"gamma")
endfunction
function offid takes nothing returns boolean
return IssueImmediateOrderById(U,OrderId("delta"))
endfunction
function move takes nothing returns boolean
return IssuePointOrder(U,"gamma",72.,16.)
endfunction
function override_move takes nothing returns nothing
set CallbackMode=1
endfunction
function override_stop takes nothing returns nothing
set CallbackMode=2
endfunction
function callback_fault takes nothing returns nothing
set CallbackMode=3
endfunction
function callback_pause takes nothing returns nothing
set CallbackMode=4
endfunction
"#;
const PROFILE: &[u8] = b"[X002]\nOrderon=repair\nOrderoff=delta\n";
/// Same-group peer ability `X004` of class `X003`, toggled by `gamma`/`beta`.
const PEER_PROFILE: &[u8] =
    b"[X002]\nOrderon=repair\nOrderoff=delta\n[X004]\nOrderon=gamma\nOrderoff=beta\n";
const FIRST_TICK: f32 = 0.5;
const GROUP: u32 = 1;
fn enable() -> Option<crate::autocast_rules::Enable> {
    Some(crate::autocast_rules::Enable {
        group: GROUP,
        first_tick: FIRST_TICK,
    })
}
fn open(default: &str, present: bool) -> (Vm, HandleId) {
    open_with_profile(default, present, PROFILE)
}
fn open_with_profile(default: &str, present: bool, profile: &[u8]) -> (Vm, HandleId) {
    open_setup(
        default,
        if present { "X002" } else { "_" },
        profile,
        enable(),
    )
}
fn open_setup(
    default: &str,
    abilities: &str,
    profile: &[u8],
    enable: Option<crate::autocast_rules::Enable>,
) -> (Vm, HandleId) {
    let mut vm = open_setup_vm(default, abilities, profile, enable);
    vm.call("main", &[]).unwrap();
    let Value::Handle(unit) = vm.global("U") else {
        panic!("unit")
    };
    (vm, unit)
}
fn open_setup_vm(
    default: &str,
    abilities: &str,
    profile: &[u8],
    enable: Option<crate::autocast_rules::Enable>,
) -> Vm {
    let mut fields = hero_fields("STR");
    fields.retain(|(id, _)| id != b"A008");
    fields.push(column(
        b"A008",
        SlkId::UnitAbilities,
        "abilList",
        FieldKind::Token,
        abilities,
    ));
    fields.push(column(
        b"A010",
        SlkId::UnitAbilities,
        "auto",
        FieldKind::Token,
        default,
    ));
    let (registry, activity) = super::super::activity::rules_with_repair();
    let class = |code: &[u8; 4], on: &str, off: &str| crate::autocast_rules::Class {
        code: *code,
        on: registry.id(on).unwrap(),
        off: registry.id(off).unwrap(),
        enable,
    };
    let autocast = crate::autocast_rules::Rules {
        origin: registry.binding().origin.clone(),
        image_sha256: registry.binding().image_sha256,
        classes: [
            Some(class(b"X001", "repair", "delta")),
            // The peer class is registered only when a unit carries it: `gamma` and `beta` are
            // ordinary orders in the other tests.
            abilities
                .contains("X004")
                .then(|| class(b"X003", "gamma", "beta")),
        ]
        .into_iter()
        .flatten()
        .collect(),
        groups: vec![(*b"X001", GROUP), (*b"X003", GROUP)],
        alarm_code: Some(*b"X005"),
    };
    let mut rules = hero_rules().0;
    rules.binding.image_sha256 = autocast.image_sha256;
    let rules = rules.with_autocast(Ok(autocast));
    let data = sylk(
        &["alias", "code", "levels", "reqLevel", "levelSkip", "hero"],
        &[
            &["X002", "X001", "1", "0", "0", "0"],
            &["X004", "X003", "1", "0", "0", "0"],
            &["X006", "X005", "1", "0", "0", "0"],
        ],
    );
    let tables = DefaultTables::from_bytes(
        Identity {
            install: source().install,
            edition_version: "fixture".into(),
        },
        &[(SlkId::AbilityData.path(), &data)],
        &[("Units\\HumanAbilityFunc.txt", profile)],
        pcx_objects::editor::EditorReadLimits {
            max_listed_paths: 4,
            max_asset_bytes: 65536,
            max_aggregate_input_bytes: 131072,
        },
    )
    .unwrap();
    let extra = format!(
        "{}\n{}\ntype playerunitevent extends eventid\nconstant native ConvertPlayerUnitEvent takes integer i returns playerunitevent\nnative RemoveUnit takes unit u returns nothing\nnative UnitIgnoreAlarm takes unit whichUnit, boolean flag returns boolean\nnative UnitIgnoreAlarmToggled takes unit whichUnit returns boolean\ntype gamecache extends agent\nnative InitGameCache takes string file returns gamecache\nnative StoreUnit takes gamecache c, string m, string k, unit u returns boolean\nconstant native GetIssuedOrderId takes nothing returns integer\nnative TriggerRegisterUnitEvent takes trigger t, unit u, unitevent e returns event\nnative TriggerRegisterPlayerUnitEvent takes trigger t, player p, playerunitevent e, boolexpr f returns event\nglobals\nconstant unitevent EVENT_UNIT_ISSUED_ORDER=ConvertUnitEvent(123)\nconstant playerunitevent EVENT_PLAYER_UNIT_ISSUED_ORDER=ConvertPlayerUnitEvent(-456)\nendglobals\n",
        enumeration::TRIGGERS,
        super::super::activity::ORDERS
    );
    let h = position_host_admitting(1000, 16, SCRIPT, &extra, &fields, |h| h)
        .with_order_registry(registry)
        .unwrap()
        .with_player_unit_events(8)
        .unwrap()
        .with_hero_skills(
            tables,
            crate::scripts::AssetIdentity {
                source: source().install,
                edition: "fixture".into(),
                version: "fixture".into(),
            },
            Default::default(),
            rules,
            crate::playercolor_host::HeroBudget {
                abilities_per_unit: 8,
                overlay_objects: 0,
                modifications_per_ability: 0,
            },
        )
        .unwrap();
    let mut h = h;
    h.admit_unit_activity(activity, 65536).unwrap();
    let context = Arc::clone(h.player_context().unwrap());
    context.scripts.clone().owned_vm(h)
}
#[test]
fn source_default_off_and_missing_ability_are_strict_non_mutating_false() {
    for (default, present) in [("_", true), ("X001", false)] {
        let (mut vm, u) = open(default, present);
        vm.call("listen", &[]).unwrap();
        vm.call("move", &[]).unwrap();
        let before = vm.host().inner.executed_unit(u).unwrap().clone();
        let handles = vm.host().inner.inner().handles();
        assert_eq!(vm.call("off", &[]).unwrap(), Value::Bool(false));
        assert_eq!(vm.call("offid", &[]).unwrap(), Value::Bool(false));
        assert_eq!(vm.global("Seen"), Value::Int(0));
        assert_eq!(vm.host().inner.executed_unit(u).unwrap(), &before);
        assert_eq!(vm.host().inner.inner().handles(), handles);
    }
}
#[test]
fn changed_off_interrupts_motion_and_all_callbacks_observe_ephemeral_order() {
    let (mut vm, u) = open("X001", true);
    vm.call("listen", &[]).unwrap();
    assert_eq!(vm.call("move", &[]).unwrap(), Value::Bool(true));
    assert_eq!(vm.call("off", &[]).unwrap(), Value::Bool(true));
    assert_eq!(vm.global("Seen"), Value::Int(2));
    let off = order(&mut vm, "delta");
    assert_eq!(vm.global("Issued"), Value::Int(off));
    assert_eq!(vm.global("Current"), Value::Int(off));
    assert_eq!(vm.global("Response"), Value::Handle(u));
    assert_eq!(current(&mut vm, u), 0);
    assert_eq!(
        vm.host().inner.unit_autocast_enabled(u, *b"X002").unwrap(),
        Some(false)
    );
    let before = vm.host().inner.executed_unit(u).unwrap().clone();
    assert_eq!(vm.call("offid", &[]).unwrap(), Value::Bool(false));
    assert_eq!(vm.global("Seen"), Value::Int(2));
    assert_eq!(vm.host().inner.executed_unit(u).unwrap(), &before);
}
#[test]
fn callback_new_move_or_stop_survives_off_settlement() {
    for mode in ["override_move", "override_stop"] {
        let (mut vm, u) = open("X001", true);
        vm.call("listen", &[]).unwrap();
        vm.call(mode, &[]).unwrap();
        assert_eq!(vm.call("offid", &[]).unwrap(), Value::Bool(true));
        let expected = if mode == "override_move" {
            order(&mut vm, "gamma")
        } else {
            0
        };
        assert_eq!(current(&mut vm, u), expected);
    }
}

fn order(vm: &mut Vm, name: &str) -> i32 {
    let Value::Int(n) = vm
        .host()
        .inner
        .native("OrderId", &[Value::Str(name.into())])
        .unwrap()
    else {
        panic!("order")
    };
    n
}
fn current(vm: &mut Vm, u: HandleId) -> i32 {
    let Value::Int(n) = vm
        .host()
        .inner
        .native("GetUnitCurrentOrder", &[Value::Handle(u)])
        .unwrap()
    else {
        panic!("current")
    };
    n
}

#[test]
fn frame_preflight_refusal_and_no_change_paths_do_not_mutate_units_or_events() {
    let (mut vm, u) = open("X001", true);
    vm.call("listen", &[]).unwrap();
    vm.call("move", &[]).unwrap();
    let before = vm.host().inner.executed_unit(u).unwrap().clone();
    let handles = vm.host().inner.inner().handles();
    let capacity = vm.host().inner.event_queue_capacity();
    vm.host().inner.test_synchronous_frame_limit();
    assert!(
        vm.call("off", &[])
            .unwrap_err()
            .message
            .contains("nested synchronous callbacks")
    );
    assert_eq!(vm.host().inner.executed_unit(u).unwrap(), &before);
    assert_eq!(vm.host().inner.inner().handles(), handles);
    assert_eq!(vm.host().inner.event_queue_capacity(), capacity);
    assert_eq!(vm.global("Seen"), Value::Int(0));
    let (mut vm, u) = open("_", true);
    vm.host().inner.test_synchronous_frame_limit();
    assert_eq!(vm.call("off", &[]).unwrap(), Value::Bool(false));
    assert_eq!(
        vm.host().inner.unit_autocast_enabled(u, *b"X002").unwrap(),
        Some(false)
    );
}
#[test]
fn undecoded_enable_refuses_and_pause_or_retirement_preserves_typed_contracts() {
    let (mut vm, u) = open_setup("_", "X002", PROFILE, None);
    let before = vm.host().inner.executed_unit(u).unwrap().clone();
    let error = vm
        .host()
        .inner
        .native(
            "IssueImmediateOrder",
            &[Value::Handle(u), Value::Str("repair".into())],
        )
        .unwrap_err();
    assert!(error.contains("enabling of"), "{error}");
    assert_eq!(vm.host().inner.executed_unit(u).unwrap(), &before);
    let (mut vm, u) = open("X001", true);
    vm.host()
        .inner
        .native("PauseUnit", &[Value::Handle(u), Value::Bool(true)])
        .unwrap();
    assert_eq!(vm.call("off", &[]).unwrap(), Value::Bool(false));
    assert_eq!(
        vm.host().inner.unit_autocast_enabled(u, *b"X002").unwrap(),
        Some(true)
    );
    vm.host()
        .inner
        .native("RemoveUnit", &[Value::Handle(u)])
        .unwrap();
    assert_eq!(vm.call("offid", &[]).unwrap(), Value::Bool(false));
    assert!(
        vm.host()
            .inner
            .native(
                "IssueImmediateOrder",
                &[
                    Value::Handle(HandleId::new(std::num::NonZeroU64::new(999999).unwrap())),
                    Value::Str("delta".into())
                ]
            )
            .is_err()
    );
}

#[test]
fn callback_abort_drops_pending_off_context_and_pause_is_not_restored() {
    let (mut vm, u) = open("X001", true);
    vm.call("listen", &[]).unwrap();
    vm.call("callback_fault", &[]).unwrap();
    assert!(vm.call("off", &[]).is_err());
    assert_eq!(current(&mut vm, u), 0);
    assert_eq!(
        vm.host().inner.unit_autocast_enabled(u, *b"X002").unwrap(),
        Some(false)
    );
    let (mut vm, u) = open("X001", true);
    vm.call("listen", &[]).unwrap();
    vm.call("callback_pause", &[]).unwrap();
    assert_eq!(vm.call("off", &[]).unwrap(), Value::Bool(true));
    assert_eq!(current(&mut vm, u), 0);
    assert_eq!(
        vm.host()
            .inner
            .native("IsUnitPaused", &[Value::Handle(u)])
            .unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn changed_state_cache_refusal_is_atomic_and_malformed_defaults_are_not_false() {
    let (mut vm, u) = open("X001", true);
    assert_eq!(vm.call("off", &[]).unwrap(), Value::Bool(true));
    let h = &mut vm.host().inner;
    let memory = Arc::new(crate::game_cache::tests::Memory::default());
    h.admit_game_caches(memory.clone(), crate::game_cache::tests::budget())
        .unwrap();
    let cache = h
        .native("InitGameCache", &[Value::Str("fixture".into())])
        .unwrap();
    let before = h.executed_unit(u).unwrap().clone();
    let handles = h.inner().handles();
    let error = h
        .native(
            "StoreUnit",
            &[
                cache,
                Value::Str("mission".into()),
                Value::Str("hero".into()),
                Value::Handle(u),
            ],
        )
        .unwrap_err();
    assert!(
        error.contains("does not encode altered autocast state"),
        "{error}"
    );
    assert_eq!(h.executed_unit(u).unwrap(), &before);
    assert_eq!(h.inner().handles(), handles);
    assert!(memory.bytes.lock().unwrap().is_none());
    let mut vm = open_setup_vm("bad", "X002", PROFILE, enable());
    assert!(vm.call("main", &[]).is_err());
    assert_eq!(vm.host().inner.executed_units().count(), 0);
}

#[test]
fn toggle_keys_fall_back_to_order_unorder_when_no_orderoff_is_authored() {
    let profile = b"[X002]\nOrder=repair\nUnorder=delta\n";
    let (mut vm, u) = open_with_profile("_", true, profile);
    vm.call("listen", &[]).unwrap();
    let before = vm.host().inner.executed_unit(u).unwrap().clone();
    assert_eq!(vm.call("off", &[]).unwrap(), Value::Bool(false));
    assert_eq!(vm.host().inner.executed_unit(u).unwrap(), &before);
    assert_eq!(vm.global("Seen"), Value::Int(0));
    let (mut vm, u) = open_with_profile("X001", true, profile);
    vm.call("listen", &[]).unwrap();
    assert_eq!(vm.call("off", &[]).unwrap(), Value::Bool(true));
    assert_eq!(
        vm.host().inner.unit_autocast_enabled(u, *b"X002").unwrap(),
        Some(false)
    );
}

#[test]
fn toggle_ids_come_from_the_class_when_only_a_cast_order_is_authored() {
    // The Mana Battery authors `Order=recharge` and no Orderon, Orderoff or Unorder.
    let profile = b"[X002]\nOrder=repair\n";
    let (mut vm, u) = open_with_profile("X001", true, profile);
    vm.call("listen", &[]).unwrap();
    assert_eq!(vm.call("off", &[]).unwrap(), Value::Bool(true));
    assert_eq!(
        vm.host().inner.unit_autocast_enabled(u, *b"X002").unwrap(),
        Some(false)
    );
}

#[test]
fn enable_on_a_unit_without_the_ability_issues_nothing() {
    // The unit lacks the class ability, so the order is not issued rather than
    // refused as an unmodelled enable.
    let (mut vm, u) = open("_", false);
    let issued = vm
        .host()
        .inner
        .native(
            "IssueImmediateOrder",
            &[Value::Handle(u), Value::Str("repair".into())],
        )
        .unwrap();
    assert_eq!(issued, Value::Bool(false));
}

fn state(vm: &mut Vm, u: HandleId, alias: &[u8; 4]) -> Option<bool> {
    vm.host().inner.unit_autocast_enabled(u, *alias).unwrap()
}
fn advance(vm: &mut Vm, now: f64) -> Result<usize, String> {
    let h = &mut vm.host().inner;
    assert!(h.next_timer_expiry(now, &mut 1000).unwrap().is_none());
    h.advance_unit_activity(now, &mut 65536)
        .map_err(|e| format!("{e:?}"))
}
#[test]
fn default_activation_uses_class_code_and_due_boundary_without_an_order() {
    let mut vm = open_setup_vm("X001", "X002", PROFILE, enable());
    vm.call("listen_player", &[]).unwrap();
    vm.call("main", &[]).unwrap();
    let Value::Handle(u) = vm.global("U") else {
        panic!("unit")
    };
    assert_eq!(vm.global("Seen"), Value::Int(0));
    let start = vm.host().inner.timer_game_time().unwrap();
    assert_eq!(state(&mut vm, u, b"X002"), Some(true));
    assert!(!modified(&mut vm, u));
    vm.call("listen", &[]).unwrap();
    let due = start + f64::from(FIRST_TICK);
    advance(&mut vm, due.next_down()).unwrap();
    assert_eq!(vm.call("on", &[]).unwrap(), Value::Bool(false));
    assert_eq!(vm.global("Seen"), Value::Int(0));
    assert_eq!(current(&mut vm, u), 0);
    let before = vm.host().inner.executed_unit(u).unwrap().clone();
    let random = vm.host().inner.script_random_state().cloned();
    let error = advance(&mut vm, due).unwrap_err();
    assert!(error.contains("automatic cast of X002"), "{error}");
    assert_eq!(vm.host().inner.executed_unit(u).unwrap(), &before);
    assert_eq!(vm.host().inner.script_random_state().cloned(), random);
    assert!(
        advance(&mut vm, due)
            .unwrap_err()
            .contains("automatic cast of X002")
    );
    assert_eq!(vm.host().inner.executed_unit(u).unwrap(), &before);
}
#[test]
fn default_activation_delay_starts_at_creation_in_an_advanced_session() {
    let mut vm = open_setup_vm("X001", "X002", PROFILE, enable());
    let created_at = vm.host().inner.timer_game_time().unwrap() + 7.;
    advance(&mut vm, created_at).unwrap();
    vm.call("main", &[]).unwrap();
    let due = created_at + f64::from(FIRST_TICK);
    advance(&mut vm, due.next_down()).unwrap();
    assert!(
        advance(&mut vm, due)
            .unwrap_err()
            .contains("automatic cast of X002")
    );
}
#[test]
fn default_activation_off_and_removal_disarm_and_reenable_restarts_delay() {
    let (mut vm, _) = open("X001", true);
    let start = vm.host().inner.timer_game_time().unwrap();
    assert_eq!(vm.call("off", &[]).unwrap(), Value::Bool(true));
    advance(&mut vm, start + 2.).unwrap();
    assert_eq!(vm.call("on", &[]).unwrap(), Value::Bool(true));
    advance(&mut vm, start + 2. + f64::from(FIRST_TICK) / 2.).unwrap();
    assert!(advance(&mut vm, start + 2. + f64::from(FIRST_TICK)).is_err());

    let (mut vm, u) = open("X001", true);
    vm.host()
        .inner
        .native("RemoveUnit", &[Value::Handle(u)])
        .unwrap();
    advance(&mut vm, start + 10.).unwrap();
}
#[test]
fn undecoded_or_invalid_default_activation_refuses_before_unit_allocation() {
    for rule in [
        None,
        Some(crate::autocast_rules::Enable {
            group: GROUP,
            first_tick: f32::NAN,
        }),
    ] {
        let mut vm = open_setup_vm("X001", "X002", PROFILE, rule);
        assert!(vm.call("main", &[]).is_err());
        assert_eq!(vm.host().inner.executed_units().count(), 0);
        let start = vm.host().inner.timer_game_time().unwrap();
        advance(&mut vm, start + 10.).unwrap();
    }
    for (default, present) in [("X001", false), ("X002", true)] {
        let (mut vm, _) = open(default, present);
        let start = vm.host().inner.timer_game_time().unwrap();
        advance(&mut vm, start + 10.).unwrap();
    }
}
fn modified(vm: &mut Vm, u: HandleId) -> bool {
    let record = vm.host().inner.executed_unit(u).unwrap();
    record.abilities.as_ref().unwrap().autocast_modified()
}
#[test]
fn enable_flips_state_emits_the_order_callbacks_and_reports_false_when_repeated() {
    let (mut vm, u) = open("_", true);
    vm.call("listen", &[]).unwrap();
    assert_eq!(vm.call("on", &[]).unwrap(), Value::Bool(true));
    assert_eq!(vm.global("Seen"), Value::Int(2));
    let on = order(&mut vm, "repair");
    assert_eq!(vm.global("Issued"), Value::Int(on));
    assert_eq!(vm.global("Current"), Value::Int(on));
    assert_eq!(vm.global("Response"), Value::Handle(u));
    assert_eq!(current(&mut vm, u), 0);
    assert_eq!(state(&mut vm, u, b"X002"), Some(true));
    let before = vm.host().inner.executed_unit(u).unwrap().clone();
    assert_eq!(vm.call("on", &[]).unwrap(), Value::Bool(false));
    assert_eq!(vm.global("Seen"), Value::Int(2));
    assert_eq!(vm.host().inner.executed_unit(u).unwrap(), &before);
    assert_eq!(vm.call("off", &[]).unwrap(), Value::Bool(true));
    assert_eq!(state(&mut vm, u, b"X002"), Some(false));
    assert!(!modified(&mut vm, u));
}
#[test]
fn enable_after_off_restores_the_authored_default_without_a_state_override() {
    let (mut vm, u) = open("X001", true);
    assert_eq!(vm.call("off", &[]).unwrap(), Value::Bool(true));
    assert_eq!(state(&mut vm, u, b"X002"), Some(false));
    assert_eq!(vm.call("on", &[]).unwrap(), Value::Bool(true));
    assert_eq!(state(&mut vm, u, b"X002"), Some(true));
    assert!(!modified(&mut vm, u));
}
#[test]
fn enable_interrupts_motion_like_off() {
    let (mut vm, u) = open("_", true);
    assert_eq!(vm.call("move", &[]).unwrap(), Value::Bool(true));
    assert_ne!(current(&mut vm, u), 0);
    assert_eq!(vm.call("on", &[]).unwrap(), Value::Bool(true));
    assert_eq!(current(&mut vm, u), 0);
}
#[test]
fn armed_first_tick_refuses_when_game_time_reaches_it() {
    let (mut vm, _) = open("_", true);
    let start = vm.host().inner.timer_game_time().unwrap();
    assert_eq!(vm.call("on", &[]).unwrap(), Value::Bool(true));
    advance(&mut vm, start + f64::from(FIRST_TICK) / 2.).unwrap();
    let error = advance(&mut vm, start + f64::from(FIRST_TICK)).unwrap_err();
    assert!(error.contains("automatic cast"), "{error}");
    assert!(error.contains("not implemented"), "{error}");
}
#[test]
fn off_or_removal_before_the_first_tick_disarms_it() {
    let (mut vm, u) = open("_", true);
    let start = vm.host().inner.timer_game_time().unwrap();
    assert_eq!(vm.call("on", &[]).unwrap(), Value::Bool(true));
    assert_eq!(vm.call("off", &[]).unwrap(), Value::Bool(true));
    advance(&mut vm, start + 10.).unwrap();
    assert_eq!(state(&mut vm, u, b"X002"), Some(false));
    let (mut vm, u) = open("_", true);
    let start = vm.host().inner.timer_game_time().unwrap();
    assert_eq!(vm.call("on", &[]).unwrap(), Value::Bool(true));
    vm.host()
        .inner
        .native("RemoveUnit", &[Value::Handle(u)])
        .unwrap();
    advance(&mut vm, start + 10.).unwrap();
}
#[test]
fn enable_switches_same_group_peers_off_without_their_own_events() {
    let (mut vm, u) = open_setup("X003", "X002,X004", PEER_PROFILE, enable());
    vm.call("listen", &[]).unwrap();
    assert_eq!(state(&mut vm, u, b"X002"), Some(false));
    assert_eq!(state(&mut vm, u, b"X004"), Some(true));
    assert_eq!(vm.call("on", &[]).unwrap(), Value::Bool(true));
    assert_eq!(vm.global("Seen"), Value::Int(2));
    assert_eq!(state(&mut vm, u, b"X002"), Some(true));
    assert_eq!(state(&mut vm, u, b"X004"), Some(false));
    // Enabling the peer switches the first ability off and restores the peer's authored default.
    assert_eq!(vm.call("onpeer", &[]).unwrap(), Value::Bool(true));
    assert_eq!(vm.global("Seen"), Value::Int(4));
    assert_eq!(state(&mut vm, u, b"X002"), Some(false));
    assert_eq!(state(&mut vm, u, b"X004"), Some(true));
    assert!(!modified(&mut vm, u));
}
#[test]
fn enable_of_a_switched_off_peer_is_disarmed() {
    let (mut vm, _) = open_setup("X003", "X002,X004", PEER_PROFILE, enable());
    let start = vm.host().inner.timer_game_time().unwrap();
    assert_eq!(vm.call("on", &[]).unwrap(), Value::Bool(true));
    assert_eq!(vm.call("onpeer", &[]).unwrap(), Value::Bool(true));
    // The second ON re-armed the peer; the first ability's tick is gone with its OFF.
    let error = advance(&mut vm, start + f64::from(FIRST_TICK)).unwrap_err();
    assert!(error.contains("X004"), "{error}");
}
#[test]
fn enabled_state_is_refused_by_the_cache_like_disabled_state() {
    let (mut vm, u) = open("_", true);
    assert_eq!(vm.call("on", &[]).unwrap(), Value::Bool(true));
    let h = &mut vm.host().inner;
    let memory = Arc::new(crate::game_cache::tests::Memory::default());
    h.admit_game_caches(memory.clone(), crate::game_cache::tests::budget())
        .unwrap();
    let cache = h
        .native("InitGameCache", &[Value::Str("fixture".into())])
        .unwrap();
    let error = h
        .native(
            "StoreUnit",
            &[
                cache,
                Value::Str("mission".into()),
                Value::Str("hero".into()),
                Value::Handle(u),
            ],
        )
        .unwrap_err();
    assert!(
        error.contains("does not encode altered autocast state"),
        "{error}"
    );
}

fn alarm(vm: &mut Vm, name: &str, args: &[Value]) -> Value {
    vm.host().inner.native(name, args).unwrap()
}
#[test]
fn ignore_alarm_sets_and_reads_the_bit_of_the_alarm_class_ability() {
    // X006 has the alarm class code (X005); X002 is an unrelated ability listed before it.
    let (mut vm, u) = open_setup("_", "X002,X006", PROFILE, enable());
    let unit = Value::Handle(u);
    assert_eq!(
        alarm(
            &mut vm,
            "UnitIgnoreAlarmToggled",
            std::slice::from_ref(&unit)
        ),
        Value::Bool(false)
    );
    assert_eq!(
        alarm(
            &mut vm,
            "UnitIgnoreAlarm",
            &[unit.clone(), Value::Bool(true)]
        ),
        Value::Bool(true)
    );
    assert_eq!(
        alarm(
            &mut vm,
            "UnitIgnoreAlarmToggled",
            std::slice::from_ref(&unit)
        ),
        Value::Bool(true)
    );
    assert!(modified_alarm(&mut vm, u));
    assert_eq!(
        alarm(
            &mut vm,
            "UnitIgnoreAlarm",
            &[unit.clone(), Value::Bool(false)]
        ),
        Value::Bool(true)
    );
    assert_eq!(
        alarm(&mut vm, "UnitIgnoreAlarmToggled", &[unit]),
        Value::Bool(false)
    );
    assert!(!modified_alarm(&mut vm, u));
    for (name, args) in [
        ("UnitIgnoreAlarm", vec![Value::Null, Value::Bool(true)]),
        ("UnitIgnoreAlarmToggled", vec![Value::Null]),
    ] {
        assert_eq!(alarm(&mut vm, name, &args), Value::Bool(false));
    }
}
#[test]
fn ignore_alarm_on_a_unit_without_the_alarm_ability_returns_false_and_changes_nothing() {
    let (mut vm, u) = open("_", true);
    let unit = Value::Handle(u);
    assert_eq!(
        alarm(
            &mut vm,
            "UnitIgnoreAlarm",
            &[unit.clone(), Value::Bool(true)]
        ),
        Value::Bool(false)
    );
    assert_eq!(
        alarm(&mut vm, "UnitIgnoreAlarmToggled", &[unit]),
        Value::Bool(false)
    );
    assert!(!modified_alarm(&mut vm, u));
}
fn modified_alarm(vm: &mut Vm, u: HandleId) -> bool {
    let record = vm.host().inner.executed_unit(u).unwrap();
    record.abilities.as_ref().unwrap().alarm_modified()
}

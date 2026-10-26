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
function off takes nothing returns boolean
return IssueImmediateOrder(U,"delta")
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
fn open(default: &str, present: bool) -> (Vm, HandleId) {
    let mut fields = hero_fields("STR");
    fields.retain(|(id, _)| id != b"A008");
    fields.push(column(
        b"A008",
        SlkId::UnitAbilities,
        "abilList",
        FieldKind::Token,
        if present { "X001" } else { "_" },
    ));
    fields.push(column(
        b"A010",
        SlkId::UnitAbilities,
        "auto",
        FieldKind::Token,
        default,
    ));
    let (registry, activity) = super::super::activity::rules_with_repair();
    let autocast = crate::autocast_rules::Rules {
        origin: registry.binding().origin.clone(),
        image_sha256: registry.binding().image_sha256,
        classes: vec![crate::autocast_rules::Class {
            code: *b"X001",
            on: registry.id("repair").unwrap(),
            off: registry.id("delta").unwrap(),
        }],
    };
    let mut rules = hero_rules().0;
    rules.binding.image_sha256 = autocast.image_sha256;
    let rules = rules.with_autocast(Ok(autocast));
    let data = sylk(
        &["alias", "code", "levels", "reqLevel", "levelSkip", "hero"],
        &[&["X001", "X001", "1", "0", "0", "0"]],
    );
    let tables = DefaultTables::from_bytes(
        Identity {
            install: source().install,
            edition_version: "fixture".into(),
        },
        &[(SlkId::AbilityData.path(), &data)],
        &[(
            "Units\\HumanAbilityFunc.txt",
            b"[X001]\nOrderon=repair\nOrderoff=delta\n",
        )],
        pcx_objects::editor::EditorReadLimits {
            max_listed_paths: 4,
            max_asset_bytes: 65536,
            max_aggregate_input_bytes: 131072,
        },
    )
    .unwrap();
    let extra = format!(
        "{}\n{}\ntype playerunitevent extends eventid\nconstant native ConvertPlayerUnitEvent takes integer i returns playerunitevent\nnative RemoveUnit takes unit u returns nothing\ntype gamecache extends agent\nnative InitGameCache takes string file returns gamecache\nnative StoreUnit takes gamecache c, string m, string k, unit u returns boolean\nconstant native GetIssuedOrderId takes nothing returns integer\nnative TriggerRegisterUnitEvent takes trigger t, unit u, unitevent e returns event\nnative TriggerRegisterPlayerUnitEvent takes trigger t, player p, playerunitevent e, boolexpr f returns event\nglobals\nconstant unitevent EVENT_UNIT_ISSUED_ORDER=ConvertUnitEvent(123)\nconstant playerunitevent EVENT_PLAYER_UNIT_ISSUED_ORDER=ConvertPlayerUnitEvent(-456)\nendglobals\n",
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
    let mut vm = context.scripts.clone().owned_vm(h);
    vm.call("main", &[]).unwrap();
    let Value::Handle(unit) = vm.global("U") else {
        panic!("unit")
    };
    (vm, unit)
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
        vm.host().inner.unit_autocast_enabled(u, *b"X001").unwrap(),
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
        vm.host().inner.unit_autocast_enabled(u, *b"X001").unwrap(),
        Some(false)
    );
}
#[test]
fn enable_refuses_and_pause_or_retirement_preserves_typed_contracts() {
    let (mut vm, u) = open("X001", true);
    let before = vm.host().inner.executed_unit(u).unwrap().clone();
    let error = vm
        .host()
        .inner
        .native(
            "IssueImmediateOrder",
            &[Value::Handle(u), Value::Str("repair".into())],
        )
        .unwrap_err();
    assert!(error.contains("enabling/effects not implemented"));
    assert_eq!(vm.host().inner.executed_unit(u).unwrap(), &before);
    vm.host()
        .inner
        .native("PauseUnit", &[Value::Handle(u), Value::Bool(true)])
        .unwrap();
    assert_eq!(vm.call("off", &[]).unwrap(), Value::Bool(false));
    assert_eq!(
        vm.host().inner.unit_autocast_enabled(u, *b"X001").unwrap(),
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
        vm.host().inner.unit_autocast_enabled(u, *b"X001").unwrap(),
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
    let (mut vm, u) = open("bad", true);
    let before = vm.host().inner.executed_unit(u).unwrap().clone();
    assert!(vm.call("off", &[]).is_err());
    assert_eq!(vm.host().inner.executed_unit(u).unwrap(), &before);
}

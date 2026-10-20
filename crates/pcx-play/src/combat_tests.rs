use super::*;
use pcx_objects::editor::misc::{CombatRules, HeroVitals};
use pcx_script::HandleId;
type Vm = pcx_script::OwnedVm<crate::scripts::OwnedLocalizedHost<ObjectHost>>;

const DECLARATIONS: &str = r#"
type unitstate extends handle
type playerunitevent extends eventid
type alliancetype extends handle
constant native ConvertUnitState takes integer i returns unitstate
constant native ConvertPlayerUnitEvent takes integer i returns playerunitevent
constant native ConvertAllianceType takes integer i returns alliancetype
constant native GetUnitState takes unit u, unitstate s returns real
native SetUnitState takes unit u, unitstate s, real v returns nothing
native IssueTargetOrder takes unit u, string o, widget t returns boolean
native KillUnit takes unit u returns nothing
native GetRandomInt takes integer a, integer b returns integer
native GetRandomReal takes real a, real b returns real
native SetRandomSeed takes integer s returns nothing
native TriggerRegisterUnitEvent takes trigger t, unit u, unitevent e returns event
constant native GetDyingUnit takes nothing returns unit
constant native GetKillingUnit takes nothing returns unit
constant native GetAttacker takes nothing returns unit
globals
constant unitstate UNIT_STATE_LIFE = ConvertUnitState(0)
constant unitstate UNIT_STATE_MAX_LIFE = ConvertUnitState(1)
constant unitstate UNIT_STATE_MANA = ConvertUnitState(2)
constant unitstate UNIT_STATE_MAX_MANA = ConvertUnitState(3)
constant playerunitevent EVENT_PLAYER_UNIT_DEATH = ConvertPlayerUnitEvent(20)
constant unitevent EVENT_UNIT_DEATH = ConvertUnitEvent(53)
constant unitevent EVENT_UNIT_ATTACKED = ConvertUnitEvent(54)
constant alliancetype ALLIANCE_PASSIVE = ConvertAllianceType(0)
constant integer PLAYER_NEUTRAL_PASSIVE = 15
endglobals
"#;

const MAP: &str = r#"
globals
unit A=null
unit B=null
trigger Died=null
trigger Hit=null
endglobals
function setup takes nothing returns nothing
set A=CreateUnit(Player(4),'u001',16.0,16.0,0.0)
set B=CreateUnit(Player(5),'u001',48.0,16.0,180.0)
endfunction
function listen takes nothing returns nothing
set Died=CreateTrigger()
set Hit=CreateTrigger()
call TriggerRegisterUnitEvent(Died,B,EVENT_UNIT_DEATH)
call TriggerRegisterUnitEvent(Hit,B,EVENT_UNIT_ATTACKED)
endfunction
"#;

fn column(
    id: &[u8; 4],
    slk: SlkId,
    column: &str,
    kind: FieldKind,
    value: &str,
) -> ([u8; 4], ObjectField) {
    let mut field = geometry_field(Ok(FieldValue::Cell(value.into())));
    let meta = field.metadata.as_mut().unwrap();
    meta.fourcc = *id;
    meta.field = column.into();
    meta.kind = kind;
    meta.source = FieldSource::Column {
        slk,
        column: column.into(),
    };
    (*id, field)
}

fn armed(range: &str, weapon: &str) -> Vec<([u8; 4], ObjectField)> {
    use FieldKind::{Int, Real, Token};
    use SlkId::{UnitBalance as B, UnitWeapons as W};
    vec![
        column(b"H001", B, "HP", Int, "20"),
        column(b"H002", B, "manaN", Int, "-"),
        column(b"H003", B, "mana0", Int, "-"),
        column(b"H004", B, "def", Int, "2"),
        column(b"H005", B, "defType", Token, "medium"),
        column(b"W001", W, "weapsOn", Int, "1"),
        column(b"W002", W, "atkType1", Token, "pierce"),
        column(b"W003", W, "weapTp1", Token, weapon),
        column(b"W004", W, "rangeN1", Int, range),
        column(b"W005", W, "acquire", Real, "0"),
        column(b"W006", W, "cool1", Real, "1.5"),
        column(b"W007", W, "dmgpt1", Real, "0.25"),
        column(b"W008", W, "dmgplus1", Int, "4"),
        column(b"W009", W, "dice1", Int, "1"),
        column(b"W010", W, "sides1", Int, "1"),
    ]
}

fn rules() -> CombatRules {
    let mut damage_bonus = [[1.0; 8]; 7];
    damage_bonus[1][1] = 0.75;
    CombatRules {
        armor_factor: 0.06,
        damage_bonus,
        call_for_help: 0.0,
        creep_call_for_help: 0.0,
        guard_distance: 0.0,
        max_guard_distance: 0.0,
        guard_return_time: 0.0,
    }
}

fn open(fields: &[([u8; 4], ObjectField)]) -> (Vm, HandleId, HandleId) {
    let extra = format!(
        "{}\n{}{DECLARATIONS}",
        enumeration::TRIGGERS,
        super::activity::ORDERS
    );
    let h = position_host_admitting(1000, 16, MAP, &extra, fields, |h| h);
    let (registry, activity) = super::activity::rules();
    let mut h = h
        .with_unit_states(crate::playercolor_host::UnitStateBudget { max_records: 8 })
        .unwrap()
        .with_order_registry(registry)
        .unwrap()
        .with_script_random(Arc::new(crate::script_random::profile::fixture()), 64)
        .unwrap();
    h.admit_unit_activity(activity, 65536).unwrap();
    h.native("SetRandomSeed", &[Value::Int(7)]).unwrap();
    h.admit_unit_vitals(HeroVitals {
        life_per_strength: 25.0,
        mana_per_intelligence: 15.0,
    })
    .unwrap();
    h.admit_combat(rules(), "alpha").unwrap();
    h.set_neutral_passive(15);
    let context = Arc::clone(h.player_context().unwrap());
    let mut vm = context.scripts.clone().owned_vm(h);
    vm.init_globals().unwrap();
    vm.call("setup", &[]).unwrap();
    let id = |vm: &Vm, name| match vm.global(name) {
        Value::Handle(id) => id,
        _ => panic!("{name}"),
    };
    let (a, b) = (id(&vm, "A"), id(&vm, "B"));
    (vm, a, b)
}

fn step(vm: &mut Vm, until: f64) {
    let h = &mut vm.host().inner;
    while h.timer_game_time().unwrap() < until {
        let next = h.timer_game_time().unwrap() + 1.0 / 64.0;
        assert!(h.next_timer_expiry(next, &mut 100).unwrap().is_none());
        h.advance_unit_activity(next, &mut 65536).unwrap();
    }
}

#[test]
fn melee_attacks_follow_the_source_cycle_and_kill_with_death_events() {
    let mut fields = armed("64", "normal");
    fields.push(column(
        b"S001",
        SlkId::UnitBalance,
        "spd",
        FieldKind::Real,
        "8",
    ));
    let (mut vm, a, b) = open(&fields);
    let life = |vm: &mut Vm, u| vm.host().inner.unit_vitals(u).unwrap().unwrap().life;
    assert_eq!(life(&mut vm, b), 20.0);
    let h = &mut vm.host().inner;
    let order = |h: &mut ObjectHost, target| {
        h.native(
            "IssueTargetOrder",
            &[Value::Handle(a), Value::Str("alpha".into()), target],
        )
    };
    assert_eq!(order(h, Value::Null).unwrap(), Value::Bool(false));
    assert!(
        h.native(
            "IssueTargetOrder",
            &[
                Value::Handle(a),
                Value::Str("beta".into()),
                Value::Handle(b)
            ]
        )
        .is_err()
    );
    assert_eq!(order(h, Value::Handle(b)).unwrap(), Value::Bool(true));
    // Pierce against medium (0.75) and 2 armor: (4 + 1) * 0.75 * (1 - 0.12 / 1.12).
    let per_hit = 5.0 * 0.75 * (1.0 - 0.12 / 1.12);
    let start = vm.host().inner.timer_game_time().unwrap();
    step(&mut vm, start + 0.3);
    assert!((life(&mut vm, b) - (20.0 - per_hit)).abs() < 1e-4);
    // The next swing waits for the cooldown, not the damage point.
    step(&mut vm, start + 1.4);
    assert!((life(&mut vm, b) - (20.0 - per_hit)).abs() < 1e-4);
    step(&mut vm, start + 1.8);
    assert!((life(&mut vm, b) - (20.0 - 2.0 * per_hit)).abs() < 1e-4);
    // Damage below zero kills once; a dead unit is no longer a valid target.
    step(&mut vm, start + 12.0);
    let state = vm_global(&vm, "UNIT_STATE_LIFE");
    let h = &mut vm.host().inner;
    assert!(h.executed_unit(b).unwrap().combat.as_ref().unwrap().dead);
    assert_eq!(h.unit_vitals(b).unwrap().unwrap().life, 0.0);
    assert_eq!(order(h, Value::Handle(b)).unwrap(), Value::Bool(false));
    assert_eq!(
        h.native("GetUnitState", &[Value::Handle(b), state])
            .unwrap(),
        Value::Real(0.0)
    );
}

fn vm_global(vm: &Vm, name: &str) -> Value {
    vm.global(name)
}

#[test]
fn missiles_land_after_travel_and_lethal_state_and_kill_unit_reuse_death() {
    let mut fields = armed("640", "missile");
    fields.push(column(
        b"S001",
        SlkId::UnitBalance,
        "spd",
        FieldKind::Real,
        "8",
    ));
    let mut speed = column(
        b"M002",
        SlkId::UnitData,
        "Missilespeed",
        FieldKind::Int,
        "64",
    );
    speed.1.metadata.as_mut().unwrap().source = FieldSource::Profile {
        key: "missilespeed".into(),
        index: 0,
        slots: 1,
    };
    fields.push(speed);
    let (mut vm, a, b) = open(&fields);
    let h = &mut vm.host().inner;
    h.native(
        "IssueTargetOrder",
        &[
            Value::Handle(a),
            Value::Str("alpha".into()),
            Value::Handle(b),
        ],
    )
    .unwrap();
    let start = h.timer_game_time().unwrap();
    // Swing at 0.25 s, then 32 units at 64/s: no damage until ~0.75 s.
    step(&mut vm, start + 0.6);
    let life = |vm: &mut Vm| vm.host().inner.unit_vitals(b).unwrap().unwrap().life;
    assert_eq!(life(&mut vm), 20.0);
    step(&mut vm, start + 0.8);
    assert!(life(&mut vm) < 20.0);
    let state = vm_global(&vm, "UNIT_STATE_LIFE");
    let h = &mut vm.host().inner;
    h.native(
        "SetUnitState",
        &[Value::Handle(b), state, Value::Real(-1.0)],
    )
    .unwrap();
    assert!(h.executed_unit(b).unwrap().combat.as_ref().unwrap().dead);
    h.native("KillUnit", &[Value::Handle(a)]).unwrap();
    h.native("KillUnit", &[Value::Handle(a)]).unwrap();
    h.native("KillUnit", &[Value::Null]).unwrap();
    assert!(h.executed_unit(a).unwrap().combat.as_ref().unwrap().dead);
}

#[test]
fn attacks_and_deaths_queue_their_registered_triggers_with_event_responses() {
    let mut fields = armed("64", "normal");
    fields.push(column(
        b"S001",
        SlkId::UnitBalance,
        "spd",
        FieldKind::Real,
        "8",
    ));
    let (mut vm, a, b) = open(&fields);
    vm.call("listen", &[]).unwrap();
    let (Value::Handle(died), Value::Handle(hit)) = (vm.global("Died"), vm.global("Hit")) else {
        panic!("triggers")
    };
    let h = &mut vm.host().inner;
    h.native(
        "IssueTargetOrder",
        &[
            Value::Handle(a),
            Value::Str("alpha".into()),
            Value::Handle(b),
        ],
    )
    .unwrap();
    let start = h.timer_game_time().unwrap();
    step(&mut vm, start + 0.1);
    let h = &mut vm.host().inner;
    let fired = h.take_fired_event().expect("attacked event");
    assert_eq!(fired.trigger, hit);
    assert!(h.take_fired_event().is_none());
    // Delivery installs the attacked unit and attacker as event responses.
    let delivery = h
        .begin_fired_event(&fired)
        .unwrap()
        .expect("enabled trigger");
    assert!(delivery.actions.is_empty());
    assert_eq!(h.native("GetTriggerUnit", &[]).unwrap(), Value::Handle(b));
    assert_eq!(h.native("GetAttacker", &[]).unwrap(), Value::Handle(a));
    h.finish_fired_event();
    assert_eq!(h.native("GetAttacker", &[]).unwrap(), Value::Null);
    h.native("KillUnit", &[Value::Handle(b)]).unwrap();
    let mut died_events = Vec::new();
    while let Some(event) = h.take_fired_event() {
        died_events.push(event);
    }
    assert_eq!(died_events.len(), 1);
    assert_eq!(died_events[0].trigger, died);
    h.begin_fired_event(&died_events[0]).unwrap().unwrap();
    assert_eq!(h.native("GetDyingUnit", &[]).unwrap(), Value::Handle(b));
    assert_eq!(h.native("GetKillingUnit", &[]).unwrap(), Value::Null);
    h.finish_fired_event();
}

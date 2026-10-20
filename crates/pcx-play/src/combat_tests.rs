use super::*;
use crate::playercolor_host::{CombatActor, CombatTraceEvent};
use pcx_objects::editor::misc::{CombatRules, DayCycle, HeroVitals};
use pcx_script::HandleId;
type Vm = pcx_script::OwnedVm<crate::scripts::OwnedLocalizedHost<ObjectHost>>;

const DECLARATIONS: &str = r#"
constant native GetLocalPlayer takes nothing returns player
native SetPlayerAlliance takes player a, player b, alliancetype s, boolean v returns nothing
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
native SetUnitExploded takes unit whichUnit, boolean exploded returns nothing
native SetUnitInvulnerable takes unit u, boolean flag returns nothing
native GetRandomInt takes integer a, integer b returns integer
native GetRandomReal takes real a, real b returns real
native SetRandomSeed takes integer s returns nothing
native TriggerRegisterPlayerUnitEvent takes trigger t, player p, playerunitevent e, boolexpr f returns event
native TriggerRegisterUnitEvent takes trigger t, unit u, unitevent e returns event
constant native GetDyingUnit takes nothing returns unit
constant native GetKillingUnit takes nothing returns unit
constant native GetAttacker takes nothing returns unit
constant native GetTriggerPlayer takes nothing returns player
globals
constant unitstate UNIT_STATE_LIFE = ConvertUnitState(0)
constant unitstate UNIT_STATE_MAX_LIFE = ConvertUnitState(1)
constant unitstate UNIT_STATE_MANA = ConvertUnitState(2)
constant unitstate UNIT_STATE_MAX_MANA = ConvertUnitState(3)
constant playerunitevent EVENT_PLAYER_UNIT_ATTACKED = ConvertPlayerUnitEvent(117)
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
unit C=null
trigger Died=null
trigger Hit=null
endglobals
function setup takes nothing returns nothing
set A=CreateUnit(Player(4),'u001',16.0,16.0,0.0)
set B=CreateUnit(Player(5),'u001',48.0,16.0,180.0)
endfunction
function spawn takes nothing returns nothing
set C=CreateUnit(Player(4),'u001',16.0,16.0,0.0)
endfunction
function protect takes nothing returns nothing
call SetUnitInvulnerable(B,true)
endfunction
function unprotect takes nothing returns nothing
call SetUnitInvulnerable(B,false)
endfunction
function attack_filter takes nothing returns boolean
return GetFilterUnit()==B and GetTriggerUnit()==B and GetAttacker()==A
endfunction
function filter_value takes nothing returns boolexpr
return Filter(function attack_filter)
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
        column(b"H006", B, "regenHP", Real, "0"),
        column(b"H007", B, "regenMana", Real, "-"),
        column(b"H008", B, "regenType", Token, "none"),
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
        sources: vec!["synthetic combat rules".into()],
        armor_factor: 0.06,
        primary_damage_per_attribute: 1.0,
        armor_per_agility: 0.3,
        hero_armor_base: -2.0,
        attack_speed_per_agility: 0.02,
        damage_bonus,
        call_for_help: 0.0,
        creep_call_for_help: 0.0,
        guard_distance: 0.0,
        max_guard_distance: 0.0,
        guard_return_time: 0.0,
    }
}

fn open(fields: &[([u8; 4], ObjectField)]) -> (Vm, HandleId, HandleId) {
    open_selected(fields, None, rules())
}

fn open_selected(
    fields: &[([u8; 4], ObjectField)],
    heroes: Option<crate::hero_skills::Rules>,
    combat: CombatRules,
) -> (Vm, HandleId, HandleId) {
    open_fixture(fields, heroes, combat, false)
}

/// The same fixture with the source fgamestate clock and time-of-day admitted at hour 9.
fn open_clocked(
    fields: &[([u8; 4], ObjectField)],
    combat: CombatRules,
) -> (Vm, HandleId, HandleId) {
    open_fixture(fields, None, combat, true)
}

/// Clock declarations: the stock `CLOCK` handle and the stock dawn/dusk window.
const CLOCK_DECLARATIONS: &str = r#"
type fgamestate extends handle
constant native ConvertFGameState takes integer i returns fgamestate
constant native GetFloatGameState takes fgamestate s returns real
native SetFloatGameState takes fgamestate s, real v returns nothing
native SuspendTimeOfDay takes boolean b returns nothing
native SetTimeOfDayScale takes real r returns nothing
native GetTimeOfDayScale takes nothing returns real
globals
constant fgamestate CLOCK = ConvertFGameState(2)
constant real bj_TOD_DAWN = 6.00
constant real bj_TOD_DUSK = 18.00
endglobals
"#;

fn open_fixture(
    fields: &[([u8; 4], ObjectField)],
    heroes: Option<crate::hero_skills::Rules>,
    combat: CombatRules,
    clock: bool,
) -> (Vm, HandleId, HandleId) {
    let extra = format!(
        "{}\n{}{DECLARATIONS}{}",
        enumeration::TRIGGERS,
        super::activity::ORDERS,
        if clock { CLOCK_DECLARATIONS } else { "" }
    );
    let map = if heroes.is_some() {
        MAP.replacen("'u001'", "'H001'", 1)
    } else {
        MAP.into()
    };
    let h = position_host_admitting(1000, 16, &map, &extra, fields, |h| {
        if clock {
            h.with_startup_clock(crate::startup_clock::tests::input(9), "CLOCK", 64, 16, 1000)
                .unwrap()
        } else {
            h
        }
    });
    let (registry, activity) = super::activity::rules();
    let mut h = h
        .with_locations()
        .unwrap()
        .with_player_unit_events(8)
        .unwrap()
        .with_unit_states(crate::playercolor_host::UnitStateBudget { max_records: 8 })
        .unwrap()
        .with_player_alliances(crate::playercolor_host::PlayerAllianceBudget { max_records: 8 })
        .unwrap()
        .with_unit_explosions(8)
        .unwrap()
        .with_order_registry(registry)
        .unwrap()
        .with_script_random(Arc::new(crate::script_random::profile::fixture()), 64)
        .unwrap();
    if let Some(rules) = heroes {
        let origin = crate::scripts::AssetIdentity {
            source: source().install,
            edition: "fixture".into(),
            version: "fixture".into(),
        };
        let tables = DefaultTables::from_bytes(
            Identity {
                install: origin.source.clone(),
                edition_version: origin.version.clone(),
            },
            &[],
            &[],
            pcx_objects::editor::EditorReadLimits {
                max_listed_paths: 0,
                max_asset_bytes: 1 << 10,
                max_aggregate_input_bytes: 1 << 10,
            },
        )
        .unwrap();
        h = h
            .with_hero_skills(
                tables,
                origin,
                Default::default(),
                rules,
                crate::playercolor_host::HeroBudget {
                    abilities_per_unit: 8,
                    overlay_objects: 0,
                    modifications_per_ability: 0,
                },
            )
            .unwrap();
    }
    h.admit_unit_activity(activity, 65536).unwrap();
    if clock {
        h.admit_time_of_day(&DayCycle {
            day_length: 480.0,
            day_hours: 24.0,
            sources: vec!["synthetic".into()],
        })
        .unwrap();
    }
    h.native("SetRandomSeed", &[Value::Int(7)]).unwrap();
    h.admit_unit_vitals(HeroVitals {
        life_per_strength: 25.0,
        mana_per_intelligence: 15.0,
        life_regen_per_strength: 0.0,
        mana_regen_per_intelligence: 0.0,
    })
    .unwrap();
    h.admit_combat(combat, "alpha").unwrap();
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
    assert!(
        h.native("GetTriggerPlayer", &[]).is_err(),
        "ordinary unit event has no admitted player response"
    );
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

#[test]
fn selection_relation_tracks_local_owner_passive_owner_and_directional_alliances() {
    use crate::playercolor_host::Relation;
    let (mut vm, a, b) = open(&armed("100", "normal"));
    let alliance = vm.global("ALLIANCE_PASSIVE");
    let h = &mut vm.host().inner;
    assert_eq!(h.local_relation(a), None);
    let context = Arc::clone(h.player_context().unwrap());
    h.admit_local_player(&context, 4).unwrap();
    assert_eq!(h.local_relation(a), Some(Relation::Friend));
    assert_eq!(h.local_relation(b), Some(Relation::Enemy));
    let local = h.native("Player", &[Value::Int(4)]).unwrap();
    let other = h.native("Player", &[Value::Int(5)]).unwrap();
    h.native(
        "SetPlayerAlliance",
        &[
            other.clone(),
            local.clone(),
            alliance.clone(),
            Value::Bool(true),
        ],
    )
    .unwrap();
    assert_eq!(h.local_relation(b), Some(Relation::Enemy));
    h.native(
        "SetPlayerAlliance",
        &[local, other, alliance, Value::Bool(true)],
    )
    .unwrap();
    assert_eq!(h.local_relation(b), Some(Relation::Friend));
    h.set_neutral_passive(5);
    assert_eq!(h.local_relation(b), Some(Relation::Neutral));
}

#[test]
fn player_attacked_events_match_the_current_owner_and_keep_filters_and_event_context() {
    let (mut vm, a, b) = open(&armed("64", "normal"));
    let filter = vm.call("filter_value", &[]).unwrap();
    let attacked = vm.global("EVENT_PLAYER_UNIT_ATTACKED");
    let death = vm.global("EVENT_PLAYER_UNIT_DEATH");
    let h = &mut vm.host().inner;
    let victim = Value::Handle(h.executed_unit(b).unwrap().owner);
    let attacker = Value::Handle(h.executed_unit(a).unwrap().owner);
    let mut matching = Vec::new();
    for (owner, kind, predicate) in [
        (victim.clone(), attacked.clone(), Value::Null),
        (attacker, attacked.clone(), Value::Null),
        (victim.clone(), death, Value::Null),
        (victim.clone(), attacked.clone(), filter),
        (victim.clone(), attacked.clone(), Value::Null),
        (victim, attacked.clone(), Value::Null),
    ] {
        let trigger = h.native("CreateTrigger", &[]).unwrap();
        h.native(
            "TriggerRegisterPlayerUnitEvent",
            &[trigger.clone(), owner, kind, predicate],
        )
        .unwrap();
        matching.push(trigger);
    }
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
    // Pending events still respect changes to their trigger before delivery.
    h.native("DisableTrigger", &[matching[4].clone()]).unwrap();
    h.native("DestroyTrigger", &[matching[5].clone()]).unwrap();
    let first = h.take_fired_event().unwrap();
    assert_eq!(Value::Handle(first.trigger), matching[0]);
    assert!(first.filter.is_none());
    h.begin_fired_event(&first).unwrap().unwrap();
    assert_eq!(h.native("GetTriggerEventId", &[]).unwrap(), attacked);
    assert_eq!(h.native("GetTriggerUnit", &[]).unwrap(), Value::Handle(b));
    assert_eq!(h.native("GetAttacker", &[]).unwrap(), Value::Handle(a));
    let registered_player = Value::Handle(h.executed_unit(b).unwrap().owner);
    assert_eq!(
        h.native("GetTriggerPlayer", &[]).unwrap(),
        registered_player
    );
    h.finish_fired_event();
    let filtered = h.take_fired_event().unwrap();
    assert_eq!(Value::Handle(filtered.trigger), matching[3]);
    assert!(filtered.filter.is_some());
    h.begin_fired_event(&filtered).unwrap().unwrap();
    assert_eq!(vm.call("attack_filter", &[]).unwrap(), Value::Bool(true));
    let h = &mut vm.host().inner;
    h.finish_fired_event();
    for expected in &matching[4..] {
        let fired = h.take_fired_event().unwrap();
        assert_eq!(Value::Handle(fired.trigger), *expected);
        assert!(h.begin_fired_event(&fired).unwrap().is_none());
    }
    assert!(h.take_fired_event().is_none());
    assert_eq!(h.native("GetAttacker", &[]).unwrap(), Value::Null);
    assert_eq!(h.native("GetFilterUnit", &[]).unwrap(), Value::Null);
    h.native("KillUnit", &[Value::Handle(b)]).unwrap();
    let death = h.take_fired_event().unwrap();
    assert_eq!(Value::Handle(death.trigger), matching[2]);
    h.begin_fired_event(&death).unwrap().unwrap();
    assert_eq!(
        h.native("GetTriggerPlayer", &[]).unwrap(),
        registered_player
    );
    h.finish_fired_event();
    assert_eq!(h.native("GetTriggerPlayer", &[]).unwrap(), Value::Null);
}

#[test]
fn point_attack_acquires_while_moving_and_resumes_then_finishes_its_destination() {
    let mut fields = armed("8", "normal");
    fields.push(column(
        b"S001",
        SlkId::UnitBalance,
        "spd",
        FieldKind::Real,
        "32",
    ));
    fields
        .iter_mut()
        .find(|(id, _)| id == b"W005")
        .unwrap()
        .1
        .value = Ok(FieldValue::Cell("40".into()));
    let (mut vm, a, b) = open(&fields);
    let h = &mut vm.host().inner;
    // Keep the target from acquiring/chasing so the attack-move must find it.
    let alliance = h.native("ConvertAllianceType", &[Value::Int(0)]).unwrap();
    h.native(
        "SetPlayerAlliance",
        &[
            Value::Handle(h.executed_unit(b).unwrap().owner),
            Value::Handle(h.executed_unit(a).unwrap().owner),
            alliance,
            Value::Bool(true),
        ],
    )
    .unwrap();
    assert_eq!(
        h.native(
            "IssuePointOrder",
            &[
                Value::Handle(a),
                Value::Str("alpha".into()),
                Value::Real(96.0),
                Value::Real(16.0)
            ]
        )
        .unwrap(),
        Value::Bool(true)
    );
    let start = h.timer_game_time().unwrap();
    step(&mut vm, start + 0.1);
    assert_eq!(
        vm.host()
            .inner
            .executed_unit(a)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .target,
        Some(b)
    );
    step(&mut vm, start + 14.0);
    let h = &mut vm.host().inner;
    assert!(h.executed_unit(b).unwrap().combat.as_ref().unwrap().dead);
    assert_eq!(h.executed_unit(a).unwrap().position, Some([96.0, 16.0]));
    assert_eq!(
        h.native("GetUnitCurrentOrder", &[Value::Handle(a)])
            .unwrap(),
        Value::Int(0)
    );
    assert!(
        h.executed_unit(a)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .attack_destination
            .is_none()
    );
}

#[test]
fn replacement_orders_cancel_the_retained_attack_move_destination() {
    let mut fields = armed("8", "normal");
    fields.push(column(
        b"S001",
        SlkId::UnitBalance,
        "spd",
        FieldKind::Real,
        "32",
    ));
    let (mut vm, a, b) = open(&fields);
    let h = &mut vm.host().inner;
    for (name, tail) in [
        ("IssueImmediateOrder", vec![Value::Str("beta".into())]),
        ("IssueImmediateOrder", vec![Value::Str("alpha".into())]),
        (
            "IssuePointOrder",
            vec![
                Value::Str("gamma".into()),
                Value::Real(80.0),
                Value::Real(32.0),
            ],
        ),
        (
            "IssueTargetOrder",
            vec![Value::Str("alpha".into()), Value::Handle(b)],
        ),
    ] {
        h.native(
            "IssuePointOrder",
            &[
                Value::Handle(a),
                Value::Str("alpha".into()),
                Value::Real(96.0),
                Value::Real(16.0),
            ],
        )
        .unwrap();
        assert!(
            h.executed_unit(a)
                .unwrap()
                .combat
                .as_ref()
                .unwrap()
                .attack_destination
                .is_some()
        );
        let args = [vec![Value::Handle(a)], tail].concat();
        assert_eq!(h.native(name, &args).unwrap(), Value::Bool(true));
        assert!(
            h.executed_unit(a)
                .unwrap()
                .combat
                .as_ref()
                .unwrap()
                .attack_destination
                .is_none()
        );
    }
}

#[test]
fn attack_move_point_and_location_variants_copy_the_admitted_destination() {
    for name in [
        "IssuePointOrderById",
        "IssuePointOrderLoc",
        "IssuePointOrderByIdLoc",
    ] {
        let mut fields = armed("8", "normal");
        fields.push(column(
            b"S001",
            SlkId::UnitBalance,
            "spd",
            FieldKind::Real,
            "32",
        ));
        let (mut vm, a, _) = open(&fields);
        let h = &mut vm.host().inner;
        let location = h
            .native("Location", &[Value::Real(96.0), Value::Real(48.0)])
            .unwrap();
        let order = if name.contains("ById") {
            h.native("OrderId", &[Value::Str("alpha".into())]).unwrap()
        } else {
            Value::Str("alpha".into())
        };
        let mut args = vec![Value::Handle(a), order];
        if name.ends_with("Loc") {
            args.push(location.clone());
        } else {
            args.extend([Value::Real(96.0), Value::Real(48.0)]);
        }
        assert_eq!(h.native(name, &args).unwrap(), Value::Bool(true));
        h.native(
            "MoveLocation",
            &[location.clone(), Value::Real(112.0), Value::Real(80.0)],
        )
        .unwrap();
        h.native("RemoveLocation", &[location]).unwrap();
        let start = h.timer_game_time().unwrap();
        step(&mut vm, start + 5.0);
        let h = &mut vm.host().inner;
        assert_eq!(h.executed_unit(a).unwrap().position, Some([96.0, 48.0]));
        assert_eq!(
            h.native("GetUnitCurrentOrder", &[Value::Handle(a)])
                .unwrap(),
            Value::Int(0)
        );
    }
}

#[test]
fn invalid_attack_move_arguments_preserve_the_previous_activity_and_combat() {
    let mut fields = armed("8", "normal");
    fields.push(column(
        b"S001",
        SlkId::UnitBalance,
        "spd",
        FieldKind::Real,
        "32",
    ));
    let (mut vm, a, b) = open(&fields);
    let h = &mut vm.host().inner;
    let location = h
        .native("Location", &[Value::Real(96.0), Value::Real(48.0)])
        .unwrap();
    h.native(
        "IssueTargetOrder",
        &[
            Value::Handle(a),
            Value::Str("alpha".into()),
            Value::Handle(b),
        ],
    )
    .unwrap();
    let before = h.executed_unit(a).unwrap().clone();
    for (unit, destination, expected) in [
        (location.clone(), location, "unit argument"),
        (Value::Handle(a), Value::Handle(b), "location argument"),
    ] {
        let error = h
            .native(
                "IssuePointOrderLoc",
                &[unit, Value::Str("alpha".into()), destination],
            )
            .unwrap_err();
        assert!(error.to_string().contains(expected), "{error}");
        assert_eq!(h.executed_unit(a).unwrap().activity, before.activity);
        assert_eq!(h.executed_unit(a).unwrap().combat, before.combat);
    }
    h.native("KillUnit", &[Value::Handle(a)]).unwrap();
    assert_eq!(
        h.native(
            "IssuePointOrder",
            &[
                Value::Handle(a),
                Value::Str("alpha".into()),
                Value::Real(96.0),
                Value::Real(48.0)
            ]
        )
        .unwrap(),
        Value::Bool(false)
    );
    assert!(
        h.executed_unit(a)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .attack_destination
            .is_none()
    );
}

#[test]
fn unarmed_attack_move_finishes_without_acquiring_a_target() {
    let mut fields = armed("8", "normal");
    fields.push(column(
        b"S001",
        SlkId::UnitBalance,
        "spd",
        FieldKind::Real,
        "32",
    ));
    fields
        .iter_mut()
        .find(|(id, _)| id == b"W001")
        .unwrap()
        .1
        .value = Ok(FieldValue::Cell("0".into()));
    let (mut vm, a, _) = open(&fields);
    let h = &mut vm.host().inner;
    assert_eq!(
        h.native(
            "IssuePointOrder",
            &[
                Value::Handle(a),
                Value::Str("alpha".into()),
                Value::Real(96.0),
                Value::Real(48.0)
            ]
        )
        .unwrap(),
        Value::Bool(true)
    );
    let start = h.timer_game_time().unwrap();
    step(&mut vm, start + 5.0);
    let h = &mut vm.host().inner;
    assert_eq!(h.executed_unit(a).unwrap().position, Some([96.0, 48.0]));
    assert_eq!(
        h.native("GetUnitCurrentOrder", &[Value::Handle(a)])
            .unwrap(),
        Value::Int(0)
    );
}

fn hero_fields(primary: &str) -> Vec<([u8; 4], ObjectField)> {
    let mut fields = armed("64", "normal");
    for (id, name, kind, value) in [
        (b"A001", "STR", FieldKind::Int, "10"),
        (b"A002", "AGI", FieldKind::Int, "5"),
        (b"A003", "INT", FieldKind::Int, "7"),
        (b"A004", "STRplus", FieldKind::Unreal, "2.5"),
        (b"A005", "AGIplus", FieldKind::Unreal, "1.5"),
        (b"A006", "INTplus", FieldKind::Unreal, "0.75"),
        (b"A007", "Primary", FieldKind::Token, primary),
        (b"H001", "HP", FieldKind::Int, "500"),
    ] {
        fields.push(column(id, SlkId::UnitBalance, name, kind, value));
    }
    for (id, name) in [(b"A008", "abilList"), (b"A009", "heroAbilList")] {
        fields.push(column(
            id,
            SlkId::UnitAbilities,
            name,
            FieldKind::Token,
            "_",
        ));
    }
    fields
}

fn hero_rules() -> (crate::hero_skills::Rules, CombatRules) {
    let mut hero = crate::hero_skills::tests::rules();
    hero.initial_level = 3;
    let mut combat = rules();
    combat.primary_damage_per_attribute = 1.75;
    combat.armor_per_agility = 0.4;
    combat.hero_armor_base = -1.0;
    combat.attack_speed_per_agility = 0.2;
    (hero, combat)
}

#[test]
fn hero_primary_types_fractional_growth_and_source_baseline_follow_live_levels() {
    for (primary, initial, fourth, fifth) in [
        ("STR", 10.0, 12.0, 15.0),
        ("AGI", 5.0, 6.0, 8.0),
        ("INT", 7.0, 7.0, 8.0),
    ] {
        let (hero, combat) = hero_rules();
        let (mut vm, a, b) = open_selected(&hero_fields(primary), Some(hero), combat);
        let initial_bonuses = vm.host().inner.hero_combat_bonuses(a).unwrap();
        assert_eq!(initial_bonuses.damage, initial * 1.75);
        assert_eq!((initial_bonuses.armor, initial_bonuses.speed), (1.0, 2.0));
        let ordinary = vm.host().inner.hero_combat_bonuses(b).unwrap();
        assert_eq!(
            (ordinary.damage, ordinary.armor, ordinary.speed),
            (0.0, 0.0, 1.0)
        );
        for (level, damage, agility) in [(4, fourth, 6.0), (5, fifth, 8.0), (5, fifth, 8.0)] {
            vm.host()
                .inner
                .native(
                    "SetHeroLevel",
                    &[Value::Handle(a), Value::Int(level), Value::Bool(false)],
                )
                .unwrap();
            let bonuses = vm.host().inner.hero_combat_bonuses(a).unwrap();
            assert_eq!(bonuses.damage, damage * 1.75);
            assert!((bonuses.armor - (-1.0 + agility * 0.4)).abs() < 0.00001);
            assert!((bonuses.speed - (1.0 + agility * 0.2)).abs() < 0.00001);
        }
    }
}

#[test]
fn hero_attack_damage_point_cooldown_and_target_armor_use_the_derived_bonuses() {
    let (hero, combat) = hero_rules();
    let (mut vm, a, b) = open_selected(&hero_fields("STR"), Some(hero), combat);
    for (source, target) in [(a, b), (b, a)] {
        vm.host()
            .inner
            .native(
                "IssueTargetOrder",
                &[
                    Value::Handle(source),
                    Value::Str("alpha".into()),
                    Value::Handle(target),
                ],
            )
            .unwrap();
    }
    let life = |vm: &mut Vm, unit| vm.host().inner.unit_vitals(unit).unwrap().unwrap().life;
    let initial_a = life(&mut vm, a);
    let initial_b = life(&mut vm, b);
    step(&mut vm, 0.125);
    assert_eq!(life(&mut vm, b), initial_b);
    step(&mut vm, 0.15625);
    let damage_b = initial_b - life(&mut vm, b);
    let expected_b = (5.0 + 10.0 * 1.75) * 0.75 / (1.0 + 2.0 * 0.06);
    assert!((damage_b - expected_b).abs() < 0.0001);
    assert_eq!(life(&mut vm, a), initial_a);
    step(&mut vm, 0.28125);
    let expected_a = 5.0 * 0.75 / (1.0 + (2.0 - 1.0 + 5.0 * 0.4) * 0.06);
    assert!((initial_a - life(&mut vm, a) - expected_a).abs() < 0.0001);
    step(&mut vm, 0.875);
    assert!((initial_b - life(&mut vm, b) - expected_b).abs() < 0.0001);
    step(&mut vm, 0.90625);
    assert!((initial_b - life(&mut vm, b) - 2.0 * expected_b).abs() < 0.0001);
}

#[test]
fn missing_hero_state_or_attributes_and_invalid_derived_speed_refuse() {
    let (hero, combat) = hero_rules();
    let (mut vm, a, b) = open_selected(&hero_fields("AGI"), Some(hero.clone()), combat.clone());
    let definition = vm.host().inner.executed_unit(a).unwrap().definition.clone();
    let mut invalid = combat;
    invalid.attack_speed_per_agility = -1.0;
    assert!(
        ObjectHost::derive_hero_combat(&definition, Some((3, 3)), Default::default(), &invalid)
            .unwrap_err()
            .contains("non-positive attack speed")
    );
    for (field, value) in [
        (b"A005", None),
        (b"A007", Some("UNKNOWN")),
        (b"A002", Some("NaN")),
    ] {
        let mut bad = definition.clone();
        if let Some(value) = value {
            bad.fields.get_mut(field).unwrap().value = Ok(FieldValue::Cell(value.into()));
        } else {
            bad.fields.remove(field);
        }
        assert!(ObjectHost::derive_hero_combat(&bad, Some((3, 4)), Default::default(), &rules()).is_err());
    }
    assert!(ObjectHost::derive_hero_combat(&definition, Some((3, 2)), Default::default(), &rules()).is_err());
    let ordinary = vm.host().inner.executed_unit(b).unwrap().definition.clone();
    assert_eq!(
        ObjectHost::derive_hero_combat(&ordinary, None, Default::default(), &invalid)
            .unwrap()
            .speed,
        1.0
    );
    assert!(
        ObjectHost::derive_hero_combat(&definition, None, Default::default(), &rules())
            .unwrap_err()
            .contains("admitted initial and current levels")
    );
}

#[test]
fn explicit_protection_rejects_new_attacks_and_cancels_swings_without_erasing_queued_events() {
    let (mut vm, a, b) = open(&armed("64", "normal"));
    assert_eq!(vm.host().inner.executed_unit(b).unwrap().invulnerable, None);
    vm.call("listen", &[]).unwrap();
    vm.call("protect", &[]).unwrap();
    let attack = |vm: &mut Vm| {
        vm.host()
            .inner
            .native(
                "IssueTargetOrder",
                &[
                    Value::Handle(a),
                    Value::Str("alpha".into()),
                    Value::Handle(b),
                ],
            )
            .unwrap()
    };
    assert_eq!(attack(&mut vm), Value::Bool(false));
    step(&mut vm, 0.5);
    assert_eq!(vm.host().inner.unit_vitals(b).unwrap().unwrap().life, 20.0);
    assert!(vm.host().inner.take_fired_event().is_none());
    vm.call("unprotect", &[]).unwrap();
    assert_eq!(attack(&mut vm), Value::Bool(true));
    step(&mut vm, 0.6);
    vm.call("protect", &[]).unwrap();
    step(&mut vm, 0.9);
    let Value::Handle(hit) = vm.global("Hit") else {
        panic!("trigger");
    };
    let fired = vm
        .host()
        .inner
        .take_fired_event()
        .expect("earlier attacked event retained");
    assert_eq!(fired.trigger, hit);
    assert!(vm.host().inner.take_fired_event().is_none());
    assert_eq!(vm.host().inner.unit_vitals(b).unwrap().unwrap().life, 20.0);
    assert!(
        vm.host()
            .inner
            .executed_unit(a)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .target
            .is_none()
    );
    vm.call("unprotect", &[]).unwrap();
    assert_eq!(attack(&mut vm), Value::Bool(true));
    step(&mut vm, 2.5);
    assert!(vm.host().inner.unit_vitals(b).unwrap().unwrap().life < 20.0);
    assert_eq!(vm.host().inner.executed_unit(a).unwrap().invulnerable, None);
}

#[test]
fn auto_acquisition_skips_explicit_protection_and_cleared_protection_becomes_eligible() {
    let mut fields = armed("128", "normal");
    fields.push(column(
        b"W005",
        SlkId::UnitWeapons,
        "acquire",
        FieldKind::Real,
        "256",
    ));
    let (mut vm, a, b) = open(&fields);
    vm.call("protect", &[]).unwrap();
    let player = vm.host().inner.executed_unit(b).unwrap().owner;
    let Value::Handle(other) = vm
        .host()
        .inner
        .native(
            "CreateUnit",
            &[
                Value::Handle(player),
                Value::Int(i32::from_be_bytes(*b"u001")),
                Value::Real(80.0),
                Value::Real(16.0),
                Value::Real(0.0),
            ],
        )
        .unwrap()
    else {
        panic!("unit");
    };
    step(&mut vm, 0.1);
    assert_eq!(
        vm.host()
            .inner
            .executed_unit(a)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .target,
        Some(other)
    );
    assert!(
        vm.host().inner.is_enemy(a, b),
        "protection does not change hostility"
    );
    vm.host()
        .inner
        .native("KillUnit", &[Value::Handle(other)])
        .unwrap();
    vm.call("unprotect", &[]).unwrap();
    step(&mut vm, 0.2);
    assert_eq!(
        vm.host()
            .inner
            .executed_unit(a)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .target,
        Some(b)
    );
}

#[test]
fn rejected_protected_order_preserves_the_accepted_order_and_protection_stops_a_chase() {
    let (mut vm, a, b) = open(&armed("4", "normal"));
    let player = vm.host().inner.executed_unit(b).unwrap().owner;
    let Value::Handle(other) = vm
        .host()
        .inner
        .native(
            "CreateUnit",
            &[
                Value::Handle(player),
                Value::Int(i32::from_be_bytes(*b"u001")),
                Value::Real(96.0),
                Value::Real(16.0),
                Value::Real(0.0),
            ],
        )
        .unwrap()
    else {
        panic!("unit");
    };
    let attack = |vm: &mut Vm, target| {
        vm.host()
            .inner
            .native(
                "IssueTargetOrder",
                &[
                    Value::Handle(a),
                    Value::Str("alpha".into()),
                    Value::Handle(target),
                ],
            )
            .unwrap()
    };
    assert_eq!(attack(&mut vm, other), Value::Bool(true));
    vm.call("protect", &[]).unwrap();
    assert_eq!(attack(&mut vm, b), Value::Bool(false));
    assert_eq!(
        vm.host()
            .inner
            .executed_unit(a)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .target,
        Some(other)
    );
    vm.call("unprotect", &[]).unwrap();
    assert_eq!(attack(&mut vm, b), Value::Bool(true));
    step(&mut vm, 0.1);
    assert!(
        vm.host()
            .inner
            .executed_unit(a)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .moving()
    );
    vm.call("protect", &[]).unwrap();
    step(&mut vm, 0.2);
    let stopped = vm.host().inner.executed_unit(a).unwrap().position;
    step(&mut vm, 0.5);
    let attacker = vm.host().inner.executed_unit(a).unwrap();
    assert_eq!(attacker.position, stopped);
    assert!(!attacker.activity.as_ref().unwrap().moving());
}

#[test]
fn in_flight_missile_uses_current_explicit_protection_at_impact() {
    let mut fields = armed("640", "missile");
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
    for clear_before_impact in [false, true] {
        let (mut vm, a, b) = open(&fields);
        vm.host()
            .inner
            .native(
                "IssueTargetOrder",
                &[
                    Value::Handle(a),
                    Value::Str("alpha".into()),
                    Value::Handle(b),
                ],
            )
            .unwrap();
        step(&mut vm, 0.4);
        assert_eq!(vm.host().inner.unit_vitals(b).unwrap().unwrap().life, 20.0);
        vm.call("protect", &[]).unwrap();
        step(&mut vm, 0.6);
        if clear_before_impact {
            vm.call("unprotect", &[]).unwrap();
        }
        step(&mut vm, 0.8);
        let life = vm.host().inner.unit_vitals(b).unwrap().unwrap().life;
        if clear_before_impact {
            assert!(life < 20.0);
        } else {
            assert_eq!(life, 20.0);
        }
        assert_eq!(
            vm.host().inner.executed_unit(b).unwrap().invulnerable,
            Some(!clear_before_impact)
        );
    }
}

#[test]
fn explicit_protection_does_not_override_script_kill_or_direct_life_assignment() {
    let (mut vm, a, b) = open(&armed("64", "normal"));
    vm.call("protect", &[]).unwrap();
    vm.host()
        .inner
        .native("KillUnit", &[Value::Handle(b)])
        .unwrap();
    assert!(
        vm.host()
            .inner
            .executed_unit(b)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .dead
    );
    vm.host()
        .inner
        .native(
            "SetUnitInvulnerable",
            &[Value::Handle(a), Value::Bool(true)],
        )
        .unwrap();
    let state = vm.global("UNIT_STATE_LIFE");
    vm.host()
        .inner
        .native(
            "SetUnitState",
            &[Value::Handle(a), state, Value::Real(-1.0)],
        )
        .unwrap();
    assert!(
        vm.host()
            .inner
            .executed_unit(a)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .dead
    );
}

#[path = "unit_hero_vital_tests.rs"]
mod hero_vital_updates;

#[path = "unit_guard_tests.rs"]
mod guard_positions;

#[cfg(test)]
#[path = "inventory_powerup_host_tests.rs"]
mod powerup_host;

#[test]
fn set_unit_exploded_then_kill_unit_retains_the_flag_through_the_death_path() {
    // Melee: the missile path needs a positive missile speed, which this test does not model.
    let (mut vm, a, _b) = open(&armed("100", "normal"));
    let h = &mut vm.host().inner;
    assert_eq!(h.unit_exploded(a), None);
    h.native("SetUnitExploded", &[Value::Handle(a), Value::Bool(true)])
        .unwrap();
    assert_eq!(h.unit_exploded(a), Some(true));
    h.native("KillUnit", &[Value::Handle(a)]).unwrap();
    assert!(h.executed_unit(a).unwrap().combat.as_ref().unwrap().dead);
    assert_eq!(h.unit_exploded(a), Some(true), "the flag outlives the death");
    // The fixture unit has no model, so no explosion sequence exists: the death is recorded unrendered.
    assert!(h.exploded_death_unrendered(a));
    // Unverified policy: a dead unit is accepted. This pins the current host behavior.
    h.native("SetUnitExploded", &[Value::Handle(a), Value::Bool(false)])
        .unwrap();
    assert_eq!(h.unit_exploded(a), Some(false));
}


#[test]
fn hold_position_attacks_only_within_reach_and_never_chases_a_departing_target() {
    // Reach is range 40 plus both collision radii. Acquire 400 would select B for an idle unit.
    let mut fields = armed("40", "normal");
    fields.push(column(
        b"S001",
        SlkId::UnitBalance,
        "spd",
        FieldKind::Real,
        "32",
    ));
    fields
        .iter_mut()
        .find(|(id, _)| id == b"W005")
        .unwrap()
        .1
        .value = Ok(FieldValue::Cell("400".into()));
    let (mut vm, a, b) = open(&fields);
    let target = |h: &ObjectHost, unit: HandleId| {
        h.executed_unit(unit)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .target
    };
    let h = &mut vm.host().inner;
    // B must not acquire A itself; only A's acquisition of B is under test.
    let alliance = h.native("ConvertAllianceType", &[Value::Int(0)]).unwrap();
    h.native(
        "SetPlayerAlliance",
        &[
            Value::Handle(h.executed_unit(b).unwrap().owner),
            Value::Handle(h.executed_unit(a).unwrap().owner),
            alliance,
            Value::Bool(true),
        ],
    )
    .unwrap();
    // In this fixture "alpha" is both Hold and the attack order; this test issues no attack.
    assert_eq!(
        h.native(
            "IssueImmediateOrder",
            &[Value::Handle(a), Value::Str("alpha".into())]
        )
        .unwrap(),
        Value::Bool(true)
    );
    let start = h.timer_game_time().unwrap();
    step(&mut vm, start + 0.1);
    assert_eq!(
        target(&vm.host().inner, a),
        Some(b),
        "B at 32 is within reach"
    );
    // B walks away at 32 per second. A holds its position and drops B once B is out of reach.
    let h = &mut vm.host().inner;
    let walk = [
        Value::Handle(b),
        Value::Str("gamma".into()),
        Value::Real(96.0),
        Value::Real(16.0),
    ];
    assert_eq!(
        h.native("IssuePointOrder", &walk).unwrap(),
        Value::Bool(true)
    );
    let start = h.timer_game_time().unwrap();
    step(&mut vm, start + 3.0);
    let h = &mut vm.host().inner;
    assert_eq!(h.executed_unit(b).unwrap().position, Some([96.0, 16.0]));
    assert_eq!(target(h, a), None);
    assert_eq!(h.executed_unit(a).unwrap().position, Some([16.0, 16.0]));
    let held = h
        .executed_unit(a)
        .unwrap()
        .activity
        .as_ref()
        .unwrap()
        .held();
    assert!(held);
    // Stop releases the hold: the same walking enemy is acquired by acquire range.
    assert_eq!(
        h.native(
            "IssueImmediateOrder",
            &[Value::Handle(a), Value::Str("beta".into())]
        )
        .unwrap(),
        Value::Bool(true)
    );
    let start = h.timer_game_time().unwrap();
    step(&mut vm, start + 0.1);
    assert_eq!(target(&vm.host().inner, a), Some(b));
}

fn traced_actor(h: &ObjectHost, unit: HandleId) -> CombatActor {
    h.combat_actor(unit).unwrap()
}

#[test]
fn combat_trace_records_acquisition_then_its_damage_with_source_identity() {
    let mut fields = armed("64", "normal");
    fields
        .iter_mut()
        .find(|(id, _)| id == b"W005")
        .unwrap()
        .1
        .value = Ok(FieldValue::Cell("400".into()));
    let (mut vm, a, b) = open(&fields);
    let h = &mut vm.host().inner;
    assert!(h.combat_trace().is_none());
    h.enable_combat_trace(16);
    let start = h.timer_game_time().unwrap();
    step(&mut vm, start + 1.0);
    let (source, target) = (
        traced_actor(&vm.host().inner, a),
        traced_actor(&vm.host().inner, b),
    );
    let trace = vm.host().inner.combat_trace().unwrap();
    assert_eq!(trace.dropped(), 0);
    assert_eq!(source.player, Some(4));
    assert_eq!(target.player, Some(5));
    assert_eq!(source.rawcode, *b"u001");
    let events = trace.events();
    let CombatTraceEvent::Acquired {
        source: s,
        target: t,
        clock,
    } = events[0]
    else {
        panic!("first event {:?}", events[0]);
    };
    assert_eq!((s, t), (source, target));
    assert!(clock.is_some_and(|c| c >= start && c <= start + 1.0));
    let damage = events.iter().find_map(|e| match e {
        CombatTraceEvent::Damage {
            source: s,
            target: t,
            damage,
            life_before,
            ..
        } => Some((*s, *t, *damage, *life_before)),
        _ => None,
    });
    let (s, t, dealt, life_before) = damage.expect("the acquired target is hit");
    assert_eq!((s, t), (source, target));
    assert!(dealt > 0.0);
    assert_eq!(life_before, Some(20.0));
}

#[test]
fn combat_trace_records_an_explicit_target_order_before_its_damage() {
    let (mut vm, a, b) = open(&armed("64", "normal"));
    vm.host().inner.enable_combat_trace(16);
    let h = &mut vm.host().inner;
    assert_eq!(
        h.native(
            "IssueTargetOrder",
            &[Value::Handle(a), Value::Str("alpha".into()), Value::Handle(b)]
        )
        .unwrap(),
        Value::Bool(true)
    );
    let start = h.timer_game_time().unwrap();
    step(&mut vm, start + 1.0);
    let trace = vm.host().inner.combat_trace().unwrap();
    let events = trace.events();
    let CombatTraceEvent::Ordered { target, .. } = events[0] else {
        panic!("first event {:?}", events[0]);
    };
    assert_eq!(target.unit, b);
    assert!(matches!(events.get(1), Some(CombatTraceEvent::Damage { .. })));
    assert!(!events.iter().any(|e| matches!(e, CombatTraceEvent::Acquired { .. })));
}

#[test]
fn explicit_target_order_attacks_a_passive_ally_while_acquisition_skips_it() {
    let mut fields = armed("64", "normal");
    fields
        .iter_mut()
        .find(|(id, _)| id == b"W005")
        .unwrap()
        .1
        .value = Ok(FieldValue::Cell("400".into()));
    let (mut vm, a, b) = open(&fields);
    let h = &mut vm.host().inner;
    // Passive alliance is directed: set it both ways so neither side acquires the other.
    let alliance = h.native("ConvertAllianceType", &[Value::Int(0)]).unwrap();
    let (owner_a, owner_b) = (
        h.executed_unit(a).unwrap().owner,
        h.executed_unit(b).unwrap().owner,
    );
    for (source, other) in [(owner_a, owner_b), (owner_b, owner_a)] {
        h.native(
            "SetPlayerAlliance",
            &[
                Value::Handle(source),
                Value::Handle(other),
                alliance.clone(),
                Value::Bool(true),
            ],
        )
        .unwrap();
    }
    h.enable_combat_trace(16);
    let start = h.timer_game_time().unwrap();
    step(&mut vm, start + 1.0);
    // Automatic acquisition never selects the passive ally.
    assert_eq!(
        vm.host().inner.combat_trace().unwrap().events(),
        &[] as &[CombatTraceEvent]
    );
    let h = &mut vm.host().inner;
    assert_eq!(
        h.native(
            "IssueTargetOrder",
            &[Value::Handle(a), Value::Str("alpha".into()), Value::Handle(b)]
        )
        .unwrap(),
        Value::Bool(true)
    );
    let start = h.timer_game_time().unwrap();
    step(&mut vm, start + 1.0);
    let h = &mut vm.host().inner;
    assert!(h.unit_vitals(b).unwrap().unwrap().life < 20.0);
    assert!(matches!(
        h.combat_trace().unwrap().events()[0],
        CombatTraceEvent::Ordered { .. }
    ));
}

/// `armed` with regeneration rates and a 50-point mana pool. Keys `H002`/`H003`
/// override the base mana columns, and `H006`..`H008` the regeneration columns.
fn regen_fields(
    life_per_second: &str,
    life_type: &str,
    mana_per_second: &str,
) -> Vec<([u8; 4], ObjectField)> {
    let mut fields = armed("64", "normal");
    for (id, column_name, kind, value) in [
        (b"H002", "manaN", FieldKind::Int, "50"),
        (b"H003", "mana0", FieldKind::Int, "0"),
        (b"H006", "regenHP", FieldKind::Real, life_per_second),
        (b"H007", "regenMana", FieldKind::Real, mana_per_second),
        (b"H008", "regenType", FieldKind::Token, life_type),
    ] {
        fields.push(column(id, SlkId::UnitBalance, column_name, kind, value));
    }
    fields
}

fn set_pools(vm: &mut Vm, unit: HandleId, life: f32, mana: f32) {
    let state = |vm: &mut Vm, name: &str| vm.global(name);
    for (name, value) in [("UNIT_STATE_LIFE", life), ("UNIT_STATE_MANA", mana)] {
        let property = state(vm, name);
        vm.host()
            .inner
            .native(
                "SetUnitState",
                &[Value::Handle(unit), property, Value::Real(value)],
            )
            .unwrap();
    }
}

fn pools(vm: &mut Vm, unit: HandleId) -> (f32, f32) {
    let v = vm.host().inner.unit_vitals(unit).unwrap().unwrap();
    (v.life, v.mana)
}

#[test]
fn regeneration_integrates_stock_rates_and_caps_at_maxima_for_any_tick_chunking() {
    let fields = regen_fields("0.25", "always", "0.5");
    let (mut whole, a, b) = open(&fields);
    let (mut stepped, c, d) = open(&fields);
    set_pools(&mut whole, a, 5.0, 0.0);
    set_pools(&mut whole, b, 19.9, 49.0);
    set_pools(&mut stepped, c, 5.0, 0.0);
    set_pools(&mut stepped, d, 19.9, 49.0);
    whole
        .host()
        .inner
        .next_timer_expiry(10.0, &mut 100)
        .unwrap();
    whole
        .host()
        .inner
        .advance_unit_activity(10.0, &mut 65536)
        .unwrap();
    step(&mut stepped, 10.0);
    // 5 + 0.25 * 10 and 0 + 0.5 * 10 are exact; the second unit hits both maxima.
    assert_eq!(pools(&mut whole, a), (7.5, 5.0));
    assert_eq!(pools(&mut stepped, c), (7.5, 5.0));
    assert_eq!(pools(&mut whole, b), (20.0, 50.0));
    assert_eq!(pools(&mut stepped, d), (20.0, 50.0));
}

#[test]
fn regeneration_never_applies_to_units_without_rate_or_with_none() {
    let fields = regen_fields("0", "none", "0");
    let (mut vm, a, _) = open(&fields);
    set_pools(&mut vm, a, 5.0, 0.0);
    step(&mut vm, 10.0);
    assert_eq!(pools(&mut vm, a), (5.0, 0.0));
}

#[test]
fn night_and_blight_regeneration_refuse_without_their_source_state() {
    for (life_type, reason) in [
        (
            "night",
            "day/night regeneration needs the admitted time-of-day window",
        ),
        ("blight", "blight regeneration needs terrain blight state"),
    ] {
        let fields = regen_fields("0.25", life_type, "0");
        let (mut vm, a, _) = open(&fields);
        set_pools(&mut vm, a, 5.0, 0.0);
        vm.host().inner.next_timer_expiry(1.0, &mut 100).unwrap();
        let error = vm
            .host()
            .inner
            .advance_unit_activity(1.0, &mut 65536)
            .unwrap_err()
            .to_string();
        assert!(error.contains(reason), "{life_type}: {error}");
        // The refused span is not integrated: the pool and projection clock are unchanged.
        assert_eq!(pools(&mut vm, a), (5.0, 0.0));
    }
}

#[test]
fn dead_units_do_not_regenerate_and_their_clock_still_advances() {
    let fields = regen_fields("0.25", "always", "0.5");
    let (mut vm, a, _) = open(&fields);
    vm.host()
        .inner
        .native("KillUnit", &[Value::Handle(a)])
        .unwrap();
    // Life above zero on a dead unit isolates the dead flag from the lifeless check.
    set_pools(&mut vm, a, 5.0, 0.0);
    step(&mut vm, 10.0);
    let vitals = vm.host().inner.unit_vitals(a).unwrap().unwrap();
    assert_eq!((vitals.life, vitals.mana), (5.0, 0.0));
    assert_eq!(vitals.regen.through, 10.0);
}

fn clock_hour(vm: &mut Vm, hour: f64) -> Result<Value, String> {
    let clock = vm.global("CLOCK");
    vm.host()
        .inner
        .native("SetFloatGameState", &[clock, Value::Real(hour as f32)])
}

fn clock_scale(vm: &mut Vm, scale: f32) -> Result<Value, String> {
    vm.host()
        .inner
        .native("SetTimeOfDayScale", &[Value::Real(scale)])
}

fn assert_pools(vm: &mut Vm, unit: HandleId, life: f32, mana: f32) {
    let (l, m) = pools(vm, unit);
    assert!(
        (l - life).abs() < 1e-3 && (m - mana).abs() < 1e-3,
        "pools ({l}, {m}) != ({life}, {mana})"
    );
}

#[test]
fn clock_rebases_integrate_each_span_once_for_day_and_night_regeneration() {
    // Hour 17.5 at scale 1 (0.05 h/s). The clock moves to t=6 without projecting the
    // unit, then scale 2 (0.1 h/s) rebases there: 0 to 6 s is at 0.05 h/s (all day),
    // 6 to 8 s at 0.1 h/s reaches dusk, so day spans 8 s of 20 s.
    for (life_type, expected) in [("day", 13.0), ("night", 17.0)] {
        let fields = regen_fields("1", life_type, "0");
        let (mut vm, a, _) = open_clocked(&fields, rules());
        set_pools(&mut vm, a, 5.0, 0.0);
        clock_hour(&mut vm, 17.5).unwrap();
        clock_scale(&mut vm, 1.0).unwrap();
        step(&mut vm, 4.0);
        vm.host().inner.next_timer_expiry(6.0, &mut 8).unwrap();
        clock_scale(&mut vm, 2.0).unwrap();
        step(&mut vm, 20.0);
        assert_pools(&mut vm, a, expected, 0.0);
    }
}

#[test]
fn rejected_clock_change_leaves_pools_and_clock_unchanged() {
    let fields = regen_fields("1", "day", "0");
    let (mut vm, a, _) = open_clocked(&fields, rules());
    set_pools(&mut vm, a, 5.0, 0.0);
    clock_hour(&mut vm, 17.5).unwrap();
    clock_scale(&mut vm, 1.0).unwrap();
    step(&mut vm, 4.0);
    let before = pools(&mut vm, a);
    assert!(clock_scale(&mut vm, f32::NAN).is_err());
    assert!(clock_hour(&mut vm, f64::NAN).is_err());
    assert_eq!(pools(&mut vm, a), before);
    assert_eq!(
        vm.host().inner.native("GetTimeOfDayScale", &[]).unwrap(),
        Value::Real(1.0)
    );
    // Scale 1 reaches dusk at t=10, so day covers 10 s of the 20 s span.
    step(&mut vm, 20.0);
    assert_pools(&mut vm, a, 15.0, 0.0);
}

#[test]
fn mid_run_units_and_manually_lifed_dead_units_gain_no_retroactive_regeneration() {
    let fields = regen_fields("1", "always", "0");
    let (mut vm, a, _) = open(&fields);
    set_pools(&mut vm, a, 5.0, 0.0);
    step(&mut vm, 5.0);
    vm.call("spawn", &[]).unwrap();
    let c = match vm.global("C") {
        Value::Handle(id) => id,
        other => panic!("spawned unit: {other:?}"),
    };
    set_pools(&mut vm, c, 5.0, 0.0);
    step(&mut vm, 10.0);
    assert_pools(&mut vm, a, 15.0, 0.0);
    assert_pools(&mut vm, c, 10.0, 0.0);

    // Manual fixture policy, not a revive: the dead flag stays set and SetUnitState writes life.
    // ReviveHero/ReviveHeroLoc are declared by common.j but not admitted by this host.
    vm.host()
        .inner
        .native("KillUnit", &[Value::Handle(a)])
        .unwrap();
    step(&mut vm, 13.0);
    set_pools(&mut vm, a, 5.0, 0.0);
    step(&mut vm, 20.0);
    assert_pools(&mut vm, a, 5.0, 0.0);
    assert_eq!(
        vm.host()
            .inner
            .unit_vitals(a)
            .unwrap()
            .unwrap()
            .regen
            .through,
        20.0
    );
}

#[test]
fn moving_unit_regenerates_at_its_rate_while_it_travels() {
    let fields = regen_fields("1", "always", "0");
    let (mut vm, a, _) = open(&fields);
    set_pools(&mut vm, a, 5.0, 0.0);
    let ordered = vm
        .host()
        .inner
        .native(
            "IssuePointOrder",
            &[
                Value::Handle(a),
                Value::Str("gamma".into()),
                Value::Real(72.0),
                Value::Real(16.0),
            ],
        )
        .unwrap();
    assert_eq!(ordered, Value::Bool(true));
    step(&mut vm, 10.0);
    assert_pools(&mut vm, a, 15.0, 0.0);
    let position = vm.host().inner.executed_unit(a).unwrap().position.unwrap();
    assert!((position[0] - 72.0).abs() < 1e-3, "{position:?}");
}

/// Holy Light rows: `AHhb` (three ranks, `Order=alpha` here) and the hero component `Yher`.
fn holy_tables() -> DefaultTables {
    let headers = [
        "alias", "code", "levels", "reqLevel", "levelSkip", "hero", "DataA1", "DataA2", "DataA3",
        "Cost1", "Cost2", "Cost3", "Cool1", "Cool2", "Cool3", "Rng1", "Rng2", "Rng3", "targs1",
        "targs2", "targs3",
    ];
    let targs = "air,ground,organic,notself,invu,vuln,nonancient";
    let rows: [Vec<&str>; 2] = [
        vec![
            "AHhb", "AHhb", "3", "1", "0", "1", "200", "400", "600", "65", "65", "65", "5", "5",
            "5", "60", "60", "60", targs, targs, targs,
        ],
        vec![
            "Yher", "Yher", "1", "0", "0", "1", "0", "0", "0", "0", "0", "0", "0", "0", "0", "0",
            "0", "0", "air,ground,invu,vuln", "air,ground,invu,vuln", "air,ground,invu,vuln",
        ],
    ];
    let mut slk = String::from("ID;PWXL;N;E\r\n");
    for (column, name) in headers.iter().enumerate() {
        slk.push_str(&format!("C;X{};Y1;K\"{name}\"\r\n", column + 1));
    }
    for (index, row) in rows.iter().enumerate() {
        for (column, value) in row.iter().enumerate() {
            let value = if value.parse::<f64>().is_ok() {
                (*value).to_owned()
            } else {
                format!("\"{value}\"")
            };
            slk.push_str(&format!("C;X{};Y{};K{value}\r\n", column + 1, index + 2));
        }
    }
    slk.push_str("E\r\n");
    let profile = "[AHhb]\r\nOrder=alpha\r\nHotkey=T\r\nTargetart=Abilities\\Spells\\Human\\HolyBolt\\HolyBoltSpecialArt.mdl\r\nUberTip=\"deal half damage to an enemy Undead unit.\"\r\n";
    DefaultTables::from_bytes(
        Identity {
            install: source().install,
            edition_version: "fixture".into(),
        },
        &[(SlkId::AbilityData.path(), slk.as_bytes())],
        &[("Units\\HumanAbilityFunc.txt", profile.as_bytes())],
        pcx_objects::editor::EditorReadLimits {
            max_listed_paths: 4,
            max_asset_bytes: 64 << 10,
            max_aggregate_input_bytes: 128 << 10,
        },
    )
    .expect("synthetic holy light tables")
}

const HOLY_MAP: &str = r#"
globals
unit H=null
unit F=null
unit E=null
unit G=null
trigger T=null
endglobals
function setup takes nothing returns nothing
set H=CreateUnit(Player(4),'H001',16.0,16.0,0.0)
set F=CreateUnit(Player(4),'u001',48.0,16.0,0.0)
set E=CreateUnit(Player(5),'u001',56.0,16.0,180.0)
set G=CreateUnit(Player(4),'u001',120.0,16.0,0.0)
endfunction
function prime takes nothing returns nothing
call SetUnitState(H,UNIT_STATE_MANA,200.0)
call SetUnitState(F,UNIT_STATE_LIFE,100.0)
endfunction
function listen takes nothing returns nothing
set T=CreateTrigger()
call TriggerRegisterUnitEvent(T,H,EVENT_UNIT_SPELL_EFFECT)
endfunction
function learn takes integer ability returns nothing
call SelectHeroSkill(H,ability)
endfunction
function cast takes unit c, unit t returns boolean
return IssueTargetOrder(c,"alpha",t)
endfunction
function attack_enemy takes unit c, unit t returns nothing
call IssueTargetOrder(c,"beta",t)
endfunction
"#;

const HOLY_DECLARATIONS: &str = r#"
globals
constant unitevent EVENT_UNIT_SPELL_EFFECT = ConvertUnitEvent(294)
constant playerunitevent EVENT_PLAYER_UNIT_SPELL_EFFECT = ConvertPlayerUnitEvent(277)
constant unittype UNIT_TYPE_UNDEAD = ConvertUnitType(2)
constant unittype UNIT_TYPE_ANCIENT = ConvertUnitType(3)
endglobals
type effect extends agent
native AddSpecialEffectTarget takes string model, widget target, string attachPoint returns effect
native DestroyEffect takes effect e returns nothing
native SelectHeroSkill takes unit u, integer ability returns nothing
type unittype extends handle
constant native ConvertUnitType takes integer i returns unittype
native IsUnitType takes unit u, unittype t returns boolean
constant native GetSpellTargetUnit takes nothing returns unit
"#;

/// Holy Light caster `H` (hero), friendly `F`, enemy `E`, far friend `G` (120 units away).
/// Every unit shares one fixture definition, so `types` is the `type` list of all of them.
/// `abilities` and `heroes` fill the `abilList` and `heroAbilList` of the caster.
fn holy_open(types: &str, abilities: &str, heroes: &str) -> (Vm, [HandleId; 4]) {
    let mut fields = hero_fields("STR");
    fields.push(column(b"H001", SlkId::UnitBalance, "HP", FieldKind::Int, "1000"));
    fields.push(column(b"H003", SlkId::UnitBalance, "mana0", FieldKind::Int, "300"));
    fields.push(column(b"A010", SlkId::UnitBalance, "type", FieldKind::Token, types));
    fields.push(column(b"A008", SlkId::UnitAbilities, "abilList", FieldKind::Token, abilities));
    fields.push(column(b"A009", SlkId::UnitAbilities, "heroAbilList", FieldKind::Token, heroes));
    let extra = format!(
        "{}\n{}{DECLARATIONS}{HOLY_DECLARATIONS}",
        enumeration::TRIGGERS,
        super::activity::ORDERS
    );
    let (registry, activity) = super::activity::rules();
    let h = position_host_admitting(1000, 16, HOLY_MAP, &extra, &fields, |h| h)
        .with_locations()
        .unwrap()
        .with_player_unit_events(8)
        .unwrap()
        .with_unit_states(crate::playercolor_host::UnitStateBudget { max_records: 8 })
        .unwrap()
        .with_player_alliances(crate::playercolor_host::PlayerAllianceBudget { max_records: 8 })
        .unwrap()
        .with_unit_explosions(8)
        .unwrap()
        .with_order_registry(registry)
        .unwrap()
        .with_script_random(Arc::new(crate::script_random::profile::fixture()), 64)
        .unwrap();
    let mut h = h.with_unit_effects().unwrap();
    let origin = crate::scripts::AssetIdentity {
        source: source().install,
        edition: "fixture".into(),
        version: "fixture".into(),
    };
    h = h
        .with_hero_skills(
            holy_tables(),
            origin,
            Default::default(),
            hero_rules().0,
            crate::playercolor_host::HeroBudget {
                abilities_per_unit: 8,
                overlay_objects: 0,
                modifications_per_ability: 0,
            },
        )
        .unwrap();
    h.admit_unit_activity(activity, 65536).unwrap();
    h.native("SetRandomSeed", &[Value::Int(7)]).unwrap();
    h.admit_unit_vitals(HeroVitals {
        life_per_strength: 25.0,
        mana_per_intelligence: 100.0,
    })
    .unwrap();
    h.admit_combat(rules(), "beta").unwrap();
    h.admit_holy_light().unwrap();
    h.set_neutral_passive(15);
    let context = Arc::clone(h.player_context().unwrap());
    let mut vm = context.scripts.clone().owned_vm(h);
    vm.init_globals().unwrap();
    vm.call("setup", &[]).unwrap();
    let id = |vm: &Vm, name| match vm.global(name) {
        Value::Handle(id) => id,
        _ => panic!("{name}"),
    };
    let ids = [id(&vm, "H"), id(&vm, "F"), id(&vm, "E"), id(&vm, "G")];
    (vm, ids)
}

fn holy_cast(vm: &mut Vm, caster: HandleId, target: HandleId) -> bool {
    match vm
        .call("cast", &[Value::Handle(caster), Value::Handle(target)])
        .unwrap()
    {
        Value::Bool(cast) => cast,
        other => panic!("cast returned {other:?}"),
    }
}

fn holy_life(vm: &mut Vm, unit: HandleId) -> f32 {
    vm.host().inner.unit_vitals(unit).unwrap().unwrap().life
}

fn holy_mana(vm: &mut Vm, unit: HandleId) -> f32 {
    vm.host().inner.unit_vitals(unit).unwrap().unwrap().mana
}

fn holy_learn(vm: &mut Vm, ability: i32) {
    vm.call("learn", &[Value::Int(ability)]).unwrap();
}

const LEARNED: &str = "AHhb";
const UNLEARNED: &str = "_";
const LEARNABLE: &str = "AHhb";
const NOTHING: &str = "_";

/// `AHhb` as `Value::Int` rawcode, as scripts pass it.
fn holy_code() -> i32 {
    i32::from_be_bytes(*b"AHhb")
}

#[test]
fn holy_light_heals_a_friendly_living_unit_and_waits_out_its_cooldown() {
    let (mut vm, [h, f, _, _]) = holy_open("", LEARNED, NOTHING);
    vm.call("prime", &[]).unwrap();
    let mana = holy_mana(&mut vm, h);
    assert_eq!(holy_life(&mut vm, f), 100.0);
    assert!(holy_cast(&mut vm, h, f));
    assert_eq!(holy_life(&mut vm, f), 300.0);
    assert_eq!(holy_mana(&mut vm, h), mana - 65.0);
    assert!(!holy_cast(&mut vm, h, f), "cooldown refuses the recast");
    step(&mut vm, 5.0);
    assert!(holy_cast(&mut vm, h, f));
    assert_eq!(holy_life(&mut vm, f), 500.0);
}

#[test]
fn holy_light_damages_an_enemy_undead_unit_for_half_the_heal_amount() {
    let (mut vm, [h, f, e, _]) = holy_open("undead", LEARNED, NOTHING);
    vm.call("prime", &[]).unwrap();
    let enemy = holy_life(&mut vm, e);
    assert!(!holy_cast(&mut vm, h, f), "friendly undead is not organic");
    assert!(holy_cast(&mut vm, h, e));
    assert_eq!(enemy - holy_life(&mut vm, e), 100.0);
}

#[test]
fn unit_type_undead_reads_the_type_token_column_through_the_placement_reader() {
    let (mut vm, [_, _, e, _]) = holy_open("undead", LEARNED, NOTHING);
    let undead = vm.global("UNIT_TYPE_UNDEAD");
    let check = |vm: &mut Vm, unit| {
        vm.host()
            .inner
            .native("IsUnitType", &[Value::Handle(unit), undead.clone()])
            .unwrap()
    };
    assert_eq!(check(&mut vm, e), Value::Bool(true));
    let (mut vm, [_, _, e, _]) = holy_open("", LEARNED, NOTHING);
    let undead = vm.global("UNIT_TYPE_UNDEAD");
    assert_eq!(
        vm.host()
            .inner
            .native("IsUnitType", &[Value::Handle(e), undead])
            .unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn ancient_undead_and_enemy_organic_and_mana_refusals_leave_state_unchanged() {
    let (mut vm, [h, f, e, _]) = holy_open("undead,ancient", LEARNED, NOTHING);
    vm.call("prime", &[]).unwrap();
    let (enemy, mana) = (holy_life(&mut vm, e), holy_mana(&mut vm, h));
    assert!(!holy_cast(&mut vm, h, e), "ancient is excluded by nonancient");
    assert_eq!((holy_life(&mut vm, e), holy_mana(&mut vm, h)), (enemy, mana));
    assert!(!holy_cast(&mut vm, f, h), "caster without the ability");
    let (mut vm, [h, f, e, _]) = holy_open("", LEARNED, NOTHING);
    vm.call("prime", &[]).unwrap();
    let enemy = holy_life(&mut vm, e);
    assert!(!holy_cast(&mut vm, h, e), "enemy organic is not a target");
    assert_eq!(holy_life(&mut vm, e), enemy);
    let mana_state = vm_global(&vm, "UNIT_STATE_MANA");
    vm.host()
        .inner
        .native("SetUnitState", &[Value::Handle(h), mana_state, Value::Real(10.0)])
        .unwrap();
    assert!(!holy_cast(&mut vm, h, f), "insufficient mana");
    assert_eq!(holy_life(&mut vm, f), 100.0);
}

#[test]
fn out_of_range_orders_walk_the_caster_into_range_then_cast_once() {
    let (mut vm, [h, _, _, g]) = holy_open("", LEARNED, NOTHING);
    vm.call("prime", &[]).unwrap();
    let life_state = vm_global(&vm, "UNIT_STATE_LIFE");
    vm.host()
        .inner
        .native("SetUnitState", &[Value::Handle(g), life_state, Value::Real(100.0)])
        .unwrap();
    let (life, mana) = (holy_life(&mut vm, g), holy_mana(&mut vm, h));
    assert!(holy_cast(&mut vm, h, g), "accepted and walking");
    assert_eq!((holy_life(&mut vm, g), holy_mana(&mut vm, h)), (life, mana));
    let mut until = 0.0;
    while holy_life(&mut vm, g) == life && until < 60.0 {
        until += 0.5;
        step(&mut vm, until);
    }
    assert_eq!(holy_life(&mut vm, g), life + 200.0);
    assert_eq!(holy_mana(&mut vm, h), mana - 65.0);
    let at = vm.host().inner.executed_unit(h).unwrap().position.unwrap();
    let goal = vm.host().inner.executed_unit(g).unwrap().position.unwrap();
    let gap = ((at[0] - goal[0]).powi(2) + (at[1] - goal[1]).powi(2)).sqrt();
    assert!(gap <= 60.0, "{at:?} {goal:?}");
    step(&mut vm, until + 10.0);
    assert_eq!(holy_life(&mut vm, g), life + 200.0, "no second cast");
}

#[test]
fn a_new_order_cancels_a_walking_cast_without_spending_mana() {
    let (mut vm, [h, _, e, g]) = holy_open("", LEARNED, NOTHING);
    vm.call("prime", &[]).unwrap();
    let (life, mana) = (holy_life(&mut vm, g), holy_mana(&mut vm, h));
    assert!(holy_cast(&mut vm, h, g));
    vm.call("attack_enemy", &[Value::Handle(h), Value::Handle(e)]).unwrap();
    step(&mut vm, 30.0);
    assert_eq!(holy_life(&mut vm, g), life);
    assert_eq!(holy_mana(&mut vm, h), mana);
}

#[test]
fn ranks_are_learned_with_skill_points_and_each_rank_uses_its_own_row() {
    let (mut vm, [h, f, _, _]) = holy_open("", UNLEARNED, LEARNABLE);
    vm.call("prime", &[]).unwrap();
    assert!(!holy_cast(&mut vm, h, f), "not learned yet");
    holy_learn(&mut vm, holy_code());
    assert!(holy_cast(&mut vm, h, f));
    assert_eq!(holy_life(&mut vm, f), 300.0, "rank one heals DataA1");
    step(&mut vm, 5.0);
    holy_learn(&mut vm, holy_code());
    assert!(holy_cast(&mut vm, h, f));
    assert_eq!(holy_life(&mut vm, f), 700.0, "rank two heals DataA2");
    step(&mut vm, 10.0);
    holy_learn(&mut vm, holy_code());
    assert!(holy_cast(&mut vm, h, f));
    assert_eq!(holy_life(&mut vm, f), 1000.0, "rank three heals DataA3, capped at max life");
}

#[test]
fn spell_effect_event_names_the_target_and_fires_once_per_cast() {
    let (mut vm, [h, f, _, _]) = holy_open("", LEARNED, NOTHING);
    vm.call("prime", &[]).unwrap();
    vm.call("listen", &[]).unwrap();
    assert!(holy_cast(&mut vm, h, f));
    let fired = vm.host().inner.take_fired_event().expect("spell effect event");
    assert_eq!(fired.subject.unit, Some(h));
    assert_eq!(fired.subject.spell_target, Some(f));
    assert!(vm.host().inner.take_fired_event().is_none());
}

#[test]
fn invulnerable_undead_is_targetable_but_takes_no_damage() {
    let (mut vm, [h, _, e, _]) = holy_open("undead", LEARNED, NOTHING);
    vm.call("prime", &[]).unwrap();
    vm.host()
        .inner
        .native("SetUnitInvulnerable", &[Value::Handle(e), Value::Bool(true)])
        .unwrap();
    let enemy = holy_life(&mut vm, e);
    let mana = holy_mana(&mut vm, h);
    assert!(holy_cast(&mut vm, h, e));
    assert_eq!(holy_life(&mut vm, e), enemy);
    assert_eq!(holy_mana(&mut vm, h), mana - 65.0);
}

#[test]
fn learned_holy_light_names_its_hotkey_and_order_to_the_player_command() {
    let (mut vm, [h, _, _, _]) = holy_open("", LEARNED, NOTHING);
    let command = vm
        .host()
        .inner
        .holy_light_command(h)
        .unwrap()
        .expect("learned Holy Light");
    // The activity fixture registers `alpha` as order 431.
    assert_eq!((command.hotkey, command.order), ('T', 431));
    let (mut vm, [h, _, _, _]) = holy_open("", UNLEARNED, NOTHING);
    assert!(vm.host().inner.holy_light_command(h).unwrap().is_none());
}

#[test]
fn a_passive_alliance_makes_an_enemy_living_unit_a_heal_target() {
    let (mut vm, [h, _, e, _]) = holy_open("", LEARNED, NOTHING);
    vm.call("prime", &[]).unwrap();
    let life_state = vm_global(&vm, "UNIT_STATE_LIFE");
    vm.host()
        .inner
        .native("SetUnitState", &[Value::Handle(e), life_state, Value::Real(100.0)])
        .unwrap();
    assert!(!holy_cast(&mut vm, h, e), "enemy organic before the alliance");
    let alliance = vm_global(&vm, "ALLIANCE_PASSIVE");
    let h_owner = vm.host().inner.native("Player", &[Value::Int(4)]).unwrap();
    let e_owner = vm.host().inner.native("Player", &[Value::Int(5)]).unwrap();
    vm.host()
        .inner
        .native(
            "SetPlayerAlliance",
            &[h_owner, e_owner, alliance, Value::Bool(true)],
        )
        .unwrap();
    assert!(holy_cast(&mut vm, h, e));
    assert_eq!(holy_life(&mut vm, e), 300.0);
}

#[test]
fn the_targetart_shows_on_the_target_and_a_recast_replaces_it() {
    let (mut vm, [h, f, _, _]) = holy_open("", LEARNED, NOTHING);
    vm.call("prime", &[]).unwrap();
    assert!(holy_cast(&mut vm, h, f));
    let art: Vec<_> = vm.host().inner.unit_effects().collect();
    assert_eq!(art.len(), 1, "{art:?}");
    let (_, effect) = art[0];
    assert_eq!(effect.unit, f);
    assert_eq!(effect.model, "Abilities\\Spells\\Human\\HolyBolt\\HolyBoltSpecialArt.mdl");
    assert_eq!(effect.attach_point, "origin", "no Targetattach is authored");
    step(&mut vm, 5.0);
    assert!(holy_cast(&mut vm, h, f));
    assert_eq!(vm.host().inner.unit_effects().count(), 1);
}

#[test]
fn killing_the_caster_cancels_a_walking_cast() {
    let (mut vm, [h, _, _, g]) = holy_open("", LEARNED, NOTHING);
    vm.call("prime", &[]).unwrap();
    let life_state = vm_global(&vm, "UNIT_STATE_LIFE");
    vm.host()
        .inner
        .native("SetUnitState", &[Value::Handle(g), life_state, Value::Real(100.0)])
        .unwrap();
    assert!(holy_cast(&mut vm, h, g));
    vm.host()
        .inner
        .native("KillUnit", &[Value::Handle(h)])
        .unwrap();
    step(&mut vm, 30.0);
    assert_eq!(holy_life(&mut vm, g), 100.0);
}

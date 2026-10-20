use super::*;
use pcx_objects::editor::misc::{CombatRules, HeroVitals};
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
trigger Died=null
trigger Hit=null
endglobals
function setup takes nothing returns nothing
set A=CreateUnit(Player(4),'u001',16.0,16.0,0.0)
set B=CreateUnit(Player(5),'u001',48.0,16.0,180.0)
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
    let extra = format!(
        "{}\n{}{DECLARATIONS}",
        enumeration::TRIGGERS,
        super::activity::ORDERS
    );
    let map = if heroes.is_some() {
        MAP.replacen("'u001'", "'H001'", 1)
    } else {
        MAP.into()
    };
    let h = position_host_admitting(1000, 16, &map, &extra, fields, |h| h);
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
    h.native("SetRandomSeed", &[Value::Int(7)]).unwrap();
    h.admit_unit_vitals(HeroVitals {
        life_per_strength: 25.0,
        mana_per_intelligence: 15.0,
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
        ObjectHost::derive_hero_combat(&definition, Some((3, 3)), &invalid)
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
        assert!(ObjectHost::derive_hero_combat(&bad, Some((3, 4)), &rules()).is_err());
    }
    assert!(ObjectHost::derive_hero_combat(&definition, Some((3, 2)), &rules()).is_err());
    let ordinary = vm.host().inner.executed_unit(b).unwrap().definition.clone();
    assert_eq!(
        ObjectHost::derive_hero_combat(&ordinary, None, &invalid)
            .unwrap()
            .speed,
        1.0
    );
    assert!(
        ObjectHost::derive_hero_combat(&definition, None, &rules())
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

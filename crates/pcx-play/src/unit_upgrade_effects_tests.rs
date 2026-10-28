//! Upgrade effects on synthetic non-stock numbers: the slot values below are fixtures, not the
//! install's rows.
use super::*;
use crate::upgrade_effects::{Slot, Table, Unapplied};
use pcx_objects::editor::{FieldKind, SlkId};
use pcx_script::{HandleId, Value};

const UNIT_LIFE: &str = "100";
const UNIT_MANA: &str = "200";
const UNIT_START_MANA: &str = "50";
/// Level-1 and per-level values of the fixture upgrade `Qeff`.
const MANA_BASE: f32 = 100.0;
const MANA_STEP: f32 = 50.0;
const LIFE_BASE: f32 = 40.0;
const LIFE_STEP: f32 = 20.0;

fn code(text: &[u8; 4]) -> i32 {
    i32::from_be_bytes(*text)
}

fn slot(number: u8, effect: &[u8; 4], base: f32, modifier: f32) -> Slot {
    Slot {
        number,
        effect: *effect,
        base,
        modifier,
        code: String::new(),
    }
}

fn set(fields: &mut [([u8; 4], ObjectField)], id: &[u8; 4], value: &str) {
    let field = fields
        .iter_mut()
        .find(|(name, _)| name == id)
        .expect("fixture field");
    field.1.value = Ok(FieldValue::Cell(value.into()));
}

/// Fixture upgrades: `Qeff` (mana, life, a zero attack-dice slot), `Qpen` (effects the host does
/// not apply), `Qneg` (a life decrease that would end the pool) and `Qoth` (not in any unit list).
fn table() -> Table {
    Table::synthetic(
        &[*b"rmnx", *b"rhpx", *b"ratd", *b"rtma", *b"rmnr"],
        vec![
            (
                code(b"Qeff"),
                vec![
                    slot(1, b"rmnx", MANA_BASE, MANA_STEP),
                    slot(2, b"rhpx", LIFE_BASE, LIFE_STEP),
                    slot(3, b"ratd", 0.0, 0.0),
                ],
            ),
            (
                code(b"Qpen"),
                vec![
                    slot(1, b"ratd", 2.0, 1.0),
                    slot(2, b"rtma", 1.0, 0.0),
                    slot(3, b"rmnr", 0.0, 0.0),
                    slot(4, b"zzzz", 1.0, 0.0),
                ],
            ),
            (code(b"Qneg"), vec![slot(1, b"rhpx", -150.0, 0.0)]),
            (code(b"Qoth"), vec![slot(1, b"rmnx", 500.0, 0.0)]),
        ],
    )
}

fn open_with(table: Table, upgrades: &str) -> (Vm, HandleId, HandleId) {
    let mut fields = armed("64", "normal");
    set(&mut fields, b"H001", UNIT_LIFE);
    set(&mut fields, b"H002", UNIT_MANA);
    set(&mut fields, b"H003", UNIT_START_MANA);
    fields.push(column(
        b"U002",
        SlkId::UnitBalance,
        "upgrades",
        FieldKind::Token,
        upgrades,
    ));
    let (mut vm, a, b) = open_fixture_with(
        &fields,
        None,
        rules(),
        false,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let h = &mut vm.host().inner;
    let ctx = Arc::clone(h.player_context().unwrap());
    h.admit_upgrade_research(
        &ctx,
        crate::upgrade_research::fixture_with_effects(&ctx.sources.catalog, table),
    )
    .unwrap();
    (vm, a, b)
}

fn open() -> (Vm, HandleId, HandleId) {
    open_with(table(), "Qeff,Qpen,Qneg")
}

fn owner(vm: &mut Vm, unit: HandleId) -> HandleId {
    vm.host().inner.executed_unit(unit).unwrap().owner
}

fn research(vm: &mut Vm, player: HandleId, upgrade: &[u8; 4], level: i32) -> Result<Value, String> {
    vm.host().native(
        "SetPlayerTechResearched",
        &[
            Value::Handle(player),
            Value::Int(code(upgrade)),
            Value::Int(level),
        ],
    )
}

fn pools(vm: &mut Vm, unit: HandleId) -> [f32; 4] {
    let v = vm.host().inner.unit_vitals(unit).unwrap().unwrap();
    [v.life, v.max_life, v.mana, v.max_mana]
}

#[test]
fn ownership_inherits_research_and_updates_the_recipients_other_units() {
    let table = table().synthetic_flags(code(b"Qeff"), true, false);
    let (mut vm, a, b) = open_with(table, "Qeff");
    let old = owner(&mut vm, a);
    let new = owner(&mut vm, b);
    research(&mut vm, old, b"Qeff", 2).unwrap();
    vm.host()
        .inner
        .native(
            "SetUnitOwner",
            &[Value::Handle(a), Value::Handle(new), Value::Bool(false)],
        )
        .unwrap();
    assert_eq!(owner(&mut vm, a), new);
    assert_eq!(pools(&mut vm, a), [160.0, 160.0, 88.0, 350.0]);
    assert_eq!(pools(&mut vm, b), [160.0, 160.0, 88.0, 350.0]);
    let settings = vm.host().inner.player_settings(new).unwrap();
    assert!(settings.research_levels.contains(&(code(b"Qeff"), 2)));
    assert!(
        settings
            .tech_researched
            .iter()
            .all(|(id, _)| *id != code(b"Qeff"))
    );
    research(&mut vm, new, b"Qeff", 3).unwrap();
    assert_eq!(pools(&mut vm, b)[1], 180.0);
    assert_eq!(
        vm.host()
            .inner
            .player_settings(new)
            .unwrap()
            .research_levels
            .iter()
            .filter(|(id, _)| *id == code(b"Qeff"))
            .count(),
        1
    );
}

#[test]
fn ownership_without_inheritance_removes_old_and_applies_new_levels() {
    let (mut vm, a, b) = open_with(table(), "Qeff");
    let old = owner(&mut vm, a);
    let new = owner(&mut vm, b);
    research(&mut vm, old, b"Qeff", 2).unwrap();
    research(&mut vm, new, b"Qeff", 1).unwrap();
    vm.host()
        .inner
        .native(
            "SetUnitOwner",
            &[Value::Handle(a), Value::Handle(new), Value::Bool(false)],
        )
        .unwrap();
    assert_eq!(pools(&mut vm, a), [140.0, 140.0, 75.0, 300.0]);
    assert_eq!(pools(&mut vm, b), [140.0, 140.0, 75.0, 300.0]);
}

#[test]
fn refused_ownership_projection_leaves_research_and_units_unchanged() {
    let table = table().synthetic_flags(code(b"Qeff"), true, false);
    let (mut vm, a, b) = open_with(table, "Qeff");
    let old = owner(&mut vm, a);
    let new = owner(&mut vm, b);
    research(&mut vm, old, b"Qeff", 2).unwrap();
    let mana = vm.global("UNIT_STATE_MANA");
    vm.host()
        .inner
        .native("SetUnitState", &[Value::Handle(a), mana, Value::Real(0.0)])
        .unwrap();
    vm.host().inner.set_test_maximum(a, None, Some(1.0));
    let before = pools(&mut vm, b);
    let error = vm
        .host()
        .inner
        .native(
            "SetUnitOwner",
            &[Value::Handle(a), Value::Handle(new), Value::Bool(false)],
        )
        .unwrap_err();
    assert!(error.to_string().contains("maximum mana"));
    assert_eq!(owner(&mut vm, a), old);
    assert_eq!(pools(&mut vm, b), before);
    assert!(
        vm.host()
            .inner
            .player_settings(new)
            .unwrap()
            .research_levels
            .is_empty()
    );
}

#[test]
fn global_upgrade_refuses_before_changing_research_or_units() {
    let table = table().synthetic_flags(code(b"Qeff"), false, true);
    let (mut vm, a, _) = open_with(table, "Qeff");
    let player = owner(&mut vm, a);
    let before = pools(&mut vm, a);
    assert!(
        research(&mut vm, player, b"Qeff", 1)
            .unwrap_err()
            .contains("player-level")
    );
    assert_eq!(pools(&mut vm, a), before);
    assert!(
        vm.host()
            .inner
            .player_settings(player)
            .unwrap()
            .research_levels
            .is_empty()
    );
}

#[test]
fn research_applies_the_level_delta_to_existing_listed_units_keeping_pool_ratios() {
    let (mut vm, a, b) = open();
    let player = owner(&mut vm, a);
    // Start: 100/100 life, 50 of 200 mana (a quarter).
    assert_eq!(pools(&mut vm, a), [100.0, 100.0, 50.0, 200.0]);
    research(&mut vm, player, b"Qeff", 1).unwrap();
    assert_eq!(
        pools(&mut vm, a),
        [140.0, 140.0, 75.0, 300.0],
        "level 1 adds the base values; mana keeps its quarter"
    );
    research(&mut vm, player, b"Qeff", 3).unwrap();
    assert_eq!(
        pools(&mut vm, a),
        [180.0, 180.0, 101.0, 400.0],
        "each level rounds its own pool change"
    );
    research(&mut vm, player, b"Qeff", 0).unwrap();
    assert_eq!(pools(&mut vm, a), [100.0, 100.0, 50.0, 200.0]);
    assert_eq!(
        pools(&mut vm, b),
        [100.0, 100.0, 50.0, 200.0],
        "another player's unit is untouched"
    );
}

#[test]
fn a_level_jump_rounds_each_step_and_undoes_each_step() {
    let table = Table::synthetic(
        &[*b"rmnx"],
        vec![(code(b"Qeff"), vec![slot(1, b"rmnx", 100.0, 50.0)])],
    );
    let (mut vm, unit, _) = open_with(table, "Qeff");
    let player = owner(&mut vm, unit);
    let mana = vm.global("UNIT_STATE_MANA");
    vm.host()
        .inner
        .native(
            "SetUnitState",
            &[Value::Handle(unit), mana, Value::Real(71.0)],
        )
        .unwrap();
    research(&mut vm, player, b"Qeff", 2).unwrap();
    assert_eq!(pools(&mut vm, unit)[2..], [125.0, 350.0]);
    research(&mut vm, player, b"Qeff", 0).unwrap();
    assert_eq!(pools(&mut vm, unit)[2..], [71.0, 200.0]);
}

#[test]
fn a_zero_net_change_still_applies_intermediate_levels() {
    let table = Table::synthetic(
        &[*b"rmnx"],
        vec![(code(b"Qeff"), vec![slot(1, b"rmnx", -100.0, 100.0)])],
    );
    let (mut vm, unit, _) = open_with(table, "Qeff");
    let player = owner(&mut vm, unit);
    let mana = vm.global("UNIT_STATE_MANA");
    vm.host()
        .inner
        .native(
            "SetUnitState",
            &[Value::Handle(unit), mana, Value::Real(73.0)],
        )
        .unwrap();
    research(&mut vm, player, b"Qeff", 2).unwrap();
    assert_eq!(pools(&mut vm, unit)[2..], [74.0, 200.0]);
}

#[test]
fn mana_regeneration_upgrade_applies_to_existing_and_new_units_and_is_removed() {
    let table = Table::synthetic(
        &[*b"rmnr"],
        vec![(code(b"Qeff"), vec![slot(1, b"rmnr", 0.3, 0.4)])],
    );
    let (mut vm, unit, _) = open_with(table, "Qeff");
    let player = owner(&mut vm, unit);
    let base = vm
        .host()
        .inner
        .unit_vitals(unit)
        .unwrap()
        .unwrap()
        .regen
        .mana_per_second;
    research(&mut vm, player, b"Qeff", 2).unwrap();
    let upgraded = vm.host().inner.unit_vitals(unit).unwrap().unwrap();
    assert!((upgraded.regen.mana_per_second - base - 0.7).abs() < 0.00001);
    vm.host().inner.next_timer_expiry(3.0, &mut 1000).unwrap();
    vm.host()
        .inner
        .advance_unit_activity(3.0, &mut 65536)
        .unwrap();
    let after = vm.host().inner.unit_vitals(unit).unwrap().unwrap();
    assert!((after.mana - upgraded.mana - 3.0 * upgraded.regen.mana_per_second).abs() < 0.0001);
    let Value::Handle(created) = vm
        .host()
        .native(
            "CreateUnit",
            &[
                Value::Handle(player),
                Value::Int(code(b"u001")),
                Value::Real(80.0),
                Value::Real(16.0),
                Value::Real(0.0),
            ],
        )
        .unwrap()
    else {
        panic!("created unit")
    };
    let created_rate = vm
        .host()
        .inner
        .unit_vitals(created)
        .unwrap()
        .unwrap()
        .regen
        .mana_per_second;
    assert!((created_rate - base - 0.7).abs() < 0.00001);
    research(&mut vm, player, b"Qeff", 0).unwrap();
    for unit in [unit, created] {
        let rate = vm
            .host()
            .inner
            .unit_vitals(unit)
            .unwrap()
            .unwrap()
            .regen
            .mana_per_second;
        assert!((rate - base).abs() < 0.00001);
    }
}

#[test]
fn negative_mana_regeneration_refuses_without_changing_the_level() {
    let table = Table::synthetic(
        &[*b"rmnr"],
        vec![(code(b"Qeff"), vec![slot(1, b"rmnr", -1000.0, 0.0)])],
    );
    let (mut vm, unit, _) = open_with(table, "Qeff");
    let player = owner(&mut vm, unit);
    let before = vm.host().inner.unit_vitals(unit).unwrap();
    assert!(
        research(&mut vm, player, b"Qeff", 1)
            .unwrap_err()
            .contains("mana regeneration")
    );
    assert_eq!(vm.host().inner.unit_vitals(unit).unwrap(), before);
    assert!(
        vm.host()
            .inner
            .player_settings(player)
            .unwrap()
            .research_levels
            .iter()
            .all(|(id, _)| *id != code(b"Qeff"))
    );
}

#[test]
fn a_wounded_unit_keeps_its_life_ratio_and_a_living_floor() {
    let (mut vm, a, _) = open();
    let player = owner(&mut vm, a);
    vm.host().inner.set_test_life(a, 50.0);
    research(&mut vm, player, b"Qeff", 1).unwrap();
    assert_eq!(pools(&mut vm, a)[..2], [70.0, 140.0]);
    vm.host().inner.set_test_life(a, 1.0);
    research(&mut vm, player, b"Qeff", 2).unwrap();
    let [life, max_life, ..] = pools(&mut vm, a);
    assert_eq!(max_life, 160.0);
    assert_eq!(life, 1.0, "a living unit keeps one hit point");
}

#[test]
fn a_unit_created_afterwards_gets_the_current_levels_from_its_creation_pools() {
    let (mut vm, a, _) = open();
    let player = owner(&mut vm, a);
    research(&mut vm, player, b"Qeff", 2).unwrap();
    let Value::Handle(unit) = vm
        .host()
        .native(
            "CreateUnit",
            &[
                Value::Handle(player),
                Value::Int(code(b"u001")),
                Value::Real(80.0),
                Value::Real(16.0),
                Value::Real(0.0),
            ],
        )
        .unwrap()
    else {
        panic!("created unit");
    };
    // Level 2: mana +150, life +60 on the creation pools (50 of 200 mana, full life).
    assert_eq!(pools(&mut vm, unit), [160.0, 160.0, 88.0, 350.0]);
    assert_eq!(pools(&mut vm, a), [160.0, 160.0, 88.0, 350.0]);
}

#[test]
fn a_unit_whose_type_does_not_list_the_upgrade_is_not_changed() {
    let (mut vm, a, _) = open();
    let player = owner(&mut vm, a);
    research(&mut vm, player, b"Qoth", 1).unwrap();
    assert_eq!(pools(&mut vm, a), [100.0, 100.0, 50.0, 200.0]);
    assert!(vm.host().inner.unapplied_upgrade_effects().is_empty());
}

#[test]
fn unestablished_effects_are_recorded_with_reasons_and_cleared_at_level_zero() {
    let (mut vm, a, _) = open();
    let player = owner(&mut vm, a);
    research(&mut vm, player, b"Qpen", 2).unwrap();
    let number = 4;
    let list: Vec<(u8, [u8; 4], i32, i32)> = vm
        .host()
        .inner
        .unapplied_upgrade_effects()
        .iter()
        .map(|u: &Unapplied| (u.slot, u.effect, u.from, u.to))
        .collect();
    // The zero-delta `rmnr` slot is a no-op of a decoded additive routine and is not listed.
    assert_eq!(
        list,
        [
            (1, *b"ratd", 0, 2),
            (2, *b"rtma", 0, 2),
            (4, *b"zzzz", 0, 2)
        ]
    );
    assert!(
        vm.host()
            .inner
            .unapplied_upgrade_effects()
            .iter()
            .all(|u| u.player == number && u.upgrade == code(b"Qpen") && !u.reason.is_empty())
    );
    assert_eq!(pools(&mut vm, a), [100.0, 100.0, 50.0, 200.0]);
    research(&mut vm, player, b"Qpen", 0).unwrap();
    assert!(vm.host().inner.unapplied_upgrade_effects().is_empty());
}

#[test]
fn a_refused_projection_leaves_the_level_and_every_pool_unchanged() {
    let (mut vm, a, _) = open();
    let player = owner(&mut vm, a);
    let error = research(&mut vm, player, b"Qneg", 1).unwrap_err();
    assert!(error.contains("Qneg"), "{error}");
    assert_eq!(pools(&mut vm, a), [100.0, 100.0, 50.0, 200.0]);
    let levels = vm
        .host()
        .inner
        .player_settings(player)
        .unwrap()
        .research_levels
        .clone();
    assert!(levels.iter().all(|(id, _)| *id != code(b"Qneg")));
}

#[test]
fn a_decrease_on_a_unit_without_maximum_mana_is_skipped_as_the_original_does() {
    let mut fields = armed("64", "normal");
    set(&mut fields, b"H001", UNIT_LIFE);
    fields.push(column(
        b"U002",
        SlkId::UnitBalance,
        "upgrades",
        FieldKind::Token,
        "Qdwn",
    ));
    let table = Table::synthetic(
        &[*b"rmnx"],
        vec![(code(b"Qdwn"), vec![slot(1, b"rmnx", 30.0, 0.0)])],
    );
    let (mut vm, a, _) = open_fixture_with(
        &fields,
        None,
        rules(),
        false,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let h = &mut vm.host().inner;
    let ctx = Arc::clone(h.player_context().unwrap());
    h.admit_upgrade_research(
        &ctx,
        crate::upgrade_research::fixture_with_effects(&ctx.sources.catalog, table),
    )
    .unwrap();
    let player = owner(&mut vm, a);
    // Increase on a unit with no mana: the maximum grows, the current stays zero.
    research(&mut vm, player, b"Qdwn", 1).unwrap();
    assert_eq!(pools(&mut vm, a), [100.0, 100.0, 0.0, 30.0]);
    research(&mut vm, player, b"Qdwn", 0).unwrap();
    assert_eq!(pools(&mut vm, a), [100.0, 100.0, 0.0, 0.0]);
    // A decrease on a unit whose maximum mana is zero is skipped, not refused.
    research(&mut vm, player, b"Qdwn", 1).unwrap();
    vm.host().inner.set_test_maximum(a, None, Some(0.0));
    research(&mut vm, player, b"Qdwn", 0).unwrap();
    assert_eq!(pools(&mut vm, a), [100.0, 100.0, 0.0, 0.0]);
}

#[test]
fn an_id_with_no_upgrade_row_stores_its_level_without_effect_or_error() {
    let (mut vm, a, _) = open();
    let player = owner(&mut vm, a);
    let before = pools(&mut vm, a);
    research(&mut vm, player, b"Qzzz", 3).unwrap();
    let settings = vm.host().inner.player_settings(player).unwrap();
    assert!(settings.tech_researched.contains(&(code(b"Qzzz"), 3)));
    assert!(
        settings
            .research_levels
            .iter()
            .all(|(id, _)| *id != code(b"Qzzz"))
    );
    assert_eq!(pools(&mut vm, a), before);
}

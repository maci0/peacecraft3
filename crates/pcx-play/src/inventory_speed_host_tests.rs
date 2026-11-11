//! Held Boots of Speed (`AIms`) through the unit speed state: the bonus is added to the base
//! before the multiplier, only the largest counts, and the limits apply last. The rules under
//! test are the ones decoded in `docs/re-notes/unit-move-speed-model.md`.
use super::*;

/// The fixture `MinUnitSpeed` and `MaxUnitSpeed`.
const MIN_SPEED: f32 = 10.0;
const MAX_SPEED: f32 = 400.0;
/// `DataA1` of the `AIms` row of `bspd` and of the `AIm2` row of `bsp2`.
const BOOTS: f32 = 60.0;
const WEAKER_BOOTS: f32 = 20.0;

fn speed(h: &ObjectHost, unit: HandleId) -> f32 {
    h.executed_unit(unit)
        .unwrap()
        .activity
        .as_ref()
        .unwrap()
        .speed()
        .expect("movable unit")
}

fn limited(raw: f32) -> f32 {
    raw.clamp(MIN_SPEED, MAX_SPEED)
}

fn script_set(h: &mut ObjectHost, unit: HandleId, value: f32) {
    h.native(
        "SetUnitMoveSpeed",
        &[Value::Handle(unit), Value::Real(value)],
    )
    .unwrap();
}

fn script_get(h: &mut ObjectHost, unit: HandleId) -> f32 {
    match h
        .native("GetUnitMoveSpeed", &[Value::Handle(unit)])
        .unwrap()
    {
        Value::Real(speed) => speed,
        other => panic!("GetUnitMoveSpeed returned {other:?}"),
    }
}

fn drop_item(h: &mut ObjectHost, unit: HandleId, item: HandleId) {
    h.native(
        "UnitRemoveItem",
        &[Value::Handle(unit), Value::Handle(item)],
    )
    .unwrap();
}

#[test]
fn boots_add_their_bonus_to_the_base_and_dropping_them_takes_it_back() {
    let Fixture {
        mut h, ordinary, ..
    } = open();
    let base = speed(&h, ordinary);
    let boots = item(&mut h, b"bspd", 0.0);
    assert_eq!(add(&mut h, ordinary, boots), Ok(Value::Bool(true)));
    assert_eq!(speed(&h, ordinary), limited(base + BOOTS));
    assert_eq!(script_get(&mut h, ordinary), limited(base + BOOTS));
    drop_item(&mut h, ordinary, boots);
    assert_eq!(speed(&h, ordinary), base);
}

#[test]
fn the_bonus_is_added_before_a_slow_multiplier_not_after_it() {
    let Fixture {
        mut h, ordinary, ..
    } = open();
    let base = speed(&h, ordinary);
    let boots = item(&mut h, b"bspd", 0.0);
    assert_eq!(add(&mut h, ordinary, boots), Ok(Value::Bool(true)));
    let decrease = 0.5;
    assert!(h.shift_unit_speed(ordinary, -decrease, "slow").unwrap());
    let expected = limited((base + BOOTS) * (1.0 - decrease));
    assert_ne!(
        expected,
        limited(base * (1.0 - decrease) + BOOTS),
        "the fixture separates the two orders"
    );
    assert_eq!(speed(&h, ordinary), expected);
    // Dropping the boots under the slow leaves the slowed base; ending the slow restores it.
    drop_item(&mut h, ordinary, boots);
    assert_eq!(speed(&h, ordinary), limited(base * (1.0 - decrease)));
    assert!(h.shift_unit_speed(ordinary, decrease, "slow").unwrap());
    assert_eq!(speed(&h, ordinary), base);
}

#[test]
fn only_the_largest_of_two_held_bonuses_counts() {
    let Fixture {
        mut h, ordinary, ..
    } = open();
    let base = speed(&h, ordinary);
    let strong = item(&mut h, b"bspd", 0.0);
    let weak = item(&mut h, b"bsp2", 0.0);
    assert_eq!(add(&mut h, ordinary, weak), Ok(Value::Bool(true)));
    assert_eq!(speed(&h, ordinary), limited(base + WEAKER_BOOTS));
    assert_eq!(add(&mut h, ordinary, strong), Ok(Value::Bool(true)));
    assert_eq!(speed(&h, ordinary), limited(base + BOOTS), "not the sum");
    drop_item(&mut h, ordinary, strong);
    assert_eq!(speed(&h, ordinary), limited(base + WEAKER_BOOTS));
    drop_item(&mut h, ordinary, weak);
    assert_eq!(speed(&h, ordinary), base);
}

#[test]
fn set_unit_move_speed_writes_the_base_and_the_bonus_stays() {
    let Fixture {
        mut h, ordinary, ..
    } = open();
    let boots = item(&mut h, b"bspd", 0.0);
    assert_eq!(add(&mut h, ordinary, boots), Ok(Value::Bool(true)));
    script_set(&mut h, ordinary, 100.0);
    assert_eq!(script_get(&mut h, ordinary), 100.0 + BOOTS);
    // The upper limit applies to the sum, and the base is kept unclamped below it.
    script_set(&mut h, ordinary, 380.0);
    assert_eq!(script_get(&mut h, ordinary), MAX_SPEED);
    drop_item(&mut h, ordinary, boots);
    assert_eq!(script_get(&mut h, ordinary), 380.0);
}

#[test]
fn boots_work_for_a_hero_and_move_with_the_item_to_a_new_holder() {
    let Fixture {
        mut h,
        hero,
        ordinary,
    } = open();
    let hero_base = speed(&h, hero);
    let base = speed(&h, ordinary);
    let boots = item(&mut h, b"bspd", 0.0);
    assert_eq!(add(&mut h, hero, boots), Ok(Value::Bool(true)));
    assert_eq!(speed(&h, hero), limited(hero_base + BOOTS));
    assert_eq!(add(&mut h, ordinary, boots), Ok(Value::Bool(true)));
    assert_eq!(speed(&h, hero), hero_base, "the old holder lost it");
    assert_eq!(speed(&h, ordinary), limited(base + BOOTS));
}

fn walk(h: &mut ObjectHost, unit: HandleId) {
    h.native(
        "IssuePointOrder",
        &[
            Value::Handle(unit),
            Value::Str("gamma".into()),
            Value::Real(50.0),
            Value::Real(50.0),
        ],
    )
    .unwrap();
    assert!(
        h.executed_unit(unit)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .moving(),
        "the fixture unit walks"
    );
}

#[test]
fn stacking_rule_sets_take_the_same_largest_bonus_formula_on_an_idle_unit() {
    let Fixture {
        mut h, ordinary, ..
    } = open_stacking();
    let base = speed(&h, ordinary);
    let strong = item(&mut h, b"bspd", 0.0);
    let weak = item(&mut h, b"bsp2", 0.0);
    assert_eq!(add(&mut h, ordinary, weak), Ok(Value::Bool(true)));
    assert_eq!(add(&mut h, ordinary, strong), Ok(Value::Bool(true)));
    assert_eq!(
        script_get(&mut h, ordinary),
        limited(base + BOOTS),
        "not the sum"
    );
    drop_item(&mut h, ordinary, strong);
    assert_eq!(script_get(&mut h, ordinary), limited(base + WEAKER_BOOTS));
    drop_item(&mut h, ordinary, weak);
    assert_eq!(script_get(&mut h, ordinary), base);
}

#[test]
fn stacking_rule_sets_refuse_a_bonus_change_on_a_walking_unit() {
    let Fixture {
        mut h, ordinary, ..
    } = open_stacking();
    let base = speed(&h, ordinary);
    let boots = item(&mut h, b"bspd", 0.0);
    walk(&mut h, ordinary);
    let err = add(&mut h, ordinary, boots).unwrap_err();
    assert!(err.contains("MoveSpeedBonusesStack"), "{err}");
    assert_eq!(speed(&h, ordinary), base, "nothing was written");
    let held = h
        .executed_items()
        .any(|(id, record)| id == boots && record.held_by.is_some());
    assert!(!held);
}

#[test]
fn stacking_rule_sets_refuse_dropping_boots_on_a_walking_unit() {
    let Fixture {
        mut h, ordinary, ..
    } = open_stacking();
    let base = speed(&h, ordinary);
    let boots = item(&mut h, b"bspd", 0.0);
    assert_eq!(add(&mut h, ordinary, boots), Ok(Value::Bool(true)));
    walk(&mut h, ordinary);
    let err = h
        .native(
            "UnitRemoveItem",
            &[Value::Handle(ordinary), Value::Handle(boots)],
        )
        .unwrap_err();
    assert!(err.to_string().contains("MoveSpeedBonusesStack"), "{err}");
    assert_eq!(speed(&h, ordinary), limited(base + BOOTS));
}

#[test]
fn rule_sets_without_stacking_change_the_bonus_while_walking() {
    let Fixture {
        mut h, ordinary, ..
    } = open();
    let base = speed(&h, ordinary);
    let boots = item(&mut h, b"bspd", 0.0);
    walk(&mut h, ordinary);
    assert_eq!(add(&mut h, ordinary, boots), Ok(Value::Bool(true)));
    assert_eq!(speed(&h, ordinary), limited(base + BOOTS));
    drop_item(&mut h, ordinary, boots);
    assert_eq!(speed(&h, ordinary), base);
}

#[test]
fn a_dying_holder_that_drops_its_boots_keeps_no_bonus() {
    let Fixture {
        mut h, ordinary, ..
    } = open();
    let base = speed(&h, ordinary);
    let boots = item(&mut h, b"bspd", 0.0);
    assert_eq!(add(&mut h, ordinary, boots), Ok(Value::Bool(true)));
    h.native("KillUnit", &[Value::Handle(ordinary)]).unwrap();
    let held = h
        .executed_items()
        .any(|(id, record)| id == boots && record.held_by.is_some());
    assert!(!held, "the fixture unit drops its items on death");
    assert_eq!(speed(&h, ordinary), base);
}

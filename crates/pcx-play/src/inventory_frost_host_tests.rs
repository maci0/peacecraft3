//! Held Orb of Frost through the combat step: the speed decreases, their timers, refresh,
//! removal and the refusals. The rules under test are the ones decoded in
//! `docs/re-notes/passive-held-items.md`.
use super::*;

/// The move speed the unit currently walks at.
fn walk_speed(h: &ObjectHost, unit: HandleId) -> f32 {
    h.executed_unit(unit)
        .unwrap()
        .activity
        .as_ref()
        .unwrap()
        .speed()
        .expect("movable unit")
}

fn buff_codes(h: &ObjectHost, unit: HandleId) -> Vec<[u8; 4]> {
    h.unit_buffs(unit)
        .unwrap()
        .iter()
        .map(|buff| buff.rawcode())
        .collect()
}

/// Lowest move speed the fixture's limits allow.
const MIN_SPEED: f32 = 10.0;

fn frosted(base: f32) -> f32 {
    (base * (1.0 - FROST_MOVE)).max(MIN_SPEED)
}

#[test]
fn frost_slows_move_and_attack_speed_for_the_duration_and_never_stacks() {
    let (Fixture { mut h, hero, .. }, _, target) = armed_hero(b"ofro");
    let base = walk_speed(&h, target);
    assert!(
        frosted(base) < base,
        "the fixture speed leaves room to slow"
    );
    attack(&mut h, hero, target);
    run(&mut h, 0.3);
    assert_eq!(walk_speed(&h, target), frosted(base));
    assert_eq!(h.attack_speed_penalty(target), FROST_ATTACK);
    assert_eq!(buff_codes(&h, target), [*b"Bfro"]);
    let per_hit = attack_damage(WEAPON_ROLL, FROST_DAMAGE, BASE_ARMOR);
    assert_life(&h, target, per_hit, "frost leaves the armor alone");
    // A second hit restarts the timer; the decreases are not added twice.
    let mut second_at = None;
    while second_at.is_none() {
        let next = h.timer_game_time().unwrap() + STEP;
        run(&mut h, next);
        if hit_count(&h, target, per_hit) >= 2 {
            second_at = h.timer_game_time();
        }
    }
    let second_at = second_at.unwrap();
    assert_eq!(walk_speed(&h, target), frosted(base));
    assert_eq!(h.attack_speed_penalty(target), FROST_ATTACK);
    h.native("PauseUnit", &[Value::Handle(hero), Value::Bool(true)])
        .unwrap();
    let duration: f64 = FROST_DURATION.parse().unwrap();
    run(&mut h, second_at + duration - 0.2);
    assert_eq!(walk_speed(&h, target), frosted(base), "still slowed");
    run(&mut h, second_at + duration + 0.2);
    assert_eq!(walk_speed(&h, target), base, "speed restored at expiry");
    assert_eq!(h.attack_speed_penalty(target), 0.0);
    assert!(buff_codes(&h, target).is_empty());
}

#[test]
fn frost_lasts_the_hero_duration_on_a_hero_target() {
    let (Fixture { mut h, hero, .. }, _, _) = armed_hero(b"ofro");
    let victim_hero = unit_for(&mut h, b"H001", 5, 30.0);
    let base = walk_speed(&h, victim_hero);
    attack(&mut h, hero, victim_hero);
    run(&mut h, 0.3);
    assert_eq!(walk_speed(&h, victim_hero), frosted(base));
    let hit_at = h.timer_game_time().unwrap();
    h.native("PauseUnit", &[Value::Handle(hero), Value::Bool(true)])
        .unwrap();
    let hero_duration: f64 = FROST_HERO_DURATION.parse().unwrap();
    let ordinary: f64 = FROST_DURATION.parse().unwrap();
    assert!(hero_duration < ordinary, "the hero timer is told apart");
    run(&mut h, hit_at + hero_duration + 0.3);
    assert_eq!(walk_speed(&h, victim_hero), base, "restored after HeroDur1");
}

#[test]
fn removing_the_frost_buff_restores_the_speed() {
    let (Fixture { mut h, hero, .. }, _, target) = armed_hero(b"ofro");
    let base = walk_speed(&h, target);
    attack(&mut h, hero, target);
    run(&mut h, 0.3);
    assert_eq!(walk_speed(&h, target), frosted(base));
    h.native("PauseUnit", &[Value::Handle(hero), Value::Bool(true)])
        .unwrap();
    h.native(
        "UnitRemoveBuffs",
        &[Value::Handle(target), Value::Bool(false), Value::Bool(true)],
    )
    .unwrap();
    assert_eq!(walk_speed(&h, target), base);
    assert_eq!(h.attack_speed_penalty(target), 0.0);
    assert!(buff_codes(&h, target).is_empty());
}

fn script_speed(h: &mut ObjectHost, unit: HandleId) -> f32 {
    match h
        .native("GetUnitMoveSpeed", &[Value::Handle(unit)])
        .unwrap()
    {
        Value::Real(speed) => speed,
        other => panic!("GetUnitMoveSpeed returned {other:?}"),
    }
}

#[test]
fn the_script_speed_natives_see_the_frost_and_set_the_base_under_it() {
    let (Fixture { mut h, hero, .. }, _, target) = armed_hero(b"ofro");
    let base = walk_speed(&h, target);
    attack(&mut h, hero, target);
    run(&mut h, 0.3);
    assert_eq!(script_speed(&mut h, target), frosted(base));
    // SetUnitMoveSpeed writes the base; the frost multiplier keeps acting on it.
    h.native(
        "SetUnitMoveSpeed",
        &[Value::Handle(target), Value::Real(base + 20.0)],
    )
    .unwrap();
    assert_eq!(script_speed(&mut h, target), frosted(base + 20.0));
    h.native("PauseUnit", &[Value::Handle(hero), Value::Bool(true)])
        .unwrap();
    h.native(
        "UnitRemoveBuffs",
        &[Value::Handle(target), Value::Bool(false), Value::Bool(true)],
    )
    .unwrap();
    assert_eq!(script_speed(&mut h, target), base + 20.0);
}

#[test]
fn a_second_decrease_on_a_frosted_unit_adds_to_the_same_multiplier() {
    let (Fixture { mut h, hero, .. }, _, target) = armed_hero(b"ofro");
    let base = walk_speed(&h, target);
    attack(&mut h, hero, target);
    run(&mut h, 0.3);
    assert_eq!(walk_speed(&h, target), frosted(base));
    // What a slow adds when it lands on the same unit (`apply_slow`).
    let slow = 0.25;
    assert!(h.shift_unit_speed(target, -slow, "slow").unwrap());
    let both = (base * (1.0 - FROST_MOVE - slow)).max(MIN_SPEED);
    assert!(
        both < frosted(base),
        "the fixture speed leaves room for both"
    );
    assert_eq!(walk_speed(&h, target), both);
    h.native("PauseUnit", &[Value::Handle(hero), Value::Bool(true)])
        .unwrap();
    assert!(h.shift_unit_speed(target, slow, "slow").unwrap());
    assert_eq!(walk_speed(&h, target), frosted(base), "the frost stays");
    h.native(
        "UnitRemoveBuffs",
        &[Value::Handle(target), Value::Bool(false), Value::Bool(true)],
    )
    .unwrap();
    assert_eq!(walk_speed(&h, target), base);
}

#[test]
fn a_missile_with_frost_slows_on_impact_not_at_launch() {
    let (Fixture { mut h, hero, .. }, _, target) = armed_hero_with(b"ofro", &missile_fields());
    let base = walk_speed(&h, target);
    attack(&mut h, hero, target);
    run(&mut h, 0.3);
    assert_eq!(h.test_missile_count(), 1);
    assert_eq!(walk_speed(&h, target), base, "not before impact");
    run(&mut h, 0.7);
    assert_eq!(walk_speed(&h, target), frosted(base));
    assert_eq!(h.attack_speed_penalty(target), FROST_ATTACK);
}

/// `DataA1` of the fixture `AIsx` row.
const HELD_ATTACK_SPEED: f32 = 0.15;

/// The attack speed multiplier the combat step uses: the held and agility part minus the
/// slow and frost decreases, as one sum.
fn attack_multiplier(h: &ObjectHost, unit: HandleId) -> f32 {
    h.hero_combat_bonuses(unit).unwrap().speed - h.attack_speed_penalty(unit)
}

#[test]
fn held_attack_speed_adds_to_the_multiplier_and_leaves_with_the_item() {
    let mut f = open_orb(&[]);
    let bare = attack_multiplier(&f.h, f.hero);
    let gloves = item(&mut f.h, b"pasp", 0.0);
    assert_eq!(add(&mut f.h, f.hero, gloves), Ok(Value::Bool(true)));
    assert!((attack_multiplier(&f.h, f.hero) - (bare + HELD_ATTACK_SPEED)).abs() < 1e-6);
    let second = item(&mut f.h, b"pasp", 4.0);
    assert_eq!(add(&mut f.h, f.hero, second), Ok(Value::Bool(true)));
    assert!((attack_multiplier(&f.h, f.hero) - (bare + 2.0 * HELD_ATTACK_SPEED)).abs() < 1e-6);
    f.h.native(
        "UnitRemoveItem",
        &[Value::Handle(f.hero), Value::Handle(gloves)],
    )
    .unwrap();
    f.h.native(
        "UnitRemoveItem",
        &[Value::Handle(f.hero), Value::Handle(second)],
    )
    .unwrap();
    assert!((attack_multiplier(&f.h, f.hero) - bare).abs() < 1e-6);
}

#[test]
fn held_attack_speed_and_the_frost_decrease_sum_into_one_multiplier() {
    let (Fixture { mut h, hero, .. }, _, _) = armed_hero(b"ofro");
    let holder = unit_for(&mut h, b"H001", 5, 30.0);
    let bare = attack_multiplier(&h, holder);
    let gloves = item(&mut h, b"pasp", 30.0);
    assert_eq!(add(&mut h, holder, gloves), Ok(Value::Bool(true)));
    attack(&mut h, hero, holder);
    run(&mut h, 0.3);
    assert_eq!(h.attack_speed_penalty(holder), FROST_ATTACK);
    let expected = bare + HELD_ATTACK_SPEED - FROST_ATTACK;
    assert!((attack_multiplier(&h, holder) - expected).abs() < 1e-6);
}

#[test]
fn a_dead_frosted_unit_keeps_no_state() {
    let (Fixture { mut h, hero, .. }, _, target) = armed_hero(b"ofro");
    attack(&mut h, hero, target);
    run(&mut h, 0.3);
    assert!(h.unit_frosted(target));
    h.native("KillUnit", &[Value::Handle(target)]).unwrap();
    assert!(!h.unit_frosted(target));
    assert_eq!(h.attack_speed_penalty(target), 0.0);
}

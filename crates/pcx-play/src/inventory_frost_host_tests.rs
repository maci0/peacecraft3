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

#[test]
fn a_frosted_unit_refuses_a_speed_query_that_would_hide_the_modifier() {
    let (Fixture { mut h, hero, .. }, _, target) = armed_hero(b"ofro");
    attack(&mut h, hero, target);
    run(&mut h, 0.3);
    let err = h
        .native("GetUnitMoveSpeed", &[Value::Handle(target)])
        .unwrap_err();
    assert!(
        err.contains("active ability speed modifiers are not admitted"),
        "{err}"
    );
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

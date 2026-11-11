//! Held Orb of Venom through the combat step: the poison ticks, refresh by the same holder,
//! stacking across holders, the hero duration, kills and the refusals. The rules under test are
//! the ones decoded in `docs/re-notes/passive-held-items.md` ("`AIpb`").
use super::*;

fn dps() -> f32 {
    POISON_DPS.parse().unwrap()
}

/// One tick: the damage per second through the spells row of the damage-bonus table.
fn tick_damage() -> f32 {
    dps() * SPELLS_VS_MEDIUM
}

fn duration() -> f64 {
    POISON_DURATION.parse().unwrap()
}

fn per_hit() -> f32 {
    attack_damage(WEAPON_ROLL, VENOM_DAMAGE, BASE_ARMOR)
}

fn buff_codes(h: &ObjectHost, unit: HandleId) -> Vec<[u8; 4]> {
    h.unit_buffs(unit)
        .unwrap()
        .iter()
        .map(|buff| buff.rawcode())
        .collect()
}

/// The fixture units' `type` column: empty is organic, `mechanical` is not.
fn type_column(kind: &str) -> Vec<([u8; 4], ObjectField)> {
    vec![column(
        b"utyp",
        SlkId::UnitBalance,
        "type",
        FieldKind::Token,
        kind,
    )]
}

fn venom_hero(kind: &str) -> (Fixture, HandleId, HandleId) {
    armed_hero_with(b"oven", &type_column(kind))
}

/// Steps `h` one tick at a time until `unit` is hurt; returns the game time of that step.
fn until_hurt(h: &mut ObjectHost, unit: HandleId) -> f64 {
    while life_of(h, unit) >= 500.0 {
        let next = h.timer_game_time().unwrap() + STEP;
        run(h, next);
    }
    h.timer_game_time().unwrap()
}

/// Steps to `until` and returns the life drops of `unit`, one per step that lowered its life.
fn drops(h: &mut ObjectHost, unit: HandleId, until: f64) -> Vec<f32> {
    let mut found = Vec::new();
    while h.timer_game_time().unwrap() < until {
        let before = life_of(h, unit);
        let next = h.timer_game_time().unwrap() + STEP;
        run(h, next);
        let lost = before - life_of(h, unit);
        if lost > 1e-4 {
            found.push(lost);
        }
    }
    found
}

fn pause(h: &mut ObjectHost, unit: HandleId) {
    h.native("PauseUnit", &[Value::Handle(unit), Value::Bool(true)])
        .unwrap();
}

#[test]
fn venom_ticks_once_a_second_from_the_first_hundredth_for_the_duration_then_ends() {
    let (Fixture { mut h, hero, .. }, _, target) = venom_hero("");
    attack(&mut h, hero, target);
    let hit_at = until_hurt(&mut h, target);
    pause(&mut h, hero);
    assert_life(&h, target, per_hit(), "the hit lands before the first tick");
    assert_eq!(buff_codes(&h, target), [*b"BIpb"]);
    let tick = tick_damage();
    run(&mut h, hit_at + 0.1);
    assert_life(&h, target, per_hit() + tick, "the first tick");
    run(&mut h, hit_at + 1.0);
    assert_life(
        &h,
        target,
        per_hit() + tick,
        "the next tick is a second later",
    );
    run(&mut h, hit_at + 1.1);
    assert_life(&h, target, per_hit() + 2.0 * tick, "the second tick");
    // The step after the hit is longer than the first hundredth, so the first tick is already due.
    // Ticks fall at 0.01, 1.01, ... 5.01 after the hit; the timer ends the poison at 6.0 first.
    let rest = drops(&mut h, target, hit_at + duration() + 3.0);
    assert_eq!(rest.len(), 4, "six ticks in all");
    assert!(
        rest.iter().all(|lost| (lost - tick).abs() < 1e-3),
        "{rest:?}"
    );
    assert!(
        buff_codes(&h, target).is_empty(),
        "the buff ends with the poison"
    );
}

#[test]
fn the_same_holder_refreshes_the_timer_and_never_adds_a_second_stack() {
    let (Fixture { mut h, hero, .. }, _, target) = venom_hero("");
    attack(&mut h, hero, target);
    let first_at = until_hurt(&mut h, target);
    // The next hit is the next drop of a whole attack.
    let mut second_at = None;
    while second_at.is_none() {
        let before = life_of(&h, target);
        let next = h.timer_game_time().unwrap() + STEP;
        run(&mut h, next);
        if before - life_of(&h, target) > per_hit() - 1e-3 {
            second_at = h.timer_game_time();
        }
    }
    let second_at = second_at.unwrap();
    pause(&mut h, hero);
    assert!(
        second_at - first_at > 0.5,
        "the second hit is a later swing: {first_at} then {second_at}"
    );
    // Two hits, one stack: the damage per second stays one tick, and the poison lasts the full
    // duration from the second hit.
    run(&mut h, second_at + 2.0);
    let rest = drops(&mut h, target, second_at + 3.0);
    assert_eq!(rest.len(), 1);
    assert!((rest[0] - tick_damage()).abs() < 1e-3, "{rest:?}");
    run(&mut h, first_at + duration() + 0.2);
    assert_eq!(
        buff_codes(&h, target),
        [*b"BIpb"],
        "alive past the first timer"
    );
    run(&mut h, second_at + duration() + 0.2);
    assert!(
        buff_codes(&h, target).is_empty(),
        "ended by the second timer"
    );
}

#[test]
fn another_holder_adds_a_stack_and_each_stack_ends_on_its_own_timer() {
    let (Fixture { mut h, hero, .. }, _, target) = venom_hero("");
    let second = create_unit(&mut h, b"H001", 8.0);
    let orb = item(&mut h, b"oven", 8.0);
    assert_eq!(add(&mut h, second, orb), Ok(Value::Bool(true)));
    attack(&mut h, hero, target);
    let first_at = until_hurt(&mut h, target);
    pause(&mut h, hero);
    // The second holder hits two seconds later.
    run(&mut h, first_at + 2.0);
    let before = life_of(&h, target);
    attack(&mut h, second, target);
    while life_of(&h, target) > before - per_hit() + 1e-3 {
        let next = h.timer_game_time().unwrap() + STEP;
        run(&mut h, next);
        assert!(h.timer_game_time().unwrap() < first_at + 4.0, "second hit");
    }
    pause(&mut h, second);
    let second_at = h.timer_game_time().unwrap();
    assert_eq!(buff_codes(&h, target), [*b"BIpb"], "one buff on the unit");
    // Two stacks tick together: a second's window holds one tick of twice the damage.
    run(&mut h, second_at + 0.5);
    let both = drops(&mut h, target, second_at + 1.5);
    assert_eq!(both.len(), 1);
    assert!((both[0] - 2.0 * tick_damage()).abs() < 1e-3, "{both:?}");
    // The first stack ends at its own timer; the second keeps ticking alone.
    run(&mut h, first_at + duration() + 0.5);
    let alone = drops(&mut h, target, first_at + duration() + 1.5);
    assert_eq!(alone.len(), 1);
    assert!((alone[0] - tick_damage()).abs() < 1e-3, "{alone:?}");
    run(&mut h, second_at + duration() + 0.5);
    assert!(
        buff_codes(&h, target).is_empty(),
        "the last stack ends the poison"
    );
    let after = drops(&mut h, target, second_at + duration() + 3.0);
    assert!(after.is_empty(), "no tick after the poison ends: {after:?}");
}

#[test]
fn venom_lasts_the_hero_duration_on_a_hero_target() {
    let (Fixture { mut h, hero, .. }, _, _) = venom_hero("");
    let victim = unit_for(&mut h, b"H001", 5, 30.0);
    attack(&mut h, hero, victim);
    let hit_at = until_hurt(&mut h, victim);
    pause(&mut h, hero);
    let hero_duration: f64 = POISON_HERO_DURATION.parse().unwrap();
    assert!(hero_duration > duration(), "the hero timer is told apart");
    let ticks = drops(&mut h, victim, hit_at + hero_duration + 2.0);
    assert_eq!(
        ticks.len(),
        8,
        "eight ticks over the hero duration: {ticks:?}"
    );
    assert!(buff_codes(&h, victim).is_empty());
}

#[test]
fn the_tick_damage_can_kill() {
    let (Fixture { mut h, hero, .. }, _, target) = venom_hero("");
    let weak = per_hit() + tick_damage() / 2.0;
    set_life(&mut h, target, weak);
    attack(&mut h, hero, target);
    while life_of(&h, target) >= weak {
        let next = h.timer_game_time().unwrap() + STEP;
        run(&mut h, next);
    }
    pause(&mut h, hero);
    let alive = |h: &ObjectHost| {
        !h.executed_unit(target)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .dead
    };
    assert!(alive(&h), "the hit leaves less than a tick of life");
    let hit_at = h.timer_game_time().unwrap();
    run(&mut h, hit_at + 0.1);
    assert!(!alive(&h), "the first tick kills");
}

#[test]
fn a_mechanical_target_is_not_armed_and_takes_only_the_hit() {
    let (Fixture { mut h, hero, .. }, _, target) = venom_hero("mechanical");
    attack(&mut h, hero, target);
    let hit_at = until_hurt(&mut h, target);
    pause(&mut h, hero);
    assert!(buff_codes(&h, target).is_empty(), "no poison on a machine");
    let later = drops(&mut h, target, hit_at + 3.0);
    assert!(later.is_empty(), "{later:?}");
    assert_life(
        &h,
        target,
        per_hit(),
        "the flat bonus still applies to the hit",
    );
}

#[test]
fn a_sleeping_unit_takes_no_tick_until_the_sleep_ends() {
    use crate::playercolor_host::{ActiveBuff, BuffTraits};
    let (Fixture { mut h, hero, .. }, _, target) = venom_hero("");
    attack(&mut h, hero, target);
    let hit_at = until_hurt(&mut h, target);
    pause(&mut h, hero);
    h.attach_unit_buff(target, ActiveBuff::new(*b"BUsl", BuffTraits::default()))
        .unwrap();
    let asleep = drops(&mut h, target, hit_at + 2.5);
    assert!(asleep.is_empty(), "no tick while asleep: {asleep:?}");
    h.detach_unit_buff(target, *b"BUsl");
    let awake = drops(&mut h, target, hit_at + 4.5);
    assert!(!awake.is_empty(), "ticks resume");
}

#[test]
fn removing_the_poison_buff_ends_the_ticks() {
    let (Fixture { mut h, hero, .. }, _, target) = venom_hero("");
    attack(&mut h, hero, target);
    let hit_at = until_hurt(&mut h, target);
    pause(&mut h, hero);
    run(&mut h, hit_at + 0.1);
    h.native(
        "UnitRemoveBuffs",
        &[Value::Handle(target), Value::Bool(false), Value::Bool(true)],
    )
    .unwrap();
    assert!(buff_codes(&h, target).is_empty());
    let after = drops(&mut h, target, hit_at + 4.0);
    assert!(
        after.is_empty(),
        "no tick after the buff is removed: {after:?}"
    );
}

#[test]
fn a_missile_with_venom_poisons_on_impact_not_at_launch() {
    let mut extra = missile_fields();
    extra.extend(type_column(""));
    let (Fixture { mut h, hero, .. }, _, target) = armed_hero_with(b"oven", &extra);
    attack(&mut h, hero, target);
    run(&mut h, 0.3);
    assert_eq!(h.test_missile_count(), 1);
    assert!(buff_codes(&h, target).is_empty(), "not before impact");
    run(&mut h, 1.0);
    assert_eq!(buff_codes(&h, target), [*b"BIpb"]);
}

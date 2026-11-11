//! Held orbs through the combat step: the primary hit, the fire splash, the corruption armor
//! penalty, the second attack against air, and the admission refusals. The rules under test are
//! the ones decoded in `docs/re-notes/passive-held-items.md`.
use super::*;

/// Weapon base plus the fixture weapon's single die: `dmgplus1` 4 and `1d1`.
const WEAPON_ROLL: f32 = 5.0;
/// `damage_bonus` of a pierce weapon against the fixture units' medium defense.
const PIERCE_VS_MEDIUM: f32 = 0.75;
const ARMOR_FACTOR: f32 = 0.06;
/// `def` of the fixture units.
const BASE_ARMOR: f32 = 2.0;
/// Splash reach: `Area1` plus the fixture collision radius of the struck unit.
const COLLISION: f32 = 2.0;
const STEP: f64 = 1.0 / 64.0;

/// `Area1` of the fixture fire orb rows.
fn area() -> f32 {
    ORB_AREA.parse().unwrap()
}

fn open_orb(extra: &[([u8; 4], ObjectField)]) -> Fixture {
    open_with_regens_overlays_extra(
        Overlay::default(),
        Overlay::default(),
        0.0,
        None,
        0.0,
        "500",
        extra,
    )
}

fn unit_for(h: &mut ObjectHost, raw: &[u8; 4], player: i32, x: f32) -> HandleId {
    unit_at(h, raw, player, [x, 0.0])
}

fn position(h: &ObjectHost, unit: HandleId) -> [f32; 2] {
    h.executed_unit(unit).unwrap().position.unwrap()
}

fn unit_at(h: &mut ObjectHost, raw: &[u8; 4], player: i32, at: [f32; 2]) -> HandleId {
    let owner = h.native("Player", &[Value::Int(player)]).unwrap();
    let Value::Handle(id) = h
        .native(
            "CreateUnit",
            &[
                owner,
                Value::Int(i32::from_be_bytes(*raw)),
                Value::Real(at[0]),
                Value::Real(at[1]),
                Value::Real(0.0),
            ],
        )
        .unwrap()
    else {
        panic!("unit handle")
    };
    id
}

fn attack(h: &mut ObjectHost, unit: HandleId, target: HandleId) {
    assert_eq!(
        h.native(
            "IssueTargetOrder",
            &[
                Value::Handle(unit),
                Value::Str("alpha".into()),
                Value::Handle(target)
            ]
        ),
        Ok(Value::Bool(true))
    );
}

fn run(h: &mut ObjectHost, until: f64) {
    let mut now = h.timer_game_time().unwrap_or(0.0);
    while now < until {
        now += STEP;
        h.next_timer_expiry(now, &mut 1000).unwrap();
        h.advance_unit_activity(now, &mut 65536).unwrap();
    }
}

fn armor_scale(armor: f32) -> f32 {
    if armor >= 0.0 {
        1.0 - armor * ARMOR_FACTOR / (1.0 + armor * ARMOR_FACTOR)
    } else {
        2.0 - (1.0 - ARMOR_FACTOR).powf(-armor)
    }
}

/// One hero attack against a unit of armor `armor`, with `flat` held damage.
fn attack_damage(roll: f32, flat: f32, armor: f32) -> f32 {
    (roll + HERO_STR * PRIMARY_DAMAGE + flat) * PIERCE_VS_MEDIUM * armor_scale(armor)
}

fn assert_life(h: &ObjectHost, unit: HandleId, lost: f32, why: &str) {
    let got = 500.0 - life_of(h, unit);
    assert!(
        (got - lost).abs() < 1e-3,
        "{why}: lost {got}, expected {lost}"
    );
}

/// The hero holding `raw`, and an enemy target of the hero at x = 30, in melee reach.
fn armed_hero(raw: &[u8; 4]) -> (Fixture, HandleId, HandleId) {
    armed_hero_with(raw, &[])
}

fn armed_hero_with(
    raw: &[u8; 4],
    extra: &[([u8; 4], ObjectField)],
) -> (Fixture, HandleId, HandleId) {
    let mut f = open_orb(extra);
    let orb = item(&mut f.h, raw, 0.0);
    assert_eq!(add(&mut f.h, f.hero, orb), Ok(Value::Bool(true)));
    let target = unit_for(&mut f.h, b"u001", 5, 30.0);
    (f, orb, target)
}

#[test]
fn fire_orb_splashes_enemies_in_the_area_and_spares_the_target_allies_and_the_far() {
    let (Fixture { mut h, hero, .. }, _, target) = armed_hero(b"ofir");
    // Placement snaps units to a 4-unit grid, so the victims are laid out from the target's
    // actual spot: 36 inside, 40.2 (outside the radius, inside radius plus collision) and 44.
    let at = position(&h, target);
    let near = unit_at(&mut h, b"u001", 5, [at[0] + 36.0, at[1]]);
    let edge = unit_at(&mut h, b"u001", 5, [at[0] + 4.0, at[1] + 40.0]);
    let far = unit_at(&mut h, b"u001", 5, [at[0] + 44.0, at[1]]);
    let gap = |unit| {
        let p = position(&h, unit);
        ((p[0] - at[0]).powi(2) + (p[1] - at[1]).powi(2)).sqrt()
    };
    assert!(gap(near) < area());
    assert!(gap(edge) > area() && gap(edge) <= area() + COLLISION);
    assert!(gap(far) > area() + COLLISION);
    let before = life_of(&h, hero);
    attack(&mut h, hero, target);
    run(&mut h, 0.3);
    assert_life(
        &h,
        target,
        attack_damage(WEAPON_ROLL, ORB_DAMAGE, BASE_ARMOR),
        "the target takes the attack with the orb's flat bonus, armored once",
    );
    // Universal splash: the full amount through the spells row, no armor.
    for (unit, why) in [(near, "inside"), (edge, "inside by the collision radius")] {
        assert_life(&h, unit, ORB_DAMAGE * SPELLS_VS_MEDIUM, why);
    }
    assert_life(&h, far, 0.0, "outside the reach");
    assert_eq!(life_of(&h, hero), before, "the attacker is not an enemy");
}

#[test]
fn spell_resistance_scales_the_spells_type_splash_the_holder_takes() {
    let (Fixture { mut h, hero, .. }, _, target) = armed_hero(b"ofir");
    let at = position(&h, target);
    let bare = unit_at(&mut h, b"H001", 5, [at[0] + 36.0, at[1]]);
    let holder = unit_at(&mut h, b"H001", 5, [at[0], at[1] + 30.0]);
    let bracers = item(&mut h, b"brac", at[0] + 16.0);
    assert_eq!(add(&mut h, holder, bracers), Ok(Value::Bool(true)));
    let (bare_before, holder_before) = (life_of(&h, bare), life_of(&h, holder));
    attack(&mut h, hero, target);
    run(&mut h, 0.3);
    let full = ORB_DAMAGE * SPELLS_VS_MEDIUM;
    assert!((bare_before - life_of(&h, bare) - full).abs() < 1e-3);
    let taken = bare_before - life_of(&h, bare);
    let scaled = holder_before - life_of(&h, holder);
    let factor = 1.0 - SPELL_REDUCTION.parse::<f32>().unwrap();
    assert!(
        (scaled - taken * factor).abs() < 1e-3,
        "the holder took {scaled} of {taken}"
    );
}

#[test]
fn splash_spares_units_of_the_attackers_own_side() {
    let (
        Fixture {
            mut h,
            hero,
            ordinary,
        },
        _,
        target,
    ) = armed_hero(b"ofir");
    // `ordinary` belongs to the hero's owner; it stands inside the splash area.
    let before = life_of(&h, ordinary);
    attack(&mut h, hero, target);
    run(&mut h, 0.3);
    assert_eq!(life_of(&h, ordinary), before);
}

#[test]
fn kiljaeden_orb_splashes_its_own_amount() {
    let (Fixture { mut h, hero, .. }, _, target) = armed_hero(b"ogld");
    let near = unit_for(&mut h, b"u001", 5, 60.0);
    attack(&mut h, hero, target);
    run(&mut h, 0.3);
    assert_life(
        &h,
        target,
        attack_damage(WEAPON_ROLL, KILJAEDEN_DAMAGE, BASE_ARMOR),
        "primary hit with the row's own amount",
    );
    assert_life(&h, near, KILJAEDEN_DAMAGE * SPELLS_VS_MEDIUM, "splash");
}

#[test]
fn dropping_the_orb_ends_the_splash_and_the_bonus() {
    let (Fixture { mut h, hero, .. }, orb, target) = armed_hero(b"ofir");
    let near = unit_for(&mut h, b"u001", 5, 60.0);
    attack(&mut h, hero, target);
    run(&mut h, 0.3);
    h.native("UnitRemoveItem", &[Value::Handle(hero), Value::Handle(orb)])
        .unwrap();
    run(&mut h, 1.2);
    assert_life(
        &h,
        target,
        attack_damage(WEAPON_ROLL, ORB_DAMAGE, BASE_ARMOR)
            + attack_damage(WEAPON_ROLL, 0.0, BASE_ARMOR),
        "the second hit has neither bonus nor splash",
    );
    assert_life(&h, near, ORB_DAMAGE * SPELLS_VS_MEDIUM, "one splash only");
}

#[test]
fn structures_are_outside_the_stock_orb_class_list() {
    // Every fixture unit is a structure, which the orb row's `targs1` does not list: no splash.
    let extra = [column(
        b"B001",
        SlkId::UnitBalance,
        "isbldg",
        FieldKind::Bool,
        "1",
    )];
    let (Fixture { mut h, hero, .. }, _, target) = armed_hero_with(b"ofir", &extra);
    let near = unit_for(&mut h, b"u001", 5, 60.0);
    attack(&mut h, hero, target);
    run(&mut h, 0.3);
    assert_life(
        &h,
        target,
        attack_damage(WEAPON_ROLL, ORB_DAMAGE, BASE_ARMOR),
        "the hit lands with the flat bonus",
    );
    assert_life(&h, near, 0.0, "no splash for a structure target");
}

fn hit_count(h: &ObjectHost, target: HandleId, per_hit: f32) -> i32 {
    ((500.0 - life_of(h, target)) / per_hit).round() as i32
}

#[test]
fn corruption_lowers_armor_before_the_hit_refreshes_without_stacking_and_expires() {
    let (Fixture { mut h, hero, .. }, _, target) = armed_hero(b"ocor");
    attack(&mut h, hero, target);
    run(&mut h, 0.3);
    let lowered = BASE_ARMOR - ORB_PENALTY;
    assert_eq!(h.orb_armor_penalty(target), ORB_PENALTY);
    assert_life(
        &h,
        target,
        attack_damage(WEAPON_ROLL, ORB_DAMAGE, lowered),
        "the first hit already sees the lowered armor",
    );
    // A second hit replaces the penalty instead of stacking another.
    let per_hit = attack_damage(WEAPON_ROLL, ORB_DAMAGE, lowered);
    let mut second_at = None;
    while second_at.is_none() {
        let next = h.timer_game_time().unwrap() + STEP;
        run(&mut h, next);
        if hit_count(&h, target, per_hit) >= 2 {
            second_at = h.timer_game_time();
        }
    }
    let second_at = second_at.unwrap();
    assert_eq!(h.orb_armor_penalty(target), ORB_PENALTY);
    assert_life(
        &h,
        target,
        2.0 * per_hit,
        "armor stays lowered by 5, not 10",
    );
    // Stop the attacks; the timer restarted with the second hit.
    h.native("PauseUnit", &[Value::Handle(hero), Value::Bool(true)])
        .unwrap();
    let duration = f64::from(ORB_DURATION);
    run(&mut h, second_at + duration - 0.2);
    assert_eq!(h.orb_armor_penalty(target), ORB_PENALTY, "still lowered");
    run(&mut h, second_at + duration + 0.2);
    assert_eq!(h.orb_armor_penalty(target), 0.0, "restored at expiry");
}

#[test]
fn corruption_lasts_the_hero_duration_on_a_hero_target() {
    let (Fixture { mut h, hero, .. }, _, _) = armed_hero(b"ocor");
    let victim_hero = unit_for(&mut h, b"H001", 5, 30.0);
    attack(&mut h, hero, victim_hero);
    run(&mut h, 0.3);
    assert_eq!(h.orb_armor_penalty(victim_hero), ORB_PENALTY);
    let hit_at = h.timer_game_time().unwrap();
    h.native("PauseUnit", &[Value::Handle(hero), Value::Bool(true)])
        .unwrap();
    // Past the ordinary duration, inside the hero duration.
    let ordinary = f64::from(ORB_DURATION);
    let hero_long = f64::from(ORB_HERO_DURATION);
    run(&mut h, hit_at + ordinary + 0.5);
    assert_eq!(h.orb_armor_penalty(victim_hero), ORB_PENALTY);
    run(&mut h, hit_at + hero_long + 0.5);
    assert_eq!(h.orb_armor_penalty(victim_hero), 0.0);
}

#[test]
fn a_second_orb_on_one_holder_is_refused_without_moving_the_item() {
    let (Fixture { mut h, hero, .. }, _, _) = armed_hero(b"ofir");
    let second = item(&mut h, b"ocor", 0.0);
    let err = add(&mut h, hero, second).unwrap_err();
    assert!(err.contains("second orb"), "{err}");
    assert!(
        h.executed_items()
            .find(|(id, _)| *id == second)
            .unwrap()
            .1
            .held_by
            .is_none()
    );
    let both = item(&mut h, b"oboo", 0.0);
    assert!(add(&mut h, hero, both).is_err());
}

#[path = "inventory_frost_host_tests.rs"]
mod frost;
#[path = "inventory_speed_host_tests.rs"]
mod speed;

fn missile_fields() -> Vec<([u8; 4], ObjectField)> {
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
    vec![
        column(
            b"W003",
            SlkId::UnitWeapons,
            "weapTp1",
            FieldKind::Token,
            "missile",
        ),
        speed,
    ]
}

#[test]
fn a_missile_carries_the_orb_and_applies_it_on_impact() {
    let (Fixture { mut h, hero, .. }, _, target) = armed_hero_with(b"ofir", &missile_fields());
    let near = unit_for(&mut h, b"u001", 5, 60.0);
    attack(&mut h, hero, target);
    // Launched at 0.125, flying 30 units at 64 per second: in the air at 0.3.
    run(&mut h, 0.3);
    assert_eq!(h.test_missile_count(), 1);
    assert_life(&h, target, 0.0, "nothing lands before impact");
    assert_life(&h, near, 0.0, "no splash before impact");
    run(&mut h, 0.7);
    assert_life(
        &h,
        target,
        attack_damage(WEAPON_ROLL, ORB_DAMAGE, BASE_ARMOR),
        "impact damage",
    );
    assert_life(&h, near, ORB_DAMAGE * SPELLS_VS_MEDIUM, "splash on impact");
}

#[test]
fn a_missile_with_corruption_is_recomputed_against_the_armor_it_lowers() {
    let (Fixture { mut h, hero, .. }, _, target) = armed_hero_with(b"ocor", &missile_fields());
    attack(&mut h, hero, target);
    run(&mut h, 0.3);
    assert_eq!(h.orb_armor_penalty(target), 0.0, "not before impact");
    run(&mut h, 0.7);
    assert_eq!(h.orb_armor_penalty(target), ORB_PENALTY);
    assert_life(
        &h,
        target,
        attack_damage(WEAPON_ROLL, ORB_DAMAGE, BASE_ARMOR - ORB_PENALTY),
        "impact damage sees the lowered armor",
    );
}

/// Every fixture unit flies; weapon 1 cannot reach air, weapon 2 is a long-range 100-damage shot.
fn air_fields(second_weapon: &str) -> Vec<([u8; 4], ObjectField)> {
    use FieldKind::{Int, Real, Token};
    use SlkId::{UnitData, UnitWeapons as W};
    vec![
        column(b"M001", UnitData, "movetp", Token, "fly"),
        column(b"W101", W, "targs1", Token, "ground,structure"),
        column(b"W102", W, "targs2", Token, "ground,structure,air"),
        column(b"W103", W, "weapTp2", Token, second_weapon),
        column(b"W104", W, "atkType2", Token, "pierce"),
        column(b"W105", W, "rangeN2", Int, "640"),
        column(b"W106", W, "cool2", Real, "1.5"),
        column(b"W107", W, "dmgpt2", Real, "0.25"),
        column(b"W108", W, "dmgplus2", Int, "100"),
        column(b"W109", W, "dice2", Int, "0"),
        column(b"W110", W, "sides2", Int, "1"),
    ]
}

#[test]
fn an_orb_lets_a_melee_hero_shoot_air_targets_with_its_second_weapon() {
    let mut f = open_orb(&air_fields("normal"));
    let orb = item(&mut f.h, b"ofir", 0.0);
    assert_eq!(add(&mut f.h, f.hero, orb), Ok(Value::Bool(true)));
    let target = unit_for(&mut f.h, b"u001", 5, 100.0);
    let Fixture { mut h, hero, .. } = f;
    let hero_start = position(&h, hero)[0];
    attack(&mut h, hero, target);
    run(&mut h, 0.3);
    // 100 units is outside weapon 1 (range 64 plus radii) and inside weapon 2 (range 640).
    assert_life(
        &h,
        target,
        attack_damage(100.0, ORB_DAMAGE, BASE_ARMOR),
        "the second weapon's 100 damage from where the hero stands",
    );
    // Not a step into weapon 1's reach (64 plus both collision radii) of the target.
    let walked = position(&h, hero)[0] - hero_start;
    assert!(
        walked < 100.0 - 64.0 - 2.0 * COLLISION,
        "the hero walked {walked}"
    );
}

#[test]
fn without_an_orb_the_first_weapon_keeps_its_range_against_air() {
    let mut f = open_orb(&air_fields("normal"));
    let target = unit_for(&mut f.h, b"u001", 5, 100.0);
    let Fixture { mut h, hero, .. } = f;
    attack(&mut h, hero, target);
    run(&mut h, 0.3);
    assert_life(
        &h,
        target,
        0.0,
        "weapon 1 is out of reach, so the hero walks",
    );
    assert!(h.executed_unit(hero).unwrap().position.unwrap()[0] > 0.0);
}

#[test]
fn an_orb_on_a_unit_with_no_second_weapon_keeps_weapon_one() {
    let mut f = open_orb(&air_fields("_"));
    let orb = item(&mut f.h, b"ofir", 0.0);
    assert_eq!(add(&mut f.h, f.hero, orb), Ok(Value::Bool(true)));
    let target = unit_for(&mut f.h, b"u001", 5, 100.0);
    let Fixture { mut h, hero, .. } = f;
    attack(&mut h, hero, target);
    run(&mut h, 0.3);
    assert_life(
        &h,
        target,
        0.0,
        "no second weapon: the hero walks as before",
    );
}

#[path = "inventory_venom_host_tests.rs"]
mod venom;

// Spell orbs (`AIsb`): the proc roll at each swing and the wrapped ability on an armed hit.

/// Hero attack of the fixture weapon with the spell orb's flat damage against an ordinary unit.
fn spell_orb_hit() -> f32 {
    attack_damage(WEAPON_ROLL, KILJAEDEN_DAMAGE, BASE_ARMOR)
}

fn mark_codes(h: &ObjectHost, unit: HandleId) -> Vec<[u8; 4]> {
    h.unit_buffs(unit)
        .unwrap()
        .iter()
        .map(|buff| buff.rawcode())
        .collect()
}

fn marked(h: &ObjectHost, unit: HandleId) -> bool {
    mark_codes(h, unit).contains(b"BNba")
}

/// Rolls recorded on the fixture's spell orb stream.
fn spell_rolls(h: &ObjectHost) -> usize {
    use crate::script_random::order::CallSite;
    h.script_random_calls()
        .unwrap()
        .iter()
        .filter(|record| {
            record.site
                == CallSite::SecondaryDie {
                    channel: usize::from(SPELL_ORB_STREAM),
                }
        })
        .count()
}

/// Runs to `until`, returning the first refusal instead of unwrapping it.
fn run_to_refusal(h: &mut ObjectHost, until: f64) -> Option<String> {
    let mut now = h.timer_game_time().unwrap_or(0.0);
    while now < until {
        now += STEP;
        h.next_timer_expiry(now, &mut 1000).unwrap();
        if let Err(error) = h.advance_unit_activity(now, &mut 65536) {
            return Some(error.to_string());
        }
    }
    None
}

#[test]
fn black_arrow_orb_marks_the_target_for_its_duration_and_the_hit_keeps_the_flat_bonus() {
    let (Fixture { mut h, hero, .. }, orb, target) = armed_hero(b"odef");
    attack(&mut h, hero, target);
    run(&mut h, 0.3);
    assert_life(
        &h,
        target,
        spell_orb_hit(),
        "the hit carries the orb's flat bonus",
    );
    assert!(marked(&h, target), "{:?}", mark_codes(&h, target));
    // The orb leaves with the hero's next swing still to land: no later hit refreshes the mark.
    h.native("UnitRemoveItem", &[Value::Handle(hero), Value::Handle(orb)])
        .unwrap();
    run(&mut h, 0.3 + f64::from(ARROW_DURATION) - 1.0);
    assert!(marked(&h, target), "the mark outlasts the next swings");
    run(&mut h, 0.3 + f64::from(ARROW_DURATION) + 1.0);
    assert!(!marked(&h, target), "the mark ends with its duration");
}

#[test]
fn a_later_hit_restarts_the_mark_timer() {
    let (Fixture { mut h, hero, .. }, _, target) = armed_hero(b"odef");
    attack(&mut h, hero, target);
    run(&mut h, f64::from(ARROW_DURATION) * 2.0);
    assert!(
        marked(&h, target),
        "hits every second keep the {ARROW_DURATION} second mark alive"
    );
    assert!(
        h.executed_unit(target)
            .unwrap()
            .combat
            .as_ref()
            .is_some_and(|c| !c.dead)
    );
}

#[test]
fn an_orb_without_chance_never_marks_and_rolls_once_per_swing_on_its_own_stream() {
    let (Fixture { mut h, hero, .. }, _, target) = armed_hero(b"odez");
    let before = h.script_random_state().unwrap().clone();
    attack(&mut h, hero, target);
    run(&mut h, 4.5);
    assert!(!marked(&h, target));
    let hits = hit_count(&h, target, spell_orb_hit());
    assert!(hits >= 3, "{hits} hits");
    let rolls = spell_rolls(&h) as i32;
    assert!(
        rolls == hits || rolls == hits + 1,
        "{rolls} rolls for {hits} hits"
    );
    let (old, new) = (
        before.secondary_words().collect::<Vec<_>>(),
        h.script_random_state()
            .unwrap()
            .secondary_words()
            .collect::<Vec<_>>(),
    );
    let stream = usize::from(SPELL_ORB_STREAM);
    for lane in 0..old.len() {
        assert_eq!(old[lane] != new[lane], lane == stream, "lane {lane}");
    }
}

#[test]
fn heroes_roll_the_hero_chance_and_are_not_marked_with_a_zero_hero_duration() {
    let (Fixture { mut h, hero, .. }, _, _) = armed_hero(b"odef");
    let enemy = unit_for(&mut h, b"H001", 5, 30.0);
    attack(&mut h, hero, enemy);
    run(&mut h, 0.3);
    assert!(spell_rolls(&h) >= 1);
    assert!(!marked(&h, enemy), "HeroDur1 of the row is zero");
}

#[test]
fn a_marked_unit_cannot_die_until_the_summon_is_decoded() {
    let (Fixture { mut h, hero, .. }, _, target) = armed_hero(b"odef");
    attack(&mut h, hero, target);
    run(&mut h, 0.3);
    assert!(marked(&h, target));
    let error = h.native("KillUnit", &[Value::Handle(target)]).unwrap_err();
    assert!(error.contains("marked by Black Arrow"), "{error}");
    assert!(error.contains("ndr1"), "{error}");
    assert!(
        h.executed_unit(target)
            .unwrap()
            .combat
            .as_ref()
            .is_some_and(|combat| !combat.dead),
        "the refused death changes nothing"
    );
    // An unmarked unit still dies.
    let other = unit_for(&mut h, b"u001", 5, 60.0);
    h.native("KillUnit", &[Value::Handle(other)]).unwrap();
}

#[test]
fn a_purge_proc_refuses_at_the_hit_with_the_wrapped_class() {
    let (Fixture { mut h, hero, .. }, _, target) = armed_hero(b"oli2");
    attack(&mut h, hero, target);
    let refusal = run_to_refusal(&mut h, 1.0).expect("the proc reaches the undecoded cast");
    assert!(
        refusal.contains("AIpg") && refusal.contains("Aprg"),
        "{refusal}"
    );
    assert!(refusal.contains("not decoded"), "{refusal}");
    assert_life(&h, target, 0.0, "the refused hit lands no damage");
}

#[test]
fn a_swing_at_a_target_outside_the_orb_classes_rolls_nothing() {
    let extra = [column(
        b"B001",
        SlkId::UnitBalance,
        "isbldg",
        FieldKind::Bool,
        "1",
    )];
    let (Fixture { mut h, hero, .. }, _, target) = armed_hero_with(b"odef", &extra);
    attack(&mut h, hero, target);
    run(&mut h, 0.3);
    assert_eq!(spell_rolls(&h), 0);
    assert!(!marked(&h, target));
}

#[test]
fn a_hero_without_the_orb_rolls_nothing() {
    let Fixture { mut h, hero, .. } = open_orb(&[]);
    let target = unit_for(&mut h, b"u001", 5, 30.0);
    attack(&mut h, hero, target);
    run(&mut h, 0.3);
    assert_eq!(spell_rolls(&h), 0);
    assert!(!marked(&h, target));
}

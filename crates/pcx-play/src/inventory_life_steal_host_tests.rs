//! Held vampiric attacks (`AIva`) through the combat step: the holder gains a share of the damage
//! of each attack hit, capped by the life the target still has. The rules under test are the
//! ones decoded in `docs/re-notes/passive-held-items.md`.
use super::*;

/// `DataA1` of the fixture `AIva` row.
fn fraction() -> f32 {
    LIFE_STEAL_FRACTION.parse().unwrap()
}

/// What one hit of the fixture hero deals to the fixture target.
fn dealt() -> f32 {
    attack_damage(WEAPON_ROLL, 0.0, BASE_ARMOR)
}

/// The holder, hurt to `life`, and an enemy target in melee reach.
fn hurt_holder(life: f32) -> (Fixture, HandleId, HandleId) {
    let (mut f, item, target) = armed_hero(b"mskd");
    set_life(&mut f.h, f.hero, life);
    (f, item, target)
}

#[test]
fn a_hit_heals_the_holder_by_the_fraction_of_the_damage() {
    let (Fixture { mut h, hero, .. }, _, target) = hurt_holder(100.0);
    attack(&mut h, hero, target);
    run(&mut h, 0.3);
    assert_life(&h, target, dealt(), "the hit is the ordinary armored hit");
    let gained = life_of(&h, hero) - 100.0;
    assert!(
        (gained - dealt() * fraction()).abs() < 1e-3,
        "gained {gained}"
    );
}

#[test]
fn the_heal_is_capped_by_the_life_the_target_has_left() {
    let (Fixture { mut h, hero, .. }, _, target) = hurt_holder(100.0);
    set_life(&mut h, target, 3.0);
    assert!(dealt() > 3.0);
    attack(&mut h, hero, target);
    run(&mut h, 0.3);
    let gained = life_of(&h, hero) - 100.0;
    assert!(
        (gained - 3.0 * fraction()).abs() < 1e-3,
        "gained {gained}, expected the share of the 3 life left"
    );
}

#[test]
fn the_heal_stops_at_maximum_life() {
    let (Fixture { mut h, hero, .. }, _, target) = hurt_holder(100.0);
    let maximum = max_life_of(&h, hero);
    set_life(&mut h, hero, maximum - 0.25);
    attack(&mut h, hero, target);
    run(&mut h, 0.3);
    assert_eq!(life_of(&h, hero), maximum);
}

#[test]
fn a_structure_is_outside_the_row_classes_and_gives_nothing() {
    let extra = [column(
        b"B001",
        SlkId::UnitBalance,
        "isbldg",
        FieldKind::Bool,
        "1",
    )];
    let (mut f, _, target) = armed_hero_with(b"mskd", &extra);
    set_life(&mut f.h, f.hero, 100.0);
    attack(&mut f.h, f.hero, target);
    run(&mut f.h, 0.3);
    assert_life(&f.h, target, dealt(), "the hit lands");
    assert_eq!(life_of(&f.h, f.hero), 100.0);
}

#[test]
fn a_missile_heals_on_impact_not_at_launch() {
    let (mut f, _, target) = armed_hero_with(b"mskd", &missile_fields());
    set_life(&mut f.h, f.hero, 100.0);
    attack(&mut f.h, f.hero, target);
    run(&mut f.h, 0.3);
    assert_eq!(f.h.test_missile_count(), 1);
    assert_eq!(life_of(&f.h, f.hero), 100.0, "nothing before impact");
    run(&mut f.h, 0.7);
    let gained = life_of(&f.h, f.hero) - 100.0;
    assert!(
        (gained - dealt() * fraction()).abs() < 1e-3,
        "gained {gained}"
    );
}

#[test]
fn dropping_the_item_ends_the_life_steal() {
    let (Fixture { mut h, hero, .. }, item, target) = hurt_holder(100.0);
    attack(&mut h, hero, target);
    run(&mut h, 0.3);
    h.native(
        "UnitRemoveItem",
        &[Value::Handle(hero), Value::Handle(item)],
    )
    .unwrap();
    let after_first = life_of(&h, hero);
    run(&mut h, 1.2);
    assert_life(&h, target, 2.0 * dealt(), "both hits landed");
    assert_eq!(life_of(&h, hero), after_first, "only the first hit healed");
}

#[test]
fn a_second_life_steal_or_an_orb_beside_one_refuses_the_pickup() {
    let Fixture { mut h, hero, .. } = open_orb(&[]);
    let twin = item(&mut h, b"msk2", 0.0);
    let err = add(&mut h, hero, twin).unwrap_err();
    assert!(err.contains("two life-steal"), "{err}");
    let mixed = item(&mut h, b"mskb", 0.0);
    let err = add(&mut h, hero, mixed).unwrap_err();
    assert!(err.contains("vampiric attack"), "{err}");
    let first = item(&mut h, b"mskd", 0.0);
    assert_eq!(add(&mut h, hero, first), Ok(Value::Bool(true)));
    let orb = item(&mut h, b"ofir", 0.0);
    let err = add(&mut h, hero, orb).unwrap_err();
    assert!(err.contains("vampiric attack"), "{err}");
    let again = item(&mut h, b"mskd", 0.0);
    let err = add(&mut h, hero, again).unwrap_err();
    assert!(err.contains("vampiric attack"), "{err}");
}

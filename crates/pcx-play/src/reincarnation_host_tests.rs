//! A held Ankh of Reincarnation through the death path: the charge, the delay, the restored
//! life and mana, the refusals. The rules under test are the ones decoded in
//! `docs/re-notes/passive-held-items.md`, "`AIrc`".
use super::*;

/// `DataA1` of the fixture ankh row.
const DELAY: f64 = 3.0;
/// `DataB1` of the fixture ankh row; below the fixture hero's life.
const RESTORED_LIFE: f32 = 200.0;

fn holds(h: &mut ObjectHost, hero: HandleId, item: HandleId) -> bool {
    h.native("UnitHasItem", &[Value::Handle(hero), Value::Handle(item)]) == Ok(Value::Bool(true))
}

fn dead(h: &ObjectHost, unit: HandleId) -> bool {
    h.executed_unit(unit)
        .unwrap()
        .combat
        .as_ref()
        .is_some_and(|combat| combat.dead)
}

/// A hero holding an ankh, with the walkability query answered as walkable.
fn ankh_hero() -> (Fixture, HandleId) {
    let mut f = open_orb(&[]);
    f.h.test_set_unwalkable(false);
    let ankh = item(&mut f.h, b"ankh", 0.0);
    assert_eq!(add(&mut f.h, f.hero, ankh), Ok(Value::Bool(true)));
    (f, ankh)
}

#[test]
fn the_ankh_brings_its_holder_back_after_the_delay_with_the_row_life_and_the_mana_it_died_with() {
    let (Fixture { mut h, hero, .. }, ankh) = ankh_hero();
    let mana = mana_of(&h, hero);
    assert!(mana > 0.0);
    let place = position(&h, hero);
    h.native("KillUnit", &[Value::Handle(hero)]).unwrap();
    let died = h.timer_game_time().unwrap_or(0.0);
    assert!(dead(&h, hero));
    assert!(h.reincarnation_pending(hero));
    assert_eq!(life_of(&h, hero), 0.0);
    assert!(
        !h.executed_items().any(|(item, _)| item == ankh),
        "the perishable item is spent at the death"
    );
    run(&mut h, died + DELAY - 0.2);
    assert!(dead(&h, hero), "the unit stays dead through the delay");
    run(&mut h, died + DELAY + 0.2);
    assert!(!dead(&h, hero));
    assert!(!h.reincarnation_pending(hero));
    assert_eq!(life_of(&h, hero), RESTORED_LIFE);
    assert_eq!(mana_of(&h, hero), mana, "DataC1 of -1 keeps the mana");
    assert_eq!(position(&h, hero), place, "it returns where it died");
}

#[test]
fn an_unwalkable_position_stops_the_handler_and_keeps_the_ankh() {
    let (Fixture { mut h, hero, .. }, ankh) = ankh_hero();
    h.test_set_unwalkable(true);
    h.native("KillUnit", &[Value::Handle(hero)]).unwrap();
    assert!(dead(&h, hero));
    assert!(!h.reincarnation_pending(hero));
    assert!(
        h.executed_items().any(|(item, _)| item == ankh),
        "no charge was spent"
    );
    run(&mut h, DELAY + 1.0);
    assert!(dead(&h, hero));
}

#[test]
fn a_spent_ankh_fires_once() {
    let (Fixture { mut h, hero, .. }, _) = ankh_hero();
    h.native("KillUnit", &[Value::Handle(hero)]).unwrap();
    let died = h.timer_game_time().unwrap_or(0.0);
    run(&mut h, died + DELAY + 0.2);
    assert!(!dead(&h, hero));
    h.native("KillUnit", &[Value::Handle(hero)]).unwrap();
    assert!(dead(&h, hero));
    assert!(!h.reincarnation_pending(hero));
    let later = h.timer_game_time().unwrap();
    run(&mut h, later + DELAY + 1.0);
    assert!(dead(&h, hero));
}

#[test]
fn removing_the_unit_forgets_the_record() {
    let (Fixture { mut h, hero, .. }, _) = ankh_hero();
    h.native("KillUnit", &[Value::Handle(hero)]).unwrap();
    assert!(h.reincarnation_pending(hero));
    h.native("RemoveUnit", &[Value::Handle(hero)]).unwrap();
    assert!(!h.reincarnation_pending(hero));
    run(&mut h, DELAY + 1.0);
}

#[test]
fn two_ankhs_refuse_before_the_death_changes_anything() {
    let (Fixture { mut h, hero, .. }, _) = ankh_hero();
    let second = item(&mut h, b"ankh", 0.0);
    assert_eq!(add(&mut h, hero, second), Ok(Value::Bool(true)));
    let error = h.native("KillUnit", &[Value::Handle(hero)]).unwrap_err();
    assert!(error.contains("several ankhs"), "{error}");
    assert!(!dead(&h, hero), "the refused death changes nothing");
    assert!(holds(&mut h, hero, second));
}

#[test]
fn a_hero_reincarnation_ability_beside_the_ankh_refuses_the_death() {
    let extra = [(*b"uabi", ability_field("abilList", "L000,AOr0"))];
    let mut f = open_orb(&extra);
    f.h.test_set_unwalkable(false);
    let ankh = item(&mut f.h, b"ankh", 0.0);
    assert_eq!(add(&mut f.h, f.hero, ankh), Ok(Value::Bool(true)));
    let error =
        f.h.native("KillUnit", &[Value::Handle(f.hero)])
            .unwrap_err();
    assert!(
        error.contains("AOr0") && error.contains("hero handler"),
        "{error}"
    );
    assert!(!dead(&f.h, f.hero));
    assert!(holds(&mut f.h, f.hero, ankh));
}

#[test]
fn a_flying_holder_refuses_the_death() {
    let extra = [column(
        b"M001",
        SlkId::UnitData,
        "movetp",
        FieldKind::Token,
        "fly",
    )];
    let mut f = open_orb(&extra);
    f.h.test_set_unwalkable(false);
    let ankh = item(&mut f.h, b"ankh", 0.0);
    assert_eq!(add(&mut f.h, f.hero, ankh), Ok(Value::Bool(true)));
    let error =
        f.h.native("KillUnit", &[Value::Handle(f.hero)])
            .unwrap_err();
    assert!(error.contains("flying unit"), "{error}");
    assert!(!dead(&f.h, f.hero));
}

#[test]
fn without_the_walkability_query_the_death_refuses() {
    let mut f = open_orb(&[]);
    let ankh = item(&mut f.h, b"ankh", 0.0);
    assert_eq!(add(&mut f.h, f.hero, ankh), Ok(Value::Bool(true)));
    let error =
        f.h.native("KillUnit", &[Value::Handle(f.hero)])
            .unwrap_err();
    assert!(error.contains("terrain pathing is not admitted"), "{error}");
    assert!(!dead(&f.h, f.hero));
}

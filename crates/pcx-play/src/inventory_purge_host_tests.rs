//! Purge through the combat step: the dispel, the slow and its steps, the summoned-unit and
//! pause refusals, for the lightning orb and the spell orb. The rules under test are the ones
//! decoded in `docs/re-notes/passive-held-items.md`, "Purge".
use super::*;
use crate::inventory_powerup::{Orb, OrbKind};
use crate::playercolor_host::{ActiveBuff, BuffTraits};

/// `DataA1` of the fixture lightning orb row (`AIlb`): its flat damage.
const LIGHTNING_DAMAGE: f32 = 6.0;
/// Lowest move speed the fixture's limits allow (the fully slowed unit walks at the floor).
const FLOOR: f32 = 10.0;
/// Margin around an expected step or expiry time, larger than the fixture's step.
const MARGIN: f64 = 0.1;

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

fn purged(h: &ObjectHost, unit: HandleId) -> bool {
    buff_codes(h, unit).contains(b"Bprg")
}

/// Runs until the target carries `Bprg`, returning the game time of the cast to one step, and
/// stops the hero so no later hit recasts.
fn until_purged(h: &mut ObjectHost, hero: HandleId, target: HandleId) -> f64 {
    let mut now = h.timer_game_time().unwrap_or(0.0);
    while !purged(h, target) {
        assert!(now < 5.0, "the hit never purged the target");
        now += STEP;
        run(h, now);
    }
    h.native("PauseUnit", &[Value::Handle(hero), Value::Bool(true)])
        .unwrap();
    now
}

fn lightning_purge(h: &ObjectHost, orb: HandleId) -> crate::inventory_powerup::Purge {
    let Some(Orb {
        kind: OrbKind::Lightning(purge),
        ..
    }) = h.held_attributes(orb).unwrap().orb
    else {
        panic!("a lightning orb")
    };
    purge
}

fn magic_buff(raw: &[u8; 4], positive: bool) -> ActiveBuff {
    ActiveBuff::new(
        *raw,
        BuffTraits {
            positive,
            negative: !positive,
            magic: true,
            magic_decoded: true,
            ..BuffTraits::default()
        },
    )
}

#[test]
fn the_lightning_orb_stops_the_target_and_gives_its_speed_back_in_steps() {
    let (Fixture { mut h, hero, .. }, _, target) = armed_hero(b"olig");
    let base = walk_speed(&h, target);
    assert!(base > 2.0 * FLOOR, "the fixture speed leaves room to slow");
    attack(&mut h, hero, target);
    let cast = until_purged(&mut h, hero, target);
    assert_eq!(walk_speed(&h, target), FLOOR, "the whole speed is removed");
    // DataA1 2 over Dur1 3: a step every 1.5 s, each giving half back.
    run(&mut h, cast + 1.5 - MARGIN);
    assert_eq!(walk_speed(&h, target), FLOOR);
    run(&mut h, cast + 1.5 + MARGIN);
    let half = walk_speed(&h, target);
    assert!(
        (half - base * 0.5).abs() < 0.5,
        "{half} is not half of {base}"
    );
    run(&mut h, cast + 3.0 + MARGIN);
    assert_eq!(walk_speed(&h, target), base, "restored at the end");
    assert!(!purged(&h, target));
}

#[test]
fn a_hero_gets_the_hero_duration_and_its_shorter_steps() {
    let (Fixture { mut h, hero, .. }, _, _) = armed_hero(b"olig");
    let victim = unit_for(&mut h, b"H001", 5, 30.0);
    let base = walk_speed(&h, victim);
    attack(&mut h, hero, victim);
    let cast = until_purged(&mut h, hero, victim);
    assert_eq!(walk_speed(&h, victim), FLOOR);
    // HeroDur1 0.3 over two steps: one every 0.15 s.
    run(&mut h, cast + 0.15 + 0.05);
    let half = walk_speed(&h, victim);
    assert!(
        (half - base * 0.5).abs() < 0.5,
        "{half} is not half of {base}"
    );
    run(&mut h, cast + 0.3 + MARGIN);
    assert_eq!(walk_speed(&h, victim), base);
    assert!(!purged(&h, victim));
}

#[test]
fn every_armed_hit_casts_and_replaces_the_buff_without_stacking() {
    let (Fixture { mut h, hero, .. }, _, target) = armed_hero(b"olig");
    let base = walk_speed(&h, target);
    attack(&mut h, hero, target);
    let per_hit = attack_damage(WEAPON_ROLL, LIGHTNING_DAMAGE, BASE_ARMOR);
    // Without a pause the hero keeps hitting every second, inside the 1.5 s step.
    run(&mut h, 3.5);
    let hits = hit_count(&h, target, per_hit);
    assert!(hits >= 3, "{hits} hits");
    assert_eq!(buff_codes(&h, target), [*b"Bprg"]);
    assert_eq!(walk_speed(&h, target), FLOOR, "each hit slows afresh");
    assert_life(
        &h,
        target,
        per_hit * hits as f32,
        "an ordinary target takes the hit only, no purge damage",
    );
    h.native("PauseUnit", &[Value::Handle(hero), Value::Bool(true)])
        .unwrap();
    run(&mut h, 3.5 + 3.2);
    assert_eq!(walk_speed(&h, target), base);
}

#[test]
fn the_cast_dispels_magic_buffs_of_both_polarities_and_spares_the_rest() {
    let (Fixture { mut h, hero, .. }, _, target) = armed_hero(b"olig");
    let plain = |raw: &[u8; 4], traits: BuffTraits| ActiveBuff::new(*raw, traits);
    h.attach_unit_buff(target, magic_buff(b"Bpos", true))
        .unwrap();
    h.attach_unit_buff(target, magic_buff(b"Bneg", false))
        .unwrap();
    // Decoded as not magic, timed life or an aura, and neither polarity: all stay.
    let decoded = BuffTraits {
        magic_decoded: true,
        ..BuffTraits::default()
    };
    h.attach_unit_buff(
        target,
        plain(
            b"Bnom",
            BuffTraits {
                positive: true,
                ..decoded
            },
        ),
    )
    .unwrap();
    h.attach_unit_buff(
        target,
        plain(
            b"Btim",
            BuffTraits {
                positive: true,
                magic: true,
                timed_life: true,
                ..decoded
            },
        ),
    )
    .unwrap();
    h.attach_unit_buff(
        target,
        plain(
            b"Baur",
            BuffTraits {
                positive: true,
                magic: true,
                aura: true,
                ..decoded
            },
        ),
    )
    .unwrap();
    h.attach_unit_buff(
        target,
        plain(
            b"Bnon",
            BuffTraits {
                magic: true,
                ..decoded
            },
        ),
    )
    .unwrap();
    attack(&mut h, hero, target);
    until_purged(&mut h, hero, target);
    let mut left = buff_codes(&h, target);
    left.sort();
    assert_eq!(left, [*b"Baur", *b"Bnom", *b"Bnon", *b"Bprg", *b"Btim"]);
}

#[test]
fn a_buff_whose_magic_bit_nobody_read_refuses_the_cast_before_any_change() {
    let (Fixture { mut h, hero, .. }, _, target) = armed_hero(b"olig");
    h.attach_unit_buff(
        target,
        ActiveBuff::new(
            *b"Bunk",
            BuffTraits {
                negative: true,
                ..BuffTraits::default()
            },
        ),
    )
    .unwrap();
    attack(&mut h, hero, target);
    let refusal = run_to_refusal(&mut h, 1.0).expect("the dispel meets the unread buff");
    assert!(
        refusal.contains("Bunk") && refusal.contains("magic bit"),
        "{refusal}"
    );
    assert_eq!(buff_codes(&h, target), [*b"Bunk"], "nothing was dispelled");
    assert!(!purged(&h, target));
}

#[test]
fn an_allied_target_is_dispelled_and_not_slowed() {
    let (Fixture { mut h, hero, .. }, orb, _) = armed_hero(b"olig");
    // The fixture hero belongs to player 4.
    let ally = unit_for(&mut h, b"u001", 4, 60.0);
    let base = walk_speed(&h, ally);
    h.attach_unit_buff(ally, magic_buff(b"Bneg", false))
        .unwrap();
    let purge = lightning_purge(&h, orb);
    h.test_cast_purge(&purge, hero, ally).unwrap();
    assert!(buff_codes(&h, ally).is_empty(), "the negative buff is gone");
    assert_eq!(walk_speed(&h, ally), base);
    assert!(!h.unit_purge_slowed(ally));
}

#[test]
fn a_summoned_enemy_refuses_while_the_magic_damage_path_is_not_decoded() {
    let (Fixture { mut h, hero, .. }, orb, target) = armed_hero(b"olig");
    h.test_mark_summoned(target);
    let purge = lightning_purge(&h, orb);
    let error = h
        .test_cast_purge(&purge, hero, target)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("summoned enemy") && error.contains("0x6677c0"),
        "{error}"
    );
    assert!(!purged(&h, target), "the refused cast changes nothing");
}

#[test]
fn removing_the_buff_or_killing_the_target_gives_the_speed_back() {
    let (Fixture { mut h, hero, .. }, _, target) = armed_hero(b"olig");
    let base = walk_speed(&h, target);
    attack(&mut h, hero, target);
    until_purged(&mut h, hero, target);
    h.native(
        "UnitRemoveBuffs",
        &[Value::Handle(target), Value::Bool(false), Value::Bool(true)],
    )
    .unwrap();
    assert_eq!(walk_speed(&h, target), base);
    assert!(buff_codes(&h, target).is_empty());
    assert!(!h.unit_purge_slowed(target));
    // A second target: its death ends the slow too.
    let other = unit_for(&mut h, b"u001", 5, 60.0);
    let orb = h.executed_items().next().map(|(item, _)| item).unwrap();
    let purge = lightning_purge(&h, orb);
    h.test_cast_purge(&purge, hero, other).unwrap();
    assert!(h.unit_purge_slowed(other));
    h.native("KillUnit", &[Value::Handle(other)]).unwrap();
    assert!(!h.unit_purge_slowed(other));
    assert!(buff_codes(&h, other).is_empty());
}

#[test]
fn the_spell_orb_purges_on_a_proc_and_a_pause_row_refuses() {
    let (Fixture { mut h, hero, .. }, _, target) = armed_hero(b"oli3");
    let base = walk_speed(&h, target);
    assert!(base > 2.0 * FLOOR);
    attack(&mut h, hero, target);
    let cast = until_purged(&mut h, hero, target);
    assert_eq!(walk_speed(&h, target), FLOOR);
    // DataA1 5 over Dur1 15: a step every 3 s, each giving a fifth back. One step leaves the
    // multiplier at 0.2, under the floor; two leave 0.4.
    run(&mut h, cast + 3.0 + MARGIN);
    assert_eq!(walk_speed(&h, target), FLOOR);
    run(&mut h, cast + 6.0 + MARGIN);
    let two = walk_speed(&h, target);
    assert!(
        (two - base * 0.4).abs() < 0.5,
        "{two} after two steps of {base}"
    );
    run(&mut h, cast + 15.0 + MARGIN);
    assert_eq!(walk_speed(&h, target), base);
    // The shipped row also pauses the target (DataD, DataE): not hosted.
    let (Fixture { mut h, hero, .. }, _, target) = armed_hero(b"oli2");
    attack(&mut h, hero, target);
    let refusal = run_to_refusal(&mut h, 1.0).expect("the proc reaches the pause");
    assert!(
        refusal.contains("pauses") && refusal.contains("DataD"),
        "{refusal}"
    );
    assert!(!purged(&h, target));
    assert_life(&h, target, 0.0, "the refused hit lands no damage");
}

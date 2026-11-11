//! Held aura items through the combat step: the holder and the units around it, the 2.0 s timer
//! and 0.5 s scan, the 4.0 s buff, and the stop of the source. The rules under test are the ones
//! decoded in `docs/re-notes/passive-held-items.md`, "Held auras".
use super::*;

const STEP: f64 = 1.0 / 64.0;
/// Timings of the synthetic image (the reviewed image holds the same integers).
const TICK: f64 = 2.0;
const SCAN: f64 = 0.5;
const BUFF_LIFE: f64 = 4.0;
/// `Area1` of the fixture aura rows.
const AREA: f32 = 50.0;
/// Inside `AREA` of the holder at x = 0.
const NEAR: f32 = 30.0;
/// `DataA1`, `DataB1` of the fixture unholy rows `AIau` and the weaker `AIa2`.
const SPEED: f32 = 0.1;
const REGEN: f32 = 0.5;
const WEAK_SPEED: f32 = 0.05;
const WEAK_REGEN: f32 = 0.25;
/// `DataA1` of the fixture devotion rows `AIad` and the weaker `AIa3`.
const ARMOR: f32 = 1.0;
const WEAK_ARMOR: f32 = 0.5;
/// Outside `AREA` and its collision reach, inside the fixture map.
const FAR: f32 = AREA + 40.0;
const MIN_SPEED: f32 = 10.0;
const MAX_SPEED: f32 = 400.0;

fn run(h: &mut ObjectHost, until: f64) {
    let mut now = h.timer_game_time().unwrap_or(0.0);
    while now < until {
        now += STEP;
        h.next_timer_expiry(now, &mut 1000).unwrap();
        h.advance_unit_activity(now, &mut 65536).unwrap();
    }
}

fn at(h: &mut ObjectHost, raw: &[u8; 4], player: i32, x: f32) -> HandleId {
    let owner = h.native("Player", &[Value::Int(player)]).unwrap();
    let Value::Handle(id) = h
        .native(
            "CreateUnit",
            &[
                owner,
                Value::Int(i32::from_be_bytes(*raw)),
                Value::Real(x),
                Value::Real(0.0),
                Value::Real(0.0),
            ],
        )
        .unwrap()
    else {
        panic!("unit handle")
    };
    id
}

fn teleport(h: &mut ObjectHost, unit: HandleId, x: f32) {
    h.native(
        "SetUnitPosition",
        &[Value::Handle(unit), Value::Real(x), Value::Real(0.0)],
    )
    .unwrap();
}

fn speed(h: &ObjectHost, unit: HandleId) -> f32 {
    h.executed_unit(unit)
        .unwrap()
        .activity
        .as_ref()
        .unwrap()
        .speed()
        .expect("movable unit")
}

fn buffs(h: &ObjectHost, unit: HandleId) -> Vec<[u8; 4]> {
    h.unit_buffs(unit)
        .unwrap()
        .iter()
        .map(|buff| buff.rawcode())
        .collect()
}

fn limited(raw: f32) -> f32 {
    raw.clamp(MIN_SPEED, MAX_SPEED)
}

fn drop_item(h: &mut ObjectHost, unit: HandleId, item: HandleId) {
    h.native(
        "UnitRemoveItem",
        &[Value::Handle(unit), Value::Handle(item)],
    )
    .unwrap();
}

fn kill(h: &mut ObjectHost, unit: HandleId) {
    h.native("KillUnit", &[Value::Handle(unit)]).unwrap();
}

/// The hero holding `raw` from the clock's current time.
fn holder(raw: &[u8; 4]) -> (Fixture, HandleId) {
    let mut f = open();
    let aura = item(&mut f.h, raw, 0.0);
    assert_eq!(add(&mut f.h, f.hero, aura), Ok(Value::Bool(true)));
    (f, aura)
}

#[test]
fn the_holder_gets_the_buff_at_pickup_and_the_speed_adds_to_the_multiplier() {
    let mut f = open();
    let base = speed(&f.h, f.hero);
    let aura = item(&mut f.h, b"ugdh", 0.0);
    assert_eq!(add(&mut f.h, f.hero, aura), Ok(Value::Bool(true)));
    assert_eq!(buffs(&f.h, f.hero), [*b"BUau"]);
    assert_eq!(speed(&f.h, f.hero), limited(base * (1.0 + SPEED)));
}

#[test]
fn a_unit_entering_range_is_buffed_by_the_first_scan_not_before() {
    let (mut f, _) = holder(b"ugdh");
    let ally = at(&mut f.h, b"u001", 4, NEAR);
    let base = speed(&f.h, ally);
    let start = f.h.timer_game_time().unwrap();
    run(&mut f.h, start + SCAN - 0.1);
    assert!(buffs(&f.h, ally).is_empty(), "the first scan is 0.5 s out");
    run(&mut f.h, start + SCAN + 0.1);
    assert_eq!(buffs(&f.h, ally), [*b"BUau"]);
    assert_eq!(speed(&f.h, ally), limited(base * (1.0 + SPEED)));
}

#[test]
fn enemies_and_units_out_of_range_are_not_buffed() {
    let (mut f, _) = holder(b"ugdh");
    let enemy = at(&mut f.h, b"u001", 5, NEAR);
    let far = at(&mut f.h, b"u001", 4, FAR);
    let start = f.h.timer_game_time().unwrap();
    run(&mut f.h, start + 3.0);
    assert!(buffs(&f.h, enemy).is_empty());
    assert!(buffs(&f.h, far).is_empty());
}

#[test]
fn a_unit_that_stays_in_range_keeps_the_buff_through_the_timer_refresh() {
    let (mut f, _) = holder(b"ugdh");
    let ally = at(&mut f.h, b"u001", 4, NEAR);
    let base = speed(&f.h, ally);
    let start = f.h.timer_game_time().unwrap();
    // Far past one 4.0 s life: only the stay reports every TICK keep the buff.
    for second in 1..=12 {
        run(&mut f.h, start + f64::from(second));
        if second >= 1 {
            assert_eq!(buffs(&f.h, ally), [*b"BUau"], "at {second} s");
            assert_eq!(speed(&f.h, ally), limited(base * (1.0 + SPEED)), "{second}");
        }
    }
}

#[test]
fn leaving_reports_nothing_and_the_buff_ends_on_its_own_timer() {
    let (mut f, _) = holder(b"ugdh");
    let ally = at(&mut f.h, b"u001", 4, NEAR);
    let base = speed(&f.h, ally);
    let start = f.h.timer_game_time().unwrap();
    run(&mut f.h, start + 5.0);
    assert_eq!(buffs(&f.h, ally), [*b"BUau"]);
    let left = f.h.timer_game_time().unwrap();
    teleport(&mut f.h, ally, FAR);
    // The last application was at most one TICK before, so the buff outlives the departure by at
    // least BUFF_LIFE - TICK - one scan, and ends BUFF_LIFE after the last application.
    run(&mut f.h, left + BUFF_LIFE - TICK - SCAN);
    assert_eq!(buffs(&f.h, ally), [*b"BUau"], "no leave event removes it");
    run(&mut f.h, left + BUFF_LIFE + 0.1);
    assert!(buffs(&f.h, ally).is_empty());
    assert_eq!(speed(&f.h, ally), base, "the speed amount is taken back");
}

#[test]
fn a_unit_that_leaves_and_returns_before_expiry_is_reported_again_as_new() {
    let (mut f, _) = holder(b"ugdh");
    let ally = at(&mut f.h, b"u001", 4, NEAR);
    let start = f.h.timer_game_time().unwrap();
    run(&mut f.h, start + 1.0);
    teleport(&mut f.h, ally, FAR);
    run(&mut f.h, start + 1.6);
    teleport(&mut f.h, ally, NEAR);
    // Inside again: the next scan reports it at once, a full life from then on.
    run(&mut f.h, start + 2.2);
    run(&mut f.h, start + 2.2 + BUFF_LIFE - SCAN);
    assert_eq!(buffs(&f.h, ally), [*b"BUau"]);
}

#[test]
fn the_holder_buff_is_refreshed_by_the_two_second_timer() {
    let (mut f, _) = holder(b"ugdh");
    let start = f.h.timer_game_time().unwrap();
    run(&mut f.h, start + 9.0);
    assert_eq!(buffs(&f.h, f.hero), [*b"BUau"]);
}

#[test]
fn dropping_the_item_stops_the_source_and_the_buffs_run_out() {
    let (mut f, aura) = holder(b"ugdh");
    let ally = at(&mut f.h, b"u001", 4, NEAR);
    let base = speed(&f.h, ally);
    let start = f.h.timer_game_time().unwrap();
    run(&mut f.h, start + 3.0);
    assert_eq!(buffs(&f.h, ally), [*b"BUau"]);
    let dropped = f.h.timer_game_time().unwrap();
    drop_item(&mut f.h, f.hero, aura);
    // Removing the item removes no buff.
    assert_eq!(buffs(&f.h, ally), [*b"BUau"]);
    assert_eq!(buffs(&f.h, f.hero), [*b"BUau"]);
    run(&mut f.h, dropped + BUFF_LIFE + 0.1);
    assert!(buffs(&f.h, ally).is_empty(), "nothing refreshes it");
    assert!(buffs(&f.h, f.hero).is_empty());
    assert_eq!(speed(&f.h, ally), base);
    run(&mut f.h, dropped + 12.0);
    assert!(buffs(&f.h, ally).is_empty(), "the source is gone");
}

#[test]
fn a_dead_holder_bestows_nothing_and_its_own_buff_ends() {
    let (mut f, _) = holder(b"ugdh");
    let ally = at(&mut f.h, b"u001", 4, NEAR);
    let start = f.h.timer_game_time().unwrap();
    run(&mut f.h, start + 3.0);
    assert_eq!(buffs(&f.h, ally), [*b"BUau"]);
    let died = f.h.timer_game_time().unwrap();
    kill(&mut f.h, f.hero);
    assert!(
        buffs(&f.h, f.hero).is_empty(),
        "the dead unit keeps no buff"
    );
    run(&mut f.h, died + BUFF_LIFE + 0.1);
    assert!(buffs(&f.h, ally).is_empty(), "no report applies it again");
    run(&mut f.h, died + 10.0);
    assert!(buffs(&f.h, ally).is_empty());
}

#[test]
fn a_dying_buffed_unit_loses_the_buff_and_its_speed_amount() {
    let (mut f, _) = holder(b"ugdh");
    let ally = at(&mut f.h, b"u001", 4, NEAR);
    let start = f.h.timer_game_time().unwrap();
    run(&mut f.h, start + 1.0);
    assert_eq!(buffs(&f.h, ally), [*b"BUau"]);
    kill(&mut f.h, ally);
    assert!(buffs(&f.h, ally).is_empty());
    assert_eq!(f.h.aura_armor_bonus(ally), 0.0);
}

#[test]
fn two_unholy_sources_share_one_buff_and_the_speed_counts_once() {
    let mut f = open();
    let base = speed(&f.h, f.hero);
    let first = item(&mut f.h, b"ugdh", 0.0);
    let second = item(&mut f.h, b"ugdh", 0.0);
    assert_eq!(add(&mut f.h, f.hero, first), Ok(Value::Bool(true)));
    assert_eq!(add(&mut f.h, f.hero, second), Ok(Value::Bool(true)));
    assert_eq!(buffs(&f.h, f.hero), [*b"BUau"], "one buff per id");
    assert_eq!(speed(&f.h, f.hero), limited(base * (1.0 + SPEED)));
    let start = f.h.timer_game_time().unwrap();
    run(&mut f.h, start + 5.0);
    assert_eq!(buffs(&f.h, f.hero), [*b"BUau"]);
    assert_eq!(speed(&f.h, f.hero), limited(base * (1.0 + SPEED)));
    assert_eq!(f.h.aura_life_regen(f.hero), f64::from(REGEN));
}

#[test]
fn a_weaker_source_does_not_lower_a_stronger_buff_and_a_stronger_one_raises_it() {
    let mut f = open();
    let base = speed(&f.h, f.hero);
    let strong = item(&mut f.h, b"ugdh", 0.0);
    let weak = item(&mut f.h, b"ugd2", 0.0);
    assert_eq!(add(&mut f.h, f.hero, strong), Ok(Value::Bool(true)));
    assert_eq!(add(&mut f.h, f.hero, weak), Ok(Value::Bool(true)));
    let start = f.h.timer_game_time().unwrap();
    run(&mut f.h, start + 3.0);
    assert_eq!(speed(&f.h, f.hero), limited(base * (1.0 + SPEED)));
    assert_eq!(f.h.aura_life_regen(f.hero), f64::from(REGEN));
    // Weaker first: the stronger source's refresh raises the stored values.
    let mut g = open();
    let base = speed(&g.h, g.hero);
    let weak = item(&mut g.h, b"ugd2", 0.0);
    let strong = item(&mut g.h, b"ugdh", 0.0);
    assert_eq!(add(&mut g.h, g.hero, weak), Ok(Value::Bool(true)));
    assert_eq!(speed(&g.h, g.hero), limited(base * (1.0 + WEAK_SPEED)));
    assert_eq!(g.h.aura_life_regen(g.hero), f64::from(WEAK_REGEN));
    assert_eq!(add(&mut g.h, g.hero, strong), Ok(Value::Bool(true)));
    assert_eq!(speed(&g.h, g.hero), limited(base * (1.0 + SPEED)));
    assert_eq!(g.h.aura_life_regen(g.hero), f64::from(REGEN));
    assert_eq!(buffs(&g.h, g.hero), [*b"BUau"]);
}

#[test]
fn the_unholy_regeneration_is_a_permanent_extra_life_rate() {
    let (mut f, _) = holder(b"ugdh");
    let hero = f.hero;
    let start = f.h.timer_game_time().unwrap();
    set_life(&mut f.h, hero, 100.0);
    run(&mut f.h, start + 3.0);
    let gained = life_of(&f.h, hero) - 100.0;
    assert!(
        (gained - REGEN * 3.0).abs() < 0.05,
        "{gained} life in three seconds at {REGEN} per second"
    );
}

#[test]
fn devotion_adds_armor_once_for_two_sources_and_takes_it_back_at_expiry() {
    let mut f = open();
    let first = item(&mut f.h, b"ublb", 0.0);
    let second = item(&mut f.h, b"ubl2", 0.0);
    assert_eq!(add(&mut f.h, f.hero, second), Ok(Value::Bool(true)));
    assert_eq!(f.h.aura_armor_bonus(f.hero), WEAK_ARMOR);
    assert_eq!(add(&mut f.h, f.hero, first), Ok(Value::Bool(true)));
    assert_eq!(f.h.aura_armor_bonus(f.hero), ARMOR, "raised, not summed");
    assert_eq!(buffs(&f.h, f.hero), [*b"BHad"]);
    let ally = at(&mut f.h, b"u001", 4, NEAR);
    let start = f.h.timer_game_time().unwrap();
    run(&mut f.h, start + 1.0);
    assert_eq!(f.h.aura_armor_bonus(ally), ARMOR);
    drop_item(&mut f.h, f.hero, first);
    drop_item(&mut f.h, f.hero, second);
    let dropped = f.h.timer_game_time().unwrap();
    assert_eq!(
        f.h.aura_armor_bonus(ally),
        ARMOR,
        "the buff outlives the item"
    );
    run(&mut f.h, dropped + BUFF_LIFE + 0.1);
    assert_eq!(f.h.aura_armor_bonus(ally), 0.0);
}

#[test]
fn the_aura_armor_reaches_the_damage_armor_of_a_buffed_unit() {
    let (mut f, _) = holder(b"ublb");
    let ally = at(&mut f.h, b"u001", 4, NEAR);
    let (plain, _) = f.h.victim_armor(ally).unwrap();
    let start = f.h.timer_game_time().unwrap();
    run(&mut f.h, start + 1.0);
    let (buffed, _) = f.h.victim_armor(ally).unwrap();
    assert_eq!(buffed - plain, ARMOR);
}

#[test]
fn unholy_and_devotion_are_separate_buffs() {
    let mut f = open();
    let unholy = item(&mut f.h, b"ugdh", 0.0);
    let devotion = item(&mut f.h, b"ublb", 0.0);
    assert_eq!(add(&mut f.h, f.hero, unholy), Ok(Value::Bool(true)));
    assert_eq!(add(&mut f.h, f.hero, devotion), Ok(Value::Bool(true)));
    assert_eq!(buffs(&f.h, f.hero), [*b"BUau", *b"BHad"]);
}

#[test]
fn removing_the_buff_by_script_ends_its_effect() {
    let mut f = open();
    let base = speed(&f.h, f.hero);
    let aura = item(&mut f.h, b"ugdh", 0.0);
    assert_eq!(add(&mut f.h, f.hero, aura), Ok(Value::Bool(true)));
    f.h.native(
        "UnitRemoveBuffs",
        &[Value::Handle(f.hero), Value::Bool(true), Value::Bool(true)],
    )
    .unwrap();
    assert!(buffs(&f.h, f.hero).is_empty());
    assert_eq!(speed(&f.h, f.hero), base);
}

#[test]
fn a_percent_mode_row_refuses_the_pickup() {
    let mut f = open();
    for raw in [b"upc1", b"upc2"] {
        let row = item(&mut f.h, raw, 0.0);
        let error = add(&mut f.h, f.hero, row).unwrap_err();
        assert!(error.contains("percent mode"), "{error}");
    }
    assert!(buffs(&f.h, f.hero).is_empty());
}

#[test]
fn two_aura_abilities_on_one_item_refuse_the_pickup() {
    let mut f = open();
    let both = item(&mut f.h, b"u2au", 0.0);
    let error = add(&mut f.h, f.hero, both).unwrap_err();
    assert!(error.contains("two aura abilities"), "{error}");
}

/// `DataA1` of the fixture brilliance row `AIba`.
const MANA_REGEN: f32 = 0.75;

#[test]
fn brilliance_is_a_permanent_extra_mana_rate_on_the_holder_and_an_ally_in_range() {
    let (mut f, aura) = holder(b"umna");
    let hero = f.hero;
    let ally = at(&mut f.h, b"u001", 4, NEAR);
    assert_eq!(buffs(&f.h, hero), [*b"BHab"]);
    assert_eq!(f.h.aura_mana_regen(hero), f64::from(MANA_REGEN));
    let start = f.h.timer_game_time().unwrap();
    set_mana(&mut f.h, hero, 10.0);
    run(&mut f.h, start + 3.0);
    let gained = mana_of(&f.h, hero) - 10.0;
    assert!(
        (gained - MANA_REGEN * 3.0).abs() < 0.05,
        "{gained} mana in three seconds at {MANA_REGEN} per second"
    );
    assert_eq!(f.h.aura_mana_regen(ally), f64::from(MANA_REGEN));
    drop_item(&mut f.h, hero, aura);
    let dropped = f.h.timer_game_time().unwrap();
    run(&mut f.h, dropped + BUFF_LIFE + 0.1);
    assert_eq!(f.h.aura_mana_regen(hero), 0.0);
    assert_eq!(f.h.aura_mana_regen(ally), 0.0);
}

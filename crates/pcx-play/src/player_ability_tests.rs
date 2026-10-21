use super::{COMMON, budget, context, id, object, slots_rule};
use crate::map_admission::ParsedMapContext;
use crate::playercolor_host::ObjectHost;
use pcx_script::{Host, Value};
use std::sync::Arc;

const NATIVE: &str = "SetPlayerAbilityAvailable";

fn common() -> String {
    format!(
        "{COMMON}\nnative {NATIVE} takes player whichPlayer, integer abilid, boolean avail returns nothing\n"
    )
}

fn setup(capacity: usize) -> (ObjectHost, Arc<ParsedMapContext>) {
    let ctx = context(
        301,
        &common(),
        "// no globals",
        "synthetic:ability-availability",
    );
    let h = object(&ctx, 32)
        .with_players(ctx.clone(), slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_player_abilities(capacity)
        .unwrap();
    (h, ctx)
}

fn player(h: &mut ObjectHost, number: i32) -> Value {
    h.native("Player", &[Value::Int(number)]).unwrap()
}

fn set(h: &mut ObjectHost, player: &Value, abilid: i32, available: bool) -> Result<Value, String> {
    h.native(
        NATIVE,
        &[player.clone(), Value::Int(abilid), Value::Bool(available)],
    )
}

#[test]
fn default_availability_is_true_and_overrides_record_per_player() {
    let (mut h, _ctx) = setup(16);
    let p0 = player(&mut h, 0);
    let p1 = player(&mut h, 1);
    let abilid = i32::from_be_bytes(*b"AHhb");
    assert!(h.player_ability_available(id(&p0), abilid).unwrap());
    set(&mut h, &p0, abilid, false).unwrap();
    assert!(!h.player_ability_available(id(&p0), abilid).unwrap());
    assert!(h.player_ability_available(id(&p1), abilid).unwrap());
    set(&mut h, &p0, abilid, true).unwrap();
    assert!(h.player_ability_available(id(&p0), abilid).unwrap());
    let other = abilid ^ 0x1000;
    assert!(h.player_ability_available(id(&p0), other).unwrap());
    set(&mut h, &p0, other, false).unwrap();
    assert!(h.player_ability_available(id(&p0), abilid).unwrap());
    assert!(!h.player_ability_available(id(&p0), other).unwrap());
}

#[test]
fn availability_is_isolated_across_players() {
    let (mut h, _ctx) = setup(16);
    let abilid = 0x1000abcd;
    for number in 0..6i32 {
        let p = player(&mut h, number);
        set(&mut h, &p, abilid, number % 2 == 0).unwrap();
    }
    for number in 0..6i32 {
        let p = player(&mut h, number);
        assert_eq!(
            h.player_ability_available(id(&p), abilid).unwrap(),
            number % 2 == 0,
            "player {number}"
        );
    }
    let p8 = player(&mut h, 8);
    assert!(h.player_ability_available(id(&p8), abilid).unwrap());
}

#[test]
fn invalid_inputs_are_refused_without_mutating_state() {
    let (mut h, _ctx) = setup(16);
    let p0 = player(&mut h, 0);
    let timer = h.native("CreateTimer", &[]).unwrap();
    let abilid = 12345;
    assert!(set(&mut h, &timer, abilid, false).is_err());
    assert!(h.native(NATIVE, &[p0.clone(), Value::Int(abilid)]).is_err());
    assert!(
        h.native(NATIVE, &[p0.clone(), Value::Bool(true), Value::Bool(false)])
            .is_err()
    );
    assert!(
        h.native(
            NATIVE,
            &[Value::Null, Value::Int(abilid), Value::Bool(false)]
        )
        .is_err()
    );
    assert!(h.player_ability_available(id(&p0), abilid).unwrap());
    set(&mut h, &p0, abilid, false).unwrap();
    assert!(!h.player_ability_available(id(&p0), abilid).unwrap());
}

#[test]
fn record_budget_refusal_is_atomic_and_replacement_does_not_allocate() {
    let (mut h, _ctx) = setup(1);
    let p0 = player(&mut h, 0);
    let p1 = player(&mut h, 1);
    let a = 10;
    let b = 11;
    set(&mut h, &p0, a, false).unwrap();
    assert!(set(&mut h, &p1, b, false).is_err());
    assert!(h.player_ability_available(id(&p1), b).unwrap());
    set(&mut h, &p0, a, true).unwrap();
    assert!(h.player_ability_available(id(&p0), a).unwrap());
}

#[test]
fn campaign_style_startup_runs_ability_setter_without_refusal() {
    let map = "globals\nplayer P=Player(0)\nendglobals\nfunction config takes nothing returns nothing\ncall SetPlayerAbilityAvailable(P, 65, false)\nendfunction\nfunction main takes nothing returns nothing\ncall SetPlayerAbilityAvailable(P, 66, true)\nendfunction\n";
    let ctx = context(301, &common(), map, "synthetic:ability-startup");
    let h = object(&ctx, 32)
        .with_players(ctx, slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_player_abilities(16)
        .unwrap();
    let mut vm = h.into_vm();
    vm.init_globals().unwrap();
    vm.call("config", &[]).unwrap();
    vm.call("main", &[]).unwrap();
    let p = vm.host().native("Player", &[Value::Int(0)]).unwrap();
    assert!(!vm.host().player_ability_available(id(&p), 65).unwrap());
    assert!(vm.host().player_ability_available(id(&p), 66).unwrap());
}

//! Attack-ground orders on synthetic, nonstock weapon rows: every radius, factor, range and
//! speed below is authored in the fixture columns, none is a stock value.
use super::*;
use FieldKind::{Int, Real, Token};
use SlkId::{UnitBalance as B, UnitWeapons as W};

const AREAS: [(&str, &str); 5] = [
    ("Farea1", "4"),
    ("Harea1", "12"),
    ("Qarea1", "20"),
    ("Hfact1", "0.6"),
    ("Qfact1", "0.2"),
];

fn fields(range: &str, weapon: &str, extra: &[(&str, &str)]) -> Vec<([u8; 4], ObjectField)> {
    let mut fields = armed(range, weapon);
    fields.retain(|(id, _)| id != b"H001" && id != b"W010");
    fields.push(column(b"H001", B, "HP", Int, "4000"));
    fields.push(column(b"Z001", B, "spd", Real, "20"));
    fields.push(column(b"Z002", B, "isbldg", FieldKind::Bool, "0"));
    fields.push(column(b"W010", W, "sides1", Int, "6"));
    for (i, (name, value)) in AREAS.iter().chain(extra).enumerate() {
        let id = [b'S', b'0', b'0' + (i / 10) as u8, b'0' + (i % 10) as u8];
        let kind = if matches!(*name, "splashTargs1") {
            Token
        } else {
            Real
        };
        fields.push(column(&id, W, name, kind, value));
    }
    if !extra.iter().any(|(name, _)| *name == "splashTargs1") {
        fields.push(column(
            b"S090",
            W,
            "splashTargs1",
            Token,
            "ground,structure",
        ));
    }
    fields
}

fn with_missile(
    mut fields: Vec<([u8; 4], ObjectField)>,
    speed: &str,
) -> Vec<([u8; 4], ObjectField)> {
    let mut column = column(b"M002", SlkId::UnitData, "Missilespeed", Int, speed);
    column.1.metadata.as_mut().unwrap().source = FieldSource::Profile {
        key: "missilespeed".into(),
        index: 0,
        slots: 1,
    };
    fields.push(column);
    fields
}

fn spawn(vm: &mut Vm, owner_of: HandleId, x: f32, y: f32) -> HandleId {
    let h = &mut vm.host().inner;
    let owner = h.executed_unit(owner_of).unwrap().owner;
    let Value::Handle(unit) = h
        .native(
            "CreateUnit",
            &[
                Value::Handle(owner),
                Value::Int(i32::from_be_bytes(*b"u001")),
                Value::Real(x),
                Value::Real(y),
                Value::Real(0.0),
            ],
        )
        .unwrap()
    else {
        panic!("unit")
    };
    unit
}

fn order(vm: &mut Vm, unit: HandleId, order: &str, x: f32, y: f32) -> Result<Value, String> {
    vm.host()
        .inner
        .native(
            "IssuePointOrder",
            &[
                Value::Handle(unit),
                Value::Str(order.into()),
                Value::Real(x),
                Value::Real(y),
            ],
        )
        .map_err(|e| e.to_string())
}

fn lost(vm: &mut Vm, unit: HandleId) -> f32 {
    4000.0 - life(vm, unit)
}

#[test]
fn splash_rings_scale_one_roll_by_the_authored_factors() {
    let (mut vm, a, b) = open(&fields("100", "normal", &[]));
    let enemy = |vm: &mut Vm, x, y| spawn(vm, b, x, y);
    let (full, half, quarter, outside) = (
        enemy(&mut vm, 90.0, 60.0),
        enemy(&mut vm, 98.0, 60.0),
        enemy(&mut vm, 90.0, 76.0),
        enemy(&mut vm, 116.0, 60.0),
    );
    assert_eq!(
        order(&mut vm, a, "attackground", 90.0, 60.0),
        Ok(Value::Bool(true))
    );
    let start = vm.host().inner.timer_game_time().unwrap();
    step(&mut vm, start + 0.5);
    let base = lost(&mut vm, full);
    assert!(base > 0.0);
    assert!((lost(&mut vm, half) - base * 0.6).abs() < 1e-3);
    assert!((lost(&mut vm, quarter) - base * 0.2).abs() < 1e-3);
    assert_eq!(lost(&mut vm, outside), 0.0);
    assert_eq!(
        lost(&mut vm, b),
        0.0,
        "the original enemy is outside every ring"
    );
    // The shooter repeats every cooldown (1.5 s): a second shot lands later.
    step(&mut vm, start + 2.0);
    assert!(lost(&mut vm, full) > base);
}

#[test]
fn a_missile_weapon_lands_after_distance_over_speed() {
    let (mut vm, a, b) = open(&with_missile(fields("100", "missile", &[]), "8"));
    let victim = spawn(&mut vm, b, 90.0, 60.0);
    order(&mut vm, a, "attackground", 90.0, 60.0).unwrap();
    let start = vm.host().inner.timer_game_time().unwrap();
    step(&mut vm, start + 0.5);
    assert_eq!(lost(&mut vm, victim), 0.0, "still in flight");
    step(&mut vm, start + 14.0);
    assert!(lost(&mut vm, victim) > 0.0);
}

#[test]
fn the_caster_walks_into_range_and_a_new_order_cancels_the_shot() {
    let (mut vm, a, b) = open(&fields("20", "normal", &[]));
    let victim = spawn(&mut vm, b, 90.0, 60.0);
    order(&mut vm, a, "attackground", 90.0, 60.0).unwrap();
    let start = vm.host().inner.timer_game_time().unwrap();
    step(&mut vm, start + 0.2);
    assert_eq!(lost(&mut vm, victim), 0.0, "out of range, still walking");
    step(&mut vm, start + 30.0);
    let position = vm.host().inner.executed_unit(a).unwrap().position.unwrap();
    let gap = ((position[0] - 90.0).powi(2) + (position[1] - 60.0).powi(2)).sqrt();
    assert!(gap <= 20.0 + 16.0, "stopped {gap} from the point");
    assert!(lost(&mut vm, victim) > 0.0);
    // A move order replaces it: no further damage.
    order(&mut vm, a, "gamma", 16.0, 16.0).unwrap();
    assert_eq!(
        vm.host()
            .inner
            .executed_unit(a)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .ground_target,
        None
    );
    let before = lost(&mut vm, victim);
    step(&mut vm, start + 40.0);
    assert_eq!(lost(&mut vm, victim), before);
}

#[test]
fn a_row_without_an_enemy_token_hits_allies_and_an_enemy_token_spares_them() {
    for (targets, ally_hit) in [("ground", true), ("ground,enemy", false)] {
        let (mut vm, a, b) = open(&fields("100", "normal", &[("splashTargs1", targets)]));
        let ally = spawn(&mut vm, a, 90.0, 60.0);
        let enemy = spawn(&mut vm, b, 98.0, 60.0);
        order(&mut vm, a, "attackground", 90.0, 60.0).unwrap();
        let start = vm.host().inner.timer_game_time().unwrap();
        step(&mut vm, start + 0.5);
        assert_eq!(lost(&mut vm, ally) > 0.0, ally_hit, "{targets}");
        assert!(lost(&mut vm, enemy) > 0.0, "{targets}");
    }
}

#[test]
fn a_notself_token_spares_the_shooter_standing_in_the_area() {
    for (targets, shooter_hit) in [("ground", true), ("ground,notself", false)] {
        let (mut vm, a, _) = open(&fields("100", "normal", &[("splashTargs1", targets)]));
        let position = vm.host().inner.executed_unit(a).unwrap().position.unwrap();
        order(&mut vm, a, "attackground", position[0], position[1]).unwrap();
        let start = vm.host().inner.timer_game_time().unwrap();
        step(&mut vm, start + 0.5);
        assert_eq!(lost(&mut vm, a) > 0.0, shooter_hit, "{targets}");
    }
}

#[test]
fn unestablished_weapon_rows_refuse_the_order_or_the_shot() {
    let mut empty = fields("100", "normal", &[]);
    empty.retain(|(id, _)| id != b"S000");
    empty.push(column(b"S000", W, "Farea1", Real, "-"));
    let (mut vm, a, _) = open(&empty);
    let error = order(&mut vm, a, "attackground", 90.0, 60.0).unwrap_err();
    assert!(error.contains("splash columns"), "{error}");
    let (mut vm, a, _) = open(&fields(
        "100",
        "normal",
        &[("splashTargs1", "ground,enemies")],
    ));
    let error = order(&mut vm, a, "attackground", 90.0, 60.0).unwrap_err();
    assert!(error.contains("no established meaning"), "{error}");
    let (mut vm, a, _) = open(&fields("100", "normal", &[("minRange", "90")]));
    order(&mut vm, a, "attackground", 90.0, 60.0).unwrap();
    let start = vm.host().inner.timer_game_time().unwrap();
    let error = step_refused(&mut vm, start + 1.0);
    assert!(error.contains("minRange 90"), "{error}");
}

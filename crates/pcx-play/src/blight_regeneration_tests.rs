use super::*;
use crate::terrain_blight::{
    InitialBlight,
    tests::{identity, terrain},
};

fn fixture() -> (Vm, HandleId) {
    fixture_with_clock(false)
}

fn fixture_with_clock(clock: bool) -> (Vm, HandleId) {
    let flags: Vec<_> = (0..21).map(|i| if i % 7 == 2 { 0x20 } else { 0 }).collect();
    let blight =
        InitialBlight::read(&terrain([7, 3], [0.0; 2], &flags), 16.0, identity(), 21).unwrap();
    let (mut vm, a, b) = open_fixture(
        &regen_fields("0.25", "blight", "0.5"),
        None,
        rules(),
        clock,
        Some(blight),
    );
    vm.host()
        .inner
        .native("KillUnit", &[Value::Handle(b)])
        .unwrap();
    set_pools(&mut vm, a, 5.0, 0.0);
    (vm, a)
}

#[test]
fn relocation_requires_a_projected_boundary_and_refuses_without_changing_position() {
    let (mut vm, a) = fixture();
    let host = &mut vm.host().inner;
    let before = host.executed_unit(a).unwrap().position;
    host.next_timer_expiry(4.0, &mut 1000).unwrap();
    let error = host
        .native(
            "SetUnitPosition",
            &[Value::Handle(a), Value::Real(32.0), Value::Real(16.0)],
        )
        .unwrap_err();
    assert!(error.to_string().contains("projected through"));
    assert_eq!(host.executed_unit(a).unwrap().position, before);
    assert_eq!(pools(&mut vm, a), (5.0, 0.0));
    project(&mut vm, 4.0).unwrap();
    vm.host()
        .inner
        .native(
            "SetUnitPosition",
            &[Value::Handle(a), Value::Real(32.0), Value::Real(16.0)],
        )
        .unwrap();
    project(&mut vm, 8.0).unwrap();
    assert_eq!(pools(&mut vm, a), (6.0, 4.0));
}

#[test]
fn unprojected_motion_refuses_a_clock_rebase_without_changing_the_clock_or_pools() {
    let (mut vm, a) = fixture_with_clock(true);
    move_to(&mut vm, a, [72.0, 16.0]);
    vm.host().inner.next_timer_expiry(1.0, &mut 1000).unwrap();
    let before = vm.host().inner.native("GetTimeOfDayScale", &[]).unwrap();
    let error = vm
        .host()
        .inner
        .native("SetTimeOfDayScale", &[Value::Real(2.0)])
        .unwrap_err();
    assert!(error.to_string().contains("motion projected"));
    assert_eq!(
        vm.host().inner.native("GetTimeOfDayScale", &[]).unwrap(),
        before
    );
    assert_eq!(pools(&mut vm, a), (5.0, 0.0));
    project(&mut vm, 1.0).unwrap();
    vm.host()
        .inner
        .native("SetTimeOfDayScale", &[Value::Real(2.0)])
        .unwrap();
}

fn project(vm: &mut Vm, now: f64) -> Result<(), String> {
    let host = &mut vm.host().inner;
    host.next_timer_expiry(now, &mut 1000).unwrap();
    host.advance_unit_activity(now, &mut 65536)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn move_to(vm: &mut Vm, unit: HandleId, end: [f32; 2]) {
    assert_eq!(
        vm.host()
            .inner
            .native(
                "IssuePointOrder",
                &[
                    Value::Handle(unit),
                    Value::Str("alpha".into()),
                    Value::Real(end[0]),
                    Value::Real(end[1])
                ]
            )
            .unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn stationary_units_use_map_blight_and_mana_remains_ungated() {
    let (mut vm, a) = fixture();
    project(&mut vm, 4.0).unwrap();
    assert_eq!(pools(&mut vm, a), (5.0, 2.0));
    vm.host()
        .inner
        .native(
            "SetUnitPosition",
            &[Value::Handle(a), Value::Real(32.0), Value::Real(16.0)],
        )
        .unwrap();
    project(&mut vm, 8.0).unwrap();
    assert_eq!(pools(&mut vm, a), (6.0, 4.0));
}

#[test]
fn moving_regeneration_splits_at_grid_crossings_and_matches_smaller_projections() {
    let (mut vm, a) = fixture();
    let speed = vm
        .host()
        .inner
        .native("GetUnitMoveSpeed", &[Value::Handle(a)])
        .unwrap()
        .as_real()
        .unwrap();
    move_to(&mut vm, a, [72.0, 16.0]);
    let until = 56.0 / f64::from(speed) + 1.0;
    project(&mut vm, until).unwrap();
    let expected = 5.0 + 0.25 * 16.0 / speed;
    assert!(
        (pools(&mut vm, a).0 - expected).abs() < 1e-5,
        "{:?} expected {expected}",
        pools(&mut vm, a)
    );
    let (mut split, other) = fixture();
    move_to(&mut split, other, [72.0, 16.0]);
    for part in 1..=16 {
        project(&mut split, until * f64::from(part) / 16.0).unwrap();
    }
    assert!((pools(&mut split, other).0 - expected).abs() < 1e-5);
    assert!((pools(&mut split, other).1 - pools(&mut vm, a).1).abs() < 1e-5);
}

#[test]
fn crossing_budget_failure_preserves_pools_and_discards_span_output() {
    for budget in 1..=128 {
        let (mut vm, a) = fixture();
        move_to(&mut vm, a, [72.0, 16.0]);
        let host = &mut vm.host().inner;
        host.next_timer_expiry(10.0, &mut 1000).unwrap();
        let mut spans = Vec::new();
        let mut work = budget;
        let result = host.advance_unit_activity_spans(10.0, &mut work, &mut spans);
        if result.is_err_and(|e| e.to_string().contains("initial blight work exhausted")) {
            assert!(spans.is_empty());
            assert_eq!(pools(&mut vm, a), (5.0, 0.0));
            return;
        }
    }
    panic!("the fixture must exercise a refusal after motion and before regeneration commit");
}

use super::*;

fn fields(acquire: &str) -> Vec<([u8; 4], ObjectField)> {
    let mut fields = armed("64", "normal");
    fields.retain(|(id, _)| id != b"W005");
    fields.push(column(
        b"W005",
        SlkId::UnitWeapons,
        "acquire",
        FieldKind::Real,
        acquire,
    ));
    fields.push(column(
        b"usle",
        SlkId::UnitData,
        "canSleep",
        FieldKind::Bool,
        "1",
    ));
    fields
}

fn hour(vm: &mut Vm, hour: f32) {
    let clock = vm.global("CLOCK");
    vm.host()
        .inner
        .native("SetFloatGameState", &[clock, Value::Real(hour)])
        .unwrap();
}

fn sleeping(vm: &mut Vm, unit: HandleId) -> bool {
    vm.host()
        .inner
        .native("UnitIsSleeping", &[Value::Handle(unit)])
        .unwrap()
        == Value::Bool(true)
}

#[test]
fn natural_sleep_uses_source_permission_and_day_window_and_wake_is_independent() {
    let (mut vm, a, b) = open_clocked(&fields("0"), rules());
    assert_eq!(
        vm.host()
            .inner
            .native("UnitCanSleep", &[Value::Handle(a)])
            .unwrap(),
        Value::Bool(true)
    );
    assert!(!sleeping(&mut vm, a));
    hour(&mut vm, 20.0);
    step(&mut vm, 1.0 / 64.0);
    assert!(sleeping(&mut vm, a));
    assert!(sleeping(&mut vm, b));
    vm.host()
        .inner
        .native("UnitAddSleep", &[Value::Handle(a), Value::Bool(false)])
        .unwrap();
    assert!(!sleeping(&mut vm, a));
    step(&mut vm, 2.0 / 64.0);
    assert!(!sleeping(&mut vm, a));
    assert!(sleeping(&mut vm, b));
    for _ in 0..2 {
        vm.host()
            .inner
            .native("UnitAddSleep", &[Value::Handle(a), Value::Bool(true)])
            .unwrap();
    }
    step(&mut vm, 3.0 / 64.0);
    assert!(sleeping(&mut vm, a));
    vm.host()
        .inner
        .native("UnitWakeUp", &[Value::Handle(a)])
        .unwrap();
    assert!(!sleeping(&mut vm, a));
    assert!(sleeping(&mut vm, b));
    step(&mut vm, 4.0 / 64.0);
    assert!(!sleeping(&mut vm, a));
    step(&mut vm, 5.0 / 64.0);
    assert!(sleeping(&mut vm, a));
    hour(&mut vm, 9.0);
    step(&mut vm, 6.0 / 64.0);
    assert!(!sleeping(&mut vm, a));
    assert!(!sleeping(&mut vm, b));
}

#[test]
fn sleeping_units_do_not_acquire_and_damage_wakes_them_to_retaliate() {
    let (mut vm, a, b) = open_clocked(&fields("64"), rules());
    hour(&mut vm, 20.0);
    step(&mut vm, 1.0 / 64.0);
    for unit in [a, b] {
        assert!(sleeping(&mut vm, unit));
        assert!(
            vm.host()
                .inner
                .executed_unit(unit)
                .unwrap()
                .combat
                .as_ref()
                .unwrap()
                .target
                .is_none()
        );
    }
    vm.host()
        .inner
        .native("UnitWakeUp", &[Value::Handle(a)])
        .unwrap();
    step(&mut vm, 2.0 / 64.0);
    assert_eq!(
        vm.host()
            .inner
            .executed_unit(a)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .target,
        Some(b)
    );
    assert!(sleeping(&mut vm, b));
    let before = vm
        .host()
        .inner
        .executed_unit(b)
        .unwrap()
        .vitals
        .unwrap()
        .life;
    step(&mut vm, 0.5);
    assert!(
        vm.host()
            .inner
            .executed_unit(b)
            .unwrap()
            .vitals
            .unwrap()
            .life
            < before
    );
    assert!(!sleeping(&mut vm, b));
    assert_eq!(
        vm.host()
            .inner
            .executed_unit(b)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .target,
        Some(a)
    );
}

#[test]
fn natural_sleep_respects_a_map_override_and_an_accepted_move_wakes_and_progresses() {
    let mut fields = fields("0");
    fields.last_mut().unwrap().1.value = Ok(FieldValue::Override(pcx_objects::Value::Int(0)));
    let (mut vm, a, _) = open_clocked(&fields, rules());
    hour(&mut vm, 20.0);
    step(&mut vm, 1.0 / 64.0);
    assert_eq!(
        vm.host()
            .inner
            .native("UnitCanSleep", &[Value::Handle(a)])
            .unwrap(),
        Value::Bool(false)
    );
    assert!(!sleeping(&mut vm, a));
    vm.host()
        .inner
        .native("UnitAddSleep", &[Value::Handle(a), Value::Bool(true)])
        .unwrap();
    step(&mut vm, 2.0 / 64.0);
    assert!(sleeping(&mut vm, a));
    let before = vm.host().inner.executed_unit(a).unwrap().position;
    assert_eq!(
        vm.host()
            .inner
            .native(
                "IssuePointOrder",
                &[
                    Value::Handle(a),
                    Value::Str("gamma".into()),
                    Value::Real(72.0),
                    Value::Real(16.0)
                ]
            )
            .unwrap(),
        Value::Bool(true)
    );
    assert!(!sleeping(&mut vm, a));
    step(&mut vm, 3.0 / 64.0);
    assert_ne!(vm.host().inner.executed_unit(a).unwrap().position, before);
    assert!(!sleeping(&mut vm, a));
}

#[test]
fn natural_sleep_checks_arguments_work_and_death() {
    let (mut vm, a, _) = open_clocked(&fields("0"), rules());
    let h = &mut vm.host().inner;
    let player = h.native("Player", &[Value::Int(4)]).unwrap();
    assert!(
        h.native("UnitAddSleep", &[player, Value::Bool(false)])
            .is_err()
    );
    assert!(
        h.native("UnitAddSleep", &[Value::Handle(a), Value::Int(0)])
            .is_err()
    );
    assert_eq!(
        h.native("UnitIsSleeping", &[Value::Null]).unwrap(),
        Value::Bool(false)
    );
    assert_eq!(h.native("UnitWakeUp", &[Value::Null]).unwrap(), Value::Null);
    hour(&mut vm, 20.0);
    let h = &mut vm.host().inner;
    let now = 1.0 / 64.0;
    assert!(h.next_timer_expiry(now, &mut 100).unwrap().is_none());
    assert!(h.advance_unit_activity(now, &mut 0).is_err());
    assert!(!h.executed_unit(a).unwrap().is_sleeping());
    let mut work = 65536;
    h.advance_unit_activity(now, &mut work).unwrap();
    assert!(work <= 65534, "sleep records charge unit work");
    assert!(h.executed_unit(a).unwrap().is_sleeping());
    h.native("KillUnit", &[Value::Handle(a)]).unwrap();
    assert!(!sleeping(&mut vm, a));
}

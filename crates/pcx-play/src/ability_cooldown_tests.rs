use super::*;

fn remaining(vm: &mut Vm, unit: HandleId, ability: [u8; 4]) -> f32 {
    match vm
        .call(
            "remaining_cd",
            &[Value::Handle(unit), Value::Int(i32::from_be_bytes(ability))],
        )
        .unwrap()
    {
        Value::Real(value) => value,
        other => panic!("cooldown result: {other:?}"),
    }
}

fn end(vm: &mut Vm, unit: HandleId, ability: [u8; 4]) {
    vm.call(
        "end_cd",
        &[Value::Handle(unit), Value::Int(i32::from_be_bytes(ability))],
    )
    .unwrap();
}

#[test]
fn script_remaining_and_end_share_the_holy_cast_deadline() {
    let (mut vm, [caster, target, _, _]) = holy_open("", LEARNED, NOTHING);
    vm.call("prime", &[]).unwrap();
    assert_eq!(remaining(&mut vm, caster, *b"AHhb"), 0.);
    assert!(holy_cast(&mut vm, caster, target));
    assert_eq!(remaining(&mut vm, caster, *b"AHhb"), 5.);
    step(&mut vm, 1.25);
    assert_eq!(remaining(&mut vm, caster, *b"AHhb"), 3.75);
    assert!(!holy_cast(&mut vm, caster, target));
    end(&mut vm, caster, *b"AHhb");
    assert_eq!(remaining(&mut vm, caster, *b"AHhb"), 0.);
    assert!(holy_cast(&mut vm, caster, target));
    assert_eq!(holy_life(&mut vm, target), 500.);
    assert_eq!(holy_mana(&mut vm, caster), 70.);
    step(&mut vm, 6.25);
    assert_eq!(remaining(&mut vm, caster, *b"AHhb"), 0.);
}

#[test]
fn queries_and_failed_arguments_leave_clock_rng_payment_and_cooldown_unchanged() {
    let (mut vm, [caster, target, _, _]) = holy_open("", LEARNED, NOTHING);
    vm.call("prime", &[]).unwrap();
    assert!(holy_cast(&mut vm, caster, target));
    let random = vm.host().inner.script_random_state().cloned();
    let clock = vm.host().inner.timer_game_time();
    let mana = holy_mana(&mut vm, caster);
    for _ in 0..8 {
        assert_eq!(remaining(&mut vm, caster, *b"AHhb"), 5.);
    }
    for args in [
        vec![Value::Handle(caster)],
        vec![Value::Int(3), Value::Int(holy_code())],
        vec![Value::Handle(caster), Value::Str("AHhb".into())],
    ] {
        assert!(
            vm.host()
                .inner
                .native("BlzEndUnitAbilityCooldown", &args)
                .is_err()
        );
    }
    let error = vm
        .host()
        .inner
        .native(
            "BlzEndUnitAbilityCooldown",
            &[
                Value::Handle(caster),
                Value::Int(i32::from_be_bytes(*b"Yher")),
            ],
        )
        .unwrap_err();
    assert!(error.contains("not admitted"), "{error}");
    assert_eq!(remaining(&mut vm, caster, *b"AHhb"), 5.);
    assert_eq!(vm.host().inner.script_random_state(), random.as_ref());
    assert_eq!(vm.host().inner.timer_game_time(), clock);
    assert_eq!(holy_mana(&mut vm, caster), mana);
    assert_eq!(holy_life(&mut vm, target), 300.);
}

#[test]
fn wrapper_absence_paths_and_disabled_or_dead_units_do_not_fake_readiness() {
    let (mut vm, [caster, target, _, _]) = holy_open("", LEARNED, NOTHING);
    vm.call("prime", &[]).unwrap();
    assert_eq!(
        vm.call("remaining_cd", &[Value::Null, Value::Int(holy_code())])
            .unwrap(),
        Value::Real(0.)
    );
    vm.call("end_cd", &[Value::Null, Value::Int(holy_code())])
        .unwrap();
    assert_eq!(remaining(&mut vm, caster, *b"MISS"), 0.);
    end(&mut vm, caster, *b"MISS");
    assert!(holy_cast(&mut vm, caster, target));
    let owner = vm.host().inner.executed_unit(caster).unwrap().owner;
    vm.host()
        .inner
        .native(
            "SetPlayerAbilityAvailable",
            &[
                Value::Handle(owner),
                Value::Int(holy_code()),
                Value::Bool(false),
            ],
        )
        .unwrap();
    assert!(
        vm.host()
            .inner
            .holy_light_readiness(caster)
            .unwrap()
            .is_none()
    );
    assert_eq!(remaining(&mut vm, caster, *b"AHhb"), 5.);
    vm.host()
        .inner
        .native("KillUnit", &[Value::Handle(caster)])
        .unwrap();
    assert_eq!(remaining(&mut vm, caster, *b"AHhb"), 5.);
    end(&mut vm, caster, *b"AHhb");
    assert_eq!(remaining(&mut vm, caster, *b"AHhb"), 0.);
    vm.host()
        .inner
        .native("RemoveUnit", &[Value::Handle(caster)])
        .unwrap();
    assert_eq!(remaining(&mut vm, caster, *b"AHhb"), 0.);
    end(&mut vm, caster, *b"AHhb");
}

#[test]
fn ending_one_ability_does_not_cancel_a_pending_summon_or_another_units_deadline() {
    let (mut vm, [caster, target, _, _]) = holy_open("", "AHhb,AHwe", NOTHING);
    vm.host().inner.admit_water_elemental().unwrap();
    vm.call("prime", &[]).unwrap();
    assert!(holy_cast(&mut vm, caster, target));
    assert_eq!(
        vm.host()
            .inner
            .native(
                "IssueImmediateOrder",
                &[Value::Handle(caster), Value::Str("gamma".into())]
            )
            .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(remaining(&mut vm, caster, *b"AHwe"), 0.);
    end(&mut vm, caster, *b"AHwe");
    step(&mut vm, 0.25);
    assert_eq!(remaining(&mut vm, caster, *b"AHwe"), 2.5);
    end(&mut vm, caster, *b"AHhb");
    assert_eq!(remaining(&mut vm, caster, *b"AHwe"), 2.5);
    let owner = vm.host().inner.executed_unit(caster).unwrap().owner;
    let other = match vm
        .host()
        .inner
        .native(
            "CreateUnit",
            &[
                Value::Handle(owner),
                Value::Int(i32::from_be_bytes(*b"H001")),
                Value::Real(16.),
                Value::Real(24.),
                Value::Real(0.),
            ],
        )
        .unwrap()
    {
        Value::Handle(unit) => unit,
        other => panic!("second caster: {other:?}"),
    };
    assert_eq!(remaining(&mut vm, other, *b"AHwe"), 0.);
    assert_eq!(
        vm.host()
            .inner
            .native(
                "IssueImmediateOrder",
                &[Value::Handle(other), Value::Str("gamma".into())]
            )
            .unwrap(),
        Value::Bool(true)
    );
    step(&mut vm, 0.5);
    assert_eq!(remaining(&mut vm, other, *b"AHwe"), 2.5);
    end(&mut vm, other, *b"AHwe");
    assert_eq!(remaining(&mut vm, other, *b"AHwe"), 0.);
    assert_eq!(remaining(&mut vm, caster, *b"AHwe"), 2.25);
}

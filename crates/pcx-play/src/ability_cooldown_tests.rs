use super::*;

fn source_duration(vm: &mut Vm, ability: [u8; 4], rank: i32) -> f32 {
    match vm
        .call(
            "source_cd",
            &[Value::Int(i32::from_be_bytes(ability)), Value::Int(rank)],
        )
        .unwrap()
    {
        Value::Real(value) => value,
        other => panic!("source duration: {other:?}"),
    }
}

#[test]
fn global_duration_uses_source_rows_without_learning_units_or_cast_admission() {
    let (mut vm, units) = holy_open("", UNLEARNED, NOTHING);
    for unit in units {
        vm.host()
            .inner
            .native("RemoveUnit", &[Value::Handle(unit)])
            .unwrap();
    }
    for (rank, expected) in [(1, 2.5), (2, 4.5), (3, 6.5)] {
        assert_eq!(source_duration(&mut vm, *b"AHwe", rank), expected);
    }
    assert_eq!(source_duration(&mut vm, *b"Yher", 1), 0.);
    for alias in [*b"NONE", *b"ahhb", [255, 0, 0, 0]] {
        assert_eq!(source_duration(&mut vm, alias, 1), 0.);
    }
    assert_eq!(source_duration(&mut vm, *b"AHhb", -1), 0.);
    assert_eq!(source_duration(&mut vm, *b"AHhb", 4), 0.);
}

#[test]
fn global_duration_is_independent_of_unit_overrides_and_active_deadlines() {
    let (mut vm, [caster, target, _, _]) = holy_open("", LEARNED, NOTHING);
    vm.call("prime", &[]).unwrap();
    assert!(holy_cast(&mut vm, caster, target));
    set_duration(&mut vm, caster, *b"AHhb", 1, 1.125);
    let clock = vm.host().inner.timer_game_time();
    let random = vm.host().inner.script_random_state().cloned();
    let mana = holy_mana(&mut vm, caster);
    for _ in 0..8 {
        assert_eq!(source_duration(&mut vm, *b"AHhb", 1), 5.);
    }
    assert_eq!(duration(&mut vm, caster, *b"AHhb", 1), 1.125);
    assert_eq!(remaining(&mut vm, caster, *b"AHhb"), 5.);
    assert_eq!(vm.host().inner.timer_game_time(), clock);
    assert_eq!(vm.host().inner.script_random_state().cloned(), random);
    assert_eq!(holy_mana(&mut vm, caster), mana);
    for args in [
        vec![],
        vec![Value::Int(holy_code()), Value::Int(0)],
        vec![Value::Handle(caster), Value::Int(1)],
    ] {
        assert!(
            vm.host()
                .inner
                .native("BlzGetAbilityCooldown", &args)
                .is_err()
        );
    }
    assert_eq!(remaining(&mut vm, caster, *b"AHhb"), 5.);
}

#[test]
fn global_duration_applies_typed_ranked_map_edits_and_custom_aliases() {
    let edit = |rank, value| pcx_objects::Mod {
        field: *b"fCoo",
        value_type: pcx_objects::ValueType::Unreal,
        selectors: Some([rank, 0]),
        value: pcx_objects::Value::Real(value),
        end_word: 0,
    };
    let mut overlay = pcx_objects::editor::Overlay::default();
    overlay.mods.insert("AHhb".into(), vec![edit(2, 1.75)]);
    overlay
        .custom
        .insert("A001".into(), ("AHwe".into(), vec![edit(1, 9.125)]));
    let (mut vm, _) = holy_open_with_inputs(
        "",
        LEARNED,
        NOTHING,
        HOLY_ART_END_MS,
        None,
        holy_tables(),
        overlay,
    );
    assert_eq!(source_duration(&mut vm, *b"AHhb", 1), 5.);
    assert_eq!(source_duration(&mut vm, *b"AHhb", 2), 1.75);
    assert_eq!(source_duration(&mut vm, *b"AHwe", 1), 2.5);
    assert_eq!(source_duration(&mut vm, *b"A001", 1), 9.125);
    assert_eq!(source_duration(&mut vm, *b"A001", 2), 4.5);
}

#[test]
fn malformed_global_duration_rows_refuse_instead_of_becoming_zero() {
    for text in ["garbage", "NaN", "-1"] {
        let (mut vm, _) = holy_open_with_inputs(
            "",
            LEARNED,
            NOTHING,
            HOLY_ART_END_MS,
            None,
            holy_tables_with(&[("Cool1", text)]),
            Default::default(),
        );
        let err = vm
            .host()
            .inner
            .native(
                "BlzGetAbilityCooldown",
                &[Value::Int(holy_code()), Value::Int(1)],
            )
            .unwrap_err();
        assert!(err.to_string().contains("BlzGetAbilityCooldown"), "{err}");
    }
}

fn duration(vm: &mut Vm, unit: HandleId, ability: [u8; 4], rank: i32) -> f32 {
    match vm
        .call(
            "duration_cd",
            &[
                Value::Handle(unit),
                Value::Int(i32::from_be_bytes(ability)),
                Value::Int(rank),
            ],
        )
        .unwrap()
    {
        Value::Real(value) => value,
        other => panic!("duration: {other:?}"),
    }
}

fn set_duration(vm: &mut Vm, unit: HandleId, ability: [u8; 4], rank: i32, value: f32) {
    vm.call(
        "set_duration_cd",
        &[
            Value::Handle(unit),
            Value::Int(i32::from_be_bytes(ability)),
            Value::Int(rank),
            Value::Real(value),
        ],
    )
    .unwrap();
}

#[test]
fn whole_unit_reset_clears_both_spells_without_payment_refund_or_config_changes() {
    let (mut vm, [caster, target, _, _]) = holy_open("", "AHhb,AHwe", NOTHING);
    vm.host().inner.admit_water_elemental().unwrap();
    vm.call("prime", &[]).unwrap();
    set_duration(&mut vm, caster, *b"AHhb", 1, 3.125);
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
    step(&mut vm, 0.25);
    assert!(remaining(&mut vm, caster, *b"AHhb") > 0.);
    assert!(remaining(&mut vm, caster, *b"AHwe") > 0.);
    let mana = holy_mana(&mut vm, caster);
    let life = holy_life(&mut vm, target);
    let random = vm.host().inner.script_random_state().cloned();
    let clock = vm.host().inner.timer_game_time();
    vm.call("reset_cd", &[Value::Handle(caster)]).unwrap();
    assert_eq!(remaining(&mut vm, caster, *b"AHhb"), 0.);
    assert_eq!(remaining(&mut vm, caster, *b"AHwe"), 0.);
    assert_eq!(duration(&mut vm, caster, *b"AHhb", 1), 3.125);
    assert_eq!(holy_mana(&mut vm, caster), mana);
    assert_eq!(holy_life(&mut vm, target), life);
    assert_eq!(vm.host().inner.script_random_state().cloned(), random);
    assert_eq!(vm.host().inner.timer_game_time(), clock);
    assert!(holy_cast(&mut vm, caster, target));
    assert_eq!(remaining(&mut vm, caster, *b"AHhb"), 3.125);
}

#[test]
fn reset_does_not_cancel_pending_summons_and_invalid_handles_leave_deadlines_alone() {
    let (mut vm, [caster, target, _, _]) = holy_open("", "AHhb,AHwe", NOTHING);
    vm.host().inner.admit_water_elemental().unwrap();
    vm.call("prime", &[]).unwrap();
    assert!(holy_cast(&mut vm, caster, target));
    let owner = vm.host().inner.executed_unit(caster).unwrap().owner;
    for args in [vec![], vec![Value::Int(3)], vec![Value::Handle(owner)]] {
        assert!(vm.host().inner.native("UnitResetCooldown", &args).is_err());
        assert_eq!(remaining(&mut vm, caster, *b"AHhb"), 5.);
    }
    vm.call("reset_cd", &[Value::Null]).unwrap();
    assert_eq!(remaining(&mut vm, caster, *b"AHhb"), 5.);
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
    vm.call("reset_cd", &[Value::Handle(caster)]).unwrap();
    step(&mut vm, 0.25);
    assert_eq!(remaining(&mut vm, caster, *b"AHwe"), 2.5);
    vm.host()
        .inner
        .native("KillUnit", &[Value::Handle(caster)])
        .unwrap();
    vm.call("reset_cd", &[Value::Handle(caster)]).unwrap();
    assert_eq!(remaining(&mut vm, caster, *b"AHwe"), 0.);
    vm.host()
        .inner
        .native("RemoveUnit", &[Value::Handle(caster)])
        .unwrap();
    vm.call("reset_cd", &[Value::Handle(caster)]).unwrap();
}

#[test]
fn configured_duration_changes_the_next_cast_without_rewriting_an_active_deadline() {
    let (mut vm, [caster, target, _, _]) = holy_open("", LEARNED, NOTHING);
    vm.call("prime", &[]).unwrap();
    let random = vm.host().inner.script_random_state().cloned();
    assert_eq!(duration(&mut vm, caster, *b"AHhb", 1), 5.);
    set_duration(&mut vm, caster, *b"AHhb", 1, 7.25);
    assert!(holy_cast(&mut vm, caster, target));
    assert_eq!(remaining(&mut vm, caster, *b"AHhb"), 7.25);
    step(&mut vm, 1.);
    set_duration(&mut vm, caster, *b"AHhb", 1, 0.75);
    assert_eq!(duration(&mut vm, caster, *b"AHhb", 1), 0.75);
    assert_eq!(remaining(&mut vm, caster, *b"AHhb"), 6.25);
    end(&mut vm, caster, *b"AHhb");
    assert!(holy_cast(&mut vm, caster, target));
    assert_eq!(remaining(&mut vm, caster, *b"AHhb"), 0.75);
    assert_eq!(vm.host().inner.script_random_state().cloned(), random);
}

#[test]
fn configured_durations_are_one_based_rank_and_unit_scoped_and_refuse_invalid_values() {
    let (mut vm, [caster, _, _, _]) = holy_open("", LEARNED, NOTHING);
    let owner = vm.host().inner.executed_unit(caster).unwrap().owner;
    let Value::Handle(other) = vm
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
    else {
        panic!()
    };
    set_duration(&mut vm, caster, *b"AHhb", 2, 1.125);
    assert_eq!(duration(&mut vm, caster, *b"AHhb", 1), 5.);
    assert_eq!(duration(&mut vm, caster, *b"AHhb", 2), 1.125);
    assert_eq!(duration(&mut vm, other, *b"AHhb", 2), 5.);
    for rank in [-1, 0, 4] {
        set_duration(&mut vm, caster, *b"AHhb", rank, 12.);
        assert_eq!(duration(&mut vm, caster, *b"AHhb", rank), 0.);
    }
    for value in [-1., f32::NAN, f32::INFINITY] {
        assert!(
            vm.host()
                .inner
                .native(
                    "BlzSetUnitAbilityCooldown",
                    &[
                        Value::Handle(caster),
                        Value::Int(holy_code()),
                        Value::Int(2),
                        Value::Real(value)
                    ]
                )
                .is_err()
        );
    }
    assert_eq!(duration(&mut vm, caster, *b"AHhb", 2), 1.125);
    let state = vm
        .host()
        .inner
        .executed_unit(caster)
        .unwrap()
        .abilities
        .as_ref()
        .unwrap();
    assert_eq!(state.cooldowns().len(), 1);
    assert_eq!(duration(&mut vm, caster, *b"NONE", 1), 0.);
}

#[test]
fn pending_summon_uses_its_units_latest_configured_duration_at_the_cast_point() {
    let (mut vm, [caster, _, _, _]) = holy_open("", "AHhb,AHwe", NOTHING);
    vm.host().inner.admit_water_elemental().unwrap();
    vm.call("prime", &[]).unwrap();
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
    set_duration(&mut vm, caster, *b"AHwe", 1, 1.375);
    step(&mut vm, 0.25);
    assert_eq!(remaining(&mut vm, caster, *b"AHwe"), 1.375);
    assert_eq!(duration(&mut vm, caster, *b"AHhb", 1), 5.);
}

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

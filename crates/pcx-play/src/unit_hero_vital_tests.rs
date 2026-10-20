use super::*;
#[test]
fn live_hero_vitals_follow_levels_and_preserve_explicit_maxima_and_deficits() {
    let (hero, combat) = hero_rules();
    let (mut vm, a, b) = open_selected(&hero_fields("STR"), Some(hero), combat);
    let initial = vm.host().inner.unit_vitals(a).unwrap().unwrap();
    let ordinary = vm.host().inner.unit_vitals(b).unwrap().unwrap();
    let state = |vm: &mut Vm, name: &str| vm.global(name);
    // Explicit maxima are independent of the retained base attribute contribution.
    for (name, value) in [
        ("UNIT_STATE_MAX_LIFE", 777.0),
        ("UNIT_STATE_LIFE", 321.0),
        ("UNIT_STATE_MAX_MANA", 333.0),
        ("UNIT_STATE_MANA", 117.0),
    ] {
        let property = state(&mut vm, name);
        vm.host()
            .inner
            .native(
                "SetUnitState",
                &[Value::Handle(a), property, Value::Real(value)],
            )
            .unwrap();
    }
    let level = |vm: &mut Vm, n| {
        vm.host()
            .inner
            .native(
                "SetHeroLevel",
                &[Value::Handle(a), Value::Int(n), Value::Bool(false)],
            )
            .unwrap()
    };
    level(&mut vm, 4);
    let fourth = vm.host().inner.unit_vitals(a).unwrap().unwrap();
    assert_eq!((fourth.max_life, fourth.life), (827.0, 371.0));
    assert_eq!((fourth.max_mana, fourth.mana), (333.0, 117.0));
    level(&mut vm, 5);
    let fifth = vm.host().inner.unit_vitals(a).unwrap().unwrap();
    assert_eq!((fifth.max_life, fifth.life), (902.0, 446.0));
    assert_eq!((fifth.max_mana, fifth.mana), (348.0, 132.0));
    assert_eq!(fifth.max_life - fifth.life, 777.0 - 321.0);
    assert_eq!(fifth.max_mana - fifth.mana, 333.0 - 117.0);
    for n in [5, 4] {
        level(&mut vm, n);
        assert_eq!(vm.host().inner.unit_vitals(a).unwrap(), Some(fifth));
    }
    assert_eq!(vm.host().inner.unit_vitals(b).unwrap(), Some(ordinary));
    assert!(initial.max_life > 0.0);
}
#[test]
fn hero_vital_update_refusal_is_atomic_at_unqualified_dead_boundary() {
    let (hero, combat) = hero_rules();
    let (mut vm, a, _) = open_selected(&hero_fields("STR"), Some(hero), combat);
    let state = vm.global("UNIT_STATE_LIFE");
    vm.host()
        .inner
        .native("SetUnitState", &[Value::Handle(a), state, Value::Real(0.0)])
        .unwrap();
    let before = vm.host().inner.unit_vitals(a).unwrap();
    let level_before = vm
        .host()
        .inner
        .executed_unit(a)
        .unwrap()
        .abilities
        .as_ref()
        .unwrap()
        .hero
        .as_ref()
        .unwrap()
        .level;
    let error = vm
        .host()
        .inner
        .native(
            "SetHeroLevel",
            &[Value::Handle(a), Value::Int(5), Value::Bool(false)],
        )
        .unwrap_err();
    assert!(
        error.contains("living/dead minimum") && error.contains("SetHeroLevel"),
        "{error}"
    );
    assert_eq!(vm.host().inner.unit_vitals(a).unwrap(), before);
    assert_eq!(
        vm.host()
            .inner
            .executed_unit(a)
            .unwrap()
            .abilities
            .as_ref()
            .unwrap()
            .hero
            .as_ref()
            .unwrap()
            .level,
        level_before
    );
}

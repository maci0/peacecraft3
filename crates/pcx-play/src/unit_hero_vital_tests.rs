use super::*;

#[test]
fn widget_life_reads_canonical_unit_state_through_death() {
    let (mut vm, a, _) = open(&armed("100", "normal"));
    let state = vm.global("UNIT_STATE_LIFE");
    let h = &mut vm.host().inner;
    for life in [13.5, 0.0] {
        h.native(
            "SetUnitState",
            &[Value::Handle(a), state.clone(), Value::Real(life)],
        )
        .unwrap();
        let before = (h.inner().handles(), h.unit_revision());
        let actual = h.native("GetWidgetLife", &[Value::Handle(a)]).unwrap();
        assert_eq!(actual, Value::Real(life));
        assert_eq!(
            actual,
            h.native("GetUnitState", &[Value::Handle(a), state.clone()])
                .unwrap()
        );
        assert_eq!((h.inner().handles(), h.unit_revision()), before);
    }
    assert!(h.executed_unit(a).unwrap().combat.as_ref().unwrap().dead);
}

#[test]
fn widget_life_refuses_unqualified_families_and_vitals_without_mutation() {
    let (mut vm, a, b) = open(&armed("100", "normal"));
    let h = &mut vm.host().inner;
    let owner = h.executed_unit(a).unwrap().owner;
    h.native("RemoveUnit", &[Value::Handle(b)]).unwrap();
    h.test_attach_placement(
        crate::unit_placement::tests::placement(source(), 100)
            .with_stationary(
                pcx_map::Pathing {
                    version: 0,
                    width: 32,
                    height: 32,
                    cells: vec![0; 1024],
                },
                Vec::new(),
            )
            .unwrap()
            .with_script_paths(|_, _| Ok("_".into())),
    );
    let tree = h
        .native(
            "CreateDestructable",
            &[
                Value::Int(i32::from_be_bytes(*b"LTlt")),
                Value::Real(16.0),
                Value::Real(16.0),
                Value::Real(0.0),
                Value::Real(1.0),
                Value::Int(0),
            ],
        )
        .unwrap();
    h.set_test_life(a, f32::NAN);
    let before = (h.inner().handles(), h.unit_revision(), h.timer_game_time());
    for value in [
        Value::Null,
        Value::Handle(owner),
        tree,
        Value::Handle(b),
        Value::Handle(a),
    ] {
        assert!(h.native("GetWidgetLife", &[value]).is_err());
    }
    assert_eq!(
        (h.inner().handles(), h.unit_revision(), h.timer_game_time()),
        before
    );
    assert!(h.unit_vitals(a).unwrap().unwrap().life.is_nan());
}

#[test]
fn widget_life_requires_selected_source_and_signature() {
    let get_state = "constant native GetUnitState takes unit u, unitstate s returns real\ntype unitstate extends handle\n";
    for (family, in_map, expected) in [
        ("unit", false, "incompatible native"),
        ("widget", true, "multiple declaration sources"),
        ("widget", false, "vitals not admitted"),
    ] {
        let native = format!("native GetWidgetLife takes {family} w returns real\n");
        let (map, common) = if in_map {
            (native.as_str(), get_state.to_string())
        } else {
            ("// no map globals", format!("{get_state}{native}"))
        };
        let mut host = position_host_admitting(8, 4, map, &common, &[], |h| h);
        let error = host.native("GetWidgetLife", &[Value::Null]).unwrap_err();
        assert!(error.contains(expected), "{error}");
    }
}

#[test]
fn live_hero_vitals_follow_levels_ignore_maximum_writes_and_preserve_deficits() {
    let (hero, combat) = hero_rules();
    let (mut vm, a, b) = open_selected(&hero_fields("STR"), Some(hero), combat);
    let initial = vm.host().inner.unit_vitals(a).unwrap().unwrap();
    let ordinary = vm.host().inner.unit_vitals(b).unwrap().unwrap();
    let state = |vm: &mut Vm, name: &str| vm.global(name);
    for (name, value) in [
        ("UNIT_STATE_MAX_LIFE", 777.0),
        ("UNIT_STATE_MAX_MANA", 333.0),
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
    assert_eq!(vm.host().inner.unit_vitals(a).unwrap(), Some(initial));
    for (name, value) in [("UNIT_STATE_LIFE", 321.0), ("UNIT_STATE_MANA", 60.0)] {
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
    assert_eq!((fourth.max_life, fourth.life), (800.0, 371.0));
    assert_eq!((fourth.max_mana, fourth.mana), (105.0, 60.0));
    level(&mut vm, 5);
    let fifth = vm.host().inner.unit_vitals(a).unwrap().unwrap();
    assert_eq!((fifth.max_life, fifth.life), (875.0, 446.0));
    assert_eq!((fifth.max_mana, fifth.mana), (120.0, 75.0));
    assert_eq!(fifth.max_life - fifth.life, 800.0 - 371.0);
    assert_eq!(fifth.max_mana - fifth.mana, 105.0 - 60.0);
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

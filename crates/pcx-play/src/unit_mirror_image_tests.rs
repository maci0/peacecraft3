use super::*;

fn open() -> (Vm, HandleId) {
    let mut rules = hero_rules().0;
    let mut xp = crate::hero_experience::tests::rules();
    xp.awards = Some((
        pcx_objects::editor::hero_experience_awards::Awards::read(
            &[
                ("synthetic", super::experience_kills::PROFILE),
                ("nonzero creep control", b"[Misc]\nHeroFactorXP=100,100,100"),
            ],
            rules.progression.max_level,
            4096,
            16,
        )
        .unwrap(),
        0.5,
    ));
    rules.experience = Some(Arc::new(xp));
    let (mut vm, [caster, _, _, _]) = holy_open_with_profile(
        "",
        "",
        "AHwe",
        HOLY_ART_END_MS,
        None,
        (holy_tables(), Default::default(), rules),
    );
    vm.call("learn", &[Value::Int(i32::from_be_bytes(*b"AHwe"))])
        .unwrap();
    vm.call("prime", &[]).unwrap();
    (vm, caster)
}

#[test]
fn image_factory_retains_effective_snapshot_without_public_hero_identity() {
    let (mut vm, caster) = open();
    let kinds = [vm.global("UNIT_TYPE_HERO"), vm.global("UNIT_TYPE_SUMMONED")];
    let h = &mut vm.host().inner;
    h.native(
        "SetHeroLevel",
        &[Value::Handle(caster), Value::Int(4), Value::Bool(false)],
    )
    .unwrap();
    let before = h.executed_unit(caster).unwrap().clone();
    let count = h.executed_units().count();
    let food = h.test_live_food(before.owner, "fused").unwrap();
    assert!(food > 0);
    let random = h.script_random_state().cloned();
    h.test_mirror_image_factory(caster, *b"AHwe", [80., 80.], [0.125, 2.75], false)
        .unwrap();
    assert_eq!(h.executed_units().count(), count);
    assert_eq!(h.executed_unit(caster).unwrap(), &before);
    assert_eq!(h.script_random_state(), random.as_ref());
    let image = h
        .test_mirror_image_factory(caster, *b"AHwe", [80., 80.], [0.125, 2.75], true)
        .unwrap()
        .unwrap();
    let record = h.executed_unit(image).unwrap();
    assert_ne!(caster, image);
    assert_eq!(h.test_live_food(before.owner, "fused").unwrap(), food);
    assert_eq!(record.owner, before.owner);
    assert_eq!(record.definition, before.definition);
    assert_eq!(record.vitals, before.vitals);
    assert!(record.abilities.as_ref().unwrap().hero.is_none());
    assert_eq!(
        record
            .abilities
            .as_ref()
            .unwrap()
            .levels()
            .collect::<Vec<_>>(),
        before
            .abilities
            .as_ref()
            .unwrap()
            .levels()
            .collect::<Vec<_>>()
    );
    assert!(!record.summoned);
    assert!(
        h.guard_state()
            .is_none_or(|state| state.get(image).is_none())
    );
    assert_eq!(record.illusion.as_ref().unwrap().caster, caster);
    assert_eq!(record.illusion.as_ref().unwrap().dealt_factor, 0.125);
    assert_eq!(record.illusion.as_ref().unwrap().taken_factor, 2.75);
    assert_eq!(
        h.hero_combat_bonuses(image).unwrap(),
        h.hero_combat_bonuses(caster).unwrap()
    );
    assert_eq!(
        h.native("IsUnitIllusion", &[Value::Handle(image)]).unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        h.native("IsUnitIllusion", &[Value::Handle(caster)])
            .unwrap(),
        Value::Bool(false)
    );
    assert_eq!(
        h.native("GetHeroLevel", &[Value::Handle(image)]).unwrap(),
        Value::Int(0)
    );
    assert_eq!(
        h.native("GetHeroSkillPoints", &[Value::Handle(image)])
            .unwrap(),
        Value::Int(0)
    );
    assert_eq!(
        h.native("GetHeroXP", &[Value::Handle(image)]).unwrap(),
        Value::Int(0)
    );
    for name in ["GetHeroStr", "GetHeroAgi", "GetHeroInt"] {
        assert_eq!(
            h.native(name, &[Value::Handle(image), Value::Bool(false)])
                .unwrap(),
            Value::Int(0)
        );
    }
    assert_eq!(
        h.native(
            "GetUnitAbilityLevel",
            &[
                Value::Handle(image),
                Value::Int(i32::from_be_bytes(*b"AHwe"))
            ]
        )
        .unwrap(),
        h.native(
            "GetUnitAbilityLevel",
            &[
                Value::Handle(caster),
                Value::Int(i32::from_be_bytes(*b"AHwe"))
            ]
        )
        .unwrap()
    );
    for kind in kinds {
        assert_eq!(
            h.native("IsUnitType", &[Value::Handle(image), kind])
                .unwrap(),
            Value::Bool(false)
        );
    }
    h.native("RemoveUnit", &[Value::Handle(image)]).unwrap();
    assert_eq!(
        h.native("IsUnitIllusion", &[Value::Handle(image)]).unwrap(),
        Value::Bool(false)
    );
    assert_eq!(
        h.native("IsUnitIllusion", &[Value::Null]).unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn image_preflight_refusals_preserve_caster_units_and_every_random_stream() {
    let (mut vm, caster) = open();
    let h = &mut vm.host().inner;
    let before = h.executed_unit(caster).unwrap().clone();
    let count = h.executed_units().count();
    let random = h.script_random_state().cloned();
    for (ability, point, factors) in [
        (*b"Nope", [80., 80.], [0., 2.]),
        (*b"AHwe", [f32::NAN, 80.], [0., 2.]),
        (*b"AHwe", [80., 80.], [-1., 2.]),
    ] {
        assert!(
            h.test_mirror_image_factory(caster, ability, point, factors, true)
                .is_err()
        );
        assert_eq!(h.executed_unit(caster).unwrap(), &before);
        assert_eq!(h.executed_units().count(), count);
        assert_eq!(h.script_random_state(), random.as_ref());
    }
}

#[test]
fn custom_cooldown_snapshot_refuses_before_creation_or_random_draws() {
    let (mut vm, caster) = open();
    vm.host().inner.admit_water_elemental().unwrap();
    vm.call(
        "set_duration_cd",
        &[
            Value::Handle(caster),
            Value::Int(i32::from_be_bytes(*b"AHwe")),
            Value::Int(1),
            Value::Real(9.25),
        ],
    )
    .unwrap();
    let h = &mut vm.host().inner;
    let before = h.executed_unit(caster).unwrap().clone();
    let count = h.executed_units().count();
    let random = h.script_random_state().cloned();
    let error = h
        .test_mirror_image_factory(caster, *b"AHwe", [80., 80.], [0., 2.], true)
        .unwrap_err();
    assert!(error.to_string().contains("custom cooldown"));
    assert_eq!(h.executed_unit(caster).unwrap(), &before);
    assert_eq!(h.executed_units().count(), count);
    assert_eq!(h.script_random_state(), random.as_ref());
}

fn public_open() -> (Vm, HandleId) {
    let (mut vm, [caster, _, _, _]) = holy_open_with_profile(
        "",
        "",
        "AHwe",
        HOLY_ART_END_MS,
        None,
        (
            holy_tables_profile(&[], true),
            Default::default(),
            hero_rules().0,
        ),
    );
    vm.call("learn", &[Value::Int(i32::from_be_bytes(*b"AHwe"))])
        .unwrap();
    vm.call("prime", &[]).unwrap();
    vm.host().inner.test_admit_mirror();
    (vm, caster)
}

#[test]
fn public_mirror_order_debits_once_creates_identity_and_expires() {
    let (mut vm, caster) = public_open();
    let h = &mut vm.host().inner;
    let before = h.executed_units().count();
    let random = h.script_random_state().cloned();
    assert_eq!(
        h.native(
            "IssueImmediateOrder",
            &[Value::Handle(caster), Value::Str("gamma".into())]
        )
        .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(h.unit_vitals(caster).unwrap().unwrap().mana, 200.);
    assert_eq!(h.script_random_state(), random.as_ref());
    step(&mut vm, 0.3);
    assert!(!vm.host().inner.executed_unit(caster).unwrap().shown);
    assert_eq!(holy_mana(&mut vm, caster), 193.);
    step(&mut vm, 0.8);
    let h = &mut vm.host().inner;
    let image = h
        .executed_units()
        .find(|(_, r)| r.illusion.is_some())
        .map(|(id, _)| id)
        .unwrap();
    assert_eq!(h.executed_units().count(), before + 1);
    assert!(h.executed_unit(caster).unwrap().shown);
    assert_eq!(
        h.native("IsUnitIllusion", &[Value::Handle(image)]).unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        h.native(
            "IssueImmediateOrder",
            &[Value::Handle(caster), Value::Str("gamma".into())]
        )
        .unwrap(),
        Value::Bool(false)
    );
    assert_ne!(h.script_random_state(), random.as_ref());
    step(&mut vm, 4.4);
    assert!(
        vm.host()
            .inner
            .executed_unit(image)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .dead
    );
}

#[test]
fn interrupted_mirror_windup_has_no_cost_images_or_random_draw() {
    let (mut vm, caster) = public_open();
    let h = &mut vm.host().inner;
    let before = h.executed_units().count();
    let random = h.script_random_state().cloned();
    assert_eq!(
        h.native(
            "IssueImmediateOrder",
            &[Value::Handle(caster), Value::Str("gamma".into())]
        )
        .unwrap(),
        Value::Bool(true)
    );
    h.native(
        "IssueImmediateOrder",
        &[Value::Handle(caster), Value::Str("beta".into())],
    )
    .unwrap();
    step(&mut vm, 1.);
    assert_eq!(holy_mana(&mut vm, caster), 200.);
    assert_eq!(vm.host().inner.executed_units().count(), before);
    assert_eq!(vm.host().inner.script_random_state(), random.as_ref());
}

fn deliver_mirror(vm: &mut Vm) {
    while let Some(event) = vm.host().inner.take_fired_event() {
        if let Some(delivery) = vm.host().inner.begin_fired_event(&event).unwrap() {
            assert!(delivery.conditions.is_empty());
            vm.run_thread(&delivery.actions).unwrap();
            vm.host().inner.finish_fired_event();
        }
    }
}

#[test]
fn synchronous_cast_keeps_boolean_return_and_nested_callback_context() {
    let (mut vm, caster) = public_open();
    vm.call("mirror_listen", &[Value::Bool(true)]).unwrap();
    assert_eq!(vm.call("mirror_cast", &[]).unwrap(), Value::Bool(true));
    assert_eq!(vm.global("MirrorChannels"), Value::Int(1));
    assert_eq!(vm.global("MirrorNested"), Value::Bool(false));
    assert_eq!(vm.global("MirrorCaster"), Value::Handle(caster));
    assert_eq!(
        vm.global("MirrorAbility"),
        Value::Int(i32::from_be_bytes(*b"AHwe"))
    );
    step(&mut vm, 1.);
    assert_eq!(vm.global("MirrorEffects"), Value::Int(0));
    assert_eq!(holy_mana(&mut vm, caster), 200.);
}

#[test]
fn effect_end_and_summon_callbacks_observe_source_ordered_state() {
    let (mut vm, caster) = public_open();
    vm.call("mirror_listen", &[Value::Bool(false)]).unwrap();
    assert_eq!(vm.call("mirror_cast", &[]).unwrap(), Value::Bool(true));
    step(&mut vm, 0.3);
    assert_eq!(holy_mana(&mut vm, caster), 200.);
    deliver_mirror(&mut vm);
    assert_eq!(vm.global("MirrorEffectMana"), Value::Real(200.));
    assert_eq!(vm.global("MirrorEndMana"), Value::Real(193.));
    assert!(!vm.host().inner.executed_unit(caster).unwrap().shown);
    step(&mut vm, 0.9);
    deliver_mirror(&mut vm);
    assert_eq!(vm.global("MirrorSummons"), Value::Int(1));
    let Value::Handle(image) = vm.global("MirrorImage") else {
        panic!("image missing")
    };
    assert!(
        vm.host()
            .inner
            .executed_unit(image)
            .unwrap()
            .illusion
            .is_some()
    );
    assert!(vm.host().inner.executed_unit(caster).unwrap().shown);
}

#[test]
fn paused_cast_deadline_resumes_and_manual_hide_is_not_restored() {
    let (mut vm, caster) = public_open();
    assert_eq!(vm.call("mirror_cast", &[]).unwrap(), Value::Bool(true));
    vm.host()
        .inner
        .native("PauseUnit", &[Value::Handle(caster), Value::Bool(true)])
        .unwrap();
    step(&mut vm, 1.);
    assert_eq!(holy_mana(&mut vm, caster), 200.);
    vm.host()
        .inner
        .native("PauseUnit", &[Value::Handle(caster), Value::Bool(false)])
        .unwrap();
    step(&mut vm, 1.3);
    assert!(!vm.host().inner.executed_unit(caster).unwrap().shown);
    vm.host()
        .inner
        .native("ShowUnit", &[Value::Handle(caster), Value::Bool(false)])
        .unwrap();
    step(&mut vm, 1.9);
    assert!(!vm.host().inner.executed_unit(caster).unwrap().shown);
    assert_eq!(
        vm.host()
            .inner
            .executed_units()
            .filter(|(_, r)| r.illusion.is_some())
            .count(),
        1
    );
}

#[test]
fn mirror_damage_factors_apply_once_to_effective_damage() {
    let (mut vm, _caster) = public_open();
    assert_eq!(vm.call("mirror_cast", &[]).unwrap(), Value::Bool(true));
    step(&mut vm, 0.9);
    let image = vm
        .host()
        .inner
        .executed_units()
        .find(|(_, r)| r.illusion.is_some())
        .map(|(id, _)| id)
        .unwrap();
    let target = match vm.global("E") {
        Value::Handle(id) => id,
        _ => panic!("enemy missing"),
    };
    let h = &mut vm.host().inner;
    let target_life = h.unit_vitals(target).unwrap().unwrap().life;
    h.damage_unit(image, target, 8.).unwrap();
    assert_eq!(
        h.unit_vitals(target).unwrap().unwrap().life,
        target_life - 3.
    );
    let image_life = h.unit_vitals(image).unwrap().unwrap().life;
    h.damage_unit(target, image, 8.).unwrap();
    assert_eq!(
        h.unit_vitals(image).unwrap().unwrap().life,
        image_life - 18.
    );
}

#[test]
fn death_during_windup_retires_pending_cast_without_draw_or_image() {
    let (mut vm, caster) = public_open();
    let random = vm.host().inner.script_random_state().cloned();
    assert_eq!(vm.call("mirror_cast", &[]).unwrap(), Value::Bool(true));
    vm.host().inner.kill_unit(caster, None).unwrap();
    step(&mut vm, 1.);
    assert_eq!(
        vm.host()
            .inner
            .executed_units()
            .filter(|(_, r)| r.illusion.is_some())
            .count(),
        0
    );
    assert_eq!(vm.host().inner.script_random_state(), random.as_ref());
}

#[test]
fn map_override_and_live_rank_snapshot_drive_images_and_later_regeneration() {
    let mut overlay = pcx_objects::editor::Overlay::default();
    overlay.mods.insert(
        "AHwe".into(),
        vec![pcx_objects::Mod {
            field: *b"fCos",
            value_type: pcx_objects::ValueType::Int,
            selectors: Some([1, 0]),
            value: pcx_objects::Value::Int(11),
            end_word: 0,
        }],
    );
    let (mut vm, [caster, _, enemy, _]) = holy_open_with_profile_vitals(
        "",
        "",
        "AHwe",
        HOLY_ART_END_MS,
        None,
        (holy_tables_profile(&[], true), overlay, hero_rules().0),
        HeroVitals {
            life_per_strength: 25.,
            mana_per_intelligence: 100.,
            life_regen_per_strength: 0.25,
            mana_regen_per_intelligence: 0.5,
        },
    );
    vm.call("learn", &[Value::Int(i32::from_be_bytes(*b"AHwe"))])
        .unwrap();
    vm.host()
        .inner
        .native(
            "SetHeroLevel",
            &[Value::Handle(caster), Value::Int(4), Value::Bool(false)],
        )
        .unwrap();
    vm.call("prime", &[]).unwrap();
    vm.host().inner.test_admit_mirror();
    let expected = vm
        .host()
        .inner
        .unit_stat_snapshot(caster)
        .unwrap()
        .attributes
        .unwrap();
    assert_eq!(vm.call("mirror_cast", &[]).unwrap(), Value::Bool(true));
    step(&mut vm, 0.9);
    let h = &mut vm.host().inner;
    let image = h
        .executed_units()
        .find(|(_, r)| r.illusion.is_some())
        .map(|(id, _)| id)
        .unwrap();
    let record = h.executed_unit(image).unwrap();
    assert_eq!(record.illusion.as_ref().unwrap().attributes, expected);
    assert_eq!(
        record.vitals.unwrap().max_life,
        h.unit_vitals(caster).unwrap().unwrap().max_life
    );
    assert_eq!(
        record.vitals.unwrap().max_mana,
        h.unit_vitals(caster).unwrap().unwrap().max_mana
    );
    let cast_mana = h.unit_vitals(image).unwrap().unwrap().mana;
    assert!(cast_mana < 200. && cast_mana > 189.);
    h.damage_unit(enemy, image, 8.).unwrap();
    let before = h.unit_vitals(image).unwrap().unwrap();
    let start = h.timer_game_time().unwrap();
    step(&mut vm, start + 0.25);
    let after = vm.host().inner.unit_vitals(image).unwrap().unwrap();
    assert!(after.life > before.life);
    assert!(after.mana > before.mana);
    assert_eq!(
        (after.max_life, after.max_mana),
        (before.max_life, before.max_mana)
    );
    assert_eq!(
        vm.host()
            .inner
            .native("GetHeroInt", &[Value::Handle(image), Value::Bool(true)])
            .unwrap(),
        Value::Int(0)
    );
}

#[test]
fn cancelling_separation_clears_protection_then_restores_without_images() {
    let (mut vm, caster) = public_open();
    vm.host()
        .inner
        .native(
            "SetUnitInvulnerable",
            &[Value::Handle(caster), Value::Bool(false)],
        )
        .unwrap();
    assert_eq!(vm.call("mirror_cast", &[]).unwrap(), Value::Bool(true));
    step(&mut vm, 0.3);
    assert_eq!(
        vm.host()
            .inner
            .native("BlzIsUnitInvulnerable", &[Value::Handle(caster)])
            .unwrap(),
        Value::Bool(false)
    );
    let random = vm.host().inner.script_random_state().cloned();
    vm.host()
        .inner
        .native(
            "IssueImmediateOrder",
            &[Value::Handle(caster), Value::Str("beta".into())],
        )
        .unwrap();
    assert_eq!(
        vm.host()
            .inner
            .native("BlzIsUnitInvulnerable", &[Value::Handle(caster)])
            .unwrap(),
        Value::Bool(true)
    );
    step(&mut vm, 1.);
    assert!(vm.host().inner.executed_unit(caster).unwrap().shown);
    assert_eq!(
        vm.host()
            .inner
            .executed_units()
            .filter(|(_, r)| r.illusion.is_some())
            .count(),
        0
    );
    assert_eq!(vm.host().inner.script_random_state(), random.as_ref());
}

#[test]
fn illusion_victim_awards_no_xp_but_ordinary_hero_control_does() {
    let (mut vm, caster) = open();
    let enemy = match vm.global("E") {
        Value::Handle(id) => id,
        _ => panic!("enemy"),
    };
    let h = &mut vm.host().inner;
    h.set_neutral_aggressive(5);
    let owner = h.executed_unit(caster).unwrap().owner;
    h.native(
        "SetPlayerHandicapXP",
        &[Value::Handle(owner), Value::Real(1.)],
    )
    .unwrap();
    let enemy_owner = h.executed_unit(enemy).unwrap().owner;
    let control = match h
        .native(
            "CreateUnit",
            &[
                Value::Handle(enemy_owner),
                Value::Int(i32::from_be_bytes(*b"H001")),
                Value::Real(40.),
                Value::Real(32.),
                Value::Real(0.),
            ],
        )
        .unwrap()
    {
        Value::Handle(id) => id,
        _ => panic!("control"),
    };
    let image = h.test_mirror_image_factory(enemy, *b"AHwe", [80., 80.], [0.5, 2.], true);
    assert!(image.is_err()); // The ordinary control has no learned hero component.
    let image = h
        .test_mirror_image_factory(caster, *b"AHwe", [80., 80.], [0.5, 2.], true)
        .unwrap()
        .unwrap();
    let enemy_owner = h.executed_unit(enemy).unwrap().owner;
    h.native(
        "SetUnitOwner",
        &[
            Value::Handle(image),
            Value::Handle(enemy_owner),
            Value::Bool(false),
        ],
    )
    .unwrap();
    let before = h.native("GetHeroXP", &[Value::Handle(caster)]).unwrap();
    h.kill_unit(image, Some(caster)).unwrap();
    assert_eq!(
        h.native("GetHeroXP", &[Value::Handle(caster)]).unwrap(),
        before
    );
    h.kill_unit(control, Some(caster)).unwrap();
    let (Value::Int(before), Value::Int(after)) = (
        before,
        h.native("GetHeroXP", &[Value::Handle(caster)]).unwrap(),
    ) else {
        panic!("XP")
    };
    assert!(after > before);
}

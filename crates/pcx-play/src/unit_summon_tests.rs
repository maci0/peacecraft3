use super::*;

fn open() -> (Vm, HandleId) {
    let (mut vm, [caster, _, _, _]) = holy_open("", "", "AHwe");
    prepare(&mut vm);
    (vm, caster)
}

fn prepare(vm: &mut Vm) {
    vm.host().inner.admit_water_elemental().unwrap();
    vm.call("learn", &[Value::Int(i32::from_be_bytes(*b"AHwe"))])
        .unwrap();
    vm.call("prime", &[]).unwrap();
}

fn animation(vm: &mut Vm, unit: HandleId) -> String {
    let host = vm.host();
    let animation = host
        .inner
        .executed_unit(unit)
        .unwrap()
        .animation
        .as_ref()
        .unwrap();
    animation.model.sequences[animation.sequence.unwrap()]
        .name
        .clone()
}

#[test]
fn summoning_presents_source_spell_and_birth_then_returns_to_stand() {
    let mut sequences = Vec::new();
    for (name, end) in [("Stand", 1000_u32), ("Spell", 750), ("Birth", 500)] {
        let mut sequence = vec![0; pcx_mdl::SEQUENCE_SIZE];
        sequence[..name.len()].copy_from_slice(name.as_bytes());
        sequence[84..88].copy_from_slice(&end.to_le_bytes());
        sequences.extend(sequence);
    }
    let mut bytes = b"MDLXVERS".to_vec();
    bytes.extend(4_u32.to_le_bytes());
    bytes.extend(800_u32.to_le_bytes());
    bytes.extend(b"SEQS");
    bytes.extend((sequences.len() as u32).to_le_bytes());
    bytes.extend(sequences);
    let model = Arc::new(
        crate::unit_model::Model::read(
            crate::scripts::AssetIdentity {
                source: "synthetic".into(),
                edition: "fixture".into(),
                version: "fixture".into(),
            },
            "summon-fixture.mdx".into(),
            bytes.into(),
            4096,
        )
        .unwrap(),
    );
    let (mut vm, [caster, _, _, _]) =
        holy_open_with_model("", "", "AHwe", HOLY_ART_END_MS, Some(model));
    prepare(&mut vm);
    assert!(cast(&mut vm, caster));
    assert_eq!(animation(&mut vm, caster), "Spell");
    step(&mut vm, 0.25);
    let summoned = vm.host().inner.executed_units().last().unwrap().0;
    assert_eq!(animation(&mut vm, summoned), "Birth");
    step(&mut vm, 0.75);
    assert_eq!(animation(&mut vm, caster), "Stand");
    assert_eq!(animation(&mut vm, summoned), "Stand");
}

fn cast(vm: &mut Vm, caster: HandleId) -> bool {
    vm.host()
        .inner
        .native(
            "IssueImmediateOrder",
            &[Value::Handle(caster), Value::Str("gamma".into())],
        )
        .unwrap()
        == Value::Bool(true)
}

#[test]
fn summoning_rank_reads_its_own_count_unit_cost_and_lifetime() {
    let (mut vm, caster) = open();
    let command = vm
        .host()
        .inner
        .water_elemental_command(caster)
        .unwrap()
        .unwrap();
    assert_eq!((command.hotkey, command.order), ('W', 1631));
    vm.host()
        .inner
        .native(
            "SetHeroLevel",
            &[Value::Handle(caster), Value::Int(3), Value::Bool(false)],
        )
        .unwrap();
    vm.call("learn", &[Value::Int(i32::from_be_bytes(*b"AHwe"))])
        .unwrap();
    let before = vm.host().inner.executed_units().count();
    assert!(cast(&mut vm, caster));
    step(&mut vm, 0.25);
    let created: Vec<_> = vm
        .host()
        .inner
        .executed_units()
        .skip(before)
        .map(|(id, unit)| (id, unit.definition.rawcode))
        .collect();
    assert_eq!(created.len(), 2);
    assert!(created.iter().all(|(_, rawcode)| *rawcode == *b"u002"));
    for &(unit, _) in &created {
        let host = vm.host();
        let caster = host.inner.executed_unit(caster).unwrap();
        let summon = host.inner.executed_unit(unit).unwrap();
        assert_eq!(
            [summon.x, summon.y],
            [
                caster.position.unwrap()[0] + 11.,
                caster.position.unwrap()[1]
            ]
        );
    }
    assert_eq!(holy_mana(&mut vm, caster), 187.);
    step(&mut vm, 6.5);
    for &(unit, _) in &created {
        assert!(
            !vm.host()
                .inner
                .executed_unit(unit)
                .unwrap()
                .combat
                .as_ref()
                .unwrap()
                .dead
        );
    }
    step(&mut vm, 6.75);
    for &(unit, _) in &created {
        assert!(
            vm.host()
                .inner
                .executed_unit(unit)
                .unwrap()
                .combat
                .as_ref()
                .unwrap()
                .dead
        );
    }
}

#[test]
fn summoning_places_the_source_area_in_front_of_each_caster_facing() {
    for (face, delta) in [
        (0., [5., 0.]),
        (90., [0., 5.]),
        (180., [-5., 0.]),
        (270., [0., -5.]),
    ] {
        let (mut vm, caster) = open();
        let position = vm
            .host()
            .inner
            .executed_unit(caster)
            .unwrap()
            .position
            .unwrap();
        vm.host()
            .inner
            .native("SetUnitFacing", &[Value::Handle(caster), Value::Real(face)])
            .unwrap();
        assert!(cast(&mut vm, caster));
        step(&mut vm, 0.25);
        let host = vm.host();
        let (_, summoned) = host.inner.executed_units().last().unwrap();
        assert_eq!(
            [summoned.x, summoned.y],
            [position[0] + delta[0], position[1] + delta[1]]
        );
    }
}

#[test]
fn summoning_uses_the_cast_point_then_spends_mana_and_expires_a_real_unit() {
    let (mut vm, caster) = open();
    let before = vm.host().inner.executed_units().count();
    let mana = holy_mana(&mut vm, caster);
    let ready = vm
        .host()
        .inner
        .water_elemental_readiness(caster)
        .unwrap()
        .unwrap();
    assert_eq!(
        (ready.mana_cost, ready.mana, ready.cooldown_remaining),
        (7., Some(mana), 0.)
    );
    assert!(ready.ready());
    assert!(cast(&mut vm, caster));
    assert_eq!(vm.host().inner.executed_units().count(), before);
    let order = vm
        .host()
        .inner
        .water_elemental_command(caster)
        .unwrap()
        .unwrap()
        .order;
    assert_eq!(
        vm.host()
            .inner
            .native("GetUnitCurrentOrder", &[Value::Handle(caster)])
            .unwrap(),
        Value::Int(order)
    );
    assert_eq!(holy_mana(&mut vm, caster), mana);
    step(&mut vm, 0.125);
    assert_eq!(
        vm.host()
            .inner
            .water_elemental_readiness(caster)
            .unwrap()
            .unwrap()
            .cooldown_remaining,
        0.
    );
    assert_eq!(vm.host().inner.executed_units().count(), before);
    step(&mut vm, 0.25);
    assert_eq!(vm.host().inner.executed_units().count(), before + 1);
    let (summoned, owner, position) = vm
        .host()
        .inner
        .executed_units()
        .last()
        .map(|(id, u)| (id, u.owner, u.position))
        .unwrap();
    assert_eq!(owner, vm.host().inner.executed_unit(caster).unwrap().owner);
    assert!(position.is_some());
    assert!(vm.host().inner.executed_unit(summoned).unwrap().summoned);
    assert_eq!(holy_mana(&mut vm, caster), mana - 7.);
    let cooling = vm
        .host()
        .inner
        .water_elemental_readiness(caster)
        .unwrap()
        .unwrap();
    assert_eq!(cooling.cooldown_remaining, 2.5);
    assert_eq!(cooling.mana, Some(mana - 7.));
    assert!(!cooling.ready());
    assert!(!cast(&mut vm, caster));
    step(&mut vm, 3.5);
    assert!(
        !vm.host()
            .inner
            .executed_unit(summoned)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .dead
    );
    step(&mut vm, 3.75);
    assert!(
        vm.host()
            .inner
            .executed_unit(summoned)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .dead
    );
}

#[test]
fn summoning_pause_preserves_the_remaining_cast_point() {
    let (mut vm, caster) = open();
    let before = vm.host().inner.executed_units().count();
    assert!(cast(&mut vm, caster));
    step(&mut vm, 0.125);
    vm.host()
        .inner
        .native("PauseUnit", &[Value::Handle(caster), Value::Bool(true)])
        .unwrap();
    step(&mut vm, 1.);
    assert_eq!(vm.host().inner.executed_units().count(), before);
    vm.host()
        .inner
        .native("PauseUnit", &[Value::Handle(caster), Value::Bool(false)])
        .unwrap();
    step(&mut vm, 1.0625);
    assert_eq!(vm.host().inner.executed_units().count(), before);
    step(&mut vm, 1.125);
    assert_eq!(vm.host().inner.executed_units().count(), before + 1);
}

#[test]
fn summoning_events_keep_caster_and_created_unit_only_during_delivery() {
    let (mut vm, caster) = open();
    let event = vm.global("EVENT_UNIT_SUMMON");
    vm.call("hero_listen_unit", &[Value::Handle(caster), event])
        .unwrap();
    let owner = vm.host().inner.native("Player", &[Value::Int(4)]).unwrap();
    let event = vm.global("EVENT_PLAYER_UNIT_SUMMON");
    vm.call("hero_listen_player", &[owner, event, Value::Null])
        .unwrap();
    assert!(cast(&mut vm, caster));
    step(&mut vm, 0.25);
    let summoned = vm.host().inner.executed_units().last().unwrap().0;
    for _ in 0..2 {
        let event = vm.host().inner.take_fired_event().unwrap();
        assert!(vm.host().inner.begin_fired_event(&event).unwrap().is_some());
        assert_eq!(
            vm.host().inner.native("GetSummoningUnit", &[]).unwrap(),
            Value::Handle(caster)
        );
        assert_eq!(
            vm.host().inner.native("GetSummonedUnit", &[]).unwrap(),
            Value::Handle(summoned)
        );
        assert_eq!(
            vm.host().inner.native("GetSpellTargetUnit", &[]).unwrap(),
            Value::Null
        );
        vm.host().inner.finish_fired_event();
        assert_eq!(
            vm.host().inner.native("GetSummoningUnit", &[]).unwrap(),
            Value::Null
        );
        assert_eq!(
            vm.host().inner.native("GetSummonedUnit", &[]).unwrap(),
            Value::Null
        );
    }
    assert!(vm.host().inner.take_fired_event().is_none());
}

#[test]
fn summoning_refuses_short_mana_and_replacement_order_cancels_without_spending() {
    let (mut vm, caster) = open();
    let before = vm.host().inner.executed_units().count();
    assert!(cast(&mut vm, caster));
    vm.host()
        .inner
        .native(
            "IssueImmediateOrder",
            &[Value::Handle(caster), Value::Str("beta".into())],
        )
        .unwrap();
    step(&mut vm, 0.5);
    assert_eq!(vm.host().inner.executed_units().count(), before);
    assert_eq!(holy_mana(&mut vm, caster), 200.);
    let mana_state = vm.global("UNIT_STATE_MANA");
    vm.host()
        .inner
        .native(
            "SetUnitState",
            &[Value::Handle(caster), mana_state, Value::Real(6.)],
        )
        .unwrap();
    let short = vm
        .host()
        .inner
        .water_elemental_readiness(caster)
        .unwrap()
        .unwrap();
    assert_eq!(
        (short.mana_cost, short.mana, short.cooldown_remaining),
        (7., Some(6.), 0.)
    );
    assert!(!short.ready());
    assert!(!cast(&mut vm, caster));
    assert_eq!(holy_mana(&mut vm, caster), 6.);
    assert_eq!(vm.host().inner.executed_units().count(), before);
}

use super::*;

const PROFILE: &[u8] = b"[Misc]\nMaxUnitLevel=4\nGrantHeroXP=31,43\nGrantNormalXP=11\nHeroFactorXP=75,25,0\nSummonedKillFactor=0.3\nHeroExpRange=19\nGlobalExperience=1\nBuildingKillsGiveExp=0\nMaxLevelHeroesDrainExp=1\nGrantHeroXPFormulaA=1\nGrantHeroXPFormulaB=3\nGrantHeroXPFormulaC=7\nGrantNormalXPFormulaA=2\nGrantNormalXPFormulaB=3\nGrantNormalXPFormulaC=7\n";
fn fixture() -> (Vm, HandleId, HandleId) {
    fixture_with(b"")
}
fn fixture_with(override_profile: &[u8]) -> (Vm, HandleId, HandleId) {
    let (mut hero, combat) = hero_rules();
    hero.initial_level = 1;
    hero.progression.max_level = 11;
    let mut xp = crate::hero_experience::tests::rules();
    xp.awards = Some((
        pcx_objects::editor::hero_experience_awards::Awards::read(
            &[("synthetic", PROFILE), ("override", override_profile)],
            11,
            4096,
            16,
        )
        .unwrap(),
        0.5,
    ));
    hero.experience = Some(Arc::new(xp));
    let mut fields = hero_fields("STR");
    fields.push(column(
        b"ulev",
        SlkId::UnitBalance,
        "level",
        FieldKind::Int,
        "1",
    ));
    fields.push(column(
        b"ubdg",
        SlkId::UnitBalance,
        "isbldg",
        FieldKind::Bool,
        "0",
    ));
    let (mut vm, a, b) = open_selected(&fields, Some(hero), combat);
    let h = &mut vm.host().inner;
    h.set_neutral_aggressive(5);
    let player = h.native("Player", &[Value::Int(4)]).unwrap();
    h.native("SetPlayerHandicapXP", &[player, Value::Real(0.5)])
        .unwrap();
    (vm, a, b)
}
fn xp(h: &ObjectHost, unit: HandleId) -> i32 {
    h.executed_unit(unit)
        .unwrap()
        .abilities
        .as_ref()
        .unwrap()
        .hero
        .as_ref()
        .unwrap()
        .experience
        .unwrap()
        .xp
}
fn spawn(h: &mut ObjectHost, owner: i32, code: [u8; 4], x: f32) -> HandleId {
    let player = h.native("Player", &[Value::Int(owner)]).unwrap();
    let Value::Handle(unit) = h
        .native(
            "CreateUnit",
            &[
                player,
                Value::Int(i32::from_be_bytes(code)),
                Value::Real(x),
                Value::Real(16.0),
                Value::Real(0.0),
            ],
        )
        .unwrap()
    else {
        panic!("created unit")
    };
    unit
}
#[test]
fn lethal_damage_uses_source_grants_sharing_creep_handicap_and_suspension_once() {
    let (mut vm, a, b) = fixture();
    let h = &mut vm.host().inner;
    h.damage_unit(a, b, 10000.0).unwrap();
    assert_eq!(xp(h, a), 4); // 11 * creep .75 * handicap .5 = 4.125.
    h.damage_unit(a, b, 10000.0).unwrap();
    assert_eq!(xp(h, a), 4);
    h.set_neutral_aggressive(3);
    let second = spawn(h, 4, *b"H001", 18.0);
    let target = spawn(h, 5, *b"u001", 32.0);
    h.damage_unit(a, target, 10000.0).unwrap();
    assert_eq!((xp(h, a), xp(h, second)), (7, 3)); // (11 / 2) * .5 rounds independently.
    h.native("SuspendHeroXP", &[Value::Handle(a), Value::Bool(true)])
        .unwrap();
    let target = spawn(h, 5, *b"u001", 32.0);
    h.damage_unit(a, target, 10000.0).unwrap();
    assert_eq!((xp(h, a), xp(h, second)), (7, 6));
    let target = spawn(h, 4, *b"u001", 32.0);
    h.damage_unit(a, target, 10000.0).unwrap();
    assert_eq!((xp(h, a), xp(h, second)), (7, 6), "owned kills award no XP");
}
#[test]
fn nearby_hero_excludes_far_hero_then_global_fallback_and_missing_handicap_refuse() {
    let (mut vm, a, _) = fixture();
    let h = &mut vm.host().inner;
    h.set_neutral_aggressive(3);
    let second = spawn(h, 4, *b"H001", 80.0);
    let target = spawn(h, 5, *b"u001", 88.0);
    h.damage_unit(a, target, 10000.0).unwrap();
    assert_eq!((xp(h, a), xp(h, second)), (0, 6));
    let target = spawn(h, 5, *b"u001", 120.0);
    h.damage_unit(a, target, 10000.0).unwrap();
    assert_eq!((xp(h, a), xp(h, second)), (3, 9));
    h.kill_unit(a, None).unwrap();
    let target = spawn(h, 5, *b"u001", 88.0);
    h.damage_unit(a, target, 10000.0).unwrap();
    assert_eq!(
        (xp(h, a), xp(h, second)),
        (3, 15),
        "dead heroes are excluded"
    );
    let own = spawn(h, 5, *b"H001", 48.0);
    let target = spawn(h, 4, *b"u001", 56.0);
    assert!(
        h.plan_hero_kill_experience(target, Some(own)).is_err(),
        "unassigned player handicap must not become one"
    );
    assert_eq!(xp(h, own), 0);
}

#[test]
fn source_global_and_maximum_level_drain_policies_change_the_award_set() {
    for (policy, expected) in [
        (b"[Misc]\nMaxLevelHeroesDrainExp=0".as_slice(), 6),
        (b"[Misc]\nMaxLevelHeroesDrainExp=1".as_slice(), 3),
    ] {
        let (mut vm, a, _) = fixture_with(policy);
        let h = &mut vm.host().inner;
        h.set_neutral_aggressive(3);
        let second = spawn(h, 4, *b"H001", 18.0);
        h.native(
            "SetHeroLevel",
            &[Value::Handle(second), Value::Int(11), Value::Bool(false)],
        )
        .unwrap();
        let before = xp(h, second);
        let victim = spawn(h, 5, *b"u001", 32.0);
        h.damage_unit(a, victim, 10000.0).unwrap();
        assert_eq!(xp(h, a), expected);
        assert_eq!(xp(h, second), before);
    }
    let (mut vm, a, _) = fixture_with(b"[Misc]\nGlobalExperience=0");
    let h = &mut vm.host().inner;
    let victim = spawn(h, 5, *b"u001", 120.0);
    h.damage_unit(a, victim, 10000.0).unwrap();
    assert_eq!(xp(h, a), 0);
}

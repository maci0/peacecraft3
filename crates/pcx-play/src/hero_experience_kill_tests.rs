use super::*;

pub(super) const PROFILE: &[u8] = b"[Misc]\nMaxUnitLevel=4\nGrantHeroXP=31,43\nGrantNormalXP=11\nHeroFactorXP=75,25,0\nSummonedKillFactor=0.3\nHeroExpRange=19\nGlobalExperience=1\nBuildingKillsGiveExp=0\nMaxLevelHeroesDrainExp=1\nGrantHeroXPFormulaA=1\nGrantHeroXPFormulaB=3\nGrantHeroXPFormulaC=7\nGrantNormalXPFormulaA=2\nGrantNormalXPFormulaB=3\nGrantNormalXPFormulaC=7\n";
fn fixture() -> (Vm, HandleId, HandleId) {
    fixture_with(b"")
}
fn fixture_with(override_profile: &[u8]) -> (Vm, HandleId, HandleId) {
    fixture_with_level(override_profile, Ok(FieldValue::Cell("1".into())))
}
fn fixture_with_level(
    override_profile: &[u8],
    level: Result<FieldValue, pcx_sim::world::FieldProblem>,
) -> (Vm, HandleId, HandleId) {
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
    fields.last_mut().unwrap().1.value = level;
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

#[test]
fn authored_no_level_victim_dies_without_xp_and_numeric_override_still_awards() {
    for level in ["-", " - ", "0"] {
        let (mut vm, hero, victim) = fixture_with_level(b"", Ok(FieldValue::Cell(level.into())));
        let h = &mut vm.host().inner;
        h.damage_unit(hero, victim, 10000.0).unwrap();
        assert!(
            h.executed_unit(victim)
                .unwrap()
                .combat
                .as_ref()
                .unwrap()
                .dead
        );
        assert_eq!(xp(h, hero), 0, "{level}");
    }
    let (mut vm, hero, victim) =
        fixture_with_level(b"", Ok(FieldValue::Override(pcx_objects::Value::Int(2))));
    let h = &mut vm.host().inner;
    h.damage_unit(hero, victim, 10000.0).unwrap();
    assert_eq!(
        xp(h, hero),
        13,
        "source level override uses the grant recurrence"
    );
}

#[test]
fn absent_malformed_and_unqualified_empty_levels_do_not_become_no_xp() {
    let mut values = ["", "_", "nonsense", "NaN", "-1", "1.5", "2147483648"]
        .map(|text| Ok(FieldValue::Cell(text.into())))
        .to_vec();
    values.push(Err(pcx_sim::world::FieldProblem::Missing));
    values.push(Ok(FieldValue::Override(pcx_objects::Value::Str(
        b"-".to_vec(),
    ))));
    for value in values {
        let (mut vm, hero, victim) = fixture_with_level(b"", value.clone());
        let h = &vm.host().inner;
        assert!(
            h.plan_hero_kill_experience(victim, Some(hero)).is_err(),
            "{value:?}"
        );
        assert_eq!(xp(h, hero), 0);
    }
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

#[test]
#[ignore = "requires PCX_XP_EMPTY_PROBE (stopped original Arm B directory) and PCX_WAR3_EXE"]
fn local_original_empty_level_victim_matches_qualified_no_award() {
    use pcx_objects::editor::defaults::{DefaultTables, Family, Identity};
    use std::path::{Path, PathBuf};
    let directory = PathBuf::from(std::env::var("PCX_XP_EMPTY_PROBE").expect("PCX_XP_EMPTY_PROBE"));
    let read = |path: &Path| {
        let path = if path.is_absolute() {
            path.to_owned()
        } else {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join(path)
        };
        assert!(std::fs::metadata(&path).unwrap().len() <= 32 << 20);
        std::fs::read(path).unwrap()
    };
    let hex = |bytes: &[u8]| {
        crate::neutral_player::sha256(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    let source_bytes = read(&directory.join("source.json"));
    let source: serde_json::Value = serde_json::from_slice(&source_bytes).unwrap();
    let result: serde_json::Value =
        serde_json::from_slice(&read(&directory.join("result.json"))).unwrap();
    let manifest: serde_json::Value =
        serde_json::from_slice(&read(&directory.join("manifest.json"))).unwrap();
    assert_eq!(result["stopped"], true);
    assert_eq!(
        result["source_sha256"].as_str().unwrap(),
        hex(&source_bytes)
    );
    assert_eq!(result["map_sha256"], manifest["map_sha256"]);
    assert_eq!(
        result["map_sha256"].as_str().unwrap(),
        hex(&read(Path::new(manifest["map"].as_str().unwrap())))
    );
    for file in result["files"].as_array().unwrap() {
        assert_eq!(
            file["sha256"].as_str().unwrap(),
            hex(&read(Path::new(file["path"].as_str().unwrap())))
        );
    }
    let table = |suffix: &str| {
        let matches: Vec<_> = source["provenance"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| p["path"].as_str().unwrap().ends_with(suffix))
            .collect();
        assert_eq!(matches.len(), 1);
        let row = matches[0];
        let bytes = read(Path::new(row["path"].as_str().unwrap()));
        assert_eq!(row["sha256"].as_str().unwrap(), hex(&bytes));
        bytes
    };
    let balance = table("War3.mpq-unitbalance.slk");
    let data = table("War3.mpq-unitdata.slk");
    let tables = DefaultTables::from_bytes(
        Identity {
            install: source["edition"].as_str().unwrap().into(),
            edition_version: "selected original Arm B, content SHA checked".into(),
        },
        &[
            ("Units\\UnitBalance.slk", &balance),
            ("Units\\UnitData.slk", &data),
        ],
        &[],
        pcx_objects::editor::EditorReadLimits {
            max_listed_paths: 2,
            max_asset_bytes: 8 << 20,
            max_aggregate_input_bytes: 16 << 20,
        },
    )
    .unwrap();
    let empty = tables
        .object(Family::Unit, source["selected"]["empty"].as_str().unwrap())
        .unwrap();
    let cell = empty.cell(SlkId::UnitBalance, "level").unwrap();
    assert_eq!(cell.text, "-");
    assert_eq!(empty.cell(SlkId::UnitBalance, "isbldg").unwrap().text, "0");
    assert_eq!(result["observation"]["empty_level"], 0);
    assert_eq!(result["observation"]["empty_xp_delta"], 0);
    assert!(result["observation"]["positive_level"].as_i64().unwrap() > 0);
    assert!(result["observation"]["positive_xp_delta"].as_i64().unwrap() > 0);
    let image = read(Path::new(
        &std::env::var("PCX_WAR3_EXE").expect("PCX_WAR3_EXE"),
    ));
    assert_eq!(
        crate::neutral_player::sha256(&image),
        crate::menu_lighting::REVIEWED_IMAGE
    );
    let pe = crate::neutral_rule_input::Pe::parse(
        &image,
        crate::neutral_player::RuleInputBudget {
            image_bytes: 32 << 20,
            profile_bytes: 12,
            sections: 96,
        },
    )
    .unwrap();
    assert_eq!(
        &pe.read_from(0x691147, true).unwrap()[..12],
        &[
            0x85, 0xf6, 0x7f, 8, 0x5e, 0x33, 0xc0, 0x5b, 0x8b, 0xe5, 0x5d, 0xc3
        ],
        "native nonpositive level returns zero"
    );
    let (mut vm, hero, victim) = fixture_with_level(b"", Ok(FieldValue::Cell(cell.text.into())));
    let h = &mut vm.host().inner;
    h.damage_unit(hero, victim, 10000.0).unwrap();
    assert!(
        h.executed_unit(victim)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .dead
    );
    assert_eq!(
        xp(h, hero),
        result["observation"]["empty_xp_delta"].as_i64().unwrap() as i32
    );
}

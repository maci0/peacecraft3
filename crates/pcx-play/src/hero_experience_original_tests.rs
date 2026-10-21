//! Private original-client marker comparison; diagnostic stimuli, not player acceptance.
use pcx_assets::{Addon, AssetMode, Behaviour, Build, ContentSource, GameMode};
use pcx_script::{HandleId, Host, Value};
use std::path::PathBuf;

#[test]
#[ignore = "requires PCX_STARTUP_* and PCX_XP_KILL_OBSERVATIONS from the retained original-client probe"]
fn local_original_kill_experience_matches_recorded_probe() {
    let env = |name| std::env::var(name).unwrap_or_else(|_| panic!("set {name}"));
    let install = PathBuf::from(env("PCX_STARTUP_INSTALL"));
    let map = PathBuf::from(env("PCX_STARTUP_MAP"));
    let bytes = std::fs::read(&map).unwrap();
    let input = std::fs::read(env("PCX_XP_KILL_OBSERVATIONS")).unwrap();
    assert!(input.len() < 64 << 10);
    let receipt: serde_json::Value = serde_json::from_slice(&input).unwrap();
    let hex = |data: &[u8]| {
        crate::neutral_player::sha256(data)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    assert_eq!(receipt["template_sha256"].as_str().unwrap(), hex(&bytes));
    let source = crate::install_source::InstallSource::open(&install).unwrap();
    let misc = source
        .read_bounded("Units\\MiscGame.txt", 8 << 20)
        .unwrap()
        .unwrap();
    assert_eq!(
        receipt["range_profile_sha256"].as_str().unwrap(),
        hex(&misc.bytes)
    );
    let profiles = [(misc.locator.as_str(), misc.bytes.as_slice())];
    let maximum = pcx_objects::editor::hero::Progression::read(&profiles, 8 << 20)
        .unwrap()
        .max_level;
    let range = pcx_objects::editor::hero_experience_awards::Awards::read(
        &profiles,
        maximum,
        8 << 20,
        65536,
    )
    .unwrap()
    .range;
    assert_eq!(env("PCX_STARTUP_ADDON"), "roc");
    let mode = GameMode {
        build: Build::V1_29b,
        addon: Addon::NoAddon,
        assets: AssetMode::Vanilla,
        behaviour: Behaviour::Vanilla,
    };
    let mut startup = crate::headless_startup::start(
        &install,
        mode,
        &map.display().to_string(),
        &bytes,
        env("PCX_STARTUP_SEED").parse().unwrap(),
        None,
    )
    .unwrap();
    let Value::Handle(hero) = startup.session.global("udg_Arthas").unwrap() else {
        panic!("source hero")
    };
    let host = startup.session.host().unwrap();
    let aggressive = host.native("GetPlayerNeutralAggressive", &[]).unwrap();
    let aggressive = host.native("Player", &[aggressive]).unwrap();
    let normal = host.native("Player", &[Value::Int(0)]).unwrap();
    let record = host.objects().executed_unit(hero).unwrap();
    let origin = record.position.unwrap_or([record.x, record.y]);
    let owner = record.owner;
    let mut second = None;
    for (stage, player, dx, dy) in [
        ("normal-near", normal.clone(), range / 4.0, 0.0),
        ("normal-far", normal.clone(), range * 2.0, 0.0),
        ("creep-near", aggressive.clone(), range / 4.0, 0.0),
        ("creep-far", aggressive, range * 2.0, 0.0),
        ("two-near", normal.clone(), range / 4.0, 0.0),
        ("secondary-only-near", normal, range * 2.0, range / 4.0),
    ] {
        let h = host.objects_mut();
        if stage == "two-near" {
            let Value::Handle(id) = h
                .native(
                    "CreateUnit",
                    &[
                        Value::Handle(owner),
                        Value::Int(i32::from_be_bytes(*b"Hart")),
                        Value::Real(origin[0]),
                        Value::Real(origin[1] + range / 8.0),
                        Value::Real(0.0),
                    ],
                )
                .unwrap()
            else {
                panic!("second hero")
            };
            second = Some(id);
        }
        if stage == "secondary-only-near" {
            h.native(
                "SetUnitPosition",
                &[
                    Value::Handle(second.unwrap()),
                    Value::Real(origin[0] + range * 2.0),
                    Value::Real(origin[1]),
                ],
            )
            .unwrap();
        }
        let xp = |h: &crate::playercolor_host::ObjectHost, id: HandleId| {
            h.executed_unit(id)
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
        };
        let before = xp(h, hero);
        let other_before = second.map_or(0, |id| xp(h, id));
        h.native("SuspendHeroXP", &[Value::Handle(hero), Value::Bool(false)])
            .unwrap();
        let Value::Handle(victim) = h
            .native(
                "CreateUnit",
                &[
                    player,
                    Value::Int(i32::from_be_bytes(*b"hfoo")),
                    Value::Real(origin[0] + dx),
                    Value::Real(origin[1] + dy),
                    Value::Real(0.0),
                ],
            )
            .unwrap()
        else {
            panic!("victim")
        };
        let life = h.executed_unit(victim).unwrap().vitals.unwrap().life;
        h.damage_unit(hero, victim, life * 2.0).unwrap();
        assert!(
            h.executed_unit(victim)
                .unwrap()
                .combat
                .as_ref()
                .unwrap()
                .dead
        );
        let primary_delta = xp(h, hero) - before;
        let secondary_delta = second.map_or(0, |id| xp(h, id)) - other_before;
        let observed = &receipt["observed"][stage];
        assert_eq!(
            i64::from(primary_delta),
            observed["primaryDelta"].as_i64().unwrap(),
            "{stage}: primary XP"
        );
        assert_eq!(
            i64::from(secondary_delta),
            observed["secondaryDelta"].as_i64().unwrap(),
            "{stage}: secondary XP"
        );
        eprintln!("XP {stage}: primary={primary_delta} second={secondary_delta}");
        h.native("RemoveUnit", &[Value::Handle(victim)]).unwrap();
    }
}

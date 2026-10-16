use super::{COMMON, budget, context, object, slots_rule};
use crate::playercolor_host::{Failure, ObjectHost, TechLimitBudget};
use pcx_script::{Host, Value};
use std::sync::Arc;

fn declared() -> String {
    COMMON.replace(
        "native SetPlayerOnScoreScreen takes player p, boolean b returns nothing\n",
        "native SetPlayerOnScoreScreen takes player p, boolean b returns nothing\n\
constant native SetPlayerTechMaxAllowed takes player whichPlayer, integer techid, integer maximum returns nothing\n\
constant native GetPlayerTechMaxAllowed takes player whichPlayer, integer techid returns integer\n\
constant native SetPlayerTechResearched takes player whichPlayer, integer techid, integer setToLevel returns nothing\n\
constant native GetPlayerTechResearched takes player whichPlayer, integer techid, boolean specificonly returns boolean\n\
constant native GetPlayerTechCount takes player whichPlayer, integer techid, boolean specificonly returns integer\n\
constant native AddPlayerTechResearched takes player whichPlayer, integer techid, integer levels returns nothing\n",
    )
}

fn admitted(max_records: usize) -> ObjectHost {
    let common = declared();
    let ctx = context(301, &common, "// map", "player-research");
    object(&ctx, 16)
        .with_players(ctx, slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_player_tech_limits(TechLimitBudget { max_records: 1 })
        .unwrap()
        .with_player_tech_researched(TechLimitBudget { max_records })
        .unwrap()
}

fn mint(host: &mut impl Host, number: i32) -> Value {
    host.native("Player", &[Value::Int(number)]).unwrap()
}

fn researched(host: &ObjectHost, player: &Value, techid: i32) -> Option<i32> {
    let Value::Handle(id) = player else {
        panic!("player handle");
    };
    host.player_settings(*id)
        .unwrap()
        .tech_researched
        .iter()
        .find(|(stored, _)| *stored == techid)
        .map(|(_, level)| *level)
}

fn set(host: &mut impl Host, player: &Value, techid: i32, level: i32) -> Result<(), String> {
    host.native(
        "SetPlayerTechResearched",
        &[player.clone(), Value::Int(techid), Value::Int(level)],
    )
    .map(|_| ())
}

#[test]
fn signed_levels_stay_on_the_source_player_and_counts_stay_unsupported() {
    let mut host = admitted(4);
    let first = mint(&mut host, 0);
    let second = mint(&mut host, 1);
    assert_eq!(researched(&host, &first, -7), None);
    set(&mut host, &first, -7, 0).unwrap();
    set(&mut host, &first, 1, -4).unwrap();
    assert_eq!(researched(&host, &first, -7), Some(0));
    assert_eq!(researched(&host, &first, 1), Some(-4));
    assert_eq!(researched(&host, &second, -7), None);
    host.native(
        "SetPlayerTechMaxAllowed",
        &[first.clone(), Value::Int(-7), Value::Int(3)],
    )
    .unwrap();
    assert_eq!(
        host.native("GetPlayerTechMaxAllowed", &[first.clone(), Value::Int(-7)])
            .unwrap(),
        Value::Int(3)
    );
    assert_eq!(researched(&host, &first, -7), Some(0));
    let color = host
        .native("ConvertPlayerColor", &[Value::Int(37)])
        .unwrap();
    let family = set(&mut host, &color, -7, 1).unwrap_err();
    assert!(family.contains("another family"), "{family}");
    for (name, args) in [
        (
            "GetPlayerTechResearched",
            vec![first.clone(), Value::Int(-7), Value::Bool(true)],
        ),
        (
            "GetPlayerTechCount",
            vec![first.clone(), Value::Int(-7), Value::Bool(false)],
        ),
        (
            "AddPlayerTechResearched",
            vec![first.clone(), Value::Int(-7), Value::Int(1)],
        ),
    ] {
        let refused = host.native(name, &args).unwrap_err();
        assert!(refused.contains(name), "{refused}");
        assert!(refused.contains("unsupported"), "{refused}");
        assert_eq!(researched(&host, &first, -7), Some(0));
    }
}

#[test]
fn zero_and_exact_record_budgets_allow_existing_key_updates() {
    let mut empty = admitted(0);
    let one = mint(&mut empty, 0);
    let refused = set(&mut empty, &one, 3, 9).unwrap_err();
    assert!(refused.contains("player tech research"), "{refused}");
    assert_eq!(researched(&empty, &one, 3), None);
    let mut host = admitted(1);
    let first = mint(&mut host, 0);
    let second = mint(&mut host, 1);
    set(&mut host, &first, 3, 9).unwrap();
    set(&mut host, &first, 3, 2).unwrap();
    assert_eq!(researched(&host, &first, 3), Some(2));
    let same_player = set(&mut host, &first, 4, 1).unwrap_err();
    let other_player = set(&mut host, &second, 3, 1).unwrap_err();
    assert!(
        same_player.contains("player tech research"),
        "{same_player}"
    );
    assert!(
        other_player.contains("player tech research"),
        "{other_player}"
    );
    assert_eq!(researched(&host, &first, 3), Some(2));
    assert_eq!(researched(&host, &second, 3), None);
}

#[test]
fn research_admission_checks_players_once_and_the_source_declaration() {
    let common = declared();
    let ctx = context(301, &common, "// map", "player-research-once");
    let host = object(&ctx, 16)
        .with_players(ctx, slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_player_tech_researched(TechLimitBudget { max_records: 1 })
        .unwrap();
    let again = match host.with_player_tech_researched(TechLimitBudget { max_records: 1 }) {
        Err(error) => error,
        Ok(_) => panic!("tech research was admitted twice"),
    };
    assert_eq!(
        again.failure,
        Failure::PlayerState("tech research already admitted")
    );
    let bare = context(301, &common, "// map", "research-before-players");
    let early =
        match object(&bare, 16).with_player_tech_researched(TechLimitBudget { max_records: 1 }) {
            Err(error) => error,
            Ok(_) => panic!("tech research was admitted without players"),
        };
    assert_eq!(early.failure, Failure::PlayerState("not admitted"));
    let missing = context(301, COMMON, "// map", "research-missing");
    let absent = match object(&missing, 16)
        .with_players(missing, slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_player_tech_researched(TechLimitBudget { max_records: 1 })
    {
        Err(error) => error,
        Ok(_) => panic!("missing declaration was admitted"),
    };
    assert_eq!(absent.failure, Failure::Signature);
    let wrong = COMMON.replace(
        "native SetPlayerOnScoreScreen takes player p, boolean b returns nothing\n",
        "native SetPlayerOnScoreScreen takes player p, boolean b returns nothing\n\
native SetPlayerTechResearched takes player whichPlayer, integer techid returns integer\n",
    );
    let mismatched = context(301, &wrong, "// map", "research-signature");
    let signature = match object(&mismatched, 16)
        .with_players(mismatched, slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_player_tech_researched(TechLimitBudget { max_records: 1 })
    {
        Err(error) => error,
        Ok(_) => panic!("wrong declaration was admitted"),
    };
    assert_eq!(signature.failure, Failure::Signature);
}

#[test]
fn argument_shape_refusal_is_the_recreation_contract() {
    let mut host = admitted(1);
    let one = mint(&mut host, 0);
    let error = host
        .native(
            "SetPlayerTechResearched",
            &[one, Value::Real(1.0), Value::Int(1)],
        )
        .unwrap_err();
    assert!(
        error.contains("arguments do not match the player native declaration"),
        "{error}"
    );
    assert!(!error.contains("exception"), "{error}");
}

#[test]
fn owned_vm_keeps_the_executed_pair_without_a_default_level() {
    let map = "globals\nplayer P = Player(0)\nendglobals\nfunction main takes nothing returns nothing\ncall SetPlayerTechResearched(P, -9, 4)\ncall SetPlayerTechMaxAllowed(P, -9, 2)\nendfunction\n";
    let common = declared();
    let ctx = context(301, &common, map, "player-research-vm");
    let host = object(&ctx, 16)
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_player_tech_limits(TechLimitBudget { max_records: 1 })
        .unwrap()
        .with_player_tech_researched(TechLimitBudget { max_records: 1 })
        .unwrap();
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host);
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    let player = vm.global("P");
    assert_eq!(
        vm.host()
            .inner
            .native("GetPlayerTechMaxAllowed", &[player.clone(), Value::Int(-9)])
            .unwrap(),
        Value::Int(2)
    );
    assert_eq!(researched(&vm.host().inner, &player, -9), Some(4));
    assert_eq!(researched(&vm.host().inner, &player, 0), None);
}

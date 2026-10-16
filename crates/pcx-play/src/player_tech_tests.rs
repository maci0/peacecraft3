use super::{COMMON, budget, context, object, slots_rule};
use crate::playercolor_host::TechLimitBudget;
use pcx_script::{Host, Value};

fn tech_common() -> String {
    COMMON.replace(
        "native SetPlayerOnScoreScreen takes player p, boolean b returns nothing\n",
        "native SetPlayerOnScoreScreen takes player p, boolean b returns nothing\n\
constant native SetPlayerTechMaxAllowed takes player whichPlayer, integer techid, integer maximum returns nothing\n\
constant native GetPlayerTechMaxAllowed takes player whichPlayer, integer techid returns integer\n",
    )
}

fn admitted(max_records: usize) -> impl Host {
    let common = tech_common();
    let ctx = context(301, &common, "// map", "player-tech");
    object(&ctx, 16)
        .with_players(ctx, slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_player_tech_limits(TechLimitBudget { max_records })
        .unwrap()
}

fn mint(host: &mut impl Host, number: i32) -> Value {
    host.native("Player", &[Value::Int(number)]).unwrap()
}

fn set(host: &mut impl Host, player: &Value, techid: i32, maximum: i32) -> Result<(), String> {
    host.native(
        "SetPlayerTechMaxAllowed",
        &[player.clone(), Value::Int(techid), Value::Int(maximum)],
    )
    .map(|_| ())
}

#[test]
fn signed_limits_stay_on_the_source_player_and_unset_getter_refuses() {
    let mut host = admitted(4);
    let first = mint(&mut host, 0);
    let second = mint(&mut host, 1);
    let missing = host
        .native("GetPlayerTechMaxAllowed", &[first.clone(), Value::Int(-7)])
        .unwrap_err();
    assert!(missing.contains("tech max is unset"), "{missing}");
    set(&mut host, &first, -7, 0).unwrap();
    set(&mut host, &first, 1, -4).unwrap();
    assert_eq!(
        host.native("GetPlayerTechMaxAllowed", &[first.clone(), Value::Int(-7)])
            .unwrap(),
        Value::Int(0)
    );
    assert_eq!(
        host.native("GetPlayerTechMaxAllowed", &[first.clone(), Value::Int(1)])
            .unwrap(),
        Value::Int(-4)
    );
    let other = host
        .native("GetPlayerTechMaxAllowed", &[second.clone(), Value::Int(-7)])
        .unwrap_err();
    assert!(other.contains("tech max is unset"), "{other}");
    let color = host
        .native("ConvertPlayerColor", &[Value::Int(37)])
        .unwrap();
    let family = host
        .native(
            "SetPlayerTechMaxAllowed",
            &[color, Value::Int(1), Value::Int(1)],
        )
        .unwrap_err();
    assert!(family.contains("another family"), "{family}");
    let kind = host
        .native(
            "SetPlayerTechMaxAllowed",
            &[first.clone(), Value::Real(1.0), Value::Int(1)],
        )
        .unwrap_err();
    assert!(
        kind.contains("arguments do not match the player native declaration"),
        "{kind}"
    );
    let arity = host
        .native(
            "GetPlayerTechMaxAllowed",
            &[first, Value::Int(1), Value::Int(2)],
        )
        .unwrap_err();
    assert!(
        arity.contains("arguments do not match the player native declaration"),
        "{arity}"
    );
}

#[test]
fn zero_and_exact_record_budgets_allow_existing_key_updates() {
    let mut empty = admitted(0);
    let one = mint(&mut empty, 0);
    let refused = set(&mut empty, &one, 3, 9).unwrap_err();
    assert!(refused.contains("player tech limits"), "{refused}");
    let still = empty
        .native("GetPlayerTechMaxAllowed", &[one, Value::Int(3)])
        .unwrap_err();
    assert!(still.contains("tech max is unset"), "{still}");

    let mut host = admitted(1);
    let first = mint(&mut host, 0);
    let second = mint(&mut host, 1);
    set(&mut host, &first, 3, 9).unwrap();
    set(&mut host, &first, 3, 2).unwrap();
    assert_eq!(
        host.native("GetPlayerTechMaxAllowed", &[first.clone(), Value::Int(3)])
            .unwrap(),
        Value::Int(2)
    );
    let full = set(&mut host, &first, 4, 1).unwrap_err();
    assert!(full.contains("player tech limits"), "{full}");
    let other = set(&mut host, &second, 3, 1).unwrap_err();
    assert!(other.contains("player tech limits"), "{other}");
    assert_eq!(
        host.native("GetPlayerTechMaxAllowed", &[first, Value::Int(3)])
            .unwrap(),
        Value::Int(2)
    );
}

#[test]
fn tech_admission_is_once_and_players_must_already_exist() {
    let common = tech_common();
    let ctx = context(301, &common, "// map", "tech-admission");
    let host = object(&ctx, 16)
        .with_players(ctx, slots_rule(301, -3, 9), budget())
        .unwrap();
    let admitted = host
        .with_player_tech_limits(TechLimitBudget { max_records: 1 })
        .unwrap();
    let again = match admitted.with_player_tech_limits(TechLimitBudget { max_records: 1 }) {
        Err(error) => error,
        Ok(_) => panic!("tech limits were admitted twice"),
    };
    assert!(
        again.to_string().contains("tech limits already admitted"),
        "{again}"
    );
    let bare = context(301, &tech_common(), "// map", "tech-before-players");
    let early = match object(&bare, 16).with_player_tech_limits(TechLimitBudget { max_records: 1 })
    {
        Err(error) => error,
        Ok(_) => panic!("tech limits were admitted without players"),
    };
    assert!(early.to_string().contains("not admitted"), "{early}");
    let mut plain = super::host();
    let one = mint(&mut plain, 0);
    let unsupported = plain
        .native(
            "SetPlayerTechMaxAllowed",
            &[one, Value::Int(1), Value::Int(1)],
        )
        .unwrap_err();
    assert!(
        unsupported.contains("SetPlayerTechMaxAllowed"),
        "{unsupported}"
    );
    assert!(
        unsupported.contains("not implemented") || unsupported.contains("unsupported"),
        "{unsupported}"
    );
}

#[test]
fn argument_shape_refusal_is_the_recreation_contract() {
    let mut host = admitted(2);
    let one = mint(&mut host, 0);
    let error = host
        .native("SetPlayerTechMaxAllowed", &[one, Value::Int(1)])
        .unwrap_err();
    assert!(
        error.contains("arguments do not match the player native declaration"),
        "{error}"
    );
    assert!(
        !error.contains("exception"),
        "refusal must not claim a native exceptional input: {error}"
    );
}

use super::*;
const DECL: &str = "constant native SetPlayerHandicapXP takes player p, real factor returns nothing\nconstant native GetPlayerHandicapXP takes player p returns real\n";
fn open(common: &str, map: &str) -> (ObjectHost, Arc<ParsedMapContext>) {
    let ctx = context(300, common, map, "handicap-map");
    let (rules, numeric) = crate::player_handicap::tests::rules();
    let h = object(&ctx, 16)
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_script_numeric(numeric)
        .with_player_experience_handicap(rules)
        .unwrap();
    (h, ctx)
}
#[test]
fn loaded_vm_assignment_and_query_use_the_same_player_rows_without_defaults() {
    let map = r#"
globals
player P = null
player ALIAS = null
real RESULT = 0.0
endglobals
function setup takes nothing returns nothing
set P = Player(4)
set ALIAS = P
endfunction
function run takes nothing returns nothing
call SetPlayerHandicapXP(ALIAS, 1.5)
set RESULT = GetPlayerHandicapXP(P)
endfunction
function integer_factor takes nothing returns nothing
call SetPlayerHandicapXP(P, 41)
set RESULT = GetPlayerHandicapXP(ALIAS)
endfunction
"#;
    let (h, ctx) = open(&format!("{COMMON}{DECL}"), map);
    let expected_literal = h.real_ops().unwrap().literal("1.5").unwrap();
    assert_ne!(expected_literal, 1.5); // The synthetic reciprocal table differs from host arithmetic.
    let mut v = Arc::clone(&ctx.scripts).owned_vm(h);
    v.init_globals().unwrap();
    v.call("setup", &[]).unwrap();
    let p = v.global("P");
    let handles = v.host().inner.inner().handles();
    // An unassigned player holds the factor the source reset installs.
    assert_eq!(
        v.host()
            .inner
            .native("GetPlayerHandicapXP", std::slice::from_ref(&p))
            .unwrap(),
        Value::Real(3.0)
    );
    v.call("run", &[]).unwrap();
    assert_eq!(v.global("RESULT"), Value::Real(expected_literal));
    assert_eq!(v.global("ALIAS"), p);
    v.call("integer_factor", &[]).unwrap();
    assert_eq!(v.global("RESULT"), Value::Real(37.0));
    let Value::Handle(id) = p else {
        panic!("player")
    };
    assert_eq!(
        v.host()
            .inner
            .player_settings(id)
            .unwrap()
            .experience_handicap,
        Some(37.0)
    );
    assert_eq!(v.host().inner.inner().handles(), handles);
}
#[test]
fn bounds_special_values_and_null_handling_follow_selected_rules() {
    let (mut h, _) = open(&format!("{COMMON}{DECL}"), "// no globals");
    let a = player(&mut h, 4);
    let b = player(&mut h, 5);
    for (value, expected) in [
        (-1.0, 0.375_f32),
        (0.5, 0.5),
        (100.0, 37.0),
        (f32::INFINITY, 37.0),
        (f32::NEG_INFINITY, 0.375),
    ] {
        h.native("SetPlayerHandicapXP", &[a.clone(), Value::Real(value)])
            .unwrap();
        assert_eq!(
            h.native("GetPlayerHandicapXP", std::slice::from_ref(&a))
                .unwrap(),
            Value::Real(expected)
        );
    }
    let bits = 0x7fc0_0123;
    h.native(
        "SetPlayerHandicapXP",
        &[a.clone(), Value::Real(f32::from_bits(bits))],
    )
    .unwrap();
    let Value::Real(value) = h
        .native("GetPlayerHandicapXP", std::slice::from_ref(&a))
        .unwrap()
    else {
        panic!("real")
    };
    assert_eq!(value.to_bits(), bits);
    h.native("SetPlayerHandicapXP", &[Value::Null, Value::Real(1.5)])
        .unwrap();
    assert_eq!(
        h.native("GetPlayerHandicapXP", &[Value::Null]).unwrap(),
        Value::Real(3.0)
    );
    assert_eq!(
        h.native("GetPlayerHandicapXP", &[b]).unwrap(),
        Value::Real(3.0)
    );
}
#[test]
fn invalid_handles_arguments_and_repeated_admission_do_not_change_factor() {
    let (mut h, _) = open(&format!("{COMMON}{DECL}"), "// no globals");
    let p = player(&mut h, 4);
    let t = h.native("CreateTimer", &[]).unwrap();
    h.native("SetPlayerHandicapXP", &[p.clone(), Value::Real(1.5)])
        .unwrap();
    for args in [
        vec![],
        vec![p.clone()],
        vec![p.clone(), Value::Null],
        vec![p.clone(), Value::Bool(true)],
        vec![t.clone(), Value::Real(2.0)],
    ] {
        assert!(h.native("SetPlayerHandicapXP", &args).is_err());
    }
    for args in [vec![], vec![p.clone(), Value::Real(2.0)], vec![t]] {
        assert!(h.native("GetPlayerHandicapXP", &args).is_err());
    }
    assert_eq!(
        h.native("GetPlayerHandicapXP", &[p]).unwrap(),
        Value::Real(1.5)
    );
    let (rules, _) = crate::player_handicap::tests::rules();
    assert!(h.with_player_experience_handicap(rules).is_err());
}
#[test]
fn contracts_sources_and_numeric_policy_are_required_before_admission() {
    for decl in [
        DECL.replace("constant native SetPlayer", "native SetPlayer"),
        DECL.replace("real factor", "integer factor"),
        DECL.replace("returns real", "returns integer"),
    ] {
        let ctx = context(300, &format!("{COMMON}{decl}"), "// empty", "handicap-map");
        let (rules, numeric) = crate::player_handicap::tests::rules();
        let h = object(&ctx, 16)
            .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
            .unwrap()
            .with_script_numeric(numeric);
        assert!(
            matches!(h.with_player_experience_handicap(rules),Err(e) if e.failure==Failure::Signature)
        );
    }
    let ctx = context(300, &format!("{COMMON}{DECL}"), "// empty", "handicap-map");
    let h = object(&ctx, 16)
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap();
    let (rules, _) = crate::player_handicap::tests::rules();
    assert!(h.with_player_experience_handicap(rules).is_err());
}
#[test]
fn omitted_policy_keeps_both_declared_natives_unsupported() {
    let ctx = context(300, &format!("{COMMON}{DECL}"), "// empty", "handicap-map");
    let mut h = object(&ctx, 16)
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap();
    let p = player(&mut h, 4);
    for (name, args) in [
        ("SetPlayerHandicapXP", vec![p.clone(), Value::Real(0.5)]),
        ("GetPlayerHandicapXP", vec![p]),
    ] {
        assert!(h.native(name, &args).unwrap_err().contains("unsupported"));
    }
}

#[test]
fn each_foreign_declaration_and_a_mismatched_numeric_image_refuse() {
    let setter = DECL.lines().next().unwrap();
    let getter = DECL.lines().nth(1).unwrap();
    for (common, map, name) in [
        (COMMON.to_owned(), DECL.to_owned(), "SetPlayerHandicapXP"),
        (
            format!("{COMMON}{setter}\n"),
            format!("{getter}\n"),
            "GetPlayerHandicapXP",
        ),
    ] {
        let ctx = context(300, &common, &map, "handicap-map");
        let (rules, numeric) = crate::player_handicap::tests::rules();
        let h = object(&ctx, 16)
            .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
            .unwrap()
            .with_script_numeric(numeric);
        let e = match h.with_player_experience_handicap(rules) {
            Err(e) => e,
            Ok(_) => panic!("foreign declaration admitted"),
        };
        assert_eq!(e.native, name);
        assert_eq!(e.failure, Failure::Source);
    }
    let ctx = context(300, &format!("{COMMON}{DECL}"), "// empty", "handicap-map");
    let (rules, _) = crate::player_handicap::tests::rules();
    let (mut image, _, mut numeric, source_budget) = crate::player_handicap::tests::fixture();
    image[0x1700] ^= 1;
    numeric.image_sha256 = crate::neutral_player::sha256(&image);
    let numeric =
        crate::script_numeric::NumericPolicy::read(&image, numeric, source_budget, 32).unwrap();
    let h = object(&ctx, 16)
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_script_numeric(numeric);
    assert!(h.with_player_experience_handicap(rules).is_err());
}
#[test]
fn rows_created_before_admission_receive_the_reset_factor() {
    let ctx = context(
        300,
        &format!("{COMMON}{DECL}"),
        "// no globals",
        "handicap-map",
    );
    let (rules, numeric) = crate::player_handicap::tests::rules();
    let mut h = object(&ctx, 16)
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_script_numeric(numeric);
    let early = player(&mut h, 4);
    let mut h = h.with_player_experience_handicap(rules).unwrap();
    let late = player(&mut h, 5);
    for p in [early, late] {
        assert_eq!(
            h.native("GetPlayerHandicapXP", &[p]).unwrap(),
            Value::Real(3.0)
        );
    }
}

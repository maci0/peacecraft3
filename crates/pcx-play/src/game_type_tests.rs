use super::{COMMON, budget, context, slots_rule};
use crate::playercolor_host::{EnumHost, Limits, ObjectHost};
use pcx_script::{Host, Value};
const GAME_TYPE: &str = "type gametype extends handle
constant native ConvertGameType takes integer i returns gametype
native GetGameTypeSelected takes nothing returns gametype
globals
constant gametype GAME_TYPE_MELEE = ConvertGameType(1)
constant gametype GAME_TYPE_USE_MAP_SETTINGS = ConvertGameType(4)
endglobals
";
fn host(declared: bool) -> ObjectHost {
    let common = if declared {
        format!("{COMMON}\n{GAME_TYPE}")
    } else {
        COMMON.to_string()
    };
    let ctx = context(
        301,
        &common,
        "function idle takes nothing returns nothing\nendfunction\n",
        "synthetic:game-type",
    );
    let mut names = vec![
        "ConvertPlacement",
        "ConvertPlayerColor",
        "ConvertRacePref",
        "ConvertMapControl",
    ];
    if declared {
        names.push("ConvertGameType");
    }
    let inner = EnumHost::checked_selected(
        ctx.scripts.shared_program(),
        "synthetic:game-type",
        &names,
        Limits {
            max_handles: 32,
            admission_steps: 1000,
            expression_depth: 16,
            expression_work: 1000,
        },
    )
    .unwrap();
    ObjectHost::checked(inner)
        .unwrap()
        .with_players(ctx, slots_rule(301, -3, 9), budget())
        .unwrap()
}
#[test]
fn selected_game_type_is_the_admitted_constant_and_no_other() {
    let mut h = host(true).with_game_type(1).unwrap();
    let melee = h.native("ConvertGameType", &[Value::Int(1)]).unwrap();
    assert_eq!(h.native("GetGameTypeSelected", &[]).unwrap(), melee);
    let map_settings = h.native("ConvertGameType", &[Value::Int(4)]).unwrap();
    assert_ne!(h.native("GetGameTypeSelected", &[]).unwrap(), map_settings);
    assert!(h.native("GetGameTypeSelected", &[Value::Int(1)]).is_err());
}
#[test]
fn a_game_type_outside_the_constants_fails_at_the_call() {
    let mut h = host(true).with_game_type(0x100).unwrap();
    let error = h.native("GetGameTypeSelected", &[]).unwrap_err();
    assert!(error.contains("256"), "{error}");
}
#[test]
fn game_type_is_unsupported_until_admitted_and_admits_once() {
    let mut h = host(true);
    assert!(h.native("GetGameTypeSelected", &[]).is_err());
    let h = h.with_game_type(1).unwrap();
    assert!(h.with_game_type(1).is_err());
}
#[test]
fn game_type_admission_needs_the_declarations() {
    assert!(host(false).with_game_type(1).is_err());
}

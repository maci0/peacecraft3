use super::{context, context_with_wts, rules, slots_rule};
use crate::map_admission::ParsedMapContext;
use crate::neutral_player::{ContextProblem, NeutralError, RuleProblem};
use crate::playercolor_host::{
    EnumHost, Failure, Limits, MapConfigBudget, ObjectHost, PlayerAdmissionError, PlayerBudget,
};
use pcx_script::{HandleId, Host, Value};
use std::sync::Arc;

#[path = "player_local_tests.rs"]
mod local_player;
#[path = "message_host_tests.rs"]
mod messages;

const COMMON: &str = "type agent extends handle\ntype player extends agent\ntype timer extends agent\ntype placement extends handle\ntype playercolor extends handle\ntype racepreference extends handle\ntype mapcontrol extends handle\nconstant native GetBJMaxPlayerSlots takes nothing returns integer\nnative CreateTimer takes nothing returns timer\nnative DestroyTimer takes timer t returns nothing\nconstant native ConvertPlacement takes integer i returns placement\nconstant native ConvertPlayerColor takes integer i returns playercolor\nconstant native ConvertRacePref takes integer i returns racepreference\nconstant native ConvertMapControl takes integer i returns mapcontrol\nnative SetMapName takes string name returns nothing\nnative SetMapDescription takes string description returns nothing\nnative SetPlayers takes integer n returns nothing\nnative SetTeams takes integer n returns nothing\nnative GetPlayers takes nothing returns integer\nnative GetTeams takes nothing returns integer\nnative SetGamePlacement takes placement p returns nothing\nconstant native GetGamePlacement takes nothing returns placement\nnative DefineStartLocation takes integer i, real x, real y returns nothing\nconstant native GetStartLocationX takes integer i returns real\nconstant native GetStartLocationY takes integer i returns real\nconstant native Player takes integer number returns player\nconstant native GetPlayerId takes player p returns integer\nnative SetPlayerTeam takes player p, integer team returns nothing\nnative GetPlayerTeam takes player p returns integer\nnative SetPlayerStartLocation takes player p, integer index returns nothing\nnative ForcePlayerStartLocation takes player p, integer index returns nothing\nnative GetPlayerStartLocation takes player p returns integer\nnative SetPlayerColor takes player p, playercolor c returns nothing\nnative GetPlayerColor takes player p returns playercolor\nnative SetPlayerRacePreference takes player p, racepreference r returns nothing\nnative SetPlayerRaceSelectable takes player p, boolean b returns nothing\nnative GetPlayerSelectable takes player p returns boolean\nnative SetPlayerController takes player p, mapcontrol c returns nothing\nnative GetPlayerController takes player p returns mapcontrol\nnative SetPlayerName takes player p, string name returns nothing\nnative GetPlayerName takes player p returns string\nnative SetPlayerOnScoreScreen takes player p, boolean b returns nothing\nglobals\nconstant placement P = ConvertPlacement(7)\nconstant playercolor C = ConvertPlayerColor(37)\nconstant racepreference R = ConvertRacePref(-5)\nconstant mapcontrol M = ConvertMapControl(91)\nendglobals\n";
fn enums(ctx: &ParsedMapContext, handles: usize) -> EnumHost {
    EnumHost::checked_selected(
        ctx.scripts.shared_program(),
        &ctx.map_digest.label,
        &[
            "ConvertPlacement",
            "ConvertPlayerColor",
            "ConvertRacePref",
            "ConvertMapControl",
        ],
        Limits {
            max_handles: handles,
            admission_steps: 1000,
            expression_depth: 16,
            expression_work: 1000,
        },
    )
    .unwrap()
}
fn object(ctx: &ParsedMapContext, handles: usize) -> ObjectHost {
    ObjectHost::checked(enums(ctx, handles))
        .unwrap()
        .with_map_config(MapConfigBudget {
            text_bytes: 128,
            start_locations: 2,
        })
        .unwrap()
}
fn budget() -> PlayerBudget {
    PlayerBudget {
        max_slots: 12,
        name_bytes: 6,
    }
}
fn host() -> ObjectHost {
    let ctx = context(300, COMMON, "// no globals", "synthetic:players");
    object(&ctx, 16)
        .with_players(ctx, slots_rule(301, -3, 9), budget())
        .unwrap()
}
fn player(h: &mut ObjectHost, number: i32) -> Value {
    h.native("Player", &[Value::Int(number)]).unwrap()
}
fn id(v: &Value) -> HandleId {
    let Value::Handle(id) = v else {
        panic!("player handle")
    };
    *id
}
#[test]
fn slot_domain_is_derived_from_the_approved_rule_and_selected_word() {
    for (word, count) in [(300, 6), (301, 9), (u32::MAX, 9)] {
        let ctx = context(word, COMMON, "// map", "slot-word");
        let mut h = object(&ctx, 16)
            .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
            .unwrap();
        assert_eq!(h.player_slots(), Some(count));
        assert!(Arc::ptr_eq(h.player_context().unwrap(), &ctx));
        let last = player(&mut h, count as i32 - 1);
        assert_eq!(
            h.native("GetPlayerId", std::slice::from_ref(&last))
                .unwrap(),
            Value::Int(count as i32 - 1)
        );
        let handles = h.inner().handles();
        for outside in [-1, count as i32, i32::MAX] {
            assert!(h.native("Player", &[Value::Int(outside)]).is_err());
        }
        assert_eq!(h.inner().handles(), handles);
    }
}
#[test]
fn repeated_player_calls_alias_one_identity_distinct_from_other_arena_families() {
    let mut h = host();
    let a = player(&mut h, 2);
    let b = player(&mut h, 4);
    let color = h.native("ConvertPlayerColor", &[Value::Int(37)]).unwrap();
    let timer = h.native("CreateTimer", &[]).unwrap();
    assert_eq!(player(&mut h, 2), a);
    assert_ne!(a, b);
    assert_ne!(a, color);
    assert_ne!(a, timer);
    assert_eq!(h.player_settings(id(&a)).unwrap().number, 2);
    assert_eq!(h.inner().kind(id(&a)).unwrap(), "player");
    assert!(h.native("GetPlayerId", &[timer]).is_err());
    assert!(h.native("GetPlayerId", &[color]).is_err());
}
#[test]
fn no_player_settings_are_fabricated_before_their_authored_assignments() {
    let mut h = host();
    let p = player(&mut h, 0);
    for native in [
        "GetPlayerTeam",
        "GetPlayerStartLocation",
        "GetPlayerColor",
        "GetPlayerSelectable",
        "GetPlayerController",
        "GetPlayerName",
    ] {
        assert!(
            h.native(native, std::slice::from_ref(&p))
                .unwrap_err()
                .contains("unset")
        );
    }
    let settings = h.player_settings(id(&p)).unwrap();
    assert!(settings.race_preference.is_none() && settings.on_score_screen.is_none());
    assert!(!settings.start_is_forced);
}
#[test]
fn typed_symbolic_settings_roundtrip_without_clamping_or_cross_family_casts() {
    let mut h = host();
    let p = player(&mut h, 0);
    let color = h.native("ConvertPlayerColor", &[Value::Int(37)]).unwrap();
    let race = h.native("ConvertRacePref", &[Value::Int(-5)]).unwrap();
    let control = h.native("ConvertMapControl", &[Value::Int(91)]).unwrap();
    for (native, value) in [
        ("SetPlayerColor", color.clone()),
        ("SetPlayerRacePreference", race.clone()),
        ("SetPlayerController", control.clone()),
    ] {
        assert_eq!(h.native(native, &[p.clone(), value]).unwrap(), Value::Null);
    }
    assert_eq!(
        h.native("GetPlayerColor", std::slice::from_ref(&p))
            .unwrap(),
        color
    );
    assert_eq!(
        h.native("GetPlayerController", std::slice::from_ref(&p))
            .unwrap(),
        control
    );
    assert!(h.native("SetPlayerColor", &[p.clone(), race]).is_err());
    assert_eq!(
        h.native("GetPlayerColor", std::slice::from_ref(&p))
            .unwrap(),
        color
    );
    h.native("SetPlayerTeam", &[p.clone(), Value::Int(8)])
        .unwrap();
    assert_eq!(
        h.native("GetPlayerTeam", std::slice::from_ref(&p)).unwrap(),
        Value::Int(8)
    );
    assert!(
        h.native("SetPlayerTeam", &[p.clone(), Value::Int(-1)])
            .is_err()
    );
    h.native("SetPlayerRaceSelectable", &[p.clone(), Value::Bool(false)])
        .unwrap();
    h.native("SetPlayerOnScoreScreen", &[p.clone(), Value::Bool(true)])
        .unwrap();
    assert_eq!(
        h.native("GetPlayerSelectable", std::slice::from_ref(&p))
            .unwrap(),
        Value::Bool(false)
    );
    assert_eq!(
        h.player_settings(id(&p)).unwrap().on_score_screen,
        Some(true)
    );
}
#[test]
fn starts_require_defined_coordinates_and_force_marks_occupancy_without_moving_units() {
    let mut h = host();
    let p = player(&mut h, 0);
    assert!(
        h.native("SetPlayerStartLocation", &[p.clone(), Value::Int(7)])
            .is_err()
    );
    h.native(
        "DefineStartLocation",
        &[Value::Int(7), Value::Real(-0.), Value::Real(8.25)],
    )
    .unwrap();
    h.native("SetPlayerStartLocation", &[p.clone(), Value::Int(7)])
        .unwrap();
    assert!(!h.start_location_occupied(7).unwrap());
    h.native("ForcePlayerStartLocation", &[p.clone(), Value::Int(7)])
        .unwrap();
    assert!(h.start_location_occupied(7).unwrap());
    assert_eq!(
        h.native("GetPlayerStartLocation", std::slice::from_ref(&p))
            .unwrap(),
        Value::Int(7)
    );
    assert!(
        h.native("ForcePlayerStartLocation", &[p.clone(), Value::Int(9)])
            .is_err()
    );
    assert!(h.start_location_occupied(7).unwrap());
    assert!(h.start_location_occupied(9).is_err());
    assert_eq!(
        h.map_config().unwrap().start_locations[0].x.to_bits(),
        (-0.0f32).to_bits()
    );
}
#[test]
fn occupancy_refuses_an_unadmitted_player_domain_or_undefined_start() {
    let ctx = context(300, COMMON, "// map", "occupancy");
    assert!(object(&ctx, 16).start_location_occupied(7).is_err());
    assert!(host().start_location_occupied(7).is_err());
}

#[test]
fn name_budget_is_aggregate_utf8_and_failed_replacements_preserve_both_players() {
    let mut h = host();
    let a = player(&mut h, 0);
    let b = player(&mut h, 1);
    h.native("SetPlayerName", &[a.clone(), Value::Str("éé".into())])
        .unwrap();
    h.native("SetPlayerName", &[b.clone(), Value::Str("ok".into())])
        .unwrap();
    assert!(
        h.native("SetPlayerName", &[a.clone(), Value::Str("abcde".into())])
            .is_err()
    );
    assert_eq!(
        h.native("GetPlayerName", std::slice::from_ref(&a)).unwrap(),
        Value::Str("éé".into())
    );
    assert_eq!(
        h.native("GetPlayerName", std::slice::from_ref(&b)).unwrap(),
        Value::Str("ok".into())
    );
    h.native("SetPlayerName", &[a, Value::Str(String::new())])
        .unwrap();
    h.native("SetPlayerName", &[b.clone(), Value::Str("abcdef".into())])
        .unwrap();
    assert_eq!(
        h.native("GetPlayerName", &[b]).unwrap(),
        Value::Str("abcdef".into())
    );
}
#[test]
fn native_arguments_and_unowned_or_unknown_player_handles_refuse() {
    let ctx = context(300, COMMON, "// map", "unowned-player");
    let mut e = enums(&ctx, 16);
    let reserved = Value::Handle(e.reserve_identity("player").unwrap());
    let mut h = ObjectHost::checked(e)
        .unwrap()
        .with_players(ctx, slots_rule(301, -3, 9), budget())
        .unwrap();
    assert!(h.native("GetPlayerId", &[reserved]).is_err());
    let p = player(&mut h, 0);
    for args in [
        vec![],
        vec![Value::Null],
        vec![Value::Int(0)],
        vec![p.clone(), Value::Int(0)],
    ] {
        assert!(h.native("GetPlayerId", &args).is_err());
    }
    for args in [
        vec![],
        vec![Value::Real(0.)],
        vec![Value::Int(0), Value::Int(1)],
    ] {
        assert!(h.native("Player", &args).is_err());
    }
    let unknown = Value::Handle(HandleId::new(std::num::NonZeroU64::new(u64::MAX).unwrap()));
    assert!(
        h.native("SetPlayerTeam", &[unknown, Value::Int(0)])
            .is_err()
    );
    assert!(
        h.native("GetPlayerSlotState", &[p])
            .unwrap_err()
            .contains("unsupported")
    );
}
#[test]
fn handle_capacity_failure_does_not_orphan_or_replace_player_state() {
    let ctx = context(300, COMMON, "// map", "capacity");
    let mut h = object(&ctx, 5)
        .with_players(ctx, slots_rule(301, -3, 9), budget())
        .unwrap();
    for (native, value) in [
        ("ConvertPlacement", 7),
        ("ConvertPlayerColor", 37),
        ("ConvertRacePref", -5),
        ("ConvertMapControl", 91),
    ] {
        h.native(native, &[Value::Int(value)]).unwrap();
    }
    let a = player(&mut h, 0);
    assert!(h.native("Player", &[Value::Int(1)]).is_err());
    assert_eq!(player(&mut h, 0), a);
    assert_eq!(h.inner().handles(), 5);
    assert_eq!(h.player_settings(id(&a)).unwrap().number, 0);
}
#[test]
fn different_program_changed_metadata_missing_witness_or_wrong_rule_are_not_admitted() {
    let ctx = context(300, COMMON, "// A", "context-A");
    let other = context(300, COMMON, "// B", "context-B");
    assert!(
        object(&ctx, 16)
            .with_players(other, slots_rule(301, -3, 9), budget())
            .is_err()
    );
    assert!(matches!(
        object(&ctx, 16).with_players(Arc::clone(&ctx), rules(), budget()),
        Err(PlayerAdmissionError::Source(NeutralError::Rule(
            RuleProblem::Binding
        )))
    ));
    let mut changed = context(300, COMMON, "// map", "changed-metadata");
    Arc::get_mut(&mut changed).unwrap().world.info.constant_b = Some(301);
    assert!(matches!(
        object(&changed, 16).with_players(changed, slots_rule(301, -3, 9), budget()),
        Err(PlayerAdmissionError::Source(NeutralError::Context {
            problem: ContextProblem::ChangedInfo,
            ..
        }))
    ));
    let mut missing = context(300, COMMON, "// map", "missing-witness");
    Arc::get_mut(&mut missing).unwrap().admission_witness = None;
    assert!(matches!(
        object(&missing, 16).with_players(missing, slots_rule(301, -3, 9), budget()),
        Err(PlayerAdmissionError::Source(NeutralError::Context {
            problem: ContextProblem::MissingAdmission,
            ..
        }))
    ));
}
#[test]
fn slot_and_name_limits_are_checked_before_domain_or_state_allocation() {
    let ctx = context(300, COMMON, "// map", "limits");
    assert!(
        object(&ctx, 16)
            .with_players(
                Arc::clone(&ctx),
                slots_rule(301, -3, 9),
                PlayerBudget {
                    max_slots: 5,
                    name_bytes: 6
                }
            )
            .is_err()
    );
    assert!(
        object(&ctx, 16)
            .with_players(
                Arc::clone(&ctx),
                slots_rule(301, -3, 9),
                PlayerBudget {
                    max_slots: 12,
                    name_bytes: usize::MAX
                }
            )
            .is_err()
    );
    assert!(
        object(&ctx, 16)
            .with_players(Arc::clone(&ctx), slots_rule(301, -3, -9), budget())
            .is_err()
    );
    let mut empty = object(&ctx, 16)
        .with_players(ctx, slots_rule(301, 0, 0), budget())
        .unwrap();
    assert_eq!(empty.player_slots(), Some(0));
    assert!(empty.native("Player", &[Value::Int(0)]).is_err());
    assert_eq!(empty.inner().handles(), 0);
}
#[test]
fn exact_player_signatures_and_script_function_overrides_are_required() {
    for common in [
        COMMON.replace("constant native Player", "native Player"),
        COMMON.replace(
            "GetPlayerId takes player p returns integer",
            "GetPlayerId takes player p returns real",
        ),
        COMMON.replace(
            "SetPlayerColor takes player p, playercolor c",
            "SetPlayerColor takes player p, racepreference c",
        ),
        COMMON.replace("type player extends agent", "type player extends handle"),
    ] {
        let ctx = context(300, &common, "// map", "bad-player-contract");
        assert!(
            object(&ctx, 16)
                .with_players(ctx, slots_rule(301, -3, 9), budget())
                .is_err()
        );
    }
    let ctx = context(
        300,
        COMMON,
        "function Player takes integer n returns player\nreturn null\nendfunction\n",
        "overridden-player",
    );
    assert!(
        object(&ctx, 16)
            .with_players(ctx, slots_rule(301, -3, 9), budget())
            .is_err()
    );
}

#[test]
fn owned_vm_retains_selected_context_localization_and_player_globals_across_calls() {
    let map = "globals\nplayer A = Player(5)\nplayer B = Player(5)\nendglobals\nfunction config takes nothing returns nothing\ncall SetPlayerName(A, \"TRIGSTR_9\")\ncall SetPlayerTeam(A, 2)\nendfunction\n";
    let ctx = context_with_wts(
        300,
        COMMON,
        map,
        "localized-player",
        Some(b"STRING 9\r\n{\r\nName\r\n}\r\n"),
    );
    let mut retained = Arc::clone(&ctx);
    let host = object(&ctx, 16)
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap();
    assert!(Arc::get_mut(&mut retained).is_none());
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host);
    assert!(Arc::ptr_eq(vm.host().inner.player_context().unwrap(), &ctx));
    drop(ctx);
    drop(retained);
    vm.init_globals().unwrap();
    assert_eq!(vm.global("A"), vm.global("B"));
    vm.call("config", &[]).unwrap();
    let p = vm.global("A");
    assert_eq!(
        vm.host()
            .inner
            .native("GetPlayerName", std::slice::from_ref(&p))
            .unwrap(),
        Value::Str("Name".into())
    );
    assert_eq!(
        vm.host()
            .inner
            .native("GetPlayerTeam", std::slice::from_ref(&p))
            .unwrap(),
        Value::Int(2)
    );
    let handles = vm.host().inner.inner().handles();
    vm.init_globals().unwrap();
    assert_eq!(vm.host().inner.inner().handles(), handles);
}

#[test]
fn a_map_cannot_ambiguously_redeclare_player_in_another_source() {
    let ctx = context(
        300,
        COMMON,
        "constant native Player takes integer number returns player\n",
        "ambiguous-player",
    );
    assert!(
        matches!(object(&ctx, 16).with_players(ctx, slots_rule(301, -3, 9), budget()),
        Err(PlayerAdmissionError::Contract(e)) if e.failure == Failure::Source)
    );
}

#[path = "player_tech_tests.rs"]
mod player_tech;

#[path = "player_research_tests.rs"]
mod player_research;

#[path = "destructable_host_tests.rs"]
mod destructable_creation;

#[path = "unit_host_tests.rs"]
mod unit_creation;

#[path = "trigger_host_tests.rs"]
mod trigger_admission;

#[path = "player_alliance_tests.rs"]
mod player_alliance;

#[path = "player_state_request_tests.rs"]
mod player_state_requests;

#[path = "force_host_tests.rs"]
mod forces;

#[path = "startup_settings_tests.rs"]
mod startup_settings;

#[path = "player_slots_tests.rs"]
mod slot_states;

#[path = "player_controllers_tests.rs"]
mod current_controllers;

#[path = "player_current_research_tests.rs"]
mod current_research;

#[path = "stock_host_tests.rs"]
mod stock_slots;

#[path = "terrain_pathing_host_tests.rs"]
mod terrain_pathing;

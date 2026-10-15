use crate::playercolor_host::{EnumHost, Failure, Limits, MapConfigBudget, ObjectHost};
use crate::scripts::{AssetIdentity, MapScripts, ScriptAsset};
use pcx_script::{Host, Prelude, Value};
use std::sync::Arc;

pub(super) const COMMON: &str = "type agent extends handle\ntype timer extends agent\ntype placement extends handle\nnative CreateTimer takes nothing returns timer\nnative DestroyTimer takes timer t returns nothing\nconstant native ConvertPlacement takes integer i returns placement\nnative SetMapName takes string name returns nothing\nnative SetMapDescription takes string description returns nothing\nnative SetPlayers takes integer n returns nothing\nnative SetTeams takes integer n returns nothing\nnative GetPlayers takes nothing returns integer\nnative GetTeams takes nothing returns integer\nnative SetGamePlacement takes placement p returns nothing\nconstant native GetGamePlacement takes nothing returns placement\nnative DefineStartLocation takes integer i, real x, real y returns nothing\nconstant native GetStartLocationX takes integer i returns real\nconstant native GetStartLocationY takes integer i returns real\nglobals\nconstant placement A = ConvertPlacement(-7)\nconstant placement B = ConvertPlacement(31)\nendglobals\n";
fn object(program: Arc<pcx_script::Program>) -> ObjectHost {
    let e = EnumHost::checked_selected(
        program,
        "synthetic:config-map",
        &["ConvertPlacement"],
        Limits {
            max_handles: 8,
            admission_steps: 100,
            expression_depth: 16,
            expression_work: 100,
        },
    )
    .unwrap();
    ObjectHost::checked(e).unwrap()
}
fn program(common: &str, map: &str) -> Arc<pcx_script::Program> {
    let common = pcx_script::parse_script(common).unwrap();
    let map = pcx_script::parse_script(map).unwrap();
    Arc::new(pcx_script::resolve_with(&map, &[Prelude::new("synthetic:common", &common)]).unwrap())
}
fn host(text: usize, starts: usize) -> ObjectHost {
    object(program(COMMON, ""))
        .with_map_config(MapConfigBudget {
            text_bytes: text,
            start_locations: starts,
        })
        .unwrap()
}
#[test]
fn settings_require_admission_and_unassigned_reads_never_invent_defaults() {
    let mut disabled = object(program(COMMON, ""));
    assert!(disabled.map_config().is_none());
    assert!(
        disabled
            .native("SetMapName", &[Value::Str("name".into())])
            .unwrap_err()
            .contains("unsupported")
    );
    let mut h = host(0, 0);
    let state = h.map_config().unwrap();
    assert!(
        state.name.is_none()
            && state.description.is_none()
            && state.players.is_none()
            && state.teams.is_none()
            && state.placement.is_none()
    );
    for name in ["GetPlayers", "GetTeams", "GetGamePlacement"] {
        assert!(h.native(name, &[]).unwrap_err().contains("unset"));
    }
    assert!(h.native("GetStartLocationX", &[Value::Int(2)]).is_err());
    h.native("SetMapName", &[Value::Str(String::new())])
        .unwrap();
    assert_eq!(h.map_config().unwrap().name.as_deref(), Some(""));
    assert!(
        h.with_map_config(MapConfigBudget {
            text_bytes: 1,
            start_locations: 1
        })
        .is_err()
    );
}
#[test]
fn combined_utf8_text_budget_replacement_is_checked_before_mutation() {
    let mut h = host(6, 0);
    h.native("SetMapName", &[Value::Str("éé".into())]).unwrap();
    h.native("SetMapDescription", &[Value::Str("ok".into())])
        .unwrap();
    assert!(
        h.native("SetMapName", &[Value::Str("abcde".into())])
            .unwrap_err()
            .contains("map config text")
    );
    assert_eq!(h.map_config().unwrap().name.as_deref(), Some("éé"));
    assert_eq!(h.map_config().unwrap().description.as_deref(), Some("ok"));
    h.native("SetMapName", &[Value::Str(String::new())])
        .unwrap();
    h.native("SetMapDescription", &[Value::Str("abcdef".into())])
        .unwrap();
    assert_eq!(
        h.map_config().unwrap().description.as_deref(),
        Some("abcdef")
    );
    assert!(h.native("SetMapDescription", &[Value::Null]).is_err());
    assert_eq!(
        h.map_config().unwrap().description.as_deref(),
        Some("abcdef")
    );
}
#[test]
fn counts_preserve_script_values_and_negative_inputs_do_not_overwrite_them() {
    let mut h = host(0, 0);
    for (setter, getter, value) in [
        ("SetPlayers", "GetPlayers", 0),
        ("SetTeams", "GetTeams", i32::MAX),
    ] {
        h.native(setter, &[Value::Int(value)]).unwrap();
        assert_eq!(h.native(getter, &[]).unwrap(), Value::Int(value));
        assert!(h.native(setter, &[Value::Int(-1)]).is_err());
        assert!(h.native(setter, &[Value::Real(1.)]).is_err());
        assert!(h.native(getter, &[Value::Int(1)]).is_err());
        assert_eq!(h.native(getter, &[]).unwrap(), Value::Int(value));
    }
}
#[test]
fn sparse_start_locations_preserve_float_bits_and_existing_rows_at_capacity() {
    let mut h = host(0, 1);
    h.native(
        "DefineStartLocation",
        &[Value::Int(9), Value::Real(-0.), Value::Int(7)],
    )
    .unwrap();
    let Value::Real(x) = h.native("GetStartLocationX", &[Value::Int(9)]).unwrap() else {
        panic!("real coordinate")
    };
    assert_eq!(x.to_bits(), (-0.0f32).to_bits());
    assert_eq!(
        h.native("GetStartLocationY", &[Value::Int(9)]).unwrap(),
        Value::Real(7.)
    );
    assert!(
        h.native(
            "DefineStartLocation",
            &[Value::Int(10), Value::Real(1.), Value::Real(2.)]
        )
        .is_err()
    );
    h.native(
        "DefineStartLocation",
        &[Value::Int(9), Value::Real(-3.25), Value::Real(4.5)],
    )
    .unwrap();
    assert_eq!(h.map_config().unwrap().start_locations.len(), 1);
    for args in [
        vec![Value::Int(-1), Value::Real(0.), Value::Real(0.)],
        vec![Value::Int(9), Value::Real(f32::NAN), Value::Real(0.)],
        vec![Value::Int(9), Value::Real(0.), Value::Real(f32::INFINITY)],
        vec![Value::Int(9), Value::Bool(false), Value::Real(0.)],
    ] {
        assert!(h.native("DefineStartLocation", &args).is_err());
    }
    assert_eq!(
        h.native("GetStartLocationX", &[Value::Int(9)]).unwrap(),
        Value::Real(-3.25)
    );
    assert_eq!(
        h.native("GetStartLocationY", &[Value::Int(9)]).unwrap(),
        Value::Real(4.5)
    );
    assert!(h.native("GetStartLocationX", &[Value::Int(10)]).is_err());
}
#[test]
fn placement_reuses_an_admitted_symbolic_identity_and_refuses_an_object_family() {
    let mut h = host(0, 0);
    let p = h.native("ConvertPlacement", &[Value::Int(31)]).unwrap();
    h.native("SetGamePlacement", std::slice::from_ref(&p))
        .unwrap();
    assert_eq!(h.native("GetGamePlacement", &[]).unwrap(), p);
    let timer = h.native("CreateTimer", &[]).unwrap();
    assert!(h.native("SetGamePlacement", &[timer]).is_err());
    assert!(h.native("SetGamePlacement", &[Value::Null]).is_err());
    assert_eq!(h.native("GetGamePlacement", &[]).unwrap(), p);
}
#[test]
fn layout_and_zero_record_budgets_refuse_before_start_location_insertion() {
    for budget in [
        MapConfigBudget {
            text_bytes: usize::MAX,
            start_locations: 0,
        },
        MapConfigBudget {
            text_bytes: 0,
            start_locations: usize::MAX,
        },
    ] {
        assert!(
            matches!(object(program(COMMON, "")).with_map_config(budget), Err(e) if e.failure == Failure::Capacity)
        );
    }
    let mut h = host(0, 0);
    assert!(
        h.native(
            "DefineStartLocation",
            &[Value::Int(0), Value::Real(0.), Value::Real(0.)]
        )
        .is_err()
    );
    assert!(h.map_config().unwrap().start_locations.is_empty());
}
#[test]
fn mismatched_native_signatures_and_function_overrides_refuse_installation() {
    for common in [
        COMMON.replace(
            "SetMapName takes string name",
            "SetMapName takes integer name",
        ),
        COMMON.replace("native GetPlayers", "constant native GetPlayers"),
        COMMON.replace(
            "constant native GetGamePlacement",
            "native GetGamePlacement",
        ),
        COMMON.replace(
            "GetStartLocationX takes integer i returns real",
            "GetStartLocationX takes integer i returns integer",
        ),
    ] {
        assert!(
            object(program(&common, ""))
                .with_map_config(MapConfigBudget {
                    text_bytes: 8,
                    start_locations: 2
                })
                .is_err()
        );
    }
    let map = "function SetMapName takes string name returns nothing\nendfunction\n";
    assert!(
        object(program(COMMON, map))
            .with_map_config(MapConfigBudget {
                text_bytes: 8,
                start_locations: 2
            })
            .is_err()
    );
}
fn scripts(wts: &[u8]) -> Arc<MapScripts> {
    Arc::new(MapScripts::compile(
        AssetIdentity {edition: "synthetic".into(), version: "fixture".into(), source: "config-map".into()},
        ScriptAsset {label: "fixture/common.j", bytes: COMMON.as_bytes()},
        ScriptAsset {label: "fixture/Blizzard.j", bytes: b"// no helpers"},
        ScriptAsset {label: "fixture/war3map.j", bytes: b"function config takes nothing returns nothing\ncall SetMapName(\"TRIGSTR_7\")\ncall SetMapDescription(\"TRIGSTR_9\")\ncall SetPlayers(3)\ncall SetTeams(2)\ncall SetGamePlacement(B)\ncall DefineStartLocation(5, -12.25, 7.5)\nendfunction\n"},
        Some(ScriptAsset {label: "fixture/war3map.wts", bytes: wts}),
    ).unwrap())
}
#[test]
fn owned_same_program_config_consumes_retained_localization_and_preserves_other_objects() {
    let scripts = scripts(
        b"STRING 7\r\n{\r\nLocalized name\r\n}\r\nSTRING 9\r\n{\r\nLocalized description\r\n}\r\n",
    );
    let program = scripts.shared_program();
    let mut host = object(Arc::clone(&program))
        .with_map_config(MapConfigBudget {
            text_bytes: 64,
            start_locations: 2,
        })
        .unwrap();
    let timer = host.native("CreateTimer", &[]).unwrap();
    let mut vm = Arc::clone(&scripts).owned_vm(host);
    assert!(std::ptr::eq(vm.program(), &*program));
    assert!(Arc::ptr_eq(vm.host().context(), &scripts));
    drop(scripts);
    vm.init_globals().unwrap();
    vm.call("config", &[]).unwrap();
    let config = vm.host().inner.map_config().unwrap();
    assert_eq!(config.name.as_deref(), Some("Localized name"));
    assert_eq!(config.description.as_deref(), Some("Localized description"));
    assert_eq!(config.players, Some(3));
    assert_eq!(config.teams, Some(2));
    assert_eq!(config.start_locations[0].index, 5);
    assert_eq!(config.start_locations[0].x, -12.25);
    assert_eq!(config.start_locations[0].y, 7.5);
    assert_eq!(vm.host().inner.live_timers(), 1);
    assert_ne!(vm.global("B"), timer);
    let handles = vm.host().inner.inner().handles();
    vm.init_globals().unwrap();
    assert_eq!(vm.host().inner.inner().handles(), handles);
}
#[test]
fn malformed_present_wts_is_a_contextual_failure_without_fabricated_description() {
    let scripts = scripts(b"STRING 7\r\n{\r\nGood\r\n}\r\nSTRING 9\r\n{\r\n\xff\r\n}\r\n");
    let host = object(scripts.shared_program())
        .with_map_config(MapConfigBudget {
            text_bytes: 64,
            start_locations: 2,
        })
        .unwrap();
    let mut vm = Arc::clone(&scripts).owned_vm(host);
    vm.init_globals().unwrap();
    let error = vm.call("config", &[]).unwrap_err();
    let source = vm.error_source().map(str::to_owned);
    let diagnostic = vm.host().diagnostic(source.as_deref(), &error);
    assert!(
        diagnostic.contains("war3map.j")
            && diagnostic.contains("war3map.wts")
            && diagnostic.contains("UTF-8")
    );
    let config = vm.host().inner.map_config().unwrap();
    assert_eq!(config.name.as_deref(), Some("Good"));
    assert!(config.description.is_none());
}

#[test]
fn another_prelude_cannot_ambiguously_redeclare_an_admitted_config_native() {
    let common = pcx_script::parse_script(COMMON).unwrap();
    let other =
        pcx_script::parse_script("native SetMapName takes string name returns nothing\n").unwrap();
    let map = pcx_script::parse_script("").unwrap();
    let program = Arc::new(
        pcx_script::resolve_with(
            &map,
            &[
                Prelude::new("synthetic:common", &common),
                Prelude::new("synthetic:other", &other),
            ],
        )
        .unwrap(),
    );
    assert!(
        matches!(object(program).with_map_config(MapConfigBudget {text_bytes: 8, start_locations: 2}), Err(e) if e.failure == Failure::Source)
    );
}

#[test]
fn platform_layout_limits_are_checked_without_allocating_the_config_budget() {
    for budget in [
        MapConfigBudget {
            text_bytes: u32::MAX as usize / 2 + 1,
            start_locations: 0,
        },
        MapConfigBudget {
            text_bytes: 0,
            start_locations: (u32::MAX as usize / 2) / std::mem::size_of::<super::StartLocation>()
                + 1,
        },
    ] {
        let result = object(program(COMMON, "")).with_map_config(budget);
        assert_eq!(result.is_err(), usize::BITS == 32);
    }
}

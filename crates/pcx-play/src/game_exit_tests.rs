use super::*;
use crate::playercolor_host::{EnumHost, Limits};
use pcx_script::{Host, Prelude, Vm};
use std::sync::Arc;

const BASE: &str = "type agent extends handle\ntype timer extends agent\nnative CreateTimer takes nothing returns timer\nnative DestroyTimer takes timer t returns nothing\n";
const DECLARATION: &str = "native EndGame takes boolean doScoreScreen returns nothing\n";
const CHANGE_LEVEL_DECLARATION: &str =
    "native ChangeLevel takes string newLevel, boolean doScoreScreen returns nothing\n";
const NEXT_LEVEL_DECLARATION: &str = "native SetNextLevel takes string map returns nothing\n";

fn change_level_program(map: &str) -> Arc<pcx_script::Program> {
    program(
        &format!("{DECLARATION}{CHANGE_LEVEL_DECLARATION}{NEXT_LEVEL_DECLARATION}"),
        "",
        map,
    )
}

fn program(declaration: &str, other: &str, map: &str) -> Arc<pcx_script::Program> {
    let common = pcx_script::parse_script(&format!("{BASE}{declaration}")).unwrap();
    let other = pcx_script::parse_script(other).unwrap();
    let map = pcx_script::parse_script(map).unwrap();
    Arc::new(
        pcx_script::resolve_with(
            &map,
            &[
                Prelude::new("synthetic:common", &common),
                Prelude::new("synthetic:other", &other),
            ],
        )
        .unwrap(),
    )
}

fn bare(program: Arc<pcx_script::Program>) -> ObjectHost {
    let enums = EnumHost::checked_selected(
        program,
        "synthetic:map",
        &[],
        Limits {
            max_handles: 8,
            admission_steps: 1024,
            expression_depth: 16,
            expression_work: 1024,
        },
    )
    .unwrap();
    ObjectHost::checked(enums).unwrap()
}

fn host(program: Arc<pcx_script::Program>, max_pending: usize) -> ObjectHost {
    bare(program).with_game_exit(max_pending).unwrap()
}

#[path = "campaign_progress_host_tests.rs"]
mod progress;

#[test]
fn admission_is_explicit_and_starts_empty() {
    let h = ObjectHost::checked(
        EnumHost::checked_selected(
            program(DECLARATION, "", ""),
            "synthetic:map",
            &[],
            Limits {
                max_handles: 8,
                admission_steps: 1024,
                expression_depth: 16,
                expression_work: 1024,
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert!(!h.game_exit_admitted());
    assert!(h.game_exit_requests().is_empty());
    let h = h.with_game_exit(4).unwrap();
    assert!(h.game_exit_admitted());
    assert!(h.game_exit_requests().is_empty());
    assert!(
        host(program(DECLARATION, "", ""), 4)
            .with_game_exit(4)
            .is_err()
    );
    let h = host(program("", "", ""), 4);
    assert!(!h.game_exit_admitted());
}

#[test]
fn vm_records_request_and_the_calling_thread_continues() {
    let p = program(
        DECLARATION,
        "",
        r#"
globals
integer progress = 0
endglobals
function quit takes nothing returns integer
call EndGame(false)
set progress = progress + 5
return progress
endfunction
"#,
    );
    let mut vm = Vm::new(&p, host(p.clone(), 4));
    vm.init_globals().unwrap();
    assert_eq!(vm.call("quit", &[]).unwrap(), Value::Int(5));
    assert_eq!(
        vm.host().game_exit_requests(),
        &[GameExitRequest {
            score_screen: false,
            next_level: None,
        }]
    );
    assert_eq!(
        vm.host().take_game_exit_requests(),
        vec![GameExitRequest {
            score_screen: false,
            next_level: None,
        }]
    );
    assert!(vm.host().game_exit_requests().is_empty());
}

#[test]
fn invalid_arguments_and_overflow_preserve_recorded_requests() {
    let p = program(DECLARATION, "", "");
    let mut h = host(p.clone(), 1);
    for args in [
        vec![],
        vec![Value::Int(1)],
        vec![Value::Bool(true), Value::Bool(true)],
    ] {
        assert!(h.native(END_GAME, &args).is_err());
    }
    assert!(h.game_exit_requests().is_empty());
    assert_eq!(
        h.native(END_GAME, &[Value::Bool(true)]).unwrap(),
        Value::Null
    );
    assert!(h.native(END_GAME, &[Value::Bool(false)]).is_err());
    assert_eq!(
        h.game_exit_requests(),
        &[GameExitRequest {
            score_screen: true,
            next_level: None,
        }]
    );
    assert!(
        h.game_exit_native("PauseGame", &[Value::Bool(true)])
            .is_none()
    );
}

#[test]
fn incompatible_signatures_and_foreign_sources_refuse_admission() {
    for declaration in [
        "native EndGame takes integer flag returns nothing\n",
        "native EndGame takes boolean flag returns boolean\n",
        "constant native EndGame takes boolean flag returns nothing\n",
        "native EndGame takes nothing returns nothing\n",
        "native ChangeLevel takes string map returns nothing\n",
        "native ChangeLevel takes string map, integer score returns nothing\n",
        "native ChangeLevel takes string map, boolean score returns boolean\n",
        "constant native ChangeLevel takes string map, boolean score returns nothing\n",
    ] {
        assert!(
            ObjectHost::checked(
                EnumHost::checked_selected(
                    program(declaration, "", ""),
                    "synthetic:map",
                    &[],
                    Limits {
                        max_handles: 8,
                        admission_steps: 1024,
                        expression_depth: 16,
                        expression_work: 1024,
                    },
                )
                .unwrap(),
            )
            .unwrap()
            .with_game_exit(4)
            .is_err()
        );
    }
    for declaration in [DECLARATION, CHANGE_LEVEL_DECLARATION] {
        assert_eq!(
            ObjectHost::checked(
                EnumHost::checked_selected(
                    program("", declaration, ""),
                    "synthetic:map",
                    &[],
                    Limits {
                        max_handles: 8,
                        admission_steps: 1024,
                        expression_depth: 16,
                        expression_work: 1024,
                    },
                )
                .unwrap(),
            )
            .unwrap()
            .with_game_exit(4)
            .err()
            .unwrap()
            .failure,
            Failure::Source
        );
    }
}

#[test]
fn change_level_records_the_target_with_its_request_and_the_vm_continues() {
    let p = change_level_program(
        r#"
globals
integer progress = 0
endglobals
function run takes nothing returns integer
call ChangeLevel("Maps\\Campaign\\Human02.w3m", true)
set progress = progress + 5
return progress
endfunction
"#,
    );
    let mut vm = Vm::new(&p, host(p.clone(), 4));
    vm.init_globals().unwrap();
    assert_eq!(vm.call("run", &[]).unwrap(), Value::Int(5));
    assert_eq!(
        vm.host().game_exit_requests(),
        &[GameExitRequest {
            score_screen: true,
            next_level: Some("Maps\\Campaign\\Human02.w3m".into()),
        }]
    );
}

#[test]
fn each_request_keeps_its_own_target_and_ambient_next_level_is_separate() {
    let p = change_level_program("");
    let mut h = host(p, 4);
    h.native(
        NEXT_LEVEL,
        &[Value::Str("Maps\\Campaign\\Ambient.w3m".into())],
    )
    .unwrap();
    h.native(
        CHANGE_LEVEL,
        &[
            Value::Str("Maps\\Campaign\\First.w3m".into()),
            Value::Bool(false),
        ],
    )
    .unwrap();
    h.native(END_GAME, &[Value::Bool(true)]).unwrap();
    h.native(
        CHANGE_LEVEL,
        &[
            Value::Str("Maps\\Campaign\\Second.w3m".into()),
            Value::Bool(false),
        ],
    )
    .unwrap();
    assert_eq!(h.next_level(), Some("Maps\\Campaign\\Ambient.w3m"));
    assert_eq!(
        h.game_exit_requests(),
        &[
            GameExitRequest {
                score_screen: false,
                next_level: Some("Maps\\Campaign\\First.w3m".into()),
            },
            GameExitRequest {
                score_screen: true,
                next_level: None,
            },
            GameExitRequest {
                score_screen: false,
                next_level: Some("Maps\\Campaign\\Second.w3m".into()),
            },
        ]
    );
    h.native(NEXT_LEVEL, &[Value::Str(String::new())]).unwrap();
    assert_eq!(h.next_level(), None);
    assert_eq!(h.game_exit_requests().len(), 3);
}

#[test]
fn change_level_argument_byte_and_queue_failures_are_atomic() {
    let p = change_level_program("");
    let mut h = host(p.clone(), 2);
    for args in [
        vec![],
        vec![Value::Str("map".into())],
        vec![Value::Bool(true), Value::Str("map".into())],
        vec![
            Value::Str("map".into()),
            Value::Bool(true),
            Value::Bool(false),
        ],
    ] {
        assert!(h.native(CHANGE_LEVEL, &args).is_err());
    }
    assert!(h.game_exit_requests().is_empty());
    h.native(
        CHANGE_LEVEL,
        &[Value::Str("kept".into()), Value::Bool(false)],
    )
    .unwrap();
    let kept = h.game_exit_requests().to_vec();
    assert!(
        h.native(
            CHANGE_LEVEL,
            &[Value::Str("bad\0map".into()), Value::Bool(false)]
        )
        .is_err()
    );
    let oversized = "x".repeat(crate::campaign_progress::MAX_BYTES + 1);
    assert!(
        h.native(CHANGE_LEVEL, &[Value::Str(oversized), Value::Bool(false)])
            .is_err()
    );
    assert_eq!(h.game_exit_requests(), kept.as_slice());

    let mut full = host(p, 1);
    full.native(
        CHANGE_LEVEL,
        &[Value::Str("first".into()), Value::Bool(false)],
    )
    .unwrap();
    let before = full.game_exit_requests().to_vec();
    assert!(
        full.native(
            CHANGE_LEVEL,
            &[Value::Str("second".into()), Value::Bool(false)]
        )
        .is_err()
    );
    assert!(full.native(END_GAME, &[Value::Bool(true)]).is_err());
    assert_eq!(full.game_exit_requests(), before.as_slice());
}

#[test]
#[ignore = "requires PEACECRAFT3_COMMON_J naming the selected Scripts\\common.j"]
fn selected_common_change_level_contract_is_admitted() {
    let path = std::env::var("PEACECRAFT3_COMMON_J").expect("explicit common.j path");
    let common = pcx_script::parse_script(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let map = pcx_script::parse_script("").unwrap();
    let program =
        Arc::new(pcx_script::resolve_with(&map, &[Prelude::new(&path, &common)]).unwrap());
    let mut h = ObjectHost::checked(
        EnumHost::checked_selected(
            program,
            &path,
            &[],
            Limits {
                max_handles: 8,
                admission_steps: 100_000,
                expression_depth: 64,
                expression_work: 100_000,
            },
        )
        .unwrap(),
    )
    .unwrap()
    .with_game_exit(2)
    .unwrap();
    h.native(
        CHANGE_LEVEL,
        &[
            Value::Str("Maps\\Campaign\\Human02.w3m".into()),
            Value::Bool(false),
        ],
    )
    .unwrap();
    assert_eq!(
        h.game_exit_requests(),
        &[GameExitRequest {
            score_screen: false,
            next_level: Some("Maps\\Campaign\\Human02.w3m".into()),
        }]
    );
}

use super::*;
use crate::playercolor_host::{EnumHost, Limits};
use pcx_script::{Host, Prelude, Vm};
use std::sync::Arc;

const BASE: &str = "type agent extends handle\ntype timer extends agent\nnative CreateTimer takes nothing returns timer\nnative DestroyTimer takes timer t returns nothing\n";
const DECLARATION: &str = "native EndGame takes boolean doScoreScreen returns nothing\n";

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

fn host(program: Arc<pcx_script::Program>, max_pending: usize) -> ObjectHost {
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
    ObjectHost::checked(enums)
        .unwrap()
        .with_game_exit(max_pending)
        .unwrap()
}

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
            score_screen: false
        }]
    );
    assert_eq!(
        vm.host().take_game_exit_requests(),
        vec![GameExitRequest {
            score_screen: false
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
        &[GameExitRequest { score_screen: true }]
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
    assert_eq!(
        ObjectHost::checked(
            EnumHost::checked_selected(
                program("", DECLARATION, ""),
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

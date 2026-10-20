use super::*;
use crate::playercolor_host::{EnumHost, Limits};
use pcx_script::{Host, Prelude, Vm};
use std::sync::Arc;

const BASE: &str = "type agent extends handle\ntype timer extends agent\nnative CreateTimer takes nothing returns timer\nnative DestroyTimer takes timer t returns nothing\n";
const DECLARATION: &str = "native PauseGame takes boolean flag returns nothing\n";

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

fn host(program: Arc<pcx_script::Program>) -> ObjectHost {
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

#[test]
fn admission_and_executed_state_are_distinct_without_a_default_flag() {
    let h = host(program(DECLARATION, "", ""));
    assert!(!h.game_pause_admitted());
    assert_eq!(h.game_paused(), None);
    let h = h.with_game_pause().unwrap();
    assert!(h.game_pause_admitted());
    assert_eq!(h.game_paused(), None);
    assert!(h.with_game_pause().is_err());
    let h = host(program("", "", "")).with_game_pause().unwrap();
    assert!(!h.game_pause_admitted());
    assert_eq!(h.game_paused(), None);
}

#[test]
fn vm_completes_pause_thread_and_accepts_later_resume() {
    let p = program(
        DECLARATION,
        "",
        r#"
globals
integer progress = 0
endglobals
function pauseAndContinue takes nothing returns integer
call PauseGame(true)
set progress = 17
return progress
endfunction
function resumeAndContinue takes nothing returns integer
call PauseGame(false)
set progress = progress + 9
return progress
endfunction
"#,
    );
    let h = host(p.clone()).with_game_pause().unwrap();
    let mut vm = Vm::new(&p, h);
    vm.init_globals().unwrap();
    assert_eq!(vm.call("pauseAndContinue", &[]).unwrap(), Value::Int(17));
    assert_eq!(vm.host().game_paused(), Some(true));
    assert_eq!(vm.call("pauseAndContinue", &[]).unwrap(), Value::Int(17));
    assert_eq!(vm.host().game_paused(), Some(true));
    assert_eq!(vm.call("resumeAndContinue", &[]).unwrap(), Value::Int(26));
    assert_eq!(vm.host().game_paused(), Some(false));
}

#[test]
fn invalid_arguments_and_unadmitted_calls_preserve_state() {
    let p = program(DECLARATION, "", "");
    let mut h = host(p.clone());
    assert!(h.native(PAUSE, &[Value::Bool(true)]).is_err());
    assert_eq!(h.game_paused(), None);
    let mut h = host(p).with_game_pause().unwrap();
    for paused in [true, false] {
        assert_eq!(
            h.native(PAUSE, &[Value::Bool(paused)]).unwrap(),
            Value::Null
        );
        for args in [
            vec![],
            vec![Value::Int(1)],
            vec![Value::Bool(false), Value::Bool(true)],
        ] {
            assert!(h.native(PAUSE, &args).is_err());
            assert_eq!(h.game_paused(), Some(paused));
        }
    }
    assert!(
        h.game_pause_native("PauseTimer", &[Value::Bool(true)])
            .is_none()
    );
    assert_eq!(h.game_paused(), Some(false));
}

#[test]
fn incompatible_signatures_and_foreign_sources_refuse_admission() {
    for declaration in [
        "native PauseGame takes integer flag returns nothing\n",
        "native PauseGame takes boolean flag returns boolean\n",
        "constant native PauseGame takes boolean flag returns nothing\n",
        "native PauseGame takes nothing returns nothing\n",
    ] {
        assert!(
            host(program(declaration, "", ""))
                .with_game_pause()
                .is_err()
        );
    }
    assert_eq!(
        host(program("", DECLARATION, ""))
            .with_game_pause()
            .err()
            .unwrap()
            .failure,
        Failure::Source
    );
}

#[test]
#[ignore = "requires explicitly supplied selected common.j"]
fn original_pause_declaration_admits_boolean_requests() {
    let source = std::env::var("PCX_GAME_PAUSE_COMMON").expect("PCX_GAME_PAUSE_COMMON");
    let common = pcx_script::parse_script(&std::fs::read_to_string(&source).unwrap()).unwrap();
    let map = pcx_script::parse_script("").unwrap();
    let p = Arc::new(pcx_script::resolve_with(&map, &[Prelude::new(&source, &common)]).unwrap());
    let mut h = host(p).with_game_pause().unwrap();
    assert!(h.game_pause_admitted());
    assert_eq!(h.game_paused(), None);
    for flag in [true, false] {
        assert_eq!(h.native(PAUSE, &[Value::Bool(flag)]).unwrap(), Value::Null);
        assert_eq!(h.game_paused(), Some(flag));
    }
}

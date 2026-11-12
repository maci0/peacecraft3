use super::*;
use crate::campaign_ai::melee_difficulty::{Input, Rules};

const DECLARATION: &str = "native MeleeDifficulty takes nothing returns integer\n";

#[test]
fn melee_difficulty_capture_is_per_caller_across_sleep_input_refresh_and_spoofed_globals() {
    let code = r#"
globals
player ai_player = null
integer difficulty = -197
integer before = 0
integer after = 0
endglobals
function main takes nothing returns nothing
set ai_player = Player(17)
set before = MeleeDifficulty()
call Sleep(1.0)
set difficulty = 814
set ai_player = Player(23)
set after = MeleeDifficulty()
endfunction
"#;
    let script = program_with_ai_common(
        "synthetic:captured.ai",
        code,
        &format!("{AI_COMMON}{DECLARATION}"),
    );
    let mut runtime = Runtime::<World>::default();
    runtime.set_loader(move |_| Ok(Arc::clone(&script)));
    runtime.set_melee_difficulty(Input::new(Rules::fixture(), &[(3, 5), (9, 9)]).unwrap());
    let shared = Arc::new(Mutex::new(State::default()));
    let mut world = Some(World(Arc::clone(&shared)));
    runtime.start(&mut world, 3, "captured.ai").unwrap();
    runtime.start(&mut world, 9, "captured.ai").unwrap();
    assert_eq!(runtime.active[&3].global("before"), Value::Int(7));
    assert_eq!(runtime.active[&9].global("before"), Value::Int(14));
    runtime
        .set_melee_difficulty(Input::new(Rules::fixture(), &[(3, 11), (9, 11), (4, 11)]).unwrap());
    runtime.start(&mut world, 3, "captured.ai").unwrap();
    shared.lock().unwrap().clock = 1.;
    for _ in 0..2 {
        let (_, player, ticket) = runtime.next_wake(1.).unwrap();
        runtime.resume(&mut world, player, ticket).unwrap();
    }
    assert_eq!(runtime.active[&3].global("after"), Value::Int(7));
    assert_eq!(runtime.active[&9].global("after"), Value::Int(14));
    runtime.start(&mut world, 4, "captured.ai").unwrap();
    assert_eq!(runtime.active[&4].global("before"), Value::Int(13));
    assert!(world.is_some());
}

#[test]
fn melee_difficulty_missing_input_refuses_only_query_and_malformed_abi_refuses_start() {
    let plain = program(
        "synthetic:plain.ai",
        "function main takes nothing returns nothing\nendfunction\n",
    );
    let query = program_with_ai_common(
        "synthetic:query.ai",
        "function main takes nothing returns nothing\nlocal integer v = MeleeDifficulty()\nendfunction\n",
        &format!("{AI_COMMON}{DECLARATION}"),
    );
    let mut runtime = Runtime::<World>::default();
    runtime
        .set_loader(move |name| Ok(Arc::clone(if name == "plain.ai" { &plain } else { &query })));
    let shared = Arc::new(Mutex::new(State::default()));
    let mut world = Some(World(Arc::clone(&shared)));
    runtime.start(&mut world, 3, "plain.ai").unwrap();
    assert!(
        runtime
            .start(&mut world, 9, "query.ai")
            .unwrap_err()
            .contains("not admitted")
    );
    assert!(!shared.lock().unwrap().active.contains_key(&9));
    runtime.set_melee_difficulty(Rules::fixture().fresh_input([3]).unwrap());
    assert!(
        runtime
            .start(&mut world, 9, "query.ai")
            .unwrap_err()
            .contains("missing for AI player 9")
    );
    assert!(world.is_some());
    for declaration in [
        "constant native MeleeDifficulty takes nothing returns integer\n",
        "native MeleeDifficulty takes integer v returns integer\n",
        "native MeleeDifficulty takes nothing returns real\n",
    ] {
        let script = program_with_ai_common(
            "synthetic:bad.ai",
            "function main takes nothing returns nothing\nendfunction\n",
            &format!("{AI_COMMON}{declaration}"),
        );
        assert!(
            admit_ai_natives(&script)
                .unwrap_err()
                .contains("MeleeDifficulty")
        );
    }
    let mut host = AiHost::new(3, World(shared));
    assert!(
        host.native("MeleeDifficulty", &[Value::Int(1)])
            .unwrap_err()
            .contains("no arguments")
    );
    assert!(
        host.native("MeleeDifficulty", &[])
            .unwrap_err()
            .contains("not admitted")
    );
}

#[test]
#[ignore = "requires selected PCX_WAR3_EXE, PCX_CAMPAIGN_AI_COMMON_J and PCX_CAMPAIGN_AI_COMMON_AI"]
fn selected_common_ai_melee_build_helper_consumes_the_captured_fresh_input() {
    let read = |key| std::fs::read_to_string(std::env::var(key).unwrap()).unwrap();
    let common = read("PCX_CAMPAIGN_AI_COMMON_J");
    let ai_common = read("PCX_CAMPAIGN_AI_COMMON_AI");
    let image = std::fs::read(std::env::var_os("PCX_WAR3_EXE").unwrap()).unwrap();
    let input = Rules::read(&image).unwrap().fresh_input([3, 9]).unwrap();
    let expected = input.for_player(3).unwrap();
    let code = r#"
globals
integer captured = 0
endglobals
function main takes nothing returns nothing
set difficulty = EASY
set ai_player = Player(17)
set captured = MeleeDifficulty()
call InitBuildArray()
call SetBuildUpgr(2, 'q123')
endfunction
"#;
    let script = program_with_sources("synthetic:selected-helper.ai", code, &common, &ai_common);
    let mut runtime = Runtime::<World>::default();
    runtime.set_loader(move |_| Ok(Arc::clone(&script)));
    runtime.set_melee_difficulty(input);
    let mut world = Some(World(Arc::new(Mutex::new(State::default()))));
    for owner in [3, 9] {
        runtime
            .start(&mut world, owner, "selected-helper.ai")
            .unwrap();
        assert_eq!(
            runtime.active[&owner].global("captured"),
            Value::Int(expected)
        );
        assert_eq!(runtime.active[&owner].global("build_length"), Value::Int(1));
    }
}

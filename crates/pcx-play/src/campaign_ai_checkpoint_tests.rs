use super::*;
use pcx_script::{
    checkpoint::{Budget, Reader, Writer},
    eval::CheckpointReference,
};

fn budget() -> Budget {
    Budget {
        bytes: 1 << 20,
        items: 10000,
        depth: 64,
    }
}
fn valid_player(player: i32) -> Result<(), String> {
    if (0..32).contains(&player) {
        Ok(())
    } else {
        Err("foreign synthetic player".into())
    }
}
fn valid_reference(reference: CheckpointReference<'_>) -> Result<(), String> {
    match reference {
        CheckpointReference::Handle { identity, .. } if identity.token() <= 32 => Ok(()),
        _ => Err("foreign synthetic AI reference".into()),
    }
}
fn encode(runtime: &mut Runtime<World>, world: &Option<World>) -> Vec<u8> {
    let mut out = Writer::new(budget()).unwrap();
    runtime
        .write_checkpoint(world.as_ref().unwrap(), &mut out, budget())
        .unwrap();
    out.finish()
}

#[test]
fn ai_root_section_restores_nested_locals_repark_and_captured_inputs_without_entrypoint_replay() {
    let script = program_with_ai_common(
        "synthetic:continued.ai",
        r#"
globals
integer before=MeleeDifficulty()
integer after=0
integer runs=0
integer phase=0
endglobals
function nested takes nothing returns nothing
local integer x=607
call Sleep(0.25)
set x=x+1
set phase=x
set after=MeleeDifficulty()
call SetPlayerState(Player(GetAiPlayer()),null,GetPlayerState(Player(GetAiPlayer()),null)+x)
call Sleep(0.5)
set phase=x+1
endfunction
function main takes nothing returns nothing
set runs=runs+1
call SetPlayerState(Player(GetAiPlayer()),null,GetPlayerState(Player(GetAiPlayer()),null)+1)
call nested()
endfunction
"#,
        &format!("{AI_COMMON}native MeleeDifficulty takes nothing returns integer\n"),
    );
    let make = || {
        let mut runtime = Runtime::<World>::default();
        let script = Arc::clone(&script);
        runtime.set_loader(move |path| {
            assert_eq!(path, "continued.ai");
            Ok(Arc::clone(&script))
        });
        runtime.set_pause_rules(Ok(crate::campaign_ai_pause_policy::Rules {
            initial: 0,
            retry_seconds: 2.,
            retry_word: 2f32.to_bits(),
        }));
        runtime.set_melee_difficulty(
            melee_difficulty::Input::new(
                melee_difficulty::Rules::fixture(),
                &[(3, 5), (9, 9), (4, 5)],
            )
            .unwrap(),
        );
        runtime
    };
    let mut live = make();
    let state = Arc::new(Mutex::new(State::default()));
    let mut world = Some(World(Arc::clone(&state)));
    for player in [3, 9] {
        live.start(&mut world, player, "continued.ai").unwrap();
    }
    live.set_melee_difficulty(
        melee_difficulty::Input::new(
            melee_difficulty::Rules::fixture(),
            &[(3, 11), (9, 11), (4, 11)],
        )
        .unwrap(),
    );
    live.pause(9, true).unwrap();
    live.pause(9, true).unwrap();
    let bytes = encode(&mut live, &world);
    let restored_state = Arc::new(Mutex::new(state.lock().unwrap().clone()));
    let mut restored_world = Some(World(Arc::clone(&restored_state)));
    let mut restored = make();
    let mut input = Reader::new(&bytes, budget()).unwrap();
    restored
        .restore_checkpoint(
            restored_world.as_ref().unwrap(),
            &mut input,
            budget(),
            valid_player,
            valid_player,
            valid_reference,
        )
        .unwrap();
    input.finish().unwrap();
    assert_eq!(restored.remaining_loads, live.remaining_loads);
    assert_eq!(*restored_state.lock().unwrap(), *state.lock().unwrap());
    for player in [3, 9] {
        assert_eq!(restored.global(player, "runs"), Some(Value::Int(1)));
        assert_eq!(
            restored.global(player, "before"),
            live.global(player, "before")
        );
        assert_eq!(
            restored.active.get_mut(&player).unwrap().host().pause_depth,
            live.active.get_mut(&player).unwrap().host().pause_depth
        );
    }
    for runtime in [&mut live, &mut restored] {
        runtime.pause(9, false).unwrap();
        runtime.pause(9, false).unwrap();
    }
    for at in [0.25, 0.75] {
        state.lock().unwrap().clock = at;
        restored_state.lock().unwrap().clock = at;
        while let Some((_, player, ticket)) = live.next_wake(at) {
            let matching = restored.next_wake(at).unwrap();
            assert_eq!((at, player, ticket), matching);
            live.resume(&mut world, player, ticket).unwrap();
            restored
                .resume(&mut restored_world, player, ticket)
                .unwrap();
        }
        assert!(restored.next_wake(at).is_none());
        assert_eq!(*restored_state.lock().unwrap(), *state.lock().unwrap());
        for player in [3, 9] {
            for name in ["runs", "phase", "before", "after"] {
                assert_eq!(restored.global(player, name), live.global(player, name));
            }
        }
    }
    for player in [3, 9] {
        assert_eq!(restored.global(player, "phase"), Some(Value::Int(609)));
    }
    live.start(&mut world, 4, "continued.ai").unwrap();
    restored
        .start(&mut restored_world, 4, "continued.ai")
        .unwrap();
    assert_eq!(restored.global(4, "before"), Some(Value::Int(13)));
    assert_eq!(*restored_state.lock().unwrap(), *state.lock().unwrap());
}

#[test]
fn ai_root_section_rejects_foreign_sources_truncation_and_lent_world_without_publishing() {
    let script = program(
        "synthetic:parked.ai",
        "function main takes nothing returns nothing\ncall Sleep(0.25)\nendfunction\n",
    );
    let make = |source: Arc<MapScripts>| {
        let mut runtime = Runtime::<World>::default();
        runtime.set_loader(move |_| Ok(Arc::clone(&source)));
        runtime
    };
    let mut live = make(Arc::clone(&script));
    let state = Arc::new(Mutex::new(State::default()));
    let mut world = Some(World(Arc::clone(&state)));
    live.start(&mut world, 1, "parked.ai").unwrap();
    let bytes = encode(&mut live, &world);
    let baseline = state.lock().unwrap().clone();
    for cut in 0..bytes.len() {
        let mut candidate = make(Arc::clone(&script));
        let mut input = Reader::new(&bytes[..cut], budget()).unwrap();
        assert!(
            candidate
                .restore_checkpoint(
                    world.as_ref().unwrap(),
                    &mut input,
                    budget(),
                    valid_player,
                    valid_player,
                    valid_reference
                )
                .is_err(),
            "cut {cut}"
        );
        assert!(candidate.active.is_empty());
        assert!(candidate.pending_retirement.is_empty());
        assert_eq!(candidate.remaining_loads, 64);
        assert_eq!(*state.lock().unwrap(), baseline);
    }
    let foreign = program(
        "synthetic:foreign.ai",
        "function main takes nothing returns nothing\ncall Sleep(0.5)\nendfunction\n",
    );
    let mut candidate = make(foreign);
    let mut input = Reader::new(&bytes, budget()).unwrap();
    let error = candidate
        .restore_checkpoint(
            world.as_ref().unwrap(),
            &mut input,
            budget(),
            valid_player,
            valid_player,
            valid_reference,
        )
        .unwrap_err();
    assert!(error.contains("source differs"), "{error}");
    assert!(candidate.active.is_empty());
    let vm = live.active.get_mut(&1).unwrap();
    vm.host().world = world.take();
    let mut out = Writer::new(budget()).unwrap();
    let unrelated = World(Arc::clone(&state));
    assert!(
        live.write_checkpoint(&unrelated, &mut out, budget())
            .unwrap_err()
            .contains("world is lent")
    );
    world = live.active.get_mut(&1).unwrap().host().world.take();
    assert!(world.is_some());
    live.active.get_mut(&1).unwrap().host().source = None;
    assert!(
        live.write_checkpoint(
            world.as_ref().unwrap(),
            &mut Writer::new(budget()).unwrap(),
            budget()
        )
        .unwrap_err()
        .contains("no pinned source")
    );
}

#[test]
fn ai_root_section_retains_failed_retirement_and_host_errors_without_replaying_main() {
    let script = program(
        "synthetic:retirement.ai",
        "function main takes nothing returns nothing\ncall SetPlayerState(Player(GetAiPlayer()),null,73)\ncall GetPlayerState(null,null)\nendfunction\n",
    );
    let make = || {
        let mut runtime = Runtime::<World>::default();
        let script = Arc::clone(&script);
        runtime.set_loader(move |_| Ok(Arc::clone(&script)));
        runtime
    };
    let mut live = make();
    let state = Arc::new(Mutex::new(State {
        retirement_blocked: true,
        ..Default::default()
    }));
    let mut world = Some(World(Arc::clone(&state)));
    assert!(
        live.start(&mut world, 1, "retirement.ai")
            .unwrap_err()
            .contains("retirement")
    );
    let vm = live.active.get_mut(&1).unwrap();
    vm.host().pending_sleep = Some(Err("retained synthetic refusal".into()));
    vm.host().next_ticket = 89;
    assert_eq!(live.pending_retirement, [1].into());
    let bytes = encode(&mut live, &world);
    let restored_state = Arc::new(Mutex::new(state.lock().unwrap().clone()));
    let mut restored_world = Some(World(Arc::clone(&restored_state)));
    let mut restored = make();
    let mut input = Reader::new(&bytes, budget()).unwrap();
    restored
        .restore_checkpoint(
            restored_world.as_ref().unwrap(),
            &mut input,
            budget(),
            valid_player,
            valid_player,
            valid_reference,
        )
        .unwrap();
    input.finish().unwrap();
    assert_eq!(restored.pending_retirement, [1].into());
    assert_eq!(restored.remaining_loads, 63);
    let host = restored.active.get_mut(&1).unwrap().host();
    assert_eq!(host.next_ticket, 89);
    assert_eq!(
        host.pending_sleep,
        Some(Err("retained synthetic refusal".into()))
    );
    assert_eq!(
        host.melee_difficulty,
        Err("MeleeDifficulty source input is not admitted".into())
    );
    assert_eq!(*restored_state.lock().unwrap(), *state.lock().unwrap());
    assert!(
        restored
            .resume(&mut restored_world, 1, 89)
            .unwrap_err()
            .contains("pending retirement")
    );
    assert!(
        restored
            .start(&mut restored_world, 1, "retirement.ai")
            .unwrap_err()
            .contains("retirement")
    );
    assert_eq!(*restored_state.lock().unwrap(), *state.lock().unwrap());
    restored_state.lock().unwrap().retirement_blocked = false;
    assert!(
        restored
            .start(&mut restored_world, 1, "retirement.ai")
            .unwrap_err()
            .contains("failed context retired")
    );
    assert!(restored.active.is_empty());
    assert!(restored.pending_retirement.is_empty());
    assert_eq!(restored_state.lock().unwrap().gold.get(&2), Some(&73));
}

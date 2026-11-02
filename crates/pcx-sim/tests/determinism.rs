//! The simulation is a pure function of its inputs.
//!
//! `step` takes the world, the tick's input and the live RNG stream, and its
//! documented contract is that the same world and the same input give the
//! same result wherever and whenever they run. The harness above proves the
//! property for whole traces; these checks pin it at the simulation itself,
//! so a failure localises to the step rather than to the trace codec.
//!
//! This is the harness's determinism floor. It is not native gameplay parity,
//! and the harness's RNG draws are recorded beside the step, not inside it.

use pcx_sim::{Command, Input, PlayerInput, Rng, World, step};

/// The commands issued on one tick.
fn commands_for(tick: u64) -> Vec<PlayerInput> {
    let mut commands = Vec::new();
    if tick.is_multiple_of(3) {
        commands.push(PlayerInput {
            player: 0,
            commands: vec![Command::Move { x: 4096, y: 0 }],
        });
    }
    if tick.is_multiple_of(5) {
        commands.push(PlayerInput {
            player: 1,
            commands: vec![Command::Select { x: 1024, y: 0 }],
        });
    }
    if tick.is_multiple_of(7) {
        commands.push(PlayerInput {
            player: 1,
            commands: vec![Command::Stop],
        });
    }
    commands
}

/// The starting world: three units owned by two players.
fn start() -> World {
    let mut world = World::empty();
    world.spawn(0, 0, 0);
    world.spawn(1, 1024, 0);
    world.spawn(1, -2048, 512);
    world
}

/// A fixed workload: the command schedule above and one draw per tick, so the
/// RNG stream advances with the world.
fn workload(seed: u64, ticks: u64) -> World {
    let mut world = start();
    let mut rng = Rng::new(seed);
    for tick in 0..ticks {
        let _ = rng.next_u64();
        step(
            &mut world,
            &Input {
                tick,
                commands: commands_for(tick),
            },
            &mut rng,
        );
    }
    world
}

#[test]
fn the_same_inputs_give_the_same_world_every_time() {
    let expected = workload(7, 64);
    for run in 0..100 {
        assert_eq!(workload(7, 64), expected, "run {run}");
    }
}

#[test]
fn the_command_delivery_order_does_not_change_the_outcome() {
    // A network layer may deliver the same tick's commands in any order; the
    // outcome must not depend on it.
    let ordered = {
        let mut world = start();
        let mut rng = Rng::new(7);
        step(
            &mut world,
            &Input {
                tick: 0,
                commands: commands_for(0),
            },
            &mut rng,
        );
        world
    };
    let mut reversed = commands_for(0);
    reversed.reverse();
    let reversed = {
        let mut world = start();
        let mut rng = Rng::new(7);
        step(
            &mut world,
            &Input {
                tick: 0,
                commands: reversed,
            },
            &mut rng,
        );
        world
    };
    assert_eq!(ordered, reversed);
}

#[test]
fn same_tick_commands_for_one_player_keep_their_declaration_order() {
    // The harness relies on this: a player that issues a move and a stop in
    // the same tick ends where its own declaration order says it should.
    let run = |commands: Vec<Command>| {
        let mut world = start();
        let mut rng = Rng::new(7);
        step(
            &mut world,
            &Input {
                tick: 0,
                commands: vec![PlayerInput {
                    player: 0,
                    commands,
                }],
            },
            &mut rng,
        );
        world.unit(1).expect("unit 1 exists").move_target
    };
    assert_eq!(
        run(vec![Command::Move { x: 4096, y: 0 }, Command::Stop]),
        None,
        "a stop declared after a move clears the target"
    );
    assert_eq!(
        run(vec![Command::Stop, Command::Move { x: 4096, y: 0 }]),
        Some((4096, 0)),
        "a move declared after a stop sets the target"
    );
}

#[test]
fn a_checkpointed_world_resumes_to_the_uninterrupted_result() {
    // The world holds everything a step needs, so a run that is copied
    // mid-stream and continued cannot drift from one that was never paused.
    let uninterrupted = workload(7, 64);
    for split in [0, 1, 32, 63] {
        let mut world = start();
        let mut rng = Rng::new(7);
        for tick in 0..split {
            let _ = rng.next_u64();
            step(
                &mut world,
                &Input {
                    tick,
                    commands: commands_for(tick),
                },
                &mut rng,
            );
        }
        // Copy the world and the live stream, then continue the copy.
        let mut resumed = (world.clone(), rng.clone());
        for tick in split..64 {
            let _ = resumed.1.next_u64();
            step(
                &mut resumed.0,
                &Input {
                    tick,
                    commands: commands_for(tick),
                },
                &mut resumed.1,
            );
        }
        assert_eq!(resumed.0, uninterrupted, "split={split}");
    }
}

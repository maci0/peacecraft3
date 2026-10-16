//! Deterministic fixed-tick game simulation, with no engine dependency.
//!
//! This crate deliberately knows nothing about Bevy, networking or time. It is a
//! pure function of `(seed, initial world, inputs) -> world`, and that property is
//! the whole design: multiplayer is only correct if the server and every client
//! compute the same world from the same inputs, so a `f32` that depends on frame
//! order, or an `Rc` that depends on thread interleaving, is a correctness bug
//! rather than a style problem.
//!
//! The rules:
//!
//! - **No float accumulation across ticks.** Positions are fixed-point integers,
//!   because IEEE-754 addition is not associative and a lockstep sim will diverge
//!   on the first network hiccup.
//! - **No wall clock inside a tick.** A tick is `tick_index -> state`, full stop.
//! - **No thread-local or OS state.** WASM has no threads to speak of, and a
//!   thread-local cache makes a tick non-reproducible.
//! - **Input is data, not behaviour.** The server decides; the client sends
//!   intent.

pub mod world;

/// Simulation tick duration in milliseconds.
///
/// The project's chosen step, exposed so every consumer agrees. The original
/// game's tick rate and frame relationship remain unverified.
pub const TICK_MS: u32 = 25;

/// Fixed-point scale: 16 fractional bits.
///
/// Positions are integers scaled by `1 << 16` in this movement harness.
/// The mapping from original authored coordinates remains a caller concern.
pub const FX_SHIFT: u32 = 16;
pub const FX_ONE: i32 = 1 << FX_SHIFT;

/// A fixed-point scalar.
pub type Fx = i32;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("command at tick {want} is out of order; last was {have}")]
    CommandOutOfOrder { want: u64, have: u64 },
    #[error("command at tick {tick} exceeds the sim horizon by {excess} ticks")]
    CommandTooFarAhead { tick: u64, excess: u64 },
    #[error("sim is at tick {have}, cannot advance to {want}")]
    CannotRewind { have: u64, want: u64 },
}

/// A player's intent for one tick.
///
/// Note what this is *not*: it is not "move unit 3 to (x, y)". That would let a
/// client author the world. It is a command plus arguments, which the sim
/// validates and either accepts or rejects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Do nothing for this tick.
    Nop,
    /// Replace this player's harness selection with owned units inside the
    /// existing axis-aligned `FX_ONE` test, and clear those units' targets.
    /// Owned units outside the test are deselected and keep any current target.
    Select { x: Fx, y: Fx },
    /// Issue a move intent to this player's selected units.
    ///
    /// `Unit::selected` starts set, so a move before any `Select` still reaches
    /// every unit that player owns. This is the harness rule, not native pathing.
    Move { x: Fx, y: Fx },
    /// Clear move targets of this player's selected units.
    Stop,
}

/// One player's commands for one tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerInput {
    pub player: u8,
    pub commands: Vec<Command>,
}

/// Everything the sim needs for one tick.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Input {
    pub tick: u64,
    pub commands: Vec<PlayerInput>,
}

/// A unit in the world.
///
/// Fixed-point throughout: `x`, `y`, and `move_x`/`move_y` are all
/// [`Fx`]. `target` is `None` when the unit is idle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unit {
    pub id: u32,
    pub owner: u8,
    /// Harness selection. Spawn sets this so a move before any `Select` still
    /// reaches every owned unit. Not a native selection flag. Version-0
    /// snapshots omit it and decode as selected.
    pub selected: bool,
    pub x: Fx,
    pub y: Fx,
    /// Current movement order, if any.
    pub move_target: Option<(Fx, Fx)>,
}

/// The authoritative world.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct World {
    pub tick: u64,
    pub units: Vec<Unit>,
    /// Monotonic id source. Never reused, so a reconnecting client cannot
    /// confuse a new unit with a dead one.
    pub next_unit_id: u32,
}

impl World {
    /// An empty world.
    pub fn empty() -> Self {
        Self {
            tick: 0,
            units: Vec::new(),
            next_unit_id: 1,
        }
    }

    /// Spawns a unit, returning its id.
    pub fn spawn(&mut self, owner: u8, x: Fx, y: Fx) -> u32 {
        let id = self.next_unit_id;
        self.next_unit_id += 1;
        self.units.push(Unit {
            id,
            owner,
            selected: true,
            x,
            y,
            move_target: None,
        });
        id
    }

    pub fn unit(&self, id: u32) -> Option<&Unit> {
        self.units.iter().find(|u| u.id == id)
    }
}

/// Deterministic RNG, split-mix64.
///
/// Not cryptographic and not the game's RNG — this exists so the sim has a
/// reproducible source without reaching for a crate that may or may not be
/// stable across platforms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self {
            state: seed.wrapping_add(0x9E37_79B9_7F4A_7C15),
        }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state ^= self.state >> 30;
        self.state = self.state.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        self.state ^= self.state >> 27;
        self.state = self.state.wrapping_mul(0x94D0_49BB_1331_11EB);
        self.state ^= self.state >> 31;
        self.state
    }
}

/// A fixed-tick simulation step.
///
/// The signature is the contract: same `world`, same `input`, same result,
/// regardless of where or when it runs. `rng` is threaded through rather than
/// stored in the world so it stays out of the equality check — two worlds are
/// equal when their *state* agrees, not when their random streams happen to.
pub fn step(world: &mut World, input: &Input, rng: &mut Rng) {
    debug_assert_eq!(world.tick, input.tick, "input must match the world's tick");

    // Every player's commands, in player order, so command application cannot
    // depend on the order a network layer happened to deliver them in.
    let mut ordered: Vec<&PlayerInput> = input.commands.iter().collect();
    ordered.sort_by_key(|c| c.player);

    for cmd in ordered {
        for c in &cmd.commands {
            apply_command(world, cmd.player, c, rng);
        }
    }

    // Movement is resolved after all commands, so a unit that is issued a move
    // and a stop in the same tick ends where "stop wins" says it should,
    // independent of array order.
    for unit in &mut world.units {
        let Some((tx, ty)) = unit.move_target else {
            continue;
        };
        // A difference between two valid Fx coordinates can exceed i32.
        let (dx, dy) = (
            i64::from(tx) - i64::from(unit.x),
            i64::from(ty) - i64::from(unit.y),
        );
        if dx == 0 && dy == 0 {
            unit.move_target = None;
            continue;
        }
        // Fixed speed, fixed step: this is the deterministic part. No
        // normalising by `hypot`, because that would need a square root and a
        // rounding mode.
        const SPEED: i32 = FX_ONE / 10;
        // Each clamped delta fits Fx; adding it stays between the current
        // coordinate and the representable target, so cannot overflow.
        let limit = i64::from(SPEED);
        let step_x = dx.clamp(-limit, limit) as Fx;
        let step_y = dy.clamp(-limit, limit) as Fx;
        unit.x += step_x;
        unit.y += step_y;
    }

    world.tick += 1;
}

/// Whether an owned unit is inside the harness selection test.
///
/// Differences are widened before the absolute value. Each axis uses the
/// existing strict `< FX_ONE` comparison. This is not a native selection radius.
pub(crate) fn unit_within_selection(unit_x: Fx, unit_y: Fx, x: Fx, y: Fx) -> bool {
    (i64::from(unit_x) - i64::from(x)).abs() < i64::from(FX_ONE)
        && (i64::from(unit_y) - i64::from(y)).abs() < i64::from(FX_ONE)
}

/// Applies one command, validating it against the world.
fn apply_command(world: &mut World, player: u8, cmd: &Command, _rng: &mut Rng) {
    match *cmd {
        Command::Nop => {}
        Command::Select { x, y } => {
            // Selection is the player's own choice of a unit they own; a client
            // cannot select another player's unit through a malformed order.
            for unit in &mut world.units {
                if unit.owner != player {
                    continue;
                }
                if unit_within_selection(unit.x, unit.y, x, y) {
                    unit.selected = true;
                    unit.move_target = None;
                } else {
                    unit.selected = false;
                }
            }
        }
        Command::Move { x, y } => {
            for unit in &mut world.units {
                if unit.owner == player && unit.selected {
                    unit.move_target = Some((x, y));
                }
            }
        }
        Command::Stop => {
            for unit in &mut world.units {
                if unit.owner == player && unit.selected {
                    unit.move_target = None;
                }
            }
        }
    }
}

/// The maximum number of ticks a command may be issued ahead of the sim.
///
/// Without a horizon a malicious client can send a command a billion ticks out
/// and make the server buffer it.
pub const MAX_INPUT_LEAD: u64 = 120;

/// Validates a batch of input against the sim's current tick.
pub fn validate_input(world: &World, input: &Input) -> Result<(), Error> {
    if input.tick < world.tick {
        return Err(Error::CannotRewind {
            have: world.tick,
            want: input.tick,
        });
    }
    let lead = input.tick - world.tick;
    if lead > MAX_INPUT_LEAD {
        return Err(Error::CommandTooFarAhead {
            tick: input.tick,
            excess: lead - MAX_INPUT_LEAD,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world_with_unit() -> (World, u32) {
        let mut w = World::empty();
        let id = w.spawn(0, 0, 0);
        (w, id)
    }

    #[test]
    fn same_input_yields_the_same_world() {
        let mut a = world_with_unit().0;
        let mut b = a.clone();
        let mut rng_a = Rng::new(1);
        let mut rng_b = Rng::new(1);
        // A fresh input each tick, because `tick` is part of the input and the
        // world advances. Reusing one input is a test bug that the debug_assert
        // in `step` exists to catch.
        for _ in 0..100 {
            let input = Input {
                tick: a.tick,
                commands: vec![PlayerInput {
                    player: 0,
                    commands: vec![Command::Move {
                        x: FX_ONE * 10,
                        y: 0,
                    }],
                }],
            };
            step(&mut a, &input, &mut rng_a);
            step(&mut b, &input, &mut rng_b);
            assert_eq!(a, b, "two runs of the same input diverged");
        }
    }

    #[test]
    fn fixed_point_movement_is_exact() {
        let (mut w, id) = world_with_unit();
        let mut rng = Rng::new(0);
        // Iterating to arrival is the property: fixed speed means the last step
        // is clamped to the remaining distance, so the unit lands *exactly* on
        // the target rather than overshooting by a fraction.
        let mut order = |w: &mut World| {
            let input = Input {
                tick: w.tick,
                commands: vec![PlayerInput {
                    player: 0,
                    commands: vec![Command::Move {
                        x: FX_ONE * 10,
                        y: 0,
                    }],
                }],
            };
            step(w, &input, &mut rng);
        };
        // The order is issued first, then the sim runs until arrival. Checking
        // for a target before any command has been issued would find none and
        // exit immediately — which is what the first version of this did.
        order(&mut w);
        let mut ticks = 1u32;
        while w.unit(id).unwrap().move_target.is_some() {
            assert!(ticks < 1_000, "movement never completed");
            order(&mut w);
            ticks += 1;
        }
        assert_eq!(
            w.unit(id).unwrap().x,
            FX_ONE * 10,
            "must land exactly on the target, not near it"
        );
        // Deliberately *not* asserting an exact tick count. `FX_ONE / 10`
        // truncates to 6553 rather than being 6553.6, so the trip takes one more
        // tick than a naive division suggests — and asserting that would be
        // asserting a rounding mode rather than a behaviour. What matters is that
        // it arrives exactly, and in a bounded number of ticks.
        assert!(ticks <= 110, "arrived implausibly late: {ticks} ticks");
        assert_eq!(w.tick, ticks as u64, "one tick per step, no more");
    }

    #[test]
    fn command_order_does_not_depend_on_delivery_order() {
        let base = || {
            let mut w = World::empty();
            w.spawn(0, 0, 0);
            w
        };

        let a = Input {
            tick: 0,
            commands: vec![
                PlayerInput {
                    player: 1,
                    commands: vec![Command::Stop],
                },
                PlayerInput {
                    player: 0,
                    commands: vec![Command::Move {
                        x: FX_ONE,
                        y: FX_ONE,
                    }],
                },
            ],
        };
        let b = Input {
            tick: 0,
            commands: vec![
                PlayerInput {
                    player: 0,
                    commands: vec![Command::Move {
                        x: FX_ONE,
                        y: FX_ONE,
                    }],
                },
                PlayerInput {
                    player: 1,
                    commands: vec![Command::Stop],
                },
            ],
        };
        let mut wa = base();
        let mut wb = base();
        let mut rng = Rng::new(0);
        step(&mut wa, &a, &mut rng);
        step(&mut wb, &b, &mut rng);
        assert_eq!(wa, wb, "command application depended on delivery order");
    }

    #[test]
    fn a_player_cannot_command_another_players_units() {
        let mut w = World::empty();
        let mine = w.spawn(0, 0, 0);
        w.spawn(1, FX_ONE * 10, FX_ONE * 10);
        let input = Input {
            tick: 0,
            commands: vec![PlayerInput {
                player: 0,
                commands: vec![Command::Move {
                    x: FX_ONE * 10,
                    y: FX_ONE * 10,
                }],
            }],
        };
        let mut rng = Rng::new(0);
        step(&mut w, &input, &mut rng);
        assert!(w.unit(mine).unwrap().move_target.is_some(), "my unit moved");
        assert!(
            w.unit(2).unwrap().move_target.is_none(),
            "another player's unit must not be ordered"
        );
    }

    #[test]
    fn an_input_far_in_the_future_is_refused() {
        let w = World::empty();
        let far = Input {
            tick: MAX_INPUT_LEAD + 1,
            commands: Vec::new(),
        };
        assert!(
            validate_input(&w, &far).is_err(),
            "horizon must be enforced"
        );
        assert!(
            validate_input(
                &w,
                &Input {
                    tick: MAX_INPUT_LEAD,
                    commands: Vec::new(),
                }
            )
            .is_ok(),
            "up to the horizon is allowed"
        );
    }

    #[test]
    fn the_sim_cannot_rewind() {
        let mut w = World::empty();
        w.tick = 10;
        let old = Input {
            tick: 9,
            commands: Vec::new(),
        };
        assert!(matches!(
            validate_input(&w, &old),
            Err(Error::CannotRewind { .. })
        ));
    }

    #[test]
    fn unit_ids_are_never_reused() {
        let mut w = World::empty();
        let first = w.spawn(0, 0, 0);
        w.units.retain(|u| u.id != first);
        let second = w.spawn(0, 0, 0);
        assert_ne!(first, second, "a dead id must not come back");
    }

    #[test]
    fn the_rng_is_reproducible() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..32 {
            assert_eq!(a.next_u64(), b.next_u64(), "same seed, different stream");
        }
    }
}

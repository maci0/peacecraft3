//! Declarative play-test scenarios and run configuration.
//!
//! A scenario is data: a name, a map identity, a tick count, a starting state
//! and a schedule of scripted actions. The same scenario run twice produces the
//! same trace, byte for byte. M1b scenarios are synthetic: the starting state is
//! declared here rather than decoded from a map container, because `pcx-map`
//! wiring lands at M3 and script execution at M7.

use pcx_sim::{Command, FX_ONE, Fx};

use crate::trace::{AssetMode, FixRow};

/// Which assets and which fix-ledger rows a run is given (plan §3.6, §3.10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunConfig {
    pub asset_mode: AssetMode,
    /// Seed for the one `pcx_sim::Rng` the run draws from.
    pub seed: u64,
    /// Active fix-ledger rows, one per row id. The runner sorts them by id
    /// before recording them in the trace header.
    pub fix_rows: Vec<FixRow>,
}

/// One unit the scenario starts with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnSpec {
    pub owner: u8,
    pub x: Fx,
    pub y: Fx,
}

/// What a scheduled action does when it fires.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptedAction {
    /// Intent for one player, applied through `pcx_sim` like any other input.
    Commands { player: u8, commands: Vec<Command> },
    /// Draws one value from the shared sim RNG and records it in the trace.
    ///
    /// This is the harness-side source of RNG draws until gameplay draws from
    /// inside `pcx_sim::step` at M5. Later tiers also use it to consume the
    /// stream in scripted, reproducible ways.
    RngDraw,
}

/// One scripted action, named so its firing is a traceable timer event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledAction {
    /// The tick it fires on, before that tick's step.
    pub tick: u64,
    pub name: String,
    pub action: ScriptedAction,
}

/// A declarative play-test scenario.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scenario {
    pub name: String,
    /// Map identity recorded in the trace header. M1b runs synthetic
    /// scenarios, so no container is read.
    pub map: String,
    /// Number of `pcx_sim` steps to run, and so the number of tick records.
    pub ticks: u64,
    pub spawns: Vec<SpawnSpec>,
    /// Scripted actions in declaration order; same-tick actions apply in that
    /// order within a player.
    pub script: Vec<ScheduledAction>,
}

/// The shipped smoke scenario: spawn one unit, draw once, order a move at tick
/// 10, run 100 ticks.
pub fn smoke() -> Scenario {
    Scenario {
        name: "smoke-move-100".to_string(),
        map: "synthetic.smoke-move".to_string(),
        ticks: SMOKE_TICKS,
        spawns: vec![SpawnSpec {
            owner: 0,
            x: 0,
            y: 0,
        }],
        script: vec![
            ScheduledAction {
                tick: 0,
                name: "rng-probe".to_string(),
                action: ScriptedAction::RngDraw,
            },
            ScheduledAction {
                tick: SMOKE_MOVE_TICK,
                name: "move-order".to_string(),
                action: ScriptedAction::Commands {
                    player: 0,
                    commands: vec![Command::Move {
                        x: FX_ONE * 10,
                        y: 0,
                    }],
                },
            },
        ],
    }
}

/// The run configuration the smoke scenario is pinned under.
pub fn smoke_config() -> RunConfig {
    RunConfig {
        asset_mode: AssetMode::Vanilla,
        seed: 1,
        fix_rows: Vec::new(),
    }
}

/// Tick count of [`smoke`].
pub const SMOKE_TICKS: u64 = 100;

/// The tick [`smoke`] issues its move order on.
pub const SMOKE_MOVE_TICK: u64 = 10;

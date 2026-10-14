//! The deterministic runner: scenario in, trace out.
//!
//! One `pcx_sim::Rng`, fixed-point arithmetic, no clock. Every scheduled action
//! fires before the step of its tick and is recorded as a timer event; the
//! step's post-state, the tick's RNG draws and its timer events form the tick
//! record.

use std::collections::{BTreeMap, BTreeSet};

use pcx_sim::{Command, Input, PlayerInput, Rng, World, step};

use crate::is_valid_name;
use crate::scenario::{RunConfig, Scenario, ScriptedAction};
use crate::trace::{
    RngDraw, TRACE_VERSION, TickRecord, TimerEvent, TimerEventKind, Trace, TraceHeader, UnitState,
};

/// Why a scenario or run configuration is not runnable.
#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error("scenario {scenario:?}: {ticks} tick records exceed the platform's vector layout")]
    TraceCapacity { scenario: String, ticks: u64 },
    #[error("scenario {scenario:?}: cannot allocate {ticks} tick records: {source}")]
    TraceAllocation {
        scenario: String,
        ticks: u64,
        #[source]
        source: std::collections::TryReserveError,
    },
    #[error("{kind} name {value:?} is invalid")]
    BadName { kind: &'static str, value: String },
    #[error(
        "script action {name:?} is scheduled at tick {tick} but the run is only {ticks} ticks long"
    )]
    ScriptTickOutOfBounds { name: String, tick: u64, ticks: u64 },
    #[error("fix-ledger row id {id:?} appears more than once")]
    DuplicateFixRow { id: String },
}

/// Runs a scenario under a configuration and returns the trace.
///
/// The trace header records the asset mode and the active fix-ledger rows, so
/// runs under different asset or fix configurations are never confused.
pub fn run(scenario: &Scenario, config: &RunConfig) -> Result<Trace, RunError> {
    validate(scenario, config)?;

    let tick_count = usize::try_from(scenario.ticks)
        .ok()
        .filter(|&n| std::alloc::Layout::array::<TickRecord>(n).is_ok())
        .ok_or_else(|| RunError::TraceCapacity {
            scenario: scenario.name.clone(),
            ticks: scenario.ticks,
        })?;
    let mut ticks: Vec<TickRecord> = Vec::new();
    ticks
        .try_reserve_exact(tick_count)
        .map_err(|source| RunError::TraceAllocation {
            scenario: scenario.name.clone(),
            ticks: scenario.ticks,
            source,
        })?;

    let mut world = World::empty();
    for spawn in &scenario.spawns {
        world.spawn(spawn.owner, spawn.x, spawn.y);
    }
    let mut rng = Rng::new(config.seed);

    // Grouping by tick keeps same-tick actions in declaration order; BTreeMap
    // iteration is by ascending tick and is the only map anywhere near the
    // output.
    let mut schedule: BTreeMap<u64, Vec<&crate::scenario::ScheduledAction>> = BTreeMap::new();
    for entry in &scenario.script {
        schedule.entry(entry.tick).or_default().push(entry);
    }

    let mut draw_index = 0u64;
    let mut timer_seq = 0u64;
    for tick in 0..scenario.ticks {
        let mut commands: BTreeMap<u8, Vec<Command>> = BTreeMap::new();
        let mut rng_draws = Vec::new();
        let mut timers = Vec::new();
        for entry in schedule.get(&tick).into_iter().flatten() {
            timers.push(TimerEvent {
                seq: timer_seq,
                kind: TimerEventKind::Fire,
                name: entry.name.clone(),
            });
            timer_seq += 1;
            match &entry.action {
                ScriptedAction::Commands {
                    player,
                    commands: scripted,
                } => {
                    commands
                        .entry(*player)
                        .or_default()
                        .extend(scripted.iter().cloned());
                }
                ScriptedAction::RngDraw => {
                    let value = rng.next_u64();
                    rng_draws.push(RngDraw {
                        index: draw_index,
                        value,
                    });
                    draw_index += 1;
                }
            }
        }

        let input = Input {
            tick,
            commands: commands
                .into_iter()
                .map(|(player, commands)| PlayerInput { player, commands })
                .collect(),
        };
        step(&mut world, &input, &mut rng);

        let mut units: Vec<UnitState> = world
            .units
            .iter()
            .map(|u| UnitState {
                id: u.id,
                owner: u.owner,
                x: u.x,
                y: u.y,
                target: u.move_target,
            })
            .collect();
        units.sort_by_key(|u| u.id);
        ticks.push(TickRecord {
            tick,
            units,
            rng_draws,
            timers,
        });
    }

    let mut fix_rows = config.fix_rows.clone();
    fix_rows.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(Trace {
        header: TraceHeader {
            format_version: TRACE_VERSION,
            scenario: scenario.name.clone(),
            map: scenario.map.clone(),
            asset_mode: config.asset_mode,
            seed: config.seed,
            fix_rows,
            tick_count: scenario.ticks,
        },
        ticks,
    })
}

fn validate(scenario: &Scenario, config: &RunConfig) -> Result<(), RunError> {
    let name = |kind, value: &str| RunError::BadName {
        kind,
        value: value.to_string(),
    };
    if !is_valid_name(&scenario.name) {
        return Err(name("scenario", &scenario.name));
    }
    if !is_valid_name(&scenario.map) {
        return Err(name("map", &scenario.map));
    }
    for entry in &scenario.script {
        if entry.tick >= scenario.ticks {
            return Err(RunError::ScriptTickOutOfBounds {
                name: entry.name.clone(),
                tick: entry.tick,
                ticks: scenario.ticks,
            });
        }
        if !is_valid_name(&entry.name) {
            return Err(name("timer", &entry.name));
        }
    }
    let mut seen = BTreeSet::new();
    for row in &config.fix_rows {
        if !is_valid_name(&row.id) {
            return Err(name("fix row id", &row.id));
        }
        if !seen.insert(row.id.as_str()) {
            return Err(RunError::DuplicateFixRow { id: row.id.clone() });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::first_divergence;
    use crate::scenario::{SMOKE_MOVE_TICK, SMOKE_TICKS, ScheduledAction, smoke, smoke_config};
    use crate::trace::{AssetMode, FixDisposition, FixRow};
    use pcx_sim::FX_ONE;

    #[test]
    fn the_same_scenario_produces_byte_identical_traces() {
        let a = run(&smoke(), &smoke_config()).unwrap_or_else(|e| panic!("{e}"));
        let b = run(&smoke(), &smoke_config()).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            a.encode(),
            b.encode(),
            "two runs of the same scenario must be byte-identical"
        );
        assert_eq!(a.canonical_hash(), b.canonical_hash());
    }

    #[test]
    fn the_smoke_scenario_completes_with_a_pinned_hash() {
        let trace = run(&smoke(), &smoke_config()).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(trace.header.tick_count, SMOKE_TICKS);
        assert_eq!(trace.ticks.len() as u64, SMOKE_TICKS);
        assert_eq!(
            trace.ticks[0].timers[0].name, "rng-probe",
            "the smoke trace must carry the harness timer events"
        );
        assert_eq!(
            trace.ticks[0].rng_draws.len(),
            1,
            "the smoke trace must carry the harness RNG draw"
        );
        assert_eq!(trace.canonical_hash(), SMOKE_TRACE_HASH);
    }

    #[test]
    fn a_perturbed_input_diverges_at_the_first_affected_tick() {
        let base = run(&smoke(), &smoke_config()).unwrap_or_else(|e| panic!("{e}"));
        let mut scenario = smoke();
        match &mut scenario.script[1].action {
            ScriptedAction::Commands { commands, .. } => match &mut commands[0] {
                pcx_sim::Command::Move { x, .. } => *x = FX_ONE * 11,
                _ => unreachable!("the smoke script moves"),
            },
            _ => unreachable!("the smoke script moves"),
        }
        let perturbed = run(&scenario, &smoke_config()).unwrap_or_else(|e| panic!("{e}"));
        let divergence = first_divergence(&base, &perturbed).expect("the traces must diverge");
        assert_eq!(
            divergence.tick,
            Some(SMOKE_MOVE_TICK),
            "the first divergent tick must be the tick the order was changed on"
        );
        assert_eq!(divergence.field, "unit[0].target");
    }

    #[test]
    fn a_misconfigured_run_is_refused() {
        let mut out_of_bounds = smoke();
        out_of_bounds.script.push(ScheduledAction {
            tick: SMOKE_TICKS,
            name: "late".to_string(),
            action: ScriptedAction::RngDraw,
        });
        assert!(matches!(
            run(&out_of_bounds, &smoke_config()),
            Err(RunError::ScriptTickOutOfBounds { .. })
        ));

        let mut bad_name = smoke();
        bad_name.name = "not a name".to_string();
        assert!(matches!(
            run(&bad_name, &smoke_config()),
            Err(RunError::BadName { .. })
        ));

        let config = RunConfig {
            asset_mode: AssetMode::Vanilla,
            seed: 1,
            fix_rows: vec![
                FixRow {
                    id: "fix.a".to_string(),
                    disposition: FixDisposition::Adopt,
                },
                FixRow {
                    id: "fix.a".to_string(),
                    disposition: FixDisposition::Defer,
                },
            ],
        };
        assert!(matches!(
            run(&smoke(), &config),
            Err(RunError::DuplicateFixRow { .. })
        ));
    }

    #[test]
    #[ignore = "long horizon"]
    fn a_10_000_tick_run_is_stable() {
        let mut scenario = smoke();
        scenario.ticks = 10_000;
        let a = run(&scenario, &smoke_config()).unwrap_or_else(|e| panic!("{e}"));
        let b = run(&scenario, &smoke_config()).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(a.ticks.len(), 10_000);
        assert_eq!(
            a.encode(),
            b.encode(),
            "a long run must be as byte-stable as a short one"
        );
        assert_eq!(a.canonical_hash(), b.canonical_hash());
    }

    /// FNV-1a 64 of the canonical smoke trace under [`smoke_config`]. Pinning it
    /// catches any change to the scenario, the runner, the codec or `pcx-sim`.
    const SMOKE_TRACE_HASH: u64 = 0x0b15_e219_287c_4812;
}

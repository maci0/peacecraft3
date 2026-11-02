//! Mid-run resume with RNG continuity (PLAN §3.5, Tier C).
//!
//! Harness mechanics, not game behavior. The original client's RNG,
//! scheduling and suspend/resume behavior are not established here; what
//! this module establishes is narrower: a run that pauses mid-stream and
//! resumes from the paused RNG/world state produces exactly the trace an
//! uninterrupted run produces. That property is what lets a long fuzz run
//! checkpoint and continue without invalidating its trace.
//!
//! Coordination: the canonical runner (`crate::runner::run`) stays with
//! the `mechanics` lane. This module never redefines its semantics; it
//! replays the same schedule grouping, the same single-`Rng` threading,
//! the same canonical header sort and the same post-step snapshot order,
//! and it delegates the uninterrupted reference to [`crate::run`]. Any
//! change to the runner's ordering must be mirrored here, which is why
//! both paths share the public `Scenario`/`RunConfig`/`Trace` types.

use std::collections::{BTreeMap, BTreeSet};

use pcx_sim::{Command, Input, PlayerInput, Rng, World, step};

use crate::is_valid_name;
use crate::runner::RunError;
use crate::scenario::{RunConfig, Scenario};
use crate::trace::{
    RngDraw, TRACE_VERSION, TickRecord, TimerEvent, TimerEventKind, Trace, TraceHeader, UnitState,
};

/// A paused run: everything needed to continue without re-seeding.
///
/// `rng` is the live stream, cloned at the pause point, never rebuilt from
/// the seed. `draw_index` and `timer_seq` continue their uninterrupted
/// counters so resumed tick records carry the same indices and sequence
/// numbers. `ticks` holds the completed prefix records in order.
#[derive(Debug, Clone)]
pub struct ResumeState {
    /// World after the last completed tick (`world.tick == next_tick`).
    pub world: World,
    /// Live RNG stream at the pause point.
    pub rng: Rng,
    /// Next RNG draw index (equals the count drawn so far).
    pub draw_index: u64,
    /// Next timer sequence number.
    pub timer_seq: u64,
    /// First tick not yet executed.
    pub next_tick: u64,
    /// Completed tick records, in order.
    pub ticks: Vec<TickRecord>,
}

/// Runs the `0..upto` prefix of `scenario` and pauses.
///
/// `upto` is clamped to `scenario.ticks`; pausing at or past the end
/// yields a state whose [`resume_to_end`][ResumeState] produces no further
/// records. Validation mirrors [`crate::run`]: bad names, out-of-bounds
/// script ticks and duplicate fix rows are refused before any step.
pub fn run_head(
    scenario: &Scenario,
    config: &RunConfig,
    upto: u64,
) -> Result<ResumeState, RunError> {
    validate(scenario, config)?;
    let upto = upto.min(scenario.ticks);
    let tick_count = usize::try_from(upto)
        .ok()
        .filter(|&n| std::alloc::Layout::array::<TickRecord>(n).is_ok())
        .ok_or_else(|| RunError::TraceCapacity {
            scenario: scenario.name.clone(),
            ticks: upto,
        })?;
    let mut ticks: Vec<TickRecord> = Vec::new();
    ticks
        .try_reserve_exact(tick_count)
        .map_err(|source| RunError::TraceAllocation {
            scenario: scenario.name.clone(),
            ticks: upto,
            source,
        })?;

    let mut world = World::empty();
    for spawn in &scenario.spawns {
        world.spawn(spawn.owner, spawn.x, spawn.y);
    }
    let mut rng = Rng::new(config.seed);
    let schedule = schedule_by_tick(scenario);

    let mut draw_index = 0u64;
    let mut timer_seq = 0u64;
    for tick in 0..upto {
        let record = step_tick(
            &mut world,
            &mut rng,
            &schedule,
            tick,
            &mut draw_index,
            &mut timer_seq,
        );
        ticks.push(record);
    }
    Ok(ResumeState {
        world,
        rng,
        draw_index,
        timer_seq,
        next_tick: upto,
        ticks,
    })
}

/// Resumes a paused state to the end of `scenario` under `config`.
///
/// The returned trace is byte-identical to [`crate::run`] on the same
/// inputs: same header sort, same record order, same RNG stream. The only
/// difference is that ticks before `state.next_tick` are replayed from the
/// stored prefix instead of re-executed.
pub fn resume_to_end(
    mut state: ResumeState,
    scenario: &Scenario,
    config: &RunConfig,
) -> Result<Trace, RunError> {
    validate(scenario, config)?;
    if state.next_tick > scenario.ticks {
        state.next_tick = scenario.ticks;
        state
            .ticks
            .truncate(usize::try_from(state.next_tick).unwrap_or(usize::MAX));
    }
    let schedule = schedule_by_tick(scenario);
    for tick in state.next_tick..scenario.ticks {
        let record = step_tick(
            &mut state.world,
            &mut state.rng,
            &schedule,
            tick,
            &mut state.draw_index,
            &mut state.timer_seq,
        );
        state.ticks.push(record);
    }
    Ok(finish(scenario, config, state.ticks))
}

/// Runs a scenario end-to-end through the checkpoint path.
///
/// Splits at `split_at`, pauses, then resumes. Convenience wrapper for
/// the continuity property: `run_via_resume(s, c, k) == crate::run(s, c)`
/// for every in-bounds `k`.
pub fn run_via_resume(
    scenario: &Scenario,
    config: &RunConfig,
    split_at: u64,
) -> Result<Trace, RunError> {
    let head = run_head(scenario, config, split_at)?;
    resume_to_end(head, scenario, config)
}

fn schedule_by_tick(scenario: &Scenario) -> BTreeMap<u64, Vec<&crate::scenario::ScheduledAction>> {
    let mut schedule: BTreeMap<u64, Vec<&crate::scenario::ScheduledAction>> = BTreeMap::new();
    for entry in &scenario.script {
        schedule.entry(entry.tick).or_default().push(entry);
    }
    schedule
}

#[allow(clippy::too_many_arguments)]
fn step_tick(
    world: &mut World,
    rng: &mut Rng,
    schedule: &BTreeMap<u64, Vec<&crate::scenario::ScheduledAction>>,
    tick: u64,
    draw_index: &mut u64,
    timer_seq: &mut u64,
) -> TickRecord {
    let mut commands: BTreeMap<u8, Vec<Command>> = BTreeMap::new();
    let mut rng_draws = Vec::new();
    let mut timers = Vec::new();
    for entry in schedule.get(&tick).into_iter().flatten() {
        timers.push(TimerEvent {
            seq: *timer_seq,
            kind: TimerEventKind::Fire,
            name: entry.name.clone(),
        });
        *timer_seq += 1;
        match &entry.action {
            crate::scenario::ScriptedAction::Commands {
                player,
                commands: scripted,
            } => {
                commands
                    .entry(*player)
                    .or_default()
                    .extend(scripted.iter().cloned());
            }
            crate::scenario::ScriptedAction::RngDraw => {
                let value = rng.next_u64();
                rng_draws.push(RngDraw {
                    index: *draw_index,
                    value,
                });
                *draw_index += 1;
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
    step(world, &input, rng);

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
    TickRecord {
        tick,
        units,
        rng_draws,
        timers,
    }
}

fn finish(scenario: &Scenario, config: &RunConfig, ticks: Vec<TickRecord>) -> Trace {
    let mut fix_rows = config.fix_rows.clone();
    fix_rows.sort_by(|a, b| a.id.cmp(&b.id));
    Trace {
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
    }
}

fn validate(scenario: &Scenario, config: &RunConfig) -> Result<(), RunError> {
    let name = |kind: &'static str, value: &str| RunError::BadName {
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
    use crate::scenario::{smoke, smoke_config};

    #[test]
    fn zero_length_checkpoint_does_not_reserve_the_full_scenario() {
        let scenario = Scenario {
            name: "empty-prefix".into(),
            map: "synthetic.empty-prefix".into(),
            ticks: u64::MAX,
            spawns: Vec::new(),
            script: Vec::new(),
        };

        let state = run_head(&scenario, &smoke_config(), 0).unwrap();
        assert_eq!(state.next_tick, 0);
        assert!(state.ticks.is_empty());
    }

    #[test]
    fn checkpoint_resume_matches_the_uninterrupted_trace() {
        let scenario = smoke();
        let config = smoke_config();
        let expected = crate::run(&scenario, &config).unwrap().encode();

        for split in [0, 1, scenario.ticks / 2, scenario.ticks] {
            let actual = run_via_resume(&scenario, &config, split).unwrap().encode();
            assert_eq!(actual, expected, "split={split}");
        }
    }

    #[test]
    fn every_corpus_entry_resumes_to_the_uninterrupted_trace() {
        let entries = crate::fuzz_corpus::corpus_entries();
        let domains = crate::fuzz_scenario::default_domains();
        let limits = crate::fuzz_scenario::default_limits();
        for (index, entry) in entries.iter().enumerate() {
            let (rpl, _) = crate::fuzz_scenario::generate_one(
                entry.seed,
                index,
                entry.ticks,
                entry.units,
                entry.actions,
                &domains,
                limits,
            )
            .unwrap();
            let parsed = crate::Rpl::parse(&rpl).unwrap();
            assert_eq!(parsed.config.seed, entry.seed, "index={index}");
            let expected = parsed.run().unwrap().encode();
            let actual = run_via_resume(&parsed.scenario, &parsed.config, entry.ticks / 2)
                .unwrap()
                .encode();
            assert_eq!(actual, expected, "seed={} index={index}", entry.seed);
        }
    }
}

//! Versioned, synthetic play scenarios. Coordinates are raw `pcx_sim::Fx` integers.
//! This format does not pretend that a map identity loads an original map.
use crate::{
    AssetMode, FixDisposition, FixRow, RunConfig, Scenario, ScheduledAction, ScriptedAction,
    SpawnSpec, Trace, run,
};
use pcx_sim::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RplError {
    pub line: usize,
    pub command: String,
    pub message: String,
}
impl std::fmt::Display for RplError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "line {} command {:?}: {}",
            self.line, self.command, self.message
        )
    }
}
impl std::error::Error for RplError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expectation {
    Units {
        tick: u64,
        count: usize,
    },
    Unit {
        tick: u64,
        id: u32,
        owner: u8,
        x: i32,
        y: i32,
        target: Option<(i32, i32)>,
    },
    Timer {
        tick: u64,
        index: usize,
        name: String,
    },
    RngDraws {
        tick: u64,
        count: usize,
    },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rpl {
    pub scenario: Scenario,
    pub config: RunConfig,
    pub expectations: Vec<(usize, Expectation)>,
}
impl Rpl {
    pub fn parse(source: &str) -> Result<Self, RplError> {
        let mut name = None;
        let mut map = None;
        let mut ticks = None;
        let mut seed = None;
        let mut mode = None;
        let mut spawns = Vec::new();
        let mut script = Vec::new();
        let mut expectations = Vec::new();
        let mut fixes = Vec::new();
        let mut header = false;
        for (index, line) in source.lines().enumerate() {
            let words: Vec<_> = line
                .split('#')
                .next()
                .unwrap_or("")
                .split_whitespace()
                .collect();
            if words.is_empty() {
                continue;
            }
            let line = index + 1;
            let error = |message: String| RplError {
                line,
                command: words[0].into(),
                message,
            };
            if !header {
                if words != ["pcx-rpl", "1"] {
                    return Err(error("expected pcx-rpl 1".into()));
                }
                header = true;
                continue;
            }
            let result = (|| -> Result<(), String> {
                match words.as_slice() {
                    ["scenario", value] => set(&mut name, value.to_string()),
                    ["map", value] => set(&mut map, value.to_string()),
                    ["ticks", value] => set(&mut ticks, number(value)?),
                    ["seed", value] => set(&mut seed, number(value)?),
                    ["asset-mode", value] => set(
                        &mut mode,
                        match *value {
                            "vanilla" => AssetMode::Vanilla,
                            "hd" => AssetMode::Hd,
                            "reforged" => AssetMode::Reforged,
                            _ => return Err(format!("unknown asset mode {value:?}")),
                        },
                    ),
                    ["fix", disposition, id] => {
                        if fixes.iter().any(|f: &FixRow| f.id == *id) {
                            return Err(format!("duplicate fix {id:?}"));
                        }
                        fixes.push(FixRow {
                            id: id.to_string(),
                            disposition: match *disposition {
                                "adopt" => FixDisposition::Adopt,
                                "reject" => FixDisposition::Reject,
                                "defer" => FixDisposition::Defer,
                                _ => return Err(format!("unknown disposition {disposition:?}")),
                            },
                        });
                        Ok(())
                    }
                    ["spawn", owner, x, y] => {
                        spawns.push(SpawnSpec {
                            owner: number(owner)?,
                            x: number(x)?,
                            y: number(y)?,
                        });
                        Ok(())
                    }
                    ["at", tick, name, "rng"] => {
                        script.push(ScheduledAction {
                            tick: number(tick)?,
                            name: name.to_string(),
                            action: ScriptedAction::RngDraw,
                        });
                        Ok(())
                    }
                    ["at", tick, name, player, command, args @ ..] => {
                        let command = match (*command, args) {
                            ("move", [x, y]) => Command::Move {
                                x: number(x)?,
                                y: number(y)?,
                            },
                            ("select", [x, y]) => Command::Select {
                                x: number(x)?,
                                y: number(y)?,
                            },
                            ("stop", []) => Command::Stop,
                            ("nop", []) => Command::Nop,
                            _ => {
                                return Err(format!(
                                    "unsupported command {command:?} or invalid arguments"
                                ));
                            }
                        };
                        script.push(ScheduledAction {
                            tick: number(tick)?,
                            name: name.to_string(),
                            action: ScriptedAction::Commands {
                                player: number(player)?,
                                commands: vec![command],
                            },
                        });
                        Ok(())
                    }
                    ["expect", tick, "units", count] => {
                        expectations.push((
                            line,
                            Expectation::Units {
                                tick: number(tick)?,
                                count: number(count)?,
                            },
                        ));
                        Ok(())
                    }
                    ["expect", tick, "rng-draws", count] => {
                        expectations.push((
                            line,
                            Expectation::RngDraws {
                                tick: number(tick)?,
                                count: number(count)?,
                            },
                        ));
                        Ok(())
                    }
                    ["expect", tick, "timer", index, name] => {
                        expectations.push((
                            line,
                            Expectation::Timer {
                                tick: number(tick)?,
                                index: number(index)?,
                                name: name.to_string(),
                            },
                        ));
                        Ok(())
                    }
                    ["expect", tick, "unit", id, owner, x, y, target @ ..] => {
                        let target = match target {
                            ["idle"] => None,
                            ["target", x, y] => Some((number(x)?, number(y)?)),
                            _ => return Err("expected idle or target <x> <y>".into()),
                        };
                        expectations.push((
                            line,
                            Expectation::Unit {
                                tick: number(tick)?,
                                id: number(id)?,
                                owner: number(owner)?,
                                x: number(x)?,
                                y: number(y)?,
                                target,
                            },
                        ));
                        Ok(())
                    }
                    _ => Err(format!(
                        "unsupported record {:?} or invalid arguments",
                        words.join(" ")
                    )),
                }
            })();
            result.map_err(error)?;
        }
        let missing = |field: &str| RplError {
            line: source.lines().count() + 1,
            command: field.into(),
            message: "required field is missing".into(),
        };
        if !header {
            return Err(missing("pcx-rpl"));
        }
        if expectations.is_empty() {
            return Err(missing("expect"));
        }
        let scenario = Scenario {
            name: name.ok_or_else(|| missing("scenario"))?,
            map: map.ok_or_else(|| missing("map"))?,
            ticks: ticks.ok_or_else(|| missing("ticks"))?,
            spawns,
            script,
        };
        for (line, expectation) in &expectations {
            let tick = match expectation {
                Expectation::Units { tick, .. }
                | Expectation::Unit { tick, .. }
                | Expectation::Timer { tick, .. }
                | Expectation::RngDraws { tick, .. } => *tick,
            };
            if tick >= scenario.ticks {
                return Err(RplError {
                    line: *line,
                    command: "expect".into(),
                    message: format!("tick {tick} outside {} ticks", scenario.ticks),
                });
            }
        }
        Ok(Self {
            scenario,
            config: RunConfig {
                asset_mode: mode.ok_or_else(|| missing("asset-mode"))?,
                seed: seed.ok_or_else(|| missing("seed"))?,
                fix_rows: fixes,
            },
            expectations,
        })
    }

    pub fn run(&self) -> Result<Trace, String> {
        let trace = run(&self.scenario, &self.config).map_err(|e| e.to_string())?;
        self.check(&trace)?;
        Ok(trace)
    }

    pub fn check(&self, trace: &Trace) -> Result<(), String> {
        for (line, expected) in &self.expectations {
            let tick = match expected {
                Expectation::Units { tick, .. }
                | Expectation::Unit { tick, .. }
                | Expectation::Timer { tick, .. }
                | Expectation::RngDraws { tick, .. } => *tick,
            };
            let record = trace
                .ticks
                .get(usize::try_from(tick).map_err(|_| format!("tick {tick} too large"))?)
                .ok_or_else(|| format!("line {line}: missing tick {tick}"))?;
            let pass = match expected {
                Expectation::Units { count, .. } => record.units.len() == *count,
                Expectation::RngDraws { count, .. } => record.rng_draws.len() == *count,
                Expectation::Timer { index, name, .. } => {
                    record.timers.get(*index).is_some_and(|t| t.name == *name)
                }
                Expectation::Unit {
                    id,
                    owner,
                    x,
                    y,
                    target,
                    ..
                } => record.units.iter().any(|u| {
                    u.id == *id
                        && u.owner == *owner
                        && u.x == *x
                        && u.y == *y
                        && u.target == *target
                }),
            };
            if !pass {
                return Err(format!(
                    "line {line}: expectation {expected:?} failed; actual {record:?}"
                ));
            }
        }
        Ok(())
    }
}
fn number<T: std::str::FromStr>(value: &str) -> Result<T, String> {
    value
        .parse()
        .map_err(|_| format!("invalid integer {value:?}"))
}
fn set<T>(slot: &mut Option<T>, value: T) -> Result<(), String> {
    if slot.is_some() {
        return Err("duplicate field".into());
    }
    *slot = Some(value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const FIXTURE: &str = include_str!("../../../tests/play/synthetic/ordering/same-tick.rpl");
    #[test]
    fn declaration_order_assertions_and_100_independent_runs() {
        let rpl = Rpl::parse(FIXTURE).unwrap();
        let first = rpl.run().unwrap().encode();
        for _ in 0..100 {
            assert_eq!(rpl.run().unwrap().encode(), first);
        }
    }
    #[test]
    fn refuses_unsupported_duplicate_overflow_and_bad_expectations() {
        for (from, to) in [
            ("stop", "attack"),
            ("seed 7", "seed 7\nseed 8"),
            ("spawn 0", "spawn 256"),
            ("expect 2", "expect 3"),
        ] {
            let error = Rpl::parse(&FIXTURE.replace(from, to)).unwrap_err();
            assert!(error.line > 0);
            assert!(!error.command.is_empty());
        }
        let mut rpl = Rpl::parse(FIXTURE).unwrap();
        rpl.expectations
            .push((99, Expectation::Units { tick: 0, count: 4 }));
        assert!(rpl.run().unwrap_err().contains("line 99"));
    }
}

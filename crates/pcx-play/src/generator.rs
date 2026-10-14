//! Version-one seeded synthetic movement scenarios, emitted as strict RPL.
//!
//! This is a harness convention, not the game's RNG, unit composition or terrain.
//! Generation and execution each start a separate `pcx_sim::Rng` at the supplied
//! seed. Modulo selection is reproducible; it is not an unbiased distribution.

use std::{alloc::Layout, fmt::Write, ops::RangeInclusive};

use pcx_sim::{Fx, Rng};

use crate::{RunConfig, is_valid_name, trace::TickRecord};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionKind {
    Move,
    Select,
    Stop,
    Nop,
    RngDraw,
}

/// Every domain is caller policy. Owners are synthetic u8 identifiers, not a
/// claim about the native game's player slots. Coordinates are raw Fx words.
#[derive(Debug, Clone)]
pub struct MovementSpec<'a> {
    pub scenario: &'a str,
    pub map: &'a str,
    pub ticks: u64,
    pub units: usize,
    pub actions: usize,
    pub owners: &'a [u8],
    pub x: RangeInclusive<Fx>,
    pub y: RangeInclusive<Fx>,
    pub action_kinds: &'a [ActionKind],
}

/// Explicit generation/output ceilings. Work counts ticks * (units + 1) +
/// actions, matching playrun's single-run estimate. It does not bound compiler,
/// runner or oracle memory, or establish a frame-time limit.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub max_units: usize,
    pub max_actions: usize,
    pub max_ticks: u64,
    pub max_fix_rows: usize,
    pub max_work: u64,
    pub max_output_bytes: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid generation field {field}: {reason}")]
    Invalid {
        field: &'static str,
        reason: &'static str,
    },
    #[error("generation {field} requires {requested}, limit is {limit}")]
    Limit {
        field: &'static str,
        requested: u128,
        limit: u128,
    },
    #[error("generation {field} exceeds the platform's layout")]
    Layout { field: &'static str },
    #[error("cannot allocate generated RPL: {0}")]
    Allocation(#[source] std::collections::TryReserveError),
}

/// Generate canonical RPL text with a final unit-count assertion. No baseline
/// positions or native outcomes are invented. Same spec/config means identical
/// bytes; changing a domain or algorithm requires a new corpus recipe version.
pub fn generate(
    spec: &MovementSpec<'_>,
    config: &RunConfig,
    limits: Limits,
) -> Result<String, Error> {
    validate(spec, config, limits)?;
    let mut out = Output {
        text: String::new(),
        limit: limits.max_output_bytes,
        error: None,
    };
    out.line(format_args!(
        "pcx-rpl 1\n# generator movement-v1; synthetic, modulo-selected domains\nscenario {}\nmap {}\nticks {}\nseed {}\nasset-mode {}\n",
        spec.scenario, spec.map, spec.ticks, config.seed, config.asset_mode.as_str(),
    ))?;
    // The caller's fix order is preserved. The trace runner canonicalizes it.
    for fix in &config.fix_rows {
        out.line(format_args!(
            "fix {} {}\n",
            fix.disposition.as_str(),
            fix.id
        ))?;
    }
    let mut rng = Rng::new(config.seed);
    for _ in 0..spec.units {
        let owner = spec.owners[choose(&mut rng, spec.owners.len() as u64) as usize];
        let x = coordinate(&mut rng, &spec.x);
        let y = coordinate(&mut rng, &spec.y);
        out.line(format_args!("spawn {owner} {x} {y}\n"))?;
    }
    for index in 0..spec.actions {
        let tick = choose(&mut rng, spec.ticks);
        let kind = spec.action_kinds[choose(&mut rng, spec.action_kinds.len() as u64) as usize];
        out.line(format_args!("at {tick} generated-{index} "))?;
        if kind == ActionKind::RngDraw {
            out.line(format_args!("rng\n"))?;
            continue;
        }
        let owner = spec.owners[choose(&mut rng, spec.owners.len() as u64) as usize];
        match kind {
            ActionKind::Move | ActionKind::Select => {
                let command = if kind == ActionKind::Move {
                    "move"
                } else {
                    "select"
                };
                let x = coordinate(&mut rng, &spec.x);
                let y = coordinate(&mut rng, &spec.y);
                out.line(format_args!("{owner} {command} {x} {y}\n"))?;
            }
            ActionKind::Stop => out.line(format_args!("{owner} stop\n"))?,
            ActionKind::Nop => out.line(format_args!("{owner} nop\n"))?,
            ActionKind::RngDraw => unreachable!("handled before owner selection"),
        }
    }
    out.line(format_args!(
        "expect {} units {}\n",
        spec.ticks - 1,
        spec.units
    ))?;
    Ok(out.text)
}

fn invalid(field: &'static str, reason: &'static str) -> Error {
    Error::Invalid { field, reason }
}

fn ceiling(field: &'static str, requested: u128, limit: u128) -> Result<(), Error> {
    if requested > limit {
        Err(Error::Limit {
            field,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn validate(spec: &MovementSpec<'_>, config: &RunConfig, limits: Limits) -> Result<(), Error> {
    for (field, name) in [("scenario", spec.scenario), ("map", spec.map)] {
        if !is_valid_name(name) {
            return Err(invalid(field, "not a canonical name"));
        }
    }
    ceiling("units", spec.units as u128, limits.max_units as u128)?;
    ceiling("actions", spec.actions as u128, limits.max_actions as u128)?;
    ceiling("ticks", spec.ticks.into(), limits.max_ticks.into())?;
    ceiling(
        "fix rows",
        config.fix_rows.len() as u128,
        limits.max_fix_rows as u128,
    )?;
    if spec.ticks == 0 {
        return Err(invalid("ticks", "must be positive for an RPL assertion"));
    }
    // World::spawn increments a u32 source after assigning each identifier.
    ceiling(
        "unit identifiers",
        spec.units as u128,
        u128::from(u32::MAX - 1),
    )?;
    let ticks = usize::try_from(spec.ticks).map_err(|_| Error::Layout { field: "ticks" })?;
    Layout::array::<TickRecord>(ticks).map_err(|_| Error::Layout { field: "ticks" })?;
    let work = u128::from(spec.ticks) * (spec.units as u128 + 1) + spec.actions as u128;
    ceiling("work", work, limits.max_work.into())?;
    if spec.owners.is_empty() {
        return Err(invalid("owners", "must not be empty"));
    }
    let mut owners = [false; 256];
    for &owner in spec.owners {
        if std::mem::replace(&mut owners[usize::from(owner)], true) {
            return Err(invalid("owners", "duplicate identifier"));
        }
    }
    if spec.actions > 0 && spec.action_kinds.is_empty() {
        return Err(invalid(
            "action kinds",
            "must not be empty when actions are requested",
        ));
    }
    let mut kinds = [false; 5];
    for &kind in spec.action_kinds {
        if std::mem::replace(&mut kinds[kind as usize], true) {
            return Err(invalid("action kinds", "duplicate choice"));
        }
    }
    for (field, range) in [("x", &spec.x), ("y", &spec.y)] {
        if range.start() > range.end() {
            return Err(invalid(field, "reversed inclusive range"));
        }
    }
    for (index, fix) in config.fix_rows.iter().enumerate() {
        if !is_valid_name(&fix.id) {
            return Err(invalid("fix rows", "not a canonical name"));
        }
        if config.fix_rows[..index]
            .iter()
            .any(|prior| prior.id == fix.id)
        {
            return Err(invalid("fix rows", "duplicate identifier"));
        }
    }
    Ok(())
}

fn choose(rng: &mut Rng, size: u64) -> u64 {
    rng.next_u64() % size
}

fn coordinate(rng: &mut Rng, range: &RangeInclusive<Fx>) -> Fx {
    let lower = i64::from(*range.start());
    let span = (i64::from(*range.end()) - lower + 1) as u64;
    // Both bounds are i32, so even the complete signed domain fits this i64 sum.
    (lower + choose(rng, span) as i64) as Fx
}

struct Output {
    text: String,
    limit: usize,
    error: Option<Error>,
}

impl Output {
    fn line(&mut self, args: std::fmt::Arguments<'_>) -> Result<(), Error> {
        self.write_fmt(args)
            .map_err(|_| self.error.take().expect("write_str records its error"))
    }
}

impl Write for Output {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        let requested = self.text.len() as u128 + value.len() as u128;
        let result = ceiling("output bytes", requested, self.limit as u128).and_then(|()| {
            let length = usize::try_from(requested).map_err(|_| Error::Layout {
                field: "output bytes",
            })?;
            Layout::array::<u8>(length).map_err(|_| Error::Layout {
                field: "output bytes",
            })?;
            self.text
                .try_reserve_exact(value.len())
                .map_err(Error::Allocation)?;
            self.text.push_str(value);
            Ok(())
        });
        if let Err(error) = result {
            self.error = Some(error);
            Err(std::fmt::Error)
        } else {
            Ok(())
        }
    }
}

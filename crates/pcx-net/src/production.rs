//! Production authority: the admitted map's startup, driven by peer command batches.
//!
//! The authority owns one [`Startup`]. A peer may join only as a human slot of the admitted map's
//! roster. Each batch is applied once, at the clock tick it names, and only after the authority
//! has checked the batch against the player's control, pause and ownership state. A script's own
//! order natives are not that check, because authored cinematics run them while player input is
//! off. A refused batch changes nothing; its refusals go to a bounded log.
//!
//! Scope of the command mapping: `Move` orders every owned unit whose class is movable, through
//! the same `IssuePointOrder` native the application uses. Selection and stop orders are refused
//! until the wire format carries selected units. Session identity is the startup's admission
//! digest, so unit snapshots are not the identity.
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use pcx_play::headless_startup::{Startup, StartupError};
use pcx_script::{Host, Value};
use pcx_sim::{Command, FX_ONE, Fx, MAX_INPUT_LEAD, Unit, World};

use crate::{
    Datagram, Peer, encode, encode_world_payload,
    session::{
        COORDINATE_LIMIT, Packet, SessionAuthority, SessionError, SessionIdentity, parse_packet,
    },
};

/// Units per snapshot. A unit is at most 23 wire bytes, so 1024 units fit the host's 64 KiB write
/// buffer with room for the header.
pub const MAX_SNAPSHOT_UNITS: usize = 1024;

/// Refusals kept for inspection. The oldest drop first, and the count of drops stays.
pub const MAX_REFUSALS: usize = 256;

/// A batch or command the authority declined.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub tick: u64,
    pub player: u8,
    pub reason: String,
}

/// The bounded refusal log.
#[derive(Debug, Default)]
pub struct RefusalLog {
    entries: VecDeque<Refusal>,
    dropped: u64,
}

impl RefusalLog {
    fn push(&mut self, refusal: Refusal) {
        if self.entries.len() == MAX_REFUSALS {
            self.entries.pop_front();
            self.dropped += 1;
        }
        self.entries.push_back(refusal);
    }

    pub fn entries(&self) -> impl Iterator<Item = &Refusal> {
        self.entries.iter()
    }

    /// Refusals dropped from the front of the log since the authority started.
    pub fn dropped(&self) -> u64 {
        self.dropped
    }
}

pub struct ProductionAuthority {
    startup: Startup,
    identity: SessionIdentity,
    roster: BTreeSet<u8>,
    peers: BTreeMap<Peer, u8>,
    /// Batches by the tick they apply at, then by player. Each batch is removed once applied.
    inputs: BTreeMap<u64, BTreeMap<u8, Vec<Command>>>,
    refusals: RefusalLog,
    /// Set when an order native failed after earlier orders of the same batch had applied. The
    /// authority stops rather than continue from a partially applied batch.
    failed: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum AuthorityError {
    #[error("the admitted roster lists a player that does not fit the wire: {0}")]
    Roster(String),
    #[error("startup: {0}")]
    Startup(#[from] StartupError),
}

impl ProductionAuthority {
    /// Takes the admitted startup. The roster is read from the map's own player records, and
    /// the identity is the startup's admission digest.
    pub fn new(startup: Startup) -> Result<Self, AuthorityError> {
        let mut roster = BTreeSet::new();
        for human in startup.context.world.info.human_players() {
            let slot = u8::try_from(human.number())
                .map_err(|_| AuthorityError::Roster(format!("slot {}", human.number())))?;
            roster.insert(slot);
        }
        let identity = SessionIdentity(startup.admission_digest());
        Ok(Self {
            startup,
            identity,
            roster,
            peers: BTreeMap::new(),
            inputs: BTreeMap::new(),
            refusals: RefusalLog::default(),
            failed: None,
        })
    }

    /// The admitted human slots, in ascending order.
    pub fn roster(&self) -> impl Iterator<Item = u8> + '_ {
        self.roster.iter().copied()
    }

    pub fn refusals(&self) -> &RefusalLog {
        &self.refusals
    }

    /// Clock ticks since publication.
    pub fn tick(&self) -> u64 {
        self.startup.tick()
    }

    fn refuse(&mut self, tick: u64, player: u8, reason: impl Into<String>) {
        self.refusals.push(Refusal {
            tick,
            player,
            reason: reason.into(),
        });
    }

    /// Checks one player's batch for `tick` against the host's control, pause and ownership
    /// state. Returns the reason to refuse, or `None` when every command may be issued.
    fn gate(&mut self, player: u8, commands: &[Command]) -> Option<String> {
        let host = self.startup.session.host()?;
        let objects = host.objects();
        if objects.game_paused() == Some(true) {
            return Some("the game is paused".into());
        }
        if objects
            .presentation()
            .is_some_and(|state| state.control_enabled == Some(false))
        {
            return Some("user control is disabled by the script".into());
        }
        for command in commands {
            match command {
                Command::Nop => {}
                Command::Move { .. } => {
                    let movable = objects.executed_units().any(|(_, unit)| {
                        unit.executed_owner == i32::from(player)
                            && unit.activity.as_ref().is_some_and(|a| a.movable())
                    });
                    if !movable {
                        return Some(format!("player {player} owns no movable unit"));
                    }
                }
                Command::Select { .. } | Command::Stop => {
                    return Some(
                        "selection and stop orders are not implemented by the production authority"
                            .into(),
                    );
                }
            }
        }
        None
    }

    /// Applies one admitted batch at the current tick, all or nothing for its checks.
    fn apply(&mut self, tick: u64, player: u8, commands: &[Command]) -> Result<(), SessionError> {
        if let Some(reason) = self.gate(player, commands) {
            self.refuse(tick, player, reason);
            return Ok(());
        }
        for command in commands {
            let Command::Move { x, y } = command else {
                continue;
            };
            let target = [world_units(*x), world_units(*y)];
            let units: Vec<_> = self
                .startup
                .session
                .host()
                .ok_or_else(|| SessionError::Failed("no admitted session".into()))?
                .objects()
                .executed_units()
                .filter(|(_, unit)| {
                    unit.executed_owner == i32::from(player)
                        && unit.activity.as_ref().is_some_and(|a| a.movable())
                })
                .map(|(id, _)| id)
                .collect();
            for unit in units {
                let result = self
                    .startup
                    .session
                    .host()
                    .ok_or_else(|| SessionError::Failed("no admitted session".into()))?
                    .objects_mut()
                    .native(
                        "IssuePointOrder",
                        &[
                            Value::Handle(unit),
                            Value::Str("move".into()),
                            Value::Real(target[0]),
                            Value::Real(target[1]),
                        ],
                    );
                match result {
                    Ok(Value::Bool(true)) => {}
                    other => {
                        let reason = format!("order native returned {other:?}");
                        self.failed = Some(reason.clone());
                        return Err(SessionError::Failed(reason));
                    }
                }
            }
        }
        Ok(())
    }

    /// The wire snapshot of the current production state, with checked handle and value mapping.
    fn export(&mut self) -> Result<Vec<u8>, SessionError> {
        let tick = self.startup.tick();
        let host = self
            .startup
            .session
            .host()
            .ok_or_else(|| SessionError::Failed("no admitted session".into()))?;
        let objects = host.objects();
        let selected: BTreeSet<u64> = objects.selection().iter().map(|id| id.token()).collect();
        let mut units = Vec::new();
        for (handle, unit) in objects.executed_units() {
            let id = u32::try_from(handle.token()).map_err(|_| {
                SessionError::Failed(format!("handle {} exceeds the wire id", handle.token()))
            })?;
            let owner = u8::try_from(unit.executed_owner).map_err(|_| {
                SessionError::Failed(format!(
                    "owner {} exceeds the wire owner",
                    unit.executed_owner
                ))
            })?;
            let [x, y] = unit.position.unwrap_or([unit.x, unit.y]);
            let move_target = match unit.activity.as_ref().filter(|a| a.moving()) {
                Some(activity) => match activity.destination() {
                    Some(point) => Some((fixed(point[0])?, fixed(point[1])?)),
                    None => None,
                },
                None => None,
            };
            units.push(Unit {
                id,
                owner,
                selected: selected.contains(&handle.token()),
                x: fixed(x)?,
                y: fixed(y)?,
                move_target,
            });
        }
        if units.len() > MAX_SNAPSHOT_UNITS {
            return Err(SessionError::Failed(format!(
                "{} units exceed the snapshot bound {MAX_SNAPSHOT_UNITS}",
                units.len()
            )));
        }
        let next_unit_id = units
            .iter()
            .map(|unit| unit.id)
            .max()
            .map_or(Some(1), |max| max.checked_add(1))
            .ok_or_else(|| SessionError::Failed("unit id space exhausted".into()))?;
        let world = World {
            tick,
            units,
            next_unit_id,
            movement: None,
        };
        Ok(encode(&Datagram::Snapshot {
            tick,
            payload: encode_world_payload(&world),
        }))
    }
}

/// A wire fixed-point coordinate as the production world's `f32` units. The conversion loses
/// precision above 2^8 world units at the 1/65536 wire step.
fn world_units(value: Fx) -> f32 {
    value as f32 / FX_ONE as f32
}

/// An `f32` production coordinate as wire fixed point, refusing values outside the wire range.
fn fixed(value: f32) -> Result<Fx, SessionError> {
    let scaled = (f64::from(value) * f64::from(FX_ONE)).round();
    if !scaled.is_finite() || scaled.abs() > f64::from(COORDINATE_LIMIT) {
        return Err(SessionError::Failed(format!(
            "coordinate {value} is outside the wire range"
        )));
    }
    Ok(scaled as Fx)
}

impl SessionAuthority for ProductionAuthority {
    fn identity(&self) -> SessionIdentity {
        self.identity.clone()
    }

    fn join(&mut self, peer: Peer, player: u8) -> Result<(), SessionError> {
        if !self.roster.contains(&player) {
            return Err(SessionError::NotAdmitted { player });
        }
        if self.peers.contains_key(&peer) || self.peers.values().any(|p| *p == player) {
            return Err(SessionError::Identity);
        }
        self.peers.insert(peer, player);
        Ok(())
    }

    fn leave(&mut self, peer: Peer) {
        if let Some(player) = self.peers.remove(&peer) {
            for batches in self.inputs.values_mut() {
                batches.remove(&player);
            }
            self.inputs.retain(|_, batches| !batches.is_empty());
        }
    }

    fn receive(&mut self, peer: Peer, bytes: &[u8]) -> Result<Option<Vec<u8>>, SessionError> {
        let player = *self.peers.get(&peer).ok_or(SessionError::Identity)?;
        match parse_packet(bytes, player)? {
            Packet::Input { tick, commands } => {
                let now = self.startup.tick();
                if tick < now {
                    return Err(SessionError::Tick(format!(
                        "tick {tick} has already run; the clock is at {now}"
                    )));
                }
                if tick - now > MAX_INPUT_LEAD {
                    return Err(SessionError::Tick(format!(
                        "tick {tick} is more than {MAX_INPUT_LEAD} ticks ahead of {now}"
                    )));
                }
                if self
                    .inputs
                    .get(&tick)
                    .is_some_and(|batches| batches.contains_key(&player))
                {
                    return Err(SessionError::Duplicate);
                }
                self.inputs
                    .entry(tick)
                    .or_default()
                    .insert(player, commands);
                Ok(None)
            }
            Packet::Resync => Ok(Some(self.export()?)),
        }
    }

    fn advance(&mut self) -> Result<Vec<u8>, SessionError> {
        if let Some(reason) = &self.failed {
            return Err(SessionError::Failed(reason.clone()));
        }
        let now = self.startup.tick();
        let batches = self.inputs.remove(&now).unwrap_or_default();
        for (player, commands) in batches {
            self.apply(now, player, &commands)?;
        }
        self.startup
            .step_clock(1)
            .map_err(|error| SessionError::Failed(error.to_string()))?;
        self.export()
    }

    fn snapshot(&mut self) -> Result<Vec<u8>, SessionError> {
        self.export()
    }

    fn player_for_new_token(&self, bound: usize) -> Option<u8> {
        self.roster.iter().nth(bound).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refusal(tick: u64) -> Refusal {
        Refusal {
            tick,
            player: 1,
            reason: format!("refusal {tick}"),
        }
    }

    #[test]
    fn the_refusal_log_keeps_the_newest_entries_and_counts_the_dropped_ones() {
        let mut log = RefusalLog::default();
        for tick in 0..(MAX_REFUSALS as u64 + 3) {
            log.push(refusal(tick));
        }
        assert_eq!(log.entries().count(), MAX_REFUSALS);
        assert_eq!(log.dropped(), 3);
        assert_eq!(log.entries().next().map(|r| r.tick), Some(3));
        assert_eq!(
            log.entries().last().map(|r| r.tick),
            Some(MAX_REFUSALS as u64 + 2)
        );
    }

    #[test]
    fn wire_fixed_point_round_trips_within_the_f32_step_and_refuses_out_of_range() {
        assert_eq!(fixed(0.0).unwrap(), 0);
        assert_eq!(fixed(-1.0).unwrap(), -FX_ONE);
        assert_eq!(world_units(FX_ONE * 64), 64.0);
        assert!(fixed(f32::NAN).is_err());
        assert!(fixed(COORDINATE_LIMIT as f32).is_err());
    }
}

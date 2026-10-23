//! Production authority: the admitted map's startup, driven by peer command batches.
//!
//! The authority owns one [`Startup`]. A peer may join only as a human slot of the admitted map's
//! roster. Each batch is applied once, at the clock tick it names, and only after the authority
//! has checked the batch against the player's control, pause and ownership state. A script's own
//! order natives are not that check, because authored cinematics run them while player input is
//! off. A refused batch changes nothing; its refusals go to a bounded log.
//!
//! Commands map to the application's own path: `Select` replaces the sending player's selection
//! with the live owned units inside the harness selection box, and `Move`/`Stop` order only that
//! selection through the admitted source order registry. Selection is held per player, so two
//! peers never replace each other's choice, and a unit is exported as selected only when its own
//! owner selected it. A player who has not selected yet keeps the harness default: every owned
//! unit is selected, preserving move-before-select. Stale handles are dropped before any native
//! call; foreign hands can never enter a selection. Session identity is the startup's admission
//! digest, so unit snapshots are not the identity.
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use pcx_play::headless_startup::{Startup, StartupError};
use pcx_play::playercolor_host::{ExecutedUnit, ObjectHost};
use pcx_play::standard_command::Kind;
use pcx_script::{HandleId, Host, Value};
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

/// Units one player may select or order in one batch. The snapshot bound is the same, so a
/// selection can never ask for more units than a snapshot can carry.
pub const MAX_SELECTION_UNITS: usize = MAX_SNAPSHOT_UNITS;

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
    /// Explicit selection by player. A missing entry means the harness default: every unit the
    /// player owns. An entry, including an empty one, replaces that default.
    selections: BTreeMap<u8, BTreeSet<HandleId>>,
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
            selections: BTreeMap::new(),
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

    /// Drops selected handles that no longer name a unit owned by the player. Removed, foreign
    /// and unmapped handles never reach a native call.
    fn prune_selection(&mut self, player: u8) {
        let Some(selected) = self.selections.get_mut(&player) else {
            return;
        };
        let Some(host) = self.startup.session.host() else {
            selected.clear();
            return;
        };
        let objects = host.objects();
        selected.retain(|id| {
            objects
                .executed_unit(*id)
                .is_ok_and(|unit| unit.executed_owner == i32::from(player))
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
        let mut current = self.selections.get(&player).cloned();
        for command in commands {
            match command {
                Command::Nop => {}
                Command::Select { x, y } => {
                    let point = [world_units(*x), world_units(*y)];
                    match units_at(objects, player, point) {
                        Ok(chosen) => current = Some(chosen),
                        Err(reason) => return Some(reason),
                    }
                }
                Command::Move { .. } => {
                    match orderable_units(objects, current.as_ref(), player, Kind::Move) {
                        Ok(units) if !units.is_empty() => {}
                        Ok(_) => {
                            return Some(format!("player {player} has no selected movable unit"));
                        }
                        Err(reason) => return Some(reason),
                    }
                }
                Command::Stop => {
                    match orderable_units(objects, current.as_ref(), player, Kind::Stop) {
                        Ok(units) if !units.is_empty() => {}
                        Ok(_) => {
                            return Some(format!("player {player} has no selected unit to stop"));
                        }
                        Err(reason) => return Some(reason),
                    }
                }
            }
        }
        None
    }

    /// Applies one admitted batch at the current tick, all or nothing for its checks. Commands
    /// keep their batch order, so a `Select` before a `Move` selects before the order runs.
    fn apply(&mut self, tick: u64, player: u8, commands: &[Command]) -> Result<(), SessionError> {
        self.prune_selection(player);
        if let Some(reason) = self.gate(player, commands) {
            self.refuse(tick, player, reason);
            return Ok(());
        }
        for command in commands {
            match command {
                Command::Nop => {}
                Command::Select { x, y } => {
                    let point = [world_units(*x), world_units(*y)];
                    let chosen = {
                        let host =
                            self.startup.session.host().ok_or_else(|| {
                                SessionError::Failed("no admitted session".into())
                            })?;
                        units_at(host.objects(), player, point).map_err(SessionError::Failed)?
                    };
                    self.selections.insert(player, chosen);
                }
                Command::Move { x, y } => {
                    let target = [world_units(*x), world_units(*y)];
                    let units = {
                        let host =
                            self.startup.session.host().ok_or_else(|| {
                                SessionError::Failed("no admitted session".into())
                            })?;
                        orderable_units(
                            host.objects(),
                            self.selections.get(&player),
                            player,
                            Kind::Move,
                        )
                        .map_err(SessionError::Failed)?
                    };
                    if units.is_empty() {
                        let reason =
                            format!("player {player} has no selected movable unit to order");
                        self.failed = Some(reason.clone());
                        return Err(SessionError::Failed(reason));
                    }
                    self.issue_point_orders(player, &units, target)?;
                }
                Command::Stop => {
                    let units = {
                        let host =
                            self.startup.session.host().ok_or_else(|| {
                                SessionError::Failed("no admitted session".into())
                            })?;
                        orderable_units(
                            host.objects(),
                            self.selections.get(&player),
                            player,
                            Kind::Stop,
                        )
                        .map_err(SessionError::Failed)?
                    };
                    if units.is_empty() {
                        let reason = format!("player {player} has no selected unit to stop");
                        self.failed = Some(reason.clone());
                        return Err(SessionError::Failed(reason));
                    }
                    self.issue_stop_orders(player, &units)?;
                }
            }
        }
        Ok(())
    }

    /// Issues `Move` to each unit through the admitted `CmdMove` identifier.
    fn issue_point_orders(
        &mut self,
        player: u8,
        units: &[HandleId],
        target: [f32; 2],
    ) -> Result<(), SessionError> {
        let host = self
            .startup
            .session
            .host()
            .ok_or_else(|| SessionError::Failed("no admitted session".into()))?;
        let objects = host.objects_mut();
        for &unit in units {
            let order = match objects.standard_command_order(unit, Kind::Move) {
                Ok(Some(order)) => order,
                Ok(None) => {
                    let reason = format!(
                        "player {player}: unit {} no longer admits a move",
                        unit.token()
                    );
                    self.failed = Some(reason.clone());
                    return Err(SessionError::Failed(reason));
                }
                Err(error) => {
                    let reason = format!("player {player}: move order lookup failed: {error}");
                    self.failed = Some(reason.clone());
                    return Err(SessionError::Failed(reason));
                }
            };
            match objects.native(
                "IssuePointOrderById",
                &[
                    Value::Handle(unit),
                    Value::Int(order),
                    Value::Real(target[0]),
                    Value::Real(target[1]),
                ],
            ) {
                Ok(Value::Bool(true)) => {}
                other => {
                    let reason = format!("player {player}: move native returned {other:?}");
                    self.failed = Some(reason.clone());
                    return Err(SessionError::Failed(reason));
                }
            }
        }
        Ok(())
    }

    /// Issues `Stop` to each unit through the admitted `CmdStop` identifier.
    fn issue_stop_orders(&mut self, player: u8, units: &[HandleId]) -> Result<(), SessionError> {
        let host = self
            .startup
            .session
            .host()
            .ok_or_else(|| SessionError::Failed("no admitted session".into()))?;
        let objects = host.objects_mut();
        for &unit in units {
            let order = match objects.standard_command_order(unit, Kind::Stop) {
                Ok(Some(order)) => order,
                Ok(None) => {
                    let reason = format!(
                        "player {player}: unit {} no longer admits a stop",
                        unit.token()
                    );
                    self.failed = Some(reason.clone());
                    return Err(SessionError::Failed(reason));
                }
                Err(error) => {
                    let reason = format!("player {player}: stop order lookup failed: {error}");
                    self.failed = Some(reason.clone());
                    return Err(SessionError::Failed(reason));
                }
            };
            match objects.native(
                "IssueImmediateOrderById",
                &[Value::Handle(unit), Value::Int(order)],
            ) {
                Ok(Value::Bool(true)) => {}
                other => {
                    let reason = format!("player {player}: stop native returned {other:?}");
                    self.failed = Some(reason.clone());
                    return Err(SessionError::Failed(reason));
                }
            }
        }
        Ok(())
    }

    /// The wire snapshot of the current production state, with checked handle and value mapping.
    /// A unit is selected when its own player selected it; a human player who has not selected
    /// keeps the harness default of every owned unit.
    fn export(&mut self) -> Result<Vec<u8>, SessionError> {
        let tick = self.startup.tick();
        let host = self
            .startup
            .session
            .host()
            .ok_or_else(|| SessionError::Failed("no admitted session".into()))?;
        let objects = host.objects();
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
            let selected = self
                .selections
                .get(&owner)
                .map_or_else(|| self.roster.contains(&owner), |set| set.contains(&handle));
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
                selected,
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

/// Whether a unit is visible and alive enough to be selected or ordered. Mirrors the local
/// selection and order filters: hidden or dead units do not join a player's selection.
fn selectable(unit: &ExecutedUnit) -> bool {
    unit.shown && unit.combat.as_ref().is_none_or(|combat| !combat.dead)
}

/// The exported or accepted position of a unit, falling back to the executed creation point.
fn unit_position(unit: &ExecutedUnit) -> [f32; 2] {
    unit.position.unwrap_or([unit.x, unit.y])
}

/// The strict per-axis harness selection box used by `pcx_sim`, in world units. This is a
/// recreation boundary, not a native selection radius.
fn within_harness_selection(unit: [f32; 2], point: [f32; 2]) -> bool {
    let radius = f64::from(world_units(FX_ONE));
    (f64::from(unit[0]) - f64::from(point[0])).abs() < radius
        && (f64::from(unit[1]) - f64::from(point[1])).abs() < radius
}

/// Replaces one player's selection with the selectable units that player owns inside `point`.
/// Foreign units at the same point stay with their own player's selection.
fn units_at(
    objects: &ObjectHost,
    player: u8,
    point: [f32; 2],
) -> Result<BTreeSet<HandleId>, String> {
    select_at(
        objects
            .executed_units()
            .filter(|(_, unit)| selectable(unit))
            .map(|(id, unit)| (id, unit.executed_owner, unit_position(unit))),
        player,
        point,
    )
}

/// The pure owner-and-box filter behind [`units_at`]. Kept separate so two-player selection
/// independence is testable without an admitted map.
fn select_at(
    units: impl Iterator<Item = (HandleId, i32, [f32; 2])>,
    player: u8,
    point: [f32; 2],
) -> Result<BTreeSet<HandleId>, String> {
    let mut chosen = BTreeSet::new();
    for (id, owner, position) in units {
        if owner != i32::from(player) || !within_harness_selection(position, point) {
            continue;
        }
        if chosen.len() == MAX_SELECTION_UNITS {
            return Err(format!(
                "selection exceeds the {MAX_SELECTION_UNITS}-unit bound"
            ));
        }
        chosen.insert(id);
    }
    Ok(chosen)
}

/// The selected units of one player, in arena order. A missing explicit selection means every
/// owned unit, the harness move-before-select default. Foreign, removed and unmapped handles are
/// absent here, so they never reach a native call.
fn selected_owned(
    units: impl Iterator<Item = (HandleId, i32)>,
    explicit: Option<&BTreeSet<HandleId>>,
    player: u8,
) -> Result<Vec<HandleId>, String> {
    let mut selected = Vec::new();
    for (id, owner) in units {
        if owner != i32::from(player) {
            continue;
        }
        if explicit.is_some_and(|set| !set.contains(&id)) {
            continue;
        }
        if selected.len() == MAX_SELECTION_UNITS {
            return Err(format!(
                "command selection exceeds the {MAX_SELECTION_UNITS}-unit bound"
            ));
        }
        selected.push(id);
    }
    Ok(selected)
}

/// Selected units that currently admit `kind`, read from the admitted source order registry.
/// Paused units are left out rather than causing a failed batch.
fn orderable_units(
    objects: &ObjectHost,
    explicit: Option<&BTreeSet<HandleId>>,
    player: u8,
    kind: Kind,
) -> Result<Vec<HandleId>, String> {
    let selected = selected_owned(
        objects
            .executed_units()
            .map(|(id, unit)| (id, unit.executed_owner)),
        explicit,
        player,
    )?;
    let mut orderable = Vec::new();
    for id in selected {
        let unit = objects
            .executed_unit(id)
            .map_err(|error| error.to_string())?;
        if unit.paused_since.is_some() {
            continue;
        }
        match objects.standard_command_order(id, kind) {
            Ok(Some(_)) => orderable.push(id),
            Ok(None) => {}
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(orderable)
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

    fn handle(token: u64) -> HandleId {
        HandleId::new(std::num::NonZeroU64::new(token).unwrap())
    }

    #[test]
    fn selection_is_strictly_inside_the_harness_box() {
        assert!(within_harness_selection([9.5, 10.5], [10.0, 10.0]));
        assert!(!within_harness_selection([9.0, 10.0], [10.0, 10.0]));
        assert!(within_harness_selection([9.5, 10.5], [10.0, 10.0]));
        assert!(!within_harness_selection([11.0, 10.0], [10.0, 10.0]));
        assert!(!within_harness_selection([10.0, 8.9], [10.0, 10.0]));
    }

    #[test]
    fn two_players_selecting_the_same_point_keep_independent_sets() {
        let units = [
            (handle(1), 1, [10.0, 10.0]),
            (handle(2), 1, [10.5, 10.0]),
            (handle(3), 1, [12.0, 10.0]),
            (handle(4), 2, [10.0, 10.0]),
        ];
        let one = select_at(units.into_iter(), 1, [10.0, 10.0]).unwrap();
        let two = select_at(units.into_iter(), 2, [10.0, 10.0]).unwrap();
        assert_eq!(one, BTreeSet::from([handle(1), handle(2)]));
        assert_eq!(two, BTreeSet::from([handle(4)]));
    }

    #[test]
    fn the_default_selection_is_every_owned_unit_and_never_a_foreign_one() {
        let units = [(handle(1), 1), (handle(2), 1), (handle(3), 2)];
        let all = selected_owned(units.into_iter(), None, 1).unwrap();
        assert_eq!(all, vec![handle(1), handle(2)]);
    }

    #[test]
    fn a_selection_beyond_the_snapshot_bound_is_refused() {
        let units = (1..=MAX_SELECTION_UNITS as u64 + 1)
            .map(|token| (handle(token), 1, [0.0, 0.0]))
            .collect::<Vec<_>>();
        assert!(select_at(units.into_iter(), 1, [0.0, 0.0]).is_err());
    }

    #[test]
    fn an_explicit_selection_narrows_the_order_and_ignores_foreign_handles() {
        let units = [(handle(1), 1), (handle(2), 1), (handle(3), 2)];
        let explicit = BTreeSet::from([handle(1), handle(3), handle(9)]);
        let selected = selected_owned(units.into_iter(), Some(&explicit), 1).unwrap();
        assert_eq!(selected, vec![handle(1)]);
        let empty = BTreeSet::new();
        assert!(
            selected_owned(units.into_iter(), Some(&empty), 1)
                .unwrap()
                .is_empty()
        );
    }
}

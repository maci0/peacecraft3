//! Transports, a wire codec, and the server that holds the world.
//!
//! Three protocols share this core, and the reason they can is that the *rules*
//! are protocolless: a client proposes intent, the server owns the world, and
//! the sim decides. What differs between classic LAN, classic Battle.net and our
//! own protocol is wire encoding and delivery guarantees, not authority.
//!
//! So this crate is written to the authority model first:
//!
//! ```text
//! client --intent--> transport --> server --validated--> sim --> snapshots
//!          <----------- snapshots / acks -----------
//! ```
//!
//! A transport that loses a command must surface that loss rather than papering
//! over it, because a dropped move that the client believes was accepted is
//! exactly the desync this design prevents.

use std::collections::BTreeMap;

use pcx_sim::{Input, World, step, validate_input};

// A real socket transport is desktop-only: a browser has no UDP to bind, and a
// wasm build that fails to link on `std::net` is worse than one that simply has
// no transport.
#[cfg(not(target_arch = "wasm32"))]
pub mod transport;

pub mod w3g;

pub mod bnet;
pub mod classic_frame;
pub mod replay;
pub mod rollback;
pub mod session;

/// How a transport delivers commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    /// A command is applied exactly once, in order, or the connection reports
    /// failure. The default for our own protocol.
    Reliable,
    /// A command may be dropped, and the sender must include enough
    /// information to detect it. Classic LAN is effectively this.
    Unreliable {
        /// Redundancy copies of each command, to survive one or two losses.
        redundancy: u8,
    },
}

/// A transport is whatever can carry a datagram to a peer and yield datagrams.
///
/// Implemented for real sockets on a desktop, and for whatever a browser exposes
/// on wasm. Keeping it a trait rather than an enum is what lets the tests run
/// with no network at all.
pub trait Transport {
    type Error;

    /// Sends a datagram.
    fn send(&mut self, to: Peer, datagram: &[u8]) -> Result<(), Self::Error>;

    /// Yields the next datagram, if one has arrived.
    fn poll(&mut self) -> Option<(Peer, Vec<u8>)>;
}

/// A peer address.
///
/// Deliberately opaque: a LAN peer is a socket address, a BNet peer is a session,
/// and neither should leak into the sim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Peer(pub u32);

/// Wire-format errors, including ambiguous unit identity in a snapshot.
#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    #[error("datagram is {len} bytes, shorter than the {need}-byte header")]
    TooShort { len: usize, need: usize },
    #[error("unknown datagram kind {0}")]
    UnknownKind(u8),
    #[error("payload for kind {kind} needs {need} bytes, datagram has {got}")]
    PayloadTooShort { kind: u8, need: usize, got: usize },
    #[error("trailing bytes after a well-formed datagram: {remaining}")]
    TrailingBytes { remaining: usize },
    #[error("snapshot span at byte {at} with length {len} exceeds the address space")]
    PayloadSpanOverflow { at: usize, len: usize },
    #[error(
        "snapshot at tick {tick} repeats unit ID {id} at record {record}; first seen at record {first_record}"
    )]
    DuplicateUnitId {
        tick: u64,
        id: u32,
        first_record: u32,
        record: u32,
    },
    #[error("unsupported world snapshot version {found}; this build reads {supported}")]
    UnsupportedSnapshotVersion { found: u8, supported: u8 },
    #[error("snapshot selection flag {flag} at record {record} is not 0 or 1")]
    BadSelectionFlag { record: u32, flag: u8 },
}

/// What a datagram means.
///
/// Kinds are part of the wire format, so changing one breaks compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DatagramKind {
    /// A client's input for one tick.
    Input = 1,
    /// The server's acknowledgement of an input, by tick.
    InputAck = 2,
    /// A snapshot of the authoritative world.
    Snapshot = 3,
    /// A request to resynchronise from a given tick.
    Resync = 4,
}

impl DatagramKind {
    fn from_u8(v: u8) -> Option<Self> {
        match v {
            1 => Some(Self::Input),
            2 => Some(Self::InputAck),
            3 => Some(Self::Snapshot),
            4 => Some(Self::Resync),
            _ => None,
        }
    }
}

/// A decoded datagram.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Datagram {
    Input {
        tick: u64,
        /// The player this input belongs to, so a client cannot speak for
        /// another player by re-labelling its own datagram.
        player: u8,
        /// Compact encoding of the commands; see [`encode_input_payload`].
        payload: Vec<u8>,
    },
    InputAck {
        tick: u64,
    },
    Snapshot {
        tick: u64,
        /// Compact encoding of the world; see [`encode_world_payload`].
        payload: Vec<u8>,
    },
    Resync {
        from_tick: u64,
    },
}

/// Fixed bytes every datagram carries before its own payload.
const WIRE_HEADER: usize = 1 /* kind */ + 8 /* tick/u64 */;

/// Encodes a datagram.
pub fn encode(d: &Datagram) -> Vec<u8> {
    let mut out = Vec::new();
    let kind = match d {
        Datagram::Input { .. } => DatagramKind::Input as u8,
        Datagram::InputAck { .. } => DatagramKind::InputAck as u8,
        Datagram::Snapshot { .. } => DatagramKind::Snapshot as u8,
        Datagram::Resync { .. } => DatagramKind::Resync as u8,
    };
    out.push(kind);
    match d {
        Datagram::Input {
            tick,
            player,
            payload,
        } => {
            out.extend_from_slice(&tick.to_le_bytes());
            out.push(*player);
            out.extend_from_slice(payload);
        }
        Datagram::Snapshot { tick, payload } => {
            out.extend_from_slice(&tick.to_le_bytes());
            out.extend_from_slice(payload);
        }
        Datagram::InputAck { tick } => out.extend_from_slice(&tick.to_le_bytes()),
        Datagram::Resync { from_tick } => out.extend_from_slice(&from_tick.to_le_bytes()),
    }
    out
}

/// Decodes a datagram, refusing anything that does not fit the framing.
pub fn decode(bytes: &[u8]) -> Result<Datagram, CodecError> {
    let need = WIRE_HEADER;
    if bytes.len() < need {
        return Err(CodecError::TooShort {
            len: bytes.len(),
            need,
        });
    }
    let kind = DatagramKind::from_u8(bytes[0]).ok_or(CodecError::UnknownKind(bytes[0]))?;
    let mut tick_bytes = [0u8; 8];
    tick_bytes.copy_from_slice(&bytes[1..9]);
    let tick = u64::from_le_bytes(tick_bytes);
    let mut rest = &bytes[9..];

    match kind {
        DatagramKind::Input => {
            if rest.is_empty() {
                return Err(CodecError::PayloadTooShort {
                    kind: kind as u8,
                    need: 1,
                    got: rest.len(),
                });
            }
            let player = rest[0];
            rest = &rest[1..];
            Ok(Datagram::Input {
                tick,
                player,
                payload: rest.to_vec(),
            })
        }
        DatagramKind::InputAck => {
            if !rest.is_empty() {
                return Err(CodecError::TrailingBytes {
                    remaining: rest.len(),
                });
            }
            Ok(Datagram::InputAck { tick })
        }
        DatagramKind::Snapshot => {
            if rest.is_empty() {
                return Err(CodecError::PayloadTooShort {
                    kind: kind as u8,
                    need: 1,
                    got: 0,
                });
            }
            Ok(Datagram::Snapshot {
                tick,
                payload: rest.to_vec(),
            })
        }
        DatagramKind::Resync => {
            if !rest.is_empty() {
                return Err(CodecError::TrailingBytes {
                    remaining: rest.len(),
                });
            }
            Ok(Datagram::Resync { from_tick: tick })
        }
    }
}

/// Command wire tags. Part of the format, not the sim.
mod cmd {
    pub const NOP: u8 = 0;
    pub const SELECT: u8 = 1;
    pub const MOVE: u8 = 2;
    pub const STOP: u8 = 3;
    /// Bytes a command occupies, excluding its tag.
    pub(super) const fn payload_len(tag: u8) -> usize {
        match tag {
            NOP | STOP => 0,
            SELECT | MOVE => 8,
            _ => usize::MAX,
        }
    }
}

/// Encodes one tick of commands for one player.
pub fn encode_input_payload(commands: &[pcx_sim::Command]) -> Vec<u8> {
    let mut out = Vec::new();
    for c in commands {
        use pcx_sim::Command;
        let tag = match c {
            Command::Nop => cmd::NOP,
            Command::Select { .. } => cmd::SELECT,
            Command::Move { .. } => cmd::MOVE,
            Command::Stop => cmd::STOP,
        };
        out.push(tag);
        match c {
            Command::Nop | Command::Stop => {}
            Command::Select { x, y } | Command::Move { x, y } => {
                out.extend_from_slice(&x.to_le_bytes());
                out.extend_from_slice(&y.to_le_bytes());
            }
        }
    }
    out
}

/// Decodes a tick's commands, refusing anything that does not fit.
pub fn decode_input_payload(bytes: &[u8]) -> Result<Vec<pcx_sim::Command>, CodecError> {
    use pcx_sim::Command;
    let mut out = Vec::new();
    let mut at = 0usize;
    while at < bytes.len() {
        let tag = bytes[at];
        at += 1;
        let len = cmd::payload_len(tag);
        if len == usize::MAX {
            return Err(CodecError::UnknownKind(tag));
        }
        if bytes.len() - at < len {
            return Err(CodecError::PayloadTooShort {
                kind: tag,
                need: len,
                got: bytes.len() - at,
            });
        }
        out.push(match tag {
            cmd::NOP => Command::Nop,
            cmd::STOP => Command::Stop,
            cmd::SELECT => {
                let x = i32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
                let y = i32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap());
                Command::Select { x, y }
            }
            cmd::MOVE => {
                let x = i32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
                let y = i32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap());
                Command::Move { x, y }
            }
            _ => unreachable!(),
        });
        at += len;
    }
    Ok(out)
}

/// A unit's fixed wire encoding: id, owner, x, y and the target flag.
const UNIT_HEAD_BYTES: usize = 4 /* id */ + 1 /* owner */ + 4 /* x */ + 4 /* y */ + 1 /* has_target */;

/// Bytes a unit's move target occupies when it has one.
const UNIT_TARGET_BYTES: usize = 8;

/// Suffix tag for snapshot selection version 1. Absent on version 0.
const SELECTION_TAG: &[u8] = b"SEL";
const SELECTION_VERSION: u8 = 1;

/// Encodes a world for the wire.
///
/// Version 0, with no suffix, is the historical payload and means every unit
/// is selected. A world with any deselected unit appends `SEL`, version 1, and
/// one 0 or 1 flag per unit. Existing decoders reject that suffix as trailing
/// bytes instead of dropping the flags.
pub fn encode_world_payload(world: &pcx_sim::World) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&world.next_unit_id.to_le_bytes());
    out.extend_from_slice(&(world.units.len() as u32).to_le_bytes());
    for u in &world.units {
        out.extend_from_slice(&u.id.to_le_bytes());
        out.push(u.owner);
        out.extend_from_slice(&u.x.to_le_bytes());
        out.extend_from_slice(&u.y.to_le_bytes());
        match u.move_target {
            Some((x, y)) => {
                out.push(1);
                out.extend_from_slice(&x.to_le_bytes());
                out.extend_from_slice(&y.to_le_bytes());
            }
            None => out.push(0),
        }
    }
    if world.units.iter().any(|unit| !unit.selected) {
        out.extend_from_slice(SELECTION_TAG);
        out.push(SELECTION_VERSION);
        for unit in &world.units {
            out.push(u8::from(unit.selected));
        }
    }
    out
}

fn world_selection_flags(bytes: &[u8], at: usize, count: u32) -> Result<Option<&[u8]>, CodecError> {
    if at == bytes.len() {
        return Ok(None);
    }
    let rest = &bytes[at..];
    if !rest.starts_with(SELECTION_TAG) {
        return Err(CodecError::TrailingBytes {
            remaining: rest.len(),
        });
    }
    if rest.len() < SELECTION_TAG.len() + 1 {
        return Err(CodecError::PayloadTooShort {
            kind: 3,
            need: at + SELECTION_TAG.len() + 1,
            got: bytes.len(),
        });
    }
    let found = rest[SELECTION_TAG.len()];
    if found != SELECTION_VERSION {
        return Err(CodecError::UnsupportedSnapshotVersion {
            found,
            supported: SELECTION_VERSION,
        });
    }
    let flag_at = at + SELECTION_TAG.len() + 1;
    let count_usize = usize::try_from(count).map_err(|_| CodecError::PayloadSpanOverflow {
        at: flag_at,
        len: usize::MAX,
    })?;
    let end = flag_at
        .checked_add(count_usize)
        .ok_or(CodecError::PayloadSpanOverflow {
            at: flag_at,
            len: count_usize,
        })?;
    if bytes.len() < end {
        return Err(CodecError::PayloadTooShort {
            kind: 3,
            need: end,
            got: bytes.len(),
        });
    }
    if bytes.len() != end {
        return Err(CodecError::TrailingBytes {
            remaining: bytes.len() - end,
        });
    }
    let flags = &bytes[flag_at..end];
    for (record, &flag) in flags.iter().enumerate() {
        if flag > 1 {
            return Err(CodecError::BadSelectionFlag {
                record: record as u32,
                flag,
            });
        }
    }
    Ok(Some(flags))
}

fn world_payload_span(bytes: &[u8], at: usize, len: usize) -> Result<(&[u8], usize), CodecError> {
    let end = at
        .checked_add(len)
        .ok_or(CodecError::PayloadSpanOverflow { at, len })?;
    let chunk = bytes.get(at..end).ok_or(CodecError::PayloadTooShort {
        kind: 3,
        need: end,
        got: bytes.len(),
    })?;
    Ok((chunk, end))
}

/// Decodes a world, refusing malformed framing and repeated unit IDs.
///
/// IDs identify one unit during correction; duplicate records cannot be chosen
/// by wire order. Unique records and the raw counter retain their supplied order
/// and values. Counter exhaustion and consistency are separate admission policy.
pub fn decode_world_payload(tick: u64, bytes: &[u8]) -> Result<pcx_sim::World, CodecError> {
    let take_u32 = |at: usize| -> Result<u32, CodecError> {
        let (chunk, _) = world_payload_span(bytes, at, std::mem::size_of::<u32>())?;
        Ok(u32::from_le_bytes(chunk.try_into().unwrap()))
    };
    let next_unit_id = take_u32(0)?;
    let count = take_u32(4)?;
    let take_unit = |at: &mut usize| -> Result<pcx_sim::Unit, CodecError> {
        let (chunk, head) = world_payload_span(bytes, *at, UNIT_HEAD_BYTES)?;
        let id = u32::from_le_bytes(chunk[0..4].try_into().unwrap());
        let owner = chunk[4];
        let x = i32::from_le_bytes(chunk[5..9].try_into().unwrap());
        let y = i32::from_le_bytes(chunk[9..13].try_into().unwrap());
        let has_target = chunk[13] != 0;
        *at = head;
        let move_target = if has_target {
            let (target, end) = world_payload_span(bytes, *at, UNIT_TARGET_BYTES)?;
            *at = end;
            Some((
                i32::from_le_bytes(target[0..4].try_into().unwrap()),
                i32::from_le_bytes(target[4..8].try_into().unwrap()),
            ))
        } else {
            None
        };
        Ok(pcx_sim::Unit {
            id,
            owner,
            selected: true,
            x,
            y,
            move_target,
        })
    };
    // Check the complete immutable frame before allocating unit metadata.
    let mut at = 8usize;
    for _ in 0..count {
        take_unit(&mut at)?;
    }
    let flags = world_selection_flags(bytes, at, count)?;
    let mut units = Vec::with_capacity(count.min(4096) as usize);
    let mut unit_ids = BTreeMap::new();
    at = 8;
    for record in 0..count {
        let mut unit = take_unit(&mut at)?;
        if let Some(flags) = &flags {
            unit.selected = flags[record as usize] == 1;
        }
        if let Some(first_record) = unit_ids.insert(unit.id, record) {
            return Err(CodecError::DuplicateUnitId {
                tick,
                id: unit.id,
                first_record,
                record,
            });
        }
        units.push(unit);
    }
    // Movement admission is match context, not a per-snapshot payload. Decode
    // leaves it empty; [`Client::reconcile_detailed`] binds the client's context
    // back before any later step. An admitted client does not keep stepping
    // this world with the harness rule.
    Ok(pcx_sim::World {
        tick,
        units,
        next_unit_id,
        movement: None,
    })
}

/// The server.
///
/// It owns the world and the only authoritative clock. Everything it accepts has
/// been through [`validate_input`], which is what stops a client from asking for
/// a tick that has not happened yet.
pub struct Server {
    world: World,
    /// Inputs accepted but not yet simulated, by tick.
    pending: BTreeMap<u64, Input>,
    /// The last tick this server acknowledged.
    last_ack: u64,
    rng: pcx_sim::Rng,
    /// Snapshots retained for resync, keyed by tick.
    snapshots: BTreeMap<u64, World>,
}

impl Server {
    pub fn new(world: World, seed: u64) -> Self {
        Self {
            world,
            pending: BTreeMap::new(),
            last_ack: 0,
            rng: pcx_sim::Rng::new(seed),
            snapshots: BTreeMap::new(),
        }
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    /// Accepts a client's input.
    ///
    /// Validation is before buffering, not after: an out-of-window input is
    /// rejected rather than stored, which is what bounds memory against a
    /// malicious client.
    pub fn accept(&mut self, input: Input) -> Result<(), ValidateError> {
        validate_input(&self.world, &input).map_err(ValidateError::from_sim)?;
        self.pending.insert(input.tick, input);
        Ok(())
    }

    /// Advances the sim by one tick if that tick's input is available.
    ///
    /// Returns true when a tick ran. Waiting for input rather than running with
    /// a default is what keeps every client on the same input: a missing command
    /// is a stall, never a silent divergence.
    pub fn advance(&mut self) -> bool {
        let tick = self.world.tick;
        let Some(input) = self.pending.remove(&tick) else {
            return false;
        };
        step(&mut self.world, &input, &mut self.rng);
        self.last_ack = tick;
        if self.snapshots.len() >= 64 {
            let oldest = *self.snapshots.keys().next().unwrap();
            self.snapshots.remove(&oldest);
        }
        self.snapshots.insert(self.world.tick, self.world.clone());
        true
    }

    pub fn last_ack(&self) -> u64 {
        self.last_ack
    }

    /// The snapshot to send a client, or `None` if none is retained.
    pub fn snapshot(&self, tick: u64) -> Option<&World> {
        self.snapshots.get(&tick)
    }

    /// Rewinds to a retained snapshot, discarding everything after it.
    pub fn resync_to(&mut self, tick: u64) -> bool {
        if let Some(snap) = self.snapshots.get(&tick) {
            self.world = snap.clone();
            self.pending.retain(|t, _| *t >= tick);
            self.snapshots.retain(|t, _| *t <= tick);
            true
        } else {
            false
        }
    }

    /// How far the sim is behind the furthest accepted input.
    pub fn input_lead(&self) -> u64 {
        self.pending
            .keys()
            .next_back()
            .map_or(0, |t| t - self.world.tick)
    }
}

/// An input the server refused.
#[derive(Debug, thiserror::Error)]
pub enum ValidateError {
    #[error("{0}")]
    Sim(String),
}

impl ValidateError {
    fn from_sim(e: pcx_sim::Error) -> Self {
        ValidateError::Sim(e.to_string())
    }
}

/// A client's view: a predicted world plus the authority it defers to.
///
/// It simulates locally so the screen moves immediately, but the world it is
/// shown is the server's, and a divergence is corrected rather than argued with.
///
/// # Rollback
///
/// Correction used to mean "snap": adopt the server's world and teleport the
/// local player to it. That is correct and it is visible, which is the classic
/// symptom of a netcode that feels broken. A client here does better when it
/// can:
///
/// 1. Remember a bounded ring of past states, each with the random stream that
///    accompanied it ([`rollback::TickHistory`]).
/// 2. Remember its own inputs for the ticks the server has not yet consumed.
/// 3. On a correction for tick `T`, take the server's world at `T` as the base,
///    restore the stream recorded at `T`, and replay inputs `T..head` through
///    [`pcx_sim::step`].
///
/// The base is always the server's. Replay only ever re-applies the client's
/// own *intent* through the same step the server runs, so the corrected state
/// is "what the server will compute", never "what the client asserts". A client
/// cannot talk its way into a state the server never sent, because the state it
/// resumes from *is* one the server sent.
///
/// The correction is replayable only when the client can reproduce the server's
/// own history, which bounds it twice over: by the ring window (memory) and by
/// the re-simulation budget (work). Beyond either bound — or before the server
/// has acknowledged anything, so the client cannot know which inputs the
/// correction supersedes — the client adopts authority whole and says so. See
/// [`rollback::Reconciliation`] and [`rollback::Correction`].
pub struct Client {
    predicted: World,
    authoritative: World,
    rng: pcx_sim::Rng,
    /// The newest tick whose authoritative world has been accepted. Monotonic:
    /// a late packet must not rewind it.
    consensus_tick: u64,
    /// The newest tick the server has confirmed it simulated, or `None` while
    /// nothing has been acknowledged. Mirrors [`Datagram::InputAck`] and
    /// [`Server::last_ack`].
    acked_tick: Option<u64>,
    /// The client's own inputs for ticks the server has not consumed.
    ///
    /// Bounded twice: acknowledging prunes what the server has run, and every
    /// prediction prunes anything too old to be replayed under the budget. So it
    /// holds at most `max_resim_ticks + 1` entries.
    pending: BTreeMap<u64, Input>,
    /// Bounded ring of past states, for rolling back instead of snapping.
    history: rollback::TickHistory,
    /// Per-correction re-simulation budget.
    max_resim_ticks: usize,
}

impl Client {
    /// A client with no world.
    ///
    /// Useful only for a client that has not yet been sent anything; prefer
    /// [`Client::from_world`], because a client whose world omits the units the
    /// server already spawned predicts a world that never had them.
    pub fn new(seed: u64) -> Self {
        Self::from_world(World::empty(), seed)
    }

    /// A client seeded with the world the server sent it.
    pub fn from_world(world: World, seed: u64) -> Self {
        Self::with_limits(world, seed, rollback::RollbackLimits::default())
    }

    /// A client with explicit rollback bounds.
    ///
    /// `limits.history_ticks = 0` disables rollback, leaving every correction a
    /// snap; `limits.max_resim_ticks` bounds the work a single correction may do.
    pub fn with_limits(world: World, seed: u64, limits: rollback::RollbackLimits) -> Self {
        let rng = pcx_sim::Rng::new(seed);
        let mut history = rollback::TickHistory::new(limits.history_ticks);
        // Seed the ring with the state the client was handed, so a correction
        // for the tick it starts on is inside the window rather than needing a
        // prediction to get there first.
        history.record(world.tick, &world, &rng);
        Self {
            predicted: world.clone(),
            authoritative: world.clone(),
            rng,
            consensus_tick: world.tick,
            acked_tick: None,
            pending: BTreeMap::new(),
            history,
            max_resim_ticks: limits.max_resim_ticks,
        }
    }

    /// Records that the server has simulated through `tick`.
    ///
    /// This is the whole basis on which a rollback may replay input: inputs the
    /// server has consumed are superseded by the snapshot and must not be
    /// re-applied, and inputs it has not consumed are still predictions that the
    /// client is entitled to re-derive. With nothing acknowledged there is no
    /// such boundary, so corrections snap.
    ///
    /// Monotonic, because an acknowledgement is never retracted: a reordered
    /// [`Datagram::InputAck`] must not move the boundary backwards.
    ///
    /// A snapshot that arrives *behind* this boundary is ignored rather than
    /// applied, because it cannot anchor a replay — the inputs it already
    /// contains have been consumed. So a driver that sends acknowledgements
    /// every tick should also send snapshots at least that often; a snapshot
    /// every tenth tick would simply be discarded.
    pub fn acknowledge(&mut self, tick: u64) {
        let acked = self.acked_tick.map_or(tick, |seen| seen.max(tick));
        self.acked_tick = Some(acked);
        self.pending.retain(|t, _| *t > acked);
    }

    /// The newest tick the server has confirmed it simulated.
    pub fn acked_tick(&self) -> Option<u64> {
        self.acked_tick
    }

    /// The tick of the newest authoritative world this client accepted.
    pub fn consensus_tick(&self) -> u64 {
        self.consensus_tick
    }

    /// The retained past states, for inspection.
    pub fn history(&self) -> &rollback::TickHistory {
        &self.history
    }

    /// How many unacknowledged inputs are held. Bounded by the budget.
    pub fn pending_inputs(&self) -> usize {
        self.pending.len()
    }

    /// Applies a local input immediately, and remembers it.
    ///
    /// The state *before* the step is retained first, because that is the frame
    /// a rollback to this tick needs.
    pub fn predict(&mut self, input: Input) {
        self.history
            .record(self.predicted.tick, &self.predicted, &self.rng);
        let tick = input.tick;
        step(&mut self.predicted, &input, &mut self.rng);
        self.pending.insert(tick, input);
        // An input older than the budget can never be replayed, so holding it
        // would only grow the log. This is what bounds the log when the server
        // never acknowledges anything.
        let oldest_replayable = self
            .predicted
            .tick
            .saturating_sub(self.max_resim_ticks as u64);
        self.pending.retain(|t, _| *t >= oldest_replayable);
    }

    /// Accepts the server's authoritative world, and re-aligns when they differ.
    ///
    /// Returning the divergence is deliberate: the caller decides whether to show
    /// it, because a silent snap is indistinguishable from a bug. Use
    /// [`Client::reconcile_detailed`] when *how* the realignment happened also
    /// matters.
    pub fn reconcile(&mut self, snapshot: World) -> (World, u64) {
        let report = self.reconcile_detailed(snapshot);
        (report.snapshot, report.divergence)
    }

    /// Accepts the server's authoritative world, reporting what it cost.
    ///
    /// The decision table, in order:
    ///
    /// 1. A correction older than authority already accepted, or older than the
    ///    server's acknowledged clock, is a late duplicate and is ignored.
    ///    Applying one would rewind the client, which is the one thing this
    ///    design forbids.
    /// 2. No positional divergence at the snapshot's own tick means there is
    ///    nothing newer to replay. An older snapshot can still share positions
    ///    with a prediction that already applied Select, Move, or Stop; those
    ///    unacknowledged inputs are replayed under the same bounds as a
    ///    positional correction.
    /// 3. Without an acknowledgement there is no boundary between inputs the
    ///    server has consumed and inputs it has not, so authority is adopted.
    /// 4. A tick the ring has forgotten cannot be replayed, because the server
    ///    does not put its random stream on the wire and this client's recording
    ///    of it is gone.
    /// 5. A replay longer than the budget is a snap, so one late packet cannot
    ///    cost unbounded work.
    /// 6. Otherwise: authority becomes the base, the recorded stream is restored
    ///    beside it, and the client's unacknowledged inputs are replayed.
    pub fn reconcile_detailed(&mut self, mut snapshot: World) -> rollback::Reconciliation {
        let tick = snapshot.tick;
        if !self.bind_admission(&mut snapshot) {
            return rollback::Reconciliation {
                snapshot,
                divergence: 0,
                correction: rollback::Correction::AdmissionMismatch { tick },
                predicted_tick: self.predicted.tick,
            };
        }
        // A late duplicate is not a disagreement, and there are two ways to get
        // one. Either the tick is behind authority this client already accepted,
        // or it is behind the server's acknowledged clock — in which case the
        // inputs between it and that clock have been consumed, so it cannot
        // anchor a replay either. Neither may move anything: applying one would
        // rewind the client, which is the one thing this design forbids.
        if tick < self.consensus_tick || self.acked_tick.is_some_and(|a| tick <= a) {
            return rollback::Reconciliation {
                snapshot,
                divergence: 0,
                correction: rollback::Correction::Stale { tick },
                predicted_tick: self.predicted.tick,
            };
        }

        let divergence = rollback::positional_divergence(&self.predicted, &snapshot);
        self.authoritative = snapshot.clone();
        self.consensus_tick = tick;

        let correction = if divergence == 0 && tick == self.predicted.tick {
            // Authority for this tick wins, including selection and targets.
            // Do not replay: the inputs that produced this tick are already
            // at or behind it, and running them again would apply them twice.
            if self.predicted != snapshot {
                self.predicted = snapshot.clone();
            }
            rollback::Correction::Aligned
        } else {
            // The snapshot is not this prediction's tick, or positions differ.
            // Replay unacknowledged inputs from the snapshot, or snap when the
            // ack, history window, or resim budget does not allow it.
            self.correct(snapshot.clone())
        };

        rollback::Reconciliation {
            snapshot,
            divergence,
            correction,
            predicted_tick: self.predicted.tick,
        }
    }

    /// Re-anchors on `snapshot` and replays unacknowledged input, or snaps.
    fn correct(&mut self, snapshot: World) -> rollback::Correction {
        let tick = snapshot.tick;
        if self.acked_tick.is_none() {
            return self.snap(snapshot, tick, rollback::SnapReason::Unconfirmed);
        }
        let Some(frame) = self.history.get(tick) else {
            return self.snap(snapshot, tick, rollback::SnapReason::OutsideWindow);
        };
        // How far ahead of authority the prediction currently reaches, and so how
        // much replay this correction could cost.
        let head = self.predicted.tick;
        let pending_replay = head.saturating_sub(tick);
        if pending_replay > self.max_resim_ticks as u64 {
            return self.snap(snapshot, tick, rollback::SnapReason::ResimBudget);
        }

        // Authority becomes the base; the recorded stream is restored beside it.
        // The server does not transmit its RNG, so this client's frame is the
        // only stream it can honestly replay from — which is exactly why a
        // missing frame above is a snap rather than a guess.
        self.predicted = snapshot;
        self.rng = frame.rng().clone();
        self.history.record(tick, &self.predicted, &self.rng);

        // Replay every input the server has not consumed, from the tick the
        // snapshot describes forward. The world advances one tick per input, and
        // `step` asserts that the two agree, so the loop cannot feed the sim a
        // tick it has already passed.
        let mut resimulated = 0u32;
        for t in tick..head {
            let Some(input) = self.pending.get(&t) else {
                // The replay ran out of inputs. The result is still anchored to
                // authority, just shorter than the prediction was, and the count
                // below says so.
                break;
            };
            step(&mut self.predicted, input, &mut self.rng);
            // Overwrite the frame: it still describes the trajectory that was
            // just corrected away, and a later rollback must not replay it.
            self.history
                .record(self.predicted.tick, &self.predicted, &self.rng);
            resimulated += 1;
        }
        self.history.truncate_after(self.predicted.tick);

        rollback::Correction::RolledBack { tick, resimulated }
    }

    /// Adopts authority whole, because it cannot be replayed forward.
    fn snap(
        &mut self,
        snapshot: World,
        tick: u64,
        reason: rollback::SnapReason,
    ) -> rollback::Correction {
        self.predicted = snapshot;
        // Nothing the client predicted past this tick is reflected anywhere now,
        // and replaying it later would assert states that were never anchored.
        // Dropping it bounds the log and keeps the next prediction starting from
        // authority rather than from a discarded trajectory.
        self.pending.clear();
        self.history.truncate_after(tick);
        rollback::Correction::Snapped { tick, reason }
    }

    /// Restores this client's movement admission onto a decoded snapshot.
    ///
    /// The wire payload has no raster. A decoded world therefore arrives without
    /// admission; leaving it that way would step the harness rule through a wall.
    /// The same admission is attached. A different admission is refused, and the
    /// caller does not step.
    fn bind_admission(&self, snapshot: &mut World) -> bool {
        if let (Some(predicted), Some(authoritative)) =
            (&self.predicted.movement, &self.authoritative.movement)
            && !std::sync::Arc::ptr_eq(predicted, authoritative)
            && predicted.as_ref() != authoritative.as_ref()
        {
            return false;
        }
        let Some(authority) = self
            .predicted
            .movement
            .clone()
            .or_else(|| self.authoritative.movement.clone())
        else {
            return true;
        };
        let accept = match snapshot.movement.as_ref() {
            None => true,
            Some(incoming) => {
                std::sync::Arc::ptr_eq(incoming, &authority)
                    || incoming.as_ref() == authority.as_ref()
            }
        };
        if accept {
            snapshot.movement = Some(authority);
        }
        accept
    }

    pub fn predicted(&self) -> &World {
        &self.predicted
    }

    pub fn authoritative(&self) -> &World {
        &self.authoritative
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn snapshot_span_endpoint_overflow_is_typed_before_indexing() {
        assert!(matches!(
            super::world_payload_span(&[], usize::MAX - 5, super::UNIT_HEAD_BYTES),
            Err(super::CodecError::PayloadSpanOverflow { at, len })
                if at == usize::MAX - 5 && len == super::UNIT_HEAD_BYTES
        ));
    }

    use super::*;
    use pcx_sim::{Command, FX_ONE, MAX_INPUT_LEAD, Rng};

    fn order(player: u8, commands: Vec<Command>, tick: u64) -> Input {
        Input {
            tick,
            commands: vec![pcx_sim::PlayerInput { player, commands }],
        }
    }

    #[test]
    fn a_datagram_survives_a_round_trip() {
        let cases = [
            Datagram::Input {
                tick: 7,
                player: 2,
                payload: encode_input_payload(&[
                    Command::Move {
                        x: -FX_ONE,
                        y: FX_ONE * 3,
                    },
                    Command::Stop,
                ]),
            },
            Datagram::InputAck { tick: 9 },
            Datagram::Snapshot {
                tick: 11,
                payload: encode_world_payload(&World::empty()),
            },
            Datagram::Resync { from_tick: 3 },
        ];
        for original in &cases {
            let bytes = encode(original);
            let back = decode(&bytes).unwrap_or_else(|e| panic!("{original:?}: {e}"));
            assert_eq!(&back, original, "round trip changed the datagram");
        }
    }

    #[test]
    fn a_truncated_or_unknown_datagram_is_refused() {
        let good = encode(&Datagram::InputAck { tick: 1 });
        assert!(decode(&good[..good.len() - 1]).is_err(), "short must fail");
        let mut bad = good.clone();
        bad[0] = 99;
        assert!(decode(&bad).is_err(), "unknown kind must fail");
        let mut trailing = good.clone();
        trailing.push(0);
        assert!(decode(&trailing).is_err(), "trailing bytes must fail");
    }

    #[test]
    fn commands_survive_a_round_trip() {
        let cmds = vec![
            Command::Nop,
            Command::Stop,
            Command::Select { x: 1, y: -2 },
            Command::Move {
                x: FX_ONE * 5,
                y: -FX_ONE,
            },
        ];
        let bytes = encode_input_payload(&cmds);
        assert_eq!(decode_input_payload(&bytes).unwrap(), cmds);
        // Truncating mid-payload must fail. Two bytes is *not* a valid example:
        // `Nop` and `Stop` are one byte each, so a two-byte prefix is still a
        // well-formed pair of commands.
        assert!(
            decode_input_payload(&bytes[..5]).is_err(),
            "a command cut mid-payload must fail"
        );
    }

    #[test]
    fn a_world_survives_a_round_trip() {
        let mut w = World::empty();
        w.spawn(0, 1, 2);
        let id = w.spawn(1, FX_ONE, -FX_ONE);
        if let Some(u) = w.units.iter_mut().find(|u| u.id == id) {
            u.move_target = Some((FX_ONE * 4, 0));
        }
        let bytes = encode_world_payload(&w);
        let back = decode_world_payload(42, &bytes).unwrap();
        assert_eq!(back.tick, 42);
        assert_eq!(back.units, w.units, "units must survive the wire");
        assert_eq!(back.next_unit_id, w.next_unit_id);
        assert!(
            decode_world_payload(0, &bytes[..10]).is_err(),
            "short must fail"
        );
    }

    #[test]
    fn a_client_cannot_nominate_its_element_for_another_player() {
        let mut world = World::empty();
        world.spawn(0, 0, 0);
        let mut server = Server::new(world, 1);

        // Player 1 tries to speak for player 0.
        let spoofed = Input {
            tick: 0,
            commands: vec![
                pcx_sim::PlayerInput {
                    player: 0,
                    commands: vec![Command::Move {
                        x: FX_ONE,
                        y: FX_ONE,
                    }],
                },
                pcx_sim::PlayerInput {
                    player: 1,
                    commands: vec![Command::Stop],
                },
            ],
        };
        assert!(
            server.accept(spoofed).is_ok(),
            "the datagram is well formed"
        );
        assert!(server.advance(), "the tick with input must run");
        // Player 1's Stop does not cancel player 0's Move, because commands are
        // applied in player order and each applies only to its own units.
        assert!(
            server.world().unit(1).unwrap().move_target.is_some(),
            "player 1 stopped someone else's unit"
        );
    }

    #[test]
    fn the_server_refuses_input_beyond_the_horizon() {
        let world = World::empty();
        let mut server = Server::new(world, 0);
        let far = order(0, vec![Command::Nop], MAX_INPUT_LEAD + 1);
        assert!(server.accept(far).is_err(), "horizon must be enforced");
        assert!(server.input_lead() == 0);
    }

    #[test]
    fn a_client_reconciles_to_the_authoritative_world() {
        let mut server_world = World::empty();
        server_world.spawn(0, 0, 0);
        let authoritative = server_world.clone();

        // Seeded from the server's world, because a client that starts empty
        // predicts a world that never had the unit the server spawned — which is
        // not a divergence the server caused.
        let mut client = Client::from_world(server_world.clone(), 1);
        // The client predicts a move the server never authorised.
        client.predict(order(
            0,
            vec![Command::Move {
                x: FX_ONE * 10,
                y: 0,
            }],
            0,
        ));
        assert_ne!(client.predicted(), client.authoritative());

        let (snap, divergence) = client.reconcile(authoritative);
        assert_eq!(snap.tick, 0);
        assert!(
            divergence > 0,
            "the divergence must be reported, not hidden"
        );
        assert_eq!(
            client.predicted().units,
            snap.units,
            "reconciliation must adopt the server's world"
        );
    }

    #[test]
    fn a_missing_tick_stalls_rather_than_diverging() {
        let mut world = World::empty();
        world.spawn(0, 0, 0);
        let mut server = Server::new(world, 0);
        assert!(!server.advance(), "tick 0 has no input, so it must not run");

        let mut w2 = World::empty();
        w2.spawn(0, 0, 0);
        let mut s2 = Server::new(w2, 0);
        s2.accept(order(0, vec![Command::Nop], 0)).unwrap();
        assert!(s2.advance());
        assert_eq!(s2.world().tick, 1);
    }

    #[test]
    fn snapshots_support_resync() {
        let mut w = World::empty();
        w.spawn(0, 0, 0);
        let mut server = Server::new(w, 0);
        for t in 0..5 {
            server.accept(order(0, vec![Command::Nop], t)).unwrap();
            server.advance();
        }
        assert!(server.snapshot(3).is_some(), "snapshots are retained");
        assert!(server.resync_to(3), "resync must rewind");
        assert_eq!(server.world().tick, 3);
        assert!(server.resync_to(2), "older snapshot honoured");
    }

    #[test]
    fn the_same_input_on_server_and_client_matches() {
        // The property the whole design rests on: given the same input, the
        // server's world and the client's prediction agree, so reconciliation
        // is a no-op rather than a correction.
        let mut world = World::empty();
        world.spawn(0, 0, 0);
        let id = world.units[0].id;
        let mut server = Server::new(world, 7);
        // Both sides start from the same world, which is the only setup in which
        // "same input" can be expected to produce the same result.
        let mut client = Client::from_world(server.world().clone(), 7);
        let mut rng = Rng::new(0);
        let mut cworld = server.world().clone();

        for t in 0..40u64 {
            let input = order(
                0,
                vec![Command::Move {
                    x: FX_ONE * 5,
                    y: 0,
                }],
                t,
            );
            server.accept(input.clone()).unwrap();
            server.advance();
            step(&mut cworld, &input, &mut rng);
            client.predict(input.clone());
        }
        assert_eq!(
            client.predicted().unit(id).unwrap().x,
            server.world().unit(id).unwrap().x,
            "prediction and authority diverged on identical input"
        );
        let (_, divergence) = client.reconcile(server.world().clone());
        assert_eq!(divergence, 0, "reconciliation should find nothing to fix");
    }

    // -- rollback ------------------------------------------------------------

    /// Runs one tick on both sides with the same input, which is the only way
    /// their worlds are expected to agree.
    fn tick_both(server: &mut Server, client: &mut Client, input: &Input) {
        server.accept(input.clone()).unwrap();
        assert!(server.advance(), "the tick's input was accepted");
        client.predict(input.clone());
    }

    /// A world with one unit for player 0, standing at the origin.
    fn unit_world() -> World {
        let mut w = World::empty();
        w.spawn(0, 0, 0);
        w
    }

    /// The speed `pcx_sim::step` moves a unit per tick.
    const STEP_PER_TICK: i32 = FX_ONE / 10;

    /// A long move for one tick, which is the input the rollback tests run on:
    /// the unit is still travelling through all of them, so a tick's worth of
    /// movement is always observable.
    fn move_order(tick: u64) -> Input {
        order(
            0,
            vec![Command::Move {
                x: FX_ONE * 10,
                y: 0,
            }],
            tick,
        )
    }

    /// The divergence scenario, end to end: the client's input for tick 4 never
    /// reached the server in time, so the server ran tick 4 with nothing while
    /// the client predicted a long move. The client is three ticks past that and
    /// holds three inputs the server has not consumed.
    ///
    /// Acknowledging is left to the caller, because *what has been acknowledged*
    /// is precisely what decides whether a correction can be replayed.
    fn delayed_input_scenario(limits: rollback::RollbackLimits) -> (Server, Client) {
        const SEED: u64 = 0x5EED;
        let mut server = Server::new(unit_world(), SEED);
        let mut client = Client::with_limits(server.world().clone(), SEED, limits);

        for t in 0..4u64 {
            tick_both(&mut server, &mut client, &order(0, vec![Command::Nop], t));
        }
        // The reordered tick: the server runs a Nop where the client ran a move.
        server
            .accept(order(0, vec![Command::Nop], 4))
            .expect("tick 4 is inside the horizon");
        server.advance();
        client.predict(move_order(4));
        // Three more ticks the client predicted but the server has not run.
        for t in 5..8u64 {
            client.predict(move_order(t));
        }
        (server, client)
    }

    #[test]
    fn a_correction_inside_the_window_rolls_back_to_the_authoritative_run() {
        let (mut server, mut client) = delayed_input_scenario(rollback::RollbackLimits::default());
        // The server confirms what it actually consumed, which is what tells the
        // client that tick 4's input is settled and ticks 5..=7 are still open.
        client.acknowledge(server.last_ack());
        let id = client.predicted().units[0].id;
        let wrong = client.predicted().clone();
        let authoritative = server.snapshot(5).expect("the server retains tick 5");

        let report = client.reconcile_detailed(authoritative.clone());

        assert!(
            report.correction.rolled_back(),
            "a correction inside the window must replay, not teleport: {:?}",
            report.correction
        );
        assert_eq!(
            report.correction,
            rollback::Correction::RolledBack {
                tick: 5,
                resimulated: 3
            }
        );
        assert_eq!(report.predicted_tick, wrong.tick, "how far it predicted");

        // The rejected input for tick 4 is gone and exactly the three
        // unacknowledged inputs were replayed: three steps of movement, not four.
        assert_eq!(
            client.predicted().unit(id).unwrap().x,
            3 * STEP_PER_TICK,
            "the tick the server rejected must not survive the rollback"
        );
        assert_ne!(
            client.predicted(),
            &wrong,
            "the divergent prediction must have been replaced"
        );

        // The server now consumes the same three inputs, and the client's
        // corrected prediction must be bit-identical to that authoritative run.
        for t in 5..8u64 {
            server.accept(move_order(t)).unwrap();
            server.advance();
        }
        assert_eq!(
            client.predicted(),
            server.world(),
            "rollback must land on what the server computes, not what the client hoped"
        );
    }

    #[test]
    fn a_no_op_correction_replays_to_the_undisturbed_prediction() {
        // Both sides run the same inputs through tick 5, so the server's state at
        // tick 6 is exactly what the client already had recorded there. The
        // correction therefore changes nothing, and replaying it must reproduce
        // the prediction that was already in hand, to the bit.
        fn scenario() -> Client {
            const SEED: u64 = 0x5EED;
            let limits = rollback::RollbackLimits::default();
            let mut server = Server::new(unit_world(), SEED);
            let mut client = Client::with_limits(server.world().clone(), SEED, limits);

            for t in 0..=5u64 {
                tick_both(&mut server, &mut client, &move_order(t));
            }
            // The client then predicts three ticks the server has not consumed.
            for t in 6..=8u64 {
                client.predict(move_order(t));
            }
            client.acknowledge(server.last_ack());

            let before = client.predicted().clone();
            let authoritative = server.snapshot(6).expect("tick 6 is retained").clone();
            assert_eq!(
                client.history().get(6).map(|f| f.world()),
                Some(&authoritative),
                "the correction must be a no-op for this test to mean anything"
            );
            let report = client.reconcile_detailed(authoritative);
            assert!(report.divergence > 0, "the client is ahead, so it differs");
            assert_eq!(
                report.correction,
                rollback::Correction::RolledBack {
                    tick: 6,
                    resimulated: 3
                },
                "a no-op correction still replays, which is what is being proven"
            );
            assert_eq!(
                client.predicted(),
                &before,
                "re-simulating a no-op must reproduce the prediction exactly"
            );
            client
        }

        let client = scenario();
        assert_eq!(
            client.predicted().tick,
            9,
            "the prediction resumes its old horizon, not a shortened one"
        );
        // And it is not a coincidence of one run: re-simulating the same
        // correction from the same base lands on the same world every time.
        let again = scenario();
        assert_eq!(
            client.predicted(),
            again.predicted(),
            "two rollbacks of the same correction disagreed"
        );
    }

    #[test]
    fn a_correction_older_than_the_window_falls_back_to_a_snap() {
        let limits = rollback::RollbackLimits {
            history_ticks: 4,
            max_resim_ticks: 30,
        };
        let (server, mut client) = delayed_input_scenario(limits);
        client.acknowledge(server.last_ack());
        // Predict far enough that tick 5 has aged out of a four-tick ring, so the
        // window alone is what refuses the replay.
        for t in 8..=10u64 {
            client.predict(move_order(t));
        }
        assert!(
            client.history().get(5).is_none(),
            "the frame for the correction has aged out"
        );
        let authoritative = server
            .snapshot(5)
            .expect("the server retains tick 5")
            .clone();

        let report = client.reconcile_detailed(authoritative.clone());

        assert_eq!(
            report.correction,
            rollback::Correction::Snapped {
                tick: 5,
                reason: rollback::SnapReason::OutsideWindow
            },
            "an unreachable tick must snap, visibly"
        );
        assert_eq!(
            client.predicted(),
            &authoritative,
            "the snap adopts authority whole"
        );
        assert_eq!(report.predicted_tick, 5);
        assert!(
            client.history().len() <= limits.history_ticks,
            "history must stay bounded whatever happens"
        );
        assert_eq!(
            client.pending_inputs(),
            0,
            "a discarded prediction is dropped"
        );
    }

    #[test]
    fn a_correction_beyond_the_resim_budget_falls_back_to_a_snap() {
        let limits = rollback::RollbackLimits {
            history_ticks: 32,
            max_resim_ticks: 2,
        };
        let (server, mut client) = delayed_input_scenario(limits);
        client.acknowledge(server.last_ack());
        let authoritative = server
            .snapshot(5)
            .expect("the server retains tick 5")
            .clone();

        let report = client.reconcile_detailed(authoritative.clone());

        assert_eq!(
            report.correction,
            rollback::Correction::Snapped {
                tick: 5,
                reason: rollback::SnapReason::ResimBudget
            },
            "one correction must not cost unbounded work"
        );
        assert_eq!(client.predicted(), &authoritative);
        // The window would have allowed the replay, so this is the budget alone
        // refusing it.
        assert!(client.history().get(5).is_some(), "the frame was available");
    }

    #[test]
    fn a_correction_nobody_acknowledged_falls_back_to_a_snap() {
        let (server, mut client) = delayed_input_scenario(rollback::RollbackLimits::default());
        assert_eq!(client.acked_tick(), None);
        let authoritative = server
            .snapshot(5)
            .expect("the server retains tick 5")
            .clone();

        let report = client.reconcile_detailed(authoritative.clone());

        assert_eq!(
            report.correction,
            rollback::Correction::Snapped {
                tick: 5,
                reason: rollback::SnapReason::Unconfirmed
            },
            "without an ack the client cannot know which inputs the correction covers"
        );
        assert_eq!(client.predicted(), &authoritative);
        // The same correction, once the server has confirmed what it consumed,
        // is a rollback instead.
        let (server, mut acked) = delayed_input_scenario(rollback::RollbackLimits::default());
        acked.acknowledge(server.last_ack());
        assert!(
            acked
                .reconcile_detailed(authoritative)
                .correction
                .rolled_back()
        );
    }

    #[test]
    fn a_late_correction_never_rewinds_the_client() {
        let (server, mut client) = delayed_input_scenario(rollback::RollbackLimits::default());
        client.acknowledge(server.last_ack());
        let fresh = server.snapshot(5).expect("tick 5 is retained").clone();
        client.reconcile_detailed(fresh.clone());
        let predicted_after_fresh = client.predicted().clone();
        assert_eq!(client.consensus_tick(), 5);

        // A snapshot for tick 3 turns up after the tick 5 one: a reordered
        // packet, not a disagreement.
        let old = server.snapshot(3).expect("tick 3 is retained").clone();
        let report = client.reconcile_detailed(old);

        assert_eq!(report.correction, rollback::Correction::Stale { tick: 3 });
        assert_eq!(report.divergence, 0, "a late duplicate is not a divergence");
        assert_eq!(
            client.predicted(),
            &predicted_after_fresh,
            "an older packet must not rewind the prediction"
        );
        assert_eq!(client.authoritative(), &fresh, "authority stays the newest");
        assert_eq!(client.consensus_tick(), 5);
    }

    #[test]
    fn a_snapshot_behind_the_acknowledged_clock_is_ignored() {
        let (server, mut client) = delayed_input_scenario(rollback::RollbackLimits::default());
        // The server is heard from about its input stream before its world: by the
        // time a snapshot for tick 5 arrives, it has confirmed running through 7,
        // so replaying ticks 5..=7 would apply consumed inputs a second time.
        client.acknowledge(7);
        let before = client.predicted().clone();
        let authority_before = client.authoritative().clone();

        let report = client.reconcile_detailed(server.snapshot(5).unwrap().clone());

        assert_eq!(report.correction, rollback::Correction::Stale { tick: 5 });
        assert_eq!(
            client.predicted(),
            &before,
            "a correction the server has already overtaken must not move anything"
        );
        assert_eq!(
            client.authoritative(),
            &authority_before,
            "nor may it become the authority the client defers to"
        );
    }

    #[test]
    fn a_client_holds_bounded_state_however_long_it_predicts() {
        let limits = rollback::RollbackLimits {
            history_ticks: 8,
            max_resim_ticks: 4,
        };
        let mut client = Client::with_limits(unit_world(), 1, limits);
        for t in 0..500u64 {
            client.predict(order(
                0,
                vec![Command::Move {
                    x: FX_ONE * 10,
                    y: 0,
                }],
                t,
            ));
            assert!(
                client.pending_inputs() <= limits.max_resim_ticks + 1,
                "the input log grew past the replay budget at tick {t}"
            );
            assert!(
                client.history().len() <= limits.history_ticks,
                "history grew past its window at tick {t}"
            );
        }
        // An acknowledgement drops what the server has already run.
        client.acknowledge(495);
        assert!(
            client.pending_inputs() < 500,
            "acknowledged inputs are dropped"
        );
    }

    #[test]
    fn a_client_with_rollback_disabled_snapshots_every_correction() {
        let limits = rollback::RollbackLimits {
            history_ticks: 0,
            max_resim_ticks: 30,
        };
        let (server, mut client) = delayed_input_scenario(limits);
        client.acknowledge(server.last_ack());
        let authoritative = server
            .snapshot(5)
            .expect("the server retains tick 5")
            .clone();
        let report = client.reconcile_detailed(authoritative.clone());

        assert_eq!(
            report.correction,
            rollback::Correction::Snapped {
                tick: 5,
                reason: rollback::SnapReason::OutsideWindow
            }
        );
        assert_eq!(client.predicted(), &authoritative);
        assert!(client.history().is_empty());
    }

    #[test]
    fn version_0_snapshot_bytes_omit_the_selection_trailer() {
        let mut world = World::empty();
        world.spawn(0, 1, 2);
        let bytes = encode_world_payload(&world);
        assert!(bytes.windows(3).all(|window| window != b"SEL"));
        let restored = decode_world_payload(0, &bytes).unwrap();
        assert!(restored.units.iter().all(|unit| unit.selected));
        assert_eq!(encode_world_payload(&restored), bytes);
    }

    #[test]
    fn narrowed_selection_round_trips_and_matches_move_and_stop() {
        let mut world = World::empty();
        let near = world.spawn(0, 0, 0);
        let far = world.spawn(0, 10, 0);
        world.units[1].selected = false;
        world.units[1].move_target = Some((FX_ONE * 20, 0));
        let bytes = encode_world_payload(&world);
        assert!(bytes.windows(3).any(|window| window == b"SEL"));
        let restored = decode_world_payload(world.tick, &bytes).unwrap();
        assert_eq!(restored, world);
        assert_eq!(encode_world_payload(&restored), bytes);
        let mut original = world.clone();
        let mut decoded = restored;
        let move_order = order(0, vec![Command::Move { x: 50, y: 0 }], 0);
        let mut rng_a = Rng::new(1);
        let mut rng_b = Rng::new(1);
        pcx_sim::step(&mut original, &move_order, &mut rng_a);
        pcx_sim::step(&mut decoded, &move_order, &mut rng_b);
        assert_eq!(original, decoded);
        assert!(original.unit(near).unwrap().move_target.is_some());
        assert_eq!(
            original.unit(far).unwrap().move_target,
            Some((FX_ONE * 20, 0))
        );
        let stop = order(0, vec![Command::Stop], 1);
        pcx_sim::step(&mut original, &stop, &mut rng_a);
        pcx_sim::step(&mut decoded, &stop, &mut rng_b);
        assert_eq!(original, decoded);
        assert!(original.unit(near).unwrap().move_target.is_none());
        assert_eq!(
            original.unit(far).unwrap().move_target,
            Some((FX_ONE * 20, 0))
        );
    }

    #[test]
    fn unknown_selection_version_is_refused_and_junk_stays_trailing() {
        let mut world = World::empty();
        world.spawn(0, 0, 0);
        world.units[0].selected = false;
        let mut bytes = encode_world_payload(&world);
        let at = bytes
            .windows(3)
            .position(|window| window == b"SEL")
            .unwrap();
        bytes.truncate(at);
        bytes.push(255);
        assert!(matches!(
            decode_world_payload(0, &bytes),
            Err(CodecError::TrailingBytes { remaining: 1 })
        ));
        bytes.truncate(at);
        bytes.extend_from_slice(b"SEL");
        bytes.push(2);
        bytes.push(0);
        assert!(matches!(
            decode_world_payload(0, &bytes),
            Err(CodecError::UnsupportedSnapshotVersion {
                found: 2,
                supported: 1
            })
        ));
    }

    #[test]
    fn aligned_reconcile_adopts_selection_before_move() {
        let mut world = World::empty();
        world.spawn(0, 0, 0);
        world.spawn(0, 4, 0);
        let mut client = Client::from_world(world.clone(), 3);
        let mut snapshot = world.clone();
        snapshot.units[1].selected = false;
        let report = client.reconcile_detailed(snapshot.clone());
        assert_eq!(report.correction, rollback::Correction::Aligned);
        assert!(!client.predicted().units[1].selected);
        assert_eq!(client.predicted().tick, snapshot.tick);
        let mut predicted = client.predicted().clone();
        let input = order(0, vec![Command::Move { x: 9, y: 0 }], predicted.tick);
        let mut rng_a = Rng::new(1);
        let mut rng_b = Rng::new(1);
        pcx_sim::step(&mut predicted, &input, &mut rng_a);
        pcx_sim::step(&mut snapshot, &input, &mut rng_b);
        assert_eq!(
            predicted.units[0].move_target,
            snapshot.units[0].move_target
        );
        assert_eq!(predicted.units[1].move_target, None);
        assert_eq!(predicted.units[1].selected, snapshot.units[1].selected);
    }
}

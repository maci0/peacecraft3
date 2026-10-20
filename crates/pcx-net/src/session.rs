//! Authenticated peer slots and bounded command batches around the shared server.

use std::collections::BTreeMap;

use pcx_sim::{Command, FX_ONE, Input, PlayerInput, World};

use crate::replay::Recorder;
use crate::{Datagram, Peer, Server, decode, decode_input_payload, encode, encode_world_payload};

pub const MAX_PLAYERS: u8 = 16;
pub const MAX_PACKET_BYTES: usize = 1024;
pub const MAX_COMMANDS_PER_TICK: usize = 32;
// Keep subtraction inside the sim's i32 fixed-point range, including opposite edges.
pub const COORDINATE_LIMIT: i32 = 16_000 * FX_ONE;

/// Digest of one admitted session start: executable, map, movement profile, seed and
/// initial world. Peers compare it before play; it is not a secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionIdentity(pub [u8; 32]);

impl SessionIdentity {
    pub fn to_hex(&self) -> String {
        self.0.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    /// Accepts exactly 64 lowercase hex digits, the form `to_hex` writes.
    pub fn from_hex(text: &str) -> Option<Self> {
        if text.len() != 64 {
            return None;
        }
        let digits = text
            .bytes()
            .map(|c| {
                let lower = c.is_ascii_digit() || c.is_ascii_lowercase();
                (lower)
                    .then(|| (c as char).to_digit(16))
                    .flatten()
                    .map(|d| d as u8)
            })
            .collect::<Option<Vec<u8>>>()?;
        let (pairs, []) = digits.as_chunks::<2>() else {
            return None;
        };
        let mut out = [0u8; 32];
        for (byte, [high, low]) in out.iter_mut().zip(pairs) {
            *byte = high << 4 | low;
        }
        Some(Self(out))
    }
}

/// One peer packet after identity, size and bounds checks. Tick checks belong to the authority
/// that owns the clock.
#[derive(Debug, PartialEq)]
pub enum Packet {
    Input { tick: u64, commands: Vec<Command> },
    Resync,
}

/// Parses a packet sent by the peer assigned to `assigned`.
pub fn parse_packet(bytes: &[u8], assigned: u8) -> Result<Packet, SessionError> {
    if bytes.len() > MAX_PACKET_BYTES {
        return Err(SessionError::Packet);
    }
    match decode(bytes).map_err(|_| SessionError::Packet)? {
        Datagram::Input {
            tick,
            player,
            payload,
        } => {
            if player != assigned {
                return Err(SessionError::Identity);
            }
            let commands = decode_input_payload(&payload).map_err(|_| SessionError::Packet)?;
            if !commands_in_bounds(&commands) {
                return Err(SessionError::Packet);
            }
            Ok(Packet::Input { tick, commands })
        }
        Datagram::Resync { .. } => Ok(Packet::Resync),
        _ => Err(SessionError::Packet),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("peer is not assigned to that player")]
    Identity,
    #[error("invalid or oversized command packet")]
    Packet,
    #[error("commands for this player and tick were already submitted")]
    Duplicate,
    #[error("{0}")]
    Tick(String),
    #[error("{0}")]
    Replay(&'static str),
    #[error("player {player} is not admitted to this session")]
    NotAdmitted { player: u8 },
    #[error("the authority stopped: {0}")]
    Failed(String),
}

/// The authority a host serves: who may join, which packets count, and what the clock exports.
pub trait SessionAuthority {
    fn identity(&self) -> SessionIdentity;
    fn join(&mut self, peer: Peer, player: u8) -> Result<(), SessionError>;
    fn leave(&mut self, peer: Peer);
    fn receive(&mut self, peer: Peer, bytes: &[u8]) -> Result<Option<Vec<u8>>, SessionError>;
    /// Runs one clock step and returns the snapshot to send to every peer.
    fn advance(&mut self) -> Result<Vec<u8>, SessionError>;
    fn snapshot(&mut self) -> Result<Vec<u8>, SessionError>;
    /// The player slot for a new reconnect token, given how many tokens are already bound.
    fn player_for_new_token(&self, bound: usize) -> Option<u8>;
}

pub struct Session {
    server: Server,
    seed: u64,
    peers: BTreeMap<Peer, u8>,
    inputs: BTreeMap<u64, BTreeMap<u8, Vec<Command>>>,
    stops_pending: [bool; MAX_PLAYERS as usize],
    recorder: Option<Recorder>,
}

/// True when every command fits the packet limits and the sim's coordinate range.
pub(crate) fn commands_in_bounds(commands: &[Command]) -> bool {
    commands.len() <= MAX_COMMANDS_PER_TICK
        && commands.iter().all(|c| match c {
            Command::Move { x, y } | Command::Select { x, y } => {
                (-COORDINATE_LIMIT..=COORDINATE_LIMIT).contains(x)
                    && (-COORDINATE_LIMIT..=COORDINATE_LIMIT).contains(y)
            }
            _ => true,
        })
}

/// Clears move targets of units owned by a disconnected player, whatever their selection.
pub(crate) fn stop_owned_moves(world: &mut World, owners: &[u8]) {
    for unit in &mut world.units {
        if owners.contains(&unit.owner) {
            unit.move_target = None;
        }
    }
}

impl Session {
    pub fn new(world: World, seed: u64) -> Self {
        Self {
            server: Server::new(world, seed),
            seed,
            peers: BTreeMap::new(),
            inputs: BTreeMap::new(),
            stops_pending: [false; MAX_PLAYERS as usize],
            recorder: None,
        }
    }

    /// Starts a replay of this session from its current world.
    ///
    /// Only tick 0 qualifies: playback reseeds the RNG, and the RNG state after
    /// earlier ticks is not in the snapshot. Admitted movement is not stored; the
    /// caller passes the same admission to [`crate::replay::play`], and the final
    /// digest refuses a different one.
    pub fn start_recording(&mut self, map_sha1: [u8; 20]) -> Result<(), SessionError> {
        if self.recorder.is_some() {
            return Err(SessionError::Replay("this session is already recording"));
        }
        if self.world().tick != 0 {
            return Err(SessionError::Replay("a replay starts at tick 0 only"));
        }
        self.recorder = Some(Recorder::new(self.world(), self.seed, map_sha1));
        Ok(())
    }

    /// Ends recording and returns the replay bytes, or `None` when not recording.
    pub fn stop_recording(&mut self) -> Option<Vec<u8>> {
        let recorder = self.recorder.take()?;
        Some(recorder.finish(self.world()))
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    pub fn world(&self) -> &World {
        self.server.world()
    }

    pub fn join(&mut self, peer: Peer, player: u8) -> Result<(), SessionError> {
        if player >= MAX_PLAYERS
            || self.peers.contains_key(&peer)
            || self.peers.values().any(|p| *p == player)
        {
            return Err(SessionError::Identity);
        }
        self.peers.insert(peer, player);
        Ok(())
    }

    pub fn leave(&mut self, peer: Peer) {
        if let Some(player) = self.peers.remove(&peer) {
            for commands in self.inputs.values_mut() {
                commands.remove(&player);
            }
            // Drop buckets emptied by the removal above so a closed peer cannot
            // leave stale slots for later ticks.
            self.inputs.retain(|_, players| !players.is_empty());
            self.stops_pending[usize::from(player)] = true;
        }
    }

    pub fn snapshot(&self) -> Vec<u8> {
        encode(&Datagram::Snapshot {
            tick: self.world().tick,
            payload: encode_world_payload(self.world()),
        })
    }

    /// Resync sends current authority; a client cannot rewind the shared game.
    pub fn receive(&mut self, peer: Peer, bytes: &[u8]) -> Result<Option<Vec<u8>>, SessionError> {
        let assigned = *self.peers.get(&peer).ok_or(SessionError::Identity)?;
        match parse_packet(bytes, assigned)? {
            Packet::Input { tick, commands } => {
                pcx_sim::validate_input(
                    self.world(),
                    &Input {
                        tick,
                        commands: vec![],
                    },
                )
                .map_err(|e| SessionError::Tick(e.to_string()))?;
                if self
                    .inputs
                    .get(&tick)
                    .is_some_and(|players| players.contains_key(&assigned))
                {
                    return Err(SessionError::Duplicate);
                }
                self.inputs
                    .entry(tick)
                    .or_default()
                    .insert(assigned, commands);
                Ok(None)
            }
            Packet::Resync => Ok(Some(Session::snapshot(self))),
        }
    }

    /// Called only by the server's 25 ms clock. Missing commands mean no new orders;
    /// the existing sim still resolves movement, even if a browser disconnects.
    pub fn advance(&mut self) -> Vec<u8> {
        // Disconnect stops every owned unit, independently of UI selection.
        let stopped: Vec<u8> = (0..MAX_PLAYERS)
            .filter(|p| self.stops_pending[usize::from(*p)])
            .collect();
        stop_owned_moves(&mut self.server.world, &stopped);
        self.stops_pending.fill(false);
        let tick = self.world().tick;
        let commands = self
            .inputs
            .remove(&tick)
            .unwrap_or_default()
            .into_iter()
            .map(|(player, commands)| PlayerInput { player, commands })
            .collect();
        let input = Input { tick, commands };
        if let Some(recorder) = &mut self.recorder {
            recorder.push(stopped, &input);
        }
        self.server
            .accept(input)
            .expect("session batches match the authoritative tick");
        assert!(self.server.advance());
        Session::snapshot(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode_input_payload;

    #[test]
    fn identity_hex_round_trips_and_refuses_other_text() {
        let identity = SessionIdentity([0xab; 32]);
        assert_eq!(
            SessionIdentity::from_hex(&identity.to_hex()),
            Some(identity)
        );
        assert!(SessionIdentity::from_hex(&"AB".repeat(32)).is_none());
        assert!(SessionIdentity::from_hex(&"a".repeat(63)).is_none());
        assert!(SessionIdentity::from_hex(&"g".repeat(64)).is_none());
    }

    fn order(tick: u64, player: u8, x: i32) -> Vec<u8> {
        encode(&Datagram::Input {
            tick,
            player,
            payload: encode_input_payload(&[Command::Move { x, y: 0 }]),
        })
    }

    fn session() -> Session {
        let mut world = World::empty();
        world.spawn(0, 0, 0);
        world.spawn(1, 0, 0);
        let mut s = Session::new(world, 1);
        s.join(Peer(10), 0).unwrap();
        s.join(Peer(11), 1).unwrap();
        s
    }

    #[test]
    fn two_players_at_one_tick_do_not_overwrite_each_other() {
        let mut s = session();
        s.receive(Peer(11), &order(0, 1, -FX_ONE)).unwrap();
        s.receive(Peer(10), &order(0, 0, FX_ONE)).unwrap();
        s.advance();
        assert_eq!(s.world().units[0].x, FX_ONE / 10);
        assert_eq!(s.world().units[1].x, -FX_ONE / 10);
    }

    #[test]
    fn identity_horizon_duplicates_and_overflow_coordinates_are_rejected() {
        let mut s = session();
        assert!(matches!(
            s.receive(Peer(10), &order(0, 1, FX_ONE)),
            Err(SessionError::Identity)
        ));
        assert!(matches!(
            s.receive(Peer(10), &order(pcx_sim::MAX_INPUT_LEAD + 1, 0, FX_ONE)),
            Err(SessionError::Tick(_))
        ));
        assert!(matches!(
            s.receive(Peer(10), &order(0, 0, i32::MIN)),
            Err(SessionError::Packet)
        ));
        s.receive(Peer(10), &order(0, 0, FX_ONE)).unwrap();
        assert!(matches!(
            s.receive(Peer(10), &order(0, 0, -FX_ONE)),
            Err(SessionError::Duplicate)
        ));
    }

    #[test]
    fn leave_prunes_empty_future_input_buckets() {
        let mut s = session();
        s.receive(Peer(11), &order(5, 1, FX_ONE)).unwrap();
        assert!(s.inputs.contains_key(&5));
        s.leave(Peer(11));
        assert!(
            !s.inputs.contains_key(&5),
            "leave must prune emptied future buckets"
        );
    }

    #[test]
    fn reconnect_resync_does_not_rewind_and_disconnect_drops_pending_orders() {
        let mut s = session();
        s.receive(Peer(10), &order(1, 0, FX_ONE)).unwrap();
        s.leave(Peer(10));
        s.advance();
        s.advance();
        assert_eq!(s.world().units[0].x, 0);
        s.join(Peer(12), 0).unwrap();
        let before = s.world().clone();
        let snapshot = s
            .receive(Peer(12), &encode(&Datagram::Resync { from_tick: 0 }))
            .unwrap()
            .unwrap();
        assert_eq!(snapshot, s.snapshot());
        assert_eq!(s.world(), &before);
        assert_eq!(before.tick, 2);
    }
}

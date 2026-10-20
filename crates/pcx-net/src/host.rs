//! Local WebSocket session host: admits peers by reconnect token, binds each
//! connection to a player slot and drives the shared [`Session`] clock.
//!
//! The caller owns the loop. [`Host::poll`] performs one bounded service pass,
//! so a test or launcher can run it on a thread and stop it deterministically.

//! Local WebSocket session host: admits peers by reconnect token, binds each connection to a
//! player slot and drives a [`SessionAuthority`] clock. The default authority is the harness
//! [`Session`].
//!
//! The caller owns the loop. [`Host::poll`] performs one bounded service pass, so a test or
//! launcher can run it on a thread and stop it deterministically.

use std::{
    collections::BTreeMap,
    io::{self, ErrorKind},
    net::{SocketAddr, TcpListener, TcpStream},
    time::{Duration, Instant},
};

use pcx_sim::{FX_ONE, TICK_MS, World};
use sha2::{Digest, Sha256};
use tungstenite::{Message, WebSocket, protocol::WebSocketConfig};

use crate::{
    Peer, encode_world_payload,
    session::{
        MAX_PACKET_BYTES, MAX_PLAYERS, Session, SessionAuthority, SessionError, SessionIdentity,
    },
};

mod admission;

const TICK_INTERVAL: Duration = Duration::from_millis(TICK_MS as u64);
// Joining requires a token message; an idle socket cannot hold a slot open.
const JOIN_DEADLINE: Duration = Duration::from_secs(5);
// Bounds per-peer work per pass so one busy browser cannot monopolize the clock.
const MESSAGES_PER_PEER_PER_POLL: usize = 32;
const WRITE_BUFFER_BYTES: usize = 64 * 1024;
const TOKEN_HEX_LEN: usize = 32;

struct Connection {
    socket: WebSocket<admission::Io<TcpStream>>,
    player: Option<u8>,
    deadline: Instant,
}

fn blocked(error: &tungstenite::Error) -> bool {
    matches!(error, tungstenite::Error::Io(e) if e.kind() == ErrorKind::WouldBlock)
}

fn send(connection: &mut Connection, message: Message) -> bool {
    match connection.socket.send(message) {
        Ok(()) => true,
        Err(e) if blocked(&e) => true, // tungstenite retains the unfinished frame for flush().
        Err(_) => false,
    }
}

/// The harness world: one unit per player slot, placed at fixed offsets.
///
/// Stands in for map initialization, which is not wired to this session yet.
pub fn local_world() -> World {
    let mut world = World::empty();
    for player in 0..MAX_PLAYERS {
        world.spawn(player, player as i32 * 2 * FX_ONE, 0);
    }
    world
}

/// A TCP/WebSocket host for one authority. The default is the harness [`Session`].
pub struct Host<A: SessionAuthority = Session> {
    listener: TcpListener,
    authority: A,
    identity: SessionIdentity,
    connections: BTreeMap<Peer, Connection>,
    pending: Vec<admission::Pending<TcpStream>>,
    slots: BTreeMap<String, u8>,
    next_peer: u32,
    next_tick: Instant,
}

impl Host {
    /// Hosts the harness session over `world`, with `seed` as its new-game seed.
    pub fn bind(address: &str, world: World, seed: u64) -> io::Result<Self> {
        Self::with_authority(address, Session::new(world, seed))
    }
}

impl<A: SessionAuthority> Host<A> {
    /// Every joining peer is told this authority's identity and must verify it before play.
    pub fn with_authority(address: &str, authority: A) -> io::Result<Self> {
        let listener = TcpListener::bind(address)?;
        listener.set_nonblocking(true)?;
        Ok(Self {
            listener,
            identity: authority.identity(),
            authority,
            connections: BTreeMap::new(),
            pending: Vec::new(),
            slots: BTreeMap::new(),
            next_peer: 0,
            next_tick: Instant::now() + TICK_INTERVAL,
        })
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    /// Runs one service pass at `now`: accepts, admits, reads, closes, then advances
    /// the shared clock when a tick is due. An authority that stops ends the pass with an error.
    pub fn poll(&mut self, now: Instant) -> io::Result<()> {
        if let Ok((stream, _)) = self.listener.accept()
            && self.connections.len().saturating_add(self.pending.len()) < MAX_PLAYERS as usize
        {
            stream.set_nonblocking(true)?;
            stream.set_nodelay(true)?;
            let config = WebSocketConfig::default()
                .max_message_size(Some(MAX_PACKET_BYTES))
                .max_frame_size(Some(MAX_PACKET_BYTES))
                .max_write_buffer_size(WRITE_BUFFER_BYTES)
                .write_buffer_size(0);
            self.pending
                .push(admission::Pending::new(stream, config, now));
        }
        let mut at = 0;
        while at < self.pending.len() {
            match self.pending[at].poll(now) {
                Ok(None) => at += 1,
                Ok(Some(socket)) => {
                    self.pending.swap_remove(at);
                    self.next_peer = self
                        .next_peer
                        .checked_add(1)
                        .ok_or_else(|| io::Error::other("peer IDs exhausted"))?;
                    self.connections.insert(
                        Peer(self.next_peer),
                        Connection {
                            socket,
                            player: None,
                            deadline: now + JOIN_DEADLINE,
                        },
                    );
                }
                Err(_) => {
                    self.pending.swap_remove(at);
                }
            }
        }
        let mut closed = vec![];
        for (&peer, connection) in &mut self.connections {
            match connection.socket.flush() {
                Err(e) if !blocked(&e) => {
                    closed.push(peer);
                    continue;
                }
                _ => {}
            }
            if connection.player.is_none() && now > connection.deadline {
                closed.push(peer);
                continue;
            }
            for _ in 0..MESSAGES_PER_PEER_PER_POLL {
                match connection.socket.read() {
                    Ok(Message::Text(token)) if connection.player.is_none() => {
                        if token.len() != TOKEN_HEX_LEN
                            || !token.bytes().all(|c| c.is_ascii_hexdigit())
                        {
                            closed.push(peer);
                            break;
                        }
                        let player = if let Some(player) = self.slots.get(token.as_str()) {
                            *player
                        } else {
                            let Some(player) =
                                self.authority.player_for_new_token(self.slots.len())
                            else {
                                closed.push(peer);
                                break;
                            };
                            self.slots.insert(token.to_string(), player);
                            player
                        };
                        if self.authority.join(peer, player).is_err() {
                            closed.push(peer);
                            break;
                        }
                        connection.player = Some(player);
                        let hello = format!(
                            "{{\"player\":{player},\"tickMs\":{TICK_MS},\"session\":\"{}\"}}",
                            self.identity.to_hex()
                        );
                        let Ok(snapshot) = self.authority.snapshot() else {
                            closed.push(peer);
                            break;
                        };
                        if !send(connection, Message::Text(hello.into()))
                            || !send(connection, Message::Binary(snapshot.into()))
                        {
                            closed.push(peer);
                            break;
                        }
                    }
                    Ok(Message::Binary(bytes)) if connection.player.is_some() => {
                        match self.authority.receive(peer, &bytes) {
                            Ok(Some(reply)) => {
                                if !send(connection, Message::Binary(reply.into())) {
                                    closed.push(peer);
                                    break;
                                }
                            }
                            Ok(None) => {}
                            Err(error) => {
                                if !send(
                                    connection,
                                    Message::Text(format!("{{\"error\":\"{error}\"}}").into()),
                                ) {
                                    closed.push(peer);
                                    break;
                                }
                            }
                        }
                    }
                    Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => {}
                    Ok(_) => {
                        closed.push(peer);
                        break;
                    }
                    Err(e) if blocked(&e) => break,
                    Err(_) => {
                        closed.push(peer);
                        break;
                    }
                }
            }
        }
        closed.sort();
        closed.dedup();
        for peer in closed {
            self.connections.remove(&peer);
            self.authority.leave(peer);
        }
        if now >= self.next_tick {
            let snapshot = self
                .authority
                .advance()
                .map_err(|error| io::Error::other(error.to_string()))?;
            let authority = &mut self.authority;
            self.connections.retain(|&peer, connection| {
                if connection.player.is_none()
                    || send(connection, Message::Binary(snapshot.clone().into()))
                {
                    true
                } else {
                    authority.leave(peer);
                    false
                }
            });
            // Bound catch-up work after scheduler delays.
            self.next_tick += TICK_INTERVAL;
            if self.next_tick < now {
                self.next_tick = now + TICK_INTERVAL;
            }
        }
        Ok(())
    }
}

/// The identity of the harness session: its seed and initial world. Peers compare it before play.
pub fn session_identity(world: &World, seed: u64) -> SessionIdentity {
    let mut hash = Sha256::new();
    hash.update(b"pcx-session-world-v1\n");
    hash.update(seed.to_le_bytes());
    hash.update(encode_world_payload(world));
    SessionIdentity(hash.finalize().into())
}

impl SessionAuthority for Session {
    fn identity(&self) -> SessionIdentity {
        session_identity(self.world(), self.seed())
    }
    fn join(&mut self, peer: Peer, player: u8) -> Result<(), SessionError> {
        Session::join(self, peer, player)
    }
    fn leave(&mut self, peer: Peer) {
        Session::leave(self, peer);
    }
    fn receive(&mut self, peer: Peer, bytes: &[u8]) -> Result<Option<Vec<u8>>, SessionError> {
        Session::receive(self, peer, bytes)
    }
    fn advance(&mut self) -> Result<Vec<u8>, SessionError> {
        Ok(Session::advance(self))
    }
    fn snapshot(&mut self) -> Result<Vec<u8>, SessionError> {
        Ok(Session::snapshot(self))
    }
    fn player_for_new_token(&self, bound: usize) -> Option<u8> {
        (bound < MAX_PLAYERS as usize).then_some(bound as u8)
    }
}

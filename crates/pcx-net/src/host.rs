//! Local WebSocket session host: admits peers by reconnect token, binds each
//! connection to a player slot and drives the shared [`Session`] clock.
//!
//! The caller owns the loop. [`Host::poll`] performs one bounded service pass,
//! so a test or launcher can run it on a thread and stop it deterministically.

use std::{
    collections::BTreeMap,
    io::{self, ErrorKind},
    net::{SocketAddr, TcpListener, TcpStream},
    time::{Duration, Instant},
};

use pcx_sim::{FX_ONE, TICK_MS, World};
use tungstenite::{Message, WebSocket, protocol::WebSocketConfig};

use crate::{
    Peer,
    session::{MAX_PACKET_BYTES, MAX_PLAYERS, Session},
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

pub struct Host {
    listener: TcpListener,
    session: Session,
    connections: BTreeMap<Peer, Connection>,
    pending: Vec<admission::Pending<TcpStream>>,
    slots: BTreeMap<String, u8>,
    next_peer: u32,
    next_tick: Instant,
}

impl Host {
    pub fn bind(address: &str, world: World, seed: u64) -> io::Result<Self> {
        let listener = TcpListener::bind(address)?;
        listener.set_nonblocking(true)?;
        Ok(Self {
            listener,
            session: Session::new(world, seed),
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
    /// the shared clock when a tick is due.
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
                            if self.slots.len() >= MAX_PLAYERS as usize {
                                closed.push(peer);
                                break;
                            }
                            let player = self.slots.len() as u8;
                            self.slots.insert(token.to_string(), player);
                            player
                        };
                        if self.session.join(peer, player).is_err() {
                            closed.push(peer);
                            break;
                        }
                        connection.player = Some(player);
                        let hello = format!("{{\"player\":{player},\"tickMs\":{TICK_MS}}}");
                        if !send(connection, Message::Text(hello.into()))
                            || !send(connection, Message::Binary(self.session.snapshot().into()))
                        {
                            closed.push(peer);
                            break;
                        }
                    }
                    Ok(Message::Binary(bytes)) if connection.player.is_some() => {
                        match self.session.receive(peer, &bytes) {
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
            self.session.leave(peer);
        }
        if now >= self.next_tick {
            let snapshot = self.session.advance();
            let session = &mut self.session;
            self.connections.retain(|&peer, connection| {
                if connection.player.is_none()
                    || send(connection, Message::Binary(snapshot.clone().into()))
                {
                    true
                } else {
                    session.leave(peer);
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

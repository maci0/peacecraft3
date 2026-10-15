//! Local session host: cargo run -p pcx-net --features session-server --example session_server
#[cfg(not(target_arch = "wasm32"))]
#[path = "session_server/admission.rs"]
mod admission;

#[cfg(not(target_arch = "wasm32"))]
use std::{
    collections::BTreeMap,
    io,
    net::{TcpListener, TcpStream},
    time::{Duration, Instant},
};

#[cfg(not(target_arch = "wasm32"))]
use pcx_net::{
    Peer,
    session::{MAX_PACKET_BYTES, MAX_PLAYERS, Session},
};
#[cfg(not(target_arch = "wasm32"))]
use pcx_sim::{FX_ONE, TICK_MS, World};
#[cfg(not(target_arch = "wasm32"))]
use tungstenite::{Message, WebSocket, protocol::WebSocketConfig};

#[cfg(not(target_arch = "wasm32"))]
struct Connection {
    socket: WebSocket<admission::Io<TcpStream>>,
    player: Option<u8>,
    deadline: Instant,
}

#[cfg(not(target_arch = "wasm32"))]
fn blocked(error: &tungstenite::Error) -> bool {
    matches!(error, tungstenite::Error::Io(e) if e.kind() == io::ErrorKind::WouldBlock)
}

#[cfg(not(target_arch = "wasm32"))]
fn send(connection: &mut Connection, message: Message) -> bool {
    match connection.socket.send(message) {
        Ok(()) => true,
        Err(e) if blocked(&e) => true, // tungstenite retains the unfinished frame for flush().
        Err(_) => false,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8932".into());
    let listener = TcpListener::bind(&address)?;
    listener.set_nonblocking(true)?;
    let mut world = World::empty();
    for player in 0..MAX_PLAYERS {
        world.spawn(player, player as i32 * 2 * FX_ONE, 0);
    }
    let mut session = Session::new(world, 1);
    let mut connections: BTreeMap<Peer, Connection> = BTreeMap::new();
    let mut pending: Vec<admission::Pending<TcpStream>> = Vec::new();
    let mut slots: BTreeMap<String, u8> = BTreeMap::new();
    let mut next_peer = 0u32;
    let interval = Duration::from_millis(TICK_MS as u64);
    let mut next_tick = Instant::now() + interval;
    println!(
        "PeaceCraft session listening on ws://{address}; local movement simulation, not a complete Warcraft match"
    );
    loop {
        if let Ok((stream, _)) = listener.accept()
            && connections.len().saturating_add(pending.len()) < MAX_PLAYERS as usize
        {
            stream.set_nonblocking(true)?;
            stream.set_nodelay(true)?;
            let config = WebSocketConfig::default()
                .max_message_size(Some(MAX_PACKET_BYTES))
                .max_frame_size(Some(MAX_PACKET_BYTES))
                .max_write_buffer_size(64 * 1024)
                .write_buffer_size(0);
            pending.push(admission::Pending::new(stream, config, Instant::now()));
        }
        let mut at = 0;
        while at < pending.len() {
            match pending[at].poll(Instant::now()) {
                Ok(None) => at += 1,
                Ok(Some(socket)) => {
                    pending.swap_remove(at);
                    next_peer = next_peer.checked_add(1).ok_or("peer IDs exhausted")?;
                    connections.insert(
                        Peer(next_peer),
                        Connection {
                            socket,
                            player: None,
                            deadline: Instant::now() + Duration::from_secs(5),
                        },
                    );
                }
                Err(_) => {
                    pending.swap_remove(at);
                }
            }
        }
        let mut closed = vec![];
        for (&peer, connection) in &mut connections {
            match connection.socket.flush() {
                Err(e) if !blocked(&e) => {
                    closed.push(peer);
                    continue;
                }
                _ => {}
            }
            if connection.player.is_none() && Instant::now() > connection.deadline {
                closed.push(peer);
                continue;
            }
            // Bound per-peer work so one busy browser cannot monopolize the clock.
            for _ in 0..32 {
                match connection.socket.read() {
                    Ok(Message::Text(token)) if connection.player.is_none() => {
                        if token.len() != 32 || !token.bytes().all(|c| c.is_ascii_hexdigit()) {
                            closed.push(peer);
                            break;
                        }
                        let player = if let Some(player) = slots.get(token.as_str()) {
                            *player
                        } else {
                            if slots.len() >= MAX_PLAYERS as usize {
                                closed.push(peer);
                                break;
                            }
                            let player = slots.len() as u8;
                            slots.insert(token.to_string(), player);
                            player
                        };
                        if session.join(peer, player).is_err() {
                            closed.push(peer);
                            break;
                        }
                        connection.player = Some(player);
                        let hello = format!("{{\"player\":{player},\"tickMs\":{TICK_MS}}}");
                        if !send(connection, Message::Text(hello.into()))
                            || !send(connection, Message::Binary(session.snapshot().into()))
                        {
                            closed.push(peer);
                            break;
                        }
                    }
                    Ok(Message::Binary(bytes)) if connection.player.is_some() => {
                        match session.receive(peer, &bytes) {
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
            connections.remove(&peer);
            session.leave(peer);
        }
        if Instant::now() >= next_tick {
            let snapshot = session.advance();
            connections.retain(|&peer, connection| {
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
            next_tick += interval;
            if next_tick < Instant::now() {
                next_tick = Instant::now() + interval;
            }
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[cfg(target_arch = "wasm32")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "the TCP/WebSocket session server requires a native target",
    )
    .into())
}

//! Native peer for the local session host. It speaks the binary protocol that
//! `web/network.js` speaks, so both clients observe the same authority.

use std::{io, net::TcpStream, time::Duration};

use pcx_sim::{Command, World};
use tungstenite::{Message, WebSocket, client::client, handshake::HandshakeError};

use crate::{
    CodecError, Datagram, decode, decode_world_payload, encode, encode_input_payload,
    session::{MAX_COMMANDS_PER_TICK, MAX_PLAYERS, SessionIdentity},
};

// Commands are submitted this many ticks ahead of the newest snapshot, as
// `web/network.js` does, so a host that has moved on by a tick or two still accepts them.
const INPUT_LEAD_TICKS: u64 = 4;
const READ_TIMEOUT: Duration = Duration::from_secs(5);
const TOKEN_HEX_LEN: usize = 32;

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("reconnect token must be 32 hex digits")]
    Token,
    #[error("session connection: {0}")]
    Io(#[from] io::Error),
    #[error("session socket: {0}")]
    Socket(#[from] tungstenite::Error),
    #[error("invalid session packet: {0}")]
    Codec(#[from] CodecError),
    #[error("unexpected session message: {0}")]
    Protocol(String),
    #[error("host session {host:?} does not match this peer's admitted session {expected}")]
    Identity {
        expected: String,
        host: Option<String>,
    },
    #[error("a tick carries at most the session limit of commands; this batch has {0}")]
    TooManyCommands(usize),
}

/// What the host sent in reply to a peer's packet.
#[derive(Debug, PartialEq)]
pub enum Received {
    /// The authoritative world at its tick.
    Snapshot(World),
    /// The host's refusal of the last packet, verbatim.
    Rejected(String),
}

pub struct SessionClient {
    socket: WebSocket<TcpStream>,
    player: u8,
    identity: Option<SessionIdentity>,
    /// Tick of the newest snapshot received.
    latest_tick: u64,
    /// Tick of the newest input submitted, if any.
    last_input: Option<u64>,
}

impl SessionClient {
    /// Joins the host at `address` (`host:port`) as the player bound to `token`.
    ///
    /// The token is the reconnect identity: the same token reclaims the same slot.
    pub fn connect(address: &str, token: &str) -> Result<Self, ClientError> {
        if token.len() != TOKEN_HEX_LEN || !token.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err(ClientError::Token);
        }
        let stream = TcpStream::connect(address)?;
        stream.set_nodelay(true)?;
        stream.set_read_timeout(Some(READ_TIMEOUT))?;
        stream.set_write_timeout(Some(READ_TIMEOUT))?;
        let (mut socket, _) = client(format!("ws://{address}/"), stream).map_err(|e| match e {
            HandshakeError::Failure(error) => error,
            HandshakeError::Interrupted(_) => {
                tungstenite::Error::Io(io::Error::other("blocking handshake interrupted"))
            }
        })?;
        socket.send(Message::Text(token.into()))?;
        let (player, identity) = match socket.read()? {
            Message::Text(hello) => parse_hello(&hello)?,
            other => return Err(ClientError::Protocol(format!("{other:?}"))),
        };
        Ok(Self {
            socket,
            player,
            identity,
            latest_tick: 0,
            last_input: None,
        })
    }

    /// Joins like [`SessionClient::connect`], then refuses unless the host announces
    /// exactly `expected`. The socket closes on refusal, before any command is sent.
    pub fn connect_admitted(
        address: &str,
        token: &str,
        expected: &SessionIdentity,
    ) -> Result<Self, ClientError> {
        let client = Self::connect(address, token)?;
        if client.identity.as_ref() != Some(expected) {
            return Err(ClientError::Identity {
                expected: expected.to_hex(),
                host: client.identity.as_ref().map(SessionIdentity::to_hex),
            });
        }
        Ok(client)
    }

    pub fn player(&self) -> u8 {
        self.player
    }

    /// The identity the host announced, or `None` for an unadmitted host.
    pub fn identity(&self) -> Option<&SessionIdentity> {
        self.identity.as_ref()
    }

    /// Submits this player's commands for the next free tick ahead of the newest snapshot.
    pub fn submit(&mut self, commands: &[Command]) -> Result<u64, ClientError> {
        if commands.len() > MAX_COMMANDS_PER_TICK {
            return Err(ClientError::TooManyCommands(commands.len()));
        }
        let ahead = self.latest_tick + INPUT_LEAD_TICKS;
        let tick = match self.last_input {
            Some(last) => ahead.max(last + 1),
            None => ahead,
        };
        self.send_datagram(&Datagram::Input {
            tick,
            player: self.player,
            payload: encode_input_payload(commands),
        })?;
        self.last_input = Some(tick);
        Ok(tick)
    }

    /// Sends a raw datagram. The host binds identity to the socket, so `player` in
    /// an input is checked against the slot this peer joined as.
    pub fn send_datagram(&mut self, datagram: &Datagram) -> Result<(), ClientError> {
        self.socket
            .send(Message::Binary(encode(datagram).into()))
            .map_err(ClientError::from)
    }

    /// Asks the host for its current authority. It never rewinds the shared world.
    pub fn resync(&mut self) -> Result<(), ClientError> {
        self.send_datagram(&Datagram::Resync {
            from_tick: self.latest_tick,
        })
    }

    /// Blocks until the host sends a snapshot or refuses a packet.
    pub fn receive(&mut self) -> Result<Received, ClientError> {
        match self.socket.read()? {
            Message::Binary(bytes) => match decode(&bytes)? {
                Datagram::Snapshot { tick, payload } => {
                    let world = decode_world_payload(tick, &payload)?;
                    self.latest_tick = self.latest_tick.max(tick);
                    Ok(Received::Snapshot(world))
                }
                other => Err(ClientError::Protocol(format!("{other:?}"))),
            },
            Message::Text(refusal) => Ok(Received::Rejected(refusal.to_string())),
            other => Err(ClientError::Protocol(format!("{other:?}"))),
        }
    }
}

/// Parses the host's greeting: `{"player":N,"tickMs":M}`, plus `,"session":"<hex>"` when
/// the host admitted a map. Anything else is refused.
fn parse_hello(hello: &str) -> Result<(u8, Option<SessionIdentity>), ClientError> {
    let refused = || ClientError::Protocol(hello.to_string());
    let body = hello
        .strip_prefix("{\"player\":")
        .and_then(|rest| rest.strip_suffix('}'))
        .ok_or_else(refused)?;
    let mut fields = body.split(',');
    let player = fields
        .next()
        .and_then(|digits| digits.parse::<u8>().ok())
        .filter(|player| *player < MAX_PLAYERS)
        .ok_or_else(refused)?;
    fields
        .next()
        .and_then(|field| field.strip_prefix("\"tickMs\":"))
        .and_then(|ms| ms.parse::<u32>().ok())
        .ok_or_else(refused)?;
    let identity = match fields.next() {
        None => None,
        Some(field) => {
            let hex = field
                .strip_prefix("\"session\":\"")
                .and_then(|rest| rest.strip_suffix('"'))
                .ok_or_else(refused)?;
            Some(SessionIdentity::from_hex(hex).ok_or_else(refused)?)
        }
    };
    if fields.next().is_some() {
        return Err(refused());
    }
    Ok((player, identity))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIGEST: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

    #[test]
    fn greeting_yields_the_assigned_slot_and_optional_identity() {
        assert_eq!(
            parse_hello("{\"player\":3,\"tickMs\":25}").unwrap(),
            (3, None)
        );
        let admitted = format!("{{\"player\":15,\"tickMs\":25,\"session\":\"{DIGEST}\"}}");
        let (player, identity) = parse_hello(&admitted).unwrap();
        assert_eq!(player, 15);
        assert_eq!(identity.unwrap().to_hex(), DIGEST);
    }

    #[test]
    fn malformed_or_out_of_range_greetings_are_refused() {
        for bad in [
            "{\"player\":16,\"tickMs\":25}",
            "{\"player\":-1,\"tickMs\":25}",
            "{\"player\":256,\"tickMs\":25}",
            "{\"error\":\"nope\"}",
            "{\"tickMs\":25}",
            "{\"player\":1}",
            "{\"player\":1,\"tickMs\":25,\"session\":\"abc\"}",
            "{\"player\":1,\"tickMs\":25,\"session\":\"ZZ\"}",
            "{\"player\":1,\"tickMs\":25,\"session\":\"00\",\"extra\":1}",
            "{\"player\":1,\"tickMs\":25}trailing",
        ] {
            assert!(
                matches!(parse_hello(bad), Err(ClientError::Protocol(_))),
                "accepted {bad}"
            );
        }
    }
}

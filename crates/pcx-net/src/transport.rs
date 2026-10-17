//! A real UDP transport, plus the LAN discovery that finds a game to join.
//!
//! Desktop-only. WASM has no UDP in the browser, so the wasm build gets the
//! trait and the codec but not this file — which is why the trait exists at all.
//!
//! This is the project's own discovery/datagram protocol, not classic WC3 wire
//! compatibility. Discovery is separate from game transport, so browsing hosts
//! does not require joining one.

use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use crate::{Peer, Transport};

/// Chosen project discovery port; native port/configuration parity is unverified.
pub const LAN_PORT: u16 = 6112;

/// How long a discovery request waits for responses.
pub const DISCOVERY_TIMEOUT: Duration = Duration::from_millis(750);

/// Sent by a client browsing for LAN games.
pub const DISCOVERY_REQUEST: &[u8] = b"PEACECRAFT3-DISCOVER-v1";

/// Prefix of a host's response to [`DISCOVERY_REQUEST`].
pub const DISCOVERY_RESPONSE_PREFIX: &[u8] = b"PEACECRAFT3-HOST-v1 ";

// Receive must provide the complete span accepted by the existing send contract.
const MAX_DATAGRAM_BYTES: usize = u16::MAX as usize;

#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("peer {} was never registered with this transport", .0.0)]
    UnknownPeer(Peer),
    #[error("datagram of {0} bytes exceeds the {max}-byte datagram limit", max = u16::MAX as usize)]
    DatagramTooLarge(usize),
    #[error(
        "failed restoring discovery socket nonblocking mode: {restore}; collection failure: {collection:?}"
    )]
    DiscoveryRestore {
        #[source]
        restore: std::io::Error,
        collection: Option<Box<TransportError>>,
    },
}

/// A `Peer` backed by a real socket address.
#[derive(Debug, Default)]
pub struct UdpTransport {
    socket: Option<UdpSocket>,
    /// Registration table: a `Peer` indexes caller-registered endpoints.
    /// Source-address matching is not endpoint authentication.
    peers: HashMap<Peer, SocketAddr>,
    /// Reverse map, so an incoming datagram can be attributed to a `Peer`.
    by_addr: HashMap<SocketAddr, Peer>,
    next: u32,
}

impl UdpTransport {
    /// Binds to `addr` and returns a transport ready to use.
    ///
    /// Broadcasting is enabled because LAN discovery depends on it; a socket
    /// that cannot broadcast would make discovery silently find nothing.
    pub fn bind(addr: &str) -> Result<Self, TransportError> {
        let socket = UdpSocket::bind(addr)?;
        socket.set_broadcast(true)?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket: Some(socket),
            ..Default::default()
        })
    }

    /// Registers a peer, returning its handle.
    pub fn register(&mut self, addr: SocketAddr) -> Peer {
        if let Some(p) = self.by_addr.get(&addr) {
            return *p;
        }
        let peer = Peer(self.next);
        self.next += 1;
        self.peers.insert(peer, addr);
        self.by_addr.insert(addr, peer);
        peer
    }

    /// The address a peer was registered with.
    pub fn address(&self, peer: Peer) -> Option<SocketAddr> {
        self.peers.get(&peer).copied()
    }

    /// Sends a discovery request to every host on this LAN.
    pub fn discover(&mut self) -> Result<(), TransportError> {
        let Some(socket) = self.socket.as_ref() else {
            return Err(TransportError::Io(std::io::Error::other("not bound")));
        };
        let broadcast = format!("255.255.255.255:{LAN_PORT}");
        socket.send_to(DISCOVERY_REQUEST, broadcast)?;
        Ok(())
    }

    /// Collects discovery responses until [`DISCOVERY_TIMEOUT`] expires.
    ///
    /// Responses are bound to their UDP source; the payload carries no address.
    /// Collection temporarily blocks, then restores nonblocking game polling on
    /// both success and error. A restoration failure retains any collection error.
    /// The window stays open after the first reply so a later host is not dropped.
    pub fn collect_hosts(&mut self) -> Result<Vec<Host>, TransportError> {
        self.collect_until(DISCOVERY_TIMEOUT, false)
    }

    /// `stop_after_first` is for tests that already know which reply they expect.
    /// Production browsing passes `false` and waits out the full window.
    fn collect_until(
        &mut self,
        window: Duration,
        stop_after_first: bool,
    ) -> Result<Vec<Host>, TransportError> {
        let Some(socket) = self.socket.as_ref() else {
            return Err(TransportError::Io(std::io::Error::other("not bound")));
        };
        socket.set_nonblocking(false)?;
        let collected = (|| {
            let deadline = Instant::now() + window;
            let mut out = Vec::new();
            let mut buf = [0u8; 2048];
            loop {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    break;
                }
                socket.set_read_timeout(Some(remaining))?;
                match socket.recv_from(&mut buf) {
                    Ok((n, from)) => {
                        if let Some(host) = Host::parse(&buf[..n]) {
                            out.push(Host { addr: from, ..host });
                            if stop_after_first {
                                break;
                            }
                        }
                    }
                    Err(e)
                        if e.kind() == std::io::ErrorKind::WouldBlock
                            || e.kind() == std::io::ErrorKind::TimedOut =>
                    {
                        break;
                    }
                    Err(e) => return Err(e.into()),
                }
            }
            out.sort_by_key(|h| h.name.clone());
            out.dedup_by(|a, b| a.addr == b.addr);
            Ok(out)
        })();
        match socket.set_nonblocking(true) {
            Ok(()) => collected,
            Err(restore) => Err(TransportError::DiscoveryRestore {
                restore,
                collection: collected.err().map(Box::new),
            }),
        }
    }
}

/// A LAN game, as advertised by a host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Host {
    pub name: String,
    pub addr: SocketAddr,
    /// The map or scenario being played, when the host advertises one.
    pub map: String,
    /// Current and maximum players.
    pub players: (u8, u8),
}

impl Host {
    /// Serialises a host for broadcast.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = DISCOVERY_RESPONSE_PREFIX.to_vec();
        out.extend_from_slice(self.name.as_bytes());
        out.push(b'\n');
        out.extend_from_slice(self.map.as_bytes());
        out.push(b'\n');
        out.extend_from_slice(format!("{} {}", self.players.0, self.players.1).as_bytes());
        out
    }

    /// Parses a discovery response, refusing anything that does not fit.
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let rest = bytes.strip_prefix(DISCOVERY_RESPONSE_PREFIX)?;
        let text = std::str::from_utf8(rest).ok()?;
        let mut lines = text.split('\n');
        let name = lines.next()?.to_string();
        // An advertisement with no name is refused rather than shown as a blank
        // entry: a host that cannot describe itself is a malformed host.
        if name.is_empty() {
            return None;
        }
        let map = lines.next().unwrap_or_default().to_string();
        let counts = lines.next().unwrap_or_default();
        let mut parts = counts.split_whitespace();
        let players = (
            parts.next().and_then(|s| s.parse().ok()).unwrap_or(0),
            parts.next().and_then(|s| s.parse().ok()).unwrap_or(0),
        );
        Some(Self {
            name,
            addr: "0.0.0.0:0".parse().ok()?,
            map,
            players,
        })
    }
}

impl Transport for UdpTransport {
    type Error = TransportError;

    fn send(&mut self, to: Peer, datagram: &[u8]) -> Result<(), Self::Error> {
        if datagram.len() > MAX_DATAGRAM_BYTES {
            return Err(TransportError::DatagramTooLarge(datagram.len()));
        }
        let Some(addr) = self.peers.get(&to).copied() else {
            return Err(TransportError::UnknownPeer(to));
        };
        let Some(socket) = self.socket.as_ref() else {
            return Err(TransportError::Io(std::io::Error::other("not bound")));
        };
        socket.send_to(datagram, addr)?;
        Ok(())
    }

    fn poll(&mut self) -> Option<(Peer, Vec<u8>)> {
        let socket = self.socket.as_ref()?;
        let mut buf = [0u8; MAX_DATAGRAM_BYTES];
        // Non-blocking, so a poll is a poll rather than a wait.
        match socket.recv_from(&mut buf) {
            Ok((n, from)) => {
                let peer = *self.by_addr.get(&from)?;
                Some((peer, buf[..n].to_vec()))
            }
            Err(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_host_advertisement_survives_a_round_trip() {
        let original = Host {
            name: "maci's game".into(),
            addr: "192.168.1.10:6112".parse().unwrap(),
            map: "Maps/(2)BootyBay.w3m".into(),
            players: (3, 5),
        };
        let bytes = original.encode();
        let parsed = Host::parse(&bytes).expect("parse");
        assert_eq!(parsed.name, original.name);
        assert_eq!(parsed.map, original.map);
        assert_eq!(parsed.players, original.players);
        // The address is deliberately not serialized: the receiver already knows
        // it from the datagram's source, and carrying it would let one host
        // advertise another's address.
    }

    #[test]
    fn a_malformed_advertisement_is_refused() {
        assert!(Host::parse(b"not a host").is_none(), "wrong prefix");
        assert!(Host::parse(DISCOVERY_RESPONSE_PREFIX).is_none(), "no name");
        assert!(
            Host::parse(b"PEACECRAFT3-HOST-v1 name\n").is_some(),
            "a host need not advertise a map"
        );
    }

    /// Datagrams travel through the real UDP stack here, so this is a genuine
    /// loopback test rather than a mock.
    #[test]
    fn datagrams_travel_over_real_udp() {
        let Ok(mut a) = UdpTransport::bind("127.0.0.1:0") else {
            eprintln!("skipping: no loopback");
            return;
        };
        let Ok(mut b) = UdpTransport::bind("127.0.0.1:0") else {
            eprintln!("skipping: no loopback");
            return;
        };
        let a_addr = a.address(Peer(0)).unwrap_or_else(|| {
            // The transport binds but does not register itself; read it from the
            // socket instead, which is the only way to learn a bound port.
            a.socket.as_ref().unwrap().local_addr().unwrap()
        });
        let b_addr = b.socket.as_ref().unwrap().local_addr().unwrap();
        let b_peer = a.register(b_addr);
        let a_peer = b.register(a_addr);

        let payload = super::super::encode(&crate::Datagram::InputAck { tick: 42 });
        a.send(b_peer, &payload).expect("send");

        // Loopback delivery is not a speed budget. The deadline only stops a hang.
        let deadline = Instant::now() + Duration::from_secs(2);
        let (peer, bytes) = loop {
            if let Some(packet) = b.poll() {
                break packet;
            }
            assert!(
                Instant::now() < deadline,
                "loopback datagram did not arrive"
            );
            std::thread::yield_now();
        };
        assert_eq!(peer, a_peer);
        assert_eq!(
            super::super::decode(&bytes).unwrap(),
            crate::Datagram::InputAck { tick: 42 }
        );
    }

    #[test]
    fn an_unregistered_peer_is_refused() {
        let Ok(mut t) = UdpTransport::bind("127.0.0.1:0") else {
            return;
        };
        let payload = super::super::encode(&crate::Datagram::InputAck { tick: 1 });
        assert!(
            matches!(
                t.send(Peer(99), &payload),
                Err(TransportError::UnknownPeer(p)) if p.0 == 99
            ),
            "an unknown peer must be refused, not silently dropped"
        );
    }

    #[test]
    fn an_empty_collection_window_returns_without_reading() {
        let Ok(mut client) = UdpTransport::bind("127.0.0.1:0") else {
            eprintln!("skipping: no loopback");
            return;
        };
        let found = client
            .collect_until(Duration::ZERO, false)
            .expect("zero window");
        assert!(found.is_empty());
        assert!(client.poll().is_none(), "polling must be nonblocking again");
    }

    #[test]
    fn a_datagram_over_the_size_limit_is_refused() {
        // 64 KiB is the UDP payload ceiling; a larger one would be truncated by
        // the kernel rather than delivered.
        let big = vec![0u8; 70_000];
        let Ok(mut t) = UdpTransport::bind("127.0.0.1:0") else {
            return;
        };
        let peer = t.register("127.0.0.1:6112".parse().unwrap());
        assert!(matches!(
            t.send(peer, &big),
            Err(TransportError::DatagramTooLarge(_))
        ));
    }
}

/// A host: binds, advertises, and answers discovery requests.
///
/// The host owns the authoritative [`Server`][crate::Server]; this is the part
/// A LAN host: binds, advertises, and answers discovery requests.
///
/// Named distinctly from [`Host`], which is the advertisement record a
/// discoverer receives. This is the thing that binds the socket and owns the
/// game port.
#[derive(Debug)]
pub struct LanHost {
    transport: UdpTransport,
    name: String,
    map: String,
    max_players: u8,
    /// The port the game runs on, which is the one the socket is bound to.
    game_port: u16,
}

impl LanHost {
    /// Binds and starts advertising `name`.
    pub fn bind(name: &str, map: &str, max_players: u8) -> Result<Self, TransportError> {
        let transport = UdpTransport::bind(&format!("0.0.0.0:{LAN_PORT}"))?;
        let game_port = transport
            .socket
            .as_ref()
            .and_then(|s| s.local_addr().ok())
            .map_or(0, |a| a.port());
        Ok(Self {
            transport,
            name: name.to_string(),
            map: map.to_string(),
            max_players,
            game_port,
        })
    }

    /// The port a client should connect to.
    pub fn game_port(&self) -> u16 {
        self.game_port
    }

    /// Answers any pending discovery request.
    pub fn serve_discovery(&mut self) -> Result<Vec<SocketAddr>, TransportError> {
        let socket = self
            .transport
            .socket
            .as_ref()
            .ok_or_else(|| TransportError::Io(std::io::Error::other("not bound")))?;
        let mut buf = [0u8; 1024];
        let mut answered = Vec::new();
        loop {
            match socket.recv_from(&mut buf) {
                Ok((n, from)) if &buf[..n] == DISCOVERY_REQUEST => {
                    let host = Host {
                        name: self.name.clone(),
                        addr: from,
                        map: self.map.clone(),
                        players: (1, self.max_players),
                    };
                    socket.send_to(&host.encode(), from)?;
                    answered.push(from);
                }
                Ok(_) => continue,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => break,
                Err(e) => return Err(e.into()),
            }
        }
        Ok(answered)
    }

    pub fn transport(&mut self) -> &mut UdpTransport {
        &mut self.transport
    }
}

/// A client browsing for LAN games.
#[derive(Debug)]
pub struct Finder {
    transport: UdpTransport,
}

impl Finder {
    pub fn bind() -> Result<Self, TransportError> {
        Ok(Self {
            transport: UdpTransport::bind("0.0.0.0:0")?,
        })
    }

    /// Broadcasts a request and returns the hosts that answered.
    pub fn find(&mut self) -> Result<Vec<Host>, TransportError> {
        self.transport.discover()?;
        self.transport.collect_hosts()
    }
}

#[cfg(test)]
mod lan_tests {
    use super::*;

    /// `LanHost::bind` always takes the LAN port; parallel tests would steal it.
    fn lan_port() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        LOCK.lock().unwrap_or_else(|poison| poison.into_inner())
    }

    fn serve_n(host: &mut LanHost, count: usize) -> Vec<SocketAddr> {
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut answered = Vec::new();
        loop {
            answered.extend(host.serve_discovery().expect("serve"));
            if answered.len() == count {
                return answered;
            }
            assert!(
                answered.len() < count,
                "answered more discoverers than were sent"
            );
            assert!(
                Instant::now() < deadline,
                "loopback discovery request did not arrive"
            );
            std::thread::yield_now();
        }
    }

    fn recv_advertisement(socket: &UdpSocket) -> Host {
        socket.set_nonblocking(true).expect("loopback socket");
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut buf = [0u8; 2048];
        loop {
            match socket.recv_from(&mut buf) {
                Ok((n, _)) => return Host::parse(&buf[..n]).expect("advertisement"),
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        || error.kind() == std::io::ErrorKind::TimedOut =>
                {
                    assert!(
                        Instant::now() < deadline,
                        "loopback advertisement did not arrive"
                    );
                    std::thread::yield_now();
                }
                Err(error) => panic!("loopback advertisement: {error}"),
            }
        }
    }

    /// A host advertises itself and a client discovers it over loopback.
    /// Broadcast routing is a machine fact, so this does not use it or the
    /// production collection window.
    #[test]
    fn a_client_discovers_a_host() {
        let _port = lan_port();
        let Ok(mut host) = LanHost::bind("maci's game", r"Maps\BootyBay.w3m", 4) else {
            eprintln!("skipping: cannot bind LAN port");
            return;
        };
        assert_ne!(host.game_port(), 0, "a host must bind a real port");
        let Ok(mut client) = UdpTransport::bind("127.0.0.1:0") else {
            eprintln!("skipping: no loopback");
            return;
        };
        let destination = SocketAddr::from(([127, 0, 0, 1], host.game_port()));
        client
            .socket
            .as_ref()
            .unwrap()
            .send_to(DISCOVERY_REQUEST, destination)
            .expect("request");
        assert_eq!(serve_n(&mut host, 1).len(), 1);
        let found = client
            .collect_until(Duration::from_secs(2), true)
            .expect("collection");
        assert_eq!(found.len(), 1, "the queued advertisement must arrive");
        assert_eq!(found[0].name, "maci's game");
        assert_eq!(found[0].map, r"Maps\BootyBay.w3m");
        assert_eq!(found[0].players, (1, 4));
        assert_eq!(found[0].addr, destination);
    }

    /// A host answers every discoverer, because dropping everyone after the
    /// first is a silent failure in a four-player game.
    #[test]
    fn a_host_answers_every_discoverer() {
        let _port = lan_port();
        let Ok(mut host) = LanHost::bind("busy", "map", 4) else {
            eprintln!("skipping: cannot bind LAN port");
            return;
        };
        let destination = SocketAddr::from(([127, 0, 0, 1], host.game_port()));
        let mut clients = Vec::new();
        for _ in 0..3 {
            let socket = UdpSocket::bind("127.0.0.1:0").expect("loopback discoverer");
            socket
                .send_to(DISCOVERY_REQUEST, destination)
                .expect("request");
            clients.push(socket);
        }
        assert_eq!(
            serve_n(&mut host, 3).len(),
            3,
            "each discoverer is answered"
        );
        for socket in &clients {
            let advertisement = recv_advertisement(socket);
            assert_eq!(advertisement.name, "busy");
            assert_eq!(advertisement.map, "map");
            assert_eq!(advertisement.players, (1, 4));
        }
    }
}

/// The states a LAN session passes through, from the host's point of view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LobbyState {
    /// Advertising; clients may discover and request to join.
    Open,
    /// Every slot is taken.
    Full,
    /// The game is running; discovery is not answered.
    Playing,
}

/// A LAN session: host transport plus lobby state, driven by real traffic.
///
/// The point of this type is that the tick is driven by input *arriving over the
/// transport* rather than by a caller choosing when to advance. A tick that runs
/// before its input lands diverges, which is the bug the whole design avoids.
#[derive(Debug)]
pub struct LanSession {
    lobby: LobbyState,
    max_players: u8,
    peers: Vec<Peer>,
    /// Inputs received but not yet applied, in arrival order.
    inbox: Vec<(Peer, crate::Datagram)>,
}

impl LanSession {
    pub fn new(max_players: u8) -> Self {
        Self {
            lobby: LobbyState::Open,
            max_players,
            peers: Vec::new(),
            inbox: Vec::new(),
        }
    }

    pub fn state(&self) -> LobbyState {
        self.lobby
    }

    pub fn peers(&self) -> &[Peer] {
        &self.peers
    }

    /// Records a joined peer, closing the lobby when the last slot fills.
    pub fn join(&mut self, peer: Peer) -> bool {
        // A peer already in the session may re-announce; only a *new* peer is
        // refused when full or already playing. Checking the cap first would drop
        // a legitimate heartbeat from a full game.
        if self.lobby == LobbyState::Playing {
            return false;
        }
        if self.peers.contains(&peer) {
            return true;
        }
        if self.peers.len() >= self.max_players as usize {
            return false;
        }
        self.peers.push(peer);
        if self.peers.len() >= self.max_players as usize {
            self.lobby = LobbyState::Full;
        }
        true
    }

    /// Starts the game, after which no more joins are accepted.
    pub fn start(&mut self) {
        self.lobby = LobbyState::Playing;
    }

    /// Queues a decoded datagram for the next drain.
    pub fn receive(&mut self, from: Peer, datagram: crate::Datagram) {
        self.inbox.push((from, datagram));
    }

    /// Takes every queued datagram, in arrival order.
    ///
    /// Order is preserved because reordering inputs across a tick is a divergence
    /// even when the set of inputs is identical.
    pub fn drain(&mut self) -> Vec<(Peer, crate::Datagram)> {
        std::mem::take(&mut self.inbox)
    }

    /// How many datagrams are waiting.
    pub fn backlog(&self) -> usize {
        self.inbox.len()
    }
}

#[cfg(test)]
mod session_tests {
    use super::*;
    use crate::Datagram;
    use pcx_sim::World;

    #[test]
    fn a_session_closes_when_every_slot_fills() {
        let mut s = LanSession::new(2);
        assert_eq!(s.state(), LobbyState::Open);
        assert!(s.join(Peer(0)), "the first peer joins");
        assert_eq!(s.state(), LobbyState::Open, "one slot still free");
        assert!(s.join(Peer(1)), "the second peer joins");
        assert_eq!(s.state(), LobbyState::Full, "now full");
        assert!(!s.join(Peer(2)), "a full session refuses more");
        // A peer that joins twice does not consume a second slot.
        assert!(s.join(Peer(0)), "rejoining is idempotent");
        assert_eq!(s.peers().len(), 2);
    }

    #[test]
    fn a_started_session_accepts_no_more_players() {
        let mut s = LanSession::new(4);
        s.join(Peer(0));
        s.start();
        assert_eq!(s.state(), LobbyState::Playing);
        assert!(!s.join(Peer(1)), "no joining once playing");
    }

    #[test]
    fn datagrams_are_drained_in_arrival_order() {
        let mut s = LanSession::new(4);
        for t in 0..5u64 {
            s.receive(Peer(0), Datagram::InputAck { tick: t });
        }
        assert_eq!(s.backlog(), 5);
        let drained = s.drain();
        assert_eq!(s.backlog(), 0, "draining empties the inbox");
        for (i, (_, d)) in drained.iter().enumerate() {
            assert_eq!(d, &Datagram::InputAck { tick: i as u64 }, "order changed");
        }
    }

    /// The loop the game actually runs: poll the transport, feed the server,
    /// advance only when that tick's input is present.
    #[test]
    fn a_tick_runs_only_once_its_input_has_arrived() {
        let mut world = World::empty();
        world.spawn(0, 0, 0);
        let mut server = crate::Server::new(world, 5);
        let mut session = LanSession::new(2);

        // Nothing sent yet, so the server must not advance.
        assert!(!server.advance(), "a tick with no input must not run");

        // Input arrives through the transport layer, so it is pushed the way the
        // real loop pushes it.
        session.receive(Peer(0), Datagram::InputAck { tick: 0 });
        assert_eq!(session.backlog(), 1);
        for (_, datagram) in session.drain() {
            let Datagram::InputAck { tick } = datagram else {
                continue;
            };
            let input = crate::Input {
                tick,
                commands: vec![pcx_sim::PlayerInput {
                    player: 0,
                    commands: vec![pcx_sim::Command::Nop],
                }],
            };
            server.accept(input).expect("accept");
        }
        assert!(server.advance(), "input arrived, so the tick runs");
        assert_eq!(server.world().tick, 1);
    }
}

#[cfg(test)]
mod receive_span_tests {
    use super::*;
    use crate::{Datagram, decode, decode_world_payload, encode, encode_world_payload};
    use pcx_sim::World;

    #[test]
    fn a_snapshot_larger_than_the_old_receive_span_arrives_complete() {
        let mut sender = UdpTransport::bind("127.0.0.1:0").expect("loopback sender must bind");
        let mut receiver = UdpTransport::bind("127.0.0.1:0").expect("loopback receiver must bind");
        let sender_addr = sender.socket.as_ref().unwrap().local_addr().unwrap();
        let receiver_addr = receiver.socket.as_ref().unwrap().local_addr().unwrap();
        let destination = sender.register(receiver_addr);
        let source = receiver.register(sender_addr);
        let mut world = World::empty();
        world.tick = 91;
        while encode(&Datagram::Snapshot {
            tick: world.tick,
            payload: encode_world_payload(&world),
        })
        .len()
            <= 2048
        {
            world.spawn(0, world.units.len() as i32, 0);
        }
        let frame = encode(&Datagram::Snapshot {
            tick: world.tick,
            payload: encode_world_payload(&world),
        });
        sender
            .send(destination, &frame)
            .expect("snapshot is within existing send ceiling");
        // Bounded liveness wait, not an elapsed-time performance/fidelity gate.
        let deadline = Instant::now() + Duration::from_secs(1);
        let (peer, bytes) = loop {
            if let Some(received) = receiver.poll() {
                break received;
            }
            assert!(
                Instant::now() < deadline,
                "loopback snapshot did not arrive"
            );
            std::thread::yield_now();
        };
        assert_eq!(peer, source);
        assert_eq!(bytes.len(), frame.len(), "UDP frame was truncated");
        assert_eq!(bytes, frame, "UDP frame was truncated before decoding");
        let Datagram::Snapshot { tick, payload } = decode(&bytes).unwrap() else {
            panic!("snapshot kind changed");
        };
        assert_eq!(decode_world_payload(tick, &payload).unwrap(), world);
    }
}

#[cfg(test)]
mod discovery_loopback {
    use super::*;
    #[test]
    fn ephemeral_host_response_reaches_collector_and_polling_resumes() {
        let mut client =
            UdpTransport::bind("127.0.0.1:0").expect("ephemeral loopback client must bind");
        let transport =
            UdpTransport::bind("127.0.0.1:0").expect("ephemeral loopback host must bind");
        let client_addr = client.socket.as_ref().unwrap().local_addr().unwrap();
        let host_addr = transport.socket.as_ref().unwrap().local_addr().unwrap();
        let mut host = LanHost {
            transport,
            name: "synthetic project host".into(),
            map: "synthetic map".into(),
            max_players: 5,
            game_port: host_addr.port(),
        };
        client
            .socket
            .as_ref()
            .unwrap()
            .send_to(DISCOVERY_REQUEST, host_addr)
            .unwrap();
        let responder = std::thread::spawn(move || {
            // Hang guard only. The reply is returned as soon as it is queued.
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                let answered = host.serve_discovery().unwrap();
                if !answered.is_empty() {
                    assert_eq!(answered, [client_addr]);
                    break;
                }
                assert!(Instant::now() < deadline, "loopback request did not arrive");
                std::thread::yield_now();
            }
            host
        });
        let found = client
            .collect_until(Duration::from_secs(2), true)
            .expect("collection must preserve socket failures");
        let mut host = responder.join().unwrap();
        assert_eq!(
            found,
            [Host {
                name: "synthetic project host".into(),
                addr: host_addr,
                map: "synthetic map".into(),
                players: (1, 5)
            }]
        );
        let destination = host.transport().register(client_addr);
        let source = client.register(host_addr);
        host.transport()
            .send(destination, b"post-collection polling")
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        let packet = loop {
            if let Some(packet) = client.poll() {
                break packet;
            }
            assert!(
                Instant::now() < deadline,
                "post-collection datagram did not arrive"
            );
            std::thread::yield_now();
        };
        assert_eq!(packet, (source, b"post-collection polling".to_vec()));
        assert_eq!(client.address(source), Some(host_addr));
    }
}

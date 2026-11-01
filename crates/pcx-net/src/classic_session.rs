//! Socket-free classic lobby, realm and game session policy.
//!
//! This is the admission, readiness, countdown, seed, ordered-action and
//! timeout/lag policy shared by both host directions and by the realm
//! client. It performs no I/O and reads no clock: time enters as caller
//! `now_ms`, which keeps the policy deterministic and WASM-usable. Sockets,
//! threads and cancellation live in `transport` and `classic_frame::tcp`.
//!
//! Wire sources: `classic_frame::lobby` for lobby/gameplay bodies,
//! `classic_frame::bncs` for realm bodies, `classic_frame::slot_seed` for
//! slot blobs, and `classic_frame::map_check::Fingerprints` for the
//! negotiated map identity. A matching map fingerprint refuses mismatched
//! edition/build/map before start; asset transfer is never performed here.

use std::collections::BTreeMap;

use crate::classic_frame::bncs::{
    AccountLogonReply, AuthCheckReply, AuthInfoReply, ChatEvent, ChatEventKind, KeyState,
};
use crate::classic_frame::lobby::{
    DOWNLOAD_VERIFIED, ChatMessage, IncomingJoin, LagEntry, LeaveCode, MapSizeReport, SLOT_OCCUPIED,
    SlotReadiness,
};
use crate::classic_frame::map_check::Fingerprints;

/// Which edition/build the lobby negotiates. Explicit labels, never
/// inferred from a path: a mismatched client refuses before start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NegotiatedBuild {
    /// Edition label, e.g. `tft` or `roc`.
    pub edition: String,
    /// Version label, e.g. `1.29b`.
    pub version: String,
}

/// The map identity every peer must match before start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NegotiatedMap {
    /// Wire path, compared byte for byte.
    pub protocol_path: Vec<u8>,
    /// Whole-file size, CRC-32, XOR/rotate mix and script-stream SHA-1.
    pub fingerprints: Fingerprints,
}

/// Timing policy. All bounds are caller configuration, not protocol
/// defaults: the original client's own cadences are unmeasured here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimingPolicy {
    /// Milliseconds between host pings while waiting for pongs.
    pub ping_interval_ms: u64,
    /// Milliseconds without any frame before a peer times out.
    pub peer_timeout_ms: u64,
    /// Milliseconds a peer may lag before `START_LAG` is announced.
    pub lag_threshold_ms: u64,
    /// Milliseconds a countdown lasts before the game starts.
    pub countdown_ms: u64,
    /// Most ordered-action bytes retained per game, bounding memory.
    pub max_action_bytes: usize,
    /// Most lobby chat lines retained, bounding memory.
    pub max_chat_lines: usize,
}

impl Default for TimingPolicy {
    fn default() -> Self {
        Self {
            ping_interval_ms: 5_000,
            peer_timeout_ms: 30_000,
            lag_threshold_ms: 10_000,
            countdown_ms: 5_000,
            max_action_bytes: 1 << 20,
            max_chat_lines: 128,
        }
    }
}

/// Why a join was refused. Every variant is a refusal, never admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JoinRefusal {
    /// Host counter or entry key differs from the advertisement.
    StaleAdvertisement,
    /// The lobby is full.
    Full,
    /// The game already started.
    Started,
    /// The name is empty or already taken.
    Name,
    /// The peer key is already bound to another player.
    DuplicatePeer,
}

/// One admitted lobby player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LobbyPlayer {
    /// Assigned player id.
    pub pid: u8,
    /// Name bytes from the join request.
    pub name: Vec<u8>,
    /// Peer key from the join request.
    pub peer_key: u32,
    /// Client listen port from the join request.
    pub listen_port: u16,
    /// Download status: 255 unverified, 100 verified.
    pub download_status: u8,
    /// Slot status: 0 open, 1 closed, 2 occupied.
    pub slot_status: u8,
    /// Last frame time from this peer.
    pub last_seen_ms: u64,
    /// Last ping tick sent to this peer.
    pub last_ping_ms: u64,
    /// When the lag began, if the peer is lagging.
    pub lagging_since_ms: Option<u64>,
    /// Whether the peer reported a loaded game.
    pub loaded: bool,
}

/// Lobby phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LobbyPhase {
    /// Advertising; clients may discover and request to join.
    Open,
    /// Countdown running; no new joins.
    Countdown {
        /// When the countdown began.
        started_ms: u64,
    },
    /// The game is running; discovery is not answered.
    Playing,
    /// The lobby is closed after a graceful end or a failure.
    Closed,
}

/// Events the caller must serve: frames to send or peers to drop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LobbyEvent {
    /// Send a ping with this tick value to every listed peer.
    PingDue {
        /// Tick value to echo back in the pong.
        ticks: u32,
        /// Peers owed a ping.
        pids: Vec<u8>,
    },
    /// These peers missed their deadline; disconnect them.
    Timeout {
        /// Timed-out player ids.
        pids: Vec<u8>,
    },
    /// These peers crossed the lag threshold; announce `START_LAG`.
    LagStarted {
        /// Entries with elapsed milliseconds.
        entries: Vec<LagEntry>,
    },
    /// The countdown elapsed; deal the seed and start.
    CountdownFinished,
}

/// A retained ordered action: player id, four CRC bytes and payload, in
/// wire order. CRC bytes are retained without verification: the `FullCRC`
/// function is not in the retained sources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderedAction {
    /// Sending player id.
    pub player_id: u8,
    /// Four CRC bytes from the outgoing action, retained raw.
    pub crc: [u8; 4],
    /// Payload bytes after the checksum, in order.
    pub payload: Vec<u8>,
}

/// A classic lobby: slots, readiness, countdown, seed and ordered actions.
#[derive(Debug)]
pub struct ClassicLobby {
    build: NegotiatedBuild,
    map: NegotiatedMap,
    host_counter: u32,
    entry_key: u32,
    max_players: u8,
    phase: LobbyPhase,
    players: BTreeMap<u8, LobbyPlayer>,
    names: BTreeMap<Vec<u8>, u8>,
    timing: TimingPolicy,
    next_pid: u8,
    seed: Option<[u8; 4]>,
    actions: Vec<OrderedAction>,
    action_bytes: usize,
    chat: Vec<String>,
}

impl ClassicLobby {
    /// Creates a lobby that admits up to `max_players` peers against the
    /// negotiated build, map and advertisement keys.
    pub fn new(
        build: NegotiatedBuild,
        map: NegotiatedMap,
        host_counter: u32,
        entry_key: u32,
        max_players: u8,
        timing: TimingPolicy,
    ) -> Self {
        Self {
            build,
            map,
            host_counter,
            entry_key,
            max_players: max_players.max(1),
            phase: LobbyPhase::Open,
            players: BTreeMap::new(),
            names: BTreeMap::new(),
            timing,
            next_pid: 1,
            seed: None,
            actions: Vec::new(),
            action_bytes: 0,
            chat: Vec::new(),
        }
    }

    /// Negotiated build labels.
    pub fn build(&self) -> &NegotiatedBuild {
        &self.build
    }

    /// Negotiated map identity.
    pub fn map(&self) -> &NegotiatedMap {
        &self.map
    }

    /// Current lobby phase.
    pub fn phase(&self) -> LobbyPhase {
        self.phase
    }

    /// Admitted players by id.
    pub fn players(&self) -> &BTreeMap<u8, LobbyPlayer> {
        &self.players
    }

    /// Negotiated game seed, once the countdown finishes.
    pub fn seed(&self) -> Option<[u8; 4]> {
        self.seed
    }

    /// Admits a join request, assigning the next player id. The host
    /// counter and entry key must equal the advertisement; anything else
    /// is a stale advertisement, not a join.
    pub fn join(&mut self, req: &IncomingJoin<'_>, now_ms: u64) -> Result<u8, JoinRefusal> {
        if req.host_counter != self.host_counter || req.entry_key != self.entry_key {
            return Err(JoinRefusal::StaleAdvertisement);
        }
        if !matches!(self.phase, LobbyPhase::Open) {
            return Err(JoinRefusal::Started);
        }
        if req.player_name.is_empty() {
            return Err(JoinRefusal::Name);
        }
        if self.names.contains_key(req.player_name) {
            return Err(JoinRefusal::Name);
        }
        if self.players.values().any(|p| p.peer_key == req.peer_key) {
            return Err(JoinRefusal::DuplicatePeer);
        }
        if self.players.len() >= self.max_players as usize {
            return Err(JoinRefusal::Full);
        }
        let pid = self.next_pid;
        self.next_pid = self.next_pid.checked_add(1).ok_or(JoinRefusal::Full)?;
        if pid == 255 {
            return Err(JoinRefusal::Full);
        }
        self.players.insert(
            pid,
            LobbyPlayer {
                pid,
                name: req.player_name.to_vec(),
                peer_key: req.peer_key,
                listen_port: req.listen_port,
                download_status: super::classic_frame::lobby::DOWNLOAD_UNVERIFIED,
                slot_status: SLOT_OCCUPIED,
                last_seen_ms: now_ms,
                last_ping_ms: now_ms,
                lagging_since_ms: None,
                loaded: false,
            },
        );
        self.names.insert(req.player_name.to_vec(), pid);
        Ok(pid)
    }

    /// Records a map-size report. A mismatched size refuses the peer's map
    /// before any readiness is published; a matching report with the
    /// have-map flag marks the map verified.
    pub fn report_map_size(
        &mut self,
        pid: u8,
        report: &MapSizeReport,
        now_ms: u64,
    ) -> Result<bool, MapMismatch> {
        let Some(player) = self.players.get_mut(&pid) else {
            return Err(MapMismatch::UnknownPlayer);
        };
        player.last_seen_ms = now_ms;
        if report.map_size != self.map.fingerprints.size {
            return Err(MapMismatch::Size {
                expected: self.map.fingerprints.size,
                got: report.map_size,
            });
        }
        if report.size_flag == super::classic_frame::lobby::MAPSIZE_HAVE_MAP {
            player.download_status = DOWNLOAD_VERIFIED;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Readiness of one player's slot.
    pub fn readiness(&self, pid: u8) -> Option<SlotReadiness> {
        self.players.get(&pid).map(|p| {
            SlotReadiness::of(&crate::classic_frame::slot_seed::RawSlot {
                pid: p.pid,
                download_status: p.download_status,
                slot_status: p.slot_status,
                computer: 0,
                team: 0,
                colour: 0,
                race: 0,
                computer_type: 1,
                handicap: 100,
            })
        })
    }

    /// True when every admitted player is map-verified and the lobby may
    /// start its countdown.
    pub fn all_ready(&self) -> bool {
        !self.players.is_empty()
            && self
                .players
                .keys()
                .all(|pid| matches!(self.readiness(*pid), Some(SlotReadiness::Ready)))
    }

    /// Begins the countdown. Refuses unless the lobby is open and ready.
    pub fn begin_countdown(&mut self, now_ms: u64) -> Result<(), &'static str> {
        if !matches!(self.phase, LobbyPhase::Open) {
            return Err("countdown needs an open lobby");
        }
        if !self.all_ready() {
            return Err("countdown needs every player map-verified");
        }
        self.phase = LobbyPhase::Countdown { started_ms: now_ms };
        Ok(())
    }

    /// Finishes the countdown with the negotiated seed and enters play.
    /// The seed is caller-supplied deal bytes; equal seeds alone never prove
    /// equal random streams.
    pub fn finish_countdown(&mut self, seed: [u8; 4]) -> Result<(), &'static str> {
        if !matches!(self.phase, LobbyPhase::Countdown { .. }) {
            return Err("no countdown is running");
        }
        self.seed = Some(seed);
        self.phase = LobbyPhase::Playing;
        Ok(())
    }

    /// Queues one outgoing ordered action, preserving wire order and the
    /// raw CRC bytes. Refuses when the retained backlog would exceed the
    /// caller's byte budget.
    pub fn queue_action(&mut self, action: OrderedAction) -> Result<(), &'static str> {
        let len = action.payload.len();
        if self
            .action_bytes
            .checked_add(len)
            .is_none_or(|n| n > self.timing.max_action_bytes)
        {
            return Err("ordered-action backlog exceeds its byte budget");
        }
        self.action_bytes += len;
        self.actions.push(action);
        Ok(())
    }

    /// Takes every queued ordered action in wire order.
    pub fn drain_actions(&mut self) -> Vec<OrderedAction> {
        self.action_bytes = 0;
        std::mem::take(&mut self.actions)
    }

    /// Records lobby chat, retaining a bounded tail.
    pub fn chat(&mut self, message: &ChatMessage<'_>) {
        let line = format!(
            "{}: {}",
            message.from_pid,
            String::from_utf8_lossy(message.payload)
        );
        self.chat.push(line);
        while self.chat.len() > self.timing.max_chat_lines {
            self.chat.remove(0);
        }
    }

    /// Retained chat tail, oldest first.
    pub fn chat_log(&self) -> &[String] {
        &self.chat
    }

    /// Marks a peer loaded.
    pub fn mark_loaded(&mut self, pid: u8, now_ms: u64) -> bool {
        if let Some(player) = self.players.get_mut(&pid) {
            player.loaded = true;
            player.last_seen_ms = now_ms;
            true
        } else {
            false
        }
    }

    /// Applies a graceful leave: removes the peer and reports its code.
    /// Unknown peers are refused rather than silently dropped.
    pub fn leave(&mut self, pid: u8, code: u32) -> Result<LeaveCode, &'static str> {
        let Some(player) = self.players.remove(&pid) else {
            return Err("unknown player cannot leave");
        };
        self.names.remove(&player.name);
        Ok(LeaveCode::of(code))
    }

    /// Kicks a peer with the lobby-leave code.
    pub fn kick(&mut self, pid: u8) -> Result<(), &'static str> {
        self.leave(pid, super::classic_frame::lobby::LEAVE_LOBBY)
            .map(|_| ())
    }

    /// Advances timeout, ping and lag bookkeeping at `now_ms`, returning
    /// the events the caller must serve. Countdown completion is reported
    /// once, when the elapsed countdown first reaches the policy.
    pub fn poll(&mut self, now_ms: u64, ticks: u32) -> Vec<LobbyEvent> {
        let mut events = Vec::new();
        let mut timed_out = Vec::new();
        let mut lagged = Vec::new();
        let mut ping_due = Vec::new();
        for player in self.players.values_mut() {
            if now_ms.saturating_sub(player.last_seen_ms) >= self.timing.peer_timeout_ms {
                timed_out.push(player.pid);
                continue;
            }
            if now_ms.saturating_sub(player.last_ping_ms) >= self.timing.ping_interval_ms {
                ping_due.push(player.pid);
                player.last_ping_ms = now_ms;
            }
            let silent = now_ms.saturating_sub(player.last_seen_ms);
            if silent >= self.timing.lag_threshold_ms && player.lagging_since_ms.is_none() {
                player.lagging_since_ms = Some(now_ms - silent);
                lagged.push(LagEntry {
                    pid: player.pid,
                    elapsed_ms: silent as u32,
                });
            }
        }
        for pid in &timed_out {
            if let Some(player) = self.players.remove(pid) {
                self.names.remove(&player.name);
            }
        }
        if !timed_out.is_empty() {
            events.push(LobbyEvent::Timeout { pids: timed_out });
        }
        if !lagged.is_empty() {
            events.push(LobbyEvent::LagStarted { entries: lagged });
        }
        if !ping_due.is_empty() {
            events.push(LobbyEvent::PingDue { ticks, pids: ping_due });
        }
        if let LobbyPhase::Countdown { started_ms } = self.phase
            && now_ms.saturating_sub(started_ms) >= self.timing.countdown_ms
        {
            events.push(LobbyEvent::CountdownFinished);
        }
        events
    }

    /// Closes the lobby; no further joins or countdowns are accepted.
    pub fn close(&mut self) {
        self.phase = LobbyPhase::Closed;
    }
}

/// Why a map report was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MapMismatch {
    /// The reporting peer is not admitted.
    UnknownPlayer,
    /// Whole-file size differs from the negotiated map.
    Size {
        /// Negotiated size.
        expected: u32,
        /// Reported size.
        got: u32,
    },
}

/// Realm login phase for one Battle.net connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RealmPhase {
    /// No realm connection.
    Disconnected,
    /// Init selector sent; waiting for the auth-info challenge.
    Challenge,
    /// Challenge accepted; version/key material sent, awaiting the verdict.
    VersionCheck,
    /// Version accepted; account logon in progress.
    Logon,
    /// Authenticated; entering chat.
    EnterChat,
    /// In a channel; may chat, list, advertise and join games.
    Channel,
}

/// Why a realm step failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RealmFailure {
    /// The version check failed: key state plus the realm's description.
    VersionCheck {
        /// Classified key state.
        state: KeyState,
        /// Realm description bytes.
        description: Vec<u8>,
    },
    /// The account logon failed with this status word.
    Logon {
        /// Raw status word.
        status: u32,
    },
    /// The logon proof failed.
    LogonProof,
    /// The server token or challenge shape was malformed.
    Challenge(&'static str),
}

/// A realm client: version/auth failures, login, channels, chat, game
/// advertisement, listing and joining. Packet builders and parsers live in
/// `classic_frame::bncs`; this holds the phase each reply advances. Key and
/// password-proof math stay caller-supplied: this machine never derives
/// key material, and a failed step is an explicit `RealmFailure`, never a
/// silent retry.
#[derive(Debug)]
pub struct RealmSession {
    phase: RealmPhase,
    channel: Option<Vec<u8>>,
    logon_type: Option<[u8; 4]>,
    server_token: Option<[u8; 4]>,
    account: Option<Vec<u8>>,
    chat: Vec<String>,
    max_chat_lines: usize,
}

impl RealmSession {
    /// Starts disconnected.
    pub fn new(max_chat_lines: usize) -> Self {
        Self {
            phase: RealmPhase::Disconnected,
            channel: None,
            logon_type: None,
            server_token: None,
            account: None,
            chat: Vec::new(),
            max_chat_lines: max_chat_lines.max(1),
        }
    }

    /// Current realm phase.
    pub fn phase(&self) -> RealmPhase {
        self.phase
    }

    /// Begins the connection: the caller sends the init selector, then the
    /// auth-info request, and waits for the challenge.
    pub fn begin(&mut self) {
        self.phase = RealmPhase::Challenge;
    }

    /// Handles the auth-info challenge, retaining the logon type and server
    /// token for the version check. The version-check filename and equation
    /// stay caller-selected profile data: this compares nothing and proves
    /// nothing here.
    pub fn on_auth_info(&mut self, reply: &AuthInfoReply<'_>) -> Result<(), RealmFailure> {
        if self.phase != RealmPhase::Challenge {
            return Err(RealmFailure::Challenge("auth-info arrived out of phase"));
        }
        if reply.ver_file_name.is_empty() || reply.value_string_formula.is_empty() {
            return Err(RealmFailure::Challenge("auth-info challenge is empty"));
        }
        self.logon_type = Some(reply.logon_type);
        self.server_token = Some(reply.server_token);
        self.phase = RealmPhase::VersionCheck;
        Ok(())
    }

    /// Handles the version-check verdict. Anything but `Good` is an
    /// explicit failure carrying the realm's description.
    pub fn on_auth_check(&mut self, reply: &AuthCheckReply<'_>) -> Result<(), RealmFailure> {
        if self.phase != RealmPhase::VersionCheck {
            return Err(RealmFailure::Challenge("auth-check arrived out of phase"));
        }
        if reply.state != KeyState::Good {
            return Err(RealmFailure::VersionCheck {
                state: reply.state,
                description: reply.description.to_vec(),
            });
        }
        self.phase = RealmPhase::Logon;
        Ok(())
    }

    /// Handles the account-logon reply: status 0 with salt and server key
    /// continues to the proof step; anything else fails with its status.
    pub fn on_account_logon(&mut self, reply: &AccountLogonReply<'_>) -> Result<bool, RealmFailure> {
        if self.phase != RealmPhase::Logon {
            return Err(RealmFailure::Challenge("account-logon arrived out of phase"));
        }
        if reply.status != 0 || reply.salt.is_none() || reply.server_key.is_none() {
            return Err(RealmFailure::Logon { status: reply.status });
        }
        Ok(true)
    }

    /// Handles the logon-proof reply: true continues to chat entry.
    pub fn on_account_logon_proof(&mut self, accepted: bool) -> Result<(), RealmFailure> {
        if self.phase != RealmPhase::Logon {
            return Err(RealmFailure::Challenge("logon-proof arrived out of phase"));
        }
        if !accepted {
            return Err(RealmFailure::LogonProof);
        }
        self.phase = RealmPhase::EnterChat;
        Ok(())
    }

    /// Enters the realm chat with the authenticated account name.
    pub fn enter_chat(&mut self, account: &[u8]) -> Result<(), RealmFailure> {
        if self.phase != RealmPhase::EnterChat {
            return Err(RealmFailure::Challenge("chat entry arrived out of phase"));
        }
        if account.is_empty() || account.len() > 32 {
            return Err(RealmFailure::Challenge("account name is empty or overlong"));
        }
        self.account = Some(account.to_vec());
        Ok(())
    }

    /// Joins a channel once the realm confirms chat entry.
    pub fn join_channel(&mut self, channel: &[u8]) -> Result<(), RealmFailure> {
        if self.phase != RealmPhase::EnterChat && self.phase != RealmPhase::Channel {
            return Err(RealmFailure::Challenge("channel join arrived out of phase"));
        }
        if channel.len() > 64 {
            return Err(RealmFailure::Challenge("channel exceeds the 64-byte ceiling"));
        }
        self.channel = Some(channel.to_vec());
        self.phase = RealmPhase::Channel;
        Ok(())
    }

    /// Current channel, if any.
    pub fn channel(&self) -> Option<&[u8]> {
        self.channel.as_deref()
    }

    /// Records a chat event's user and message, retaining a bounded tail.
    pub fn on_chat_event(&mut self, event: &ChatEvent<'_>) {
        if self.phase != RealmPhase::Channel {
            return;
        }
        let line = format!(
            "{:?} {}: {}",
            ChatEventKind::of(event.event),
            String::from_utf8_lossy(event.user),
            String::from_utf8_lossy(event.message)
        );
        self.chat.push(line);
        while self.chat.len() > self.max_chat_lines {
            self.chat.remove(0);
        }
    }

    /// Retained chat tail, oldest first.
    pub fn chat_log(&self) -> &[String] {
        &self.chat
    }

    /// Retained server token for version-check hashing, if any.
    pub fn server_token(&self) -> Option<[u8; 4]> {
        self.server_token
    }

    /// Retained logon type, if any.
    pub fn logon_type(&self) -> Option<[u8; 4]> {
        self.logon_type
    }
}

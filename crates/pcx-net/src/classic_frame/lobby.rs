//! W3GS lobby and gameplay control messages.
//!
//! Pinned source: GHost++ `ghost/gameprotocol.h` and `ghost/gameprotocol.cpp`
//! at commit `6ba90f426e96f80fe013a468d1e07151a510adbe`. Slot value
//! semantics come from `ghost/gameslot.h` at the same commit. Every layout
//! below follows those files' `SEND_*`/`RECEIVE_*` bodies, including their
//! unspecified/reserved words, which are preserved raw and never assigned a
//! meaning here.
//!
//! What this module does not do: verify checksums (`FullCRC` is not in the
//! retained sources and is never reimplemented here), decode game orders,
//! download maps (see [`MapPartHeader`]), or run a socket. Builders return
//! frames; parsers borrow them. Admission, readiness and countdown policy
//! live in `crate::classic_session`.

use super::{Frame, FrameError, OwnedFrame, W3GS_FAMILY, encode};
use crate::classic_frame::slot_seed::RawSlot;

/// `W3GS_PING_FROM_HOST`.
pub const PING_FROM_HOST_ID: u8 = 1;
/// `W3GS_PLAYERINFO`.
pub const PLAYERINFO_ID: u8 = 6;
/// `W3GS_PLAYERLEAVE_OTHERS`.
pub const PLAYERLEAVE_OTHERS_ID: u8 = 7;
/// `W3GS_GAMELOADED_OTHERS`.
pub const GAMELOADED_OTHERS_ID: u8 = 8;
/// `W3GS_COUNTDOWN_START`.
pub const COUNTDOWN_START_ID: u8 = 10;
/// `W3GS_COUNTDOWN_END`.
pub const COUNTDOWN_END_ID: u8 = 11;
/// `W3GS_CHAT_FROM_HOST`.
pub const CHAT_FROM_HOST_ID: u8 = 15;
/// `W3GS_START_LAG`.
pub const START_LAG_ID: u8 = 16;
/// `W3GS_STOP_LAG`.
pub const STOP_LAG_ID: u8 = 17;
/// `W3GS_HOST_KICK_PLAYER`. Body layout is not in the retained sources.
pub const HOST_KICK_PLAYER_ID: u8 = 28;
/// `W3GS_LEAVEGAME`.
pub const LEAVEGAME_ID: u8 = 33;
/// `W3GS_GAMELOADED_SELF`.
pub const GAMELOADED_SELF_ID: u8 = 35;
/// `W3GS_OUTGOING_KEEPALIVE`.
pub const OUTGOING_KEEPALIVE_ID: u8 = 39;
/// `W3GS_CHAT_TO_HOST`.
pub const CHAT_TO_HOST_ID: u8 = 40;
/// `W3GS_DROPREQ`. Body layout is not in the retained sources.
pub const DROPREQ_ID: u8 = 41;
/// `W3GS_CREATEGAME` (UDP/LAN announce).
pub const CREATEGAME_ID: u8 = 49;
/// `W3GS_REFRESHGAME` (UDP/LAN announce).
pub const REFRESHGAME_ID: u8 = 50;
/// `W3GS_DECREATEGAME` (UDP/LAN announce).
pub const DECREATEGAME_ID: u8 = 51;
/// `W3GS_CHAT_OTHERS`. Relay framing is not in the retained sources.
pub const CHAT_OTHERS_ID: u8 = 52;
/// `W3GS_PING_FROM_OTHERS`. Relay framing is not in the retained sources.
pub const PING_FROM_OTHERS_ID: u8 = 53;
/// `W3GS_PONG_TO_OTHERS`. Relay framing is not in the retained sources.
pub const PONG_TO_OTHERS_ID: u8 = 54;
/// `W3GS_STARTDOWNLOAD`.
pub const STARTDOWNLOAD_ID: u8 = 63;
/// `W3GS_MAPPART`.
pub const MAPPART_ID: u8 = 67;
/// `W3GS_MAPPARTOK`.
pub const MAPPARTOK_ID: u8 = 68;
/// `W3GS_MAPPARTNOTOK`. Marked "just a guess" in the pinned source.
pub const MAPPARTNOTOK_ID: u8 = 69;
/// `W3GS_PONG_TO_HOST`.
pub const PONG_TO_HOST_ID: u8 = 70;

/// `PLAYERLEAVE_DISCONNECT`.
pub const LEAVE_DISCONNECT: u32 = 1;
/// `PLAYERLEAVE_LOST`.
pub const LEAVE_LOST: u32 = 7;
/// `PLAYERLEAVE_LOSTBUILDINGS`.
pub const LEAVE_LOSTBUILDINGS: u32 = 8;
/// `PLAYERLEAVE_WON`.
pub const LEAVE_WON: u32 = 9;
/// `PLAYERLEAVE_DRAW`.
pub const LEAVE_DRAW: u32 = 10;
/// `PLAYERLEAVE_OBSERVER`.
pub const LEAVE_OBSERVER: u32 = 11;
/// `PLAYERLEAVE_LOBBY`.
pub const LEAVE_LOBBY: u32 = 13;
/// `PLAYERLEAVE_GPROXY`.
pub const LEAVE_GPROXY: u32 = 100;

/// `SLOTSTATUS_OPEN`.
pub const SLOT_OPEN: u8 = 0;
/// `SLOTSTATUS_CLOSED`.
pub const SLOT_CLOSED: u8 = 1;
/// `SLOTSTATUS_OCCUPIED`.
pub const SLOT_OCCUPIED: u8 = 2;

/// `SLOTRACE_HUMAN`.
pub const RACE_HUMAN: u8 = 1;
/// `SLOTRACE_ORC`.
pub const RACE_ORC: u8 = 2;
/// `SLOTRACE_NIGHTELF`.
pub const RACE_NIGHTELF: u8 = 4;
/// `SLOTRACE_UNDEAD`.
pub const RACE_UNDEAD: u8 = 8;
/// `SLOTRACE_RANDOM`.
pub const RACE_RANDOM: u8 = 32;
/// `SLOTRACE_SELECTABLE`.
pub const RACE_SELECTABLE: u8 = 64;

/// `SLOTCOMP_EASY`.
pub const COMP_EASY: u8 = 0;
/// `SLOTCOMP_NORMAL`.
pub const COMP_NORMAL: u8 = 1;
/// `SLOTCOMP_HARD`.
pub const COMP_HARD: u8 = 2;

/// `CHAT_TO_HOST` flag: a plain message.
pub const CHAT_FLAG_MESSAGE: u8 = 16;
/// `CHAT_TO_HOST` flag: a team-change request.
pub const CHAT_FLAG_TEAM: u8 = 17;
/// `CHAT_TO_HOST` flag: a colour-change request.
pub const CHAT_FLAG_COLOUR: u8 = 18;
/// `CHAT_TO_HOST` flag: a race-change request.
pub const CHAT_FLAG_RACE: u8 = 19;
/// `CHAT_TO_HOST` flag: a handicap-change request.
pub const CHAT_FLAG_HANDICAP: u8 = 20;
/// `CHAT_TO_HOST` flag: a message with extra flags.
pub const CHAT_FLAG_EXTRA: u8 = 32;

/// `SEND_SID_STARTADVEX3` state: public game.
pub const ADVEX_STATE_PUBLIC: u32 = 16;
/// `SEND_SID_STARTADVEX3` state: private game.
pub const ADVEX_STATE_PRIVATE: u32 = 17;

/// Download status an original 1.29.2 client reports once its map is
/// verified: 255 while unverified, 100 after the host accepts MAPSIZE.
/// Observed in the original-to-original lobby baseline; see
/// `docs/re-notes/classic-network-interoperability.md`.
pub const DOWNLOAD_VERIFIED: u8 = 100;
/// Download status before the peer reports a verified map.
pub const DOWNLOAD_UNVERIFIED: u8 = 255;

/// Readiness of one lobby slot, from its raw record bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotReadiness {
    /// `SLOTSTATUS_OPEN`: any client may claim it.
    Open,
    /// `SLOTSTATUS_CLOSED`: not claimable.
    Closed,
    /// Occupied and map-verified: may start.
    Ready,
    /// Occupied but the map is not verified yet.
    WaitingForMap,
    /// A status byte outside the pinned `SLOTSTATUS_*` set.
    Unknown(u8),
}

impl SlotReadiness {
    /// Classifies a raw slot record. Computer slots use the same map rule:
    /// the host publishes their records, so an unverified computer slot is
    /// still waiting.
    pub fn of(slot: &RawSlot) -> Self {
        match slot.slot_status {
            SLOT_OPEN => SlotReadiness::Open,
            SLOT_CLOSED => SlotReadiness::Closed,
            SLOT_OCCUPIED => {
                if slot.download_status == DOWNLOAD_VERIFIED {
                    SlotReadiness::Ready
                } else {
                    SlotReadiness::WaitingForMap
                }
            }
            other => SlotReadiness::Unknown(other),
        }
    }
}

/// Why a lobby/gameplay body was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LobbyError {
    /// Header family is not [`W3GS_FAMILY`].
    WrongFamily(u8),
    /// Header id is not the one this call accepts.
    WrongId(u8),
    /// A required field is short or malformed.
    Malformed(&'static str),
    /// A caller-supplied field is out of range.
    Fields(&'static str),
    /// Frame allocation or length failure.
    Frame(FrameError),
}

impl std::fmt::Display for LobbyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongFamily(v) => write!(f, "W3GS lobby family {v} is not W3GS"),
            Self::WrongId(v) => write!(f, "W3GS lobby id {v} is not accepted here"),
            Self::Malformed(v) => write!(f, "W3GS lobby: {v}"),
            Self::Fields(v) => write!(f, "W3GS lobby: {v}"),
            Self::Frame(v) => write!(f, "{v}"),
        }
    }
}

impl std::error::Error for LobbyError {}

impl From<FrameError> for LobbyError {
    fn from(v: FrameError) -> Self {
        Self::Frame(v)
    }
}

fn check(family: u8, id: u8, want: u8, body: &[u8]) -> Result<&[u8], LobbyError> {
    if family != W3GS_FAMILY {
        return Err(LobbyError::WrongFamily(family));
    }
    if id != want {
        return Err(LobbyError::WrongId(id));
    }
    Ok(body)
}

fn cstring(body: &[u8], what: &'static str) -> Result<(&[u8], &[u8]), LobbyError> {
    let end = body
        .iter()
        .position(|&b| b == 0)
        .ok_or(LobbyError::Malformed(what))?;
    Ok((&body[..end], &body[end + 1..]))
}

/// Encodes a stat string with the pinned `UTIL_EncodeStatString` relation:
/// every eighth output byte is a mask, and each following byte has its low
/// bit set. The decoder in `game_info` inverts exactly this relation.
pub fn encode_stat_string(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len() + raw.len() / 7 + 1);
    for group in raw.chunks(7) {
        let mut mask = 1u8;
        for (i, b) in group.iter().enumerate() {
            if b & 1 == 0 {
                mask |= 1 << (i + 1);
            }
        }
        out.push(mask);
        for b in group {
            out.push(b | 1);
        }
    }
    out
}

/// Builds a `W3GS_PING_FROM_HOST` frame carrying the caller's tick value.
pub fn encode_ping(ticks: u32) -> Result<Vec<u8>, LobbyError> {
    Ok(encode(W3GS_FAMILY, PING_FROM_HOST_ID, &ticks.to_le_bytes())?)
}

/// Reads a `W3GS_PING_FROM_HOST` tick value.
pub fn decode_ping(family: u8, id: u8, body: &[u8]) -> Result<u32, LobbyError> {
    let body = check(family, id, PING_FROM_HOST_ID, body)?;
    if body.len() != 4 {
        return Err(LobbyError::Malformed("ping body is not the 4-byte tick"));
    }
    Ok(u32::from_le_bytes(body.try_into().expect("checked length")))
}

/// Builds a `W3GS_PONG_TO_HOST` frame echoing the host's ping value.
pub fn encode_pong(pong: u32) -> Result<Vec<u8>, LobbyError> {
    Ok(encode(W3GS_FAMILY, PONG_TO_HOST_ID, &pong.to_le_bytes())?)
}

/// Reads a `W3GS_PONG_TO_HOST` echo value.
pub fn decode_pong(family: u8, id: u8, body: &[u8]) -> Result<u32, LobbyError> {
    let body = check(family, id, PONG_TO_HOST_ID, body)?;
    if body.len() != 4 {
        return Err(LobbyError::Malformed("pong body is not the 4-byte echo"));
    }
    Ok(u32::from_le_bytes(body.try_into().expect("checked length")))
}

/// Builds a `W3GS_LEAVEGAME` frame with the caller's reason word.
pub fn encode_leave(reason: u32) -> Result<Vec<u8>, LobbyError> {
    Ok(encode(W3GS_FAMILY, LEAVEGAME_ID, &reason.to_le_bytes())?)
}

/// Reads a `W3GS_LEAVEGAME` reason word.
pub fn decode_leave(family: u8, id: u8, body: &[u8]) -> Result<u32, LobbyError> {
    let body = check(family, id, LEAVEGAME_ID, body)?;
    if body.len() != 4 {
        return Err(LobbyError::Malformed("leave body is not the 4-byte reason"));
    }
    Ok(u32::from_le_bytes(body.try_into().expect("checked length")))
}

/// Classifies a leave/disconnect code. Unknown codes stay raw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaveCode {
    /// Clean disconnect.
    Disconnect,
    /// Lost the connection.
    Lost,
    /// Lost buildings (observer downgrade path).
    LostBuildings,
    /// Won.
    Won,
    /// Draw.
    Draw,
    /// Became an observer.
    Observer,
    /// Returned to the lobby.
    Lobby,
    /// GProxy reconnect extension. Separate negotiation; never assumed.
    GProxy,
    /// Any other code, retained raw.
    Unknown(u32),
}

impl LeaveCode {
    /// Classifies a `PLAYERLEAVE`/`LEAVEGAME` code word.
    pub fn of(code: u32) -> Self {
        match code {
            LEAVE_DISCONNECT => Self::Disconnect,
            LEAVE_LOST => Self::Lost,
            LEAVE_LOSTBUILDINGS => Self::LostBuildings,
            LEAVE_WON => Self::Won,
            LEAVE_DRAW => Self::Draw,
            LEAVE_OBSERVER => Self::Observer,
            LEAVE_LOBBY => Self::Lobby,
            LEAVE_GPROXY => Self::GProxy,
            other => Self::Unknown(other),
        }
    }
}

/// Builds a `W3GS_GAMELOADED_SELF` frame. Empty body.
pub fn encode_gameloaded_self() -> Result<Vec<u8>, LobbyError> {
    Ok(encode(W3GS_FAMILY, GAMELOADED_SELF_ID, &[])?)
}

/// Reads a `W3GS_GAMELOADED_SELF` frame. Any body is refused.
pub fn decode_gameloaded_self(family: u8, id: u8, body: &[u8]) -> Result<(), LobbyError> {
    let body = check(family, id, GAMELOADED_SELF_ID, body)?;
    if !body.is_empty() {
        return Err(LobbyError::Malformed("gameloaded-self carries a body"));
    }
    Ok(())
}

/// Builds a `W3GS_GAMELOADED_OTHERS` frame for one player id.
pub fn encode_gameloaded_others(pid: u8) -> Result<Vec<u8>, LobbyError> {
    Ok(encode(W3GS_FAMILY, GAMELOADED_OTHERS_ID, &[pid])?)
}

/// Reads the player id from a `W3GS_GAMELOADED_OTHERS` frame.
pub fn decode_gameloaded_others(family: u8, id: u8, body: &[u8]) -> Result<u8, LobbyError> {
    let body = check(family, id, GAMELOADED_OTHERS_ID, body)?;
    if body.len() != 1 {
        return Err(LobbyError::Malformed(
            "gameloaded-others is not the 1-byte player id",
        ));
    }
    Ok(body[0])
}

/// Builds `W3GS_COUNTDOWN_START` / `W3GS_COUNTDOWN_END`. Empty bodies.
pub fn encode_countdown(start: bool) -> Result<Vec<u8>, LobbyError> {
    let id = if start {
        COUNTDOWN_START_ID
    } else {
        COUNTDOWN_END_ID
    };
    Ok(encode(W3GS_FAMILY, id, &[])?)
}

/// Reads a countdown frame, returning true for start and false for end.
/// Any body is refused.
pub fn decode_countdown(family: u8, id: u8, body: &[u8]) -> Result<bool, LobbyError> {
    if family != W3GS_FAMILY {
        return Err(LobbyError::WrongFamily(family));
    }
    let start = match id {
        COUNTDOWN_START_ID => true,
        COUNTDOWN_END_ID => false,
        other => return Err(LobbyError::WrongId(other)),
    };
    if !body.is_empty() {
        return Err(LobbyError::Malformed("countdown carries a body"));
    }
    Ok(start)
}

/// Builds a `W3GS_PLAYERLEAVE_OTHERS` frame. PID 255 is refused, following
/// the pinned sender.
pub fn encode_playerleave_others(pid: u8, code: u32) -> Result<Vec<u8>, LobbyError> {
    if pid == 255 {
        return Err(LobbyError::Fields("playerleave pid 255 is refused"));
    }
    let mut body = vec![pid];
    body.extend_from_slice(&code.to_le_bytes());
    Ok(encode(W3GS_FAMILY, PLAYERLEAVE_OTHERS_ID, &body)?)
}

/// A player-leave notice: who left and with which code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerLeft {
    /// Leaving player id.
    pub pid: u8,
    /// Raw leave code; classify with [`LeaveCode::of`].
    pub code: u32,
}

/// Reads a `W3GS_PLAYERLEAVE_OTHERS` frame.
pub fn decode_playerleave_others(
    family: u8,
    id: u8,
    body: &[u8],
) -> Result<PlayerLeft, LobbyError> {
    let body = check(family, id, PLAYERLEAVE_OTHERS_ID, body)?;
    if body.len() != 5 {
        return Err(LobbyError::Malformed(
            "playerleave-others is not pid plus the 4-byte code",
        ));
    }
    Ok(PlayerLeft {
        pid: body[0],
        code: u32::from_le_bytes(body[1..5].try_into().expect("checked length")),
    })
}

/// Caller-supplied player-info fields for `W3GS_PLAYERINFO`.
#[derive(Debug, Clone, Copy)]
pub struct PlayerInfo<'a> {
    /// Join counter the pinned sender writes as 2; preserved as supplied.
    pub join_counter: u32,
    /// Player id.
    pub pid: u8,
    /// Player name bytes: nonempty, at most 15, no terminator.
    pub name: &'a [u8],
    /// External IPv4 address, network order.
    pub external_ip: [u8; 4],
    /// Internal IPv4 address, network order.
    pub internal_ip: [u8; 4],
}

/// A parsed `W3GS_PLAYERINFO` body, borrowed from the frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedPlayerInfo<'a> {
    /// Raw join counter, not a project peer id.
    pub join_counter: u32,
    /// Player id.
    pub pid: u8,
    /// Name bytes without the terminator.
    pub name: &'a [u8],
    /// External IPv4 address, network order.
    pub external_ip: [u8; 4],
    /// Internal IPv4 address, network order.
    pub internal_ip: [u8; 4],
}

/// Builds a `W3GS_PLAYERINFO` frame from caller fields.
pub fn encode_playerinfo(info: &PlayerInfo<'_>) -> Result<Vec<u8>, LobbyError> {
    if info.name.is_empty() || info.name.contains(&0) {
        return Err(LobbyError::Fields("player name must be nonempty and unterminated"));
    }
    if info.name.len() > 15 {
        return Err(LobbyError::Fields("player name exceeds the 15-byte sender profile"));
    }
    let mut body = Vec::with_capacity(32 + info.name.len());
    body.extend_from_slice(&info.join_counter.to_le_bytes());
    body.push(info.pid);
    body.extend_from_slice(info.name);
    body.extend_from_slice(&[1, 0, 2, 0, 0, 0]);
    body.extend_from_slice(&info.external_ip);
    body.extend_from_slice(&[0; 8]);
    body.extend_from_slice(&[2, 0, 0, 0]);
    body.extend_from_slice(&info.internal_ip);
    body.extend_from_slice(&[0; 8]);
    Ok(encode(W3GS_FAMILY, PLAYERINFO_ID, &body)?)
}

/// Reads a `W3GS_PLAYERINFO` frame, checking the pinned reserved words.
pub fn decode_playerinfo(
    family: u8,
    id: u8,
    body: &[u8],
) -> Result<ParsedPlayerInfo<'_>, LobbyError> {
    let body = check(family, id, PLAYERINFO_ID, body)?;
    if body.len() < 4 + 1 + 1 {
        return Err(LobbyError::Malformed("playerinfo is shorter than counter, pid and name"));
    }
    let join_counter = u32::from_le_bytes(body[0..4].try_into().expect("checked length"));
    let pid = body[4];
    let (name, rest) = cstring(&body[5..], "unterminated playerinfo name")?;
    if name.is_empty() || name.len() > 31 {
        return Err(LobbyError::Malformed("playerinfo name is empty or overlong"));
    }
    // Reserved words in sender order: 01 00 | 02 00 | port 00 00 |
    // external 4 | zeros 8 | 02 00 | port 00 00 | internal 4 | zeros 8.
    if rest.len() != 32 {
        return Err(LobbyError::Malformed(
            "playerinfo tail is not the 32-byte address block",
        ));
    }
    if rest[0..2] != [1, 0]
        || rest[2..4] != [2, 0]
        || rest[4..6] != [0, 0]
        || rest[10..18] != [0; 8]
        || rest[18..20] != [2, 0]
        || rest[20..22] != [0, 0]
        || rest[26..32] != [0; 6]
    {
        return Err(LobbyError::Malformed(
            "playerinfo reserved words differ from the pinned sender",
        ));
    }
    Ok(ParsedPlayerInfo {
        join_counter,
        pid,
        name,
        external_ip: rest[6..10].try_into().expect("checked tail"),
        internal_ip: rest[22..26].try_into().expect("checked tail"),
    })
}

/// A parsed W3GS chat body, borrowed from the frame. Shared by
/// `W3GS_CHAT_FROM_HOST` and `W3GS_CHAT_TO_HOST`, which carry the same
/// layout in opposite directions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChatMessage<'a> {
    /// Receiver ids, in order.
    pub to_pids: &'a [u8],
    /// Sender id.
    pub from_pid: u8,
    /// Flag: 16 message, 17-20 change request, 32 message with extra flags.
    pub flag: u8,
    /// Four extra flag bytes, present only when `flag == 32`.
    pub extra: Option<&'a [u8; 4]>,
    /// Message bytes (flag 16/32) or the single change byte (flag 17-20).
    pub payload: &'a [u8],
}

fn decode_chat_body(body: &[u8]) -> Result<ChatMessage<'_>, LobbyError> {
    if body.is_empty() {
        return Err(LobbyError::Malformed("chat body has no receiver count"));
    }
    let total = body[0] as usize;
    if total == 0 || total > 16 {
        return Err(LobbyError::Malformed(
            "chat receiver count is not in 1..=16",
        ));
    }
    if body.len() < 1 + total + 2 {
        return Err(LobbyError::Malformed("chat body is shorter than its receivers"));
    }
    let to_pids = &body[1..1 + total];
    let from_pid = body[1 + total];
    let flag = body[1 + total + 1];
    let rest = &body[1 + total + 2..];
    match flag {
        CHAT_FLAG_MESSAGE => {
            if rest.is_empty() || rest.contains(&0) {
                return Err(LobbyError::Malformed("chat message is empty or NUL-bearing"));
            }
            Ok(ChatMessage {
                to_pids,
                from_pid,
                flag,
                extra: None,
                payload: rest,
            })
        }
        CHAT_FLAG_TEAM | CHAT_FLAG_COLOUR | CHAT_FLAG_RACE | CHAT_FLAG_HANDICAP => {
            if rest.len() != 1 {
                return Err(LobbyError::Malformed(
                    "chat change request is not the single change byte",
                ));
            }
            Ok(ChatMessage {
                to_pids,
                from_pid,
                flag,
                extra: None,
                payload: rest,
            })
        }
        CHAT_FLAG_EXTRA => {
            if rest.len() < 5 || rest[4..].is_empty() || rest[4..].contains(&0) {
                return Err(LobbyError::Malformed(
                    "chat extra-flags body is not 4 flag bytes plus a message",
                ));
            }
            Ok(ChatMessage {
                to_pids,
                from_pid,
                flag,
                extra: Some(rest[0..4].try_into().expect("checked length")),
                payload: &rest[4..],
            })
        }
        _ => Err(LobbyError::Malformed("chat flag is not 16, 17-20 or 32")),
    }
}

/// Reads a `W3GS_CHAT_FROM_HOST` body.
pub fn decode_chat_from_host(family: u8, id: u8, body: &[u8]) -> Result<ChatMessage<'_>, LobbyError> {
    check(family, id, CHAT_FROM_HOST_ID, body)?;
    decode_chat_body(body)
}

/// Reads a `W3GS_CHAT_TO_HOST` body.
pub fn decode_chat_to_host(family: u8, id: u8, body: &[u8]) -> Result<ChatMessage<'_>, LobbyError> {
    check(family, id, CHAT_TO_HOST_ID, body)?;
    decode_chat_body(body)
}

/// Caller-supplied chat fields for `W3GS_CHAT_FROM_HOST`.
#[derive(Debug, Clone, Copy)]
pub struct ChatSend<'a> {
    /// Receiver ids: 1 to 16 entries.
    pub to_pids: &'a [u8],
    /// Sender id.
    pub from_pid: u8,
    /// Flag: 16, 17-20 or 32.
    pub flag: u8,
    /// Four extra bytes, required exactly when `flag == 32`.
    pub extra: Option<&'a [u8; 4]>,
    /// Message bytes (flag 16/32) or the single change byte.
    pub payload: &'a [u8],
}

/// Builds a `W3GS_CHAT_FROM_HOST` frame.
pub fn encode_chat_from_host(chat: &ChatSend<'_>) -> Result<Vec<u8>, LobbyError> {
    if chat.to_pids.is_empty() || chat.to_pids.len() > 16 {
        return Err(LobbyError::Fields("chat needs 1..=16 receivers"));
    }
    let mut body = Vec::with_capacity(3 + chat.to_pids.len() + chat.payload.len() + 4);
    body.push(chat.to_pids.len() as u8);
    body.extend_from_slice(chat.to_pids);
    body.push(chat.from_pid);
    body.push(chat.flag);
    match chat.flag {
        CHAT_FLAG_MESSAGE => {
            if chat.payload.is_empty() || chat.payload.contains(&0) {
                return Err(LobbyError::Fields("chat message is empty or NUL-bearing"));
            }
            if chat.extra.is_some() {
                return Err(LobbyError::Fields("flag 16 carries no extra bytes"));
            }
            body.extend_from_slice(chat.payload);
        }
        CHAT_FLAG_TEAM | CHAT_FLAG_COLOUR | CHAT_FLAG_RACE | CHAT_FLAG_HANDICAP => {
            if chat.payload.len() != 1 {
                return Err(LobbyError::Fields(
                    "chat change request is exactly one byte",
                ));
            }
            if chat.extra.is_some() {
                return Err(LobbyError::Fields("change requests carry no extra bytes"));
            }
            body.extend_from_slice(chat.payload);
        }
        CHAT_FLAG_EXTRA => {
            let Some(extra) = chat.extra else {
                return Err(LobbyError::Fields("flag 32 needs four extra bytes"));
            };
            if chat.payload.is_empty() || chat.payload.contains(&0) {
                return Err(LobbyError::Fields("chat message is empty or NUL-bearing"));
            }
            body.extend_from_slice(extra);
            body.extend_from_slice(chat.payload);
        }
        _ => return Err(LobbyError::Fields("chat flag is not 16, 17-20 or 32")),
    }
    Ok(encode(W3GS_FAMILY, CHAT_FROM_HOST_ID, &body)?)
}

/// One lagging player: id plus milliseconds since the lag began.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LagEntry {
    /// Lagging player id.
    pub pid: u8,
    /// Milliseconds since the lag began, as the host reports it.
    pub elapsed_ms: u32,
}

/// Builds a `W3GS_START_LAG` frame. An empty lagger list is refused,
/// following the pinned sender.
pub fn encode_start_lag(laggers: &[LagEntry]) -> Result<Vec<u8>, LobbyError> {
    if laggers.is_empty() || laggers.len() > 255 {
        return Err(LobbyError::Fields("start-lag needs 1..=255 laggers"));
    }
    let mut body = Vec::with_capacity(1 + 5 * laggers.len());
    body.push(laggers.len() as u8);
    for lagger in laggers {
        body.push(lagger.pid);
        body.extend_from_slice(&lagger.elapsed_ms.to_le_bytes());
    }
    Ok(encode(W3GS_FAMILY, START_LAG_ID, &body)?)
}

/// Reads a `W3GS_START_LAG` frame.
pub fn decode_start_lag(family: u8, id: u8, body: &[u8]) -> Result<Vec<LagEntry>, LobbyError> {
    let body = check(family, id, START_LAG_ID, body)?;
    if body.is_empty() {
        return Err(LobbyError::Malformed("start-lag has no lagger count"));
    }
    let count = body[0] as usize;
    if count == 0 || body.len() != 1 + 5 * count {
        return Err(LobbyError::Malformed(
            "start-lag entries do not fill the body exactly",
        ));
    }
    Ok(body[1..]
        .chunks_exact(5)
        .map(|e| LagEntry {
            pid: e[0],
            elapsed_ms: u32::from_le_bytes(e[1..5].try_into().expect("chunked entry")),
        })
        .collect())
}

/// Builds a `W3GS_STOP_LAG` frame.
pub fn encode_stop_lag(entry: &LagEntry) -> Result<Vec<u8>, LobbyError> {
    let mut body = vec![entry.pid];
    body.extend_from_slice(&entry.elapsed_ms.to_le_bytes());
    Ok(encode(W3GS_FAMILY, STOP_LAG_ID, &body)?)
}

/// Reads a `W3GS_STOP_LAG` frame.
pub fn decode_stop_lag(family: u8, id: u8, body: &[u8]) -> Result<LagEntry, LobbyError> {
    let body = check(family, id, STOP_LAG_ID, body)?;
    if body.len() != 5 {
        return Err(LobbyError::Malformed(
            "stop-lag is not pid plus the 4-byte elapsed time",
        ));
    }
    Ok(LagEntry {
        pid: body[0],
        elapsed_ms: u32::from_le_bytes(body[1..5].try_into().expect("checked length")),
    })
}

/// A host-side join request (`W3GS_REQJOIN`), borrowed from the frame.
/// Complements the client-side [`super::game_info::GameInfo::join_request`]
/// encoder: this is what a PeaceCraft host reads from an original client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IncomingJoin<'a> {
    /// Host counter the client echoes from the advertisement.
    pub host_counter: u32,
    /// Entry key the client echoes from the advertisement.
    pub entry_key: u32,
    /// Unspecified byte before the listen port, preserved as supplied.
    pub unknown: u8,
    /// Client's listen port, little-endian.
    pub listen_port: u16,
    /// Caller-supplied peer key, not a project player id.
    pub peer_key: u32,
    /// Name bytes without the terminator.
    pub player_name: &'a [u8],
    /// Bytes after the name terminator. The pinned receiver needs at least
    /// ten; the original 1.29.2 client sends a longer address tail, kept
    /// opaque here.
    pub address_tail: &'a [u8],
}

impl<'a> Frame<'a> {
    /// Reads the host side of a `W3GS_REQJOIN` frame.
    pub fn incoming_join(&self) -> Result<IncomingJoin<'a>, LobbyError> {
        decode_incoming_join(self.family(), self.id(), self.body())
    }
}

impl OwnedFrame {
    /// Reads the host side of a retained `W3GS_REQJOIN` frame.
    pub fn incoming_join(&self) -> Result<IncomingJoin<'_>, LobbyError> {
        decode_incoming_join(self.family(), self.id(), self.body())
    }
}

/// Reads the host side of a `W3GS_REQJOIN` body.
pub fn decode_incoming_join(
    family: u8,
    id: u8,
    body: &[u8],
) -> Result<IncomingJoin<'_>, LobbyError> {
    use super::game_info::REQJOIN_ID;
    let body = check(family, id, REQJOIN_ID, body)?;
    if body.len() < 20 {
        return Err(LobbyError::Malformed(
            "reqjoin is shorter than counters, ports and a name",
        ));
    }
    let host_counter = u32::from_le_bytes(body[0..4].try_into().expect("checked length"));
    let entry_key = u32::from_le_bytes(body[4..8].try_into().expect("checked length"));
    let unknown = body[8];
    let listen_port = u16::from_le_bytes(body[9..11].try_into().expect("checked length"));
    let peer_key = u32::from_le_bytes(body[11..15].try_into().expect("checked length"));
    let (player_name, address_tail) = cstring(&body[15..], "unterminated reqjoin name")?;
    if player_name.is_empty() {
        return Err(LobbyError::Malformed("reqjoin name is empty"));
    }
    if address_tail.len() < 10 {
        return Err(LobbyError::Malformed(
            "reqjoin address tail is shorter than ten bytes",
        ));
    }
    Ok(IncomingJoin {
        host_counter,
        entry_key,
        unknown,
        listen_port,
        peer_key,
        player_name,
        address_tail,
    })
}

/// A client's map-size report (`W3GS_MAPSIZE`), read by the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapSizeReport {
    /// Four unspecified prefix bytes, preserved as supplied.
    pub prefix: [u8; 4],
    /// Size flag: 1 means the client has the map, 3 continues a download.
    pub size_flag: u8,
    /// Whole-file map size the client reports.
    pub map_size: u32,
}

impl<'a> Frame<'a> {
    /// Reads the host side of a `W3GS_MAPSIZE` frame.
    pub fn map_size_report(&self) -> Result<MapSizeReport, LobbyError> {
        decode_map_size_report(self.family(), self.id(), self.body())
    }
}

impl OwnedFrame {
    /// Reads the host side of a retained `W3GS_MAPSIZE` frame.
    pub fn map_size_report(&self) -> Result<MapSizeReport, LobbyError> {
        decode_map_size_report(self.family(), self.id(), self.body())
    }
}

/// Reads the host side of a `W3GS_MAPSIZE` body.
pub fn decode_map_size_report(
    family: u8,
    id: u8,
    body: &[u8],
) -> Result<MapSizeReport, LobbyError> {
    use super::map_check::MAPSIZE_ID;
    let body = check(family, id, MAPSIZE_ID, body)?;
    if body.len() != 9 {
        return Err(LobbyError::Malformed(
            "mapsize is not prefix, size flag and map size",
        ));
    }
    Ok(MapSizeReport {
        prefix: body[0..4].try_into().expect("checked length"),
        size_flag: body[4],
        map_size: u32::from_le_bytes(body[5..9].try_into().expect("checked length")),
    })
}

/// Size flag a client sends when it already holds the map.
pub const MAPSIZE_HAVE_MAP: u8 = 1;
/// Size flag a client sends to continue a map download.
pub const MAPSIZE_CONTINUE_DOWNLOAD: u8 = 3;

/// A `W3GS_MAPPART` header, validated but never used to transfer bytes.
/// Map download is out of scope: proprietary asset transfer is not part of
/// the test harness, and a lobby must refuse mismatched maps before any
/// part is sent. The payload span is borrowed so the caller can account for
/// the bytes without copying them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapPartHeader<'a> {
    /// Sender player id.
    pub from_pid: u8,
    /// Receiver player id.
    pub to_pid: u8,
    /// Four unspecified prefix bytes, preserved as supplied.
    pub prefix: [u8; 4],
    /// Start offset into the map file.
    pub start: u32,
    /// CRC-32 of the payload span, as sent. Never verified here.
    pub crc: u32,
    /// Payload bytes after the header.
    pub data: &'a [u8],
}

impl<'a> Frame<'a> {
    /// Validates a `W3GS_MAPPART` header without accepting a transfer.
    pub fn map_part_header(&self) -> Result<MapPartHeader<'a>, LobbyError> {
        decode_map_part_header(self.family(), self.id(), self.body())
    }
}

impl OwnedFrame {
    /// Validates a retained `W3GS_MAPPART` header without accepting a transfer.
    pub fn map_part_header(&self) -> Result<MapPartHeader<'_>, LobbyError> {
        decode_map_part_header(self.family(), self.id(), self.body())
    }
}

/// Validates a `W3GS_MAPPART` body header without accepting a transfer.
pub fn decode_map_part_header(
    family: u8,
    id: u8,
    body: &[u8],
) -> Result<MapPartHeader<'_>, LobbyError> {
    let body = check(family, id, MAPPART_ID, body)?;
    if body.len() < 14 {
        return Err(LobbyError::Malformed(
            "mappart is shorter than pids, prefix, start and crc",
        ));
    }
    Ok(MapPartHeader {
        to_pid: body[0],
        from_pid: body[1],
        prefix: body[2..6].try_into().expect("checked length"),
        start: u32::from_le_bytes(body[6..10].try_into().expect("checked length")),
        crc: u32::from_le_bytes(body[10..14].try_into().expect("checked length")),
        data: &body[14..],
    })
}

/// A `W3GS_MAPPARTOK` acknowledgement, read by the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapPartOk {
    /// Sender player id.
    pub from_pid: u8,
    /// Receiver player id.
    pub to_pid: u8,
    /// Four unspecified prefix bytes, preserved as supplied.
    pub prefix: [u8; 4],
    /// Map size the receiver acknowledges.
    pub map_size: u32,
}

/// Reads a `W3GS_MAPPARTOK` body.
pub fn decode_map_part_ok(family: u8, id: u8, body: &[u8]) -> Result<MapPartOk, LobbyError> {
    let body = check(family, id, MAPPARTOK_ID, body)?;
    if body.len() != 10 {
        return Err(LobbyError::Malformed(
            "mappartok is not pids, prefix and map size",
        ));
    }
    Ok(MapPartOk {
        from_pid: body[0],
        to_pid: body[1],
        prefix: body[2..6].try_into().expect("checked length"),
        map_size: u32::from_le_bytes(body[6..10].try_into().expect("checked length")),
    })
}

/// Builds a `W3GS_STARTDOWNLOAD` frame for one player id.
pub fn encode_start_download(from_pid: u8) -> Result<Vec<u8>, LobbyError> {
    let mut body = vec![1, 0, 0, 0];
    body.push(from_pid);
    Ok(encode(W3GS_FAMILY, STARTDOWNLOAD_ID, &body)?)
}

/// Reads a `W3GS_STARTDOWNLOAD` frame, returning the player id.
pub fn decode_start_download(family: u8, id: u8, body: &[u8]) -> Result<u8, LobbyError> {
    let body = check(family, id, STARTDOWNLOAD_ID, body)?;
    if body.len() != 5 {
        return Err(LobbyError::Malformed(
            "startdownload is not prefix plus the player id",
        ));
    }
    Ok(body[4])
}

/// Builds a `W3GS_OUTGOING_KEEPALIVE` frame: one unspecified byte plus the
/// caller's checksum word, used in replays.
pub fn encode_keepalive(unknown: u8, checksum: u32) -> Result<Vec<u8>, LobbyError> {
    let mut body = vec![unknown];
    body.extend_from_slice(&checksum.to_le_bytes());
    Ok(encode(W3GS_FAMILY, OUTGOING_KEEPALIVE_ID, &body)?)
}

/// A parsed keepalive: the unspecified byte and checksum word, both raw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Keepalive {
    /// Unspecified byte, preserved as supplied.
    pub unknown: u8,
    /// Checksum word, retained without verification.
    pub checksum: u32,
}

/// Reads a `W3GS_OUTGOING_KEEPALIVE` frame.
pub fn decode_keepalive(family: u8, id: u8, body: &[u8]) -> Result<Keepalive, LobbyError> {
    let body = check(family, id, OUTGOING_KEEPALIVE_ID, body)?;
    if body.len() != 5 {
        return Err(LobbyError::Malformed(
            "keepalive is not the unspecified byte plus checksum",
        ));
    }
    Ok(Keepalive {
        unknown: body[0],
        checksum: u32::from_le_bytes(body[1..5].try_into().expect("checked length")),
    })
}

/// A LAN announce frame: create, refresh or decreates a broadcast game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Announce {
    /// `W3GS_CREATEGAME`: product, version and host counter.
    Create {
        /// Four product bytes, wire order.
        product: [u8; 4],
        /// Full little-endian version word.
        version: u32,
        /// Raw host counter.
        host_counter: u32,
    },
    /// `W3GS_REFRESHGAME`: host counter plus player counts.
    Refresh {
        /// Raw host counter.
        host_counter: u32,
        /// Current players.
        players: u32,
        /// Player slots (non-observer count in the source comment).
        player_slots: u32,
    },
    /// `W3GS_DECREATEGAME`: host counter.
    Decreate {
        /// Raw host counter.
        host_counter: u32,
    },
}

/// Builds a `W3GS_CREATEGAME` announce frame.
pub fn encode_creategame(product: [u8; 4], version: u32, host_counter: u32) -> Result<Vec<u8>, LobbyError> {
    let mut body = Vec::with_capacity(12);
    body.extend_from_slice(&product);
    body.extend_from_slice(&version.to_le_bytes());
    body.extend_from_slice(&host_counter.to_le_bytes());
    Ok(encode(W3GS_FAMILY, CREATEGAME_ID, &body)?)
}

/// Builds a `W3GS_REFRESHGAME` announce frame.
pub fn encode_refreshgame(
    host_counter: u32,
    players: u32,
    player_slots: u32,
) -> Result<Vec<u8>, LobbyError> {
    let mut body = Vec::with_capacity(12);
    body.extend_from_slice(&host_counter.to_le_bytes());
    body.extend_from_slice(&players.to_le_bytes());
    body.extend_from_slice(&player_slots.to_le_bytes());
    Ok(encode(W3GS_FAMILY, REFRESHGAME_ID, &body)?)
}

/// Builds a `W3GS_DECREATEGAME` announce frame.
pub fn encode_decreategame(host_counter: u32) -> Result<Vec<u8>, LobbyError> {
    Ok(encode(W3GS_FAMILY, DECREATEGAME_ID,
        &host_counter.to_le_bytes(),
    )?)
}

/// Reads a `W3GS_CREATEGAME`, `W3GS_REFRESHGAME` or `W3GS_DECREATEGAME` body.
pub fn decode_announce(family: u8, id: u8, body: &[u8]) -> Result<Announce, LobbyError> {
    if family != W3GS_FAMILY {
        return Err(LobbyError::WrongFamily(family));
    }
    match id {
        CREATEGAME_ID => {
            if body.len() != 12 {
                return Err(LobbyError::Malformed(
                    "creategame is not product, version and host counter",
                ));
            }
            Ok(Announce::Create {
                product: body[0..4].try_into().expect("checked length"),
                version: u32::from_le_bytes(body[4..8].try_into().expect("checked length")),
                host_counter: u32::from_le_bytes(body[8..12].try_into().expect("checked length")),
            })
        }
        REFRESHGAME_ID => {
            if body.len() != 12 {
                return Err(LobbyError::Malformed(
                    "refreshgame is not host counter, players and player slots",
                ));
            }
            Ok(Announce::Refresh {
                host_counter: u32::from_le_bytes(body[0..4].try_into().expect("checked length")),
                players: u32::from_le_bytes(body[4..8].try_into().expect("checked length")),
                player_slots: u32::from_le_bytes(body[8..12].try_into().expect("checked length")),
            })
        }
        DECREATEGAME_ID => {
            if body.len() != 4 {
                return Err(LobbyError::Malformed(
                    "decreategame is not the host counter",
                ));
            }
            Ok(Announce::Decreate {
                host_counter: u32::from_le_bytes(body[0..4].try_into().expect("checked length")),
            })
        }
        other => Err(LobbyError::WrongId(other)),
    }
}

macro_rules! lobby_frame_methods {
    ($frame:ty, $lifetime:lifetime) => {
        impl<$lifetime> $frame {
            /// Reads a `W3GS_PING_FROM_HOST` tick value.
            pub fn ping(&self) -> Result<u32, LobbyError> {
                decode_ping(self.family(), self.id(), self.body())
            }
            /// Reads a `W3GS_PONG_TO_HOST` echo value.
            pub fn pong(&self) -> Result<u32, LobbyError> {
                decode_pong(self.family(), self.id(), self.body())
            }
            /// Reads a `W3GS_LEAVEGAME` reason word.
            pub fn leave_reason(&self) -> Result<u32, LobbyError> {
                decode_leave(self.family(), self.id(), self.body())
            }
            /// Reads a `W3GS_GAMELOADED_SELF` frame.
            pub fn gameloaded_self(&self) -> Result<(), LobbyError> {
                decode_gameloaded_self(self.family(), self.id(), self.body())
            }
            /// Reads a `W3GS_GAMELOADED_OTHERS` player id.
            pub fn gameloaded_others(&self) -> Result<u8, LobbyError> {
                decode_gameloaded_others(self.family(), self.id(), self.body())
            }
            /// Reads a countdown frame: true for start, false for end.
            pub fn countdown(&self) -> Result<bool, LobbyError> {
                decode_countdown(self.family(), self.id(), self.body())
            }
            /// Reads a `W3GS_PLAYERLEAVE_OTHERS` notice.
            pub fn player_left(&self) -> Result<PlayerLeft, LobbyError> {
                decode_playerleave_others(self.family(), self.id(), self.body())
            }
            /// Reads a `W3GS_PLAYERINFO` body.
            pub fn player_info(&self) -> Result<ParsedPlayerInfo<'_>, LobbyError> {
                decode_playerinfo(self.family(), self.id(), self.body())
            }
            /// Reads a `W3GS_CHAT_FROM_HOST` body.
            pub fn chat_from_host(&self) -> Result<ChatMessage<'_>, LobbyError> {
                decode_chat_from_host(self.family(), self.id(), self.body())
            }
            /// Reads a `W3GS_CHAT_TO_HOST` body.
            pub fn chat_to_host(&self) -> Result<ChatMessage<'_>, LobbyError> {
                decode_chat_to_host(self.family(), self.id(), self.body())
            }
            /// Reads a `W3GS_START_LAG` frame.
            pub fn start_lag(&self) -> Result<Vec<LagEntry>, LobbyError> {
                decode_start_lag(self.family(), self.id(), self.body())
            }
            /// Reads a `W3GS_STOP_LAG` frame.
            pub fn stop_lag(&self) -> Result<LagEntry, LobbyError> {
                decode_stop_lag(self.family(), self.id(), self.body())
            }
            /// Reads a `W3GS_OUTGOING_KEEPALIVE` frame.
            pub fn keepalive(&self) -> Result<Keepalive, LobbyError> {
                decode_keepalive(self.family(), self.id(), self.body())
            }
            /// Reads a LAN announce frame.
            pub fn announce(&self) -> Result<Announce, LobbyError> {
                decode_announce(self.family(), self.id(), self.body())
            }
        }
    };
}

lobby_frame_methods!(Frame<'frame>, 'frame);
lobby_frame_methods!(OwnedFrame, '_);

/// Address bytes the pinned `SEND_W3GS_SLOTINFOJOIN` writes: little-endian
/// `AF_INET`, as raw bytes.
pub const JOIN_AF_INET_LE: [u8; 2] = [2, 0];

/// Caller-supplied join endpoint for a `W3GS_SLOTINFOJOIN` frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JoinEndpoint {
    /// Receiver player id.
    pub pid: u8,
    /// Two address-family bytes, wire order.
    pub address_family: [u8; 2],
    /// Two port bytes, wire order.
    pub port: [u8; 2],
    /// Four external-IPv4 bytes, network order.
    pub external_ip: [u8; 4],
}

/// Encodes the shared `EncodeSlotInfo` blob: count byte, nine bytes per
/// slot, four seed bytes, layout style and player slots. The slot count is
/// capped by the caller so one blob cannot allocate unbounded records.
pub fn encode_slot_blob(
    slots: &[crate::classic_frame::slot_seed::RawSlot],
    seed: [u8; 4],
    layout_style: u8,
    player_slots: u8,
    max_slots: usize,
) -> Result<Vec<u8>, LobbyError> {
    if slots.len() > max_slots || slots.len() > 255 {
        return Err(LobbyError::Fields(
            "slot count exceeds the caller ceiling",
        ));
    }
    let mut blob = Vec::with_capacity(1 + 9 * slots.len() + 6);
    blob.push(slots.len() as u8);
    for slot in slots {
        blob.extend_from_slice(&slot.bytes());
    }
    blob.extend_from_slice(&seed);
    blob.push(layout_style);
    blob.push(player_slots);
    Ok(blob)
}

fn encode_prefixed_blob(blob: &[u8]) -> Result<Vec<u8>, LobbyError> {
    let len = u16::try_from(blob.len()).map_err(|_| {
        LobbyError::Frame(FrameError::Oversize {
            declared: blob.len(),
            limit: u16::MAX as usize,
        })
    })?;
    let mut body = Vec::with_capacity(2 + blob.len());
    body.extend_from_slice(&len.to_le_bytes());
    body.extend_from_slice(blob);
    Ok(body)
}

/// Builds a `W3GS_SLOTINFO` frame around the caller's slot blob.
pub fn encode_slotinfo(
    slots: &[crate::classic_frame::slot_seed::RawSlot],
    seed: [u8; 4],
    layout_style: u8,
    player_slots: u8,
    max_slots: usize,
) -> Result<Vec<u8>, LobbyError> {
    use crate::classic_frame::slot_seed::SLOTINFO_ID;
    let blob = encode_slot_blob(slots, seed, layout_style, player_slots, max_slots)?;
    let body = encode_prefixed_blob(&blob)?;
    Ok(encode(W3GS_FAMILY, SLOTINFO_ID, &body)?)
}

/// Builds a `W3GS_SLOTINFOJOIN` frame around the caller's slot blob and
/// join endpoint. The trailing unspecified words stay zero, following the
/// pinned sender.
pub fn encode_slotinfojoin(
    slots: &[crate::classic_frame::slot_seed::RawSlot],
    seed: [u8; 4],
    layout_style: u8,
    player_slots: u8,
    max_slots: usize,
    endpoint: &JoinEndpoint,
) -> Result<Vec<u8>, LobbyError> {
    use crate::classic_frame::slot_seed::SLOTINFOJOIN_ID;
    let blob = encode_slot_blob(slots, seed, layout_style, player_slots, max_slots)?;
    let mut body = encode_prefixed_blob(&blob)?;
    body.push(endpoint.pid);
    body.extend_from_slice(&endpoint.address_family);
    body.extend_from_slice(&endpoint.port);
    body.extend_from_slice(&endpoint.external_ip);
    body.extend_from_slice(&[0; 8]);
    Ok(encode(W3GS_FAMILY, SLOTINFOJOIN_ID, &body)?)
}

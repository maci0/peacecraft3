//! Classic Battle.net (BNCS) message bodies.
//!
//! Pinned sources: GHost++ `ghost/bnetprotocol.h` and
//! `ghost/bnetprotocol.cpp` at commit
//! `6ba90f426e96f80fe013a468d1e07151a510adbe` for message ids, chat event
//! ids, key results and every `SEND_*`/`RECEIVE_*` body below; PvPGN-PRO
//! `src/common/bnet_protocol.h` at `9cd173f4e02ba3d9f8f15a67ca308b5eb78723e`
//! for the shared header contract and the older game-list framing. The BNCS
//! connection-init byte is outside the four-byte frame format; see
//! [`super`] for framing.
//!
//! What this module does not do: prove a password, verify a key, execute a
//! version-check equation, or answer Warden. Those need key material or the
//! original binaries (BNCSUtil territory) and stay explicit refusals here.
//! Bodies return frames or borrow them; login, channel and advertisement
//! policy live in `crate::classic_session`.

use super::{BNCS_FAMILY, Frame, FrameError, OwnedFrame, encode};

/// `SID_NULL`.
pub const SID_NULL: u8 = 0;
/// `SID_STOPADV`.
pub const SID_STOPADV: u8 = 2;
/// `SID_GETADVLISTEX`.
pub const SID_GETADVLISTEX: u8 = 9;
/// `SID_ENTERCHAT`.
pub const SID_ENTERCHAT: u8 = 10;
/// `SID_JOINCHANNEL`.
pub const SID_JOINCHANNEL: u8 = 12;
/// `SID_CHATCOMMAND`.
pub const SID_CHATCOMMAND: u8 = 14;
/// `SID_CHATEVENT`.
pub const SID_CHATEVENT: u8 = 15;
/// `SID_CHECKAD`.
pub const SID_CHECKAD: u8 = 21;
/// `SID_STARTADVEX3`.
pub const SID_STARTADVEX3: u8 = 28;
/// `SID_DISPLAYAD`. No receive body in the retained sources.
pub const SID_DISPLAYAD: u8 = 33;
/// `SID_NOTIFYJOIN`.
pub const SID_NOTIFYJOIN: u8 = 34;
/// `SID_PING`.
pub const SID_PING: u8 = 37;
/// `SID_LOGONRESPONSE`.
pub const SID_LOGONRESPONSE: u8 = 41;
/// `SID_NETGAMEPORT`.
pub const SID_NETGAMEPORT: u8 = 69;
/// `SID_AUTH_INFO`.
pub const SID_AUTH_INFO: u8 = 80;
/// `SID_AUTH_CHECK`.
pub const SID_AUTH_CHECK: u8 = 81;
/// `SID_AUTH_ACCOUNTLOGON`.
pub const SID_AUTH_ACCOUNTLOGON: u8 = 83;
/// `SID_AUTH_ACCOUNTLOGONPROOF`.
pub const SID_AUTH_ACCOUNTLOGONPROOF: u8 = 84;
/// `SID_WARDEN`. Anti-cheat; refused, never answered.
pub const SID_WARDEN: u8 = 94;
/// `SID_FRIENDSLIST`.
pub const SID_FRIENDSLIST: u8 = 101;
/// `SID_CLANMEMBERLIST`.
pub const SID_CLANMEMBERLIST: u8 = 125;
/// `SID_CLANMEMBERSTATUSCHANGE`.
pub const SID_CLANMEMBERSTATUSCHANGE: u8 = 127;

/// `KR_GOOD`.
pub const KEY_GOOD: u32 = 0;
/// `KR_OLD_GAME_VERSION`.
pub const KEY_OLD_GAME_VERSION: u32 = 256;
/// `KR_INVALID_VERSION`.
pub const KEY_INVALID_VERSION: u32 = 257;
/// `KR_ROC_KEY_IN_USE`.
pub const KEY_ROC_IN_USE: u32 = 513;
/// `KR_TFT_KEY_IN_USE`.
pub const KEY_TFT_IN_USE: u32 = 529;

/// `EID_SHOWUSER`.
pub const EID_SHOWUSER: u32 = 1;
/// `EID_JOIN`.
pub const EID_JOIN: u32 = 2;
/// `EID_LEAVE`.
pub const EID_LEAVE: u32 = 3;
/// `EID_WHISPER`.
pub const EID_WHISPER: u32 = 4;
/// `EID_TALK`.
pub const EID_TALK: u32 = 5;
/// `EID_BROADCAST`.
pub const EID_BROADCAST: u32 = 6;
/// `EID_CHANNEL`.
pub const EID_CHANNEL: u32 = 7;
/// `EID_USERFLAGS`.
pub const EID_USERFLAGS: u32 = 9;
/// `EID_WHISPERSENT`.
pub const EID_WHISPERSENT: u32 = 10;
/// `EID_CHANNELFULL`.
pub const EID_CHANNELFULL: u32 = 13;
/// `EID_CHANNELDOESNOTEXIST`.
pub const EID_CHANNELDOESNOTEXIST: u32 = 14;
/// `EID_CHANNELRESTRICTED`.
pub const EID_CHANNELRESTRICTED: u32 = 15;
/// `EID_INFO`.
pub const EID_INFO: u32 = 18;
/// `EID_ERROR`.
pub const EID_ERROR: u32 = 19;
/// `EID_EMOTE`.
pub const EID_EMOTE: u32 = 23;

/// `SEND_SID_JOINCHANNEL` flag for joining the first channel.
pub const JOIN_FIRST: u32 = 1;
/// `SEND_SID_JOINCHANNEL` flag for joining without creating.
pub const JOIN_NO_CREATE: u32 = 2;

/// `SEND_SID_STARTADVEX3` state: public game.
pub const ADVEX_PUBLIC: u32 = 16;
/// `SEND_SID_STARTADVEX3` state: private game.
pub const ADVEX_PRIVATE: u32 = 17;
/// `SEND_SID_STARTADVEX3` state: close the advertisement.
pub const ADVEX_CLOSE: u32 = 18;

/// `SEND_SID_AUTH_LOGON` product version for Warcraft III.
pub const TFT_PRODUCT_VERSION: u32 = 14;

/// Why a BNCS body was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BncsError {
    /// Header family is not [`BNCS_FAMILY`].
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

impl std::fmt::Display for BncsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongFamily(v) => write!(f, "BNCS family {v} is not BNCS"),
            Self::WrongId(v) => write!(f, "BNCS id {v} is not accepted here"),
            Self::Malformed(v) => write!(f, "BNCS: {v}"),
            Self::Fields(v) => write!(f, "BNCS: {v}"),
            Self::Frame(v) => write!(f, "{v}"),
        }
    }
}

impl std::error::Error for BncsError {}

impl From<FrameError> for BncsError {
    fn from(v: FrameError) -> Self {
        Self::Frame(v)
    }
}

fn check(family: u8, id: u8, want: u8, body: &[u8]) -> Result<&[u8], BncsError> {
    if family != BNCS_FAMILY {
        return Err(BncsError::WrongFamily(family));
    }
    if id != want {
        return Err(BncsError::WrongId(id));
    }
    Ok(body)
}

fn cstring(body: &[u8], what: &'static str) -> Result<(&[u8], &[u8]), BncsError> {
    let end = body
        .iter()
        .position(|&b| b == 0)
        .ok_or(BncsError::Malformed(what))?;
    Ok((&body[..end], &body[end + 1..]))
}

/// Builds a `SID_NULL` frame. Empty body.
pub fn encode_null() -> Result<Vec<u8>, BncsError> {
    Ok(encode(BNCS_FAMILY, SID_NULL, &[])?)
}

/// Reads a `SID_NULL` frame. Any body is refused.
pub fn decode_null(family: u8, id: u8, body: &[u8]) -> Result<(), BncsError> {
    let body = check(family, id, SID_NULL, body)?;
    if !body.is_empty() {
        return Err(BncsError::Malformed("null carries a body"));
    }
    Ok(())
}

/// Builds the one-byte BNCS connection-init selector the pinned sender
/// writes before any framed packet.
pub fn encode_init_selector() -> Vec<u8> {
    vec![1]
}

/// Builds a `SID_PING` frame echoing the caller's four-byte value.
pub fn encode_ping(value: [u8; 4]) -> Result<Vec<u8>, BncsError> {
    Ok(encode(BNCS_FAMILY, SID_PING, &value)?)
}

/// Reads a `SID_PING` cookie: exactly four bytes, retained raw.
pub fn decode_ping(family: u8, id: u8, body: &[u8]) -> Result<[u8; 4], BncsError> {
    let body = check(family, id, SID_PING, body)?;
    if body.len() != 4 {
        return Err(BncsError::Malformed("ping is not the 4-byte value"));
    }
    Ok(body.try_into().expect("checked length"))
}

/// Builds a `SID_ENTERCHAT` frame. Account and stat string stay NULL on
/// Warcraft III, following the pinned sender.
pub fn encode_enterchat() -> Result<Vec<u8>, BncsError> {
    Ok(encode(BNCS_FAMILY, SID_ENTERCHAT, &[0, 0])?)
}

/// Reads a `SID_ENTERCHAT` reply: the unique name without its terminator.
pub fn decode_enterchat(family: u8, id: u8, body: &[u8]) -> Result<&[u8], BncsError> {
    let body = check(family, id, SID_ENTERCHAT, body)?;
    let (name, rest) = cstring(body, "unterminated enterchat name")?;
    if name.is_empty() {
        return Err(BncsError::Malformed("enterchat name is empty"));
    }
    if !rest.is_empty() {
        return Err(BncsError::Malformed("enterchat carries bytes past the name"));
    }
    Ok(name)
}

/// Builds a `SID_JOINCHANNEL` frame. An empty channel selects first-join
/// flags; a nonempty one selects no-create flags, following the pinned
/// sender. The channel is sent raw, without a terminator.
pub fn encode_joinchannel(channel: &[u8]) -> Result<Vec<u8>, BncsError> {
    if channel.contains(&0) {
        return Err(BncsError::Fields("channel holds a terminator byte"));
    }
    if channel.len() > 64 {
        return Err(BncsError::Fields("channel exceeds the 64-byte caller ceiling"));
    }
    let mut body = Vec::with_capacity(4 + channel.len());
    body.extend_from_slice(
        &(if channel.is_empty() {
            JOIN_FIRST
        } else {
            JOIN_NO_CREATE
        })
        .to_le_bytes(),
    );
    body.extend_from_slice(channel);
    Ok(encode(BNCS_FAMILY, SID_JOINCHANNEL, &body)?)
}

/// Builds a `SID_CHATCOMMAND` frame from raw command bytes, without a
/// terminator and capped at one realm line.
pub fn encode_chatcommand(command: &[u8]) -> Result<Vec<u8>, BncsError> {
    if command.is_empty() || command.contains(&0) {
        return Err(BncsError::Fields("chat command is empty or NUL-bearing"));
    }
    if command.len() > 255 {
        return Err(BncsError::Fields("chat command exceeds one 255-byte line"));
    }
    Ok(encode(BNCS_FAMILY, SID_CHATCOMMAND, command)?)
}

/// A parsed `SID_CHATEVENT` body, borrowed from the frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChatEvent<'a> {
    /// Raw event id; classify with [`ChatEventKind::of`].
    pub event: u32,
    /// Four reserved bytes after the event id, preserved raw.
    pub reserved: &'a [u8; 4],
    /// Ping word, signed in the pinned receiver.
    pub ping: i32,
    /// Twelve reserved bytes after the ping, preserved raw.
    pub reserved2: &'a [u8; 12],
    /// User bytes without the terminator.
    pub user: &'a [u8],
    /// Message bytes without the terminator.
    pub message: &'a [u8],
}

/// A classified chat event id. Unknown ids stay raw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatEventKind {
    /// Another user shown on channel join.
    ShowUser,
    /// A user joined the channel.
    Join,
    /// A user left the channel.
    Leave,
    /// An incoming whisper.
    Whisper,
    /// Channel talk.
    Talk,
    /// Server broadcast.
    Broadcast,
    /// Channel metadata on join.
    Channel,
    /// User flags update.
    UserFlags,
    /// Own whisper echo.
    WhisperSent,
    /// Channel is full.
    ChannelFull,
    /// Channel does not exist.
    ChannelDoesNotExist,
    /// Channel is restricted.
    ChannelRestricted,
    /// Information message.
    Info,
    /// Error message.
    Error,
    /// Emote.
    Emote,
    /// Any other event id, retained raw.
    Unknown(u32),
}

impl ChatEventKind {
    /// Classifies a `SID_CHATEVENT` event id.
    pub fn of(event: u32) -> Self {
        match event {
            EID_SHOWUSER => Self::ShowUser,
            EID_JOIN => Self::Join,
            EID_LEAVE => Self::Leave,
            EID_WHISPER => Self::Whisper,
            EID_TALK => Self::Talk,
            EID_BROADCAST => Self::Broadcast,
            EID_CHANNEL => Self::Channel,
            EID_USERFLAGS => Self::UserFlags,
            EID_WHISPERSENT => Self::WhisperSent,
            EID_CHANNELFULL => Self::ChannelFull,
            EID_CHANNELDOESNOTEXIST => Self::ChannelDoesNotExist,
            EID_CHANNELRESTRICTED => Self::ChannelRestricted,
            EID_INFO => Self::Info,
            EID_ERROR => Self::Error,
            EID_EMOTE => Self::Emote,
            other => Self::Unknown(other),
        }
    }
}

/// Reads a `SID_CHATEVENT` frame, accepting every pinned event id.
pub fn decode_chatevent(
    family: u8,
    id: u8,
    body: &[u8],
) -> Result<ChatEvent<'_>, BncsError> {
    let body = check(family, id, SID_CHATEVENT, body)?;
    if body.len() < 25 {
        return Err(BncsError::Malformed(
            "chatevent is shorter than ids, ping, reserved words and a name",
        ));
    }
    let event = u32::from_le_bytes(body[0..4].try_into().expect("checked length"));
    if ChatEventKind::of(event) == ChatEventKind::Unknown(event) {
        return Err(BncsError::Malformed("chatevent id is not a pinned event"));
    }
    let reserved: &[u8; 4] = body[4..8].try_into().expect("checked length");
    let ping = i32::from_le_bytes(body[8..12].try_into().expect("checked length"));
    let reserved2: &[u8; 12] = body[12..24].try_into().expect("checked length");
    let (user, rest) = cstring(&body[24..], "unterminated chatevent user")?;
    let (message, tail) = cstring(rest, "unterminated chatevent message")?;
    if !tail.is_empty() {
        return Err(BncsError::Malformed("chatevent carries bytes past the message"));
    }
    Ok(ChatEvent {
        event,
        reserved,
        ping,
        reserved2,
        user,
        message,
    })
}

impl<'a> Frame<'a> {
    /// Reads a `SID_CHATEVENT` body borrowed from this frame.
    pub fn bncs_chatevent(&self) -> Result<ChatEvent<'a>, BncsError> {
        decode_chatevent(self.family(), self.id(), self.body())
    }
}

impl OwnedFrame {
    /// Reads a `SID_CHATEVENT` body borrowed from this retained frame.
    pub fn bncs_chatevent(&self) -> Result<ChatEvent<'_>, BncsError> {
        decode_chatevent(self.family(), self.id(), self.body())
    }
}

/// Caller-supplied `SID_AUTH_INFO` request fields.
#[derive(Debug, Clone, Copy)]
pub struct AuthInfoRequest<'a> {
    /// Protocol version byte (war3 builds use 1 in the pinned sender's call
    /// sites; preserved as supplied, never defaulted here).
    pub ver: u8,
    /// True for The Frozen Throne (`W3XP`), false for Reign of Chaos.
    pub tft: bool,
    /// Locale id, sent twice (locale and language).
    pub locale_id: u32,
    /// Country abbreviation bytes, no terminator.
    pub country_abbrev: &'a [u8],
    /// Country bytes, no terminator.
    pub country: &'a [u8],
}

/// Builds a `SID_AUTH_INFO` request. Language stays `enUS`, local IP
/// `127.0.0.1` and timezone bias 300, following the pinned sender; the
/// caller selects only version, product, locale and country.
pub fn encode_auth_info(req: &AuthInfoRequest<'_>) -> Result<Vec<u8>, BncsError> {
    if req.country_abbrev.contains(&0) || req.country.contains(&0) {
        return Err(BncsError::Fields("country fields must be unterminated"));
    }
    if req.country_abbrev.len() > 16 || req.country.len() > 64 {
        return Err(BncsError::Fields("country fields exceed caller ceilings"));
    }
    let mut body = Vec::with_capacity(48 + req.country_abbrev.len() + req.country.len());
    body.extend_from_slice(&[0, 0, 0, 0]); // protocol id
    body.extend_from_slice(b"IX86");
    body.extend_from_slice(if req.tft { b"W3XP" } else { b"WAR3" });
    body.extend_from_slice(&[req.ver, 0, 0, 0]);
    body.extend_from_slice(b"enUS");
    body.extend_from_slice(&[127, 0, 0, 1]);
    body.extend_from_slice(&300u32.to_le_bytes());
    body.extend_from_slice(&req.locale_id.to_le_bytes());
    body.extend_from_slice(&req.locale_id.to_le_bytes());
    body.extend_from_slice(req.country_abbrev);
    body.extend_from_slice(req.country);
    Ok(encode(BNCS_FAMILY, SID_AUTH_INFO, &body)?)
}

/// A parsed `SID_AUTH_INFO` server reply, borrowed from the frame.
/// Mirrors `RECEIVE_SID_AUTH_INFO`: logon type, server token, MPQ time and
/// the selected version-check filename/equation bytes. The equation is
/// compared by the caller against the selected profile; it is never
/// executed here. See also `auth_info.rs` for the PvPGN-profile reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthInfoReply<'a> {
    /// Logon type word, preserved raw.
    pub logon_type: [u8; 4],
    /// Server token bytes.
    pub server_token: [u8; 4],
    /// Four reserved bytes, preserved raw.
    pub reserved: [u8; 4],
    /// MPQ file-time bytes.
    pub mpq_file_time: [u8; 8],
    /// Version-check filename bytes without the terminator.
    pub ver_file_name: &'a [u8],
    /// Checksum-equation bytes without the terminator.
    pub value_string_formula: &'a [u8],
}

/// Reads a `SID_AUTH_INFO` server reply.
pub fn decode_auth_info(
    family: u8,
    id: u8,
    body: &[u8],
) -> Result<AuthInfoReply<'_>, BncsError> {
    let body = check(family, id, SID_AUTH_INFO, body)?;
    if body.len() < 21 {
        return Err(BncsError::Malformed(
            "auth-info is shorter than tokens, file time and a filename",
        ));
    }
    let (ver_file_name, rest) = cstring(&body[20..], "unterminated auth-info filename")?;
    let (formula, tail) = cstring(rest, "unterminated auth-info equation")?;
    if !tail.is_empty() {
        return Err(BncsError::Malformed("auth-info carries bytes past the equation"));
    }
    Ok(AuthInfoReply {
        logon_type: body[0..4].try_into().expect("checked length"),
        server_token: body[4..8].try_into().expect("checked length"),
        reserved: body[8..12].try_into().expect("checked length"),
        mpq_file_time: body[12..20].try_into().expect("checked length"),
        ver_file_name,
        value_string_formula: formula,
    })
}

/// Caller-supplied `SID_AUTH_CHECK` request fields. Key and hash bytes come
/// from BNCSUtil-style version/key computation, which is not implemented
/// here: this builds the frame around caller-supplied material and refuses
/// to derive any of it.
#[derive(Debug, Clone, Copy)]
pub struct AuthCheckRequest<'a> {
    /// True for The Frozen Throne (two key infos), false for ROC (one).
    pub tft: bool,
    /// Four client-token bytes.
    pub client_token: [u8; 4],
    /// Four EXE-version bytes.
    pub exe_version: [u8; 4],
    /// Four EXE-version-hash bytes.
    pub exe_version_hash: [u8; 4],
    /// Exactly 36 ROC key-info bytes.
    pub key_info_roc: &'a [u8],
    /// Exactly 36 TFT key-info bytes when `tft` is set.
    pub key_info_tft: Option<&'a [u8]>,
    /// EXE-info bytes (version string and friends), raw.
    pub exe_info: &'a [u8],
    /// CD-key owner name bytes, raw.
    pub key_owner_name: &'a [u8],
}

/// Builds a `SID_AUTH_CHECK` frame around caller-supplied key material.
pub fn encode_auth_check(req: &AuthCheckRequest<'_>) -> Result<Vec<u8>, BncsError> {
    if req.key_info_roc.len() != 36 {
        return Err(BncsError::Fields("ROC key info is not 36 bytes"));
    }
    if req.tft != req.key_info_tft.is_some() {
        return Err(BncsError::Fields(
            "TFT mode needs TFT key info and ROC mode forbids it",
        ));
    }
    if let Some(tft) = req.key_info_tft
        && tft.len() != 36
    {
        return Err(BncsError::Fields("TFT key info is not 36 bytes"));
    }
    if req.exe_info.contains(&0) || req.key_owner_name.contains(&0) {
        return Err(BncsError::Fields("exe info and owner name must be unterminated"));
    }
    if req.exe_info.len() > 256 || req.key_owner_name.len() > 64 {
        return Err(BncsError::Fields("exe info or owner name exceeds caller ceilings"));
    }
    let mut body = Vec::with_capacity(60 + req.exe_info.len() + req.key_owner_name.len());
    body.extend_from_slice(&req.client_token);
    body.extend_from_slice(&req.exe_version);
    body.extend_from_slice(&req.exe_version_hash);
    body.extend_from_slice(&(if req.tft { 2u32 } else { 1u32 }).to_le_bytes());
    body.extend_from_slice(&0u32.to_le_bytes()); // spawn flag: false
    body.extend_from_slice(req.key_info_roc);
    if let Some(tft) = req.key_info_tft {
        body.extend_from_slice(tft);
    }
    body.extend_from_slice(req.exe_info);
    body.extend_from_slice(req.key_owner_name);
    Ok(encode(BNCS_FAMILY, SID_AUTH_CHECK, &body)?)
}

/// Classifies a `SID_AUTH_CHECK` key-state word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyState {
    /// `KR_GOOD`: the realm accepted the key material.
    Good,
    /// `KR_OLD_GAME_VERSION`.
    OldGameVersion,
    /// `KR_INVALID_VERSION`.
    InvalidVersion,
    /// `KR_ROC_KEY_IN_USE`.
    RocKeyInUse,
    /// `KR_TFT_KEY_IN_USE`.
    TftKeyInUse,
    /// Any other state, retained raw with its description.
    Unknown(u32),
}

impl KeyState {
    /// Classifies a key-state word.
    pub fn of(state: u32) -> Self {
        match state {
            KEY_GOOD => Self::Good,
            KEY_OLD_GAME_VERSION => Self::OldGameVersion,
            KEY_INVALID_VERSION => Self::InvalidVersion,
            KEY_ROC_IN_USE => Self::RocKeyInUse,
            KEY_TFT_IN_USE => Self::TftKeyInUse,
            other => Self::Unknown(other),
        }
    }
}

/// A parsed `SID_AUTH_CHECK` reply: the classified state plus the realm's
/// description bytes without the terminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthCheckReply<'a> {
    /// Classified key state.
    pub state: KeyState,
    /// Raw key-state word.
    pub raw: u32,
    /// Realm description bytes.
    pub description: &'a [u8],
}

/// Reads a `SID_AUTH_CHECK` reply.
pub fn decode_auth_check(
    family: u8,
    id: u8,
    body: &[u8],
) -> Result<AuthCheckReply<'_>, BncsError> {
    let body = check(family, id, SID_AUTH_CHECK, body)?;
    if body.len() < 5 {
        return Err(BncsError::Malformed("auth-check is shorter than state and a description"));
    }
    let raw = u32::from_le_bytes(body[0..4].try_into().expect("checked length"));
    let (description, tail) = cstring(&body[4..], "unterminated auth-check description")?;
    if !tail.is_empty() {
        return Err(BncsError::Malformed("auth-check carries bytes past the description"));
    }
    Ok(AuthCheckReply {
        state: KeyState::of(raw),
        raw,
        description,
    })
}

/// Builds a `SID_AUTH_ACCOUNTLOGON` frame around a caller-supplied 32-byte
/// client public key. The NLS proof math is not implemented here.
pub fn encode_account_logon(client_key: &[u8; 32], account: &[u8]) -> Result<Vec<u8>, BncsError> {
    if account.is_empty() || account.contains(&0) {
        return Err(BncsError::Fields("account name is empty or NUL-bearing"));
    }
    if account.len() > 32 {
        return Err(BncsError::Fields("account name exceeds the 32-byte ceiling"));
    }
    let mut body = Vec::with_capacity(32 + account.len());
    body.extend_from_slice(client_key);
    body.extend_from_slice(account);
    Ok(encode(BNCS_FAMILY, SID_AUTH_ACCOUNTLOGON, &body)?)
}

/// A parsed `SID_AUTH_ACCOUNTLOGON` reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccountLogonReply<'a> {
    /// Raw status word: 0 carries salt and server key.
    pub status: u32,
    /// 32 salt bytes, present only when `status == 0`.
    pub salt: Option<&'a [u8; 32]>,
    /// 32 server-public-key bytes, present only when `status == 0`.
    pub server_key: Option<&'a [u8; 32]>,
}

/// Reads a `SID_AUTH_ACCOUNTLOGON` reply.
pub fn decode_account_logon(
    family: u8,
    id: u8,
    body: &[u8],
) -> Result<AccountLogonReply<'_>, BncsError> {
    let body = check(family, id, SID_AUTH_ACCOUNTLOGON, body)?;
    if body.len() < 4 {
        return Err(BncsError::Malformed("account-logon is shorter than its status"));
    }
    let status = u32::from_le_bytes(body[0..4].try_into().expect("checked length"));
    if status != 0 {
        if body.len() != 4 {
            return Err(BncsError::Malformed(
                "failed account-logon carries bytes past its status",
            ));
        }
        return Ok(AccountLogonReply {
            status,
            salt: None,
            server_key: None,
        });
    }
    if body.len() != 72 {
        return Err(BncsError::Malformed(
            "accepted account-logon is not status, salt and server key",
        ));
    }
    Ok(AccountLogonReply {
        status,
        salt: Some(body[4..36].try_into().expect("checked length")),
        server_key: Some(body[36..72].try_into().expect("checked length")),
    })
}

/// Builds a `SID_AUTH_ACCOUNTLOGONPROOF` frame around a caller-supplied
/// 20-byte password proof. The NLS proof math is not implemented here.
pub fn encode_account_logon_proof(proof: &[u8; 20]) -> Result<Vec<u8>, BncsError> {
    Ok(encode(BNCS_FAMILY, SID_AUTH_ACCOUNTLOGONPROOF, proof)?)
}

/// Reads a `SID_AUTH_ACCOUNTLOGONPROOF` reply: status 0 or 0x0E accepts.
pub fn decode_account_logon_proof(family: u8, id: u8, body: &[u8]) -> Result<bool, BncsError> {
    let body = check(family, id, SID_AUTH_ACCOUNTLOGONPROOF, body)?;
    if body.len() != 4 {
        return Err(BncsError::Malformed("account-logon-proof is not the status word"));
    }
    Ok(matches!(
        u32::from_le_bytes(body.try_into().expect("checked length")),
        0 | 0x0E
    ))
}

/// Builds a `SID_LOGONRESPONSE` frame around caller-supplied tokens and
/// hash. The old-style password hash needs the session key; it is never
/// derived here.
pub fn encode_logon_response(
    client_token: [u8; 4],
    server_token: [u8; 4],
    password_hash: &[u8],
    account: &[u8],
) -> Result<Vec<u8>, BncsError> {
    if password_hash.is_empty() || password_hash.len() > 64 {
        return Err(BncsError::Fields("password hash is empty or overlong"));
    }
    if account.is_empty() || account.contains(&0) || account.len() > 32 {
        return Err(BncsError::Fields("account name is empty, NUL-bearing or overlong"));
    }
    let mut body = Vec::with_capacity(8 + password_hash.len() + account.len());
    body.extend_from_slice(&client_token);
    body.extend_from_slice(&server_token);
    body.extend_from_slice(password_hash);
    body.extend_from_slice(account);
    Ok(encode(BNCS_FAMILY, SID_LOGONRESPONSE, &body)?)
}

/// Reads a `SID_LOGONRESPONSE` reply: status 1 accepts.
pub fn decode_logon_response(family: u8, id: u8, body: &[u8]) -> Result<bool, BncsError> {
    let body = check(family, id, SID_LOGONRESPONSE, body)?;
    if body.len() != 4 {
        return Err(BncsError::Malformed("logon-response is not the status word"));
    }
    Ok(u32::from_le_bytes(body.try_into().expect("checked length")) == 1)
}

/// Builds a `SID_NETGAMEPORT` frame with the local game-server port.
pub fn encode_netgameport(port: u16) -> Result<Vec<u8>, BncsError> {
    Ok(encode(BNCS_FAMILY, SID_NETGAMEPORT, &port.to_le_bytes())?)
}

/// Reads a `SID_NETGAMEPORT` body.
pub fn decode_netgameport(family: u8, id: u8, body: &[u8]) -> Result<u16, BncsError> {
    let body = check(family, id, SID_NETGAMEPORT, body)?;
    if body.len() != 2 {
        return Err(BncsError::Malformed("netgameport is not the 2-byte port"));
    }
    Ok(u16::from_le_bytes(body.try_into().expect("checked length")))
}

/// Caller-supplied `SID_STARTADVEX3` advertisement fields.
#[derive(Debug, Clone, Copy)]
pub struct GameAdvertisement<'a> {
    /// `ADVEX_PUBLIC`, `ADVEX_PRIVATE` or `ADVEX_CLOSE`.
    pub state: u32,
    /// Seconds since creation, as the host reports it.
    pub uptime: u32,
    /// Four game-type bytes, wire order.
    pub map_game_type: [u8; 4],
    /// Four map-flag bytes, wire order.
    pub map_flags: [u8; 4],
    /// Map width bytes, wire order.
    pub map_width: [u8; 2],
    /// Map height bytes, wire order.
    pub map_height: [u8; 2],
    /// Game name bytes, nonempty.
    pub game_name: &'a [u8],
    /// Host name bytes, nonempty.
    pub host_name: &'a [u8],
    /// Map path bytes, nonempty.
    pub map_path: &'a [u8],
    /// Four map-CRC bytes.
    pub map_crc: [u8; 4],
    /// Twenty map-SHA1 bytes.
    pub map_sha1: [u8; 20],
    /// Raw host counter.
    pub host_counter: u32,
    /// Free lobby slots. The pinned sender hard-codes 98 (`'b'`); preserved
    /// as a caller field with that default documented, never derived here.
    pub slots_free: u8,
}

/// Builds a `SID_STARTADVEX3` frame. The host counter is sent as eight
/// reversed ASCII hex digits, following the pinned sender exactly.
pub fn encode_startadvex(ad: &GameAdvertisement<'_>) -> Result<Vec<u8>, BncsError> {
    if !matches!(ad.state, ADVEX_PUBLIC | ADVEX_PRIVATE | ADVEX_CLOSE) {
        return Err(BncsError::Fields("advertise state is not public, private or close"));
    }
    for (field, bytes) in [
        ("game name", ad.game_name),
        ("host name", ad.host_name),
        ("map path", ad.map_path),
    ] {
        if bytes.is_empty() || bytes.contains(&0) {
            return Err(BncsError::Fields(match field {
                "game name" => "game name is empty or NUL-bearing",
                "host name" => "host name is empty or NUL-bearing",
                _ => "map path is empty or NUL-bearing",
            }));
        }
    }
    if ad.game_name.len() > 31 || ad.host_name.len() > 31 || ad.map_path.len() > 255 {
        return Err(BncsError::Fields("advertise names exceed caller ceilings"));
    }
    let mut stat = Vec::new();
    stat.extend_from_slice(&ad.map_flags);
    stat.push(0);
    stat.extend_from_slice(&ad.map_width);
    stat.extend_from_slice(&ad.map_height);
    stat.extend_from_slice(&ad.map_crc);
    stat.extend_from_slice(ad.map_path);
    stat.extend_from_slice(ad.host_name);
    stat.push(0);
    stat.extend_from_slice(&ad.map_sha1);
    let stat = super::lobby::encode_stat_string(&stat);
    if stat.len() >= 128 {
        return Err(BncsError::Fields("encoded stat string exceeds 127 bytes"));
    }
    let hexed = format!("{:08x}", ad.host_counter);
    let reversed: Vec<u8> = hexed.bytes().rev().collect();
    let mut body = Vec::with_capacity(32 + ad.game_name.len() + stat.len());
    body.extend_from_slice(&ad.state.to_le_bytes());
    body.extend_from_slice(&ad.uptime.to_le_bytes());
    body.extend_from_slice(&ad.map_game_type);
    body.extend_from_slice(&[255, 3, 0, 0]);
    body.extend_from_slice(&[0, 0, 0, 0]);
    body.extend_from_slice(ad.game_name);
    body.push(0);
    body.push(ad.slots_free);
    body.extend_from_slice(&reversed);
    body.extend_from_slice(&stat);
    body.push(0);
    Ok(encode(BNCS_FAMILY, SID_STARTADVEX3, &body)?)
}

/// Reads a `SID_STARTADVEX3` reply: status 0 accepts.
pub fn decode_startadvex(family: u8, id: u8, body: &[u8]) -> Result<bool, BncsError> {
    let body = check(family, id, SID_STARTADVEX3, body)?;
    if body.len() != 4 {
        return Err(BncsError::Malformed("startadvex reply is not the status word"));
    }
    Ok(u32::from_le_bytes(body.try_into().expect("checked length")) == 0)
}

/// Builds a `SID_STOPADV` frame. Empty body.
pub fn encode_stopadv() -> Result<Vec<u8>, BncsError> {
    Ok(encode(BNCS_FAMILY, SID_STOPADV, &[])?)
}

/// Builds a `SID_CHECKAD` frame: four reserved zero words, following the
/// pinned sender.
pub fn encode_checkad() -> Result<Vec<u8>, BncsError> {
    Ok(encode(BNCS_FAMILY, SID_CHECKAD, &[0; 16])?)
}

/// Reads a `SID_CHECKAD` reply. Empty body.
pub fn decode_checkad(family: u8, id: u8, body: &[u8]) -> Result<(), BncsError> {
    let body = check(family, id, SID_CHECKAD, body)?;
    if !body.is_empty() {
        return Err(BncsError::Malformed("checkad carries a body"));
    }
    Ok(())
}

/// Builds a `SID_GETADVLISTEX` frame listing games matching `game_name`,
/// with the pinned sender's map filters and one-game ceiling.
pub fn encode_getadvlistex(game_name: &[u8]) -> Result<Vec<u8>, BncsError> {
    if game_name.contains(&0) {
        return Err(BncsError::Fields("game-name filter holds a terminator byte"));
    }
    if game_name.len() > 31 {
        return Err(BncsError::Fields("game-name filter exceeds the 31-byte ceiling"));
    }
    let mut body = Vec::with_capacity(18 + game_name.len());
    body.extend_from_slice(&[255, 3, 0, 0]);
    body.extend_from_slice(&[255, 3, 0, 0]);
    body.extend_from_slice(&[0, 0, 0, 0]);
    body.extend_from_slice(&1u32.to_le_bytes());
    body.extend_from_slice(game_name);
    body.extend_from_slice(&[0, 0]);
    Ok(encode(BNCS_FAMILY, SID_GETADVLISTEX, &body)?)
}

/// One advertised game host from a `SID_GETADVLISTEX` reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdvertisedHost<'a> {
    /// Server IPv4 address, network order.
    pub ip: [u8; 4],
    /// Server port, little-endian on the wire.
    pub port: u16,
    /// Game-name bytes without the terminator.
    pub game_name: &'a [u8],
    /// Four raw host-counter bytes in the sender's hex-swapped order.
    pub host_counter_raw: [u8; 4],
}

/// Reads a `SID_GETADVLISTEX` reply, retaining up to `max_hosts` advertised
/// hosts in reply order. A nonzero game count with no parseable host is
/// refused rather than reported as empty.
pub fn decode_getadvlistex(
    family: u8,
    id: u8,
    body: &[u8],
    max_hosts: usize,
) -> Result<Vec<AdvertisedHost<'_>>, BncsError> {
    let body = check(family, id, SID_GETADVLISTEX, body)?;
    if body.len() < 4 {
        return Err(BncsError::Malformed("advlistex is shorter than its game count"));
    }
    let found = u32::from_le_bytes(body[0..4].try_into().expect("checked length")) as usize;
    if found == 0 {
        if body.len() != 4 {
            return Err(BncsError::Malformed(
                "empty advlistex carries bytes past its game count",
            ));
        }
        return Ok(Vec::new());
    }
    if max_hosts == 0 {
        return Err(BncsError::Fields("host ceiling must admit one advertised host"));
    }
    let mut hosts = Vec::new();
    let mut at = 4;
    while hosts.len() < found.min(max_hosts) {
        // Ten reserved bytes, port, IP, then the game name.
        if body.len() < at + 10 + 2 + 4 + 1 {
            return Err(BncsError::Malformed("advlistex host is shorter than its header"));
        }
        at += 10;
        let port = u16::from_le_bytes(body[at..at + 2].try_into().expect("checked host"));
        let ip: [u8; 4] = body[at + 2..at + 6].try_into().expect("checked host");
        let (game_name, rest) = cstring(&body[at + 6..], "unterminated advlistex game name")?;
        if game_name.is_empty() {
            return Err(BncsError::Malformed("advlistex game name is empty"));
        }
        // Two reserved bytes, then the hex-swapped host counter.
        if rest.len() < 2 + 4 {
            return Err(BncsError::Malformed("advlistex host is shorter than its counter"));
        }
        hosts.push(AdvertisedHost {
            ip,
            port,
            game_name,
            host_counter_raw: rest[2..6].try_into().expect("checked counter"),
        });
        at = body.len() - rest.len() + 2 + 4;
    }
    if hosts.len() != found.min(max_hosts) {
        return Err(BncsError::Malformed("advlistex host count disagrees with its bodies"));
    }
    Ok(hosts)
}

/// Builds a `SID_NOTIFYJOIN` frame announcing interest in `game_name`.
/// Product id stays zero and product version 14, following the pinned sender.
pub fn encode_notifyjoin(game_name: &[u8]) -> Result<Vec<u8>, BncsError> {
    if game_name.is_empty() || game_name.contains(&0) {
        return Err(BncsError::Fields("notifyjoin game name is empty or NUL-bearing"));
    }
    if game_name.len() > 31 {
        return Err(BncsError::Fields("notifyjoin game name exceeds the 31-byte ceiling"));
    }
    let mut body = Vec::with_capacity(9 + game_name.len());
    body.extend_from_slice(&[0, 0, 0, 0]);
    body.extend_from_slice(&TFT_PRODUCT_VERSION.to_le_bytes());
    body.extend_from_slice(game_name);
    body.push(0);
    Ok(encode(BNCS_FAMILY, SID_NOTIFYJOIN, &body)?)
}

/// Refuses a `SID_WARDEN` payload. Warden is anti-cheat; it is never
/// answered and its bytes are never interpreted here.
pub fn refuse_warden(family: u8, id: u8, body: &[u8]) -> Result<(), BncsError> {
    let body = check(family, id, SID_WARDEN, body)?;
    if body.is_empty() {
        return Err(BncsError::Malformed("warden has no payload to refuse"));
    }
    Err(BncsError::Malformed(
        "warden payloads are refused: anti-cheat is never answered",
    ))
}

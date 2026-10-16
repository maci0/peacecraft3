//! W3GS LAN discovery bodies from pinned GHost++ `6ba90f426e96f80fe013a468d1e07151a510adbe`.
//!
//! The supplied original 1.29.2 host also appends data after the stat suffix.
//! Those bytes remain opaque. Product/version are selected by the caller, not
//! inferred from a map or a project advertisement. This is not player admission.

use super::{Frame, FrameError, OwnedFrame, W3GS_FAMILY};

/// Pinned `W3GS_SEARCHGAME` identifier.
pub const SEARCHGAME_ID: u8 = 47;
/// Pinned `W3GS_GAMEINFO` identifier.
pub const GAMEINFO_ID: u8 = 48;
/// Pinned `W3GS_REQJOIN` identifier.
pub const REQJOIN_ID: u8 = 30;

/// IPv4 tail of the pinned 1.29 join sender profile, without allocating.
///
/// GoWarcraft3 `bfda02e6ef6481fd41f4009f730ac7b7723d5883` `Join.Serialize`
/// and `WriteSockAddr`: two reserved bytes, then an IPv4 sockaddr with a
/// little-endian family and network-order port/address. Reserved bytes are
/// protocol zeros. The caller supplies the endpoint; this does not bind it,
/// infer a route, select a version, or establish peer reachability.
pub fn join_ipv4_tail_129(endpoint: std::net::SocketAddrV4) -> [u8; 19] {
    let mut tail = [0; 19];
    tail[0] = 2;
    tail[3..5].copy_from_slice(&2u16.to_le_bytes());
    tail[5..7].copy_from_slice(&endpoint.port().to_be_bytes());
    tail[7..11].copy_from_slice(&endpoint.ip().octets());
    tail
}

/// Explicit client fields for a join request; no native address defaults.
#[derive(Debug, Clone, Copy)]
pub struct JoinRequest<'a> {
    /// Unspecified byte before the listen port, preserved as supplied.
    pub unknown: u8,
    /// Client's listen port, encoded little-endian, not the host's port.
    pub listen_port: u16,
    /// Caller-selected peer key, not a project player id.
    pub peer_key: u32,
    /// Nonempty byte name without a NUL terminator; no UTF-8 conversion.
    pub player_name: &'a [u8],
    /// Source-profile bytes after the name terminator, kept opaquely.
    ///
    /// The pinned receiver requires at least ten bytes here. Original 1.29.2
    /// supplies a longer tail; this API does not infer its address layout.
    pub address_tail: &'a [u8],
}

/// Client-field or frame-size refusal before constructing a join request.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum JoinRequestError {
    /// Missing name, embedded terminator or an incomplete address tail.
    #[error("W3GS REQJOIN: {0}")]
    Fields(&'static str),
    /// Caller/wire size ceiling or allocation failure.
    #[error(transparent)]
    Frame(#[from] FrameError),
}

/// Caller-selected protocol identity; no edition mapping or default version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiscoveryProfile {
    /// Four product bytes in wire order.
    pub product: [u8; 4],
    /// Full little-endian version word, without truncating its high bytes.
    pub version: u32,
}

impl DiscoveryProfile {
    /// Build the pinned sender's 16-byte search request without allocating.
    pub fn search_request(self) -> [u8; 16] {
        let mut bytes = [0; 16];
        bytes[..4].copy_from_slice(&[W3GS_FAMILY, SEARCHGAME_ID, 16, 0]);
        bytes[4..8].copy_from_slice(&self.product);
        bytes[8..12].copy_from_slice(&self.version.to_le_bytes());
        bytes
    }
}

/// One checked advertisement; string bytes are not converted or normalized.
#[derive(Debug)]
pub struct GameInfo<'a> {
    /// Checked selected product/version.
    pub profile: DiscoveryProfile,
    /// Raw host counter, not a project peer id.
    pub host_counter: u32,
    /// Raw entry key, not authentication.
    pub entry_key: u32,
    /// Borrowed game name, without its terminator.
    pub game_name: &'a [u8],
    /// Byte following the game-name terminator; its meaning is not assigned.
    pub name_suffix: u8,
    /// Advertised total slots; never used to allocate records.
    pub slots_total: u32,
    /// Game-type bytes in source order.
    pub game_type: [u8; 4],
    /// Unspecified word in the fixed tail, without guessed semantics.
    pub tail_word: [u8; 4],
    /// Advertised open slots, independent of total slots.
    pub slots_open: u32,
    /// Advertised uptime word, not a local clock.
    pub uptime: u32,
    /// Advertised little-endian game port.
    pub port: u16,
    stat: Vec<u8>,
    path_end: usize,
    host_end: usize,
}

impl GameInfo<'_> {
    /// Encode REQJOIN using this checked advertisement's counter and entry key.
    ///
    /// All client fields remain explicit. This constructs bytes only; it does
    /// not open a socket, admit a player or compare the selected local map.
    pub fn join_request(
        &self,
        fields: JoinRequest<'_>,
        max_frame_bytes: usize,
    ) -> Result<Vec<u8>, JoinRequestError> {
        if max_frame_bytes < 4 {
            return Err(FrameError::Limit.into());
        }
        if fields.player_name.is_empty() || fields.player_name.contains(&0) {
            return Err(JoinRequestError::Fields(
                "nonempty unterminated name required",
            ));
        }
        if fields.address_tail.len() < 10 {
            return Err(JoinRequestError::Fields(
                "address tail is shorter than ten bytes",
            ));
        }
        let limit = max_frame_bytes.min(usize::from(u16::MAX));
        let len = 20usize
            .checked_add(fields.player_name.len())
            .and_then(|len| len.checked_add(fields.address_tail.len()))
            .ok_or(FrameError::Oversize {
                declared: usize::MAX,
                limit,
            })?;
        if len > limit {
            return Err(FrameError::Oversize {
                declared: len,
                limit,
            }
            .into());
        }
        let mut packet = Vec::new();
        packet
            .try_reserve(len)
            .map_err(|_| FrameError::Allocation)?;
        packet.extend([W3GS_FAMILY, REQJOIN_ID]);
        packet.extend(
            u16::try_from(len)
                .expect("checked frame length")
                .to_le_bytes(),
        );
        packet.extend(self.host_counter.to_le_bytes());
        packet.extend(self.entry_key.to_le_bytes());
        packet.push(fields.unknown);
        packet.extend(fields.listen_port.to_le_bytes());
        packet.extend(fields.peer_key.to_le_bytes());
        packet.extend(fields.player_name);
        packet.push(0);
        packet.extend(fields.address_tail);
        Ok(packet)
    }

    /// Map flags in decoded source order.
    pub fn map_flags(&self) -> [u8; 4] {
        self.stat[..4].try_into().expect("checked stat")
    }
    /// Unspecified byte between flags and dimensions.
    pub fn stat_reserved(&self) -> u8 {
        self.stat[4]
    }
    /// Stored width and height words; no world-coordinate scale is assigned.
    pub fn map_dimensions(&self) -> [u16; 2] {
        [
            u16::from_le_bytes(self.stat[5..7].try_into().expect("checked stat")),
            u16::from_le_bytes(self.stat[7..9].try_into().expect("checked stat")),
        ]
    }
    /// Map checksum bytes, preserved without verifying the selected map.
    pub fn map_crc(&self) -> [u8; 4] {
        self.stat[9..13].try_into().expect("checked stat")
    }
    /// Protocol path bytes; not an inferred resolver member or display label.
    pub fn map_path(&self) -> &[u8] {
        &self.stat[13..self.path_end]
    }
    /// Host-name bytes without their terminator.
    pub fn host_name(&self) -> &[u8] {
        &self.stat[self.path_end + 1..self.host_end]
    }
    /// Bytes after the pinned sender's final zero; original 1.29.2 adds data here.
    pub fn stat_extension(&self) -> &[u8] {
        &self.stat[self.host_end + 2..]
    }
}

/// A malformed, incompatible or over-budget discovery message.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DiscoveryError {
    /// Wrong packet shape or invalid stat encoding.
    #[error("W3GS discovery: {0}")]
    Packet(&'static str),
    /// Product/version differs from the caller-selected profile.
    #[error("W3GS discovery product/version mismatch")]
    Profile,
    /// Decoded bytes exceed the caller's explicit ceiling, before allocation.
    #[error("W3GS decoded stat length {bytes} exceeds limit {limit}")]
    StatBudget {
        /// Derived decoded size.
        bytes: usize,
        /// Caller ceiling.
        limit: usize,
    },
    /// Fallible decoded-buffer allocation refused.
    #[error("W3GS stat allocation failed")]
    Allocation,
}

impl<'a> Frame<'a> {
    /// Check the pinned sender's exact search shape and selected identity.
    pub fn check_search_game(&self, profile: DiscoveryProfile) -> Result<(), DiscoveryError> {
        check_search(self.family(), self.id(), self.body(), profile)
    }
    /// Read an advertisement with the selected profile and decoded-stat ceiling.
    pub fn game_info(
        &self,
        profile: DiscoveryProfile,
        max_stat_bytes: usize,
    ) -> Result<GameInfo<'a>, DiscoveryError> {
        parse(
            self.family(),
            self.id(),
            self.body(),
            profile,
            max_stat_bytes,
        )
    }
}

impl OwnedFrame {
    /// Same search validation for a retained frame.
    pub fn check_search_game(&self, profile: DiscoveryProfile) -> Result<(), DiscoveryError> {
        check_search(self.family(), self.id(), self.body(), profile)
    }
    /// Same reader for a retained datagram or streamed frame.
    pub fn game_info(
        &self,
        profile: DiscoveryProfile,
        max_stat_bytes: usize,
    ) -> Result<GameInfo<'_>, DiscoveryError> {
        parse(
            self.family(),
            self.id(),
            self.body(),
            profile,
            max_stat_bytes,
        )
    }
}

fn check_search(
    family: u8,
    id: u8,
    body: &[u8],
    profile: DiscoveryProfile,
) -> Result<(), DiscoveryError> {
    use DiscoveryError as E;
    if family != W3GS_FAMILY || id != SEARCHGAME_ID || body.len() != 12 {
        return Err(E::Packet("wrong search family, identifier or length"));
    }
    if body[..4] != profile.product || body[4..8] != profile.version.to_le_bytes() {
        return Err(E::Profile);
    }
    if body[8..] != [0; 4] {
        return Err(E::Packet("nonzero search suffix"));
    }
    Ok(())
}

fn cstring<'a>(bytes: &mut &'a [u8]) -> Result<&'a [u8], DiscoveryError> {
    let end = bytes
        .iter()
        .position(|&byte| byte == 0)
        .ok_or(DiscoveryError::Packet("unterminated string"))?;
    let (text, rest) = bytes.split_at(end);
    *bytes = &rest[1..];
    Ok(text)
}

fn parse<'a>(
    family: u8,
    id: u8,
    body: &'a [u8],
    profile: DiscoveryProfile,
    max_stat_bytes: usize,
) -> Result<GameInfo<'a>, DiscoveryError> {
    use DiscoveryError as E;
    if family != W3GS_FAMILY || id != GAMEINFO_ID {
        return Err(E::Packet("wrong family or identifier"));
    }
    let (head, mut rest) = body
        .split_first_chunk::<16>()
        .ok_or(E::Packet("short header"))?;
    if head[..4] != profile.product
        || u32::from_le_bytes(head[4..8].try_into().expect("checked header")) != profile.version
    {
        return Err(E::Profile);
    }
    let game_name = cstring(&mut rest)?;
    if game_name.is_empty() {
        return Err(E::Packet("empty game name"));
    }
    let (&name_suffix, after_name) = rest.split_first().ok_or(E::Packet("missing name suffix"))?;
    rest = after_name;
    let encoded = cstring(&mut rest)?;
    let tail: &[u8; 22] = rest
        .try_into()
        .map_err(|_| E::Packet("wrong fixed tail length"))?;
    if encoded.len() % 8 == 1 || encoded.iter().any(|byte| byte & 1 == 0) {
        return Err(E::Packet("invalid stat encoding"));
    }
    let bytes = encoded.len() - encoded.len().div_ceil(8);
    if bytes < 18 {
        return Err(E::Packet("short decoded stat"));
    }
    if bytes > max_stat_bytes {
        return Err(E::StatBudget {
            bytes,
            limit: max_stat_bytes,
        });
    }
    let mut stat = Vec::new();
    stat.try_reserve_exact(bytes).map_err(|_| E::Allocation)?;
    for group in encoded.chunks(8) {
        let mask = group[0];
        for (index, &byte) in group[1..].iter().enumerate() {
            stat.push(if mask & (1 << (index + 1)) == 0 {
                byte - 1
            } else {
                byte
            });
        }
    }
    let path_end = 13
        + stat[13..]
            .iter()
            .position(|&byte| byte == 0)
            .ok_or(E::Packet("unterminated map path"))?;
    let host_start = path_end + 1;
    let host_end = host_start
        + stat[host_start..]
            .iter()
            .position(|&byte| byte == 0)
            .ok_or(E::Packet("unterminated host name"))?;
    if path_end == 13 || host_end == host_start {
        return Err(E::Packet("empty map path or host name"));
    }
    if stat.get(host_end + 1) != Some(&0) {
        return Err(E::Packet("missing stat suffix"));
    }
    let word = |at| u32::from_le_bytes(tail[at..at + 4].try_into().expect("checked tail"));
    Ok(GameInfo {
        profile,
        host_counter: u32::from_le_bytes(head[8..12].try_into().expect("checked header")),
        entry_key: u32::from_le_bytes(head[12..16].try_into().expect("checked header")),
        game_name,
        name_suffix,
        slots_total: word(0),
        game_type: tail[4..8].try_into().expect("checked tail"),
        tail_word: tail[8..12].try_into().expect("checked tail"),
        slots_open: word(12),
        uptime: word(16),
        port: u16::from_le_bytes(tail[20..].try_into().expect("checked tail")),
        stat,
        path_end,
        host_end,
    })
}

#[cfg(not(target_arch = "wasm32"))]
pub mod udp;

#[cfg(test)]
mod tests;

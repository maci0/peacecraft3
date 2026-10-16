//! W3GS LAN discovery bodies from pinned GHost++ `6ba90f426e96f80fe013a468d1e07151a510adbe`.
//!
//! The supplied original 1.29.2 host also appends data after the stat suffix.
//! Those bytes remain opaque. Product/version are selected by the caller, not
//! inferred from a map or a project advertisement. This is not player admission.

use super::{Frame, OwnedFrame, W3GS_FAMILY};

/// Pinned `W3GS_SEARCHGAME` identifier.
pub const SEARCHGAME_ID: u8 = 47;
/// Pinned `W3GS_GAMEINFO` identifier.
pub const GAMEINFO_ID: u8 = 48;

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

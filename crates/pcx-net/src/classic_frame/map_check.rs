//! Selected-map fingerprint checks for GHost++ `6ba90f426e96f80fe013a468d1e07151a510adbe`.
//!
//! `SEND_W3GS_MAPCHECK` frames the path and fingerprints; `CMap::Load` defines
//! the member order and script overrides. SHA-1 is protocol compatibility data,
//! not authentication. A match is not a download, lobby or simulation readiness.

use super::{Frame, OwnedFrame, W3GS_FAMILY};
use pcx_mpq::checked::{CheckedArchive, ReadBudget};
use sha1::{Digest, Sha1};

/// Pinned `CGameProtocol::W3GS_MAPCHECK` identifier.
pub const MAPCHECK_ID: u8 = 61;

/// Borrowed caller-selected bytes with their edition/version and resolver source.
#[derive(Debug, Clone, Copy)]
pub struct SelectedBytes<'a> {
    /// Explicit edition, not inferred from a path.
    pub edition: &'a str,
    /// Explicit version or source-qualified version label.
    pub version: &'a str,
    /// Resolver source identity, not the W3GS path.
    pub source: &'a str,
    /// The exact selected payload.
    pub bytes: &'a [u8],
}

/// Views into the caller's existing map and script bindings; no payload is cloned.
#[derive(Debug, Clone, Copy)]
pub struct SelectedMap<'a> {
    /// Caller-selected wire path, compared byte for byte, without its terminator.
    pub protocol_path: &'a [u8],
    /// Selected complete map archive, including any container prefix/trailer.
    pub map: SelectedBytes<'a>,
    /// Fallback only when `Scripts\common.j` is absent or empty in the map.
    pub common: SelectedBytes<'a>,
    /// Fallback only when `Scripts\blizzard.j` is absent or empty in the map.
    pub blizzard: SelectedBytes<'a>,
}

/// Explicit allocation and total hashing ceilings, not protocol defaults.
#[derive(Debug, Clone, Copy)]
pub struct MapCheckBudget {
    /// Archive/table/packed/member limits for the shared checked MPQ reader.
    /// `max_archive_bytes` also bounds the complete selected map file.
    pub archive: ReadBudget,
    /// Sum of whole-file CRC input and every script/member hashed into the mix.
    pub max_hashed_bytes: usize,
}

/// Different protocol algorithms, never substituted with project FNV/SHA-256.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fingerprints {
    /// Whole-file size, including any prefix/trailer.
    pub size: u32,
    /// ISO-HDLC CRC-32 of the whole file (`map_info`).
    pub crc32: u32,
    /// GHost++ member XOR/rotate mix (`map_crc`).
    pub xoro: u32,
    /// SHA-1 of the selected script/member stream, not the whole file.
    pub sha1: [u8; 20],
}

/// Successful comparison with the original caller-selected map bytes.
#[derive(Debug, Clone, Copy)]
pub struct MapMatch<'a> {
    map: SelectedBytes<'a>,
    protocol_path: &'a [u8],
    prefix: [u8; 4],
    fingerprints: Fingerprints,
    map_preludes: [bool; 2],
}

impl<'a> MapMatch<'a> {
    /// Exact selected map source and bytes.
    pub fn map(&self) -> SelectedBytes<'a> {
        self.map
    }
    /// Exact caller-supplied W3GS path.
    pub fn protocol_path(&self) -> &'a [u8] {
        self.protocol_path
    }
    /// Four prefix bytes the pinned sender labels unknown, preserved without meaning.
    pub fn prefix(&self) -> [u8; 4] {
        self.prefix
    }
    /// Independently computed local fingerprints equal to the received fields.
    pub fn fingerprints(&self) -> Fingerprints {
        self.fingerprints
    }
    /// Whether common.j and blizzard.j respectively came from the map overrides.
    pub fn map_preludes(&self) -> [bool; 2] {
        self.map_preludes
    }
}

/// Packet, selected-source or checksum refusal; none denotes a joined player.
#[derive(Debug, thiserror::Error)]
pub enum MapCheckError {
    /// Wrong family/identifier, truncated fields or malformed path/tail.
    #[error("W3GS MAPCHECK: {0}")]
    Packet(&'static str),
    /// Caller did not supply the required selected identity or bytes.
    #[error("selected map check source: {0}")]
    Selection(&'static str),
    /// Protocol map path is independent from diagnostic/resolver labels.
    #[error("W3GS map path differs from the selected protocol path")]
    PathMismatch,
    /// A protocol fingerprint differs; later checks are not performed.
    #[error("W3GS selected map {0} mismatch")]
    Mismatch(&'static str),
    /// Explicit total hashing bound exceeded before the next read/hash.
    #[error("selected map hashing exceeds caller byte limit")]
    HashBudget,
    /// The shared reader retains the source and member diagnostic.
    #[error(transparent)]
    Archive(#[from] pcx_mpq::checked::ReadError),
    /// The shared map parser validates container prefix/archive/trailer framing.
    #[error("selected map {map}: {error}")]
    Map {
        /// Caller-selected resolver identity.
        map: String,
        /// Shared container framing error.
        #[source]
        error: pcx_map::Error,
    },
}

impl Frame<'_> {
    /// Compare this MAPCHECK with exact selected map bytes and applicable preludes.
    pub fn bind_selected_map<'a>(
        &self,
        selected: SelectedMap<'a>,
        budget: MapCheckBudget,
    ) -> Result<MapMatch<'a>, MapCheckError> {
        bind(self.family(), self.id(), self.body(), selected, budget)
    }
}

impl OwnedFrame {
    /// Same selected-map check for frames emitted by the stream/TCP decoder.
    pub fn bind_selected_map<'a>(
        &self,
        selected: SelectedMap<'a>,
        budget: MapCheckBudget,
    ) -> Result<MapMatch<'a>, MapCheckError> {
        bind(self.family(), self.id(), self.body(), selected, budget)
    }
}

fn bind<'a>(
    family: u8,
    id: u8,
    body: &[u8],
    selected: SelectedMap<'a>,
    budget: MapCheckBudget,
) -> Result<MapMatch<'a>, MapCheckError> {
    use MapCheckError as E;
    if family != W3GS_FAMILY || id != MAPCHECK_ID {
        return Err(E::Packet("wrong family or identifier"));
    }
    let (prefix, rest) = body
        .split_first_chunk::<4>()
        .ok_or(E::Packet("short prefix"))?;
    let end = rest
        .iter()
        .position(|&byte| byte == 0)
        .ok_or(E::Packet("unterminated path"))?;
    let (path, tail) = rest.split_at(end);
    let tail: &[u8; 32] = tail[1..]
        .try_into()
        .map_err(|_| E::Packet("wrong fingerprint tail length"))?;
    if path.is_empty() {
        return Err(E::Packet("empty map path"));
    }
    if selected.protocol_path.is_empty() || selected.protocol_path.contains(&0) {
        return Err(E::Selection("empty or terminated protocol path"));
    }
    if path != selected.protocol_path {
        return Err(E::PathMismatch);
    }
    identity(selected.map)?;
    if selected.map.bytes.is_empty() {
        return Err(E::Selection("map bytes are absent"));
    }
    if selected.map.bytes.len() > budget.archive.max_archive_bytes {
        return Err(E::Selection("map bytes exceed archive budget"));
    }
    let word = |at| u32::from_le_bytes(tail[at..at + 4].try_into().expect("checked tail"));
    let expected = Fingerprints {
        size: word(0),
        crc32: word(4),
        xoro: word(8),
        sha1: tail[12..].try_into().expect("checked tail"),
    };
    let size = u32::try_from(selected.map.bytes.len())
        .map_err(|_| E::Selection("map size exceeds protocol word"))?;
    if size != expected.size {
        return Err(E::Mismatch("size"));
    }
    let mut remaining = budget.max_hashed_bytes;
    charge(&mut remaining, selected.map.bytes.len())?;
    let crc32 = crc32fast::hash(selected.map.bytes);
    if crc32 != expected.crc32 {
        return Err(E::Mismatch("CRC-32"));
    }
    let map = pcx_map::Map::parse(selected.map.bytes).map_err(|error| E::Map {
        map: selected.map.source.into(),
        error,
    })?;
    let archive = CheckedArchive::parse(map.archive(), selected.map.source, budget.archive)?;
    let mut sha = Sha1::new();
    let mut xoro = 0;
    let mut map_preludes = [false; 2];
    for (index, (member, external)) in [
        ("Scripts\\common.j", selected.common),
        ("Scripts\\blizzard.j", selected.blizzard),
    ]
    .into_iter()
    .enumerate()
    {
        if let Some(bytes) = extract(&archive, member, budget, &mut remaining)? {
            map_preludes[index] = true;
            xoro ^= xor_rotate(&bytes);
            sha.update(&bytes);
        } else {
            identity(external)?;
            if external.edition != selected.map.edition || external.version != selected.map.version
            {
                return Err(E::Selection(
                    "external prelude edition/version differs from map",
                ));
            }
            if external.bytes.is_empty() {
                return Err(E::Selection("required external prelude is absent"));
            }
            if external.bytes.len() > budget.archive.max_decoded_bytes {
                return Err(E::Selection("external prelude exceeds member budget"));
            }
            charge(&mut remaining, external.bytes.len())?;
            xoro ^= xor_rotate(external.bytes);
            sha.update(external.bytes);
        }
    }
    // GHost++ CMap::Load's protocol mix separator, not an asset value.
    const MIX: u32 = 0x03F1_379E;
    xoro = (xoro.rotate_left(3) ^ MIX).rotate_left(3);
    sha.update(MIX.to_le_bytes());
    let mut found_script = false;
    for member in [
        "war3map.j",
        "scripts\\war3map.j",
        "war3map.w3e",
        "war3map.wpm",
        "war3map.doo",
        "war3map.w3u",
        "war3map.w3b",
        "war3map.w3d",
        "war3map.w3a",
        "war3map.w3q",
    ] {
        if found_script && member == "scripts\\war3map.j" {
            continue;
        }
        if let Some(bytes) = extract(&archive, member, budget, &mut remaining)? {
            if matches!(member, "war3map.j" | "scripts\\war3map.j") {
                found_script = true;
            }
            xoro = (xoro ^ xor_rotate(&bytes)).rotate_left(3);
            sha.update(&bytes);
        }
    }
    if !found_script {
        return Err(E::Selection("map has no nonempty script"));
    }
    if xoro != expected.xoro {
        return Err(E::Mismatch("XORO"));
    }
    let sha1: [u8; 20] = sha.finalize().into();
    if sha1 != expected.sha1 {
        return Err(E::Mismatch("SHA-1"));
    }
    Ok(MapMatch {
        map: selected.map,
        protocol_path: selected.protocol_path,
        prefix: *prefix,
        fingerprints: Fingerprints {
            size,
            crc32,
            xoro,
            sha1,
        },
        map_preludes,
    })
}

fn identity(source: SelectedBytes<'_>) -> Result<(), MapCheckError> {
    if [source.edition, source.version, source.source]
        .iter()
        .any(|value| value.trim().is_empty())
    {
        Err(MapCheckError::Selection(
            "incomplete edition/version/source identity",
        ))
    } else {
        Ok(())
    }
}

fn charge(remaining: &mut usize, bytes: usize) -> Result<(), MapCheckError> {
    *remaining = remaining
        .checked_sub(bytes)
        .ok_or(MapCheckError::HashBudget)?;
    Ok(())
}

fn extract(
    archive: &CheckedArchive<'_>,
    member: &'static str,
    budget: MapCheckBudget,
    remaining: &mut usize,
) -> Result<Option<Vec<u8>>, MapCheckError> {
    let Some(size) = archive
        .preflight_member(member, budget.archive.max_decoded_bytes)?
        .filter(|&size| size != 0)
    else {
        return Ok(None);
    };
    charge(remaining, size)?;
    Ok(archive.extract(member, budget.archive.max_decoded_bytes)?)
}

fn xor_rotate(bytes: &[u8]) -> u32 {
    let (words, tail) = bytes.as_chunks::<4>();
    let value = words.iter().fold(0u32, |value, word| {
        (value ^ u32::from_le_bytes(*word)).rotate_left(3)
    });
    tail.iter().fold(value, |value, &byte| {
        (value ^ u32::from(byte)).rotate_left(3)
    })
}

#[cfg(test)]
mod tests;

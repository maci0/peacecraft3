//! Reader and bounded writer for Warcraft III `.w3g` replays written by the
//! 1.29 client.
//!
//! Follows the public layout notes (header, data blocks, startup records, replay
//! data blocks) and is checked against one Legacy 1.29.2.9231 sample; see
//! `docs/re-notes/replay-recording.md`. The header CRC32 is verified. Every item is
//! read by its authored id and length, and a shape outside those notes is refused.
//! Block checksums are folded IEEE CRC32 checks of the header and compressed
//! payload. Checksum records and the encoded startup tail remain uninterpreted.
//! Structure only: a parsed replay is not evidence that the game would reproduce it.

#[path = "w3g_action.rs"]
pub mod action;

use std::io::Write;
use std::ops::Range;

use flate2::{Decompress, FlushDecompress, Status};

const TITLE: &[u8; 28] = b"Warcraft III recorded game\x1a\x00";
const MAGIC: &[u8; 4] = b"PX3W";
/// Header size and version of the observed 1.07+ layout.
const HEADER_LEN: usize = 68;
const HEADER_VERSION: u32 = 1;
const CRC_FIELD: Range<usize> = 64..68;
const BLOCK_HEAD_LEN: usize = 8;
/// Writer buffer size; the format stores each block's actual size separately.
const BLOCK_SIZE: usize = 8192;
/// Leading startup value. The notes call it a record id, not confirmed.
const FIRST_STARTUP_VALUE: u32 = 0x110;
const HOST_RECORD: u8 = 0x00;
const ADDITIONAL_RECORD: u8 = 0x16;
/// Size of a player record's extra data for custom games. Ladder (0x08) is refused.
const CUSTOM_EXTRA_SIZE: u8 = 0x01;
const GAME_START_ID: u8 = 0x19;
const SLOT_LEN: usize = 9;
const SETTINGS_LEN: usize = 13;
const START_BLOCKS: [(u8, u32); 3] = [(0x1A, 1), (0x1B, 1), (0x1C, 1)];
const TIME_SLOT_IDS: [u8; 2] = [0x1E, 0x1F];
const TIME_INCREMENT_LEN: usize = 2;
const COMMAND_HEAD_LEN: usize = 3;
const CHECKSUM_ID: u8 = 0x22;
const CHECKSUM_COUNT: u8 = 4;
const LEAVE_ID: u8 = 0x17;
const LEAVE_REASONS: [u32; 3] = [0x01, 0x0C, 0x0E];
const COUNTDOWN_ID: u8 = 0x2F;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub game_version: u32,
    pub build: u16,
    pub flags: u16,
    pub length_ms: u32,
    pub decompressed_size: u32,
    /// Verified header/payload checksums at each data block's offset 4.
    pub block_fields: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerRecord {
    pub player_id: u8,
    pub name: Vec<u8>,
}

/// One slot record. Fields are raw bytes as the notes describe them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    pub player_id: u8,
    pub download_percent: u8,
    pub status: u8,
    pub computer: u8,
    pub team: u8,
    pub color: u8,
    pub race_flags: u8,
    pub ai_strength: u8,
    pub handicap: u8,
}

/// The startup records, in the order they appear before the replay data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Startup {
    pub host: PlayerRecord,
    pub game_name: Vec<u8>,
    /// The 13 decoded game-setting bytes. Bit meanings are in the notes; the map checksum is not decoded.
    pub settings: [u8; SETTINGS_LEN],
    /// Map path as stored, for example `Maps/(2)BootyBay.w3m`.
    pub map_path: Vec<u8>,
    pub creator_name: Vec<u8>,
    /// Bytes after the strings, undecoded.
    pub encoded_tail: Vec<u8>,
    pub player_count: u32,
    pub game_type: u32,
    pub language_id: u32,
    pub players: Vec<PlayerRecord>,
    pub slots: Vec<Slot>,
    pub random_seed: u32,
    pub select_mode: u8,
    pub start_spots: u8,
}

/// A command block inside a time slot. `action` indexes the replay content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub player_id: u8,
    pub action: Range<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Leave {
    pub reason: u32,
    pub player_id: u8,
    pub result: u32,
    /// Unidentified; the notes ask whether it counts saved replays.
    pub trailer: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Record {
    TimeSlot {
        time_increment: u16,
        commands: Vec<Command>,
    },
    /// Value after a time slot. Kept raw, not verified.
    Checksum(u32),
    Leave(Leave),
    Countdown {
        mode: u32,
        seconds: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replay {
    pub header: Header,
    pub startup: Startup,
    /// Inflated content, exactly `header.decompressed_size` bytes.
    pub content: Vec<u8>,
    pub records: Vec<Record>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum W3gError {
    #[error("file is truncated at byte {at}")]
    Truncated { at: usize },
    #[error("missing the Warcraft III recorded game title")]
    BadTitle,
    #[error("header size {size} and version {version} are not the observed 68 and 1")]
    UnsupportedHeader { size: u32, version: u32 },
    #[error("missing the PX3W header magic")]
    BadMagic,
    #[error("header claims {claimed} bytes for a {actual}-byte file")]
    FileSize { claimed: u32, actual: usize },
    #[error("header CRC32 is 0x{stored:08x}, computed 0x{computed:08x}")]
    HeaderCrc { stored: u32, computed: u32 },
    #[error("block {index} checksum is 0x{stored:08x}, computed 0x{computed:08x}")]
    BlockCrc {
        index: usize,
        stored: u32,
        computed: u32,
    },
    #[error("block {index} does not inflate to exactly its declared size with all input consumed")]
    BlockMismatch { index: usize },
    #[error("blocks inflate to {available} bytes, short of the declared {declared}")]
    ContentShort { available: usize, declared: usize },
    #[error("content byte {at} past the declared end is not zero padding")]
    Padding { at: usize },
    #[error("unsupported shape at byte {at}: {what}")]
    Unsupported { at: usize, what: &'static str },
    #[error("malformed data at byte {at}: {what}")]
    Malformed { at: usize, what: &'static str },
    #[error("no reader for replay data block 0x{id:02x} at byte {at}")]
    UnsupportedBlock { at: usize, id: u8 },
    #[error("replay allocation exceeds available capacity")]
    Capacity,
}

/// Parses a `.w3g` file. Any shape the notes do not cover is refused.
pub fn parse(bytes: &[u8]) -> Result<Replay, W3gError> {
    if bytes.get(..TITLE.len()) != Some(TITLE.as_slice()) {
        return Err(W3gError::BadTitle);
    }
    let (header, block_count) = read_header(bytes)?;
    let (mut content, block_fields) = inflate_blocks(bytes, block_count)?;
    let declared = header.decompressed_size as usize;
    if content.len() < declared {
        return Err(W3gError::ContentShort {
            available: content.len(),
            declared,
        });
    }
    if let Some(at) = content[declared..].iter().position(|&b| b != 0) {
        return Err(W3gError::Padding { at: declared + at });
    }
    content.truncate(declared);
    let (startup, records) = read_content(&content)?;
    Ok(Replay {
        header: Header {
            block_fields,
            ..header
        },
        startup,
        content,
        records,
    })
}

fn read_content(content: &[u8]) -> Result<(Startup, Vec<Record>), W3gError> {
    let mut cursor = Cursor {
        bytes: content,
        at: 0,
    };
    let startup = read_startup(&mut cursor)?;
    let records = read_records(&mut cursor)?;
    Ok((startup, records))
}

/// Serializes a parsed `Replay` back to `.w3g` bytes.
///
/// The documented fields are recomputed: the header (with its verifiable CRC32),
/// the data-block framing and zlib streams, and the decompressed content. The
/// block checksums are recomputed after compression. The `0x22` record values and
/// encoded startup tail remain verbatim in `content`; their meaning is unresolved.
/// Recompression may change `header.block_fields` without changing replay content.
/// `content` is authoritative; contradictory `startup` or `records` are refused.
pub fn write(replay: &Replay) -> Result<Vec<u8>, W3gError> {
    if replay.content.is_empty() {
        return Err(W3gError::Malformed {
            at: 0,
            what: "cannot write an empty replay",
        });
    }
    if replay.header.decompressed_size as usize != replay.content.len() {
        return Err(W3gError::Malformed {
            at: 0,
            what: "header decompressed size does not match the replay content",
        });
    }
    let (startup, records) = read_content(&replay.content)?;
    if startup != replay.startup || records != replay.records {
        return Err(W3gError::Malformed {
            at: 0,
            what: "parsed startup or records disagree with replay content",
        });
    }
    let count = replay.content.len().div_ceil(BLOCK_SIZE);
    let block_count = u32::try_from(count).map_err(|_| W3gError::Malformed {
        at: 0,
        what: "block count exceeds the replay format",
    })?;
    let mut blocks = Vec::new();
    for (index, chunk) in replay.content.chunks(BLOCK_SIZE).enumerate() {
        let mut padded = chunk.to_vec();
        padded.resize(BLOCK_SIZE, 0);
        let mut encoder =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder
            .write_all(&padded)
            .map_err(|_| W3gError::BlockMismatch { index })?;
        encoder
            .flush()
            .map_err(|_| W3gError::BlockMismatch { index })?;
        let payload = encoder.get_ref().clone();
        let compressed =
            u16::try_from(payload.len()).map_err(|_| W3gError::BlockMismatch { index })?;
        blocks
            .try_reserve(BLOCK_HEAD_LEN + payload.len())
            .map_err(|_| W3gError::Capacity)?;
        blocks.extend_from_slice(&compressed.to_le_bytes());
        blocks.extend_from_slice(&(BLOCK_SIZE as u16).to_le_bytes());
        let checksum = block_checksum(compressed, BLOCK_SIZE as u16, &payload);
        blocks.extend_from_slice(&checksum.to_le_bytes());
        blocks.extend_from_slice(&payload);
    }
    let compressed_total = blocks
        .len()
        .checked_add(HEADER_LEN)
        .and_then(|size| u32::try_from(size).ok())
        .ok_or(W3gError::Malformed {
            at: 0,
            what: "file size exceeds the replay format",
        })?;
    let mut head = Vec::with_capacity(HEADER_LEN);
    head.extend_from_slice(TITLE);
    head.extend_from_slice(&(HEADER_LEN as u32).to_le_bytes());
    head.extend_from_slice(&compressed_total.to_le_bytes());
    head.extend_from_slice(&HEADER_VERSION.to_le_bytes());
    head.extend_from_slice(&replay.header.decompressed_size.to_le_bytes());
    head.extend_from_slice(&block_count.to_le_bytes());
    head.extend_from_slice(MAGIC);
    head.extend_from_slice(&replay.header.game_version.to_le_bytes());
    head.extend_from_slice(&replay.header.build.to_le_bytes());
    head.extend_from_slice(&replay.header.flags.to_le_bytes());
    head.extend_from_slice(&replay.header.length_ms.to_le_bytes());
    head.extend_from_slice(&[0, 0, 0, 0]);
    let crc = crc32fast::hash(&head);
    head[CRC_FIELD].copy_from_slice(&crc.to_le_bytes());
    let mut out = Vec::new();
    out.try_reserve_exact(compressed_total as usize)
        .map_err(|_| W3gError::Capacity)?;
    out.extend_from_slice(&head);
    out.extend_from_slice(&blocks);
    Ok(out)
}

fn block_checksum(compressed: u16, inflated: u16, payload: &[u8]) -> u32 {
    let mut header = [0; BLOCK_HEAD_LEN];
    header[..2].copy_from_slice(&compressed.to_le_bytes());
    header[2..4].copy_from_slice(&inflated.to_le_bytes());
    let fold = |crc: u32| (crc ^ (crc >> 16)) & 0xffff;
    fold(crc32fast::hash(&header)) | (fold(crc32fast::hash(payload)) << 16)
}

/// Returns the header and the declared block count.
fn read_header(bytes: &[u8]) -> Result<(Header, usize), W3gError> {
    let size = u32_at(bytes, 28)?;
    let version = u32_at(bytes, 36)?;
    if size as usize != HEADER_LEN || version != HEADER_VERSION {
        return Err(W3gError::UnsupportedHeader { size, version });
    }
    if bytes.get(48..52) != Some(MAGIC.as_slice()) {
        return Err(W3gError::BadMagic);
    }
    let claimed = u32_at(bytes, 32)?;
    if claimed as usize != bytes.len() {
        return Err(W3gError::FileSize {
            claimed,
            actual: bytes.len(),
        });
    }
    let mut head = take(bytes, 0, HEADER_LEN)?.to_vec();
    let stored = u32_at(&head, CRC_FIELD.start)?;
    head[CRC_FIELD].fill(0);
    let computed = crc32fast::hash(&head);
    if stored != computed {
        return Err(W3gError::HeaderCrc { stored, computed });
    }
    let block_count = u32_at(bytes, 44)? as usize;
    let header = Header {
        game_version: u32_at(bytes, 52)?,
        build: u16::from_le_bytes(take(bytes, 56, 2)?.try_into().expect("two bytes")),
        flags: u16::from_le_bytes(take(bytes, 58, 2)?.try_into().expect("two bytes")),
        length_ms: u32_at(bytes, 60)?,
        decompressed_size: u32_at(bytes, 40)?,
        block_fields: Vec::new(),
    };
    Ok((header, block_count))
}

fn inflate_blocks(bytes: &[u8], count: usize) -> Result<(Vec<u8>, Vec<u32>), W3gError> {
    if count > bytes.len().saturating_sub(HEADER_LEN) / BLOCK_HEAD_LEN {
        return Err(W3gError::Truncated { at: bytes.len() });
    }
    let mut content = Vec::new();
    let mut fields = Vec::new();
    let mut at = HEADER_LEN;
    for index in 0..count {
        let head = take(bytes, at, BLOCK_HEAD_LEN)?;
        let compressed = usize::from(u16::from_le_bytes([head[0], head[1]]));
        let inflated = usize::from(u16::from_le_bytes([head[2], head[3]]));
        let stored = u32::from_le_bytes([head[4], head[5], head[6], head[7]]);
        let payload = take(bytes, at + BLOCK_HEAD_LEN, compressed)?;
        let computed = block_checksum(compressed as u16, inflated as u16, payload);
        if stored != computed {
            return Err(W3gError::BlockCrc {
                index,
                stored,
                computed,
            });
        }
        fields.try_reserve(1).map_err(|_| W3gError::Capacity)?;
        fields.push(stored);
        let mut out = vec![0u8; inflated];
        let mut zlib = Decompress::new(true);
        // The sample's streams end without an end-of-stream marker, so Sync decodes what is there.
        let status = zlib
            .decompress(payload, &mut out, FlushDecompress::Sync)
            .map_err(|_| W3gError::BlockMismatch { index })?;
        let whole = zlib.total_in() == compressed as u64 && zlib.total_out() == inflated as u64;
        if !whole || !matches!(status, Status::Ok | Status::StreamEnd) {
            return Err(W3gError::BlockMismatch { index });
        }
        content
            .try_reserve(out.len())
            .map_err(|_| W3gError::Capacity)?;
        content.extend_from_slice(&out);
        at += BLOCK_HEAD_LEN + compressed;
    }
    if at != bytes.len() {
        return Err(W3gError::FileSize {
            claimed: at as u32,
            actual: bytes.len(),
        });
    }
    Ok((content, fields))
}

fn read_startup(c: &mut Cursor) -> Result<Startup, W3gError> {
    let at = c.at;
    if c.u32()? != FIRST_STARTUP_VALUE {
        return Err(W3gError::Unsupported {
            at,
            what: "leading startup value is not 0x00000110",
        });
    }
    let host = read_player(c, HOST_RECORD)?;
    let game_name = c.cstring()?;
    if c.u8()? != 0 {
        return Err(W3gError::Malformed {
            at: c.at - 1,
            what: "byte after the game name is not zero",
        });
    }
    let encoded_at = c.at;
    let encoded = c.cstring()?;
    let decoded = decode_encoded(&encoded);
    let Decoded {
        settings,
        map_path,
        creator_name,
        tail,
    } = split_decoded(&decoded, encoded_at)?;
    let player_count = c.u32()?;
    let game_type = c.u32()?;
    let language_id = c.u32()?;
    let mut players = Vec::new();
    while c.peek()? == ADDITIONAL_RECORD {
        players.push(read_player(c, ADDITIONAL_RECORD)?);
        let trailer_at = c.at;
        if c.u32()? != 0 {
            return Err(W3gError::Unsupported {
                at: trailer_at,
                what: "additional player trailer is not zero (patch 1.07 and later)",
            });
        }
    }
    let (slots, random_seed, select_mode, start_spots) = read_game_start(c)?;
    for (id, value) in START_BLOCKS {
        let at = c.at;
        if c.u8()? != id || c.u32()? != value {
            return Err(W3gError::Unsupported {
                at,
                what: "startup block differs from 0x1A, 0x1B, 0x1C with value 1",
            });
        }
    }
    Ok(Startup {
        host,
        game_name,
        settings,
        map_path,
        creator_name,
        encoded_tail: tail,
        player_count,
        game_type,
        language_id,
        players,
        slots,
        random_seed,
        select_mode,
        start_spots,
    })
}

fn read_player(c: &mut Cursor, expected_id: u8) -> Result<PlayerRecord, W3gError> {
    let at = c.at;
    if c.u8()? != expected_id {
        return Err(W3gError::Malformed {
            at,
            what: "player record id differs from the expected one",
        });
    }
    let player_id = c.u8()?;
    let name = c.cstring()?;
    let extra_at = c.at;
    match c.u8()? {
        CUSTOM_EXTRA_SIZE => {
            if c.u8()? != 0 {
                return Err(W3gError::Malformed {
                    at: c.at - 1,
                    what: "custom player data is not one null byte",
                });
            }
        }
        _ => {
            return Err(W3gError::Unsupported {
                at: extra_at,
                what: "player record extra data is not the custom size (ladder is not decoded)",
            });
        }
    }
    Ok(PlayerRecord { player_id, name })
}

fn read_game_start(c: &mut Cursor) -> Result<(Vec<Slot>, u32, u8, u8), W3gError> {
    let at = c.at;
    if c.u8()? != GAME_START_ID {
        return Err(W3gError::Malformed {
            at,
            what: "expected the game start record 0x19",
        });
    }
    let length = usize::from(c.u16()?);
    let count = usize::from(c.u8()?);
    // Covers the slot count byte, the slots, the seed, the select mode and the start spots.
    if length != 1 + count * SLOT_LEN + 4 + 1 + 1 {
        return Err(W3gError::Malformed {
            at,
            what: "game start length does not cover its slots, seed, mode and start spots",
        });
    }
    let mut slots = Vec::with_capacity(count);
    for _ in 0..count {
        let b = c.take(SLOT_LEN)?;
        slots.push(Slot {
            player_id: b[0],
            download_percent: b[1],
            status: b[2],
            computer: b[3],
            team: b[4],
            color: b[5],
            race_flags: b[6],
            ai_strength: b[7],
            handicap: b[8],
        });
    }
    Ok((slots, c.u32()?, c.u8()?, c.u8()?))
}

/// Inverts the notes' control-byte encoding. Each control byte covers the next seven bytes.
fn decode_encoded(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len());
    let mut control = 0u8;
    for (pos, &byte) in raw.iter().enumerate() {
        if pos % 8 == 0 {
            control = byte;
        } else if (control >> (pos % 8)) & 1 == 1 {
            out.push(byte);
        } else {
            out.push(byte.wrapping_sub(1));
        }
    }
    out
}

/// The decoded encoded string. The notes list settings, two strings and an empty
/// string. The sample also has `tail` after them: 20 bytes this reader does not
/// identify (not the map's SHA-1). The raw field is delimited by its null
/// terminator, so the tail cannot move the record boundaries; it is kept raw.
struct Decoded {
    settings: [u8; SETTINGS_LEN],
    map_path: Vec<u8>,
    creator_name: Vec<u8>,
    tail: Vec<u8>,
}

fn split_decoded(decoded: &[u8], at: usize) -> Result<Decoded, W3gError> {
    let mut c = Cursor {
        bytes: decoded,
        at: 0,
    };
    let settings: [u8; SETTINGS_LEN] = c
        .take(SETTINGS_LEN)
        .map_err(|_| W3gError::Malformed {
            at,
            what: "encoded string is shorter than the game settings",
        })?
        .try_into()
        .expect("took exactly the settings length");
    let map_path = c.cstring()?;
    let creator_name = c.cstring()?;
    if !c.cstring()?.is_empty() {
        return Err(W3gError::Malformed {
            at,
            what: "third string of the encoded string is not empty",
        });
    }
    let tail = c.rest();
    Ok(Decoded {
        settings,
        map_path,
        creator_name,
        tail,
    })
}

fn read_records(c: &mut Cursor) -> Result<Vec<Record>, W3gError> {
    let mut records = Vec::new();
    while !c.done() {
        let at = c.at;
        let id = c.u8()?;
        if TIME_SLOT_IDS.contains(&id) {
            let length = usize::from(c.u16()?);
            if length < TIME_INCREMENT_LEN {
                return Err(W3gError::Malformed {
                    at,
                    what: "time slot is shorter than its time increment",
                });
            }
            let body_at = c.at;
            let body = c.take(length)?;
            let time_increment = u16::from_le_bytes([body[0], body[1]]);
            let commands =
                read_commands(&body[TIME_INCREMENT_LEN..], body_at + TIME_INCREMENT_LEN)?;
            records.push(Record::TimeSlot {
                time_increment,
                commands,
            });
        } else if id == CHECKSUM_ID {
            let count_at = c.at;
            if c.u8()? != CHECKSUM_COUNT {
                return Err(W3gError::Malformed {
                    at: count_at,
                    what: "checksum record byte count is not 4",
                });
            }
            records.push(Record::Checksum(c.u32()?));
        } else if id == LEAVE_ID {
            let reason_at = c.at;
            let reason = c.u32()?;
            if !LEAVE_REASONS.contains(&reason) {
                return Err(W3gError::Unsupported {
                    at: reason_at,
                    what: "leave reason is not one of 0x01, 0x0C, 0x0E",
                });
            }
            records.push(Record::Leave(Leave {
                reason,
                player_id: c.u8()?,
                result: c.u32()?,
                trailer: c.u32()?,
            }));
        } else if id == COUNTDOWN_ID {
            let mode_at = c.at;
            let mode = c.u32()?;
            if mode > 1 {
                return Err(W3gError::Malformed {
                    at: mode_at,
                    what: "countdown mode is not 0 or 1",
                });
            }
            records.push(Record::Countdown {
                mode,
                seconds: c.u32()?,
            });
        } else {
            return Err(W3gError::UnsupportedBlock { at, id });
        }
    }
    Ok(records)
}

/// Splits a time slot's command data. `base` is the content offset of `body`.
fn read_commands(body: &[u8], base: usize) -> Result<Vec<Command>, W3gError> {
    let mut commands = Vec::new();
    let mut at = 0;
    while at < body.len() {
        if body.len() - at < COMMAND_HEAD_LEN {
            return Err(W3gError::Malformed {
                at: base + at,
                what: "command block header runs past its time slot",
            });
        }
        let player_id = body[at];
        let length = usize::from(u16::from_le_bytes([body[at + 1], body[at + 2]]));
        let start = at + COMMAND_HEAD_LEN;
        if start + length > body.len() {
            return Err(W3gError::Malformed {
                at: base + at,
                what: "action block runs past its time slot",
            });
        }
        commands.push(Command {
            player_id,
            action: base + start..base + start + length,
        });
        at = start + length;
    }
    Ok(commands)
}

struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], W3gError> {
        let taken = take(self.bytes, self.at, n)?;
        self.at += n;
        Ok(taken)
    }

    fn u8(&mut self) -> Result<u8, W3gError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, W3gError> {
        Ok(u16::from_le_bytes(
            self.take(2)?.try_into().expect("two bytes"),
        ))
    }

    fn u32(&mut self) -> Result<u32, W3gError> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().expect("four bytes"),
        ))
    }

    fn peek(&self) -> Result<u8, W3gError> {
        self.bytes
            .get(self.at)
            .copied()
            .ok_or(W3gError::Truncated { at: self.at })
    }

    /// Reads a null-terminated string and returns it without the terminator.
    fn cstring(&mut self) -> Result<Vec<u8>, W3gError> {
        let rest = &self.bytes[self.at..];
        let len = rest
            .iter()
            .position(|&b| b == 0)
            .ok_or(W3gError::Truncated {
                at: self.bytes.len(),
            })?;
        self.at += len + 1;
        Ok(rest[..len].to_vec())
    }

    fn done(&self) -> bool {
        self.at == self.bytes.len()
    }

    fn rest(&self) -> Vec<u8> {
        self.bytes[self.at..].to_vec()
    }
}

fn take(bytes: &[u8], at: usize, len: usize) -> Result<&[u8], W3gError> {
    at.checked_add(len)
        .and_then(|end| bytes.get(at..end))
        .ok_or(W3gError::Truncated { at })
}

fn u32_at(bytes: &[u8], at: usize) -> Result<u32, W3gError> {
    Ok(u32::from_le_bytes(
        take(bytes, at, 4)?.try_into().expect("four bytes"),
    ))
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    const BLOCK: usize = 8192;
    const SETTINGS: [u8; SETTINGS_LEN] = [1, 0x0B, 0, 0, 0, 0, 0, 0, 0, 0xAA, 0xBB, 0xCC, 0xDD];
    const TAIL: [u8; 20] = [0x5A; 20];

    /// Inverse of `decode_encoded`: odd bytes are stored as-is, even bytes plus one.
    fn encode(decoded: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        for group in decoded.chunks(7) {
            let mut control = 1u8;
            let mut data = Vec::new();
            for (i, &c) in group.iter().enumerate() {
                if c % 2 == 1 {
                    control |= 1 << (i + 1);
                    data.push(c);
                } else {
                    data.push(c + 1);
                }
            }
            out.push(control);
            out.extend(data);
        }
        out.push(0);
        out
    }

    fn encoded_settings() -> Vec<u8> {
        let decoded = [
            SETTINGS.as_slice(),
            b"Maps/(2)BootyBay.w3m\0",
            b"replay1\0",
            b"\0",
            &TAIL,
        ]
        .concat();
        encode(&decoded)
    }

    /// Startup records with mutable documented values. `encoded` includes its terminator.
    fn startup(first: u32, extra: u8, start_len: u16, block_value: u32, encoded: &[u8]) -> Vec<u8> {
        let mut out = first.to_le_bytes().to_vec();
        out.extend([HOST_RECORD, 1]);
        out.extend(b"replay1\0");
        out.extend([extra, 0]);
        out.extend(b"Local Game\0");
        out.push(0);
        out.extend(encoded);
        out.extend(24u32.to_le_bytes());
        out.extend(9u32.to_le_bytes());
        out.extend(0u32.to_le_bytes());
        let slots = [
            [1, 0x64, 2, 0, 0, 0, 0x60, 1, 0x64],
            [0, 0x64, 2, 1, 1, 1, 0x60, 1, 0x64],
        ];
        out.push(GAME_START_ID);
        out.extend(start_len.to_le_bytes());
        out.push(slots.len() as u8);
        for slot in slots {
            out.extend(slot);
        }
        out.extend(0x04b3_ff52u32.to_le_bytes());
        out.push(0);
        out.push(2);
        for id in [0x1A, 0x1B, 0x1C] {
            out.push(id);
            out.extend(block_value.to_le_bytes());
        }
        out
    }

    fn sample_startup() -> Vec<u8> {
        startup(
            FIRST_STARTUP_VALUE,
            CUSTOM_EXTRA_SIZE,
            25,
            1,
            &encoded_settings(),
        )
    }

    /// Replay data: a slot with one command, a checksum, an empty slot, a checksum,
    /// a countdown, `extra`, and a leave record with `leave_reason`.
    fn records(extra: &[u8], leave_reason: u32) -> Vec<u8> {
        let mut out = vec![0x1F];
        let body = [0x66, 0x00, 1, 2, 0, 0xAA, 0xBB];
        out.extend((body.len() as u16).to_le_bytes());
        out.extend(body);
        out.extend([CHECKSUM_ID, 4, 1, 2, 3, 4]);
        out.extend([0x1F, 2, 0, 0x63, 0]);
        out.extend([CHECKSUM_ID, 4, 0xEF, 0xBE, 0xAD, 0xDE]);
        out.extend([COUNTDOWN_ID, 0, 0, 0, 0, 10, 0, 0, 0]);
        out.extend(extra);
        out.push(LEAVE_ID);
        out.extend(leave_reason.to_le_bytes());
        out.extend([1, 7, 0, 0, 0, 1, 0, 0, 0]);
        out
    }

    /// Builds a `.w3g` with a valid header CRC, zlib blocks sized as the sample's, and zero padding.
    fn file(startup: &[u8], data: &[u8], declared: Option<u32>, tail: &[u8]) -> Vec<u8> {
        let body = [startup, data].concat();
        let mut content = [body.as_slice(), tail].concat();
        content.resize(content.len().div_ceil(BLOCK) * BLOCK, 0);
        let mut blocks = Vec::new();
        for chunk in content.chunks(BLOCK) {
            let mut encoder =
                flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
            encoder.write_all(chunk).unwrap();
            encoder.flush().unwrap();
            let payload = encoder.get_ref().clone();
            blocks.extend((payload.len() as u16).to_le_bytes());
            blocks.extend((BLOCK as u16).to_le_bytes());
            blocks
                .extend(block_checksum(payload.len() as u16, BLOCK as u16, &payload).to_le_bytes());
            blocks.extend(payload);
        }
        let mut head = Vec::new();
        head.extend_from_slice(TITLE);
        head.extend((HEADER_LEN as u32).to_le_bytes());
        head.extend(((HEADER_LEN + blocks.len()) as u32).to_le_bytes());
        head.extend(HEADER_VERSION.to_le_bytes());
        head.extend(declared.unwrap_or(body.len() as u32).to_le_bytes());
        head.extend(((content.len() / BLOCK) as u32).to_le_bytes());
        head.extend(MAGIC);
        head.extend(29u32.to_le_bytes());
        head.extend(6060u16.to_le_bytes());
        head.extend(0u16.to_le_bytes());
        head.extend(73_625u32.to_le_bytes());
        head.extend([0, 0, 0, 0]);
        let crc = crc32fast::hash(&head);
        head[CRC_FIELD].copy_from_slice(&crc.to_le_bytes());
        [head, blocks].concat()
    }

    fn sample_file() -> Vec<u8> {
        file(&sample_startup(), &records(&[], 12), None, &[])
    }

    #[test]
    fn decodes_startup_and_every_record_of_a_synthetic_replay() {
        let replay = parse(&sample_file()).unwrap();
        let s = &replay.startup;
        assert_eq!(
            s.host,
            PlayerRecord {
                player_id: 1,
                name: b"replay1".to_vec()
            }
        );
        assert_eq!(s.game_name, b"Local Game");
        assert_eq!(s.settings, SETTINGS);
        assert_eq!(s.map_path, b"Maps/(2)BootyBay.w3m");
        assert_eq!(s.creator_name, b"replay1");
        assert_eq!(s.encoded_tail, TAIL);
        assert_eq!((s.player_count, s.game_type, s.language_id), (24, 9, 0));
        assert!(s.players.is_empty());
        assert_eq!(s.slots.len(), 2);
        assert_eq!((s.slots[1].computer, s.slots[1].ai_strength), (1, 1));
        assert_eq!(
            (s.random_seed, s.select_mode, s.start_spots),
            (0x04b3_ff52, 0, 2)
        );
        // Startup, then the first time slot: tag, length, time increment, command head.
        let action_at = sample_startup().len() + 1 + 2 + 2 + COMMAND_HEAD_LEN;
        assert_eq!(
            replay.records[0],
            Record::TimeSlot {
                time_increment: 0x66,
                commands: vec![Command {
                    player_id: 1,
                    action: action_at..action_at + 2,
                }],
            }
        );
        assert_eq!(&replay.content[action_at..action_at + 2], &[0xAA, 0xBB]);
        assert_eq!(
            replay.records[1..],
            [
                Record::Checksum(0x0403_0201),
                Record::TimeSlot {
                    time_increment: 0x63,
                    commands: vec![],
                },
                Record::Checksum(0xDEAD_BEEF),
                Record::Countdown {
                    mode: 0,
                    seconds: 10
                },
                Record::Leave(Leave {
                    reason: 12,
                    player_id: 1,
                    result: 7,
                    trailer: 1,
                }),
            ]
        );
    }

    #[test]
    fn encoded_string_decodes_to_the_bytes_it_was_built_from() {
        let plain: Vec<u8> = (0..=255u8).collect();
        assert_eq!(
            decode_encoded(&encode(&plain)[..encode(&plain).len() - 1]),
            plain
        );
        let zeros = [0u8; 9];
        assert_eq!(
            decode_encoded(&encode(&zeros)[..encode(&zeros).len() - 1]),
            zeros
        );
    }

    #[test]
    fn every_truncation_is_refused() {
        let bytes = sample_file();
        for n in 0..bytes.len() {
            assert!(parse(&bytes[..n]).is_err(), "prefix of {n} bytes parsed");
        }
    }

    #[test]
    fn every_header_byte_change_is_refused() {
        let good = sample_file();
        for at in 0..HEADER_LEN {
            let mut bad = good.clone();
            bad[at] ^= 0x5A;
            assert!(parse(&bad).is_err(), "header byte {at} change parsed");
        }
    }

    #[test]
    fn a_header_crc_that_is_only_the_crc_field_is_refused() {
        let mut bytes = sample_file();
        bytes[CRC_FIELD.start] ^= 1;
        assert!(matches!(parse(&bytes), Err(W3gError::HeaderCrc { .. })));
    }

    #[test]
    fn a_corrupt_zlib_payload_is_refused() {
        let mut bytes = sample_file();
        bytes[HEADER_LEN + BLOCK_HEAD_LEN] ^= 0xFF;
        assert!(matches!(
            parse(&bytes),
            Err(W3gError::BlockCrc { index: 0, .. })
        ));
    }

    #[test]
    fn nonzero_bytes_past_the_declared_content_are_refused() {
        let body_len = sample_startup().len() + records(&[], 12).len();
        let bytes = file(&sample_startup(), &records(&[], 12), None, &[1]);
        assert_eq!(parse(&bytes), Err(W3gError::Padding { at: body_len }));
    }

    #[test]
    fn declared_content_longer_than_the_blocks_is_refused() {
        let bytes = file(
            &sample_startup(),
            &records(&[], 12),
            Some(BLOCK as u32 * 3),
            &[],
        );
        assert!(matches!(parse(&bytes), Err(W3gError::ContentShort { .. })));
    }

    #[test]
    fn startup_shapes_outside_the_notes_are_refused_not_guessed() {
        let encoded = encoded_settings();
        let cases = [
            startup(0x111, CUSTOM_EXTRA_SIZE, 25, 1, &encoded),
            startup(FIRST_STARTUP_VALUE, 0x08, 25, 1, &encoded),
            startup(FIRST_STARTUP_VALUE, CUSTOM_EXTRA_SIZE, 26, 1, &encoded),
            startup(FIRST_STARTUP_VALUE, CUSTOM_EXTRA_SIZE, 25, 2, &encoded),
        ];
        for startup in cases {
            let bytes = file(&startup, &records(&[], 12), None, &[]);
            assert!(
                matches!(
                    parse(&bytes),
                    Err(W3gError::Unsupported { .. } | W3gError::Malformed { .. })
                ),
                "startup shape parsed"
            );
        }
    }

    #[test]
    fn replay_data_shapes_outside_the_notes_are_refused() {
        let cases = [
            (
                records(&[0x20, 0, 0, 0, 0], 12),
                W3gError::UnsupportedBlock { at: 0, id: 0x20 },
            ),
            (
                records(&[CHECKSUM_ID, 3, 1, 2, 3], 12),
                W3gError::Malformed { at: 0, what: "" },
            ),
            (
                records(&[0x1F, 1, 0, 0], 12),
                W3gError::Malformed { at: 0, what: "" },
            ),
            (
                records(&[0x1F, 6, 0, 0x66, 0, 1, 9, 0, 0xAA], 12),
                W3gError::Malformed { at: 0, what: "" },
            ),
            (
                records(&[COUNTDOWN_ID, 2, 0, 0, 0, 0, 0, 0, 0], 12),
                W3gError::Malformed { at: 0, what: "" },
            ),
            (records(&[], 5), W3gError::Unsupported { at: 0, what: "" }),
        ];
        for (data, expected) in cases {
            let bytes = file(&sample_startup(), &data, None, &[]);
            let got = parse(&bytes).unwrap_err();
            assert_eq!(
                std::mem::discriminant(&got),
                std::mem::discriminant(&expected),
                "{got:?} for a shape that should be {expected:?}"
            );
        }
    }

    #[test]
    fn a_fake_slot_chain_inside_the_settings_is_never_read_as_records() {
        // A time slot tag, length 2, time 0x63, then a checksum record, all inside
        // the settings bytes. A scan for a slot start would begin here and could
        // reach the content end. The authored layout reads these 13 bytes as settings.
        let fake = [0x1F, 0x02, 0x00, 0x63, 0x00, 0x22, 0x04, 1, 2, 3, 4, 5, 6];
        let decoded = [
            fake.as_slice(),
            b"Maps/(2)BootyBay.w3m\0",
            b"replay1\0",
            b"\0",
        ]
        .concat();
        let bytes = file(
            &startup(
                FIRST_STARTUP_VALUE,
                CUSTOM_EXTRA_SIZE,
                25,
                1,
                &encode(&decoded),
            ),
            &records(&[], 12),
            None,
            &[],
        );
        let replay = parse(&bytes).unwrap();
        assert_eq!(replay.startup.settings, fake);
        assert_eq!(replay.records.len(), 6);
    }

    #[test]
    fn write_round_trips_the_synthetic_file() {
        let original = sample_file();
        let replay = parse(&original).unwrap();
        assert_eq!(parse(&write(&replay).unwrap()).unwrap(), replay);
    }

    #[test]
    fn block_checksum_detects_every_header_checksum_and_payload_byte_change() {
        let bytes = sample_file();
        for at in HEADER_LEN..bytes.len() {
            let mut bad = bytes.clone();
            bad[at] ^= 1;
            assert!(parse(&bad).is_err(), "block byte {at} change parsed");
        }
    }

    #[test]
    fn writer_rejects_changed_parsed_views_instead_of_discarding_the_edit() {
        let original = parse(&sample_file()).unwrap();
        let mut replay = original.clone();
        replay.startup.random_seed ^= 1;
        assert!(matches!(write(&replay), Err(W3gError::Malformed { .. })));
        let mut replay = original;
        replay.records[1] = Record::Checksum(99);
        assert!(matches!(write(&replay), Err(W3gError::Malformed { .. })));
    }

    #[test]
    fn writer_checksums_cover_recompressed_multiple_blocks() {
        let extra = [COUNTDOWN_ID, 0, 0, 0, 0, 10, 0, 0, 0].repeat(2000);
        let original = file(&sample_startup(), &records(&extra, 12), None, &[]);
        let replay = parse(&original).unwrap();
        let written = write(&replay).unwrap();
        let round = parse(&written).unwrap();
        assert!(round.header.block_fields.len() > 1);
        assert_eq!(round.content, replay.content);
        assert_eq!(round.records, replay.records);
        for at in HEADER_LEN + 4..HEADER_LEN + 8 {
            let mut bad = written.clone();
            bad[at] ^= 1;
            assert!(matches!(
                parse(&bad),
                Err(W3gError::BlockCrc { index: 0, .. })
            ));
        }
    }

    #[test]
    fn write_recomputes_block_checksums_without_source_fields() {
        let mut replay = parse(&sample_file()).unwrap();
        replay.header.block_fields.clear();
        let round = parse(&write(&replay).unwrap()).unwrap();
        assert!(!round.header.block_fields.is_empty());
        assert_eq!(round.content, replay.content);
    }

    #[test]
    fn write_refuses_when_decompressed_size_mismatches_the_content() {
        let mut replay = parse(&sample_file()).unwrap();
        replay.header.decompressed_size += 1;
        assert!(matches!(write(&replay), Err(W3gError::Malformed { .. })));
    }

    #[test]
    fn write_carries_the_uninterpreted_record_values() {
        let replay = parse(&sample_file()).unwrap();
        let written = write(&replay).unwrap();
        let round = parse(&written).unwrap();
        // Record values remain unchanged; block checksums are computed separately.
        assert_eq!(
            round
                .records
                .iter()
                .filter_map(|r| if let Record::Checksum(value) = r {
                    Some(*value)
                } else {
                    None
                })
                .collect::<Vec<_>>(),
            replay
                .records
                .iter()
                .filter_map(|r| if let Record::Checksum(value) = r {
                    Some(*value)
                } else {
                    None
                })
                .collect::<Vec<_>>()
        );
        assert_eq!(round.startup.encoded_tail, replay.startup.encoded_tail);
    }

    #[test]
    fn a_player_name_with_slot_like_bytes_is_read_by_its_terminator() {
        let name = [0x1F, 0x01, 0x63, 0x22, 0x04];
        let mut startup = sample_startup();
        let at = 6;
        startup.splice(at..at + 7, name);
        let bytes = file(&startup, &records(&[], 12), None, &[]);
        let replay = parse(&bytes).unwrap();
        assert_eq!(replay.startup.host.name, name);
    }
}

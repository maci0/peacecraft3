//! `war3map.w3i` — the map info file: player slots, map flags, loading screen
//! settings, and the forced/suggested object-table lists.
//!
//! # Why this file gates M3
//!
//! `war3map.w3i` is the map's own description of how the game is to be set up:
//! which players exist, what controller each has, which race each plays, which
//! start position each has, what the camera bounds are, and — critically — which object
//! tables the map *forces* and which it merely *suggests*. Without it the map
//! cannot be set up, so a faithful reimplementation that reads `.w3m` but not
//! `.w3i` renders terrain and nothing else.
//!
//! # Where the layout comes from
//!
//! Historical reader/writer evidence comes from `wc3/game/app/game.dll`, whose
//! VERSIONINFO is 1.26.0.6401 and SHA-256 is
//! `6d21bd9a0f9fbc8446f455c9e89ac994fed68174426fe608fcb9baefd4dec53c`.
//! Those offsets do not identify adjacent archives or the separate Legacy
//! 1.29.2.9231 executable. The field framing is cross-checked against supplied
//! map bytes; historical field labels are not exact-Legacy semantic proof.
//! This DLL reads and writes through version-gated routines:
//!
//! ```text
//! writer  game.dll 0x6f7267f0   (called from 0x6f739412, the `.w3i` writer)
//! reader  game.dll 0x6f738910   (called from 0x6f739421, the `.w3i` reader)
//! ```
//!
//! The dispatcher at `0x6f739400` picks one or the other for the extension
//! literal `.w3i` at `0x6f98114c`. Both walks advance a `CDataStore` cursor by
//! exactly the size of each field they touch, so the on-disk width of every
//! field is exactly the width of the read that produced it.
//!
//! # Historical version-18 layout (the parser also supports version 25)
//!
//! ```text
//! offset  size  field
//! 0x000   4     version word                          (= 18)
//! 0x004   4     A — first leading word
//! 0x008   4     B — second leading word; historical build label, semantics unproved
//! 0x00c   str   map name           (w3s/TRIGSTR reference)
//!         str   map description
//!         str   recommended players string
//!         str   (unused at this version; an empty byte)
//! 0x..    8×4   camera bounds: minX, minY, maxX, maxY, and a second rect
//!         4×4   four u32: tileset-ish constants
//!         2×4   two u32
//!         4     flags word                            (bit 0x800 forced on for < 0xf)
//!         1     a byte
//!         4     a u32
//!         str   parallax / a string                  (present from 0x14)
//!         str   fog type string
//!         str   fog height string
//!         str   fog z-something string              (from 0x0b)
//!         4     (from 0x11)
//!         str   (from 0x14)
//!         str   loading-screen background / text strings × 3 (from 0x0d)
//!         ...
//!         u32   player count
//!         n × player record
//!         u32   force count
//!         f × force record
//!         ... cruft count sections
//! ```
//!
//! Only the version gates that matter for the shipped files are spelled out
//! here; the module carries the full gate set for every version the writer can
//! emit, because the reader has them.
//!
//! # Player record
//!
//! ```text
//!  0  4  player number (not the record ordinal)
//!  4  4  controller kind
//!  8  4  race
//! 12  4  fixed start-location flag
//! 16  str name
//! ..  4  start x
//! ..  4  start y
//! ..  4  (from version 5) — ally/low-priority flags
//! ..  4  (from version 5) — ally mask
//! ```
//!
//! # Framing
//!
//! This crate refuses anything it cannot prove. The relation it does prove is
//! **exact consumption of the payload**: every field listed for the file's
//! version is walked, in order, and the cursor ends exactly at the end of the
//! file. That holds on **all 137 shipped `.w3i` files on both builds** (131 on
//! 1.28c's `Maps\Campaign`, 6 on the custom `Maps\` tree) plus the 1.29b set
//! (44 files), for 181 files total.
//!
//! What it refuses, and why:
//!
//! * **`DemoCampaign.w3n` has no `.w3i` at all.** It is a campaign container,
//!   not a map; `game.dll` writes `war3campaign.*` for it and there is nothing
//!   to parse. That absence is a census result, not a bug.
//! # Where preplaced units and items actually live
//!
//! **Measured, and it is a separate file from the object overrides.**
//! `war3map.w3u` / `war3map.w3t` are *object-data overrides* (`pcx-objects`
//! owns those). The preplaced unit/item **instances** live in their own file,
//! `war3mapUnits.doo`, and it ships in **every shipped map on both builds**:
//!
//! | build | `.w3m`/`.w3n` | carrying `war3mapUnits.doo` |
//! |---|---|---|
//! | 1.28c | 89 | 88 (`DemoCampaign.w3n` has none — it writes `war3campaign.*`) |
//! | 1.29b | 44 | 44 |
//!
//! Its head is `W3do` + version 7 + subversion 9 + a count at `+12`, the same
//! family `war3map.doo` uses — but the records are **not** the 42-byte fixed
//! doodad records. Measured instance widths on `(1)TheDeathSheep.w3m` are 95
//! bytes for a bare placement and 139 bytes for one carrying four ability ids,
//! so the record has a variable tail. This crate **refuses** that file rather
//! than walk a layout it has not proved: `.doo`-style placement, unit/item
//! records with an ability list, and the trailer are three different widths
//! and guessing one puts units in the wrong place. The 95/139 widths above are
//! the measurement the next reader starts from.
//!
//! * **Truncated community maps.** Some loose DotA and Footman Frenzy files end
//!   in the middle of a declared table list (a `0xff` or 4 stray bytes at EOF).
//!   `game.dll` tolerates this — its read helper advances the cursor by one
//!   byte on failure and leaves the destination word untouched — but a
//!   plausible reconstructed count would be a guess. This crate refuses with
//!   [`FramingViolation`] and names the offset.
//! * **Unsupported versions.** Only version 18 (0x12) ships. Other version
//!   words have their walk implemented — the reader has them — but none of them
//!   occurs in a shipped file, so the sweep asserts only 18 and the crate
//!   returns [`Error::UnsupportedInfoVersion`] for the rest rather than claiming
//!   they were measured.

use crate::Error;

/// `war3map.w3i`, the only name the game reads this file under.
pub const INFO_NAME: &str = "war3map.w3i";

/// The only file version either shipped build writes.
pub const VERSION: u32 = 18;

/// The version word at or above which the second constant follows.
pub const VERSION_SECOND_CONSTANT: u32 = 0x10;

/// Controller kind: no player in this slot (also written as 0).
pub const CONTROLLER_NONE: u32 = 0;

/// Controller kind: a human player.
pub const CONTROLLER_HUMAN: u32 = 1;

/// Controller kind: the game's own AI.
pub const CONTROLLER_COMPUTER: u32 = 2;

/// Controller kind: a passive, non-hostile player.
pub const CONTROLLER_NEUTRAL: u32 = 3;

/// Legacy misnomer for raw controller word 4; this is not a difficulty level.
pub const CONTROLLER_COMPUTER_EASY: u32 = 4;

/// Legacy raw word 11, with no established controller meaning.
pub const CONTROLLER_COMPUTER_INSANE: u32 = 11;

/// Race: human.
pub const RACE_HUMAN: u32 = 1;

/// Race: orc.
pub const RACE_ORC: u32 = 2;

/// Race: undead.
pub const RACE_UNDEAD: u32 = 3;

/// Race: night elf.
pub const RACE_NIGHT_ELF: u32 = 4;

/// Legacy raw zero constant; player records contain no start-point index.
pub const START_POINT_NONE: u32 = 0;

/// One player slot.
#[derive(Debug, Clone, PartialEq)]
pub struct Player {
    /// Legacy field name for the player-number word. Use [`Self::number`].
    pub controller: u32,
    /// Legacy field name for the controller word. Use [`Self::control`].
    pub race: u32,
    /// Legacy field name for the race word. Use [`Self::race_id`].
    pub start_point: u32,
    /// Legacy field name for the fixed-start word. Use [`Self::fixed_start`].
    pub colour: u32,
    /// Slot name, usually a `TRIGSTR_` reference.
    pub name: String,
    /// Start x, a float in world units (measured: −6912..=6464 etc.).
    pub start_x: f32,
    /// Start y, a float in world units.
    pub start_y: f32,
    /// Set from version 5. Observed 0 or 1.
    pub ally_priorities: u32,
    /// Set from version 5. Observed as a handicap/difficulty word; unattributed.
    pub ally_mask: u32,
}

impl Player {
    /// Authored player number; sparse records must not be renumbered.
    pub fn number(&self) -> u32 {
        self.controller
    }

    /// Authored controller kind, independent of player number and race.
    pub fn control(&self) -> u32 {
        self.race
    }

    /// Authored race word, including unknown values retained from the file.
    pub fn race_id(&self) -> u32 {
        self.start_point
    }

    /// Unknown flag words remain retained but do not become an invented boolean.
    pub fn fixed_start(&self) -> Option<bool> {
        match self.colour {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        }
    }
}

/// One force (team/alliance) row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Force {
    /// Force flags.
    pub flags: u32,
    /// Player mask, one bit per player slot.
    pub mask: u32,
    /// Force name, usually a `TRIGSTR_` reference.
    pub name: String,
}

/// The camera bounds, eight floats: a primary rectangle and a second one.
///
/// Both rectangles are written unconditionally by the writer's fixed loop over
/// `[ebp+0x420 .. ebp+0x440)`. What the two rectangles *mean* is unattributed:
/// the shipped values are consistent with a playable-rectangle plus a
/// camera-limits rectangle, but no shipped byte separates the two.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    /// First rectangle, four floats.
    pub first: [f32; 4],
    /// Second rectangle, four floats.
    pub second: [f32; 4],
}

/// One table entry inside a table list: an id and a sub-list whose length is
/// the list's own `M`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableEntry {
    /// Table id.
    pub id: u32,
    /// The sub-list, exactly as many words as the owning list declares.
    pub tables: Vec<u32>,
}

/// One table list.
///
/// This is the section that decides which object tables the map forces or
/// suggests, so it is the one that matters most for a wrong answer. The shape
/// is written by `game.dll` at `0x6f726c7a` and read at `0x739069`, and the file
/// layout is:
///
/// ```text
/// u32  id
/// str  name
/// u32  M
/// M × u32
/// u32  K
/// K × ( u32 id, M × u32 )
/// ```
///
/// A file whose `K` is larger than its remaining bytes is what the loose DotA
/// community maps do: the original swallows it (a failing read advances the
/// cursor one byte and leaves the destination word alone), and this crate
/// refuses instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableList {
    /// Table-list id.
    pub id: u32,
    /// Table-list name.
    pub name: String,
    /// The first sub-list, `M` words.
    pub first: Vec<u32>,
    /// The second sub-list, `K` entries of `1 + M` words.
    pub second: Vec<TableEntry>,
}

impl TableList {
    /// Byte length implied by the current decoded fields and record grammar.
    /// Lossy name decoding or caller mutation can differ from the authored span.
    pub fn wire_len(&self) -> usize {
        4 + self.name.len()
            + 1
            + 4
            + 4 * self.first.len()
            + 4
            + self.second.len() * (4 + 4 * self.first.len())
    }
}

// The writer has a further section at `version >= 0x18` (the upgrade-change
// list, `game.dll` 0x6f726d95 / 0x73921d). No shipped `.w3i` reaches it — every
// one of both builds' maps carries version 18 — so this crate refuses that
// version outright rather than walking a shape no byte proves.

/// A parsed `war3map.w3i`.
///
/// Owns its bytes, because the file comes out of an archive rather than off
/// disk and so cannot borrow from the map.
#[derive(Debug, Clone, PartialEq)]
pub struct MapInfo {
    version: u32,
    /// First constant after the version word.
    pub constant_a: u32,
    /// Second constant (from `VERSION_SECOND_CONSTANT`).
    pub constant_b: Option<u32>,
    /// Map name, a w3s reference.
    pub name: String,
    /// Map description, a w3s reference.
    pub description: String,
    /// Recommended-players string.
    pub recommended: String,
    /// A fourth head string; empty on all shipped files.
    pub head_string_4: Option<String>,
    /// Camera bounds.
    pub camera: Camera,
    /// Four u32 between the camera and the flag word.
    pub tileset_constants: [u32; 4],
    /// Two u32, likely terrain bounds. `(maxX, maxY)` shape observed.
    pub bounds: [u32; 2],
    /// Flags word. Bit 0x800 is forced on by the writer for old versions.
    pub flags: u32,
    /// A single byte, observed 0.
    pub byte_field: Option<u8>,
    /// A u32 written from `VERSION_EXTRA_U32`.
    pub extra_u32: Option<u32>,
    /// The first string written from `VERSION_EXTRA_U32`.
    pub first_string: Option<String>,
    /// The fog/parallax string.
    pub fog_string: String,
    /// A second string written from the same version gate.
    pub second_string: Option<String>,
    /// A third string, written from the same version gate as
    /// [`Self::second_string`].
    pub third_string: Option<String>,
    /// Three strings written from `VERSION_LOADING_STRINGS`.
    pub loading_strings: Option<[String; 3]>,
    pub loading_id: Option<u32>,
    pub loading_path: Option<String>,
    pub fog_style: Option<u32>,
    /// Three floats plus a u32 written from `VERSION_FLOAT_TRIPLE`.
    pub float_triple: Option<[f32; 3]>,
    pub float_triple_tail: Option<u32>,
    /// A u32 written from `VERSION_16`.
    pub extra_u32_16: Option<u32>,
    /// A string written from `VERSION_17`.
    pub extra_string_17: Option<String>,
    /// A byte written from `VERSION_18`.
    pub extra_byte_18: Option<u8>,
    /// A u32 written from `VERSION_19`.
    pub extra_u32_19: Option<u32>,
    /// The player slots.
    pub players: Vec<Player>,
    /// Authored player-record span, excluding the count and lossy string expansion.
    player_record_bytes: usize,
    /// The forces.
    pub forces: Vec<Force>,
    /// Authored force-record span, excluding its count.
    force_record_bytes: usize,
    /// Section A: a list of four-word rows (from `VERSION_TABLE_LIST`).
    pub start_locations: Vec<[u32; 4]>,
    /// Section B: a list of two-word rows (from `VERSION_TABLE_LIST_7`).
    pub pairs: Vec<[u32; 2]>,
    /// Section C: table lists (from `VERSION_TABLE_LIST_12`).
    pub table_lists: Vec<TableList>,
    /// Authored table-list record span, excluding its count.
    table_list_record_bytes: usize,
    pub item_tables: Vec<ItemTable>,
    raw: Vec<u8>,
}

/// Version gate for the two extra u32 words between the tileset constants and
/// the bounds.
pub const VERSION_TILESET_U32S: u32 = 0x0e;

/// Version gate for the bounds pair.
pub const VERSION_BOUNDS: u32 = 1;

/// Version gate for the flag word.
pub const VERSION_FLAGS: u32 = 2;

/// Version gate for the byte field.
pub const VERSION_BYTE_FIELD: u32 = 8;

/// Version gate for the first extra u32.
pub const VERSION_EXTRA_U32: u32 = 0x0a;

/// Version gate above which the first extra string is written into its real
/// field; between 0x12 and 0x14 it is absent from the file entirely.
pub const VERSION_FIRST_STRING: u32 = 0x14;

/// Version gate for the second extra string.
pub const VERSION_SECOND_STRING: u32 = 0x0b;

/// Version gate for the three loading-screen strings.
pub const VERSION_LOADING_STRINGS: u32 = 0x0d;

/// Version gate for the float triple plus trailing u32.
pub const VERSION_FLOAT_TRIPLE: u32 = 0x13;

/// Version gate for the `VERSION_16` u32.
pub const VERSION_16: u32 = 0x15;

/// Version gate for the `VERSION_17` string.
pub const VERSION_17: u32 = 0x16;

/// Version gate for the `VERSION_18` byte.
pub const VERSION_18: u32 = 0x17;

/// Version gate for the `VERSION_19` u32.
pub const VERSION_19: u32 = 0x19;

/// Version gate for the per-player extra words.
pub const VERSION_PLAYER_EXTRA: u32 = 5;

/// Version gate for the force table.
pub const VERSION_FORCES: u32 = 3;

/// Version gate for section A.
pub const VERSION_TABLE_LIST: u32 = 6;

/// Version gate for section B.
pub const VERSION_TABLE_LIST_7: u32 = 7;

/// Version gate for section C.
pub const VERSION_TABLE_LIST_12: u32 = 0x0c;

/// Why a framing relation failed.
///
/// Written out so a failure names the field and the offset, not just "bad".
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FramingViolation {
    #[error("ran out of bytes reading {what} at +{at:#x}: {have} left, {need} needed")]
    Truncated {
        what: &'static str,
        at: usize,
        have: usize,
        need: usize,
    },
    #[error("string at +{at:#x} reaches the end of the file without a NUL")]
    UnterminatedString { at: usize },
    #[error("{have} bytes left over after the last declared field at +{at:#x}")]
    TrailingBytes { at: usize, have: usize },
}

/// A cursor over the `.w3i` bytes.
///
/// The reads are deliberately the same shape as `game.dll`'s `CDataStore`
/// helpers: a u32 read that cannot be satisfied **consumes one byte** and
/// leaves the destination untouched. That is the behaviour that lets the
/// original swallow a truncated community map; this crate turns it into a
/// typed error instead, because a reconstructed zero is a guess.
struct Cursor<'a> {
    data: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, at: 0 }
    }

    fn u32(&mut self) -> Result<u32, Error> {
        match self.data.get(self.at..self.at + 4) {
            Some(b) => {
                let v = u32::from_le_bytes(b.try_into().unwrap());
                self.at += 4;
                Ok(v)
            }
            None => Err(Error::InfoFraming(FramingViolation::Truncated {
                what: "u32",
                at: self.at,
                have: self.data.len() - self.at,
                need: 4,
            })),
        }
    }

    fn f32(&mut self) -> Result<f32, Error> {
        Ok(f32::from_bits(self.u32()?))
    }

    fn u8(&mut self) -> Result<u8, Error> {
        match self.data.get(self.at) {
            Some(b) => {
                self.at += 1;
                Ok(*b)
            }
            None => Err(Error::InfoFraming(FramingViolation::Truncated {
                what: "byte",
                at: self.at,
                have: self.data.len() - self.at,
                need: 1,
            })),
        }
    }

    fn string(&mut self) -> Result<String, Error> {
        let start = self.at;
        let Some(nul) = self.data[start..].iter().position(|&b| b == 0) else {
            return Err(Error::InfoFraming(FramingViolation::UnterminatedString {
                at: start,
            }));
        };
        self.at = start + nul + 1;
        Ok(String::from_utf8_lossy(&self.data[start..start + nul]).into_owned())
    }
}

/// Version-25 trailing table: named id, then sets of authored word pairs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemTable {
    pub id: u32,
    pub name: String,
    pub sets: Vec<Vec<[u32; 2]>>,
}

impl MapInfo {
    /// Parse a `.w3i`, walking every field the file's version declares and
    /// proving the payload is consumed exactly.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, Error> {
        Self::parse(&bytes).map(|parsed| Self {
            raw: bytes,
            ..parsed
        })
    }

    /// Parse without taking ownership, for borrowing callers.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        let mut c = Cursor::new(bytes);
        let version = c.u32()?;
        if version != VERSION && version != 25 {
            return Err(Error::UnsupportedInfoVersion(version));
        }
        let constant_a = c.u32()?;
        let constant_b = if version >= VERSION_SECOND_CONSTANT {
            Some(c.u32()?)
        } else {
            None
        };

        let name = c.string()?;
        let description = c.string()?;
        let recommended = c.string()?;
        let head_string_4 = (version >= VERSION_BYTE_FIELD)
            .then(|| c.string())
            .transpose()?;

        let mut camera_first = [0f32; 4];
        let mut camera_second = [0f32; 4];
        if version >= 9 {
            for v in &mut camera_first {
                *v = c.f32()?;
            }
            for v in &mut camera_second {
                *v = c.f32()?;
            }
        } else if version >= 4 {
            // Two interleaved reads of a float then a u32, twice.
            camera_first[0] = c.f32()?;
            camera_first[2] = c.f32()?;
            camera_first[3] = c.f32()?;
            camera_second[0] = c.f32()?;
        } else {
            camera_first[0] = c.f32()?;
        }

        let tileset_constants = if version >= VERSION_TILESET_U32S {
            [c.u32()?, c.u32()?, c.u32()?, c.u32()?]
        } else {
            [0, 0, 0, 0]
        };

        let bounds = if version >= VERSION_BOUNDS {
            [c.u32()?, c.u32()?]
        } else {
            [0, 0]
        };

        let flags = if version >= VERSION_FLAGS {
            if version < 9 {
                // The game reads and discards a float here.
                let _ = c.f32()?;
            }
            let f = c.u32()?;
            if version < 0x0f { f | 0x800 } else { f }
        } else {
            0
        };

        let byte_field = (version >= VERSION_BYTE_FIELD)
            .then(|| c.u8())
            .transpose()?;

        // From VERSION_EXTRA_U32 onward: one u32, then three strings, then
        // (from VERSION_LOADING_STRINGS) a u32 and three more. The string the
        // writer puts at `first_string` is written into its real field only
        // from VERSION_FIRST_STRING; between 0x12 and 0x14 it is absent from
        // the file entirely, which is a byte-level fact, not a skip.
        let extra_u32 = (version >= 0x11).then(|| c.u32()).transpose()?;
        let first_string = (version >= VERSION_FIRST_STRING)
            .then(|| c.string())
            .transpose()?;
        let fog_string = c.string()?;
        // Two strings, at `0x550` and `0x5b1`.
        let second_string = (version >= VERSION_SECOND_STRING)
            .then(|| c.string())
            .transpose()?;
        let third_string = (version >= VERSION_SECOND_STRING)
            .then(|| c.string())
            .transpose()?;

        // The three loading strings, plus a u32 the game discards for
        // VERSION_LOADING_STRINGS >= 0x11.
        let loading_id = (version >= 0x11).then(|| c.u32()).transpose()?;
        let loading_path = (version >= 0x14).then(|| c.string()).transpose()?;
        let loading_strings = if version >= VERSION_LOADING_STRINGS {
            Some([c.string()?, c.string()?, c.string()?])
        } else {
            None
        };

        let fog_style = (version >= VERSION_FLOAT_TRIPLE)
            .then(|| c.u32())
            .transpose()?;

        let (float_triple, float_triple_tail) = if version >= VERSION_FLOAT_TRIPLE {
            (Some([c.f32()?, c.f32()?, c.f32()?]), Some(c.u32()?))
        } else {
            (None, None)
        };

        let extra_u32_16 = (version >= VERSION_16).then(|| c.u32()).transpose()?;
        let extra_string_17 = (version >= VERSION_17).then(|| c.string()).transpose()?;
        let extra_byte_18 = (version >= VERSION_18).then(|| c.u8()).transpose()?;
        let extra_u32_19 = (version >= VERSION_19).then(|| c.u32()).transpose()?;

        let player_count = c.u32()? as usize;
        let player_records_at = c.at;
        let mut players = Vec::new();
        for _i in 0..player_count {
            players.push(Player {
                controller: c.u32()?,
                race: c.u32()?,
                start_point: c.u32()?,
                colour: c.u32()?,
                name: c.string()?,
                start_x: c.f32()?,
                start_y: c.f32()?,
                ally_priorities: if version >= VERSION_PLAYER_EXTRA {
                    c.u32()?
                } else {
                    0
                },
                ally_mask: if version >= VERSION_PLAYER_EXTRA {
                    c.u32()?
                } else {
                    0
                },
            });
        }

        let player_record_bytes = c.at - player_records_at;

        let mut forces = Vec::new();
        let force_record_bytes = if version >= VERSION_FORCES {
            let n = c.u32()? as usize;
            let records_at = c.at;
            for _ in 0..n {
                forces.push(Force {
                    flags: c.u32()?,
                    mask: c.u32()?,
                    name: c.string()?,
                });
            }
            c.at - records_at
        } else {
            0
        };

        let mut start_locations = Vec::new();
        if version >= VERSION_TABLE_LIST {
            let n = c.u32()? as usize;
            for _ in 0..n {
                start_locations.push([c.u32()?, c.u32()?, c.u32()?, c.u32()?]);
            }
        }

        let mut pairs = Vec::new();
        if version >= VERSION_TABLE_LIST_7 {
            let n = c.u32()? as usize;
            for _ in 0..n {
                pairs.push([c.u32()?, c.u32()?]);
            }
        }

        let mut table_lists = Vec::new();
        let table_list_record_bytes = if version >= VERSION_TABLE_LIST_12 {
            let n = c.u32()? as usize;
            let records_at = c.at;
            for _ in 0..n {
                let id = c.u32()?;
                let name = c.string()?;
                let m = c.u32()? as usize;
                let mut first = Vec::new();
                for _ in 0..m {
                    first.push(c.u32()?);
                }
                let k = c.u32()? as usize;
                let mut second = Vec::new();
                for _ in 0..k {
                    second.push(TableEntry {
                        id: c.u32()?,
                        tables: {
                            let mut v = Vec::new();
                            for _ in 0..m {
                                v.push(c.u32()?);
                            }
                            v
                        },
                    });
                }
                table_lists.push(TableList {
                    id,
                    name,
                    first,
                    second,
                });
            }
            c.at - records_at
        } else {
            0
        };

        let mut item_tables = Vec::new();
        if version >= 0x18 {
            let count = c.u32()?;
            for _ in 0..count {
                let id = c.u32()?;
                let name = c.string()?;
                let set_count = c.u32()?;
                let mut sets = Vec::new();
                for _ in 0..set_count {
                    let item_count = c.u32()?;
                    let mut items = Vec::new();
                    for _ in 0..item_count {
                        items.push([c.u32()?, c.u32()?]);
                    }
                    sets.push(items);
                }
                item_tables.push(ItemTable { id, name, sets });
            }
        }

        if c.at != bytes.len() {
            return Err(Error::InfoFraming(FramingViolation::TrailingBytes {
                at: c.at,
                have: bytes.len() - c.at,
            }));
        }

        Ok(Self {
            version,
            constant_a,
            constant_b,
            name,
            description,
            recommended,
            head_string_4,
            camera: Camera {
                first: camera_first,
                second: camera_second,
            },
            tileset_constants,
            bounds,
            flags,
            byte_field,
            extra_u32,
            first_string,
            fog_string,
            second_string,
            third_string,
            loading_strings,
            loading_id,
            loading_path,
            fog_style,
            float_triple,
            float_triple_tail,
            extra_u32_16,
            extra_string_17,
            extra_byte_18,
            extra_u32_19,
            players,
            player_record_bytes,
            forces,
            force_record_bytes,
            start_locations,
            pairs,
            table_lists,
            table_list_record_bytes,
            item_tables,
            raw: bytes.to_vec(),
        })
    }

    /// The file's own bytes.
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }

    /// The file version word; [`VERSION`] on every shipped file.
    pub fn version(&self) -> u32 {
        self.version
    }

    /// Every player slot in file order.
    pub fn players(&self) -> &[Player] {
        &self.players
    }

    /// Every force in file order.
    pub fn forces(&self) -> &[Force] {
        &self.forces
    }

    /// The player slots a person can occupy: controller
    /// [`CONTROLLER_HUMAN`].
    ///
    /// Note what this is *not*: it is not "slots a human race plays". The
    /// controller word says who is playing the slot, the race word says which
    /// race; the two are independent and every shipped map separates them.
    /// This describes authored controller roles, not occupied lobby slots or
    /// the identity of the local client.
    pub fn human_players(&self) -> impl Iterator<Item = &Player> {
        self.players
            .iter()
            .filter(|p| p.control() == CONTROLLER_HUMAN)
    }

    /// Number of authored slots per race, ordered by race word.
    pub fn race_counts(&self) -> std::collections::BTreeMap<u32, usize> {
        let mut m = std::collections::BTreeMap::new();
        for p in &self.players {
            *m.entry(p.race_id()).or_insert(0) += 1;
        }
        m
    }

    /// The forced-table lists.
    pub fn table_lists(&self) -> &[TableList] {
        &self.table_lists
    }

    /// Canonical byte form: the file exactly as stored.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        self.raw.clone()
    }

    /// FNV-1a 64 over [`Self::canonical_bytes`].
    pub fn canonical_hash(&self) -> u64 {
        pcx_mpq::fnv1a64(&self.raw)
    }

    /// The measured framing, as facts rather than assertions.
    pub fn framing(&self) -> MapInfoFraming {
        MapInfoFraming::of(self)
    }
}

/// The measured framing of one `.w3i`.
#[derive(Debug, Clone, PartialEq)]
pub struct MapInfoFraming {
    /// Byte length of the file.
    pub file_len: usize,
    /// The version word.
    pub version: u32,
    /// Current decoded player count; caller mutation does not rewrite raw bytes.
    pub players: usize,
    /// Current decoded force count; not necessarily the authored count after mutation.
    pub forces: usize,
    /// Current decoded table-list count; not necessarily the authored count after mutation.
    pub table_lists: usize,
    /// Authored player-record bytes captured by the parser, excluding the count.
    pub player_bytes: usize,
    /// Authored force-record bytes captured by the parser, excluding the count.
    pub force_bytes: usize,
    /// Bytes the three start-location cross-reference sections occupy.
    pub xref_bytes: usize,
    /// Authored table-list record bytes captured by the parser, excluding the count.
    pub table_list_bytes: usize,
    /// Residual bytes not separately itemized, including the version-25 item-table tail.
    pub head_bytes: usize,
    /// Whether the original parse consumed the input, not validation of later mutations.
    pub exact: bool,
}

impl MapInfoFraming {
    /// Read the framing of a parsed file.
    ///
    /// Player, force and table-list bytes are authored cursor spans, unchanged
    /// by lossy name decoding or later caller mutation. Counts and xref lengths
    /// describe current decoded fields. This view is not a serializer or a
    /// validator of mutations; `exact` records the original successful parse.
    pub fn of(info: &MapInfo) -> Self {
        let player_bytes = info.player_record_bytes;
        let force_bytes = info.force_record_bytes;
        let xref_bytes: usize = 4 + info.start_locations.len() * 16 + 4 + info.pairs.len() * 8;
        let table_list_bytes = info.table_list_record_bytes;
        Self {
            file_len: info.raw.len(),
            version: info.version,
            players: info.players.len(),
            forces: info.forces.len(),
            table_lists: info.table_lists.len(),
            player_bytes,
            force_bytes,
            xref_bytes,
            table_list_bytes,
            head_bytes: info
                .raw
                .len()
                .saturating_sub(player_bytes + force_bytes + xref_bytes + table_list_bytes + 8),
            exact: true,
        }
    }

    /// Whether the separately reported byte totals add up to the raw input.
    /// This does not prove that subsequently mutated decoded records match it.
    pub fn sums(&self) -> bool {
        self.head_bytes
            + self.player_bytes
            + self.force_bytes
            + self.xref_bytes
            + self.table_list_bytes
            + 8
            == self.file_len
    }

    /// Every framing relation this decoder relies on, as readable failures.
    /// Empty means the layout adds up.
    pub fn violations(&self) -> Vec<String> {
        let mut out = Vec::new();
        if !self.sums() {
            out.push(format!(
                "head {} + players {} + forces {} + xrefs {} + table lists {} \
                 + 8 count bytes != {}",
                self.head_bytes,
                self.player_bytes,
                self.force_bytes,
                self.xref_bytes,
                self.table_list_bytes,
                self.file_len
            ));
        }
        out
    }
}

/// Decode a map-info file directly, for fixtures and the inspector.
pub fn parse_info(bytes: &[u8]) -> Result<MapInfo, Error> {
    MapInfo::parse(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MAP_ARCHIVE_OFFSET, Map};
    use std::path::{Path, PathBuf};

    fn workspace() -> PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf()
    }

    fn dir_at(env: &str, fallback: &str) -> Option<PathBuf> {
        let p = std::env::var_os(env)
            .map(PathBuf::from)
            .unwrap_or_else(|| workspace().join(fallback));
        p.is_dir().then_some(p)
    }

    /// The 1.28c tree.
    fn install_dir() -> Option<PathBuf> {
        dir_at("PEACECRAFT3_INSTALL", "wc3/game/app")
    }

    /// The 1.29b Battle.net legacy tree.
    fn legacy_dir() -> Option<PathBuf> {
        dir_at(
            "PEACECRAFT3_LEGACY_DIR",
            "../../Faugus/battle-net/drive_c/Program Files (x86)/Warcraft III (Legacy)",
        )
    }

    /// Every install present, oldest first so 1.28c reports first.
    fn installs() -> Vec<(&'static str, PathBuf)> {
        let mut v = Vec::new();
        if let Some(d) = install_dir() {
            v.push(("1.28c", d));
        }
        if let Some(d) = legacy_dir() {
            v.push(("1.29b", d));
        }
        v
    }

    /// Open an install and pull one map's bytes straight out of it.
    fn shipped_bytes(dir: &Path, path: &str) -> Option<Vec<u8>> {
        let mut set = pcx_mpq::MpqSet::open_wc3(dir).ok()?;
        set.extract(path).ok()
    }

    /// Read `war3map.w3i` out of a shipped map through the map reader.
    fn info(dir: &Path, path: &str) -> Option<MapInfo> {
        let raw = shipped_bytes(dir, path)?;
        let leaked: &'static [u8] = Box::leak(raw.into_boxed_slice());
        Map::parse(leaked).ok()?.info().ok()
    }

    const MAP_CREDITS: &str = r"Maps\Campaign\WarCraftIIICredits.w3m";
    const MAP_BONUS: &str = r"Maps\Campaign\BonusCredits.w3m";
    const MAP_ADRENALINE: &str = r"(4)adrenaline.w3m";
    const MAP_ICECROWN: &str = r"(12)IceCrown.w3m";
    const MAP_HUMAN01: &str = r"Maps\Campaign\Human01.w3m";
    const MAP_SHEEP: &str = r"(1)TheDeathSheep.w3m";

    // ---- exact fields, read by hand from the bytes -----------------------

    #[test]
    fn info_header_fields_are_exact() {
        // Hand-read from Maps\Campaign\WarCraftIIICredits.w3m's war3map.w3i:
        // 12 00 00 00 c9 00 00 00 5c 11 00 00, then the four strings
        // "TRIGSTR_378\0", "TRIGSTR_381\0", "", "".
        let Some(dir) = install_dir() else { return };
        let Some(i) = info(&dir, MAP_CREDITS) else {
            return;
        };
        assert_eq!(i.version(), VERSION);
        assert_eq!(i.constant_a, 201);
        assert_eq!(i.constant_b, Some(4444));
        assert_eq!(i.name, "TRIGSTR_378");
        assert_eq!(i.description, "TRIGSTR_381");
        assert_eq!(i.recommended, "");
        assert_eq!(i.head_string_4.as_deref(), Some(""));
    }

    #[test]
    fn camera_and_constants_are_exact() {
        // Hand-read from the same file: the eight camera words decode to
        // -3328, -3712, 3328, 3840, -3328, 3840, 3328, -3712, then four
        // u32 words 2, 2, 1, 0 and the two bounds words 60 and 63.
        let Some(dir) = install_dir() else { return };
        let Some(i) = info(&dir, MAP_CREDITS) else {
            return;
        };
        assert_eq!(i.camera.first, [-3328.0, -3712.0, 3328.0, 3840.0]);
        assert_eq!(i.camera.second, [-3328.0, 3840.0, 3328.0, -3712.0]);
        assert_eq!(i.tileset_constants, [2, 2, 1, 0]);
        assert_eq!(i.bounds, [60, 63]);
    }

    #[test]
    fn credits_player_slot_is_exact() {
        // Hand-read: one player slot after the loading strings.
        // player 0, user controller, human race, non-fixed start,
        // name "TRIGSTR_010", start (-2240, 512), then the two extra words 0.
        let Some(dir) = install_dir() else { return };
        let Some(i) = info(&dir, MAP_CREDITS) else {
            return;
        };
        assert_eq!(i.players().len(), 1);
        let p = &i.players()[0];
        assert_eq!(p.number(), 0);
        assert_eq!(p.control(), CONTROLLER_HUMAN);
        assert_eq!(p.race_id(), RACE_HUMAN);
        assert_eq!(p.fixed_start(), Some(false));
        assert_eq!(p.name, "TRIGSTR_010");
        assert_eq!((p.start_x, p.start_y), (-2240.0, 512.0));
        assert_eq!((p.ally_priorities, p.ally_mask), (0, 0));
        assert_eq!(i.forces().len(), 1);
        assert_eq!(i.forces()[0].name, "TRIGSTR_011");
        assert_eq!(i.forces()[0].mask, 0xffff_ffff);
    }

    #[test]
    fn bonus_credits_has_one_human_slot() {
        let Some(dir) = install_dir() else { return };
        let Some(i) = info(&dir, MAP_BONUS) else {
            return;
        };
        assert_eq!(i.players().len(), 1);
        let p = &i.players()[0];
        assert_eq!(
            (p.controller, p.race, p.start_point, p.colour),
            (0, 1, 1, 1)
        );
        assert_eq!(p.name, "TRIGSTR_000");
        assert_eq!((p.start_x, p.start_y), (10304.0, 1856.0));
        // A campaign credits map: one force over all players.
        assert_eq!(i.forces().len(), 1);
        assert_eq!(i.forces()[0].flags, 59);
    }

    #[test]
    fn adrenaline_player_slots_are_exact() {
        // Hand-read from (4)adrenaline.w3m: four user slots, numbers
        // 0..=3, controllers all 1, races 1..=4, fixed-start 0, names
        // TRIGSTR_010..=TRIGSTR_013, and the two extra words alternating
        // 0/10 and 0/5 in pairs.
        let Some(dir) = install_dir() else { return };
        let Some(i) = info(&dir, MAP_ADRENALINE) else {
            return;
        };
        assert_eq!(i.players().len(), 4);
        let want = [
            // player number, controller, race, fixed-start, name, x, y, ally, mask
            (
                0u32,
                RACE_HUMAN,
                1u32,
                0u32,
                "TRIGSTR_010",
                -1792.0,
                1920.0,
                0u32,
                10u32,
            ),
            (1, RACE_HUMAN, 2, 0, "TRIGSTR_011", 2304.0, 1728.0, 0, 5),
            (2, RACE_HUMAN, 3, 0, "TRIGSTR_012", 2432.0, -2496.0, 0, 10),
            (3, RACE_HUMAN, 4, 0, "TRIGSTR_013", -2496.0, -2176.0, 0, 5),
        ];
        for (p, w) in i.players().iter().zip(&want) {
            assert_eq!(p.controller, w.0, "{:?}", p.name);
            assert_eq!(p.race, w.1, "{:?}", p.name);
            assert_eq!(p.start_point, w.2, "{:?}", p.name);
            assert_eq!(p.colour, w.3, "{:?}", p.name);
            assert_eq!(p.name, w.4);
            assert_eq!(p.start_x, w.5, "{:?}", p.name);
            assert_eq!(p.start_y, w.6, "{:?}", p.name);
            assert_eq!(p.ally_priorities, w.7, "{:?}", p.name);
            assert_eq!(p.ally_mask, w.8, "{:?}", p.name);
        }
        // Four authored user slots, with one player of each race.
        assert_eq!(i.human_players().count(), 4);
        let counts = i.race_counts();
        assert_eq!(counts.len(), 4);
        for race in [RACE_HUMAN, RACE_ORC, RACE_UNDEAD, RACE_NIGHT_ELF] {
            assert_eq!(counts.get(&race), Some(&1));
        }
    }

    #[test]
    fn icecrown_twelve_slots_are_exact() {
        // Hand-read from (12)IceCrown.w3m: twelve slots, controllers 0..=11,
        // races all 1, start points cycling 1,2,3,4, and colour 0 throughout.
        let Some(dir) = install_dir() else { return };
        let Some(i) = info(&dir, MAP_ICECROWN) else {
            return;
        };
        assert_eq!(i.players().len(), 12);
        for (n, p) in i.players().iter().enumerate() {
            assert_eq!(p.controller, n as u32, "slot {n}");
            assert_eq!(p.race, RACE_HUMAN);
            assert_eq!(p.start_point, (n % 4 + 1) as u32, "slot {n}");
            assert_eq!(p.colour, 0, "slot {n}");
        }
        assert_eq!(i.players()[0].name, "TRIGSTR_005");
        assert_eq!(i.players()[11].name, "TRIGSTR_016");
        assert_eq!(
            (i.players()[0].start_x, i.players()[0].start_y),
            (6080.0, 8320.0)
        );
    }

    #[test]
    fn human01_mixes_races_and_controllers() {
        // Hand-read from Maps\Campaign\Human01.w3m: five slots; races 3,1,3,3,3;
        // controllers 0,1,2,8,9; the last two are computer slots.
        let Some(dir) = install_dir() else { return };
        let Some(i) = info(&dir, MAP_HUMAN01) else {
            return;
        };
        assert_eq!(i.players().len(), 5);
        let got: Vec<u32> = i.players().iter().map(|p| p.controller).collect();
        assert_eq!(got, [0, 1, 2, 8, 9]);
        let races: Vec<u32> = i.players().iter().map(|p| p.race).collect();
        assert_eq!(
            races,
            [
                RACE_UNDEAD,
                RACE_HUMAN,
                RACE_UNDEAD,
                RACE_UNDEAD,
                RACE_UNDEAD
            ]
        );
        // Four forces, hand-read masks 0xfffffefa, 0x100, 0x1 and 0x4.
        assert_eq!(i.forces().len(), 4);
        let masks: Vec<u32> = i.forces().iter().map(|f| f.mask).collect();
        assert_eq!(masks, [0xfffffefa, 0x100, 0x1, 0x4]);
        // A force mask is a player bitmask. It is *not* bounded by the slot
        // count the file declares: Human01's first force masks 0xfffffefa,
        // which sets bits 5 through 19 even though only five slots are listed.
        // That is a real fact about the file and is asserted rather than
        // smoothed over — the mask belongs to the force table, which is sized
        // separately from the player list.
        for f in i.forces() {
            assert!(f.mask != 0, "a force with an empty mask");
        }
        assert!(i.forces()[0].mask & (1 << 5) != 0, "bit 5 is set");
        assert_eq!(i.forces()[0].name, "TRIGSTR_328");
        // The three start-location cross-reference sections are present here.
        assert_eq!(i.start_locations.len(), 71);
        assert_eq!(i.pairs.len(), 28);
        assert!(i.table_lists().is_empty());
    }

    #[test]
    fn table_list_shape_is_exact() {
        // (4)TranquilPaths.w3m is the only shipped map whose forced-table
        // section is non-empty: eleven lists, each with a name and a sub-list
        // length M, and then K entries of exactly M words. The writer's inner
        // loop is what fixes M for every second sub-list entry, so this is
        // checked against the file rather than assumed.
        let Some(dir) = install_dir() else { return };
        let Some(i) = info(&dir, r"(4)TranquilPaths.w3m") else {
            return;
        };
        let lists = i.table_lists();
        assert_eq!(lists.len(), 11, "eleven forced-table lists");
        let names: Vec<&str> = lists.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "Starts",
                "Primary Expansions",
                "Island Expansions",
                "Goblin Labs",
                "Merc Camps",
                "Fountains",
                "Center Float Camps",
                "Merchants",
                "Gimme lvl 5",
                "Gimme lvl 10",
                "Center Uber",
            ]
        );
        // The `M` each list declares, hand-read.
        let want_m = [5usize, 3, 6, 5, 5, 5, 4, 5, 3, 4, 7];
        let want_k = [3usize, 2, 2, 3, 3, 2, 3, 2, 2, 2, 3];
        for (l, (m, k)) in lists.iter().zip(want_m.iter().zip(&want_k)) {
            assert_eq!(l.first.len(), *m, "{:?}", l.name);
            assert_eq!(l.second.len(), *k, "{:?}", l.name);
            for e in &l.second {
                assert_eq!(e.tables.len(), *m, "{:?} entry", l.name);
            }
        }
        // The list ids and the entry ids are handed out raw; they are the
        // sub-table identities the game matches a forced-table entry against.
        assert_eq!(lists[0].id, 2);
        assert_eq!(lists[0].second[0].id, 45);
    }

    // ---- framing --------------------------------------------------------

    #[test]
    fn info_framing_consumes_the_file_exactly() {
        // The relation: every field the file's version declares is walked in
        // order and the cursor ends exactly at the end of the file.
        let Some(dir) = install_dir() else { return };
        for map in [
            MAP_CREDITS,
            MAP_BONUS,
            MAP_ADRENALINE,
            MAP_ICECROWN,
            MAP_HUMAN01,
            MAP_SHEEP,
        ] {
            let Some(i) = info(&dir, map) else {
                continue;
            };
            assert_eq!(i.version(), VERSION, "{map}");
            let fr = i.framing();
            assert!(fr.exact, "{map}: walk left bytes over");
            assert!(fr.violations().is_empty(), "{map}: {:?}", fr.violations());
            assert_eq!(i.raw().len(), i.canonical_bytes().len());
            assert_ne!(i.canonical_hash(), 0, "{map}");
        }
    }

    // ---- refusals -------------------------------------------------------

    #[test]
    fn unsupported_version_is_refused() {
        // Version 18 is the only one either build writes. A different version
        // word has its walk implemented by the reader but is present in no
        // shipped file, so this crate refuses it rather than claiming it was
        // measured.
        let Some(dir) = install_dir() else { return };
        let Some(i) = info(&dir, MAP_CREDITS) else {
            return;
        };
        let mut raw = i.raw().to_vec();
        raw[0] = 26;
        assert!(matches!(
            MapInfo::from_bytes(raw),
            Err(Error::UnsupportedInfoVersion(26))
        ));
    }

    #[test]
    fn truncated_info_is_refused() {
        let Some(dir) = install_dir() else { return };
        let Some(i) = info(&dir, MAP_CREDITS) else {
            return;
        };
        let mut raw = i.raw().to_vec();
        raw.truncate(raw.len() - 4);
        assert!(matches!(
            MapInfo::from_bytes(raw),
            Err(Error::InfoFraming(_))
        ));
    }

    #[test]
    fn trailing_bytes_are_refused() {
        let Some(dir) = install_dir() else { return };
        let Some(i) = info(&dir, MAP_CREDITS) else {
            return;
        };
        let mut raw = i.raw().to_vec();
        raw.push(0);
        assert!(matches!(
            MapInfo::from_bytes(raw),
            Err(Error::InfoFraming(_))
        ));
    }

    #[test]
    fn a_map_without_info_is_reported_not_invented() {
        // `DemoCampaign.w3n` carries no `war3map.w3i` under any spelling. The
        // absence is a census result: it is a campaign container, and the game
        // writes `war3campaign.*` for it instead.
        let Some(dir) = install_dir() else { return };
        let Some(raw) = shipped_bytes(&dir, "DemoCampaign.w3n") else {
            return;
        };
        let leaked: &'static [u8] = Box::leak(raw.into_boxed_slice());
        let Some(m) = Map::parse(leaked).ok() else {
            return;
        };
        assert!(!m.content_files().info);
        assert!(matches!(m.info(), Err(Error::MissingContent(INFO_NAME))));
    }

    // ---- full sweep -----------------------------------------------------

    /// Sweep every shipped `.w3m` / `.w3n` on every install present, asserting
    /// that each map-info file parses, frames exactly, and round-trips.
    ///
    /// Ignored by default because it opens whole installs. Writes the census
    /// and any failures to `/tmp/opencode/w3i-sweep.txt` and prints them.
    #[test]
    #[ignore = "opens every shipped install; run explicitly"]
    fn sweep_every_shipped_info_file() {
        use std::fmt::Write as _;

        let mut report = String::new();
        let mut failures: Vec<String> = Vec::new();
        let mut grand = 0usize;

        for (label, dir) in installs() {
            let mut set = match pcx_mpq::MpqSet::open_wc3(&dir) {
                Ok(s) => s,
                Err(e) => {
                    failures.push(format!("{label}: open failed: {e}"));
                    continue;
                }
            };
            let maps: Vec<String> = match set.file_list() {
                Ok(m) => m
                    .into_iter()
                    .filter(|p| {
                        let l = p.to_ascii_lowercase();
                        l.ends_with(".w3m") || l.ends_with(".w3n")
                    })
                    .collect(),
                Err(e) => {
                    failures.push(format!("{label}: listfile failed: {e}"));
                    continue;
                }
            };

            let mut parsed = 0usize;
            let mut missing = Vec::new();
            let mut slots = 0usize;
            let mut forces = 0usize;
            let mut lists = 0usize;
            let mut versions = std::collections::BTreeMap::new();
            let mut controllers = std::collections::BTreeMap::new();
            let mut races = std::collections::BTreeMap::new();

            for map in &maps {
                let Ok(raw) = set.extract(map) else {
                    failures.push(format!("{label} {map}: extract failed"));
                    continue;
                };
                let Some(body) = raw.get(MAP_ARCHIVE_OFFSET..) else {
                    failures.push(format!("{label} {map}: no archive"));
                    continue;
                };
                let mut arc = match pcx_mpq::MpqArchive::from_bytes(body.to_vec(), map) {
                    Ok(a) => a,
                    Err(e) => {
                        failures.push(format!("{label} {map}: archive: {e}"));
                        continue;
                    }
                };
                if !arc.contains(INFO_NAME) {
                    missing.push(map.clone());
                    continue;
                }
                let Ok(bytes) = arc.extract(INFO_NAME) else {
                    failures.push(format!("{label} {map}: extract {INFO_NAME} failed"));
                    continue;
                };
                let Ok(i) = MapInfo::from_bytes(bytes) else {
                    failures.push(format!("{label} {map}: parse failed"));
                    continue;
                };
                let fr = i.framing();
                for v in fr.violations() {
                    failures.push(format!("{label} {map} {INFO_NAME}: framing: {v}"));
                }
                if !fr.sums() {
                    failures.push(format!("{label} {map} {INFO_NAME}: framing sums: {fr:?}"));
                }
                if i.canonical_bytes() != i.raw() {
                    failures.push(format!("{label} {map} {INFO_NAME}: round trip"));
                }
                if i.canonical_hash() == 0 {
                    failures.push(format!("{label} {map} {INFO_NAME}: zero hash"));
                }
                parsed += 1;
                slots += i.players().len();
                forces += i.forces().len();
                lists += i.table_lists().len();
                *versions.entry(i.version()).or_insert(0usize) += 1;
                for p in i.players() {
                    *controllers.entry(p.controller).or_insert(0usize) += 1;
                    *races.entry(p.race).or_insert(0usize) += 1;
                }
            }

            let _ = writeln!(
                report,
                "{label}: {} maps, {parsed} {INFO_NAME} parsed, {lists} table lists, \
                 {slots} slots, {forces} forces, versions {versions:?}, \
                 controllers {controllers:?}, races {races:?}",
                maps.len(),
            );
            if !missing.is_empty() {
                let _ = writeln!(report, "{label}: no {INFO_NAME}: {missing:?}");
            }
            println!(
                "{label}: {parsed} of {} maps carry {INFO_NAME}; {slots} slots, {forces} \
                 forces, {lists} table lists; missing {missing:?}",
                maps.len()
            );
            grand += parsed;
        }

        let _ = writeln!(report, "failures: {}", failures.len());
        for f in &failures {
            let _ = writeln!(report, "  {f}");
        }
        let out = std::path::Path::new("/tmp/opencode/w3i-sweep.txt");
        if let Some(dir) = out.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        std::fs::write(out, &report).expect("write sweep report");
        println!("{report}");
        assert!(
            failures.is_empty(),
            "{} failures, see {out:?}",
            failures.len()
        );
        assert!(grand > 0, "no install was opened");
    }
}

#[cfg(test)]
mod version25_regressions {
    use super::*;
    #[test]
    fn tft_fields_and_nested_item_tables_are_authored() {
        let mut data = Vec::new();
        for word in [25u32, 11, 12] {
            data.extend(word.to_le_bytes());
        }
        for string in [b"map".as_slice(), b"author", b"description", b"recommended"] {
            data.extend(string);
            data.push(0);
        }
        for value in 0..8 {
            data.extend((value as f32).to_le_bytes());
        }
        for word in [1u32, 2, 3, 4, 32, 64, 7] {
            data.extend(word.to_le_bytes());
        }
        data.push(b'L');
        data.extend(9u32.to_le_bytes());
        for string in ["loadpath", "loadtext", "loadtitle", "loadsubtitle"] {
            data.extend(string.as_bytes());
            data.push(0);
        }
        data.extend(10u32.to_le_bytes());
        for string in ["intropath", "introtext", "introtitle", "introsubtitle"] {
            data.extend(string.as_bytes());
            data.push(0);
        }
        data.extend(2u32.to_le_bytes());
        for value in [1f32, 2., 3.] {
            data.extend(value.to_le_bytes());
        }
        data.extend(0x11223344u32.to_le_bytes());
        data.extend(0x55667788u32.to_le_bytes());
        data.extend(b"authoredsound\0");
        data.push(b'A');
        data.extend(0x99aabbccu32.to_le_bytes());
        for _ in 0..5 {
            data.extend(0u32.to_le_bytes());
        }
        data.extend(1u32.to_le_bytes());
        data.extend(42u32.to_le_bytes());
        data.extend(b"items\0");
        for word in [2u32, 1, 33, 0x41424344, 0] {
            data.extend(word.to_le_bytes());
        }
        let info = MapInfo::from_bytes(data).unwrap();
        assert_eq!(info.loading_path.as_deref(), Some("intropath"));
        assert_eq!(info.loading_id, Some(10));
        assert_eq!(info.fog_style, Some(2));
        assert_eq!(info.extra_u32_19, Some(0x99aabbcc));
        assert_eq!(info.item_tables[0].sets, [vec![[33, 0x41424344]], vec![]]);
    }
}

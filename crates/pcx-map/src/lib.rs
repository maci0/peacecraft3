//! W3M/W3N map container and the `war3map.w3e` terrain file.
//!
//! Scope is deliberately one chunk: the map header and the terrain file. No
//! other map sub-file is touched.
//!
//! # The map header is not a chunk table
//!
//! The framing everyone expects — a `(fourcc, offset, size)` chunk table with a
//! `TERRAIN` chunk in it — is **not what either shipped build writes**. Both
//! builds ship the same shape, and it is not a guess: the embedded archive's
//! declared `archive_size` equals `file_len - 0x200` on all 133 shipped maps,
//! and the game's own writer stores the literal `HM3W` where the chunk table
//! would begin (`game.dll` file offsets `0x719b8b`, `0x726e9b`, `0x726f53`).
//! The same writer then opens an MPQ and never returns to a chunk table.
//!
//! ```text
//! offset  size  field
//! 0x000   4     "HM3W"
//! 0x004   4     version, 0 in all 133 shipped maps
//! 0x008   n     map name, NUL-terminated, variable width
//!   +0   4     field A — varies per map, meaning unattributed
//!   +0   4     field B — the map's player count
//! 0x020   0x1E0 zero padding
//! 0x200   ...   MPQ archive, "MPQ\x1a", `archive_size == len - 0x200`
//! EOF-260 260   trailer: "NGIS" + 256 unattributed bytes
//! ```
//!
//! `field B` is *measured*, not named from the filename: it equals the `(N)`
//! prefix on all 44 of the 1.28c custom maps — `(2)BootyBay.w3m` → 2,
//! `(12)IceCrown.w3m` → 12 — and it is 1 on all 45 campaign maps and on the
//! one `.w3n`. That is a player count. `field A` is kept raw: it ranges over
//! 1..39966 with no relation to any other field measured here, and guessing at
//! it would be a plausible-looking lie.
//!
//! The trailer is present on 89 of the 133 maps and absent on the other 44;
//! where present it always starts `NGIS` followed by 256 opaque bytes. Those
//! bytes are **not** decoded and **not** interpreted as a hash of anything. A
//! map with a trailer of any other length, or without the `NGIS` tag, is
//! refused rather than parsed.
//!
//! # The terrain file
//!
//! The terrain lives in `war3map.w3e` inside the embedded archive. It opens
//! `"W3E!"` and a version word, and the version is `11` on every shipped file.
//! The layout below is taken from `game.dll`'s own writer
//! (`0x6f7481e0` → `0x6f7406b0` → `0x6f740770`) and its reader
//! (`0x6f771020` → `0x6f7707a0` → `0x6f756ca0` → `0x6f740610`); it is then
//! confirmed byte-for-byte against all 132 shipped terrain files. The 133rd
//! shipped map, `DemoCampaign.w3n`, has no single terrain file at all: it is
//! a campaign container, and this crate does not go looking inside it.
//!
//! ```text
//! terrain file
//! 0x000  4   "W3E!"
//! 0x004  4   version                       (= 11 everywhere)
//! 0x008  1   default tile-texture slot id
//! 0x009  4   unattributed flag word
//! 0x00D  4   N   count of the first slot list
//! 0x011  4N  first slot list:  N x fourcc
//!       4   M   count of the second slot list
//!       4M  second slot list: M x fourcc
//!       4   width + 1
//!       4   height + 1
//!       4   origin x, as f32
//!       4   origin y, as f32
//!       7 * (width+1) * (height+1)  corner records
//! ```
//!
//! Corner record, seven bytes:
//!
//! ```text
//! 0  2  height, unsigned, 14 bits used
//! 2  2  position word, opaque (bit 14 is a flag)
//! 4  1  low nibble = tile-texture slot index, high nibble = four corner flags
//! 5  1  opaque
//! 6  1  opaque
//! ```
//!
//! # What is refused, and why
//!
//! **There is no quantised heightmap in this format, so there is no height
//! ramp to derive.** This is not a failure to find the ramp, it is a proof
//! that the ramp's input does not exist here:
//!
//! * the file is consumed *exactly* by the corner table — `7 * (w+1) * (h+1)`
//!   bytes with zero left over, on all 132 files;
//! * a `(w*8+1) * (h*8+1)` interior grid could not fit: the smallest shipped
//!   terrain is 29 644 bytes and its corner table alone is 29 575 of them.
//!
//! Terrain resolution is therefore **one vertex per tile corner**
//! (`corner_size == 1`): a `w x h` map has `(w+1) * (h+1)` samples and no
//! interior vertices at all. Heights are per-sample 14-bit indices, not
//! bytes interpolated through a four-point ramp.
//!
//! Those 14 bits are handed out as [`Corner::height_index`] and nothing more.
//! The game converts them to a world height with per-map constants this crate
//! has not proven, and the shipped data cannot prove them either — across all
//! 132 files the index only ever spans 5 477..14 336 of the 16 384 available,
//! so no scale factor can be recovered from the assets. Inventing one is
//! precisely the plausible-but-wrong failure this project refuses: a wrong
//! height scale renders terrain that looks right and plays wrong.
//!
//! Likewise the tile-texture slot: the game's writer stores the slot index in
//! the low nibble, while its reader recovers `0xF ^ nibble` and then clamps
//! anything `>= N` to 0. That asymmetry is reported as observed rather than
//! resolved — see [`Corner::slot_nibble`].

//! # The map's content files
//!
//! Beyond the terrain this crate decodes the map's remaining content files,
//! each with its own framing proof (see the submodules):
//!
//! * [`doodads`] — `war3map.doo`, placed doodad instances and terrain doodads;
//!   `16 + 42·n + 8 + 16·m == file_len`.
//! * [`shadows`] — `war3map.shd`, a `(4·w) × (4·h)` shadow mask that carries
//!   its own grid; `file_len == 16·w·h` against the terrain's tile size.
//! * [`minimap`] — the minimap pair: `war3mapMap.blp` (a BLP, framed through
//!   `pcx-blp`) and `war3map.mmp` (markers, `8 + 16·n == file_len`).
//!
//! The tracker names `war3map.map` / `war3map.ter` for the minimap pair do
//! not exist in the vanilla bytes — 0 of 133 shipped maps, 0 of 200 loose
//! community maps, unknown to `game.dll` and to the Reforged PDB name
//! tables. The measured names are above; the absence is recorded in
//! [`minimap`] and a map carrying the literal names is refused. Nothing in
//! this crate is derived from a filename that does not ship.

use std::collections::BTreeSet;

pub mod cameras;
pub mod doodads;
pub mod grid_alignment;
pub mod info;
pub mod minimap;
pub mod shadows;

pub mod pathing;
pub mod regions;
pub mod units;
pub use grid_alignment::{
    GridAlignment, GridAlignmentError, GridDimensions, PathingGridDimensions, TerrainGridDimensions,
};
pub use pathing::{Pathing, PathingError};
pub use regions::{Region, RegionBounds, RegionError, Regions};
pub use units::{RandomUnit, UnitPlacement, UnitPlacementError, Units};

pub use doodads::{Doodad, Doodads, ItemDropData, TerrainDoodad};
pub use info::{Force, MapInfo, Player};
pub use minimap::{Marker, MinimapImage, MinimapImageKind, MinimapMarkers};
pub use shadows::Shadows;

/// Retains the requested member even when archive preflight fails.
#[derive(Debug, thiserror::Error)]
#[error("{member}: {source}")]
pub struct MapMemberError {
    pub member: &'static str,
    #[source]
    pub source: pcx_mpq::checked::ReadError,
}

/// Container magic, written by the game before anything else.
pub const MAP_MAGIC: [u8; 4] = *b"HM3W";

/// The only map version either shipped build writes.
pub const MAP_VERSION: u32 = 0;

/// Byte offset of the embedded MPQ archive, fixed across every shipped map.
pub const MAP_ARCHIVE_OFFSET: usize = 0x200;

/// Byte length of the trailer that may follow the archive.
pub const MAP_TRAILER_LEN: usize = 260;

/// First four bytes of the trailer, where one is present.
pub const MAP_TRAILER_TAG: [u8; 4] = [0x4E, 0x47, 0x49, 0x53];

/// Magic of the terrain file inside the map archive.
pub const TERRAIN_MAGIC: [u8; 4] = *b"W3E!";

/// The only terrain version either shipped build writes.
pub const TERRAIN_VERSION: u32 = 11;

/// Byte length of one corner record. Measured, and the reason the file has a
/// seven-byte period in its bulk: `game.dll` `0x6f740610` reads exactly
/// `u16 + u16 + u8 + u8 + u8` per corner.
pub const CORNER_RECORD: usize = 7;

/// Number of fourcc tile-texture slots a map may declare.
pub const TERRAIN_MIN_VERSION_SLOTS: u32 = 6;

/// Terrain versions at or above this read the two origin floats.
pub const TERRAIN_ORIGIN_VERSION: u32 = 11;

/// Chunk tags, spelled out so an unrecognised one is an error rather than a
/// silent skip.
pub mod magic {
    pub const MAP: [u8; 4] = *b"HM3W";
    pub const TERRAIN: [u8; 4] = *b"W3E!";
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("terrain {what} overflows at byte {at}")]
    TerrainOverflow { what: &'static str, at: usize },
    #[error(
        "terrain corner dimensions at byte {at} cannot represent tiles: {corners_x}x{corners_y}"
    )]
    TerrainCornerDimensions {
        at: usize,
        corners_x: u32,
        corners_y: u32,
    },
    #[error(
        "terrain {what} at byte {at}: {count} records cannot fit the decoded allocation layout"
    )]
    TerrainLayout {
        what: &'static str,
        at: usize,
        count: usize,
    },
    #[error("terrain {what} allocation at byte {at}: {error}")]
    TerrainAllocation {
        what: &'static str,
        at: usize,
        #[source]
        error: std::collections::TryReserveError,
    },
    #[error("map extraction: {0}")]
    Extraction(#[from] pcx_mpq::Error),
    #[error("map pathing: {0}")]
    Pathing(#[from] pathing::PathingError),
    #[error("map regions: {0}")]
    Regions(#[from] regions::RegionError),
    #[error("unit placements: {0}")]
    Units(#[from] units::UnitPlacementError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("bad magic {0:?}, expected HM3W")]
    BadMagic([u8; 4]),
    #[error("unsupported map version {0}; only 0 ships")]
    UnsupportedMapVersion(u32),
    #[error("map name at +8 is not NUL-terminated within {0} bytes")]
    UnterminatedName(usize),
    #[error("no \"MPQ\\x1a\" at +0x{0:x}; the map body is not an embedded archive")]
    NoArchive(usize),
    #[error("archive declares {declared} bytes but only {available} follow the 0x200 header")]
    ArchiveSize { declared: usize, available: usize },
    #[error("map is {have} bytes, which is {left} short of header + archive + trailer")]
    TrailingBytes { have: usize, left: usize },
    #[error("bad terrain magic {0:?}, expected W3E!")]
    BadTerrainMagic([u8; 4]),
    #[error("unsupported terrain version {0}; 1.28c and 1.29b both write 11")]
    UnsupportedTerrainVersion(u32),
    #[error("terrain is {have} bytes; the header needs {need}")]
    TerrainTruncated { have: usize, need: usize },
    #[error(
        "terrain declares {width}x{height} but its {have} bytes hold {have_after_header} corner bytes, not {need}"
    )]
    TerrainSize {
        width: u32,
        height: u32,
        have: usize,
        have_after_header: usize,
        need: usize,
    },
    #[error("unsupported map-info version {0}; supported versions are 18 and 25")]
    UnsupportedInfoVersion(u32),
    #[error("map info: {0}")]
    InfoFraming(#[from] info::FramingViolation),
    #[error("bad doodad magic {0:?}, expected W3do")]
    BadDoodadMagic([u8; 4]),
    #[error("unsupported doodad version {0}; supported versions are 7 and 8")]
    UnsupportedDoodadVersion(u32),
    #[error("doodad file is {have} bytes; the declared counts need {need}")]
    DoodadTruncated { have: usize, need: usize },
    #[error(
        "doodad file declares {instances} instances and {terrain_doodads} terrain doodads, which need {need} bytes, not {have}"
    )]
    DoodadSize {
        instances: usize,
        terrain_doodads: usize,
        have: usize,
        need: usize,
    },
    #[error("shadow map is {have} bytes; a {width}x{height}-tile map needs {need} = 16·w·h")]
    BadShadowSize {
        width: u32,
        height: u32,
        have: usize,
        need: usize,
    },
    #[error("shadow map {width}x{height} tiles overflows the checked 16·w·h cell count")]
    ShadowSizeOverflow { width: u32, height: u32 },
    #[error("minimap file is {have} bytes; the declared counts need {need}")]
    MinimapTruncated { have: usize, need: usize },
    #[error("minimap file declares {markers} markers, which need {need} bytes, not {have}")]
    MinimapSize {
        markers: usize,
        have: usize,
        need: usize,
    },
    #[error("map archive has no {0}")]
    MissingContent(&'static str),
    #[error("no shipped byte proves a layout for {0}; refusing to invent one")]
    UnprovenContent(&'static str),
    #[error("blp: {0}")]
    Blp(#[from] pcx_blp::Error),
    #[error("tga: {0}")]
    Tga(#[from] pcx_tga::Error),
}

/// The 0x200-byte map header, parsed.
///
/// `field_a` and `field_b` are exposed under those names because that is all
/// the bytes support; see the module docs for what *is* measured about them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapHeader {
    pub version: u32,
    pub name: String,
    /// Varies 1..39966 across the shipped set; meaning unattributed.
    pub field_a: u32,
    /// The map's player count.
    pub field_b: u32,
    /// Offset of the map name within the file, always 8.
    pub name_offset: usize,
    /// Offset of the embedded archive, always [`MAP_ARCHIVE_OFFSET`].
    pub archive_offset: usize,
    /// The archive's declared size, which equals `file_len - archive_offset`.
    pub archive_size: usize,
    /// The trailer, when the file has one.
    pub trailer: Option<Vec<u8>>,
}

/// Which content files a map's archive holds, by their measured names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentFiles {
    pub doodads: bool,
    pub shadows: bool,
    /// The minimap-image member that is present, if any.
    pub minimap_image: Option<&'static str>,
    pub minimap_markers: bool,
    /// The map-info member, `war3map.w3i`.
    pub info: bool,
    /// A literal `war3map.map` / `war3map.ter`, if one is somehow present.
    pub unproven_minimap: Option<&'static str>,
}

/// A parsed map: header plus the bytes the header points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Map<'a> {
    pub header: MapHeader,
    data: &'a [u8],
}

impl<'a> Map<'a> {
    /// Parse the header and the whole framing relation around it.
    ///
    /// Proves, on the bytes, that the archive is framed by the header: the
    /// declared archive size plus the fixed header plus any trailer add up to
    /// the file length exactly. A map that fails this is refused rather than
    /// returned with a shrug.
    #[cfg_attr(
        feature = "profile-cpu",
        tracing::instrument(skip_all, target = "pcx_cpu", name = "map.parse_container")
    )]
    pub fn parse(data: &'a [u8]) -> Result<Self, Error> {
        let magic: [u8; 4] = data
            .get(..4)
            .ok_or(Error::NoArchive(4))?
            .try_into()
            .unwrap();
        if magic != MAP_MAGIC {
            return Err(Error::BadMagic(magic));
        }
        let version = u32_at(data, 4)?;
        if version != MAP_VERSION {
            return Err(Error::UnsupportedMapVersion(version));
        }
        let name_offset = 8;
        let name_end = data[name_offset..]
            .iter()
            .position(|&b| b == 0)
            .and_then(|n| name_offset.checked_add(n + 1))
            .ok_or(Error::UnterminatedName(data.len()))?;
        let name = String::from_utf8_lossy(&data[name_offset..name_end - 1]).into_owned();
        let field_a = u32_at(data, name_end)?;
        let field_b = u32_at(data, name_end + 4)?;

        let archive_offset = MAP_ARCHIVE_OFFSET;
        let arch: [u8; 4] = data
            .get(archive_offset..archive_offset + 4)
            .ok_or(Error::NoArchive(archive_offset))?
            .try_into()
            .unwrap();
        if &arch != b"MPQ\x1a" {
            return Err(Error::NoArchive(archive_offset));
        }
        let archive_size = u32_at(data, archive_offset + 8)? as usize;
        let available = data.len() - archive_offset;
        if archive_size > available {
            return Err(Error::ArchiveSize {
                declared: archive_size,
                available,
            });
        }

        let archive_end = archive_offset + archive_size;
        let rest = &data[archive_end..];
        let trailer = match rest.len() {
            0 => None,
            n if n == MAP_TRAILER_LEN => {
                if rest[..4] == MAP_TRAILER_TAG {
                    Some(rest.to_vec())
                } else {
                    return Err(Error::TrailingBytes {
                        have: data.len(),
                        left: rest.len(),
                    });
                }
            }
            n => {
                return Err(Error::TrailingBytes {
                    have: data.len(),
                    left: n,
                });
            }
        };

        Ok(Self {
            header: MapHeader {
                version,
                name,
                field_a,
                field_b,
                name_offset,
                archive_offset,
                archive_size,
                trailer,
            },
            data,
        })
    }

    /// The embedded archive's bytes, exactly as declared.
    pub fn archive(&self) -> &'a [u8] {
        &self.data
            [self.header.archive_offset..self.header.archive_offset + self.header.archive_size]
    }

    pub fn raw(&self) -> &'a [u8] {
        self.data
    }

    /// Extract `name` from the embedded archive.
    pub fn extract(&self, name: &str) -> Result<Vec<u8>, pcx_mpq::Error> {
        pcx_mpq::MpqArchive::from_bytes(self.archive().to_vec(), "map")?.extract(name)
    }

    /// Extract one named member with explicit source and allocation policy.
    /// The borrowed checked MPQ path refuses distinct locale/platform/block candidates.
    pub fn extract_checked(
        &self,
        source_label: &str,
        member: &'static str,
        budget: pcx_mpq::checked::ReadBudget,
    ) -> Result<Option<Vec<u8>>, MapMemberError> {
        let extract = || {
            pcx_mpq::checked::CheckedArchive::parse(self.archive(), source_label, budget)?
                .extract(member, budget.max_decoded_bytes)
        };
        extract().map_err(|source| MapMemberError { member, source })
    }

    /// Open the embedded archive for name-based lookup.
    pub fn open_archive(&self) -> Result<pcx_mpq::MpqArchive, pcx_mpq::Error> {
        pcx_mpq::MpqArchive::from_bytes(self.archive().to_vec(), "map")
    }

    /// Decode the terrain file.
    pub fn terrain(&self) -> Result<Terrain, Error> {
        let bytes = self.extract("war3map.w3e")?;
        Terrain::from_bytes(bytes)
    }

    /// Which content files the archive holds, by their measured names.
    ///
    /// `unproven_minimap` names a literal `war3map.map` / `war3map.ter` if
    /// one is present; see [`minimap`] for what that means.
    pub fn content_files(&self) -> ContentFiles {
        let has = |n: &str| self.extract(n).is_ok();
        let image = if has(minimap::MINIMAP_IMAGE_NAME) {
            Some(minimap::MINIMAP_IMAGE_NAME)
        } else if has(minimap::MINIMAP_IMAGE_TGA_NAME) {
            Some(minimap::MINIMAP_IMAGE_TGA_NAME)
        } else {
            None
        };
        ContentFiles {
            doodads: has("war3map.doo"),
            shadows: has("war3map.shd"),
            minimap_image: image,
            minimap_markers: has(minimap::MINIMAP_MARKERS_NAME),
            info: has(info::INFO_NAME),
            unproven_minimap: minimap::UNPROVEN_MINIMAP_NAMES
                .iter()
                .copied()
                .find(|n| has(n)),
        }
    }

    /// Refuse the tracker's literal `war3map.map` / `war3map.ter` names.
    ///
    /// Both minimap accessors call this first: a map that really carries one
    /// of those names is outside every measurement this crate has, and a
    /// guessed layout would be worse than an error.
    fn refuse_unproven_minimap(&self) -> Result<(), Error> {
        match self.content_files().unproven_minimap {
            Some(n) => Err(Error::UnprovenContent(n)),
            None => Ok(()),
        }
    }

    /// Decode `war3map.doo`, the doodad placements.
    pub fn doodads(&self) -> Result<Doodads, Error> {
        let bytes = self
            .extract("war3map.doo")
            .map_err(|_| Error::MissingContent("war3map.doo"))?;
        Doodads::from_bytes(bytes)
    }

    /// Decode `war3map.w3i`, the map's own info file.
    ///
    /// `DemoCampaign.w3n` has none: it is a campaign container, and the
    /// absence is a measured fact rather than a failure. Every `.w3m` on both
    /// builds carries one.
    pub fn info(&self) -> Result<MapInfo, Error> {
        let bytes = self
            .extract(info::INFO_NAME)
            .map_err(|_| Error::MissingContent(info::INFO_NAME))?;
        MapInfo::from_bytes(bytes)
    }

    /// Decode `war3map.shd`, the shadow map, against this map's terrain.
    ///
    /// The terrain is read because the shadow file has no dimensions of its
    /// own; `Shadows::from_bytes` proves `file_len == 16·w·h` against them.
    pub fn shadows(&self) -> Result<Shadows, Error> {
        let t = self.terrain()?;
        let bytes = self
            .extract("war3map.shd")
            .map_err(|_| Error::MissingContent("war3map.shd"))?;
        Shadows::from_bytes(bytes, t.width(), t.height())
    }

    /// Decode `war3map.mmp`, the minimap markers (the tracker's "terrain
    /// minimap" role; the file is not named `war3map.ter` — see [`minimap`]).
    pub fn minimap_markers(&self) -> Result<MinimapMarkers, Error> {
        self.refuse_unproven_minimap()?;
        let bytes = self
            .extract(minimap::MINIMAP_MARKERS_NAME)
            .map_err(|_| Error::MissingContent(minimap::MINIMAP_MARKERS_NAME))?;
        MinimapMarkers::from_bytes(bytes)
    }

    /// Decode the minimap image (the tracker's "minimap image" role; the file
    /// is named `war3mapMap.blp`, not `war3map.map` — see [`minimap`]).
    ///
    /// BLP images are parsed and framed by `pcx-blp`, never here.
    pub fn minimap_image(&self) -> Result<MinimapImage, Error> {
        self.refuse_unproven_minimap()?;
        for name in [minimap::MINIMAP_IMAGE_NAME, minimap::MINIMAP_IMAGE_TGA_NAME] {
            if let Ok(bytes) = self.extract(name) {
                return MinimapImage::from_named_bytes(name, bytes);
            }
        }
        Err(Error::MissingContent(minimap::MINIMAP_IMAGE_NAME))
    }

    /// Byte length of the file.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

fn u32_at(data: &[u8], at: usize) -> Result<u32, Error> {
    data.get(at..at + 4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .ok_or(Error::TerrainTruncated {
            have: data.len(),
            need: at + 4,
        })
}

/// One terrain vertex, at a tile corner.
///
/// Field names are the ones the game's own code uses. Nothing here is a world
/// coordinate and nothing here is a height in world units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Corner {
    /// Raw 14-bit height index, 0..=16383.
    ///
    /// The world height this maps to is **not** decoded; see the module docs.
    pub height_index: u16,
    /// The opaque second word. Bit 14 is a flag the game folds into its
    /// per-corner flag word; the rest is handed back untouched.
    pub position_word: u16,
    /// Low nibble of the third byte.
    ///
    /// The game's writer stores the tile-texture slot index here, but its
    /// reader recovers `0xF ^ nibble` and then clamps anything `>= slot count`
    /// to 0. Shipped files use 0..7 here. The two readings disagree and this
    /// crate does not pick one.
    pub slot_nibble: u8,
    /// The four high-nibble flag bits. Bit 1 is the on-disk blight flag;
    /// the remaining bits retain their original representation.
    pub corner_flags: u8,
    /// Fifth byte, opaque.
    pub byte5: u8,
    /// Packed cliff texture (high nibble) and elevation layer (low nibble).
    pub byte6: u8,
}

impl Corner {
    /// Low nibble of the packed cliff byte: the W3E elevation layer.
    pub fn layer_height(&self) -> u8 {
        self.byte6 & 0x0f
    }

    /// High nibble of the packed cliff byte: the cliff texture selector.
    pub fn cliff_texture(&self) -> u8 {
        self.byte6 >> 4
    }

    /// W3E texture/flags byte bit 0x20, retained in bit 1 of `corner_flags`.
    /// This is an on-disk flag, independent of the selected ground texture.
    pub fn is_blighted(&self) -> bool {
        self.corner_flags & 0b0010 != 0
    }

    /// `true` when bit 14 of [`Self::position_word`] is set.
    pub fn position_flag(&self) -> bool {
        self.position_word & 0x4000 != 0
    }
}

/// A decoded `war3map.w3e`.
///
/// Owns the file bytes, because the bytes come out of an archive rather than
/// off disk and so cannot borrow from the map.
#[derive(Debug, Clone, PartialEq)]
pub struct Terrain {
    parsed: TerrainParsed,
    raw: Vec<u8>,
}

/// The owned part of a [`Terrain`], kept separate so it can be built without
/// borrowing.
#[derive(Debug, Clone, PartialEq)]
struct TerrainParsed {
    version: u32,
    default_slot: u8,
    slot_flag_word: u32,
    slots: Vec<[u8; 4]>,
    secondary_slots: Vec<[u8; 4]>,
    width: u32,
    height: u32,
    origin_x: f32,
    origin_y: f32,
    corners: Vec<Corner>,
}

impl TerrainParsed {
    fn parse(bytes: &[u8]) -> Result<Self, Error> {
        let magic: [u8; 4] = bytes
            .get(..4)
            .ok_or(Error::TerrainTruncated {
                have: bytes.len(),
                need: 4,
            })?
            .try_into()
            .unwrap();
        if magic != TERRAIN_MAGIC {
            return Err(Error::BadTerrainMagic(magic));
        }
        let version = u32_at(bytes, 4)?;
        if version != TERRAIN_VERSION {
            return Err(Error::UnsupportedTerrainVersion(version));
        }
        let default_slot = *bytes.get(8).ok_or(Error::TerrainTruncated {
            have: bytes.len(),
            need: 9,
        })?;
        let slot_flag_word = u32_at(bytes, 9)?;
        let n = u32_at(bytes, 13)? as usize;
        let mut at = 17usize;
        let slots_at = at;
        let slots_size =
            n.checked_mul(std::mem::size_of::<[u8; 4]>())
                .ok_or(Error::TerrainOverflow {
                    what: "primary slots",
                    at,
                })?;
        let slots_raw = terrain_take(bytes, &mut at, slots_size, "primary slots")?;
        let secondary_count = terrain_take(
            bytes,
            &mut at,
            std::mem::size_of::<u32>(),
            "secondary count",
        )?;
        let m = u32::from_le_bytes(secondary_count.try_into().unwrap()) as usize;
        let secondary_at = at;
        let secondary_size =
            m.checked_mul(std::mem::size_of::<[u8; 4]>())
                .ok_or(Error::TerrainOverflow {
                    what: "secondary slots",
                    at,
                })?;
        let secondary_raw = terrain_take(bytes, &mut at, secondary_size, "secondary slots")?;
        let dimensions_at = at;
        let dimensions = terrain_take(
            bytes,
            &mut at,
            2 * std::mem::size_of::<u32>(),
            "corner dimensions",
        )?;
        let corners_x = u32::from_le_bytes(dimensions[..4].try_into().unwrap());
        let corners_y = u32::from_le_bytes(dimensions[4..].try_into().unwrap());
        let (width, height) = corners_x
            .checked_sub(1)
            .zip(corners_y.checked_sub(1))
            .ok_or(Error::TerrainCornerDimensions {
                at: dimensions_at,
                corners_x,
                corners_y,
            })?;
        let (origin_x, origin_y) = if version >= TERRAIN_ORIGIN_VERSION {
            let origin = terrain_take(bytes, &mut at, 2 * std::mem::size_of::<f32>(), "origin")?;
            (
                f32::from_le_bytes(origin[..4].try_into().unwrap()),
                f32::from_le_bytes(origin[4..].try_into().unwrap()),
            )
        } else {
            (0.0, 0.0)
        };
        let count =
            (corners_x as usize)
                .checked_mul(corners_y as usize)
                .ok_or(Error::TerrainOverflow {
                    what: "corner count",
                    at: dimensions_at,
                })?;
        let corner_bytes = count
            .checked_mul(CORNER_RECORD)
            .ok_or(Error::TerrainOverflow {
                what: "corner bytes",
                at,
            })?;
        let need = at.checked_add(corner_bytes).ok_or(Error::TerrainOverflow {
            what: "corner end",
            at,
        })?;
        if need != bytes.len() {
            return Err(Error::TerrainSize {
                width,
                height,
                have: bytes.len(),
                have_after_header: bytes.len() - at,
                need,
            });
        }
        // Validate every decoded layout before reserving any array.
        terrain_layout::<[u8; 4]>(n, slots_at, "primary slots")?;
        terrain_layout::<[u8; 4]>(m, secondary_at, "secondary slots")?;
        terrain_layout::<Corner>(count, at, "corners")?;
        let mut slots = terrain_reserve(n, slots_at, "primary slots")?;
        slots.extend(slots_raw.as_chunks::<4>().0.iter().copied());
        let mut secondary = terrain_reserve(m, secondary_at, "secondary slots")?;
        secondary.extend(secondary_raw.as_chunks::<4>().0.iter().copied());
        let mut corners = terrain_reserve(count, at, "corners")?;
        for rec in bytes[at..].as_chunks::<CORNER_RECORD>().0 {
            let h = u16::from_le_bytes(rec[0..2].try_into().unwrap());
            let p = u16::from_le_bytes(rec[2..4].try_into().unwrap());
            corners.push(Corner {
                height_index: h & 0x3FFF,
                position_word: p,
                slot_nibble: rec[4] & 0x0F,
                corner_flags: (rec[4] >> 4) & 0x0F,
                byte5: rec[5],
                byte6: rec[6],
            });
        }
        Ok(Self {
            version,
            default_slot,
            slot_flag_word,
            slots,
            secondary_slots: secondary,
            width,
            height,
            origin_x,
            origin_y,
            corners,
        })
    }
}

fn terrain_take<'a>(
    bytes: &'a [u8],
    at: &mut usize,
    size: usize,
    what: &'static str,
) -> Result<&'a [u8], Error> {
    let end = at
        .checked_add(size)
        .ok_or(Error::TerrainOverflow { what, at: *at })?;
    let raw = bytes.get(*at..end).ok_or(Error::TerrainTruncated {
        have: bytes.len(),
        need: end,
    })?;
    *at = end;
    Ok(raw)
}

fn terrain_layout<T>(count: usize, at: usize, what: &'static str) -> Result<(), Error> {
    std::alloc::Layout::array::<T>(count)
        .map(|_| ())
        .map_err(|_| Error::TerrainLayout { what, at, count })
}

fn terrain_reserve<T>(count: usize, at: usize, what: &'static str) -> Result<Vec<T>, Error> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|error| Error::TerrainAllocation { what, at, error })?;
    Ok(values)
}

impl Terrain {
    /// Parse terrain bytes, keeping them for [`Self::raw`].
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, Error> {
        let parsed = TerrainParsed::parse(&bytes)?;
        Ok(Self { parsed, raw: bytes })
    }

    /// The terrain file exactly as stored.
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }

    /// Terrain file version; 11 on every shipped file.
    pub fn version(&self) -> u32 {
        self.parsed.version
    }

    /// The declared default tile-texture slot id.
    pub fn default_slot(&self) -> u8 {
        self.parsed.default_slot
    }

    /// The flag word between the default slot and the slot list. Unattributed.
    pub fn slot_flag_word(&self) -> u32 {
        self.parsed.slot_flag_word
    }

    /// Map width in tiles.
    pub fn width(&self) -> u32 {
        self.parsed.width
    }

    /// Map height in tiles.
    pub fn height(&self) -> u32 {
        self.parsed.height
    }

    /// `(width + 1) * (height + 1)`, the number of terrain vertices.
    pub fn corner_count(&self) -> usize {
        self.parsed.corners.len()
    }

    /// The fourcc tile-texture slot ids, in file order.
    pub fn slots(&self) -> &[[u8; 4]] {
        &self.parsed.slots
    }

    /// The second fourcc list, in file order.
    pub fn secondary_slots(&self) -> &[[u8; 4]] {
        &self.parsed.secondary_slots
    }

    /// Declared origin, first of the two floats after the dimensions.
    pub fn origin(&self) -> (f32, f32) {
        (self.parsed.origin_x, self.parsed.origin_y)
    }

    /// All corners, row-major, `(height + 1)` per row.
    pub fn corners(&self) -> &[Corner] {
        &self.parsed.corners
    }

    /// The corner at tile-grid `(x, y)`, where `x` and `y` range over
    /// `0..=width` and `0..=height`.
    pub fn corner(&self, x: u32, y: u32) -> Option<&Corner> {
        if x > self.width() || y > self.height() {
            return None;
        }
        self.parsed
            .corners
            .get(y as usize * (self.width() as usize + 1) + x as usize)
    }

    /// Canonical byte form: the terrain file exactly as stored. Hashing this
    /// lets two decoders be compared without either of them inventing a
    /// height scale.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&TERRAIN_MAGIC);
        out.extend_from_slice(&self.parsed.version.to_le_bytes());
        out.push(self.parsed.default_slot);
        out.extend_from_slice(&self.parsed.slot_flag_word.to_le_bytes());
        out.extend_from_slice(&(self.parsed.slots.len() as u32).to_le_bytes());
        for s in &self.parsed.slots {
            out.extend_from_slice(s);
        }
        out.extend_from_slice(&(self.parsed.secondary_slots.len() as u32).to_le_bytes());
        for s in &self.parsed.secondary_slots {
            out.extend_from_slice(s);
        }
        out.extend_from_slice(&(self.width() + 1).to_le_bytes());
        out.extend_from_slice(&(self.height() + 1).to_le_bytes());
        out.extend_from_slice(&self.parsed.origin_x.to_le_bytes());
        out.extend_from_slice(&self.parsed.origin_y.to_le_bytes());
        for c in &self.parsed.corners {
            out.extend_from_slice(&c.height_index.to_le_bytes());
            out.extend_from_slice(&c.position_word.to_le_bytes());
            out.push((c.corner_flags << 4) | c.slot_nibble);
            out.push(c.byte5);
            out.push(c.byte6);
        }
        out
    }

    /// FNV-1a 64 over [`Self::canonical_bytes`].
    pub fn canonical_hash(&self) -> u64 {
        pcx_mpq::fnv1a64(&self.canonical_bytes())
    }

    /// Every distinct slot nibble used by any corner.
    pub fn slot_nibbles_used(&self) -> BTreeSet<u8> {
        self.parsed.corners.iter().map(|c| c.slot_nibble).collect()
    }

    /// Smallest and largest raw height index present.
    pub fn height_index_range(&self) -> Option<(u16, u16)> {
        let mut it = self.parsed.corners.iter().map(|c| c.height_index);
        let first = it.next()?;
        let mut lo = first;
        let mut hi = first;
        for h in it {
            lo = lo.min(h);
            hi = hi.max(h);
        }
        Some((lo, hi))
    }
}

/// FNV-1a 64 over a map file, header and archive and trailer alike.
pub fn map_hash(bytes: &[u8]) -> u64 {
    pcx_mpq::fnv1a64(bytes)
}

/// Every `.w3m` / `.w3n` path in an install, from the union of every member
/// archive's `(listfile)`.
///
/// Use this rather than `MpqSet::file_list()`'s caller-side filtering: the
/// shipped maps live in `war3.mpq`, which is *last* in the search order, so a
/// `(listfile)` read through `extract` would not list them.
pub fn shipped_maps(set: &mut pcx_mpq::MpqSet) -> Result<Vec<String>, pcx_mpq::Error> {
    Ok(set
        .file_list()?
        .into_iter()
        .filter(|p| {
            let l = p.to_ascii_lowercase();
            l.ends_with(".w3m") || l.ends_with(".w3n")
        })
        .collect())
}

/// Decode a terrain file directly, for fixtures and for the inspector.
pub fn parse_terrain(bytes: &[u8]) -> Result<Terrain, Error> {
    let parsed = TerrainParsed::parse(bytes)?;
    terrain_layout::<u8>(bytes.len(), 0, "raw bytes")?;
    let mut raw = terrain_reserve(bytes.len(), 0, "raw bytes")?;
    raw.extend_from_slice(bytes);
    Ok(Terrain { parsed, raw })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn workspace() -> PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf()
    }

    fn install_dir() -> Option<PathBuf> {
        let p = workspace().join("wc3/game/app");
        p.is_dir().then_some(p)
    }

    fn legacy_dir() -> Option<PathBuf> {
        let p = std::env::var_os("PEACECRAFT3_LEGACY_DIR")
            .map(PathBuf::from)
            .or_else(|| {
                let p = workspace().join(
                    "../../Faugus/battle-net/drive_c/Program Files (x86)/Warcraft III (Legacy)",
                );
                p.is_dir().then_some(p)
            })?;
        p.is_dir().then_some(p)
    }

    /// Open an install and pull one shipped map out of it.
    fn shipped_bytes(dir: &std::path::Path, path: &str) -> Option<Vec<u8>> {
        let mut set = pcx_mpq::MpqSet::open_wc3(dir).ok()?;
        set.extract(path).ok()
    }

    /// Same, but parsed. The leaked slice keeps the map borrowable for the
    /// length of one test; these are tests over 100 KB fixtures, not a leak in
    /// anything long-lived.
    fn shipped(dir: &std::path::Path, path: &str) -> Option<Map<'static>> {
        let bytes: &'static [u8] = Box::leak(shipped_bytes(dir, path)?.into_boxed_slice());
        Map::parse(bytes).ok()
    }

    const MAP_128C: &str = r"maps\campaign\undead08.w3m";
    const MAP_128C_CUSTOM: &str = r"(4)adrenaline.w3m";
    const MAP_129B: &str = r"maps\campaign\human01.w3m";

    // ---- header ---------------------------------------------------------

    #[test]
    fn shipped_map_counts() {
        let Some(dir) = install_dir() else { return };
        let mut set = pcx_mpq::MpqSet::open_wc3(&dir).unwrap();
        let maps = shipped_maps(&mut set).unwrap();
        let n_m = maps.iter().filter(|p| p.ends_with(".w3m")).count();
        let n_n = maps.iter().filter(|p| p.ends_with(".w3n")).count();
        assert_eq!((maps.len(), n_m, n_n), (89, 88, 1), "1.28c map census");
        assert!(maps.iter().any(|p| p == "DemoCampaign.w3n"));
    }

    #[test]
    fn shipped_map_counts_legacy() {
        let Some(dir) = legacy_dir() else { return };
        let mut set = pcx_mpq::MpqSet::open_wc3(&dir).unwrap();
        let maps = shipped_maps(&mut set).unwrap();
        let n_n = maps.iter().filter(|p| p.ends_with(".w3n")).count();
        assert_eq!((maps.len(), n_n), (44, 0), "1.29b map census");
    }

    #[test]
    fn header_fields_are_exact() {
        let Some(dir) = install_dir() else { return };
        let Some(m) = shipped(&dir, MAP_128C) else {
            return;
        };
        let h = &m.header;
        assert_eq!(h.version, 0);
        assert_eq!(h.name, "Undead08");
        assert_eq!(h.name_offset, 8);
        assert_eq!(h.archive_offset, 0x200);
        // Read by hand from the bytes, not from the parser.
        let raw = m.raw();
        assert_eq!(&raw[..4], b"HM3W");
        assert_eq!(u32::from_le_bytes(raw[4..8].try_into().unwrap()), 0);
        assert_eq!(&raw[8..17], b"Undead08\0");
        assert_eq!(h.field_a, 3809);
        assert_eq!(h.field_b, 1);
    }

    #[test]
    fn player_count_matches_the_filename_prefix() {
        let Some(dir) = install_dir() else { return };
        for (path, players) in [
            (r"(2)BootyBay.w3m", 2u32),
            (r"(4)Adrenaline.w3m", 4),
            (r"(10)DustwallowKeys.w3m", 10),
            (r"(12)IceCrown.w3m", 12),
            (r"(1)TheDeathSheep.w3m", 1),
        ] {
            let Some(m) = shipped(&dir, path) else {
                return;
            };
            assert_eq!(m.header.field_b, players, "{path}");
            assert!(!m.header.name.is_empty(), "{path} keeps a name");
        }
    }

    #[test]
    fn archive_size_is_framed_by_the_header() {
        let Some(dir) = install_dir() else { return };
        for path in [MAP_128C, MAP_128C_CUSTOM] {
            let Some(m) = shipped(&dir, path) else {
                return;
            };
            let h = &m.header;
            let end = h.archive_offset + h.archive_size;
            assert_eq!(&m.raw()[h.archive_offset..h.archive_offset + 4], b"MPQ\x1a");
            assert_eq!(end + h.trailer.as_ref().map_or(0, Vec::len), m.len());
            assert_eq!(
                m.len() - 0x200 - h.archive_size,
                h.trailer.as_ref().map_or(0, Vec::len)
            );
        }
    }

    #[test]
    fn trailer_is_tagged_where_present() {
        let Some(dir) = install_dir() else { return };
        let Some(m) = shipped(&dir, MAP_128C_CUSTOM) else {
            return;
        };
        let t = m.header.trailer.as_ref().expect("trailer");
        assert_eq!(t.len(), MAP_TRAILER_LEN);
        assert_eq!(&t[..4], &MAP_TRAILER_TAG);
        assert!(
            t[4..].iter().any(|&b| b != 0),
            "the 256 opaque bytes are not padding"
        );
    }

    #[test]
    fn bad_magic_is_refused() {
        let mut d = vec![0u8; 0x400];
        d[..4].copy_from_slice(b"XXXX");
        assert!(matches!(Map::parse(&d), Err(Error::BadMagic(_))));
    }

    #[test]
    fn missing_archive_is_refused() {
        let mut d = vec![0u8; 0x400];
        d[..4].copy_from_slice(b"HM3W");
        assert!(matches!(Map::parse(&d), Err(Error::NoArchive(_))));
    }

    // ---- terrain --------------------------------------------------------

    #[test]
    fn terrain_fields_are_exact() {
        let Some(dir) = install_dir() else { return };
        let Some(m) = shipped(&dir, MAP_128C) else {
            return;
        };
        let t = m.terrain().unwrap();
        assert_eq!(t.version(), 11);
        assert_eq!((t.width(), t.height()), (128, 128));
        assert_eq!(t.corner_count(), 129 * 129);
        assert_eq!(t.slots().len(), 9);
        assert_eq!(t.secondary_slots().len(), 2);
        assert_eq!(t.slots()[0], *b"Xdrt");
        assert_eq!(t.secondary_slots()[0], *b"CXdi");
        assert_eq!(t.default_slot(), 0x58);
        // Read by hand from the bytes: 0x49 and 0x4D are both `00 00 c6 c6`.
        assert_eq!(t.origin().0.to_bits(), 0xc600_0000); // -8192.0
        assert_eq!(t.origin().1.to_bits(), 0xc600_0000); // -8192.0
    }

    #[test]
    fn terrain_framing_is_exact_for_every_corner() {
        let Some(dir) = install_dir() else { return };
        let Some(m) = shipped(&dir, MAP_128C_CUSTOM) else {
            return;
        };
        let t = m.terrain().unwrap();
        // 53 header bytes + 16 dimension/origin bytes + 7 * 65 * 65 corners.
        assert_eq!(t.width(), 64);
        assert_eq!(t.height(), 64);
        assert_eq!(t.slots().len(), 6);
        assert_eq!(t.secondary_slots().len(), 2);
        let bytes = t.canonical_bytes();
        assert_eq!(bytes.len(), 53 + 16 + 7 * 65 * 65);
        assert_eq!(bytes.len(), 29644, "the shipped file is exactly consumed");
    }

    #[test]
    fn corners_index_row_major() {
        let Some(dir) = install_dir() else { return };
        let Some(m) = shipped(&dir, MAP_128C) else {
            return;
        };
        let t = m.terrain().unwrap();
        let stride = t.width() as usize + 1;
        for (i, c) in t.corners().iter().enumerate() {
            assert_eq!(Some(c), t.corner((i % stride) as u32, (i / stride) as u32));
        }
        assert!(t.corner(t.width() + 1, 0).is_none());
    }

    #[test]
    fn corner_record_is_seven_bytes() {
        // Hand-read from Maps\Campaign\Undead08.w3m's terrain: the corner table
        // starts at 0x51 and its first record is 60 01 04 f2 00 20 00.
        let Some(dir) = install_dir() else { return };
        let Some(m) = shipped(&dir, MAP_128C) else {
            return;
        };
        let raw = m.extract("war3map.w3e").unwrap();
        let rec = &raw[0x51..0x51 + CORNER_RECORD];
        assert_eq!(rec, &[0x00, 0x20, 0x00, 0x60, 0x01, 0x04, 0xf2]);
        let t = m.terrain().unwrap();
        let c = t.corners()[0];
        assert_eq!(c.height_index, 0x2000);
        assert_eq!(c.position_word, 0x6000);
        assert!(c.position_flag());
        assert_eq!(
            (c.slot_nibble, c.corner_flags, c.byte5, c.byte6),
            (1, 0, 0x04, 0xf2)
        );
    }

    #[test]
    fn height_indices_stay_inside_fourteen_bits() {
        let Some(dir) = install_dir() else { return };
        let Some(m) = shipped(&dir, MAP_128C) else {
            return;
        };
        let t = m.terrain().unwrap();
        let (lo, hi) = t.height_index_range().unwrap();
        assert!(hi <= 0x3FFF);
        // The shipped assets do not span the whole range, which is why no
        // world scale can be recovered from them.
        assert!(lo > 0, "no map uses index 0");
        assert!(hi < 0x3FFF, "no map uses the top of the range");
    }

    #[test]
    fn legacy_build_uses_the_same_terrain_framing() {
        let Some(dir) = legacy_dir() else { return };
        let Some(m) = shipped(&dir, MAP_129B) else {
            return;
        };
        let t = m.terrain().unwrap();
        assert_eq!(t.version(), 11);
        let bytes = t.canonical_bytes();
        let want = 17
            + 4 * t.slots().len()
            + 4
            + 4 * t.secondary_slots().len()
            + 16
            + 7 * (t.width() as usize + 1) * (t.height() as usize + 1);
        assert_eq!(bytes.len(), want);
    }

    #[test]
    fn canonical_hash_is_stable_and_recomputable() {
        let Some(dir) = install_dir() else { return };
        let Some(m) = shipped(&dir, MAP_128C) else {
            return;
        };
        let t = m.terrain().unwrap();
        let h1 = t.canonical_hash();
        let raw = m.extract("war3map.w3e").unwrap();
        let t2 = parse_terrain(&raw).unwrap();
        assert_eq!(h1, t2.canonical_hash());
        assert_eq!(t.canonical_bytes(), raw);
    }

    #[test]
    fn truncated_terrain_is_refused() {
        let bytes = vec![0u8; 32];
        assert!(parse_terrain(&bytes).is_err());
    }

    // ---- doodads --------------------------------------------------------

    const MAP_SHEEP: &str = r"(1)TheDeathSheep.w3m";
    const MAP_ORC02: &str = r"Maps\Campaign\Orc02.w3m";
    const MAP_HARROW: &str = r"(2)Harrow.w3m";

    #[test]
    fn doodad_header_and_first_record_are_exact() {
        // Hand-read from (1)TheDeathSheep.w3m's war3map.doo: W3do 07 00 00 00
        // 09 00 00 00 41 00 00 00 at the top, then the first 42-byte record
        // at 0x10.
        let Some(dir) = install_dir() else { return };
        let Some(m) = shipped(&dir, MAP_SHEEP) else {
            return;
        };
        let raw = m.extract("war3map.doo").unwrap();
        assert_eq!(&raw[..4], b"W3do");
        assert_eq!(u32::from_le_bytes(raw[4..8].try_into().unwrap()), 7);
        assert_eq!(u32::from_le_bytes(raw[8..12].try_into().unwrap()), 9);
        assert_eq!(u32::from_le_bytes(raw[12..16].try_into().unwrap()), 65);
        let rec = &raw[0x10..0x10 + 42];
        assert_eq!(&rec[0..4], b"LOsm");
        assert_eq!(&rec[4..8], &2u32.to_le_bytes());
        assert_eq!(&rec[8..12], &0xc3ad_cabbu32.to_le_bytes());
        assert_eq!(&rec[12..16], &0x4444_1237u32.to_le_bytes());
        assert_eq!(&rec[16..20], &0xc1c8_9fb9u32.to_le_bytes());
        assert_eq!(&rec[20..24], &0u32.to_le_bytes());
        assert_eq!(&rec[36..42], &[0x02, 0xff, 0x00, 0x00, 0x00, 0x00]);

        let d = m.doodads().unwrap();
        assert_eq!(d.version(), 7);
        assert_eq!(d.subversion(), 9);
        assert_eq!(d.instances().len(), 65);
        assert_eq!(d.terrain_section_version(), 0);
        assert!(d.terrain_doodads().is_empty());
        let d0 = &d.instances()[0];
        assert_eq!(d0.type_id, *b"LOsm");
        assert_eq!(d0.variation, 2);
        assert_eq!(d0.position[0].to_bits(), 0xc3ad_cabb);
        assert_eq!(d0.position[1].to_bits(), 0x4444_1237);
        assert_eq!(d0.position[2].to_bits(), 0xc1c8_9fb9);
        assert_eq!(d0.angle.to_bits(), 0x0000_0000);
        assert_eq!(d0.scale[0].to_bits(), 0x3f6a_8aa2);
        assert_eq!(d0.scale[1].to_bits(), 0x3f64_fc31);
        assert_eq!(d0.scale[2].to_bits(), 0x3f72_f51a);
        assert_eq!((d0.flags, d0.life, d0.creation_number), (0x02, 0xff, 0));
    }

    #[test]
    fn doodad_record_layout_is_pinned_across_types() {
        // Three more hand-read records from the same file: record 16 at 0x2b0
        // ("LPcw"), record 25 at 0x42a ("LTw2", life 100), and the last record
        // at 0xa90 ("LOth"). The stride is 42 for all of them.
        let Some(dir) = install_dir() else { return };
        let Some(m) = shipped(&dir, MAP_SHEEP) else {
            return;
        };
        let d = m.doodads().unwrap();
        let d16 = &d.instances()[16];
        assert_eq!(d16.type_id, *b"LPcw");
        assert_eq!(d16.variation, 0);
        assert_eq!(d16.position[0].to_bits(), 0x418d_93bd);
        assert_eq!(d16.position[1].to_bits(), 0x442d_2dbe);
        assert_eq!(d16.position[2].to_bits(), 0xc291_83c2);
        assert_eq!(d16.angle.to_bits(), 0x3f3b_a867);
        assert_eq!(d16.scale[0].to_bits(), 0x3f8a_0d15);
        assert_eq!(d16.scale[1].to_bits(), 0x3f8a_0d15);
        assert_eq!(d16.scale[2].to_bits(), 0x3f8a_0d15);
        assert_eq!((d16.flags, d16.life, d16.creation_number), (0x02, 0xff, 30));

        let d25 = &d.instances()[25];
        assert_eq!(d25.type_id, *b"LTw2");
        assert_eq!(d25.variation, 2);
        assert_eq!(d25.position[0].to_bits(), 0xc418_0000);
        assert_eq!(d25.position[1].to_bits(), 0x4478_0000);
        assert_eq!(d25.position[2].to_bits(), 0xc05e_0000);
        assert_eq!(d25.angle.to_bits(), 0x4096_cbe5);
        assert_eq!(d25.scale, [1.0, 1.0, 1.0]);
        assert_eq!((d25.flags, d25.life, d25.creation_number), (0x02, 0x64, 35));

        let last = &d.instances()[64];
        assert_eq!(last.type_id, *b"LOth");
        assert_eq!(last.variation, 0);
        assert_eq!(last.angle.to_bits(), 0x4096_3ceb);
        assert_eq!(last.scale, [1.0, 1.0, 1.0]);
        assert_eq!(
            (last.flags, last.life, last.creation_number),
            (0x02, 0xff, 51)
        );
    }

    #[test]
    fn doodad_framing_consumes_the_file_exactly() {
        let Some(dir) = install_dir() else { return };
        for path in [MAP_SHEEP, MAP_ORC02] {
            let Some(m) = shipped(&dir, path) else {
                return;
            };
            let raw = m.extract("war3map.doo").unwrap();
            let d = m.doodads().unwrap();
            let want = 16 + 42 * d.instances().len() + 8 + 16 * d.terrain_doodads().len();
            assert_eq!(raw.len(), want, "{path}");
            assert_eq!(d.canonical_bytes(), raw, "{path}");
        }
    }

    #[test]
    fn terrain_doodads_are_exact() {
        // Hand-read from Maps\Campaign\Orc02.w3m's terrain section: head 0
        // and 8, then eight 16-byte records of (fourcc, variation, x, y).
        let Some(dir) = install_dir() else { return };
        let Some(m) = shipped(&dir, MAP_ORC02) else {
            return;
        };
        let d = m.doodads().unwrap();
        assert_eq!(d.terrain_section_version(), 0);
        let t = d.terrain_doodads();
        assert_eq!(t.len(), 8);
        let want = [
            (*b"LCc0", 23, 152),
            (*b"LCc2", 45, 104),
            (*b"LCc0", 49, 65),
            (*b"LCc0", 56, 76),
            (*b"LCc0", 64, 152),
            (*b"LCc0", 13, 81),
            (*b"LCc0", 24, 141),
            (*b"LCc2", 41, 152),
        ];
        for (r, (id, x, y)) in t.iter().zip(&want) {
            assert_eq!(r.type_id, *id);
            assert_eq!(r.variation, 0);
            assert_eq!((r.grid_x, r.grid_y), (*x, *y));
        }
        // Whole grid cells inside the map, not world floats.
        let terr = m.terrain().unwrap();
        assert_eq!((terr.width(), terr.height()), (96, 160));
        assert!(
            t.iter()
                .all(|r| r.grid_x < terr.width() && r.grid_y < terr.height())
        );
    }

    #[test]
    fn creation_numbers_are_unique_but_not_dense() {
        let Some(dir) = install_dir() else { return };
        // Orc02: 1463 instances but ids up to 1464. Undead08: 1271 up to
        // 1573. They are editor identities, not indices.
        let Some(m) = shipped(&dir, MAP_ORC02) else {
            return;
        };
        let d = m.doodads().unwrap();
        assert!(d.creation_numbers_are_unique());
        assert_eq!(d.instances().len(), 1463);
        assert_eq!(d.creation_number_range(), Some((0, 1464)));
        assert!(d.by_creation_number(1464).is_some());
        assert!(d.by_creation_number(1465).is_none());

        let Some(m) = shipped(&dir, MAP_128C) else {
            return;
        };
        let d = m.doodads().unwrap();
        assert!(d.creation_numbers_are_unique());
        assert_eq!(d.instances().len(), 1271);
        assert_eq!(d.creation_number_range(), Some((0, 1573)));
    }

    #[test]
    fn unproven_doo_version_is_refused() {
        // Version 8 adds item-drop fields no shipped byte proves the place
        // of. One synthetic flip of the version word is refused, not guessed.
        let Some(dir) = install_dir() else { return };
        let Some(m) = shipped(&dir, MAP_SHEEP) else {
            return;
        };
        let mut raw = m.extract("war3map.doo").unwrap();
        raw[4..8].copy_from_slice(&9u32.to_le_bytes());
        assert!(matches!(
            Doodads::from_bytes(raw),
            Err(Error::UnsupportedDoodadVersion(9))
        ));
    }

    #[test]
    fn truncated_doo_is_refused() {
        let Some(dir) = install_dir() else { return };
        let Some(m) = shipped(&dir, MAP_SHEEP) else {
            return;
        };
        let mut raw = m.extract("war3map.doo").unwrap();
        raw.truncate(raw.len() - 8);
        assert!(matches!(
            Doodads::from_bytes(raw),
            Err(Error::DoodadTruncated { .. })
        ));
        // A file one byte longer than its counts describe is refused too.
        let mut raw = m.extract("war3map.doo").unwrap();
        raw.push(0);
        assert!(matches!(
            Doodads::from_bytes(raw),
            Err(Error::DoodadSize { .. })
        ));
    }

    // ---- shadows --------------------------------------------------------

    #[test]
    fn shadow_grid_is_four_cells_per_tile_edge() {
        // Hand-read from (4)Adrenaline.w3m: 64x64 tiles, so the file is
        // 256 x 256 bytes. The first 0xFF is byte 1130, cell (106, 4).
        let Some(dir) = install_dir() else { return };
        let Some(m) = shipped(&dir, MAP_128C_CUSTOM) else {
            return;
        };
        let t = m.terrain().unwrap();
        assert_eq!((t.width(), t.height()), (64, 64));
        let raw = m.extract("war3map.shd").unwrap();
        assert_eq!(raw.len(), 65536);
        let s = m.shadows().unwrap();
        assert_eq!((s.columns(), s.rows()), (256, 256));
        assert_eq!(s.cell(0, 0), Some(0x00));
        assert_eq!(s.cell(106, 4), Some(0xFF));
        assert_eq!(s.cell(255, 255), Some(0x00));
        assert_eq!(s.cell(256, 0), None);
        assert_eq!(s.is_shadowed(106, 4), Some(true));
        assert_eq!(
            s.values_used().into_iter().collect::<Vec<_>>(),
            [0x00, 0xFF]
        );
    }

    #[test]
    fn shadow_size_must_match_the_terrain() {
        let Some(dir) = install_dir() else { return };
        let Some(m) = shipped(&dir, MAP_128C_CUSTOM) else {
            return;
        };
        let raw = m.extract("war3map.shd").unwrap();
        // The same bytes are refused against the wrong map: the grid size
        // comes from the terrain, never from the file.
        assert!(matches!(
            Shadows::from_bytes(raw.clone(), 65, 64),
            Err(Error::BadShadowSize {
                width: 65,
                height: 64,
                ..
            })
        ));
        assert!(matches!(
            Shadows::from_bytes(raw[..16 * 64 * 63].to_vec(), 64, 64),
            Err(Error::BadShadowSize { .. })
        ));
    }

    // ---- minimap --------------------------------------------------------

    #[test]
    fn minimap_markers_are_exact() {
        // Hand-read from (2)Harrow.w3m's war3map.mmp: head 0 and 7, then
        // seven 16-byte markers. Colours are little-endian 0xAARRGGBB:
        // 03 03 ff ff is 0xFFFF0303 (player red), ff 42 00 ff is
        // 0xFF0042FF (player blue).
        let Some(dir) = install_dir() else { return };
        let Some(m) = shipped(&dir, MAP_HARROW) else {
            return;
        };
        let raw = m.extract("war3map.mmp").unwrap();
        assert_eq!(u32::from_le_bytes(raw[0..4].try_into().unwrap()), 0);
        assert_eq!(u32::from_le_bytes(raw[4..8].try_into().unwrap()), 7);
        assert_eq!(raw.len(), 8 + 16 * 7);
        let mm = m.minimap_markers().unwrap();
        assert_eq!(mm.version(), 0);
        let ms = mm.markers();
        assert_eq!(ms.len(), 7);
        assert_eq!(
            ms[0],
            Marker {
                kind: 0,
                x: 24,
                y: 21,
                colour: 0xFFFF_FFFF
            }
        );
        assert_eq!(
            ms[2],
            Marker {
                kind: 1,
                x: 128,
                y: 137,
                colour: 0xFFFF_FFFF
            }
        );
        assert_eq!(
            ms[5],
            Marker {
                kind: 2,
                x: 220,
                y: 223,
                colour: 0xFFFF_0303
            }
        );
        assert_eq!(
            ms[6],
            Marker {
                kind: 2,
                x: 39,
                y: 42,
                colour: 0xFF00_42FF
            }
        );
        assert_eq!(mm.kinds_used().into_iter().collect::<Vec<_>>(), [0, 1, 2]);
    }

    #[test]
    fn minimap_marker_framing_is_exact() {
        let Some(dir) = install_dir() else { return };
        for path in [MAP_HARROW, MAP_128C_CUSTOM] {
            let Some(m) = shipped(&dir, path) else {
                return;
            };
            let raw = m.extract("war3map.mmp").unwrap();
            let mm = m.minimap_markers().unwrap();
            assert_eq!(raw.len(), 8 + 16 * mm.markers().len(), "{path}");
            assert_eq!(mm.canonical_bytes(), raw, "{path}");
        }
        // Adrenaline is a four-player map: 13 markers, and the four kind-2
        // ones carry the players' colours in red, blue, teal, purple order
        // of appearance.
        let Some(m) = shipped(&dir, MAP_128C_CUSTOM) else {
            return;
        };
        let mm = m.minimap_markers().unwrap();
        assert_eq!(mm.markers().len(), 13);
        let players: Vec<u32> = mm
            .markers()
            .iter()
            .filter(|x| x.kind == 2)
            .map(|x| x.colour)
            .collect();
        assert_eq!(
            players,
            [0xFF1C_E6B9, 0xFF00_42FF, 0xFFFF_0303, 0xFF54_0081]
        );
        assert_eq!(mm.coordinate_range(), Some((7, 248, 13, 249)));
    }

    #[test]
    fn minimap_image_is_blp_and_frames() {
        // Hand-read from (2)Harrow.w3m's war3mapMap.blp: BLP1, JPEG content,
        // 256x256, one mip at 0x49c with 0x6017 bytes, and a 624-byte shared
        // JPEG header — 0x49c + 0x6017 is the end of the 25 779-byte file.
        let Some(dir) = install_dir() else { return };
        let Some(m) = shipped(&dir, MAP_HARROW) else {
            return;
        };
        let raw = m.extract("war3mapMap.blp").unwrap();
        assert_eq!(raw.len(), 25779);
        assert_eq!(&raw[..4], b"BLP1");
        assert_eq!(
            u32::from_le_bytes(raw[0x1c..0x20].try_into().unwrap()),
            0x49c
        );
        assert_eq!(
            u32::from_le_bytes(raw[0x5c..0x60].try_into().unwrap()),
            0x6017
        );
        assert_eq!(u32::from_le_bytes(raw[0x9c..0xa0].try_into().unwrap()), 624);
        let img = m.minimap_image().unwrap();
        assert_eq!(img.name, "war3mapMap.blp");
        assert!(img.is_blp());
        assert_eq!(img.dimensions(), (256, 256));
        assert!(img.framing_holds());
        match img.kind() {
            MinimapImageKind::Blp { header, framing } => {
                assert_eq!(header.encoding(), pcx_blp::Encoding::Jpeg);
                assert_eq!(header.mip_count(), 1);
                assert!(framing.sums());
                assert!(framing.jpeg_gap_is_padding(&raw));
            }
            other => panic!("expected a BLP, got {other:?}"),
        }
    }

    #[test]
    fn tracker_minimap_names_are_absent() {
        // The measured fact behind the minimap rename: war3map.map and
        // war3map.ter occur in none of the shipped maps. The pair that ships
        // is war3mapMap.blp + war3map.mmp.
        let Some(dir) = install_dir() else { return };
        for path in [MAP_SHEEP, MAP_HARROW, MAP_128C] {
            let Some(m) = shipped(&dir, path) else {
                return;
            };
            let c = m.content_files();
            assert_eq!(c.unproven_minimap, None, "{path}");
            assert!(c.doodads && c.shadows && c.minimap_markers, "{path}");
            assert_eq!(c.minimap_image, Some("war3mapMap.blp"), "{path}");
            assert!(m.extract("war3map.map").is_err(), "{path}");
            assert!(m.extract("war3map.ter").is_err(), "{path}");
        }
    }

    #[test]
    fn content_hashes_are_stable_and_recomputable() {
        let Some(dir) = install_dir() else { return };
        let Some(m) = shipped(&dir, MAP_128C_CUSTOM) else {
            return;
        };
        let d = m.doodads().unwrap();
        let d2 = Doodads::from_bytes(m.extract("war3map.doo").unwrap()).unwrap();
        assert_eq!(d.canonical_hash(), d2.canonical_hash());
        assert_ne!(d.canonical_hash(), 0);

        let s = m.shadows().unwrap();
        let s2 = Shadows::from_bytes(m.extract("war3map.shd").unwrap(), 64, 64).unwrap();
        assert_eq!(s.canonical_hash(), s2.canonical_hash());
        // The canonical form commits to the grid shape: the same bytes under
        // a transposed map hash differently, as they must.
        let transposed = Shadows::from_bytes(s.raw().to_vec(), 128, 32).unwrap();
        assert_ne!(s.canonical_hash(), transposed.canonical_hash());

        let mm = m.minimap_markers().unwrap();
        let mm2 = MinimapMarkers::from_bytes(m.extract("war3map.mmp").unwrap()).unwrap();
        assert_eq!(mm.canonical_hash(), mm2.canonical_hash());
        assert_eq!(mm.canonical_hash(), pcx_mpq::fnv1a64(&mm.canonical_bytes()));

        let img = m.minimap_image().unwrap();
        assert_eq!(img.canonical_hash(), pcx_mpq::fnv1a64(img.raw()));
    }

    #[test]
    fn legacy_build_uses_the_same_content_framing() {
        let Some(dir) = legacy_dir() else { return };
        let Some(m) = shipped(&dir, MAP_129B) else {
            return;
        };
        let d = m.doodads().unwrap();
        assert_eq!(d.version(), 7);
        assert_eq!(d.canonical_bytes(), d.raw());
        assert_eq!(
            d.raw().len(),
            16 + 42 * d.instances().len() + 8 + 16 * d.terrain_doodads().len()
        );
        let t = m.terrain().unwrap();
        let s = m.shadows().unwrap();
        assert_eq!(s.raw().len(), 16 * t.width() as usize * t.height() as usize);
        let mm = m.minimap_markers().unwrap();
        assert_eq!(mm.raw().len(), 8 + 16 * mm.markers().len());
        let img = m.minimap_image().unwrap();
        assert!(img.is_blp() && img.framing_holds());
        assert_eq!(m.content_files().unproven_minimap, None);
    }

    /// Full sweep over every shipped map on every available build.
    ///
    /// Ignored by default because it opens the whole install. Writes failures
    /// to `/tmp/opencode/map-sweep.txt` and prints a per-type census.
    #[test]
    #[ignore = "opens every shipped install; run explicitly"]
    fn sweep_every_shipped_map() {
        use std::collections::BTreeMap;
        use std::fmt::Write as _;
        let mut report = String::new();
        let mut totals = (0usize, 0usize, 0usize);
        let mut failures: Vec<String> = Vec::new();
        let mut dims: BTreeSet<(u32, u32)> = BTreeSet::new();
        let mut slots: BTreeSet<[u8; 4]> = BTreeSet::new();
        // Per type: (present, parsed) for doo, shd, mmp, minimap image;
        // plus how many maps carry an unproved name.
        let mut present = [0usize; 5];
        let mut parsed = [0usize; 4];
        let mut types: BTreeMap<String, usize> = BTreeMap::new();

        let mut builds: Vec<(&str, Option<PathBuf>)> =
            vec![("1.28c", install_dir()), ("1.29b", legacy_dir())];

        for (label, dir) in builds.drain(..) {
            let Some(dir) = dir else {
                let _ = writeln!(report, "{label}: install not present, skipped");
                continue;
            };
            let mut set = match pcx_mpq::MpqSet::open_wc3(&dir) {
                Ok(s) => s,
                Err(e) => {
                    let _ = writeln!(report, "{label}: open failed: {e}");
                    failures.push(format!("{label}: open failed: {e}"));
                    continue;
                }
            };
            let maps = match shipped_maps(&mut set) {
                Ok(m) => m,
                Err(e) => {
                    let _ = writeln!(report, "{label}: listfile failed: {e}");
                    failures.push(format!("{label}: listfile failed: {e}"));
                    continue;
                }
            };
            let mut here = (0usize, 0usize, 0usize);
            for path in &maps {
                here.0 += 1;
                let raw = match set.extract(path) {
                    Ok(b) => b,
                    Err(e) => {
                        failures.push(format!("{label} {path}: extract: {e}"));
                        continue;
                    }
                };
                let m = match Map::parse(&raw) {
                    Ok(m) => m,
                    Err(e) => {
                        failures.push(format!("{label} {path}: header: {e}"));
                        continue;
                    }
                };
                match m.terrain() {
                    Ok(t) => {
                        here.1 += 1;
                        dims.insert((t.width(), t.height()));
                        for s in t.slots().iter().chain(t.secondary_slots()) {
                            slots.insert(*s);
                        }
                        if t.canonical_hash() == 0 {
                            failures.push(format!("{label} {path}: zero hash"));
                        }
                        // The framing relation, restated per file.
                        let want = 17
                            + 4 * t.slots().len()
                            + 4
                            + 4 * t.secondary_slots().len()
                            + 16
                            + 7 * (t.width() as usize + 1) * (t.height() as usize + 1);
                        if t.canonical_bytes().len() != want {
                            failures.push(format!("{label} {path}: terrain framing"));
                        }
                    }
                    Err(e) => {
                        // The one .w3n has no single terrain file.
                        if path.to_ascii_lowercase().ends_with(".w3m") {
                            failures.push(format!("{label} {path}: terrain: {e}"));
                        }
                    }
                }

                let c = m.content_files();
                if c.doodads {
                    present[0] += 1;
                    *types.entry("war3map.doo".into()).or_default() += 1;
                    match m.doodads() {
                        Ok(d) => {
                            parsed[0] += 1;
                            let want =
                                16 + 42 * d.instances().len() + 8 + 16 * d.terrain_doodads().len();
                            if d.raw().len() != want {
                                failures.push(format!("{label} {path}: doo framing"));
                            }
                            if d.canonical_bytes() != d.raw() {
                                failures.push(format!("{label} {path}: doo canonical"));
                            }
                            if d.canonical_hash() == 0 {
                                failures.push(format!("{label} {path}: doo zero hash"));
                            }
                            if !d.creation_numbers_are_unique() {
                                failures.push(format!(
                                    "{label} {path}: doo duplicate creation numbers"
                                ));
                            }
                        }
                        Err(e) => failures.push(format!("{label} {path}: doo: {e}")),
                    }
                }
                if c.shadows {
                    present[1] += 1;
                    *types.entry("war3map.shd".into()).or_default() += 1;
                    match m.shadows() {
                        Ok(s) => {
                            parsed[1] += 1;
                            if s.raw().len() != 16 * s.width() as usize * s.height() as usize {
                                failures.push(format!("{label} {path}: shd framing"));
                            }
                            if s.canonical_hash() == 0 {
                                failures.push(format!("{label} {path}: shd zero hash"));
                            }
                        }
                        Err(e) => failures.push(format!("{label} {path}: shd: {e}")),
                    }
                }
                if c.minimap_markers {
                    present[2] += 1;
                    *types.entry("war3map.mmp".into()).or_default() += 1;
                    match m.minimap_markers() {
                        Ok(mm) => {
                            parsed[2] += 1;
                            if mm.raw().len() != 8 + 16 * mm.markers().len() {
                                failures.push(format!("{label} {path}: mmp framing"));
                            }
                            if mm.canonical_bytes() != mm.raw() {
                                failures.push(format!("{label} {path}: mmp canonical"));
                            }
                        }
                        Err(e) => failures.push(format!("{label} {path}: mmp: {e}")),
                    }
                }
                if let Some(name) = c.minimap_image {
                    present[3] += 1;
                    *types.entry(name.to_string()).or_default() += 1;
                    match m.minimap_image() {
                        Ok(img) => {
                            parsed[3] += 1;
                            if !img.framing_holds() {
                                failures.push(format!("{label} {path}: minimap framing"));
                            }
                        }
                        Err(e) => failures.push(format!("{label} {path}: minimap: {e}")),
                    }
                }
                if let Some(n) = c.unproven_minimap {
                    present[4] += 1;
                    failures.push(format!("{label} {path}: unproven name {n}"));
                }
                here.2 += 1;
            }
            totals.0 += here.0;
            totals.1 += here.1;
            totals.2 += here.2;
            let _ = writeln!(
                report,
                "{label}: {} maps, {} terrain decoded, {} examined",
                here.0, here.1, here.2
            );
        }

        let _ = writeln!(report, "total: {} maps, {} decoded", totals.0, totals.1);
        let _ = writeln!(report, "distinct sizes: {:?}", dims);
        let _ = writeln!(report, "distinct slots: {}", slots.len());
        let names = ["war3map.doo", "war3map.shd", "war3map.mmp", "minimap image"];
        for (i, n) in names.iter().enumerate() {
            let _ = writeln!(report, "{n}: {} present, {} parsed", present[i], parsed[i]);
        }
        let _ = writeln!(
            report,
            "war3map.map / war3map.ter present: {} (expected 0)",
            present[4]
        );
        let _ = writeln!(report, "per-type counts: {:?}", types);
        let _ = writeln!(report, "failures: {}", failures.len());
        for f in &failures {
            let _ = writeln!(report, "  {f}");
        }

        let out = std::path::Path::new("/tmp/opencode/map-sweep.txt");
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
    }
}

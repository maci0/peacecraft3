//! Blizzard MPQ archive reader for the Warcraft III Complete Edition.
//!
//! Everything here is verified against the four shipped archives. The reader
//! gets as far as a decoded, decrypted, self-consistent hash and block table and
//! a working name lookup; file *payload* decryption and decompression are still
//! open (see [`Error::Unsupported`]).

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

pub mod checked;
pub mod crypt;
pub mod extraction;
pub mod hash;
pub mod header;
pub mod huffman;
pub mod payload;
pub mod sparse;

pub use extraction::{ExtractionBudget, ExtractionCause, ExtractionError};
pub use hash::HashResult;
pub use header::{Header, HeaderParseError};

/// 16-byte MPQ hash-table entry.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HashEntry {
    pub name1: u32,
    pub name2: u32,
    pub locale: u16,
    pub platform: u16,
    pub block_index: u32,
}

impl HashEntry {
    pub const EMPTY_MARK: u32 = 0xFFFF_FFFF;
    pub const DELETED_MARK: u32 = 0xFFFF_FFFE;
    pub const REGION_END_MARK: u32 = 0xFFFF_FFFD;

    pub fn is_unpopulated(&self) -> bool {
        matches!(
            self.name1,
            0 | Self::EMPTY_MARK | Self::DELETED_MARK | Self::REGION_END_MARK
        )
    }
}

/// 16-byte MPQ block-table entry.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BlockEntry {
    pub file_pos: u32,
    pub compressed_size: u32,
    pub file_size: u32,
    pub flags: u32,
}

pub mod flags {
    pub const IMPLODE: u32 = 0x0000_0100;
    pub const COMPRESS: u32 = 0x0000_0200;
    pub const ENCRYPTED: u32 = 0x0001_0000;
    pub const FIX_KEY: u32 = 0x0002_0000;
    pub const PATCH_FILE: u32 = 0x0010_0000;
    pub const SINGLE_UNIT: u32 = 0x0100_0000;
    pub const DELETE_MARKER: u32 = 0x0200_0000;
    pub const SECTOR_CRC: u32 = 0x0400_0000;
    pub const SIGNATURE: u32 = 0x0800_0000;
    pub const EXISTING: u32 = 0x8000_0000;
}

impl BlockEntry {
    pub fn exists(&self) -> bool {
        self.flags & flags::EXISTING != 0 && self.flags & flags::DELETE_MARKER == 0
    }
    pub fn is_encrypted(&self) -> bool {
        self.flags & flags::ENCRYPTED != 0
    }
    pub fn is_single_unit(&self) -> bool {
        self.flags & flags::SINGLE_UNIT != 0
    }
    pub fn is_patch(&self) -> bool {
        self.flags & flags::PATCH_FILE != 0
    }
    pub fn is_imploded(&self) -> bool {
        self.flags & flags::IMPLODE != 0
    }
    pub fn is_compressed(&self) -> bool {
        self.flags & flags::COMPRESS != 0
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("header: {0}")]
    Header(#[from] HeaderParseError),
    #[error("file not found in archive: {0}")]
    NotFound(String),
    #[error("block index {index} out of range ({count} entries)")]
    BadBlockIndex { index: u32, count: usize },
    #[error("unsupported: {0}")]
    Unsupported(&'static str),
    #[error("payload: {0}")]
    Payload(#[from] payload::PayloadError),
    #[error("bad PE magic {0:?}")]
    BadMagic([u8; 4]),
    #[error("truncated: {what} needs {need} bytes at {at}, file has {have}")]
    Truncated {
        what: &'static str,
        at: usize,
        need: usize,
        have: usize,
    },
}

/// Archive priority order, highest first: the game reads a newer archive before
/// falling back to an older one for the same internal path.
///
/// Matched case-insensitively and against a canonical name, because the two
/// builds spell them differently on a case-sensitive filesystem: 1.28c's retail
/// install uses `war3.mpq` / `War3xlocal.mpq`, while 1.29b's Battle.net install
/// uses `War3.mpq` / `War3Local.mpq`. An exact-match list would silently open
/// one of five archives on the primary target.
pub const DEFAULT_PRIORITY: [&str; 6] = [
    "war3patch.mpq",
    "war3x.mpq",
    "war3xlocal.mpq",
    "war3local.mpq",
    "war3.mpq",
    // 1.29b only: content Blizzard removed from the live game but still ships.
    "deprecated.mpq",
];

/// Canonical (lower-cased) name of each archive a WC3 install is expected to
/// hold, in the order the game reads them.
pub const ARCHIVE_NAMES: [&str; 6] = [
    "war3patch.mpq",
    "war3x.mpq",
    "war3xlocal.mpq",
    "war3local.mpq",
    "war3.mpq",
    "deprecated.mpq",
];

/// Finds `name` in `dir` ignoring case, preferring an exact match.
///
/// Needed because the legacy Battle.net build capitalises differently from the
/// retail disc, and Linux compares case-sensitively where Windows does not.
fn find_archive_ci(dir: &Path, name: &str) -> Option<PathBuf> {
    let want = name.to_ascii_lowercase();
    let mut found = None;
    let entries = std::fs::read_dir(dir).ok()?;
    for e in entries.flatten() {
        let p = e.path();
        if !p.is_file() {
            continue;
        }
        let Some(fname) = p.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let have = fname.to_ascii_lowercase();
        if have == want {
            return Some(p);
        }
        // Remember any other member of the same archive set so a build that
        // omits a patch archive still resolves the rest.
        if ARCHIVE_NAMES.contains(&have.as_str()) && found.is_none() {
            found = Some(p);
        }
    }
    found
}

/// Which classic retail build an install belongs to.
///
/// Both 1.28c and 1.29b are in scope. They are *expected* to be format-identical;
/// listing them separately exists so that a difference can be noticed rather than
/// silently averaged away.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Version {
    /// The Complete Edition retail build this project was first built against.
    V1_28,
    /// Battle.net product `w3_legacy`, build 1.29.2.9231 (installed executable VERSIONINFO). The last
    /// stable legacy release and this project's primary target.
    V1_29,
    /// The build could not be identified. Reported rather than guessed.
    ///
    /// A decoder that silently assumed a version would be wrong on exactly the
    /// builds nobody tested, so this is a first-class outcome and not a
    /// fallback.
    Unknown,
}

/// FNV-1a 64, used for install fingerprints.
///
/// Content hashes rather than a library dependency, because these are compared
/// against values recorded from this machine's own installs and never exposed.
pub fn fnv1a64(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in data {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

/// Install fingerprints: the FNV-1a 64 digest of a member archive's `(listfile)`,
/// matched one archive at a time.
///
/// The version resource in the binaries is unusable, and this is why. The
/// shipped 1.28c install's on-disk `game.dll` reports 1.26, because the retail
/// disc is a 1.26 base and the 1.28c code arrives inside the MPQs rather than by
/// patching the DLL. The `Game.dll` archived inside `war3.mpq` reports 1.0.
/// 1.29b does not ship `game.dll` at all — its game code is inside
/// `Warcraft III.exe`, whose own version resource says 1.29.2 but is the wrong
/// build. None of that is a patch version, so the build is identified from
/// archive content instead.
pub fn known_digests() -> std::collections::HashMap<u64, Version> {
    let mut m = std::collections::HashMap::new();

    // 1.29b — Battle.net product `w3_legacy`, build 1.29.2.9231 (installed executable VERSIONINFO).
    // Archives: War3.mpq, War3x.mpq, War3Local.mpq, War3xLocal.mpq,
    // Deprecated.mpq. Only the two largest carry a distinctive digest, but all
    // five are recorded so that a truncated install is still recognisable.
    m.insert(0x1474_00bf_2dbe_e1e1, Version::V1_29); // War3.mpq, 448,139 B
    m.insert(0x8202_bd6a_ca54_e51d, Version::V1_29); // War3x.mpq, 387,288 B
    m.insert(0x32fd_9ba6_5ffe_f93d, Version::V1_29); // War3Local.mpq, 70,119 B
    m.insert(0x37ab_a266_3b84_ffa1, Version::V1_29); // War3xLocal.mpq, 132,230 B
    m.insert(0xd2fa_ba8a_1b8f_e63f, Version::V1_29); // Deprecated.mpq, 32,347 B

    // 1.28c — Complete Edition retail, lower-case archive names.
    m.insert(0xf46a_7f2a_a311_f7ab, Version::V1_28); // war3.mpq, 529,298 B
    m
}

/// Internal paths whose digests are checked against every member archive.
///
/// `MpqSet::extract` returns the first archive that holds a path, so it would
/// always fetch `War3x.mpq`'s listfile and never see `war3.mpq`'s. The
/// fingerprints are therefore matched against each member individually.
pub const INSTALL_FINGERPRINTS: [&str; 1] = ["(listfile)"];

impl Version {
    /// True when the build is one this project has verified decoders against.
    pub fn is_known(self) -> bool {
        !matches!(self, Version::Unknown)
    }

    /// `(major, minor)` as the binary reports it.
    pub fn parts(self) -> (u16, u16) {
        match self {
            Version::V1_28 => (1, 28),
            Version::V1_29 => (1, 29),
            Version::Unknown => (0, 0),
        }
    }

    pub fn label(self) -> String {
        match self {
            Version::V1_28 => "1.28c".into(),
            Version::V1_29 => "1.29b".into(),
            Version::Unknown => "unknown".into(),
        }
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label())
    }
}

/// Where an archive's bytes come from.
///
/// Two sources, one interface: a file on disk, or bytes already in memory. The
/// second exists for the WASM target, where a browser fetches the archive over
/// HTTP and there is no filesystem to `seek` — and it is also what makes an
/// `MpqSet` constructible in a test without a mounted install.
pub enum Source {
    /// A file on disk, read on demand.
    File(BufReader<File>),
    /// The whole archive already in memory.
    Memory(Vec<u8>),
}

impl Source {
    /// A reader over the source's bytes, so the header and table parsing can
    /// stay written against `Read`/`Seek` and work for either backing store.
    fn reader(&mut self) -> SourceReader<'_> {
        SourceReader { src: self, at: 0 }
    }

    fn len(&self) -> u64 {
        match self {
            Source::File(f) => f.get_ref().metadata().map(|m| m.len()).unwrap_or(0),
            Source::Memory(v) => v.len() as u64,
        }
    }

    fn read_at(&mut self, offset: u64, len: usize) -> Result<Vec<u8>, Error> {
        let start =
            usize::try_from(offset).map_err(|_| Error::BadBlockIndex { index: 0, count: 0 })?;
        match self {
            Source::File(f) => {
                f.seek(SeekFrom::Start(offset))?;
                let mut buf = vec![0u8; len];
                f.read_exact(&mut buf)?;
                Ok(buf)
            }
            Source::Memory(v) => {
                v.get(start..start + len)
                    .map(|s| s.to_vec())
                    .ok_or(Error::NotFound(format!(
                        "read of {len} bytes at {offset} is past the end of the {}-byte buffer",
                        v.len()
                    )))
            }
        }
    }
}

/// `Read` + `Seek` over a [`Source`].
///
/// The parser is written against `Read`/`Seek` because that is the natural shape
/// for a streamed archive; this adapts an in-memory buffer to the same traits so
/// one parser serves both stores and they cannot drift.
struct SourceReader<'a> {
    src: &'a mut Source,
    at: u64,
}

impl std::io::Read for SourceReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // A file is handed to `Read` directly: its own cursor is the position,
        // and `Seek` below keeps `self.at` in step with it.
        match &mut self.src {
            Source::File(f) => {
                let n = f.read(buf)?;
                self.at += n as u64;
                Ok(n)
            }
            Source::Memory(v) => {
                let start = usize::try_from(self.at).unwrap_or(v.len());
                let avail = v.len().saturating_sub(start);
                let n = avail.min(buf.len());
                buf[..n].copy_from_slice(&v[start..start + n]);
                self.at += n as u64;
                Ok(n)
            }
        }
    }
}

impl std::io::Seek for SourceReader<'_> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        match &mut self.src {
            Source::File(f) => {
                self.at = f.seek(pos)?;
                Ok(self.at)
            }
            Source::Memory(v) => {
                let len = v.len() as u64;
                self.at = match pos {
                    SeekFrom::Start(n) => n.min(len),
                    SeekFrom::End(n) => (len as i64 + n).clamp(0, len as i64) as u64,
                    SeekFrom::Current(n) => (self.at as i64 + n).clamp(0, len as i64) as u64,
                };
                Ok(self.at)
            }
        }
    }
}

pub struct MpqArchive {
    file: Source,
    path: PathBuf,
    pub header: Header,
    hashes: Vec<HashEntry>,
    blocks: Vec<BlockEntry>,
    huffman: Option<huffman::HuffmanTable>,
    huffman_checked: bool,
}

fn read_tables<R: Read + Seek>(
    reader: &mut R,
    h: Header,
) -> Result<(Vec<HashEntry>, Vec<BlockEntry>), Error> {
    if !h.is_self_consistent() {
        return Err(Error::Unsupported("header table layout is inconsistent"));
    }
    let t = crypt::table();

    let mut raw = vec![0u8; h.hash_table_bytes() as usize];
    reader.seek(SeekFrom::Start(u64::from(h.hash_table_pos)))?;
    reader.read_exact(&mut raw)?;
    crypt::decrypt(&mut raw, hash::hash_table_key(), t);
    let hashes = raw
        .as_chunks::<16>()
        .0
        .iter()
        .map(|e| HashEntry {
            name1: u32::from_le_bytes(e[0..4].try_into().unwrap()),
            name2: u32::from_le_bytes(e[4..8].try_into().unwrap()),
            locale: u16::from_le_bytes(e[8..10].try_into().unwrap()),
            platform: u16::from_le_bytes(e[10..12].try_into().unwrap()),
            block_index: u32::from_le_bytes(e[12..16].try_into().unwrap()),
        })
        .collect();

    let mut raw = vec![0u8; h.block_table_bytes() as usize];
    reader.seek(SeekFrom::Start(u64::from(h.block_table_pos)))?;
    reader.read_exact(&mut raw)?;
    crypt::decrypt(&mut raw, hash::block_table_key(), t);
    let blocks = raw
        .as_chunks::<16>()
        .0
        .iter()
        .map(|e| BlockEntry {
            file_pos: u32::from_le_bytes(e[0..4].try_into().unwrap()),
            compressed_size: u32::from_le_bytes(e[4..8].try_into().unwrap()),
            file_size: u32::from_le_bytes(e[8..12].try_into().unwrap()),
            flags: u32::from_le_bytes(e[12..16].try_into().unwrap()),
        })
        .collect();

    Ok((hashes, blocks))
}

/// Huffman weights come from the archive directory's `Storm.dll`, else its
/// `Warcraft III.exe`. Legacy 1.29 ships no Storm.dll and links the weights into
/// the executable; the same table decodes its WAV members identically.
fn codec_source(directory: &Path) -> std::io::Result<Option<PathBuf>> {
    let (mut storm, mut game) = (None, None);
    for entry in std::fs::read_dir(directory)? {
        let candidate = entry?.path();
        let name = candidate
            .file_name()
            .map(|name| name.to_string_lossy().to_ascii_lowercase());
        match name.as_deref() {
            Some("storm.dll") => storm = Some(candidate),
            Some("warcraft iii.exe") => game = Some(candidate),
            _ => {}
        }
    }
    Ok(storm.or(game))
}

impl MpqArchive {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref().to_path_buf();
        let mut file = Source::File(BufReader::new(File::open(&path)?));
        let header = Header::parse(&mut file.reader())?;
        let mut me = Self {
            file,
            path,
            header,
            hashes: Vec::new(),
            blocks: Vec::new(),
            huffman: None,
            huffman_checked: false,
        };
        me.load_tables()?;
        Ok(me)
    }

    /// Import local Storm.dll bytes, including for memory archives and browser imports.
    /// Malformed, truncated or unsupported PE input fails before replacing the table.
    pub fn set_huffman_table(
        &mut self,
        bytes: &[u8],
        source: impl AsRef<Path>,
    ) -> Result<(), huffman::HuffmanError> {
        let table = huffman::HuffmanTable::from_pe(bytes, source)?;
        self.huffman = Some(table);
        self.huffman_checked = true;
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn hash_entries(&self) -> &[HashEntry] {
        &self.hashes
    }

    pub fn block_entries(&self) -> &[BlockEntry] {
        &self.blocks
    }

    /// Decrypt and decode both tables through the shared reader.
    fn load_tables(&mut self) -> Result<(), Error> {
        let (hashes, blocks) = read_tables(&mut self.file.reader(), self.header)?;
        self.hashes = hashes;
        self.blocks = blocks;
        Ok(())
    }

    /// Resolve an archive-internal path to its block entry.
    ///
    /// Uses the Storm probe order: start at `hash(name, 0) % entries`, then walk
    /// forward until an unpopulated slot ends the chain.
    pub fn find_block(&self, path: &str) -> Option<&BlockEntry> {
        let (name1, name2) = hash::hash_string_both(path);
        if self.hashes.is_empty() {
            return None;
        }
        let n = self.hashes.len();
        let mut slot = hash::start_slot(path, n);
        for _ in 0..n {
            let e = &self.hashes[slot];
            if e.is_unpopulated() {
                return None;
            }
            if e.name1 == name1 && e.name2 == name2 {
                let idx = e.block_index as usize;
                if idx < self.blocks.len() && self.blocks[idx].exists() {
                    return Some(&self.blocks[idx]);
                }
                return None;
            }
            slot += 1;
            if slot == n {
                slot = 0;
            }
        }
        None
    }

    pub fn contains(&self, path: &str) -> bool {
        self.find_block(path).is_some()
    }

    /// Block entries whose flags say the payload is stored verbatim and
    /// unencrypted. Useful for RE: these are directly comparable to the bytes
    /// sitting at `file_pos`.
    pub fn readable_blocks(&self) -> impl Iterator<Item = (usize, &BlockEntry)> {
        self.blocks.iter().enumerate().filter(|(_, b)| {
            b.exists()
                && !b.is_encrypted()
                && !b.is_compressed()
                && !b.is_imploded()
                && b.file_size > 0
        })
    }

    pub fn block(&self, index: usize) -> Result<&BlockEntry, Error> {
        self.blocks.get(index).ok_or(Error::BadBlockIndex {
            index: index as u32,
            count: self.blocks.len(),
        })
    }

    /// Open an archive whose bytes are already in memory.
    ///
    /// The same code path as [`MpqArchive::open`], so a browser fetch and a
    /// mounted file are not two implementations that can drift. `name` is only
    /// used in errors, since there is no path to report.
    pub fn from_bytes(bytes: Vec<u8>, name: &str) -> Result<Self, Error> {
        let mut me = Self {
            file: Source::Memory(bytes),
            path: PathBuf::from(name),
            header: Header::default(),
            hashes: Vec::new(),
            blocks: Vec::new(),
            huffman: None,
            huffman_checked: false,
        };
        me.header = Header::parse(&mut me.file.reader())?;
        me.load_tables()?;
        Ok(me)
    }

    /// Read `len` bytes at absolute archive `offset`.
    pub fn read_at(&mut self, offset: u64, len: usize) -> Result<Vec<u8>, Error> {
        self.file.read_at(offset, len)
    }

    /// Total size of the archive's bytes.
    pub fn byte_len(&self) -> u64 {
        self.file.len()
    }

    /// Read the raw bytes a block occupies, with no decryption or decompression.
    pub fn read_block_raw(&mut self, block: &BlockEntry) -> Result<Vec<u8>, Error> {
        self.read_at(u64::from(block.file_pos), block.compressed_size as usize)
    }

    /// Extract a file: read its packed bytes, decrypt the sector layout and
    /// decompress each sector.
    pub fn extract(&mut self, path: &str) -> Result<Vec<u8>, Error> {
        let block = *self
            .find_block(path)
            .ok_or_else(|| Error::NotFound(path.to_string()))?;
        let raw = self.read_block_raw(&block)?;
        let key = payload::file_key(path, block.file_pos, block.file_size, block.flags);
        let sector_size = self.header.sector_size();
        let decode = |table: Option<&huffman::HuffmanTable>| {
            payload::decode_block_with_huffman(
                &raw,
                key,
                block.file_size,
                block.flags,
                sector_size,
                table,
            )
        };
        match decode(self.huffman.as_ref()) {
            Err(payload::PayloadError::UnsupportedCompression { mask })
                if payload::needs_codec_tables(mask)
                    && self.huffman.is_none()
                    && !self.huffman_checked
                    && matches!(self.file, Source::File(_)) =>
            {
                self.huffman_checked = true;
                let directory = self.path.parent().unwrap_or(Path::new("."));
                let Some(source) = codec_source(directory)? else {
                    return Err(payload::PayloadError::UnsupportedCompression { mask }.into());
                };
                self.huffman = Some(
                    huffman::HuffmanTable::open(source).map_err(payload::PayloadError::Huffman)?,
                );
                Ok(decode(self.huffman.as_ref())?)
            }
            result => Ok(result?),
        }
    }
}

/// A set of archives searched as one virtual filesystem.
pub struct MpqSet {
    members: Vec<MpqArchive>,
}

/// Reads `VS_VERSION_INFO` / `VS_FIXEDFILEINFO` from a PE to recover the file
/// version. Returns the high word of the version, i.e. 28 for a 1.28.x file.
///
/// Walks the optional header's data directories for the resource table rather
/// than searching for a pattern, so a coincidental byte sequence cannot produce
/// a version.
pub fn pe_file_version(path: &Path) -> Result<u32, Error> {
    let d = std::fs::read(path)?;
    if d.len() < 0x40 || &d[0..2] != b"MZ" {
        return Err(Error::BadMagic([0, 0, 0, 0]));
    }
    let pe = u32::from_le_bytes(d[0x3c..0x40].try_into().unwrap()) as usize;
    if pe + 90 > d.len() || &d[pe..pe + 4] != b"PE\0\0" {
        return Err(Error::BadMagic([0, 0, 0, 0]));
    }
    let nsec = u16::from_le_bytes(d[pe + 6..pe + 8].try_into().unwrap()) as usize;
    let opt = pe + 24;
    let magic = u16::from_le_bytes(d[opt..opt + 2].try_into().unwrap());
    // Data directories sit at the same offset in PE32+ regardless of pointer
    // width for the first eight entries; debug and version are beyond that, so
    // be explicit about which header we are on.
    let (dd, dirs): (usize, usize) = if magic == 0x20b {
        (opt + 112, 16)
    } else {
        (opt + 96, 16)
    };
    if dd + dirs * 8 > d.len() {
        return Err(Error::Truncated {
            what: "data directories",
            at: dd,
            need: dirs * 8,
            have: d.len().saturating_sub(dd),
        });
    }
    // VS_VERSION_INFO is directory index 11 (after export, import, resource, ...).
    let (rva, size) = (
        u32::from_le_bytes(d[dd + 11 * 8..][..4].try_into().unwrap()),
        u32::from_le_bytes(d[dd + 11 * 8 + 4..][..4].try_into().unwrap()),
    );
    if rva == 0 || size == 0 {
        return Err(Error::NotFound(format!(
            "no version info in {}",
            path.display()
        )));
    }
    let secs = opt + u16::from_le_bytes(d[pe + 20..pe + 22].try_into().unwrap()) as usize;
    let mut file_off = None;
    for i in 0..nsec {
        let b = secs + i * 40;
        if b + 40 > d.len() {
            break;
        }
        let (vs, va, rs, raw) = (
            u32::from_le_bytes(d[b + 8..b + 12].try_into().unwrap()),
            u32::from_le_bytes(d[b + 12..b + 16].try_into().unwrap()),
            u32::from_le_bytes(d[b + 16..b + 20].try_into().unwrap()),
            u32::from_le_bytes(d[b + 20..b + 24].try_into().unwrap()),
        );
        if va != 0 && va <= rva && rva < va + vs.max(rs) {
            file_off = Some((raw + (rva - va)) as usize);
            break;
        }
    }
    let off = file_off
        .ok_or_else(|| Error::NotFound(format!("cannot map version RVA in {}", path.display())))?;
    // VS_VERSIONINFO: wLength, wValueLength, wType, szValue (UTF-16 "VS_VERSION_INFO"),
    // padding, then VS_FIXEDFILEINFO whose first field is dwSignature.
    let sig = u32::from_le_bytes(
        d.get(off + 40..off + 44)
            .unwrap_or(&[0; 4])
            .try_into()
            .unwrap(),
    );
    if sig != 0xFEEF04BD {
        return Err(Error::NotFound(format!(
            "no VS_FIXEDFILEINFO in {}",
            path.display()
        )));
    }
    // dwFileVersionMS at +44, dwFileVersionLS at +48.
    let ms = u32::from_le_bytes(
        d.get(off + 44..off + 48)
            .unwrap_or(&[0; 4])
            .try_into()
            .unwrap(),
    );
    Ok(ms >> 16)
}

/// A recognised WC3 install: which classic build it is, and where it lives.
///
/// Carrying the version alongside the archive set is what keeps decoders from
/// hard-coding one build: they can branch on a value that was read out of the
/// shipped binary rather than on a directory name.
pub struct Install {
    pub dir: std::path::PathBuf,
    pub version: Version,
    pub archives: MpqSet,
}

impl Install {
    /// Opens `dir` and identifies the build by content.
    ///
    /// `Version::Unknown` is a normal outcome: an install nobody has recorded
    /// must be reported as unidentified rather than assumed, because the whole
    /// point of tracking the version is to notice when a decoder's assumptions
    /// stop holding.
    pub fn open(dir: impl AsRef<Path>) -> Result<Self, Error> {
        let dir = dir.as_ref().to_path_buf();
        let mut archives = MpqSet::open_wc3(&dir)?;

        let mut version = Version::Unknown;
        let known = known_digests();
        'outer: for path in INSTALL_FINGERPRINTS {
            for member in archives.members_mut() {
                if let Ok(bytes) = member.extract(path)
                    && let Some(hit) = known.get(&fnv1a64(&bytes))
                {
                    version = *hit;
                    break 'outer;
                }
            }
        }

        Ok(Self {
            dir,
            version,
            archives,
        })
    }

    pub fn extract(&mut self, path: &str) -> Result<Vec<u8>, Error> {
        self.archives.extract(path)
    }

    pub fn file_list(&mut self) -> Result<Vec<String>, Error> {
        self.archives.file_list()
    }
}

impl MpqSet {
    pub fn open_wc3(install_dir: impl AsRef<Path>) -> Result<Self, Error> {
        Self::open_named(install_dir, &DEFAULT_PRIORITY)
    }

    /// Open the named archives present in `install_dir`, highest priority first.
    /// Absent names are skipped; matching is case-insensitive.
    pub fn open_named(install_dir: impl AsRef<Path>, names: &[&str]) -> Result<Self, Error> {
        let dir = install_dir.as_ref();
        let mut members = Vec::new();
        let mut opened = std::collections::HashSet::new();
        for &name in names {
            let Some(p) = find_archive_ci(dir, name) else {
                continue;
            };
            // A build may expose the same file under two spellings; opening it
            // twice would double-count the archive in lookups.
            if !opened.insert(p.clone()) {
                continue;
            }
            members.push(MpqArchive::open(p)?);
        }
        if members.is_empty() {
            return Err(Error::NotFound(format!(
                "no MPQ archives in {}",
                dir.display()
            )));
        }
        Ok(Self { members })
    }

    pub fn len(&self) -> usize {
        self.members.len()
    }
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }
    pub fn members(&self) -> &[MpqArchive] {
        &self.members
    }
    pub fn members_mut(&mut self) -> &mut [MpqArchive] {
        &mut self.members
    }

    /// Find `path` in the highest-priority archive that holds it.
    pub fn locate(&self, path: &str) -> Option<(usize, &BlockEntry)> {
        self.members
            .iter()
            .enumerate()
            .find_map(|(i, a)| a.find_block(path).map(|b| (i, b)))
    }

    pub fn contains(&self, path: &str) -> bool {
        self.locate(path).is_some()
    }

    /// Extract `path` from the highest-priority archive that holds it.
    pub fn extract(&mut self, path: &str) -> Result<Vec<u8>, Error> {
        let i = self
            .members
            .iter()
            .position(|a| a.contains(path))
            .ok_or_else(|| Error::NotFound(path.to_string()))?;
        self.members[i].extract(path)
    }

    /// The union of every member's `(listfile)`, deduplicated.
    ///
    /// Each archive ships its own listfile, and the searched order puts
    /// `war3.mpq` last, so reading `(listfile)` through [`Self::extract`]
    /// silently returns only `War3Patch.mpq`'s view. The real on-disk file set
    /// is the union, so callers enumerating content must use this instead.
    pub fn file_list(&mut self) -> Result<Vec<String>, Error> {
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        let mut missing = Vec::new();
        for a in self.members.iter_mut() {
            match a.extract("(listfile)") {
                Ok(bytes) => {
                    let Ok(text) = String::from_utf8(bytes) else {
                        missing.push(a.path().display().to_string());
                        continue;
                    };
                    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
                        if seen.insert(normalize(line)) {
                            out.push(line.to_string());
                        }
                    }
                }
                Err(_) => missing.push(a.path().display().to_string()),
            }
        }
        if out.is_empty() {
            return Err(Error::NotFound(format!(
                "no (listfile) in any archive (missing from {})",
                missing.join(", ")
            )));
        }
        Ok(out)
    }
}

/// Normalise an MPQ path: backslash separators, no leading slash, no `.`/`..`.
pub fn normalize(path: &str) -> String {
    path.replace('\\', "/")
        .split('/')
        .filter(|s| !s.is_empty() && *s != "." && *s != "..")
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_dir() -> Option<PathBuf> {
        let p = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("wc3/game/app");
        p.is_dir().then_some(p)
    }

    /// Slot counts measured from the shipped archives; a regression here means
    /// the table cipher or the header layout changed.
    const EXPECTED: &[(&str, usize, usize)] = &[
        ("war3.mpq", 10760, 32768),
        ("War3x.mpq", 6144, 32768),
        ("War3xlocal.mpq", 1145, 4096),
        ("War3Patch.mpq", 576, 16384),
    ];

    #[test]
    fn opens_every_shipped_archive_with_consistent_tables() {
        let Some(dir) = app_dir() else {
            eprintln!("skipping: no extracted install");
            return;
        };
        for (name, blocks, hashes) in EXPECTED {
            let a = MpqArchive::open(dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(a.block_entries().len(), *blocks, "{name} block count");
            assert_eq!(a.hash_entries().len(), *hashes, "{name} hash count");

            let good = a
                .block_entries()
                .iter()
                .filter(|b| {
                    b.exists()
                        && u64::from(b.file_pos) < u64::from(a.header.archive_size)
                        && b.compressed_size <= b.file_size
                })
                .count();
            let ratio = good as f64 / a.block_entries().len() as f64;
            // Measured floor is 94.2% (War3x.mpq); real files hold a handful of
            // entries that legitimately break the size assumptions.
            assert!(
                ratio > 0.90,
                "{name}: only {good} self-consistent block entries"
            );

            // Every existing block must be reachable, so the number of hash
            // entries pointing inside the block table cannot be smaller.
            let existing = a.block_entries().iter().filter(|b| b.exists()).count();
            let live = a
                .hash_entries()
                .iter()
                .filter(|e| {
                    !e.is_unpopulated() && (e.block_index as usize) < a.block_entries().len()
                })
                .count();
            assert!(
                live >= existing,
                "{name}: {live} live hash entries cannot cover {existing} existing blocks"
            );
        }
    }

    /// The primary target: the Battle.net legacy build, 1.29b.
    ///
    /// Skipped silently when the install is absent, but present it must be
    /// recognised *and* all five of its archives must be found — a case-only
    /// spelling difference would otherwise open just one.
    #[test]
    fn identifies_the_legacy_129_build() {
        let Ok(extra) = std::env::var("PEACECRAFT3_LEGACY_DIR") else {
            eprintln!("skipping: PEACECRAFT3_LEGACY_DIR unset");
            return;
        };
        let dir = std::path::Path::new(&extra);
        if !dir.join("War3.mpq").is_file() {
            eprintln!("skipping: no legacy install at {}", dir.display());
            return;
        }
        let mut install = Install::open(dir).unwrap_or_else(|e| panic!("{dir:?}: {e}"));
        assert_eq!(
            install.version,
            Version::V1_29,
            "the legacy install should be 1.29b, not {}",
            install.version
        );
        // All five archives, found despite the capitalisation differences.
        assert_eq!(
            install.archives.members().len(),
            5,
            "1.29b ships War3/War3x/War3Local/War3xLocal/Deprecated"
        );
        let files = install.file_list().expect("union listfile");
        assert!(files.len() > 10_000, "only {} files", files.len());
        // The Footman must exist and be the same size as in 1.28c, which is the
        // first sign the two builds are format-identical.
        let foot = install
            .extract("Units\\Human\\Footman\\Footman.mdx")
            .expect("footman missing from the legacy build");
        assert_eq!(foot.len(), 105_022, "the legacy Footman differs from 1.28c");
    }

    #[test]
    fn identifies_the_shipped_build() {
        let Some(dir) = app_dir() else { return };
        let install = Install::open(&dir).unwrap_or_else(|e| panic!("{dir:?}: {e}"));
        // The extracted install is the Complete Edition retail build.
        assert_eq!(
            install.version,
            Version::V1_28,
            "wc3/game/ should be 1.28c, not {}",
            install.version
        );
        assert_eq!(install.version.parts(), (1, 28));
        assert_eq!(install.version.to_string(), "1.28c");
    }

    /// An install nobody recorded must come back `Unknown`, not a guessed
    /// version.
    #[test]
    fn an_unrecorded_install_is_unknown_rather_than_guessed() {
        let Some(dir) = app_dir() else { return };
        let mut set = MpqSet::open_wc3(&dir).unwrap();

        // The real digests must all hit, and no two members must collide.
        let known = known_digests();
        let mut hits = std::collections::BTreeSet::new();
        for path in INSTALL_FINGERPRINTS {
            for m in set.members_mut() {
                if let Ok(bytes) = m.extract(path)
                    && let Some(v) = known.get(&fnv1a64(&bytes))
                {
                    hits.insert(*v);
                }
            }
        }
        assert!(
            hits.contains(&Version::V1_28),
            "fingerprint did not match the shipped install"
        );

        // A digest that matches nothing must yield Unknown.
        let bogus = 0x0123_4567_89ab_cdef;
        assert!(
            !known.contains_key(&bogus),
            "a bogus digest must not resolve to a build"
        );

        // fnv1a64 must be stable and must distinguish near-identical input.
        let mut map = std::collections::HashMap::new();
        for (i, probe) in [vec![0u8], vec![1u8], vec![0u8, 0u8]]
            .into_iter()
            .enumerate()
        {
            let d = fnv1a64(&probe);
            assert!(map.insert(d, i).is_none(), "fnv1a64 collided on {probe:?}");
        }
        assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);

        // Every recognised version must be distinct and must render itself.
        for v in [Version::V1_28, Version::V1_29, Version::Unknown] {
            assert!(!v.to_string().is_empty(), "version {v:?} has no label");
        }
        assert_eq!(Version::V1_28.to_string(), "1.28c");
        assert_eq!(Version::V1_29.to_string(), "1.29b");
        assert_eq!(Version::Unknown.to_string(), "unknown");
        assert!(Version::V1_28.is_known());
        assert!(!Version::Unknown.is_known());
    }

    #[test]
    fn lookup_is_stable_and_case_insensitive() {
        let Some(dir) = app_dir() else { return };
        let a = MpqArchive::open(dir.join("war3.mpq")).unwrap();
        let lower = a.contains("(listfile)");
        let upper = a.contains("(LISTFILE)");
        assert_eq!(lower, upper, "hash lookup must be case-insensitive");
        if lower {
            assert!(a.find_block("(listfile)").is_some());
        }
    }

    /// The searched order puts `war3.mpq` last, so a naive `(listfile)` read
    /// returns only the patch archive's view and hides most shipped content.
    #[test]
    fn file_list_unions_every_archive_rather_than_the_first_match() {
        let Some(dir) = app_dir() else { return };
        let mut set = MpqSet::open_wc3(&dir).unwrap();
        let naive: Vec<String> = {
            let bytes = set.extract("(listfile)").unwrap();
            String::from_utf8(bytes)
                .unwrap()
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(str::to_string)
                .collect()
        };
        let union = set.file_list().unwrap();

        let norm = |v: &[String]| -> std::collections::HashSet<String> {
            v.iter().map(|s| normalize(s)).collect()
        };
        let (naive_n, union_n) = (norm(&naive), norm(&union));

        assert!(
            union_n.len() > naive_n.len(),
            "union ({} entries) must exceed a single archive's listfile ({} entries)",
            union_n.len(),
            naive_n.len()
        );
        assert!(
            union_n.is_superset(&naive_n),
            "union dropped entries present in the patch listfile"
        );
        assert_eq!(
            union_n
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            union_n.len(),
            "file_list returned duplicates"
        );

        // A texture that lives in war3.mpq, the archive searched last.
        let from_war3 = "ReplaceableTextures\\WorldEditUI\\WarcraftIIIFTLogo.blp";
        assert!(
            union_n.contains(&normalize(from_war3)),
            "{from_war3} missing from union"
        );
        assert!(
            set.contains(from_war3),
            "{from_war3} is listed but not extractable through the set"
        );
    }

    #[test]
    fn extracts_the_listfile_end_to_end() {
        let Some(dir) = app_dir() else { return };
        let mut a = MpqArchive::open(dir.join("war3.mpq")).unwrap();
        let mut bytes = match a.extract("(listfile)") {
            Ok(b) => b,
            Err(e) => panic!("(listfile): {e}"),
        };
        assert!(bytes.len() > 400_000, "listfile truncated: {}", bytes.len());
        // CRLF-separated, and every line is an internal path.
        let text = String::from_utf8_lossy(&bytes).to_string();
        assert!(
            text.starts_with("ReplaceableTextures\\Selection\\SelectionCircleLarge.blp\r\n"),
            "listfile must start with the shipped texture entry"
        );
        let lines: Vec<&str> = text.split(['\r', '\n']).filter(|l| !l.is_empty()).collect();
        assert!(lines.len() > 5_000, "only {} listfile entries", lines.len());
        assert!(lines.iter().all(|l| !l.contains('\u{0}')));
        assert!(
            lines.iter().any(|l| l.contains("ReplaceableTextures")),
            "expected shipped paths in the listfile"
        );
        bytes.clear();
    }

    #[test]
    fn extracts_known_wc3_entries_across_archives() {
        const CANDIDATES: &[&str] = &[
            "ReplaceableTextures\\Selection\\SelectionCircleLarge.blp",
            "ReplaceableTextures\\Selection\\SelectionCircleSmall.blp",
            "war3map.w3e",
            "war3map.w3i",
            "table\\unit.ini",
            "units\\human\\Footman\\Footman.mdl",
            "war3common.w3e",
        ];
        let Some(dir) = app_dir() else { return };
        let mut decoded = 0;
        let mut seen = 0;
        for name in DEFAULT_PRIORITY {
            let Ok(mut a) = MpqArchive::open(dir.join(name)) else {
                continue;
            };
            for path in CANDIDATES {
                if !a.contains(path) {
                    continue;
                }
                seen += 1;
                let bytes = a
                    .extract(path)
                    .unwrap_or_else(|e| panic!("{name}: {path}: {e}"));
                assert!(!bytes.is_empty(), "{name}: {path} decoded empty");
                decoded += 1;
            }
        }
        assert!(seen > 0, "no candidate entry was present in any archive");
        assert!(decoded > 0, "no candidate entry decoded");
    }

    #[test]
    fn unknown_name_is_absent() {
        let Some(dir) = app_dir() else { return };
        let a = MpqArchive::open(dir.join("war3.mpq")).unwrap();
        assert!(a.find_block("definitely\\not\\a\\real\\file.xyz").is_none());
    }
}

#[cfg(test)]
mod memory_tests {
    use super::*;

    #[test]
    fn missing_foreign_and_truncated_storm_sources_are_typed_refusals() {
        let directory =
            std::env::temp_dir().join(format!("pcx-mpq-codec-errors-{}", std::process::id()));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("synthetic.mpq");
        std::fs::write(&path, [1u8, 0]).unwrap();
        let (name1, name2) = hash::hash_string_both("fixture");
        let make_archive = || MpqArchive {
            file: Source::File(BufReader::new(File::open(&path).unwrap())),
            path: path.clone(),
            header: Header::default(),
            hashes: vec![HashEntry {
                name1,
                name2,
                block_index: 0,
                ..HashEntry::default()
            }],
            blocks: vec![BlockEntry {
                file_pos: 0,
                file_size: 4,
                compressed_size: 2,
                flags: flags::EXISTING | flags::SINGLE_UNIT | flags::COMPRESS,
            }],
            huffman: None,
            huffman_checked: false,
        };
        assert!(matches!(
            make_archive().extract("fixture"),
            Err(Error::Payload(
                payload::PayloadError::UnsupportedCompression { mask: 1 }
            ))
        ));
        // Legacy has no Storm.dll: the directory's executable is the source.
        std::fs::write(directory.join("Warcraft III.exe"), b"foreign exe").unwrap();
        assert!(matches!(
            make_archive().extract("fixture"),
            Err(Error::Payload(payload::PayloadError::Huffman(
                huffman::HuffmanError::Source { source_path, .. }
            ))) if source_path.ends_with("Warcraft III.exe")
        ));
        // Storm.dll wins when both are present, and a bad source is never skipped.
        std::fs::write(directory.join("Storm.dll"), b"foreign DLL").unwrap();
        assert!(matches!(
            make_archive().extract("fixture"),
            Err(Error::Payload(payload::PayloadError::Huffman(
                huffman::HuffmanError::Source { source_path, .. }
            ))) if source_path.ends_with("Storm.dll")
        ));
        assert!(matches!(
            make_archive().set_huffman_table(b"MZ", "truncated.dll"),
            Err(huffman::HuffmanError::Source { .. })
        ));
        std::fs::remove_dir_all(directory).unwrap();
    }

    /// A malformed Storm.dll is refused and never replaced by a valid executable beside it.
    /// Storm.dll is the sole source once present.
    #[test]
    fn malformed_storm_does_not_fall_back_to_a_valid_executable() {
        let directory =
            std::env::temp_dir().join(format!("pcx-mpq-codec-priority-{}", std::process::id()));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("synthetic.mpq");
        std::fs::write(&path, [1u8, 0]).unwrap();
        let (name1, name2) = hash::hash_string_both("fixture");
        let make_archive = || MpqArchive {
            file: Source::File(BufReader::new(File::open(&path).unwrap())),
            path: path.clone(),
            header: Header::default(),
            hashes: vec![HashEntry {
                name1,
                name2,
                block_index: 0,
                ..HashEntry::default()
            }],
            blocks: vec![BlockEntry {
                file_pos: 0,
                file_size: 4,
                compressed_size: 2,
                flags: flags::EXISTING | flags::SINGLE_UNIT | flags::COMPRESS,
            }],
            huffman: None,
            huffman_checked: false,
        };
        let valid_exe = huffman::tests::synthetic_pe();
        std::fs::write(directory.join("Warcraft III.exe"), &valid_exe).unwrap();
        std::fs::write(directory.join("Storm.dll"), b"malformed DLL").unwrap();
        let outcome = make_archive().extract("fixture");
        std::fs::remove_dir_all(&directory).unwrap();
        assert!(
            matches!(
                outcome,
                Err(Error::Payload(payload::PayloadError::Huffman(
                    huffman::HuffmanError::Source { ref source_path, .. }
                ))) if source_path.ends_with("Storm.dll")
            ),
            "{outcome:?}"
        );
    }

    /// Without Storm.dll a valid Warcraft III.exe is the codec source: discovery accepts it,
    /// so the Huffman stage runs instead of refusing the mask.
    #[test]
    fn valid_executable_is_accepted_when_storm_is_absent() {
        let directory =
            std::env::temp_dir().join(format!("pcx-mpq-codec-exe-{}", std::process::id()));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("synthetic.mpq");
        std::fs::write(&path, [1u8, 0]).unwrap();
        std::fs::write(
            directory.join("Warcraft III.exe"),
            huffman::tests::synthetic_pe(),
        )
        .unwrap();
        let (name1, name2) = hash::hash_string_both("fixture");
        let mut archive = MpqArchive {
            file: Source::File(BufReader::new(File::open(&path).unwrap())),
            path: path.clone(),
            header: Header::default(),
            hashes: vec![HashEntry {
                name1,
                name2,
                block_index: 0,
                ..HashEntry::default()
            }],
            blocks: vec![BlockEntry {
                file_pos: 0,
                file_size: 4,
                compressed_size: 2,
                flags: flags::EXISTING | flags::SINGLE_UNIT | flags::COMPRESS,
            }],
            huffman: None,
            huffman_checked: false,
        };
        let outcome = archive.extract("fixture");
        std::fs::remove_dir_all(&directory).unwrap();
        // The synthetic weights in row one are empty, so the stream is refused as corrupt.
        assert!(
            matches!(
                outcome,
                Err(Error::Payload(payload::PayloadError::Huffman(
                    huffman::HuffmanError::Corrupt
                )))
            ),
            "{outcome:?}"
        );
    }

    /// An archive opened from memory must behave exactly like one opened from
    /// disk — same lookup, same bytes.
    ///
    /// This is the WASM prerequisite: a browser fetches bytes and there is no
    /// filesystem to `seek`. If the two paths can drift, the wasm build is a
    /// different program from the desktop one.
    #[test]
    fn an_in_memory_archive_matches_a_file_backed_one() {
        let Some(dir) = std::env::var_os("PEACECRAFT3_LEGACY_DIR")
            .map(PathBuf::from)
            .filter(|d| d.is_dir())
        else {
            eprintln!("skipping: no install");
            return;
        };
        let path = dir.join("War3.mpq");
        let bytes = std::fs::read(&path).expect("read the archive");

        let mut from_file = MpqArchive::open(&path).expect("open from file");
        let mut from_memory =
            MpqArchive::from_bytes(bytes.clone(), "War3.mpq").expect("open from memory");

        assert_eq!(from_file.byte_len(), from_memory.byte_len());
        assert_eq!(
            from_file.hash_entries().len(),
            from_memory.hash_entries().len()
        );
        assert_eq!(
            from_file.block_entries().len(),
            from_memory.block_entries().len()
        );

        // The real proof: the same path extracts to the same bytes either way.
        for rel in ["(listfile)", "Units\\Human\\Footman\\Footman.mdx"] {
            let a = from_file.extract(rel).expect("from file");
            let b = from_memory.extract(rel).expect("from memory");
            assert_eq!(a, b, "{rel} differs between the two backing stores");
        }

        // Reading past the end must fail rather than return zero-filled bytes.
        let past = from_memory.read_at(bytes.len() as u64 + 64, 16);
        assert!(past.is_err(), "a read past the end must be an error");
    }
}

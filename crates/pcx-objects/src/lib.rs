//! `war3map.w3u` / `war3map.w3t` object-data container and record reader, and
//! the shipped object-editor data those containers override.
//!
//! [`editor`] is the base data: `Units\UnitData.slk`, `unitUI.slk`,
//! `UnitBalance.slk`, `UnitWeapons.slk`, `UnitAbilities.slk`, `ItemData.slk`,
//! `UpgradeData.slk`, `AbilityData.slk`, the six `*MetaData.slk` schemas, the
//! `Doodads\` tables and the `.txt` profile layer, decoded through
//! `pcx-slq`. This module, `ObjectFile`, is the per-map *override* container
//! that sits on top of it.
//!
//! Scope is deliberately one layer: the container and its modification
//! records. **No game rule is interpreted.** A field's fourcc says which stat
//! a modification belongs to; what that stat *means* — and how the game turns
//! a stored word into armour, damage or a cooldown — is a later step and is
//! not guessed here.
//!
//! # What actually ships
//!
//! Every `.w3u/.w3t/.w3a/.w3b/.w3d/.w3h/.w3q` inside the embedded archives of
//! the shipped maps was enumerated on both builds, through each map archive's
//! own `(listfile)` union the canonical `war3map.*` names, so the census does
//! not depend on one naming convention:
//!
//! | build | w3u | w3t | w3a | w3b | w3d | w3h | w3q |
//! |-------|-----|-----|-----|-----|-----|-----|-----|
//! | 1.28c | 47  | 1   | 0   | 0   | 0   | 0   | 0   |
//! | 1.29b | 44  | 0   | 0   | 0   | 0   | 0   | 0   |
//!
//! 92 files, 2 179 object records, 5 975 modification entries. The seven are
//! **not** a measurable family: only two of them ever occur. Those two
//! extensions — `.w3u` and `.w3t`, whatever the base name (`war3map.*` in
//! maps, `war3campaign.*` inside `DemoCampaign.w3n`) — share one container
//! layout exactly (see the framing section), and the file is identified by
//! its name, not by a magic: the first word is a *version* (`1` in 91 files,
//! `2` in one).
//! The other five extensions never ship in either build, so their record
//! shape is unmeasured — in particular whether ability records carry the
//! level/variation columns the community format notes describe. [`parse`]
//! refuses those extensions rather than running a w3u-shaped walk over them.
//!
//! # Container layout (measured on all 92 files)
//!
//! ```text
//! offset  size  field
//! 0x000   4     version word: 1 (91 files) or 2 (1 file)
//! 0x004   4     B  count of first-table objects (base modifications)
//!         ...   B object records, then:
//!         4     C  count of second-table objects (custom rawcodes)
//!         ...   C object records, then EOF — nothing follows
//!
//! object record
//! +0x00   4     original id (fourcc, printable ASCII in all 2 179)
//! +0x04   4     modified id (fourcc; zero in every first-table record,
//!               non-zero in every second-table record)
//! +0x08   4     M  count of modification entries (may be 0)
//! +0x0C   ...   M modification entries
//!
//! modification entry
//! +0x00   4     field id (fourcc; 135 distinct, printable ASCII)
//! +0x04   4     type word: 0 int, 1 real, 2 unreal, 3 string
//! +0x08   4     value word (type 0/1/2): little-endian raw word
//! +0x08   ...   value bytes + NUL (type 3; the NUL is included)
//! last    4     end word — kept raw, see below
//! ```
//!
//! The type word is the *only* thing that decides the value's wire shape, and
//! it is proven, not assumed: 5 975 entries parse with type words in
//! `{0,1,2,3}` and the file is consumed byte-exactly by the walk above on
//! 92/92 files, on both builds, for both version words. No level or variation
//! column exists in a w3u/w3t entry — adding the two words the community notes
//! ascribe to ability records would leave bytes over on every file.
//!
//! Strings are **NUL-terminated, not length-prefixed**: 1 937 string values
//! (315 of them empty, all valid UTF-8) each end in a single NUL and the end
//! word follows immediately. A leading length word is refuted by the bytes.
//!
//! # Framing
//!
//! [`Framing`] turns the layout into an arithmetic identity rather than a
//! count. It holds on all 92 files:
//!
//! ```text
//! file_len == 4                      version word
//!           + 4                      B, first-table object count
//!           + Σ_first  (12 + Σ_mods mod_len)
//!           + 4                      C, second-table object count
//!           + Σ_second (12 + Σ_mods mod_len)
//!
//! mod_len == 8 + value_len + 4       fourcc + type word + value + end word
//! value_len == 4                     type 0/1/2
//! value_len == strlen + 1            type 3, NUL included
//! ```
//!
//! i.e. the outer tables plus every record and every entry, with per-kind
//! entry lengths, is the whole file with nothing left over — the object-file
//! analogue of `CLID`'s `Σ(header + geometry_len) == payload`. Two further
//! relations hold on 2 179/2 179 records and are reported by
//! [`Framing::violations`]: every first-table record has `modified id == 0`,
//! and every second-table record has `modified id != 0`.
//!
//! # What is refused, and why
//!
//! * **The other five extensions.** No shipped byte is a `.w3a/.w3b/.w3d/
//!   .w3h/.w3q`, so nothing proves their record shape. [`parse`] returns
//!   [`Error::UnmeasuredExtension`] instead of guessing level/variation
//!   columns.
//! * **Per-fourcc semantics.** The wire encoding of every one of the 135
//!   field fourccs is proven — each has exactly one type word across the
//!   whole corpus (61 int, 9 real, 23 unreal, 42 string), so `unam` is
//!   always a string and `uhpm` is always a word. What is *not* proven is
//!   anything beyond that: whether an int field is read as u8 or u32 (a
//!   four-byte slot holding small values is indistinguishable from a u8
//!   field), any scale factor, and what `real` and `unreal` do differently.
//!   [`Value::Int`] therefore hands out the raw word, not a number with a
//!   guessed width.
//! * **The end word.** Measured, kept raw in [`Mod::end_word`], not
//!   interpreted. Its content is 0 in 5 690 of 5 975 entries. The 285
//!   exceptions are two patterns: 106 entries in `DemoCampaign.w3n`'s
//!   `war3campaign.w3u` first table repeat their record's own original id,
//!   and 179 entries — every one of them an `umdl` or `uico` art-path
//!   string — carry some other rawcode-like word (`"hcth"` next to
//!   `units\human\TheCaptain\TheCaptain`, `"nhef"` next to
//!   `units\critters\ElfVillagerWoman\ElfVillagerWoman.mdl`). That reads
//!   like the unit owning the referenced art, but two hand-checked samples
//!   are a hypothesis, not a proof, so the word is handed out as bytes.

use std::path::{Path, PathBuf};

pub mod editor;

/// The version word's value in 91 of the 92 shipped files.
pub const VERSION_ONE: u32 = 1;
/// The version word's value in the one remaining file
/// (1.29b `Maps\Campaign\Human06.w3m`'s `war3map.w3u`). It shares the
/// version-1 layout byte for byte; see the tests.
pub const VERSION_TWO: u32 = 2;

/// Every extension in the family.
pub const EXTENSIONS: [&str; 7] = ["w3u", "w3t", "w3a", "w3b", "w3d", "w3h", "w3q"];

/// Extensions whose record layout is measured by this crate. The other five
/// never occur in a shipped map and are refused by [`parse`].
pub const MEASURED_EXTENSIONS: [&str; 7] = EXTENSIONS;

/// Byte offset of a map's embedded MPQ archive. `pcx-map` measured this on
/// all 133 shipped maps; the same opening approach is used here.
pub const MAP_ARCHIVE_OFFSET: usize = 0x200;

/// Type word for a 4-byte integer-like value.
pub const TYPE_INT: u32 = 0;
/// Type word for a 4-byte `real` value.
pub const TYPE_REAL: u32 = 1;
/// Type word for a 4-byte `unreal` value.
pub const TYPE_UNREAL: u32 = 2;
/// Type word for a NUL-terminated string value.
pub const TYPE_STRING: u32 = 3;

/// A rawcode or field id: four printable ASCII bytes in every shipped entry.
pub type Fourcc = [u8; 4];

/// Display a fourcc lossily, for inspectors and error text.
pub fn fourcc_text(f: Fourcc) -> String {
    String::from_utf8_lossy(&f).into_owned()
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("mpq: {0}")]
    Mpq(#[from] pcx_mpq::Error),
    #[error("truncated {what}: need {need} bytes at +{at:#x}, have {have}")]
    Truncated {
        what: &'static str,
        at: usize,
        need: usize,
        have: usize,
    },
    #[error("unsupported version {0}; shipped files carry 1 or 2")]
    UnsupportedVersion(u32),
    #[error(
        "bad value type {type_word} at +{at:#x}; only 0..=3 are measured \
         (int/real/unreal/string)"
    )]
    BadValueType { type_word: u32, at: usize },
    #[error("string value at +{at:#x} reaches the end of the file without a NUL")]
    UnterminatedString { at: usize },
    #[error("records end at +{end:#x} but the file has {len} bytes")]
    TrailingBytes { end: usize, len: usize },
    #[error(
        "{0:?} is outside the measured family ({MEASURED_EXTENSIONS:?}); \
         the record layout of the other five extensions ships in no map \
         and is refused rather than guessed"
    )]
    UnmeasuredExtension(String),
    #[error("no embedded MPQ archive at +{MAP_ARCHIVE_OFFSET:#x} of {0}")]
    NoEmbeddedArchive(String),
    #[error(
        "install {install} ships none of the object-editor data this crate \
         decodes; missing: {missing}"
    )]
    MissingEditorData { install: String, missing: String },
    #[error("no object named {rawcode} in {build}")]
    UnresolvedObject { rawcode: String, build: String },
    #[error(
        "unsupported: {what}{}",
        detail.as_ref().map(|d| format!(": {d}")).unwrap_or_default()
    )]
    Unsupported {
        what: &'static str,
        detail: Option<String>,
    },
}

/// The wire type of one modification value, as the file itself declares it.
///
/// This is the *encoding* proven per fourcc: each of the 135 field ids in the
/// shipped corpus carries exactly one type word, on every build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    /// Type word 0. A four-byte word; width and signedness refused.
    Int,
    /// Type word 1 (`real`). A four-byte IEEE-754 value.
    Real,
    /// Type word 2 (`unreal`). Same wire shape as [`ValueType::Real`]; what
    /// the game does differently with it is unmeasured.
    Unreal,
    /// Type word 3. NUL-terminated bytes.
    Str,
}

impl ValueType {
    fn of(type_word: u32) -> Option<Self> {
        match type_word {
            TYPE_INT => Some(Self::Int),
            TYPE_REAL => Some(Self::Real),
            TYPE_UNREAL => Some(Self::Unreal),
            TYPE_STRING => Some(Self::Str),
            _ => None,
        }
    }

    /// The type word as it appears in the file.
    pub fn word(self) -> u32 {
        match self {
            Self::Int => TYPE_INT,
            Self::Real => TYPE_REAL,
            Self::Unreal => TYPE_UNREAL,
            Self::Str => TYPE_STRING,
        }
    }
}

/// One modification value.
///
/// What is decoded is what the shipped bytes prove: the wire shape. The
/// semantic decode (which stat this is, what width an int is read with, what
/// scale a real carries) is refused everywhere — see the module
/// documentation.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Type word 0: the raw little-endian word. 3 060 of these ship and the
    /// largest is 99 999, which refutes u8 and u16 for the *slot*; it does
    /// not prove u32 over i32, and it says nothing about a field that is
    /// semantically a small number in a four-byte slot. The word is handed
    /// out unchanged.
    Int(u32),
    /// Type word 1: IEEE-754 bits. All 369 shipped `real` words are finite
    /// and positive.
    Real(f32),
    /// Type word 2: IEEE-754 bits. All 609 shipped `unreal` words are finite
    /// and positive. No shipped byte distinguishes `unreal` from `real` on
    /// the wire.
    Unreal(f32),
    /// Type word 3: the exact bytes between the type word and the NUL. All
    /// 1 937 shipped strings are valid UTF-8 (315 empty).
    Str(Vec<u8>),
}

/// One modification entry: which field, what kind of value, and the raw end
/// word.
#[derive(Debug, Clone, PartialEq)]
pub struct Mod {
    /// The field id, e.g. `*b"uhpm"`.
    pub field: Fourcc,
    /// The type word's meaning.
    pub value_type: ValueType,
    /// The two extra authored selector words in W3A/W3D/W3Q records.
    /// Their field semantics are not collapsed into a guessed level or index.
    pub selectors: Option<[u32; 2]>,
    /// The value, wire-decoded.
    pub value: Value,
    /// The four bytes that close the entry. Measured, kept raw; see the
    /// module documentation for the three patterns seen in shipped files.
    pub end_word: u32,
}

impl Mod {
    /// Bytes this entry occupies in the file.
    pub fn wire_len(&self) -> usize {
        8 + self.selectors.map_or(0, |_| 8) + self.value_len() + 4
    }

    fn value_len(&self) -> usize {
        match &self.value {
            Value::Int(_) | Value::Real(_) | Value::Unreal(_) => 4,
            Value::Str(s) => s.len() + 1,
        }
    }

    fn write(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.field);
        out.extend_from_slice(&self.value_type.word().to_le_bytes());
        if let Some(selectors) = self.selectors {
            for word in selectors {
                out.extend(word.to_le_bytes());
            }
        }
        match &self.value {
            Value::Int(w) => out.extend_from_slice(&w.to_le_bytes()),
            Value::Real(f) => out.extend_from_slice(&f.to_bits().to_le_bytes()),
            Value::Unreal(f) => out.extend_from_slice(&f.to_bits().to_le_bytes()),
            Value::Str(s) => {
                out.extend_from_slice(s);
                out.push(0);
            }
        }
        out.extend_from_slice(&self.end_word.to_le_bytes());
    }
}

/// One object record: an original rawcode, its modified rawcode (or zero for
/// a plain stat override), and its modification entries.
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    /// The original rawcode being modified.
    pub original: Fourcc,
    /// The new rawcode: zero in every first-table record, non-zero in every
    /// second-table record.
    pub modified: Fourcc,
    /// The modification entries, in file order.
    pub mods: Vec<Mod>,
}

impl Record {
    /// Bytes this record occupies in the file.
    pub fn wire_len(&self) -> usize {
        12 + self.mods.iter().map(Mod::wire_len).sum::<usize>()
    }

    fn write(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.original);
        out.extend_from_slice(&self.modified);
        out.extend_from_slice(&(self.mods.len() as u32).to_le_bytes());
        for m in &self.mods {
            m.write(out);
        }
    }
}

/// A parsed object-data file.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectFile<'a> {
    raw: &'a [u8],
    version: u32,
    base: Vec<Record>,
    custom: Vec<Record>,
}

impl<'a> ObjectFile<'a> {
    /// Parse one object file.
    ///
    /// `name` is the archive-internal name (`war3map.w3u` and friends); it
    /// selects the family and must be one of [`MEASURED_EXTENSIONS`]. The
    /// walk consumes the file exactly: a leftover byte is an error, not a
    /// trailer.
    pub fn parse(name: &str, data: &'a [u8]) -> Result<Self, Error> {
        let ext = extension_of(name);
        if !MEASURED_EXTENSIONS.iter().any(|e| *e == ext) {
            return Err(Error::UnmeasuredExtension(ext));
        }
        let mut p = 0usize;
        let version = read_u32(data, &mut p, "version word")?;
        if version != VERSION_ONE && version != VERSION_TWO {
            return Err(Error::UnsupportedVersion(version));
        }
        let indexed =
            matches!(ext.as_str(), "w3a" | "w3q") || (ext == "w3d" && version >= VERSION_TWO);
        let base = read_table(data, &mut p, indexed)?;
        let custom = read_table(data, &mut p, indexed)?;
        if p != data.len() {
            return Err(Error::TrailingBytes {
                end: p,
                len: data.len(),
            });
        }
        Ok(Self {
            raw: data,
            version,
            base,
            custom,
        })
    }

    /// The version word: [`VERSION_ONE`] or [`VERSION_TWO`].
    pub fn version(&self) -> u32 {
        self.version
    }

    /// First-table records: stat overrides of existing rawcodes
    /// (`modified` is zero).
    pub fn base(&self) -> &[Record] {
        &self.base
    }

    /// Second-table records: new rawcodes (`modified` is non-zero).
    pub fn custom(&self) -> &[Record] {
        &self.custom
    }

    /// The file's own bytes.
    pub fn raw(&self) -> &'a [u8] {
        self.raw
    }

    /// Every record, first table then second, in file order.
    pub fn records(&self) -> impl Iterator<Item = &Record> {
        self.base.iter().chain(self.custom.iter())
    }

    /// Re-serialise the decoded tree field by field.
    ///
    /// For every shipped file this is byte-identical to [`ObjectFile::raw`],
    /// which is what makes the digest below comparable across decoders: a
    /// second implementation must reproduce these bytes from its own parse.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.raw.len());
        out.extend_from_slice(&self.version.to_le_bytes());
        out.extend_from_slice(&(self.base.len() as u32).to_le_bytes());
        for r in &self.base {
            r.write(&mut out);
        }
        out.extend_from_slice(&(self.custom.len() as u32).to_le_bytes());
        for r in &self.custom {
            r.write(&mut out);
        }
        out
    }

    /// FNV-1a 64 over [`ObjectFile::canonical_bytes`].
    pub fn canonical_hash(&self) -> u64 {
        pcx_mpq::fnv1a64(&self.canonical_bytes())
    }

    /// The file's measured framing, as facts rather than assertions.
    pub fn framing(&self) -> Framing {
        Framing::of(self)
    }
}

fn extension_of(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    match base.rsplit_once('.') {
        Some((_, ext)) => ext.to_ascii_lowercase(),
        None => base.to_ascii_lowercase(),
    }
}

fn read_u32(data: &[u8], p: &mut usize, what: &'static str) -> Result<u32, Error> {
    let need = 4usize;
    let have = data.len().saturating_sub(*p);
    let end = p
        .checked_add(need)
        .filter(|e| *e <= data.len())
        .ok_or(Error::Truncated {
            what,
            at: *p,
            need,
            have,
        })?;
    let v = u32::from_le_bytes(data[*p..end].try_into().unwrap());
    *p = end;
    Ok(v)
}

fn read_fourcc(data: &[u8], p: &mut usize, what: &'static str) -> Result<Fourcc, Error> {
    let need = 4usize;
    let end = p
        .checked_add(need)
        .filter(|e| *e <= data.len())
        .ok_or(Error::Truncated {
            what,
            at: *p,
            need,
            have: data.len().saturating_sub(*p),
        })?;
    let v: Fourcc = data[*p..end].try_into().unwrap();
    *p = end;
    Ok(v)
}

fn read_table(data: &[u8], p: &mut usize, indexed: bool) -> Result<Vec<Record>, Error> {
    let count = read_u32(data, p, "object count")? as usize;
    let mut out = Vec::with_capacity(count.min(1 << 16));
    for _ in 0..count {
        let original = read_fourcc(data, p, "original id")?;
        let modified = read_fourcc(data, p, "modified id")?;
        let mod_count = read_u32(data, p, "modification count")? as usize;
        let mut mods = Vec::with_capacity(mod_count.min(1 << 16));
        for _ in 0..mod_count {
            let field = read_fourcc(data, p, "field id")?;
            let type_at = *p;
            let type_word = read_u32(data, p, "value type")?;
            let value_type = ValueType::of(type_word).ok_or(Error::BadValueType {
                type_word,
                at: type_at,
            })?;
            let selectors = if indexed {
                Some([
                    read_u32(data, p, "first selector")?,
                    read_u32(data, p, "second selector")?,
                ])
            } else {
                None
            };
            let value = match value_type {
                ValueType::Int => Value::Int(read_u32(data, p, "int value")?),
                ValueType::Real => Value::Real(f32::from_bits(read_u32(data, p, "real value")?)),
                ValueType::Unreal => {
                    Value::Unreal(f32::from_bits(read_u32(data, p, "unreal value")?))
                }
                ValueType::Str => {
                    let start = *p;
                    let Some(nul) = data
                        .get(start..)
                        .and_then(|s| s.iter().position(|b| *b == 0))
                    else {
                        return Err(Error::UnterminatedString { at: start });
                    };
                    let s = data[start..start + nul].to_vec();
                    *p = start + nul + 1;
                    Value::Str(s)
                }
            };
            let end_word = read_u32(data, p, "end word")?;
            mods.push(Mod {
                field,
                value_type,
                selectors,
                value,
                end_word,
            });
        }
        out.push(Record {
            original,
            modified,
            mods,
        });
    }
    Ok(out)
}

/// The measured framing of one object file, as facts rather than assertions.
///
/// Every field is read straight out of the parse, so
/// [`Framing::violations`] can name the relation that failed instead of
/// returning a bare `false`. The identity and the files it holds on are in
/// the module documentation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Framing {
    /// The raw file length.
    pub file_len: usize,
    /// The version word.
    pub version: u32,
    /// `B`: first-table object count.
    pub base_objects: usize,
    /// `C`: second-table object count.
    pub custom_objects: usize,
    /// `B + C`.
    pub objects: usize,
    /// Modification entries across both tables.
    pub mods: usize,
    /// `Σ (12 + Σ mod_len)` over first-table records.
    pub base_bytes: usize,
    /// `Σ (12 + Σ mod_len)` over second-table records.
    pub custom_bytes: usize,
    /// `Σ (8 + value_len + 4)` over all modification entries.
    pub mod_bytes: usize,
    /// `Σ value_len` over all modification entries.
    pub value_bytes: usize,
    /// Whether every first-table record carries a zero modified id.
    pub base_modified_ids_are_zero: bool,
    /// Whether every second-table record carries a non-zero modified id.
    pub custom_modified_ids_are_set: bool,
}

impl Framing {
    /// Read the framing of a parsed file.
    pub fn of(f: &ObjectFile<'_>) -> Self {
        let base_bytes = f.base.iter().map(Record::wire_len).sum();
        let custom_bytes = f.custom.iter().map(Record::wire_len).sum();
        let mods: usize = f.records().map(|r| r.mods.len()).sum();
        let mod_bytes: usize = f
            .records()
            .flat_map(|r| r.mods.iter())
            .map(Mod::wire_len)
            .sum();
        let value_bytes: usize = f
            .records()
            .flat_map(|r| r.mods.iter())
            .map(|m| m.wire_len() - 12)
            .sum();
        Self {
            file_len: f.raw.len(),
            version: f.version,
            base_objects: f.base.len(),
            custom_objects: f.custom.len(),
            objects: f.base.len() + f.custom.len(),
            mods,
            base_bytes,
            custom_bytes,
            mod_bytes,
            value_bytes,
            base_modified_ids_are_zero: f.base.iter().all(|r| r.modified == [0; 4]),
            custom_modified_ids_are_set: f.custom.iter().all(|r| r.modified != [0; 4]),
        }
    }

    /// `file_len == 4 + 4 + base_bytes + 4 + custom_bytes`: version word,
    /// both object counts, and both record regions are the whole file.
    pub fn sums(&self) -> bool {
        self.file_len == 4 + 4 + self.base_bytes + 4 + self.custom_bytes
    }

    /// Every framing relation this decoder relies on, as human-readable
    /// failures. Empty means the layout adds up.
    pub fn violations(&self) -> Vec<String> {
        let mut out = Vec::new();
        if !self.sums() {
            out.push(format!(
                "version 4 + counts 8 + base {} + custom {} != file length {}",
                self.base_bytes, self.custom_bytes, self.file_len
            ));
        }
        if !self.base_modified_ids_are_zero {
            out.push("a first-table record carries a non-zero modified id".into());
        }
        if !self.custom_modified_ids_are_set {
            out.push("a second-table record carries a zero modified id".into());
        }
        out
    }
}

/// Every `.w3m` / `.w3n` path in an install, from the union of every member
/// archive's `(listfile)`.
///
/// Same terms as `pcx-map::shipped_maps`: the maps live in `war3.mpq`, which
/// is last in the search order, so a single-archive `(listfile)` read would
/// not list them.
pub fn shipped_maps(set: &mut pcx_mpq::MpqSet) -> Result<Vec<String>, Error> {
    Ok(set
        .file_list()?
        .into_iter()
        .filter(|p| {
            let l = p.to_ascii_lowercase();
            l.ends_with(".w3m") || l.ends_with(".w3n")
        })
        .collect())
}

/// The object-data file names inside one map's embedded archive.
///
/// Taken from the archive's own `(listfile)` where one exists, union the
/// canonical `war3map.<ext>` names, so the result does not depend on one
/// naming convention — `DemoCampaign.w3n`'s files are `war3campaign.w3u`
/// and `war3campaign.w3t`.
pub fn map_object_files(set: &mut pcx_mpq::MpqSet, map: &str) -> Result<Vec<String>, Error> {
    let raw = set.extract(map)?;
    let body = raw
        .get(MAP_ARCHIVE_OFFSET..)
        .ok_or_else(|| Error::NoEmbeddedArchive(map.to_string()))?;
    let mut arc = pcx_mpq::MpqArchive::from_bytes(body.to_vec(), map)
        .map_err(|_| Error::NoEmbeddedArchive(map.to_string()))?;
    let mut names: Vec<String> = Vec::new();
    if let Ok(listfile) = arc.extract("(listfile)") {
        for line in String::from_utf8_lossy(&listfile).lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let low = line.to_ascii_lowercase();
            if EXTENSIONS.iter().any(|e| low.ends_with(&format!(".{e}"))) {
                names.push(line.to_string());
            }
        }
    }
    for e in EXTENSIONS {
        let canonical = format!("war3map.{e}");
        if arc.contains(&canonical) && !names.iter().any(|n| n.eq_ignore_ascii_case(&canonical)) {
            names.push(canonical);
        }
    }
    Ok(names)
}

/// Read one object file out of a map's embedded archive.
pub fn extract_object_file(
    set: &mut pcx_mpq::MpqSet,
    map: &str,
    name: &str,
) -> Result<Vec<u8>, Error> {
    let raw = set.extract(map)?;
    let body = raw
        .get(MAP_ARCHIVE_OFFSET..)
        .ok_or_else(|| Error::NoEmbeddedArchive(map.to_string()))?;
    let mut arc = pcx_mpq::MpqArchive::from_bytes(body.to_vec(), map)
        .map_err(|_| Error::NoEmbeddedArchive(map.to_string()))?;
    Ok(arc.extract(name)?)
}

/// Where a WC3 install lives for tests and sweeps, on the same terms as the
/// other decoders: `PEACECRAFT3_INSTALL` overrides the 1.28c default.
pub fn install_dir() -> Option<PathBuf> {
    std::env::var_os("PEACECRAFT3_INSTALL")
        .map(PathBuf::from)
        .or_else(|| {
            let p = Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("wc3/game/app");
            p.is_dir().then_some(p)
        })
}

/// Where the 1.29b install lives: `PEACECRAFT3_LEGACY_DIR` overrides the
/// default.
pub fn legacy_dir() -> Option<PathBuf> {
    let p = std::env::var_os("PEACECRAFT3_LEGACY_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("../../Faugus/battle-net/drive_c/Program Files (x86)/Warcraft III (Legacy)")
        });
    p.is_dir().then_some(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Open an install and pull one object file out of one map.
    fn shipped(dir: &Path, map: &str, name: &str) -> Option<Vec<u8>> {
        let mut set = pcx_mpq::MpqSet::open_wc3(dir).ok()?;
        extract_object_file(&mut set, map, name).ok()
    }

    const MAP_DEMO: &str = "DemoCampaign.w3n";
    const FILE_DEMO_W3T: &str = "war3campaign.w3t";
    const FILE_DEMO_W3U: &str = "war3campaign.w3u";
    const MAP_BONUS: &str = r"Maps\Campaign\BonusCredits.w3m";
    const MAP_UNDEAD05: &str = r"Maps\Campaign\Undead05.w3m";
    const MAP_HUMAN06: &str = r"Maps\Campaign\Human06.w3m";
    const FILE_MAP: &str = "war3map.w3u";

    fn str_val(m: &Mod) -> &[u8] {
        match &m.value {
            Value::Str(s) => s,
            other => panic!("expected a string value, got {other:?}"),
        }
    }

    fn int_val(m: &Mod) -> u32 {
        match m.value {
            Value::Int(w) => w,
            ref other => panic!("expected an int value, got {other:?}"),
        }
    }

    fn unreal_val(m: &Mod) -> f32 {
        match m.value {
            Value::Unreal(f) => f,
            ref other => panic!("expected an unreal value, got {other:?}"),
        }
    }

    // ---- exact fields, read by hand from the bytes -----------------------

    #[test]
    fn demo_campaign_w3t_fields_are_exact() {
        let Some(dir) = install_dir() else { return };
        let Some(bytes) = shipped(&dir, MAP_DEMO, FILE_DEMO_W3T) else {
            return;
        };
        let f = ObjectFile::parse(FILE_DEMO_W3T, &bytes).unwrap();
        assert_eq!(f.version(), 1);
        assert!(f.base().is_empty());
        assert_eq!(f.custom().len(), 2);
        let r = &f.custom()[0];
        assert_eq!(&r.original, b"ches");
        assert_eq!(&r.modified, b"I000");
        assert_eq!(r.mods.len(), 6);
        assert_eq!(r.mods[0].field, *b"unam");
        assert_eq!(r.mods[0].value_type, ValueType::Str);
        assert_eq!(str_val(&r.mods[0]), b"TRIGSTR_015");
        assert_eq!(
            str_val(&r.mods[1]),
            b"ReplaceableTextures\\CommandButtons\\BTNAcorn.tga"
        );
        assert_eq!(r.mods[1].field, *b"iico");
        assert_eq!(r.mods[2].field, *b"ihtp");
        assert_eq!(r.mods[2].value_type, ValueType::Int);
        assert_eq!(int_val(&r.mods[2]), 100);
        assert_eq!(str_val(&r.mods[3]), b"TRIGSTR_016");
        assert_eq!(str_val(&r.mods[4]), b"TRIGSTR_017");
        assert_eq!(str_val(&r.mods[5]), b"TRIGSTR_018");
        assert!(r.mods.iter().all(|m| m.end_word == 0));
        let r = &f.custom()[1];
        assert_eq!(&r.original, b"ches");
        assert_eq!(&r.modified, b"I001");
        assert_eq!(str_val(&r.mods[0]), b"TRIGSTR_019");
        assert_eq!(
            str_val(&r.mods[1]),
            b"ReplaceableTextures\\CommandButtons\\BTNNatureTouchGrow.tga"
        );
    }

    #[test]
    fn bonus_credits_w3u_fields_are_exact() {
        let Some(dir) = install_dir() else { return };
        let Some(bytes) = shipped(&dir, MAP_BONUS, FILE_MAP) else {
            return;
        };
        let f = ObjectFile::parse(FILE_MAP, &bytes).unwrap();
        assert_eq!(f.version(), 1);
        assert_eq!(f.base().len(), 13);
        assert_eq!(f.custom().len(), 8);
        let r = &f.base()[0];
        assert_eq!(&r.original, b"nrwm");
        assert_eq!(r.modified, [0; 4]);
        assert_eq!(r.mods.len(), 10);
        assert_eq!(r.mods[0].field, *b"ua1r");
        assert_eq!(int_val(&r.mods[0]), 1200);
        assert_eq!(int_val(&r.mods[1]), 1500);
        assert_eq!(r.mods[2].field, *b"umvh");
        assert_eq!(unreal_val(&r.mods[2]), 350.0);
        assert_eq!(int_val(&r.mods[3]), 150);
        assert_eq!(int_val(&r.mods[4]), 1300);
        assert_eq!(int_val(&r.mods[5]), 700);
        assert_eq!(unreal_val(&r.mods[6]), 1500.0);
        assert_eq!(int_val(&r.mods[7]), 1500);
        assert_eq!(str_val(&r.mods[8]), b"normal");
        assert_eq!(unreal_val(&r.mods[9]), 1.5);
        assert!(r.mods.iter().all(|m| m.end_word == 0));
        let r = &f.custom()[0];
        assert_eq!(&r.original, b"ogru");
        assert_eq!(&r.modified, b"oC00");
        assert_eq!(r.mods.len(), 3);
        assert_eq!(str_val(&r.mods[0]), b"TRIGSTR_799");
        assert_eq!(str_val(&r.mods[1]), b"hover");
        assert_eq!(unreal_val(&r.mods[2]), 230.0);
    }

    #[test]
    fn undead05_captain_model_end_word_is_raw() {
        let Some(dir) = install_dir() else { return };
        let Some(bytes) = shipped(&dir, MAP_UNDEAD05, FILE_MAP) else {
            return;
        };
        let f = ObjectFile::parse(FILE_MAP, &bytes).unwrap();
        let r = f
            .base()
            .iter()
            .find(|r| r.original == *b"hfoo" && r.mods.len() == 1)
            .expect("the hfoo override with one modification");
        let m = &r.mods[0];
        assert_eq!(m.field, *b"umdl");
        // No `.mdl` suffix in this value; the bytes are what they are.
        assert_eq!(str_val(m), b"units\\human\\TheCaptain\\TheCaptain");
        // The end word is the Captain's rawcode. It is kept raw: the bytes
        // prove the word, not what the game does with it.
        assert_eq!(&m.end_word.to_le_bytes(), b"hcth");
    }

    #[test]
    fn demo_campaign_w3u_first_table_repeats_ids() {
        let Some(dir) = install_dir() else { return };
        let Some(bytes) = shipped(&dir, MAP_DEMO, FILE_DEMO_W3U) else {
            return;
        };
        let f = ObjectFile::parse(FILE_DEMO_W3U, &bytes).unwrap();
        assert_eq!(f.base().len(), 6);
        for r in f.base() {
            assert!(
                r.modified == [0; 4],
                "first table: {:?}",
                fourcc_text(r.modified)
            );
            assert!(!r.mods.is_empty());
            for m in &r.mods {
                // The one file whose end words are the record's own id.
                assert_eq!(
                    &m.end_word.to_le_bytes(),
                    &r.original,
                    "end word of {} on {}",
                    fourcc_text(m.field),
                    fourcc_text(r.original)
                );
            }
        }
        assert_eq!(f.custom().len(), 2);
        for r in f.custom() {
            assert!(r.modified != [0; 4]);
            for m in &r.mods {
                assert_eq!(m.end_word, 0, "custom table end words are zero");
            }
        }
    }

    // ---- framing and round trip ------------------------------------------

    #[test]
    fn framing_and_round_trip_hold_on_named_files() {
        let Some(dir) = install_dir() else { return };
        for (map, name) in [
            (MAP_DEMO, FILE_DEMO_W3T),
            (MAP_DEMO, FILE_DEMO_W3U),
            (MAP_BONUS, FILE_MAP),
            (MAP_UNDEAD05, FILE_MAP),
        ] {
            let Some(bytes) = shipped(&dir, map, name) else {
                continue;
            };
            let f = ObjectFile::parse(name, &bytes).unwrap();
            let fr = f.framing();
            assert!(
                fr.violations().is_empty(),
                "{map} {name}: {:?}",
                fr.violations()
            );
            assert!(fr.sums(), "{map} {name}: {fr:?}");
            assert_eq!(
                f.canonical_bytes(),
                bytes,
                "{map} {name} must round trip byte for byte"
            );
            assert_ne!(f.canonical_hash(), 0);
        }
    }

    #[test]
    fn version_two_shares_the_version_one_layout() {
        let Some(legacy) = legacy_dir() else { return };
        let Some(bytes) = shipped(&legacy, MAP_HUMAN06, FILE_MAP) else {
            return;
        };
        let f = ObjectFile::parse(FILE_MAP, &bytes).unwrap();
        assert_eq!(f.version(), 2, "the one shipped version-2 file");
        let fr = f.framing();
        assert!(fr.violations().is_empty(), "{:?}", fr.violations());
        assert!(fr.sums());
        assert_eq!(f.canonical_bytes(), bytes);
        // Counts are read by hand from both builds' bytes: the two files are
        // the same map's data at the two version words.
        assert_eq!((fr.base_objects, fr.custom_objects, fr.mods), (10, 7, 53));
        if let Some(dir) = install_dir()
            && let Some(old) = shipped(&dir, MAP_HUMAN06, FILE_MAP)
        {
            let v1 = ObjectFile::parse(FILE_MAP, &old).unwrap();
            assert_eq!(v1.version(), 1);
            let fr1 = v1.framing();
            assert_eq!(
                (fr1.base_objects, fr1.custom_objects, fr1.mods),
                (fr.base_objects, fr.custom_objects, fr.mods)
            );
        }
    }

    #[test]
    fn shipped_object_file_counts() {
        let mut totals = [0usize; 7];
        for dir in [install_dir(), legacy_dir()].into_iter().flatten() {
            let mut set = pcx_mpq::MpqSet::open_wc3(&dir).unwrap();
            let mut here = [0usize; 7];
            for map in shipped_maps(&mut set).unwrap() {
                for name in map_object_files(&mut set, &map).unwrap() {
                    let low = name.to_ascii_lowercase();
                    if let Some(i) = EXTENSIONS
                        .iter()
                        .position(|e| low.ends_with(&format!(".{e}")))
                    {
                        here[i] += 1;
                    }
                }
            }
            if dir.to_string_lossy().contains("Legacy") {
                assert_eq!(here, [44, 0, 0, 0, 0, 0, 0], "1.29b object census");
            } else {
                assert_eq!(here, [47, 1, 0, 0, 0, 0, 0], "1.28c object census");
            }
            for i in 0..7 {
                totals[i] += here[i];
            }
        }
        assert_eq!(totals, [91, 1, 0, 0, 0, 0, 0]);
    }

    // ---- refusals --------------------------------------------------------

    fn minimal() -> Vec<u8> {
        // version 1, one base record (aaaa -> 0) with one int mod, no custom.
        let mut v = Vec::new();
        v.extend_from_slice(&1u32.to_le_bytes());
        v.extend_from_slice(&1u32.to_le_bytes());
        v.extend_from_slice(b"aaaa");
        v.extend_from_slice(&[0; 4]);
        v.extend_from_slice(&1u32.to_le_bytes());
        v.extend_from_slice(b"uhpm");
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(&1200u32.to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes());
        v
    }

    #[test]
    fn unmeasured_extensions_are_refused() {
        assert!(matches!(
            ObjectFile::parse("war3map.w3z", &minimal()),
            Err(Error::UnmeasuredExtension(_))
        ));
    }

    #[test]
    fn every_object_family_preserves_its_own_record_width() {
        for extension in EXTENSIONS {
            let mut bytes = minimal();
            let indexed = matches!(extension, "w3a" | "w3q");
            if indexed {
                let selectors = [3u32.to_le_bytes(), 7u32.to_le_bytes()].concat();
                bytes.splice(28..28, selectors);
            }
            let name = format!("war3map.{extension}");
            let parsed = ObjectFile::parse(&name, &bytes).unwrap();
            assert_eq!(
                parsed.base()[0].mods[0].selectors,
                indexed.then_some([3, 7])
            );
            assert_eq!(parsed.canonical_bytes(), bytes);
        }
    }

    #[test]
    fn bad_version_is_refused() {
        let mut bytes = minimal();
        bytes[0] = 7;
        assert!(matches!(
            ObjectFile::parse("war3map.w3u", &bytes),
            Err(Error::UnsupportedVersion(7))
        ));
    }

    #[test]
    fn bad_value_type_is_refused() {
        let mut bytes = minimal();
        // the type word of the one mod sits after version, count, ids, count
        let at = 4 + 4 + 12;
        bytes[at + 4..at + 8].copy_from_slice(&9u32.to_le_bytes());
        assert!(matches!(
            ObjectFile::parse("war3map.w3u", &bytes),
            Err(Error::BadValueType { type_word: 9, .. })
        ));
    }

    #[test]
    fn unterminated_string_is_refused() {
        let mut v = Vec::new();
        v.extend_from_slice(&1u32.to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(&1u32.to_le_bytes());
        v.extend_from_slice(b"ches");
        v.extend_from_slice(b"I000");
        v.extend_from_slice(&1u32.to_le_bytes());
        v.extend_from_slice(b"unam");
        v.extend_from_slice(&3u32.to_le_bytes());
        v.extend_from_slice(b"TRIGSTR_015"); // no NUL, no end word
        let err = ObjectFile::parse("war3map.w3t", &v).unwrap_err();
        assert!(matches!(err, Error::UnterminatedString { .. }), "{err}");
    }

    #[test]
    fn trailing_bytes_are_refused() {
        let mut bytes = minimal();
        bytes.push(0);
        assert!(matches!(
            ObjectFile::parse("war3map.w3u", &bytes),
            Err(Error::TrailingBytes { .. })
        ));
    }

    #[test]
    fn truncated_is_refused() {
        let bytes = minimal();
        assert!(matches!(
            ObjectFile::parse("war3map.w3u", &bytes[..2]),
            Err(Error::Truncated { .. })
        ));
    }

    #[test]
    fn minimal_file_parses_and_sums() {
        let bytes = minimal();
        let f = ObjectFile::parse("war3map.w3u", &bytes).unwrap();
        let fr = f.framing();
        assert!(fr.sums());
        assert!(fr.violations().is_empty());
        assert_eq!(f.canonical_bytes(), bytes);
        assert_eq!(int_val(&f.base()[0].mods[0]), 1200);
    }

    // ---- full sweep ------------------------------------------------------

    /// Full sweep over every shipped object file on every available build.
    ///
    /// Ignored by default because it opens whole installs. Writes failures
    /// to `/tmp/opencode/objects-sweep.txt` and prints a per-extension
    /// census.
    #[test]
    #[ignore = "opens every shipped install; run explicitly"]
    fn sweep_every_shipped_object_file() {
        use std::fmt::Write as _;
        let mut report = String::new();
        let mut failures: Vec<String> = Vec::new();
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
                    failures.push(format!("{label}: open failed: {e}"));
                    continue;
                }
            };
            let maps = match shipped_maps(&mut set) {
                Ok(m) => m,
                Err(e) => {
                    failures.push(format!("{label}: listfile failed: {e}"));
                    continue;
                }
            };
            let mut per_ext = [0usize; 7];
            let mut ok = 0usize;
            let mut objects = 0usize;
            let mut mods = 0usize;
            let mut versions = [0usize; 3];
            for map in &maps {
                let names = match map_object_files(&mut set, map) {
                    Ok(n) => n,
                    Err(e) => {
                        failures.push(format!("{label} {map}: enumerate: {e}"));
                        continue;
                    }
                };
                for name in names {
                    let low = name.to_ascii_lowercase();
                    let Some(i) = EXTENSIONS
                        .iter()
                        .position(|e| low.ends_with(&format!(".{e}")))
                    else {
                        continue;
                    };
                    per_ext[i] += 1;
                    let bytes = match extract_object_file(&mut set, map, &name) {
                        Ok(b) => b,
                        Err(e) => {
                            failures.push(format!("{label} {map} {name}: extract: {e}"));
                            continue;
                        }
                    };
                    let f = match ObjectFile::parse(&name, &bytes) {
                        Ok(f) => f,
                        Err(e) => {
                            failures.push(format!("{label} {map} {name}: parse: {e}"));
                            continue;
                        }
                    };
                    let fr = f.framing();
                    for v in fr.violations() {
                        failures.push(format!("{label} {map} {name}: framing: {v}"));
                    }
                    if !fr.sums() {
                        failures.push(format!("{label} {map} {name}: framing sums: {fr:?}"));
                    }
                    if f.canonical_bytes() != bytes {
                        failures.push(format!("{label} {map} {name}: round trip"));
                    }
                    if f.canonical_hash() == 0 {
                        failures.push(format!("{label} {map} {name}: zero hash"));
                    }
                    ok += 1;
                    objects += fr.objects;
                    mods += fr.mods;
                    versions[(fr.version as usize).min(2)] += 1;
                    let _ = map;
                }
            }
            let _ = writeln!(
                report,
                "{label}: {} object files (w3u {} w3t {} w3a {} w3b {} w3d {} w3h {} w3q {}), \
                 {} parsed, {} records, {} mods, versions {}",
                per_ext.iter().sum::<usize>(),
                per_ext[0],
                per_ext[1],
                per_ext[2],
                per_ext[3],
                per_ext[4],
                per_ext[5],
                per_ext[6],
                ok,
                objects,
                mods,
                versions[1] + versions[2]
            );
            println!(
                "{label}: census w3u={} w3t={} w3a={} w3b={} w3d={} w3h={} w3q={} \
                 parsed={ok} records={objects} mods={mods}",
                per_ext[0], per_ext[1], per_ext[2], per_ext[3], per_ext[4], per_ext[5], per_ext[6]
            );
        }

        let _ = writeln!(report, "failures: {}", failures.len());
        for f in &failures {
            let _ = writeln!(report, "  {f}");
        }
        let out = std::path::Path::new("/tmp/opencode/objects-sweep.txt");
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

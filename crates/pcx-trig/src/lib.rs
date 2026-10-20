//! `war3map.wts` / `war3map.wtg` / `war3map.wct` — the three scripting-side
//! map containers.
//!
//! Scope is deliberately one layer: the containers and their records. **No
//! trigger semantics are interpreted.** An ECA name says which editor
//! function is stored; what that function does, and the later GUI→JASS
//! desugaring, are separate stages and are not guessed here.
//!
//! # What actually ships
//!
//! Every `*.wts` / `*.wtg` / `*.wct` inside the embedded archives of the
//! shipped maps was enumerated on both builds, through each map archive's own
//! `(listfile)` union the canonical `war3map.*` names (so `DemoCampaign.w3n`'s
//! `war3campaign.wts` is counted):
//!
//! | build | maps | wts | wtg | wct | lacking |
//! |-------|------|-----|-----|-----|---------|
//! | 1.28c | 89   | 89  | 88  | 88  | `DemoCampaign.w3n` has a `war3campaign.wts` but no `wtg`/`wct` under either name |
//! | 1.29b | 44   | 44  | 44  | 44  | none |
//!
//! 133 WTS, 132 WTG, 132 WCT files: 9 602 string entries, 112 325 trigger
//! elements (ECA), 338 209 parameters, 9 131 trigger slots in the WCT files.
//! Every one of the 397 files is consumed byte-exactly by the walks below;
//! the arithmetic identities are in the per-format framing sections.
//!
//! # `war3map.wts` — table strings
//!
//! A WTS file is **not** a quoted format: the header is `STRING <n>` with a
//! space (the `STRING:<n>` in some notes is refuted by every byte), the body
//! is delimited by brace lines, and the body itself is raw — no escaping
//! layer exists. `|cAARRGGBB`/`|r` colour codes and `|n` newlines are
//! ordinary text in the body (69 files carry colour codes, 4 carry `|n`), and
//! the `^^` code tags the JASS lexer knows about appear in no shipped
//! WTS/WTG/WCT byte (they belong to `war3map.j` string literals, see
//! `pcx-script`).
//!
//! ```text
//! offset  size  field
//! 0x000   3     UTF-8 BOM (EF BB BF) — present in all 133 files, but the
//!               layout below does not depend on it; `Wts::bom` records it
//!         ...   entries until EOF; nothing follows the last entry
//!
//! entry
//! +0x00   7     "STRING "
//! +0x07   ...   id digits (ASCII decimal, optional '-'; raw digits kept)
//! last    2     CRLF
//!         ...   zero or more comment lines: "//" + text + CRLF
//!         3     "{\r\n"
//!         ...   body: raw bytes, may span lines, may be empty
//! last    7     "\r\n}\r\n\r\n"   ← the entry terminator
//! ```
//!
//! The body is read up to the *first* `\r\n}\r\n\r\n`; a body containing that
//! sequence cannot be represented by the format at all (the World Editor
//! would misread it too), and a file that does not then continue with another
//! `STRING` header or EOF is refused rather than re-framed. Measured on 9 602
//! entries: bodies are CRLF-only (no bare LF), no body contains `\r\n}`, 36
//! entries carry exactly one comment line and none carry two, and entry ids
//! are strictly increasing with no duplicates (the game tolerates stranger
//! ids — first definition wins — but no shipped byte exercises that, so a
//! non-decimal id is refused rather than atoi'd).
//!
//! # `war3map.wct` — custom text script
//!
//! ```text
//! offset  size  field
//! 0x000   4     unknown word (0 in all 132 files; kept raw, not interpreted)
//! 0x004   4     N  count of text blocks
//!         ...   N text blocks; EOF, nothing follows
//!
//! text block
//! +0x00   4     L  byte length
//! +0x04   L     raw bytes
//! ```
//!
//! The blocks are in `war3map.wtg` trigger order, one per trigger — an empty
//! block for a GUI trigger, the custom JASS text for a custom-text trigger.
//! The 14 non-empty texts are NUL-terminated JASS and the NUL is **inside**
//! `L` (all 14 end in exactly one NUL; the 9 117 empty blocks are `L == 0`
//! with no NUL). This is measured, not assumed: `WctBlock::text` hands out
//! the exact `L` bytes.
//!
//! # `war3map.wtg` — trigger graph
//!
//! ```text
//! offset  size  field
//! 0x000   4     magic "WTG!"
//! 0x004   4     version: 4 in all 132 files (anything else refused, see
//!               below)
//! 0x008   4     C  count of categories
//!         ...   C category records
//!         4     unknown word (1 in all 132 files; kept raw)
//!         4     V  count of variables
//!         ...   V variable records
//!         4     T  count of triggers
//!         ...   T trigger records; EOF, nothing follows
//!
//! category
//! +0x00   4     id (not an index: ids are non-contiguous and unordered)
//! +0x04   ...   name + NUL
//!
//! variable
//! +0x00   ...   name + NUL
//!         ...   type + NUL (25 distinct, every one a [TriggerTypes] key)
//!         4     unknown word (1 in all 6 320 variables; kept raw)
//!         4     is array: 0/1 (1 in 82 variables)
//!         4     is initialized: 0/1
//!         ...   initial value + NUL (empty when not initialized)
//!
//! trigger
//! +0x00   ...   name + NUL
//!         ...   description + NUL
//!         4     is enabled (0/1)
//!         4     is custom text (0/1; 1 in the 14 triggers whose JASS lives
//!               in the WCT)
//!         4     is initially off (0/1)
//!         4     run on map initialization (0 in all 9 131 triggers)
//!         4     category id (must name a category record)
//!         4     E  count of events/conditions/actions
//!         ...   E ECA records
//!
//! ECA
//! +0x00   4     kind: 0 event (5 925), 1 condition (8 224), 2 action
//!               (98 176)
//! +0x04   ...   name + NUL (the editor function; its parameter count comes
//!               from TriggerData.txt, never from this file)
//!         4     is enabled (0/1)
//!         ...   Parameter × the function's arity
//!
//! Parameter
//! +0x00   4     type: 0 preset (74 858), 1 variable (98 263),
//!               2 function (82 849), 3 string (82 239)
//! +0x04   ...   value + NUL
//!         4     has sub-record: 0/1 (1 in 82 849)
//!         ...   Sub record when 1
//!         4     type 2: the function trailer, 0 in all 82 849
//!         4     type 0/1/3: is array: 0/1 (1 in 1 431)
//!         ...   Parameter (the array index) when that is 1
//!
//! Sub
//! +0x00   4     kind: 1 wrapped condition (11 517), 2 wrapped action
//!               (20 340), 3 nested call (50 992)
//! +0x04   ...   name + NUL (the function whose parameters follow)
//!         4     flag (1 in all 82 849)
//!         ...   Parameter × that name's arity
//! ```
//!
//! The Sub record is wire-identical for the three kinds — the kind's meaning
//! is measured from what surrounds it, not from its bytes: kind 3 always has
//! `value == name` (the nested-call form), kind 1 always has an empty `value`
//! (the boolexpr/boolcall wrapper), kind 2 has `value == "DoNothing"` in
//! 20 295 cases and an empty `value` in 45 (the `code` wrapper). The kind is
//! decoded as measured; what a wrapped condition *means* is a later stage.
//!
//! # Framing
//!
//! Each format turns its layout into an arithmetic identity rather than a
//! count; each holds on every shipped file of its kind:
//!
//! ```text
//! WTS:  file_len == bom_len + Σ (7 + id_digits + 2 + Σ (2 + comment + 2)
//!                                 + 3 + body + 7)
//! WCT:  file_len == 8 + Σ (4 + text_len)            [= 8 + 4·N + Σ text_len]
//! WTG:  file_len == 12 + Σ category + 8 + Σ variable + 4 + Σ trigger
//!       trigger == name + desc + 20 + 4 + Σ eca
//!       eca     == 4 + name + 4 + Σ parameter       (parameter count from
//!                                                    TriggerData.txt)
//!       param   == 4 + value + 4 + [sub] + 4 + [index]
//! ```
//!
//! plus the cross-file identity `wct.blocks == wtg.triggers`, per-trigger
//! `is_custom != 0` exactly when the WCT text is non-empty, and every custom
//! trigger carries zero ECAs (`wct_wtg_violations`). See [`WtsFraming`],
//! [`WctFraming`], [`WtgFraming`].
//!
//! # What is refused, and why
//!
//! * **WTG version 7** (Reforged-era files: child ECAs, `is_comment` fields,
//!   array-size words). No shipped byte is version 7 — all 132 files are
//!   version 4 — and its record shapes differ, so [`Wtg::parse`] returns
//!   [`Error::UnsupportedWtgVersion`] instead of walking v4 over it.
//! * **Parameter types outside `{0,1,2,3}`.** The four measured kinds are
//!   decoded; `-1` (the "invalid" some notes mention) appears in no shipped
//!   byte and any other discriminant is [`Error::BadParamType`] — a plausible
//!   guess here silently executes the wrong action.
//! * **A function parameter that is also an array access**, and any
//!   non-zero function trailer. Array indexing is proven only for
//!   preset/variable/string parameters (1 431 index parameters); what a
//!   trailing word on a function parameter would mean is unmeasured
//!   ([`Error::UnsupportedFunctionArray`]).
//! * **A Sub flag other than 1.** Whether a flag of 0 suppresses the
//!   parameter list is unmeasured (all 82 849 Sub records carry 1), so
//!   [`Error::UnsupportedSubFlag`] beats a guessed split.
//! * **ECA/parameter/Sub discriminants outside the measured sets**
//!   ([`Error::BadEcaKind`], [`Error::BadParamType`], [`Error::BadSubKind`]).
//! * **Any function name that is not in the supplied [`TriggerArity`].**
//!   The parameter count of an ECA or Sub is not in the file; it comes from
//!   `UI\TriggerData.txt`. [`TriggerArity::from_trigger_data`] builds that
//!   table (4 groups, 1 045 names on 1.28c, 1 136 on 1.29b — 0 arity
//!   conflicts between them), and a name outside it is
//!   [`Error::UnknownFunction`] rather than a zero-arity assumption.
//! * **Non-decimal WTS ids, LF-only line endings, and `STRING`-header
//!   garbage** the game would atoi. Strict text, typed errors.
//!
//! [`wts`] decodes `war3map.wts` in its own module.
pub mod catalog;
pub mod lower;
pub mod wct;
pub mod wtg;
pub mod wts;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The WTG magic.
pub const WTG_MAGIC: [u8; 4] = *b"WTG!";
/// The only WTG version measured in shipped files.
pub const WTG_VERSION: u32 = 4;

/// The names to look for in a map archive, canonical spellings first.
pub const FILE_NAMES: [&str; 3] = ["war3map.wts", "war3map.wtg", "war3map.wct"];

/// The extensions in the family.
pub const EXTENSIONS: [&str; 3] = ["wts", "wtg", "wct"];

/// Byte offset of a map's embedded MPQ archive (`pcx-map` measured this on
/// all 133 shipped maps).
pub const MAP_ARCHIVE_OFFSET: usize = 0x200;

/// The UTF-8 BOM that starts every shipped WTS file.
pub const WTS_BOM: [u8; 3] = [0xef, 0xbb, 0xbf];

/// ECA kind 0: an event.
pub const ECA_EVENT: u32 = 0;
/// ECA kind 1: a condition.
pub const ECA_CONDITION: u32 = 1;
/// ECA kind 2: an action.
pub const ECA_ACTION: u32 = 2;

/// Parameter type 0: a preset value.
pub const PARAM_PRESET: u32 = 0;
/// Parameter type 1: a variable reference.
pub const PARAM_VARIABLE: u32 = 1;
/// Parameter type 2: a function call.
pub const PARAM_FUNCTION: u32 = 2;
/// Parameter type 3: a string literal.
pub const PARAM_STRING: u32 = 3;

/// Sub kind 1: the boolexpr/boolcall wrapper (an inlined condition).
pub const SUB_CONDITION: u32 = 1;
/// Sub kind 2: the `code` wrapper (an inlined action).
pub const SUB_ACTION: u32 = 2;
/// Sub kind 3: a nested call's parameters.
pub const SUB_CALL: u32 = 3;

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
    #[error("bytes at +{at:#x} are not NUL-terminated before the end of the file")]
    UnterminatedCString { at: usize },
    #[error("records end at +{end:#x} but the file has {len} bytes")]
    TrailingBytes { end: usize, len: usize },

    #[error(
        "WCT declares {count} blocks at +{at:#x}, but only {length_words} minimum length words fit"
    )]
    WctCountOverruns {
        at: usize,
        count: u32,
        length_words: usize,
    },
    #[error("WCT {region} reservation for {elements} elements failed: {source}")]
    WctAllocation {
        region: &'static str,
        elements: usize,
        source: std::collections::TryReserveError,
    },

    // ---- wts ----
    #[error("entry at +{at:#x} does not start with `STRING `")]
    BadWtsHeader { at: usize },
    #[error("entry id at +{at:#x} is not plain ASCII decimal")]
    BadWtsId { at: usize },
    #[error("entry at +{at:#x} has no closing `\\r\\n}}\\r\\n\\r\\n`")]
    UnterminatedWtsEntry { at: usize },

    // ---- wtg ----
    #[error("not a WTG file: magic at +0 is {got:?}, want {WTG_MAGIC:?}")]
    BadWtgMagic { got: [u8; 4] },
    #[error("unsupported WTG version {got}; only version {WTG_VERSION} is measured")]
    UnsupportedWtgVersion { got: u32 },
    #[error("ECA kind {got} at +{at:#x}; only 0/1/2 (event/condition/action) are measured")]
    BadEcaKind { got: u32, at: usize },
    #[error(
        "parameter type {got} at +{at:#x}; only 0/1/2/3 \
         (preset/variable/function/string) are measured"
    )]
    BadParamType { got: i32, at: usize },
    #[error("sub kind {got} at +{at:#x}; only 1/2/3 (condition/action/call) are measured")]
    BadSubKind { got: u32, at: usize },
    #[error("sub flag {got} at +{at:#x}; every measured sub record carries 1")]
    UnsupportedSubFlag { got: u32, at: usize },
    #[error(
        "function parameter at +{at:#x} is followed by an array word of {got}; \
         array indexing is measured only on preset/variable/string parameters"
    )]
    UnsupportedFunctionArray { got: u32, at: usize },
    #[error("parameter word at +{at:#x} is {got}; has-sub is measured as 0 or 1")]
    UnsupportedHasSub { got: u32, at: usize },

    // ---- arity ----
    #[error("function {name:?} is not in the TriggerData arity table")]
    UnknownFunction { name: String },
    #[error("function {name:?} has conflicting arities {first} and {second}")]
    ArityConflict {
        name: String,
        first: usize,
        second: usize,
    },
    #[error("TriggerData line {line} has no `=`: {text:?}")]
    BadTriggerDataLine { line: usize, text: String },
}

// ---------------------------------------------------------------------------
// TriggerData.txt arity table
// ---------------------------------------------------------------------------

/// How many parameters each editor function takes.
///
/// `war3map.wtg` never states this; the count lives in `UI\TriggerData.txt`
/// (`[TriggerEvents]`, `[TriggerConditions]`, `[TriggerActions]`,
/// `[TriggerCalls]`), so a WTG parse takes the table as input and refuses
/// names it does not cover.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TriggerArity {
    map: BTreeMap<Vec<u8>, usize>,
}

impl TriggerArity {
    /// An empty table.
    pub fn new() -> Self {
        Self::default()
    }

    /// Build the table from one `UI\TriggerData.txt`'s text.
    ///
    /// For events/conditions/actions the value is
    /// `version,argument types…`; for calls it is
    /// `version,flag,return type,argument types…`. The arity is the number
    /// of argument-type fields that are neither empty nor `nothing`
    /// (the first `trigger` argument of an event is not in the file at all,
    /// and a call's return type is not an argument). Keys starting with `_`
    /// are editor-internal metadata and are skipped.
    pub fn from_trigger_data(text: &str) -> Result<Self, Error> {
        let mut out = Self::new();
        let mut section = String::new();
        for (i, line) in text.lines().enumerate() {
            let line = line.trim_end_matches(['\r', '\n']);
            let t = line.trim();
            if t.is_empty() || t.starts_with("//") {
                continue;
            }
            if t.starts_with('[') && t.ends_with(']') {
                section = t[1..t.len() - 1].to_string();
                continue;
            }
            if t.starts_with('_') {
                continue;
            }
            let Some((key, value)) = t.split_once('=') else {
                return Err(Error::BadTriggerDataLine {
                    line: i + 1,
                    text: line.to_string(),
                });
            };
            let key = key.trim();
            let skip = match section.as_str() {
                "TriggerEvents" | "TriggerConditions" | "TriggerActions" => 1,
                "TriggerCalls" => 3,
                _ => continue,
            };
            let args = value
                .split(',')
                .skip(skip)
                .filter(|a| {
                    let a = a.trim();
                    !a.is_empty() && a != "nothing"
                })
                .count();
            out.insert(key.as_bytes(), args)?;
        }
        Ok(out)
    }

    /// Insert one arity, rejecting conflicts rather than overwriting.
    pub fn insert(&mut self, name: &[u8], arity: usize) -> Result<(), Error> {
        match self.map.get(name) {
            Some(&first) if first != arity => Err(Error::ArityConflict {
                name: String::from_utf8_lossy(name).into_owned(),
                first,
                second: arity,
            }),
            Some(_) => Ok(()),
            None => {
                self.map.insert(name.to_vec(), arity);
                Ok(())
            }
        }
    }

    /// The arity of one function, if the table covers it.
    pub fn get(&self, name: &[u8]) -> Option<usize> {
        self.map.get(name).copied()
    }

    /// Number of covered functions.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether the table is empty.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Every `(name, arity)` pair, sorted by name.
    pub fn iter(&self) -> impl Iterator<Item = (&[u8], usize)> {
        self.map.iter().map(|(k, v)| (k.as_slice(), *v))
    }
}

// ---------------------------------------------------------------------------
// war3map.wts
// ---------------------------------------------------------------------------

/// One `STRING <n>` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WtsEntry {
    /// The entry id, as the digits decode.
    pub id: i32,
    /// The exact id digits as written (no re-formatting on the way out).
    pub id_digits: Vec<u8>,
    /// Comment lines between the header and the `{` line: the text after
    /// `//`, without the line ending. Zero or one per entry in shipped files.
    pub comments: Vec<Vec<u8>>,
    /// The raw body bytes between `{\r\n` and `\r\n}\r\n\r\n`. May be empty;
    /// `|c…|r`, `|n` and friends stay exactly as written.
    pub body: Vec<u8>,
}

impl WtsEntry {
    /// Bytes this entry occupies in the file.
    pub fn wire_len(&self) -> usize {
        7 + self.id_digits.len()
            + 2
            + self.comments.iter().map(|c| 2 + c.len() + 2).sum::<usize>()
            + 3
            + self.body.len()
            + 7
    }

    fn write(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(b"STRING ");
        out.extend_from_slice(&self.id_digits);
        out.extend_from_slice(b"\r\n");
        for c in &self.comments {
            out.extend_from_slice(b"//");
            out.extend_from_slice(c);
            out.extend_from_slice(b"\r\n");
        }
        out.extend_from_slice(b"{\r\n");
        out.extend_from_slice(&self.body);
        out.extend_from_slice(b"\r\n}\r\n\r\n");
    }
}

/// A parsed table-strings file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wts<'a> {
    raw: &'a [u8],
    bom: bool,
    entries: Vec<WtsEntry>,
}

impl<'a> Wts<'a> {
    /// Parse one WTS file. The walk consumes the file exactly: a leftover
    /// byte is an error, not a trailer.
    pub fn parse(data: &'a [u8]) -> Result<Self, Error> {
        let bom = data.starts_with(&WTS_BOM);
        let mut p = if bom { 3 } else { 0 };
        let mut entries = Vec::new();
        while p < data.len() {
            let at = p;
            if !data[p..].starts_with(b"STRING ") {
                return Err(Error::BadWtsHeader { at });
            }
            p += 7;
            let id_at = p;
            let id_start = p;
            if data.get(p) == Some(&b'-') {
                p += 1;
            }
            let digits_start = p;
            while matches!(data.get(p), Some(b'0'..=b'9')) {
                p += 1;
            }
            if p == digits_start {
                return Err(Error::BadWtsId { at: id_at });
            }
            let id_digits = data[id_start..p].to_vec();
            let id_text =
                std::str::from_utf8(&id_digits).map_err(|_| Error::BadWtsId { at: id_at })?;
            let id: i32 = id_text.parse().map_err(|_| Error::BadWtsId { at: id_at })?;
            if !data[p..].starts_with(b"\r\n") {
                return Err(Error::BadWtsId { at: id_at });
            }
            p += 2;
            let mut comments = Vec::new();
            while data[p..].starts_with(b"//") {
                p += 2;
                let end = find_crlf(data, p).ok_or(Error::UnterminatedWtsEntry { at })?;
                comments.push(data[p..end].to_vec());
                p = end + 2;
            }
            if !data[p..].starts_with(b"{\r\n") {
                return Err(Error::UnterminatedWtsEntry { at });
            }
            p += 3;
            let body_start = p;
            let end = find_terminator(data, p).ok_or(Error::UnterminatedWtsEntry { at })?;
            let body = data[body_start..end].to_vec();
            p = end + 7;
            entries.push(WtsEntry {
                id,
                id_digits,
                comments,
                body,
            });
        }
        Ok(Self {
            raw: data,
            bom,
            entries,
        })
    }

    /// Whether the file started with a UTF-8 BOM (true in all 133 shipped
    /// files).
    pub fn bom(&self) -> bool {
        self.bom
    }

    /// The entries in file order.
    pub fn entries(&self) -> &[WtsEntry] {
        &self.entries
    }

    /// Look up one entry by id (first definition wins, like the game).
    pub fn get(&self, id: i32) -> Option<&WtsEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// The file's own bytes.
    pub fn raw(&self) -> &'a [u8] {
        self.raw
    }

    /// Re-serialise the decoded table field by field.
    ///
    /// For every shipped file this is byte-identical to [`Wts::raw`].
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.raw.len());
        if self.bom {
            out.extend_from_slice(&WTS_BOM);
        }
        for e in &self.entries {
            e.write(&mut out);
        }
        out
    }

    /// FNV-1a 64 over [`Wts::canonical_bytes`].
    pub fn canonical_hash(&self) -> u64 {
        pcx_mpq::fnv1a64(&self.canonical_bytes())
    }

    /// The file's measured framing, as facts rather than assertions.
    pub fn framing(&self) -> WtsFraming {
        WtsFraming::of(self)
    }
}

fn find_crlf(data: &[u8], from: usize) -> Option<usize> {
    data[from..]
        .windows(2)
        .position(|w| w == b"\r\n")
        .map(|i| from + i)
}

fn find_terminator(data: &[u8], from: usize) -> Option<usize> {
    data[from..]
        .windows(7)
        .position(|w| w == b"\r\n}\r\n\r\n")
        .map(|i| from + i)
}

/// The measured framing of one WTS file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WtsFraming {
    /// The raw file length.
    pub file_len: usize,
    /// 3 when the BOM is present, else 0.
    pub bom_len: usize,
    /// Number of `STRING` entries.
    pub entries: usize,
    /// `Σ entry.wire_len`.
    pub entry_bytes: usize,
    /// Comment lines across all entries.
    pub comments: usize,
    /// Whether entry ids are strictly increasing with no duplicates.
    pub ids_strictly_increasing: bool,
}

impl WtsFraming {
    /// Read the framing of a parsed file.
    pub fn of(w: &Wts<'_>) -> Self {
        let ids: Vec<i32> = w.entries.iter().map(|e| e.id).collect();
        Self {
            file_len: w.raw.len(),
            bom_len: if w.bom { 3 } else { 0 },
            entries: w.entries.len(),
            entry_bytes: w.entries.iter().map(WtsEntry::wire_len).sum(),
            comments: w.entries.iter().map(|e| e.comments.len()).sum(),
            ids_strictly_increasing: ids.windows(2).all(|w| w[0] < w[1]),
        }
    }

    /// `file_len == bom_len + Σ entry.wire_len`: the BOM and every entry,
    /// headers, comments, braces and terminators included, are the whole
    /// file.
    pub fn sums(&self) -> bool {
        self.file_len == self.bom_len + self.entry_bytes
    }

    /// Every framing relation this decoder relies on, as human-readable
    /// failures. Empty means the layout adds up.
    pub fn violations(&self) -> Vec<String> {
        let mut out = Vec::new();
        if !self.sums() {
            out.push(format!(
                "bom {} + entries {} != file length {}",
                self.bom_len, self.entry_bytes, self.file_len
            ));
        }
        if !self.ids_strictly_increasing {
            out.push("entry ids are not strictly increasing".into());
        }
        out
    }
}

// ---------------------------------------------------------------------------
// war3map.wct
// ---------------------------------------------------------------------------

/// One custom-text block: the exact `L` bytes of one trigger's custom JASS
/// (the trailing NUL of a non-empty text is inside those bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WctBlock {
    /// Raw block bytes, length-prefixed in the file.
    pub text: Vec<u8>,
}

impl WctBlock {
    /// Bytes this block occupies in the file (length word plus text).
    pub fn wire_len(&self) -> usize {
        4 + self.text.len()
    }

    fn write(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&(self.text.len() as u32).to_le_bytes());
        out.extend_from_slice(&self.text);
    }
}

/// A parsed custom-text script file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wct<'a> {
    raw: &'a [u8],
    unknown0: u32,
    blocks: Vec<WctBlock>,
}

impl<'a> Wct<'a> {
    /// Parse one WCT file. The walk consumes the file exactly.
    pub fn parse(data: &'a [u8]) -> Result<Self, Error> {
        let mut p = 0usize;
        let unknown0 = read_u32(data, &mut p, "unknown0")?;
        let stored_count = read_u32(data, &mut p, "block count")?;
        // Even empty blocks require a length word; reject impossible counts
        // before reserving any caller-controlled metadata.
        let length_words = (data.len() - p) / std::mem::size_of::<u32>();
        let count = usize::try_from(stored_count)
            .ok()
            .filter(|n| *n <= length_words)
            .ok_or(Error::WctCountOverruns {
                at: 4,
                count: stored_count,
                length_words,
            })?;
        let first = p;
        for _ in 0..count {
            read_wct_span(data, &mut p)?;
        }
        if p != data.len() {
            return Err(Error::TrailingBytes {
                end: p,
                len: data.len(),
            });
        }
        let mut blocks = Vec::new();
        blocks
            .try_reserve_exact(count)
            .map_err(|source| Error::WctAllocation {
                region: "block metadata",
                elements: count,
                source,
            })?;
        p = first;
        for _ in 0..count {
            let span = read_wct_span(data, &mut p)?;
            let mut text = Vec::new();
            text.try_reserve_exact(span.len())
                .map_err(|source| Error::WctAllocation {
                    region: "block text",
                    elements: span.len(),
                    source,
                })?;
            text.extend_from_slice(&data[span]);
            blocks.push(WctBlock { text });
        }
        Ok(Self {
            raw: data,
            unknown0,
            blocks,
        })
    }

    /// The leading word: 0 in all 132 shipped files. Kept raw; its meaning
    /// is unmeasured.
    pub fn unknown0(&self) -> u32 {
        self.unknown0
    }

    /// The text blocks, in `war3map.wtg` trigger order.
    pub fn blocks(&self) -> &[WctBlock] {
        &self.blocks
    }

    /// The file's own bytes.
    pub fn raw(&self) -> &'a [u8] {
        self.raw
    }

    /// Re-serialise the decoded file field by field.
    ///
    /// For every shipped file this is byte-identical to [`Wct::raw`].
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.raw.len());
        out.extend_from_slice(&self.unknown0.to_le_bytes());
        out.extend_from_slice(&(self.blocks.len() as u32).to_le_bytes());
        for b in &self.blocks {
            b.write(&mut out);
        }
        out
    }

    /// FNV-1a 64 over [`Wct::canonical_bytes`].
    pub fn canonical_hash(&self) -> u64 {
        pcx_mpq::fnv1a64(&self.canonical_bytes())
    }

    /// The file's measured framing, as facts rather than assertions.
    pub fn framing(&self) -> WctFraming {
        WctFraming::of(self)
    }
}

/// Borrow one length-delimited raw block span without allocating.
fn read_wct_span(data: &[u8], p: &mut usize) -> Result<std::ops::Range<usize>, Error> {
    let at = *p;
    let len = read_u32(data, p, "text length")? as usize;
    let start = *p;
    let have = data.len().saturating_sub(start);
    let end = start
        .checked_add(len)
        .filter(|e| *e <= data.len())
        .ok_or(Error::Truncated {
            what: "text",
            at,
            need: len,
            have,
        })?;
    *p = end;
    Ok(start..end)
}

/// The measured framing of one WCT file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WctFraming {
    /// The raw file length.
    pub file_len: usize,
    /// `N`: number of text blocks.
    pub blocks: usize,
    /// `Σ text_len`.
    pub text_bytes: usize,
    /// Blocks with non-empty text.
    pub nonempty: usize,
    /// Whether every non-empty text ends in exactly one NUL and no empty one
    /// carries bytes at all (measured on all 9 131 blocks).
    pub texts_are_nul_terminated: bool,
}

impl WctFraming {
    /// Read the framing of a parsed file.
    pub fn of(w: &Wct<'_>) -> Self {
        let texts_are_nul_terminated = w.blocks.iter().all(|b| match b.text.last() {
            None => true,
            Some(&last) => last == 0 && !b.text[..b.text.len() - 1].contains(&0),
        });
        Self {
            file_len: w.raw.len(),
            blocks: w.blocks.len(),
            text_bytes: w.blocks.iter().map(|b| b.text.len()).sum(),
            nonempty: w.blocks.iter().filter(|b| !b.text.is_empty()).count(),
            texts_are_nul_terminated,
        }
    }

    /// `file_len == 8 + Σ (4 + text_len)`: the two words and every length
    /// word and text are the whole file.
    pub fn sums(&self) -> bool {
        self.file_len == 8 + self.blocks * 4 + self.text_bytes
    }

    /// Every framing relation this decoder relies on. Empty means the layout
    /// adds up.
    pub fn violations(&self) -> Vec<String> {
        let mut out = Vec::new();
        if !self.sums() {
            out.push(format!(
                "8 + blocks {}×4 + texts {} != file length {}",
                self.blocks * 4,
                self.text_bytes,
                self.file_len
            ));
        }
        if !self.texts_are_nul_terminated {
            out.push("a text block is not exactly one NUL-terminated payload".into());
        }
        out
    }
}

// ---------------------------------------------------------------------------
// war3map.wtg
// ---------------------------------------------------------------------------

/// One trigger category record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Category {
    /// The category id a trigger's `category_id` names (not an index).
    pub id: u32,
    /// The category name.
    pub name: Vec<u8>,
}

impl Category {
    fn wire_len(&self) -> usize {
        4 + self.name.len() + 1
    }

    fn write(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.id.to_le_bytes());
        out.extend_from_slice(&self.name);
        out.push(0);
    }
}

/// One trigger variable definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variable {
    /// The variable name.
    pub name: Vec<u8>,
    /// The type name: a `[TriggerTypes]` key in all 6 320 shipped variables.
    pub type_name: Vec<u8>,
    /// The word after the type: 1 in every shipped variable. Kept raw.
    pub unknown: u32,
    /// Array flag: 0/1.
    pub is_array: u32,
    /// Initialization flag: 0/1.
    pub is_initialized: u32,
    /// The initial value, exactly as stored (empty when not initialized).
    pub initial_value: Vec<u8>,
}

impl Variable {
    fn wire_len(&self) -> usize {
        self.name.len() + 1 + self.type_name.len() + 1 + 12 + self.initial_value.len() + 1
    }

    fn write(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.name);
        out.push(0);
        out.extend_from_slice(&self.type_name);
        out.push(0);
        out.extend_from_slice(&self.unknown.to_le_bytes());
        out.extend_from_slice(&self.is_array.to_le_bytes());
        out.extend_from_slice(&self.is_initialized.to_le_bytes());
        out.extend_from_slice(&self.initial_value);
        out.push(0);
    }
}

/// The kind of one ECA: event, condition or action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EcaKind {
    /// Kind 0 (5 925 shipped).
    Event,
    /// Kind 1 (8 224 shipped).
    Condition,
    /// Kind 2 (98 176 shipped).
    Action,
}

impl EcaKind {
    fn of(w: u32) -> Option<Self> {
        match w {
            ECA_EVENT => Some(Self::Event),
            ECA_CONDITION => Some(Self::Condition),
            ECA_ACTION => Some(Self::Action),
            _ => None,
        }
    }

    /// The kind word as it appears in the file.
    pub fn word(self) -> u32 {
        match self {
            Self::Event => ECA_EVENT,
            Self::Condition => ECA_CONDITION,
            Self::Action => ECA_ACTION,
        }
    }
}

/// One event, condition or action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Eca {
    /// Event, condition or action.
    pub kind: EcaKind,
    /// The editor function name. Its parameter count is not in the file.
    pub name: Vec<u8>,
    /// Enabled flag: 0/1.
    pub is_enabled: u32,
    /// Exactly `arity(name)` parameters, in file order.
    pub params: Vec<Param>,
}

impl Eca {
    /// Bytes this ECA occupies in the file.
    pub fn wire_len(&self) -> usize {
        4 + self.name.len() + 1 + 4 + self.params.iter().map(Param::wire_len).sum::<usize>()
    }

    fn write(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.kind.word().to_le_bytes());
        out.extend_from_slice(&self.name);
        out.push(0);
        out.extend_from_slice(&self.is_enabled.to_le_bytes());
        for p in &self.params {
            p.write(out);
        }
    }
}

/// The type of one parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamKind {
    /// Type 0: a preset value (74 858 shipped).
    Preset,
    /// Type 1: a variable reference (98 263 shipped).
    Variable,
    /// Type 2: a function call (82 849 shipped).
    Function,
    /// Type 3: a string literal (82 239 shipped).
    Str,
}

impl ParamKind {
    fn of(w: i32) -> Option<Self> {
        match w {
            x if x == PARAM_PRESET as i32 => Some(Self::Preset),
            x if x == PARAM_VARIABLE as i32 => Some(Self::Variable),
            x if x == PARAM_FUNCTION as i32 => Some(Self::Function),
            x if x == PARAM_STRING as i32 => Some(Self::Str),
            _ => None,
        }
    }

    /// The type word as it appears in the file.
    pub fn word(self) -> i32 {
        match self {
            Self::Preset => PARAM_PRESET as i32,
            Self::Variable => PARAM_VARIABLE as i32,
            Self::Function => PARAM_FUNCTION as i32,
            Self::Str => PARAM_STRING as i32,
        }
    }
}

/// The kind of one sub record.
///
/// The three are wire-identical; the names are the measured reading (see the
/// module documentation for the value/name correlations).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubKind {
    /// Kind 1: a wrapped condition (boolexpr/boolcall parameter).
    WrappedCondition,
    /// Kind 2: a wrapped action (`code` parameter).
    WrappedAction,
    /// Kind 3: a nested call's parameters.
    NestedCall,
}

impl SubKind {
    fn of(w: u32) -> Option<Self> {
        match w {
            SUB_CONDITION => Some(Self::WrappedCondition),
            SUB_ACTION => Some(Self::WrappedAction),
            SUB_CALL => Some(Self::NestedCall),
            _ => None,
        }
    }

    /// The kind word as it appears in the file.
    pub fn word(self) -> u32 {
        match self {
            Self::WrappedCondition => SUB_CONDITION,
            Self::WrappedAction => SUB_ACTION,
            Self::NestedCall => SUB_CALL,
        }
    }
}

/// The sub record of a function parameter: an inlined ECA (wrapper) or a
/// nested call's own parameter list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sub {
    /// Which of the three measured kinds.
    pub kind: SubKind,
    /// The function whose parameters follow; this name, not the outer
    /// value, is the arity key (measured on 82 849 records).
    pub name: Vec<u8>,
    /// The flag word: 1 in all 82 849 shipped sub records.
    pub flag: u32,
    /// Exactly `arity(name)` parameters.
    pub params: Vec<Param>,
}

impl Sub {
    /// Bytes this sub record occupies in the file.
    pub fn wire_len(&self) -> usize {
        4 + self.name.len() + 1 + 4 + self.params.iter().map(Param::wire_len).sum::<usize>()
    }

    fn write(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.kind.word().to_le_bytes());
        out.extend_from_slice(&self.name);
        out.push(0);
        out.extend_from_slice(&self.flag.to_le_bytes());
        for p in &self.params {
            p.write(out);
        }
    }
}

/// One parameter of an ECA or sub record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    /// Preset, variable, function or string.
    pub kind: ParamKind,
    /// The value, exactly as stored.
    pub value: Vec<u8>,
    /// The sub record, present when the has-sub word is 1 (always a
    /// function parameter in shipped files).
    pub sub: Option<Sub>,
    /// The array index parameter: measured only on preset/variable/string
    /// parameters (1 431 shipped).
    pub array_index: Option<Box<Param>>,
}

impl Param {
    /// Bytes this parameter occupies in the file.
    pub fn wire_len(&self) -> usize {
        4 + self.value.len()
            + 1
            + 4
            + self.sub.as_ref().map(Sub::wire_len).unwrap_or(0)
            + 4
            + self.array_index.as_ref().map(|p| p.wire_len()).unwrap_or(0)
    }

    fn write(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.kind.word().to_le_bytes());
        out.extend_from_slice(&self.value);
        out.push(0);
        out.extend_from_slice(&(self.sub.is_some() as u32).to_le_bytes());
        if let Some(s) = &self.sub {
            s.write(out);
        }
        match &self.array_index {
            Some(idx) if self.kind == ParamKind::Function => {
                // refused at parse time; unreachable for a decoded tree
                let _ = idx;
                out.extend_from_slice(&0u32.to_le_bytes());
            }
            Some(idx) => {
                out.extend_from_slice(&1u32.to_le_bytes());
                idx.write(out);
            }
            None => out.extend_from_slice(&0u32.to_le_bytes()),
        }
    }
}

/// One trigger definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trigger {
    /// The trigger name.
    pub name: Vec<u8>,
    /// The trigger description.
    pub description: Vec<u8>,
    /// Enabled flag: 0/1.
    pub is_enabled: u32,
    /// Custom-text flag: 0/1. When non-zero the JASS lives in the WCT block
    /// of the same index and `ecas` is empty.
    pub is_custom: u32,
    /// Initially-off flag: 0/1.
    pub is_initially_off: u32,
    /// Run-on-initialization flag: 0 in all 9 131 shipped triggers.
    pub run_on_init: u32,
    /// Which category record this trigger belongs to.
    pub category_id: u32,
    /// The events, conditions and actions in file order.
    pub ecas: Vec<Eca>,
}

impl Trigger {
    /// Bytes this trigger occupies in the file.
    pub fn wire_len(&self) -> usize {
        self.name.len()
            + 1
            + self.description.len()
            + 1
            + 20
            + 4
            + self.ecas.iter().map(Eca::wire_len).sum::<usize>()
    }

    fn write(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.name);
        out.push(0);
        out.extend_from_slice(&self.description);
        out.push(0);
        out.extend_from_slice(&self.is_enabled.to_le_bytes());
        out.extend_from_slice(&self.is_custom.to_le_bytes());
        out.extend_from_slice(&self.is_initially_off.to_le_bytes());
        out.extend_from_slice(&self.run_on_init.to_le_bytes());
        out.extend_from_slice(&self.category_id.to_le_bytes());
        out.extend_from_slice(&(self.ecas.len() as u32).to_le_bytes());
        for e in &self.ecas {
            e.write(out);
        }
    }
}

/// A parsed trigger-graph file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wtg<'a> {
    raw: &'a [u8],
    version: u32,
    categories: Vec<Category>,
    unknown_root: u32,
    variables: Vec<Variable>,
    triggers: Vec<Trigger>,
}

impl<'a> Wtg<'a> {
    /// Parse one WTG file against a [`TriggerArity`] table.
    ///
    /// The walk consumes the file exactly: a leftover byte is an error, not
    /// a trailer. Every function name the file uses must be in `arity` —
    /// the parameter count lives in `TriggerData.txt`, never in the file.
    pub fn parse(data: &'a [u8], arity: &TriggerArity) -> Result<Self, Error> {
        let mut p = 0usize;
        let got: [u8; 4] =
            data.get(p..p + 4)
                .and_then(|s| s.try_into().ok())
                .ok_or(Error::Truncated {
                    what: "magic",
                    at: 0,
                    need: 4,
                    have: data.len(),
                })?;
        if got != WTG_MAGIC {
            return Err(Error::BadWtgMagic { got });
        }
        p += 4;
        let version = read_u32(data, &mut p, "version")?;
        if version != WTG_VERSION {
            return Err(Error::UnsupportedWtgVersion { got: version });
        }
        let cat_count = read_u32(data, &mut p, "category count")? as usize;
        let mut categories = Vec::with_capacity(cat_count.min(1 << 16));
        for _ in 0..cat_count {
            let id = read_u32(data, &mut p, "category id")?;
            let name = read_cstring(data, &mut p, "category name")?;
            categories.push(Category { id, name });
        }
        let unknown_root = read_u32(data, &mut p, "unknown root word")?;
        let var_count = read_u32(data, &mut p, "variable count")? as usize;
        let mut variables = Vec::with_capacity(var_count.min(1 << 16));
        for _ in 0..var_count {
            let name = read_cstring(data, &mut p, "variable name")?;
            let type_name = read_cstring(data, &mut p, "variable type")?;
            let unknown = read_u32(data, &mut p, "variable unknown word")?;
            let is_array = read_u32(data, &mut p, "variable is-array")?;
            let is_initialized = read_u32(data, &mut p, "variable is-initialized")?;
            let initial_value = read_cstring(data, &mut p, "variable initial value")?;
            variables.push(Variable {
                name,
                type_name,
                unknown,
                is_array,
                is_initialized,
                initial_value,
            });
        }
        let trig_count = read_u32(data, &mut p, "trigger count")? as usize;
        let mut triggers = Vec::with_capacity(trig_count.min(1 << 16));
        for _ in 0..trig_count {
            let name = read_cstring(data, &mut p, "trigger name")?;
            let description = read_cstring(data, &mut p, "trigger description")?;
            let is_enabled = read_u32(data, &mut p, "trigger is-enabled")?;
            let is_custom = read_u32(data, &mut p, "trigger is-custom")?;
            let is_initially_off = read_u32(data, &mut p, "trigger is-initially-off")?;
            let run_on_init = read_u32(data, &mut p, "trigger run-on-init")?;
            let category_id = read_u32(data, &mut p, "trigger category id")?;
            let eca_count = read_u32(data, &mut p, "ECA count")? as usize;
            let mut ecas = Vec::with_capacity(eca_count.min(1 << 16));
            for _ in 0..eca_count {
                ecas.push(read_eca(data, &mut p, arity)?);
            }
            triggers.push(Trigger {
                name,
                description,
                is_enabled,
                is_custom,
                is_initially_off,
                run_on_init,
                category_id,
                ecas,
            });
        }
        if p != data.len() {
            return Err(Error::TrailingBytes {
                end: p,
                len: data.len(),
            });
        }
        Ok(Self {
            raw: data,
            version,
            categories,
            unknown_root,
            variables,
            triggers,
        })
    }

    /// The version word: [`WTG_VERSION`] in every shipped file.
    pub fn version(&self) -> u32 {
        self.version
    }

    /// The category records, in file order.
    pub fn categories(&self) -> &[Category] {
        &self.categories
    }

    /// The word after the categories: 1 in all 132 shipped files. Kept raw.
    pub fn unknown_root(&self) -> u32 {
        self.unknown_root
    }

    /// The variable definitions, in file order.
    pub fn variables(&self) -> &[Variable] {
        &self.variables
    }

    /// The triggers, in file order (the same order the WCT blocks use).
    pub fn triggers(&self) -> &[Trigger] {
        &self.triggers
    }

    /// Find one category record by id.
    pub fn category(&self, id: u32) -> Option<&Category> {
        self.categories.iter().find(|c| c.id == id)
    }

    /// The file's own bytes.
    pub fn raw(&self) -> &'a [u8] {
        self.raw
    }

    /// Re-serialise the decoded tree field by field.
    ///
    /// For every shipped file this is byte-identical to [`Wtg::raw`]; that
    /// is what makes [`Wtg::canonical_hash`] comparable across decoders.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.raw.len());
        out.extend_from_slice(&WTG_MAGIC);
        out.extend_from_slice(&self.version.to_le_bytes());
        out.extend_from_slice(&(self.categories.len() as u32).to_le_bytes());
        for c in &self.categories {
            c.write(&mut out);
        }
        out.extend_from_slice(&self.unknown_root.to_le_bytes());
        out.extend_from_slice(&(self.variables.len() as u32).to_le_bytes());
        for v in &self.variables {
            v.write(&mut out);
        }
        out.extend_from_slice(&(self.triggers.len() as u32).to_le_bytes());
        for t in &self.triggers {
            t.write(&mut out);
        }
        out
    }

    /// FNV-1a 64 over [`Wtg::canonical_bytes`].
    pub fn canonical_hash(&self) -> u64 {
        pcx_mpq::fnv1a64(&self.canonical_bytes())
    }

    /// The file's measured framing, as facts rather than assertions.
    pub fn framing(&self) -> WtgFraming {
        WtgFraming::of(self)
    }
}

fn read_eca(data: &[u8], p: &mut usize, arity: &TriggerArity) -> Result<Eca, Error> {
    let at = *p;
    let kind_word = read_u32(data, p, "ECA kind")?;
    let kind = EcaKind::of(kind_word).ok_or(Error::BadEcaKind { got: kind_word, at })?;
    let name = read_cstring(data, p, "ECA name")?;
    let is_enabled = read_u32(data, p, "ECA is-enabled")?;
    let n = arity_of(arity, &name)?;
    let mut params = Vec::with_capacity(n.min(1 << 12));
    for _ in 0..n {
        params.push(read_param(data, p, arity)?);
    }
    Ok(Eca {
        kind,
        name,
        is_enabled,
        params,
    })
}

fn arity_of(arity: &TriggerArity, name: &[u8]) -> Result<usize, Error> {
    arity.get(name).ok_or_else(|| Error::UnknownFunction {
        name: String::from_utf8_lossy(name).into_owned(),
    })
}

fn read_param(data: &[u8], p: &mut usize, arity: &TriggerArity) -> Result<Param, Error> {
    let at = *p;
    let type_word = read_u32(data, p, "parameter type")? as i32;
    let kind = ParamKind::of(type_word).ok_or(Error::BadParamType { got: type_word, at })?;
    let value = read_cstring(data, p, "parameter value")?;
    let has_sub_at = *p;
    let has_sub = read_u32(data, p, "has-sub")?;
    let sub = match has_sub {
        0 => None,
        1 => Some(read_sub(data, p, arity)?),
        got => {
            return Err(Error::UnsupportedHasSub {
                got,
                at: has_sub_at,
            });
        }
    };
    let trailer_at = *p;
    let trailer = read_u32(data, p, "parameter trailer")?;
    let array_index = if kind == ParamKind::Function {
        if trailer != 0 {
            return Err(Error::UnsupportedFunctionArray {
                got: trailer,
                at: trailer_at,
            });
        }
        None
    } else if trailer == 1 {
        Some(Box::new(read_param(data, p, arity)?))
    } else if trailer == 0 {
        None
    } else {
        return Err(Error::UnsupportedHasSub {
            got: trailer,
            at: trailer_at,
        });
    };
    Ok(Param {
        kind,
        value,
        sub,
        array_index,
    })
}

fn read_sub(data: &[u8], p: &mut usize, arity: &TriggerArity) -> Result<Sub, Error> {
    let at = *p;
    let kind_word = read_u32(data, p, "sub kind")?;
    let kind = SubKind::of(kind_word).ok_or(Error::BadSubKind { got: kind_word, at })?;
    let name = read_cstring(data, p, "sub name")?;
    let flag_at = *p;
    let flag = read_u32(data, p, "sub flag")?;
    if flag != 1 {
        return Err(Error::UnsupportedSubFlag {
            got: flag,
            at: flag_at,
        });
    }
    let n = arity_of(arity, &name)?;
    let mut params = Vec::with_capacity(n.min(1 << 12));
    for _ in 0..n {
        params.push(read_param(data, p, arity)?);
    }
    Ok(Sub {
        kind,
        name,
        flag,
        params,
    })
}

/// The measured framing of one WTG file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WtgFraming {
    /// The raw file length.
    pub file_len: usize,
    /// Number of category records.
    pub categories: usize,
    /// Number of variable records.
    pub variables: usize,
    /// Number of triggers.
    pub triggers: usize,
    /// ECAs across all triggers.
    pub ecas: usize,
    /// Parameters across all ECAs and sub records.
    pub params: usize,
    /// Sub records across all parameters.
    pub subs: usize,
    /// `Σ category.wire_len`.
    pub category_bytes: usize,
    /// `Σ variable.wire_len`.
    pub variable_bytes: usize,
    /// `Σ trigger.wire_len`.
    pub trigger_bytes: usize,
    /// Whether every trigger's category id names a category record.
    pub trigger_category_ids_resolve: bool,
}

impl WtgFraming {
    /// Read the framing of a parsed file.
    pub fn of(w: &Wtg<'_>) -> Self {
        let mut ecas = 0usize;
        let mut params = 0usize;
        let mut subs = 0usize;
        fn walk(ps: &[Param], params: &mut usize, subs: &mut usize) {
            for p in ps {
                *params += 1;
                if let Some(s) = &p.sub {
                    *subs += 1;
                    walk(&s.params, params, subs);
                }
                if let Some(i) = &p.array_index {
                    walk(std::slice::from_ref(i.as_ref()), params, subs);
                }
            }
        }
        for t in &w.triggers {
            ecas += t.ecas.len();
            for e in &t.ecas {
                walk(&e.params, &mut params, &mut subs);
            }
        }
        Self {
            file_len: w.raw.len(),
            categories: w.categories.len(),
            variables: w.variables.len(),
            triggers: w.triggers.len(),
            ecas,
            params,
            subs,
            category_bytes: w.categories.iter().map(Category::wire_len).sum(),
            variable_bytes: w.variables.iter().map(Variable::wire_len).sum(),
            trigger_bytes: w.triggers.iter().map(Trigger::wire_len).sum(),
            trigger_category_ids_resolve: w
                .triggers
                .iter()
                .all(|t| w.categories.iter().any(|c| c.id == t.category_id)),
        }
    }

    /// `file_len == 12 + Σ category + 8 + Σ variable + 4 + Σ trigger`:
    /// magic, version, the category count and records, the unknown root
    /// word, the variable count and records, the trigger count and records
    /// are the whole file.
    pub fn sums(&self) -> bool {
        self.file_len == 12 + self.category_bytes + 8 + self.variable_bytes + 4 + self.trigger_bytes
    }

    /// Every framing relation this decoder relies on. Empty means the layout
    /// adds up.
    pub fn violations(&self) -> Vec<String> {
        let mut out = Vec::new();
        if !self.sums() {
            out.push(format!(
                "12 + categories {} + 8 + variables {} + 4 + triggers {} != file length {}",
                self.category_bytes, self.variable_bytes, self.trigger_bytes, self.file_len
            ));
        }
        if !self.trigger_category_ids_resolve {
            out.push("a trigger names a category id that has no category record".into());
        }
        out
    }
}

fn read_u32(data: &[u8], p: &mut usize, what: &'static str) -> Result<u32, Error> {
    let end = p
        .checked_add(4)
        .filter(|e| *e <= data.len())
        .ok_or(Error::Truncated {
            what,
            at: *p,
            need: 4,
            have: data.len().saturating_sub(*p),
        })?;
    let v = u32::from_le_bytes(data[*p..end].try_into().unwrap());
    *p = end;
    Ok(v)
}

fn read_cstring(data: &[u8], p: &mut usize, what: &'static str) -> Result<Vec<u8>, Error> {
    let at = *p;
    let Some(nul) = data.get(at..).and_then(|s| s.iter().position(|b| *b == 0)) else {
        return Err(Error::UnterminatedCString { at });
    };
    let _ = what;
    let s = data[at..at + nul].to_vec();
    *p = at + nul + 1;
    Ok(s)
}

// ---------------------------------------------------------------------------
// cross-file relations
// ---------------------------------------------------------------------------

/// The measured relations between one WCT file and its map's WTG file.
///
/// Every shipped (WCT, WTG) pair satisfies all three:
///
/// * the WCT has exactly one text block per trigger, in trigger order;
/// * `is_custom != 0` exactly when the block's text is non-empty;
/// * a custom-text trigger carries zero ECAs (its JASS is the WCT text).
pub fn wct_wtg_violations(wct: &Wct<'_>, wtg: &Wtg<'_>) -> Vec<String> {
    let mut out = Vec::new();
    if wct.blocks().len() != wtg.triggers().len() {
        out.push(format!(
            "WCT has {} text blocks but the WTG has {} triggers",
            wct.blocks().len(),
            wtg.triggers().len()
        ));
        return out;
    }
    for (i, (b, t)) in wct.blocks().iter().zip(wtg.triggers()).enumerate() {
        let custom = t.is_custom != 0;
        let nonempty = !b.text.is_empty();
        if custom != nonempty {
            out.push(format!(
                "trigger {i} {:?}: is_custom {} but the WCT text is {}",
                String::from_utf8_lossy(&t.name),
                t.is_custom,
                if nonempty { "non-empty" } else { "empty" }
            ));
        }
        if custom && !t.ecas.is_empty() {
            out.push(format!(
                "trigger {i} {:?} is custom text but carries {} ECAs",
                String::from_utf8_lossy(&t.name),
                t.ecas.len()
            ));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// install helpers
// ---------------------------------------------------------------------------

/// Every `.w3m` / `.w3n` path in an install, from the union of every member
/// archive's `(listfile)`.
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

/// The scripting-side file names inside one map's embedded archive.
///
/// Taken from the archive's own `(listfile)` where one exists, union the
/// canonical `war3map.*` names, so `DemoCampaign.w3n`'s `war3campaign.wts`
/// is found and an absent file is an observed fact rather than a naming
/// accident.
pub fn map_trig_files(set: &mut pcx_mpq::MpqSet, map: &str) -> Result<Vec<String>, Error> {
    let raw = set.extract(map)?;
    let body = raw.get(MAP_ARCHIVE_OFFSET..).ok_or(Error::TrailingBytes {
        end: 0,
        len: raw.len(),
    })?;
    let mut arc =
        pcx_mpq::MpqArchive::from_bytes(body.to_vec(), map).map_err(|_| Error::TrailingBytes {
            end: 0,
            len: raw.len(),
        })?;
    let mut names: Vec<String> = Vec::new();
    if let Ok(listfile) = arc.extract("(listfile)") {
        for line in String::from_utf8_lossy(&listfile).lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let low = line.to_ascii_lowercase();
            if EXTENSIONS.iter().any(|e| low.ends_with(&format!(".{e}")))
                && !names.iter().any(|n| n.eq_ignore_ascii_case(line))
            {
                names.push(line.to_string());
            }
        }
    }
    for n in FILE_NAMES {
        if arc.contains(n) && !names.iter().any(|x| x.eq_ignore_ascii_case(n)) {
            names.push(n.to_string());
        }
    }
    Ok(names)
}

/// Read one scripting-side file out of a map's embedded archive.
pub fn extract_trig_file(
    set: &mut pcx_mpq::MpqSet,
    map: &str,
    name: &str,
) -> Result<Vec<u8>, Error> {
    let raw = set.extract(map)?;
    let body = raw.get(MAP_ARCHIVE_OFFSET..).ok_or(Error::TrailingBytes {
        end: 0,
        len: raw.len(),
    })?;
    let mut arc =
        pcx_mpq::MpqArchive::from_bytes(body.to_vec(), map).map_err(|_| Error::TrailingBytes {
            end: 0,
            len: raw.len(),
        })?;
    Ok(arc.extract(name)?)
}

/// Read `UI\TriggerData.txt` out of an install and build the arity table.
pub fn install_trigger_arity(dir: &Path) -> Result<TriggerArity, Error> {
    let mut set = pcx_mpq::MpqSet::open_wc3(dir)?;
    let bytes = set.extract("UI\\TriggerData.txt")?;
    let text = String::from_utf8_lossy(&bytes);
    TriggerArity::from_trigger_data(&text)
}

/// The 1.28c install: `PEACECRAFT3_INSTALL` overrides `wc3/game/app`.
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

/// The 1.29b install: `PEACECRAFT3_LEGACY_DIR` overrides the default.
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

/// The two installs to sweep: `PEACECRAFT3_LEGACY_DIR` set selects the
/// 1.29b build at that path and unset leaves the 1.28c default in play.
/// Both are returned when both are present, so one run covers the matrix.
pub fn builds() -> Vec<(&'static str, PathBuf)> {
    let mut out = Vec::new();
    if let Some(dir) = install_dir() {
        out.push(("1.28c", dir));
    }
    if let Some(dir) = legacy_dir()
        && !out.iter().any(|(_, d)| *d == dir)
    {
        out.push(("1.29b", dir));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Open an install and pull one scripting-side file out of one map.
    fn shipped(dir: &Path, map: &str, name: &str) -> Option<Vec<u8>> {
        let mut set = pcx_mpq::MpqSet::open_wc3(dir).ok()?;
        extract_trig_file(&mut set, map, name).ok()
    }

    /// Guard against a fixture silently skipping: a named-map test must find
    /// its bytes on every present install that ships the map.
    ///
    /// `melee` fixtures live only in 1.28c's map set (1.29b ships just the
    /// campaign maps), so they are only required when a 1.28c install is
    /// present.
    fn assert_ran(ran: usize, melee: bool) {
        let builds = builds();
        if builds.is_empty() {
            return;
        }
        if melee && !builds.iter().any(|(l, _)| *l == "1.28c") {
            return;
        }
        assert!(ran > 0, "fixture found on no present install");
    }

    // The melee maps sit at the map-archive root, without a `Maps\` prefix.
    const MAP_OGRE: &str = "(2)OgreMound.w3m";
    const MAP_DUST: &str = "(10)DustwallowKeys.w3m";
    const MAP_HE05: &str = r"Maps\Campaign\Human05Interlude.w3m";
    const MAP_NE06: &str = r"Maps\Campaign\NightElf06.w3m";
    const MAP_HE06: &str = r"Maps\Campaign\Human06.w3m";
    const FILE_WTS: &str = "war3map.wts";
    const FILE_WTG: &str = "war3map.wtg";
    const FILE_WCT: &str = "war3map.wct";

    // ---- wts: exact fields, read by hand from the bytes ------------------

    #[test]
    fn ogre_mound_wts_fields_are_exact() {
        let mut ran = 0usize;
        for (_, dir) in builds() {
            let Some(bytes) = shipped(&dir, MAP_OGRE, FILE_WTS) else {
                continue;
            };
            ran += 1;
            let w = Wts::parse(&bytes).unwrap();
            assert!(w.bom(), "every shipped WTS starts with the BOM");
            let ids: Vec<i32> = w.entries().iter().map(|e| e.id).collect();
            assert_eq!(ids, [0, 1, 2, 3, 7, 8, 9, 10, 11, 12]);
            assert_eq!(w.entries()[0].body, b"Ogre Mound");
            assert_eq!(w.entries()[1].body, b"1v1");
            assert_eq!(
                w.entries()[2].body,
                b"These islands are rich with gold, but the treasure is controlled by Ogres."
            );
            assert_eq!(w.entries()[3].body, b"Blizzard Entertainment");
            for (&id, entry) in ids[4..=6].iter().zip(&w.entries()[4..=6]) {
                assert_eq!(entry.body, b"", "STRING {id} is empty");
                assert!(entry.comments.is_empty());
            }
            assert_eq!(w.entries()[7].body, b"Player 1");
            assert_eq!(w.entries()[8].body, b"Player 2");
            assert_eq!(w.entries()[9].body, b"Force 1");
            assert!(w.entries().iter().all(|e| e.comments.is_empty()));
            let fr = w.framing();
            assert!(fr.violations().is_empty(), "{:?}", fr.violations());
            assert_eq!(w.canonical_bytes(), bytes);
        }
        assert_ran(ran, true);
    }

    #[test]
    fn human06_wts_comments_are_exact() {
        // The one shipped WTS with `//` comment lines: 1.29b Human06. The
        // 1.28c file is the same map without the comments and is checked
        // for their absence here too.
        let mut ran = 0usize;
        for (label, dir) in builds() {
            let Some(bytes) = shipped(&dir, MAP_HE06, FILE_WTS) else {
                continue;
            };
            ran += 1;
            let w = Wts::parse(&bytes).unwrap();
            let commented: Vec<(i32, &Vec<u8>)> = w
                .entries()
                .iter()
                .flat_map(|e| e.comments.iter().map(move |c| (e.id, c)))
                .collect();
            if label == "1.29b" {
                assert_eq!(commented.len(), 36, "1.29b Human06 comment lines");
                let six = w.get(6).unwrap();
                assert_eq!(six.comments.len(), 1);
                assert_eq!(
                    six.comments[0],
                    b" Sound\\Dialogue\\HumanCampaign\\Human06\\H06MalGanis41.mp3"
                );
                assert_eq!(six.body, b"I live once more!");
                assert!(commented.windows(2).all(|w| w[0].0 < w[1].0));
            } else {
                assert!(commented.is_empty(), "1.28c Human06 carries no comments");
            }
        }
        assert_ran(ran, false);
    }

    #[test]
    fn wts_control_codes_stay_literal() {
        // Human04 opens a description with a colour code on both builds;
        // the code is raw body text and survives the round trip untouched.
        let map = r"Maps\Campaign\Human04.w3m";
        let mut ran = 0usize;
        for (_, dir) in builds() {
            let Some(bytes) = shipped(&dir, map, FILE_WTS) else {
                continue;
            };
            ran += 1;
            let w = Wts::parse(&bytes).unwrap();
            assert!(
                w.entries().iter().any(|e| e.body.starts_with(b"|cfff")),
                "Human04 opens a description with a colour code"
            );
            assert!(w.entries().iter().any(|e| e.body.contains(&b'|')));
            assert_eq!(w.canonical_bytes(), bytes);
        }
        assert_ran(ran, false);
    }

    // ---- wct: exact fields ----------------------------------------------

    #[test]
    fn dustwallow_wct_is_one_empty_block() {
        let mut ran = 0usize;
        for (_, dir) in builds() {
            let Some(bytes) = shipped(&dir, MAP_DUST, FILE_WCT) else {
                continue;
            };
            ran += 1;
            let w = Wct::parse(&bytes).unwrap();
            assert_eq!(w.unknown0(), 0);
            assert_eq!(w.blocks().len(), 1);
            assert_eq!(w.blocks()[0].text, b"");
            let fr = w.framing();
            assert!(fr.sums(), "{fr:?}");
            assert!(fr.violations().is_empty(), "{:?}", fr.violations());
            assert_eq!(w.canonical_bytes(), bytes);
        }
        assert_ran(ran, true);
    }

    #[test]
    fn nightelf06_wct_custom_texts_are_exact() {
        let mut ran = 0usize;
        for (_, dir) in builds() {
            let Some(bytes) = shipped(&dir, MAP_NE06, FILE_WCT) else {
                continue;
            };
            ran += 1;
            let w = Wct::parse(&bytes).unwrap();
            assert_eq!(w.blocks().len(), 69);
            let nonempty: Vec<usize> = w
                .blocks()
                .iter()
                .enumerate()
                .filter(|(_, b)| !b.text.is_empty())
                .map(|(i, _)| i)
                .collect();
            assert_eq!(nonempty, [14, 15]);
            // Both texts are NUL-terminated JASS; the NUL is inside the
            // length. Hand-read lengths from the file's own words.
            assert_eq!(w.blocks()[14].text.len(), 1585);
            assert_eq!(w.blocks()[15].text.len(), 1582);
            assert_eq!(w.blocks()[14].text.last(), Some(&0));
            assert!(w.blocks()[14].text.starts_with(b"//===="));
            assert!(
                w.blocks()[14]
                    .text
                    .windows(22)
                    .any(|x| x == b"Trig_Mana_Pool_Actions")
            );
            let fr = w.framing();
            assert!(fr.texts_are_nul_terminated);
            assert!(fr.sums(), "{fr:?}");
            assert!(fr.violations().is_empty(), "{:?}", fr.violations());
            assert_eq!(w.canonical_bytes(), bytes);
        }
        assert_ran(ran, false);
    }

    // ---- wtg: exact fields, read by hand from the bytes ------------------

    #[test]
    fn dustwallow_wtg_fields_are_exact() {
        let mut ran = 0usize;
        for (label, dir) in builds() {
            let Some(bytes) = shipped(&dir, MAP_DUST, FILE_WTG) else {
                continue;
            };
            ran += 1;
            let arity = install_trigger_arity(&dir).unwrap();
            let w = Wtg::parse(&bytes, &arity).unwrap();
            assert_eq!(w.version(), WTG_VERSION);
            assert_eq!(w.categories().len(), 1);
            assert_eq!(w.categories()[0].id, 0);
            assert_eq!(w.categories()[0].name, b"Initialization");
            assert_eq!(w.unknown_root(), 1);
            assert!(w.variables().is_empty());
            assert_eq!(w.triggers().len(), 1);
            let t = &w.triggers()[0];
            assert_eq!(t.name, b"Melee Initialization");
            assert_eq!(
                t.description,
                b"Default melee game initialization for all players"
            );
            assert_eq!(
                (t.is_enabled, t.is_custom, t.is_initially_off, t.run_on_init),
                (1, 0, 0, 0)
            );
            assert_eq!(t.category_id, 0);
            assert_eq!(t.ecas.len(), 9);
            assert_eq!(t.ecas[0].kind, EcaKind::Event);
            assert_eq!(t.ecas[0].name, b"MapInitializationEvent");
            assert_eq!(t.ecas[0].is_enabled, 1);
            assert!(t.ecas[0].params.is_empty());
            for e in &t.ecas[1..] {
                assert_eq!(e.kind, EcaKind::Action);
                assert_eq!(e.is_enabled, 1);
                assert!(e.params.is_empty());
            }
            let names: Vec<&[u8]> = t.ecas[1..].iter().map(|e| e.name.as_slice()).collect();
            assert_eq!(
                names,
                [
                    &b"MeleeStartingVisibility"[..],
                    &b"MeleeStartingHeroLimit"[..],
                    &b"MeleeGrantHeroItems"[..],
                    &b"MeleeStartingResources"[..],
                    &b"MeleeClearExcessUnits"[..],
                    &b"MeleeStartingUnits"[..],
                    &b"MeleeStartingAI"[..],
                    &b"MeleeInitVictoryDefeat"[..],
                ]
            );
            let fr = w.framing();
            assert!(fr.sums(), "{fr:?}");
            assert!(fr.violations().is_empty(), "{:?}", fr.violations());
            assert_eq!(w.canonical_bytes(), bytes, "{label}");
            assert_ne!(w.canonical_hash(), 0);
        }
        assert_ran(ran, true);
    }

    #[test]
    fn human05_wtg_variables_and_params_are_exact() {
        // Byte-identical on both builds; hand-read from the hex.
        let mut ran = 0usize;
        for (_, dir) in builds() {
            let Some(bytes) = shipped(&dir, MAP_HE05, FILE_WTG) else {
                continue;
            };
            ran += 1;
            let arity = install_trigger_arity(&dir).unwrap();
            let w = Wtg::parse(&bytes, &arity).unwrap();
            assert_eq!(w.categories().len(), 1);
            assert_eq!(w.categories()[0].id, 0);
            assert_eq!(w.categories()[0].name, b"Human05Interlude");
            assert_eq!(w.unknown_root(), 1);
            assert_eq!(w.variables().len(), 4);
            let vs = w.variables();
            assert_eq!(vs[0].name, b"JainFade");
            assert_eq!(vs[0].type_name, b"real");
            assert_eq!(
                (vs[0].unknown, vs[0].is_array, vs[0].is_initialized),
                (1, 0, 1)
            );
            assert_eq!(vs[0].initial_value, b"100.00");
            assert_eq!(vs[1].name, b"counter");
            assert_eq!(vs[1].type_name, b"integer");
            assert_eq!(
                (vs[1].unknown, vs[1].is_array, vs[1].is_initialized),
                (1, 0, 0)
            );
            assert_eq!(vs[1].initial_value, b"");
            assert_eq!(vs[2].name, b"Escape");
            assert_eq!(vs[2].type_name, b"boolean");
            assert_eq!(vs[2].initial_value, b"");
            assert_eq!(vs[3].name, b"ThePlayer");
            assert_eq!(vs[3].type_name, b"player");
            assert_eq!((vs[3].is_array, vs[3].is_initialized), (0, 1));
            assert_eq!(vs[3].initial_value, b"Player00");
            assert_eq!(w.triggers().len(), 10);
            let t = &w.triggers()[0];
            assert_eq!(t.name, b"SceneEscape");
            assert_eq!(t.description, b"");
            assert_eq!(
                (t.is_enabled, t.is_custom, t.is_initially_off, t.run_on_init),
                (1, 0, 0, 0)
            );
            assert_eq!(t.ecas.len(), 4);
            // Event: one preset parameter.
            let e = &t.ecas[0];
            assert_eq!(e.kind, EcaKind::Event);
            assert_eq!(e.name, b"TriggerRegisterPlayerEventEndCinematic");
            assert_eq!(e.params.len(), 1);
            assert_eq!(e.params[0].kind, ParamKind::Preset);
            assert_eq!(e.params[0].value, b"Player00");
            assert!(e.params[0].sub.is_none());
            assert!(e.params[0].array_index.is_none());
            // Action: DisableTrigger(GetTriggeringTrigger()).
            let e = &t.ecas[1];
            assert_eq!(e.kind, EcaKind::Action);
            assert_eq!(e.name, b"DisableTrigger");
            assert_eq!(e.params.len(), 1);
            let p = &e.params[0];
            assert_eq!(p.kind, ParamKind::Function);
            assert_eq!(p.value, b"GetTriggeringTrigger");
            let sub = p.sub.as_ref().unwrap();
            assert_eq!(sub.kind, SubKind::NestedCall);
            assert_eq!(sub.name, b"GetTriggeringTrigger");
            assert_eq!(sub.flag, 1);
            assert!(sub.params.is_empty());
            assert!(p.array_index.is_none());
            // Action: SetVariable(Escape, true).
            let e = &t.ecas[2];
            assert_eq!(e.name, b"SetVariable");
            assert_eq!(e.params.len(), 2);
            assert_eq!(e.params[0].kind, ParamKind::Variable);
            assert_eq!(e.params[0].value, b"Escape");
            assert_eq!(e.params[1].kind, ParamKind::Str);
            assert_eq!(e.params[1].value, b"true");
            // Action: ConditionalTriggerExecute(gg_trg_VICTORY).
            let e = &t.ecas[3];
            assert_eq!(e.name, b"ConditionalTriggerExecute");
            assert_eq!(e.params.len(), 1);
            assert_eq!(e.params[0].kind, ParamKind::Variable);
            assert_eq!(e.params[0].value, b"gg_trg_VICTORY");
            let fr = w.framing();
            assert!(fr.sums(), "{fr:?}");
            assert!(fr.violations().is_empty(), "{:?}", fr.violations());
            assert_eq!(w.canonical_bytes(), bytes);
        }
        assert_ran(ran, false);
    }

    #[test]
    fn nightelf06_wct_aligns_with_its_wtg() {
        let mut ran = 0usize;
        for (_, dir) in builds() {
            let Some(wct_b) = shipped(&dir, MAP_NE06, FILE_WCT) else {
                continue;
            };
            ran += 1;
            let Some(wtg_b) = shipped(&dir, MAP_NE06, FILE_WTG) else {
                continue;
            };
            let arity = install_trigger_arity(&dir).unwrap();
            let wct = Wct::parse(&wct_b).unwrap();
            let wtg = Wtg::parse(&wtg_b, &arity).unwrap();
            assert!(wct_wtg_violations(&wct, &wtg).is_empty());
            let t14 = &wtg.triggers()[14];
            let t15 = &wtg.triggers()[15];
            assert_eq!(t14.name, b"Mana Pool");
            assert_eq!(t14.is_custom, 1);
            assert!(t14.ecas.is_empty());
            assert_eq!(t15.name, b"Doom Take Positions");
            assert_eq!(t15.is_custom, 1);
            // Every other trigger is GUI with an empty custom text.
            for (i, t) in wtg.triggers().iter().enumerate() {
                if i != 14 && i != 15 {
                    assert_eq!(
                        t.is_custom,
                        0,
                        "trigger {i} {}",
                        String::from_utf8_lossy(&t.name)
                    );
                    assert!(wct.blocks()[i].text.is_empty());
                }
            }
        }
        assert_ran(ran, false);
    }

    // ---- framing and round trip on named files --------------------------

    #[test]
    fn framing_and_round_trip_hold_on_named_files() {
        for (_, dir) in builds() {
            let arity = install_trigger_arity(&dir).unwrap();
            for (map, name) in [
                (MAP_OGRE, FILE_WTS),
                (MAP_DUST, FILE_WTS),
                (MAP_DUST, FILE_WTG),
                (MAP_DUST, FILE_WCT),
                (MAP_HE05, FILE_WTS),
                (MAP_HE05, FILE_WTG),
                (MAP_HE05, FILE_WCT),
                (MAP_NE06, FILE_WTS),
                (MAP_NE06, FILE_WTG),
                (MAP_NE06, FILE_WCT),
            ] {
                let Some(bytes) = shipped(&dir, map, name) else {
                    continue;
                };
                match name {
                    FILE_WTS => {
                        let f = Wts::parse(&bytes).unwrap();
                        assert!(f.framing().violations().is_empty(), "{map} {name}");
                        assert_eq!(f.canonical_bytes(), bytes, "{map} {name}");
                    }
                    FILE_WTG => {
                        let f = Wtg::parse(&bytes, &arity).unwrap();
                        assert!(f.framing().violations().is_empty(), "{map} {name}");
                        assert_eq!(f.canonical_bytes(), bytes, "{map} {name}");
                    }
                    FILE_WCT => {
                        let f = Wct::parse(&bytes).unwrap();
                        assert!(f.framing().violations().is_empty(), "{map} {name}");
                        assert_eq!(f.canonical_bytes(), bytes, "{map} {name}");
                    }
                    _ => unreachable!(),
                }
            }
        }
    }

    // ---- arity table ----------------------------------------------------

    #[test]
    fn trigger_data_arity_matches_the_hand_read_functions() {
        for (_, dir) in builds() {
            let arity = install_trigger_arity(&dir).unwrap();
            assert!(arity.len() > 1000);
            assert_eq!(arity.get(b"MapInitializationEvent"), Some(0));
            assert_eq!(arity.get(b"GetTriggeringTrigger"), Some(0));
            assert_eq!(arity.get(b"DoNothing"), Some(0));
            assert_eq!(arity.get(b"ReturnAction"), Some(0));
            assert_eq!(arity.get(b"SetVariable"), Some(2));
            assert_eq!(
                arity.get(b"TriggerRegisterPlayerEventEndCinematic"),
                Some(1)
            );
            assert_eq!(arity.get(b"ConditionalTriggerExecute"), Some(1));
            assert_eq!(arity.get(b"DisableTrigger"), Some(1));
            assert_eq!(arity.get(b"OperatorCompareOrderCode"), Some(3));
            assert_eq!(arity.get(b"IsUnitAliveBJ"), Some(1));
        }
    }

    // ---- refusals -------------------------------------------------------

    fn tiny_arity() -> TriggerArity {
        let mut a = TriggerArity::new();
        a.insert(b"MapInitializationEvent", 0).unwrap();
        a.insert(b"SetVariable", 2).unwrap();
        a.insert(b"GetOwningPlayer", 1).unwrap();
        a
    }

    fn tiny_wtg() -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&WTG_MAGIC);
        v.extend_from_slice(&WTG_VERSION.to_le_bytes());
        v.extend_from_slice(&1u32.to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(b"Initialization\0");
        v.extend_from_slice(&1u32.to_le_bytes()); // unknown root word
        v.extend_from_slice(&0u32.to_le_bytes()); // no variables
        v.extend_from_slice(&1u32.to_le_bytes()); // one trigger
        v.extend_from_slice(b"T\0");
        v.extend_from_slice(b"\0");
        v.extend_from_slice(&1u32.to_le_bytes()); // enabled
        v.extend_from_slice(&0u32.to_le_bytes()); // not custom
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes()); // category 0
        v.extend_from_slice(&1u32.to_le_bytes()); // one ECA
        v.extend_from_slice(&ECA_EVENT.to_le_bytes());
        v.extend_from_slice(b"MapInitializationEvent\0");
        v.extend_from_slice(&1u32.to_le_bytes());
        v
    }

    #[test]
    fn bad_wtg_magic_is_refused() {
        let mut v = tiny_wtg();
        v[0] = b'X';
        let err = Wtg::parse(&v, &tiny_arity()).unwrap_err();
        assert!(matches!(err, Error::BadWtgMagic { .. }), "{err}");
    }

    #[test]
    fn wtg_version_7_is_refused() {
        let mut v = tiny_wtg();
        v[4..8].copy_from_slice(&7u32.to_le_bytes());
        assert!(matches!(
            Wtg::parse(&v, &tiny_arity()),
            Err(Error::UnsupportedWtgVersion { got: 7 })
        ));
    }

    #[test]
    fn unknown_function_is_refused() {
        let err = Wtg::parse(&tiny_wtg(), &TriggerArity::new()).unwrap_err();
        assert!(
            matches!(&err, Error::UnknownFunction { name } if name == "MapInitializationEvent"),
            "{err}"
        );
    }

    #[test]
    fn bad_eca_kind_is_refused() {
        let mut v = tiny_wtg();
        // the ECA kind word sits after the trigger header
        let at = v.len() - 4 - 23 - 4;
        v[at..at + 4].copy_from_slice(&3u32.to_le_bytes());
        assert!(matches!(
            Wtg::parse(&v, &tiny_arity()),
            Err(Error::BadEcaKind { got: 3, .. })
        ));
    }

    fn tiny_wtg_with_param(param: Vec<u8>) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&WTG_MAGIC);
        v.extend_from_slice(&WTG_VERSION.to_le_bytes());
        v.extend_from_slice(&1u32.to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(b"Initialization\0");
        v.extend_from_slice(&1u32.to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(&1u32.to_le_bytes());
        v.extend_from_slice(b"T\0");
        v.extend_from_slice(b"\0");
        v.extend_from_slice(
            &[1u32, 0, 0, 0, 0]
                .iter()
                .flat_map(|w| w.to_le_bytes())
                .collect::<Vec<u8>>(),
        );
        v.extend_from_slice(&1u32.to_le_bytes());
        v.extend_from_slice(&ECA_ACTION.to_le_bytes());
        v.extend_from_slice(b"SetVariable\0");
        v.extend_from_slice(&1u32.to_le_bytes());
        v.extend_from_slice(&param);
        // SetVariable takes two: the second is a harmless empty preset.
        v.extend_from_slice(&PARAM_PRESET.to_le_bytes());
        v.extend_from_slice(b"p\0");
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes());
        v
    }

    #[test]
    fn bad_param_type_is_refused() {
        let mut p = Vec::new();
        p.extend_from_slice(&9i32.to_le_bytes());
        p.extend_from_slice(b"p\0");
        p.extend_from_slice(&0u32.to_le_bytes());
        p.extend_from_slice(&0u32.to_le_bytes());
        assert!(matches!(
            Wtg::parse(&tiny_wtg_with_param(p), &tiny_arity()),
            Err(Error::BadParamType { got: 9, .. })
        ));
    }

    #[test]
    fn minus_one_param_type_is_refused() {
        let mut p = Vec::new();
        p.extend_from_slice(&(-1i32).to_le_bytes());
        p.extend_from_slice(b"p\0");
        p.extend_from_slice(&0u32.to_le_bytes());
        p.extend_from_slice(&0u32.to_le_bytes());
        assert!(matches!(
            Wtg::parse(&tiny_wtg_with_param(p), &tiny_arity()),
            Err(Error::BadParamType { got: -1, .. })
        ));
    }

    #[test]
    fn function_array_access_is_refused() {
        let mut p = Vec::new();
        p.extend_from_slice(&PARAM_FUNCTION.to_le_bytes());
        p.extend_from_slice(b"GetOwningPlayer\0");
        p.extend_from_slice(&0u32.to_le_bytes()); // no sub
        p.extend_from_slice(&1u32.to_le_bytes()); // array word, unproven on functions
        let err = Wtg::parse(&tiny_wtg_with_param(p), &tiny_arity()).unwrap_err();
        assert!(
            matches!(err, Error::UnsupportedFunctionArray { got: 1, .. }),
            "{err}"
        );
    }

    #[test]
    fn bad_sub_kind_is_refused() {
        let mut p = Vec::new();
        p.extend_from_slice(&PARAM_FUNCTION.to_le_bytes());
        p.extend_from_slice(b"GetOwningPlayer\0");
        p.extend_from_slice(&1u32.to_le_bytes()); // has sub
        p.extend_from_slice(&0u32.to_le_bytes()); // sub kind 0: unmeasured
        p.extend_from_slice(b"GetOwningPlayer\0");
        p.extend_from_slice(&1u32.to_le_bytes());
        p.extend_from_slice(&0u32.to_le_bytes()); // trailer
        assert!(matches!(
            Wtg::parse(&tiny_wtg_with_param(p), &tiny_arity()),
            Err(Error::BadSubKind { got: 0, .. })
        ));
    }

    #[test]
    fn sub_flag_other_than_one_is_refused() {
        let mut p = Vec::new();
        p.extend_from_slice(&PARAM_FUNCTION.to_le_bytes());
        p.extend_from_slice(b"GetOwningPlayer\0");
        p.extend_from_slice(&1u32.to_le_bytes());
        p.extend_from_slice(&SUB_CALL.to_le_bytes());
        p.extend_from_slice(b"GetOwningPlayer\0");
        p.extend_from_slice(&0u32.to_le_bytes()); // flag 0: unproven
        p.extend_from_slice(&0u32.to_le_bytes()); // trailer
        assert!(matches!(
            Wtg::parse(&tiny_wtg_with_param(p), &tiny_arity()),
            Err(Error::UnsupportedSubFlag { got: 0, .. })
        ));
    }

    #[test]
    fn unmeasured_has_sub_word_is_refused() {
        let mut p = Vec::new();
        p.extend_from_slice(&PARAM_PRESET.to_le_bytes());
        p.extend_from_slice(b"p\0");
        p.extend_from_slice(&2u32.to_le_bytes()); // neither 0 nor 1
        p.extend_from_slice(&0u32.to_le_bytes());
        assert!(matches!(
            Wtg::parse(&tiny_wtg_with_param(p), &tiny_arity()),
            Err(Error::UnsupportedHasSub { got: 2, .. })
        ));
    }

    #[test]
    fn arity_conflict_is_refused() {
        let mut a = TriggerArity::new();
        a.insert(b"F", 1).unwrap();
        assert!(matches!(
            a.insert(b"F", 2),
            Err(Error::ArityConflict {
                first: 1,
                second: 2,
                ..
            })
        ));
    }

    #[test]
    fn bad_wts_header_is_refused() {
        let err = Wts::parse(b"STRING 0\r\n{\r\nx\r\n}\r\n\r\nTRING 1\r\n").unwrap_err();
        assert!(matches!(err, Error::BadWtsHeader { .. }), "{err}");
    }

    #[test]
    fn non_decimal_wts_id_is_refused() {
        assert!(matches!(
            Wts::parse(b"STRING x\r\n{\r\n\r\n}\r\n\r\n"),
            Err(Error::BadWtsId { .. })
        ));
    }

    #[test]
    fn unterminated_wts_entry_is_refused() {
        assert!(matches!(
            Wts::parse(b"STRING 0\r\n{\r\nbody with no end"),
            Err(Error::UnterminatedWtsEntry { .. })
        ));
    }

    #[test]
    fn wts_trailing_bytes_are_refused() {
        assert!(matches!(
            Wts::parse(b"STRING 0\r\n{\r\n\r\n}\r\n\r\nx"),
            Err(Error::BadWtsHeader { .. })
        ));
    }

    #[test]
    fn wct_trailing_bytes_are_refused() {
        let mut v = Vec::new();
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(&1u32.to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes());
        v.push(0);
        assert!(matches!(Wct::parse(&v), Err(Error::TrailingBytes { .. })));
    }

    #[test]
    fn wct_truncated_text_is_refused() {
        let mut v = Vec::new();
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(&1u32.to_le_bytes());
        v.extend_from_slice(&4u32.to_le_bytes());
        v.extend_from_slice(b"ab");
        assert!(matches!(Wct::parse(&v), Err(Error::Truncated { .. })));
    }

    #[test]
    fn wtg_trailing_bytes_are_refused() {
        let mut v = tiny_wtg();
        v.push(0);
        assert!(matches!(
            Wtg::parse(&v, &tiny_arity()),
            Err(Error::TrailingBytes { .. })
        ));
    }

    #[test]
    fn minimal_files_parse_sum_and_round_trip() {
        let wtg = tiny_wtg();
        let w = Wtg::parse(&wtg, &tiny_arity()).unwrap();
        let fr = w.framing();
        assert!(fr.sums(), "{fr:?}");
        assert!(fr.violations().is_empty(), "{:?}", fr.violations());
        assert_eq!(w.canonical_bytes(), wtg);

        let wts = b"\xef\xbb\xbfSTRING 0\r\n// c\r\n{\r\nbody\r\n}\r\n\r\n";
        let w = Wts::parse(wts).unwrap();
        let fr = w.framing();
        assert!(fr.sums(), "{fr:?}");
        assert!(fr.violations().is_empty(), "{:?}", fr.violations());
        assert_eq!(w.canonical_bytes(), wts);
        assert_eq!(w.entries()[0].comments, vec![b" c".to_vec()]);
        assert_eq!(w.entries()[0].body, b"body");

        let mut wct = Vec::new();
        wct.extend_from_slice(&0u32.to_le_bytes());
        wct.extend_from_slice(&2u32.to_le_bytes());
        wct.extend_from_slice(&0u32.to_le_bytes());
        wct.extend_from_slice(&2u32.to_le_bytes());
        wct.extend_from_slice(b"j\0");
        let w = Wct::parse(&wct).unwrap();
        let fr = w.framing();
        assert!(fr.sums(), "{fr:?}");
        assert!(fr.violations().is_empty(), "{:?}", fr.violations());
        assert_eq!(w.canonical_bytes(), wct);
    }

    // ---- census and full sweep ------------------------------------------

    /// The per-build census, as measured. Runs against every install that
    /// is present.
    #[test]
    fn shipped_trig_file_counts() {
        let mut seen = 0;
        for (label, dir) in builds() {
            seen += 1;
            let mut set = pcx_mpq::MpqSet::open_wc3(&dir).unwrap();
            let maps = shipped_maps(&mut set).unwrap();
            let (mut wts, mut wtg, mut wct) = (0usize, 0usize, 0usize);
            let mut lacking: Vec<String> = Vec::new();
            for map in &maps {
                let names = map_trig_files(&mut set, map).unwrap();
                let mut have = [false; 3];
                for n in &names {
                    let low = n.to_ascii_lowercase();
                    if let Some(i) = EXTENSIONS
                        .iter()
                        .position(|e| low.ends_with(&format!(".{e}")))
                    {
                        have[i] = true;
                    }
                }
                wts += have[0] as usize;
                wtg += have[1] as usize;
                wct += have[2] as usize;
                if !have.iter().all(|h| *h) {
                    lacking.push(format!("{map} {}", format_have(have)));
                }
            }
            match label {
                "1.28c" => {
                    assert_eq!(maps.len(), 89, "1.28c shipped maps");
                    assert_eq!((wts, wtg, wct), (89, 88, 88), "1.28c trig census");
                    assert_eq!(lacking, ["DemoCampaign.w3n Y.."]);
                }
                "1.29b" => {
                    assert_eq!(maps.len(), 44, "1.29b shipped maps");
                    assert_eq!((wts, wtg, wct), (44, 44, 44), "1.29b trig census");
                    assert!(lacking.is_empty(), "{lacking:?}");
                }
                other => panic!("unexpected build {other}"),
            }
        }
        assert!(seen > 0, "no install present");
    }

    fn format_have(have: [bool; 3]) -> String {
        have.iter().map(|h| if *h { 'Y' } else { '.' }).collect()
    }

    /// Full sweep over every shipped scripting-side file on every available
    /// build.
    ///
    /// Ignored by default because it opens whole installs. Writes failures
    /// to `/tmp/opencode/trig-sweep.txt` and prints a per-type census.
    #[test]
    #[ignore = "opens every shipped install; run explicitly"]
    fn sweep_every_shipped_trig_file() {
        use std::fmt::Write as _;
        let mut report = String::new();
        let mut failures: Vec<String> = Vec::new();

        for (label, dir) in builds() {
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
            let arity = match install_trigger_arity(&dir) {
                Ok(a) => a,
                Err(e) => {
                    failures.push(format!("{label}: TriggerData: {e}"));
                    continue;
                }
            };
            let mut per_type = [0usize; 3];
            let mut ok = 0usize;
            let mut entries = 0usize;
            let mut variables = 0usize;
            let mut triggers = 0usize;
            let mut ecas = 0usize;
            let mut params = 0usize;
            let mut blocks = 0usize;
            let mut customs = 0usize;
            for map in &maps {
                let names = match map_trig_files(&mut set, map) {
                    Ok(n) => n,
                    Err(e) => {
                        failures.push(format!("{label} {map}: enumerate: {e}"));
                        continue;
                    }
                };
                // Collect the map's files first so the parsed WTG/WCT can be
                // paired for the cross-file relations.
                let mut extracted: Vec<(usize, String, Vec<u8>)> = Vec::new();
                for name in &names {
                    let low = name.to_ascii_lowercase();
                    let Some(i) = EXTENSIONS
                        .iter()
                        .position(|e| low.ends_with(&format!(".{e}")))
                    else {
                        continue;
                    };
                    per_type[i] += 1;
                    match extract_trig_file(&mut set, map, name) {
                        Ok(b) => extracted.push((i, name.clone(), b)),
                        Err(e) => failures.push(format!("{label} {map} {name}: extract: {e}")),
                    }
                }
                let mut wtg_parsed: Option<Wtg<'_>> = None;
                let mut wct_parsed: Option<Wct<'_>> = None;
                for (i, name, bytes) in &extracted {
                    let (i, name, bytes) = (*i, name.as_str(), bytes.as_slice());
                    match i {
                        0 => match Wts::parse(bytes) {
                            Ok(f) => {
                                let fr = f.framing();
                                for v in fr.violations() {
                                    failures.push(format!("{label} {map} {name}: framing: {v}"));
                                }
                                if f.canonical_bytes().as_slice() != bytes {
                                    failures.push(format!("{label} {map} {name}: round trip"));
                                }
                                if f.canonical_hash() == 0 {
                                    failures.push(format!("{label} {map} {name}: zero hash"));
                                }
                                entries += fr.entries;
                                ok += 1;
                            }
                            Err(e) => failures.push(format!("{label} {map} {name}: parse: {e}")),
                        },
                        1 => match Wtg::parse(bytes, &arity) {
                            Ok(f) => {
                                let fr = f.framing();
                                for v in fr.violations() {
                                    failures.push(format!("{label} {map} {name}: framing: {v}"));
                                }
                                if f.canonical_bytes().as_slice() != bytes {
                                    failures.push(format!("{label} {map} {name}: round trip"));
                                }
                                if f.canonical_hash() == 0 {
                                    failures.push(format!("{label} {map} {name}: zero hash"));
                                }
                                variables += fr.variables;
                                triggers += fr.triggers;
                                ecas += fr.ecas;
                                params += fr.params;
                                customs += f.triggers().iter().filter(|t| t.is_custom != 0).count();
                                ok += 1;
                                wtg_parsed = Some(f);
                            }
                            Err(e) => failures.push(format!("{label} {map} {name}: parse: {e}")),
                        },
                        2 => match Wct::parse(bytes) {
                            Ok(f) => {
                                let fr = f.framing();
                                for v in fr.violations() {
                                    failures.push(format!("{label} {map} {name}: framing: {v}"));
                                }
                                if f.canonical_bytes().as_slice() != bytes {
                                    failures.push(format!("{label} {map} {name}: round trip"));
                                }
                                blocks += fr.blocks;
                                ok += 1;
                                wct_parsed = Some(f);
                            }
                            Err(e) => failures.push(format!("{label} {map} {name}: parse: {e}")),
                        },
                        _ => unreachable!(),
                    }
                }
                if let (Some(wtg), Some(wct)) = (wtg_parsed.as_ref(), wct_parsed.as_ref()) {
                    for v in wct_wtg_violations(wct, wtg) {
                        failures.push(format!("{label} {map}: wct/wtg: {v}"));
                    }
                }
            }
            let _ = writeln!(
                report,
                "{label}: {} maps, files wts {} wtg {} wct {}, {ok} parsed, \
                 {entries} string entries, {variables} variables, {triggers} triggers \
                 ({customs} custom), {ecas} ECAs, {params} parameters, {blocks} wct blocks",
                maps.len(),
                per_type[0],
                per_type[1],
                per_type[2],
            );
            println!(
                "{label}: census wts={} wtg={} wct={} parsed={ok} entries={entries} \
                 variables={variables} triggers={triggers} custom={customs} \
                 ecas={ecas} params={params} wct_blocks={blocks}",
                per_type[0], per_type[1], per_type[2]
            );
        }

        let _ = writeln!(report, "failures: {}", failures.len());
        for f in &failures {
            let _ = writeln!(report, "  {f}");
        }
        let out = std::path::Path::new("/tmp/opencode/trig-sweep.txt");
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

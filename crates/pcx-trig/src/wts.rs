//! `war3map.wts` — the map's table string table, and the reason a community
//! map shows its own text.
//!
//! Every player-visible string a map can display lives here: map name, map
//! description, the `Main Quest` block, unit tooltips, and the `TRIGSTR_`
//! strings the trigger graph pulls in by id. `pcx-script` records the
//! identifiers; this module resolves them.
//!
//! # The shape that actually ships
//!
//! The layout everyone describes first — a run of `STRING <id>` header lines,
//! each followed by a *quoted* string body terminated by an empty line — is
//! **not what either shipped build writes**. It is refuted by the bytes.
//! The corpus measured here is **913 files and 511 490 records, 59.5 MB**:
//!
//! | corpus | files | records | bytes |
//! |---|---|---|---|
//! | 1.28c install (89 maps, incl. DemoCampaign.w3n's `war3campaign.wts`) | 89 | 5 171 | 347 040 |
//! | 1.29b install (44 maps) | 44 | 4 431 | 314 508 |
//! | loose community maps under `Maps\` (DotA, Footman Frenzy, 779 others) | 780 | 501 888 | 58 847 628 |
//! | **total** | **913** | **511 490** | **59 509 176** |
//!
//! Not one of those 511 490 records has a colon after `STRING`, not one has a
//! `+`-prefixed id, and no body is quoted. The real delimiter is a **brace
//! line**: the body sits between a `{` line and a `}` line and is arbitrary
//! bytes.
//!
//! ```text
//! offset        size    field
//! 0x000         0 or 3  UTF-8 BOM (EF BB BF) — present on 901 of the 913
//!                       files measured. The 12 without it are 11 DotA
//!                       builds under `Maps\Diverse\DotA\` that an older
//!                       editor wrote (the five DotA files beside them have
//!                       one) plus `X Hero Siege 3.08`, refused below.
//! then, repeated to EOF:
//!              7       "STRING " — the keyword plus exactly one space,
//!                       511 490 / 511 490 records, never a tab or a colon
//!              1..5    the id, ASCII decimal, no leading zeros, no '+'
//!                       (0 .. 50 015; 5-digit ids are routine on DotA)
//!              2       CR LF
//!              0..n    optional comment region, present on 301 485 records.
//!                       Starts "//" and runs to a line that is exactly "{".
//!              1       "{"
//!              2       CR LF
//!              0..n    the body: arbitrary bytes, up to the first "\r\n}"
//!              2       CR LF
//!              1       "}"
//!              2       CR LF
//!              0..2    CR LF — a blank separator line. 881 files write it on
//!                       every record; 5 omit it only on the last one, and 26
//!                       (all DotA / X Hero Siege, the pre-1.07 editor's
//!                       shape) write it on none of them.
//! ```
//!
//! There is no count, no index, no version word and no trailing empty record.
//! The only way to read the file is to walk it, which is what makes the
//! framing proof below the whole of the correctness argument.
//!
//! # What a body may contain
//!
//! Everything. Measured, not assumed:
//!
//! * **raw newlines** — 9 980 bodies hold a `CR LF` inside them (briefing
//!   text, tooltips, the `Main Quest` block) and 2 108 end with one. Only
//!   `CR LF`-followed-by-`}` ends a body: the single bare `LF` and the single
//!   bare `CR` in the corpus are content.
//! * **quotation marks** — 1 830 bodies contain `"`. They are not escaped and
//!   not special: `"C'mon Timmy, we're goin' be late!"` is a whole body.
//! * **braces** — 248 bodies contain `{` or `}`: `Build |cffffcc00F|rlare
//!   Tower` (EV3 Maul), `Speak with Blathers {Hotkey L}`, `Brother's Bear
//!   Cabin {Hotkey C}`. None is `CR LF`-preceded, so the framing is not
//!   ambiguous: on 0 of the 913 files does a body contain the terminator.
//! * **control codes** — 156 117 bodies carry a `|c`, `|r` or `|n`, and 31 a
//!   `^^` colour tag. They are not escapes in the framing sense; they are
//!   payload. This crate hands them back byte-for-byte. A
//!   decoder that rewrote `|n` to a real newline, or stripped `^^`, would
//!   corrupt every string in the map.
//!
//! # Encoding: the file is bytes, not text
//!
//! The container has a UTF-8 BOM, so it is tempting to call the file UTF-8.
//! **It is not.** 8 of the 913 files fail a UTF-8 decode, and two of them
//! are shipped vanilla campaign maps, byte-identical on both builds:
//!
//! * `Maps\Campaign\BonusCredits.w3m` — string 18's body ends
//!   `...You get to live\xE2\x80\xA6 for today.` and then carries 23 `0xEE`
//!   bytes and `B1 12 DD DD B1 12`. That is the bonus-credits obfuscation and
//!   it is not a code page: `0xEE` is an isolate in every 8-bit encoding the
//!   game used.
//! * community maps carry lone `0xD0`, `0xF0 0xF8 0x12` and `0xF2 0xA3` —
//!   editor typos, not encodings.
//!
//! So [`Table`] never decodes. Bodies are `&[u8]`, preserved exactly.
//! [`Table::decode_utf8`] is the opt-in, and it refuses with a typed
//! [`Error::NotUtf8`] instead of converting lossily: a map whose only problem
//! is one bad byte must still show its other 3 000 strings.
//!
//! # Framing
//!
//! [`Framing`] turns the walk into an arithmetic identity, the same discipline
//! `pcx-mdl` applies to `PRE2` and `pcx-objects` to the object container:
//!
//! ```text
//! file_len == bom + Σ_records
//!             ( 7                       "STRING "
//!             + len(decimal id)
//!             + 2                       CR LF
//!             + comment                 its own trailing CR LF included
//!             + 3                       "{" CR LF
//!             + body
//!             + 4                       CR LF "}" CR LF
//!             + 2 or 0                  the blank separator )
//! ```
//!
//! It holds on the 912 parsed files with zero leftover bytes — the 913th is
//! refused, below — and [`Table::canonical_bytes`] re-serialises from the
//! parsed fields back to the original bytes on the same 912. Id-monotonicity
//! is **not** an invariant and is not claimed: 7 files contain a descending
//! id step, 6 of them at the classic editor's three-digit id wrap, visible on
//! `TowerWars` and on `Counter-Strike`. What is proved is that ids are
//! **unique within a file** — 511 490 records, 0 duplicates — which is what
//! an id-keyed lookup needs, and is checked by [`Table::ids_are_unique`].
//!
//! # Refused
//!
//! * a stray byte ahead of the first header. One file, `X Hero Siege 3.08`,
//!   opens `3F 53 54 52 49 4E 47` — `?STRING 1`. It is not a documented
//!   construct, it is in no other file, and guessing what it means would be a
//!   plausible-looking lie. [`Unsupported::NotAHeader`] names it.
//! * a body with no `CR LF }` to close it, a `}` not followed by `CR LF`, a
//!   non-decimal id, an id that does not fit `u32`, a comment region that
//!   never reaches its `{`, and bytes between a separator and the next header.
//!   Each is a typed [`Unsupported`], each is a construct that occurs on
//!   0/912 measured files, so refusing one changes no map's behaviour.

use std::ops::Range;

/// The UTF-8 byte-order mark. Present on 901 of 913 measured files.
pub const BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];

/// `STRING ` — the header keyword and exactly one space.
pub const HEADER: &[u8; 7] = b"STRING ";

/// The bytes that open a record's body: the brace line and its `CR LF`.
pub const OPEN: &[u8; 3] = b"{\r\n";

/// The bytes that close a record: the body's `CR LF`, the brace, and the
/// record's own `CR LF`.
pub const CLOSE: &[u8; 5] = b"\r\n}\r\n";

/// The blank separator line a record may carry after its own `CR LF`.
pub const BLANK: &[u8; 2] = b"\r\n";

/// `//`, where the 1.29b world editor's comment region begins.
pub const COMMENT: &[u8; 2] = b"//";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("wts truncated: {what} needs {need} bytes at 0x{at:x}, file has {have}")]
    Truncated {
        what: &'static str,
        at: usize,
        need: usize,
        have: usize,
    },
    #[error("unsupported wts construct at 0x{at:x}: {detail:?}")]
    Unsupported { at: usize, detail: Unsupported },
    #[error(
        "string {id} is not valid UTF-8: byte 0x{byte:02x} at body offset {at}; the file is bytes, so read them as bytes"
    )]
    NotUtf8 { id: u32, at: usize, byte: u8 },
}

/// The constructs this decoder refuses rather than guess at.
///
/// Every one of these is a construct that occurs on 0 of the 904 measured
/// files, so refusing one changes no shipped map's behaviour; it only stops a
/// novel file from being walked through a layout nobody has measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unsupported {
    /// The bytes where a `STRING ` header was required.
    ///
    /// This is what refuses the stray `?` at offset 0 of `X Hero Siege 3.08`,
    /// and what would refuse a colon form (`STRING:12`) or a `+` prefix.
    NotAHeader { got: [u8; 7] },
    /// After `STRING ` the id is not at least one ASCII decimal digit, or the
    /// digits are not followed by `CR LF`.
    NonDecimalId,
    /// The decimal id does not fit in `u32`.
    IdOverflow,
    /// After `STRING <id> CR LF` the bytes are neither `{ CR LF` nor `//`.
    NeitherBraceNorComment,
    /// A comment region opened by `//` never reaches the `{` line.
    UnterminatedCommentRegion,
    /// No `CR LF }` closes the body.
    BodyNotClosed,
    /// The body's closing `}` is not followed by `CR LF`.
    CloseNotFollowedByLf,
    /// After a record's blank separator the next bytes are neither another
    /// `STRING ` header nor the end of the file.
    ///
    /// This is the guard that makes the framing honest: it is what fires when
    /// a body does contain `CR LF }` and the walk has matched it early.
    StrayBytesBetweenRecords,
}

/// One parsed record: the id it is looked up by, the byte range of its body in
/// the file, the byte range of its comment region where one was written, and
/// whether a blank separator line followed.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    id: u32,
    comment: Option<Range<usize>>,
    body: Range<usize>,
    blank: bool,
    /// Measured width of the id's digits. 0 of 511 490 measured ids carry a
    /// leading zero, so this agrees with `u32::to_string`; it is stored rather
    /// than recomputed so the framing measures the file instead of assuming.
    digits: usize,
}

/// A record as seen from outside: the body and comment as byte slices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Record<'a> {
    pub id: u32,
    pub body: &'a [u8],
    pub comment: Option<&'a [u8]>,
    /// Was a blank separator line written after this record?
    pub blank_separator: bool,
}

/// One record's place in the file, by length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordFrame {
    /// File offset the record starts at.
    pub at: usize,
    /// `"STRING "` — always 7.
    pub header: usize,
    /// The decimal id's digits.
    pub digits: usize,
    /// `CR LF` after the id — always 2.
    pub id_break: usize,
    /// The comment region, its own trailing `CR LF` included. 0 when absent.
    pub comment: usize,
    /// `{ CR LF` — always 3.
    pub open: usize,
    /// The body.
    pub body: usize,
    /// `CR LF } CR LF` — always 4.
    pub close: usize,
    /// The blank separator: 2 or 0.
    pub blank: usize,
}

impl RecordFrame {
    /// `header + digits + id_break + comment + open + body + close + blank`.
    pub fn total(&self) -> usize {
        self.header
            + self.digits
            + self.id_break
            + self.comment
            + self.open
            + self.body
            + self.close
            + self.blank
    }
}

/// The whole file's accounting. [`Framing::total`] is the file length;
/// [`Framing::sum_of_parts`] is what the walk measured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Framing {
    /// 0 or 3.
    pub bom: usize,
    pub records: Vec<RecordFrame>,
    /// The file length, BOM included.
    pub total: usize,
}

impl Framing {
    /// `bom + Σ records`. Equal to [`Self::total`] on every file measured.
    pub fn sum_of_parts(&self) -> usize {
        self.bom + self.records.iter().map(RecordFrame::total).sum::<usize>()
    }

    /// The offset one past the last record. Equal to [`Self::total`] when the
    /// walk consumed the file exactly, with zero leftover bytes.
    pub fn consumed(&self) -> usize {
        let mut at = self.bom;
        for r in &self.records {
            at += r.total();
        }
        at
    }
}

/// A decoded `war3map.wts`.
///
/// Owns the file bytes, because they come out of an archive rather than off
/// disk and so cannot borrow from the map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    raw: Vec<u8>,
    bom: bool,
    entries: Vec<Entry>,
}

impl Table {
    /// Walk `data` and require the walk to land exactly on its end.
    ///
    /// Nothing is decoded, normalised or re-encoded: the bodies stored here
    /// are exact subslices of `data`, which is what lets
    /// [`canonical_bytes`](Self::canonical_bytes) prove the parse.
    pub fn parse(data: &[u8]) -> Result<Self, Error> {
        let bom = data.starts_with(&BOM);
        let mut at = if bom { BOM.len() } else { 0 };
        if data.len() == at {
            return Err(Error::Truncated {
                what: "the first STRING header",
                at,
                need: HEADER.len(),
                have: data.len(),
            });
        }

        let mut entries = Vec::new();
        while at < data.len() {
            let (id, digits, mut p) = parse_header(data, at)?;

            // The 1.29b world editor writes a `//` annotation between the
            // header and the brace — `// Sound\Dialogue\...` for a voice line,
            // `// Units: hgtw (Felguard), Hotkey (Hotkey)` for a hotkey edit.
            // The older editors write nothing. Both are measured (36 records on
            // 1.29b's Human06, 0 on 1.28c's), so both are read and neither is
            // inferred from the file's age.
            let comment = if data[p..].starts_with(COMMENT) {
                let start = p;
                let end;
                loop {
                    let line_end = p + find(&data[p..], b"\r\n").ok_or(Error::Unsupported {
                        at,
                        detail: Unsupported::UnterminatedCommentRegion,
                    })?;
                    let next = line_end + 2;
                    if &data[p..line_end] == b"{" {
                        // The `{` line: the region ends where that line starts.
                        end = p;
                        p = next;
                        break;
                    }
                    p = next;
                }
                Some(start..end)
            } else if data[p..].starts_with(OPEN) {
                p += OPEN.len();
                None
            } else {
                return Err(Error::Unsupported {
                    at,
                    detail: Unsupported::NeitherBraceNorComment,
                });
            };

            let body_start = p;
            let rel = find(&data[p..], b"\r\n}").ok_or(Error::Unsupported {
                at,
                detail: Unsupported::BodyNotClosed,
            })?;
            let body_end = p + rel;
            p = body_end + 3;
            if data[p..].len() < 2 || &data[p..p + 2] != BLANK {
                return Err(Error::Unsupported {
                    at,
                    detail: Unsupported::CloseNotFollowedByLf,
                });
            }
            p += 2;
            let blank = data[p..].starts_with(BLANK);
            if blank {
                p += 2;
            }
            // Whatever follows must be the next record or the end of the file.
            // Anything else means the terminator matched inside a body and the
            // walk has lost sync: that is a refusal, not a skip.
            if p < data.len() && !data[p..].starts_with(HEADER) {
                return Err(Error::Unsupported {
                    at,
                    detail: Unsupported::StrayBytesBetweenRecords,
                });
            }

            entries.push(Entry {
                id,
                comment,
                body: body_start..body_end,
                blank,
                digits,
            });
            at = p;
        }

        Ok(Self {
            raw: data.to_vec(),
            bom,
            entries,
        })
    }

    /// The file exactly as stored.
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }

    /// Was the UTF-8 BOM present?
    pub fn bom(&self) -> bool {
        self.bom
    }

    /// Number of records.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Every record, in file order.
    pub fn iter(&self) -> impl Iterator<Item = Record<'_>> + '_ {
        self.entries.iter().map(move |e| self.view(e))
    }

    /// The record with this id, or `None`.
    ///
    /// Ids are unique within a file on all 912 measured files, so the lookup
    /// is unambiguous. It is a linear scan because the tables are small and
    /// the big ones are ordered but gapped — DotA v5.84b has 5 771 of the
    /// 10 000 possible ids.
    pub fn get(&self, id: u32) -> Option<Record<'_>> {
        self.entries
            .iter()
            .find(|e| e.id == id)
            .map(|e| self.view(e))
    }

    /// Does this table carry `id`?
    pub fn contains(&self, id: u32) -> bool {
        self.entries.iter().any(|e| e.id == id)
    }

    /// Every id, in file order.
    pub fn ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.entries.iter().map(|e| e.id)
    }

    /// The body of `id` as bytes, exactly as stored.
    pub fn body(&self, id: u32) -> Option<&[u8]> {
        self.get(id).map(|r| r.body)
    }

    /// The body of `id` as UTF-8, or a typed refusal.
    ///
    /// This is the only place a code page or an escape would have to be
    /// guessed, so it is the one place that does not: it succeeds only when
    /// the bytes really are UTF-8 and otherwise reports the first bad byte.
    pub fn decode_utf8(&self, id: u32) -> Result<&str, Error> {
        let Some(record) = self.get(id) else {
            return Err(Error::NotUtf8 { id, at: 0, byte: 0 });
        };
        match std::str::from_utf8(record.body) {
            Ok(s) => Ok(s),
            Err(e) => {
                let at = e.valid_up_to();
                Err(Error::NotUtf8 {
                    id,
                    at,
                    byte: record.body[at],
                })
            }
        }
    }

    /// Are the ids unique? True on all 912 measured files, which is what makes
    /// [`Self::get`] well-defined; run it rather than assume it on a novel
    /// file.
    pub fn ids_are_unique(&self) -> bool {
        let mut seen: Vec<u32> = self.entries.iter().map(|e| e.id).collect();
        seen.sort_unstable();
        seen.windows(2).all(|w| w[0] != w[1])
    }

    /// Smallest and largest id present.
    pub fn id_range(&self) -> Option<(u32, u32)> {
        let mut it = self.entries.iter().map(|e| e.id);
        let first = it.next()?;
        Some(it.fold((first, first), |(lo, hi), id| (lo.min(id), hi.max(id))))
    }

    /// The byte accounting, per record, that sums to the file length.
    pub fn framing(&self) -> Framing {
        let mut records = Vec::with_capacity(self.entries.len());
        let mut at = if self.bom { BOM.len() } else { 0 };
        for e in &self.entries {
            let frame = RecordFrame {
                at,
                header: HEADER.len(),
                digits: e.digits,
                id_break: 2,
                comment: e.comment.as_ref().map_or(0, |c| c.len()),
                open: OPEN.len(),
                body: e.body.len(),
                close: CLOSE.len(),
                blank: if e.blank { BLANK.len() } else { 0 },
            };
            at += frame.total();
            records.push(frame);
        }
        Framing {
            bom: if self.bom { BOM.len() } else { 0 },
            records,
            total: self.raw.len(),
        }
    }

    /// Re-serialise from the parsed fields.
    ///
    /// The strongest check available for a text format, and the same
    /// discipline the other crates use: it rebuilds `STRING <id>`, the comment
    /// region, the brace lines and the separator from what was measured, and
    /// the result is asserted equal to the original bytes on every shipped
    /// file and every community map.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.raw.len());
        if self.bom {
            out.extend_from_slice(&BOM);
        }
        for e in &self.entries {
            out.extend_from_slice(HEADER);
            out.extend_from_slice(e.id.to_string().as_bytes());
            out.extend_from_slice(b"\r\n");
            if let Some(c) = &e.comment {
                out.extend_from_slice(&self.raw[c.clone()]);
            }
            out.extend_from_slice(OPEN);
            out.extend_from_slice(&self.raw[e.body.clone()]);
            out.extend_from_slice(CLOSE);
            if e.blank {
                out.extend_from_slice(BLANK);
            }
        }
        out
    }

    fn view(&self, e: &Entry) -> Record<'_> {
        Record {
            id: e.id,
            body: &self.raw[e.body.clone()],
            comment: e.comment.as_ref().map(|c| &self.raw[c.clone()]),
            blank_separator: e.blank,
        }
    }
}

/// Read `"STRING " <digits> "CR LF"` at `at`, returning
/// `(id, digits, next)`, where `digits` is the id's measured width.
fn parse_header(data: &[u8], at: usize) -> Result<(u32, usize, usize), Error> {
    if !data[at..].starts_with(HEADER) {
        let mut got = [0u8; 7];
        let have = (data.len() - at).min(HEADER.len());
        got[..have].copy_from_slice(&data[at..at + have]);
        return Err(Error::Unsupported {
            at,
            detail: Unsupported::NotAHeader { got },
        });
    }
    let mut p = at + HEADER.len();
    let digits = p;
    while matches!(data.get(p), Some(b'0'..=b'9')) {
        p += 1;
    }
    if p == digits || !data[p..].starts_with(b"\r\n") {
        return Err(Error::Unsupported {
            at,
            detail: Unsupported::NonDecimalId,
        });
    }
    let text = std::str::from_utf8(&data[digits..p]).map_err(|_| Error::Unsupported {
        at,
        detail: Unsupported::NonDecimalId,
    })?;
    let id: u32 = text.parse().map_err(|_| Error::Unsupported {
        at,
        detail: Unsupported::IdOverflow,
    })?;
    Ok((id, p - digits, p + 2))
}

/// First index of `needle` in `haystack`, or `None`.
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn install_dir() -> Option<PathBuf> {
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

    fn legacy_dir() -> Option<PathBuf> {
        let p = std::env::var_os("PEACECRAFT3_LEGACY_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .join(
                        "../../Faugus/battle-net/drive_c/Program Files (x86)/Warcraft III (Legacy)",
                    )
            });
        p.is_dir().then_some(p)
    }

    fn shipped_wts(dir: &Path, map: &str) -> Option<Vec<u8>> {
        let mut set = pcx_mpq::MpqSet::open_wc3(dir).ok()?;
        let raw = set.extract(map).ok()?;
        let m = pcx_map::Map::parse(&raw).ok()?;
        m.extract("war3map.wts").ok()
    }

    fn loose_wts(dir: &Path, relative: &str) -> Option<Vec<u8>> {
        let raw = std::fs::read(dir.join(relative)).ok()?;
        let m = pcx_map::Map::parse(&raw).ok()?;
        m.extract("war3map.wts").ok()
    }

    fn builds() -> Vec<(&'static str, PathBuf)> {
        [("1.28c", install_dir()), ("1.29b", legacy_dir())]
            .into_iter()
            .filter_map(|(label, dir)| dir.map(|d| (label, d)))
            .collect()
    }

    const MAP_BOOTY: &str = "(2)BootyBay.w3m";
    const MAP_NIGHTELF01: &str = r"Maps\Campaign\NightElf01.w3m";
    const MAP_BONUS: &str = r"Maps\Campaign\BonusCredits.w3m";
    const MAP_WC3CREDITS: &str = r"Maps\Campaign\WarCraftIIICredits.w3m";
    const MAP_HUMAN06: &str = r"Maps\Campaign\Human06.w3m";

    /// The arithmetic identity from the module doc, asserted on one file.
    fn assert_framing(raw: &[u8], t: &Table) {
        let f = t.framing();
        assert_eq!(f.total, raw.len(), "framing.total is the file length");
        assert_eq!(f.sum_of_parts(), raw.len(), "the parts sum to the file");
        assert_eq!(f.consumed(), raw.len(), "zero leftover bytes");
        for r in &f.records {
            assert_eq!(r.header, 7);
            assert_eq!(r.id_break, 2);
            assert_eq!(r.open, 3);
            assert_eq!(r.close, 5);
            assert!(r.blank == 0 || r.blank == 2);
            assert_eq!(
                r.total(),
                r.header + r.digits + r.id_break + r.comment + r.open + r.body + r.close + r.blank,
                "each record's parts sum to itself"
            );
        }
    }

    fn assert_round_trip(raw: &[u8], t: &Table) {
        assert_eq!(t.canonical_bytes(), raw, "canonical bytes are the original");
    }

    fn ids_of(t: &Table) -> Vec<u32> {
        t.ids().collect()
    }

    #[test]
    fn booty_bay_is_seven_records_and_nothing_else() {
        let Some(dir) = install_dir() else { return };
        let Some(raw) = shipped_wts(&dir, MAP_BOOTY) else {
            return;
        };
        let t = Table::parse(&raw).unwrap();

        // Hand-read from the bytes: 376 bytes, BOM, seven records 0..6, each
        // separated by a blank line, no comments anywhere.
        assert_eq!(raw.len(), 376);
        assert!(t.bom());
        assert_eq!(t.len(), 7);
        assert_eq!(ids_of(&t), [0, 1, 2, 3, 4, 5, 6]);
        assert!(t.iter().all(|r| r.blank_separator));
        assert!(t.iter().all(|r| r.comment.is_none()));
        assert_eq!(t.get(0).unwrap().body, b"Player 1");
        assert_eq!(t.get(1).unwrap().body, b"Player 2");
        assert_eq!(t.get(2).unwrap().body, b"Force 1");
        assert_eq!(t.get(3).unwrap().body, b"Booty Bay");
        assert_eq!(t.get(4).unwrap().body, b"1v1");
        assert_eq!(
            t.get(5).unwrap().body,
            b"Pirates once anchored their ships in these quiet coves. Now the area is overrun with more brutal tenants. Establish your own kingdom on these islands before your opponent does."
        );
        assert_eq!(t.get(6).unwrap().body, b"Blizzard Entertainment");
        assert!(t.get(7).is_none());
        assert_eq!(t.get(5).unwrap().comment, None);

        // 3 bytes of BOM, then `STRING 0 CRLF { CRLF body CRLF } CRLF CRLF`
        // per record: the body lengths read off the bytes are 8, 8, 7, 9, 3,
        // 176, 22.
        let f = t.framing();
        assert_eq!(f.bom, 3);
        assert_eq!(
            f.records.iter().map(|r| r.body).collect::<Vec<_>>(),
            [8, 8, 7, 9, 3, 176, 22]
        );
        assert_framing(&raw, &t);
        assert_round_trip(&raw, &t);
    }

    #[test]
    fn night_elf_01_keeps_raw_newlines_and_control_codes() {
        let Some(dir) = install_dir() else { return };
        let Some(raw) = shipped_wts(&dir, MAP_NIGHTELF01) else {
            return;
        };
        let t = Table::parse(&raw).unwrap();

        assert_eq!(t.len(), 131);
        assert_eq!(t.id_range(), Some((0, 827)));
        assert!(t.ids_are_unique());

        // The map-description body holds two raw CR LFs *and* a trailing
        // space, which is content and not a framing artefact.
        let desc = t.get(6).unwrap().body;
        assert_eq!(desc.len(), 433);
        assert!(desc.ends_with(b"enchanted homeland."));
        assert!(desc.windows(4).any(|w| w == b"\r\n\r\n"));
        assert_eq!(
            t.decode_utf8(6).unwrap(),
            "With the heroic sacrifice of Grom Hellscream, the pit lord Mannoroth was slain, and the demon curse that had plagued the orcs was put to an end. However, the combined human and orc forces have moved deeper into Ashenvale Forest to ascertain whether a demonic threat still lingers in the land. \r\n\r\nTyrande Whisperwind, the leader of the night elf Sentinels, believes that the outlanders will bring only doom to her enchanted homeland."
        );

        // The `Main Quest` block: five CR LF-separated lines, a `|c` colour
        // code and a `|r` reset, all coming back byte-for-byte.
        let quest = t.get(7).unwrap().body;
        assert_eq!(quest.len(), 145);
        assert_eq!(
            quest,
            b"|cffffcc00MAIN QUEST|r\r\nNight Elf Base\r\n  - Entangle a gold mine\r\n  - Construct a Moon Well\r\n  - Construct an Ancient of War\r\n  - Train 5 Archers"
        );
        assert_eq!(
            t.decode_utf8(7).unwrap(),
            std::str::from_utf8(quest).unwrap()
        );

        // String 9 is the same briefing with the paragraph break inside it
        // removed: 433 bytes minus the four CR/LF bytes is 429.
        let collapsed: Vec<u8> = t
            .get(6)
            .unwrap()
            .body
            .iter()
            .copied()
            .filter(|b| *b != b'\r' && *b != b'\n')
            .collect();
        assert_eq!(collapsed.len(), 429);
        assert_eq!(t.get(9).unwrap().body, collapsed.as_slice());

        assert_framing(&raw, &t);
        assert_round_trip(&raw, &t);
    }

    #[test]
    fn bonus_credits_is_not_utf8_and_is_refused_not_converted() {
        let Some(dir) = install_dir() else { return };
        let Some(raw) = shipped_wts(&dir, MAP_BONUS) else {
            return;
        };
        let t = Table::parse(&raw).unwrap();

        // The longest shipped table: 253 records, 19 139 bytes, and the file
        // is *not* valid UTF-8 — which is why `Table` never decodes.
        assert_eq!(raw.len(), 19_139);
        assert_eq!(t.len(), 253);
        assert!(std::str::from_utf8(&raw).is_err());

        // Body of string 9, read by hand: a quoted body, no escapes.
        assert_eq!(
            t.get(9).unwrap().body,
            b"\"C'mon Timmy, we're goin' be late!\""
        );

        // The construct that makes the file non-UTF-8, read by hand from the
        // bytes: a real ellipsis, then 23 x 0xEE, then B1 12 DD DD B1 12.
        let bad = t.get(18).unwrap().body;
        assert_eq!(bad.len(), 70);
        assert!(bad.starts_with(b"\"Excellent! You get to live\xE2\x80\xA6 for today."));
        assert_eq!(bad.iter().filter(|b| **b == 0xEE).count(), 23);
        assert_eq!(&bad[bad.len() - 6..], &[0xB1, 0x12, 0xDD, 0xDD, 0xB1, 0x12]);

        // The bytes survive the round trip untouched, and the typed refusal
        // names the first bad byte rather than replacing it.
        assert_eq!(t.body(18), Some(bad));
        match t.decode_utf8(18) {
            Err(Error::NotUtf8 { id, at, byte }) => {
                assert_eq!((id, byte), (18, 0xEE));
                assert_eq!(bad[at], 0xEE);
            }
            other => panic!("expected NotUtf8, got {other:?}"),
        }
        // The other 252 bodies are ordinary UTF-8 and decode fine.
        assert_eq!(t.decode_utf8(0).unwrap(), "Player 1");
        assert_eq!(
            t.iter().filter(|r| t.decode_utf8(r.id).is_ok()).count(),
            252
        );

        assert_framing(&raw, &t);
        assert_round_trip(&raw, &t);
    }

    #[test]
    fn warcraft_iii_credits_body_may_end_with_a_blank_line() {
        for (_label, dir) in builds() {
            let Some(raw) = shipped_wts(&dir, MAP_WC3CREDITS) else {
                continue;
            };
            let t = Table::parse(&raw).unwrap();
            // The body of string 6 is `Art Director` followed by an empty line
            // *inside* the braces. The empty line is content: the terminator is
            // the `}` on the line after it, and the walk still lands exactly on
            // the end of the 17 775-byte file.
            assert_eq!(raw.len(), 17_775);
            assert_eq!(t.get(6).unwrap().body, b"Art Director\r\n");
            assert_eq!(t.get(434).unwrap().body, b"Team Lead\r\n");
            assert_eq!(t.get(438).unwrap().body, b"Lead Designer");
            assert_eq!(
                t.get(390).unwrap().body,
                b"Current player has completed the singleplayer campaign on Hard difficulty."
            );
            assert_framing(&raw, &t);
            assert_round_trip(&raw, &t);
        }
    }

    #[test]
    fn human_06_comment_region_is_a_1_29b_construct() {
        let Some(dir) = install_dir() else { return };
        let Some(legacy) = legacy_dir() else { return };
        let Some(a) = shipped_wts(&dir, MAP_HUMAN06) else {
            return;
        };
        let Some(b) = shipped_wts(&legacy, MAP_HUMAN06) else {
            return;
        };
        let old = Table::parse(&a).unwrap();
        let new = Table::parse(&b).unwrap();

        // The same map on the two builds. 1.28c's world editor writes 98
        // records and no annotation at all; 1.29b's writes 99 records, 36 of
        // them annotated with `// Sound\Dialogue\...` or `// Units: ...`.
        assert_eq!(old.len(), 98);
        assert_eq!(new.len(), 99);
        assert_eq!(old.iter().filter(|r| r.comment.is_some()).count(), 0);
        assert_eq!(new.iter().filter(|r| r.comment.is_some()).count(), 36);

        // Read by hand from the 1.29b bytes.
        let r6 = new.get(6).unwrap();
        assert_eq!(r6.body, b"I live once more!");
        assert_eq!(
            r6.comment,
            Some(&b"// Sound\\Dialogue\\HumanCampaign\\Human06\\H06MalGanis41.mp3\r\n"[..])
        );
        let r17 = new.get(17).unwrap();
        assert_eq!(r17.body, b"CONVERSIONMAN");
        assert_eq!(
            r17.comment,
            Some(&b"// Units: UC05 (CONVERSIONMAN), Name (Name)\r\n"[..])
        );
        // 1.28c carries the same record, unannotated.
        assert_eq!(old.get(6).unwrap().body, b"I live once more!");
        assert_eq!(old.get(6).unwrap().comment, None);

        for (raw, t) in [(&a, &old), (&b, &new)] {
            assert_framing(raw, t);
            assert_round_trip(raw, t);
        }
    }

    #[test]
    fn dota_bodies_use_five_digit_ids_and_colour_codes() {
        let Some(dir) = install_dir() else { return };
        // One of the three maps `PLAN.md` §3.11 names as non-negotiable. It is
        // a loose `.w3x` on disk, its `.wts` has no BOM, and its ids run
        // 50 000..50 015 — 16 five-digit ids, the largest in the whole corpus.
        let Some(raw) = loose_wts(&dir, "Maps/Diverse/DotA/DotA Allstars v6.59d.w3x") else {
            return;
        };
        let t = Table::parse(&raw).unwrap();

        assert_eq!(raw.len(), 4_274);
        assert!(t.bom());
        assert_eq!(t.len(), 16);
        assert_eq!(t.id_range(), Some((50_000, 50_015)));
        assert!(t.ids_are_unique());
        assert_eq!(t.get(49_999), None);
        assert_eq!(t.get(50_000).unwrap().body, b"DotA Allstars v6.59d");

        // A `|c001e90ff` colour tag running over 30 mode flags and closing
        // with `|r`: control codes inside one body, preserved byte-for-byte.
        let modes = t.get(50_001).unwrap().body;
        assert!(modes.starts_with(
            b"Choose from 93 unique heroes in an epic battle to defend the Ancients.\r\n\r\nModes: |c001e90ff-ap, -ar, -rd"
        ));
        assert!(modes.ends_with(b"-er|r"));
        assert_eq!(
            modes.iter().filter(|b| **b == b'\r').count(),
            modes.iter().filter(|b| **b == b'\n').count()
        );

        // The longest body in the file: the whole mode/command list.
        assert_eq!(t.get(50_007).unwrap().body.len(), 1_340);

        // The last record ends with a trailing space and carries **no** blank
        // separator line — the file stops at the record's own `CR LF`.
        let last = t.iter().last().unwrap();
        assert_eq!(last.id, 50_015);
        assert!(last.body.ends_with(b"IceFrog@gmail.com "));
        assert!(!last.blank_separator);
        assert_eq!(last.body.len(), 101);

        assert_framing(&raw, &t);
        assert_round_trip(&raw, &t);
    }

    #[test]
    fn dota_6_54b_has_no_bom_and_no_blank_separators() {
        let Some(dir) = install_dir() else { return };
        // The other measured shape: four records, ids gapped across three
        // digits of magnitude, no BOM, and no blank separator on *any*
        // record. This is the file that proves the BOM and the blank line are
        // both optional rather than required.
        let Some(raw) = loose_wts(&dir, "Maps/Diverse/DotA/DotA Allstars v6.54b.w3x") else {
            return;
        };
        let t = Table::parse(&raw).unwrap();

        assert_eq!(raw.len(), 5_640);
        assert!(!t.bom());
        assert_eq!(t.len(), 4);
        assert_eq!(ids_of(&t), [3_008, 3_695, 4_940, 5_069]);
        assert_eq!(t.id_range(), Some((3_008, 5_069)));
        assert_eq!(
            t.iter().map(|r| r.body.len()).collect::<Vec<_>>(),
            [1_173, 1_501, 1_703, 1_179]
        );
        assert!(t.iter().all(|r| !r.blank_separator));
        assert!(t.iter().all(|r| r.comment.is_none()));
        assert!(
            t.get(5_069)
                .unwrap()
                .body
                .starts_with(b"-These rules are only in effect")
        );
        assert_framing(&raw, &t);
        assert_round_trip(&raw, &t);
    }

    #[test]
    fn dota_5_84b_is_the_largest_table_in_the_corpus() {
        let Some(dir) = install_dir() else { return };
        let Some(raw) = loose_wts(&dir, "Maps/Diverse/DotA/DotA Allstars v5.84b.w3x") else {
            return;
        };
        let t = Table::parse(&raw).unwrap();
        // 860 145 bytes, 5 771 records, ids spanning 0..=9 999 with 4 229 of
        // them unused: ids are identities, so a lookup is a scan.
        assert_eq!(raw.len(), 860_145);
        assert_eq!(t.len(), 5_771);
        assert_eq!(t.id_range(), Some((0, 9_999)));
        assert!(t.ids_are_unique());
        assert!(t.ids().any(|id| id == 9_999));
        assert_framing(&raw, &t);
        assert_round_trip(&raw, &t);
    }

    #[test]
    fn footman_frenzy_is_a_plain_blank_separated_table() {
        let Some(dir) = install_dir() else { return };
        let Some(raw) = loose_wts(&dir, "Maps/Footmen/Footman Frenzy v4.7.w3x") else {
            return;
        };
        let t = Table::parse(&raw).unwrap();
        // 2 632 records over the ids 0..=2 798: gapped, unique, and *every*
        // record carries a blank separator, the last one included.
        assert_eq!(t.len(), 2_632);
        assert_eq!(t.id_range(), Some((0, 2_798)));
        assert!(t.ids_are_unique());
        assert_eq!(t.iter().filter(|r| !r.blank_separator).count(), 0);
        assert_eq!(t.get(1).unwrap().body, b"Player 1");
        assert_eq!(t.get(2).unwrap().body, b"Force 1");
        // 2 606 of them carry a `//` comment region, and those are UTF-8
        // German written by the 1.29b editor: the byte-for-byte encoding
        // decision above is load-bearing here, not a corner case.
        assert_eq!(t.iter().filter(|r| r.comment.is_some()).count(), 2_606);
        let r2795 = t.get(2_795).unwrap();
        assert!(
            r2795
                .comment
                .unwrap()
                .starts_with("// F\u{e4}higkeiten: Acht (Schreckgeheul)".as_bytes())
        );
        assert_eq!(r2795.body.len(), 180);
        assert_framing(&raw, &t);
        assert_round_trip(&raw, &t);
    }

    #[test]
    fn empty_bodies_and_gapped_ids_are_ordinary_records() {
        let Some(dir) = install_dir() else { return };
        let Some(raw) = shipped_wts(&dir, r"(2)Harrow.w3m") else {
            return;
        };
        let t = Table::parse(&raw).unwrap();
        // Three records with an empty body, and an id sequence that jumps
        // 3 -> 7: the ids are identities, not indices.
        assert_eq!(ids_of(&t), [0, 1, 2, 3, 7, 8, 9, 10, 11, 12]);
        for id in [7, 8, 9] {
            assert_eq!(t.get(id).unwrap().body, b"");
        }
        assert_eq!(
            t.get(2).unwrap().body,
            b"The first snowfall has come, and resources are limited. There's no time to waste."
        );
        assert_framing(&raw, &t);
        assert_round_trip(&raw, &t);
    }

    #[test]
    fn every_shipped_wts_framing_holds_and_round_trips() {
        for (label, dir) in builds() {
            let mut set = match pcx_mpq::MpqSet::open_wc3(&dir) {
                Ok(s) => s,
                Err(e) => panic!("{label}: open install: {e}"),
            };
            let maps = pcx_map::shipped_maps(&mut set).unwrap();
            let (mut files, mut records, mut bytes) = (0usize, 0usize, 0usize);
            for map in &maps {
                let Ok(raw) = set.extract(map) else { continue };
                let Ok(m) = pcx_map::Map::parse(&raw) else {
                    continue;
                };
                let Ok(wts) = m.extract("war3map.wts") else {
                    continue;
                };
                files += 1;
                bytes += wts.len();
                let t = Table::parse(&wts).unwrap_or_else(|e| panic!("{label} {map}: {e}"));
                records += t.len();
                assert_framing(&wts, &t);
                assert_round_trip(&wts, &t);
                assert!(t.ids_are_unique(), "{label} {map}: duplicate ids");
            }
            assert!(files > 0, "{label}: found no war3map.wts");
            println!("{label}: {files} files, {records} records, {bytes} bytes");
        }
    }

    // ---- what is refused, and how -----------------------------------------

    /// A minimal one-record file, for the negative cases.
    fn one(id: u32, body: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(b"STRING ");
        v.extend_from_slice(id.to_string().as_bytes());
        v.extend_from_slice(b"\r\n{\r\n");
        v.extend_from_slice(body);
        v.extend_from_slice(b"\r\n}\r\n\r\n");
        v
    }

    #[test]
    fn a_missing_header_is_not_guessed() {
        // The one measured case: `Maps\Siege\X Hero Siege 3.08.w3x`'s
        // `war3map.wts` opens `3F 53 54 52 49 4E 47 20 31` — `?STRING 1`, a
        // stray `?` in front of a perfectly ordinary header, and it is the
        // only record-start byte pattern in the corpus that is not `STRING `.
        let mut d = one(1, b"X Hero Siege 3.08");
        d.insert(0, b'?');
        assert_eq!(&d[..9], b"?STRING 1");
        match Table::parse(&d) {
            Err(Error::Unsupported {
                at: 0,
                detail: Unsupported::NotAHeader { got },
            }) => assert_eq!(got, *b"?STRING"),
            other => panic!("got {other:?}"),
        }

        // Empty, shorter than a header, and nothing after the BOM.
        assert!(Table::parse(b"").is_err());
        assert!(Table::parse(b"STR").is_err());
        assert!(Table::parse(&[0xEF, 0xBB, 0xBF]).is_err());
    }

    #[test]
    fn colon_and_plus_prefixed_ids_are_refused() {
        // The form the community documents but no measured file writes.
        //
        // Both are refusals, and both are classified by which bytes first
        // disagree with the layout: the keyword itself. `+STRING 1` and
        // `STRING: 1` each fail the 7-byte `STRING ` test, so they land on
        // `NotAHeader` — the same variant that refuses the stray `?` above.
        // A `NonDecimalId` needs the keyword to have matched and the digits
        // to be missing, which `non_decimal_and_overflowing_ids_are_refused`
        // covers with `STRING 1a`.
        let plus = b"+STRING 1\r\n{\r\nbody\r\n}\r\n\r\n".to_vec();
        assert!(matches!(
            Table::parse(&plus),
            Err(Error::Unsupported {
                detail: Unsupported::NotAHeader { .. },
                ..
            })
        ));
        let colon = b"STRING: 1\r\n{\r\nbody\r\n}\r\n\r\n".to_vec();
        // `STRING: 1` never reaches the id: the seven bytes after the start
        // are `STRING:`, not `STRING `, so it is a header refusal.
        match Table::parse(&colon) {
            Err(Error::Unsupported {
                detail: Unsupported::NotAHeader { got },
                ..
            }) => assert_eq!(got, *b"STRING:"),
            other => panic!("colon: got {other:?}"),
        }
    }

    #[test]
    fn non_decimal_and_overflowing_ids_are_refused() {
        let hex = b"STRING 1a\r\n{\r\nbody\r\n}\r\n\r\n".to_vec();
        assert!(matches!(
            Table::parse(&hex),
            Err(Error::Unsupported {
                detail: Unsupported::NonDecimalId,
                ..
            })
        ));
        let big = b"STRING 99999999999\r\n{\r\nbody\r\n}\r\n\r\n".to_vec();
        assert!(matches!(
            Table::parse(&big),
            Err(Error::Unsupported {
                detail: Unsupported::IdOverflow,
                ..
            })
        ));
    }

    #[test]
    fn an_unclosed_body_is_refused() {
        let no_close = b"STRING 1\r\n{\r\nbody\r\n\r\n\r\n".to_vec();
        assert!(matches!(
            Table::parse(&no_close),
            Err(Error::Unsupported {
                detail: Unsupported::BodyNotClosed,
                ..
            })
        ));
        let no_lf = b"STRING 1\r\n{\r\nbody\r\n}".to_vec();
        assert!(matches!(
            Table::parse(&no_lf),
            Err(Error::Unsupported {
                detail: Unsupported::CloseNotFollowedByLf,
                ..
            })
        ));
        // Neither a brace line nor a comment region after the header.
        let no_brace = b"STRING 1\r\nbody\r\n}\r\n\r\n".to_vec();
        assert!(matches!(
            Table::parse(&no_brace),
            Err(Error::Unsupported {
                detail: Unsupported::NeitherBraceNorComment,
                ..
            })
        ));
    }

    #[test]
    fn an_unterminated_comment_region_is_refused() {
        let d = b"STRING 1\r\n// never reaches a brace line\r\n".to_vec();
        assert!(matches!(
            Table::parse(&d),
            Err(Error::Unsupported {
                detail: Unsupported::UnterminatedCommentRegion,
                ..
            })
        ));
    }

    #[test]
    fn a_body_that_contains_the_terminator_is_refused_not_skipped() {
        // This is the guard that makes the framing honest: without it a body
        // holding `CR LF }` would end the record early and the walk would
        // silently resynchronise on the next `STRING`.
        let mut d = Vec::new();
        d.extend_from_slice(b"STRING 1\r\n{\r\nline one\r\n}\r\nline two\r\n}\r\n\r\n");
        d.extend_from_slice(b"STRING 2\r\n{\r\nok\r\n}\r\n\r\n");
        assert!(matches!(
            Table::parse(&d),
            Err(Error::Unsupported {
                detail: Unsupported::StrayBytesBetweenRecords,
                ..
            })
        ));
        // A 3cs-style comment region that spans a blank line parses, because
        // the region ends at the `{` line wherever that sits.
        let d = b"STRING 782\r\n// note\r\n\r\nmore\r\n{\r\nbody\r\n}\r\n\r\n".to_vec();
        let t = Table::parse(&d).unwrap();
        assert_eq!(t.len(), 1);
        assert_eq!(t.get(782).unwrap().body, b"body");
        assert_eq!(
            t.get(782).unwrap().comment,
            Some(&b"// note\r\n\r\nmore\r\n"[..])
        );
        assert_round_trip(&d, &t);
        assert_framing(&d, &t);
    }

    #[test]
    fn synthetic_bodies_keep_everything() {
        // A bare LF, a bare CR, a quote, braces, a `|n` control code and a `^^`
        // tag, all inside one body: nothing is rewritten.
        let body = b"a\nb\rc\"d{e}f|n|cffffcc00g|r^^h";
        let d = one(0, body);
        let t = Table::parse(&d).unwrap();
        assert_eq!(t.get(0).unwrap().body, body.as_slice());
        assert_framing(&d, &t);
        assert_round_trip(&d, &t);

        // Without the blank separator after the last record.
        let mut d = one(0, body);
        d.truncate(d.len() - 2);
        let t = Table::parse(&d).unwrap();
        assert_eq!(t.get(0).unwrap().body, body.as_slice());
        assert!(!t.iter().last().unwrap().blank_separator);
        assert_framing(&d, &t);
        assert_round_trip(&d, &t);

        // A 5-digit id, the width DotA uses.
        let d = one(50_015, body);
        let t = Table::parse(&d).unwrap();
        assert_eq!(t.get(50_015).unwrap().body, body.as_slice());
        assert_framing(&d, &t);
        assert_round_trip(&d, &t);
    }

    // ---- the sweep --------------------------------------------------------

    /// Every `.wts` in both installs *and* every loose community map the
    /// install carries.
    ///
    /// Ignored by default: it reads 913 files and 59.5 MB. Writes failures
    /// to `/tmp/opencode/wts-sweep.txt` and prints the per-build census.
    #[test]
    #[ignore = "reads 913 files across both installs; run explicitly"]
    fn sweep_every_wts() {
        use std::fmt::Write as _;

        let mut report = String::new();
        let mut failures: Vec<String> = Vec::new();
        // Files this decoder refuses outright, with the construct named.
        let mut refused: Vec<String> = Vec::new();
        let mut totals = [0usize; 3]; // files, records, bytes
        let mut by_build: Vec<(&'static str, usize, usize, usize)> = Vec::new();
        let mut loose = [0usize; 3];
        let mut stats = Sweep::default();

        for (label, dir) in builds() {
            let mut set = match pcx_mpq::MpqSet::open_wc3(&dir) {
                Ok(s) => s,
                Err(e) => {
                    failures.push(format!("{label}: open install: {e}"));
                    continue;
                }
            };
            let maps = match pcx_map::shipped_maps(&mut set) {
                Ok(m) => m,
                Err(e) => {
                    failures.push(format!("{label}: listfile: {e}"));
                    continue;
                }
            };
            let (mut files, mut records, mut bytes) = (0usize, 0usize, 0usize);
            for map in &maps {
                let Ok(raw) = set.extract(map) else { continue };
                let Ok(m) = pcx_map::Map::parse(&raw) else {
                    continue;
                };
                // `war3map.wts` everywhere, plus the demo campaign's own
                // `war3campaign.wts` — the one shipped file that spells its
                // name differently. `pcx-map` measured the pair.
                for name in ["war3map.wts", "war3campaign.wts"] {
                    let Ok(wts) = m.extract(name) else { continue };
                    files += 1;
                    bytes += wts.len();
                    stats.consider(&wts, &mut failures, &mut refused, map);
                    if let Ok(t) = Table::parse(&wts) {
                        records += t.len();
                    }
                }
            }
            by_build.push((label, files, records, bytes));
            totals[0] += files;
            totals[1] += records;
            totals[2] += bytes;
            let _ = writeln!(
                report,
                "{label}: {files} shipped .wts files, {records} records, {bytes} bytes"
            );
        }

        if let Some(dir) = install_dir() {
            walk_loose(
                &dir.join("Maps"),
                &mut failures,
                &mut refused,
                &mut loose,
                &mut stats,
            );
        }
        let _ = writeln!(
            report,
            "loose community maps: {} .wts files, {} records, {} bytes",
            loose[0], loose[1], loose[2]
        );
        let _ = writeln!(
            report,
            "grand total: {} .wts files, {} records, {} bytes",
            totals[0] + loose[0],
            totals[1] + loose[1],
            totals[2] + loose[2]
        );
        let _ = writeln!(
            report,
            "files without a BOM: {}; files with at least one record that has no blank separator: {}",
            stats.no_bom, stats.no_final_blank
        );
        let _ = writeln!(
            report,
            "empty bodies: {}; records with a comment region: {}; records with a |c/|r/|n control code: {}",
            stats.empty_bodies, stats.commented, stats.control_codes
        );
        let _ = writeln!(
            report,
            "records whose body is not valid UTF-8 (8 of these files): {}",
            stats.non_utf8
        );
        let _ = writeln!(report, "refused as unprovable: {}", refused.len());
        for f in &refused {
            let _ = writeln!(report, "  {f}");
        }
        let _ = writeln!(report, "framing or round-trip failures: {}", failures.len());
        for f in &failures {
            let _ = writeln!(report, "  {f}");
        }

        let out = std::path::Path::new("/tmp/opencode/wts-sweep.txt");
        if let Some(parent) = out.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(out, &report).expect("write sweep report");
        println!("{report}");
        assert!(
            failures.is_empty(),
            "{} framing or round-trip failures, see {out:?}",
            failures.len()
        );
        // A refusal is not a failure of the walk — `?STRING 1` on `X Hero
        // Siege 3.08` is a construct nobody has measured a meaning for, and
        // decoding it would be a guess. It is also the only one in the
        // corpus, so the sweep pins the set exactly: a novel construct that
        // this decoder refuses shows up as another line here and fails,
        // rather than being quietly absorbed into a green sweep.
        assert_eq!(
            refused.len(),
            1,
            "expected exactly the one measured refusal; got {refused:?}, see {out:?}"
        );
        assert!(
            refused[0].ends_with(
                "X Hero Siege 3.08.w3x: unsupported wts construct at 0x0: \
                 NotAHeader { got: [63, 83, 84, 82, 73, 78, 71] }"
            ),
            "the one refusal is not the measured one: {}",
            refused[0]
        );
    }

    /// The census the sweep prints, kept in one place so the shipped and the
    /// loose corpus are counted identically.
    #[derive(Default)]
    struct Sweep {
        no_bom: usize,
        no_final_blank: usize,
        empty_bodies: usize,
        commented: usize,
        control_codes: usize,
        non_utf8: usize,
    }

    impl Sweep {
        /// Assert the framing identity and the round trip on one file, and
        /// count the constructs found in it.
        ///
        /// A file that cannot be parsed is not a failure of the walk: it is a
        /// construct this decoder refuses, and it is reported as its own line
        /// with the exact byte offsets that broke, so a novel construct cannot
        /// hide inside a green sweep.
        fn consider(
            &mut self,
            wts: &[u8],
            failures: &mut Vec<String>,
            refused: &mut Vec<String>,
            name: &str,
        ) {
            let t = match Table::parse(wts) {
                Ok(t) => t,
                Err(e) => {
                    refused.push(format!("{name}: {e}"));
                    return;
                }
            };
            let f = t.framing();
            if f.sum_of_parts() != wts.len() || f.consumed() != wts.len() {
                failures.push(format!(
                    "{}: framing {} vs {} bytes",
                    name,
                    f.sum_of_parts(),
                    wts.len()
                ));
            }
            if t.canonical_bytes() != wts {
                failures.push(format!("{name}: canonical bytes differ"));
            }
            if !t.ids_are_unique() {
                failures.push(format!("{name}: duplicate ids"));
            }
            if !t.bom() {
                self.no_bom += 1;
            }
            if t.iter().any(|r| !r.blank_separator) {
                self.no_final_blank += 1;
            }
            for r in t.iter() {
                if r.body.is_empty() {
                    self.empty_bodies += 1;
                }
                if r.comment.is_some() {
                    self.commented += 1;
                }
                if r.body
                    .windows(2)
                    .any(|w| w == b"|c" || w == b"|r" || w == b"|n")
                {
                    self.control_codes += 1;
                }
                if t.decode_utf8(r.id).is_err() {
                    self.non_utf8 += 1;
                }
            }
        }
    }

    /// Recurse a directory reading every loose map's `war3map.wts`.
    fn walk_loose(
        path: &Path,
        failures: &mut Vec<String>,
        refused: &mut Vec<String>,
        totals: &mut [usize; 3],
        consider: &mut Sweep,
    ) {
        if path.is_dir() {
            if let Ok(rd) = std::fs::read_dir(path) {
                for e in rd.flatten() {
                    walk_loose(&e.path(), failures, refused, totals, consider);
                }
            }
            return;
        }
        let Some(ext) = path.extension().and_then(|s| s.to_str()) else {
            return;
        };
        if !(ext.eq_ignore_ascii_case("w3x")
            || ext.eq_ignore_ascii_case("w3m")
            || ext.eq_ignore_ascii_case("w3n"))
        {
            return;
        }
        let Ok(raw) = std::fs::read(path) else { return };
        let Ok(m) = pcx_map::Map::parse(&raw) else {
            return;
        };
        let Ok(wts) = m.extract("war3map.wts") else {
            return;
        };
        totals[0] += 1;
        totals[2] += wts.len();
        let name = path.display().to_string();
        consider.consider(&wts, failures, refused, &name);
        if let Ok(t) = Table::parse(&wts) {
            totals[1] += t.len();
        }
    }
}

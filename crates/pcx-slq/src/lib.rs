//! WC3 `.slk` table parser — the "sound list" tables under `UI\SoundInfo\`.
//!
//! # What the format actually is
//!
//! There is no binary Miles "sound list" container. Every `.slk` in the
//! shipped archives is **plain ASCII in the SYLK (Spreadsheet Link Key)
//! dialect**, the same format `Units\UnitData.slk` and `Units\ItemData.slk`
//! use. The sound tables are the twelve `UI\SoundInfo\*.slk` members the
//! engine enumerates by name; 1.29b adds a thirteenth, `AmbientMusic.slk`.
//!
//! Vanilla 1.28c `game.dll` settles the identity independently of the bytes:
//! the NUL-terminated literal `ID;PWXL;N;E` sits at `.rdata+0x95494e`
//! (`VA 0x6f95494e`, image base `0x6f000000`), immediately followed by
//! `\Database.cpp`, `DATABASE`, `DBFIELD` and `DBENTRY` — the engine compares
//! the first eleven bytes against it and dispatches on single-letter records.
//! The twelve `UI\SoundInfo\` paths are listed verbatim at `.rdata+0x937064`.
//!
//! # Byte offsets
//!
//! The container has no fixed-size header: after an 11-byte signature the file
//! is a CRLF-delimited record stream. Measured on the smallest sound table,
//! `UI\SoundInfo\EnvironmentSounds.slk` (1,239 bytes, byte-identical in 1.28c
//! and 1.29b):
//!
//! ```text
//! offset  len  bytes                            record
//! 0x0000    11  ID;PWXL;N;E                     signature / `I` record
//! 0x000b     2  \r\n
//! 0x000d    12  B;X17;Y11;D0                    declared 17 cols, 11 rows
//! 0x001b    26  C;X1;Y1;K"EnvironmentType"      row 1 col 1 — the header
//! 0x0037    16  C;X2;K"RoomType"                row 1 col 2 — no `Y`
//! 0x0049    20  C;X3;K"EffectVolume"            row 1 col 3 — no `X`
//! ...
//! 0x0175    18  C;X1;Y2;K"Default"              row 2 restarts with `Y`
//! ...
//! 0x04d4     1  E                                end record
//! 0x04d5     2  \r\n                             EOF at 0x04d7
//! ```
//!
//! # Record grammar
//!
//! A record is `<letter>` followed by `;`-separated fields. Each field is a
//! one-character tag plus its value; `"` quotes a value and `""` escapes a
//! literal quote inside it. Field order is **not** fixed (`F;Y341;X1` puts `Y`
//! first, `C;Y1;X7` puts `Y` first), so every field is scanned by tag.
//!
//! ```text
//! letter  shipped forms                        what this crate takes
//! I       ID;PWXL;N;E                          the 11-byte signature, nothing else
//! B       B;X17;Y11;D0                         X, Y, D — a capacity hint, see below
//!                                            (position is not fixed)
//! C       C;X1;Y1;K"EnvironmentType"           X, Y, K, and E (recorded, not evaluated)
//! F       F;Y341;X1  F;P0;DG0G8;M255           X, Y only — F is a formatting record that
//!                                              doubles as a cursor carrier
//! P       P;PGeneral  P;EArial;M200;SB         nothing — Excel print/format hints
//! O       O;L;S;D;V0;K47;G100 0.001            nothing — Excel outline hints
//! E       E                                       nothing — the terminator
//! ```
//!
//! Anything else is a hard [`Error::Unsupported`]: an unrecognised record letter
//! might carry data, and guessing is worse than refusing.
//!
//! # The cursor rule
//!
//! `X` and `Y` are **sticky state shared by `C` and `F` records**. A record
//! that omits a tag reuses the last value seen on *either* letter. This is not
//! a subtlety: `UI\SoundInfo\DialogSounds.slk` carries no `Y` on any of its
//! 14,634 `C` records. Its rows are advanced entirely by interleaved format
//! records:
//!
//! ```text
//! F;Y340;X1
//! C;K"U02Acolyte02"
//! F;X2
//! C;K"U02Acolyte02.mp3";ECONCATENATE(RC[-1],".mp3")
//! F;X3
//! C;K"Sound\\Dialogue\\UndeadCampaign\\Undead02\\"
//! ```
//!
//! Reading `X`/`Y` off the `C` record alone collapses that file to 2,231 of
//! its 2,439 rows, double-writes 1,249 cells and leaves 202 rows that exist
//! only as gaps. `Splats\T_SplatData.slk` doubles 1,361 cells the same way.
//! Reading the cursor off both letters leaves zero duplicates and zero gaps in
//! all 124 shipped files.
//!
//! # `B` counts are a hint, not a bound, and `B` is not at a fixed offset
//!
//! All 124 files carry exactly one `B` record, but three of them
//! (`Splats\T_SpawnData.slk`, `Splats\T_SplatData.slk`,
//! `UI\SoundInfo\DialogSounds.slk`) open with `P;PGeneral` instead and only
//! reach `B` after 120, 126 and 222 records respectively — `DialogSounds`
//! writes `B;Y2439;X6;D0 0 2438 5` on line 223. Those three also pack three
//! extra space-separated numbers into `D`, which is passed through as
//! [`Dimensions::date_extras`] rather than decoded.
//!
//! Where the `B` record *does* sit, it is a hint and nothing more.
//! `EnvironmentSounds.slk` declares `Y11` and stops at row 5.
//! `Splats\LightningData.slk` declares `Y97` and `X16` and stops at row 16
//! column 14. `Units\UnitBalance.slk` declares `X61` but row 1 names only 60
//! columns — column 6 exists in every data row with the value `FALSE` and has
//! no header at all. Nothing may be sized or bounds-checked from `B`; see
//! [`Table::declared`].
//!
//! # `E` formulas are redundant, not authoritative
//!
//! 1.28c's `DialogSounds.slk` gives 2,430 cells an `E` expression, all of the
//! form `ECONCATENATE(RC[-1],".mp3")` — Excel's record of how the `.mp3` suffix
//! was produced. 1.29b ships the same table with no `E` records at all and
//! byte-identical `K` values in all 14,634 cells. `K` is therefore the value
//! and `E` is kept only so a caller can see that a cell was generated.
//!
//! # What is *not* provable
//!
//! * Cell text is UTF-8-looking ASCII only by measurement: across all 124 files
//!   no cell value contains a byte above `0x7e` or a control character other
//!   than tab, and none contains a `""` escape. Values are returned as
//!   [`String`]s but nothing in the format requires them to be UTF-8.
//! * `Splats\T_SplatData.slk` (1.28c only) has three records written
//!   `C;Y1;X7;A…` — `A` is not a SYLK parameter, and the payload is an Excel
//!   cell comment. The real header follows a few records later, so nothing is
//!   lost, but the payload is never read as a value: see
//!   [`AnomalyKind::CellValueUnknown`].
//! * The three files whose `B` record is not on line 2 pack extra numbers into
//!   `D` whose meaning three samples do not identify; they are passed through
//!   as [`Dimensions::date_extras`].
//!
//! [`slqx`](../slqx/index.html) (`info` / `dump` / `hash`) is the inspector.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

/// The eleven bytes every shipped `.slk` starts with.
///
/// Verified as the exact opening line of all 124 `.slk` members in 1.28c
/// (59 files) and 1.29b (65 files), and present NUL-terminated in vanilla
/// `game.dll`.
pub const SIGNATURE: &[u8] = b"ID;PWXL;N;E";

/// The `B` record's declared capacity.
///
/// Always advisory: see the module docs. [`Table::declared`] returns it, the
/// grid is sized from what the `C` records actually carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dimensions {
    /// `B;X` — declared column count.
    pub columns: u32,
    /// `B;Y` — declared row count.
    pub rows: u32,
    /// `B;D` — the leading integer of the date field. `0` in every shipped
    /// file.
    pub date: u32,
    /// Whatever follows that integer in the `D` field, verbatim.
    ///
    /// Empty for 121 of the 124 shipped files, which write just `D0`. The other
    /// three — `Splats\T_SpawnData.slk`, `Splats\T_SplatData.slk` and
    /// `UI\SoundInfo\DialogSounds.slk` — write `D0 0 <last row> <n>` where
    /// `<n>` is `1`, `29` and `5` respectively. Three samples do not identify
    /// the field, so it is passed through rather than decoded.
    pub date_extras: String,
}

/// A record letter the crate understands.
///
/// `P`, `O` and `F` are Excel presentation records. They are counted and, for
/// `F`, read for their `X`/`Y` cursor — but nothing they carry is exposed,
/// because nothing in either build's data needs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RecordKind {
    /// `I` — the signature record.
    Id,
    /// `B` — declared capacity.
    Begin,
    /// `C` — a cell.
    Cell,
    /// `F` — a cell format record; also a cursor carrier.
    Format,
    /// `P` — a print/format hint.
    Print,
    /// `O` — an outline hint.
    Outline,
    /// `E` — the end-of-file record.
    End,
}

impl RecordKind {
    fn from_letter(c: u8) -> Option<Self> {
        Some(match c {
            b'I' => Self::Id,
            b'B' => Self::Begin,
            b'C' => Self::Cell,
            b'F' => Self::Format,
            b'P' => Self::Print,
            b'O' => Self::Outline,
            b'E' => Self::End,
            _ => return None,
        })
    }

    /// The ASCII letter this kind is written with.
    pub fn letter(self) -> char {
        match self {
            Self::Id => 'I',
            Self::Begin => 'B',
            Self::Cell => 'C',
            Self::Format => 'F',
            Self::Print => 'P',
            Self::Outline => 'O',
            Self::End => 'E',
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("empty file")]
    Empty,
    #[error("bad signature {found:?}, expected {SIGNATURE:?}")]
    BadSignature { found: Vec<u8> },
    #[error("no CRLF after the signature at byte {offset}")]
    NoLineEnd { offset: usize },
    #[error("byte {offset}: {letter:?} is not a record letter")]
    NotARecord { offset: usize, letter: u8 },
    #[error("record {index} at byte {offset}: unterminated quoted value")]
    UnterminatedQuote { index: usize, offset: usize },
    #[error("record {index}: parameter {tag:?} has a non-numeric value {value:?}")]
    NonNumeric {
        index: usize,
        tag: char,
        value: String,
    },
    #[error("record {index}: X or Y index is zero, which addresses no cell")]
    ZeroIndex { index: usize },
    #[error(
        "record {index} at byte {offset}: {axis} coordinate {actual} exceeds caller limit {limit}"
    )]
    CoordinateLimit {
        index: usize,
        offset: usize,
        axis: char,
        actual: u32,
        limit: u32,
    },
    #[error("truncated: {what} at byte {offset}, file is {len} bytes")]
    Truncated {
        what: &'static str,
        offset: usize,
        len: usize,
    },
    #[error("no terminating `E` record")]
    NoTerminator,
    #[error("unsupported: {what}{}", detail.as_ref().map(|d| format!(": {d}")).unwrap_or_default())]
    Unsupported {
        what: &'static str,
        detail: Option<String>,
    },
}

/// How bad an [`Anomaly`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// The file said something this crate does not model, and nothing is lost.
    Info,
    /// The file contradicts itself, but every cell still decoded.
    Warning,
    /// Something in the file is not recoverable.
    Error,
}

/// The kinds of thing a shipped file does that a strict reader has to notice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AnomalyKind {
    /// A later K value addresses an already populated cell. The existing first
    /// value is retained; no native duplicate precedence is established.
    DuplicateCell,
    /// A `C` record carried a payload under a tag other than `K`.
    ///
    /// The three `Splats\T_SplatData.slk` records written `C;Y1;X7;A…` hit
    /// this: `A` is not a SYLK parameter, and the payload is an Excel cell
    /// comment (`Rob M:<ESC> :This is the cell # of the texture…`). The real
    /// value follows a few records later, so [`Severity::Warning`]; had it not,
    /// the cell would have stayed empty and the severity would be
    /// [`Severity::Error`]. Either way the payload is never read as a value.
    CellValueUnknown,
    /// A `C` record had no value at all.
    CellValueMissing,
    /// The `B` record's `X`/`Y` disagree with the cells that are present.
    DeclaredSizeMismatch,
    /// A column has cells but row 1 does not name it.
    UnnamedColumn,
    /// A `C` record carried an `E` formula. The `K` value is still the value;
    /// this only records that the cell was generated.
    GeneratedCell,
    /// The file ended without an `E` record. Treated as truncation above, so
    /// this exists only for a file that ends in `E` somewhere other than last.
    StrayDataAfterEnd,
}

/// One provable defect or surprise in a shipped table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anomaly {
    pub kind: AnomalyKind,
    pub severity: Severity,
    /// 1-based row, when the anomaly is attached to one.
    pub row: Option<u32>,
    /// 1-based column, when the anomaly is attached to one.
    pub column: Option<u32>,
    /// Zero-based index of the offending record within the file.
    pub record: usize,
    pub detail: String,
}

/// A decoded `.slk`: a sparse 1-based grid plus everything the parse could not
/// prove.
///
/// Row 1 is the header row by convention across all 124 shipped files. Rows 2
/// and up are records. Columns are sparse in both directions — 33 of the 59
/// 1.28c files have data rows missing columns, and `Units\UnitBalance.slk` has
/// a column (`X6`, value `FALSE`) that exists in every row and is named by
/// none.
#[derive(Debug, Clone, Default)]
pub struct Table {
    cells: BTreeMap<(u32, u32), String>,
    headers: BTreeMap<u32, String>,
    generated: BTreeSet<(u32, u32)>,
    declared: Option<Dimensions>,
    max_row: u32,
    max_column: u32,
    record_counts: BTreeMap<char, u32>,
    anomalies: Vec<Anomaly>,
}

/// One row's cells, by column index.
#[derive(Debug, Clone)]
pub struct Row<'a> {
    table: &'a Table,
    index: u32,
}

impl<'a> Row<'a> {
    /// 1-based row number.
    pub fn index(&self) -> u32 {
        self.index
    }

    /// This row's cell for a 1-based column, if it has one.
    pub fn get(&self, column: u32) -> Option<&'a str> {
        self.table
            .cells
            .get(&(self.index, column))
            .map(String::as_str)
    }

    /// This row's cell for a column named in row 1, if both exist.
    pub fn by_name(&self, name: &str) -> Option<&'a str> {
        self.get(self.table.column_of(name)?)
    }

    /// This row's populated cells in column order.
    pub fn cells(&self) -> impl Iterator<Item = (u32, &'a str)> {
        self.table
            .row_cells(self.index)
            .map(|(c, v)| (c, v.as_str()))
    }

    /// True when the row has no cells at all.
    pub fn is_empty(&self) -> bool {
        self.cells().next().is_none()
    }
}

/// One cell of a table plus the byte offset it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    /// 1-based row.
    pub row: u32,
    /// 1-based column.
    pub column: u32,
    /// Row 1's name for this column, if it has one.
    pub header: Option<String>,
    /// The cell's text, with quoting removed.
    pub value: String,
    /// Whether the cell carried an `E` formula, i.e. Excel generated it.
    pub generated: bool,
}

/// Explicit coordinate ceilings for [`Table::parse_bounded`].
///
/// Limits apply to the sticky cursor of both C and F records, not advisory B
/// dimensions. They bound grid extent and coordinate-driven postprocessing;
/// input bytes, record count and total allocator memory are not capped here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoordinateLimits {
    pub max_row: u32,
    pub max_column: u32,
}

impl Table {
    /// Parse a whole `.slk`.
    ///
    /// Fails only on something that makes the grid unrecoverable: a wrong
    /// signature, an unknown record letter, an unterminated quote, a
    /// non-numeric index, a zero index, or truncation. Provable-but-lossy
    /// oddities come back in [`Table::anomalies`] instead.
    pub fn parse(data: &[u8]) -> Result<Self, Error> {
        Self::parse_with_limits(data, None)
    }

    /// Parse with caller-selected C/F cursor coordinate ceilings.
    ///
    /// A zero ceiling refuses every positive coordinate on that axis. Limits
    /// are checked before cell insertion or coordinate-driven finish work.
    /// The caller must separately bound source bytes when admitting input;
    /// this does not provide an allocation or total-work budget.
    pub fn parse_bounded(data: &[u8], limits: CoordinateLimits) -> Result<Self, Error> {
        Self::parse_with_limits(data, Some(limits))
    }

    fn parse_with_limits(data: &[u8], limits: Option<CoordinateLimits>) -> Result<Self, Error> {
        let mut t = Table::default();

        let head = data.get(..SIGNATURE.len()).ok_or(Error::Empty)?;
        if head != SIGNATURE {
            return Err(Error::BadSignature {
                found: head.to_vec(),
            });
        }
        let mut offset = SIGNATURE.len();
        if !data[offset..].starts_with(b"\r\n") {
            return Err(Error::NoLineEnd { offset });
        }
        offset += 2;
        *t.record_counts.entry('I').or_default() += 1;

        // `X` and `Y` are sticky across *both* `C` and `F` records.
        let mut cursor_x: u32 = 1;
        let mut cursor_y: u32 = 1;
        let mut cursor_seen = false;
        let mut record = 1usize;
        let mut terminated = false;

        while offset < data.len() {
            // Linear scan for the next CRLF; re-slicing the tail per record
            // would make this quadratic on the 828 KB `Units\UnitWeapons.slk`.
            let tail = &data[offset..];
            let end = tail
                .windows(2)
                .position(|w| w == b"\r\n")
                .ok_or(Error::Truncated {
                    what: "record terminator",
                    offset,
                    len: data.len(),
                })?;
            let line = &tail[..end];
            let line_offset = offset;
            offset += end + 2;

            if line.is_empty() {
                // A blank line is not a record; skip it rather than guess.
                continue;
            }
            let kind = RecordKind::from_letter(line[0]).ok_or(Error::NotARecord {
                offset: line_offset,
                letter: line[0],
            })?;
            *t.record_counts.entry(kind.letter()).or_default() += 1;

            let record_index = record;
            let fields = split_fields(&line[1..], record, line_offset)?;
            record += 1;

            if terminated {
                t.push_anomaly(Anomaly {
                    kind: AnomalyKind::StrayDataAfterEnd,
                    severity: Severity::Warning,
                    row: None,
                    column: None,
                    record,
                    detail: format!("{:?} record after `E`", kind.letter()),
                });
            }

            match kind {
                RecordKind::Id => {
                    // The signature is checked byte-exactly above; nothing in
                    // the payload after `I` is read.
                }
                RecordKind::Begin => {
                    if let Some(d) = read_dimensions(&fields, record)? {
                        t.declared = Some(d);
                    }
                }
                RecordKind::Cell | RecordKind::Format => {
                    if let Some(v) = field(&fields, 'X') {
                        cursor_x = parse_index(v, 'X', record)?;
                    }
                    if let Some(v) = field(&fields, 'Y') {
                        cursor_y = parse_index(v, 'Y', record)?;
                    }
                    cursor_seen = true;
                    if cursor_x == 0 || cursor_y == 0 {
                        return Err(Error::ZeroIndex { index: record });
                    }
                    if let Some(limits) = limits {
                        for (axis, actual, limit) in [
                            ('X', cursor_x, limits.max_column),
                            ('Y', cursor_y, limits.max_row),
                        ] {
                            if actual > limit {
                                return Err(Error::CoordinateLimit {
                                    index: record_index,
                                    offset: line_offset,
                                    axis,
                                    actual,
                                    limit,
                                });
                            }
                        }
                    }
                    if kind == RecordKind::Cell {
                        let generated = field(&fields, 'E').is_some();
                        match field(&fields, 'K') {
                            Some(value) => {
                                t.insert(cursor_y, cursor_x, value, generated, record_index);
                                // Extra K fields were previously ignored; preserve the
                                // first value while retaining each repeated write.
                                for _ in fields.iter().filter(|(tag, _)| *tag == 'K').skip(1) {
                                    t.duplicate_cell(cursor_y, cursor_x, record_index);
                                }
                            }
                            None => {
                                let stray = fields
                                    .iter()
                                    .find(|(tag, _)| !matches!(*tag, 'X' | 'Y' | 'E'));
                                let kind = if stray.is_some() {
                                    AnomalyKind::CellValueUnknown
                                } else {
                                    AnomalyKind::CellValueMissing
                                };
                                let detail = match stray {
                                    Some((tag, v)) => format!(
                                        "row {cursor_y} col {cursor_x}: payload under \
                                         {tag:?} ({:?}) instead of `K`",
                                        truncate(v, 48)
                                    ),
                                    None => format!("row {cursor_y} col {cursor_x}: no `K`"),
                                };
                                t.push_anomaly(Anomaly {
                                    kind,
                                    severity: Severity::Error,
                                    row: Some(cursor_y),
                                    column: Some(cursor_x),
                                    record,
                                    detail,
                                });
                                t.max_row = t.max_row.max(cursor_y);
                                t.max_column = t.max_column.max(cursor_x);
                            }
                        }
                    }
                }
                RecordKind::Print | RecordKind::Outline | RecordKind::End => {}
            }
            if kind == RecordKind::End {
                terminated = true;
            }
        }

        if !cursor_seen {
            // No `C` record at all: a header-only file is legal SYLK but has
            // nothing to decode, and shipping one would be a surprise.
            return Err(Error::Unsupported {
                what: "file contains no C records",
                detail: None,
            });
        }
        if !terminated {
            return Err(Error::NoTerminator);
        }

        t.finish();
        Ok(t)
    }

    fn insert(&mut self, row: u32, column: u32, value: &str, generated: bool, record: usize) {
        if self.cells.contains_key(&(row, column)) {
            self.duplicate_cell(row, column, record);
        }
        self.max_row = self.max_row.max(row);
        self.max_column = self.max_column.max(column);
        if row == 1 {
            self.headers
                .entry(column)
                .or_insert_with(|| value.to_string());
        }
        if generated {
            self.generated.insert((row, column));
            self.push_anomaly(Anomaly {
                kind: AnomalyKind::GeneratedCell,
                severity: Severity::Info,
                row: Some(row),
                column: Some(column),
                record: 0,
                detail: format!("row {row} col {column} carries an E formula"),
            });
        }
        self.cells
            .entry((row, column))
            .or_insert_with(|| value.to_string());
    }

    fn duplicate_cell(&mut self, row: u32, column: u32, record: usize) {
        self.push_anomaly(Anomaly {
            kind: AnomalyKind::DuplicateCell,
            severity: Severity::Warning,
            row: Some(row),
            column: Some(column),
            record,
            detail: format!("row {row} col {column}: repeated K value; first value retained"),
        });
    }

    fn push_anomaly(&mut self, a: Anomaly) {
        self.anomalies.push(a);
    }

    /// Post-pass: header row, unnamed columns, declared-size comparison.
    ///
    /// Also settles how bad each [`AnomalyKind::CellValueUnknown`] was: a cell
    /// that a later record filled in was only annotated, not damaged.
    fn finish(&mut self) {
        for a in &mut self.anomalies {
            if a.kind != AnomalyKind::CellValueUnknown {
                continue;
            }
            let filled = match (a.row, a.column) {
                (Some(row), Some(column)) => self.cells.contains_key(&(row, column)),
                _ => false,
            };
            if filled {
                a.severity = Severity::Warning;
                a.detail.push_str(" (a later record supplies the value)");
            }
        }
        for &(row, column) in self.cells.keys() {
            if row == 1 {
                self.headers
                    .entry(column)
                    .or_insert_with(|| self.cells[&(row, column)].clone());
            }
        }
        let named: Vec<u32> = self.headers.keys().copied().collect();
        let unnamed: Vec<u32> = (1..=self.max_column)
            .filter(|c| !named.contains(c))
            .collect();
        for column in unnamed {
            self.push_anomaly(Anomaly {
                kind: AnomalyKind::UnnamedColumn,
                severity: Severity::Warning,
                row: None,
                column: Some(column),
                record: 0,
                detail: format!("column {column} has cells but no row-1 header"),
            });
        }
        if let Some(d) = self.declared.clone()
            && (d.rows != self.max_row || d.columns != self.max_column)
        {
            self.push_anomaly(Anomaly {
                kind: AnomalyKind::DeclaredSizeMismatch,
                severity: Severity::Info,
                row: None,
                column: None,
                record: 0,
                detail: format!(
                    "B record declares {}x{}, cells reach {}x{}",
                    d.columns, d.rows, self.max_column, self.max_row
                ),
            });
        }
    }

    /// The `B` record's declared capacity, if the file has one.
    ///
    /// Three of the 59 1.28c tables have no `B` record at all — the two
    /// `Splats\T_*.slk` files and `UI\SoundInfo\DialogSounds.slk` open with
    /// `P;PGeneral` instead — so this is `None` for them.
    pub fn declared(&self) -> Option<&Dimensions> {
        self.declared.as_ref()
    }

    /// Highest 1-based row index that carries a cell.
    pub fn row_count(&self) -> u32 {
        self.max_row
    }

    /// Highest 1-based column index that carries a cell.
    pub fn column_count(&self) -> u32 {
        self.max_column
    }

    /// Number of populated cells, header row included.
    pub fn cell_count(&self) -> usize {
        self.cells.len()
    }

    /// How many records of each letter the file carried.
    pub fn record_counts(&self) -> &BTreeMap<char, u32> {
        &self.record_counts
    }

    /// Everything the parse could not prove. Never fatal; see [`Anomaly`].
    ///
    /// `DialogSounds.slk` contributes 2,430 `GeneratedCell` entries, so this
    /// is not small for every file.
    pub fn anomalies(&self) -> &[Anomaly] {
        &self.anomalies
    }

    /// `(row, column)` of every cell Excel generated from a formula.
    pub fn generated_cells(&self) -> &BTreeSet<(u32, u32)> {
        &self.generated
    }

    /// Row 1's column names, in column order.
    pub fn headers(&self) -> impl Iterator<Item = (u32, &str)> {
        self.headers.iter().map(|(c, s)| (*c, s.as_str()))
    }

    /// Row 1's name for a 1-based column.
    pub fn header(&self, column: u32) -> Option<&str> {
        self.headers.get(&column).map(String::as_str)
    }

    /// The 1-based column a row-1 name refers to.
    ///
    /// Names are compared verbatim. Two shipped headers carry spaces —
    /// `MIDISounds.slk` spells its priority column `Priority ` (trailing
    /// space) and `PortraitAnims.slk` spells its first `Sound Label` — so
    /// trimming here would answer for a column that does not exist.
    pub fn column_of(&self, name: &str) -> Option<u32> {
        self.headers
            .iter()
            .find(|(_, v)| v.as_str() == name)
            .map(|(k, _)| *k)
    }

    /// One cell by 1-based coordinates.
    pub fn get(&self, row: u32, column: u32) -> Option<&str> {
        self.cells.get(&(row, column)).map(String::as_str)
    }

    /// One cell by row and row-1 name.
    pub fn by_name(&self, row: u32, name: &str) -> Option<&str> {
        self.by_name_checked(row, name)
    }

    fn by_name_checked(&self, row: u32, name: &str) -> Option<&str> {
        let column = self.column_of(name)?;
        self.get(row, column)
    }

    /// Every populated cell in `(row, column)` order.
    pub fn cells(&self) -> impl Iterator<Item = (&(u32, u32), &str)> {
        self.cells.iter().map(|(k, v)| (k, v.as_str()))
    }

    /// The populated cells of one row, in column order.
    pub fn row_cells(&self, row: u32) -> impl Iterator<Item = (u32, &String)> {
        let lo = (row, 0);
        let hi = (row, u32::MAX);
        self.cells
            .range(lo..=hi)
            .filter_map(move |(k, v)| (k.0 == row).then_some((k.1, v)))
    }

    /// Rows 1..=`row_count`, including any that turned out to be empty.
    pub fn rows(&self) -> impl Iterator<Item = Row<'_>> {
        (1..=self.max_row).map(move |i| Row {
            table: self,
            index: i,
        })
    }

    /// One row by 1-based index.
    pub fn row(&self, index: u32) -> Row<'_> {
        Row { table: self, index }
    }

    /// Data rows — everything below the header.
    pub fn data_rows(&self) -> impl Iterator<Item = Row<'_>> {
        (2..=self.max_row).map(move |i| Row {
            table: self,
            index: i,
        })
    }

    /// A `Cell` view of one coordinate, carrying its header.
    pub fn cell(&self, row: u32, column: u32) -> Option<Cell> {
        let value = self.get(row, column)?.to_string();
        Some(Cell {
            row,
            column,
            header: self.header(column).map(str::to_string),
            value,
            generated: self.generated.contains(&(row, column)),
        })
    }

    /// Every populated cell as a [`Cell`].
    pub fn cell_list(&self) -> Vec<Cell> {
        self.cells
            .keys()
            .filter_map(|&(r, c)| self.cell(r, c))
            .collect()
    }

    /// A deterministic re-serialisation of the decoded grid.
    ///
    /// One line per populated cell, in `(row, column)` order:
    /// `Y<row>;X<col>;K<value>\n`. This is what [`Table::fnv1a64`] digests, so
    /// an independent decoder can be compared byte for byte without agreeing
    /// on anything but this format.
    pub fn canonical(&self) -> Vec<u8> {
        let mut out = String::with_capacity(self.cells.len() * 24);
        for (&(row, column), value) in &self.cells {
            let _ = writeln!(out, "Y{row};X{column};K{value}");
        }
        out.into_bytes()
    }

    /// FNV-1a 64 over [`Table::canonical`].
    pub fn fnv1a64(&self) -> u64 {
        fnv1a64(&self.canonical())
    }
}

/// Sound-list accessors for the `UI\SoundInfo\` tables.
///
/// Every measurement behind these is in the module docs; they exist because
/// the raw cells need three separate pieces of knowledge to be usable.
#[derive(Debug, Clone, Copy)]
pub struct SoundTable<'a>(pub &'a Table);

impl<'a> SoundTable<'a> {
    /// The files named by a row's `FileNames` cell, split on `,`.
    ///
    /// Empty parts are dropped because 46 `FileNames` cells in
    /// `UnitCombatSounds.slk` end in a trailing comma (`…,MetalHeavyBashFlesh3.wav,`).
    /// A disabled entry keeps its `_` placeholder and is returned as `_`:
    /// `AnimSounds.slk` rows 74, 255 and 266 carry exactly that.
    pub fn file_names(&self, row: u32) -> Vec<&'a str> {
        self.0
            .by_name(row, "FileNames")
            .unwrap_or("")
            .split(',')
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// The archive path for file `index` of a row.
    ///
    /// `DirectoryBase` is inconsistent about its trailing separator —
    /// `AbilitySounds.slk` writes `Abilities\Spells\Human\Blizzard` with
    /// none, `AmbienceSounds.slk` writes `Units\Human\Knight\` with one — so
    /// exactly one `\` is inserted when needed. The result is an MPQ path:
    /// `Abilities\Spells\Human\Blizzard\BlizzardTarget1.wav` is in
    /// `war3.mpq` (spelled with a lowercase `a`; MPQ lookup is
    /// case-insensitive).
    pub fn sound_path(&self, row: u32, index: usize) -> Option<String> {
        let base = self.0.by_name(row, "DirectoryBase")?;
        let file = self.file_names(row).get(index)?.to_string();
        if base.is_empty() {
            return Some(file);
        }
        if base.ends_with('\\') {
            Some(format!("{base}{file}"))
        } else {
            Some(format!("{base}\\{file}"))
        }
    }

    /// The row's `EAXFlags` label, which names a row of `EAXDefs.slk`.
    ///
    /// `AmbientMusic.slk` (1.29b only) has no `EAXFlags` column, so this is
    /// `None` there — not an empty string.
    pub fn eax_flags(&self, row: u32) -> Option<&'a str> {
        self.0.by_name(row, "EAXFlags")
    }
}

/// FNV-1a 64, the digest `slqx hash` reports.
pub fn fnv1a64(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// One `;`-separated field: a one-character tag and its unquoted value.
type Fields = Vec<(char, String)>;

fn split_fields(line: &[u8], index: usize, offset: usize) -> Result<Fields, Error> {
    let mut out: Fields = Vec::new();
    let mut tag: Option<char> = None;
    let mut value: Vec<u8> = Vec::new();
    let mut quoted = false;
    let mut i = 0usize;
    while i < line.len() {
        let c = line[i];
        if quoted {
            if c == b'"' {
                if line.get(i + 1) == Some(&b'"') {
                    value.push(b'"');
                    i += 2;
                    continue;
                }
                quoted = false;
                i += 1;
                continue;
            }
            value.push(c);
            i += 1;
            continue;
        }
        match c {
            b'"' => {
                quoted = true;
                i += 1;
            }
            b';' => {
                push_field(&mut out, tag, &mut value, line, index)?;
                tag = None;
                i += 1;
            }
            _ => {
                if tag.is_none() {
                    // The tag is the first byte of the field. Anything that is
                    // not printable ASCII is not a tag this crate can read.
                    tag = Some(c as char);
                    i += 1;
                } else {
                    value.push(c);
                    i += 1;
                }
            }
        }
    }
    if quoted {
        return Err(Error::UnterminatedQuote { index, offset });
    }
    push_field(&mut out, tag, &mut value, line, index)?;
    Ok(out)
}

fn push_field(
    out: &mut Fields,
    tag: Option<char>,
    value: &mut Vec<u8>,
    line: &[u8],
    index: usize,
) -> Result<(), Error> {
    if let Some(t) = tag {
        let v = match std::str::from_utf8(value) {
            Ok(s) => s.to_string(),
            Err(_) => {
                return Err(Error::Unsupported {
                    what: "field value is not UTF-8",
                    detail: Some(format!(
                        "record {index}: {:?}",
                        truncate(&String::from_utf8_lossy(line), 48)
                    )),
                });
            }
        };
        out.push((t, v));
    }
    value.clear();
    Ok(())
}

fn field(fields: &Fields, tag: char) -> Option<&str> {
    fields
        .iter()
        .find(|(t, _)| *t == tag)
        .map(|(_, v)| v.as_str())
}

fn parse_index(v: &str, tag: char, index: usize) -> Result<u32, Error> {
    v.parse::<u32>().map_err(|_| Error::NonNumeric {
        index,
        tag,
        value: v.to_string(),
    })
}

fn read_dimensions(fields: &Fields, index: usize) -> Result<Option<Dimensions>, Error> {
    let (Some(x), Some(y)) = (field(fields, 'X'), field(fields, 'Y')) else {
        // A `B` record without both counts declares nothing usable. Every
        // shipped `B` has all three, so this is a shape we have not seen.
        return Ok(None);
    };
    let date = match field(fields, 'D') {
        Some(d) => {
            // Three shipped files pack extra numbers into `D`, separated by
            // spaces rather than by the `;` field separator. Only the leading
            // integer is decoded; the rest is passed through untouched.
            let (head, tail) = match d.split_once(' ') {
                Some((h, t)) => (h, t),
                None => (d, ""),
            };
            (
                parse_index(head, 'D', index)?,
                tail.trim_start().to_string(),
            )
        }
        None => (0, String::new()),
    };
    Ok(Some(Dimensions {
        columns: parse_index(x, 'X', index)?,
        rows: parse_index(y, 'Y', index)?,
        date: date.0,
        date_extras: date.1,
    }))
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        return s.to_string();
    }
    s.chars().take(n).chain(['…']).collect()
}

pub fn load(path: impl AsRef<Path>) -> Result<Table, Error> {
    Table::parse(&std::fs::read(path)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Where to find a WC3 install.
    ///
    /// `PEACECRAFT3_INSTALL` overrides it so the same sweeps can be pointed
    /// at any build; `PEACECRAFT3_LEGACY_DIR` is accepted as an alias because
    /// that is where the 1.29b Battle.net install lives by default.
    fn install_dir() -> Option<std::path::PathBuf> {
        for key in ["PEACECRAFT3_INSTALL", "PEACECRAFT3_LEGACY_DIR"] {
            if let Ok(v) = std::env::var(key) {
                let p = std::path::PathBuf::from(v);
                if p.is_dir() {
                    return Some(p);
                }
            }
        }
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("wc3/game/app");
        p.is_dir().then_some(p)
    }

    /// Extracts one file out of the shipped archives.
    ///
    /// Searches every archive, not just `war3.mpq`: only `Units\*.slk` and a
    /// few others live there, and the `UI\SoundInfo\` tables this crate is
    /// about come out of `War3x.mpq`.
    fn shipped(rel: &str) -> Option<Vec<u8>> {
        let dir = install_dir()?;
        pcx_mpq::MpqSet::open_wc3(dir).ok()?.extract(rel).ok()
    }

    fn shipped_table(rel: &str) -> Option<Table> {
        let d = shipped(rel)?;
        match Table::parse(&d) {
            Ok(t) => Some(t),
            Err(e) => panic!("{rel}: {e}"),
        }
    }

    #[test]
    fn environment_sounds_is_the_smallest_sound_table_and_has_four_rows() {
        let Some(d) = shipped("UI\\SoundInfo\\EnvironmentSounds.slk") else {
            eprintln!("skipping: no extracted install");
            return;
        };
        assert_eq!(
            d.len(),
            1239,
            "size changed; the offset table in the module docs is stale"
        );
        let t = Table::parse(&d).expect("parses");

        // Byte offsets quoted in the module docs.
        assert_eq!(&d[0x0000..0x000b], b"ID;PWXL;N;E");
        assert_eq!(&d[0x000b..0x000d], b"\r\n");
        assert_eq!(&d[0x000d..0x0019], b"B;X17;Y11;D0");
        assert_eq!(&d[0x001b..0x0035], b"C;X1;Y1;K\"EnvironmentType\"");
        assert_eq!(&d[0x0037..0x0047], b"C;X2;K\"RoomType\"");
        assert_eq!(&d[0x04d4..0x04d5], b"E");

        assert_eq!(
            t.declared(),
            Some(Dimensions {
                columns: 17,
                rows: 11,
                date: 0,
                date_extras: String::new(),
            })
            .as_ref()
        );
        // The `B` record over-declares: 5 rows exist, 11 were declared.
        assert_eq!(t.row_count(), 5);
        assert_eq!(t.column_count(), 17);
        assert_eq!(t.cell_count(), 17 * 5);
        assert!(
            t.anomalies()
                .iter()
                .any(|a| a.kind == AnomalyKind::DeclaredSizeMismatch)
        );

        assert_eq!(t.header(1), Some("EnvironmentType"));
        assert_eq!(t.header(17), Some("DisplayText"));
        assert_eq!(t.by_name(2, "EnvironmentType"), Some("Default"));
        assert_eq!(t.by_name(2, "RoomType"), Some("MOUNTAINS"));
        assert_eq!(t.by_name(2, "EffectVolume"), Some("1"));
        assert_eq!(t.by_name(2, "DecayTime"), Some("2.5"));
        assert_eq!(t.by_name(2, "Size"), Some("10"));
        assert_eq!(t.by_name(2, "Room"), Some("0"));
        assert_eq!(t.by_name(2, "DecayHFRatio"), Some("1"));
        assert_eq!(t.by_name(2, "Reflections"), Some("-2250"));
        assert_eq!(t.by_name(2, "ReflectionsDelay"), Some("0.07"));
        assert_eq!(t.by_name(2, "Reverb"), Some("-2500"));
        assert_eq!(t.by_name(2, "ReverbDelay"), Some("0.03"));
        assert_eq!(t.by_name(2, "RoomRolloff"), Some("0"));
        assert_eq!(t.by_name(2, "AirAbsorption"), Some("-2"));
        assert_eq!(t.by_name(2, "DisplayText"), Some("WESTRING_SNDENV_DEFAULT"));
        assert_eq!(t.by_name(5, "EnvironmentType"), Some("Dungeon"));
        assert_eq!(t.by_name(5, "RoomType"), Some("MOUNTAINS"));
        assert_eq!(t.by_name(5, "DisplayText"), Some("WESTRING_SNDENV_DUNGEON"));
        // Nothing in row 6: the declared 11 rows are not there.
        assert!(t.get(6, 1).is_none());
    }

    #[test]
    fn ability_sounds_carries_the_engine_path_in_two_columns() {
        let Some(t) = shipped_table("UI\\SoundInfo\\AbilitySounds.slk") else {
            return;
        };
        assert_eq!(t.declared().unwrap().columns, 15);
        assert_eq!(t.row_count(), 58);
        assert_eq!(t.header(14), Some("version"));
        assert_eq!(t.header(15), Some("InBeta"));

        assert_eq!(t.by_name(2, "SoundName"), Some("BlizzardWave"));
        assert_eq!(
            t.by_name(2, "FileNames"),
            Some("BlizzardTarget1.wav,BlizzardTarget2.wav,BlizzardTarget3.wav")
        );
        assert_eq!(
            t.by_name(2, "DirectoryBase"),
            Some("Abilities\\Spells\\Human\\Blizzard")
        );
        assert_eq!(t.by_name(2, "Volume"), Some("127"));
        assert_eq!(t.by_name(2, "Pitch"), Some("1"));
        assert_eq!(t.by_name(2, "Priority"), Some("10"));
        assert_eq!(t.by_name(2, "Channel"), Some("13"));
        assert_eq!(t.by_name(2, "Flags"), Some("WANT3D,IGNOREUSERNAME"));
        assert_eq!(t.by_name(2, "MinDistance"), Some("750"));
        assert_eq!(t.by_name(2, "MaxDistance"), Some("100000"));
        assert_eq!(t.by_name(2, "DistanceCutoff"), Some("2000"));
        assert_eq!(t.by_name(2, "EAXFlags"), Some("SpellsEAX"));
        assert_eq!(t.by_name(4, "SoundName"), Some("RainOfFireWave"));

        // Row 3 has a single file name, so the list collapses to one entry.
        let s = SoundTable(&t);
        assert_eq!(s.file_names(3), vec!["BlizzardLoop1.wav"]);
        assert_eq!(s.file_names(2).len(), 3);
        assert_eq!(
            s.sound_path(2, 0).as_deref(),
            Some("Abilities\\Spells\\Human\\Blizzard\\BlizzardTarget1.wav")
        );
        assert_eq!(s.sound_path(3, 1), None);
        assert_eq!(s.eax_flags(2), Some("SpellsEAX"));
    }

    /// `DirectoryBase` carries a trailing separator in some tables and not in
    /// others; both have to produce the same MPQ path.
    #[test]
    fn ambience_sounds_directory_base_already_ends_in_a_separator() {
        let Some(t) = shipped_table("UI\\SoundInfo\\AmbienceSounds.slk") else {
            return;
        };
        assert_eq!(
            t.by_name(2, "DirectoryBase"),
            Some("Units\\Human\\Knight\\")
        );
        assert_eq!(t.by_name(2, "SoundName"), Some("HumanKnightMovement"));
        assert_eq!(
            t.by_name(2, "FileNames"),
            Some("HorseLoop1.wav,HorseLoop2.wav,HorseLoop3.wav")
        );
        assert_eq!(
            t.by_name(2, "Flags"),
            Some("WANT3D,CHANNELFULLPREEMPT,NODUPLICATES,SCALEPRIORITY")
        );
        let s = SoundTable(&t);
        assert_eq!(
            s.sound_path(2, 0).as_deref(),
            Some("Units\\Human\\Knight\\HorseLoop1.wav")
        );

        // Row 13 is the `_` disabled marker and carries nothing else. Treating
        // it as a row would fabricate 14 empty sound definitions.
        assert_eq!(t.row(13).cells().count(), 1);
        assert_eq!(t.by_name(13, "SoundName"), Some("_"));
        assert!(s.file_names(13).is_empty());
    }

    /// The cursor rule. `DialogSounds.slk` never puts `Y` on a `C` record, so a
    /// reader that only looks at `C` mis-files 1,249 cells and invents 202 row
    /// gaps. This pins the correct mapping at the far end of the table.
    #[test]
    fn dialog_sounds_takes_its_cursor_from_the_interleaved_f_records() {
        let Some(t) = shipped_table("UI\\SoundInfo\\DialogSounds.slk") else {
            return;
        };
        // In 1.28c the `B` record is not on line 2 here: Excel emits 220 print
        // records and 1,463 format records first, then declares the sheet on
        // line 223 and packs extra numbers into `D`. 1.29b strips the
        // presentation records, so the same file declares itself on line 2.
        let d = t
            .declared()
            .expect("every shipped .slk declares a B record");
        assert_eq!((d.columns, d.rows, d.date), (6, 2439, 0));
        if !d.date_extras.is_empty() {
            assert_eq!(d.date_extras, "0 2438 5", "the 1.28c D payload");
        }
        assert_eq!(t.row_count(), 2439);
        assert_eq!(t.column_count(), 6);
        assert_eq!(t.cell_count(), 6 * 2439);
        assert_eq!(
            t.anomalies()
                .iter()
                .filter(|a| a.severity == Severity::Error)
                .count(),
            0
        );

        assert_eq!(t.header(1), Some("SoundName"));
        assert_eq!(t.header(6), Some("version"));
        // Row 2, whose column cursor is set by a leading `F;Y2;X1`.
        assert_eq!(t.by_name(2, "SoundName"), Some("H01Arthas02"));
        assert_eq!(t.by_name(2, "FileNames"), Some("H01Arthas02.mp3"));
        assert_eq!(
            t.by_name(2, "DirectoryBase"),
            Some("Sound\\Dialogue\\HumanCampaign\\Human01\\")
        );
        assert_eq!(t.by_name(2, "Volume"), Some("120"));
        assert_eq!(
            t.by_name(2, "Flags"),
            Some("CHANNELFULLPREEMPT|LISTFULLPREEMPT")
        );
        assert_eq!(t.by_name(2, "version"), Some("0"));
        // Row 340, whose row number arrives as `F;Y340;X1`, not on the `C`.
        assert_eq!(t.by_name(340, "SoundName"), Some("U02Acolyte02"));
        assert_eq!(t.by_name(340, "FileNames"), Some("U02Acolyte02.mp3"));
        assert_eq!(t.by_name(341, "SoundName"), Some("U02Acolyte04"));
        assert_eq!(t.by_name(341, "Volume"), Some("120"));
        // Row 512, deep past where a `C`-only reader loses the thread.
        assert_eq!(t.by_name(512, "SoundName"), Some("O01Cairne23"));
        assert!(t.by_name(2439, "FileNames").is_some());
    }

    /// The `E` expression in 1.28c is Excel's recipe, not the value: 1.29b
    /// ships the same table without it and with identical `K` text.
    #[test]
    fn dialog_sounds_generated_cells_report_the_k_value_not_the_formula() {
        let Some(t) = shipped_table("UI\\SoundInfo\\DialogSounds.slk") else {
            return;
        };
        // Counted against the file's own bytes rather than a constant: 1.28c
        // carries 2,430 `E` formulas and 1.29b carries none, but the `K` values
        // are identical either way.
        let d = shipped("UI\\SoundInfo\\DialogSounds.slk").expect("shipped");
        let formulas = String::from_utf8_lossy(&d).matches(";ECONCATENATE").count();
        assert_eq!(
            t.generated_cells().len(),
            formulas,
            "every E-bearing record must be reported and no other"
        );
        let cell = t.cell(2, 2).expect("row 2 col 2 is populated");
        assert_eq!(cell.generated, formulas > 0);
        assert_eq!(cell.value, "H01Arthas02.mp3");
        assert_eq!(cell.header.as_deref(), Some("FileNames"));
    }

    /// Two shipped headers carry spaces. Silently trimming would make
    /// `column_of("Priority")` answer for a column that does not exist.
    #[test]
    fn headers_keep_their_spaces() {
        let Some(midi) = shipped_table("UI\\SoundInfo\\MIDISounds.slk") else {
            return;
        };
        assert_eq!(midi.header(6), Some("Priority "));
        assert_eq!(midi.column_of("Priority "), Some(6));
        assert_eq!(midi.column_of("Priority"), None);
        assert_eq!(midi.by_name(2, "SoundLabel"), Some("AshenvaleDay"));
        assert_eq!(midi.by_name(2, "MIDIFileName"), Some("AshenvaleDay.mid"));
        assert_eq!(midi.by_name(2, "DLSFileName"), Some("Ashenvale.dls"));
        assert_eq!(midi.by_name(2, "Priority "), Some("1000"));

        let Some(p) = shipped_table("UI\\SoundInfo\\PortraitAnims.slk") else {
            return;
        };
        assert_eq!(p.header(1), Some("Sound Label"));
        assert_eq!(
            p.header(2),
            Some("Anim Indices (MUST BE PRECEDED BY NON-NUMERIC CHAR)")
        );
        assert_eq!(
            p.header(3),
            Some("Speech Duration (MUST BE PRECEDED BY NON-NUMERIC CHAR)")
        );
        assert_eq!(p.by_name(2, "Sound Label"), Some("MortarTeamPissed"));
        assert_eq!(
            p.by_name(2, "Anim Indices (MUST BE PRECEDED BY NON-NUMERIC CHAR)"),
            Some("_4,3,4,3,3,4,4,4,4")
        );
    }

    /// 46 `FileNames` cells in this table end in a comma, so a plain `split`
    /// hands back an empty trailing element for every one of them.
    #[test]
    fn unit_combat_sound_file_names_end_in_a_trailing_comma() {
        let Some(t) = shipped_table("UI\\SoundInfo\\UnitCombatSounds.slk") else {
            return;
        };
        assert_eq!(t.row_count(), 61);
        assert_eq!(
            t.column_count(),
            13,
            "no EAXFlags/version/InBeta columns here"
        );
        let raw = t.by_name(4, "FileNames").unwrap();
        assert!(
            raw.ends_with(','),
            "row 4 must keep its trailing comma verbatim"
        );
        assert_eq!(t.by_name(4, "SoundName"), Some("MetalHeavyBashFlesh"));
        assert_eq!(
            SoundTable(&t).file_names(4),
            vec![
                "MetalHeavyBashFlesh1.wav",
                "MetalHeavyBashFlesh2.wav",
                "MetalHeavyBashFlesh3.wav"
            ]
        );
    }

    /// `B` declares 61 columns, 60 are named, and column 6 exists in every
    /// data row with no header anywhere.
    #[test]
    fn a_column_can_have_cells_and_no_name() {
        let Some(t) = shipped_table("Units\\UnitBalance.slk") else {
            return;
        };
        assert_eq!(t.declared().unwrap().columns, 61);
        assert_eq!(t.column_count(), 61);
        assert_eq!(t.header(6), None);
        assert_eq!(t.header(5), Some("level"));
        assert_eq!(t.get(2, 6), Some("FALSE"));
        let unnamed = t
            .anomalies()
            .iter()
            .filter(|a| a.kind == AnomalyKind::UnnamedColumn)
            .collect::<Vec<_>>();
        assert_eq!(unnamed.len(), 1);
        assert_eq!(unnamed[0].column, Some(6));
        assert_eq!(unnamed[0].severity, Severity::Warning);
    }

    /// The one oddity in 124 shipped files: three `Splats\\T_SplatData.slk`
    /// records whose payload sits under `A` instead of `K`. The text is an
    /// Excel cell comment, not a value, and the real header arrives a few
    /// records later — so the file decodes completely and the payload is only
    /// reported.
    #[test]
    fn a_payload_under_an_unknown_tag_is_reported_not_guessed() {
        let Some(t) = shipped_table("Splats\\T_SplatData.slk") else {
            eprintln!("skipping: 1.29b dropped this file");
            return;
        };
        let unknown: Vec<&Anomaly> = t
            .anomalies()
            .iter()
            .filter(|a| a.kind == AnomalyKind::CellValueUnknown)
            .collect();
        assert_eq!(unknown.len(), 3);
        let mut columns: Vec<u32> = unknown.iter().filter_map(|a| a.column).collect();
        columns.sort_unstable();
        assert_eq!(columns, vec![7, 11, 12]);
        assert!(
            unknown
                .iter()
                .all(|a| a.row == Some(1) && a.severity == Severity::Warning),
            "a later record supplies each value, so this is an annotation"
        );
        // The comment text must never surface as a cell value.
        for column in columns {
            let v = t
                .header(column)
                .expect("the real header follows the comment");
            assert!(
                !v.contains("Options are"),
                "column {column} leaked a comment"
            );
        }
        assert_eq!(t.header(7), Some("BlendMode"));
        assert_eq!(t.header(11), Some("UVLifespanStart"));
        assert_eq!(t.header(12), Some("UVLifespanEnd"));
    }

    /// `P`, `F` and `O` are Excel presentation records. 1.29b ships none of
    /// them at all, so they must be counted and discarded without being load
    /// bearing.
    #[test]
    fn presentation_records_are_counted_and_discarded() {
        let Some(d) = shipped("Splats\\T_SplatData.slk") else {
            return;
        };
        let t = Table::parse(&d).expect("parses");
        let c = t.record_counts();
        assert_eq!(c.get(&'P'), Some(&124));
        assert_eq!(c.get(&'F'), Some(&1399));
        assert_eq!(c.get(&'O'), Some(&1));
        assert_eq!(c.get(&'C'), Some(&3963));
        assert_eq!(c.get(&'E'), Some(&1));
        // The comment text after `A` is never promoted to a cell value; the
        // header comes from the real `K` record that follows it.
        assert_eq!(t.get(1, 7), Some("BlendMode"));
        assert!(!t.get(1, 7).unwrap().contains("Options are"));
    }

    #[test]
    fn rejects_a_non_slk() {
        assert!(matches!(
            Table::parse(b"ID;PWXL;N;X\r\nB;X1;Y1;D0\r\nE\r\n"),
            Err(Error::BadSignature { .. })
        ));
        assert!(matches!(Table::parse(b""), Err(Error::Empty)));
        assert!(matches!(
            Table::parse(b"ID;PWXL;N;E\r\nZ;X1\r\nE\r\n"),
            Err(Error::NotARecord { letter: b'Z', .. })
        ));
        assert!(matches!(
            Table::parse(b"ID;PWXL;N;E\r\nC;X1;K\"unclosed\r\nE\r\n"),
            Err(Error::UnterminatedQuote { .. })
        ));
        assert!(matches!(
            Table::parse(b"ID;PWXL;N;E\r\nC;X0;Y1;K0\r\nE\r\n"),
            Err(Error::ZeroIndex { .. })
        ));
        assert!(matches!(
            Table::parse(b"ID;PWXL;N;E\r\nC;X1;Y1;K0\r\n"),
            Err(Error::NoTerminator)
        ));
        assert!(matches!(
            Table::parse(b"ID;PWXL;N;E\r\nC;X1;Y1;K0"),
            Err(Error::Truncated { .. })
        ));
        assert!(matches!(
            Table::parse(b"ID;PWXL;N;E\r\nB;X1;Y1;D0\r\nE\r\n"),
            Err(Error::Unsupported { .. })
        ));
    }

    #[test]
    fn quoting_round_trips_semicolons_backslashes_and_escaped_quotes() {
        let src = b"ID;PWXL;N;E\r\nC;X1;Y1;K\"head;er\"\r\nC;X1;Y2;K\"a\\\\b\"\r\nC;X1;Y3;K\"say \"\"hi\"\"\"\r\nE\r\n";
        let t = Table::parse(src).expect("parses");
        assert_eq!(t.get(1, 1), Some("head;er"));
        // Backslash is not an escape in this format, so the two source bytes
        // survive as two value bytes.
        assert_eq!(t.get(2, 1), Some("a\\\\b"));
        assert_eq!(t.get(3, 1), Some("say \"hi\""));
    }

    /// `slqx hash` exists so an independent decoder can be compared byte for
    /// byte. Pin the canonical form and the digest it produces.
    #[test]
    fn the_canonical_form_is_stable_and_separates_every_cell() {
        let t = shipped_table("UI\\SoundInfo\\EnvironmentSounds.slk");
        let Some(t) = t else { return };
        let canon = t.canonical();
        let text = String::from_utf8(canon.clone()).expect("ascii");
        assert!(text.starts_with("Y1;X1;KEnvironmentType\nY1;X2;KRoomType\n"));
        assert_eq!(text.lines().count(), t.cell_count());
        assert!(text.ends_with("Y5;X17;KWESTRING_SNDENV_DUNGEON\n"));
        assert_eq!(t.fnv1a64(), fnv1a64(&canon));
        // Reparsing the canonical form is not a round trip (it is not quoted),
        // but it must still hash identically to prove the form is the one
        // documented.
        assert_ne!(t.fnv1a64(), 0);
    }

    /// Every `.slk` the archives actually ship, not a hand-picked subset.
    ///
    /// Ignored by default because it walks the whole archive set; run with
    /// `cargo test -p pcx-slq -- --ignored`.
    #[test]
    #[ignore = "walks the whole shipped .slk set"]
    fn every_shipped_slk_decodes() {
        let Some(dir) = install_dir() else {
            eprintln!("skipping: no extracted install");
            return;
        };
        let mut set = pcx_mpq::MpqSet::open_wc3(&dir).expect("open WC3 archive set");
        let slks: Vec<String> = set
            .file_list()
            .expect("union listfile")
            .into_iter()
            .filter(|l| l.to_ascii_lowercase().ends_with(".slk"))
            .collect();
        assert!(
            slks.len() >= 59,
            "only {} .slk entries; the 1.28c set is 59",
            slks.len()
        );

        let mut unread = Vec::new();
        let mut failed = Vec::new();
        let mut gaps: Vec<String> = Vec::new();
        let mut sound_tables = 0usize;
        let mut cells = 0usize;
        let mut bytes = 0usize;
        let mut by_kind: std::collections::BTreeMap<&str, usize> = Default::default();

        for rel in &slks {
            let d = match set.extract(rel) {
                Ok(d) => d,
                Err(e) => {
                    unread.push(format!("{rel}: {e}"));
                    continue;
                }
            };
            bytes += d.len();
            let t = match Table::parse(&d) {
                Ok(t) => t,
                Err(e) => {
                    failed.push(format!("{rel}: {e}"));
                    continue;
                }
            };
            cells += t.cell_count();
            if rel.to_ascii_lowercase().contains("soundinfo") {
                sound_tables += 1;
            }
            // Every shipped table has row 1 as a header row and contiguous rows.
            if t.row_count() == 0 {
                failed.push(format!("{rel}: no rows"));
            } else if t.header(1).is_none() {
                failed.push(format!("{rel}: row 1 has no cell in column 1"));
            }
            for row in t.rows() {
                if row.index() > 1 && row.is_empty() {
                    failed.push(format!("{rel}: row {} is empty", row.index()));
                }
            }
            for a in t.anomalies() {
                if a.severity == Severity::Info {
                    continue;
                }
                *by_kind
                    .entry(match a.kind {
                        AnomalyKind::CellValueUnknown => "CellValueUnknown",
                        AnomalyKind::CellValueMissing => "CellValueMissing",
                        AnomalyKind::UnnamedColumn => "UnnamedColumn",
                        AnomalyKind::StrayDataAfterEnd => "StrayDataAfterEnd",
                        _ => "other",
                    })
                    .or_default() += 1;
                gaps.push(format!(
                    "{rel}: {:?}/{:?} row {:?} col {:?}: {}",
                    a.kind, a.severity, a.row, a.column, a.detail
                ));
            }
        }

        eprintln!("install      {}", dir.display());
        eprintln!(
            "slk found={} decoded={} failed={} unread={} cells={} bytes={} soundTables={sound_tables}",
            slks.len(),
            slks.len() - failed.len() - unread.len(),
            failed.len(),
            unread.len(),
            cells,
            bytes
        );
        eprintln!("gaps by kind: {by_kind:?}");
        for g in gaps.iter().take(20) {
            eprintln!("  gap  {g}");
        }

        let report = std::path::Path::new("/tmp/opencode/slq-sweep.txt");
        if let Some(dir) = report.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(mut w) = std::fs::File::create(report) {
            use std::io::Write;
            let _ = writeln!(w, "install: {}", dir.display());
            let _ = writeln!(
                w,
                "found={} decoded={} failed={} unread={} cells={} bytes={} soundTables={sound_tables}",
                slks.len(),
                slks.len() - failed.len() - unread.len(),
                failed.len(),
                unread.len(),
                cells,
                bytes
            );
            for (k, v) in &by_kind {
                let _ = writeln!(w, "gap x{v}: {k}");
            }
            for line in failed.iter().chain(unread.iter()).chain(gaps.iter()) {
                let _ = writeln!(w, "{line}");
            }
        }
        eprintln!("full report: {}", report.display());

        assert!(unread.is_empty(), "{} .slk files unreadable", unread.len());
        assert!(
            failed.is_empty(),
            "{} .slk files failed to decode",
            failed.len()
        );
        assert!(
            sound_tables >= 12,
            "expected at least the 12 shipped UI\\SoundInfo tables, saw {sound_tables}"
        );
    }
}

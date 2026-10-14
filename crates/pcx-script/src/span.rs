//! Byte spans and the line/column map built over a script's source.
//!
//! Every token and every AST node carries a [`Span`] measured in **byte**
//! offsets into the UTF-8 source, end-exclusive. Byte offsets are what the
//! lexer actually advances over, so they never need a decode step and never
//! lie about a position.
//!
//! A later VM must be able to report the *map's own* line and column —
//! `PLAN.md` requires a missing native to fail loudly with the map's
//! identifier, not with a synthesized zero — so [`SourceMap`] turns those byte
//! offsets into the 1-based line/column a map author sees in their editor.

use std::fmt;

/// A half-open byte range `[start, end)` in the source.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub const fn new(start: usize, end: usize) -> Span {
        Span {
            start: start as u32,
            end: end as u32,
        }
    }

    pub const EMPTY: Span = Span { start: 0, end: 0 };

    /// Byte length of the covered text.
    pub const fn len(self) -> u32 {
        self.end - self.start
    }

    pub const fn is_empty(self) -> bool {
        self.end <= self.start
    }

    /// Smallest span covering both inputs. Used to give a node the span of
    /// its first through last token.
    pub fn join(self, other: Span) -> Span {
        Span {
            start: if self.start <= other.start {
                self.start
            } else {
                other.start
            },
            end: if self.end >= other.end {
                self.end
            } else {
                other.end
            },
        }
    }

    /// The source text this span covers, or `""` if it does not land on
    /// character boundaries.
    pub fn text(self, src: &str) -> &str {
        let (a, b) = (self.start as usize, self.end as usize);
        if a > b || b > src.len() || !src.is_char_boundary(a) || !src.is_char_boundary(b) {
            return "";
        }
        &src[a..b]
    }

    /// 1-based line and column (in characters) of `self.start`.
    pub fn line_col(self, map: &SourceMap) -> (u32, u32) {
        map.line_col(self.start)
    }
}

impl fmt::Debug for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

/// Byte offsets of every line start, plus a prefix table mapping byte
/// offsets to character offsets, for turning byte offsets into the
/// line/column a map author sees.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceMap {
    line_starts: Vec<u32>,
    /// `chars_before[i]` = number of characters in `src[..i]` at every
    /// character boundary; indexed by byte offset, `usize::MAX` marks
    /// non-boundaries.
    chars_before: Vec<u32>,
    len: u32,
}

impl SourceMap {
    pub fn new(src: &str) -> SourceMap {
        let bytes = src.as_bytes();
        let mut line_starts = Vec::with_capacity(bytes.len() / 40 + 1);
        line_starts.push(0);
        // `\n`, bare `\r` and `\r\n` each end exactly one line, so a CR-only
        // map reports its own line numbers instead of everything being line 1.
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'\r' => {
                    i += 1;
                    if bytes.get(i) == Some(&b'\n') {
                        i += 1;
                    }
                    line_starts.push(i as u32);
                }
                b'\n' => {
                    i += 1;
                    line_starts.push(i as u32);
                }
                _ => i += 1,
            }
        }
        let mut chars_before = vec![u32::MAX; bytes.len() + 1];
        let mut n = 0u32;
        for (i, _) in src.char_indices() {
            chars_before[i] = n;
            n += 1;
        }
        chars_before[bytes.len()] = n;
        SourceMap {
            line_starts,
            chars_before,
            len: bytes.len() as u32,
        }
    }

    pub fn len(&self) -> u32 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    /// 1-based line, 1-based column counted in **characters** — the
    /// coordinates a map author sees in their editor, which is what a
    /// missing-native diagnostic has to report.
    ///
    /// Offsets past the end of the source clamp to the last line rather than
    /// panicking; a lexer that ran off the end should already have errored,
    /// but a diagnostic must never be the thing that crashes the process.
    pub fn line_col(&self, offset: u32) -> (u32, u32) {
        let offset = offset.min(self.len);
        let idx = match self.line_starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => i - 1,
        };
        let line_start = self.line_starts[idx] as usize;
        let (o, ls) = (offset as usize, line_start);
        let col = match (
            self.chars_before.get(o).copied(),
            self.chars_before.get(ls).copied(),
        ) {
            (Some(a), Some(b)) if a != u32::MAX && b != u32::MAX => a - b,
            _ => (o.saturating_sub(ls)) as u32,
        };
        (idx as u32 + 1, 1 + col)
    }

    /// Renders a span as `line:col-line:col`, the form a map author reads.
    pub fn render(&self, span: Span) -> String {
        let (l1, c1) = self.line_col(span.start);
        let (l2, c2) = self.line_col(span.end);
        if l1 == l2 {
            format!("{l1}:{c1}-{c2}")
        } else {
            format!("{l1}:{c1}-{l2}:{c2}")
        }
    }
}

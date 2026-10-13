//! The JASS lexer.
//!
//! # Why this is not a "reasonable" tokenizer
//!
//! The pieces real maps depend on that a textbook tokenizer drops:
//!
//! - **Rawcode literals.** `'hfoo'` is an integer built from four characters.
//!   The shipped `war3map.j` corpus is full of them. A lexer without them
//!   fails on essentially every map. Vanilla also accepts the one-character
//!   spelling `'A'` (its ASCII value).
//! - **Integer bases.** `$AB12` and `0xAB12` hex, `012` octal (yes, octal —
//!   `012` is 10), `%1010` binary where a value is expected, plain decimal.
//!   A leading-`0` number containing 8 or 9 is an error, not decimal.
//! - **Real spellings.** `1.50`, `1.5`, `1.`, `.5`, and exponents. The
//!   lexeme is kept verbatim: `1.50` and `1.5` are the same `f64` but
//!   different map text.
//! - **`^^` code tags inside strings.** Recorded structurally (see
//!   [`StrPart::CodeTag`]) only when *paired*; an unpaired `^^` stays
//!   ordinary text. Shipped maps use `^^` as a smiley at the end of strings,
//!   and a lexer that swallowed the rest of the string on a lone `^^` would
//!   silently corrupt them — the classic breakage this exists to avoid.
//! - **`|n`, `|r`, `|c…` control codes.** These are *ordinary characters* in
//!   a JASS string; the engine interprets them at display time. The lexer
//!   must preserve them byte for byte, which is why every string body is
//!   stored as a slice of the original source rather than a decoded `String`.
//! - **Escapes are strict.** Exactly `\" \\ \b \t \r \n \f` are escapes;
//!   anything else after a backslash is an error, as the community checker
//!   (pjass) and the manual agree. No shipped script uses an unknown escape.
//! - **Newlines inside strings are legal.** Vanilla allows them ("strings
//!   may contain carriage returns without causing any trouble"); pjass
//!   counts them as line breaks. They are preserved, not an error.
//! - **`#` / `//!` / `//#` directive lines.** Tooling emits them; they are
//!   recorded in a side table and are never statements.
//!
//! # Case
//!
//! Vanilla JASS is case-sensitive, and `common.j` proves it: the game's own
//! script declares `constant boolean TRUE = true`, so `TRUE` is an ordinary
//! identifier and only the lowercase words are reserved. Type names,
//! identifiers and rawcodes likewise keep their spelling exactly: name
//! resolution and rawcode values are case-sensitive, and the tree must not
//! fold them. (A task note claiming case-insensitive keywords is contradicted
//! by the shipped `common.j`; the shipped artifact wins.)

use crate::span::Span;

/// Reserved words, matched **exactly** as spelled.
///
/// Vanilla JASS is case-sensitive here: `game.dll`'s own `common.j` declares
/// `constant boolean TRUE = true` and `constant boolean FALSE = false`, so
/// `TRUE` must be an ordinary identifier and only the lowercase words are
/// reserved. pjass and every other community parser agree.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub enum Kw {
    Alias,
    And,
    Array,
    Call,
    Constant,
    Debug,
    Else,
    ElseIf,
    EndFunction,
    EndGlobals,
    EndIf,
    EndLoop,
    ExitWhen,
    Extends,
    False,
    Function,
    Globals,
    If,
    Local,
    Loop,
    Native,
    Not,
    Nothing,
    Null,
    Or,
    Return,
    Returns,
    Set,
    Takes,
    Then,
    True,
    Type,
}

impl Kw {
    /// Every reserved word, lowercase.
    pub const ALL: [Kw; 32] = [
        Kw::Alias,
        Kw::And,
        Kw::Array,
        Kw::Call,
        Kw::Constant,
        Kw::Debug,
        Kw::Else,
        Kw::ElseIf,
        Kw::EndFunction,
        Kw::EndGlobals,
        Kw::EndIf,
        Kw::EndLoop,
        Kw::ExitWhen,
        Kw::Extends,
        Kw::False,
        Kw::Function,
        Kw::Globals,
        Kw::If,
        Kw::Local,
        Kw::Loop,
        Kw::Native,
        Kw::Not,
        Kw::Nothing,
        Kw::Null,
        Kw::Or,
        Kw::Return,
        Kw::Returns,
        Kw::Set,
        Kw::Takes,
        Kw::Then,
        Kw::True,
        Kw::Type,
    ];

    pub fn text(self) -> &'static str {
        match self {
            Kw::Alias => "alias",
            Kw::And => "and",
            Kw::Array => "array",
            Kw::Call => "call",
            Kw::Constant => "constant",
            Kw::Debug => "debug",
            Kw::Else => "else",
            Kw::ElseIf => "elseif",
            Kw::EndFunction => "endfunction",
            Kw::EndGlobals => "endglobals",
            Kw::EndIf => "endif",
            Kw::EndLoop => "endloop",
            Kw::ExitWhen => "exitwhen",
            Kw::Extends => "extends",
            Kw::False => "false",
            Kw::Function => "function",
            Kw::Globals => "globals",
            Kw::If => "if",
            Kw::Local => "local",
            Kw::Loop => "loop",
            Kw::Native => "native",
            Kw::Not => "not",
            Kw::Nothing => "nothing",
            Kw::Null => "null",
            Kw::Or => "or",
            Kw::Return => "return",
            Kw::Returns => "returns",
            Kw::Set => "set",
            Kw::Takes => "takes",
            Kw::Then => "then",
            Kw::True => "true",
            Kw::Type => "type",
        }
    }

    /// Exact lookup: vanilla keywords are case-sensitive.
    pub fn lookup(word: &str) -> Option<Kw> {
        Kw::ALL.into_iter().find(|k| k.text() == word)
    }
}

/// Base an integer literal was written in. The value is kept as `i64`; the
/// engine's 32-bit wraparound is a VM-stage decision, not a syntax one.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum IntBase {
    Dec,
    Hex,
    Octal,
    Bin,
}

/// An integer literal, normalised to its value plus the base it was written
/// in. `255`, `$FF` and `0xFF` are one value in three spellings; keeping the
/// base means the printer can hand back a faithful form.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct IntLit {
    pub value: i64,
    pub base: IntBase,
}

impl IntLit {
    pub fn dec(value: i64) -> IntLit {
        IntLit {
            value,
            base: IntBase::Dec,
        }
    }
}

/// A real literal.
///
/// The original lexeme is kept because it is the only lossless form: `1.50`,
/// `1.5` and `15e-1` are the same `f64` but different maps' text, and a
/// canonical hash that normalized them would hide a real edit. `value` is the
/// parse-time `f64`, provided for callers that want it; **nothing in this
/// crate hashes or prints the `f64`**, so there is no float-formatting
/// instability.
#[derive(Clone, PartialEq, Debug)]
pub struct RealLit {
    pub text: String,
    pub value: f64,
}

impl RealLit {
    pub fn as_str(&self) -> &str {
        &self.text
    }
}

/// A backslash escape JASS defines. Anything else after a backslash is a
/// lex error: the community checker rejects it and no shipped script needs
/// it, so passing it through would be a silent wrong-accept.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Esc {
    /// `\"`
    Quote,
    /// `\\`
    Backslash,
    /// `\b`
    Bsp,
    /// `\t`
    Tab,
    /// `\r`
    Cr,
    /// `\n`
    Nl,
    /// `\f`
    Ff,
}

impl Esc {
    pub fn spelling(self) -> &'static str {
        match self {
            Esc::Quote => "\\\"",
            Esc::Backslash => "\\\\",
            Esc::Bsp => "\\b",
            Esc::Tab => "\\t",
            Esc::Cr => "\\r",
            Esc::Nl => "\\n",
            Esc::Ff => "\\f",
        }
    }

    pub fn from_char(c: char) -> Option<Esc> {
        Some(match c {
            '"' => Esc::Quote,
            '\\' => Esc::Backslash,
            'b' => Esc::Bsp,
            't' => Esc::Tab,
            'r' => Esc::Cr,
            'n' => Esc::Nl,
            'f' => Esc::Ff,
            _ => return None,
        })
    }

    /// The character the escape stands for.
    pub fn decoded(self) -> char {
        match self {
            Esc::Quote => '"',
            Esc::Backslash => '\\',
            Esc::Bsp => '\u{8}',
            Esc::Tab => '\t',
            Esc::Cr => '\r',
            Esc::Nl => '\n',
            Esc::Ff => '\u{c}',
        }
    }
}

/// One piece of a string literal, kept structurally so nothing is lost.
#[derive(Clone, PartialEq, Eq, Debug, Hash)]
pub enum StrPart {
    /// Ordinary characters, stored exactly as they appear in the source
    /// including any `|n` / `|r` / `|cff0000ff` control codes and any
    /// unpaired `^^`.
    Text(String),
    /// A defined backslash escape. Re-emitted with its original spelling.
    Escape(Esc),
    /// A paired `^^…^^` code tag. `text` is the payload *between* the
    /// markers; the markers themselves are not stored and are re-emitted by
    /// the printer. An unpaired `^^` is ordinary text and lands in
    /// [`StrPart::Text`] instead.
    ///
    /// What the engine substitutes for a code tag is deliberately **not**
    /// decided here; that is a VM-stage question and guessing would be
    /// exactly the "plausible but wrong" failure this crate exists to avoid.
    CodeTag { text: String },
}

impl StrPart {
    /// The characters this part contributes, for callers that want the
    /// literal text. Escapes are decoded; code tags contribute nothing.
    pub fn decoded(&self, out: &mut String) {
        match self {
            StrPart::Text(t) => out.push_str(t),
            StrPart::Escape(e) => out.push(e.decoded()),
            StrPart::CodeTag { .. } => {}
        }
    }
}

/// A string literal, stored as its parts.
#[derive(Clone, PartialEq, Eq, Debug, Hash)]
pub struct StrLit {
    pub parts: Vec<StrPart>,
}

impl StrLit {
    /// The decoded characters, with escapes resolved and code tags dropped.
    /// Control codes inside `StrPart::Text` are left alone: they are
    /// characters the engine interprets, not text we should normalize.
    pub fn decoded(&self) -> String {
        let mut s = String::new();
        for p in &self.parts {
            p.decoded(&mut s);
        }
        s
    }
}

/// A `'hfoo'` rawcode — or the one-character `'A'` spelling, or the
/// single-escape `'\n'` spelling. The interior is kept exactly as written,
/// because its case and order *are* the value.
#[derive(Clone, PartialEq, Eq, Debug, Hash)]
pub struct RawLit {
    pub text: String,
}

/// An identifier. `name` keeps the map's own spelling; vanilla name lookup is
/// case-sensitive, so nothing here folds case.
#[derive(Clone, PartialEq, Eq, Debug, Hash)]
pub struct Ident {
    pub name: String,
    /// The token's byte span in the source. Name resolution reports a use
    /// against this, so an unresolved name carries the map's own position
    /// instead of the start of its statement.
    pub span: Span,
}

impl Ident {
    pub fn new(name: impl Into<String>) -> Ident {
        Ident {
            name: name.into(),
            span: Span::EMPTY,
        }
    }

    /// The spelling with its span, the form the parser builds tokens into.
    pub fn at(name: impl Into<String>, span: Span) -> Ident {
        Ident {
            name: name.into(),
            span,
        }
    }

    pub fn as_str(&self) -> &str {
        &self.name
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

impl std::ops::Deref for Ident {
    type Target = str;
    fn deref(&self) -> &str {
        &self.name
    }
}

/// A directive line: JASSHelper's `//! …`, the `//# …` annotation form, or a
/// bare `#name …` line.
///
/// The lexer cannot know what a directive *means* — that is a preprocessing
/// question — so it records the directive name and its arguments verbatim and
/// the tree keeps them in a side table.
#[derive(Clone, PartialEq, Eq, Debug, Hash)]
pub struct Directive {
    pub name: String,
    pub args: String,
    /// The whole directive line, without the newline, exactly as written.
    pub text: String,
}

/// A directive together with where it sat. Kept apart from [`Directive`]
/// itself so the token stays a plain value that compares equal regardless of
/// position, which is what makes the span-stripped token comparison in the
/// round-trip check meaningful.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DirectiveRecord {
    pub directive: Directive,
    pub span: Span,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Tok {
    Kw(Kw),
    Ident(Ident),
    Int(IntLit),
    Real(RealLit),
    Str(StrLit),
    Raw(RawLit),
    Directive(Directive),
    LParen,
    RParen,
    LBracket,
    RBracket,
    Comma,
    Plus,
    Minus,
    Star,
    Slash,
    /// Bare `%` in operator position: integer modulus. In operand position a
    /// `%` followed by binary digits is a [`IntBase::Bin`] literal instead.
    Percent,
    /// `=`
    Assign,
    /// `==`
    EqEq,
    /// `!=`
    BangEq,
    Lt,
    Le,
    Gt,
    Ge,
    Eof,
}

impl Tok {
    pub fn is_kw(&self, k: Kw) -> bool {
        matches!(self, Tok::Kw(x) if *x == k)
    }

    pub fn ident(&self) -> Option<&Ident> {
        match self {
            Tok::Ident(i) => Some(i),
            _ => None,
        }
    }

    /// A short stable label used by `scriptx lex` and by error messages.
    pub fn label(&self) -> String {
        match self {
            Tok::Kw(k) => format!("kw:{}", k.text()),
            Tok::Ident(i) => format!("ident:{}", i.name),
            Tok::Int(v) => format!("int:{}", v.value),
            Tok::Real(v) => format!("real:{}", v.text),
            Tok::Str(_) => "string".into(),
            Tok::Raw(v) => format!("rawcode:'{}'", v.text),
            Tok::Directive(d) => format!("directive:#{}", d.name),
            Tok::LParen => "(".into(),
            Tok::RParen => ")".into(),
            Tok::LBracket => "[".into(),
            Tok::RBracket => "]".into(),
            Tok::Comma => ",".into(),
            Tok::Plus => "+".into(),
            Tok::Minus => "-".into(),
            Tok::Star => "*".into(),
            Tok::Slash => "/".into(),
            Tok::Percent => "%".into(),
            Tok::Assign => "=".into(),
            Tok::EqEq => "==".into(),
            Tok::BangEq => "!=".into(),
            Tok::Lt => "<".into(),
            Tok::Le => "<=".into(),
            Tok::Gt => ">".into(),
            Tok::Ge => ">=".into(),
            Tok::Eof => "<eof>".into(),
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Token {
    pub kind: Tok,
    pub span: Span,
}

impl Token {
    pub fn is_kw(&self, k: Kw) -> bool {
        self.kind.is_kw(k)
    }

    pub fn is(&self, t: &Tok) -> bool {
        &self.kind == t
    }
}

/// A comment, kept in a side table rather than the tree. Community maps are
/// debugged through their comments; dropping them would lose the map author's
/// own explanation of every block.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Comment {
    pub kind: CommentKind,
    /// Text between the delimiters, without them.
    pub text: String,
    pub span: Span,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum CommentKind {
    Line,
    Block,
}

/// The lexer's full output: tokens, comments and directives, each with spans,
/// in source order within its own list.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Lexed {
    pub tokens: Vec<Token>,
    pub comments: Vec<Comment>,
    pub directives: Vec<DirectiveRecord>,
}

pub fn lex(src: &str) -> Result<Lexed, crate::Error> {
    Lexer::new(src).run()
}

struct Lexer<'a> {
    src: &'a str,
    b: &'a [u8],
    i: usize,
    tokens: Vec<Token>,
    comments: Vec<Comment>,
    directives: Vec<DirectiveRecord>,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str) -> Lexer<'a> {
        Lexer {
            src,
            b: src.as_bytes(),
            i: 0,
            tokens: Vec::new(),
            comments: Vec::new(),
            directives: Vec::new(),
        }
    }

    fn run(mut self) -> Result<Lexed, crate::Error> {
        loop {
            self.skip_trivia()?;
            if self.i >= self.b.len() {
                break;
            }
            let start = self.i;
            let kind = self.next_token()?;
            self.tokens.push(Token {
                kind,
                span: Span::new(start, self.i),
            });
        }
        let end = self.b.len();
        self.tokens.push(Token {
            kind: Tok::Eof,
            span: Span::new(end, end),
        });
        Ok(Lexed {
            tokens: self.tokens,
            comments: self.comments,
            directives: self.directives,
        })
    }

    fn err(&self, span: Span, message: impl Into<String>) -> crate::Error {
        crate::Error::lex(span, message)
    }

    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }

    fn peek_at(&self, n: usize) -> Option<u8> {
        self.b.get(self.i + n).copied()
    }

    /// Whitespace and comments. Everything skipped is recorded.
    fn skip_trivia(&mut self) -> Result<(), crate::Error> {
        loop {
            while let Some(c) = self.peek() {
                if c == b' ' || c == b'\t' || c == b'\r' || c == b'\n' || c == 0x0b || c == 0x0c {
                    self.i += 1;
                } else {
                    break;
                }
            }
            if self.peek() == Some(0xEF)
                && self.peek_at(1) == Some(0xBB)
                && self.peek_at(2) == Some(0xBF)
            {
                // UTF-8 BOM, which shipped string tables do carry. It is
                // trivia, as in the community checker.
                self.i += 3;
                continue;
            }
            if self.peek() == Some(b'/') && self.peek_at(1) == Some(b'/') {
                let start = self.i;
                self.i += 2;
                // `//! …` (JASSHelper) and `//# …` (annotation) lines are
                // directives, not plain comments; they are recorded in the
                // directive table instead so tooling sees them.
                let marker = matches!(self.peek(), Some(b'!') | Some(b'#'));
                if marker {
                    self.i += 1; // the `!` or `#` after `//`
                    let text_start = start;
                    while !matches!(self.peek(), Some(b'\n') | Some(b'\r') | None) {
                        self.i += 1;
                    }
                    let text = self.src[text_start..self.i].trim_end().to_string();
                    self.push_directive_from_body(&text, start);
                    continue;
                }
                let text_start = self.i;
                while let Some(c) = self.peek() {
                    // A `\r` immediately before the `\n` belongs to the line
                    // ending, not to the comment text, so a CRLF file does not
                    // grow a stray `\r` on every line.
                    if c == b'\n' || c == b'\r' {
                        break;
                    }
                    self.i += 1;
                }
                self.comments.push(Comment {
                    kind: CommentKind::Line,
                    text: self.src[text_start..self.i].to_string(),
                    span: Span::new(start, self.i),
                });
                continue;
            }
            if self.peek() == Some(b'/') && self.peek_at(1) == Some(b'*') {
                let start = self.i;
                self.i += 2;
                let text_start = self.i;
                loop {
                    if self.i >= self.b.len() {
                        return Err(self.err(Span::new(start, self.i), "unterminated `/*` comment"));
                    }
                    if self.peek() == Some(b'*') && self.peek_at(1) == Some(b'/') {
                        let text_end = self.i;
                        self.i += 2;
                        self.comments.push(Comment {
                            kind: CommentKind::Block,
                            text: self.src[text_start..text_end].to_string(),
                            span: Span::new(start, self.i),
                        });
                        break;
                    }
                    self.i += 1;
                }
                continue;
            }
            return Ok(());
        }
    }

    /// Split the body of a directive line into name + args and record it.
    /// `text` is the full line without the trailing newline.
    fn push_directive_from_body(&mut self, text: &str, start: usize) {
        let body = text
            .trim_start_matches('/')
            .trim_start_matches('!')
            .trim_start_matches('#');
        let body = body.trim_start();
        let mut name_end = 0;
        for (n, c) in body.char_indices() {
            if c == ' ' || c == '\t' {
                name_end = n;
                break;
            }
            name_end = n + c.len_utf8();
        }
        let name = body[..name_end].to_ascii_lowercase();
        let args = body[name_end..].trim().to_string();
        self.directives.push(DirectiveRecord {
            directive: Directive {
                name,
                args,
                text: text.to_string(),
            },
            span: Span::new(start, self.i),
        });
    }

    fn next_token(&mut self) -> Result<Tok, crate::Error> {
        let start = self.i;
        let c = self.peek().expect("caller checked for input");

        if c.is_ascii_alphabetic() {
            return Ok(self.lex_word());
        }
        if c.is_ascii_digit() {
            return self.lex_number(start);
        }
        match c {
            b'"' => self.lex_string(start),
            b'\'' => self.lex_rawcode(start),
            b'$' => self.lex_radix(start, 16, "$"),
            b'%' => {
                // A binary literal can only begin where an expression can; in
                // `a % 10` the `%` is the modulus operator.
                if self.peek_at(1).is_some_and(|d| d == b'0' || d == b'1')
                    && !self.prev_ends_expression()
                {
                    self.lex_radix(start, 2, "%")
                } else {
                    self.i += 1;
                    Ok(Tok::Percent)
                }
            }
            b'#' => self.lex_directive(start),
            b'.' => {
                if self.peek_at(1).is_some_and(|d| d.is_ascii_digit()) {
                    self.lex_number(start)
                } else {
                    self.i += 1;
                    Err(self.err(
                        Span::new(start, self.i),
                        "unexpected `.` (vanilla JASS has no member access or `..`)",
                    ))
                }
            }
            b'(' => self.single(Tok::LParen),
            b')' => self.single(Tok::RParen),
            b'[' => self.single(Tok::LBracket),
            b']' => self.single(Tok::RBracket),
            b',' => self.single(Tok::Comma),
            b'+' => self.single(Tok::Plus),
            b'-' => self.single(Tok::Minus),
            b'*' => self.single(Tok::Star),
            b'/' => self.single(Tok::Slash),
            b'=' => {
                if self.peek_at(1) == Some(b'=') {
                    self.i += 2;
                    Ok(Tok::EqEq)
                } else {
                    self.single(Tok::Assign)
                }
            }
            b'!' => {
                if self.peek_at(1) == Some(b'=') {
                    self.i += 2;
                    Ok(Tok::BangEq)
                } else {
                    self.i += 1;
                    Err(self.err(
                        Span::new(start, self.i),
                        "unexpected `!` (JASS spells negation `not`)",
                    ))
                }
            }
            b'<' => {
                if self.peek_at(1) == Some(b'=') {
                    self.i += 2;
                    Ok(Tok::Le)
                } else {
                    self.single(Tok::Lt)
                }
            }
            b'>' => {
                if self.peek_at(1) == Some(b'=') {
                    self.i += 2;
                    Ok(Tok::Ge)
                } else {
                    self.single(Tok::Gt)
                }
            }
            _ => Err(self.err(
                Span::new(start, start + 1),
                format!("unexpected byte 0x{:02x} (`{}`)", c, c as char),
            )),
        }
    }

    fn single(&mut self, t: Tok) -> Result<Tok, crate::Error> {
        self.i += 1;
        Ok(t)
    }

    /// True when the previous token could end an expression, so the current
    /// token is in operand position rather than operator position. This is
    /// what lets `%` be both the modulus operator and the binary-literal
    /// prefix without ambiguity.
    fn prev_ends_expression(&self) -> bool {
        for t in self.tokens.iter().rev() {
            return match &t.kind {
                Tok::Ident(_)
                | Tok::Int(_)
                | Tok::Real(_)
                | Tok::Str(_)
                | Tok::Raw(_)
                | Tok::RParen
                | Tok::RBracket => true,
                Tok::Kw(k) => matches!(k, Kw::True | Kw::False | Kw::Null | Kw::Nothing),
                _ => false,
            };
        }
        false
    }

    fn lex_word(&mut self) -> Tok {
        let start = self.i;
        while self
            .peek()
            .is_some_and(|c| c == b'_' || c.is_ascii_alphanumeric())
        {
            self.i += 1;
        }
        let word = &self.src[start..self.i];
        let span = Span::new(start, self.i);
        match Kw::lookup(word) {
            Some(k) => Tok::Kw(k),
            None => Tok::Ident(Ident::at(word, span)),
        }
    }

    /// `$…` hex and `%…` binary share one shape.
    fn lex_radix(&mut self, start: usize, radix: u32, prefix: &str) -> Result<Tok, crate::Error> {
        self.i += 1; // prefix
        let digits_start = self.i;
        while self.peek().is_some_and(|c| match radix {
            16 => c.is_ascii_hexdigit(),
            _ => c == b'0' || c == b'1',
        }) {
            self.i += 1;
        }
        if self.i == digits_start {
            return Err(self.err(
                Span::new(start, self.i),
                format!("`{prefix}` must be followed by digits"),
            ));
        }
        let digits = &self.src[digits_start..self.i];
        let value = i64::from_str_radix(digits, radix)
            .map_err(|e| self.err(Span::new(start, self.i), format!("integer literal: {e}")))?;
        Ok(Tok::Int(IntLit {
            value,
            base: if radix == 16 {
                IntBase::Hex
            } else {
                IntBase::Bin
            },
        }))
    }

    /// Decimal, `0x` hex, leading-`0` octal, and real literals (including
    /// the `1.` / `.5` shapes and exponents).
    fn lex_number(&mut self, start: usize) -> Result<Tok, crate::Error> {
        // `0x` hex, the spelling the manual and the community checker use
        // alongside `$`.
        if self.peek() == Some(b'0') && matches!(self.peek_at(1), Some(b'x') | Some(b'X')) {
            self.i += 2;
            let digits_start = self.i;
            while self.peek().is_some_and(|c| c.is_ascii_hexdigit()) {
                self.i += 1;
            }
            if self.i == digits_start {
                return Err(self.err(
                    Span::new(start, self.i),
                    "`0x` must be followed by hex digits",
                ));
            }
            let value = i64::from_str_radix(&self.src[digits_start..self.i], 16)
                .map_err(|e| self.err(Span::new(start, self.i), format!("integer literal: {e}")))?;
            return Ok(Tok::Int(IntLit {
                value,
                base: IntBase::Hex,
            }));
        }

        let mut is_real = false;
        // `.5` — the fraction starts immediately; otherwise integer part and
        // an optional fraction. `1.` is a real even with no fraction digits.
        if self.peek() == Some(b'.') {
            is_real = true;
            self.i += 1;
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.i += 1;
            }
        } else {
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.i += 1;
            }
            if self.peek() == Some(b'.') {
                is_real = true;
                self.i += 1;
                while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                    self.i += 1;
                }
            }
        }

        // Exponent. `1e3`, `1E-3`, `1.5e+2`. A bare `e` with no digits is not
        // an exponent and the literal simply ends there.
        if matches!(self.peek(), Some(b'e') | Some(b'E')) {
            let save = self.i;
            self.i += 1;
            if matches!(self.peek(), Some(b'+') | Some(b'-')) {
                self.i += 1;
            }
            if self.peek().is_some_and(|c| c.is_ascii_digit()) {
                is_real = true;
                while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                    self.i += 1;
                }
            } else {
                self.i = save;
            }
        }

        let text = &self.src[start..self.i];
        if is_real {
            let value: f64 = text.parse().map_err(|e| {
                self.err(
                    Span::new(start, self.i),
                    format!("real literal `{text}`: {e}"),
                )
            })?;
            if !value.is_finite() {
                return Err(self.err(
                    Span::new(start, self.i),
                    format!("real literal `{text}` is not finite"),
                ));
            }
            Ok(Tok::Real(RealLit {
                text: text.to_string(),
                value,
            }))
        } else {
            self.lex_int_text(start, text)
        }
    }

    /// Integer value for a plain digit run: decimal, or octal when it starts
    /// with `0`. A leading-`0` run containing 8 or 9 is an error — the
    /// community checker and the manual agree, and silently reading `08` as
    /// decimal would be a wrong value in exactly the maps that use it.
    fn lex_int_text(&mut self, start: usize, text: &str) -> Result<Tok, crate::Error> {
        let octal = text.len() > 1 && text.starts_with('0');
        if octal {
            if text.bytes().any(|c| c == b'8' || c == b'9') {
                return Err(self.err(
                    Span::new(start, self.i),
                    "invalid digit in octal integer notation",
                ));
            }
            let value = i64::from_str_radix(text, 8)
                .map_err(|e| self.err(Span::new(start, self.i), format!("integer literal: {e}")))?;
            Ok(Tok::Int(IntLit {
                value,
                base: IntBase::Octal,
            }))
        } else {
            let value = text
                .parse::<i64>()
                .map_err(|e| self.err(Span::new(start, self.i), format!("integer literal: {e}")))?;
            Ok(Tok::Int(IntLit {
                value,
                base: IntBase::Dec,
            }))
        }
    }

    fn lex_string(&mut self, start: usize) -> Result<Tok, crate::Error> {
        self.i += 1; // opening quote
        let mut parts: Vec<StrPart> = Vec::new();
        let mut run_start = self.i;
        loop {
            let Some(c) = self.peek() else {
                return Err(self.err(Span::new(start, self.i), "unterminated string"));
            };
            match c {
                b'"' => {
                    self.push_text(&mut parts, run_start, self.i);
                    self.i += 1;
                    return Ok(Tok::Str(StrLit { parts }));
                }
                // A newline inside a string is legal in vanilla and is kept
                // as ordinary text.
                b'\\' => {
                    self.push_text(&mut parts, run_start, self.i);
                    self.i += 1;
                    let esc_start = self.i;
                    let Some(next) = self.next_ch() else {
                        return Err(self.err(Span::new(start, self.i), "unterminated string"));
                    };
                    match Esc::from_char(next) {
                        Some(e) => {
                            self.i += next.len_utf8();
                            parts.push(StrPart::Escape(e));
                        }
                        None => {
                            return Err(self.err(
                                Span::new(start, esc_start + next.len_utf8()),
                                format!(
                                    "invalid escape character sequence `\\{next}` (JASS defines \\\" \\\\ \\b \\t \\r \\n \\f)"
                                ),
                            ));
                        }
                    }
                    run_start = self.i;
                }
                0 => {
                    return Err(self.err(
                        Span::new(start, self.i + 1),
                        "NUL not allowed in string literal",
                    ));
                }
                0xff => {
                    return Err(self.err(
                        Span::new(start, self.i + 1),
                        "byte \\xFF not allowed in string literal",
                    ));
                }
                b'^' if self.peek_at(1) == Some(b'^') => {
                    // A `^^` is a code tag only when a closing `^^` arrives
                    // before the string's end quote; otherwise it is literal
                    // text. Shipped maps end strings with smiley `^^`, and
                    // swallowing the rest of the string there is the classic
                    // silent breakage this rule exists to prevent.
                    let close = self.find_code_tag_close();
                    if let Some(close) = close {
                        self.push_text(&mut parts, run_start, self.i);
                        self.i += 2;
                        let payload = self.src[self.i..close].to_string();
                        self.i = close + 2;
                        parts.push(StrPart::CodeTag { text: payload });
                        run_start = self.i;
                    } else {
                        self.i += 2;
                    }
                }
                _ => self.i += utf8_len(c),
            }
        }
    }

    /// Offset of the `^^` that closes a code tag opened at the current
    /// position, or `None` when no closing marker exists before the string's
    /// end quote or a line break.
    fn find_code_tag_close(&self) -> Option<usize> {
        let mut j = self.i + 2;
        while j < self.b.len() {
            match self.b[j] {
                b'"' | b'\n' | b'\r' => return None,
                b'^' if j + 1 < self.b.len() && self.b[j + 1] == b'^' => return Some(j),
                _ => j += 1,
            }
        }
        None
    }

    fn push_text(&self, parts: &mut Vec<StrPart>, start: usize, end: usize) {
        if start < end {
            parts.push(StrPart::Text(self.src[start..end].to_string()));
        }
    }

    fn next_ch(&self) -> Option<char> {
        self.src[self.i..].chars().next()
    }

    fn lex_rawcode(&mut self, start: usize) -> Result<Tok, crate::Error> {
        self.i += 1; // opening quote
        let text_start = self.i;
        // Single-escape spelling: `'\n'` and friends, one escape and out.
        if self.peek() == Some(b'\\') && self.peek_at(2) == Some(b'\'') {
            let esc = self.peek_at(1).unwrap() as char;
            match Esc::from_char(esc) {
                Some(_) => {
                    self.i += 3;
                    return Ok(Tok::Raw(RawLit {
                        text: self.src[text_start..self.i - 1].to_string(),
                    }));
                }
                None => {
                    self.i += 2;
                    return Err(self.err(
                        Span::new(start, self.i),
                        format!("invalid escape character sequence `\\{esc}` in rawcode"),
                    ));
                }
            }
        }
        while let Some(c) = self.peek() {
            if c == b'\'' {
                break;
            }
            if c == b'\n' || c == b'\r' {
                return Err(self.err(Span::new(start, self.i), "newline inside rawcode literal"));
            }
            if c == 0 {
                return Err(self.err(Span::new(start, self.i + 1), "NUL not allowed in rawcode"));
            }
            if c == 0xff {
                return Err(self.err(
                    Span::new(start, self.i + 1),
                    "byte \\xFF not allowed in rawcode",
                ));
            }
            self.i += utf8_len(c);
        }
        if self.peek() != Some(b'\'') {
            return Err(self.err(Span::new(start, self.i), "unterminated rawcode literal"));
        }
        let text = self.src[text_start..self.i].to_string();
        self.i += 1;
        // Vanilla rawcodes are one character or exactly four bytes.
        let bytes = text.len();
        let chars = text.chars().count();
        if !((chars == 1 && bytes == 1) || (chars == 4 && bytes == 4)) {
            return Err(self.err(
                Span::new(start, self.i),
                format!(
                    "rawcode literal must be 1 or 4 ASCII characters, got `{}`",
                    text.escape_debug()
                ),
            ));
        }
        Ok(Tok::Raw(RawLit { text }))
    }

    /// A bare `#name args` directive line.
    fn lex_directive(&mut self, start: usize) -> Result<Tok, crate::Error> {
        let text_start = self.i;
        while !matches!(self.peek(), Some(b'\n') | Some(b'\r') | None) {
            self.i += 1;
        }
        let text = self.src[text_start..self.i].trim_end().to_string();
        let body = text.trim_start_matches('#').trim_start();
        let name_end = body
            .char_indices()
            .find(|(_, c)| *c == ' ' || *c == '\t')
            .map(|(n, _)| n)
            .unwrap_or(body.len());
        if name_end == 0 {
            return Err(self.err(
                Span::new(start, self.i),
                "`#` must be followed by a directive name",
            ));
        }
        let name = body[..name_end].to_ascii_lowercase();
        let args = body[name_end..].trim().to_string();
        let d = Directive {
            name,
            args,
            text: text.clone(),
        };
        self.directives.push(DirectiveRecord {
            directive: d.clone(),
            span: Span::new(start, self.i),
        });
        Ok(Tok::Directive(d))
    }
}

/// UTF-8 length of the character starting with `c`.
fn utf8_len(c: u8) -> usize {
    if c < 0x80 {
        1
    } else if c < 0xE0 {
        2
    } else if c < 0xF0 {
        3
    } else {
        4
    }
}

//! JASS for Warcraft III maps: lexer, parser, name resolution, value model and
//! an interpreter with no game state.
//!
//! This crate is the raw-JASS half of `PLAN.md` §3.11 / M7. A community map's
//! hand-written `war3map.j` (DotA is this path) has to reach the same engine as
//! the editor's trigger graph (Athena is that path), and the raw-JASS half
//! starts here.
//!
//! # The layers, and where each one stops
//!
//! | layer | module | stops at |
//! |---|---|---|
//! | syntax | [`lexer`], [`parser`], [`ast`], [`mod@print`], [`canon`] | an exact tree, every node spanned |
//! | resolution | [`mod@resolve`] | one flat environment, three checks |
//! | values | [`value`] | `null`, boolean, integer, real, string, `code` |
//! | execution | [`eval`] | statements and expressions, no natives but two |
//! | text seams | [`text`], [`eval::Host`] | the caller supplies `war3map.wts` |
//!
//! A layer that guessed at what the map meant would be worse than one that
//! refused: the loss is invisible and the result is *plausible but wrong*, the
//! failure mode this project refuses. So the interpreter runs what it can prove
//! and refuses the rest with the map's identifier and span, and
//! [`eval::Host`] is the seam a host fills in.
//!
//! # What "exact" means
//!
//! - Every token and every node carries a [`Span`], and [`SourceMap`] turns
//!   those byte offsets into the map's own 1-based line and column. An
//!   unresolved name, a missing native and an unimplemented native are all
//!   reported against the map's identifier and position, as `PLAN.md` requires,
//!   instead of silently returning zero.
//! - Parenthesisation is a node, so nothing is re-derived from precedence.
//! - String literals keep their escapes and `^^` code tags structurally;
//!   `|n` / `|r` / `|c…` control codes stay byte-for-byte in the text.
//! - Comments and preprocessor directives are kept in side tables with spans.
//! - Case is preserved everywhere: vanilla JASS is case-sensitive.
//!
//! # Resolution, and what a load list is
//!
//! A map script is never compiled alone. The game loads the Blizzard scripts
//! beside it (`Scripts\common.j` carries 965 natives on this machine) and then
//! the map's `war3map.j`; an AI script is compiled with `common.ai`.
//! [`resolve_with`] takes those sources as a prelude, so a name the map uses and
//! nothing declares is a genuine finding rather than an artifact of resolving
//! the script in isolation.
//!
//! The checks are only the ones the engine's own compiler performs: an unknown
//! type, a wrong argument count where the declaration gives the arity, and
//! `library` `requires` / `uses` ordering. JASS has no static type system, so
//! none is invented here: mixed `real` / `integer` operands and string
//! comparisons resolve, and the value model decides what they mean.
//!
//! # Round-trip
//!
//! [`Script::to_source`] prints the tree back to JASS, and the test suite
//! proves `parse(s).to_source()` re-parses to an identical tree (compared on
//! the span-free canonical encoding) and re-lexes to identical token kinds,
//! checked on hand-written corner cases *and* on every real script on this
//! machine. [`Script::hash`] is FNV-1a 64 over that canonical form.
//!
//! # Corpus this was built against
//!
//! Every real JASS source on this machine: 1326 files, all parsed and
//! round-tripped, 1311 of them resolving with no unresolved name.
//!
//! - 462 install scripts (`Scripts\*.j`, `*.ai`) out of the MPQ archives of
//!   both installs here, the 1.28c tree under `wc3/game/app` and the 1.29b
//!   Legacy install.
//! - 191 `war3map.j` files from the shipped `.w3m` maps of both installs.
//! - 676 `war3map.j` files from the community `.w3x` maps dropped into those
//!   Maps trees (673 UTF-8 plus the 3 non-UTF-8 ones).
//! - 3 `war3map.j` files from the embedded maps of `DemoCampaign.w3n`.
//!
//! DotA Allstars v5.84b and v6.00, Footman Frenzy and Footman Wars are in that
//! corpus. The `#[ignore]`d sweep carries the numbers; the four constructs it
//! documents as unresolved are listed next to it.
//!
//! # Deliberate refusals
//!
//! - **Natives.** Two are implemented, `I2R` and `R2I`, both proved from
//!   `Blizzard.j`'s `ModuloReal` rather than from their names. Everything else
//!   errors with the native's name, its declaration and the span of the call,
//!   which is what `PLAN.md` requires of a missing native. The formatting
//!   natives are refused because their exact output is not derivable here.
//! - **Handles.** Opaque non-null identity tokens can be supplied by the host.
//!   Allocation, lifetime, recycling and native numeric IDs belong to the host;
//!   the interpreter does not invent them.
//! - **vJass lowering.** `struct` / `library` *rewriting* is JASSHelper's pass.
//!   An allocate call `T.create()` reaches resolution as a call to the type's
//!   own name and is refused naming the rewrite; `method`, `cast`, `delegate`
//!   and Zinc are refused by the parser.
//! - **`^^` code tags and `TRIGSTR_`** are host seams ([`eval::Host::code_tag`],
//!   [`eval::Host::table_string`]). `pcx-trig` owns `war3map.wts`; this crate
//!   does not depend on it.
//! - `.` member access and the `..` concatenation spelling do not exist in
//!   vanilla (concatenation is `+`); both are rejected by the lexer.
//! - Bare `!` is not an operator (JASS spells negation `not`); `!=` of course
//!   is.
//! - Comma-separated local lists (`local integer a, b`) and `constant …
//!   array` are rejected: vanilla has neither.
//! - Nothing is normalized away: no whitespace, no comment folding, no
//!   identifier case folding in the tree itself.

pub mod ast;
pub mod canon;
pub mod eval;
pub mod lexer;
pub mod natives;
pub mod parser;
pub mod print;
pub mod resolve;
pub mod span;
pub mod text;
pub mod value;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod tests_vm;

pub use ast::*;
pub use canon::fnv1a64;
pub use eval::{Host, NullHost, Vm, run_main};
pub use lexer::{
    Comment, CommentKind, Directive, DirectiveRecord, Esc, Ident, IntBase, IntLit, Kw, Lexed,
    RawLit, RealLit, StrLit, StrPart, Tok, Token,
};
pub use parser::parse;
pub use print::{CommentPolicy, PrintOptions};
pub use resolve::{
    Prelude, Program, ResolveDiagnostic, resolve, resolve_with, resolve_with_diagnostics,
};
pub use span::{SourceMap, Span};
pub use value::{HandleId, Value};

/// Counts a caller can print without walking the tree by hand.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Stats {
    pub bytes: usize,
    pub lines: usize,
    pub tokens: usize,
    pub comments: usize,
    pub directives: usize,
    pub items: usize,
    pub globals: usize,
    pub functions: usize,
    pub natives: usize,
    pub type_decls: usize,
    pub libraries: usize,
    pub scopes: usize,
    pub structs: usize,
    pub statements: usize,
    pub expressions: usize,
    pub int_literals: usize,
    pub real_literals: usize,
    pub string_literals: usize,
    pub rawcodes: usize,
    pub identifiers: usize,
}

impl Lexed {
    /// Token-kind counts, used by `scriptx lex`.
    pub fn stats(&self) -> Stats {
        let mut s = Stats {
            tokens: self.tokens.len(),
            comments: self.comments.len(),
            directives: self.directives.len(),
            ..Stats::default()
        };
        for t in &self.tokens {
            match &t.kind {
                Tok::Int(_) => s.int_literals += 1,
                Tok::Real(_) => s.real_literals += 1,
                Tok::Str(_) => s.string_literals += 1,
                Tok::Raw(_) => s.rawcodes += 1,
                Tok::Ident(_) => s.identifiers += 1,
                _ => {}
            }
        }
        s
    }
}

impl Script {
    /// Tree statistics. Deterministic: every counter is a walk over a `Vec`,
    /// never a map.
    pub fn stats(&self) -> Stats {
        let mut s = Stats {
            items: self.items.len(),
            directives: self.directives.len(),
            ..Stats::default()
        };
        for item in &self.items {
            self.count_item(item, &mut s);
        }
        s
    }

    fn count_item(&self, item: &Item, s: &mut Stats) {
        match item {
            Item::Globals(vars, _) => {
                s.globals += vars.len();
                for v in vars {
                    if let Some(e) = &v.init {
                        count_expr(e, s);
                    }
                }
            }
            Item::Type(_) => s.type_decls += 1,
            Item::Native(n) => {
                s.natives += 1;
                s.identifiers += n.params.len();
            }
            Item::Function(f) => {
                s.functions += 1;
                s.identifiers += f.params.len();
                if let Some(body) = &f.body {
                    for st in body {
                        count_stmt(st, s);
                    }
                }
            }
            Item::Library(l) => {
                s.libraries += 1;
                for it in &l.body {
                    self.count_item(it, s);
                }
            }
            Item::Scope(sc) => {
                s.scopes += 1;
                for it in &sc.body {
                    self.count_item(it, s);
                }
            }
            Item::Struct(st) => {
                s.structs += 1;
                for it in &st.body {
                    self.count_item(it, s);
                }
            }
        }
    }
}

fn count_stmt(st: &Stmt, s: &mut Stats) {
    s.statements += 1;
    match st {
        Stmt::Set { target, value, .. } => {
            count_expr(target, s);
            count_expr(value, s);
        }
        Stmt::Call { args, .. } => {
            for a in args {
                count_expr(a, s);
            }
        }
        Stmt::Return(Some(e), _) | Stmt::ExitWhen(e, _) => count_expr(e, s),
        Stmt::Return(None, _) => {}
        Stmt::If {
            branches,
            else_body,
            ..
        } => {
            for b in branches {
                count_expr(&b.cond, s);
                for st in &b.body {
                    count_stmt(st, s);
                }
            }
            if let Some(b) = else_body {
                for st in b {
                    count_stmt(st, s);
                }
            }
        }
        Stmt::Loop { body, .. } => {
            for st in body {
                count_stmt(st, s);
            }
        }
        Stmt::Local(v, _) | Stmt::Constant(v, _) => {
            if let Some(e) = &v.init {
                count_expr(e, s);
            }
        }
        Stmt::Debug(inner, _) => count_stmt(inner, s),
    }
}

fn count_expr(e: &Expr, s: &mut Stats) {
    s.expressions += 1;
    match e {
        Expr::Ident(_, _) => s.identifiers += 1,
        Expr::Literal(l, _) => match l {
            Literal::Int(_) => s.int_literals += 1,
            Literal::Real(_) => s.real_literals += 1,
            Literal::Str(_) => s.string_literals += 1,
            Literal::Raw(_) => s.rawcodes += 1,
            Literal::Bool(_) | Literal::Null => {}
        },
        Expr::Group(inner, _) | Expr::Unary { operand: inner, .. } => count_expr(inner, s),
        Expr::Call { callee, args, .. } => {
            count_expr(callee, s);
            for a in args {
                count_expr(a, s);
            }
        }
        Expr::Index { base, index, .. } => {
            count_expr(base, s);
            count_expr(index, s);
        }
        Expr::Binary { lhs, rhs, .. } => {
            count_expr(lhs, s);
            count_expr(rhs, s);
        }
        Expr::FunctionRef { .. } => {}
    }
}

/// A syntax-layer error. The span is a byte range; [`Error::locate`] turns it
/// into the line and column a map author sees.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{stage} error at {span}: {message}")]
pub struct Error {
    pub stage: Stage,
    pub span: Span,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stage {
    Lex,
    Parse,
    /// Name resolution: the checks in [`mod@resolve`].
    Resolve,
    /// Evaluation: the interpreter in [`eval`].
    Run,
}

impl Stage {
    pub fn name(self) -> &'static str {
        match self {
            Stage::Lex => "lex",
            Stage::Parse => "parse",
            Stage::Resolve => "resolve",
            Stage::Run => "run",
        }
    }
}

impl std::fmt::Display for Stage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl Error {
    pub(crate) fn lex(span: Span, message: impl Into<String>) -> Error {
        Error {
            stage: Stage::Lex,
            span,
            message: message.into(),
        }
    }

    pub(crate) fn parse(span: Span, message: impl Into<String>) -> Error {
        Error {
            stage: Stage::Parse,
            span,
            message: message.into(),
        }
    }

    pub(crate) fn resolve(span: Span, message: impl Into<String>) -> Error {
        Error {
            stage: Stage::Resolve,
            span,
            message: message.into(),
        }
    }

    /// A resolution or evaluation error the crate's public API raises. The
    /// span is a byte offset into the script that produced it, so
    /// [`Error::render`] reports the map's own line and column.
    pub fn run(span: Span, message: impl Into<String>) -> Error {
        Error {
            stage: Stage::Run,
            span,
            message: message.into(),
        }
    }

    /// `(line, column)` of the error start, 1-based in both components,
    /// columns counted in characters.
    pub fn locate(&self, map: &SourceMap) -> (u32, u32) {
        map.line_col(self.span.start)
    }

    /// The message with the map's own line and column prefixed.
    pub fn render(&self, src: &str) -> String {
        let map = SourceMap::new(src);
        let (l, c) = self.locate(&map);
        format!(
            "{l}:{c} {span}: {stage} error: {message}",
            l = l,
            c = c,
            span = map.render(self.span),
            stage = self.stage.name(),
            message = self.message
        )
    }
}

/// Tokenize JASS. Comments and directives come back in side tables.
pub fn lex(src: &str) -> Result<Lexed, Error> {
    lexer::lex(src)
}

/// Tokenize and parse JASS into an exact syntax tree.
pub fn parse_script(src: &str) -> Result<Script, Error> {
    parse(src)
}

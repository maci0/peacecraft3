//! The exact JASS syntax tree.
//!
//! # What "exact" means here
//!
//! Every node keeps enough to reproduce the map's own text and to report the
//! map's own line and column:
//!
//! - Every node carries a [`Span`].
//! - Parenthesisation is a node ([`Expr::Group`]), not a reconstruction from
//!   precedence. `parse` never has to guess what the author meant.
//! - String literals keep their escapes as [`StrPart`]s rather than a decoded
//!   `String`, so `\\` stays distinguishable from `\` and a `^^` code tag is
//!   not silently flattened into text.
//! - Integer literals keep the base they were written in; real literals keep
//!   their exact lexeme. Neither is normalised away, so re-printing and
//!   re-parsing is byte-identical.
//! - Vanilla JASS is case-sensitive: keywords, type names, identifiers and
//!   rawcodes all keep their spelling, and the tree preserves it. `common.j`
//!   proves the keyword half by declaring `constant boolean TRUE = true`;
//!   `'A000'` and `'a000'` are likewise different object ids.
//!
//! # Vanilla shapes only
//!
//! The statement and declaration grammar here is the vanilla one, measured
//! against `pjass`'s grammar (the checker the community validates maps with)
//! and the shipped `common.j` / `Blizzard.j` / map corpus. The vJass block
//! shapes (`library` / `scope` / `struct` with `requires` / `uses` /
//! `initializer`) are kept as containers because real maps wrap their code in
//! them, but everything inside them is still vanilla declarations and
//! statements. vJass *methods*, `cast`, `delegate` and Zinc are refused
//! loudly by the parser rather than guessed at: their meaning is defined by
//! JASSHelper's rewrite pass, and a plausible-but-wrong tree here would be
//! exactly what this crate exists to avoid.
//!
//! # What this is not
//!
//! There is no evaluation, no constant folding, no type checking and no name
//! resolution here. Those belong to the VM stage, and a tree that had already
//! guessed at them would be the "plausible but wrong" layer this crate refuses
//! to be.

use crate::span::Span;

/// Visibility in front of a declaration. Vanilla JASS has none; vJass
/// libraries and scopes do, and maps that ship their code through
/// library/scope wrappers carry the spelling.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Default)]
pub enum Vis {
    #[default]
    Public,
    Private,
}

impl Vis {
    pub fn keyword(self) -> Option<&'static str> {
        match self {
            Vis::Public => None,
            Vis::Private => Some("private"),
        }
    }
}

/// A type reference: a builtin or user-declared type name plus the `array`
/// marker, which in JASS sits between the type and the variable name.
#[derive(Clone, PartialEq, Debug)]
pub struct TypeRef {
    pub kind: TypeKind,
    pub array: bool,
    pub span: Span,
}

#[derive(Clone, PartialEq, Debug)]
pub enum TypeKind {
    /// `nothing` — the empty parameter list and the absent return value.
    Nothing,
    Integer,
    Real,
    String,
    Boolean,
    /// `code` — a function-reference value.
    Code,
    /// `handle`, the root of the handle hierarchy.
    Handle,
    /// A user-declared type, spelled exactly as written.
    Named(String),
}

impl TypeKind {
    pub fn name(&self) -> &str {
        match self {
            TypeKind::Nothing => "nothing",
            TypeKind::Integer => "integer",
            TypeKind::Real => "real",
            TypeKind::String => "string",
            TypeKind::Boolean => "boolean",
            TypeKind::Code => "code",
            TypeKind::Handle => "handle",
            TypeKind::Named(n) => n,
        }
    }

    /// The builtin type spellings, matched exactly: JASS name lookup is
    /// case-sensitive, so `Integer` is a user-declared type name and keeps
    /// its spelling in [`TypeKind::Named`] rather than being folded.
    pub fn builtin(word: &str) -> Option<TypeKind> {
        Some(match word {
            "nothing" => TypeKind::Nothing,
            "integer" => TypeKind::Integer,
            "real" => TypeKind::Real,
            "string" => TypeKind::String,
            "boolean" => TypeKind::Boolean,
            "code" => TypeKind::Code,
            "handle" => TypeKind::Handle,
            _ => return None,
        })
    }
}

/// One declaration inside `globals … endglobals`, or a `local` / `constant`
/// statement inside a function. Vanilla allows exactly one name per
/// declaration line and forbids `constant … array` (arrays cannot be
/// initialised, so a constant array would be meaningless).
#[derive(Clone, PartialEq, Debug)]
pub struct VarDecl {
    pub vis: Vis,
    pub is_constant: bool,
    pub ty: TypeRef,
    pub name: Ident,
    pub init: Option<Expr>,
    pub span: Span,
}

/// `type unit extends widget` — vanilla's flat type extension, which the VM
/// turns into the handle hierarchy. `extends` is required: the game's own
/// compiler rejects a type declaration without one.
#[derive(Clone, PartialEq, Debug)]
pub struct TypeDecl {
    pub vis: Vis,
    pub name: Ident,
    pub extends: Ident,
    pub array: bool,
    pub span: Span,
}

/// `native Foo takes integer a returns nothing`, optionally `constant`
/// (common.j declares `constant native`s).
#[derive(Clone, PartialEq, Debug)]
pub struct NativeDecl {
    pub vis: Vis,
    pub is_constant: bool,
    pub name: Ident,
    pub params: Vec<Param>,
    pub ret: TypeRef,
    pub span: Span,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Param {
    pub name: Ident,
    pub ty: TypeRef,
    pub span: Span,
}

#[derive(Clone, PartialEq, Debug)]
pub struct FunctionDecl {
    pub vis: Vis,
    /// `constant function` — callable from constant initialisers. The game's
    /// compiler has dedicated errors for it, so it is a real vanilla form.
    pub is_constant: bool,
    /// vJass `static function`, kept because library-wrapped maps carry it.
    pub is_static: bool,
    pub name: Ident,
    pub params: Vec<Param>,
    pub ret: TypeRef,
    /// `None` for a body-less declaration terminated immediately by
    /// `endfunction`.
    pub body: Option<Vec<Stmt>>,
    pub span: Span,
}

/// One entry of a vJass `requires` / `uses` clause.
#[derive(Clone, PartialEq, Debug)]
pub struct LibRef {
    pub name: Ident,
    pub optional: bool,
    pub public: bool,
}

/// `library NAME requires A B uses C … endlibrary` (vJass wrapper shape).
#[derive(Clone, PartialEq, Debug)]
pub struct LibraryDecl {
    pub name: Ident,
    pub requires: Vec<LibRef>,
    pub uses: Option<Vec<LibRef>>,
    pub initializer: Option<Ident>,
    pub body: Vec<Item>,
    pub span: Span,
}

/// `scope NAME initializer initScope … endscope` (vJass wrapper shape).
#[derive(Clone, PartialEq, Debug)]
pub struct ScopeDecl {
    pub name: Ident,
    pub initializer: Option<Ident>,
    pub body: Vec<Item>,
    pub span: Span,
}

/// `struct NAME … endstruct` (vJass wrapper shape). Its *semantics* — how a
/// struct becomes a scope or a handle table — are a JASSHelper rewrite; the
/// container is kept so wrapped maps parse, and any method inside it is
/// refused loudly.
#[derive(Clone, PartialEq, Debug)]
pub struct StructDecl {
    pub name: Ident,
    pub body: Vec<Item>,
    pub span: Span,
}

/// A top-level item, in source order.
#[derive(Clone, PartialEq, Debug)]
pub enum Item {
    Globals(Vec<VarDecl>, Span),
    Type(TypeDecl),
    Native(NativeDecl),
    Function(FunctionDecl),
    Library(LibraryDecl),
    Scope(ScopeDecl),
    Struct(StructDecl),
}

impl Item {
    pub fn span(&self) -> Span {
        match self {
            Item::Globals(_, s)
            | Item::Type(TypeDecl { span: s, .. })
            | Item::Native(NativeDecl { span: s, .. })
            | Item::Function(FunctionDecl { span: s, .. })
            | Item::Library(LibraryDecl { span: s, .. })
            | Item::Scope(ScopeDecl { span: s, .. })
            | Item::Struct(StructDecl { span: s, .. }) => *s,
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum Stmt {
    /// `set lhs = rhs`
    Set {
        target: Expr,
        value: Expr,
        span: Span,
    },
    /// `call f(a, b)`. The callee is a plain name, exactly as vanilla's
    /// `call` statement demands.
    Call {
        callee: Ident,
        args: Vec<Expr>,
        span: Span,
    },
    Return(Option<Expr>, Span),
    If {
        branches: Vec<IfBranch>,
        else_body: Option<Vec<Stmt>>,
        span: Span,
    },
    Loop {
        body: Vec<Stmt>,
        span: Span,
    },
    /// `exitwhen cond`
    ExitWhen(Expr, Span),
    /// `local type name = init`
    Local(VarDecl, Span),
    /// `constant type name = init` — a declaration statement in its own right
    /// (what maps write where a named constant is local to a function).
    Constant(VarDecl, Span),
    /// `debug statement` — `debug` wraps a whole statement (`debug call f()`,
    /// `debug set x = 1`, …), as Blizzard's own `common.ai` writes it.
    Debug(Box<Stmt>, Span),
}

impl Stmt {
    pub fn span(&self) -> Span {
        match self {
            Stmt::Set { span, .. }
            | Stmt::Call { span, .. }
            | Stmt::Return(_, span)
            | Stmt::If { span, .. }
            | Stmt::Loop { span, .. }
            | Stmt::ExitWhen(_, span)
            | Stmt::Local(_, span)
            | Stmt::Constant(_, span)
            | Stmt::Debug(_, span) => *span,
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct IfBranch {
    pub cond: Expr,
    pub body: Vec<Stmt>,
    pub span: Span,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    /// `%`. Integer modulus; the engine's own JASS writer emits it as ` % `
    /// and its parser has a dedicated `NodeMod`. (Blizzard's `ModuloInteger`
    /// in Blizzard.j is a convenience function, not evidence of absence.)
    Modulo,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

impl BinOp {
    pub fn spelling(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Modulo => "%",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
            BinOp::And => "and",
            BinOp::Or => "or",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum UnOp {
    Not,
    Neg,
    /// Unary `+`, which vanilla accepts as numeric identity.
    Pos,
}

impl UnOp {
    pub fn spelling(self) -> &'static str {
        match self {
            UnOp::Not => "not",
            UnOp::Neg => "-",
            UnOp::Pos => "+",
        }
    }
}

/// A literal, kept in the exact form it was written.
#[derive(Clone, PartialEq, Debug)]
pub enum Literal {
    Int(IntLit),
    Real(RealLit),
    Str(StrLit),
    Raw(RawLit),
    Bool(bool),
    Null,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Expr {
    Ident(Ident, Span),
    Literal(Literal, Span),
    /// An explicit parenthesis pair. Kept as a node so the map's own
    /// grouping is reproduced verbatim instead of being re-derived.
    Group(Box<Expr>, Span),
    /// `f(a, b)`
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
        span: Span,
    },
    /// `a[i]`
    Index {
        base: Box<Expr>,
        index: Box<Expr>,
        span: Span,
    },
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: Span,
    },
    Unary {
        op: UnOp,
        operand: Box<Expr>,
        span: Span,
    },
    /// `function Foo` (and the tolerated `function Foo()` spelling) used as
    /// a `code` value, e.g. `ForGroup(g, function X)`.
    FunctionRef {
        name: Ident,
        span: Span,
    },
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Ident(_, s)
            | Expr::Literal(_, s)
            | Expr::Group(_, s)
            | Expr::Call { span: s, .. }
            | Expr::Index { span: s, .. }
            | Expr::Binary { span: s, .. }
            | Expr::Unary { span: s, .. }
            | Expr::FunctionRef { span: s, .. } => *s,
        }
    }

    /// Binding strength, used by the printer to re-parenthesise a
    /// hand-built tree. Explicit [`Expr::Group`] nodes keep the map's own
    /// parentheses on top of this.
    ///
    /// The ladder is vanilla's, not C's: `and` binds *looser* than `or`,
    /// and `not` sits between the comparison and arithmetic rungs. Both
    /// community parsers (pjass, jassjass) agree on this, and shipped maps
    /// parenthesise every mixed `and`/`or` condition, so nothing in the
    /// corpus depends on the ordering — but a hand-built tree must not
    /// re-associate against the engine.
    pub fn precedence(&self) -> u8 {
        match self {
            Expr::Call { .. } | Expr::Index { .. } | Expr::FunctionRef { .. } => 100,
            Expr::Literal(..) | Expr::Ident(_, _) | Expr::Group(_, _) => 90,
            // Unary `-` / `+` bind tightest of the operators.
            Expr::Unary {
                op: UnOp::Neg | UnOp::Pos,
                ..
            } => 80,
            Expr::Binary { op, .. } => match op {
                BinOp::Mul | BinOp::Div | BinOp::Modulo => 70,
                BinOp::Add | BinOp::Sub => 60,
                // `not` reaches over `+`/`-`: `not a + b` is `not (a + b)`.
                BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => 40,
                BinOp::Eq | BinOp::Ne => 30,
                BinOp::Or => 20,
                BinOp::And => 10,
            },
            Expr::Unary { op: UnOp::Not, .. } => 50,
        }
    }
}

/// A parsed script.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Script {
    pub items: Vec<Item>,
    /// Preprocessor / tooling directive lines, in source order. They do not
    /// shape the tree — what they mean is a preprocessing question — but
    /// they are kept and re-emitted by the printer, so a map's tooling
    /// instructions survive a parse/print cycle. This covers the forms
    /// community tooling actually emits: JASSHelper's `//! …` lines, the
    /// `//# …` annotation lines pjass understands, and bare `#name …` lines.
    pub directives: Vec<DirectiveRecord>,
    /// Comments, in source order, with spans. Community maps are debugged
    /// through their comments.
    pub comments: Vec<Comment>,
}

impl Script {
    pub fn functions(&self) -> impl Iterator<Item = &FunctionDecl> {
        self.items.iter().filter_map(|i| match i {
            Item::Function(f) => Some(f),
            _ => None,
        })
    }

    pub fn natives(&self) -> impl Iterator<Item = &NativeDecl> {
        self.items.iter().filter_map(|i| match i {
            Item::Native(n) => Some(n),
            _ => None,
        })
    }

    /// Every global, including those nested in libraries and scopes.
    pub fn globals(&self) -> Vec<&VarDecl> {
        let mut out = Vec::new();
        collect_globals(&self.items, &mut out);
        out
    }

    /// Every function, including those nested in libraries, scopes and
    /// structs. Library-wrapped maps hide real entry points in here.
    pub fn all_functions(&self) -> Vec<&FunctionDecl> {
        let mut out = Vec::new();
        collect_functions(&self.items, &mut out);
        out
    }
}

fn collect_globals<'a>(items: &'a [Item], out: &mut Vec<&'a VarDecl>) {
    for item in items {
        match item {
            Item::Globals(vars, _) => out.extend(vars.iter()),
            Item::Library(l) => collect_globals(&l.body, out),
            Item::Scope(s) => collect_globals(&s.body, out),
            Item::Struct(s) => collect_globals(&s.body, out),
            _ => {}
        }
    }
}

fn collect_functions<'a>(items: &'a [Item], out: &mut Vec<&'a FunctionDecl>) {
    for item in items {
        match item {
            Item::Function(f) => out.push(f),
            Item::Library(l) => collect_functions(&l.body, out),
            Item::Scope(s) => collect_functions(&s.body, out),
            Item::Struct(s) => collect_functions(&s.body, out),
            _ => {}
        }
    }
}

/// Re-exported so downstream crates need one import.
pub use crate::lexer::{
    Comment, DirectiveRecord, Esc, Ident, IntBase, IntLit, RawLit, RealLit, StrLit, StrPart,
};

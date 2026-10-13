//! A canonical byte form of the tree, and an FNV-1a 64 hash over it.
//!
//! The point is the same one the asset decoders use: two implementations
//! that claim to have parsed the same script must agree byte for byte on what
//! they parsed. [`Script::canonical_bytes`] is a total, ordered encoding of
//! the syntax with **no spans and no whitespace**, so it is stable against
//! reformatting but sensitive to every difference that could change what the
//! map means.
//!
//! What it deliberately does *not* include:
//!
//! - Spans. Two trees parsed from re-indented copies of the same script must
//!   hash the same.
//! - Comments. They are trivia; [`Script::comments`] keeps them for humans.
//! - Real-literal *values*. The exact lexeme is hashed, because `1.50` and
//!   `1.5` are the same `f64` but different map text. No `f64` is ever
//!   formatted or hashed here, so there is no float-formatting instability to
//!   worry about.
//!
//! Every collection is emitted in source order. Nothing iterates a hash map.

use crate::ast::*;
use crate::lexer::{CommentKind, DirectiveRecord, Esc, Ident, IntBase, RealLit, StrLit, StrPart};
use crate::span::Span;

/// FNV-1a 64, the offset basis `0xcbf29ce484222325` and prime
/// `0x100000001b3`, matching `pcx_mpq::fnv1a64`.
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

/// Byte tags. One byte per node kind, so a mis-tagged tree cannot collide
/// with a differently-shaped one.
mod tag {
    pub const IDENT: u8 = 0x01;
    pub const INT: u8 = 0x02;
    pub const REAL: u8 = 0x03;
    pub const STR: u8 = 0x04;
    pub const RAW: u8 = 0x05;
    pub const BOOL: u8 = 0x06;
    pub const NULL: u8 = 0x07;
    pub const GROUP: u8 = 0x09;
    pub const CALL: u8 = 0x0a;
    pub const INDEX: u8 = 0x0b;
    pub const BINARY: u8 = 0x0c;
    pub const UNARY: u8 = 0x0d;
    pub const FUNCREF: u8 = 0x0e;
    pub const GLOBALS: u8 = 0x10;
    pub const VAR: u8 = 0x11;
    pub const TYPE: u8 = 0x12;
    pub const NATIVE: u8 = 0x13;
    pub const FUNCTION: u8 = 0x14;
    pub const LIBRARY: u8 = 0x15;
    pub const SCOPE: u8 = 0x16;
    pub const STRUCT: u8 = 0x17;
    pub const PARAM: u8 = 0x18;
    pub const LIBREF: u8 = 0x19;
    pub const SET: u8 = 0x20;
    pub const CALLSTMT: u8 = 0x21;
    pub const RETURN: u8 = 0x22;
    pub const IF: u8 = 0x23;
    pub const BRANCH: u8 = 0x24;
    pub const LOOP: u8 = 0x25;
    pub const EXITWHEN: u8 = 0x26;
    pub const LOCAL: u8 = 0x28;
    pub const CONSTANT: u8 = 0x29;
    pub const DEBUG: u8 = 0x2a;
    pub const DIRECTIVE: u8 = 0x30;
    pub const STRING_PART: u8 = 0x31;
}

struct Buf {
    b: Vec<u8>,
}

impl Buf {
    fn new() -> Buf {
        Buf { b: Vec::new() }
    }

    fn tag(&mut self, t: u8) {
        self.b.push(t);
    }

    fn u8(&mut self, v: u8) {
        self.b.push(v);
    }

    fn len(&mut self, v: usize) {
        self.b.extend_from_slice(&(v as u32).to_le_bytes());
    }

    fn str(&mut self, s: &str) {
        self.len(s.len());
        self.b.extend_from_slice(s.as_bytes());
    }

    fn items(&mut self, v: &[Item]) {
        self.len(v.len());
        for i in v {
            self.item(i);
        }
    }

    fn stmts(&mut self, v: &[Stmt]) {
        self.len(v.len());
        for s in v {
            self.stmt(s);
        }
    }

    fn exprs(&mut self, v: &[Expr]) {
        self.len(v.len());
        for e in v {
            self.expr(e);
        }
    }

    fn vars(&mut self, v: &[VarDecl]) {
        self.len(v.len());
        for x in v {
            self.var(x);
        }
    }

    fn type_ref(&mut self, t: &TypeRef) {
        let k = match &t.kind {
            TypeKind::Nothing => 0u8,
            TypeKind::Integer => 1,
            TypeKind::Real => 2,
            TypeKind::String => 3,
            TypeKind::Boolean => 4,
            TypeKind::Code => 5,
            TypeKind::Handle => 6,
            TypeKind::Named(_) => 8,
        };
        self.u8(k);
        self.u8(u8::from(t.array));
        if let TypeKind::Named(n) = &t.kind {
            self.str(n);
        }
    }

    fn var(&mut self, v: &VarDecl) {
        self.tag(tag::VAR);
        self.u8(u8::from(v.vis == Vis::Private));
        self.u8(u8::from(v.is_constant));
        self.type_ref(&v.ty);
        self.str(v.name.as_str());
        match &v.init {
            Some(e) => {
                self.u8(1);
                self.expr(e);
            }
            None => self.u8(0),
        }
    }

    fn item(&mut self, i: &Item) {
        match i {
            Item::Globals(vars, _) => {
                self.tag(tag::GLOBALS);
                self.vars(vars);
            }
            Item::Type(t) => {
                self.tag(tag::TYPE);
                self.u8(u8::from(t.vis == Vis::Private));
                self.str(t.name.as_str());
                self.str(t.extends.as_str());
                self.u8(u8::from(t.array));
            }
            Item::Native(n) => {
                self.tag(tag::NATIVE);
                self.u8(u8::from(n.vis == Vis::Private));
                self.u8(u8::from(n.is_constant));
                self.str(n.name.as_str());
                self.params(&n.params);
                self.type_ref(&n.ret);
            }
            Item::Function(f) => {
                self.tag(tag::FUNCTION);
                self.u8(u8::from(f.vis == Vis::Private));
                self.u8(u8::from(f.is_constant));
                self.u8(u8::from(f.is_static));
                self.str(f.name.as_str());
                self.params(&f.params);
                self.type_ref(&f.ret);
                match &f.body {
                    Some(b) => {
                        self.u8(1);
                        self.stmts(b);
                    }
                    None => self.u8(0),
                }
            }
            Item::Library(l) => {
                self.tag(tag::LIBRARY);
                self.str(l.name.as_str());
                self.lib_refs(&l.requires);
                match &l.uses {
                    Some(u) => {
                        self.u8(1);
                        self.lib_refs(u);
                    }
                    None => self.u8(0),
                }
                match &l.initializer {
                    Some(i) => {
                        self.u8(1);
                        self.str(i.as_str());
                    }
                    None => self.u8(0),
                }
                self.items(&l.body);
            }
            Item::Scope(s) => {
                self.tag(tag::SCOPE);
                self.str(s.name.as_str());
                match &s.initializer {
                    Some(i) => {
                        self.u8(1);
                        self.str(i.as_str());
                    }
                    None => self.u8(0),
                }
                self.items(&s.body);
            }
            Item::Struct(s) => {
                self.tag(tag::STRUCT);
                self.str(s.name.as_str());
                self.items(&s.body);
            }
        }
    }

    fn params(&mut self, p: &[Param]) {
        self.len(p.len());
        for x in p {
            self.tag(tag::PARAM);
            self.type_ref(&x.ty);
            self.str(x.name.as_str());
        }
    }

    fn lib_refs(&mut self, v: &[LibRef]) {
        self.len(v.len());
        for r in v {
            self.tag(tag::LIBREF);
            self.u8(u8::from(r.optional));
            self.u8(u8::from(r.public));
            self.str(r.name.as_str());
        }
    }

    fn stmt(&mut self, s: &Stmt) {
        match s {
            Stmt::Set { target, value, .. } => {
                self.tag(tag::SET);
                self.expr(target);
                self.expr(value);
            }
            Stmt::Call { callee, args, .. } => {
                self.tag(tag::CALLSTMT);
                self.str(callee.as_str());
                self.exprs(args);
            }
            Stmt::Return(v, _) => {
                self.tag(tag::RETURN);
                match v {
                    Some(e) => {
                        self.u8(1);
                        self.expr(e);
                    }
                    None => self.u8(0),
                }
            }
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                self.tag(tag::IF);
                self.len(branches.len());
                for b in branches {
                    self.tag(tag::BRANCH);
                    self.expr(&b.cond);
                    self.stmts(&b.body);
                }
                match else_body {
                    Some(b) => {
                        self.u8(1);
                        self.stmts(b);
                    }
                    None => self.u8(0),
                }
            }
            Stmt::Loop { body, .. } => {
                self.tag(tag::LOOP);
                self.stmts(body);
            }
            Stmt::ExitWhen(e, _) => {
                self.tag(tag::EXITWHEN);
                self.expr(e);
            }
            Stmt::Local(v, _) => {
                self.tag(tag::LOCAL);
                self.var(v);
            }
            Stmt::Constant(v, _) => {
                self.tag(tag::CONSTANT);
                self.var(v);
            }
            Stmt::Debug(inner, _) => {
                self.tag(tag::DEBUG);
                self.stmt(inner);
            }
        }
    }

    fn expr(&mut self, e: &Expr) {
        match e {
            Expr::Ident(i, _) => {
                self.tag(tag::IDENT);
                self.str(i.as_str());
            }
            Expr::Literal(l, _) => self.literal(l),
            Expr::Group(inner, _) => {
                self.tag(tag::GROUP);
                self.expr(inner);
            }
            Expr::Call { callee, args, .. } => {
                self.tag(tag::CALL);
                self.expr(callee);
                self.exprs(args);
            }
            Expr::Index { base, index, .. } => {
                self.tag(tag::INDEX);
                self.expr(base);
                self.expr(index);
            }
            Expr::Binary { op, lhs, rhs, .. } => {
                self.tag(tag::BINARY);
                self.u8(binop_code(*op));
                self.expr(lhs);
                self.expr(rhs);
            }
            Expr::Unary { op, operand, .. } => {
                self.tag(tag::UNARY);
                self.u8(match op {
                    UnOp::Not => 0,
                    UnOp::Neg => 1,
                    UnOp::Pos => 2,
                });
                self.expr(operand);
            }
            Expr::FunctionRef { name, .. } => {
                self.tag(tag::FUNCREF);
                self.str(name.as_str());
            }
        }
    }

    fn literal(&mut self, l: &Literal) {
        match l {
            Literal::Int(v) => {
                self.tag(tag::INT);
                self.u8(match v.base {
                    IntBase::Dec => 0,
                    IntBase::Hex => 1,
                    IntBase::Octal => 2,
                    IntBase::Bin => 3,
                });
                self.b.extend_from_slice(&v.value.to_le_bytes());
            }
            Literal::Real(v) => {
                self.tag(tag::REAL);
                self.str(&v.text);
            }
            Literal::Str(s) => {
                self.tag(tag::STR);
                self.string(s);
            }
            Literal::Raw(r) => {
                self.tag(tag::RAW);
                self.str(&r.text);
            }
            Literal::Bool(b) => {
                self.tag(tag::BOOL);
                self.u8(u8::from(*b));
            }
            Literal::Null => self.tag(tag::NULL),
        }
    }

    fn string(&mut self, s: &StrLit) {
        self.len(s.parts.len());
        for p in &s.parts {
            self.tag(tag::STRING_PART);
            match p {
                StrPart::Text(t) => {
                    self.u8(0);
                    self.str(t);
                }
                StrPart::Escape(e) => {
                    self.u8(1);
                    self.u8(match e {
                        Esc::Quote => 0,
                        Esc::Backslash => 1,
                        Esc::Bsp => 2,
                        Esc::Tab => 3,
                        Esc::Cr => 4,
                        Esc::Nl => 5,
                        Esc::Ff => 6,
                    });
                }
                StrPart::CodeTag { text } => {
                    self.u8(2);
                    self.str(text);
                }
            }
        }
    }
}

impl Script {
    /// Total, ordered, span-free encoding of the tree.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut buf = Buf::new();
        buf.items(&self.items);
        buf.len(self.directives.len());
        for d in &self.directives {
            directive(&mut buf, d);
        }
        buf.b
    }

    /// FNV-1a 64 over [`Script::canonical_bytes`].
    pub fn hash(&self) -> u64 {
        fnv1a64(&self.canonical_bytes())
    }
}

fn directive(buf: &mut Buf, d: &DirectiveRecord) {
    buf.tag(tag::DIRECTIVE);
    buf.str(&d.directive.name);
    buf.str(&d.directive.args);
    buf.str(&d.directive.text);
}

fn binop_code(op: BinOp) -> u8 {
    match op {
        BinOp::Add => 0,
        BinOp::Sub => 1,
        BinOp::Mul => 2,
        BinOp::Div => 3,
        BinOp::Modulo => 4,
        BinOp::Eq => 6,
        BinOp::Ne => 7,
        BinOp::Lt => 8,
        BinOp::Le => 9,
        BinOp::Gt => 10,
        BinOp::Ge => 11,
        BinOp::And => 12,
        BinOp::Or => 13,
    }
}

/// `CommentKind` is re-exported through the AST; this keeps the import used
/// and documents that comments never reach the canonical form.
#[allow(dead_code)]
fn comments_are_not_canonical(_: CommentKind) {}

#[allow(dead_code)]
fn spans_are_not_canonical(_: Span) {}

#[allow(dead_code)]
fn reals_are_not_canonical(_: &RealLit) {}

#[allow(dead_code)]
fn idents_are_named_not_folded(_: &Ident) {}

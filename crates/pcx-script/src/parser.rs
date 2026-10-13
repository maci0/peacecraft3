//! Recursive-descent JASS parser.
//!
//! # Line orientation
//!
//! Vanilla JASS is line-oriented: a statement starts on its own line and
//! leaves nothing behind on that line (the game's own compiler reports
//! "unexpected data at the end of line"). Every shipped script obeys this,
//! and the parser enforces it, so a script that only works because the
//! syntax layer is sloppy never gets that far.
//!
//! # Precedence
//!
//! The ladder below is vanilla's, and it is *not* C's. Both community
//! parsers (pjass's yacc table, jassjass's `p1…p6` descent) agree:
//!
//! ```text
//! and            (loosest)
//! or
//! ==  !=
//! <  <=  >  >=
//! not            (unary; reaches over + and -: `not a + b` = `not (a + b)`)
//! +  -
//! *  /  %
//! unary -  +     (tightest)
//! primary
//! ```
//!
//! `and` binds *looser* than `or`, so `a and b or c` is `a and (b or c)`.
//! Shipped maps parenthesise every mixed condition they write, so nothing in
//! the corpus depends on this — but a re-implementation that assumed C
//! grouping would silently re-associate whatever a map did leave unparenthesised.
//!
//! String concatenation is plain `+` (type-directed); there is no `..`
//! operator in vanilla JASS.

use crate::Error;
use crate::ast::*;
use crate::lexer::{Ident, Kw, Tok, Token};
use crate::span::Span;

pub fn parse(src: &str) -> Result<Script, Error> {
    let lexed = crate::lexer::lex(src)?;
    let mut p = Parser::new(lexed.tokens, line_starts(src));
    let mut items = Vec::new();
    while !p.at_eof() {
        if matches!(p.peek().kind, Tok::Directive(_)) {
            // Already recorded by the lexer; directives are not items.
            p.bump();
            continue;
        }
        items.push(p.item()?);
    }
    Ok(Script {
        items,
        directives: lexed.directives,
        comments: lexed.comments,
    })
}

/// Byte offsets of every line start, for the line-orientation checks.
/// `\n`, bare `\r` and `\r\n` each end exactly one line: a shipped protected
/// map (`Anime Maul Beta v0.952(Protect).w3x`) writes its `war3map.j` with
/// bare-CR endings and the game accepts it.
fn line_starts(src: &str) -> Vec<u32> {
    let b = src.as_bytes();
    let mut v = vec![0u32];
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'\r' => {
                i += 1;
                if b.get(i) == Some(&b'\n') {
                    i += 1;
                }
                v.push(i as u32);
            }
            b'\n' => {
                i += 1;
                v.push(i as u32);
            }
            _ => i += 1,
        }
    }
    v
}

struct Parser {
    toks: Vec<Token>,
    pos: usize,
    lines: Vec<u32>,
}

impl Parser {
    fn new(toks: Vec<Token>, lines: Vec<u32>) -> Parser {
        Parser {
            toks,
            pos: 0,
            lines,
        }
    }

    fn peek(&self) -> &Token {
        &self.toks[self.pos.min(self.toks.len() - 1)]
    }

    fn span(&self) -> Span {
        self.peek().span
    }

    fn at_eof(&self) -> bool {
        matches!(self.peek().kind, Tok::Eof)
    }

    fn bump(&mut self) -> Token {
        let t = self.peek().clone();
        if self.pos < self.toks.len() - 1 {
            self.pos += 1;
        }
        t
    }

    fn line_of(&self, offset: u32) -> usize {
        match self.lines.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => i - 1,
        }
    }

    /// A statement or declaration must start on its own line.
    fn check_fresh_line(&self) -> Result<(), Error> {
        if self.pos == 0 {
            return Ok(());
        }
        let prev_end = self.toks[self.pos - 1].span.end;
        let cur = self.peek().span.start;
        if self.line_of(prev_end) == self.line_of(cur) {
            return Err(self.error_here(format!(
                "expected a line break before `{}`",
                self.peek().kind.label()
            )));
        }
        Ok(())
    }

    /// A statement or header leaves nothing behind on its line.
    fn expect_line_end(&self) -> Result<(), Error> {
        if self.at_eof() {
            return Ok(());
        }
        let prev_end = self.toks[self.pos.saturating_sub(1).min(self.toks.len() - 1)]
            .span
            .end;
        let cur = self.peek().span.start;
        if self.line_of(prev_end) == self.line_of(cur) {
            return Err(self.error_here(format!(
                "unexpected data at the end of the line: `{}`",
                self.peek().kind.label()
            )));
        }
        Ok(())
    }

    fn eat_kw(&mut self, k: Kw) -> bool {
        if self.peek().is_kw(k) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn expect_kw(&mut self, k: Kw) -> Result<Span, Error> {
        if self.peek().is_kw(k) {
            Ok(self.bump().span)
        } else {
            Err(self.error_here(format!("expected `{}`", k.text())))
        }
    }

    fn eat(&mut self, t: &Tok) -> bool {
        if &self.peek().kind == t {
            self.bump();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, t: Tok) -> Result<Span, Error> {
        if self.peek().kind == t {
            Ok(self.bump().span)
        } else {
            Err(self.error_here(format!("expected `{}`", t.label())))
        }
    }

    fn error_here(&self, message: impl Into<String>) -> Error {
        Error::parse(self.span(), message)
    }

    /// A name. Reserved words (including the reserved-but-unused `alias`)
    /// can never be one.
    fn expect_ident(&mut self) -> Result<Ident, Error> {
        match self.peek().kind.clone() {
            Tok::Ident(i) => {
                self.bump();
                Ok(i)
            }
            _ => Err(self.error_here("expected an identifier")),
        }
    }

    /// A contextual vJass word: an ordinary identifier with this spelling.
    fn at_word(&self, word: &str) -> bool {
        matches!(&self.peek().kind, Tok::Ident(i) if i.name == word)
    }

    fn eat_word(&mut self, word: &str) -> bool {
        if self.at_word(word) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn expect_word(&mut self, word: &str) -> Result<Span, Error> {
        if self.at_word(word) {
            Ok(self.bump().span)
        } else {
            Err(self.error_here(format!("expected `{word}`")))
        }
    }

    // ----- top level -------------------------------------------------

    fn item(&mut self) -> Result<Item, Error> {
        self.check_fresh_line()?;
        // vJass declaration modifiers, recognised contextually so vanilla
        // names like `public` or `static` stay legal identifiers.
        let mut vis = Vis::Public;
        let mut is_constant = false;
        let mut is_static = false;
        loop {
            if self.at_word("private") {
                self.bump();
                vis = Vis::Private;
            } else if self.at_word("public") {
                self.bump();
                vis = Vis::Public;
            } else if self.at_word("static") {
                self.bump();
                is_static = true;
            } else if self.peek().is_kw(Kw::Constant) {
                self.bump();
                is_constant = true;
            } else {
                break;
            }
        }
        if vis != Vis::Public || is_constant || is_static {
            // A modifier can only prefix `function` or `native`.
            return if self.peek().is_kw(Kw::Native) {
                self.native_decl(vis, is_constant).map(Item::Native)
            } else if self.peek().is_kw(Kw::Function) {
                self.function_decl(vis, is_constant, is_static)
                    .map(Item::Function)
            } else {
                Err(self
                    .error_here("declaration modifiers are only valid on `function` and `native`"))
            };
        }
        match self.peek().kind.clone() {
            Tok::Kw(Kw::Globals) => self.globals_block(),
            Tok::Kw(Kw::Type) => self.type_decl(vis).map(Item::Type),
            Tok::Kw(Kw::Native) => self.native_decl(vis, false).map(Item::Native),
            Tok::Kw(Kw::Function) => self.function_decl(vis, false, false).map(Item::Function),
            Tok::Ident(i) => match i.name.as_str() {
                "library" => self.library_decl().map(Item::Library),
                "scope" => self.scope_decl().map(Item::Scope),
                "struct" => self.struct_decl().map(Item::Struct),
                _ => Err(self.error_here(format!(
                    "expected a top-level declaration, found `{}`",
                    self.peek().kind.label()
                ))),
            },
            _ => Err(self.error_here(format!(
                "expected a top-level declaration, found `{}`",
                self.peek().kind.label()
            ))),
        }
    }

    fn prev_span(&self) -> Span {
        self.toks[self.pos.saturating_sub(1).min(self.toks.len() - 1)].span
    }

    fn globals_block(&mut self) -> Result<Item, Error> {
        let start = self.expect_kw(Kw::Globals)?;
        self.expect_line_end()?;
        let mut vars = Vec::new();
        loop {
            if self.peek().is_kw(Kw::EndGlobals) || self.at_eof() {
                break;
            }
            if matches!(self.peek().kind, Tok::Directive(_)) {
                self.bump();
                continue;
            }
            self.check_fresh_line()?;
            let v = self.var_decl()?;
            self.expect_line_end()?;
            vars.push(v);
        }
        self.expect_kw(Kw::EndGlobals)?;
        self.expect_line_end()?;
        Ok(Item::Globals(vars, start.join(self.prev_span())))
    }

    fn type_decl(&mut self, vis: Vis) -> Result<TypeDecl, Error> {
        let start = self.expect_kw(Kw::Type)?;
        let name = self.expect_ident()?;
        self.expect_kw(Kw::Extends)?;
        let extends = self.expect_ident()?;
        let array = self.eat_kw(Kw::Array);
        self.expect_line_end()?;
        Ok(TypeDecl {
            vis,
            name,
            extends,
            array,
            span: start.join(self.prev_span()),
        })
    }

    /// One variable declaration inside `globals`. `constant` is legal here;
    /// `constant … array` never is (an array has no initialiser).
    fn var_decl(&mut self) -> Result<VarDecl, Error> {
        let start = self.span();
        let vis = if self.eat_word("private") {
            Vis::Private
        } else {
            let _ = self.eat_word("public");
            Vis::Public
        };
        let is_constant = self.eat_kw(Kw::Constant);
        let ty = self.type_ref(true)?;
        if is_constant && ty.array {
            return Err(Error::parse(
                start.join(self.prev_span()),
                "arrays cannot be declared `constant` (an array has no initialiser)",
            ));
        }
        let name = self.expect_ident()?;
        let init = if self.eat(&Tok::Assign) {
            Some(self.expr()?)
        } else {
            None
        };
        Ok(VarDecl {
            vis,
            is_constant,
            ty,
            name,
            init,
            span: start.join(self.prev_span()),
        })
    }

    /// A type reference; `array` is optional where variables allow it.
    fn type_ref(&mut self, allow_array: bool) -> Result<TypeRef, Error> {
        let start = self.span();
        let kind = match self.peek().kind.clone() {
            Tok::Ident(i) => {
                self.bump();
                TypeKind::builtin(&i.name).unwrap_or(TypeKind::Named(i.name))
            }
            _ => {
                return Err(self.error_here(format!(
                    "expected a type, found `{}`",
                    self.peek().kind.label()
                )));
            }
        };
        let array = allow_array && self.eat_kw(Kw::Array);
        Ok(TypeRef {
            kind,
            array,
            span: start.join(self.prev_span()),
        })
    }

    fn param_list(&mut self) -> Result<Vec<Param>, Error> {
        self.expect_kw(Kw::Takes)?;
        let mut params = Vec::new();
        if self.peek().is_kw(Kw::Nothing) {
            self.bump();
            return Ok(params);
        }
        loop {
            let start = self.span();
            let ty = self.type_ref(false)?;
            let name = self.expect_ident()?;
            params.push(Param {
                name,
                ty,
                span: start.join(self.prev_span()),
            });
            if !self.eat(&Tok::Comma) {
                break;
            }
        }
        Ok(params)
    }

    /// `returns <type>` — required, even for `returns nothing`.
    fn return_type(&mut self) -> Result<TypeRef, Error> {
        let start = self.expect_kw(Kw::Returns)?;
        if self.peek().is_kw(Kw::Nothing) {
            let sp = self.bump().span;
            return Ok(TypeRef {
                kind: TypeKind::Nothing,
                array: false,
                span: start.join(sp),
            });
        }
        self.type_ref(false)
    }

    fn native_decl(&mut self, vis: Vis, is_constant: bool) -> Result<NativeDecl, Error> {
        let start = self.expect_kw(Kw::Native)?;
        let name = self.expect_ident()?;
        let params = self.param_list()?;
        let ret = self.return_type()?;
        self.expect_line_end()?;
        Ok(NativeDecl {
            vis,
            is_constant,
            name,
            params,
            ret,
            span: start.join(self.prev_span()),
        })
    }

    fn function_decl(
        &mut self,
        vis: Vis,
        is_constant: bool,
        is_static: bool,
    ) -> Result<FunctionDecl, Error> {
        let start = self.expect_kw(Kw::Function)?;
        let name = self.expect_ident()?;
        let params = self.param_list()?;
        let ret = self.return_type()?;
        self.expect_line_end()?;
        let mut body = Vec::new();
        loop {
            if self.peek().is_kw(Kw::EndFunction) || self.at_eof() {
                break;
            }
            if matches!(self.peek().kind, Tok::Directive(_)) {
                self.bump();
                continue;
            }
            body.push(self.stmt(true)?);
        }
        self.expect_kw(Kw::EndFunction)?;
        self.expect_line_end()?;
        let body = if body.is_empty() { None } else { Some(body) };
        Ok(FunctionDecl {
            vis,
            is_constant,
            is_static,
            name,
            params,
            ret,
            body,
            span: start.join(self.prev_span()),
        })
    }

    // ----- vJass wrapper shapes --------------------------------------

    /// `library NAME [requires …] [uses …] [initializer F] … endlibrary`
    fn library_decl(&mut self) -> Result<LibraryDecl, Error> {
        let start = self.expect_word("library")?;
        let name = self.expect_ident()?;
        let mut requires = Vec::new();
        if self.eat_word("requires") {
            loop {
                requires.push(self.lib_ref()?);
                if !self.eat(&Tok::Comma) {
                    break;
                }
            }
        }
        let uses = if self.eat_word("uses") {
            let mut v = Vec::new();
            loop {
                v.push(self.lib_ref()?);
                if !self.eat(&Tok::Comma) {
                    break;
                }
            }
            Some(v)
        } else {
            None
        };
        let initializer = if self.eat_word("initializer") {
            Some(self.expect_ident()?)
        } else {
            None
        };
        self.expect_line_end()?;
        let body = self.item_block(&["endlibrary"], "library")?;
        Ok(LibraryDecl {
            name,
            requires,
            uses,
            initializer,
            body,
            span: start.join(self.prev_span()),
        })
    }

    fn lib_ref(&mut self) -> Result<LibRef, Error> {
        let optional = self.eat_word("optional");
        let public = self.eat_word("public");
        let name = self.expect_ident()?;
        Ok(LibRef {
            name,
            optional,
            public,
        })
    }

    /// `scope NAME [initializer F] … endscope`
    fn scope_decl(&mut self) -> Result<ScopeDecl, Error> {
        let start = self.expect_word("scope")?;
        let name = self.expect_ident()?;
        let initializer = if self.eat_word("initializer") {
            Some(self.expect_ident()?)
        } else {
            None
        };
        self.expect_line_end()?;
        let body = self.item_block(&["endscope"], "scope")?;
        Ok(ScopeDecl {
            name,
            initializer,
            body,
            span: start.join(self.prev_span()),
        })
    }

    /// `struct NAME … endstruct`. Methods are refused inside: their meaning
    /// is defined by JASSHelper's rewrite, not by vanilla JASS.
    fn struct_decl(&mut self) -> Result<StructDecl, Error> {
        let start = self.expect_word("struct")?;
        let name = self.expect_ident()?;
        self.expect_line_end()?;
        let body = self.item_block(&["endstruct"], "struct")?;
        Ok(StructDecl {
            name,
            body,
            span: start.join(self.prev_span()),
        })
    }

    fn item_block(&mut self, enders: &[&str], what: &str) -> Result<Vec<Item>, Error> {
        let mut body = Vec::new();
        loop {
            if enders.iter().any(|w| self.at_word(w)) || self.at_eof() {
                break;
            }
            if matches!(self.peek().kind, Tok::Directive(_)) {
                self.bump();
                continue;
            }
            body.push(self.item()?);
        }
        if self.at_eof() {
            return Err(self.error_here(format!("unterminated `{what}` block")));
        }
        self.bump(); // the end word
        self.expect_line_end()?;
        Ok(body)
    }

    // ----- statements ------------------------------------------------

    fn stmt(&mut self, fresh_line: bool) -> Result<Stmt, Error> {
        if fresh_line {
            self.check_fresh_line()?;
        }
        let start = self.span();
        match self.peek().kind.clone() {
            Tok::Kw(Kw::Set) => {
                self.bump();
                // Vanilla `set` targets are a name or a name with one index.
                let nstart = self.span();
                let name = self.expect_ident()?;
                let target = if self.eat(&Tok::LBracket) {
                    let index = self.expr()?;
                    let end = self.expect(Tok::RBracket)?;
                    Expr::Index {
                        base: Box::new(Expr::Ident(name, nstart)),
                        index: Box::new(index),
                        span: nstart.join(end),
                    }
                } else {
                    Expr::Ident(name, nstart)
                };
                self.expect(Tok::Assign)?;
                let value = self.expr()?;
                self.expect_line_end()?;
                Ok(Stmt::Set {
                    target,
                    value,
                    span: start.join(self.prev_span()),
                })
            }
            Tok::Kw(Kw::Call) => {
                self.bump();
                let callee = self.expect_ident()?;
                let args = self.call_args()?;
                self.expect_line_end()?;
                Ok(Stmt::Call {
                    callee,
                    args,
                    span: start.join(self.prev_span()),
                })
            }
            Tok::Kw(Kw::Return) => {
                self.bump();
                let value = if self.starts_expr() {
                    Some(self.expr()?)
                } else {
                    None
                };
                self.expect_line_end()?;
                Ok(Stmt::Return(value, start.join(self.prev_span())))
            }
            Tok::Kw(Kw::If) => self.if_stmt(),
            Tok::Kw(Kw::Loop) => {
                self.bump();
                self.expect_line_end()?;
                let body = self.stmt_block(&[Kw::EndLoop], "loop")?;
                self.expect_kw(Kw::EndLoop)?;
                self.expect_line_end()?;
                Ok(Stmt::Loop {
                    body,
                    span: start.join(self.prev_span()),
                })
            }
            Tok::Kw(Kw::ExitWhen) => {
                self.bump();
                let cond = self.expr()?;
                self.expect_line_end()?;
                Ok(Stmt::ExitWhen(cond, start.join(self.prev_span())))
            }
            Tok::Kw(Kw::Local) => {
                self.bump();
                let v = self.local_decl()?;
                self.expect_line_end()?;
                Ok(Stmt::Local(v, start.join(self.prev_span())))
            }
            Tok::Kw(Kw::Constant) => {
                self.bump();
                let mut v = self.local_decl()?;
                v.is_constant = true;
                self.expect_line_end()?;
                Ok(Stmt::Constant(v, start.join(self.prev_span())))
            }
            Tok::Kw(Kw::Debug) => {
                self.bump();
                // `debug` wraps a whole statement on the same line.
                let inner = self.stmt(false)?;
                Ok(Stmt::Debug(Box::new(inner), start.join(self.prev_span())))
            }
            _ => Err(self.error_here(format!(
                "expected a statement, found `{}`",
                self.peek().kind.label()
            ))),
        }
    }

    /// The variable part of a `local` / `constant` statement. One name per
    /// declaration — vanilla has no comma-separated local lists.
    fn local_decl(&mut self) -> Result<VarDecl, Error> {
        let start = self.span();
        let is_constant = self.eat_kw(Kw::Constant);
        let ty = self.type_ref(true)?;
        if is_constant && ty.array {
            return Err(Error::parse(
                start.join(self.prev_span()),
                "arrays cannot be declared `constant`",
            ));
        }
        let name = self.expect_ident()?;
        let init = if self.eat(&Tok::Assign) {
            Some(self.expr()?)
        } else {
            None
        };
        Ok(VarDecl {
            vis: Vis::Public,
            is_constant,
            ty,
            name,
            init,
            span: start.join(self.prev_span()),
        })
    }

    fn if_stmt(&mut self) -> Result<Stmt, Error> {
        let start = self.expect_kw(Kw::If)?;
        let cond = self.expr()?;
        self.expect_kw(Kw::Then)?;
        self.expect_line_end()?;
        let body = self.stmt_block(&[Kw::ElseIf, Kw::Else, Kw::EndIf], "if")?;
        let mut branches = vec![IfBranch {
            cond,
            body,
            span: start.join(self.prev_span()),
        }];
        while self.peek().is_kw(Kw::ElseIf) {
            let bstart = self.bump().span;
            let cond = self.expr()?;
            self.expect_kw(Kw::Then)?;
            self.expect_line_end()?;
            let body = self.stmt_block(&[Kw::ElseIf, Kw::Else, Kw::EndIf], "elseif")?;
            branches.push(IfBranch {
                cond,
                body,
                span: bstart.join(self.prev_span()),
            });
        }
        let else_body = if self.eat_kw(Kw::Else) {
            self.expect_line_end()?;
            Some(self.stmt_block(&[Kw::EndIf], "else")?)
        } else {
            None
        };
        self.expect_kw(Kw::EndIf)?;
        self.expect_line_end()?;
        Ok(Stmt::If {
            branches,
            else_body,
            span: start.join(self.prev_span()),
        })
    }

    /// Statements up to the first ender. The ender must be on its own line;
    /// each statement must be too.
    fn stmt_block(&mut self, enders: &[Kw], what: &str) -> Result<Vec<Stmt>, Error> {
        let mut body = Vec::new();
        loop {
            if enders.iter().any(|k| self.peek().is_kw(*k)) || self.at_eof() {
                break;
            }
            if matches!(self.peek().kind, Tok::Directive(_)) {
                self.bump();
                continue;
            }
            body.push(self.stmt(true)?);
        }
        if self.at_eof() {
            return Err(self.error_here(format!("unterminated `{what}` block")));
        }
        Ok(body)
    }

    fn call_args(&mut self) -> Result<Vec<Expr>, Error> {
        self.expect(Tok::LParen)?;
        let mut args = Vec::new();
        if self.peek().kind != Tok::RParen {
            loop {
                args.push(self.expr()?);
                if !self.eat(&Tok::Comma) {
                    break;
                }
            }
        }
        self.expect(Tok::RParen)?;
        Ok(args)
    }

    // ----- expressions -----------------------------------------------

    fn starts_expr(&self) -> bool {
        match &self.peek().kind {
            Tok::Ident(_)
            | Tok::Int(_)
            | Tok::Real(_)
            | Tok::Str(_)
            | Tok::Raw(_)
            | Tok::LParen
            | Tok::Minus
            | Tok::Plus => true,
            Tok::Kw(k) => matches!(k, Kw::Not | Kw::True | Kw::False | Kw::Null | Kw::Function),
            _ => false,
        }
    }

    fn expr(&mut self) -> Result<Expr, Error> {
        self.binary(0)
    }

    /// Precedence-climbing over the ladder documented at the module head.
    /// `min_prec` of 0 parses the whole expression.
    fn binary(&mut self, min_prec: u8) -> Result<Expr, Error> {
        let mut lhs = self.unary()?;
        loop {
            let Some((op, prec)) = self.peek_binop() else {
                break;
            };
            if prec < min_prec {
                break;
            }
            self.bump();
            let rhs = self.binary(prec + 1)?;
            let span = lhs.span().join(rhs.span());
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        Ok(lhs)
    }

    fn peek_binop(&self) -> Option<(BinOp, u8)> {
        let t = &self.peek().kind;
        Some(match t {
            Tok::Kw(Kw::And) => (BinOp::And, 10),
            Tok::Kw(Kw::Or) => (BinOp::Or, 20),
            Tok::EqEq => (BinOp::Eq, 30),
            Tok::BangEq => (BinOp::Ne, 30),
            Tok::Lt => (BinOp::Lt, 40),
            Tok::Le => (BinOp::Le, 40),
            Tok::Gt => (BinOp::Gt, 40),
            Tok::Ge => (BinOp::Ge, 40),
            Tok::Plus => (BinOp::Add, 60),
            Tok::Minus => (BinOp::Sub, 60),
            Tok::Star => (BinOp::Mul, 70),
            Tok::Slash => (BinOp::Div, 70),
            Tok::Percent => (BinOp::Modulo, 70),
            _ => return None,
        })
    }

    fn unary(&mut self) -> Result<Expr, Error> {
        let start = self.span();
        match self.peek().kind.clone() {
            // `not` reaches over `+`/`-` but stops at the comparison rungs.
            Tok::Kw(Kw::Not) => {
                self.bump();
                let operand = self.binary(60)?;
                Ok(Expr::Unary {
                    op: UnOp::Not,
                    span: start.join(operand.span()),
                    operand: Box::new(operand),
                })
            }
            // Unary `-` / `+` bind tightest: `-a * b` is `(-a) * b`.
            Tok::Minus | Tok::Plus => {
                let op = if self.peek().is(&Tok::Minus) {
                    UnOp::Neg
                } else {
                    UnOp::Pos
                };
                self.bump();
                let operand = self.unary()?;
                Ok(Expr::Unary {
                    op,
                    span: start.join(operand.span()),
                    operand: Box::new(operand),
                })
            }
            _ => self.postfix(),
        }
    }

    fn postfix(&mut self) -> Result<Expr, Error> {
        let mut e = self.primary()?;
        // Vanilla calls and indexes apply to a plain name (`rid(…)`,
        // `rid[…]`), not to arbitrary expressions: `f(x)[i]`, `(f)()` and
        // `function g()` are all rejected.
        if !matches!(e, Expr::Ident(..)) {
            return Ok(e);
        }
        loop {
            match self.peek().kind {
                Tok::LParen => {
                    let start = e.span();
                    let args = self.call_args()?;
                    let end = self.prev_span();
                    e = Expr::Call {
                        callee: Box::new(e),
                        args,
                        span: start.join(end),
                    };
                    break;
                }
                Tok::LBracket => {
                    let start = e.span();
                    self.bump();
                    let index = self.expr()?;
                    let end = self.expect(Tok::RBracket)?;
                    e = Expr::Index {
                        base: Box::new(e),
                        index: Box::new(index),
                        span: start.join(end),
                    };
                    break;
                }
                _ => break,
            }
        }
        Ok(e)
    }

    fn primary(&mut self) -> Result<Expr, Error> {
        let start = self.span();
        match self.peek().kind.clone() {
            Tok::Ident(i) => {
                self.bump();
                Ok(Expr::Ident(i, start))
            }
            Tok::Int(v) => {
                self.bump();
                Ok(Expr::Literal(Literal::Int(v), start))
            }
            Tok::Real(v) => {
                self.bump();
                Ok(Expr::Literal(Literal::Real(v), start))
            }
            Tok::Str(v) => {
                self.bump();
                Ok(Expr::Literal(Literal::Str(v), start))
            }
            Tok::Raw(v) => {
                self.bump();
                Ok(Expr::Literal(Literal::Raw(v), start))
            }
            Tok::Kw(Kw::True) => {
                self.bump();
                Ok(Expr::Literal(Literal::Bool(true), start))
            }
            Tok::Kw(Kw::False) => {
                self.bump();
                Ok(Expr::Literal(Literal::Bool(false), start))
            }
            Tok::Kw(Kw::Null) => {
                self.bump();
                Ok(Expr::Literal(Literal::Null, start))
            }
            // `function Foo` — a `code` value. Parentheses are not part of
            // the form; the checker's `function f(…)` grammar exists only to
            // report a better error and is always rejected.
            Tok::Kw(Kw::Function) => {
                self.bump();
                let name = self.expect_ident()?;
                Ok(Expr::FunctionRef {
                    name,
                    span: start.join(self.prev_span()),
                })
            }
            Tok::LParen => {
                self.bump();
                let inner = self.expr()?;
                let end = self.expect(Tok::RParen)?;
                Ok(Expr::Group(Box::new(inner), start.join(end)))
            }
            // vJass method/field syntax needs JASSHelper to have already
            // rewritten the script, so it is refused loudly here.
            _ => Err(self.error_here(format!(
                "expected an expression, found `{}`",
                self.peek().kind.label()
            ))),
        }
    }
}

//! Printing a [`Script`] back to JASS source.
//!
//! The printer exists so round-trip fidelity can be *proved* rather than
//! asserted: `parse(s).to_source()` must re-parse to the identical tree
//! (compared on the canonical, span-free encoding) and re-lex to the
//! identical token kinds. Three rules keep that true:
//!
//! - Parentheses that were in the source are [`Expr::Group`] nodes, so they
//!   are reproduced verbatim; nothing is re-derived from precedence.
//! - Spacing never creates or removes a token boundary. One statement per
//!   line, exactly as vanilla's line-oriented grammar demands.
//! - Keyword case, whitespace and indentation are normalized; identifiers,
//!   rawcodes, string bytes, integer bases and real lexemes are not.
//!   Directive lines are re-emitted byte for byte from their recorded text.
//!
//! Comments are trivia and are dropped by default. `#` / `//!` / `//#`
//! directives are **not** trivia — they are the map's own tooling
//! instructions — so they are always re-emitted at their original spans,
//! including inside a function body.

use std::fmt::Write as _;

use crate::ast::*;
use crate::lexer::{
    Comment, CommentKind, DirectiveRecord, Ident, IntBase, RawLit, RealLit, StrLit, StrPart,
};

/// Whether [`Script::to_source_with`] re-emits comments.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CommentPolicy {
    /// Code and directives only. This is the form a later stage would hand
    /// back to the parser.
    Drop,
    /// Comments are re-inserted at their original spans, for showing a map
    /// author their own script after a rewrite. The text is preserved; only
    /// the indentation inside a multi-line `/* */` is re-flowed.
    Keep,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PrintOptions {
    pub indent: &'static str,
    pub comments: CommentPolicy,
}

impl Default for PrintOptions {
    fn default() -> PrintOptions {
        PrintOptions {
            indent: "    ",
            comments: CommentPolicy::Drop,
        }
    }
}

struct Out {
    s: String,
    indent: &'static str,
    level: usize,
}

impl Out {
    fn line(&mut self, text: &str) {
        for _ in 0..self.level {
            self.s.push_str(self.indent);
        }
        self.s.push_str(text);
        self.s.push('\n');
    }
}

/// Walks the comment and directive side tables in source order, flushing
/// whatever sits before each construct the printer emits.
struct Tracer<'a> {
    comments: &'a [Comment],
    directives: &'a [DirectiveRecord],
    ci: usize,
    di: usize,
    policy: CommentPolicy,
}

impl<'a> Tracer<'a> {
    fn flush(&mut self, before: u32, out: &mut Out) {
        loop {
            let c_at = self.comments.get(self.ci).map(|c| c.span.start);
            let d_at = self.directives.get(self.di).map(|d| d.span.start);
            let next = match (c_at, d_at) {
                (None, None) => return,
                (Some(a), Some(b)) => a.min(b),
                (Some(a), None) => a,
                (None, Some(b)) => b,
            };
            if next >= before {
                return;
            }
            if d_at == Some(next) {
                // Verbatim: the directive line is the map's own text.
                out.line(&self.directives[self.di].directive.text);
                self.di += 1;
            } else {
                if self.policy == CommentPolicy::Keep {
                    emit_comment(out, &self.comments[self.ci]);
                }
                self.ci += 1;
            }
        }
    }
}

fn emit_comment(out: &mut Out, c: &Comment) {
    match c.kind {
        CommentKind::Line => out.line(&format!("//{}", c.text)),
        CommentKind::Block => {
            if !c.text.contains('\n') && !c.text.contains('\r') {
                out.line(&format!("/*{}*/", c.text));
                return;
            }
            out.line("/*");
            for l in c.text.split('\n') {
                out.line(l.trim_end_matches('\r'));
            }
            out.line("*/");
        }
    }
}

impl Script {
    /// Canonical JASS source for this tree.
    pub fn to_source(&self) -> String {
        self.to_source_with(PrintOptions::default())
    }

    pub fn to_source_with(&self, opts: PrintOptions) -> String {
        let mut out = Out {
            s: String::new(),
            indent: opts.indent,
            level: 0,
        };
        let mut tr = Tracer {
            comments: &self.comments,
            directives: &self.directives,
            ci: 0,
            di: 0,
            policy: opts.comments,
        };
        for item in &self.items {
            let sp = item.span();
            tr.flush(sp.start, &mut out);
            self.emit_item(&mut out, item, opts, &mut tr);
            tr.flush(sp.end, &mut out);
        }
        tr.flush(u32::MAX, &mut out);
        out.s
    }

    fn emit_item(&self, out: &mut Out, item: &Item, opts: PrintOptions, tr: &mut Tracer<'_>) {
        match item {
            Item::Globals(vars, _) => {
                out.line("globals");
                out.level += 1;
                for v in vars {
                    tr.flush(v.span.start, out);
                    out.line(&var_text(v, true));
                }
                out.level -= 1;
                out.line("endglobals");
            }
            Item::Type(t) => {
                let mut s = String::new();
                write_vis(&mut s, t.vis);
                s.push_str("type ");
                s.push_str(t.name.as_str());
                let _ = write!(s, " extends {}", t.extends.as_str());
                if t.array {
                    s.push_str(" array");
                }
                out.line(&s);
            }
            Item::Native(n) => {
                let mut s = String::new();
                write_vis(&mut s, n.vis);
                if n.is_constant {
                    s.push_str("constant ");
                }
                s.push_str("native ");
                s.push_str(n.name.as_str());
                push_params(&mut s, &n.params);
                push_returns(&mut s, &n.ret);
                out.line(&s);
            }
            Item::Function(f) => emit_function(out, f, tr),
            Item::Library(l) => {
                let mut s = String::from("library ");
                s.push_str(l.name.as_str());
                if !l.requires.is_empty() {
                    s.push_str(" requires ");
                    s.push_str(&join_lib_refs(&l.requires));
                }
                if let Some(u) = &l.uses {
                    s.push_str(" uses ");
                    s.push_str(&join_lib_refs(u));
                }
                if let Some(init) = &l.initializer {
                    let _ = write!(s, " initializer {}", init.as_str());
                }
                out.line(&s);
                out.level += 1;
                self.emit_body(out, &l.body, opts, tr);
                out.level -= 1;
                out.line("endlibrary");
            }
            Item::Scope(sc) => {
                let mut s = String::from("scope ");
                s.push_str(sc.name.as_str());
                if let Some(init) = &sc.initializer {
                    let _ = write!(s, " initializer {}", init.as_str());
                }
                out.line(&s);
                out.level += 1;
                self.emit_body(out, &sc.body, opts, tr);
                out.level -= 1;
                out.line("endscope");
            }
            Item::Struct(st) => {
                out.line(&format!("struct {}", st.name.as_str()));
                out.level += 1;
                self.emit_body(out, &st.body, opts, tr);
                out.level -= 1;
                out.line("endstruct");
            }
        }
    }

    fn emit_body(&self, out: &mut Out, body: &[Item], opts: PrintOptions, tr: &mut Tracer<'_>) {
        for it in body {
            tr.flush(it.span().start, out);
            self.emit_item(out, it, opts, tr);
        }
    }
}

fn emit_function(out: &mut Out, f: &FunctionDecl, tr: &mut Tracer<'_>) {
    let mut s = String::new();
    write_vis(&mut s, f.vis);
    if f.is_constant {
        s.push_str("constant ");
    }
    if f.is_static {
        s.push_str("static ");
    }
    s.push_str("function ");
    s.push_str(f.name.as_str());
    push_params(&mut s, &f.params);
    push_returns(&mut s, &f.ret);
    out.line(&s);
    if let Some(body) = &f.body {
        out.level += 1;
        for st in body {
            tr.flush(st.span().start, out);
            emit_stmt(out, st);
        }
        tr.flush(u32::MAX, out);
        out.level -= 1;
    }
    out.line("endfunction");
}

fn emit_stmt(out: &mut Out, s: &Stmt) {
    emit_stmt_pref(out, s, "");
}

/// Emit one statement, prefixing its *first* line with `prefix` (used for
/// `debug`, which wraps a whole statement on one line).
fn emit_stmt_pref(out: &mut Out, s: &Stmt, prefix: &str) {
    let mut pf = prefix;
    match s {
        Stmt::Set { target, value, .. } => {
            out.line(&format!("{pf}set {} = {}", expr(target), expr(value)));
        }
        Stmt::Call { callee, args, .. } => {
            out.line(&format!("{pf}call {}", call_text(callee, args)));
        }
        Stmt::Return(Some(v), _) => out.line(&format!("{pf}return {}", expr(v))),
        Stmt::Return(None, _) => out.line(&format!("{pf}return")),
        Stmt::If {
            branches,
            else_body,
            ..
        } => {
            for (i, b) in branches.iter().enumerate() {
                let kw = if i == 0 { "if" } else { "elseif" };
                out.line(&format!("{pf}{kw} {} then", expr(&b.cond)));
                pf = "";
                out.level += 1;
                for st in &b.body {
                    emit_stmt(out, st);
                }
                out.level -= 1;
            }
            if let Some(eb) = else_body {
                out.line(&format!("{pf}else"));
                pf = "";
                out.level += 1;
                for st in eb {
                    emit_stmt(out, st);
                }
                out.level -= 1;
            }
            out.line(&format!("{pf}endif"));
        }
        Stmt::Loop { body, .. } => {
            out.line(&format!("{pf}loop"));
            out.level += 1;
            for st in body {
                emit_stmt(out, st);
            }
            out.level -= 1;
            out.line("endloop");
        }
        Stmt::ExitWhen(c, _) => out.line(&format!("{pf}exitwhen {}", expr(c))),
        Stmt::Local(v, _) => out.line(&format!("{pf}local {}", var_text(v, true))),
        Stmt::Constant(v, _) => out.line(&format!("{pf}constant {}", var_text(v, false))),
        Stmt::Debug(inner, _) => {
            let p = format!("{pf}debug ");
            emit_stmt_pref(out, inner, &p);
        }
    }
}

fn write_vis(s: &mut String, v: Vis) {
    if let Some(k) = v.keyword() {
        s.push_str(k);
        s.push(' ');
    }
}

fn push_params(s: &mut String, params: &[Param]) {
    s.push_str(" takes ");
    if params.is_empty() {
        s.push_str("nothing");
        return;
    }
    for (i, p) in params.iter().enumerate() {
        if i > 0 {
            s.push_str(", ");
        }
        s.push_str(&type_text(&p.ty));
        s.push(' ');
        s.push_str(p.name.as_str());
    }
}

fn push_returns(s: &mut String, ret: &TypeRef) {
    if matches!(ret.kind, TypeKind::Nothing) {
        s.push_str(" returns nothing");
        return;
    }
    let _ = write!(s, " returns {}", type_text(ret));
}

fn type_text(t: &TypeRef) -> String {
    let mut s = t.kind.name().to_string();
    if t.array {
        s.push_str(" array");
    }
    s
}

/// A variable declaration line. `with_constant` is false where the caller has
/// already written the `constant` keyword (the `constant` statement form).
fn var_text(v: &VarDecl, with_constant: bool) -> String {
    let mut s = String::new();
    write_vis(&mut s, v.vis);
    if v.is_constant && with_constant {
        s.push_str("constant ");
    }
    s.push_str(&type_text(&v.ty));
    s.push(' ');
    s.push_str(v.name.as_str());
    if let Some(init) = &v.init {
        let _ = write!(s, " = {}", expr(init));
    }
    s
}

fn join_lib_refs(v: &[LibRef]) -> String {
    v.iter()
        .map(|r| {
            let mut s = String::new();
            if r.optional {
                s.push_str("optional ");
            }
            if r.public {
                s.push_str("public ");
            }
            s.push_str(r.name.as_str());
            s
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn call_text(callee: &Ident, args: &[Expr]) -> String {
    let mut s = callee.as_str().to_string();
    s.push('(');
    for (i, a) in args.iter().enumerate() {
        if i > 0 {
            s.push_str(", ");
        }
        s.push_str(&expr(a));
    }
    s.push(')');
    s
}

/// Renders an expression, re-parenthesising only where a hand-built tree
/// would otherwise re-associate. An [`Expr::Group`] prints its own
/// parentheses and its child unguarded, so a parsed tree reproduces the
/// map's own text.
pub fn expr(e: &Expr) -> String {
    expr_at(e, 0)
}

fn expr_at(e: &Expr, parent_prec: u8) -> String {
    match e {
        Expr::Ident(i, _) => i.name.clone(),
        Expr::Literal(l, _) => literal_text(l),
        Expr::FunctionRef { name, .. } => format!("function {}", name.as_str()),
        Expr::Group(inner, _) => format!("({})", expr(inner)),
        Expr::Call { callee, args, .. } => {
            let mut s = expr_at(callee, 100);
            s.push('(');
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    s.push_str(", ");
                }
                s.push_str(&expr(a));
            }
            s.push(')');
            s
        }
        Expr::Index { base, index, .. } => {
            let b = expr_at(base, 100);
            format!("{b}[{}]", expr(index))
        }
        Expr::Unary { op, operand, .. } => {
            let prec = e.precedence();
            let o = expr_at(operand, prec);
            match op {
                UnOp::Not => format!("not {o}"),
                UnOp::Neg => format!("-{o}"),
                UnOp::Pos => format!("+{o}"),
            }
        }
        Expr::Binary { op, lhs, rhs, .. } => {
            let prec = e.precedence();
            let l = expr_at(lhs, prec);
            let r = expr_at(rhs, prec + 1);
            let body = format!("{} {} {}", l, op.spelling(), r);
            if prec < parent_prec {
                format!("({body})")
            } else {
                body
            }
        }
    }
}

pub fn literal_text(l: &Literal) -> String {
    match l {
        Literal::Int(v) => match v.base {
            IntBase::Dec => format!("{}", v.value),
            IntBase::Hex => format!("${:X}", v.value),
            IntBase::Octal => format!("0{:o}", v.value),
            IntBase::Bin => format!("%{:b}", v.value),
        },
        Literal::Real(v) => real_text(v),
        Literal::Str(s) => string_text(s),
        Literal::Raw(r) => raw_text(r),
        Literal::Bool(b) => if *b { "true" } else { "false" }.to_string(),
        Literal::Null => "null".to_string(),
    }
}

fn real_text(v: &RealLit) -> String {
    v.text.clone()
}

fn raw_text(v: &RawLit) -> String {
    format!("'{}'", v.text)
}

pub fn string_text(s: &StrLit) -> String {
    let mut out = String::from("\"");
    for p in &s.parts {
        match p {
            StrPart::Text(t) => out.push_str(t),
            StrPart::Escape(e) => out.push_str(e.spelling()),
            StrPart::CodeTag { text } => {
                out.push_str("^^");
                out.push_str(text);
                out.push_str("^^");
            }
        }
    }
    out.push('"');
    out
}

//! Statement execution and expression evaluation.
//!
//! This is the half of the VM that needs no game state: a function body runs,
//! `+`, `if` and `loop` behave, and every native the map asks for goes to the
//! [`Host`]. Timers, unit handles, groups and triggers are host concerns and
//! are deliberately absent.
//!
//! # The host seams
//!
//! [`Host::native`] is where a native this crate refuses gets a chance to run.
//! A host that implements nothing gets the loud error instead, so a native is
//! never silently run as a zero.
//!
//! [`Host::table_string`] is the `war3map.wts` seam. `pcx-trig` decodes that
//! file; this crate does not depend on it, so wiring it is one method:
//!
//! ```ignore
//! impl Host for MyMap {
//!     fn table_string(&self, id: u32) -> Option<String> {
//!         self.wts.body(id).map(|b| b.to_string())
//!     }
//! }
//! ```
//!
//! [`Host::code_tag`] is the `^^…^^` seam. A code tag the host cannot resolve
//! is an error naming the tag, not a dropped tag.

use crate::ast::{BinOp, Expr, Literal, Stmt, StrLit, TypeKind, TypeRef};
use crate::lexer::StrPart;
use crate::natives::{NativeCall, NativeImpl};
use crate::resolve::Program;
use crate::span::Span;
use crate::text::trigstr_id;
use crate::value::{OpError, Value, binary, unary};
use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::Arc;

/// Call depth a program may reach. The engine's own limit is not measured, so
/// this is a harness guard against a stack overflow, not a game rule.
pub const MAX_CALL_DEPTH: u32 = 512;

/// Step budget, so a loop that never exits fails instead of hanging the
/// process. The engine keeps running a runaway loop, so this is a harness
/// guard, not a game rule.
pub const MAX_STEPS: u64 = 20_000_000;

/// Everything the VM cannot decide for itself.
pub trait Host {
    /// A native the map calls that this crate does not implement. Returning
    /// `Err` is the honest default; a host with the game state returns the
    /// map's own value.
    fn native(&mut self, name: &str, args: &[Value]) -> Result<Value, String>;

    /// The body of the map's `war3map.wts` entry `id`, for `TRIGSTR_<id>`.
    fn table_string(&self, id: u32) -> Option<String> {
        let _ = id;
        None
    }

    /// Resolve a table entry while preserving an encoding or container failure.
    /// Existing hosts can continue implementing `table_string`.
    fn resolve_table_string(&self, id: u32) -> Result<Option<String>, String> {
        Ok(self.table_string(id))
    }

    /// What a paired `^^tag^^` in a string substitutes to.
    fn code_tag(&self, tag: &str) -> Option<String> {
        let _ = tag;
        None
    }
}

/// A host with no game state, so every native call and every trigger string is
/// a loud refusal.
pub struct NullHost;

impl Host for NullHost {
    fn native(&mut self, name: &str, _args: &[Value]) -> Result<Value, String> {
        Err(format!("no host implements the native `{name}`"))
    }
}

/// Control flow out of a statement body.
enum Flow {
    Normal,
    Return(Value),
    /// `exitwhen`, out of the innermost `loop`.
    Exit,
}

/// One variable: a scalar or an array. JASS scopes a `local … array` per
/// function, so it belongs in the frame and not in one program-wide table.
enum Slot {
    Scalar(Value),
    Array(BTreeMap<i32, Value>),
}

type Frame = BTreeMap<String, Slot>;

/// A running program.
pub struct Vm<'a, H: Host, P: Deref<Target = Program> = &'a Program> {
    program: P,
    lifetime: PhantomData<&'a Program>,
    host: H,
    globals: Frame,
    steps: u64,
    depth: u32,
    /// Whether `debug` statements run. Defaults to `true`, so a harness sees
    /// them; the release JASS the game ships has them stripped.
    run_debug: bool,
    ready: bool,
    init_error: Option<crate::Error>,
    error_source: Option<String>,
}

/// A VM retaining its resolved program without borrowing its caller.
pub type OwnedVm<H> = Vm<'static, H, Arc<Program>>;

impl<'a, H: Host> Vm<'a, H> {
    pub fn new(program: &'a Program, host: H) -> Vm<'a, H> {
        Self::with_program(program, host)
    }

    pub fn program(&self) -> &'a Program {
        self.program
    }
}

impl<H: Host> OwnedVm<H> {
    pub fn new_owned(program: Arc<Program>, host: H) -> Self {
        Self::with_program(program, host)
    }

    pub fn program(&self) -> &Program {
        &self.program
    }
}

impl<H: Host, P: Deref<Target = Program>> Vm<'_, H, P> {
    fn with_program(program: P, host: H) -> Self {
        Self {
            program,
            lifetime: PhantomData,
            host,
            globals: Frame::new(),
            steps: 0,
            depth: 0,
            run_debug: true,
            ready: false,
            init_error: None,
            error_source: None,
        }
    }

    /// Keep or drop `debug` statements when executing.
    pub fn set_debug(&mut self, on: bool) {
        self.run_debug = on;
    }

    pub fn host(&mut self) -> &mut H {
        &mut self.host
    }

    /// Source owning the most recent execution error, including prelude errors.
    pub fn error_source(&self) -> Option<&str> {
        self.error_source.as_deref()
    }

    /// Run every global initialiser, in declaration order.
    pub fn init_globals(&mut self) -> Result<(), crate::Error> {
        if let Some(error) = &self.init_error {
            return Err(error.clone());
        }
        if self.ready {
            return Ok(());
        }
        self.error_source = None;
        let inits = self
            .program
            .globals()
            .iter()
            .map(|g| {
                (
                    g.name.clone(),
                    g.init.clone(),
                    g.array,
                    g.source.clone(),
                    g.span,
                )
            })
            .collect::<Vec<_>>();
        let mut frame = Frame::new();
        for (name, init, array, source, span) in inits {
            if array && init.is_some() {
                let error = crate::Error::run(
                    span,
                    format!("`{name}` is declared `array`, and an array has no initialiser"),
                );
                self.error_source = Some(source);
                self.init_error = Some(error.clone());
                return Err(error);
            }
            let slot = match init {
                Some(e) => match self.eval(&e, &mut frame) {
                    Ok(value) => Slot::Scalar(value),
                    Err(error) => {
                        if self.error_source.is_none() {
                            self.error_source = Some(source);
                        }
                        self.init_error = Some(error.clone());
                        return Err(error);
                    }
                },
                None if array => Slot::Array(BTreeMap::new()),
                None => Slot::Scalar(Value::Null),
            };
            self.globals.insert(name, slot);
        }
        self.ready = true;
        Ok(())
    }

    /// Read a global, for a caller inspecting a finished run.
    pub fn global(&self, name: &str) -> Value {
        match self.globals.get(name) {
            Some(Slot::Scalar(v)) => v.clone(),
            _ => Value::Null,
        }
    }

    /// Read one slot of an array global.
    pub fn global_at(&self, name: &str, index: i32) -> Value {
        match self.globals.get(name) {
            Some(Slot::Array(a)) => a.get(&index).cloned().unwrap_or(Value::Null),
            _ => Value::Null,
        }
    }

    /// Call a function by its declared name. Arguments match parameters
    /// positionally, which is all JASS has.
    pub fn call(&mut self, name: &str, args: &[Value]) -> Result<Value, crate::Error> {
        self.init_globals()?;
        self.error_source = None;
        self.enter(name, args.to_vec(), Span::EMPTY)
    }

    fn enter(&mut self, name: &str, argv: Vec<Value>, span: Span) -> Result<Value, crate::Error> {
        let f = self
            .program
            .function(name)
            .ok_or_else(|| {
                crate::Error::run(span, format!("`{name}` is not a function in this program"))
            })?
            .clone();
        if f.params.len() != argv.len() {
            let span = if span == Span::EMPTY {
                self.error_source = Some(f.source.clone());
                f.span
            } else {
                span
            };
            return Err(crate::Error::run(
                span,
                format!(
                    "`{name}` takes {} argument{}, {} given",
                    f.params.len(),
                    plural(f.params.len()),
                    argv.len()
                ),
            ));
        }
        if self.depth >= MAX_CALL_DEPTH {
            return Err(crate::Error::run(
                span,
                format!("`{name}` recursed past {MAX_CALL_DEPTH} frames"),
            ));
        }
        let mut frame = Frame::new();
        for (p, v) in f.params.iter().zip(&argv) {
            frame.insert(p.name.as_str().to_string(), Slot::Scalar(v.clone()));
        }
        self.depth += 1;
        let body = f.body.unwrap_or_default();
        let out = match self.run_body(&body, &mut frame) {
            Ok(Flow::Return(v)) => Ok(v),
            Ok(_) => self.implicit_return(&f.ret, name, f.span),
            Err(e) => Err(e),
        };
        self.depth -= 1;
        if out.is_err() && self.error_source.is_none() {
            self.error_source = Some(f.source.clone());
        }
        out
    }

    /// A function that reaches `endfunction` without a `return`.
    fn implicit_return(
        &self,
        ret: &TypeRef,
        name: &str,
        span: Span,
    ) -> Result<Value, crate::Error> {
        match ret.kind {
            TypeKind::Nothing => Ok(Value::Null),
            _ => Err(crate::Error::run(
                span,
                format!(
                    "`{name}` declares it returns {} and reaches `endfunction` without a value",
                    ret.kind.name()
                ),
            )),
        }
    }

    fn run_body(&mut self, stmts: &[Stmt], frame: &mut Frame) -> Result<Flow, crate::Error> {
        for st in stmts {
            match self.step(st, frame)? {
                Flow::Normal => {}
                other => return Ok(other),
            }
        }
        Ok(Flow::Normal)
    }

    fn tick(&mut self, span: Span) -> Result<(), crate::Error> {
        self.steps += 1;
        if self.steps > MAX_STEPS {
            return Err(crate::Error::run(
                span,
                format!("the program ran more than {MAX_STEPS} steps without finishing"),
            ));
        }
        Ok(())
    }

    fn step(&mut self, st: &Stmt, frame: &mut Frame) -> Result<Flow, crate::Error> {
        self.tick(st.span())?;
        match st {
            Stmt::Local(v, _) | Stmt::Constant(v, _) => {
                if v.ty.array && v.init.is_some() {
                    return Err(crate::Error::run(
                        v.span,
                        format!(
                            "`{}` is declared `array`, and an array has no initialiser",
                            v.name.as_str()
                        ),
                    ));
                }
                let slot = match &v.init {
                    Some(e) => Slot::Scalar(self.eval(e, frame)?),
                    None => {
                        if v.ty.array {
                            Slot::Array(BTreeMap::new())
                        } else {
                            Slot::Scalar(Value::Null)
                        }
                    }
                };
                frame.insert(v.name.as_str().to_string(), slot);
                Ok(Flow::Normal)
            }
            Stmt::Set { target, value, .. } => {
                let rhs = self.eval(value, frame)?;
                self.assign(target, rhs, frame)
            }
            Stmt::Call { callee, args, span } => {
                self.eval_call(callee, args, frame, *span)?;
                Ok(Flow::Normal)
            }
            Stmt::Return(e, _) => Ok(Flow::Return(match e {
                Some(e) => self.eval(e, frame)?,
                None => Value::Null,
            })),
            Stmt::ExitWhen(e, _) => {
                let c = self.eval(e, frame)?;
                Ok(if self.truth(&c, e.span())? {
                    Flow::Exit
                } else {
                    Flow::Normal
                })
            }
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                for b in branches {
                    let c = self.eval(&b.cond, frame)?;
                    if self.truth(&c, b.cond.span())? {
                        return self.run_body(&b.body, frame);
                    }
                }
                match else_body {
                    Some(body) => self.run_body(body, frame),
                    None => Ok(Flow::Normal),
                }
            }
            Stmt::Loop { body, .. } => {
                loop {
                    self.tick(st.span())?;
                    match self.run_body(body, frame)? {
                        Flow::Normal => {}
                        Flow::Exit => break,
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                    }
                }
                Ok(Flow::Normal)
            }
            Stmt::Debug(inner, _) => {
                if !self.run_debug {
                    return Ok(Flow::Normal);
                }
                self.step(inner, frame)
            }
        }
    }

    fn assign(
        &mut self,
        target: &Expr,
        value: Value,
        frame: &mut Frame,
    ) -> Result<Flow, crate::Error> {
        let (name, idx) = match target {
            Expr::Ident(name, _) => (name, None),
            Expr::Index { base, index, .. } => {
                let Expr::Ident(name, _) = base.as_ref() else {
                    return Err(crate::Error::run(
                        target.span(),
                        "only a variable name or an array slot can be assigned to",
                    ));
                };
                let i = self.eval(index, frame)?;
                (name, Some(self.index_of(&i, index.span())?))
            }
            _ => {
                return Err(crate::Error::run(
                    target.span(),
                    "only a variable name or an array slot can be assigned to",
                ));
            }
        };
        // A local shadows a global of the same name, so the frame is the first
        // place to look, exactly as a read does.
        if let Some(slot) = frame.get_mut(name.as_str()) {
            return store(slot, idx, value, target.span());
        }
        match self.globals.get_mut(name.as_str()) {
            Some(slot) => store(slot, idx, value, target.span()),
            None => Err(crate::Error::run(
                target.span(),
                format!("`{}` is not a variable in this program", name.as_str()),
            )),
        }
    }

    fn index_of(&self, v: &Value, span: Span) -> Result<i32, crate::Error> {
        match v {
            Value::Int(i) => Ok(*i),
            _ => Err(crate::Error::run(
                span,
                format!("an array index needs an integer, got {}", v.type_name()),
            )),
        }
    }

    fn truth(&self, v: &Value, span: Span) -> Result<bool, crate::Error> {
        v.as_bool()
            .ok_or_else(|| self.op_error(&OpError::Logic(v.class()), span))
    }

    // ----- expressions ---------------------------------------------------

    fn eval(&mut self, e: &Expr, frame: &mut Frame) -> Result<Value, crate::Error> {
        self.tick(e.span())?;
        match e {
            Expr::Literal(l, span) => self.literal(l, *span),
            Expr::Ident(name, span) => self.eval_ident(name, *span, frame),
            Expr::Group(inner, _) => self.eval(inner, frame),
            Expr::Unary { op, operand, span } => {
                let v = self.eval(operand, frame)?;
                unary(*op, &v).map_err(|e| self.op_error(&e, *span))
            }
            Expr::Binary { op, lhs, rhs, span } => {
                if *op == BinOp::And {
                    let a = self.eval(lhs, frame)?;
                    if !self.truth(&a, lhs.span())? {
                        return Ok(Value::Bool(false));
                    }
                    let b = self.eval(rhs, frame)?;
                    return self.truth(&b, rhs.span()).map(Value::Bool);
                }
                if *op == BinOp::Or {
                    let a = self.eval(lhs, frame)?;
                    if self.truth(&a, lhs.span())? {
                        return Ok(Value::Bool(true));
                    }
                    let b = self.eval(rhs, frame)?;
                    return self.truth(&b, rhs.span()).map(Value::Bool);
                }
                let a = self.eval(lhs, frame)?;
                let b = self.eval(rhs, frame)?;
                binary(*op, &a, &b).map_err(|e| self.op_error(&e, *span))
            }
            Expr::Call { callee, args, span } => {
                let Expr::Ident(name, _) = callee.as_ref() else {
                    return Err(crate::Error::run(
                        *span,
                        "a call needs a name: only a function or a native can be called",
                    ));
                };
                self.eval_call(name, args, frame, *span)
            }
            Expr::Index { base, index, span } => {
                let Expr::Ident(name, _) = base.as_ref() else {
                    return Err(crate::Error::run(
                        *span,
                        "an array slot needs an array name",
                    ));
                };
                let i = self.eval(index, frame)?;
                let idx = self.index_of(&i, index.span())?;
                let slot = frame
                    .get(name.as_str())
                    .or_else(|| self.globals.get(name.as_str()));
                match slot {
                    Some(Slot::Array(a)) => Ok(a.get(&idx).cloned().unwrap_or(Value::Null)),
                    Some(Slot::Scalar(_)) => Err(crate::Error::run(
                        *span,
                        format!("`{}` is not an array", name.as_str()),
                    )),
                    None => Err(crate::Error::run(
                        *span,
                        format!("`{}` is not a variable in this program", name.as_str()),
                    )),
                }
            }
            Expr::FunctionRef { name, span } => self
                .program
                .function_id(name.as_str())
                .map(Value::Code)
                .ok_or_else(|| {
                    crate::Error::run(
                        *span,
                        format!(
                            "`function {}` needs a function, and `{}` is not one",
                            name.as_str(),
                            name.as_str()
                        ),
                    )
                }),
        }
    }

    fn eval_ident(
        &mut self,
        name: &crate::ast::Ident,
        span: Span,
        frame: &mut Frame,
    ) -> Result<Value, crate::Error> {
        if let Some(slot) = frame
            .get(name.as_str())
            .or_else(|| self.globals.get(name.as_str()))
        {
            return match slot {
                Slot::Scalar(v) => Ok(v.clone()),
                Slot::Array(_) => Err(crate::Error::run(
                    span,
                    format!("`{}` is an array and needs an index", name.as_str()),
                )),
            };
        }
        // A trigger string spelled as an identifier.
        if let Some(id) = trigstr_id(name.as_str()) {
            return self.table_string(id, span);
        }
        if self.program.function(name.as_str()).is_some()
            || self.program.natives().get(name.as_str()).is_some()
        {
            return Err(crate::Error::run(
                span,
                format!(
                    "`{}` is a function, and a function is not a value",
                    name.as_str()
                ),
            ));
        }
        Err(crate::Error::run(
            span,
            format!("`{}` is not a variable in this program", name.as_str()),
        ))
    }

    fn eval_call(
        &mut self,
        name: &crate::ast::Ident,
        args: &[Expr],
        frame: &mut Frame,
        span: Span,
    ) -> Result<Value, crate::Error> {
        if matches!(frame.get(name.as_str()), Some(Slot::Scalar(_))) {
            return Err(crate::Error::run(
                span,
                format!(
                    "`{}` is a variable, and a variable cannot be called",
                    name.as_str()
                ),
            ));
        }
        let mut argv: Vec<Value> = Vec::with_capacity(args.len());
        for a in args {
            argv.push(self.eval(a, frame)?);
        }
        if self.program.natives().ambiguous(name.as_str()).is_some() {
            return Err(crate::Error::run(
                span,
                format!(
                    "`{}` is declared with two different signatures, so which one this call means cannot be told",
                    name.as_str()
                ),
            ));
        }
        match self.program.natives().call(name.as_str()) {
            Some(NativeCall::Implemented(n, which)) => {
                if n.params.len() != argv.len() {
                    return Err(crate::Error::run(
                        span,
                        format!(
                            "native `{}` takes {} argument{}, {} given",
                            name.as_str(),
                            n.params.len(),
                            plural(n.params.len()),
                            argv.len()
                        ),
                    ));
                }
                self.run_native(which, &argv, span)
            }
            Some(NativeCall::SignatureMismatch(n, which)) => {
                let signature = crate::natives::signature_of(&n.name, &n.params, &n.ret);
                Err(crate::Error::run(
                    span,
                    format!(
                        "native `{}` cannot dispatch {which:?}: supplied declaration `{signature}` at {}:{}..{} is incompatible with the implemented conversion",
                        n.name, n.source, n.span.start, n.span.end
                    ),
                ))
            }
            Some(NativeCall::Unimplemented(n)) => {
                if n.params.len() != argv.len() {
                    return Err(crate::Error::run(
                        span,
                        format!(
                            "native `{}` takes {} argument{}, {} given",
                            name.as_str(),
                            n.params.len(),
                            plural(n.params.len()),
                            argv.len()
                        ),
                    ));
                }
                // The host owns every native the table cannot run. When it
                // has none, the error names the native, its declaration and
                // the span of the call, which is what PLAN.md requires.
                let signature = crate::natives::signature_of(&n.name, &n.params, &n.ret);
                self.host.native(name.as_str(), &argv).map_err(|why| {
                    crate::Error::run(
                        span,
                        format!(
                            "native `{}` is not implemented; it is declared as `{signature}` at {}:{}..{}: {why}",
                            name.as_str(),
                            n.source,
                            n.span.start,
                            n.span.end
                        ),
                    )
                })
            }
            _ => {
                if self.program.function(name.as_str()).is_some() {
                    return self.enter(name.as_str(), argv, span);
                }
                Err(crate::Error::run(
                    span,
                    format!(
                        "`{}` is not declared as a function or native in the loaded script sources",
                        name.as_str()
                    ),
                ))
            }
        }
    }

    fn run_native(
        &self,
        which: NativeImpl,
        args: &[Value],
        span: Span,
    ) -> Result<Value, crate::Error> {
        match which {
            NativeImpl::IntToReal => match args[0] {
                Value::Int(i) => Ok(Value::Real(i as f32)),
                ref v => Err(crate::Error::run(
                    span,
                    format!("I2R takes an integer, got {}", v.type_name()),
                )),
            },
            NativeImpl::RealToInt => match args[0] {
                // Truncation toward zero, which `Blizzard.j`'s `ModuloReal`
                // proves by computing a ring remainder with it.
                Value::Real(r) => Ok(Value::Int(r.trunc() as i32)),
                ref v => Err(crate::Error::run(
                    span,
                    format!("R2I takes a real, got {}", v.type_name()),
                )),
            },
        }
    }

    fn table_string(&mut self, id: u32, span: Span) -> Result<Value, crate::Error> {
        let body = self
            .host
            .resolve_table_string(id)
            .map_err(|why| crate::Error::run(span, format!("TRIGSTR_{id}: {why}")))?
            .ok_or_else(|| {
                crate::Error::run(
                    span,
                    format!("TRIGSTR_{id} is not in this map's war3map.wts"),
                )
            })?;
        Ok(Value::Str(self.expand_tags(body, span)?))
    }

    /// Expand every paired `^^tag^^` in a string, through the host. A tag with
    /// no substitution is left as written, so nothing silently disappears from
    /// a tooltip.
    fn expand_tags(&self, body: String, span: Span) -> Result<String, crate::Error> {
        if !body.contains("^^") {
            return Ok(body);
        }
        let mut out = String::with_capacity(body.len());
        let mut rest = body.as_str();
        while let Some(open) = rest.find("^^") {
            out.push_str(&rest[..open]);
            let after = &rest[open + 2..];
            match after.find("^^") {
                Some(close) => {
                    let tag = &after[..close];
                    match self.host.code_tag(tag) {
                        Some(sub) => out.push_str(&sub),
                        None => {
                            return Err(crate::Error::run(
                                span,
                                format!("no substitution for the `^{tag}^` code tag"),
                            ));
                        }
                    }
                    rest = &after[close + 2..];
                }
                None => {
                    out.push_str(&rest[open..]);
                    return Ok(out);
                }
            }
        }
        out.push_str(rest);
        Ok(out)
    }

    fn literal(&mut self, l: &Literal, span: Span) -> Result<Value, crate::Error> {
        match l {
            Literal::Int(i) => Ok(Value::Int(Value::wrap32(i.value))),
            Literal::Real(r) => Ok(Value::Real(r.value as f32)),
            Literal::Bool(b) => Ok(Value::Bool(*b)),
            Literal::Null => Ok(Value::Null),
            Literal::Raw(rc) => Ok(Value::Int(Value::wrap32(rawcode_int(&rc.text)))),
            Literal::Str(s) => self.string(s, span),
        }
    }

    /// A string literal: a `TRIGSTR_<n>` spelling resolves through the map's
    /// own table, and a paired `^^…^^` expands through the host.
    fn string(&mut self, s: &StrLit, span: Span) -> Result<Value, crate::Error> {
        let has_tag = s.parts.iter().any(|p| matches!(p, StrPart::CodeTag { .. }));
        let decoded = s.decoded();
        if !has_tag && let Some(id) = trigstr_id(&decoded) {
            let body = self
                .host
                .resolve_table_string(id)
                .map_err(|why| crate::Error::run(span, format!("TRIGSTR_{id}: {why}")))?
                .ok_or_else(|| {
                    crate::Error::run(
                        span,
                        format!("TRIGSTR_{id} is not in this map's war3map.wts"),
                    )
                })?;
            return Ok(Value::Str(self.expand_tags(body, span)?));
        }
        if has_tag {
            let mut out = String::new();
            for p in &s.parts {
                match p {
                    StrPart::Text(t) => out.push_str(t),
                    StrPart::Escape(e) => out.push(e.decoded()),
                    StrPart::CodeTag { text } => match self.host.code_tag(text) {
                        Some(sub) => out.push_str(&sub),
                        None => {
                            return Err(crate::Error::run(
                                span,
                                format!("no substitution for the `^{text}^` code tag"),
                            ));
                        }
                    },
                }
            }
            return Ok(Value::Str(out));
        }
        Ok(Value::Str(decoded))
    }

    fn op_error(&self, e: &OpError, span: Span) -> crate::Error {
        crate::Error::run(span, e.message())
    }
}

/// The integer a rawcode literal denotes.
///
/// The four characters are read in text order with the *last* character in the
/// low byte, so `'A070' + 1` is `'A071'`. That is what the corpus proves: the
/// nine sites that enumerate object ids that way (`MAP_412_SwatAfter134w.j`'s
/// `'A070'+iInt`, `'A023'+idx`, `'I031'+…`) only work if the fourth character
/// is the low one. The same convention is the only one that makes the object
/// editor generate a numerically larger id from the reversed original, which
/// is why `pcx-objects` finds `'hfoo'` cases whose new id is `hfoo` reversed.
///
/// `'A'`, the one-character spelling, is the ASCII value of that character.
pub fn rawcode_int(text: &str) -> i64 {
    if let Some(rest) = text.strip_prefix('\\') {
        return match rest {
            "n" => 10,
            "r" => 13,
            "t" => 9,
            "b" => 8,
            "f" => 12,
            "\"" => 34,
            "'" => 39,
            "\\" => 92,
            _ => 0,
        };
    }
    let bytes = text.as_bytes();
    if bytes.len() == 1 {
        return i64::from(bytes[0]);
    }
    let mut v: i64 = 0;
    for c in bytes.iter().take(4) {
        v = (v << 8) | i64::from(*c);
    }
    v
}

/// Write a value into one slot. An indexed write needs an array; a plain write
/// needs a scalar, so `set arr = …` is a refusal rather than a silent drop.
fn store(
    slot: &mut Slot,
    idx: Option<i32>,
    value: Value,
    span: Span,
) -> Result<Flow, crate::Error> {
    match (slot, idx) {
        (Slot::Array(a), Some(i)) => {
            a.insert(i, value);
            Ok(Flow::Normal)
        }
        (Slot::Array(_), None) => Err(crate::Error::run(
            span,
            "an array needs an index; `set arrayName = …` is not a JASS assignment",
        )),
        (Slot::Scalar(s), None) => {
            *s = value;
            Ok(Flow::Normal)
        }
        (Slot::Scalar(_), Some(_)) => Err(crate::Error::run(span, "this variable is not an array")),
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

/// Run a script's `main`, initialising globals first. A script whose entry
/// point is elsewhere is invoked through [`Vm::call`] instead.
pub fn run_main<H: Host>(program: &Program, host: H) -> Result<(), crate::Error> {
    let mut vm = Vm::new(program, host);
    vm.init_globals()?;
    let _ = vm.call("main", &[])?;
    Ok(())
}

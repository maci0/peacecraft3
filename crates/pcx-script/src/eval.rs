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
//! [`Host::suspend`] is consulted first for that same native. Returning `true`
//! parks the thread and does not call [`Host::native`] for that invocation, so
//! a parked native is not a successful value. [`Vm::invoke`] and [`Vm::resume`]
//! keep the parked thread. [`Vm::call`] still returns a value only when the
//! call runs to completion; if the host parks, `call` returns an error and the
//! thread stays parked.
//!
//! [`Vm::resume`] supplies the parked native's value. A `call` statement
//! discards that value and continues at the next statement. A park in an
//! operand, an argument or a nested call uses the value as the native's result
//! and does not evaluate earlier operands or arguments again. `and` and `or`
//! still skip the right operand when the left operand decides the result.
//! When the parked native is declared `real`, the resumed integer is converted
//! before that result is used.
//!
//! Globals, locals, the call stack, spans and the step counter survive a park.
//! [`Vm::init_globals`] does not park. A suspend while initialising globals is
//! an error, and that error is reused on the next attempt.
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

use crate::ast::{BinOp, Expr, IfBranch, Literal, Param, Stmt, StrLit, TypeKind, TypeRef, UnOp};
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

/// Threads one native may start inside another before the first finishes or
/// parks. A harness guard against host recursion, not a game rule.
pub const MAX_SPAWN_DEPTH: u32 = 64;

/// Step budget per entry (call, new thread or resumed thread), so a loop that never exits fails instead of hanging the
/// process. The engine keeps running a runaway loop, so this is a harness
/// guard, not a game rule.
pub const MAX_STEPS: u64 = 20_000_000;

/// Arithmetic and literal conversion qualified by the caller's selected runtime.
pub trait RealOps {
    fn literal(&self, spelling: &str) -> Result<f32, String>;
    fn integer(&self, value: i32) -> Result<f32, String>;
    fn binary(&self, op: BinOp, left: f32, right: f32) -> Result<Value, String>;
}

/// A host native calling back into the same script thread and global state.
#[derive(Debug)]
pub enum ScriptCall {
    Call(crate::value::FunctionId),
    Return(Value),
    /// Run these callbacks in order as one new thread, then return `null` to the
    /// caller. A park inside that thread stores it under [`Host::park_thread`]'s
    /// ticket and the caller continues. Either way the VM then calls
    /// [`Host::leave_thread`].
    Spawn(Vec<crate::value::FunctionId>),
}

/// Everything the VM cannot decide for itself.
pub trait Host {
    /// Absent preserves the interpreter's ordinary host-float operations.
    ///
    /// Only an integer crossing a non-array `real` declaration is converted:
    /// through [`RealOps::integer`] when this is present, and through the same
    /// `as f32` cast as unbound `I2R` when it is absent.
    fn real_ops(&self) -> Option<&dyn RealOps> {
        None
    }
    /// A native the map calls that this crate does not implement. Returning
    /// `Err` is the honest default; a host with the game state returns the
    /// map's own value.
    ///
    /// This is not called for an invocation that [`Host::suspend`] parks.
    fn native(&mut self, name: &str, args: &[Value]) -> Result<Value, String>;

    /// Optional callback dispatch. `None` uses ordinary native dispatch.
    /// A callback takes no arguments; its return reaches `continue_script_call`.
    fn begin_script_call(
        &mut self,
        _name: &str,
        _args: &[Value],
    ) -> Result<Option<ScriptCall>, String> {
        Ok(None)
    }

    fn continue_script_call(&mut self, _value: Value) -> Result<ScriptCall, String> {
        Err("no native script callback is active".into())
    }

    /// Unwind callback contexts after an execution error, preserving prior effects.
    fn abort_script_calls(&mut self) {}

    /// `true` parks the running thread instead of producing a value.
    ///
    /// Existing hosts never park. The arguments are the values already
    /// evaluated for this call, left to right. The VM does not read a duration
    /// and does not schedule anything. [`Vm::resume`] supplies the value this
    /// native produces when the thread continues.
    fn suspend(&mut self, _name: &str, _args: &[Value]) -> bool {
        false
    }

    /// A thread parked at a thread boundary. Return a fresh ticket that a later
    /// [`Vm::resume_thread`] presents; the host keeps that thread's own context.
    fn park_thread(&mut self) -> Result<u64, String> {
        Err("this host cannot park a script thread".into())
    }

    /// Install the context saved for `ticket` before its thread continues.
    fn resume_thread(&mut self, _ticket: u64) -> Result<(), String> {
        Err("this host cannot resume a script thread".into())
    }

    /// A spawned or resumed thread finished or parked; restore the caller's context.
    fn leave_thread(&mut self) -> Result<(), String> {
        Err("this host cannot leave a script thread".into())
    }

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

/// Outcome of [`Vm::invoke`] and [`Vm::resume`].
///
/// [`Vm::call`] stays `Result<Value, Error>` and finishes only when the host
/// does not suspend.
#[derive(Clone, PartialEq, Debug)]
#[must_use = "a suspended thread is not a finished value"]
pub enum Exec {
    Finished(Value),
    Suspended,
}

/// One variable: a scalar or an array, plus the type its declaration spelled.
/// JASS scopes a `local … array` per function, so it belongs in the frame and
/// not in one program-wide table.
enum Slot {
    Scalar(Value, TypeKind),
    Array(BTreeMap<i32, Value>, TypeKind),
}

type Frame = BTreeMap<String, Slot>;

/// Control produced while executing a statement list.
enum Flow {
    Normal,
    Return(Value),
    /// `exitwhen`, out of the innermost `loop`.
    Exit,
    /// The statement finished by parking. Continue at the next statement.
    Yield,
    /// The statement is parked inside its own expression.
    Pause(Box<PausedExpr>),
    /// The statement is parked inside a nested body.
    Park(Box<Nested>),
    /// A child activation is already on the stack.
    Invoke {
        after: Box<PausedExpr>,
        nested: Option<Box<Nested>>,
    },
    /// Saved directly on an activation.
    Suspend(Box<Cont>),
    /// Saved directly on an activation. The child is already pushed.
    Switch {
        cont: Box<Cont>,
        after: Box<PausedExpr>,
    },
}

/// Statement cursor. `paused` is an expression hole in this statement.
/// `nested` is a hole in a child body of this statement. Only one is set.
#[derive(Debug, Default)]
struct Cont {
    at: usize,
    nested: Option<Box<Nested>>,
    paused: Option<Box<PausedExpr>>,
}

#[derive(Debug)]
struct Nested {
    kind: Nest,
    cont: Cont,
}

#[derive(Clone, Copy, Debug)]
enum Nest {
    IfBranch(usize),
    IfElse,
    Loop,
    Debug,
}

/// An expression waiting for one value, then the statement that consumes it.
#[derive(Debug)]
struct PausedExpr {
    dest: ExprDest,
    /// Outermost frame first. `pop` yields the frame nearest the hole.
    zip: Vec<Zip>,
}

#[derive(Debug)]
enum ExprDest {
    Bind {
        name: String,
        ty: TypeKind,
    },
    Set {
        target: Expr,
        /// `None` while the right-hand side is still the hole.
        rhs: Option<Value>,
        /// Span of the index expression, set once the right-hand side is known.
        index_span: Option<Span>,
    },
    /// A statement `call`. The value is discarded.
    Discard,
    Return,
    ExitWhen {
        span: Span,
    },
    IfCond {
        branch: usize,
        span: Span,
    },
}

/// One operator waiting above the hole. The nearest frame is at the back.
#[derive(Debug)]
enum Zip {
    ScriptCall {
        span: Span,
    },
    Unary {
        op: UnOp,
        span: Span,
    },
    /// The left operand is the hole. `rhs` has not been evaluated.
    Left {
        op: BinOp,
        span: Span,
        left_span: Span,
        rhs: Expr,
    },
    /// The right operand is the hole. `left` already has a value.
    Right {
        op: BinOp,
        span: Span,
        left: Value,
        rhs_span: Span,
    },
    /// Argument `got.len()` is the hole. `rest` are the arguments after it.
    Arg {
        callee: String,
        span: Span,
        got: Vec<Value>,
        rest: Arc<[Expr]>,
        next: usize,
    },
    Index {
        name: String,
        span: Span,
        index_span: Span,
    },
    /// The hole is a native whose declaration returns `real`.
    DeclaredReal {
        span: Span,
    },
}

struct ArgSlice<'a> {
    expressions: &'a [Expr],
    retained: Option<(&'a Arc<[Expr]>, usize)>,
}

enum ExprOut {
    Value(Value),
    Park(Vec<Zip>),
    /// A user function was pushed. `zip` applies to its return value.
    Push(Vec<Zip>),
}

/// A thread parked at a thread boundary, with the callbacks it has still to run.
struct Thread {
    stack: Vec<Activation>,
    depth: u32,
    remaining: std::collections::VecDeque<crate::value::FunctionId>,
}

struct Activation {
    name: String,
    source: String,
    ret: TypeRef,
    span: Span,
    body: Arc<[Stmt]>,
    frame: Frame,
    cont: Cont,
    after_call: Option<Box<PausedExpr>>,
    incoming: Option<Value>,
}

enum TopStop {
    Finished(Value),
    Suspended,
    Switched,
}

/// A running program.
pub struct Vm<'a, H: Host, P: Deref<Target = Program> = &'a Program> {
    program: P,
    lifetime: PhantomData<&'a Program>,
    host: H,
    globals: Frame,
    /// Parked and running activations, outermost first. Empty means idle.
    stack: Vec<Activation>,
    steps: u64,
    depth: u32,
    /// Whether `debug` statements run. Defaults to `true`, so a harness sees
    /// them; the release JASS the game ships has them stripped.
    run_debug: bool,
    ready: bool,
    init_error: Option<crate::Error>,
    error_source: Option<String>,
    /// Set for the duration of `invoke`, `resume` and `init_globals`.
    busy: bool,
    suspended_span: Option<Span>,
    /// Parked threads by host ticket.
    parked: BTreeMap<u64, Thread>,
    spawn_depth: u32,
    #[cfg(test)]
    copied_body_statements: usize,
    #[cfg(test)]
    copied_argument_expressions: usize,
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
            stack: Vec::new(),
            steps: 0,
            depth: 0,
            run_debug: true,
            ready: false,
            init_error: None,
            error_source: None,
            busy: false,
            suspended_span: None,
            parked: BTreeMap::new(),
            spawn_depth: 0,
            #[cfg(test)]
            copied_body_statements: 0,
            #[cfg(test)]
            copied_argument_expressions: 0,
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

    /// Steps taken since this VM was created. A park does not reset the count.
    #[cfg(test)]
    pub(crate) fn steps(&self) -> u64 {
        self.steps
    }

    /// Live activations. A park does not reset the depth.
    #[cfg(test)]
    pub(crate) fn depth(&self) -> u32 {
        self.depth
    }

    #[cfg(test)]
    pub(crate) fn set_steps_for_test(&mut self, steps: u64) {
        self.steps = steps;
    }

    #[cfg(test)]
    pub(crate) fn set_busy_for_test(&mut self, busy: bool) {
        self.busy = busy;
    }

    #[cfg(test)]
    pub(crate) fn copied_body_statements(&self) -> usize {
        self.copied_body_statements
    }

    #[cfg(test)]
    pub(crate) fn copied_argument_expressions(&self) -> usize {
        self.copied_argument_expressions
    }

    /// Run every global initialiser, in declaration order.
    ///
    /// A native that asks to suspend here is refused. The VM is left idle, the
    /// error is saved, and a later call returns that same error.
    pub fn init_globals(&mut self) -> Result<(), crate::Error> {
        self.refuse_reentry()?;
        if let Some(error) = &self.init_error {
            return Err(error.clone());
        }
        if self.ready {
            return Ok(());
        }
        self.busy = true;
        let result = self.initialise_globals();
        self.busy = false;
        result
    }

    /// Read a global, for a caller inspecting a finished run.
    pub fn global(&self, name: &str) -> Value {
        match self.globals.get(name) {
            Some(Slot::Scalar(v, _)) => v.clone(),
            _ => Value::Null,
        }
    }

    /// Read one slot of an array global.
    pub fn global_at(&self, name: &str, index: i32) -> Value {
        match self.globals.get(name) {
            Some(Slot::Array(a, _)) => a.get(&index).cloned().unwrap_or(Value::Null),
            _ => Value::Null,
        }
    }

    /// Call a function by its declared name. Arguments match parameters
    /// positionally, which is all JASS has.
    ///
    /// A host that suspends must use [`Vm::invoke`] and [`Vm::resume`]. If this
    /// call parks, the error names that fact and the thread stays parked.
    pub fn call(&mut self, name: &str, args: &[Value]) -> Result<Value, crate::Error> {
        match self.invoke(name, args)? {
            Exec::Finished(value) => Ok(value),
            Exec::Suspended => {
                self.error_source = self.stack.last().map(|frame| frame.source.clone());
                Err(crate::Error::run(
                    self.suspended_span.unwrap_or(Span::EMPTY),
                    "script thread suspended; resume the VM instead of reading a finished value",
                ))
            }
        }
    }

    /// Start a call that may park.
    ///
    /// A second start while this VM is parked or already executing is refused.
    /// Globals and the saved stack are left as they were.
    pub fn invoke(&mut self, name: &str, args: &[Value]) -> Result<Exec, crate::Error> {
        self.refuse_reentry()?;
        if !self.stack.is_empty() {
            return Err(crate::Error::run(
                Span::EMPTY,
                "script thread is suspended; resume it before starting another call",
            ));
        }
        self.init_globals()?;
        self.error_source = None;
        self.suspended_span = None;
        self.steps = 0;
        self.busy = true;
        let result = self.begin(name, args.to_vec(), Span::EMPTY);
        self.busy = false;
        if result.is_err() {
            self.host.abort_script_calls();
        }
        result
    }

    /// Continue a parked thread.
    ///
    /// `value` is the result of the native that parked. [`Host::native`] is not
    /// called again for that invocation. A `call` statement discards `value`.
    /// Any other park uses `value` as the native's result.
    ///
    /// Resuming a thread that is not parked, including after it has finished
    /// or after an error cleared it, is an error.
    pub fn resume(&mut self, value: Value) -> Result<Exec, crate::Error> {
        self.refuse_reentry()?;
        if self.stack.is_empty() {
            return Err(crate::Error::run(
                Span::EMPTY,
                "script thread is not suspended",
            ));
        }
        self.suspended_span = None;
        self.error_source = None;
        self.busy = true;
        self.stack.last_mut().unwrap().incoming = Some(value);
        let result = self.drive_until(0);
        self.busy = false;
        if result.is_err() {
            self.host.abort_script_calls();
        }
        result
    }

    /// Start `functions` in order as one new thread on an idle VM. A park stores
    /// the thread for [`Vm::resume_thread`]; the host keeps its own context.
    /// The step budget restarts here.
    pub fn run_thread(
        &mut self,
        functions: &[crate::value::FunctionId],
    ) -> Result<(), crate::Error> {
        self.enter_thread()?;
        let result = self.run_functions(functions.iter().copied().collect(), Span::EMPTY);
        self.exit_thread(result)
    }

    /// Continue a parked thread by its ticket. `TriggerSleepAction`-style parks
    /// produce no value, so the parked native receives `null`.
    pub fn resume_thread(&mut self, ticket: u64) -> Result<(), crate::Error> {
        self.enter_thread()?;
        let Some(thread) = self.parked.remove(&ticket) else {
            self.busy = false;
            return Err(crate::Error::run(
                Span::EMPTY,
                "no script thread parked under this ticket",
            ));
        };
        if let Err(why) = self.host.resume_thread(ticket) {
            self.busy = false;
            return Err(crate::Error::run(Span::EMPTY, why));
        }
        self.stack = thread.stack;
        self.depth = thread.depth;
        if let Some(top) = self.stack.last_mut() {
            top.incoming = Some(Value::Null);
        }
        let result = self
            .run_functions(thread.remaining, Span::EMPTY)
            .and_then(|()| {
                self.host
                    .leave_thread()
                    .map_err(|why| crate::Error::run(Span::EMPTY, why))
            });
        self.exit_thread(result)
    }

    /// Tickets of parked threads, ascending.
    pub fn parked_threads(&self) -> impl Iterator<Item = u64> + '_ {
        self.parked.keys().copied()
    }

    fn enter_thread(&mut self) -> Result<(), crate::Error> {
        self.refuse_reentry()?;
        if !self.stack.is_empty() {
            return Err(crate::Error::run(
                Span::EMPTY,
                "script thread is suspended; resume it before starting another thread",
            ));
        }
        self.init_globals()?;
        self.error_source = None;
        self.suspended_span = None;
        self.steps = 0;
        self.busy = true;
        Ok(())
    }

    fn exit_thread(&mut self, result: Result<(), crate::Error>) -> Result<(), crate::Error> {
        self.busy = false;
        if result.is_err() {
            self.host.abort_script_calls();
        }
        result
    }

    /// Drive the current stack, then each remaining callback, until the thread
    /// finishes or parks. A park moves the whole stack into `parked`.
    fn run_functions(
        &mut self,
        mut remaining: std::collections::VecDeque<crate::value::FunctionId>,
        span: Span,
    ) -> Result<(), crate::Error> {
        loop {
            if self.stack.is_empty() {
                let Some(id) = remaining.pop_front() else {
                    return Ok(());
                };
                let function = self.program.functions().get(id.index()).ok_or_else(|| {
                    crate::Error::run(span, "thread callback is not in the loaded program")
                })?;
                if !function.params.is_empty() {
                    return Err(crate::Error::run(span, "thread callback must take nothing"));
                }
                let name = function.name.clone();
                self.push_activation(&name, Vec::new(), span)?;
            }
            if let Exec::Suspended = self.drive_until(0)? {
                let at = self.suspended_span.unwrap_or(span);
                let ticket = self
                    .host
                    .park_thread()
                    .map_err(|why| crate::Error::run(at, why))?;
                if self.parked.contains_key(&ticket) {
                    return Err(crate::Error::run(at, "host reused a parked thread ticket"));
                }
                let thread = Thread {
                    stack: std::mem::take(&mut self.stack),
                    depth: std::mem::replace(&mut self.depth, 0),
                    remaining,
                };
                self.parked.insert(ticket, thread);
                return Ok(());
            }
        }
    }

    fn refuse_reentry(&self) -> Result<(), crate::Error> {
        if self.busy {
            Err(crate::Error::run(
                Span::EMPTY,
                "reentrant script invocation",
            ))
        } else {
            Ok(())
        }
    }

    fn initialise_globals(&mut self) -> Result<(), crate::Error> {
        self.error_source = None;
        let result = self.initialise_globals_inner();
        if result.is_err() {
            self.host.abort_script_calls();
            self.stack.clear();
            self.depth = 0;
            self.suspended_span = None;
        }
        result
    }

    fn initialise_globals_inner(&mut self) -> Result<(), crate::Error> {
        let inits = self
            .program
            .globals()
            .iter()
            .map(|g| {
                (
                    g.name.clone(),
                    g.init.clone(),
                    g.array,
                    g.ty.kind.clone(),
                    g.source.clone(),
                    g.span,
                )
            })
            .collect::<Vec<_>>();
        let mut frame = Frame::new();
        for (name, init, array, kind, source, span) in inits {
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
                Some(e) => match self.eval_value(&e, &mut frame) {
                    Ok(value) => match self.coerce_declared(false, &kind, value, e.span()) {
                        Ok(value) => Slot::Scalar(value, kind),
                        Err(error) => {
                            if self.error_source.is_none() {
                                self.error_source = Some(source);
                            }
                            self.init_error = Some(error.clone());
                            return Err(error);
                        }
                    },
                    Err(error) => {
                        if self.error_source.is_none() {
                            self.error_source = Some(source);
                        }
                        self.init_error = Some(error.clone());
                        return Err(error);
                    }
                },
                None if array => Slot::Array(BTreeMap::new(), kind),
                None => Slot::Scalar(Value::Null, kind),
            };
            self.globals.insert(name, slot);
        }
        self.ready = true;
        Ok(())
    }

    fn begin(&mut self, name: &str, argv: Vec<Value>, span: Span) -> Result<Exec, crate::Error> {
        let base = self.stack.len();
        self.push_activation(name, argv, span)?;
        self.drive_until(base)
    }

    fn push_activation(
        &mut self,
        name: &str,
        argv: Vec<Value>,
        span: Span,
    ) -> Result<(), crate::Error> {
        let (params, source, ret, func_span) = {
            let found = self.program.function(name).ok_or_else(|| {
                crate::Error::run(span, format!("`{name}` is not a function in this program"))
            })?;
            (
                found.params.clone(),
                found.source.clone(),
                found.ret.clone(),
                found.span,
            )
        };
        if params.len() != argv.len() {
            let span = if span == Span::EMPTY {
                self.error_source = Some(source);
                func_span
            } else {
                span
            };
            return Err(crate::Error::run(
                span,
                format!(
                    "`{name}` takes {} argument{}, {} given",
                    params.len(),
                    plural(params.len()),
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
        let body = self
            .program
            .function(name)
            .and_then(|f| f.body.as_deref())
            .unwrap_or_default();
        #[cfg(test)]
        {
            self.copied_body_statements += body.len();
        }
        let body = Arc::from(body);
        let mut frame = Frame::new();
        let report = if span == Span::EMPTY { func_span } else { span };
        for (p, v) in params.iter().zip(&argv) {
            let value = match self.coerce_declared(p.ty.array, &p.ty.kind, v.clone(), report) {
                Ok(value) => value,
                Err(error) => {
                    if span == Span::EMPTY {
                        self.error_source = Some(source.clone());
                    }
                    return Err(error);
                }
            };
            frame.insert(
                p.name.as_str().to_string(),
                Slot::Scalar(value, p.ty.kind.clone()),
            );
        }
        self.stack.push(Activation {
            name: name.to_string(),
            source,
            ret,
            span: func_span,
            body,
            frame,
            cont: Cont {
                at: 0,
                nested: None,
                paused: None,
            },
            after_call: None,
            incoming: None,
        });
        self.depth += 1;
        Ok(())
    }

    fn drive_until(&mut self, base: usize) -> Result<Exec, crate::Error> {
        let result = self.drive_loop(base);
        if result.is_err() && base == 0 {
            self.stack.clear();
            self.depth = 0;
            self.suspended_span = None;
        }
        result
    }

    #[cfg_attr(
        feature = "profile-cpu",
        tracing::instrument(skip_all, target = "pcx_cpu", name = "script.vm")
    )]
    fn drive_loop(&mut self, base: usize) -> Result<Exec, crate::Error> {
        loop {
            if self.stack.len() <= base {
                return Ok(Exec::Finished(Value::Null));
            }
            match self.run_top()? {
                TopStop::Suspended => return Ok(Exec::Suspended),
                TopStop::Switched => {}
                TopStop::Finished(value) => {
                    self.stack.pop();
                    self.depth = self.depth.saturating_sub(1);
                    if self.stack.len() == base {
                        return Ok(Exec::Finished(value));
                    }
                    self.deliver_return(value)?;
                }
            }
        }
    }

    fn deliver_return(&mut self, value: Value) -> Result<(), crate::Error> {
        let idx = self.stack.len() - 1;
        let Some(paused) = self.stack[idx].after_call.take() else {
            return Err(broken(
                self.stack[idx].span,
                "a returned call had no expression to fill",
            ));
        };
        self.stack[idx].incoming = Some(value);
        plant_paused(&mut self.stack[idx].cont, *paused)
    }

    fn run_top(&mut self) -> Result<TopStop, crate::Error> {
        let idx = self.stack.len() - 1;
        let cont = std::mem::take(&mut self.stack[idx].cont);
        let incoming = self.stack[idx].incoming.take();
        let body = Arc::clone(&self.stack[idx].body);
        let mut frame = std::mem::take(&mut self.stack[idx].frame);
        let flow = self.exec_cont(&body, &mut frame, cont, incoming);
        self.stack[idx].frame = frame;
        match flow {
            Ok(Flow::Suspend(cont)) => {
                self.stack[idx].cont = *cont;
                Ok(TopStop::Suspended)
            }
            Ok(Flow::Switch { cont, after }) => {
                if self.stack.len() != idx + 2 {
                    return Err(broken(
                        self.stack[idx].span,
                        "a call continuation did not push a frame",
                    ));
                }
                self.stack[idx].cont = *cont;
                self.stack[idx].after_call = Some(after);
                Ok(TopStop::Switched)
            }
            Ok(Flow::Return(value)) => Ok(TopStop::Finished(value)),
            Ok(Flow::Normal) | Ok(Flow::Exit) => {
                let act = &self.stack[idx];
                match implicit_return(&act.ret, &act.name, act.span) {
                    Ok(value) => Ok(TopStop::Finished(value)),
                    Err(error) => self.note_activation_error(idx, error),
                }
            }
            Ok(_) => Err(broken(
                self.stack[idx].span,
                "internal continuation left a statement yield on the function body",
            )),
            Err(error) => self.note_activation_error(idx, error),
        }
    }

    fn note_activation_error<T>(
        &mut self,
        idx: usize,
        error: crate::Error,
    ) -> Result<T, crate::Error> {
        if self.error_source.is_none() {
            self.error_source = Some(self.stack[idx].source.clone());
        }
        Err(error)
    }

    fn exec_cont(
        &mut self,
        stmts: &[Stmt],
        frame: &mut Frame,
        cont: Cont,
        incoming: Option<Value>,
    ) -> Result<Flow, crate::Error> {
        let Cont { at, nested, paused } = cont;
        if let Some(nested) = nested {
            return self.after_nested(stmts, frame, at, *nested, incoming);
        }
        if let Some(paused) = paused {
            let Some(value) = incoming else {
                return Err(broken(
                    stmt_span(stmts, at),
                    "suspended expression resumed without a value",
                ));
            };
            return self.finish_statement(stmts, frame, at, *paused, value);
        }
        let _ = incoming;
        self.run_range(stmts, frame, at)
    }

    fn after_nested(
        &mut self,
        stmts: &[Stmt],
        frame: &mut Frame,
        at: usize,
        nested: Nested,
        incoming: Option<Value>,
    ) -> Result<Flow, crate::Error> {
        match self.resume_nested(stmts, frame, at, nested, incoming)? {
            Flow::Park(nested) => Ok(suspend_at(at, Some(*nested), None)),
            Flow::Invoke { after, nested } => Ok(Flow::Switch {
                cont: Box::new(Cont {
                    at,
                    nested,
                    paused: None,
                }),
                after,
            }),
            Flow::Normal => self.run_range(stmts, frame, at + 1),
            other => Ok(other),
        }
    }

    fn resume_nested(
        &mut self,
        stmts: &[Stmt],
        frame: &mut Frame,
        at: usize,
        nested: Nested,
        incoming: Option<Value>,
    ) -> Result<Flow, crate::Error> {
        let kind = nested.kind;
        let cont = nested.cont;
        match kind {
            Nest::IfBranch(index) => {
                let Stmt::If { branches, .. } = stmt_ref(stmts, at)? else {
                    return Err(broken(
                        stmt_span(stmts, at),
                        "suspended if branch is missing",
                    ));
                };
                let Some(branch) = branches.get(index) else {
                    return Err(broken(
                        stmt_span(stmts, at),
                        "suspended if branch is missing",
                    ));
                };
                self.finish_child(&branch.body, frame, cont, Nest::IfBranch(index), incoming)
            }
            Nest::IfElse => {
                let Stmt::If { else_body, .. } = stmt_ref(stmts, at)? else {
                    return Err(broken(
                        stmt_span(stmts, at),
                        "suspended else body is missing",
                    ));
                };
                let Some(body) = else_body.as_deref() else {
                    return Err(broken(
                        stmt_span(stmts, at),
                        "suspended else body is missing",
                    ));
                };
                self.finish_child(body, frame, cont, Nest::IfElse, incoming)
            }
            Nest::Loop => self.resume_loop(stmts, frame, at, cont, incoming),
            Nest::Debug => self.resume_debug(stmts, frame, at, cont, incoming),
        }
    }

    fn resume_loop(
        &mut self,
        stmts: &[Stmt],
        frame: &mut Frame,
        at: usize,
        cont: Cont,
        incoming: Option<Value>,
    ) -> Result<Flow, crate::Error> {
        let Stmt::Loop { body, span } = stmt_ref(stmts, at)? else {
            return Err(broken(
                stmt_span(stmts, at),
                "suspended loop body is missing",
            ));
        };
        let span = *span;
        match self.exec_cont(body, frame, cont, incoming)? {
            Flow::Suspend(cont) => Ok(Flow::Park(Box::new(Nested {
                kind: Nest::Loop,
                cont: *cont,
            }))),
            Flow::Exit => Ok(Flow::Normal),
            Flow::Normal => self.run_loop(body, span, frame),
            Flow::Return(value) => Ok(Flow::Return(value)),
            Flow::Switch { cont, after } => Ok(Flow::Invoke {
                after,
                nested: Some(Box::new(Nested {
                    kind: Nest::Loop,
                    cont: *cont,
                })),
            }),
            _ => Err(broken(
                span,
                "internal continuation left a statement yield in a loop",
            )),
        }
    }

    fn resume_debug(
        &mut self,
        stmts: &[Stmt],
        frame: &mut Frame,
        at: usize,
        cont: Cont,
        incoming: Option<Value>,
    ) -> Result<Flow, crate::Error> {
        let Stmt::Debug(inner, span) = stmt_ref(stmts, at)? else {
            return Err(broken(
                stmt_span(stmts, at),
                "suspended debug statement is missing",
            ));
        };
        let body = std::slice::from_ref(inner.as_ref());
        let span = *span;
        match self.exec_cont(body, frame, cont, incoming)? {
            Flow::Suspend(cont) => Ok(Flow::Park(Box::new(Nested {
                kind: Nest::Debug,
                cont: *cont,
            }))),
            Flow::Normal => Ok(Flow::Normal),
            Flow::Return(value) => Ok(Flow::Return(value)),
            Flow::Exit => Ok(Flow::Exit),
            Flow::Switch { cont, after } => Ok(Flow::Invoke {
                after,
                nested: Some(Box::new(Nested {
                    kind: Nest::Debug,
                    cont: *cont,
                })),
            }),
            _ => Err(broken(
                span,
                "internal continuation left a statement yield in debug",
            )),
        }
    }

    fn finish_child(
        &mut self,
        body: &[Stmt],
        frame: &mut Frame,
        cont: Cont,
        kind: Nest,
        incoming: Option<Value>,
    ) -> Result<Flow, crate::Error> {
        match self.exec_cont(body, frame, cont, incoming)? {
            Flow::Suspend(cont) => Ok(Flow::Park(Box::new(Nested { kind, cont: *cont }))),
            Flow::Switch { cont, after } => Ok(Flow::Invoke {
                after,
                nested: Some(Box::new(Nested { kind, cont: *cont })),
            }),
            Flow::Normal => Ok(Flow::Normal),
            Flow::Return(value) => Ok(Flow::Return(value)),
            Flow::Exit => Ok(Flow::Exit),
            _ => Err(broken(
                Span::EMPTY,
                "internal continuation left a statement yield in a branch",
            )),
        }
    }

    fn run_range(
        &mut self,
        stmts: &[Stmt],
        frame: &mut Frame,
        start: usize,
    ) -> Result<Flow, crate::Error> {
        for (i, stmt) in stmts.iter().enumerate().skip(start) {
            match self.step(stmt, frame)? {
                Flow::Normal => {}
                Flow::Yield => return Ok(suspend_at(i + 1, None, None)),
                Flow::Pause(paused) => return Ok(suspend_at(i, None, Some(*paused))),
                Flow::Park(nested) => return Ok(suspend_at(i, Some(*nested), None)),
                Flow::Return(value) => return Ok(Flow::Return(value)),
                Flow::Exit => return Ok(Flow::Exit),
                Flow::Invoke { after, nested } => {
                    return Ok(Flow::Switch {
                        cont: Box::new(Cont {
                            at: i,
                            nested,
                            paused: None,
                        }),
                        after,
                    });
                }
                Flow::Suspend(_) | Flow::Switch { .. } => {
                    return Err(broken(
                        stmt.span(),
                        "internal continuation was nested twice",
                    ));
                }
            }
        }
        Ok(Flow::Normal)
    }

    fn finish_statement(
        &mut self,
        stmts: &[Stmt],
        frame: &mut Frame,
        at: usize,
        paused: PausedExpr,
        value: Value,
    ) -> Result<Flow, crate::Error> {
        let stmt = stmt_ref(stmts, at)?;
        match self.finish_paused(stmt, paused, value, frame)? {
            Flow::Normal => self.run_range(stmts, frame, at + 1),
            Flow::Yield => Ok(suspend_at(at + 1, None, None)),
            Flow::Pause(paused) => Ok(suspend_at(at, None, Some(*paused))),
            Flow::Park(nested) => Ok(suspend_at(at, Some(*nested), None)),
            Flow::Return(value) => Ok(Flow::Return(value)),
            Flow::Exit => Ok(Flow::Exit),
            Flow::Invoke { after, nested } => Ok(Flow::Switch {
                cont: Box::new(Cont {
                    at,
                    nested,
                    paused: None,
                }),
                after,
            }),
            Flow::Suspend(_) | Flow::Switch { .. } => Err(broken(
                stmt.span(),
                "internal continuation was nested twice",
            )),
        }
    }

    fn run_loop(
        &mut self,
        body: &[Stmt],
        span: Span,
        frame: &mut Frame,
    ) -> Result<Flow, crate::Error> {
        loop {
            self.tick(span)?;
            match self.run_range(body, frame, 0)? {
                Flow::Normal => {}
                Flow::Exit => return Ok(Flow::Normal),
                Flow::Return(value) => return Ok(Flow::Return(value)),
                Flow::Suspend(cont) => {
                    return Ok(Flow::Park(Box::new(Nested {
                        kind: Nest::Loop,
                        cont: *cont,
                    })));
                }
                Flow::Switch { cont, after } => {
                    return Ok(Flow::Invoke {
                        after,
                        nested: Some(Box::new(Nested {
                            kind: Nest::Loop,
                            cont: *cont,
                        })),
                    });
                }
                _ => {
                    return Err(broken(
                        span,
                        "internal continuation left a statement yield in a loop",
                    ));
                }
            }
        }
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
            Stmt::Local(v, _) | Stmt::Constant(v, _) => self.bind_decl(v, frame),
            Stmt::Set { target, value, .. } => self.eval_set(target, value, frame),
            Stmt::Call { callee, args, span } => {
                let out = self.eval_args(
                    callee.as_str(),
                    *span,
                    Vec::new(),
                    ArgSlice {
                        expressions: args,
                        retained: None,
                    },
                    frame,
                    Vec::new(),
                )?;
                Ok(statement_call(out))
            }
            Stmt::Return(None, _) => Ok(Flow::Return(Value::Null)),
            Stmt::Return(Some(e), _) => self.eval_return(e, frame),
            Stmt::ExitWhen(e, _) => self.eval_exit(e, frame),
            Stmt::If {
                branches,
                else_body,
                ..
            } => self.eval_if(branches, else_body.as_deref(), frame, 0),
            Stmt::Loop { body, span } => self.run_loop(body, *span, frame),
            Stmt::Debug(inner, _) => {
                if !self.run_debug {
                    return Ok(Flow::Normal);
                }
                Self::wrap_debug(self.step(inner, frame)?)
            }
        }
    }

    fn bind_decl(
        &mut self,
        v: &crate::ast::VarDecl,
        frame: &mut Frame,
    ) -> Result<Flow, crate::Error> {
        if v.ty.array && v.init.is_some() {
            return Err(crate::Error::run(
                v.span,
                format!(
                    "`{}` is declared `array`, and an array has no initialiser",
                    v.name.as_str()
                ),
            ));
        }
        let Some(init) = &v.init else {
            let slot = if v.ty.array {
                Slot::Array(BTreeMap::new(), v.ty.kind.clone())
            } else {
                Slot::Scalar(Value::Null, v.ty.kind.clone())
            };
            frame.insert(v.name.as_str().to_string(), slot);
            return Ok(Flow::Normal);
        };
        let name = v.name.as_str().to_string();
        let ty = v.ty.kind.clone();
        match self.eval_expr(init, frame)? {
            ExprOut::Value(value) => {
                let value = self.coerce_declared(false, &ty, value, init.span())?;
                frame.insert(name, Slot::Scalar(value, ty));
                Ok(Flow::Normal)
            }
            other => Ok(suspend_in(ExprDest::Bind { name, ty }, other)),
        }
    }

    fn eval_set(
        &mut self,
        target: &Expr,
        value: &Expr,
        frame: &mut Frame,
    ) -> Result<Flow, crate::Error> {
        match self.eval_expr(value, frame)? {
            ExprOut::Value(rhs) => self.store_set(target, rhs, frame),
            other => Ok(suspend_in(
                ExprDest::Set {
                    target: target.clone(),
                    rhs: None,
                    index_span: None,
                },
                other,
            )),
        }
    }

    fn eval_return(&mut self, e: &Expr, frame: &mut Frame) -> Result<Flow, crate::Error> {
        match self.eval_expr(e, frame)? {
            ExprOut::Value(value) => Ok(Flow::Return(self.coerce_return(value, e.span())?)),
            other => Ok(suspend_in(ExprDest::Return, other)),
        }
    }

    fn eval_exit(&mut self, e: &Expr, frame: &mut Frame) -> Result<Flow, crate::Error> {
        let span = e.span();
        match self.eval_expr(e, frame)? {
            ExprOut::Value(value) => self.exit_value(&value, span),
            other => Ok(suspend_in(ExprDest::ExitWhen { span }, other)),
        }
    }

    fn exit_value(&self, value: &Value, span: Span) -> Result<Flow, crate::Error> {
        Ok(if self.truth(value, span)? {
            Flow::Exit
        } else {
            Flow::Normal
        })
    }

    fn eval_if(
        &mut self,
        branches: &[IfBranch],
        else_body: Option<&[Stmt]>,
        frame: &mut Frame,
        start: usize,
    ) -> Result<Flow, crate::Error> {
        for (index, branch) in branches.iter().enumerate().skip(start) {
            let span = branch.cond.span();
            match self.eval_expr(&branch.cond, frame)? {
                ExprOut::Value(value) => {
                    if self.truth(&value, span)? {
                        return self.open_body(&branch.body, frame, Nest::IfBranch(index));
                    }
                }
                other => {
                    return Ok(suspend_in(
                        ExprDest::IfCond {
                            branch: index,
                            span,
                        },
                        other,
                    ));
                }
            }
        }
        match else_body {
            Some(body) => self.open_body(body, frame, Nest::IfElse),
            None => Ok(Flow::Normal),
        }
    }

    fn open_body(
        &mut self,
        body: &[Stmt],
        frame: &mut Frame,
        kind: Nest,
    ) -> Result<Flow, crate::Error> {
        match self.run_range(body, frame, 0)? {
            Flow::Suspend(cont) => Ok(Flow::Park(Box::new(Nested { kind, cont: *cont }))),
            Flow::Switch { cont, after } => Ok(Flow::Invoke {
                after,
                nested: Some(Box::new(Nested { kind, cont: *cont })),
            }),
            Flow::Normal => Ok(Flow::Normal),
            Flow::Return(value) => Ok(Flow::Return(value)),
            Flow::Exit => Ok(Flow::Exit),
            _ => Err(broken(
                Span::EMPTY,
                "internal continuation left a statement yield in a branch",
            )),
        }
    }

    fn wrap_debug(flow: Flow) -> Result<Flow, crate::Error> {
        match flow {
            Flow::Yield => Ok(Flow::Yield),
            Flow::Pause(paused) => Ok(Flow::Park(Box::new(Nested {
                kind: Nest::Debug,
                cont: Cont {
                    at: 0,
                    nested: None,
                    paused: Some(paused),
                },
            }))),
            Flow::Park(nested) => Ok(Flow::Park(Box::new(Nested {
                kind: Nest::Debug,
                cont: Cont {
                    at: 0,
                    nested: Some(nested),
                    paused: None,
                },
            }))),
            Flow::Invoke { after, nested } => Ok(Flow::Invoke {
                after,
                nested: Some(Box::new(Nested {
                    kind: Nest::Debug,
                    cont: Cont {
                        at: 0,
                        nested,
                        paused: None,
                    },
                })),
            }),
            Flow::Normal => Ok(Flow::Normal),
            Flow::Return(value) => Ok(Flow::Return(value)),
            Flow::Exit => Ok(Flow::Exit),
            Flow::Suspend(_) | Flow::Switch { .. } => Err(broken(
                Span::EMPTY,
                "internal continuation left a driver yield in debug",
            )),
        }
    }

    fn finish_paused(
        &mut self,
        stmt: &Stmt,
        paused: PausedExpr,
        value: Value,
        frame: &mut Frame,
    ) -> Result<Flow, crate::Error> {
        match self.finish_zip(paused.zip, value, frame)? {
            ExprOut::Value(value) => self.apply_dest(stmt, paused.dest, value, frame),
            ExprOut::Park(zip) if zip.is_empty() && matches!(paused.dest, ExprDest::Discard) => {
                Ok(Flow::Yield)
            }
            ExprOut::Park(zip) => Ok(pause(paused.dest, zip)),
            ExprOut::Push(zip) => Ok(invoke_expr(paused.dest, zip, None)),
        }
    }

    fn apply_dest(
        &mut self,
        stmt: &Stmt,
        dest: ExprDest,
        value: Value,
        frame: &mut Frame,
    ) -> Result<Flow, crate::Error> {
        match dest {
            ExprDest::Bind { name, ty } => {
                let value = self.coerce_declared(false, &ty, value, stmt.span())?;
                frame.insert(name, Slot::Scalar(value, ty));
                Ok(Flow::Normal)
            }
            ExprDest::Set {
                target, rhs: None, ..
            } => self.store_set(&target, value, frame),
            ExprDest::Set {
                target,
                rhs: Some(rhs),
                index_span,
            } => {
                let span = index_span.unwrap_or_else(|| target.span());
                let idx = self.index_of(&value, span)?;
                self.write_target(&target, Some(idx), rhs, frame)
            }
            ExprDest::Discard => Ok(Flow::Normal),
            ExprDest::Return => Ok(Flow::Return(self.coerce_return(value, stmt.span())?)),
            ExprDest::ExitWhen { span } => self.exit_value(&value, span),
            ExprDest::IfCond { branch, span } => self.apply_if(stmt, branch, span, &value, frame),
        }
    }

    fn apply_if(
        &mut self,
        stmt: &Stmt,
        branch: usize,
        span: Span,
        value: &Value,
        frame: &mut Frame,
    ) -> Result<Flow, crate::Error> {
        let Stmt::If {
            branches,
            else_body,
            ..
        } = stmt
        else {
            return Err(broken(stmt.span(), "suspended if condition is missing"));
        };
        if self.truth(value, span)? {
            let Some(body) = branches.get(branch).map(|b| b.body.as_slice()) else {
                return Err(broken(stmt.span(), "suspended if branch is missing"));
            };
            return self.open_body(body, frame, Nest::IfBranch(branch));
        }
        self.eval_if(branches, else_body.as_deref(), frame, branch + 1)
    }

    fn store_set(
        &mut self,
        target: &Expr,
        rhs: Value,
        frame: &mut Frame,
    ) -> Result<Flow, crate::Error> {
        match target {
            Expr::Ident(_, _) => self.write_target(target, None, rhs, frame),
            Expr::Index { base, index, .. } => {
                let Expr::Ident(_, _) = base.as_ref() else {
                    return Err(crate::Error::run(
                        target.span(),
                        "only a variable name or an array slot can be assigned to",
                    ));
                };
                let index_span = index.span();
                match self.eval_expr(index, frame)? {
                    ExprOut::Value(value) => {
                        let idx = self.index_of(&value, index_span)?;
                        self.write_target(target, Some(idx), rhs, frame)
                    }
                    other => Ok(suspend_in(
                        ExprDest::Set {
                            target: target.clone(),
                            rhs: Some(rhs),
                            index_span: Some(index_span),
                        },
                        other,
                    )),
                }
            }
            _ => Err(crate::Error::run(
                target.span(),
                "only a variable name or an array slot can be assigned to",
            )),
        }
    }

    fn write_target(
        &mut self,
        target: &Expr,
        idx: Option<i32>,
        value: Value,
        frame: &mut Frame,
    ) -> Result<Flow, crate::Error> {
        let name = match target {
            Expr::Ident(name, _) => name.as_str(),
            Expr::Index { base, .. } => {
                let Expr::Ident(name, _) = base.as_ref() else {
                    return Err(crate::Error::run(
                        target.span(),
                        "only a variable name or an array slot can be assigned to",
                    ));
                };
                name.as_str()
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
        let real = if let Some(slot) = frame.get(name) {
            slot_is_real(slot)
        } else {
            self.globals.get(name).is_some_and(slot_is_real)
        };
        let value = if real {
            self.coerce_declared(false, &TypeKind::Real, value, target.span())?
        } else {
            value
        };
        if let Some(slot) = frame.get_mut(name) {
            return store(slot, idx, value, target.span());
        }
        match self.globals.get_mut(name) {
            Some(slot) => store(slot, idx, value, target.span()),
            None => Err(crate::Error::run(
                target.span(),
                format!("`{name}` is not a variable in this program"),
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

    /// Evaluate an initialiser that must finish. A park or a parked callee is
    /// an error and leaves no thread.
    fn eval_value(&mut self, e: &Expr, frame: &mut Frame) -> Result<Value, crate::Error> {
        let mut out = self.eval_expr(e, frame)?;
        loop {
            match out {
                ExprOut::Value(value) => return Ok(value),
                ExprOut::Park(_) => {
                    return Err(crate::Error::run(
                        e.span(),
                        "a native suspended during global initialisation",
                    ));
                }
                ExprOut::Push(zip) => {
                    let base = self.stack.len() - 1;
                    match self.drive_until(base)? {
                        Exec::Finished(value) => out = self.finish_zip(zip, value, frame)?,
                        Exec::Suspended => {
                            self.stack.clear();
                            self.depth = 0;
                            return Err(crate::Error::run(
                                e.span(),
                                "a native suspended during global initialisation",
                            ));
                        }
                    }
                }
            }
        }
    }

    fn eval_expr(&mut self, e: &Expr, frame: &mut Frame) -> Result<ExprOut, crate::Error> {
        self.tick(e.span())?;
        match e {
            Expr::Literal(l, span) => self.literal(l, *span).map(ExprOut::Value),
            Expr::Ident(name, span) => self.eval_ident(name, *span, frame).map(ExprOut::Value),
            Expr::Group(inner, _) => self.eval_expr(inner, frame),
            Expr::Unary { op, operand, span } => self.eval_unary(*op, operand, *span, frame),
            Expr::Binary { op, lhs, rhs, span } => self.eval_binary(*op, lhs, rhs, *span, frame),
            Expr::Call { callee, args, span } => self.eval_call_expr(callee, args, *span, frame),
            Expr::Index { base, index, span } => self.eval_index(base, index, *span, frame),
            Expr::FunctionRef { name, span } => self.function_ref(name, *span),
        }
    }

    fn eval_unary(
        &mut self,
        op: UnOp,
        operand: &Expr,
        span: Span,
        frame: &mut Frame,
    ) -> Result<ExprOut, crate::Error> {
        match self.eval_expr(operand, frame)? {
            ExprOut::Value(value) => unary(op, &value)
                .map(ExprOut::Value)
                .map_err(|e| self.op_error(&e, span)),
            ExprOut::Park(inner) => Ok(ExprOut::Park(join_zip(
                Vec::new(),
                Zip::Unary { op, span },
                inner,
            ))),
            ExprOut::Push(inner) => Ok(ExprOut::Push(join_zip(
                Vec::new(),
                Zip::Unary { op, span },
                inner,
            ))),
        }
    }

    fn eval_binary(
        &mut self,
        op: BinOp,
        lhs: &Expr,
        rhs: &Expr,
        span: Span,
        frame: &mut Frame,
    ) -> Result<ExprOut, crate::Error> {
        let left_span = lhs.span();
        match self.eval_expr(lhs, frame)? {
            ExprOut::Value(left) => self.continue_right(
                Rhs {
                    op,
                    span,
                    left_span,
                    left,
                    rhs,
                },
                frame,
                Vec::new(),
            ),
            ExprOut::Park(inner) => Ok(ExprOut::Park(join_zip(
                Vec::new(),
                Zip::Left {
                    op,
                    span,
                    left_span,
                    rhs: rhs.clone(),
                },
                inner,
            ))),
            ExprOut::Push(inner) => Ok(ExprOut::Push(join_zip(
                Vec::new(),
                Zip::Left {
                    op,
                    span,
                    left_span,
                    rhs: rhs.clone(),
                },
                inner,
            ))),
        }
    }

    fn continue_right(
        &mut self,
        wait: Rhs<'_>,
        frame: &mut Frame,
        outer: Vec<Zip>,
    ) -> Result<ExprOut, crate::Error> {
        if wait.op == BinOp::And && !self.truth(&wait.left, wait.left_span)? {
            return self.finish_zip(outer, Value::Bool(false), frame);
        }
        if wait.op == BinOp::Or && self.truth(&wait.left, wait.left_span)? {
            return self.finish_zip(outer, Value::Bool(true), frame);
        }
        let rhs_span = wait.rhs.span();
        match self.eval_expr(wait.rhs, frame)? {
            ExprOut::Value(right) => {
                let value = self.combine(wait.op, wait.span, rhs_span, wait.left, right)?;
                self.finish_zip(outer, value, frame)
            }
            ExprOut::Park(inner) => Ok(ExprOut::Park(join_zip(
                outer,
                Zip::Right {
                    op: wait.op,
                    span: wait.span,
                    left: wait.left,
                    rhs_span,
                },
                inner,
            ))),
            ExprOut::Push(inner) => Ok(ExprOut::Push(join_zip(
                outer,
                Zip::Right {
                    op: wait.op,
                    span: wait.span,
                    left: wait.left,
                    rhs_span,
                },
                inner,
            ))),
        }
    }

    fn eval_call_expr(
        &mut self,
        callee: &Expr,
        args: &[Expr],
        span: Span,
        frame: &mut Frame,
    ) -> Result<ExprOut, crate::Error> {
        let Expr::Ident(name, _) = callee else {
            return Err(crate::Error::run(
                span,
                "a call needs a name: only a function or a native can be called",
            ));
        };
        if matches!(frame.get(name.as_str()), Some(Slot::Scalar(_, _))) {
            return Err(crate::Error::run(
                span,
                format!(
                    "`{}` is a variable, and a variable cannot be called",
                    name.as_str()
                ),
            ));
        }
        self.eval_args(
            name.as_str(),
            span,
            Vec::new(),
            ArgSlice {
                expressions: args,
                retained: None,
            },
            frame,
            Vec::new(),
        )
    }

    fn eval_args(
        &mut self,
        callee: &str,
        span: Span,
        mut got: Vec<Value>,
        rest: ArgSlice<'_>,
        frame: &mut Frame,
        outer: Vec<Zip>,
    ) -> Result<ExprOut, crate::Error> {
        if matches!(frame.get(callee), Some(Slot::Scalar(_, _))) {
            return Err(crate::Error::run(
                span,
                format!("`{callee}` is a variable, and a variable cannot be called"),
            ));
        }
        for (index, arg) in rest.expressions.iter().enumerate() {
            match self.eval_expr(arg, frame)? {
                ExprOut::Value(value) => got.push(value),
                ExprOut::Park(inner) => {
                    let (remaining, next) = self.retain_args(&rest, index + 1);
                    return Ok(ExprOut::Park(join_zip(
                        outer,
                        Zip::Arg {
                            callee: callee.to_string(),
                            span,
                            got,
                            rest: remaining,
                            next,
                        },
                        inner,
                    )));
                }
                ExprOut::Push(inner) => {
                    let (remaining, next) = self.retain_args(&rest, index + 1);
                    return Ok(ExprOut::Push(join_zip(
                        outer,
                        Zip::Arg {
                            callee: callee.to_string(),
                            span,
                            got,
                            rest: remaining,
                            next,
                        },
                        inner,
                    )));
                }
            }
        }
        self.dispatch(callee, span, got, frame, outer)
    }

    fn retain_args(&mut self, args: &ArgSlice<'_>, next: usize) -> (Arc<[Expr]>, usize) {
        if let Some((retained, start)) = args.retained {
            (Arc::clone(retained), start + next)
        } else {
            let tail = &args.expressions[next..];
            #[cfg(test)]
            {
                self.copied_argument_expressions += tail.len();
            }
            (Arc::from(tail), 0)
        }
    }

    fn dispatch(
        &mut self,
        callee: &str,
        span: Span,
        argv: Vec<Value>,
        frame: &mut Frame,
        outer: Vec<Zip>,
    ) -> Result<ExprOut, crate::Error> {
        if self.program.natives().ambiguous(callee).is_some() {
            return Err(crate::Error::run(
                span,
                format!(
                    "`{callee}` is declared with two different signatures, so which one this call means cannot be told"
                ),
            ));
        }
        match self.program.natives().call(callee) {
            Some(NativeCall::Implemented(native, which)) => {
                let arity = native.params.len();
                let params = native.params.clone();
                let ret = native.ret.clone();
                self.check_native_arity(callee, arity, argv.len(), span)?;
                let argv = self.coerce_params(&params, argv, span)?;
                let value = self.run_native(which, &argv, span)?;
                let value = self.coerce_type(&ret, value, span)?;
                self.finish_zip(outer, value, frame)
            }
            Some(NativeCall::SignatureMismatch(native, which)) => {
                let signature =
                    crate::natives::signature_of(&native.name, &native.params, &native.ret);
                Err(crate::Error::run(
                    span,
                    format!(
                        "native `{}` cannot dispatch {which:?}: supplied declaration `{signature}` at {}:{}..{} is incompatible with the implemented conversion",
                        native.name, native.source, native.span.start, native.span.end
                    ),
                ))
            }
            Some(NativeCall::Unimplemented(native)) => {
                let arity = native.params.len();
                let params = native.params.clone();
                let ret = native.ret.clone();
                let declared = native.name.clone();
                let signature =
                    crate::natives::signature_of(&native.name, &native.params, &native.ret);
                let source = native.source.clone();
                let declared_at = native.span;
                self.check_native_arity(callee, arity, argv.len(), span)?;
                let argv = self.coerce_params(&params, argv, span)?;
                if self.host.suspend(callee, &argv) {
                    self.suspended_span = Some(span);
                    return Ok(ExprOut::Park(park_real_return(outer, &ret, span)));
                }
                if let Some(call) = self
                    .host
                    .begin_script_call(callee, &argv)
                    .map_err(|why| crate::Error::run(span, why))?
                {
                    return self.dispatch_script_call(
                        call,
                        span,
                        frame,
                        park_real_return(outer, &ret, span),
                    );
                }
                match self.host.native(callee, &argv) {
                    Ok(value) => {
                        let value = self.coerce_type(&ret, value, span)?;
                        self.finish_zip(outer, value, frame)
                    }
                    Err(why) => Err(crate::Error::run(
                        span,
                        format!(
                            "native `{declared}` is not implemented; it is declared as `{signature}` at {source}:{}..{}: {why}",
                            declared_at.start, declared_at.end
                        ),
                    )),
                }
            }
            None if self.program.function(callee).is_some() => {
                self.push_activation(callee, argv, span)?;
                Ok(ExprOut::Push(outer))
            }
            None => Err(crate::Error::run(
                span,
                format!(
                    "`{callee}` is not declared as a function or native in the loaded script sources"
                ),
            )),
        }
    }

    fn dispatch_script_call(
        &mut self,
        call: ScriptCall,
        span: Span,
        frame: &mut Frame,
        mut outer: Vec<Zip>,
    ) -> Result<ExprOut, crate::Error> {
        self.tick(span)?;
        match call {
            ScriptCall::Return(value) => self.finish_zip(outer, value, frame),
            ScriptCall::Call(id) => {
                let function = self.program.functions().get(id.index()).ok_or_else(|| {
                    crate::Error::run(span, "native callback is not in the loaded program")
                })?;
                if !function.params.is_empty() {
                    return Err(crate::Error::run(span, "native callback must take nothing"));
                }
                let name = function.name.clone();
                self.push_activation(&name, Vec::new(), span)?;
                outer.push(Zip::ScriptCall { span });
                Ok(ExprOut::Push(outer))
            }
            ScriptCall::Spawn(functions) => {
                if self.spawn_depth >= MAX_SPAWN_DEPTH {
                    return Err(crate::Error::run(
                        span,
                        format!("threads nested past {MAX_SPAWN_DEPTH} spawns"),
                    ));
                }
                let caller = std::mem::take(&mut self.stack);
                let depth = std::mem::replace(&mut self.depth, 0);
                self.spawn_depth += 1;
                let result = self.run_functions(functions.into(), span);
                self.spawn_depth -= 1;
                self.stack = caller;
                self.depth = depth;
                result?;
                self.host
                    .leave_thread()
                    .map_err(|why| crate::Error::run(span, why))?;
                self.finish_zip(outer, Value::Null, frame)
            }
        }
    }

    fn check_native_arity(
        &self,
        name: &str,
        arity: usize,
        given: usize,
        span: Span,
    ) -> Result<(), crate::Error> {
        if arity == given {
            Ok(())
        } else {
            Err(crate::Error::run(
                span,
                format!(
                    "native `{name}` takes {} argument{}, {given} given",
                    arity,
                    plural(arity)
                ),
            ))
        }
    }

    fn eval_index(
        &mut self,
        base: &Expr,
        index: &Expr,
        span: Span,
        frame: &mut Frame,
    ) -> Result<ExprOut, crate::Error> {
        let Expr::Ident(name, _) = base else {
            return Err(crate::Error::run(span, "an array slot needs an array name"));
        };
        let name = name.as_str().to_string();
        let index_span = index.span();
        match self.eval_expr(index, frame)? {
            ExprOut::Value(value) => self
                .read_index(&name, &value, span, index_span, frame)
                .map(ExprOut::Value),
            ExprOut::Park(inner) => Ok(ExprOut::Park(join_zip(
                Vec::new(),
                Zip::Index {
                    name,
                    span,
                    index_span,
                },
                inner,
            ))),
            ExprOut::Push(inner) => Ok(ExprOut::Push(join_zip(
                Vec::new(),
                Zip::Index {
                    name,
                    span,
                    index_span,
                },
                inner,
            ))),
        }
    }

    fn read_index(
        &self,
        name: &str,
        index: &Value,
        span: Span,
        index_span: Span,
        frame: &Frame,
    ) -> Result<Value, crate::Error> {
        let idx = self.index_of(index, index_span)?;
        let slot = frame.get(name).or_else(|| self.globals.get(name));
        match slot {
            Some(Slot::Array(a, _)) => Ok(a.get(&idx).cloned().unwrap_or(Value::Null)),
            Some(Slot::Scalar(_, _)) => {
                Err(crate::Error::run(span, format!("`{name}` is not an array")))
            }
            None => Err(crate::Error::run(
                span,
                format!("`{name}` is not a variable in this program"),
            )),
        }
    }

    fn function_ref(&self, name: &crate::ast::Ident, span: Span) -> Result<ExprOut, crate::Error> {
        self.program
            .function_id(name.as_str())
            .map(Value::Code)
            .map(ExprOut::Value)
            .ok_or_else(|| {
                crate::Error::run(
                    span,
                    format!(
                        "`function {}` needs a function, and `{}` is not one",
                        name.as_str(),
                        name.as_str()
                    ),
                )
            })
    }

    fn finish_zip(
        &mut self,
        mut zip: Vec<Zip>,
        mut value: Value,
        frame: &mut Frame,
    ) -> Result<ExprOut, crate::Error> {
        while let Some(top) = zip.pop() {
            match top {
                Zip::ScriptCall { span } => {
                    let call = self
                        .host
                        .continue_script_call(value)
                        .map_err(|why| crate::Error::run(span, why))?;
                    return self.dispatch_script_call(call, span, frame, zip);
                }
                Zip::Unary { op, span } => {
                    value = unary(op, &value).map_err(|e| self.op_error(&e, span))?;
                }
                Zip::Left {
                    op,
                    span,
                    left_span,
                    rhs,
                } => {
                    return self.continue_right(
                        Rhs {
                            op,
                            span,
                            left_span,
                            left: value,
                            rhs: &rhs,
                        },
                        frame,
                        zip,
                    );
                }
                Zip::Right {
                    op,
                    span,
                    left,
                    rhs_span,
                } => {
                    value = self.combine(op, span, rhs_span, left, value)?;
                }
                Zip::Index {
                    name,
                    span,
                    index_span,
                } => {
                    value = self.read_index(&name, &value, span, index_span, frame)?;
                }
                Zip::DeclaredReal { span } => {
                    value = self.coerce_declared(false, &TypeKind::Real, value, span)?;
                }
                Zip::Arg {
                    callee,
                    span,
                    mut got,
                    rest,
                    next,
                } => {
                    got.push(value);
                    return self.eval_args(
                        &callee,
                        span,
                        got,
                        ArgSlice {
                            expressions: &rest[next..],
                            retained: Some((&rest, next)),
                        },
                        frame,
                        zip,
                    );
                }
            }
        }
        Ok(ExprOut::Value(value))
    }

    fn combine(
        &self,
        op: BinOp,
        span: Span,
        rhs_span: Span,
        left: Value,
        right: Value,
    ) -> Result<Value, crate::Error> {
        if op == BinOp::And || op == BinOp::Or {
            return self.truth(&right, rhs_span).map(Value::Bool);
        }
        if let Some(real) = self.host.real_ops()
            && matches!(
                (&left, &right),
                (Value::Real(_), Value::Real(_) | Value::Int(_)) | (Value::Int(_), Value::Real(_))
            )
        {
            let number = |value: &Value| match value {
                Value::Real(value) => Ok(*value),
                Value::Int(value) => real.integer(*value),
                _ => unreachable!(),
            };
            return number(&left)
                .and_then(|left| number(&right).and_then(|right| real.binary(op, left, right)))
                .map_err(|why| crate::Error::run(span, why));
        }
        binary(op, &left, &right).map_err(|e| self.op_error(&e, span))
    }

    fn eval_ident(
        &mut self,
        name: &crate::ast::Ident,
        span: Span,
        frame: &Frame,
    ) -> Result<Value, crate::Error> {
        if let Some(slot) = frame
            .get(name.as_str())
            .or_else(|| self.globals.get(name.as_str()))
        {
            return match slot {
                Slot::Scalar(v, _) => Ok(v.clone()),
                Slot::Array(_, _) => Err(crate::Error::run(
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

    /// `I2R` and a declared `real` share this conversion.
    fn real_from_integer(&self, value: i32, span: Span) -> Result<f32, crate::Error> {
        match self.host.real_ops() {
            Some(real) => real
                .integer(value)
                .map_err(|why| crate::Error::run(span, why)),
            None => Ok(value as f32),
        }
    }

    fn coerce_declared(
        &self,
        array: bool,
        ty: &TypeKind,
        value: Value,
        span: Span,
    ) -> Result<Value, crate::Error> {
        if array || *ty != TypeKind::Real {
            return Ok(value);
        }
        let Value::Int(integer) = value else {
            return Ok(value);
        };
        self.real_from_integer(integer, span).map(Value::Real)
    }

    fn coerce_type(&self, ty: &TypeRef, value: Value, span: Span) -> Result<Value, crate::Error> {
        self.coerce_declared(ty.array, &ty.kind, value, span)
    }

    fn coerce_params(
        &self,
        params: &[Param],
        args: Vec<Value>,
        span: Span,
    ) -> Result<Vec<Value>, crate::Error> {
        let mut out = Vec::with_capacity(args.len());
        for (param, value) in params.iter().zip(args) {
            out.push(self.coerce_declared(param.ty.array, &param.ty.kind, value, span)?);
        }
        Ok(out)
    }

    fn coerce_return(&self, value: Value, span: Span) -> Result<Value, crate::Error> {
        let Some((array, kind)) = self
            .stack
            .last()
            .map(|act| (act.ret.array, act.ret.kind.clone()))
        else {
            return Ok(value);
        };
        self.coerce_declared(array, &kind, value, span)
    }

    fn run_native(
        &self,
        which: NativeImpl,
        args: &[Value],
        span: Span,
    ) -> Result<Value, crate::Error> {
        match which {
            NativeImpl::IntToReal => match args[0] {
                Value::Int(i) => self.real_from_integer(i, span).map(Value::Real),
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
            Literal::Real(r) => match self.host.real_ops() {
                Some(real) => real
                    .literal(&r.text)
                    .map(Value::Real)
                    .map_err(|why| crate::Error::run(span, why)),
                None => Ok(Value::Real(r.value as f32)),
            },
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

struct Rhs<'a> {
    op: BinOp,
    span: Span,
    left_span: Span,
    left: Value,
    rhs: &'a Expr,
}

/// `outer` is outside `mid`, and `inner` is nearest the hole.
fn join_zip(mut outer: Vec<Zip>, mid: Zip, inner: Vec<Zip>) -> Vec<Zip> {
    outer.push(mid);
    outer.extend(inner);
    outer
}

fn pause(dest: ExprDest, zip: Vec<Zip>) -> Flow {
    Flow::Pause(Box::new(PausedExpr { dest, zip }))
}

fn invoke_expr(dest: ExprDest, zip: Vec<Zip>, nested: Option<Nested>) -> Flow {
    Flow::Invoke {
        after: Box::new(PausedExpr { dest, zip }),
        nested: nested.map(Box::new),
    }
}

fn suspend_in(dest: ExprDest, out: ExprOut) -> Flow {
    match out {
        ExprOut::Value(_) => Flow::Normal,
        ExprOut::Park(zip) => pause(dest, zip),
        ExprOut::Push(zip) => invoke_expr(dest, zip, None),
    }
}

fn statement_call(out: ExprOut) -> Flow {
    match out {
        ExprOut::Value(_) => Flow::Normal,
        ExprOut::Park(zip) if zip.is_empty() => Flow::Yield,
        ExprOut::Park(zip) => pause(ExprDest::Discard, zip),
        ExprOut::Push(zip) => invoke_expr(ExprDest::Discard, zip, None),
    }
}

fn suspend_at(at: usize, nested: Option<Nested>, paused: Option<PausedExpr>) -> Flow {
    Flow::Suspend(Box::new(Cont {
        at,
        nested: nested.map(Box::new),
        paused: paused.map(Box::new),
    }))
}

fn plant_paused(cont: &mut Cont, paused: PausedExpr) -> Result<(), crate::Error> {
    let mut leaf = cont;
    while let Some(nested) = leaf.nested.as_mut() {
        leaf = &mut nested.cont;
    }
    if leaf.paused.is_some() {
        return Err(broken(
            Span::EMPTY,
            "a call returned into an expression that was already paused",
        ));
    }
    leaf.paused = Some(Box::new(paused));
    Ok(())
}

fn stmt_ref(stmts: &[Stmt], at: usize) -> Result<&Stmt, crate::Error> {
    stmts
        .get(at)
        .ok_or_else(|| broken(Span::EMPTY, "suspended statement is outside its body"))
}

fn stmt_span(stmts: &[Stmt], at: usize) -> Span {
    stmts.get(at).map(Stmt::span).unwrap_or(Span::EMPTY)
}

fn broken(span: Span, what: &str) -> crate::Error {
    crate::Error::run(span, format!("internal continuation error: {what}"))
}

/// A function that reaches `endfunction` without a `return`.
fn implicit_return(ret: &TypeRef, name: &str, span: Span) -> Result<Value, crate::Error> {
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
        (Slot::Array(a, _), Some(i)) => {
            a.insert(i, value);
            Ok(Flow::Normal)
        }
        (Slot::Array(_, _), None) => Err(crate::Error::run(
            span,
            "an array needs an index; `set arrayName = …` is not a JASS assignment",
        )),
        (Slot::Scalar(s, _), None) => {
            *s = value;
            Ok(Flow::Normal)
        }
        (Slot::Scalar(_, _), Some(_)) => {
            Err(crate::Error::run(span, "this variable is not an array"))
        }
    }
}

fn slot_is_real(slot: &Slot) -> bool {
    matches!(
        slot,
        Slot::Scalar(_, TypeKind::Real) | Slot::Array(_, TypeKind::Real)
    )
}

/// A parked native declared `returns real` converts the resumed value before
/// the surrounding expression sees it.
fn park_real_return(mut outer: Vec<Zip>, ret: &TypeRef, span: Span) -> Vec<Zip> {
    if !ret.array && ret.kind == TypeKind::Real {
        outer.push(Zip::DeclaredReal { span });
    }
    outer
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

#[cfg(test)]
mod callback_work_tests {
    use super::*;

    #[test]
    fn native_callback_chains_spend_work_even_when_the_function_body_is_empty() {
        #[derive(Default)]
        struct Chain {
            function: Option<crate::value::FunctionId>,
            calls: usize,
        }
        impl Host for Chain {
            fn native(&mut self, _: &str, _: &[Value]) -> Result<Value, String> {
                Err("unexpected native".into())
            }
            fn begin_script_call(
                &mut self,
                _: &str,
                args: &[Value],
            ) -> Result<Option<ScriptCall>, String> {
                let [Value::Code(function)] = args else {
                    return Err("callback required".into());
                };
                self.function = Some(*function);
                Ok(Some(ScriptCall::Call(*function)))
            }
            fn continue_script_call(&mut self, _: Value) -> Result<ScriptCall, String> {
                self.calls += 1;
                Ok(if self.calls < MAX_STEPS as usize {
                    ScriptCall::Call(self.function.unwrap())
                } else {
                    ScriptCall::Return(Value::Null)
                })
            }
        }
        let script = crate::parse_script("native Drive takes code callback returns nothing\nfunction empty takes nothing returns nothing\nendfunction\nfunction main takes nothing returns nothing\ncall Drive(function empty)\nendfunction\n").unwrap();
        let program = crate::resolve(&script).unwrap();
        let mut vm = Vm::new(&program, Chain::default());
        vm.set_steps_for_test(MAX_STEPS);
        assert!(
            vm.call("main", &[])
                .unwrap_err()
                .to_string()
                .contains("steps")
        );
        // The budget restarts per entry and still bounds an empty-bodied chain.
        assert!(vm.host().calls < MAX_STEPS as usize);
    }
}

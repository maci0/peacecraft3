//! Timers on the existing arena, driven by caller game time. Timeouts are
//! positive, or zero for a one-shot that expires at the next boundary.
use super::{Entry, Error, Failure, ObjectHost};
use pcx_script::value::FunctionId;
use pcx_script::{HandleId, Value};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Run {
    timeout: f64,
    start: f64,
    deadline: Option<f64>,
    periodic: bool,
    handler: Option<FunctionId>,
    sequence: u64,
    /// Elapsed seconds held by `PauseTimer` until `ResumeTimer`.
    paused: Option<f64>,
    query: Option<QueryRun>,
}

#[derive(Clone, Copy, Debug)]
struct QueryRun {
    epoch: crate::script_numeric::timer_query::Epoch,
    cached_remaining: f32,
    resume_dispatch: bool,
}

pub(super) struct State {
    now: f64,
    sequence: u64,
    active: Option<Expired>,
}

#[derive(Clone, Copy)]
struct Expired {
    source: HandleId,
    timer: Option<HandleId>,
    sequence: u64,
    timeout: f64,
    query_now: Option<f32>,
    query_epoch: Option<crate::script_numeric::timer_query::Epoch>,
    resume_dispatch: bool,
}

/// One prepared expiry. Finish it even when its callback fails.
///
/// `triggers` holds every live subscription snapshot in arena (registration)
/// order. The session delivers each trigger's conditions and actions before
/// the direct `callback`: expiry-trigger actions observe the expired timer
/// first, and the direct handler runs with cleared trigger/event context.
/// Snapshots are prepared before the deadline is consumed, so callback-list
/// edits apply on the next firing.
#[derive(Debug)]
pub struct TimerExpiry {
    source: HandleId,
    sequence: u64,
    pub callback: Option<FunctionId>,
    triggers: Vec<super::TriggerDelivery>,
}

impl TimerExpiry {
    /// Trigger snapshots in delivery order: arena (registration) order.
    pub fn triggers(&self) -> &[super::TriggerDelivery] {
        &self.triggers
    }
}

impl ObjectHost {
    pub(super) fn timer_delivery_active(&self) -> bool {
        self.timers
            .as_ref()
            .is_some_and(|state| state.active.is_some())
    }
    /// Opt in before script initialization. A numeric policy with timer-query
    /// rules enables the reviewed readpoints; without them this is the explicit
    /// partial reconstruction mode. No application clock or speed is inferred.
    pub fn with_timer_runtime(mut self, now: f64) -> Result<Self, Error> {
        let fail = |reason| self.inner.error("TimerStart", reason);
        if self.timers.is_some() || !now.is_finite() {
            return Err(fail(Failure::TimerState(
                "invalid or already admitted game time",
            )));
        }
        for (name, params, result) in [
            (
                "TimerStart",
                &["timer", "real", "boolean", "code"][..],
                "nothing",
            ),
            ("TimerGetElapsed", &["timer"][..], "real"),
            ("TimerGetRemaining", &["timer"][..], "real"),
            ("TimerGetTimeout", &["timer"][..], "real"),
            ("GetExpiredTimer", &[][..], "timer"),
        ] {
            Self::contract(&self.inner, name, params, result, false, Some("timer"))?;
            if self
                .inner
                .program
                .natives()
                .get(name)
                .expect("checked timer contract")
                .source
                != self
                    .inner
                    .program
                    .natives()
                    .get("CreateTimer")
                    .expect("checked timer contract")
                    .source
            {
                return Err(fail(Failure::Source));
            }
        }
        let source = &self
            .inner
            .program
            .natives()
            .get("TimerStart")
            .expect("checked timer contract")
            .source;
        if self
            .inner
            .program
            .natives()
            .get("DestroyTimer")
            .expect("checked timer contract")
            .source
            != *source
        {
            return Err(fail(Failure::Source));
        }
        let mut kind = "timer";
        for _ in 0..=self.inner.program.types().count() {
            if kind == "handle" {
                break;
            }
            let ty = self
                .inner
                .program
                .types()
                .find(|ty| ty.name == kind)
                .ok_or_else(|| fail(Failure::Signature))?;
            if &ty.source != source {
                return Err(fail(Failure::Source));
            }
            kind = &ty.parent;
        }
        if kind != "handle" {
            return Err(fail(Failure::Signature));
        }
        for optional in ["PauseTimer", "ResumeTimer"] {
            if self.inner.program.natives().get(optional).is_some() {
                Self::contract(
                    &self.inner,
                    optional,
                    &["timer"],
                    "nothing",
                    false,
                    Some("timer"),
                )?;
            }
        }
        self.timers = Some(State {
            now,
            sequence: 0,
            active: None,
        });
        Ok(self)
    }

    pub fn timer_game_time(&self) -> Option<f64> {
        self.timers.as_ref().map(|state| state.now)
    }

    /// Mandatory world work can split a timer advance, but cannot finish it early.
    pub(crate) fn next_timer_activity_boundary(&self, until: f64) -> Option<f64> {
        [
            self.next_training_completion(until),
            self.next_construction_completion(until),
            self.next_mirror_boundary(until),
            self.next_morph_boundary(until),
        ]
        .into_iter()
        .flatten()
        .min_by(f64::total_cmp)
    }

    /// Earliest pending timer or timed-event deadline.
    pub(super) fn next_timer_deadline(&self) -> Option<f64> {
        self.inner
            .arena
            .iter()
            .filter_map(|entry| match entry {
                Entry::Timer {
                    live: true,
                    run: Some(run),
                } => run.deadline,
                Entry::RegisteredTimedEvent { trigger, run }
                    if matches!(self.inner.entry(*trigger), Ok(Entry::Trigger { .. })) =>
                {
                    run.deadline
                }
                _ => None,
            })
            .min_by(f64::total_cmp)
    }

    /// Move the clock forward to `at` between deliveries; never backwards.
    pub(crate) fn set_timer_game_time(&mut self, at: f64) -> Result<(), Error> {
        let fail = |reason| {
            self.inner
                .error("timer delivery", Failure::TimerState(reason))
        };
        let state = self.timers.as_mut().ok_or_else(|| fail("not admitted"))?;
        if state.active.is_some() || !at.is_finite() {
            return Err(fail("invalid time or unfinished expiry"));
        }
        state.now = state.now.max(at);
        Ok(())
    }

    fn timer_run(&self, id: HandleId, name: &str) -> Result<Option<Run>, Error> {
        match self
            .inner
            .entry(id)
            .map_err(|e| self.inner.error(name, e.failure))?
        {
            Entry::Timer { live: true, run } => Ok(*run),
            Entry::Timer { live: false, .. } => Err(self.inner.error(name, Failure::Identity)),
            _ => Err(self.inner.error(name, Failure::WrongFamily)),
        }
    }

    pub(super) fn timer_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        self.timers.as_ref()?;
        match name {
            "TimerStart" => Some(self.start_timer(args)),
            "TimerGetElapsed" | "TimerGetRemaining" | "TimerGetTimeout" | "GetExpiredTimer" => {
                Some(self.timer_query(name, args))
            }
            "PauseTimer" | "ResumeTimer" => {
                self.inner.program.natives().get(name)?;
                Some(self.pause_timer(name, args))
            }
            _ => None,
        }
    }

    fn start_timer(&mut self, args: &[Value]) -> Result<Value, Error> {
        let name = "TimerStart";
        let fail = |reason| self.inner.error(name, reason);
        let [
            Value::Handle(timer),
            timeout,
            Value::Bool(periodic),
            handler,
        ] = args
        else {
            return Err(fail(Failure::TimerArguments));
        };
        let previous = self.timer_run(*timer, name)?;
        let timeout = match timeout {
            Value::Real(value) => f64::from(*value),
            Value::Int(value) => f64::from(*value as f32),
            _ => return Err(fail(Failure::TimerArguments)),
        };
        let handler = match handler {
            Value::Null => None,
            Value::Code(id) => {
                let function = self
                    .inner
                    .program
                    .functions()
                    .get(id.index())
                    .ok_or_else(|| fail(Failure::TimerState("callback is not loaded")))?;
                if !function.params.is_empty()
                    || function.ret.array
                    || function.ret.kind.name() != "nothing"
                {
                    return Err(fail(Failure::TimerState(
                        "callback must take and return nothing",
                    )));
                }
                Some(*id)
            }
            _ => return Err(fail(Failure::TimerArguments)),
        };
        let mut run = self.new_timer_run(timeout, *periodic, handler, name)?;
        run.query = self
            .numeric
            .as_ref()
            .and_then(|n| n.timer_queries().map(|rules| (n, rules)))
            .map(|(numeric, rules)| {
                rules
                    .begin(
                        numeric,
                        self.timer_query_now(),
                        timeout as f32,
                        usize::from(u16::MAX),
                    )
                    .map(|epoch| QueryRun {
                        epoch,
                        cached_remaining: rules.zero(),
                        resume_dispatch: false,
                    })
                    .map_err(|_| {
                        fail(Failure::TimerState(
                            "unsupported or over-budget native timer query",
                        ))
                    })
            })
            .transpose()?;
        if let Some(query) = run.query {
            let rules = self
                .numeric
                .as_ref()
                .and_then(|n| n.timer_queries())
                .expect("query constructed above");
            run.deadline = Some(
                deadline(run.start, rules.delivery_delay(query.epoch, timeout))
                    .ok_or_else(|| fail(Failure::TimerState("native timer deadline overflow")))?,
            );
        }
        if let Some(query) = &mut run.query
            && let Some(previous) = previous
        {
            let previous = previous.query.ok_or_else(|| {
                fail(Failure::TimerState(
                    "query policy attached after timer start",
                ))
            })?;
            if !self
                .numeric
                .as_ref()
                .and_then(|n| n.timer_queries())
                .is_some_and(|rules| rules.check_epoch(previous.epoch))
            {
                return Err(fail(Failure::TimerState("timer query policy changed")));
            }
            query.cached_remaining = previous.cached_remaining;
        }
        if let Entry::Timer { run: slot, .. } = self.inner.entry_mut(*timer)? {
            *slot = Some(run);
        }
        self.commit_timer_sequence(&run);
        Ok(Value::Null)
    }

    /// Pause keeps the elapsed time; resume restarts from it. Unstarted,
    /// expired or already-matching timers are unchanged.
    fn pause_timer(&mut self, name: &str, args: &[Value]) -> Result<Value, Error> {
        let fail = |reason| self.inner.error(name, reason);
        let timer = match args {
            [Value::Null] => return Ok(Value::Null),
            [Value::Handle(timer)] => *timer,
            _ => return Err(fail(Failure::TimerArguments)),
        };
        let Some(mut run) = self.timer_run(timer, name)? else {
            return Ok(Value::Null);
        };
        let now = self
            .timers
            .as_ref()
            .ok_or_else(|| fail(Failure::TimerState("not admitted")))?
            .now;
        if let (Some(numeric), Some(query)) = (self.numeric.as_ref(), run.query.as_mut()) {
            let rules = numeric
                .timer_queries()
                .ok_or_else(|| fail(Failure::TimerState("timer query policy changed")))?;
            if !rules.check_epoch(query.epoch) {
                return Err(fail(Failure::TimerState("timer query policy changed")));
            }
            let native_now = self.timer_query_now();
            match (name, run.paused, run.deadline) {
                ("PauseTimer", None, Some(_)) => {
                    let epoch = self
                        .timers
                        .as_ref()
                        .and_then(|s| s.active)
                        .filter(|a| a.timer == Some(timer) && a.sequence == run.sequence)
                        .and_then(|a| a.query_epoch)
                        .unwrap_or(query.epoch);
                    query.cached_remaining = rules
                        .remaining(numeric, epoch, native_now)
                        .map_err(|_| fail(Failure::TimerState("unsupported native timer query")))?;
                }
                ("ResumeTimer", Some(_), _) => {
                    query.epoch = rules
                        .begin(
                            numeric,
                            native_now,
                            query.cached_remaining,
                            usize::from(u16::MAX),
                        )
                        .map_err(|_| {
                            fail(Failure::TimerState(
                                "unsupported or over-budget native timer query",
                            ))
                        })?;
                }
                _ => {}
            }
        }
        match (name, run.paused, run.deadline) {
            ("PauseTimer", None, Some(_)) => {
                run.paused = Some((now - run.start).clamp(0.0, run.timeout));
                run.deadline = None;
                if let Some(query) = &mut run.query {
                    run.periodic = false;
                    query.resume_dispatch = false;
                }
            }
            ("ResumeTimer", Some(held), _) => {
                run.start = now - held;
                run.deadline = Some(run.start + run.timeout);
                if let Some(query) = &mut run.query {
                    let rules = self
                        .numeric
                        .as_ref()
                        .and_then(|n| n.timer_queries())
                        .expect("query checked above");
                    run.deadline = Some(
                        deadline(
                            now,
                            rules.delivery_delay(query.epoch, f64::from(query.cached_remaining)),
                        )
                        .ok_or_else(|| {
                            fail(Failure::TimerState("native resume deadline overflow"))
                        })?,
                    );
                    query.resume_dispatch = true;
                    run.start = now;
                    run.sequence = self
                        .timers
                        .as_ref()
                        .expect("admitted above")
                        .sequence
                        .checked_add(1)
                        .ok_or_else(|| fail(Failure::Capacity))?;
                }
                run.paused = None;
            }
            _ => return Ok(Value::Null),
        }
        if let Entry::Timer { run: slot, .. } = self.inner.entry_mut(timer)? {
            *slot = Some(run);
        }
        if name == "ResumeTimer" && run.query.is_some() {
            self.commit_timer_sequence(&run);
        }
        Ok(Value::Null)
    }

    pub(super) fn new_timer_run(
        &self,
        timeout: f64,
        periodic: bool,
        handler: Option<FunctionId>,
        name: &str,
    ) -> Result<Run, Error> {
        let fail = |why| self.inner.error(name, why);
        let state = self
            .timers
            .as_ref()
            .ok_or_else(|| fail(Failure::TimerState("not admitted")))?;
        // A one-shot zero timeout expires at the next boundary (recreation choice;
        // `CinematicFadeBJ` passes zero durations). Periodic zero stays refused.
        let immediate = timeout == 0.0 && !periodic;
        let deadline = if immediate {
            Some(state.now)
        } else {
            deadline(state.now, timeout)
        };
        let deadline = deadline.ok_or_else(|| {
            fail(Failure::TimerState(
                "timeout must produce a finite later deadline",
            ))
        })?;
        let sequence = state
            .sequence
            .checked_add(1)
            .ok_or_else(|| fail(Failure::Capacity))?;
        Ok(Run {
            timeout,
            start: state.now,
            deadline: Some(deadline),
            periodic,
            handler,
            sequence,
            paused: None,
            query: None,
        })
    }

    pub(super) fn commit_timer_sequence(&mut self, run: &Run) {
        self.timers.as_mut().expect("admitted above").sequence = run.sequence;
    }

    fn timer_query_now(&self) -> f32 {
        let state = self.timers.as_ref().expect("admitted timer runtime");
        // shortcut: outside timer callbacks the caller supplies the clock; qualify
        // the application clock producer before claiming frame-level parity.
        state
            .active
            .and_then(|a| a.query_now)
            .unwrap_or(state.now as f32)
    }

    fn timer_query(&self, name: &str, args: &[Value]) -> Result<Value, Error> {
        let fail = |reason| self.inner.error(name, reason);
        let state = self
            .timers
            .as_ref()
            .ok_or_else(|| fail(Failure::TimerState("not admitted")))?;
        if name == "GetExpiredTimer" {
            if !args.is_empty() {
                return Err(fail(Failure::TimerArguments));
            }
            let active = state
                .active
                .ok_or_else(|| fail(Failure::TimerState("no expiry callback")))?;
            let Some(timer) = active.timer else {
                return Ok(Value::Null);
            };
            self.timer_run(timer, name)?;
            return Ok(Value::Handle(timer));
        }
        let [Value::Handle(timer)] = args else {
            return Err(fail(Failure::TimerArguments));
        };
        let Some(run) = self.timer_run(*timer, name)? else {
            return Ok(Value::Real(0.0));
        };
        if let Some(query) = run.query {
            let numeric = self
                .numeric
                .as_ref()
                .ok_or_else(|| fail(Failure::TimerState("timer query policy changed")))?;
            let rules = numeric
                .timer_queries()
                .ok_or_else(|| fail(Failure::TimerState("timer query policy changed")))?;
            if !rules.check_epoch(query.epoch) {
                return Err(fail(Failure::TimerState("timer query policy changed")));
            }
            let result = if name == "TimerGetTimeout" {
                Ok(run.timeout as f32)
            } else if run.deadline.is_none() {
                if name == "TimerGetRemaining" {
                    Ok(query.cached_remaining)
                } else {
                    rules.inactive_elapsed(numeric, run.timeout as f32, query.cached_remaining)
                }
            } else {
                let epoch = state
                    .active
                    .filter(|a| a.timer == Some(*timer) && a.sequence == run.sequence)
                    .and_then(|a| a.query_epoch)
                    .unwrap_or(query.epoch);
                rules
                    .remaining(numeric, epoch, self.timer_query_now())
                    .and_then(|remaining| {
                        if name == "TimerGetRemaining" {
                            Ok(remaining)
                        } else {
                            rules.elapsed(numeric, epoch, remaining)
                        }
                    })
            };
            return result
                .map(Value::Real)
                .map_err(|_| fail(Failure::TimerState("unsupported native timer query")));
        }
        if self
            .numeric
            .as_ref()
            .is_some_and(|n| n.timer_queries().is_some())
        {
            return Err(fail(Failure::TimerState(
                "query policy attached after timer start",
            )));
        }
        let elapsed = if let Some(held) = run.paused {
            held
        } else if run.deadline.is_none() {
            run.timeout
        } else if let Some(active) = state
            .active
            .filter(|active| active.timer == Some(*timer) && active.sequence == run.sequence)
        {
            active.timeout
        } else {
            (state.now - run.start).clamp(0.0, run.timeout)
        };
        let value = match name {
            "TimerGetElapsed" => elapsed,
            "TimerGetRemaining" => run.timeout - elapsed,
            _ => run.timeout,
        };
        Ok(Value::Real(value as f32))
    }

    /// Validate a delivery boundary without advancing the admitted clock.
    pub fn validate_timer_target(&self, until: f64) -> Result<(), Error> {
        let fail = |reason| self.inner.error("timer delivery", reason);
        let state = self
            .timers
            .as_ref()
            .ok_or_else(|| fail(Failure::TimerState("not admitted")))?;
        if !until.is_finite() || until < state.now || state.active.is_some() {
            return Err(fail(Failure::TimerState(
                "invalid time or unfinished expiry",
            )));
        }
        Ok(())
    }

    /// Prepare at most one firing, in deadline/start order (a recreation tie policy).
    /// Work is bounded by the arena and retained registration counts per firing. A budget
    /// refusal preserves the next firing; previously delivered effects remain.
    /// Trigger snapshots are prepared before consuming the deadline.
    pub fn next_timer_expiry(
        &mut self,
        until: f64,
        budget: &mut usize,
    ) -> Result<Option<TimerExpiry>, Error> {
        let fail = |reason| self.inner.error("timer delivery", reason);
        self.validate_timer_target(until)?;
        let state = self.timers.as_ref().expect("validated timer clock");
        let next = self
            .inner
            .timer_slots
            .iter()
            .filter_map(|&index| {
                let (run, timed) = match &self.inner.arena[index] {
                    Entry::Timer {
                        live: true,
                        run: Some(run),
                    } => (*run, false),
                    Entry::RegisteredTimedEvent { trigger, run }
                        if matches!(self.inner.entry(*trigger), Ok(Entry::Trigger { .. })) =>
                    {
                        (*run, true)
                    }
                    _ => return None,
                };
                run.deadline
                    .filter(|at| *at <= until)
                    .map(|at| (index, run, at, timed))
            })
            .min_by(|a, b| expiry_order((a.2, a.1.sequence), (b.2, b.1.sequence)));
        let Some((index, mut run, at, timed)) = next else {
            self.timers.as_mut().expect("admitted above").now = until;
            return Ok(None);
        };
        let source = self.inner.id(index, "timer delivery")?;
        if *budget == 0 {
            return Err(fail(Failure::Budget("timer delivery")));
        }
        if let Some(query) = run.query
            && !self
                .numeric
                .as_ref()
                .and_then(|n| n.timer_queries())
                .is_some_and(|rules| rules.check_epoch(query.epoch))
        {
            return Err(fail(Failure::TimerState("timer query policy changed")));
        }
        let resume_dispatch = run.query.is_some_and(|q| q.resume_dispatch);
        if let Some(query) = &mut run.query {
            query.resume_dispatch = false;
        }
        let query_now = run.query.map(|q| q.epoch.expiry);
        let mut query_epoch = run.query.map(|q| q.epoch);
        if run.periodic
            && let Some(query) = &mut run.query
        {
            let numeric = self
                .numeric
                .as_ref()
                .ok_or_else(|| fail(Failure::TimerState("timer query policy changed")))?;
            let rules = numeric
                .timer_queries()
                .ok_or_else(|| fail(Failure::TimerState("timer query policy changed")))?;
            let next = rules
                .begin(
                    numeric,
                    query.epoch.expiry,
                    run.timeout as f32,
                    usize::from(u16::MAX),
                )
                .map_err(|_| {
                    fail(Failure::TimerState(
                        "unsupported or over-budget native timer query",
                    ))
                })?;
            if query.epoch.segmented() {
                query_epoch = Some(next);
            }
            query.epoch = next;
        }
        let triggers = self.prepare_timer_triggers(source, timed)?;
        let sequence = state
            .sequence
            .checked_add(1)
            .ok_or_else(|| fail(Failure::Capacity))?;
        run.start = at;
        run.deadline = if run.periodic {
            Some(
                deadline(
                    at,
                    run.query
                        .map(|q| {
                            self.numeric
                                .as_ref()
                                .and_then(|n| n.timer_queries())
                                .expect("query checked above")
                                .delivery_delay(q.epoch, run.timeout)
                        })
                        .unwrap_or(run.timeout),
                )
                .ok_or_else(|| fail(Failure::TimerState("periodic deadline overflow")))?,
            )
        } else {
            None
        };
        run.sequence = sequence;
        let state = self.timers.as_mut().expect("admitted above");
        state.now = at;
        state.sequence = sequence;
        state.active = Some(Expired {
            source,
            timer: (!timed).then_some(source),
            sequence,
            timeout: run.timeout,
            query_now,
            query_epoch,
            resume_dispatch,
        });
        match &mut self.inner.arena[index] {
            Entry::Timer { run: slot, .. } => *slot = Some(run),
            Entry::RegisteredTimedEvent { run: slot, .. } => *slot = run,
            _ => unreachable!("selected a timer run"),
        }
        *budget -= 1;
        Ok(Some(TimerExpiry {
            source,
            sequence,
            callback: run.handler,
            triggers,
        }))
    }

    pub(super) fn check_timer_expiry(&self, expiry: &TimerExpiry) -> Result<(), Error> {
        if !self
            .timers
            .as_ref()
            .and_then(|s| s.active)
            .is_some_and(|a| a.source == expiry.source && a.sequence == expiry.sequence)
        {
            return Err(self.inner.error(
                "timer delivery",
                Failure::TimerState("expiry context mismatch"),
            ));
        }
        Ok(())
    }

    /// Finish the source resume-dispatch rearm, then clear callback context.
    /// Normal expiries preserve a callback's restart or destroy.
    pub fn finish_timer_expiry(&mut self, expiry: TimerExpiry) -> Result<(), Error> {
        self.check_timer_expiry(&expiry)?;
        self.finish_timer_trigger();
        let active = self
            .timers
            .as_ref()
            .and_then(|s| s.active)
            .expect("checked above");
        let rearm = if active.resume_dispatch {
            active
                .timer
                .and_then(|timer| match self.inner.entry(timer) {
                    Ok(Entry::Timer {
                        live: true,
                        run: Some(run),
                    }) => run.handler.map(|handler| (timer, *run, handler)),
                    _ => None,
                })
        } else {
            None
        };
        let result = if let Some((timer, run, handler)) = rearm {
            self.start_timer(&[
                Value::Handle(timer),
                Value::Real(run.timeout as f32),
                Value::Bool(run.periodic),
                Value::Code(handler),
            ])
            .map(|_| ())
        } else {
            Ok(())
        };
        self.timers.as_mut().expect("checked above").active = None;
        result
    }
}

fn deadline(now: f64, timeout: f64) -> Option<f64> {
    let at = now + timeout;
    (timeout.is_finite() && timeout > 0.0 && at.is_finite() && at > now).then_some(at)
}

/// Delivery order across armed expiries: earliest deadline first; exact ties
/// break by creation/restart sequence, so a restarted timer loses its old
/// slot. This is an explicit recreation tie policy: native equal-deadline
/// order remains open (see `docs/re-notes/legacy-timer-start.md`). Timer and
/// timed-event runs share this one queue; kinds never reorder around it.
fn expiry_order(left: (f64, u64), right: (f64, u64)) -> std::cmp::Ordering {
    left.0.total_cmp(&right.0).then(left.1.cmp(&right.1))
}

//! Positive-timeout timers on the existing arena, driven by caller game time.
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
}

/// One prepared expiry. Finish it even when its callback fails.
#[derive(Debug)]
pub struct TimerExpiry {
    source: HandleId,
    sequence: u64,
    pub callback: Option<FunctionId>,
    triggers: Vec<super::TriggerDelivery>,
}

impl TimerExpiry {
    pub fn triggers(&self) -> &[super::TriggerDelivery] {
        &self.triggers
    }
}

impl ObjectHost {
    /// Opt in before script initialization. No wall clock, speed or pause policy is inferred.
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
        self.timer_run(*timer, name)?;
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
        let run = self.new_timer_run(timeout, *periodic, handler, name)?;
        if let Entry::Timer { run: slot, .. } = self.inner.entry_mut(*timer)? {
            *slot = Some(run);
        }
        self.commit_timer_sequence(&run);
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
        let deadline = deadline(state.now, timeout).ok_or_else(|| {
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
        })
    }

    pub(super) fn commit_timer_sequence(&mut self, run: &Run) {
        self.timers.as_mut().expect("admitted above").sequence = run.sequence;
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
        let run = self
            .timer_run(*timer, name)?
            .ok_or_else(|| fail(Failure::TimerState("unstarted timer query is unsupported")))?;
        let elapsed = if run.deadline.is_none() {
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
        let state = self
            .timers
            .as_ref()
            .ok_or_else(|| fail(Failure::TimerState("not admitted")))?;
        if !until.is_finite() || until < state.now || state.active.is_some() {
            return Err(fail(Failure::TimerState(
                "invalid time or unfinished expiry",
            )));
        }
        let next = self
            .inner
            .arena
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                let (run, timed) = match entry {
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
            .min_by(|a, b| a.2.total_cmp(&b.2).then(a.1.sequence.cmp(&b.1.sequence)));
        let Some((index, mut run, at, timed)) = next else {
            self.timers.as_mut().expect("admitted above").now = until;
            return Ok(None);
        };
        let source = self.inner.id(index, "timer delivery")?;
        if *budget == 0 {
            return Err(fail(Failure::Budget("timer delivery")));
        }
        let triggers = self.prepare_timer_triggers(source, timed)?;
        let sequence = state
            .sequence
            .checked_add(1)
            .ok_or_else(|| fail(Failure::Capacity))?;
        run.start = at;
        run.deadline = if run.periodic {
            Some(
                deadline(at, run.timeout)
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

    /// Clear callback context without overwriting a callback's restart or destroy.
    pub fn finish_timer_expiry(&mut self, expiry: TimerExpiry) -> Result<(), Error> {
        self.check_timer_expiry(&expiry)?;
        self.finish_timer_trigger();
        self.timers.as_mut().expect("checked above").active = None;
        Ok(())
    }
}

fn deadline(now: f64, timeout: f64) -> Option<f64> {
    let at = now + timeout;
    (timeout.is_finite() && timeout > 0.0 && at.is_finite() && at > now).then_some(at)
}

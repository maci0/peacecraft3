//! Script threads: per-thread trigger context and `TriggerSleepAction` parking.
//! Legacy sleeps use caller game time; native sleeps use the real queue.
//! Equal deadlines retain the explicit recreation tie policy.
use super::{Error, Failure, ObjectHost};
use crate::native_game_clock::{Domain, Point, SleepRequest};
use pcx_script::{HandleId, Value};
use std::collections::BTreeMap;

pub(super) const SLEEP: &str = "TriggerSleepAction";
const WAIT_FOR_SOUND: &str = "TriggerWaitForSound";

/// Event responses a thread keeps across a park.
#[derive(Clone, Copy, Default)]
pub(super) struct Context {
    trigger: Option<HandleId>,
    event: Option<HandleId>,
    range: Option<(HandleId, HandleId)>,
    subject: super::EventSubject,
    expired_timer: Option<Option<HandleId>>,
}

#[derive(Clone, Copy)]
struct Sleep {
    legacy_deadline: f64,
    native: Option<SleepRequest>,
}
struct Sleeper {
    sleep: Sleep,
    context: Context,
}

pub(crate) struct PreparedNativeThreadRows {
    rows: Vec<(u64, SleepRequest, SleepRequest)>,
}
#[derive(Clone, Copy)]
pub(crate) struct NativeThreadWake {
    pub ticket: u64,
    pub point: Point,
    pub terminal: bool,
    request: SleepRequest,
    advanced: SleepRequest,
}

#[derive(Default)]
pub(super) struct Threads {
    /// Caller contexts of running spawned/resumed threads, with the callback depth
    /// each thread started at. A park must happen at that depth.
    stack: Vec<(Context, usize)>,
    sleepers: BTreeMap<u64, Sleeper>,
    pending: Option<Result<Sleep, String>>,
    next: u64,
    /// None inherits native delivery; Some(None) retains a null event response.
    expired_timer: Option<Option<HandleId>>,
    /// `Some` once `TriggerSleepAction` is admitted, bounding parked threads.
    limit: Option<usize>,
}

impl ObjectHost {
    /// Requires the trigger runtime and the declaration in `CreateTrigger`'s source.
    pub fn admit_thread_sleep(&mut self, max_parked: usize) -> Result<(), Error> {
        let fail = |reason| self.inner.error(SLEEP, Failure::TriggerState(reason));
        if self.trigger_runtime.is_none() || self.threads.limit.is_some() {
            return Err(fail("requires the trigger runtime, once"));
        }
        Self::contract_in(&self.inner, SLEEP, &["real"], "nothing", false, None, false)?;
        if self.inner.program.natives().get(WAIT_FOR_SOUND).is_some() {
            Self::contract_in(
                &self.inner,
                WAIT_FOR_SOUND,
                &["sound", "real"],
                "nothing",
                false,
                None,
                false,
            )?;
        }
        let natives = self.inner.program.natives();
        if natives.get(SLEEP).map(|n| &n.source) != natives.get("CreateTrigger").map(|n| &n.source)
        {
            return Err(self.inner.error(SLEEP, Failure::Source));
        }
        std::alloc::Layout::array::<(u64, Sleeper)>(max_parked)
            .map_err(|_| self.inner.error(SLEEP, Failure::Capacity))?;
        self.threads.limit = Some(max_parked);
        Ok(())
    }

    pub fn parked_thread_count(&self) -> usize {
        self.threads.sleepers.len()
    }

    /// The parked thread due first at or before `until`, unless a timer is due
    /// no later. Advances the game clock to that deadline; resume it next.
    pub fn next_thread_wake(&mut self, until: f64) -> Result<Option<u64>, Error> {
        let Some((&ticket, sleeper)) = self
            .threads
            .sleepers
            .iter()
            .filter(|(_, sleeper)| sleeper.sleep.legacy_deadline <= until)
            .min_by(|a, b| {
                a.1.sleep
                    .legacy_deadline
                    .total_cmp(&b.1.sleep.legacy_deadline)
                    .then(a.0.cmp(b.0))
            })
        else {
            return Ok(None);
        };
        let deadline = sleeper.sleep.legacy_deadline;
        if self.next_timer_deadline().is_some_and(|at| at <= deadline) {
            return Ok(None);
        }
        // shortcut: legacy sleep deadlines do not identify native real-queue entries; bind that queue before resuming them.
        if self.native_clock_driving() {
            return Err(self.inner.error(
                SLEEP,
                Failure::GameClock("parked map wake has no native real-queue entry".into()),
            ));
        }
        self.set_timer_game_time(deadline)?;
        Ok(Some(ticket))
    }

    pub(crate) fn prepare_native_thread_admission(
        &self,
        work: &mut usize,
    ) -> Result<PreparedNativeThreadRows, Error> {
        self.prepare_native_thread_rows(None, work)
    }
    pub(crate) fn prepare_native_thread_rebase(
        &self,
        domain: Domain,
        work: &mut usize,
    ) -> Result<PreparedNativeThreadRows, Error> {
        if domain != Domain::Real {
            return Ok(PreparedNativeThreadRows { rows: Vec::new() });
        }
        self.prepare_native_thread_rows(Some(domain), work)
    }
    fn prepare_native_thread_rows(
        &self,
        rebase: Option<Domain>,
        work: &mut usize,
    ) -> Result<PreparedNativeThreadRows, Error> {
        let fail = |error: String| self.inner.error(SLEEP, Failure::GameClock(error));
        let mut rows = Vec::new();
        rows.try_reserve_exact(self.threads.sleepers.len())
            .map_err(|_| fail("script sleep admission allocation failed".into()))?;
        for (&ticket, sleeper) in &self.threads.sleepers {
            *work = work
                .checked_sub(1)
                .ok_or_else(|| fail("script sleep admission work exhausted".into()))?;
            let before = sleeper.sleep.native.ok_or_else(|| {
                fail("parked script sleep has no qualified REAL queue request".into())
            })?;
            if before.domain() != Domain::Real {
                return Err(fail(
                    "script sleep request has a foreign queue family".into(),
                ));
            }
            let mut after = before;
            if rebase.is_some() {
                self.rebase_native_sleep(&mut after)?;
            } else {
                after = self.activate_native_sleep(before, work)?;
            }
            rows.push((ticket, before, after));
        }
        Ok(PreparedNativeThreadRows { rows })
    }
    /// No script, source refresh or queue mutation may run between preflight and commit.
    pub(crate) fn commit_native_thread_rows(&mut self, prepared: PreparedNativeThreadRows) {
        for (ticket, before, after) in prepared.rows {
            let sleeper = self
                .threads
                .sleepers
                .get_mut(&ticket)
                .expect("unchanged preflighted script sleeper");
            debug_assert_eq!(sleeper.sleep.native, Some(before));
            sleeper.sleep.native = Some(after);
        }
    }
    pub(crate) fn next_native_thread_wake(
        &self,
        until: Point,
        work: &mut usize,
    ) -> Result<Option<NativeThreadWake>, Error> {
        let fail = |error: &str| self.inner.error(SLEEP, Failure::GameClock(error.into()));
        let mut selected: Option<(u64, Point, SleepRequest)> = None;
        for (&ticket, sleeper) in &self.threads.sleepers {
            *work = work
                .checked_sub(1)
                .ok_or_else(|| fail("native thread selection work exhausted"))?;
            let request = sleeper
                .sleep
                .native
                .ok_or_else(|| fail("parked script sleep lacks a native REAL request"))?;
            if request.domain() != Domain::Real {
                return Err(fail("script sleep request has a foreign queue family"));
            }
            let point = request
                .boundary()
                .ok_or_else(|| fail("script sleep request has not been natively admitted"))?;
            // Valid queue locals are nonnegative finite source words; ticket ties retain the reconstruction policy.
            let key = (point.cycle, point.local, ticket);
            if (point.cycle, point.local) > (until.cycle, until.local) {
                continue;
            }
            if selected.is_none_or(|(previous, old, _)| key < (old.cycle, old.local, previous)) {
                selected = Some((ticket, point, request));
            }
        }
        let Some((ticket, point, request)) = selected else {
            return Ok(None);
        };
        let mut advanced = request;
        let terminal = self.advance_native_sleep(&mut advanced)?;
        Ok(Some(NativeThreadWake {
            ticket,
            point,
            terminal,
            request,
            advanced,
        }))
    }
    pub(crate) fn validate_native_thread_wake(&self, wake: &NativeThreadWake) -> Result<(), Error> {
        let sleeper = self.threads.sleepers.get(&wake.ticket).ok_or_else(|| {
            self.inner.error(
                SLEEP,
                Failure::GameClock("selected script sleeper is absent".into()),
            )
        })?;
        if sleeper.sleep.native != Some(wake.request) || wake.request.boundary() != Some(wake.point)
        {
            return Err(self.inner.error(
                SLEEP,
                Failure::GameClock("selected script sleep request changed".into()),
            ));
        }
        Ok(())
    }
    /// Final rows remain owned until Host::resume_thread installs their retained context.
    pub(crate) fn commit_native_thread_wake(
        &mut self,
        wake: NativeThreadWake,
    ) -> Result<bool, Error> {
        self.validate_native_thread_wake(&wake)?;
        if !wake.terminal {
            self.threads
                .sleepers
                .get_mut(&wake.ticket)
                .expect("validated script sleeper")
                .sleep
                .native = Some(wake.advanced);
        }
        Ok(wake.terminal)
    }

    pub(super) fn thread_sleep_requested(&mut self, name: &str, args: &[Value]) -> bool {
        if name == SLEEP || name == WAIT_FOR_SOUND {
            self.threads.pending = None;
        }
        if name == WAIT_FOR_SOUND && self.threads.limit.is_some() {
            let (Some(now), [Value::Handle(sound), offset]) = (self.timer_game_time(), args) else {
                return false;
            };
            let (Ok(remaining), Some(offset)) = (self.sound_remaining(*sound), offset.as_real())
            else {
                return false;
            };
            let wait = remaining - f64::from(offset);
            if wait.is_nan() || wait <= 0.0 {
                return false;
            }
            self.threads.pending = Some(if self.native_clock_driving() {
                Err("TriggerWaitForSound native queue family is not qualified".into())
            } else {
                Ok(Sleep {
                    legacy_deadline: now + wait,
                    native: None,
                })
            });
            return self.threads.pending.as_ref().is_some_and(Result::is_ok);
        }
        if name != SLEEP || self.threads.limit.is_none() {
            return false;
        }
        let (Some(now), [Value::Real(seconds)]) = (self.timer_game_time(), args) else {
            return false;
        };
        if !seconds.is_finite() {
            return false;
        }
        // One request admission is bounded; actual internal queue entries spend the drain budget.
        let mut work = 1;
        let prepared = self
            .prepare_native_sleep(Domain::Real, *seconds, &mut work)
            .map(|native| Sleep {
                legacy_deadline: now + f64::from(seconds.max(0.0)),
                native,
            })
            .map_err(|error| error.to_string());
        let accepted = prepared.is_ok();
        self.threads.pending = Some(prepared);
        accepted
    }

    /// Reached only when a sleep could not park: invalid input or no runtime.
    /// A sound wait that needs no park returns at once.
    pub(super) fn thread_sleep_native(&self, name: &str) -> Option<Result<Value, Error>> {
        if matches!(name, SLEEP | WAIT_FOR_SOUND)
            && let Some(Err(error)) = &self.threads.pending
        {
            return Some(Err(self
                .inner
                .error(name, Failure::GameClock(error.clone()))));
        }
        if name == WAIT_FOR_SOUND && self.threads.limit.is_some() {
            self.inner.program.natives().get(name)?;
            return Some(Ok(Value::Null));
        }
        (name == SLEEP && self.threads.limit.is_some()).then(|| {
            Err(self.inner.error(
                name,
                Failure::TriggerState("finite duration and game clock required"),
            ))
        })
    }

    fn thread_context(&self) -> Context {
        Context {
            trigger: self.trigger_runtime.as_ref().and_then(|s| s.active),
            event: self.trigger_runtime.as_ref().and_then(|s| s.event_context),
            range: self.range_context(),
            subject: self.events.subject,
            expired_timer: self.threads.expired_timer,
        }
    }

    fn install_thread_context(&mut self, context: Context) {
        if let Some(state) = &mut self.trigger_runtime {
            state.active = context.trigger;
            state.event_context = context.event;
        }
        self.set_range_context(context.range);
        self.events.subject = context.subject;
        self.threads.expired_timer = context.expired_timer;
    }

    pub(super) fn thread_expired_timer(&self) -> Option<Option<HandleId>> {
        self.threads.expired_timer
    }

    /// Depth of spawned or resumed threads currently active.
    pub(super) fn spawned_thread_depth(&self) -> usize {
        self.threads.stack.len()
    }

    /// Save the caller's context before a spawned thread installs its own.
    pub(super) fn enter_spawned_thread(&mut self) {
        let saved = (self.thread_context(), self.callbacks.len());
        self.threads.stack.push(saved);
    }

    pub(super) fn park_thread_now(&mut self) -> Result<u64, Error> {
        let fail = |reason| self.inner.error(SLEEP, Failure::TriggerState(reason));
        let sleep = self
            .threads
            .pending
            .take()
            .ok_or_else(|| fail("no pending sleep"))?
            .map_err(|error| self.inner.error(SLEEP, Failure::GameClock(error)))?;
        let base = self.threads.stack.last().map_or(0, |(_, base)| *base);
        if self.callbacks.len() != base {
            return Err(fail("sleep inside a native callback"));
        }
        if self
            .threads
            .limit
            .is_none_or(|limit| self.threads.sleepers.len() >= limit)
        {
            return Err(self.inner.error(SLEEP, Failure::Budget("parked threads")));
        }
        self.threads.next = self
            .threads
            .next
            .checked_add(1)
            .ok_or_else(|| fail("tickets"))?;
        let ticket = self.threads.next;
        let mut context = self.thread_context();
        context.expired_timer = context
            .expired_timer
            .or_else(|| self.active_expired_timer());
        self.threads
            .sleepers
            .insert(ticket, Sleeper { sleep, context });
        Ok(ticket)
    }

    pub(super) fn resume_thread_now(&mut self, ticket: u64) -> Result<(), Error> {
        let sleeper = self.threads.sleepers.remove(&ticket).ok_or_else(|| {
            self.inner
                .error(SLEEP, Failure::TriggerState("unknown thread ticket"))
        })?;
        self.enter_spawned_thread();
        self.install_thread_context(sleeper.context);
        Ok(())
    }

    pub(super) fn leave_thread_now(&mut self) -> Result<(), Error> {
        let (context, _) = self.threads.stack.pop().ok_or_else(|| {
            self.inner
                .error(SLEEP, Failure::TriggerState("no running thread to leave"))
        })?;
        self.install_thread_context(context);
        Ok(())
    }

    /// After an execution error: return to the outermost caller's context.
    pub(super) fn abort_threads(&mut self) {
        self.threads.pending = None;
        if let Some(&(context, _)) = self.threads.stack.first() {
            self.install_thread_context(context);
        }
        self.threads.stack.clear();
    }
}

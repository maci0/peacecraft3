//! Script threads: per-thread trigger context and `TriggerSleepAction` parking.
//! Sleeps run on the owned game clock. Ordering against timers is a recreation
//! policy: earlier deadline first, a timer before a sleeper at the same time.
use super::{Error, Failure, ObjectHost};
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
}

#[derive(Default)]
pub(super) struct Threads {
    /// Caller contexts of running spawned/resumed threads, with the callback depth
    /// each thread started at. A park must happen at that depth.
    stack: Vec<(Context, usize)>,
    sleepers: BTreeMap<u64, (f64, Context)>,
    pending: Option<f64>,
    next: u64,
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
        std::alloc::Layout::array::<(u64, f64, Context)>(max_parked)
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
        let Some((&ticket, &(deadline, _))) = self
            .threads
            .sleepers
            .iter()
            .filter(|(_, (at, _))| *at <= until)
            .min_by(|a, b| a.1.0.total_cmp(&b.1.0).then(a.0.cmp(b.0)))
        else {
            return Ok(None);
        };
        if self.next_timer_deadline().is_some_and(|at| at <= deadline) {
            return Ok(None);
        }
        self.set_timer_game_time(deadline)?;
        Ok(Some(ticket))
    }

    pub(super) fn thread_sleep_requested(&mut self, name: &str, args: &[Value]) -> bool {
        if name == WAIT_FOR_SOUND && self.threads.limit.is_some() {
            // Park only while the sound has playback left beyond the offset.
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
            self.threads.pending = Some(now + wait);
            return true;
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
        self.threads.pending = Some(now + f64::from(seconds.max(0.0)));
        true
    }

    /// Reached only when a sleep could not park: invalid input or no runtime.
    /// A sound wait that needs no park returns at once.
    pub(super) fn thread_sleep_native(&self, name: &str) -> Option<Result<Value, Error>> {
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
        }
    }

    fn install_thread_context(&mut self, context: Context) {
        if let Some(state) = &mut self.trigger_runtime {
            state.active = context.trigger;
            state.event_context = context.event;
        }
        self.set_range_context(context.range);
        self.events.subject = context.subject;
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
        let deadline = self
            .threads
            .pending
            .take()
            .ok_or_else(|| fail("no pending sleep"))?;
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
        let context = self.thread_context();
        self.threads.sleepers.insert(ticket, (deadline, context));
        Ok(ticket)
    }

    pub(super) fn resume_thread_now(&mut self, ticket: u64) -> Result<(), Error> {
        let (_, context) = self.threads.sleepers.remove(&ticket).ok_or_else(|| {
            self.inner
                .error(SLEEP, Failure::TriggerState("unknown thread ticket"))
        })?;
        self.enter_spawned_thread();
        self.install_thread_context(context);
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

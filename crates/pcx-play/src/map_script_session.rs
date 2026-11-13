//! Persistent execution of one admitted map with an explicitly supplied Host.
use crate::map_admission::ParsedMapContext;
use crate::scripts::{AssetIdentity, OwnedLocalizedHost};
use pcx_script::{Error, Host, OwnedVm, Value, value::FunctionId};
use std::sync::{Arc, Weak};

#[derive(Debug, thiserror::Error)]
#[error("{diagnostic}")]
pub struct ScriptExecutionFailure {
    pub identity: AssetIdentity,
    /// None denotes global initialization before the explicit entrypoint list.
    pub function: Option<FunctionId>,
    pub source_label: Option<String>,
    #[source]
    pub error: Error,
    pub diagnostic: String,
}
#[derive(Debug, thiserror::Error)]
pub enum ScriptSessionError {
    #[error("no script session has been initialized")]
    NoSession,
    #[error("script delivery cannot enter an unfinished timer projection")]
    PendingTimerProjection,
    #[error("{identity:?}: function id {id:?} is not declared in the admitted program")]
    UnknownFunction {
        identity: AssetIdentity,
        id: FunctionId,
    },
    #[error("{identity:?}: candidate Host construction failed: {source}")]
    Factory {
        identity: AssetIdentity,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("callback function id {id:?} belongs to another or replaced session")]
    StaleCallback { id: FunctionId },
    #[error(transparent)]
    Execution(#[from] Box<ScriptExecutionFailure>),
}
/// A callback selected for one published session, including its initialization.
/// Holding a ticket retains no discarded map, VM or Host.
#[derive(Clone, Debug)]
pub struct CallbackTicket {
    session: Weak<()>,
    function: FunctionId,
}
struct ActiveSession<H: Host> {
    witness: Arc<()>,
    context: Arc<ParsedMapContext>,
    vm: OwnedVm<OwnedLocalizedHost<H>>,
    timer_delivery: Option<TimerDelivery>,
}

struct TimerDelivery {
    expiry: crate::playercolor_host::TimerExpiry,
    trigger: usize,
    phase: TimerDeliveryPhase,
    projection_fault: Option<Arc<crate::playercolor_host::Error>>,
}

#[derive(Clone, Copy)]
enum TimerDeliveryPhase {
    MandatoryProjection,
    Trigger,
    Conditions { next: usize, accepted: bool },
    Actions { accepted: bool },
    Handler,
    Complete,
}
pub struct MapScriptSession<H: Host + Send + Sync + 'static> {
    active: Option<ActiveSession<H>>,
}
impl<H: Host + Send + Sync + 'static> Default for MapScriptSession<H> {
    fn default() -> Self {
        Self { active: None }
    }
}
impl<H: Host + Send + Sync + 'static> MapScriptSession<H> {
    fn reject_unfinished_projection(&self) -> Result<(), ScriptSessionError> {
        if self
            .active
            .as_ref()
            .and_then(|active| active.timer_delivery.as_ref())
            .is_some_and(|delivery| {
                matches!(delivery.phase, TimerDeliveryPhase::MandatoryProjection)
            })
        {
            return Err(ScriptSessionError::PendingTimerProjection);
        }
        Ok(())
    }
    pub fn context(&self) -> Option<&Arc<ParsedMapContext>> {
        self.active.as_ref().map(|active| &active.context)
    }
    /// Published-session witness. [`CallbackTicket`] stores this same `Arc`.
    /// Successful `replace` allocates the next witness before the previous one drops.
    /// `clear` drops it.
    pub fn session_witness(&self) -> Option<Weak<()>> {
        self.active
            .as_ref()
            .map(|active| Arc::downgrade(&active.witness))
    }
    pub fn host(&mut self) -> Option<&mut H> {
        self.active
            .as_mut()
            .map(|active| &mut active.vm.host().inner)
    }
    pub fn global(&self, name: &str) -> Option<Value> {
        self.active.as_ref().map(|active| active.vm.global(name))
    }
    /// The caller must isolate candidate Host effects from the active session.
    /// Failure preserves this resource, but cannot undo arbitrary external effects.
    /// An empty explicit entrypoint list initializes only globals.
    pub fn replace<F, E>(
        &mut self,
        context: Arc<ParsedMapContext>,
        create_host: F,
        entrypoints: &[FunctionId],
    ) -> Result<(), ScriptSessionError>
    where
        F: FnOnce(Arc<ParsedMapContext>) -> Result<H, E>,
        E: std::error::Error + Send + Sync + 'static,
    {
        self.replace_sequenced(context, create_host, entrypoints, |_, _| {
            Ok::<(), std::convert::Infallible>(())
        })
    }
    /// `between(vm, completed)` runs after globals (`completed == 0`) and after
    /// each entrypoint that returned. A between-callback error is a factory
    /// failure: the previous session stays published.
    #[cfg_attr(
        feature = "profile-cpu",
        tracing::instrument(skip_all, target = "pcx_cpu", name = "script.initialize")
    )]
    pub fn replace_sequenced<F, E, B, BE>(
        &mut self,
        context: Arc<ParsedMapContext>,
        create_host: F,
        entrypoints: &[FunctionId],
        mut between: B,
    ) -> Result<(), ScriptSessionError>
    where
        F: FnOnce(Arc<ParsedMapContext>) -> Result<H, E>,
        E: std::error::Error + Send + Sync + 'static,
        B: FnMut(&mut OwnedVm<OwnedLocalizedHost<H>>, usize) -> Result<(), BE>,
        BE: std::error::Error + Send + Sync + 'static,
    {
        for &id in entrypoints {
            function_name(&context, id)?;
        }
        let host =
            create_host(Arc::clone(&context)).map_err(|source| ScriptSessionError::Factory {
                identity: context.sources.map.clone(),
                source: Box::new(source),
            })?;
        let mut vm = Arc::clone(&context.scripts).owned_vm(host);
        if let Err(error) = vm.init_globals() {
            return Err(execution_failure(&context, &mut vm, None, error));
        }
        if let Err(source) = between(&mut vm, 0) {
            return Err(ScriptSessionError::Factory {
                identity: context.sources.map.clone(),
                source: Box::new(source),
            });
        }
        for (index, &id) in entrypoints.iter().enumerate() {
            let name = function_name(&context, id)?;
            if let Err(error) = vm.call(&name, &[]) {
                return Err(execution_failure(&context, &mut vm, Some(id), error));
            }
            if let Err(source) = between(&mut vm, index + 1) {
                return Err(ScriptSessionError::Factory {
                    identity: context.sources.map.clone(),
                    source: Box::new(source),
                });
            }
        }
        self.active = Some(ActiveSession {
            witness: Arc::new(()),
            context,
            vm,
            timer_delivery: None,
        });
        Ok(())
    }
    /// Mint immediately when binding a function from this active context.
    /// Do not rebind a queued bare ID after replacement; its origin is not encoded.
    pub fn callback_ticket(&self, id: FunctionId) -> Result<CallbackTicket, ScriptSessionError> {
        let active = self.active.as_ref().ok_or(ScriptSessionError::NoSession)?;
        function_name(&active.context, id)?;
        Ok(CallbackTicket {
            session: Arc::downgrade(&active.witness),
            function: id,
        })
    }
    /// Reject other/replaced session tickets before any VM or Host execution.
    pub fn call_callback(
        &mut self,
        ticket: &CallbackTicket,
        args: &[Value],
    ) -> Result<Value, ScriptSessionError> {
        let Some(active) = &self.active else {
            return Err(ScriptSessionError::StaleCallback {
                id: ticket.function,
            });
        };
        if !Weak::ptr_eq(&ticket.session, &Arc::downgrade(&active.witness)) {
            return Err(ScriptSessionError::StaleCallback {
                id: ticket.function,
            });
        }
        self.call_function(ticket.function, args)
    }
    /// Explicit direct calls must select IDs and arguments for the active context.
    /// Queued callbacks use call_callback and retain their original ticket.
    pub fn call_function(
        &mut self,
        id: FunctionId,
        args: &[Value],
    ) -> Result<Value, ScriptSessionError> {
        self.reject_unfinished_projection()?;
        let active = self.active.as_mut().ok_or(ScriptSessionError::NoSession)?;
        let name = function_name(&active.context, id)?;
        active
            .vm
            .call(&name, args)
            .map_err(|error| execution_failure(&active.context, &mut active.vm, Some(id), error))
    }
    /// Start trigger actions as one new thread; a sleep parks it in this VM.
    pub fn run_thread(&mut self, ids: &[FunctionId]) -> Result<(), ScriptSessionError> {
        self.reject_unfinished_projection()?;
        let active = self.active.as_mut().ok_or(ScriptSessionError::NoSession)?;
        for &id in ids {
            function_name(&active.context, id)?;
        }
        active.vm.run_thread(ids).map_err(|error| {
            execution_failure(&active.context, &mut active.vm, ids.first().copied(), error)
        })
    }
    pub fn resume_thread(&mut self, ticket: u64) -> Result<(), ScriptSessionError> {
        self.reject_unfinished_projection()?;
        let active = self.active.as_mut().ok_or(ScriptSessionError::NoSession)?;
        active
            .vm
            .resume_thread(ticket)
            .map_err(|error| execution_failure(&active.context, &mut active.vm, None, error))
    }
    pub fn clear(&mut self) {
        self.active = None;
    }
}
fn function_name(context: &ParsedMapContext, id: FunctionId) -> Result<String, ScriptSessionError> {
    context
        .scripts
        .program()
        .functions()
        .get(id.index())
        .map(|function| function.name.clone())
        .ok_or_else(|| ScriptSessionError::UnknownFunction {
            identity: context.sources.map.clone(),
            id,
        })
}
fn execution_failure<H: Host>(
    context: &ParsedMapContext,
    vm: &mut OwnedVm<OwnedLocalizedHost<H>>,
    function: Option<FunctionId>,
    error: Error,
) -> ScriptSessionError {
    let source_label = vm.error_source().map(str::to_owned);
    let diagnostic = vm.host().diagnostic(source_label.as_deref(), &error);
    ScriptSessionError::Execution(Box::new(ScriptExecutionFailure {
        identity: context.sources.map.clone(),
        function,
        source_label,
        error,
        diagnostic,
    }))
}

#[derive(Debug, thiserror::Error)]
pub enum TimerSessionError {
    #[error("timer delivery is already active")]
    PendingDelivery,
    #[error("mandatory world projection failed before timer delivery: {0}")]
    MandatoryProjection(#[source] Arc<crate::playercolor_host::Error>),
    #[error("campaign AI: {0}")]
    Ai(String),
    #[error("native queue: {0}")]
    NativeQueue(String),
    #[error("range callback work budget exhausted")]
    RangeBudget,
    #[error("thread wake budget exhausted")]
    WakeBudget,
    #[error("{0} queue work budget exhausted")]
    QueueBudget(&'static str),
    #[error("trigger condition {id:?} did not return boolean")]
    ConditionResult { id: FunctionId },
    #[error(transparent)]
    Session(#[from] ScriptSessionError),
    #[error(transparent)]
    Timer(#[from] crate::playercolor_host::Error),
}

fn queue_selection_error(error: crate::playercolor_host::Error) -> TimerSessionError {
    if let crate::playercolor_host::Failure::Budget(stage) = &error.failure {
        TimerSessionError::QueueBudget(stage)
    } else {
        error.into()
    }
}
fn source_queue_error(error: crate::native_game_clock::QueueError) -> TimerSessionError {
    match error {
        crate::native_game_clock::QueueError::Budget(stage) => {
            TimerSessionError::QueueBudget(stage)
        }
        crate::native_game_clock::QueueError::Invalid(reason) => {
            TimerSessionError::NativeQueue(reason)
        }
    }
}

impl MapScriptSession<crate::map_startup_host::MapStartupHost> {
    pub(crate) fn check_pending_projection(&self) -> Result<(), TimerSessionError> {
        if let Some(delivery) = self
            .active
            .as_ref()
            .and_then(|active| active.timer_delivery.as_ref())
        {
            if let Some(fault) = &delivery.projection_fault {
                return Err(TimerSessionError::MandatoryProjection(Arc::clone(fault)));
            }
            return Err(TimerSessionError::PendingDelivery);
        }
        Ok(())
    }

    #[cfg(any(test, feature = "test-fixtures"))]
    pub fn test_advance_timers_with_projection_work(
        &mut self,
        until: f64,
        mut budget: usize,
        mut work: usize,
    ) -> Result<usize, TimerSessionError> {
        self.check_pending_projection()?;
        let host = self.host().ok_or(ScriptSessionError::NoSession)?;
        let advances_clock = host.objects().game_paused() != Some(true);
        if !host.objects().native_clock_driving() {
            host.objects_mut()
                .select_legacy_clock_domain(until, advances_clock)?;
        }
        self.advance_native_timers(until, &mut budget, &mut work)
    }
    /// Queue-budget refusals may resume only while no selected delivery has faulted.
    pub fn native_pacing_can_resume(&mut self, error: &TimerSessionError) -> bool {
        matches!(
            error,
            TimerSessionError::QueueBudget(_) | TimerSessionError::WakeBudget
        ) && self.check_pending_projection().is_ok()
            && self
                .host()
                .is_some_and(|host| host.objects().check_native_physics_fault().is_ok())
    }

    /// Admit one independently supplied frame, or resume retained frame work.
    /// This transport does not infer game speed or real time from a legacy horizon.
    pub fn advance_native_frame(
        &mut self,
        input: Option<crate::native_game_clock::FrameInput>,
        budget: usize,
    ) -> Result<usize, TimerSessionError> {
        self.advance_native_input(
            input.map(crate::native_game_clock::Input::Independent),
            budget,
        )
    }
    /// Simulate fresh local GetTickCount input, retaining sub-millisecond carry.
    pub fn advance_native_counter_frame(
        &mut self,
        input: Option<std::time::Duration>,
        budget: usize,
    ) -> Result<usize, TimerSessionError> {
        self.advance_native_input(input.map(crate::native_game_clock::Input::Counter), budget)
    }
    fn advance_native_input(
        &mut self,
        input: Option<crate::native_game_clock::Input>,
        mut budget: usize,
    ) -> Result<usize, TimerSessionError> {
        use crate::native_game_clock::Domain;
        self.check_pending_projection()?;
        self.host()
            .ok_or(ScriptSessionError::NoSession)?
            .objects()
            .check_native_physics_fault()?;
        let mut work = 65536;
        if let Some(input) = input {
            let host = self.host().ok_or(ScriptSessionError::NoSession)?;
            let fresh = host.objects().native_game_point().is_some()
                && !host.objects().native_clock_driving();
            let prepared = if fresh {
                Some(
                    host.prepare_native_ai_sleep_admission(&mut work)
                        .map_err(TimerSessionError::Ai)?,
                )
            } else {
                None
            };
            host.objects_mut().enqueue_native_input(input, &mut work)?;
            if let Some(rows) = prepared {
                host.commit_native_ai_sleep_rows(rows);
            }
        }
        let mut delivered = 0;
        loop {
            let segment = self
                .host()
                .ok_or(ScriptSessionError::NoSession)?
                .objects_mut()
                .next_native_frame_segment(&mut work)
                .map_err(queue_selection_error)?;
            let Some((domain, segment)) = segment else {
                return Ok(delivered);
            };
            match domain {
                Domain::Game => {
                    let objects = self.host().ok_or(ScriptSessionError::NoSession)?.objects();
                    let until = objects.native_game_clock_seconds(segment.point)?;
                    delivered += self.advance_native_timers(until, &mut budget, &mut work)?;
                }
                Domain::Real => loop {
                    let wake = self
                        .host()
                        .ok_or(ScriptSessionError::NoSession)?
                        .objects()
                        .next_native_thread_wake(segment.point, &mut work)
                        .map_err(queue_selection_error)?;
                    let Some(wake) = wake else { break };
                    budget = budget.checked_sub(1).ok_or(TimerSessionError::WakeBudget)?;
                    let objects = self
                        .host()
                        .ok_or(ScriptSessionError::NoSession)?
                        .objects_mut();
                    objects.validate_native_thread_wake(&wake)?;
                    if !wake.terminal {
                        objects.commit_native_thread_wake(wake)?;
                        continue;
                    }
                    objects.begin_native_domain_callback(domain, wake.point)?;
                    let resumed = objects
                        .commit_native_thread_wake(wake)
                        .map_err(TimerSessionError::from)
                        .and_then(|_| {
                            self.resume_thread(wake.ticket)
                                .map_err(TimerSessionError::from)
                        })
                        .and_then(|_| self.advance_queued_events(budget));
                    let cleanup = self
                        .host()
                        .ok_or(ScriptSessionError::NoSession)?
                        .objects_mut()
                        .end_native_domain_callback();
                    let events = match (resumed, cleanup) {
                        (Err(error), Err(cleanup)) => {
                            return Err(TimerSessionError::NativeQueue(format!(
                                "{error}; scope cleanup: {cleanup}"
                            )));
                        }
                        (Err(error), Ok(())) => return Err(error),
                        (Ok(_), Err(error)) => return Err(error.into()),
                        (Ok(events), Ok(())) => events,
                    };
                    budget = budget
                        .checked_sub(events)
                        .ok_or(TimerSessionError::WakeBudget)?;
                    delivered += 1 + events;
                },
            }
            let host = self.host().ok_or(ScriptSessionError::NoSession)?;
            let ai = if domain == Domain::Game && segment.ends_cycle {
                Some(
                    host.prepare_native_ai_sleep_rebase(domain, &mut work)
                        .map_err(TimerSessionError::Ai)?,
                )
            } else {
                None
            };
            host.objects_mut()
                .finish_native_frame_segment(domain, segment, &mut work)?;
            if let Some(rows) = ai {
                host.commit_native_ai_sleep_rows(rows);
            }
        }
    }

    /// Sample admitted shared vision after script delivery. A blocked admission
    /// is retried, so placement or day/night that arrives later can still admit.
    /// An already live ledger is left in place. Sessions that stay blocked are
    /// unchanged. The work limit is the vision grid budget, not `max_expiries`.
    pub fn sample_admitted_vision(&mut self) -> Result<(), TimerSessionError> {
        if let Some(host) = self.host() {
            host.objects_mut().admit_shared_vision()?;
        }
        let admitted = self
            .host()
            .is_some_and(|host| host.objects().shared_vision_admitted());
        if !admitted {
            return Ok(());
        }
        let limit = self
            .host()
            .ok_or(ScriptSessionError::NoSession)?
            .objects()
            .shared_vision_work_limit()?;
        self.host()
            .ok_or(ScriptSessionError::NoSession)?
            .objects_mut()
            .sample_shared_vision(limit)?;
        Ok(())
    }

    /// Deliver positive-timeout callbacks in this published VM using explicit game time.
    /// Errors retain prior effects; the host clock records how far delivery reached.
    /// An executed game pause freezes future timer/wake delivery after the current event.
    #[cfg_attr(
        feature = "profile-cpu",
        tracing::instrument(skip_all, target = "pcx_cpu", name = "simulation.timers")
    )]
    pub fn advance_timers(
        &mut self,
        until: f64,
        mut budget: usize,
    ) -> Result<usize, TimerSessionError> {
        self.check_pending_projection()?;
        let host = self.host().ok_or(ScriptSessionError::NoSession)?;
        let advances_clock = host.objects().game_paused() != Some(true);
        host.objects_mut()
            .select_legacy_clock_domain(until, advances_clock)?;
        self.advance_native_timers(until, &mut budget, &mut 65536)
    }

    pub(crate) fn advance_native_timers(
        &mut self,
        until: f64,
        budget: &mut usize,
        activity_work: &mut usize,
    ) -> Result<usize, TimerSessionError> {
        self.check_pending_projection()?;
        let result = self.advance_timers_inner(until, budget, activity_work);
        if result.is_err()
            && let Some(host) = self.host()
        {
            host.objects_mut().discard_unit_activity_routes();
        }
        result
    }

    fn advance_timers_inner(
        &mut self,
        until: f64,
        budget: &mut usize,
        activity_work: &mut usize,
    ) -> Result<usize, TimerSessionError> {
        self.host()
            .ok_or(ScriptSessionError::NoSession)?
            .objects()
            .validate_timer_target(until)?;
        let mut delivered = 0;
        loop {
            self.host()
                .ok_or(ScriptSessionError::NoSession)?
                .objects()
                .check_native_physics_fault()?;
            if self
                .host()
                .ok_or(ScriptSessionError::NoSession)?
                .objects()
                .has_queued_training_events()
                || self
                    .host()
                    .ok_or(ScriptSessionError::NoSession)?
                    .objects()
                    .has_queued_mirror_events()
            {
                let events = match self.advance_queued_events(*budget) {
                    Ok(events) => events,
                    Err(error) => {
                        if let Some(host) = self.host() {
                            host.objects_mut().fail_native_physics(error.to_string());
                        }
                        return Err(error);
                    }
                };
                *budget = budget
                    .checked_sub(events.max(1))
                    .ok_or(TimerSessionError::WakeBudget)?;
                delivered += events;
            }
            if let Some(at) = self
                .host()
                .ok_or(ScriptSessionError::NoSession)?
                .objects()
                .pending_native_physics()
            {
                self.host()
                    .ok_or(ScriptSessionError::NoSession)?
                    .objects_mut()
                    .finish_native_physics(at, activity_work)?;
                continue;
            }
            if self
                .host()
                .ok_or(ScriptSessionError::NoSession)?
                .objects()
                .game_paused()
                == Some(true)
                && !self
                    .host()
                    .ok_or(ScriptSessionError::NoSession)?
                    .objects()
                    .native_frame_active()
            {
                return Ok(delivered);
            }
            let host = self.host().ok_or(ScriptSessionError::NoSession)?;
            let paired = host.objects().native_frame_active();
            let native_ai = if paired {
                let point = host
                    .objects()
                    .native_game_point()
                    .expect("native frame Game point");
                host.next_native_ai_wake(point, activity_work)
                    .map_err(source_queue_error)?
            } else {
                None
            };
            let ai_wake = if paired {
                None
            } else {
                host.next_ai_wake(until)
            };
            let native_limit = paired.then(|| {
                host.objects()
                    .native_game_point()
                    .expect("native frame Game point")
            });
            let native_target = if let Some(limit) = native_limit {
                host.objects()
                    .next_native_timer_boundary(limit, activity_work)
                    .map_err(queue_selection_error)?
                    .into_iter()
                    .chain(native_ai.map(|wake| wake.point))
                    .min_by_key(|point| point.local)
                    .or(Some(limit))
            } else {
                None
            };
            let step_until = if let Some(point) = native_target {
                host.objects().native_game_clock_seconds(point)?
            } else {
                ai_wake.map_or(until, |(at, _, _)| at)
            };
            let activity_wake = self
                .host()
                .ok_or(ScriptSessionError::NoSession)?
                .objects()
                .next_timer_activity_boundary(step_until);
            let mut step_until = activity_wake.unwrap_or(step_until);
            let physics = {
                let objects = self
                    .host()
                    .ok_or(ScriptSessionError::NoSession)?
                    .objects_mut();
                if let Some(outer) = objects.native_game_point() {
                    if let Some(at) = objects.next_native_physics_boundary(outer)? {
                        let seconds = objects.native_physics_seconds(at);
                        if seconds <= step_until {
                            if let Some(target) = native_target {
                                if at.cycle != target.cycle {
                                    return Err(TimerSessionError::NativeQueue(
                                        "physics and selected game boundary cycles differ".into(),
                                    ));
                                }
                                (f32::from_bits(at.local) <= f32::from_bits(target.local))
                                    .then_some(at)
                            } else {
                                let timer =
                                    objects.next_native_timer_boundary(outer, activity_work)?;
                                if let Some(timer) = timer.filter(|timer| {
                                    f32::from_bits(timer.local) < f32::from_bits(at.local)
                                }) {
                                    step_until =
                                        step_until.min(objects.native_physics_seconds(timer));
                                    None
                                } else {
                                    Some(at)
                                }
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            };
            // shortcut: physics wins equal-family deadlines until native cross-family heap order is qualified.
            if let Some(at) = physics {
                *budget = budget.checked_sub(1).ok_or(TimerSessionError::WakeBudget)?;
                self.host()
                    .ok_or(ScriptSessionError::NoSession)?
                    .objects_mut()
                    .begin_native_physics(at, activity_work)?;
                continue;
            }
            let objects = self
                .host()
                .ok_or(ScriptSessionError::NoSession)?
                .objects_mut();
            let wake = if objects.native_frame_active() {
                None
            } else {
                objects.next_thread_wake(step_until)?
            };
            if let Some(ticket) = wake {
                let objects = self
                    .host()
                    .ok_or(ScriptSessionError::NoSession)?
                    .objects_mut();
                let now = objects.timer_game_time().expect("selected timer clock");
                objects.advance_unit_activity_mandatory(now, activity_work)?;
                *budget = budget.checked_sub(1).ok_or(TimerSessionError::WakeBudget)?;
                self.resume_thread(ticket)?;
                delivered += 1;
                continue;
            }
            let objects = self.host().ok_or(ScriptSessionError::NoSession)?.objects();
            // shortcut: an AI deadline needs its native game-queue entry before it can enter the callback scope.
            if objects.native_clock_driving()
                && ai_wake.is_some_and(|(at, _, _)| at == step_until)
                && activity_wake != Some(step_until)
                && objects
                    .next_timer_deadline()
                    .is_none_or(|at| at > step_until)
            {
                return Err(TimerSessionError::Ai(
                    "parked AI wake has no native game-queue entry".into(),
                ));
            }
            let expiry = self
                .host()
                .ok_or(ScriptSessionError::NoSession)?
                .objects_mut()
                .next_timer_expiry(step_until, budget)
                .map_err(|error| {
                    if paired {
                        queue_selection_error(error)
                    } else {
                        error.into()
                    }
                })?;
            let selected_expiry = expiry.is_some();
            if let Some(expiry) = expiry {
                self.active
                    .as_mut()
                    .expect("selected timer session")
                    .timer_delivery = Some(TimerDelivery {
                    expiry,
                    trigger: 0,
                    phase: TimerDeliveryPhase::MandatoryProjection,
                    projection_fault: None,
                });
            }
            let movement = {
                let objects = self
                    .host()
                    .ok_or(ScriptSessionError::NoSession)?
                    .objects_mut();
                let now = objects.timer_game_time().expect("selected timer clock");
                objects.advance_unit_activity_mandatory(now, activity_work)
            };
            if let Err(error) = movement {
                if selected_expiry {
                    let error = Arc::new(error);
                    self.timer_delivery_mut().projection_fault = Some(Arc::clone(&error));
                    return Err(TimerSessionError::MandatoryProjection(error));
                }
                return Err(error.into());
            }
            if !selected_expiry {
                if activity_wake == Some(step_until) {
                    *budget = budget.checked_sub(1).ok_or(TimerSessionError::WakeBudget)?;
                    continue;
                }
                if let Some(wake) = native_ai.filter(|wake| Some(wake.point) == native_target) {
                    *budget = budget.checked_sub(1).ok_or(TimerSessionError::WakeBudget)?;
                    let outcome = self
                        .host()
                        .ok_or(ScriptSessionError::NoSession)?
                        .deliver_native_ai_wake(wake, activity_work)
                        .map_err(TimerSessionError::Ai)?;
                    delivered +=
                        usize::from(outcome == crate::campaign_ai::AiWakeDelivery::Resumed);
                    continue;
                }
                if paired && native_target != native_limit {
                    continue;
                }
                if let Some((_, player, ticket)) = ai_wake {
                    *budget = budget.checked_sub(1).ok_or(TimerSessionError::WakeBudget)?;
                    self.host()
                        .ok_or(ScriptSessionError::NoSession)?
                        .resume_ai(player, ticket)
                        .map_err(TimerSessionError::Ai)?;
                    delivered += 1;
                    continue;
                }
                let objects = self
                    .host()
                    .ok_or(ScriptSessionError::NoSession)?
                    .objects_mut();
                let now = objects.timer_game_time().expect("selected timer clock");
                objects.flush_unit_activity_routes(now, activity_work)?;
                return Ok(delivered);
            };
            self.timer_delivery_mut().phase = TimerDeliveryPhase::Trigger;
            self.deliver_prepared_timer_expiry()?;
            delivered += 1;
        }
    }

    fn deliver_prepared_timer_expiry(&mut self) -> Result<(), TimerSessionError> {
        let result = self.drive_timer_delivery();
        let delivery = self
            .active
            .as_mut()
            .expect("delivery retains its published session")
            .timer_delivery
            .take()
            .expect("prepared timer delivery");
        let objects = self
            .host()
            .ok_or(ScriptSessionError::NoSession)?
            .objects_mut();
        if matches!(
            delivery.phase,
            TimerDeliveryPhase::Conditions { .. } | TimerDeliveryPhase::Actions { .. }
        ) {
            objects.finish_timer_trigger();
        }
        objects.finish_timer_expiry(delivery.expiry)?;
        result
    }

    fn timer_delivery_mut(&mut self) -> &mut TimerDelivery {
        self.active
            .as_mut()
            .expect("delivery retains its published session")
            .timer_delivery
            .as_mut()
            .expect("prepared timer delivery")
    }

    fn drive_timer_delivery(&mut self) -> Result<(), TimerSessionError> {
        loop {
            let active = self.active.as_mut().ok_or(ScriptSessionError::NoSession)?;
            let delivery = active
                .timer_delivery
                .as_ref()
                .expect("prepared timer delivery");
            let trigger = delivery.trigger;
            match delivery.phase {
                TimerDeliveryPhase::MandatoryProjection => {
                    return Err(TimerSessionError::PendingDelivery);
                }
                TimerDeliveryPhase::Trigger => {
                    if trigger == delivery.expiry.triggers().len() {
                        self.timer_delivery_mut().phase = TimerDeliveryPhase::Handler;
                    } else if active
                        .vm
                        .host()
                        .inner
                        .objects_mut()
                        .begin_timer_trigger(&delivery.expiry, trigger)?
                    {
                        self.timer_delivery_mut().phase = TimerDeliveryPhase::Conditions {
                            next: 0,
                            accepted: true,
                        };
                    } else {
                        self.timer_delivery_mut().trigger += 1;
                    }
                }
                TimerDeliveryPhase::Conditions { next, accepted } => {
                    let id = delivery.expiry.triggers()[trigger]
                        .conditions
                        .get(next)
                        .copied();
                    if let Some(id) = id {
                        let ticket = self.callback_ticket(id)?;
                        let value = self.call_callback(&ticket, &[])?;
                        let Value::Bool(value) = value else {
                            return Err(TimerSessionError::ConditionResult { id });
                        };
                        self.timer_delivery_mut().phase = TimerDeliveryPhase::Conditions {
                            next: next + 1,
                            accepted: accepted & value,
                        };
                    } else {
                        self.timer_delivery_mut().phase = TimerDeliveryPhase::Actions { accepted };
                    }
                }
                TimerDeliveryPhase::Actions { accepted } => {
                    if accepted {
                        let actions = &delivery.expiry.triggers()[trigger].actions;
                        for &id in actions {
                            function_name(&active.context, id)?;
                        }
                        let first = actions.first().copied();
                        active.vm.run_thread(actions).map_err(|error| {
                            execution_failure(&active.context, &mut active.vm, first, error)
                        })?;
                    }
                    self.host()
                        .ok_or(ScriptSessionError::NoSession)?
                        .objects_mut()
                        .finish_timer_trigger();
                    let delivery = self.timer_delivery_mut();
                    delivery.trigger += 1;
                    delivery.phase = TimerDeliveryPhase::Trigger;
                }
                TimerDeliveryPhase::Handler => {
                    let callback = active
                        .vm
                        .host()
                        .inner
                        .objects()
                        .current_timer_callback(&delivery.expiry)?;
                    if let Some(id) = callback {
                        let ticket = self.callback_ticket(id)?;
                        self.call_callback(&ticket, &[])?;
                    }
                    self.timer_delivery_mut().phase = TimerDeliveryPhase::Complete;
                }
                TimerDeliveryPhase::Complete => return Ok(()),
            }
        }
    }
}

/// One completed script/tick boundary; registration itself never invokes a filter.
/// Effects before a later callback failure remain, as with timed delivery.
fn drain_unit_ranges(
    context: &Arc<ParsedMapContext>,
    vm: &mut OwnedVm<OwnedLocalizedHost<crate::map_startup_host::MapStartupHost>>,
    mut work: usize,
) -> Result<usize, TimerSessionError> {
    if !vm.host().inner.objects().unit_range_runtime_enabled() {
        return Ok(0);
    }
    let entries = vm.host().inner.objects_mut().poll_unit_ranges(&mut work)?;
    let mut delivered = 0;
    for entry in entries {
        let filter = vm
            .host()
            .inner
            .objects_mut()
            .begin_unit_range_entry(entry)?;
        let result = (|| -> Result<bool, TimerSessionError> {
            if let Some(id) = filter {
                let Value::Bool(accepted) = call_range_callback(context, vm, &mut work, id)? else {
                    return Err(TimerSessionError::ConditionResult { id });
                };
                if !accepted {
                    return Ok(false);
                }
            }
            let Some(trigger) = vm.host().inner.objects_mut().begin_unit_range_trigger()? else {
                return Ok(false);
            };
            let mut conditions_accepted = true;
            for id in trigger.conditions {
                let Value::Bool(accepted) = call_range_callback(context, vm, &mut work, id)? else {
                    return Err(TimerSessionError::ConditionResult { id });
                };
                conditions_accepted &= accepted;
            }
            if !conditions_accepted {
                return Ok(false);
            }
            work = work
                .checked_sub(trigger.actions.len())
                .ok_or(TimerSessionError::RangeBudget)?;
            for &id in &trigger.actions {
                function_name(context, id)?;
            }
            vm.run_thread(&trigger.actions).map_err(|error| {
                execution_failure(context, vm, trigger.actions.first().copied(), error)
            })?;
            Ok(true)
        })();
        vm.host().inner.objects_mut().finish_unit_range_entry();
        delivered += usize::from(result?);
    }
    Ok(delivered)
}

impl MapScriptSession<crate::map_startup_host::MapStartupHost> {
    /// One script boundary: range entries, then region membership and every
    /// queued event, each as a filtered trigger thread.
    pub fn advance_unit_ranges(&mut self, work: usize) -> Result<usize, TimerSessionError> {
        self.check_pending_projection()?;
        let active = self.active.as_mut().ok_or(ScriptSessionError::NoSession)?;
        let delivered = drain_unit_ranges(&active.context, &mut active.vm, work)?;
        let mut work = work;
        active
            .vm
            .host()
            .inner
            .objects_mut()
            .poll_region_events(&mut work)?;
        active
            .vm
            .host()
            .inner
            .objects_mut()
            .poll_player_state_events(&mut work)?;
        active
            .vm
            .host()
            .inner
            .objects_mut()
            .poll_unit_state_events(&mut work)?;
        let watches = active.vm.host().inner.objects().variable_event_watches();
        let samples: Vec<_> = watches
            .iter()
            .map(|(id, name)| (*id, active.vm.global(name)))
            .collect();
        active
            .vm
            .host()
            .inner
            .objects_mut()
            .poll_variable_events(&samples, &mut work)?;
        self.advance_queued_events(work)
            .map(|events| delivered + events)
    }

    /// Deliver already queued UI/script callbacks without polling world geometry or time.
    #[cfg_attr(
        feature = "profile-cpu",
        tracing::instrument(skip_all, target = "pcx_cpu", name = "simulation.events")
    )]
    pub fn advance_queued_events(&mut self, mut work: usize) -> Result<usize, TimerSessionError> {
        self.check_pending_projection()?;
        let active = self.active.as_mut().ok_or(ScriptSessionError::NoSession)?;
        let mut delivered = 0;
        while let Some(fired) = active.vm.host().inner.objects_mut().take_fired_event() {
            let Some(trigger) = active
                .vm
                .host()
                .inner
                .objects_mut()
                .begin_fired_event(&fired)?
            else {
                continue;
            };
            let result = (|| -> Result<bool, TimerSessionError> {
                work = work
                    .checked_sub(trigger.actions.len().max(1))
                    .ok_or(TimerSessionError::RangeBudget)?;
                for id in fired.filter.into_iter().chain(trigger.conditions) {
                    let Value::Bool(accepted) =
                        call_range_callback(&active.context, &mut active.vm, &mut work, id)?
                    else {
                        return Err(TimerSessionError::ConditionResult { id });
                    };
                    if !accepted {
                        return Ok(false);
                    }
                }
                for &id in &trigger.actions {
                    function_name(&active.context, id)?;
                }
                active.vm.run_thread(&trigger.actions).map_err(|error| {
                    execution_failure(
                        &active.context,
                        &mut active.vm,
                        trigger.actions.first().copied(),
                        error,
                    )
                })?;
                Ok(true)
            })();
            active.vm.host().inner.objects_mut().finish_fired_event();
            delivered += usize::from(result?);
        }
        Ok(delivered)
    }
}

fn call_range_callback(
    context: &Arc<ParsedMapContext>,
    vm: &mut OwnedVm<OwnedLocalizedHost<crate::map_startup_host::MapStartupHost>>,
    work: &mut usize,
    id: FunctionId,
) -> Result<Value, TimerSessionError> {
    *work = work.checked_sub(1).ok_or(TimerSessionError::RangeBudget)?;
    let name = function_name(context, id)?;
    vm.call(&name, &[])
        .map_err(|error| execution_failure(context, vm, Some(id), error).into())
}

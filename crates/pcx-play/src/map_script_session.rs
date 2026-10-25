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
        let active = self.active.as_mut().ok_or(ScriptSessionError::NoSession)?;
        let name = function_name(&active.context, id)?;
        active
            .vm
            .call(&name, args)
            .map_err(|error| execution_failure(&active.context, &mut active.vm, Some(id), error))
    }
    /// Start trigger actions as one new thread; a sleep parks it in this VM.
    pub fn run_thread(&mut self, ids: &[FunctionId]) -> Result<(), ScriptSessionError> {
        let active = self.active.as_mut().ok_or(ScriptSessionError::NoSession)?;
        for &id in ids {
            function_name(&active.context, id)?;
        }
        active.vm.run_thread(ids).map_err(|error| {
            execution_failure(&active.context, &mut active.vm, ids.first().copied(), error)
        })
    }
    pub fn resume_thread(&mut self, ticket: u64) -> Result<(), ScriptSessionError> {
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
    #[error("campaign AI: {0}")]
    Ai(String),
    #[error("range callback work budget exhausted")]
    RangeBudget,
    #[error("thread wake budget exhausted")]
    WakeBudget,
    #[error("trigger condition {id:?} did not return boolean")]
    ConditionResult { id: FunctionId },
    #[error(transparent)]
    Session(#[from] ScriptSessionError),
    #[error(transparent)]
    Timer(#[from] crate::playercolor_host::Error),
}

impl MapScriptSession<crate::map_startup_host::MapStartupHost> {
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
        self.host()
            .ok_or(ScriptSessionError::NoSession)?
            .objects()
            .validate_timer_target(until)?;
        let mut delivered = 0;
        let mut activity_work = 65536;
        loop {
            if self
                .host()
                .ok_or(ScriptSessionError::NoSession)?
                .objects()
                .has_queued_training_events()
            {
                let events = self.advance_queued_events(budget)?;
                budget = budget
                    .checked_sub(events.max(1))
                    .ok_or(TimerSessionError::WakeBudget)?;
                delivered += events;
            }
            if self
                .host()
                .ok_or(ScriptSessionError::NoSession)?
                .objects()
                .game_paused()
                == Some(true)
            {
                return Ok(delivered);
            }
            let ai_wake = self
                .host()
                .ok_or(ScriptSessionError::NoSession)?
                .next_ai_wake(until);
            let step_until = ai_wake.map_or(until, |(at, _, _)| at);
            let training_wake = self
                .host()
                .ok_or(ScriptSessionError::NoSession)?
                .objects()
                .next_training_completion(step_until);
            let step_until = training_wake.unwrap_or(step_until);
            let wake = self
                .host()
                .ok_or(ScriptSessionError::NoSession)?
                .objects_mut()
                .next_thread_wake(step_until)?;
            if let Some(ticket) = wake {
                let objects = self
                    .host()
                    .ok_or(ScriptSessionError::NoSession)?
                    .objects_mut();
                let now = objects.timer_game_time().expect("selected timer clock");
                objects.advance_unit_activity(now, &mut activity_work)?;
                budget = budget.checked_sub(1).ok_or(TimerSessionError::WakeBudget)?;
                self.resume_thread(ticket)?;
                delivered += 1;
                continue;
            }
            let expiry = self
                .host()
                .ok_or(ScriptSessionError::NoSession)?
                .objects_mut()
                .next_timer_expiry(step_until, &mut budget)?;
            let movement = {
                let objects = self
                    .host()
                    .ok_or(ScriptSessionError::NoSession)?
                    .objects_mut();
                let now = objects.timer_game_time().expect("selected timer clock");
                objects.advance_unit_activity(now, &mut activity_work)
            };
            if let Err(error) = movement {
                if let Some(expiry) = expiry {
                    self.host()
                        .ok_or(ScriptSessionError::NoSession)?
                        .objects_mut()
                        .finish_timer_expiry(expiry)?;
                }
                return Err(error.into());
            }
            let Some(expiry) = expiry else {
                if training_wake == Some(step_until) {
                    budget = budget.checked_sub(1).ok_or(TimerSessionError::WakeBudget)?;
                    continue;
                }
                if let Some((_, player, ticket)) = ai_wake {
                    budget = budget.checked_sub(1).ok_or(TimerSessionError::WakeBudget)?;
                    self.host()
                        .ok_or(ScriptSessionError::NoSession)?
                        .resume_ai(player, ticket)
                        .map_err(TimerSessionError::Ai)?;
                    delivered += 1;
                    continue;
                }
                return Ok(delivered);
            };
            let result = (|| -> Result<(), TimerSessionError> {
                for (index, trigger) in expiry.triggers().iter().enumerate() {
                    if !self
                        .host()
                        .ok_or(ScriptSessionError::NoSession)?
                        .objects_mut()
                        .begin_timer_trigger(&expiry, index)?
                    {
                        continue;
                    }
                    let result = (|| -> Result<(), TimerSessionError> {
                        let mut accepted = true;
                        for &id in &trigger.conditions {
                            let ticket = self.callback_ticket(id)?;
                            let value = self.call_callback(&ticket, &[])?;
                            let Value::Bool(value) = value else {
                                return Err(TimerSessionError::ConditionResult { id });
                            };
                            accepted &= value;
                        }
                        if accepted {
                            self.run_thread(&trigger.actions)?;
                        }
                        Ok(())
                    })();
                    self.host()
                        .ok_or(ScriptSessionError::NoSession)?
                        .objects_mut()
                        .finish_timer_trigger();
                    result?;
                }
                if let Some(id) = expiry.callback {
                    let ticket = self.callback_ticket(id)?;
                    self.call_callback(&ticket, &[])?;
                }
                Ok(())
            })();
            self.host()
                .ok_or(ScriptSessionError::NoSession)?
                .objects_mut()
                .finish_timer_expiry(expiry)?;
            result?;
            delivered += 1;
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
        self.advance_queued_events(work)
            .map(|events| delivered + events)
    }

    /// Deliver already queued UI/script callbacks without polling world geometry or time.
    #[cfg_attr(
        feature = "profile-cpu",
        tracing::instrument(skip_all, target = "pcx_cpu", name = "simulation.events")
    )]
    pub fn advance_queued_events(&mut self, mut work: usize) -> Result<usize, TimerSessionError> {
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

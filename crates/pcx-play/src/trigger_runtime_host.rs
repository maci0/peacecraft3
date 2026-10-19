//! Non-yielding timer-trigger delivery on the existing object arena and clock.
use super::{Entry, Error, Failure, NativeCallback, ObjectHost, timers};
use pcx_script::{HandleId, Value, ast::Expr, value::FunctionId};

/// Caller-qualified defaults; no captured engine value is a production fallback.
#[derive(Clone, Debug)]
pub struct TriggerRuntimeInput {
    pub initial_enabled: bool,
    /// Constant `gameevent` global from the selected ConvertGameEvent source.
    pub timer_event_global: String,
    pub max_timed_events: usize,
}

pub(super) struct State {
    pub(super) initial_enabled: bool,
    event: HandleId,
    limit: usize,
    retained: usize,
    active: Option<HandleId>,
    timer_context: bool,
}

pub(super) struct DirectCall {
    previous: Option<HandleId>,
    callbacks: Vec<FunctionId>,
    next: usize,
    evaluate: bool,
    accepted: bool,
}

/// Callback snapshot from one arena. The session executes it in its existing VM.
/// shortcut: callback-list edits apply on the next firing; qualify mid-firing edits before extending this policy.
#[derive(Debug)]
pub struct TriggerDelivery {
    pub trigger: HandleId,
    pub conditions: Vec<FunctionId>,
    pub actions: Vec<FunctionId>,
}

impl ObjectHost {
    pub fn with_trigger_runtime(mut self, input: TriggerRuntimeInput) -> Result<Self, Error> {
        let name = "TriggerRegisterTimerEvent";
        let fail = |why| self.inner.error(name, why);
        if self.trigger_runtime.is_some() || self.timers.is_none() || self.triggers.is_none() {
            return Err(fail(Failure::TriggerState(
                "requires trigger admission and timer runtime, once",
            )));
        }
        if self
            .inner
            .arena
            .iter()
            .any(|entry| matches!(entry, Entry::Trigger { .. } | Entry::DeadTrigger))
        {
            return Err(fail(Failure::TriggerState(
                "admit runtime before creating triggers",
            )));
        }
        std::alloc::Layout::array::<HandleId>(input.max_timed_events)
            .map_err(|_| fail(Failure::Capacity))?;
        let source = &self
            .inner
            .program
            .natives()
            .get("CreateTrigger")
            .ok_or_else(|| fail(Failure::Signature))?
            .source;
        for (native, args, result, constant) in [
            (name, &["trigger", "real", "boolean"][..], "event", false),
            ("DestroyTrigger", &["trigger"][..], "nothing", false),
            ("GetTriggeringTrigger", &[][..], "trigger", true),
            ("GetTriggerEventId", &[][..], "eventid", true),
            ("ConvertGameEvent", &["integer"][..], "gameevent", true),
        ] {
            Self::contract_in(&self.inner, native, args, result, constant, None, false)?;
            if &self
                .inner
                .program
                .natives()
                .get(native)
                .expect("checked contract")
                .source
                != source
            {
                return Err(fail(Failure::Source));
            }
        }
        for native in ["TimerStart", "TriggerRegisterTimerExpireEvent"] {
            if self
                .inner
                .program
                .natives()
                .get(native)
                .is_some_and(|n| &n.source != source)
            {
                return Err(fail(Failure::Source));
            }
        }
        for (native, result) in [
            ("TriggerEvaluate", "boolean"),
            ("TriggerExecute", "nothing"),
        ] {
            if self.inner.program.natives().get(native).is_some() {
                Self::contract_in(
                    &self.inner,
                    native,
                    &["trigger"],
                    result,
                    false,
                    None,
                    false,
                )?;
                if self
                    .inner
                    .program
                    .natives()
                    .get(native)
                    .expect("checked contract")
                    .source
                    != *source
                {
                    return Err(fail(Failure::Source));
                }
            }
        }
        let ty = self
            .inner
            .program
            .types()
            .find(|ty| ty.name == "gameevent")
            .ok_or_else(|| fail(Failure::Signature))?;
        if &ty.source != source || ty.parent != "eventid" {
            return Err(fail(Failure::Source));
        }
        let global = self
            .inner
            .program
            .globals()
            .iter()
            .find(|g| g.name == input.timer_event_global)
            .ok_or_else(|| fail(Failure::Initializer(input.timer_event_global.clone())))?;
        if !global.is_constant
            || global.array
            || global.ty.array
            || global.ty.kind.name() != "gameevent"
            || &global.source != source
        {
            return Err(fail(Failure::Initializer(input.timer_event_global)));
        }
        let Some(Expr::Call { callee, args, .. }) = &global.init else {
            return Err(fail(Failure::Initializer(input.timer_event_global)));
        };
        if !matches!(callee.as_ref(), Expr::Ident(name, _) if name.as_str() == "ConvertGameEvent")
            || args.len() != 1
        {
            return Err(fail(Failure::Initializer(input.timer_event_global)));
        }
        // Same bounded expression evaluator used by the selected enum admission.
        let mut work = 4096;
        let value = super::super::integer(&args[0], 64, &mut work).map_err(fail)?;
        let Value::Handle(event) = self
            .inner
            .convert_for("ConvertGameEvent", &[Value::Int(value)])?
        else {
            unreachable!("enum conversion returns a handle")
        };
        self.trigger_runtime = Some(State {
            initial_enabled: input.initial_enabled,
            event,
            limit: input.max_timed_events,
            retained: 0,
            active: None,
            timer_context: false,
        });
        Ok(self)
    }

    pub(super) fn trigger_runtime_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        let state = self.trigger_runtime.as_ref()?;
        match name {
            "TriggerRegisterTimerEvent" => Some(self.register_timed_event(args)),
            "DestroyTrigger" => Some((|| {
                let [Value::Handle(trigger)] = args else {
                    return Err(self
                        .inner
                        .error(name, Failure::TriggerState("expected one trigger")));
                };
                self.trigger_enabled(*trigger)
                    .map_err(|e| self.inner.error(name, e.failure))?;
                *self.inner.entry_mut(*trigger)? = Entry::DeadTrigger;
                Ok(Value::Null)
            })()),
            "GetTriggeringTrigger" | "GetTriggerEventId" => Some(if args.is_empty() {
                Ok(match state.active {
                    None => Value::Null,
                    Some(trigger) if name == "GetTriggeringTrigger" => Value::Handle(trigger),
                    Some(_) if state.timer_context => Value::Handle(state.event),
                    Some(_) => Value::Null,
                })
            } else {
                Err(self
                    .inner
                    .error(name, Failure::TriggerState("expected no arguments")))
            }),
            _ => None,
        }
    }

    fn register_timed_event(&mut self, args: &[Value]) -> Result<Value, Error> {
        let name = "TriggerRegisterTimerEvent";
        let fail = |why| self.inner.error(name, why);
        let [Value::Handle(trigger), timeout, Value::Bool(periodic)] = args else {
            return Err(fail(Failure::TimerArguments));
        };
        self.trigger_enabled(*trigger)
            .map_err(|e| self.inner.error(name, e.failure))?;
        let timeout = match timeout {
            Value::Real(n) => f64::from(*n),
            Value::Int(n) => f64::from(*n as f32),
            _ => return Err(fail(Failure::TimerArguments)),
        };
        let state = self.trigger_runtime.as_ref().expect("admitted");
        if state.retained >= state.limit {
            return Err(fail(Failure::Budget("timed trigger registration")));
        }
        let run = self.new_timer_run(timeout, *periodic, None, name)?;
        let id = self.inner.allocate(
            Entry::RegisteredTimedEvent {
                trigger: *trigger,
                run,
            },
            name,
        )?;
        self.commit_timer_sequence(&run);
        self.trigger_runtime.as_mut().expect("admitted").retained += 1;
        Ok(Value::Handle(id))
    }

    pub(super) fn prepare_timer_triggers(
        &self,
        source: HandleId,
        timed: bool,
    ) -> Result<Vec<TriggerDelivery>, Error> {
        let name = "timer delivery";
        let mut owners = Vec::new();
        if !timed {
            for entry in &self.inner.arena {
                if let Entry::RegisteredTimerEvent { trigger, timer } = entry
                    && *timer == source
                {
                    owners
                        .try_reserve(1)
                        .map_err(|_| self.inner.error(name, Failure::Capacity))?;
                    owners.push(*trigger);
                }
            }
        }
        if timed && let Entry::RegisteredTimedEvent { trigger, .. } = self.inner.entry(source)? {
            owners
                .try_reserve(1)
                .map_err(|_| self.inner.error(name, Failure::Capacity))?;
            owners.push(*trigger);
        }
        if !owners.is_empty() && self.trigger_runtime.is_none() {
            return Err(self.inner.error(
                name,
                Failure::TimerState("expiry-trigger execution is unsupported"),
            ));
        }
        let mut deliveries = Vec::new();
        for trigger in owners {
            if matches!(self.inner.entry(trigger)?, Entry::DeadTrigger)
                || self.trigger_enabled(trigger)? == Some(false)
            {
                continue;
            }
            let mut conditions = Vec::new();
            let mut actions = Vec::new();
            for entry in &self.inner.arena {
                let (list, function) = match entry {
                    Entry::TriggerCondition(r) if r.trigger == trigger => {
                        let function = match self.inner.entry(r.condition)? {
                            Entry::Condition { .. } => {
                                self.condition_callback(r.condition)?.function
                            }
                            Entry::Filter { .. } => self.filter_callback(r.condition)?.function,
                            _ => return Err(self.inner.error(name, Failure::WrongFamily)),
                        };
                        (&mut conditions, function)
                    }
                    Entry::TriggerAction {
                        trigger: owner,
                        function,
                    } if *owner == trigger => (&mut actions, *function),
                    _ => continue,
                };
                list.try_reserve(1)
                    .map_err(|_| self.inner.error(name, Failure::Capacity))?;
                list.push(function);
            }
            deliveries
                .try_reserve(1)
                .map_err(|_| self.inner.error(name, Failure::Capacity))?;
            deliveries.push(TriggerDelivery {
                trigger,
                conditions,
                actions,
            });
        }
        Ok(deliveries)
    }

    pub fn begin_timer_trigger(
        &mut self,
        expiry: &timers::TimerExpiry,
        index: usize,
    ) -> Result<bool, Error> {
        self.check_timer_expiry(expiry)?;
        let record = expiry
            .triggers()
            .get(index)
            .ok_or_else(|| self.inner.error("timer delivery", Failure::Identity))?;
        if matches!(self.inner.entry(record.trigger)?, Entry::DeadTrigger)
            || self.trigger_enabled(record.trigger)? == Some(false)
        {
            return Ok(false);
        }
        let state = self
            .trigger_runtime
            .as_mut()
            .expect("preflight requires runtime");
        if state.active.is_some() {
            return Err(self.inner.error(
                "timer delivery",
                Failure::TriggerState("unfinished trigger"),
            ));
        }
        state.active = Some(record.trigger);
        state.timer_context = true;
        Ok(true)
    }

    pub fn finish_timer_trigger(&mut self) {
        if let Some(state) = &mut self.trigger_runtime {
            state.active = None;
            state.timer_context = false;
        }
    }

    pub(super) fn begin_trigger_call(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Result<Option<pcx_script::ScriptCall>, Error> {
        if !matches!(name, "TriggerEvaluate" | "TriggerExecute") || self.trigger_runtime.is_none() {
            return Ok(None);
        }
        if self.inner.program.natives().get(name).is_none() {
            return Err(self.inner.error(name, Failure::Signature));
        }
        let [Value::Handle(trigger)] = args else {
            return Err(self
                .inner
                .error(name, Failure::TriggerState("expected one trigger")));
        };
        // Explicit calls validate identity but do not depend on automatic-event enablement.
        self.trigger_enabled(*trigger)?;
        let evaluate = name == "TriggerEvaluate";
        // shortcut: list edits become visible on the next call; qualify mid-call edits before extending this policy.
        let mut callbacks = Vec::new();
        for entry in &self.inner.arena {
            let function = match entry {
                Entry::TriggerCondition(record) if evaluate && record.trigger == *trigger => {
                    match self.inner.entry(record.condition)? {
                        Entry::Condition { .. } => {
                            self.condition_callback(record.condition)?.function
                        }
                        Entry::Filter { .. } => self.filter_callback(record.condition)?.function,
                        _ => return Err(self.inner.error(name, Failure::WrongFamily)),
                    }
                }
                Entry::TriggerAction {
                    trigger: owner,
                    function,
                } if !evaluate && *owner == *trigger => *function,
                _ => continue,
            };
            callbacks
                .try_reserve(1)
                .map_err(|_| self.inner.error(name, Failure::Capacity))?;
            callbacks.push(function);
        }
        let state = self.trigger_runtime.as_mut().expect("checked runtime");
        if callbacks.is_empty() {
            return Ok(Some(pcx_script::ScriptCall::Return(if evaluate {
                Value::Bool(true)
            } else {
                Value::Null
            })));
        }
        if self.callbacks.len() >= pcx_script::eval::MAX_CALL_DEPTH as usize {
            return Err(self
                .inner
                .error(name, Failure::Budget("nested trigger callbacks")));
        }
        self.callbacks
            .try_reserve(1)
            .map_err(|_| self.inner.error(name, Failure::Capacity))?;
        let first = callbacks[0];
        self.callbacks.push(NativeCallback::Trigger(DirectCall {
            previous: state.active,
            callbacks,
            next: 1,
            evaluate,
            accepted: true,
        }));
        state.active = Some(*trigger);
        Ok(Some(pcx_script::ScriptCall::Call(first)))
    }

    pub(super) fn continue_trigger_call(
        &mut self,
        value: Value,
    ) -> Result<pcx_script::ScriptCall, Error> {
        let name = "trigger callback";
        let state = self
            .trigger_runtime
            .as_mut()
            .ok_or_else(|| self.inner.error(name, Failure::TriggerState("no runtime")))?;
        let Some(NativeCallback::Trigger(frame)) = self.callbacks.last_mut() else {
            return Err(self
                .inner
                .error(name, Failure::TriggerState("no active call")));
        };
        if frame.evaluate {
            let Value::Bool(accepted) = value else {
                return Err(self.inner.error(
                    name,
                    Failure::TriggerState("condition did not return boolean"),
                ));
            };
            frame.accepted &= accepted;
        }
        if let Some(&next) = frame.callbacks.get(frame.next) {
            frame.next += 1;
            return Ok(pcx_script::ScriptCall::Call(next));
        }
        let Some(NativeCallback::Trigger(frame)) = self.callbacks.pop() else {
            unreachable!("active trigger call");
        };
        state.active = frame.previous;
        Ok(pcx_script::ScriptCall::Return(if frame.evaluate {
            Value::Bool(frame.accepted)
        } else {
            Value::Null
        }))
    }

    pub(super) fn abort_native_callbacks(&mut self) {
        while let Some(frame) = self.callbacks.pop() {
            if let NativeCallback::Trigger(frame) = frame
                && let Some(state) = self.trigger_runtime.as_mut()
            {
                state.active = frame.previous;
            }
        }
    }
}

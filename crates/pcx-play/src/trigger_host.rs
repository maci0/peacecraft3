//! Shared-arena trigger creation, event subscriptions and action registration.
//!
//! Enabled state needs an explicit runtime input or an enable/disable request.
//! Timed delivery is separately admitted; other event families remain registrations.
//! `TriggerAddAction` retains the loaded callback and does not run it.
use super::super::{Entry, Error, Failure, below};
use super::ObjectHost;
use pcx_script::resolve::Program;
use pcx_script::span::Span;
use pcx_script::value::FunctionId;
use pcx_script::{HandleId, Value};

#[path = "timer_event_host.rs"]
mod timer_events;
pub use timer_events::TimerEventRegistration;
#[path = "game_state_event_host.rs"]
mod game_state_events;
pub use game_state_events::GameStateEventRegistration;

#[path = "player_state_event_host.rs"]
mod player_state_events;
pub(super) use player_state_events::PlayerStateChange;
pub use player_state_events::PlayerStateEventRegistration;

#[path = "unit_state_event_host.rs"]
mod unit_state_events;
pub use unit_state_events::UnitStateEventRegistration;

#[path = "player_unit_event_host.rs"]
mod player_unit_events;
pub use player_unit_events::PlayerUnitEventRegistration;

#[path = "player_event_host.rs"]
mod player_events;
pub use player_events::PlayerEventRegistration;
#[path = "player_chat_event_host.rs"]
mod player_chat_events;
pub use player_chat_events::PlayerChatRegistration;
#[path = "game_event_host.rs"]
mod game_events;

#[path = "trigger_condition_host.rs"]
mod trigger_conditions;
pub use trigger_conditions::TriggerConditionRegistration;

#[path = "death_event_host.rs"]
mod death_events;
pub use death_events::DeathEventRegistration;

#[path = "range_event_host.rs"]
mod range_events;
pub use range_events::UnitInRangeRegistration;

const CREATE: &str = "CreateTrigger";
const REGISTER: &str = "TriggerRegisterUnitEvent";
const CONVERT: &str = "ConvertUnitEvent";
const ADD: &str = "TriggerAddAction";
const ENABLE: &str = "EnableTrigger";
const DISABLE: &str = "DisableTrigger";
const IS_ENABLED: &str = "IsTriggerEnabled";

/// Caller bounds for retained identities. Not a firing or population limit.
#[derive(Clone, Copy, Debug)]
pub struct TriggerBudget {
    /// Retained `CreateTrigger` identities.
    pub max_triggers: usize,
    /// Retained `TriggerRegisterUnitEvent` event identities.
    pub max_unit_events: usize,
    /// Retained `TriggerAddAction` identities.
    pub max_actions: usize,
}

/// Callback retained for later delivery. `function`, `source` and `span` are the
/// loaded declaration. `trigger` is the registration's owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TriggerActionRecord<'a> {
    pub function: FunctionId,
    pub source: &'a str,
    pub span: Span,
    pub trigger: HandleId,
}

/// Identities retained from one successful registration. `enumerand` is the
/// loaded symbolic `unitevent` value, not a guessed event number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnitEventRegistration {
    pub trigger: HandleId,
    pub unit: HandleId,
    pub unitevent: HandleId,
    pub enumerand: i32,
}

pub(super) struct Admission {
    max_triggers: usize,
    max_unit_events: usize,
    max_actions: usize,
    retained_triggers: usize,
    retained_events: usize,
    retained_actions: usize,
    condition_limit: Option<usize>,
    retained_conditions: usize,
    player_unit_event_limit: Option<usize>,
    retained_player_unit_events: usize,
    death_event_limit: Option<usize>,
    retained_death_events: usize,
    death_event: Option<HandleId>,
    pub(super) death_event_source: Option<String>,
    unit_in_range_limit: Option<usize>,
    retained_unit_in_range: usize,
    player_event_limit: Option<usize>,
    retained_player_events: usize,
    player_chat_event_limit: Option<usize>,
    retained_player_chat_events: usize,
    game_events: bool,
    timer_event_limit: Option<usize>,
    retained_timer_events: usize,
    game_state_event_limit: Option<usize>,
    retained_game_state_events: usize,
    player_state_event_limit: Option<usize>,
    retained_player_state_events: usize,
    player_state_ops: Option<[i32; 6]>,
    player_state_event: Option<HandleId>,
    player_state_registrations: Vec<HandleId>,
    unit_state: Option<unit_state_events::Admission>,
    variable_ops: Option<[i32; 6]>,
    variable_registrations: Vec<HandleId>,
}

impl ObjectHost {
    /// Admit the loaded `CreateTrigger` and `TriggerRegisterUnitEvent` contracts.
    ///
    /// Both natives, `ConvertUnitEvent`, and the `trigger`, `event`, `eventid`,
    /// `unitevent`, and `unit` types must share one declaration source. Programs
    /// that never call this keep those natives unsupported. Present enable/disable
    /// and enabled-query declarations must also match that source and signature.
    pub fn with_trigger_admission(mut self, budget: TriggerBudget) -> Result<Self, Error> {
        let fail = |reason| self.inner.error(CREATE, reason);
        if self.triggers.is_some() {
            return Err(fail(Failure::TriggerState("already admitted")));
        }
        std::alloc::Layout::array::<HandleId>(budget.max_triggers)
            .map_err(|_| fail(Failure::Capacity))?;
        std::alloc::Layout::array::<UnitEventRegistration>(budget.max_unit_events)
            .map_err(|_| fail(Failure::Capacity))?;
        std::alloc::Layout::array::<(HandleId, FunctionId)>(budget.max_actions)
            .map_err(|_| fail(Failure::Capacity))?;
        Self::contract(&self.inner, CREATE, &[], "trigger", false, Some("trigger"))?;
        Self::contract(
            &self.inner,
            REGISTER,
            &["trigger", "unit", "unitevent"],
            "event",
            false,
            Some("event"),
        )?;
        Self::contract_in(
            &self.inner,
            CONVERT,
            &["integer"],
            "unitevent",
            true,
            None,
            true,
        )?;
        let create = self
            .inner
            .program
            .natives()
            .get(CREATE)
            .ok_or_else(|| fail(Failure::Signature))?;
        let source = create.source.clone();
        let register = self
            .inner
            .program
            .natives()
            .get(REGISTER)
            .ok_or_else(|| self.inner.error(REGISTER, Failure::Signature))?;
        if register.source != source {
            return Err(self.inner.error(REGISTER, Failure::Source));
        }
        let convert = self
            .inner
            .program
            .natives()
            .get(CONVERT)
            .ok_or_else(|| self.inner.error(CONVERT, Failure::Signature))?;
        if convert.source != source {
            return Err(self.inner.error(CONVERT, Failure::Source));
        }
        if self
            .inner
            .conversions
            .get(CONVERT)
            .is_none_or(|conversion| conversion.source != source || conversion.kind != "unitevent")
        {
            return Err(self.inner.error(CONVERT, Failure::Signature));
        }
        for (native, type_name, parent) in [
            (CREATE, "trigger", "agent"),
            (REGISTER, "event", "agent"),
            (REGISTER, "eventid", "handle"),
            (REGISTER, "unitevent", "eventid"),
            (CREATE, "unit", "widget"),
        ] {
            loaded_type(&self.inner, native, type_name, parent, &source)?;
        }
        if !below(&self.inner.program, "trigger", "agent")
            || !below(&self.inner.program, "event", "agent")
            || !below(&self.inner.program, "unit", "widget")
            || !below(&self.inner.program, "unitevent", "handle")
        {
            return Err(fail(Failure::Signature));
        }
        if let Some(add) = self.inner.program.natives().get(ADD) {
            Self::contract_in(
                &self.inner,
                ADD,
                &["trigger", "code"],
                "triggeraction",
                false,
                Some("triggeraction"),
                false,
            )?;
            if add.source != source {
                return Err(self.inner.error(ADD, Failure::Source));
            }
            loaded_type(&self.inner, ADD, "triggeraction", "handle", &source)?;
        }
        for (name, result) in [
            (ENABLE, "nothing"),
            (DISABLE, "nothing"),
            (IS_ENABLED, "boolean"),
        ] {
            if let Some(native) = self.inner.program.natives().get(name) {
                Self::contract(&self.inner, name, &["trigger"], result, false, None)?;
                if native.source != source {
                    return Err(self.inner.error(name, Failure::Source));
                }
            }
        }
        self.triggers = Some(Admission {
            max_triggers: budget.max_triggers,
            max_unit_events: budget.max_unit_events,
            max_actions: budget.max_actions,
            retained_triggers: 0,
            retained_events: 0,
            retained_actions: 0,
            condition_limit: None,
            retained_conditions: 0,
            player_unit_event_limit: None,
            retained_player_unit_events: 0,
            death_event_limit: None,
            retained_death_events: 0,
            death_event: None,
            death_event_source: None,
            unit_in_range_limit: None,
            retained_unit_in_range: 0,
            player_event_limit: None,
            retained_player_events: 0,
            player_chat_event_limit: None,
            retained_player_chat_events: 0,
            game_events: false,
            timer_event_limit: None,
            retained_timer_events: 0,
            game_state_event_limit: None,
            retained_game_state_events: 0,
            player_state_event_limit: None,
            retained_player_state_events: 0,
            player_state_ops: None,
            player_state_event: None,
            player_state_registrations: Vec::new(),
            unit_state: None,
            variable_ops: None,
            variable_registrations: Vec::new(),
        });
        Ok(self)
    }

    /// Retained registration for this event handle.
    pub fn unit_event_registration(&self, id: HandleId) -> Result<UnitEventRegistration, Error> {
        let _ = self.admitted_triggers()?;
        match self
            .inner
            .entry(id)
            .map_err(|error| self.inner.error(REGISTER, error.failure))?
        {
            Entry::RegisteredUnitEvent {
                trigger,
                unit,
                unitevent,
                enumerand,
            } => Ok(UnitEventRegistration {
                trigger: *trigger,
                unit: *unit,
                unitevent: *unitevent,
                enumerand: *enumerand,
            }),
            _ => Err(self.inner.error(REGISTER, Failure::WrongFamily)),
        }
    }

    /// Trigger identities issued. Tokens are not recycled.
    pub fn retained_triggers(&self) -> Result<usize, Error> {
        Ok(self.admitted_triggers()?.retained_triggers)
    }

    /// Event identities issued by `TriggerRegisterUnitEvent`. Tokens are not recycled.
    pub fn retained_unit_events(&self) -> Result<usize, Error> {
        Ok(self.admitted_triggers()?.retained_events)
    }

    pub(super) fn trigger_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        self.triggers.as_ref()?;
        match name {
            CREATE => Some(self.create_trigger(args)),
            REGISTER => Some(self.register_unit_event(args)),
            game_events::REGISTER if self.triggers.as_ref()?.game_events => {
                Some(self.register_game_event(args))
            }
            death_events::REGISTER if self.triggers.as_ref()?.death_event_limit.is_some() => {
                Some(self.register_death_event(args))
            }
            range_events::REGISTER if self.triggers.as_ref()?.unit_in_range_limit.is_some() => {
                Some(self.register_unit_in_range(args))
            }
            player_events::REGISTER if self.triggers.as_ref()?.player_event_limit.is_some() => {
                Some(self.register_player_event(args))
            }
            player_chat_events::REGISTER
                if self.triggers.as_ref()?.player_chat_event_limit.is_some() =>
            {
                Some(self.register_player_chat_event(args))
            }
            player_unit_events::REGISTER
                if self.triggers.as_ref()?.player_unit_event_limit.is_some() =>
            {
                Some(self.register_player_unit_event(args))
            }
            timer_events::REGISTER if self.triggers.as_ref()?.timer_event_limit.is_some() => {
                Some(self.register_timer_event(args))
            }
            game_state_events::REGISTER
                if self.triggers.as_ref()?.game_state_event_limit.is_some() =>
            {
                Some(self.register_game_state_event(args))
            }
            player_state_events::REGISTER
                if self.triggers.as_ref()?.player_state_event_limit.is_some() =>
            {
                Some(self.register_player_state_event(args))
            }
            unit_state_events::REGISTER if self.triggers.as_ref()?.unit_state.is_some() => {
                Some(self.register_unit_state_event(args))
            }
            trigger_conditions::ADD if self.triggers.as_ref()?.condition_limit.is_some() => {
                Some(self.add_trigger_condition(args))
            }
            ADD if self.inner.program.natives().get(ADD).is_some() => Some(self.add_action(args)),
            ENABLE | DISABLE | IS_ENABLED if self.inner.program.natives().get(name).is_some() => {
                Some(self.trigger_state_native(name, args))
            }
            _ => None,
        }
    }

    fn admitted_triggers(&self) -> Result<&Admission, Error> {
        self.triggers.as_ref().ok_or_else(|| {
            self.inner
                .error(CREATE, Failure::TriggerState("not admitted"))
        })
    }

    fn create_trigger(&mut self, args: &[Value]) -> Result<Value, Error> {
        let fail = |reason| self.inner.error(CREATE, reason);
        if !args.is_empty() {
            return Err(fail(Failure::TriggerState(
                "arguments do not match the trigger native declaration",
            )));
        }
        let (retained, max) = {
            let admitted = self.admitted_triggers()?;
            (admitted.retained_triggers, admitted.max_triggers)
        };
        if retained >= max {
            return Err(fail(Failure::Budget("trigger creation")));
        }
        if self.inner.handles() >= self.inner.max_handles {
            return Err(fail(Failure::Capacity));
        }
        let id = self.inner.allocate(
            Entry::Trigger {
                enabled: self.trigger_runtime.as_ref().map(|s| s.initial_enabled),
            },
            CREATE,
        )?;
        let admitted = self.triggers.as_mut().ok_or_else(|| {
            self.inner
                .error(CREATE, Failure::TriggerState("not admitted"))
        })?;
        admitted.retained_triggers += 1;
        Ok(Value::Handle(id))
    }

    fn register_unit_event(&mut self, args: &[Value]) -> Result<Value, Error> {
        let (trigger, unit, unitevent, enumerand) = {
            let fail = |reason| self.inner.error(REGISTER, reason);
            let [
                Value::Handle(trigger),
                Value::Handle(unit),
                Value::Handle(unitevent),
            ] = args
            else {
                return Err(fail(Failure::TriggerState(
                    "arguments do not match the trigger native declaration",
                )));
            };
            match self
                .inner
                .entry(*trigger)
                .map_err(|error| self.inner.error(REGISTER, error.failure))?
            {
                Entry::Trigger { .. } => {}
                _ => return Err(fail(Failure::WrongFamily)),
            }
            match self
                .inner
                .entry(*unit)
                .map_err(|error| self.inner.error(REGISTER, error.failure))?
            {
                Entry::Unit { .. } => {}
                _ => return Err(fail(Failure::WrongFamily)),
            }
            let enumerand = self
                .inner
                .symbolic_value(*unitevent, "unitevent")
                .map_err(|error| self.inner.error(REGISTER, error.failure))?;
            let conversion = self
                .inner
                .conversions
                .get(CONVERT)
                .ok_or_else(|| fail(Failure::Signature))?;
            if !conversion.domain.contains(&enumerand) {
                return Err(fail(Failure::OutsideDomain(enumerand)));
            }
            let (retained, max) = {
                let admitted = self.admitted_triggers()?;
                (admitted.retained_events, admitted.max_unit_events)
            };
            if retained >= max {
                return Err(fail(Failure::Budget("unit event registration")));
            }
            if self.inner.handles() >= self.inner.max_handles {
                return Err(fail(Failure::Capacity));
            }
            (*trigger, *unit, *unitevent, enumerand)
        };
        let id = self.inner.allocate(
            Entry::RegisteredUnitEvent {
                trigger,
                unit,
                unitevent,
                enumerand,
            },
            REGISTER,
        )?;
        let admitted = self.triggers.as_mut().ok_or_else(|| {
            self.inner
                .error(REGISTER, Failure::TriggerState("not admitted"))
        })?;
        admitted.retained_events += 1;
        Ok(Value::Handle(id))
    }

    /// Loaded `trigger, code -> triggeraction` registration. The callback body
    /// is not entered.
    pub fn trigger_action(&self, id: HandleId) -> Result<TriggerActionRecord<'_>, Error> {
        let _ = self.admitted_triggers()?;
        let (trigger, function_id) = match self
            .inner
            .entry(id)
            .map_err(|error| self.inner.error(ADD, error.failure))?
        {
            Entry::TriggerAction { trigger, function } => (*trigger, *function),
            _ => return Err(self.inner.error(ADD, Failure::WrongFamily)),
        };
        let function = callback(&self.inner.program, function_id)
            .map_err(|failure| self.inner.error(ADD, failure))?;
        Ok(TriggerActionRecord {
            function: function_id,
            source: &function.source,
            span: function.span,
            trigger,
        })
    }

    fn add_action(&mut self, args: &[Value]) -> Result<Value, Error> {
        let (trigger, function_id) = {
            let fail = |reason| self.inner.error(ADD, reason);
            let [Value::Handle(trigger), Value::Code(function_id)] = args else {
                return Err(fail(Failure::TriggerState(
                    "arguments do not match the trigger native declaration",
                )));
            };
            match self
                .inner
                .entry(*trigger)
                .map_err(|error| self.inner.error(ADD, error.failure))?
            {
                Entry::Trigger { .. } => {}
                _ => return Err(fail(Failure::WrongFamily)),
            }
            callback(&self.inner.program, *function_id).map_err(fail)?;
            let (retained, max) = {
                let admitted = self.admitted_triggers()?;
                (admitted.retained_actions, admitted.max_actions)
            };
            if retained >= max {
                return Err(fail(Failure::Budget("trigger action")));
            }
            if self.inner.handles() >= self.inner.max_handles {
                return Err(fail(Failure::Capacity));
            }
            (*trigger, *function_id)
        };
        let id = self.inner.allocate(
            Entry::TriggerAction {
                trigger,
                function: function_id,
            },
            ADD,
        )?;
        let admitted = self
            .triggers
            .as_mut()
            .ok_or_else(|| self.inner.error(ADD, Failure::TriggerState("not admitted")))?;
        admitted.retained_actions += 1;
        Ok(Value::Handle(id))
    }

    /// Assigned state, or the explicit runtime input supplied before creation.
    pub fn trigger_enabled(&self, id: HandleId) -> Result<Option<bool>, Error> {
        self.admitted_triggers()?;
        match self
            .inner
            .entry(id)
            .map_err(|e| self.inner.error(IS_ENABLED, e.failure))?
        {
            Entry::Trigger { enabled } => Ok(*enabled),
            Entry::DeadTrigger => Err(self.inner.error(IS_ENABLED, Failure::Identity)),
            _ => Err(self.inner.error(IS_ENABLED, Failure::WrongFamily)),
        }
    }

    fn trigger_state_native(&mut self, name: &str, args: &[Value]) -> Result<Value, Error> {
        let [Value::Handle(id)] = args else {
            return Err(self
                .inner
                .error(name, Failure::TriggerState("expected one created trigger")));
        };
        let state = self
            .trigger_enabled(*id)
            .map_err(|e| self.inner.error(name, e.failure))?;
        if name == IS_ENABLED {
            return state.map(Value::Bool).ok_or_else(|| {
                self.inner
                    .error(name, Failure::TriggerState("enabled state is not assigned"))
            });
        }
        let Entry::Trigger { enabled } = self.inner.entry_mut(*id).expect("validated trigger")
        else {
            unreachable!("validated trigger")
        };
        *enabled = Some(name == ENABLE);
        Ok(Value::Null)
    }
}

pub(super) fn loaded_type(
    inner: &super::super::EnumHost,
    native: &str,
    type_name: &str,
    parent: &str,
    source: &str,
) -> Result<(), Error> {
    let ty = inner
        .program
        .types()
        .find(|ty| ty.name == type_name)
        .ok_or_else(|| inner.error(native, Failure::Signature))?;
    if ty.source != source {
        return Err(inner.error(native, Failure::Source));
    }
    if ty.parent != parent {
        return Err(inner.error(native, Failure::Signature));
    }
    Ok(())
}

const VARIABLE_EVENT: &str = "TriggerRegisterVariableEvent";
const VARIABLE_OPS: [&str; 6] = [
    "LESS_THAN",
    "LESS_THAN_OR_EQUAL",
    "EQUAL",
    "GREATER_THAN_OR_EQUAL",
    "GREATER_THAN",
    "NOT_EQUAL",
];

fn variable_holds(index: usize, value: f64, limit: f64) -> bool {
    match index {
        0 => value < limit,
        1 => value <= limit,
        2 => value == limit,
        3 => value >= limit,
        4 => value > limit,
        5 => value != limit,
        _ => false,
    }
}

impl ObjectHost {
    /// `TriggerRegisterVariableEvent`. The first later sample of a changed
    /// value fires when the comparison holds. The sample that records the
    /// current value does not.
    pub(super) fn register_variable_event(&mut self, args: &[Value]) -> Result<Value, Error> {
        let fail = |reason| self.inner.error(VARIABLE_EVENT, reason);
        self.admitted_triggers()?;
        let [
            Value::Handle(trigger),
            Value::Str(name),
            Value::Handle(limitop),
            limit,
        ] = args
        else {
            return Err(fail(Failure::TriggerState(
                "expected trigger, variable name, limitop and real threshold",
            )));
        };
        if name.is_empty() || name.as_bytes().contains(&0) {
            return Err(fail(Failure::TriggerState("variable event name is empty")));
        }
        let global =
            self.inner.program.global(name).ok_or_else(|| {
                fail(Failure::TriggerState("variable event name is not a global"))
            })?;
        if global.array
            || !matches!(
                global.ty.kind,
                pcx_script::ast::TypeKind::Real | pcx_script::ast::TypeKind::Integer
            )
        {
            return Err(fail(Failure::TriggerState(
                "variable event watches a real or integer",
            )));
        }
        if !matches!(
            self.inner.entry(*trigger).map_err(|e| fail(e.failure))?,
            Entry::Trigger { .. }
        ) {
            return Err(fail(Failure::WrongFamily));
        }
        let limit = limit
            .as_real()
            .filter(|value| value.is_finite())
            .ok_or_else(|| fail(Failure::TriggerState("finite real threshold required")))?;
        let limitop_value = self
            .inner
            .symbolic_value(*limitop, "limitop")
            .map_err(|e| fail(e.failure))?;
        let ops = self.variable_limit_ops()?;
        if !ops.contains(&limitop_value) {
            return Err(self
                .inner
                .error(VARIABLE_EVENT, Failure::OutsideDomain(limitop_value)));
        }
        let id = self.inner.allocate(
            Entry::RegisteredVariableEvent {
                trigger: *trigger,
                name: name.clone(),
                limitop_value,
                limit: f64::from(limit),
                last: None,
            },
            VARIABLE_EVENT,
        )?;
        self.triggers
            .as_mut()
            .expect("admitted")
            .variable_registrations
            .push(id);
        Ok(Value::Handle(id))
    }

    fn variable_limit_ops(&mut self) -> Result<[i32; 6], Error> {
        if let Some(ops) = self.triggers.as_ref().and_then(|state| state.variable_ops) {
            return Ok(ops);
        }
        let mut ops = [0; VARIABLE_OPS.len()];
        for (value, name) in ops.iter_mut().zip(VARIABLE_OPS) {
            *value = self.source_constant_enumerand(name, "limitop")?;
        }
        if ops
            .iter()
            .enumerate()
            .any(|(index, value)| ops[..index].contains(value))
        {
            return Err(self.inner.error(
                VARIABLE_EVENT,
                Failure::TriggerState("variable event limit operations are not distinct"),
            ));
        }
        self.triggers.as_mut().expect("admitted").variable_ops = Some(ops);
        Ok(ops)
    }

    pub(crate) fn variable_event_watches(&self) -> Vec<(HandleId, String)> {
        let Some(state) = &self.triggers else {
            return Vec::new();
        };
        state
            .variable_registrations
            .iter()
            .filter_map(|id| match self.inner.entry(*id).ok() {
                Some(Entry::RegisteredVariableEvent { name, .. }) => Some((*id, name.clone())),
                _ => None,
            })
            .collect()
    }

    pub(crate) fn poll_variable_events(
        &mut self,
        samples: &[(HandleId, Value)],
        work: &mut usize,
    ) -> Result<(), Error> {
        let ops = self.triggers.as_ref().and_then(|state| state.variable_ops);
        let Some(ops) = ops else {
            return Ok(());
        };
        for &(id, ref value) in samples {
            *work = work.checked_sub(1).ok_or_else(|| {
                self.inner
                    .error(VARIABLE_EVENT, Failure::Budget("variable event scan"))
            })?;
            let Entry::RegisteredVariableEvent {
                trigger,
                limitop_value,
                limit,
                last,
                ..
            } = self
                .inner
                .entry(id)
                .map_err(|e| self.inner.error(VARIABLE_EVENT, e.failure))?
            else {
                return Err(self.inner.error(VARIABLE_EVENT, Failure::WrongFamily));
            };
            let (trigger, limitop_value, limit, last) = (*trigger, *limitop_value, *limit, *last);
            let current = match value {
                Value::Real(value) if value.is_finite() => f64::from(*value),
                Value::Int(value) => *value as f64,
                _ => {
                    return Err(self.inner.error(
                        VARIABLE_EVENT,
                        Failure::TriggerState("variable event value is not a real or integer"),
                    ));
                }
            };
            let Some(index) = ops.iter().position(|op| *op == limitop_value) else {
                return Err(self
                    .inner
                    .error(VARIABLE_EVENT, Failure::OutsideDomain(limitop_value)));
            };
            let fire = match last {
                None => false,
                Some(previous) if previous == current => false,
                Some(_) => {
                    self.trigger_enabled(trigger)?.unwrap_or(false)
                        && variable_holds(index, current, limit)
                }
            };
            if fire {
                self.queue_event(trigger, None, super::super::EventSubject::default(), None)?;
            }
            if last != Some(current) {
                let step = match self.inner.entry_mut(id) {
                    Ok(Entry::RegisteredVariableEvent { last, .. }) => {
                        *last = Some(current);
                        None
                    }
                    Ok(_) => Some(Failure::WrongFamily),
                    Err(error) => Some(error.failure),
                };
                if let Some(failure) = step {
                    return Err(self.inner.error(VARIABLE_EVENT, failure));
                }
            }
        }
        Ok(())
    }
}

/// Same index lookup the chapter runner uses before a later `call`.
fn callback(program: &Program, id: FunctionId) -> Result<&pcx_script::resolve::Function, Failure> {
    let function = program
        .functions()
        .get(id.index())
        .ok_or(Failure::TriggerState("callback is not a loaded function"))?;
    if !function.params.is_empty() || function.ret.array || function.ret.kind.name() != "nothing" {
        return Err(Failure::TriggerState(
            "callback takes nothing returns nothing",
        ));
    }
    Ok(function)
}

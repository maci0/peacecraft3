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

#[path = "player_unit_event_host.rs"]
mod player_unit_events;
pub use player_unit_events::PlayerUnitEventRegistration;

#[path = "player_event_host.rs"]
mod player_events;
pub use player_events::PlayerEventRegistration;

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
    unit_in_range_limit: Option<usize>,
    retained_unit_in_range: usize,
    player_event_limit: Option<usize>,
    retained_player_events: usize,
    timer_event_limit: Option<usize>,
    retained_timer_events: usize,
    game_state_event_limit: Option<usize>,
    retained_game_state_events: usize,
    player_state_event_limit: Option<usize>,
    retained_player_state_events: usize,
    player_state_ops: Option<[i32; 6]>,
    player_state_event: Option<HandleId>,
    player_state_registrations: Vec<HandleId>,
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
            unit_in_range_limit: None,
            retained_unit_in_range: 0,
            player_event_limit: None,
            retained_player_events: 0,
            timer_event_limit: None,
            retained_timer_events: 0,
            game_state_event_limit: None,
            retained_game_state_events: 0,
            player_state_event_limit: None,
            retained_player_state_events: 0,
            player_state_ops: None,
            player_state_event: None,
            player_state_registrations: Vec::new(),
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
            death_events::REGISTER if self.triggers.as_ref()?.death_event_limit.is_some() => {
                Some(self.register_death_event(args))
            }
            range_events::REGISTER if self.triggers.as_ref()?.unit_in_range_limit.is_some() => {
                Some(self.register_unit_in_range(args))
            }
            player_events::REGISTER if self.triggers.as_ref()?.player_event_limit.is_some() => {
                Some(self.register_player_event(args))
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

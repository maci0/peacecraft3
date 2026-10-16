//! Shared-arena `CreateTrigger` and `TriggerRegisterUnitEvent` admission.
//!
//! Enabled state is not stored. `EnableTrigger`, `DisableTrigger`,
//! `IsTriggerEnabled`, conditions, actions and event delivery stay unsupported.
//! `TriggerAddAction` is not this contract: the loaded declaration returns
//! `triggeraction`.
use super::super::{Entry, Error, Failure, below};
use super::ObjectHost;
use pcx_script::{HandleId, Value};

const CREATE: &str = "CreateTrigger";
const REGISTER: &str = "TriggerRegisterUnitEvent";
const CONVERT: &str = "ConvertUnitEvent";

/// Caller bounds for retained identities. Not a firing or population limit.
#[derive(Clone, Copy, Debug)]
pub struct TriggerBudget {
    /// Retained `CreateTrigger` identities.
    pub max_triggers: usize,
    /// Retained `TriggerRegisterUnitEvent` event identities.
    pub max_unit_events: usize,
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
    retained_triggers: usize,
    retained_events: usize,
}

impl ObjectHost {
    /// Admit the loaded `CreateTrigger` and `TriggerRegisterUnitEvent` contracts.
    ///
    /// Both natives, `ConvertUnitEvent`, and the `trigger`, `event`, `eventid`,
    /// `unitevent`, and `unit` types must share one declaration source. Programs
    /// that never call this keep those natives unsupported.
    pub fn with_trigger_admission(mut self, budget: TriggerBudget) -> Result<Self, Error> {
        let fail = |reason| self.inner.error(CREATE, reason);
        if self.triggers.is_some() {
            return Err(fail(Failure::TriggerState("already admitted")));
        }
        std::alloc::Layout::array::<HandleId>(budget.max_triggers)
            .map_err(|_| fail(Failure::Capacity))?;
        std::alloc::Layout::array::<UnitEventRegistration>(budget.max_unit_events)
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
        self.triggers = Some(Admission {
            max_triggers: budget.max_triggers,
            max_unit_events: budget.max_unit_events,
            retained_triggers: 0,
            retained_events: 0,
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
        let id = self.inner.allocate(Entry::Trigger, CREATE)?;
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
                Entry::Trigger => {}
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
}

fn loaded_type(
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

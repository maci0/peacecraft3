//! Source-bound death subscriptions; no death detection or event delivery.
use super::{Entry, Error, Failure, ObjectHost, loaded_type};
use pcx_script::{HandleId, Value};

pub(super) const REGISTER: &str = "TriggerRegisterDeathEvent";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeathEventRegistration {
    pub trigger: HandleId,
    pub widget: HandleId,
}

impl ObjectHost {
    pub fn with_death_events(mut self, max_records: usize) -> Result<Self, Error> {
        if self.admitted_triggers()?.death_event_limit.is_some() {
            return Err(self.inner.error(
                REGISTER,
                Failure::TriggerState("death events already admitted"),
            ));
        }
        std::alloc::Layout::array::<DeathEventRegistration>(max_records)
            .map_err(|_| self.inner.error(REGISTER, Failure::Capacity))?;
        Self::contract(
            &self.inner,
            REGISTER,
            &["trigger", "widget"],
            "event",
            false,
            Some("event"),
        )?;
        let source = &self
            .inner
            .program
            .natives()
            .get(REGISTER)
            .expect("checked contract")
            .source;
        for name in ["CreateTrigger", "CreateUnit", "CreateDestructable"] {
            if let Some(native) = self.inner.program.natives().get(name)
                && &native.source != source
            {
                return Err(self.inner.error(REGISTER, Failure::Source));
            }
        }
        for (name, parent) in [("widget", "agent"), ("unit", "widget")] {
            loaded_type(&self.inner, REGISTER, name, parent, source)?;
        }
        if self
            .inner
            .program
            .types()
            .any(|ty| ty.name == "destructable")
        {
            loaded_type(&self.inner, REGISTER, "destructable", "widget", source)?;
        }
        self.triggers
            .as_mut()
            .expect("admitted above")
            .death_event_limit = Some(max_records);
        Ok(self)
    }

    pub fn retained_death_events(&self) -> Result<usize, Error> {
        let a = self.admitted_triggers()?;
        if a.death_event_limit.is_none() {
            return Err(self
                .inner
                .error(REGISTER, Failure::TriggerState("death events not admitted")));
        }
        Ok(a.retained_death_events)
    }

    pub fn death_event_registration(&self, id: HandleId) -> Result<DeathEventRegistration, Error> {
        self.retained_death_events()?;
        match self
            .inner
            .entry(id)
            .map_err(|e| self.inner.error(REGISTER, e.failure))?
        {
            Entry::RegisteredDeathEvent(record) => Ok(*record),
            _ => Err(self.inner.error(REGISTER, Failure::WrongFamily)),
        }
    }

    pub(super) fn register_death_event(&mut self, args: &[Value]) -> Result<Value, Error> {
        let fail = |failure| self.inner.error(REGISTER, failure);
        let [Value::Handle(trigger), Value::Handle(widget)] = args else {
            return Err(fail(Failure::TriggerState("expected trigger and widget")));
        };
        if !matches!(
            self.inner.entry(*trigger).map_err(|e| fail(e.failure))?,
            Entry::Trigger { .. }
        ) || !matches!(
            self.inner.entry(*widget).map_err(|e| fail(e.failure))?,
            Entry::Unit { .. } | Entry::Destructable { .. }
        ) {
            return Err(fail(Failure::WrongFamily));
        }
        let a = self.admitted_triggers()?;
        let limit = a
            .death_event_limit
            .ok_or_else(|| fail(Failure::TriggerState("death events not admitted")))?;
        if a.retained_death_events >= limit {
            return Err(fail(Failure::Budget("death event registration")));
        }
        let id = self.inner.allocate(
            Entry::RegisteredDeathEvent(DeathEventRegistration {
                trigger: *trigger,
                widget: *widget,
            }),
            REGISTER,
        )?;
        self.triggers
            .as_mut()
            .expect("admitted above")
            .retained_death_events += 1;
        Ok(Value::Handle(id))
    }
}

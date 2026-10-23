//! Source-bound death subscriptions; no death detection or event delivery.
use super::{Entry, Error, Failure, ObjectHost, loaded_type};
use pcx_script::{HandleId, Value};

pub(super) const REGISTER: &str = "TriggerRegisterDeathEvent";
const CONVERT_EVENT: &str = "ConvertWidgetEvent";
const DEATH_EVENT: &str = "EVENT_WIDGET_DEATH";

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
        let source = self
            .inner
            .program
            .natives()
            .get(REGISTER)
            .expect("checked contract")
            .source
            .clone();
        for name in ["CreateTrigger", "CreateUnit", "CreateDestructable"] {
            if let Some(native) = self.inner.program.natives().get(name)
                && native.source != source
            {
                return Err(self.inner.error(REGISTER, Failure::Source));
            }
        }
        for (name, parent) in [("widget", "agent"), ("unit", "widget")] {
            loaded_type(&self.inner, REGISTER, name, parent, &source)?;
        }
        if self
            .inner
            .program
            .types()
            .any(|ty| ty.name == "destructable")
        {
            loaded_type(&self.inner, REGISTER, "destructable", "widget", &source)?;
        }
        for (name, result, kind) in [
            ("GetTriggerWidget", "widget", "widget"),
            ("GetTriggerDestructable", "destructable", "destructable"),
        ] {
            if self.inner.program.natives().get(name).is_some() {
                Self::contract(&self.inner, name, &[], result, true, Some(kind))?;
                if self
                    .inner
                    .program
                    .natives()
                    .get(name)
                    .is_none_or(|n| n.source != source)
                {
                    return Err(self.inner.error(name, Failure::Source));
                }
            }
        }
        let death_event = self.widget_death_event(&source)?;
        self.triggers
            .as_mut()
            .expect("admitted above")
            .death_event_limit = Some(max_records);
        let admitted = self.triggers.as_mut().expect("admitted above");
        admitted.death_event = Some(death_event);
        admitted.death_event_source = Some(source);
        Ok(self)
    }

    /// Derive the selected widget-death event handle from the source global and
    /// its admitted conversion. A missing global, another source, a different
    /// initializer, or an enumerand outside the conversion domain is an error.
    pub(crate) fn widget_death_event(&mut self, source: &str) -> Result<HandleId, Error> {
        let global = self
            .inner
            .program
            .globals()
            .iter()
            .find(|g| g.name == DEATH_EVENT)
            .ok_or_else(|| {
                self.inner
                    .error(DEATH_EVENT, Failure::TriggerState("global is not declared"))
            })?;
        if global.source != source {
            return Err(self.inner.error(DEATH_EVENT, Failure::Source));
        }
        if !global.is_constant
            || global.array
            || global.ty.array
            || global.ty.kind.name() != "widgetevent"
        {
            return Err(self.inner.error(DEATH_EVENT, Failure::Signature));
        }
        let Some(pcx_script::ast::Expr::Call { callee, args, .. }) = &global.init else {
            return Err(self
                .inner
                .error(DEATH_EVENT, Failure::Initializer(DEATH_EVENT.to_owned())));
        };
        let (pcx_script::ast::Expr::Ident(conversion, _), [arg]) =
            (callee.as_ref(), args.as_slice())
        else {
            return Err(self
                .inner
                .error(DEATH_EVENT, Failure::Initializer(DEATH_EVENT.to_owned())));
        };
        if conversion.as_str() != CONVERT_EVENT {
            return Err(self
                .inner
                .error(DEATH_EVENT, Failure::Initializer(DEATH_EVENT.to_owned())));
        }
        Self::contract(
            &self.inner,
            CONVERT_EVENT,
            &["integer"],
            "widgetevent",
            true,
            None,
        )?;
        let admitted = self
            .inner
            .conversions()
            .find(|c| c.name == CONVERT_EVENT && c.kind == "widgetevent")
            .ok_or_else(|| self.inner.error(CONVERT_EVENT, Failure::Signature))?;
        if admitted.source != source
            || admitted.domain.is_empty()
            || self
                .inner
                .program
                .natives()
                .get(CONVERT_EVENT)
                .is_none_or(|native| native.source != source)
        {
            return Err(self.inner.error(CONVERT_EVENT, Failure::Source));
        }
        let mut work = 4096;
        let enumerand = crate::playercolor_host::integer(arg, 64, &mut work)
            .map_err(|failure| self.inner.error(DEATH_EVENT, failure))?;
        if !admitted.domain.contains(&enumerand) {
            return Err(self
                .inner
                .error(DEATH_EVENT, Failure::OutsideDomain(enumerand)));
        }
        loaded_type(&self.inner, DEATH_EVENT, "widgetevent", "eventid", source)?;
        loaded_type(&self.inner, DEATH_EVENT, "eventid", "handle", source)?;
        match self
            .inner
            .convert_for(CONVERT_EVENT, &[Value::Int(enumerand)])
            .map_err(|e| self.inner.error(DEATH_EVENT, e.failure))?
        {
            Value::Handle(event) => Ok(event),
            _ => Err(self.inner.error(DEATH_EVENT, Failure::Signature)),
        }
    }

    /// Derive the widget-death event only when the global is declared.
    /// A declared global still has to satisfy the full source contract.
    pub(crate) fn optional_widget_death_event(
        &mut self,
        source: &str,
    ) -> Result<Option<HandleId>, Error> {
        let declared = self
            .inner
            .program
            .globals()
            .iter()
            .any(|global| global.name == DEATH_EVENT);
        declared
            .then(|| self.widget_death_event(source))
            .transpose()
    }

    pub(crate) fn destructable_death_registered(&self) -> bool {
        self.triggers.as_ref().is_some_and(|admitted| {
            admitted.death_event_limit.is_some() && admitted.retained_death_events > 0
        })
    }

    pub(crate) fn destructable_death_event_handle(&self) -> Option<HandleId> {
        self.triggers
            .as_ref()
            .and_then(|admitted| admitted.death_event)
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

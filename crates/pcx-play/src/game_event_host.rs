//! Generic game-event subscriptions and delivery on the shared trigger runtime.
use super::{Entry, Error, Failure, ObjectHost};
use pcx_script::Value;

pub(super) const REGISTER: &str = "TriggerRegisterGameEvent";

impl ObjectHost {
    /// Registrations share the session arena bound; event numbers come from loaded globals.
    pub fn with_game_events(mut self) -> Result<Self, Error> {
        if self.admitted_triggers()?.game_events {
            return Err(self.inner.error(
                REGISTER,
                Failure::TriggerState("game events already admitted"),
            ));
        }
        Self::contract(
            &self.inner,
            REGISTER,
            &["trigger", "gameevent"],
            "event",
            false,
            Some("event"),
        )?;
        let source = &self
            .inner
            .program
            .natives()
            .get(REGISTER)
            .expect("checked")
            .source;
        Self::contract_in(
            &self.inner,
            "ConvertGameEvent",
            &["integer"],
            "gameevent",
            true,
            None,
            false,
        )?;
        for name in ["CreateTrigger", "ConvertGameEvent"] {
            if self
                .inner
                .program
                .natives()
                .get(name)
                .is_none_or(|n| &n.source != source)
            {
                return Err(self.inner.error(REGISTER, Failure::Source));
            }
        }
        if self
            .inner
            .conversions
            .get("ConvertGameEvent")
            .is_none_or(|c| &c.source != source || c.kind != "gameevent")
        {
            return Err(self.inner.error(REGISTER, Failure::Signature));
        }
        super::loaded_type(&self.inner, REGISTER, "gameevent", "eventid", source)?;
        self.triggers.as_mut().expect("admitted").game_events = true;
        Ok(self)
    }

    pub(super) fn register_game_event(&mut self, args: &[Value]) -> Result<Value, Error> {
        let [Value::Handle(trigger), Value::Handle(event)] = args else {
            return Err(self.inner.error(REGISTER, Failure::Signature));
        };
        self.trigger_enabled(*trigger)
            .map_err(|e| self.inner.error(REGISTER, e.failure))?;
        let enumerand = self
            .inner
            .symbolic_value(*event, "gameevent")
            .map_err(|e| self.inner.error(REGISTER, e.failure))?;
        if !self
            .inner
            .conversions
            .get("ConvertGameEvent")
            .is_some_and(|c| c.domain.contains(&enumerand))
        {
            return Err(self
                .inner
                .error(REGISTER, Failure::OutsideDomain(enumerand)));
        }
        self.inner
            .allocate(
                Entry::RegisteredGameEvent {
                    trigger: *trigger,
                    gameevent: *event,
                    enumerand,
                },
                REGISTER,
            )
            .map(Value::Handle)
    }

    /// Publish an actual engine/UI event, identified by its source constant name.
    /// Registration alone never fires it; the session delivers its callbacks normally.
    pub fn fire_game_event(&mut self, global: &str) -> Result<usize, Error> {
        if !self.admitted_triggers()?.game_events || self.trigger_runtime.is_none() {
            return Err(self.inner.error(
                REGISTER,
                Failure::TriggerState("game-event delivery is not admitted"),
            ));
        }
        let enumerand = self.source_constant_enumerand(global, "gameevent")?;
        let mut fired = Vec::new();
        for entry in &self.inner.arena {
            if let Entry::RegisteredGameEvent {
                trigger,
                gameevent,
                enumerand: value,
            } = entry
                && *value == enumerand
                && matches!(self.inner.entry(*trigger)?, Entry::Trigger { .. })
            {
                fired
                    .try_reserve(1)
                    .map_err(|_| self.inner.error(global, Failure::Capacity))?;
                fired.push((*trigger, *gameevent));
            }
        }
        self.reserve_game_event_delivery(fired.len())?;
        for &(trigger, gameevent) in &fired {
            self.push_reserved_event(trigger, None, Default::default(), Some(gameevent));
        }
        Ok(fired.len())
    }
}

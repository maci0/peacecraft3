//! Source-bound player-event associations; no event evaluation or dispatch.
use super::{Entry, Error, Failure, ObjectHost, loaded_type};
use pcx_script::{HandleId, Value};

pub(super) const REGISTER: &str = "TriggerRegisterPlayerEvent";
const CONVERT: &str = "ConvertPlayerEvent";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerEventRegistration {
    pub trigger: HandleId,
    pub player: HandleId,
    pub playerevent: HandleId,
    pub enumerand: i32,
}

impl ObjectHost {
    /// Retain registrations under an explicit bound, without inventing trigger state.
    pub fn with_player_events(mut self, max_records: usize) -> Result<Self, Error> {
        self.admitted_triggers()?;
        let fail = |failure| self.inner.error(REGISTER, failure);
        if self
            .triggers
            .as_ref()
            .is_some_and(|a| a.player_event_limit.is_some())
        {
            return Err(fail(Failure::TriggerState(
                "player events already admitted",
            )));
        }
        std::alloc::Layout::array::<PlayerEventRegistration>(max_records)
            .map_err(|_| fail(Failure::Capacity))?;
        Self::contract(
            &self.inner,
            REGISTER,
            &["trigger", "player", "playerevent"],
            "event",
            false,
            Some("event"),
        )?;
        Self::contract(
            &self.inner,
            "Player",
            &["integer"],
            "player",
            true,
            Some("player"),
        )?;
        Self::contract(
            &self.inner,
            CONVERT,
            &["integer"],
            "playerevent",
            true,
            None,
        )?;
        let source = &self
            .inner
            .program
            .natives()
            .get(REGISTER)
            .expect("checked contract")
            .source;
        for name in ["CreateTrigger", "Player", CONVERT] {
            if self
                .inner
                .program
                .natives()
                .get(name)
                .is_none_or(|n| &n.source != source)
            {
                return Err(fail(Failure::Source));
            }
        }
        if self
            .inner
            .conversions
            .get(CONVERT)
            .is_none_or(|c| &c.source != source || c.kind != "playerevent")
        {
            return Err(fail(Failure::Signature));
        }
        for (name, parent) in [("player", "agent"), ("playerevent", "eventid")] {
            loaded_type(&self.inner, REGISTER, name, parent, source)?;
        }
        self.triggers
            .as_mut()
            .expect("admitted above")
            .player_event_limit = Some(max_records);
        Ok(self)
    }

    pub fn retained_player_events(&self) -> Result<usize, Error> {
        let admitted = self.admitted_triggers()?;
        if admitted.player_event_limit.is_none() {
            return Err(self.inner.error(
                REGISTER,
                Failure::TriggerState("player events not admitted"),
            ));
        }
        Ok(admitted.retained_player_events)
    }

    pub fn player_event_registration(
        &self,
        id: HandleId,
    ) -> Result<PlayerEventRegistration, Error> {
        self.retained_player_events()?;
        match self
            .inner
            .entry(id)
            .map_err(|e| self.inner.error(REGISTER, e.failure))?
        {
            Entry::RegisteredPlayerEvent(record) => Ok(*record),
            _ => Err(self.inner.error(REGISTER, Failure::WrongFamily)),
        }
    }

    pub(super) fn register_player_event(&mut self, args: &[Value]) -> Result<Value, Error> {
        let fail = |failure| self.inner.error(REGISTER, failure);
        let [
            Value::Handle(trigger),
            Value::Handle(player),
            Value::Handle(event),
        ] = args
        else {
            return Err(fail(Failure::TriggerState(
                "expected trigger, player and playerevent",
            )));
        };
        if !matches!(
            self.inner.entry(*trigger).map_err(|e| fail(e.failure))?,
            Entry::Trigger { .. }
        ) || !matches!(
            self.inner.entry(*player).map_err(|e| fail(e.failure))?,
            Entry::Player { .. }
        ) {
            return Err(fail(Failure::WrongFamily));
        }
        let enumerand = self
            .inner
            .symbolic_value(*event, "playerevent")
            .map_err(|e| fail(e.failure))?;
        if !self
            .inner
            .conversions
            .get(CONVERT)
            .is_some_and(|c| c.domain.contains(&enumerand))
        {
            return Err(fail(Failure::OutsideDomain(enumerand)));
        }
        let admitted = self.admitted_triggers()?;
        let limit = admitted
            .player_event_limit
            .ok_or_else(|| fail(Failure::TriggerState("player events not admitted")))?;
        if admitted.retained_player_events >= limit {
            return Err(fail(Failure::Budget("player event registration")));
        }
        let record = PlayerEventRegistration {
            trigger: *trigger,
            player: *player,
            playerevent: *event,
            enumerand,
        };
        let id = self
            .inner
            .allocate(Entry::RegisteredPlayerEvent(record), REGISTER)?;
        self.triggers
            .as_mut()
            .expect("admitted above")
            .retained_player_events += 1;
        Ok(Value::Handle(id))
    }
}

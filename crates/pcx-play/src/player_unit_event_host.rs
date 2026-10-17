//! Source-bound player/unit-event associations; no event evaluation or dispatch.
use super::{Entry, Error, Failure, ObjectHost, loaded_type};
use pcx_script::{HandleId, Value};

pub(super) const REGISTER: &str = "TriggerRegisterPlayerUnitEvent";
const CONVERT: &str = "ConvertPlayerUnitEvent";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerUnitEventRegistration {
    pub trigger: HandleId,
    pub player: HandleId,
    pub playerunitevent: HandleId,
    pub enumerand: i32,
    pub filter: Option<HandleId>,
}

impl ObjectHost {
    /// Retain registrations under an explicit bound, without inventing trigger state.
    pub fn with_player_unit_events(mut self, max_records: usize) -> Result<Self, Error> {
        self.admitted_triggers()?;
        let fail = |failure| self.inner.error(REGISTER, failure);
        if self
            .triggers
            .as_ref()
            .is_some_and(|a| a.player_unit_event_limit.is_some())
        {
            return Err(fail(Failure::TriggerState(
                "player-unit events already admitted",
            )));
        }
        std::alloc::Layout::array::<PlayerUnitEventRegistration>(max_records)
            .map_err(|_| fail(Failure::Capacity))?;
        Self::contract(
            &self.inner,
            REGISTER,
            &["trigger", "player", "playerunitevent", "boolexpr"],
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
            "playerunitevent",
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
            .is_none_or(|c| &c.source != source || c.kind != "playerunitevent")
        {
            return Err(fail(Failure::Signature));
        }
        for (name, parent) in [
            ("player", "agent"),
            ("playerunitevent", "eventid"),
            ("boolexpr", "agent"),
        ] {
            loaded_type(&self.inner, REGISTER, name, parent, source)?;
        }
        self.triggers
            .as_mut()
            .expect("admitted above")
            .player_unit_event_limit = Some(max_records);
        Ok(self)
    }

    pub fn retained_player_unit_events(&self) -> Result<usize, Error> {
        let admitted = self.admitted_triggers()?;
        if admitted.player_unit_event_limit.is_none() {
            return Err(self.inner.error(
                REGISTER,
                Failure::TriggerState("player-unit events not admitted"),
            ));
        }
        Ok(admitted.retained_player_unit_events)
    }

    pub fn player_unit_event_registration(
        &self,
        id: HandleId,
    ) -> Result<PlayerUnitEventRegistration, Error> {
        self.retained_player_unit_events()?;
        match self
            .inner
            .entry(id)
            .map_err(|e| self.inner.error(REGISTER, e.failure))?
        {
            Entry::RegisteredPlayerUnitEvent(record) => Ok(*record),
            _ => Err(self.inner.error(REGISTER, Failure::WrongFamily)),
        }
    }

    pub(super) fn register_player_unit_event(&mut self, args: &[Value]) -> Result<Value, Error> {
        let fail = |failure| self.inner.error(REGISTER, failure);
        let [
            Value::Handle(trigger),
            Value::Handle(player),
            Value::Handle(event),
            filter,
        ] = args
        else {
            return Err(fail(Failure::TriggerState(
                "expected trigger, player, playerunitevent and nullable filter",
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
            .symbolic_value(*event, "playerunitevent")
            .map_err(|e| fail(e.failure))?;
        if !self
            .inner
            .conversions
            .get(CONVERT)
            .is_some_and(|c| c.domain.contains(&enumerand))
        {
            return Err(fail(Failure::OutsideDomain(enumerand)));
        }
        let filter = match filter {
            Value::Null => None,
            Value::Handle(id) => {
                self.checked_boolean_wrapper(*id, REGISTER)?;
                Some(*id)
            }
            _ => {
                return Err(fail(Failure::TriggerState(
                    "expected nullable filter identity",
                )));
            }
        };
        let admitted = self.admitted_triggers()?;
        let limit = admitted
            .player_unit_event_limit
            .ok_or_else(|| fail(Failure::TriggerState("player-unit events not admitted")))?;
        if admitted.retained_player_unit_events >= limit {
            return Err(fail(Failure::Budget("player-unit event registration")));
        }
        let record = PlayerUnitEventRegistration {
            trigger: *trigger,
            player: *player,
            playerunitevent: *event,
            enumerand,
            filter,
        };
        let id = self
            .inner
            .allocate(Entry::RegisteredPlayerUnitEvent(record), REGISTER)?;
        self.triggers
            .as_mut()
            .expect("admitted above")
            .retained_player_unit_events += 1;
        Ok(Value::Handle(id))
    }
}

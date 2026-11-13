//! Source-bound player chat subscriptions. Registration retains the trigger, player,
//! exact message and match flag. The host has no typed-chat source, so no
//! registration fires, and `GetEventPlayerChatString` and
//! `GetEventPlayerChatStringMatched` stay unsupported.
use super::{Entry, Error, Failure, ObjectHost, loaded_type};
use pcx_script::{HandleId, Value};

pub(super) const REGISTER: &str = "TriggerRegisterPlayerChatEvent";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerChatRegistration {
    pub trigger: HandleId,
    pub player: HandleId,
    pub message: pcx_script::ScriptString,
    pub exact_match_only: bool,
}

impl ObjectHost {
    /// Retain chat registrations under an explicit bound, without inventing trigger state.
    pub fn with_player_chat_events(mut self, max_records: usize) -> Result<Self, Error> {
        self.admitted_triggers()?;
        let fail = |failure| self.inner.error(REGISTER, failure);
        if self
            .triggers
            .as_ref()
            .is_some_and(|a| a.player_chat_event_limit.is_some())
        {
            return Err(fail(Failure::TriggerState(
                "player chat events already admitted",
            )));
        }
        std::alloc::Layout::array::<PlayerChatRegistration>(max_records)
            .map_err(|_| fail(Failure::Capacity))?;
        Self::contract(
            &self.inner,
            REGISTER,
            &["trigger", "player", "string", "boolean"],
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
        let source = &self
            .inner
            .program
            .natives()
            .get(REGISTER)
            .expect("checked contract")
            .source;
        for name in ["CreateTrigger", "Player"] {
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
        loaded_type(&self.inner, REGISTER, "player", "agent", source)?;
        self.triggers
            .as_mut()
            .expect("admitted above")
            .player_chat_event_limit = Some(max_records);
        Ok(self)
    }

    pub fn retained_player_chat_events(&self) -> Result<usize, Error> {
        let admitted = self.admitted_triggers()?;
        if admitted.player_chat_event_limit.is_none() {
            return Err(self.inner.error(
                REGISTER,
                Failure::TriggerState("player chat events not admitted"),
            ));
        }
        Ok(admitted.retained_player_chat_events)
    }

    pub fn player_chat_registration(&self, id: HandleId) -> Result<&PlayerChatRegistration, Error> {
        self.retained_player_chat_events()?;
        match self
            .inner
            .entry(id)
            .map_err(|e| self.inner.error(REGISTER, e.failure))?
        {
            Entry::RegisteredPlayerChat(record) => Ok(record),
            _ => Err(self.inner.error(REGISTER, Failure::WrongFamily)),
        }
    }

    pub(super) fn register_player_chat_event(&mut self, args: &[Value]) -> Result<Value, Error> {
        self.register_player_chat_event_with_strings(args, None)
    }
    pub(in crate::playercolor_host) fn register_player_chat_event_with_strings(
        &mut self,
        args: &[Value],
        strings: Option<&pcx_script::text::NativeStringLookup<'_>>,
    ) -> Result<Value, Error> {
        let fail = |failure| self.inner.error(REGISTER, failure);
        let [
            Value::Handle(trigger),
            Value::Handle(player),
            Value::Str(message),
            Value::Bool(exact_match_only),
        ] = args
        else {
            return Err(fail(Failure::TriggerState(
                "expected trigger, player, string and boolean",
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
        let admitted = self.admitted_triggers()?;
        let limit = admitted
            .player_chat_event_limit
            .ok_or_else(|| fail(Failure::TriggerState("player chat events not admitted")))?;
        let message = pcx_script::text::native_map_bytes(message.as_bytes(), strings)
            .map_err(|why| fail(Failure::ScriptString(why)))?;
        if admitted.retained_player_chat_events >= limit {
            return Err(fail(Failure::Budget("player chat event registration")));
        }
        let mut owned = Vec::new();
        owned
            .try_reserve_exact(message.len())
            .map_err(|_| fail(Failure::Capacity))?;
        owned.extend_from_slice(&message);
        let record = PlayerChatRegistration {
            trigger: *trigger,
            player: *player,
            message: owned.into(),
            exact_match_only: *exact_match_only,
        };
        let id = self
            .inner
            .allocate(Entry::RegisteredPlayerChat(record), REGISTER)?;
        self.triggers
            .as_mut()
            .expect("admitted above")
            .retained_player_chat_events += 1;
        Ok(Value::Handle(id))
    }
}

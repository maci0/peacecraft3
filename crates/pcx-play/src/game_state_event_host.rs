//! Source-bound state subscriptions; comparison and event delivery stay separate.
use super::{Entry, Error, Failure, ObjectHost, below};
use pcx_script::{HandleId, Value};
pub(super) const REGISTER: &str = "TriggerRegisterGameStateEvent";

/// Typed source identities and executed threshold, not an evaluated condition.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GameStateEventRegistration<'a> {
    pub trigger: HandleId,
    pub state: HandleId,
    pub state_kind: &'a str,
    pub state_value: i32,
    pub limitop: HandleId,
    pub limitop_value: i32,
    pub limit: f32,
}
impl ObjectHost {
    /// Admit selected I/F game-state and limitop domains on the existing arena.
    /// No selector, comparison opcode, threshold or state default is supplied.
    pub fn with_game_state_events(mut self, max_records: usize) -> Result<Self, Error> {
        let fail = |reason| self.inner.error(REGISTER, reason);
        let admitted = self.triggers.as_ref().ok_or_else(|| {
            fail(Failure::TriggerState(
                "trigger creation must already be admitted",
            ))
        })?;
        if admitted.game_state_event_limit.is_some() {
            return Err(fail(Failure::TriggerState(
                "game state events already admitted",
            )));
        }
        std::alloc::Layout::array::<GameStateEventRegistration<'_>>(max_records)
            .map_err(|_| fail(Failure::Capacity))?;
        Self::contract(
            &self.inner,
            REGISTER,
            &["trigger", "gamestate", "limitop", "real"],
            "event",
            false,
            Some("event"),
        )?;
        let program = &self.inner.program;
        let source = &program
            .natives()
            .get(REGISTER)
            .ok_or_else(|| fail(Failure::Signature))?
            .source;
        if program
            .natives()
            .get("CreateTrigger")
            .is_none_or(|n| &n.source != source)
        {
            return Err(fail(Failure::Source));
        }
        for (native, kind, state) in [
            ("ConvertIGameState", "igamestate", true),
            ("ConvertFGameState", "fgamestate", true),
            ("ConvertLimitOp", "limitop", false),
        ] {
            Self::contract(&self.inner, native, &["integer"], kind, true, None)?;
            let conversion = self
                .inner
                .conversions
                .get(native)
                .ok_or_else(|| fail(Failure::Signature))?;
            if conversion.kind != kind {
                return Err(fail(Failure::Signature));
            }
            if &conversion.source != source
                || program
                    .natives()
                    .get(native)
                    .is_none_or(|n| &n.source != source)
            {
                return Err(self.inner.error(native, Failure::Source));
            }
            if state && !below(program, kind, "gamestate") {
                return Err(fail(Failure::Signature));
            }
            let mut current = kind;
            for _ in 0..=program.types().count() {
                if current == "handle" {
                    break;
                }
                let ty = program
                    .types()
                    .find(|ty| ty.name == current)
                    .ok_or_else(|| fail(Failure::Signature))?;
                if &ty.source != source {
                    return Err(fail(Failure::Source));
                }
                current = &ty.parent;
            }
            if current != "handle" {
                return Err(fail(Failure::Signature));
            }
        }
        self.triggers
            .as_mut()
            .ok_or_else(|| fail(Failure::TriggerState("not admitted")))?
            .game_state_event_limit = Some(max_records);
        Ok(self)
    }
    pub fn retained_game_state_events(&self) -> Result<usize, Error> {
        let admitted = self
            .triggers
            .as_ref()
            .filter(|a| a.game_state_event_limit.is_some())
            .ok_or_else(|| {
                self.inner.error(
                    REGISTER,
                    Failure::TriggerState("game state events not admitted"),
                )
            })?;
        Ok(admitted.retained_game_state_events)
    }
    pub fn game_state_event_registration(
        &self,
        id: HandleId,
    ) -> Result<GameStateEventRegistration<'_>, Error> {
        self.retained_game_state_events()?;
        let Entry::RegisteredGameStateEvent {
            trigger,
            state,
            limitop,
            limit,
        } = self
            .inner
            .entry(id)
            .map_err(|e| self.inner.error(REGISTER, e.failure))?
        else {
            return Err(self.inner.error(REGISTER, Failure::WrongFamily));
        };
        let state_kind = self.inner.kind(*state)?;
        Ok(GameStateEventRegistration {
            trigger: *trigger,
            state: *state,
            state_kind,
            state_value: self.inner.symbolic_value(*state, state_kind)?,
            limitop: *limitop,
            limitop_value: self.inner.symbolic_value(*limitop, "limitop")?,
            limit: *limit,
        })
    }
    fn game_state_event_value(&self, id: HandleId, kind: &str, native: &str) -> Result<i32, Error> {
        let fail = |reason| self.inner.error(REGISTER, reason);
        let value = self
            .inner
            .symbolic_value(id, kind)
            .map_err(|e| fail(e.failure))?;
        let conversion = self
            .inner
            .conversions
            .get(native)
            .ok_or_else(|| fail(Failure::Signature))?;
        if !conversion.domain.contains(&value) {
            return Err(fail(Failure::OutsideDomain(value)));
        }
        Ok(value)
    }
    pub(super) fn register_game_state_event(&mut self, args: &[Value]) -> Result<Value, Error> {
        let fail = |reason| self.inner.error(REGISTER, reason);
        let [
            Value::Handle(trigger),
            Value::Handle(state),
            Value::Handle(limitop),
            limit,
        ] = args
        else {
            return Err(fail(Failure::TriggerState(
                "expected trigger, game state, limitop and real threshold",
            )));
        };
        if !matches!(
            self.inner.entry(*trigger).map_err(|e| fail(e.failure))?,
            Entry::Trigger
        ) {
            return Err(fail(Failure::WrongFamily));
        }
        let kind = self.inner.kind(*state).map_err(|e| fail(e.failure))?;
        let conversion = match kind {
            "igamestate" => "ConvertIGameState",
            "fgamestate" => "ConvertFGameState",
            _ => return Err(fail(Failure::WrongFamily)),
        };
        self.game_state_event_value(*state, kind, conversion)?;
        self.game_state_event_value(*limitop, "limitop", "ConvertLimitOp")?;
        let limit = limit
            .as_real()
            .filter(|v| v.is_finite())
            .ok_or_else(|| fail(Failure::TriggerState("finite real threshold required")))?;
        let admitted = self
            .triggers
            .as_ref()
            .ok_or_else(|| fail(Failure::TriggerState("not admitted")))?;
        let max = admitted
            .game_state_event_limit
            .ok_or_else(|| fail(Failure::TriggerState("game state events not admitted")))?;
        if admitted.retained_game_state_events >= max {
            return Err(fail(Failure::Budget("game state event registration")));
        }
        let id = self.inner.allocate(
            Entry::RegisteredGameStateEvent {
                trigger: *trigger,
                state: *state,
                limitop: *limitop,
                limit,
            },
            REGISTER,
        )?;
        let admitted = self.triggers.as_mut().ok_or_else(|| {
            self.inner
                .error(REGISTER, Failure::TriggerState("not admitted"))
        })?;
        admitted.retained_game_state_events += 1;
        Ok(Value::Handle(id))
    }
}

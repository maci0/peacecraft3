//! Executed result transitions, without ending the owning script/session.
use super::{Entry, Error, EventSubject, Failure, ObjectHost};
use crate::player_removal_rules::Rules;
use pcx_script::{HandleId, Value};

/// Application resource limits, not original game player/event limits.
#[derive(Clone, Copy, Debug)]
pub struct PlayerRemovalBudget {
    pub max_players: usize,
    /// Cumulative callbacks queued by accepted removal transitions in this session.
    pub max_event_deliveries: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerRemoval {
    pub player: HandleId,
    pub game_result: HandleId,
    /// Victory retains PLAYING; the other source results transition to LEFT.
    pub left: bool,
}
pub struct PlayerRemovalState {
    budget: PlayerRemovalBudget,
    rules: Rules,
    records: Vec<PlayerRemoval>,
    deliveries: usize,
    playing: HandleId,
    left: HandleId,
}
impl PlayerRemovalState {
    pub fn get(&self, player: HandleId) -> Option<&PlayerRemoval> {
        self.records.iter().find(|r| r.player == player)
    }
    pub fn records(&self) -> &[PlayerRemoval] {
        &self.records
    }
    pub fn source_rules(&self) -> &Rules {
        &self.rules
    }
}
const REMOVE: &str = "RemovePlayer";
impl ObjectHost {
    pub fn with_player_removal(
        mut self,
        rules: Rules,
        budget: PlayerRemovalBudget,
    ) -> Result<Self, Error> {
        if self.player_removal.is_some() {
            return Err(self.inner.error(
                REMOVE,
                Failure::PlayerState("player removal already admitted"),
            ));
        }
        if self.inner.program.natives().get(REMOVE).is_none()
            && self.inner.program.natives().ambiguous(REMOVE).is_none()
        {
            return Ok(self);
        }
        if self.player_slots().is_none() {
            return Err(self
                .inner
                .error(REMOVE, Failure::PlayerState("player rows are not admitted")));
        }
        self.retained_player_events()?;
        Self::contract_in(
            &self.inner,
            "GetPlayerSlotState",
            &["player"],
            "playerslotstate",
            false,
            None,
            false,
        )?;
        std::alloc::Layout::array::<PlayerRemoval>(budget.max_players)
            .map_err(|_| self.inner.error(REMOVE, Failure::Capacity))?;
        Self::contract_in(
            &self.inner,
            REMOVE,
            &["player", "playergameresult"],
            "nothing",
            false,
            Some("player"),
            true,
        )?;
        let source = self
            .inner
            .program
            .natives()
            .get(REMOVE)
            .expect("checked native")
            .source
            .clone();
        for name in [
            "Player",
            "GetPlayerSlotState",
            "CreateTimer",
            "TriggerRegisterPlayerEvent",
        ] {
            if self
                .inner
                .program
                .natives()
                .get(name)
                .is_none_or(|n| n.source != source)
            {
                return Err(self.inner.error(REMOVE, Failure::Source));
            }
        }
        for (convert, kind) in [
            ("ConvertPlayerGameResult", "playergameresult"),
            ("ConvertPlayerSlotState", "playerslotstate"),
            ("ConvertPlayerEvent", "playerevent"),
        ] {
            Self::contract_in(&self.inner, convert, &["integer"], kind, true, None, false)?;
            if self
                .inner
                .program
                .natives()
                .get(convert)
                .is_none_or(|n| n.source != source)
                || self
                    .inner
                    .conversions
                    .get(convert)
                    .is_none_or(|c| c.source != source || c.kind != kind)
            {
                return Err(self.inner.error(REMOVE, Failure::Source));
            }
        }
        for (global, kind, expected) in [
            (
                "PLAYER_GAME_RESULT_VICTORY",
                "playergameresult",
                rules.victory,
            ),
            (
                "PLAYER_GAME_RESULT_DEFEAT",
                "playergameresult",
                rules.defeat,
            ),
            ("PLAYER_GAME_RESULT_TIE", "playergameresult", rules.tie),
            (
                "PLAYER_GAME_RESULT_NEUTRAL",
                "playergameresult",
                rules.neutral,
            ),
            (
                "PLAYER_SLOT_STATE_PLAYING",
                "playerslotstate",
                rules.playing,
            ),
            ("PLAYER_SLOT_STATE_LEFT", "playerslotstate", rules.left),
            ("EVENT_PLAYER_DEFEAT", "playerevent", rules.defeat_event),
            ("EVENT_PLAYER_VICTORY", "playerevent", rules.victory_event),
        ] {
            if self.source_enumerand(REMOVE, global, kind)? != expected {
                return Err(self.inner.error(
                    REMOVE,
                    Failure::PlayerState("selected script/native removal rules disagree"),
                ));
            }
        }
        let symbolic = |this: &mut Self, value| match this
            .inner
            .convert_for("ConvertPlayerSlotState", &[Value::Int(value)])?
        {
            Value::Handle(id) => Ok(id),
            _ => Err(this.inner.error(REMOVE, Failure::Signature)),
        };
        let playing = symbolic(&mut self, rules.playing)?;
        let left = symbolic(&mut self, rules.left)?;
        self.player_removal = Some(PlayerRemovalState {
            budget,
            rules,
            records: Vec::new(),
            deliveries: 0,
            playing,
            left,
        });
        Ok(self)
    }
    pub fn player_removal_state(&self) -> Option<&PlayerRemovalState> {
        self.player_removal.as_ref()
    }
    pub(super) fn player_removal_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if name != REMOVE {
            return None;
        }
        self.player_removal.as_ref()?;
        Some(self.remove_player(args))
    }
    fn remove_player(&mut self, args: &[Value]) -> Result<Value, Error> {
        let fail = |failure| self.inner.error(REMOVE, failure);
        if let [Value::Null, Value::Handle(_) | Value::Null] = args {
            return Ok(Value::Null);
        }
        let [Value::Handle(player), Value::Handle(result)] = args else {
            return Err(fail(Failure::PlayerArguments));
        };
        let row = self.player_settings(*player).map_err(|e| fail(e.failure))?;
        let state = self.player_removal.as_ref().expect("admitted removal");
        let value = self
            .inner
            .symbolic_value(*result, "playergameresult")
            .map_err(|e| fail(e.failure))?;
        if ![
            state.rules.victory,
            state.rules.defeat,
            state.rules.tie,
            state.rules.neutral,
        ]
        .contains(&value)
        {
            return Err(fail(Failure::OutsideDomain(value)));
        }
        if state.get(*player).is_some() {
            return Ok(Value::Null);
        }
        let slot = row
            .slot_state
            .ok_or_else(|| fail(Failure::PlayerState("current player slot is not admitted")))?;
        if slot != state.playing {
            return Ok(Value::Null);
        }
        if state.records.len() >= state.budget.max_players {
            return Err(fail(Failure::Budget("player removal records")));
        }
        let event = if value == state.rules.victory {
            Some(state.rules.victory_event)
        } else if value == state.rules.defeat {
            Some(state.rules.defeat_event)
        } else {
            None
        };
        let mut fired = Vec::new();
        for entry in &self.inner.arena {
            if let Entry::RegisteredPlayerEvent(record) = entry
                && record.player == *player
                && Some(record.enumerand) == event
            {
                if state
                    .deliveries
                    .checked_add(fired.len())
                    .and_then(|n| n.checked_add(1))
                    .is_none_or(|n| n > state.budget.max_event_deliveries)
                {
                    return Err(fail(Failure::Budget("player removal event deliveries")));
                }
                fired.try_reserve(1).map_err(|_| fail(Failure::Capacity))?;
                fired.push((record.trigger, record.playerevent));
            }
        }
        let deliveries = state
            .deliveries
            .checked_add(fired.len())
            .filter(|n| *n <= state.budget.max_event_deliveries)
            .ok_or_else(|| fail(Failure::Budget("player removal event deliveries")))?;
        let left = value != state.rules.victory;
        let left_state = state.left;
        self.reserve_event_delivery(fired.len())?;
        self.player_removal
            .as_mut()
            .expect("admitted removal")
            .records
            .try_reserve(1)
            .map_err(|_| self.inner.error(REMOVE, Failure::Capacity))?;
        if left {
            self.mark_removed_player_slot(*player, left_state)?;
        }
        let state = self.player_removal.as_mut().expect("admitted removal");
        state.records.push(PlayerRemoval {
            player: *player,
            game_result: *result,
            left,
        });
        state.deliveries = deliveries;
        // All queue capacity was reserved before publishing the transition.
        for (trigger, event) in fired {
            self.queue_event(
                trigger,
                None,
                EventSubject {
                    player: Some(*player),
                    ..Default::default()
                },
                Some(event),
            )?;
        }
        Ok(Value::Null)
    }
}

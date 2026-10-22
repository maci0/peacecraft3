//! Admitted campaign targeting operands and the bounded automatic-acquisition
//! scorer they configure. Native scorer terms that this host cannot derive are
//! refused instead of being replaced by invented values.
use super::{Error, Failure, ObjectHost};
use crate::campaign_targeting_policy::TargetingPolicy;
use crate::map_admission::ParsedMapContext;
use pcx_script::HandleId;
use std::collections::BTreeMap;
use std::sync::Arc;

const NATIVE: &str = "SetTargetHeroes";

pub(super) struct State {
    policy: TargetingPolicy,
    /// Active campaign AI players and their current target-heroes flag.
    active: BTreeMap<i32, bool>,
}

impl ObjectHost {
    /// Admits the selected targeting operands for this session. The policy is
    /// only accepted for the same parsed map context; no partial state survives
    /// a refusal.
    pub fn admit_campaign_targeting(
        &mut self,
        context: &Arc<ParsedMapContext>,
        policy: TargetingPolicy,
    ) -> Result<(), Error> {
        if self.campaign_targeting.is_some() {
            return Err(self
                .inner
                .error(NATIVE, Failure::ConfigState("already admitted")));
        }
        if !self
            .player_context()
            .is_some_and(|current| Arc::ptr_eq(current, context))
        {
            return Err(self
                .inner
                .error(NATIVE, Failure::ConfigState("map context differs")));
        }
        self.campaign_targeting = Some(State {
            policy,
            active: BTreeMap::new(),
        });
        Ok(())
    }

    pub fn campaign_targeting_policy(&self) -> Option<&TargetingPolicy> {
        self.campaign_targeting.as_ref().map(|state| &state.policy)
    }

    /// `None` when targeting operands are not admitted or the owner is not an
    /// active campaign AI player. `Some(false)` is the source default for an
    /// active AI player that has not called `SetTargetHeroes(true)`.
    pub fn campaign_targeting_enabled(&self, owner: HandleId) -> Result<Option<bool>, Error> {
        let Some(state) = &self.campaign_targeting else {
            return Ok(None);
        };
        let number = self.player_settings(owner)?.number;
        Ok(state.active.get(&number).copied())
    }

    /// Registers an active campaign AI player. Returns whether this call created
    /// the context; an already-active player keeps its current flag. Targeting
    /// admission is independent, so sessions without operands keep old behavior.
    pub(crate) fn activate_campaign_ai_player(&mut self, player: i32) -> Result<bool, Error> {
        let Some(state) = &mut self.campaign_targeting else {
            return Ok(false);
        };
        if state.active.contains_key(&player) {
            return Ok(false);
        }
        state
            .active
            .insert(player, state.policy.target_heroes_default);
        Ok(true)
    }

    pub(crate) fn deactivate_campaign_ai_player(&mut self, player: i32) {
        if let Some(state) = &mut self.campaign_targeting {
            state.active.remove(&player);
        }
    }

    pub(crate) fn set_ai_target_heroes(&mut self, player: i32, enabled: bool) -> Result<(), Error> {
        let Some(state) = &mut self.campaign_targeting else {
            return Err(self
                .inner
                .error(NATIVE, Failure::ConfigState("operands are not admitted")));
        };
        let Some(current) = state.active.get_mut(&player) else {
            return Err(self
                .inner
                .error(NATIVE, Failure::ConfigState("AI player is not active")));
        };
        *current = enabled;
        Ok(())
    }

    /// The admitted target-heroes flag for `actor`, or `None` when no active
    /// campaign AI context applies and the caller keeps its prior nearest rule.
    pub(super) fn campaign_target_heroes(&self, actor: HandleId) -> Option<bool> {
        let state = self.campaign_targeting.as_ref()?;
        let owner = self.executed_unit(actor).ok()?.owner;
        let player = self.player_settings(owner).ok()?.number;
        state.active.get(&player).copied()
    }

    /// Automatic acquisition for an idle weapon scan. `Err` names a native
    /// scorer term this host cannot derive; the caller then leaves any previous
    /// target untouched.
    pub(super) fn campaign_acquisition<I>(
        &self,
        target_heroes: bool,
        candidates: I,
    ) -> Result<Option<HandleId>, &'static str>
    where
        I: IntoIterator<Item = (HandleId, f32)>,
    {
        let state = self
            .campaign_targeting
            .as_ref()
            .ok_or("targeting policy is not admitted")?;
        self.score_campaign_candidates(state, target_heroes, candidates)
    }

    fn score_campaign_candidates<I>(
        &self,
        state: &State,
        target_heroes: bool,
        candidates: I,
    ) -> Result<Option<HandleId>, &'static str>
    where
        I: IntoIterator<Item = (HandleId, f32)>,
    {
        let mut candidates = candidates.into_iter().peekable();
        if candidates.peek().is_none() {
            return Ok(None);
        }

        let policy = &state.policy;
        let difficulty = self
            .game_difficulty
            .and_then(|handle| self.inner.symbolic_value(handle, "gamedifficulty").ok())
            .ok_or("game difficulty is not admitted")?;
        if difficulty >= i32::from(policy.optional_damage_gate)
            || difficulty >= i32::from(policy.optional_level_gate)
        {
            return Err("optional scorer bands are enabled");
        }

        let aggressive = self.combat.as_ref().and_then(|r| r.neutral_aggressive);
        let mut best: Option<(u32, f32, u64, HandleId)> = None;
        let mut heroes = 0usize;
        for (candidate, gap) in candidates {
            let record = self
                .executed_unit(candidate)
                .map_err(|_| "candidate is not an executed unit")?;
            let rawcode = record.definition.rawcode;
            if matches!(&rawcode, b"pso1" | b"pso2" | b"pso3") {
                return Err("candidate is an unclassified special object");
            }
            let owner = self
                .player_settings(record.owner)
                .map_err(|_| "candidate owner is not admitted")?
                .number;
            if aggressive == Some(owner) {
                return Err("candidate is neutral aggressive");
            }
            if self.unit_has_effect_history(candidate) {
                return Err("candidate has effect history");
            }

            let hero = policy.hero_first <= rawcode[0] && rawcode[0] <= policy.hero_last;
            let hero_band = difficulty >= i32::from(policy.hero_min_game_difficulty);
            let mut key = 0;
            if hero && target_heroes && hero_band {
                key |= policy.hero_priority_mask;
                heroes += 1;
            }
            let token = candidate.token();
            let replace = match best {
                None => true,
                Some((best_key, best_gap, best_token, _)) => {
                    key > best_key
                        || (key == best_key
                            && (gap < best_gap || (gap == best_gap && token > best_token)))
                }
            };
            if replace {
                best = Some((key, gap, token, candidate));
            }
        }
        if heroes > 1 {
            return Err("multiple hero candidates need unmodelled lower bands");
        }
        Ok(best.map(|(_, _, _, id)| id))
    }

    #[cfg(test)]
    pub(crate) fn test_clear_combat_target(&mut self, unit: HandleId) {
        if let Ok(super::Entry::Unit { record }) = self.inner.entry_mut(unit) {
            record.activity = None;
            if let Some(combat) = record.combat.as_mut() {
                combat.target = None;
                combat.attack_destination = None;
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn test_set_game_difficulty(&mut self, ordinal: i32) {
        let handle = self
            .inner
            .allocate(
                super::Entry::Symbolic {
                    kind: "gamedifficulty".into(),
                    enumerand: ordinal,
                },
                "GetGameDifficulty",
            )
            .expect("difficulty handle");
        self.game_difficulty = Some(handle);
    }
}

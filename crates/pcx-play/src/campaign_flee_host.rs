//! Canonical per-player campaign AI flags shared by targeting and flee policy.
//! Admission only stores source operands; it never invents a flag for an
//! unadmitted policy or mints a player identity.
use super::{Error, Failure, ObjectHost};
use crate::campaign_flee_policy::FleePolicy;
use crate::map_admission::ParsedMapContext;
use pcx_script::HandleId;
use std::collections::BTreeMap;
use std::sync::Arc;

const NATIVE: &str = "SetUnitsFlee";
const TARGETING_NATIVE: &str = "SetTargetHeroes";
const START_CAMPAIGN_AI: &str = "StartCampaignAI";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct PlayerFlags {
    target_heroes: Option<bool>,
    units_flee: Option<bool>,
    gathering: Option<bool>,
    gathering_unit: Option<HandleId>,
}

#[derive(Default)]
pub(super) struct Registry {
    players: BTreeMap<i32, PlayerFlags>,
}

impl Registry {
    pub(super) fn activate(
        &mut self,
        player: i32,
        target_heroes: Option<bool>,
        units_flee: Option<bool>,
    ) -> bool {
        if self.players.contains_key(&player) {
            return false;
        }
        self.players.insert(
            player,
            PlayerFlags {
                target_heroes,
                units_flee,
                gathering: None,
                gathering_unit: None,
            },
        );
        true
    }

    pub(super) fn deactivate(&mut self, player: i32) {
        self.players.remove(&player);
    }

    pub(super) fn target_heroes(&self, player: i32) -> Option<bool> {
        self.players
            .get(&player)
            .and_then(|flags| flags.target_heroes)
    }

    pub(super) fn units_flee(&self, player: i32) -> Option<bool> {
        self.players.get(&player).and_then(|flags| flags.units_flee)
    }

    pub(super) fn gathering(&self, player: i32) -> Option<(Option<bool>, Option<HandleId>)> {
        self.players
            .get(&player)
            .map(|flags| (flags.gathering, flags.gathering_unit))
    }

    pub(super) fn stop_gathering(&mut self, player: i32, unit: Option<HandleId>) -> bool {
        let Some(flags) = self.players.get_mut(&player) else {
            return false;
        };
        flags.gathering = Some(false);
        flags.gathering_unit = unit;
        true
    }

    pub(super) fn set_target_heroes(&mut self, player: i32, enabled: bool) -> bool {
        let Some(flags) = self.players.get_mut(&player) else {
            return false;
        };
        flags.target_heroes = Some(enabled);
        true
    }

    pub(super) fn set_units_flee(&mut self, player: i32, enabled: bool) -> bool {
        let Some(flags) = self.players.get_mut(&player) else {
            return false;
        };
        flags.units_flee = Some(enabled);
        true
    }

    pub(super) fn admit_target_heroes_default(&mut self, default: bool) {
        for flags in self.players.values_mut() {
            if flags.target_heroes.is_none() {
                flags.target_heroes = Some(default);
            }
        }
    }

    pub(super) fn admit_units_flee_default(&mut self, default: bool) {
        for flags in self.players.values_mut() {
            if flags.units_flee.is_none() {
                flags.units_flee = Some(default);
            }
        }
    }
}

impl ObjectHost {
    /// Admits the selected flee operands for this session. The policy is only
    /// accepted for the same parsed map context; a refusal leaves all admitted
    /// state unchanged.
    pub fn admit_campaign_flee(
        &mut self,
        context: &Arc<ParsedMapContext>,
        policy: FleePolicy,
    ) -> Result<(), Error> {
        if self.campaign_flee.is_some() {
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
        self.campaign_ai
            .admit_units_flee_default(policy.units_flee_default);
        self.campaign_flee = Some(policy);
        Ok(())
    }

    pub fn campaign_flee_policy(&self) -> Option<&FleePolicy> {
        self.campaign_flee.as_ref()
    }

    /// `None` when flee operands are not admitted, the owner has no active
    /// campaign AI context, or that player is not active. An invalid owner
    /// handle is an `Err`, not `None`.
    pub fn campaign_flee_enabled(&self, owner: HandleId) -> Result<Option<bool>, Error> {
        if self.campaign_flee.is_none() {
            return Ok(None);
        }
        let number = self.player_settings(owner)?.number;
        Ok(self.campaign_ai.units_flee(number))
    }

    /// Registers an active campaign AI player using the defaults of every
    /// admitted policy. Returns whether this call created the context; an
    /// already-active player keeps its current flags.
    pub(crate) fn activate_campaign_ai_player(&mut self, player: i32) -> Result<bool, Error> {
        let target_heroes = self
            .campaign_targeting
            .as_ref()
            .map(|state| state.target_heroes_default());
        let units_flee = self
            .campaign_flee
            .as_ref()
            .map(|policy| policy.units_flee_default);
        if self.player_identity(player).is_none() {
            return Err(self.inner.error(
                START_CAMPAIGN_AI,
                Failure::ConfigState("player is not admitted"),
            ));
        }
        if target_heroes.is_none() && units_flee.is_none() && self.campaign_gathering.is_none() {
            return Ok(false);
        }
        Ok(self.campaign_ai.activate(player, target_heroes, units_flee))
    }

    pub(crate) fn deactivate_campaign_ai_player(&mut self, player: i32) {
        self.campaign_ai.deactivate(player);
    }

    pub(crate) fn set_ai_target_heroes(&mut self, player: i32, enabled: bool) -> Result<(), Error> {
        if self.campaign_targeting.is_none() {
            return Err(self.inner.error(
                TARGETING_NATIVE,
                Failure::ConfigState("operands are not admitted"),
            ));
        }
        self.require_active_campaign_ai_player(player, TARGETING_NATIVE)?;
        self.campaign_ai.set_target_heroes(player, enabled);
        Ok(())
    }

    pub(crate) fn set_ai_units_flee(&mut self, player: i32, enabled: bool) -> Result<(), Error> {
        if self.campaign_flee.is_none() {
            return Err(self
                .inner
                .error(NATIVE, Failure::ConfigState("operands are not admitted")));
        }
        if self.player_identity(player).is_none() {
            return Err(self
                .inner
                .error(NATIVE, Failure::ConfigState("player is not admitted")));
        }
        self.require_active_campaign_ai_player(player, NATIVE)?;
        self.campaign_ai.set_units_flee(player, enabled);
        Ok(())
    }

    fn require_active_campaign_ai_player(
        &self,
        player: i32,
        native: &'static str,
    ) -> Result<(), Error> {
        if !self.campaign_ai.players.contains_key(&player) {
            return Err(self
                .inner
                .error(native, Failure::ConfigState("AI player is not active")));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Registry;

    #[test]
    fn players_keep_independent_target_heroes_and_flee_flags() {
        let mut registry = Registry::default();
        assert!(registry.activate(0, Some(true), Some(false)));
        assert!(registry.activate(1, Some(false), Some(true)));
        assert_eq!(registry.target_heroes(0), Some(true));
        assert_eq!(registry.units_flee(0), Some(false));
        assert_eq!(registry.target_heroes(1), Some(false));
        assert_eq!(registry.units_flee(1), Some(true));

        assert!(registry.set_target_heroes(0, false));
        assert!(registry.set_units_flee(1, false));
        assert_eq!(registry.target_heroes(0), Some(false));
        assert_eq!(registry.units_flee(0), Some(false));
        assert_eq!(registry.target_heroes(1), Some(false));
        assert_eq!(registry.units_flee(1), Some(false));
    }

    #[test]
    fn unadmitted_policy_fields_remain_none_until_admission_backfills_them() {
        let mut registry = Registry::default();
        assert!(registry.activate(0, None, Some(true)));
        assert_eq!(registry.target_heroes(0), None);
        assert_eq!(registry.units_flee(0), Some(true));

        registry.admit_target_heroes_default(false);
        assert_eq!(registry.target_heroes(0), Some(false));
        assert_eq!(registry.units_flee(0), Some(true));

        assert!(registry.activate(1, None, None));
        assert_eq!(registry.target_heroes(1), None);
        assert_eq!(registry.units_flee(1), None);

        registry.admit_units_flee_default(false);
        assert_eq!(registry.units_flee(0), Some(true));
        assert_eq!(registry.units_flee(1), Some(false));
        assert_eq!(registry.target_heroes(1), None);
    }

    #[test]
    fn repeated_activation_keeps_first_flags_and_deactivation_forgets_them() {
        let mut registry = Registry::default();
        assert!(registry.activate(0, Some(false), Some(false)));
        assert!(registry.set_target_heroes(0, true));
        assert!(registry.set_units_flee(0, true));
        assert!(!registry.activate(0, Some(false), Some(false)));
        assert_eq!(registry.target_heroes(0), Some(true));
        assert_eq!(registry.units_flee(0), Some(true));

        registry.deactivate(0);
        assert_eq!(registry.target_heroes(0), None);
        assert_eq!(registry.units_flee(0), None);
        registry.deactivate(0);
        assert!(registry.activate(0, Some(false), Some(false)));
        assert_eq!(registry.target_heroes(0), Some(false));
        assert_eq!(registry.units_flee(0), Some(false));
    }

    #[test]
    fn missing_player_refuses_setters_without_creating_a_record() {
        let mut registry = Registry::default();
        assert!(!registry.set_target_heroes(7, true));
        assert!(!registry.set_units_flee(7, true));
        assert_eq!(registry.target_heroes(7), None);
        assert_eq!(registry.units_flee(7), None);
        assert!(registry.activate(7, Some(true), Some(true)));
        assert!(registry.set_target_heroes(7, false));
        assert!(registry.set_units_flee(7, false));
        assert_eq!(registry.target_heroes(7), Some(false));
        assert_eq!(registry.units_flee(7), Some(false));
    }
}

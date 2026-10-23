//! Campaign context configuration and queries over canonical unit ownership.
use super::{Error, Failure, ObjectHost};
use crate::{campaign_mode_policy::CampaignModePolicy, map_admission::ParsedMapContext};
use pcx_script::HandleId;
use std::sync::Arc;

const NATIVE: &str = "SetCampaignAI";

impl ObjectHost {
    pub fn admit_campaign_mode(
        &mut self,
        context: &Arc<ParsedMapContext>,
        policy: CampaignModePolicy,
    ) -> Result<(), Error> {
        if self.campaign_mode.is_some() {
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
        if !policy.context_mask.is_power_of_two()
            || !policy.unit_mask.is_power_of_two()
            || policy.unit_mask >= 0x80
            || policy.replacement_count_field == 0
            || !policy.replacement_count_field.is_multiple_of(4)
        {
            return Err(self.inner.error(
                NATIVE,
                Failure::ConfigState("mode masks or replacement field are invalid"),
            ));
        }
        for image in [
            self.campaign_flee.as_ref().map(|p| p.image_sha256),
            self.campaign_captains.as_ref().map(|p| p.image_sha256),
            self.campaign_group_timed_life
                .as_ref()
                .map(|p| p.image_sha256),
        ] {
            if image.is_some_and(|image| image != policy.image_sha256) {
                return Err(self
                    .inner
                    .error(NATIVE, Failure::ConfigState("source image differs")));
            }
        }
        self.campaign_mode = Some(policy);
        Ok(())
    }

    pub fn campaign_mode_policy(&self) -> Option<&CampaignModePolicy> {
        self.campaign_mode.as_ref()
    }

    pub fn campaign_ai_mode(&self, owner: HandleId) -> Result<Option<bool>, Error> {
        if self.campaign_mode.is_none() {
            return Ok(None);
        }
        let player = self.player_settings(owner)?.number;
        Ok(self.campaign_ai.campaign_mode(player))
    }

    pub fn campaign_unit_mode(&self, unit: HandleId) -> Result<Option<bool>, Error> {
        let record = self.executed_unit(unit)?;
        let mode = self.campaign_ai_mode(record.owner)?;
        if mode.is_some()
            && (record.cached || record.summoned || self.unit_has_effect_history(unit))
        {
            return Err(self.inner.error(
                NATIVE,
                Failure::UnitActivity("unit attachment state is unqualified".into()),
            ));
        }
        // shortcut: canonical direct-created ownership supplies mode attachment; qualify native manager exclusions before widening it.
        Ok(mode)
    }

    pub(crate) fn set_ai_campaign_mode(&mut self, player: i32) -> Result<(), Error> {
        let enabled = self
            .campaign_mode
            .as_ref()
            .ok_or_else(|| {
                self.inner
                    .error(NATIVE, Failure::ConfigState("operands are not admitted"))
            })?
            .campaign_mode;
        self.require_active_campaign_ai_player(player, NATIVE)?;
        self.campaign_ai.set_campaign_mode(player, enabled);
        Ok(())
    }

    /// The explicit script value for the active owner, unknown before its first setter.
    // shortcut: FillGuardPosts still refuses; consume this threshold when canonical guard-post replenishment is admitted.
    pub fn campaign_replacement_count(&self, owner: HandleId) -> Result<Option<i32>, Error> {
        let player = self.player_settings(owner)?.number;
        Ok(self.campaign_ai.replacement_count(player))
    }

    pub(crate) fn set_ai_replacement_count(
        &mut self,
        player: i32,
        count: i32,
    ) -> Result<(), Error> {
        const SETTER: &str = "SetReplacementCount";
        if self.campaign_mode.is_none() {
            return Err(self.inner.error(
                SETTER,
                Failure::ConfigState("campaign context operands are not admitted"),
            ));
        }
        self.require_active_campaign_ai_player(player, SETTER)?;
        self.campaign_ai.set_replacement_count(player, count);
        Ok(())
    }

    pub(crate) fn ai_unit_count(&self, player: i32, unit: i32, done: bool) -> Result<i32, Error> {
        let name = if done {
            "GetUnitCountDone"
        } else {
            "GetUnitCount"
        };
        self.require_active_campaign_ai_player(player, name)?;
        let owner = self.player_identity(player).ok_or_else(|| {
            self.inner
                .error(name, Failure::ConfigState("player is not admitted"))
        })?;
        self.count_completed_units(owner, unit, name, false)
    }

    pub(crate) fn ai_ignored_units(&self, player: i32, unit: i32) -> Result<i32, Error> {
        const NAME: &str = "IgnoredUnits";
        self.require_active_campaign_ai_player(player, NAME)?;
        let owner = self.player_identity(player).ok_or_else(|| {
            self.inner
                .error(NAME, Failure::ConfigState("player is not admitted"))
        })?;
        self.count_completed_units(owner, unit, NAME, true)
    }

    pub(crate) fn ai_player_unit_type_count(
        &self,
        owner: Option<HandleId>,
        unit: i32,
    ) -> Result<i32, Error> {
        match owner {
            None => Ok(0),
            Some(owner) => self.count_completed_units(owner, unit, "GetPlayerUnitTypeCount", false),
        }
    }

    fn count_completed_units(
        &self,
        owner: HandleId,
        unit: i32,
        name: &'static str,
        ignored: bool,
    ) -> Result<i32, Error> {
        self.player_settings(owner)
            .map_err(|e| self.inner.error(name, e.failure))?;
        self.unit_declaration()
            .map_err(|e| self.inner.error(name, e.failure))?;
        // shortcut: all accepted unit factories create completed units; add canonical pending work when training/construction is admitted.
        self.executed_units()
            .filter(|(_, record)| {
                record.owner == owner && record.definition.rawcode == unit.to_be_bytes()
            })
            .try_fold(0i32, |count, (id, record)| {
                if record.combat.is_none() && record.vitals.is_none() {
                    return Err(self
                        .inner
                        .error(name, Failure::ConfigState("unit liveness is not admitted")));
                }
                if record.vitals.as_ref().is_some_and(|v| !v.life.is_finite()) {
                    return Err(self
                        .inner
                        .error(name, Failure::ConfigState("unit life is not finite")));
                }
                if record.combat.as_ref().is_some_and(|c| c.dead)
                    || record.vitals.as_ref().is_some_and(|v| v.life <= 0.)
                {
                    return Ok(count);
                }
                // shortcut: use canonical guard/captain membership; add autonomous worker associations when that controller is supported.
                if ignored
                    && (self
                        .guard_state()
                        .and_then(|s| s.get(id))
                        .is_some_and(|g| g.owner == owner && !g.released)
                        || self.campaign_ai.captain_member_owner(id).is_some())
                {
                    return Ok(count);
                }
                count.checked_add(1).ok_or_else(|| {
                    self.inner.error(
                        name,
                        Failure::ConfigState("unit count exceeds integer range"),
                    )
                })
            })
    }
}

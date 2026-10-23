//! Canonical captain association for the selected timed-life factory.
use super::{Error, Failure, ObjectHost};
use crate::campaign_group_timed_life_policy::GroupTimedLifePolicy;
use crate::map_admission::ParsedMapContext;
use pcx_script::HandleId;
use std::sync::Arc;

const SET: &str = "GroupTimedLife";
const APPLY: &str = "UnitApplyTimedLife";

impl ObjectHost {
    pub fn admit_campaign_group_timed_life(
        &mut self,
        context: &Arc<ParsedMapContext>,
        policy: GroupTimedLifePolicy,
    ) -> Result<(), Error> {
        if self.campaign_group_timed_life.is_some() {
            return Err(self
                .inner
                .error(SET, Failure::ConfigState("already admitted")));
        }
        if !self
            .player_context()
            .is_some_and(|current| Arc::ptr_eq(current, context))
        {
            return Err(self
                .inner
                .error(SET, Failure::ConfigState("map context differs")));
        }
        if !policy.context_mask.is_power_of_two() {
            return Err(self
                .inner
                .error(SET, Failure::ConfigState("context mask is not one bit")));
        }
        for image in [
            self.campaign_captains.as_ref().map(|p| p.image_sha256),
            self.campaign_gathering.as_ref().map(|p| p.image_sha256),
            self.campaign_flee.as_ref().map(|p| p.image_sha256),
        ] {
            if image.is_some_and(|image| image != policy.image_sha256) {
                return Err(self
                    .inner
                    .error(SET, Failure::ConfigState("source image differs")));
            }
        }
        if let Some(native) = self.inner.program.natives().get(APPLY) {
            Self::contract(
                &self.inner,
                APPLY,
                &["unit", "integer", "real"],
                "nothing",
                false,
                Some("unit"),
            )?;
            let create = self
                .inner
                .program
                .natives()
                .get("CreateUnit")
                .ok_or_else(|| {
                    self.inner
                        .error(SET, Failure::ConfigState("unit source is not admitted"))
                })?;
            if native.source != create.source {
                return Err(self.inner.error(APPLY, Failure::Source));
            }
        }
        self.campaign_group_timed_life = Some(policy);
        Ok(())
    }

    pub fn campaign_group_timed_life_policy(&self) -> Option<&GroupTimedLifePolicy> {
        self.campaign_group_timed_life.as_ref()
    }

    pub fn campaign_group_timed_life_enabled(
        &self,
        owner: HandleId,
    ) -> Result<Option<bool>, Error> {
        if self.campaign_group_timed_life.is_none() {
            return Ok(None);
        }
        let player = self.player_settings(owner)?.number;
        Ok(self.campaign_ai.group_timed_life(player))
    }

    pub(crate) fn set_ai_group_timed_life(
        &mut self,
        player: i32,
        enabled: bool,
    ) -> Result<(), Error> {
        if self.campaign_group_timed_life.is_none() {
            return Err(self
                .inner
                .error(SET, Failure::ConfigState("operands are not admitted")));
        }
        if self.player_identity(player).is_none() {
            return Err(self
                .inner
                .error(SET, Failure::ConfigState("player is not admitted")));
        }
        self.require_active_campaign_ai_player(player, SET)?;
        self.campaign_ai.set_group_timed_life(player, enabled);
        Ok(())
    }

    pub(crate) fn captain_route_owned(&self, unit: HandleId) -> bool {
        let Some(player) = self.campaign_ai.captain_member_owner(unit) else {
            return false;
        };
        let Some((_, [Some(home), _])) = self.campaign_ai.captains(player) else {
            return false;
        };
        self.campaign_ai.has_captain_route(player, unit)
            && self
                .executed_unit(unit)
                .ok()
                .and_then(|record| record.activity.as_ref())
                .and_then(|activity| activity.destination())
                == Some(home)
    }

    pub(super) fn forget_captain_member(&mut self, unit: HandleId) {
        self.campaign_ai.detach_captain_member(unit);
    }

    pub(super) fn apply_qualified_timed_life(
        &mut self,
        unit: HandleId,
        key: [u8; 4],
        duration: f32,
    ) -> Result<(), Error> {
        let policy = self.campaign_group_timed_life.as_ref().ok_or_else(|| {
            self.inner
                .error(APPLY, Failure::ConfigState("factory is not admitted"))
        })?;
        if key != policy.timed_life_factory {
            return Err(self.inner.error(
                APPLY,
                Failure::UnitActivity("timed-life factory is unsupported".into()),
            ));
        }
        let now = self.timer_game_time().ok_or_else(|| {
            self.inner
                .error(APPLY, Failure::UnitActivity("clock is not admitted".into()))
        })?;
        if !duration.is_finite() || duration <= 0. || !now.is_finite() || now < 0. {
            return Err(self.inner.error(
                APPLY,
                Failure::UnitActivity("positive finite duration and clock required".into()),
            ));
        }
        let deadline = now + f64::from(duration);
        if !deadline.is_finite() || deadline <= now || self.unit_expirations.contains_key(&unit) {
            return Err(self.inner.error(
                APPLY,
                Failure::UnitActivity("timed-life deadline is invalid or already set".into()),
            ));
        }
        if self.unit_expirations.len()
            >= self
                .admitted_units()
                .map_err(|error| self.inner.error(APPLY, error.failure))?
                .max_records
        {
            return Err(self
                .inner
                .error(APPLY, Failure::Budget("unit expiration records")));
        }
        let record = self
            .executed_unit(unit)
            .map_err(|error| self.inner.error(APPLY, error.failure))?;
        if !record.shown || record.combat.as_ref().is_none_or(|combat| combat.dead) {
            return Err(self.inner.error(
                APPLY,
                Failure::UnitActivity("live visible unit required".into()),
            ));
        }
        let player = self
            .player_settings(record.owner)
            .map_err(|error| self.inner.error(APPLY, error.failure))?
            .number;
        let active = self.campaign_ai.is_active(player);
        let enabled = if active {
            Some(self.campaign_ai.group_timed_life(player).ok_or_else(|| {
                self.inner
                    .error(APPLY, Failure::ConfigState("active AI flag is unset"))
            })?)
        } else {
            None
        };
        let prepared = if let Some(enabled) = enabled {
            if record.summoned
                || record.cached
                || record.admitted_order.is_some()
                || self.unit_has_effect_history(unit)
            {
                return Err(self.inner.error(
                    APPLY,
                    Failure::UnitActivity("unit creation or effect state is unqualified".into()),
                ));
            }
            let linked = self.campaign_ai.captain_member_owner(unit);
            if linked.is_some_and(|owner| owner != player) {
                return Err(self.inner.error(
                    APPLY,
                    Failure::UnitActivity("foreign captain association".into()),
                ));
            }
            let destination = if enabled {
                // shortcut: ordinary members use the declared first home; qualify native formation before modeling it.
                let Some((true, [Some(home), _])) = self.campaign_ai.captains(player) else {
                    return Err(self
                        .inner
                        .error(APPLY, Failure::ConfigState("first captain home is absent")));
                };
                if linked.is_some() {
                    None
                } else {
                    self.campaign_ai
                        .reserve_captain_member(player)
                        .map_err(|_| self.inner.error(APPLY, Failure::Capacity))?;
                    Some(home)
                }
            } else {
                None
            };
            if enabled && linked.is_some() {
                None
            } else {
                let prepared = self
                    .prepare_steer_unit_in_range(unit, destination, 0., APPLY)?
                    .ok_or_else(|| {
                        self.inner.error(
                            APPLY,
                            Failure::UnitActivity("unit cannot take the captain activity".into()),
                        )
                    })?;
                self.reserve_steer_commits(1, APPLY)?;
                Some(prepared)
            }
        } else {
            None
        };
        if let Some(prepared) = prepared {
            self.commit_steer_unit(unit, prepared, APPLY)?;
            self.clear_combat_target(unit);
            if enabled == Some(true) {
                self.campaign_ai.attach_captain_member(player, unit);
                self.campaign_ai.mark_captain_route(player, unit);
            } else {
                self.campaign_ai.detach_captain_member(unit);
            }
        }
        self.unit_expirations.insert(unit, deadline);
        Ok(())
    }
}

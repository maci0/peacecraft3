//! Per-player captain home targets in the canonical campaign AI context.
use super::{Entry, Error, Failure, ObjectHost};
use crate::campaign_captain_policy::CaptainPolicy;
use crate::map_admission::ParsedMapContext;
use pcx_script::HandleId;
use std::sync::Arc;

const CREATE: &str = "CreateCaptains";
const SET_HOME: &str = "SetCaptainHome";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CaptainObservation {
    pub created: bool,
    pub homes: [Option<[f32; 2]>; 2],
}

impl ObjectHost {
    pub fn campaign_captain_in_combat(
        &self,
        owner: HandleId,
        attack: bool,
    ) -> Result<Option<bool>, Error> {
        const NAME: &str = "CaptainInCombat";
        if self.campaign_captains.is_none() {
            return Ok(None);
        }
        let player = self.player_settings(owner)?.number;
        let Some((created, homes)) = self.campaign_ai.captains(player) else {
            return Ok(None);
        };
        if !created || homes[usize::from(!attack)].is_none() {
            return Err(self.inner.error(
                NAME,
                Failure::ConfigState("selected captain is not created"),
            ));
        }
        let focus = self
            .campaign_ai
            .captain_focus(player, attack)
            .expect("active player row");
        match focus.and_then(|target| self.inner.entry(target).ok()) {
            Some(Entry::Unit { .. }) => Ok(Some(true)),
            Some(Entry::RemovedUnit { .. }) | None => Ok(Some(false)),
            Some(_) => Err(self.inner.error(
                NAME,
                Failure::ConfigState("captain target class is not qualified"),
            )),
        }
    }

    pub(super) fn refresh_captain_combat(&mut self) {
        let inner = &self.inner;
        // shortcut: publish canonical member targets at the house combat step;
        // qualify native controller cadence and target priority before timing parity.
        self.campaign_ai.refresh_captain_focus(|player, unit| {
            let Entry::Unit { record } = inner.entry(unit).ok()? else { return None; };
            if !matches!(inner.entry(record.owner).ok(), Some(Entry::Player { number }) if *number == player) {
                return None;
            }
            let combat = record.combat.as_ref()?;
            if combat.dead {
                return None;
            }
            let target = combat.target?;
            match inner.entry(target).ok()? {
                Entry::Unit { record } if record.combat.as_ref().is_none_or(|combat| !combat.dead)
                    && record.vitals.is_none_or(|vitals| vitals.life > 0.) => Some(target),
                Entry::Destructable { state, .. } if !state.dead && state.life.is_none_or(|life| life > 0.) => Some(target),
                _ => None,
            }
        });
    }

    #[cfg(test)]
    pub(crate) fn fixture_captain_member(&mut self, player: i32, attack: bool, unit: HandleId) {
        self.campaign_ai
            .reserve_captain_member(player, attack)
            .unwrap();
        self.campaign_ai.attach_captain_member(player, attack, unit);
    }

    pub fn admit_campaign_captains(
        &mut self,
        context: &Arc<ParsedMapContext>,
        policy: CaptainPolicy,
    ) -> Result<(), Error> {
        if self.campaign_captains.is_some() {
            return Err(self
                .inner
                .error(CREATE, Failure::ConfigState("already admitted")));
        }
        if !self
            .player_context()
            .is_some_and(|current| Arc::ptr_eq(current, context))
        {
            return Err(self
                .inner
                .error(CREATE, Failure::ConfigState("map context differs")));
        }
        if policy.first_selector_mask.count_ones() != 1
            || policy.second_selector_mask.count_ones() != 1
            || policy.first_selector_mask == policy.second_selector_mask
        {
            return Err(self.inner.error(
                CREATE,
                Failure::ConfigState("captain selectors must be distinct single bits"),
            ));
        }
        if self
            .campaign_gathering
            .as_ref()
            .is_some_and(|gathering| gathering.image_sha256 != policy.image_sha256)
        {
            return Err(self
                .inner
                .error(CREATE, Failure::ConfigState("source image differs")));
        }
        self.campaign_captains = Some(policy);
        Ok(())
    }

    pub fn campaign_captain_policy(&self) -> Option<&CaptainPolicy> {
        self.campaign_captains.as_ref()
    }

    pub fn campaign_captain_state(
        &self,
        owner: HandleId,
    ) -> Result<Option<CaptainObservation>, Error> {
        if self.campaign_captains.is_none() {
            return Ok(None);
        }
        let player = self.player_settings(owner)?.number;
        Ok(self
            .campaign_ai
            .captains(player)
            .map(|(created, homes)| CaptainObservation { created, homes }))
    }

    pub fn campaign_captain_group_size(&self, owner: HandleId) -> Result<Option<i32>, Error> {
        if self.campaign_captains.is_none() || self.campaign_group_timed_life.is_none() {
            return Ok(None);
        }
        let player = self.player_settings(owner)?.number;
        let Some((created, _)) = self.campaign_ai.captains(player) else {
            return Ok(None);
        };
        if !created {
            return Err(self.inner.error(
                "CaptainGroupSize",
                Failure::ConfigState("captains are not created"),
            ));
        }
        let count = self
            .campaign_ai
            .captain_members(player)
            .expect("active player row")
            .len();
        Ok(Some(i32::try_from(count).map_err(|_| {
            self.inner.error("CaptainGroupSize", Failure::Capacity)
        })?))
    }

    pub fn campaign_captain_is_empty(&self, owner: HandleId) -> Result<Option<bool>, Error> {
        Ok(self
            .campaign_captain_group_size(owner)?
            .map(|count| count == 0))
    }

    pub(crate) fn create_ai_captains(&mut self, player: i32) -> Result<(), Error> {
        let policy = self.campaign_captains.as_ref().ok_or_else(|| {
            self.inner
                .error(CREATE, Failure::ConfigState("operands are not admitted"))
        })?;
        let gathering = self.campaign_gathering.as_ref().ok_or_else(|| {
            self.inner.error(
                CREATE,
                Failure::ConfigState("gathering operands are not admitted"),
            )
        })?;
        if policy.image_sha256 != gathering.image_sha256 {
            return Err(self
                .inner
                .error(CREATE, Failure::ConfigState("source image differs")));
        }
        let owner = self.player_identity(player).ok_or_else(|| {
            self.inner
                .error(CREATE, Failure::ConfigState("player is not admitted"))
        })?;
        let Some((created, _)) = self.campaign_ai.captains(player) else {
            return Err(self
                .inner
                .error(CREATE, Failure::ConfigState("AI player is not active")));
        };
        if created {
            return Err(self
                .inner
                .error(CREATE, Failure::ConfigState("captains already created")));
        }
        // shortcut: copy the canonical position into both targets; qualify the
        // native numeric setter before claiming exact corner-case conversion.
        let home = self
            .current_gathering_anchor(owner, CREATE)?
            .map(|(_, position)| position);
        self.campaign_ai.create_captains(player, home);
        Ok(())
    }

    pub(crate) fn set_ai_captain_home(
        &mut self,
        player: i32,
        selector: i32,
        home: [f32; 2],
    ) -> Result<(), Error> {
        let policy = self.campaign_captains.as_ref().ok_or_else(|| {
            self.inner
                .error(SET_HOME, Failure::ConfigState("operands are not admitted"))
        })?;
        if home.iter().any(|value| !value.is_finite()) {
            return Err(self.inner.error(
                SET_HOME,
                Failure::UnitActivity("non-finite captain home".into()),
            ));
        }
        if self.player_identity(player).is_none() {
            return Err(self
                .inner
                .error(SET_HOME, Failure::ConfigState("player is not admitted")));
        }
        let first = (selector as u32 & u32::from(policy.first_selector_mask)) != 0;
        let second = (selector as u32 & u32::from(policy.second_selector_mask)) != 0;
        let members = if first {
            self.campaign_ai.captain_members(player).unwrap_or_default()
        } else {
            &[]
        };
        let mut routes = Vec::new();
        routes
            .try_reserve(members.len())
            .map_err(|_| self.inner.error(SET_HOME, Failure::Capacity))?;
        for &unit in members {
            let record = self.executed_unit(unit)?;
            if record.owner != self.player_identity(player).expect("validated owner")
                || self.live(unit).is_none()
            {
                return Err(self.inner.error(
                    SET_HOME,
                    Failure::ConfigState("captain member is no longer owned and live"),
                ));
            }
            let prepared = self
                .prepare_steer_unit_in_range(unit, Some(home), 0., SET_HOME)?
                .ok_or_else(|| {
                    self.inner.error(
                        SET_HOME,
                        Failure::UnitActivity("captain member cannot route home".into()),
                    )
                })?;
            routes.push((unit, prepared));
        }
        if !routes.is_empty() {
            self.reserve_steer_commits(routes.len(), SET_HOME)?;
            self.campaign_ai
                .reserve_captain_routes(player, routes.len())
                .map_err(|_| self.inner.error(SET_HOME, Failure::Capacity))?;
        }
        if !self
            .campaign_ai
            .set_captain_home(player, first, second, home)
        {
            return Err(self.inner.error(
                SET_HOME,
                Failure::ConfigState("selected captain home is not present"),
            ));
        }
        for (unit, prepared) in routes {
            self.commit_steer_unit(unit, prepared, SET_HOME)?;
            self.campaign_ai.mark_captain_route(player, unit);
        }
        Ok(())
    }
}

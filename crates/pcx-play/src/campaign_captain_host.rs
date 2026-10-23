//! Per-player captain home targets in the canonical campaign AI context.
use super::{Error, Failure, ObjectHost};
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
        if !self
            .campaign_ai
            .set_captain_home(player, first, second, home)
        {
            return Err(self.inner.error(
                SET_HOME,
                Failure::ConfigState("selected captain home is not present"),
            ));
        }
        Ok(())
    }
}

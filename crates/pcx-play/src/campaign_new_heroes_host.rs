//! Canonical per-player `SetNewHeroes` flag. Admission stores only the
//! source-bound policy; activation leaves the flag unset until the native
//! setter runs.
use super::{Error, Failure, ObjectHost};
use crate::campaign_new_heroes_policy::NewHeroesPolicy;
use crate::map_admission::ParsedMapContext;
use pcx_script::HandleId;
use std::sync::Arc;

const NATIVE: &str = "SetNewHeroes";

impl ObjectHost {
    /// Admits the selected new-heroes operands for this session. The policy is
    /// only accepted for the same parsed map context; a refusal leaves all
    /// admitted state unchanged.
    pub fn admit_campaign_new_heroes(
        &mut self,
        context: &Arc<ParsedMapContext>,
        policy: NewHeroesPolicy,
    ) -> Result<(), Error> {
        if self.campaign_new_heroes.is_some() {
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
        if policy.context_mask == 0 || policy.context_mask.count_ones() != 1 {
            return Err(self.inner.error(
                NATIVE,
                Failure::ConfigState("context mask is not a single bit"),
            ));
        }
        self.campaign_new_heroes = Some(policy);
        Ok(())
    }

    pub fn campaign_new_heroes_policy(&self) -> Option<&NewHeroesPolicy> {
        self.campaign_new_heroes.as_ref()
    }

    /// `None` when new-heroes operands are not admitted, the owner has no
    /// active campaign AI context, or that player has not run the setter. An
    /// invalid owner handle is an `Err`, not `None`.
    pub fn campaign_new_heroes_enabled(&self, owner: HandleId) -> Result<Option<bool>, Error> {
        if self.campaign_new_heroes.is_none() {
            return Ok(None);
        }
        let number = self.player_settings(owner)?.number;
        Ok(self.campaign_ai.new_heroes(number))
    }

    pub(crate) fn set_ai_new_heroes(&mut self, player: i32, enabled: bool) -> Result<(), Error> {
        if self.campaign_new_heroes.is_none() {
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
        self.campaign_ai.set_new_heroes(player, enabled);
        Ok(())
    }
}

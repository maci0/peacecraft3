//! Canonical per-player `SetSlowChopping` flag. Admission stores only the
//! source-bound policy; activation leaves the flag unset until the native
//! setter runs.
use super::{Error, Failure, ObjectHost};
use crate::campaign_slow_chopping_policy::SlowChoppingPolicy;
use crate::map_admission::ParsedMapContext;
use pcx_script::HandleId;
use std::sync::Arc;

const NATIVE: &str = "SetSlowChopping";

impl ObjectHost {
    /// Admits the selected slow-chopping operands for this session. The policy is
    /// only accepted for the same parsed map context; a refusal leaves all
    /// admitted state unchanged.
    pub fn admit_campaign_slow_chopping(
        &mut self,
        context: &Arc<ParsedMapContext>,
        policy: SlowChoppingPolicy,
    ) -> Result<(), Error> {
        if self.campaign_slow_chopping.is_some() {
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
        if policy.context_mask == 0
            || policy.context_mask.count_ones() != 1
            || policy.context_mask >= 0x80
        {
            return Err(self.inner.error(
                NATIVE,
                Failure::ConfigState("context mask is not a single positive imm8 bit"),
            ));
        }
        if !policy.tree_damage.is_normal()
            || policy.tree_damage <= 0.0
            || policy.deposit_lumber <= 0
        {
            return Err(self.inner.error(
                NATIVE,
                Failure::ConfigState("ordinary harvest overrides must be finite and positive"),
            ));
        }
        self.campaign_slow_chopping = Some(policy);
        Ok(())
    }

    pub fn campaign_slow_chopping_policy(&self) -> Option<&SlowChoppingPolicy> {
        self.campaign_slow_chopping.as_ref()
    }

    /// `None` when slow-chopping operands are not admitted, the owner has no
    /// active campaign AI context, or that player has not run the setter. An
    /// invalid owner handle is an `Err`, not `None`.
    // shortcut: controller effects and modified workers remain unqualified; extend after their behavior is mapped.
    pub fn campaign_slow_chopping_enabled(&self, owner: HandleId) -> Result<Option<bool>, Error> {
        if self.campaign_slow_chopping.is_none() {
            return Ok(None);
        }
        let number = self.player_settings(owner)?.number;
        Ok(self.campaign_ai.slow_chopping(number))
    }

    pub(crate) fn set_ai_slow_chopping(&mut self, player: i32, enabled: bool) -> Result<(), Error> {
        if self.campaign_slow_chopping.is_none() {
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
        self.campaign_ai.set_slow_chopping(player, enabled);
        Ok(())
    }
}

//! Per-player campaign gathering policy and one canonical unit association.
use super::{Entry, Error, Failure, ObjectHost};
use crate::campaign_gathering_policy::GatheringPolicy;
use crate::map_admission::ParsedMapContext;
use pcx_script::HandleId;
use std::collections::BTreeSet;
use std::sync::Arc;

const NATIVE: &str = "StopGathering";

#[derive(Clone, Debug, PartialEq)]
pub struct GatheringObservation {
    /// Unknown until the owning AI calls `StopGathering`.
    pub enabled: Option<bool>,
    /// The current canonical handle and position; invalid associations are omitted.
    pub association: Option<(HandleId, [f32; 2])>,
}

impl ObjectHost {
    #[cfg(test)]
    pub(crate) fn fixture_gathering_class(&mut self, unit: HandleId, class: &str) {
        let Entry::Unit { record } = self.inner.entry_mut(unit).expect("fixture unit") else {
            panic!("fixture unit family")
        };
        record
            .definition
            .fields
            .get_mut(b"F002")
            .expect("fixture class field")
            .value = Ok(pcx_sim::world::FieldValue::Cell(class.into()));
    }

    pub fn admit_campaign_gathering(
        &mut self,
        context: &Arc<ParsedMapContext>,
        policy: GatheringPolicy,
    ) -> Result<(), Error> {
        if self.campaign_gathering.is_some() {
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
        self.campaign_gathering = Some(policy);
        Ok(())
    }

    pub fn campaign_gathering_policy(&self) -> Option<&GatheringPolicy> {
        self.campaign_gathering.as_ref()
    }

    pub fn campaign_gathering_state(
        &self,
        owner: HandleId,
    ) -> Result<Option<GatheringObservation>, Error> {
        if self.campaign_gathering.is_none() {
            return Ok(None);
        }
        let player = self.player_settings(owner)?.number;
        let Some((enabled, associated)) = self.campaign_ai.gathering(player) else {
            return Ok(None);
        };
        let association = associated.and_then(|unit| self.live_gathering_unit(unit, owner));
        // shortcut: a later direct birth can fill an empty stopped association;
        // qualify native linked-list insertion order before changing this choice.
        let association = if association.is_none() && enabled == Some(false) {
            self.select_gathering_unit(owner, NATIVE)?
                .and_then(|unit| self.live_gathering_unit(unit, owner))
        } else {
            association
        };
        Ok(Some(GatheringObservation {
            enabled,
            association,
        }))
    }

    pub(crate) fn stop_ai_gathering(&mut self, player: i32) -> Result<(), Error> {
        if self.campaign_gathering.is_none() {
            return Err(self
                .inner
                .error(NATIVE, Failure::ConfigState("operands are not admitted")));
        }
        let owner = self.player_identity(player).ok_or_else(|| {
            self.inner
                .error(NATIVE, Failure::ConfigState("player is not admitted"))
        })?;
        self.campaign_ai.gathering(player).ok_or_else(|| {
            self.inner
                .error(NATIVE, Failure::ConfigState("AI player is not active"))
        })?;
        // An accepted association retains its slot. Selection and all refusals
        // precede the policy write; no explicit unit order is changed.
        let unit = self
            .current_gathering_anchor(owner, NATIVE)?
            .map(|(unit, _)| unit);
        self.campaign_ai
            .stop_gathering(player, unit)
            .then_some(())
            .ok_or_else(|| {
                self.inner
                    .error(NATIVE, Failure::ConfigState("AI player is not active"))
            })
    }

    /// Resolve the current canonical association without changing AI flags.
    /// Creation can use this before `StopGathering` is called.
    pub(super) fn current_gathering_anchor(
        &self,
        owner: HandleId,
        native: &'static str,
    ) -> Result<Option<(HandleId, [f32; 2])>, Error> {
        if self.campaign_gathering.is_none() {
            return Err(self
                .inner
                .error(native, Failure::ConfigState("operands are not admitted")));
        }
        let player = self.player_settings(owner)?.number;
        if let Some((_, Some(unit))) = self.campaign_ai.gathering(player)
            && let Some(association) = self.live_gathering_unit(unit, owner)
        {
            return Ok(Some(association));
        }
        Ok(self
            .select_gathering_unit(owner, native)?
            .and_then(|unit| self.live_gathering_unit(unit, owner)))
    }

    fn live_gathering_unit(&self, unit: HandleId, owner: HandleId) -> Option<(HandleId, [f32; 2])> {
        let Entry::Unit { record } = self.inner.entry(unit).ok()? else {
            return None;
        };
        if record.owner != owner
            || !record.shown
            || record.summoned
            || record.paused_since.is_some()
            || record.combat.as_ref().is_some_and(|combat| combat.dead)
            || record
                .vitals
                .as_ref()
                .is_some_and(|vitals| vitals.life <= 0.0)
            || self
                .inner
                .arena
                .iter()
                .any(|entry| matches!(entry, Entry::Effect { unit: target, .. } if *target == unit))
        {
            return None;
        }
        let position = record.position?;
        position
            .iter()
            .all(|value| value.is_finite())
            .then_some((unit, position))
    }

    fn select_gathering_unit(
        &self,
        owner: HandleId,
        native: &'static str,
    ) -> Result<Option<HandleId>, Error> {
        let number = self.player_settings(owner)?.number;
        self.campaign_gathering.as_ref().ok_or_else(|| {
            self.inner
                .error(native, Failure::ConfigState("operands are not admitted"))
        })?;
        // The symbolic enum and cached unit-class bit occupy distinct native domains.
        self.source_constant_enumerand("UNIT_TYPE_PEON", "unittype")?;
        let mut preferred = None;
        let mut fallback = None;
        let affected: BTreeSet<_> = self
            .inner
            .arena
            .iter()
            .filter_map(|entry| match entry {
                Entry::Effect { unit, .. } => Some(*unit),
                _ => None,
            })
            .collect();
        for (unit, record) in self
            .executed_units()
            .filter(|(_, record)| record.owner == owner)
        {
            if record.combat.as_ref().is_some_and(|combat| combat.dead)
                || record
                    .vitals
                    .as_ref()
                    .is_some_and(|vitals| vitals.life <= 0.0)
            {
                continue;
            }
            // shortcut: the selected native's dynamic construction/instant-path
            // flags have no current host equivalent; accept direct effect-free units.
            if record.executed_owner != number
                || record.summoned
                || !record.shown
                || record.paused_since.is_some()
                || record.position.is_none()
                || record
                    .position
                    .is_some_and(|position| position.iter().any(|value| !value.is_finite()))
                || affected.contains(&unit)
            {
                return Err(self.inner.error(
                    native,
                    Failure::UnitActivity(
                        "candidate membership has unmodeled dynamic state".into(),
                    ),
                ));
            }
            if super::unit_types::class(&record.definition, "UNIT_TYPE_PEON")
                .map_err(|detail| self.inner.error(native, Failure::UnitActivity(detail)))?
            {
                return Ok(Some(unit));
            }
            let structure = super::unit_types::structure(&record.definition)
                .map_err(|detail| self.inner.error(native, Failure::UnitActivity(detail)))?;
            if structure {
                preferred = Some(unit);
            } else {
                fallback = Some(unit);
            }
        }
        Ok(preferred.or(fallback))
    }
}

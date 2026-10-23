//! Read-only vision inputs from the canonical live world.
use super::{Error, Failure, ObjectHost};
use pcx_script::HandleId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VisionState {
    Masked,
    Fogged,
    Visible,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VisionCircle {
    pub center: [f32; 2],
    pub radius: f32,
    pub state: VisionState,
    /// Modifiers requested after units follow the ordinary unit sources.
    pub after_units: bool,
}

impl ObjectHost {
    /// Source-derived day/night classification on the existing game clock.
    pub fn vision_is_night(&self) -> Option<bool> {
        self.natural_sleep_night()
    }

    pub fn vision_world_bounds(&self) -> Option<[[f32; 2]; 2]> {
        self.unit_placement
            .as_ref()
            .map(|placement| placement.world_bounds())
    }

    fn shares_vision(&self, owner: HandleId, viewer: HandleId) -> Result<bool, Error> {
        if owner == viewer {
            return Ok(true);
        }
        let source = self.player_settings(owner)?;
        let target = self.player_settings(viewer)?;
        Ok(["ALLIANCE_SHARED_VISION", "ALLIANCE_SHARED_VISION_FORCED"]
            .into_iter()
            .filter_map(|name| self.source_constant_enumerand(name, "alliancetype").ok())
            .any(|kind| self.effective_alliance(source, target, kind)))
    }

    /// Live circles only; this neither changes the arena nor retains a second unit store.
    pub fn vision_sources(
        &self,
        viewer: HandleId,
        night: bool,
        limit: usize,
    ) -> Result<Vec<VisionCircle>, Error> {
        let fail = |detail| self.inner.error("vision", Failure::UnitActivity(detail));
        self.player_settings(viewer)?;
        let mut sources = Vec::new();
        let mut scanned = 0usize;
        let mut push = |circle: VisionCircle| -> Result<(), Error> {
            if sources.len() >= limit {
                return Err(fail("vision source budget exceeded".into()));
            }
            if !circle.center.iter().all(|n| n.is_finite())
                || !circle.radius.is_finite()
                || circle.radius < 0.0
            {
                return Err(fail("finite nonnegative vision circle required".into()));
            }
            sources
                .try_reserve(1)
                .map_err(|_| fail("vision allocation failed".into()))?;
            sources.push(circle);
            Ok(())
        };
        for modifier in self.fog_modifiers().filter(|m| m.started) {
            scanned = scanned.saturating_add(1);
            if scanned > limit {
                return Err(fail("vision scan budget exceeded".into()));
            }
            if modifier.player != viewer
                && !(modifier.shared_vision && self.shares_vision(modifier.player, viewer)?)
            {
                continue;
            }
            let value = self.inner.symbolic_value(modifier.state, "fogstate")?;
            let state = [
                ("FOG_OF_WAR_MASKED", VisionState::Masked),
                ("FOG_OF_WAR_FOGGED", VisionState::Fogged),
                ("FOG_OF_WAR_VISIBLE", VisionState::Visible),
            ]
            .into_iter()
            .find_map(|(name, state)| {
                (self.source_constant_enumerand(name, "fogstate").ok() == Some(value))
                    .then_some(state)
            })
            .ok_or_else(|| fail("unsupported source fog state".into()))?;
            push(VisionCircle {
                center: modifier.center,
                radius: modifier.radius,
                state,
                after_units: modifier.after_units,
            })?;
        }
        // Preserve modifier creation order within each phase.
        sources.sort_by_key(|circle| circle.after_units);
        let after = sources.partition_point(|circle| !circle.after_units);
        let mut units = Vec::new();
        for (_, unit) in self.executed_units() {
            scanned = scanned.saturating_add(1);
            if scanned > limit {
                return Err(fail("vision scan budget exceeded".into()));
            }
            if !unit.shown
                || unit.combat.as_ref().is_some_and(|c| c.dead)
                || unit.vitals.is_some_and(|v| v.life <= 0.)
                || !self.shares_vision(unit.owner, viewer)?
            {
                continue;
            }
            let center = unit
                .position
                .ok_or_else(|| fail("unit has no accepted vision position".into()))?;
            // shortcut: base day/night sight only; apply sight-changing upgrades and effects when their live consumers are modeled.
            let radius = super::unit_activity::balance_real(
                &unit.definition,
                if night { "nsight" } else { "sight" },
            )
            .map_err(fail)?;
            units
                .try_reserve(1)
                .map_err(|_| fail("vision allocation failed".into()))?;
            units.push(VisionCircle {
                center,
                radius,
                state: VisionState::Visible,
                after_units: false,
            });
        }
        sources
            .try_reserve(units.len())
            .map_err(|_| fail("vision allocation failed".into()))?;
        sources.splice(after..after, units);
        Ok(sources)
    }
}

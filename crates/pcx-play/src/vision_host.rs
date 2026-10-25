//! Read-only vision inputs from the canonical live world.
use super::{Entry, Error, Failure, ObjectHost};
use pcx_script::{HandleId, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VisionState {
    Masked,
    Fogged,
    Visible,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VisionShape {
    Circle { center: [f32; 2], radius: f32 },
    Rectangle { bounds: [f32; 4] },
}
impl VisionShape {
    pub fn bounds(self) -> Option<[f64; 4]> {
        match self {
            Self::Circle { center, radius }
                if center.iter().all(|v| v.is_finite()) && radius.is_finite() && radius >= 0. =>
            {
                let [x, y] = center.map(f64::from);
                let r = f64::from(radius);
                Some([x - r, y - r, x + r, y + r])
            }
            Self::Rectangle { bounds }
                if bounds.iter().all(|v| v.is_finite())
                    && bounds[0] <= bounds[2]
                    && bounds[1] <= bounds[3] =>
            {
                Some(bounds.map(f64::from))
            }
            _ => None,
        }
    }
    pub fn contains(self, [x, y]: [f64; 2]) -> bool {
        match self {
            Self::Circle { center, radius } => {
                (x - f64::from(center[0])).powi(2) + (y - f64::from(center[1])).powi(2)
                    <= f64::from(radius).powi(2)
            }
            Self::Rectangle {
                bounds: [x0, y0, x1, y1],
            } => {
                (f64::from(x0)..=f64::from(x1)).contains(&x)
                    && (f64::from(y0)..=f64::from(y1)).contains(&y)
            }
        }
    }
}

/// Who produced a vision input. Circle modifiers and unit sight are both
/// circles and can both run before the unit phase, so occlusion eligibility
/// cannot be inferred from shape or `after_units`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VisionOrigin {
    Unit { unit: HandleId },
    Modifier { modifier: HandleId },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VisionSource {
    pub shape: VisionShape,
    pub state: VisionState,
    /// Modifiers requested after units follow the ordinary unit sources.
    pub after_units: bool,
    pub origin: VisionOrigin,
    /// Qualified eye or fly height. Live records do not supply one yet.
    pub source_height: Option<f32>,
}

#[path = "vision_field.rs"]
mod field;
pub(crate) use field::VisionSlot;
pub use field::{VisionCapability, VisionCell, VisionSampleStats, present_vision};

impl ObjectHost {
    pub(super) fn unit_vision_declaration(&self, source: &str) -> Result<(), Error> {
        let name = "UnitShareVision";
        if self.inner.program.natives().get(name).is_some()
            || self.inner.program.natives().ambiguous(name).is_some()
        {
            Self::contract(
                &self.inner,
                name,
                &["unit", "player", "boolean"],
                "nothing",
                false,
                Some("unit"),
            )?;
            if self
                .inner
                .program
                .natives()
                .get(name)
                .is_none_or(|n| n.source != source)
            {
                return Err(self.inner.error(name, Failure::Source));
            }
        }
        Ok(())
    }

    pub(super) fn unit_share_vision(&mut self, args: &[Value]) -> Result<Value, Error> {
        let name = "UnitShareVision";
        let [
            Value::Handle(unit),
            Value::Handle(viewer),
            Value::Bool(share),
        ] = args
        else {
            return Err(self.inner.error(name, Failure::Signature));
        };
        self.player_settings(*viewer)?;
        self.executed_unit(*unit)?;
        let Entry::Unit { record } = self.inner.entry_mut(*unit)? else {
            unreachable!("validated live unit");
        };
        let existing = record.vision_viewers.iter().position(|p| p == viewer);
        if *share && existing.is_none() {
            record
                .vision_viewers
                .try_reserve(1)
                .map_err(|_| self.inner.error(name, Failure::Capacity))?;
            // Reborrow after the fallible allocation; no grants have changed on failure.
            let Entry::Unit { record } = self.inner.entry_mut(*unit)? else {
                unreachable!()
            };
            record.vision_viewers.push(*viewer);
        } else if !*share && let Some(index) = existing {
            record.vision_viewers.remove(index);
        }
        Ok(Value::Null)
    }

    /// Source-derived day/night classification on the existing game clock.
    pub fn vision_is_night(&self) -> Option<bool> {
        self.natural_sleep_night()
    }

    pub fn vision_world_bounds(&self) -> Option<[[f32; 2]; 2]> {
        self.world_bounds
            .map(|[x0, y0, x1, y1]| [[x0, y0], [x1, y1]])
            .or_else(|| {
                self.unit_placement
                    .as_ref()
                    .map(|placement| placement.world_bounds())
            })
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

    /// Live vision areas only; this neither changes the arena nor retains a second unit store.
    pub fn vision_sources(
        &self,
        viewer: HandleId,
        night: bool,
        limit: usize,
    ) -> Result<Vec<VisionSource>, Error> {
        Ok(self.list_vision_sources(viewer, night, limit)?.sources)
    }

    pub(super) fn list_vision_sources(
        &self,
        viewer: HandleId,
        night: bool,
        limit: usize,
    ) -> Result<ListedVision, Error> {
        let fail = |detail| self.inner.error("vision", Failure::UnitActivity(detail));
        self.player_settings(viewer)?;
        let mut sources = Vec::new();
        let mut scanned = 0usize;
        let mut push = |circle: VisionSource| -> Result<(), Error> {
            if sources.len() >= limit {
                return Err(fail("vision source budget exceeded".into()));
            }
            if circle.shape.bounds().is_none() {
                return Err(fail("finite ordered vision geometry required".into()));
            }
            sources
                .try_reserve(1)
                .map_err(|_| fail("vision allocation failed".into()))?;
            sources.push(circle);
            Ok(())
        };
        for (index, entry) in self.inner.arena.iter().enumerate() {
            let Entry::FogModifier(Some(modifier)) = entry else {
                continue;
            };
            if !modifier.started {
                continue;
            }
            scanned = scanned.saturating_add(1);
            if scanned > limit {
                return Err(fail("vision scan budget exceeded".into()));
            }
            if modifier.player != viewer
                && !(modifier.shared_vision && self.shares_vision(modifier.player, viewer)?)
            {
                continue;
            }
            let modifier_id = self.inner.id(index, "vision")?;
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
            push(VisionSource {
                shape: modifier.shape,
                state,
                after_units: modifier.after_units,
                origin: VisionOrigin::Modifier {
                    modifier: modifier_id,
                },
                source_height: None,
            })?;
        }
        // Preserve modifier creation order within each phase.
        sources.sort_by_key(|circle| circle.after_units);
        let after = sources.partition_point(|circle| !circle.after_units);
        let mut units = Vec::new();
        for (unit_id, unit) in self.executed_units() {
            scanned = scanned.saturating_add(1);
            if scanned > limit {
                return Err(fail("vision scan budget exceeded".into()));
            }
            if !unit.shown
                || unit.combat.as_ref().is_some_and(|c| c.dead)
                || unit.vitals.is_some_and(|v| v.life <= 0.)
                || !(unit.vision_viewers.contains(&viewer)
                    || self.shares_vision(unit.owner, viewer)?)
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
            units.push(VisionSource {
                shape: VisionShape::Circle { center, radius },
                state: VisionState::Visible,
                after_units: false,
                origin: VisionOrigin::Unit { unit: unit_id },
                source_height: None,
            });
        }
        sources
            .try_reserve(units.len())
            .map_err(|_| fail("vision allocation failed".into()))?;
        sources.splice(after..after, units);
        Ok(ListedVision {
            sources,
            candidates_scanned: scanned,
        })
    }
}

pub(super) struct ListedVision {
    pub(super) sources: Vec<VisionSource>,
    /// Started modifiers plus live units examined, including those that emit nothing.
    pub(super) candidates_scanned: usize,
}

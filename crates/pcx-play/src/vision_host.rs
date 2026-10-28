//! Read-only vision inputs from the canonical live world.
use super::{Entry, Error, Failure, ObjectHost};
use pcx_script::{HandleId, Value};
use std::collections::BTreeMap;

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

    fn shares_vision(
        &self,
        owner: HandleId,
        viewer: HandleId,
        kinds: &[i32],
    ) -> Result<bool, Error> {
        if owner == viewer {
            return Ok(true);
        }
        let source = self.player_settings(owner)?;
        let target = self.player_settings(viewer)?;
        Ok(kinds
            .iter()
            .any(|&kind| self.effective_alliance(source, target, kind)))
    }

    /// The selected alliance kinds that share vision. A constant the script does not declare
    /// is skipped.
    fn sharing_kinds(&self) -> Vec<i32> {
        ["ALLIANCE_SHARED_VISION", "ALLIANCE_SHARED_VISION_FORCED"]
            .into_iter()
            .filter_map(|name| self.source_constant_enumerand(name, "alliancetype").ok())
            .collect()
    }

    /// The selected fog state values, which `FOG_OF_WAR_*` constants map to a [`VisionState`].
    fn fog_state_values(&self) -> [(Option<i32>, VisionState); 3] {
        [
            ("FOG_OF_WAR_MASKED", VisionState::Masked),
            ("FOG_OF_WAR_FOGGED", VisionState::Fogged),
            ("FOG_OF_WAR_VISIBLE", VisionState::Visible),
        ]
        .map(|(name, state)| (self.source_constant_enumerand(name, "fogstate").ok(), state))
    }

    /// Live vision areas only; this neither changes the arena nor retains a second unit store.
    pub fn vision_sources(
        &self,
        viewer: HandleId,
        night: bool,
        limit: usize,
    ) -> Result<Vec<VisionSource>, Error> {
        let mut table = self.source_table(night)?;
        Ok(self
            .list_vision_sources(viewer, limit, &mut SightRadii::default(), &mut table)?
            .sources)
    }

    /// Gathers what every viewer's listing reads, so a sample walks the world once.
    pub(super) fn source_table(&self, night: bool) -> Result<SourceTable<'_>, Error> {
        let fail = |detail: &str| {
            self.inner
                .error("vision", Failure::UnitActivity(detail.into()))
        };
        let mut modifiers = Vec::new();
        modifiers
            .try_reserve(self.inner.fog_slots.len())
            .map_err(|_| fail("vision allocation failed"))?;
        modifiers.extend(
            self.inner
                .fog_slots
                .iter()
                .filter_map(|&index| match &self.inner.arena[index] {
                    Entry::FogModifier(Some(modifier)) if modifier.started => {
                        Some((index, modifier))
                    }
                    _ => None,
                }),
        );
        let mut units = Vec::new();
        units
            .try_reserve(self.inner.unit_slots.len())
            .map_err(|_| fail("vision allocation failed"))?;
        units.extend(self.executed_units().map(|(id, unit)| TableUnit {
            id,
            unit,
            live: unit.shown
                && !unit.combat.as_ref().is_some_and(|c| c.dead)
                && !unit.vitals.is_some_and(|v| v.life <= 0.),
            radius: None,
        }));
        Ok(SourceTable {
            night,
            modifiers,
            units,
            sharing_kinds: None,
            fog_states: None,
        })
    }

    pub(super) fn list_vision_sources(
        &self,
        viewer: HandleId,
        limit: usize,
        radii: &mut SightRadii,
        table: &mut SourceTable<'_>,
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
        for &(index, modifier) in &table.modifiers {
            scanned = scanned.saturating_add(1);
            if scanned > limit {
                return Err(fail("vision scan budget exceeded".into()));
            }
            if modifier.player != viewer
                && !(modifier.shared_vision
                    && self.shares_vision(
                        modifier.player,
                        viewer,
                        table
                            .sharing_kinds
                            .get_or_insert_with(|| self.sharing_kinds()),
                    )?)
            {
                continue;
            }
            let modifier_id = self.inner.id(index, "vision")?;
            let value = self.inner.symbolic_value(modifier.state, "fogstate")?;
            let state = table
                .fog_states
                .get_or_insert_with(|| self.fog_state_values())
                .iter()
                .find_map(|&(known, state)| (known == Some(value)).then_some(state))
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
        // Whether each unit owner shares vision with `viewer`; the answer depends on the owner only.
        let mut shares: Vec<(HandleId, bool)> = Vec::new();
        let night = table.night;
        for listed in &mut table.units {
            scanned = scanned.saturating_add(1);
            if scanned > limit {
                return Err(fail("vision scan budget exceeded".into()));
            }
            if !listed.live {
                continue;
            }
            let unit = listed.unit;
            if !unit.vision_viewers.contains(&viewer) {
                let shared = match shares.iter().find(|(owner, _)| *owner == unit.owner) {
                    Some(&(_, shared)) => shared,
                    None => {
                        let kinds = table
                            .sharing_kinds
                            .get_or_insert_with(|| self.sharing_kinds());
                        let shared = self.shares_vision(unit.owner, viewer, kinds)?;
                        shares.push((unit.owner, shared));
                        shared
                    }
                };
                if !shared {
                    continue;
                }
            }
            let center = unit
                .position
                .ok_or_else(|| fail("unit has no accepted vision position".into()))?;
            // shortcut: base day/night sight only; apply sight-changing upgrades and effects when their live consumers are modeled.
            let radius = listed
                .radius
                .get_or_insert_with(|| {
                    radii.of(listed.id, unit.definition_revision, &unit.definition, night)
                })
                .clone()
                .map_err(fail)?;
            units
                .try_reserve(1)
                .map_err(|_| fail("vision allocation failed".into()))?;
            units.push(VisionSource {
                shape: VisionShape::Circle { center, radius },
                state: VisionState::Visible,
                after_units: false,
                origin: VisionOrigin::Unit { unit: listed.id },
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

/// What every viewer's source listing reads, gathered once per sample.
pub(super) struct SourceTable<'a> {
    night: bool,
    /// Started fog modifiers with their arena index, in creation order.
    modifiers: Vec<(usize, &'a super::FogModifier)>,
    /// Every executed unit in arena order.
    units: Vec<TableUnit<'a>>,
    /// Resolved on first use: the alliance kinds that share vision.
    sharing_kinds: Option<Vec<i32>>,
    /// Resolved on first use: the fog state values.
    fog_states: Option<[(Option<i32>, VisionState); 3]>,
}

struct TableUnit<'a> {
    id: HandleId,
    unit: &'a super::ExecutedUnit,
    /// Shown, not dead and with life left: the units that can emit a source.
    live: bool,
    /// Sight radius, read the first time a viewer needs it.
    radius: Option<Result<f32, String>>,
}

/// Sight radius of a unit's definition at day or night. A unit's definition changes only with
/// `definition_revision`, so one entry serves every viewer and every later sample until then.
/// Entries not read in a sample are dropped when the sample ends, so removed units do not linger.
#[derive(Default)]
pub(super) struct SightRadii {
    kept: BTreeMap<SightKey, Result<f32, String>>,
    read: BTreeMap<SightKey, Result<f32, String>>,
}

type SightKey = (HandleId, u64, bool);

impl SightRadii {
    fn of(
        &mut self,
        unit: HandleId,
        revision: u64,
        definition: &pcx_sim::world::Definition,
        night: bool,
    ) -> Result<f32, String> {
        let key = (unit, revision, night);
        if let Some(known) = self.read.get(&key) {
            return known.clone();
        }
        let radius = self.kept.remove(&key).unwrap_or_else(|| {
            super::unit_activity::balance_real(definition, if night { "nsight" } else { "sight" })
        });
        self.read.insert(key, radius.clone());
        radius
    }

    /// The entries this sample read, ready for the next sample.
    pub(super) fn retained(self) -> Self {
        Self {
            kept: self.read,
            read: BTreeMap::new(),
        }
    }
}

pub(super) struct ListedVision {
    pub(super) sources: Vec<VisionSource>,
    /// Started modifiers plus live units examined, including those that emit nothing.
    pub(super) candidates_scanned: usize,
}

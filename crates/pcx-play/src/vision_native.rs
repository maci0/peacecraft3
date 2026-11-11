//! Native lattice adapter for the canonical vision ledger.
use super::*;
use crate::native_sight::{HeightGrid, Rules, Work};
use std::collections::{BTreeMap, BTreeSet};

impl ObjectHost {
    pub fn with_native_sight(mut self, rules: Rules) -> Result<Self, Error> {
        let fail = || {
            self.inner.error(
                "vision",
                Failure::UnitActivity(
                    "matching, unadmitted native sight source and empty unit arena required".into(),
                ),
            )
        };
        if self.native_sight.is_some()
            || self.shared_vision.is_some()
            || self.executed_units().next().is_some()
            || self
                .numeric
                .as_ref()
                .is_none_or(|n| *n.image_sha256() != rules.image_sha256)
        {
            return Err(fail());
        }
        self.native_sight = Some(Arc::new(rules));
        Ok(self)
    }
    pub(super) fn native_sight_updates(
        &self,
        night: bool,
        work: &mut Work,
    ) -> Result<Vec<(pcx_script::HandleId, crate::native_sight::SightState)>, String> {
        let rules = self
            .native_sight
            .as_ref()
            .ok_or("native sight rules absent")?;
        let numeric = self
            .numeric
            .as_ref()
            .ok_or("native sight numeric policy absent")?;
        let mut updates = Vec::new();
        for (id, unit) in self.executed_units() {
            work.charge(1)?;
            let state = super::super::SightRadii::next_sight(unit, night, rules, numeric)?;
            if unit.sight != Some(state) {
                updates
                    .try_reserve(1)
                    .map_err(|_| "native sight state allocation")?;
                updates.push((id, state));
            }
        }
        Ok(updates)
    }

    pub(super) fn commit_native_sight_updates(
        &mut self,
        updates: Vec<(pcx_script::HandleId, crate::native_sight::SightState)>,
    ) -> Result<(), Error> {
        for (id, state) in updates {
            let crate::playercolor_host::Entry::Unit { record } = self.inner.entry_mut(id)? else {
                unreachable!("staged live unit")
            };
            record.sight = Some(state);
        }
        Ok(())
    }

    pub(super) fn vision_pitch(&self) -> Result<f32, Error> {
        let fail = |s: String| self.inner.error("vision", Failure::UnitActivity(s));
        let cell = self
            .unit_placement
            .as_ref()
            .ok_or_else(|| fail("native sight placement absent".into()))?
            .rules()
            .cell_size();
        self.native_sight
            .as_ref()
            .map_or(Ok(cell), |r| r.cell_world(cell).map_err(fail))
    }
    pub(super) fn native_sight_movement(
        &self,
        unit: &crate::playercolor_host::ExecutedUnit,
    ) -> Result<bool, String> {
        let value = super::super::super::unit_positions::text(
            &unit.definition,
            pcx_objects::editor::SlkId::UnitData,
            "movetp",
        )
        .map_err(|e| format!("native sight movement: {e}"))?;
        if self
            .native_sight
            .as_ref()
            .is_some_and(|r| r.ignores_occlusion(value.trim()))
        {
            return Ok(true);
        }
        match value.trim() {
            "foot" | "horse" | "hover" | "float" | "amph" | "" | "_" | "none" => Ok(false),
            _ => Err(format!("native sight movement class unsupported: {value}")),
        }
    }
    pub(super) fn native_height_grid(
        &self,
        rules: &Rules,
        work: &mut Work,
    ) -> Result<HeightGrid, String> {
        let context = self
            .player_context()
            .ok_or("native sight map context absent")?;
        let placement = self
            .unit_placement
            .as_ref()
            .ok_or("native sight placement absent")?;
        if placement.rules().binding().image_sha256 != rules.image_sha256 {
            return Err("native sight placement source differs".into());
        }
        let pitch = rules.cell_world(placement.rules().cell_size())?;
        if pitch != placement.rules().terrain_encoding()[0] {
            return Err("native sight terrain/lattice ratio unsupported".into());
        }
        let mut grid =
            HeightGrid::new(&context.terrain, placement.rules().cell_size(), rules, work)?;
        let [width, height] = grid.dimensions();
        let origin = grid.origin();
        if placement.world_bounds()
            != [
                origin,
                [
                    origin[0] + width as f32 * pitch,
                    origin[1] + height as f32 * pitch,
                ],
            ]
        {
            return Err("native sight map/placement extents differ".into());
        }
        let mut definitions = BTreeMap::new();
        for (handle, creation, state) in self.destructables_with_handles() {
            work.charge(1)?;
            if state.dead || state.hidden || !state.life.is_some_and(|v| v > 0.) {
                continue;
            }
            let value = if let Some(value) = definitions.get(&creation.objectid) {
                *value
            } else {
                let catalog = self
                    .destructable_catalog
                    .as_ref()
                    .ok_or("native sight destructable catalog absent")?;
                let value = catalog.sight_occlusion(creation.objectid)?;
                definitions.insert(creation.objectid, value);
                value
            };
            if let Some((catalog_height, wide)) = value {
                // `SetDestructableOccluderHeight` replaces the catalog `occH` of one destructable.
                let height = self
                    .destructable_occluder
                    .get(&handle)
                    .copied()
                    .unwrap_or(catalog_height);
                if height > 0. && wide {
                    return Err(format!(
                        "native sight wide occluder unsupported: {:?}",
                        creation.objectid.to_be_bytes()
                    ));
                }
                grid.occlude([creation.x, creation.y], height, rules, work)?;
            }
        }
        for (_, unit) in self.executed_units() {
            work.charge(1)?;
            if !unit.shown
                || unit.combat.as_ref().is_some_and(|c| c.dead)
                || unit.vitals.is_some_and(|v| v.life <= 0.)
            {
                continue;
            }
            let height = super::super::super::unit_activity::column_real(
                &unit.definition,
                pcx_objects::editor::SlkId::UnitUi,
                "occH",
            )
            .map_err(|e| format!("native sight unit occlusion height: {e}"))?;
            grid.occlude(
                unit.position
                    .ok_or("native sight occluder position absent")?,
                height,
                rules,
                work,
            )?;
        }
        Ok(grid)
    }
}

impl SharedVision {
    pub(super) fn apply_native(
        &mut self,
        plans: Vec<(HandleId, Vec<VisionSource>)>,
        heights: HeightGrid,
        rules: &Rules,
        work: &mut Work,
        candidates_scanned: usize,
        unblocked: &BTreeSet<pcx_script::HandleId>,
    ) -> Result<VisionSampleStats, String> {
        if heights.dimensions() != [self.shape.width, self.shape.height] {
            return Err("native sight ledger dimensions differ".into());
        }
        let same_height =
            self.native_height.as_ref() == Some(&heights) && self.native_unblocked == *unblocked;
        let mut staged = Vec::new();
        let mut live = BTreeSet::new();
        let mut emitted_sources = 0usize;
        for (id, sources) in plans {
            live.insert(id);
            emitted_sources = emitted_sources
                .checked_add(sources.len())
                .ok_or("native sight source count overflow")?;
            let previous = self.viewers.get(&id);
            if same_height && previous.is_some_and(|p| p.sources == sources) {
                continue;
            }
            work.charge(self.shape.cells())?;
            let mut viewer = ViewerGrid::blank(self.shape.cells())?;
            if let Some(previous) = previous {
                viewer.explored.clone_from(&previous.explored);
                for (state, explored) in viewer.current.iter_mut().zip(&viewer.explored) {
                    if *explored {
                        *state = VisionState::Fogged;
                    }
                }
            }
            for source in &sources {
                let cells = match (source.origin, source.shape) {
                    (
                        super::super::VisionOrigin::Unit { unit },
                        VisionShape::Circle { center, radius },
                    ) if unblocked.contains(&unit) => {
                        rules.unblocked_cells(&heights, center, radius, work)?
                    }
                    (
                        super::super::VisionOrigin::Unit { .. },
                        VisionShape::Circle { center, radius },
                    ) => rules.visible_cells(&heights, center, radius, work)?,
                    (_, VisionShape::Circle { center, radius }) => {
                        rules.circle_cells(&heights, center, radius, work)?
                    }
                    (_, VisionShape::Rectangle { bounds }) => {
                        rules.rectangle_cells(&heights, bounds, work)?
                    }
                };
                work.charge(cells.len())?;
                for at in cells {
                    viewer.current[at] = source.state;
                    viewer.explored[at] = source.state != VisionState::Masked;
                }
            }
            viewer.sources = sources;
            staged.push((id, viewer));
        }
        let changed = !staged.is_empty() || self.viewers.keys().any(|id| !live.contains(id));
        self.viewers.extend(staged);
        self.viewers.retain(|id, _| live.contains(id));
        self.native_height = Some(heights);
        self.native_unblocked.clone_from(unblocked);
        self.ready = true;
        self.occlusion_applied = true;
        self.revision += u64::from(changed);
        self.last = VisionSampleStats {
            cell_visits: work.spent(),
            emitted_sources,
            candidates_scanned,
        };
        Ok(self.last)
    }
}

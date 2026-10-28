//! Shared per-viewer vision grid. Exploration changes only when a sample stamps
//! cells. Fog and mask switches are applied by [`present_vision`] and do not
//! write exploration.
//!
//! Pitch and extent come from the admitted placement grid, including a final
//! partial cell so the source extent is not truncated. This is a recreation
//! sample, not a traced native fog lattice. Terrain and tree occlusion are not
//! applied. Script queries, world actor fog, picking, and combat acquisition
//! are not wired here; they should call [`ObjectHost::vision_cell`] instead of
//! rebuilding sight circles.
use super::{Error, Failure, ObjectHost, VisionShape, VisionSource, VisionState};
use pcx_script::HandleId;
use std::collections::BTreeMap;

/// Application allocation ceiling for one viewer's grid. Not a map constant.
const MAX_GRID_CELLS: usize = 4_000_000;
/// How many full-grid stamp coverings one sample may perform across viewers.
/// Not a game rule. The charge is the touched rectangles, not viewers times
/// every cell times every source.
const STAMP_COVERINGS: usize = 64;
/// Changed sources above which a viewer is restamped in full instead of by region. Bounds the
/// pairwise region merge; not a game rule.
const MAX_INCREMENTAL_SOURCES: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VisionCell {
    /// Raw sample state before fog and mask switches.
    pub raw: VisionState,
    /// Set by a non-masked stamp. Switches never change it.
    pub explored: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VisionSampleStats {
    pub cell_visits: usize,
    /// Stamp inputs after alliance, life, and fog-state filtering.
    /// This is not the number of units and modifiers examined.
    pub emitted_sources: usize,
    /// Live units plus started fog modifiers read by `vision_sources`, once per viewer.
    pub candidates_scanned: usize,
}

/// Presented state. Does not record exploration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VisionCapability {
    /// This world never admitted shared vision.
    Unadmitted,
    /// Admission was attempted and the ledger cannot be read yet.
    Unavailable(&'static str),
    /// Fog switches from the admitted startup settings.
    Ready { fog: bool, mask: bool },
}

pub fn present_vision(cell: VisionCell, fog: bool, mask: bool) -> VisionState {
    if cell.raw == VisionState::Visible || (!fog && (!mask || cell.explored)) {
        VisionState::Visible
    } else if mask && !cell.explored {
        VisionState::Masked
    } else {
        VisionState::Fogged
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum VisionBlock {
    MissingPlacement,
    MissingNight,
    ExtentMismatch,
    GridTooLarge,
}

impl VisionBlock {
    fn as_str(self) -> &'static str {
        match self {
            Self::MissingPlacement => "shared vision needs admitted placement",
            Self::MissingNight => "shared vision needs admitted day/night sight",
            Self::ExtentMismatch => "shared vision extent differs from the admitted placement grid",
            Self::GridTooLarge => "shared vision grid exceeds the allocation bound",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct GridShape {
    left: f64,
    bottom: f64,
    right: f64,
    top: f64,
    pitch: f64,
    width: usize,
    height: usize,
}

impl GridShape {
    fn from_bounds(bounds: [[f32; 2]; 2], pitch: f32) -> Result<Self, String> {
        let left = f64::from(bounds[0][0]);
        let bottom = f64::from(bounds[0][1]);
        let right = f64::from(bounds[1][0]);
        let top = f64::from(bounds[1][1]);
        let pitch = f64::from(pitch);
        if !(left < right && bottom < top && pitch.is_finite() && pitch > 0.) {
            return Err("shared vision extent is invalid".into());
        }
        let width = axis_count(right - left, pitch)?;
        let height = axis_count(top - bottom, pitch)?;
        width
            .checked_mul(height)
            .filter(|n| *n > 0 && *n <= MAX_GRID_CELLS)
            .ok_or("shared vision grid exceeds the allocation bound")?;
        Ok(Self {
            left,
            bottom,
            right,
            top,
            pitch,
            width,
            height,
        })
    }

    fn cells(self) -> usize {
        self.width * self.height
    }

    fn sample_x(self, index: usize) -> f64 {
        sample_on(self.left, self.right, self.pitch, index)
    }

    fn sample_y(self, index: usize) -> f64 {
        sample_on(self.bottom, self.top, self.pitch, index)
    }

    fn index_x(self, value: f64) -> Option<usize> {
        axis_index(value, self.left, self.right, self.pitch, self.width)
    }

    fn index_y(self, value: f64) -> Option<usize> {
        axis_index(value, self.bottom, self.top, self.pitch, self.height)
    }
}

fn axis_count(span: f64, pitch: f64) -> Result<usize, String> {
    let count = (span / pitch).ceil();
    if !count.is_finite() || count < 1. || count > MAX_GRID_CELLS as f64 {
        return Err("shared vision grid exceeds the allocation bound".into());
    }
    Ok(count as usize)
}

fn sample_on(origin: f64, far: f64, pitch: f64, index: usize) -> f64 {
    let start = origin + index as f64 * pitch;
    let end = (start + pitch).min(far);
    (start + end) * 0.5
}

fn axis_index(value: f64, origin: f64, far: f64, pitch: f64, count: usize) -> Option<usize> {
    if !value.is_finite() || value < origin || value > far || count == 0 {
        return None;
    }
    let mut index = ((value - origin) / pitch).floor();
    if !index.is_finite() || index < 0. {
        return None;
    }
    if index >= count as f64 {
        if value == far {
            index = count as f64 - 1.;
        } else {
            return None;
        }
    }
    Some(index as usize)
}

fn axis_range(
    origin: f64,
    far: f64,
    pitch: f64,
    count: usize,
    min: f64,
    max: f64,
) -> (usize, usize) {
    if max < origin || min > far || max < min || count == 0 {
        return (0, 0);
    }
    let min = min.max(origin);
    let max = max.min(far);
    if max < min {
        return (0, 0);
    }
    let Some(start) = axis_index(min, origin, far, pitch, count) else {
        return (0, 0);
    };
    let Some(end) = axis_index(max, origin, far, pitch, count) else {
        return (0, 0);
    };
    (start, end + 1)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Rect {
    x0: usize,
    x1: usize,
    y0: usize,
    y1: usize,
}

impl Rect {
    fn area(self) -> usize {
        self.x1
            .saturating_sub(self.x0)
            .saturating_mul(self.y1.saturating_sub(self.y0))
    }

    fn from_source(shape: super::VisionShape, grid: &GridShape) -> Result<Self, String> {
        let [x0, y0, x1, y1] = shape
            .bounds()
            .ok_or("shared vision source geometry is invalid")?;
        let (x0, x1) = axis_range(grid.left, grid.right, grid.pitch, grid.width, x0, x1);
        let (y0, y1) = axis_range(grid.bottom, grid.top, grid.pitch, grid.height, y0, y1);
        Ok(Self { x0, x1, y0, y1 })
    }

    fn intersection(self, other: Self) -> Self {
        Self {
            x0: self.x0.max(other.x0),
            x1: self.x1.min(other.x1),
            y0: self.y0.max(other.y0),
            y1: self.y1.min(other.y1),
        }
    }

    fn overlaps(self, other: Self) -> bool {
        self.intersection(other).area() > 0
    }

    fn bounding(self, other: Self) -> Self {
        Self {
            x0: self.x0.min(other.x0),
            x1: self.x1.max(other.x1),
            y0: self.y0.min(other.y0),
            y1: self.y1.max(other.y1),
        }
    }
}

/// Merges overlapping regions into their bounding boxes until all remaining regions are disjoint.
fn merge_overlapping(regions: &mut Vec<Rect>) {
    'again: loop {
        for i in 0..regions.len() {
            for j in i + 1..regions.len() {
                if regions[i].overlaps(regions[j]) {
                    let other = regions.swap_remove(j);
                    regions[i] = regions[i].bounding(other);
                    continue 'again;
                }
            }
        }
        return;
    }
}

struct ViewerGrid {
    explored: Vec<bool>,
    current: Vec<VisionState>,
    /// Cell rectangle of each entry of `sources`, in the same order.
    rects: Vec<Rect>,
    sources: Vec<VisionSource>,
}

impl ViewerGrid {
    fn blank(cells: usize) -> Result<Self, String> {
        let mut explored = Vec::new();
        let mut current = Vec::new();
        explored
            .try_reserve_exact(cells)
            .map_err(|_| "shared vision allocation")?;
        current
            .try_reserve_exact(cells)
            .map_err(|_| "shared vision allocation")?;
        explored.resize(cells, false);
        current.resize(cells, VisionState::Masked);
        Ok(Self {
            explored,
            current,
            rects: Vec::new(),
            sources: Vec::new(),
        })
    }

    fn reset_rect(&mut self, rect: Rect, width: usize, visits: &mut usize) {
        for y in rect.y0..rect.y1 {
            *visits += rect.x1.saturating_sub(rect.x0);
            for at in y * width + rect.x0..y * width + rect.x1 {
                self.current[at] = if self.explored[at] {
                    VisionState::Fogged
                } else {
                    VisionState::Masked
                };
            }
        }
    }

    /// Stamps `sources` in order, optionally only inside `clip`. Cell sample coordinates are read
    /// from the grid's per-column and per-row tables, the same values `sample_x`/`sample_y` return.
    fn stamp(
        &mut self,
        sources: &[VisionSource],
        rects: &[Rect],
        clip: Option<Rect>,
        vision: &SharedVision,
        visits: &mut usize,
    ) {
        let width = vision.shape.width;
        for (source, rect) in sources.iter().zip(rects) {
            let rect = clip.map_or(*rect, |clip| rect.intersection(clip));
            let explored = source.state != VisionState::Masked;
            for y in rect.y0..rect.y1 {
                *visits += rect.x1.saturating_sub(rect.x0);
                let sy = vision.rows[y];
                let row = y * width;
                match source.shape {
                    VisionShape::Circle { center, radius } => {
                        let dy2 = (sy - f64::from(center[1])).powi(2);
                        let radius2 = f64::from(radius).powi(2);
                        let cx = f64::from(center[0]);
                        for x in rect.x0..rect.x1 {
                            if (vision.columns[x] - cx).powi(2) + dy2 <= radius2 {
                                self.current[row + x] = source.state;
                                self.explored[row + x] = explored;
                            }
                        }
                    }
                    shape @ VisionShape::Rectangle { .. } => {
                        for x in rect.x0..rect.x1 {
                            if shape.contains([vision.columns[x], sy]) {
                                self.current[row + x] = source.state;
                                self.explored[row + x] = explored;
                            }
                        }
                    }
                }
            }
        }
    }
}

/// How one viewer's grid reaches the new source list.
enum Update {
    Unchanged,
    /// Reset every previous rectangle and stamp every source.
    Full,
    /// Reset and restamp only these disjoint regions, which contain every cell a changed source
    /// covered before or covers now. Cells outside them are covered only by unchanged sources.
    Regions(Vec<Rect>),
}

struct Planned {
    id: HandleId,
    sources: Vec<VisionSource>,
    rects: Vec<Rect>,
    update: Update,
}

pub(crate) struct SharedVision {
    shape: GridShape,
    /// Sample coordinate of each column's cell centre.
    columns: Vec<f64>,
    /// Sample coordinate of each row's cell centre.
    rows: Vec<f64>,
    viewers: BTreeMap<HandleId, ViewerGrid>,
    radii: Box<super::SightRadii>,
    ready: bool,
    /// Terrain and tree occlusion are not part of this sampler.
    occlusion_applied: bool,
    last: VisionSampleStats,
}

impl SharedVision {
    fn new(shape: GridShape) -> Self {
        Self {
            shape,
            columns: (0..shape.width).map(|x| shape.sample_x(x)).collect(),
            rows: (0..shape.height).map(|y| shape.sample_y(y)).collect(),
            viewers: BTreeMap::new(),
            radii: Box::default(),
            ready: false,
            occlusion_applied: false,
            last: VisionSampleStats {
                cell_visits: 0,
                emitted_sources: 0,
                candidates_scanned: 0,
            },
        }
    }

    /// Chooses the cheapest exact update from a viewer's previous sources to `sources`.
    fn plan_update(
        previous: Option<&ViewerGrid>,
        sources: &[VisionSource],
        rects: &[Rect],
    ) -> Update {
        let Some(previous) = previous else {
            return Update::Full;
        };
        if previous.sources.len() != sources.len() {
            return Update::Full;
        }
        let mut regions = Vec::new();
        for (index, (old, new)) in previous.sources.iter().zip(sources).enumerate() {
            if old == new {
                continue;
            }
            if regions.len() >= 2 * MAX_INCREMENTAL_SOURCES {
                return Update::Full;
            }
            regions.extend([previous.rects[index], rects[index]]);
        }
        regions.retain(|region| region.area() > 0);
        merge_overlapping(&mut regions);
        Update::Regions(regions)
    }

    fn apply(
        &mut self,
        plans: Vec<(HandleId, Vec<VisionSource>)>,
        work_limit: usize,
        candidates_scanned: usize,
    ) -> Result<VisionSampleStats, String> {
        let mut planned: Vec<Planned> = Vec::new();
        planned
            .try_reserve_exact(plans.len())
            .map_err(|_| "shared vision allocation")?;
        let mut cell_visits = 0usize;
        let mut emitted_sources = 0usize;
        for (id, sources) in plans {
            emitted_sources = emitted_sources
                .checked_add(sources.len())
                .ok_or("shared vision work overflow")?;
            let previous = self.viewers.get(&id);
            if previous.is_some_and(|viewer| viewer.sources == sources) {
                planned.push(Planned {
                    id,
                    sources,
                    rects: Vec::new(),
                    update: Update::Unchanged,
                });
                continue;
            }
            let mut rects = Vec::new();
            rects
                .try_reserve_exact(sources.len())
                .map_err(|_| "shared vision allocation")?;
            for source in &sources {
                rects.push(Rect::from_source(source.shape, &self.shape)?);
            }
            let update = Self::plan_update(previous, &sources, &rects);
            let area_sum = |rects: &[Rect]| {
                rects.iter().try_fold(0usize, |sum, rect| {
                    sum.checked_add(rect.area())
                        .ok_or("shared vision work overflow")
                })
            };
            let work = match &update {
                Update::Unchanged => 0,
                Update::Full => {
                    let reset = previous.map_or(Ok(0), |viewer| area_sum(&viewer.rects))?;
                    reset
                        .checked_add(area_sum(&rects)?)
                        .ok_or("shared vision work overflow")?
                }
                Update::Regions(regions) => {
                    let mut work = 0usize;
                    for region in regions {
                        let stamped = rects.iter().try_fold(0usize, |sum, rect| {
                            sum.checked_add(rect.intersection(*region).area())
                                .ok_or("shared vision work overflow")
                        })?;
                        work = work
                            .checked_add(region.area())
                            .and_then(|sum| sum.checked_add(stamped))
                            .ok_or("shared vision work overflow")?;
                    }
                    work
                }
            };
            cell_visits = cell_visits
                .checked_add(work)
                .ok_or("shared vision work overflow")?;
            planned.push(Planned {
                id,
                sources,
                rects,
                update,
            });
        }
        if cell_visits > work_limit {
            return Err("shared vision work budget".into());
        }
        let mut created = Vec::new();
        for plan in &planned {
            if matches!(plan.update, Update::Unchanged) || self.viewers.contains_key(&plan.id) {
                continue;
            }
            created.push((plan.id, ViewerGrid::blank(self.shape.cells())?));
        }
        let mut visits = 0usize;
        let width = self.shape.width;
        let mut live = Vec::with_capacity(planned.len());
        let mut applied = Vec::with_capacity(planned.len());
        for plan in planned {
            live.push(plan.id);
            if matches!(plan.update, Update::Unchanged) {
                continue;
            }
            let mut viewer = match self.viewers.remove(&plan.id) {
                Some(viewer) => viewer,
                None => {
                    let position = created
                        .iter()
                        .position(|(created_id, _)| *created_id == plan.id)
                        .expect("allocated viewer");
                    created.swap_remove(position).1
                }
            };
            match &plan.update {
                Update::Unchanged => {}
                Update::Full => {
                    for rect in std::mem::take(&mut viewer.rects) {
                        viewer.reset_rect(rect, width, &mut visits);
                    }
                    viewer.stamp(&plan.sources, &plan.rects, None, self, &mut visits);
                }
                Update::Regions(regions) => {
                    for &region in regions {
                        viewer.reset_rect(region, width, &mut visits);
                        viewer.stamp(&plan.sources, &plan.rects, Some(region), self, &mut visits);
                    }
                }
            }
            viewer.rects = plan.rects;
            viewer.sources = plan.sources;
            applied.push((plan.id, viewer));
        }
        debug_assert_eq!(visits, cell_visits);
        self.viewers.extend(applied);
        self.viewers.retain(|id, _| live.contains(id));
        self.ready = true;
        self.last = VisionSampleStats {
            cell_visits,
            emitted_sources,
            candidates_scanned,
        };
        Ok(self.last)
    }
}

pub(crate) enum VisionSlot {
    Blocked(VisionBlock),
    Live(SharedVision),
}

impl ObjectHost {
    /// Admit the placement grid when day/night classification is available.
    /// A missing prerequisite records a block and leaves any previous sample
    /// untouched only when vision is already live. Callers that are not live
    /// do not sample.
    pub fn admit_shared_vision(&mut self) -> Result<bool, Error> {
        if self.shared_vision_admitted() {
            return Ok(true);
        }
        let Some(placement) = self.unit_placement.as_ref() else {
            self.shared_vision = Some(VisionSlot::Blocked(VisionBlock::MissingPlacement));
            return Ok(false);
        };
        if self.vision_is_night().is_none() {
            self.shared_vision = Some(VisionSlot::Blocked(VisionBlock::MissingNight));
            return Ok(false);
        }
        let bounds = placement.world_bounds();
        if self.vision_world_bounds() != Some(bounds) {
            self.shared_vision = Some(VisionSlot::Blocked(VisionBlock::ExtentMismatch));
            return Ok(false);
        }
        let shape = match GridShape::from_bounds(bounds, placement.rules().cell_size()) {
            Ok(shape) => shape,
            Err(_) => {
                self.shared_vision = Some(VisionSlot::Blocked(VisionBlock::GridTooLarge));
                return Ok(false);
            }
        };
        self.shared_vision = Some(VisionSlot::Live(SharedVision::new(shape)));
        Ok(true)
    }

    pub fn shared_vision_admitted(&self) -> bool {
        matches!(self.shared_vision, Some(VisionSlot::Live(_)))
    }

    /// Why a minimap or later caller cannot read the ledger.
    pub fn vision_readiness(&self) -> Result<(), &'static str> {
        match &self.shared_vision {
            Some(VisionSlot::Live(vision)) if vision.ready => Ok(()),
            Some(VisionSlot::Live(_)) => Err("shared vision has not been sampled"),
            Some(VisionSlot::Blocked(reason)) => Err(reason.as_str()),
            None => Err("shared vision is not admitted"),
        }
    }

    /// Whether this world has a shared-vision ledger callers may use.
    ///
    /// `Unadmitted` keeps the caller's previous behavior. `Unavailable` refuses
    /// a new sight query before that query writes; work that does not query
    /// sight still runs. `Ready` carries the admitted fog switches and does
    /// not invent them.
    pub fn vision_capability(&self) -> VisionCapability {
        match &self.shared_vision {
            None => VisionCapability::Unadmitted,
            Some(VisionSlot::Blocked(reason)) => VisionCapability::Unavailable(reason.as_str()),
            Some(VisionSlot::Live(vision)) if !vision.ready => {
                VisionCapability::Unavailable("shared vision has not been sampled")
            }
            Some(VisionSlot::Live(_)) => match self.startup_settings() {
                Some(settings) => VisionCapability::Ready {
                    fog: settings.fog_enabled,
                    mask: settings.fog_mask_enabled,
                },
                None => VisionCapability::Unavailable("shared vision needs admitted fog settings"),
            },
        }
    }

    pub fn vision_occlusion_applied(&self) -> Option<bool> {
        match &self.shared_vision {
            Some(VisionSlot::Live(vision)) => Some(vision.occlusion_applied),
            _ => None,
        }
    }

    pub fn vision_sample_stats(&self) -> Option<VisionSampleStats> {
        match &self.shared_vision {
            Some(VisionSlot::Live(vision)) if vision.ready => Some(vision.last),
            _ => None,
        }
    }

    /// Cell-visit ceiling from the admitted grid and the current materialized
    /// viewers. Independent of timer expiry budgets.
    pub fn shared_vision_work_limit(&self) -> Result<usize, Error> {
        let fail = |detail: &str| {
            self.inner
                .error("vision", Failure::UnitActivity(detail.into()))
        };
        let Some(VisionSlot::Live(vision)) = &self.shared_vision else {
            return Err(fail("shared vision is not admitted"));
        };
        let viewers = self.materialized_player_ids().len().max(1);
        vision
            .shape
            .cells()
            .checked_mul(viewers)
            .and_then(|cells| cells.checked_mul(STAMP_COVERINGS))
            .ok_or_else(|| fail("shared vision work limit overflow"))
    }

    /// Sample every materialized player, including rows with no start location.
    /// Refuses without changing the previous ledger when the stamp budget is
    /// exceeded. An unchanged source list performs no cell writes.
    pub fn sample_shared_vision(&mut self, work_limit: usize) -> Result<VisionSampleStats, Error> {
        let fail = |detail: String| self.inner.error("vision", Failure::UnitActivity(detail));
        if !self.shared_vision_admitted() {
            return Err(fail(
                self.vision_readiness()
                    .err()
                    .unwrap_or("shared vision is not admitted")
                    .into(),
            ));
        }
        let Some(night) = self.vision_is_night() else {
            return Err(fail("shared vision needs admitted day/night sight".into()));
        };
        let placement = self
            .unit_placement
            .as_ref()
            .ok_or_else(|| fail("shared vision needs admitted placement".into()))?;
        let bounds = placement.world_bounds();
        if self.vision_world_bounds() != Some(bounds) {
            return Err(fail(
                "shared vision extent differs from the admitted placement grid".into(),
            ));
        }
        let shape = GridShape::from_bounds(bounds, placement.rules().cell_size()).map_err(fail)?;
        let VisionSlot::Live(vision) = self.shared_vision.as_ref().expect("admitted") else {
            return Err(fail("shared vision is not admitted".into()));
        };
        if vision.shape != shape {
            return Err(fail("shared vision grid changed after admission".into()));
        }
        let units = self.executed_units().count();
        let started_modifiers = self
            .fog_modifiers()
            .filter(|modifier| modifier.started)
            .count();
        let source_limit = units
            .saturating_add(self.fog_modifiers().count())
            .saturating_add(1);
        let mut plans = Vec::new();
        let mut candidates_scanned = 0usize;
        let viewers = self.materialized_player_ids();
        let mut radii = match self.shared_vision.as_mut() {
            Some(VisionSlot::Live(vision)) => std::mem::take(&mut *vision.radii),
            _ => super::SightRadii::default(),
        };
        plans
            .try_reserve_exact(viewers.len())
            .map_err(|_| fail("shared vision allocation".into()))?;
        for viewer in viewers {
            let listed = self
                .list_vision_sources(viewer, night, source_limit, &mut radii)
                .map_err(|error| fail(error.to_string()))?;
            candidates_scanned = candidates_scanned
                .checked_add(listed.candidates_scanned)
                .ok_or_else(|| fail("shared vision scan count overflow".into()))?;
            debug_assert_eq!(
                listed.candidates_scanned,
                units.saturating_add(started_modifiers)
            );
            plans.push((viewer, listed.sources));
        }
        let VisionSlot::Live(vision) = self.shared_vision.as_mut().expect("admitted") else {
            return Err(fail("shared vision is not admitted".into()));
        };
        let stats = vision
            .apply(plans, work_limit, candidates_scanned)
            .map_err(fail)?;
        *vision.radii = radii.retained();
        Ok(stats)
    }

    /// Raw cell at a world point. `Ok(None)` is outside the admitted extent.
    /// Null-handle policy belongs to a future native wrapper, not this query.
    pub fn vision_cell(
        &self,
        viewer: HandleId,
        point: [f32; 2],
    ) -> Result<Option<VisionCell>, Error> {
        let fail = |detail: &str| {
            self.inner
                .error("vision", Failure::UnitActivity(detail.into()))
        };
        self.vision_readiness().map_err(fail)?;
        let VisionSlot::Live(vision) = self.shared_vision.as_ref().expect("ready") else {
            return Err(fail("shared vision is not admitted"));
        };
        let Some(x) = vision.shape.index_x(f64::from(point[0])) else {
            return Ok(None);
        };
        let Some(y) = vision.shape.index_y(f64::from(point[1])) else {
            return Ok(None);
        };
        let grid = vision
            .viewers
            .get(&viewer)
            .ok_or_else(|| fail("viewer has no shared vision sample"))?;
        let at = y * vision.shape.width + x;
        Ok(Some(VisionCell {
            raw: grid.current[at],
            explored: grid.explored[at],
        }))
    }

    pub fn vision_presented(
        &self,
        viewer: HandleId,
        point: [f32; 2],
        fog: bool,
        mask: bool,
    ) -> Result<Option<VisionState>, Error> {
        Ok(self
            .vision_cell(viewer, point)?
            .map(|cell| present_vision(cell, fog, mask)))
    }
}

#[cfg(test)]
mod tests {
    use super::super::{VisionOrigin, VisionShape};
    use super::*;

    fn id(token: u64) -> HandleId {
        HandleId::new(std::num::NonZeroU64::new(token).unwrap())
    }

    fn grid(bounds: [[f32; 2]; 2], pitch: f32) -> SharedVision {
        SharedVision::new(GridShape::from_bounds(bounds, pitch).unwrap())
    }

    fn circle(center: [f32; 2], radius: f32, state: VisionState) -> VisionSource {
        VisionSource {
            shape: VisionShape::Circle { center, radius },
            state,
            after_units: false,
            origin: VisionOrigin::Unit { unit: id(1) },
            source_height: None,
        }
    }

    #[test]
    fn partial_extent_keeps_the_final_cell_and_rejects_the_outside() {
        let shape = GridShape::from_bounds([[0., 0.], [10., 10.]], 4.).unwrap();
        assert_eq!((shape.width, shape.height), (3, 3));
        assert_eq!(shape.index_x(9.5), Some(2));
        assert_eq!(shape.index_x(10.), Some(2));
        assert_eq!(shape.index_x(10.1), None);
        assert_eq!(shape.index_x(-0.01), None);
        let sample = shape.sample_x(2);
        assert!(sample > 8. && sample < 10.);
    }

    #[test]
    fn stamps_explore_only_covered_cells_and_switches_do_not_write_history() {
        let mut vision = grid([[0., 0.], [4., 4.]], 1.);
        let viewer = id(7);
        let stats = vision
            .apply(
                vec![(viewer, vec![circle([0.5, 3.5], 0.6, VisionState::Visible)])],
                64,
                0,
            )
            .unwrap();
        assert!(stats.cell_visits > 0);
        assert!(stats.cell_visits < 16);
        let seen = cell(&vision, viewer, [0.5, 3.5]);
        assert_eq!(
            seen,
            VisionCell {
                raw: VisionState::Visible,
                explored: true
            }
        );
        assert_eq!(present_vision(seen, false, false), VisionState::Visible);
        let hidden = cell(&vision, viewer, [2.5, 2.5]);
        assert_eq!(
            hidden,
            VisionCell {
                raw: VisionState::Masked,
                explored: false
            }
        );
        assert_eq!(present_vision(hidden, false, false), VisionState::Visible);
        assert!(!hidden.explored);
        assert_eq!(present_vision(hidden, true, false), VisionState::Fogged);
        assert_eq!(present_vision(hidden, true, true), VisionState::Masked);
        assert_eq!(present_vision(seen, false, true), VisionState::Visible);
        let moved = vision
            .apply(
                vec![(viewer, vec![circle([3.5, 0.5], 0.6, VisionState::Visible)])],
                64,
                0,
            )
            .unwrap();
        assert!(moved.cell_visits >= 2);
        assert_eq!(
            cell(&vision, viewer, [0.5, 3.5]),
            VisionCell {
                raw: VisionState::Fogged,
                explored: true
            }
        );
        assert_eq!(cell(&vision, viewer, [3.5, 0.5]).raw, VisionState::Visible);
        assert!(!vision.occlusion_applied);
    }

    #[test]
    fn unchanged_sources_do_no_cell_work_and_a_refused_budget_keeps_history() {
        let mut vision = grid([[0., 0.], [64., 64.]], 1.);
        let viewer = id(3);
        let sources = [VisionSource {
            shape: VisionShape::Rectangle {
                bounds: [0., 0., 64., 64.],
            },
            state: VisionState::Visible,
            after_units: false,
            origin: VisionOrigin::Modifier { modifier: id(9) },
            source_height: None,
        }];
        let wide = std::iter::repeat_n(sources[0], 20).collect::<Vec<_>>();
        let first = vision
            .apply(vec![(viewer, wide.clone())], 262_144, 0)
            .unwrap();
        assert_eq!(first.cell_visits, 20 * 64 * 64);
        assert_eq!(first.emitted_sources, 20);
        assert_eq!(first.candidates_scanned, 0);
        assert!(first.cell_visits > 65_536);
        let again = vision.apply(vec![(viewer, wide)], 0, 40).unwrap();
        assert_eq!(again.cell_visits, 0);
        assert_eq!(again.emitted_sources, 20);
        assert_eq!(again.candidates_scanned, 40);
        let kept = vision.last;
        let err = vision
            .apply(
                vec![(viewer, vec![circle([1.5, 1.5], 0.4, VisionState::Visible)])],
                0,
                99,
            )
            .unwrap_err();
        assert!(err.contains("budget"));
        assert_eq!(cell(&vision, viewer, [0.5, 0.5]).raw, VisionState::Visible);
        assert_eq!(vision.last, kept);
    }

    #[test]
    fn masked_modifier_clears_exploration_and_unit_origin_is_not_required() {
        let mut vision = grid([[0., 0.], [2., 2.]], 1.);
        let viewer = id(4);
        let masked = VisionSource {
            shape: VisionShape::Circle {
                center: [0.5, 0.5],
                radius: 0.4,
            },
            state: VisionState::Masked,
            after_units: false,
            origin: VisionOrigin::Modifier { modifier: id(8) },
            source_height: None,
        };
        vision
            .apply(
                vec![(
                    viewer,
                    vec![circle([0.5, 0.5], 0.4, VisionState::Visible), masked],
                )],
                32,
                0,
            )
            .unwrap();
        assert_eq!(
            cell(&vision, viewer, [0.5, 0.5]),
            VisionCell {
                raw: VisionState::Masked,
                explored: false
            }
        );
    }

    /// The full-recompute update (reset every previous rectangle, stamp every source), kept as the
    /// oracle the regional update must match cell for cell.
    struct Oracle {
        explored: Vec<bool>,
        current: Vec<VisionState>,
        rects: Vec<Rect>,
    }

    impl Oracle {
        /// Applies `sources` and returns the cells visited.
        fn apply(&mut self, shape: &GridShape, sources: &[VisionSource]) -> usize {
            let mut visits = 0;
            for rect in std::mem::take(&mut self.rects) {
                for y in rect.y0..rect.y1 {
                    for x in rect.x0..rect.x1 {
                        visits += 1;
                        let at = y * shape.width + x;
                        self.current[at] = if self.explored[at] {
                            VisionState::Fogged
                        } else {
                            VisionState::Masked
                        };
                    }
                }
            }
            for source in sources {
                let rect = Rect::from_source(source.shape, shape).unwrap();
                for y in rect.y0..rect.y1 {
                    for x in rect.x0..rect.x1 {
                        visits += 1;
                        if source
                            .shape
                            .contains([shape.sample_x(x), shape.sample_y(y)])
                        {
                            let at = y * shape.width + x;
                            self.current[at] = source.state;
                            self.explored[at] = source.state != VisionState::Masked;
                        }
                    }
                }
                self.rects.push(rect);
            }
            visits
        }
    }

    struct Lcg(u64);

    impl Lcg {
        fn below(&mut self, bound: u64) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (self.0 >> 33) % bound
        }

        fn unit(&mut self, extent: f32) -> f32 {
            self.below(1000) as f32 / 1000.0 * extent
        }
    }

    fn random_source(random: &mut Lcg, width: f32, height: f32) -> VisionSource {
        let state = [
            VisionState::Visible,
            VisionState::Visible,
            VisionState::Fogged,
            VisionState::Masked,
        ][random.below(4) as usize];
        let (x, y) = (random.unit(width), random.unit(height));
        let shape = if random.below(5) == 0 {
            VisionShape::Rectangle {
                bounds: [x, y, x + random.unit(10.0), y + random.unit(10.0)],
            }
        } else {
            VisionShape::Circle {
                center: [x, y],
                radius: 0.5 + random.unit(8.0),
            }
        };
        VisionSource {
            shape,
            state,
            after_units: false,
            origin: VisionOrigin::Unit { unit: id(1) },
            source_height: None,
        }
    }

    #[test]
    fn regional_updates_match_a_full_recompute_cell_for_cell() {
        let bounds = [[0., 0.], [40., 30.]];
        let mut vision = grid(bounds, 1.);
        let shape = vision.shape;
        let mut oracle = Oracle {
            explored: vec![false; shape.cells()],
            current: vec![VisionState::Masked; shape.cells()],
            rects: Vec::new(),
        };
        let viewer = id(5);
        let mut random = Lcg(7);
        let mut sources: Vec<VisionSource> = (0..6)
            .map(|_| random_source(&mut random, 40.0, 30.0))
            .collect();
        let (mut regional_visits, mut full_visits) = (0, 0);
        for step in 0..400 {
            match random.below(10) {
                0..=5 => {
                    let index = random.below(sources.len() as u64) as usize;
                    let (dx, dy) = (random.unit(3.0) - 1.5, random.unit(3.0) - 1.5);
                    sources[index].shape = match sources[index].shape {
                        VisionShape::Circle { center, radius } => VisionShape::Circle {
                            center: [center[0] + dx, center[1] + dy],
                            radius,
                        },
                        VisionShape::Rectangle {
                            bounds: [a, b, c, d],
                        } => VisionShape::Rectangle {
                            bounds: [a + dx, b + dy, c + dx, d + dy],
                        },
                    };
                }
                6 => sources.push(random_source(&mut random, 40.0, 30.0)),
                7 if sources.len() > 1 => {
                    sources.remove(random.below(sources.len() as u64) as usize);
                }
                8 => {
                    let index = random.below(sources.len() as u64) as usize;
                    sources[index].state = [
                        VisionState::Visible,
                        VisionState::Fogged,
                        VisionState::Masked,
                    ][random.below(3) as usize];
                }
                _ => {
                    let (a, b) = (
                        random.below(sources.len() as u64) as usize,
                        random.below(sources.len() as u64) as usize,
                    );
                    sources.swap(a, b);
                }
            }
            let stats = vision
                .apply(vec![(viewer, sources.clone())], usize::MAX, 0)
                .unwrap();
            full_visits += oracle.apply(&shape, &sources);
            regional_visits += stats.cell_visits;
            let actual = vision.viewers.get(&viewer).unwrap();
            assert_eq!(actual.current, oracle.current, "step {step}");
            assert_eq!(actual.explored, oracle.explored, "step {step}");
        }
        assert!(
            regional_visits < full_visits,
            "regional updates did not reduce work: {regional_visits} vs {full_visits}"
        );
    }

    fn cell(vision: &SharedVision, viewer: HandleId, point: [f32; 2]) -> VisionCell {
        let x = vision.shape.index_x(f64::from(point[0])).unwrap();
        let y = vision.shape.index_y(f64::from(point[1])).unwrap();
        let grid = vision.viewers.get(&viewer).unwrap();
        let at = y * vision.shape.width + x;
        VisionCell {
            raw: grid.current[at],
            explored: grid.explored[at],
        }
    }
}

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
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

/// Application allocation ceiling for one viewer's grid. Not a map constant.
const MAX_GRID_CELLS: usize = 4_000_000;
/// How many full-grid stamp coverings one sample may perform across viewers.
/// Not a game rule. The charge is the touched rectangles, not viewers times
/// every cell times every source.
const STAMP_COVERINGS: usize = 64;

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

    fn contains(self, x: usize, y: usize) -> bool {
        (self.x0..self.x1).contains(&x) && (self.y0..self.y1).contains(&y)
    }
}

/// What one source shape covers on the grid: its bounding rectangle and the run of covered cells
/// in each row of it. A function of the shape alone, so every viewer that lists the same unit
/// shares one footprint.
struct Footprint {
    rect: Rect,
    /// Covered cells `(x0, x1)` of row `rect.y0 + n`; `x0 == x1` when none.
    runs: Vec<(usize, usize)>,
}

impl Footprint {
    /// Covered cells of row `y`, empty outside the rectangle.
    fn run(&self, y: usize) -> (usize, usize) {
        y.checked_sub(self.rect.y0)
            .and_then(|n| self.runs.get(n))
            .copied()
            .unwrap_or((0, 0))
    }
}

/// Bit pattern of a shape: equal shapes share one footprint.
type ShapeKey = [u32; 5];

fn shape_key(shape: VisionShape) -> ShapeKey {
    match shape {
        VisionShape::Circle { center, radius } => [
            0,
            center[0].to_bits(),
            center[1].to_bits(),
            radius.to_bits(),
            0,
        ],
        VisionShape::Rectangle {
            bounds: [a, b, c, d],
        } => [1, a.to_bits(), b.to_bits(), c.to_bits(), d.to_bits()],
    }
}

struct ViewerGrid {
    explored: Vec<bool>,
    current: Vec<VisionState>,
    /// Footprint of each entry of `sources`, in the same order.
    footprints: Vec<Arc<Footprint>>,
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
            footprints: Vec::new(),
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

    /// Stamps `sources` in order. Each row of a source writes its contiguous covered run.
    fn stamp(
        &mut self,
        sources: &[VisionSource],
        footprints: &[Arc<Footprint>],
        vision: &SharedVision,
        visits: &mut usize,
    ) {
        let width = vision.shape.width;
        for (source, footprint) in sources.iter().zip(footprints) {
            let rect = footprint.rect;
            let explored = source.state != VisionState::Masked;
            for y in rect.y0..rect.y1 {
                *visits += rect.x1.saturating_sub(rect.x0);
                let (x0, x1) = footprint.run(y);
                let row = y * width;
                self.current[row + x0..row + x1].fill(source.state);
                self.explored[row + x0..row + x1].fill(explored);
            }
        }
    }

    /// Sets one cell to what stamping `sources` in order would leave there: the last source that
    /// covers it, else the previous exploration without current sight.
    fn recompute(
        &mut self,
        sources: &[VisionSource],
        footprints: &[Arc<Footprint>],
        vision: &SharedVision,
        x: usize,
        y: usize,
    ) {
        let at = y * vision.shape.width + x;
        let point = [vision.columns[x], vision.rows[y]];
        let covering = sources
            .iter()
            .zip(footprints)
            .rev()
            .find(|(source, footprint)| {
                footprint.rect.contains(x, y) && source.shape.contains(point)
            });
        match covering {
            Some((source, _)) => {
                self.current[at] = source.state;
                self.explored[at] = source.state != VisionState::Masked;
            }
            None => {
                self.current[at] = if self.explored[at] {
                    VisionState::Fogged
                } else {
                    VisionState::Masked
                };
            }
        }
    }
}

/// Cells `x0..x1` of row `y`.
#[derive(Clone, Copy, Debug)]
struct Run {
    y: usize,
    x0: usize,
    x1: usize,
}

/// How one viewer's grid reaches the new source list.
enum Update {
    Unchanged,
    /// Reset every previous rectangle and stamp every source.
    Full,
    /// Recompute only these cells. Same-length source lists differ only at changed indices, and a
    /// cell's value depends only on which sources cover it, so cells whose coverage by every
    /// changed source (and its state) is the same before and after keep their value.
    Cells(Vec<Run>),
}

struct Planned {
    id: HandleId,
    sources: Vec<VisionSource>,
    footprints: Vec<Arc<Footprint>>,
    update: Update,
}

pub(crate) struct SharedVision {
    shape: GridShape,
    /// Sample coordinate of each column's cell centre.
    columns: Vec<f64>,
    /// Sample coordinate of each row's cell centre.
    rows: Vec<f64>,
    viewers: BTreeMap<HandleId, ViewerGrid>,
    /// Footprints of the shapes in the latest sample, kept so an unchanged shape is not
    /// covered again.
    footprints: HashMap<ShapeKey, Arc<Footprint>>,
    radii: Box<super::SightRadii>,
    /// Counts samples that changed any viewer's cells or the set of viewers.
    revision: u64,
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
            footprints: HashMap::new(),
            radii: Box::default(),
            ready: false,
            revision: 0,
            occlusion_applied: false,
            last: VisionSampleStats {
                cell_visits: 0,
                emitted_sources: 0,
                candidates_scanned: 0,
            },
        }
    }

    /// Cells of row `y` inside `rect` that `shape` covers. They form one run, found by binary
    /// search over the column table with the exact tests `VisionShape::contains` applies: a
    /// circle's test only gets harder with distance from its centre column on either side, and a
    /// rectangle's bounds are two thresholds. An empty run is `(rect.x0, rect.x0)`.
    fn row_run(&self, shape: VisionShape, rect: Rect, y: usize) -> (usize, usize) {
        let sy = self.rows[y];
        let columns = &self.columns[rect.x0..rect.x1];
        let (first, end) = match shape {
            VisionShape::Circle { center, radius } => {
                let cx = f64::from(center[0]);
                let dy2 = (sy - f64::from(center[1])).powi(2);
                let radius2 = f64::from(radius).powi(2);
                let inside = |column: f64| (column - cx).powi(2) + dy2 <= radius2;
                let (left, right) = columns.split_at(columns.partition_point(|&c| c <= cx));
                (
                    left.partition_point(|&c| !inside(c)),
                    left.len() + right.partition_point(|&c| inside(c)),
                )
            }
            VisionShape::Rectangle {
                bounds: [x0, y0, x1, y1],
            } => {
                if (f64::from(y0)..=f64::from(y1)).contains(&sy) {
                    (
                        columns.partition_point(|&c| c < f64::from(x0)),
                        columns.partition_point(|&c| c <= f64::from(x1)),
                    )
                } else {
                    (0, 0)
                }
            }
        };
        if first < end {
            (rect.x0 + first, rect.x0 + end)
        } else {
            (rect.x0, rect.x0)
        }
    }

    /// Appends runs that include every cell where `old` and `new` differ in what they stamp:
    /// cells exactly one of them covers, or every cell either covers when their states differ.
    fn changed_runs(
        old: &VisionSource,
        old_footprint: &Footprint,
        new: &VisionSource,
        new_footprint: &Footprint,
        runs: &mut Vec<Run>,
    ) {
        let (old_rect, new_rect) = (old_footprint.rect, new_footprint.rect);
        let (first, last) = [old_rect, new_rect]
            .into_iter()
            .filter(|rect| rect.y0 < rect.y1)
            .fold((usize::MAX, 0), |(first, last), rect| {
                (first.min(rect.y0), last.max(rect.y1))
            });
        let same_state = old.state == new.state;
        for y in first..last {
            let (old_run, new_run) = (old_footprint.run(y), new_footprint.run(y));
            let mut push = |(x0, x1): (usize, usize)| {
                if x0 < x1 {
                    runs.push(Run { y, x0, x1 });
                }
            };
            let overlapping = old_run.0.max(new_run.0) < old_run.1.min(new_run.1);
            if same_state && overlapping {
                push((old_run.0.min(new_run.0), old_run.0.max(new_run.0)));
                push((old_run.1.min(new_run.1), old_run.1.max(new_run.1)));
            } else {
                push(old_run);
                push(new_run);
            }
        }
    }

    /// The footprint of `shape`, from this sample's shapes or the previous sample's, else computed.
    fn footprint(
        &self,
        current: &mut HashMap<ShapeKey, Arc<Footprint>>,
        shape: VisionShape,
    ) -> Result<Arc<Footprint>, String> {
        let key = shape_key(shape);
        if let Some(known) = current.get(&key) {
            return Ok(Arc::clone(known));
        }
        let footprint = match self.footprints.get(&key) {
            Some(known) => Arc::clone(known),
            None => {
                let rect = Rect::from_source(shape, &self.shape)?;
                Arc::new(Footprint {
                    rect,
                    runs: (rect.y0..rect.y1)
                        .map(|y| self.row_run(shape, rect, y))
                        .collect(),
                })
            }
        };
        current.insert(key, Arc::clone(&footprint));
        Ok(footprint)
    }

    /// Chooses the cheapest exact update from a viewer's previous sources to `sources`.
    fn plan_update(
        previous: Option<&ViewerGrid>,
        sources: &[VisionSource],
        footprints: &[Arc<Footprint>],
    ) -> Update {
        let Some(previous) = previous else {
            return Update::Full;
        };
        if previous.sources.len() != sources.len() {
            return Update::Full;
        }
        let mut runs = Vec::new();
        for (index, (old, new)) in previous.sources.iter().zip(sources).enumerate() {
            if old != new {
                Self::changed_runs(
                    old,
                    &previous.footprints[index],
                    new,
                    &footprints[index],
                    &mut runs,
                );
            }
        }
        // Each recomputed cell scans at most every source; the full update touches each
        // previous and new rectangle once.
        let recomputed = runs.iter().map(|run| run.x1 - run.x0).sum::<usize>();
        let full = previous
            .footprints
            .iter()
            .chain(footprints)
            .map(|footprint| footprint.rect.area())
            .fold(0usize, usize::saturating_add);
        if recomputed.saturating_mul(sources.len()) > full {
            Update::Full
        } else {
            Update::Cells(runs)
        }
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
        let mut current_footprints = HashMap::new();
        for (id, sources) in plans {
            emitted_sources = emitted_sources
                .checked_add(sources.len())
                .ok_or("shared vision work overflow")?;
            let previous = self.viewers.get(&id);
            if let Some(viewer) = previous.filter(|viewer| viewer.sources == sources) {
                for (source, footprint) in viewer.sources.iter().zip(&viewer.footprints) {
                    current_footprints
                        .entry(shape_key(source.shape))
                        .or_insert_with(|| Arc::clone(footprint));
                }
                planned.push(Planned {
                    id,
                    sources,
                    footprints: Vec::new(),
                    update: Update::Unchanged,
                });
                continue;
            }
            let mut footprints = Vec::new();
            footprints
                .try_reserve_exact(sources.len())
                .map_err(|_| "shared vision allocation")?;
            for source in &sources {
                footprints.push(self.footprint(&mut current_footprints, source.shape)?);
            }
            let update = Self::plan_update(previous, &sources, &footprints);
            let area_sum = |footprints: &[Arc<Footprint>]| {
                footprints.iter().try_fold(0usize, |sum, footprint| {
                    sum.checked_add(footprint.rect.area())
                        .ok_or("shared vision work overflow")
                })
            };
            let work = match &update {
                Update::Unchanged => 0,
                Update::Full => {
                    let reset = previous.map_or(Ok(0), |viewer| area_sum(&viewer.footprints))?;
                    reset
                        .checked_add(area_sum(&footprints)?)
                        .ok_or("shared vision work overflow")?
                }
                Update::Cells(runs) => runs.iter().try_fold(0usize, |sum, run| {
                    sum.checked_add(run.x1 - run.x0)
                        .ok_or("shared vision work overflow")
                })?,
            };
            cell_visits = cell_visits
                .checked_add(work)
                .ok_or("shared vision work overflow")?;
            planned.push(Planned {
                id,
                sources,
                footprints,
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
        let mut changed = false;
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
                    for footprint in std::mem::take(&mut viewer.footprints) {
                        viewer.reset_rect(footprint.rect, width, &mut visits);
                    }
                    viewer.stamp(&plan.sources, &plan.footprints, self, &mut visits);
                }
                Update::Cells(runs) => {
                    for run in runs {
                        visits += run.x1 - run.x0;
                        for x in run.x0..run.x1 {
                            viewer.recompute(&plan.sources, &plan.footprints, self, x, run.y);
                        }
                    }
                }
            }
            changed |= match &plan.update {
                Update::Unchanged => false,
                Update::Full => true,
                Update::Cells(runs) => !runs.is_empty(),
            };
            viewer.footprints = plan.footprints;
            viewer.sources = plan.sources;
            applied.push((plan.id, viewer));
        }
        debug_assert_eq!(visits, cell_visits);
        self.viewers.extend(applied);
        self.footprints = current_footprints;
        let viewer_count = self.viewers.len();
        self.viewers.retain(|id, _| live.contains(id));
        changed |= self.viewers.len() != viewer_count;
        self.revision += u64::from(changed);
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
    Live(Box<SharedVision>),
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
        self.shared_vision = Some(VisionSlot::Live(Box::new(SharedVision::new(shape))));
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

    /// Count of samples that changed any viewer's raw cells or the set of viewers. While it is
    /// equal, every [`Self::vision_cell`] answer is equal, so a consumer can skip re-reading
    /// the field. Fog and mask switches are not part of it. `None` until the first sample.
    pub fn vision_revision(&self) -> Option<u64> {
        match &self.shared_vision {
            Some(VisionSlot::Live(vision)) if vision.ready => Some(vision.revision),
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
        let mut table = self
            .source_table(night)
            .map_err(|error| fail(error.to_string()))?;
        for viewer in viewers {
            let listed = self
                .list_vision_sources(viewer, source_limit, &mut radii, &mut table)
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

    fn random_source(
        random: &mut Lcg,
        width: f32,
        height: f32,
        rectangle_one_in: u64,
    ) -> VisionSource {
        let state = [
            VisionState::Visible,
            VisionState::Visible,
            VisionState::Fogged,
            VisionState::Masked,
        ][random.below(4) as usize];
        let (x, y) = (random.unit(width), random.unit(height));
        let shape = if random.below(rectangle_one_in) == 0 {
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

    /// One random mutation of a viewer's source list.
    fn mutate(random: &mut Lcg, sources: &mut Vec<VisionSource>, extent: [f32; 2], rect_odds: u64) {
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
            6 => sources.push(random_source(random, extent[0], extent[1], rect_odds)),
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
    }

    /// Runs `steps` random mutations through `apply` and the full-recompute oracle, comparing
    /// both grids after each. Returns the cells `apply` visited and the oracle visited.
    fn differential(
        extent: [f32; 2],
        pitch: f32,
        count: usize,
        rect_odds: u64,
        steps: usize,
        seed: u64,
    ) -> (usize, usize) {
        let mut vision = grid([[0., 0.], extent], pitch);
        let shape = vision.shape;
        let mut oracle = Oracle {
            explored: vec![false; shape.cells()],
            current: vec![VisionState::Masked; shape.cells()],
            rects: Vec::new(),
        };
        let viewer = id(5);
        let mut random = Lcg(seed);
        let mut sources: Vec<VisionSource> = (0..count)
            .map(|_| random_source(&mut random, extent[0], extent[1], rect_odds))
            .collect();
        let (mut incremental, mut full) = (0, 0);
        for step in 0..steps {
            mutate(&mut random, &mut sources, extent, rect_odds);
            let stats = vision
                .apply(vec![(viewer, sources.clone())], usize::MAX, 0)
                .unwrap();
            full += oracle.apply(&shape, &sources);
            incremental += stats.cell_visits;
            let actual = vision.viewers.get(&viewer).unwrap();
            assert_eq!(actual.current, oracle.current, "seed {seed} step {step}");
            assert_eq!(actual.explored, oracle.explored, "seed {seed} step {step}");
        }
        (incremental, full)
    }

    #[test]
    fn incremental_updates_match_a_full_recompute_cell_for_cell() {
        let (incremental, full) = differential([40., 30.], 1., 6, 5, 400, 7);
        assert!(
            incremental < full,
            "incremental updates did not reduce work: {incremental} vs {full}"
        );
        // Partial final cells, circle-only lists and crowded lists.
        for seed in 0..6 {
            differential([40.3, 30.7], 1.7, 12, 5, 300, seed);
            differential([64., 48.], 1., 20, u64::MAX, 300, 100 + seed);
            differential([25., 25.], 0.5, 40, 8, 200, 200 + seed);
        }
    }

    #[test]
    fn a_moving_circle_among_many_recomputes_only_its_crescent() {
        let mut vision = grid([[0., 0.], [200., 200.]], 1.);
        let viewer = id(6);
        let mut sources: Vec<VisionSource> = (0..30)
            .map(|n| circle([20. + 5. * n as f32, 100.], 30., VisionState::Visible))
            .collect();
        vision
            .apply(vec![(viewer, sources.clone())], usize::MAX, 0)
            .unwrap();
        sources[4].shape = VisionShape::Circle {
            center: [40.5, 100.],
            radius: 30.,
        };
        let moved = vision
            .apply(vec![(viewer, sources)], usize::MAX, 0)
            .unwrap();
        assert!(
            (1..200).contains(&moved.cell_visits),
            "{} cells",
            moved.cell_visits
        );
    }

    #[test]
    fn row_runs_equal_the_cell_test_for_every_row() {
        let vision = grid([[0., 0.], [37.3, 21.1]], 1.3);
        let whole = Rect {
            x0: 0,
            x1: vision.shape.width,
            y0: 0,
            y1: vision.shape.height,
        };
        let mut random = Lcg(11);
        let mut shapes = vec![
            // Radius zero, centre on a sample column, centre outside the grid, huge radius.
            VisionShape::Circle {
                center: [vision.columns[3] as f32, vision.rows[2] as f32],
                radius: 0.,
            },
            VisionShape::Circle {
                center: [-5., -5.],
                radius: 9.,
            },
            VisionShape::Circle {
                center: [18., 10.],
                radius: 1000.,
            },
            VisionShape::Rectangle {
                bounds: [3., 3., 3., 3.],
            },
        ];
        for _ in 0..300 {
            shapes.push(random_source(&mut random, 37.3, 21.1, 3).shape);
        }
        for shape in shapes {
            let rect = Rect::from_source(shape, &vision.shape).unwrap();
            for y in rect.y0..rect.y1 {
                let (x0, x1) = vision.row_run(shape, rect, y);
                for x in rect.x0..rect.x1 {
                    let covered = shape.contains([vision.columns[x], vision.rows[y]]);
                    assert_eq!((x0..x1).contains(&x), covered, "{shape:?} cell {x},{y}");
                }
            }
            for y in 0..vision.shape.height {
                let (x0, x1) = vision.row_run(shape, whole, y);
                let covered = (0..vision.shape.width)
                    .filter(|&x| shape.contains([vision.columns[x], vision.rows[y]]))
                    .collect::<Vec<_>>();
                assert_eq!(covered, (x0..x1).collect::<Vec<_>>(), "{shape:?} row {y}");
            }
        }
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

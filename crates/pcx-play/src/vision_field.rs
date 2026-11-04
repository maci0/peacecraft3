//! Shared per-viewer vision grid. Exploration changes only when a sample stamps
//! cells. Fog and mask switches are applied by [`present_vision`] and do not
//! write exploration.
//!
//! Campaigns with selected client rules use its lattice, height plane and
//! predecessor graph. Other callers retain the labelled height/ray approximation.
//! Exploration and the public cell queries share this ledger; presentation
//! switches do not write explored history.
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

/// Occlusion inputs for one stamp: the pass plus the per-source sight
/// origins, in source order.
#[derive(Clone, Copy, Debug)]
struct StampOcclusion<'a> {
    input: &'a OcclusionInput<'a>,
    eyes: &'a [Option<[f64; 3]>],
    /// Per-plan relevant tree indices; the same subset the pre-check charged.
    trees: &'a [usize],
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
    /// Occluded cells leave earlier sources and exploration unchanged.
    fn stamp(
        &mut self,
        sources: &[VisionSource],
        footprints: &[Arc<Footprint>],
        vision: &SharedVision,
        occlusion: Option<StampOcclusion<'_>>,
        visits: &mut usize,
    ) {
        let width = vision.shape.width;
        for (index, (source, footprint)) in sources.iter().zip(footprints).enumerate() {
            let rect = footprint.rect;
            let explored = source.state != VisionState::Masked;
            let eye = occlusion.and_then(|stamp| stamp.eyes.get(index).copied().flatten());
            for y in rect.y0..rect.y1 {
                *visits += rect.x1.saturating_sub(rect.x0);
                let (x0, x1) = footprint.run(y);
                let row = y * width;
                match (occlusion, eye) {
                    (Some(stamp), Some(eye)) => {
                        for x in x0..x1 {
                            *visits += OcclusionInput::charge_for(stamp.trees.len());
                            let point = [vision.columns[x], vision.rows[y]];
                            if !stamp
                                .input
                                .blocked(eye, point, vision.shape.pitch, stamp.trees)
                            {
                                self.current[row + x] = source.state;
                                self.explored[row + x] = explored;
                            }
                        }
                    }
                    _ => {
                        self.current[row + x0..row + x1].fill(source.state);
                        self.explored[row + x0..row + x1].fill(explored);
                    }
                }
            }
        }
    }

    /// Sets one cell to what stamping `sources` in order would leave there: the last source that
    /// covers it, else the previous exploration without current sight. Occlusion
    /// applies exactly as in [`Self::stamp`].
    fn recompute(
        &mut self,
        sources: &[VisionSource],
        footprints: &[Arc<Footprint>],
        vision: &SharedVision,
        occlusion: Option<StampOcclusion<'_>>,
        cell: [usize; 2],
    ) {
        let [x, y] = cell;
        let at = y * vision.shape.width + x;
        let point = [vision.columns[x], vision.rows[y]];
        let covering =
            sources
                .iter()
                .zip(footprints)
                .enumerate()
                .rev()
                .find(|(_, (source, footprint))| {
                    footprint.rect.contains(x, y) && source.shape.contains(point)
                });
        match covering {
            Some((index, (source, _))) => {
                let eye = occlusion.and_then(|stamp| stamp.eyes.get(index).copied().flatten());
                match (occlusion, eye) {
                    (Some(stamp), Some(eye))
                        if stamp
                            .input
                            .blocked(eye, point, vision.shape.pitch, stamp.trees) =>
                    {
                        self.current[at] = VisionState::Fogged;
                    }
                    _ => {
                        self.current[at] = source.state;
                        self.explored[at] = source.state != VisionState::Masked;
                    }
                }
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
    /// Sight origin per source, in the same order; empty when no occlusion
    /// input was supplied or the plan needs no stamping.
    eyes: Vec<Option<[f64; 3]>>,
    /// Admitted tree indices that can shadow this plan's cells. Only these
    /// are tested and charged; anything farther cannot intersect an
    /// eye-to-cell segment drawn for these sources (see
    /// `OcclusionInput::relevant`).
    trees: Vec<usize>,
    update: Update,
}

// Sight occlusion against the admitted heightfield and live tree discs.
//
// Native evidence constrains but does not specify the sampling: the fog
// updater `658ac0` calls `657340`, which samples two visibility grids through
// a shared mask, and the per-unit mask-word gate at `65bf63..65bf20` varies
// with the size class (`docs/re-notes/campaign-minimap.md:122-144,322-329`).
// shortcut: callers without selected native sight rules retain this approximate
// height/ray sampler; migrate them when their client sight policy is qualified.
// Campaign startup supplies the native lattice and blocker graph instead.
//
// What is admitted and therefore not a choice: the corner heightfield
// (`height_index`, `layer_height`) from the admitted map context, the
// `[tile, bias, factor]` terrain encoding from the admitted placement rules,
// live tree positions and aliveness from the destructable ledger with `tree`
// from the admitted `targType` record, and the per-viewer 2D origin and
// radius from the existing source list.

/// Eye height above the viewer's own ground when
/// [`VisionSource::source_height`] is `None`. Live records never supply one,
/// so no qualified native value exists; this is a recreation choice.
const VIEWER_EYE_ABOVE_GROUND: f32 = 128.0;
/// Tree-disc radius per unit of admitted destructable `scale`. The catalog
/// carries `targType=tree` and `scale` but no occluder radius, so the radius
/// is derived transparently as `scale * TREE_RADIUS_PER_SCALE` (see
/// [`tree_occluder`]); the per-scale unit is the recreation choice.
const TREE_RADIUS_PER_SCALE: f64 = 48.0;
/// Canopy top above the tree base per unit of admitted destructable `scale`.
/// Same provenance caveat as the radius.
const TREE_CANOPY_PER_SCALE: f64 = 192.0;
/// W3E elevation-layer encoding bias: a corner's world height adds
/// `(layer - 2) * tile`. Format invariant, cf. `pcx-tera/src/world.rs`
/// (`ZERO_LAYER`); not a per-map authored value.
const W3E_LAYER_BIAS: f64 = 2.0;
/// Ray-march samples per occludable cell, charged to the stamp budget as an
/// upper bound and clamped from the ray length. Fixed-count sampling is the
/// recreation choice; the exact native pattern is undecoded (see above).
const OCCLUSION_STEPS_MAX: usize = 32;
/// Short rays still test at least this many interior points.
const OCCLUSION_STEPS_MIN: usize = 4;
/// A blocker shadows only when it clears the sight ray by more than this, in
/// world units. Numerical tolerance only; gameplay heights are orders above it.
const OCCLUSION_TOLERANCE: f64 = 0.001;

/// One live tree as a sight occluder: admitted position, admitted-scale radius
/// and canopy, base sampled from the admitted heightfield.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TreeOccluder {
    pub x: f64,
    pub y: f64,
    pub radius: f64,
    pub base: f64,
    pub top: f64,
}

/// Admitted inputs to the occlusion pass: the map heightfield, its placement
/// encoding, and the live tree discs. `None` (no input) leaves
/// `occlusion_applied` false.
#[derive(Clone, Copy, Debug)]
pub(crate) struct OcclusionInput<'a> {
    pub terrain: &'a pcx_map::Terrain,
    pub encoding: [f32; 3],
    pub trees: &'a [TreeOccluder],
}

/// Bilinear terrain height under a world point from the admitted corner grid.
/// Mirrors the mesh surface the renderer draws (`pcx-tera` `sample_world_z`
/// over `Mapping::sample_ground`): `height_index / factor - bias` plus the
/// elevation-layer offset, with the admitted `terrain_encoding`
/// `[tile, bias, factor]` from the placement rules. `None` on unadmitted
/// encoding or outside the corner grid.
fn ground_z(terrain: &pcx_map::Terrain, encoding: [f32; 3], x: f64, y: f64) -> Option<f64> {
    let [tile, bias, factor] = encoding;
    if !(tile.is_finite() && tile > 0.0 && factor.is_finite() && factor > 0.0 && bias.is_finite()) {
        return None;
    }
    let (origin_x, origin_y) = terrain.origin();
    if !(origin_x.is_finite() && origin_y.is_finite()) {
        return None;
    }
    let tile = f64::from(tile);
    let point_x = (x - f64::from(origin_x)) / tile;
    let point_y = (y - f64::from(origin_y)) / tile;
    if !(point_x >= 0.0 && point_y >= 0.0) {
        return None;
    }
    let (width, height) = (terrain.width(), terrain.height());
    if point_x > f64::from(width) || point_y > f64::from(height) {
        return None;
    }
    let (cell_x, cell_y) = (point_x.floor() as u32, point_y.floor() as u32);
    if cell_x >= width || cell_y >= height {
        return None;
    }
    let corner_height = |corner: &pcx_map::Corner| {
        f64::from(corner.height_index) / f64::from(factor) - f64::from(bias)
            + (f64::from(corner.layer_height()) - W3E_LAYER_BIAS) * tile
    };
    let (dx, dy) = (point_x - f64::from(cell_x), point_y - f64::from(cell_y));
    let (south_west, south_east, north_west, north_east) = (
        terrain.corner(cell_x, cell_y)?,
        terrain.corner(cell_x + 1, cell_y)?,
        terrain.corner(cell_x, cell_y + 1)?,
        terrain.corner(cell_x + 1, cell_y + 1)?,
    );
    let south = corner_height(south_west) * (1.0 - dx) + corner_height(south_east) * dx;
    let north = corner_height(north_west) * (1.0 - dx) + corner_height(north_east) * dx;
    Some(south * (1.0 - dy) + north * dy)
}

/// Live tree disc from admitted creation data: position and `scale` from the
/// executed destructable record, `tree` from the admitted `targType` record
/// (`destructable_host.rs` `stats_if_destructable`), base from the admitted
/// heightfield. Radius and canopy are the labelled recreation choices above.
/// `None` (fail-open: the tree casts no shadow) when any input is unusable.
pub(crate) fn tree_occluder(
    terrain: &pcx_map::Terrain,
    encoding: [f32; 3],
    creation: &super::super::DestructableCreation,
) -> Option<TreeOccluder> {
    if !creation.scale.is_finite() || creation.scale <= 0.0 {
        return None;
    }
    if ![creation.x, creation.y]
        .iter()
        .all(|value| value.is_finite())
    {
        return None;
    }
    let (x, y) = (f64::from(creation.x), f64::from(creation.y));
    let base = ground_z(terrain, encoding, x, y)?;
    let scale = f64::from(creation.scale);
    let radius = scale * TREE_RADIUS_PER_SCALE;
    let top = base + scale * TREE_CANOPY_PER_SCALE;
    if !(radius.is_finite() && radius > 0.0 && top.is_finite() && top > base) {
        return None;
    }
    Some(TreeOccluder {
        x,
        y,
        radius,
        base,
        top,
    })
}

impl OcclusionInput<'_> {
    /// Per-cell budget charge for a plan: the ray-march bound plus one test
    /// per tree in that plan's relevant subset. The pre-check and the stamp
    /// below add exactly this per occludable cell, so `cell_visits` stays
    /// exact and the budget refusal still protects.
    fn charge_for(tree_count: usize) -> usize {
        OCCLUSION_STEPS_MAX.saturating_add(tree_count)
    }

    /// Indices of admitted trees that can shadow cells stamped for these
    /// sources. A tree farther than its canopy plus one cell from every
    /// eye-bearing source centre (sight radius included) cannot intersect any
    /// eye-to-cell segment drawn for this plan: the segment stays within the
    /// sight radius of its eye, so the closest approach exceeds the canopy
    /// radius. Excluding farther trees changes neither the ledger (they can
    /// never block) nor the budget accounting (pre-check and stamp use this
    /// same subset). Sources without eyes stamp unoccluded and contribute
    /// no occludable cells, so only eye-bearing sources qualify trees.
    fn relevant(
        &self,
        sources: &[VisionSource],
        eyes: &[Option<[f64; 3]>],
        pitch: f64,
    ) -> Vec<usize> {
        debug_assert_eq!(sources.len(), eyes.len());
        let mut out = Vec::new();
        'trees: for (index, tree) in self.trees.iter().enumerate() {
            if !tree.x.is_finite()
                || !tree.y.is_finite()
                || !tree.radius.is_finite()
                || tree.radius <= 0.0
            {
                // `blocked` skips these too, so they can never shadow.
                continue;
            }
            for (source, eye) in sources.iter().zip(eyes) {
                if eye.is_none() {
                    continue;
                }
                let (center, radius) = match source.shape {
                    VisionShape::Circle { center, radius } => {
                        (center.map(f64::from), f64::from(radius))
                    }
                    // Modifiers stamp unoccluded and carry no eye, but a
                    // rectangular source still bounds possibly-shadowed cells;
                    // its half-diagonal is a safe over-approximation.
                    VisionShape::Rectangle { bounds } => {
                        let [left, bottom, right, top] = bounds.map(f64::from);
                        (
                            [(left + right) / 2.0, (bottom + top) / 2.0],
                            ((right - left).hypot(top - bottom)) / 2.0,
                        )
                    }
                };
                if !radius.is_finite() || radius < 0.0 {
                    continue;
                }
                let dx = tree.x - center[0];
                let dy = tree.y - center[1];
                // Eye and cell both lie within sight radius plus sampling
                // slop of the source centre; anything farther than that plus
                // the canopy cannot touch the segment.
                if dx.hypot(dy) <= radius + tree.radius + pitch {
                    out.push(index);
                    continue 'trees;
                }
            }
        }
        out
    }

    /// Change detector over the tree list plus the terrain identity. Terrain
    /// content is immutable per admitted context, so dimensions, origin and
    /// the placement encoding identify the heightfield; a tree add/remove/move
    /// is what forces a restamp of otherwise unchanged sources. FNV-1a over
    /// the admitted bits; a local change detector, not a format hash.
    fn sig(&self) -> u64 {
        let mut sig = 0xcbf2_9ce4_8422_3255u64;
        let mut mix = |word: u32| {
            sig ^= u64::from(word);
            sig = sig.wrapping_mul(0x100_0000_01b3);
        };
        let (origin_x, origin_y) = self.terrain.origin();
        mix(self.terrain.width());
        mix(self.terrain.height());
        mix(origin_x.to_bits());
        mix(origin_y.to_bits());
        for value in self.encoding {
            mix(value.to_bits());
        }
        for tree in self.trees {
            for word in [
                tree.x.to_bits(),
                tree.y.to_bits(),
                tree.radius.to_bits(),
                tree.base.to_bits(),
                tree.top.to_bits(),
            ] {
                sig ^= word;
                sig = sig.wrapping_mul(0x100_0000_01b3);
            }
        }
        sig
    }

    /// The sight origin for a source, or `None` when it is not occludable:
    /// only `Visible` circles participate (rectangular modifiers stamp
    /// unoccluded; native rectangle-vs-occluder sampling is undecoded), and an
    /// unsamplable eye fails open.
    fn eye(&self, source: &VisionSource) -> Option<[f64; 3]> {
        if source.state != VisionState::Visible {
            return None;
        }
        let VisionShape::Circle { center, .. } = source.shape else {
            return None;
        };
        if !center.iter().all(|value| value.is_finite()) {
            return None;
        }
        let (x, y) = (f64::from(center[0]), f64::from(center[1]));
        let ground = ground_z(self.terrain, self.encoding, x, y)?;
        let lift = f64::from(source.source_height.unwrap_or(VIEWER_EYE_ABOVE_GROUND));
        if !lift.is_finite() || lift < 0.0 {
            return None;
        }
        Some([x, y, ground + lift])
    }

    /// Whether the sight ray from `eye` to a cell centre is shadowed.
    /// Analytic 2D segment-vs-disc per tree (blocked when the ray is under the
    /// canopy at segment entry or exit), then a height-lerp ray-march over the
    /// admitted heightfield. Symmetric in height (high sees low as low sees
    /// high); native height-asymmetric rules are undecoded. Any unsamplable
    /// point fails open to visible. Only the plan's relevant trees are tested;
    /// farther trees cannot intersect the segment (see `relevant`).
    fn blocked(&self, eye: [f64; 3], cell: [f64; 2], pitch: f64, trees: &[usize]) -> bool {
        let Some(target) = ground_z(self.terrain, self.encoding, cell[0], cell[1]) else {
            return false;
        };
        let [eye_x, eye_y, eye_z] = eye;
        if !eye_z.is_finite() || !target.is_finite() {
            return false;
        }
        let (dx, dy) = (cell[0] - eye_x, cell[1] - eye_y);
        let along2 = dx * dx + dy * dy;
        if !along2.is_finite() || along2 <= 0.0 {
            return false;
        }
        for &index in trees {
            // Indices come from `relevant` over this same list; anything else
            // fails open rather than panicking.
            let Some(tree) = self.trees.get(index) else {
                continue;
            };
            if ![tree.x, tree.y, tree.radius, tree.top]
                .iter()
                .all(|value| value.is_finite())
                || tree.radius <= 0.0
            {
                continue;
            }
            // Segment from the eye to the cell against the tree disc.
            let (fx, fy) = (eye_x - tree.x, eye_y - tree.y);
            if fx * fx + fy * fy < tree.radius * tree.radius {
                // The eye stands inside the canopy: no self-shadow.
                continue;
            }
            let along = fx * dx + fy * dy;
            let discriminant =
                along * along - along2 * (fx * fx + fy * fy - tree.radius * tree.radius);
            if discriminant < 0.0 || !discriminant.is_finite() {
                continue;
            }
            let root = discriminant.sqrt();
            let (enter, exit) = (
                ((-along - root) / along2).max(0.0),
                ((-along + root) / along2).min(1.0),
            );
            if enter > exit {
                continue;
            }
            // Recreation choice: under the canopy at either overlap end
            // shadows; a ray that stays above the top throughout stays visible.
            let height_at = |t: f64| eye_z + (target - eye_z) * t;
            if height_at(enter) < tree.top || height_at(exit) < tree.top {
                return true;
            }
        }
        if !pitch.is_finite() || pitch <= 0.0 {
            return false;
        }
        let dist = along2.sqrt();
        let steps =
            ((dist / pitch).ceil() as usize).clamp(OCCLUSION_STEPS_MIN, OCCLUSION_STEPS_MAX);
        for step in 1..=steps {
            let t = step as f64 / steps as f64;
            let ray = eye_z + (target - eye_z) * t;
            let (px, py) = (eye_x + dx * t, eye_y + dy * t);
            if ground_z(self.terrain, self.encoding, px, py)
                .is_some_and(|ground| ground > ray + OCCLUSION_TOLERANCE)
            {
                return true;
            }
        }
        false
    }
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
    /// Whether the last successful sample ran the occlusion pass. Stays false
    /// without occlusion input; a refused budget changes nothing.
    occlusion_applied: bool,
    /// Tree-list signature the current cells were stamped with (`None` when
    /// the last successful sample carried no occlusion input). A changed
    /// signature forces a full restamp of otherwise unchanged sources so a
    /// chopped or grown tree cannot leave a stale shadow.
    occlusion_sig: Option<u64>,
    native_height: Option<crate::native_sight::HeightGrid>,
    native_unblocked: std::collections::BTreeSet<HandleId>,
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
            occlusion_sig: None,
            native_height: None,
            native_unblocked: Default::default(),
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
        occlusion: Option<&OcclusionInput>,
    ) -> Result<VisionSampleStats, String> {
        let occlusion_sig = occlusion.map(|input| input.sig());
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
            if previous.is_some_and(|viewer| viewer.sources == sources)
                && self.occlusion_sig == occlusion_sig
            {
                let viewer = previous.expect("checked sources");
                for (source, footprint) in viewer.sources.iter().zip(&viewer.footprints) {
                    current_footprints
                        .entry(shape_key(source.shape))
                        .or_insert_with(|| Arc::clone(footprint));
                }
                planned.push(Planned {
                    id,
                    sources,
                    footprints: Vec::new(),
                    eyes: Vec::new(),
                    // Unchanged plans stamp nothing, so no tree subset is needed.
                    trees: Vec::new(),
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
            let eyes: Vec<Option<[f64; 3]>> = match occlusion {
                Some(input) => sources.iter().map(|source| input.eye(source)).collect(),
                None => Vec::new(),
            };
            // Trees that can shadow this plan's cells; pre-check and stamp
            // charge exactly this subset (see `relevant`).
            let trees = match occlusion {
                Some(input) => input.relevant(&sources, &eyes, self.shape.pitch),
                None => Vec::new(),
            };
            let mut update = Self::plan_update(previous, &sources, &footprints);
            if self.occlusion_sig != occlusion_sig {
                // The occlusion input changed (added, removed, or different
                // trees): every cell is re-evaluated, so a chopped tree cannot
                // leave a stale shadow and a removed input cannot keep shadows.
                update = Update::Full;
            }
            let area_sum = |footprints: &[Arc<Footprint>]| {
                footprints.iter().try_fold(0usize, |sum, footprint| {
                    sum.checked_add(footprint.rect.area())
                        .ok_or("shared vision work overflow")
                })
            };
            let mut work = match &update {
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
            // Per-plan occlusion charge for the same subset the stamp below
            // tests (see `relevant`); zero without occlusion input.
            let charge = match &occlusion {
                Some(_) => OcclusionInput::charge_for(trees.len()),
                None => 0,
            };
            if charge > 0 {
                // Upper bound of the occlusion ray-march, in the same cells the
                // stamp below charges exactly, so the budget refusal still
                // protects and `cell_visits` stays exact. Only relevant trees
                // are charged (see `relevant`).
                let occludable = match &update {
                    Update::Unchanged => 0,
                    Update::Full => {
                        eyes.iter()
                            .zip(&footprints)
                            .try_fold(0usize, |sum, (eye, footprint)| {
                                if eye.is_none() {
                                    return Ok(sum);
                                }
                                let covered =
                                    footprint.runs.iter().try_fold(0usize, |sum, &(x0, x1)| {
                                        sum.checked_add(x1 - x0)
                                            .ok_or("shared vision work overflow")
                                    })?;
                                sum.checked_add(covered)
                                    .ok_or("shared vision work overflow")
                            })?
                    }
                    // Over-charges cells no source shadows, exactly as the
                    // stamp below charges every recomputed cell.
                    Update::Cells(runs) => runs.iter().try_fold(0usize, |sum, run| {
                        sum.checked_add(run.x1 - run.x0)
                            .ok_or("shared vision work overflow")
                    })?,
                };
                work = work
                    .checked_add(
                        occludable
                            .checked_mul(charge)
                            .ok_or("shared vision work overflow")?,
                    )
                    .ok_or("shared vision work overflow")?;
            }
            cell_visits = cell_visits
                .checked_add(work)
                .ok_or("shared vision work overflow")?;
            planned.push(Planned {
                id,
                sources,
                footprints,
                eyes,
                trees,
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
                    let occlusion = occlusion.map(|input| StampOcclusion {
                        input,
                        eyes: &plan.eyes,
                        trees: &plan.trees,
                    });
                    viewer.stamp(
                        &plan.sources,
                        &plan.footprints,
                        self,
                        occlusion,
                        &mut visits,
                    );
                }
                Update::Cells(runs) => {
                    let occlusion = occlusion.map(|input| StampOcclusion {
                        input,
                        eyes: &plan.eyes,
                        trees: &plan.trees,
                    });
                    // No occlusion input means no occlusion charge, matching
                    // the pre-check gate above; relevant trees are per plan.
                    let charge = match &occlusion {
                        Some(stamp) => OcclusionInput::charge_for(stamp.trees.len()),
                        None => 0,
                    };
                    for run in runs {
                        visits += (run.x1 - run.x0) * (1 + charge);
                        for x in run.x0..run.x1 {
                            viewer.recompute(
                                &plan.sources,
                                &plan.footprints,
                                self,
                                occlusion,
                                [x, run.y],
                            );
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
        self.occlusion_sig = occlusion_sig;
        self.occlusion_applied = occlusion.is_some();
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

/// Admitted heightfield, placement encoding and live tree discs backing one
/// occlusion pass.
#[path = "vision_native.rs"]
mod native;

type OcclusionStore = (
    Arc<crate::map_admission::ParsedMapContext>,
    [f32; 3],
    Vec<TreeOccluder>,
);

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
        let shape = match GridShape::from_bounds(bounds, self.vision_pitch()?) {
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

    #[cfg(feature = "test-fixtures")]
    pub fn native_sight_grid_fixture(&self) -> Option<&crate::native_sight::HeightGrid> {
        match &self.shared_vision {
            Some(VisionSlot::Live(vision)) if vision.ready => vision.native_height.as_ref(),
            _ => None,
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
        let shape = GridShape::from_bounds(bounds, self.vision_pitch()?).map_err(fail)?;
        let VisionSlot::Live(vision) = self.shared_vision.as_ref().expect("admitted") else {
            return Err(fail("shared vision is not admitted".into()));
        };
        if vision.shape != shape {
            return Err(fail("shared vision grid changed after admission".into()));
        }
        let units = self.inner.unit_slots.len();
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
        if let Some(rules) = self.native_sight.clone() {
            let mut work = crate::native_sight::Work::new(work_limit);
            let sight_updates = self.native_sight_updates(night, &mut work).map_err(fail)?;
            let heights = self.native_height_grid(&rules, &mut work).map_err(fail)?;
            let mut unblocked = std::collections::BTreeSet::new();
            for (viewer, sources) in &mut plans {
                let number = self.player_settings(*viewer)?.number;
                let mut filtered = Vec::new();
                for source in sources.drain(..) {
                    if let super::VisionOrigin::Unit { unit } = source.origin {
                        let unit_id = unit;
                        let unit = self.executed_unit(unit)?;
                        if !rules.eligible(unit.executed_owner, number) {
                            continue;
                        }
                        if self.native_sight_movement(unit).map_err(fail)? {
                            unblocked.insert(unit_id);
                        }
                    }
                    filtered.push(source);
                }
                *sources = filtered;
            }
            let VisionSlot::Live(vision) = self.shared_vision.as_mut().expect("admitted") else {
                unreachable!("checked live vision")
            };
            let stats = vision
                .apply_native(
                    plans,
                    heights,
                    &rules,
                    &mut work,
                    candidates_scanned,
                    &unblocked,
                )
                .map_err(fail)?;
            *vision.radii = radii.retained();
            self.commit_native_sight_updates(sight_updates)?;
            return Ok(stats);
        }
        let occlusion_store = self.occlusion_store()?;
        let VisionSlot::Live(vision) = self.shared_vision.as_mut().expect("admitted") else {
            return Err(fail("shared vision is not admitted".into()));
        };
        let occlusion = occlusion_store
            .as_ref()
            .map(|(context, encoding, trees)| OcclusionInput {
                terrain: &context.terrain,
                encoding: *encoding,
                trees,
            });
        let stats = vision
            .apply(plans, work_limit, candidates_scanned, occlusion.as_ref())
            .map_err(fail)?;
        *vision.radii = radii.retained();
        Ok(stats)
    }

    /// Admitted heightfield plus live tree discs for the occlusion pass, or
    /// `None` when the map context or the placement encoding is unavailable
    /// (occlusion then stays unapplied). A missing destructable catalog leaves
    /// terrain-only occlusion rather than refusing the sample; unusable trees
    /// fail open (cast no shadow) via [`tree_occluder`]. Tree liveness mirrors
    /// the harvest `tree_usable` checks (dead, hidden, invulnerable, lifeless)
    /// and `tree` comes from the admitted `targType` record.
    fn occlusion_store(&self) -> Result<Option<OcclusionStore>, Error> {
        let fail = |detail: String| self.inner.error("vision", Failure::UnitActivity(detail));
        let Some(context) = self.player_context().cloned() else {
            return Ok(None);
        };
        let Some(placement) = self.unit_placement.as_ref() else {
            return Ok(None);
        };
        let encoding = placement.rules().terrain_encoding();
        let mut trees = Vec::new();
        if let Some(catalog) = self.destructable_catalog.as_ref() {
            for (_, creation, state) in self.destructables_with_handles() {
                if state.dead
                    || state.hidden
                    || state.invulnerable
                    || !state.life.is_some_and(|life| life > 0.0)
                {
                    continue;
                }
                let tree = catalog
                    .stats_if_destructable(creation.objectid)
                    .ok()
                    .flatten()
                    .is_some_and(|stats| stats.tree);
                if !tree {
                    continue;
                }
                let Some(disc) = tree_occluder(&context.terrain, encoding, &creation) else {
                    continue;
                };
                trees
                    .try_reserve(1)
                    .map_err(|_| fail("shared vision allocation".into()))?;
                trees.push(disc);
            }
        }
        Ok(Some((context, encoding, trees)))
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
    fn native_lattice_reuses_unchanged_sources_and_refuses_atomically() {
        use crate::native_sight::{
            HeightGrid, Work,
            tests::{rules, terrain},
        };
        let rules = rules();
        let t = terrain(16, 16, &vec![0; 17 * 17]);
        let heights = HeightGrid::new(&t, 1., &rules, &mut Work::new(10000)).unwrap();
        let mut vision = grid([[0., 0.], [32., 32.]], 2.);
        let viewer = id(5);
        let source = circle([16., 16.], 8., VisionState::Visible);
        let plans = vec![(viewer, vec![source])];
        vision
            .apply_native(
                plans.clone(),
                heights.clone(),
                &rules,
                &mut Work::new(10000),
                1,
                &Default::default(),
            )
            .unwrap();
        let current = vision.viewers[&viewer].current.clone();
        let explored = vision.viewers[&viewer].explored.clone();
        let revision = vision.revision;
        let reused = vision
            .apply_native(
                plans,
                heights.clone(),
                &rules,
                &mut Work::new(0),
                1,
                &Default::default(),
            )
            .unwrap();
        assert_eq!(reused.cell_visits, 0);
        assert_eq!(vision.revision, revision);
        let removed = vec![(viewer, vec![])];
        assert!(
            vision
                .apply_native(
                    removed.clone(),
                    heights.clone(),
                    &rules,
                    &mut Work::new(1),
                    0,
                    &Default::default(),
                )
                .is_err()
        );
        assert_eq!(vision.viewers[&viewer].current, current);
        assert_eq!(vision.viewers[&viewer].explored, explored);
        assert_eq!(vision.revision, revision);
        vision
            .apply_native(
                removed,
                heights,
                &rules,
                &mut Work::new(10000),
                0,
                &Default::default(),
            )
            .unwrap();
        assert_eq!(vision.viewers[&viewer].explored, explored);
        assert!(
            vision.viewers[&viewer]
                .current
                .iter()
                .all(|s| *s != VisionState::Visible)
        );
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
                None,
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
                None,
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
            .apply(vec![(viewer, wide.clone())], 262_144, 0, None)
            .unwrap();
        assert_eq!(first.cell_visits, 20 * 64 * 64);
        assert_eq!(first.emitted_sources, 20);
        assert_eq!(first.candidates_scanned, 0);
        assert!(first.cell_visits > 65_536);
        let again = vision.apply(vec![(viewer, wide)], 0, 40, None).unwrap();
        assert_eq!(again.cell_visits, 0);
        assert_eq!(again.emitted_sources, 20);
        assert_eq!(again.candidates_scanned, 40);
        let kept = vision.last;
        let err = vision
            .apply(
                vec![(viewer, vec![circle([1.5, 1.5], 0.4, VisionState::Visible)])],
                0,
                99,
                None,
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
                None,
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
                .apply(vec![(viewer, sources.clone())], usize::MAX, 0, None)
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
            .apply(vec![(viewer, sources.clone())], usize::MAX, 0, None)
            .unwrap();
        sources[4].shape = VisionShape::Circle {
            center: [40.5, 100.],
            radius: 30.,
        };
        let moved = vision
            .apply(vec![(viewer, sources)], usize::MAX, 0, None)
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

    /// Synthetic corner grid: `corners` holds `(height_index, layer)`
    /// row-major over `(width + 1) * (height + 1)` corners at origin `(0, 0)`.
    /// Layout mirrors `terrain_blight_tests::terrain`; the per-corner height
    /// index is what the occlusion pass reads.
    fn terrain_grid(width: u32, height: u32, corners: &[(u16, u8)]) -> pcx_map::Terrain {
        assert_eq!(corners.len(), (width + 1) as usize * (height + 1) as usize);
        let mut bytes = b"W3E!".to_vec();
        bytes.extend(11u32.to_le_bytes());
        bytes.push(b'L');
        bytes.extend(0u32.to_le_bytes());
        bytes.extend(1u32.to_le_bytes());
        bytes.extend(b"T001");
        bytes.extend(0u32.to_le_bytes());
        bytes.extend((width + 1).to_le_bytes());
        bytes.extend((height + 1).to_le_bytes());
        bytes.extend(0f32.to_le_bytes());
        bytes.extend(0f32.to_le_bytes());
        for (height_index, layer) in corners {
            bytes.extend(height_index.to_le_bytes());
            bytes.extend(0u16.to_le_bytes());
            bytes.push(0);
            bytes.push(0);
            bytes.push(*layer);
        }
        pcx_map::Terrain::from_bytes(bytes).unwrap()
    }

    /// Synthetic placement encoding: one world unit per tile, zero bias, four
    /// raw indices per world unit (so `world z = height_index / 4`).
    const OCCLUSION_ENCODING: [f32; 3] = [1.0, 0.0, 4.0];

    fn occlusion<'a>(
        terrain: &'a pcx_map::Terrain,
        trees: &'a [TreeOccluder],
    ) -> OcclusionInput<'a> {
        OcclusionInput {
            terrain,
            encoding: OCCLUSION_ENCODING,
            trees,
        }
    }

    fn flat_terrain() -> pcx_map::Terrain {
        terrain_grid(8, 8, &[(0, 2); 81])
    }

    /// A north-south ridge one corner wide at world `x == 4`, `height` world
    /// units tall, ramping over the neighbouring tiles.
    fn ridge_terrain(height: f32) -> pcx_map::Terrain {
        let index = (height * OCCLUSION_ENCODING[2]) as u16;
        let corners = (0..81)
            .map(|n| if n % 9 == 4 { (index, 2) } else { (0, 2) })
            .collect::<Vec<_>>();
        terrain_grid(8, 8, &corners)
    }

    #[test]
    fn flat_terrain_leaves_circular_sight_unchanged() {
        let terrain = flat_terrain();
        let input = occlusion(&terrain, &[]);
        let mut vision = grid([[0., 0.], [8., 8.]], 1.);
        let viewer = id(20);
        let sources = vec![
            circle([1.5, 3.5], 3.0, VisionState::Visible),
            VisionSource {
                shape: VisionShape::Rectangle {
                    bounds: [5., 5., 7., 7.],
                },
                state: VisionState::Visible,
                after_units: false,
                origin: VisionOrigin::Modifier { modifier: id(21) },
                source_height: None,
            },
        ];
        vision
            .apply(vec![(viewer, sources.clone())], usize::MAX, 0, Some(&input))
            .unwrap();
        assert!(vision.occlusion_applied);
        // The occluded stamp matches an unoccluded stamp cell for cell,
        // including rectangular modifiers (which never participate).
        let mut plain = grid([[0., 0.], [8., 8.]], 1.);
        plain
            .apply(vec![(viewer, sources)], usize::MAX, 0, None)
            .unwrap();
        assert!(!plain.occlusion_applied);
        let (stamped, expected) = (
            vision.viewers.get(&viewer).unwrap(),
            plain.viewers.get(&viewer).unwrap(),
        );
        assert_eq!(stamped.current, expected.current);
        assert_eq!(stamped.explored, expected.explored);
        assert_eq!(
            cell(&vision, viewer, [1.5, 3.5]),
            VisionCell {
                raw: VisionState::Visible,
                explored: true
            }
        );
        assert_eq!(
            cell(&vision, viewer, [6.5, 5.5]),
            VisionCell {
                raw: VisionState::Visible,
                explored: true
            }
        );
        assert_eq!(
            cell(&vision, viewer, [7.5, 7.5]),
            VisionCell {
                raw: VisionState::Masked,
                explored: false
            }
        );
    }

    #[test]
    fn ridge_shadows_cells_behind_it_and_keeps_explored_history() {
        let terrain = ridge_terrain(256.0);
        let input = occlusion(&terrain, &[]);
        let mut vision = grid([[0., 0.], [8., 8.]], 1.);
        let viewer = id(22);
        // Explore the far cell first without occlusion, so the shadowed sample
        // below must retain its explored history while fogging the raw state.
        vision
            .apply(
                vec![(viewer, vec![circle([1.5, 3.5], 5.0, VisionState::Visible)])],
                usize::MAX,
                0,
                None,
            )
            .unwrap();
        assert_eq!(
            cell(&vision, viewer, [6.5, 3.5]),
            VisionCell {
                raw: VisionState::Visible,
                explored: true
            }
        );
        vision
            .apply(
                vec![(viewer, vec![circle([1.5, 3.5], 5.0, VisionState::Visible)])],
                usize::MAX,
                0,
                Some(&input),
            )
            .unwrap();
        assert!(vision.occlusion_applied);
        assert_eq!(
            cell(&vision, viewer, [2.5, 3.5]),
            VisionCell {
                raw: VisionState::Visible,
                explored: true
            }
        );
        assert_eq!(
            cell(&vision, viewer, [6.5, 3.5]),
            VisionCell {
                raw: VisionState::Fogged,
                explored: true
            }
        );
    }

    #[test]
    fn eye_over_a_low_wall_still_sees_beyond_it() {
        let terrain = ridge_terrain(32.0);
        let input = occlusion(&terrain, &[]);
        let mut vision = grid([[0., 0.], [8., 8.]], 1.);
        let viewer = id(23);
        vision
            .apply(
                vec![(viewer, vec![circle([1.5, 3.5], 5.0, VisionState::Visible)])],
                usize::MAX,
                0,
                Some(&input),
            )
            .unwrap();
        assert!(vision.occlusion_applied);
        // The 128-unit recreation eye clears the 32-unit ridge.
        assert_eq!(
            cell(&vision, viewer, [6.5, 3.5]),
            VisionCell {
                raw: VisionState::Visible,
                explored: true
            }
        );
    }

    #[test]
    fn single_tree_disc_shadows_and_its_removal_restores_sight() {
        let terrain = flat_terrain();
        let tree = [TreeOccluder {
            x: 3.0,
            y: 3.5,
            radius: 1.0,
            base: 0.0,
            top: 200.0,
        }];
        let viewer = id(24);
        let sources = || vec![circle([0.5, 3.5], 6.0, VisionState::Visible)];
        // Explore the far cell first without occlusion, as above.
        let mut vision = grid([[0., 0.], [8., 8.]], 1.);
        vision
            .apply(vec![(viewer, sources())], usize::MAX, 0, None)
            .unwrap();
        assert_eq!(cell(&vision, viewer, [5.5, 3.5]).raw, VisionState::Visible);
        // The same sources with one tree between source and cell: the far cell
        // fogs but keeps its explored history; the off-axis cell stays visible.
        let input = occlusion(&terrain, &tree);
        vision
            .apply(vec![(viewer, sources())], usize::MAX, 0, Some(&input))
            .unwrap();
        assert!(vision.occlusion_applied);
        assert_eq!(
            cell(&vision, viewer, [5.5, 3.5]),
            VisionCell {
                raw: VisionState::Fogged,
                explored: true
            }
        );
        assert_eq!(
            cell(&vision, viewer, [5.5, 6.5]),
            VisionCell {
                raw: VisionState::Visible,
                explored: true
            }
        );
        // Removing the tree restamps the unchanged sources: the shadow lifts.
        // The signature change forces a full restamp, not an unchanged skip.
        let cleared = occlusion(&terrain, &[]);
        vision
            .apply(vec![(viewer, sources())], usize::MAX, 0, Some(&cleared))
            .unwrap();
        assert!(vision.occlusion_applied);
        assert_eq!(
            cell(&vision, viewer, [5.5, 3.5]),
            VisionCell {
                raw: VisionState::Visible,
                explored: true
            }
        );
    }

    #[test]
    fn blocked_sight_preserves_other_sources_and_unexplored_cells() {
        let terrain = flat_terrain();
        let trees = [TreeOccluder {
            x: 3.0,
            y: 3.5,
            radius: 1.0,
            base: 0.0,
            top: 200.0,
        }];
        let input = occlusion(&terrain, &trees);
        let viewer = id(26);
        let blocked = circle([0.5, 3.5], 6.0, VisionState::Visible);
        let clear = circle([5.5, 3.5], 1.0, VisionState::Visible);
        let target = [5.5, 3.5];
        for sources in [vec![clear, blocked], vec![blocked, clear]] {
            let mut vision = grid([[0., 0.], [8., 8.]], 1.);
            vision
                .apply(vec![(viewer, sources)], usize::MAX, 0, Some(&input))
                .unwrap();
            assert_eq!(
                cell(&vision, viewer, target),
                VisionCell {
                    raw: VisionState::Visible,
                    explored: true
                }
            );
            vision
                .apply(vec![(viewer, vec![blocked])], usize::MAX, 0, Some(&input))
                .unwrap();
            assert_eq!(
                cell(&vision, viewer, target),
                VisionCell {
                    raw: VisionState::Fogged,
                    explored: true
                }
            );
        }
        let mut fresh = grid([[0., 0.], [8., 8.]], 1.);
        fresh
            .apply(vec![(viewer, vec![blocked])], usize::MAX, 0, Some(&input))
            .unwrap();
        assert_eq!(
            cell(&fresh, viewer, target),
            VisionCell {
                raw: VisionState::Masked,
                explored: false
            }
        );
    }

    #[test]
    fn tree_discs_derive_radius_and_canopy_from_admitted_scale() {
        use crate::playercolor_host::{DestructableCreation, DestructableOrigin};
        let terrain = flat_terrain();
        let creation = DestructableCreation {
            origin: DestructableOrigin::Script,
            objectid: 0,
            x: 1.5,
            y: 1.5,
            z: None,
            face: 0.0,
            scale: 2.0,
            variation: 0,
            dead: false,
        };
        assert_eq!(
            tree_occluder(&terrain, OCCLUSION_ENCODING, &creation),
            Some(TreeOccluder {
                x: 1.5,
                y: 1.5,
                radius: 2.0 * TREE_RADIUS_PER_SCALE,
                base: 0.0,
                top: 2.0 * TREE_CANOPY_PER_SCALE,
            })
        );
        for scale in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert_eq!(
                tree_occluder(
                    &terrain,
                    OCCLUSION_ENCODING,
                    &DestructableCreation { scale, ..creation }
                ),
                None,
                "unusable scale casts no shadow: {scale}"
            );
        }
        assert_eq!(
            tree_occluder(
                &terrain,
                OCCLUSION_ENCODING,
                &DestructableCreation {
                    x: 500.0,
                    y: 500.0,
                    ..creation
                }
            ),
            None,
            "a tree outside the admitted heightfield casts no shadow"
        );
    }

    #[test]
    fn flag_tracks_the_pass_and_a_refused_budget_keeps_history() {
        let terrain = flat_terrain();
        let input = occlusion(&terrain, &[]);
        let viewer = id(25);
        let mut vision = grid([[0., 0.], [8., 8.]], 1.);
        vision
            .apply(
                vec![(viewer, vec![circle([0.5, 3.5], 1.0, VisionState::Visible)])],
                usize::MAX,
                0,
                None,
            )
            .unwrap();
        assert!(!vision.occlusion_applied);
        vision
            .apply(
                vec![(viewer, vec![circle([0.5, 3.5], 1.0, VisionState::Visible)])],
                usize::MAX,
                7,
                Some(&input),
            )
            .unwrap();
        assert!(vision.occlusion_applied);
        let kept = vision.last;
        // A moved source charges work; a zero limit refuses before any write.
        let err = vision
            .apply(
                vec![(viewer, vec![circle([6.5, 6.5], 1.0, VisionState::Visible)])],
                0,
                99,
                Some(&input),
            )
            .unwrap_err();
        assert!(err.contains("budget"));
        assert_eq!(
            cell(&vision, viewer, [0.5, 3.5]),
            VisionCell {
                raw: VisionState::Visible,
                explored: true
            }
        );
        assert_eq!(cell(&vision, viewer, [6.5, 6.5]).raw, VisionState::Masked);
        assert_eq!(vision.last, kept);
        assert!(vision.occlusion_applied);
        // Without any prior success a refusal leaves the flag false.
        let mut fresh = grid([[0., 0.], [8., 8.]], 1.);
        let err = fresh
            .apply(
                vec![(viewer, vec![circle([0.5, 3.5], 1.0, VisionState::Visible)])],
                0,
                0,
                Some(&input),
            )
            .unwrap_err();
        assert!(err.contains("budget"));
        assert!(!fresh.occlusion_applied);
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

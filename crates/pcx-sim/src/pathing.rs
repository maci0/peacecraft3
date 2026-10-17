//! Admitted movement over an authored WPM raster.
//!
//! File bits do not name a walk mask. A caller profile states which bits block,
//! and the pathing-cell extent is the caller-supplied terrain-tile extent
//! divided by the uniform scale of [`pcx_map::grid_alignment::terrain_pathing_alignment`].
//! That scale is derived from the terrain and pathing dimensions. The harness
//! fixed-point origin is caller-supplied: `war3map.w3e` stores an `f32` origin
//! and neither file stores a pathing-cell extent.
//!
//! `step` consults an [`AdmittedMovement`] installed on the fixed-tick world.
//! With none installed, the harness step is unchanged.

use std::cmp::Ordering;
use std::sync::Arc;

use pcx_map::{GridAlignment, GridAlignmentError, Pathing};

use crate::{Fx, MOVE_SPEED};

/// Cells one harness step may touch along one axis, including its start cell.
///
/// This bounds traversal work. It is not an original pathing limit.
pub const MAX_CELLS_PER_AXIS: u32 = 8;

/// Cell-intersection checks one `clamp_step` may perform.
///
/// Past this bound the step stays put. It does not cross the remaining cells.
pub const MAX_SEGMENT_CHECKS: u32 = 512;

/// Caller-supplied movement admission.
///
/// `blocked_bits` is the whole mask. A cell blocks when any of those bits is
/// set. There is no built-in mask.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MovementProfile {
    pub origin_x: Fx,
    pub origin_y: Fx,
    /// Extent of one terrain tile in the same fixed-point space as unit positions.
    pub terrain_tile_fx: Fx,
    pub blocked_bits: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AdmitError {
    #[error(transparent)]
    Alignment(#[from] GridAlignmentError),
    #[error("movement admission needs at least one blocking bit")]
    EmptyMask,
    #[error("terrain tile extent {0} is not positive")]
    NonPositiveTile(Fx),
    #[error("pathing scale {0} does not fit the fixed-point tile extent")]
    ScaleOverflow(u32),
    #[error("terrain tile extent {terrain_tile_fx} is not divisible by pathing scale {scale}")]
    TileScale { terrain_tile_fx: Fx, scale: u32 },
    #[error("a harness step can cross {cells_per_axis} cells per axis, above the bound {bound}")]
    StepTooFine { cells_per_axis: u32, bound: u32 },
    #[error("pathing raster {width}x{height} overflows the cell count")]
    RasterOverflow { width: u32, height: u32 },
    #[error("pathing raster {width}x{height} holds {bytes} cells")]
    RasterBytes {
        width: u32,
        height: u32,
        bytes: usize,
    },
    #[error("pathing grid extent overflows the fixed-point span")]
    SpanOverflow,
    #[error("pathing raster is {bytes} bytes and the caller bound is {bound}")]
    Capacity { bytes: usize, bound: usize },
    #[error("pathing raster of {bytes} bytes could not be reserved")]
    Allocation { bytes: usize },
    #[error("supplied grid alignment does not match this pathing raster")]
    AlignmentMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellQuery {
    Inside {
        x: u32,
        y: u32,
        flags: u8,
        blocked: bool,
    },
    NegativeX,
    NegativeY,
    PastEdgeX,
    PastEdgeY,
    Overflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StepClamp {
    pub x: Fx,
    pub y: Fx,
    pub checks: u32,
}

/// Movement grid installed on the fixed-tick world.
///
/// Fields stay private so a later write cannot clear the mask or the cell
/// extent and turn every cell passable. Snapshots share one `Arc`.
#[derive(Debug, PartialEq, Eq)]
pub struct AdmittedMovement {
    alignment: GridAlignment,
    origin_x: Fx,
    origin_y: Fx,
    terrain_tile_fx: Fx,
    cell_fx: Fx,
    blocked_bits: u8,
    cells: Vec<u8>,
}

/// Admit `pathing` for straight-line movement.
///
/// The uniform scale comes from the terrain and pathing dimensions. Unaligned
/// grids are refused. An empty mask is refused. `max_bytes` is the caller's
/// retention ceiling, checked before the raster is copied. Map admission's
/// `pathing_bytes` is that ceiling when the caller has one.
pub fn admit_movement(
    pathing: &Pathing,
    terrain_width: u32,
    terrain_height: u32,
    profile: MovementProfile,
    max_bytes: usize,
) -> Result<Arc<AdmittedMovement>, AdmitError> {
    let alignment = pcx_map::grid_alignment::terrain_pathing_alignment(
        terrain_width,
        terrain_height,
        pathing.width,
        pathing.height,
    )?;
    admit_with_alignment(pathing, alignment, profile, max_bytes)
}

/// Admit using an existing [`GridAlignment`].
///
/// The alignment's terrain and pathing dimensions are checked against `pathing`.
/// A mismatched alignment is refused. Origin and tile extent stay on `profile`:
/// the alignment does not carry a world extent, and the W3E origin is `f32`.
pub fn admit_aligned_movement(
    pathing: &Pathing,
    alignment: &GridAlignment,
    profile: MovementProfile,
    max_bytes: usize,
) -> Result<Arc<AdmittedMovement>, AdmitError> {
    let derived = pcx_map::grid_alignment::terrain_pathing_alignment(
        alignment.terrain.tiles.width,
        alignment.terrain.tiles.height,
        pathing.width,
        pathing.height,
    )?;
    if derived != *alignment
        || alignment.pathing.raster.width != pathing.width
        || alignment.pathing.raster.height != pathing.height
    {
        return Err(AdmitError::AlignmentMismatch);
    }
    admit_with_alignment(pathing, derived, profile, max_bytes)
}

fn admit_with_alignment(
    pathing: &Pathing,
    alignment: GridAlignment,
    profile: MovementProfile,
    max_bytes: usize,
) -> Result<Arc<AdmittedMovement>, AdmitError> {
    if profile.blocked_bits == 0 {
        return Err(AdmitError::EmptyMask);
    }
    if profile.terrain_tile_fx <= 0 {
        return Err(AdmitError::NonPositiveTile(profile.terrain_tile_fx));
    }
    let scale = i32::try_from(alignment.uniform_scale)
        .map_err(|_| AdmitError::ScaleOverflow(alignment.uniform_scale))?;
    if profile.terrain_tile_fx % scale != 0 {
        return Err(AdmitError::TileScale {
            terrain_tile_fx: profile.terrain_tile_fx,
            scale: alignment.uniform_scale,
        });
    }
    let cell_fx = profile.terrain_tile_fx / scale;
    if cell_fx <= 0 {
        return Err(AdmitError::NonPositiveTile(profile.terrain_tile_fx));
    }
    let per_axis = i64::from(MOVE_SPEED) / i64::from(cell_fx) + 2;
    let Ok(cells_per_axis) = u32::try_from(per_axis) else {
        return Err(AdmitError::StepTooFine {
            cells_per_axis: u32::MAX,
            bound: MAX_CELLS_PER_AXIS,
        });
    };
    if cells_per_axis > MAX_CELLS_PER_AXIS {
        return Err(AdmitError::StepTooFine {
            cells_per_axis,
            bound: MAX_CELLS_PER_AXIS,
        });
    }
    let Some(count) = pathing.width.checked_mul(pathing.height) else {
        return Err(AdmitError::RasterOverflow {
            width: pathing.width,
            height: pathing.height,
        });
    };
    let Ok(count_usize) = usize::try_from(count) else {
        return Err(AdmitError::RasterOverflow {
            width: pathing.width,
            height: pathing.height,
        });
    };
    if pathing.cells.len() != count_usize {
        return Err(AdmitError::RasterBytes {
            width: pathing.width,
            height: pathing.height,
            bytes: pathing.cells.len(),
        });
    }
    if count_usize > max_bytes {
        return Err(AdmitError::Capacity {
            bytes: count_usize,
            bound: max_bytes,
        });
    }
    if !span_fits(profile.origin_x, cell_fx, pathing.width)
        || !span_fits(profile.origin_y, cell_fx, pathing.height)
    {
        return Err(AdmitError::SpanOverflow);
    }
    let cells = copy_cells(&pathing.cells)?;
    Ok(Arc::new(AdmittedMovement {
        alignment,
        origin_x: profile.origin_x,
        origin_y: profile.origin_y,
        terrain_tile_fx: profile.terrain_tile_fx,
        cell_fx,
        blocked_bits: profile.blocked_bits,
        cells,
    }))
}

fn copy_cells(cells: &[u8]) -> Result<Vec<u8>, AdmitError> {
    let mut owned = Vec::new();
    owned
        .try_reserve_exact(cells.len())
        .map_err(|_| AdmitError::Allocation { bytes: cells.len() })?;
    owned.extend_from_slice(cells);
    Ok(owned)
}

fn span_fits(origin: Fx, cell: Fx, extent: u32) -> bool {
    let end = i128::from(origin) + i128::from(cell) * i128::from(extent);
    let low = i128::from(origin) - i128::from(cell);
    low >= i128::from(i64::MIN) && end.saturating_add(i128::from(cell)) <= i128::from(i64::MAX)
}

enum AxisIndex {
    Inside(u32),
    Negative,
    Past,
    Overflow,
}

impl AdmittedMovement {
    pub fn alignment(&self) -> &GridAlignment {
        &self.alignment
    }

    pub fn origin(&self) -> (Fx, Fx) {
        (self.origin_x, self.origin_y)
    }

    pub fn terrain_tile_fx(&self) -> Fx {
        self.terrain_tile_fx
    }

    pub fn cell_fx(&self) -> Fx {
        self.cell_fx
    }

    pub fn blocked_bits(&self) -> u8 {
        self.blocked_bits
    }

    fn holds(&self) -> bool {
        if self.cell_fx <= 0 || self.blocked_bits == 0 {
            return false;
        }
        let Some(count) = (self.alignment.pathing.raster.width as usize)
            .checked_mul(self.alignment.pathing.raster.height as usize)
        else {
            return false;
        };
        self.cells.len() == count
    }

    /// Locate one fixed-point position on the admitted raster.
    ///
    /// Cell `(i, j)` owns `[origin + i * cell, origin + (i + 1) * cell)` on
    /// each axis. A negative offset is reported before a far edge. X is
    /// reported before Y.
    pub fn locate(&self, x: Fx, y: Fx) -> CellQuery {
        if !self.holds() {
            return CellQuery::Overflow;
        }
        let cx = match self.axis(x, self.origin_x, self.alignment.pathing.raster.width) {
            AxisIndex::Inside(index) => index,
            AxisIndex::Negative => return CellQuery::NegativeX,
            AxisIndex::Past => return CellQuery::PastEdgeX,
            AxisIndex::Overflow => return CellQuery::Overflow,
        };
        let cy = match self.axis(y, self.origin_y, self.alignment.pathing.raster.height) {
            AxisIndex::Inside(index) => index,
            AxisIndex::Negative => return CellQuery::NegativeY,
            AxisIndex::Past => return CellQuery::PastEdgeY,
            AxisIndex::Overflow => return CellQuery::Overflow,
        };
        let Some(flags) = self.flags(cx, cy) else {
            return CellQuery::Overflow;
        };
        CellQuery::Inside {
            x: cx,
            y: cy,
            flags,
            blocked: flags & self.blocked_bits != 0,
        }
    }

    /// Furthest point of the straight harness step that stays in passable cells.
    ///
    /// Outside the raster is not passable. A step whose cell box exceeds
    /// [`MAX_CELLS_PER_AXIS`], or whose checks exceed [`MAX_SEGMENT_CHECKS`],
    /// stays at the start. This is straight-line blocking, not a path search.
    pub fn clamp_step(&self, x: Fx, y: Fx, step_x: Fx, step_y: Fx) -> StepClamp {
        let stay = StepClamp { x, y, checks: 0 };
        if !self.holds() {
            return stay;
        }
        let (Ok(end_x), Ok(end_y)) = (
            i64::try_from(i128::from(x) + i128::from(step_x)),
            i64::try_from(i128::from(y) + i128::from(step_y)),
        ) else {
            return stay;
        };
        let ax = i64::from(x) - i64::from(self.origin_x);
        let ay = i64::from(y) - i64::from(self.origin_y);
        let bx = end_x - i64::from(self.origin_x);
        let by = end_y - i64::from(self.origin_y);
        let mut grid = Geom {
            width: i64::from(self.alignment.pathing.raster.width),
            height: i64::from(self.alignment.pathing.raster.height),
            cell: i64::from(self.cell_fx),
            cells: &self.cells,
            mask: self.blocked_bits,
            checks: 0,
        };
        let (rx, ry) = grid.clamp(ax, ay, bx, by);
        let (Ok(wx), Ok(wy)) = (
            Fx::try_from(rx + i64::from(self.origin_x)),
            Fx::try_from(ry + i64::from(self.origin_y)),
        ) else {
            return StepClamp {
                x,
                y,
                checks: grid.checks,
            };
        };
        StepClamp {
            x: wx,
            y: wy,
            checks: grid.checks,
        }
    }

    fn axis(&self, value: Fx, origin: Fx, extent: u32) -> AxisIndex {
        let rel = i128::from(value) - i128::from(origin);
        if rel < 0 {
            return AxisIndex::Negative;
        }
        let cell = i128::from(self.cell_fx);
        if cell <= 0 {
            return AxisIndex::Overflow;
        }
        let idx = rel / cell;
        let Ok(idx) = u32::try_from(idx) else {
            return AxisIndex::Overflow;
        };
        if idx >= extent {
            AxisIndex::Past
        } else {
            AxisIndex::Inside(idx)
        }
    }

    fn flags(&self, x: u32, y: u32) -> Option<u8> {
        let index = (y as usize)
            .checked_mul(self.alignment.pathing.raster.width as usize)?
            .checked_add(x as usize)?;
        self.cells.get(index).copied()
    }
}

#[derive(Clone, Copy)]
struct Rat {
    num: i128,
    den: i128,
}

#[derive(Clone, Copy)]
struct Iv {
    lo: Rat,
    lo_in: bool,
    hi: Rat,
    hi_in: bool,
}

fn gcd_pos(mut a: i128, mut b: i128) -> i128 {
    while b != 0 {
        let r = a % b;
        a = b;
        b = r;
    }
    a
}

fn reduce(r: Rat) -> Rat {
    let Some(abs_num) = r.num.checked_abs() else {
        return r;
    };
    let g = gcd_pos(abs_num, r.den);
    Rat {
        num: r.num / g,
        den: r.den / g,
    }
}

fn norm_div(n: i128, d: i128) -> Rat {
    reduce(if d > 0 {
        Rat { num: n, den: d }
    } else {
        Rat { num: -n, den: -d }
    })
}

fn cmp_rat(a: Rat, b: Rat) -> Ordering {
    (a.num * b.den).cmp(&(b.num * a.den))
}

fn nonempty(iv: Iv) -> bool {
    match cmp_rat(iv.lo, iv.hi) {
        Ordering::Less => true,
        Ordering::Equal => iv.lo_in && iv.hi_in,
        Ordering::Greater => false,
    }
}

fn raise_lo(iv: &mut Iv, bound: Rat, inclusive: bool) {
    match cmp_rat(iv.lo, bound) {
        Ordering::Less => {
            iv.lo = bound;
            iv.lo_in = inclusive;
        }
        Ordering::Equal => iv.lo_in = iv.lo_in && inclusive,
        Ordering::Greater => {}
    }
}

fn lower_hi(iv: &mut Iv, bound: Rat, inclusive: bool) {
    match cmp_rat(iv.hi, bound) {
        Ordering::Greater => {
            iv.hi = bound;
            iv.hi_in = inclusive;
        }
        Ordering::Equal => iv.hi_in = iv.hi_in && inclusive,
        Ordering::Less => {}
    }
}

fn kill(iv: &mut Iv) {
    *iv = Iv {
        lo: Rat { num: 1, den: 1 },
        lo_in: false,
        hi: Rat { num: 0, den: 1 },
        hi_in: false,
    };
}

fn add_ge(iv: &mut Iv, coeff: i128, rhs: i128) {
    if coeff > 0 {
        raise_lo(iv, norm_div(rhs, coeff), true);
    } else if coeff < 0 {
        lower_hi(iv, norm_div(rhs, coeff), true);
    } else if rhs > 0 {
        kill(iv);
    }
}

fn add_lt(iv: &mut Iv, coeff: i128, rhs: i128) {
    if coeff > 0 {
        lower_hi(iv, norm_div(rhs, coeff), false);
    } else if coeff < 0 {
        raise_lo(iv, norm_div(rhs, coeff), false);
    } else if rhs <= 0 {
        kill(iv);
    }
}

fn hits_interval(
    (ax, ay): (i64, i64),
    (bx, by): (i64, i64),
    left: i64,
    bottom: i64,
    right: i64,
    top: i64,
) -> Option<Iv> {
    if right <= left || top <= bottom {
        return None;
    }
    let mut iv = Iv {
        lo: Rat { num: 0, den: 1 },
        lo_in: true,
        hi: Rat { num: 1, den: 1 },
        hi_in: true,
    };
    let dx = i128::from(bx) - i128::from(ax);
    let dy = i128::from(by) - i128::from(ay);
    add_ge(&mut iv, dx, i128::from(left) - i128::from(ax));
    add_lt(&mut iv, dx, i128::from(right) - i128::from(ax));
    add_ge(&mut iv, dy, i128::from(bottom) - i128::from(ay));
    add_lt(&mut iv, dy, i128::from(top) - i128::from(ay));
    nonempty(iv).then_some(iv)
}

fn earlier(a: Iv, b: Iv) -> bool {
    match cmp_rat(a.lo, b.lo) {
        Ordering::Less => true,
        Ordering::Greater => false,
        Ordering::Equal => a.lo_in && !b.lo_in,
    }
}

fn cell_rect(c: i64, cell: i64) -> Option<(i64, i64)> {
    let left = i128::from(c).checked_mul(i128::from(cell))?;
    let right = left.checked_add(i128::from(cell))?;
    if left < i128::from(i64::MIN) || right > i128::from(i64::MAX) {
        return None;
    }
    Some((left as i64, right as i64))
}

enum Class {
    Clear,
    Blocked(Iv),
    Unbounded,
}

struct Geom<'a> {
    width: i64,
    height: i64,
    cell: i64,
    cells: &'a [u8],
    mask: u8,
    checks: u32,
}

impl Geom<'_> {
    fn blocked_at(&self, cx: i64, cy: i64) -> Option<bool> {
        if cx < 0 || cy < 0 || cx >= self.width || cy >= self.height {
            return None;
        }
        let index = (cy as usize)
            .checked_mul(self.width as usize)?
            .checked_add(cx as usize)?;
        self.cells.get(index).map(|flags| flags & self.mask != 0)
    }

    fn point_good(&self, x: i64, y: i64) -> bool {
        self.blocked_at(x.div_euclid(self.cell), y.div_euclid(self.cell)) == Some(false)
    }

    fn classify(&mut self, ax: i64, ay: i64, bx: i64, by: i64) -> Class {
        let (minx, maxx) = (
            ax.div_euclid(self.cell).min(bx.div_euclid(self.cell)),
            ax.div_euclid(self.cell).max(bx.div_euclid(self.cell)),
        );
        let (miny, maxy) = (
            ay.div_euclid(self.cell).min(by.div_euclid(self.cell)),
            ay.div_euclid(self.cell).max(by.div_euclid(self.cell)),
        );
        if maxx - minx >= i64::from(MAX_CELLS_PER_AXIS)
            || maxy - miny >= i64::from(MAX_CELLS_PER_AXIS)
        {
            return Class::Unbounded;
        }
        let mut earliest = None;
        for cy in miny..=maxy {
            for cx in minx..=maxx {
                self.checks = self.checks.saturating_add(1);
                if self.checks > MAX_SEGMENT_CHECKS {
                    return Class::Unbounded;
                }
                let Some((left, right)) = cell_rect(cx, self.cell) else {
                    return Class::Unbounded;
                };
                let Some((bottom, top)) = cell_rect(cy, self.cell) else {
                    return Class::Unbounded;
                };
                let Some(iv) = hits_interval((ax, ay), (bx, by), left, bottom, right, top) else {
                    continue;
                };
                if self.blocked_at(cx, cy) == Some(false) {
                    continue;
                }
                earliest = Some(match earliest {
                    Some(prev) if earlier(prev, iv) => prev,
                    _ => iv,
                });
            }
        }
        match earliest {
            Some(iv) => Class::Blocked(iv),
            None => Class::Clear,
        }
    }

    fn clamp(&mut self, ax: i64, ay: i64, bx: i64, by: i64) -> (i64, i64) {
        if !self.point_good(ax, ay) || (ax, ay) == (bx, by) {
            return (ax, ay);
        }
        let iv = match self.classify(ax, ay, bx, by) {
            Class::Clear => return (bx, by),
            Class::Unbounded => return (ax, ay),
            Class::Blocked(iv) => iv,
        };
        let dx = i128::from(bx) - i128::from(ax);
        let dy = i128::from(by) - i128::from(ay);
        let span128 = dx.abs().max(dy.abs());
        let Ok(span) = i64::try_from(span128) else {
            return (ax, ay);
        };
        if span == 0 {
            return (ax, ay);
        }
        let mut k = max_k(i128::from(span), iv.lo, iv.lo_in);
        loop {
            let landed = if k == 0 {
                Some((ax, ay))
            } else {
                point(ax, bx, k, span).zip(point(ay, by, k, span))
            };
            let Some((x, y)) = landed else {
                return (ax, ay);
            };
            match self.classify(ax, ay, x, y) {
                Class::Clear if k == 0 || self.point_good(x, y) => return (x, y),
                Class::Unbounded => return (ax, ay),
                _ => {
                    if k == 0 || self.checks > MAX_SEGMENT_CHECKS {
                        return (ax, ay);
                    }
                    k -= 1;
                }
            }
        }
    }
}

fn max_k(span: i128, lo: Rat, lo_in: bool) -> i64 {
    if span <= 0 {
        return 0;
    }
    let prod = span * lo.num;
    let k = if lo_in {
        if prod <= 0 { 0 } else { (prod - 1) / lo.den }
    } else if prod < 0 {
        0
    } else {
        prod / lo.den
    };
    k.clamp(0, span) as i64
}

fn point(a: i64, b: i64, k: i64, span: i64) -> Option<i64> {
    if span <= 0 || k < 0 {
        return None;
    }
    let delta = (i128::from(b) - i128::from(a)).checked_mul(i128::from(k))? / i128::from(span);
    i64::try_from(i128::from(a).checked_add(delta)?).ok()
}

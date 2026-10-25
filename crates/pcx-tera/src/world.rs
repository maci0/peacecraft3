//! The corner → world mapping, measured tile by tile on both builds.
//!
//! See the crate docs for the evidence: 11 800 (1.28c) and 6 540 (1.29b)
//! flat-tile doodads whose stored world `z` satisfies
//! `(index - 8192) * 0.25` exactly in `f32`, across 176 and 64 distinct
//! height indices.

use pcx_map::Terrain;

/// World units per tile edge, on both `x` and `y`.
///
/// Measured: every shipped terrain's doodads fall inside its own tile grid at
/// this scale, both origin floats are a multiple of 128 on all 132 terrains,
/// and the widest observed doodad span is 99.35 % of the grid.
pub const TILE_XY: f32 = 128.0;

/// World units of height per unit of raw 14-bit `height_index`.
///
/// Measured: 0.25, exactly, over 18 340 independent exact matches.
pub const Z_PER_INDEX: f32 = 0.25;

/// The `height_index` that sits at world `z == 0`: the range midpoint.
///
/// Measured: the fit over the exact matches is `z = 0.25·index - 2048`, which
/// is `z = 0.25·(index - 8192)`.
pub const Z_REFERENCE_INDEX: f32 = 8192.0;

/// World `z` of a raw corner height index.
#[inline]
#[must_use]
pub fn world_z(height_index: u16) -> f32 {
    (height_index as f32 - Z_REFERENCE_INDEX) * Z_PER_INDEX
}

/// The raw corner height index a world `z` corresponds to, rounded.
#[inline]
#[must_use]
pub fn height_index(world_z: f32) -> u16 {
    let raw = world_z / Z_PER_INDEX + Z_REFERENCE_INDEX;
    raw.round().clamp(0.0, 16383.0) as u16
}

/// World position of terrain corner `(cx, cy)` under the measured constants.
///
/// `cx` ranges over `0..=width` and `cy` over `0..=height`, both row-major with
/// the **rows not flipped**: `cy` 0 is the low-`y` end of the grid, which is the
/// end `origin_y` names. Returns `None` outside the grid.
#[must_use]
pub fn corner_world(terrain: &Terrain, cx: u32, cy: u32) -> Option<[f32; 3]> {
    corner_world_scaled(terrain, cx, cy, TILE_XY, Z_PER_INDEX, Z_REFERENCE_INDEX)
}

/// [`corner_world`] with a caller-supplied scale.
///
/// This exists so a viewer can override the convention without forking the
/// crate; the defaults are the ones the shipped bytes measure, and nothing in
/// this crate calls this with anything else.
#[must_use]
pub fn corner_world_scaled(
    terrain: &Terrain,
    cx: u32,
    cy: u32,
    xy_per_tile: f32,
    z_per_index: f32,
    z_reference_index: f32,
) -> Option<[f32; 3]> {
    let corner = terrain.corner(cx, cy)?;
    let (origin_x, origin_y) = terrain.origin();
    Some([
        origin_x + cx as f32 * xy_per_tile,
        origin_y + cy as f32 * xy_per_tile,
        (corner.height_index as f32 - z_reference_index) * z_per_index,
    ])
}

/// The corner a world point falls inside, if any.
///
/// This is the inverse of [`corner_world`]: `corner_at` maps a world `x`/`y`
/// pair onto the fractional corner column and row, which is what a caller
/// needs to sample terrain height under a world point. It is *not* rounded —
/// the fractional part is the position inside the tile, exactly as the tiles
/// in the file are laid out.
#[must_use]
pub fn corner_at(terrain: &Terrain, world_x: f32, world_y: f32) -> Option<(f32, f32)> {
    let (origin_x, origin_y) = terrain.origin();
    let fx = (world_x - origin_x) / TILE_XY;
    let fy = (world_y - origin_y) / TILE_XY;
    if fx < 0.0 || fy < 0.0 || fx > terrain.width() as f32 || fy > terrain.height() as f32 {
        return None;
    }
    Some((fx, fy))
}

/// Terrain world `z` at a world `x`/`y`, bilinearly interpolated over the
/// corner grid.
///
/// Provided because it is the inverse every caller needs, not because the
/// terrain file carries a heightfield: the interpolation is a property of the
/// mesh this crate builds, so it is the same surface a renderer draws.
#[must_use]
pub fn sample_world_z(terrain: &Terrain, world_x: f32, world_y: f32) -> Option<f32> {
    let (fx, fy) = corner_at(terrain, world_x, world_y)?;
    let tx = fx.floor() as u32;
    let ty = fy.floor() as u32;
    if tx >= terrain.width() || ty >= terrain.height() {
        return None;
    }
    let (dx, dy) = (fx - tx as f32, fy - ty as f32);
    let h = |cx: u32, cy: u32| terrain.corner(cx, cy).unwrap().height_index as f32;
    let bottom = h(tx, ty) * (1.0 - dx) + h(tx + 1, ty) * dx;
    let top = h(tx, ty + 1) * (1.0 - dx) + h(tx + 1, ty + 1) * dx;
    let index = bottom * (1.0 - dy) + top * dy;
    Some((index - Z_REFERENCE_INDEX) * Z_PER_INDEX)
}

/// Coordinate encoding supplied by a checked source reader, rather than measured defaults.
#[derive(Clone, Copy, Debug)]
pub struct Mapping {
    tile: f32,
    bias: f32,
    factor: f32,
}

impl Mapping {
    pub fn new(tile: f32, bias: f32, factor: f32) -> Option<Self> {
        [tile, bias, factor]
            .iter()
            .all(|v| v.is_finite() && *v > 0.0)
            .then_some(Self { tile, bias, factor })
    }

    pub fn tile_size(self) -> f32 {
        self.tile
    }

    pub fn height(self, index: f32) -> f32 {
        index / self.factor - self.bias
    }

    pub fn corner_at(self, terrain: &Terrain, x: f32, y: f32) -> Option<[f32; 2]> {
        let (ox, oy) = terrain.origin();
        let point = [(x - ox) / self.tile, (y - oy) / self.tile];
        (point.iter().all(|v| v.is_finite() && *v >= 0.0)
            && point[0] < terrain.width() as f32
            && point[1] < terrain.height() as f32)
            .then_some(point)
    }

    pub fn sample_ground(self, terrain: &Terrain, x: f32, y: f32) -> Option<f32> {
        let point = self.corner_at(terrain, x, y)?;
        let index = sample_corners(terrain, point, |c| c.height_index as f32)?;
        Some(self.height(index))
    }
}

pub(crate) fn sample_corners(
    terrain: &Terrain,
    point: [f32; 2],
    height: impl Fn(&pcx_map::Corner) -> f32,
) -> Option<f32> {
    let [x, y] = point.map(|v| v.floor() as u32);
    let [dx, dy] = [point[0] - x as f32, point[1] - y as f32];
    let h = |x, y| terrain.corner(x, y).map(&height);
    let bottom = h(x, y)? * (1.0 - dx) + h(x + 1, y)? * dx;
    let top = h(x, y + 1)? * (1.0 - dx) + h(x + 1, y + 1)? * dx;
    Some(bottom * (1.0 - dy) + top * dy)
}

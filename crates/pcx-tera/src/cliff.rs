//! Raw-height diagnostics, not an elevation-layer or native cliff-mesh classifier.
//!
//! Raw corner heights vary continuously; that does not mean W3E lacks discrete
//! cliffs. `Corner::layer_height` supplies their separate elevation-layer field.
//! These diagnostics retain their raw-index meaning. Native cliff mesh selection
//! and ramp geometry remain separate from the layer-aware ground height codec.

use pcx_map::Terrain;

use crate::Unsupported;

/// The measured corner geometry of one tile.
///
/// All deltas are in raw 14-bit `height_index` units; multiply by
/// [`crate::world::Z_PER_INDEX`] for world units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileProfile {
    /// Corner heights, in the mesh's order: bottom-left, bottom-right,
    /// top-left, top-right.
    pub heights: [u16; 4],
    /// Smallest and largest of the four.
    pub min: u16,
    /// Largest of the four.
    pub max: u16,
    /// `max - min`, the tile's full corner-to-corner step.
    pub step: u16,
    /// How many of the four corners sit at `max`.
    pub at_max: u8,
    /// How many of the four corners sit at `min`.
    pub at_min: u8,
    /// `true` when all four corners are equal: a flat quad.
    pub flat: bool,
    /// `true` when exactly two corners sit at `max` and they are diagonal
    /// from each other — WC3's ramp/saddle shape.
    pub diagonal_split: bool,
}

impl TileProfile {
    /// Read the profile of tile `(tx, ty)` from a terrain.
    ///
    /// Returns `None` outside the grid.
    #[must_use]
    pub fn of(terrain: &Terrain, tx: u32, ty: u32) -> Option<Self> {
        let h = |cx: u32, cy: u32| terrain.corner(cx, cy).map(|c| c.height_index);
        let heights = [
            h(tx, ty)?,
            h(tx + 1, ty)?,
            h(tx, ty + 1)?,
            h(tx + 1, ty + 1)?,
        ];
        let min = *heights.iter().min().unwrap();
        let max = *heights.iter().max().unwrap();
        let at_max = heights.iter().filter(|&&x| x == max).count() as u8;
        let at_min = heights.iter().filter(|&&x| x == min).count() as u8;
        // Diagonal split: the two `max` corners are 0 and 3, or 1 and 2.
        let diagonal =
            (heights[0] == max && heights[3] == max) || (heights[1] == max && heights[2] == max);
        Some(Self {
            heights,
            min,
            max,
            step: max - min,
            at_max,
            at_min,
            flat: max == min,
            diagonal_split: at_max == 2 && diagonal,
        })
    }

    /// The tile's step in world units.
    #[must_use]
    pub fn step_world(&self) -> f32 {
        self.step as f32 * crate::world::Z_PER_INDEX
    }

    /// `true` when the tile's two triangles describe a face rather than a flat
    /// quad, i.e. the terrain is sloped here.
    ///
    /// This is measured geometry and needs no threshold: a tile is sloped
    /// exactly when its corners are not equal. It is **not** a cliff test —
    /// see the module docs.
    #[must_use]
    pub fn sloped(&self) -> bool {
        !self.flat
    }
}

/// The cliff classification of a tile.
///
/// Refused everywhere. The variant set is kept uninhabited on purpose: there is
/// no measured rule that can put a tile in it, so any caller asking for one is
/// asking this crate to guess.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliffClass {}

/// Classify a tile as cliff, ramp or flat ground.
///
/// Always [`Unsupported::CliffRule`] today. The three refusals in the module
/// docs are the reason; the measured [`TileProfile`] is handed out by
/// [`TileProfile::of`] instead.
pub fn cliff_class(_profile: &TileProfile) -> Result<CliffClass, Unsupported> {
    Err(Unsupported::CliffRule)
}

/// The world-unit step at which a corner-to-corner difference becomes a cliff
/// wall.
///
/// Always [`Unsupported::CliffThreshold`] today: the measured step distribution
/// is continuous (the largest single step is 862 index units and there is no
/// gap at any candidate value), so no threshold in the data exists to read.
pub fn cliff_threshold() -> Result<f32, Unsupported> {
    Err(Unsupported::CliffThreshold)
}

/// Whether ground is walkable.
///
/// Always [`Unsupported::Walkability`] today. `Terrain.slk`'s `walkable` column
/// is a property of a *tile type*, and the game's pathing is its own: a cliff
/// is unwalkable in the game for reasons that live in `game.dll` and its
/// pathing maps, which this crate has not measured.
pub fn walkable(_profile: &TileProfile) -> Result<bool, Unsupported> {
    Err(Unsupported::Walkability)
}

/// A census of a terrain's tiles by measured profile.
///
/// Counts are what a sweep prints; they are facts about the file, not an
/// interpretation of it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TileCensus {
    /// Tiles examined.
    pub tiles: u64,
    /// Tiles with four equal corners.
    pub flat: u64,
    /// Tiles with unequal corners.
    pub sloped: u64,
    /// Sloped tiles whose two-max corners are diagonal.
    pub diagonal_split: u64,
    /// Tiles whose step exceeds 512 index units (one 128-world-unit candidate
    /// cliff level), a measured count rather than a rule.
    pub over_one_candidate_level: u64,
    /// Largest step seen, in index units.
    pub max_step: u16,
}

impl TileCensus {
    /// Census a whole terrain.
    #[must_use]
    pub fn of(terrain: &Terrain) -> Self {
        let mut c = Self::default();
        for ty in 0..terrain.height() {
            for tx in 0..terrain.width() {
                let Some(p) = TileProfile::of(terrain, tx, ty) else {
                    continue;
                };
                c.tiles += 1;
                if p.flat {
                    c.flat += 1;
                } else {
                    c.sloped += 1;
                    if p.diagonal_split {
                        c.diagonal_split += 1;
                    }
                }
                if p.step > 512 {
                    c.over_one_candidate_level += 1;
                }
                c.max_step = c.max_step.max(p.step);
            }
        }
        c
    }
}

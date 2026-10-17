//! Terrain mesh generation: corner heights → world geometry, ground-slot
//! selection, and the measured limits of cliff detection.
//!
//! This crate is **Bevy-free and `std`-only on purpose**. Everything it returns
//! is plain `positions` / `normals` / `uvs` / `slot_per_vertex` / `indices`
//! vectors that `pcx-gfx` can lift into a `bevy::Mesh`, which is what keeps the
//! terrain constructible headless and for `wasm32-unknown-unknown`.
//!
//! It does **not** re-parse `war3map.w3e`. The terrain arrives as
//! [`pcx_map::Terrain`] and this crate reads
//! [`pcx_map::Corner::height_index`], [`pcx_map::Corner::slot_nibble`] and
//! [`pcx_map::Corner::corner_flags`] out of it, inventing nothing.
//!
//! # The corner → world mapping is measured, not assumed
//!
//! `war3map.w3e` hands out one vertex per tile corner (`corner_size == 1`), a
//! raw 14-bit `height_index`, and no world scale — see [`pcx_map`] for why
//! there is no interior heightfield to interpolate. A mesh needs a world
//! mapping, and multiplying a raw index by a guess is exactly the
//! "plausible and wrong" failure this project refuses, so the mapping was
//! measured against every shipped terrain on both builds:
//!
//! ```text
//! world_x = origin_x + corner_x * TILE_XY          TILE_XY           = 128.0
//! world_y = origin_y + corner_y * TILE_XY          Z_PER_INDEX       =   0.25
//! world_z = (height_index - Z_REFERENCE_INDEX)     Z_REFERENCE_INDEX = 8192.0
//!           * Z_PER_INDEX
//! ```
//!
//! **`Z_PER_INDEX` / `Z_REFERENCE_INDEX`, proven exactly.** A doodad's stored
//! world `z` is its terrain height, so a doodad standing on a *flat* tile (all
//! four of that tile's corners equal) must satisfy
//! `z == (index - 8192) * 0.25` exactly, in `f32`. Across the shipped set:
//!
//! | build | flat-tile doodads | exact matches | distinct indices |
//! |---|---|---|---|
//! | 1.28c | 33 485 | 11 800 | 176, span 7 680..13 312 |
//! | 1.29b | 21 463 | 6 540 | 64, span 7 680..13 312 |
//!
//! A least-squares fit **over the exact matches only** lands on slope
//! `0.250000000001` / intercept `-2048.000000015` (1.28c) and
//! `0.250000000000` / `-2047.999999997` (1.29b), with a worst residual of
//! `2.0e-5` / `9.5e-7` — `f32` rounding, nothing else. Two hundred and forty
//! independent points spanning a 5 632-index range pin the line; that is a
//! proof, not a corroboration. The two candidate hypotheses are refuted on the
//! same evidence:
//!
//! * *linear over the observed index range* (5 477..14 336) — the measured
//!   intercept is `-2048` and the slope is exactly `0.25`, which is the
//!   `index / 4 - 2048` convention, not any fit to the observed range; a
//!   range-fitted line through 11 800 points would have to be `0.25` exactly
//!   for a different reference index, and it is not.
//! * *linear over the full `0..=16383` range* — same refutation: the fit is
//!   `z = 0.25·index - 2048`, i.e. `z = 0.25·(index - 8192)`, so index `8192`
//!   is world `z == 0` and index `0` would be `-2048`, not the bottom of the
//!   file's range.
//!
//! **`TILE_XY`, and the grid's orientation.** `(x - origin_x) / 128` is the
//! corner column and `(y - origin_y) / 128` is the corner row, **with the
//! row not flipped**. Measured: the exact-match rate is 35.2 % of flat-tile
//! doodads under that mapping and only 9.6 % with the row flipped; the offsets
//! `±0.125` … `±0.25` of a tile change nothing while `±0.5` collapses it, so
//! the grid is aligned to the origin floats and not shifted. Every doodad of
//! every shipped map on both builds falls inside `[origin, origin + 128·w] ×
//! [origin, origin + 128·h]` (the widest observed doodad span is 99.35 % of the
//! grid), and both origin floats are a multiple of 128 — of 256, in fact — on
//! all 132 terrains. The origin floats are therefore the world position of
//! corner `(0, 0)`, the lower-left corner of the grid.
//!
//! # Slot selection is proven; blending weights are refused
//!
//! The per-corner low nibble is the ground slot index **directly**: not one of
//! 88 (1.28c) or 44 (1.29b) shipped terrains carries a nibble `>= N`, while the
//! reader reading `0xF ^ nibble` (then clamping `>= N` to 0) disagrees with the
//! writer reading on 1 343 225 (1.28c) and 547 800 (1.29b) corners and would
//! collapse almost every one of them to slot 0. So the nibble is a slot index,
//! and a tile's four corners may carry up to four different ones
//! (measured: 1 distinct on 1 052 223 tiles, 2 on 462 297, 3 on 86 065, 4 on
//! 5 047 of the 1.28c corpus).
//!
//! What the bytes do **not** say is a numeric weight for those four slots.
//! [`GroundTiles::blend`] stays [`Unsupported::BlendWeights`]. A ground
//! renderer samples the alpha stored in the variation cells.
//! [`ground::tile_texture_selectors`] packs the slot and that cell. It does
//! not invent a weight, and a wrong weight would be a whole-terrain error that
//! looks fine on flat ground.
//!
//! # Cliff detection is refused, and here is the measurement that refuses it
//!
//! Three measured facts, each of which kills a tempting rule:
//!
//! 1. **Cliff levels are not quantised.** Among 1 009 520 stepped tiles, only
//!    524 have a corner-to-corner step that is a multiple of 512 index units.
//!    The most common steps are 65..256 and 257..512 index units and the step
//!    distribution is continuous, so "one level = 128 world units" is not a
//!    property of the data.
//! 2. **No ground slot is a cliff slot.** `TerrainArt\Terrain.slk` carries a
//!    `cliffSet` column separate from `-1` on 36 of its 177 tile rows, and
//!    **zero** corners anywhere in the shipped set select one of them. The
//!    ground slot list of a shipped map never contains a cliff-set tile, so
//!    "the slot changed ⇒ this is cliff ground" is refuted.
//! 3. **A slot change is a weak cliff signal.** 20.6 % of perfectly flat tiles
//!    also have a slot change between their own corners, against 47.7 % of
//!    tiles with a 257..512-index step. A rule built on that would misclassify
//!    a fifth of the map's flat ground.
//!
//! So the cliff look is a *renderer* decision — a threshold on the corner step
//! plus a vertical wall quad — and neither the threshold nor the wall is in any
//! shipped byte this crate measured. The measured primitives are handed out
//! ([`TileProfile`]) and the decision is refused (see [`cliff`]).
//!
//! [`mod@cliff_wall`] constructs optional wall quads from caller-supplied geometry
//! and a caller-supplied threshold; it does not answer the refused cliff rule.
//! [`WaterData`] preserves original corner records with explicit provenance.
//! W3E corner flags and a caller-selected `Water.slk` row supply world levels
//! and surface quads. Colour, texture frames, and animation stay raw cells.
//! See the crate's `WATER_EVIDENCE.md`.

use std::fmt;

pub use pcx_map::{Corner, Terrain};

pub mod cliff;
pub mod cliff_wall;
pub mod ground;
pub mod mesh;
pub mod tiles;
pub mod water;
pub mod world;

#[cfg(test)]
mod tests;

pub use cliff::{CliffClass, TileProfile, cliff_class};
pub use cliff_wall::{WallGeometryError, WallQuad, cliff_wall, cliff_walls};
pub use ground::{
    EMPTY_SELECTOR, GroundSelectError, GroundSlot, GroundTiles, ground_variation, reference_layer,
    select_slot, slot_fourcc, tile_texture_selectors,
};
pub use mesh::{ManifoldReport, TerrainMesh};
pub use tiles::{TileCatalog, TilesetError};
pub use water::{
    MissingWaterProvenance, RawWaterCorner, WaterBudget, WaterData, WaterError, WaterMesh,
    WaterProfile, WaterProvenance, WaterSelection,
};
pub use world::{
    TILE_XY, Z_PER_INDEX, Z_REFERENCE_INDEX, corner_world, corner_world_scaled, height_index,
    world_z,
};

/// Errors this crate can return.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("tileset catalogue: {0}")]
    Tileset(#[from] TilesetError),
}

impl From<pcx_slq::Error> for Error {
    fn from(e: pcx_slq::Error) -> Self {
        Error::Tileset(TilesetError::Slk(e))
    }
}

/// A semantic that is real in the game but is **not** measured in any shipped
/// byte this crate read.
///
/// Every one of these is returned rather than guessed. A caller that wanted a
/// plausible answer instead of this type is asking for the one failure mode the
/// project refuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unsupported {
    /// The four high-nibble bits of the slot/flags byte.
    ///
    /// Measured: their set-rate does not depend on whether the corner they sit
    /// on is the high or the low one inside its own tile (the per-corner
    /// distributions are identical across all four corner positions within a
    /// tile-geometry class), and they do not track map position either. The
    /// bits are handed out raw as [`mesh::TerrainMesh::flags_per_vertex`] and
    /// interpreted nowhere.
    CornerFlags,
    /// How a tile weights its four corner slots across the quad.
    BlendWeights,
    /// Which tiles are cliff walls, which are ramps.
    CliffRule,
    /// Where the cliff threshold sits in world units.
    CliffThreshold,
    /// Retained so older callers can name the refusal. [`WaterData`] does not
    /// return it: a selected [`WaterProfile`] decodes the second word.
    WaterLevel,
    /// Retained so older callers can name the refusal. [`WaterData::mesh`]
    /// builds quads when a profile is supplied and returns an empty mesh when
    /// no tile is wet.
    WaterGeometry,
    /// Whether ground is walkable. The game answers this from its own pathing
    /// tables and `game.dll`, not from the terrain file; `Terrain.slk`'s
    /// `walkable` column is per *tile type*, not per tile.
    Walkability,
}

impl fmt::Display for Unsupported {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Unsupported::CornerFlags => {
                "the four corner-flag bits have no measured meaning; they are carried raw"
            }
            Unsupported::BlendWeights => {
                "the weights blending a tile's four corner slots are not in the w3e bytes"
            }
            Unsupported::CliffRule => {
                "the cliff/ramp decision is a renderer rule; no shipped map selects a \
                 cliff-set ground slot (0 of 132), so it is not in the ground data"
            }
            Unsupported::CliffThreshold => {
                "the cliff threshold in world units is a renderer constant, not a measured one"
            }
            Unsupported::WaterLevel => {
                "use WaterData with a selected Water.slk profile; this variant is not \
                 returned for a decoded level (see WATER_EVIDENCE.md)"
            }
            Unsupported::WaterGeometry => {
                "use WaterData::mesh with a selected Water.slk profile; this variant is \
                 not returned for that mesh (see WATER_EVIDENCE.md)"
            }
            Unsupported::Walkability => "walkability is game.dll's, not the terrain file's",
        };
        f.write_str(s)
    }
}

/// `std::error::Error` for [`Unsupported`], so it can sit in a `Result`.
impl std::error::Error for Unsupported {}

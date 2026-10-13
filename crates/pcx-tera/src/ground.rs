//! Ground slot selection: what the per-corner nibble means, and the one thing
//! it cannot tell you.
//!
//! **Proven: the nibble is the slot index, directly.** Across 88 terrains on
//! 1.28c and 44 on 1.29b, not one corner carries a nibble `>= N` where `N` is
//! the map's own ground-slot count. The competing reading — the game's reader
//! recovers `0xF ^ nibble` and clamps anything `>= N` to 0 — disagrees with the
//! writer on 1 343 225 corners (1.28c) and 547 800 (1.29b) and would collapse
//! almost every corner to slot 0, which no map's appearance supports. The test
//! suite pins the zero-out-of-range census, which is what makes this a proof
//! rather than a preference: the direct reading is the only one of the two that
//! is *consistent with the bytes on every shipped file*.
//!
//! **Refused: the blend weights.** A tile's four corners may carry up to four
//! different slot indices (measured: 1 distinct on 1 052 223 tiles, 2 on
//! 462 297, 3 on 86 065, 4 on 5 047 of the 1.28c corpus). How the renderer
//! weights those four across the quad is not in the file, so it is refused —
//! see [`Unsupported::BlendWeights`].

use pcx_map::{Corner, Terrain};

use crate::{Unsupported, world::TILE_XY};

/// One ground texture slot as a tile's corner selects it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroundSlot {
    /// Index into the terrain's own slot list.
    pub index: u8,
    /// The slot fourcc, from the terrain's first slot list.
    pub fourcc: [u8; 4],
}

/// The ground slot a corner selects, or `None` if its nibble is out of range.
///
/// `None` is the refused case, and it is not a corner case in practice: the
/// census above says it happens on zero of 132 shipped terrains. A *new* map
/// that breaks it gets `None` rather than a silently clamped slot.
#[must_use]
pub fn select_slot(terrain: &Terrain, corner: &Corner) -> Option<GroundSlot> {
    let index = corner.slot_nibble;
    let fourcc = *terrain.slots().get(index as usize)?;
    Some(GroundSlot { index, fourcc })
}

/// The slot fourcc of a raw nibble, ignoring range.
///
/// For callers that already validated the nibble (or that want to see what a
/// malformed map would have said).
#[must_use]
pub fn slot_fourcc(terrain: &Terrain, nibble: u8) -> Option<[u8; 4]> {
    terrain.slots().get(nibble as usize).copied()
}

/// A terrain's whole ground texture set, in the file's own order.
///
/// This is what a renderer builds its texture set from: one entry per slot the
/// map declares, each with the fourcc that identifies it in
/// `TerrainArt\Terrain.slk`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroundTiles {
    /// Slot fourccs, in file order. The nth is what slot nibble `n` selects.
    fourccs: Vec<[u8; 4]>,
    /// Corners whose nibble was out of range, as `(row-major index, nibble)`.
    /// Empty on every shipped terrain; see the module docs.
    out_of_range: Vec<(usize, u8)>,
    /// The terrain's declared default slot id, handed out raw.
    default_slot: u8,
}

impl GroundTiles {
    /// Read a terrain's ground slots and audit its corner nibbles.
    #[must_use]
    pub fn of(terrain: &Terrain) -> Self {
        let fourccs = terrain.slots().to_vec();
        let mut out_of_range = Vec::new();
        for (i, corner) in terrain.corners().iter().enumerate() {
            if corner.slot_nibble as usize >= fourccs.len() {
                out_of_range.push((i, corner.slot_nibble));
            }
        }
        Self {
            fourccs,
            out_of_range,
            default_slot: terrain.default_slot(),
        }
    }

    /// Number of ground slots the map declares.
    #[must_use]
    pub fn len(&self) -> usize {
        self.fourccs.len()
    }

    /// `true` when the map declares no ground slots.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fourccs.is_empty()
    }

    /// The nth slot fourcc.
    #[must_use]
    pub fn fourcc(&self, index: u8) -> Option<[u8; 4]> {
        self.fourccs.get(index as usize).copied()
    }

    /// All slot fourccs, in file order.
    #[must_use]
    pub fn fourccs(&self) -> &[[u8; 4]] {
        &self.fourccs
    }

    /// The terrain's declared default slot id, raw and uninterpreted.
    #[must_use]
    pub fn default_slot(&self) -> u8 {
        self.default_slot
    }

    /// Corners whose nibble fell outside the declared slot list.
    ///
    /// Empty for every shipped terrain on both builds; a non-empty result means
    /// the map is outside the measurement, and a renderer must not guess.
    #[must_use]
    pub fn out_of_range(&self) -> &[(usize, u8)] {
        &self.out_of_range
    }

    /// The distinct slot indices the terrain's corners actually select,
    /// ascending.
    #[must_use]
    pub fn used_indices(&self, terrain: &Terrain) -> Vec<u8> {
        let mut v: Vec<u8> = terrain
            .corners()
            .iter()
            .map(|c| c.slot_nibble)
            .filter(|n| (*n as usize) < self.fourccs.len())
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    /// How the tile at `(tx, ty)` selects its slots: the four corner slots, or
    /// `None` for a corner whose nibble is out of range.
    ///
    /// The order is the mesh's own: bottom-left, bottom-right, top-left,
    /// top-right.
    #[must_use]
    pub fn tile_slots(&self, terrain: &Terrain, tx: u32, ty: u32) -> [Option<GroundSlot>; 4] {
        let corner = |cx: u32, cy: u32| terrain.corner(cx, cy).copied();
        let pick = |cx: u32, cy: u32| -> Option<GroundSlot> {
            let c = corner(cx, cy)?;
            let index = c.slot_nibble;
            let fourcc = *self.fourccs.get(index as usize)?;
            Some(GroundSlot { index, fourcc })
        };
        [
            pick(tx, ty),
            pick(tx + 1, ty),
            pick(tx, ty + 1),
            pick(tx + 1, ty + 1),
        ]
    }

    /// The blend of that tile's four corners.
    ///
    /// Always refused today. The function exists so a caller cannot
    /// accidentally invent the weights: the only way to get a blend variant is
    /// to have measured one, and none has been measured. See
    /// [`Unsupported::BlendWeights`].
    pub fn blend(
        &self,
        _terrain: &Terrain,
        _tx: u32,
        _ty: u32,
    ) -> Result<BlendVariant, Unsupported> {
        Err(Unsupported::BlendWeights)
    }

    /// World-space size of one ground texture repeat: one tile.
    #[must_use]
    pub fn texture_repeat_world() -> f32 {
        TILE_XY
    }
}

/// A tile's blend between its corner slots.
///
/// Deliberately empty: nothing in `war3map.w3e` names a variant, and the shape
/// is kept so a future measurement has somewhere to land without changing the
/// call sites.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendVariant {}

/// The four corner flags of a single corner, bits kept as-is.
///
/// The bits are handed back because `game.dll`'s own reader folds them into a
/// per-corner flag word, but *which bit is what* is not in any byte this crate
/// measured: the set-rate of each bit is the same for a corner that is the high
/// one and the low one inside its own tile, and none of them tracks map
/// position. See [`Unsupported::CornerFlags`].
#[must_use]
pub fn corner_flags_meaning() -> Result<CornerFlags, Unsupported> {
    Err(Unsupported::CornerFlags)
}

/// What the four flag bits mean. Uninhabited, because the answer is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CornerFlags {}

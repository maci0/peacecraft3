//! Ground slot selection: what the per-corner nibble means, and the one thing
//! it cannot tell you.
//!
//! **Proven: the nibble is the slot index, directly.** Across 88 terrains on
//! 1.28c and 44 on 1.29b, not one corner carries a nibble `>= N` where `N` is
//! the map's own ground-slot count. The competing reading (the game's reader
//! recovers `0xF ^ nibble` and clamps anything `>= N` to 0) disagrees with the
//! writer on 1 343 225 corners (1.28c) and 547 800 (1.29b) and would collapse
//! almost every corner to slot 0, which no map's appearance supports. The test
//! suite pins the zero-out-of-range census, which is what makes this a proof
//! rather than a preference: the direct reading is the only one of the two that
//! is *consistent with the bytes on every shipped file*.
//!
//! **Refused: the blend weights.** A tile's four corners may carry up to four
//! different slot indices (measured: 1 distinct on 1 052 223 tiles, 2 on
//! 462 297, 3 on 86 065, 4 on 5 047 of the 1.28c corpus). How the renderer
//! weights those four across the quad is not in the file, so it is refused.
//! See [`Unsupported::BlendWeights`].
//!
//! [`tile_texture_selectors`] packs each tile for a ground renderer. The low
//! five bits are the slot. The next five bits are the variation cell of the
//! lowest slot, or the corner mask of a later slot. Empty components stay
//! [`EMPTY_SELECTOR`]. The cell alpha is the blend. This does not store a
//! numeric weight, and [`GroundTiles::blend`] stays refused.

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

/// Selector component that does not name a slot. The renderer samples nothing.
pub const EMPTY_SELECTOR: u16 = 17;

/// Low five bits of the corner's fifth byte.
///
/// The pinned loader calls this `groundVariation`. Bits above that stay unused
/// here, including the cliff variation in the top three bits.
#[must_use]
pub fn ground_variation(corner: &Corner) -> u8 {
    corner.byte5 & 0x1F
}

/// Variation cell inside one slot's sheet.
///
/// `extended` is that slot's own sheet (`width / tile == 8`), not a property
/// of the corner. The mapping is the pinned `Terrain.getVariation`.
#[must_use]
pub fn reference_layer(extended: bool, variation: u8) -> u8 {
    if extended {
        if variation <= 15 {
            16 + variation
        } else if variation == 16 {
            15
        } else {
            0
        }
    } else if variation == 0 {
        0
    } else {
        15
    }
}

/// Why a tile cannot be packed into four selectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum GroundSelectError {
    /// `extended_by_slot` is not one flag per terrain slot.
    #[error("extended flags {extended} do not match slot count {slots}")]
    ExtendedLen { slots: usize, extended: usize },
    /// `(tx, ty)` is outside the tile grid.
    #[error("tile {tx},{ty} is outside the terrain")]
    TileRange { tx: u32, ty: u32 },
    /// A corner nibble is outside `terrain.slots()`. Not clamped to slot 0.
    #[error("corner nibble {nibble} is outside {slot_count} ground slots")]
    SlotRange { nibble: u8, slot_count: usize },
}

/// Four ground selectors for the tile at `(tx, ty)`, mesh corner order.
///
/// Order is bottom-left, bottom-right, top-left, top-right. Distinct slots are
/// written in ascending id. Component 0 uses the bottom-left corner's variation
/// and the lowest slot's sheet flag, even when that slot is not the bottom-left
/// corner's slot. Later components use the corner mask: bit 0 bottom-right,
/// bit 1 bottom-left, bit 2 top-right, bit 3 top-left.
pub fn tile_texture_selectors(
    terrain: &Terrain,
    extended_by_slot: &[bool],
    tx: u32,
    ty: u32,
) -> Result<[u16; 4], GroundSelectError> {
    if extended_by_slot.len() != terrain.slots().len() {
        return Err(GroundSelectError::ExtendedLen {
            slots: terrain.slots().len(),
            extended: extended_by_slot.len(),
        });
    }
    if tx >= terrain.width() || ty >= terrain.height() {
        return Err(GroundSelectError::TileRange { tx, ty });
    }
    let corners = [
        terrain.corner(tx, ty),
        terrain.corner(tx + 1, ty),
        terrain.corner(tx, ty + 1),
        terrain.corner(tx + 1, ty + 1),
    ];
    let mut nibbles = [0u8; 4];
    for (index, corner) in corners.iter().enumerate() {
        let corner = corner.ok_or(GroundSelectError::TileRange { tx, ty })?;
        let nibble = corner.slot_nibble;
        if nibble as usize >= terrain.slots().len() {
            return Err(GroundSelectError::SlotRange {
                nibble,
                slot_count: terrain.slots().len(),
            });
        }
        nibbles[index] = nibble;
    }
    let mut ids = nibbles;
    // Five compare-exchanges sort four values, with no generic sort setup per tile.
    for (low, high) in [(0, 1), (2, 3), (0, 2), (1, 3), (1, 2)] {
        if ids[low] > ids[high] {
            ids.swap(low, high);
        }
    }
    let mut unique = [0u8; 4];
    let mut count = 0usize;
    for id in ids {
        if count == 0 || unique[count - 1] != id {
            unique[count] = id;
            count += 1;
        }
    }
    let mut out = [EMPTY_SELECTOR; 4];
    let Some(lowest) = unique.first().copied().filter(|_| count > 0) else {
        return Ok(out);
    };
    let bottom_left = corners[0].ok_or(GroundSelectError::TileRange { tx, ty })?;
    let layer = reference_layer(
        extended_by_slot[usize::from(lowest)],
        ground_variation(bottom_left),
    );
    out[0] = u16::from(lowest) + (u16::from(layer) << 5);
    for (component, id) in unique[1..count].iter().copied().enumerate() {
        let mut mask = 0u16;
        if nibbles[1] == id {
            mask |= 1 << 0;
        }
        if nibbles[0] == id {
            mask |= 1 << 1;
        }
        if nibbles[3] == id {
            mask |= 1 << 2;
        }
        if nibbles[2] == id {
            mask |= 1 << 3;
        }
        out[component + 1] = u16::from(id) + (mask << 5);
    }
    Ok(out)
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
pub fn corner_flags_meaning() -> Result<CornerFlags, Unsupported> {
    Err(Unsupported::CornerFlags)
}

/// What the four flag bits mean. Uninhabited, because the answer is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CornerFlags {}

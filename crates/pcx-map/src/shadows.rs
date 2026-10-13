//! `war3map.shd` — the shadow map.
//!
//! # The file carries its own grid
//!
//! The shadow map is **not** sampled at the terrain corners and **not**
//! derived at load time from a height ramp: there is no interior heightmap in
//! `war3map.w3e` to derive it from (the corner table consumes that file
//! exactly, see the crate docs), and this file is a complete mask in its own
//! right. One byte per cell, on a grid of `(4·w) × (4·h)` — four cells per
//! tile edge, sixteen per tile — where `w × h` is the terrain's tile
//! dimension.
//!
//! ```text
//! (4·w) · (4·h) bytes, no header, row-major, row 0 first
//! ```
//!
//! # Framing relation
//!
//! ```text
//! file_len == 16 · width · height
//! ```
//!
//! against the *terrain's* tile dimensions — the only place the file's grid
//! size is written down. Holds exactly on all 132 shipped files. A shadow map
//! whose length does not fit its map is refused; a cell width guessed from
//! the file alone would be a whole-world error.
//!
//! Cell values on every shipped file are exactly `0x00` (unshadowed) and
//! `0xFF` (shadowed). The parser hands back any byte unchanged — a mid-value
//! is not refused, because the *reader* for one has not been proved, only the
//! writer's output has been measured.

use crate::Error;

/// Cells per tile edge. The file has `4 · width` columns and `4 · height`
/// rows; this is the factor, not a guess.
pub const SHADOW_CELLS_PER_TILE: usize = 4;

/// The value every shipped file uses for a shadowed cell.
pub const SHADOW_ON: u8 = 0xFF;

/// The value every shipped file uses for an unshadowed cell.
pub const SHADOW_OFF: u8 = 0x00;

/// A decoded `war3map.shd`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shadows {
    /// Terrain width in tiles.
    width: u32,
    /// Terrain height in tiles.
    height: u32,
    cells: Vec<u8>,
    raw: Vec<u8>,
}

impl Shadows {
    /// Parse shadow bytes against the terrain's tile dimensions.
    ///
    /// The dimensions are an argument because the file does not contain them:
    /// `bytes.len() == 16 · width · height` is the proof that they are the
    /// right ones. Anything else is refused.
    pub fn from_bytes(bytes: Vec<u8>, width: u32, height: u32) -> Result<Self, Error> {
        let cols = SHADOW_CELLS_PER_TILE * width as usize;
        let rows = SHADOW_CELLS_PER_TILE * height as usize;
        let need = cols * rows;
        if bytes.len() != need {
            return Err(Error::BadShadowSize {
                width,
                height,
                have: bytes.len(),
                need,
            });
        }
        Ok(Self {
            width,
            height,
            cells: bytes.clone(),
            raw: bytes,
        })
    }

    /// The file exactly as stored.
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }

    /// Terrain width in tiles.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Terrain height in tiles.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Grid columns: `4 · width`.
    pub fn columns(&self) -> usize {
        SHADOW_CELLS_PER_TILE * self.width as usize
    }

    /// Grid rows: `4 · height`.
    pub fn rows(&self) -> usize {
        SHADOW_CELLS_PER_TILE * self.height as usize
    }

    /// The raw cell at grid `(x, y)`, x across and y up, row-major.
    pub fn cell(&self, x: u32, y: u32) -> Option<u8> {
        let (cols, rows) = (self.columns(), self.rows());
        if x as usize >= cols || y as usize >= rows {
            return None;
        }
        Some(self.cells[y as usize * cols + x as usize])
    }

    /// `true` when the cell is exactly [`SHADOW_ON`].
    ///
    /// Mid-values answer `false`; see the module docs for why they are not
    /// refused at parse time.
    pub fn is_shadowed(&self, x: u32, y: u32) -> Option<bool> {
        self.cell(x, y).map(|v| v == SHADOW_ON)
    }

    /// Every distinct cell value present.
    pub fn values_used(&self) -> std::collections::BTreeSet<u8> {
        self.cells.iter().copied().collect()
    }

    /// Canonical byte form: the tile dimensions, then the grid.
    ///
    /// The dimensions are prefixed on purpose. The stored file does not
    /// commit to its own grid shape — the same byte run is a valid `96×128`
    /// or `128×96` shadow map — so a hash of the file alone cannot tell two
    /// decoders apart when they disagree about the map. The canonical hash
    /// can.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 + self.cells.len());
        out.extend_from_slice(&self.width.to_le_bytes());
        out.extend_from_slice(&self.height.to_le_bytes());
        out.extend_from_slice(&self.cells);
        out
    }

    /// FNV-1a 64 over [`Self::canonical_bytes`].
    pub fn canonical_hash(&self) -> u64 {
        pcx_mpq::fnv1a64(&self.canonical_bytes())
    }
}

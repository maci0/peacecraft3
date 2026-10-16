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
//! the file alone would be a whole-world error. The product is checked: a
//! grid that does not fit `usize` is refused instead of wrapping into a
//! shorter length. A zero tile axis still has byte length 0 when the other
//! axis's cell count fits `usize`.
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
    /// `4 · width`, recorded only after that count fits `usize`.
    columns: usize,
    /// `4 · height`, recorded only after that count fits `usize`.
    rows: usize,
    /// The file bytes. Also the cell grid: one buffer, row-major.
    raw: Vec<u8>,
}

fn shadow_layout(width: u32, height: u32) -> Result<(usize, usize, usize), Error> {
    let overflow = Error::ShadowSizeOverflow { width, height };
    let columns = SHADOW_CELLS_PER_TILE
        .checked_mul(width as usize)
        .ok_or(overflow)?;
    let overflow = Error::ShadowSizeOverflow { width, height };
    let rows = SHADOW_CELLS_PER_TILE
        .checked_mul(height as usize)
        .ok_or(overflow)?;
    let overflow = Error::ShadowSizeOverflow { width, height };
    let need = columns.checked_mul(rows).ok_or(overflow)?;
    Ok((columns, rows, need))
}

impl Shadows {
    /// Parse shadow bytes against the terrain's tile dimensions.
    ///
    /// The dimensions are an argument because the file does not contain them:
    /// `bytes.len() == 16 · width · height` is the proof that they are the
    /// right ones, when that product fits `usize`. Anything else is refused.
    /// [`Map::shadows`](crate::Map::shadows) passes the terrain tile width and
    /// height through this function unchanged.
    pub fn from_bytes(bytes: Vec<u8>, width: u32, height: u32) -> Result<Self, Error> {
        let (columns, rows, need) = shadow_layout(width, height)?;
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
            columns,
            rows,
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
        self.columns
    }

    /// Grid rows: `4 · height`.
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// The raw cell at grid `(x, y)`, x across and y up, row-major.
    pub fn cell(&self, x: u32, y: u32) -> Option<u8> {
        if x as usize >= self.columns || y as usize >= self.rows {
            return None;
        }
        let index = (y as usize)
            .checked_mul(self.columns)?
            .checked_add(x as usize)?;
        self.raw.get(index).copied()
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
        self.raw.iter().copied().collect()
    }

    /// Canonical byte form: the tile dimensions, then the grid.
    ///
    /// The dimensions are prefixed on purpose. The stored file does not
    /// commit to its own grid shape — the same byte run is a valid `96×128`
    /// or `128×96` shadow map — so a hash of the file alone cannot tell two
    /// decoders apart when they disagree about the map. The canonical hash
    /// can.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 + self.raw.len());
        out.extend_from_slice(&self.width.to_le_bytes());
        out.extend_from_slice(&self.height.to_le_bytes());
        out.extend_from_slice(&self.raw);
        out
    }

    /// FNV-1a 64 over [`Self::canonical_bytes`].
    pub fn canonical_hash(&self) -> u64 {
        pcx_mpq::fnv1a64(&self.canonical_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn overflow(width: u32, height: u32) -> bool {
        matches!(
            Shadows::from_bytes(Vec::new(), width, height),
            Err(Error::ShadowSizeOverflow {
                width: got_w,
                height: got_h,
            }) if got_w == width && got_h == height
        )
    }

    #[test]
    fn zero_axis_keeps_an_empty_grid_when_the_other_axis_fits() {
        let empty = Shadows::from_bytes(Vec::new(), 0, 0).unwrap();
        assert_eq!(
            (empty.columns(), empty.rows(), empty.raw().len()),
            (0, 0, 0)
        );
        assert_eq!(empty.cell(0, 0), None);
        let flat = Shadows::from_bytes(Vec::new(), 0, 4).unwrap();
        assert_eq!((flat.columns(), flat.rows()), (0, 16));
        let tall = Shadows::from_bytes(Vec::new(), 3, 0).unwrap();
        assert_eq!((tall.columns(), tall.rows()), (12, 0));
        assert!(matches!(
            Shadows::from_bytes(vec![0], 0, 1),
            Err(Error::BadShadowSize {
                width: 0,
                height: 1,
                have: 1,
                need: 0,
            })
        ));
    }

    #[test]
    fn caller_grid_uses_one_buffer_and_checked_indices() {
        // Same call `Map::shadows` makes: caller tile width and height, file bytes.
        let bytes: Vec<u8> = (0u8..16).collect();
        let parsed = Shadows::from_bytes(bytes.clone(), 1, 1).unwrap();
        assert_eq!(parsed.raw(), bytes.as_slice());
        assert_eq!((parsed.columns(), parsed.rows()), (4, 4));
        for y in 0..4u32 {
            for x in 0..4u32 {
                let index = y as usize * 4 + x as usize;
                assert_eq!(parsed.cell(x, y), Some(bytes[index]));
            }
        }
        assert_eq!(parsed.cell(4, 0), None);
        assert_eq!(parsed.is_shadowed(0, 0), Some(false));
        let marked = {
            let mut cells = bytes;
            cells[0] = SHADOW_ON;
            cells
        };
        let shadowed = Shadows::from_bytes(marked.clone(), 1, 1).unwrap();
        assert_eq!(shadowed.is_shadowed(0, 0), Some(true));
        assert_eq!(shadowed.canonical_bytes()[8..], marked);
        assert_eq!(
            shadowed.canonical_hash(),
            pcx_mpq::fnv1a64(&shadowed.canonical_bytes())
        );
        assert!(matches!(
            Shadows::from_bytes(vec![0; 15], 1, 1),
            Err(Error::BadShadowSize {
                width: 1,
                height: 1,
                have: 15,
                need: 16,
            })
        ));
    }

    #[test]
    fn unrepresentable_caller_grid_is_refused() {
        assert!(overflow(1 << 30, 1 << 30));
        assert!(overflow(u32::MAX, u32::MAX));
        let narrow = Shadows::from_bytes(Vec::new(), 1 << 30, 1);
        if usize::BITS >= 64 {
            let wide_need = SHADOW_CELLS_PER_TILE
                .checked_mul(1usize << 30)
                .and_then(|columns| columns.checked_mul(SHADOW_CELLS_PER_TILE))
                .expect("64-bit 2^30 by 1 shadow length fits");
            assert!(matches!(
                narrow,
                Err(Error::BadShadowSize {
                    width: w,
                    height: 1,
                    have: 0,
                    need,
                }) if w == 1 << 30 && need == wide_need
            ));
            let zero_by_huge = Shadows::from_bytes(Vec::new(), 0, 1 << 30).unwrap();
            let huge_rows = SHADOW_CELLS_PER_TILE
                .checked_mul(1usize << 30)
                .expect("64-bit cell axis fits");
            assert_eq!(
                (
                    zero_by_huge.columns(),
                    zero_by_huge.rows(),
                    zero_by_huge.raw().len()
                ),
                (0, huge_rows, 0)
            );
            assert_eq!(zero_by_huge.cell(0, 0), None);
        } else if usize::BITS == 32 {
            assert!(
                matches!(
                    narrow,
                    Err(Error::ShadowSizeOverflow { width, height: 1 }) if width == 1 << 30
                ),
                "{narrow:?}"
            );
            assert!(overflow(0, 1 << 30));
            assert!(overflow(1 << 30, 0));
        } else {
            panic!(
                "pointer width {} is outside the checked shadow targets",
                usize::BITS
            );
        }
    }
}

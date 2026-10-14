//! The eight-piece backdrop edge atlas: slice geometry, and what is not proven.
//!
//! A backdrop draws its border from a separate `BackdropEdgeFile` cut into
//! pieces. The FDF names them:
//!
//! ```text
//! BackdropCornerFlags "UL|UR|BL|BR|T|L|B|R",
//! BackdropCornerSize  0.016,
//! BackdropEdgeFile  "UI\Widgets\Glues\GlueScreen-Button1-BackdropBorder.blp",
//! ```
//!
//! and the shipped file is **256 × 32**, alpha 8, paletted (`blpx info`).
//!
//! ## What the shipped bytes suggest — a **candidate** partition
//!
//! The texture is 256 × 32, so it *can* be cut into eight square 32×32 slices.
//! That is a candidate, not the native framing. **The number of named flags does
//! not bound the slot count**: the token table ships `BackdropLeftFile`,
//! `BackdropTopFile`, `BackdropRightFile`, `BackdropBottomFile`,
//! `BackdropHalfSides` and `BackdropMirrored` alongside `BackdropEdgeFile` and
//! `BackdropCornerFlags`, so a texture may carry every native slot while the FDF
//! enables only some, and mirroring may mean it carries half. Treating
//! `width == height × enabled_flags` as a validity rule would reject a legal file.
//!
//! ## What is **not** proven
//!
//! **Which slice belongs to which flag**, and even how many slots the native
//! atlas has. The flags' written order is authored, but nothing in the FDF or the
//! texture says the layout follows it, and no native observation of the edge
//! renderer was made. So this module exposes candidate slice geometry **by slot**
//! and refuses to answer `flag -> slot`. Guessing it would put the corners on the
//! edges, which is a plausible-looking wrong border.
//!
//! **This is not ready for rendering.** See `docs/re-notes/menu-border-atlas.md`
//! for the diagnosed blocker, the exact token/handler addresses, and the next
//! probe.

use std::fmt;

/// Why an edge atlas could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BorderError {
    /// No flags were named, so there is no atlas to cut.
    NoFlags,
    /// A flag token the FDF does not define.
    UnknownFlag(String),
    /// The texture width is not a whole multiple of its height, so it cannot be
    /// cut into square slices of the texture height.
    NotDivisible {
        /// Texture width in texels.
        width: u32,
        /// Texture height in texels.
        height: u32,
    },
    /// A slice index past the candidate slice count.
    SlotOutOfRange {
        /// The requested slot.
        slot: usize,
        /// The candidate slice count.
        slices: usize,
    },
    /// The texture has no height to take the slice size from.
    NoHeight,
}

impl fmt::Display for BorderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoFlags => write!(f, "no corner flags were named"),
            Self::UnknownFlag(s) => write!(f, "unknown corner flag {s:?}"),
            Self::NotDivisible { width, height } => write!(
                f,
                "{width}x{height} is not a whole number of {height}x{height} square slices"
            ),
            Self::SlotOutOfRange { slot, slices } => {
                write!(f, "slice {slot} is past the {slices} candidate slices")
            }
            Self::NoHeight => write!(f, "the texture has no height"),
        }
    }
}

impl std::error::Error for BorderError {}

/// One of the eight pieces a backdrop's `BackdropCornerFlags` can name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CornerFlag {
    /// `UL`
    UpperLeft,
    /// `UR`
    UpperRight,
    /// `BL`
    BottomLeft,
    /// `BR`
    BottomRight,
    /// `T`
    Top,
    /// `L`
    Left,
    /// `B`
    Bottom,
    /// `R`
    Right,
}

impl CornerFlag {
    /// The flag a token names.
    ///
    /// # Errors
    ///
    /// [`BorderError::UnknownFlag`] for anything the FDF does not define.
    pub fn parse(token: &str) -> Result<Self, BorderError> {
        Ok(match token.trim().to_ascii_uppercase().as_str() {
            "UL" => Self::UpperLeft,
            "UR" => Self::UpperRight,
            "BL" => Self::BottomLeft,
            "BR" => Self::BottomRight,
            "T" => Self::Top,
            "L" => Self::Left,
            "B" => Self::Bottom,
            "R" => Self::Right,
            other => return Err(BorderError::UnknownFlag(other.to_string())),
        })
    }

    /// The `UL|UR|…` string an FDF writes.
    ///
    /// # Errors
    ///
    /// [`BorderError::UnknownFlag`] when a token is not one of the eight.
    pub fn parse_flags(list: &str) -> Result<Vec<Self>, BorderError> {
        let mut out = Vec::new();
        for token in list.split('|') {
            if token.trim().is_empty() {
                continue;
            }
            out.push(Self::parse(token)?);
        }
        if out.is_empty() {
            return Err(BorderError::NoFlags);
        }
        Ok(out)
    }
}

/// A normalized texture rectangle, `(u0, v0, u1, v1)` in `0..=1`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UvRect {
    /// Left edge.
    pub u0: f32,
    /// Top edge.
    pub v0: f32,
    /// Right edge.
    pub u1: f32,
    /// Bottom edge.
    pub v1: f32,
}

/// The **candidate** slice geometry of an edge texture: square slices of the
/// texture height across its width. The flag list is the *enabled* set, not a
/// divisor, and does not bound the slice count.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeAtlas {
    /// Texture width in texels.
    pub width: u32,
    /// Texture height in texels.
    pub height: u32,
    /// The named flags, in authored order.
    pub flags: Vec<CornerFlag>,
    /// The FDF's `BackdropCornerSize`, in frame units — the on-screen size of a
    /// corner. Kept as authored, not converted to pixels.
    pub corner_size: f32,
}

impl EdgeAtlas {
    /// Reads a **candidate** atlas from the texture's dimensions and the FDF's
    /// flags.
    ///
    /// The candidate slice size is the texture **height** — the one quantity the
    /// file itself supplies — so the width must divide by it. That is all the
    /// texture proves: a 256×32 file divides into eight 32×32 squares. It does
    /// **not** prove the native slot count, which the flag list does not bound,
    /// so the flags are kept as the *enabled* set rather than used as a divisor.
    ///
    /// # Errors
    ///
    /// [`BorderError::NoFlags`], [`BorderError::NoHeight`] or
    /// [`BorderError::NotDivisible`].
    pub fn new(
        flags: Vec<CornerFlag>,
        width: u32,
        height: u32,
        corner_size: f32,
    ) -> Result<Self, BorderError> {
        if flags.is_empty() {
            return Err(BorderError::NoFlags);
        }
        if height == 0 {
            return Err(BorderError::NoHeight);
        }
        if width % height != 0 {
            return Err(BorderError::NotDivisible { width, height });
        }
        Ok(Self {
            width,
            height,
            flags,
            corner_size,
        })
    }

    /// The **candidate** number of slices: how many square slices of the texture
    /// height fit across the width. Not the number of enabled flags, and not
    /// proven to be the native slot count.
    #[must_use]
    pub fn slices(&self) -> usize {
        (self.width / self.height) as usize
    }

    /// The slice size in texels. Square by construction.
    #[must_use]
    pub fn slice_px(&self) -> (u32, u32) {
        (self.height, self.height)
    }

    /// The normalized texture rectangle of slice `slot`, laid out left to right.
    ///
    /// This is pure geometry: it says where slice `slot` is, **not** which flag it
    /// draws. See [`EdgeAtlas::slot_for`].
    ///
    /// # Errors
    ///
    /// [`BorderError::NotDivisible`] when `slot` is past the end.
    pub fn slice_uv(&self, slot: usize) -> Result<UvRect, BorderError> {
        if slot >= self.slices() {
            return Err(BorderError::SlotOutOfRange {
                slot,
                slices: self.slices(),
            });
        }
        let n = self.slices() as f32;
        let i = slot as f32;
        Ok(UvRect {
            u0: i / n,
            v0: 0.0,
            u1: (i + 1.0) / n,
            v1: 1.0,
        })
    }

    /// Which slice draws a flag.
    ///
    /// **Always `None`.** The layout order is not established: the flags' written
    /// order is authored, but nothing proves the texture follows it, and no native
    /// observation of the edge renderer exists. A caller that needs this must pass
    /// a verified order in; returning `flags[i] -> slice i` would be a guess, and
    /// a wrong guess puts corners on edges.
    #[must_use]
    pub fn slot_for(&self, _flag: CornerFlag) -> Option<usize> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUTTON_FLAGS: &str = "UL|UR|BL|BR|T|L|B|R";

    /// The shipped shape: eight flags and a 256x32 texture divide into eight
    /// square 32x32 slices.
    #[test]
    fn the_shipped_edge_texture_divides_into_square_slices() {
        let flags = CornerFlag::parse_flags(BUTTON_FLAGS).unwrap();
        assert_eq!(flags.len(), 8);
        let a = EdgeAtlas::new(flags, 256, 32, 0.016).unwrap();
        assert_eq!(a.slices(), 8);
        assert_eq!(a.slice_px(), (32, 32), "the slice is the texture height");
        assert_eq!(a.corner_size, 0.016, "authored, not converted");
    }

    /// A texture that does not divide into square slices is refused; a flag list
    /// of a different length is **not** a reason to refuse, because the flag count
    /// does not bound the slot count.
    #[test]
    fn only_the_texture_dimensions_decide_the_candidate() {
        let eight = CornerFlag::parse_flags(BUTTON_FLAGS).unwrap();
        assert!(matches!(
            EdgeAtlas::new(eight, 256, 48, 0.016),
            Err(BorderError::NotDivisible { .. })
        ));
        // Four enabled flags on the same 256x32 texture is legal: the file still
        // carries eight candidate slots, only some of them enabled.
        let four = CornerFlag::parse_flags("UL|UR|BL|BR").unwrap();
        let a = EdgeAtlas::new(four, 256, 32, 0.016).unwrap();
        assert_eq!(
            a.slices(),
            8,
            "the slots come from the texture, not the flags"
        );
        assert_eq!(a.flags.len(), 4, "the flags are the enabled set");
    }

    /// Slices are laid out left to right and tile the whole texture.
    #[test]
    fn slices_tile_the_texture_left_to_right() {
        let a = EdgeAtlas::new(
            CornerFlag::parse_flags(BUTTON_FLAGS).unwrap(),
            256,
            32,
            0.016,
        )
        .unwrap();
        assert_eq!(
            a.slice_uv(0).unwrap(),
            UvRect {
                u0: 0.0,
                v0: 0.0,
                u1: 0.125,
                v1: 1.0
            }
        );
        assert_eq!(a.slice_uv(7).unwrap().u1, 1.0, "the last slice ends at 1");
        // Contiguous, and the span is the whole width.
        let mut cursor = 0.0f32;
        for slot in 0..a.slices() {
            let r = a.slice_uv(slot).unwrap();
            assert!((r.u0 - cursor).abs() < 1e-6, "gap before slice {slot}");
            cursor = r.u1;
        }
        assert!((cursor - 1.0).abs() < 1e-6);
        assert!(a.slice_uv(8).is_err(), "a slot past the end is refused");
    }

    /// The flag tokens are the eight the FDF defines, and anything else is
    /// refused rather than skipped.
    #[test]
    fn only_the_eight_flags_are_accepted() {
        assert_eq!(CornerFlag::parse("UL").unwrap(), CornerFlag::UpperLeft);
        assert_eq!(CornerFlag::parse("r").unwrap(), CornerFlag::Right);
        assert_eq!(
            CornerFlag::parse_flags("UL|UR|BL|BR").unwrap().len(),
            4,
            "a four-flag list is legal"
        );
        assert!(matches!(
            CornerFlag::parse("XX"),
            Err(BorderError::UnknownFlag(_))
        ));
        assert!(matches!(
            CornerFlag::parse_flags("UL|ZZ"),
            Err(BorderError::UnknownFlag(_))
        ));
        assert_eq!(CornerFlag::parse_flags(""), Err(BorderError::NoFlags));
    }

    /// The flag -> slot mapping is **not** answered, because the layout order is
    /// unproven. This test pins that it stays unanswered rather than silently
    /// becoming an identity.
    #[test]
    fn the_flag_to_slot_order_is_not_guessed() {
        let a = EdgeAtlas::new(
            CornerFlag::parse_flags(BUTTON_FLAGS).unwrap(),
            256,
            32,
            0.016,
        )
        .unwrap();
        for flag in a.flags.clone() {
            assert_eq!(
                a.slot_for(flag),
                None,
                "{flag:?} must not be assigned a slot"
            );
        }
    }

    /// A zero-height texture has no slice size to take.
    #[test]
    fn a_zero_height_texture_is_refused() {
        let flags = CornerFlag::parse_flags(BUTTON_FLAGS).unwrap();
        assert_eq!(
            EdgeAtlas::new(flags, 0, 0, 0.016),
            Err(BorderError::NoHeight)
        );
    }
}

//! BLP0/BLP1/BLP2 texture decoder.
//!
//! # What actually ships
//!
//! Vanilla 1.28c (6,273 `.blp` entries) and vanilla 1.29b (6,696) were
//! enumerated twice: once through the union of every archive's `(listfile)`, and
//! once by walking **every** block of every archive, so the result does not
//! depend on a name list existing. Both agree:
//!
//! | build | BLP0 | BLP1 content 0 | BLP1 content 1 | BLP2 | any other content |
//! |-------|------|----------------|----------------|------|-------------------|
//! | 1.28c | 0    | 4,924          | 1,349          | 0    | 0                 |
//! | 1.29b | 0    | 5,318          | 1,378          | 0    | 0                 |
//!
//! The block walk also reaches files no `(listfile)` entry names: 356 blocks on
//! 1.28c and 10 on 1.29b. Of those, 349 on 1.28c read without a key (114 are
//! superseded `BLP1` revisions the patch archives overwrote, 235 are other
//! formats); the remaining 17 are encrypted, so no install records the basename
//! their key is derived from. They are read anyway: `crypt::decrypt`'s
//! first-dword mask is `key + 0xEEEEEEEE + TABLE[key & 0xFF]` and the first
//! sector-offset word is always `(sectors + 1) * 4`, so the key falls out in 256
//! guesses. Every one of the 17 is a stale `local.txt` / `war3x.txt` /
//! `Detector.js` / `(attributes)` blob, not a texture. Between the two walks,
//! every existing block in every archive of both builds has been read.
//!
//! In that census, no shipped byte is BLP0, none is BLP2, and no
//! `content` byte outside `{0, 1}` exists. So [`Header::parse`] refuses BLP0
//! outright and [`decode_mip`] refuses BLP2 payloads: both layouts are described
//! by the public format notes but confirmed by no byte we are allowed to trust,
//! and this decoder refuses to guess at an encoding it cannot check. [`Census`]
//! reports that refusal rather than hiding it, and `blpx info <install_dir>`
//! prints the table above for any install.
//!
//! # Header layout (BLP1, verified against both shipped builds)
//!
//! ```text
//! BLP1                              BLP2
//! 0x00 magic                        0x00 magic
//! 0x04 content (0 JPEG, 1 direct)   0x04 content
//! 0x08 alpha bit depth              0x08 encoding, 0x09 alphaDepth, 0x0A alphaType, 0x0B hasMips
//! 0x0C width                        0x0C width
//! 0x10 height                       0x10 height
//! 0x14 picture type (2 / 3 / 4 / 5) 0x14 mip offsets [16]
//! 0x18 has mipmaps                  0x54 mip sizes   [16]
//! 0x1C mip offsets [16]             0x94 palette (variable)
//! 0x5C mip sizes   [16]
//! 0x9C palette (variable — runs to mip_offsets[0])
//! ```
//!
//! `0x04` and `0x08` are **not** the same field across the two magics. On BLP1
//! `0x08` is an alpha bit depth. The public format notes put BLP2's encoding
//! selector at `0x08` and its alpha depth at `0x09` — unmeasured, since no BLP2
//! ships, but cheap to keep apart: [`Header`] stores them as
//! [`Header::alpha_bits`] and [`Header::format`] rather than sharing one byte
//! between two meanings, which is how a BLP2 with encoding `3` and alpha depth
//! `1` would otherwise be classified as palettised.
//!
//! # Framing
//!
//! Mip `n` occupies `[offset[n], offset[n] + size[n])` and each mip halves both
//! dimensions. [`Framing`] turns that into assertions rather than assumptions;
//! they hold on all 12,969 shipped textures:
//!
//! 1. **`file_len == mip_offsets[0] + Σ mip_sizes[0..n)`** — the BLP analogue of
//!    `RIBB`'s `node_size == 96 + Σ tracks`. The palette gap plus every mip is
//!    the whole file, with nothing appended and nothing interleaved.
//! 2. **`mip_offsets[0] % 4 == 0` and `mip_offsets[0] >= palette_base`**, so the
//!    palette is a whole number of BGRA entries: `palette_entry_count ==
//!    (mip_offsets[0] - palette_base) / 4`. This is what makes a *short* palette
//!    (10 entries in `Editor-Toolbar-MapValidation.blp`, 1.29b only) legible
//!    instead of looking like a transparent tail.
//! 3. **`content == 1`: `size[n] == mip_size(n).0 * mip_size(n).1 * bpp`** with a
//!    single `bpp` for the whole chain, and `bpp ∈ {1, 2}`. One plane, or an
//!    index plane followed by a separate alpha plane. `picture_type` does *not*
//!    predict `bpp`: type 5 ships both (297 files at 1, 17 at 2 on 1.28c),
//!    which is why the payload size decides. All 102 odd-sized textures carry a
//!    single mip, so the halving rule is never contradicted for them.
//! 4. **`content == 0`: `[0xA0 + jpeg_header_size, mip_offsets[0])` is either
//!    zeros or `FF D9` followed by zeros**, and `0xA0` starts with the SOI. The
//!    two builds differ here, which is what makes it worth asserting: 1.28c
//!    ships a header truncated mid-`SOF0` (`FF C0` at `0x30E`, two bytes of it
//!    inside the declared length) with 396 zero bytes of padding, while 1.29b
//!    ships a *complete* header ending at the EOI with `FF D9` plus 448 bytes of
//!    padding. Ten thousand JPEG textures satisfy both forms.

use std::collections::BTreeMap;
use std::path::Path;

/// Number of palette entries a decoded palette always reports.
///
/// The file may carry fewer; see [`palette`].
pub const PALETTE_ENTRIES: usize = 256;
/// Maximum mip levels the header can address.
pub const MAX_MIPS: usize = 16;

/// Where the palette starts in a BLP1.
pub const PALETTE_BASE: usize = 0x9C;
/// Where the palette starts in a BLP2.
pub const PALETTE_BASE_BLP2: usize = 0x94;
/// Where the mip offset table starts in a BLP1.
pub const MIP_BASE: usize = 0x1C;
/// Where the mip offset table starts in a BLP2.
pub const MIP_BASE_BLP2: usize = 0x14;
/// The `u32` JPEG header length of a `content == 0` BLP1.
pub const JPEG_HEADER_SIZE_AT: usize = 0x9C;
/// The first byte of that JPEG header.
pub const JPEG_HEADER_AT: usize = 0xA0;

/// Bytes per pixel of a `content == 1` mip that carries no alpha plane.
pub const BPP_INDEX: u32 = 1;
/// Bytes per pixel of a `content == 1` mip that carries a separate alpha plane.
pub const BPP_INDEX_ALPHA: u32 = 2;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("bad magic {0:?}, expected BLP0/BLP1/BLP2")]
    BadMagic([u8; 4]),
    #[error(
        "BLP0 keeps its mips in sibling .b00-.b15 files, not in the header; no BLP0 ships in vanilla 1.28c or 1.29b, so that layout is unmeasured and refused"
    )]
    Blp0ExternalMips,
    #[error(
        "BLP2 encoding byte {format} at 0x08; no BLP2 ships in vanilla 1.28c or 1.29b, so the payload layout is unmeasured and refused"
    )]
    Blp2Unmeasured { format: u8 },
    #[error(
        "BLP1 content byte {content} is neither 0 (JPEG) nor 1 (direct); refused rather than guessed"
    )]
    UnknownContent { content: u32 },
    #[error(
        "content is 0 (JPEG), so this mip is scan data behind a shared header; use decode_jpeg"
    )]
    NotDirect,
    #[error("truncated: need {need} bytes at {what}, have {have}")]
    Truncated {
        what: &'static str,
        need: usize,
        have: usize,
    },
    #[error("mip {index} [{offset}, {end}) lies outside the {len}-byte file")]
    MipOutOfBounds {
        index: usize,
        offset: usize,
        end: usize,
        len: usize,
    },
    #[error("mip index {index} exceeds the {MAX_MIPS} header slots")]
    InvalidMipIndex { index: usize },
    #[error("mip {index} is {size} bytes, which is not a recognised encoding of {w}x{h}")]
    UnrecognisedMip {
        index: usize,
        size: usize,
        w: u32,
        h: u32,
    },
    #[error("unsupported: {0}")]
    Unsupported(&'static str),
}

impl Error {
    /// A short stable label, so a census can tally refusals by cause.
    ///
    /// The labels are the census's own vocabulary; they are not error codes and
    /// are not part of any wire format.
    pub fn kind(&self) -> &'static str {
        match self {
            Error::Io(_) => "io",
            Error::BadMagic(_) => "bad-magic",
            Error::Blp0ExternalMips => "blp0-external-mips",
            Error::Blp2Unmeasured { .. } => "blp2-unmeasured",
            Error::UnknownContent { .. } => "unknown-content",
            Error::NotDirect => "jpeg-content",
            Error::Truncated { .. } => "truncated",
            Error::MipOutOfBounds { .. } => "mip-out-of-bounds",
            Error::InvalidMipIndex { .. } => "invalid-mip-index",
            Error::UnrecognisedMip { .. } => "unrecognised-mip",
            Error::Unsupported(_) => "unsupported",
        }
    }
}

/// The header magic, as a value rather than four bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Variant {
    /// `BLP0`: mips in sibling `.b00`-`.b15` files. Refused; see module docs.
    Blp0,
    /// `BLP1`: the only variant either shipped build contains.
    Blp1,
    /// `BLP2`: encoding byte at `0x08`, palette at `0x94`. Refused; see module docs.
    Blp2,
}

impl Variant {
    /// Read a magic without parsing the rest of the header.
    ///
    /// BLP0 is refused, so there is no way to ask a parsed header which one it
    /// was; the census needs the magic either way, so this is the one reader
    /// that accepts all three.
    pub fn from_magic(magic: [u8; 4]) -> Result<Self, Error> {
        match &magic {
            b"BLP0" => Ok(Variant::Blp0),
            b"BLP1" => Ok(Variant::Blp1),
            b"BLP2" => Ok(Variant::Blp2),
            _ => Err(Error::BadMagic(magic)),
        }
    }

    /// Where this variant's mip offset table starts.
    pub fn mip_base(self) -> usize {
        match self {
            Variant::Blp2 => MIP_BASE_BLP2,
            _ => MIP_BASE,
        }
    }

    /// Where this variant's palette starts.
    pub fn palette_base(self) -> usize {
        match self {
            Variant::Blp2 => PALETTE_BASE_BLP2,
            _ => PALETTE_BASE,
        }
    }
}

/// Which pixel encoding a BLP uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    /// BLP1, `content == 0`: JPEG scan data behind a shared header.
    /// Decode with [`decode_jpeg`], not [`decode_mip`].
    Jpeg,
    /// BLP1, `content == 1`: palette indices, one byte per pixel.
    Paletted,
    /// BLP1, `content == 1`: palette index plus a separate 8-bit alpha plane,
    /// two bytes per pixel. Chosen by the payload size, never by the header.
    PalettedWithAlpha,
    /// BLP2 encoding byte 2, DXT1/3/5. Refused; no shipped byte confirms it.
    Dxt,
    /// BLP2 encoding byte 3, raw BGRA, four bytes per pixel. Refused; no
    /// shipped byte confirms it.
    Bgra,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub magic: [u8; 4],
    pub width: u32,
    pub height: u32,
    /// BLP1 `0x08`, the alpha bit depth. **Not** BLP2's encoding byte.
    pub alpha_bits: u32,
    pub picture_type: u32,
    pub has_mipmaps: bool,
    pub content: u32,
    pub mip_offsets: [u32; MAX_MIPS],
    pub mip_sizes: [u32; MAX_MIPS],
    /// BLP2 `0x08`, the encoding selector. Zero on BLP1, where `0x08` is
    /// [`Header::alpha_bits`] instead. Kept in its own field so the two
    /// meanings cannot be read through the same byte.
    pub format: u8,
}

fn u32_at(d: &[u8], o: usize, what: &'static str) -> Result<u32, Error> {
    d.get(o..o + 4)
        .map(|s| u32::from_le_bytes(s.try_into().unwrap()))
        .ok_or(Error::Truncated {
            what,
            need: o + 4,
            have: d.len(),
        })
}

impl Header {
    pub fn parse(d: &[u8]) -> Result<Self, Error> {
        let magic: [u8; 4] = d
            .get(0..4)
            .ok_or(Error::Truncated {
                what: "magic",
                need: 4,
                have: d.len(),
            })?
            .try_into()
            .unwrap();
        let variant = Variant::from_magic(magic)?;
        if variant == Variant::Blp0 {
            return Err(Error::Blp0ExternalMips);
        }

        let content = u32_at(d, 0x04, "content")?;
        let width = u32_at(d, 0x0C, "width")?;
        let height = u32_at(d, 0x10, "height")?;
        let mip_base = variant.mip_base();

        // 0x08 means different things per magic; 0x09 only exists on BLP2.
        let alpha_bits = match variant {
            Variant::Blp2 => u32::from(d.get(0x09).copied().unwrap_or(0)),
            _ => u32_at(d, 0x08, "alpha bits")?,
        };
        let format = match variant {
            Variant::Blp2 => d.get(0x08).copied().unwrap_or(0),
            _ => 0,
        };
        let picture_type = match variant {
            Variant::Blp2 => 0,
            _ => u32_at(d, 0x14, "picture type")?,
        };
        let has_mipmaps = match variant {
            Variant::Blp2 => d.get(0x0B).copied().unwrap_or(0) != 0,
            _ => u32_at(d, 0x18, "has mipmaps")? != 0,
        };

        let mut mip_offsets = [0u32; MAX_MIPS];
        let mut mip_sizes = [0u32; MAX_MIPS];
        for i in 0..MAX_MIPS {
            mip_offsets[i] = u32_at(d, mip_base + i * 4, "mip offsets")?;
            mip_sizes[i] = u32_at(d, mip_base + 64 + i * 4, "mip sizes")?;
        }

        Ok(Self {
            magic,
            width,
            height,
            alpha_bits,
            picture_type,
            has_mipmaps,
            content,
            mip_offsets,
            mip_sizes,
            format,
        })
    }

    /// Which of the three magics this header carries.
    ///
    /// Every header this crate parsed is `Blp1` or `Blp2`; BLP0 is refused and an
    /// unknown magic never reaches a `Header`. The `Blp1` fallback is therefore
    /// only reachable from a hand-built struct, where the BLP1 reading is the
    /// conservative one.
    pub fn variant(&self) -> Variant {
        Variant::from_magic(self.magic).unwrap_or(Variant::Blp1)
    }

    /// Where this header's palette starts.
    pub fn palette_base(&self) -> usize {
        self.variant().palette_base()
    }

    /// Number of populated mip levels.
    pub fn mip_count(&self) -> usize {
        self.mip_sizes.iter().filter(|s| **s > 0).count()
    }

    /// Dimensions of mip `index`.
    ///
    /// Halving is a right shift, matching every shipped texture that carries
    /// more than one mip. The 102 odd-sized textures all carry exactly one mip,
    /// so the odd case is never exercised past mip 0 and no rounding rule is
    /// asserted there.
    pub fn mip_size(&self, index: usize) -> (u32, u32) {
        let mut w = self.width;
        let mut h = self.height;
        // After 32 halvings both u32 dimensions stay at one.
        for _ in 0..index.min(u32::BITS as usize) {
            w = (w >> 1).max(1);
            h = (h >> 1).max(1);
        }
        (w, h)
    }

    /// The encoding the header declares, before any payload is consulted.
    ///
    /// For `content == 1` this is [`Encoding::Paletted`] whatever the alpha
    /// arrangement; only [`Header::mip_encoding`] can tell the one-plane and
    /// two-plane forms apart, because the header does not.
    pub fn encoding(&self) -> Encoding {
        match self.variant() {
            Variant::Blp1 => {
                if self.content == 0 {
                    Encoding::Jpeg
                } else {
                    Encoding::Paletted
                }
            }
            Variant::Blp2 => match self.format {
                1 => Encoding::Paletted,
                2 => Encoding::Dxt,
                _ => Encoding::Bgra,
            },
            // Unreachable: `parse` refuses BLP0. `Jpeg` is the one answer that
            // routes the caller to the path which refuses a BLP0 outright.
            Variant::Blp0 => Encoding::Jpeg,
        }
    }

    /// The encoding of one mip, which for direct content also resolves the
    /// alpha-plane question the header leaves open.
    pub fn mip_encoding(&self, d: &[u8], index: usize) -> Result<Encoding, Error> {
        match self.encoding() {
            // The BLP2 payload formats are refused, so their plane layout is
            // never asserted against bytes we do not have.
            e @ (Encoding::Dxt | Encoding::Bgra | Encoding::Jpeg) => Ok(e),
            _ => match bytes_per_pixel(d, self, index)? {
                Some(BPP_INDEX) => Ok(Encoding::Paletted),
                // `None` is an empty mip; a third value already failed above.
                _ => Ok(Encoding::PalettedWithAlpha),
            },
        }
    }
}

/// The BGRX palette, always returned as 256 entries.
///
/// The palette is **variable length in the file**: it occupies the space between
/// [`Header::palette_base`] and `mip_offsets[0]`, not a fixed 1024 bytes.
///
/// That was found by sweeping 1.29b rather than 1.28c. Every 1.28c texture
/// carries a full 256-entry palette, so a fixed-size assumption held there and
/// was never contradicted. `ReplaceableTextures\WorldEditUI\Editor-Toolbar-
/// MapValidation.blp` in the legacy build is 878 bytes with a 16x16 image and a
/// ten-entry palette, and its mip indices reach only 8 — the palette genuinely
/// ends there and the mip data starts immediately after. Reading a fixed 1024
/// bytes fails that file as "truncated".
///
/// Absent entries come back zero, so a short palette is indistinguishable from a
/// palette whose tail is transparent black. Use [`palette_entry_count`] to tell
/// the difference.
pub fn palette(d: &[u8], h: &Header) -> Result<[[u8; 4]; PALETTE_ENTRIES], Error> {
    let base = h.palette_base();
    let end = h.mip_offsets[0] as usize;
    if end < base {
        return Err(Error::Truncated {
            what: "palette ends before it starts",
            need: base,
            have: end,
        });
    }
    let avail = (end - base) / 4;
    let slice = d.get(base..base + avail * 4).ok_or(Error::Truncated {
        what: "palette",
        need: base + avail * 4,
        have: d.len(),
    })?;
    let mut p = [[0u8; 4]; PALETTE_ENTRIES];
    for (i, e) in p.iter_mut().enumerate().take(avail) {
        *e = slice[i * 4..i * 4 + 4].try_into().unwrap();
    }
    Ok(p)
}

/// Palette entries actually present in the file.
///
/// Zero means the file carries no palette at all, which is legal for a
/// JPEG-content BLP where the palette area holds the JPEG header instead.
pub fn palette_entry_count(h: &Header) -> usize {
    (h.mip_offsets[0] as usize).saturating_sub(h.palette_base()) / 4
}

/// Raw bytes of mip `index`; an unpopulated slot returns an empty slice.
pub fn mip_bytes<'a>(d: &'a [u8], h: &Header, index: usize) -> Result<&'a [u8], Error> {
    let (&off, &size) = h
        .mip_offsets
        .get(index)
        .zip(h.mip_sizes.get(index))
        .ok_or(Error::InvalidMipIndex { index })?;
    let off = off as usize;
    let size = size as usize;
    if size == 0 {
        return Ok(&[]);
    }
    if off == 0 {
        let (w, height) = h.mip_size(index);
        return Err(Error::UnrecognisedMip {
            index,
            size,
            w,
            h: height,
        });
    }
    let end = off.checked_add(size).ok_or(Error::MipOutOfBounds {
        index,
        offset: off,
        end: usize::MAX,
        len: d.len(),
    })?;
    d.get(off..end).ok_or(Error::MipOutOfBounds {
        index,
        offset: off,
        end,
        len: d.len(),
    })
}

/// Bytes per pixel of a `content == 1` mip.
///
/// Measured over every shipped `content == 1` texture: the plane count is a whole
/// file property, `1` (index only) or `2` (index plus a separate alpha plane),
/// and it never varies between the mips of one file. `picture_type` does not
/// predict it — picture type 5 ships both — so the payload size decides, and
/// `blpx info` reports the split per bucket.
///
/// JPEG mips carry compressed scan data and have no bytes-per-pixel, so they
/// return `None` rather than a misleading `1`.
pub fn bytes_per_pixel(d: &[u8], h: &Header, index: usize) -> Result<Option<u32>, Error> {
    if h.content != 1 {
        return Ok(None);
    }
    let raw = mip_bytes(d, h, index)?;
    let (w, hh) = h.mip_size(index);
    let px = (w as u64) * (hh as u64);
    if px == 0 {
        return Err(Error::UnrecognisedMip {
            index,
            size: raw.len(),
            w,
            h: hh,
        });
    }
    if raw.is_empty() {
        return Ok(None);
    }
    let n = raw.len() as u64;
    if !n.is_multiple_of(px) || n / px > u64::from(BPP_INDEX_ALPHA) {
        return Err(Error::UnrecognisedMip {
            index,
            size: raw.len(),
            w,
            h: hh,
        });
    }
    Ok(Some((n / px) as u32))
}

/// Decode mip `index` to straight RGBA8, row-major, top-left origin.
///
/// Only the direct/palettised content (`content == 1`) is decoded here. Those
/// mips are uncompressed and come in two measured shapes, both planar:
///
/// * `w * h` bytes — a palette-index plane, alpha fully opaque.
/// * `w * h * 2` bytes — the index plane followed by a separate alpha plane.
///
/// Measured over all 12,969 shipped textures, every `content == 1` file is
/// exactly one of those two shapes and uses one of them for every mip;
/// interleaving was disproved by the index plane of `SelectionCircleLarge.blp`
/// holding a single distinct value while the trailing plane held 83.
///
/// JPEG-content BLPs need [`decode_jpeg`].
#[cfg_attr(
    feature = "profile-cpu",
    tracing::instrument(skip_all, target = "pcx_cpu", name = "assets.blp_decode")
)]
pub fn decode_mip(d: &[u8], h: &Header, index: usize) -> Result<Vec<u8>, Error> {
    match Variant::from_magic(h.magic)? {
        Variant::Blp0 => return Err(Error::Blp0ExternalMips),
        Variant::Blp2 => return Err(Error::Blp2Unmeasured { format: h.format }),
        Variant::Blp1 => {}
    }
    match h.content {
        0 => return Err(Error::NotDirect),
        1 => {}
        content => return Err(Error::UnknownContent { content }),
    }
    let raw = mip_bytes(d, h, index)?;
    let (w, hh) = h.mip_size(index);
    let bpp = bytes_per_pixel(d, h, index)?.ok_or(Error::UnrecognisedMip {
        index,
        size: raw.len(),
        w,
        h: hh,
    })?;
    let px = raw.len() / bpp as usize;
    let rgba_bytes = px
        .checked_mul(4)
        .ok_or(Error::Unsupported("direct RGBA size overflow"))?;
    let pal = palette(d, h)?;
    let mut out = Vec::new();
    out.try_reserve_exact(rgba_bytes)
        .map_err(|_| Error::Unsupported("direct RGBA allocation"))?;
    out.resize(rgba_bytes, 0);
    let (indices, alpha) = raw.split_at(px);
    for (i, &index) in indices.iter().enumerate() {
        let [b, g, r, _] = pal[index as usize];
        out[i * 4..i * 4 + 4].copy_from_slice(&[r, g, b, alpha.get(i).copied().unwrap_or(255)]);
    }
    Ok(out)
}

/// The shared JPEG header that precedes every mip in a JPEG-content BLP1.
///
/// Mip payloads carry only the scan data; the SOI/JFIF and quantisation and
/// Huffman tables live once at [`JPEG_HEADER_AT`], so a mip is only a valid JPEG
/// stream once this prefix is prepended. Its declared length stops short of
/// `mip_offsets[0]`; see [`Framing::jpeg_gap_is_padding`] for what fills the
/// difference in each build.
pub fn jpeg_header<'a>(d: &'a [u8], h: &Header) -> Result<&'a [u8], Error> {
    if h.content != 0 {
        return Err(Error::Unsupported("not a JPEG-content BLP"));
    }
    let size = u32_at(d, JPEG_HEADER_SIZE_AT, "jpeg header size")? as usize;
    let end = JPEG_HEADER_AT
        .checked_add(size)
        .ok_or(Error::Unsupported("jpeg header size overflow"))?;
    d.get(JPEG_HEADER_AT..end).ok_or(Error::Truncated {
        what: "jpeg header",
        need: end,
        have: d.len(),
    })
}

/// Decode a JPEG-content mip by prepending [`jpeg_header`] to the mip bytes.
///
/// `jpeg-decoder` picks its output layout from the stream, so the format is
/// read back from the decoder rather than assumed: every shipped texture
/// decodes to a 4-channel layout, which is why this cannot just ask for RGB24.
pub fn decode_jpeg(d: &[u8], h: &Header, index: usize) -> Result<Vec<u8>, Error> {
    decode_jpeg_with_limit(d, h, index, usize::MAX)
}

/// Decode with a caller budget checked against both BLP and JPEG dimensions before pixels.
#[cfg_attr(
    feature = "profile-cpu",
    tracing::instrument(skip_all, target = "pcx_cpu", name = "assets.jpeg_decode")
)]
pub fn decode_jpeg_with_limit(
    d: &[u8],
    h: &Header,
    index: usize,
    max_rgba_bytes: usize,
) -> Result<Vec<u8>, Error> {
    if h.content != 0 {
        return Err(match h.content {
            1 => Error::NotDirect,
            content => Error::UnknownContent { content },
        });
    }
    let body = mip_bytes(d, h, index)?;
    let (w, hh) = h.mip_size(index);
    if body.is_empty() {
        return Err(Error::UnrecognisedMip {
            index,
            size: 0,
            w,
            h: hh,
        });
    }
    let px = usize::try_from(w)
        .ok()
        .and_then(|w| usize::try_from(hh).ok()?.checked_mul(w))
        .ok_or(Error::Unsupported("jpeg dimensions overflow"))?;
    let rgba_bytes = px
        .checked_mul(4)
        .filter(|n| *n <= max_rgba_bytes)
        .ok_or(Error::Unsupported("jpeg RGBA budget"))?;
    let head = jpeg_header(d, h)?;

    // The shared header then the mip scan data, read in place rather than joined.
    let mut decoder = jpeg_decoder::Decoder::new(std::io::Read::chain(head, body));
    decoder.set_max_decoding_buffer_size(max_rgba_bytes);
    decoder
        .read_info()
        .map_err(|_| Error::Unsupported("unsupported jpeg metadata"))?;
    let info = decoder.info().ok_or(Error::Unsupported("jpeg frame"))?;
    if usize::from(info.width) != w as usize || usize::from(info.height) != hh as usize {
        return Err(Error::UnrecognisedMip {
            index,
            size: usize::from(info.width) * usize::from(info.height),
            w,
            h: hh,
        });
    }
    let rgb = decoder.decode().map_err(|e| {
        Error::Unsupported(match e {
            jpeg_decoder::Error::Io(_) => "jpeg io",
            _ => "unsupported jpeg data",
        })
    })?;
    let format = info.pixel_format;

    let channels = format.pixel_bytes();
    if rgb.len() != px * channels {
        return Err(Error::UnrecognisedMip {
            index,
            size: rgb.len(),
            w,
            h: hh,
        });
    }

    let mut out = Vec::new();
    out.try_reserve_exact(rgba_bytes)
        .map_err(|_| Error::Unsupported("jpeg RGBA allocation"))?;
    out.resize(rgba_bytes, 0);
    // One pass per format, so the format is not matched again for every pixel.
    use jpeg_decoder::PixelFormat;
    match format {
        PixelFormat::L8 => fill_rgba::<1>(&mut out, &rgb, |p| rgba_from_jpeg(PixelFormat::L8, p)),
        PixelFormat::L16 => {
            fill_rgba::<2>(&mut out, &rgb, |p| rgba_from_jpeg(PixelFormat::L16, p));
        }
        PixelFormat::RGB24 => {
            fill_rgba::<3>(&mut out, &rgb, |p| rgba_from_jpeg(PixelFormat::RGB24, p));
        }
        PixelFormat::CMYK32 => {
            fill_rgba::<4>(&mut out, &rgb, |p| rgba_from_jpeg(PixelFormat::CMYK32, p));
        }
    }
    Ok(out)
}

fn fill_rgba<const N: usize>(out: &mut [u8], source: &[u8], convert: impl Fn(&[u8; N]) -> [u8; 4]) {
    for (pixel, components) in out
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(source.as_chunks::<N>().0)
    {
        pixel.copy_from_slice(&convert(components));
    }
}

/// One JPEG pixel's components as straight RGBA8.
///
/// BLP JPEG color components use BGR order. A four-component stream carries
/// inverted BGRA whatever the header's `alpha_bits` says; materials decide
/// whether alpha is used. With `alpha_bits 0`, the original client blends
/// `HumanSymbol.blp` (filter mode 2) with its fourth component as alpha on the
/// Lordaeron loading screen. `Textures\Base.blp` stores `255` there at every
/// mip below 0, which an opaque material ignores, so the fourth component is
/// not black ink either.
/// Grayscale has no channel order to change. This is a format conversion, not
/// an artwork-specific tint or correction in the renderer.
fn rgba_from_jpeg(format: jpeg_decoder::PixelFormat, p: &[u8]) -> [u8; 4] {
    match format {
        jpeg_decoder::PixelFormat::L8 => [p[0], p[0], p[0], 255],
        jpeg_decoder::PixelFormat::L16 => {
            let v = (u16::from(p[0]) << 8 | u16::from(p[1])) as u32;
            let s = (v >> 8) as u8;
            [s, s, s, 255]
        }
        jpeg_decoder::PixelFormat::RGB24 => [p[2], p[1], p[0], 255],
        jpeg_decoder::PixelFormat::CMYK32 => [255 - p[2], 255 - p[1], 255 - p[0], 255 - p[3]],
    }
}

/// Convenience: decode mip 0.
pub fn decode(d: &[u8]) -> Result<(Header, Vec<u8>), Error> {
    let h = Header::parse(d)?;
    let px = if h.content == 0 {
        decode_jpeg(d, &h, 0)?
    } else {
        decode_mip(d, &h, 0)?
    };
    Ok((h, px))
}

pub fn load(path: impl AsRef<Path>) -> Result<(Header, Vec<u8>), Error> {
    decode(&std::fs::read(path)?)
}

/// The measured framing of one texture, as facts rather than assertions.
///
/// Every field is read straight out of the bytes, so [`Framing::violations`] can
/// name the relation that failed instead of a bare `false`. The relations and
/// the files they hold on are listed in the module documentation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Framing {
    pub file_len: usize,
    pub mip_count: usize,
    pub palette_base: usize,
    /// `mip_offsets[0]`: where the header says mip data starts.
    pub payload_start: usize,
    pub palette_entries: usize,
    /// `Σ mip_sizes[0..mip_count)`.
    pub mip_bytes: usize,
    /// Mip-0 bytes per pixel, for `content == 1` files.
    pub bytes_per_pixel: Option<u32>,
    /// The declared JPEG header length, for `content == 0` files.
    pub jpeg_header_len: Option<usize>,
    /// Bytes between the end of the declared JPEG header and `payload_start`.
    pub jpeg_gap_len: Option<usize>,
}

impl Framing {
    /// Read the framing of `d` as described by `h`.
    pub fn of(d: &[u8], h: &Header) -> Self {
        let n = h.mip_count();
        let payload_start = h.mip_offsets[0] as usize;
        let palette_base = h.palette_base();
        let jpeg = (h.content == 0)
            .then(|| u32_at(d, JPEG_HEADER_SIZE_AT, "jpeg header size").unwrap_or(0) as usize);
        Self {
            file_len: d.len(),
            mip_count: n,
            palette_base,
            payload_start,
            palette_entries: payload_start.saturating_sub(palette_base) / 4,
            mip_bytes: h.mip_sizes[..n].iter().map(|s| *s as usize).sum(),
            bytes_per_pixel: bytes_per_pixel(d, h, 0).unwrap_or(None),
            jpeg_header_len: jpeg,
            jpeg_gap_len: jpeg.map(|len| payload_start.saturating_sub(JPEG_HEADER_AT + len)),
        }
    }

    /// `file_len == payload_start + mip_bytes`: palette gap plus every mip is
    /// the whole file.
    pub fn sums(&self) -> bool {
        self.file_len == self.payload_start + self.mip_bytes
    }

    /// The palette is a whole number of 4-byte entries and does not run past
    /// the payload.
    pub fn palette_is_whole_entries(&self) -> bool {
        self.payload_start >= self.palette_base && self.payload_start.is_multiple_of(4)
    }

    /// Every mip is exactly `width x height x bpp` for one `bpp` across the
    /// whole chain, and `bpp` is 1 or 2.
    ///
    /// Only meaningful for `content == 1`; JPEG mips are compressed.
    pub fn mips_are_planar(&self, d: &[u8], h: &Header) -> bool {
        if h.content != 1 {
            return true;
        }
        let mut bpp = None;
        for i in 0..self.mip_count {
            match bytes_per_pixel(d, h, i) {
                Ok(Some(b)) if b <= BPP_INDEX_ALPHA => match bpp {
                    None => bpp = Some(b),
                    // A file that mixes planes across its chain is not a
                    // layout this decoder has measured.
                    Some(seen) if seen == b => {}
                    Some(_) => return false,
                },
                _ => return false,
            }
        }
        bpp.is_some()
    }

    /// The bytes between the declared JPEG header and the payload are an
    /// optional EOI marker and zeros — never image data.
    pub fn jpeg_gap_is_padding(&self, d: &[u8]) -> bool {
        let (Some(len), Some(gap)) = (self.jpeg_header_len, self.jpeg_gap_len) else {
            return true;
        };
        let start = JPEG_HEADER_AT + len;
        let Some(tail) = d.get(start..start + gap) else {
            return false;
        };
        // 1.28c ships zeros here; 1.29b ships the header's own `FF D9` EOI
        // followed by zeros. Anything else means mip data leaked into the gap
        // and the header length or the mip base is wrong.
        let has_eoi = match tail.first() {
            Some(0xFF) => tail.get(1) == Some(&0xD9),
            _ => true,
        };
        has_eoi && tail.iter().skip(2).all(|b| *b == 0x00)
    }

    /// Every framing relation this decoder relies on, as human-readable
    /// failures. Empty means the layout adds up.
    pub fn violations(&self, d: &[u8], h: &Header) -> Vec<String> {
        let mut out = Vec::new();
        if !self.sums() {
            out.push(format!(
                "payload_start {} + mip bytes {} != file length {}",
                self.payload_start, self.mip_bytes, self.file_len
            ));
        }
        if !self.palette_is_whole_entries() {
            out.push(format!(
                "payload_start {} is not 4-aligned at or past palette base {}",
                self.payload_start, self.palette_base
            ));
        }
        if !self.mips_are_planar(d, h) {
            out.push(format!(
                "mips are not width x height x {{1,2}} with one plane count across {} mips",
                self.mip_count
            ));
        }
        if !self.jpeg_gap_is_padding(d) {
            out.push(format!(
                "bytes [{}, payload_start={}) after the {}-byte JPEG header are not padding",
                JPEG_HEADER_AT + self.jpeg_header_len.unwrap_or(0),
                self.payload_start,
                self.jpeg_header_len.unwrap_or(0)
            ));
        }
        out
    }
}

/// One `(magic, content)` row of a texture census.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bucket {
    pub magic: [u8; 4],
    pub content: u32,
    /// Files seen in this bucket.
    pub files: usize,
    /// Files whose header names an encoding this crate decodes.
    ///
    /// A classification, not a decode: [`decode`] succeeding for every file of
    /// a bucket is what the ignored full sweep proves.
    pub handled: usize,
    /// Files this crate refuses, tallied by [`Error::kind`].
    pub refused: BTreeMap<&'static str, usize>,
    /// `content == 1` only: files per mip-0 bytes-per-pixel value.
    pub bytes_per_pixel: BTreeMap<u32, usize>,
}

/// A streaming `(magic, content)` census of a texture set.
///
/// [`Census::record`] takes one file at a time, so a whole install can be
/// tallied without holding it in memory.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Census {
    buckets: BTreeMap<([u8; 4], u32), Bucket>,
    total: usize,
}

impl Census {
    pub fn new() -> Self {
        Self::default()
    }

    /// File size in bytes.
    pub fn len(&self) -> usize {
        self.total
    }

    pub fn is_empty(&self) -> bool {
        self.total == 0
    }

    /// Rows, ordered by magic then `content`.
    pub fn buckets(&self) -> Vec<&Bucket> {
        self.buckets.values().collect()
    }

    /// Files in rows this crate handles.
    pub fn handled(&self) -> usize {
        self.buckets.values().map(|b| b.handled).sum()
    }

    /// Files this crate refuses, across every cause.
    pub fn refused(&self) -> usize {
        self.buckets.values().flat_map(|b| b.refused.values()).sum()
    }

    /// Tally one file.
    pub fn record(&mut self, d: &[u8]) {
        self.total += 1;
        // Read the magic and content bytes directly: BLP0 is refused by
        // `Header::parse`, and a census still has to be able to say so.
        let magic = d
            .get(0..4)
            .and_then(|m| m.try_into().ok())
            .unwrap_or([0; 4]);
        let content = u32_at(d, 0x04, "content").unwrap_or(u32::MAX);
        let bucket = self
            .buckets
            .entry((magic, content))
            .or_insert_with(|| Bucket {
                magic,
                content,
                ..Default::default()
            });
        bucket.files += 1;

        // A file is handled only if the decoder will turn it into pixels. The
        // refusals are the same errors `decode` would raise, so the census can
        // never report a row as handled that the decode path would reject.
        let verdict = match Header::parse(d) {
            Ok(h) => match h.variant() {
                Variant::Blp1 if h.content <= 1 => {
                    bucket.handled += 1;
                    // JPEG mips have no bytes-per-pixel, so they leave the
                    // sub-split empty rather than inventing a value.
                    if let Some(bpp) = bytes_per_pixel(d, &h, 0).unwrap_or(None) {
                        *bucket.bytes_per_pixel.entry(bpp).or_default() += 1;
                    }
                    None
                }
                Variant::Blp1 => Some(Error::UnknownContent { content }),
                Variant::Blp2 => Some(Error::Blp2Unmeasured { format: h.format }),
                Variant::Blp0 => Some(Error::Blp0ExternalMips),
            },
            Err(e) => Some(e),
        };
        if let Some(e) = verdict {
            *bucket.refused.entry(e.kind()).or_default() += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Where to find a WC3 install.
    ///
    /// `PEACECRAFT3_INSTALL` overrides it so the same sweeps can be pointed
    /// at any build; otherwise the 1.28c tree this project was extracted
    /// from. `PEACECRAFT3_LEGACY_DIR` is accepted as an alias because that is
    /// where the 1.29b Battle.net install lives by default.
    fn install_dir() -> Option<std::path::PathBuf> {
        for key in ["PEACECRAFT3_INSTALL", "PEACECRAFT3_LEGACY_DIR"] {
            if let Ok(v) = std::env::var(key) {
                let p = std::path::PathBuf::from(v);
                if p.is_dir() {
                    return Some(p);
                }
            }
        }
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("wc3/game/app");
        p.is_dir().then_some(p)
    }

    /// Extracts one file out of the shipped archives, so the tests run against
    /// real bytes rather than fixtures we control.
    ///
    /// Searches every archive, not just `war3.mpq`: most textures do not live
    /// there, which is the coverage bug `file_list_unions_every_archive` pins.
    fn shipped(rel: &str) -> Option<Vec<u8>> {
        let dir = install_dir()?;
        pcx_mpq::MpqSet::open_wc3(dir).ok()?.extract(rel).ok()
    }

    const PALETTED: &[&str] = &[
        "ReplaceableTextures\\Selection\\SelectionCircleLarge.blp",
        "ReplaceableTextures\\Selection\\SelectionCircleSmall.blp",
        "ReplaceableTextures\\Selection\\SelectionCircleMed.blp",
    ];

    const JPEG: &[&str] = &[
        "UI\\Glues\\Loading\\Backgrounds\\Campaigns\\AshenvaleExpansion-TopLeft.blp",
        "ReplaceableTextures\\WorldEditUI\\WarcraftIIIFTLogo.blp",
    ];

    /// Hand-read from both builds, byte for byte. The two builds ship these
    /// files identically, which is what lets one expectation cover both.
    const LARGE_SELECTION: &str = "ReplaceableTextures\\Selection\\SelectionCircleLarge.blp";
    const SHADOW: &str = "ReplaceableTextures\\Shadows\\ShadowAltarOfDepths.blp";
    /// 1.29b only: the ten-entry palette the variable-length reading exists for.
    const SHORT_PALETTE: &str =
        "ReplaceableTextures\\WorldEditUI\\Editor-Toolbar-MapValidation.blp";

    /// Picks the decoder the file's own header asks for.
    fn decode_any(d: &[u8], h: &Header, mip: usize) -> Result<Vec<u8>, Error> {
        if h.content == 0 {
            decode_jpeg(d, h, mip)
        } else {
            decode_mip(d, h, mip)
        }
    }

    #[test]
    fn named_palettised_headers_match_hand_read_bytes() {
        let Some(_) = install_dir() else {
            eprintln!("skipping: no extracted install");
            return;
        };
        let Some(d) = shipped(LARGE_SELECTION) else {
            return;
        };
        assert_eq!(d.len(), 44_870, "file length");
        let h = Header::parse(&d).unwrap();
        assert_eq!(&h.magic, b"BLP1");
        assert_eq!(h.content, 1, "direct content");
        assert_eq!(h.alpha_bits, 8);
        assert_eq!(h.picture_type, 4);
        assert!(h.has_mipmaps);
        assert_eq!((h.width, h.height), (128, 128));
        assert_eq!(h.mip_count(), 8);
        assert_eq!(
            &h.mip_offsets[..8],
            &[1180, 33948, 42140, 44188, 44700, 44828, 44860, 44868]
        );
        assert_eq!(&h.mip_sizes[..8], &[32768, 8192, 2048, 512, 128, 32, 8, 2]);
        assert_eq!(h.mip_offsets[8], 0, "mip 8 is empty");
        assert_eq!(palette_entry_count(&h), 256);
        assert_eq!(bytes_per_pixel(&d, &h, 0).unwrap(), Some(BPP_INDEX_ALPHA));
        // 128*128*2, and the last mip is 1x1 with the alpha plane still present.
        assert_eq!(h.mip_sizes[7], 2);
        assert_eq!(h.mip_encoding(&d, 7).unwrap(), Encoding::PalettedWithAlpha);
        assert_eq!(h.mip_encoding(&d, 0).unwrap(), Encoding::PalettedWithAlpha);

        let pal = palette(&d, &h).unwrap();
        assert_eq!(
            pal[0],
            [0xFF, 0xFF, 0xFF, 0x00],
            "palette entry 0 is BGRA white"
        );
        assert_eq!(pal[255], [0, 0, 0, 0]);
    }

    #[test]
    fn named_single_mip_header_matches_hand_read_bytes() {
        let Some(_) = install_dir() else { return };
        let Some(d) = shipped(SHADOW) else { return };
        assert_eq!(d.len(), 1828, "file length");
        let h = Header::parse(&d).unwrap();
        assert_eq!(h.content, 1);
        assert_eq!(h.picture_type, 3);
        assert!(!h.has_mipmaps, "has_mipmaps is 0 here");
        assert_eq!((h.width, h.height), (18, 18));
        assert_eq!(h.mip_count(), 1);
        assert_eq!(h.mip_offsets[0], 1180);
        assert_eq!(h.mip_sizes[0], 648, "18*18*2");
        assert_eq!(h.mip_encoding(&d, 0).unwrap(), Encoding::PalettedWithAlpha);
    }

    #[test]
    fn a_short_palette_ends_exactly_where_the_mips_begin() {
        let Some(_) = install_dir() else { return };
        let Some(d) = shipped(SHORT_PALETTE) else {
            eprintln!("skipping: {SHORT_PALETTE} is 1.29b only");
            return;
        };
        assert_eq!(d.len(), 878, "file length");
        let h = Header::parse(&d).unwrap();
        assert_eq!(h.content, 1);
        assert_eq!((h.width, h.height), (16, 16));
        assert_eq!(h.mip_count(), 5);
        assert_eq!(h.mip_offsets[0], 196);
        // 196 == 0x9C + 10 * 4: ten BGRA entries, then the pixels, immediately.
        assert_eq!(h.palette_base(), 0x9C);
        assert_eq!(palette_entry_count(&h), 10);
        assert_eq!(h.mip_offsets[0] as usize - h.palette_base(), 10 * 4);
        assert_eq!(h.mip_sizes[0], 512, "16*16*2");
        let f = Framing::of(&d, &h);
        assert_eq!(f.palette_entries, 10);
        assert!(
            f.violations(&d, &h).is_empty(),
            "{:?}",
            f.violations(&d, &h)
        );
    }

    #[test]
    fn a_named_jpeg_header_starts_at_0xa0_and_ends_before_the_payload() {
        let Some(_) = install_dir() else { return };
        let Some(d) = shipped(JPEG[1]) else { return };
        let h = Header::parse(&d).unwrap();
        assert_eq!(h.content, 0);
        assert_eq!((h.width, h.height), (512, 256));
        assert_eq!(h.mip_count(), 10);
        assert_eq!(h.mip_offsets[0], 1180);
        assert_eq!(h.encoding(), Encoding::Jpeg);
        // The palette area holds the JPEG header here, not a palette.
        assert_eq!(palette_entry_count(&h), (1180 - 0x9C) / 4);
        assert_eq!(d[JPEG_HEADER_AT], 0xFF, "SOI");
        assert_eq!(d[JPEG_HEADER_AT + 1], 0xD8, "SOI");
        assert_eq!(bytes_per_pixel(&d, &h, 0).unwrap(), None, "JPEG has no bpp");
        let head = jpeg_header(&d, &h).unwrap();
        assert_eq!(head[0], 0xFF);
        assert_eq!(head[1], 0xD8);
        // The declared length is shorter than the space before the payload; the
        // difference is padding, and that is asserted rather than assumed.
        let f = Framing::of(&d, &h);
        assert!(
            f.jpeg_gap_len.unwrap() > 0,
            "the gap must exist to be tested"
        );
        assert!(f.jpeg_gap_is_padding(&d));
        assert!(
            f.violations(&d, &h).is_empty(),
            "{:?}",
            f.violations(&d, &h)
        );
    }

    /// The whole framing relation, on real assets.
    ///
    /// Relation 1 is `file_len == payload_start + Σ mip_sizes`; relation 2 is
    /// that the palette span is a whole number of BGRA entries; relation 3 is
    /// that each mip is exactly `width x height x bpp` with one `bpp` per file.
    /// A header read at the wrong base, a mip count that includes an empty
    /// entry, or an off-by-one palette base all break at least one.
    #[test]
    fn framing_identity_holds_on_named_textures() {
        let Some(_) = install_dir() else {
            eprintln!("skipping: no extracted install");
            return;
        };
        let mut seen = 0;
        for rel in PALETTED
            .iter()
            .chain(JPEG)
            .chain([SHADOW, SHORT_PALETTE].iter())
        {
            let Some(d) = shipped(rel) else { continue };
            let h = Header::parse(&d).unwrap_or_else(|e| panic!("{rel}: {e}"));
            let f = Framing::of(&d, &h);
            assert!(f.mip_count >= 1, "{rel}: no mips");
            assert!(
                f.file_len == f.payload_start + f.mip_bytes,
                "{rel}: {} + {} != {}",
                f.payload_start,
                f.mip_bytes,
                f.file_len
            );
            assert_eq!(
                f.payload_start % 4,
                0,
                "{rel}: payload starts mid-entry at {}",
                f.payload_start
            );
            assert!(
                f.payload_start >= f.palette_base,
                "{rel}: payload inside palette"
            );
            assert_eq!(
                f.palette_entries,
                (f.payload_start - f.palette_base) / 4,
                "{rel}: palette span"
            );
            assert!(
                f.mips_are_planar(&d, &h),
                "{rel}: a mip is not width x height x 1 or 2"
            );
            assert!(
                f.jpeg_gap_is_padding(&d),
                "{rel}: JPEG gap holds image data"
            );
            let v = f.violations(&d, &h);
            assert!(v.is_empty(), "{rel}: {v:?}");
            seen += 1;
        }
        assert!(seen >= 4, "only {seen} samples were readable");
    }

    #[test]
    fn shipped_textures_have_a_coherent_mip_chain() {
        let Some(_) = install_dir() else {
            eprintln!("skipping: no extracted install");
            return;
        };
        let mut seen = 0;
        for rel in PALETTED.iter().chain(JPEG) {
            let Some(d) = shipped(rel) else { continue };
            let h = Header::parse(&d).unwrap_or_else(|e| panic!("{rel}: {e}"));
            assert_eq!(&h.magic, b"BLP1", "{rel}");
            assert!(h.width > 0 && h.height > 0, "{rel}: bad dims");
            assert!(h.has_mipmaps, "{rel}");

            let n = h.mip_count();
            assert!((1..=MAX_MIPS).contains(&n), "{rel}: {n} mips");
            for i in 0..n {
                let off = h.mip_offsets[i] as usize;
                let end = off + h.mip_sizes[i] as usize;
                assert!(end <= d.len(), "{rel}: mip {i} past EOF");
                if i + 1 < n {
                    assert_eq!(
                        end,
                        h.mip_offsets[i + 1] as usize,
                        "{rel}: mip {i} does not abut mip {}",
                        i + 1
                    );
                } else {
                    assert_eq!(end, d.len(), "{rel}: last mip does not end at EOF");
                }
                // Direct mips are uncompressed, so their byte count must be
                // exactly one or two bytes per pixel. JPEG mips are scan data
                // and carry no such relation.
                if h.content == 1 {
                    let (w, hh) = h.mip_size(i);
                    let size = h.mip_sizes[i] as usize;
                    assert!(
                        size == (w * hh) as usize || size == (w * hh * 2) as usize,
                        "{rel}: mip {i} is {size} bytes, not 1 or 2 bytes per pixel for {w}x{hh}"
                    );
                }
            }
            seen += 1;
        }
        assert!(seen > 0, "no sample texture could be read from war3.mpq");
    }

    #[test]
    fn decodes_to_exactly_one_rgba_pixel_per_image_pixel() {
        let Some(_) = install_dir() else { return };
        let mut checked = 0;
        for rel in PALETTED.iter().chain(JPEG) {
            let Some(d) = shipped(rel) else { continue };
            let h = Header::parse(&d).unwrap_or_else(|e| panic!("{rel}: {e}"));
            let px = decode_any(&d, &h, 0).unwrap_or_else(|e| panic!("{rel}: {e}"));
            assert_eq!(
                px.len(),
                (h.width * h.height * 4) as usize,
                "{rel}: expected w*h RGBA8"
            );
            assert!(px.chunks(4).any(|p| p[3] != 0), "{rel}: fully transparent");
            checked += 1;
        }
        assert!(checked >= 4, "only {checked} samples were readable");
    }

    /// Every mip of a direct texture must decode, not just mip 0, and each must
    /// come out at its own halved dimensions.
    #[test]
    fn every_mip_of_a_direct_texture_decodes_at_its_own_size() {
        let Some(_) = install_dir() else { return };
        let Some(d) = shipped(LARGE_SELECTION) else {
            return;
        };
        let h = Header::parse(&d).unwrap();
        for i in 0..h.mip_count() {
            let (w, hh) = h.mip_size(i);
            let px = decode_mip(&d, &h, i).unwrap_or_else(|e| panic!("mip {i}: {e}"));
            assert_eq!(px.len(), (w * hh * 4) as usize, "mip {i} is {w}x{hh}");
        }
    }

    /// The alpha plane must be planar, not interleaved. `SelectionCircleLarge`
    /// is the proof: one distinct index value, 83 distinct alpha values.
    #[test]
    fn paletted_alpha_is_a_separate_plane() {
        let Some(_) = install_dir() else { return };
        let Some(d) = shipped(PALETTED[0]) else {
            return;
        };
        let h = Header::parse(&d).unwrap();
        assert_eq!(h.content, 1, "sample must be direct content");
        let raw = mip_bytes(&d, &h, 0).unwrap();
        let px = (h.width * h.height) as usize;
        assert_eq!(raw.len(), px * 2, "sample must carry an alpha plane");

        let distinct = |s: &[u8]| -> std::collections::BTreeSet<u8> { s.iter().copied().collect() };
        let (indices, alpha) = raw.split_at(px);
        assert!(
            distinct(indices).len() < distinct(alpha).len(),
            "index plane ({} distinct) should be flatter than alpha ({} distinct)",
            distinct(indices).len(),
            distinct(alpha).len()
        );
    }

    #[test]
    fn rejects_a_non_blp() {
        assert!(matches!(Header::parse(&[0u8; 64]), Err(Error::BadMagic(_))));
    }

    /// BLP0 is refused outright. Its mips live in sibling `.b00`-`.b15` files,
    /// so the offsets a BLP1 header would read are not pixel offsets at all, and
    /// no vanilla build ships one to check the layout against.
    #[test]
    fn refuses_blp0_rather_than_reading_a_blp1_header() {
        let mut d = vec![0u8; 0x9C + 64];
        d[..4].copy_from_slice(b"BLP0");
        d[0x04..0x08].copy_from_slice(&1u32.to_le_bytes());
        // Offsets placed where a BLP1 header would read them; the refusal must
        // not depend on them.
        d[0x1C..0x20].copy_from_slice(&1180u32.to_le_bytes());
        let e = Header::parse(&d).unwrap_err();
        assert_eq!(e.kind(), "blp0-external-mips");
        assert!(matches!(e, Error::Blp0ExternalMips));
    }

    /// BLP2's encoding selector is at `0x08`, not its alpha depth at `0x09`.
    ///
    /// These synthetic headers exist only to pin the *classification* and the
    /// refusal; no shipped byte is BLP2, so nothing here claims to decode one.
    #[test]
    fn blp2_encoding_is_read_from_0x08_and_its_payloads_are_refused() {
        for (format, expect) in [
            (1u8, Encoding::Paletted),
            (2, Encoding::Dxt),
            (3, Encoding::Bgra),
        ] {
            let mut d = vec![0u8; 0x94 + 64 + 0x100];
            d[..4].copy_from_slice(b"BLP2");
            d[0x04..0x08].copy_from_slice(&1u32.to_le_bytes());
            d[0x08] = format;
            // An alpha depth of 1, which is what a shared `0x08` would read as
            // `1` and misclassify as palettised for every format except 1.
            d[0x09] = 1;
            d[0x14..0x18].copy_from_slice(&0x124u32.to_le_bytes());
            d[0x54 + 64..0x54 + 68].copy_from_slice(&0x100u32.to_le_bytes());
            let h = Header::parse(&d).unwrap_or_else(|e| panic!("format {format}: {e}"));
            assert_eq!(h.format, format);
            assert_eq!(h.alpha_bits, 1, "0x09 is the alpha depth on BLP2");
            assert_eq!(h.encoding(), expect, "encoding byte {format}");
            let e = decode_mip(&d, &h, 0).unwrap_err();
            assert!(matches!(e, Error::Blp2Unmeasured { format: f } if f == format));
            assert_eq!(e.kind(), "blp2-unmeasured");
        }
    }

    /// A `content` byte outside the measured set is refused, and the message
    /// says which byte rather than blaming JPEG.
    #[test]
    fn refuses_a_content_byte_outside_the_measured_set() {
        for content in [2u32, 7, 0xFFFF_FFFF] {
            let mut d = vec![0u8; 0x9C + 64 + 0x100];
            d[..4].copy_from_slice(b"BLP1");
            d[0x04..0x08].copy_from_slice(&content.to_le_bytes());
            d[0x1C..0x20].copy_from_slice(&0x124u32.to_le_bytes());
            let h = Header::parse(&d).unwrap();
            let e = decode_mip(&d, &h, 0).unwrap_err();
            assert!(
                matches!(e, Error::UnknownContent { content: c } if c == content),
                "content {content}: {e}"
            );
            let e = decode_jpeg(&d, &h, 0).unwrap_err();
            assert!(
                matches!(e, Error::UnknownContent { content: c } if c == content),
                "jpeg path, content {content}: {e}"
            );
        }
    }

    /// Each decode entry point refuses the other content kind instead of
    /// producing bytes that are not pixels.
    #[test]
    fn each_content_byte_has_exactly_one_decoding_path() {
        let mut d = vec![0u8; 0x9C + 64 + 0x100];
        d[..4].copy_from_slice(b"BLP1");
        d[0x0C..0x10].copy_from_slice(&1u32.to_le_bytes());
        d[0x10..0x14].copy_from_slice(&1u32.to_le_bytes());
        d[0x1C..0x20].copy_from_slice(&0x124u32.to_le_bytes());
        d[0x5C..0x60].copy_from_slice(&1u32.to_le_bytes());
        // content 0: the JPEG path owns it, the direct path refuses.
        d[0x04..0x08].copy_from_slice(&0u32.to_le_bytes());
        let h = Header::parse(&d).unwrap();
        assert!(matches!(decode_mip(&d, &h, 0), Err(Error::NotDirect)));
        // content 1: the direct path owns it, the JPEG path refuses.
        d[0x04..0x08].copy_from_slice(&1u32.to_le_bytes());
        let h = Header::parse(&d).unwrap();
        assert!(matches!(decode_jpeg(&d, &h, 0), Err(Error::NotDirect)));
        assert_eq!(h.encoding(), Encoding::Paletted);
        assert_eq!(decode(&d).unwrap().1, [0, 0, 0, 255]);
    }

    /// The census buckets a refusal into its own row rather than dropping it,
    /// and names the cause rather than blaming an unrelated one.
    #[test]
    fn census_reports_each_refusal_in_its_own_row() {
        let mut c = Census::new();
        let mut blp0 = vec![0u8; 0x9C + 64];
        blp0[..4].copy_from_slice(b"BLP0");
        blp0[0x04..0x08].copy_from_slice(&1u32.to_le_bytes());
        let mut blp2 = vec![0u8; 0x94 + 64 + 0x40];
        blp2[..4].copy_from_slice(b"BLP2");
        blp2[0x04..0x08].copy_from_slice(&1u32.to_le_bytes());
        blp2[0x08] = 3; // BGRA
        c.record(&blp0);
        c.record(&blp2);
        c.record(b"not a blp at all!");
        assert_eq!(c.len(), 3);
        assert_eq!(c.handled(), 0);
        assert_eq!(c.refused(), 3);
        let rows = c.buckets();
        assert_eq!(rows.len(), 3);
        assert_eq!(&rows[0].magic, b"BLP0");
        assert_eq!(rows[0].refused["blp0-external-mips"], 1);
        assert_eq!(&rows[1].magic, b"BLP2");
        assert_eq!(rows[1].refused["blp2-unmeasured"], 1);
        assert_eq!(&rows[2].magic, b"not ");
        assert_eq!(rows[2].refused["bad-magic"], 1);
    }

    /// An unmeasured `content` byte lands in the BLP1 row and is reported as
    /// such, rather than being counted as handled or blamed on BLP0.
    #[test]
    fn census_refuses_an_unmeasured_content_byte() {
        let mut d = vec![0u8; 0x9C + 64 + 0x40];
        d[..4].copy_from_slice(b"BLP1");
        d[0x04..0x08].copy_from_slice(&7u32.to_le_bytes());
        let mut c = Census::new();
        c.record(&d);
        let rows = c.buckets();
        assert_eq!(&rows[0].magic, b"BLP1");
        assert_eq!(rows[0].content, 7);
        assert_eq!(rows[0].handled, 0);
        assert_eq!(rows[0].refused["unknown-content"], 1);
    }

    #[test]
    fn census_counts_a_short_palette_as_ten_entries_not_twenty_six() {
        let Some(_) = install_dir() else { return };
        let Some(d) = shipped(SHORT_PALETTE) else {
            return;
        };
        let mut c = Census::new();
        c.record(&d);
        let rows = c.buckets();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].handled, 1);
        assert!(rows[0].refused.is_empty());
        assert_eq!(rows[0].bytes_per_pixel[&BPP_INDEX_ALPHA], 1);
        assert_eq!(rows[0].bytes_per_pixel.get(&BPP_INDEX), None);
    }

    /// **The enumeration that closes the variant question.**
    ///
    /// Every `.blp` either shipped build contains is bucketed by magic and
    /// `content` byte, and the answer is asserted rather than printed: two rows,
    /// `BLP1`/`0` and `BLP1`/`1`, and nothing refused. A single `BLP0`, `BLP2` or
    /// out-of-range content byte would add a third row and fail here.
    #[test]
    fn shipped_textures_occupy_exactly_two_buckets_and_nothing_is_refused() {
        let Some(dir) = install_dir() else {
            eprintln!("skipping: no extracted install");
            return;
        };
        let mut set = pcx_mpq::MpqSet::open_wc3(&dir).expect("open WC3 archive set");
        let blps: Vec<String> = set
            .file_list()
            .expect("union listfile")
            .into_iter()
            .filter(|l| l.to_ascii_lowercase().ends_with(".blp"))
            .collect();
        assert!(blps.len() > 500, "only {} .blp entries", blps.len());

        let mut census = Census::new();
        let mut framing_failures = Vec::new();
        for rel in &blps {
            let Ok(d) = set.extract(rel) else { continue };
            census.record(&d);
            let Ok(h) = Header::parse(&d) else { continue };
            let f = Framing::of(&d, &h);
            for v in f.violations(&d, &h) {
                framing_failures.push(format!("{rel}: {v}"));
            }
        }

        let rows = census.buckets();
        for row in &rows {
            eprintln!(
                "  magic={} content={} files={} handled={} refused={:?} bpp={:?}",
                String::from_utf8_lossy(&row.magic),
                row.content,
                row.files,
                row.handled,
                row.refused,
                row.bytes_per_pixel
            );
        }
        eprintln!(
            "blps={} handled={} refused={} framing_failures={}",
            census.len(),
            census.handled(),
            census.refused(),
            framing_failures.len()
        );

        let keys: Vec<([u8; 4], u32)> = rows.iter().map(|r| (r.magic, r.content)).collect();
        assert_eq!(
            keys,
            vec![(*b"BLP1", 0), (*b"BLP1", 1)],
            "unexpected magic x content rows: {keys:?}"
        );
        assert_eq!(census.refused(), 0, "some shipped texture is refused");
        assert_eq!(census.handled(), census.len(), "some file went uncounted");
        assert!(
            framing_failures.is_empty(),
            "{} framing failures, first: {:?}",
            framing_failures.len(),
            &framing_failures[..framing_failures.len().min(5)]
        );
    }

    /// Every `.blp` the archive actually ships, not a hand-picked subset.
    ///
    /// Ignored by default because it decodes the whole texture set; run with
    /// `cargo test -p pcx-blp -- --ignored`.
    #[test]
    #[ignore = "decodes the full shipped texture set"]
    fn every_shipped_blp_has_a_coherent_chain_and_decodes() {
        let Some(dir) = install_dir() else {
            eprintln!("skipping: no extracted install");
            return;
        };
        let mut set = pcx_mpq::MpqSet::open_wc3(&dir).expect("open WC3 archive set");
        let blps: Vec<String> = set
            .file_list()
            .expect("union listfile")
            .into_iter()
            .filter(|l| l.to_ascii_lowercase().ends_with(".blp"))
            .collect();
        assert!(blps.len() > 500, "only {} .blp entries", blps.len());
        let blps: Vec<&str> = blps.iter().map(String::as_str).collect();

        let mut unread = Vec::new();
        let mut failed = Vec::new();
        let mut framing_failures = Vec::new();
        let mut census = Census::new();
        let mut decoded_bytes = 0usize;
        let mut mips_decoded = 0usize;
        for rel in &blps {
            let d = match set.extract(rel) {
                Ok(d) => d,
                Err(e) => {
                    unread.push(format!("{rel}: {e}"));
                    continue;
                }
            };
            census.record(&d);
            let h = match Header::parse(&d) {
                Ok(h) => h,
                Err(e) => {
                    failed.push(format!("{rel}: header: {e}"));
                    continue;
                }
            };

            let n = h.mip_count();
            if n == 0 {
                failed.push(format!("{rel}: no mips"));
                continue;
            }
            for i in 0..n {
                let off = h.mip_offsets[i] as usize;
                let end = off + h.mip_sizes[i] as usize;
                if end > d.len() {
                    failed.push(format!("{rel}: mip {i} past EOF"));
                    break;
                }
                if i + 1 < n && end != h.mip_offsets[i + 1] as usize {
                    failed.push(format!("{rel}: mip {i} does not abut mip {}", i + 1));
                    break;
                }
                if i + 1 == n && end != d.len() {
                    failed.push(format!(
                        "{rel}: last mip ends at {end}, file is {}",
                        d.len()
                    ));
                    break;
                }
            }

            // Framing: palette gap plus every mip is the whole file, the
            // palette is a whole number of entries, and direct mips are
            // exactly width x height x 1 or 2.
            let f = Framing::of(&d, &h);
            for v in f.violations(&d, &h) {
                framing_failures.push(format!("{rel}: {v}"));
            }

            // Every mip decodes, not just mip 0: a chain that only starts
            // correct is not a decoder.
            for i in 0..n {
                let (w, hh) = h.mip_size(i);
                match decode_any(&d, &h, i) {
                    Ok(px) => {
                        if px.len() != (w * hh * 4) as usize {
                            failed.push(format!("{rel}: mip {i} decoded {} bytes", px.len()));
                            break;
                        }
                        mips_decoded += 1;
                    }
                    Err(e) => {
                        failed.push(format!("{rel}: mip {i}: decode: {e}"));
                        break;
                    }
                }
            }
            decoded_bytes += d.len();
        }

        eprintln!("--- census: magic x content ---");
        for row in census.buckets() {
            eprintln!(
                "  magic={:<6} content={} files={:<6} handled={:<6} refused={:?} bpp={:?}",
                String::from_utf8_lossy(&row.magic),
                row.content,
                row.files,
                row.handled,
                row.refused,
                row.bytes_per_pixel
            );
        }
        eprintln!(
            "blps={} unread={} failed={} framing_failures={} mips_decoded={} decoded={} bytes",
            census.len(),
            unread.len(),
            failed.len(),
            framing_failures.len(),
            mips_decoded,
            decoded_bytes
        );
        eprintln!("handled={} refused={}", census.handled(), census.refused());

        // Group by shape so a new encoding shows up as its own bucket.
        let mut shapes: std::collections::BTreeMap<String, usize> = Default::default();
        for f in failed
            .iter()
            .chain(unread.iter())
            .chain(framing_failures.iter())
        {
            let key = if let Some(rest) = f.split("decode: ").nth(1) {
                rest.to_string()
            } else if let Some(rest) = f.split(": ").nth(1) {
                rest.to_string()
            } else {
                f.clone()
            };
            *shapes.entry(key).or_default() += 1;
        }
        for (k, v) in &shapes {
            eprintln!("  shape x{v:<5} {k}");
        }
        let report = std::path::Path::new("/tmp/opencode/blp-sweep.txt");
        if let Ok(mut w) = std::fs::File::create(report) {
            use std::io::Write;
            for f in failed
                .iter()
                .chain(unread.iter())
                .chain(framing_failures.iter())
            {
                let _ = writeln!(w, "{f}");
            }
        }
        eprintln!("full report: {}", report.display());
        for f in failed.iter().take(5) {
            eprintln!("  fail   {f}");
        }
        for f in framing_failures.iter().take(5) {
            eprintln!("  frame  {f}");
        }
        assert!(unread.is_empty(), "{} textures unreadable", unread.len());
        assert!(failed.is_empty(), "{} textures inconsistent", failed.len());
        assert!(
            framing_failures.is_empty(),
            "{} textures violate the framing relation",
            framing_failures.len()
        );
    }
}

#[cfg(test)]
mod jpeg_component_tests {
    use super::*;
    use jpeg_decoder::PixelFormat;

    /// Inverted BGRA with no Warcraft files involved: the fourth component is
    /// alpha and never darkens the colors as black ink would.
    #[test]
    fn four_components_are_inverted_bgra() {
        for p in [
            [200u8, 100, 50, 0],
            [200, 100, 50, 128],
            [200, 100, 50, 255],
        ] {
            assert_eq!(
                rgba_from_jpeg(PixelFormat::CMYK32, &p),
                [205, 155, 55, 255 - p[3]]
            );
        }
        assert_eq!(
            rgba_from_jpeg(PixelFormat::CMYK32, &[40, 66, 67, 0]),
            [188, 189, 215, 255]
        );
    }

    /// Three-component color is reordered; grayscale stays grayscale; both are opaque.
    #[test]
    fn color_channels_reorder_and_grayscale_is_unchanged() {
        assert_eq!(
            rgba_from_jpeg(PixelFormat::RGB24, &[1, 2, 3]),
            [3, 2, 1, 255]
        );
        assert_eq!(rgba_from_jpeg(PixelFormat::L8, &[9]), [9, 9, 9, 255]);
    }
}

#[cfg(test)]
mod logo_alpha_tests {
    use super::*;
    use std::path::PathBuf;

    /// Where the shipped logo might be, from the environment or the repository.
    ///
    /// No absolute home directory: a test that only runs on one machine is a
    /// test that fails everywhere else, and one that silently returns is worse.
    fn install_roots() -> Vec<PathBuf> {
        let mut roots = Vec::new();
        if let Some(p) = std::env::var_os("PEACECRAFT3_LEGACY_DIR") {
            roots.push(PathBuf::from(p));
        }
        if let Some(p) = std::env::var_os("PEACECRAFT3_INSTALL") {
            roots.push(PathBuf::from(p));
        }
        // The repository's own extracted tree, relative to this crate.
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.join("wc3/game/app"));
        if let Some(p) = repo {
            roots.push(p);
        }
        roots
    }

    /// The shipped `WarcraftIII-logo-alpha.blp`, or `None` when no install is
    /// present. The caller turns that into a skip, so the reason is explicit.
    fn logo() -> Option<(Vec<u8>, String)> {
        for root in install_roots() {
            if !root.is_dir() {
                continue;
            }
            if let Ok(mut set) = pcx_mpq::MpqSet::open_wc3(&root)
                && let Ok(b) = set.extract(r"Textures\WarcraftIII-logo-alpha.blp")
                && !b.is_empty()
            {
                return Some((b, root.display().to_string()));
            }
        }
        None
    }

    /// The logo's fourth JPEG component is **alpha**, and reading it as CMYK
    /// makes the whole texture opaque black.
    ///
    /// Ignored by default because it needs a local Warcraft III install: the
    /// fixture is a shipped asset and this repository does not commit those.
    /// Run it with `cargo test -p pcx-blp -- --ignored` where an install exists,
    /// setting `PEACECRAFT3_LEGACY_DIR` if it is not at the repository path.
    #[test]
    #[ignore = "needs a local Warcraft III install; run with --ignored and PEACECRAFT3_LEGACY_DIR set"]
    fn the_logo_decodes_to_transparent_artwork() {
        let Some((bytes, root)) = logo() else {
            panic!(
                "no Warcraft III install found; set PEACECRAFT3_LEGACY_DIR to one of {:?}",
                install_roots()
            );
        };
        let (h, px) = decode(&bytes).unwrap_or_else(|e| panic!("{root}: {e}"));
        assert_eq!(h.content, 0, "JPEG-content BLP");
        assert!(h.alpha_bits > 0, "the texture declares alpha");
        assert_eq!(px.len(), h.width as usize * h.height as usize * 4);
        let pixels = px.as_chunks::<4>().0;
        assert!(pixels.iter().any(|p| p[3] == 0), "transparent field");
        assert!(pixels.iter().any(|p| p[3] == 255), "opaque artwork");
        assert!(
            pixels.iter().any(|p| (1..255).contains(&p[3])),
            "partial-alpha edges"
        );
    }
}

#[cfg(test)]
mod bounded_jpeg_tests {
    use super::*;
    // Synthetic two-pixel grayscale JPEG; no supplied game content.
    fn source() -> Vec<u8> {
        let jpeg: &[u8] = &[
            255, 216, 255, 224, 0, 16, 74, 70, 73, 70, 0, 1, 1, 0, 0, 1, 0, 1, 0, 0, 255, 219, 0,
            67, 0, 6, 4, 5, 5, 5, 4, 6, 5, 5, 5, 7, 6, 6, 7, 9, 15, 10, 9, 8, 8, 9, 19, 13, 14, 11,
            15, 22, 19, 23, 23, 22, 19, 21, 21, 24, 27, 35, 30, 24, 26, 33, 26, 21, 21, 30, 41, 31,
            33, 36, 37, 39, 40, 39, 24, 29, 43, 46, 43, 38, 46, 35, 38, 39, 38, 255, 192, 0, 11, 8,
            0, 1, 0, 2, 1, 1, 17, 0, 255, 196, 0, 20, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 6, 255, 196, 0, 27, 16, 0, 0, 7, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 4,
            5, 6, 54, 117, 179, 255, 218, 0, 8, 1, 1, 0, 0, 63, 0, 109, 12, 167, 177, 103, 167,
            230, 81, 255, 217,
        ];
        let mut bytes = vec![0; 160];
        bytes[..4].copy_from_slice(b"BLP1");
        bytes[12..16].copy_from_slice(&2_u32.to_le_bytes());
        bytes[16..20].copy_from_slice(&1_u32.to_le_bytes());
        bytes[28..32].copy_from_slice(&160_u32.to_le_bytes());
        bytes[92..96].copy_from_slice(&(jpeg.len() as u32).to_le_bytes());
        bytes.extend(jpeg);
        bytes
    }
    #[test]
    fn bounded_decode_preserves_pixels_and_refuses_insufficient_output_budget() {
        let bytes = source();
        let header = Header::parse(&bytes).unwrap();
        let expected = decode_jpeg(&bytes, &header, 0).unwrap();
        assert_eq!(expected.len(), 8);
        assert_eq!(
            decode_jpeg_with_limit(&bytes, &header, 0, 8).unwrap(),
            expected
        );
        assert!(matches!(
            decode_jpeg_with_limit(&bytes, &header, 0, 7),
            Err(Error::Unsupported("jpeg RGBA budget"))
        ));
    }
    #[test]
    fn mismatched_jpeg_frame_is_refused_from_metadata_before_pixel_decode() {
        let mut bytes = source();
        bytes[12..16].copy_from_slice(&1_u32.to_le_bytes());
        let header = Header::parse(&bytes).unwrap();
        assert!(matches!(
            decode_jpeg_with_limit(&bytes, &header, 0, 4),
            Err(Error::UnrecognisedMip { w: 1, h: 1, .. })
        ));
    }
}

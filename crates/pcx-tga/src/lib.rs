//! Truevision TGA decoder for the assets Warcraft III ships as `.tga`.
//!
//! TGA has no magic number. The 18-byte header is identified by its length and
//! by a self-consistency check on the image type and pixel depth; the optional
//! Truevision 2.0 footer at the end of the file is the only reliable
//! discriminator, and it is *not* present on every file (see [`footer`]).
//!
//! ```text
//! 0x00  1  image id length            (bytes skipped before the image data)
//! 0x01  1  colour map type            0 = none, 1 = present
//! 0x02  1  image type                 2 = uncompressed true-colour
//!                                     10 = RLE true-colour
//! 0x03  5  colour map specification   first entry (u16), entry count (u16),
//!                                     entry size in bits
//! 0x08  4  x origin (u16), y origin (u16)
//! 0x0C  4  width (u16), height (u16)
//! 0x10  1  pixel depth in bits        8 / 16 / 24 / 32
//! 0x11  1  descriptor
//!          bit 4  1 = pixels run right-to-left
//!          bit 5  1 = first row is the *top* row (1.0 says it is the bottom)
//!          bits 0..3  attribute (alpha) channel size
//! 0x12  .. image id field, colour map, then the pixel data
//! ```
//!
//! RLE (image type 10) is the Truevision run-length scheme: a packet header
//! byte with the top bit set marks a run of `(b & 0x7F) + 1` copies of the next
//! whole pixel, and a clear top bit marks `(b & 0x7F) + 1` literal pixels
//! following verbatim. Packets are free to straddle row boundaries.
//!
//! ## What the shipped assets actually contain
//!
//! Measured across all 383 `.tga` entries in the union of the four WC3
//! archives:
//!
//! | property                          | measured                              |
//! |-----------------------------------|---------------------------------------|
//! | colour map type                   | 0 on all 383 — no colour-mapped image |
//! | image type                        | 2 on 368, 10 on 15                    |
//! | pixel depth                       | 24 on 198, 32 on 185                  |
//! | descriptor bits 0..3              | 0 on every 24bpp file, 8 on every 32bpp |
//! | descriptor bit 4 (right-to-left)  | 0 on all 383                          |
//! | descriptor bit 5 (top origin)     | 0 on all 383 — every file is bottom-up |
//! | colour map entries / id length    | 0 on all 383                          |
//! | trailing Truevision 2.0 footer    | present on 381, absent on 2           |
//!
//! Consequences the decoder has to honour:
//!
//! * **Every shipped file is bottom-up.** Rows are stored last-row-first, so
//!   decoding without flipping yields vertically mirrored art. This is the one
//!   flag that is uniformly set, so getting it wrong is silently wrong for the
//!   whole game.
//! * **The footer is not universal.** The two exceptions are
//!   `Objects\Spawnmodels\Orc\OrcBlood\BloodSpurtSmall01.tga` and its duplicate
//!   `...\OrcSmallDeathExplode\BloodSpurtSmall01.tga`, whose RLE stream ends
//!   26 bytes before EOF with pixel data rather than a signature. The image
//!   therefore ends when `width * height` pixels have been produced, and
//!   anything after that is metadata — never a reason to fail.
//! * **39 of the 183 uncompressed 32bpp textures ship an all-zero attribute
//!   byte**, so their `descriptor` says "8 attribute bits" while the data is
//!   really 24bpp padding. They decode to fully transparent images, which is
//!   correct output, not a failure. Only 2 more 32bpp files are uniformly
//!   opaque; the remaining 142 use alpha for real.
//! * **Nothing exercises colour-mapped images, 8/16-bit depths or
//!   right-to-left rows.** Colour maps and unsupported depths are refused.
//!   Right-to-left ordering follows the Truevision specification. 16-bit packing is
//!   implemented because it is unambiguous in the public spec, but no shipped
//!   file reaches that branch.
//!
//! All output is straight (non-premultiplied) RGBA8, row-major, top-left
//! origin, matching [`pcx_blp`]. The decoded output of all 383 files was
//! compared byte for byte against an independent decoder (Pillow's TGA plugin)
//! via [`fnv1a64`]; all 383 agree.

use std::path::Path;

/// Length of the fixed TGA header, in bytes.
pub const HEADER_LEN: usize = 18;
/// Length of the Truevision 2.0 footer, in bytes, as Blizzard writes it.
pub const FOOTER_LEN: usize = 26;
/// The 17-byte ASCII tag in the footer, excluding its NUL terminator.
pub const FOOTER_SIGNATURE: &[u8] = b"TRUEVISION-XFILE.";

/// The 18-byte TGA header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    /// Bytes of image-identification field before the pixel data.
    pub id_length: u8,
    /// 0 when no colour map is present.
    pub colour_map_type: u8,
    pub image_type: ImageType,
    pub colour_map_first_entry: u16,
    pub colour_map_entries: u16,
    pub colour_map_entry_bits: u8,
    pub x_origin: u16,
    pub y_origin: u16,
    pub width: u16,
    pub height: u16,
    /// Bits per stored pixel: 8, 15, 16, 24 or 32 in practice.
    pub pixel_depth: u8,
    /// The raw descriptor byte; read it through [`Header::origin`] etc.
    pub descriptor: u8,
}

/// The image-type field. Only the variants WC3 ships are named; everything
/// else is kept as [`ImageType::Other`] so the error message stays exact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ImageType {
    UncompressedColourMapped,
    UncompressedTrueColour,
    RleColourMapped,
    RleTrueColour,
    Other(u8),
}

impl ImageType {
    fn from_raw(b: u8) -> Self {
        match b {
            1 => Self::UncompressedColourMapped,
            2 => Self::UncompressedTrueColour,
            9 => Self::RleColourMapped,
            10 => Self::RleTrueColour,
            other => Self::Other(other),
        }
    }

    /// The raw header byte, including for [`ImageType::Other`].
    pub fn raw(self) -> u8 {
        match self {
            Self::UncompressedColourMapped => 1,
            Self::UncompressedTrueColour => 2,
            Self::RleColourMapped => 9,
            Self::RleTrueColour => 10,
            Self::Other(b) => b,
        }
    }

    /// True for the two image types whose pixels are run-length encoded.
    pub fn is_run_length_encoded(self) -> bool {
        matches!(self, Self::RleTrueColour | Self::RleColourMapped)
    }
}

/// Where the first stored pixel sits within the raster.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// `descriptor` bit 5 clear: the first stored row is the *bottom* row.
    /// Truevision 1.0's only legal option, and what all 383 files use.
    BottomLeft,
    /// `descriptor` bit 5 set: the first stored row is the top row.
    TopLeft,
}

/// A TGA variant this crate deliberately does not decode.
///
/// Each variant is listed rather than folded into a message string so the
/// crate's full-sweep test can bucket them and report an exact count, and so
/// a caller can match on the reason instead of parsing text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unsupported {
    /// Image types 1 and 9 need a colour map, and no shipped file has one.
    ColourMapped {
        image_type: ImageType,
        entries: u16,
        entry_bits: u8,
    },
    /// A `colour map type` byte that is neither 0 nor 1, so the colour map's
    /// size — and therefore the image data offset — cannot be derived.
    ColourMapType(u8),
    /// Grayscale (types 3 and 11) and every other undefined image type.
    ImageType(ImageType),
    /// A pixel depth outside the documented 8/15/16/24/32 set.
    PixelDepth(u8),
    /// A 16-bit pixel whose descriptor attribute size matches neither
    /// A1R5G5B5 nor A4R4G4B4.
    AmbiguousDepth16 { attribute_bits: u8 },
    /// `descriptor` bit 4. No shipped file sets it, so the flip is
    /// implemented per spec but has no real-world data behind it.
    RightToLeft,
}

impl std::fmt::Display for Unsupported {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ColourMapped {
                image_type,
                entries,
                entry_bits,
            } => write!(
                f,
                "colour-mapped image type {} ({entries} entries of {entry_bits} bits); \
                 no shipped file uses one",
                image_type.raw()
            ),
            Self::ColourMapType(t) => write!(f, "colour map type {t} is neither 0 nor 1"),
            Self::ImageType(t) => write!(f, "image type {} is not decoded", t.raw()),
            Self::PixelDepth(b) => write!(f, "pixel depth {b} bits is not decoded"),
            Self::AmbiguousDepth16 { attribute_bits } => write!(
                f,
                "16-bit pixels with {attribute_bits} attribute bits match neither \
                 A1R5G5B5 nor A4R4G4B4"
            ),
            Self::RightToLeft => {
                write!(f, "right-to-left row order is not decoded")
            }
        }
    }
}

impl std::error::Error for Unsupported {}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("truncated: need {need} bytes at {what}, have {have}")]
    Truncated {
        what: &'static str,
        need: usize,
        have: usize,
    },
    #[error("unsupported: {0}")]
    Unsupported(#[from] Unsupported),
    /// A run-length packet that would produce more than `width * height`
    /// pixels, which means the stream and the header disagree.
    #[error("RLE packet of {count} pixels overruns the {total}-pixel raster at {at}")]
    RleOverrun {
        at: usize,
        count: usize,
        total: usize,
    },
    /// The image data ended before the header said the image would.
    #[error("image data ended after {produced} of {total} pixels")]
    RleTruncated { produced: usize, total: usize },
    #[error("uncompressed image wants {want} bytes of pixels at offset {at}, have {have}")]
    RawTooShort { at: usize, want: usize, have: usize },
}

fn u16_at(d: &[u8], o: usize, what: &'static str) -> Result<u16, Error> {
    d.get(o..o + 2)
        .map(|s| u16::from_le_bytes(s.try_into().unwrap()))
        .ok_or(Error::Truncated {
            what,
            need: o + 2,
            have: d.len(),
        })
}

impl Header {
    pub fn parse(d: &[u8]) -> Result<Self, Error> {
        d.get(0..HEADER_LEN).ok_or(Error::Truncated {
            what: "header",
            need: HEADER_LEN,
            have: d.len(),
        })?;
        Ok(Self {
            id_length: d[0],
            colour_map_type: d[1],
            image_type: ImageType::from_raw(d[2]),
            colour_map_first_entry: u16_at(d, 0x03, "colour map first entry")?,
            colour_map_entries: u16_at(d, 0x05, "colour map entry count")?,
            colour_map_entry_bits: d[7],
            x_origin: u16_at(d, 0x08, "x origin")?,
            y_origin: u16_at(d, 0x0A, "y origin")?,
            width: u16_at(d, 0x0C, "width")?,
            height: u16_at(d, 0x0E, "height")?,
            pixel_depth: d[16],
            descriptor: d[17],
        })
    }

    /// Total pixels in the raster.
    pub fn pixel_count(&self) -> usize {
        self.width as usize * self.height as usize
    }

    /// Bytes one stored pixel occupies. Zero for depths that are not decoded.
    pub fn pixel_stride(&self) -> Option<usize> {
        match self.pixel_depth {
            15 | 16 | 24 | 32 => Some(self.pixel_depth as usize / 8),
            _ => None,
        }
    }

    /// `descriptor` bit 5.
    pub fn origin(&self) -> Origin {
        if self.descriptor & 0x20 != 0 {
            Origin::TopLeft
        } else {
            Origin::BottomLeft
        }
    }

    /// `descriptor` bit 4. False on every shipped file.
    pub fn is_right_to_left(&self) -> bool {
        self.descriptor & 0x10 != 0
    }

    /// Size of the colour map in bytes, or `None` when its type byte is
    /// neither 0 nor 1 and the map's extent is therefore unknowable.
    pub fn colour_map_bytes(&self) -> Option<usize> {
        match self.colour_map_type {
            0 => Some(0),
            1 => {
                let entry = usize::from(self.colour_map_entry_bits).div_ceil(8);
                Some(self.colour_map_entries as usize * entry)
            }
            _ => None,
        }
    }

    /// Offset of the first pixel. `None` when the colour map type is invalid.
    pub fn image_data_offset(&self) -> Option<usize> {
        Some(HEADER_LEN + self.id_length as usize + self.colour_map_bytes()?)
    }

    /// Bytes of pixel data a fully decoded image occupies, before any
    /// run-length compression.
    pub fn raw_pixel_bytes(&self) -> Option<usize> {
        Some(self.pixel_count() * self.pixel_stride()?)
    }
}

/// The optional Truevision 2.0 footer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Footer {
    /// Offset of the extension area, or `None` when it is zero.
    pub extension_area_offset: Option<u32>,
    /// Offset of the developer directory, or `None` when it is zero.
    pub developer_directory_offset: Option<u32>,
}

/// Parse the trailing Truevision 2.0 footer, if the file ends with one.
///
/// Measured: 381 of the 383 shipped files end with the byte-exact footer
/// `00 00 00 00 00 00 00 00 "TRUEVISION-XFILE." 00`. Note that Blizzard writes
/// 26 bytes, not the 28 the 2.0 text describes — the version number is absent
/// and the signature is NUL-terminated in its place. Two shipped files end in
/// pixel data instead, so this returns `None` for them rather than guessing.
///
/// A footer is never required to decode: the image is exactly `width * height`
/// pixels starting at [`Header::image_data_offset`].
pub fn footer(d: &[u8]) -> Option<Footer> {
    let tail = d.get(d.len().checked_sub(FOOTER_LEN)?..)?;
    if &tail[8..25] != FOOTER_SIGNATURE {
        return None;
    }
    let ext = u32::from_le_bytes(tail[0..4].try_into().unwrap());
    let dev = u32::from_le_bytes(tail[4..8].try_into().unwrap());
    Some(Footer {
        extension_area_offset: (ext != 0).then_some(ext),
        developer_directory_offset: (dev != 0).then_some(dev),
    })
}

/// How one stored pixel expands to RGBA8.
///
/// Resolved once from the header so no pixel is ever disambiguated by peeking
/// at its own value: a decoder that guesses a packing from pixel content will
/// silently pick differently depending on the image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Packing {
    /// 24bpp, file order B, G, R, no attribute byte.
    Bgr,
    /// 32bpp, file order B, G, R, A.
    Bgra,
    /// 15/16bpp A1R5G5B5. Selected by 0 or 1 attribute bits.
    A1R5G5B5,
    /// 16bpp A4R4G4B4. Selected by 3, 4 or 8 attribute bits.
    A4R4G4B4,
}

impl Packing {
    fn stride(self) -> usize {
        match self {
            Self::Bgr => 3,
            Self::Bgra => 4,
            Self::A1R5G5B5 | Self::A4R4G4B4 => 2,
        }
    }

    /// Expand the 5-bit and 4-bit forms by bit replication, the convention
    /// every Truevision reader uses: 5-bit `10000` becomes `11111000`, not
    /// `11111000 & 0xF8`. Reproducing the high bits into the low two is what
    /// makes full-scale white come out as 0xFF.
    fn expand(self, raw: &[u8], out: &mut [u8; 4]) {
        match self {
            Self::Bgr => out.copy_from_slice(&[raw[2], raw[1], raw[0], 0xFF]),
            Self::Bgra => out.copy_from_slice(&[raw[2], raw[1], raw[0], raw[3]]),
            Self::A1R5G5B5 => {
                let v = u16::from_le_bytes([raw[0], raw[1]]);
                let f = |c: u16| {
                    let c = (c & 0x1F) as u8;
                    (c << 3) | (c >> 2)
                };
                let a = if v & 0x8000 != 0 { 0xFF } else { 0x00 };
                out.copy_from_slice(&[f(v >> 10), f(v >> 5), f(v), a]);
            }
            Self::A4R4G4B4 => {
                let v = u16::from_le_bytes([raw[0], raw[1]]);
                let q = |c: u16| ((c & 0x0F) as u8) * 17;
                out.copy_from_slice(&[q(v >> 12), q(v >> 8), q(v >> 4), q(v)]);
            }
        }
    }
}

/// Refuse anything this crate will not decode, and pin down the pixel packing.
fn resolve(h: &Header) -> Result<(usize, usize, Packing), Unsupported> {
    match h.image_type {
        ImageType::UncompressedTrueColour | ImageType::RleTrueColour => {}
        ImageType::UncompressedColourMapped | ImageType::RleColourMapped => {
            return Err(Unsupported::ColourMapped {
                image_type: h.image_type,
                entries: h.colour_map_entries,
                entry_bits: h.colour_map_entry_bits,
            });
        }
        ImageType::Other(_) => return Err(Unsupported::ImageType(h.image_type)),
    }
    if h.colour_map_bytes().is_none() {
        return Err(Unsupported::ColourMapType(h.colour_map_type));
    }
    let packing = match h.pixel_depth {
        24 => Packing::Bgr,
        32 => Packing::Bgra,
        15 | 16 => match h.descriptor & 0x0F {
            0 | 1 => Packing::A1R5G5B5,
            3 | 4 | 8 => Packing::A4R4G4B4,
            other => {
                return Err(Unsupported::AmbiguousDepth16 {
                    attribute_bits: other,
                });
            }
        },
        depth => return Err(Unsupported::PixelDepth(depth)),
    };
    Ok((
        h.image_data_offset()
            .expect("colour map type validated above"),
        packing.stride(),
        packing,
    ))
}

/// Decode to straight RGBA8, row-major, top-left origin.
///
/// Reads exactly `width * height` pixels from the image data offset and stops
/// there; any trailing bytes (typically the [`footer`]) are ignored. Rows are
/// reversed when the header says the first stored row is the bottom one, which
/// is the case for every shipped asset.
///
/// Variants this crate cannot prove are refused with [`Error::Unsupported`]
/// rather than decoded speculatively.
pub fn decode_with(d: &[u8], h: &Header) -> Result<Vec<u8>, Error> {
    let (at, stride, packing) = resolve(h).map_err(Error::from)?;
    let total = h.pixel_count();

    // Filled in stored order first: bottom row first when the origin is
    // bottom-left, so the flip is one contiguous reversal afterwards.
    let mut raw = vec![0u8; total * 4];

    if h.image_type.is_run_length_encoded() {
        let mut i = at;
        let mut done = 0usize;
        let mut px = [0u8; 4];
        while done < total {
            let pkt = *d.get(i).ok_or(Error::RleTruncated {
                produced: done,
                total,
            })?;
            i += 1;
            // The count is 1..=128; the bias is what makes a zero-count byte
            // mean "one pixel" instead of stalling the loop.
            let count = usize::from(pkt & 0x7F) + 1;
            if done + count > total {
                return Err(Error::RleOverrun {
                    at: i - 1,
                    count,
                    total,
                });
            }
            let end = done + count;
            if pkt & 0x80 != 0 {
                let one = d.get(i..i + stride).ok_or(Error::RleTruncated {
                    produced: done,
                    total,
                })?;
                packing.expand(one, &mut px);
                for slot in done..end {
                    px_at(&mut raw, slot).copy_from_slice(&px);
                }
                i += stride;
            } else {
                let span = d.get(i..i + count * stride).ok_or(Error::RleTruncated {
                    produced: done,
                    total,
                })?;
                for (slot, one) in (done..end).zip(span.chunks_exact(stride)) {
                    packing.expand(one, px_at(&mut raw, slot));
                }
                i += count * stride;
            }
            done = end;
        }
    } else {
        let want = total * stride;
        let span = d.get(at..at + want).ok_or(Error::RawTooShort {
            at,
            want,
            have: d.len().saturating_sub(at),
        })?;
        for (slot, one) in (0..total).zip(span.chunks_exact(stride)) {
            packing.expand(one, px_at(&mut raw, slot));
        }
    }

    if h.origin() == Origin::BottomLeft {
        raw = flip_vertically(&raw, h.width as usize, h.height as usize);
    }
    if h.is_right_to_left() && h.width != 0 {
        for row in raw.chunks_exact_mut(h.width as usize * 4) {
            for x in 0..h.width as usize / 2 {
                let opposite = h.width as usize - 1 - x;
                for channel in 0..4 {
                    row.swap(x * 4 + channel, opposite * 4 + channel);
                }
            }
        }
    }
    Ok(raw)
}

/// Borrow pixel `i` of an RGBA8 buffer as a fixed-size array.
#[inline]
fn px_at(px: &mut [u8], i: usize) -> &mut [u8; 4] {
    <&mut [u8; 4]>::try_from(&mut px[i * 4..i * 4 + 4]).expect("4-byte pixel slot")
}

/// Reverse the row order: a bottom-left stored raster becomes top-left.
///
/// Rows are whole 4-byte pixels, so this cannot split a pixel, and the source
/// and destination never overlap because the output is a fresh buffer.
fn flip_vertically(px: &[u8], width: usize, height: usize) -> Vec<u8> {
    let row = width * 4;
    let mut out = vec![0u8; px.len()];
    for y in 0..height {
        let src = (height - 1 - y) * row;
        out[y * row..y * row + row].copy_from_slice(&px[src..src + row]);
    }
    out
}

/// FNV-1a over 64 bits, so an independent decoder can be compared byte for
/// byte against [`decode`]'s output.
pub fn fnv1a64(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in data {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

/// Parse the header and decode to RGBA8.
pub fn decode(d: &[u8]) -> Result<(Header, Vec<u8>), Error> {
    let h = Header::parse(d)?;
    let px = decode_with(d, &h)?;
    Ok((h, px))
}

pub fn load(path: impl AsRef<Path>) -> Result<(Header, Vec<u8>), Error> {
    decode(&std::fs::read(path)?)
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
    fn shipped(rel: &str) -> Option<Vec<u8>> {
        let dir = install_dir()?;
        pcx_mpq::MpqSet::open_wc3(dir).ok()?.extract(rel).ok()
    }

    /// (path, image type, width, height, pixel depth, descriptor, footer).
    ///
    /// The set covers every shape the archives actually contain: both image
    /// types, both pixel depths, both with and without a trailing footer.
    const SAMPLES: &[(&str, ImageType, u16, u16, u8, u8, bool)] = &[
        (
            "PathTextures\\10x10Unbuildable.tga",
            ImageType::UncompressedTrueColour,
            10,
            10,
            32,
            0x08,
            true,
        ),
        (
            "PathTextures\\12x10Pen45Path.tga",
            ImageType::UncompressedTrueColour,
            12,
            10,
            24,
            0x00,
            true,
        ),
        (
            "SharedModels\\Destroyer.tga",
            ImageType::UncompressedTrueColour,
            256,
            256,
            32,
            0x08,
            true,
        ),
        (
            "Objects\\Spawnmodels\\Undead\\UCancelDeath\\AuraRune7Green.tga",
            ImageType::UncompressedTrueColour,
            256,
            256,
            32,
            0x08,
            true,
        ),
        (
            "Objects\\Spawnmodels\\Undead\\UCancelDeath\\GenericGlow2c.tga",
            ImageType::UncompressedTrueColour,
            128,
            128,
            24,
            0x00,
            true,
        ),
        (
            "SharedModels\\Red_Glow3.tga",
            ImageType::RleTrueColour,
            32,
            32,
            24,
            0x00,
            true,
        ),
        (
            "Objects\\Spawnmodels\\Undead\\UCancelDeath\\pixies1.tga",
            ImageType::RleTrueColour,
            128,
            128,
            24,
            0x00,
            true,
        ),
        (
            // One of the two files with no footer at all.
            "Objects\\Spawnmodels\\Orc\\OrcBlood\\BloodSpurtSmall01.tga",
            ImageType::RleTrueColour,
            16,
            16,
            32,
            0x08,
            false,
        ),
    ];

    /// FNV-1a 64 of each sample's decoded RGBA8, cross-checked against an
    /// independent decoder (Pillow's TGA plugin) over the whole shipped set.
    ///
    /// These are the digests `tgax hash <file>` prints, so a regression in
    /// channel order, the origin flip or the RLE walk shows up as a changed
    /// number rather than as a subtly wrong image.
    const SAMPLE_DIGESTS: &[(&str, u64)] = &[
        ("PathTextures\\10x10Unbuildable.tga", 0x3183_c773_fef1_7d85),
        ("PathTextures\\12x10Pen45Path.tga", 0x59cc_60bd_986d_3da5),
        ("SharedModels\\Destroyer.tga", 0xd787_6563_ecc5_e378),
        (
            "Objects\\Spawnmodels\\Undead\\UCancelDeath\\AuraRune7Green.tga",
            0x0872_b705_ac5c_ed25,
        ),
        (
            "Objects\\Spawnmodels\\Undead\\UCancelDeath\\GenericGlow2c.tga",
            0xdb80_58b0_b1a5_f7bc,
        ),
        ("SharedModels\\Red_Glow3.tga", 0x877d_31e6_3be6_3cc6),
        (
            "Objects\\Spawnmodels\\Undead\\UCancelDeath\\pixies1.tga",
            0x36ef_46b7_5325_9801,
        ),
        (
            "Objects\\Spawnmodels\\Orc\\OrcBlood\\BloodSpurtSmall01.tga",
            0xaacc_e886_ce21_6371,
        ),
    ];

    #[test]
    fn shipped_headers_match_the_measured_layout() {
        let Some(_) = install_dir() else {
            eprintln!("skipping: no extracted install");
            return;
        };
        let mut seen = 0;
        for (rel, ity, w, h, depth, desc, has_footer) in SAMPLES {
            let Some(d) = shipped(rel) else {
                eprintln!("skipping {rel}: not in archives");
                continue;
            };
            let hdr = Header::parse(&d).unwrap_or_else(|e| panic!("{rel}: {e}"));
            assert_eq!(hdr.image_type, *ity, "{rel}: image type");
            assert_eq!(hdr.width, *w, "{rel}: width");
            assert_eq!(hdr.height, *h, "{rel}: height");
            assert_eq!(hdr.pixel_depth, *depth, "{rel}: pixel depth");
            assert_eq!(hdr.descriptor, *desc, "{rel}: descriptor");
            assert_eq!(hdr.id_length, 0, "{rel}: image id field");
            assert_eq!(hdr.colour_map_type, 0, "{rel}: colour map type");
            assert_eq!(hdr.colour_map_entries, 0, "{rel}: colour map entries");
            assert_eq!(
                hdr.image_data_offset(),
                Some(HEADER_LEN),
                "{rel}: image data offset"
            );
            assert_eq!(hdr.origin(), Origin::BottomLeft, "{rel}: origin");
            assert!(!hdr.is_right_to_left(), "{rel}: row order");
            assert_eq!(footer(&d).is_some(), *has_footer, "{rel}: footer");
            seen += 1;
        }
        assert!(seen >= 6, "only {seen} of the sample files were readable");
    }

    #[test]
    fn decodes_to_exactly_one_rgba_pixel_per_image_pixel() {
        let Some(_) = install_dir() else { return };
        let mut checked = 0;
        let mut any_opaque = 0;
        for (rel, _, w, h, ..) in SAMPLES {
            let Some(d) = shipped(rel) else { continue };
            let (hdr, px) = decode(&d).unwrap_or_else(|e| panic!("{rel}: {e}"));
            assert_eq!(
                px.len(),
                (*w as usize) * (*h as usize) * 4,
                "{rel}: expected w*h RGBA8"
            );
            assert_eq!(hdr.pixel_count(), px.len() / 4, "{rel}: pixel count");
            // Opacity is deliberately *not* asserted per file: 39 of the 183
            // uncompressed 32bpp textures ship an all-zero attribute byte, so a
            // "must have an opaque pixel" check would fail on correct output.
            any_opaque += px.chunks(4).filter(|p| p[3] != 0).count();
            checked += 1;
        }
        assert!(checked >= 6, "only {checked} samples were readable");
        assert!(any_opaque > 0, "no sample decoded a single opaque pixel");
    }

    /// For an uncompressed file the payload is exactly the raster, so flipping
    /// the decode back and re-packing to BGRA has to reproduce the source
    /// bytes. This is the strongest available check that channel order, byte
    /// order and the origin flip are all correct at once.
    #[test]
    fn uncompressed_payload_round_trips_through_the_decoder() {
        let Some(_) = install_dir() else { return };
        let mut checked = 0;
        for (rel, ..) in SAMPLES {
            let Some(d) = shipped(rel) else { continue };
            let hdr = Header::parse(&d).unwrap();
            if hdr.image_type.is_run_length_encoded() || hdr.pixel_depth != 32 {
                continue;
            }
            let (_, px) = decode(&d).unwrap();
            let (w, h) = (hdr.width as usize, hdr.height as usize);
            let at = hdr.image_data_offset().unwrap();
            let source = &d[at..at + w * h * 4];

            // Reverse the origin flip, then swap RGBA back to BGRA.
            let mut rebuilt = vec![0u8; w * h * 4];
            for y in 0..h {
                let row = w * 4;
                let dst = y * row;
                let src = (h - 1 - y) * row;
                for x in 0..w {
                    let p = &px[dst + x * 4..dst + x * 4 + 4];
                    rebuilt[src + x * 4..src + x * 4 + 4]
                        .copy_from_slice(&[p[2], p[1], p[0], p[3]]);
                }
            }
            assert_eq!(rebuilt, source, "{rel}: decode does not round-trip");
            checked += 1;
        }
        assert!(checked >= 2, "only {checked} 32bpp samples were readable");
    }

    /// Pinned FNV-1a 64 digests of the decoded RGBA, taken from a run that was
    /// verified byte-for-byte against an independent decoder over all 383
    /// shipped files. A channel-order or flip regression moves these.
    #[test]
    fn decoded_digests_match_an_independent_decoder() {
        let Some(_) = install_dir() else { return };
        let mut checked = 0;
        for (rel, want) in SAMPLE_DIGESTS {
            let Some(d) = shipped(rel) else {
                eprintln!("skipping {rel}: not in archives");
                continue;
            };
            let (_, px) = decode(&d).unwrap_or_else(|e| panic!("{rel}: {e}"));
            assert_eq!(fnv1a64(&px), *want, "{rel}: decoded digest changed");
            checked += 1;
        }
        assert!(checked >= 6, "only {checked} digests were checked");
    }

    /// Four shipped copies of the same artwork must decode identically, which
    /// is a cross-file check the per-file tests cannot make.
    #[test]
    fn duplicated_artwork_decodes_identically() {
        let Some(_) = install_dir() else { return };
        const COPIES: &[&str] = &[
            "Objects\\Spawnmodels\\Undead\\UCancelDeath\\AuraRune7Green.tga",
            "Objects\\Spawnmodels\\Undead\\UDeathMedium\\AuraRune7Green.tga",
            "Objects\\Spawnmodels\\Undead\\UDeathSmall\\AuraRune7Green.tga",
            "SharedModels\\AuraRune7Green.tga",
        ];
        let mut digests = std::collections::BTreeSet::new();
        let mut seen = 0;
        for rel in COPIES {
            let Some(d) = shipped(rel) else { continue };
            let (_, px) = decode(&d).unwrap_or_else(|e| panic!("{rel}: {e}"));
            digests.insert(fnv1a64(&px));
            seen += 1;
        }
        assert!(seen >= 2, "only {seen} copies were readable");
        assert_eq!(
            digests.len(),
            1,
            "duplicate AuraRune7Green textures decoded differently"
        );
    }

    /// Rows are stored bottom-first in every shipped file, so the decoded
    /// image has to be vertically flipped. `12x12WallFountain` was picked
    /// because its stored first and last rows actually differ, which is what
    /// makes the flip observable rather than incidental.
    #[test]
    fn stored_rows_are_bottom_up_and_get_flipped() {
        let Some(_) = install_dir() else { return };
        let Some(d) = shipped("PathTextures\\12x12WallFountain.tga") else {
            return;
        };
        let hdr = Header::parse(&d).unwrap();
        assert_eq!(hdr.image_type, ImageType::UncompressedTrueColour);
        assert_eq!(hdr.origin(), Origin::BottomLeft);
        let stride = hdr.pixel_stride().unwrap();
        let w = hdr.width as usize;
        let h = hdr.height as usize;
        let payload = &d[hdr.image_data_offset().unwrap()..];

        // Row 0 as the file stores it, expanded to RGBA so it can be compared
        // against the decoder's output rather than against raw BGR.
        let to_rgba = |row: &[u8]| -> Vec<u8> {
            row.chunks_exact(stride)
                .flat_map(|p| [p[2], p[1], p[0], 255])
                .collect()
        };
        let stored_first = to_rgba(&payload[..w * stride]);
        let stored_last = to_rgba(&payload[(w * (h - 1) * stride)..][..w * stride]);
        assert_ne!(stored_first, stored_last, "sample rows must differ");

        let (_, px) = decode(&d).unwrap();
        assert_eq!(
            &px[..w * 4],
            &stored_last[..],
            "decoded row 0 did not come from the file's last stored row"
        );
        assert_eq!(
            &px[w * (h - 1) * 4..][..w * 4],
            &stored_first[..],
            "decoded last row did not come from the file's first stored row"
        );
    }

    /// A bottom-up file and an explicitly top-down copy of it must decode to
    /// the same image, which is the only way to show the flip is an inversion
    /// rather than a shift or a dropped row.
    #[test]
    fn top_and_bottom_origin_encode_the_same_raster() {
        // A 2x2 image, top row red/green, bottom row blue/white.
        let image: [[u8; 3]; 4] = [
            [255, 0, 0],     // (0, 0) top-left
            [0, 255, 0],     // (1, 0) top-right
            [0, 0, 255],     // (0, 1) bottom-left
            [255, 255, 255], // (1, 1) bottom-right
        ];
        let bgr = |c: [u8; 3]| [c[2], c[1], c[0]];
        let header = |desc: u8| vec![0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 2, 0, 24, desc];

        let mut bottom_up = header(0x00);
        // Stored order runs bottom row first.
        for row in [2usize, 3, 0, 1] {
            bottom_up.extend_from_slice(&bgr(image[row]));
        }

        let mut top_down = header(0x20);
        for row in 0..4 {
            top_down.extend_from_slice(&bgr(image[row]));
        }

        let (hb, pb) = decode(&bottom_up).unwrap();
        let (ht, pt) = decode(&top_down).unwrap();
        assert_eq!(hb.origin(), Origin::BottomLeft);
        assert_eq!(ht.origin(), Origin::TopLeft);
        assert_eq!(pb, pt, "the origin flag changed the decoded image");
        assert_eq!(
            pb,
            [
                255, 0, 0, 255, // top-left red
                0, 255, 0, 255, // top-right green
                0, 0, 255, 255, // bottom-left blue
                255, 255, 255, 255, // bottom-right white
            ]
        );
    }

    /// A literal packet and a run packet over the same pixels must agree, and
    /// an all-transparent input must survive the RLE path unchanged.
    #[test]
    fn rle_run_and_literal_packets_agree() {
        let px: [[u8; 4]; 4] = [[1, 2, 3, 4], [1, 2, 3, 4], [1, 2, 3, 4], [9, 9, 9, 9]];
        let bgra = |p: [u8; 4]| [p[2], p[1], p[0], p[3]];

        // Literal packet of 3, then a run packet of 1.
        let mut body = vec![0x02];
        for p in &px[..3] {
            body.extend_from_slice(&bgra(*p));
        }
        body.push(0x80); // run of 1
        body.extend_from_slice(&bgra(px[3]));

        let mut file = vec![0, 0, 10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 0, 1, 0, 32, 0x08];
        file.extend_from_slice(&body);

        let (_, out) = decode(&file).unwrap();
        assert_eq!(
            out,
            [1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 9, 9, 9, 9],
            "RLE decoded to the wrong pixels"
        );
    }

    /// The two shipped files with no footer must still decode, and their
    /// trailing bytes must be pixel data rather than a signature.
    #[test]
    fn a_missing_footer_is_not_an_error() {
        let Some(_) = install_dir() else { return };
        let rel = "Objects\\Spawnmodels\\Orc\\OrcBlood\\BloodSpurtSmall01.tga";
        let Some(d) = shipped(rel) else { return };
        assert_eq!(footer(&d), None, "{rel} is documented as footer-less");
        let (hdr, px) = decode(&d).unwrap_or_else(|e| panic!("{rel}: {e}"));
        assert_eq!(px.len(), hdr.pixel_count() * 4);
    }

    #[test]
    fn rejects_short_headers_and_odd_variants() {
        assert!(matches!(
            Header::parse(&[0u8; 17]),
            Err(Error::Truncated { .. })
        ));

        // A colour-mapped image: refused, and named precisely.
        let mut colour_mapped = vec![0u8; HEADER_LEN];
        colour_mapped[1] = 1;
        colour_mapped[2] = 1; // uncompressed colour-mapped
        colour_mapped[5] = 4; // 4 entries
        colour_mapped[7] = 24; // 24 bits per entry
        let hdr = Header::parse(&colour_mapped).unwrap();
        assert!(matches!(
            decode_with(&colour_mapped, &hdr),
            Err(Error::Unsupported(Unsupported::ColourMapped {
                image_type: ImageType::UncompressedColourMapped,
                entries: 4,
                entry_bits: 24,
            }))
        ));

        // An 8-bit depth, which would be greyscale or colour-mapped.
        let mut eight = vec![0u8; HEADER_LEN];
        eight[2] = 2;
        eight[12] = 1;
        eight[14] = 1;
        eight[16] = 8;
        let hdr = Header::parse(&eight).unwrap();
        assert!(matches!(
            decode_with(&eight, &hdr),
            Err(Error::Unsupported(Unsupported::PixelDepth(8)))
        ));

        // Right-to-left rows.
        let mut rtl = vec![0u8; HEADER_LEN + 3];
        rtl[2] = 2;
        rtl[12] = 1;
        rtl[14] = 1;
        rtl[16] = 24;
        rtl[17] = 0x10;
        let hdr = Header::parse(&rtl).unwrap();
        assert!(hdr.is_right_to_left());
        assert_eq!(decode_with(&rtl, &hdr).unwrap(), [0, 0, 0, 255]);
    }

    #[test]
    fn refuses_a_run_that_overruns_the_raster() {
        // One pixel header, but a packet that claims 4.
        let mut f = vec![0, 0, 10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 0, 24, 0];
        f.extend_from_slice(&[0x83, 1, 2, 3]); // run of 4 pixels
        let (hdr, _) = Header::parse(&f).map(|h| (h, ())).unwrap();
        assert!(matches!(
            decode_with(&f, &hdr),
            Err(Error::RleOverrun {
                count: 4,
                total: 1,
                ..
            })
        ));
    }

    #[test]
    fn footer_recognises_the_shipped_signature_only() {
        // Byte-for-byte the shape Blizzard writes: two zero u32 offsets, the
        // 17-byte signature, then a NUL where the spec would put a version.
        let mut f = vec![0u8; 8];
        f.extend_from_slice(FOOTER_SIGNATURE);
        f.push(0);
        assert_eq!(f.len(), FOOTER_LEN);
        assert_eq!(
            footer(&f),
            Some(Footer {
                extension_area_offset: None,
                developer_directory_offset: None,
            })
        );
        // A file whose last 26 bytes merely happen to be pixel data.
        assert_eq!(footer(&[0xAB; 40]), None);
        assert_eq!(footer(&[0xAB; 10]), None);
    }

    #[test]
    fn fnv1a64_matches_the_reference_vectors() {
        assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a64(b"foobar"), 0x8594_4171_f739_67e8);
    }

    /// Every `.tga` the archives actually ship, not a hand-picked subset.
    ///
    /// Ignored by default because it decodes the whole set; run with
    /// `cargo test -p pcx-tga -- --ignored`.
    #[test]
    #[ignore = "decodes the full shipped TGA set"]
    fn every_shipped_tga_decodes() {
        let Some(dir) = install_dir() else {
            eprintln!("skipping: no extracted install");
            return;
        };
        let mut set = pcx_mpq::MpqSet::open_wc3(&dir).expect("open WC3 archive set");
        let tgas: Vec<String> = set
            .file_list()
            .expect("union listfile")
            .into_iter()
            .filter(|l| l.to_ascii_lowercase().ends_with(".tga"))
            .collect();
        assert!(tgas.len() > 300, "only {} .tga entries", tgas.len());

        let mut unread = Vec::new();
        let mut failed = Vec::new();
        let mut variants: std::collections::BTreeMap<String, usize> = Default::default();
        let mut decoded_bytes = 0usize;
        let mut with_footer = 0usize;
        let mut without_footer = 0usize;
        let mut fully_transparent = 0usize;

        for rel in &tgas {
            let d = match set.extract(rel) {
                Ok(d) => d,
                Err(e) => {
                    unread.push(format!("{rel}: {e}"));
                    continue;
                }
            };
            let hdr = match Header::parse(&d) {
                Ok(h) => h,
                Err(e) => {
                    failed.push(format!("{rel}: header: {e}"));
                    continue;
                }
            };
            *variants
                .entry(format!(
                    "type={} bpp={} descriptor={:#04x} cmap={} origin={:?}",
                    hdr.image_type.raw(),
                    hdr.pixel_depth,
                    hdr.descriptor,
                    hdr.colour_map_type,
                    hdr.origin(),
                ))
                .or_default() += 1;

            match footer(&d) {
                Some(_) => with_footer += 1,
                None => without_footer += 1,
            }

            match decode_with(&d, &hdr) {
                Ok(px) => {
                    if px.len() != hdr.pixel_count() * 4 {
                        failed.push(format!(
                            "{rel}: decoded {} bytes, expected {}",
                            px.len(),
                            hdr.pixel_count() * 4
                        ));
                    }
                    // Transparency is a measured property of the assets, not a
                    // decode failure: 39 of the 183 uncompressed 32bpp textures
                    // carry an all-zero attribute byte and are meant to be
                    // fully transparent. Counted, not asserted.
                    if !px.chunks(4).any(|p| p[3] != 0) {
                        fully_transparent += 1;
                    }
                    decoded_bytes += px.len();
                }
                Err(e) => failed.push(format!("{rel}: decode: {e}")),
            }
        }

        eprintln!(
            "tgas={} unread={} failed={} decoded={decoded_bytes} bytes footer={with_footer} \
             no-footer={without_footer} all-alpha-zero={fully_transparent}",
            tgas.len(),
            unread.len(),
            failed.len(),
        );
        for (k, v) in &variants {
            eprintln!("  {v:>4}  {k}");
        }
        // Group by error shape so a new variant shows up as its own bucket.
        let mut shapes: std::collections::BTreeMap<String, usize> = Default::default();
        for f in failed.iter().chain(unread.iter()) {
            let key = match f.split_once("decode: ") {
                Some((_, rest)) => rest.to_string(),
                None => f.clone(),
            };
            *shapes.entry(key).or_default() += 1;
        }
        for (k, v) in &shapes {
            eprintln!("  shape x{v:<5} {k}");
        }

        let report = std::path::Path::new("/tmp/opencode/tga-sweep.txt");
        if let Some(parent) = report.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(mut w) = std::fs::File::create(report) {
            use std::io::Write;
            for f in failed.iter().chain(unread.iter()) {
                let _ = writeln!(w, "{f}");
            }
        }
        eprintln!("full report: {}", report.display());
        for f in failed.iter().take(5) {
            eprintln!("  fail   {f}");
        }

        assert!(unread.is_empty(), "{} textures unreadable", unread.len());
        assert!(failed.is_empty(), "{} textures inconsistent", failed.len());
        // The measured footer split is the whole reason `footer()` is optional;
        // if it ever changes the odd files were quietly re-exported.
        //
        // The split is per build, so it is pinned per build rather than to a
        // single constant. 1.28c reuses 1.29b's TGA set, so it is the *narrower*
        // build that wins the bigger split; a single pinned value can only ever
        // have been correct for one of them, and the 1.29b sweep was silently
        // reporting `failed=0` while its own assertion could not hold.
        let (expected_with, expected_without, build) =
            match std::env::var_os("PEACECRAFT3_LEGACY_DIR") {
                Some(_) => (433, 2, "1.29b"),
                None => (381, 2, "1.28c"),
            };
        assert_eq!(
            (with_footer, without_footer),
            (expected_with, expected_without),
            "footer split changed on {build}"
        );
        assert_eq!(fully_transparent, 39, "all-zero-alpha count changed");
    }
}

#[cfg(test)]
mod orientation_regressions {
    use super::*;

    #[test]
    fn raw_and_rle_honor_all_four_authored_origins() {
        // Four asymmetric synthetic pixels, arranged in top-left output order.
        let pixels = [[3u8, 2, 1], [6, 5, 4], [9, 8, 7], [12, 11, 10]];
        let expected = [1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255, 10, 11, 12, 255];
        for (descriptor, order) in [
            (0x20, [0, 1, 2, 3]),
            (0x30, [1, 0, 3, 2]),
            (0x00, [2, 3, 0, 1]),
            (0x10, [3, 2, 1, 0]),
        ] {
            for image_type in [2, 10] {
                let mut data = vec![0; HEADER_LEN];
                data[2] = image_type;
                data[12..14].copy_from_slice(&2u16.to_le_bytes());
                data[14..16].copy_from_slice(&2u16.to_le_bytes());
                data[16] = 24;
                data[17] = descriptor;
                // One four-pixel literal RLE packet crosses the row boundary.
                if image_type == 10 {
                    data.push(3);
                }
                for index in order {
                    data.extend(pixels[index]);
                }
                assert_eq!(
                    decode(&data).unwrap().1,
                    expected,
                    "type={image_type} descriptor={descriptor}"
                );
            }
        }
    }

    #[test]
    fn empty_right_origin_image_does_not_panic() {
        let mut data = vec![0; HEADER_LEN];
        data[2] = 2;
        data[16] = 24;
        data[17] = 0x10;
        assert!(decode(&data).unwrap().1.is_empty());
    }
}

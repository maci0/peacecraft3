//! Checked legacy 2D BC1/BC3 bytes only. See DDS.md for the exact profile.
use std::{io::Cursor, ops::Range};

use crate::LoadedTexture;

/// Caller budgets; no asset-size or device-dependent defaults are supplied.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub max_encoded_bytes: u64,
    pub max_rgba_bytes: u64,
    /// Logical decoder/conversion buffers, excluding input and allocator overhead.
    pub max_working_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Bc1,
    Bc3,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mip {
    pub width: u32,
    pub height: u32,
    pub bytes: Range<usize>,
}

/// Raw fields remain metadata, not a gamma, channel, shader or sampling policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub format: Format,
    pub width: u32,
    pub height: u32,
    pub header_flags: u32,
    pub pixel_flags: u32,
    pub caps: u32,
    pub authored_mip_count: u32,
    pub reserved: [u32; 11],
    pub mips: Vec<Mip>,
}

#[derive(Debug, Clone)]
pub struct Decoded {
    /// Top mip only; remaining authored mip spans are validated and retained.
    pub texture: LoadedTexture,
    pub profile: Profile,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{source_name} at {offset:#x}: {kind}")]
pub struct Error {
    pub source_name: String,
    pub offset: usize,
    pub kind: Failure,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Failure {
    #[error("malformed DDS: {0}")]
    Malformed(String),
    #[error("unsupported DDS: {0}")]
    Unsupported(String),
    #[error("{resource} needs {needed} bytes, caller limit {limit}")]
    Limits {
        resource: &'static str,
        needed: u64,
        limit: u64,
    },
    #[error("malformed DDS: arithmetic overflow in {0}")]
    Overflow(&'static str),
    #[error("decoder limit: {0}")]
    DecoderLimits(String),
}

pub(crate) fn failure(source: &str, offset: usize, kind: Failure) -> Error {
    Error {
        source_name: source.into(),
        offset,
        kind,
    }
}
pub(crate) fn malformed(source: &str, offset: usize, detail: &str) -> Error {
    failure(source, offset, Failure::Malformed(detail.into()))
}
pub(crate) fn unsupported(source: &str, offset: usize, detail: impl Into<String>) -> Error {
    failure(source, offset, Failure::Unsupported(detail.into()))
}
pub(crate) fn overflow(source: &str, detail: &'static str) -> Error {
    failure(source, 16, Failure::Overflow(detail))
}
pub(crate) fn limit(
    source: &str,
    resource: &'static str,
    needed: u64,
    maximum: u64,
) -> Result<(), Error> {
    if needed > maximum {
        return Err(failure(
            source,
            0,
            Failure::Limits {
                resource,
                needed,
                limit: maximum,
            },
        ));
    }
    Ok(())
}
pub(crate) fn word(header: &[u8; 128], at: usize) -> u32 {
    u32::from_le_bytes(header[at..at + 4].try_into().unwrap())
}

pub(crate) fn legacy_header<'a>(bytes: &'a [u8], source: &str) -> Result<&'a [u8; 128], Error> {
    let header: &[u8; 128] = bytes
        .get(..128)
        .and_then(|b| b.try_into().ok())
        .ok_or_else(|| malformed(source, 0, "truncated 128-byte legacy envelope"))?;
    if &header[..4] != b"DDS " {
        return Err(malformed(source, 0, "magic is not DDS "));
    }
    if word(header, 4) != 124 || word(header, 76) != 32 {
        return Err(malformed(
            source,
            4,
            "header/pixel-format sizes must be 124/32",
        ));
    }
    Ok(header)
}

pub(crate) fn single_2d(header: &[u8; 128], source: &str) -> Result<(u32, u32, u32), Error> {
    let flags = word(header, 8);
    let pixel_flags = word(header, 80);
    let caps = word(header, 108);
    if word(header, 24) != 0 || flags & 0x800000 != 0 || word(header, 112) != 0 {
        return Err(unsupported(
            source,
            24,
            "depth/cube/volume/other caps2 layouts",
        ));
    }
    if flags & 0x1007 != 0x1007 || pixel_flags & 4 == 0 || caps & 0x1000 == 0 {
        return Err(malformed(
            source,
            8,
            "missing required 2D/FourCC/texture flags",
        ));
    }
    if flags & !0xa1007 != 0
        || caps & !0x401008 != 0
        || [116, 120, 124].iter().any(|&at| word(header, at) != 0)
    {
        return Err(unsupported(
            source,
            8,
            "unknown header flags/caps/control fields",
        ));
    }
    Ok((flags, pixel_flags, caps))
}

pub(crate) fn mip_chain(
    bytes: &[u8],
    source: &str,
    header: &[u8; 128],
    block_size: u64,
    check_bc1_alpha: bool,
) -> Result<Vec<Mip>, Error> {
    let flags = word(header, 8);
    let caps = word(header, 108);
    let (width, height) = (word(header, 16), word(header, 12));
    if width == 0 || height == 0 {
        return Err(malformed(source, 12, "zero 2D extent"));
    }
    let authored_mip_count = word(header, 28);
    let levels = if flags & 0x20000 != 0 {
        if authored_mip_count == 0 {
            return Err(malformed(source, 28, "flagged mip count is zero"));
        }
        authored_mip_count
    } else {
        if authored_mip_count > 1 {
            return Err(malformed(
                source,
                28,
                "multiple mips without mip-count flag",
            ));
        }
        1
    };
    if levels > width.max(height).ilog2() + 1 {
        return Err(malformed(
            source,
            28,
            "mip count exceeds extent-derived chain",
        ));
    }
    if levels > 1 && caps & 0x400008 != 0x400008 {
        return Err(malformed(
            source,
            108,
            "multiple mips without complex/mipmap caps",
        ));
    }
    let mut mips = Vec::with_capacity(levels as usize);
    let mut start = 128usize;
    let (mut w, mut h) = (width, height);
    for _ in 0..levels {
        let size = u64::from(w.div_ceil(4))
            .checked_mul(u64::from(h.div_ceil(4)))
            .and_then(|n| n.checked_mul(block_size))
            .ok_or_else(|| overflow(source, "mip block bytes"))?;
        let size =
            usize::try_from(size).map_err(|_| overflow(source, "addressable mip byte count"))?;
        let end = start
            .checked_add(size)
            .ok_or_else(|| overflow(source, "mip end offset"))?;
        let body = bytes
            .get(start..end)
            .ok_or_else(|| malformed(source, start, "truncated authored mip span"))?;
        if check_bc1_alpha {
            for (index, block) in body.as_chunks::<8>().0.iter().enumerate() {
                let a = u16::from_le_bytes(block[..2].try_into().unwrap());
                let b = u16::from_le_bytes(block[2..4].try_into().unwrap());
                let selectors = u32::from_le_bytes(block[4..8].try_into().unwrap());
                if a <= b && (0..16).any(|lane| selectors >> (lane * 2) & 3 == 3) {
                    return Err(unsupported(
                        source,
                        start + index * 8,
                        "BC1 punch-through alpha; RGB decoder would discard it",
                    ));
                }
            }
        }
        mips.push(Mip {
            width: w,
            height: h,
            bytes: start..end,
        });
        start = end;
        w = (w / 2).max(1);
        h = (h / 2).max(1);
    }
    if start != bytes.len() {
        return Err(malformed(
            source,
            start,
            "bytes outside the complete authored mip chain",
        ));
    }
    if flags & 0x80000 != 0 && u64::from(word(header, 20)) != mips[0].bytes.len() as u64 {
        return Err(malformed(
            source,
            20,
            "linear size disagrees with top mip block span",
        ));
    }
    if flags & 0x80000 == 0 && word(header, 20) != 0 {
        return Err(unsupported(
            source,
            20,
            "nonzero unflagged pitch/linear-size semantics",
        ));
    }
    Ok(mips)
}

/// Validate all authored mip spans and BC1 alpha cases before decoding/allocation.
/// This is an explicit local import policy, not native malformed-file parity.
pub fn inspect(bytes: &[u8], source: &str, limits: Limits) -> Result<Profile, Error> {
    limit(
        source,
        "encoded input",
        bytes.len() as u64,
        limits.max_encoded_bytes,
    )?;
    let header = legacy_header(bytes, source)?;
    let fourcc = &header[84..88];
    let format = match fourcc {
        b"DXT1" => Format::Bc1,
        b"DXT5" => Format::Bc3,
        _ => {
            return Err(unsupported(
                source,
                84,
                format!("FourCC {fourcc:?}; only legacy DXT1/DXT5 are enabled"),
            ));
        }
    };
    let (flags, pixel_flags, caps) = single_2d(header, source)?;
    // DirectXTex documents NVTT's non-standard 0x40000000 marker. Retain it raw.
    let reserved = std::array::from_fn(|i| word(header, 32 + i * 4));
    if pixel_flags & !0x40000005 != 0
        || (pixel_flags & 0x40000000 != 0 && reserved[9] != u32::from_le_bytes(*b"NVTT"))
        || [88, 92, 96, 100, 104]
            .iter()
            .any(|&at| word(header, at) != 0)
    {
        return Err(unsupported(
            source,
            80,
            "unknown pixel flags/channel masks/swizzle profile",
        ));
    }
    let width = word(header, 16);
    let height = word(header, 12);
    if width == 0 || height == 0 {
        return Err(malformed(source, 12, "zero 2D extent"));
    }
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(|| overflow(source, "pixel count"))?;
    let rgba = pixels
        .checked_mul(4)
        .ok_or_else(|| overflow(source, "RGBA byte count"))?;
    limit(source, "RGBA output", rgba, limits.max_rgba_bytes)?;
    if !width.is_multiple_of(4) || !height.is_multiple_of(4) {
        return Err(unsupported(
            source,
            12,
            "image DDS top-level dimensions must be multiples of four; no padding inferred",
        ));
    }
    let block_size = if format == Format::Bc1 { 8u64 } else { 16 };
    let native = pixels
        .checked_mul(if format == Format::Bc1 { 3 } else { 4 })
        .ok_or_else(|| overflow(source, "native pixel buffer"))?;
    let encoded_row = u64::from(width / 4)
        .checked_mul(block_size)
        .ok_or_else(|| overflow(source, "decoder block row"))?;
    let decode_peak = native
        .checked_add(encoded_row)
        .ok_or_else(|| overflow(source, "decoder working bytes"))?;
    let conversion_peak = if format == Format::Bc1 {
        native
            .checked_add(rgba)
            .ok_or_else(|| overflow(source, "RGB to RGBA working bytes"))?
    } else {
        rgba
    };
    limit(
        source,
        "decoder/conversion working buffers",
        decode_peak.max(conversion_peak),
        limits.max_working_bytes,
    )?;
    let rgba_len =
        usize::try_from(rgba).map_err(|_| overflow(source, "addressable RGBA byte count"))?;
    // RGB-to-RGBA conversion allocates a Vec; fitting usize alone is insufficient.
    std::alloc::Layout::array::<u8>(rgba_len)
        .map_err(|_| overflow(source, "RGBA vector layout"))?;
    let authored_mip_count = word(header, 28);
    let mips = mip_chain(bytes, source, header, block_size, format == Format::Bc1)?;
    Ok(Profile {
        format,
        width,
        height,
        header_flags: flags,
        pixel_flags,
        caps,
        authored_mip_count,
        reserved,
        mips,
    })
}

/// Caller-supplied DDS bytes and budgets, producing top-mip RGBA8 without I/O.
/// No model binding, material interpretation, color-space conversion or render claim.
pub fn decode(bytes: &[u8], source: &str, limits: Limits) -> Result<Decoded, Error> {
    let profile = inspect(bytes, source, limits)?;
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), image::ImageFormat::Dds);
    let mut decoder_limits = image::Limits::default();
    decoder_limits.max_image_width = Some(profile.width);
    decoder_limits.max_image_height = Some(profile.height);
    decoder_limits.max_alloc = Some(limits.max_working_bytes);
    reader.limits(decoder_limits);
    let decoded = reader
        .decode()
        .map_err(|e| {
            let kind = match e {
                image::ImageError::Unsupported(_) => Failure::Unsupported(e.to_string()),
                image::ImageError::Limits(_) => Failure::DecoderLimits(e.to_string()),
                _ => Failure::Malformed(e.to_string()),
            };
            failure(source, 128, kind)
        })?
        .into_rgba8();
    if decoded.width() != profile.width || decoded.height() != profile.height {
        return Err(malformed(
            source,
            12,
            "decoder dimensions disagree with checked header",
        ));
    }
    Ok(Decoded {
        texture: LoadedTexture {
            width: profile.width,
            height: profile.height,
            rgba: decoded.into_raw(),
            source: source.into(),
        },
        profile,
    })
}

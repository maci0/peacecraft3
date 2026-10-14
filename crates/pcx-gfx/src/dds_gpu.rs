//! Device-gated, checked legacy ATI2 storage. No channel or material interpretation.
//! See DDS_GPU.md for framing, allocation accounting and evidence limits.
use std::alloc::Layout;

use bevy::{
    asset::RenderAssetUsages,
    image::{CompressedImageFormats, Image, ImageFormat, ImageSampler, ImageType},
    render::{
        render_resource::{TextureDimension, TextureFormat, WgpuFeatures},
        renderer::RenderDevice,
    },
};

use crate::dds::{self, Mip, limit, overflow, unsupported, word};

/// Explicit caller budgets, excluding the borrowed input and allocator overhead.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub max_encoded_bytes: u64,
    pub max_payload_bytes: u64,
    /// Conservative reader capacity, mip metadata and retained source label.
    pub max_working_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub source: String,
    /// Verbatim source envelope, including all opaque private/reserved fields.
    pub raw_header: [u8; 128],
    pub width: u32,
    pub height: u32,
    /// Offsets into the caller's original DDS bytes, not into `Image::data`.
    pub mips: Vec<Mip>,
}

#[derive(Debug)]
pub struct Imported {
    pub profile: Profile,
    /// BC5 RG UNORM blocks and all authored mips, never expanded RGBA.
    pub image: Image,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Profile(#[from] dds::Error),
    #[error("{source_name}: enabled device features {enabled:?} lack TEXTURE_COMPRESSION_BC")]
    Capability {
        source_name: String,
        enabled: WgpuFeatures,
    },
    #[error("{source_name}: {width}x{height} exceeds device 2D extent limit {maximum}")]
    Extent {
        source_name: String,
        width: u32,
        height: u32,
        maximum: u32,
    },
    #[error("{source_name}: compressed DDS reader refused: {detail}")]
    Reader { source_name: String, detail: String },
}

fn layout(source: &str, bytes: u64) -> Result<(), dds::Error> {
    let count = usize::try_from(bytes)
        .map_err(|_| overflow(source, "addressable compressed allocation"))?;
    Layout::array::<u8>(count).map_err(|_| overflow(source, "compressed vector layout"))?;
    Ok(())
}

/// Pure framing validation; success makes no device-capability claim.
pub fn inspect(bytes: &[u8], source: &str, limits: Limits) -> Result<Profile, dds::Error> {
    limit(
        source,
        "encoded input",
        bytes.len() as u64,
        limits.max_encoded_bytes,
    )?;
    let header = dds::legacy_header(bytes, source)?;
    if &header[84..88] != b"ATI2" {
        return Err(unsupported(
            source,
            84,
            "only legacy ATI2/BC5_UNORM storage is supported",
        ));
    }
    let (_, pixel_flags, _) = dds::single_2d(header, source)?;
    // DirectXTex documents these two high bits as non-storage hints. Retain
    // them without interpreting them or requiring a particular vendor marker.
    if pixel_flags & !0xc0000004 != 0 {
        return Err(unsupported(
            source,
            80,
            "unknown or conflicting pixel-format storage flags",
        ));
    }
    let (width, height) = (word(header, 16), word(header, 12));
    if width == 0 || height == 0 {
        return Err(dds::malformed(source, 12, "zero 2D extent"));
    }
    if !width.is_multiple_of(4) || !height.is_multiple_of(4) {
        return Err(unsupported(
            source,
            12,
            "top extent would require Bevy block-size padding",
        ));
    }
    // Check theoretical top storage even for a truncated huge input.
    let top = u64::from(width.div_ceil(4))
        .checked_mul(u64::from(height.div_ceil(4)))
        .and_then(|n| n.checked_mul(16))
        .ok_or_else(|| overflow(source, "BC5 top block span"))?;
    layout(source, top)?;
    let payload = (bytes.len() - 128) as u64;
    limit(
        source,
        "compressed payload",
        payload,
        limits.max_payload_bytes,
    )?;
    layout(source, payload)?;
    // u32 extents permit at most u32::BITS mip records. Check this small
    // metadata ceiling before the common parser allocates its mip vector.
    Layout::array::<Mip>(u32::BITS as usize)
        .map_err(|_| overflow(source, "mip metadata vector layout"))?;
    let metadata = u64::from(u32::BITS) * std::mem::size_of::<Mip>() as u64;
    // ddsfile reads a Cursor into one Vec; allow geometric growth plus the
    // reader's small initial buffer. No BC5 transcoding/conversion is invoked.
    let reader_capacity = payload
        .checked_mul(2)
        .and_then(|n| n.checked_add(32))
        .ok_or_else(|| overflow(source, "compressed reader capacity"))?;
    layout(source, reader_capacity)?;
    layout(source, source.len() as u64)?;
    let working = reader_capacity
        .checked_add(metadata)
        .and_then(|n| n.checked_add(source.len() as u64))
        .ok_or_else(|| overflow(source, "compressed working buffers"))?;
    limit(
        source,
        "compressed working buffers",
        working,
        limits.max_working_bytes,
    )?;
    let mips = dds::mip_chain(bytes, source, header, 16, false)?;
    Ok(Profile {
        source: source.into(),
        raw_header: *header,
        width,
        height,
        mips,
    })
}

/// Uses the actual device's ENABLED features and limits. Sampler and asset
/// residency are caller policy; no shader, material or native binding is supplied.
pub fn import(
    bytes: &[u8],
    source: &str,
    device: &RenderDevice,
    limits: Limits,
    sampler: ImageSampler,
    asset_usage: RenderAssetUsages,
) -> Result<Imported, Error> {
    import_enabled(
        bytes,
        source,
        device.features(),
        device.limits().max_texture_dimension_2d,
        limits,
        sampler,
        asset_usage,
    )
}

fn import_enabled(
    bytes: &[u8],
    source: &str,
    enabled: WgpuFeatures,
    maximum: u32,
    limits: Limits,
    sampler: ImageSampler,
    asset_usage: RenderAssetUsages,
) -> Result<Imported, Error> {
    let profile = inspect(bytes, source, limits)?;
    if !enabled.contains(WgpuFeatures::TEXTURE_COMPRESSION_BC) {
        return Err(Error::Capability {
            source_name: source.into(),
            enabled,
        });
    }
    if profile.width > maximum || profile.height > maximum {
        return Err(Error::Extent {
            source_name: source.into(),
            width: profile.width,
            height: profile.height,
            maximum,
        });
    }
    // BC5 has no sRGB storage variant. This argument selects no native colour policy.
    let image = Image::from_buffer(
        bytes,
        ImageType::Format(ImageFormat::Dds),
        CompressedImageFormats::from_features(enabled),
        false,
        sampler,
        asset_usage,
    )
    .map_err(|e| Error::Reader {
        source_name: source.into(),
        detail: e.to_string(),
    })?;
    let descriptor = &image.texture_descriptor;
    if descriptor.format != TextureFormat::Bc5RgUnorm
        || descriptor.dimension != TextureDimension::D2
        || descriptor.size.width != profile.width
        || descriptor.size.height != profile.height
        || descriptor.size.depth_or_array_layers != 1
        || descriptor.mip_level_count as usize != profile.mips.len()
        || image.data.as_deref() != Some(&bytes[128..])
        || image
            .data
            .as_ref()
            .is_none_or(|data| data.capacity() as u64 > (bytes.len() - 128) as u64 * 2 + 32)
    {
        return Err(Error::Reader {
            source_name: source.into(),
            detail: "reader changed checked storage, extent, mips, payload or capacity bound"
                .into(),
        });
    }
    Ok(Imported { profile, image })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(bytes: &mut [u8], at: usize, value: u32) {
        bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    fn fixture() -> Vec<u8> {
        let mut bytes = vec![0; 176];
        bytes[..4].copy_from_slice(b"DDS ");
        for (at, n) in [
            (4, 124),
            (8, 0xa1007),
            (12, 4),
            (16, 8),
            (20, 32),
            (28, 2),
            (76, 32),
            (80, 4),
            (108, 0x401008),
        ] {
            put(&mut bytes, at, n);
        }
        bytes[84..88].copy_from_slice(b"ATI2");
        for (i, b) in bytes[128..].iter_mut().enumerate() {
            *b = i as u8;
        }
        bytes
    }
    fn budgets() -> Limits {
        Limits {
            max_encoded_bytes: 4096,
            max_payload_bytes: 4096,
            max_working_bytes: 8192,
        }
    }
    fn policy(bytes: &[u8], features: WgpuFeatures, maximum: u32) -> Result<Imported, Error> {
        import_enabled(
            bytes,
            "synthetic test policy",
            features,
            maximum,
            budgets(),
            ImageSampler::nearest(),
            RenderAssetUsages::MAIN_WORLD,
        )
    }
    #[test]
    fn test_policy_enabled_bc_keeps_raw_metadata_and_payload() {
        let mut bytes = fixture();
        // Arbitrary synthetic private words: no measured vendor signature whitelist.
        for at in (32..76).step_by(4).chain((88..108).step_by(4)) {
            put(&mut bytes, at, 0x12345678 + at as u32);
        }
        put(&mut bytes, 80, 0xc0000004);
        let imported = policy(&bytes, WgpuFeatures::TEXTURE_COMPRESSION_BC, 8).unwrap();
        assert_eq!(imported.profile.raw_header.as_slice(), &bytes[..128]);
        assert_eq!(
            imported
                .profile
                .mips
                .iter()
                .map(|m| m.bytes.clone())
                .collect::<Vec<_>>(),
            vec![128..160, 160..176]
        );
        assert_eq!(imported.image.data.as_deref(), Some(&bytes[128..]));
        assert_eq!(
            imported.image.texture_descriptor.format,
            TextureFormat::Bc5RgUnorm
        );
        assert_eq!(imported.image.asset_usage, RenderAssetUsages::MAIN_WORLD);
        assert!(matches!(
            imported.image.sampler,
            ImageSampler::Descriptor(_)
        ));
    }
    #[test]
    fn test_policy_missing_enabled_feature_and_extent_are_typed() {
        assert!(matches!(
            policy(&fixture(), WgpuFeatures::empty(), 8),
            Err(Error::Capability { .. })
        ));
        assert!(matches!(
            policy(&fixture(), WgpuFeatures::TEXTURE_COMPRESSION_BC, 4),
            Err(Error::Extent { .. })
        ));
    }
    #[test]
    fn truncation_wrong_length_and_bad_mips_are_malformed() {
        let bytes = fixture();
        for n in [0, 127, 128, 175] {
            assert!(matches!(
                inspect(&bytes[..n], "truncated", budgets())
                    .unwrap_err()
                    .kind,
                dds::Failure::Malformed(_)
            ));
        }
        let mut extra = bytes.clone();
        extra.push(0);
        assert!(matches!(
            inspect(&extra, "extra", budgets()).unwrap_err().kind,
            dds::Failure::Malformed(_)
        ));
        for (at, n) in [
            (4, 123),
            (76, 31),
            (28, 0),
            (28, 5),
            (20, 31),
            (108, 0x1000),
            (12, 0),
        ] {
            let mut b = bytes.clone();
            put(&mut b, at, n);
            assert!(
                matches!(
                    inspect(&b, "bad mip", budgets()).unwrap_err().kind,
                    dds::Failure::Malformed(_)
                ),
                "offset {at}"
            );
        }
    }
    #[test]
    fn storage_controls_other_profiles_and_padding_are_refused() {
        for fourcc in [*b"DX10", *b"DXT1", *b"BC5S", *b"????"] {
            let mut b = fixture();
            b[84..88].copy_from_slice(&fourcc);
            assert!(matches!(
                inspect(&b, "profile", budgets()).unwrap_err().kind,
                dds::Failure::Unsupported(_)
            ));
        }
        for (at, n) in [
            (24, 1),
            (112, 0x200),
            (112, 0x200000),
            (116, 1),
            (120, 1),
            (124, 1),
            (8, 0xa1017),
            (80, 0x44),
            (16, 7),
        ] {
            let mut b = fixture();
            put(&mut b, at, n);
            assert!(
                matches!(
                    inspect(&b, "control", budgets()).unwrap_err().kind,
                    dds::Failure::Unsupported(_)
                ),
                "offset {at}"
            );
        }
    }
    #[test]
    fn caller_budgets_and_vector_layout_are_checked() {
        for limits in [
            Limits {
                max_encoded_bytes: 175,
                ..budgets()
            },
            Limits {
                max_payload_bytes: 47,
                ..budgets()
            },
            Limits {
                max_working_bytes: 0,
                ..budgets()
            },
        ] {
            assert!(matches!(
                inspect(&fixture(), "budget", limits).unwrap_err().kind,
                dds::Failure::Limits { .. }
            ));
        }
        let mut b = fixture();
        put(&mut b, 16, u32::MAX - 3);
        put(&mut b, 12, u32::MAX - 3);
        assert!(matches!(
            inspect(&b, "layout", budgets()).unwrap_err().kind,
            dds::Failure::Overflow(_)
        ));
    }
}

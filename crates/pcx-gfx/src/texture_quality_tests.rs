// Texture Quality policy and consumer tests (headless). Fixture mirrors
// `image_mip_tests::fixture` but imported so both stay in sync.
use crate::texture_quality::{PolicyError, texture_quality_for_ordinal};
use crate::{LoadedTexture, one_mip};
use pcx_blp::{PALETTE_BASE, MIP_BASE};

fn fixture() -> Vec<u8> {
    let mut data = vec![0; PALETTE_BASE + 256 * 4];
    data[..4].copy_from_slice(b"BLP1");
    for (offset, value) in [(4, 1u32), (8, 8), (12, 4), (16, 2), (20, 4), (24, 1)] {
        data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    for (level, (count, rgba)) in [
        (8, [11, 37, 83, 149]),
        (2, [17, 61, 127, 173]),
        (1, [23, 67, 131, 197]),
    ]
    .into_iter()
    .enumerate()
    {
        let palette = PALETTE_BASE + level * 4;
        data[palette..palette + 4].copy_from_slice(&[rgba[2], rgba[1], rgba[0], 0]);
        let offset = MIP_BASE + level * 4;
        let start = data.len() as u32;
        data[offset..offset + 4].copy_from_slice(&start.to_le_bytes());
        data[offset + 64..offset + 68].copy_from_slice(&(count as u32 * 2).to_le_bytes());
        data.extend(std::iter::repeat_n(level as u8, count));
        data.extend(std::iter::repeat_n(rgba[3], count));
    }
    data
}

#[test]
fn nonstock_ordinal_mapping_follows_native_branches() {
    // Derived from the write-routine branch chains, not a lookup table:
    // q=0 -> (mip 1, depth 0x10); q=1,2 -> (mip 0, depth 0x10);
    // q=3 -> (mip 0, depth 0x20).
    let q0 = texture_quality_for_ordinal(0).unwrap();
    assert_eq!(q0.mip_level, 1);
    assert_eq!(q0.color_depth_bytes, 4);
    assert_eq!(q0.texcolordepth_word, 0x10);

    for q in [1, 2] {
        let v = texture_quality_for_ordinal(q).unwrap();
        assert_eq!(v.mip_level, 0, "q={q}");
        assert_eq!(v.color_depth_bytes, 4, "q={q}");
        assert_eq!(v.texcolordepth_word, 0x10, "q={q}");
    }

    let q3 = texture_quality_for_ordinal(3).unwrap();
    assert_eq!(q3.mip_level, 0);
    assert_eq!(q3.color_depth_bytes, 8);
    assert_eq!(q3.texcolordepth_word, 0x20);
}

#[test]
fn out_of_range_ordinals_refuse() {
    for bad in [4, 5, 100, u32::MAX] {
        assert_eq!(
            texture_quality_for_ordinal(bad),
            Err(PolicyError::OrdinalOutOfRange { ordinal: bad })
        );
    }
}

#[test]
fn quality_selects_chosen_mip_and_preserves_depth() {
    let data = fixture();
    // q=1 picks the top mip (level 0): the fixture header declares 4x2 at
    // level 0, so 8 pixels of [11,37,83,149].
    let top = one_mip(&data, "q.blp", &texture_quality_for_ordinal(1).unwrap(), 64).unwrap();
    assert_eq!(top.width, 4, "top {:?}", top.width);
    assert_eq!(top.height, 2, "top {:?}", top.height);
    assert_eq!(top.rgba.len(), 4 * 2 * 4, "top len {}", top.rgba.len());
    assert_eq!(top.rgba, [11, 37, 83, 149].repeat(8));
    assert!(top.mipmaps.is_empty());

    // q=0 picks authored mip level 1 with its own pixels and dimensions.
    let lower = one_mip(&data, "q.blp", &texture_quality_for_ordinal(0).unwrap(), 64).unwrap();
    assert_eq!(lower.width, 2, "lower {:?}", lower.width);
    assert_eq!(lower.height, 1, "lower {:?}", lower.height);
    assert_eq!(lower.rgba.len(), 2 * 1 * 4);
    assert_eq!(lower.rgba, [17, 61, 127, 173].repeat(2));
    assert!(lower.mipmaps.is_empty());

    // A LoadedTexture that is not a quality-selected load is untouched by
    // this path: the unselected call shape stays the base-only loader.
    let keep: LoadedTexture = top;
    assert_eq!(keep.width, 4);
}

#[test]
fn absent_or_invalid_mip_refuses_without_fallback() {
    let data = fixture();
    // Manually request an absent level beyond the authored chain.
    let bad = crate::TextureQuality {
        mip_level: 5,
        color_depth_bytes: 4,
        texcolordepth_word: 0x10,
    };
    let err = one_mip(&data, "absent.blp", &bad, 64).unwrap_err();
    assert!(err.to_string().contains("absent mip level 5"), "{err}");

    // Zeroed mip_sizes entry for an in-range level is also a refusal.
    let mut gap = data.clone();
    gap[MIP_BASE + 8..MIP_BASE + 12].fill(0);
    let zero = crate::TextureQuality {
        mip_level: 2,
        color_depth_bytes: 4,
        texcolordepth_word: 0x10,
    };
    assert!(one_mip(&gap, "gap.blp", &zero, 64).is_err());

    // Budget is enforced before selection allocation.
    assert!(one_mip(&data, "budget.blp", &texture_quality_for_ordinal(1).unwrap(), 10).is_err());
}

#[test]
fn depth_beyond_rgba_refuses() {
    let data = fixture();
    let q3 = texture_quality_for_ordinal(3).unwrap();
    assert_eq!(q3.color_depth_bytes, 8);
    // 8 bytes/pixel has no RGBA8 store; the consumer refuses fail-closed.
    assert!(one_mip(&data, "q.blp", &q3, 64).is_err());
}

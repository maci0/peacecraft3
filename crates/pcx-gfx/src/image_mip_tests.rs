use super::*;

fn fixture() -> Vec<u8> {
    let mut data = vec![0; pcx_blp::PALETTE_BASE + 256 * 4];
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
        let palette = pcx_blp::PALETTE_BASE + level * 4;
        data[palette..palette + 4].copy_from_slice(&[rgba[2], rgba[1], rgba[0], 0]);
        let offset = pcx_blp::MIP_BASE + level * 4;
        let start = data.len() as u32;
        data[offset..offset + 4].copy_from_slice(&start.to_le_bytes());
        data[offset + 64..offset + 68].copy_from_slice(&(count as u32 * 2).to_le_bytes());
        data.extend(std::iter::repeat_n(level as u8, count));
        data.extend(std::iter::repeat_n(rgba[3], count));
    }
    data
}

#[test]
fn authored_mips_keep_their_own_pixels_alpha_and_gpu_order() {
    let source = fixture();
    let texture = load_mipmapped_image_bounded(&source, "synthetic.blp", 44).unwrap();
    assert_eq!(texture.rgba_bytes(), Some(44));
    assert_eq!(texture.rgba, [11, 37, 83, 149].repeat(8));
    assert_eq!(
        texture.mipmaps,
        vec![[17, 61, 127, 173].repeat(2), vec![23, 67, 131, 197]]
    );
    let image = viewer::rgba_image(&texture);
    assert_eq!(image.texture_descriptor.mip_level_count, 3);
    assert_eq!(
        image.data.unwrap(),
        texture
            .rgba
            .iter()
            .chain(texture.mipmaps.iter().flatten())
            .copied()
            .collect::<Vec<_>>()
    );
    // Base-only consumers keep their existing allocation contract.
    let base = load_image_bounded(&source, "synthetic.blp", 32).unwrap();
    assert!(base.mipmaps.is_empty());
    assert_eq!(base.rgba, texture.rgba);
}

#[test]
fn complete_mip_budget_and_malformed_lower_levels_refuse() {
    let data = fixture();
    let error = load_mipmapped_image_bounded(&data, "budget.blp", 43)
        .unwrap_err()
        .to_string();
    assert!(error.contains("budget.blp") && error.contains("budget"));
    let mut gap = data.clone();
    gap[pcx_blp::MIP_BASE + 68..pcx_blp::MIP_BASE + 72].fill(0);
    assert!(
        load_mipmapped_image_bounded(&gap, "gap.blp", 44)
            .unwrap_err()
            .to_string()
            .contains("noncontiguous")
    );
    assert!(load_mipmapped_image_bounded(&data[..data.len() - 1], "truncated.blp", 44).is_err());
    assert_eq!(rgba_mip_bytes(2, 1, &[0; 8], &[vec![0; 4]]), Some(12));
    assert_eq!(rgba_mip_bytes(2, 1, &[0; 8], &[vec![0; 3]]), None);
    assert_eq!(rgba_mip_bytes(1, 1, &[0; 4], &[vec![0; 4]]), None);
    assert_eq!(rgba_mip_bytes(0, 1, &[], &[]), None);
    assert_eq!(rgba_mip_bytes(u32::MAX, u32::MAX, &[], &[]), None);
}

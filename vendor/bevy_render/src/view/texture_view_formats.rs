// WebGL renders the main format directly and cannot declare alternate view formats.
fn main_texture_view_formats(
    format: TextureFormat,
    supports_view_formats: bool,
) -> &'static [TextureFormat] {
    if !supports_view_formats {
        return &[];
    }
    match format {
        TextureFormat::Bgra8Unorm => &[TextureFormat::Bgra8UnormSrgb],
        TextureFormat::Rgba8Unorm => &[TextureFormat::Rgba8UnormSrgb],
        _ => &[],
    }
}

#[cfg(test)]
mod view_format_tests {
    use super::*;

    #[test]
    fn alternate_main_texture_formats_require_adapter_support() {
        for (base, alternate) in [
            (TextureFormat::Bgra8Unorm, TextureFormat::Bgra8UnormSrgb),
            (TextureFormat::Rgba8Unorm, TextureFormat::Rgba8UnormSrgb),
        ] {
            assert!(main_texture_view_formats(base, false).is_empty());
            assert_eq!(main_texture_view_formats(base, true), &[alternate]);
        }
        assert!(main_texture_view_formats(TextureFormat::Rgba16Float, true).is_empty());
        assert!(main_texture_view_formats(TextureFormat::Rgba8UnormSrgb, true).is_empty());
    }
}

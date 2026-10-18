//! Original-menu working channels stay numeric through sampling and blending.
//! The final blit decodes that buffer once for the display's sRGB attachment.
use bevy::{
    camera::CompositingSpace,
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems,
        camera::sort_cameras,
        extract_component::{ExtractComponent, ExtractComponentPlugin},
        render_resource::TextureFormat,
        view::{ResolveCompositingSpaces, ResolvedCompositingSpace},
    },
};

/// Only cameras whose geometry, particles and UI all use menu working channels.
#[derive(Component, Clone, ExtractComponent)]
#[extract_app(RenderApp)]
#[require(CompositingSpace = CompositingSpace::Srgb)]
pub(crate) struct MenuColourCamera;

pub(crate) struct MenuCompositionPlugin;

impl Plugin for MenuCompositionPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ExtractComponentPlugin::<MenuColourCamera>::default());
        if let Some(render) = app.get_sub_app_mut(RenderApp) {
            render.add_systems(
                Render,
                (
                    clear_request
                        .in_set(RenderSystems::CreateViews)
                        .after(sort_cameras)
                        .before(ResolveCompositingSpaces),
                    resolve_menu
                        .in_set(RenderSystems::CreateViews)
                        .after(ResolveCompositingSpaces),
                ),
            );
        }
    }
}

// The built-in resolver assumes ordinary linear 3D inputs and rejects Camera3d.
// This separate path has raw working inputs throughout, including ordinary UI.
fn clear_request(mut views: Query<&mut ResolvedCompositingSpace, With<MenuColourCamera>>) {
    for mut space in &mut views {
        space.0 = None;
    }
}

fn resolve_menu(mut views: Query<&mut ResolvedCompositingSpace, With<MenuColourCamera>>) {
    for mut space in &mut views {
        space.0 = Some(CompositingSpace::Srgb);
    }
}

/// Bevy uploads these numbers directly; no sRGB-to-linear conversion before blending.
pub(crate) fn colour([r, g, b, a]: [f32; 4]) -> Color {
    Color::linear_rgba(r, g, b, a)
}

pub(crate) fn image(texture: &pcx_gfx::LoadedTexture) -> Image {
    let mut image = pcx_gfx::viewer::rgba_image(texture);
    image.texture_descriptor.format = TextureFormat::Rgba8Unorm;
    image
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn working_channels_and_source_alpha_are_not_gamma_converted() {
        let source = [0.13, 0.29, 0.47, 0.61];
        let uploaded = colour(source).to_linear();
        assert_eq!(uploaded.to_f32_array(), source);
        let texture = pcx_gfx::LoadedTexture {
            source: "synthetic.tga".into(),
            width: 1,
            height: 1,
            rgba: vec![31, 79, 157, 109],
        };
        let image = image(&texture);
        assert_eq!(image.texture_descriptor.format, TextureFormat::Rgba8Unorm);
        assert_eq!(image.data.as_deref(), Some(texture.rgba.as_slice()));
    }
}

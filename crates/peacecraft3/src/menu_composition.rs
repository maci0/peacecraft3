//! Original-menu working channels stay numeric through sampling and blending.
//! The final blit decodes that buffer once for the display's sRGB attachment.
use bevy::{
    camera::CompositingSpace,
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems,
        camera::sort_cameras,
        extract_component::{ExtractComponent, ExtractComponentPlugin},
        render_resource::{AsBindGroup, TextureFormat},
        view::{ResolveCompositingSpaces, ResolvedCompositingSpace},
    },
    shader::ShaderRef,
};

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(crate) struct RenderedImage {
    #[texture(0)]
    pub(crate) image: Handle<Image>,
    #[uniform(1)]
    pub(crate) factor: u32,
}
impl UiMaterial for RenderedImage {
    fn fragment_shader() -> ShaderRef {
        "embedded://peacecraft3/internal_resolution.wgsl".into()
    }
}

/// Only cameras whose geometry, particles and UI all use menu working channels.
#[derive(Component, Clone, ExtractComponent)]
#[extract_app(RenderApp)]
#[require(CompositingSpace = CompositingSpace::Srgb)]
pub(crate) struct MenuColourCamera;

pub(crate) struct MenuCompositionPlugin;

impl Plugin for MenuCompositionPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "internal_resolution.wgsl");
        app.add_plugins((
            ExtractComponentPlugin::<MenuColourCamera>::default(),
            UiMaterialPlugin::<RenderedImage>::default(),
        ))
        .add_systems(
            PostUpdate,
            bind_world_channels.before(bevy::camera::CameraUpdateSystems),
        );
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

// Campaign models, ground textures and HUD inputs share numeric source channels.
type UnboundWorldCamera = (
    Without<MenuColourCamera>,
    Or<(
        With<peacecraft3::campaign_camera::CampaignSceneCamera>,
        With<crate::campaign_actors::sky::SkyCamera>,
        With<peacecraft3::campaign_presentation::CampaignUiCamera>,
    )>,
);
fn bind_world_channels(mut commands: Commands, cameras: Query<Entity, UnboundWorldCamera>) {
    for entity in &cameras {
        commands.entity(entity).insert((
            MenuColourCamera,
            bevy::core_pipeline::tonemapping::Tonemapping::None,
            bevy::core_pipeline::tonemapping::DebandDither::Disabled,
        ));
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

/// Byte-valued source colours retain numeric channels, including alpha.
pub(crate) fn colour_u8(r: u8, g: u8, b: u8, a: u8) -> Color {
    colour([r, g, b, a].map(|value| f32::from(value) / f32::from(u8::MAX)))
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
    fn text_defaults_to_unhinted_but_preserves_an_explicit_override() {
        use bevy::text::FontHinting;
        let mut world = World::new();
        let default = world.spawn(Text::new("default")).id();
        assert_eq!(
            world.get::<FontHinting>(default),
            Some(&FontHinting::Disabled)
        );
        let explicit = world
            .spawn((Text::new("explicit"), FontHinting::Enabled))
            .id();
        assert_eq!(
            world.get::<FontHinting>(explicit),
            Some(&FontHinting::Enabled)
        );
    }

    #[test]
    fn world_and_sky_bind_source_channels_without_changing_other_cameras() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let scene = world
            .spawn(peacecraft3::campaign_camera::CampaignSceneCamera)
            .id();
        let sky = world.spawn(crate::campaign_actors::sky::SkyCamera).id();
        let hud = world
            .spawn(peacecraft3::campaign_presentation::CampaignUiCamera)
            .id();
        let other = world.spawn_empty().id();
        world.run_system_once(bind_world_channels).unwrap();
        for entity in [scene, sky, hud] {
            assert!(world.get::<MenuColourCamera>(entity).is_some());
            assert_eq!(
                world.get::<CompositingSpace>(entity),
                Some(&CompositingSpace::Srgb)
            );
            assert_eq!(
                world.get::<bevy::core_pipeline::tonemapping::Tonemapping>(entity),
                Some(&bevy::core_pipeline::tonemapping::Tonemapping::None)
            );
        }
        assert!(world.get::<MenuColourCamera>(other).is_none());
    }

    #[test]
    fn working_channels_and_source_alpha_are_not_gamma_converted() {
        let source = [0.13, 0.29, 0.47, 0.61];
        let uploaded = colour(source).to_linear();
        assert_eq!(uploaded.to_f32_array(), source);
        let rgba = [31, 79, 157, 109];
        assert_eq!(
            colour_u8(rgba[0], rgba[1], rgba[2], rgba[3])
                .to_linear()
                .to_f32_array(),
            rgba.map(|channel| f32::from(channel) / f32::from(u8::MAX))
        );
        let texture = pcx_gfx::LoadedTexture {
            mipmaps: Vec::new(),
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

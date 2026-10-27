//! Launcher-only supersampling; original UI cameras retain their display targets.
use crate::campaign_actors::sky::SkyCamera;
use bevy::{
    camera::{CameraUpdateSystems, ImageRenderTarget, RenderTarget, visibility::RenderLayers},
    ecs::system::SystemParam,
    prelude::*,
    render::{
        render_resource::{AsBindGroup, Extent3d, TextureDimension, TextureFormat, TextureUsages},
        renderer::RenderDevice,
    },
    shader::ShaderRef,
    window::{PrimaryWindow, WindowRef},
};
use peacecraft3::campaign_camera::CampaignSceneCamera;

#[derive(Resource)]
pub(crate) struct InternalResolution {
    pub(crate) factor: u32,
    pub(crate) error: Option<String>,
    pass: Option<Pass>,
    clear: Option<Entity>,
}
impl Default for InternalResolution {
    fn default() -> Self {
        Self {
            factor: 1,
            error: None,
            pass: None,
            clear: None,
        }
    }
}
struct Pass {
    scene: Entity,
    camera: Entity,
    root: Entity,
    image: Handle<Image>,
    material: Handle<Downsample>,
    output: RenderTarget,
    viewport: Option<bevy::camera::Viewport>,
    order: isize,
    size: UVec2,
    scale: f32,
    factor: u32,
}
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct Downsample {
    #[texture(0)]
    image: Handle<Image>,
    #[uniform(1)]
    factor: u32,
}
impl UiMaterial for Downsample {
    fn fragment_shader() -> ShaderRef {
        "embedded://peacecraft3/internal_resolution.wgsl".into()
    }
}

pub(crate) fn install(app: &mut App) {
    bevy::asset::embedded_asset!(app, "internal_resolution.wgsl");
    app.init_resource::<InternalResolution>()
        .add_plugins(UiMaterialPlugin::<Downsample>::default())
        .add_systems(PostUpdate, sync.before(CameraUpdateSystems));
}

// Application allocation ceiling, independent of original game content.
fn extent(size: UVec2, factor: u32, limit: u32) -> Result<UVec2, String> {
    if !(1..=4).contains(&factor) {
        return Err("Internal resolution must be 1×, 2×, 3× or 4×.".into());
    }
    let result = size
        .checked_mul(UVec2::splat(factor))
        .ok_or("Internal resolution dimensions overflow.")?;
    if result.min_element() == 0 || result.max_element() > limit {
        return Err(format!(
            "{factor}× exceeds the GPU texture limit ({limit} pixels per side). Using 1×."
        ));
    }
    if u64::from(result.x) * u64::from(result.y) > 64 * 1024 * 1024 {
        return Err(format!(
            "{factor}× exceeds the internal render-target budget (64 megapixels). Using 1×."
        ));
    }
    Ok(result)
}

#[derive(SystemParam)]
struct Targets<'w, 's> {
    windows: Query<'w, 's, (Entity, &'static Window)>,
    primary: Query<'w, 's, Entity, With<PrimaryWindow>>,
    images: ResMut<'w, Assets<Image>>,
    materials: ResMut<'w, Assets<Downsample>>,
    device: Option<Res<'w, RenderDevice>>,
}
impl Targets<'_, '_> {
    fn size(&self, target: &RenderTarget) -> Option<(UVec2, f32)> {
        match target {
            RenderTarget::Window(reference) => {
                let entity = match reference {
                    WindowRef::Primary => self.primary.single().ok()?,
                    WindowRef::Entity(entity) => *entity,
                };
                let (_, window) = self.windows.get(entity).ok()?;
                Some((
                    UVec2::new(window.physical_width(), window.physical_height()),
                    window.resolution.scale_factor(),
                ))
            }
            RenderTarget::Image(image) => {
                Some((self.images.get(&image.handle)?.size(), image.scale_factor))
            }
            _ => None,
        }
    }
}
fn retire(
    commands: &mut Commands,
    pass: Pass,
    targets: &mut Targets,
    cameras: &mut Query<(Entity, &mut Camera, &mut RenderTarget), With<CampaignSceneCamera>>,
) {
    if let Ok((_, mut camera, mut target)) = cameras.get_mut(pass.scene) {
        *target = pass.output;
        camera.viewport = pass.viewport;
        camera.order = pass.order;
    }
    commands.entity(pass.root).try_despawn();
    commands.entity(pass.camera).try_despawn();
    targets.images.remove(pass.image.id());
    targets.materials.remove(pass.material.id());
}
type SkyFilter = (With<SkyCamera>, Without<CampaignSceneCamera>);

fn world_viewport(frame: [f32; 4], size: UVec2) -> bevy::camera::Viewport {
    let edge = |fraction: f32, extent: u32| {
        (fraction * extent as f32).round().clamp(0., extent as f32) as u32
    };
    let start = UVec2::new(edge(frame[0], size.x), edge(frame[1], size.y));
    let end = UVec2::new(
        edge(frame[0] + frame[2], size.x),
        edge(frame[1] + frame[3], size.y),
    );
    bevy::camera::Viewport {
        physical_position: start,
        physical_size: (end - start).max(UVec2::ONE),
        ..default()
    }
}

fn sync(
    mut commands: Commands,
    mut state: ResMut<InternalResolution>,
    mut targets: Targets,
    mut scenes: Query<(Entity, &mut Camera, &mut RenderTarget), With<CampaignSceneCamera>>,
    mut skies: Query<(&mut Camera, &mut RenderTarget), SkyFilter>,
    card: Option<Res<crate::campaign_command_card::CommandCard>>,
    mut session: Option<
        ResMut<
            peacecraft3::map_script_session::MapScriptSession<
                pcx_play::map_startup_host::MapStartupHost,
            >,
        >,
    >,
) {
    let scene = scenes
        .single()
        .ok()
        .filter(|(_, c, _)| c.is_active)
        .map(|(e, _, _)| e);
    if let Some(entity) = scene {
        let normal = session
            .as_mut()
            .and_then(|s| s.host())
            .and_then(|h| h.objects().presentation())
            .is_none_or(|p| p.interface_visible != Some(false));
        if let Some(frame) = card.as_ref().and_then(|c| c.world_frame()) {
            let (_, mut camera, target) = scenes.get_mut(entity).expect("live scene");
            let output = state
                .pass
                .as_ref()
                .filter(|p| p.scene == entity)
                .map_or(target.as_ref(), |p| &p.output)
                .clone();
            if let Some((size, _)) = targets
                .size(&output)
                .filter(|(size, _)| size.min_element() > 0)
            {
                let viewport = normal.then(|| world_viewport(frame, size));
                camera.clear_color = bevy::camera::ClearColorConfig::None;
                if let Some(pass) = state.pass.as_mut().filter(|p| p.scene == entity) {
                    pass.viewport = viewport.clone();
                    camera.viewport = viewport.map(|mut v| {
                        v.physical_position *= pass.factor;
                        v.physical_size *= pass.factor;
                        v
                    });
                } else {
                    camera.viewport = viewport;
                }
                let clear = *state.clear.get_or_insert_with(|| {
                    commands
                        .spawn((
                            Camera2d,
                            RenderLayers::none(),
                            Camera {
                                order: -3,
                                clear_color: bevy::camera::ClearColorConfig::Custom(Color::BLACK),
                                ..default()
                            },
                            output.clone(),
                        ))
                        .id()
                });
                // Clear the render target outside the source world frame, below every scene pass.
                commands.entity(clear).insert((
                    Camera {
                        order: -3,
                        is_active: true,
                        clear_color: bevy::camera::ClearColorConfig::Custom(Color::BLACK),
                        ..default()
                    },
                    output,
                ));
            }
        }
    } else if let Some(clear) = state.clear {
        commands.entity(clear).insert(Camera {
            is_active: false,
            ..default()
        });
    }
    let mut desired = None;
    let mut error = if scene.is_none() && state.factor != 1 {
        state.error.clone()
    } else {
        None
    };
    if let Some(entity) = scene
        && state.factor != 1
    {
        let (_, camera, target) = scenes.get(entity).expect("live scene");
        let old = state.pass.as_ref().filter(|p| p.scene == entity);
        let output = old.map_or(target, |p| &p.output);
        if let Some((size, scale)) = targets.size(output) {
            match targets
                .device
                .as_ref()
                .ok_or_else(|| "Rendering device is unavailable.".to_string())
                .and_then(|d| extent(size, state.factor, d.limits().max_texture_dimension_2d))
            {
                Ok(internal) => {
                    desired = Some((
                        entity,
                        output.clone(),
                        old.map_or_else(|| camera.viewport.clone(), |p| p.viewport.clone()),
                        old.map_or(camera.order, |p| p.order),
                        internal,
                        scale,
                    ))
                }
                Err(e) => error = Some(e),
            }
        } else {
            error = Some(
                "Internal resolution requires a readable display or image target. Using 1×.".into(),
            );
        }
    }
    if state.error != error {
        if let Some(error) = &error {
            warn!("Internal resolution: {error}");
        }
        state.error = error;
    }
    let unchanged = desired
        .as_ref()
        .is_some_and(|(entity, _, _, _, size, scale)| {
            state.pass.as_ref().is_some_and(|p| {
                p.scene == *entity
                    && p.size == *size
                    && p.scale == *scale
                    && p.factor == state.factor
            })
        });
    if !unchanged {
        if let Some(pass) = state.pass.take() {
            retire(&mut commands, pass, &mut targets, &mut scenes);
        }
        if let Some((entity, output, viewport, order, size, scale)) = desired {
            let mut image = Image::new_uninit(
                Extent3d {
                    width: size.x,
                    height: size.y,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                TextureFormat::Rgba8UnormSrgb,
                default(),
            );
            image.texture_descriptor.usage =
                TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT;
            let image = targets.images.add(image);
            let material = targets.materials.add(Downsample {
                image: image.clone(),
                factor: state.factor,
            });
            let (_, mut camera, mut target) = scenes.get_mut(entity).expect("live scene");
            camera.order = -1;
            camera.viewport = viewport.clone().map(|mut v| {
                v.physical_position *= state.factor;
                v.physical_size *= state.factor;
                v
            });
            *target = RenderTarget::Image(ImageRenderTarget {
                handle: image.clone(),
                scale_factor: scale * state.factor as f32,
            });
            let compositor = commands
                .spawn((
                    Camera2d,
                    Camera {
                        order,
                        viewport: None,
                        clear_color: ClearColorConfig::None,
                        ..default()
                    },
                    output.clone(),
                    RenderLayers::none(),
                    Name::new("Internal resolution downsample"),
                ))
                .id();
            let root = commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        width: percent(100),
                        height: percent(100),
                        ..default()
                    },
                    UiTargetCamera(compositor),
                    MaterialNode(material.clone()),
                    Pickable::IGNORE,
                ))
                .id();
            state.pass = Some(Pass {
                scene: entity,
                camera: compositor,
                root,
                image,
                material,
                output,
                viewport,
                order,
                size,
                scale,
                factor: state.factor,
            });
        }
    }
    // Actor projection refreshes the sky camera every frame, before this system.
    for (mut camera, mut target) in &mut skies {
        if scene.is_some() && state.clear.is_some() {
            camera.clear_color = bevy::camera::ClearColorConfig::None;
        }
        if let Some(pass) = &state.pass {
            camera.order = -2;
            let (_, scene, scene_target) = scenes.get(pass.scene).expect("live scene");
            camera.viewport = scene.viewport.clone();
            camera.is_active = scene.is_active;
            *target = scene_target.clone();
        } else if camera.order == -2 {
            camera.order = -1;
            if let Ok((_, scene, scene_target)) = scenes.single() {
                camera.is_active = scene.is_active;
                camera.viewport = scene.viewport.clone();
                *target = scene_target.clone();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_world_frame_scales_both_edges_without_losing_the_display_origin() {
        for size in [UVec2::new(800, 600), UVec2::new(1600, 1200)] {
            let v = world_viewport([0.025, 0.05, 0.95, 0.7], size);
            assert_eq!(v.physical_position, UVec2::new(size.x / 40, size.y / 20));
            assert_eq!(
                v.physical_size,
                UVec2::new(size.x * 19 / 20, size.y * 7 / 10)
            );
        }
    }

    #[test]
    fn factors_preserve_aspect_and_refuse_unbounded_allocations() {
        for factor in 1..=4 {
            assert_eq!(
                extent(UVec2::new(1920, 1080), factor, 16384).unwrap(),
                UVec2::new(1920 * factor, 1080 * factor)
            );
        }
        for (size, factor, limit) in [
            (UVec2::ZERO, 2, 16384),
            (UVec2::new(u32::MAX, 1), 4, u32::MAX),
            (UVec2::new(3840, 2160), 4, 8192),
            (UVec2::new(7680, 4320), 4, 65536),
            (UVec2::ONE, 5, 16384),
        ] {
            assert!(extent(size, factor, limit).is_err());
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "internal_resolution_tests.rs"]
mod render_tests;

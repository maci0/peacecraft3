//! Executed cinematic filter on the published campaign view.
use crate::{
    browser_configuration::SelectedMapSources,
    campaign_camera::{CampaignCamera, CampaignCameraSet},
    map_binding::{AssetReceipt, receipt},
    map_requests::MapRequests,
    map_script_session::MapScriptSession,
};
use bevy::{
    camera::visibility::RenderLayers,
    ecs::system::SystemParam,
    prelude::*,
    render::render_resource::{AsBindGroup, TextureFormat},
    shader::ShaderRef,
};
use pcx_play::{map_startup_host::MapStartupHost, playercolor_host::CineFilter};
use std::sync::{Arc, Weak};

/// Roots which render on the gameplay UI camera when the campaign is visible.
#[derive(Component)]
pub struct CampaignHud;
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CampaignPresentationSet;

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct FilterMaterial {
    #[texture(0)]
    #[sampler(1)]
    texture: Handle<Image>,
    #[uniform(2)]
    uv: Vec4,
    #[uniform(3)]
    color: Vec4,
}
impl UiMaterial for FilterMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://peacecraft3/campaign_filter.wgsl".into()
    }
}

#[derive(Resource, Default)]
pub struct CampaignPresentation {
    session: Option<Weak<()>>,
    camera: Option<Entity>,
    overlay: Option<Entity>,
    barrier: Option<Entity>,
    image: Option<Handle<Image>>,
    material: Option<Handle<FilterMaterial>>,
    attempted: Option<u64>,
    shown: Option<CineFilter>,
    blocked: bool,
    pub texture_receipt: Option<AssetReceipt>,
    pub last_error: Option<String>,
}
impl CampaignPresentation {
    pub fn input_blocked(&self) -> bool {
        self.blocked
    }
}

pub fn install(app: &mut App) {
    bevy::asset::embedded_asset!(app, "campaign_filter.wgsl");
    app.add_plugins(UiMaterialPlugin::<FilterMaterial>::default())
        .init_resource::<CampaignPresentation>()
        .add_systems(
            Update,
            project
                .in_set(CampaignPresentationSet)
                .after(CampaignCameraSet),
        );
}

#[derive(SystemParam)]
struct ViewAssets<'w> {
    images: ResMut<'w, Assets<Image>>,
    materials: ResMut<'w, Assets<FilterMaterial>>,
    sources: Res<'w, SelectedMapSources>,
    requests: Res<'w, MapRequests>,
}
fn fullscreen() -> Node {
    Node {
        position_type: PositionType::Absolute,
        width: percent(100),
        height: percent(100),
        ..default()
    }
}
fn visible(commands: &mut Commands, entity: Option<Entity>, show: bool) {
    if let Some(entity) = entity {
        commands.entity(entity).insert(if show {
            Visibility::Visible
        } else {
            Visibility::Hidden
        });
    }
}
fn release(state: &mut CampaignPresentation, assets: &mut ViewAssets) {
    if let Some(image) = state.image.take() {
        assets.images.remove(image.id());
    }
    if let Some(material) = state.material.take() {
        assets.materials.remove(material.id());
    }
    state.attempted = None;
    state.shown = None;
    state.texture_receipt = None;
    state.last_error = None;
}

fn project(
    mut commands: Commands,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
    view: Res<CampaignCamera>,
    mut state: ResMut<CampaignPresentation>,
    mut assets: ViewAssets,
    mut cameras: Query<&mut Camera>,
    hud: Query<(Entity, Option<&UiTargetCamera>), With<CampaignHud>>,
) {
    let witness = session.session_witness();
    if !match (&state.session, &witness) {
        (Some(a), Some(b)) => Weak::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    } {
        release(&mut state, &mut assets);
        state.session = witness;
    }
    let objects = session.host().map(|h| h.objects());
    let now = objects.and_then(|o| o.timer_game_time());
    let presentation = objects.and_then(|o| o.presentation());
    let active = view.active();
    state.blocked = active && presentation.is_some_and(|p| p.ui_enabled == Some(false));
    let has_filter = presentation.is_some_and(|p| p.filter.is_some());
    let changed = presentation
        .and_then(|p| p.filter.as_ref())
        .filter(|f| state.attempted != Some(f.sequence))
        .cloned();
    if !active {
        if let Some(camera) = state.camera.and_then(|e| cameras.get_mut(e).ok()) {
            camera.into_inner().is_active = false;
        }
        visible(&mut commands, state.overlay, false);
        visible(&mut commands, state.barrier, false);
        return;
    }
    let camera = *state.camera.get_or_insert_with(|| {
        commands
            .spawn((
                Camera2d,
                Camera {
                    order: 1,
                    clear_color: ClearColorConfig::None,
                    ..default()
                },
                RenderLayers::none(),
                Name::new("Campaign UI camera"),
            ))
            .id()
    });
    if let Ok(mut camera) = cameras.get_mut(camera) {
        camera.is_active = true;
    }
    for (entity, target) in &hud {
        if target.is_none_or(|target| target.0 != camera) {
            commands.entity(entity).insert(UiTargetCamera(camera));
        }
    }
    let barrier = *state.barrier.get_or_insert_with(|| {
        commands
            .spawn((
                fullscreen(),
                UiTargetCamera(camera),
                GlobalZIndex(i32::MAX),
                Pickable {
                    should_block_lower: true,
                    is_hoverable: false,
                },
                Name::new("Campaign UI input barrier"),
            ))
            .id()
    });
    visible(&mut commands, Some(barrier), state.blocked);
    if !has_filter {
        state.shown = None;
        visible(&mut commands, state.overlay, false);
        return;
    }
    if let Some(filter) = changed {
        state.attempted = Some(filter.sequence);
        match prepare(&filter, &session, &state, &mut assets) {
            Ok((image, receipt)) => {
                let old_image = state.image.replace(image.clone());
                if let Some(old) = old_image.filter(|old| old.id() != image.id()) {
                    assets.images.remove(old.id());
                }
                let material = FilterMaterial {
                    texture: image,
                    uv: Vec4::ZERO,
                    color: Vec4::ZERO,
                };
                if let Some(handle) = &state.material {
                    assets
                        .materials
                        .insert(handle.id(), material)
                        .expect("retained filter material");
                } else {
                    state.material = Some(assets.materials.add(material));
                }
                state.texture_receipt = Some(receipt);
                state.shown = Some(filter.clone());
                state.last_error = None;
            }
            Err(error) => state.last_error = Some(error),
        }
    }
    if let Some(shown) = &state.shown {
        match shown.sample(now) {
            Ok(sample) => {
                let material = state.material.as_ref().expect("shown filter material");
                let mut value = assets
                    .materials
                    .get_mut(material)
                    .expect("retained filter material");
                value.uv = Vec4::new(
                    sample.uv[2] - sample.uv[0],
                    sample.uv[3] - sample.uv[1],
                    sample.uv[0],
                    sample.uv[1],
                );
                value.color = Vec4::from_array(sample.color);
                if state.overlay.is_none() {
                    state.overlay = Some(
                        commands
                            .spawn((
                                fullscreen(),
                                UiTargetCamera(camera),
                                GlobalZIndex(i32::MAX - 1),
                                MaterialNode(material.clone()),
                                Pickable::IGNORE,
                                Name::new("Campaign cinematic filter"),
                            ))
                            .id(),
                    );
                } else if let Some(overlay) = state.overlay {
                    commands
                        .entity(overlay)
                        .insert(MaterialNode(material.clone()));
                }
                visible(&mut commands, state.overlay, true);
            }
            Err(error) => {
                state.last_error = Some(error.into());
                visible(&mut commands, state.overlay, false);
            }
        }
    } else {
        visible(&mut commands, state.overlay, false);
    }
}

fn prepare(
    filter: &CineFilter,
    session: &MapScriptSession<MapStartupHost>,
    state: &CampaignPresentation,
    assets: &mut ViewAssets,
) -> Result<(Handle<Image>, AssetReceipt), String> {
    let context = session
        .context()
        .ok_or("campaign filter has no script context")?;
    let requests = &assets.requests;
    let bindings = requests
        .bindings
        .as_ref()
        .ok_or("campaign filter has no bindings")?;
    if !requests
        .admission
        .shared_context()
        .as_ref()
        .is_some_and(|a| Arc::ptr_eq(a, context))
        || !requests
            .published_receipts
            .as_ref()
            .is_some_and(|a| Arc::ptr_eq(a, &bindings.receipts))
    {
        return Err("campaign filter source does not match the published session".into());
    }
    if let (Some(shown), Some(image), Some(receipt)) =
        (&state.shown, &state.image, &state.texture_receipt)
        && shown.texture == filter.texture
    {
        return Ok((image.clone(), receipt.clone()));
    }
    let reader = assets
        .sources
        .for_receipts(&bindings.receipts)
        .ok_or("campaign filter has no retained source reader")?;
    // Application allocation policies, independent of authored image dimensions.
    let asset = texture_asset(&reader, &filter.texture, 16 << 20)
        .map_err(|e| format!("{}: {e}", filter.texture))?;
    let decoded = pcx_gfx::load_image_bounded(&asset.bytes, asset.path.as_str(), 64 << 20)
        .map_err(|e| e.to_string())?;
    let mut image = pcx_gfx::viewer::rgba_image(&decoded);
    // Raw numeric channels for the UI working space; no implicit texture transfer curve.
    image.texture_descriptor.format = TextureFormat::Rgba8Unorm;
    image.sampler = bevy::image::ImageSampler::linear();
    Ok((assets.images.add(image), receipt(&asset)))
}

// Legacy texture references may name TGA while the supplied archive stores BLP.
// Only a wholly absent exact path permits the format alias; read/decode errors do not.
fn texture_asset(
    reader: &pcx_assets::Resolver,
    path: &str,
    limit: u64,
) -> Result<pcx_assets::ResolvedAsset, pcx_assets::Error> {
    match reader.resolve_bounded(path, limit) {
        Err(pcx_assets::Error::MissingAsset { path }) => match path.as_str().strip_suffix(".tga") {
            Some(stem) => reader.resolve_bounded(&format!("{stem}.blp"), limit),
            None => Err(pcx_assets::Error::MissingAsset { path }),
        },
        result => result,
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "campaign_filter_gpu_tests.rs"]
mod gpu_tests;

//! Receipt-bound minimap terrain. Visibility and overlays are separate consumers.
use bevy::{
    picking::{events::PointerPress, pointer::PointerButton},
    prelude::*,
    ui::RelativeCursorPosition,
};
use pcx_play::{map_startup_host::MapStartupHost, minimap_layout::Rules};
use peacecraft3::{
    campaign_camera::{CampaignCamera, CampaignCameraSet},
    campaign_presentation::{CampaignHud, CampaignInterface, CampaignPresentationSet},
    map_requests::MapRequests,
    map_script_session::MapScriptSession,
};
use sha2::{Digest, Sha256};
use std::{
    collections::VecDeque,
    sync::{Arc, Weak},
};

// Application allocation limits, independent of authored map/image dimensions.
const IMAGE_BYTES: usize = 4 << 20;
const PIXEL_BYTES: usize = 16 << 20;
const FOG_ERROR: &str = "minimap fog-of-war projection is not yet supported";
const SETTINGS_ERROR: &str = "minimap requires admitted fog settings";
const OCCLUSION_ERROR: &str =
    "minimap circular sight is rendered; terrain/tree sight occlusion is not implemented";
const PROFILE_BYTES: usize = 1 << 20;
const VISION_WORK: usize = 16 << 20;
const POINTER_PRESSES: usize = 64;

#[path = "campaign_minimap_markers.rs"]
mod markers;

struct FogImage {
    raster: crate::campaign_visibility::Raster,
    alpha: pcx_objects::editor::minimap_fog::MinimapFogAlpha,
    original_alpha: Vec<u8>,
    original_pixels: Vec<u8>,
    viewer: Option<pcx_script::HandleId>,
    width: u32,
    height: u32,
}
#[derive(Resource, Default)]
pub(crate) struct CampaignMinimap {
    witness: Option<Weak<()>>,
    attempted: bool,
    root: Option<Entity>,
    image_node: Option<Entity>,
    image: Option<Handle<Image>>,
    fog: Option<FogImage>,
    markers: Option<Result<markers::Markers, String>>,
    pending_pointer: VecDeque<MinimapPress>,
    drag: Option<Entity>,
    load_error: Option<String>,
    pub member: Option<String>,
    pub shown: bool,
    pub last_error: Option<String>,
}
#[cfg(all(test, not(target_arch = "wasm32"), feature = "test-fixtures"))]
impl CampaignMinimap {
    pub(crate) fn diagnostic_image(&self) -> Option<Handle<Image>> {
        self.image.clone()
    }
}
#[derive(Component)]
enum MinimapPart {
    Surface,
    Image,
}
struct MinimapPress {
    surface: Entity,
    button: PointerButton,
    fraction: Option<Vec2>,
}
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct MinimapInputSet;

pub(crate) fn install(app: &mut App) {
    app.init_resource::<CampaignMinimap>()
        .add_observer(capture_press)
        .add_systems(
            Update,
            project
                .after(CampaignCameraSet)
                .after(peacecraft3::campaign_runtime::CampaignAdvanceSet)
                .after(peacecraft3::campaign_startup::CampaignStartupSet)
                .before(crate::campaign_hud::CampaignHudSet)
                .before(CampaignPresentationSet),
        )
        .add_systems(
            Update,
            consume_pointer
                .in_set(crate::campaign_input::CampaignInputSet)
                .in_set(MinimapInputSet)
                .after(crate::campaign_hud::CampaignHudSet)
                .after(crate::campaign_script_dialogs::CampaignScriptDialogSet),
        );
}
fn capture_press(
    event: On<PointerPress>,
    parts: Query<(&MinimapPart, Option<&RelativeCursorPosition>)>,
    mut state: ResMut<CampaignMinimap>,
    presentation: Option<Res<peacecraft3::campaign_presentation::CampaignPresentation>>,
) {
    let Ok((MinimapPart::Surface, cursor)) = parts.get(event.entity) else {
        return;
    };
    if !crate::button_actions::input_allowed(presentation) {
        return;
    }
    if state.pending_pointer.len() >= POINTER_PRESSES
        || state.pending_pointer.try_reserve(1).is_err()
    {
        state.last_error = Some("minimap pointer input budget exceeded".into());
        warn!("minimap pointer input budget exceeded");
        return;
    }
    state.pending_pointer.push_back(MinimapPress {
        surface: event.entity,
        button: event.button,
        fraction: cursor
            .and_then(|p| p.normalized)
            .map(|p| p + Vec2::splat(0.5)),
    });
}

#[derive(bevy::ecs::system::SystemParam)]
struct PointerControls<'w> {
    camera: Option<Res<'w, CampaignCamera>>,
    mouse: Option<Res<'w, ButtonInput<MouseButton>>>,
    presentation: Option<Res<'w, peacecraft3::campaign_presentation::CampaignPresentation>>,
    dialogs: Option<Res<'w, crate::campaign_script_dialogs::CampaignScriptDialogs>>,
}

fn consume_pointer(
    mut state: ResMut<CampaignMinimap>,
    mut hud: ResMut<crate::campaign_hud::CampaignHud>,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
    controls: PointerControls,
    cursors: Query<&RelativeCursorPosition, With<MinimapPart>>,
    mut input: ResMut<crate::campaign_input::CampaignInput>,
) {
    use crate::campaign_input::MinimapAction;
    let mut requests = std::mem::take(&mut state.pending_pointer);
    let pressed = !requests.is_empty();
    let left_held = controls
        .mouse
        .as_ref()
        .is_some_and(|m| m.pressed(MouseButton::Left));
    if !pressed && let Some(entity) = state.drag {
        if left_held {
            requests.push_back(MinimapPress {
                surface: entity,
                button: PointerButton::Primary,
                fraction: cursors
                    .get(entity)
                    .ok()
                    .and_then(|p| p.normalized)
                    .map(|p| p + Vec2::splat(0.5)),
            });
        } else {
            state.drag = None;
        }
    }
    if requests.is_empty() {
        state.pending_pointer = requests;
        return;
    }
    hud.pointer_consumed = true;
    if !controls
        .camera
        .is_some_and(|c| c.active() && !c.menu_open())
        || !crate::button_actions::input_allowed(controls.presentation)
        || hud.quest_log_open
        || controls.dialogs.is_some_and(|d| d.input_blocked())
    {
        state.drag = None;
        requests.clear();
        state.pending_pointer = requests;
        return;
    }
    while let Some(request) = requests.pop_front() {
        if pressed {
            state.drag = None;
        }
        let result = (|| -> Result<(), String> {
            if !state
                .witness
                .as_ref()
                .zip(session.session_witness())
                .is_some_and(|(a, b)| Weak::ptr_eq(a, &b))
            {
                return Err("minimap request belongs to a retired session".into());
            }
            let witness = session.session_witness();
            let host = session.host().ok_or("minimap has no live session")?;
            let objects = host.objects_mut();
            if objects.presentation().is_some_and(|p| {
                p.control_enabled == Some(false)
                    || p.ui_enabled == Some(false)
                    || p.interface_visible == Some(false)
            }) {
                state.drag = None;
                return Ok(());
            }
            if objects.local_player_settings().is_some_and(|local| {
                objects
                    .player_removal_state()
                    .and_then(|s| s.get(local.identity))
                    .is_some_and(|p| p.left)
            }) {
                return Err("minimap owner has left".into());
            }
            if !matches!(
                request.button,
                PointerButton::Primary | PointerButton::Secondary
            ) {
                return Err("unsupported minimap pointer button".into());
            }
            let action = if pressed {
                crate::campaign_input::minimap_action(
                    &mut input,
                    witness,
                    request.button == PointerButton::Primary,
                )
            } else {
                if input.command.is_some() {
                    state.drag = None;
                    return Ok(());
                }
                MinimapAction::Camera
            };
            if action == MinimapAction::Cancel {
                state.drag = None;
                return Ok(());
            }
            let fraction = request
                .fraction
                .ok_or("minimap pointer position is unavailable")?;
            let bounds = objects
                .vision_world_bounds()
                .ok_or("minimap world extent is absent")?;
            let target = state
                .fog
                .as_ref()
                .ok_or("minimap image is absent")?
                .raster
                .world_at(bounds, fraction.to_array())?;
            match action {
                MinimapAction::Camera => {
                    peacecraft3::campaign_camera::scroll_to(objects, target)?;
                    state.drag = left_held.then_some(request.surface);
                    debug!(?target, "campaign minimap camera target");
                }
                MinimapAction::PointOrder(kind) => {
                    state.drag = None;
                    let before = input.commands.entries().len();
                    crate::campaign_input::standard_target(
                        objects, &mut input, kind, target, None,
                    )?;
                    debug!(
                        ?target,
                        ?kind,
                        recorded = input.commands.entries().len() - before,
                        "campaign minimap point order"
                    );
                }
                MinimapAction::Cancel => unreachable!(),
            }
            Ok(())
        })();
        if let Err(error) = result {
            state.drag = None;
            state.last_error = Some(error);
        }
    }
    state.pending_pointer = requests;
}
fn visibility(settings: Option<pcx_play::playercolor_host::StartupSettings>) -> Result<(), String> {
    let settings = settings.ok_or(SETTINGS_ERROR)?;
    if settings.fog_enabled || settings.fog_mask_enabled {
        return Err(FOG_ERROR.into());
    }
    Ok(())
}
fn node(r: pcx_play::inventory_layout::Rect, viewport: [f32; 2]) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Percent(r.x / viewport[0] * 100.),
        bottom: Val::Percent(r.y / viewport[1] * 100.),
        width: Val::Percent(r.width / viewport[0] * 100.),
        height: Val::Percent(r.height / viewport[1] * 100.),
        ..default()
    }
}
fn load_image(
    names: &[String],
    max_dimension: u32,
    mut extract: impl FnMut(&'static str) -> Result<Option<Vec<u8>>, String>,
) -> Result<(String, Image), String> {
    for name in names {
        let member = if name == pcx_map::minimap::MINIMAP_IMAGE_TGA_NAME {
            pcx_map::minimap::MINIMAP_IMAGE_TGA_NAME
        } else if name == pcx_map::minimap::MINIMAP_IMAGE_NAME {
            pcx_map::minimap::MINIMAP_IMAGE_NAME
        } else {
            return Err(format!("unsupported minimap member {name}"));
        };
        let Some(bytes) = extract(member)? else {
            continue;
        };
        if bytes.len() > IMAGE_BYTES {
            return Err(format!("{member}: minimap input byte budget exceeded"));
        }
        // Frame the declared format before the generic decoder's other-format fallback.
        let framed = pcx_map::MinimapImage::from_named_bytes(member, bytes.clone())
            .map_err(|e| format!("{member}: {e}"))?;
        let (width, height) = framed.dimensions();
        if width > max_dimension || height > max_dimension {
            return Err(format!(
                "{member}: minimap exceeds the render device's texture dimensions"
            ));
        }
        let decoded =
            pcx_gfx::load_image_bounded(&bytes, member, PIXEL_BYTES).map_err(|e| e.to_string())?;
        // shortcut: house clamp/linear sampling; qualify native sampling with the paired render.
        let image = pcx_gfx::viewer::rgba_image(&decoded);
        return Ok((member.into(), image));
    }
    Err("admitted map has no minimap image in the selected client's lookup order".into())
}

#[derive(bevy::ecs::system::SystemParam)]
struct Inputs<'w> {
    requests: Res<'w, MapRequests>,
    camera: Res<'w, CampaignCamera>,
    launcher: Option<Res<'w, crate::launch_gui::Launcher>>,
    device: Option<Res<'w, bevy::render::renderer::RenderDevice>>,
    sources: Option<Res<'w, peacecraft3::browser_configuration::SelectedMapSources>>,
}

fn publish_minimap_alpha(pixels: &mut [u8], base: &[u8], alpha: &[u8]) {
    pixels.copy_from_slice(base);
    for (pixel, alpha) in pixels.as_chunks_mut::<4>().0.iter_mut().zip(alpha) {
        pixel[3] = *alpha;
    }
}

fn update_fog(
    state: &mut CampaignMinimap,
    host: &pcx_play::playercolor_host::ObjectHost,
    images: &mut Assets<Image>,
) -> Result<(), String> {
    let settings = host.startup_settings().ok_or(SETTINGS_ERROR)?;
    let image_id = state.image.as_ref().ok_or("minimap image missing")?;
    let pixel_len = images
        .get(image_id)
        .and_then(|image| image.data.as_ref())
        .map(|pixels| pixels.len())
        .ok_or("minimap image asset missing")?;
    let fog = state
        .fog
        .as_mut()
        .ok_or("minimap fog image is not loaded")?;
    if pixel_len != fog.original_pixels.len()
        || pixel_len / 4 != fog.original_alpha.len()
        || !pixel_len.is_multiple_of(4)
    {
        return Err("minimap pixel buffer changed dimensions".into());
    }
    // Both switches off restores the authored alpha without a ledger.
    if !settings.fog_enabled && !settings.fog_mask_enabled {
        let original_alpha = fog.original_alpha.clone();
        let original_pixels = fog.original_pixels.clone();
        let mut image = images
            .get_mut(image_id)
            .ok_or("minimap CPU pixels unavailable")?;
        let pixels = image
            .data
            .as_mut()
            .ok_or("minimap CPU pixels unavailable")?;
        publish_minimap_alpha(pixels, &original_pixels, &original_alpha);
        return Ok(());
    }
    // Stage the ledger projection before replacing accepted pixels. A refusal
    // keeps the previous masked image, including markers already drawn on it.
    host.vision_readiness()?;
    let viewer = host
        .local_player_settings()
        .ok_or("minimap viewer is not admitted")?
        .identity;
    let bounds = host
        .vision_world_bounds()
        .ok_or("minimap needs the admitted world extent")?;
    let fog_enabled = settings.fog_enabled;
    let mask_enabled = settings.fog_mask_enabled;
    let mut staged_raster = if fog.viewer == Some(viewer) {
        None
    } else {
        Some(crate::campaign_visibility::Raster::new(
            fog.width,
            fog.height,
            PIXEL_BYTES / 4,
        )?)
    };
    let raster = staged_raster.as_mut().unwrap_or(&mut fog.raster);
    let alpha = raster
        .project(bounds, fog.alpha, VISION_WORK, |point| {
            Ok(host
                .vision_presented(
                    viewer,
                    [point[0] as f32, point[1] as f32],
                    fog_enabled,
                    mask_enabled,
                )
                .map_err(|error| error.to_string())?
                .unwrap_or(pcx_play::playercolor_host::VisionState::Masked))
        })?
        .to_vec();
    if let Some(raster) = staged_raster {
        fog.raster = raster;
        fog.viewer = Some(viewer);
    }
    let original_pixels = fog.original_pixels.clone();
    let mut image = images
        .get_mut(image_id)
        .ok_or("minimap CPU pixels unavailable")?;
    let pixels = image
        .data
        .as_mut()
        .ok_or("minimap CPU pixels unavailable")?;
    publish_minimap_alpha(pixels, &original_pixels, &alpha);
    Ok(())
}
fn project(
    mut commands: Commands,
    mut state: ResMut<CampaignMinimap>,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
    input: Inputs,
    mut images: ResMut<Assets<Image>>,
    mut parts: Query<(&MinimapPart, &mut Node)>,
) {
    let witness = session.session_witness();
    let same = match (&state.witness, &witness) {
        (Some(a), Some(b)) => Weak::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    };
    if !same {
        if let Some(root) = state.root {
            commands.entity(root).try_despawn();
        }
        if let Some(image) = state.image.take() {
            images.remove(image.id());
        }
        *state = CampaignMinimap {
            witness,
            ..default()
        };
    }
    let visibility = session
        .host()
        .map(|host| visibility(host.objects().startup_settings()));
    let fog_result = if state.fog.is_some() {
        session
            .host()
            .map(|h| update_fog(&mut state, h.objects(), &mut images))
    } else {
        None
    };
    let marker_error = {
        let state = &mut *state;
        match (&mut state.markers, &state.fog, &state.image) {
            (Some(Err(error)), _, _) => Some(error.clone()),
            (Some(Ok(markers)), Some(fog), Some(image)) if matches!(fog_result, Some(Ok(()))) => {
                session.host().and_then(|host| {
                    images
                        .get_mut(image)
                        .ok_or("minimap marker pixels unavailable".to_string())
                        .and_then(|mut image| {
                            let pixels = image
                                .data
                                .as_mut()
                                .ok_or("minimap marker pixels unavailable")?;
                            markers.draw(host.objects(), fog, pixels)
                        })
                        .err()
                })
            }
            _ => None,
        }
    };
    let visible = input.camera.active()
        && (matches!(visibility, Some(Ok(()))) || matches!(fog_result, Some(Ok(()))));
    for (part, mut node) in &mut parts {
        let allowed = match part {
            MinimapPart::Surface => input.camera.active(),
            MinimapPart::Image => visible,
        };
        let display = if allowed {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
    }
    state.shown = visible
        && state.image_node.is_some()
        && session.host().is_some_and(|h| {
            h.objects()
                .presentation()
                .is_none_or(|p| p.interface_visible != Some(false))
        });
    let mode_error = marker_error.or_else(|| match fog_result {
        Some(Err(error)) => Some(error),
        Some(Ok(())) if matches!(visibility, Some(Err(_))) => Some(OCCLUSION_ERROR.into()),
        _ => visibility.and_then(Result::err),
    });
    if let Some(error) = &mode_error {
        state.last_error = Some(error.clone());
    } else if matches!(
        state.last_error.as_deref(),
        Some(FOG_ERROR | SETTINGS_ERROR | OCCLUSION_ERROR)
    ) {
        state.last_error = state.load_error.clone();
    }
    if state.attempted {
        return;
    }
    let Some(context) = session.context() else {
        return;
    };
    let Some(bindings) = input.requests.bindings.as_ref().filter(|b| {
        input
            .requests
            .admission
            .shared_context()
            .as_ref()
            .is_some_and(|c| Arc::ptr_eq(c, context))
            && input
                .requests
                .published_receipts
                .as_ref()
                .is_some_and(|r| Arc::ptr_eq(r, &b.receipts))
    }) else {
        return;
    };
    state.attempted = true;
    let geometry: Result<Rules, String> = (|| {
        if bindings.catalog_map_bytes.len() > bindings.max_map_file_bytes {
            return Err("minimap map byte budget exceeded".into());
        }
        let digest: [u8; 32] = Sha256::digest(&bindings.catalog_map_bytes).into();
        if digest != context.map_sha256 {
            return Err("minimap map bytes differ from the admitted context".into());
        }
        let install = input
            .launcher
            .as_deref()
            .and_then(crate::launch_gui::selected_install);
        crate::menu_lighting::with_client_image(install.as_deref(), |source, bytes| {
            Rules::reviewed_legacy(
                bytes,
                pcx_play::scripts::AssetIdentity {
                    source: source.into(),
                    edition: "selected campaign client".into(),
                    version: "reviewed Legacy 1.29.2.9231 minimap image".into(),
                },
            )
        })
    })();
    let rules = match geometry {
        Ok(rules) => rules,
        Err(error) => {
            state.load_error = Some(error.clone());
            state.last_error = Some(error);
            return;
        }
    };
    state.markers = input
        .sources
        .as_deref()
        .and_then(|sources| sources.for_receipts(&bindings.receipts))
        .map(|reader| markers::Markers::read(reader, &rules));
    if let Some(Err(error)) = &state.markers {
        state.last_error = Some(error.clone());
    }
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                position_type: PositionType::Absolute,
                ..default()
            },
            GlobalZIndex(10),
            CampaignHud,
            CampaignInterface,
            Pickable::IGNORE,
        ))
        .id();
    let mut surface_node = node(rules.frame, rules.viewport);
    if !input.camera.active() {
        surface_node.display = Display::None;
    }
    let surface = commands
        .spawn((
            surface_node,
            BackgroundColor(Color::srgba_u8(
                rules.clear_color[0],
                rules.clear_color[1],
                rules.clear_color[2],
                rules.clear_color[3],
            )),
            MinimapPart::Surface,
            RelativeCursorPosition::default(),
        ))
        .id();
    commands.entity(root).add_child(surface);
    state.root = Some(root);
    let result: Result<(String, Image), String> = (|| {
        let map = pcx_map::Map::parse(&bindings.catalog_map_bytes).map_err(|e| e.to_string())?;
        let budget = pcx_mpq::checked::ReadBudget {
            max_archive_bytes: bindings.max_map_file_bytes,
            max_archive_table_bytes: 4 << 20,
            max_raw_packed_bytes: IMAGE_BYTES,
            max_decoded_bytes: IMAGE_BYTES,
        };
        let max_dimension = input
            .device
            .as_ref()
            .ok_or("minimap render device unavailable")?
            .limits()
            .max_texture_dimension_2d;
        load_image(&rules.image_members, max_dimension, |name| {
            map.extract_checked(&bindings.map_label, name, budget)
                .map_err(|e| e.to_string())
        })
    })();
    match result {
        Ok((member, image)) => {
            let fog = (|| -> Result<FogImage, String> {
                let reader = input
                    .sources
                    .as_deref()
                    .and_then(|sources| sources.for_receipts(&bindings.receipts))
                    .ok_or("minimap fog has no matching source reader")?;
                let profile = reader
                    .resolve_bounded("UI\\MiscData.txt", PROFILE_BYTES as u64)
                    .map_err(|e| e.to_string())?;
                let alpha = pcx_objects::editor::minimap_fog::MinimapFogAlpha::read(
                    &profile.bytes,
                    PROFILE_BYTES,
                )?;
                let width = image.texture_descriptor.size.width;
                let height = image.texture_descriptor.size.height;
                let raster =
                    crate::campaign_visibility::Raster::new(width, height, PIXEL_BYTES / 4)?;
                let pixels = image
                    .data
                    .as_ref()
                    .ok_or("minimap CPU pixels unavailable")?;
                let mut original_alpha = Vec::new();
                original_alpha
                    .try_reserve_exact(pixels.len() / 4)
                    .map_err(|_| "minimap alpha allocation")?;
                original_alpha.extend(pixels.as_chunks::<4>().0.iter().map(|p| p[3]));
                Ok(FogImage {
                    raster,
                    alpha,
                    original_pixels: pixels.clone(),
                    original_alpha,
                    viewer: None,
                    width,
                    height,
                })
            })();
            match fog {
                Ok(fog) => state.fog = Some(fog),
                Err(error) => {
                    state.load_error = Some(error.clone());
                    state.last_error = Some(error);
                    return;
                }
            }
            let image = images.add(image);
            let child = commands
                .spawn((
                    Node {
                        width: Val::Percent(100.),
                        height: Val::Percent(100.),
                        display: if visible {
                            Display::Flex
                        } else {
                            Display::None
                        },
                        ..default()
                    },
                    ImageNode::new(image.clone()),
                    MinimapPart::Image,
                    Pickable::IGNORE,
                ))
                .id();
            commands.entity(surface).add_child(child);
            state.image = Some(image);
            state.image_node = Some(child);
            state.member = Some(member);
            state.last_error = mode_error;
            state.shown = visible
                && session.host().is_some_and(|h| {
                    h.objects()
                        .presentation()
                        .is_none_or(|p| p.interface_visible != Some(false))
                });
        }
        Err(error) => {
            state.load_error = Some(error.clone());
            state.last_error = Some(error);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    fn tga() -> Vec<u8> {
        let mut bytes = vec![0; 18];
        bytes[2] = 2;
        bytes[12..14].copy_from_slice(&2u16.to_le_bytes());
        bytes[14..16].copy_from_slice(&1u16.to_le_bytes());
        bytes[16] = 24;
        bytes[17] = 0x20;
        bytes.extend_from_slice(&[3, 2, 1, 9, 8, 7]);
        bytes
    }
    #[test]
    fn image_selection_decodes_the_source_order_without_substituting_missing_art() {
        let tga_name = pcx_map::minimap::MINIMAP_IMAGE_TGA_NAME;
        let blp_name = pcx_map::minimap::MINIMAP_IMAGE_NAME;
        let mut reads = Vec::new();
        let (member, image) = load_image(&[blp_name.into(), tga_name.into()], 2, |name| {
            reads.push(name);
            Ok((name == tga_name).then(tga))
        })
        .unwrap();
        assert_eq!(reads, [blp_name, tga_name]);
        assert_eq!(member, tga_name);
        assert_eq!(image.texture_descriptor.size.width, 2);
        assert_eq!(image.texture_descriptor.size.height, 1);
        assert_eq!(
            image.data.as_deref(),
            Some(&[1, 2, 3, 255, 7, 8, 9, 255][..])
        );
        assert!(
            load_image(&[tga_name.into()], 2, |_| Ok(None))
                .unwrap_err()
                .contains("no minimap image")
        );
        let mut reads = 0;
        assert!(
            load_image(&[tga_name.into(), blp_name.into()], 2, |_| {
                reads += 1;
                Ok(Some(vec![0; 2]))
            })
            .is_err()
        );
        assert_eq!(
            reads, 1,
            "malformed present art must not hide behind another member"
        );
        assert!(
            load_image(&[tga_name.into()], 2, |_| Err("source failed".into()))
                .unwrap_err()
                .contains("source failed")
        );
        assert!(
            load_image(&[tga_name.into()], 1, |_| Ok(Some(tga())))
                .unwrap_err()
                .contains("texture dimensions")
        );
    }
    #[test]
    fn unconditional_image_requires_explicitly_disabled_fog_and_mask() {
        use pcx_play::playercolor_host::StartupSettings;
        let speed = pcx_script::HandleId::new(std::num::NonZeroU64::new(1).unwrap());
        assert!(visibility(None).is_err());
        for (fog_enabled, fog_mask_enabled) in [(true, false), (false, true), (true, true)] {
            assert!(
                visibility(Some(StartupSettings {
                    speed,
                    fog_enabled,
                    fog_mask_enabled
                }))
                .is_err()
            );
        }
        assert!(
            visibility(Some(StartupSettings {
                speed,
                fog_enabled: false,
                fog_mask_enabled: false
            }))
            .is_ok()
        );
    }
    #[cfg(all(not(target_arch = "wasm32"), feature = "test-fixtures"))]
    #[test]
    fn fog_refusal_keeps_the_accepted_minimap_pixels() {
        use bevy::{
            asset::RenderAssetUsages,
            render::render_resource::{Extent3d, TextureDimension, TextureFormat},
        };
        use pcx_play::map_startup_host::SettingsObservation;
        const EXTRA: &str = "type mapflag extends handle
constant native ConvertMapFlag takes integer i returns mapflag
native SetMapFlag takes mapflag flag, boolean value returns nothing
native IsMapFlagSet takes mapflag flag returns boolean
type gamespeed extends handle
constant native ConvertGameSpeed takes integer i returns gamespeed
constant native GetGameSpeed takes nothing returns gamespeed
native SetGameSpeed takes gamespeed speed returns nothing
native IsFogEnabled takes nothing returns boolean
native IsFogMaskEnabled takes nothing returns boolean
native FogEnable takes boolean enabled returns nothing
native FogMaskEnable takes boolean enabled returns nothing
globals
constant mapflag MAP_LOCK_SPEED = ConvertMapFlag(17*19)
constant gamespeed CURRENT_SPEED = ConvertGameSpeed(-17)
endglobals
";
        let mut app = App::new();
        peacecraft3::map_requests::install(&mut app);
        peacecraft3::campaign_startup::install(&mut app);
        let mut fixture = crate::campaign_startup_cases::fixture(
            "function config takes nothing returns nothing\nendfunction\nfunction main takes nothing returns nothing\nendfunction\n",
        );
        fixture.common.extend_from_slice(EXTRA.as_bytes());
        fixture.rebuild();
        let resolver = fixture.resolver();
        let bound =
            peacecraft3::map_binding::bind_map(&resolver, fixture.input(1), fixture.limits())
                .unwrap();
        let mut inputs =
            crate::campaign_startup_cases::host_inputs(std::sync::Arc::clone(&bound.catalog.0));
        inputs
            .enumeration
            .names
            .extend(["ConvertGameSpeed".into(), "ConvertMapFlag".into()]);
        inputs.post_config.settings = Some(SettingsObservation {
            speed_global: "CURRENT_SPEED".into(),
            fog_enabled: true,
            fog_mask_enabled: true,
        });
        let bytes = std::sync::Arc::clone(&bound.catalog_map_bytes);
        app.insert_resource(peacecraft3::map_requests::MapRequests {
            bindings: Some(bound),
            ..default()
        })
        .insert_resource(peacecraft3::campaign_startup::CampaignHostSupply {
            revision: 1,
            inputs: Some(inputs),
        });
        app.world_mut()
            .write_message(peacecraft3::map_requests::MapRequest {
                label: "synthetic fog publication".into(),
                map_member: pcx_assets::AssetPath::new("selected map.w3m").unwrap(),
                identity: fixture.map_id.clone(),
                binding_revision: 1,
                payload: peacecraft3::map_requests::MapPayload::Shared(bytes),
            });
        app.update();
        assert!(
            app.world()
                .resource::<peacecraft3::campaign_startup::CampaignStartup>()
                .last_error
                .is_none(),
            "{:?}",
            app.world()
                .resource::<peacecraft3::campaign_startup::CampaignStartup>()
                .last_error
        );
        let accepted = [4u8, 5, 6, 9];
        let mut images = Assets::<Image>::default();
        let image = images.add(Image::new(
            Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            accepted.to_vec(),
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        ));
        let mut state = CampaignMinimap {
            image: Some(image),
            fog: Some(FogImage {
                raster: crate::campaign_visibility::Raster::new(1, 1, 16).unwrap(),
                alpha: pcx_objects::editor::minimap_fog::MinimapFogAlpha {
                    masked: 3,
                    fogged: 71,
                    visible: 241,
                },
                original_alpha: vec![255],
                original_pixels: vec![1, 2, 3, 255],
                viewer: None,
                width: 1,
                height: 1,
            }),
            ..default()
        };
        let error = {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let host = session.host().expect("published host");
            update_fog(&mut state, host.objects(), &mut images).unwrap_err()
        };
        state.last_error = Some(error);
        assert_eq!(
            images
                .get(state.image.as_ref().unwrap())
                .unwrap()
                .data
                .as_deref(),
            Some(&accepted[..])
        );
        assert!(
            state
                .last_error
                .as_deref()
                .is_some_and(|error| error.contains("shared vision")),
            "{:?}",
            state.last_error
        );
        {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let host = session.host().expect("published host");
            assert!(update_fog(&mut state, host.objects(), &mut images).is_err());
        }
        assert_eq!(
            images
                .get(state.image.as_ref().unwrap())
                .unwrap()
                .data
                .as_deref(),
            Some(&accepted[..])
        );
        {
            use pcx_script::{Host, Value};
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let host = session.host().expect("published host");
            host.objects_mut()
                .native("FogEnable", &[Value::Bool(false)])
                .unwrap();
            host.objects_mut()
                .native("FogMaskEnable", &[Value::Bool(false)])
                .unwrap();
            update_fog(&mut state, host.objects(), &mut images).unwrap();
        }
        assert_eq!(
            images
                .get(state.image.as_ref().unwrap())
                .unwrap()
                .data
                .as_deref(),
            Some(&[1, 2, 3, 255][..])
        );
    }
    #[test]
    fn minimap_press_batches_are_bounded_and_cannot_leak_into_a_missing_scene() {
        let mut world = World::new();
        world.init_resource::<CampaignMinimap>();
        world.init_resource::<MapScriptSession<MapStartupHost>>();
        world.init_resource::<crate::campaign_hud::CampaignHud>();
        world.init_resource::<crate::campaign_input::CampaignInput>();
        world.add_observer(capture_press);
        let image = world.spawn(MinimapPart::Surface).id();
        for _ in 0..POINTER_PRESSES + 1 {
            world.commands().trigger(PointerPress {
                entity: image,
                pointer: bevy::picking::events::Pointer::new(
                    bevy::picking::pointer::PointerId::Mouse,
                    bevy::picking::pointer::Location {
                        target: bevy::camera::NormalizedRenderTarget::None {
                            width: 1,
                            height: 1,
                        },
                        position: Vec2::ZERO,
                    },
                ),
                button: bevy::picking::pointer::PointerButton::Secondary,
                hit: bevy::picking::backend::HitData::new(image, 0., None, None),
                count: 1,
            });
        }
        world.flush();
        assert_eq!(
            world.resource::<CampaignMinimap>().pending_pointer.len(),
            POINTER_PRESSES
        );
        assert!(
            world
                .resource::<CampaignMinimap>()
                .last_error
                .as_ref()
                .unwrap()
                .contains("budget")
        );
        world.run_system_once(consume_pointer).unwrap();
        assert!(
            world
                .resource::<crate::campaign_hud::CampaignHud>()
                .pointer_consumed
        );
        assert!(
            world
                .resource::<CampaignMinimap>()
                .pending_pointer
                .is_empty()
        );
        assert!(
            world
                .resource::<crate::campaign_input::CampaignInput>()
                .commands
                .entries()
                .is_empty()
        );
        world
            .resource_mut::<crate::campaign_hud::CampaignHud>()
            .pointer_consumed = false;
        world.run_system_once(consume_pointer).unwrap();
        assert!(
            !world
                .resource::<crate::campaign_hud::CampaignHud>()
                .pointer_consumed
        );
    }
    #[test]
    fn losing_pointer_position_cancels_a_held_drag() {
        let mut world = World::new();
        world.init_resource::<CampaignMinimap>();
        world.init_resource::<crate::campaign_hud::CampaignHud>();
        world.init_resource::<crate::campaign_input::CampaignInput>();
        world.init_resource::<MapScriptSession<MapStartupHost>>();
        world.init_resource::<ButtonInput<MouseButton>>();
        let surface = world
            .spawn((MinimapPart::Surface, RelativeCursorPosition::default()))
            .id();
        world.resource_mut::<CampaignMinimap>().drag = Some(surface);
        world
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        world.run_system_once(consume_pointer).unwrap();
        assert!(world.resource::<CampaignMinimap>().drag.is_none());
        assert!(
            world
                .resource::<crate::campaign_hud::CampaignHud>()
                .pointer_consumed
        );
    }
    #[test]
    fn retiring_the_session_removes_the_tree_and_owned_image() {
        let mut app = App::new();
        peacecraft3::campaign_camera::install(&mut app, 0, 0);
        let world = app.world_mut();
        world.init_resource::<MapRequests>();
        world.init_resource::<MapScriptSession<MapStartupHost>>();
        world.init_resource::<Assets<Image>>();
        let image = world.resource_mut::<Assets<Image>>().add(Image::default());
        let root = world.spawn(Node::default()).id();
        let surface = world.spawn((Node::default(), MinimapPart::Surface)).id();
        let child = world.spawn((Node::default(), MinimapPart::Image)).id();
        world.entity_mut(surface).add_child(child);
        world.entity_mut(root).add_child(surface);
        let witness = Arc::new(());
        world.insert_resource(CampaignMinimap {
            witness: Some(Arc::downgrade(&witness)),
            root: Some(root),
            image_node: Some(child),
            image: Some(image.clone()),
            member: Some("old map".into()),
            attempted: true,
            pending_pointer: VecDeque::from([MinimapPress {
                surface,
                button: PointerButton::Primary,
                fraction: Some(Vec2::splat(0.25)),
            }]),
            drag: Some(surface),
            ..default()
        });
        world.run_system_once(project).unwrap();
        world.flush();
        assert!(world.get_entity(root).is_err());
        assert!(world.get_entity(child).is_err());
        assert!(world.get_entity(surface).is_err());
        assert!(world.resource::<Assets<Image>>().get(&image).is_none());
        let state = world.resource::<CampaignMinimap>();
        assert!(state.root.is_none() && state.image_node.is_none() && state.member.is_none());
        assert!(!state.attempted && !state.shown);
        assert!(state.pending_pointer.is_empty() && state.drag.is_none());
    }
    #[test]
    fn minimap_node_preserves_the_logical_viewport_relationships() {
        let node = node(
            pcx_play::inventory_layout::Rect {
                x: 0.1,
                y: 0.2,
                width: 0.4,
                height: 0.3,
            },
            [1., 2.],
        );
        assert_eq!(node.left, Val::Percent(10.));
        assert_eq!(node.bottom, Val::Percent(10.));
        assert_eq!(node.width, Val::Percent(40.));
        assert!(
            (match node.height {
                Val::Percent(value) => value,
                _ => panic!("relative height"),
            } - 15.)
                .abs()
                < 0.00001
        );
    }

    #[cfg(all(feature = "test-fixtures", not(target_arch = "wasm32")))]
    #[test]
    #[ignore = "requires selected PCX_NATIVE_CHAPTER_* inputs"]
    fn selected_chapter_minimap_pans_orders_cancels_and_rechecks_control_and_session() {
        use bevy::picking::{
            backend::HitData,
            events::Pointer,
            pointer::{Location, PointerButton, PointerId},
        };
        use pcx_script::{Host, Value};
        let mut app = crate::native_campaign::tests::started_app();
        let camera = app.world_mut().spawn_empty().id();
        app.insert_resource(CampaignCamera::active_for_test(camera))
            .init_resource::<CampaignMinimap>()
            .init_resource::<crate::campaign_hud::CampaignHud>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_observer(capture_press);
        let (witness, bounds, camera_bounds) = {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let witness = session.session_witness().unwrap();
            let objects = session.host().unwrap().objects_mut();
            objects
                .native("EnableUserControl", &[Value::Bool(true)])
                .unwrap();
            objects
                .native("EnableUserUI", &[Value::Bool(true)])
                .unwrap();
            objects
                .native("ShowInterface", &[Value::Bool(true), Value::Real(0.)])
                .unwrap();
            (
                witness,
                objects.vision_world_bounds().unwrap(),
                objects.camera_state().unwrap().bounds.unwrap(),
            )
        };
        let fog = FogImage {
            raster: crate::campaign_visibility::Raster::new(80, 40, 3200).unwrap(),
            alpha: pcx_objects::editor::minimap_fog::MinimapFogAlpha {
                masked: 7,
                fogged: 91,
                visible: 223,
            },
            original_alpha: Vec::new(),
            original_pixels: Vec::new(),
            viewer: None,
            width: 80,
            height: 40,
        };
        app.insert_resource(CampaignMinimap {
            witness: Some(witness.clone()),
            fog: Some(fog),
            ..default()
        });
        let surface = app
            .world_mut()
            .spawn((
                MinimapPart::Surface,
                RelativeCursorPosition {
                    cursor_over: true,
                    normalized: Some(Vec2::new(-0.25, 0.25)),
                },
            ))
            .id();
        let press_button = |world: &mut World, button| {
            world.trigger(PointerPress {
                entity: surface,
                pointer: Pointer::new(
                    PointerId::Mouse,
                    Location {
                        target: bevy::camera::NormalizedRenderTarget::None {
                            width: 1,
                            height: 1,
                        },
                        position: Vec2::ZERO,
                    },
                ),
                button,
                hit: HitData::new(camera, 0., None, None),
                count: 1,
            });
            world.flush();
        };
        let press = |world: &mut World| press_button(world, PointerButton::Primary);
        let target = |world: &mut World| {
            world
                .resource_mut::<MapScriptSession<MapStartupHost>>()
                .host()
                .unwrap()
                .objects()
                .camera_target()
                .unwrap()
        };
        let expected = |world: &World, fraction: [f32; 2]| {
            let mut point = world
                .resource::<CampaignMinimap>()
                .fog
                .as_ref()
                .unwrap()
                .raster
                .world_at(bounds, fraction)
                .unwrap();
            for axis in 0..2 {
                let corners = camera_bounds.as_chunks::<2>().0;
                point[axis] = point[axis].clamp(
                    corners
                        .iter()
                        .map(|c| c[axis])
                        .fold(f32::INFINITY, f32::min),
                    corners
                        .iter()
                        .map(|c| c[axis])
                        .fold(f32::NEG_INFINITY, f32::max),
                );
            }
            point
        };
        press(app.world_mut());
        app.world_mut().run_system_once(consume_pointer).unwrap();
        assert_eq!(target(app.world_mut()), expected(app.world(), [0.25, 0.75]));
        assert!(
            app.world()
                .resource::<crate::campaign_hud::CampaignHud>()
                .pointer_consumed
        );
        // Hold after a new press, then drag beyond the surface: camera bounds still win.
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        press(app.world_mut());
        app.world_mut().run_system_once(consume_pointer).unwrap();
        app.world_mut()
            .get_mut::<RelativeCursorPosition>(surface)
            .unwrap()
            .normalized = Some(Vec2::new(1., -1.));
        app.world_mut().run_system_once(consume_pointer).unwrap();
        let edge = target(app.world_mut());
        assert_eq!(edge, expected(app.world(), [1.5, -0.5]));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.world_mut()
            .get_mut::<RelativeCursorPosition>(surface)
            .unwrap()
            .normalized = Some(Vec2::ZERO);
        app.world_mut().run_system_once(consume_pointer).unwrap();
        assert_eq!(target(app.world_mut()), edge);
        assert!(app.world().resource::<CampaignMinimap>().drag.is_none());
        // The same consumer must issue owned point orders without moving the camera.
        use crate::campaign_input::CampaignInput;
        use pcx_play::standard_command::Kind as Standard;
        let seed = app
            .world()
            .resource::<peacecraft3::campaign_startup::CampaignSeed>()
            .prepared
            .unwrap();
        let (units, foreign, foreign_before, point, start) = {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let start = pcx_play::player_command_log::StartIdentity::from_context(
                seed,
                session.context().unwrap(),
            );
            let objects = session.host().unwrap().objects_mut();
            let local = objects.local_player_settings().unwrap().identity;
            let units: Vec<_> = objects
                .executed_units()
                .filter(|(id, u)| {
                    u.owner == local
                        && u.shown
                        && u.combat.as_ref().is_some_and(|c| !c.dead)
                        && [Standard::Move, Standard::Attack, Standard::Patrol]
                            .into_iter()
                            .all(|kind| {
                                objects
                                    .standard_command_order(*id, kind)
                                    .is_ok_and(|o| o.is_some())
                            })
                })
                .map(|(id, _)| id)
                .take(2)
                .collect();
            assert_eq!(units.len(), 2);
            let (foreign, record) = objects
                .executed_units()
                .find(|(_, u)| u.owner != local)
                .unwrap();
            let foreign_before = record.clone();
            objects.native("PauseGame", &[Value::Bool(false)]).unwrap();
            for unit in &units {
                objects
                    .native("PauseUnit", &[Value::Handle(*unit), Value::Bool(false)])
                    .unwrap();
            }
            let mut selection = units.clone();
            selection.push(foreign);
            objects.select_units(&selection, true);
            let [x, y] = objects.executed_unit(units[0]).unwrap().position.unwrap();
            (units, foreign, foreign_before, [x + 128., y], start)
        };
        app.world_mut()
            .resource_mut::<CampaignInput>()
            .commands
            .begin(start);
        let dimensions = {
            let fog = app
                .world()
                .resource::<CampaignMinimap>()
                .fog
                .as_ref()
                .unwrap();
            [fog.width as f32, fog.height as f32]
        };
        let extent = [bounds[1][0] - bounds[0][0], bounds[1][1] - bounds[0][1]];
        let scale = (dimensions[0] / extent[0]).min(dimensions[1] / extent[1]);
        let fraction = Vec2::new(
            ((point[0] - bounds[0][0]) * scale + (dimensions[0] - extent[0] * scale) * 0.5)
                / dimensions[0],
            ((bounds[1][1] - point[1]) * scale + (dimensions[1] - extent[1] * scale) * 0.5)
                / dimensions[1],
        );
        app.world_mut()
            .get_mut::<RelativeCursorPosition>(surface)
            .unwrap()
            .normalized = Some(fraction - Vec2::splat(0.5));
        press_button(app.world_mut(), PointerButton::Secondary);
        app.world_mut().run_system_once(consume_pointer).unwrap();
        assert_eq!(target(app.world_mut()), edge);
        assert!(app.world().resource::<CampaignMinimap>().drag.is_none());
        let entries = app.world().resource::<CampaignInput>().commands.entries();
        assert_eq!(
            entries.len(),
            2,
            "{:?}",
            app.world().resource::<CampaignInput>().last_error
        );
        for entry in entries {
            let pcx_play::player_command_log::PlayerCommand::Point { unit, x, y, .. } =
                entry.command
            else {
                panic!("point command")
            };
            assert!(units.contains(&unit));
            assert!((x - point[0]).abs() < 0.01 && (y - point[1]).abs() < 0.01);
        }
        assert_eq!(
            app.world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>()
                .host()
                .unwrap()
                .objects()
                .executed_unit(foreign)
                .unwrap(),
            &foreign_before
        );
        app.world_mut().run_system_once(consume_pointer).unwrap();
        assert_eq!(
            app.world()
                .resource::<CampaignInput>()
                .commands
                .entries()
                .len(),
            2
        );
        press_button(app.world_mut(), PointerButton::Secondary);
        press_button(app.world_mut(), PointerButton::Secondary);
        assert_eq!(
            app.world()
                .resource::<CampaignMinimap>()
                .pending_pointer
                .len(),
            2
        );
        app.world_mut().run_system_once(consume_pointer).unwrap();
        assert_eq!(
            app.world()
                .resource::<CampaignInput>()
                .commands
                .entries()
                .len(),
            6
        );
        for kind in [Standard::Move, Standard::Attack, Standard::Patrol] {
            app.world_mut()
                .resource_scope(|world, mut input: Mut<CampaignInput>| {
                    let mut session = world.resource_mut::<MapScriptSession<MapStartupHost>>();
                    crate::campaign_input::activate_standard(
                        session.host().unwrap(),
                        &mut input,
                        units[0],
                        kind,
                        witness.clone(),
                    )
                    .unwrap();
                });
            let before = app
                .world()
                .resource::<CampaignInput>()
                .commands
                .entries()
                .len();
            press(app.world_mut());
            app.world_mut().run_system_once(consume_pointer).unwrap();
            assert_eq!(
                app.world()
                    .resource::<CampaignInput>()
                    .commands
                    .entries()
                    .len(),
                before + 2
            );
            assert!(app.world().resource::<CampaignInput>().command.is_none());
            assert!(app.world().resource::<CampaignMinimap>().drag.is_none());
            assert_eq!(target(app.world_mut()), edge);
        }
        app.world_mut()
            .resource_scope(|world, mut input: Mut<CampaignInput>| {
                let mut session = world.resource_mut::<MapScriptSession<MapStartupHost>>();
                crate::campaign_input::activate_standard(
                    session.host().unwrap(),
                    &mut input,
                    units[0],
                    Standard::Move,
                    witness.clone(),
                )
                .unwrap();
            });
        let recorded = app
            .world()
            .resource::<CampaignInput>()
            .commands
            .entries()
            .len();
        let cursor = app
            .world_mut()
            .get_mut::<RelativeCursorPosition>(surface)
            .unwrap()
            .normalized
            .take();
        press_button(app.world_mut(), PointerButton::Secondary);
        app.world_mut().run_system_once(consume_pointer).unwrap();
        assert!(app.world().resource::<CampaignInput>().command.is_none());
        assert_eq!(
            app.world()
                .resource::<CampaignInput>()
                .commands
                .entries()
                .len(),
            recorded
        );
        app.world_mut()
            .get_mut::<RelativeCursorPosition>(surface)
            .unwrap()
            .normalized = cursor;
        app.world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .host()
            .unwrap()
            .objects_mut()
            .native("EnableUserControl", &[Value::Bool(false)])
            .unwrap();
        press(app.world_mut());
        app.world_mut().run_system_once(consume_pointer).unwrap();
        assert_eq!(target(app.world_mut()), edge);
        press_button(app.world_mut(), PointerButton::Secondary);
        app.world_mut().run_system_once(consume_pointer).unwrap();
        assert_eq!(
            app.world()
                .resource::<CampaignInput>()
                .commands
                .entries()
                .len(),
            recorded
        );
        app.world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .host()
            .unwrap()
            .objects_mut()
            .native("EnableUserControl", &[Value::Bool(true)])
            .unwrap();
        app.world_mut()
            .resource_mut::<crate::campaign_hud::CampaignHud>()
            .quest_log_open = true;
        press(app.world_mut());
        app.world_mut().run_system_once(consume_pointer).unwrap();
        assert_eq!(target(app.world_mut()), edge);
        app.world_mut()
            .resource_mut::<crate::campaign_hud::CampaignHud>()
            .quest_log_open = false;
        app.world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .host()
            .unwrap()
            .objects_mut()
            .native("EnableUserUI", &[Value::Bool(false)])
            .unwrap();
        press(app.world_mut());
        app.world_mut().run_system_once(consume_pointer).unwrap();
        assert_eq!(target(app.world_mut()), edge);
        app.world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .host()
            .unwrap()
            .objects_mut()
            .native("EnableUserUI", &[Value::Bool(true)])
            .unwrap();
        app.world_mut().resource_mut::<CampaignCamera>().suspend();
        press(app.world_mut());
        app.world_mut().run_system_once(consume_pointer).unwrap();
        assert_eq!(target(app.world_mut()), edge);
        app.insert_resource(CampaignCamera::active_for_test(camera));
        let stale = Arc::new(());
        app.world_mut().resource_mut::<CampaignMinimap>().witness = Some(Arc::downgrade(&stale));
        press(app.world_mut());
        app.world_mut().run_system_once(consume_pointer).unwrap();
        assert_eq!(target(app.world_mut()), edge);
        assert!(
            app.world()
                .resource::<CampaignMinimap>()
                .last_error
                .as_ref()
                .unwrap()
                .contains("retired")
        );
        press_button(app.world_mut(), PointerButton::Secondary);
        app.world_mut().run_system_once(consume_pointer).unwrap();
        assert_eq!(
            app.world()
                .resource::<CampaignInput>()
                .commands
                .entries()
                .len(),
            recorded
        );
    }
}

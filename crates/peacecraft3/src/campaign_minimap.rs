//! Receipt-bound minimap terrain. Visibility and overlays are separate consumers.
use bevy::{picking::events::PointerPress, prelude::*};
use pcx_play::{map_startup_host::MapStartupHost, minimap_layout::Rules};
use peacecraft3::{
    campaign_camera::{CampaignCamera, CampaignCameraSet},
    campaign_presentation::{CampaignHud, CampaignInterface, CampaignPresentationSet},
    map_requests::MapRequests,
    map_script_session::MapScriptSession,
};
use sha2::{Digest, Sha256};
use std::sync::{Arc, Weak};

// Application allocation limits, independent of authored map/image dimensions.
const IMAGE_BYTES: usize = 4 << 20;
const PIXEL_BYTES: usize = 16 << 20;
const FOG_ERROR: &str = "minimap fog-of-war projection is not yet supported";
const SETTINGS_ERROR: &str = "minimap requires admitted fog settings";
#[derive(Resource, Default)]
pub(crate) struct CampaignMinimap {
    witness: Option<Weak<()>>,
    attempted: bool,
    root: Option<Entity>,
    image_node: Option<Entity>,
    image: Option<Handle<Image>>,
    pending_pointer: bool,
    load_error: Option<String>,
    pub member: Option<String>,
    pub shown: bool,
    pub last_error: Option<String>,
}
#[derive(Component)]
enum MinimapPart {
    Surface,
    Image,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<CampaignMinimap>()
        .add_observer(capture_press)
        .add_systems(
            Update,
            project
                .after(CampaignCameraSet)
                .before(crate::campaign_hud::CampaignHudSet)
                .before(CampaignPresentationSet),
        )
        .add_systems(
            Update,
            consume_pointer
                .after(crate::campaign_hud::CampaignHudSet)
                .before(crate::campaign_input::CampaignInputSet),
        );
}
fn capture_press(
    event: On<PointerPress>,
    parts: Query<&MinimapPart>,
    mut state: ResMut<CampaignMinimap>,
    presentation: Option<Res<peacecraft3::campaign_presentation::CampaignPresentation>>,
) {
    if matches!(parts.get(event.entity), Ok(MinimapPart::Surface))
        && crate::button_actions::input_allowed(presentation)
    {
        state.pending_pointer = true;
    }
}
fn consume_pointer(
    mut state: ResMut<CampaignMinimap>,
    mut hud: ResMut<crate::campaign_hud::CampaignHud>,
) {
    if std::mem::take(&mut state.pending_pointer) {
        hud.pointer_consumed = true;
        state.last_error = Some("minimap camera and order projection is not yet supported".into());
    }
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
    let visible = input.camera.active() && matches!(visibility, Some(Ok(())));
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
    let mode_error = visibility.and_then(Result::err);
    if let Some(error) = &mode_error {
        state.last_error = Some(error.clone());
    } else if matches!(
        state.last_error.as_deref(),
        Some(FOG_ERROR | SETTINGS_ERROR)
    ) {
        state.last_error = state.load_error.clone();
    }
    if !input.camera.active() || state.attempted {
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
    let surface = commands
        .spawn((node(rules.frame, rules.viewport), MinimapPart::Surface))
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
    fn minimap_does_not_reveal_the_image_for_unknown_or_enabled_fog() {
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
    #[test]
    fn minimap_press_is_consumed_and_refused_instead_of_becoming_a_world_order() {
        let mut world = World::new();
        world.init_resource::<CampaignMinimap>();
        world.init_resource::<crate::campaign_hud::CampaignHud>();
        world.add_observer(capture_press);
        let image = world.spawn(MinimapPart::Surface).id();
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
        world.flush();
        world.run_system_once(consume_pointer).unwrap();
        assert!(
            world
                .resource::<crate::campaign_hud::CampaignHud>()
                .pointer_consumed
        );
        assert!(!world.resource::<CampaignMinimap>().pending_pointer);
        assert!(
            world
                .resource::<CampaignMinimap>()
                .last_error
                .as_ref()
                .unwrap()
                .contains("projection")
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
}

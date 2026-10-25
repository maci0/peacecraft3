//! Receipt-bound held item projection. Item actions are separate consumers.
use crate::{
    campaign_hud::upper_buttons::{image_sampler, read, skin_file, text},
    menu_skin::SkinTable,
};
use bevy::{
    ecs::system::SystemParam,
    picking::{events::PointerPress, hover::Hovered},
    prelude::*,
};
use pcx_play::{
    inventory_layout::{Rect as SourceRect, Rules},
    map_startup_host::MapStartupHost,
    neutral_player::RuleInputBudget,
    options_defaults::{Defaults, DiscoveryBudget},
    playercolor_host::ObjectHost,
    scripts::AssetIdentity,
};
use pcx_script::HandleId;
use peacecraft3::{
    browser_configuration::SelectedMapSources,
    campaign_camera::{CampaignCamera, CampaignCameraSet},
    campaign_presentation::{CampaignInterface, CampaignPresentationSet},
    map_binding::AssetReceipt,
    map_requests::MapRequests,
    map_script_session::MapScriptSession,
};
use std::{
    collections::HashMap,
    sync::{Arc, Weak},
};
const SKIN: &str = r"UI\war3skins.txt";
const GLOBALS: &str = r"UI\FrameDef\GlobalStrings.fdf";
const INFO_PANEL_STRINGS: &str = r"UI\FrameDef\InfoPanelStrings.fdf";
const MISC_UI: &str = r"UI\MiscUI.txt";
const CONSOLE: &str = r"UI\FrameDef\UI\ConsoleUI.fdf";
// Preference name the menu stores the HUD tooltip checkbox under.
const TOOLTIP_TOGGLE: &str = "TooltipsCheckBox";
// Application budgets, unrelated to authored image dimensions or slot counts.
const MAX_PAYLOAD: usize = 64 << 20;
const MAX_FAILED_IMAGES: usize = 64;
const DISCOVERY: DiscoveryBudget = DiscoveryBudget {
    image: RuleInputBudget {
        image_bytes: 32 << 20,
        profile_bytes: 128 << 10,
        sections: 96,
    },
    string_bytes: 4096,
    candidate_limit: 32,
    work_bytes: 64 << 20,
};
#[derive(Resource, Default)]
pub(crate) struct CampaignInventory {
    pub pointer_consumed: bool,
    pub last_error: Option<String>,
    pub receipts: Vec<AssetReceipt>,
    /// Holder and slot index of a slot press that asks to activate the slot's item.
    pub use_request: Option<(HandleId, usize)>,
    pending_pointer: bool,
    witness: Option<Weak<()>>,
    attempted: bool,
    root: Option<Entity>,
    parent: Option<Entity>,
    source: Option<Source>,
    view: Option<View>,
    tooltip: Option<TooltipView>,
    action_error: Option<String>,
}
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct CampaignInventorySet;
#[derive(Component)]
struct InventoryRoot;
#[derive(Component)]
struct InventorySlot(usize);
#[derive(Component, Clone, PartialEq)]
pub(crate) struct ControlTooltip {
    pub name: String,
    pub ubertip: String,
}
#[derive(Component, Clone, Copy)]
pub(crate) struct TooltipAnchor {
    pub rect: SourceRect,
    pub viewport: [f32; 2],
}
#[derive(Component)]
pub(crate) struct TooltipFailure(pub String);
#[derive(Clone, Debug, PartialEq)]
struct ItemCard {
    id: HandleId,
    art: String,
    name: String,
    ubertip: String,
}
#[derive(Clone, Debug, PartialEq)]
struct View {
    unit: HandleId,
    capacity: usize,
    items: Vec<Option<ItemCard>>,
}
struct Source {
    rules: Rules,
    console_frame: String,
    label: String,
    font: Handle<Font>,
    font_height: f32,
    disabled: String,
    cover: String,
    images: HashMap<(String, bool), Handle<Image>>,
    sizes: HashMap<(String, bool), Vec2>,
    failed_images: HashMap<(String, bool), String>,
    tooltip_font: Handle<Font>,
    tooltip_name_height: f32,
    tooltip_description_height: f32,
    tooltip_background: String,
    tooltip_edge: String,
    tooltip_materials:
        HashMap<crate::glue_menu::EdgePiece, Handle<crate::glue_menu::BackdropImage>>,
    /// Source fallback for the description checkbox, used when no saved value applies.
    tooltip_seed: Result<bool, String>,
    retained: usize,
    receipts: Vec<AssetReceipt>,
}
#[derive(SystemParam)]
struct Inputs<'w, 's> {
    requests: Res<'w, MapRequests>,
    sources: Res<'w, SelectedMapSources>,
    camera: Res<'w, CampaignCamera>,
    launcher: Option<Res<'w, crate::launch_gui::Launcher>>,
    frames: Query<'w, 's, (Entity, &'static crate::hud_render::HudSourceAttachment)>,
}
#[derive(SystemParam)]
struct Assets<'w> {
    fonts: ResMut<'w, bevy::prelude::Assets<Font>>,
    images: ResMut<'w, bevy::prelude::Assets<Image>>,
    materials: ResMut<'w, bevy::prelude::Assets<crate::glue_menu::BackdropImage>>,
}
pub(crate) fn install(app: &mut App) {
    app.init_resource::<CampaignInventory>()
        .add_observer(capture_press)
        .add_systems(
            Update,
            (capture, project, refresh_tooltip)
                .chain()
                .in_set(CampaignInventorySet)
                .after(crate::hud_render::HudRenderSet)
                .after(CampaignCameraSet)
                .before(CampaignPresentationSet),
        )
        .add_systems(
            PostUpdate,
            measure_tooltip.after(bevy::ui::UiSystems::PostLayout),
        );
}
fn capture(mut state: ResMut<CampaignInventory>) {
    state.pointer_consumed = std::mem::take(&mut state.pending_pointer);
}
fn capture_press(
    event: On<PointerPress>,
    slots: Query<(Option<&ControlTooltip>, &InventorySlot)>,
    mut state: ResMut<CampaignInventory>,
    presentation: Option<Res<peacecraft3::campaign_presentation::CampaignPresentation>>,
) {
    let Ok((item, slot)) = slots.get(event.entity) else {
        return;
    };
    if crate::button_actions::input_allowed(presentation) {
        state.pending_pointer = true;
        if item.is_some()
            && let Some(unit) = state.view.as_ref().map(|view| view.unit)
        {
            state.use_request = Some((unit, slot.0));
        }
    }
}
fn image(
    reader: &pcx_assets::Resolver,
    path: &str,
    repeat: bool,
    source: &mut Source,
    images: &mut bevy::prelude::Assets<Image>,
) -> Result<Handle<Image>, String> {
    let key = (path.to_owned(), repeat);
    if let Some(image) = source.images.get(&key) {
        return Ok(image.clone());
    }
    if let Some(error) = source.failed_images.get(&key) {
        return Err(error.clone());
    }
    decode_image(reader, path, repeat, source, images).inspect_err(|error| {
        if source.failed_images.len() < MAX_FAILED_IMAGES {
            source.failed_images.insert(key, error.clone());
        }
    })
}
fn decode_image(
    reader: &pcx_assets::Resolver,
    path: &str,
    repeat: bool,
    source: &mut Source,
    images: &mut bevy::prelude::Assets<Image>,
) -> Result<Handle<Image>, String> {
    let mut receipts = Vec::new();
    let bytes = read(reader, path, 4 << 20, &mut receipts)?;
    let remaining = MAX_PAYLOAD
        .checked_sub(source.retained)
        .ok_or("inventory image aggregate budget")?;
    let texture = pcx_gfx::load_image_bounded(&bytes, path, (16 << 20).min(remaining))
        .map_err(|e| e.to_string())?;
    source.retained = source
        .retained
        .checked_add(
            texture
                .rgba_bytes()
                .ok_or("invalid inventory image payload")?,
        )
        .filter(|n| *n <= MAX_PAYLOAD)
        .ok_or("inventory image aggregate budget")?;
    let mut image = pcx_gfx::viewer::rgba_image(&texture);
    image.sampler = bevy::image::ImageSampler::Descriptor(image_sampler(repeat));
    // Receipts describe only assets that became part of the projection.
    source.receipts.extend(receipts);
    let handle = images.add(image);
    let key = (path.to_owned(), repeat);
    source.sizes.insert(
        key.clone(),
        Vec2::new(texture.width as f32, texture.height as f32),
    );
    source.images.insert(key, handle.clone());
    Ok(handle)
}
fn load(
    reader: &pcx_assets::Resolver,
    rules: Rules,
    tooltip_seed: Result<bool, String>,
    race: Result<Option<String>, String>,
    fonts: &mut bevy::prelude::Assets<Font>,
) -> Result<Source, String> {
    let mut receipts = Vec::new();
    let skin = SkinTable::parse(&text(reader, SKIN, &mut receipts)?).map_err(|e| e.to_string())?;
    let sections = crate::campaign_hud::race_sections(&skin, race?)?;
    let console_frame = skin_file(&skin, &sections, &rules.console_key)?;
    // This receipt is also the existing mounted parent's authored frame source.
    text(reader, CONSOLE, &mut receipts)?;
    let globals = text(reader, GLOBALS, &mut receipts)?;
    let info_panel = text(reader, INFO_PANEL_STRINGS, &mut receipts)?;
    let label = source_label(&[&globals, &info_panel], &rules.label_key)?;
    let profile =
        SkinTable::parse(&text(reader, MISC_UI, &mut receipts)?).map_err(|e| e.to_string())?;
    let height = |key: &str| -> Result<f32, String> {
        let values = profile
            .sections()
            .iter()
            .filter(|s| s.name.eq_ignore_ascii_case(&rules.font_section))
            .flat_map(|s| s.entries.iter())
            .filter(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v)
            .collect::<Vec<_>>();
        let [value] = values.as_slice() else {
            return Err(format!("missing or ambiguous font height {key}"));
        };
        value
            .parse::<f32>()
            .ok()
            .filter(|f| f.is_finite() && *f > 0.)
            .ok_or_else(|| format!("invalid font height {key}"))
    };
    let font_height = height(&rules.font_height_key)?;
    let tooltip_name_height = height(&rules.tooltip.name_height_key)?;
    let tooltip_description_height = height(&rules.tooltip.description_height_key)?;
    // Every read and check precedes `fonts.add`, so a refused load leaves no font asset.
    let font_path = skin_file(&skin, &sections, &rules.font_key)?;
    let font_bytes = read(reader, &font_path, 16 << 20, &mut receipts)?;
    if !crate::hud_render::font_registers(&font_bytes) {
        return Err(format!("{font_path}: unreadable inventory font"));
    }
    let tip_font_path = skin_file(&skin, &sections, &rules.tooltip.font_key)?;
    let tip_font_bytes = if tip_font_path == font_path {
        None
    } else {
        let bytes = read(reader, &tip_font_path, 16 << 20, &mut receipts)?;
        if !crate::hud_render::font_registers(&bytes) {
            return Err(format!("{tip_font_path}: unreadable tooltip font"));
        }
        Some(bytes)
    };
    let retained = font_bytes.len() + tip_font_bytes.as_ref().map_or(0, Vec::len);
    if retained > MAX_PAYLOAD {
        return Err("inventory font aggregate budget".into());
    }
    let tooltip_background = skin_file(&skin, &sections, &rules.tooltip.background_key)?;
    let tooltip_edge = skin_file(&skin, &sections, &rules.tooltip.edge_key)?;
    let disabled = skin_file(&skin, &sections, &rules.disabled_key)?;
    let cover = skin_file(&skin, &sections, &rules.cover_key)?;
    // Low bits are the native font format's left/center/right justification mask.
    if !matches!(rules.label_flags & 7, 1 | 2 | 4) {
        return Err("unsupported inventory font justification".into());
    }
    let font = fonts.add(Font::from_bytes(font_bytes));
    let tooltip_font = match tip_font_bytes {
        Some(bytes) => fonts.add(Font::from_bytes(bytes)),
        None => font.clone(),
    };
    Ok(Source {
        rules,
        console_frame,
        label,
        font,
        font_height,
        disabled,
        cover,
        images: HashMap::new(),
        sizes: HashMap::new(),
        failed_images: HashMap::new(),
        tooltip_font,
        tooltip_name_height,
        tooltip_description_height,
        tooltip_background,
        tooltip_edge,
        tooltip_materials: HashMap::new(),
        tooltip_seed,
        retained,
        receipts,
    })
}
/// The pushed label key names one record across every selected StringList. Identifiers
/// are unique across lists (InfoPanelStrings.fdf header), so a second match is refused.
fn source_label(lists: &[&str], key: &str) -> Result<String, String> {
    let mut found = lists
        .iter()
        .filter_map(|list| crate::menu_text::global_string(list, key));
    let label = found
        .next()
        .ok_or_else(|| format!("missing source inventory label {key}"))?;
    if found.next().is_some() {
        return Err(format!("ambiguous source inventory label {key}"));
    }
    Ok(label)
}
/// Source fallback for the descriptor at `index`, the operand the native tooltip
/// checkbox read selects. Only the exact zero/one profile is admitted.
fn tooltip_seed(image: &[u8], origin: AssetIdentity, index: u32) -> Result<bool, String> {
    let defaults = Defaults::discover(image, origin, DISCOVERY).map_err(|e| e.to_string())?;
    let raw = defaults
        .entries()
        .iter()
        .find(|e| e.index == index)
        .ok_or("tooltip preference index is absent from the descriptor table")?
        .fallback;
    checkbox(raw)
}
fn checkbox(raw: &[u8]) -> Result<bool, String> {
    match raw {
        b"0" => Ok(false),
        b"1" => Ok(true),
        _ => Err("tooltips fallback is outside the zero/one checkbox profile".into()),
    }
}
/// A saved value wins; otherwise the source fallback seeds the checkbox.
fn effective_verbose(saved: Option<bool>, seed: &Result<bool, String>) -> Result<bool, String> {
    match saved {
        Some(saved) => Ok(saved),
        None => seed.clone(),
    }
}
fn field(definition: &pcx_sim::world::Definition, key: &str) -> Result<String, String> {
    use pcx_objects::editor::{FieldKind, FieldSource, SlkId};
    let found=definition.fields.iter().filter(|(id,f)|f.metadata.as_ref().is_some_and(|m|m.fourcc==**id&&m.from==SlkId::UnitMetaData&&m.kind==FieldKind::Token&&matches!(&m.source,FieldSource::Profile{key:k,..}if k.eq_ignore_ascii_case(key)))).collect::<Vec<_>>();
    let [(_, field)] = found.as_slice() else {
        return Err(format!("item {key}: missing or ambiguous profile metadata"));
    };
    let value = match field.value.as_ref() {
        Ok(pcx_sim::world::FieldValue::Cell(s)) => s.clone(),
        Ok(pcx_sim::world::FieldValue::Override(pcx_objects::Value::Str(s))) => {
            std::str::from_utf8(s)
                .map_err(|e| e.to_string())?
                .to_owned()
        }
        other => return Err(format!("item {key}: unsupported resolved value {other:?}")),
    };
    if value.starts_with("TRIGSTR_") {
        return Err(format!("item {key}: unresolved map string {value}"));
    }
    Ok(value)
}
fn wanted(objects: &ObjectHost, count: usize) -> Result<Option<View>, String> {
    let [unit] = objects.selection() else {
        return Ok(None);
    };
    let policy = objects.inventory_policy(*unit).map_err(|e| e.to_string())?;
    if policy.slots > count {
        return Err("inventory capacity exceeds source HUD button count".into());
    }
    let mut items = vec![None; count];
    for (id, item) in objects.executed_items() {
        let Some((holder, slot)) = item.held_by.filter(|(holder, _)| holder == unit) else {
            continue;
        };
        let _ = holder;
        if slot >= policy.slots || items[slot].is_some() {
            return Err("canonical inventory slot ownership is inconsistent".into());
        }
        let art = field(&item.definition, "art")?;
        pcx_assets::AssetPath::new(&art).map_err(|e| e.to_string())?;
        items[slot] = Some(ItemCard {
            id,
            art,
            name: field(&item.definition, "name")?,
            ubertip: field(&item.definition, "ubertip")?,
        });
    }
    Ok(Some(View {
        unit: *unit,
        capacity: policy.slots,
        items,
    }))
}
fn node(rect: SourceRect, viewport: [f32; 2]) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Percent(rect.x / viewport[0] * 100.),
        bottom: Val::Percent(rect.y / viewport[1] * 100.),
        width: Val::Percent(rect.width / viewport[0] * 100.),
        height: Val::Percent(rect.height / viewport[1] * 100.),
        ..default()
    }
}
/// Scales the normalized `[left, top, right, bottom]` source rectangle to decoded pixels.
fn cover_source_rect(uv: [f32; 4], size: Vec2) -> bevy::math::Rect {
    bevy::math::Rect::new(
        uv[0] * size.x,
        uv[1] * size.y,
        uv[2] * size.x,
        uv[3] * size.y,
    )
}
fn colour(argb: u32) -> Color {
    let [a, r, g, b] = argb.to_be_bytes();
    Color::srgba_u8(r, g, b, a)
}
fn paint(
    commands: &mut Commands,
    root: Entity,
    source: &Source,
    view: &View,
) -> Result<(), String> {
    commands.entity(root).despawn_children();
    if view.capacity == 0 {
        let key = (source.cover.clone(), source.rules.cover_repeat);
        let image = source
            .images
            .get(&key)
            .ok_or("inventory cover unavailable")?;
        let size = source
            .sizes
            .get(&key)
            .ok_or("inventory cover size unavailable")?;
        let mut cover = ImageNode::new(image.clone());
        cover.rect = Some(cover_source_rect(source.rules.cover_uv, *size));
        let child = commands
            .spawn((
                node(source.rules.cover(), source.rules.viewport),
                cover,
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(root).add_child(child);
        return Ok(());
    }
    let label = commands
        .spawn((
            node(source.rules.label(), source.rules.viewport),
            Text::new(source.label.clone()),
            TextFont {
                font: source.font.clone().into(),
                font_size: bevy::text::FontSize::Vh(
                    source.font_height / source.rules.viewport[1] * 100.,
                ),
                ..default()
            },
            TextColor(colour(source.rules.label_colour)),
            TextLayout::new(
                match source.rules.label_flags & 7 {
                    1 => bevy::text::Justify::Left,
                    2 => bevy::text::Justify::Center,
                    _ => bevy::text::Justify::Right,
                },
                bevy::text::LineBreak::NoWrap,
            ),
            TextShadow {
                offset: Vec2::ZERO,
                color: colour(source.rules.shadow_colour),
            },
            crate::glue_menu::FrameShadow(Vec2::from(source.rules.shadow_offset)),
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(root).add_child(label);
    for (index, item) in view.items.iter().enumerate() {
        let rect = source.rules.slot(index)?;
        let slot = commands
            .spawn((
                node(rect, source.rules.viewport),
                InventorySlot(index),
                TooltipAnchor {
                    rect,
                    viewport: source.rules.viewport,
                },
                Hovered::default(),
            ))
            .id();
        if let Some(item) = item {
            debug!(
                "inventory held slot={} slot_rect={:?} viewport={:?} art={:?}",
                index, rect, source.rules.viewport, item.art
            );
            commands.entity(slot).insert((
                ImageNode::new(
                    source.images[&(item.art.clone(), source.rules.icon_repeat)].clone(),
                ),
                ControlTooltip {
                    name: item.name.clone(),
                    ubertip: item.ubertip.clone(),
                },
            ));
        } else if index >= view.capacity {
            commands.entity(slot).insert(ImageNode::new(
                source.images[&(source.disabled.clone(), source.rules.icon_repeat)].clone(),
            ));
        }
        commands.entity(root).add_child(slot);
    }
    Ok(())
}
fn project(
    mut commands: Commands,
    mut state: ResMut<CampaignInventory>,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
    inputs: Inputs,
    mut assets: Assets,
    nodes: Query<&Node>,
) {
    let witness = session.session_witness();
    if !match (&state.witness, &witness) {
        (Some(a), Some(b)) => Weak::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    } {
        clear_tooltip(&mut commands, &mut state);
        if let Some(root) = state.root {
            commands.entity(root).try_despawn();
        }
        if let Some(source) = state.source.take() {
            assets.fonts.remove(source.font.id());
            assets.fonts.remove(source.tooltip_font.id());
            for material in source.tooltip_materials.values() {
                assets.materials.remove(material.id());
            }
            for image in source.images.values() {
                assets.images.remove(image.id());
            }
        }
        *state = CampaignInventory {
            witness,
            ..default()
        };
        return;
    }
    let Some(bindings) = inputs.requests.bindings.as_ref().filter(|b| {
        inputs
            .requests
            .published_receipts
            .as_ref()
            .is_some_and(|r| Arc::ptr_eq(r, &b.receipts))
            && inputs
                .requests
                .admission
                .shared_context()
                .as_ref()
                .zip(session.context())
                .is_some_and(|(a, b)| Arc::ptr_eq(a, b))
    }) else {
        return;
    };
    let Some(reader) = inputs.sources.for_receipts(&bindings.receipts) else {
        return;
    };
    let Some(host) = session.host() else {
        return;
    };
    if !state.attempted {
        state.attempted = true;
        let install = inputs
            .launcher
            .as_deref()
            .and_then(crate::launch_gui::selected_install);
        let loaded =
            crate::menu_lighting::with_client_image(install.as_deref(), |source, image| {
                let origin = AssetIdentity {
                    source: source.into(),
                    edition: "selected campaign client".into(),
                    version: "reviewed Legacy 1.29.2.9231 inventory layout image".into(),
                };
                let rules = Rules::reviewed_legacy(image, origin.clone())?;
                let seed = tooltip_seed(image, origin, rules.tooltip.preference_index);
                Ok((rules, seed))
            })
            .and_then(|(rules, seed)| {
                load(
                    &reader,
                    rules,
                    seed,
                    host.objects()
                        .local_race_preference_constant()
                        .map_err(|e| e.to_string()),
                    &mut assets.fonts,
                )
            });
        match loaded {
            Ok(source) => state.source = Some(source),
            Err(error) => {
                state.last_error = Some(error);
                return;
            }
        }
    }
    let Some(mut source) = state.source.take() else {
        return;
    };
    let parents = inputs
        .frames
        .iter()
        .filter(|(_, a)| {
            a.frame == source.console_frame
                && a.source_member == CONSOLE
                && a.receipt_path == bindings.receipts.map.path.as_str()
                && a.binding_revision == bindings.revision
        })
        .map(|(e, _)| e)
        .collect::<Vec<_>>();
    let result = (|| {
        let [parent] = parents.as_slice() else {
            return Err("inventory awaits exact receipt-bound ConsoleUI parent".into());
        };
        if state.parent != Some(*parent) || state.root.is_none_or(|root| nodes.get(root).is_err()) {
            for (path, repeat) in [
                (
                    source.tooltip_background.clone(),
                    source.rules.tooltip.background_repeat,
                ),
                (
                    source.tooltip_edge.clone(),
                    source.rules.tooltip.edge_repeat,
                ),
            ] {
                image(&reader, &path, repeat, &mut source, &mut assets.images)?;
            }
            clear_tooltip(&mut commands, &mut state);
            if let Some(root) = state.root {
                commands.entity(root).try_despawn();
            }
            let root = commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        width: Val::Percent(100.),
                        height: Val::Percent(100.),
                        ..default()
                    },
                    InventoryRoot,
                    CampaignInterface,
                    Pickable::IGNORE,
                ))
                .id();
            commands.entity(*parent).add_child(root);
            state.root = Some(root);
            state.parent = Some(*parent);
            state.view = None;
        }
        let desired = wanted(host.objects(), source.rules.count)?;
        let root = state.root.expect("inventory root");
        commands.entity(root).insert(Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.),
            height: Val::Percent(100.),
            display: if inputs.camera.active() && desired.is_some() {
                Display::Flex
            } else {
                Display::None
            },
            ..default()
        });
        if desired != state.view {
            if let Some(view) = &desired {
                let paths = if view.capacity == 0 {
                    vec![(source.cover.clone(), source.rules.cover_repeat)]
                } else {
                    std::iter::once((source.disabled.clone(), source.rules.icon_repeat))
                        .chain(
                            view.items
                                .iter()
                                .flatten()
                                .map(|i| (i.art.clone(), source.rules.icon_repeat)),
                        )
                        .collect()
                };
                for (path, repeat) in paths {
                    image(&reader, &path, repeat, &mut source, &mut assets.images)?;
                }
                paint(&mut commands, root, &source, view)?;
            }
            state.view = desired;
        }
        Ok(())
    })();
    if result.is_err()
        && let Some(root) = state.root
    {
        commands.entity(root).insert(Node {
            display: Display::None,
            ..default()
        });
    }
    state.last_error = result.err().or_else(|| state.action_error.clone());
    state.receipts = source.receipts.clone();
    state.source = Some(source);
}

struct TooltipView {
    owner: Entity,
    record: ControlTooltip,
    verbose: Option<bool>,
    root: Entity,
    name: Entity,
    description: Option<Entity>,
    slot: SourceRect,
    width: Option<f32>,
    ready: bool,
}
#[derive(Component)]
struct EmptyTooltipText;

fn clear_tooltip(commands: &mut Commands, state: &mut CampaignInventory) {
    if let Some(tip) = state.tooltip.take() {
        commands.entity(tip.root).try_despawn();
    }
}
fn justified(flags: u32) -> Result<bevy::text::Justify, String> {
    match flags & 7 {
        1 => Ok(bevy::text::Justify::Left),
        2 => Ok(bevy::text::Justify::Center),
        4 => Ok(bevy::text::Justify::Right),
        _ => Err("unsupported native inventory text justification".into()),
    }
}
fn text_block(
    commands: &mut Commands,
    parent: Entity,
    raw: &str,
    font: Handle<Font>,
    height: f32,
    base: Color,
    justify: bevy::text::Justify,
) -> Result<Entity, String> {
    let parsed = crate::rich_text::parse(raw).map_err(|e| e.to_string())?;
    if !parsed.unsupported.is_empty() {
        return Err("inventory tooltip contains unsupported substitutions".into());
    }
    let empty = parsed.spans.iter().all(|span| span.text.trim().is_empty());
    let style = TextFont {
        font: font.into(),
        font_size: bevy::text::FontSize::Vh(height * 100.),
        ..default()
    };
    let root = commands
        .spawn((
            Node::default(),
            Text::default(),
            style.clone(),
            TextColor(base),
            TextLayout::new(justify, bevy::text::LineBreak::NoWrap),
            Pickable::IGNORE,
        ))
        .id();
    if empty {
        commands.entity(root).insert(EmptyTooltipText);
    }
    for run in parsed.spans {
        let c = run
            .colour
            .map_or(base, |c| Color::srgba(c[0], c[1], c[2], c[3]));
        let span = commands
            .spawn((TextSpan::new(run.text), style.clone(), TextColor(c)))
            .id();
        commands.entity(root).add_child(span);
    }
    commands.entity(parent).add_child(root);
    Ok(root)
}
type HoveredControls<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static TooltipAnchor,
        Option<&'static ControlTooltip>,
        Option<&'static TooltipFailure>,
        &'static Hovered,
    ),
>;

fn refresh_tooltip(
    mut commands: Commands,
    mut state: ResMut<CampaignInventory>,
    camera: Res<CampaignCamera>,
    navigation: Res<crate::menu_screens::Navigation>,
    hovered: HoveredControls,
) {
    let desired = camera
        .active()
        .then(|| hovered.iter().find(|(_, _, _, _, hover)| hover.get()))
        .flatten();
    let Some((owner, anchor, record, failure, _)) = desired else {
        clear_tooltip(&mut commands, &mut state);
        return;
    };
    if let Some(failure) = failure {
        clear_tooltip(&mut commands, &mut state);
        state.last_error = Some(format!("control tooltip: {}", failure.0));
        return;
    }
    let Some(record) = record else {
        clear_tooltip(&mut commands, &mut state);
        return;
    };
    let seed = state
        .source
        .as_ref()
        .map_or(Err("inventory source unavailable".into()), |s| {
            s.tooltip_seed.clone()
        });
    let verbose = effective_verbose(
        navigation.applied.toggles.get(TOOLTIP_TOGGLE).copied(),
        &seed,
    );
    if let Err(error) = &verbose {
        state.last_error = Some(format!(
            "inventory tooltip description preference is unavailable ({error}); only its name is projected"
        ));
    }
    let verbose = verbose.ok();
    if state
        .tooltip
        .as_ref()
        .is_some_and(|tip| tip.owner == owner && tip.record == *record && tip.verbose == verbose)
    {
        return;
    }
    clear_tooltip(&mut commands, &mut state);
    let Some(source) = state.source.as_ref() else {
        return;
    };
    let Some(parent) = state.parent else {
        return;
    };
    if anchor.viewport != source.rules.viewport {
        state.last_error = Some("control tooltip viewport differs from its source style".into());
        return;
    }
    let tooltip = &source.rules.tooltip;
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                flex_direction: FlexDirection::Column,
                ..default()
            },
            Visibility::Hidden,
            ZIndex(1),
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(parent).add_child(root);
    let result = (|| {
        let name = text_block(
            &mut commands,
            root,
            &record.name,
            source.tooltip_font.clone(),
            source.tooltip_name_height / source.rules.viewport[1],
            colour(tooltip.name_colour),
            justified(u32::from(tooltip.name_flags))?,
        )?;
        let description = if verbose == Some(true) && !record.ubertip.is_empty() {
            Some(text_block(
                &mut commands,
                root,
                &record.ubertip,
                source.tooltip_font.clone(),
                source.tooltip_description_height / source.rules.viewport[1],
                colour(tooltip.description_colour),
                justified(u32::from(tooltip.description_flags))?,
            )?)
        } else {
            None
        };
        Ok(TooltipView {
            owner,
            record: record.clone(),
            verbose,
            root,
            name,
            description,
            slot: anchor.rect,
            width: None,
            ready: false,
        })
    })();
    match result {
        Ok(tip) => {
            debug!(
                "control tooltip slot_rect={:?} viewport={:?} name={:?} description={} verbose={:?}",
                tip.slot,
                source.rules.viewport,
                tip.record.name,
                tip.description.is_some(),
                tip.verbose
            );
            state.tooltip = Some(tip);
        }
        Err(error) => {
            commands.entity(root).try_despawn();
            state.last_error = Some(error);
        }
    }
}
fn relative(rect: crate::fdf::Rect, size: Vec2) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Percent(rect.left / size.x * 100.),
        bottom: Val::Percent(rect.bottom / size.y * 100.),
        width: Val::Percent(rect.width / size.x * 100.),
        height: Val::Percent(rect.height / size.y * 100.),
        ..default()
    }
}
fn tooltip_art(
    commands: &mut Commands,
    parent: Entity,
    source: &mut Source,
    size: Vec2,
    images: &bevy::prelude::Assets<Image>,
    materials: &mut bevy::prelude::Assets<crate::glue_menu::BackdropImage>,
) -> Result<(), String> {
    use crate::glue_menu::{self, EdgePiece};
    let t = &source.rules.tooltip;
    let inset = t.inset;
    let background = source
        .images
        .get(&(source.tooltip_background.clone(), t.background_repeat))
        .ok_or("tooltip background is unavailable")?;
    let inner = crate::fdf::Rect {
        left: inset,
        bottom: inset,
        width: size.x - inset * 2.,
        height: size.y - inset * 2.,
    };
    if inner.width <= 0. || inner.height <= 0. {
        return Err("tooltip background insets exceed measured extent".into());
    }
    let child = commands
        .spawn((
            relative(inner, size),
            ImageNode::new(background.clone()).with_mode(bevy::ui::widget::NodeImageMode::Stretch),
            ZIndex(-1),
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(parent).add_child(child);
    let edge = source
        .images
        .get(&(source.tooltip_edge.clone(), t.edge_repeat))
        .ok_or("tooltip edge is unavailable")?;
    let image = images.get(edge).ok_or("tooltip edge image was removed")?;
    for (index, piece) in EdgePiece::ALL.into_iter().enumerate() {
        if t.edge_flags & (1 << index) == 0 {
            continue;
        }
        let rect = glue_menu::edge_rect(piece, size, t.edge_size);
        if rect.width <= 0. || rect.height <= 0. {
            continue;
        }
        let uv = glue_menu::edge_source_rect(piece, image.width(), image.height())
            .map_err(|e| e.to_string())?;
        let mut child = commands.spawn((relative(rect, size), ZIndex(-1), Pickable::IGNORE));
        if matches!(piece, EdgePiece::Top | EdgePiece::Bottom) {
            let material = source
                .tooltip_materials
                .entry(piece)
                .or_insert_with(|| {
                    materials.add(glue_menu::horizontal_edge_material(
                        edge.clone(),
                        uv,
                        Vec2::new(image.width() as f32, image.height() as f32),
                        false,
                    ))
                })
                .clone();
            child.insert(MaterialNode(material));
        } else {
            let mut node =
                ImageNode::new(edge.clone()).with_mode(bevy::ui::widget::NodeImageMode::Stretch);
            node.rect = Some(uv);
            child.insert(node);
        }
        let child = child.id();
        commands.entity(parent).add_child(child);
    }
    Ok(())
}
#[derive(SystemParam)]
struct TooltipAssets<'w> {
    images: Res<'w, bevy::prelude::Assets<Image>>,
    materials: ResMut<'w, bevy::prelude::Assets<crate::glue_menu::BackdropImage>>,
}
fn measure_tooltip(
    mut commands: Commands,
    mut state: ResMut<CampaignInventory>,
    text: Query<(
        &bevy::text::TextLayoutInfo,
        &bevy::ui::ComputedUiRenderTargetInfo,
        Has<EmptyTooltipText>,
    )>,
    mut assets: TooltipAssets,
) {
    let Some(mut tip) = state.tooltip.take() else {
        return;
    };
    let Some(mut source) = state.source.take() else {
        state.tooltip = Some(tip);
        return;
    };
    let result = (|| {
        let Ok((title, target, empty)) = text.get(tip.name) else {
            return Ok(());
        };
        let pixels = target.physical_size().as_vec2();
        if pixels.x <= 0. || pixels.y <= 0. || (!empty && title.glyphs.is_empty()) {
            return Ok(());
        }
        let scale = Vec2::from(source.rules.viewport) / pixels;
        let name = if empty {
            Vec2::ZERO
        } else {
            title.size * scale
        };
        let description = match tip.description {
            Some(entity) => {
                let Ok((info, _, empty)) = text.get(entity) else {
                    return Ok(());
                };
                if empty {
                    None
                } else if info.glyphs.is_empty() {
                    return Ok(());
                } else {
                    Some(info.size * scale)
                }
            }
            None => None,
        };
        let t = &source.rules.tooltip;
        if tip.width.is_none() {
            let content = name
                .x
                .max(description.map_or(0., |size| size.x.min(t.description_cap)));
            let width = content + t.outer_padding;
            commands.entity(tip.root).insert(Node {
                position_type: PositionType::Absolute,
                width: Val::Vh(width / source.rules.viewport[1] * 100.),
                flex_direction: FlexDirection::Column,
                ..default()
            });
            if let Some(entity) = tip.description {
                commands.entity(entity).insert((
                    Node {
                        width: Val::Vh(content / source.rules.viewport[1] * 100.),
                        ..default()
                    },
                    TextLayout::new(
                        justified(u32::from(t.description_flags))?,
                        bevy::text::LineBreak::WordBoundary,
                    ),
                ));
            }
            tip.width = Some(width);
            return Ok(());
        }
        let width = tip.width.expect("measured tooltip width");
        let height =
            name.y + description.map_or(0., |size| size.y + t.height_gap) + t.outer_padding;
        let rect = source.rules.tooltip_rect(tip.slot, width, height)?;
        commands
            .entity(tip.root)
            .insert(node(rect, source.rules.viewport));
        commands.entity(tip.name).insert(relative(
            crate::fdf::Rect {
                left: t.title_inset[0],
                bottom: height - t.title_inset[1] - name.y,
                width: width - t.outer_padding,
                height: name.y,
            },
            Vec2::new(width, height),
        ));
        if let (Some(entity), Some(size)) = (tip.description, description) {
            commands.entity(entity).insert(relative(
                crate::fdf::Rect {
                    left: t.title_inset[0],
                    bottom: height - t.title_inset[1] - name.y - t.description_gap - size.y,
                    width: width - t.outer_padding,
                    height: size.y,
                },
                Vec2::new(width, height),
            ));
        }
        if !tip.ready {
            tooltip_art(
                &mut commands,
                tip.root,
                &mut source,
                Vec2::new(width, height),
                &assets.images,
                &mut assets.materials,
            )?;
            commands.entity(tip.root).insert(Visibility::Inherited);
            tip.ready = true;
        }
        Ok(())
    })();
    if let Err(error) = result {
        state.last_error = Some(error);
        commands.entity(tip.root).insert(Visibility::Hidden);
    }
    state.source = Some(source);
    state.tooltip = Some(tip);
}

#[cfg(all(test, feature = "test-fixtures"))]
pub(crate) fn tooltip_component_fixture(
    app: &mut App,
    reader: &pcx_assets::Resolver,
    client: &[u8],
    origin: AssetIdentity,
    race: Option<String>,
    parent: Entity,
    camera: Entity,
) {
    let rules = Rules::reviewed_legacy(client, origin.clone()).unwrap();
    let seed = tooltip_seed(client, origin, rules.tooltip.preference_index);
    let mut source = load(
        reader,
        rules,
        seed,
        Ok(race),
        &mut app
            .world_mut()
            .resource_mut::<bevy::prelude::Assets<Font>>(),
    )
    .unwrap();
    for (path, repeat) in [
        (
            source.tooltip_background.clone(),
            source.rules.tooltip.background_repeat,
        ),
        (
            source.tooltip_edge.clone(),
            source.rules.tooltip.edge_repeat,
        ),
    ] {
        image(
            reader,
            &path,
            repeat,
            &mut source,
            &mut app
                .world_mut()
                .resource_mut::<bevy::prelude::Assets<Image>>(),
        )
        .unwrap();
    }
    bevy::asset::embedded_asset!(app, "glue_backdrop.wgsl");
    app.add_plugins(UiMaterialPlugin::<crate::glue_menu::BackdropImage>::default())
        .insert_resource(CampaignInventory {
            parent: Some(parent),
            source: Some(source),
            ..default()
        })
        .insert_resource(CampaignCamera::active_for_test(camera))
        .init_resource::<crate::menu_screens::Navigation>();
    // Operator preference for this visual check, not a replacement source default.
    app.world_mut()
        .resource_mut::<crate::menu_screens::Navigation>()
        .applied
        .toggles
        .insert(TOOLTIP_TOGGLE.into(), true);
    app.add_systems(Update, refresh_tooltip).add_systems(
        PostUpdate,
        (
            measure_tooltip.after(bevy::ui::UiSystems::PostLayout),
            |state: Res<CampaignInventory>,
             windows: Query<&Window>,
             text: Query<&bevy::text::TextLayoutInfo>,
             mut previous: Local<Option<(Entity, UVec2)>>| {
                let Some(tip) = state.tooltip.as_ref().filter(|tip| tip.ready) else {
                    return;
                };
                let window = windows.single().unwrap();
                let key = (tip.root, window.physical_size());
                if *previous == Some(key) {
                    return;
                }
                assert!(state.last_error.is_none(), "{:?}", state.last_error);
                assert!(!text.get(tip.name).unwrap().glyphs.is_empty());
                assert!(
                    !text
                        .get(tip.description.unwrap())
                        .unwrap()
                        .glyphs
                        .is_empty()
                );
                println!(
                    "TOOLTIP_READY width={} height={} expanded=true",
                    key.1.x, key.1.y
                );
                *previous = Some(key);
            },
        )
            .chain(),
    );
    app.add_systems(
        Last,
        |state: Res<CampaignInventory>,
         windows: Query<&Window>,
         hovered: Query<
            &bevy::picking::hover::Hovered,
            With<crate::campaign_command_card::ButtonOrder>,
        >,
         mut previous: Local<String>| {
            let window = windows.single().unwrap();
            let value = format!(
                "cursor={:?} hovered={:?} tooltip={:?} error={:?}",
                window.cursor_position(),
                hovered.iter().map(|hover| hover.get()).collect::<Vec<_>>(),
                state.tooltip.as_ref().map(|tip| (tip.width, tip.ready)),
                state.last_error
            );
            if *previous != value {
                println!("TOOLTIP_STATE {value}");
                *previous = value;
            }
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkbox_fallback_admits_only_exact_zero_or_one() {
        assert_eq!(checkbox(b"0"), Ok(false));
        assert_eq!(checkbox(b"1"), Ok(true));
        for raw in [&b""[..], b" 1", b"01", b"2", b"1\0", b"true"] {
            assert!(checkbox(raw).is_err(), "{raw:?}");
        }
    }

    #[test]
    fn saved_tooltip_preference_wins_over_source_seed() {
        let refused = || Err("refused".to_owned());
        assert_eq!(effective_verbose(Some(false), &Ok(true)), Ok(false));
        assert_eq!(effective_verbose(Some(true), &refused()), Ok(true));
        assert_eq!(effective_verbose(None, &Ok(true)), Ok(true));
        assert_eq!(effective_verbose(None, &refused()), refused());
    }

    #[test]
    fn cover_uv_scales_to_decoded_pixels() {
        assert_eq!(
            cover_source_rect([0.25, 0., 0.75, 0.5], Vec2::new(256., 128.)),
            bevy::math::Rect::new(64., 0., 192., 64.)
        );
    }

    #[test]
    fn source_label_resolves_one_record_across_selected_lists() {
        let globals = "ACCEPT  \"Accept\",\n";
        let info_panel = "INVENTORY  \"Inventory\",\n";
        assert_eq!(
            source_label(&[globals, info_panel], "INVENTORY"),
            Ok("Inventory".into())
        );
        assert_eq!(
            source_label(&[globals, info_panel], "MISSING"),
            Err("missing source inventory label MISSING".into())
        );
        assert_eq!(
            source_label(&[info_panel, info_panel], "INVENTORY"),
            Err("ambiguous source inventory label INVENTORY".into())
        );
    }

    #[test]
    #[ignore = "requires PCX_PLAYER_RESOURCE_IMAGE selecting the explicitly supplied reviewed client"]
    fn supplied_client_label_bank_resolves_the_pushed_inventory_key() {
        let path = std::path::PathBuf::from(
            std::env::var("PCX_PLAYER_RESOURCE_IMAGE").expect("explicit local client image"),
        );
        let image = std::fs::read(&path).unwrap();
        let origin = AssetIdentity {
            source: path.display().to_string(),
            edition: "explicit selected local client".into(),
            version: "reviewed client; digest retained".into(),
        };
        let rules = Rules::reviewed_legacy(&image, origin.clone()).unwrap();
        let install = path.parent().expect("selected install directory");
        let mut archives = pcx_mpq::Install::open(install).expect("selected install archives");
        let text = |archives: &mut pcx_mpq::Install, member: &str| {
            String::from_utf8(archives.extract(member).expect(member)).expect("utf-8 string list")
        };
        let globals = text(&mut archives, GLOBALS);
        let info_panel = text(&mut archives, INFO_PANEL_STRINGS);
        // Relation check: the pushed key lives in the info-panel list, not GlobalStrings.
        assert!(crate::menu_text::global_string(&globals, &rules.label_key).is_none());
        assert!(source_label(&[&globals, &info_panel], &rules.label_key).is_ok());
        assert!(tooltip_seed(&image, origin, rules.tooltip.preference_index).is_ok());
    }

    #[test]
    #[ignore = "requires PCX_PLAYER_RESOURCE_IMAGE selecting the explicitly supplied reviewed client"]
    fn supplied_client_tooltip_index_joins_its_descriptor() {
        let path = std::env::var("PCX_PLAYER_RESOURCE_IMAGE").expect("explicit local client image");
        let bytes = std::fs::read(&path).unwrap();
        let origin = AssetIdentity {
            source: path,
            edition: "explicit selected local client".into(),
            version: "reviewed client; digest retained".into(),
        };
        let rules = Rules::reviewed_legacy(&bytes, origin.clone()).unwrap();
        let index = rules.tooltip.preference_index;
        let defaults = Defaults::discover(&bytes, origin.clone(), DISCOVERY).unwrap();
        let entry = defaults
            .entries()
            .iter()
            .find(|e| e.index == index)
            .expect("checked operand selects a discovered descriptor");
        // Verification only: production joins by index and never by this name.
        assert_eq!(entry.name, "tooltips");
        assert!(tooltip_seed(&bytes, origin, index).is_ok());
    }

    /// Tests that need the synthetic layout and the ECS harness. Run with
    /// `cargo test -p peacecraft3 --features test-fixtures`.
    #[cfg(feature = "test-fixtures")]
    mod hosted {
        use super::*;
        use bevy::ecs::system::RunSystemOnce;
        use pcx_play::inventory_layout::fixtures;
        use std::{
            collections::BTreeMap,
            sync::{Arc, atomic::AtomicUsize},
        };

        const SKIN_TEXT: &str = "[Human]\nConsole=UI\\Console.fdf\nFont=Fonts\\main.ttf\nTipFont=Fonts\\tip.ttf\nBackground=UI\\bg.blp\nEdge=UI\\edge.blp\nDisabled=UI\\disabled.blp\nCover=UI\\cover.blp\n";
        const RACE: &str = "RACE_PREF_Human";
        const FONT: &[u8] = include_bytes!("../art/DejaVuSans.ttf");

        struct MemorySource {
            info: pcx_assets::SourceInfo,
            files: BTreeMap<String, Vec<u8>>,
            reads: Arc<AtomicUsize>,
        }

        impl pcx_assets::ContentSource for MemorySource {
            fn info(&self) -> &pcx_assets::SourceInfo {
                &self.info
            }
            fn paths(&self) -> Result<Vec<String>, pcx_assets::SourceError> {
                Ok(self.files.keys().cloned().collect())
            }
            fn read(
                &self,
                member: &str,
            ) -> Result<Option<pcx_assets::SourceContent>, pcx_assets::SourceError> {
                self.read_bounded(member, u64::MAX)
            }
            fn read_bounded(
                &self,
                member: &str,
                limit: u64,
            ) -> Result<Option<pcx_assets::SourceContent>, pcx_assets::SourceError> {
                let Some(bytes) = self.files.get(member) else {
                    return Ok(None);
                };
                if bytes.len() as u64 > limit {
                    return Err(pcx_assets::SourceError::Limit {
                        requested: bytes.len() as u64,
                        limit,
                    });
                }
                self.reads
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                Ok(Some(pcx_assets::SourceContent {
                    bytes: bytes.clone(),
                    declared_length: bytes.len() as u64,
                    locator: member.to_string(),
                }))
            }
        }

        /// Keys are lowercase, forward-slash member paths.
        fn memory_reader(files: Vec<(&str, Vec<u8>)>) -> (pcx_assets::Resolver, Arc<AtomicUsize>) {
            let reads = Arc::new(AtomicUsize::new(0));
            let files = files
                .into_iter()
                .map(|(path, bytes)| (path.to_owned(), bytes))
                .collect();
            let resolver = pcx_assets::Resolver::new(
                pcx_assets::GameMode {
                    build: pcx_assets::Build::V1_29b,
                    addon: pcx_assets::Addon::Tft,
                    assets: pcx_assets::AssetMode::Vanilla,
                    behaviour: pcx_assets::Behaviour::Vanilla,
                },
                None,
                vec![pcx_assets::ArchiveLayer {
                    priority: 1,
                    source: Box::new(MemorySource {
                        info: pcx_assets::SourceInfo {
                            id: "inventory-fixture".into(),
                            kind: pcx_assets::LayerKind::Mpq,
                            origin: "inventory-fixture".into(),
                            version: Some("1.29.2".into()),
                        },
                        files,
                        reads: Arc::clone(&reads),
                    }),
                }],
            )
            .expect("memory resolver");
            (resolver, reads)
        }

        fn load_files(skin: &str, main: &[u8], tip: &[u8]) -> Vec<(&'static str, Vec<u8>)> {
            vec![
                ("ui/war3skins.txt", skin.as_bytes().to_vec()),
                ("ui/framedef/ui/consoleui.fdf", b"Console\n".to_vec()),
                (
                    "ui/framedef/globalstrings.fdf",
                    b"LABEL  \"Items\",\n".to_vec(),
                ),
                (
                    "ui/framedef/infopanelstrings.fdf",
                    b"UNRELATED  \"Other\",\n".to_vec(),
                ),
                (
                    "ui/miscui.txt",
                    b"[Heights]\nCaption=0.02\nName=0.02\nBody=0.015\n".to_vec(),
                ),
                ("fonts/main.ttf", main.to_vec()),
                ("fonts/tip.ttf", tip.to_vec()),
            ]
        }

        fn source(seed: Result<bool, String>) -> Source {
            Source {
                rules: fixtures::synthetic(),
                console_frame: String::new(),
                label: "Items".into(),
                font: Handle::default(),
                font_height: 0.02,
                disabled: "disabled".into(),
                cover: "cover".into(),
                images: HashMap::new(),
                sizes: HashMap::new(),
                failed_images: HashMap::new(),
                tooltip_font: Handle::default(),
                tooltip_name_height: 0.02,
                tooltip_description_height: 0.015,
                tooltip_background: "background".into(),
                tooltip_edge: "edge".into(),
                tooltip_materials: HashMap::new(),
                tooltip_seed: seed,
                retained: 0,
                receipts: Vec::new(),
            }
        }

        #[test]
        fn refused_font_admission_adds_no_font_asset() {
            let (reader, _) = memory_reader(load_files(SKIN_TEXT, FONT, b"not a font"));
            let mut fonts = bevy::prelude::Assets::<Font>::default();
            let error = load(
                &reader,
                fixtures::synthetic(),
                Ok(true),
                Ok(Some(RACE.into())),
                &mut fonts,
            )
            .err()
            .expect("unreadable tooltip font");
            assert!(error.contains("unreadable tooltip font"), "{error}");
            assert_eq!(fonts.len(), 0);

            // Both fonts read, then a later skin lookup fails: still no font asset.
            let skin = SKIN_TEXT.replace("Background=UI\\bg.blp\n", "");
            let (reader, _) = memory_reader(load_files(&skin, FONT, FONT));
            let mut fonts = bevy::prelude::Assets::<Font>::default();
            let error = load(
                &reader,
                fixtures::synthetic(),
                Ok(true),
                Ok(Some(RACE.into())),
                &mut fonts,
            )
            .err()
            .expect("missing background key");
            assert!(error.contains("Background"), "{error}");
            assert_eq!(fonts.len(), 0);
        }

        #[test]
        fn admitted_font_load_registers_both_distinct_faces() {
            let (reader, _) = memory_reader(load_files(SKIN_TEXT, FONT, FONT));
            let mut fonts = bevy::prelude::Assets::<Font>::default();
            let loaded = load(
                &reader,
                fixtures::synthetic(),
                Ok(true),
                Ok(Some(RACE.into())),
                &mut fonts,
            )
            .expect("admitted inventory source");
            assert_eq!(fonts.len(), 2);
            assert_eq!(loaded.label, "Items");
        }

        #[test]
        fn failed_image_decode_is_memoized_without_rereading_the_asset() {
            let (reader, reads) = memory_reader(vec![("ui/bad.blp", b"not an image".to_vec())]);
            let mut source = source(Ok(true));
            let mut images = bevy::prelude::Assets::<Image>::default();
            assert!(image(&reader, r"UI\bad.blp", false, &mut source, &mut images).is_err());
            let after_first = reads.load(std::sync::atomic::Ordering::Relaxed);
            assert!(image(&reader, r"UI\bad.blp", false, &mut source, &mut images).is_err());
            assert_eq!(
                reads.load(std::sync::atomic::Ordering::Relaxed),
                after_first,
                "a memoized failure must not reread the asset"
            );
            assert!(
                source.receipts.is_empty(),
                "failed decodes are not receipts"
            );
            assert_eq!(images.len(), 0);
        }

        #[test]
        fn empty_capacity_cover_applies_its_source_rectangle() {
            let mut app = App::new();
            let root = app.world_mut().spawn(Node::default()).id();
            let mut source = source(Ok(true));
            let key = (source.cover.clone(), source.rules.cover_repeat);
            source.images.insert(key.clone(), Handle::default());
            source.sizes.insert(key, Vec2::new(256., 128.));
            let view = View {
                unit: pcx_script::HandleId::new(std::num::NonZeroU64::MIN),
                capacity: 0,
                items: Vec::new(),
            };
            app.world_mut()
                .run_system_once(move |mut commands: Commands| {
                    paint(&mut commands, root, &source, &view)
                })
                .expect("paint system")
                .expect("cover paints");
            let rects = app
                .world_mut()
                .query::<&ImageNode>()
                .iter(app.world())
                .filter_map(|node| node.rect)
                .collect::<Vec<_>>();
            // Synthetic cover_uv is [0, 0.2, 1, 1], so the top edge is 0.2 * 128.
            assert_eq!(rects.len(), 1);
            assert_eq!(rects[0].min.x, 0.);
            assert!((rects[0].min.y - 25.6).abs() < 1e-4, "{:?}", rects[0]);
            assert_eq!(rects[0].max, Vec2::new(256., 128.));
        }

        fn tooltip_app(seed: Result<bool, String>) -> App {
            let mut app = App::new();
            app.init_resource::<CampaignInventory>()
                .init_resource::<crate::menu_screens::Navigation>()
                .add_systems(Update, refresh_tooltip);
            let camera = app.world_mut().spawn_empty().id();
            app.insert_resource(CampaignCamera::active_for_test(camera));
            let root = app.world_mut().spawn(Node::default()).id();
            let mut state = app.world_mut().resource_mut::<CampaignInventory>();
            state.root = Some(root);
            state.parent = Some(root);
            state.source = Some(source(seed));
            app
        }

        #[test]
        fn tooltip_art_fills_the_source_rectangles_instead_of_preserving_image_aspect() {
            let mut world = World::new();
            let root = world.spawn_empty().id();
            let mut source = source(Ok(true));
            let mut images = bevy::prelude::Assets::<Image>::default();
            let background = images.add(Image::default());
            let mut edge = Image::default();
            edge.resize(bevy::render::render_resource::Extent3d {
                width: 128,
                height: 16,
                depth_or_array_layers: 1,
            });
            let edge = images.add(edge);
            source.images.insert(
                (
                    source.tooltip_background.clone(),
                    source.rules.tooltip.background_repeat,
                ),
                background,
            );
            source.images.insert(
                (
                    source.tooltip_edge.clone(),
                    source.rules.tooltip.edge_repeat,
                ),
                edge,
            );
            let mut materials = bevy::prelude::Assets::<crate::glue_menu::BackdropImage>::default();
            world
                .run_system_once(move |mut commands: Commands| {
                    tooltip_art(
                        &mut commands,
                        root,
                        &mut source,
                        Vec2::new(0.3, 0.12),
                        &images,
                        &mut materials,
                    )
                })
                .unwrap()
                .unwrap();
            let mut nodes = world.query::<&ImageNode>();
            let nodes = nodes.iter(&world).collect::<Vec<_>>();
            assert_eq!(nodes.len(), 7); // Background, four corners and two vertical edges.
            assert!(
                nodes.iter().all(|node| matches!(
                    node.image_mode,
                    bevy::ui::widget::NodeImageMode::Stretch
                ))
            );
        }

        fn spawn_item(app: &mut App, index: usize, name: &str, hovered: bool) -> Entity {
            let anchor = {
                let state = app.world().resource::<CampaignInventory>();
                let source = state.source.as_ref().unwrap();
                TooltipAnchor {
                    rect: source.rules.slot(index).unwrap(),
                    viewport: source.rules.viewport,
                }
            };
            app.world_mut()
                .spawn((
                    InventorySlot(index),
                    anchor,
                    ControlTooltip {
                        name: name.into(),
                        ubertip: "Deals damage.".into(),
                    },
                    Hovered(hovered),
                ))
                .id()
        }

        #[test]
        fn hovered_held_item_projects_name_and_seeded_description() {
            let mut app = tooltip_app(Ok(true));
            let slot = spawn_item(&mut app, 0, "Sword", true);
            app.update();
            let state = app.world().resource::<CampaignInventory>();
            let tip = state.tooltip.as_ref().expect("hovered item tooltip");
            assert_eq!(tip.owner, slot);
            assert_eq!(tip.record.name, "Sword");
            assert_eq!(tip.verbose, Some(true));
            assert!(tip.description.is_some());
            let root = tip.root;

            app.world_mut().entity_mut(slot).insert(Hovered(false));
            app.update();
            assert!(
                app.world()
                    .resource::<CampaignInventory>()
                    .tooltip
                    .is_none()
            );
            assert!(
                app.world().get_entity(root).is_err(),
                "tooltip root despawned"
            );
        }

        #[test]
        fn saved_preference_overrides_the_source_seed() {
            let mut app = tooltip_app(Ok(true));
            app.world_mut()
                .resource_mut::<crate::menu_screens::Navigation>()
                .applied
                .toggles
                .insert(TOOLTIP_TOGGLE.into(), false);
            spawn_item(&mut app, 0, "Sword", true);
            app.update();
            let tip = app
                .world()
                .resource::<CampaignInventory>()
                .tooltip
                .as_ref()
                .map(|tip| (tip.verbose, tip.description.is_some()));
            assert_eq!(tip, Some((Some(false), false)));
        }

        #[test]
        fn refused_seed_projects_name_only_with_its_reason() {
            let mut app = tooltip_app(Err("outside profile".into()));
            spawn_item(&mut app, 0, "Sword", true);
            app.update();
            let state = app.world().resource::<CampaignInventory>();
            let tip = state.tooltip.as_ref().expect("name-only tooltip");
            assert_eq!(tip.verbose, None);
            assert!(tip.description.is_none());
            let error = state.last_error.as_deref().expect("reported reason");
            assert!(error.contains("outside profile"), "{error}");
        }

        #[test]
        fn hovered_empty_slot_projects_no_tooltip() {
            let mut app = tooltip_app(Ok(true));
            app.world_mut().spawn((InventorySlot(1), Hovered(true)));
            app.update();
            assert!(
                app.world()
                    .resource::<CampaignInventory>()
                    .tooltip
                    .is_none()
            );
        }

        #[test]
        fn an_empty_title_stays_empty_without_blocking_description_layout() {
            for title in ["", "|cffaabbcc|r"] {
                let mut app = tooltip_app(Ok(true));
                spawn_item(&mut app, 0, title, true);
                app.update();
                let tip = app
                    .world()
                    .resource::<CampaignInventory>()
                    .tooltip
                    .as_ref()
                    .unwrap();
                assert!(app.world().get::<EmptyTooltipText>(tip.name).is_some());
                assert!(
                    app.world()
                        .get::<EmptyTooltipText>(tip.description.unwrap())
                        .is_none()
                );
                assert_eq!(tip.record.name, title);
            }
        }

        #[test]
        fn malformed_hover_text_releases_partial_nodes_on_every_retry() {
            for malformed_name in [true, false] {
                let mut app = tooltip_app(Ok(true));
                let item = spawn_item(&mut app, 0, "Spell", true);
                let mut tip = app.world_mut().get_mut::<ControlTooltip>(item).unwrap();
                if malformed_name {
                    tip.name = "|q".into();
                } else {
                    tip.ubertip = "|q".into();
                }
                let mut ui = app
                    .world_mut()
                    .query_filtered::<Entity, Or<(With<Node>, With<TextSpan>)>>();
                let before = ui.iter(app.world()).count();
                for _ in 0..3 {
                    app.update();
                    let state = app.world().resource::<CampaignInventory>();
                    assert!(state.tooltip.is_none());
                    assert!(
                        state
                            .last_error
                            .as_ref()
                            .unwrap()
                            .contains("malformed tooltip")
                    );
                    assert_eq!(ui.iter(app.world()).count(), before);
                }
            }
        }

        #[test]
        fn non_inventory_control_uses_the_shared_tooltip_and_reports_refusal() {
            let mut app = tooltip_app(Ok(true));
            let anchor = {
                let state = app.world().resource::<CampaignInventory>();
                let source = state.source.as_ref().unwrap();
                TooltipAnchor {
                    rect: source.rules.slot(0).unwrap(),
                    viewport: source.rules.viewport,
                }
            };
            let control = app
                .world_mut()
                .spawn((
                    anchor,
                    Hovered(true),
                    ControlTooltip {
                        name: "Ranked spell".into(),
                        ubertip: "Heals 17.".into(),
                    },
                ))
                .id();
            app.update();
            let tip = app
                .world()
                .resource::<CampaignInventory>()
                .tooltip
                .as_ref()
                .unwrap();
            assert_eq!(tip.owner, control);
            assert_eq!(tip.record.ubertip, "Heals 17.");
            let old_root = tip.root;
            app.world_mut()
                .entity_mut(control)
                .remove::<ControlTooltip>()
                .insert(TooltipFailure("missing <TEST,DataA1>".into()));
            app.update();
            assert!(app.world().get_entity(old_root).is_err());
            let state = app.world().resource::<CampaignInventory>();
            assert!(state.tooltip.is_none());
            assert!(state.last_error.as_ref().unwrap().contains("<TEST,DataA1>"));
        }
    }
}

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
    playercolor_host::ObjectHost,
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
const MISC_UI: &str = r"UI\MiscUI.txt";
const CONSOLE: &str = r"UI\FrameDef\UI\ConsoleUI.fdf";
// Application budgets, unrelated to authored image dimensions or slot counts.
const MAX_PAYLOAD: usize = 64 << 20;
#[derive(Resource, Default)]
pub(crate) struct CampaignInventory {
    pub pointer_consumed: bool,
    pub last_error: Option<String>,
    pub receipts: Vec<AssetReceipt>,
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
struct ItemTooltip {
    name: String,
    ubertip: String,
}
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
    tooltip_font: Handle<Font>,
    tooltip_name_height: f32,
    tooltip_description_height: f32,
    tooltip_background: String,
    tooltip_edge: String,
    tooltip_materials:
        HashMap<crate::glue_menu::EdgePiece, Handle<crate::glue_menu::BackdropImage>>,
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
    slots: Query<Option<&ItemTooltip>, With<InventorySlot>>,
    mut state: ResMut<CampaignInventory>,
    presentation: Option<Res<peacecraft3::campaign_presentation::CampaignPresentation>>,
) {
    if slots.contains(event.entity) && crate::button_actions::input_allowed(presentation) {
        state.pending_pointer = true;
        if slots.get(event.entity).is_ok_and(|item| item.is_some()) && state.action_error.is_none()
        {
            let message = "inventory item use and drag/drop controls are not implemented";
            state.action_error = Some(message.into());
            warn!("{message}");
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
    let bytes = read(reader, path, 4 << 20, &mut source.receipts)?;
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
    let handle = images.add(image);
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
    let label = crate::menu_text::global_string(&globals, &rules.label_key)
        .ok_or("missing source inventory label")?;
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
    let font_path = skin_file(&skin, &sections, &rules.font_key)?;
    let bytes = read(reader, &font_path, 16 << 20, &mut receipts)?;
    if !crate::hud_render::font_registers(&bytes) {
        return Err(format!("{font_path}: unreadable inventory font"));
    }
    let mut retained = bytes.len();
    let font = fonts.add(Font::from_bytes(bytes));
    let tip_font_path = skin_file(&skin, &sections, &rules.tooltip.font_key)?;
    let tooltip_font = if tip_font_path == font_path {
        font.clone()
    } else {
        let bytes = read(reader, &tip_font_path, 16 << 20, &mut receipts)?;
        if !crate::hud_render::font_registers(&bytes) {
            return Err(format!("{tip_font_path}: unreadable tooltip font"));
        }
        retained = retained
            .checked_add(bytes.len())
            .filter(|n| *n <= MAX_PAYLOAD)
            .ok_or("inventory font aggregate budget")?;
        fonts.add(Font::from_bytes(bytes))
    };
    let tooltip_background = skin_file(&skin, &sections, &rules.tooltip.background_key)?;
    let tooltip_edge = skin_file(&skin, &sections, &rules.tooltip.edge_key)?;
    let disabled = skin_file(&skin, &sections, &rules.disabled_key)?;
    let cover = skin_file(&skin, &sections, &rules.cover_key)?;
    // Low bits are the native font format's left/center/right justification mask.
    if !matches!(rules.label_flags & 7, 1 | 2 | 4) {
        return Err("unsupported inventory font justification".into());
    }
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
        tooltip_font,
        tooltip_name_height,
        tooltip_description_height,
        tooltip_background,
        tooltip_edge,
        tooltip_materials: HashMap::new(),
        retained,
        receipts,
    })
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
        let image = source
            .images
            .get(&(source.cover.clone(), source.rules.cover_repeat))
            .ok_or("inventory cover unavailable")?;
        let child = commands
            .spawn((
                // shortcut: cover UV crop is not applied; complete before rendered fidelity approval.
                node(source.rules.cover(), source.rules.viewport),
                ImageNode::new(image.clone()),
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
        let slot = commands
            .spawn((
                node(source.rules.slot(index)?, source.rules.viewport),
                InventorySlot(index),
                Hovered::default(),
            ))
            .id();
        if let Some(item) = item {
            commands.entity(slot).insert((
                ImageNode::new(
                    source.images[&(item.art.clone(), source.rules.icon_repeat)].clone(),
                ),
                ItemTooltip {
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
                Rules::reviewed_legacy(
                    image,
                    pcx_play::scripts::AssetIdentity {
                        source: source.into(),
                        edition: "selected campaign client".into(),
                        version: "reviewed Legacy 1.29.2.9231 inventory layout image".into(),
                    },
                )
            })
            .and_then(|rules| {
                load(
                    &reader,
                    rules,
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
                        .chain([
                            (
                                source.tooltip_background.clone(),
                                source.rules.tooltip.background_repeat,
                            ),
                            (
                                source.tooltip_edge.clone(),
                                source.rules.tooltip.edge_repeat,
                            ),
                        ])
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
    record: ItemTooltip,
    verbose: Option<bool>,
    root: Entity,
    name: Entity,
    description: Option<Entity>,
    slot: SourceRect,
    width: Option<f32>,
    ready: bool,
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
fn refresh_tooltip(
    mut commands: Commands,
    mut state: ResMut<CampaignInventory>,
    camera: Res<CampaignCamera>,
    navigation: Res<crate::menu_screens::Navigation>,
    hovered: Query<(Entity, &InventorySlot, &ItemTooltip, &Hovered)>,
) {
    let desired = camera
        .active()
        .then(|| hovered.iter().find(|(_, _, _, hover)| hover.get()))
        .flatten();
    let Some((owner, slot, record, _)) = desired else {
        if let Some(tip) = state.tooltip.take() {
            commands.entity(tip.root).try_despawn();
        }
        return;
    };
    let verbose = navigation.applied.toggles.get("TooltipsCheckBox").copied();
    if verbose.is_none() {
        state.last_error = Some(
            "inventory tooltip description preference is unavailable; only its name is projected"
                .into(),
        );
    }
    if state
        .tooltip
        .as_ref()
        .is_some_and(|tip| tip.owner == owner && tip.record == *record && tip.verbose == verbose)
    {
        return;
    }
    if let Some(tip) = state.tooltip.take() {
        commands.entity(tip.root).try_despawn();
    }
    let Some(source) = state.source.as_ref() else {
        return;
    };
    let Some(parent) = state.root else {
        return;
    };
    let tooltip = &source.rules.tooltip;
    let result = (|| {
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
            slot: source.rules.slot(slot.0)?,
            width: None,
            ready: false,
        })
    })();
    match result {
        Ok(tip) => state.tooltip = Some(tip),
        Err(error) => state.last_error = Some(error),
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
            ImageNode::new(background.clone()),
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
            let mut node = ImageNode::new(edge.clone());
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
        let Ok((title, target)) = text.get(tip.name) else {
            return Ok(());
        };
        let pixels = target.physical_size().as_vec2();
        if pixels.x <= 0. || pixels.y <= 0. || title.glyphs.is_empty() {
            return Ok(());
        }
        let scale = Vec2::from(source.rules.viewport) / pixels;
        let name = title.size * scale;
        let description = match tip.description {
            Some(entity) => {
                let Ok((info, _)) = text.get(entity) else {
                    return Ok(());
                };
                if info.glyphs.is_empty() {
                    return Ok(());
                }
                Some(info.size * scale)
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

//! Selected-unit text projected from the canonical world onto the source console.
use crate::{
    campaign_hud::upper_buttons::{image_sampler, read, skin_file, text},
    fdf::Fdf,
    menu_skin::SkinTable,
    menu_text,
};
use bevy::{ecs::system::SystemParam, prelude::*};
use pcx_play::{
    inventory_layout::{Rect, Rules as InventoryRules},
    map_startup_host::MapStartupHost,
    playercolor_host::ObjectHost,
    scripts::{AssetIdentity, MapScripts},
    unit_panel_layout::Rules,
};
use peacecraft3::{
    browser_configuration::SelectedMapSources,
    campaign_camera::CampaignCamera,
    campaign_presentation::{CampaignInterface, CampaignPresentationSet},
    map_binding::AssetReceipt,
    map_requests::MapRequests,
    map_script_session::MapScriptSession,
};
use std::sync::{Arc, Weak};

const MEMBER: &str = r"UI\FrameDef\UI\SimpleInfoPanel.fdf";
const STRINGS: &str = r"UI\FrameDef\InfoPanelStrings.fdf";
const CONSOLE: &str = r"UI\FrameDef\UI\ConsoleUI.fdf";
/// Skin keys for the one hero-attribute icon. Order is the primary token, not a native table index.
const HERO_ICON_KEYS: [&str; 3] = [
    "InfoPanelIconHeroIconSTR",
    "InfoPanelIconHeroIconAGI",
    "InfoPanelIconHeroIconINT",
];
#[derive(Resource, Default)]
pub(crate) struct UnitPanel {
    witness: Option<Weak<()>>,
    attempted: bool,
    root: Option<Entity>,
    parent: Option<Entity>,
    source: Option<Source>,
    view: Option<View>,
    pub last_error: Option<String>,
}
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct UnitPanelSet;
impl UnitPanel {
    pub(crate) fn portrait_binding(&self) -> Option<(&Rules, [f32; 2], Entity)> {
        let source = self.source.as_ref()?;
        Some((&source.rules, source.viewport, self.parent?))
    }
    pub(crate) fn receipt_count(&self) -> usize {
        self.source.as_ref().map_or(0, |s| s.receipts.len())
    }
}
struct Source {
    rules: Rules,
    viewport: [f32; 2],
    info: Rect,
    console: String,
    title_style: menu_text::TextStyle,
    title_offset: [f32; 2],
    fonts: [Handle<Font>; 2],
    stat_height: f32,
    stat_font: Handle<Font>,
    label_style: menu_text::TextStyle,
    value_style: menu_text::TextStyle,
    rows: StatRows,
    hero_icons: [Handle<Image>; 3],
    receipts: Vec<AssetReceipt>,
}
struct StatRows {
    icon: [f32; 4],
    label_at: [f32; 2],
    value_at: [f32; 2],
    damage: String,
    armor: String,
    hero_icon: [f32; 2],
    hero_height: f32,
    hero_text_x: f32,
    hero_top_y: f32,
    strength_at: [f32; 2],
    hero_value_at: [f32; 2],
    hero_step_at: [f32; 2],
    strength: String,
    agility: String,
    intellect: String,
}
#[derive(Clone, Debug, PartialEq)]
struct View {
    unit: pcx_script::HandleId,
    name: String,
    health: String,
    mana: String,
    colour: u32,
    damage: String,
    armor: String,
    strength: String,
    agility: String,
    intelligence: String,
    hero_icon: Option<u8>,
}
#[derive(Component)]
struct PanelRoot;
#[derive(SystemParam)]
struct Inputs<'w, 's> {
    requests: Res<'w, MapRequests>,
    sources: Res<'w, SelectedMapSources>,
    camera: Res<'w, CampaignCamera>,
    launcher: Option<Res<'w, crate::launch_gui::Launcher>>,
    frames: Query<'w, 's, (Entity, &'static crate::hud_render::HudSourceAttachment)>,
}

pub(crate) fn install(app: &mut App) {
    crate::campaign_portrait::install(app);
    app.init_resource::<UnitPanel>().add_systems(
        Update,
        project
            .in_set(UnitPanelSet)
            .after(crate::hud_render::HudRenderSet)
            .after(crate::campaign_input::CampaignInputSet)
            .before(CampaignPresentationSet),
    );
}
fn project(
    mut commands: Commands,
    mut state: ResMut<UnitPanel>,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
    inputs: Inputs,
    mut fonts: ResMut<Assets<Font>>,
    mut images: ResMut<Assets<Image>>,
    nodes: Query<(), With<PanelRoot>>,
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
        if let Some(source) = state.source.take() {
            let stat = source.stat_font.id();
            let shared = source.fonts.iter().any(|font| font.id() == stat);
            for font in source.fonts {
                fonts.remove(font.id());
            }
            if !shared {
                fonts.remove(stat);
            }
            for image in source.hero_icons {
                images.remove(image.id());
            }
        }
        *state = UnitPanel {
            witness,
            ..default()
        };
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
    let Some(context) = session.context().cloned() else {
        return;
    };
    let Some(host) = session.host() else {
        return;
    };
    if !state.attempted {
        if !inputs.camera.active() {
            return;
        }
        state.attempted = true;
        let install = inputs
            .launcher
            .as_deref()
            .and_then(crate::launch_gui::selected_install);
        let result = crate::menu_lighting::with_client_image(install.as_deref(), |path, image| {
            let origin = AssetIdentity {
                source: path.into(),
                edition: "selected campaign client".into(),
                version: "Legacy 1.29.2.9231 unit panel".into(),
            };
            Ok((
                Rules::reviewed_legacy(image, origin.clone())?,
                InventoryRules::reviewed_legacy(image, origin)?,
            ))
        })
        .and_then(|(rules, inventory)| {
            load(
                &reader,
                rules,
                inventory,
                host.objects()
                    .local_race_preference_constant()
                    .map_err(|e| e.to_string())?,
                &mut fonts,
                &mut images,
            )
        });
        match result {
            Ok(source) => state.source = Some(source),
            Err(error) => state.last_error = Some(error),
        }
    }
    let Some(source) = state.source.as_ref() else {
        return;
    };
    let parents = inputs
        .frames
        .iter()
        .filter(|(_, a)| {
            a.frame == source.console
                && a.source_member == CONSOLE
                && a.receipt_path == bindings.receipts.map.path.as_str()
                && a.binding_revision == bindings.revision
        })
        .map(|(e, _)| e)
        .collect::<Vec<_>>();
    let [parent] = parents.as_slice() else {
        state.last_error = Some("unit panel awaits receipt-bound ConsoleUI parent".into());
        return;
    };
    if state.parent != Some(*parent) || state.root.is_none_or(|root| nodes.get(root).is_err()) {
        if let Some(root) = state.root {
            commands.entity(root).try_despawn();
        }
        let root = commands
            .spawn((
                surface(true),
                PanelRoot,
                CampaignInterface,
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(*parent).add_child(root);
        state.root = Some(root);
        state.parent = Some(*parent);
        state.view = None;
    }
    let source = state.source.as_ref().unwrap();
    let result = wanted(host.objects(), &context.scripts, &source.rules);
    let root = state.root.unwrap();
    let mut desired = match result {
        Ok(view) => {
            state.last_error = None;
            view
        }
        Err(error) => {
            state.last_error = Some(error);
            None
        }
    };
    // The portrait updater refreshes its colour only when the displayed life pair changes.
    if let (Some(previous), Some(next)) = (&state.view, &mut desired)
        && previous.unit == next.unit
        && previous.health == next.health
    {
        next.colour = previous.colour;
    }
    commands
        .entity(root)
        .insert(surface(inputs.camera.active() && desired.is_some()));
    if desired != state.view {
        commands.entity(root).despawn_children();
        if let Some(view) = &desired {
            paint(&mut commands, root, state.source.as_ref().unwrap(), view);
        }
        state.view = desired;
    }
}
fn surface(shown: bool) -> Node {
    Node {
        position_type: PositionType::Absolute,
        width: percent(100),
        height: percent(100),
        display: if shown { Display::Flex } else { Display::None },
        ..default()
    }
}
fn normalize(input: &str) -> Result<String, String> {
    let mut result = String::new();
    for (index, raw) in input.lines().enumerate() {
        let line = raw.split_once("//").map_or(raw, |(l, _)| l).trim();
        if let Some(rest) = line.strip_prefix("String ") {
            result.push_str(&format!("Frame \"TEXT\" {rest}\n"));
        } else if let Some(rest) = line.strip_prefix("Texture ") {
            if !rest.starts_with('"') {
                return Err("unit panel: unnamed texture".into());
            }
            result.push_str(&format!("Frame \"TEXTURE\" {rest}\n"));
        } else if line.starts_with("Layer \"") && line.ends_with('{') {
            result.push_str(&format!("Frame \"LAYER\" \"$UnitPanelLayer{index}\" {{\n"));
        } else if let Some(rest) = line.strip_prefix("Font ") {
            result.push_str(&format!("FrameFont {rest}\n"));
        } else {
            result.push_str(line);
            result.push('\n');
        }
    }
    Ok(result)
}
fn title_style(
    input: &str,
    frame: &str,
    name: &str,
) -> Result<(menu_text::TextStyle, [f32; 2]), String> {
    let fdf = Fdf::parse(&normalize(input)?).map_err(|e| e.to_string())?;
    if fdf.frames().iter().filter(|f| f.name == name).count() != 1
        || fdf.frames().iter().filter(|f| f.name == frame).count() != 1
    {
        return Err("unit panel: ambiguous detail frame/title".into());
    }
    let title = fdf
        .frame(name)
        .filter(|f| f.parent.as_deref() == Some(frame))
        .ok_or("unit panel: title is not in selected detail frame")?;
    if title.kind != "TEXT" {
        return Err("unit panel: title kind".into());
    }
    let anchor = fdf
        .record(name, "Anchor")
        .ok_or("unit panel: missing title anchor")?;
    let values = anchor.split(',').map(str::trim).collect::<Vec<_>>();
    let ["TOP", x, y] = values.as_slice() else {
        return Err("unit panel: unsupported title anchor".into());
    };
    let number = |value: &str| {
        value
            .parse::<f32>()
            .ok()
            .filter(|n| n.is_finite())
            .ok_or_else(|| "unit panel: title offset".to_string())
    };
    Ok((
        menu_text::render_style(&fdf, name)?,
        [number(x)?, number(y)?],
    ))
}
fn load(
    reader: &pcx_assets::Resolver,
    rules: Rules,
    inventory: InventoryRules,
    race: Option<String>,
    fonts: &mut Assets<Font>,
    images: &mut Assets<Image>,
) -> Result<Source, String> {
    let mut receipts = Vec::new();
    let skin = SkinTable::parse(&text(reader, r"UI\war3skins.txt", &mut receipts)?)
        .map_err(|e| e.to_string())?;
    let sections = crate::campaign_hud::race_sections(&skin, race)?;
    let console = skin_file(&skin, &sections, &inventory.console_key)?;
    let member = text(reader, MEMBER, &mut receipts)?;
    let (title_style, title_offset) = title_style(&member, &rules.frame, &rules.title)?;
    let fdf = Fdf::parse(&normalize(&member)?).map_err(|e| e.to_string())?;
    let strings = text(reader, STRINGS, &mut receipts)?;
    let label_style = menu_text::render_style(&fdf, "SimpleInfoPanelLabelTextTemplate")?;
    let value_style = menu_text::render_style(&fdf, "SimpleInfoPanelValueTextTemplate")?;
    if !value_style.font.eq_ignore_ascii_case(&label_style.font) {
        return Err("unit panel: stat value font".into());
    }
    let rows = stat_rows(&fdf, &strings)?;
    let info = inventory.info_panel();
    if info.width <= rows.icon[0] + rows.icon[2] {
        return Err("unit panel: stat row wider than the information frame".into());
    }
    let damage_width = rules.damage_row_width();
    let icon_right = rows.icon[0] + rows.icon[2];
    if damage_width.partial_cmp(&icon_right) != Some(std::cmp::Ordering::Greater) {
        return Err("unit panel: stat row narrower than its icon".into());
    }
    let profile = SkinTable::parse(&text(reader, r"UI\MiscUI.txt", &mut receipts)?)
        .map_err(|e| e.to_string())?;
    let heights = profile
        .sections()
        .iter()
        .filter(|s| s.name.eq_ignore_ascii_case(&rules.section))
        .flat_map(|s| &s.entries)
        .filter(|(key, _)| key.eq_ignore_ascii_case(&rules.height))
        .map(|(_, value)| value)
        .collect::<Vec<_>>();
    let [height] = heights.as_slice() else {
        return Err("unit panel: ambiguous or missing portrait font height".into());
    };
    let stat_height = height
        .parse::<f32>()
        .ok()
        .filter(|v| v.is_finite() && *v > 0.)
        .ok_or("unit panel: invalid portrait font height")?;
    let mut decoded = Vec::new();
    for key in [&title_style.font, &rules.font] {
        let path = skin_file(&skin, &sections, key)?;
        let bytes = read(reader, &path, 16 << 20, &mut receipts)?;
        if !crate::hud_render::font_registers(&bytes) {
            return Err(format!("unit panel: unreadable font {path}"));
        }
        decoded.push(Font::from_bytes(bytes));
    }
    let loaded: [Handle<Font>; 2] = decoded
        .into_iter()
        .map(|font| fonts.add(font))
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    let stat_font = if label_style.font.eq_ignore_ascii_case(&rules.font) {
        loaded[1].clone()
    } else if label_style.font.eq_ignore_ascii_case(&title_style.font) {
        loaded[0].clone()
    } else {
        let path = skin_file(&skin, &sections, &label_style.font)?;
        let bytes = read(reader, &path, 16 << 20, &mut receipts)?;
        if !crate::hud_render::font_registers(&bytes) {
            return Err(format!("unit panel: unreadable font {path}"));
        }
        fonts.add(Font::from_bytes(bytes))
    };
    let mut hero_icons = Vec::new();
    for key in HERO_ICON_KEYS {
        let path = skin_file(&skin, &sections, key)?;
        hero_icons.push(ui_image(reader, &path, &mut receipts, images)?);
    }
    let hero_icons = <[Handle<Image>; 3]>::try_from(hero_icons)
        .map_err(|_| "unit panel: hero icon count".to_string())?;
    Ok(Source {
        rules,
        viewport: inventory.viewport,
        info,
        console,
        title_style,
        title_offset,
        fonts: loaded,
        stat_height,
        stat_font,
        label_style,
        value_style,
        rows,
        hero_icons,
        receipts,
    })
}
fn ui_image(
    reader: &pcx_assets::Resolver,
    path: &str,
    receipts: &mut Vec<AssetReceipt>,
    images: &mut Assets<Image>,
) -> Result<Handle<Image>, String> {
    let bytes = read(reader, path, 4 << 20, receipts)?;
    let texture = pcx_gfx::load_image_bounded(&bytes, path, 16 << 20).map_err(|e| e.to_string())?;
    let mut image = pcx_gfx::viewer::rgba_image(&texture);
    image.sampler = bevy::image::ImageSampler::Descriptor(image_sampler(false));
    Ok(images.add(image))
}
fn hero_icon_index(primary: &str) -> Result<u8, String> {
    match primary {
        "STR" => Ok(0),
        "AGI" => Ok(1),
        "INT" => Ok(2),
        other => Err(format!("unit panel: hero icon {other}")),
    }
}
fn child<'a>(fdf: &'a Fdf, parent: &str, name: &str) -> Result<&'a crate::fdf::Frame, String> {
    let found = fdf
        .frames()
        .iter()
        .filter(|frame| frame.name == name && frame.parent.as_deref() == Some(parent))
        .collect::<Vec<_>>();
    let [frame] = found.as_slice() else {
        return Err(format!("unit panel: {parent}/{name}"));
    };
    Ok(frame)
}
fn anchor(props: &crate::fdf::Props, point: &str) -> Result<[f32; 2], String> {
    let value = props
        .records
        .iter()
        .find(|(key, _)| key == "Anchor")
        .map(|(_, value)| value.as_str())
        .ok_or("unit panel: missing anchor")?;
    let parts = value.split(',').map(str::trim).collect::<Vec<_>>();
    let [kind, x, y] = parts.as_slice() else {
        return Err("unit panel: anchor".into());
    };
    if *kind != point {
        return Err(format!("unit panel: anchor {kind}"));
    }
    let number = |text: &str| {
        text.trim_end_matches(',')
            .parse::<f32>()
            .ok()
            .filter(|n| n.is_finite())
            .ok_or_else(|| format!("unit panel: anchor {text}"))
    };
    Ok([number(x)?, number(y)?])
}
fn stat_rows(fdf: &Fdf, strings: &str) -> Result<StatRows, String> {
    let text = |key: &str| {
        menu_text::global_string(strings, key).ok_or_else(|| format!("unit panel: missing {key}"))
    };
    let icon = fdf
        .frame("InfoPanelIconTemplate")
        .ok_or("unit panel: icon template")?;
    let at = anchor(&icon.props, "TOPLEFT")?;
    let (Some(width), Some(height)) = (icon.props.width, icon.props.height) else {
        return Err("unit panel: icon size".into());
    };
    if width <= 0. || height <= 0. {
        return Err("unit panel: icon size".into());
    }
    let (label_at, value_at) = icon_text_anchors(fdf, "SimpleInfoPanelIconDamage")?;
    let armor_anchors = icon_text_anchors(fdf, "SimpleInfoPanelIconArmor")?;
    if armor_anchors != (label_at, value_at) {
        return Err("unit panel: armor text anchors".into());
    }
    let hero_icon = fdf
        .frame("InfoPanelIconHeroIcon")
        .ok_or("unit panel: hero icon")?;
    let hero_at = anchor(&hero_icon.props, "LEFT")?;
    let hero_width = fdf
        .width("InfoPanelIconHeroIcon")
        .map_err(|e| e.to_string())?;
    let hero_icon_height = fdf
        .height("InfoPanelIconHeroIcon")
        .map_err(|e| e.to_string())?;
    let hero_height = fdf
        .height("SimpleInfoPanelIconHero")
        .map_err(|e| e.to_string())?;
    if hero_width != width
        || hero_icon_height != height
        || hero_height <= 0.
        || !hero_at[1].is_finite()
    {
        return Err("unit panel: hero icon size".into());
    }
    let hero_text = child(
        fdf,
        "SimpleInfoPanelIconHero",
        "SimpleInfoPanelIconHeroText",
    )?;
    let hero_text_x = point_offset(
        hero_text,
        crate::fdf::Point::Left,
        "InfoPanelIconHeroIcon",
        crate::fdf::Point::Right,
    )?;
    let hero_top_y = point_offset(
        hero_text,
        crate::fdf::Point::Top,
        "SimpleInfoPanelIconHero",
        crate::fdf::Point::Top,
    )?;
    let strength = child(
        fdf,
        "SimpleInfoPanelIconHeroText",
        "InfoPanelIconHeroStrengthLabel",
    )?;
    let strength_at = anchor(&strength.props, "TOPLEFT")?;
    let hero_value_at = same_value_offset(
        fdf,
        &[
            (
                "InfoPanelIconHeroStrengthValue",
                "InfoPanelIconHeroStrengthLabel",
            ),
            (
                "InfoPanelIconHeroAgilityValue",
                "InfoPanelIconHeroAgilityLabel",
            ),
            (
                "InfoPanelIconHeroIntellectValue",
                "InfoPanelIconHeroIntellectLabel",
            ),
        ],
    )?;
    let agility_step = point_offset(
        child(
            fdf,
            "SimpleInfoPanelIconHeroText",
            "InfoPanelIconHeroAgilityLabel",
        )?,
        crate::fdf::Point::TopLeft,
        "InfoPanelIconHeroStrengthValue",
        crate::fdf::Point::BottomLeft,
    )?;
    let intellect_step = point_offset(
        child(
            fdf,
            "SimpleInfoPanelIconHeroText",
            "InfoPanelIconHeroIntellectLabel",
        )?,
        crate::fdf::Point::TopLeft,
        "InfoPanelIconHeroAgilityValue",
        crate::fdf::Point::BottomLeft,
    )?;
    if agility_step != intellect_step {
        return Err("unit panel: hero line step".into());
    }
    Ok(StatRows {
        icon: [at[0], at[1], width, height],
        label_at,
        value_at,
        damage: text("COLON_DAMAGE")?,
        armor: text("COLON_ARMOR")?,
        hero_icon: hero_at,
        hero_height,
        hero_text_x: hero_text_x[0],
        hero_top_y: hero_top_y[1],
        strength_at,
        hero_value_at,
        hero_step_at: agility_step,
        strength: text("COLON_STRENGTH")?,
        agility: text("COLON_AGILITY")?,
        intellect: text("COLON_INTELLECT")?,
    })
}
fn icon_text_anchors(fdf: &Fdf, frame: &str) -> Result<([f32; 2], [f32; 2]), String> {
    let label = child(fdf, frame, "InfoPanelIconLabel")?;
    let value = child(fdf, frame, "InfoPanelIconValue")?;
    let [label_point] = label.props.set_points.as_slice() else {
        return Err(format!("unit panel: {frame} label anchor"));
    };
    let [value_point] = value.props.set_points.as_slice() else {
        return Err(format!("unit panel: {frame} value anchor"));
    };
    if label_point.point != crate::fdf::Point::TopLeft
        || label_point.relative_point != crate::fdf::Point::TopRight
        || label_point.relative != "InfoPanelIconBackdrop"
        || value_point.point != crate::fdf::Point::TopLeft
        || value_point.relative_point != crate::fdf::Point::BottomLeft
        || value_point.relative != "InfoPanelIconLabel"
    {
        return Err(format!("unit panel: {frame} text anchors"));
    }
    Ok((
        [label_point.x, label_point.y],
        [value_point.x, value_point.y],
    ))
}
fn point_offset(
    frame: &crate::fdf::Frame,
    point: crate::fdf::Point,
    relative: &str,
    relative_point: crate::fdf::Point,
) -> Result<[f32; 2], String> {
    let found = frame
        .props
        .set_points
        .iter()
        .filter(|anchor| {
            anchor.point == point
                && anchor.relative == relative
                && anchor.relative_point == relative_point
        })
        .collect::<Vec<_>>();
    let [anchor] = found.as_slice() else {
        return Err(format!("unit panel: {} anchor", frame.name));
    };
    Ok([anchor.x, anchor.y])
}
fn same_value_offset(fdf: &Fdf, pairs: &[(&str, &str)]) -> Result<[f32; 2], String> {
    let mut offset = None;
    for (value, label) in pairs {
        let point = point_offset(
            child(fdf, "SimpleInfoPanelIconHeroText", value)?,
            crate::fdf::Point::TopLeft,
            label,
            crate::fdf::Point::BottomLeft,
        )?;
        if offset.get_or_insert(point) != &point {
            return Err("unit panel: hero value anchor".into());
        }
    }
    offset.ok_or_else(|| "unit panel: hero value anchor".into())
}
fn profile_name(
    definition: &pcx_sim::world::Definition,
    key: &str,
    mut localized: impl FnMut(u32) -> Result<Option<String>, String>,
) -> Result<String, String> {
    use pcx_objects::editor::{FieldKind, FieldSource, SlkId};
    let matches = definition
        .fields
        .iter()
        .filter(|(id, f)| {
            f.metadata.as_ref().is_some_and(|m|
        m.fourcc == **id && m.from == SlkId::UnitMetaData && m.kind == FieldKind::Token
        && matches!(&m.source,FieldSource::Profile{key:k,..} if k.eq_ignore_ascii_case(key)))
        })
        .collect::<Vec<_>>();
    let [(_, field)] = matches.as_slice() else {
        return Err(format!("unit panel: missing/ambiguous profile {key}"));
    };
    let value = match field
        .value
        .as_ref()
        .map_err(|e| format!("unit panel: {key}: {e:?}"))?
    {
        pcx_sim::world::FieldValue::Cell(s) => s.clone(),
        pcx_sim::world::FieldValue::Override(pcx_objects::Value::Str(s)) => {
            std::str::from_utf8(s).map_err(|e| e.to_string())?.into()
        }
        _ => return Err(format!("unit panel: non-string {key}")),
    };
    let value = if let Some(id) = pcx_script::text::trigstr_id(&value) {
        localized(id)?.ok_or_else(|| format!("unit panel: missing map string {value}"))?
    } else if value.starts_with("TRIGSTR_") {
        return Err(format!("unit panel: invalid map string {value}"));
    } else {
        value
    };
    // Application text-work limit, independent of authored name lengths.
    if value.len() > 4096 || value.trim().is_empty() || value.contains('\0') {
        return Err(format!("unit panel: empty/invalid {key}"));
    }
    Ok(value)
}
fn wanted(
    objects: &ObjectHost,
    scripts: &MapScripts,
    rules: &Rules,
) -> Result<Option<View>, String> {
    let [id] = objects.selection() else {
        return Ok(None);
    };
    let unit = objects.executed_unit(*id).map_err(|e| e.to_string())?;
    if !unit.shown || unit.combat.as_ref().is_some_and(|c| c.dead) {
        return Ok(None);
    }
    let abilities = unit
        .abilities
        .as_ref()
        .ok_or("unit panel: ability classification unavailable")?;
    let key = if abilities.hero.is_some() {
        "Propernames"
    } else {
        "Name"
    };
    let name = profile_name(&unit.definition, key, |id| scripts.table_string(id))?;
    // shortcut: creation does not retain a randomized proper-name choice yet; admit only single-name heroes.
    if abilities.hero.is_some() && name.contains(',') {
        return Err("unit panel: hero's creation-time proper-name choice is unavailable".into());
    }
    let vitals = unit
        .vitals
        .as_ref()
        .ok_or("unit panel: live vitals unavailable")?;
    if vitals.max_life <= 0. || !vitals.max_mana.is_finite() || vitals.max_mana < 0. {
        return Err("unit panel: invalid live maximum".into());
    }
    let stats = objects
        .unit_stat_snapshot(*id)
        .map_err(|error| error.to_string())?;
    let damage = match stats.damage {
        Some(damage) => {
            rules.damage_text(damage.minimum, damage.maximum, damage.natural, damage.bonus)?
        }
        None => String::new(),
    };
    let armor = rules.armor_text(stats.armor, stats.armor_bonus)?;
    let hero_icon = match stats.attributes.as_ref() {
        Some(attributes) => Some(hero_icon_index(&attributes.primary)?),
        None => None,
    };
    let (strength, agility, intelligence) = if let Some(attributes) = stats.attributes {
        (
            rules.attribute_text(attributes.strength, attributes.strength_bonus)?,
            rules.attribute_text(attributes.agility, attributes.agility_bonus)?,
            rules.attribute_text(attributes.intelligence, attributes.intelligence_bonus)?,
        )
    } else {
        (String::new(), String::new(), String::new())
    };
    let view = View {
        unit: *id,
        name,
        health: rules.readout(vitals.life, vitals.max_life, true)?,
        mana: if vitals.max_mana > 0. {
            rules.readout(vitals.mana, vitals.max_mana, false)?
        } else {
            String::new()
        },
        colour: rules.health_colour(vitals.life / vitals.max_life)?,
        damage,
        armor,
        strength,
        agility,
        intelligence,
        hero_icon,
    };
    for value in [
        &view.name,
        &view.health,
        &view.mana,
        &view.damage,
        &view.armor,
        &view.strength,
        &view.agility,
        &view.intelligence,
    ] {
        crate::rich_text::parse(value).map_err(|e| e.to_string())?;
    }
    Ok(Some(view))
}
fn colour(word: u32) -> Color {
    let [a, r, g, b] = word.to_be_bytes();
    Color::srgba_u8(r, g, b, a)
}
fn label_frame(rect: Rect, viewport: [f32; 2], justify: bevy::text::Justify) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: percent(rect.x / viewport[0] * 100.),
        bottom: percent(rect.y / viewport[1] * 100.),
        width: percent(rect.width / viewport[0] * 100.),
        height: percent(rect.height / viewport[1] * 100.),
        flex_direction: FlexDirection::Column,
        align_items: match justify {
            bevy::text::Justify::Left => AlignItems::FlexStart,
            bevy::text::Justify::Right => AlignItems::FlexEnd,
            _ => AlignItems::Center,
        },
        justify_content: JustifyContent::FlexStart,
        ..default()
    }
}
#[expect(
    clippy::too_many_arguments,
    reason = "one leaf combines the source text records"
)]
fn text_node(
    commands: &mut Commands,
    parent: Entity,
    rect: Rect,
    viewport: [f32; 2],
    value: &str,
    font: &Handle<Font>,
    height: f32,
    justify: bevy::text::Justify,
    foreground: Color,
    shadow: Color,
    offset: [f32; 2],
) {
    let frame = commands
        .spawn((label_frame(rect, viewport, justify), Pickable::IGNORE))
        .id();
    commands.entity(parent).add_child(frame);
    let entity = commands
        .spawn((
            Node {
                flex_shrink: 0.,
                ..default()
            },
            Text::default(),
            TextFont {
                font: font.clone().into(),
                font_size: bevy::text::FontSize::Vh(height / viewport[1] * 100.),
                ..default()
            },
            TextColor(foreground),
            bevy::text::LineHeight::RelativeToFont(1.0),
            TextLayout::new(justify, bevy::text::LineBreak::NoWrap),
            TextShadow {
                offset: Vec2::ZERO,
                color: shadow,
            },
            crate::glue_menu::FrameShadow(Vec2::from(offset)),
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(frame).add_child(entity);
    for span in crate::rich_text::parse(value)
        .expect("preflighted text")
        .spans
    {
        let child = commands
            .spawn((
                TextSpan::new(span.text),
                bevy::text::LineHeight::RelativeToFont(1.0),
                TextFont {
                    font: font.clone().into(),
                    font_size: bevy::text::FontSize::Vh(height / viewport[1] * 100.),
                    ..default()
                },
                TextColor(
                    span.colour
                        .map(|c| Color::srgba(c[0], c[1], c[2], c[3]))
                        .unwrap_or(foreground),
                ),
            ))
            .id();
        commands.entity(entity).add_child(child);
    }
}
fn paint(commands: &mut Commands, root: Entity, source: &Source, view: &View) {
    let style = &source.title_style;
    let [r, g, b, a] = style.colour;
    let [sr, sg, sb, sa] = style.shadow_colour;
    text_node(
        commands,
        root,
        Rect {
            x: source.info.x + source.title_offset[0],
            y: source.info.y + source.info.height + source.title_offset[1] - style.height,
            width: source.info.width,
            height: style.height,
        },
        source.viewport,
        &view.name,
        &source.fonts[0],
        style.height,
        bevy::text::Justify::Center,
        Color::srgba(r, g, b, a),
        Color::srgba(sr, sg, sb, sa),
        style.shadow_offset,
    );
    for (rect, value, foreground) in [
        (source.rules.health, &view.health, view.colour),
        (source.rules.mana, &view.mana, source.rules.mana_colour),
    ] {
        if value.is_empty() {
            continue;
        }
        text_node(
            commands,
            root,
            Rect {
                y: rect.y - rect.height,
                ..rect
            },
            source.viewport,
            value,
            &source.fonts[1],
            source.stat_height,
            match source.rules.justification {
                1 => bevy::text::Justify::Left,
                2 => bevy::text::Justify::Center,
                _ => bevy::text::Justify::Right,
            },
            colour(foreground),
            colour(source.rules.shadow),
            source.rules.shadow_offset,
        );
    }
    paint_stats(commands, root, source, view);
}
fn paint_stats(commands: &mut Commands, root: Entity, source: &Source, view: &View) {
    // Damage is TopLeft on the detail frame. Armor and the hero block are
    // TopLeft on that row's TopRight, one constructor width across.
    // Damage and armor textures stay unmounted. `GetUnitTypeId` (`0x497d30`)
    // returns the resolved unit's `+0x34`; that integer is the unit-type key
    // hashed by `0x95c860` and looked up in `0x113e8d8` by `0x6993a0`, the same
    // registry as `GetFoodMade`. `0x694230` then tests three fields of that type
    // record against its `+0x234` vector. `GetOwningPlayer` does not read `+0x34`.
    // The Neutral-suffix keys stay unused until those type fields are named.
    let top = source.info.y + source.info.height + source.rules.damage_row_dy();
    let left = source.info.x;
    let beside = left + source.rules.damage_row_width();
    if !view.damage.is_empty() {
        paint_pair(
            commands,
            root,
            source,
            left,
            top,
            &source.rows.damage,
            &view.damage,
        );
    }
    if !view.armor.is_empty() {
        paint_pair(
            commands,
            root,
            source,
            beside,
            top,
            &source.rows.armor,
            &view.armor,
        );
    }
    if view.hero_icon.is_some()
        || !view.strength.is_empty()
        || !view.agility.is_empty()
        || !view.intelligence.is_empty()
    {
        paint_hero(commands, root, source, beside, top, view);
    }
}
fn paint_hero(
    commands: &mut Commands,
    root: Entity,
    source: &Source,
    left: f32,
    top: f32,
    view: &View,
) {
    let rows = &source.rows;
    if let Some(index) = view.hero_icon {
        let center = top - rows.hero_height / 2. + rows.hero_icon[1];
        paint_image(
            commands,
            root,
            source,
            Rect {
                x: left + rows.hero_icon[0],
                y: center - rows.icon[3] / 2.,
                width: rows.icon[2],
                height: rows.icon[3],
            },
            &source.hero_icons[usize::from(index)],
        );
    }
    let text_left = left + rows.hero_icon[0] + rows.icon[2] + rows.hero_text_x;
    let text_top = top + rows.hero_top_y;
    let mut label_left = text_left + rows.strength_at[0];
    let mut label_top = text_top + rows.strength_at[1];
    for (label, value) in [
        (&rows.strength, &view.strength),
        (&rows.agility, &view.agility),
        (&rows.intellect, &view.intelligence),
    ] {
        paint_styled(
            commands,
            root,
            source,
            label_left,
            label_top,
            label,
            &source.label_style,
        );
        let value_left = label_left + rows.hero_value_at[0];
        let value_top = label_top - source.label_style.height + rows.hero_value_at[1];
        paint_styled(
            commands,
            root,
            source,
            value_left,
            value_top,
            value,
            &source.value_style,
        );
        label_left = value_left + rows.hero_step_at[0];
        label_top = value_top - source.value_style.height + rows.hero_step_at[1];
    }
}
fn paint_image(
    commands: &mut Commands,
    root: Entity,
    source: &Source,
    rect: Rect,
    image: &Handle<Image>,
) {
    let frame = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: percent(rect.x / source.viewport[0] * 100.),
                bottom: percent(rect.y / source.viewport[1] * 100.),
                width: percent(rect.width / source.viewport[0] * 100.),
                height: percent(rect.height / source.viewport[1] * 100.),
                ..default()
            },
            ImageNode::new(image.clone()),
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(root).add_child(frame);
}
fn paint_pair(
    commands: &mut Commands,
    root: Entity,
    source: &Source,
    left: f32,
    top: f32,
    label: &str,
    value: &str,
) {
    let icon_left = left + source.rows.icon[0];
    let icon_top = top + source.rows.icon[1];
    let label_left = icon_left + source.rows.icon[2] + source.rows.label_at[0];
    let label_top = icon_top + source.rows.label_at[1];
    paint_styled(
        commands,
        root,
        source,
        label_left,
        label_top,
        label,
        &source.label_style,
    );
    let value_top = label_top - source.label_style.height + source.rows.value_at[1];
    paint_styled(
        commands,
        root,
        source,
        label_left + source.rows.value_at[0],
        value_top,
        value,
        &source.value_style,
    );
}
fn paint_styled(
    commands: &mut Commands,
    root: Entity,
    source: &Source,
    left: f32,
    top: f32,
    text: &str,
    style: &menu_text::TextStyle,
) {
    let width = source.info.x + source.info.width - left;
    if text.is_empty() || width <= 0. {
        return;
    }
    let [r, g, b, a] = style.colour;
    let [sr, sg, sb, sa] = style.shadow_colour;
    text_node(
        commands,
        root,
        Rect {
            x: left,
            y: top - style.height,
            width,
            height: style.height,
        },
        source.viewport,
        text,
        &source.stat_font,
        style.height,
        match style.justify_h {
            menu_text::JustifyH::Left => bevy::text::Justify::Left,
            menu_text::JustifyH::Center => bevy::text::Justify::Center,
            menu_text::JustifyH::Right => bevy::text::Justify::Right,
        },
        Color::srgba(r, g, b, a),
        Color::srgba(sr, sg, sb, sa),
        style.shadow_offset,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_wrap_readouts_align_their_intrinsic_text_in_the_source_frame() {
        let rect = Rect {
            x: 0.2,
            y: 0.1,
            width: 0.4,
            height: 0.03,
        };
        for (justify, expected) in [
            (bevy::text::Justify::Left, AlignItems::FlexStart),
            (bevy::text::Justify::Center, AlignItems::Center),
            (bevy::text::Justify::Right, AlignItems::FlexEnd),
        ] {
            let node = label_frame(rect, [1., 2.], justify);
            assert_eq!(node.left, percent(20));
            assert_eq!(node.bottom, percent(5));
            assert_eq!(node.width, percent(40));
            assert_eq!(node.flex_direction, FlexDirection::Column);
            assert_eq!(node.align_items, expected);
            assert_eq!(node.justify_content, JustifyContent::FlexStart);
        }
    }
    #[test]
    fn title_inherits_source_font_colour_and_anchor_without_other_contexts_leaking() {
        let source = "String \"Style\" {\nFont \"FixtureFont\",0.023,\nFontColor 0.2 0.4 0.6 0.8,\n}\nFrame \"SIMPLEFRAME\" \"Panel\" {\nString \"Title\" INHERITS \"Style\" {\nAnchor TOP,0.01,-0.02,\n}\n}\nFrame \"SIMPLEFRAME\" \"Other\" {\nLayer \"ARTWORK\" {\nTexture \"Art\" {\nWidth 0.1,\n}\n}\n}\n";
        let (style, offset) = title_style(source, "Panel", "Title").unwrap();
        assert_eq!(style.font, "FixtureFont");
        assert_eq!(style.height, 0.023);
        assert_eq!(style.colour, [0.2, 0.4, 0.6, 0.8]);
        assert_eq!(offset, [0.01, -0.02]);
        for bad in [
            source.replace("TOP,", "BOTTOM,"),
            source.replace("0.01,", "NaN,"),
            source.replace("\"Panel\"", "\"OtherPanel\""),
        ] {
            assert!(title_style(&bad, "Panel", "Title").is_err());
        }
    }
    fn definition() -> pcx_sim::world::Definition {
        use pcx_objects::editor::{FieldDef, FieldKind, FieldSource, ObjectKind, SlkId};
        use pcx_sim::world::{FieldValue, ObjectField};
        let id = *b"N001";
        pcx_sim::world::Definition {
            rawcode: *b"U001",
            base_rawcode: "U001".into(),
            kind: ObjectKind::Unit,
            modifications: Vec::new(),
            fields: [(
                id,
                ObjectField {
                    metadata: Some(FieldDef {
                        fourcc: id,
                        from: SlkId::UnitMetaData,
                        field: "Name".into(),
                        slk: "Profile".into(),
                        index: -1,
                        declared: "string".into(),
                        kind: FieldKind::Token,
                        category: "text".into(),
                        source: FieldSource::Profile {
                            key: "Name".into(),
                            index: -1,
                            slots: 1,
                        },
                        measured: true,
                    }),
                    value: Ok(FieldValue::Cell("TRIGSTR_42".into())),
                },
            )]
            .into(),
        }
    }
    #[test]
    fn names_use_effective_profile_metadata_and_the_retained_map_string() {
        let mut definition = definition();
        assert_eq!(
            profile_name(&definition, "name", |id| {
                assert_eq!(id, 42);
                Ok(Some("Map-authored Name".into()))
            })
            .unwrap(),
            "Map-authored Name"
        );
        assert!(profile_name(&definition, "name", |_| Ok(None)).is_err());
        let field = definition.fields.get_mut(b"N001").unwrap();
        field.value = Ok(pcx_sim::world::FieldValue::Override(
            pcx_objects::Value::Str(b"Changed name".to_vec()),
        ));
        assert_eq!(
            profile_name(&definition, "Name", |_| panic!("literal needs no WTS")).unwrap(),
            "Changed name"
        );
        definition
            .fields
            .get_mut(b"N001")
            .unwrap()
            .metadata
            .as_mut()
            .unwrap()
            .source = pcx_objects::editor::FieldSource::Column {
            slk: pcx_objects::editor::SlkId::UnitUi,
            column: "name".into(),
        };
        assert!(profile_name(&definition, "Name", |_| Ok(None)).is_err());
    }
    #[test]
    fn stat_rows_place_armor_with_damage_and_chain_hero_text_from_the_icon() {
        let source = r#"
Texture "InfoPanelIconTemplate" {
    Width 0.032,
    Height 0.032,
    Anchor TOPLEFT, 0.004, -0.001,
}
Frame "SIMPLEFRAME" "SimpleInfoPanelIconDamage" {
    Height 0.03125,
    Texture "InfoPanelIconBackdrop" INHERITS "InfoPanelIconTemplate" {
    }
    String "InfoPanelIconLabel" {
        SetPoint TOPLEFT, "InfoPanelIconBackdrop", TOPRIGHT, 0.0, -0.003,
    }
    String "InfoPanelIconValue" {
        SetPoint TOPLEFT, "InfoPanelIconLabel", BOTTOMLEFT, 0.005, -0.003,
    }
}
Frame "SIMPLEFRAME" "SimpleInfoPanelIconArmor" {
    Height 0.03125,
    Texture "InfoPanelIconBackdrop" INHERITS "InfoPanelIconTemplate" {
    }
    String "InfoPanelIconLabel" {
        SetPoint TOPLEFT, "InfoPanelIconBackdrop", TOPRIGHT, 0.0, -0.003,
    }
    String "InfoPanelIconValue" {
        SetPoint TOPLEFT, "InfoPanelIconLabel", BOTTOMLEFT, 0.005, -0.003,
    }
}
Frame "SIMPLEFRAME" "SimpleInfoPanelIconHero" {
    Height 0.0625,
    Texture "InfoPanelIconHeroIcon" INHERITS "InfoPanelIconTemplate" {
        Anchor LEFT, 0.004, 0.0,
    }
    Frame "SIMPLEFRAME" "SimpleInfoPanelIconHeroText" {
        SetPoint LEFT, "InfoPanelIconHeroIcon", RIGHT, 0.0, 0.0,
        SetPoint TOP, "SimpleInfoPanelIconHero", TOP, 0.0, 0.0,
        String "InfoPanelIconHeroStrengthLabel" {
            Anchor TOPLEFT, 0.0, -0.003,
        }
        String "InfoPanelIconHeroStrengthValue" {
            SetPoint TOPLEFT, "InfoPanelIconHeroStrengthLabel", BOTTOMLEFT, 0.005, 0.0,
        }
        String "InfoPanelIconHeroAgilityLabel" {
            SetPoint TOPLEFT, "InfoPanelIconHeroStrengthValue", BOTTOMLEFT, -0.005, -0.004,
        }
        String "InfoPanelIconHeroAgilityValue" {
            SetPoint TOPLEFT, "InfoPanelIconHeroAgilityLabel", BOTTOMLEFT, 0.005, 0.0,
        }
        String "InfoPanelIconHeroIntellectLabel" {
            SetPoint TOPLEFT, "InfoPanelIconHeroAgilityValue", BOTTOMLEFT, -0.005, -0.004,
        }
        String "InfoPanelIconHeroIntellectValue" {
            SetPoint TOPLEFT, "InfoPanelIconHeroIntellectLabel", BOTTOMLEFT, 0.005, 0.0,
        }
    }
}
"#;
        let strings = "COLON_DAMAGE \"Damage:\",\nCOLON_ARMOR \"Armor:\",\nCOLON_STRENGTH \"Strength:\",\nCOLON_AGILITY \"Agility:\",\nCOLON_INTELLECT \"Intelligence:\",\n";
        let rows = stat_rows(&Fdf::parse(&normalize(source).unwrap()).unwrap(), strings).unwrap();
        assert_eq!(
            HERO_ICON_KEYS[usize::from(hero_icon_index("STR").unwrap())],
            "InfoPanelIconHeroIconSTR"
        );
        assert_eq!(
            (
                rows.damage.as_str(),
                rows.armor.as_str(),
                rows.strength.as_str(),
                rows.agility.as_str(),
                rows.intellect.as_str()
            ),
            (
                "Damage:",
                "Armor:",
                "Strength:",
                "Agility:",
                "Intelligence:"
            )
        );
        assert_eq!(rows.hero_icon, [0.004, 0.0]);
        assert_eq!(rows.hero_height, 0.0625);
        assert_eq!(hero_icon_index("STR").unwrap(), 0);
        assert_eq!(hero_icon_index("AGI").unwrap(), 1);
        assert_eq!(hero_icon_index("INT").unwrap(), 2);
        assert!(hero_icon_index("Neutral").is_err());
        assert_eq!(rows.hero_text_x, 0.0);
        assert_eq!(rows.hero_top_y, 0.0);
        assert_eq!(rows.strength_at, [0.0, -0.003]);
        assert_eq!(rows.hero_value_at, [0.005, 0.0]);
        assert_eq!(rows.hero_step_at, [-0.005, -0.004]);
    }
    #[cfg(all(feature = "test-fixtures", not(target_arch = "wasm32")))]
    #[test]
    #[ignore = "requires selected PCX_NATIVE_CHAPTER_* inputs"]
    fn selected_chapter_unit_panel_projects_live_selection_and_retires_it() {
        use bevy::ecs::system::RunSystemOnce;
        let mut app = crate::native_campaign::tests::started_app();
        let camera = app.world_mut().spawn_empty().id();
        app.insert_resource(CampaignCamera::active_for_test(camera))
            .init_resource::<UnitPanel>();
        let (revision, path, hero, other, witness) = {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let witness = session.session_witness().unwrap();
            let host = session.host().unwrap();
            let units = host
                .objects()
                .executed_units()
                .filter(|(_, u)| u.shown && !u.combat.as_ref().is_some_and(|c| c.dead))
                .collect::<Vec<_>>();
            let hero = units
                .iter()
                .find(|(_, u)| u.definition.rawcode == *b"Hart")
                .unwrap()
                .0;
            let other = units
                .iter()
                .find(|(_, u)| u.abilities.as_ref().is_some_and(|a| a.hero.is_none()))
                .unwrap()
                .0;
            let requests = app.world().resource::<MapRequests>();
            let b = requests.bindings.as_ref().unwrap();
            (
                b.revision,
                b.receipts.map.path.to_string(),
                hero,
                other,
                witness,
            )
        };
        // The native rules select the console frame key; no test placement fits.
        let install =
            std::path::PathBuf::from(std::env::var("PCX_NATIVE_CHAPTER_INSTALL").unwrap());
        let rules = crate::menu_lighting::with_client_image(Some(&install), |path, bytes| {
            InventoryRules::reviewed_legacy(
                bytes,
                AssetIdentity {
                    source: path.into(),
                    edition: "test".into(),
                    version: "selected".into(),
                },
            )
        })
        .unwrap();
        let console = {
            let requests = app.world().resource::<MapRequests>();
            let b = requests.bindings.as_ref().unwrap();
            let reader = app
                .world()
                .resource::<SelectedMapSources>()
                .for_receipts(&b.receipts)
                .unwrap();
            let mut receipts = Vec::new();
            let skin =
                SkinTable::parse(&text(&reader, r"UI\war3skins.txt", &mut receipts).unwrap())
                    .unwrap();
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let sections = crate::campaign_hud::race_sections(
                &skin,
                session
                    .host()
                    .unwrap()
                    .objects()
                    .local_race_preference_constant()
                    .unwrap(),
            )
            .unwrap();
            skin_file(&skin, &sections, &rules.console_key).unwrap()
        };
        let parent = app
            .world_mut()
            .spawn(crate::hud_render::HudSourceAttachment {
                source_label: "selected fixture".into(),
                frame: console,
                source_member: CONSOLE.into(),
                receipt_path: path,
                binding_revision: revision,
            })
            .id();
        let select = |world: &mut World, ids: &[pcx_script::HandleId]| {
            world
                .resource_mut::<MapScriptSession<MapStartupHost>>()
                .host()
                .unwrap()
                .objects_mut()
                .select_units(ids, true)
        };
        select(app.world_mut(), &[hero]);
        app.world_mut().run_system_once(project).unwrap();
        let state = app.world().resource::<UnitPanel>();
        assert!(state.last_error.is_none(), "{:?}", state.last_error);
        assert_eq!(state.view.as_ref().unwrap().name, "Arthas");
        assert!(state.view.as_ref().unwrap().damage.contains(" - "));
        assert!(!state.view.as_ref().unwrap().strength.is_empty());
        assert!(!state.view.as_ref().unwrap().armor.is_empty());
        assert_eq!(state.parent, Some(parent));
        assert!(state.receipt_count() > 0);
        assert!(Weak::ptr_eq(state.witness.as_ref().unwrap(), &witness));
        let previous = state.view.as_ref().unwrap().health.clone();
        {
            use pcx_script::{Host, Value};
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let life = session.global("UNIT_STATE_LIFE").unwrap();
            let objects = session.host().unwrap().objects_mut();
            objects
                .native(
                    "SetUnitState",
                    &[Value::Handle(hero), life, Value::Real(7.2)],
                )
                .unwrap();
        }
        app.world_mut().run_system_once(project).unwrap();
        assert_ne!(
            app.world()
                .resource::<UnitPanel>()
                .view
                .as_ref()
                .unwrap()
                .health,
            previous
        );
        select(app.world_mut(), &[other]);
        app.world_mut().run_system_once(project).unwrap();
        assert_eq!(
            app.world()
                .resource::<UnitPanel>()
                .view
                .as_ref()
                .unwrap()
                .unit,
            other
        );
        select(app.world_mut(), &[]);
        app.world_mut().run_system_once(project).unwrap();
        let state = app.world().resource::<UnitPanel>();
        assert!(state.view.is_none());
        assert_eq!(
            app.world()
                .get::<Node>(state.root.unwrap())
                .unwrap()
                .display,
            Display::None
        );
        let root = state.root.unwrap();
        app.world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .clear();
        app.world_mut().run_system_once(project).unwrap();
        assert!(app.world().resource::<UnitPanel>().source.is_none());
        assert!(app.world().get_entity(root).is_err());
    }
}

//! Selected-unit text projected from the canonical world onto the source console.
use crate::{
    campaign_hud::upper_buttons::{read, skin_file, text},
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
const CONSOLE: &str = r"UI\FrameDef\UI\ConsoleUI.fdf";
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
impl UnitPanel {
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
    receipts: Vec<AssetReceipt>,
}
#[derive(Clone, Debug, PartialEq)]
struct View {
    unit: pcx_script::HandleId,
    name: String,
    health: String,
    mana: String,
    colour: u32,
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
    app.init_resource::<UnitPanel>().add_systems(
        Update,
        project
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
            for font in source.fonts {
                fonts.remove(font.id());
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
) -> Result<Source, String> {
    let mut receipts = Vec::new();
    let skin = SkinTable::parse(&text(reader, r"UI\war3skins.txt", &mut receipts)?)
        .map_err(|e| e.to_string())?;
    let sections = crate::campaign_hud::race_sections(&skin, race)?;
    let console = skin_file(&skin, &sections, &inventory.console_key)?;
    let (title_style, title_offset) = title_style(
        &text(reader, MEMBER, &mut receipts)?,
        &rules.frame,
        &rules.title,
    )?;
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
    let fonts = decoded
        .into_iter()
        .map(|font| fonts.add(font))
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    Ok(Source {
        rules,
        viewport: inventory.viewport,
        info: inventory.info_panel(),
        console,
        title_style,
        title_offset,
        fonts,
        stat_height,
        receipts,
    })
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
    };
    for value in [&view.name, &view.health, &view.mana] {
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

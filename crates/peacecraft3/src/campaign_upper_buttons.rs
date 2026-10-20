//! Source-authored upper controls and live quest log. Root placement is a
//! recreation choice: upper controls at the viewport top left, quest log centered.
use crate::glue_menu::{self, BackdropImage, EdgePiece};
use crate::{fdf::Fdf, menu_skin::SkinTable, menu_text};
use bevy::ecs::system::SystemParam;
use bevy::picking::events::PointerPress;
use bevy::prelude::*;
use bevy::ui::InteractionDisabled;
use bevy::ui_widgets::Button as UiButton;
use pcx_play::{map_startup_host::MapStartupHost, quest_view};
use peacecraft3::{
    browser_configuration::SelectedMapSources, campaign_camera::CampaignCamera,
    map_requests::MapRequests, map_script_session::MapScriptSession,
};
use std::{collections::HashMap, sync::Weak};

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct CampaignHudSet;

#[derive(Message, Clone, Copy, Debug)]
pub(crate) enum CampaignHudAction {
    Menu,
}
#[derive(Component)]
enum Action {
    Quests,
    Menu,
    Close,
    Select(pcx_script::HandleId),
}
#[derive(Component)]
struct AuthoredShortcut(KeyCode);
#[derive(Component)]
struct CampaignControlsRoot;
#[derive(Resource, Default)]
struct UpperControls {
    witness: Option<Weak<()>>,
    root: Option<Entity>,
    log: Option<Entity>,
    attempted: bool,
    escape_consumed: bool,
    pointer_consumed: bool,
    pointer_pending: bool,
    open: bool,
    signature: String,
    selected: Option<pcx_script::HandleId>,
    assets: Option<Source>,
    pub last_error: Option<String>,
}
struct Source {
    receipts: Vec<peacecraft3::map_binding::AssetReceipt>,
    fdf: Fdf,
    globals: String,
    quests_key: KeyCode,
    fonts: HashMap<String, Handle<Font>>,
    backdrop: glue_menu::Backdrop,
    backdrop_images: HashMap<(String, bool), Handle<Image>>,
    materials: Vec<Handle<BackdropImage>>,
    image: Handle<Image>,
    image_size: Vec2,
}

pub(super) fn install(app: &mut App) {
    crate::button_actions::install(app);
    app.add_observer(capture_pointer_press);
    app.init_resource::<UpperControls>()
        .add_message::<CampaignHudAction>()
        .add_systems(
            Update,
            (
                keyboard_shortcuts,
                activate.before(crate::button_actions::clear),
                project,
                publish_error,
                show_controls,
            )
                .chain()
                .in_set(CampaignHudSet)
                .after(peacecraft3::campaign_camera::CampaignCameraSet)
                .before(peacecraft3::campaign_presentation::CampaignPresentationSet),
        );
}
fn keyboard_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Query<(Entity, &AuthoredShortcut), Without<InteractionDisabled>>,
    mut actions: ResMut<crate::button_actions::ButtonActions>,
    camera: Option<Res<CampaignCamera>>,
    presentation: Option<Res<peacecraft3::campaign_presentation::CampaignPresentation>>,
) {
    if camera.as_ref().is_some_and(|camera| !camera.active())
        || !crate::button_actions::input_allowed(presentation)
    {
        return;
    }
    for (entity, shortcut) in &buttons {
        if keys.just_pressed(shortcut.0) && !actions.entities.contains(&entity) {
            actions.entities.push(entity);
        }
    }
}
fn activate(
    buttons: Query<&Action>,
    activations: Res<crate::button_actions::ButtonActions>,
    mut state: ResMut<UpperControls>,
    mut actions: MessageWriter<CampaignHudAction>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    state.escape_consumed = state.open && keys.just_pressed(KeyCode::Escape);
    state.pointer_consumed = std::mem::take(&mut state.pointer_pending);
    for entity in &activations.entities {
        let Ok(action) = buttons.get(*entity) else {
            continue;
        };
        state.pointer_consumed = true;
        match action {
            Action::Quests => state.open = !state.open,
            Action::Close => state.open = false,
            Action::Select(id) => {
                state.selected = Some(*id);
                state.signature.clear();
            }
            Action::Menu => {
                actions.write(CampaignHudAction::Menu);
            }
        }
    }
    if keys.just_pressed(KeyCode::Escape) {
        state.open = false;
    }
}
fn capture_pointer_press(
    event: On<PointerPress>,
    buttons: Query<(), (With<Action>, Without<InteractionDisabled>)>,
    mut state: ResMut<UpperControls>,
    presentation: Option<Res<peacecraft3::campaign_presentation::CampaignPresentation>>,
) {
    if buttons.contains(event.entity) && crate::button_actions::input_allowed(presentation) {
        state.pointer_pending = true;
    }
}
#[derive(SystemParam)]
struct RenderAssets<'w> {
    fonts: ResMut<'w, Assets<Font>>,
    images: ResMut<'w, Assets<Image>>,
    materials: ResMut<'w, Assets<BackdropImage>>,
}
fn project(
    mut commands: Commands,
    mut state: ResMut<UpperControls>,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
    sources: Res<SelectedMapSources>,
    requests: Res<MapRequests>,
    camera: Res<CampaignCamera>,
    assets: RenderAssets,
) {
    let RenderAssets {
        mut fonts,
        mut images,
        mut materials,
    } = assets;
    let witness = session.session_witness();
    let same = match (&state.witness, &witness) {
        (Some(a), Some(b)) => Weak::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    };
    if !same {
        for entity in [state.root.take(), state.log.take()].into_iter().flatten() {
            commands.entity(entity).despawn();
        }
        if let Some(assets) = state.assets.take() {
            for handle in assets.fonts.values() {
                fonts.remove(handle.id());
            }
            for handle in assets.backdrop_images.values() {
                images.remove(handle.id());
            }
            for handle in assets.materials {
                materials.remove(handle.id());
            }
            images.remove(assets.image.id());
        }
        *state = UpperControls {
            witness,
            ..default()
        };
    }
    if let Some(root) = state.root {
        commands.entity(root).insert(if camera.active() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
    if let Some(log) = state.log {
        commands.entity(log).insert(if camera.active() {
            Visibility::Visible
        } else {
            Visibility::Hidden
        });
    }
    if !camera.active() {
        return;
    }
    if !state.attempted {
        let Some(bindings) = requests.bindings.as_ref() else {
            return;
        };
        let Some(context) = session.context() else {
            return;
        };
        if !requests
            .admission
            .shared_context()
            .as_ref()
            .is_some_and(|published| std::sync::Arc::ptr_eq(published, context))
            || !requests
                .published_receipts
                .as_ref()
                .is_some_and(|published| std::sync::Arc::ptr_eq(published, &bindings.receipts))
        {
            return;
        }
        let Some(reader) = sources.for_receipts(&bindings.receipts) else {
            return;
        };
        let Some(host) = session.host() else {
            return;
        };
        state.attempted = true;
        let result = load(
            &reader,
            host.objects()
                .local_race_preference_constant()
                .map_err(|e| e.to_string()),
            &mut fonts,
            &mut images,
        );
        match result {
            Ok(source) => match upper(&mut commands, &source) {
                Ok(root) => {
                    state.root = Some(root);
                    state.assets = Some(source);
                }
                Err(e) => state.last_error = Some(e),
            },
            Err(e) => state.last_error = Some(e),
        }
    }
    if !state.open {
        if let Some(log) = state.log.take() {
            commands.entity(log).despawn();
        }
        state.signature.clear();
        return;
    }
    let board = session
        .host()
        .and_then(|host| host.objects().quest_session().map(quest_view::project));
    let board = match board {
        Some(Ok(board)) => board,
        Some(Err(error)) => {
            state.last_error = Some(format!("quest projection: {error:?}"));
            return;
        }
        None => {
            state.last_error = Some("quest state is unavailable".into());
            return;
        }
    };
    let quests: Vec<_> = board
        .quests
        .iter()
        .filter(|q| q.enabled && q.discovered)
        .collect();
    let signature = format!("{quests:?}");
    if state.log.is_some() && state.signature == signature {
        return;
    }
    let selected = state.selected;
    let Some(source) = state.assets.as_mut() else {
        return;
    };
    let previous_materials = std::mem::take(&mut source.materials);
    let candidate = quest_log(
        &mut commands,
        source,
        &quests,
        selected,
        &images,
        &mut materials,
    );
    match &candidate {
        Ok(_) => {
            for handle in previous_materials {
                materials.remove(handle.id());
            }
        }
        Err(_) => {
            for handle in source.materials.drain(..) {
                materials.remove(handle.id());
            }
            source.materials = previous_materials;
        }
    }
    match candidate {
        Ok(root) => {
            if let Some(old) = state.log.replace(root) {
                commands.entity(old).despawn();
            }
            state.signature = signature;
        }
        Err(e) => state.last_error = Some(e),
    }
}
fn normalize(text: &str) -> String {
    text.lines()
        .map(|line| {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("String ") {
                format!("Frame \"TEXT\" {rest}\n")
            } else if let Some(rest) = line.strip_prefix("Texture ") {
                format!("Frame \"TEXTURE\" {rest}\n")
            } else if let Some(rest) = line.strip_prefix("Font ") {
                format!("FrameFont {rest}\n")
            } else {
                format!("{line}\n")
            }
        })
        .collect()
}
pub(crate) fn skin_file(
    skin: &SkinTable,
    sections: &[String],
    key: &str,
) -> Result<String, String> {
    for section in sections {
        let values: Vec<_> = skin
            .sections()
            .iter()
            .filter(|s| s.name == *section)
            .flat_map(|s| &s.entries)
            .filter(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
            .collect();
        match values.as_slice() {
            [] => {}
            [value] if !value.is_empty() => return Ok(value.clone()),
            _ => return Err(format!("ambiguous/empty {section}.{key}")),
        }
    }
    Err(format!("missing skin key {key}"))
}
pub(crate) fn read(
    reader: &pcx_assets::Resolver,
    path: &str,
    limit: u64,
    receipts: &mut Vec<peacecraft3::map_binding::AssetReceipt>,
) -> Result<Vec<u8>, String> {
    let asset = reader
        .resolve_bounded(path, limit)
        .map_err(|e| format!("{path}: {e}"))?;
    receipts.push(peacecraft3::map_binding::receipt(&asset));
    Ok(asset.bytes)
}
pub(crate) fn text(
    reader: &pcx_assets::Resolver,
    path: &str,
    receipts: &mut Vec<peacecraft3::map_binding::AssetReceipt>,
) -> Result<String, String> {
    String::from_utf8(read(reader, path, super::MAX_TEXT_BYTES as u64, receipts)?)
        .map_err(|e| format!("{path}: {e}"))
}
fn load(
    reader: &pcx_assets::Resolver,
    race: Result<Option<String>, String>,
    fonts: &mut Assets<Font>,
    images: &mut Assets<Image>,
) -> Result<Source, String> {
    let mut receipts = Vec::new();
    let skin =
        SkinTable::parse(&text(reader, super::SKIN, &mut receipts)?).map_err(|e| e.to_string())?;
    let sections = super::race_sections(&skin, race?)?;
    let mut fdf = Fdf::parse(&normalize(&text(
        reader,
        "UI\\FrameDef\\UI\\UpperButtonBar.fdf",
        &mut receipts,
    )?))
    .map_err(|e| e.to_string())?;
    for path in [
        "UI\\FrameDef\\UI\\EscMenuTemplates.fdf",
        "UI\\FrameDef\\UI\\QuestDialog.fdf",
    ] {
        fdf.parse_into(&text(reader, path, &mut receipts)?)
            .map_err(|e| format!("{path}: {e}"))?;
    }
    for name in ["QuestMainTitle", "QuestOptionalTitle"] {
        let style = menu_text::render_style(&fdf, name)?;
        let width = fdf.width(name).map_err(|e| e.to_string())?;
        fdf.measure_text(name, (width, style.height));
    }
    let view = (
        fdf.width("QuestDialog").map_err(|e| e.to_string())?,
        fdf.height("QuestDialog").map_err(|e| e.to_string())?,
    );
    for name in [
        "QuestMainTitle",
        "QuestOptionalTitle",
        "QuestMainContainer",
        "QuestOptionalContainer",
        "QuestDisplayBackdrop",
        "QuestAcceptButton",
    ] {
        fdf.rect(name, view).map_err(|e| e.to_string())?;
    }
    let file = |key: &str| skin_file(&skin, &sections, key);
    let mut decoded_fonts = HashMap::new();
    for name in [
        "UpperButtonBarButtonTextTemplate",
        "QuestDetailsTitle",
        "EscMenuInfoTextTemplate",
        "EscMenuButtonTextTemplate",
        "QuestListItemTitle",
        "QuestDisplay",
        "QuestMainTitle",
        "QuestOptionalTitle",
    ] {
        let style = menu_text::render_style(&fdf, name)?;
        if decoded_fonts.contains_key(&style.font) {
            continue;
        }
        let path = file(&style.font)?;
        let font = read(reader, &path, 16 << 20, &mut receipts)?;
        if !crate::hud_render::font_registers(&font) {
            return Err(format!("{path}: unreadable font"));
        }
        decoded_fonts.insert(style.font, Font::from_bytes(font));
    }
    let backdrop = glue_menu::backdrop(&fdf, "QuestBackdrop")
        .map_err(|e| e.to_string())?
        .ok_or("missing quest backdrop")?;
    let mut decoded_backdrops = HashMap::new();
    for (token, tiled) in backdrop
        .background
        .iter()
        .map(|token| (token, backdrop.tile))
        .chain(backdrop.edge.iter().map(|edge| (&edge.file, false)))
    {
        let path = file(token)?;
        let bytes = read(reader, &path, 4 << 20, &mut receipts)?;
        let decoded =
            pcx_gfx::load_image_bounded(&bytes, &path, 16 << 20).map_err(|e| e.to_string())?;
        if let Some(edge) = &backdrop.edge
            && &edge.file == token
        {
            for piece in &edge.pieces {
                glue_menu::edge_source_rect(*piece, decoded.width, decoded.height)
                    .map_err(|e| format!("{path}: {e:?}"))?;
            }
        }
        let mut image = pcx_gfx::viewer::rgba_image(&decoded);
        image.sampler = bevy::image::ImageSampler::Descriptor(image_sampler(tiled));
        decoded_backdrops.insert((token.clone(), tiled), image);
    }
    let image_key = fdf
        .string("UpperMenuButtonBackground", "File")
        .ok_or("missing upper-menu texture")?;
    let image_path = file(&image_key)?;
    let image = read(reader, &image_path, 4 << 20, &mut receipts)?;
    let decoded =
        pcx_gfx::load_image_bounded(&image, &image_path, 16 << 20).map_err(|e| e.to_string())?;
    let image_size = Vec2::new(decoded.width as f32, decoded.height as f32);
    let globals = text(reader, "UI\\FrameDef\\GlobalStrings.fdf", &mut receipts)?;
    for key in [
        "KEY_QUESTS",
        "KEY_MENU",
        "QUESTSMAIN",
        "QUESTSOPTIONAL",
        "QUESTACCEPT",
        "QUESTCOMPLETED",
        "QUESTFAILED",
        "QUESTNOTCOMPLETED",
    ] {
        let value = menu_text::global_string(&globals, key)
            .ok_or_else(|| format!("missing global string {key}"))?;
        menu_text::Label::parse(&value).map_err(|e| e.to_string())?;
    }
    if backdrop.tile
        && backdrop
            .background_size
            .is_none_or(|tile| !tile.is_finite() || tile <= 0.)
    {
        return Err("invalid quest backdrop tile size".into());
    }
    let quests_key = peacecraft3::campaign_camera::authored_function_key(&globals, "KEY_QUESTS")?;
    Ok(Source {
        receipts,
        quests_key,
        fdf,
        globals,
        fonts: decoded_fonts
            .into_iter()
            .map(|(key, font)| (key, fonts.add(font)))
            .collect(),
        backdrop,
        backdrop_images: decoded_backdrops
            .into_iter()
            .map(|(key, image)| (key, images.add(image)))
            .collect(),
        materials: Vec::new(),
        image: images.add(pcx_gfx::viewer::rgba_image(&decoded)),
        image_size,
    })
}
fn colour(value: [f32; 4]) -> Color {
    Color::srgba(value[0], value[1], value[2], value[3])
}
fn label(
    commands: &mut Commands,
    parent: Entity,
    text: String,
    source: &Source,
    style: &str,
) -> Result<(), String> {
    let style = menu_text::render_style(&source.fdf, style)?;
    let parsed = crate::rich_text::parse(&text).map_err(|e| e.to_string())?;
    if !parsed.unsupported.is_empty() {
        return Err("quest label contains unsupported substitutions".into());
    }
    let font = TextFont {
        font: source.fonts[&style.font].clone().into(),
        font_size: bevy::text::FontSize::Vh(style.height / crate::mainmenu::FRAME_H * 100.),
        ..default()
    };
    let child = commands
        .spawn((
            Text::default(),
            font.clone(),
            TextColor(colour(style.colour)),
            Pickable::IGNORE,
        ))
        .id();
    for span in parsed.spans {
        let run = commands
            .spawn((
                TextSpan::new(span.text),
                font.clone(),
                TextColor(colour(span.colour.unwrap_or(style.colour))),
            ))
            .id();
        commands.entity(child).add_child(run);
    }
    commands.entity(parent).add_child(child);
    Ok(())
}
fn global(source: &Source, key: &str) -> Result<String, String> {
    menu_text::global_string(&source.globals, key)
        .ok_or_else(|| format!("missing global string {key}"))
}
fn upper(commands: &mut Commands, source: &Source) -> Result<Entity, String> {
    let fdf = &source.fdf;
    let width = fdf
        .width("UpperButtonBarFrame")
        .map_err(|e| e.to_string())?;
    let height = fdf
        .height("UpperButtonBarFrame")
        .map_err(|e| e.to_string())?;
    let uv = fdf
        .numbers("UpperMenuButtonBackground", "TexCoord")
        .map_err(|e| e.to_string())?
        .ok_or("missing menu TexCoord")?;
    let [u0, u1, v0, v1] = <[f32; 4]>::try_from(uv).map_err(|_| "invalid menu TexCoord")?;
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(0.),
                top: Val::Percent(0.),
                width: Val::Percent(width / crate::mainmenu::FRAME_W * 100.),
                height: Val::Percent(height / crate::mainmenu::FRAME_H * 100.),
                ..default()
            },
            GlobalZIndex(10),
            CampaignControlsRoot,
            peacecraft3::campaign_presentation::CampaignHud,
            peacecraft3::campaign_presentation::CampaignInterface,
        ))
        .id();
    for (name, action) in [
        ("UpperButtonBarQuestsButton", Action::Quests),
        ("UpperButtonBarMenuButton", Action::Menu),
    ] {
        let rect = fdf.rect(name, (width, height)).map_err(|e| e.to_string())?;
        let text = fdf
            .record(name, "NormalText")
            .ok_or("missing upper-menu NormalText")?;
        let quoted: Vec<_> = text.split('"').skip(1).step_by(2).collect();
        let [style, key] = quoted.as_slice() else {
            return Err(format!("invalid NormalText {text}"));
        };
        let mut image = ImageNode::new(source.image.clone());
        image.rect = Some(Rect::new(
            u0 * source.image_size.x,
            v0 * source.image_size.y,
            u1 * source.image_size.x,
            v1 * source.image_size.y,
        ));
        let button = commands
            .spawn((
                UiButton,
                action,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(rect.left / width * 100.),
                    top: Val::Percent((height - rect.top()) / height * 100.),
                    width: Val::Percent(rect.width / width * 100.),
                    height: Val::Percent(rect.height / height * 100.),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                image,
            ))
            .id();
        if matches!(name, "UpperButtonBarQuestsButton") {
            commands
                .entity(button)
                .insert(AuthoredShortcut(source.quests_key));
        }
        commands.entity(root).add_child(button);
        label(commands, button, global(source, key)?, source, style)?;
    }
    Ok(root)
}
fn quest_log(
    commands: &mut Commands,
    source: &mut Source,
    quests: &[&quest_view::QuestProjection<'_>],
    selected: Option<pcx_script::HandleId>,
    images: &Assets<Image>,
    materials: &mut Assets<BackdropImage>,
) -> Result<Entity, String> {
    for quest in quests {
        for text in quest
            .title
            .into_iter()
            .chain(quest.description)
            .chain(quest.items.iter().filter_map(|item| item.description))
        {
            let parsed = crate::rich_text::parse(text).map_err(|e| e.to_string())?;
            if !parsed.unsupported.is_empty() {
                return Err("quest label contains unsupported substitutions".into());
            }
        }
    }
    let width = source.fdf.width("QuestDialog").map_err(|e| e.to_string())?;
    let height = source
        .fdf
        .height("QuestDialog")
        .map_err(|e| e.to_string())?;
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent((1. - width / crate::mainmenu::FRAME_W) * 50.),
                top: Val::Percent((1. - height / crate::mainmenu::FRAME_H) * 50.),
                width: Val::Percent(width / crate::mainmenu::FRAME_W * 100.),
                height: Val::Percent(height / crate::mainmenu::FRAME_H * 100.),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            GlobalZIndex(20),
            CampaignControlsRoot,
            peacecraft3::campaign_presentation::CampaignHud,
        ))
        .id();
    paint_backdrop(
        commands,
        root,
        source,
        Vec2::new(width, height),
        images,
        materials,
    )?;
    let place = |r: crate::fdf::Rect| Node {
        position_type: PositionType::Absolute,
        left: Val::Percent(r.left / width * 100.),
        top: Val::Percent((height - r.top()) / height * 100.),
        width: Val::Percent(r.width / width * 100.),
        height: Val::Percent(r.height / height * 100.),
        ..default()
    };
    for (required, heading, container) in [
        (true, "QuestMainTitle", "QuestMainContainer"),
        (false, "QuestOptionalTitle", "QuestOptionalContainer"),
    ] {
        let rect = source
            .fdf
            .rect(heading, (width, height))
            .map_err(|e| e.to_string())?;
        let parent = commands.spawn((place(rect), Pickable::IGNORE)).id();
        commands.entity(root).add_child(parent);
        let key = source
            .fdf
            .text_value(heading)
            .ok_or("missing quest heading")?;
        label(commands, parent, global(source, &key)?, source, heading)?;
        let rect = source
            .fdf
            .rect(container, (width, height))
            .map_err(|e| e.to_string())?;
        let mut node = place(rect);
        node.flex_direction = FlexDirection::Column;
        node.overflow = Overflow::scroll_y();
        let parent = commands.spawn(node).id();
        commands.entity(root).add_child(parent);
        let row = source
            .fdf
            .height("QuestListItem")
            .map_err(|e| e.to_string())?;
        for quest in quests.iter().filter(|q| q.required == required) {
            let item = commands
                .spawn((
                    UiButton,
                    Action::Select(quest.id),
                    Node {
                        width: Val::Percent(100.),
                        min_height: Val::Vh(row / crate::mainmenu::FRAME_H * 100.),
                        flex_shrink: 0.,
                        ..default()
                    },
                ))
                .id();
            commands.entity(parent).add_child(item);
            if let Some(title) = quest.title {
                label(commands, item, title.into(), source, "QuestListItemTitle")?;
            }
        }
    }
    let rect = source
        .fdf
        .rect("QuestDisplayBackdrop", (width, height))
        .map_err(|e| e.to_string())?;
    let mut node = place(rect);
    node.flex_direction = FlexDirection::Column;
    node.overflow = Overflow::scroll_y();
    // shortcut: details flow within the authored panel, replace after native item/scroll layout is measured.
    let details = commands.spawn(node).id();
    commands.entity(root).add_child(details);
    if let Some(quest) = quests
        .iter()
        .find(|q| Some(q.id) == selected)
        .or_else(|| quests.first())
    {
        if let Some(title) = quest.title {
            label(commands, details, title.into(), source, "QuestDetailsTitle")?;
        }
        let status = if quest.failed {
            "QUESTFAILED"
        } else if quest.completed {
            "QUESTCOMPLETED"
        } else {
            "QUESTNOTCOMPLETED"
        };
        label(
            commands,
            details,
            global(source, status)?,
            source,
            "EscMenuInfoTextTemplate",
        )?;
        for item in &quest.items {
            if let Some(description) = item.description {
                label(
                    commands,
                    details,
                    description.into(),
                    source,
                    "EscMenuInfoTextTemplate",
                )?;
            }
        }
        if let Some(description) = quest.description {
            label(
                commands,
                details,
                description.into(),
                source,
                "QuestDisplay",
            )?;
        }
    }
    let rect = source
        .fdf
        .rect("QuestAcceptButton", (width, height))
        .map_err(|e| e.to_string())?;
    let close = commands.spawn((UiButton, Action::Close, place(rect))).id();
    commands.entity(root).add_child(close);
    label(
        commands,
        close,
        global(source, "QUESTACCEPT")?,
        source,
        "EscMenuButtonTextTemplate",
    )?;
    Ok(root)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn pointer() -> bevy::picking::events::Pointer {
        bevy::picking::events::Pointer::new(
            bevy::picking::pointer::PointerId::Mouse,
            bevy::picking::pointer::Location {
                target: bevy::camera::NormalizedRenderTarget::None {
                    width: 1,
                    height: 1,
                },
                position: Vec2::ZERO,
            },
        )
    }
    fn click(world: &mut World, entity: Entity) {
        use bevy::picking::{
            backend::HitData,
            events::{PointerClick, PointerPress, PointerRelease},
            pointer::PointerButton,
        };
        let mut commands = world.commands();
        commands.trigger(PointerPress {
            entity,
            pointer: pointer(),
            button: PointerButton::Primary,
            hit: HitData::new(entity, 0., None, None),
            count: 1,
        });
        commands.trigger(PointerClick {
            entity,
            pointer: pointer(),
            button: PointerButton::Primary,
            hit: HitData::new(entity, 0., None, None),
            count: 1,
            duration: std::time::Duration::from_millis(1),
        });
        commands.trigger(PointerRelease {
            entity,
            pointer: pointer(),
            button: PointerButton::Primary,
            hit: HitData::new(entity, 0., None, None),
        });
        world.flush();
    }
    #[test]
    fn fast_pointer_events_open_and_close_modal_without_leaking_input() {
        let mut app = App::new();
        app.add_plugins(bevy::ui_widgets::ButtonPlugin)
            .init_resource::<UpperControls>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_message::<CampaignHudAction>()
            .add_observer(capture_pointer_press)
            .add_systems(
                Update,
                (keyboard_shortcuts, activate, crate::button_actions::clear).chain(),
            );
        crate::button_actions::install(&mut app);
        let open = app
            .world_mut()
            .spawn((UiButton, Action::Quests, AuthoredShortcut(KeyCode::F7)))
            .id();
        let close = app.world_mut().spawn((UiButton, Action::Close)).id();
        click(app.world_mut(), open);
        assert!(
            app.world().resource::<UpperControls>().pointer_pending,
            "pointer press is retained independently of activation"
        );
        assert!(!app.world().entity(open).contains::<bevy::ui::Pressed>());
        app.update();
        let state = app.world().resource::<UpperControls>();
        assert!(state.open);
        assert!(state.pointer_consumed);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        click(app.world_mut(), close);
        app.update();
        let state = app.world().resource::<UpperControls>();
        assert!(!state.open);
        assert!(state.escape_consumed);
        assert!(state.pointer_consumed);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        app.update();
        let state = app.world().resource::<UpperControls>();
        assert!(!state.escape_consumed);
        assert!(!state.pointer_consumed);
        app.world_mut()
            .trigger(bevy::ui_widgets::Activate { entity: open });
        app.update();
        assert!(
            app.world().resource::<UpperControls>().open,
            "keyboard Activate uses the same action queue"
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::F7);
        app.update();
        assert!(
            !app.world().resource::<UpperControls>().open,
            "authored function key closes the same modal"
        );
    }
    #[test]
    fn shared_backdrop_tokens_keep_distinct_authored_sampler_modes() {
        let fdf=Fdf::parse("Frame \"BACKDROP\" \"Stretch\" {\nBackdropBackground \"SharedArt\",\n}\nFrame \"BACKDROP\" \"Tile\" {\nBackdropBackground \"SharedArt\",\nBackdropTileBackground,\nBackdropBackgroundSize 0.125,\n}\n").unwrap();
        let token = "SharedArt".to_string();
        let samplers: HashMap<_, _> = ["Stretch", "Tile"]
            .into_iter()
            .map(|name| {
                let art = glue_menu::backdrop(&fdf, name).unwrap().unwrap();
                ((art.background.unwrap(), art.tile), image_sampler(art.tile))
            })
            .collect();
        assert_eq!(samplers.len(), 2);
        for (tiled, address) in [
            (false, bevy::image::ImageAddressMode::ClampToEdge),
            (true, bevy::image::ImageAddressMode::Repeat),
        ] {
            let sampler = &samplers[&(token.clone(), tiled)];
            assert_eq!(sampler.address_mode_u, address);
            assert_eq!(sampler.address_mode_v, address);
        }
    }
    #[test]
    fn simple_templates_keep_font_inheritance_and_authored_offsets() {
        let text = "String \"Label\" {\nFont \"Face\",0.027,\nFontColor 0.1 0.2 0.3,\n}\nFrame \"SIMPLEFRAME\" \"Bar\" {\nWidth 0.3,\nHeight 0.04,\nFrame \"SIMPLEBUTTON\" \"Button\" {\nWidth 0.09,\nHeight 0.03,\nSetPoint TOPLEFT,\"Bar\",TOPLEFT,0.007,-0.002,\n}\n}\n";
        let fdf = Fdf::parse(&normalize(text)).unwrap();
        let style = menu_text::render_style(&fdf, "Label").unwrap();
        assert_eq!(style.height, 0.027);
        assert_eq!(style.colour, [0.1, 0.2, 0.3, 1.]);
        let rect = fdf.rect("Button", (0.8, 0.6)).unwrap();
        assert_eq!(rect.left, 0.007);
        assert!((rect.top() - 0.038).abs() < 0.000001);
    }
}

pub(crate) fn image_sampler(tiled: bool) -> bevy::image::ImageSamplerDescriptor {
    let address = if tiled {
        bevy::image::ImageAddressMode::Repeat
    } else {
        bevy::image::ImageAddressMode::ClampToEdge
    };
    bevy::image::ImageSamplerDescriptor {
        address_mode_u: address,
        address_mode_v: address,
        ..default()
    }
}
fn paint_backdrop(
    commands: &mut Commands,
    parent: Entity,
    source: &mut Source,
    size: Vec2,
    images: &Assets<Image>,
    materials: &mut Assets<BackdropImage>,
) -> Result<(), String> {
    let node = |r: crate::fdf::Rect| Node {
        position_type: PositionType::Absolute,
        left: Val::Percent(r.left / size.x * 100.),
        top: Val::Percent((size.y - r.top()) / size.y * 100.),
        width: Val::Percent(r.width / size.x * 100.),
        height: Val::Percent(r.height / size.y * 100.),
        ..default()
    };
    if let Some(token) = &source.backdrop.background {
        let inset = source.backdrop.inset.unwrap_or(0.);
        let rect = crate::fdf::Rect {
            left: inset,
            bottom: inset,
            width: size.x - 2. * inset,
            height: size.y - 2. * inset,
        };
        let handle = source.backdrop_images[&(token.clone(), source.backdrop.tile)].clone();
        let entity = commands
            .spawn((node(rect), Pickable::IGNORE, ZIndex(-1)))
            .id();
        if source.backdrop.tile {
            let tile = source
                .backdrop
                .background_size
                .ok_or("quest backdrop missing tile size")?;
            if !tile.is_finite() || tile <= 0. {
                return Err("quest backdrop invalid tile size".into());
            }
            let repeats = Vec2::new(rect.width, rect.height) / tile;
            let material = materials.add(BackdropImage {
                texture: handle,
                uv_transform: Vec4::new(repeats.x, repeats.y, 0., 0.),
                uv_rows: Vec4::new(1., 0., 0., 1.),
            });
            commands
                .entity(entity)
                .insert(MaterialNode(material.clone()));
            source.materials.push(material);
        } else {
            commands.entity(entity).insert(ImageNode::new(handle));
        }
        commands.entity(parent).add_child(entity);
    }
    if let Some(edge) = &source.backdrop.edge {
        let handle = source.backdrop_images[&(edge.file.clone(), false)].clone();
        let image = images.get(&handle).ok_or("quest border image removed")?;
        for piece in &edge.pieces {
            let rect = glue_menu::edge_rect(*piece, size, edge.corner_size);
            let uv = glue_menu::edge_source_rect(*piece, image.width(), image.height())
                .map_err(|e| format!("quest border: {e:?}"))?;
            let entity = commands.spawn((node(rect), Pickable::IGNORE)).id();
            if matches!(piece, EdgePiece::Top | EdgePiece::Bottom) {
                let material = materials.add(glue_menu::horizontal_edge_material(
                    handle.clone(),
                    uv,
                    Vec2::new(image.width() as f32, image.height() as f32),
                    false,
                ));
                commands
                    .entity(entity)
                    .insert(MaterialNode(material.clone()));
                source.materials.push(material);
            } else {
                let mut image = ImageNode::new(handle.clone());
                image.rect = Some(uv);
                commands.entity(entity).insert(image);
            }
            commands.entity(parent).add_child(entity);
        }
    }
    Ok(())
}

fn publish_error(state: Res<UpperControls>, mut hud: ResMut<super::CampaignHud>) {
    let receipts = state
        .assets
        .as_ref()
        .map(|source| source.receipts.as_slice())
        .unwrap_or_default();
    if hud.control_receipts != receipts {
        hud.control_receipts = receipts.to_vec();
    }
    hud.quest_log_open = state.open;
    hud.quest_escape_consumed = state.escape_consumed;
    hud.pointer_consumed = state.pointer_consumed;
    if let Some(error) = &state.last_error {
        hud.last_error = Some(format!("campaign controls: {error}"));
    }
}

fn show_controls(
    camera: Res<CampaignCamera>,
    mut roots: Query<&mut Node, With<CampaignControlsRoot>>,
) {
    for mut node in &mut roots {
        node.display = if camera.active() {
            Display::Flex
        } else {
            Display::None
        };
    }
}

#[cfg(test)]
mod supplied_asset_tests {
    use super::*;
    #[test]
    #[ignore = "requires PCX_HUD_FIXTURE_DIR with locally extracted original UpperButtonBar.fdf, QuestDialog.fdf, and EscMenuTemplates.fdf"]
    fn original_upper_controls_and_quest_frames_parse() {
        let root = std::path::PathBuf::from(
            std::env::var_os("PCX_HUD_FIXTURE_DIR").expect("PCX_HUD_FIXTURE_DIR required"),
        );
        let mut fdf = Fdf::parse(&normalize(
            &std::fs::read_to_string(root.join("UpperButtonBar.fdf")).unwrap(),
        ))
        .unwrap();
        for name in ["EscMenuTemplates.fdf", "QuestDialog.fdf"] {
            fdf.parse_into(&std::fs::read_to_string(root.join(name)).unwrap())
                .unwrap();
        }
        for name in [
            "UpperButtonBarButtonTextTemplate",
            "QuestMainTitle",
            "QuestDetailsTitle",
            "QuestListItemTitle",
            "QuestDisplay",
        ] {
            menu_text::render_style(&fdf, name).unwrap();
        }
        assert!(
            crate::glue_menu::backdrop(&fdf, "QuestBackdrop")
                .unwrap()
                .is_some()
        );
    }
}

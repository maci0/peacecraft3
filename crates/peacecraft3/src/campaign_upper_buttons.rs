//! Source-authored upper controls and live quest log. Root placement is a
//! recreation choice: upper controls at the viewport top left, quest log centered.
use crate::glue_menu::{self, AdditiveImage, BackdropImage, EdgePiece};
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

#[path = "campaign_message_log.rs"]
mod message_log;

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
    MessageLog,
    Close,
    CloseMessageLog,
    Select(pcx_script::HandleId),
}
/// Session-provider tags `Warcraft III.exe` compares at 0x005ad700 (global object
/// 13, field +8, offset 0x2650): the fourth bar button is Chat unless the tag is
/// `LOOP` or `NONE`, in which case it is Log. 0x0062e440 enables Allies only when
/// that same test says Chat. The tags are read as the local single-player session
/// (S, shipped code); that reading is not traced further.
const PROVIDER_LOOPBACK: u32 = u32::from_be_bytes(*b"LOOP");
const PROVIDER_NONE: u32 = u32::from_be_bytes(*b"NONE");
/// The campaign HUD only serves the local single-player session.
const CAMPAIGN_PROVIDER: u32 = PROVIDER_LOOPBACK;
fn networked(provider: u32) -> bool {
    !matches!(provider, PROVIDER_LOOPBACK | PROVIDER_NONE)
}
/// The upper bar's buttons in the authored `UpperButtonBar.fdf` anchor chain.
/// The fourth FDF frame is `UpperButtonBarChatButton`; the client relabels it Log
/// and makes it open the message log for a local session (see [`networked`]).
/// Allies is disabled for a local session. 0x0062e440 also keeps it disabled when
/// a byte at CUpperButtonBar+0x170 is set or when the predicate at 0x005edaa0
/// holds for the local player; neither is traced, and both are moot while the
/// session is local.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UpperButton {
    Quests,
    Menu,
    Allies,
    Log,
}
impl UpperButton {
    const ALL: [Self; 4] = [Self::Quests, Self::Menu, Self::Allies, Self::Log];
    fn frame(self) -> &'static str {
        match self {
            Self::Quests => "UpperButtonBarQuestsButton",
            Self::Menu => "UpperButtonBarMenuButton",
            Self::Allies => "UpperButtonBarAlliesButton",
            Self::Log => "UpperButtonBarChatButton",
        }
    }
    fn action(self, provider: u32) -> Option<Action> {
        match self {
            Self::Quests => Some(Action::Quests),
            Self::Menu => Some(Action::Menu),
            Self::Allies => None,
            Self::Log => (!networked(provider)).then_some(Action::MessageLog),
        }
    }
    fn disabled(self, provider: u32) -> bool {
        self == Self::Allies && !networked(provider)
    }
    /// Global-string key shown for the authored `key`.
    fn text_key(self, authored: &str, provider: u32) -> &str {
        match (self, authored, networked(provider)) {
            (Self::Log, "KEY_CHAT", false) => "KEY_LOG",
            (Self::Log, "CHAT", false) => "LOG",
            _ => authored,
        }
    }
}
#[derive(Component)]
struct AuthoredShortcut(KeyCode);
#[derive(Component)]
struct CampaignControlsRoot;
#[derive(Component, Clone, Copy)]
struct UpperButtonArt {
    normal: Rect,
    pushed: Rect,
    disabled: Rect,
    highlight: Rect,
    text_offset: Vec2,
}
#[derive(Component)]
struct UpperButtonLabel(Entity);
#[derive(Component)]
struct UpperButtonHover(Entity);
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum UpperButtonTextState {
    Normal,
    Highlight,
    Disabled,
}

#[derive(Clone, Copy)]
enum QuestButtonState {
    Normal,
    Pushed,
    Disabled,
    Hover,
}
#[derive(Component)]
struct QuestButtonLayer(Entity, QuestButtonState);
#[derive(Component)]
struct QuestButtonLabel {
    owner: Entity,
    offset: Vec2,
}

#[derive(Component)]
struct InheritedLabelColour;
#[derive(Component)]
struct QuestButtonText {
    owner: Entity,
    normal: Color,
    highlight: Color,
    disabled: Color,
}

#[derive(Component)]
#[require(bevy::ui_widgets::ScrollArea)]
struct QuestScrollArea;

fn redirect_quest_scroll(
    mut event: On<bevy::picking::events::PointerScroll>,
    areas: Query<(), With<QuestScrollArea>>,
    parents: Query<&ChildOf>,
    mut commands: Commands,
) {
    if areas.contains(event.entity) {
        return;
    }
    if let Some(area) = parents
        .iter_ancestors(event.entity)
        .find(|entity| areas.contains(*entity))
    {
        let forwarded = bevy::picking::events::PointerScroll {
            entity: area,
            ..event.event().clone()
        };
        event.propagate(false);
        commands.trigger(forwarded);
    }
}

#[derive(Resource, Default)]
struct UpperControls {
    witness: Option<Weak<()>>,
    root: Option<Entity>,
    /// The quest log dialog, not the message log.
    log: Option<Entity>,
    messages: message_log::Shown,
    messages_open: bool,
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
    message_log: message_log::Art,
    fonts: HashMap<String, Handle<Font>>,
    backdrop: glue_menu::Backdrop,
    details_backdrop: glue_menu::Backdrop,
    accept: glue_menu::Button,
    accept_material: Option<Handle<AdditiveImage>>,
    backdrop_images: HashMap<(String, bool), Handle<Image>>,
    materials: Vec<Handle<BackdropImage>>,
    highlights: Vec<Handle<AdditiveImage>>,
    image: Handle<Image>,
    image_size: Vec2,
}

pub(super) fn install(app: &mut App) {
    crate::button_actions::install(app);
    app.add_observer(capture_pointer_press);
    app.add_observer(redirect_quest_scroll);
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
                show_button_states,
                show_hover,
                show_quest_button,
                show_quest_button_text,
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
    debug: Option<Res<pcx_gfx::performance::DebugOverlay>>,
) {
    if camera.as_ref().is_some_and(|camera| !camera.active())
        || !crate::button_actions::input_allowed(presentation)
    {
        return;
    }
    for (entity, shortcut) in &buttons {
        if shortcut.0 == KeyCode::F7 && debug.is_some() {
            continue;
        }
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
    camera: Option<Res<CampaignCamera>>,
) {
    if camera.is_some_and(|camera| camera.menu_open()) {
        state.escape_consumed = false;
        state.pointer_consumed = std::mem::take(&mut state.pointer_pending);
        return;
    }
    state.escape_consumed =
        (state.open || state.messages_open) && keys.just_pressed(KeyCode::Escape);
    state.pointer_consumed = std::mem::take(&mut state.pointer_pending);
    for entity in &activations.entities {
        let Ok(action) = buttons.get(*entity) else {
            continue;
        };
        state.pointer_consumed = true;
        // One modal dialog at a time (recreation choice).
        match action {
            Action::Quests => {
                state.open = !state.open;
                state.messages_open = false;
            }
            Action::Close => state.open = false,
            Action::MessageLog => {
                state.messages_open = !state.messages_open;
                state.open = false;
            }
            Action::CloseMessageLog => state.messages_open = false,
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
        state.messages_open = false;
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
    highlights: ResMut<'w, Assets<AdditiveImage>>,
    history: ResMut<'w, crate::campaign_messages::MessageHistory>,
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
        mut highlights,
        mut history,
    } = assets;
    let witness = session.session_witness();
    let same = match (&state.witness, &witness) {
        (Some(a), Some(b)) => Weak::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    };
    if !same {
        for entity in [
            state.root.take(),
            state.log.take(),
            state.messages.entity.take(),
        ]
        .into_iter()
        .flatten()
        {
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
            for handle in assets.highlights {
                highlights.remove(handle.id());
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
    for log in state.log.into_iter().chain(state.messages.entity) {
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
            Ok(mut source) => match upper(&mut commands, &mut source, &mut highlights) {
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
    }
    message_log::sync(
        &mut commands,
        &mut state,
        &mut history,
        &images,
        &mut materials,
    );
    if !state.open {
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
        message_log::FDF,
    ] {
        fdf.parse_into(&text(reader, path, &mut receipts)?)
            .map_err(|e| format!("{path}: {e}"))?;
    }
    for name in ["QuestMainTitle", "QuestOptionalTitle", "QuestDetailsTitle"] {
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
        "QuestItemListContainer",
        "QuestDisplay",
        "QuestAcceptButton",
    ] {
        fdf.rect(name, view).map_err(|e| e.to_string())?;
    }
    text_area(&fdf, "QuestDisplay")?;
    let globals = text(reader, "UI\\FrameDef\\GlobalStrings.fdf", &mut receipts)?;
    let message_log = message_log::Art::load(&mut fdf, &globals)?;
    let file = |key: &str| skin_file(&skin, &sections, key);
    let mut decoded_fonts = HashMap::new();
    let mut font_styles = vec![
        "UpperButtonBarButtonTextTemplate".to_string(),
        "QuestDetailsTitle".into(),
        "EscMenuInfoTextTemplate".into(),
        "EscMenuButtonTextTemplate".into(),
        "QuestListItemTitle".into(),
        "QuestItemListItemTitle".into(),
        "QuestDisplay".into(),
        "QuestMainTitle".into(),
        "QuestOptionalTitle".into(),
        "QuestListItemComplete".into(),
    ];
    for button in UpperButton::ALL {
        for property in ["NormalText", "HighlightText", "DisabledText"] {
            font_styles.push(upper_text(&fdf, button.frame(), property)?.0);
        }
    }
    font_styles.push(
        fdf.string("QuestAcceptButton", "ButtonText")
            .ok_or("missing quest accept text")?,
    );
    font_styles.extend(message_log.font_styles());
    for name in font_styles {
        let style = menu_text::render_style(&fdf, &name)?;
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
    let details_backdrop = glue_menu::backdrop(&fdf, "QuestDisplayBackdrop")
        .map_err(|e| e.to_string())?
        .ok_or("missing quest details backdrop")?;
    let accept = glue_menu::button(&fdf, "QuestAcceptButton").map_err(|e| e.to_string())?;
    if accept.normal.is_none()
        || accept.pushed.is_none()
        || accept.disabled.is_none()
        || accept.pushed_text_offset.is_none()
    {
        return Err("quest accept button lacks authored state art/offset".into());
    }
    let mut decoded_backdrops = HashMap::new();
    for art in [&backdrop, &details_backdrop]
        .into_iter()
        .chain(
            [
                accept.normal.as_ref(),
                accept.pushed.as_ref(),
                accept.disabled.as_ref(),
            ]
            .into_iter()
            .flatten(),
        )
        .chain(message_log.backdrops())
    {
        for (token, tiled) in art
            .background
            .iter()
            .map(|token| (token, art.tile))
            .chain(art.edge.iter().map(|edge| (&edge.file, false)))
        {
            if decoded_backdrops.contains_key(&(token.clone(), tiled)) {
                continue;
            }
            let path = file(token)?;
            let bytes = read(reader, &path, 4 << 20, &mut receipts)?;
            let decoded =
                pcx_gfx::load_image_bounded(&bytes, &path, 16 << 20).map_err(|e| e.to_string())?;
            if let Some(edge) = &art.edge
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
    }
    for glow in accept
        .mouse_over
        .iter()
        .chain(message_log.ok.mouse_over.iter())
    {
        if !glow.additive {
            return Err("dialog button highlight requires unsupported blend mode".into());
        }
        if decoded_backdrops.contains_key(&(glow.file.clone(), false)) {
            continue;
        }
        let path = file(&glow.file)?;
        let bytes = read(reader, &path, 4 << 20, &mut receipts)?;
        let decoded =
            pcx_gfx::load_image_bounded(&bytes, &path, 16 << 20).map_err(|e| e.to_string())?;
        decoded_backdrops.insert(
            (glow.file.clone(), false),
            pcx_gfx::viewer::rgba_image(&decoded),
        );
    }
    let image_key = fdf
        .string("UpperMenuButtonBackground", "File")
        .ok_or("missing upper-menu texture")?;
    let image_path = file(&image_key)?;
    let image = read(reader, &image_path, 4 << 20, &mut receipts)?;
    let decoded =
        pcx_gfx::load_image_bounded(&image, &image_path, 16 << 20).map_err(|e| e.to_string())?;
    let image_size = Vec2::new(decoded.width as f32, decoded.height as f32);
    for key in [
        "KEY_QUESTS",
        "KEY_MENU",
        "KEY_ALLIES",
        "KEY_LOG",
        "ALLIES",
        "LOG",
        "QUESTSMAIN",
        "QUESTSOPTIONAL",
        "QUESTACCEPT",
        "QUESTCOMPLETED",
        "QUESTFAILED",
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
        message_log,
        fdf,
        globals,
        fonts: decoded_fonts
            .into_iter()
            .map(|(key, font)| (key, fonts.add(font)))
            .collect(),
        backdrop,
        details_backdrop,
        accept,
        accept_material: None,
        backdrop_images: decoded_backdrops
            .into_iter()
            .map(|(key, image)| (key, images.add(image)))
            .collect(),
        materials: Vec::new(),
        highlights: Vec::new(),
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
) -> Result<Entity, String> {
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
            TextShadow {
                offset: Vec2::ZERO,
                color: colour(style.shadow_colour),
            },
            glue_menu::FrameShadow(Vec2::from(style.shadow_offset)),
            Pickable::IGNORE,
        ))
        .id();
    for span in parsed.spans {
        let mut run = commands.spawn((
            TextSpan::new(span.text),
            font.clone(),
            TextColor(colour(span.colour.unwrap_or(style.colour))),
        ));
        if span.colour.is_none() {
            run.insert(InheritedLabelColour);
        }
        let run = run.id();
        commands.entity(child).add_child(run);
    }
    commands.entity(parent).add_child(child);
    Ok(child)
}
fn global(source: &Source, key: &str) -> Result<String, String> {
    menu_text::global_string(&source.globals, key)
        .ok_or_else(|| format!("missing global string {key}"))
}
fn text_area_number(fdf: &Fdf, frame: &str, property: &'static str) -> Result<f32, String> {
    let values = fdf
        .numbers(frame, property)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("{frame}: missing {property}"))?;
    let [value] = values.as_slice() else {
        return Err(format!("{frame}: invalid {property}"));
    };
    if !value.is_finite() {
        return Err(format!("{frame}: nonfinite {property}"));
    }
    Ok(*value)
}
/// A `TEXTAREA`'s inset and line pitch (line height plus gap) as a multiple of
/// its font height.
fn text_area(fdf: &Fdf, frame: &str) -> Result<(f32, f32), String> {
    let inset = text_area_number(fdf, frame, "TextAreaInset")?;
    let height = text_area_number(fdf, frame, "TextAreaLineHeight")?;
    let gap = text_area_number(fdf, frame, "TextAreaLineGap")?;
    let font = menu_text::render_style(fdf, frame)?;
    let ratio = (height + gap) / font.height;
    if inset < 0. || height <= 0. || gap < 0. || !ratio.is_finite() || ratio <= 0. {
        return Err(format!("{frame}: invalid text area dimensions"));
    }
    Ok((inset, ratio))
}
fn upper_text(fdf: &Fdf, name: &str, property: &str) -> Result<(String, String), String> {
    let text = fdf
        .record(name, property)
        .ok_or_else(|| format!("{name}: missing {property}"))?;
    let quoted = text.split('"').skip(1).step_by(2).collect::<Vec<_>>();
    let [style, key] = quoted.as_slice() else {
        return Err(format!("{name}: invalid {property}"));
    };
    menu_text::render_style(fdf, style)?;
    Ok((style.to_string(), key.to_string()))
}
fn upper_button_art(
    fdf: &Fdf,
    name: &str,
    image_size: Vec2,
    size: Vec2,
) -> Result<UpperButtonArt, String> {
    let atlas = fdf
        .string("UpperMenuButtonBackground", "File")
        .ok_or("missing upper-menu atlas")?;
    let region = |property| -> Result<Rect, String> {
        let texture = fdf
            .string(name, property)
            .ok_or_else(|| format!("{name}: missing {property}"))?;
        if fdf.string(&texture, "File").as_ref() != Some(&atlas) {
            return Err(format!("{name}: state texture uses another atlas"));
        }
        let uv = fdf
            .numbers(&texture, "TexCoord")
            .map_err(|e| e.to_string())?
            .ok_or("missing menu TexCoord")?;
        let [u0, u1, v0, v1] = <[f32; 4]>::try_from(uv).map_err(|_| "invalid menu TexCoord")?;
        if [u0, u1, v0, v1]
            .iter()
            .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
            || u1 <= u0
            || v1 <= v0
        {
            return Err(format!("{name}: invalid menu texture region"));
        }
        Ok(Rect::new(
            u0 * image_size.x,
            v0 * image_size.y,
            u1 * image_size.x,
            v1 * image_size.y,
        ))
    };
    let offset = fdf
        .numbers(name, "ButtonPushedTextOffset")
        .map_err(|e| e.to_string())?
        .ok_or("missing pushed-text offset")?;
    let [x, y] = <[f32; 2]>::try_from(offset).map_err(|_| "invalid pushed-text offset")?;
    if !Vec2::new(x, y).is_finite()
        || !size.is_finite()
        || size.min_element() <= 0.
        || !image_size.is_finite()
        || image_size.min_element() <= 0.
    {
        return Err(format!("{name}: invalid upper-button dimensions/offset"));
    }
    let text_offset = Vec2::new(x / size.x * 100., -y / size.y * 100.);
    if !text_offset.is_finite() {
        return Err(format!("{name}: pushed-text ratio overflow"));
    }
    Ok(UpperButtonArt {
        normal: region("NormalTexture")?,
        pushed: region("PushedTexture")?,
        disabled: region("DisabledTexture")?,
        highlight: {
            let texture = fdf
                .string(name, "UseHighlight")
                .ok_or("missing upper-menu highlight")?;
            if fdf.string(&texture, "AlphaMode").as_deref() != Some("ADD") {
                return Err(format!("{name}: unsupported highlight alpha mode"));
            }
            region("UseHighlight")?
        },
        text_offset,
    })
}
fn show_button_states(
    mut buttons: Query<(
        &UpperButtonArt,
        Has<bevy::ui::Pressed>,
        Has<InteractionDisabled>,
        &mut ImageNode,
    )>,
    mut labels: Query<(&UpperButtonLabel, &mut Node)>,
) {
    for (art, pressed, disabled, mut image) in &mut buttons {
        let rect = if disabled {
            art.disabled
        } else if pressed {
            art.pushed
        } else {
            art.normal
        };
        if image.rect != Some(rect) {
            image.rect = Some(rect);
        }
    }
    for (label, mut node) in &mut labels {
        let Ok((art, pressed, disabled, _)) = buttons.get(label.0) else {
            continue;
        };
        let offset = if pressed && !disabled {
            art.text_offset
        } else {
            Vec2::ZERO
        };
        let (left, top) = (Val::Percent(offset.x), Val::Percent(offset.y));
        if node.left != left || node.top != top {
            node.left = left;
            node.top = top;
        }
    }
}
fn show_hover(
    buttons: Query<
        (&bevy::picking::hover::Hovered, Has<InteractionDisabled>),
        With<UpperButtonArt>,
    >,
    mut layers: Query<(&UpperButtonHover, &mut Visibility), Without<UpperButtonTextState>>,
    mut labels: Query<(&UpperButtonLabel, &UpperButtonTextState, &mut Visibility)>,
) {
    for (owner, mut visibility) in &mut layers {
        let active = buttons
            .get(owner.0)
            .is_ok_and(|(hover, disabled)| hover.get() && !disabled);
        *visibility = if active {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    for (owner, state, mut visibility) in &mut labels {
        let active = buttons
            .get(owner.0)
            .map(|(hover, disabled)| {
                if disabled {
                    UpperButtonTextState::Disabled
                } else if hover.get() {
                    UpperButtonTextState::Highlight
                } else {
                    UpperButtonTextState::Normal
                }
            })
            .ok();
        *visibility = if active == Some(*state) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}
fn show_quest_button(
    buttons: Query<(
        &bevy::picking::hover::Hovered,
        Has<bevy::ui::Pressed>,
        Has<InteractionDisabled>,
    )>,
    mut layers: Query<(&QuestButtonLayer, &mut Visibility)>,
    mut labels: Query<(&QuestButtonLabel, &mut Node)>,
) {
    for (layer, mut visibility) in &mut layers {
        let visible = buttons
            .get(layer.0)
            .is_ok_and(|(hover, pressed, disabled)| match layer.1 {
                QuestButtonState::Normal => !pressed && !disabled,
                QuestButtonState::Pushed => pressed && !disabled,
                QuestButtonState::Disabled => disabled,
                QuestButtonState::Hover => hover.get() && !disabled,
            });
        *visibility = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    for (label, mut node) in &mut labels {
        let offset = if buttons
            .get(label.owner)
            .is_ok_and(|(_, pressed, disabled)| pressed && !disabled)
        {
            label.offset
        } else {
            Vec2::ZERO
        };
        node.left = Val::Percent(offset.x);
        node.top = Val::Percent(offset.y);
    }
}
fn show_quest_button_text(
    buttons: Query<(&bevy::picking::hover::Hovered, Has<InteractionDisabled>)>,
    mut labels: Query<(&QuestButtonText, &Children, &mut TextColor)>,
    mut spans: Query<&mut TextColor, (With<InheritedLabelColour>, Without<QuestButtonText>)>,
) {
    for (style, children, mut root_colour) in &mut labels {
        let Ok((hover, disabled)) = buttons.get(style.owner) else {
            continue;
        };
        let colour = if disabled {
            style.disabled
        } else if hover.get() {
            style.highlight
        } else {
            style.normal
        };
        root_colour.0 = colour;
        for child in children {
            if let Ok(mut span_colour) = spans.get_mut(*child) {
                span_colour.0 = colour;
            }
        }
    }
}
/// The additive hover material for a dialog button's authored highlight art.
fn glow_material(
    source: &mut Source,
    file: Option<String>,
    highlights: &mut Assets<AdditiveImage>,
) -> Result<Option<Handle<AdditiveImage>>, String> {
    let Some(file) = file else {
        return Ok(None);
    };
    let handle = source
        .backdrop_images
        .get(&(file, false))
        .ok_or("dialog button highlight missing")?;
    let material = highlights.add(AdditiveImage::new(handle.clone()));
    source.highlights.push(material.clone());
    Ok(Some(material))
}
fn upper(
    commands: &mut Commands,
    source: &mut Source,
    highlights: &mut Assets<AdditiveImage>,
) -> Result<Entity, String> {
    let accept_glow = source.accept.mouse_over.as_ref().map(|g| g.file.clone());
    source.accept_material = glow_material(source, accept_glow, highlights)?;
    let ok_glow = source
        .message_log
        .ok
        .mouse_over
        .as_ref()
        .map(|g| g.file.clone());
    source.message_log.ok_material = glow_material(source, ok_glow, highlights)?;
    let fdf = &source.fdf;
    let width = fdf
        .width("UpperButtonBarFrame")
        .map_err(|e| e.to_string())?;
    let height = fdf
        .height("UpperButtonBarFrame")
        .map_err(|e| e.to_string())?;
    let profiles = UpperButton::ALL
        .into_iter()
        .map(|button| {
            let name = button.frame();
            let rect = fdf.rect(name, (width, height)).map_err(|e| e.to_string())?;
            let art = upper_button_art(
                fdf,
                name,
                source.image_size,
                Vec2::new(rect.width, rect.height),
            )?;
            let texts = [
                (UpperButtonTextState::Normal, "NormalText"),
                (UpperButtonTextState::Highlight, "HighlightText"),
                (UpperButtonTextState::Disabled, "DisabledText"),
            ]
            .map(|(state, property)| {
                let (style, key) = upper_text(fdf, name, property)?;
                Ok((
                    state,
                    style,
                    global(source, button.text_key(&key, CAMPAIGN_PROVIDER))?,
                ))
            })
            .into_iter()
            .collect::<Result<Vec<_>, String>>()?;
            Ok((button, rect, art, texts))
        })
        .collect::<Result<Vec<_>, String>>()?;
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
    for (profile, rect, art, texts) in profiles {
        let name = profile.frame();
        let mut image = ImageNode::new(source.image.clone())
            .with_mode(bevy::ui::widget::NodeImageMode::Stretch);
        image.rect = Some(art.normal);
        let button = commands
            .spawn((
                UiButton,
                bevy::picking::hover::Hovered::default(),
                art,
                Name::new(name),
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
        if let Some(action) = profile.action(CAMPAIGN_PROVIDER) {
            commands.entity(button).insert(action);
        }
        if profile.disabled(CAMPAIGN_PROVIDER) {
            commands.entity(button).insert(InteractionDisabled);
        }
        if profile == UpperButton::Quests {
            commands
                .entity(button)
                .insert(AuthoredShortcut(source.quests_key));
        }
        // Menu's key is consumed by the campaign camera; Allies has no dialog.
        if profile == UpperButton::Log && !networked(CAMPAIGN_PROVIDER) {
            commands
                .entity(button)
                .insert(AuthoredShortcut(source.message_log.key));
        }
        commands.entity(root).add_child(button);
        let uv = art.highlight;
        let material = highlights.add(AdditiveImage {
            texture: source.image.clone(),
            uv_transform: Vec4::new(
                uv.width() / source.image_size.x,
                uv.height() / source.image_size.y,
                uv.min.x / source.image_size.x,
                uv.min.y / source.image_size.y,
            ),
        });
        source.highlights.push(material.clone());
        commands.entity(button).with_child((
            MaterialNode(material),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                ..default()
            },
            UpperButtonHover(button),
            Visibility::Hidden,
            Pickable::IGNORE,
        ));
        for (state, style, text) in texts {
            let wrapper = commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Percent(0.),
                        top: Val::Percent(0.),
                        width: Val::Percent(100.),
                        height: Val::Percent(100.),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                    UpperButtonLabel(button),
                    state,
                    if state == UpperButtonTextState::Normal {
                        Visibility::Inherited
                    } else {
                        Visibility::Hidden
                    },
                    Pickable::IGNORE,
                ))
                .id();
            commands.entity(button).add_child(wrapper);
            label(commands, wrapper, text, source, &style)?;
        }
    }
    Ok(root)
}
/// Text around a completed quest item's `QUESTCOMPLETED` suffix, the literals
/// `Warcraft III.exe` appends at 0x00657c49..0x00657c68.
const COMPLETED_OPEN: &str = " ( ";
const COMPLETED_CLOSE: &str = " )";
/// The global string a quest list row shows in `QuestListItemComplete`. The
/// client loads only `QUESTCOMPLETED` and `QUESTFAILED` (0x00633c1c, 0x00633c3c),
/// tests completed before failed (0x00657d6c..0x00657d99) and hides the label for
/// neither.
fn row_status_key(completed: bool, failed: bool) -> Option<&'static str> {
    if completed {
        Some("QUESTCOMPLETED")
    } else if failed {
        Some("QUESTFAILED")
    } else {
        None
    }
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
    let backdrop = source.backdrop.clone();
    paint_backdrop(
        commands,
        root,
        source,
        &backdrop,
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
        let parent = commands.spawn((node, QuestScrollArea)).id();
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
                        flex_direction: FlexDirection::Column,
                        flex_shrink: 0.,
                        ..default()
                    },
                ))
                .id();
            commands.entity(parent).add_child(item);
            if let Some(title) = quest.title {
                label(commands, item, title.into(), source, "QuestListItemTitle")?;
            }
            if let Some(key) = row_status_key(quest.completed, quest.failed) {
                label(
                    commands,
                    item,
                    global(source, key)?,
                    source,
                    "QuestListItemComplete",
                )?;
            }
        }
    }
    let rect = source
        .fdf
        .rect("QuestDisplayBackdrop", (width, height))
        .map_err(|e| e.to_string())?;
    let details = commands
        .spawn((place(rect), Name::new("QuestDisplayBackdrop")))
        .id();
    commands.entity(root).add_child(details);
    let details_art = source.details_backdrop.clone();
    paint_backdrop(
        commands,
        details,
        source,
        &details_art,
        Vec2::new(rect.width, rect.height),
        images,
        materials,
    )?;
    if let Some(quest) = quests
        .iter()
        .find(|q| Some(q.id) == selected)
        .or_else(|| quests.first())
    {
        if let Some(title) = quest.title {
            let title_rect = source
                .fdf
                .rect("QuestDetailsTitle", (width, height))
                .map_err(|e| e.to_string())?;
            let title_frame = commands
                .spawn((
                    place(title_rect),
                    Name::new("QuestDetailsTitle"),
                    Pickable::IGNORE,
                ))
                .id();
            commands.entity(root).add_child(title_frame);
            label(
                commands,
                title_frame,
                title.into(),
                source,
                "QuestDetailsTitle",
            )?;
        }
        let item_rect = source
            .fdf
            .rect("QuestItemListContainer", (width, height))
            .map_err(|e| e.to_string())?;
        let mut node = place(item_rect);
        node.flex_direction = FlexDirection::Column;
        node.overflow = Overflow::scroll_y();
        let items = commands
            .spawn((node, QuestScrollArea, Name::new("QuestItemListContainer")))
            .id();
        commands.entity(root).add_child(items);
        for item in &quest.items {
            if let Some(description) = item.description {
                let text = if item.completed {
                    format!(
                        "{description}{COMPLETED_OPEN}{}{COMPLETED_CLOSE}",
                        global(source, "QUESTCOMPLETED")?
                    )
                } else {
                    description.into()
                };
                label(commands, items, text, source, "QuestItemListItemTitle")?;
            }
        }
        if let Some(description) = quest.description {
            let rect = source
                .fdf
                .rect("QuestDisplay", (width, height))
                .map_err(|e| e.to_string())?;
            let (inset, line_height) = text_area(&source.fdf, "QuestDisplay")?;
            if rect.width <= 2. * inset || rect.height <= 2. * inset {
                return Err("QuestDisplay: inset consumes text area".into());
            }
            let mut node = place(rect);
            node.padding = UiRect::all(Val::Vh(inset / crate::mainmenu::FRAME_H * 100.));
            node.overflow = Overflow::scroll_y();
            let display = commands
                .spawn((node, QuestScrollArea, Name::new("QuestDisplay")))
                .id();
            commands.entity(root).add_child(display);
            let text = label(
                commands,
                display,
                description.into(),
                source,
                "QuestDisplay",
            )?;
            commands
                .entity(text)
                .insert(bevy::text::LineHeight::RelativeToFont(line_height));
            // shortcut: native scrollbar and line-count behavior remain unqualified; bind after their source lifecycle is reviewed.
        }
    }
    let accept = source.accept.clone();
    let material = source.accept_material.clone();
    dialog_button(
        commands,
        source,
        root,
        DialogButton {
            frame: "QuestAcceptButton",
            art: accept,
            material,
            action: Action::Close,
        },
        (width, height),
        images,
        materials,
    )?;
    Ok(root)
}
/// A modal dialog's authored button: state art, hover glow and label, bound to
/// `action`. `view` is the dialog's size in frame units.
struct DialogButton {
    frame: &'static str,
    art: glue_menu::Button,
    material: Option<Handle<AdditiveImage>>,
    action: Action,
}
fn dialog_button(
    commands: &mut Commands,
    source: &mut Source,
    parent: Entity,
    button: DialogButton,
    view: (f32, f32),
    images: &Assets<Image>,
    materials: &mut Assets<BackdropImage>,
) -> Result<Entity, String> {
    let DialogButton {
        frame,
        art: accept,
        material,
        action,
    } = button;
    let rect = source.fdf.rect(frame, view).map_err(|e| e.to_string())?;
    let (width, height) = view;
    let close = commands
        .spawn((
            UiButton,
            bevy::picking::hover::Hovered::default(),
            action,
            Name::new(frame),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(rect.left / width * 100.),
                top: Val::Percent((height - rect.top()) / height * 100.),
                width: Val::Percent(rect.width / width * 100.),
                height: Val::Percent(rect.height / height * 100.),
                ..default()
            },
        ))
        .id();
    commands.entity(parent).add_child(close);
    let size = Vec2::new(rect.width, rect.height);
    for (state, art) in [
        (QuestButtonState::Normal, accept.normal.as_ref()),
        (QuestButtonState::Pushed, accept.pushed.as_ref()),
        (QuestButtonState::Disabled, accept.disabled.as_ref()),
    ] {
        let art = art.ok_or("dialog button state art missing")?;
        let layer = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.),
                    height: Val::Percent(100.),
                    ..default()
                },
                QuestButtonLayer(close, state),
                Visibility::Hidden,
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(close).add_child(layer);
        paint_backdrop(commands, layer, source, art, size, images, materials)?;
    }
    if let Some(material) = &material {
        commands.entity(close).with_child((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                ..default()
            },
            MaterialNode(material.clone()),
            QuestButtonLayer(close, QuestButtonState::Hover),
            Visibility::Hidden,
            Pickable::IGNORE,
        ));
    }
    let offset = accept
        .pushed_text_offset
        .ok_or("dialog button text offset missing")?;
    let offset = Vec2::new(offset[0] / size.x * 100., -offset[1] / size.y * 100.);
    if !offset.is_finite() {
        return Err("dialog button text offset ratio invalid".into());
    }
    let wrapper = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            QuestButtonLabel {
                owner: close,
                offset,
            },
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(close).add_child(wrapper);
    let text_frame = accept.text.as_deref().ok_or("dialog button text missing")?;
    let key = source
        .fdf
        .text_value(text_frame)
        .ok_or("dialog button text key missing")?;
    let style = menu_text::render_style(&source.fdf, text_frame)?;
    let text = label(commands, wrapper, global(source, &key)?, source, text_frame)?;
    commands.entity(text).insert(QuestButtonText {
        owner: close,
        normal: colour(style.colour),
        highlight: colour(style.highlight),
        disabled: colour(style.disabled),
    });
    Ok(close)
}
#[cfg(test)]
mod tests {
    pub(super) fn blank_backdrop() -> glue_menu::Backdrop {
        glue_menu::Backdrop {
            background: None,
            tile: false,
            background_size: None,
            mirrored: false,
            inset: None,
            edge: None,
        }
    }
    pub(super) fn fixture_source(fdf: Fdf) -> Source {
        let font = Handle::<Font>::default();
        Source {
            receipts: Vec::new(),
            fdf,
            globals: String::new(),
            quests_key: KeyCode::F1,
            message_log: message_log::Art {
                backdrop: blank_backdrop(),
                area_backdrop: blank_backdrop(),
                ok: glue_menu::Button {
                    normal: Some(blank_backdrop()),
                    pushed: Some(blank_backdrop()),
                    disabled: Some(blank_backdrop()),
                    mouse_over: None,
                    text: Some("LogOkButtonText".into()),
                    pushed_text_offset: Some([0.001, -0.001]),
                    shortcut: None,
                },
                ok_material: None,
                key: KeyCode::F12,
                max_entries: 4,
            },
            fonts: HashMap::from([("FixtureFont".into(), font)]),
            backdrop: glue_menu::Backdrop {
                background: None,
                tile: false,
                background_size: None,
                mirrored: false,
                inset: None,
                edge: None,
            },
            details_backdrop: glue_menu::Backdrop {
                background: None,
                tile: false,
                background_size: None,
                mirrored: false,
                inset: None,
                edge: None,
            },
            accept: glue_menu::Button {
                normal: None,
                pushed: None,
                disabled: None,
                mouse_over: None,
                text: None,
                pushed_text_offset: None,
                shortcut: None,
            },
            accept_material: None,
            backdrop_images: HashMap::new(),
            materials: Vec::new(),
            highlights: Vec::new(),
            image: Handle::default(),
            image_size: Vec2::ONE,
        }
    }

    #[test]
    fn quest_scroll_area_receives_child_wheel_events_and_clamps() {
        use bevy::picking::{backend::HitData, events::PointerScroll};
        let mut app = App::new();
        app.init_resource::<bevy::input::mouse::MouseScrollPixelsPerLine>()
            .add_plugins(bevy::ui_widgets::ScrollAreaPlugin)
            .add_observer(redirect_quest_scroll);
        let area = app
            .world_mut()
            .spawn((
                Node {
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                QuestScrollArea,
                ComputedNode {
                    size: Vec2::new(120., 40.),
                    content_size: Vec2::new(120., 130.),
                    inverse_scale_factor: 0.5,
                    ..default()
                },
            ))
            .id();
        let child = app.world_mut().spawn_empty().id();
        app.world_mut().entity_mut(area).add_child(child);
        app.world_mut().flush();
        for target in [area, child] {
            for (delta, expected) in [(-12., 12.), (-600., 45.), (200., 0.)] {
                app.world_mut().trigger(PointerScroll {
                    entity: target,
                    pointer: pointer(),
                    unit: bevy::input::mouse::MouseScrollUnit::Pixel,
                    x: 0.,
                    y: delta,
                    hit: HitData::new(child, 0., None, None),
                    phase: bevy::input::touch::TouchPhase::Moved,
                });
                app.world_mut().flush();
                assert_eq!(app.world().get::<ScrollPosition>(area).unwrap().y, expected);
                assert_eq!(app.world().get::<ScrollPosition>(area).unwrap().x, 0.);
            }
        }
    }

    #[test]
    fn quest_text_area_uses_source_inset_and_line_metrics() {
        let source = r#"
            Frame "TEXTAREA" "QuestDisplay" {
                FrameFont "FixtureFont", 0.023,
                TextAreaInset 0.004,
                TextAreaLineHeight 0.019,
                TextAreaLineGap 0.007,
            }
        "#;
        let read = |source: &str| text_area(&Fdf::parse(source).unwrap(), "QuestDisplay");
        let (inset, ratio) = read(source).unwrap();
        assert_eq!(inset, 0.004);
        assert!((ratio - 0.026 / 0.023).abs() < 0.000001);
        for source in [
            source.replace("0.004", "-0.004"),
            source.replace("0.019", "0"),
            source.replace("0.007", "-0.007"),
            source.replace("TextAreaInset 0.004,", ""),
            source.replace("0.007", "NaN"),
        ] {
            assert!(read(&source).is_err());
        }
    }

    #[test]
    fn quest_button_states_preserve_offsets_and_explicit_text_colours() {
        use bevy::picking::hover::Hovered;
        let mut world = World::new();
        let button = world.spawn(Hovered(false)).id();
        let layers = [
            QuestButtonState::Normal,
            QuestButtonState::Pushed,
            QuestButtonState::Disabled,
            QuestButtonState::Hover,
        ]
        .map(|state| {
            world
                .spawn((QuestButtonLayer(button, state), Visibility::Hidden))
                .id()
        });
        let wrapper = world
            .spawn((
                QuestButtonLabel {
                    owner: button,
                    offset: Vec2::new(2.3, -4.7),
                },
                Node::default(),
            ))
            .id();
        let normal = Color::srgba(0.2, 0.3, 0.4, 1.);
        let highlight = Color::srgba(0.6, 0.7, 0.8, 1.);
        let disabled = Color::srgba(0.1, 0.2, 0.3, 0.4);
        let text = world
            .spawn((
                QuestButtonText {
                    owner: button,
                    normal,
                    highlight,
                    disabled,
                },
                TextColor(normal),
            ))
            .id();
        let inherited = world.spawn((TextColor(normal), InheritedLabelColour)).id();
        let explicit = world.spawn(TextColor(Color::BLACK)).id();
        world.entity_mut(text).add_children(&[inherited, explicit]);
        for (hover, pressed, off, visible, colour) in [
            (false, false, false, [true, false, false, false], normal),
            (true, false, false, [true, false, false, true], highlight),
            (true, true, false, [false, true, false, true], highlight),
            (true, true, true, [false, false, true, false], disabled),
            (false, false, false, [true, false, false, false], normal),
        ] {
            world.entity_mut(button).insert(Hovered(hover));
            if pressed {
                world.entity_mut(button).insert(bevy::ui::Pressed);
            } else {
                world.entity_mut(button).remove::<bevy::ui::Pressed>();
            }
            if off {
                world.entity_mut(button).insert(InteractionDisabled);
            } else {
                world.entity_mut(button).remove::<InteractionDisabled>();
            }
            world.run_system_once(show_quest_button).unwrap();
            world.run_system_once(show_quest_button_text).unwrap();
            for (entity, shown) in layers.into_iter().zip(visible) {
                assert_eq!(
                    *world.get::<Visibility>(entity).unwrap(),
                    if shown {
                        Visibility::Inherited
                    } else {
                        Visibility::Hidden
                    }
                );
            }
            assert_eq!(
                world.get::<Node>(wrapper).unwrap().left,
                Val::Percent(if pressed && !off { 2.3 } else { 0. })
            );
            assert_eq!(
                world.get::<Node>(wrapper).unwrap().top,
                Val::Percent(if pressed && !off { -4.7 } else { 0. })
            );
            assert_eq!(world.get::<TextColor>(text).unwrap().0, colour);
            assert_eq!(world.get::<TextColor>(inherited).unwrap().0, colour);
            assert_eq!(world.get::<TextColor>(explicit).unwrap().0, Color::BLACK);
        }
    }

    #[test]
    fn quest_backdrop_images_fill_source_rectangles() {
        let mut source = fixture_source(Fdf::default());
        let mut images = Assets::<Image>::default();
        let image = Image::new_fill(
            bevy::render::render_resource::Extent3d {
                width: 16,
                height: 2,
                depth_or_array_layers: 1,
            },
            bevy::render::render_resource::TextureDimension::D2,
            &[40, 60, 80, 255],
            bevy::render::render_resource::TextureFormat::Rgba8Unorm,
            bevy::asset::RenderAssetUsages::default(),
        );
        let handle = images.add(image);
        source.backdrop.background = Some("Synthetic".into());
        source.backdrop.edge = Some(glue_menu::Edge {
            file: "Synthetic".into(),
            corner_size: 0.03,
            pieces: EdgePiece::ALL.to_vec(),
        });
        source
            .backdrop_images
            .insert(("Synthetic".into(), false), handle);
        let mut world = World::new();
        let parent = world.spawn_empty().id();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let mut materials = Assets::<BackdropImage>::default();
        let backdrop = source.backdrop.clone();
        paint_backdrop(
            &mut Commands::new(&mut queue, &world),
            parent,
            &mut source,
            &backdrop,
            Vec2::new(0.43, 0.29),
            &images,
            &mut materials,
        )
        .unwrap();
        queue.apply(&mut world);
        let nodes = world.query::<&ImageNode>().iter(&world).collect::<Vec<_>>();
        assert_eq!(nodes.len(), 7);
        assert!(
            nodes
                .iter()
                .all(|node| node.image_mode == bevy::ui::widget::NodeImageMode::Stretch)
        );
        assert_eq!(nodes.iter().filter(|node| node.rect.is_some()).count(), 6);
        assert_eq!(source.materials.len(), 2);
    }

    #[test]
    fn quest_label_preserves_inherited_shadow_and_markup() {
        let fdf = Fdf::parse(
            r#"
            Frame "TEXT" "Style" {
                FrameFont "FixtureFont", 0.017,
                FontColor 0.2 0.4 0.6 0.8,
                FontShadowColor 0.3 0.1 0.2 0.7,
                FontShadowOffset -0.003 0.005,
            }
            Frame "TEXT" "Child" INHERITS "Style" {
            }
        "#,
        )
        .unwrap();
        let source = fixture_source(fdf);
        let mut world = World::new();
        let parent = world.spawn_empty().id();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        label(
            &mut Commands::new(&mut queue, &world),
            parent,
            "Quest |cffff0000red|r".into(),
            &source,
            "Child",
        )
        .unwrap();
        queue.apply(&mut world);
        let (shadow, offset) = world
            .query::<(&TextShadow, &glue_menu::FrameShadow)>()
            .single(&world)
            .unwrap();
        assert_eq!(shadow.color, Color::srgba(0.3, 0.1, 0.2, 0.7));
        assert_eq!(offset.0, Vec2::new(-0.003, 0.005));
        let text = world
            .query_filtered::<Entity, With<Text>>()
            .single(&world)
            .unwrap();
        let spans = world
            .get::<Children>(text)
            .unwrap()
            .iter()
            .map(|child| {
                (
                    world.get::<TextSpan>(child).unwrap().0.clone(),
                    world.get::<TextColor>(child).unwrap().0,
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            spans,
            vec![
                ("Quest ".into(), Color::srgba(0.2, 0.4, 0.6, 0.8)),
                ("red".into(), Color::srgba(1., 0., 0., 1.))
            ]
        );
    }

    #[test]
    fn hover_layer_and_authored_text_restore_on_exit_and_disable() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy::picking::hover::Hovered;
        let mut world = World::new();
        let button = world
            .spawn((
                UpperButtonArt {
                    normal: Rect::default(),
                    pushed: Rect::default(),
                    disabled: Rect::default(),
                    highlight: Rect::default(),
                    text_offset: Vec2::ZERO,
                },
                Hovered(false),
            ))
            .id();
        let layer = world
            .spawn((UpperButtonHover(button), Visibility::Hidden))
            .id();
        let normal = world
            .spawn((
                UpperButtonLabel(button),
                UpperButtonTextState::Normal,
                Visibility::Inherited,
            ))
            .id();
        let highlighted = world
            .spawn((
                UpperButtonLabel(button),
                UpperButtonTextState::Highlight,
                Visibility::Hidden,
            ))
            .id();
        let disabled_text = world
            .spawn((
                UpperButtonLabel(button),
                UpperButtonTextState::Disabled,
                Visibility::Hidden,
            ))
            .id();
        for (hover, disabled) in [(false, false), (true, false), (true, true), (false, false)] {
            world.entity_mut(button).insert(Hovered(hover));
            if disabled {
                world.entity_mut(button).insert(InteractionDisabled);
            } else {
                world.entity_mut(button).remove::<InteractionDisabled>();
            }
            world.run_system_once(show_hover).unwrap();
            let active = hover && !disabled;
            for (entity, visible) in [
                (layer, active),
                (highlighted, active),
                (normal, !hover && !disabled),
                (disabled_text, disabled),
            ] {
                assert_eq!(
                    *world.get::<Visibility>(entity).unwrap(),
                    if visible {
                        Visibility::Inherited
                    } else {
                        Visibility::Hidden
                    }
                );
            }
        }
    }

    #[test]
    fn upper_button_pressed_art_and_text_follow_source_and_restore_on_release() {
        let source = r#"
            Texture "UpperMenuButtonBackground" {
                File "SyntheticAtlas",
                TexCoord 0.04, 0.60, 0.13, 0.30,
            }
            Texture "Down" {
                File "SyntheticAtlas",
                TexCoord 0.14, 0.70, 0.45, 0.72,
            }
            Texture "Disabled" {
                File "SyntheticAtlas",
                TexCoord 0.3, 0.9, 0.02, 0.19,
            }
            Texture "Glow" {
                File "SyntheticAtlas",
                TexCoord 0.2, 0.8, 0.6, 0.9,
                AlphaMode "ADD",
            }
            Frame "SIMPLEBUTTON" "Button" {
                NormalTexture "UpperMenuButtonBackground",
                PushedTexture "Down",
                DisabledTexture "Disabled",
                UseHighlight "Glow",
                ButtonPushedTextOffset -0.002 0.004,
            }
        "#;
        let fdf = Fdf::parse(&normalize(source)).unwrap();
        let art =
            upper_button_art(&fdf, "Button", Vec2::splat(100.), Vec2::new(0.11, 0.031)).unwrap();
        assert!((art.normal.min - Vec2::new(4., 13.)).length() < 0.00001);
        assert!((art.normal.max - Vec2::new(60., 30.)).length() < 0.00001);
        assert_eq!(art.pushed, Rect::new(14., 45., 70., 72.));
        assert!((art.disabled.min - Vec2::new(30., 2.)).length() < 0.00001);
        assert_eq!(art.disabled.max, Vec2::new(90., 19.));
        assert!((art.highlight.min - Vec2::new(20., 60.)).length() < 0.00001);
        assert_eq!(art.highlight.max, Vec2::new(80., 90.));
        assert!(
            upper_button_art(
                &Fdf::parse(&normalize(&source.replace("ADD", "BLEND"))).unwrap(),
                "Button",
                Vec2::splat(100.),
                Vec2::ONE
            )
            .is_err()
        );
        assert!(
            upper_button_art(
                &fdf,
                "Button",
                Vec2::splat(100.),
                Vec2::splat(f32::from_bits(1))
            )
            .is_err()
        );
        let mut world = World::new();
        let button = world
            .spawn((art, bevy::ui::Pressed, ImageNode::default()))
            .id();
        let label = world
            .spawn((UpperButtonLabel(button), Node::default()))
            .id();
        world.run_system_once(show_button_states).unwrap();
        assert_eq!(
            world.get::<ImageNode>(button).unwrap().rect,
            Some(art.pushed)
        );
        assert_eq!(
            world.get::<Node>(label).unwrap().left,
            Val::Percent(art.text_offset.x)
        );
        assert_eq!(
            world.get::<Node>(label).unwrap().top,
            Val::Percent(art.text_offset.y)
        );
        world.entity_mut(button).insert(InteractionDisabled);
        world.run_system_once(show_button_states).unwrap();
        assert_eq!(
            world.get::<ImageNode>(button).unwrap().rect,
            Some(art.disabled)
        );
        assert_eq!(world.get::<Node>(label).unwrap().left, Val::Percent(0.));
        world
            .entity_mut(button)
            .remove::<(InteractionDisabled, bevy::ui::Pressed)>();
        world.run_system_once(show_button_states).unwrap();
        assert_eq!(
            world.get::<ImageNode>(button).unwrap().rect,
            Some(art.normal)
        );
        assert!(
            upper_button_art(
                &Fdf::parse(&normalize(&source.replace("0.70", "1.7"))).unwrap(),
                "Button",
                Vec2::splat(100.),
                Vec2::ONE
            )
            .is_err()
        );
        assert!(
            upper_button_art(
                &Fdf::parse(&normalize(
                    &source.replace("ButtonPushedTextOffset -0.002 0.004,", "")
                ))
                .unwrap(),
                "Button",
                Vec2::splat(100.),
                Vec2::ONE
            )
            .is_err()
        );
    }
    #[cfg(feature = "test-fixtures")]
    #[test]
    fn escape_in_the_game_menu_does_not_close_the_underlying_quest_log() {
        use bevy::ecs::system::RunSystemOnce;
        let mut app = App::new();
        app.init_resource::<UpperControls>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<crate::button_actions::ButtonActions>()
            .add_message::<CampaignHudAction>();
        let mut camera = CampaignCamera::active_for_test(Entity::PLACEHOLDER);
        camera.set_menu_open(true);
        app.insert_resource(camera);
        app.world_mut().resource_mut::<UpperControls>().open = true;
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.world_mut().run_system_once(activate).unwrap();
        assert!(app.world().resource::<UpperControls>().open);
        assert!(!app.world().resource::<UpperControls>().escape_consumed);
        app.world_mut()
            .resource_mut::<CampaignCamera>()
            .set_menu_open(false);
        app.world_mut().run_system_once(activate).unwrap();
        assert!(!app.world().resource::<UpperControls>().open);
    }
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn f7_debug_overlay_does_not_also_activate_the_authored_quest_shortcut() {
        let mut world = World::new();
        world.init_resource::<ButtonInput<KeyCode>>();
        world.init_resource::<crate::button_actions::ButtonActions>();
        world.init_resource::<pcx_gfx::performance::DebugOverlay>();
        let quest = world.spawn(AuthoredShortcut(KeyCode::F7)).id();
        world
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::F7);
        world.run_system_once(keyboard_shortcuts).unwrap();
        assert!(
            world
                .resource::<crate::button_actions::ButtonActions>()
                .entities
                .is_empty()
        );
        world.remove_resource::<pcx_gfx::performance::DebugOverlay>();
        world.run_system_once(keyboard_shortcuts).unwrap();
        assert_eq!(
            world
                .resource::<crate::button_actions::ButtonActions>()
                .entities,
            vec![quest]
        );
    }

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

    #[test]
    fn upper_bar_spawns_all_four_buttons_with_source_text_and_states() {
        let mut fdf_text = String::from(
            "Texture \"UpperMenuButtonBackground\" {\nFile \"Atlas\",\nTexCoord 0.0, 0.5, 0.0, 0.25,\n}\n\
             Texture \"Pushed\" {\nFile \"Atlas\",\nTexCoord 0.0, 0.5, 0.25, 0.5,\n}\n\
             Texture \"Disabled\" {\nFile \"Atlas\",\nTexCoord 0.0, 0.5, 0.5, 0.75,\n}\n\
             Texture \"Glow\" {\nFile \"Atlas\",\nTexCoord 0.0, 0.5, 0.75, 1.0,\nAlphaMode \"ADD\",\n}\n\
             Frame \"SIMPLEBUTTON\" \"Template\" {\nWidth 0.085,\nHeight 0.022,\n\
             ButtonPushedTextOffset 0.001 -0.001,\nNormalTexture \"UpperMenuButtonBackground\",\n\
             PushedTexture \"Pushed\",\nDisabledTexture \"Disabled\",\nUseHighlight \"Glow\",\n}\n\
             String \"Text\" {\nFont \"FixtureFont\",0.01,\nFontColor 1.0 1.0 1.0,\n}\n\
             Frame \"SIMPLEFRAME\" \"UpperButtonBarFrame\" {\nWidth 0.34,\nHeight 0.022,\n",
        );
        let chain = [
            ("UpperButtonBarQuestsButton", "KEY_QUESTS", "QUESTS"),
            ("UpperButtonBarMenuButton", "KEY_MENU", "MENU"),
            ("UpperButtonBarAlliesButton", "KEY_ALLIES", "ALLIES"),
            ("UpperButtonBarChatButton", "KEY_CHAT", "CHAT"),
        ];
        for (index, (name, normal, disabled)) in chain.iter().enumerate() {
            let anchor = match index {
                0 => "TOPLEFT,\"UpperButtonBarFrame\",TOPLEFT,0.002,0".to_string(),
                _ => format!("LEFT,\"{}\",RIGHT,0,0", chain[index - 1].0),
            };
            fdf_text.push_str(&format!(
                "Frame \"SIMPLEBUTTON\" \"{name}\" INHERITS \"Template\" {{\nSetPoint {anchor},\n\
                 NormalText \"Text\" \"{normal}\",\nDisabledText \"Text\" \"{disabled}\",\n\
                 HighlightText \"Text\" \"{normal}\",\n}}\n"
            ));
        }
        fdf_text.push_str("}\n");
        let mut source = fixture_source(Fdf::parse(&normalize(&fdf_text)).unwrap());
        source.globals = [
            ("KEY_QUESTS", "Quests (F9)"),
            ("KEY_MENU", "Menu (F10)"),
            ("KEY_ALLIES", "Allies (F11)"),
            ("KEY_CHAT", "Chat (Enter)"),
            ("KEY_LOG", "Log (F12)"),
            ("QUESTS", "Quests"),
            ("MENU", "Menu"),
            ("ALLIES", "Allies"),
            ("CHAT", "Chat"),
            ("LOG", "Log"),
        ]
        .map(|(key, value)| format!("{key} \"{value}\",\n"))
        .concat();
        source.image_size = Vec2::splat(64.);
        let mut world = World::new();
        let mut highlights = Assets::<AdditiveImage>::default();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        upper(
            &mut Commands::new(&mut queue, &world),
            &mut source,
            &mut highlights,
        )
        .unwrap();
        queue.apply(&mut world);
        let mut shown = HashMap::new();
        let mut buttons =
            world.query::<(Entity, &Name, Has<InteractionDisabled>, Option<&Action>)>();
        let buttons: Vec<_> = buttons
            .iter(&world)
            .map(|(entity, name, disabled, action)| {
                (
                    entity,
                    name.to_string(),
                    disabled,
                    action.map(|a| matches!(a, Action::MessageLog)),
                )
            })
            .collect();
        assert_eq!(buttons.len(), 4);
        for (button, name, _, _) in &buttons {
            for child in world.get::<Children>(*button).unwrap() {
                if world.get::<UpperButtonTextState>(*child) != Some(&UpperButtonTextState::Normal)
                {
                    continue;
                }
                let text = world.get::<Children>(*child).unwrap()[0];
                let visible: String = world
                    .get::<Children>(text)
                    .unwrap()
                    .iter()
                    .map(|span| world.get::<TextSpan>(span).unwrap().0.clone())
                    .collect();
                shown.insert(name.clone(), visible);
            }
        }
        let text = |name: &str| shown[name].as_str();
        assert_eq!(text("UpperButtonBarQuestsButton"), "Quests (F9)");
        assert_eq!(text("UpperButtonBarAlliesButton"), "Allies (F11)");
        assert_eq!(text("UpperButtonBarChatButton"), "Log (F12)");
        let state = |name: &str| buttons.iter().find(|b| b.1 == name).unwrap();
        assert!(state("UpperButtonBarAlliesButton").2);
        assert!(state("UpperButtonBarAlliesButton").3.is_none());
        assert!(!state("UpperButtonBarChatButton").2);
        assert_eq!(state("UpperButtonBarChatButton").3, Some(true));
        assert!(!state("UpperButtonBarMenuButton").2);
        let shortcut = world
            .get::<AuthoredShortcut>(state("UpperButtonBarChatButton").0)
            .map(|s| s.0);
        assert_eq!(shortcut, Some(KeyCode::F12));
        assert!(
            world
                .get::<AuthoredShortcut>(state("UpperButtonBarAlliesButton").0)
                .is_none()
        );
    }

    /// A session provider tag other than `LOOP` and `NONE`.
    const NETWORK_PROVIDER: u32 = u32::from_be_bytes(*b"UDPN");

    #[test]
    fn quest_row_status_follows_the_client_string_selection() {
        assert_eq!(row_status_key(true, false), Some("QUESTCOMPLETED"));
        assert_eq!(row_status_key(false, true), Some("QUESTFAILED"));
        assert_eq!(row_status_key(true, true), Some("QUESTCOMPLETED"));
        assert_eq!(row_status_key(false, false), None);
    }

    #[test]
    fn allies_and_log_follow_the_session_provider_tag() {
        for provider in [PROVIDER_LOOPBACK, PROVIDER_NONE] {
            assert!(UpperButton::Allies.disabled(provider));
            assert!(matches!(
                UpperButton::Log.action(provider),
                Some(Action::MessageLog)
            ));
            assert_eq!(UpperButton::Log.text_key("KEY_CHAT", provider), "KEY_LOG");
            assert_eq!(UpperButton::Log.text_key("CHAT", provider), "LOG");
        }
        assert!(!UpperButton::Allies.disabled(NETWORK_PROVIDER));
        assert!(UpperButton::Log.action(NETWORK_PROVIDER).is_none());
        assert_eq!(UpperButton::Log.text_key("CHAT", NETWORK_PROVIDER), "CHAT");
        for provider in [PROVIDER_LOOPBACK, NETWORK_PROVIDER] {
            assert_eq!(UpperButton::Quests.text_key("CHAT", provider), "CHAT");
            assert!(!UpperButton::Quests.disabled(provider));
            assert!(!UpperButton::Menu.disabled(provider));
        }
        assert!(!networked(CAMPAIGN_PROVIDER));
    }

    fn press(world: &mut World, entity: Entity) {
        world
            .resource_mut::<crate::button_actions::ButtonActions>()
            .entities = vec![entity];
        world.run_system_once(activate).unwrap();
    }
    #[test]
    fn message_log_and_quest_log_never_open_together_and_escape_closes_either() {
        let mut app = App::new();
        app.init_resource::<UpperControls>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<crate::button_actions::ButtonActions>()
            .add_message::<CampaignHudAction>();
        let log = app.world_mut().spawn(Action::MessageLog).id();
        let ok = app.world_mut().spawn(Action::CloseMessageLog).id();
        let quests = app.world_mut().spawn(Action::Quests).id();
        let flags = |app: &App| {
            let state = app.world().resource::<UpperControls>();
            (state.open, state.messages_open)
        };
        press(app.world_mut(), log);
        assert_eq!(flags(&app), (false, true));
        assert!(app.world().resource::<UpperControls>().pointer_consumed);
        press(app.world_mut(), quests);
        assert_eq!(flags(&app), (true, false));
        press(app.world_mut(), log);
        assert_eq!(flags(&app), (false, true));
        press(app.world_mut(), log);
        assert_eq!(flags(&app), (false, false), "the Log button toggles");
        press(app.world_mut(), log);
        press(app.world_mut(), ok);
        assert_eq!(flags(&app), (false, false));
        press(app.world_mut(), log);
        app.world_mut()
            .resource_mut::<crate::button_actions::ButtonActions>()
            .entities
            .clear();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.world_mut().run_system_once(activate).unwrap();
        assert_eq!(flags(&app), (false, false));
        assert!(app.world().resource::<UpperControls>().escape_consumed);
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
    art: &glue_menu::Backdrop,
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
    if let Some(token) = &art.background {
        let inset = art.inset.unwrap_or(0.);
        let rect = crate::fdf::Rect {
            left: inset,
            bottom: inset,
            width: size.x - 2. * inset,
            height: size.y - 2. * inset,
        };
        let handle = source.backdrop_images[&(token.clone(), art.tile)].clone();
        let entity = commands
            .spawn((node(rect), Pickable::IGNORE, ZIndex(-1)))
            .id();
        if art.tile {
            let tile = art
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
            commands
                .entity(entity)
                .insert(ImageNode::new(handle).with_mode(bevy::ui::widget::NodeImageMode::Stretch));
        }
        commands.entity(parent).add_child(entity);
    }
    if let Some(edge) = &art.edge {
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
                let mut image = ImageNode::new(handle.clone())
                    .with_mode(bevy::ui::widget::NodeImageMode::Stretch);
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
    // Names every modal dialog the bar opens, so gameplay input stays blocked.
    hud.quest_log_open = state.open || state.messages_open;
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
    #[ignore = "requires PCX_HUD_FIXTURE_DIR with locally extracted original UpperButtonBar.fdf, QuestDialog.fdf, LogDialog.fdf, EscMenuTemplates.fdf and GlobalStrings.fdf"]
    fn original_upper_controls_quest_and_log_frames_parse() {
        let root = std::path::PathBuf::from(
            std::env::var_os("PCX_HUD_FIXTURE_DIR").expect("PCX_HUD_FIXTURE_DIR required"),
        );
        let mut fdf = Fdf::parse(&normalize(
            &std::fs::read_to_string(root.join("UpperButtonBar.fdf")).unwrap(),
        ))
        .unwrap();
        for name in ["EscMenuTemplates.fdf", "QuestDialog.fdf", "LogDialog.fdf"] {
            fdf.parse_into(&std::fs::read_to_string(root.join(name)).unwrap())
                .unwrap();
        }
        for name in [
            "UpperButtonBarButtonTextTemplate",
            "QuestMainTitle",
            "QuestDetailsTitle",
            "QuestListItemTitle",
            "QuestListItemComplete",
            "QuestDisplay",
        ] {
            menu_text::render_style(&fdf, name).unwrap();
        }
        assert!(
            crate::glue_menu::backdrop(&fdf, "QuestBackdrop")
                .unwrap()
                .is_some()
        );
        let globals = std::fs::read_to_string(root.join("GlobalStrings.fdf")).unwrap();
        let art = message_log::Art::load(&mut fdf, &globals).unwrap();
        assert_eq!(art.key, KeyCode::F12);
        assert_eq!(art.max_entries, 128);
        for key in ["QUESTCOMPLETED", "QUESTFAILED", "MESSAGE_LOG", "OK"] {
            assert!(menu_text::global_string(&globals, key).is_some(), "{key}");
        }
    }
}

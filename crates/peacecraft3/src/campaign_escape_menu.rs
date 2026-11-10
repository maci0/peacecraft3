//! Source-authored escape panels over the retained campaign view.
use crate::{
    campaign_hud::upper_buttons::{read, skin_file, text},
    fdf::{Fdf, Rect},
    glue_menu::{self, Kind},
    menu_skin::SkinTable,
};
use bevy::{
    ecs::system::SystemParam,
    input::keyboard::{Key, KeyboardInput},
    picking::events::PointerClick,
    prelude::*,
    ui::InteractionDisabled,
    ui_widgets::Activate,
};
use pcx_play::map_startup_host::MapStartupHost;
use peacecraft3::{
    browser_configuration::SelectedMapSources,
    campaign_camera::{CampaignCamera, CampaignCameraSet},
    campaign_presentation::{CampaignHud, CampaignPresentationSet},
    campaign_runtime::CampaignTransitionRequest,
    map_binding::AssetReceipt,
    map_requests::MapRequests,
    map_script_session::MapScriptSession,
};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Weak},
};
const PANEL: &str = r"UI\FrameDef\UI\EscMenuMainPanel.fdf";
const TEMPLATES: &str = r"UI\FrameDef\UI\EscMenuTemplates.fdf";
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Page {
    #[default]
    Main,
    End,
    ConfirmExit,
    Help,
    Tips,
}
impl Page {
    fn frame(self) -> &'static str {
        match self {
            Self::Main => "MainPanel",
            Self::End => "EndGamePanel",
            Self::ConfirmExit => "ConfirmQuitPanel",
            Self::Help => "HelpPanel",
            Self::Tips => "TipsPanel",
        }
    }
    fn back(self) -> Option<Self> {
        match self {
            Self::Main => None,
            Self::End => Some(Self::Main),
            Self::ConfirmExit => Some(Self::End),
            Self::Help => Some(Self::Main),
            Self::Tips => Some(Self::Main),
        }
    }
}
#[derive(Resource, Default)]
pub(crate) struct EscapeMenu {
    witness: Option<Weak<()>>,
    page: Page,
    painted: Option<Page>,
    root: Option<Entity>,
    source: Option<Source>,
    attempted: bool,
    pending: Option<(String, Page, Weak<()>)>,
    pub last_error: Option<String>,
    tips: pcx_play::escape_help_tips::TipSelection,
}
impl EscapeMenu {
    pub(crate) fn receipt_count(&self) -> usize {
        self.source
            .as_ref()
            .map_or(0, |source| source.receipts.len())
    }
}
struct Source {
    layout: pcx_play::escape_layout::Rules,
    fdf: Fdf,
    skin: SkinTable,
    sections: Vec<String>,
    strings: String,
    fonts: HashMap<String, Handle<Font>>,
    reader: Arc<pcx_assets::Resolver>,
    receipts: Vec<AssetReceipt>,
    help: pcx_play::escape_help_tips::HelpStrings,
    help_style: crate::menu_text::TextStyle,
    help_rect: Rect,
    help_inset: f32,
    tips: pcx_play::escape_help_tips::TipStrings,
    tips_style: crate::menu_text::TextStyle,
    tips_rect: Rect,
    tips_inset: f32,
}
#[derive(Component)]
pub(crate) struct EscapeRoot;
#[derive(Component)]
struct EscapeButton {
    frame: String,
    key: Option<char>,
    page: Page,
    witness: Weak<()>,
}
#[derive(Component)]
pub(crate) struct EscapeLabel;
#[derive(Component)]
struct EscapeText(String);
#[derive(Component)]
struct EscapePosition(String);
#[derive(SystemParam)]
struct Inputs<'w> {
    requests: Res<'w, MapRequests>,
    sources: Res<'w, SelectedMapSources>,
    session: ResMut<'w, MapScriptSession<MapStartupHost>>,
    view: ResMut<'w, CampaignCamera>,
    route: Option<ResMut<'w, crate::campaign_in_game_options::InGameOptionsRoute>>,
    nav: Option<ResMut<'w, crate::menu_screens::Navigation>>,
}
pub(crate) fn install(app: &mut App) {
    app.init_resource::<EscapeMenu>()
        .add_observer(pressed)
        .add_observer(clicked)
        .add_systems(
            Update,
            (keyboard, act, project, fill_labels)
                .chain()
                .after(CampaignCameraSet)
                .before(CampaignPresentationSet)
                .before(crate::button_actions::clear),
        )
        .add_systems(PostUpdate, measure.after(bevy::ui::UiSystems::PostLayout));
}
fn queue(state: &mut EscapeMenu, button: &EscapeButton, view: &CampaignCamera) {
    if view.menu_open()
        && state.page == button.page
        && state
            .witness
            .as_ref()
            .is_some_and(|w| Weak::ptr_eq(w, &button.witness))
        && state.pending.is_none()
    {
        state.pending = Some((button.frame.clone(), button.page, button.witness.clone()));
    }
}
fn pressed(
    event: On<Activate>,
    buttons: Query<&EscapeButton, Without<InteractionDisabled>>,
    mut state: ResMut<EscapeMenu>,
    view: Res<CampaignCamera>,
) {
    if let Ok(button) = buttons.get(event.entity) {
        queue(&mut state, button, &view);
    }
}
fn clicked(
    event: On<PointerClick>,
    buttons: Query<&EscapeButton, Without<InteractionDisabled>>,
    mut state: ResMut<EscapeMenu>,
    view: Res<CampaignCamera>,
) {
    if event.button == bevy::picking::pointer::PointerButton::Primary
        && let Ok(button) = buttons.get(event.entity)
    {
        queue(&mut state, button, &view);
    }
}
fn keyboard(
    mut events: MessageReader<KeyboardInput>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    buttons: Query<&EscapeButton, Without<InteractionDisabled>>,
    mut state: ResMut<EscapeMenu>,
    mut view: ResMut<CampaignCamera>,
) {
    if !view.menu_open() {
        events.clear();
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        keys.clear_just_pressed(KeyCode::Escape);
        if let Some(page) = state.page.back() {
            state.page = page;
        } else {
            view.set_menu_open(false);
        }
        events.clear();
        return;
    }
    for event in events.read().filter(|e| e.state.is_pressed() && !e.repeat) {
        let Key::Character(value) = &event.logical_key else {
            continue;
        };
        for button in &buttons {
            if button.key.is_some_and(|key| {
                value.chars().eq(key.to_lowercase()) || value.chars().eq(key.to_uppercase())
            }) {
                queue(&mut state, button, &view);
            }
        }
    }
}
fn enabled(frame: &str) -> bool {
    // G01-owned escape controls known to dispatch in `act`. `OptionsButton`
    // is included by the campaign_in_game_options entry slice; the
    // `act` arm for it is the actual dispatch.
    matches!(
        frame,
        "ReturnButton"
            | "OptionsButton"
            | "EndGameButton"
            | "PreviousButton"
            | "QuitButton"
            | "RestartButton"
            | "ExitButton"
            | "ConfirmQuitQuitButton"
            | "ConfirmQuitCancelButton"
            | "HelpButton"
            | "HelpOKButton"
            | "TipsButton"
            | "TipsBackButton"
            | "TipsNextButton"
            | "TipsOKButton"
    )
}
fn act(
    mut commands: Commands,
    mut state: ResMut<EscapeMenu>,
    mut inputs: Inputs,
    mut transitions: MessageWriter<CampaignTransitionRequest>,
    mut exits: MessageWriter<AppExit>,
    menus: Query<Entity, With<crate::launch_gui::MenuLayer>>,
) {
    let Some((frame, page, witness)) = state.pending.take() else {
        return;
    };
    if !inputs.view.menu_open()
        || page != state.page
        || !inputs
            .session
            .session_witness()
            .is_some_and(|w| Weak::ptr_eq(&w, &witness))
    {
        return;
    }
    match frame.as_str() {
        "ReturnButton" => inputs.view.set_menu_open(false),
        "OptionsButton" => {
            // Admitted-session witness; keep CampaignCamera menu open (pause)
            // while Options route is active. The escape root stays mounted so
            // Back/Escape-close can unwind; this slice does not add OK/Cancel.
            let Some(witness) = inputs.session.session_witness() else {
                return;
            };
            let (Some(route), Some(nav)) = (inputs.route.as_deref_mut(), inputs.nav.as_deref_mut())
            else {
                return;
            };
            crate::campaign_in_game_options::route_open_escape_options(route, nav, &witness);
        }
        "EndGameButton" => state.page = Page::End,
        "PreviousButton" => state.page = Page::Main,
        "ExitButton" => state.page = Page::ConfirmExit,
        "ConfirmQuitCancelButton" => state.page = Page::End,
        "HelpButton" => state.page = Page::Help,
        "HelpOKButton" => state.page = Page::Main,
        "TipsButton" => state.page = Page::Tips,
        "TipsBackButton" => {
            let tips = state.source.as_ref().map(|s| s.tips.clone());
            if let Some(tips) = tips {
                state.tips.previous(&tips);
            }
            state.painted = None;
        }
        "TipsNextButton" => {
            let tips = state.source.as_ref().map(|s| s.tips.clone());
            if let Some(tips) = tips {
                state.tips.next(&tips);
            }
            state.painted = None;
        }
        "TipsOKButton" => state.page = Page::Main,
        "ConfirmQuitQuitButton" => {
            if cfg!(target_arch = "wasm32") {
                inputs.session.clear();
                inputs.view.suspend();
                for menu in &menus {
                    commands.entity(menu).try_despawn();
                }
            } else {
                exits.write(AppExit::Success);
            }
        }
        "RestartButton" => {
            if let (Some(context), Some(bindings)) = (
                inputs.session.context().cloned(),
                inputs.requests.bindings.as_ref(),
            ) {
                transitions.write(CampaignTransitionRequest {
                    next_level: bindings.receipts.map.path.to_string(),
                    context,
                });
            }
        }
        "QuitButton" => {
            use pcx_script::{Host, Value};
            if let Some(host) = inputs.session.host()
                && let Err(error) = host.native("EndGame", &[Value::Bool(false)])
            {
                state.last_error = Some(error);
            }
        }
        _ => {}
    }
}
fn resolve_art(
    art: &mut glue_menu::Backdrop,
    skin: &SkinTable,
    sections: &[String],
) -> Result<(), String> {
    for path in art
        .background
        .iter_mut()
        .chain(art.edge.iter_mut().map(|e| &mut e.file))
    {
        if !path.contains('\\') && !path.contains('/') {
            *path = skin_file(skin, sections, path)?;
        }
    }
    Ok(())
}
fn label_offset(fdf: &Fdf, name: &str) -> Result<Option<[f32; 2]>, String> {
    fdf.numbers(name, "FontJustificationOffset")
        .map_err(|e| e.to_string())?
        .map(|value| {
            let [x, y] = value.as_slice() else {
                return Err(format!("{name}: invalid justification offset"));
            };
            if !x.is_finite() || !y.is_finite() {
                return Err(format!("{name}: nonfinite justification offset"));
            }
            Ok([*x, *y])
        })
        .transpose()
}
fn items(
    fdf: &Fdf,
    skin: &SkinTable,
    sections: &[String],
    page: Page,
    layout: &pcx_play::escape_layout::Rules,
) -> Result<Vec<glue_menu::Item>, String> {
    let view = layout.viewport;
    let size = (
        fdf.width(page.frame()).map_err(|e| e.to_string())?,
        fdf.height(page.frame()).map_err(|e| e.to_string())?,
    );
    if ![size.0, size.1].iter().all(|n| n.is_finite() && *n > 0.) {
        return Err("escape menu: invalid panel extent".into());
    }
    let (mut items, unsupported) =
        glue_menu::control_items(fdf, page.frame(), size).map_err(|e| e.to_string())?;
    // TEXTAREA children are rendered as labels in project(); the
    // control whitelist only admits the Help/Tips panel chrome here.
    let unsupported: Vec<_> = unsupported
        .into_iter()
        .filter(|name| name != "TEXTAREA HelpTextArea" && name != "TEXTAREA TipsTextArea")
        .collect();
    if !unsupported.is_empty() {
        return Err(format!("escape menu: unsupported controls {unsupported:?}"));
    }
    // The observed single-player menu hides Pause; its frame still anchors later controls.
    items.retain(|item| item.frame != "PauseButton");
    let backdrop = glue_menu::backdrop(fdf, &layout.backdrop_frame)
        .map_err(|e| e.to_string())?
        .ok_or("escape menu: missing backdrop")?;
    items.insert(
        0,
        glue_menu::Item {
            frame: layout.backdrop_frame.clone(),
            rect: Rect {
                left: 0.,
                bottom: 0.,
                width: size.0,
                height: size.1,
            },
            kind: Kind::Backdrop(backdrop),
        },
    );
    let origin = layout.position([size.0, size.1])?;
    for item in &mut items {
        item.rect.left += origin[0];
        item.rect.bottom += origin[1];
        if ![
            item.rect.left / view[0],
            item.rect.bottom / view[1],
            item.rect.width / view[0],
            item.rect.height / view[1],
        ]
        .iter()
        .all(|value| (value * 100.).is_finite())
            || item.rect.width < 0.
            || item.rect.height < 0.
        {
            return Err(format!("{}: invalid escape-menu rectangle", item.frame));
        }
        match &mut item.kind {
            Kind::Backdrop(art) => resolve_art(art, skin, sections)?,
            Kind::Button(button) => {
                for art in [&mut button.normal, &mut button.pushed, &mut button.disabled]
                    .into_iter()
                    .flatten()
                {
                    resolve_art(art, skin, sections)?;
                }
                if let Some(art) = &mut button.mouse_over {
                    art.file = skin_file(skin, sections, &art.file)?;
                }
            }
            _ => {}
        }
    }
    Ok(items)
}
fn read_layout(
    install: Option<&std::path::Path>,
) -> Result<pcx_play::escape_layout::Rules, String> {
    crate::menu_lighting::with_client_image(install, |source, image| {
        pcx_play::escape_layout::Rules::read(
            image,
            pcx_play::scripts::AssetIdentity {
                source: source.into(),
                edition: "selected menu client".into(),
                version: "reviewed Legacy 1.29.2.9231 escape layout".into(),
            },
        )
    })
}
fn load(
    reader: Arc<pcx_assets::Resolver>,
    race: Option<String>,
    fonts: &mut Assets<Font>,
    layout: pcx_play::escape_layout::Rules,
    install: Option<&std::path::Path>,
) -> Result<Source, String> {
    let mut receipts = Vec::new();
    let mut fdf =
        Fdf::parse(&text(&reader, TEMPLATES, &mut receipts)?).map_err(|e| e.to_string())?;
    fdf.parse_into(&text(&reader, PANEL, &mut receipts)?)
        .map_err(|e| e.to_string())?;
    crate::menu_screens::prime_text_sizes(&mut fdf);
    let skin = SkinTable::parse(&text(&reader, r"UI\war3skins.txt", &mut receipts)?)
        .map_err(|e| e.to_string())?;
    let sections = crate::campaign_hud::race_sections(&skin, race)?;
    let strings = text(&reader, r"UI\FrameDef\GlobalStrings.fdf", &mut receipts)?;
    // Help body + qualified default font (bounded; no scrollbar first slice).
    let help_text = text(&reader, r"UI\HelpStrings.txt", &mut receipts)?;
    let help = pcx_play::escape_help_tips::HelpStrings::parse(&help_text)?;
    let tips_text = text(&reader, r"UI\TipStrings.txt", &mut receipts)?;
    let tips = pcx_play::escape_help_tips::TipStrings::parse(&tips_text)?;
    let misc = text(&reader, r"UI\MiscUI.txt", &mut receipts)?;
    let default_font = crate::menu_lighting::with_client_image(install, |src, image| {
        pcx_play::escape_help_tips::DefaultFont::read(
            image,
            pcx_play::scripts::AssetIdentity {
                source: src.into(),
                edition: "selected menu client".into(),
                version: "reviewed Legacy 1.29.2.9231 escape default font".into(),
            },
            &misc,
        )
    })?;
    let help_rect = fdf
        .rect("HelpTextArea", (layout.viewport[0], layout.viewport[1]))
        .map_err(|e| e.to_string())?;
    let inset_numbers = fdf
        .numbers("EscMenuTextAreaTemplate", "TextAreaInset")
        .map_err(|e| e.to_string())?
        .ok_or("EscMenuTextAreaTemplate: missing TextAreaInset")?;
    let [inset] = inset_numbers.as_slice() else {
        return Err("EscMenuTextAreaTemplate: invalid TextAreaInset".into());
    };
    if !inset.is_finite() || *inset < 0. {
        return Err("EscMenuTextAreaTemplate: invalid TextAreaInset".into());
    }
    let help_style = crate::menu_text::render_style_with_font(
        &fdf,
        "HelpTextArea",
        "HelpPanel",
        Some((
            default_font.font_path.clone(),
            default_font.wrapping_height(),
        )),
    )?;
    let tips_rect = fdf
        .rect("TipsTextArea", (layout.viewport[0], layout.viewport[1]))
        .map_err(|e| e.to_string())?;
    let tips_style = crate::menu_text::render_style_with_font(
        &fdf,
        "TipsTextArea",
        "TipsPanel",
        Some((
            default_font.font_path.clone(),
            default_font.wrapping_height(),
        )),
    )?;
    // Ensure decoded default-font bytes resolve through the same reader.
    {
        let asset = reader
            .resolve_bounded(&default_font.font_path, 16 << 20)
            .map_err(|e| format!("{}: {e}", default_font.font_path))?;
        if !crate::hud_render::font_registers(&asset.bytes) {
            return Err(format!("{}: unreadable font", default_font.font_path));
        }
    }
    if fdf.frame(&layout.root_frame).is_none() {
        return Err(format!(
            "escape menu: missing native root {}",
            layout.root_frame
        ));
    }
    let mut source = Source {
        layout,
        fdf,
        skin,
        sections,
        strings,
        fonts: HashMap::new(),
        reader,
        receipts,
        help,
        help_style,
        help_rect,
        help_inset: *inset,
        tips,
        tips_style,
        tips_rect,
        tips_inset: *inset,
    };
    let mut decoded = HashMap::new();
    let mut seen = HashSet::new();
    let mut total = 0usize;
    let mut font_bytes = 0usize;
    for page in [
        Page::Main,
        Page::End,
        Page::ConfirmExit,
        Page::Help,
        Page::Tips,
    ] {
        for item in items(
            &source.fdf,
            &source.skin,
            &source.sections,
            page,
            &source.layout,
        )? {
            let label = match &item.kind {
                Kind::Button(b) => b.text.as_deref(),
                Kind::Text => Some(item.frame.as_str()),
                _ => None,
            };
            if let Some(label) = label {
                label_offset(&source.fdf, label)?;
                let style = crate::menu_text::render_style(&source.fdf, label)?;
                let key = source
                    .fdf
                    .text_value(label)
                    .ok_or_else(|| format!("{label}: missing Text"))?;
                let value = crate::menu_text::global_string(&source.strings, &key)
                    .ok_or_else(|| format!("{label}: missing string {key}"))?;
                let parsed = crate::rich_text::parse(&value).map_err(|e| e.to_string())?;
                if !parsed.unsupported.is_empty() {
                    return Err(format!("{label}: unsupported text substitutions"));
                }
                if let std::collections::hash_map::Entry::Vacant(entry) = decoded.entry(style.font)
                {
                    let path = skin_file(&source.skin, &source.sections, entry.key())?;
                    let bytes = read(&source.reader, &path, 16 << 20, &mut source.receipts)?;
                    font_bytes = font_bytes
                        .checked_add(bytes.len())
                        .filter(|n| *n <= 32 << 20)
                        .ok_or("escape font aggregate budget")?;
                    if !crate::hud_render::font_registers(&bytes) {
                        return Err(format!("{path}: unreadable font"));
                    }
                    entry.insert(Font::from_bytes(bytes));
                }
            }
            let mut paths = Vec::new();
            let mut add = |art: &glue_menu::Backdrop| {
                paths.extend(art.background.iter().cloned());
                paths.extend(art.edge.iter().map(|e| e.file.clone()));
            };
            match &item.kind {
                Kind::Backdrop(a) => add(a),
                Kind::Button(b) => {
                    for a in [&b.normal, &b.pushed, &b.disabled].into_iter().flatten() {
                        add(a);
                    }
                    if let Some(a) = &b.mouse_over {
                        paths.push(a.file.clone());
                    }
                }
                _ => {}
            }
            for path in paths.into_iter().filter(|p| seen.insert(p.clone())) {
                let bytes = read(&source.reader, &path, 4 << 20, &mut source.receipts)?;
                let image = pcx_gfx::load_image_bounded(&bytes, &path, 16 << 20)
                    .map_err(|e| e.to_string())?;
                total = total
                    .checked_add(image.rgba_bytes().ok_or("escape texture size")?)
                    .filter(|n| *n <= 64 << 20)
                    .ok_or("escape texture budget")?;
            }
        }
    }
    source.fonts = decoded
        .into_iter()
        .map(|(name, font)| (name, fonts.add(font)))
        .collect();
    Ok(source)
}
fn project(
    mut commands: Commands,
    mut state: ResMut<EscapeMenu>,
    mut inputs: Inputs,
    mut fonts: ResMut<Assets<Font>>,
    mut images: ResMut<glue_menu::GlueImages>,
    launcher: Option<Res<crate::launch_gui::Launcher>>,
) {
    let witness = inputs.session.session_witness();
    let same = match (&witness, &state.witness) {
        (Some(a), Some(b)) => Weak::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    };
    if !same {
        if let Some(root) = state.root.take() {
            commands.entity(root).try_despawn();
        }
        if let Some(source) = state.source.take() {
            for font in source.fonts.into_values() {
                fonts.remove(font.id());
            }
            images.forget_source(&source.reader);
        }
        *state = EscapeMenu {
            witness,
            ..default()
        };
    }
    if !state.attempted {
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
                    .zip(inputs.session.context())
                    .is_some_and(|(admitted, active)| Arc::ptr_eq(admitted, active))
        }) else {
            return;
        };
        let Some(reader) = inputs.sources.for_receipts(&bindings.receipts) else {
            return;
        };
        let Some(host) = inputs.session.host() else {
            return;
        };
        state.attempted = true;
        match host
            .objects()
            .local_race_preference_constant()
            .map_err(|e| e.to_string())
            .and_then(|race| {
                let install = launcher
                    .as_deref()
                    .and_then(crate::launch_gui::selected_install);
                let layout = read_layout(install.as_deref())?;
                load(reader, race, &mut fonts, layout, install.as_deref())
            }) {
            Ok(source) => state.source = Some(source),
            Err(error) => {
                state.last_error = Some(error);
                return;
            }
        }
    }
    if !inputs.view.menu_open() {
        if let Some(root) = state.root.take() {
            commands.entity(root).try_despawn();
        }
        state.page = Page::Main;
        state.painted = None;
        state.pending = None;
        return;
    }
    if state.painted == Some(state.page) {
        return;
    }
    let Some(source) = state.source.as_ref() else {
        return;
    };
    let items = match items(
        &source.fdf,
        &source.skin,
        &source.sections,
        state.page,
        &source.layout,
    ) {
        Ok(items) => items,
        Err(error) => {
            state.last_error = Some(error);
            return;
        }
    };
    let names = items
        .iter()
        .filter(|item| matches!(item.kind, Kind::Button(_)))
        .map(|item| item.frame.clone())
        .collect::<Vec<_>>();
    let order = crate::menu_screens::focus_order(&source.fdf, &names);
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                ..default()
            },
            EscapeRoot,
            bevy::input_focus::tab_navigation::TabGroup::modal(),
            CampaignHud,
            GlobalZIndex(i32::MAX),
            glue_menu::ArtSource(source.reader.clone()),
        ))
        .id();
    glue_menu::spawn_in(&mut commands, root, &items, source.layout.viewport);
    let text_area = match state.page {
        Page::Help => Some((
            "HelpPanel",
            "HelpTextArea",
            source.help_rect,
            source.help_inset,
            &source.help_style,
            source.help.body.as_str(),
        )),
        Page::Tips => Some((
            "TipsPanel",
            "TipsTextArea",
            source.tips_rect,
            source.tips_inset,
            &source.tips_style,
            state.tips.current(&source.tips).unwrap_or_default(),
        )),
        _ => None,
    };
    let mut font_error = None;
    if let Some((panel_frame, area, mut rect, inset, style, body)) = text_area {
        let view = source.layout.viewport;
        let panel = (
            source.fdf.width(panel_frame).unwrap_or(0.),
            source.fdf.height(panel_frame).unwrap_or(0.),
        );
        let origin = source
            .layout
            .position([panel.0, panel.1])
            .unwrap_or([0., 0.]);
        rect.left += origin[0];
        rect.bottom += origin[1];
        let container = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: percent(rect.left / view[0] * 100.),
                    top: percent((view[1] - rect.top()) / view[1] * 100.),
                    width: percent(rect.width / view[0] * 100.),
                    height: percent(rect.height / view[1] * 100.),
                    padding: UiRect::all(Val::Vh(inset / view[1] * 100.)),
                    overflow: Overflow::clip(),
                    ..default()
                },
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(root).add_child(container);
        if let Some(font) = source.fonts.get(&style.font).cloned() {
            let _ =
                crate::campaign_script_dialogs::label(&mut commands, container, body, style, &font);
        } else {
            font_error = Some(format!("{area}: missing decoded font {}", style.font));
        }
    }
    if font_error.is_some() {
        state.last_error = font_error;
    }
    if let Some(old) = state.root.replace(root) {
        commands.entity(old).try_despawn();
    }
    let page = state.page;
    let witness = state.witness.clone().unwrap();
    commands.queue(move |world: &mut World| {
        let mut pending = vec![root];
        let mut first = None;
        while let Some(entity) = pending.pop() {
            if let Some(children) = world.get::<Children>(entity) {
                pending.extend(children.iter());
            }
            let button = world
                .get::<glue_menu::GlueButton>(entity)
                .map(|b| (b.frame.clone(), b.shortcut));
            let label = world.get::<glue_menu::GlueLabel>(entity).is_some();
            let position = world
                .get::<crate::menu_screens::ScreenPosition>(entity)
                .map(|p| p.0.clone());
            let mut node = world.entity_mut(entity);
            node.remove::<crate::menu_screens::ScreenPosition>();
            if let Some(frame) = position {
                node.insert(EscapePosition(frame));
            }
            if let Some((frame, key)) = button {
                let available = enabled(&frame);
                let index = order.iter().position(|name| name == &frame);
                node.insert((
                    crate::menu_screens::CampaignAction,
                    EscapeButton {
                        frame,
                        key,
                        page,
                        witness: witness.clone(),
                    },
                ));
                if !available {
                    node.insert(InteractionDisabled);
                } else if let Some(index) = index {
                    node.insert(bevy::input_focus::tab_navigation::TabIndex(
                        i32::try_from(index).expect("bounded FDF control count"),
                    ));
                    if first.is_none_or(|(previous, _)| index < previous) {
                        first = Some((index, entity));
                    }
                }
            }
            if label {
                node.insert(EscapeLabel);
            }
        }
        if let Some((_, entity)) = first
            && let Some(mut focus) = world.get_resource_mut::<bevy::input_focus::InputFocus>()
        {
            focus.set(entity, bevy::input_focus::FocusCause::Auto);
        }
    });
    state.painted = Some(page);
    state.last_error = None;
}
type PendingLabels = (With<EscapeLabel>, Without<EscapeText>);

fn fill_labels(
    mut commands: Commands,
    state: Res<EscapeMenu>,
    disabled: Query<(), With<InteractionDisabled>>,
    labels: Query<(Entity, &glue_menu::GlueLabel, &ChildOf), PendingLabels>,
) {
    let Some(source) = &state.source else { return };
    let view = source.layout.viewport;
    for (entity, label, parent) in &labels {
        let result = (|| {
            let mut style = crate::menu_text::render_style(&source.fdf, &label.frame)?;
            if disabled.contains(label.button) {
                style.colour = style.disabled;
            }
            let key = source
                .fdf
                .text_value(&label.frame)
                .ok_or("escape label Text")?;
            let mut value = crate::menu_text::global_string(&source.strings, &key)
                .ok_or("escape label string")?;
            if disabled.contains(label.button) {
                value = crate::menu_text::Label::parse(&value)
                    .map_err(|e| e.to_string())?
                    .visible();
            }
            let replacement = crate::campaign_script_dialogs::label(
                &mut commands,
                parent.parent(),
                &value,
                &style,
                &source.fonts[&style.font],
            )?;
            commands.entity(replacement).insert((
                EscapeText(label.frame.clone()),
                EscapeLabel,
                glue_menu::GlueLabel {
                    frame: label.frame.clone(),
                    button: label.button,
                },
            ));
            if source.fdf.width(&label.frame).is_ok_and(|w| w == 0.) {
                commands.entity(replacement).insert((
                    Node {
                        width: Val::Auto,
                        flex_shrink: 0.,
                        ..default()
                    },
                    TextLayout::new(bevy::text::Justify::Left, bevy::text::LineBreak::NoWrap),
                ));
            }
            if let Some([x, y]) = label_offset(&source.fdf, &label.frame)? {
                commands.entity(replacement).insert(Node {
                    left: Val::Vw(x / view[0] * 100.),
                    top: Val::Vh(-y / view[1] * 100.),
                    width: if source.fdf.width(&label.frame).is_ok_and(|w| w == 0.) {
                        Val::Auto
                    } else {
                        percent(100)
                    },
                    flex_shrink: 0.,
                    ..default()
                });
            }
            commands.entity(entity).try_despawn();
            Ok::<_, String>(())
        })();
        if let Err(error) = result {
            warn!("escape menu label: {error}");
        }
    }
}
fn measure(
    mut state: ResMut<EscapeMenu>,
    roots: Query<&ComputedNode, With<EscapeRoot>>,
    labels: Query<(&EscapeText, &bevy::text::TextLayoutInfo)>,
    mut nodes: Query<(&EscapePosition, &mut Node)>,
) {
    let Ok(root) = roots.single() else { return };
    let size = root.size();
    if size.min_element() <= 0. {
        return;
    }
    let page = state.page;
    let Some(source) = &mut state.source else {
        return;
    };
    let view = source.layout.viewport;
    for (label, layout) in &labels {
        if layout.size.min_element() > 0. {
            source.fdf.measure_text(
                &label.0,
                (
                    layout.size.x / size.x * view[0],
                    layout.size.y / size.y * view[1],
                ),
            );
        }
    }
    let panel = (
        source.fdf.width(page.frame()).unwrap(),
        source.fdf.height(page.frame()).unwrap(),
    );
    let origin = match source.layout.position([panel.0, panel.1]) {
        Ok(origin) => origin,
        Err(error) => {
            state.last_error = Some(error);
            return;
        }
    };
    for (position, mut node) in &mut nodes {
        if position.0 == source.layout.backdrop_frame {
            continue;
        }
        if let Ok(mut rect) = source.fdf.rect(&position.0, panel) {
            rect.left += origin[0];
            rect.bottom += origin[1];
            node.left = percent(rect.left / view[0] * 100.);
            node.top = percent((view[1] - rect.top()) / view[1] * 100.);
            node.width = percent(rect.width / view[0] * 100.);
            node.height = percent(rect.height / view[1] * 100.);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const VIEW: (f32, f32) = (1.2, 0.9);
    fn layout() -> pcx_play::escape_layout::Rules {
        use pcx_play::escape_layout::{Anchor, Rules};
        Rules {
            origin: pcx_play::scripts::AssetIdentity {
                source: "synthetic".into(),
                edition: "fixture".into(),
                version: "1".into(),
            },
            image_sha256: [0; 32],
            viewport: [VIEW.0, VIEW.1],
            root_extent: [VIEW.0, VIEW.1],
            root: Anchor {
                point: 4,
                relative: 4,
                offset: [0., 0.],
            },
            panel: Anchor {
                point: 4,
                relative: 4,
                offset: [0., 0.],
            },
            root_frame: "EscMenuMainPanel".into(),
            backdrop_frame: "EscMenuBackdrop".into(),
        }
    }
    #[test]
    fn justification_offsets_refuse_malformed_values() {
        for (value, expected) in [
            ("0.004 -0.031", Some([0.004, -0.031])),
            ("NaN 0", None),
            ("1", None),
        ] {
            let fdf = Fdf::parse(&format!(
                "Frame \"TEXT\" \"Label\" {{\n FontJustificationOffset {value},\n}}\n"
            ))
            .unwrap();
            match expected {
                Some(offset) => assert_eq!(label_offset(&fdf, "Label").unwrap(), Some(offset)),
                None => assert!(label_offset(&fdf, "Label").is_err()),
            }
        }
    }

    #[cfg(feature = "test-fixtures")]
    #[test]
    fn escape_backs_out_before_closing_and_consumes_the_cinematic_key() {
        use bevy::ecs::system::RunSystemOnce;
        let mut app = App::new();
        app.add_message::<KeyboardInput>()
            .init_resource::<ButtonInput<KeyCode>>()
            .insert_resource(EscapeMenu {
                page: Page::ConfirmExit,
                ..default()
            });
        let mut view = CampaignCamera::active_for_test(Entity::PLACEHOLDER);
        view.set_menu_open(true);
        app.insert_resource(view);
        for (page, open) in [(Page::End, true), (Page::Main, true), (Page::Main, false)] {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.reset_all();
            keys.press(KeyCode::Escape);
            app.world_mut().run_system_once(keyboard).unwrap();
            assert_eq!(app.world().resource::<EscapeMenu>().page, page);
            assert_eq!(app.world().resource::<CampaignCamera>().menu_open(), open);
            assert!(
                !app.world()
                    .resource::<ButtonInput<KeyCode>>()
                    .just_pressed(KeyCode::Escape)
            );
        }
    }
    #[test]
    fn help_page_maps_to_help_panel_and_backs_to_main() {
        assert_eq!(Page::Help.frame(), "HelpPanel");
        assert_eq!(Page::Help.back(), Some(Page::Main));
        assert!(enabled("HelpButton"));
        assert!(enabled("HelpOKButton"));
    }
    #[test]
    fn tips_page_maps_to_tips_panel_and_backs_to_main() {
        assert_eq!(Page::Tips.frame(), "TipsPanel");
        assert_eq!(Page::Tips.back(), Some(Page::Main));
        assert!(enabled("TipsButton"));
        assert!(enabled("TipsBackButton"));
        assert!(enabled("TipsNextButton"));
        assert!(enabled("TipsOKButton"));
        let skin = SkinTable::parse("[Default]\nPanelArt=synthetic/panel.blp\n").unwrap();
        let fdf_tips = Fdf::parse(
            r#"
Frame "FRAME" "EscMenuMainPanel" {
 SetAllPoints,
}
Frame "FRAME" "TipsPanel" {
 Width 0.432,
 Height 0.284,
 Frame "TEXTAREA" "TipsTextArea" {
  Width 0.362,
  Height 0.147,
  SetPoint TOP, "TipsPanel", TOP, 0.0, -0.05,
 }
}
Frame "BACKDROP" "EscMenuBackdrop" {
 BackdropBackground "PanelArt",
}
"#,
        )
        .unwrap();
        let plan = items(&fdf_tips, &skin, &["Default".into()], Page::Tips, &layout()).unwrap();
        assert!(plan.iter().all(|i| i.frame != "TipsTextArea"));
    }
    #[test]
    fn tips_next_back_wrap_and_retain_selection_across_hide() {
        use pcx_play::escape_help_tips::TipStrings;
        let tips = TipStrings::parse("[General]\nTipCount=3\nTip1=\"a\"\nTip2=\"b\"\nTip3=\"c\"\n")
            .unwrap();
        let mut sel = pcx_play::escape_help_tips::TipSelection::default();
        assert_eq!(sel.current(&tips), Some("a"));
        sel.previous(&tips);
        assert_eq!(
            sel.current(&tips),
            Some("c"),
            "Back from first wraps to last"
        );
        sel.next(&tips);
        assert_eq!(
            sel.current(&tips),
            Some("a"),
            "Next from last wraps to first"
        );
        sel.next(&tips);
        assert_eq!(sel.current(&tips), Some("b"));
        let retained = sel;
        assert_eq!(retained.current(&tips), Some("b"));
        let mut state = EscapeMenu {
            tips: retained,
            page: Page::Main,
            painted: None,
            ..Default::default()
        };
        assert_eq!(
            state.tips.current(&tips),
            Some("b"),
            "selection retained across hide"
        );
        state.painted = Some(Page::Tips);
        state.tips.next(&tips);
        state.painted = None;
        assert!(state.painted.is_none(), "Next/Back marks page dirty");
    }
    #[test]
    fn single_player_pause_is_hidden_without_reflowing_anchored_controls() {
        let skin = SkinTable::parse("[Default]\nPanelArt=synthetic/panel.blp\n").unwrap();
        let fdf_help = Fdf::parse(
            r#"
Frame "FRAME" "EscMenuMainPanel" {
 SetAllPoints,
}
Frame "FRAME" "HelpPanel" {
 Width 0.432,
 Height 0.384,
 Frame "TEXTAREA" "HelpTextArea" {
  Width 0.362,
  Height 0.264,
  SetPoint TOP, "HelpPanel", TOP, 0.0, -0.05,
 }
 Frame "GLUETEXTBUTTON" "HelpOKButton" {
  Width 0.16,
  Height 0.031,
  SetPoint BOTTOM, "HelpPanel", BOTTOM, 0.0, 0.03,
 }
}
Frame "BACKDROP" "EscMenuBackdrop" {
 BackdropBackground "PanelArt",
}
"#,
        )
        .unwrap();
        let plan = items(&fdf_help, &skin, &["Default".into()], Page::Help, &layout()).unwrap();
        assert!(plan.iter().any(|i| i.frame == "HelpOKButton"));
        assert!(plan.iter().all(|i| i.frame != "HelpTextArea"));
        let fdf = Fdf::parse(
            r#"
Frame "FRAME" "EscMenuMainPanel" {
 SetAllPoints,
}
Frame "FRAME" "MainPanel" {
 Width 0.41,
 Height 0.37,
 Frame "GLUETEXTBUTTON" "PauseButton" {
  Width 0.19,
  Height 0.031,
  SetPoint TOP, "EscMenuMainPanel", TOP, 0.0, -0.041,
 }
 Frame "GLUETEXTBUTTON" "SaveGameButton" {
  Width 0.19,
  Height 0.031,
  SetPoint TOP, "PauseButton", BOTTOM, 0.0, -0.017,
 }
}
Frame "BACKDROP" "EscMenuBackdrop" {
 BackdropBackground "PanelArt",
}
"#,
        )
        .unwrap();
        let skin = SkinTable::parse("[Default]\nPanelArt=synthetic/panel.blp\n").unwrap();
        let source = fdf.rect("SaveGameButton", (0.41, 0.37)).unwrap();
        let plan = items(&fdf, &skin, &["Default".into()], Page::Main, &layout()).unwrap();
        assert!(plan.iter().all(|item| item.frame != "PauseButton"));
        assert!(
            fdf.frame("PauseButton").is_some(),
            "anchor remains in the source graph"
        );
        let save = plan
            .iter()
            .find(|item| item.frame == "SaveGameButton")
            .unwrap();
        assert!((save.rect.left - source.left - (VIEW.0 - 0.41) * 0.5).abs() < 1e-6);
        assert!((save.rect.bottom - source.bottom - (VIEW.1 - 0.37) * 0.5).abs() < 1e-6);
    }

    #[test]
    fn nonstock_panel_geometry_and_skin_are_preserved() {
        let skin = SkinTable::parse("[Default]\nPanelArt=synthetic/other.blp\n").unwrap();
        for (width, height) in [(0.41, 0.29), (0.49, 0.37)] {
            let fdf = Fdf::parse(&format!(
                r#"
Frame "FRAME" "EscMenuMainPanel" {{
 SetAllPoints,
}}
Frame "FRAME" "MainPanel" {{
 Width {width},
 Height {height},
 Frame "GLUETEXTBUTTON" "ReturnButton" {{
  Width 0.13,
  Height 0.023,
  SetPoint BOTTOMLEFT, "EscMenuMainPanel", BOTTOMLEFT, 0.017, 0.043,
  ControlShortcutKey "Z",
 }}
}}
Frame "BACKDROP" "EscMenuBackdrop" {{
 BackdropBackground "PanelArt",
}}
"#
            ))
            .unwrap();
            let mut layout = layout();
            layout.root.offset = [-0.021, 0.013];
            layout.panel.offset = [0.037, 0.089];
            let plan = items(&fdf, &skin, &["Default".into()], Page::Main, &layout).unwrap();
            let backdrop = &plan[0];
            assert_eq!(backdrop.rect.width, width);
            assert_eq!(backdrop.rect.height, height);
            let Kind::Backdrop(art) = &backdrop.kind else {
                panic!("backdrop")
            };
            assert_eq!(art.background.as_deref(), Some("synthetic/other.blp"));
            let button = plan.iter().find(|i| i.frame == "ReturnButton").unwrap();
            assert!(
                (button.rect.left - ((VIEW.0 - width) * 0.5 - 0.021 + 0.037 + 0.017)).abs() < 1e-6
            );
            assert!(
                (button.rect.bottom - ((VIEW.1 - height) * 0.5 + 0.013 + 0.089 + 0.043)).abs()
                    < 1e-6
            );
            let Kind::Button(button) = &button.kind else {
                panic!("button")
            };
            assert_eq!(button.shortcut, Some('Z'));
        }
    }

    #[cfg(feature = "test-fixtures")]
    #[test]
    fn old_phase_or_session_buttons_cannot_queue_an_action() {
        let old = Arc::new(());
        let current = Arc::new(());
        let mut state = EscapeMenu {
            witness: Some(Arc::downgrade(&current)),
            page: Page::End,
            ..default()
        };
        let mut view = CampaignCamera::active_for_test(Entity::PLACEHOLDER);
        view.set_menu_open(true);
        for (phase, witness) in [(Page::Main, &current), (Page::End, &old)] {
            queue(
                &mut state,
                &EscapeButton {
                    frame: "QuitButton".into(),
                    key: None,
                    page: phase,
                    witness: Arc::downgrade(witness),
                },
                &view,
            );
            assert!(state.pending.is_none());
        }
        queue(
            &mut state,
            &EscapeButton {
                frame: "QuitButton".into(),
                key: None,
                page: Page::End,
                witness: Arc::downgrade(&current),
            },
            &view,
        );
        assert_eq!(state.pending.as_ref().unwrap().0, "QuitButton");
    }

    #[cfg(feature = "test-fixtures")]
    #[test]
    fn no_session_options_button_does_not_open_in_game_route() {
        // `act` never touches real assets: pending/guards are all set directly.
        // No real session witness exists here; the dispatcher requires the
        // admitted witness already checked at the top of `act`, so this test
        // runs through `act` with a witness-bearing pending entry only when a
        // session is admitted. With no session, `act` must refuse.
        use bevy::ecs::system::RunSystemOnce;
        let witness = Arc::new(());
        let foreign = Arc::downgrade(&witness);
        let mut app = App::new();
        app.init_resource::<EscapeMenu>()
            .init_resource::<MapRequests>()
            .init_resource::<SelectedMapSources>()
            .init_resource::<MapScriptSession<MapStartupHost>>()
            .init_resource::<crate::campaign_in_game_options::InGameOptionsRoute>()
            .init_resource::<crate::menu_screens::Navigation>()
            .add_message::<CampaignTransitionRequest>()
            .add_message::<AppExit>();
        let mut view = CampaignCamera::active_for_test(Entity::PLACEHOLDER);
        view.set_menu_open(true);
        app.insert_resource(view);
        app.insert_resource(EscapeMenu {
            page: Page::Main,
            pending: Some(("OptionsButton".into(), Page::Main, foreign)),
            ..default()
        });
        app.world_mut().run_system_once(act).unwrap();
        let route = app
            .world()
            .resource::<crate::campaign_in_game_options::InGameOptionsRoute>();
        assert_eq!(
            route.state,
            crate::campaign_in_game_options::InGameRoute::Escape
        );
        assert!(!route.is_open());
        assert!(app.world().resource::<CampaignCamera>().menu_open());
    }

    #[cfg(all(feature = "test-fixtures", not(target_arch = "wasm32")))]
    #[test]
    #[ignore = "requires selected unchanged PCX_NATIVE_CHAPTER_* inputs; assets only, no rendered parity claim"]
    fn selected_campaign_escape_assets_admit_all_owned_panels() {
        use bevy::ecs::system::RunSystemOnce;
        let mut app = crate::native_campaign::tests::started_app();
        let reader = {
            let requests = app.world().resource::<MapRequests>();
            app.world()
                .resource::<SelectedMapSources>()
                .for_receipts(&requests.bindings.as_ref().unwrap().receipts)
                .unwrap()
        };
        let race = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .host()
            .unwrap()
            .objects()
            .local_race_preference_constant()
            .unwrap();
        let mut fonts = Assets::<Font>::default();
        let install =
            std::path::PathBuf::from(std::env::var("PCX_NATIVE_CHAPTER_INSTALL").unwrap());
        let source = load(
            reader,
            race,
            &mut fonts,
            read_layout(Some(&install)).unwrap(),
            Some(&install),
        )
        .unwrap();
        assert!(!source.receipts.is_empty() && !fonts.is_empty());
        assert!(!source.help.body.is_empty());
        assert!(source.help_style.height > 0.);
        assert!(!source.help_style.font.is_empty());
        assert!(!source.tips.tips.is_empty());
        assert!(source.tips_style.height > 0.);
        assert!(!source.tips_style.font.is_empty());
        // Tips on real assets: current text comes from runtime TipStrings.
        {
            use pcx_play::escape_help_tips::TipSelection;
            let mut tips = TipSelection::default();
            assert_eq!(
                tips.current(&source.tips),
                Some(source.tips.tips[0].as_str())
            );
            tips.next(&source.tips);
            assert_eq!(
                tips.current(&source.tips),
                Some(source.tips.tips[1 % source.tips.tips.len()].as_str())
            );
            tips.previous(&source.tips);
            assert_eq!(
                tips.current(&source.tips),
                Some(source.tips.tips[0].as_str())
            );
            // Retained hide/show: same selection survives page reset to Main.
            let state = EscapeMenu {
                tips,
                page: Page::Main,
                painted: None,
                ..Default::default()
            };
            assert_eq!(
                state.tips.current(&source.tips),
                Some(source.tips.tips[0].as_str()),
                "selection retained across hide"
            );
        }
        for page in [
            Page::Main,
            Page::End,
            Page::ConfirmExit,
            Page::Help,
            Page::Tips,
        ] {
            let plan = items(
                &source.fdf,
                &source.skin,
                &source.sections,
                page,
                &source.layout,
            )
            .unwrap();
            assert!(
                plan.iter()
                    .any(|i| matches!(i.kind, Kind::Button(_)) && enabled(&i.frame))
            );
        }
        app.add_message::<AppExit>()
            .add_message::<CampaignTransitionRequest>()
            .init_resource::<crate::campaign_in_game_options::InGameOptionsRoute>()
            .init_resource::<crate::menu_screens::Navigation>();
        app.world_mut()
            .resource_mut::<crate::menu_screens::Navigation>()
            .screen = crate::menu_screens::Screen::Main;
        let context = app
            .world()
            .resource::<MapScriptSession<MapStartupHost>>()
            .context()
            .cloned()
            .unwrap();
        let witness = app
            .world()
            .resource::<MapScriptSession<MapStartupHost>>()
            .session_witness()
            .unwrap();
        let member = app
            .world()
            .resource::<MapRequests>()
            .bindings
            .as_ref()
            .unwrap()
            .receipts
            .map
            .path
            .to_string();
        let mut view = CampaignCamera::active_for_test(Entity::PLACEHOLDER);
        view.set_menu_open(true);
        app.insert_resource(view);
        app.insert_resource(EscapeMenu {
            witness: Some(witness.clone()),
            page: Page::End,
            pending: Some(("RestartButton".into(), Page::End, witness.clone())),
            ..default()
        });
        app.world_mut().run_system_once(act).unwrap();
        let requests = app
            .world_mut()
            .resource_mut::<Messages<CampaignTransitionRequest>>()
            .drain()
            .collect::<Vec<_>>();
        assert_eq!(requests.len(), 1);
        assert!(Arc::ptr_eq(&requests[0].context, &context));
        assert_eq!(requests[0].next_level, member);
        assert!(app.world().resource::<Messages<AppExit>>().is_empty());
        {
            let mut state = app.world_mut().resource_mut::<EscapeMenu>();
            state.page = Page::Main;
            state.pending = Some(("OptionsButton".into(), Page::Main, witness.clone()));
        }
        app.world_mut().run_system_once(act).unwrap();
        {
            let route = app
                .world()
                .resource::<crate::campaign_in_game_options::InGameOptionsRoute>();
            assert!(route.is_open());
            assert_eq!(
                route.state,
                crate::campaign_in_game_options::InGameRoute::Options
            );
            assert!(
                route
                    .session
                    .as_ref()
                    .is_some_and(|w| Weak::ptr_eq(w, &witness))
            );
        }
        assert!(app.world().resource::<CampaignCamera>().menu_open());
        assert_eq!(
            app.world()
                .resource::<crate::menu_screens::Navigation>()
                .requested,
            Some(crate::menu_screens::Screen::Options(
                crate::menu_screens::Page::Unselected
            ))
        );
        {
            let mut state = app.world_mut().resource_mut::<EscapeMenu>();
            state.page = Page::End;
            state.pending = Some(("RestartButton".into(), Page::End, witness.clone()));
        }
        app.world_mut().run_system_once(act).unwrap();
        let requests = app
            .world_mut()
            .resource_mut::<Messages<CampaignTransitionRequest>>()
            .drain()
            .collect::<Vec<_>>();
        assert_eq!(requests.len(), 1);
        assert!(Arc::ptr_eq(&requests[0].context, &context));
        assert_eq!(requests[0].next_level, member);
        assert!(app.world().resource::<Messages<AppExit>>().is_empty());
        {
            let mut state = app.world_mut().resource_mut::<EscapeMenu>();
            state.page = Page::ConfirmExit;
            state.pending = Some(("ConfirmQuitQuitButton".into(), Page::ConfirmExit, witness));
        }
        app.world_mut().run_system_once(act).unwrap();
        let exits = app
            .world_mut()
            .resource_mut::<Messages<AppExit>>()
            .drain()
            .collect::<Vec<_>>();
        assert!(matches!(exits.as_slice(), [AppExit::Success]));
        // Tips real act(): Next advances from runtime TipStrings, Back wraps,
        // OK returns to Main, selection retained across hide/show.
        let witness2 = app
            .world()
            .resource::<MapScriptSession<MapStartupHost>>()
            .session_witness()
            .unwrap();
        {
            let mut state = app.world_mut().resource_mut::<EscapeMenu>();
            state.source = Some(source);
            state.page = Page::Tips;
            state.painted = Some(Page::Tips);
            state.tips = pcx_play::escape_help_tips::TipSelection::default();
            state.pending = Some(("TipsNextButton".into(), Page::Tips, witness2.clone()));
        }
        app.world_mut().run_system_once(act).unwrap();
        {
            let state = app.world().resource::<EscapeMenu>();
            let tips = &state.source.as_ref().unwrap().tips;
            assert_eq!(
                state.tips.current(tips),
                Some(tips.tips[1 % tips.tips.len()].as_str())
            );
            assert!(state.painted.is_none(), "Next marks page dirty");
        }
        {
            let mut state = app.world_mut().resource_mut::<EscapeMenu>();
            state.painted = Some(Page::Tips);
            state.pending = Some(("TipsBackButton".into(), Page::Tips, witness2.clone()));
        }
        app.world_mut().run_system_once(act).unwrap();
        {
            let state = app.world().resource::<EscapeMenu>();
            let tips = &state.source.as_ref().unwrap().tips;
            assert_eq!(state.tips.current(tips), Some(tips.tips[0].as_str()));
            assert!(state.painted.is_none(), "Back marks page dirty");
        }
        {
            let mut state = app.world_mut().resource_mut::<EscapeMenu>();
            state.pending = Some(("TipsOKButton".into(), Page::Tips, witness2.clone()));
        }
        app.world_mut().run_system_once(act).unwrap();
        {
            let state = app.world().resource::<EscapeMenu>();
            assert_eq!(state.page, Page::Main);
            let tips = &state.source.as_ref().unwrap().tips;
            assert_eq!(
                state.tips.current(tips),
                Some(tips.tips[0].as_str()),
                "selection retained after OK/hide"
            );
        }
    }
}

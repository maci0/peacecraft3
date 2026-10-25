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
const VIEW: (f32, f32) = (crate::mainmenu::FRAME_W, crate::mainmenu::FRAME_H);
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Page {
    #[default]
    Main,
    End,
    ConfirmExit,
}
impl Page {
    fn frame(self) -> &'static str {
        match self {
            Self::Main => "MainPanel",
            Self::End => "EndGamePanel",
            Self::ConfirmExit => "ConfirmQuitPanel",
        }
    }
    fn back(self) -> Option<Self> {
        match self {
            Self::Main => None,
            Self::End => Some(Self::Main),
            Self::ConfirmExit => Some(Self::End),
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
}
impl EscapeMenu {
    pub(crate) fn receipt_count(&self) -> usize {
        self.source
            .as_ref()
            .map_or(0, |source| source.receipts.len())
    }
}
struct Source {
    fdf: Fdf,
    skin: SkinTable,
    sections: Vec<String>,
    strings: String,
    fonts: HashMap<String, Handle<Font>>,
    reader: Arc<pcx_assets::Resolver>,
    receipts: Vec<AssetReceipt>,
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
    matches!(
        frame,
        "ReturnButton"
            | "EndGameButton"
            | "PreviousButton"
            | "QuitButton"
            | "RestartButton"
            | "ExitButton"
            | "ConfirmQuitQuitButton"
            | "ConfirmQuitCancelButton"
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
        "EndGameButton" => state.page = Page::End,
        "PreviousButton" => state.page = Page::Main,
        "ExitButton" => state.page = Page::ConfirmExit,
        "ConfirmQuitCancelButton" => state.page = Page::End,
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
) -> Result<Vec<glue_menu::Item>, String> {
    let size = (
        fdf.width(page.frame()).map_err(|e| e.to_string())?,
        fdf.height(page.frame()).map_err(|e| e.to_string())?,
    );
    if ![size.0, size.1].iter().all(|n| n.is_finite() && *n > 0.) {
        return Err("escape menu: invalid panel extent".into());
    }
    let (mut items, unsupported) =
        glue_menu::control_items(fdf, page.frame(), size).map_err(|e| e.to_string())?;
    if !unsupported.is_empty() {
        return Err(format!("escape menu: unsupported controls {unsupported:?}"));
    }
    let backdrop = glue_menu::backdrop(fdf, "EscMenuBackdrop")
        .map_err(|e| e.to_string())?
        .ok_or("escape menu: missing backdrop")?;
    items.insert(
        0,
        glue_menu::Item {
            frame: "EscMenuBackdrop".into(),
            rect: Rect {
                left: 0.,
                bottom: 0.,
                width: size.0,
                height: size.1,
            },
            kind: Kind::Backdrop(backdrop),
        },
    );
    for item in &mut items {
        item.rect.left += (VIEW.0 - size.0) * 0.5;
        item.rect.bottom += (VIEW.1 - size.1) * 0.5;
        if ![
            item.rect.left / VIEW.0,
            item.rect.bottom / VIEW.1,
            item.rect.width / VIEW.0,
            item.rect.height / VIEW.1,
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
fn load(
    reader: Arc<pcx_assets::Resolver>,
    race: Option<String>,
    fonts: &mut Assets<Font>,
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
    let mut source = Source {
        fdf,
        skin,
        sections,
        strings,
        fonts: HashMap::new(),
        reader,
        receipts,
    };
    let mut decoded = HashMap::new();
    let mut seen = HashSet::new();
    let mut total = 0usize;
    let mut font_bytes = 0usize;
    for page in [Page::Main, Page::End, Page::ConfirmExit] {
        for item in items(&source.fdf, &source.skin, &source.sections, page)? {
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
            .and_then(|race| load(reader, race, &mut fonts))
        {
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
    let items = match items(&source.fdf, &source.skin, &source.sections, state.page) {
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
    glue_menu::spawn(&mut commands, root, &items);
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
                    left: Val::Vw(x / VIEW.0 * 100.),
                    top: Val::Vh(-y / VIEW.1 * 100.),
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
    for (label, layout) in &labels {
        if layout.size.min_element() > 0. {
            source.fdf.measure_text(
                &label.0,
                (
                    layout.size.x / size.x * VIEW.0,
                    layout.size.y / size.y * VIEW.1,
                ),
            );
        }
    }
    let panel = (
        source.fdf.width(page.frame()).unwrap(),
        source.fdf.height(page.frame()).unwrap(),
    );
    for (position, mut node) in &mut nodes {
        if position.0 == "EscMenuBackdrop" {
            continue;
        }
        if let Ok(mut rect) = source.fdf.rect(&position.0, panel) {
            rect.left += (VIEW.0 - panel.0) * 0.5;
            rect.bottom += (VIEW.1 - panel.1) * 0.5;
            node.left = percent(rect.left / VIEW.0 * 100.);
            node.top = percent((VIEW.1 - rect.top()) / VIEW.1 * 100.);
            node.width = percent(rect.width / VIEW.0 * 100.);
            node.height = percent(rect.height / VIEW.1 * 100.);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
            let plan = items(&fdf, &skin, &["Default".into()], Page::Main).unwrap();
            let backdrop = &plan[0];
            assert_eq!(backdrop.rect.width, width);
            assert_eq!(backdrop.rect.height, height);
            let Kind::Backdrop(art) = &backdrop.kind else {
                panic!("backdrop")
            };
            assert_eq!(art.background.as_deref(), Some("synthetic/other.blp"));
            let button = plan.iter().find(|i| i.frame == "ReturnButton").unwrap();
            assert!((button.rect.left - ((VIEW.0 - width) * 0.5 + 0.017)).abs() < 1e-6);
            assert!((button.rect.bottom - ((VIEW.1 - height) * 0.5 + 0.043)).abs() < 1e-6);
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
        let source = load(reader, race, &mut fonts).unwrap();
        assert!(!source.receipts.is_empty() && !fonts.is_empty());
        for page in [Page::Main, Page::End, Page::ConfirmExit] {
            let plan = items(&source.fdf, &source.skin, &source.sections, page).unwrap();
            assert!(
                plan.iter()
                    .any(|i| matches!(i.kind, Kind::Button(_)) && enabled(&i.frame))
            );
        }
        app.add_message::<AppExit>()
            .add_message::<CampaignTransitionRequest>();
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
    }
}

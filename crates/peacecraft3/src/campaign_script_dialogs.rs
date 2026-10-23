//! Live JASS script dialogs. Dimensions/art/text come from the selected source;
//! native dynamic geometry is decoded from the qualified selected client.
use crate::{
    campaign_hud::upper_buttons::{image_sampler, read, skin_file, text},
    fdf::{Fdf, Rect},
    glue_menu::{self, Backdrop, BackdropImage, EdgePiece},
    menu_skin::SkinTable,
    menu_text,
};
use bevy::{
    ecs::system::SystemParam,
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    prelude::*,
};
use pcx_play::{
    map_startup_host::MapStartupHost,
    playercolor_host::{DialogQuitRequest, ScriptDialog},
};
use pcx_script::HandleId;
use peacecraft3::{
    browser_configuration::SelectedMapSources,
    campaign_camera::{CampaignCamera, CampaignCameraSet},
    campaign_presentation::{CampaignHud, CampaignPresentationSet},
    map_binding::AssetReceipt,
    map_requests::MapRequests,
    map_script_session::MapScriptSession,
};
use std::{collections::HashMap, sync::Weak};
const VIEW: (f32, f32) = (crate::mainmenu::FRAME_W, crate::mainmenu::FRAME_H);
const PANEL: &str = "UI\\FrameDef\\UI\\ScriptDialog.fdf";
const TEMPLATES: &str = "UI\\FrameDef\\UI\\EscMenuTemplates.fdf";
const DIALOG_LAYER: i32 = peacecraft3::campaign_presentation::CAMPAIGN_CONTROL_BARRIER_LAYER + 1;

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct CampaignScriptDialogSet;
#[derive(Message, Clone, Debug)]
pub(crate) struct CampaignDialogQuit {
    pub request: DialogQuitRequest,
    pub session: Weak<()>,
}
#[derive(Resource, Default)]
pub(crate) struct CampaignScriptDialogs {
    witness: Option<Weak<()>>,
    root: Option<Entity>,
    assets: Option<Source>,
    attempted: bool,
    revision: Option<u64>,
    blocked: bool,
    consumed: bool,
    pending: Option<HandleId>,
    caption: Option<CaptionExtent>,
    ready: bool,
    pub last_error: Option<String>,
    pub receipts: Vec<AssetReceipt>,
}
impl CampaignScriptDialogs {
    pub fn input_blocked(&self) -> bool {
        self.blocked || self.consumed
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
struct CaptionExtent {
    dialog: HandleId,
    revision: u64,
    viewport: UVec2,
    scale: f32,
    width: f32,
    height: f32,
}
#[derive(Component)]
struct CaptionProbe {
    root: Entity,
    dialog: HandleId,
    revision: u64,
    witness: Weak<()>,
}
struct CaptionLayout {
    formatting_only: bool,
    revision: u64,
    witness: Weak<()>,
    extent: Option<CaptionExtent>,
}
#[derive(Component)]
struct Activate(HandleId);
#[derive(Component)]
struct ButtonVisual {
    owner: Entity,
    pushed: bool,
}
#[derive(Component)]
struct PushedLabel {
    owner: Entity,
    offset: Vec2,
}
struct Source {
    fdf: Fdf,
    rules: pcx_play::dialog_layout::Rules,
    styles: [menu_text::TextStyle; 2],
    fonts: HashMap<String, Handle<Font>>,
    backdrop: Backdrop,
    buttons: glue_menu::Button,
    images: HashMap<(String, bool), Handle<Image>>,
    materials: Vec<Handle<BackdropImage>>,
}
#[derive(SystemParam)]
struct RenderAssets<'w> {
    fonts: ResMut<'w, Assets<Font>>,
    images: ResMut<'w, Assets<Image>>,
    materials: ResMut<'w, Assets<BackdropImage>>,
}
pub(crate) fn install(app: &mut App) {
    crate::button_actions::install(app);
    app.init_resource::<CampaignScriptDialogs>()
        .add_message::<CampaignDialogQuit>()
        .add_observer(capture_activate)
        .add_observer(capture_click)
        .add_systems(
            Update,
            (
                activate.before(crate::button_actions::clear),
                project
                    .after(CampaignCameraSet)
                    .before(CampaignPresentationSet),
                sync_pressed,
            )
                .chain()
                .in_set(CampaignScriptDialogSet),
        )
        .add_systems(
            PostUpdate,
            measure_caption.after(bevy::ui::UiSystems::PostLayout),
        );
}
fn capture_activate(
    event: On<bevy::ui_widgets::Activate>,
    controls: Query<&Activate, Without<bevy::ui::InteractionDisabled>>,
    mut state: ResMut<CampaignScriptDialogs>,
) {
    if state.blocked
        && let Ok(button) = controls.get(event.entity)
    {
        state.pending = Some(button.0);
    }
}
fn capture_click(
    event: On<bevy::picking::events::PointerClick>,
    controls: Query<&Activate, Without<bevy::ui::InteractionDisabled>>,
    mut state: ResMut<CampaignScriptDialogs>,
) {
    if state.blocked
        && event.button == bevy::picking::pointer::PointerButton::Primary
        && let Ok(button) = controls.get(event.entity)
    {
        state.pending = Some(button.0);
    }
}
fn take_displayed_input(
    state: &mut CampaignScriptDialogs,
    current: Option<Weak<()>>,
) -> Option<(Weak<()>, Option<HandleId>)> {
    let pending = state.pending.take();
    let current = current?;
    let displayed = state.witness.as_ref()?;
    Weak::ptr_eq(displayed, &current).then_some((current, pending))
}
fn activate(
    mut keys: MessageReader<KeyboardInput>,
    mut state: ResMut<CampaignScriptDialogs>,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
    mut quits: MessageWriter<CampaignDialogQuit>,
    view: Res<CampaignCamera>,
) {
    state.consumed = false;
    let displayed_input = take_displayed_input(&mut state, session.session_witness());
    let keyboard: Vec<_> = keys
        .read()
        .filter(|e| e.state == ButtonState::Pressed && !e.repeat)
        .filter_map(|e| match &e.logical_key {
            Key::Character(s) => {
                let mut chars = s.chars();
                let c = chars.next()?;
                chars.next().is_none().then_some(c as i32)
            }
            Key::Enter => Some(13),
            Key::Escape => Some(27),
            _ => None,
        })
        .collect();
    if !state.blocked || !state.ready || !view.active() {
        return;
    }
    let Some((witness, pending)) = displayed_input else {
        return;
    };
    let Some(host) = session.host() else {
        return;
    };
    let Some(number) = host.objects().local_player_number() else {
        return;
    };
    let Some(player) = host.objects().player_identity(number) else {
        return;
    };
    let Some(dialog) = host.objects().dialog_state().and_then(|s| {
        s.dialogs()
            .filter(|d| d.visible_to(number))
            .max_by_key(|d| d.shown_revision)
    }) else {
        return;
    };
    let clicked = pending
        .filter(|id| dialog.buttons.iter().any(|b| b.id == *id))
        .or_else(|| {
            dialog
                .buttons
                .iter()
                .find(|b| {
                    b.hotkey != 0 && keyboard.iter().any(|key| hotkey_matches(b.hotkey, *key))
                })
                .map(|b| b.id)
        });
    let Some(button) = clicked else {
        return;
    };
    state.consumed = true;
    match host.objects_mut().click_dialog_button(player, button) {
        Ok(_) => {
            for request in host.objects_mut().take_dialog_quit_requests() {
                quits.write(CampaignDialogQuit {
                    request,
                    session: witness.clone(),
                });
            }
        }
        Err(e) => state.last_error = Some(e.to_string()),
    }
}
fn sync_pressed(
    buttons: Query<(), With<bevy::ui::Pressed>>,
    mut visuals: Query<(&ButtonVisual, &mut Node), Without<PushedLabel>>,
    mut labels: Query<(&PushedLabel, &mut Node), Without<ButtonVisual>>,
) {
    for (visual, mut node) in &mut visuals {
        node.display = if buttons.contains(visual.owner) == visual.pushed {
            Display::Flex
        } else {
            Display::None
        };
    }
    for (label, mut node) in &mut labels {
        let offset = if buttons.contains(label.owner) {
            label.offset
        } else {
            Vec2::ZERO
        };
        node.left = percent(offset.x);
        node.top = percent(offset.y);
    }
}
fn hotkey_matches(authored: i32, input: i32) -> bool {
    if authored == input {
        return true;
    }
    match (
        u32::try_from(authored).ok().and_then(char::from_u32),
        u32::try_from(input).ok().and_then(char::from_u32),
    ) {
        (Some(a), Some(b)) => a.to_uppercase().eq(b.to_uppercase()),
        _ => false,
    }
}
fn clear_tree(
    commands: &mut Commands,
    state: &mut CampaignScriptDialogs,
    materials: &mut Assets<BackdropImage>,
) {
    if let Some(root) = state.root.take() {
        commands.entity(root).despawn();
    }
    if let Some(source) = state.assets.as_mut() {
        for h in source.materials.drain(..) {
            materials.remove(h.id());
        }
    }
}
fn project(
    mut commands: Commands,
    mut state: ResMut<CampaignScriptDialogs>,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
    view: Res<CampaignCamera>,
    requests: Res<MapRequests>,
    sources: Res<SelectedMapSources>,
    mut assets: RenderAssets,
) {
    let witness = session.session_witness();
    if !match (&state.witness, &witness) {
        (Some(a), Some(b)) => Weak::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    } {
        clear_tree(&mut commands, &mut state, &mut assets.materials);
        if let Some(source) = state.assets.take() {
            for h in source.fonts.values() {
                assets.fonts.remove(h.id());
            }
            for h in source.images.values() {
                assets.images.remove(h.id());
            }
        }
        *state = CampaignScriptDialogs {
            witness,
            ..default()
        };
    }
    let Some(host) = session.host() else {
        state.blocked = false;
        return;
    };
    let local = host.objects().local_player_number();
    let revision = host.objects().dialog_state().map(|s| s.revision());
    let dialog = local.and_then(|number| {
        host.objects()
            .dialog_state()?
            .dialogs()
            .filter(|d| d.visible_to(number))
            .max_by_key(|d| d.shown_revision)
            .cloned()
    });
    state.blocked = view.active() && dialog.is_some();
    if !state.blocked {
        clear_tree(&mut commands, &mut state, &mut assets.materials);
        state.revision = None;
        state.caption = None;
        state.ready = false;
        return;
    }
    if state.revision == revision && state.root.is_some() {
        return;
    }
    let Some(dialog) = dialog else {
        return;
    };
    if state.assets.is_none() {
        if state.attempted {
            return;
        }
        let Some(rules) = session
            .host()
            .and_then(|h| h.objects().dialog_layout().cloned())
        else {
            state.attempted = true;
            state.last_error =
                Some("script dialog source-native layout rules are not admitted".into());
            return;
        };
        let (Some(context), Some(bindings)) = (session.context(), requests.bindings.as_ref())
        else {
            return;
        };
        if !requests
            .admission
            .shared_context()
            .as_ref()
            .is_some_and(|c| std::sync::Arc::ptr_eq(c, context))
            || !requests
                .published_receipts
                .as_ref()
                .is_some_and(|p| std::sync::Arc::ptr_eq(p, &bindings.receipts))
        {
            return;
        }
        let Some(reader) = sources.for_receipts(&bindings.receipts) else {
            return;
        };
        let race = session
            .host()
            .and_then(|h| h.objects().local_race_preference_constant().ok().flatten());
        state.attempted = true;
        let result = race
            .as_deref()
            .and_then(|s| s.strip_prefix("RACE_PREF_"))
            .ok_or_else(|| "script dialog has no selected local race".to_string())
            .and_then(|race| load(&reader, race, &mut assets, rules, &mut state.receipts));
        match result {
            Ok(source) => state.assets = Some(source),
            Err(e) => {
                state.last_error = Some(e);
                return;
            }
        }
    }
    let formatting_only = match caption_is_formatting_only(dialog.message.as_deref()) {
        Ok(empty) => empty,
        Err(error) => {
            clear_tree(&mut commands, &mut state, &mut assets.materials);
            state.ready = false;
            state.last_error = Some(error);
            return;
        }
    };
    let layout = CaptionLayout {
        formatting_only,
        revision: revision.expect("displayed dialog revision"),
        witness: state.witness.clone().expect("displayed session witness"),
        extent: state
            .caption
            .filter(|c| c.dialog == dialog.id && Some(c.revision) == revision),
    };
    let ready = dialog.message.as_ref().is_none_or(String::is_empty)
        || layout.formatting_only
        || layout.extent.is_some();
    clear_tree(&mut commands, &mut state, &mut assets.materials);
    state.ready = false;
    let source = state.assets.as_mut().expect("loaded source");
    match paint(
        &mut commands,
        source,
        &dialog,
        layout,
        &assets.images,
        &mut assets.materials,
    ) {
        Ok(root) => {
            state.root = Some(root);
            state.ready = ready;
            state.revision = revision;
            state.last_error = None;
        }
        Err(e) => state.last_error = Some(e),
    }
}
fn caption_is_formatting_only(message: Option<&str>) -> Result<bool, String> {
    let Some(message) = message.filter(|s| !s.is_empty()) else {
        return Ok(false);
    };
    let parsed = crate::rich_text::parse(message).map_err(|e| e.to_string())?;
    if !parsed.unsupported.is_empty() {
        return Err("script dialog contains unsupported substitutions".into());
    }
    Ok(parsed.spans.is_empty())
}
fn caption_extent(
    probe: &CaptionProbe,
    size: Vec2,
    viewport: UVec2,
    scale: f32,
    width: f32,
) -> Result<CaptionExtent, String> {
    if viewport.min_element() == 0
        || !size.is_finite()
        || size.y <= 0.
        || !scale.is_finite()
        || scale <= 0.
        || !width.is_finite()
        || width <= 0.
    {
        return Err("script dialog caption has no finite shaped extent/target".into());
    }
    Ok(CaptionExtent {
        dialog: probe.dialog,
        revision: probe.revision,
        viewport,
        scale,
        width,
        height: size.y / viewport.y as f32 * VIEW.1,
    })
}
fn measure_caption(
    mut state: ResMut<CampaignScriptDialogs>,
    labels: Query<(
        &CaptionProbe,
        &bevy::text::TextLayoutInfo,
        &bevy::ui::ComputedUiRenderTargetInfo,
        &ComputedNode,
    )>,
    mut roots: Query<&mut Visibility>,
) {
    for (probe, text, target, node) in &labels {
        if state.root != Some(probe.root)
            || !state
                .witness
                .as_ref()
                .is_some_and(|w| Weak::ptr_eq(w, &probe.witness))
            || state.revision != Some(probe.revision)
        {
            continue;
        }
        let extent = caption_extent(
            probe,
            text.size,
            target.physical_size(),
            target.scale_factor(),
            node.content_box().width(),
        );
        match extent {
            Ok(extent) if state.caption != Some(extent) => {
                state.caption = Some(extent);
                state.revision = None;
                state.ready = false;
                if let Ok(mut visibility) = roots.get_mut(probe.root) {
                    *visibility = Visibility::Hidden;
                }
            }
            Ok(_) => {}
            Err(error) => state.last_error = Some(error),
        }
    }
}
fn art_paths(art: &mut Backdrop, skin: &SkinTable, sections: &[String]) -> Result<(), String> {
    if let Some(token) = art.background.as_mut() {
        *token = skin_file(skin, sections, token)?;
    }
    if let Some(edge) = art.edge.as_mut() {
        edge.file = skin_file(skin, sections, &edge.file)?;
    }
    Ok(())
}
fn load(
    reader: &pcx_assets::Resolver,
    race: &str,
    assets: &mut RenderAssets,
    rules: pcx_play::dialog_layout::Rules,
    receipts: &mut Vec<AssetReceipt>,
) -> Result<Source, String> {
    let mut fdf = Fdf::parse(&text(reader, TEMPLATES, receipts)?).map_err(|e| e.to_string())?;
    fdf.parse_into(&text(reader, PANEL, receipts)?)
        .map_err(|e| e.to_string())?;
    let skin = SkinTable::parse(&text(reader, "UI\\war3skins.txt", receipts)?)
        .map_err(|e| format!("{e:?}"))?;
    let race_section = skin
        .sections()
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case(race))
        .ok_or("script dialog skin has no selected race section")?
        .name
        .clone();
    let sections = vec![race_section, "Default".into()];
    let styles = [
        menu_text::render_style(&fdf, "ScriptDialogText")?,
        menu_text::render_style(&fdf, "ScriptDialogButtonText")?,
    ];
    let mut font_bytes = HashMap::new();
    for style in &styles {
        if font_bytes.contains_key(&style.font) {
            continue;
        }
        let path = skin
            .unique_file(&style.font)
            .map_err(|e| format!("{e:?}"))?
            .file;
        let bytes = read(reader, &path, 16 << 20, receipts)?;
        if !crate::hud_render::font_registers(&bytes) {
            return Err(format!("{path}: no readable font face"));
        }
        font_bytes.insert(style.font.clone(), Font::from_bytes(bytes));
    }
    let mut backdrop = glue_menu::backdrop(&fdf, "ScriptDialogBackdrop")
        .map_err(|e| e.to_string())?
        .ok_or("missing ScriptDialogBackdrop")?;
    art_paths(&mut backdrop, &skin, &sections)?;
    let mut buttons = glue_menu::button(&fdf, "ScriptDialogButton").map_err(|e| e.to_string())?;
    for art in [
        &mut buttons.normal,
        &mut buttons.pushed,
        &mut buttons.disabled,
    ]
    .into_iter()
    .flatten()
    {
        art_paths(art, &skin, &sections)?;
    }
    if let Some(highlight) = buttons.mouse_over.as_mut() {
        highlight.file = skin_file(&skin, &sections, &highlight.file)?;
    }
    let mut decoded = HashMap::new();
    let mut total = 0usize;
    for art in std::iter::once(&backdrop).chain(
        [&buttons.normal, &buttons.pushed, &buttons.disabled]
            .into_iter()
            .flatten(),
    ) {
        for (path, tiled) in art
            .background
            .iter()
            .map(|p| (p, art.tile))
            .chain(art.edge.iter().map(|e| (&e.file, false)))
        {
            let key = (path.clone(), tiled);
            if decoded.contains_key(&key) {
                continue;
            }
            let bytes = read(reader, path, 4 << 20, receipts)?;
            let texture = pcx_gfx::load_image_bounded(&bytes, path, 16 << 20)
                .map_err(|e| format!("{path}: {e}"))?;
            total = total
                .checked_add(
                    texture
                        .rgba_bytes()
                        .ok_or("invalid script dialog texture size")?,
                )
                .filter(|n| *n <= 64 << 20)
                .ok_or("script dialog texture budget exceeded")?;
            let mut image = crate::menu_composition::image(&texture);
            image.sampler = bevy::image::ImageSampler::Descriptor(image_sampler(tiled));
            decoded.insert(key, image);
        }
    }
    dimensions(&fdf, &backdrop)?;
    Ok(Source {
        fdf,
        rules,
        styles,
        backdrop,
        buttons,
        fonts: font_bytes
            .into_iter()
            .map(|(k, v)| (k, assets.fonts.add(v)))
            .collect(),
        images: decoded
            .into_iter()
            .map(|(k, v)| (k, assets.images.add(v)))
            .collect(),
        materials: Vec::new(),
    })
}
#[derive(Debug)]
struct Dimensions {
    width: f32,
    inset: f32,
    button_width: f32,
    button_height: f32,
}
fn dimensions(fdf: &Fdf, backdrop: &Backdrop) -> Result<Dimensions, String> {
    let width = fdf.width("ScriptDialog").map_err(|e| e.to_string())?;
    let source_height = fdf.height("ScriptDialog").map_err(|e| e.to_string())?;
    let button_width = fdf.width("ScriptDialogButton").map_err(|e| e.to_string())?;
    let button_height = fdf
        .height("ScriptDialogButton")
        .map_err(|e| e.to_string())?;
    let inset = backdrop
        .inset
        .ok_or("script dialog missing authored equal background insets")?;
    if [width, source_height, button_width, button_height]
        .iter()
        .any(|v| !v.is_finite() || *v <= 0.)
        || !inset.is_finite()
        || inset < 0.
        || width > VIEW.0
        || button_width + inset * 2. > width
    {
        return Err("script dialog authored dimensions are invalid".into());
    }
    Ok(Dimensions {
        width,
        inset,
        button_width,
        button_height,
    })
}

pub(crate) fn label(
    commands: &mut Commands,
    parent: Entity,
    value: &str,
    style: &menu_text::TextStyle,
    font: &Handle<Font>,
) -> Result<Entity, String> {
    let parsed = crate::rich_text::parse(value).map_err(|e| e.to_string())?;
    if !parsed.unsupported.is_empty() {
        return Err("script dialog contains unsupported substitutions".into());
    }
    let color = |c: [f32; 4]| Color::srgba(c[0], c[1], c[2], c[3]);
    let font = TextFont {
        font: bevy::text::FontSource::Handle(font.clone()),
        font_size: bevy::text::FontSize::Vh(style.height / VIEW.1 * 100.),
        ..default()
    };
    let line = commands
        .spawn((
            Node {
                width: percent(100.),
                flex_shrink: 0.,
                ..default()
            },
            Text::new(""),
            font.clone(),
            TextColor(color(style.colour)),
            TextLayout::new(
                match style.justify_h {
                    menu_text::JustifyH::Left => bevy::text::Justify::Left,
                    menu_text::JustifyH::Center => bevy::text::Justify::Center,
                    menu_text::JustifyH::Right => bevy::text::Justify::Right,
                },
                bevy::text::LineBreak::WordBoundary,
            ),
            TextShadow {
                offset: Vec2::ZERO,
                color: color(style.shadow_colour),
            },
            glue_menu::FrameShadow(Vec2::from(style.shadow_offset)),
            Pickable::IGNORE,
        ))
        .id();
    for span in parsed.spans {
        let child = commands
            .spawn((
                TextSpan::new(span.text),
                font.clone(),
                TextColor(
                    span.colour
                        .map(color)
                        .unwrap_or_else(|| color(style.colour)),
                ),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(line).add_child(child);
    }
    commands.entity(parent).add_child(line);
    Ok(line)
}
fn place(rect: Rect, size: Vec2) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: percent(rect.left / size.x * 100.),
        top: percent((size.y - rect.top()) / size.y * 100.),
        width: percent(rect.width / size.x * 100.),
        height: percent(rect.height / size.y * 100.),
        ..default()
    }
}
fn backdrop(
    commands: &mut Commands,
    parent: Entity,
    source: &mut Source,
    art: &Backdrop,
    size: Vec2,
    images: &Assets<Image>,
    materials: &mut Assets<BackdropImage>,
) -> Result<(), String> {
    if art.mirrored {
        return Err("script dialog mirrored backdrop is unsupported".into());
    }
    if let Some(path) = &art.background {
        let inset = art.inset.unwrap_or(0.);
        let rect = Rect {
            left: inset,
            bottom: inset,
            width: size.x - inset * 2.,
            height: size.y - inset * 2.,
        };
        if !inset.is_finite() || inset < 0. || rect.width <= 0. || rect.height <= 0. {
            return Err("script dialog backdrop inset is invalid".into());
        }
        let image = source
            .images
            .get(&(path.clone(), art.tile))
            .ok_or("missing script dialog background")?
            .clone();
        let mut child = commands.spawn((place(rect, size), Pickable::IGNORE));
        if art.tile {
            let tile = art
                .background_size
                .ok_or("script dialog missing authored background tile size")?;
            if !tile.is_finite() || tile <= 0. {
                return Err("invalid script dialog tile size".into());
            }
            let repeats = Vec2::new(rect.width, rect.height) / tile;
            let material = materials.add(BackdropImage {
                texture: image,
                uv_transform: Vec4::new(repeats.x, repeats.y, 0., 0.),
                uv_rows: Vec4::new(1., 0., 0., 1.),
            });
            child.insert(MaterialNode(material.clone()));
            source.materials.push(material);
        } else {
            child.insert(bevy::ui::widget::ImageNode::new(image));
        }
        let id = child.id();
        commands.entity(parent).add_child(id);
    }
    if let Some(edge) = &art.edge {
        if !edge.corner_size.is_finite() || edge.corner_size <= 0. {
            return Err("invalid script dialog corner size".into());
        }
        let handle = source
            .images
            .get(&(edge.file.clone(), false))
            .ok_or("missing script dialog edge")?
            .clone();
        let image = images
            .get(&handle)
            .ok_or("script dialog edge image removed")?;
        for &piece in &edge.pieces {
            let rect = glue_menu::edge_rect(piece, size, edge.corner_size);
            if rect.width <= 0. || rect.height <= 0. {
                continue;
            }
            let uv = glue_menu::edge_source_rect(piece, image.width(), image.height())
                .map_err(|e| e.to_string())?;
            let mut child = commands.spawn((place(rect, size), Pickable::IGNORE));
            if matches!(piece, EdgePiece::Top | EdgePiece::Bottom) {
                let material = materials.add(glue_menu::horizontal_edge_material(
                    handle.clone(),
                    uv,
                    Vec2::new(image.width() as f32, image.height() as f32),
                    false,
                ));
                child.insert(MaterialNode(material.clone()));
                source.materials.push(material);
            } else {
                let mut node = bevy::ui::widget::ImageNode::new(handle.clone());
                node.rect = Some(uv);
                child.insert(node);
            }
            let id = child.id();
            commands.entity(parent).add_child(id);
        }
    }
    Ok(())
}
fn paint(
    commands: &mut Commands,
    source: &mut Source,
    dialog: &ScriptDialog,
    layout: CaptionLayout,
    images: &Assets<Image>,
    materials: &mut Assets<BackdropImage>,
) -> Result<Entity, String> {
    let l = dimensions(&source.fdf, &source.backdrop)?;
    let has_caption = dialog.message.as_ref().is_some_and(|s| !s.is_empty());
    let geometry = source.rules.geometry(
        layout
            .extent
            .map(|c| c.height)
            .or(layout.formatting_only.then_some(0.)),
        l.button_height,
        dialog.buttons.len(),
    )?;
    let height = geometry.height;
    if height > VIEW.1 {
        return Err("script dialog source layout exceeds the viewport".into());
    }
    // Hidden candidates still lay out text; only measured native geometry is shown.
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: percent(100.),
                height: percent(100.),
                ..default()
            },
            if has_caption && !layout.formatting_only && layout.extent.is_none() {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            },
            GlobalZIndex(DIALOG_LAYER),
            CampaignHud,
            Name::new("Script dialog input barrier"),
        ))
        .id();
    let panel = commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            left: percent((1. - l.width / VIEW.0) * 50.),
            top: percent((1. - height / VIEW.1) * 50.),
            width: percent(l.width / VIEW.0 * 100.),
            height: percent(height / VIEW.1 * 100.),
            ..default()
        })
        .id();
    commands.entity(root).add_child(panel);
    let result = (|| -> Result<(), String> {
        let art = source.backdrop.clone();
        backdrop(
            commands,
            panel,
            source,
            &art,
            Vec2::new(l.width, height),
            images,
            materials,
        )?;
        let caption = commands
            .spawn(Node {
                position_type: PositionType::Absolute,
                left: percent(l.inset / l.width * 100.),
                top: percent(geometry.caption_top / height * 100.),
                width: percent((l.width - l.inset * 2.) / l.width * 100.),
                ..default()
            })
            .id();
        commands.entity(panel).add_child(caption);
        if let Some(message) = &dialog.message
            && has_caption
            && !layout.formatting_only
        {
            let label = label(
                commands,
                caption,
                message,
                &source.styles[0],
                &source.fonts[&source.styles[0].font],
            )?;
            commands.entity(label).insert(CaptionProbe {
                root,
                dialog: dialog.id,
                revision: layout.revision,
                witness: layout.witness,
            });
        }
        for (index, button) in dialog.buttons.iter().enumerate() {
            let rect = Rect {
                left: (l.width - l.button_width) * 0.5,
                bottom: height
                    - geometry.first_button_top
                    - geometry.button_step * index as f32
                    - l.button_height,
                width: l.button_width,
                height: l.button_height,
            };
            let mut button_node = place(rect, Vec2::new(l.width, height));
            button_node.align_items = AlignItems::Center;
            let node = commands
                .spawn((
                    bevy::ui_widgets::Button,
                    Activate(button.id),
                    button_node,
                    Name::new("Script dialog button"),
                ))
                .id();
            commands.entity(panel).add_child(node);
            for (pushed, art) in [
                (false, source.buttons.normal.clone()),
                (true, source.buttons.pushed.clone()),
            ] {
                if let Some(art) = art {
                    let layer = commands
                        .spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                width: percent(100.),
                                height: percent(100.),
                                ..default()
                            },
                            ButtonVisual {
                                owner: node,
                                pushed,
                            },
                            Pickable::IGNORE,
                        ))
                        .id();
                    commands.entity(node).add_child(layer);
                    backdrop(
                        commands,
                        layer,
                        source,
                        &art,
                        Vec2::new(l.button_width, l.button_height),
                        images,
                        materials,
                    )?;
                }
            }
            let label = label(
                commands,
                node,
                &button.text,
                &source.styles[1],
                &source.fonts[&source.styles[1].font],
            )?;
            if let Some([x, y]) = source.buttons.pushed_text_offset {
                commands.entity(label).insert(PushedLabel {
                    owner: node,
                    offset: Vec2::new(x / l.button_width * 100., -y / l.button_height * 100.),
                });
            }
        }
        Ok(())
    })();
    if let Err(error) = result {
        commands.entity(root).despawn();
        for handle in source.materials.drain(..) {
            materials.remove(handle.id());
        }
        return Err(error);
    }
    Ok(root)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn shape_caption(value: &str, width: f32, scale: f32) -> Vec2 {
        use bevy::{
            ecs::system::RunSystemOnce,
            text::{
                ComputedTextBlock, DEFAULT_FONT_DATA, FontAtlasSet, FontCx, FontHinting, LayoutCx,
                RemSize, ScaleCx, TextBounds, TextElement, TextLayoutInfo, TextPipeline,
            },
        };
        let mut app = App::new();
        app.init_resource::<FontCx>();
        let mut fonts = Assets::<Font>::default();
        let font = fonts.add(Font::from_bytes(DEFAULT_FONT_DATA.to_vec()));
        app.insert_resource(fonts);
        app.world_mut()
            .run_system_once(bevy::text::load_font_assets_into_font_collection)
            .unwrap();
        let mut font_cx = app.world_mut().remove_resource::<FontCx>().unwrap();
        let font = TextFont {
            font: bevy::text::FontSource::Handle(font),
            font_size: bevy::text::FontSize::Px(20.),
            ..default()
        };
        let parsed = crate::rich_text::parse(value).unwrap();
        let runs = parsed.spans.iter().map(|span| {
            (
                Entity::PLACEHOLDER,
                0,
                TextElement::Text {
                    text: &span.text,
                    font: &font,
                    color: Color::WHITE,
                    line_height: default(),
                    letter_spacing: default(),
                },
            )
        });
        let mut block = ComputedTextBlock::default();
        let mut info = TextLayoutInfo::default();
        let mut pipeline = TextPipeline::default();
        let bounds = TextBounds::new_horizontal(width * scale);
        pipeline
            .update_buffer(
                app.world().resource::<Assets<Font>>(),
                runs,
                bevy::text::LineBreak::WordBoundary,
                bevy::text::Justify::Left,
                bounds,
                scale,
                &mut block,
                &mut font_cx,
                &mut LayoutCx::default(),
                Vec2::new(800., 600.) * scale,
                RemSize::default(),
            )
            .unwrap();
        pipeline
            .update_text_layout_info(
                &mut info,
                &mut FontAtlasSet::default(),
                &mut Assets::<Image>::default(),
                &mut block,
                &mut ScaleCx::default(),
                bounds,
                bevy::text::Justify::Left,
                FontHinting::default(),
            )
            .unwrap();
        info.size
    }
    #[test]
    fn shaped_caption_wraps_breaks_and_scales_without_font_height_estimates() {
        let witness = std::sync::Arc::new(());
        let probe = CaptionProbe {
            root: Entity::PLACEHOLDER,
            dialog: HandleId::new(std::num::NonZeroU64::new(7).unwrap()),
            revision: 3,
            witness: std::sync::Arc::downgrade(&witness),
        };
        let text = "First |cffff0000coloured caption|r with several words";
        let wide = shape_caption(text, 700., 1.);
        let narrow = shape_caption(text, 120., 1.);
        let broken = shape_caption("First|nSecond|nThird", 700., 1.);
        assert!(narrow.y > wide.y);
        assert!(broken.y > wide.y);
        let measured = caption_extent(&probe, narrow, UVec2::new(800, 600), 1., 120.).unwrap();
        assert_eq!(measured.height, narrow.y / 600. * VIEW.1);
        assert_eq!(
            measured,
            caption_extent(&probe, narrow, UVec2::new(800, 600), 1., 120.).unwrap(),
            "unchanged geometry converges"
        );
        let doubled = caption_extent(
            &probe,
            shape_caption(text, 120., 2.),
            UVec2::new(1600, 1200),
            2.,
            240.,
        )
        .unwrap();
        assert!((doubled.height - measured.height).abs() < 0.00001);
        assert_ne!(
            doubled, measured,
            "viewport and effective width/scale invalidate old measurements"
        );
        assert!(caption_extent(&probe, Vec2::ZERO, UVec2::new(800, 600), 1., 120.).is_err());
        assert!(caption_extent(&probe, narrow, UVec2::ZERO, 1., 120.).is_err());
        assert!(caption_extent(&probe, narrow, UVec2::new(800, 600), f32::NAN, 120.).is_err());
    }
    #[test]
    fn pointer_press_and_release_reach_modal_above_enabled_gameplay_barrier() {
        use bevy::{
            app::{HierarchyPropagatePlugin, PropagateSet},
            camera::{ComputedCameraValues, RenderTarget, RenderTargetInfo},
            input::{InputPlugin, mouse::MouseButtonInput},
            picking::DefaultPickingPlugins,
            ui::{
                ComputedUiRenderTargetInfo, ComputedUiTargetCamera, UiGlobalTransform, UiStack,
                picking_backend::UiPickingPlugin,
            },
            ui_widgets::ButtonPlugin,
            window::{CursorMoved, PrimaryWindow, WindowEvent, WindowRef},
        };
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            InputPlugin,
            DefaultPickingPlugins,
            UiPickingPlugin,
            ButtonPlugin,
            HierarchyPropagatePlugin::<ComputedUiTargetCamera>::new(PostUpdate),
            HierarchyPropagatePlugin::<ComputedUiRenderTargetInfo>::new(PostUpdate),
        ));
        app.add_message::<WindowEvent>()
            .init_resource::<UiStack>()
            .init_resource::<UiScale>();
        app.insert_resource(CampaignScriptDialogs {
            blocked: true,
            ready: true,
            ..default()
        });
        crate::button_actions::install(&mut app);
        app.add_observer(capture_activate)
            .add_observer(capture_click);
        app.add_systems(
            PostUpdate,
            bevy::ui::update::propagate_ui_target_cameras
                .before(PropagateSet::<ComputedUiTargetCamera>::default())
                .before(PropagateSet::<ComputedUiRenderTargetInfo>::default()),
        );
        let window = app
            .world_mut()
            .spawn((Window::default(), PrimaryWindow))
            .id();
        let camera = app
            .world_mut()
            .spawn((
                Camera {
                    computed: ComputedCameraValues {
                        target_info: Some(RenderTargetInfo {
                            physical_size: UVec2::new(400, 300),
                            scale_factor: 1.,
                        }),
                        ..default()
                    },
                    ..default()
                },
                RenderTarget::Window(WindowRef::Entity(window)),
            ))
            .id();
        let mut spawn_node = |size, center, layer, pickable| {
            app.world_mut()
                .spawn((
                    Node::default(),
                    ComputedNode { size, ..default() },
                    UiGlobalTransform::from_translation(center),
                    InheritedVisibility::VISIBLE,
                    UiTargetCamera(camera),
                    GlobalZIndex(layer),
                    pickable,
                ))
                .id()
        };
        let hud = spawn_node(Vec2::splat(50.), Vec2::splat(40.), 0, Pickable::default());
        let barrier = spawn_node(
            Vec2::new(400., 300.),
            Vec2::new(200., 150.),
            peacecraft3::campaign_presentation::CAMPAIGN_CONTROL_BARRIER_LAYER,
            Pickable {
                should_block_lower: true,
                is_hoverable: false,
            },
        );
        let button = spawn_node(
            Vec2::new(120., 50.),
            Vec2::new(180., 90.),
            DIALOG_LAYER,
            Pickable::default(),
        );
        let decorative = spawn_node(
            Vec2::new(120., 50.),
            Vec2::new(180., 90.),
            DIALOG_LAYER,
            Pickable::IGNORE,
        );
        let id = HandleId::new(std::num::NonZeroU64::new(7).unwrap());
        app.world_mut()
            .entity_mut(hud)
            .insert(bevy::ui_widgets::Button);
        app.world_mut()
            .entity_mut(button)
            .insert((bevy::ui_widgets::Button, Activate(id)))
            .add_child(decorative);
        let mut roots = [hud, barrier, button];
        roots.sort_by_key(|e| app.world().get::<GlobalZIndex>(*e).unwrap().0);
        assert_eq!(roots, [hud, barrier, button]);
        {
            let mut stack = app.world_mut().resource_mut::<UiStack>();
            stack.uinodes = vec![hud, barrier, button, decorative];
            stack.partition = std::iter::once(0..4).collect();
        }
        app.update();
        app.update();
        let move_to = |app: &mut App, position| {
            app.world_mut()
                .write_message(WindowEvent::CursorMoved(CursorMoved {
                    window,
                    position,
                    delta: None,
                }));
            app.update();
        };
        let mouse = |app: &mut App, state| {
            app.world_mut()
                .write_message(WindowEvent::MouseButtonInput(MouseButtonInput {
                    window,
                    button: MouseButton::Left,
                    state,
                }));
            app.update();
        };
        move_to(&mut app, Vec2::splat(40.));
        mouse(&mut app, ButtonState::Pressed);
        assert!(!app.world().entity(hud).contains::<bevy::ui::Pressed>());
        mouse(&mut app, ButtonState::Released);
        assert!(
            app.world()
                .resource::<crate::button_actions::ButtonActions>()
                .entities
                .is_empty()
        );
        assert!(
            app.world()
                .resource::<CampaignScriptDialogs>()
                .pending
                .is_none()
        );
        move_to(&mut app, Vec2::new(180., 90.));
        mouse(&mut app, ButtonState::Pressed);
        assert!(app.world().entity(button).contains::<bevy::ui::Pressed>());
        mouse(&mut app, ButtonState::Released);
        assert!(!app.world().entity(button).contains::<bevy::ui::Pressed>());
        assert_eq!(
            app.world().resource::<CampaignScriptDialogs>().pending,
            Some(id)
        );
        assert_eq!(
            app.world()
                .resource::<crate::button_actions::ButtonActions>()
                .entities,
            [button]
        );
    }
    #[test]
    fn formatting_only_caption_has_no_text_to_shape() {
        for message in ["|cffff0000|r", "|r|cffffffff|R", "|C01020304|c11223344|r"] {
            assert!(caption_is_formatting_only(Some(message)).unwrap());
        }
        for message in [
            None,
            Some(""),
            Some("|cffff0000text|r"),
            Some("|n"),
            Some(" "),
        ] {
            assert!(!caption_is_formatting_only(message).unwrap());
        }
        assert!(caption_is_formatting_only(Some("|czzz")).is_err());
        assert!(caption_is_formatting_only(Some("^^unsupported^^")).is_err());
    }
    #[test]
    fn stale_caption_probes_cannot_invalidate_the_displayed_geometry() {
        use bevy::ecs::system::RunSystemOnce;
        let current = std::sync::Arc::new(());
        let old = std::sync::Arc::new(());
        let dialog = HandleId::new(std::num::NonZeroU64::new(7).unwrap());
        for stale in ["root", "session", "revision", "none"] {
            let mut app = App::new();
            let root = app.world_mut().spawn(Visibility::Inherited).id();
            app.insert_resource(CampaignScriptDialogs {
                witness: Some(std::sync::Arc::downgrade(&current)),
                root: Some(root),
                revision: Some(3),
                blocked: true,
                ready: true,
                ..default()
            });
            app.world_mut().spawn((
                CaptionProbe {
                    root: if stale == "root" {
                        Entity::PLACEHOLDER
                    } else {
                        root
                    },
                    witness: std::sync::Arc::downgrade(if stale == "session" {
                        &old
                    } else {
                        &current
                    }),
                    revision: if stale == "revision" { 2 } else { 3 },
                    dialog,
                },
                bevy::text::TextLayoutInfo::default(),
                bevy::ui::ComputedUiRenderTargetInfo::default(),
                ComputedNode::default(),
            ));
            app.world_mut().run_system_once(measure_caption).unwrap();
            let state = app.world().resource::<CampaignScriptDialogs>();
            assert_eq!(
                state.last_error.is_some(),
                stale == "none",
                "only the current probe reaches extent validation"
            );
            assert_eq!(state.revision, Some(3));
            assert!(state.ready && state.input_blocked());
            assert!(state.caption.is_none());
            assert_eq!(
                *app.world().get::<Visibility>(root).unwrap(),
                Visibility::Inherited
            );
        }
    }
    #[test]
    fn stale_displayed_session_discards_pending_input_before_handle_matching() {
        let old = std::sync::Arc::new(());
        let new = std::sync::Arc::new(());
        let button = HandleId::new(std::num::NonZeroU64::new(7).unwrap());
        let mut state = CampaignScriptDialogs {
            witness: Some(std::sync::Arc::downgrade(&old)),
            blocked: true,
            pending: Some(button),
            ..default()
        };
        assert!(take_displayed_input(&mut state, Some(std::sync::Arc::downgrade(&new))).is_none());
        assert!(
            state.pending.is_none(),
            "old UI input cannot survive into the replacement session"
        );
        state.pending = Some(button);
        assert!(take_displayed_input(&mut state, None).is_none());
        assert!(state.pending.is_none());
        state.pending = Some(button);
        let (_, pending) =
            take_displayed_input(&mut state, Some(std::sync::Arc::downgrade(&old))).unwrap();
        assert_eq!(pending, Some(button));
        assert!(state.pending.is_none());
    }
    #[test]
    fn completed_dialog_click_still_blocks_same_frame_gameplay_input() {
        let state = CampaignScriptDialogs {
            blocked: false,
            consumed: true,
            ..default()
        };
        assert!(state.input_blocked());
        let state = CampaignScriptDialogs {
            blocked: false,
            consumed: false,
            ..default()
        };
        assert!(!state.input_blocked());
    }
    #[test]
    #[ignore = "requires PEACECRAFT3_DIALOGUE_FDF_DIR with explicitly supplied original ScriptDialog.fdf/EscMenuTemplates.fdf"]
    fn local_original_script_dialog_uses_declared_control_dimensions() {
        let dir = std::path::PathBuf::from(
            std::env::var_os("PEACECRAFT3_DIALOGUE_FDF_DIR")
                .expect("explicit original FDF directory"),
        );
        let mut fdf =
            Fdf::parse(&std::fs::read_to_string(dir.join("EscMenuTemplates.fdf")).unwrap())
                .unwrap();
        fdf.parse_into(&std::fs::read_to_string(dir.join("ScriptDialog.fdf")).unwrap())
            .unwrap();
        let style = menu_text::render_style(&fdf, "ScriptDialogText").unwrap();
        let art = glue_menu::backdrop(&fdf, "ScriptDialogBackdrop")
            .unwrap()
            .unwrap();
        let l = dimensions(&fdf, &art).unwrap();
        assert!(style.height > 0.);
        assert_eq!(l.width, fdf.width("ScriptDialog").unwrap());
        assert_eq!(l.button_width, fdf.width("ScriptDialogButton").unwrap());
        assert_eq!(l.button_height, fdf.height("ScriptDialogButton").unwrap());
        assert_eq!(l.inset, art.inset.unwrap());
    }
    #[test]
    fn hotkeys_preserve_executed_codes_and_match_logical_letter_case() {
        assert!(hotkey_matches(82, 114));
        assert!(hotkey_matches(13, 13));
        assert!(!hotkey_matches(82, 83));
        assert!(!hotkey_matches(-1, 82));
    }
    #[test]
    fn script_modal_uses_source_dimensions_and_insets_without_copied_spacing() {
        let fdf=Fdf::parse("Frame \"DIALOG\" \"ScriptDialog\" {\nWidth 0.4,\nHeight 0.1,\n}\nFrame \"TEXT\" \"ScriptDialogText\" {\nSetPoint TOP,\"ScriptDialog\",TOP,0,-0.02,\n}\nFrame \"GLUETEXTBUTTON\" \"ScriptDialogButton\" {\nWidth 0.3,\nHeight 0.04,\n}\n").unwrap();
        let art = Backdrop {
            background: None,
            tile: false,
            background_size: None,
            mirrored: false,
            inset: Some(0.015),
            edge: None,
        };
        let l = dimensions(&fdf, &art).unwrap();
        assert_eq!(l.width, 0.4);
        assert_eq!(l.button_height, 0.04);
        assert_eq!(l.inset, 0.015);
        let invalid = Fdf::parse("Frame \"DIALOG\" \"ScriptDialog\" {\nWidth NaN,\nHeight 0.1,\n}\nFrame \"GLUETEXTBUTTON\" \"ScriptDialogButton\" {\nWidth 0.3,\nHeight 0.04,\n}\n").unwrap();
        assert!(dimensions(&invalid, &art).is_err());
    }
}

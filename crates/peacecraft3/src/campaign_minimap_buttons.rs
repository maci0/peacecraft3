//! Minimap filter buttons and the idle-worker button under the console parent.
//!
//! Geometry, art keys, tooltip keys and initial option values come from
//! [`pcx_play::minimap_buttons_layout`]. Presses change per-session filter state:
//! the terrain button hides the minimap terrain colours; the signal, ally-colour,
//! creep and formation buttons change their art and state only, since the minimap
//! has no creep indicators, ally-mode colours or signals yet and unit movement has no
//! formation mode. The idle-worker count and selection follow
//! [`pcx_play::playercolor_host::ObjectHost::idle_workers`], a recreation choice.
use crate::campaign_hud::upper_buttons::{image_sampler, read, skin_file, text};
use bevy::{
    ecs::system::SystemParam, picking::events::PointerPress, prelude::*, ui::Pressed,
    ui_widgets::Button,
};
use pcx_play::{
    command_layout,
    inventory_layout::Rect,
    map_startup_host::MapStartupHost,
    minimap_buttons_layout::{Control, Rules},
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
use std::sync::{Arc, Weak};

const SKIN: &str = r"UI\war3skins.txt";
const CONSOLE: &str = r"UI\FrameDef\UI\ConsoleUI.fdf";
const GLOBALS: &str = r"UI\FrameDef\GlobalStrings.fdf";
const MISC_UI: &str = r"UI\MiscUI.txt";
// Application read ceilings, not authored sizes.
const IMAGE_BYTES: u64 = 4 << 20;
const FONT_BYTES: u64 = 16 << 20;
const DECODED_BYTES: usize = 16 << 20;

/// Option value of each button, parallel to `Rules::buttons`.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Filters(Vec<u8>);

impl Filters {
    fn initial(rules: &Rules) -> Self {
        Self(rules.buttons.iter().map(|b| b.initial).collect())
    }
    /// A press adds one and wraps past the last art set: toggles flip, the ally colour
    /// mode cycles through its three values, the signal button has one value.
    fn press(&mut self, rules: &Rules, index: usize) {
        if let (Some(value), Some(button)) = (self.0.get_mut(index), rules.buttons.get(index)) {
            *value = ((usize::from(*value) + 1) % button.art.len()) as u8;
        }
    }
    fn terrain_visible(&self, rules: &Rules) -> bool {
        rules
            .buttons
            .iter()
            .zip(&self.0)
            .find(|(b, _)| b.control == Control::Terrain)
            .is_none_or(|(_, value)| *value != 0)
    }
}

struct Tip {
    name: String,
    ubertip: String,
}

struct ButtonArt {
    /// Enabled and pushed handles per option value.
    states: Vec<[Handle<Image>; 2]>,
    tip: Result<Tip, String>,
    rect: Rect,
}

struct Source {
    rules: Rules,
    console: String,
    art: Vec<ButtonArt>,
    idle_icon: Handle<Image>,
    idle_tip: Result<Tip, String>,
    badge: command_layout::RankBadge,
    badge_art: Handle<Image>,
    badge_font: Handle<Font>,
    badge_height: f32,
}

#[derive(Component)]
struct FilterButton {
    index: usize,
    session: Weak<()>,
}

#[derive(Component)]
struct IdleButton {
    session: Weak<()>,
}

#[derive(Component)]
struct IdleCount;

#[derive(Resource, Default)]
pub(crate) struct MinimapButtons {
    witness: Option<Weak<()>>,
    source: Option<Source>,
    filters: Option<Filters>,
    attempted: bool,
    root: Option<Entity>,
    parent: Option<Entity>,
    buttons: Vec<Entity>,
    idle_button: Option<Entity>,
    idle_count: Option<Entity>,
    idle: Vec<HandleId>,
    /// Position in `idle` the next press selects.
    next_idle: usize,
    pending_pointer: bool,
    pub last_error: Option<String>,
}

impl MinimapButtons {
    /// Whether the terrain button shows the minimap terrain. `None` before the buttons load.
    pub(crate) fn terrain_visible(&self) -> Option<bool> {
        Some(
            self.filters
                .as_ref()?
                .terrain_visible(&self.source.as_ref()?.rules),
        )
    }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct MinimapButtonsSet;
#[derive(SystemParam)]
struct ButtonAssets<'w> {
    images: ResMut<'w, Assets<Image>>,
    fonts: ResMut<'w, Assets<Font>>,
}

#[derive(SystemParam)]
struct Inputs<'w, 's> {
    requests: Res<'w, MapRequests>,
    sources: Res<'w, SelectedMapSources>,
    camera: Res<'w, CampaignCamera>,
    prepared: Res<'w, crate::hud_render::HudPrepare>,
    launcher: Option<Res<'w, crate::launch_gui::Launcher>>,
    frames: Query<'w, 's, (Entity, &'static crate::hud_render::HudSourceAttachment)>,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<MinimapButtons>()
        .add_observer(capture_press)
        .add_systems(
            Update,
            (capture, project, publish_terrain, show_state)
                .chain()
                .in_set(MinimapButtonsSet)
                .after(crate::hud_render::HudRenderSet)
                .after(CampaignCameraSet)
                .after(crate::campaign_hud::CampaignHudSet)
                .before(CampaignPresentationSet),
        )
        .add_systems(
            Update,
            activate
                .after(MinimapButtonsSet)
                .before(crate::button_actions::clear),
        );
}

type PressTarget = Or<(With<FilterButton>, With<IdleButton>)>;

fn capture_press(
    event: On<PointerPress>,
    buttons: Query<(), PressTarget>,
    mut state: ResMut<MinimapButtons>,
    presentation: Option<Res<peacecraft3::campaign_presentation::CampaignPresentation>>,
) {
    if buttons.contains(event.entity) && crate::button_actions::input_allowed(presentation) {
        state.pending_pointer = true;
    }
}

fn capture(mut state: ResMut<MinimapButtons>, mut hud: ResMut<crate::campaign_hud::CampaignHud>) {
    hud.pointer_consumed |= std::mem::take(&mut state.pending_pointer);
}

fn same_session(a: Option<&Weak<()>>, b: Option<&Weak<()>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Weak::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

fn viewport_node(rect: Rect, viewport: [f32; 2]) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Percent(rect.x / viewport[0] * 100.),
        bottom: Val::Percent(rect.y / viewport[1] * 100.),
        width: Val::Percent(rect.width / viewport[0] * 100.),
        height: Val::Percent(rect.height / viewport[1] * 100.),
        ..default()
    }
}

fn root_node(display: Display) -> Node {
    Node {
        position_type: PositionType::Absolute,
        width: Val::Percent(100.),
        height: Val::Percent(100.),
        display,
        ..default()
    }
}

fn load_image(
    reader: &pcx_assets::Resolver,
    path: &str,
    images: &mut Assets<Image>,
    receipts: &mut Vec<AssetReceipt>,
) -> Result<Handle<Image>, String> {
    let bytes = read(reader, path, IMAGE_BYTES, receipts)?;
    let texture = pcx_gfx::load_image_bounded(&bytes, path, DECODED_BYTES)
        .map_err(|error| format!("{path}: {error}"))?;
    let mut decoded = pcx_gfx::viewer::rgba_image(&texture);
    decoded.sampler = bevy::image::ImageSampler::Descriptor(image_sampler(false));
    Ok(images.add(decoded))
}

fn tip(globals: &str, title: &str, description: &str) -> Result<Tip, String> {
    let lookup = |key: &str| {
        crate::menu_text::global_string(globals, key)
            .ok_or_else(|| format!("missing global string {key}"))
    };
    Ok(Tip {
        name: lookup(title)?,
        ubertip: lookup(description)?,
    })
}

/// Reads the layout, skin art, tooltip strings and badge style once per session.
fn load(
    reader: &pcx_assets::Resolver,
    install: Option<&std::path::Path>,
    race: Option<String>,
    images: &mut Assets<Image>,
    fonts: &mut Assets<Font>,
) -> Result<Source, String> {
    let (rules, command) = crate::menu_lighting::with_client_image(install, |label, image| {
        let identity = pcx_play::campaign_inputs::client_identity(image, label);
        Ok((
            Rules::reviewed_legacy(image, identity.clone())?,
            command_layout::Rules::reviewed_legacy(image, identity)?,
        ))
    })?;
    let mut receipts = Vec::new();
    let skin = crate::menu_skin::SkinTable::parse(&text(reader, SKIN, &mut receipts)?)
        .map_err(|error| error.to_string())?;
    let sections = crate::campaign_hud::race_sections(&skin, race)?;
    let globals = text(reader, GLOBALS, &mut receipts)?;
    let console = skin_file(&skin, &sections, &command.console_key)?;
    let mut load_key = |key: &str| -> Result<Handle<Image>, String> {
        let path = skin_file(&skin, &sections, key)?;
        load_image(reader, &path, images, &mut receipts)
    };
    let mut art = Vec::new();
    for button in &rules.buttons {
        let states = button
            .art
            .iter()
            .map(|state| Ok([load_key(&state.enabled)?, load_key(&state.pushed)?]))
            .collect::<Result<Vec<_>, String>>()?;
        art.push(ButtonArt {
            states,
            tip: tip(&globals, &button.tip_key, &button.uber_key),
            rect: button.rect,
        });
    }
    let idle_icon = load_key(&rules.idle.art_key)?;
    let badge_art = load_key(&command.badge.art_key)?;
    let idle_tip = tip(&globals, &rules.idle.tip_key, &rules.idle.desc_key);
    let font_path = skin_file(&skin, &sections, &command.badge.font_key)?;
    let font_bytes = read(reader, &font_path, FONT_BYTES, &mut receipts)?;
    if !crate::hud_render::font_registers(&font_bytes) {
        return Err(format!("idle count font is unreadable: {font_path}"));
    }
    let profile = crate::menu_skin::SkinTable::parse(&text(reader, MISC_UI, &mut receipts)?)
        .map_err(|error| error.to_string())?;
    let heights = profile
        .sections()
        .iter()
        .filter(|s| s.name.eq_ignore_ascii_case(&command.badge.section))
        .flat_map(|s| &s.entries)
        .filter(|(key, _)| key.eq_ignore_ascii_case(&command.badge.height_key))
        .map(|(_, value)| value)
        .collect::<Vec<_>>();
    let [height] = heights.as_slice() else {
        return Err("idle count font height missing or ambiguous".into());
    };
    let badge_height = height
        .parse::<f32>()
        .ok()
        .filter(|h| h.is_finite() && *h > 0.)
        .ok_or("idle count font height is invalid")?;
    Ok(Source {
        rules,
        console,
        art,
        idle_icon,
        idle_tip,
        badge: command.badge,
        badge_art,
        badge_font: fonts.add(Font::from_bytes(font_bytes)),
        badge_height,
    })
}

fn reset(commands: &mut Commands, state: &mut MinimapButtons, witness: Option<Weak<()>>) {
    if let Some(root) = state.root {
        commands.entity(root).try_despawn();
    }
    *state = MinimapButtons {
        witness,
        ..default()
    };
}

fn project(
    mut commands: Commands,
    mut state: ResMut<MinimapButtons>,
    inputs: Inputs,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
    art: ButtonAssets,
    mut nodes: Query<&mut Node>,
    mut texts: Query<&mut Text, With<IdleCount>>,
) {
    let witness = session.session_witness();
    let ButtonAssets {
        mut images,
        mut fonts,
    } = art;
    if !same_session(state.witness.as_ref(), witness.as_ref()) {
        reset(&mut commands, &mut state, witness.clone());
    }
    let (Some(witness), Some(context)) = (witness, session.context().cloned()) else {
        return;
    };
    let Some(bindings) = inputs.requests.bindings.as_ref().filter(|bindings| {
        inputs
            .requests
            .published_receipts
            .as_ref()
            .is_some_and(|receipts| Arc::ptr_eq(receipts, &bindings.receipts))
            && inputs
                .requests
                .admission
                .shared_context()
                .is_some_and(|admitted| Arc::ptr_eq(&admitted, &context))
    }) else {
        return;
    };
    if !inputs.prepared.mounted_for(&context, bindings.revision) {
        return;
    }
    let Some(reader) = inputs.sources.for_receipts(&bindings.receipts) else {
        return;
    };
    let Some(host) = session.host() else { return };
    if !state.attempted {
        state.attempted = true;
        let install = inputs
            .launcher
            .as_deref()
            .and_then(crate::launch_gui::selected_install);
        let race = host
            .objects()
            .local_race_preference_constant()
            .map_err(|error| error.to_string());
        let loaded =
            race.and_then(|race| load(&reader, install.as_deref(), race, &mut images, &mut fonts));
        match loaded {
            Ok(source) => {
                state.filters = Some(Filters::initial(&source.rules));
                state.source = Some(source);
            }
            Err(error) => {
                warn!("minimap buttons: {error}");
                state.last_error = Some(error);
                return;
            }
        }
    }
    let Some(source) = state.source.as_ref() else {
        return;
    };
    let mut parents = inputs
        .frames
        .iter()
        .filter(|(_, frame)| {
            frame.frame == source.console
                && frame.source_member == CONSOLE
                && frame.binding_revision == bindings.revision
                && frame.receipt_path == bindings.receipts.map.path.as_str()
        })
        .map(|(entity, _)| entity);
    let (Some(parent), None) = (parents.next(), parents.next()) else {
        state.last_error = Some("minimap buttons await one mounted console parent".into());
        return;
    };
    let rebuilt =
        state.parent != Some(parent) || state.root.is_none_or(|root| nodes.get(root).is_err());
    if rebuilt {
        let source = state.source.as_ref().expect("loaded above");
        let (root, buttons, idle_button, idle_count) =
            mount(&mut commands, parent, source, &witness);
        if let Some(old) = state.root {
            commands.entity(old).try_despawn();
        }
        state.root = Some(root);
        state.buttons = buttons;
        state.idle_button = Some(idle_button);
        state.idle_count = Some(idle_count);
        state.parent = Some(parent);
    }
    let idle = host
        .objects()
        .local_player_number()
        .map_or(Ok(Vec::new()), |number| host.objects().idle_workers(number));
    match idle {
        Ok(list) => {
            state.idle = list;
            state.last_error = None;
        }
        Err(error) => {
            state.idle.clear();
            state.last_error = Some(format!("idle workers: {error}"));
        }
    }
    if state.next_idle >= state.idle.len() {
        state.next_idle = 0;
    }
    let display = if inputs.camera.active() {
        Display::Flex
    } else {
        Display::None
    };
    if let Some(root) = state.root
        && nodes.get(root).is_ok_and(|node| node.display != display)
    {
        commands.entity(root).insert(root_node(display));
    }
    if let Some(button) = state.idle_button
        && let Ok(mut node) = nodes.get_mut(button)
    {
        let display = if state.idle.is_empty() {
            Display::None
        } else {
            Display::Flex
        };
        if node.display != display {
            node.display = display;
        }
    }
    if let Some(entity) = state.idle_count
        && let Ok(mut label) = texts.get_mut(entity)
    {
        let count = state.idle.len().to_string();
        if label.0 != count {
            label.0 = count;
        }
    }
}

/// Spawns the root, the five filter buttons and the idle-worker button under `parent`.
fn mount(
    commands: &mut Commands,
    parent: Entity,
    source: &Source,
    witness: &Weak<()>,
) -> (Entity, Vec<Entity>, Entity, Entity) {
    let viewport = source.rules.viewport;
    let root = commands
        .spawn((
            root_node(Display::Flex),
            CampaignInterface,
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(parent).add_child(root);
    let mut buttons = Vec::new();
    for (index, art) in source.art.iter().enumerate() {
        let initial = source.rules.buttons[index].initial;
        let mut button = commands.spawn((
            viewport_node(art.rect, viewport),
            ImageNode::new(art.states[usize::from(initial)][0].clone())
                .with_mode(bevy::ui::widget::NodeImageMode::Stretch),
            Button,
            bevy::picking::hover::Hovered::default(),
            crate::campaign_inventory::TooltipAnchor {
                rect: art.rect,
                viewport,
            },
            FilterButton {
                index,
                session: witness.clone(),
            },
            ChildOf(root),
        ));
        match &art.tip {
            Ok(tip) => button.insert(crate::campaign_inventory::ControlTooltip {
                name: tip.name.clone(),
                ubertip: tip.ubertip.clone(),
            }),
            Err(error) => button.insert(crate::campaign_inventory::TooltipFailure(error.clone())),
        };
        buttons.push(button.id());
    }
    let idle = &source.rules.idle;
    let mut node = viewport_node(idle.rect, viewport);
    node.display = Display::None;
    let mut button = commands.spawn((
        node,
        ImageNode::new(source.idle_icon.clone())
            .with_mode(bevy::ui::widget::NodeImageMode::Stretch),
        Button,
        bevy::picking::hover::Hovered::default(),
        crate::campaign_inventory::TooltipAnchor {
            rect: idle.rect,
            viewport,
        },
        IdleButton {
            session: witness.clone(),
        },
        ChildOf(root),
    ));
    match &source.idle_tip {
        Ok(tip) => button.insert(crate::campaign_inventory::ControlTooltip {
            name: tip.name.clone(),
            ubertip: tip.ubertip.clone(),
        }),
        Err(error) => button.insert(crate::campaign_inventory::TooltipFailure(error.clone())),
    };
    let idle_button = button.id();
    let count = spawn_count(commands, idle_button, source);
    (root, buttons, idle_button, count)
}

/// The count overlay at the button's bottom-right, styled like the command-button badge.
fn spawn_count(commands: &mut Commands, button: Entity, source: &Source) -> Entity {
    let badge = &source.badge;
    let rect = source.rules.idle.rect;
    let [a, r, g, b] = badge.colour.to_be_bytes();
    let plate = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Percent(-badge.offset[0] / rect.width * 100.),
                bottom: Val::Percent(badge.offset[1] / rect.height * 100.),
                width: Val::Percent(badge.size[0] / rect.width * 100.),
                height: Val::Percent(badge.size[1] / rect.height * 100.),
                ..default()
            },
            ImageNode::new(source.badge_art.clone())
                .with_mode(bevy::ui::widget::NodeImageMode::Stretch),
            Pickable::IGNORE,
            ChildOf(button),
        ))
        .id();
    let holder = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(badge.text_offset[0] / badge.size[0] * 100.),
                bottom: Val::Percent(badge.text_offset[1] / badge.size[1] * 100.),
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(plate),
        ))
        .id();
    commands
        .spawn((
            Text::new("0"),
            TextFont {
                font: source.badge_font.clone().into(),
                font_size: bevy::text::FontSize::Vh(
                    source.badge_height / source.rules.viewport[1] * 100.,
                ),
                ..default()
            },
            TextColor(Color::srgba_u8(r, g, b, a)),
            TextLayout::new(bevy::text::Justify::Center, bevy::text::LineBreak::NoWrap),
            bevy::text::LineHeight::RelativeToFont(1.0),
            Pickable::IGNORE,
            IdleCount,
            ChildOf(holder),
        ))
        .id()
}

/// Hands the terrain button's state to the minimap.
fn publish_terrain(
    state: Res<MinimapButtons>,
    mut minimap: ResMut<crate::campaign_minimap::CampaignMinimap>,
) {
    if let Some(visible) = state.terrain_visible() {
        minimap.set_terrain_hidden(!visible);
    }
}

fn show_state(
    state: Res<MinimapButtons>,
    mut buttons: Query<(&FilterButton, Has<Pressed>, &mut ImageNode)>,
) {
    let (Some(source), Some(filters)) = (state.source.as_ref(), state.filters.as_ref()) else {
        return;
    };
    for (button, pressed, mut image) in &mut buttons {
        let Some(states) = source.art.get(button.index).map(|art| &art.states) else {
            continue;
        };
        let Some(value) = filters.0.get(button.index) else {
            continue;
        };
        let Some(pair) = states.get(usize::from(*value)) else {
            continue;
        };
        let handle = &pair[usize::from(pressed)];
        if image.image != *handle {
            image.image = handle.clone();
        }
    }
}

#[derive(SystemParam)]
struct Controls<'w> {
    view: Res<'w, CampaignCamera>,
    hud: Res<'w, crate::campaign_hud::CampaignHud>,
    dialogs: Res<'w, crate::campaign_script_dialogs::CampaignScriptDialogs>,
}

/// Filter presses change the stored option value; an idle-worker press selects the next
/// idle worker.
fn activate(
    actions: Res<crate::button_actions::ButtonActions>,
    filter_buttons: Query<&FilterButton, Without<bevy::ui::InteractionDisabled>>,
    idle_buttons: Query<&IdleButton, Without<bevy::ui::InteractionDisabled>>,
    controls: Controls,
    mut state: ResMut<MinimapButtons>,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
) {
    let witness = session.session_witness();
    let Some(host) = session.host() else { return };
    if !crate::campaign_input::commands_available(
        &controls.view,
        &controls.hud,
        &controls.dialogs,
        host,
    ) {
        return;
    }
    let current = |recorded: &Weak<()>| witness.as_ref().is_some_and(|w| Weak::ptr_eq(w, recorded));
    for entity in &actions.entities {
        if let Ok(button) = filter_buttons.get(*entity)
            && current(&button.session)
        {
            let state = &mut *state;
            if let (Some(source), Some(filters)) = (&state.source, &mut state.filters) {
                filters.press(&source.rules, button.index);
            }
        } else if let Ok(button) = idle_buttons.get(*entity)
            && current(&button.session)
            && let Some(unit) = state.idle.get(state.next_idle).copied()
        {
            host.objects_mut().select_units(&[unit], true);
            state.next_idle = (state.next_idle + 1) % state.idle.len();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pcx_play::minimap_buttons_layout::{Button as LayoutButton, Idle, StateArt};

    fn art(label: &str) -> StateArt {
        StateArt {
            enabled: format!("{label}E"),
            pushed: format!("{label}P"),
            disabled: format!("{label}D"),
        }
    }

    fn rules() -> Rules {
        let button = |control, count: usize, initial| LayoutButton {
            control,
            rect: Rect {
                x: 0.,
                y: 0.,
                width: 0.1,
                height: 0.1,
            },
            tip_key: String::new(),
            uber_key: String::new(),
            art: (0..count).map(|i| art(&i.to_string())).collect(),
            initial,
        };
        Rules {
            origin: pcx_play::scripts::AssetIdentity {
                source: "fixture".into(),
                edition: "fixture".into(),
                version: "fixture".into(),
            },
            image_sha256: [0; 32],
            viewport: [0.8, 0.6],
            buttons: vec![
                button(Control::Signal, 1, 0),
                button(Control::Terrain, 2, 1),
                button(Control::Ally, 3, 1),
            ],
            idle: Idle {
                rect: Rect {
                    x: 0.,
                    y: 0.,
                    width: 0.1,
                    height: 0.1,
                },
                art_key: String::new(),
                tip_key: String::new(),
                desc_key: String::new(),
            },
        }
    }

    #[test]
    fn presses_toggle_cycle_or_ignore_by_art_count() {
        let rules = rules();
        let mut filters = Filters::initial(&rules);
        assert_eq!(filters.0, [0, 1, 1]);
        assert!(filters.terrain_visible(&rules));
        filters.press(&rules, 0);
        assert_eq!(filters.0[0], 0, "a one-value button keeps its state");
        filters.press(&rules, 1);
        assert_eq!(filters.0[1], 0);
        assert!(!filters.terrain_visible(&rules));
        filters.press(&rules, 1);
        assert!(filters.terrain_visible(&rules));
        for expected in [2, 0, 1] {
            filters.press(&rules, 2);
            assert_eq!(filters.0[2], expected);
        }
        filters.press(&rules, 9);
        assert_eq!(filters.0, [0, 1, 1]);
    }

    #[cfg(feature = "test-fixtures")]
    #[test]
    #[ignore = "requires PCX_NATIVE_CHAPTER_* selecting a chapter, and PCX_WAR3_EXE"]
    fn selected_chapter_mounts_the_buttons_and_applies_presses() {
        let mut app = crate::native_campaign::tests::started_app();
        let camera = app.world_mut().spawn_empty().id();
        app.insert_resource(CampaignCamera::active_for_test(camera))
            .init_resource::<Assets<Image>>()
            .init_resource::<Assets<Font>>()
            .init_resource::<Assets<crate::glue_menu::BackdropImage>>()
            .init_resource::<Assets<crate::glue_menu::AdditiveImage>>()
            .init_resource::<crate::campaign_script_dialogs::CampaignScriptDialogs>()
            .init_resource::<crate::campaign_minimap::CampaignMinimap>()
            .init_resource::<crate::campaign_messages::MessageHistory>()
            .init_resource::<ButtonInput<KeyCode>>();
        crate::campaign_hud::install(&mut app);
        crate::hud_render::install(&mut app);
        install(&mut app);
        for _ in 0..6 {
            app.update();
        }
        let (root, parent, buttons, idle, count, initial) = {
            let state = app.world().resource::<MinimapButtons>();
            assert!(state.last_error.is_none(), "{:?}", state.last_error);
            let source = state.source.as_ref().expect("loaded buttons");
            let root = state.root.expect("button root");
            let parent = state.parent.expect("console parent");
            assert_eq!(app.world().get::<ChildOf>(root).unwrap().parent(), parent);
            assert_eq!(state.buttons.len(), 5);
            for (entity, art) in state.buttons.iter().zip(&source.art) {
                let tooltip = app
                    .world()
                    .get::<crate::campaign_inventory::ControlTooltip>(*entity)
                    .expect("global-string tooltip");
                assert!(!tooltip.name.is_empty() && !tooltip.ubertip.is_empty());
                assert_eq!(
                    app.world().get::<ImageNode>(*entity).unwrap().image,
                    art.states[usize::from(
                        source.rules.buttons
                            [app.world().get::<FilterButton>(*entity).unwrap().index]
                            .initial
                    )][0]
                );
            }
            let count = app
                .world()
                .get::<Text>(state.idle_count.unwrap())
                .unwrap()
                .0
                .clone();
            assert_eq!(count, state.idle.len().to_string());
            (
                root,
                parent,
                state.buttons.clone(),
                state.idle_button.unwrap(),
                count,
                state.filters.clone().unwrap(),
            )
        };
        // The opening script hides the Prologue02 peon until later; it is the only worker.
        assert_eq!(count, "0");
        assert_eq!(
            app.world().get::<Node>(idle).unwrap().display,
            Display::None
        );
        let peon = {
            use pcx_script::{Host, Value};
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let objects = session.host().unwrap().objects_mut();
            let peon = objects
                .executed_units()
                .find(|(_, unit)| &unit.definition.rawcode == b"opeo")
                .map(|(id, _)| id)
                .expect("the chapter's peon");
            objects
                .native("EnableUserControl", &[Value::Bool(true)])
                .unwrap();
            objects
                .native("ShowUnit", &[Value::Handle(peon), Value::Bool(true)])
                .unwrap();
            peon
        };
        app.update();
        let state = app.world().resource::<MinimapButtons>();
        assert_eq!(state.idle, [peon]);
        assert_eq!(
            app.world()
                .get::<Text>(state.idle_count.unwrap())
                .unwrap()
                .0,
            "1"
        );
        assert_ne!(
            app.world().get::<Node>(idle).unwrap().display,
            Display::None
        );
        // A press selects the idle worker.
        app.world_mut()
            .resource_mut::<crate::button_actions::ButtonActions>()
            .entities
            .push(idle);
        app.update();
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        assert_eq!(session.host().unwrap().objects().selection(), [peon]);
        // Pressing the terrain button hides the minimap terrain; a second press shows it.
        let terrain = buttons[1];
        for expected in [false, true] {
            app.world_mut()
                .resource_mut::<crate::button_actions::ButtonActions>()
                .entities
                .push(terrain);
            app.update();
            assert_eq!(
                app.world().resource::<MinimapButtons>().terrain_visible(),
                Some(expected)
            );
        }
        assert_eq!(
            app.world().resource::<MinimapButtons>().filters,
            Some(initial)
        );
        assert_eq!(app.world().resource::<MinimapButtons>().root, Some(root));
        assert_eq!(
            app.world().resource::<MinimapButtons>().parent,
            Some(parent)
        );
    }
}

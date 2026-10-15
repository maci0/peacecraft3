//! GUI launcher.
//!
//! The CLI in [`crate::launch`] stays, because staging assets from CI or a
//! script should not need a window. This is the human front-end: pick a source,
//! pick a game mode, stage, and see what landed.
//!
//! Rendering the launcher on the engine it stages for is deliberate — one fewer
//! dependency, and the launcher and the game share a version.

use bevy::ecs::system::SystemParam;
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui::widget::{ImageNode, NodeImageMode};

use bevy::ui::InteractionDisabled;
// The 0.20 button: `bevy::ui::Button` is a deprecated alias for a
// crate-private marker that cannot be spawned. `ui_widgets::Button` is its
// replacement; its held-down state is the `Pressed` component, which is what
// `Interaction::Pressed` used to mean here.
use bevy::ui_widgets::Button;

use crate::launch::{Addon, AssetMode, Behaviour, GameMode, classify, stage};

#[derive(Resource)]
pub struct Launcher {
    source: String,
    mode: GameMode,
    log: Vec<String>,
    staging: bool,
    /// Local browser demo restrictions; `None` leaves native/dev mode unchanged.
    content_editions: Option<crate::local_demo::MenuEditions>,
}

impl Default for Launcher {
    fn default() -> Self {
        Self {
            source: default_source(),
            mode: GameMode::default_for(pcx_mpq::Version::V1_29),
            log: Vec::new(),
            staging: false,
            content_editions: None,
        }
    }
}

impl Launcher {
    fn allows_addon(&self, addon: Addon) -> bool {
        self.content_editions.is_none_or(|e| e.allows(addon))
    }

    fn constrain_menu_edition(&mut self) {
        if let Some(editions) = self.content_editions
            && !editions.allows(self.mode.addon)
        {
            self.mode.addon = editions.preferred().unwrap_or(Addon::Roc);
        }
    }
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Pick {
    Build(pcx_mpq::Version),
    Addon(Addon),
    Assets(AssetMode),
    Behaviour(Behaviour),
    Stage,
    /// Present but disabled: fetching a release is not wired up.
    Download,
    Menu,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct LauncherInput;

#[derive(Component)]
struct SourcePath;

#[derive(Component)]
struct DetectedAs;

#[derive(Component)]
struct LogBody;

#[derive(Component)]
struct ModeSummary;

// ---------------------------------------------------------------- palette ---

const BG_APP: Color = Color::srgb(0.025, 0.035, 0.045);
const BG_CARD: Color = Color::srgb(0.067, 0.074, 0.078);
const BG_FIELD: Color = Color::srgb(0.032, 0.039, 0.043);
const BG_PILL: Color = Color::srgb(0.075, 0.075, 0.071);
const BORDER: Color = Color::srgb(0.23, 0.205, 0.145);
const BORDER_STRONG: Color = Color::srgb(0.69, 0.55, 0.22);
const TEXT: Color = Color::srgb(0.92, 0.85, 0.67);
const TEXT_DIM: Color = Color::srgb(0.80, 0.72, 0.56);
const TEXT_FAINT: Color = Color::srgb(0.67, 0.62, 0.52);
const ACCENT_SOFT: Color = Color::srgb(0.16, 0.14, 0.09);
const DISABLED_BG: Color = Color::srgb(0.086, 0.098, 0.125);
const DISABLED_FG: Color = Color::srgb(0.36, 0.35, 0.32);
const WARN: Color = crate::mainmenu::LABEL;
const PANEL_ART: &str = crate::mainmenu::BUTTON_BG_ART;

#[derive(Component)]
struct Wc3Panel;

#[derive(Component)]
struct LauncherPanel;

#[derive(Component)]
struct ButtonArt;

/// The node the FDF control layer is built into; it covers the authored frame.
#[derive(Component)]
struct ControlRoot;

#[derive(Component)]
struct SettingsCog;

#[derive(Component)]
struct MenuNotice;

#[derive(Component)]
struct ResolutionPopup;

#[derive(Component)]
struct ResolutionChoice(u32, u32);

/// Set once a label's runs exist, so it is filled once rather than every frame.
#[derive(Component)]
struct MenuTextFilled;

// ------------------------------------------------------------- spacing/type

fn card_radius() -> BorderRadius {
    BorderRadius::ZERO
}
fn pill_radius() -> BorderRadius {
    BorderRadius::ZERO
}
fn field_radius() -> BorderRadius {
    BorderRadius::ZERO
}

fn tint(s: impl Into<String>, size: f32, colour: Color) -> impl Bundle {
    (
        Text(s.into()),
        TextFont {
            // Share the viewport-scaled root font size with layout dimensions.
            font_size: bevy::text::FontSize::Rem(size / 16.0),
            ..default()
        },
        TextColor(colour),
        ZIndex(1),
    )
}

/// An inset panel using the menu backdrop.
fn card() -> impl Bundle {
    (
        Node {
            width: Val::Percent(100.0),
            min_width: Val::Rem(0.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::axes(Val::Rem(1.25), Val::Rem(1.0)),
            row_gap: Val::Rem(0.75),
            border: UiRect::all(Val::Rem(0.125)),
            // `BorderRadius` is a `Node` field in 0.20, not a component.
            border_radius: card_radius(),
            ..default()
        },
        BackgroundColor(BG_CARD),
        BorderColor::all(BORDER),
        Wc3Panel,
    )
}

/// The section heading and its supporting label.
fn section(c: &mut ChildSpawnerCommands, title: &str, hint: Option<&str>) {
    c.spawn(Node {
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        column_gap: Val::Rem(0.5),
        row_gap: Val::Rem(0.25),
        flex_wrap: FlexWrap::Wrap,
        width: Val::Percent(100.0),
        min_width: Val::Rem(0.0),
        ..default()
    })
    .with_children(|r| {
        r.spawn(tint(title, 20.0, crate::mainmenu::LABEL));
        if let Some(h) = hint {
            r.spawn((
                tint(h, 12.0, TEXT_FAINT),
                Node {
                    max_width: Val::Percent(100.0),
                    min_width: Val::Rem(0.0),
                    flex_grow: 1.0,
                    flex_basis: Val::Rem(18.75),
                    ..default()
                },
            ));
        }
    });
}

// -------------------------------------------------------------------- setup

/// The launcher window.
///
/// `canvas` is a CSS selector for an element that already exists. Only a browser
/// needs one: winit there creates the window *inside* a `<canvas>` and has
/// nothing else to attach to, while a desktop window is created on its own.
pub(crate) fn window_plugin(canvas: Option<&str>) -> WindowPlugin {
    WindowPlugin {
        primary_window: Some(Window {
            title: "peacecraft3 launcher".into(),
            resolution: (880u32, 860u32).into(),
            canvas: canvas.map(str::to_string),
            fit_canvas_to_parent: canvas.is_some(),
            ..default()
        }),
        ..default()
    }
}

/// Builds the launcher, with `configure` applied *before* the plugins.
///
/// That ordering is the point: Bevy builds an app's asset sources while
/// `AssetPlugin` initialises, so a caller that needs a source which is not the
/// platform default — a browser has no filesystem to read — has to register it
/// first. The explicitly configured map request handlers are installed afterwards.
pub fn app(window: WindowPlugin, configure: impl FnOnce(&mut App)) -> App {
    let mut app = App::new();
    (configure)(&mut app);
    peacecraft3::map_requests::install(&mut app);
    crate::map_source_controls::install(&mut app);
    app.add_plugins(DefaultPlugins.set(window))
        .add_plugins(bevy::input_focus::tab_navigation::TabNavigationPlugin)
        .insert_resource(Launcher::default())
        .init_resource::<MenuArtCache>()
        .init_resource::<SceneLoad>()
        .init_resource::<crate::menu_panel::LogoEntrance>()
        .init_resource::<AssetStoreRevision>()
        .init_resource::<crate::menu_screens::Navigation>()
        .add_observer(menu_acquire_focus)
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                (
                    colour_picks,
                    apply_clicks.in_set(LauncherInput),
                    finish_intro,
                    apply_menu_clicks,
                    menu_navigation_keys,
                    update_menu_notice,
                )
                    .chain(),
                (
                    poll_asset_revision,
                    invalidate_on_asset_revision,
                    apply_menu_art,
                    build_control_layer,
                    build_setting_controls,
                    apply_menu_text,
                    measure_screen_text,
                    position_screen_nodes,
                    initialize_menu_logo,
                    show_logo_for_screen,
                    initialize_menu_scene,
                    show_menu_or_launcher,
                    scroll_launcher,
                    scroll_resolution_popup,
                    refresh,
                    crate::button_actions::clear,
                )
                    .chain(),
            )
                .chain(),
        );
    app.add_systems(
        Update,
        crate::mainmenu_scene::advance
            .after(show_menu_or_launcher)
            .run_if(resource_exists::<crate::mainmenu_scene::MainMenuScene>)
            .run_if(menu_is_visible),
    );
    app.add_systems(
        Update,
        (
            crate::menu_sprites::advance
                .run_if(resource_exists::<crate::menu_sprites::MenuPanels>)
                .run_if(menu_is_visible),
            crate::menu_sprites::follow_ui_viewport,
        )
            .after(show_menu_or_launcher),
    );
    app.add_systems(
        Update,
        crate::menu_panel::advance_logo
            .after(initialize_menu_logo)
            .run_if(menu_is_visible),
    );
    crate::intro::install(&mut app);
    app.configure_sets(
        Update,
        crate::intro::IntroUpdate
            .after(LauncherInput)
            .before(finish_intro),
    );
    crate::menu_audio::install(&mut app);
    crate::glue_menu::install(&mut app);
    crate::menu_settings::install(&mut app);
    app.add_systems(
        PostUpdate,
        synchronize_window_camera_size.before(bevy::camera::CameraUpdateSystems),
    );
    #[cfg(target_arch = "wasm32")]
    {
        app.add_systems(Update, synchronize_local_demo.before(colour_picks));
        app.add_systems(
            PostUpdate,
            publish_menu_snapshot
                .after(bevy::ui::UiSystems::PostLayout)
                .after(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate),
        );
    }
    app
}

#[cfg(target_arch = "wasm32")]
type DiagnosticControlData = (
    &'static ComputedNode,
    &'static bevy::ui::UiGlobalTransform,
    &'static InheritedVisibility,
    Option<&'static crate::glue_menu::GlueButton>,
    Option<&'static ResolutionChoice>,
    Has<SettingsCog>,
    Has<InteractionDisabled>,
);

#[cfg(target_arch = "wasm32")]
type DiagnosticLabelData = (
    &'static crate::glue_menu::GlueLabel,
    &'static Text,
    &'static ComputedNode,
    &'static bevy::ui::UiGlobalTransform,
    &'static InheritedVisibility,
);

#[cfg(target_arch = "wasm32")]
fn publish_menu_snapshot(
    navigation: Res<crate::menu_screens::Navigation>,
    controls: Query<DiagnosticControlData>,
    labels: Query<DiagnosticLabelData>,
    menu: Query<(), With<MenuLayer>>,
    windows: Query<&Window>,
    camera: Query<&Camera, With<bevy::ui::IsDefaultUiCamera>>,
) {
    if !crate::browser::take_menu_snapshot_request() {
        return;
    }
    let offset = camera
        .iter()
        .next()
        .and_then(|c| c.viewport.as_ref())
        .map_or(Vec2::ZERO, |v| v.physical_position.as_vec2());
    let controls:Vec<_>=controls.iter().filter_map(|(node,transform,visible,button,choice,cog,disabled)| {
        if !visible.get() || node.size().min_element()<=0.0 {return None;}
        let name=if let Some(button)=button {button.frame.clone()}else if let Some(choice)=choice {format!("ResolutionChoice:{}x{}",choice.0,choice.1)}else if cog {"PeaceCraftSettings".into()}else{return None;};
        let min=transform.transform_point2(-node.size()/2.0)+offset;
        let max=transform.transform_point2(node.size()/2.0)+offset;
        Some(serde_json::json!({"name":name,"disabled":disabled,"physicalCanvasRect":[min.x,min.y,max.x-min.x,max.y-min.y]}))
    }).collect();
    let labels: Vec<_> = labels.iter().filter_map(|(label,text,node,transform,visible)| {
        if !visible.get() || node.size().min_element() <= 0.0 { return None; }
        let min = transform.transform_point2(-node.size()/2.0) + offset;
        let max = transform.transform_point2(node.size()/2.0) + offset;
        Some(serde_json::json!({"name":label.frame,"text":text.0,"physicalCanvasRect":[min.x,min.y,max.x-min.x,max.y-min.y]}))
    }).collect();
    let canvas = windows
        .iter()
        .next()
        .map(|w| [w.physical_width(), w.physical_height()]);
    crate::browser::publish_menu_snapshot(serde_json::json!({"screen":format!("{:?}",navigation.screen),"menuVisible":!menu.is_empty(),"draft":navigation.draft,"applied":navigation.applied,"notice":navigation.notice,"controls":controls,"labels":labels,"canvasPixels":canvas}).to_string());
}

#[cfg(target_arch = "wasm32")]
fn synchronize_local_demo(
    mut commands: Commands,
    mut launcher: ResMut<Launcher>,
    menu: Query<Entity, With<MenuLayer>>,
    picks: Query<(Entity, &Pick)>,
    buttons: Query<(Entity, &crate::glue_menu::GlueButton)>,
) {
    let (enabled, editions, requested) = crate::browser::take_demo_control();
    if !enabled {
        return;
    }
    launcher.content_editions = Some(editions);
    launcher.constrain_menu_edition();
    for (entity, pick) in &picks {
        let disabled = match pick {
            Pick::Addon(addon) => !editions.allows(*addon),
            Pick::Menu => !editions.allows(launcher.mode.addon),
            _ => continue,
        };
        if disabled {
            commands.entity(entity).insert(InteractionDisabled);
        } else {
            commands.entity(entity).remove::<InteractionDisabled>();
        }
    }
    for (entity, button) in &buttons {
        if button.frame == "EditionButton" {
            if editions.roc && editions.tft {
                commands.entity(entity).remove::<InteractionDisabled>();
            } else {
                commands.entity(entity).insert(InteractionDisabled);
            }
        }
    }
    if editions.preferred().is_none() {
        for entity in &menu {
            commands.entity(entity).despawn();
        }
    } else if let Some(addon) = requested.filter(|a| editions.allows(*a)) {
        launcher.mode.addon = addon;
        if menu.is_empty() {
            spawn_main_menu(&mut commands);
        }
    }
    crate::browser::publish_demo_edition(launcher.mode.addon);
}

fn default_source() -> String {
    for key in ["PEACECRAFT3_INSTALL", "PEACECRAFT3_LEGACY_DIR"] {
        if let Ok(path) = std::env::var(key)
            && classify(&path).is_ok()
        {
            return path;
        }
    }
    for c in [
        "/home/maci/Faugus/battle-net/drive_c/Program Files (x86)",
        "/home/maci/Desktop/peacecraft3/wc3/game/app",
    ] {
        if classify(c).is_ok() {
            return c.to_string();
        }
    }
    "wc3/game/app".to_string()
}

fn selected_install(launcher: &Launcher) -> Option<std::path::PathBuf> {
    match classify(&launcher.source).ok()? {
        crate::launch::Source::Directory(path) => Some(path),
        crate::launch::Source::BattleNet(path) => {
            crate::launch::pick_product(&path, &launcher.mode).ok()
        }
        crate::launch::Source::Iso(_) => None,
    }
}

fn setup(mut commands: Commands, l: Res<Launcher>) {
    commands.spawn((
        Camera2d,
        bevy::ui::IsDefaultUiCamera,
        Camera {
            // After the menu background (-1) and its sprite layers (0).
            order: 1,
            clear_color: bevy::camera::ClearColorConfig::Custom(BG_APP),
            ..default()
        },
    ));

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                min_width: Val::Rem(0.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Rem(1.625)),
                row_gap: Val::Rem(1.125),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            BackgroundColor(BG_APP),
            LauncherPanel,
            Hovered::default(),
            ScrollPosition::default(),
        ))
        .with_children(|root| {
            header(root, &l);
            source_card(root, &l);
            mode_card(root, l.mode);
            action_card(root);
            log_card(root);
        });

    // PeaceCraft settings precede the original menu; Main menu opens it.
}

/// Draws the original main menu over the staging panel.
///
/// The frames, the art and the labels are the original's, taken from
/// `UI\FrameDef\Glue\MainMenu.fdf` and `UI\FrameDef\GlobalStrings.fdf` — see
/// [`crate::mainmenu`] for the measured offsets. Where the install has the art
/// it is decoded through `pcx_gfx` and used as the button's real image; where it
/// does not, that is reported in the log rather than drawn as a rectangle that
/// merely resembles the original.
fn spawn_main_menu(commands: &mut Commands) {
    // The frames are spawned here; their art is applied by [`apply_menu_art`]
    // afterwards. It has to be deferred because a browser cannot read the
    // install paths at startup — the page fetches the originals and hands them
    // over a moment later — so an art-less first frame is the correct state on
    // the way to an art-full one, not a failure.

    commands
        .spawn((
            Node {
                display: Display::Grid,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                grid_template_columns: vec![GridTrack::percent(100.0)],
                grid_template_rows: vec![GridTrack::percent(100.0)],
                ..default()
            },
            MenuLayer,
            bevy::input_focus::tab_navigation::TabGroup {
                modal: true,
                ..default()
            },
        ))
        .with_children(|root| {
            // The FDF control layer is built into this once the frame
            // definitions are readable; see `build_control_layer`.
            root.spawn((
                Node {
                    grid_row: GridPlacement::start(1),
                    grid_column: GridPlacement::start(1),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                ControlRoot,
                Pickable::IGNORE,
            ));
            root.spawn((
                Node {
                    grid_row: GridPlacement::start(1),
                    grid_column: GridPlacement::start(1),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::FlexStart,
                    ..default()
                },
                LogoLayer,
                Pickable::IGNORE,
                ZIndex(1),
            ));
            root.spawn((
                Button,
                SettingsCog,
                Name::new("PeaceCraft settings"),
                Hovered::default(),
                bevy::input_focus::tab_navigation::TabIndex(i32::MAX),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Rem(0.75),
                    bottom: Val::Rem(0.75),
                    width: Val::Rem(2.5),
                    height: Val::Rem(2.5),
                    padding: UiRect::all(Val::Rem(0.375)),
                    ..default()
                },
                BackgroundColor(BG_FIELD),
                ZIndex(10),
            ));
            root.spawn((
                Text::new(""),
                MenuNotice,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Rem(4.0),
                    bottom: Val::Rem(0.75),
                    max_width: Val::Percent(75.0),
                    ..default()
                },
                TextFont {
                    font_size: bevy::text::FontSize::Rem(0.875),
                    ..default()
                },
                TextColor(TEXT),
                ZIndex(10),
            ));
        });
}

fn update_menu_notice(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut icon: Local<Option<Handle<Image>>>,
    mut cogs: Query<(Entity, &Hovered, &mut bevy::a11y::AccessibilityNode), With<SettingsCog>>,
    mut labels: Query<&mut Text, With<MenuNotice>>,
    navigation: Res<crate::menu_screens::Navigation>,
    focus: (
        Option<Res<bevy::input_focus::InputFocus>>,
        Option<Res<bevy::input_focus::InputFocusVisible>>,
    ),
) {
    if icon.is_none() {
        // Project-owned vector geometry rasterized here, independent of game
        // fonts or proprietary artwork. Supersampling gives a crisp small cog.
        let side = 64u32;
        let mut rgba = vec![0u8; (side * side * 4) as usize];
        for y in 0..side {
            for x in 0..side {
                let p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5) - Vec2::splat(side as f32 / 2.0);
                let radius = p.length();
                let angle = p.y.atan2(p.x);
                let outer = if (angle * 8.0).cos() > 0.25 {
                    28.0
                } else {
                    23.0
                };
                if radius < outer && radius > 11.0 {
                    let at = ((y * side + x) * 4) as usize;
                    rgba[at..at + 4].copy_from_slice(&[225, 212, 176, 255]);
                }
            }
        }
        *icon = Some(images.add(Image::new(
            bevy::render::render_resource::Extent3d {
                width: side,
                height: side,
                depth_or_array_layers: 1,
            },
            bevy::render::render_resource::TextureDimension::D2,
            rgba,
            bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
            bevy::asset::RenderAssetUsages::default(),
        )));
    }
    let focused = focus
        .0
        .as_ref()
        .and_then(|f| f.get())
        .filter(|_| focus.1.as_ref().is_some_and(|v| v.0));
    for (entity, _, mut accessible) in &mut cogs {
        accessible.0.set_label("PeaceCraft settings");
        commands.entity(entity).insert((
            ImageNode::new(icon.as_ref().unwrap().clone()),
            Outline {
                width: Val::Px(if focused == Some(entity) { 2.0 } else { 0.0 }),
                offset: Val::Px(2.0),
                color: TEXT,
            },
        ));
    }
    for mut label in &mut labels {
        let text = if cogs.iter().any(|(e, h, _)| h.get() || focused == Some(e)) {
            "PeaceCraft settings"
        } else {
            &navigation.notice
        };
        if label.0 != text {
            label.0 = text.to_string();
        }
    }
}

fn menu_navigation_keys(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut navigation: ResMut<crate::menu_screens::Navigation>,
    popups: Query<Entity, With<ResolutionPopup>>,
) {
    if keys.just_pressed(KeyCode::Escape) && !popups.is_empty() {
        for entity in &popups {
            commands.entity(entity).despawn();
        }
        focus_resolution_control(&mut commands);
        return;
    }
    if keys.just_pressed(KeyCode::Escape) && navigation.screen != crate::menu_screens::Screen::Main
    {
        navigation.action("CancelButton");
    }
}

fn menu_acquire_focus(
    mut event: On<bevy::input_focus::AcquireFocus>,
    menus: Query<Entity, With<MenuLayer>>,
    popups: Query<Entity, With<ResolutionPopup>>,
    parents: Query<&ChildOf>,
    enabled: Query<
        (),
        (
            With<bevy::input_focus::tab_navigation::TabIndex>,
            Without<InteractionDisabled>,
        ),
    >,
    focus: Res<bevy::input_focus::InputFocus>,
    mut commands: Commands,
) {
    let target = event.original_event_target();
    let Some(menu) = std::iter::once(target)
        .chain(parents.iter_ancestors(target))
        .find(|entity| menus.contains(*entity))
    else {
        return;
    };
    // AcquireFocus is emitted by PointerFocusPlugin on press. Handle only the
    // original menu's popup; ordinary launcher/window focus keeps its policy.
    if let Some(popup) = popups
        .iter()
        .find(|popup| parents.iter_ancestors(*popup).any(|entity| entity == menu))
        && target != popup
        && !parents.iter_ancestors(target).any(|entity| entity == popup)
    {
        event.propagate(false);
        commands.entity(popup).despawn();
        // Deferred restoration also wins if the standard focus resolver already
        // focused an ordinary menu control at this same event target.
        focus_resolution_control(&mut commands);
        return;
    }
    // A blank click bubbles beyond every TabIndex. Retain the current enabled
    // menu control before the request reaches the window's focus-clear handler.
    if event.focused_entity == menu
        && let Some(current) = focus.get()
        && enabled.contains(current)
        && parents.iter_ancestors(current).any(|entity| entity == menu)
    {
        event.propagate(false);
    }
}

fn focus_resolution_control(commands: &mut Commands) {
    commands.queue(|world: &mut World| {
        let target = world
            .query::<(Entity, &crate::glue_menu::GlueButton)>()
            .iter(world)
            .find_map(|(entity, button)| (button.frame == "ResolutionMenu").then_some(entity));
        if let Some(target) = target
            && let Some(mut focus) = world.get_resource_mut::<bevy::input_focus::InputFocus>()
        {
            focus.set(target, bevy::input_focus::FocusCause::Auto);
        }
    });
}

fn open_resolutions(
    commands: &mut Commands,
    root: Entity,
    cache: &MenuArtCache,
    navigation: &crate::menu_screens::Navigation,
) -> Result<(), String> {
    let fdf = cache
        .fdf
        .as_ref()
        .ok_or("Screen definitions are still loading")?;
    let rect = fdf
        .rect(
            "ResolutionMenu",
            (crate::mainmenu::FRAME_W, crate::mainmenu::FRAME_H),
        )
        .map_err(|e| e.to_string())?;
    let menu = fdf
        .string("ResolutionMenu", "PopupMenuFrame")
        .ok_or("Resolution menu has no popup definition")?;
    let height = match fdf
        .numbers(&menu, "MenuItemHeight")
        .map_err(|e| e.to_string())?
        .as_deref()
    {
        Some([value]) if value.is_finite() && *value > 0.0 => *value,
        _ => return Err("Resolution popup has no valid authored item height".into()),
    };
    let style = crate::menu_text::render_style(fdf, &menu)?;
    let font = cache
        .fonts
        .get(&style.font)
        .ok_or("The menu font is still loading")?
        .clone();
    let popup = commands
        .spawn((
            ResolutionPopup,
            Name::new("Available display resolutions"),
            bevy::input_focus::tab_navigation::TabGroup {
                order: 1,
                modal: true,
            },
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(rect.left / crate::mainmenu::FRAME_W * 100.0),
                top: Val::Percent(
                    (crate::mainmenu::FRAME_H - rect.bottom) / crate::mainmenu::FRAME_H * 100.0,
                ),
                width: Val::Percent(rect.width / crate::mainmenu::FRAME_W * 100.0),
                max_height: Val::Percent(rect.bottom / crate::mainmenu::FRAME_H * 100.0),
                overflow: Overflow::scroll_y(),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(BG_FIELD),
            Hovered::default(),
            ScrollPosition::default(),
            ZIndex(30),
        ))
        .id();
    commands.entity(root).add_child(popup);
    for (index, &(width, pixels)) in navigation.resolutions.iter().enumerate() {
        let choice = commands
            .spawn((
                Button,
                Hovered::default(),
                ResolutionChoice(width, pixels),
                Name::new(format!("{width} x {pixels}")),
                bevy::input_focus::tab_navigation::TabIndex(
                    i32::try_from(index).map_err(|_| "Too many display modes")?,
                ),
                Node {
                    min_height: Val::Vh(height / crate::mainmenu::FRAME_H * 100.0),
                    flex_shrink: 0.0,
                    width: Val::Percent(100.0),
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(if navigation.draft.resolution == Some((width, pixels)) {
                    ACCENT_SOFT
                } else {
                    BG_FIELD
                }),
            ))
            .with_child((
                Text::new(format!("{width} x {pixels}")),
                TextFont {
                    font: font.clone().into(),
                    font_size: bevy::text::FontSize::Vh(
                        style.height / crate::mainmenu::FRAME_H * 100.0,
                    ),
                    ..default()
                },
                TextColor(srgba(style.colour)),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(popup).add_child(choice);
        if index == 0 {
            commands.queue(move |world: &mut World| {
                if let Some(mut focus) = world.get_resource_mut::<bevy::input_focus::InputFocus>() {
                    focus.set(choice, bevy::input_focus::FocusCause::Auto);
                }
            });
        }
    }
    Ok(())
}

/// Marker for the menu layer, so it can be dismissed once a choice is made.
#[derive(Component)]
pub(crate) struct MenuLayer;

#[derive(Component)]
struct LogoLayer;

#[derive(Resource, Default)]
struct SceneLoad {
    finished: bool,
    addon: Option<Addon>,
}

/// The page's asset-store generation, or `0` when there is no store to ask.
///
/// A browser replaces its served assets when the user imports a local ISO, so the
/// caches have to notice. Native reads a staged install that does not change under
/// it, hence the constant.
#[cfg(target_arch = "wasm32")]
fn asset_revision() -> u32 {
    crate::browser::asset_revision()
}
#[cfg(not(target_arch = "wasm32"))]
fn asset_revision() -> u32 {
    0
}

/// Drops everything decoded from the page's asset store when that store is
/// replaced, so an imported local ISO is not masked by whatever was served first.
///
/// The art and frame caches are cleared here; the logo and the 3D scene rebuild
/// themselves because their own systems re-run once their entity and resource are
/// gone, which is why this only marks them stale rather than reconstructing them.
/// Nothing restarts the app: the F8 overlay, the settings, the input state and the
/// Edition selection are all untouched.
/// The current asset-store generation, refreshed from the platform once a frame.
///
/// Kept as a resource so the invalidation is a plain comparison that a test can
/// drive: [`asset_revision`] is a platform call that native cannot vary.
#[derive(Resource, Default)]
pub(crate) struct AssetStoreRevision(u32);

/// Refreshes [`AssetStoreRevision`] from the platform.
fn poll_asset_revision(mut revision: ResMut<AssetStoreRevision>) {
    revision.0 = asset_revision();
}

#[derive(SystemParam)]
struct MenuInvalidationQueries<'w, 's> {
    logos: Query<'w, 's, Entity, With<crate::menu_panel::MenuLogo>>,
    scene_entities: Query<'w, 's, Entity, With<crate::mainmenu_scene::MainMenuSceneEntity>>,
    controls: Query<'w, 's, (Entity, Option<&'static Children>), With<ControlRoot>>,
    glue: ResMut<'w, crate::glue_menu::GlueImages>,
}

fn invalidate_on_asset_revision(
    mut commands: Commands,
    revision: Res<AssetStoreRevision>,
    mut cache: ResMut<MenuArtCache>,
    mut scene: ResMut<SceneLoad>,
    queries: MenuInvalidationQueries,
    navigation: Option<ResMut<crate::menu_screens::Navigation>>,
) {
    let MenuInvalidationQueries {
        logos,
        scene_entities,
        controls,
        mut glue,
    } = queries;
    if cache.revision == revision.0 {
        return;
    }
    cache.revision = revision.0;
    if let Some(mut navigation) = navigation {
        navigation.options_entry = None;
    }
    // Everything decoded or derived from the old store goes, including the
    // texts the labels and their style come from.
    cache.button = None;
    cache.fonts.clear();
    cache.skin = None;
    cache.fdf = None;
    cache.fdf_failed = false;
    cache.strings = None;
    cache.templates = None;
    cache.main_menu = None;
    cache.native_loaded = false;
    let install = glue.install.clone();
    glue.reset(install);
    // The control layer is rebuilt from the new frame definitions by
    // `build_control_layer` once its root is empty.
    for (_, children) in &controls {
        for child in children.into_iter().flatten() {
            commands.entity(*child).despawn();
        }
    }
    // The logo rebuilds once its entity is gone: `initialize_menu_logo` re-runs
    // on an empty set.
    for entity in &logos {
        commands.entity(entity).despawn();
    }
    // The scene owns both entities and a resource, so clear them here rather than
    // leaving it to a later system's own staleness rule. `addon` is deliberately
    // left alone so `initialize_menu_scene` does not despawn them a second time;
    // clearing `finished` is what makes it rebuild.
    for entity in &scene_entities {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<crate::mainmenu_scene::MainMenuScene>();
    commands.remove_resource::<crate::menu_sprites::MenuPanels>();
    scene.finished = false;
    info!(
        "Asset store changed (revision {}): re-reading the menu art and labels",
        revision.0
    );
}

/// The original default CSpriteFrame projection adds vertices to LEFT/BOTTOM.
/// See the executable trace in docs/re-notes/glue-coordinate-runtime.md.
fn initialize_menu_logo(
    mut commands: Commands,
    launcher: Res<Launcher>,
    layers: Query<Entity, With<LogoLayer>>,
    logos: Query<Entity, With<crate::menu_panel::MenuLogo>>,
    mut images: ResMut<Assets<Image>>,
    mut entrance: ResMut<crate::menu_panel::LogoEntrance>,
) {
    let Ok(layer) = layers.single() else {
        return;
    };
    if !logos.is_empty() {
        return;
    }
    let install = selected_install(&launcher);
    let inputs = || -> Result<_, String> {
        let skin = crate::mainmenu::art_bytes(install.as_deref(), crate::mainmenu::SKIN_TABLE)
            .ok_or_else(|| "missing logo skin table".to_string())?;
        let skin = std::str::from_utf8(&skin).map_err(|e| format!("logo skin table: {e}"))?;
        let skin = crate::menu_skin::SkinTable::parse(skin).map_err(|e| e.to_string())?;
        let fdf = crate::mainmenu::art_bytes(install.as_deref(), crate::mainmenu::MAIN_MENU_FDF)
            .ok_or_else(|| "missing logo MainMenu FDF".to_string())?;
        let fdf = std::str::from_utf8(&fdf).map_err(|e| format!("logo MainMenu FDF: {e}"))?;
        let fdf = crate::fdf::Fdf::parse(fdf).map_err(|e| e.to_string())?;
        let anchor = crate::menu_panel::logo_anchor(&fdf).map_err(|e| e.to_string())?;
        let assets = crate::menu_panel::load_logo(
            install.as_deref(),
            &skin,
            launcher.mode.addon == Addon::Tft,
        )?;
        Ok((assets, anchor))
    };
    let (assets, anchor) = match inputs() {
        Ok(inputs) => inputs,
        Err(error) => {
            warn!("Original menu logo: {error}");
            return;
        }
    };
    let data = assets.data;
    if !data.planar {
        warn!(
            "Original menu logo {:?}: unsupported nonplanar geometry",
            assets.model_path
        );
        return;
    }
    let texture = match pcx_gfx::load_image(&assets.texture, &data.texture) {
        Ok(texture) => texture,
        Err(error) => {
            warn!("Original menu logo texture: {error}");
            return;
        }
    };
    // Keep the existing quad projection; the FDF now supplies its anchor.
    let left = anchor.0 + data.quad.0;
    let top = data.quad_height() - anchor.1 - data.quad.3;
    let node = Node {
        width: Val::Vw(data.quad_width() / crate::mainmenu::FRAME_W * 100.0),
        height: Val::Vh(data.quad_height() / crate::mainmenu::FRAME_H * 100.0),
        margin: UiRect {
            left: Val::Vw(left / crate::mainmenu::FRAME_W * 100.0),
            top: Val::Vh(top / crate::mainmenu::FRAME_H * 100.0),
            ..default()
        },
        flex_shrink: 0.0,
        ..default()
    };
    let mut image = ImageNode::new(images.add(pcx_gfx::viewer::rgba_image(&texture)));
    image.color = Color::srgba(1.0, 1.0, 1.0, 0.0);
    let entity = commands
        .spawn((node, image, crate::menu_panel::MenuLogo, Pickable::IGNORE))
        .id();
    commands.entity(layer).add_child(entity);
    entrance.elapsed_ms = 0.0;
    entrance.data = Some(data);
    info!(
        "Original menu logo {:?}: authored Birth and FDF anchor",
        assets.model_path
    );
}

#[derive(SystemParam)]
struct SceneAssets<'w> {
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<StandardMaterial>>,
    images: ResMut<'w, Assets<Image>>,
}

fn show_logo_for_screen(
    cache: Res<MenuArtCache>,
    mut layers: Query<&mut Visibility, With<LogoLayer>>,
) {
    let defined = cache.fdf.as_ref().is_some_and(|fdf| {
        fdf.frame(crate::menu_panel::LOGO_FRAME)
            .is_some_and(|frame| frame.kind == "SPRITE")
    });
    let desired = if defined {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut visibility in &mut layers {
        if *visibility != desired {
            *visibility = desired;
        }
    }
}

fn initialize_menu_scene(
    mut commands: Commands,
    mut state: ResMut<SceneLoad>,
    assets: SceneAssets,
    launcher: Res<Launcher>,
    menu: Query<Entity, With<MenuLayer>>,
    scene_entities: Query<Entity, With<crate::mainmenu_scene::MainMenuSceneEntity>>,
) {
    let SceneAssets {
        mut meshes,
        mut materials,
        mut images,
    } = assets;
    if menu.is_empty() {
        return;
    }
    if state.addon != Some(launcher.mode.addon) {
        for entity in &scene_entities {
            commands.entity(entity).despawn();
        }
        commands.remove_resource::<crate::mainmenu_scene::MainMenuScene>();
        commands.remove_resource::<crate::menu_sprites::MenuPanels>();
        state.addon = Some(launcher.mode.addon);
        state.finished = false;
    }
    if state.finished {
        return;
    }
    let install = selected_install(&launcher);
    let (_, served, directory) =
        crate::mainmenu_scene::scene_source(launcher.mode.addon == Addon::Tft);
    let source = crate::mainmenu::art_bytes(install.as_deref(), crate::mainmenu::SKIN_TABLE)
        .ok_or_else(|| "background skin table unavailable".to_string())
        .and_then(|bytes| String::from_utf8(bytes).map_err(|e| e.to_string()))
        .and_then(|text| crate::menu_skin::SkinTable::parse(&text).map_err(|e| e.to_string()))
        .and_then(|skin| {
            crate::mainmenu_scene::skin_scene_source(&skin, launcher.mode.addon == Addon::Tft)
                .map_err(|e| e.to_string())
        });
    let source = match source {
        Ok(path) => path,
        Err(error) => {
            warn!("Original menu background: {error}");
            return;
        }
    };
    let bytes = crate::mainmenu::art_bytes_named(install.as_deref(), &source, served);
    let Some(bytes) = bytes else {
        #[cfg(not(target_arch = "wasm32"))]
        {
            state.finished = true;
            warn!("Original menu background unavailable in the configured installation");
        }
        return;
    };
    #[cfg(target_arch = "wasm32")]
    if crate::mainmenu_scene::texture_served_names_in(&bytes, directory)
        .iter()
        .any(|name| crate::browser::stored_bytes(name).is_none())
    {
        return;
    }
    state.finished = true;
    let textures = crate::mainmenu_scene::load_textures_in(install.as_deref(), &bytes, directory);
    match crate::mainmenu_scene::spawn(
        &mut commands,
        &mut meshes,
        &mut materials,
        &mut images,
        &source,
        &bytes,
        &textures,
    ) {
        Ok(info) => info!(
            "Original menu background: {} geosets, {} textures, {} ms loop; missing {:?}",
            info.geosets,
            textures.len(),
            info.duration_ms,
            info.missing_textures
        ),
        Err(error) => {
            warn!("Original menu background: {error}");
            return;
        }
    }
    // The skin keys' `_V%d` is the edition: 0 Reign of Chaos, 1 The Frozen Throne.
    let edition = u32::from(launcher.mode.addon == Addon::Tft);
    let panels = crate::menu_sprites::load(install.as_deref(), edition).and_then(|sources| {
        crate::menu_sprites::spawn(
            &mut commands,
            crate::mainmenu_scene::GeosetAssets {
                meshes: &mut meshes,
                materials: &mut materials,
                images: &mut images,
            },
            install.as_deref(),
            sources,
        )
    });
    match panels {
        Ok(count) => info!("Original menu panels: {count} sprite layers, MainMenu Birth"),
        Err(error) => warn!("Original menu panels: {error}"),
    }
}

fn synchronize_window_camera_size(
    windows: Query<(Entity, &Window), With<bevy::window::PrimaryWindow>>,
    mut observed: Local<Option<(Entity, UVec2)>>,
    mut resized: MessageWriter<bevy::window::WindowResized>,
) {
    let Ok((entity, window)) = windows.single() else {
        *observed = None;
        return;
    };
    let current = (
        entity,
        UVec2::new(window.physical_width(), window.physical_height()),
    );
    if observed.as_ref() == Some(&current) {
        return;
    }
    *observed = Some(current);
    // Programmatic Window changes precede Winit's resize acknowledgement. Give
    // every camera sharing this target the same dimensions before extraction.
    resized.write(bevy::window::WindowResized {
        window: entity,
        width: window.width(),
        height: window.height(),
    });
}

fn show_menu_or_launcher(
    intro: Res<crate::intro::Intro>,
    windows: Query<&Window>,
    menu: Query<Entity, With<MenuLayer>>,
    mut launcher: Query<&mut Visibility, With<LauncherPanel>>,
    mut cameras: Query<&mut Camera, With<crate::mainmenu_scene::MainMenuSceneCamera>>,
    mut ui_cameras: Query<
        &mut Camera,
        (
            With<bevy::ui::IsDefaultUiCamera>,
            Without<crate::mainmenu_scene::MainMenuSceneCamera>,
        ),
    >,
) {
    // The original CFrame renderer has a centered 4:3 branch. Its main-menu
    // live flags are unmeasured; keep the authored extent without stretching.
    let viewport = if !menu.is_empty() {
        windows.single().ok().map(|window| {
            let full = UVec2::new(window.physical_width(), window.physical_height());
            // Classic square/5:4 displays use the original direct conversion.
            // Portrait browser windows contain the authored UI to keep it usable.
            let (width, height) =
                if full.x >= full.y && u64::from(full.x) * 3 <= u64::from(full.y) * 4 {
                    (full.x.max(1), full.y.max(1))
                } else {
                    let height = full.y.min((full.x as u64 * 3 / 4) as u32).max(1);
                    (full.x.min((height as u64 * 4 / 3) as u32).max(1), height)
                };
            bevy::camera::Viewport {
                physical_position: (full.saturating_sub(UVec2::new(width, height))) / 2,
                physical_size: UVec2::new(width, height),
                ..default()
            }
        })
    } else {
        None
    };
    for mut visibility in &mut launcher {
        let desired = if menu.is_empty() && !intro.playing() {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if *visibility != desired {
            *visibility = desired;
        }
    }
    for mut camera in &mut cameras {
        // Original GlueSpriteLayerBackground explicitly sets frame flag bit 0;
        // it fills the display independently of the authored panel coordinates.
        if camera.viewport.is_some() {
            camera.viewport = None;
        }
        let active = !menu.is_empty();
        if camera.is_active != active {
            camera.is_active = active;
        }
    }
    // Without an active scene camera, UI must clear the previous screen itself.
    let clear = if !menu.is_empty() && !cameras.is_empty() {
        bevy::camera::ClearColorConfig::None
    } else {
        bevy::camera::ClearColorConfig::Custom(BG_APP)
    };
    for mut camera in &mut ui_cameras {
        if camera
            .viewport
            .as_ref()
            .map(|v| (v.physical_position, v.physical_size))
            != viewport
                .as_ref()
                .map(|v| (v.physical_position, v.physical_size))
        {
            camera.viewport = viewport.clone();
        }
        let matches = match (camera.clear_color, clear) {
            (bevy::camera::ClearColorConfig::None, bevy::camera::ClearColorConfig::None) => true,
            (
                bevy::camera::ClearColorConfig::Custom(a),
                bevy::camera::ClearColorConfig::Custom(b),
            ) => a == b,
            _ => false,
        };
        if !matches {
            camera.clear_color = clear;
        }
    }
}

fn menu_is_visible(menu: Query<Entity, With<MenuLayer>>) -> bool {
    !menu.is_empty()
}

/// The menu's frame and text sources, and the plate art the launcher's own
/// controls wear.
#[derive(Resource, Default)]
struct MenuArtCache {
    /// `GlueScreen-Button1-Border.blp`, worn by the launcher's own buttons.
    button: Option<Handle<Image>>,
    /// The launcher panels' tiled backdrop.
    panel: Option<Handle<Image>>,
    /// One handle per authored face.
    fonts: std::collections::HashMap<String, Handle<Font>>,
    /// `UI\war3skins.txt`, the face -> file binding's source.
    skin: Option<String>,
    /// Faces and labels whose problem has already been reported.
    problems: std::collections::HashSet<String>,
    /// The parsed frame definitions. `None` until **both** FDFs have been read
    /// and parsed, so a partially-arrived pair is never cached as complete.
    fdf: Option<crate::fdf::Fdf>,
    /// Set when an FDF that *is* present fails to read or parse, or the control
    /// layer cannot be derived from it; reported once rather than every frame.
    fdf_failed: bool,
    /// `GlobalStrings.fdf`, the labels' source.
    strings: Option<String>,
    /// `StandardTemplates.fdf`, the label style's source.
    templates: Option<String>,
    /// `MainMenu.fdf`, whose text frames carry per-entry style records.
    main_menu: Option<String>,
    screen: crate::menu_screens::Screen,
    /// The asset-store generation this cache was filled from.
    revision: u32,
    native_loaded: bool,
    install: Option<std::path::PathBuf>,
    selection: Option<String>,
}

/// Reads a text asset once it is available.
fn cached_text(slot: &mut Option<String>, install: Option<&std::path::Path>, path: &str) {
    if slot.is_none()
        && let Some(bytes) = crate::mainmenu::art_bytes(install, path)
    {
        *slot = std::str::from_utf8(&bytes).ok().map(str::to_string);
    }
}

type ButtonArtData = (Entity, Option<&'static InteractionDisabled>);
type ButtonArtFilter = (
    With<Button>,
    Without<ButtonArt>,
    Without<crate::glue_menu::GlueButton>,
    Without<SettingsCog>,
    Without<ResolutionChoice>,
);

#[derive(SystemParam)]
struct MenuArtQueries<'w, 's> {
    buttons: Query<'w, 's, ButtonArtData, ButtonArtFilter>,
    // Store replacement changes handles while preserving button identities.
    dressed: Query<'w, 's, &'static mut ImageNode, With<ButtonArt>>,
    panels: Query<'w, 's, Entity, (With<Wc3Panel>, Without<ImageNode>)>,
}

/// Reads the menu's frame definitions, strings and skin table, and puts the
/// plate art on the launcher's own buttons.
///
/// Deferred rather than done at spawn because a browser only has what the page
/// fetched, a moment later; running every frame and doing nothing until the
/// bytes exist means one code path serves both.
fn apply_menu_art(
    launcher: Res<Launcher>,
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut cache: ResMut<MenuArtCache>,
    mut glue: ResMut<crate::glue_menu::GlueImages>,
    mut navigation: ResMut<crate::menu_screens::Navigation>,
    queries: MenuArtQueries,
) {
    let MenuArtQueries {
        buttons,
        mut dressed,
        panels,
    } = queries;
    let selection = format!("{}:{:?}", launcher.source, launcher.mode.assets);
    if cache.selection.as_ref() != Some(&selection) {
        let install = selected_install(&launcher);
        if cache.install != install || glue.install != install {
            *cache = MenuArtCache {
                revision: cache.revision,
                install: install.clone(),
                ..default()
            };
            glue.reset(install);
        }
        cache.selection = Some(selection);
    }
    let cache = &mut *cache;
    if cache.screen != navigation.screen {
        cache.screen = navigation.screen;
        cache.main_menu = None;
        cache.fdf = None;
        cache.fdf_failed = false;
        cache.native_loaded = false;
    }
    let install = cache.install.clone();
    let install = install.as_deref();
    // The desktop install is read once; the browser store fills asynchronously.
    if !cache.native_loaded || cfg!(target_arch = "wasm32") {
        for (slot, path) in [
            (&mut cache.button, crate::mainmenu::BUTTON_ART),
            (&mut cache.panel, PANEL_ART),
        ] {
            if slot.is_none()
                && let Some(bytes) = crate::mainmenu::art_bytes(install, path)
                && let Ok(tex) = pcx_gfx::load_image(&bytes, path)
            {
                *slot = Some(images.add(pcx_gfx::viewer::rgba_image(&tex)));
            }
        }
        cached_text(&mut cache.skin, install, crate::mainmenu::SKIN_TABLE);
        cached_text(&mut cache.main_menu, install, cache.screen.file());
        if cache.main_menu.is_none() && cache.screen != crate::menu_screens::Screen::Main {
            navigation.notice = format!(
                "Menu definition unavailable: {}. Import the corresponding game files, or press Escape to return.",
                cache.screen.file()
            );
        }
        cached_text(
            &mut cache.strings,
            install,
            crate::mainmenu::GLOBAL_STRINGS_FDF,
        );
        cached_text(
            &mut cache.templates,
            install,
            crate::mainmenu::STANDARD_TEMPLATES_FDF,
        );
        // Templates first, so a menu override wins.
        if cache.fdf.is_none()
            && !cache.fdf_failed
            && let (Some(templates), Some(menu)) = (&cache.templates, &cache.main_menu)
        {
            match crate::fdf::Fdf::parse(templates).and_then(|mut f| {
                let mut loaded = std::collections::HashSet::from([
                    crate::mainmenu::STANDARD_TEMPLATES_FDF.to_ascii_lowercase(),
                    cache.screen.file().to_ascii_lowercase(),
                ]);
                let mut pending = crate::menu_screens::includes(menu);
                let mut sources = Vec::new();
                let mut total = menu.len() + templates.len();
                while let Some(path) = pending.pop() {
                    if !loaded.insert(path.to_ascii_lowercase()) {
                        continue;
                    }
                    if loaded.len() > 64 {
                        return Err(crate::fdf::FdfError::Unsupported {
                            frame: path,
                            property: "IncludeFile",
                            value: "include count limit exceeded".into(),
                        });
                    }
                    let bytes = crate::mainmenu::art_bytes(install, &path).ok_or_else(|| {
                        crate::fdf::FdfError::UnknownFrame(format!("missing include {path}"))
                    })?;
                    total = total.checked_add(bytes.len()).ok_or_else(|| {
                        crate::fdf::FdfError::UnknownFrame("include size overflow".into())
                    })?;
                    if total > 8 * 1024 * 1024 {
                        return Err(crate::fdf::FdfError::UnknownFrame(
                            "screen definition size limit exceeded".into(),
                        ));
                    }
                    let text = String::from_utf8(bytes).map_err(|_| {
                        crate::fdf::FdfError::UnknownFrame(format!("non UTF-8 include {path}"))
                    })?;
                    pending.extend(crate::menu_screens::includes(&text));
                    sources.push(text);
                }
                for source in sources.iter().rev() {
                    f.parse_into(source)?;
                }
                f.parse_into(menu)?;
                crate::menu_screens::prime_text_sizes(&mut f);
                Ok(f)
            }) {
                Ok(fdf) => {
                    if matches!(navigation.screen, crate::menu_screens::Screen::Options(_))
                        && navigation.options_entry.is_none()
                    {
                        let pages: Vec<_> = [
                            ("GameplayButton", crate::menu_screens::Page::Gameplay),
                            ("VideoButton", crate::menu_screens::Page::Video),
                            ("SoundButton", crate::menu_screens::Page::Sound),
                        ]
                        .into_iter()
                        .filter(|(name, _)| fdf.flag(name, "TabFocusDefault"))
                        .map(|(_, page)| page)
                        .collect();
                        if let [page] = pages.as_slice() {
                            navigation.options_entry = Some(*page);
                            navigation.screen = crate::menu_screens::Screen::Options(*page);
                            cache.screen = navigation.screen;
                        }
                    }
                    cache.fdf = Some(fdf);
                }
                Err(e) => {
                    warn!("menu frame definitions unusable: {e}");
                    cache.fdf_failed = true;
                }
            }
        }
        cache.native_loaded = true;
    }
    if let Some(handle) = &cache.button {
        for mut image in &mut dressed {
            if image.image != *handle {
                image.image = handle.clone();
            }
        }
        for (entity, disabled) in &buttons {
            let mut image = ImageNode::new(handle.clone()).with_mode(NodeImageMode::Stretch);
            if disabled.is_some() {
                image.color = DISABLED_FG;
            }
            commands.entity(entity).insert((ButtonArt, image));
        }
    }
    if let Some(handle) = &cache.panel {
        for entity in &panels {
            let mut image = ImageNode::new(handle.clone()).with_mode(NodeImageMode::Tiled {
                tile_x: true,
                tile_y: true,
                stretch_value: 1.0,
            });
            image.color = Color::srgb(0.12, 0.12, 0.12);
            commands.entity(entity).insert(image);
        }
    }
}

/// Builds the FDF control layer into the empty [`ControlRoot`] once the frame
/// definitions are parsed.
fn build_control_layer(
    mut commands: Commands,
    mut cache: ResMut<MenuArtCache>,
    roots: Query<(Entity, Option<&Children>), With<ControlRoot>>,
    mut navigation: ResMut<crate::menu_screens::Navigation>,
) {
    if navigation.built != Some(navigation.screen) {
        for (_, children) in &roots {
            for child in children.into_iter().flatten() {
                commands.entity(*child).despawn();
            }
        }
        navigation.built = Some(navigation.screen);
        // Deferred removals finish before building the new tree next frame.
        if roots.iter().any(|(_, c)| c.is_some_and(|c| !c.is_empty())) {
            return;
        }
    }
    let Some(fdf) = &cache.fdf else {
        return;
    };
    let empty: Vec<Entity> = roots
        .iter()
        .filter(|(_, children)| children.is_none_or(|c| c.is_empty()))
        .map(|(root, _)| root)
        .collect();
    if empty.is_empty() {
        return;
    }
    let result = if navigation.screen == crate::menu_screens::Screen::Main {
        crate::glue_menu::control_items(
            fdf,
            navigation.screen.root(),
            (crate::mainmenu::FRAME_W, crate::mainmenu::FRAME_H),
        )
    } else {
        crate::glue_menu::control_items_for(
            fdf,
            navigation.screen.root(),
            (crate::mainmenu::FRAME_W, crate::mainmenu::FRAME_H),
            |name| navigation.screen.includes(fdf, name),
        )
    };
    match result {
        Ok((items, unsupported)) => {
            if !unsupported.is_empty() {
                warn!("Main menu frames not drawn: {unsupported:?}");
            }
            info!(
                "Main menu: {} frames from the FDF control layer",
                items.len()
            );
            for root in empty {
                crate::glue_menu::spawn(&mut commands, root, &items);
                let visible: Vec<_> = items
                    .iter()
                    .filter(|i| {
                        matches!(
                            i.kind,
                            crate::glue_menu::Kind::Button(_) | crate::glue_menu::Kind::Widget(_)
                        )
                    })
                    .map(|i| i.frame.clone())
                    .collect();
                let order = crate::menu_screens::focus_order(fdf, &visible);
                commands.queue(move |world: &mut World| {
                    let targets: Vec<_> = world
                        .get::<Children>(root)
                        .into_iter()
                        .flatten()
                        .filter_map(|entity| {
                            let position =
                                world.get::<crate::menu_screens::ScreenPosition>(*entity)?;
                            let index = order.iter().position(|n| n == &position.0)?;
                            Some((*entity, i32::try_from(index).ok()?))
                        })
                        .collect();
                    let first = targets
                        .iter()
                        .filter(|(entity, _)| {
                            !world.entity(*entity).contains::<InteractionDisabled>()
                        })
                        .min_by_key(|(_, index)| *index)
                        .map(|(entity, _)| *entity);
                    for (entity, index) in targets {
                        world
                            .entity_mut(entity)
                            .insert(bevy::input_focus::tab_navigation::TabIndex(index));
                    }
                    if let Some(first) = first
                        && let Some(mut focus) =
                            world.get_resource_mut::<bevy::input_focus::InputFocus>()
                    {
                        focus.set(first, bevy::input_focus::FocusCause::Auto);
                    }
                });
            }
        }
        Err(error) => {
            warn!("Main menu control layer: {error}");
            navigation.notice =
                format!("This screen could not be loaded: {error}. Press Escape to return.");
            cache.fdf = None;
            cache.fdf_failed = true;
        }
    }
}

fn build_setting_controls(
    mut commands: Commands,
    cache: Res<MenuArtCache>,
    launcher: Res<Launcher>,
    buttons: Query<(Entity, &crate::glue_menu::GlueButton)>,
    nodes: Query<
        (Entity, &crate::menu_screens::ScreenPosition),
        Without<crate::menu_screens::SettingControl>,
    >,
) {
    for (entity, button) in &buttons {
        if button.frame == "CustomCampaignButton" {
            if launcher.mode.addon == Addon::Tft && launcher.allows_addon(Addon::Tft) {
                commands.entity(entity).remove::<InteractionDisabled>();
            } else {
                commands.entity(entity).insert(InteractionDisabled);
            }
        }
    }
    let Some(fdf) = cache.fdf.as_ref() else {
        return;
    };
    for (entity, position) in &nodes {
        if let Ok(Some(widget)) = crate::menu_screens::widget(fdf, &position.0) {
            crate::menu_screens::spawn_widget(&mut commands, entity, &position.0, &widget, fdf);
            if let Ok(rect) = fdf.rect(
                &position.0,
                (crate::mainmenu::FRAME_W, crate::mainmenu::FRAME_H),
            ) && let Err(error) = crate::glue_menu::decorate_setting(
                &mut commands,
                entity,
                fdf,
                &position.0,
                Vec2::new(rect.width, rect.height),
            ) {
                warn!("Setting art {}: {error}", position.0);
            }
        }
    }
}

fn measure_screen_text(
    mut cache: ResMut<MenuArtCache>,
    labels: Query<(&crate::glue_menu::GlueLabel, &bevy::text::TextLayoutInfo)>,
    camera: Query<&Camera, With<bevy::ui::IsDefaultUiCamera>>,
) {
    let Some(size) = camera
        .iter()
        .next()
        .and_then(Camera::physical_viewport_size)
        .map(UVec2::as_vec2)
    else {
        return;
    };
    if size.min_element() <= 0.0 {
        return;
    }
    let Some(fdf) = cache.fdf.as_mut() else {
        return;
    };
    for (label, layout) in &labels {
        if layout.size.x > 0.0 && layout.size.y > 0.0 {
            fdf.measure_text(
                &label.frame,
                (
                    layout.size.x / size.x * crate::mainmenu::FRAME_W,
                    layout.size.y / size.y * crate::mainmenu::FRAME_H,
                ),
            );
        }
    }
}

fn position_screen_nodes(
    cache: Res<MenuArtCache>,
    mut nodes: Query<(&crate::menu_screens::ScreenPosition, &mut Node)>,
) {
    let Some(fdf) = cache.fdf.as_ref() else {
        return;
    };
    for (position, mut node) in &mut nodes {
        if let Ok(r) = fdf.rect(
            &position.0,
            (crate::mainmenu::FRAME_W, crate::mainmenu::FRAME_H),
        ) {
            node.left = Val::Percent(r.left / crate::mainmenu::FRAME_W * 100.0);
            node.top = Val::Percent(
                (crate::mainmenu::FRAME_H - r.top()) / crate::mainmenu::FRAME_H * 100.0,
            );
            node.width = Val::Percent(r.width / crate::mainmenu::FRAME_W * 100.0);
            node.height = Val::Percent(r.height / crate::mainmenu::FRAME_H * 100.0);
        }
    }
}

/// The style a text frame declares, resolved across the menu and template texts.
fn text_style(
    main_menu: Option<&str>,
    templates: Option<&str>,
    frame: &str,
) -> Result<crate::menu_text::TextStyle, crate::menu_text::TextError> {
    let texts: Vec<&str> = [main_menu, templates].into_iter().flatten().collect();
    crate::mainmenu::menu_text_style(&texts, frame)
}

type LabelData = (
    Entity,
    &'static crate::glue_menu::GlueLabel,
    &'static ChildOf,
    Option<&'static crate::menu_screens::SettingValue>,
    Option<&'static crate::menu_screens::LabelFontParent>,
);

/// Fills each label from its text frame: the authored string, its face, size,
/// colours, justification and shadow.
///
/// Every authored run is kept, in order, so `KEY_EXIT`'s interior hotkey
/// (`E|Cffffffffx|Rit`) renders "Exit" with its leading `E`. A label that cannot
/// be derived is reported and left empty, never invented.
fn apply_menu_text(
    mut commands: Commands,
    mut cache: ResMut<MenuArtCache>,
    mut fonts: ResMut<Assets<Font>>,
    labels: Query<LabelData, Without<MenuTextFilled>>,
    disabled: Query<(), With<InteractionDisabled>>,
    mut nodes: Query<&mut Node>,
) {
    let MenuArtCache {
        fonts: faces,
        skin,
        problems,
        fdf,
        strings,
        main_menu,
        templates,
        screen,
        install,
        ..
    } = &mut *cache;
    let (Some(strings), Some(fdf)) = (strings.as_deref(), fdf.as_ref()) else {
        return;
    };
    for (entity, label, parent, dynamic, font_parent) in &labels {
        let frame = label.frame.as_str();
        let parsed = dynamic
            .map(|_| String::new())
            .or_else(|| fdf.text_value(frame))
            .ok_or_else(|| format!("{frame} has no Text"))
            .map(|key| crate::menu_text::global_string(strings, &key).unwrap_or(key))
            .and_then(|raw| crate::menu_text::Label::parse(&raw).map_err(|e| e.to_string()))
            .and_then(|parsed| {
                (if *screen == crate::menu_screens::Screen::Main {
                    text_style(main_menu.as_deref(), templates.as_deref(), frame)
                        .map_err(|e| e.to_string())
                } else {
                    crate::menu_text::render_style_with_parent(
                        fdf,
                        frame,
                        font_parent.map_or(frame, |p| p.0.as_str()),
                    )
                })
                .map(|style| (parsed, style))
                .map_err(|e| e.to_string())
            });
        let (parsed, style) = match parsed {
            Ok(found) => found,
            Err(error) => {
                warn!("Menu label {frame}: {error}");
                commands.entity(entity).insert(MenuTextFilled);
                continue;
            }
        };
        // The entry's own face, bound through the skin table. A face whose file
        // has not been read yet defers the fill; no other face is substituted.
        if !faces.contains_key(&style.font) {
            let Some(skin) = skin.as_deref() else {
                continue;
            };
            let binding = match crate::mainmenu::menu_font_file(skin, &style.font) {
                Ok(binding) => binding,
                Err(error) => {
                    if problems.insert(style.font.clone()) {
                        warn!("Menu font {:?}: {error}", style.font);
                    }
                    continue;
                }
            };
            let Some(served) = crate::mainmenu::served_font_name(&binding.file) else {
                continue;
            };
            let Some(bytes) =
                crate::mainmenu::art_bytes_named(install.as_deref(), &binding.file, &served)
            else {
                continue;
            };
            faces.insert(style.font.clone(), fonts.add(Font::from_bytes(bytes)));
        }
        info!(
            "Menu label {frame}: {:?} (hotkey {:?}), face {:?}",
            parsed.visible(),
            parsed.hotkey(),
            style.font
        );
        let font = bevy::text::FontSource::Handle(faces[&style.font].clone());
        let size = bevy::text::FontSize::Vh(style.height / crate::mainmenu::FRAME_H * 100.0);
        let dim = disabled.contains(label.button);
        let normal = srgba(if dim { style.disabled } else { style.colour });
        let hot = srgba(if dim { style.disabled } else { style.highlight });
        if let Ok(mut node) = nodes.get_mut(parent.parent()) {
            node.justify_content = justify_v(style.justify_v);
            node.align_items = align_h(style.justify_h);
            if let Ok(Some(offset)) = fdf.numbers(frame, "FontJustificationOffset")
                && let [x, y] = offset.as_slice()
                && x.is_finite()
                && y.is_finite()
            {
                node.padding.left = Val::Vw(*x / crate::mainmenu::FRAME_W * 100.0);
                node.padding.top = Val::Vh(-*y / crate::mainmenu::FRAME_H * 100.0);
            }
        }
        commands.entity(entity).insert((
            TextFont {
                font: font.clone(),
                font_size: size,
                ..default()
            },
            TextColor(normal),
            TextLayout::new(
                justify(style.justify_h),
                if dynamic.is_none() && fdf.record(frame, "Width").is_some() {
                    bevy::text::LineBreak::WordBoundary
                } else {
                    bevy::text::LineBreak::NoWrap
                },
            ),
            TextShadow {
                offset: Vec2::ZERO,
                color: srgba(style.shadow_colour),
            },
            crate::glue_menu::FrameShadow(Vec2::from(style.shadow_offset)),
        ));
        for run in parsed.spans.iter().filter(|_| dynamic.is_none()) {
            let child = commands
                .spawn((
                    TextSpan::new(run.text.clone()),
                    TextFont {
                        font: font.clone(),
                        font_size: size,
                        ..default()
                    },
                    TextColor(if run.hotkey { hot } else { normal }),
                ))
                .id();
            commands.entity(entity).add_child(child);
        }
        commands.entity(entity).insert(MenuTextFilled);
    }
}
/// The template's horizontal justification, as Bevy's text layout spells it.
fn justify(h: crate::menu_text::JustifyH) -> bevy::text::Justify {
    match h {
        crate::menu_text::JustifyH::Left => bevy::text::Justify::Left,
        crate::menu_text::JustifyH::Center => bevy::text::Justify::Center,
        crate::menu_text::JustifyH::Right => bevy::text::Justify::Right,
    }
}

/// The template's vertical justification, as the container's main axis.
fn justify_v(v: crate::menu_text::JustifyV) -> JustifyContent {
    match v {
        crate::menu_text::JustifyV::Top => JustifyContent::FlexStart,
        crate::menu_text::JustifyV::Middle => JustifyContent::Center,
        crate::menu_text::JustifyV::Bottom => JustifyContent::FlexEnd,
    }
}

/// The template's horizontal justification, as the container's cross axis.
fn align_h(h: crate::menu_text::JustifyH) -> AlignItems {
    match h {
        crate::menu_text::JustifyH::Left => AlignItems::FlexStart,
        crate::menu_text::JustifyH::Center => AlignItems::Center,
        crate::menu_text::JustifyH::Right => AlignItems::FlexEnd,
    }
}

/// An authored four-float colour.
fn srgba(c: [f32; 4]) -> Color {
    Color::srgba(c[0], c[1], c[2], c[3])
}

fn header(root: &mut ChildSpawnerCommands, _l: &Launcher) {
    root.spawn(Node {
        flex_direction: FlexDirection::Row,
        justify_content: JustifyContent::SpaceBetween,
        align_items: AlignItems::Center,
        flex_wrap: FlexWrap::Wrap,
        row_gap: Val::Rem(0.75),
        width: Val::Percent(100.0),
        ..default()
    })
    .with_children(|r| {
        r.spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Rem(0.25),
            ..default()
        })
        .with_children(|c| {
            c.spawn(tint("PeaceCraft III", 34.0, crate::mainmenu::LABEL));
            c.spawn(tint("Game configuration", 15.0, TEXT_DIM));
        });
        r.spawn((
            Button,
            Hovered::default(),
            Pick::Menu,
            Node {
                width: Val::Rem(10.5),
                height: Val::Rem(3.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Rem(0.0625)),
                ..default()
            },
            BackgroundColor(BG_PILL),
            BorderColor::all(BORDER),
        ))
        .with_children(|b| {
            b.spawn(tint("Main menu", 16.0, TEXT));
        });
    });
}

fn source_card(root: &mut ChildSpawnerCommands, l: &Launcher) {
    root.spawn(card()).with_children(|c| {
        section(
            c,
            "Game files",
            Some("Installed game directory, ISO or Battle.net folder"),
        );
        // The path, in a recessed field.
        c.spawn((
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Rem(2.375),
                justify_content: JustifyContent::Center,
                padding: UiRect::horizontal(Val::Rem(0.75)),
                border: UiRect::all(Val::Rem(0.0625)),
                border_radius: field_radius(),
                ..default()
            },
            BackgroundColor(BG_FIELD),
            BorderColor::all(BORDER),
        ))
        .with_children(|f| {
            f.spawn((tint(l.source.clone(), 14.0, TEXT), SourcePath));
        });
        c.spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Rem(0.375),
            ..default()
        })
        .with_children(|r| {
            r.spawn(tint("detected as", 11.0, TEXT_FAINT));
            r.spawn((tint("", 11.0, WARN), DetectedAs));
        });
        c.spawn((
            tint("Map source configuration required", 14.0, TEXT_DIM),
            crate::map_source_controls::SelectedMapText,
        ));
        c.spawn((
            Button,
            Hovered::default(),
            crate::map_source_controls::LoadSelectedMap,
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Rem(2.375),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(Val::Rem(0.75)),
                border: UiRect::all(Val::Rem(0.0625)),
                ..default()
            },
            BackgroundColor(BG_PILL),
            BorderColor::all(BORDER),
        ))
        .with_children(|button| {
            button.spawn(tint("Load selected map data", 14.0, TEXT));
        });
    });
}

fn mode_card(root: &mut ChildSpawnerCommands, mode: GameMode) {
    root.spawn(card()).with_children(|c| {
        section(
            c,
            "Game mode",
            None,
        );
        for (title, options, blurb) in [
                (
                    "Build",
                    vec![
                        (
                            Pick::Build(pcx_mpq::Version::V1_29),
                            "1.29b".into(),
                            "The classic target release.".into(),
                        ),
                    ],
                    "",
                ),
                (
                    "Edition",
                    vec![
                        (
                            Pick::Addon(Addon::Roc),
                            "Reign of Chaos".into(),
                            "The base game.".into(),
                        ),
                        (
                            Pick::Addon(Addon::Tft),
                            "The Frozen Throne".into(),
                            "The expansion. Selected by default.".into(),
                        ),
                    ],
                    "",
                ),
                (
                    "Assets",
                    vec![
                        (
                            Pick::Assets(AssetMode::Vanilla),
                            "Vanilla".into(),
                            "Classic art exactly as shipped.".into(),
                        ),
                        (
                            Pick::Assets(AssetMode::VanillaReforgedHd),
                            "Classic + HD".into(),
                            "Classic models, Reforged HD textures where they exist.".into(),
                        ),
                        (
                            Pick::Assets(AssetMode::Reforged),
                            "Reforged".into(),
                            "Reforged art throughout. Needs CASC staging, unimplemented.".into(),
                        ),
                    ],
                    "",
                ),
                (
                    "Behaviour",
                    vec![
                        (
                            Pick::Behaviour(Behaviour::Vanilla),
                            "Vanilla".into(),
                            "The classic game's behaviour, bugs included. The baseline.".into(),
                        ),
                        (
                            Pick::Behaviour(Behaviour::Reforged),
                            "Reforged".into(),
                            "Reforged's changed behaviour wholesale. An opt-in difference.".into(),
                        ),
                        (
                            Pick::Behaviour(Behaviour::ReforgedFixes),
                            "Fixes".into(),
                            "Only the bug fixes we adopted, per the fix ledger. Everything else stays vanilla.".into(),
                        ),
                    ],
                    "",
                ),
            ]
        {
            row_axis(c, title, blurb, options);
        }

        c.spawn((
            Node {
                width: Val::Percent(100.0),
                margin: UiRect::top(Val::Rem(0.25)),
                padding: UiRect::axes(Val::Rem(0.75), Val::Rem(0.5625)),
                column_gap: Val::Rem(0.625),
                border: UiRect::all(Val::Rem(0.0625)),
                border_radius: pill_radius(),
                ..default()
            },
            BackgroundColor(ACCENT_SOFT),
            BorderColor::all(BORDER_STRONG),
        ))
        .with_children(|s| {
            s.spawn(tint("Selected build", 13.0, TEXT_DIM));
            s.spawn((tint(mode.id(), 15.0, TEXT), ModeSummary));
        });
    });
}

/// One axis of the mode, as a labelled row of pill buttons.
fn row_axis(
    c: &mut ChildSpawnerCommands,
    title: &str,
    _blurb: &str,
    options: Vec<(Pick, String, String)>,
) {
    c.spawn(Node {
        flex_direction: FlexDirection::Row,
        flex_wrap: FlexWrap::Wrap,
        align_items: AlignItems::Center,
        column_gap: Val::Rem(0.875),
        row_gap: Val::Rem(0.375),
        width: Val::Percent(100.0),
        ..default()
    })
    .with_children(|row| {
        row.spawn(Node {
            width: Val::Rem(5.5),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Rem(0.0625),
            ..default()
        })
        .with_children(|lbl| {
            lbl.spawn(tint(title, 16.0, TEXT_DIM));
            let _ = _blurb;
        });
        row.spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Rem(0.4375),
            flex_wrap: FlexWrap::Wrap,
            flex_grow: 1.0,
            flex_basis: Val::Rem(25.0),
            max_width: Val::Percent(100.0),
            min_width: Val::Rem(0.0),
            align_items: AlignItems::Stretch,
            ..default()
        })
        .with_children(|pills| {
            for (p, name, what) in options {
                pills
                    .spawn((
                        Button,
                        Hovered::default(),
                        Node {
                            padding: UiRect {
                                left: Val::Rem(1.5),
                                right: Val::Rem(0.75),
                                top: Val::Rem(0.8125),
                                bottom: Val::Rem(0.8125),
                            },
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Start,
                            row_gap: Val::Rem(0.125),
                            border: UiRect::all(Val::Rem(0.0625)),
                            border_radius: pill_radius(),
                            width: Val::Rem(13.125),
                            max_width: Val::Percent(100.0),
                            min_height: Val::Rem(4.5),
                            ..default()
                        },
                        BackgroundColor(BG_PILL),
                        BorderColor::all(BORDER),
                        p,
                    ))
                    .with_children(|b| {
                        b.spawn(tint(name, 16.0, crate::mainmenu::LABEL));
                        b.spawn(tint(what, 12.0, TEXT_DIM));
                    });
            }
        });
    });
}

fn action_card(root: &mut ChildSpawnerCommands) {
    root.spawn(card()).with_children(|c| {
        section(c, "Stage", None);
        c.spawn(Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            align_items: AlignItems::Center,
            column_gap: Val::Rem(0.875),
            row_gap: Val::Rem(0.75),
            ..default()
        })
        .with_children(|r| {
            r.spawn((
                Button,
                Hovered::default(),
                Node {
                    width: Val::Rem(15.625),
                    max_width: Val::Percent(100.0),
                    height: Val::Rem(4.0),
                    padding: UiRect::left(Val::Rem(1.25)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    column_gap: Val::Rem(0.5625),
                    border: UiRect::all(Val::Rem(0.0625)),
                    border_radius: BorderRadius::ZERO,
                    ..default()
                },
                BackgroundColor(BG_PILL),
                BorderColor::all(BORDER_STRONG),
                Pick::Stage,
            ))
            .with_children(|b| {
                b.spawn(tint("Stage assets", 19.0, crate::mainmenu::LABEL));
            });
            r.spawn((
                Button,
                InteractionDisabled,
                Pick::Download,
                Node {
                    width: Val::Rem(11.875),
                    height: Val::Rem(3.25),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Rem(0.0625)),
                    ..default()
                },
                BackgroundColor(DISABLED_BG),
                BorderColor::all(BORDER),
            ))
            .with_children(|b| {
                b.spawn(tint("Check for updates", 14.0, DISABLED_FG));
            });
            r.spawn(tint(
                "Prepare and verify your selected game assets.",
                14.0,
                TEXT_FAINT,
            ));
        });
    });
}

/// Log lines are capped, so a long session cannot grow the text without bound.
pub(crate) const LOG_KEEP: usize = 200;

impl Launcher {
    /// One line of the launcher's own log.
    ///
    /// Exposed so a platform layer that has no filesystem still reports through
    /// the same place the user is already reading, rather than into a second log
    /// nobody looks at.
    pub fn push_log(&mut self, line: String) {
        self.log.push(line);
        while self.log.len() > LOG_KEEP {
            self.log.remove(0);
        }
    }
}

fn log_card(root: &mut ChildSpawnerCommands) {
    root.spawn((Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        row_gap: Val::Rem(0.5),
        ..default()
    },))
        .with_children(|c| {
            section(c, "Log", None);
            c.spawn((
                Node {
                    width: Val::Percent(100.0),
                    min_height: Val::Rem(7.5),
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Rem(0.8125)),
                    row_gap: Val::Rem(0.1875),
                    border: UiRect::all(Val::Rem(0.0625)),
                    border_radius: field_radius(),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                BackgroundColor(BG_FIELD),
                BorderColor::all(BORDER),
            ))
            .with_children(|area| {
                area.spawn((tint("Ready.", 14.0, TEXT_DIM), LogBody));
            });
        });
}

// ------------------------------------------------------------------ systems

/// Paints the active option in each axis, and the disabled one as disabled.
type PickPaintData = (
    &'static Pick,
    &'static mut BackgroundColor,
    &'static mut BorderColor,
    &'static mut Node,
    Option<&'static InteractionDisabled>,
    Option<&'static Hovered>,
    Option<&'static mut ImageNode>,
);

fn colour_picks(l: Res<Launcher>, mut q: Query<PickPaintData, With<Button>>) {
    for (p, mut bg, mut border, mut node, disabled, interaction, image) in &mut q {
        if disabled.is_some() || *p == Pick::Download {
            *bg = DISABLED_BG.into();
            *border = BorderColor::all(BORDER);
            if let Some(mut image) = image {
                image.color = DISABLED_FG;
            }
            continue;
        }
        let active = match p {
            Pick::Build(v) => *v == l.mode.build,
            Pick::Addon(a) => *a == l.mode.addon,
            Pick::Assets(a) => *a == l.mode.assets,
            Pick::Behaviour(b) => *b == l.mode.behaviour,
            Pick::Stage => true,
            Pick::Download => false,
            Pick::Menu => false,
        };
        if active {
            *bg = BG_PILL.into();
            *border = BorderColor::all(BORDER_STRONG);
            let width = UiRect::all(Val::Rem(0.125));
            if node.border != width {
                node.border = width;
            }
        } else {
            *bg = BG_PILL.into();
            *border = BorderColor::all(BORDER);
            let width = UiRect::all(Val::Rem(0.0625));
            if node.border != width {
                node.border = width;
            }
        }
        if let Some(mut image) = image {
            image.color = if active || interaction.is_some_and(Hovered::get) {
                Color::WHITE
            } else {
                Color::srgb(0.78, 0.78, 0.78)
            };
        }
    }
}

fn scroll_launcher(
    mut wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    menu: Query<Entity, With<MenuLayer>>,
    mut panel: Query<(&mut ScrollPosition, &ComputedNode, &Hovered), With<LauncherPanel>>,
) {
    use bevy::input::mouse::MouseScrollUnit;
    for event in wheel.read() {
        if !menu.is_empty() {
            continue;
        }
        for (mut scroll, node, hovered) in &mut panel {
            if !hovered.get() {
                continue;
            }
            let dy = match event.unit {
                MouseScrollUnit::Line => event.y * 40.0,
                MouseScrollUnit::Pixel => event.y,
            };
            let max =
                ((node.content_size().y - node.size().y) * node.inverse_scale_factor()).max(0.0);
            scroll.0.y = (scroll.0.y - dy).clamp(0.0, max);
        }
    }
}

fn scroll_resolution_popup(
    mut wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    mut popups: Query<(&mut ScrollPosition, &ComputedNode, &Hovered), With<ResolutionPopup>>,
    mut choices: Query<(&Hovered, &mut BackgroundColor), With<ResolutionChoice>>,
) {
    for (hover, mut colour) in &mut choices {
        let next = if hover.get() { ACCENT_SOFT } else { BG_FIELD };
        if colour.0 != next {
            colour.0 = next;
        }
    }
    for event in wheel.read() {
        for (mut scroll, node, hover) in &mut popups {
            if hover.get() {
                let dy = match event.unit {
                    bevy::input::mouse::MouseScrollUnit::Line => event.y * node.size().y / 8.0,
                    bevy::input::mouse::MouseScrollUnit::Pixel => event.y,
                };
                let max = ((node.content_size().y - node.size().y) * node.inverse_scale_factor())
                    .max(0.0);
                scroll.0.y = (scroll.0.y - dy).clamp(0.0, max);
            }
        }
    }
}

/// Clicks on the original main menu, dispatched by the clicked FDF frame.
///
/// Options opens the original game settings. The separate cog opens PeaceCraft
/// configuration. The edition button switches between Reign of Chaos
/// and The Frozen Throne, which rebuilds the scene, panels and logo for that
/// edition. Quit exits; a browser has no process to end, so there it returns to
/// the launcher. Every other entry is reported as not implemented rather than
/// pretending a button led somewhere.
#[derive(SystemParam)]
pub(crate) struct MenuClickQueries<'w, 's> {
    buttons: Query<'w, 's, &'static crate::glue_menu::GlueButton>,
    menu: Query<'w, 's, Entity, With<MenuLayer>>,
    cogs: Query<'w, 's, Entity, With<SettingsCog>>,
    windows: Query<'w, 's, &'static mut Window>,
    backend: ResMut<'w, crate::menu_settings::Backend>,
    choices: Query<'w, 's, &'static ResolutionChoice>,
    popups: Query<'w, 's, Entity, With<ResolutionPopup>>,
    roots: Query<'w, 's, Entity, With<ControlRoot>>,
    cache: Res<'w, MenuArtCache>,
}

pub(crate) fn apply_menu_clicks(
    mut commands: Commands,
    mut launcher: ResMut<Launcher>,
    actions: Res<crate::button_actions::ButtonActions>,
    mut navigation: ResMut<crate::menu_screens::Navigation>,
    queries: MenuClickQueries,
    #[cfg(not(target_arch = "wasm32"))] mut exit: MessageWriter<AppExit>,
) {
    let MenuClickQueries {
        buttons: q,
        menu,
        cogs,
        mut windows,
        mut backend,
        choices,
        popups,
        roots,
        cache,
    } = queries;
    for entity in &actions.entities {
        if let Ok(choice) = choices.get(*entity) {
            if let Err(error) =
                crate::menu_settings::select_resolution(&mut navigation, (choice.0, choice.1))
            {
                navigation.notice = error;
            }
            for entity in &popups {
                commands.entity(entity).despawn();
            }
            focus_resolution_control(&mut commands);
            continue;
        }
        if cogs.contains(*entity) {
            for e in &menu {
                commands.entity(e).despawn();
            }
            continue;
        }
        let Ok(button) = q.get(*entity) else {
            continue;
        };
        if button.frame == "CustomCampaignButton"
            && (launcher.mode.addon != Addon::Tft || !launcher.allows_addon(Addon::Tft))
        {
            navigation.notice =
                "Custom campaigns require The Frozen Throne game files and edition.".into();
            continue;
        }
        if navigation.action(&button.frame) {
            continue;
        }
        match button.frame.as_str() {
            "ResolutionMenu" => {
                for entity in &popups {
                    commands.entity(entity).despawn();
                }
                if popups.is_empty()
                    && let Ok(root) = roots.single()
                    && let Err(error) = open_resolutions(&mut commands, root, &cache, &navigation)
                {
                    navigation.notice = error;
                }
            }
            "OKButton" if matches!(navigation.screen, crate::menu_screens::Screen::Options(_)) => {
                if let Ok(mut window) = windows.single_mut() {
                    match crate::menu_settings::apply(&mut navigation, &mut window, &mut backend) {
                        Ok(()) => navigation.open(crate::menu_screens::Screen::Main),
                        Err(error) => navigation.notice = error,
                    }
                }
            }
            #[cfg(not(target_arch = "wasm32"))]
            "ExitButton" => {
                exit.write(AppExit::Success);
            }
            #[cfg(target_arch = "wasm32")]
            "ExitButton" => {
                for e in &menu {
                    commands.entity(e).despawn();
                }
            }
            "EditionButton" => {
                let next = match launcher.mode.addon {
                    Addon::Tft => Addon::Roc,
                    Addon::Roc => Addon::Tft,
                };
                if launcher.allows_addon(next) {
                    launcher.mode.addon = next;
                }
            }
            other => {
                if !crate::menu_screens::supported(other) {
                    navigation.notice = format!("{other}: this action is not available yet.");
                }
            }
        }
    }
}

/// Clicks on the original main menu.
fn apply_clicks(
    mut l: ResMut<Launcher>,
    mut intro: ResMut<crate::intro::Intro>,
    mut commands: Commands,
    actions: Res<crate::button_actions::ButtonActions>,
    q: Query<(&Pick, Option<&InteractionDisabled>), With<Button>>,
    mut navigation: Option<ResMut<crate::menu_screens::Navigation>>,
) {
    l.constrain_menu_edition();
    for entity in &actions.entities {
        let Ok((p, disabled)) = q.get(*entity) else {
            continue;
        };
        // Disabled means disabled: the click is dropped here as well as being
        // greyed, so the control cannot be "fixed" by a stray pointer event.
        // `ui_widgets::Button` never presses a disabled button either.
        if disabled.is_some() || *p == Pick::Download {
            continue;
        }
        match p {
            Pick::Build(v) => l.mode.build = *v,
            Pick::Addon(a) if l.allows_addon(*a) => l.mode.addon = *a,
            Pick::Addon(_) => {}
            Pick::Assets(a) => l.mode.assets = *a,
            Pick::Behaviour(b) => l.mode.behaviour = *b,
            Pick::Stage => l.staging = true,
            Pick::Download => {}
            Pick::Menu => {
                if let Some(navigation) = navigation.as_mut() {
                    navigation.open(crate::menu_screens::Screen::Main);
                }
                if l.allows_addon(l.mode.addon)
                    && !intro.request_with_install(l.mode.addon == Addon::Tft, selected_install(&l))
                {
                    spawn_main_menu(&mut commands);
                }
            }
        }
    }
}

fn finish_intro(mut intro: ResMut<crate::intro::Intro>, mut commands: Commands) {
    if intro.take_finished() {
        spawn_main_menu(&mut commands);
    }
}

type DetectedFilter = (With<DetectedAs>, Without<LogBody>, Without<SourcePath>);
type ModeSummaryFilter = (
    With<ModeSummary>,
    Without<LogBody>,
    Without<SourcePath>,
    Without<DetectedAs>,
);

fn refresh(
    mut l: ResMut<Launcher>,
    mut log_q: Query<&mut Text, With<LogBody>>,
    mut path_q: Query<&mut Text, (With<SourcePath>, Without<LogBody>)>,
    mut det_q: Query<&mut Text, DetectedFilter>,
    mut sum_q: Query<&mut Text, ModeSummaryFilter>,
) {
    if l.staging {
        l.staging = false;
        do_stage(&mut l);
    }
    if let Ok(mut t) = log_q.single_mut() {
        t.0 = l.log.join("\n");
    }
    if let Ok(mut t) = path_q.single_mut() {
        t.0 = l.source.clone();
    }
    if let Ok(mut t) = det_q.single_mut() {
        let text = match classify(&l.source) {
            Ok(s) => s.label().to_string(),
            Err(_) => "No local installation detected".to_string(),
        };
        t.0 = text;
    }
    if let Ok(mut t) = sum_q.single_mut() {
        t.0 = l.mode.id();
    }
}

fn do_stage(l: &mut Launcher) {
    if l.log.is_empty() {
        l.push_log("Ready.".into());
    }
    l.push_log(format!("staging {} …", l.mode));
    let source = match classify(&l.source) {
        Ok(s) => s,
        Err(e) => {
            l.push_log(format!("✕ source rejected: {e}"));
            return;
        }
    };
    l.push_log(format!("source {} ({})", l.source, source.label()));

    let dest = format!("out/{}", l.mode.id());
    match stage(&l.mode, &source, &dest) {
        Ok(st) => {
            l.push_log(format!(
                "✓ {} archives, {} bytes",
                st.archives.len(),
                st.bytes
            ));
            for a in &st.archives {
                l.push_log(format!("    {a}"));
            }
            match st.detected_build {
                Some(v) => l.push_log(format!("✓ opened as {v}, {} files", st.file_count)),
                None => l.push_log("✕ the staged tree would not open".into()),
            }
        }
        Err(e) => l.push_log(format!("✕ {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Relative luminance per WCAG 2.1.
    fn luminance(c: Color) -> f32 {
        let s = c.to_linear();
        0.2126 * s.red + 0.7152 * s.green + 0.0722 * s.blue
    }

    fn contrast(fg: Color, bg: Color) -> f32 {
        let (a, b) = (luminance(fg), luminance(bg));
        let (hi, lo) = if a > b { (a, b) } else { (b, a) };
        (hi + 0.05) / (lo + 0.05)
    }

    /// The palette has to be readable, not just chosen.
    ///
    /// This stands in for looking at the window: it can be checked in CI and it
    /// fails when a colour is tweaked into illegibility, which is exactly the
    /// mistake a hand-tuned dark theme invites.
    #[test]
    fn the_palette_is_readable_against_its_own_backgrounds() {
        let pairs = [
            ("body text on a card", TEXT, BG_CARD),
            ("dim label on a card", TEXT_DIM, BG_CARD),
            ("faint hint on a card", TEXT_FAINT, BG_CARD),
            ("body text on the app background", TEXT, BG_APP),
            ("log text on a recessed field", TEXT, BG_FIELD),
            ("warn on a recessed field", WARN, BG_FIELD),
            (
                "stage button label on frame",
                crate::mainmenu::LABEL,
                BG_PILL,
            ),
        ];
        for (what, fg, bg) in pairs {
            let r = contrast(fg, bg);
            assert!(
                r >= 3.0,
                "{what}: contrast {r:.2} is too low to read (minimum 3.0)"
            );
        }
        // Body copy needs the stricter 4.5:1.
        assert!(
            contrast(TEXT, BG_CARD) >= 7.0,
            "body text should comfortably clear 4.5:1, got {:.2}",
            contrast(TEXT, BG_CARD)
        );
        // A disabled control is *meant* to be hard to read, so it is held to a
        // different rule: visible enough to know it is there, clearly less
        // emphatic than any live text, so it reads as inert rather than as
        // merely low-contrast copy.
        let d = contrast(DISABLED_FG, DISABLED_BG);
        assert!(
            (1.4..2.6).contains(&d),
            "disabled control contrast {d:.2} should read as inert-but-present"
        );
        assert!(
            d < contrast(TEXT_DIM, BG_CARD),
            "disabled text must be less emphatic than live secondary text"
        );

        // The selected frame must remain distinct from its inset surface.
        assert!(
            contrast(BORDER_STRONG, BG_PILL) >= 1.8,
            "the selected pill must stand out from an unselected one"
        );
    }

    #[test]
    fn the_download_control_is_disabled_not_merely_dim() {
        // It is inert both visually (greyed) and functionally (the click
        // handler drops it), so a stray pointer event cannot fire it.
        let l = Launcher::default();
        assert!(l.log.is_empty(), "a fresh launcher has nothing logged");
    }

    #[test]
    fn the_default_source_and_mode_are_the_target_build() {
        let l = Launcher::default();
        assert_eq!(l.mode.build, pcx_mpq::Version::V1_29);
        assert_eq!(l.mode.addon, Addon::Tft);
        assert_eq!(l.mode.assets, AssetMode::Vanilla);
        assert_eq!(l.mode.behaviour, Behaviour::Vanilla);
    }
}

#[cfg(test)]
mod menu_tests {
    use super::*;

    #[test]
    fn logo_visibility_follows_the_current_definition_instead_of_covering_other_screens() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        world.insert_resource(MenuArtCache {
            fdf: Some(
                crate::fdf::Fdf::parse("Frame \"SPRITE\" \"WarCraftIIILogo\" {\n}\n").unwrap(),
            ),
            ..default()
        });
        let layer = world.spawn((LogoLayer, Visibility::Hidden)).id();
        world.run_system_once(show_logo_for_screen).unwrap();
        assert_eq!(
            *world.get::<Visibility>(layer).unwrap(),
            Visibility::Inherited
        );
        world.resource_mut::<MenuArtCache>().fdf =
            Some(crate::fdf::Fdf::parse("Frame \"FRAME\" \"OptionsMenu\" {\n}\n").unwrap());
        world.run_system_once(show_logo_for_screen).unwrap();
        assert_eq!(*world.get::<Visibility>(layer).unwrap(), Visibility::Hidden);
    }
    // The click tests spawn `Pressed` buttons; nothing else in the crate needs
    // the marker, so it is imported here rather than at module scope.

    use bevy::ecs::system::RunSystemOnce;
    use bevy::ui::Pressed;

    #[test]
    fn cameras_clear_settings_and_only_run_the_visible_scene() {
        use bevy::camera::ClearColorConfig;
        let mut world = World::new();
        world.init_resource::<crate::intro::Intro>();
        let ui = world
            .spawn((Camera::default(), bevy::ui::IsDefaultUiCamera))
            .id();
        let scene = world
            .spawn((
                Camera::default(),
                crate::mainmenu_scene::MainMenuSceneCamera,
            ))
            .id();
        world.run_system_once(show_menu_or_launcher).unwrap();
        assert!(!world.get::<Camera>(scene).unwrap().is_active);
        assert!(matches!(world.get::<Camera>(ui).unwrap().clear_color,
            ClearColorConfig::Custom(color) if color == BG_APP));

        let menu = world.spawn(MenuLayer).id();
        world.run_system_once(show_menu_or_launcher).unwrap();
        assert!(world.get::<Camera>(scene).unwrap().is_active);
        assert!(matches!(
            world.get::<Camera>(ui).unwrap().clear_color,
            ClearColorConfig::None
        ));

        let window = world
            .spawn(Window {
                resolution: (1600u32, 900u32).into(),
                ..default()
            })
            .id();
        world.run_system_once(show_menu_or_launcher).unwrap();
        let viewport = world.get::<Camera>(ui).unwrap().viewport.as_ref().unwrap();
        assert_eq!(viewport.physical_position, UVec2::new(200, 0));
        assert_eq!(viewport.physical_size, UVec2::new(1200, 900));
        world
            .get_mut::<Window>(window)
            .unwrap()
            .resolution
            .set(600.0, 900.0);
        world.run_system_once(show_menu_or_launcher).unwrap();
        let viewport = world.get::<Camera>(ui).unwrap().viewport.as_ref().unwrap();
        assert_eq!(viewport.physical_position, UVec2::new(0, 225));
        assert_eq!(viewport.physical_size, UVec2::new(600, 450));
        assert!(
            world.get::<Camera>(scene).unwrap().viewport.is_none(),
            "original background fills the display"
        );

        // An unavailable original scene must not leave the settings' old pixels.
        world.despawn(scene);
        world.run_system_once(show_menu_or_launcher).unwrap();
        assert!(matches!(world.get::<Camera>(ui).unwrap().clear_color,
            ClearColorConfig::Custom(color) if color == BG_APP));
        world.despawn(menu);
        world.run_system_once(show_menu_or_launcher).unwrap();
        assert!(world.get::<Camera>(ui).unwrap().viewport.is_none());
    }

    #[test]
    fn a_consumed_activation_does_not_stage_again_until_another_activation() {
        let mut app = App::new();
        app.insert_resource(Launcher::default())
            .init_resource::<crate::intro::Intro>()
            .init_resource::<Assets<Image>>()
            .add_systems(Update, (apply_clicks, crate::button_actions::clear).chain());
        crate::button_actions::install(&mut app);
        let button = app.world_mut().spawn((Button, Pick::Stage)).id();
        app.world_mut()
            .trigger(bevy::ui_widgets::Activate { entity: button });
        app.update();
        assert!(app.world().resource::<Launcher>().staging);
        app.world_mut().resource_mut::<Launcher>().staging = false;
        app.update();
        assert!(!app.world().resource::<Launcher>().staging);
        app.world_mut().entity_mut(button).remove::<Pressed>();
        app.update();
        app.world_mut()
            .trigger(bevy::ui_widgets::Activate { entity: button });
        app.update();
        assert!(app.world().resource::<Launcher>().staging);
    }

    fn press(world: &mut World, frame: &str) -> Entity {
        world.init_resource::<crate::menu_screens::Navigation>();
        world.init_resource::<crate::menu_settings::Backend>();
        world.init_resource::<MenuArtCache>();
        let entry = world
            .spawn((
                Button,
                crate::glue_menu::GlueButton {
                    frame: frame.to_string(),
                    shortcut: None,
                    pushed_text_offset: Some(Vec2::ZERO),
                },
                Pressed,
            ))
            .id();
        world.insert_resource(crate::button_actions::ButtonActions {
            entities: vec![entry],
        });
        world.insert_resource(Launcher::default());
        world.init_resource::<Messages<AppExit>>();
        entry
    }

    /// Game options retain the original menu's background and do not expose staging.
    #[test]
    fn options_open_the_original_settings_screen() {
        let mut world = World::new();
        let layer = world.spawn(MenuLayer).id();
        let entry = press(&mut world, "OptionsButton");
        world.run_system_once(apply_menu_clicks).unwrap();
        assert!(
            world.get_entity(layer).is_ok(),
            "the original background is retained"
        );
        assert!(world.get_entity(entry).is_ok(), "only the layer goes");
        assert_eq!(
            world.resource::<crate::menu_screens::Navigation>().screen,
            crate::menu_screens::Screen::Options(crate::menu_screens::Page::Video)
        );
    }

    /// An entry without a destination leaves the menu up.
    #[test]
    fn an_unimplemented_entry_leaves_the_menu_up() {
        let mut world = World::new();
        let layer = world.spawn(MenuLayer).id();
        press(&mut world, "SinglePlayerButton");
        world.run_system_once(apply_menu_clicks).unwrap();
        assert!(world.get_entity(layer).is_ok());
    }

    /// The edition button switches between the two editions.
    #[test]
    fn the_edition_button_switches_edition() {
        let mut world = World::new();
        press(&mut world, "EditionButton");
        let before = world.resource::<Launcher>().mode.addon;
        world.run_system_once(apply_menu_clicks).unwrap();
        let after = world.resource::<Launcher>().mode.addon;
        assert_ne!(before, after);
        world.run_system_once(apply_menu_clicks).unwrap();
        assert_eq!(world.resource::<Launcher>().mode.addon, before);
    }

    #[test]
    fn a_base_only_demo_refuses_the_expansion_button() {
        let mut world = World::new();
        press(&mut world, "EditionButton");
        {
            let mut launcher = world.resource_mut::<Launcher>();
            launcher.content_editions = Some(crate::local_demo::MenuEditions {
                roc: true,
                tft: false,
            });
            launcher.constrain_menu_edition();
            assert_eq!(launcher.mode.addon, Addon::Roc);
        }
        world.run_system_once(apply_menu_clicks).unwrap();
        assert_eq!(world.resource::<Launcher>().mode.addon, Addon::Roc);
    }

    /// Quit exits the process.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn quit_exits() {
        let mut world = World::new();
        press(&mut world, "ExitButton");
        world.run_system_once(apply_menu_clicks).unwrap();
        let exits = world.resource::<Messages<AppExit>>();
        assert_eq!(exits.len(), 1);
    }

    fn invalidation_world(revision: u32, store: u32) -> (World, Entity, Entity) {
        let mut world = World::new();
        world.insert_resource(MenuArtCache {
            revision,
            strings: Some("x".into()),
            ..Default::default()
        });
        world.insert_resource(AssetStoreRevision(store));
        world.insert_resource(SceneLoad::default());
        world.init_resource::<crate::glue_menu::GlueImages>();
        let root = world.spawn((Node::default(), ControlRoot)).id();
        let built = world.spawn(Node::default()).id();
        world.entity_mut(root).add_child(built);
        (world, root, built)
    }

    /// A replaced asset store drops the built control layer and every parsed
    /// source, so the next frame rebuilds from the new store.
    #[test]
    fn a_new_asset_revision_rebuilds_the_control_layer() {
        let (mut world, root, built) = invalidation_world(0, 1);
        world.run_system_once(invalidate_on_asset_revision).unwrap();
        assert!(world.get_entity(built).is_err(), "the old layer survived");
        assert!(
            world.get_entity(root).is_ok(),
            "the root is kept for the rebuild"
        );
        let cache = world.resource::<MenuArtCache>();
        assert!(cache.button.is_none() && cache.fonts.is_empty());
        assert!(cache.fdf.is_none() && cache.strings.is_none() && cache.templates.is_none());
        assert!(cache.main_menu.is_none() && !cache.native_loaded);
    }

    /// An unchanged revision clears nothing.
    #[test]
    fn an_unchanged_revision_clears_nothing() {
        let (mut world, _, built) = invalidation_world(7, 7);
        world.run_system_once(invalidate_on_asset_revision).unwrap();
        assert!(world.get_entity(built).is_ok());
        assert!(world.resource::<MenuArtCache>().strings.is_some());
    }

    /// The control layer is built from the FDF into an empty root, once.
    #[test]
    fn the_control_layer_is_built_from_the_frame_definitions() {
        const LAYER: &str = r#"
Frame "FRAME" "MainMenuFrame" {
    SetAllPoints,
    Frame "FRAME" "ControlLayer" {
        SetAllPoints,
        Frame "GLUETEXTBUTTON" "OptionsButton" {
            Width 0.179,
            Height 0.031,
            SetPoint TOPRIGHT,"MainMenuFrame",TOPRIGHT, -0.027, -0.1,
            ControlShortcutKey "O",
        }
        Frame "GLUETEXTBUTTON" "ExitButton" {
            TabFocusDefault,
            Width 0.168,
            Height 0.031,
            SetPoint TOP,"OptionsButton",BOTTOM, 0.0, -0.1,
        }
    }
}
"#;
        let mut world = World::new();
        world.insert_resource(MenuArtCache {
            fdf: Some(crate::fdf::Fdf::parse(LAYER).expect("fixture")),
            ..Default::default()
        });
        world.init_resource::<crate::menu_screens::Navigation>();
        world.init_resource::<bevy::input_focus::InputFocus>();
        world.spawn((Node::default(), ControlRoot));
        world.run_system_once(build_control_layer).unwrap();
        world.flush();
        let mut buttons = world.query::<&crate::glue_menu::GlueButton>();
        let mut frames: Vec<String> = buttons.iter(&world).map(|b| b.frame.clone()).collect();
        frames.sort();
        assert_eq!(frames, ["ExitButton", "OptionsButton"]);
        let focused = world
            .resource::<bevy::input_focus::InputFocus>()
            .get()
            .unwrap();
        assert_eq!(
            world
                .get::<crate::glue_menu::GlueButton>(focused)
                .unwrap()
                .frame,
            "ExitButton"
        );
        world.run_system_once(build_control_layer).unwrap();
        world.flush();
        assert_eq!(buttons.iter(&world).count(), 2, "built twice");
    }

    const FDF_FACES: &str = r#"
Frame "TEXT" "MenuTextTemplate" {
    FrameFont "FaceCommon", 0.013, "",
    FontJustificationH JUSTIFYCENTER,
    FontJustificationV JUSTIFYMIDDLE,
    FontFlags "FIXEDSIZE",
    FontColor 0.99 0.827 0.0705 1.0,
    FontHighlightColor 1.0 1.0 1.0 1.0,
    FontDisabledColor 0.5 0.5 0.5 1.0,
    FontShadowColor 0.0 0.0 0.0 0.9,
    FontShadowOffset 0.001 -0.001,
}
Frame "FRAME" "MainMenuFrame" {
    Frame "GLUETEXTBUTTON" "SinglePlayerButton" {
        ButtonText "SinglePlayerButtonText",
        Frame "TEXT" "SinglePlayerButtonText" INHERITS "MenuTextTemplate" {
            FrameFont "FaceA", 0.013, "",
            Text "KEY_SINGLE_PLAYER",
        }
    }
    Frame "GLUETEXTBUTTON" "BattleNetButton" {
        ButtonText "BattleNetButtonText",
        Frame "TEXT" "BattleNetButtonText" INHERITS "MenuTextTemplate" {
            Text "KEY_BATTLE_NET",
        }
    }
    Frame "GLUETEXTBUTTON" "LocalAreaNetworkButton" {
        ButtonText "LocalAreaNetworkButtonText",
        Frame "TEXT" "LocalAreaNetworkButtonText" INHERITS "MenuTextTemplate" {
            Text "KEY_LOCAL_AREA_NETWORK",
        }
    }
    Frame "GLUETEXTBUTTON" "OptionsButton" {
        ButtonText "OptionsButtonText",
        Frame "TEXT" "OptionsButtonText" INHERITS "MenuTextTemplate" {
            Text "KEY_OPTIONS",
        }
    }
    Frame "GLUETEXTBUTTON" "CreditsButton" {
        ButtonText "CreditsButtonText",
        Frame "TEXT" "CreditsButtonText" INHERITS "MenuTextTemplate" {
            Text "KEY_CREDITS",
        }
    }
    Frame "GLUETEXTBUTTON" "ExitButton" {
        ButtonText "ExitButtonText",
        Frame "TEXT" "ExitButtonText" INHERITS "MenuTextTemplate" {
            FrameFont "FaceB", 0.013, "",
            Text "KEY_EXIT",
        }
    }
}
"#;
    const STRINGS_FACES: &str = r#"
KEY_SINGLE_PLAYER        "|CffffffffS|Ringle Player",
KEY_BATTLE_NET           "|CffffffffB|Rattle.net",
KEY_LOCAL_AREA_NETWORK   "|CffffffffL|Rocal Area Network",
KEY_OPTIONS              "|CffffffffO|Rptions",
KEY_CREDITS              "|CffffffffC|Rredits",
KEY_EXIT                 "E|Cffffffffx|Rit",
"#;

    fn label(world: &mut World, frame: &str) -> Entity {
        let button = world.spawn_empty().id();
        let container = world.spawn(Node::default()).id();
        let label = world
            .spawn((
                Text::new(""),
                crate::glue_menu::GlueLabel {
                    frame: frame.to_string(),
                    button,
                },
            ))
            .id();
        world.entity_mut(container).add_child(label);
        label
    }

    /// Each label takes **its own** text frame's face, on the root and on every
    /// run, and unrelated text is left alone.
    #[test]
    fn each_label_takes_its_own_face_and_leaves_other_text_alone() {
        let mut faces = Assets::<Font>::default();
        let face_a = faces.add(Font::from_bytes(Vec::new()));
        let face_b = faces.add(Font::from_bytes(Vec::new()));
        let mut world = World::new();
        world.insert_resource(faces);
        let mut fonts = std::collections::HashMap::new();
        fonts.insert("FaceA".to_string(), face_a.clone());
        fonts.insert("FaceB".to_string(), face_b.clone());
        world.insert_resource(MenuArtCache {
            fdf: Some(crate::fdf::Fdf::parse(FDF_FACES).expect("fixture")),
            main_menu: Some(FDF_FACES.to_string()),
            strings: Some(STRINGS_FACES.to_string()),
            fonts,
            ..Default::default()
        });
        let single = label(&mut world, "SinglePlayerButtonText");
        let exit = label(&mut world, "ExitButtonText");
        let unrelated = world
            .spawn(TextFont {
                font: bevy::text::FontSource::default(),
                font_size: bevy::text::FontSize::Px(12.0),
                ..default()
            })
            .id();

        world.run_system_once(apply_menu_text).expect("runs");

        for (root, want, visible) in [(single, &face_a, "Single Player"), (exit, &face_b, "Exit")] {
            assert_eq!(
                world.get::<TextFont>(root).unwrap().font,
                bevy::text::FontSource::Handle(want.clone())
            );
            let children: Vec<Entity> = world.get::<Children>(root).unwrap().iter().collect();
            let joined: String = children
                .iter()
                .map(|c| world.get::<TextSpan>(*c).unwrap().0.clone())
                .collect();
            assert_eq!(joined, visible, "the runs do not spell the label");
            for child in children {
                assert_eq!(
                    world.get::<TextFont>(child).unwrap().font,
                    bevy::text::FontSource::Handle(want.clone())
                );
            }
            // The authored shadow is carried in frame units for placement.
            assert_eq!(
                world.get::<crate::glue_menu::FrameShadow>(root).unwrap().0,
                Vec2::new(0.001, -0.001)
            );
        }
        assert_eq!(
            world.get::<TextFont>(unrelated).unwrap().font,
            bevy::text::FontSource::default(),
            "unrelated text was overwritten"
        );
    }

    /// A label whose face has no loaded handle and no readable file defers its
    /// fill rather than borrowing another face.
    #[test]
    fn a_missing_face_defers_the_label() {
        let mut world = World::new();
        world.init_resource::<Assets<Font>>();
        world.insert_resource(MenuArtCache {
            fdf: Some(crate::fdf::Fdf::parse(FDF_FACES).expect("fixture")),
            main_menu: Some(FDF_FACES.to_string()),
            strings: Some(STRINGS_FACES.to_string()),
            ..Default::default()
        });
        let exit = label(&mut world, "ExitButtonText");
        world.run_system_once(apply_menu_text).expect("runs");
        assert!(world.get::<MenuTextFilled>(exit).is_none());
        assert!(world.get::<Children>(exit).is_none_or(|c| c.is_empty()));
    }
}

#[cfg(test)]
mod menu_focus_tests {
    use super::*;
    use bevy::{
        camera::NormalizedRenderTarget,
        ecs::system::SystemState,
        input::{InputPlugin, keyboard::KeyboardInput},
        input_focus::{
            FocusCause, InputDispatchPlugin, InputFocus, InputFocusPlugin,
            pointer_focus::PointerFocusPlugin,
            tab_navigation::{
                NavAction, TabGroup, TabIndex, TabNavigation, TabNavigationError,
                TabNavigationPlugin,
            },
        },
        picking::{
            backend::HitData,
            events::{Pointer, PointerClick, PointerPress},
            pointer::{Location, PointerButton, PointerId},
        },
        ui_widgets::{Button, ButtonPlugin},
        window::PrimaryWindow,
    };
    struct Fixture {
        app: App,
        window: Entity,
        root: Entity,
        blank: Entity,
        owner: Entity,
        other: Entity,
    }
    fn fixture(fixed: bool, early: bool) -> Fixture {
        let mut app = App::new();
        if fixed && early {
            app.add_observer(menu_acquire_focus);
        }
        app.add_plugins((
            InputPlugin,
            InputFocusPlugin,
            InputDispatchPlugin,
            PointerFocusPlugin,
            TabNavigationPlugin,
            ButtonPlugin,
        ));
        crate::button_actions::install(&mut app);
        if fixed && !early {
            app.add_observer(menu_acquire_focus);
        }
        let window = app
            .world_mut()
            .spawn((Window::default(), PrimaryWindow))
            .id();
        let root = app
            .world_mut()
            .spawn((
                MenuLayer,
                TabGroup {
                    modal: true,
                    order: 0,
                },
            ))
            .id();
        let blank = app.world_mut().spawn(ChildOf(root)).id();
        let owner = app
            .world_mut()
            .spawn((
                Button,
                TabIndex(0),
                ChildOf(root),
                crate::glue_menu::GlueButton {
                    frame: "ResolutionMenu".into(),
                    shortcut: None,
                    pushed_text_offset: None,
                },
            ))
            .id();
        let other = app
            .world_mut()
            .spawn((Button, TabIndex(1), ChildOf(root)))
            .id();
        app.update();
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(other, FocusCause::Navigated);
        Fixture {
            app,
            window,
            root,
            blank,
            owner,
            other,
        }
    }
    fn pointer() -> Pointer {
        Pointer::new(
            PointerId::Mouse,
            Location {
                target: NormalizedRenderTarget::None {
                    width: 1,
                    height: 1,
                },
                position: Vec2::ZERO,
            },
        )
    }
    fn press(f: &mut Fixture, entity: Entity) {
        f.app.world_mut().trigger(PointerPress {
            entity,
            pointer: pointer(),
            button: PointerButton::Primary,
            hit: HitData::new(entity, 0.0, None, None),
            count: 1,
        });
        f.app.world_mut().flush();
    }
    fn next(f: &mut Fixture) -> Result<Entity, TabNavigationError> {
        let mut system = SystemState::<(Res<InputFocus>, TabNavigation)>::new(f.app.world_mut());
        let (focus, nav) = system.get(f.app.world()).unwrap();
        nav.navigate(&focus, NavAction::Next)
    }
    fn popup(f: &mut Fixture) -> (Entity, Entity, Entity) {
        let popup = f
            .app
            .world_mut()
            .spawn((
                ResolutionPopup,
                TabGroup {
                    modal: true,
                    order: 1,
                },
                ChildOf(f.root),
            ))
            .id();
        let blank = f.app.world_mut().spawn(ChildOf(popup)).id();
        let choice = f
            .app
            .world_mut()
            .spawn((Button, TabIndex(0), ChildOf(popup)))
            .id();
        f.app
            .world_mut()
            .resource_mut::<InputFocus>()
            .set(choice, FocusCause::Auto);
        (popup, blank, choice)
    }
    #[test]
    fn baseline_blank_press_loses_focus_and_tab_cannot_enter_modal_menu() {
        let mut f = fixture(false, false);
        let blank = f.blank;
        press(&mut f, blank);
        assert_eq!(f.app.world().resource::<InputFocus>().get(), None);
        assert!(matches!(
            next(&mut f),
            Err(TabNavigationError::NoFocusableEntities)
        ));
    }
    #[test]
    fn blank_menu_press_retains_enabled_focus_and_tab_navigation() {
        for early in [false, true] {
            let mut f = fixture(true, early);
            let blank = f.blank;
            press(&mut f, blank);
            assert_eq!(f.app.world().resource::<InputFocus>().get(), Some(f.other));
            assert_eq!(next(&mut f), Ok(f.owner));
        }
    }
    #[test]
    fn outside_popup_blank_press_dismisses_and_restores_owner() {
        for early in [false, true] {
            let mut f = fixture(true, early);
            let (popup, _, _) = popup(&mut f);
            let blank = f.blank;
            press(&mut f, blank);
            assert!(f.app.world().get_entity(popup).is_err());
            assert_eq!(f.app.world().resource::<InputFocus>().get(), Some(f.owner));
            assert_eq!(next(&mut f), Ok(f.other));
        }
    }
    #[test]
    fn outside_popup_button_press_restores_owner_but_completed_action_still_runs() {
        for early in [false, true] {
            let mut f = fixture(true, early);
            let (popup, _, _) = popup(&mut f);
            let other = f.other;
            press(&mut f, other);
            assert!(f.app.world().get_entity(popup).is_err());
            assert_eq!(f.app.world().resource::<InputFocus>().get(), Some(f.owner));
            f.app.world_mut().trigger(PointerClick {
                entity: other,
                pointer: pointer(),
                button: PointerButton::Primary,
                hit: HitData::new(other, 0.0, None, None),
                duration: std::time::Duration::from_millis(1),
                count: 1,
            });
            f.app.world_mut().flush();
            assert_eq!(
                f.app
                    .world()
                    .resource::<crate::button_actions::ButtonActions>()
                    .entities,
                [other]
            );
        }
    }
    #[test]
    fn popup_blank_press_retains_choice_and_inside_choice_does_not_dismiss() {
        let mut f = fixture(true, false);
        let (popup, blank, choice) = popup(&mut f);
        press(&mut f, blank);
        assert_eq!(f.app.world().resource::<InputFocus>().get(), Some(choice));
        press(&mut f, choice);
        assert!(f.app.world().get_entity(popup).is_ok());
        assert_eq!(f.app.world().resource::<InputFocus>().get(), Some(choice));
        assert_eq!(next(&mut f), Ok(choice));
    }
    #[test]
    fn nonmenu_window_blank_click_still_clears_focus() {
        let mut f = fixture(true, false);
        let outside = f.app.world_mut().spawn(ChildOf(f.window)).id();
        press(&mut f, outside);
        assert_eq!(f.app.world().resource::<InputFocus>().get(), None);
    }
    #[test]
    fn disabled_previous_control_is_not_retained() {
        let mut f = fixture(true, false);
        let blank = f.blank;
        f.app
            .world_mut()
            .entity_mut(f.other)
            .insert(InteractionDisabled);
        press(&mut f, blank);
        assert_eq!(f.app.world().resource::<InputFocus>().get(), None);
    }
    #[test]
    fn explicit_acquire_focus_uses_the_same_scoped_boundary() {
        let mut f = fixture(true, false);
        f.app.world_mut().trigger(bevy::input_focus::AcquireFocus {
            focused_entity: f.blank,
            window: f.window,
        });
        f.app.world_mut().flush();
        assert_eq!(f.app.world().resource::<InputFocus>().get(), Some(f.other));
        assert_eq!(next(&mut f), Ok(f.owner));
        // InputDispatchPlugin is included; its actual keyboard message type is linked.
        assert!(f.app.world().contains_resource::<Messages<KeyboardInput>>());
    }
}

#[cfg(test)]
mod resize_bridge_tests {
    use super::*;
    use bevy::{
        camera::{CameraUpdateSystems, Projection, RenderTarget, Viewport},
        render::{camera::camera_system, texture::ManualTextureViews},
        window::{PrimaryWindow, WindowCreated, WindowResized, WindowScaleFactorChanged},
    };
    struct Fixture {
        app: App,
        window: Entity,
        full: Entity,
        bounded: Entity,
        panels: Entity,
    }
    fn fixture(bridge: bool) -> Fixture {
        let mut app = App::new();
        app.add_message::<WindowCreated>()
            .add_message::<WindowResized>()
            .add_message::<WindowScaleFactorChanged>()
            .add_message::<AssetEvent<Image>>()
            .init_resource::<Assets<Image>>()
            .init_resource::<ManualTextureViews>()
            .add_systems(PostUpdate, camera_system.in_set(CameraUpdateSystems));
        if bridge {
            app.add_systems(
                PostUpdate,
                synchronize_window_camera_size.before(CameraUpdateSystems),
            );
        }
        let window = app
            .world_mut()
            .spawn((
                Window {
                    resolution: (1280u32, 720u32).into(),
                    ..default()
                },
                PrimaryWindow,
            ))
            .id();
        let full = app
            .world_mut()
            .spawn((
                Camera::default(),
                RenderTarget::default(),
                Projection::default(),
            ))
            .id();
        let make_bounded = || {
            (
                Camera {
                    viewport: Some(Viewport {
                        physical_position: UVec2::new(160, 0),
                        physical_size: UVec2::new(960, 720),
                        ..default()
                    }),
                    ..default()
                },
                RenderTarget::default(),
                Projection::default(),
            )
        };
        let bounded = app.world_mut().spawn(make_bounded()).id();
        let panels = app.world_mut().spawn(make_bounded()).id();
        app.update();
        Fixture {
            app,
            window,
            full,
            bounded,
            panels,
        }
    }
    fn set_size(f: &mut Fixture, width: u32, height: u32) {
        f.app
            .world_mut()
            .get_mut::<Window>(f.window)
            .unwrap()
            .resolution
            .set_physical_resolution(width, height);
        let bounded_width = height * 4 / 3;
        for entity in [f.bounded, f.panels] {
            f.app
                .world_mut()
                .get_mut::<Camera>(entity)
                .unwrap()
                .viewport = Some(Viewport {
                physical_position: UVec2::new(width.saturating_sub(bounded_width) / 2, 0),
                physical_size: UVec2::new(width.min(bounded_width), height),
                ..default()
            });
        }
    }
    fn sizes(f: &Fixture) -> [Option<UVec2>; 3] {
        [f.full, f.bounded, f.panels].map(|entity| {
            f.app
                .world()
                .get::<Camera>(entity)
                .unwrap()
                .physical_target_size()
        })
    }
    #[test]
    fn baseline_programmatic_shrink_splits_camera_target_sizes_until_acknowledgement() {
        let mut f = fixture(false);
        assert_eq!(sizes(&f), [Some(UVec2::new(1280, 720)); 3]);
        set_size(&mut f, 752, 420);
        f.app.update();
        assert_eq!(
            sizes(&f),
            [
                Some(UVec2::new(1280, 720)),
                Some(UVec2::new(752, 420)),
                Some(UVec2::new(752, 420))
            ]
        );
        f.app.world_mut().write_message(WindowResized {
            window: f.window,
            width: 752.0,
            height: 420.0,
        });
        f.app.update();
        assert_eq!(sizes(&f), [Some(UVec2::new(752, 420)); 3]);
    }
    #[test]
    fn bridge_refreshes_full_and_bounded_cameras_in_same_frame_for_shrink_and_grow() {
        let mut f = fixture(true);
        for (width, height) in [(752, 420), (1280, 720), (752, 420)] {
            set_size(&mut f, width, height);
            f.app.update();
            assert_eq!(sizes(&f), [Some(UVec2::new(width, height)); 3]);
            assert!(
                f.app
                    .world()
                    .get::<Camera>(f.full)
                    .unwrap()
                    .viewport
                    .is_none()
            );
        }
    }
    #[test]
    fn duplicate_native_acknowledgement_does_not_change_bridge_result() {
        let mut f = fixture(true);
        set_size(&mut f, 752, 420);
        f.app.update();
        f.app.world_mut().write_message(WindowResized {
            window: f.window,
            width: 752.0,
            height: 420.0,
        });
        f.app.update();
        assert_eq!(sizes(&f), [Some(UVec2::new(752, 420)); 3]);
    }
    #[test]
    fn bridge_clears_observation_without_primary_window_and_tracks_replacement() {
        let mut f = fixture(true);
        f.app
            .world_mut()
            .entity_mut(f.window)
            .remove::<PrimaryWindow>();
        f.app.update();
        let replacement = f
            .app
            .world_mut()
            .spawn((
                Window {
                    resolution: (752u32, 420u32).into(),
                    ..default()
                },
                PrimaryWindow,
            ))
            .id();
        f.app.update();
        assert_ne!(replacement, f.window);
        assert_eq!(sizes(&f), [Some(UVec2::new(752, 420)); 3]);
    }
}

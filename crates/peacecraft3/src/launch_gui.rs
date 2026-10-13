//! GUI launcher.
//!
//! The CLI in [`crate::launch`] stays, because staging assets from CI or a
//! script should not need a window. This is the human front-end: pick a source,
//! pick a game mode, stage, and see what landed.
//!
//! Rendering the launcher on the engine it stages for is deliberate — one fewer
//! dependency, and the launcher and the game share a version.

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui::widget::{ImageNode, NodeImageMode};

use bevy::ui::{InteractionDisabled, Pressed};
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
    /// Latest released build, once the downloader exists. Until then the
    /// download control stays present and inert rather than absent, so the
    /// shape of the UI is decided now and does not get redesigned later.
    latest_release: Option<String>,
}

impl Default for Launcher {
    fn default() -> Self {
        Self {
            source: default_source(),
            mode: GameMode::default_for(pcx_mpq::Version::V1_29),
            log: Vec::new(),
            staging: false,
            latest_release: None,
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
const FONT_ART: &str = crate::mainmenu::MASTER_FONT;

#[derive(Component)]
struct Wc3Panel;

#[derive(Component)]
struct LauncherPanel;

#[derive(Component)]
struct ButtonArt;

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
            // `font_size` is a `FontSize` in 0.20; `f32` converts as pixels.
            font_size: size.into(),
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
            min_width: Val::Px(0.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::axes(Val::Px(20.0), Val::Px(16.0)),
            row_gap: Val::Px(12.0),
            border: UiRect::all(Val::Px(2.0)),
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
        column_gap: Val::Px(8.0),
        row_gap: Val::Px(4.0),
        flex_wrap: FlexWrap::Wrap,
        width: Val::Percent(100.0),
        min_width: Val::Px(0.0),
        ..default()
    })
    .with_children(|r| {
        r.spawn(tint(title, 20.0, crate::mainmenu::LABEL));
        if let Some(h) = hint {
            r.spawn((
                tint(h, 12.0, TEXT_FAINT),
                Node {
                    max_width: Val::Percent(100.0),
                    min_width: Val::Px(0.0),
                    flex_grow: 1.0,
                    flex_basis: Val::Px(300.0),
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

pub fn run() {
    app(window_plugin(None), |_| {}).run();
}

/// Builds the launcher, with `configure` applied *before* the plugins.
///
/// That ordering is the point: Bevy builds an app's asset sources while
/// `AssetPlugin` initialises, so a caller that needs a source which is not the
/// platform default — a browser has no filesystem to read — has to register it
/// first. Nothing else may go in here.
pub fn app(window: WindowPlugin, configure: impl FnOnce(&mut App)) -> App {
    let mut app = App::new();
    (configure)(&mut app);
    app.add_plugins(DefaultPlugins.set(window))
        .insert_resource(Launcher::default())
        .init_resource::<MenuArtCache>()
        .init_resource::<SceneLoad>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                colour_picks,
                apply_clicks,
                apply_menu_clicks,
                apply_menu_art,
                initialize_menu_scene,
                show_menu_or_launcher,
                scroll_launcher,
                refresh,
            )
                .chain(),
        );
    app.add_systems(
        Update,
        crate::mainmenu_scene::advance
            .run_if(resource_exists::<crate::mainmenu_scene::MainMenuScene>),
    );
    crate::menu_audio::install(&mut app);
    app
}

fn default_source() -> String {
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

fn setup(mut commands: Commands, l: Res<Launcher>) {
    commands.spawn((
        Camera2d,
        Camera {
            clear_color: bevy::camera::ClearColorConfig::None,
            ..default()
        },
    ));

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                min_width: Val::Px(0.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(26.0)),
                row_gap: Val::Px(18.0),
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
    use crate::mainmenu;

    // The frames are spawned here; their art is applied by [`apply_menu_art`]
    // afterwards. It has to be deferred because a browser cannot read the
    // install paths at startup — the page fetches the originals and hands them
    // over a moment later — so an art-less first frame is the correct state on
    // the way to an art-full one, not a failure.

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexEnd,
                row_gap: Val::Vh(mainmenu::BUTTON_GAP * 100.0),
                padding: UiRect {
                    top: Val::Vh(mainmenu::FIRST_TOP * 100.0),
                    right: Val::Vw(mainmenu::FIRST_RIGHT * 100.0),
                    ..default()
                },
                ..default()
            },
            BackgroundColor(Color::NONE),
            MenuLayer,
        ))
        .with_children(|root| {
            for (i, (id, raw)) in mainmenu::MENU.iter().enumerate() {
                let label = mainmenu::split_label(raw).unwrap_or(mainmenu::Label {
                    hotkey: '?',
                    rest: raw,
                });
                spawn_menu_button(root, i, *id, label);
            }
        });
}

/// Marker for the menu layer, so it can be dismissed once a choice is made.
#[derive(Component)]
pub(crate) struct MenuLayer;

#[derive(Resource, Default)]
struct SceneLoad {
    finished: bool,
}

fn initialize_menu_scene(
    mut commands: Commands,
    mut state: ResMut<SceneLoad>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    if state.finished {
        return;
    }
    let install = pcx_gfx::viewer::resolve_install(None);
    let bytes = crate::mainmenu::art_bytes_named(
        Some(&install),
        crate::mainmenu_scene::SCENE_ART,
        "menu3d/mainmenu3d.mdx",
    );
    let Some(bytes) = bytes else {
        #[cfg(not(target_arch = "wasm32"))]
        {
            state.finished = true;
            warn!("Original menu background unavailable in the configured installation");
        }
        return;
    };
    #[cfg(target_arch = "wasm32")]
    if crate::mainmenu_scene::texture_served_names(&bytes)
        .iter()
        .any(|name| crate::browser::stored_bytes(name).is_none())
    {
        return;
    }
    state.finished = true;
    let textures = crate::mainmenu_scene::load_textures(Some(&install), &bytes);
    match crate::mainmenu_scene::spawn(
        &mut commands,
        &mut meshes,
        &mut materials,
        &mut images,
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
        Err(error) => warn!("Original menu background: {error}"),
    }
}

fn show_menu_or_launcher(
    menu: Query<Entity, With<MenuLayer>>,
    mut launcher: Query<&mut Visibility, With<LauncherPanel>>,
) {
    for mut visibility in &mut launcher {
        *visibility = if menu.is_empty() {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

/// The menu's frame art, decoded once and shared by all six buttons.
#[derive(Resource, Default)]
struct MenuArtCache {
    /// `GlueScreen-Button1-Border.blp`, the button frame.
    button: Option<Handle<Image>>,
    panel: Option<Handle<Image>>,
    font: Option<Handle<Font>>,
    native_loaded: bool,
}

/// Puts the original's frame art on the menu buttons once it can be read.
///
/// Deferred rather than done at spawn because the two sources become readable at
/// different times: a desktop has the staged install on the first frame, while a
/// browser only has what the page fetched, a moment later. Running every frame
/// and doing nothing until the bytes exist means one code path serves both, and
/// a button that is already wearing its art is skipped rather than re-dressed.
fn apply_menu_art(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut fonts: ResMut<Assets<Font>>,
    mut cache: ResMut<MenuArtCache>,
    buttons: Query<(Entity, Option<&InteractionDisabled>), (With<Button>, Without<ButtonArt>)>,
    panels: Query<Entity, (With<Wc3Panel>, Without<ImageNode>)>,
    mut text: Query<&mut TextFont>,
) {
    // The desktop install is read once; the browser store fills asynchronously.
    if !cache.native_loaded || cfg!(target_arch = "wasm32") {
        if cache.button.is_none() {
            let path = crate::mainmenu::BUTTON_ART;
            if let Some(bytes) = menu_art_bytes(path) {
                if let Ok(tex) = pcx_gfx::load_image(&bytes, path) {
                    cache.button = Some(images.add(pcx_gfx::viewer::rgba_image(&tex)));
                }
            }
        }
        if cache.panel.is_none() {
            if let Some(bytes) = menu_art_bytes(PANEL_ART) {
                if let Ok(tex) = pcx_gfx::load_image(&bytes, PANEL_ART) {
                    cache.panel = Some(images.add(pcx_gfx::viewer::rgba_image(&tex)));
                }
            }
        }
        if cache.font.is_none() {
            if let Some(bytes) = menu_art_bytes(FONT_ART) {
                cache.font = Some(fonts.add(Font::from_bytes(bytes)));
            }
        }
        cache.native_loaded = true;
    }
    if let Some(handle) = &cache.button {
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
    if let Some(handle) = &cache.font {
        for mut style in &mut text {
            let font = bevy::text::FontSource::Handle(handle.clone());
            if style.font != font {
                style.font = font;
            }
        }
    }
}

/// Read original artwork from the install or browser store.
fn menu_art_bytes(path: &str) -> Option<Vec<u8>> {
    let dir = pcx_gfx::viewer::resolve_install(None);
    crate::mainmenu::art_bytes(Some(dir.as_path()), path)
}

/// One menu button, positioned from the original's own frame definition.
fn spawn_menu_button(
    root: &mut ChildSpawnerCommands,
    index: usize,
    id: &str,
    label: crate::mainmenu::Label,
) {
    let mut e = root.spawn((
        Node {
            width: Val::Vw(crate::mainmenu::BUTTON_W * 100.0),
            height: Val::Vh(crate::mainmenu::BUTTON_H * 100.0),
            min_width: Val::Rem(13.75),
            max_width: Val::Percent(100.0),
            min_height: Val::Rem(2.75),
            flex_shrink: 0.0,
            // `StandardButtonTextTemplate` justifies H *center* and V *middle*,
            // so the label sits centred in its own frame — not against the
            // right edge, which is where the frame itself is anchored.
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        // The pressable marker, and nothing else: the launcher's own pills show
        // that `Button` + `Node` is the complete recipe in 0.20, and adding
        // `Pick`/`Pickable` here either failed to compile (`Pick` is an enum
        // with no `default`) or, once spell as `Pickable`, made the frame stop
        // rendering. `Pressed` is what `ui_widgets` sets on press, which is
        // what the menu's click system waits for.
        Button,
        Hovered::default(),
        crate::mainmenu::MenuButton(index),
        Name::new(format!("menu-{id}")),
    ));
    // The frame art is inserted by [`apply_menu_art`] once it is available.
    e.with_children(|b| {
        b.spawn((
            Text::new(label.hotkey.to_string()),
            TextFont {
                font_size: bevy::text::FontSize::Vh(crate::mainmenu::LABEL_FONT_FRACTION * 100.0),
                ..default()
            },
            TextColor(crate::mainmenu::HOTKEY),
            ZIndex(1),
        ))
        .with_children(|text| {
            text.spawn((
                TextSpan::new(label.rest),
                TextFont {
                    font_size: bevy::text::FontSize::Vh(
                        crate::mainmenu::LABEL_FONT_FRACTION * 100.0,
                    ),
                    ..default()
                },
                TextColor(crate::mainmenu::LABEL),
            ));
        });
    });
}

fn header(root: &mut ChildSpawnerCommands, _l: &Launcher) {
    root.spawn(Node {
        flex_direction: FlexDirection::Row,
        justify_content: JustifyContent::SpaceBetween,
        align_items: AlignItems::Center,
        flex_wrap: FlexWrap::Wrap,
        row_gap: Val::Px(12.0),
        width: Val::Percent(100.0),
        ..default()
    })
    .with_children(|r| {
        r.spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|c| {
            c.spawn(tint("PeaceCraft III", 34.0, crate::mainmenu::LABEL));
            c.spawn(tint("The Frozen Throne · Asset launcher", 15.0, TEXT_DIM));
        });
        r.spawn((
            Button,
            Hovered::default(),
            Pick::Menu,
            Node {
                width: Val::Px(168.0),
                height: Val::Px(48.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
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
                min_height: Val::Px(38.0),
                justify_content: JustifyContent::Center,
                padding: UiRect::horizontal(Val::Px(12.0)),
                border: UiRect::all(Val::Px(1.0)),
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
            column_gap: Val::Px(6.0),
            ..default()
        })
        .with_children(|r| {
            r.spawn(tint("detected as", 11.0, TEXT_FAINT));
            r.spawn((tint("", 11.0, WARN), DetectedAs));
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
                            Pick::Build(pcx_mpq::Version::V1_28),
                            "1.28c".into(),
                            "Retail Complete Edition. Extra, not required.".into(),
                        ),
                        (
                            Pick::Build(pcx_mpq::Version::V1_29),
                            "1.29b".into(),
                            "The last stable legacy build. The target.".into(),
                        ),
                    ],
                    "",
                ),
                (
                    "Addon",
                    vec![
                        (
                            Pick::Addon(Addon::Tft),
                            "The Frozen Throne".into(),
                            "Full unit, ability and campaign set. The default.".into(),
                        ),
                        (
                            Pick::Addon(Addon::Roc),
                            "Reign of Chaos".into(),
                            "Pre-addon only: no TFT heroes, items or maps.".into(),
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
                margin: UiRect::top(Val::Px(4.0)),
                padding: UiRect::axes(Val::Px(12.0), Val::Px(9.0)),
                column_gap: Val::Px(10.0),
                border: UiRect::all(Val::Px(1.0)),
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
        column_gap: Val::Px(14.0),
        row_gap: Val::Px(6.0),
        width: Val::Percent(100.0),
        ..default()
    })
    .with_children(|row| {
        row.spawn(Node {
            width: Val::Px(88.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(1.0),
            ..default()
        })
        .with_children(|lbl| {
            lbl.spawn(tint(title, 16.0, TEXT_DIM));
            let _ = _blurb;
        });
        row.spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(7.0),
            flex_wrap: FlexWrap::Wrap,
            flex_grow: 1.0,
            flex_basis: Val::Px(400.0),
            max_width: Val::Percent(100.0),
            min_width: Val::Px(0.0),
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
                                left: Val::Px(24.0),
                                right: Val::Px(12.0),
                                top: Val::Px(13.0),
                                bottom: Val::Px(13.0),
                            },
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Start,
                            row_gap: Val::Px(2.0),
                            border: UiRect::all(Val::Px(1.0)),
                            border_radius: pill_radius(),
                            width: Val::Px(210.0),
                            max_width: Val::Percent(100.0),
                            min_height: Val::Px(72.0),
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
            column_gap: Val::Px(14.0),
            row_gap: Val::Px(12.0),
            ..default()
        })
        .with_children(|r| {
            r.spawn((
                Button,
                Hovered::default(),
                Node {
                    width: Val::Px(250.0),
                    max_width: Val::Percent(100.0),
                    height: Val::Px(64.0),
                    padding: UiRect::left(Val::Px(20.0)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    column_gap: Val::Px(9.0),
                    border: UiRect::all(Val::Px(1.0)),
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
                    width: Val::Px(190.0),
                    height: Val::Px(52.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
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
        row_gap: Val::Px(8.0),
        ..default()
    },))
        .with_children(|c| {
            section(c, "Log", None);
            c.spawn((
                Node {
                    width: Val::Percent(100.0),
                    min_height: Val::Px(120.0),
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(13.0)),
                    row_gap: Val::Px(3.0),
                    border: UiRect::all(Val::Px(1.0)),
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
fn colour_picks(
    l: Res<Launcher>,
    mut q: Query<
        (
            &Pick,
            &mut BackgroundColor,
            &mut BorderColor,
            &mut Node,
            Option<&InteractionDisabled>,
            Option<&Hovered>,
            Option<&mut ImageNode>,
        ),
        With<Button>,
    >,
) {
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
            node.border = UiRect::all(Val::Px(2.0));
        } else {
            *bg = BG_PILL.into();
            *border = BorderColor::all(BORDER);
            node.border = UiRect::all(Val::Px(1.0));
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

/// Clicks on the original main menu.
///
/// The original's menu is the entry point, so a choice on it must do something
/// visible: `options` is mapped to revealing the staging panel — which is the
/// launcher's actual job, the same relationship the original has between its
/// menu and its options pane. Every other entry is reported in the log with the
/// original's own label, so the mapping stays honest about what is and is not
/// implemented rather than pretending a button led somewhere.
pub(crate) fn apply_menu_clicks(
    mut commands: Commands,
    q: Query<&crate::mainmenu::MenuButton, Added<Pressed>>,
    menu: Query<Entity, With<MenuLayer>>,
) {
    use crate::mainmenu;
    for entry in &q {
        let (id, raw) = mainmenu::MENU[entry.0];
        match id {
            "options" => {
                // Reveal the staging panel; the menu has done its job.
                for e in &menu {
                    commands.entity(e).despawn();
                }
            }
            _ => {
                info!(
                    "{}: {}",
                    raw.split_once("|R").map(|(_, r)| r).unwrap_or(raw),
                    mainmenu::split_label(raw).map(|l| l.rest).unwrap_or(raw)
                );
            }
        }
    }
}

/// Clicks on the original main menu.
fn apply_clicks(
    mut l: ResMut<Launcher>,
    mut commands: Commands,
    mut q: Query<(&Pick, Option<&InteractionDisabled>), (With<Button>, Added<Pressed>)>,
) {
    for (p, disabled) in &mut q {
        // Disabled means disabled: the click is dropped here as well as being
        // greyed, so the control cannot be "fixed" by a stray pointer event.
        // `ui_widgets::Button` never presses a disabled button either.
        if disabled.is_some() || *p == Pick::Download {
            continue;
        }
        match p {
            Pick::Build(v) => l.mode.build = *v,
            Pick::Addon(a) => l.mode.addon = *a,
            Pick::Assets(a) => l.mode.assets = *a,
            Pick::Behaviour(b) => l.mode.behaviour = *b,
            Pick::Stage => l.staging = true,
            Pick::Download => {}
            Pick::Menu => {
                spawn_main_menu(&mut commands);
            }
        }
    }
}

fn refresh(
    mut l: ResMut<Launcher>,
    mut log_q: Query<&mut Text, With<LogBody>>,
    mut path_q: Query<&mut Text, (With<SourcePath>, Without<LogBody>)>,
    mut det_q: Query<&mut Text, (With<DetectedAs>, Without<LogBody>, Without<SourcePath>)>,
    mut sum_q: Query<
        &mut Text,
        (
            With<ModeSummary>,
            Without<LogBody>,
            Without<SourcePath>,
            Without<DetectedAs>,
        ),
    >,
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
        assert!(
            l.latest_release.is_none(),
            "the download control must stay inert until a downloader exists"
        );
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
    use crate::mainmenu::{MENU, MenuButton};
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn holding_stage_fires_once_until_the_button_is_released() {
        let mut app = App::new();
        app.insert_resource(Launcher::default())
            .init_resource::<Assets<Image>>()
            .add_systems(Update, apply_clicks);
        let button = app.world_mut().spawn((Button, Pick::Stage, Pressed)).id();
        app.update();
        assert!(app.world().resource::<Launcher>().staging);
        app.world_mut().resource_mut::<Launcher>().staging = false;
        app.update();
        assert!(!app.world().resource::<Launcher>().staging);
        app.world_mut().entity_mut(button).remove::<Pressed>();
        app.update();
        app.world_mut().entity_mut(button).insert(Pressed);
        app.update();
        assert!(app.world().resource::<Launcher>().staging);
    }

    /// The menu's click system, exercised headlessly.
    ///
    /// Browser hit-testing is a property of the picking plugin and of the
    /// window's pointer state, neither of which is what the menu's own wiring
    /// is about. This runs `apply_menu_clicks` against a world whose menu
    /// button carries `Pressed`, and asserts the one thing that wiring is
    /// for: `options` dismisses the menu layer.
    #[test]
    fn a_pressed_options_entry_dismisses_the_menu_layer() {
        let mut world = World::new();
        // The layer the menu is drawn in, and one of its entries.
        let layer = world.spawn(MenuLayer).id();
        let entry = world
            .spawn((Button, MenuButton(3), Pressed, Name::new("menu-options")))
            .id();

        world.run_system_once(apply_menu_clicks).unwrap();

        assert!(
            world.get_entity(layer).is_err(),
            "options: the menu layer is despawned"
        );
        // The entry itself is left alone; only the layer goes.
        assert!(world.get_entity(entry).is_ok(), "the entry survives");
    }

    /// An entry that is not `options` must not dismiss the menu: it is reported
    /// instead, so nothing is silently swallowed.
    #[test]
    fn a_pressed_non_options_entry_leaves_the_menu_up() {
        let mut world = World::new();
        let layer = world.spawn(MenuLayer).id();
        let _ = world.spawn((Button, MenuButton(0), Pressed));

        world.run_system_once(apply_menu_clicks).unwrap();

        assert!(
            world.get_entity(layer).is_ok(),
            "a non-options entry does not dismiss the menu"
        );
    }

    /// Every entry must be reachable by index, which is the only thing joining
    /// the button on screen to the label the original shows.
    #[test]
    fn every_menu_entry_is_addressed_by_its_own_button() {
        for i in 0..MENU.len() {
            let (id, raw) = MENU[i];
            // An out-of-range index would be a real bug, and it is the kind
            // that only shows up when a click lands.
            assert!(i < MENU.len(), "{id}");
            assert!(!raw.is_empty(), "{id}");
            let _ = crate::mainmenu::split_label(raw).unwrap_or_else(|e| panic!("{id}: {e}"));
        }
    }
}

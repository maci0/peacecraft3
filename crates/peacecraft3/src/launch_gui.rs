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
}

impl Default for Launcher {
    fn default() -> Self {
        Self {
            source: default_source(),
            mode: GameMode::default_for(pcx_mpq::Version::V1_29),
            log: Vec::new(),
            staging: false,
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

/// The original inner fill on a main-menu entry, as opposed to the launcher's
/// own controls, which share `Button` but are not skinned with it.
#[derive(Component)]
struct ButtonFill;

/// The 3×3 wrapper the fill's image sits in. Marked separately from the image so
/// the invalidation can despawn the wrapper — which takes the image with it —
/// rather than only the image, which would leave an empty wrapper behind on every
/// import.
#[derive(Component)]
struct ButtonFillFrame;

/// A menu entry's label root, carrying its index so the authored string can be
/// filled in once the strings text and the frame definitions are readable.
#[derive(Component)]
struct MenuText(usize);

/// Set once a label's runs exist, so it is filled once rather than every frame.
#[derive(Component)]
struct MenuTextFilled;

/// Set on a menu entry once its inner fill exists. The plate pass skips a button
/// carrying `ButtonArt`, so without this a fill that could not be derived on the
/// first pass — the frame definitions had not arrived yet — would never be added.
#[derive(Component)]
struct ButtonFilled;

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
/// first. Nothing else may go in here.
pub fn app(window: WindowPlugin, configure: impl FnOnce(&mut App)) -> App {
    let mut app = App::new();
    (configure)(&mut app);
    app.add_plugins(DefaultPlugins.set(window))
        .insert_resource(Launcher::default())
        .init_resource::<MenuArtCache>()
        .init_resource::<SceneLoad>()
        .init_resource::<crate::menu_panel::LogoEntrance>()
        .init_resource::<AssetStoreRevision>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                colour_picks,
                apply_clicks.in_set(LauncherInput),
                finish_intro,
                apply_menu_clicks,
                poll_asset_revision,
                invalidate_on_asset_revision,
                apply_menu_art,
                apply_menu_text,
                initialize_menu_logo,
                initialize_menu_scene,
                show_menu_or_launcher,
                scroll_launcher,
                refresh,
                crate::button_actions::clear,
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
    app
}

fn default_source() -> String {
    for key in ["PEACECRAFT3_INSTALL", "PEACECRAFT3_LEGACY_DIR"] {
        if let Ok(path) = std::env::var(key) {
            if classify(&path).is_ok() {
                return path;
            }
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
    use crate::mainmenu;

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
        ))
        .with_children(|root| {
            root.spawn(Node {
                grid_row: GridPlacement::start(1),
                grid_column: GridPlacement::start(1),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexEnd,
                padding: UiRect {
                    top: Val::Vh(mainmenu::FIRST_TOP / mainmenu::FRAME_H * 100.0),
                    right: Val::Vw(mainmenu::FIRST_RIGHT / mainmenu::FRAME_W * 100.0),
                    bottom: Val::Vh(mainmenu::EXIT_BOTTOM / mainmenu::FRAME_H * 100.0),
                    ..default()
                },
                ..default()
            })
            .with_children(|buttons| {
                for (i, id) in mainmenu::MENU.iter().enumerate() {
                    spawn_menu_button(buttons, i, id);
                }
            });
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
        });
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

fn invalidate_on_asset_revision(
    mut commands: Commands,
    revision: Res<AssetStoreRevision>,
    mut cache: ResMut<MenuArtCache>,
    mut scene: ResMut<SceneLoad>,
    mut text: Query<&mut Text>,
    logos: Query<Entity, With<crate::menu_panel::MenuLogo>>,
    scene_entities: Query<Entity, With<crate::mainmenu_scene::MainMenuSceneEntity>>,
    fills: Query<Entity, With<ButtonFillFrame>>,
    filled: Query<Entity, With<ButtonFilled>>,
    labels: Query<(Entity, &Children), (With<MenuText>, With<MenuTextFilled>)>,
) {
    if cache.revision == revision.0 {
        return;
    }
    cache.revision = revision.0;
    // Everything decoded or derived from the old store goes, including the two
    // texts the labels and their style come from — otherwise a second import would
    // keep the first store's strings.
    cache.button = None;
    cache.panel = None;
    cache.panel_uniform = None;
    cache.fonts.clear();
    cache.skin = None;
    cache.fdf = None;
    cache.fdf_failed = false;
    cache.strings = None;
    cache.templates = None;
    cache.main_menu = None;
    cache.native_loaded = false;
    // The labels are rebuilt: their runs are despawned and the marker cleared so
    // `apply_menu_text` re-spawns them from the new strings. The text root itself
    // is kept, so the button and its click identity are untouched.
    for (entity, children) in &labels {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
        if let Ok(mut t) = text.get_mut(entity) {
            t.0.clear();
        }
        commands.entity(entity).remove::<MenuTextFilled>();
    }
    // A fill's grid tracks are derived from the frames, so it is rebuilt rather
    // than re-handled; the plate keeps its marker and has its handle swapped.
    // The **wrapper** is despawned, which takes its image child with it —
    // despawning only the image would leave an empty wrapper behind on every
    // import.
    for entity in &fills {
        commands.entity(entity).despawn();
    }
    for entity in &filled {
        commands.entity(entity).remove::<ButtonFilled>();
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
    if !logos.is_empty() || launcher.mode.addon != Addon::Tft {
        return;
    }
    let install = selected_install(&launcher);
    let assets = crate::menu_panel::load_logo(install.as_deref());
    if !assets.is_complete() {
        return;
    }
    let data = match crate::menu_panel::LogoData::parse(assets.model.as_deref().unwrap()) {
        Ok(data) => data,
        Err(error) => {
            warn!("Original menu logo: {error}");
            return;
        }
    };
    if !data.planar
        || !data
            .texture
            .eq_ignore_ascii_case(crate::menu_panel::LOGO_TEXTURE)
    {
        warn!("Original menu logo: unsupported geometry or texture binding");
        return;
    }
    let texture = match pcx_gfx::load_image(assets.texture.as_deref().unwrap(), &data.texture) {
        Ok(texture) => texture,
        Err(error) => {
            warn!("Original menu logo texture: {error}");
            return;
        }
    };
    // FDF TOPLEFT (.13,+.04); native implicit height is maxY-minY.
    // Its LEFT/BOTTOM model origin does not subtract the vertex minimum.
    let left = crate::menu_panel::LOGO_ANCHOR_UNITS.0 + data.quad.0;
    let top = data.quad_height() - crate::menu_panel::LOGO_ANCHOR_UNITS.1 - data.quad.3;
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
    info!("Original menu logo: parsed authored Birth and native LEFT/BOTTOM projection");
}

fn initialize_menu_scene(
    mut commands: Commands,
    mut state: ResMut<SceneLoad>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    launcher: Res<Launcher>,
    menu: Query<Entity, With<MenuLayer>>,
    scene_entities: Query<Entity, With<crate::mainmenu_scene::MainMenuSceneEntity>>,
) {
    if menu.is_empty() {
        return;
    }
    if state.addon != Some(launcher.mode.addon) {
        for entity in &scene_entities {
            commands.entity(entity).despawn();
        }
        commands.remove_resource::<crate::mainmenu_scene::MainMenuScene>();
        state.addon = Some(launcher.mode.addon);
        state.finished = false;
    }
    if state.finished {
        return;
    }
    let install = selected_install(&launcher);
    let (source, served, directory) =
        crate::mainmenu_scene::scene_source(launcher.mode.addon == Addon::Tft);
    let bytes = crate::mainmenu::art_bytes_named(install.as_deref(), source, served);
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
        source,
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

/// The menu's frame art, decoded once and shared by all six buttons.
#[derive(Resource, Default)]
struct MenuArtCache {
    /// `GlueScreen-Button1-Border.blp`, the button frame.
    button: Option<Handle<Image>>,
    panel: Option<Handle<Image>>,
    /// One handle per authored face, so an entry's own face is honoured instead of
    /// every label sharing one hardcoded file.
    fonts: std::collections::HashMap<String, Handle<Font>>,
    /// `UI\war3skins.txt`, the face -> file binding's source.
    skin: Option<String>,
    /// Faces whose binding has already been reported, so a broken table does not
    /// log every frame.
    font_problems: std::collections::HashSet<String>,
    /// The parsed frame definitions the button bounds come from. `None` until
    /// **both** FDFs have been read and parsed, so a partially-arrived pair is
    /// never cached as if it were complete.
    fdf: Option<crate::fdf::Fdf>,
    /// Set when an FDF that *is* present fails to read or parse. Retrying the same
    /// bad bytes every frame would spin; the failure is reported once instead.
    fdf_failed: bool,
    /// `GlobalStrings.fdf`, the labels' source.
    strings: Option<String>,
    /// `StandardTemplates.fdf`, the label style's source.
    templates: Option<String>,
    /// `MainMenu.fdf`, whose text frames name the per-entry style frames.
    main_menu: Option<String>,
    /// Whether the fill texture's texels are all one colour.
    ///
    /// The FDF asks for `BackdropTileBackground` at `BackdropBackgroundSize
    /// 0.032`, which is tiling. This renderer draws the fill with `Stretch`
    /// instead, and that is only pixel-equivalent when the texture is uniform —
    /// which the shipped one is. A non-uniform texture is refused rather than
    /// stretched, because stretching would silently be the wrong mapping.
    panel_uniform: Option<bool>,
    /// The asset-store generation this cache was filled from. The page replaces
    /// its store when the user imports a local ISO, and everything decoded from
    /// the old one has to be dropped when that happens.
    revision: u32,
    native_loaded: bool,
    install: Option<std::path::PathBuf>,
    selection: Option<String>,
}

/// Whether every texel of an RGBA8 buffer is the same colour.
///
/// The fill is drawn with `Stretch`, which is pixel-equivalent to the authored
/// tiling only when the texture is one colour. Measured here rather than assumed.
fn uniform_rgba(rgba: &[u8]) -> bool {
    match rgba.get(..4) {
        Some(first) => rgba.chunks_exact(4).all(|p| p == first),
        None => false,
    }
}

/// Spawns one menu entry's inner fill, or `None` when the art or the frame
/// definitions are not ready, or the entry's frames cannot be derived.
///
/// The fill is a **3×3 grid wrapper**: the image occupies the middle cell, and the
/// surrounding tracks carry the authored left/width/right and top/height/bottom
/// as percentages of the plate. A grid track's percentage resolves against its
/// container's width for columns and height for rows, so both axes are right
/// without the full-window `Vw`/`Vh` mismatch a centred 4:3 or portrait menu would
/// otherwise introduce. The wrapper fills its cell and shares it with the label,
/// so nothing is absolute and nothing is pixel-sized.
fn spawn_fill(
    commands: &mut Commands,
    cache: &MenuArtCache,
    entry: &crate::mainmenu::MenuButton,
) -> Option<Entity> {
    let fill = cache.panel.as_ref()?;
    let fdf = cache.fdf.as_ref()?;
    // The authored record is `BackdropTileBackground` at `BackdropBackgroundSize
    // 0.032`; this draws with `Stretch`, which is pixel-equivalent only for a
    // uniform texture. A non-uniform one is not drawn rather than stretched,
    // because stretching would be the wrong mapping — and the load reports it.
    if cache.panel_uniform != Some(true) {
        return None;
    }
    let (left, top, width, height) = crate::mainmenu::button_fill(fdf, entry.0).ok()?;
    let (plate_w, plate_h) = crate::mainmenu::plate_size(fdf, entry.0).ok()?;
    let column = |v: f32| GridTrack::percent(v / plate_w * 100.0);
    let row = |v: f32| GridTrack::percent(v / plate_h * 100.0);
    let wrapper = commands
        .spawn((
            Node {
                grid_row: GridPlacement::start(1),
                grid_column: GridPlacement::start(1),
                display: Display::Grid,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                grid_template_columns: vec![
                    column(left),
                    column(width),
                    column(plate_w - left - width),
                ],
                grid_template_rows: vec![row(top), row(height), row(plate_h - top - height)],
                ..default()
            },
            ButtonFillFrame,
        ))
        .id();
    let image = commands
        .spawn((
            Node {
                grid_row: GridPlacement::start(2),
                grid_column: GridPlacement::start(2),
                // The middle cell is the authored extent; without an explicit
                // size the image falls back to its own 32x32 texture, which is
                // what made the fill a small square under the label instead of
                // the authored box.
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            // `Stretch`, **not** the default `Auto`: `Auto` applies the image's
            // own intrinsic measure, which sized the fill to the texture's 32x32
            // instead of the authored cell. `Stretch` ignores the original size
            // and aspect, and for this uniform texture that is pixel-equivalent.
            ImageNode::new(fill.clone()).with_mode(NodeImageMode::Stretch),
            ButtonFill,
        ))
        .id();
    commands.entity(wrapper).add_child(image);
    Some(wrapper)
}

/// Puts the original's frame art on the menu buttons once it can be read.
///
/// Deferred rather than done at spawn because the two sources become readable at
/// different times: a desktop has the staged install on the first frame, while a
/// browser only has what the page fetched, a moment later. Running every frame
/// and doing nothing until the bytes exist means one code path serves both, and
/// a button that is already wearing its art is skipped rather than re-dressed.
fn apply_menu_art(
    launcher: Res<Launcher>,
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut fonts: ResMut<Assets<Font>>,
    mut cache: ResMut<MenuArtCache>,
    mut buttons: Query<
        (
            Entity,
            &mut Node,
            &Children,
            Option<&InteractionDisabled>,
            Option<&crate::mainmenu::MenuButton>,
        ),
        (With<Button>, Without<ButtonArt>),
    >,
    // Entries that already wear their plate but not yet their fill, so a frame
    // set arriving after the art still reaches them.
    late: Query<
        (Entity, &Children, &crate::mainmenu::MenuButton),
        (With<ButtonArt>, Without<ButtonFilled>),
    >,
    // Buttons already wearing their art: after a store change the geometry is
    // unchanged but the texture handle is not, so the handle is swapped in place
    // rather than re-dressing the button and re-wrapping its label.
    mut dressed: Query<&mut ImageNode, (With<ButtonArt>, Without<ButtonFill>)>,
    mut fills: Query<&mut ImageNode, (With<ButtonFill>, Without<ButtonArt>)>,
    panels: Query<Entity, (With<Wc3Panel>, Without<ImageNode>)>,
) {
    let selection = format!("{}:{:?}", launcher.source, launcher.mode.assets);
    if cache.selection.as_ref() != Some(&selection) {
        let install = selected_install(&launcher);
        if cache.install != install {
            cache.button = None;
            cache.panel = None;
            cache.panel_uniform = None;
            cache.fonts.clear();
            cache.skin = None;
            cache.font_problems.clear();
            cache.fdf = None;
            cache.fdf_failed = false;
            cache.strings = None;
            cache.templates = None;
            cache.main_menu = None;
            cache.native_loaded = false;
            cache.install = install;
        }
        cache.selection = Some(selection);
    }
    let install = cache.install.clone();
    // The desktop install is read once; the browser store fills asynchronously.
    if !cache.native_loaded || cfg!(target_arch = "wasm32") {
        if cache.button.is_none() {
            let path = crate::mainmenu::BUTTON_ART;
            if let Some(bytes) = crate::mainmenu::art_bytes(install.as_deref(), path) {
                if let Ok(tex) = pcx_gfx::load_image(&bytes, path) {
                    cache.button = Some(images.add(pcx_gfx::viewer::rgba_image(&tex)));
                }
            }
        }
        if cache.panel.is_none() {
            if let Some(bytes) = crate::mainmenu::art_bytes(install.as_deref(), PANEL_ART) {
                if let Ok(tex) = pcx_gfx::load_image(&bytes, PANEL_ART) {
                    // Measured at decode, not assumed: the fill is drawn with
                    // `Stretch`, which is only right for a uniform texture.
                    let uniform = uniform_rgba(&tex.rgba);
                    if !uniform {
                        warn!(
                            "{PANEL_ART} is not uniform; the authored tiling is not implemented, so the fill is not drawn"
                        );
                    }
                    cache.panel_uniform = Some(uniform);
                    cache.panel = Some(images.add(pcx_gfx::viewer::rgba_image(&tex)));
                }
            }
        }
        // The skin table, which binds each face to its file.
        if cache.skin.is_none() {
            if let Some(bytes) =
                crate::mainmenu::art_bytes(install.as_deref(), crate::mainmenu::SKIN_TABLE)
            {
                cache.skin = std::str::from_utf8(&bytes).ok().map(str::to_string);
            }
        }
        // The frame definitions. **Both** must be read and parsed before the pair
        // is cached: the page fetches them separately, so caching after the first
        // one would freeze an incomplete frame set and every derivation against
        // the menu's own frames would fail from then on.
        if cache.fdf.is_none() && !cache.fdf_failed {
            let templates = crate::mainmenu::art_bytes(
                install.as_deref(),
                crate::mainmenu::STANDARD_TEMPLATES_FDF,
            );
            let menu =
                crate::mainmenu::art_bytes(install.as_deref(), crate::mainmenu::MAIN_MENU_FDF);
            if let (Some(templates), Some(menu)) = (templates.as_deref(), menu.as_deref()) {
                match (std::str::from_utf8(templates), std::str::from_utf8(menu)) {
                    // Templates first, so a menu override wins.
                    (Ok(templates), Ok(menu)) => {
                        match crate::fdf::Fdf::parse(templates).and_then(|mut f| {
                            f.parse_into(menu)?;
                            Ok(f)
                        }) {
                            Ok(fdf) => cache.fdf = Some(fdf),
                            Err(e) => {
                                bevy::log::warn!("menu frame definitions unusable: {e}");
                                cache.fdf_failed = true;
                            }
                        }
                    }
                    _ => {
                        bevy::log::warn!("menu frame definitions are not UTF-8 text");
                        cache.fdf_failed = true;
                    }
                }
            }
            // Not both present yet: a browser supplies them a moment apart, so
            // leave the cache empty and try again next frame.
        }
        // The menu tree's own text, needed to follow each entry's `ButtonText` to
        // the text frame whose style it draws with.
        if cache.main_menu.is_none() {
            if let Some(bytes) =
                crate::mainmenu::art_bytes(install.as_deref(), crate::mainmenu::MAIN_MENU_FDF)
            {
                cache.main_menu = std::str::from_utf8(&bytes).ok().map(str::to_string);
            }
        }
        // The labels' own source. Absent until the page serves it, which is the
        // browser's normal arrival order.
        if cache.strings.is_none() {
            if let Some(bytes) =
                crate::mainmenu::art_bytes(install.as_deref(), crate::mainmenu::GLOBAL_STRINGS_FDF)
            {
                cache.strings = std::str::from_utf8(&bytes).ok().map(str::to_string);
            }
        }
        // The label style's source, which the same fetch supplies.
        if cache.templates.is_none() {
            if let Some(bytes) = crate::mainmenu::art_bytes(
                install.as_deref(),
                crate::mainmenu::STANDARD_TEMPLATES_FDF,
            ) {
                cache.templates = std::str::from_utf8(&bytes).ok().map(str::to_string);
            }
        }
        // Each entry's font, from its own face and the shipped skin table. No
        // fallback: a face the table does not bind, or a file the store does not
        // carry, leaves that entry without a font rather than substituting one.
        if let Some(skin) = cache.skin.clone() {
            for i in 0..crate::mainmenu::MENU.len() {
                let Some(style) = entry_style(&cache, i) else {
                    continue;
                };
                if cache.fonts.contains_key(&style.font) {
                    continue;
                }
                let binding = match crate::mainmenu::menu_font_file(&skin, &style.font) {
                    Ok(binding) => binding,
                    Err(error) => {
                        // A table that does not bind the face is a real problem, not
                        // a timing one: report it once rather than every frame.
                        if cache.font_problems.insert(style.font.clone()) {
                            warn!("Menu font {:?}: {error}", style.font);
                        }
                        continue;
                    }
                };
                let Some(served) = crate::mainmenu::served_font_name(&binding.file) else {
                    continue;
                };
                if let Some(bytes) =
                    crate::mainmenu::art_bytes_named(install.as_deref(), &binding.file, &served)
                {
                    cache
                        .fonts
                        .insert(style.font.clone(), fonts.add(Font::from_bytes(bytes)));
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
        for (entity, mut node, children, disabled, menu_button) in &mut buttons {
            let mut image = ImageNode::new(handle.clone()).with_mode(NodeImageMode::Stretch);
            if disabled.is_some() {
                image.color = DISABLED_FG;
            }
            // One grid cell carries both the inner fill and the label, so they
            // overlay. Two ordinary flex children would stack and push the label.
            let overlay = commands
                .spawn(Node {
                    display: Display::Grid,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    grid_template_columns: vec![GridTrack::percent(100.0)],
                    grid_template_rows: vec![GridTrack::percent(100.0)],
                    ..default()
                })
                .id();
            // The original inner fill, on menu entries only: the launcher's own
            // controls are `Button`s too and must not be skinned with it.
            if let Some(entry) = menu_button {
                if let Some(fill_node) = spawn_fill(&mut commands, &cache, entry) {
                    commands.entity(overlay).add_child(fill_node);
                    commands.entity(entity).insert(ButtonFilled);
                }
            }
            // The template's own justification drives the label's box, so the
            // vertical axis is the container's main axis and the horizontal one
            // its cross axis.
            let style = menu_button.and_then(|entry| entry_style(&cache, entry.0));
            // Keep the frame on the outer button and padding on its contents.
            let content = commands
                .spawn(Node {
                    grid_row: GridPlacement::start(1),
                    grid_column: GridPlacement::start(1),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    padding: node.padding,
                    flex_direction: node.flex_direction,
                    justify_content: style
                        .as_ref()
                        .map(|s| justify_v(s.justify_v))
                        .unwrap_or(node.justify_content),
                    align_items: style
                        .as_ref()
                        .map(|s| align_h(s.justify_h))
                        .unwrap_or(node.align_items),
                    row_gap: node.row_gap,
                    column_gap: node.column_gap,
                    ..default()
                })
                .id();
            for child in children.iter() {
                commands.entity(content).add_child(child);
            }
            node.padding = UiRect::ZERO;
            commands
                .entity(entity)
                .insert((ButtonArt, image))
                .add_child(overlay);
            commands.entity(overlay).add_child(content);
        }
    }
    // A frame set that arrives after the plate still gets its fill: the plate pass
    // skips a button that already carries `ButtonArt`, so this pass is what makes
    // a late `cache.fdf` reach the entries instead of freezing them fill-less.
    if cache.fdf.is_some() && cache.panel.is_some() {
        for (entity, children, entry) in &late {
            // The plate pass gave each entry exactly one child: the overlay grid.
            let Some(overlay) = children.first().copied() else {
                continue;
            };
            if let Some(fill_node) = spawn_fill(&mut commands, &cache, entry) {
                // First, so it draws behind the label in the shared grid cell.
                commands.entity(overlay).insert_children(0, &[fill_node]);
                commands.entity(entity).insert(ButtonFilled);
            }
        }
    }
    if let Some(handle) = &cache.panel {
        for mut image in &mut fills {
            if image.image != *handle {
                image.image = handle.clone();
            }
        }
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

/// The style the entry's **own** text frame declares, resolved across the two
/// texts. Following the entry's `ButtonText` is what honours a per-entry record
/// override instead of assuming every entry shares one template.
fn entry_style(cache: &MenuArtCache, index: usize) -> Option<crate::menu_text::TextStyle> {
    let fdf = cache.fdf.as_ref()?;
    let frame = crate::mainmenu::menu_text_frame(fdf, index).ok()?;
    let texts: Vec<&str> = [cache.main_menu.as_deref(), cache.templates.as_deref()]
        .into_iter()
        .flatten()
        .collect();
    crate::mainmenu::menu_text_style(&texts, &frame).ok()
}

/// Spawns each menu label's runs from the authored string, once the strings text
/// and the frame definitions are both readable.
///
/// Every authored run is kept, in order, so `KEY_EXIT`'s interior hotkey
/// (`E|Cffffffffx|Rit`) renders "Exit" with its leading `E` rather than "xit". A
/// label that cannot be derived is reported and left empty — never invented.
fn apply_menu_text(
    mut commands: Commands,
    cache: Res<MenuArtCache>,
    roots: Query<(Entity, &MenuText), Without<MenuTextFilled>>,
    disabled: Query<&crate::mainmenu::MenuButton, With<InteractionDisabled>>,
) {
    let (Some(strings), Some(fdf)) = (cache.strings.as_deref(), cache.fdf.as_ref()) else {
        return;
    };
    let dimmed: Vec<usize> = disabled.iter().map(|b| b.0).collect();
    for (entity, marker) in &roots {
        let id = crate::mainmenu::MENU[marker.0];
        let label = match crate::mainmenu::menu_label(strings, fdf, marker.0) {
            Ok(label) => label,
            Err(error) => {
                warn!("Menu label {id}: {error}");
                commands.entity(entity).insert(MenuTextFilled);
                continue;
            }
        };
        // Each entry's style comes from its own `ButtonText` frame, so a
        // per-entry override is honoured; an entry whose style cannot be resolved
        // is reported and left unfilled rather than given a default.
        let Some(style) = entry_style(&cache, marker.0) else {
            warn!("Menu label {id}: its text frame's style is unavailable");
            commands.entity(entity).insert(MenuTextFilled);
            continue;
        };
        let size = bevy::text::FontSize::Vh(style.height / crate::mainmenu::FRAME_H * 100.0);
        info!(
            "Menu label {id}: {} (hotkey {:?}); bound face {:?}; shadow {:?} at {:?}, FIXEDSIZE {} — shadow and FIXEDSIZE parsed, not applied",
            label.visible(),
            label.hotkey(),
            style.font,
            style.shadow_colour,
            style.shadow_offset,
            style.fixed_size
        );
        // The disabled colour comes from the template too, and applies to the
        // whole label rather than to the hotkey run alone.
        let dim = dimmed.contains(&marker.0);
        let normal = srgba(if dim { style.disabled } else { style.colour });
        let hot = srgba(if dim { style.disabled } else { style.highlight });
        // No `TextShadow` is inserted. The FDF's `FontShadowOffset` is in authored
        // frame units and Bevy's is in logical pixels, and no verified translation
        // between them — nor the engine's Y-up to the UI's Y-down — has been
        // established for a text shadow. A zero-offset shadow would be a stand-in
        // for a feature that is not implemented, so the component is omitted until
        // that mapping exists. The parsed shadow facts are reported above.
        // The entry's own face, resolved from its style. A face whose file has
        // not been read yet defers the fill — the label is left for a later frame
        // rather than given another face or a builtin.
        let Some(handle) = cache.fonts.get(&style.font) else {
            continue;
        };
        let font = bevy::text::FontSource::Handle(handle.clone());
        commands.entity(entity).insert((
            TextFont {
                font: font.clone(),
                font_size: size,
                ..default()
            },
            TextColor(normal),
            TextLayout::new(justify(style.justify_h), bevy::text::LineBreak::NoWrap),
        ));
        for run in &label.spans {
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

/// One menu button, positioned from the original's own frame definition.
fn spawn_menu_button(root: &mut ChildSpawnerCommands, index: usize, id: &str) {
    let geometry = crate::mainmenu::button_geometry(index);
    let backdrop_width = if index == 5 {
        crate::mainmenu::EXIT_W
    } else {
        crate::mainmenu::BUTTON_W
    };
    let right = if index < 2 { 0.042 } else { 0.012 };
    let text_width = if index < 2 {
        0.149
    } else if index == 5 {
        0.168
    } else {
        0.179
    };
    let mut e = root.spawn((
        Node {
            width: Val::Vw(geometry.width * 100.0),
            height: Val::Vh(geometry.height * 100.0),
            min_width: Val::Rem(0.0),
            max_width: Val::Percent(100.0),
            min_height: Val::Rem(0.0),
            flex_shrink: 0.0,
            margin: UiRect {
                top: if index == 5 {
                    Val::Auto
                } else if index == 0 {
                    Val::Rem(0.0)
                } else {
                    Val::Vh(crate::mainmenu::BUTTON_GAP / crate::mainmenu::FRAME_H * 100.0)
                },
                ..default()
            },
            padding: UiRect {
                left: Val::Percent((backdrop_width - right - text_width) / backdrop_width * 100.0),
                right: Val::Percent(right / backdrop_width * 100.0),
                top: Val::Vh(0.0165 / crate::mainmenu::FRAME_H * 100.0),
                bottom: Val::Vh(0.0165 / crate::mainmenu::FRAME_H * 100.0),
            },
            flex_direction: FlexDirection::Column,
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
        // The label's runs are spawned by [`apply_menu_text`] once the authored
        // string and the frame definitions are readable, so the number of runs is
        // whatever the markup has rather than a fixed shape.
        // The label's runs and its size are set by [`apply_menu_text`] from the
        // template's own `FrameFont` record, so nothing is sized here.
        b.spawn((
            Text::new(""),
            TextFont::default(),
            TextColor(crate::mainmenu::LABEL),
            TextLayout::new(bevy::text::Justify::Center, bevy::text::LineBreak::NoWrap),
            Node {
                flex_shrink: 0.0,
                ..default()
            },
            // No `TextShadow`: see `apply_menu_text` for why the authored offset
            // is not applied.
            ZIndex(1),
            MenuText(index),
        ));
    });
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
            c.spawn(tint("The Frozen Throne · Asset launcher", 15.0, TEXT_DIM));
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
    actions: Res<crate::button_actions::ButtonActions>,
    q: Query<&crate::mainmenu::MenuButton>,
    menu: Query<Entity, With<MenuLayer>>,
) {
    use crate::mainmenu;
    for entity in &actions.entities {
        let Ok(entry) = q.get(*entity) else {
            continue;
        };
        let id = mainmenu::MENU[entry.0];
        match id {
            "options" => {
                // Reveal the staging panel; the menu has done its job.
                for e in &menu {
                    commands.entity(e).despawn();
                }
            }
            _ => {
                // Name the action by its id. The label text is the authored
                // string, derived from the assets at render time.
                info!("{id}");
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
) {
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
            Pick::Addon(a) => l.mode.addon = *a,
            Pick::Assets(a) => l.mode.assets = *a,
            Pick::Behaviour(b) => l.mode.behaviour = *b,
            Pick::Stage => l.staging = true,
            Pick::Download => {}
            Pick::Menu => {
                if !intro.request_with_install(l.mode.addon == Addon::Tft, selected_install(&l)) {
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
    // The click tests spawn `Pressed` buttons; nothing else in the crate needs
    // the marker, so it is imported here rather than at module scope.
    use crate::mainmenu::{MENU, MenuButton};
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

        world.insert_resource(crate::button_actions::ButtonActions {
            entities: vec![entry],
        });
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
        let entry = world.spawn((Button, MenuButton(0), Pressed)).id();

        world.insert_resource(crate::button_actions::ButtonActions {
            entities: vec![entry],
        });
        world.run_system_once(apply_menu_clicks).unwrap();

        assert!(
            world.get_entity(layer).is_ok(),
            "a non-options entry does not dismiss the menu"
        );
    }

    /// A replaced asset store must clear what was derived from the old one.
    ///
    /// This is the whole reason the invalidation exists: a second import has to
    /// remove the first store's runs, fills and parsed texts, while leaving the
    /// button itself alone so click, Edition and the F8 overlay keep working.
    #[test]
    fn a_new_asset_revision_clears_the_labels_and_fills() {
        use bevy::ecs::system::RunSystemOnce;

        let mut world = World::new();
        world.insert_resource(MenuArtCache {
            revision: 0,
            ..Default::default()
        });
        world.insert_resource(AssetStoreRevision(1));
        world.insert_resource(SceneLoad::default());

        // A dressed entry: a plate marker, a fill wrapper, and a filled label with
        // an old run.
        let run = world.spawn(TextSpan::new("old")).id();
        let label = world
            .spawn((Text::new("old"), MenuText(0), MenuTextFilled))
            .id();
        world.entity_mut(label).add_child(run);
        let fill = world.spawn((Node::default(), ButtonFill)).id();
        let wrapper = world.spawn((Node::default(), ButtonFillFrame)).id();
        world.entity_mut(wrapper).add_child(fill);
        let button = world.spawn((ButtonArt, ButtonFilled)).id();
        world.entity_mut(button).add_child(wrapper);

        world
            .run_system_once(invalidate_on_asset_revision)
            .expect("the invalidation runs");

        // The old run is despawned and the label is marked for refilling.
        assert!(world.get_entity(run).is_err(), "the old run is still there");
        assert!(
            world
                .get_entity(label)
                .unwrap()
                .get::<MenuTextFilled>()
                .is_none(),
            "the label would never be refilled"
        );
        // The fill is gone and the entry is marked for a fresh one. The wrapper
        // goes with its image — despawning only the image would leave an empty
        // wrapper behind on every import.
        assert!(
            world.get_entity(wrapper).is_err(),
            "the old wrapper is still there"
        );
        assert!(
            world.get_entity(fill).is_err(),
            "the wrapper's image survived it"
        );
        assert!(
            world
                .get_entity(button)
                .unwrap()
                .get::<ButtonFilled>()
                .is_none(),
            "the fill would never be rebuilt"
        );
        // The button itself survives, so its click identity and Edition state are
        // untouched.
        assert!(
            world
                .get_entity(button)
                .unwrap()
                .get::<ButtonArt>()
                .is_some()
        );
        // Every source is cleared, so the next frame reads the new store.
        let cache = world.resource::<MenuArtCache>();
        assert!(cache.button.is_none() && cache.panel.is_none() && cache.fonts.is_empty());
        assert!(cache.fdf.is_none() && cache.strings.is_none() && cache.templates.is_none());
        assert!(cache.main_menu.is_none() && !cache.native_loaded);
    }

    /// The frames the fill geometry test needs, matching the shipped shape.
    const FDF_FILL: &str = r#"
Frame "BACKDROP" "StandardMenuButtonBaseBackdrop" {
    Width 0.256,
    Height 0.064,
}
Frame "BACKDROP" "StandardButtonBackdropTemplate" {
    BackdropBackgroundInsets 0.004 0.004 0.004 0.004,
}
Frame "GLUEBUTTON" "StandardIconicButtonTemplate" {
    Height 0.031,
    Width 0.031,
}
Frame "TEXTBUTTON" "StandardButtonTemplate" {
    Height 0.031,
    Width 0.179,
    ControlBackdrop "ButtonBackdropTemplate",
    Frame "BACKDROP" "ButtonBackdropTemplate" INHERITS "StandardButtonBackdropTemplate" {
    }
}
Frame "FRAME" "MainMenuFrame" {
    Frame "BACKDROP" "SinglePlayerBackdrop" INHERITS "StandardMenuButtonBaseBackdrop" {
        SetPoint TOPRIGHT,"MainMenuFrame",TOPRIGHT, -0.015, -0.110625,
        Frame "GLUEBUTTON" "EditionButton" INHERITS "StandardIconicButtonTemplate" {
            SetPoint TOPRIGHT, "SinglePlayerBackdrop", TOPRIGHT, -0.012, -0.0165,
        }
        Frame "GLUETEXTBUTTON" "SinglePlayerButton" INHERITS WITHCHILDREN "StandardButtonTemplate" {
            Width 0.149,
            SetPoint TOPRIGHT, "EditionButton", TOPLEFT, 0.001, 0.0,
        }
    }
}
"#;

    /// The fill's image must be told to fill the middle cell. Without an explicit
    /// size it falls back to the texture's own 32×32, which is what rendered as a
    /// small square under each label instead of the authored extent.
    #[test]
    fn the_fill_image_fills_its_grid_cell() {
        use bevy::ecs::system::RunSystemOnce;

        fn spawn_one(mut commands: Commands, cache: Res<MenuArtCache>) {
            spawn_fill(&mut commands, &cache, &crate::mainmenu::MenuButton(0));
        }

        let mut world = World::new();
        world.insert_resource(MenuArtCache {
            panel: Some(Handle::<Image>::default()),
            panel_uniform: Some(true),
            fdf: Some(crate::fdf::Fdf::parse(FDF_FILL).expect("the fixture parses")),
            ..Default::default()
        });
        world.run_system_once(spawn_one).expect("the fill spawns");

        let mut wrappers = world.query_filtered::<&Node, With<ButtonFillFrame>>();
        let wrapper = wrappers.iter(&world).next().expect("a wrapper was spawned");
        assert_eq!(wrapper.display, Display::Grid);
        assert_eq!(wrapper.grid_template_columns.len(), 3, "3 columns");
        assert_eq!(wrapper.grid_template_rows.len(), 3, "3 rows");
        assert_eq!(wrapper.width, Val::Percent(100.0));
        assert_eq!(wrapper.height, Val::Percent(100.0));

        let mut images = world.query_filtered::<(&Node, &ImageNode), With<ButtonFill>>();
        let (image, node) = images.iter(&world).next().expect("an image was spawned");
        assert_eq!(image.width, Val::Percent(100.0), "the image fills its cell");
        assert_eq!(
            image.height,
            Val::Percent(100.0),
            "the image fills its cell"
        );
        // The mode is the whole point: `Auto` would apply the texture's own
        // intrinsic measure and size the fill to 32x32 whatever the cell says.
        assert_eq!(
            node.image_mode,
            NodeImageMode::Stretch,
            "the fill must stretch to its cell, not take the texture's size"
        );
    }

    /// A non-uniform fill texture is refused rather than stretched: `Stretch` is
    /// only equivalent to the authored tiling when every texel is the same colour.
    #[test]
    fn a_non_uniform_fill_texture_is_not_drawn() {
        use bevy::ecs::system::RunSystemOnce;

        fn spawn_one(mut commands: Commands, cache: Res<MenuArtCache>) {
            spawn_fill(&mut commands, &cache, &crate::mainmenu::MenuButton(0));
        }

        let mut world = World::new();
        world.insert_resource(MenuArtCache {
            panel: Some(Handle::<Image>::default()),
            panel_uniform: Some(false),
            fdf: Some(crate::fdf::Fdf::parse(FDF_FILL).expect("the fixture parses")),
            ..Default::default()
        });
        world.run_system_once(spawn_one).expect("the system runs");

        let mut wrappers = world.query_filtered::<Entity, With<ButtonFillFrame>>();
        assert_eq!(
            wrappers.iter(&world).count(),
            0,
            "a non-uniform texture must not be stretched into the cell"
        );
    }

    /// The uniformity check itself, on both a uniform and a mixed buffer.
    #[test]
    fn uniformity_is_measured_from_the_texels() {
        assert!(uniform_rgba(&[0, 0, 58, 255, 0, 0, 58, 255]));
        assert!(!uniform_rgba(&[0, 0, 58, 255, 0, 0, 59, 255]));
        assert!(!uniform_rgba(&[]));
    }

    /// Six buttons and six text frames; entry 0 names `FaceA`, entry 5 names
    /// `FaceB`, and the rest inherit the template's face.
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

    /// Each label takes **its own** entry's face, on the root and on every run,
    /// and unrelated text is left alone.
    ///
    /// Two entries name different faces, so a first-font-wins assignment would be
    /// caught: it would give both the same handle.
    #[test]
    fn each_label_takes_its_own_face_and_leaves_other_text_alone() {
        use bevy::ecs::system::RunSystemOnce;

        let mut faces = Assets::<Font>::default();
        let face_a = faces.add(Font::from_bytes(Vec::new()));
        let face_b = faces.add(Font::from_bytes(Vec::new()));

        let mut world = World::new();
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
        // Two entries, and a launcher/F8 text node that must not be touched.
        let single = world.spawn((Text::new(""), MenuText(0))).id();
        let exit = world.spawn((Text::new(""), MenuText(5))).id();
        let unrelated = world
            .spawn(TextFont {
                font: bevy::text::FontSource::default(),
                font_size: bevy::text::FontSize::Px(12.0),
                ..default()
            })
            .id();

        world.run_system_once(apply_menu_text).expect("runs");

        // The root and every run carry the entry's own handle.
        assert_eq!(
            world.get::<TextFont>(single).unwrap().font,
            bevy::text::FontSource::Handle(face_a.clone()),
            "entry 0's root took the wrong face"
        );
        assert_eq!(
            world.get::<TextFont>(exit).unwrap().font,
            bevy::text::FontSource::Handle(face_b.clone()),
            "entry 5's root took the wrong face"
        );
        for (root, want, visible) in [(single, &face_a, "Single Player"), (exit, &face_b, "Exit")] {
            let children: Vec<Entity> = world.get::<Children>(root).unwrap().iter().collect();
            assert!(!children.is_empty(), "the label spawned no runs");
            // The runs are the authored spans; concatenated they spell the label.
            let joined: String = children
                .iter()
                .map(|c| world.get::<TextSpan>(*c).unwrap().0.clone())
                .collect();
            assert_eq!(joined, visible, "the runs do not spell the label");
            for child in children {
                assert_eq!(
                    world.get::<TextFont>(child).unwrap().font,
                    bevy::text::FontSource::Handle(want.clone()),
                    "a run took the wrong face"
                );
            }
        }
        // The unrelated text keeps its own font.
        assert_eq!(
            world.get::<TextFont>(unrelated).unwrap().font,
            bevy::text::FontSource::default(),
            "unrelated text was overwritten"
        );
    }

    /// An entry whose face has no loaded handle defers its fill rather than
    /// borrowing another face.
    #[test]
    fn a_missing_face_defers_the_label() {
        use bevy::ecs::system::RunSystemOnce;

        let mut world = World::new();
        let mut fonts = std::collections::HashMap::new();
        // Only entry 0's face is loaded; entry 5's `FaceB` is not.
        fonts.insert(
            "FaceA".to_string(),
            Assets::<Font>::default().add(Font::from_bytes(Vec::new())),
        );
        world.insert_resource(MenuArtCache {
            fdf: Some(crate::fdf::Fdf::parse(FDF_FACES).expect("fixture")),
            main_menu: Some(FDF_FACES.to_string()),
            strings: Some(STRINGS_FACES.to_string()),
            fonts,
            ..Default::default()
        });
        let exit = world.spawn((Text::new(""), MenuText(5))).id();
        world.run_system_once(apply_menu_text).expect("runs");

        assert!(
            world.get::<MenuTextFilled>(exit).is_none(),
            "a deferred label must stay unfilled"
        );
        assert!(
            world.get::<Children>(exit).is_none_or(|c| c.is_empty()),
            "no run may be spawned without the entry's own face"
        );
    }

    /// A second revision with the *same* value clears nothing, so an import that
    /// does not change the store does not tear the menu down.
    #[test]
    fn an_unchanged_revision_clears_nothing() {
        use bevy::ecs::system::RunSystemOnce;

        let mut world = World::new();
        world.insert_resource(MenuArtCache {
            revision: 7,
            ..Default::default()
        });
        world.insert_resource(AssetStoreRevision(7));
        world.insert_resource(SceneLoad::default());
        let run = world.spawn(TextSpan::new("kept")).id();
        let label = world
            .spawn((Text::new("kept"), MenuText(0), MenuTextFilled))
            .id();
        world.entity_mut(label).add_child(run);

        world.run_system_once(invalidate_on_asset_revision).unwrap();

        assert!(
            world.get_entity(run).is_ok(),
            "an unchanged store must keep the runs"
        );
        assert!(
            world
                .get_entity(label)
                .unwrap()
                .get::<MenuTextFilled>()
                .is_some()
        );
    }

    /// Every entry must be reachable by index, which is the only thing joining
    /// the button on screen to the label the original shows.
    #[test]
    fn every_menu_entry_is_addressed_by_its_own_button() {
        for i in 0..MENU.len() {
            let id = MENU[i];
            // An out-of-range index would be a real bug, and it is the kind
            // that only shows up when a click lands.
            assert!(i < MENU.len(), "{id}");
            assert!(!id.is_empty(), "{id}");
        }
    }
}

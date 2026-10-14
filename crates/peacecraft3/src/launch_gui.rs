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

/// The node the FDF control layer is built into; it covers the authored frame.
#[derive(Component)]
struct ControlRoot;

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
                build_control_layer,
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
    app
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

#[derive(SystemParam)]
struct SceneAssets<'w> {
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<StandardMaterial>>,
    images: ResMut<'w, Assets<Image>>,
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
        cached_text(&mut cache.main_menu, install, crate::mainmenu::MAIN_MENU_FDF);
        cached_text(&mut cache.strings, install, crate::mainmenu::GLOBAL_STRINGS_FDF);
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
                f.parse_into(menu)?;
                Ok(f)
            }) {
                Ok(fdf) => cache.fdf = Some(fdf),
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
) {
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
    match crate::glue_menu::control_items(
        fdf,
        crate::glue_menu::CONTROL_LAYER,
        (crate::mainmenu::FRAME_W, crate::mainmenu::FRAME_H),
    ) {
        Ok((items, unsupported)) => {
            if !unsupported.is_empty() {
                warn!("Main menu frames not drawn: {unsupported:?}");
            }
            info!("Main menu: {} frames from the FDF control layer", items.len());
            for root in empty {
                crate::glue_menu::spawn(&mut commands, root, &items);
            }
        }
        Err(error) => {
            warn!("Main menu control layer: {error}");
            cache.fdf = None;
            cache.fdf_failed = true;
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

type LabelData = (Entity, &'static crate::glue_menu::GlueLabel, &'static ChildOf);

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
        templates,
        main_menu,
        install,
        ..
    } = &mut *cache;
    let (Some(strings), Some(fdf)) = (strings.as_deref(), fdf.as_ref()) else {
        return;
    };
    for (entity, label, parent) in &labels {
        let frame = label.frame.as_str();
        let parsed = fdf
            .text_value(frame)
            .ok_or_else(|| format!("{frame} has no Text"))
            .and_then(|key| {
                crate::menu_text::global_string(strings, &key)
                    .ok_or_else(|| format!("GlobalStrings has no {key}"))
            })
            .and_then(|raw| crate::menu_text::Label::parse(&raw).map_err(|e| e.to_string()))
            .and_then(|parsed| {
                text_style(main_menu.as_deref(), templates.as_deref(), frame)
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
        }
        commands.entity(entity).insert((
            TextFont {
                font: font.clone(),
                font_size: size,
                ..default()
            },
            TextColor(normal),
            TextLayout::new(justify(style.justify_h), bevy::text::LineBreak::NoWrap),
            TextShadow {
                offset: Vec2::ZERO,
                color: srgba(style.shadow_colour),
            },
            crate::glue_menu::FrameShadow(Vec2::from(style.shadow_offset)),
        ));
        for run in &parsed.spans {
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

/// Clicks on the original main menu, dispatched by the clicked FDF frame.
///
/// Options reveals the staging panel, the launcher's equivalent of the
/// original's options pane. The edition button switches between Reign of Chaos
/// and The Frozen Throne, which rebuilds the scene, panels and logo for that
/// edition. Quit exits; a browser has no process to end, so there it returns to
/// the launcher. Every other entry is reported as not implemented rather than
/// pretending a button led somewhere.
pub(crate) fn apply_menu_clicks(
    mut commands: Commands,
    mut launcher: ResMut<Launcher>,
    actions: Res<crate::button_actions::ButtonActions>,
    q: Query<&crate::glue_menu::GlueButton>,
    menu: Query<Entity, With<MenuLayer>>,
    #[cfg(not(target_arch = "wasm32"))] mut exit: MessageWriter<AppExit>,
) {
    for entity in &actions.entities {
        let Ok(button) = q.get(*entity) else {
            continue;
        };
        match button.frame.as_str() {
            "OptionsButton" => {
                for e in &menu {
                    commands.entity(e).despawn();
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
                launcher.mode.addon = match launcher.mode.addon {
                    Addon::Tft => Addon::Roc,
                    Addon::Roc => Addon::Tft,
                };
            }
            other => info!("{other}: not implemented"),
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
        let entry = world
            .spawn((
                Button,
                crate::glue_menu::GlueButton {
                    frame: frame.to_string(),
                    shortcut: None,
                    pushed_text_offset: Vec2::ZERO,
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

    /// Options reveals the staging panel by dismissing the menu layer.
    #[test]
    fn options_dismisses_the_menu_layer() {
        let mut world = World::new();
        let layer = world.spawn(MenuLayer).id();
        let entry = press(&mut world, "OptionsButton");
        world.run_system_once(apply_menu_clicks).unwrap();
        assert!(world.get_entity(layer).is_err(), "the menu layer is despawned");
        assert!(world.get_entity(entry).is_ok(), "only the layer goes");
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
        assert!(world.get_entity(root).is_ok(), "the root is kept for the rebuild");
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
        world.spawn((Node::default(), ControlRoot));
        world.run_system_once(build_control_layer).unwrap();
        world.flush();
        let mut buttons = world.query::<&crate::glue_menu::GlueButton>();
        let mut frames: Vec<String> = buttons.iter(&world).map(|b| b.frame.clone()).collect();
        frames.sort();
        assert_eq!(frames, ["ExitButton", "OptionsButton"]);
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

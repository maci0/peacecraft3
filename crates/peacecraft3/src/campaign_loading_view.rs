//! The campaign loading screen drawn from the selected `UI\FrameDef\Glue\Loading.fdf`:
//! the background sprite model at the sequence the map selects, the progress bar
//! sprite posed by the bar's progress, and the title, subtitle and text frames.
//! See `docs/re-notes/campaign-loading-screen.md`.
use crate::campaign_score_model::{
    self, AuthoredBackground, ScoreBackgroundState, ScoreSceneHandles,
};
use crate::fdf::{Fdf, Point, Rect};
use crate::mainmenu::{FRAME_H, FRAME_W};
use crate::menu_material::MenuMaterial;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use peacecraft3::campaign_loading::{CampaignLoading, LoadingRequest, Phase};
use peacecraft3::campaign_score_layout::includes;
use peacecraft3::global_strings::global_string;
use std::sync::Arc;

const FDF_PATH: &str = r"UI\FrameDef\Glue\Loading.fdf";
const STRINGS_PATH: &str = r"UI\FrameDef\GlobalStrings.fdf";
const UI_FILE_BYTES: u64 = 4 << 20;
const FONT_BYTES: u64 = 32 << 20;
/// Skin key that names the bar model (`UI\war3skins.txt`).
const BAR_MODEL_KEY: &str = "LoadingProgressBar";
/// Sequence of the bar model the progress poses: the first.
const BAR_SEQUENCE: usize = 0;

const ROOT: &str = "Loading";
const BACKGROUND: &str = "LoadingBackground";
const BAR: &str = "LoadingBar";
const BAR_TEXT: &str = "LoadingBarText";
const TEXT_STACK: [&str; 3] = ["LoadingTitleText", "LoadingSubtitleText", "LoadingText"];
const KEY_LOADING: &str = "LOADING_LOADING";
const KEY_PRESS: &str = "LOADING_PRESS_A_KEY";

/// Composition order of this screen's cameras: above the menu, the campaign view
/// and the cinematic filter. Not a source asset value.
const CAMERA_ORDER: isize = 1000;
/// Render layer of the screen's models, apart from the menu, score and HUD layers.
const MODEL_LAYER: usize = 10;
/// Sprite art is 2D; the recreation draws it without scene lighting.
const SPRITES_UNLIT: bool = true;

#[derive(Component)]
struct LoadingCamera;

#[derive(Component)]
struct LoadingRoot;

/// What the view built for one screen.
struct Built {
    root: Entity,
    models: Entity,
    cameras: [Entity; 3],
    handles: ScoreSceneHandles,
    fonts: Vec<Handle<Font>>,
    /// The `LoadingBarText` container, whose label changes with the phase.
    bar_text: Entity,
    bar_style: crate::menu_text::TextStyle,
    bar_font: Handle<Font>,
    strings: String,
    shown: Phase,
    label: Entity,
}

#[derive(Resource, Default)]
struct LoadingView {
    generation: u64,
    built: Option<Built>,
}

pub fn install(app: &mut App) {
    app.init_resource::<LoadingView>().add_systems(
        Update,
        (sync_view, fit_cameras, drive_bar)
            .chain()
            .after(peacecraft3::campaign_loading::CampaignLoadingSet),
    );
}

fn read_text(request: &LoadingRequest, path: &str) -> Result<String, String> {
    let asset = request
        .resolver
        .resolve_bounded(path, UI_FILE_BYTES)
        .map_err(|e| format!("{path}: {e}"))?;
    String::from_utf8(asset.bytes).map_err(|_| format!("{path}: not UTF-8 text"))
}

fn load_fdf(request: &LoadingRequest) -> Result<(Fdf, String), String> {
    let screen = read_text(request, FDF_PATH)?;
    let mut fdf = Fdf::default();
    for path in includes(&screen) {
        fdf.parse_into(&read_text(request, path)?)
            .map_err(|e| format!("{path}: {e}"))?;
    }
    fdf.parse_into(&screen)
        .map_err(|e| format!("{FDF_PATH}: {e}"))?;
    Ok((fdf, read_text(request, STRINGS_PATH)?))
}

/// Where the title, subtitle and text frames sit, in frame units (Y up).
#[derive(Debug, Clone, PartialEq)]
struct TextStack {
    /// Distance from the screen's right edge to the stack's right edge.
    right: f32,
    /// Distance from the screen's top edge to the stack's top edge.
    top: f32,
    width: f32,
    /// Space above each frame, below the one before it.
    gaps: Vec<f32>,
}

/// The first frame hangs from the screen's top right; each next frame hangs from the
/// bottom of the one before at the same width, so they share one column whose height
/// the text itself decides. Any other anchoring refuses.
fn text_stack(fdf: &Fdf, frames: &[&str]) -> Result<TextStack, String> {
    let anchor = |frame: &str| {
        fdf.set_point(frame)
            .ok_or_else(|| format!("{frame}: no SetPoint"))
    };
    let width = |frame: &str| fdf.width(frame).map_err(|e| format!("{frame}: {e}"));
    let [first, rest @ ..] = frames else {
        return Err("no text frames".into());
    };
    let a = anchor(first)?;
    if (a.point, a.relative.as_str(), a.relative_point) != (Point::TopRight, ROOT, Point::TopRight)
    {
        return Err(format!("{first}: expected a TOPRIGHT anchor on {ROOT}"));
    }
    let stack_width = width(first)?;
    let mut gaps = vec![0.0];
    let mut previous = *first;
    for frame in rest {
        let a = anchor(frame)?;
        if (a.point, a.relative.as_str(), a.relative_point) != (Point::Top, previous, Point::Bottom)
            || a.x != 0.0
            || width(frame)? != stack_width
        {
            return Err(format!(
                "{frame}: expected a TOP anchor on the bottom of {previous} at the same width"
            ));
        }
        gaps.push(-a.y);
        previous = frame;
    }
    Ok(TextStack {
        right: -a.x,
        top: -a.y,
        width: stack_width,
        gaps,
    })
}

/// The bar sprite's origin: its bottom-left anchored to the screen's bottom left.
fn bar_origin(fdf: &Fdf, root: Rect) -> Result<(f32, f32), String> {
    let a = fdf
        .set_point(BAR)
        .ok_or_else(|| format!("{BAR}: no SetPoint"))?;
    if (a.point, a.relative.as_str(), a.relative_point)
        != (Point::BottomLeft, ROOT, Point::BottomLeft)
    {
        return Err(format!("{BAR}: expected a BOTTOMLEFT anchor on {ROOT}"));
    }
    let (x, y) = root.point(Point::BottomLeft);
    Ok((x + a.x, y + a.y))
}

/// The bar text: bottom centre of the screen, moved by the anchor offset.
fn bar_text_offset(fdf: &Fdf) -> Result<(f32, f32), String> {
    let a = fdf
        .set_point(BAR_TEXT)
        .ok_or_else(|| format!("{BAR_TEXT}: no SetPoint"))?;
    if (a.point, a.relative.as_str(), a.relative_point) != (Point::Bottom, ROOT, Point::Bottom) {
        return Err(format!("{BAR_TEXT}: expected a BOTTOM anchor on {ROOT}"));
    }
    Ok((a.x, a.y))
}

/// Model frame of the bar at `fraction` of its sequence.
fn bar_frame(interval: [u32; 2], fraction: f32) -> u32 {
    let length = interval[1].saturating_sub(interval[0]);
    interval[0] + (f64::from(length) * f64::from(fraction.clamp(0.0, 1.0))).round() as u32
}

fn pct(value: f32, whole: f32) -> Val {
    percent(value / whole * 100.0)
}

fn teardown(
    commands: &mut Commands,
    built: Built,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<MenuMaterial>,
    images: &mut Assets<Image>,
    fonts: &mut Assets<Font>,
) {
    campaign_score_model::release_handles(&built.handles, meshes, materials, images);
    for font in &built.fonts {
        fonts.remove(font.id());
    }
    for entity in [built.root, built.models].into_iter().chain(built.cameras) {
        commands.entity(entity).despawn();
    }
}

struct Stores<'a> {
    meshes: &'a mut Assets<Mesh>,
    materials: &'a mut Assets<MenuMaterial>,
    images: &'a mut Assets<Image>,
    fonts: &'a mut Assets<Font>,
}

fn font_for(
    request: &LoadingRequest,
    skin: &str,
    style: &crate::menu_text::TextStyle,
    stores: &mut Stores,
    loaded: &mut Vec<(String, Handle<Font>)>,
) -> Result<Handle<Font>, String> {
    let binding =
        crate::mainmenu::menu_font_file(skin, &style.font).map_err(|e| format!("{e:?}"))?;
    if let Some((_, handle)) = loaded.iter().find(|(file, _)| *file == binding.file) {
        return Ok(handle.clone());
    }
    let bytes = request
        .resolver
        .resolve_bounded(&binding.file, FONT_BYTES)
        .map_err(|e| format!("{}: {e}", binding.file))?
        .bytes;
    if !crate::hud_render::font_registers(&bytes) {
        return Err(format!("{}: font has no readable face", binding.file));
    }
    let handle = stores.fonts.add(Font::from_bytes(bytes));
    loaded.push((binding.file, handle.clone()));
    Ok(handle)
}

fn build(
    commands: &mut Commands,
    request: &Arc<LoadingRequest>,
    window: UVec2,
    stores: &mut Stores,
) -> Result<Built, String> {
    let (fdf, strings) = load_fdf(request)?;
    let viewport = (FRAME_W, FRAME_H);
    let root_rect = fdf
        .rect(ROOT, viewport)
        .map_err(|e| format!("{ROOT}: {e}"))?;
    let skin = read_text(request, crate::mainmenu::SKIN_TABLE)?;
    let skin_table = crate::menu_skin::SkinTable::parse(&skin).map_err(|e| e.to_string())?;
    let bar_model = skin_table
        .unique_file(BAR_MODEL_KEY)
        .map_err(|e| format!("{BAR_MODEL_KEY}: {e}"))?
        .file;

    // Models.
    let background_rect = fdf
        .rect(BACKGROUND, viewport)
        .map_err(|e| format!("{BACKGROUND}: {e}"))?;
    let (bar_x, bar_y) = bar_origin(&fdf, root_rect)?;
    let sequence = usize::try_from(request.screen.sequence)
        .map_err(|_| format!("sequence {} is out of range", request.screen.sequence))?;
    let mut background = campaign_score_model::prepare_sequence(
        &request.resolver,
        &AuthoredBackground {
            frame: BACKGROUND.into(),
            model: request.screen.model.clone(),
            rect: background_rect,
            unlit: SPRITES_UNLIT,
        },
        Some(sequence),
    )?;
    background.driven = false;
    let mut bar = campaign_score_model::prepare_sequence(
        &request.resolver,
        &AuthoredBackground {
            frame: BAR.into(),
            model: bar_model,
            rect: Rect {
                left: bar_x,
                bottom: bar_y,
                width: 0.0,
                height: 0.0,
            },
            unlit: SPRITES_UNLIT,
        },
        Some(BAR_SEQUENCE),
    )?;
    bar.driven = true;
    let plans = vec![background, bar];
    let (low, high) = plans
        .iter()
        .fold((f32::MAX, f32::MIN), |(low, high), plan| {
            (
                low.min(plan.model.minimum_extent[2]),
                high.max(plan.model.maximum_extent[2]),
            )
        });
    let frame_box = crate::launch_gui::frame_viewport(window);
    let models = commands
        .spawn((
            Transform::default(),
            Visibility::default(),
            Name::new("Loading models"),
        ))
        .id();
    let handles = campaign_score_model::spawn_backgrounds_on(
        commands,
        crate::mainmenu_scene::GeosetAssets {
            meshes: &mut *stores.meshes,
            materials: &mut *stores.materials,
            images: &mut *stores.images,
        },
        models,
        plans,
        MODEL_LAYER,
    )?;

    // Cameras: black bars, the 4:3 model view, the 4:3 interface.
    let bars = commands
        .spawn((
            Camera2d,
            Camera {
                order: CAMERA_ORDER,
                clear_color: ClearColorConfig::Custom(Color::BLACK),
                ..default()
            },
            RenderLayers::none(),
            LoadingCamera,
        ))
        .id();
    let model_camera = commands
        .spawn((
            Camera3d::default(),
            Projection::Orthographic(OrthographicProjection {
                scaling_mode: bevy::camera::ScalingMode::Fixed {
                    width: FRAME_W,
                    height: FRAME_H,
                },
                near: -high,
                far: -low,
                ..OrthographicProjection::default_3d()
            }),
            Transform::from_xyz(FRAME_W / 2.0, FRAME_H / 2.0, 0.0),
            Camera {
                order: CAMERA_ORDER + 1,
                viewport: Some(frame_box.clone()),
                clear_color: ClearColorConfig::None,
                ..default()
            },
            bevy::core_pipeline::tonemapping::Tonemapping::None,
            bevy::core_pipeline::tonemapping::DebandDither::Disabled,
            RenderLayers::layer(MODEL_LAYER),
            LoadingCamera,
        ))
        .id();
    let ui_camera = commands
        .spawn((
            Camera2d,
            Camera {
                order: CAMERA_ORDER + 2,
                viewport: Some(frame_box),
                clear_color: ClearColorConfig::None,
                ..default()
            },
            RenderLayers::none(),
            LoadingCamera,
        ))
        .id();

    // Text.
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                ..default()
            },
            UiTargetCamera(ui_camera),
            GlobalZIndex(i32::MAX),
            Pickable {
                should_block_lower: true,
                is_hoverable: false,
            },
            LoadingRoot,
            Name::new("Campaign loading screen"),
        ))
        .id();
    let mut loaded = Vec::new();
    let mut label = |commands: &mut Commands,
                     stores: &mut Stores,
                     parent: Entity,
                     frame: &str,
                     value: &str|
     -> Result<(Entity, crate::menu_text::TextStyle, Handle<Font>), String> {
        let style = crate::menu_text::render_style(&fdf, frame)?;
        let font = font_for(request, &skin, &style, stores, &mut loaded)?;
        let line = crate::campaign_script_dialogs::label(commands, parent, value, &style, &font)?;
        Ok((line, style, font))
    };
    let stack = text_stack(&fdf, &TEXT_STACK)?;
    let column = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: pct(stack.right, FRAME_W),
                top: pct(stack.top, FRAME_H),
                width: pct(stack.width, FRAME_W),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    let text = &request.text;
    for ((frame, value), gap) in TEXT_STACK
        .iter()
        .zip([&text.title, &text.subtitle, &text.text])
        .zip(&stack.gaps)
    {
        let holder = commands
            .spawn((
                Node {
                    width: percent(100),
                    margin: UiRect::top(pct(*gap, FRAME_H)),
                    ..default()
                },
                ChildOf(column),
            ))
            .id();
        if value.is_empty() {
            continue;
        }
        if let Err(error) = label(commands, stores, holder, frame, value) {
            warn!("loading screen {frame} refused: {error}");
        }
    }
    let (dx, dy) = bar_text_offset(&fdf)?;
    let bar_text = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: pct(dx, FRAME_W),
                bottom: pct(dy, FRAME_H),
                width: percent(100),
                justify_content: JustifyContent::Center,
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    let key = KEY_LOADING;
    let value =
        global_string(&strings, key).ok_or_else(|| format!("{STRINGS_PATH}: missing {key}"))?;
    global_string(&strings, KEY_PRESS)
        .ok_or_else(|| format!("{STRINGS_PATH}: missing {KEY_PRESS}"))?;
    let (line, bar_style, bar_font) = label(commands, stores, bar_text, BAR_TEXT, &value)?;
    let fonts = loaded.into_iter().map(|(_, handle)| handle).collect();
    Ok(Built {
        root,
        models,
        cameras: [bars, model_camera, ui_camera],
        handles,
        fonts,
        bar_text,
        bar_style,
        bar_font,
        strings,
        shown: Phase::Loading,
        label: line,
    })
}

#[derive(bevy::ecs::system::SystemParam)]
struct ViewStores<'w> {
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<MenuMaterial>>,
    images: ResMut<'w, Assets<Image>>,
    fonts: ResMut<'w, Assets<Font>>,
}

fn sync_view(
    mut commands: Commands,
    mut loading: ResMut<CampaignLoading>,
    mut view: ResMut<LoadingView>,
    mut stores: ViewStores,
    windows: Query<&Window>,
) {
    let mut s = Stores {
        meshes: &mut stores.meshes,
        materials: &mut stores.materials,
        images: &mut stores.images,
        fonts: &mut stores.fonts,
    };
    let wanted = loading.shown().then(|| loading.generation());
    if (view.generation != loading.generation() || wanted.is_none())
        && let Some(built) = view.built.take()
    {
        teardown(
            &mut commands,
            built,
            s.meshes,
            s.materials,
            s.images,
            s.fonts,
        );
    }
    let (Some(generation), Some(request)) = (wanted, loading.request().cloned()) else {
        return;
    };
    if view.built.is_none() && view.generation != generation {
        view.generation = generation;
        let window = windows
            .single()
            .map(|w| UVec2::new(w.physical_width(), w.physical_height()))
            .unwrap_or(UVec2::new(1, 1));
        match build(&mut commands, &request, window, &mut s) {
            Ok(built) => view.built = Some(built),
            Err(error) => {
                warn!("campaign loading screen refused: {error}");
                loading.cancel();
            }
        }
        return;
    }
    // The bar text follows the phase.
    if let Some(built) = view.built.as_mut()
        && built.shown != loading.phase()
    {
        let key = match loading.phase() {
            Phase::AwaitKey => KEY_PRESS,
            _ => KEY_LOADING,
        };
        if let Some(value) = global_string(&built.strings, key) {
            commands.entity(built.label).despawn();
            match crate::campaign_script_dialogs::label(
                &mut commands,
                built.bar_text,
                &value,
                &built.bar_style,
                &built.bar_font,
            ) {
                Ok(line) => built.label = line,
                Err(error) => warn!("loading screen bar text refused: {error}"),
            }
        }
        built.shown = loading.phase();
    }
}

fn fit_cameras(windows: Query<&Window>, mut cameras: Query<&mut Camera, With<LoadingCamera>>) {
    let Ok(window) = windows.single() else {
        return;
    };
    let full = UVec2::new(window.physical_width(), window.physical_height());
    let desired = crate::launch_gui::frame_viewport(full);
    for mut camera in &mut cameras {
        // The black bar camera is the only one without a viewport and is left alone.
        let Some(current) = camera.viewport.as_ref() else {
            continue;
        };
        if (current.physical_position, current.physical_size)
            != (desired.physical_position, desired.physical_size)
        {
            camera.viewport = Some(desired.clone());
        }
    }
}

fn drive_bar(
    loading: Res<CampaignLoading>,
    mut states: Query<&mut ScoreBackgroundState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<MenuMaterial>>,
    mut visibility: Query<&mut Visibility>,
) {
    if !loading.shown() {
        return;
    }
    let fraction = loading.fraction();
    for mut state in &mut states {
        if !state.driven {
            continue;
        }
        let frame = bar_frame(state.interval, fraction);
        let elapsed = u64::from(frame - state.interval[0]);
        campaign_score_model::pose(
            &mut state,
            frame,
            elapsed,
            &mut meshes,
            &mut materials,
            &mut visibility,
        );
    }
}

/// Builds the screen in a bare world, for tests of original assets.
#[cfg(test)]
pub(crate) mod harness {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    /// What a built screen shows.
    pub(crate) struct Shown {
        /// Every text run, in spawn order.
        pub texts: Vec<String>,
        /// Poses of the bar at the start, middle and end of its progress.
        pub bar_frames: Vec<u32>,
    }

    /// Render the screen for `request` at `stages` reached stages to a 1280x960 offscreen
    /// target on a Vulkan adapter, and write it as a binary PPM to `out`. No window is
    /// opened. The target is not colour-qualified against the menu's working space.
    pub(crate) fn capture(
        request: LoadingRequest,
        stages: usize,
        out: &std::path::Path,
    ) -> Result<(), String> {
        use bevy::{
            asset::AssetPlugin,
            camera::RenderTarget,
            render::{
                RenderPlugin,
                gpu_readback::{Readback, ReadbackComplete},
                render_resource::{PollType, TextureFormat, TextureUsages},
                renderer::RenderDevice,
                settings::{Backends, RenderCreation, WgpuSettings},
            },
            window::ExitCondition,
            winit::WinitPlugin,
        };
        const WIDTH: u32 = 1280;
        const HEIGHT: u32 = 960;
        const FRAMES: usize = 160;
        const READ_AT: usize = 110;
        #[derive(Resource, Default)]
        struct Shot(Vec<u8>);
        let mut app = App::new();
        app.add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: None,
                    exit_condition: ExitCondition::DontExit,
                    ..default()
                })
                .set(AssetPlugin {
                    file_path: concat!(env!("CARGO_MANIFEST_DIR"), "/assets").into(),
                    ..default()
                })
                .set(RenderPlugin {
                    synchronous_pipeline_compilation: true,
                    render_creation: RenderCreation::Automatic(Box::new(WgpuSettings {
                        backends: Some(Backends::VULKAN),
                        ..default()
                    })),
                    ..default()
                })
                .disable::<WinitPlugin>()
                .disable::<bevy::log::LogPlugin>()
                .disable::<bevy::audio::AudioPlugin>(),
        )
        .add_plugins(crate::menu_composition::MenuCompositionPlugin)
        .add_plugins(crate::mainmenu_scene::ParticleMaterialPlugin)
        .add_plugins(MaterialPlugin::<MenuMaterial>::default())
        .init_resource::<crate::button_actions::ButtonActions>()
        .init_resource::<LoadingView>()
        .init_resource::<Shot>();
        crate::glue_menu::install(&mut app);
        app.add_systems(
            Update,
            (
                sync_view,
                drive_bar,
                campaign_score_model::animate_backgrounds,
            )
                .chain(),
        );
        let mut loading = CampaignLoading::default();
        loading.begin(
            request,
            &peacecraft3::map_requests::MapRequests::default(),
            None,
        );
        app.insert_resource(loading);
        app.finish();
        app.cleanup();
        app.update();
        app.update();
        let mut image =
            Image::new_target_texture(WIDTH, HEIGHT, TextureFormat::Rgba8UnormSrgb, None);
        image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
        let target = app.world_mut().resource_mut::<Assets<Image>>().add(image);
        let cameras: Vec<Entity> = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<LoadingCamera>>();
            query.iter(world).collect()
        };
        if cameras.len() != 3 {
            return Err(format!("{} loading cameras, expected 3", cameras.len()));
        }
        for camera in cameras {
            let mut entity = app.world_mut().entity_mut(camera);
            entity.insert(RenderTarget::Image(target.clone().into()));
            if let Some(mut camera) = entity.get_mut::<Camera>() {
                camera.viewport = None;
            }
        }
        for frame in 0..FRAMES {
            let signals = [
                peacecraft3::campaign_loading::Signals {
                    admitted: true,
                    ..Default::default()
                },
                peacecraft3::campaign_loading::Signals {
                    admitted: true,
                    prepared: true,
                    ..Default::default()
                },
                peacecraft3::campaign_loading::Signals {
                    admitted: true,
                    prepared: true,
                    published: true,
                    ..Default::default()
                },
            ];
            if let Some(reached) = stages.checked_sub(1).and_then(|i| signals.get(i)) {
                app.world_mut()
                    .resource_mut::<CampaignLoading>()
                    .observe(*reached);
            }
            if frame == READ_AT {
                app.world_mut()
                    .spawn(Readback::texture(target.clone()))
                    .observe(|event: On<ReadbackComplete>, mut shot: ResMut<Shot>| {
                        shot.0.clone_from(&event.data);
                    });
            }
            app.update();
            app.world()
                .resource::<RenderDevice>()
                .wgpu_device()
                .poll(PollType::Wait {
                    submission_index: None,
                    timeout: Some(std::time::Duration::from_secs(10)),
                })
                .map_err(|e| e.to_string())?;
        }
        let data = &app.world().resource::<Shot>().0;
        let (width, height) = (WIDTH as usize, HEIGHT as usize);
        if data.is_empty() || !data.len().is_multiple_of(height) {
            return Err(format!("readback returned {} bytes", data.len()));
        }
        let stride = data.len() / height;
        let mut ppm = format!("P6\n{WIDTH} {HEIGHT}\n255\n").into_bytes();
        for row in data.chunks(stride).take(height) {
            for pixel in row[..width * 4].chunks(4) {
                ppm.extend_from_slice(&pixel[..3]);
            }
        }
        std::fs::write(out, ppm).map_err(|e| e.to_string())
    }

    /// Build, label and pose the screen for `request` without a GPU or window.
    pub(crate) fn show(request: LoadingRequest) -> Result<Shown, String> {
        let mut world = World::new();
        world.init_resource::<Assets<Mesh>>();
        world.init_resource::<Assets<MenuMaterial>>();
        world.init_resource::<Assets<Image>>();
        world.init_resource::<Assets<Font>>();
        let request = Arc::new(request);
        world
            .run_system_once({
                let request = Arc::clone(&request);
                move |mut commands: Commands, mut stores: ViewStores| {
                    let mut s = Stores {
                        meshes: &mut stores.meshes,
                        materials: &mut stores.materials,
                        images: &mut stores.images,
                        fonts: &mut stores.fonts,
                    };
                    build(&mut commands, &request, UVec2::new(1280, 960), &mut s).map(|_| ())
                }
            })
            .map_err(|e| e.to_string())??;
        let mut texts = world.query::<&TextSpan>();
        let texts = texts.iter(&world).map(|span| span.0.clone()).collect();
        let mut states = world.query::<&ScoreBackgroundState>();
        let driven: Vec<_> = states.iter(&world).filter(|s| s.driven).collect();
        let [bar] = driven[..] else {
            return Err(format!("{} driven backgrounds, expected 1", driven.len()));
        };
        let bar_frames = [0.0, 0.5, 1.0]
            .into_iter()
            .map(|fraction| bar_frame(bar.interval, fraction))
            .collect();
        Ok(Shown { texts, bar_frames })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FDF: &str = r#"
Frame "FRAME" "Loading" {
    SetAllPoints,
}
Frame "SPRITE" "LoadingBar" {
    SetPoint BOTTOMLEFT, "Loading", BOTTOMLEFT, 0.01, 0.0025,
}
Frame "TEXT" "LoadingBarText" {
    SetPoint BOTTOM, "Loading", BOTTOM, 0.02, 0.057,
}
Frame "TEXT" "LoadingTitleText" {
    Width 0.36,
    SetPoint TOPRIGHT, "Loading", TOPRIGHT, -0.025, -0.295,
}
Frame "TEXT" "LoadingSubtitleText" {
    Width 0.36,
    SetPoint TOP, "LoadingTitleText", BOTTOM, 0.0, -0.003,
}
Frame "TEXT" "LoadingText" {
    Width 0.36,
    SetPoint TOP, "LoadingSubtitleText", BOTTOM, 0.0, -0.01,
}
"#;

    fn fdf(text: &str) -> Fdf {
        let mut fdf = Fdf::default();
        fdf.parse_into(text).unwrap();
        fdf
    }

    #[test]
    fn the_text_column_is_read_from_the_anchors() {
        let stack = text_stack(&fdf(FDF), &TEXT_STACK).unwrap();
        assert!((stack.right - 0.025).abs() < 1e-6);
        assert!((stack.top - 0.295).abs() < 1e-6);
        assert_eq!(stack.width, 0.36);
        assert_eq!(stack.gaps.len(), 3);
        assert_eq!(stack.gaps[0], 0.0);
        assert!((stack.gaps[1] - 0.003).abs() < 1e-6);
        assert!((stack.gaps[2] - 0.01).abs() < 1e-6);
    }

    #[test]
    fn an_anchor_or_width_the_column_cannot_express_refuses() {
        let narrower = FDF.replace(
            "Width 0.36,\n    SetPoint TOP, \"LoadingTitleText\"",
            "Width 0.2,\n    SetPoint TOP, \"LoadingTitleText\"",
        );
        assert_ne!(narrower, FDF);
        assert!(text_stack(&fdf(&narrower), &TEXT_STACK).is_err());
        let offset = FDF.replace("BOTTOM, 0.0, -0.003", "BOTTOM, 0.01, -0.003");
        assert!(text_stack(&fdf(&offset), &TEXT_STACK).is_err());
        let moved = FDF.replace(
            "TOPRIGHT, \"Loading\", TOPRIGHT",
            "TOPLEFT, \"Loading\", TOPLEFT",
        );
        assert!(text_stack(&fdf(&moved), &TEXT_STACK).is_err());
    }

    #[test]
    fn the_bar_and_its_text_sit_where_their_anchors_put_them() {
        let f = fdf(FDF);
        let root = f.rect(ROOT, (0.8, 0.6)).unwrap();
        let (x, y) = bar_origin(&f, root).unwrap();
        assert!((x - 0.01).abs() < 1e-6 && (y - 0.0025).abs() < 1e-6);
        assert_eq!(bar_text_offset(&f).unwrap(), (0.02, 0.057));
        let moved = FDF.replace("BOTTOMLEFT, 0.01", "BOTTOMRIGHT, 0.01");
        assert!(bar_origin(&fdf(&moved), root).is_err());
    }

    #[test]
    fn the_bar_pose_follows_its_fraction_and_never_leaves_the_sequence() {
        assert_eq!(bar_frame([3333, 26800], 0.0), 3333);
        assert_eq!(bar_frame([3333, 26800], 1.0), 26800);
        assert_eq!(bar_frame([0, 1000], 0.25), 250);
        assert_eq!(bar_frame([0, 1000], 7.0), 1000);
        assert_eq!(bar_frame([40, 40], 0.5), 40);
    }
}

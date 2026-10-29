//! `SetSkyModel` dome. The selected binary model is drawn on its own camera, which renders
//! before the scene camera and clears the target. While the sky is shown, the scene camera
//! clears nothing, so uncovered pixels show the sky instead of the clear colour. The scene
//! camera's previous clear colour is kept and restored when the sky hides.
//!
//! Source relationships (`docs/re-notes/campaign-sky.md`):
//! - Geometry is bound only to bone 0, which has no tracks. The helper `Dummy01` with its
//!   `KGRT` track is evaluated by the joint plan but no vertex references it, so the dome
//!   is drawn at rest.
//! - The authored lit layer uses the sky model's own directional-light tracks, sampled
//!   at the executed day fraction. Terrain and unit lighting are separate inputs.
//! - Placement at the scene camera eye and unit scale are unverified against native
//!   observation.
use super::{Actor, ActorQueries, ActorSource, DrawAssets, UnitActors, Visual, item_model};
use bevy::{
    camera::{Camera, ClearColorConfig, RenderTarget, visibility::RenderLayers},
    prelude::*,
};
use pcx_assets::Resolver;
use peacecraft3::campaign_camera::CampaignSceneCamera;

/// Render isolation for the sky dome, separate from scene geometry (`campaign_camera::RENDER_LAYER`).
pub(super) const RENDER_LAYER: usize = 2;
/// The sky camera renders before the scene camera.
const CAMERA_ORDER: isize = -1;
/// The dome is authored in world units and no script gives it a scale. Identity is a
/// placement choice; the source's world-to-model mapping is not separately established.
pub(super) const MODEL_SCALE: f32 = 1.0;
/// Source byte budget for one sky model member.
const MODEL_BYTES: usize = 16 << 20;
pub(super) struct SkyState {
    script: String,
    animation: pcx_play::unit_model::Animation,
    lighting: Result<pcx_play::day_night::Assets, String>,
    actor: Option<Actor>,
}

/// Sky lifecycle owned by the actor resource.
#[derive(Default)]
pub(super) struct Sky {
    /// The requested dome. Spawn failures clear it, so `Some` means the dome is shown.
    state: Option<SkyState>,
    /// The script whose load or spawn failed. It is not retried until the script changes.
    failed: Option<String>,
    camera: Option<Entity>,
    /// The scene camera's clear colour before the sky overrode it, and that camera.
    scene_clear: Option<(Entity, ClearColorConfig)>,
}

impl Sky {
    pub(super) fn is_shown(&self) -> bool {
        self.state.is_some()
    }
}

/// Marks the one camera that draws the sky.
#[derive(Component)]
pub(crate) struct SkyCamera;

pub(super) struct Input<'a> {
    /// Executed `SetSkyModel` path. `None` (null or never set) shows no sky.
    pub(super) script: Option<String>,
    pub(super) reader: &'a Resolver,
    pub(super) edition: &'a str,
    pub(super) palette: &'a super::TeamPalette,
    pub(super) terrain: &'a pcx_map::Terrain,
    pub(super) now: f64,
    pub(super) channel_scale: f32,
    pub(super) cutout: f32,
    pub(super) day_fraction: Option<f32>,
    pub(super) parameters: &'a pcx_play::menu_lighting::Parameters,
}

pub(super) type SceneCameras<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static Projection,
        &'static mut Camera,
        Option<&'static RenderTarget>,
    ),
    With<CampaignSceneCamera>,
>;

fn load(reader: &Resolver, edition: &str, script: &str, now: f64) -> Result<SkyState, String> {
    let member = item_model::model_member(script)?;
    let model = item_model::read_model(reader, edition, member, MODEL_BYTES)?;
    let lighting = own_lighting(&model, script);
    Ok(SkyState {
        script: script.into(),
        animation: pcx_play::unit_model::Animation::new(model, now)?,
        lighting,
        actor: None,
    })
}

fn own_lighting(
    model: &pcx_play::unit_model::Model,
    script: &str,
) -> Result<pcx_play::day_night::Assets, String> {
    use pcx_play::{
        day_night::{Assets, Budget, ModelInput},
        scripts::ScriptAsset,
    };
    Assets::read(
        &[ModelInput {
            script_path: script,
            asset: ScriptAsset {
                label: &model.member,
                bytes: &model.bytes,
            },
            origin: model.origin.clone(),
        }],
        Budget {
            models: 1,
            model_bytes: MODEL_BYTES,
            total_bytes: MODEL_BYTES,
            path_bytes: 64 << 10,
        },
    )
    .map_err(|error| error.to_string())
}

fn sun(sky: &SkyState, fraction: Option<f32>) -> Result<pcx_play::day_night::Sun, String> {
    let lighting = sky.lighting.as_ref().map_err(Clone::clone)?;
    lighting.models()[0].sun(fraction.ok_or("sky day fraction is unavailable")?)
}

/// Despawns the dome and its camera. The scene clear colour is restored by the next sync.
/// Session replacement: restore the scene camera clear and drop the dome before the
/// actor state resets, so a replaced session never inherits the sky override.
pub(super) fn release(commands: &mut Commands, state: &mut UnitActors, scene: &mut SceneCameras) {
    retire(commands, state);
    sync_scene_camera(commands, &mut state.sky, scene, false);
}

pub(super) fn retire(commands: &mut Commands, state: &mut UnitActors) {
    if let Some(sky) = state.sky.state.take()
        && let Some(actor) = sky.actor
    {
        commands.entity(actor.root).despawn();
        state.image_bytes -= actor.image_bytes;
        state.vertices -= actor.vertices;
    }
    if let Some(camera) = state.sky.camera.take() {
        commands.entity(camera).despawn();
    }
}

/// Keeps one sky camera mirroring the scene camera while shown. The scene camera's clear
/// colour is saved before it is set to `None`, and restored only if it is still `None`
/// when the sky hides. Other scene camera fields are never written.
pub(super) fn sync_scene_camera(
    commands: &mut Commands,
    sky: &mut Sky,
    scene: &mut SceneCameras,
    showing: bool,
) {
    let live: Vec<Entity> = scene.iter().map(|(entity, ..)| entity).collect();
    if sky
        .scene_clear
        .as_ref()
        .is_some_and(|(saved, _)| !live.contains(saved))
    {
        sky.scene_clear = None;
    }
    for (entity, transform, projection, mut camera, target) in scene.iter_mut() {
        if showing {
            if !matches!(camera.clear_color, ClearColorConfig::None) {
                let previous = std::mem::replace(&mut camera.clear_color, ClearColorConfig::None);
                sky.scene_clear = Some((entity, previous));
            }
        } else if let Some((_, previous)) = sky.scene_clear.take_if(|(saved, _)| *saved == entity)
            && matches!(camera.clear_color, ClearColorConfig::None)
        {
            camera.clear_color = previous;
        }
        if !showing {
            continue;
        }
        let sky_camera = *sky.camera.get_or_insert_with(|| {
            commands
                .spawn((SkyCamera, Name::new("Campaign sky camera")))
                .id()
        });
        // Only the settings that select where and what the camera draws are mirrored.
        commands.entity(sky_camera).insert((
            Camera {
                order: CAMERA_ORDER,
                is_active: camera.is_active,
                viewport: camera.viewport.clone(),
                sub_camera_view: camera.sub_camera_view,
                clear_color: sky
                    .scene_clear
                    .as_ref()
                    .filter(|(saved, _)| *saved == entity)
                    .map_or(camera.clear_color, |(_, previous)| *previous),
                ..default()
            },
            Camera3d::default(),
            *transform,
            projection.clone(),
            RenderLayers::layer(RENDER_LAYER),
        ));
        if let Some(target) = target {
            commands.entity(sky_camera).insert(target.clone());
        }
    }
    if !showing && let Some(camera) = sky.camera.take() {
        commands.entity(camera).despawn();
    }
}

/// Camera step shared by the frame system and the tests.
pub(super) fn sync_frame(
    commands: &mut Commands,
    state: &mut UnitActors,
    scene: &mut SceneCameras,
) {
    let showing = state.sky.is_shown();
    sync_scene_camera(commands, &mut state.sky, scene, showing);
}

fn fail(commands: &mut Commands, state: &mut UnitActors, script: &str, error: String) {
    retire(commands, state);
    state.last_error = Some(format!("sky {script}: {error}"));
    state.sky.failed = Some(script.to_owned());
}

/// Loads, spawns and updates the dome for this frame.
fn show(
    commands: &mut Commands,
    state: &mut UnitActors,
    assets: &mut DrawAssets,
    queries: &mut ActorQueries,
    input: &Input,
) {
    let Some(script) = input.script.as_deref() else {
        retire(commands, state);
        return;
    };
    // Without a live scene camera there is no eye to centre the dome on.
    let Some(eye) = queries.scene.iter().next().map(|(_, t, ..)| t.translation) else {
        return;
    };
    if state.sky.failed.as_deref() == Some(script) {
        return;
    }
    if state
        .sky
        .state
        .as_ref()
        .is_some_and(|sky| sky.script != script)
    {
        retire(commands, state);
    }
    if state.sky.state.is_none() {
        match load(input.reader, input.edition, script, input.now) {
            Ok(sky) => state.sky.state = Some(sky),
            Err(error) => {
                fail(commands, state, script, error);
                return;
            }
        }
    }
    let Some(animation) = state.sky.state.as_ref().map(|sky| sky.animation.clone()) else {
        return;
    };
    let sun = match sun(
        state.sky.state.as_ref().expect("loaded sky"),
        input.day_fraction,
    ) {
        Ok(sun) => Some(sun),
        Err(error) => {
            state.last_error = Some(format!("sky {script}: {error}"));
            None
        }
    };
    let visual = Visual::Sky(&animation, eye);
    if state
        .sky
        .state
        .as_ref()
        .is_some_and(|sky| sky.actor.is_none())
    {
        let result = Actor::spawn(
            commands,
            assets,
            ActorSource {
                reader: input.reader,
                palette: input.palette,
                visual,
                terrain: input.terrain,
                color: 0,
                now: input.now,
                channel_scale: input.channel_scale,
                cutout: input.cutout,
                particle_parameters: None,
                available_images: (512usize << 20).saturating_sub(state.image_bytes),
                available_vertices: 2_000_000usize.saturating_sub(state.vertices),
            },
        );
        match result {
            Ok((actor, bytes, vertices)) => {
                state.image_bytes += bytes;
                state.vertices += vertices;
                if let Some(sky) = state.sky.state.as_mut() {
                    sky.actor = Some(actor);
                }
            }
            Err(error) => {
                fail(commands, state, script, error);
                return;
            }
        }
    }
    if let Some(actor) = state.sky.state.as_mut().and_then(|sky| sky.actor.as_mut())
        && let Err(error) = actor.update(
            visual,
            input.now,
            assets,
            (&mut queries.visibility, &mut queries.transforms),
            input.terrain,
            (sun.as_ref().map(|sun| (sun, input.parameters)), None),
        )
    {
        state.last_error = Some(format!("sky: {error}"));
    }
}

pub(super) fn project(
    commands: &mut Commands,
    state: &mut UnitActors,
    assets: &mut DrawAssets,
    queries: &mut ActorQueries,
    input: Input,
) {
    show(commands, state, assets, queries, &input);
    sync_frame(commands, state, &mut queries.scene);
}

#[cfg(test)]
mod tests {
    //! Narrow camera checks. They drive `sync_frame` and `fail`/`retire` in a minimal `App`,
    //! one scripted step per update, with a substitute dome state that has no actor. They do
    //! not exercise the production `project` reset or load path, which the integration
    //! publication tests in `tests/support/campaign_sky_cases.rs` cover only up to the
    //! published session.
    use super::*;
    use bevy::camera::Viewport;
    use std::collections::VecDeque;

    const SCENE_LAYER: usize = peacecraft3::campaign_camera::RENDER_LAYER;
    fn red() -> ClearColorConfig {
        ClearColorConfig::Custom(Color::srgb(1.0, 0.0, 0.0))
    }
    fn green() -> ClearColorConfig {
        ClearColorConfig::Custom(Color::srgb(0.0, 1.0, 0.0))
    }
    fn blue() -> ClearColorConfig {
        ClearColorConfig::Custom(Color::srgb(0.0, 0.0, 1.0))
    }

    enum Step {
        /// Show a dome (a substitute state with no actor).
        Dome,
        /// A failed load for the named script.
        Fail,
        /// `SetSkyModel(null)`.
        Null,
        /// Session replacement: the old scene camera is despawned and a new one with the
        /// given clear colour takes its place, as the session reset does.
        Replace(ClearColorConfig),
    }

    #[derive(Resource, Default)]
    struct Script(VecDeque<Step>);

    fn substitute_dome() -> SkyState {
        let model = pcx_play::unit_model::Model {
            origin: pcx_play::scripts::AssetIdentity {
                edition: "test".into(),
                version: "test".into(),
                source: "test".into(),
            },
            member: String::new(),
            bytes: std::sync::Arc::from(Vec::new()),
            sequences: Vec::new(),
            blend_time_ms: 0,
            joints: Vec::new(),
            global_sequences: Vec::new(),
        };
        SkyState {
            script: "Environment\\Sky\\Test\\Test.mdl".into(),
            animation: pcx_play::unit_model::Animation::new(std::sync::Arc::new(model), 0.0)
                .unwrap(),
            lighting: Err("synthetic camera-only fixture has no light".into()),
            actor: None,
        }
    }

    fn scene_bundle(clear: ClearColorConfig) -> impl Bundle {
        (
            CampaignSceneCamera,
            Camera3d::default(),
            Camera {
                clear_color: clear,
                ..default()
            },
            Transform::from_xyz(1.0, 2.0, 3.0),
            Projection::Perspective(PerspectiveProjection {
                far: 2500.0,
                ..default()
            }),
            RenderLayers::layer(SCENE_LAYER),
        )
    }

    fn tick(
        mut commands: Commands,
        mut state: ResMut<UnitActors>,
        mut script: ResMut<Script>,
        mut scene: SceneCameras,
    ) {
        match script.0.pop_front() {
            None => {}
            Some(Step::Dome) => state.sky.state = Some(substitute_dome()),
            Some(Step::Fail) => fail(
                &mut commands,
                &mut state,
                "Environment\\Sky\\Missing\\Missing.mdl",
                "load failed".into(),
            ),
            Some(Step::Null) => retire(&mut commands, &mut state),
            Some(Step::Replace(clear)) => {
                for (entity, ..) in scene.iter() {
                    commands.entity(entity).despawn();
                }
                retire(&mut commands, &mut state);
                *state = UnitActors::default();
                commands.spawn(scene_bundle(clear));
                // The new camera appears in the next frame, so the sync waits for it.
                return;
            }
        }
        sync_frame(&mut commands, &mut state, &mut scene);
    }

    fn app(steps: impl IntoIterator<Item = Step>, scene_clear: ClearColorConfig) -> App {
        let mut app = App::new();
        app.init_resource::<UnitActors>()
            .insert_resource(Script(steps.into_iter().collect()))
            .add_systems(Update, tick);
        app.world_mut().spawn(scene_bundle(scene_clear));
        app
    }

    fn scene(app: &mut App) -> Entity {
        app.world_mut()
            .query_filtered::<Entity, With<CampaignSceneCamera>>()
            .single(app.world())
            .unwrap()
    }

    fn scene_clear(app: &mut App) -> ClearColorConfig {
        let entity = scene(app);
        app.world().get::<Camera>(entity).unwrap().clear_color
    }

    fn sky_camera_count(app: &mut App) -> usize {
        app.world_mut()
            .query_filtered::<Entity, With<SkyCamera>>()
            .iter(app.world())
            .count()
    }

    fn assert_clear(actual: ClearColorConfig, expected: &ClearColorConfig) {
        match (&actual, expected) {
            (ClearColorConfig::Default, ClearColorConfig::Default)
            | (ClearColorConfig::None, ClearColorConfig::None) => {}
            (ClearColorConfig::Custom(a), ClearColorConfig::Custom(b)) => assert_eq!(a, b),
            _ => panic!("clear colour {actual:?}, expected {expected:?}"),
        }
    }

    #[test]
    fn initially_none_stays_none_after_hiding() {
        let mut app = app([Step::Dome, Step::Null], ClearColorConfig::None);
        app.update();
        assert_clear(scene_clear(&mut app), &ClearColorConfig::None);
        assert_eq!(sky_camera_count(&mut app), 1);
        app.update();
        assert_clear(scene_clear(&mut app), &ClearColorConfig::None);
        assert_eq!(sky_camera_count(&mut app), 0);
    }

    #[test]
    fn previous_custom_colour_is_restored_on_null() {
        let mut app = app([Step::Dome, Step::Null], red());
        app.update();
        assert_clear(scene_clear(&mut app), &ClearColorConfig::None);
        app.update();
        assert_clear(scene_clear(&mut app), &red());
        assert_eq!(sky_camera_count(&mut app), 0);
    }

    #[test]
    fn load_failure_restores_the_previous_colour() {
        let mut app = app([Step::Dome, Step::Fail], green());
        app.update();
        app.update();
        assert_clear(scene_clear(&mut app), &green());
        assert_eq!(sky_camera_count(&mut app), 0);
        assert!(!app.world().resource::<UnitActors>().sky.is_shown());
        assert!(app.world().resource::<UnitActors>().sky.failed.is_some());
    }

    #[test]
    fn replaced_scene_camera_keeps_its_own_colour() {
        let mut app = app(
            [Step::Dome, Step::Replace(blue()), Step::Dome, Step::Null],
            red(),
        );
        app.update();
        app.update();
        // The replacement camera keeps its own colour and never receives the old red.
        assert_clear(scene_clear(&mut app), &blue());
        assert_eq!(sky_camera_count(&mut app), 0);
        app.update();
        assert_clear(scene_clear(&mut app), &ClearColorConfig::None);
        assert_eq!(sky_camera_count(&mut app), 1);
        app.update();
        assert_clear(scene_clear(&mut app), &blue());
        assert_eq!(sky_camera_count(&mut app), 0);
    }

    #[test]
    fn caller_clear_change_while_shown_is_kept_and_other_fields_are_untouched() {
        let mut app = app([Step::Dome, Step::Null, Step::Null], red());
        app.update();
        // Unrelated caller edits while the sky is shown.
        let entity = scene(&mut app);
        {
            let mut camera = app.world_mut().get_mut::<Camera>(entity).unwrap();
            camera.order = 7;
            camera.is_active = true;
        }
        app.world_mut()
            .get_mut::<Camera>(entity)
            .unwrap()
            .clear_color = green();
        app.update();
        let camera = app.world().get::<Camera>(entity).unwrap();
        assert_eq!(camera.order, 7, "unrelated field is not reset");
        assert_clear(camera.clear_color, &green());
        assert_eq!(sky_camera_count(&mut app), 0);
    }

    #[test]
    fn sky_camera_mirrors_viewport_projection_and_layers_once() {
        let mut app = app([Step::Dome, Step::Dome, Step::Dome], ClearColorConfig::None);
        {
            let entity = scene(&mut app);
            let mut camera = app.world_mut().get_mut::<Camera>(entity).unwrap();
            camera.viewport = Some(Viewport {
                physical_position: UVec2::new(3, 4),
                physical_size: UVec2::new(640, 480),
                ..default()
            });
        }
        app.update();
        app.update();
        app.update();
        assert_eq!(sky_camera_count(&mut app), 1, "exactly one sky camera");
        let sky = app
            .world_mut()
            .query_filtered::<Entity, With<SkyCamera>>()
            .single(app.world())
            .unwrap();
        let camera = app.world().get::<Camera>(sky).unwrap();
        assert_eq!(camera.order, CAMERA_ORDER);
        assert_eq!(
            camera.viewport.as_ref().unwrap().physical_size,
            UVec2::new(640, 480)
        );
        assert!(matches!(camera.clear_color, ClearColorConfig::None));
        let scene_entity = scene(&mut app);
        let world = app.world();
        assert!(
            !world
                .get::<RenderLayers>(sky)
                .unwrap()
                .intersects(world.get::<RenderLayers>(scene_entity).unwrap())
        );
        assert!(world.get::<RenderTarget>(sky).is_some());
    }

    #[test]
    fn sky_camera_uses_the_scene_clear_colour() {
        let mut app = app([Step::Dome, Step::Dome], red());
        app.update();
        app.update();
        let sky = app
            .world_mut()
            .query_filtered::<Entity, With<SkyCamera>>()
            .single(app.world())
            .unwrap();
        assert_clear(app.world().get::<Camera>(sky).unwrap().clear_color, &red());
    }

    #[test]
    fn sky_script_path_selects_the_binary_member() {
        assert_eq!(
            item_model::model_member(r"Environment\Sky\LordaeronSummerSky\LordaeronSummerSky.mdl")
                .unwrap(),
            r"Environment\Sky\LordaeronSummerSky\LordaeronSummerSky.mdx"
        );
        assert!(item_model::model_member("Environment\\Sky\\Sky.blp").is_err());
    }

    fn lighting_model() -> pcx_play::unit_model::Model {
        fn chunk(bytes: &mut Vec<u8>, tag: &[u8; 4], data: &[u8]) {
            bytes.extend(tag);
            bytes.extend((data.len() as u32).to_le_bytes());
            bytes.extend(data);
        }
        let mut bytes = b"MDLX".to_vec();
        chunk(&mut bytes, b"VERS", &800u32.to_le_bytes());
        chunk(&mut bytes, b"MODL", &[0; 372]);
        let mut sequence = [0; 132];
        sequence[..5].copy_from_slice(b"Stand");
        sequence[80..84].copy_from_slice(&17u32.to_le_bytes());
        sequence[84..88].copy_from_slice(&117u32.to_le_bytes());
        chunk(&mut bytes, b"SEQS", &sequence);
        chunk(&mut bytes, b"GLBS", &0u32.to_le_bytes());
        let mut node = vec![0; 96];
        node[..4].copy_from_slice(&132u32.to_le_bytes());
        node[88..92].copy_from_slice(&(-1i32).to_le_bytes());
        node.extend(b"KGRT");
        for v in [1u32, 0, 0, 17] {
            node.extend(v.to_le_bytes());
        }
        for v in [0.0f32, 0., 0., 1.] {
            node.extend(v.to_le_bytes());
        }
        let mut light = vec![0; 4];
        light.extend(node);
        light.extend(1u32.to_le_bytes());
        for v in [0.0f32, 0., 0.15, 0.25, 0.35, 2.25, 0.05, 0.1, 0.15, 1.75] {
            light.extend(v.to_le_bytes());
        }
        for (tag, first, last) in [
            (b"KLAC", [0.15f32, 0.25, 0.35], [0.85, 0.75, 0.65]),
            (b"KLBC", [0.05, 0.1, 0.15], [0.4, 0.3, 0.2]),
        ] {
            light.extend(tag);
            for v in [2u32, 1, u32::MAX] {
                light.extend(v.to_le_bytes());
            }
            for (frame, values) in [(17u32, first), (117, last)] {
                light.extend(frame.to_le_bytes());
                for v in values {
                    light.extend(v.to_le_bytes());
                }
            }
        }
        let length = light.len() as u32;
        light[..4].copy_from_slice(&length.to_le_bytes());
        chunk(&mut bytes, b"LITE", &light);
        pcx_play::unit_model::Model::read(
            pcx_play::scripts::AssetIdentity {
                edition: "synthetic".into(),
                version: "nonstock".into(),
                source: "owned-sky-light".into(),
            },
            "synthetic-sky.mdx".into(),
            bytes.into(),
            MODEL_BYTES,
        )
        .unwrap()
    }

    #[test]
    fn own_sky_light_uses_source_tracks_and_live_day_fraction() {
        let model = std::sync::Arc::new(lighting_model());
        let lighting = own_lighting(&model, "synthetic-sky.mdl").unwrap();
        assert_eq!(lighting.models()[0].origin(), &model.origin);
        let sky = SkyState {
            script: "synthetic-sky.mdl".into(),
            lighting: Ok(lighting),
            animation: pcx_play::unit_model::Animation::new(model, 0.).unwrap(),
            actor: None,
        };
        let first = sun(&sky, Some(0.)).unwrap();
        let last = sun(&sky, Some(1.)).unwrap();
        let middle = sun(&sky, Some(0.5)).unwrap();
        assert_eq!(first.intensity, 2.25);
        assert_eq!(first.ambient_intensity, 1.75);
        assert_eq!(first.direction, [0., 0., -1.]);
        assert_ne!(first.colour, last.colour);
        for i in 0..3 {
            assert!((middle.colour[i] - (first.colour[i] + last.colour[i]) * 0.5).abs() < 0.00001);
        }
        assert!(sun(&sky, None).is_err());
        assert!(sun(&sky, Some(f32::NAN)).is_err());
    }

    #[test]
    #[ignore = "requires PCX_SKY_ORIGINAL_MDX pointing at a local LordaeronSummerSky.mdx"]
    fn original_sky_model_has_a_stand_sequence() {
        let path = std::env::var_os("PCX_SKY_ORIGINAL_MDX").expect("explicit sky model path");
        let bytes = std::fs::read(path).unwrap();
        let origin = pcx_play::scripts::AssetIdentity {
            edition: "explicit local".into(),
            version: "unversioned source".into(),
            source: "PCX_SKY_ORIGINAL_MDX".into(),
        };
        let model = pcx_play::unit_model::Model::read(
            origin,
            r"Environment\Sky\LordaeronSummerSky\LordaeronSummerSky.mdx".into(),
            bytes.into(),
            MODEL_BYTES,
        )
        .unwrap();
        let lighting = own_lighting(&model, "selected sky").unwrap();
        let first = lighting.models()[0].sun(0.).unwrap();
        let noon = lighting.models()[0].sun(0.5).unwrap();
        assert_ne!(first.colour, noon.colour);
        assert_ne!(first.ambient_colour, noon.ambient_colour);
        assert_eq!(noon.intensity, lighting.models()[0].lights()[0].intensity);
        let animation =
            pcx_play::unit_model::Animation::new(std::sync::Arc::new(model), 0.0).unwrap();
        assert!(animation.has_sequence("stand"));
    }
}

#[cfg(all(test, not(target_arch = "wasm32"), feature = "test-fixtures"))]
#[allow(
    dead_code,
    reason = "shared synthetic fixture; the production tests use the model and texture"
)]
#[path = "../tests/support/synthetic_sky.rs"]
pub(super) mod synthetic_sky;

/// Production `project` through a published session. Publication, reader and session come
/// from the shared startup fixture (`test-fixtures`). Lighting parameters and the palette
/// are seeded on `UnitActors` with the labelled synthetic values from `menu_lighting::tests`
/// and `TeamPalette`, because reading them needs the installed client.
#[cfg(all(test, not(target_arch = "wasm32"), feature = "test-fixtures"))]
mod production {
    use super::*;
    use crate::binding::Reply;
    use crate::campaign_actors::{TeamPalette, UnitActors};
    use crate::campaign_startup_cases::{fixture, host_inputs};
    use peacecraft3::{
        browser_configuration::SelectedMapSources, campaign_startup::CampaignHostSupply,
        map_requests::MapRequests,
    };
    use std::{collections::BTreeMap, sync::Arc};

    const SKY_PATH: &str = r"Environment\Sky\Test\Test.mdl";
    const SKY_MEMBER: &str = "Environment/Sky/Test/Test.mdx";
    const SKY_TEXTURE: &str = "Environment/Sky/Test/Test.blp";

    fn red() -> ClearColorConfig {
        ClearColorConfig::Custom(Color::srgb(1.0, 0.0, 0.0))
    }

    fn sky_script(call: &str) -> String {
        format!(
            "globals\ninteger marker = 1\nendglobals\n\
             function config takes nothing returns nothing\nendfunction\n\
             function main takes nothing returns nothing\n{call}\nendfunction\n"
        )
    }

    /// The script spelling of a path: doubled backslashes, as shipped map scripts use.
    fn set_sky(path: &str) -> String {
        sky_script(&format!(
            "call SetSkyModel(\"{}\")",
            path.replace('\\', "\\\\")
        ))
    }

    fn sky_assets() -> BTreeMap<String, Reply> {
        BTreeMap::from([
            (
                SKY_MEMBER.to_owned(),
                Reply::Bytes(super::synthetic_sky::sky_model()),
            ),
            (
                SKY_TEXTURE.to_owned(),
                Reply::Bytes(super::synthetic_sky::sky_texture()),
            ),
        ])
    }

    fn seed(app: &mut App) {
        let mut state = app.world_mut().resource_mut::<UnitActors>();
        state.parameters_attempted = true;
        state.parameters = Some(std::sync::Arc::new(
            crate::menu_lighting::tests::parameters(),
        ));
        state.palette = Some(std::sync::Arc::new(TeamPalette {
            prefixes: [
                "ReplaceableTextures\\TeamColor\\TeamColor".into(),
                "ReplaceableTextures\\TeamGlow\\TeamGlow".into(),
            ],
            count: 1,
        }));
    }

    /// Publish a session through the production map request path, then seed the lighting
    /// inputs the session's first `project` pass could not read.
    fn publish(app: &mut App, script: &str, revision: u64, assets: BTreeMap<String, Reply>) {
        let mut fixture = fixture(script);
        fixture.common.extend_from_slice(
            crate::campaign_startup_cases::script_timer_cases::TIMER_COMMON.as_bytes(),
        );
        fixture.rebuild();
        let asset_count = assets.len();
        let resolver = Arc::new(fixture.resolver_with_assets(assets));
        let mut limits = fixture.limits();
        limits.reader.max_listed_paths += asset_count;
        let mut requests = MapRequests::default();
        let mut sources = SelectedMapSources::default();
        let outcome =
            requests.replace_selected(&mut sources, resolver, fixture.input(revision), limits);
        assert!(outcome.result.is_ok(), "{:?}", outcome.result);
        let mut inputs = host_inputs(requests.bindings.as_ref().unwrap().catalog.0.clone());
        inputs.timer_time = Some(0.0);
        app.insert_resource(requests)
            .insert_resource(sources)
            .insert_resource(CampaignHostSupply {
                revision,
                inputs: Some(inputs),
            });
        app.update();
        app.update();
        seed(app);
    }

    fn app() -> App {
        crate::campaign_workers::initialize();
        let mut app = App::new();
        peacecraft3::map_requests::install(&mut app);
        peacecraft3::campaign_startup::install(&mut app);
        crate::campaign_actors::install(&mut app);
        app.init_resource::<crate::launch_gui::Launcher>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Assets<crate::menu_material::MenuMaterial>>()
            .init_resource::<Assets<crate::mainmenu_scene::ParticleMaterial>>();
        app
    }

    fn scene_camera(app: &mut App, clear: ClearColorConfig) -> Entity {
        app.world_mut()
            .spawn((
                CampaignSceneCamera,
                Camera3d::default(),
                Camera {
                    clear_color: clear,
                    ..default()
                },
                Transform::from_xyz(1.0, 2.0, 3.0),
                Projection::Perspective(PerspectiveProjection {
                    far: 2500.0,
                    ..default()
                }),
                RenderLayers::layer(peacecraft3::campaign_camera::RENDER_LAYER),
            ))
            .id()
    }

    fn scene_clear(app: &mut App) -> ClearColorConfig {
        let entity = app
            .world_mut()
            .query_filtered::<Entity, With<CampaignSceneCamera>>()
            .single(app.world())
            .unwrap();
        app.world().get::<Camera>(entity).unwrap().clear_color
    }

    fn sky_camera_count(app: &mut App) -> usize {
        app.world_mut()
            .query_filtered::<Entity, With<SkyCamera>>()
            .iter(app.world())
            .count()
    }

    fn mesh_entities(app: &mut App) -> usize {
        app.world_mut()
            .query_filtered::<Entity, With<Mesh3d>>()
            .iter(app.world())
            .count()
    }

    /// Owned dome resources: spawned geosets, decoded image bytes and vertex count.
    fn owned(app: &App) -> (usize, usize, usize) {
        let state = app.world().resource::<UnitActors>();
        let geosets = state
            .sky
            .state
            .as_ref()
            .and_then(|sky| sky.actor.as_ref())
            .map_or(0, |actor| actor.geosets.bound.len());
        (geosets, state.image_bytes, state.vertices)
    }

    fn is_shown(app: &App) -> bool {
        app.world().resource::<UnitActors>().sky.is_shown()
    }

    fn assert_red(clear: ClearColorConfig) {
        match clear {
            ClearColorConfig::Custom(color) => assert_eq!(color, Color::srgb(1.0, 0.0, 0.0)),
            other => panic!("scene clear {other:?}, expected the caller's red"),
        }
    }

    #[test]
    fn executed_sky_is_spawned_by_project_and_hides_the_scene_clear() {
        let mut app = app();
        scene_camera(&mut app, red());
        publish(&mut app, &set_sky(SKY_PATH), 1, sky_assets());
        app.update();
        assert!(
            is_shown(&app),
            "last error: {:?}",
            app.world().resource::<UnitActors>().last_error
        );
        let (geosets, image_bytes, vertices) = owned(&app);
        assert!(geosets >= 1, "the dome geoset is spawned");
        assert!(
            image_bytes > 0 && vertices > 0,
            "decoded texture and vertices are owned"
        );
        assert!(
            mesh_entities(&mut app) >= 1,
            "the geoset mesh entity exists"
        );
        assert_eq!(sky_camera_count(&mut app), 1);
        assert!(matches!(scene_clear(&mut app), ClearColorConfig::None));
    }

    #[test]
    fn null_session_replacement_retires_the_dome_and_restores_the_caller_clear() {
        let mut app = app();
        scene_camera(&mut app, red());
        publish(&mut app, &set_sky(SKY_PATH), 1, sky_assets());
        app.update();
        assert!(
            owned(&app).0 >= 1 && mesh_entities(&mut app) >= 1,
            "dome owns geosets before retirement"
        );
        publish(
            &mut app,
            &sky_script("call SetSkyModel(null)"),
            2,
            BTreeMap::new(),
        );
        app.update();
        assert!(!is_shown(&app));
        assert_eq!(
            owned(&app),
            (0, 0, 0),
            "retired dome releases its geosets, images and vertices"
        );
        assert_eq!(
            mesh_entities(&mut app),
            0,
            "retired geoset entities are despawned"
        );
        assert_eq!(sky_camera_count(&mut app), 0);
        assert_red(scene_clear(&mut app));
    }

    #[test]
    fn load_failure_shows_nothing_and_leaves_the_caller_clear_untouched() {
        let mut app = app();
        scene_camera(&mut app, red());
        publish(
            &mut app,
            &set_sky(r"Environment\Sky\Missing\Missing.mdl"),
            1,
            BTreeMap::new(),
        );
        app.update();
        assert!(!is_shown(&app));
        assert!(app.world().resource::<UnitActors>().sky.failed.is_some());
        assert_eq!(sky_camera_count(&mut app), 0);
        assert_red(scene_clear(&mut app));
    }

    #[test]
    fn replacing_the_session_with_the_same_sky_keeps_one_camera_and_one_saved_clear() {
        let mut app = app();
        scene_camera(&mut app, red());
        publish(&mut app, &set_sky(SKY_PATH), 1, sky_assets());
        app.update();
        publish(&mut app, &set_sky(SKY_PATH), 2, sky_assets());
        app.update();
        app.update();
        assert!(is_shown(&app));
        assert_eq!(sky_camera_count(&mut app), 1, "exactly one sky camera");
        assert!(matches!(scene_clear(&mut app), ClearColorConfig::None));
        publish(
            &mut app,
            &sky_script("call SetSkyModel(null)"),
            3,
            BTreeMap::new(),
        );
        app.update();
        assert_eq!(sky_camera_count(&mut app), 0);
        assert_red(scene_clear(&mut app));
    }
}

//! Camera consumer for executed instant setup applications on the published VM.
use crate::{
    browser_configuration::SelectedMapSources,
    campaign_startup::CampaignStartupSet,
    map_binding::{AssetReceipt, receipt},
    map_ground::GroundPublication,
    map_requests::MapRequests,
    map_script_session::MapScriptSession,
};
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use pcx_play::map_startup_host::MapStartupHost;
use pcx_script::Value;
use std::sync::{Arc, Weak};

const MISC_DATA: &str = "UI\\MiscData.txt";
/// Application render isolation, separate from menu geometry and menu sprite layer 7.
pub const RENDER_LAYER: usize = 1;
const ROLL: &str = "CAMERA_FIELD_ROLL";
const FIELDS: [&str; 7] = [
    "CAMERA_FIELD_TARGET_DISTANCE",
    "CAMERA_FIELD_FARZ",
    "CAMERA_FIELD_ANGLE_OF_ATTACK",
    "CAMERA_FIELD_FIELD_OF_VIEW",
    "CAMERA_FIELD_ROLL",
    "CAMERA_FIELD_ROTATION",
    "CAMERA_FIELD_ZOFFSET",
];

#[derive(Component)]
pub struct CampaignSceneCamera;

/// Projection selection is application policy. Clip values still come from the selected assets.
#[derive(Resource)]
pub struct CampaignCamera {
    zoom: usize,
    read_limit: usize,
    session: Option<Weak<()>>,
    attempted: Option<u64>,
    near: Option<f32>,
    pub menu_key: Option<Result<KeyCode, String>>,
    pub menu_key_receipt: Option<AssetReceipt>,
    visible: bool,
    menu_open: bool,
    ready: bool,
    pub entity: Option<Entity>,
    pub clip_receipt: Option<AssetReceipt>,
    pub last_error: Option<String>,
}

impl CampaignCamera {
    /// Hide a retired or explicitly suspended campaign view.
    pub fn suspend(&mut self) {
        self.visible = false;
        self.menu_open = false;
    }

    pub fn menu_open(&self) -> bool {
        self.menu_open
    }

    pub fn set_menu_open(&mut self, open: bool) {
        self.menu_open = open && self.active();
    }

    pub fn active(&self) -> bool {
        self.ready && self.visible && self.entity.is_some()
    }

    /// Admitted, visible projection for the inventory ECS tests, which cannot reach the
    /// private camera state. Built only with the `test-fixtures` feature.
    #[cfg(feature = "test-fixtures")]
    pub fn active_for_test(entity: Entity) -> Self {
        Self {
            zoom: 0,
            read_limit: 0,
            session: None,
            attempted: None,
            near: None,
            menu_key: None,
            menu_key_receipt: None,
            visible: true,
            menu_open: false,
            ready: true,
            entity: Some(entity),
            clip_receipt: None,
            last_error: None,
        }
    }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CampaignCameraSet;

pub fn install(app: &mut App, zoom: usize, read_limit: usize) {
    app.insert_resource(CampaignCamera {
        zoom,
        read_limit,
        session: None,
        attempted: None,
        near: None,
        menu_key: None,
        menu_key_receipt: None,
        visible: true,
        menu_open: false,
        ready: false,
        entity: None,
        clip_receipt: None,
        last_error: None,
    })
    .add_systems(
        Update,
        (menu_keys, scroll_camera, apply_camera, pause_view)
            .chain()
            .in_set(CampaignCameraSet)
            .after(CampaignStartupSet),
    );
}

/// Arrow-key scrolling at the selected `[CameraRates]` speeds, along the live
/// camera rotation, clamped to the executed camera bounds. Real time paces it;
/// it is off while scripts disable user control.
fn scroll_camera(
    keys: Option<Res<ButtonInput<KeyCode>>>,
    time: Option<Res<Time<Real>>>,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
    mut view: ResMut<CampaignCamera>,
) {
    let (Some(keys), Some(time)) = (keys, time) else {
        return;
    };
    let axis = |plus, minus| f32::from(keys.pressed(plus)) - f32::from(keys.pressed(minus));
    let (forward, strafe) = (
        axis(KeyCode::ArrowUp, KeyCode::ArrowDown),
        axis(KeyCode::ArrowRight, KeyCode::ArrowLeft),
    );
    if !view.active() || view.menu_open() || (forward == 0.0 && strafe == 0.0) {
        return;
    }
    let rotation = session.global("CAMERA_FIELD_ROTATION");
    let Some(host) = session.host() else {
        return;
    };
    let objects = host.objects_mut();
    if objects
        .presentation()
        .is_some_and(|p| p.control_enabled == Some(false))
    {
        return;
    }
    let (Some([x, y]), Some([forward_rate, strafe_rate])) =
        (objects.camera_target(), objects.camera_scroll_rates())
    else {
        return;
    };
    let angle = match rotation {
        Some(Value::Handle(field)) => objects.camera_field(field).ok().flatten(),
        _ => None,
    };
    let Some(angle) = angle else {
        return;
    };
    let (sin, cos) = angle.to_radians().sin_cos();
    let dt = time.delta_secs();
    let target = [
        x + (cos * forward * forward_rate + sin * strafe * strafe_rate) * dt,
        y + (sin * forward * forward_rate - cos * strafe * strafe_rate) * dt,
    ];
    if let Err(error) = scroll_to(objects, target) {
        view.last_error = Some(error);
    }
}

/// Player camera input shares the executed bounds and cancels the same live pan.
pub fn scroll_to(
    objects: &mut pcx_play::playercolor_host::ObjectHost,
    mut target: [f32; 2],
) -> Result<(), String> {
    if !target.iter().all(|v| v.is_finite()) {
        return Err("camera input target must be finite".into());
    }
    if let Some(bounds) = objects.camera_state().and_then(|c| c.bounds) {
        let corners = bounds.as_chunks::<2>().0;
        for axis in 0..2 {
            let low = corners
                .iter()
                .map(|c| c[axis])
                .fold(f32::INFINITY, f32::min);
            let high = corners
                .iter()
                .map(|c| c[axis])
                .fold(f32::NEG_INFINITY, f32::max);
            target[axis] = target[axis].clamp(low, high);
        }
    }
    objects.scroll_camera(target).map_err(|e| e.to_string())
}

fn clear(commands: &mut Commands, camera: &mut CampaignCamera) {
    if let Some(entity) = camera.entity.take() {
        commands.entity(entity).despawn();
    }
    camera.attempted = None;
    camera.near = None;
    camera.menu_key = None;
    camera.menu_key_receipt = None;
    camera.clip_receipt = None;
    camera.last_error = None;
    camera.visible = true;
    camera.menu_open = false;
    camera.ready = false;
}

// Application read budget, independent of the camera configuration budget.
const MAX_MENU_STRINGS_BYTES: u64 = 1 << 20;
const GLOBAL_STRINGS: &str = "UI\\FrameDef\\GlobalStrings.fdf";

fn menu_key(text: &str) -> Result<KeyCode, String> {
    authored_function_key(text, "KEY_MENU")
}

/// Reads a source-qualified function-key label from the supplied global strings.
pub fn authored_function_key(text: &str, label_key: &str) -> Result<KeyCode, String> {
    let value = crate::global_strings::global_string(text, label_key)
        .ok_or_else(|| format!("{GLOBAL_STRINGS}: missing {label_key}"))?;
    let key =
        if let Some((_, highlighted)) = value.split_once("|C").or_else(|| value.split_once("|c")) {
            let (_, text) = highlighted
                .split_at_checked(8)
                .filter(|(color, _)| color.bytes().all(|byte| byte.is_ascii_hexdigit()))
                .ok_or_else(|| format!("{GLOBAL_STRINGS}: malformed {label_key} color"))?;
            text.split_once("|R")
                .or_else(|| text.split_once("|r"))
                .map(|(key, _)| key)
                .ok_or_else(|| format!("{GLOBAL_STRINGS}: unterminated {label_key} highlight"))?
        } else {
            value.as_str()
        };
    // Function-key names are a keyboard format, not selected asset values.
    let keys = [
        KeyCode::F1,
        KeyCode::F2,
        KeyCode::F3,
        KeyCode::F4,
        KeyCode::F5,
        KeyCode::F6,
        KeyCode::F7,
        KeyCode::F8,
        KeyCode::F9,
        KeyCode::F10,
        KeyCode::F11,
        KeyCode::F12,
        KeyCode::F13,
        KeyCode::F14,
        KeyCode::F15,
        KeyCode::F16,
        KeyCode::F17,
        KeyCode::F18,
        KeyCode::F19,
        KeyCode::F20,
        KeyCode::F21,
        KeyCode::F22,
        KeyCode::F23,
        KeyCode::F24,
    ];
    key.strip_prefix('F')
        .and_then(|number| number.parse::<usize>().ok())
        .and_then(|number| number.checked_sub(1))
        .and_then(|index| keys.get(index).copied())
        .ok_or_else(|| format!("{GLOBAL_STRINGS}: unsupported {label_key} {value:?}"))
}

fn menu_keys(keys: Option<ResMut<ButtonInput<KeyCode>>>, mut view: ResMut<CampaignCamera>) {
    if view.entity.is_some()
        && let Some(mut keys) = keys
        && let Some(Ok(key)) = view.menu_key
        && keys.just_pressed(key)
    {
        keys.clear_just_pressed(key);
        let open = !view.menu_open();
        view.set_menu_open(open);
    }
}

fn pause_view(
    view: Res<CampaignCamera>,
    runtime: Option<ResMut<crate::campaign_runtime::CampaignRuntime>>,
    mut cameras: Query<&mut Camera, With<CampaignSceneCamera>>,
) {
    if let Some(mut runtime) = runtime {
        runtime.view_paused = view.entity.is_some() && (!view.visible || view.menu_open());
    }
    for mut camera in &mut cameras {
        camera.is_active = view.active();
    }
}

fn apply_camera(
    mut commands: Commands,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
    ground: Res<GroundPublication>,
    requests: Res<MapRequests>,
    sources: Res<SelectedMapSources>,
    mut camera: ResMut<CampaignCamera>,
) {
    let witness = session.session_witness();
    camera.ready = false;
    if !match (&camera.session, &witness) {
        (Some(old), Some(new)) => Weak::ptr_eq(old, new),
        (None, None) => true,
        _ => false,
    } {
        clear(&mut commands, &mut camera);
        camera.session = witness;
    }
    let Some(context) = session.context().cloned() else {
        return;
    };
    // Never view a new VM over terrain from a different admission.
    if !ground
        .context
        .as_ref()
        .is_some_and(|ground| Arc::ptr_eq(ground, &context))
    {
        return;
    }
    camera.ready = true;
    let has_target = session.host().is_some_and(|host| {
        let objects = host.objects();
        objects.camera_target().is_some() || objects.applied_camera_setup().is_some()
    });
    if !has_target {
        return;
    }
    if camera.near.is_none() {
        if camera.attempted.is_some() {
            return;
        }
        let Some(bindings) = requests.bindings.as_ref() else {
            return;
        };
        if !requests
            .admission
            .shared_context()
            .as_ref()
            .is_some_and(|admitted| Arc::ptr_eq(admitted, &context))
            || !requests
                .published_receipts
                .as_ref()
                .is_some_and(|published| Arc::ptr_eq(published, &bindings.receipts))
        {
            return;
        }
        let Some(reader) = sources.for_receipts(&bindings.receipts) else {
            camera.last_error =
                Some("campaign camera has no source reader for its binding receipts".into());
            return;
        };
        camera.menu_key = Some((|| {
            let asset = reader
                .resolve_bounded(GLOBAL_STRINGS, MAX_MENU_STRINGS_BYTES)
                .map_err(|error| format!("{GLOBAL_STRINGS}: {error}"))?;
            camera.menu_key_receipt = Some(receipt(&asset));
            let text = std::str::from_utf8(&asset.bytes)
                .map_err(|error| format!("{GLOBAL_STRINGS}: {error}"))?;
            menu_key(text)
        })());
        camera.attempted = Some(0);
        match reader.resolve_bounded(MISC_DATA, camera.read_limit as u64) {
            Ok(asset) => match pcx_objects::editor::camera::near_clip(
                &asset.bytes,
                MISC_DATA,
                camera.zoom,
                camera.read_limit,
            ) {
                Ok(near) => {
                    camera.near = Some(near);
                    camera.clip_receipt = Some(receipt(&asset));
                }
                Err(error) => {
                    camera.last_error = Some(error);
                    return;
                }
            },
            Err(error) => {
                camera.last_error = Some(format!("{MISC_DATA}: {error}"));
                return;
            }
        }
    }
    let result = executed_pose(
        &mut session,
        &context.terrain,
        camera.near.expect("source clip"),
    );
    match result {
        Ok((transform, projection)) => {
            if let Some(entity) = camera.entity {
                commands
                    .entity(entity)
                    .insert((transform, Projection::Perspective(projection)));
            } else {
                camera.entity = Some(
                    commands
                        .spawn((
                            CampaignSceneCamera,
                            RenderLayers::layer(RENDER_LAYER),
                            Camera3d::default(),
                            transform,
                            Projection::Perspective(projection),
                        ))
                        .id(),
                );
            }
            camera.last_error = None;
        }
        Err(error) => {
            camera.last_error = Some(format!(
                "{}: live camera: {error}",
                context.sources.map.source
            ))
        }
    }
}

fn executed_pose(
    session: &mut MapScriptSession<MapStartupHost>,
    terrain: &pcx_map::Terrain,
    near: f32,
) -> Result<(Transform, PerspectiveProjection), String> {
    // Read actual initialized globals, never copy the conversion's numeric enumerands.
    let handles = FIELDS.map(|name| session.global(name));
    let objects = session
        .host()
        .ok_or("campaign session is absent")?
        .objects();
    let mut values = [0.0; 7];
    for (index, handle) in handles.into_iter().enumerate() {
        let Some(Value::Handle(field)) = handle else {
            return Err(format!(
                "{} is not an initialized field handle",
                FIELDS[index]
            ));
        };
        // The live camera wins; without it the last instant application stands.
        values[index] = match objects.camera_field(field).map_err(|e| e.to_string())? {
            Some(value) => value,
            None if FIELDS[index] == ROLL && objects.applied_camera_field(field).is_err() => {
                // No setup or game camera sets roll: the view has no roll rotation.
                0.0
            }
            None => objects
                .applied_camera_field(field)
                .map_err(|error| error.to_string())?,
        };
    }
    let [x, y] = match objects.camera_target() {
        Some(target) => target,
        None => {
            let view = objects
                .applied_camera_setup()
                .ok_or("camera target is absent")?;
            [view.destination.x, view.destination.y]
        }
    };
    let z = pcx_tera::world::sample_world_z(terrain, x, y)
        .ok_or("camera target lies outside the terrain surface")?;
    pose(values, [x, y, z], near)
}

fn pose(
    values: [f32; 7],
    target: [f32; 3],
    near: f32,
) -> Result<(Transform, PerspectiveProjection), String> {
    let [distance, far, attack, fov, roll, rotation, offset] = values;
    if values
        .iter()
        .chain(target.iter())
        .chain([&near])
        .any(|value| !value.is_finite())
        || distance <= 0.0
        || near <= 0.0
        || far <= near
        || !(0.0..180.0).contains(&fov)
        || fov == 0.0
    {
        return Err("camera has invalid distance, clipping, angle or target values".into());
    }
    let (sin_attack, cos_attack) = attack.to_radians().sin_cos();
    let (sin_rotation, cos_rotation) = rotation.to_radians().sin_cos();
    let target = Vec3::new(target[0], target[2] + offset, -target[1]);
    // Source is Z-up; the ground renderer uses (x,z,-y). Angles are setter degrees.
    let forward = Vec3::new(
        cos_attack * cos_rotation,
        sin_attack,
        -cos_attack * sin_rotation,
    );
    let up = Vec3::new(sin_rotation, 0.0, cos_rotation).cross(forward);
    let eye = target - forward * distance;
    if !target.is_finite() || !eye.is_finite() {
        return Err("camera pose overflows".into());
    }
    let direction = (target - eye)
        .try_normalize()
        .ok_or("camera eye and target collapse at this precision")?;
    if direction.cross(up).try_normalize().is_none() {
        return Err("camera direction is parallel to its up axis".into());
    }
    let mut transform = Transform::from_translation(eye).looking_to(direction, up);
    transform.rotate_local_z(roll.to_radians());
    // shortcut: vertical FOV and bilinear terrain targeting are recreation conventions; qualify native projection/smoothing separately.
    let projection = PerspectiveProjection {
        fov: fov.to_radians(),
        near,
        far,
        ..default()
    };
    if !bevy::camera::CameraProjection::get_clip_from_view(&projection).is_finite() {
        return Err("camera projection overflows".into());
    }
    Ok((transform, projection))
}

#[cfg(test)]
mod tests {
    #[test]
    fn menu_pauses_the_view_clock_without_hiding_the_scene_or_erasing_script_pause() {
        use super::*;
        use bevy::ecs::system::RunSystemOnce;
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>();
        install(&mut app, 0, 1024);
        let scene = app
            .world_mut()
            .spawn((Camera3d::default(), CampaignSceneCamera))
            .id();
        {
            let mut view = app.world_mut().resource_mut::<CampaignCamera>();
            view.entity = Some(scene);
            view.ready = true;
            view.menu_key = Some(Ok(KeyCode::F7));
        }
        let mut runtime = crate::campaign_runtime::CampaignRuntime::new(
            std::time::Duration::from_millis(23),
            8,
            8,
        );
        runtime.paused = true;
        app.insert_resource(runtime);
        for open in [true, false] {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.reset_all();
            keys.press(KeyCode::F7);
            app.world_mut().run_system_once(menu_keys).unwrap();
            app.world_mut().run_system_once(pause_view).unwrap();
            let view = app.world().resource::<CampaignCamera>();
            assert!(view.active());
            assert_eq!(view.menu_open(), open);
            assert!(app.world().get::<Camera>(scene).unwrap().is_active);
            let runtime = app
                .world()
                .resource::<crate::campaign_runtime::CampaignRuntime>();
            assert_eq!(runtime.view_paused, open);
            assert!(runtime.paused);
        }
    }

    #[test]
    fn authored_function_keys_share_mapping_for_localized_labels() {
        use super::*;
        assert_eq!(
            authored_function_key(
                "KEY_QUESTS \"Objectives (|cabc12345F7|r)\",\n",
                "KEY_QUESTS"
            ),
            Ok(KeyCode::F7)
        );
        assert_eq!(
            authored_function_key("KEY_QUESTS \"F12\",\n", "KEY_QUESTS"),
            Ok(KeyCode::F12)
        );
        assert!(authored_function_key("KEY_MENU \"F7\",\n", "KEY_QUESTS").is_err());
    }
    #[test]
    fn menu_key_follows_selected_strings_and_refuses_unknown_names() {
        use super::*;
        assert_eq!(menu_key("KEY_MENU \"F7\",\n"), Ok(KeyCode::F7));
        assert_eq!(menu_key("KEY_MENU \"F12\",\n"), Ok(KeyCode::F12));
        assert_eq!(
            menu_key("KEY_MENU \"Menu (|Cabc12345F9|R)\",\n"),
            Ok(KeyCode::F9)
        );
        for text in [
            "",
            "KEY_MENU \"F0\",",
            "KEY_MENU \"F25\",",
            "KEY_MENU \"unknown\",",
            "KEY_MENU \"Menu (|CbadF9|R)\",",
            "KEY_MENU \"Menu (|Cabc12345F9)\",",
        ] {
            assert!(menu_key(text).is_err());
        }
    }
    use super::*;

    #[test]
    fn source_axes_angles_and_clips_drive_the_pose() {
        let (transform, projection) = pose(
            [80.0, 900.0, 330.0, 65.0, 0.0, 90.0, 7.0],
            [11.0, 23.0, 5.0],
            9.0,
        )
        .unwrap();
        let target = Vec3::new(11.0, 12.0, -23.0);
        assert!((transform.translation.distance(target) - 80.0).abs() < 0.0001);
        assert!(
            (transform.forward().as_vec3() - (target - transform.translation).normalize()).length()
                < 0.0001
        );
        assert!(transform.translation.y > target.y);
        assert!(transform.translation.z > target.z);
        assert_eq!(projection.near, 9.0);
        assert_eq!(projection.far, 900.0);
        assert_eq!(projection.fov, 65f32.to_radians());
        for bad in [0.0, f32::NAN, 900.0] {
            assert!(
                pose(
                    [80.0, 900.0, 330.0, 65.0, 0.0, 90.0, 7.0],
                    [11.0, 23.0, 5.0],
                    bad
                )
                .is_err()
            );
        }
        assert!(
            pose(
                [f32::MIN_POSITIVE, 900.0, 330.0, 65.0, 0.0, 90.0, 7.0],
                [11.0, 23.0, 5.0],
                9.0
            )
            .is_err()
        );
        assert!(
            pose(
                [80.0, 900.0, 330.0, f32::from_bits(1), 0.0, 90.0, 7.0],
                [11.0, 23.0, 5.0],
                9.0
            )
            .is_err()
        );
        for attack in [90.0, 270.0] {
            let (transform, _) = pose(
                [80.0, 900.0, attack, 65.0, 35.0, 90.0, 7.0],
                [11.0, 23.0, 5.0],
                9.0,
            )
            .unwrap();
            assert!(transform.rotation.is_finite());
            assert!(
                (transform.forward().as_vec3() - (target - transform.translation).normalize())
                    .length()
                    < 0.0001
            );
        }
    }
}

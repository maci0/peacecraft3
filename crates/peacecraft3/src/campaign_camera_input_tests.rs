//! Camera input through the real systems on a synthetic published session. Keys, wheel
//! and pointer positions are injected as input messages and window state; there is no
//! host window, pointer or audio.
use std::time::Duration;

use bevy::{
    camera::{CameraProjection, ComputedCameraValues, RenderTargetInfo},
    input::{
        ButtonState, InputPlugin,
        keyboard::{Key, KeyboardInput},
        mouse::{MouseButtonInput, MouseScrollUnit, MouseWheel},
    },
    math::DVec2,
    time::TimeUpdateStrategy,
    window::PrimaryWindow,
};
use pcx_play::{
    camera_input::ScrollOptions,
    player_command_log::{PlayerCommand, StartIdentity},
};

use super::*;
use crate::{campaign_input::CampaignInput, campaign_startup_cases::camera_input_cases};
use peacecraft3::map_ground::GroundPublication;

const FRAME: Duration = Duration::from_millis(50);
/// Game clock at the start of the synthetic session.
const START: f64 = 10.0;
const SCREEN: [f32; 2] = [1024.0, 768.0];
const CENTRE: [f32; 2] = [64.0, 64.0];
const HEIGHT: f32 = 100.0;
const SEED: i32 = 7;
const DISTANCE: &str = "CAMERA_FIELD_TARGET_DISTANCE";
const ANGLE: &str = "CAMERA_FIELD_ANGLE_OF_ATTACK";
const ROTATION: &str = "CAMERA_FIELD_ROTATION";

struct Rig {
    app: App,
    window: Entity,
    ground: f32,
}

impl Rig {
    fn new(script: &str, rules: Option<CameraInputRules>) -> Self {
        let script = format!(
            "function config takes nothing returns nothing\nendfunction\nfunction main takes nothing returns nothing\n{script}endfunction\n"
        );
        let session = camera_input_cases::session(&script);
        let context = session.context().cloned().unwrap();
        let ground = pcx_tera::world::sample_world_z(&context.terrain, CENTRE[0], CENTRE[1])
            .expect("scene centre is on the synthetic terrain");
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, InputPlugin))
            .insert_resource(TimeUpdateStrategy::ManualDuration(FRAME))
            .insert_resource(session)
            .init_resource::<crate::campaign_hud::CampaignHud>()
            .init_resource::<crate::campaign_inventory::CampaignInventory>()
            .init_resource::<crate::campaign_script_dialogs::CampaignScriptDialogs>()
            .init_resource::<crate::campaign_actors::UnitActors>()
            .init_resource::<GroundPublication>()
            .init_resource::<CampaignInput>()
            .init_resource::<peacecraft3::game_input::GameInput>()
            .insert_resource(rules.map_or_else(CameraInput::default, CameraInput::with_rules))
            .add_systems(
                Update,
                (
                    crate::campaign_game_input::refresh_gate,
                    session_state,
                    camera_keys,
                    scroll,
                )
                    .chain(),
            );
        app.world_mut().resource_mut::<GroundPublication>().context = Some(context);
        let window = app
            .world_mut()
            .spawn((
                Window {
                    resolution: [SCREEN[0] as u32, SCREEN[1] as u32].into(),
                    ..default()
                },
                PrimaryWindow,
            ))
            .id();
        let projection = PerspectiveProjection {
            fov: std::f32::consts::FRAC_PI_4,
            aspect_ratio: SCREEN[0] / SCREEN[1],
            near: 10.0,
            far: 1000.0,
            ..default()
        };
        let camera = app
            .world_mut()
            .spawn((
                Camera {
                    computed: ComputedCameraValues {
                        clip_from_view: projection.get_clip_from_view(),
                        target_info: Some(RenderTargetInfo {
                            physical_size: UVec2::new(SCREEN[0] as u32, SCREEN[1] as u32),
                            scale_factor: 1.0,
                        }),
                        ..default()
                    },
                    ..default()
                },
                Projection::Perspective(projection),
                // Straight down at the scene centre, game +y up the screen.
                GlobalTransform::from(
                    Transform::from_xyz(CENTRE[0], ground + HEIGHT, -CENTRE[1])
                        .looking_at(Vec3::new(CENTRE[0], ground, -CENTRE[1]), Vec3::NEG_Z),
                ),
                peacecraft3::campaign_camera::CampaignSceneCamera,
            ))
            .id();
        app.insert_resource(CampaignCamera::active_for_test(camera));
        let mut rig = Self {
            app,
            window,
            ground,
        };
        // The first frame initialises time and input state.
        rig.frame();
        rig
    }

    fn frame(&mut self) {
        self.app.update();
    }

    /// Whole frames covering at least `seconds` of real time.
    fn wait(&mut self, seconds: f32) {
        for _ in 0..(seconds / FRAME.as_secs_f32()).ceil() as usize {
            self.frame();
        }
    }

    fn key(&mut self, key_code: KeyCode, state: ButtonState, repeat: bool) {
        let window = self.window;
        self.app.world_mut().write_message(KeyboardInput {
            key_code,
            logical_key: Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified),
            state,
            text: None,
            repeat,
            window,
        });
    }

    /// A press frame, then a release frame.
    fn tap(&mut self, key_code: KeyCode) {
        self.key(key_code, ButtonState::Pressed, false);
        self.frame();
        self.key(key_code, ButtonState::Released, false);
        self.frame();
    }

    fn wheel(&mut self, unit: MouseScrollUnit, y: f32) {
        let window = self.window;
        self.app.world_mut().write_message(MouseWheel {
            unit,
            x: 0.0,
            y,
            window,
            phase: bevy::input::touch::TouchPhase::Moved,
        });
        self.frame();
    }

    fn hold(&mut self, key: KeyCode, held: bool) {
        let mut keys = self.app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        if held {
            keys.press(key);
        } else {
            keys.release(key);
        }
    }

    fn cursor(&mut self, at: Option<[f32; 2]>) {
        self.app
            .world_mut()
            .get_mut::<Window>(self.window)
            .unwrap()
            .set_physical_cursor_position(at.map(|p| DVec2::new(f64::from(p[0]), f64::from(p[1]))));
    }

    fn button(&mut self, button: MouseButton, state: ButtonState) {
        let window = self.window;
        self.app.world_mut().write_message(MouseButtonInput {
            button,
            state,
            window,
        });
    }

    fn objects<T>(&mut self, read: impl FnOnce(&mut ObjectHost) -> T) -> T {
        let mut session = self
            .app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        read(session.host().unwrap().objects_mut())
    }

    /// Moves the game clock to `seconds` after the session start.
    fn game_clock(&mut self, seconds: f64) {
        self.app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .advance_timers(START + seconds, 8)
            .unwrap();
    }

    fn field(&mut self, name: &str) -> f32 {
        let mut session = self
            .app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let Some(Value::Handle(field)) = session.global(name) else {
            panic!("{name} is not a camera field")
        };
        session
            .host()
            .unwrap()
            .objects()
            .camera_field(field)
            .unwrap()
            .unwrap()
    }

    /// Distance, angle of attack and rotation now.
    fn pose(&mut self) -> [f32; 3] {
        [DISTANCE, ANGLE, ROTATION].map(|name| self.field(name))
    }

    /// The pose `seconds` after the session start.
    fn pose_at(&mut self, seconds: f64) -> [f32; 3] {
        self.game_clock(seconds);
        self.pose()
    }

    fn target(&mut self) -> [f32; 2] {
        self.objects(|o| o.camera_target().unwrap())
    }

    fn set_target(&mut self, at: [f32; 2]) {
        self.objects(|o| o.scroll_camera(at).unwrap());
    }

    fn open_menu(&mut self, open: bool) {
        self.app
            .world_mut()
            .resource_mut::<CampaignCamera>()
            .set_menu_open(open);
    }

    fn error(&self) -> Option<String> {
        self.app
            .world()
            .resource::<CameraInput>()
            .last_error
            .clone()
    }

    fn alert(&mut self, at: [f32; 2]) {
        self.app
            .world_mut()
            .resource_mut::<CameraInput>()
            .alerts
            .push(at);
    }

    fn selection(&mut self) -> Vec<HandleId> {
        self.objects(|o| o.selection().to_vec())
    }

    /// Game point on the scene ground under a screen position.
    fn ground_under(&mut self, screen: [f32; 2]) -> [f32; 2] {
        let ground = self.ground;
        let mut cameras = self
            .app
            .world_mut()
            .query_filtered::<(&Camera, &GlobalTransform), With<peacecraft3::campaign_camera::CampaignSceneCamera>>();
        let (camera, transform) = cameras.single(self.app.world()).unwrap();
        let ray = camera
            .viewport_to_world(transform, Vec2::from(screen))
            .unwrap();
        let point = ray.get_point((ground - ray.origin.y) / ray.direction.y);
        [point.x, -point.z]
    }
}

fn rules() -> CameraInputRules {
    CameraInputRules::fixture()
}

/// Float-tolerant comparison; `f32` trigonometry leaves rotation residue below 1e-3.
fn near<const N: usize>(actual: [f32; N], expected: [f32; N]) {
    for (a, e) in actual.into_iter().zip(expected) {
        assert!((a - e).abs() < 1e-3, "{actual:?} != {expected:?}");
    }
}

#[test]
fn wheel_and_page_keys_ease_to_the_table_pose_over_one_second() {
    let mut rig = Rig::new("", Some(rules()));
    near(rig.pose(), [1650.0, 304.0, 90.0]);
    rig.wheel(MouseScrollUnit::Line, 1.0);
    near(rig.pose_at(0.5), [1625.0, 307.5, 90.0]);
    near(rig.pose_at(1.0), [1600.0, 311.0, 90.0]);
    near(rig.pose_at(3.0), [1600.0, 311.0, 90.0]);
    // Key repeat zooms again, as the client's handler does not filter it.
    rig.key(KeyCode::PageUp, ButtonState::Pressed, false);
    rig.key(KeyCode::PageUp, ButtonState::Pressed, true);
    rig.frame();
    near(rig.pose_at(5.0), [1400.0, 325.0, 90.0]);
    // The last levels settle at the executable's literals.
    for _ in 0..5 {
        rig.tap(KeyCode::PageUp);
    }
    near(rig.pose_at(7.0), [1200.0, 335.0, 90.0]);
    rig.tap(KeyCode::PageDown);
    near(rig.pose_at(9.0), [1275.0, 332.0, 90.0]);
    // A wheel step away from the user zooms out, one level however large the delta.
    rig.wheel(MouseScrollUnit::Line, -3.0);
    near(rig.pose_at(11.0), [1400.0, 325.0, 90.0]);
}

#[test]
fn zoom_stops_at_both_ends_and_fine_wheel_steps_add_up() {
    let mut rig = Rig::new("", Some(rules()));
    rig.tap(KeyCode::PageDown);
    rig.wheel(MouseScrollUnit::Line, -1.0);
    near(rig.pose_at(2.0), [1650.0, 304.0, 90.0]);
    assert_eq!(rig.error(), None);
    // Two half notches make one level; pixel deltas count as wheel units.
    rig.wheel(MouseScrollUnit::Line, 0.5);
    near(rig.pose_at(4.0), [1650.0, 304.0, 90.0]);
    rig.wheel(MouseScrollUnit::Line, 0.5);
    near(rig.pose_at(6.0), [1600.0, 311.0, 90.0]);
    rig.wheel(MouseScrollUnit::Pixel, 120.0);
    near(rig.pose_at(8.0), [1500.0, 318.0, 90.0]);
    // A reversal drops the half notch already collected.
    rig.wheel(MouseScrollUnit::Line, 0.5);
    rig.wheel(MouseScrollUnit::Line, -0.5);
    near(rig.pose_at(10.0), [1500.0, 318.0, 90.0]);
    for _ in 0..8 {
        rig.tap(KeyCode::PageUp);
    }
    near(rig.pose_at(12.0), [1200.0, 335.0, 90.0]);
}

#[test]
fn insert_and_delete_swing_the_rotation_while_held() {
    let mut rig = Rig::new("", Some(rules()));
    rig.key(KeyCode::Insert, ButtonState::Pressed, false);
    rig.frame();
    near(rig.pose_at(0.25), [1650.0, 304.0, 55.0]);
    near(rig.pose_at(1.0), [1650.0, 304.0, 20.0]);
    // Repeats and the other key change nothing while Insert is held.
    rig.key(KeyCode::Insert, ButtonState::Pressed, true);
    rig.key(KeyCode::Delete, ButtonState::Pressed, false);
    rig.frame();
    near(rig.pose_at(3.0), [1650.0, 304.0, 20.0]);
    // Zooming keeps the swing: the Insert pose of the next level.
    rig.tap(KeyCode::PageUp);
    near(rig.pose_at(5.0), [1600.0, 311.0, 20.0]);
    rig.key(KeyCode::Insert, ButtonState::Released, false);
    rig.frame();
    near(rig.pose_at(5.25), [1600.0, 311.0, 55.0]);
    near(rig.pose_at(7.0), [1600.0, 311.0, 90.0]);
    rig.key(KeyCode::Delete, ButtonState::Pressed, false);
    rig.frame();
    near(rig.pose_at(9.0), [1600.0, 311.0, 160.0]);
    // Numpad Decimal releases the Delete state, Numpad 0 would release Insert.
    rig.key(KeyCode::Numpad0, ButtonState::Released, false);
    rig.frame();
    near(rig.pose_at(11.0), [1600.0, 311.0, 160.0]);
    rig.key(KeyCode::NumpadDecimal, ButtonState::Released, false);
    rig.frame();
    near(rig.pose_at(13.0), [1600.0, 311.0, 90.0]);
}

#[test]
fn a_menu_blocks_presses_but_never_strands_a_held_rotation() {
    let mut rig = Rig::new("", Some(rules()));
    rig.key(KeyCode::Insert, ButtonState::Pressed, false);
    rig.frame();
    rig.open_menu(true);
    rig.wheel(MouseScrollUnit::Line, 1.0);
    rig.key(KeyCode::PageUp, ButtonState::Pressed, false);
    rig.key(KeyCode::Delete, ButtonState::Pressed, false);
    rig.frame();
    near(rig.pose_at(2.0), [1650.0, 304.0, 20.0]);
    rig.key(KeyCode::Insert, ButtonState::Released, false);
    rig.frame();
    near(rig.pose_at(4.0), [1650.0, 304.0, 90.0]);
    // The blocked wheel step left no residue for later.
    rig.open_menu(false);
    rig.wheel(MouseScrollUnit::Line, 0.5);
    near(rig.pose_at(6.0), [1650.0, 304.0, 90.0]);
}

#[test]
fn without_the_client_rules_zoom_and_edges_stay_off_but_arrows_scroll() {
    let mut rig = Rig::new("", None);
    rig.wheel(MouseScrollUnit::Line, 1.0);
    rig.tap(KeyCode::PageUp);
    rig.tap(KeyCode::Insert);
    rig.tap(KeyCode::Backspace);
    near(rig.pose_at(3.0), [1650.0, 304.0, 90.0]);
    rig.cursor(Some([0.0, 384.0]));
    rig.frame();
    assert_eq!(rig.target(), CENTRE);
    rig.hold(KeyCode::ArrowUp, true);
    rig.frame();
    // 3000 units per second at the default multiplier for one 50 ms frame.
    near(rig.target(), [64.0, 214.0]);
}

#[test]
fn arrow_keys_scroll_along_the_rotation_at_the_key_multiplier() {
    let mut rules = rules();
    rules.fallback = ScrollOptions {
        mouse: 0,
        key: 100,
        mouse_disable: 0,
    };
    let mut rig = Rig::new("", Some(rules));
    rig.hold(KeyCode::ArrowUp, true);
    rig.frame();
    // Rotation 90 faces +y; slider 100 is a 1.5 multiplier: 3000 * 1.5 * 0.05.
    let [x, y] = rig.target();
    assert!(
        (x - 64.0).abs() < 1e-3 && (y - 289.0).abs() < 1e-3,
        "{x} {y}"
    );
    rig.hold(KeyCode::ArrowDown, true);
    let before = rig.target();
    rig.frame();
    assert_eq!(rig.target(), before, "opposite arrows cancel");
    rig.hold(KeyCode::ArrowUp, false);
    rig.hold(KeyCode::ArrowDown, false);
    rig.hold(KeyCode::ArrowRight, true);
    rig.frame();
    let [x2, y2] = rig.target();
    assert!((x2 - before[0] - 225.0).abs() < 1e-2 && (y2 - before[1]).abs() < 1e-2);
}

#[test]
fn edge_scroll_follows_the_margin_corners_and_the_mouse_options() {
    let mut rules = rules();
    rules.fallback.mouse = 0;
    let mut rig = Rig::new("", Some(rules.clone()));
    // Strictly inside 0.004 of the 0.8 frame: 5.12 px at 1024 wide.
    rig.cursor(Some([5.0, 384.0]));
    rig.frame();
    // Mouse slider 0 is a 0.5 multiplier: 3000 * 0.5 * 0.05 to the left.
    near(rig.target(), [64.0 - 75.0, 64.0]);
    rig.set_target(CENTRE);
    rig.cursor(Some([5.5, 384.0]));
    rig.frame();
    assert_eq!(rig.target(), CENTRE);
    // Bottom-right corner scrolls both axes: right and down.
    rig.cursor(Some([1023.0, 767.0]));
    rig.frame();
    near(rig.target(), [64.0 + 75.0, 64.0 - 75.0]);
    // An unfocused window, a missing pointer and the disable flag all stop it.
    rig.set_target(CENTRE);
    rig.app
        .world_mut()
        .get_mut::<Window>(rig.window)
        .unwrap()
        .focused = false;
    rig.frame();
    assert_eq!(rig.target(), CENTRE);
    rig.app
        .world_mut()
        .get_mut::<Window>(rig.window)
        .unwrap()
        .focused = true;
    rig.cursor(None);
    rig.frame();
    assert_eq!(rig.target(), CENTRE);
    let mut disabled = rules;
    disabled.fallback.mouse_disable = 1;
    let mut rig = Rig::new("", Some(disabled));
    rig.cursor(Some([0.0, 0.0]));
    rig.frame();
    assert_eq!(rig.target(), CENTRE);
}

#[test]
fn a_held_arrow_wins_over_the_pointer_in_the_same_direction() {
    let mut rules = rules();
    rules.fallback = ScrollOptions {
        mouse: 0,
        key: 100,
        mouse_disable: 0,
    };
    let mut rig = Rig::new("", Some(rules));
    rig.cursor(Some([0.0, 384.0]));
    rig.hold(KeyCode::ArrowLeft, true);
    rig.frame();
    near(rig.target(), [64.0 - 225.0, 64.0]);
}

#[test]
fn a_middle_drag_keeps_the_grabbed_ground_under_the_pointer() {
    let mut rig = Rig::new("", Some(rules()));
    let start = [SCREEN[0] / 2.0, SCREEN[1] / 2.0];
    let held = rig.ground_under(start);
    rig.cursor(Some(start));
    rig.button(MouseButton::Middle, ButtonState::Pressed);
    rig.frame();
    assert_eq!(
        rig.target(),
        CENTRE,
        "a press alone does not move the camera"
    );
    let moved = [start[0] + 100.0, start[1] - 60.0];
    let under = rig.ground_under(moved);
    rig.cursor(Some(moved));
    rig.frame();
    let [x, y] = rig.target();
    assert!(
        (x - (CENTRE[0] + held[0] - under[0])).abs() < 0.05
            && (y - (CENTRE[1] + held[1] - under[1])).abs() < 0.05,
        "{x} {y}"
    );
    assert!(x < CENTRE[0] && y < CENTRE[1]);
    // Releasing ends the drag.
    rig.button(MouseButton::Middle, ButtonState::Released);
    rig.frame();
    let at_release = rig.target();
    rig.cursor(Some(start));
    rig.frame();
    assert_eq!(rig.target(), at_release);
}

#[test]
fn a_middle_press_over_a_blocked_pointer_or_closed_input_grabs_nothing() {
    let mut rig = Rig::new("", Some(rules()));
    let start = [SCREEN[0] / 2.0, SCREEN[1] / 2.0];
    rig.cursor(Some(start));
    rig.app
        .world_mut()
        .resource_mut::<crate::campaign_hud::CampaignHud>()
        .pointer_consumed = true;
    rig.button(MouseButton::Middle, ButtonState::Pressed);
    rig.frame();
    rig.cursor(Some([start[0] + 80.0, start[1]]));
    rig.frame();
    assert_eq!(rig.target(), CENTRE);
    rig.button(MouseButton::Middle, ButtonState::Released);
    rig.frame();
    rig.app
        .world_mut()
        .resource_mut::<crate::campaign_hud::CampaignHud>()
        .pointer_consumed = false;
    rig.open_menu(true);
    rig.button(MouseButton::Middle, ButtonState::Pressed);
    rig.frame();
    rig.open_menu(false);
    rig.cursor(Some([start[0] + 80.0, start[1]]));
    rig.frame();
    assert_eq!(rig.target(), CENTRE);
}

#[test]
fn space_steps_back_through_the_alerts_inside_the_window() {
    let mut rig = Rig::new("", Some(rules()));
    rig.tap(KeyCode::Space);
    assert_eq!(rig.target(), CENTRE, "an empty history does nothing");
    for at in [[10.0, 12.0], [20.0, 22.0], [30.0, 32.0]] {
        rig.alert(at);
    }
    rig.tap(KeyCode::Space);
    assert_eq!(rig.target(), [30.0, 32.0]);
    rig.tap(KeyCode::Space);
    assert_eq!(rig.target(), [20.0, 22.0]);
    rig.tap(KeyCode::Space);
    assert_eq!(rig.target(), [10.0, 12.0]);
    rig.tap(KeyCode::Space);
    assert_eq!(rig.target(), [30.0, 32.0], "the history wraps");
    // After a full second without a press the newest alert comes first again.
    rig.tap(KeyCode::Space);
    assert_eq!(rig.target(), [20.0, 22.0]);
    rig.wait(1.0);
    rig.tap(KeyCode::Space);
    assert_eq!(rig.target(), [30.0, 32.0]);
}

#[test]
fn backspace_cycles_the_own_town_halls_and_restarts_after_the_window() {
    let mut rig = Rig::new(
        "call CreateUnit(Player(0),'peon',16.0,16.0,0.0)\ncall CreateUnit(Player(0),'hall',40.0,20.0,0.0)\ncall CreateUnit(Player(1),'hall',90.0,90.0,0.0)\ncall CreateUnit(Player(0),'hall',70.0,30.0,0.0)\n",
        Some(rules()),
    );
    rig.tap(KeyCode::Backspace);
    assert_eq!(rig.target(), [40.0, 20.0]);
    rig.tap(KeyCode::Backspace);
    assert_eq!(rig.target(), [70.0, 30.0]);
    rig.tap(KeyCode::Backspace);
    assert_eq!(rig.target(), [40.0, 20.0], "wraps inside the window");
    rig.tap(KeyCode::Backspace);
    rig.wait(1.0);
    rig.tap(KeyCode::Backspace);
    assert_eq!(rig.target(), [40.0, 20.0], "restarts at the first hall");
    // Held-key repeats advance like presses.
    rig.key(KeyCode::Backspace, ButtonState::Pressed, true);
    rig.frame();
    assert_eq!(rig.target(), [70.0, 30.0]);
}

#[test]
fn backspace_without_an_own_hall_does_nothing() {
    let mut rig = Rig::new(
        "call CreateUnit(Player(0),'peon',16.0,16.0,0.0)\ncall CreateUnit(Player(1),'hall',90.0,90.0,0.0)\n",
        Some(rules()),
    );
    rig.tap(KeyCode::Backspace);
    assert_eq!(rig.target(), CENTRE);
    assert_eq!(rig.error(), None);
    // A closed input also keeps Space and Backspace quiet.
    rig.open_menu(true);
    rig.alert([5.0, 5.0]);
    rig.tap(KeyCode::Space);
    assert_eq!(rig.target(), CENTRE);
}

#[test]
fn idle_worker_keys_select_centre_and_cycle() {
    let mut rig = Rig::new(
        "call CreateUnit(Player(0),'peon',16.0,16.0,0.0)\ncall CreateUnit(Player(0),'peon',44.0,36.0,0.0)\ncall CreateUnit(Player(0),'peon',60.0,50.0,0.0)\ncall CreateUnit(Player(0),'hall',70.0,30.0,0.0)\n",
        Some(rules()),
    );
    let units = rig.objects(|o| o.executed_units().map(|(id, _)| id).collect::<Vec<_>>());
    let idle = [units[0], units[1], units[2]];
    let at = [[16.0, 16.0], [44.0, 36.0], [60.0, 50.0]];
    let mut cycle = Cycle::default();
    let mut journal = CampaignInput::default();
    let context = rig
        .app
        .world()
        .resource::<MapScriptSession<MapStartupHost>>()
        .context()
        .cloned()
        .unwrap();
    journal
        .commands
        .begin(StartIdentity::from_context(SEED, &context));
    let mut jump = |rig: &mut Rig, now: f64, list: &[HandleId]| {
        let mut error = None;
        rig.objects(|o| jump_to_idle(o, &mut cycle, &mut journal, 1.0, now, list, &mut error));
        assert_eq!(error, None);
    };
    // (time, expected index): continuing inside the window, wrapping, then restarting.
    for (now, index) in [(5.0, 0), (5.5, 1), (6.0, 2), (6.5, 0), (8.0, 0), (8.5, 1)] {
        jump(&mut rig, now, &idle);
        assert_eq!(rig.selection(), [units[index]], "at {now}");
        near(rig.target(), at[index]);
    }
    // With nobody idle the selection and the camera stay.
    rig.set_target(CENTRE);
    jump(&mut rig, 9.0, &[]);
    assert_eq!((rig.selection(), rig.target()), (vec![units[1]], CENTRE));
    let selects = journal
        .commands
        .entries()
        .iter()
        .filter(|e| matches!(e.command, PlayerCommand::Select { replace: true, .. }))
        .count();
    assert_eq!((selects, journal.last_error), (6, None));
}

#[test]
fn the_idle_keys_report_an_unavailable_worker_list_instead_of_acting() {
    // The synthetic session has no unit activity, so the client's idle test is unavailable.
    let mut rig = Rig::new(
        "call CreateUnit(Player(0),'peon',16.0,16.0,0.0)\n",
        Some(rules()),
    );
    for key in [KeyCode::F8, KeyCode::Backquote] {
        rig.tap(key);
        assert!(rig.error().is_some_and(|e| e.starts_with("idle workers")));
        assert_eq!(rig.target(), CENTRE);
        rig.app.world_mut().resource_mut::<CameraInput>().last_error = None;
    }
    // A repeat of the held key is not a second press.
    rig.key(KeyCode::F8, ButtonState::Pressed, true);
    rig.frame();
    assert_eq!(rig.error(), None);
}

#[test]
fn a_new_session_returns_to_the_configured_zoom_level() {
    let mut rig = Rig::new("", Some(rules()));
    rig.tap(KeyCode::PageUp);
    rig.tap(KeyCode::PageUp);
    assert_eq!(rig.app.world().resource::<CameraInput>().view.level, 2);
    let fresh = camera_input_cases::session(
        "function config takes nothing returns nothing\nendfunction\nfunction main takes nothing returns nothing\nendfunction\n",
    );
    rig.app.insert_resource(fresh);
    rig.frame();
    let state = rig.app.world().resource::<CameraInput>();
    assert_eq!(state.view, View::default());
    assert!(
        state.rules.is_some(),
        "the client rules outlive the session"
    );
}

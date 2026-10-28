//! Drag-box selection through the real input systems on a synthetic published
//! session. Pointer events are injected as input messages; there is no host
//! window, pointer or audio.
use std::time::Duration;

use bevy::{
    camera::{CameraProjection, ComputedCameraValues, RenderTargetInfo},
    input::{
        ButtonState, InputPlugin,
        keyboard::{Key, KeyboardInput},
        mouse::MouseButtonInput,
    },
    math::DVec2,
    time::TimeUpdateStrategy,
};
use pcx_objects::editor::selection::{CircleBand, SelectionCircles};
use pcx_play::{player_command_log::StartIdentity, selection_box::Parameters};

use super::*;
use crate::{
    campaign_box_select::{BoxSelect, Outline},
    campaign_startup_cases::box_select_cases::{self, FOOTMAN, HALL, PEON},
};

const FRAME: Duration = Duration::from_millis(50);
const CAP: usize = 12;
/// Visible ground is centred here, at this height above the terrain.
const CENTRE: [f32; 2] = [64.0, 64.0];
const HEIGHT: f32 = 100.0;
const FRIEND: [u8; 4] = [255, 0, 255, 0];
const SEED: i32 = 7;

fn parameters() -> Parameters {
    Parameters {
        origin: pcx_play::scripts::AssetIdentity {
            source: "synthetic".into(),
            edition: "fixture".into(),
            version: "fixture".into(),
        },
        image_sha256: [0; 32],
        selection_cap: CAP,
        double_click_ms: 500,
        double_click_tolerance: 0.01,
    }
}

/// One unit of the synthetic scene: owner player number, type and game position.
pub(crate) type Spec = (u8, &'static str, [f32; 2]);

pub(crate) struct Rig {
    pub(crate) app: App,
    pub(crate) window: Entity,
    pub(crate) units: Vec<HandleId>,
    pub(crate) ground: f32,
}

impl Rig {
    pub(crate) fn new(specs: &[Spec], size: [f32; 2]) -> Self {
        let creates: String = specs
            .iter()
            .map(|(player, kind, [x, y])| {
                format!("call CreateUnit(Player({player}),'{kind}',{x:.1},{y:.1},0.0)\n")
            })
            .collect();
        let script = format!(
            "function config takes nothing returns nothing\nendfunction\nfunction main takes nothing returns nothing\n{creates}endfunction\n"
        );
        let mut session = box_select_cases::session(&script);
        let context = session.context().cloned().unwrap();
        let units: Vec<HandleId> = session
            .host()
            .unwrap()
            .objects()
            .executed_units()
            .map(|(id, _)| id)
            .collect();
        assert_eq!(units.len(), specs.len());
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
            .init_resource::<crate::campaign_command_card::CommandCard>()
            .init_resource::<GroundPublication>()
            .insert_resource(BoxSelect::with_parameters(parameters()))
            .add_systems(
                Update,
                (
                    crate::campaign_game_input::refresh_gate,
                    sync_queue_flag,
                    crate::campaign_box_select::gesture_input,
                    click,
                    crate::campaign_box_select::draw_outline,
                )
                    .chain(),
            );
        peacecraft3::game_input::install(&mut app);
        app.world_mut().resource_mut::<GroundPublication>().context = Some(context.clone());
        let window = app
            .world_mut()
            .spawn((
                Window {
                    resolution: [size[0] as u32, size[1] as u32].into(),
                    ..default()
                },
                PrimaryWindow,
            ))
            .id();
        let projection = PerspectiveProjection {
            fov: std::f32::consts::FRAC_PI_4,
            aspect_ratio: size[0] / size[1],
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
                            physical_size: UVec2::new(size[0] as u32, size[1] as u32),
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
                CampaignSceneCamera,
            ))
            .id();
        let circles = SelectionCircles {
            scale_factor: 72.0,
            bands: vec![CircleBand {
                max_size: None,
                texture: "circle".into(),
                dotted: "dotted".into(),
            }],
            friend: FRIEND,
            neutral: [255, 255, 255, 0],
            enemy: [255, 255, 0, 0],
        };
        let mut input = CampaignInput {
            circles: Some(circles),
            ..default()
        };
        input
            .commands
            .begin(StartIdentity::from_context(SEED, &context));
        app.insert_resource(input)
            .insert_resource(CampaignCamera::active_for_test(camera));
        let mut rig = Self {
            app,
            window,
            units,
            ground,
        };
        // The first frame initialises time and input state.
        rig.frame();
        rig
    }

    pub(crate) fn frame(&mut self) {
        self.app.update();
    }

    pub(crate) fn cursor(&mut self, at: [f32; 2]) {
        self.app
            .world_mut()
            .get_mut::<Window>(self.window)
            .unwrap()
            .set_physical_cursor_position(Some(DVec2::new(f64::from(at[0]), f64::from(at[1]))));
    }

    pub(crate) fn button(&mut self, button: MouseButton, state: ButtonState) {
        let window = self.window;
        self.app.world_mut().write_message(MouseButtonInput {
            button,
            state,
            window,
        });
    }

    pub(crate) fn press(&mut self, at: [f32; 2]) {
        self.cursor(at);
        self.button(MouseButton::Left, ButtonState::Pressed);
        self.frame();
    }

    pub(crate) fn drag(&mut self, to: [f32; 2]) {
        self.cursor(to);
        self.frame();
    }

    pub(crate) fn release(&mut self, at: [f32; 2]) {
        self.cursor(at);
        self.button(MouseButton::Left, ButtonState::Released);
        self.frame();
    }

    pub(crate) fn sweep(&mut self, from: [f32; 2], to: [f32; 2]) {
        self.press(from);
        self.drag([(from[0] + to[0]) / 2.0, (from[1] + to[1]) / 2.0]);
        self.release(to);
    }

    pub(crate) fn click(&mut self, at: [f32; 2]) {
        self.press(at);
        self.release(at);
    }

    pub(crate) fn hold(&mut self, key: KeyCode, held: bool) {
        let mut keys = self.app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        if held {
            keys.press(key);
        } else {
            keys.release(key);
        }
    }

    /// Screen position of a game point on the scene ground, through the scene camera.
    pub(crate) fn screen(&mut self, game: [f32; 2]) -> [f32; 2] {
        let ground = self.ground;
        let mut cameras = self
            .app
            .world_mut()
            .query_filtered::<(&Camera, &GlobalTransform), With<CampaignSceneCamera>>();
        let (camera, transform) = cameras.single(self.app.world()).unwrap();
        camera
            .world_to_viewport(transform, Vec3::new(game[0], ground, -game[1]))
            .unwrap()
            .to_array()
    }

    /// Screen rectangle around two game points, padded by `pad` pixels.
    pub(crate) fn around(&mut self, a: [f32; 2], b: [f32; 2], pad: f32) -> ([f32; 2], [f32; 2]) {
        let (a, b) = (self.screen(a), self.screen(b));
        (
            [a[0].min(b[0]) - pad, a[1].min(b[1]) - pad],
            [a[0].max(b[0]) + pad, a[1].max(b[1]) + pad],
        )
    }

    pub(crate) fn selection(&mut self) -> Vec<HandleId> {
        self.app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .host()
            .unwrap()
            .objects()
            .selection()
            .to_vec()
    }

    pub(crate) fn select(&mut self, units: &[HandleId]) {
        self.app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .host()
            .unwrap()
            .objects_mut()
            .select_units(units, true);
    }

    pub(crate) fn error(&self) -> Option<String> {
        self.app
            .world()
            .resource::<CampaignInput>()
            .last_error
            .clone()
    }

    pub(crate) fn outlines(&mut self) -> Vec<(Node, BorderColor)> {
        self.app
            .world_mut()
            .query_filtered::<(&Node, &BorderColor), With<Outline>>()
            .iter(self.app.world())
            .map(|(node, border)| (node.clone(), *border))
            .collect()
    }

    pub(crate) fn selects_journaled(&self) -> usize {
        self.app
            .world()
            .resource::<CampaignInput>()
            .commands
            .entries()
            .iter()
            .filter(|entry| matches!(entry.command, PlayerCommand::Select { .. }))
            .count()
    }
}

/// Five own peons in a row, an own hall behind them and a hall of player 1.
pub(crate) fn row() -> Vec<Spec> {
    vec![
        (0, PEON, [24.0, 40.0]),
        (0, PEON, [40.0, 40.0]),
        (0, PEON, [56.0, 40.0]),
        (0, PEON, [72.0, 40.0]),
        (0, PEON, [88.0, 40.0]),
        (0, HALL, [56.0, 76.0]),
        (1, HALL, [100.0, 76.0]),
    ]
}

pub(crate) const SCREEN: [f32; 2] = [1024.0, 768.0];

#[test]
fn a_sweep_selects_own_units_over_structures_and_other_players() {
    let mut rig = Rig::new(&row(), SCREEN);
    let (min, max) = rig.around([24.0, 40.0], [100.0, 76.0], 6.0);
    rig.sweep(min, max);
    assert_eq!(rig.selection(), rig.units[..5]);
    assert_eq!(rig.selects_journaled(), 1);
    assert!(rig.outlines().is_empty());
}

#[test]
fn a_sweep_over_structures_only_takes_the_own_structure_and_a_lone_foreign_one() {
    let mut rig = Rig::new(&row(), SCREEN);
    let (min, max) = rig.around([56.0, 76.0], [56.0, 76.0], 6.0);
    rig.sweep(min, max);
    assert_eq!(rig.selection(), [rig.units[5]]);
    let (min, max) = rig.around([100.0, 76.0], [100.0, 76.0], 6.0);
    rig.sweep(min, max);
    assert_eq!(rig.selection(), [rig.units[6]]);
}

#[test]
fn an_empty_sweep_keeps_the_selection() {
    let mut rig = Rig::new(&row(), SCREEN);
    let first = rig.units[0];
    rig.select(&[first]);
    let from = rig.screen([104.0, 40.0]);
    rig.sweep(
        [from[0] - 10.0, from[1] - 10.0],
        [from[0] + 10.0, from[1] + 10.0],
    );
    assert_eq!(rig.selection(), [first]);
    assert_eq!(rig.selects_journaled(), 0);
}

#[test]
fn several_other_player_units_are_refused_loudly() {
    let mut rig = Rig::new(&[(1, PEON, [40.0, 40.0]), (1, PEON, [56.0, 40.0])], SCREEN);
    let (min, max) = rig.around([40.0, 40.0], [56.0, 40.0], 6.0);
    rig.sweep(min, max);
    assert!(rig.selection().is_empty());
    assert!(
        rig.error()
            .is_some_and(|e| e.contains("2 units of other players")),
        "{:?}",
        rig.error()
    );
}

#[test]
fn a_zero_area_press_is_the_plain_click_it_was() {
    let mut rig = Rig::new(&row(), SCREEN);
    let at = rig.screen([40.0, 40.0]);
    rig.click(at);
    assert_eq!(rig.selection(), [rig.units[1]]);
    // Shift click adds one more, exactly as before.
    rig.hold(KeyCode::ShiftLeft, true);
    let at = rig.screen([56.0, 40.0]);
    rig.click(at);
    rig.hold(KeyCode::ShiftLeft, false);
    assert_eq!(rig.selection(), [rig.units[1], rig.units[2]]);
    // A horizontal-only drag has no area and clicks at the press position.
    let at = rig.screen([72.0, 40.0]);
    rig.press(at);
    rig.release([at[0] + 40.0, at[1]]);
    assert_eq!(rig.selection(), [rig.units[3]]);
}

#[test]
fn the_rectangle_follows_the_cursor_in_relative_layout_and_vanishes_on_release() {
    for size in [SCREEN, [640.0, 480.0], [1920.0, 1080.0]] {
        let mut rig = Rig::new(&row(), size);
        let from = [size[0] * 0.25, size[1] * 0.25];
        let to = [size[0] * 0.5, size[1] * 0.5];
        rig.press(from);
        assert!(rig.outlines().is_empty());
        rig.drag(to);
        let outlines = rig.outlines();
        assert_eq!(outlines.len(), 1);
        let (node, border) = &outlines[0];
        assert_eq!(node.position_type, PositionType::Absolute);
        for (value, expected) in [
            (node.left, 25.0),
            (node.top, 25.0),
            (node.width, 25.0),
            (node.height, 25.0),
        ] {
            let Val::Percent(percent) = value else {
                panic!("outline is positioned in percent, got {value:?}");
            };
            assert!((percent - expected).abs() < 0.01, "{size:?} {percent}");
        }
        // The colour is the profile's ColorFriend (ARGB), not a constant of the module.
        assert_eq!(*border, BorderColor::all(Color::srgba_u8(0, 255, 0, 255)));
        // Dragging back through the press point flips the rectangle, never inverts it.
        rig.drag([from[0] - 20.0, from[1] - 20.0]);
        let (node, _) = &rig.outlines()[0];
        assert!(matches!(node.width, Val::Percent(p) if p > 0.0));
        rig.release(to);
        assert!(rig.outlines().is_empty());
    }
}

#[test]
fn the_cap_keeps_twelve_and_says_the_choice_is_not_established() {
    let specs: Vec<Spec> = (0..14)
        .map(|n| (0, PEON, [12.0 + 8.0 * n as f32, 40.0]))
        .collect();
    let mut rig = Rig::new(&specs, SCREEN);
    let (min, max) = rig.around([12.0, 40.0], [12.0 + 8.0 * 13.0, 40.0], 4.0);
    rig.sweep(min, max);
    assert_eq!(rig.selection(), rig.units[..CAP]);
    assert!(
        rig.error()
            .is_some_and(|e| e.contains("not established") && e.contains("12")),
        "{:?}",
        rig.error()
    );
}

#[test]
fn shift_sweeps_add_and_never_toggle() {
    let mut rig = Rig::new(&row(), SCREEN);
    let (min, max) = rig.around([24.0, 40.0], [40.0, 40.0], 6.0);
    rig.sweep(min, max);
    assert_eq!(rig.selection(), rig.units[..2]);
    rig.hold(KeyCode::ShiftLeft, true);
    let (min, max) = rig.around([56.0, 40.0], [72.0, 40.0], 6.0);
    rig.sweep(min, max);
    assert_eq!(rig.selection(), rig.units[..4]);
    // Sweeping already-selected units again keeps them.
    let (min, max) = rig.around([24.0, 40.0], [40.0, 40.0], 6.0);
    rig.sweep(min, max);
    rig.hold(KeyCode::ShiftLeft, false);
    assert_eq!(rig.selection(), rig.units[..4]);
}

#[test]
fn an_armed_command_claims_the_press_and_no_box_starts() {
    let mut rig = Rig::new(&row(), SCREEN);
    let first = rig.units[0];
    rig.select(&[first]);
    let witness = rig
        .app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>()
        .session_witness()
        .unwrap();
    rig.app.world_mut().resource_mut::<CampaignInput>().command = Some(ArmedCommand {
        order: 3,
        kind: crate::campaign_command_card::Kind::TargetUnit,
        session: witness,
    });
    let (min, max) = rig.around([24.0, 40.0], [88.0, 40.0], 6.0);
    rig.press(min);
    rig.drag(max);
    assert!(rig.outlines().is_empty());
    rig.release(max);
    // A left press with no unit target disarms and selects nothing.
    assert_eq!(rig.selection(), [first]);
    assert!(
        rig.app
            .world()
            .resource::<CampaignInput>()
            .command
            .is_none()
    );
    assert_eq!(rig.selects_journaled(), 0);
}

#[test]
fn arming_a_command_during_a_drag_cancels_it() {
    let mut rig = Rig::new(&row(), SCREEN);
    let (min, max) = rig.around([24.0, 40.0], [88.0, 40.0], 6.0);
    rig.press(min);
    rig.drag(max);
    assert_eq!(rig.outlines().len(), 1);
    let witness = rig
        .app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>()
        .session_witness()
        .unwrap();
    rig.app.world_mut().resource_mut::<CampaignInput>().command = Some(ArmedCommand {
        order: 3,
        kind: crate::campaign_command_card::Kind::TargetUnit,
        session: witness,
    });
    rig.drag(max);
    assert!(rig.outlines().is_empty());
    rig.release(max);
    assert!(rig.selection().is_empty());
}

#[test]
fn escape_cancels_the_drag_without_selecting_or_ending_a_cinematic() {
    let mut rig = Rig::new(&row(), SCREEN);
    let (min, max) = rig.around([24.0, 40.0], [88.0, 40.0], 6.0);
    rig.press(min);
    rig.drag(max);
    let window = rig.window;
    rig.app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::Escape,
        logical_key: Key::Escape,
        state: ButtonState::Pressed,
        text: None,
        repeat: false,
        window,
    });
    rig.frame();
    assert!(rig.outlines().is_empty());
    assert!(
        rig.app
            .world()
            .resource::<CampaignInput>()
            .escape_cancelled_drag
    );
    rig.release(max);
    assert!(rig.selection().is_empty());
    assert_eq!(rig.selects_journaled(), 0);
}

#[test]
fn double_click_takes_every_own_unit_of_that_type_on_screen() {
    let mut specs = row();
    specs.push((0, FOOTMAN, [16.0, 28.0]));
    specs.push((1, PEON, [110.0, 28.0]));
    let mut rig = Rig::new(&specs, SCREEN);
    let at = rig.screen([40.0, 40.0]);
    rig.click(at);
    rig.click(at);
    assert_eq!(rig.selection(), rig.units[..5]);
    // A pause past the window is two single clicks.
    let mut rig = Rig::new(&specs, SCREEN);
    rig.click(at);
    for _ in 0..12 {
        rig.frame();
    }
    rig.click(at);
    assert_eq!(rig.selection(), [rig.units[1]]);
    // Another player's unit never groups.
    let mut rig = Rig::new(&specs, SCREEN);
    let foreign = rig.screen([110.0, 28.0]);
    rig.click(foreign);
    rig.click(foreign);
    assert_eq!(rig.selection(), [rig.units[8]]);
}

#[test]
fn without_the_client_rules_a_press_selects_at_once_and_no_box_is_offered() {
    let mut rig = Rig::new(&row(), SCREEN);
    rig.app.insert_resource(BoxSelect::default());
    let at = rig.screen([40.0, 40.0]);
    rig.cursor(at);
    rig.button(MouseButton::Left, ButtonState::Pressed);
    rig.frame();
    assert_eq!(rig.selection(), [rig.units[1]]);
    rig.button(MouseButton::Left, ButtonState::Released);
    rig.frame();
    // Every press is a point pick now: a sweep over bare ground draws nothing and selects nothing.
    let (min, max) = rig.around([104.0, 40.0], [110.0, 46.0], 2.0);
    rig.press(min);
    rig.drag(max);
    assert!(rig.outlines().is_empty());
    rig.release(max);
    assert_eq!(rig.selection(), [rig.units[1]]);
}

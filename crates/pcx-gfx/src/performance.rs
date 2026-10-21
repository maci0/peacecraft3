//! Optional screen-space diagnostics; timing is observational, not a test gate.
use std::fmt::Write;

use bevy::{
    camera::visibility::RenderLayers,
    diagnostic::{Diagnostic, DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    prelude::*,
};

#[path = "performance_debug.rs"]
mod debug;
pub use debug::DebugOverlay;

/// Display policy: one column per recent frame, independent of simulation ticks.
const GRAPH_SAMPLES: usize = 120;

pub struct PerformanceOverlayPlugin;

impl Plugin for PerformanceOverlayPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<FrameTimeDiagnosticsPlugin>() {
            app.add_plugins(FrameTimeDiagnosticsPlugin::new(GRAPH_SAMPLES));
        }
        app.init_resource::<OverlayState>()
            .init_resource::<DebugOverlay>()
            .add_systems(Startup, setup)
            .add_systems(Update, (toggle, refresh, debug::refresh).chain());
        #[cfg(feature = "profile-gpu")]
        app.add_plugins((
            bevy::render::diagnostic::RenderDiagnosticsPlugin,
            bevy::diagnostic::LogDiagnosticsPlugin::default(),
        ));
    }
}

#[derive(Resource)]
struct OverlayState {
    visible: bool,
    refresh: Timer,
}

impl Default for OverlayState {
    fn default() -> Self {
        Self {
            visible: false,
            refresh: Timer::from_seconds(0.25, TimerMode::Repeating),
        }
    }
}

#[derive(Component)]
struct OverlayCamera;
#[derive(Component)]
struct OverlayRoot;
#[derive(Component)]
struct CounterText;
#[derive(Component)]
struct GraphLabel;
#[derive(Component)]
struct GraphBar(usize);
#[derive(Component)]
struct DebugRoot;

type OverlayPanels<'w, 's> = Query<
    'w,
    's,
    (&'static mut Visibility, &'static mut Node, Has<DebugRoot>),
    Or<(With<OverlayRoot>, With<DebugRoot>)>,
>;

fn panel() -> Node {
    Node {
        display: Display::None,
        width: Val::Rem(22.0),
        max_width: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        padding: UiRect::all(Val::Rem(0.5)),
        row_gap: Val::Rem(0.25),
        ..default()
    }
}

fn setup(mut commands: Commands) {
    let camera = commands
        .spawn((
            Camera2d,
            Camera {
                order: 100,
                clear_color: bevy::camera::ClearColorConfig::None,
                is_active: false,
                ..default()
            },
            // Exclude game meshes; this camera draws only its explicitly targeted UI.
            RenderLayers::layer(31),
            OverlayCamera,
            Name::new("performance-overlay-camera"),
        ))
        .id();
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Start,
                align_items: AlignItems::End,
                row_gap: Val::Rem(0.5),
                padding: UiRect::all(Val::Rem(0.5)),
                ..default()
            },
            UiTargetCamera(camera),
            GlobalZIndex(i32::MAX),
            Pickable::IGNORE,
            Name::new("performance-overlay"),
        ))
        .with_children(|root| {
            root.spawn((
                panel(),
                Visibility::Hidden,
                OverlayRoot,
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.8)),
                Pickable::IGNORE,
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new("FPS -  - ms"),
                    TextFont {
                        font_size: bevy::text::FontSize::Rem(1.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    Pickable::IGNORE,
                    CounterText,
                ));
                panel.spawn((
                    Text::new("Frame-time history unavailable"),
                    TextFont {
                        font_size: bevy::text::FontSize::Rem(0.75),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    Pickable::IGNORE,
                    GraphLabel,
                ));
                panel
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Rem(4.0),
                            align_items: AlignItems::End,
                            overflow: Overflow::clip(),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.08)),
                        Pickable::IGNORE,
                    ))
                    .with_children(|graph| {
                        for index in 0..GRAPH_SAMPLES {
                            graph.spawn((
                                Node {
                                    flex_basis: Val::Px(0.0),
                                    flex_grow: 1.0,
                                    min_width: Val::Px(0.0),
                                    height: Val::Percent(0.0),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.35, 0.8, 1.0)),
                                Pickable::IGNORE,
                                GraphBar(index),
                            ));
                        }
                    });
            });
            root.spawn((
                Node {
                    max_height: Val::Vh(65.0),
                    overflow: Overflow::clip(),
                    ..panel()
                },
                Visibility::Hidden,
                DebugRoot,
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.8)),
                Pickable::IGNORE,
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new("Debug (F7)"),
                    TextFont {
                        font_size: bevy::text::FontSize::Rem(0.8),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    Pickable::IGNORE,
                    debug::DebugText,
                ));
            });
        });
}

fn toggle(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<OverlayState>,
    mut debug: ResMut<DebugOverlay>,
    mut roots: OverlayPanels,
    mut cameras: Query<&mut Camera, With<OverlayCamera>>,
) {
    if !keys.just_pressed(KeyCode::F8) && !keys.just_pressed(KeyCode::F7) {
        return;
    }
    if keys.just_pressed(KeyCode::F8) {
        state.visible = !state.visible;
    }
    if keys.just_pressed(KeyCode::F7) {
        debug.visible = !debug.visible;
    }
    let duration = state.refresh.duration();
    state.refresh.set_elapsed(duration);
    for (mut visibility, mut node, is_debug) in &mut roots {
        let visible = if is_debug {
            debug.visible
        } else {
            state.visible
        };
        node.display = if visible {
            Display::Flex
        } else {
            Display::None
        };
        *visibility = if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    for mut camera in &mut cameras {
        camera.is_active = state.visible || debug.visible;
    }
}

fn refresh(
    time: Res<Time<Real>>,
    diagnostics: Res<DiagnosticsStore>,
    mut state: ResMut<OverlayState>,
    mut counters: Query<&mut Text, With<CounterText>>,
    mut labels: Query<&mut Text, (With<GraphLabel>, Without<CounterText>)>,
    mut bars: Query<(&GraphBar, &mut Node)>,
) {
    if !state.visible || !state.refresh.tick(time.delta()).just_finished() {
        return;
    }
    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|value| value.smoothed())
        .filter(|value| value.is_finite() && *value >= 0.0);
    let frame_ms = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
        .and_then(|value| value.smoothed())
        .filter(|value| value.is_finite() && *value >= 0.0);
    for mut counter in &mut counters {
        counter.0.clear();
        match (fps, frame_ms) {
            (Some(fps), Some(ms)) => {
                let _ = write!(counter.0, "FPS {fps:.0}  {ms:.1} ms");
            }
            _ => counter.0.push_str("FPS -  - ms"),
        }
    }
    let (heights, ceiling, count) =
        graph_history(diagnostics.get(&FrameTimeDiagnosticsPlugin::FRAME_TIME));
    for mut label in &mut labels {
        label.0 = if count == 0 {
            "Frame-time history unavailable".into()
        } else {
            format!("Frame time | last {count} frames | 0..{ceiling:.1} ms")
        };
    }
    for (bar, mut node) in &mut bars {
        node.height = Val::Percent(heights[bar.0]);
    }
}

fn graph_history(diagnostic: Option<&Diagnostic>) -> ([f32; GRAPH_SAMPLES], f64, usize) {
    let mut values = [0.0; GRAPH_SAMPLES];
    let mut ceiling = 1.0_f64;
    let mut count = 0;
    if let Some(diagnostic) = diagnostic {
        count = diagnostic.history_len().min(GRAPH_SAMPLES);
        for (slot, &value) in values[GRAPH_SAMPLES - count..]
            .iter_mut()
            .zip(diagnostic.values().skip(diagnostic.history_len() - count))
        {
            if value.is_finite() && value >= 0.0 {
                *slot = value;
                ceiling = ceiling.max(value);
            }
        }
    }
    ceiling = ceiling.ceil();
    (
        values.map(|value| (value / ceiling * 100.0) as f32),
        ceiling,
        count,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn graph_keeps_recent_frames_and_gaps_invalid_values() {
        use bevy::diagnostic::DiagnosticMeasurement;
        let mut diagnostic = Diagnostic::new(FrameTimeDiagnosticsPlugin::FRAME_TIME)
            .with_max_history_length(GRAPH_SAMPLES + 2);
        for value in std::iter::once(900.0)
            .chain(std::iter::repeat_n(10.0, GRAPH_SAMPLES - 1))
            .chain([f64::NAN, 20.0])
        {
            diagnostic.add_measurement(DiagnosticMeasurement {
                time: bevy::platform::time::Instant::now(),
                value,
            });
        }
        let (heights, ceiling, count) = graph_history(Some(&diagnostic));
        assert_eq!(count, GRAPH_SAMPLES);
        assert_eq!(ceiling, 20.0);
        assert_eq!(heights[0], 50.0);
        assert_eq!(heights[GRAPH_SAMPLES - 2], 0.0);
        assert_eq!(heights[GRAPH_SAMPLES - 1], 100.0);
        assert!(
            heights
                .iter()
                .all(|height| height.is_finite() && (0.0..=100.0).contains(height))
        );
        assert_eq!(graph_history(None), ([0.0; GRAPH_SAMPLES], 1.0, 0));
    }

    #[test]
    fn f7_and_f8_are_independent_and_share_the_overlay_camera() {
        let mut world = World::new();
        world.init_resource::<OverlayState>();
        world.init_resource::<DebugOverlay>();
        world.init_resource::<ButtonInput<KeyCode>>();
        let performance = world.spawn((OverlayRoot, Visibility::Hidden, panel())).id();
        let debug = world.spawn((DebugRoot, Visibility::Hidden, panel())).id();
        let camera = world
            .spawn((
                OverlayCamera,
                Camera {
                    is_active: false,
                    ..default()
                },
            ))
            .id();
        for (key, fps, debugging) in [
            (KeyCode::F7, false, true),
            (KeyCode::F8, true, true),
            (KeyCode::F8, false, true),
            (KeyCode::F7, false, false),
        ] {
            world.resource_mut::<ButtonInput<KeyCode>>().reset_all();
            world.resource_mut::<ButtonInput<KeyCode>>().press(key);
            world.run_system_once(toggle).unwrap();
            assert_eq!(
                world.get::<Visibility>(performance),
                Some(&if fps {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                })
            );
            assert_eq!(
                world.get::<Visibility>(debug),
                Some(&if debugging {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                })
            );
            assert_eq!(
                world.get::<Camera>(camera).unwrap().is_active,
                fps || debugging
            );
        }
    }

    #[test]
    fn f8_toggles_visibility_and_camera_without_repeating_while_held() {
        let mut world = World::new();
        world.init_resource::<OverlayState>();
        world.init_resource::<DebugOverlay>();
        world.init_resource::<ButtonInput<KeyCode>>();
        let root = world.spawn((OverlayRoot, Visibility::Hidden, panel())).id();
        let camera = world
            .spawn((
                OverlayCamera,
                Camera {
                    is_active: false,
                    ..default()
                },
            ))
            .id();
        world
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::F8);
        world.run_system_once(toggle).unwrap();
        assert_eq!(*world.get::<Visibility>(root).unwrap(), Visibility::Visible);
        assert!(world.get::<Camera>(camera).unwrap().is_active);
        world.resource_mut::<ButtonInput<KeyCode>>().clear();
        world.run_system_once(toggle).unwrap();
        assert!(world.resource::<OverlayState>().visible);
        world
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::F8);
        world
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::F8);
        world.run_system_once(toggle).unwrap();
        assert_eq!(*world.get::<Visibility>(root).unwrap(), Visibility::Hidden);
        assert!(!world.get::<Camera>(camera).unwrap().is_active);
    }
}

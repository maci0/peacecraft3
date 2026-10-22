//! Optional screen-space diagnostics; timing is observational, not a test gate.
use std::fmt::Write;

use bevy::{
    camera::visibility::RenderLayers,
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    prelude::*,
};

#[path = "performance_debug.rs"]
mod debug;
pub use debug::DebugOverlay;

#[path = "performance_history.rs"]
mod history;
use history::{History, SAMPLES as GRAPH_SAMPLES};

pub struct PerformanceOverlayPlugin;

impl Plugin for PerformanceOverlayPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<FrameTimeDiagnosticsPlugin>() {
            app.add_plugins(FrameTimeDiagnosticsPlugin::default());
        }
        app.init_resource::<OverlayState>()
            .init_resource::<DebugOverlay>()
            .add_systems(Startup, setup)
            .add_systems(Update, (toggle, refresh, debug::refresh).chain())
            .add_systems(PostUpdate, fit_debug.after(bevy::ui::UiSystems::Layout));
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
    history: History,
}

impl Default for OverlayState {
    fn default() -> Self {
        Self {
            visible: false,
            refresh: Timer::from_seconds(0.25, TimerMode::Repeating),
            history: History::default(),
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
struct LiveMarker;
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
        width: Val::Rem(20.0),
        max_width: Val::Percent(100.0),
        min_width: Val::Px(0.0),
        flex_shrink: 0.0,
        flex_direction: FlexDirection::Column,
        padding: UiRect::all(Val::Rem(0.625)),
        border: UiRect::all(Val::Px(1.0)),
        border_radius: BorderRadius::all(Val::Px(6.0)),
        row_gap: Val::Rem(0.375),
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
                flex_direction: FlexDirection::RowReverse,
                flex_wrap: FlexWrap::Wrap,
                justify_content: JustifyContent::Start,
                align_items: AlignItems::Start,
                align_content: AlignContent::Start,
                column_gap: Val::Rem(0.5),
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
                BackgroundColor(Color::srgba(0.012, 0.018, 0.016, 0.32)),
                BorderColor::all(Color::srgba(0.85, 0.95, 0.9, 0.10)),
                Pickable::IGNORE,
            ))
            .with_children(|panel| {
                panel
                    .spawn((
                        Node {
                            justify_content: JustifyContent::SpaceBetween,
                            padding: UiRect::bottom(Val::Rem(0.375)),
                            border: UiRect::bottom(Val::Px(1.0)),
                            ..default()
                        },
                        BorderColor::all(Color::srgba(0.85, 0.95, 0.95, 0.12)),
                        Pickable::IGNORE,
                    ))
                    .with_children(|row| {
                        for text in ["PERFORMANCE", "F8"] {
                            row.spawn((
                                Text::new(text),
                                TextFont {
                                    font_size: bevy::text::FontSize::Rem(0.65),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.71, 0.86, 0.77)),
                                Pickable::IGNORE,
                            ));
                        }
                    });
                panel
                    .spawn((
                        Node {
                            justify_content: JustifyContent::SpaceBetween,
                            ..default()
                        },
                        Pickable::IGNORE,
                    ))
                    .with_children(|row| {
                        row.spawn((
                            Text::new("FPS\nFrame interval"),
                            TextFont {
                                font_size: bevy::text::FontSize::Rem(0.85),
                                ..default()
                            },
                            TextColor(Color::srgb(0.82, 0.88, 0.85)),
                            TextShadow {
                                offset: Vec2::splat(1.0),
                                ..default()
                            },
                            Pickable::IGNORE,
                        ));
                        row.spawn((
                            Text::new("-\n- ms"),
                            TextLayout::justify(Justify::Right),
                            TextFont {
                                font_size: bevy::text::FontSize::Rem(0.85),
                                ..default()
                            },
                            TextColor(Color::srgb(0.96, 0.98, 0.96)),
                            TextShadow {
                                offset: Vec2::splat(1.0),
                                ..default()
                            },
                            Pickable::IGNORE,
                            CounterText,
                        ));
                    });
                panel.spawn((
                    Text::new("Frame-time history unavailable"),
                    TextFont {
                        font_size: bevy::text::FontSize::Rem(0.65),
                        ..default()
                    },
                    TextColor(Color::srgb(0.66, 0.75, 0.73)),
                    Pickable::IGNORE,
                    GraphLabel,
                ));
                panel
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Rem(3.0),
                            align_items: AlignItems::End,
                            overflow: Overflow::clip(),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.08)),
                        Pickable::IGNORE,
                    ))
                    .with_children(|graph| {
                        for top in [25.0, 50.0, 75.0] {
                            graph.spawn((
                                Node {
                                    position_type: PositionType::Absolute,
                                    top: Val::Percent(top),
                                    width: Val::Percent(100.0),
                                    height: Val::Px(1.0),
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(0.7, 0.85, 0.9, 0.12)),
                                Pickable::IGNORE,
                            ));
                        }
                        for index in 0..GRAPH_SAMPLES {
                            graph.spawn((
                                Node {
                                    flex_basis: Val::Px(0.0),
                                    flex_grow: 1.0,
                                    min_width: Val::Px(0.0),
                                    height: Val::Percent(0.0),
                                    border: UiRect::top(Val::Px(1.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(0.35, 0.8, 1.0, 0.10)),
                                BorderColor::all(Color::srgba(0.5, 0.88, 1.0, 0.85)),
                                Pickable::IGNORE,
                                GraphBar(index),
                            ));
                        }
                        graph.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                right: Val::Px(0.0),
                                bottom: Val::Percent(0.0),
                                width: Val::Px(5.0),
                                height: Val::Px(2.0),
                                margin: UiRect::bottom(Val::Px(-1.0)),
                                ..default()
                            },
                            BackgroundColor(Color::WHITE),
                            Pickable::IGNORE,
                            LiveMarker,
                        ));
                    });
            });
            root.spawn((
                Node {
                    width: Val::Rem(34.0),
                    max_width: Val::Percent(100.0),
                    flex_shrink: 0.0,
                    margin: UiRect::right(Val::Auto),
                    max_height: Val::Vh(95.0),
                    overflow: Overflow::scroll_y(),
                    ..panel()
                },
                Visibility::Hidden,
                DebugRoot,
                ScrollPosition::default(),
                BackgroundColor(Color::srgba(0.012, 0.018, 0.016, 0.32)),
                BorderColor::all(Color::srgba(0.85, 0.95, 0.9, 0.10)),
                Pickable::IGNORE,
            ))
            .with_children(|panel| {
                panel
                    .spawn((
                        Node {
                            justify_content: JustifyContent::SpaceBetween,
                            padding: UiRect::bottom(Val::Rem(0.375)),
                            border: UiRect::bottom(Val::Px(1.0)),
                            ..default()
                        },
                        BorderColor::all(Color::srgba(0.85, 0.95, 0.95, 0.12)),
                        Pickable::IGNORE,
                    ))
                    .with_children(|row| {
                        for text in ["RENDERER / WORLD", "F7 | PgUp / PgDn"] {
                            row.spawn((
                                Text::new(text),
                                TextFont {
                                    font_size: bevy::text::FontSize::Rem(0.65),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.71, 0.86, 0.77)),
                                Pickable::IGNORE,
                            ));
                        }
                    });
                panel
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            min_width: Val::Px(0.0),
                            flex_wrap: FlexWrap::Wrap,
                            column_gap: Val::Rem(0.75),
                            row_gap: Val::Rem(0.75),
                            ..default()
                        },
                        Pickable::IGNORE,
                    ))
                    .with_children(|sections| {
                        for (index, title) in debug::SECTIONS.iter().enumerate() {
                            sections
                                .spawn((
                                    Node {
                                        flex_basis: Val::Rem(15.0),
                                        flex_grow: 1.0,
                                        min_width: Val::Px(0.0),
                                        ..default()
                                    },
                                    Text::new(format!("{title}\n")),
                                    TextFont {
                                        font_size: bevy::text::FontSize::Rem(0.7),
                                        ..default()
                                    },
                                    TextColor(Color::srgb(0.71, 0.86, 0.77)),
                                    TextShadow {
                                        offset: Vec2::splat(1.0),
                                        ..default()
                                    },
                                    Pickable::IGNORE,
                                ))
                                .with_children(|section| {
                                    section.spawn((
                                        TextSpan::new("Unavailable"),
                                        TextFont {
                                            font_size: bevy::text::FontSize::Rem(0.7),
                                            ..default()
                                        },
                                        TextColor(Color::srgb(0.96, 0.98, 0.96)),
                                        debug::DebugText(index),
                                    ));
                                });
                        }
                    });
            });
        });
}

fn fit_debug(
    debug: Res<DebugOverlay>,
    keys: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut panels: Query<
        (
            &ComputedNode,
            &UiGlobalTransform,
            &mut Node,
            &mut ScrollPosition,
        ),
        With<DebugRoot>,
    >,
) {
    if !debug.visible {
        return;
    }
    let Ok(window) = windows.single() else { return };
    for (computed, transform, mut node, mut scroll) in &mut panels {
        let top = transform.to_scale_angle_translation().2.y - computed.size().y * 0.5;
        let scale = computed.inverse_scale_factor();
        // Bound against the actual viewport even when the flex row wraps.
        let width = Val::Px((window.physical_width() as f32 * scale - 20.0).max(0.0));
        if node.max_width != width {
            node.max_width = width;
        }
        // Wrapped F7 uses only the space below F8; keyboard paging keeps every row reachable.
        let height = ((window.physical_height() as f32 - top) * scale - 10.0).max(0.0);
        let limit = Val::Px(height);
        if node.max_height != limit {
            node.max_height = limit;
        }
        let page = computed.size().y * scale * 0.8;
        if keys.just_pressed(KeyCode::PageDown) {
            let end = ((computed.content_size().y - computed.size().y) * scale).max(0.0);
            scroll.0.y = (scroll.0.y + page).min(end);
        }
        if keys.just_pressed(KeyCode::PageUp) {
            scroll.0.y = (scroll.0.y - page).max(0.0);
        }
    }
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
        state.history.clear();
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
    mut bars: Query<(&GraphBar, &mut Node), Without<LiveMarker>>,
    mut marker: Query<&mut Node, With<LiveMarker>>,
) {
    if !state.visible {
        return;
    }
    // Same real-time source as Bevy's frame diagnostic; never sample simulation ticks.
    if !time.delta().is_zero() {
        state.history.push(time.delta_secs_f64() * 1000.0);
    }
    let (heights, ceiling, count) = state.history.graph();
    for (bar, mut node) in &mut bars {
        node.height = Val::Percent(heights[bar.0]);
    }
    for mut node in &mut marker {
        node.bottom = Val::Percent(heights[GRAPH_SAMPLES - 1]);
    }
    if !state.refresh.tick(time.delta()).just_finished() {
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
                let _ = write!(counter.0, "{fps:.2}\n{ms:.2} ms");
            }
            _ => counter.0.push_str("-\n- ms"),
        }
    }
    for mut label in &mut labels {
        label.0 = if count == 0 {
            "Frame-time history unavailable".into()
        } else {
            format!("{count} frames | older peaks 2x..32x | 0..{ceiling:.0} ms")
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn wrapped_debug_fits_below_performance_and_pages_without_changing_game_input() {
        let mut world = World::new();
        world.insert_resource(DebugOverlay {
            visible: true,
            ..default()
        });
        world.init_resource::<ButtonInput<KeyCode>>();
        world.spawn((
            Window {
                resolution: (480, 800).into(),
                ..default()
            },
            bevy::window::PrimaryWindow,
        ));
        let panel = world
            .spawn((
                DebugRoot,
                panel(),
                ScrollPosition::default(),
                ComputedNode {
                    size: Vec2::new(440.0, 500.0),
                    content_size: Vec2::new(440.0, 1000.0),
                    inverse_scale_factor: 1.0,
                    ..default()
                },
                UiGlobalTransform::from(bevy::math::Affine2::from_translation(Vec2::new(
                    230.0, 470.0,
                ))),
            ))
            .id();
        world
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::PageDown);
        world.run_system_once(fit_debug).unwrap();
        assert_eq!(world.get::<Node>(panel).unwrap().max_height, Val::Px(570.0));
        assert_eq!(world.get::<Node>(panel).unwrap().max_width, Val::Px(460.0));
        assert_eq!(world.get::<ScrollPosition>(panel).unwrap().0.y, 400.0);
        world.resource_mut::<ButtonInput<KeyCode>>().reset_all();
        world
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::PageUp);
        world.run_system_once(fit_debug).unwrap();
        assert_eq!(world.get::<ScrollPosition>(panel).unwrap().0.y, 0.0);
        assert!(
            world
                .resource::<ButtonInput<KeyCode>>()
                .just_pressed(KeyCode::PageUp)
        );
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

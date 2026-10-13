//! Optional screen-space frame counter; timing is observational, not a test gate.
use std::fmt::Write;

use bevy::{
    camera::visibility::RenderLayers,
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    prelude::*,
};

pub struct PerformanceOverlayPlugin;

impl Plugin for PerformanceOverlayPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<FrameTimeDiagnosticsPlugin>() {
            app.add_plugins(FrameTimeDiagnosticsPlugin::default());
        }
        app.init_resource::<OverlayState>()
            .add_systems(Startup, setup)
            .add_systems(Update, (toggle, refresh).chain());
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
                justify_content: JustifyContent::End,
                align_items: AlignItems::Start,
                padding: UiRect::all(Val::Rem(0.5)),
                ..default()
            },
            UiTargetCamera(camera),
            GlobalZIndex(i32::MAX),
            Visibility::Hidden,
            Pickable::IGNORE,
            OverlayRoot,
            Name::new("performance-overlay"),
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("FPS —"),
                TextFont {
                    font_size: bevy::text::FontSize::Rem(1.0),
                    ..default()
                },
                TextColor(Color::WHITE),
                Node {
                    padding: UiRect::axes(Val::Rem(0.5), Val::Rem(0.25)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.8)),
                Pickable::IGNORE,
                CounterText,
            ));
        });
}

fn toggle(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<OverlayState>,
    mut roots: Query<&mut Visibility, With<OverlayRoot>>,
    mut cameras: Query<&mut Camera, With<OverlayCamera>>,
) {
    if !keys.just_pressed(KeyCode::F8) {
        return;
    }
    state.visible = !state.visible;
    for mut visibility in &mut roots {
        *visibility = if state.visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    for mut camera in &mut cameras {
        camera.is_active = state.visible;
    }
}

fn refresh(
    time: Res<Time<Real>>,
    diagnostics: Res<DiagnosticsStore>,
    mut state: ResMut<OverlayState>,
    mut counters: Query<&mut Text, With<CounterText>>,
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
                let _ = write!(counter.0, "FPS {fps:.0} · {ms:.1} ms");
            }
            _ => counter.0.push_str("FPS —"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn f8_toggles_visibility_and_camera_without_repeating_while_held() {
        let mut world = World::new();
        world.init_resource::<OverlayState>();
        world.init_resource::<ButtonInput<KeyCode>>();
        let root = world.spawn((OverlayRoot, Visibility::Hidden)).id();
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

//! Keeps menu slider track presses in the coordinate space used by Bevy's Slider.
use bevy::{picking::events::PointerPress, prelude::*, ui::ComputedUiTargetCamera};

#[derive(Component)]
struct TrackHitSurface;

/// The source-sized parent keeps its layout and thumb; this child only receives track hits.
pub(super) fn attach(commands: &mut Commands, parent: Entity) {
    let track = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            TrackHitSurface,
        ))
        .observe(localize_press)
        .id();
    commands.entity(parent).add_child(track);
}

fn localize_press(
    mut press: On<PointerPress>,
    tracks: Query<&ComputedUiTargetCamera, With<TrackHitSurface>>,
    cameras: Query<&Camera>,
) {
    // This child receives the event before it bubbles to the unchanged Slider handler.
    let Some(camera) = tracks
        .get(press.entity)
        .ok()
        .and_then(ComputedUiTargetCamera::get)
        .filter(|camera| *camera == press.hit.camera)
        .and_then(|entity| cameras.get(entity).ok())
    else {
        press.propagate(false);
        warn!(track = ?press.entity, "Menu slider track: missing matching UI camera");
        return;
    };
    let Some((scale, viewport)) = camera
        .target_scaling_factor()
        .zip(camera.physical_viewport_rect())
        .filter(|(scale, _)| scale.is_finite() && *scale > 0.0)
    else {
        press.propagate(false);
        warn!(track = ?press.entity, "Menu slider track: unavailable camera viewport");
        return;
    };
    press.pointer.position -= viewport.min.as_vec2() / scale;
}

#[cfg(test)]
#[path = "menu_slider_tests.rs"]
mod tests;

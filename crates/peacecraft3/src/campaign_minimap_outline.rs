//! Camera view outline over the minimap image.
//!
//! The client intersects the view frustum's four near-to-far edges with the horizontal plane
//! through the camera target, takes the bounding box of the intersection points and draws
//! its size around the target as a one-pixel line loop in the control's alpha and an RGB
//! operand of the draw method (`0x63fd30`, `0x640676`), clipped to the control.
use super::*;
use bevy::ui::UiRect;
use peacecraft3::campaign_camera::CampaignSceneCamera;

#[derive(Component)]
pub(super) struct Outline;

/// A pixel-wide outline: its width belongs to the screen, not to the minimap's scale.
const LINE_WIDTH: Val = Val::Px(1.);

/// Where a ray meets the plane at `plane_y`, if it runs down through it.
fn meets_plane(origin: Vec3, direction: Vec3, plane_y: f32) -> Option<Vec3> {
    if direction.y >= 0. || origin.y <= plane_y {
        return None;
    }
    Some(origin + direction * ((plane_y - origin.y) / direction.y))
}

/// Height of the plane through the camera's target, which lies on the optical axis.
/// `target` is the horizontal target in Bevy's X and Z.
fn target_plane(eye: Vec3, forward: Vec3, target: [f32; 2]) -> Option<f32> {
    let along = if forward.x.abs() >= forward.z.abs() {
        (target[0] - eye.x) / forward.x
    } else {
        (target[1] - eye.z) / forward.z
    };
    let y = eye.y + along * forward.y;
    (forward.x.hypot(forward.z) > f32::EPSILON && y.is_finite()).then_some(y)
}

/// World-space `[min, max]` of the corner rays' plane hits. World Y is Bevy's negated Z.
fn ground_box(rays: [(Vec3, Vec3); 4], plane_y: f32) -> Option<[[f32; 2]; 2]> {
    let mut low = [f32::INFINITY; 2];
    let mut high = [f32::NEG_INFINITY; 2];
    for (origin, direction) in rays {
        let hit = meets_plane(origin, direction, plane_y)?;
        let point = [hit.x, -hit.z];
        for axis in 0..2 {
            low[axis] = low[axis].min(point[axis]);
            high[axis] = high[axis].max(point[axis]);
        }
    }
    (low.iter().chain(&high).all(|v| v.is_finite())).then_some([low, high])
}

/// Left, top, width and height as fractions of the image, clipped to it.
fn clip(centre: [f64; 2], half: [f64; 2], dimensions: [u32; 2]) -> Option<[f32; 4]> {
    let [width, height] = dimensions.map(f64::from);
    let (x0, x1) = (
        (centre[0] - half[0]).max(0.),
        (centre[0] + half[0]).min(width),
    );
    let (y0, y1) = (
        (centre[1] - half[1]).max(0.),
        (centre[1] + half[1]).min(height),
    );
    (x1 > x0 && y1 > y0).then(|| {
        [
            (x0 / width) as f32,
            (y0 / height) as f32,
            ((x1 - x0) / width) as f32,
            ((y1 - y0) / height) as f32,
        ]
    })
}

pub(super) fn update(
    mut commands: Commands,
    mut state: ResMut<CampaignMinimap>,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
    camera: Res<CampaignCamera>,
    cameras: Query<(&Camera, &GlobalTransform), With<CampaignSceneCamera>>,
    mut outlines: Query<(&mut Node, &mut BorderColor), With<Outline>>,
) {
    let Some(parent) = state.image_node else {
        return;
    };
    let Some(rgb) = state.outline_rgb else {
        return;
    };
    let rect = (|| -> Option<[f32; 4]> {
        if !state.shown || !camera.active() {
            return None;
        }
        let fog = state.fog.as_ref()?;
        let objects = session.host()?.objects();
        let bounds = objects.vision_world_bounds()?;
        let target = objects.camera_target().or_else(|| {
            objects
                .applied_camera_setup()
                .map(|view| [view.destination.x, view.destination.y])
        })?;
        let (view, transform) = cameras.get(camera.entity?).ok()?;
        let size = view.logical_viewport_size()?;
        let mut rays = [(Vec3::ZERO, Vec3::ZERO); 4];
        for (ray, corner) in rays.iter_mut().zip([
            Vec2::ZERO,
            Vec2::new(size.x, 0.),
            size,
            Vec2::new(0., size.y),
        ]) {
            let cast = view.viewport_to_world(transform, corner).ok()?;
            *ray = (cast.origin, cast.direction.as_vec3());
        }
        let plane = target_plane(
            transform.translation(),
            transform.forward().as_vec3(),
            [target[0], -target[1]],
        )?;
        let [low, high] = ground_box(rays, plane)?;
        let [x0, y0] = fog.raster.pixel_of(bounds, low).ok()?;
        let [x1, y1] = fog.raster.pixel_of(bounds, high).ok()?;
        let centre = fog.raster.pixel_of(bounds, target).ok()?;
        clip(
            centre,
            [(x1 - x0).abs() * 0.5, (y1 - y0).abs() * 0.5],
            [fog.width, fog.height],
        )
    })();
    let color = BorderColor::all(Color::srgb_u8(rgb[0], rgb[1], rgb[2]));
    let Some(entity) = state.outline else {
        let entity = commands
            .spawn((
                Outline,
                Node {
                    position_type: PositionType::Absolute,
                    display: Display::None,
                    border: UiRect::all(LINE_WIDTH),
                    ..default()
                },
                color,
                Pickable::IGNORE,
                ChildOf(parent),
            ))
            .id();
        state.outline = Some(entity);
        return;
    };
    let Ok((mut node, mut border)) = outlines.get_mut(entity) else {
        return;
    };
    *border = color;
    match rect {
        Some([left, top, width, height]) => {
            node.display = Display::Flex;
            node.left = Val::Percent(left * 100.);
            node.top = Val::Percent(top * 100.);
            node.width = Val::Percent(width * 100.);
            node.height = Val::Percent(height * 100.);
        }
        None => node.display = Display::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rays_meet_the_plane_only_when_they_run_down_through_it() {
        let down = Vec3::new(0.5, -1., 0.);
        let hit = meets_plane(Vec3::new(1., 9., 2.), down, 3.).unwrap();
        assert_eq!(hit, Vec3::new(4., 3., 2.));
        assert!(meets_plane(Vec3::new(0., 2., 0.), down, 3.).is_none());
        assert!(meets_plane(Vec3::new(0., 9., 0.), Vec3::X, 3.).is_none());
        assert!(meets_plane(Vec3::new(0., 9., 0.), Vec3::Y, 3.).is_none());
    }

    #[test]
    fn the_plane_passes_through_the_target_on_the_optical_axis() {
        let eye = Vec3::new(10., 30., -4.);
        let forward = Vec3::new(0.6, -0.8, 0.).normalize();
        // Axis points are eye + t * forward: x = 22 is t = 20, so y = 30 - 20 * 0.8.
        let y = target_plane(eye, forward, [22., -4.]).unwrap();
        assert!((y - 14.).abs() < 1e-4, "{y}");
        // Looking along Z instead of X.
        let forward = Vec3::new(0., -0.6, -0.8);
        let y = target_plane(eye, forward, [10., -44.]).unwrap();
        assert!((y - (30. - 0.6 * 50.)).abs() < 1e-4, "{y}");
        assert!(target_plane(eye, Vec3::NEG_Y, [10., -4.]).is_none());
    }

    #[test]
    fn the_ground_box_is_the_corner_hits_in_world_axes_and_needs_all_four() {
        let eye = Vec3::new(0., 10., 0.);
        let rays = [
            (eye, Vec3::new(-1., -1., -2.)),
            (eye, Vec3::new(3., -1., -2.)),
            (eye, Vec3::new(3., -1., -1.)),
            (eye, Vec3::new(-1., -1., -1.)),
        ];
        // Hits at plane height 4 (t = 6): x from -6 to 18, Bevy z from -12 to -6.
        assert_eq!(ground_box(rays, 4.), Some([[-6., 6.], [18., 12.]]));
        let mut up = rays;
        up[2].1 = Vec3::new(3., 1., -1.);
        assert_eq!(ground_box(up, 4.), None);
        assert_eq!(ground_box(rays, 11.), None);
    }

    #[test]
    fn the_outline_is_clipped_to_the_image_and_empty_when_outside_it() {
        assert_eq!(
            clip([8., 4.], [2., 1.], [16, 8]),
            Some([0.375, 0.375, 0.25, 0.25])
        );
        // Half outside on the left and top: only the part inside remains.
        assert_eq!(
            clip([1., 1.], [3., 2.], [16, 8]),
            Some([0., 0., 4. / 16., 3. / 8.])
        );
        assert_eq!(clip([20., 4.], [2., 1.], [16, 8]), None);
        assert_eq!(clip([-3., 4.], [2., 1.], [16, 8]), None);
        assert_eq!(clip([8., 4.], [0., 1.], [16, 8]), None);
    }
}

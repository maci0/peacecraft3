//! Camera view outline over the minimap image.
//!
//! The client clips finite frustum edges with its signed near-minus-far operation,
//! measures their bounds and draws those extents around the mapped camera target
//! (`0x63fd30`). Preserve that operation rather than substituting ray intersection.
use super::*;
use peacecraft3::campaign_camera::CampaignSceneCamera;

#[derive(Component)]
pub(super) struct Outline;

#[derive(Component)]
pub(super) struct OutlineLine(usize);

// Aliased line coverage uses integer pixel centers and the diamond-exit rule.
fn line_span(start: f64, end: f64, fixed: f64, horizontal: bool) -> Option<[f32; 4]> {
    let row = (fixed + 0.5).floor();
    let radius = 0.5 - (fixed - row).abs();
    let (first, last) = if end > start {
        ((start - radius).floor() + 1., (end - radius).ceil() - 1.)
    } else if start > end {
        ((end + radius).floor() + 1., (start + radius).floor())
    } else {
        return None;
    };
    if last < first || ![first, last, row].into_iter().all(f64::is_finite) {
        return None;
    }
    Some(if horizontal {
        [first as f32, row as f32, (last - first + 1.) as f32, 1.]
    } else {
        [row as f32, first as f32, 1., (last - first + 1.) as f32]
    })
}

fn line_rects([x, y, width, height]: [f32; 4]) -> [Option<[f32; 4]>; 4] {
    let [x, y, width, height] = [x, y, width, height].map(f64::from);
    let (right, bottom) = (x + width, y + height);
    [
        line_span(y, bottom, x, false),
        line_span(x, right, bottom, true),
        line_span(bottom, y, right, false),
        line_span(right, x, y, true),
    ]
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

/// Source-clipped world-space bounds; source world Y is Bevy's negated Z.
fn ground_box(corners: [Vec3; 8], plane_y: f32, minimum_separation: f64) -> Option<[[f32; 2]; 2]> {
    if !plane_y.is_finite() || corners.iter().any(|point| !point.is_finite()) {
        return None;
    }
    let mut low = [f32::INFINITY; 2];
    let mut high = [f32::NEG_INFINITY; 2];
    for (near, mut far) in corners[..4].iter().zip(corners[4..].iter().copied()) {
        let delta = near.y - far.y;
        if near.y > plane_y && far.y < plane_y && f64::from(delta).abs() >= minimum_separation {
            let phase = (near.y - plane_y) / delta;
            // The source adds near-minus-far; ordinary ray intersection reverses this sign.
            far = *near + (*near - far) * phase;
        }
        let point = [far.x, -far.z];
        for axis in 0..2 {
            low[axis] = low[axis].min(point[axis]);
            high[axis] = high[axis].max(point[axis]);
        }
    }
    (low.iter().chain(&high).all(|v| v.is_finite())).then_some([low, high])
}

fn edge_visibility(centre: [f64; 2], half: [f64; 2], dimensions: [u32; 2]) -> [bool; 4] {
    [
        centre[0] - half[0] > 0.,
        centre[1] + half[1] < f64::from(dimensions[1]),
        centre[0] + half[0] < f64::from(dimensions[0]),
        centre[1] - half[1] > 0.,
    ]
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
    cameras: Query<(&GlobalTransform, &Projection), With<CampaignSceneCamera>>,
    ui_camera: Query<&Camera, With<peacecraft3::campaign_presentation::CampaignUiCamera>>,
    mut lines: Query<(&OutlineLine, &mut Node, &mut BackgroundColor)>,
) {
    let Some(parent) = state.root else {
        return;
    };
    let Some(rgb) = state.outline_rgb else {
        return;
    };
    let plan = (|| {
        if !state.shown || !camera.active() {
            return None;
        }
        let fog = state.fog.as_ref()?;
        let objects = session.host()?.objects();
        let bounds = fog.bounds(objects).ok()?;
        let target = objects.camera_target().or_else(|| {
            objects
                .applied_camera_setup()
                .map(|view| [view.destination.x, view.destination.y])
        })?;
        let (transform, projection) = cameras.get(camera.entity?).ok()?;
        let near = match projection {
            Projection::Perspective(p) => p.near,
            Projection::Custom(p) => p
                .get::<peacecraft3::campaign_camera::SourceProjection>()?
                .near(),
            Projection::Orthographic(_) => return None,
        };
        let corners = projection
            .get_frustum_corners(-near, -projection.far())
            .map(|point| transform.transform_point(Vec3::from(point)));
        let plane = target_plane(
            transform.translation(),
            transform.forward().as_vec3(),
            [target[0], -target[1]],
        )?;
        let [low, high] = ground_box(corners, plane, state.outline_minimum_separation?)?;
        let [x0, y0] = fog.raster.pixel_of(bounds, low).ok()?;
        let [x1, y1] = fog.raster.pixel_of(bounds, high).ok()?;
        let centre = fog.raster.pixel_of(bounds, target).ok()?;
        let half = [(x1 - x0).abs() * 0.5, (y1 - y0).abs() * 0.5];
        let edges = edge_visibility(centre, half, [fog.width, fog.height]);
        let [left, top, width, height] = clip(centre, half, [fog.width, fog.height])?;
        let mapping = fog.mapping.as_ref()?;
        // Stroke width follows the HUD target, independently of internal world resolution.
        let extent = ui_camera.single().ok()?.physical_target_size()?.as_vec2();
        if extent.min_element() <= 0. {
            return None;
        }
        let scale = extent / Vec2::from(mapping.viewport);
        let frame = mapping.frame;
        let rect = [
            (frame.x + left * frame.width) * scale.x,
            (mapping.viewport[1] - frame.y - frame.height + top * frame.height) * scale.y,
            width * frame.width * scale.x,
            height * frame.height * scale.y,
        ];
        let mut lines = line_rects(rect);
        for (line, visible) in lines.iter_mut().zip(edges) {
            if !visible {
                *line = None;
            }
        }
        Some((lines, extent))
    })();
    let color = crate::menu_composition::colour_u8(rgb[0], rgb[1], rgb[2], u8::MAX);
    if state.outline.is_none() {
        let root = commands
            .spawn((
                Outline,
                ZIndex(2),
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.),
                    height: Val::Percent(100.),
                    ..default()
                },
                Pickable::IGNORE,
                ChildOf(parent),
            ))
            .id();
        for index in 0..4 {
            commands.spawn((
                OutlineLine(index),
                Node {
                    position_type: PositionType::Absolute,
                    display: Display::None,
                    ..default()
                },
                BackgroundColor(color),
                Pickable::IGNORE,
                ChildOf(root),
            ));
        }
        state.outline = Some(root);
        return;
    }
    for (line, mut node, mut background) in &mut lines {
        background.0 = color;
        if let Some((rects, extent)) = &plan
            && let Some([left, top, width, height]) = rects[line.0]
        {
            node.display = Display::Flex;
            node.left = Val::Percent(left / extent.x * 100.);
            node.top = Val::Percent(top / extent.y * 100.);
            node.width = Val::Percent(width / extent.x * 100.);
            node.height = Val::Percent(height / extent.y * 100.);
        } else {
            node.display = Display::None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn aliased_lines_keep_directional_endpoint_coverage() {
        assert_eq!(line_span(1.6, 7.2, 2.3, true), Some([2., 2., 5., 1.]));
        assert_eq!(line_span(7.2, 1.6, 2.3, true), Some([2., 2., 6., 1.]));
        assert_eq!(line_span(7.2, 1.6, 2.3, false), Some([2., 2., 1., 6.]));
        assert_eq!(line_span(2., 2., 2., true), None);
        assert_eq!(line_span(f64::NAN, 3., 2., true), None);
        assert!(
            line_rects([1.6, 2.3, 5.6, 4.5])
                .into_iter()
                .all(|line| line.is_some())
        );
    }

    #[test]
    fn source_signed_clip_preserves_finite_far_corners_and_height_gate() {
        let near = Vec3::new(2., 10., -3.);
        let far = Vec3::new(8., -2., -9.);
        let corners = [near, near, near, near, far, far, far, far];
        assert_eq!(ground_box(corners, 4., 0.125), Some([[-1., 0.], [-1., 0.]]));
        assert_eq!(ground_box(corners, 4., 13.), Some([[8., 9.], [8., 9.]]));
        assert_eq!(ground_box(corners, 11., 0.125), Some([[8., 9.], [8., 9.]]));
        assert_eq!(ground_box(corners, -3., 0.125), Some([[8., 9.], [8., 9.]]));
        assert_eq!(ground_box([Vec3::NAN; 8], 4., 0.125), None);
    }

    #[test]
    fn clipped_sides_do_not_create_edges_on_the_map_boundary() {
        assert_eq!(edge_visibility([8., 4.], [2., 1.], [16, 8]), [true; 4]);
        assert_eq!(
            edge_visibility([1., 1.], [3., 2.], [16, 8]),
            [false, true, true, false]
        );
        assert_eq!(
            edge_visibility([15., 7.], [3., 2.], [16, 8]),
            [true, false, false, true]
        );
        assert_eq!(edge_visibility([8., 4.], [8., 4.], [16, 8]), [false; 4]);
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

//! Authored CAMS poses using the existing MDX curve interpolator and clip clocks.
use pcx_mdl::{AnimTrack, Camera, NO_ID};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub eye: [f32; 3],
    pub target: [f32; 3],
    pub roll: f32,
}
struct Curve {
    track: AnimTrack,
    duration: Option<u32>,
}
pub struct CameraPlan {
    base: Pose,
    interval: [u32; 2],
    curves: Vec<Curve>,
}
impl CameraPlan {
    pub fn checked(
        camera: &Camera,
        interval: [u32; 2],
        global_durations: &[u32],
    ) -> Result<Self, &'static str> {
        if interval[1] < interval[0]
            || !camera
                .eye
                .iter()
                .chain(&camera.target)
                .all(|v| v.is_finite())
            || camera.eye == camera.target
            || !camera.field_of_view.is_finite()
            || camera.field_of_view <= 0.
            || camera.field_of_view >= std::f32::consts::PI
            || !camera.near.is_finite()
            || !camera.far.is_finite()
            || camera.near <= 0.
            || camera.far <= camera.near
        {
            return Err("invalid authored camera/clip");
        }
        let mut curves = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for input in &camera.tracks {
            let width = match &input.magic {
                b"KCTR" | b"KTTR" => 3,
                b"KCRL" => 1,
                _ => return Err("unsupported camera track"),
            };
            let stride = 4 + width
                * 4
                * if input.interpolation.has_tangents() {
                    3
                } else {
                    1
                };
            if !seen.insert(input.magic)
                || input.count == 0
                || input.width != width
                || input.stride != stride
                || (input.count as usize).checked_mul(stride) != Some(input.raw.len())
            {
                return Err("invalid camera track shape");
            }
            let duration = if input.global_seq_id == NO_ID {
                None
            } else {
                Some(
                    *global_durations
                        .get(input.global_seq_id as usize)
                        .filter(|d| **d > 0)
                        .ok_or("missing camera global duration")?,
                )
            };
            let mut previous = None;
            let mut raw = Vec::new();
            for key in input.raw.chunks_exact(stride) {
                let frame = u32::from_le_bytes(key[..4].try_into().unwrap());
                if frame > i32::MAX as u32
                    || previous.replace(frame).is_some_and(|p| frame <= p)
                    || duration.is_some_and(|d| frame > d)
                    || key[4..]
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .any(|v| !f32::from_le_bytes(*v).is_finite())
                {
                    return Err("invalid camera key/tangent");
                }
                if duration.is_some() || (interval[0]..=interval[1]).contains(&frame) {
                    raw.extend_from_slice(key);
                }
            }
            if raw.is_empty() {
                continue;
            }
            curves.push(Curve {
                track: AnimTrack {
                    magic: input.magic,
                    count: (raw.len() / stride) as u32,
                    interpolation: input.interpolation,
                    global_seq_id: input.global_seq_id,
                    raw,
                },
                duration,
            });
        }
        Ok(Self {
            base: Pose {
                eye: camera.eye,
                target: camera.target,
                roll: 0.,
            },
            interval,
            curves,
        })
    }
    pub fn sample(&self, frame: u32, elapsed_ms: u64) -> Result<Pose, &'static str> {
        let mut pose = self.base;
        for curve in &self.curves {
            let frame = curve
                .duration
                .map_or(frame.clamp(self.interval[0], self.interval[1]), |d| {
                    (elapsed_ms % u64::from(d)) as u32
                });
            match &curve.track.magic {
                b"KCTR" => {
                    let delta = pcx_mdl::sample_vector(&curve.track, frame);
                    for (p, d) in pose.eye.iter_mut().zip(delta) {
                        *p += d;
                    }
                }
                b"KTTR" => {
                    let delta = pcx_mdl::sample_vector(&curve.track, frame);
                    for (p, d) in pose.target.iter_mut().zip(delta) {
                        *p += d;
                    }
                }
                b"KCRL" => pose.roll = pcx_mdl::sample_scalar(&curve.track, frame),
                _ => unreachable!("checked camera curve"),
            }
        }
        if !pose
            .eye
            .iter()
            .chain(&pose.target)
            .chain([&pose.roll])
            .all(|v| v.is_finite())
            || pose.eye == pose.target
        {
            return Err("invalid sampled camera pose");
        }
        Ok(pose)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pcx_mdl::{Interpolation, KeyTrack};
    fn track(magic: [u8; 4], width: usize, keys: &[(u32, &[f32])]) -> KeyTrack {
        let mut raw = Vec::new();
        for (frame, values) in keys {
            raw.extend(frame.to_le_bytes());
            assert_eq!(values.len(), width);
            for value in *values {
                raw.extend(value.to_le_bytes());
            }
        }
        KeyTrack {
            magic,
            width,
            count: keys.len() as u32,
            interpolation: Interpolation::Linear,
            global_seq_id: NO_ID,
            stride: 4 + 4 * width,
            raw,
        }
    }
    fn camera() -> Camera {
        Camera {
            offset: 0,
            size: 120,
            name: "fixture".into(),
            eye: [10., 20., 30.],
            target: [1., 2., 3.],
            field_of_view: 0.7,
            near: 2.,
            far: 400.,
            tracks: vec![
                track(
                    *b"KCTR",
                    3,
                    &[
                        (10, &[0., 1., 2.]),
                        (20, &[10., 11., 12.]),
                        (50, &[900., 900., 900.]),
                    ],
                ),
                track(*b"KTTR", 3, &[(10, &[2., 4., 6.]), (20, &[4., 8., 12.])]),
                track(*b"KCRL", 1, &[(0, &[0.]), (100, &[1.])]),
            ],
        }
    }
    #[test]
    fn local_vectors_keep_source_bases_and_global_roll_keeps_its_clock() {
        let mut camera = camera();
        camera.tracks[2].global_seq_id = 0;
        let p = CameraPlan::checked(&camera, [10, 20], &[100]).unwrap();
        assert_eq!(
            p.sample(15, 125).unwrap(),
            Pose {
                eye: [15., 26., 37.],
                target: [4., 8., 12.],
                roll: 0.25
            }
        );
        assert_eq!(p.sample(40, 0).unwrap().eye, [20., 31., 42.]);
        let p = CameraPlan::checked(&camera, [30, 40], &[100]).unwrap();
        assert_eq!(p.sample(35, 0).unwrap().eye, camera.eye);
    }
    #[test]
    fn malformed_camera_tracks_and_source_bounds_refuse() {
        let c = camera();
        for changed in 0..8 {
            let mut c = c.clone();
            match changed {
                0 => c.near = 0.,
                1 => c.eye[0] = f32::NAN,
                2 => c.tracks[0].width = 1,
                3 => c.tracks[0].raw.pop().map(|_| ()).unwrap(),
                4 => c.tracks.push(c.tracks[0].clone()),
                5 => c.tracks[0].raw[0..4].copy_from_slice(&u32::MAX.to_le_bytes()),
                6 => c.tracks[0].raw[4..8].copy_from_slice(&f32::INFINITY.to_le_bytes()),
                _ => c.tracks[0].global_seq_id = 0,
            }
            assert!(
                CameraPlan::checked(&c, [10, 20], &[]).is_err(),
                "mutant {changed}"
            );
        }
    }
    #[test]
    fn tangented_camera_vectors_share_the_full_three_component_curve() {
        let mut c = camera();
        c.tracks.clear();
        let mut raw = Vec::new();
        for (frame, value) in [(0u32, [0f32; 3]), (100, [10., 20., 30.])] {
            raw.extend(frame.to_le_bytes());
            for component in value.into_iter().chain([0f32; 6]) {
                raw.extend(component.to_le_bytes());
            }
        }
        c.tracks.push(KeyTrack {
            magic: *b"KCTR",
            count: 2,
            interpolation: Interpolation::Hermite,
            global_seq_id: NO_ID,
            width: 3,
            stride: 40,
            raw,
        });
        let p = CameraPlan::checked(&c, [0, 100], &[]).unwrap();
        assert_eq!(p.sample(25, 0).unwrap().eye, [11.5625, 23.125, 34.6875]);
    }
    #[test]
    #[ignore = "requires PCX_PORTRAIT_MODEL pointing to a locally supplied original MDX portrait"]
    fn supplied_portrait_cameras_and_clips_admit_without_substitution() {
        let path = std::env::var("PCX_PORTRAIT_MODEL").unwrap();
        let bytes = std::fs::read(path).unwrap();
        let model = pcx_mdl::Mdlx::parse(&bytes).unwrap();
        let cameras = model.cameras(&bytes).unwrap();
        assert!(!cameras.is_empty());
        let durations = model
            .payload(&bytes, pcx_mdl::tag::GLBS)
            .unwrap_or_default()
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| u32::from_le_bytes(*b))
            .collect::<Vec<_>>();
        for camera in &cameras {
            for sequence in model.sequences(&bytes).unwrap() {
                let interval = [sequence.interval_start, sequence.interval_end];
                let plan = CameraPlan::checked(camera, interval, &durations).unwrap();
                for frame in [
                    interval[0],
                    interval[0] + (interval[1] - interval[0]) / 2,
                    interval[1],
                ] {
                    plan.sample(frame, u64::from(frame)).unwrap();
                }
            }
        }
    }
}

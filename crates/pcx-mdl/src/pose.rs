//! Shared local-pose sampling and clip transitions before hierarchy composition.
use crate::{Bone, BoneTrack, Node, Pose, TrackKind};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NodeTransformError {
    #[error("unsupported node transform {0:?}")]
    Unsupported([u8; 4]),
    #[error("malformed node transform {0}")]
    Track(usize),
}
impl Node {
    /// Decoder adapter: HELP and BONE use the same transform channels.
    pub fn transform_bone(&self) -> Result<Bone, NodeTransformError> {
        let encoded_len = self
            .tracks
            .iter()
            .try_fold(0usize, |n, t| n.checked_add(16)?.checked_add(t.raw.len()))
            .ok_or(NodeTransformError::Track(0))?;
        let header_len = (self.size as usize)
            .checked_sub(encoded_len)
            .ok_or(NodeTransformError::Track(0))?;
        let mut offset = self
            .offset
            .checked_add(header_len)
            .ok_or(NodeTransformError::Track(0))?;
        let mut tracks = Vec::new();
        for (index, track) in self.tracks.iter().enumerate() {
            let (kind, width) = match &track.magic {
                b"KGTR" => (TrackKind::Translation, 3),
                b"KGRT" => (TrackKind::Rotation, 4),
                b"KGSC" => (TrackKind::Scaling, 3),
                _ => return Err(NodeTransformError::Unsupported(track.magic)),
            };
            if track.width != width {
                return Err(NodeTransformError::Track(index));
            }
            tracks.push(BoneTrack {
                kind,
                count: track.count,
                interpolation: track.interpolation,
                global_sequence_id: i32::from_le_bytes(track.global_seq_id.to_le_bytes()),
                offset,
                stride: track.stride,
                raw: track.raw.clone(),
            });
            offset = offset
                .checked_add(16)
                .and_then(|n| n.checked_add(track.raw.len()))
                .ok_or(NodeTransformError::Track(index))?;
        }
        Ok(Bone {
            offset: self.offset,
            span: self.size as usize,
            name: self.name.clone(),
            object_id: self.object_id,
            parent_id: self.parent_id,
            flags: self.flags,
            tracks,
            geoset_id: -1,
            geoset_anim_id: -1,
        })
    }
}

pub fn sample_joint_pose(
    bone: &crate::Bone,
    sequence_frame: u32,
    elapsed_ms: u32,
    durations: &[u32],
    interval: [u32; 2],
) -> crate::Pose {
    if bone.tracks.iter().all(|t| t.global_sequence_id < 0) {
        let sample = |track| crate::sample_bone_track_in_interval(track, sequence_frame, interval);
        return crate::Pose {
            translation: bone
                .translation()
                .map_or([0.0; 3], |t| sample(t).translation),
            rotation: bone
                .rotation()
                .map_or([0.0, 0.0, 0.0, 1.0], |t| sample(t).rotation),
            scale: bone.scaling().map_or([1.0; 3], |t| sample(t).scale),
        };
    }
    let mut out = crate::Pose::default();
    for track in &bone.tracks {
        let frame = match u32::try_from(track.global_sequence_id) {
            Ok(id) => match durations.get(id as usize).copied().unwrap_or(0) {
                // A global sequence of length zero cannot be sampled; the clip
                // clock is the only sane fallback and is recorded as such.
                0 => sequence_frame,
                d => elapsed_ms % d,
            },
            Err(_) => sequence_frame,
        };
        let pose = if track.global_sequence_id < 0 {
            crate::sample_bone_track_in_interval(track, frame, interval)
        } else {
            crate::sample_bone_track(track, frame)
        };
        match track.kind {
            crate::TrackKind::Translation => out.translation = pose.translation,
            crate::TrackKind::Rotation => out.rotation = pose.rotation,
            crate::TrackKind::Scaling => out.scale = pose.scale,
        }
    }
    out
}

/// Clip-local channels interpolate; globally clocked channels keep their current phase.
pub fn blend_joint_pose(
    bone: &Bone,
    from: Pose,
    to: Pose,
    weight: f32,
) -> Result<Pose, &'static str> {
    if !weight.is_finite() || !(0.0..=1.0).contains(&weight) {
        return Err("invalid pose blend weight");
    }
    if from
        .translation
        .iter()
        .chain(&from.scale)
        .chain(&from.rotation)
        .chain(&to.translation)
        .chain(&to.scale)
        .chain(&to.rotation)
        .any(|v| !v.is_finite())
    {
        return Err("nonfinite blend pose");
    }
    if weight == 1.0 {
        return Ok(to);
    }
    let global = |kind| {
        bone.tracks
            .iter()
            .rev()
            .find(|t| t.kind == kind)
            .is_some_and(|t| t.global_sequence_id >= 0)
    };
    let vector = |a: [f32; 3], b: [f32; 3], kind| {
        if global(kind) {
            b
        } else {
            std::array::from_fn(|i| a[i] + (b[i] - a[i]) * weight)
        }
    };
    let rotation = if global(TrackKind::Rotation) {
        to.rotation
    } else {
        blend_rotation(from.rotation, to.rotation, weight)?
    };
    let pose = Pose {
        translation: vector(from.translation, to.translation, TrackKind::Translation),
        scale: vector(from.scale, to.scale, TrackKind::Scaling),
        rotation,
    };
    if pose
        .translation
        .iter()
        .chain(&pose.scale)
        .chain(&pose.rotation)
        .any(|v| !v.is_finite())
    {
        return Err("nonfinite blended pose");
    }
    Ok(pose)
}
fn blend_rotation(a: [f32; 4], b: [f32; 4], weight: f32) -> Result<[f32; 4], &'static str> {
    if weight == 0. || a == b {
        return Ok(a);
    }
    let normalize = |q: [f32; 4]| {
        let norm = q
            .iter()
            .map(|&v| f64::from(v) * f64::from(v))
            .sum::<f64>()
            .sqrt();
        if !norm.is_finite() || norm == 0. {
            return Err("invalid blend quaternion");
        }
        Ok(q.map(|v| f64::from(v) / norm))
    };
    let a = normalize(a)?;
    let mut b = normalize(b)?;
    let mut dot = a.iter().zip(b).map(|(a, b)| a * b).sum::<f64>();
    if dot < 0. {
        b = b.map(|v| -v);
        dot = -dot;
    }
    let angle = dot.clamp(0., 1.).acos();
    let sine = angle.sin();
    let t = f64::from(weight);
    let (left, right) = if sine == 0. {
        (1. - t, t)
    } else {
        (((1. - t) * angle).sin() / sine, (t * angle).sin() / sine)
    };
    let q = std::array::from_fn(|i| a[i] * left + b[i] * right);
    let norm = q.iter().map(|v| v * v).sum::<f64>().sqrt();
    if !norm.is_finite() || norm == 0. {
        return Err("invalid blended quaternion");
    }
    Ok(q.map(|v| (v / norm) as f32))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn joint() -> Bone {
        Bone {
            offset: 0,
            span: 0,
            name: "joint".into(),
            object_id: 7,
            parent_id: -1,
            flags: 0,
            tracks: Vec::new(),
            geoset_id: -1,
            geoset_anim_id: -1,
        }
    }
    #[test]
    fn transition_interpolates_local_channels_with_shortest_rotation() {
        let bone = joint();
        let from = Pose::default();
        let to = Pose {
            translation: [10., -4., 2.],
            scale: [3., 5., 7.],
            rotation: [0., 0., 1., 0.],
        };
        assert_eq!(blend_joint_pose(&bone, from, to, 0.).unwrap(), from);
        assert_eq!(blend_joint_pose(&bone, from, to, 1.).unwrap(), to);
        let middle = blend_joint_pose(&bone, from, to, 0.5).unwrap();
        assert_eq!(middle.translation, [5., -2., 1.]);
        assert_eq!(middle.scale, [2., 3., 4.]);
        let expected = std::f32::consts::FRAC_1_SQRT_2;
        assert!((middle.rotation[2] - expected).abs() < 1e-6);
        assert!((middle.rotation[3] - expected).abs() < 1e-6);
        let opposite = Pose {
            rotation: [0., 0., 0., -1.],
            ..from
        };
        assert_eq!(blend_joint_pose(&bone, from, opposite, 0.5).unwrap(), from);
        for weight in [-0.1, 1.1, f32::NAN] {
            assert!(blend_joint_pose(&bone, from, to, weight).is_err());
        }
        assert!(
            blend_joint_pose(
                &bone,
                from,
                Pose {
                    translation: [f32::NAN; 3],
                    ..to
                },
                1.
            )
            .is_err()
        );
    }
    #[test]
    fn helper_adapter_and_global_channels_use_the_current_global_phase() {
        let raw: Vec<u8> = [(0u32, 3f32), (100, 13.)]
            .into_iter()
            .flat_map(|(t, x)| {
                t.to_le_bytes()
                    .into_iter()
                    .chain([x, 0., 0.].into_iter().flat_map(f32::to_le_bytes))
            })
            .collect();
        let mut node = Node {
            offset: 40,
            size: (96 + 16 + raw.len()) as u32,
            name: "helper".into(),
            object_id: 7,
            parent_id: -1,
            flags: 0,
            tracks: vec![crate::KeyTrack {
                magic: *b"KGTR",
                count: 2,
                interpolation: crate::Interpolation::Linear,
                global_seq_id: 0,
                width: 3,
                stride: 16,
                raw,
            }],
        };
        let bone = node.transform_bone().unwrap();
        assert_eq!(bone.tracks[0].offset, 136);
        let current = sample_joint_pose(&bone, 2050, 150, &[100], [2000, 2100]);
        assert_eq!(current.translation, [8., 0., 0.]);
        let from = Pose {
            translation: [123., 0., 0.],
            ..Pose::default()
        };
        assert_eq!(
            blend_joint_pose(&bone, from, current, 0.)
                .unwrap()
                .translation,
            current.translation
        );
        node.tracks[0].width = 4;
        assert_eq!(node.transform_bone(), Err(NodeTransformError::Track(0)));
        node.tracks[0].width = 3;
        node.tracks[0].magic = *b"BAD!";
        assert_eq!(
            node.transform_bone(),
            Err(NodeTransformError::Unsupported(*b"BAD!"))
        );
        node.size = 1;
        assert!(node.transform_bone().is_err());
    }
}

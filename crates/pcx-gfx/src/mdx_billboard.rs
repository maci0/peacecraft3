//! Full MDX billboards with the source X-facing, Z-up camera convention.
use crate::{Skeleton, mdx_runtime::JointPlan};
use bevy::prelude::*;

// Format identifiers, not values copied from a model record.
const BILLBOARD: u32 = 0x8;
const OTHER_TRANSFORM_FLAGS: u32 = 0xf7;

struct Joint {
    id: u32,
    name: String,
    parent: Option<usize>,
    pivot: [f32; 3],
    billboard: bool,
}

/// Checked hierarchy with a camera basis that can follow authored camera motion.
/// Axis locks and inheritance flags outside a full-billboard family retain the
/// ordinary pose path's existing behavior; this plan does not implement them.
pub struct CameraPosePlan {
    joints: Vec<Joint>,
    order: Vec<usize>,
    camera: Quat,
}

impl CameraPosePlan {
    pub fn checked(plan: &JointPlan, camera: Quat) -> Result<Self, &'static str> {
        let camera = camera_basis(camera)?;
        let bones = plan.joints();
        let parents: Vec<_> = bones
            .iter()
            .map(|bone| {
                (bone.parent_id >= 0)
                    .then(|| plan.index_of(bone.parent_id as u32))
                    .flatten()
            })
            .collect();
        let mut order = Vec::with_capacity(bones.len());
        let mut done = vec![false; bones.len()];
        for start in 0..bones.len() {
            let mut chain = Vec::new();
            let mut at = Some(start);
            while let Some(i) = at {
                if done[i] {
                    break;
                }
                chain.push(i);
                at = parents[i];
            }
            for i in chain.into_iter().rev() {
                done[i] = true;
                order.push(i);
            }
        }
        // Ancestors and descendants of full billboards form the affected families.
        let mut relevant: Vec<_> = bones
            .iter()
            .map(|bone| bone.flags & BILLBOARD != 0)
            .collect();
        for &i in order.iter().rev() {
            if relevant[i]
                && let Some(parent) = parents[i]
            {
                relevant[parent] = true;
            }
        }
        let mut inside = vec![false; bones.len()];
        for &i in &order {
            inside[i] = bones[i].flags & BILLBOARD != 0 || parents[i].is_some_and(|p| inside[p]);
            if (relevant[i] || inside[i]) && bones[i].flags & OTHER_TRANSFORM_FLAGS != 0 {
                return Err("unsupported inheritance/axis-lock flags in billboard hierarchy");
            }
        }
        let joints = bones
            .iter()
            .enumerate()
            .map(|(index, bone)| Joint {
                id: bone.object_id,
                name: bone.name.clone(),
                parent: parents[index],
                pivot: plan.pivots().position(bone.object_id).unwrap(),
                billboard: bone.flags & BILLBOARD != 0,
            })
            .collect();
        Ok(Self {
            joints,
            order,
            camera,
        })
    }

    /// Update facing without rebuilding the hierarchy; invalid input preserves the current basis.
    pub fn set_camera(&mut self, camera: Quat) -> Result<(), &'static str> {
        self.camera = camera_basis(camera)?;
        Ok(())
    }

    pub fn pose(&self, poses: &[pcx_mdl::Pose]) -> Result<Skeleton, &'static str> {
        if poses.len() != self.joints.len() {
            return Err("billboard pose count differs from checked hierarchy");
        }
        let mut matrices = vec![pcx_mdl::identity(); self.joints.len()];
        let mut rotations = vec![Quat::IDENTITY; self.joints.len()];
        for &i in &self.order {
            let joint = &self.joints[i];
            let parent = joint.parent.map_or(pcx_mdl::identity(), |p| matrices[p]);
            let parent_rotation = joint.parent.map_or(Quat::IDENTITY, |p| rotations[p]);
            let local_rotation = Quat::from_array(poses[i].rotation);
            let length = local_rotation.length();
            if !length.is_finite() || length == 0.0 {
                return Err("invalid billboard hierarchy quaternion");
            }
            let mut rotation = parent_rotation * (local_rotation / length);
            let mut matrix =
                Mat4::from_cols_array(&pcx_mdl::compose(parent, poses[i], joint.pivot));
            if !matrix.is_finite() {
                return Err("nonfinite billboard hierarchy pose");
            }
            if joint.billboard {
                let pivot = matrix.transform_point3(Vec3::from(joint.pivot));
                // Keep authored scale/shear; rotation comes from the source quaternion chain.
                let correction = self.camera * parent_rotation.conjugate();
                matrix = Mat4::from_rotation_translation(correction, pivot - correction * pivot)
                    * matrix;
                rotation = correction * rotation;
            }
            if !matrix.is_finite() {
                return Err("nonfinite camera-facing pose");
            }
            matrices[i] = matrix.to_cols_array();
            rotations[i] = rotation.normalize();
        }
        Ok(Skeleton {
            matrices,
            ids: self.joints.iter().map(|joint| joint.id).collect(),
            names: self.joints.iter().map(|joint| joint.name.clone()).collect(),
        })
    }
}

fn camera_basis(camera: Quat) -> Result<Quat, &'static str> {
    if !camera.is_finite() || (camera.length_squared() - 1.0).abs() > 1e-5 {
        return Err("invalid billboard camera rotation");
    }
    // Bevy camera columns are right/up/toward-eye; MDX faces local +X.
    let basis = Mat3::from_quat(camera);
    Ok(Quat::from_mat3(&Mat3::from_cols(basis.z_axis, basis.x_axis, basis.y_axis)).normalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pcx_mdl::{Bone, Pivots, Pose};

    fn bone(id: u32, parent: i32, flags: u32) -> Bone {
        Bone {
            offset: 0,
            span: 104,
            name: format!("synthetic-{id}"),
            object_id: id,
            parent_id: parent,
            flags,
            tracks: vec![],
            geoset_id: -1,
            geoset_anim_id: -1,
        }
    }
    fn pivots() -> Pivots {
        Pivots(vec![[0.0; 3]; 16])
    }
    fn near(a: Vec3, b: Vec3) {
        assert!((a - b).length() < 1e-4, "{a:?} != {b:?}");
    }

    #[test]
    fn animated_camera_updates_facing_and_refuses_bad_updates_atomically() {
        let joints = JointPlan::checked(&[bone(3, -1, BILLBOARD)], &[], &pivots()).unwrap();
        let mut plan = CameraPosePlan::checked(&joints, Quat::IDENTITY).unwrap();
        let camera = Quat::from_rotation_y(0.37) * Quat::from_rotation_z(-0.21);
        plan.set_camera(camera).unwrap();
        let first = plan.pose(&[Pose::default()]).unwrap();
        near(
            Mat4::from_cols_array(&first.matrices[0]).transform_vector3(Vec3::X),
            camera * Vec3::Z,
        );
        assert!(plan.set_camera(Quat::from_xyzw(0., 0., 0., 0.)).is_err());
        assert_eq!(
            plan.pose(&[Pose::default()]).unwrap().matrices,
            first.matrices
        );
    }

    #[test]
    fn nested_billboards_face_the_camera_once_and_keep_their_pivots() {
        let bones = [bone(9, 3, BILLBOARD), bone(3, -1, BILLBOARD)];
        let mut pivots = pivots();
        for (id, point) in [(9, [2.0f32, 0.0, 3.0]), (3, [1.0, 0.0, 0.0])] {
            pivots.0[id] = point;
        }
        let joints = JointPlan::checked(&bones, &[], &pivots).unwrap();
        let poses = [Pose::default(); 2];
        for camera in [
            Quat::IDENTITY,
            Quat::from_rotation_y(0.7),
            Quat::from_rotation_z(-0.4),
        ] {
            let plan = CameraPosePlan::checked(&joints, camera).unwrap();
            let skeleton = plan.pose(&poses).unwrap();
            let parent = Mat4::from_cols_array(&skeleton.matrices[1]);
            let child = Mat4::from_cols_array(&skeleton.matrices[0]);
            near(child.transform_vector3(Vec3::X), camera * Vec3::Z);
            near(parent.transform_vector3(Vec3::X), camera * Vec3::Z);
            near(parent.transform_point3(Vec3::X), Vec3::X);
            near(
                child.transform_point3(Vec3::new(2.0, 0.0, 3.0)),
                parent.transform_point3(Vec3::new(2.0, 0.0, 3.0)),
            );
        }
    }
    #[test]
    fn plain_sparse_hierarchy_keeps_exact_existing_composition() {
        let bones = [bone(9, 3, 0), bone(3, -1, 0), bone(12, 9, 0)];
        let pivots = pivots();
        let joints = JointPlan::checked(&bones, &[], &pivots).unwrap();
        let poses = [
            Pose {
                translation: [2.0, -1.0, 3.0],
                rotation: Quat::from_rotation_x(0.4).to_array(),
                scale: [2.0, 3.0, 4.0],
            },
            Pose {
                translation: [1.0, 5.0, -2.0],
                rotation: Quat::from_rotation_z(-0.7).to_array(),
                ..Pose::default()
            },
            Pose::default(),
        ];
        let expected = pcx_mdl::bone_matrices_from_poses(
            &bones,
            &|id| {
                bones
                    .iter()
                    .find(|b| b.object_id == id)
                    .and_then(|b| (b.parent_id >= 0).then_some(b.parent_id as u32))
            },
            &pivots,
            &poses,
        );
        let actual = CameraPosePlan::checked(&joints, Quat::from_rotation_y(0.3))
            .unwrap()
            .pose(&poses)
            .unwrap();
        assert_eq!(actual.matrices, expected);
        assert_eq!(actual.ids, [9, 3, 12]);
    }

    #[test]
    fn parent_rotation_is_cancelled_but_local_spin_and_scale_survive() {
        let bones = [bone(3, -1, 0), bone(9, 3, BILLBOARD)];
        let joints = JointPlan::checked(&bones, &[], &pivots()).unwrap();
        let spin = Quat::from_rotation_x(0.4);
        let poses = [
            Pose {
                translation: [3.0, 2.0, -1.0],
                rotation: Quat::from_rotation_z(0.8).to_array(),
                scale: [2.0; 3],
            },
            Pose {
                rotation: spin.to_array(),
                scale: [1.0, 3.0, 4.0],
                ..Pose::default()
            },
        ];
        let camera = Quat::from_rotation_y(-0.6);
        let plan = CameraPosePlan::checked(&joints, camera).unwrap();
        let skeleton = plan.pose(&poses).unwrap();
        let m = Mat4::from_cols_array(&skeleton.matrices[1]);
        near(m.transform_vector3(Vec3::X).normalize(), camera * Vec3::Z);
        near(m.transform_point3(Vec3::ZERO), Vec3::new(3.0, 2.0, -1.0));
        let basis = Mat3::from_quat(camera);
        let facing = Quat::from_mat3(&Mat3::from_cols(basis.z_axis, basis.x_axis, basis.y_axis));
        near(
            m.transform_vector3(Vec3::Y),
            facing * (spin * Vec3::Y) * 6.0,
        );
        near(
            m.transform_vector3(Vec3::Z),
            facing * (spin * Vec3::Z) * 8.0,
        );
    }

    #[test]
    fn malformed_camera_pose_and_unsupported_affected_flags_refuse() {
        let joints = JointPlan::checked(&[bone(3, -1, BILLBOARD)], &[], &pivots()).unwrap();
        for camera in [
            Quat::from_xyzw(0.0, 0.0, 0.0, 0.0),
            Quat::from_xyzw(f32::NAN, 0.0, 0.0, 1.0),
        ] {
            assert!(CameraPosePlan::checked(&joints, camera).is_err());
        }
        let plan = CameraPosePlan::checked(&joints, Quat::IDENTITY).unwrap();
        assert!(plan.pose(&[]).is_err());
        assert!(
            plan.pose(&[Pose {
                translation: [f32::INFINITY, 0.0, 0.0],
                ..Pose::default()
            }])
            .is_err()
        );
        assert!(
            plan.pose(&[Pose {
                rotation: [0.0; 4],
                ..Pose::default()
            }])
            .is_err()
        );
        for bones in [
            vec![bone(3, -1, BILLBOARD | 0x10)],
            vec![bone(3, -1, 0x40), bone(9, 3, BILLBOARD)],
            vec![bone(3, -1, BILLBOARD), bone(9, 3, 0x40)],
        ] {
            let joints = JointPlan::checked(&bones, &[], &pivots()).unwrap();
            assert!(CameraPosePlan::checked(&joints, Quat::IDENTITY).is_err());
        }
    }

    #[test]
    fn unrelated_axis_lock_retains_the_ordinary_pose_path() {
        let bones = [bone(3, -1, 0), bone(9, 3, BILLBOARD), bone(12, 3, 0x40)];
        let joints = JointPlan::checked(&bones, &[], &pivots()).unwrap();
        let poses = [Pose::default(); 3];
        let plan = CameraPosePlan::checked(&joints, Quat::IDENTITY).unwrap();
        let skeleton = plan.pose(&poses).unwrap();
        assert_eq!(skeleton.matrices[2], pcx_mdl::identity());
    }

    #[test]
    fn signed_scale_and_shear_survive_rotation_cancellation() {
        let bones = [bone(3, -1, 0), bone(9, 3, 0), bone(12, 9, BILLBOARD)];
        let joints = JointPlan::checked(&bones, &[], &pivots()).unwrap();
        let plan = CameraPosePlan::checked(&joints, Quat::IDENTITY).unwrap();
        let spin = Quat::from_rotation_z(0.4);
        let facing = Quat::from_mat3(&Mat3::from_cols(Vec3::Z, Vec3::X, Vec3::Y));
        for scale in [[0.0, 1.0, 1.0], [-2.0, 1.0, 3.0], [2.0, 1.0, 3.0]] {
            let poses = [
                Pose {
                    scale,
                    ..Pose::default()
                },
                Pose {
                    rotation: spin.to_array(),
                    ..Pose::default()
                },
                Pose::default(),
            ];
            let skeleton = plan.pose(&poses).unwrap();
            let actual = Mat4::from_cols_array(&skeleton.matrices[2]);
            for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
                near(
                    actual.transform_vector3(axis),
                    facing * (spin.conjugate() * (Vec3::from(scale) * (spin * axis))),
                );
            }
            near(actual.transform_point3(Vec3::ZERO), Vec3::ZERO);
        }
    }
}

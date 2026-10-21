//! Scripted aiming on the existing source X-facing, Z-up skeleton.
use crate::Skeleton;
use bevy::prelude::*;

/// Correct one posed bone and its descendants, preserving its pivot and scale.
/// `target` is already in model space. A coincident target has no direction.
pub fn apply(
    skeleton: &mut Skeleton,
    bones: &[pcx_mdl::Bone],
    pivots: &pcx_mdl::Pivots,
    bone: &str,
    target: Vec3,
) -> Result<(), &'static str> {
    if !target.is_finite() || skeleton.matrices.len() != bones.len() {
        return Err("invalid look-at target or skeleton shape");
    }
    let mut matching = bones
        .iter()
        .enumerate()
        .filter(|(_, b)| b.name.eq_ignore_ascii_case(bone));
    let (index, joint) = matching.next().ok_or("look-at bone is missing")?;
    if matching.next().is_some() {
        return Err("look-at bone is ambiguous");
    }
    let matrix = Mat4::from_cols_array(&skeleton.matrices[index]);
    let pivot = matrix.transform_point3(Vec3::from(
        pivots
            .position(joint.object_id)
            .ok_or("look-at pivot is missing")?,
    ));
    let direction = target - pivot;
    if direction == Vec3::ZERO {
        return Ok(());
    }
    let forward = matrix
        .x_axis
        .truncate()
        .try_normalize()
        .ok_or("degenerate look-at bone transform")?;
    let direction = direction
        .try_normalize()
        .ok_or("invalid look-at direction")?;
    // shortcut: immediate minimal rotation; qualify native turn limits and blending before claiming pose parity.
    let rotation = Quat::from_rotation_arc(forward, direction);
    let correction = Mat4::from_rotation_translation(rotation, pivot - rotation * pivot);
    if !correction.is_finite() {
        return Err("nonfinite look-at correction");
    }
    let mut children = std::collections::BTreeMap::<i32, Vec<usize>>::new();
    for (i, bone) in bones.iter().enumerate() {
        children.entry(bone.parent_id).or_default().push(i);
    }
    let mut pending = vec![index];
    let mut visited = vec![false; bones.len()];
    while let Some(i) = pending.pop() {
        if std::mem::replace(&mut visited[i], true) {
            continue;
        }
        skeleton.matrices[i] =
            (correction * Mat4::from_cols_array(&skeleton.matrices[i])).to_cols_array();
        if let Ok(id) = i32::try_from(bones[i].object_id)
            && let Some(descendants) = children.get(&id)
        {
            pending.extend(descendants);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bone(id: u32, parent: i32) -> pcx_mdl::Bone {
        pcx_mdl::Bone {
            offset: 0,
            span: 0,
            name: format!("joint{id}"),
            object_id: id,
            parent_id: parent,
            flags: 0,
            tracks: vec![],
            geoset_id: -1,
            geoset_anim_id: -1,
        }
    }
    #[test]
    fn aiming_preserves_pivot_and_moves_descendants_without_turning_the_root() {
        let bones = vec![bone(0, -1), bone(1, 0), bone(2, 1)];
        let pivots = pcx_mdl::Pivots(vec![[0.; 3], [2., 0., 0.], [3., 0., 0.]]);
        let rest = vec![pcx_mdl::identity(); 3];
        let mut skeleton = Skeleton {
            matrices: rest.clone(),
            ids: vec![0, 1, 2],
            names: bones.iter().map(|b| b.name.clone()).collect(),
        };
        apply(
            &mut skeleton,
            &bones,
            &pivots,
            "JOINT1",
            Vec3::new(2., 10., 0.),
        )
        .unwrap();
        assert_eq!(skeleton.matrices[0], rest[0]);
        let head = Mat4::from_cols_array(&skeleton.matrices[1]);
        assert!(
            head.transform_point3(Vec3::new(2., 0., 0.))
                .abs_diff_eq(Vec3::new(2., 0., 0.), 1e-5)
        );
        assert!(head.x_axis.truncate().abs_diff_eq(Vec3::Y, 1e-5));
        let child = Mat4::from_cols_array(&skeleton.matrices[2]);
        assert!(
            child
                .transform_point3(Vec3::new(3., 0., 0.))
                .abs_diff_eq(Vec3::new(2., 1., 0.), 1e-5)
        );
        let mesh = crate::LoadedMesh {
            positions: vec![[3., 0., 0.]],
            normals: vec![],
            tex_coords: vec![],
            indices: vec![],
            vertex_groups: vec![0],
            bone_indices: vec![2],
            group_counts: vec![1],
            geoset: 0,
            material_id: 0,
        };
        assert!(
            Vec3::from(crate::skin_mesh(&mesh, &skeleton)[0])
                .abs_diff_eq(Vec3::new(2., 1., 0.), 1e-5)
        );
        let mut reset = Skeleton {
            matrices: rest.clone(),
            ids: vec![0, 1, 2],
            names: vec![],
        };
        apply(
            &mut reset,
            &bones,
            &pivots,
            "joint1",
            Vec3::new(12., 0., 0.),
        )
        .unwrap();
        assert_eq!(reset.matrices, rest);
    }
    #[test]
    fn malformed_targets_and_bones_leave_the_pose_unchanged() {
        let bones = vec![bone(0, -1)];
        let pivots = pcx_mdl::Pivots(vec![[0.; 3]]);
        let mut skeleton = Skeleton {
            matrices: vec![pcx_mdl::identity()],
            ids: vec![0],
            names: vec![],
        };
        let before = skeleton.matrices.clone();
        assert!(apply(&mut skeleton, &bones, &pivots, "missing", Vec3::X).is_err());
        assert!(apply(&mut skeleton, &bones, &pivots, "joint0", Vec3::NAN).is_err());
        apply(&mut skeleton, &bones, &pivots, "joint0", Vec3::ZERO).unwrap();
        assert_eq!(skeleton.matrices, before);
    }
}

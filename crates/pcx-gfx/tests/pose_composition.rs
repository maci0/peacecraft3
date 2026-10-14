use pcx_gfx::{LoadedMesh, LoadedModel, Skeleton};
use pcx_mdl::{Bone, Pivots, Pose};
fn legacy_pose_from_poses(
    _model: &LoadedModel,
    bones: &[pcx_mdl::Bone],
    pivots: &pcx_mdl::Pivots,
    poses: &[pcx_mdl::Pose],
) -> Skeleton {
    let keyed: Vec<pcx_mdl::Bone> = bones
        .iter()
        .zip(
            poses
                .iter()
                .copied()
                .chain(std::iter::repeat(pcx_mdl::Pose::default())),
        )
        .map(|(b, pose)| one_key_bone(b, pose))
        .collect();
    let by_id: std::collections::HashMap<u32, usize> = keyed
        .iter()
        .enumerate()
        .map(|(i, b)| (b.object_id, i))
        .collect();
    let parent = |object_id: u32| -> Option<u32> {
        let b = keyed.iter().find(|b| b.object_id == object_id)?;
        if b.parent_id < 0 {
            return None;
        }
        let p = b.parent_id as u32;
        by_id.contains_key(&p).then_some(p)
    };
    let matrices = pcx_mdl::bone_matrices(&keyed, &parent, pivots, 0);
    Skeleton {
        matrices,
        ids: bones.iter().map(|b| b.object_id).collect(),
        names: bones.iter().map(|joint| joint.name.clone()).collect(),
    }
}

fn one_key_bone(bone: &pcx_mdl::Bone, pose: pcx_mdl::Pose) -> pcx_mdl::Bone {
    let track = |kind: pcx_mdl::TrackKind, value: [f32; 4]| {
        let mut raw = Vec::with_capacity(20);
        raw.extend_from_slice(&0u32.to_le_bytes());
        for v in value {
            raw.extend_from_slice(&v.to_le_bytes());
        }
        pcx_mdl::BoneTrack {
            kind,
            count: 1,
            interpolation: pcx_mdl::Interpolation::None,
            global_sequence_id: -1,
            offset: 0,
            stride: 20,
            raw,
        }
    };
    pcx_mdl::Bone {
        offset: bone.offset,
        span: bone.span,
        name: bone.name.clone(),
        object_id: bone.object_id,
        parent_id: bone.parent_id,
        flags: bone.flags,
        tracks: vec![
            track(
                pcx_mdl::TrackKind::Translation,
                [
                    pose.translation[0],
                    pose.translation[1],
                    pose.translation[2],
                    0.0,
                ],
            ),
            track(pcx_mdl::TrackKind::Rotation, pose.rotation),
            track(
                pcx_mdl::TrackKind::Scaling,
                [pose.scale[0], pose.scale[1], pose.scale[2], 1.0],
            ),
        ],
        geoset_id: bone.geoset_id,
        geoset_anim_id: bone.geoset_anim_id,
    }
}

fn bone(id: u32, parent: i32) -> Bone {
    Bone {
        offset: 13,
        span: 37,
        name: format!("joint-{id}"),
        object_id: id,
        parent_id: parent,
        flags: 17,
        tracks: vec![],
        geoset_id: -1,
        geoset_anim_id: -1,
    }
}
fn model() -> LoadedModel {
    LoadedModel {
        name: "synthetic".into(),
        minimum_extent: [-1.0; 3],
        maximum_extent: [2.0; 3],
        meshes: vec![],
        bone_names: vec![],
        bone_parents: vec![],
        sequences: vec![],
    }
}
fn mesh(ids: &[u32]) -> LoadedMesh {
    LoadedMesh {
        positions: vec![[1.0, 2.0, 3.0]; ids.len() + 1],
        normals: vec![[0.0, 0.0, 1.0]; ids.len() + 1],
        tex_coords: vec![[0.0; 2]; ids.len() + 1],
        indices: vec![],
        vertex_groups: (0..=ids.len()).map(|i| i as u8).collect(),
        bone_indices: ids.to_vec(),
        group_counts: vec![1; ids.len()],
        geoset: 0,
        material_id: 0,
    }
}
fn bits(v: impl IntoIterator<Item = f32>) -> Vec<u32> {
    v.into_iter().map(f32::to_bits).collect()
}
fn compare(bones: &[Bone], pivots: &Pivots, poses: &[Pose]) {
    let before = bones.to_vec();
    let pose_bits = bits(
        poses
            .iter()
            .flat_map(|p| p.translation.into_iter().chain(p.rotation).chain(p.scale)),
    );
    let model = model();
    let expected = legacy_pose_from_poses(&model, bones, pivots, poses);
    let actual = pcx_gfx::pose_from_poses(&model, bones, pivots, poses);
    assert_eq!(
        bits(actual.matrices.iter().flatten().copied()),
        bits(expected.matrices.iter().flatten().copied())
    );
    assert_eq!(actual.ids, expected.ids);
    assert_eq!(actual.names, expected.names);
    let mesh = mesh(&actual.ids);
    assert_eq!(
        bits(pcx_gfx::skin_mesh(&mesh, &actual).into_iter().flatten()),
        bits(pcx_gfx::skin_mesh(&mesh, &expected).into_iter().flatten())
    );
    assert_eq!(bones, before);
    assert_eq!(
        pose_bits,
        bits(
            poses
                .iter()
                .flat_map(|p| p.translation.into_iter().chain(p.rotation).chain(p.scale))
        )
    );
}
fn run_cases() -> u32 {
    let poses = [
        Pose {
            translation: [3.0, -4.0, 5.0],
            rotation: [0.0, 0.0, 0.5, 0.75],
            scale: [2.0, 3.0, 4.0],
        },
        Pose {
            translation: [-6.0, 7.0, 8.0],
            rotation: [0.25, 0.5, 0.0, 0.75],
            scale: [5.0, 6.0, 7.0],
        },
        Pose {
            translation: [9.0, 10.0, -11.0],
            ..Pose::default()
        },
    ];
    let pivots = Pivots((0..32).map(|i| [i as f32, -(i as f32), 0.25]).collect());
    compare(&[], &Pivots(vec![]), &poses);
    compare(&[bone(9, 2), bone(2, -1)], &pivots, &poses[..2]);
    compare(&[bone(9, 31), bone(2, -2)], &pivots, &poses[..2]);
    compare(&[bone(2, 2)], &pivots, &poses[..1]);
    compare(&[bone(2, 3), bone(3, 2)], &pivots, &poses[..2]);
    compare(&[bone(7, 2), bone(2, 3), bone(3, 2)], &pivots, &poses);
    compare(&[bone(2, -1), bone(2, 3), bone(3, 2)], &pivots, &poses);
    compare(&[bone(9, 2), bone(2, -1)], &pivots, &[]);
    compare(&[bone(9, 2), bone(2, -1)], &pivots, &poses[..1]);
    compare(&[bone(9, 2), bone(2, -1)], &Pivots(vec![]), &poses);
    let special = [Pose {
        translation: [-0.0, f32::INFINITY, f32::from_bits(0x7fc01234)],
        rotation: [-0.0, 0.0, 0.0, 1.0],
        scale: [-0.0, 1.0, 1.0],
    }];
    compare(&[bone(2, -1)], &Pivots(vec![]), &special);
    let cyclic = pcx_gfx::pose_from_poses(&model(), &[bone(2, 3), bone(3, 2)], &pivots, &poses);
    assert!(
        cyclic
            .matrices
            .iter()
            .flatten()
            .all(|value| value.to_bits() == 0)
    );
    11
}
#[test]
fn supplied_pose_adapter_preserves_exact_legacy_boundary_outputs() {
    assert_eq!(run_cases(), 11);
}

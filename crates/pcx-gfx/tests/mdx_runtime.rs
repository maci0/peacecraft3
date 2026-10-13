//! Leaf tests can run without Bevy by compiling this test file directly.
#[path = "../src/mdx_opacity.rs"]
pub mod mdx_opacity;
#[path = "../src/mdx_runtime.rs"]
pub mod mdx_runtime;
use mdx_runtime::{GeosetInput, JointKind, JointPlan, PlanError, RuntimeInput, RuntimePlan};
use pcx_mdl::{
    Bone, GeosetAnim, Interpolation, KeyTrack, Material, NO_ID, Node, Pivots, Texture, TextureAnim,
};

fn bone(id: u32, parent: i32) -> Bone {
    Bone {
        offset: 0,
        span: 104,
        name: format!("bone{id}"),
        object_id: id,
        parent_id: parent,
        flags: 0x100,
        tracks: vec![],
        geoset_id: -1,
        geoset_anim_id: -1,
    }
}
fn helper(id: u32, parent: i32) -> Node {
    let mut raw = Vec::new();
    for (time, value) in [(0u32, [0f32, 0., 0.]), (100, [8., 0., 0.])] {
        raw.extend(time.to_le_bytes());
        for component in value {
            raw.extend(component.to_le_bytes());
        }
    }
    Node {
        offset: 0,
        size: 144,
        name: format!("helper{id}"),
        object_id: id,
        parent_id: parent,
        flags: 0,
        tracks: vec![KeyTrack {
            magic: *b"KGTR",
            count: 2,
            interpolation: Interpolation::Linear,
            global_seq_id: NO_ID,
            width: 3,
            stride: 16,
            raw,
        }],
    }
}
fn pivots() -> Pivots {
    Pivots(vec![[0.; 3]; 10])
}

#[test]
fn bone_under_animated_helper_inherits_the_authored_helper_track() {
    let plan = JointPlan::checked(&[bone(7, 2)], &[helper(2, -1)], &pivots()).unwrap();
    assert_eq!(
        plan.joints()
            .iter()
            .map(|joint| joint.object_id)
            .collect::<Vec<_>>(),
        [7, 2]
    );
    assert_eq!(plan.kinds(), [JointKind::Bone, JointKind::Helper]);
    assert_eq!(plan.index_of(7), Some(0));
    let sampled = plan.sample_decoder_frame(50);
    assert_eq!(sampled.object_ids, [7, 2]);
    assert_eq!(
        sampled.matrices[0][12], 4.,
        "a dropped helper would leave this bone at zero"
    );
    assert_eq!(sampled.matrices[1][12], 4.);
    assert_eq!(plan.sample_decoder_frame(100).matrices[0][12], 8.);
}

#[test]
fn invalid_graphs_and_missing_pivots_are_errors_not_rest_poses() {
    assert!(matches!(
        JointPlan::checked(&[bone(2, -1)], &[helper(2, -1)], &pivots()),
        Err(PlanError::DuplicateJoint(2))
    ));
    assert!(matches!(
        JointPlan::checked(&[bone(7, 2)], &[], &pivots()),
        Err(PlanError::MissingParent {
            joint: 7,
            parent: 2
        })
    ));
    assert!(matches!(
        JointPlan::checked(&[bone(7, 2)], &[helper(2, 7)], &pivots()),
        Err(PlanError::Cycle(_))
    ));
    assert!(matches!(
        JointPlan::checked(&[bone(7, -2)], &[], &pivots()),
        Err(PlanError::InvalidParent { .. })
    ));
    assert!(matches!(
        JointPlan::checked(&[bone(7, -1)], &[], &Pivots(vec![])),
        Err(PlanError::MissingPivot(7))
    ));
    let mut malformed = helper(2, -1);
    malformed.tracks[0].raw.pop();
    assert!(matches!(
        JointPlan::checked(&[], &[malformed], &pivots()),
        Err(PlanError::TrackShape { .. })
    ));
}
fn material() -> Material {
    let mut data = Vec::new();
    data.extend((-4i32).to_le_bytes());
    data.extend(0x20u32.to_le_bytes());
    data.extend(b"LAYS");
    data.extend(2u32.to_le_bytes());
    for (alpha, txan) in [(0.25f32, NO_ID), (0.75, 0)] {
        // The second layer also carries authored alpha and texture tracks.
        let mut tracks = Vec::new();
        if txan != NO_ID {
            tracks.extend(b"KMTA");
            for word in [1u32, 1, NO_ID, 30] {
                tracks.extend(word.to_le_bytes());
            }
            tracks.extend(0.5f32.to_le_bytes());
            tracks.extend(b"KMTF");
            for word in [1u32, 1, NO_ID, 30, 1] {
                tracks.extend(word.to_le_bytes());
            }
        }
        for word in [28 + tracks.len() as u32, 2, 16, 0, txan, 0] {
            data.extend(word.to_le_bytes());
        }
        data.extend(alpha.to_le_bytes());
        data.extend(tracks);
    }
    Material {
        offset: 0,
        len: data.len() as u32 + 4,
        data,
    }
}
fn animation(geoset_id: u32, alpha: f32) -> GeosetAnim {
    GeosetAnim {
        offset: 0,
        len: 28,
        flags: 0,
        geoset_id,
        static_alpha: alpha,
        static_colour: [1.; 3],
        tracks: vec![],
        trailing: 0,
    }
}
fn input() -> RuntimeInput {
    RuntimeInput {
        bones: vec![bone(7, 2)],
        helpers: vec![helper(2, -1)],
        pivots: pivots(),
        geosets: vec![
            GeosetInput {
                vertex_count: 4,
                material_id: 0,
                matrix_joint_ids: vec![7],
            },
            GeosetInput {
                vertex_count: 4,
                material_id: 0,
                matrix_joint_ids: vec![2],
            },
            GeosetInput {
                vertex_count: 9,
                material_id: 0,
                matrix_joint_ids: vec![7],
            },
        ],
        geoset_animations: vec![animation(2, 0.25), animation(NO_ID, 0.75)],
        materials: vec![material()],
        textures: vec![
            Texture {
                slot: 0,
                path: "syntheticA".into(),
                flags: 0,
            },
            Texture {
                slot: 0,
                path: "syntheticB".into(),
                flags: 0,
            },
        ],
        texture_animations: vec![TextureAnim {
            offset: 0,
            len: 4,
            tracks: vec![],
        }],
        global_sequences: vec![],
    }
}
#[test]
fn sparse_geoa_uses_actual_geoset_id_and_all_material_layers_survive() {
    let plan = RuntimePlan::checked(input()).unwrap();
    assert!(plan.geosets[0].animation_records.is_empty());
    assert!(plan.geosets[1].animation_records.is_empty());
    assert_eq!(plan.geosets[2].animation_records, [0]);
    assert_eq!(plan.unbound_geoset_animations, [1]);
    assert_eq!(plan.materials[0].priority_plane, -4);
    assert_eq!(plan.materials[0].flags, 0x20);
    let layers = &plan.materials[0].layers;
    assert_eq!(layers.len(), 2);
    assert_eq!(layers[0].properties.static_alpha, 0.25);
    assert_eq!(layers[1].properties.static_alpha, 0.75);
    assert_eq!(layers[1].alpha_tracks.len(), 1);
    assert_eq!(layers[1].texture_tracks[0].keys, [(30, 1)]);
    assert_eq!(layers[1].properties.texture_anim_id, 0);
    assert_eq!(plan.duplicate_vertex_counts().get(&4).unwrap(), &[0, 1]);
}

#[test]
fn geoset_opacity_uses_sparse_ids_and_preserves_unbound_records() {
    let plan = RuntimePlan::checked(input()).unwrap();
    let opacity = mdx_opacity::OpacityPlan::checked(&plan, [0, 100]).unwrap();
    assert_eq!(opacity.sample(0, 50, 50), Some(0.25));
    assert_eq!(opacity.sample(1, 50, 50), Some(0.25));
    assert_eq!(opacity.sample(2, 50, 50), Some(0.0625));
    assert_eq!(opacity.sample(3, 50, 50), None);
}

fn alpha_track(keys: &[(u32, f32)], global_seq_id: u32) -> pcx_mdl::AnimTrack {
    let raw = keys
        .iter()
        .flat_map(|&(frame, alpha)| frame.to_le_bytes().into_iter().chain(alpha.to_le_bytes()))
        .collect();
    pcx_mdl::AnimTrack {
        magic: *b"KGAO",
        count: keys.len() as u32,
        interpolation: Interpolation::Linear,
        global_seq_id,
        raw,
    }
}

#[test]
fn local_alpha_keys_do_not_leak_from_another_sequence() {
    let mut data = input();
    data.geoset_animations[0].tracks = vec![alpha_track(&[(0, 0.1), (100, 0.9)], NO_ID)];
    let plan = RuntimePlan::checked(data).unwrap();
    let opacity = mdx_opacity::OpacityPlan::checked(&plan, [1000, 2000]).unwrap();
    assert_eq!(
        opacity.sample(2, 1500, 50),
        Some(0.0625),
        "no local keys means authored static alpha"
    );
}

#[test]
fn global_alpha_uses_its_own_clock_and_rejects_ambiguous_or_invalid_data() {
    let mut data = input();
    data.global_sequences = vec![1000];
    data.geoset_animations[0].tracks = vec![alpha_track(&[(0, 0.0), (1000, 1.0)], 0)];
    let mut plan = RuntimePlan::checked(data).unwrap();
    let opacity = mdx_opacity::OpacityPlan::checked(&plan, [2000, 3000]).unwrap();
    assert_eq!(opacity.sample(2, 2500, 1250), Some(0.0625));
    plan.global_sequences[0] = 0;
    assert!(mdx_opacity::OpacityPlan::checked(&plan, [2000, 3000]).is_err());
    plan.global_sequences[0] = 1000;
    plan.geosets[2].animation_records.push(0);
    assert!(mdx_opacity::OpacityPlan::checked(&plan, [2000, 3000]).is_err());
    plan.geosets[2].animation_records.pop();
    plan.geoset_animations[0].tracks[0].raw.pop();
    assert!(mdx_opacity::OpacityPlan::checked(&plan, [2000, 3000]).is_err());
}
#[test]
fn missing_matrix_material_texture_and_geoa_references_are_errors() {
    let mut data = input();
    data.geosets[0].matrix_joint_ids = vec![9];
    assert!(matches!(
        RuntimePlan::checked(data),
        Err(PlanError::MissingMatrixJoint {
            geoset: 0,
            joint: 9
        })
    ));
    let mut data = input();
    data.geosets[0].material_id = 1;
    assert!(matches!(
        RuntimePlan::checked(data),
        Err(PlanError::Reference { .. })
    ));
    let mut data = input();
    data.textures.clear();
    assert!(matches!(
        RuntimePlan::checked(data),
        Err(PlanError::Reference { .. })
    ));
    let mut data = input();
    data.geoset_animations[0].geoset_id = 3;
    assert!(matches!(
        RuntimePlan::checked(data),
        Err(PlanError::Reference { .. })
    ));
}

#[test]
#[ignore = "requires PEACECRAFT3_MDX_INSTALL, PEACECRAFT3_MDX_MODEL and PEACECRAFT3_MDX_SOURCE"]
fn original_model_has_a_checked_typed_runtime_plan() {
    let install = std::env::var("PEACECRAFT3_MDX_INSTALL").expect("set PEACECRAFT3_MDX_INSTALL");
    let model = std::env::var("PEACECRAFT3_MDX_MODEL").expect("set PEACECRAFT3_MDX_MODEL");
    let source =
        std::env::var("PEACECRAFT3_MDX_SOURCE").expect("set PEACECRAFT3_MDX_SOURCE explicitly");
    assert!(!source.is_empty(), "fixture provenance must not be empty");
    let mut archives = pcx_mpq::MpqSet::open_wc3(&install).expect("open original install");
    let bytes = archives
        .extract(&model)
        .unwrap_or_else(|error| panic!("{model}: {error}"));
    let decoded = pcx_mdl::Mdlx::parse(&bytes).unwrap();
    let bones = decoded.bones(&bytes).unwrap();
    let helpers = decoded.nodes(&bytes, pcx_mdl::tag::HELP).unwrap();
    let plan = RuntimePlan::parse(&bytes).unwrap_or_else(|error| panic!("{model}: {error}"));
    assert_eq!(plan.joints.joints().len(), bones.len() + helpers.len());
    assert!(
        !plan.geosets.is_empty(),
        "fixture must exercise mesh bindings"
    );
    for (index, animation) in plan.geoset_animations.iter().enumerate() {
        if animation.geoset_id == NO_ID {
            assert!(plan.unbound_geoset_animations.contains(&index));
        } else {
            assert!(
                plan.geosets[animation.geoset_id as usize]
                    .animation_records
                    .contains(&index)
            );
        }
    }
    let helper_ids: std::collections::BTreeSet<_> =
        helpers.iter().map(|helper| helper.object_id).collect();
    let bones_under_helpers = bones
        .iter()
        .filter(|bone| bone.parent_id >= 0 && helper_ids.contains(&(bone.parent_id as u32)))
        .count();
    let helper_tracks = helpers
        .iter()
        .map(|helper| helper.tracks.len())
        .sum::<usize>();
    eprintln!(
        "{model}: bones_under_helpers={bones_under_helpers} retained_helper_tracks={helper_tracks}"
    );
    let decoded_geosets = decoded.geosets(&bytes).unwrap();
    let duplicate_counts = plan.duplicate_vertex_counts();
    for (vertices, geosets) in &duplicate_counts {
        let first = decoded.mesh(&bytes, &decoded_geosets[geosets[0]]).unwrap();
        let mut different_positions = false;
        let mut different_skin_bindings = false;
        for &index in &geosets[1..] {
            let other = decoded.mesh(&bytes, &decoded_geosets[index]).unwrap();
            different_positions |= first.vertices.raw != other.vertices.raw;
            different_skin_bindings |= first.vertex_groups != other.vertex_groups
                || first.matrix_groups != other.matrix_groups
                || first.matrix_indices != other.matrix_indices;
        }
        eprintln!(
            "{model}: duplicate_vertex_count={vertices} distinct_geosets={geosets:?} different_rest_positions={different_positions} different_skin_bindings={different_skin_bindings}"
        );
    }
    eprintln!(
        "source={source:?} install={install:?} model={model:?} bones={} helpers={} geosets={} geoa={} layers={} txan={} duplicate_vertex_count_groups={}",
        bones.len(),
        helpers.len(),
        plan.geosets.len(),
        plan.geoset_animations.len(),
        plan.materials
            .iter()
            .map(|material| material.layers.len())
            .sum::<usize>(),
        plan.texture_animations.len(),
        duplicate_counts.len()
    );
    let sampled = plan.joints.sample_decoder_frame(0);
    assert_eq!(sampled.object_ids.len(), sampled.matrices.len());
}

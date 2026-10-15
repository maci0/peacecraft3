//! Public checked first-layer texture-selection API.
use pcx_gfx::mdx_runtime::{GeosetInput, RuntimeInput, RuntimePlan};
use pcx_gfx::mdx_texture_selection::TextureSelectionPlan;
use pcx_mdl::{Interpolation, Material, NO_ID, Pivots, Texture, TextureFrameTrack};

fn track(global: u32, keys: &[(i32, u32)]) -> TextureFrameTrack {
    TextureFrameTrack {
        interpolation: Interpolation::None,
        global_seq_id: global,
        keys: keys.to_vec(),
    }
}

fn material(base: u32, tracks: &[TextureFrameTrack]) -> Material {
    let mut layer = Vec::new();
    for word in [0u32, 0, base, NO_ID, 0] {
        layer.extend(word.to_le_bytes());
    }
    layer.extend(1f32.to_le_bytes());
    for track in tracks {
        layer.extend(b"KMTF");
        for word in [
            track.keys.len() as u32,
            track.interpolation as u32,
            track.global_seq_id,
        ] {
            layer.extend(word.to_le_bytes());
        }
        for &(time, texture) in &track.keys {
            layer.extend(time.to_le_bytes());
            layer.extend(texture.to_le_bytes());
        }
    }
    let mut data = vec![0; 8];
    data.extend(b"LAYS");
    data.extend(1u32.to_le_bytes());
    data.extend((layer.len() as u32 + 4).to_le_bytes());
    data.extend(layer);
    Material {
        offset: 0,
        len: data.len() as u32 + 4,
        data,
    }
}

fn runtime(tracks: &[TextureFrameTrack]) -> RuntimePlan {
    RuntimePlan::checked(RuntimeInput {
        bones: vec![],
        helpers: vec![],
        pivots: Pivots::default(),
        geosets: (0..2)
            .map(|material_id| GeosetInput {
                vertex_count: 0,
                material_id,
                matrix_joint_ids: vec![],
            })
            .collect(),
        geoset_animations: vec![],
        materials: vec![material(1, tracks), material(7, &[])],
        textures: (0..9)
            .map(|id| Texture {
                slot: 0,
                path: "Synthetic\\SameImage.blp".into(),
                flags: id % 4,
            })
            .collect(),
        texture_animations: vec![],
        global_sequences: vec![13],
    })
    .unwrap()
}

fn refused(plan: &RuntimePlan, interval: [u32; 2], reason: &str) {
    let before = plan.materials.clone();
    let error = TextureSelectionPlan::checked(plan, interval).unwrap_err();
    assert!(error.reason.contains(reason), "{error}");
    assert_eq!(error.context, "geoset 0 material 0 layer 0");
    assert_eq!(plan.materials, before, "admission changed source tracks");
}

#[test]
fn global_step_overrides_static_at_spawn_and_uses_independent_u64_clock() {
    let runtime = runtime(&[track(0, &[(0, 2), (7, 5)])]);
    let source = runtime.materials.clone();
    let plan = TextureSelectionPlan::checked(&runtime, [100, 117]).unwrap();
    for (elapsed, expected) in [(0, 2), (6, 2), (7, 5), (12, 5), (13, 2), (20, 5)] {
        assert_eq!(plan.sample(0, 103, elapsed), Some(expected));
    }
    assert_ne!(plan.sample(0, 103, 20), plan.sample(0, 103, 20 % 17));
    let expected = if u64::MAX % 13 < 7 { 2 } else { 5 };
    assert_eq!(plan.sample(0, 100, u64::MAX), Some(expected));
    assert_eq!(plan.sample(1, u32::MAX, u64::MAX), Some(7));
    assert_eq!(plan.sample(2, 100, 0), None);
    assert_eq!(runtime.textures[2].path, runtime.textures[5].path);
    assert_ne!(runtime.textures[2].flags, runtime.textures[5].flags);
    assert_eq!(runtime.materials, source);
}

#[test]
fn local_projection_filters_other_clips_and_clamps_applicable_endpoints() {
    let runtime = runtime(&[track(NO_ID, &[(-9, 8), (17, 2), (23, 5), (99, 8)])]);
    let plan = TextureSelectionPlan::checked(&runtime, [10, 30]).unwrap();
    for (frame, expected) in [(0, 2), (17, 2), (22, 2), (23, 5), (30, 5), (99, 5)] {
        assert_eq!(plan.sample(0, frame, u64::MAX), Some(expected));
    }
    let absent = TextureSelectionPlan::checked(&runtime, [40, 80]).unwrap();
    assert_eq!(absent.sample(0, 60, 0), Some(1));
    let single = TextureSelectionPlan::checked(&runtime, [23, 23]).unwrap();
    assert_eq!(single.sample(0, 0, 0), Some(5));
    for global in [0, NO_ID] {
        let empty = runtime_with_empty_track(global);
        assert_eq!(empty.sample(0, 0, u64::MAX), Some(1));
    }
}

fn runtime_with_empty_track(global: u32) -> TextureSelectionPlan {
    TextureSelectionPlan::checked(&runtime(&[track(global, &[])]), [10, 30]).unwrap()
}

#[test]
fn duplicate_unordered_and_unsupported_channels_refuse_before_projection() {
    for keys in [
        vec![(1, 2), (1, 5)],
        vec![(1, 2), (1, 2)],
        vec![(2, 2), (1, 5)],
    ] {
        refused(
            &runtime(&[track(NO_ID, &keys)]),
            [100, 117],
            if keys[0].0 == keys[1].0 {
                "duplicate"
            } else {
                "unordered"
            },
        );
    }
    refused(
        &runtime(&[track(NO_ID, &[]), track(NO_ID, &[])]),
        [0, 10],
        "multiple",
    );
    for interpolation in [
        Interpolation::Linear,
        Interpolation::Hermite,
        Interpolation::Bezier,
    ] {
        let mut source = track(NO_ID, &[(0, 2)]);
        source.interpolation = interpolation;
        refused(
            &runtime(&[source]),
            [0, 10],
            "unsupported KMTF interpolation",
        );
    }
}

#[test]
fn malformed_references_clocks_and_bindings_have_context() {
    let mut plan = runtime(&[track(0, &[(0, 2)])]);
    plan.global_sequences[0] = 0;
    refused(&plan, [0, 10], "zero-duration");
    plan.global_sequences.clear();
    refused(&plan, [0, 10], "missing KMTF global");
    let mut plan = runtime(&[track(NO_ID, &[(0, 2)])]);
    plan.materials[0].layers[0].texture_tracks[0].keys[0].1 = 99;
    refused(&plan, [100, 117], "missing TEXS record 99");
    plan.materials[0].layers[0].texture_tracks.clear();
    plan.materials[0].layers[0].properties.texture_id = 99;
    refused(&plan, [0, 10], "missing TEXS record 99");
    plan.materials[0].layers.clear();
    refused(&plan, [0, 10], "missing first material");
    plan.materials.clear();
    refused(&plan, [0, 10], "missing material");
    let mut plan = runtime(&[]);
    plan.geosets[0].geoset_id = 3;
    let error = TextureSelectionPlan::checked(&plan, [0, 10]).unwrap_err();
    assert!(error.context.contains("geoset 3"));
    assert!(error.reason.contains("ordinal"));
    plan.geosets.clear();
    let error = TextureSelectionPlan::checked(&plan, [10, 0]).unwrap_err();
    assert_eq!(error.context, "clip");
    assert!(error.reason.contains("reversed"));
}

#[test]
fn additional_layers_are_outside_this_plan() {
    let mut runtime = runtime(&[track(NO_ID, &[(0, 2)])]);
    let mut secondary = runtime.materials[0].layers[0].clone();
    secondary.properties.texture_id = 5;
    secondary.texture_tracks[0].interpolation = Interpolation::Linear;
    runtime.materials[0].layers.push(secondary);
    let plan = TextureSelectionPlan::checked(&runtime, [0, 10]).unwrap();
    assert_eq!(plan.sample(0, 0, 0), Some(2));
}

#[test]
#[ignore = "needs explicit local PEACECRAFT3_TEXTURE_SELECTION_MODEL; original bytes remain local"]
fn original_model_uses_authored_key_boundaries_and_global_wrap() {
    let path = std::env::var_os("PEACECRAFT3_TEXTURE_SELECTION_MODEL")
        .expect("supply the explicit retained local model path");
    let metadata = std::fs::metadata(&path).unwrap();
    assert!(
        metadata.len() <= 16 * 1024 * 1024,
        "local test admission ceiling"
    );
    let bytes = std::fs::read(&path).unwrap();
    let mdl = pcx_mdl::Mdlx::parse(&bytes).unwrap();
    let sequences = mdl.sequences(&bytes).unwrap();
    let clip = sequences.iter().find(|s| s.name == "Stand").unwrap();
    let runtime = RuntimePlan::parse(&bytes).unwrap();
    let source = runtime.materials.clone();
    let plan =
        TextureSelectionPlan::checked(&runtime, [clip.interval_start, clip.interval_end]).unwrap();
    let mut boundaries = 0;
    let mut static_overridden = false;
    for binding in &runtime.geosets {
        let layer = &runtime.materials[binding.material_id].layers[0];
        for track in &layer.texture_tracks {
            if track.global_seq_id == NO_ID || track.keys.is_empty() {
                continue;
            }
            let duration = u64::from(runtime.global_sequences[track.global_seq_id as usize]);
            for &(time, texture) in &track.keys {
                let elapsed = u64::try_from(time).unwrap();
                assert!(
                    elapsed < duration,
                    "this fixture requires keys within its global clock"
                );
                assert_eq!(
                    plan.sample(binding.geoset_id, clip.interval_start, elapsed),
                    Some(texture)
                );
                assert_eq!(
                    plan.sample(binding.geoset_id, clip.interval_end, elapsed + duration),
                    Some(texture)
                );
                boundaries += 1;
            }
            let first = track.keys.first().unwrap().1;
            assert_eq!(
                track.keys[0].0, 0,
                "explicit fixture needs its initial key at zero"
            );
            assert_eq!(
                plan.sample(binding.geoset_id, clip.interval_start, 0),
                Some(first)
            );
            assert_eq!(
                plan.sample(binding.geoset_id, clip.interval_end, duration),
                Some(first)
            );
            assert_eq!(
                plan.sample(binding.geoset_id, clip.interval_start, duration - 1),
                Some(track.keys.last().unwrap().1)
            );
            static_overridden |= first != layer.properties.texture_id;
        }
    }
    assert!(
        boundaries > 0 && static_overridden,
        "needs an authored channel distinct from static art"
    );
    assert_eq!(runtime.materials, source);
}

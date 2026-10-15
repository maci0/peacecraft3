use pcx_gfx::mdx_runtime::{GeosetInput, RuntimeInput, RuntimePlan};
use pcx_gfx::mdx_uv_translation::UvTranslationPlan;
use pcx_mdl::{Interpolation, KeyTrack, Material, NO_ID, Pivots, Texture, TextureAnim};

type VectorKey = (u32, [f32; 3], [f32; 3], [f32; 3]);

fn vector_track(interpolation: Interpolation, global: u32, keys: &[VectorKey]) -> KeyTrack {
    let mut raw = Vec::new();
    for &(frame, value, incoming, outgoing) in keys {
        raw.extend(frame.to_le_bytes());
        for v in value {
            raw.extend(v.to_le_bytes());
        }
        if interpolation.has_tangents() {
            for v in incoming.into_iter().chain(outgoing) {
                raw.extend(v.to_le_bytes());
            }
        }
    }
    KeyTrack {
        magic: *b"KTAT",
        count: keys.len() as u32,
        interpolation,
        global_seq_id: global,
        width: 3,
        stride: 4 + 12 * if interpolation.has_tangents() { 3 } else { 1 },
        raw,
    }
}

fn linear(global: u32, keys: &[(u32, [f32; 3])]) -> KeyTrack {
    vector_track(
        Interpolation::Linear,
        global,
        &keys
            .iter()
            .map(|&(f, v)| (f, v, [0.; 3], [0.; 3]))
            .collect::<Vec<_>>(),
    )
}

fn runtime(tracks: Vec<KeyTrack>) -> RuntimePlan {
    let mut data = vec![0; 8];
    data.extend(b"LAYS");
    data.extend(1u32.to_le_bytes());
    data.extend(28u32.to_le_bytes());
    for word in [2u32, 1, 0, 0, 0, 0.5f32.to_bits()] {
        data.extend(word.to_le_bytes());
    }
    RuntimePlan::checked(RuntimeInput {
        bones: vec![],
        helpers: vec![],
        pivots: Pivots::default(),
        geosets: vec![GeosetInput {
            vertex_count: 3,
            material_id: 0,
            matrix_joint_ids: vec![],
        }],
        geoset_animations: vec![],
        materials: vec![Material {
            offset: 0,
            len: data.len() as u32 + 4,
            data,
        }],
        textures: vec![Texture {
            slot: 0,
            path: "Synthetic\\Image.blp".into(),
            flags: 3,
        }],
        texture_animations: vec![TextureAnim {
            offset: 0,
            len: 4,
            tracks,
        }],
        global_sequences: vec![13],
    })
    .unwrap()
}

fn plan(runtime: &RuntimePlan, interval: [u32; 2]) -> UvTranslationPlan {
    UvTranslationPlan::checked(runtime, interval, &[1], "synthetic-source").unwrap()
}

#[test]
fn identity_no_animation_empty_track_and_no_applicable_local_keys() {
    let mut source = runtime(vec![]);
    for no_animation in [false, true] {
        if no_animation {
            source.materials[0].layers[0].properties.texture_anim_id = NO_ID;
        }
        let plan = plan(&source, [10, 30]);
        let sample = plan.sample(0, 20, u64::MAX).unwrap().unwrap();
        assert_eq!(sample.coord_id, 0);
        assert_eq!(sample.offset, [0., 0.]);
        assert_eq!(sample.apply([0.25, 0.75]), [0.25, 0.75]);
        assert_eq!(plan.sample(1, 20, 0).unwrap(), None);
    }
    let empty = runtime(vec![linear(NO_ID, &[])]);
    assert_eq!(
        plan(&empty, [10, 30])
            .sample(0, 20, 0)
            .unwrap()
            .unwrap()
            .offset,
        [0., 0.]
    );
    let absent = runtime(vec![linear(
        NO_ID,
        &[(5, [1., 2., 3.]), (90, [4., 5., 6.])],
    )]);
    assert_eq!(
        plan(&absent, [10, 30])
            .sample(0, 20, 0)
            .unwrap()
            .unwrap()
            .offset,
        [0., 0.]
    );
}

#[test]
fn local_projection_clamps_endpoints_and_applies_translation_to_authored_uv() {
    let source = runtime(vec![linear(
        NO_ID,
        &[
            (1, [99.; 3]),
            (17, [2., 7., 19.]),
            (23, [8., 9., -91.]),
            (90, [99.; 3]),
        ],
    )]);
    let original = source.texture_animations.clone();
    let plan = plan(&source, [10, 30]);
    for (frame, expected) in [
        (0, [2., 7.]),
        (17, [2., 7.]),
        (20, [5., 8.]),
        (23, [8., 9.]),
        (100, [8., 9.]),
    ] {
        assert_eq!(
            plan.sample(0, frame, 123).unwrap().unwrap().offset,
            expected
        );
    }
    let translated = plan.sample(0, 17, 0).unwrap().unwrap();
    assert_eq!(translated.apply([0.25, 0.75]), [2.25, 7.75]);
    let second = plan.sample(0, 23, 0).unwrap().unwrap();
    assert_eq!(
        second.apply(translated.apply([0.25, 0.75])),
        [10.25, 16.75],
        "affine translation composition adds offsets"
    );
    assert_eq!(source.texture_animations, original);
    assert_eq!(source.textures[0].flags, 3);
}

#[test]
fn global_clock_uses_u64_elapsed_independent_of_local_clip_and_wraps() {
    let source = runtime(vec![linear(
        0,
        &[(0, [0., 0., 99.]), (12, [12., 24., -99.])],
    )]);
    let plan = plan(&source, [100, 117]);
    for elapsed in [0, 7, 12, 13, 20, u64::MAX] {
        let expected = (elapsed % 13) as f32;
        assert_eq!(
            plan.sample(0, 103, elapsed).unwrap().unwrap().offset,
            [expected, 2. * expected]
        );
    }
    assert_ne!(
        plan.sample(0, 103, 20).unwrap(),
        plan.sample(0, 103, 20 % 17).unwrap()
    );
}

#[test]
fn step_hermite_and_bezier_keep_distinct_authored_interpolation() {
    for (interpolation, outgoing, incoming, expected) in [
        (Interpolation::None, [0.; 3], [0.; 3], [0., 0.]),
        (Interpolation::Linear, [0.; 3], [0.; 3], [4., 2.]),
        (
            Interpolation::Hermite,
            [4., 0., 0.],
            [-4., 0., 0.],
            [5., 2.],
        ),
        (Interpolation::Bezier, [0.; 3], [0.; 3], [1., 0.5]),
    ] {
        let track = vector_track(
            interpolation,
            NO_ID,
            &[
                (0, [0.; 3], [0.; 3], outgoing),
                (100, [8., 4., 0.], incoming, [0.; 3]),
            ],
        );
        let source = runtime(vec![track]);
        let plan = plan(&source, [0, 100]);
        assert_eq!(
            plan.sample(0, 50, 0).unwrap().unwrap().offset,
            expected,
            "{interpolation:?}"
        );
        assert_eq!(plan.sample(0, 100, 0).unwrap().unwrap().offset, [8., 4.]);
    }
}

#[test]
fn malformed_and_unsupported_inputs_refuse_with_source_binding_context() {
    for case in 0..15 {
        let mut source = runtime(vec![linear(0, &[(0, [0.; 3]), (7, [7.; 3])])]);
        let track = &mut source.texture_animations[0].tracks[0];
        match case {
            0 => track.width = 4,
            1 => track.stride = 12,
            2 => track.count = 3,
            3 => track.raw[16..20].copy_from_slice(&0u32.to_le_bytes()),
            4 => track.raw[16..20].copy_from_slice(&u32::MAX.to_le_bytes()),
            5 => track.raw[4..8].copy_from_slice(&f32::NAN.to_le_bytes()),
            6 => track.global_seq_id = 2,
            7 => source.global_sequences[0] = 0,
            8 => source.materials[0].layers[0].properties.texture_anim_id = 2,
            9 => source.materials[0].layers.clear(),
            10 => source.geosets[0].geoset_id = 2,
            11 => track.magic = *b"KTAR",
            12 => track.magic = *b"KTAS",
            13 => {
                let duplicate = track.clone();
                source.texture_animations[0].tracks.push(duplicate);
            }
            14 => source.materials[0].layers[0].properties.coord_id = 1,
            _ => unreachable!(),
        }
        let before = source.texture_animations.clone();
        let error =
            UvTranslationPlan::checked(&source, [100, 117], &[1], "model-identity").unwrap_err();
        assert!(
            error
                .context
                .contains("model-identity geoset 0 material 0 layer 0"),
            "case={case}: {error}"
        );
        assert_eq!(source.texture_animations, before);
    }
    let source = runtime(vec![]);
    assert!(
        UvTranslationPlan::checked(&source, [10, 0], &[1], "model")
            .unwrap_err()
            .reason
            .contains("reversed")
    );
    assert!(
        UvTranslationPlan::checked(&source, [0, 10], &[], "model")
            .unwrap_err()
            .reason
            .contains("count mismatch")
    );
    assert!(
        UvTranslationPlan::checked(&source, [0, 10], &[0], "model")
            .unwrap_err()
            .reason
            .contains("coordinate set")
    );
    assert!(
        UvTranslationPlan::checked(&source, [0, 10], &[2], "model")
            .unwrap_err()
            .reason
            .contains("multiple UV")
    );
}

#[test]
fn source_tangents_are_validated_before_projection_and_sample_overflow_reports() {
    let mut track = vector_track(
        Interpolation::Bezier,
        NO_ID,
        &[(0, [0.; 3], [0.; 3], [0.; 3])],
    );
    track.raw[16..20].copy_from_slice(&f32::INFINITY.to_le_bytes());
    let source = runtime(vec![track]);
    assert!(
        UvTranslationPlan::checked(&source, [100, 200], &[1], "model")
            .unwrap_err()
            .reason
            .contains("nonfinite")
    );
    let source = runtime(vec![linear(
        NO_ID,
        &[(0, [f32::MAX; 3]), (100, [-f32::MAX; 3])],
    )]);
    assert!(
        plan(&source, [0, 100])
            .sample(0, 50, 0)
            .unwrap_err()
            .reason
            .contains("nonfinite sampled")
    );
}

#[test]
#[ignore = "requires explicit local source-qualified metadata manifest; no game bytes are included"]
fn retained_menu_requirements_are_admitted_and_exact_keys_preserve_xy() {
    let input = std::fs::read_to_string(
        std::env::var("PEACECRAFT3_UV_MODELS").expect("explicit retained models manifest"),
    )
    .unwrap();
    let mut required = 0;
    for line in input.lines() {
        let row: Vec<_> = line.split('\t').collect();
        let bytes = std::fs::read(row[1]).unwrap();
        let mdl = pcx_mdl::Mdlx::parse(&bytes).unwrap();
        let source = RuntimePlan::parse(&bytes).unwrap();
        let original = source.texture_animations.clone();
        let counts: Vec<u32> = row[3].split(',').map(|n| n.parse().unwrap()).collect();
        let clips = mdl.sequences(&bytes).unwrap();
        let clip = clips
            .iter()
            .find(|clip| clip.name == "Stand")
            .or_else(|| clips.first())
            .unwrap();
        assert_eq!(
            source.uv_set_counts,
            counts.into_iter().map(Some).collect::<Vec<_>>()
        );
        let plan = UvTranslationPlan::from_runtime(
            &source,
            [clip.interval_start, clip.interval_end],
            row[0],
        )
        .unwrap();
        for binding in &source.geosets {
            let layer = &source.materials[binding.material_id].layers[0];
            if layer.properties.texture_anim_id == NO_ID {
                assert_eq!(
                    plan.sample(binding.geoset_id, clip.interval_start, u64::MAX)
                        .unwrap()
                        .unwrap()
                        .offset,
                    [0., 0.]
                );
                continue;
            }
            for track in
                &source.texture_animations[layer.properties.texture_anim_id as usize].tracks
            {
                required += 1;
                assert_eq!(track.magic, *b"KTAT");
                let duration = u64::from(source.global_sequences[track.global_seq_id as usize]);
                for key in 0..track.count as usize {
                    let frame = track.frame(key).unwrap();
                    if u64::from(frame) >= duration {
                        continue;
                    }
                    let value = track.value(key).unwrap();
                    for elapsed in [u64::from(frame), u64::from(frame) + duration] {
                        assert_eq!(
                            plan.sample(binding.geoset_id, clip.interval_start, elapsed)
                                .unwrap()
                                .unwrap()
                                .offset,
                            [value[0], value[1]]
                        );
                    }
                }
                for elapsed in [0, duration / 2, duration - 1, duration, u64::MAX] {
                    assert!(
                        plan.sample(binding.geoset_id, clip.interval_start, elapsed)
                            .unwrap()
                            .unwrap()
                            .offset
                            .into_iter()
                            .all(f32::is_finite)
                    );
                }
            }
        }
        assert_eq!(source.texture_animations, original);
        println!(
            "admitted source={:?} sha256={} geosets={}",
            row[0],
            row[2],
            source.geosets.len()
        );
    }
    assert!(
        required > 0,
        "fixture must exercise animated menu requirements"
    );
}

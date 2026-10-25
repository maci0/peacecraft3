use pcx_gfx::mdx_particle_render::{
    Blend, CameraBasis, Decoded, Failure, ImageGraph, ImageLimits, Piece, draw_requirements, quads,
};
use pcx_gfx::mdx_particles;
use pcx_gfx::mdx_particles::{Clip, Limits, Particle, Plan};
use pcx_mdl::{Particle2, Particle2Properties, Pivots, Texture};
use std::time::Duration;
const SOURCE: &str = "synthetic:TFT:archive:member:hash";
fn props() -> Particle2Properties {
    Particle2Properties {
        speed: 2.,
        variation: 0.,
        latitude: 0.,
        gravity: 0.,
        life_span: 4.,
        emission_rate: 1.,
        width: 0.,
        length: 0.,
        filter_mode: 0,
        rows: 2,
        columns: 3,
        head_or_tail: 2,
        tail_length: 0.5,
        time: 0.5,
        segment_colours: [[0.2, 0.4, 0.6]; 3],
        segment_alpha: [128; 3],
        particle_scaling: [2.; 3],
        life_span_uv_anim: [4, 4, 1],
        decay_uv_anim: [4, 4, 1],
        tail_uv_anim: [2, 2, 1],
        tail_decay_uv_anim: [2, 2, 1],
        texture_id: 0,
        squirt: 0,
        priority_plane: -7,
        replaceable_id: 0,
    }
}
fn tex(path: &str, flags: u32) -> Texture {
    Texture {
        slot: 0,
        path: path.into(),
        flags,
    }
}
fn plan(p: Particle2Properties, flags: u32, textures: &[Texture]) -> Plan {
    let mut e = Particle2 {
        offset: 0,
        total_size: 0,
        node_size: 96,
        name: "synthetic".into(),
        object_id: 0,
        parent_id: -1,
        flags,
        node_tracks: vec![],
        state: p.to_bytes(),
        tracks: vec![],
    };
    e.node_size = e.measured_node_size() as u32;
    e.total_size = e.measured_size() as u32;
    Plan::checked(
        &[e],
        textures,
        &Pivots(vec![[0.; 3]]),
        &[],
        Clip {
            interval: [0, 1000],
            looping: true,
        },
        SOURCE,
        Limits {
            max_emitters: 4,
            max_keys: 100,
            particle_capacity: 16,
            max_ticks: 10,
            max_updates: 100,
            tick: Duration::from_millis(10),
        },
    )
    .unwrap()
}
fn particle() -> Particle {
    Particle {
        emitter: 0,
        age: 1.,
        position: [10., 20., 30.],
        velocity: [4., 0., 0.],
        gravity: 0.,
    }
}
fn camera() -> CameraBasis {
    CameraBasis::checked([1., 0., 0.], [0., 1., 0.], [0., 0., 1.]).unwrap()
}
fn graph(p: &Plan, t: &[Texture], geometry: &[u32]) -> ImageGraph {
    ImageGraph::checked(p, t, geometry, ImageLimits { max_requests: 10 }).unwrap()
}
fn image<'a>(path: &'a str, rgba: &'a [u8], width: u32, height: u32) -> Decoded<'a> {
    Decoded {
        mipmaps: &[],
        source_context: SOURCE,
        path,
        width,
        height,
        rgba,
    }
}
fn approx(a: f32, b: f32) {
    assert!((a - b).abs() < 0.00001, "{a} != {b}");
}

#[test]
fn mip_chains_participate_in_particle_admission_budget_and_identity() {
    let t = [tex("a.blp", 0)];
    let p = plan(props(), 0, &t);
    let g = graph(&p, &t, &[]);
    let pixels = [0; 16];
    let mips = [vec![17; 4]];
    let mut a = image("a.blp", &pixels, 2, 2);
    a.mipmaps = &mips;
    let prepared = g.preflight(&[a], 20).unwrap();
    assert_eq!(prepared.rgba_bytes, 20);
    assert!(std::ptr::eq(prepared.images[0].mipmaps, &mips));
    assert!(
        matches!(g.preflight(&[a],19), Err(e) if e.failure == Failure::Budget("decoded backing bytes"))
    );
    let other = [vec![19; 4]];
    let mut b = a;
    b.mipmaps = &other;
    assert!(
        matches!(g.preflight(&[a,b],40),Err(e) if e.failure == Failure::Conflict("decoded canonical pixels"))
    );
    let invalid = [vec![0; 3]];
    b.mipmaps = &invalid;
    assert!(
        matches!(g.preflight(&[b],20), Err(e) if e.failure == Failure::Malformed("decoded dimensions/RGBA length"))
    );
}
#[test]
fn request_graph_unions_particle_and_geometry_ids_and_splits_sampler_ownership() {
    let t = [
        tex("A\\Marker.blp", 0),
        tex("a/marker.BLP", 0),
        tex("A/Marker.blp", 1),
        tex("other.blp", 0),
    ];
    let p = plan(props(), 0, &t);
    let g = graph(&p, &t, &[3, 2, 1, 3]);
    assert_eq!(
        g.bindings()
            .iter()
            .map(|b| b.texture_id)
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3]
    );
    assert_eq!(g.backings().len(), 3);
    assert_eq!(g.backing_for(0), g.backing_for(1));
    assert_ne!(g.backing_for(0), g.backing_for(2));
    assert_eq!(g.backings()[1].sampler_flags, 1);
    let pixels = [11, 22, 33, 17];
    let other = [44, 55, 66, 200];
    let prepared = g
        .preflight(
            &[
                image("a/marker.blp", &pixels, 1, 1),
                image("other.blp", &other, 1, 1),
            ],
            12,
        )
        .unwrap();
    assert_eq!(prepared.rgba_bytes, 12);
    assert_eq!(prepared.images.len(), 3);
    assert!(std::ptr::eq(
        prepared.images[0].rgba.as_ptr(),
        pixels.as_ptr()
    ));
    assert_eq!(prepared.images[0].rgba[3], 17);
    assert!(
        matches!(g.preflight(&[image("a/marker.blp",&pixels,1,1),image("other.blp",&other,1,1)],11),Err(e) if e.failure==Failure::Budget("decoded backing bytes"))
    );
}
#[test]
fn missing_late_image_refuses_complete_batch_without_mutating_graph() {
    let t = [tex("a.blp", 0), tex("b.blp", 0)];
    let p = plan(props(), 0, &t);
    let g = graph(&p, &t, &[1]);
    let bytes = [1, 2, 3, 4];
    assert!(
        matches!(g.preflight(&[image("a.blp",&bytes,1,1)],100),Err(e) if e.texture==Some(1)&&e.failure==Failure::Missing("decoded canonical image"))
    );
    assert_eq!(g.bindings().len(), 2);
    assert!(
        g.preflight(
            &[image("a.blp", &bytes, 1, 1), image("b.blp", &bytes, 1, 1)],
            8
        )
        .is_ok()
    );
}
#[test]
fn canonical_duplicate_pixels_and_qualified_source_conflicts_are_refused() {
    let t = [tex("a.blp", 0)];
    let p = plan(props(), 0, &t);
    let g = graph(&p, &t, &[]);
    let a = [1, 2, 3, 4];
    let b = [1, 2, 3, 5];
    assert!(
        matches!(g.preflight(&[image("a.blp",&a,1,1),image("A.BLP",&b,1,1)],100),Err(e) if e.failure==Failure::Conflict("decoded canonical pixels"))
    );
    let mut stale = image("a.blp", &a, 1, 1);
    stale.source_context = "synthetic:other-edition";
    assert!(
        matches!(g.preflight(&[image("a.blp",&a,1,1),stale],100),Err(e) if e.failure==Failure::Conflict("decoded source context"))
    );
    assert!(
        g.preflight(&[image("a.blp", &a, 1, 1), image("A.BLP", &a, 1, 1)], 4)
            .is_ok()
    );
}
#[test]
fn image_dimensions_lengths_and_overflow_are_checked() {
    let t = [tex("a.blp", 0)];
    let p = plan(props(), 0, &t);
    let g = graph(&p, &t, &[]);
    for (w, h, bytes) in [
        (0, 1, &[1u8, 2, 3, 4][..]),
        (1, 2, &[1u8, 2, 3, 4][..]),
        (u32::MAX, u32::MAX, &[][..]),
    ] {
        assert!(
            matches!(g.preflight(&[image("a.blp",bytes,w,h)],usize::MAX),Err(e) if e.failure==Failure::Malformed("decoded dimensions/RGBA length"))
        );
    }
}
#[test]
fn invalid_texture_requests_fail_with_context() {
    let t = [tex("a.blp", 0), tex("b.blp", 0)];
    let p = plan(props(), 0, &t);
    assert!(
        matches!(ImageGraph::checked(&p,&t,&[9],ImageLimits{max_requests:10}),Err(e) if e.texture==Some(9)&&e.failure==Failure::Missing("TEXS record"))
    );
    assert!(
        matches!(ImageGraph::checked(&p,&t,&[1],ImageLimits{max_requests:1}),Err(e) if e.failure==Failure::Budget("texture requests"))
    );
    let bad = [tex("a.blp", 2)];
    assert!(
        matches!(ImageGraph::checked(&p,&bad,&[],ImageLimits{max_requests:10}),Err(e) if e.failure==Failure::Conflict("CPU plan/TEXS sampler"))
    );
    let bad = [tex("a.blp", 4)];
    assert!(
        matches!(ImageGraph::checked(&p,&bad,&[],ImageLimits{max_requests:10}),Err(e) if e.failure==Failure::Unsupported("sampler bits"))
    );
}
#[test]
fn head_and_tail_preserve_distinct_asymmetric_atlas_corners_and_segment_rgba() {
    let p = plan(props(), 0, &[tex("a.blp", 0)]);
    let pieces = quads(&p, &particle(), camera()).unwrap();
    let h = pieces[0].unwrap();
    let t = pieces[1].unwrap();
    assert_eq!(h.piece, Piece::Head);
    assert_eq!(t.piece, Piece::Tail);
    assert_eq!(
        h.positions,
        [
            [8., 18., 30.],
            [8., 22., 30.],
            [12., 22., 30.],
            [12., 18., 30.]
        ]
    );
    assert_eq!(
        t.positions,
        [
            [8., 18., 30.],
            [10., 18., 30.],
            [10., 22., 30.],
            [8., 22., 30.]
        ]
    );
    assert_eq!(
        h.uv,
        [[2. / 3., 0.5], [1. / 3., 0.5], [1. / 3., 1.], [2. / 3., 1.]]
    );
    assert_eq!(t.uv, [[1., 0.], [2. / 3., 0.], [2. / 3., 0.5], [1., 0.5]]);
    assert_eq!(h.rgba, [0.2, 0.4, 0.6, 128. / 255.]);
    assert_eq!(t.rgba, h.rgba);
}
#[test]
fn active_piece_selection_is_authored() {
    for (mode, heads, tails) in [(0, true, false), (1, false, true), (2, true, true)] {
        let mut v = props();
        v.head_or_tail = mode;
        let p = plan(v, 0, &[tex("a.blp", 0)]);
        let q = quads(&p, &particle(), camera()).unwrap();
        assert_eq!(q[0].is_some(), heads);
        assert_eq!(q[1].is_some(), tails);
    }
}
#[test]
fn camera_rotation_changes_billboard_but_xy_geometry_stays_in_source_xy() {
    let t = [tex("a.blp", 0)];
    let p = plan(props(), 0, &t);
    let cam = CameraBasis::checked([0., 1., 0.], [0., 0., 1.], [1., 0., 0.]).unwrap();
    let q = quads(&p, &particle(), cam).unwrap()[0].unwrap();
    assert_eq!(q.positions[0], [10., 18., 28.]);
    let xy = plan(props(), mdx_particles::XY_QUAD, &t);
    let a = quads(&xy, &particle(), cam).unwrap()[0].unwrap();
    let b = quads(&xy, &particle(), camera()).unwrap()[0].unwrap();
    assert_eq!(a, b);
    let angle = -std::f32::consts::PI + std::f32::consts::PI / 8.;
    approx(a.positions[0][0], 10. + 2. * (-angle.cos() + angle.sin()));
    approx(a.positions[0][1], 20. + 2. * (-angle.sin() - angle.cos()));
    assert_eq!(a.positions[0][2], 30.);
}
#[test]
fn zero_speed_and_eye_parallel_tails_collapse_without_nan() {
    let p = plan(props(), 0, &[tex("a.blp", 0)]);
    for velocity in [[0., 0., 0.], [0., 0., 4.], [f32::MIN_POSITIVE, 0., 0.]] {
        let mut v = particle();
        v.velocity = velocity;
        let q = quads(&p, &v, camera()).unwrap()[1].unwrap();
        assert!(q.positions.iter().flatten().all(|v| v.is_finite()));
        assert_eq!(q.positions[0], q.positions[3]);
        assert_eq!(q.positions[1], q.positions[2]);
    }
}
#[test]
fn tail_width_uses_camera_plane_even_with_depth_velocity() {
    let p = plan(props(), 0, &[tex("a.blp", 0)]);
    let turned = CameraBasis::checked([0., 1., 0.], [0., 0., 1.], [1., 0., 0.]).unwrap();
    for depth in [0., 4., 400.] {
        for (basis, velocity, side) in [
            (camera(), [4., 0., depth], [0., 2., 0.]),
            (turned, [depth, 4., 0.], [0., 0., 2.]),
        ] {
            let mut v = particle();
            v.velocity = velocity;
            let q = quads(&p, &v, basis).unwrap()[1].unwrap();
            for (i, offset) in side.into_iter().enumerate() {
                approx(q.positions[1][i], v.position[i] - offset);
                approx(q.positions[2][i], v.position[i] + offset);
            }
        }
    }
}
#[test]
fn subunit_projected_tails_taper_without_depth_dependent_width() {
    let p = plan(props(), 0, &[tex("a.blp", 0)]);
    for depth in [0., 4., 400.] {
        let mut v = particle();
        v.velocity = [1., 0., depth];
        let q = quads(&p, &v, camera()).unwrap()[1].unwrap();
        approx(q.positions[1][1], 19.);
        approx(q.positions[2][1], 21.);
    }
}
#[test]
fn invalid_camera_and_nonfinite_particle_refuse_with_source_and_emitter_context() {
    for (right, up, eye) in [
        ([0.; 3], [0., 1., 0.], [0., 0., 1.]),
        ([1., 0., 0.], [1., 0., 0.], [0., 0., 1.]),
        ([1., 0., 0.], [0., 1., 0.], [0., 0., -1.]),
        ([f32::NAN, 0., 0.], [0., 1., 0.], [0., 0., 1.]),
    ] {
        assert!(CameraBasis::checked(right, up, eye).is_err());
    }
    let p = plan(props(), 0, &[tex("a.blp", 0)]);
    let mut v = particle();
    v.position[0] = f32::NAN;
    let e = quads(&p, &v, camera()).unwrap_err();
    assert_eq!(e.source_name, SOURCE);
    assert_eq!(e.emitter, Some(0));
    assert_eq!(
        e.failure,
        Failure::Appearance(mdx_particles::Failure::Arithmetic)
    );
    v = particle();
    v.emitter = 7;
    assert_eq!(
        quads(&p, &v, camera()).unwrap_err().failure,
        Failure::Appearance(mdx_particles::Failure::Malformed("particle emitter"))
    );
}
#[test]
fn draw_requirements_retain_authored_priority_filter_and_scene_effect_needs() {
    for (filter, blend) in [(0, Blend::SourceAlpha), (1, Blend::Additive)] {
        let mut v = props();
        v.filter_mode = filter;
        let flags = mdx_particles::SORT_FAR_Z | mdx_particles::PARTICLE_EMITTER_TYPE;
        let p = plan(v, flags, &[tex("a.blp", 0)]);
        let r = draw_requirements(&p, 0).unwrap();
        assert_eq!(r.flags, flags);
        assert_eq!(r.priority_plane, -7);
        assert_eq!(r.texture, 0);
        assert_eq!(r.blend, blend);
        assert_eq!(r.filter_mode, filter);
        assert!(r.sort_far_z && r.lighting_required && r.fog_required);
        let p = plan(
            v,
            mdx_particles::UNSHADED | mdx_particles::UNFOGGED,
            &[tex("a.blp", 0)],
        );
        let r = draw_requirements(&p, 0).unwrap();
        assert!(!r.lighting_required && !r.fog_required);
    }
}

#[test]
fn geometry_overflow_is_a_contextual_refusal() {
    let mut v = props();
    v.particle_scaling = [f32::MAX; 3];
    let p = plan(v, 0, &[tex("a.blp", 0)]);
    let mut particle = particle();
    particle.position = [f32::MAX; 3];
    let e = quads(&p, &particle, camera()).unwrap_err();
    assert_eq!(e.failure, Failure::Arithmetic);
    assert_eq!(e.source_name, SOURCE);
    assert_eq!(e.emitter, Some(0));
    assert_eq!(e.texture, Some(0));
}
#[test]
fn source_quad_has_no_implicit_scene_basis_or_second_birth_transform() {
    let p = plan(props(), 0, &[tex("a.blp", 0)]);
    let particle = particle();
    let quad = quads(&p, &particle, camera()).unwrap()[0].unwrap();
    // Synthetic proper source-to-scene basis: (x,y,z) -> (z,x,y), plus placement.
    let to_scene = |[x, y, z]: [f32; 3]| [z + 100., x + 200., y + 300.];
    let scene = quad.positions.map(to_scene);
    assert_eq!(scene[0], [130., 208., 318.]);
    assert_eq!(scene[2], [130., 212., 322.]);
    assert_ne!(scene[0], to_scene(scene[0]));
    assert_eq!(particle.position, [10., 20., 30.]);
}

#[test]
#[ignore = "requires explicit retained TFT model and source context; no image decode or render"]
fn explicit_tft_source_graph_and_all_profile_geometry() {
    let bytes =
        std::fs::read(std::env::var("PCX_TFT_PRE2_MODEL").expect("explicit fixture path")).unwrap();
    let source = std::env::var("PCX_TFT_PRE2_SOURCE").expect("explicit qualified source context");
    let mdl = pcx_mdl::Mdlx::parse(&bytes).unwrap();
    let emitters = mdl.particles2(&bytes).unwrap();
    let textures = mdl.textures(&bytes).unwrap();
    let pivots = mdl.pivots(&bytes).unwrap();
    let glbs = mdl.payload(&bytes, b"GLBS").unwrap();
    let (words, remainder) = glbs.as_chunks::<4>();
    assert!(remainder.is_empty());
    let globals: Vec<_> = words.iter().map(|v| u32::from_le_bytes(*v)).collect();
    let mut geometry_ids = Vec::new();
    let materials = mdl.materials(&bytes).unwrap();
    for geoset in mdl.geosets(&bytes).unwrap() {
        let mesh = mdl.mesh(&bytes, &geoset).unwrap();
        let layers = materials[mesh.material_id as usize]
            .checked_layers()
            .unwrap();
        let layer = layers.first().expect("app first material layer");
        geometry_ids.push(layer.properties.texture_id);
        for track in &layer.texture_tracks {
            geometry_ids.extend(track.keys.iter().map(|k| k.1));
        }
    }
    let clips = mdl.sequences(&bytes).unwrap();
    assert!(!clips.is_empty() && !emitters.is_empty());
    let mut pieces = 0;
    for clip in &clips {
        let p = Plan::checked(
            &emitters,
            &textures,
            &pivots,
            &globals,
            Clip {
                interval: [clip.interval_start, clip.interval_end],
                looping: !clip.non_looping,
            },
            &source,
            Limits {
                max_emitters: 1000,
                max_keys: 1_000_000,
                particle_capacity: 10000,
                max_ticks: 1000,
                max_updates: 1_000_000,
                tick: Duration::from_millis(10),
            },
        )
        .unwrap();
        let g = ImageGraph::checked(
            &p,
            &textures,
            &geometry_ids,
            ImageLimits {
                max_requests: 10000,
            },
        )
        .unwrap();
        for &id in &geometry_ids {
            assert!(g.backing_for(id).is_some());
        }
        for (i, profile) in p.profiles().iter().enumerate() {
            assert!(g.backing_for(profile.properties.texture_id).is_some());
            let requirements = draw_requirements(&p, i).unwrap();
            assert_eq!(requirements.flags, profile.flags);
            for fraction in [0., 0.25, 0.5, 0.75] {
                let particle = Particle {
                    emitter: i,
                    age: f64::from(profile.properties.life_span) * fraction,
                    position: profile.pivot,
                    velocity: [1., 2., 3.],
                    gravity: profile.properties.gravity,
                };
                for quad in quads(&p, &particle, camera())
                    .unwrap()
                    .into_iter()
                    .flatten()
                {
                    assert!(
                        quad.positions
                            .iter()
                            .flatten()
                            .chain(quad.uv.iter().flatten())
                            .chain(&quad.rgba)
                            .all(|v| v.is_finite())
                    );
                    pieces += 1;
                }
            }
        }
        println!(
            "source graph: profiles={}, TEXS requests={}, sampler backings={}; original image preflight not exercised",
            p.profiles().len(),
            g.bindings().len(),
            g.backings().len()
        );
    }
    println!(
        "source geometry: clips={}, sampled pieces={pieces}; finite construction only, no rendered acceptance",
        clips.len()
    );
}

#[test]
fn model_space_draws_move_head_and_tail_with_the_current_emitter_pose() {
    use pcx_mdl::{Interpolation, KeyTrack, NO_ID};
    let mut e = Particle2 {
        offset: 0,
        total_size: 0,
        node_size: 96,
        name: "moving-model-space".into(),
        object_id: 0,
        parent_id: -1,
        flags: mdx_particles::MODEL_SPACE,
        state: props().to_bytes(),
        node_tracks: vec![],
        tracks: vec![],
    };
    let mut raw = vec![];
    for (frame, value) in [(0u32, [0f32, 0., 0.]), (1000, [8., 0., 0.])] {
        raw.extend(frame.to_le_bytes());
        for v in value {
            raw.extend(v.to_le_bytes());
        }
    }
    e.node_tracks.push(KeyTrack {
        magic: *b"KGTR",
        count: 2,
        interpolation: Interpolation::Linear,
        global_seq_id: NO_ID,
        width: 3,
        stride: 16,
        raw,
    });
    e.node_size = e.measured_node_size() as u32;
    e.total_size = e.measured_size() as u32;
    let p = Plan::checked(
        &[e],
        &[tex("marker.blp", 0)],
        &Pivots(vec![[0.; 3]]),
        &[],
        Clip {
            interval: [0, 1000],
            looping: false,
        },
        SOURCE,
        Limits {
            max_emitters: 4,
            max_keys: 100,
            particle_capacity: 16,
            max_ticks: 10,
            max_updates: 100,
            tick: Duration::from_millis(10),
        },
    )
    .unwrap();
    let local = particle();
    let before = quads(&p, &local, camera()).unwrap();
    let after = pcx_gfx::mdx_particle_render::quads_at(&p, &local, camera(), 500).unwrap();
    for (before, after) in before.into_iter().zip(after) {
        let before = before.unwrap();
        let after = after.unwrap();
        assert_eq!(before.rgba, after.rgba);
        assert_eq!(before.uv, after.uv);
        for (a, b) in before.positions.into_iter().zip(after.positions) {
            assert_eq!(b, [a[0] + 4., a[1], a[2]]);
        }
    }
}

#[test]
fn model_space_quads_follow_the_current_parent_joint_pose() {
    use pcx_mdl::{Bone, BoneTrack, Interpolation, TrackKind};
    let mut raw = Vec::new();
    for (time, value) in [(0u32, [0f32, 0., 0.]), (1000, [8., 0., 0.])] {
        raw.extend(time.to_le_bytes());
        for component in value {
            raw.extend(component.to_le_bytes());
        }
    }
    let bone = Bone {
        offset: 0,
        span: 104,
        name: "parent".into(),
        object_id: 1,
        parent_id: -1,
        flags: 0x100,
        tracks: vec![BoneTrack {
            kind: TrackKind::Translation,
            count: 2,
            interpolation: Interpolation::Linear,
            global_sequence_id: -1,
            offset: 0,
            stride: 16,
            raw,
        }],
        geoset_id: -1,
        geoset_anim_id: -1,
    };
    let joints =
        pcx_gfx::mdx_runtime::JointPlan::checked(&[bone], &[], &Pivots(vec![[0.; 3], [0.; 3]]))
            .unwrap();
    let mut emitter = Particle2 {
        offset: 0,
        total_size: 0,
        node_size: 96,
        name: "parented-model-space".into(),
        object_id: 0,
        parent_id: 1,
        flags: mdx_particles::MODEL_SPACE,
        state: props().to_bytes(),
        node_tracks: vec![],
        tracks: vec![],
    };
    emitter.node_size = emitter.measured_node_size() as u32;
    emitter.total_size = emitter.measured_size() as u32;
    let plan = Plan::checked_with_joints(
        &[emitter],
        &[tex("marker.blp", 0)],
        &[],
        Clip {
            interval: [0, 1000],
            looping: false,
        },
        SOURCE,
        Limits {
            max_emitters: 4,
            max_keys: 100,
            particle_capacity: 16,
            max_ticks: 10,
            max_updates: 100,
            tick: Duration::from_millis(10),
        },
        &joints,
    )
    .unwrap();
    let local = Particle {
        position: [0.; 3],
        ..particle()
    };
    let before = quads(&plan, &local, camera()).unwrap();
    let after = pcx_gfx::mdx_particle_render::quads_at(&plan, &local, camera(), 500).unwrap();
    for (before, after) in before.into_iter().zip(after) {
        let before = before.unwrap();
        let after = after.unwrap();
        for (a, b) in before.positions.into_iter().zip(after.positions) {
            assert_eq!(b, [a[0] + 4., a[1], a[2]]);
        }
    }
}

use pcx_gfx::mdx_particles;
use pcx_gfx::mdx_particles::{Appearance, Clip, Failure, Limits, Particle, Plan, Simulation};
use pcx_mdl::{Interpolation, KeyTrack, NO_ID, Particle2, Particle2Properties, Pivots, Texture};
use std::{sync::Arc, time::Duration};

fn limits() -> Limits {
    Limits {
        max_emitters: 128,
        max_keys: 10000,
        particle_capacity: 4096,
        max_ticks: 10000,
        max_updates: 1_000_000,
        tick: Duration::from_millis(125),
    }
}
fn properties() -> Particle2Properties {
    Particle2Properties {
        speed: 0.,
        variation: 0.,
        latitude: 0.,
        gravity: 0.,
        life_span: 4.,
        emission_rate: 4.,
        width: 0.,
        length: 0.,
        filter_mode: 0,
        rows: 2,
        columns: 3,
        head_or_tail: 2,
        tail_length: 0.5,
        time: 0.25,
        segment_colours: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
        segment_alpha: [0, 128, 255],
        particle_scaling: [1., 3., 7.],
        life_span_uv_anim: [0, 6, 2],
        decay_uv_anim: [0, 6, 1],
        tail_uv_anim: [4, 4, 1],
        tail_decay_uv_anim: [4, 4, 1],
        texture_id: 0,
        squirt: 0,
        priority_plane: -7,
        replaceable_id: 0,
    }
}
fn emitter(p: Particle2Properties) -> Particle2 {
    let mut e = Particle2 {
        offset: 0,
        total_size: 0,
        node_size: 96,
        name: "synthetic".into(),
        object_id: 0,
        parent_id: -1,
        flags: mdx_particles::UNSHADED,
        node_tracks: vec![],
        state: p.to_bytes(),
        tracks: vec![],
    };
    finish(&mut e);
    e
}
fn finish(e: &mut Particle2) {
    e.node_size = e.measured_node_size() as u32;
    e.total_size = e.measured_size() as u32;
}
fn texture() -> Vec<Texture> {
    vec![Texture {
        slot: 0,
        path: "synthetic-marker.blp".into(),
        flags: pcx_mdl::TEXTURE_WRAP_WIDTH,
    }]
}
fn plan(e: &[Particle2], globals: &[u32], interval: [u32; 2], looping: bool) -> Arc<Plan> {
    Arc::new(
        Plan::checked(
            e,
            &texture(),
            &Pivots(vec![[7., 11., 13.]; e.len()]),
            globals,
            Clip { interval, looping },
            "synthetic:markers",
            limits(),
        )
        .unwrap(),
    )
}
type TrackKey<'a> = (i32, &'a [f32], &'a [f32], &'a [f32]);
fn track(
    tag: [u8; 4],
    interpolation: Interpolation,
    global: u32,
    keys: &[TrackKey<'_>],
) -> KeyTrack {
    let width = keys[0].1.len();
    let stride = 4 + width * 4 * if interpolation.has_tangents() { 3 } else { 1 };
    let mut raw = vec![];
    for (frame, v, i, o) in keys {
        raw.extend(frame.to_le_bytes());
        for x in *v {
            raw.extend(x.to_le_bytes());
        }
        if interpolation.has_tangents() {
            for x in i.iter().chain(o.iter()) {
                raw.extend(x.to_le_bytes());
            }
        }
    }
    KeyTrack {
        magic: tag,
        count: keys.len() as u32,
        interpolation,
        global_seq_id: global,
        width,
        stride,
        raw,
    }
}
fn approx(a: f32, b: f32) {
    assert!((a - b).abs() < 0.00002, "{a} != {b}");
}
fn live(age: f64) -> Particle {
    Particle {
        emitter: 0,
        age,
        position: [2., 4., 8.],
        velocity: [4., -2., 0.],
        gravity: 0.,
    }
}

#[test]
fn queued_hitches_preserve_time_particles_and_random_stream() {
    let mut properties = properties();
    properties.variation = 0.5;
    properties.speed = 7.;
    let plan = plan(&[emitter(properties)], &[], [100, 1100], true);
    let mut expected = Simulation::new(Arc::clone(&plan), 17, limits()).unwrap();
    let mut queued = Simulation::new(plan, 17, limits()).unwrap();
    let bounded = Limits {
        max_ticks: 2,
        ..limits()
    };
    let hitch = Duration::from_millis(3037);
    expected.advance(hitch, limits()).unwrap();
    queued.advance_queued(hitch, bounded).unwrap();
    assert_eq!(queued.elapsed(), Duration::from_millis(250));
    assert_eq!(queued.elapsed() + queued.pending(), hitch);
    for _ in 0..11 {
        queued.advance_queued(Duration::ZERO, bounded).unwrap();
    }
    assert_eq!(queued.pending(), Duration::from_millis(37));
    assert_eq!(queued.work, expected.work);
    assert_eq!(queued.particles(), expected.particles());
    expected
        .advance(Duration::from_millis(213), limits())
        .unwrap();
    queued
        .advance_queued(Duration::from_millis(213), bounded)
        .unwrap();
    assert_eq!(queued.work, expected.work);
    assert_eq!(queued.particles(), expected.particles());
}

#[test]
fn authored_rate_fraction_and_partition_are_deterministic_and_both_is_one_particle() {
    let mut p = properties();
    p.emission_rate = 2.5;
    let plan = plan(&[emitter(p)], &[], [100, 1100], true);
    let mut a = Simulation::new(Arc::clone(&plan), 17, limits()).unwrap();
    let mut b = Simulation::new(Arc::clone(&plan), 17, limits()).unwrap();
    a.advance(Duration::from_secs(1), limits()).unwrap();
    for _ in 0..8 {
        b.advance(Duration::from_millis(125), limits()).unwrap();
    }
    assert_eq!(a.work, b.work);
    assert_eq!(a.particles(), b.particles());
    assert_eq!(a.work.births, 2);
    a.advance(Duration::from_millis(250), limits()).unwrap();
    assert_eq!(a.work.births, 3);
    for particle in a.particles() {
        let appearance = plan.appearance(particle).unwrap();
        assert!(appearance.head_cell.is_some() && appearance.tail_cell.is_some());
    }
}

#[test]
fn source_extent_axes_are_halves_and_pivot_is_retained() {
    let mut p = properties();
    p.width = 2.;
    p.length = 40.;
    p.emission_rate = 512.;
    let plan = plan(&[emitter(p)], &[], [0, 1000], true);
    let mut s = Simulation::new(Arc::clone(&plan), 123, limits()).unwrap();
    s.advance(Duration::from_millis(125), limits()).unwrap();
    assert_eq!(s.particles().len(), 64);
    let mut max_x = 0f32;
    let mut max_y = 0f32;
    for particle in s.particles() {
        max_x = max_x.max((particle.position[0] - 7.).abs());
        max_y = max_y.max((particle.position[1] - 11.).abs());
        approx(particle.position[2], 13.);
    }
    assert!(max_x <= 1. && max_x > 0.8);
    assert!(max_y <= 20. && max_y > 16.);
}

#[test]
fn visibility_is_selected_clip_and_hidden_emitters_keep_live_particles_aging() {
    let mut e = emitter(properties());
    e.tracks.push(track(
        *b"KP2V",
        Interpolation::None,
        NO_ID,
        &[(2000, &[0.], &[], &[])],
    ));
    finish(&mut e);
    let p = plan(&[e.clone()], &[], [0, 1000], true);
    assert!(p.sample_emitter(0, 0).unwrap().visible);
    e.tracks = vec![track(
        *b"KP2V",
        Interpolation::None,
        NO_ID,
        &[(0, &[1.], &[], &[]), (250, &[0.], &[], &[])],
    )];
    finish(&mut e);
    let mut props = e.properties();
    props.emission_rate = 8.;
    e.state = props.to_bytes();
    let p = plan(&[e], &[], [0, 1000], false);
    let mut s = Simulation::new(Arc::clone(&p), 13, limits()).unwrap();
    s.advance(Duration::from_millis(125), limits()).unwrap();
    assert_eq!(s.work.births, 1);
    s.advance(Duration::from_millis(125), limits()).unwrap();
    assert_eq!(s.work.births, 1);
    approx(s.particles()[0].age as f32, 0.25);
}

#[test]
fn independent_long_global_clock_does_not_follow_clip_wrap_or_zero_duration() {
    let mut e = emitter(properties());
    e.node_tracks.push(track(
        *b"KGTR",
        Interpolation::Linear,
        0,
        &[
            (0, &[0., 0., 0.], &[], &[]),
            (13, &[13., 26., 39.], &[], &[]),
        ],
    ));
    finish(&mut e);
    let p = plan(&[e.clone()], &[13], [100, 117], true);
    let at = u64::MAX;
    assert_eq!(p.frame(at), 100 + (at % 17) as u32);
    let s = p.sample_emitter(0, at).unwrap();
    approx(s.translation[0], (at % 13) as f32);
    approx(s.translation[1], 2. * (at % 13) as f32);
    let p = plan(&[e], &[13], [300, 300], true);
    assert_eq!(p.frame(at), 300);
    approx(
        p.sample_emitter(0, at).unwrap().translation[2],
        3. * (at % 13) as f32,
    );
}

#[test]
fn signed_preroll_is_ordered_and_absent_local_keys_use_identity() {
    let mut e = emitter(properties());
    e.node_tracks.push(track(
        *b"KGTR",
        Interpolation::Linear,
        NO_ID,
        &[
            (-7, &[8., 9., 10.], &[], &[]),
            (2500, &[1., 2., 3.], &[], &[]),
        ],
    ));
    finish(&mut e);
    let p = plan(&[e.clone()], &[], [0, 1000], true);
    assert_eq!(p.sample_emitter(0, 400).unwrap().translation, [0.; 3]);
    e.node_tracks[0] = track(
        *b"KGTR",
        Interpolation::Linear,
        0,
        &[
            (i32::MIN, &[0., 0., 0.], &[], &[]),
            (i32::MAX, &[2., 4., 6.], &[], &[]),
        ],
    );
    finish(&mut e);
    let p = plan(&[e], &[13], [0, 1000], true);
    assert_eq!(p.sample_emitter(0, 0).unwrap().translation, [0.; 3]);
}

#[test]
fn bezier_translation_and_spherical_cubic_rotation_are_not_linear_replacements() {
    let mut e = emitter(properties());
    e.node_tracks.push(track(
        *b"KGTR",
        Interpolation::Bezier,
        NO_ID,
        &[
            (0, &[0., 0., 0.], &[0.; 3], &[8., 0., 0.]),
            (1000, &[4., 0., 0.], &[8., 0., 0.], &[0.; 3]),
        ],
    ));
    e.node_tracks.push(track(
        *b"KGRT",
        Interpolation::Hermite,
        NO_ID,
        &[
            (0, &[0., 0., 0., 1.], &[0., 0., 0., 1.], &[0., 0., 1., 0.]),
            (
                1000,
                &[0., 0., 1., 0.],
                &[0., 0., 1., 0.],
                &[0., 0., 1., 0.],
            ),
        ],
    ));
    finish(&mut e);
    let p = plan(&[e], &[], [0, 1000], false);
    let s = p.sample_emitter(0, 500).unwrap();
    approx(s.translation[0], 6.5);
    approx(s.rotation[2], (3. * std::f32::consts::PI / 8.).sin());
    approx(s.rotation[3], (3. * std::f32::consts::PI / 8.).cos());
}

#[test]
fn constant_gravity_matches_the_ballistic_path_across_tick_sizes() {
    let mut props = properties();
    props.speed = 8.;
    props.gravity = 8.;
    props.emission_rate = 8.;
    let p = plan(&[emitter(props)], &[], [0, 2000], true);
    for millis in [125, 250] {
        let mut policy = limits();
        policy.tick = Duration::from_millis(millis);
        let mut simulation = Simulation::new(Arc::clone(&p), 11, policy).unwrap();
        simulation.advance(Duration::from_secs(1), policy).unwrap();
        let first = simulation.particles()[0];
        assert_eq!(first.age, 1.);
        assert_eq!(first.velocity, [0., 0., 0.]);
        assert_eq!(first.position, [7., 11., 17.]);
    }
}

#[test]
fn signed_speed_node_rotation_translation_and_ballistic_gravity_are_preserved() {
    let mut p = properties();
    p.speed = -4.;
    p.gravity = 8.;
    p.emission_rate = 8.;
    let mut e = emitter(p);
    e.node_tracks.push(track(
        *b"KGTR",
        Interpolation::None,
        NO_ID,
        &[(0, &[2., 3., 4.], &[], &[])],
    ));
    e.node_tracks.push(track(
        *b"KGRT",
        Interpolation::Linear,
        NO_ID,
        &[(0, &[0., 1., 0., 0.], &[], &[])],
    ));
    finish(&mut e);
    let p = plan(&[e], &[], [0, 1000], true);
    let mut s = Simulation::new(Arc::clone(&p), 1, limits()).unwrap();
    s.advance(Duration::from_millis(125), limits()).unwrap();
    let x = s.particles()[0];
    assert_eq!(x.position[0], 9.);
    assert_eq!(x.position[1], 14.);
    approx(x.velocity[2], 3.);
    approx(x.position[2], 17.4375);
}

#[test]
fn colour_alpha_size_and_tail_follow_two_source_phases() {
    let p = plan(&[emitter(properties())], &[], [0, 1000], true);
    let a = p.appearance(&live(0.5)).unwrap();
    assert_eq!(a.rgba[0], 0.5);
    assert_eq!(a.rgba[1], 0.5);
    approx(a.rgba[3], 64. / 255.);
    assert_eq!(a.scale, 2.);
    assert_eq!(a.tail_start, [0., 5., 8.]);
    let b = p.appearance(&live(2.5)).unwrap();
    assert_eq!(b.rgba[0], 0.);
    assert_eq!(b.rgba[1], 0.5);
    assert_eq!(b.rgba[2], 0.5);
    approx(b.rgba[3], 191.5 / 255.);
    assert_eq!(b.scale, 5.);
    assert_eq!(b.priority_plane, -7);
    assert_eq!(b.texture_flags, pcx_mdl::TEXTURE_WRAP_WIDTH);
}

#[test]
fn inclusive_atlas_interval_reaches_its_last_frame() {
    let mut props = properties();
    props.rows = 1;
    props.columns = 2;
    props.head_or_tail = 0;
    props.life_span_uv_anim = [0, 1, 1];
    let p = plan(&[emitter(props)], &[], [0, 1000], true);
    assert_eq!(p.appearance(&live(0.75)).unwrap().head_cell, Some(1));
}

#[test]
fn native_appearance_uses_supplied_coefficients_for_colour_size_and_both_atlas_directions() {
    let mut props = properties();
    props.rows = 1;
    props.columns = 4;
    props.segment_colours = [[0., 1., 0.5], [1., 0., 0.5], [0.; 3]];
    props.segment_alpha = [0, 255, 0];
    props.life_span_uv_anim = [1, 3, 1];
    props.tail_uv_anim = [3, 1, 7];
    let p = Arc::try_unwrap(plan(&[emitter(props)], &[], [0, 1000], true))
        .unwrap()
        .with_native_appearance(
            "synthetic appearance operands",
            [0.5, 0.25],
            [128., 1024.],
            13,
        )
        .unwrap();
    let a = p.appearance(&live(0.)).unwrap();
    assert_eq!(a.rgba, [32., 96., 64., 63.].map(|v| v / 255.));
    assert_eq!(a.scale, 1.5);
    assert_eq!((a.head_cell, a.tail_cell), (Some(1), Some(3)));
    let b = p.appearance(&live(0.5)).unwrap();
    assert_eq!(b.rgba, [64., 64., 64., 127.].map(|v| v / 255.));
    assert_eq!(b.scale, 2.);
    assert_eq!((b.head_cell, b.tail_cell), (Some(2), Some(2)));
    let c = p.appearance(&live(0.999)).unwrap();
    assert_eq!((c.head_cell, c.tail_cell), (Some(3), Some(1)));
    assert_eq!(p.source(), "synthetic:markers");
    assert_eq!(p.appearance_source(), Some("synthetic appearance operands"));
}

#[test]
fn native_appearance_refuses_unusable_phase_or_float_packing_parameters() {
    for (source, phase, colour, shift) in [
        ("", [0.5, 0.25], [128., 1024.], 13),
        ("fixture", [f32::NAN, 0.25], [128., 1024.], 13),
        ("fixture", [0.75, 0.5], [128., 1024.], 13),
        ("fixture", [-0.5, 0.25], [128., 1024.], 13),
        ("fixture", [0.5, 0.25], [0., 1024.], 13),
        ("fixture", [0.5, 0.25], [128., 1023.], 13),
        ("fixture", [0.5, 0.25], [128., 1024.], 32),
        ("fixture", [0.5, 0.25], [128., 1024.], 14),
    ] {
        let p = Arc::try_unwrap(plan(&[emitter(properties())], &[], [0, 1000], true)).unwrap();
        assert!(
            p.with_native_appearance(source, phase, colour, shift)
                .is_err()
        );
    }
}

#[test]
fn native_appearance_refuses_non_power_of_two_atlas_dimensions() {
    let mut props = properties();
    props.columns = 3;
    let p = Arc::try_unwrap(plan(&[emitter(props)], &[], [0, 1000], true)).unwrap();
    let error = p
        .with_native_appearance("synthetic", [0.5, 0.25], [128., 1024.], 13)
        .unwrap_err();
    assert!(matches!(error.failure, Failure::Unsupported(_)));
}

#[test]
fn asymmetric_atlas_repeat_inclusive_end_and_single_cell_are_distinct() {
    let p = plan(&[emitter(properties())], &[], [0, 1000], true);
    let a = p.appearance(&live(0.3)).unwrap();
    assert_eq!(a.head_cell, Some(4));
    assert_eq!(a.tail_cell, Some(4));
    let corners = a.cell_corners(true).unwrap();
    assert_eq!(
        corners,
        [[2. / 3., 0.5], [1. / 3., 0.5], [1. / 3., 1.], [2. / 3., 1.]]
    );
    let mut prop = properties();
    prop.rows = 1;
    prop.columns = 1;
    prop.life_span_uv_anim = [0, 64, 2];
    let p = plan(&[emitter(prop)], &[], [0, 1000], true);
    assert_eq!(p.appearance(&live(0.3)).unwrap().head_cell, Some(0));
}

#[test]
fn maximum_static_atlas_row_has_no_integer_overflow() {
    let mut props = properties();
    props.rows = 1;
    props.columns = 1;
    props.life_span_uv_anim = [u32::MAX, u32::MAX, 0];
    let p = plan(&[emitter(props)], &[], [0, 1000], true);
    let corners = p
        .appearance(&live(0.3))
        .unwrap()
        .cell_corners(false)
        .unwrap();
    assert!(corners.into_iter().flatten().all(f32::is_finite));
}

#[test]
fn inactive_tail_intervals_do_not_refuse_head_only_and_zero_phase_is_defined() {
    let mut p = properties();
    p.head_or_tail = 0;
    p.tail_uv_anim = [9, 1, 0];
    p.tail_decay_uv_anim = [8, 2, 0];
    p.time = 0.;
    p.tail_length = f32::MAX;
    let plan = plan(&[emitter(p)], &[], [0, 1000], true);
    let a = plan.appearance(&live(0.)).unwrap();
    assert_eq!(a.scale, 3.);
    assert_eq!(a.tail_cell, None);
    assert_eq!(a.tail_start, live(0.).position);
}

#[test]
fn tick_and_update_budgets_fail_before_particle_mutation_and_allow_work_retry() {
    let p = plan(&[emitter(properties())], &[], [0, 1000], true);
    let mut s = Simulation::new(Arc::clone(&p), 7, limits()).unwrap();
    let mut constrained = limits();
    constrained.max_ticks = 1;
    assert!(matches!(
        s.advance(Duration::from_secs(1), constrained)
            .unwrap_err()
            .failure,
        Failure::Budget { kind: "ticks", .. }
    ));
    assert_eq!(s.elapsed(), Duration::ZERO);
    assert_eq!(s.pending(), Duration::ZERO);
    s.advance(Duration::from_millis(250), limits()).unwrap();
    let particles = s.particles().to_vec();
    let old = s.work;
    constrained = limits();
    constrained.max_updates = 0;
    assert!(matches!(
        s.advance(Duration::from_millis(125), constrained)
            .unwrap_err()
            .failure,
        Failure::Budget {
            kind: "updates",
            ..
        }
    ));
    assert_eq!(s.work, old);
    assert_eq!(s.particles(), particles);
    assert_eq!(s.pending(), Duration::from_millis(125));
    s.advance(Duration::ZERO, limits()).unwrap();
    assert_eq!(s.elapsed(), Duration::from_millis(375));
}

#[test]
fn capacity_is_checked_atomically_without_dropping_emissions() {
    let mut l = limits();
    l.particle_capacity = 1;
    let mut props = properties();
    props.emission_rate = 16.;
    let p = plan(&[emitter(props)], &[], [0, 1000], true);
    let mut s = Simulation::new(Arc::clone(&p), 7, l).unwrap();
    let error = s.advance(Duration::from_millis(125), l).unwrap_err();
    assert_eq!(error.emitter, Some(0));
    assert!(matches!(
        error.failure,
        Failure::Budget {
            kind: "particles",
            needed: 2,
            limit: 1
        }
    ));
    assert_eq!(s.elapsed(), Duration::ZERO);
    assert_eq!(s.work.births, 0);
    assert!(s.particles().is_empty());
}

#[test]
fn malformed_and_unsupported_inputs_fail_with_source_and_emitter_context() {
    let mut cases = vec![];
    let mut e = emitter(properties());
    e.parent_id = 12;
    cases.push((e, Failure::Unsupported("parented emitter")));
    let mut p = properties();
    p.squirt = 1;
    cases.push((
        emitter(p),
        Failure::Unsupported("burst emission needs a STEP rate channel"),
    ));
    let mut e = emitter(properties());
    e.node_tracks.push(track(
        *b"KGTR",
        Interpolation::Linear,
        0,
        &[(0, &[0., 0., 0.], &[], &[])],
    ));
    finish(&mut e);
    cases.push((e, Failure::Malformed("global reference")));
    let mut e = emitter(properties());
    e.tracks.push(track(
        *b"KP2G",
        Interpolation::None,
        NO_ID,
        &[(0, &[1.], &[], &[])],
    ));
    finish(&mut e);
    cases.push((e, Failure::Unsupported("particle scalar channel")));
    let mut p = properties();
    p.speed = f32::NAN;
    cases.push((emitter(p), Failure::Malformed("particle scalars")));
    let mut e = emitter(properties());
    e.node_tracks.push(track(
        *b"KGTR",
        Interpolation::Linear,
        NO_ID,
        &[(5, &[0., 0., 0.], &[], &[]), (-1, &[1., 0., 0.], &[], &[])],
    ));
    finish(&mut e);
    cases.push((e, Failure::Malformed("duplicate/unordered signed keys")));
    for (e, expected) in cases {
        let error = Plan::checked(
            &[e],
            &texture(),
            &Pivots(vec![[0.; 3]]),
            &[],
            Clip {
                interval: [0, 1000],
                looping: true,
            },
            "fixture:edition/hash/member",
            limits(),
        )
        .unwrap_err();
        assert_eq!(error.failure, expected);
        assert_eq!(error.emitter, Some(0));
        assert_eq!(error.source_name, "fixture:edition/hash/member");
    }
}

#[test]
#[ignore = "requires explicit retained TFT model path; reads in place, no asset outputs"]
fn explicit_tft_original_admission_and_cpu_steps() {
    let path = std::env::var("PCX_TFT_PRE2_MODEL").expect("explicit source fixture path");
    let bytes = std::fs::read(path).unwrap();
    let mdl = pcx_mdl::Mdlx::parse(&bytes).unwrap();
    let emitters = mdl.particles2(&bytes).unwrap();
    let textures = mdl.textures(&bytes).unwrap();
    let pivots = mdl.pivots(&bytes).unwrap();
    let glbs = mdl.payload(&bytes, b"GLBS").unwrap();
    assert_eq!(glbs.len() % 4, 0);
    let globals: Vec<u32> = glbs
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| u32::from_le_bytes(*b))
        .collect();
    let sequences = mdl.sequences(&bytes).unwrap();
    let mut l = limits();
    l.particle_capacity = 100000;
    l.max_updates = 100_000_000;
    l.max_ticks = 10000;
    l.tick = Duration::from_millis(10);
    let mut totals = (0u64, 0u64, 0u64);
    let mut gapped_channels = 0usize;
    for clip in &sequences {
        for e in &emitters {
            for t in e.node_tracks.iter().chain(&e.tracks) {
                let (start, end) = if t.global_seq_id == NO_ID {
                    (clip.interval_start, clip.interval_end)
                } else {
                    (0, globals[t.global_seq_id as usize])
                };
                let keys: Vec<i32> = (0..t.count as usize)
                    .map(|i| t.frame_signed(i).unwrap())
                    .filter(|f| {
                        i64::from(*f) >= i64::from(start) && i64::from(*f) <= i64::from(end)
                    })
                    .collect();
                if keys.len() > 1
                    && (i64::from(keys[0]) != i64::from(start)
                        || i64::from(*keys.last().unwrap()) != i64::from(end))
                {
                    gapped_channels += 1;
                }
            }
        }
    }
    println!("selected multi-key channels lacking boundary keys: {gapped_channels}");
    for clip in &sequences {
        let p=Arc::new(Plan::checked(&emitters,&textures,&pivots,&globals,Clip{interval:[clip.interval_start,clip.interval_end],looping:!clip.non_looping},"TFT:selected-prepared-member:15586ffd9dc25b82a4f8ce2e679581f6e14cbcdc0916c7422d89308f010f6293",l).unwrap());
        assert_eq!(p.profiles().len(), emitters.len());
        for i in 0..emitters.len() {
            for t in [0, u64::from(clip.duration()), u64::MAX] {
                let s = p.sample_emitter(i, t).unwrap();
                assert!(
                    s.translation
                        .into_iter()
                        .chain(s.rotation)
                        .all(f32::is_finite)
                );
            }
            let a = p
                .appearance(&Particle {
                    emitter: i,
                    age: f64::from(p.profiles()[i].properties.life_span) * 0.3,
                    position: [0.; 3],
                    velocity: [1., 2., 3.],
                    gravity: 0.,
                })
                .unwrap();
            assert!(a.rgba.into_iter().all(f32::is_finite));
        }
        let mut sim = Simulation::new(Arc::clone(&p), 1, l).unwrap();
        let work = sim.advance(Duration::from_secs(10), l).unwrap();
        for particle in sim.particles() {
            p.appearance(particle).unwrap();
        }
        totals.0 += work.births;
        totals.1 += work.updates;
        totals.2 += sim.particles().len() as u64;
    }
    println!(
        "qualified TFT CPU gate: emitters={}, clips={}, summed births={}, updates={}, retained={}; no rendered acceptance",
        emitters.len(),
        sequences.len(),
        totals.0,
        totals.1,
        totals.2
    );
}

// Keep public renderer-bound output in the external-consumer API check.
const _: Option<Appearance> = None;

#[test]
fn retained_cyclic_gap_and_equal_value_conventions_are_explicit() {
    let mut e = emitter(properties());
    e.node_tracks.push(track(
        *b"KGTR",
        Interpolation::Linear,
        NO_ID,
        &[
            (250, &[0., 0., 0.], &[], &[]),
            (750, &[4., 0., 0.], &[], &[]),
        ],
    ));
    finish(&mut e);
    let p = plan(&[e.clone()], &[], [0, 1000], false);
    approx(p.sample_emitter(0, 875).unwrap().translation[0], 3.);
    // This negative gap factor is the pinned SdSequence behavior before its first key.
    approx(p.sample_emitter(0, 125).unwrap().translation[0], 5.);
    e.node_tracks = vec![track(
        *b"KGTR",
        Interpolation::Bezier,
        NO_ID,
        &[
            (0, &[2., 0., 0.], &[0.; 3], &[8., 0., 0.]),
            (1000, &[2., 0., 0.], &[8., 0., 0.], &[0.; 3]),
        ],
    )];
    finish(&mut e);
    let p = plan(&[e], &[], [0, 1000], false);
    approx(p.sample_emitter(0, 500).unwrap().translation[0], 2.);
}

#[test]
fn non_line_emitters_stay_inside_the_authored_round_cone() {
    for latitude in [10., 30., 60.] {
        let mut props = properties();
        props.speed = 8.;
        props.latitude = latitude;
        props.emission_rate = 1024.;
        let p = plan(&[emitter(props)], &[], [0, 1000], true);
        let mut s = Simulation::new(p, 11, limits()).unwrap();
        s.advance(Duration::from_millis(125), limits()).unwrap();
        assert_eq!(s.particles().len(), 128);
        let minimum_z = props.speed * latitude.to_radians().cos();
        let mut quadrants = [false; 4];
        for particle in s.particles() {
            let [x, y, z] = particle.velocity;
            approx((x * x + y * y + z * z).sqrt(), props.speed);
            assert!(
                z >= minimum_z - 0.00002,
                "latitude {latitude}: velocity {:?} leaves the cone",
                particle.velocity,
            );
            quadrants[usize::from(x < 0.) + 2 * usize::from(y < 0.)] = true;
        }
        assert!(quadrants.into_iter().all(|seen| seen));
    }
}

#[test]
fn line_angular_plane_variation_and_xy_facing_are_source_driven() {
    let mut props = properties();
    props.speed = 8.;
    props.variation = 0.6;
    props.latitude = 47.;
    props.emission_rate = 256.;
    let mut line = emitter(props);
    line.flags |=
        mdx_particles::LINE | mdx_particles::XY_QUAD | mdx_particles::PARTICLE_EMITTER_TYPE;
    let p = plan(&[line.clone()], &[], [0, 1000], true);
    let mut s = Simulation::new(Arc::clone(&p), 11, limits()).unwrap();
    s.advance(Duration::from_millis(125), limits()).unwrap();
    for particle in s.particles() {
        assert!(particle.velocity[0].abs() < 0.00001);
        let speed = particle
            .velocity
            .into_iter()
            .map(|v| v * v)
            .sum::<f32>()
            .sqrt();
        assert!((3.1999..=12.8001).contains(&speed));
        let a = p.appearance(particle).unwrap();
        approx(
            a.xy_facing.unwrap(),
            particle.velocity[1].atan2(particle.velocity[0]) - 7. * std::f32::consts::PI / 8.,
        );
    }
    line.flags &= !mdx_particles::LINE;
    let p = plan(&[line], &[], [0, 1000], true);
    let mut s = Simulation::new(Arc::clone(&p), 11, limits()).unwrap();
    s.advance(Duration::from_millis(125), limits()).unwrap();
    assert!(
        s.particles()
            .iter()
            .any(|particle| particle.velocity[0].abs() > 0.1)
    );
}

#[test]
fn known_type_bit_is_inert_and_unknown_flag_is_contextually_refused() {
    let mut e = emitter(properties());
    e.flags |= mdx_particles::PARTICLE_EMITTER_TYPE;
    let p = plan(&[e.clone()], &[], [0, 1000], true);
    assert_eq!(p.appearance(&live(0.1)).unwrap().flags, e.flags);
    e.flags |= 1 << 31;
    let error = Plan::checked(
        &[e],
        &texture(),
        &Pivots(vec![[0.; 3]]),
        &[],
        Clip {
            interval: [0, 1000],
            looping: true,
        },
        "synthetic:unknownflag",
        limits(),
    )
    .unwrap_err();
    assert_eq!(error.failure, Failure::Unsupported("unknown PRE2 flags"));
    assert_eq!(error.emitter, Some(0));
}

#[test]
fn global_quaternion_sampling_uses_its_own_duration() {
    let mut e = emitter(properties());
    e.node_tracks.push(track(
        *b"KGRT",
        Interpolation::Linear,
        0,
        &[
            (0, &[0., 0., 0., 1.], &[], &[]),
            (16, &[0., 0., 1., 0.], &[], &[]),
        ],
    ));
    finish(&mut e);
    let p = plan(&[e], &[16], [300, 303], true);
    let at = u64::MAX - 7;
    assert_eq!(at % 16, 8);
    let q = p.sample_emitter(0, at).unwrap().rotation;
    approx(q[2], std::f32::consts::FRAC_1_SQRT_2);
    approx(q[3], std::f32::consts::FRAC_1_SQRT_2);
}

#[test]
fn huge_authored_rate_and_invalid_public_particle_fail_without_panics() {
    let mut props = properties();
    props.emission_rate = f32::MAX;
    let p = plan(&[emitter(props)], &[], [0, 1000], true);
    let mut s = Simulation::new(Arc::clone(&p), 1, limits()).unwrap();
    assert!(matches!(
        s.advance(Duration::from_millis(125), limits())
            .unwrap_err()
            .failure,
        Failure::Budget {
            kind: "particles",
            ..
        }
    ));
    assert_eq!(s.work.births, 0);
    let mut particle = live(0.);
    particle.position[0] = f32::NAN;
    assert_eq!(
        p.appearance(&particle).unwrap_err().failure,
        Failure::Arithmetic
    );
}

#[test]
fn simulation_owns_plan_without_a_self_reference_or_leak() {
    let p = plan(&[emitter(properties())], &[], [0, 4000], true);
    let weak = Arc::downgrade(&p);
    let mut simulation = Simulation::new(Arc::clone(&p), 19, limits()).unwrap();
    drop(p);
    assert!(weak.upgrade().is_some());
    simulation
        .advance(Duration::from_millis(250), limits())
        .unwrap();
    assert_eq!(simulation.plan().profiles().len(), 1);
    drop(simulation);
    assert!(weak.upgrade().is_none());
}

#[test]
fn model_space_particles_follow_current_emitter_pose_without_reintegrating_world_motion() {
    let mut props = properties();
    props.emission_rate = 8.;
    let mut e = emitter(props);
    e.flags |= mdx_particles::MODEL_SPACE;
    e.node_tracks.push(track(
        *b"KGTR",
        Interpolation::Linear,
        NO_ID,
        &[
            (0, &[0., 0., 0.], &[], &[]),
            (1000, &[8., 0., 0.], &[], &[]),
        ],
    ));
    finish(&mut e);
    let p = plan(&[e.clone()], &[], [0, 1000], false);
    let mut simulation = Simulation::new(Arc::clone(&p), 17, limits()).unwrap();
    simulation
        .advance(Duration::from_millis(500), limits())
        .unwrap();
    assert_eq!(simulation.particles().len(), 4);
    for particle in simulation.particles() {
        assert_eq!(particle.position, [7., 11., 13.]);
        assert_eq!(
            p.world_particle(particle, 500).unwrap().position,
            [11., 11., 13.]
        );
        assert_eq!(
            p.world_particle(particle, 1000).unwrap().position,
            [15., 11., 13.]
        );
    }
    e.flags &= !mdx_particles::MODEL_SPACE;
    let p = plan(&[e], &[], [0, 1000], false);
    let mut world = Simulation::new(Arc::clone(&p), 17, limits()).unwrap();
    world.advance(Duration::from_millis(500), limits()).unwrap();
    assert_eq!(world.particles()[0].position, [8., 11., 13.]);
    assert_eq!(
        p.world_particle(&world.particles()[0], 1000)
            .unwrap()
            .position,
        [8., 11., 13.]
    );
}

#[test]
fn model_space_pose_rotates_displacement_and_velocity_about_the_authored_pivot() {
    let mut e = emitter(properties());
    e.flags |= mdx_particles::MODEL_SPACE;
    let q = std::f32::consts::FRAC_1_SQRT_2;
    e.node_tracks.push(track(
        *b"KGRT",
        Interpolation::None,
        NO_ID,
        &[(0, &[0., q, 0., q], &[], &[])],
    ));
    finish(&mut e);
    let p = plan(&[e], &[], [0, 1000], false);
    let particle = Particle {
        position: [8., 11., 13.],
        velocity: [0., 0., 2.],
        ..live(0.5)
    };
    let world = p.world_particle(&particle, 500).unwrap();
    for (a, b) in world.position.into_iter().zip([7., 11., 12.]) {
        approx(a, b);
    }
    for (a, b) in world.velocity.into_iter().zip([2., 0., 0.]) {
        approx(a, b);
    }
    assert_eq!(particle.position, [8., 11., 13.]);
}

fn burst_plan(keys: &[TrackKey<'_>], global: u32, globals: &[u32], looping: bool) -> Arc<Plan> {
    let mut props = properties();
    props.squirt = 1;
    props.emission_rate = 0.;
    let mut e = emitter(props);
    e.tracks
        .push(track(*b"KP2E", Interpolation::None, global, keys));
    finish(&mut e);
    plan(&[e], globals, [0, 1000], looping)
}

fn entrance_plan(e: &[Particle2], globals: &[u32]) -> Arc<Plan> {
    let first = Arc::try_unwrap(plan(e, globals, [10, 260], false)).unwrap();
    let next = Arc::try_unwrap(plan(e, globals, [1000, 1500], true)).unwrap();
    Arc::new(first.then(next).unwrap())
}

#[test]
fn entrance_changes_local_tracks_without_resetting_particles_rng_or_global_visibility() {
    let mut e = emitter(properties());
    e.node_tracks.push(track(
        *b"KGTR",
        Interpolation::Linear,
        NO_ID,
        &[
            (10, &[0., 0., 0.], &[], &[]),
            (260, &[10., 0., 0.], &[], &[]),
            (1000, &[40., 0., 0.], &[], &[]),
            (1500, &[60., 0., 0.], &[], &[]),
        ],
    ));
    e.tracks.push(track(
        *b"KP2V",
        Interpolation::None,
        0,
        &[(0, &[1.], &[], &[]), (500, &[0.], &[], &[])],
    ));
    finish(&mut e);
    let p = entrance_plan(&[e], &[1000]);
    assert_eq!(
        [p.frame(249), p.frame(250), p.frame(375), p.frame(875)],
        [259, 1000, 1125, 1125]
    );
    approx(p.sample_emitter(0, 375).unwrap().translation[0], 45.);
    assert!(
        !p.sample_emitter(0, 750).unwrap().visible,
        "GLBS does not restart at Stand"
    );
    let mut a = Simulation::new(Arc::clone(&p), 17, limits()).unwrap();
    a.advance(Duration::from_millis(250), limits()).unwrap();
    assert_eq!(a.particles().len(), 1);
    a.advance(Duration::from_millis(125), limits()).unwrap();
    assert_eq!(
        a.particles().len(),
        1,
        "fractional emission must survive the handoff"
    );
    approx(a.particles()[0].age as f32, 0.25);
    a.advance(Duration::from_millis(625), limits()).unwrap();
    let mut b = Simulation::new(p, 17, limits()).unwrap();
    b.advance(Duration::from_secs(1), limits()).unwrap();
    assert_eq!(a.work, b.work);
    assert_eq!(a.particles(), b.particles());
}

#[test]
fn entrance_local_bursts_start_once_and_global_bursts_keep_their_clock() {
    for global in [NO_ID, 0] {
        let mut props = properties();
        props.squirt = 1;
        props.emission_rate = 0.;
        let mut e = emitter(props);
        let keys: &[TrackKey<'_>] = if global == NO_ID {
            &[
                (10, &[1.6], &[], &[]),
                (135, &[0.], &[], &[]),
                (1000, &[2.4], &[], &[]),
                (1125, &[0.], &[], &[]),
                (1250, &[1.], &[], &[]),
            ]
        } else {
            &[
                (0, &[1.6], &[], &[]),
                (125, &[0.], &[], &[]),
                (250, &[2.4], &[], &[]),
                (375, &[0.], &[], &[]),
            ]
        };
        e.tracks
            .push(track(*b"KP2E", Interpolation::None, global, keys));
        finish(&mut e);
        let globals = if global == NO_ID { &[][..] } else { &[500][..] };
        let p = entrance_plan(&[e.clone()], globals);
        let mut a = Simulation::new(Arc::clone(&p), 37, limits()).unwrap();
        a.advance(Duration::from_secs(1), limits()).unwrap();
        let mut b = Simulation::new(p, 37, limits()).unwrap();
        for _ in 0..8 {
            b.advance(Duration::from_millis(125), limits()).unwrap();
        }
        assert_eq!(a.work, b.work);
        assert_eq!(a.particles(), b.particles());
        assert_eq!(a.work.births, if global == NO_ID { 8 } else { 9 });
        if global != NO_ID {
            let mut baseline =
                Simulation::new(plan(&[e], globals, [0, 1000], true), 37, limits()).unwrap();
            baseline.advance(Duration::from_secs(1), limits()).unwrap();
            assert_eq!(a.work.births, baseline.work.births);
            assert_eq!(a.particles(), baseline.particles());
        }
    }
}

#[test]
fn entrance_rejects_a_different_emitter_batch_or_global_channel() {
    let mut a = emitter(properties());
    a.tracks.push(track(
        *b"KP2V",
        Interpolation::None,
        0,
        &[(0, &[1.], &[], &[])],
    ));
    finish(&mut a);
    let first = || Arc::try_unwrap(plan(&[a.clone()], &[500], [10, 260], false)).unwrap();
    let mut other = a.clone();
    other.state = {
        let mut p = properties();
        p.life_span = 9.;
        p.to_bytes()
    };
    finish(&mut other);
    assert!(
        first()
            .then(Arc::try_unwrap(plan(&[other], &[500], [1000, 1500], true)).unwrap())
            .is_err()
    );
    let mut other = a.clone();
    other.tracks[0] = track(*b"KP2V", Interpolation::None, 0, &[(0, &[0.], &[], &[])]);
    finish(&mut other);
    assert!(
        first()
            .then(Arc::try_unwrap(plan(&[other], &[500], [1000, 1500], true)).unwrap())
            .is_err()
    );
    let looping = Arc::try_unwrap(plan(&[a.clone()], &[500], [10, 260], true)).unwrap();
    assert!(looping.then(first()).is_err());
}

#[test]
fn burst_keys_retain_fractional_carry_across_loops_and_frame_partitions() {
    let p = burst_plan(
        &[
            (125, &[1.6], &[], &[]),
            (250, &[0.], &[], &[]),
            (500, &[2.4], &[], &[]),
            (750, &[0.], &[], &[]),
        ],
        NO_ID,
        &[],
        true,
    );
    let mut a = Simulation::new(Arc::clone(&p), 17, limits()).unwrap();
    let mut b = Simulation::new(p, 17, limits()).unwrap();
    a.advance(Duration::from_secs(2), limits()).unwrap();
    for _ in 0..16 {
        b.advance(Duration::from_millis(125), limits()).unwrap();
    }
    assert_eq!(a.work.births, 8);
    assert_eq!(a.work, b.work);
    assert_eq!(a.particles(), b.particles());
}

#[test]
fn burst_keys_inside_one_tick_and_global_loops_are_not_skipped() {
    let p = burst_plan(
        &[
            (0, &[0.], &[], &[]),
            (25, &[1.], &[], &[]),
            (50, &[0.], &[], &[]),
            (75, &[2.], &[], &[]),
            (100, &[0.], &[], &[]),
        ],
        NO_ID,
        &[],
        false,
    );
    let mut simulation = Simulation::new(p, 17, limits()).unwrap();
    simulation
        .advance(Duration::from_millis(125), limits())
        .unwrap();
    assert_eq!(simulation.work.births, 3);
    let p = burst_plan(
        &[
            (0, &[0.], &[], &[]),
            (50, &[1.], &[], &[]),
            (100, &[0.], &[], &[]),
        ],
        0,
        &[250],
        false,
    );
    let mut simulation = Simulation::new(p, 17, limits()).unwrap();
    simulation
        .advance(Duration::from_secs(2), limits())
        .unwrap();
    assert_eq!(
        simulation.work.births, 8,
        "global emission continues after a nonlooping local clip"
    );
}

#[test]
fn burst_initial_key_fires_once_and_capacity_failure_is_transactional() {
    let p = burst_plan(
        &[(0, &[3.], &[], &[]), (500, &[0.], &[], &[])],
        NO_ID,
        &[],
        false,
    );
    let mut simulation = Simulation::new(Arc::clone(&p), 17, limits()).unwrap();
    simulation
        .advance(Duration::from_secs(2), limits())
        .unwrap();
    assert_eq!(simulation.work.births, 3);
    let tiny = Limits {
        particle_capacity: 2,
        ..limits()
    };
    let mut simulation = Simulation::new(p, 17, tiny).unwrap();
    assert!(matches!(
        simulation
            .advance(Duration::from_millis(125), tiny)
            .unwrap_err()
            .failure,
        Failure::Budget {
            kind: "particles",
            ..
        }
    ));
    assert!(simulation.particles().is_empty());
    assert_eq!(simulation.elapsed(), Duration::ZERO);
    assert_eq!(simulation.work.births, 0);
}

#[test]
fn animated_bezier_emission_uses_the_track_instead_of_the_static_rate() {
    let mut props = properties();
    props.emission_rate = 1000.;
    let mut e = emitter(props);
    e.tracks.push(track(
        *b"KP2E",
        Interpolation::Bezier,
        NO_ID,
        &[(0, &[0.], &[0.], &[0.]), (1000, &[8.], &[8.], &[8.])],
    ));
    finish(&mut e);
    let p = plan(&[e], &[], [0, 1000], false);
    let mut simulation = Simulation::new(p, 17, limits()).unwrap();
    simulation
        .advance(Duration::from_millis(500), limits())
        .unwrap();
    assert_eq!(simulation.work.births, 1);
    simulation
        .advance(Duration::from_millis(500), limits())
        .unwrap();
    assert_eq!(simulation.work.births, 4);
}

#[test]
fn burst_key_scans_obey_the_work_budget_before_emitting() {
    let p = burst_plan(
        &[
            (0, &[0.], &[], &[]),
            (25, &[1.], &[], &[]),
            (50, &[0.], &[], &[]),
        ],
        NO_ID,
        &[],
        false,
    );
    let constrained = Limits {
        max_updates: 2,
        ..limits()
    };
    let mut simulation = Simulation::new(p, 17, constrained).unwrap();
    assert!(matches!(
        simulation
            .advance(Duration::from_millis(125), constrained)
            .unwrap_err()
            .failure,
        Failure::Budget {
            kind: "updates",
            needed: 3,
            limit: 2
        }
    ));
    assert!(simulation.particles().is_empty());
    assert_eq!(simulation.elapsed(), Duration::ZERO);
    assert_eq!(simulation.work, mdx_particles::Work::default());
}

#[test]
fn negative_and_duplicate_rate_channels_and_nonstep_bursts_refuse() {
    for (squirt, interpolation, negative, duplicate) in [
        (0, Interpolation::Linear, true, false),
        (0, Interpolation::None, false, true),
        (1, Interpolation::Linear, false, false),
    ] {
        let mut props = properties();
        props.squirt = squirt;
        let mut e = emitter(props);
        let rate = track(
            *b"KP2E",
            interpolation,
            NO_ID,
            &[(0, &[if negative { -1. } else { 1. }], &[], &[])],
        );
        e.tracks.push(rate.clone());
        if duplicate {
            e.tracks.push(rate);
        }
        finish(&mut e);
        let error = Plan::checked(
            &[e],
            &texture(),
            &Pivots(vec![[0.; 3]]),
            &[],
            Clip {
                interval: [0, 1000],
                looping: true,
            },
            "synthetic:bad-rate",
            limits(),
        )
        .unwrap_err();
        assert!(matches!(
            error.failure,
            Failure::Malformed(_) | Failure::Unsupported(_)
        ));
        assert_eq!(error.source_name, "synthetic:bad-rate");
        assert_eq!(error.emitter, Some(0));
    }
}

#[test]
fn executed_clip_changes_preserve_live_particles_rng_remainders_and_global_time() {
    let mut e = emitter(properties());
    e.tracks.push(track(
        *b"KP2V",
        Interpolation::None,
        0,
        &[(0, &[1.], &[], &[]), (500, &[0.], &[], &[])],
    ));
    finish(&mut e);
    let mut changed =
        Simulation::new(plan(&[e.clone()], &[1000], [101, 401], true), 37, limits()).unwrap();
    let mut control =
        Simulation::new(plan(&[e.clone()], &[1000], [101, 401], true), 37, limits()).unwrap();
    for simulation in [&mut changed, &mut control] {
        simulation
            .advance(Duration::from_millis(375), limits())
            .unwrap();
    }
    let before = changed.particles().to_vec();
    changed
        .select_plan(
            Arc::try_unwrap(plan(&[e.clone()], &[1000], [1703, 1853], false)).unwrap(),
            Duration::from_millis(375),
        )
        .unwrap();
    assert_eq!(changed.particles(), before);
    assert_eq!(changed.elapsed(), Duration::from_millis(375));
    assert_eq!(changed.plan().frame(375), 1703);
    assert_eq!(changed.plan().frame(500), 1828);
    assert_eq!(changed.plan().frame(625), 1853);
    assert!(!changed.plan().sample_emitter(0, 625).unwrap().visible);
    for simulation in [&mut changed, &mut control] {
        simulation
            .advance(Duration::from_millis(750), limits())
            .unwrap();
    }
    assert_eq!(changed.particles(), control.particles());
    let before = changed.particles().to_vec();
    let mut different = e;
    let mut different_properties = different.properties();
    different_properties.speed = 17.;
    different.state = different_properties.to_bytes();
    assert!(
        changed
            .select_plan(
                Arc::try_unwrap(plan(&[different], &[1000], [1703, 1853], false)).unwrap(),
                Duration::from_millis(1125)
            )
            .is_err()
    );
    assert_eq!(changed.particles(), before);
    assert_eq!(changed.plan().frame(1125), 1853);
}

#[test]
fn a_clip_change_waits_for_old_queued_work_without_relabeling_that_history() {
    let e = emitter(properties());
    let mut small = limits();
    small.max_ticks = 1;
    let mut simulation = Simulation::new(
        plan(std::slice::from_ref(&e), &[], [101, 401], true),
        37,
        small,
    )
    .unwrap();
    let end = Duration::from_millis(500);
    simulation.advance_queued(end, small).unwrap();
    assert_eq!(simulation.elapsed(), Duration::from_millis(125));
    assert!(
        !simulation
            .select_plan(
                Arc::try_unwrap(plan(std::slice::from_ref(&e), &[], [1703, 1853], false)).unwrap(),
                end
            )
            .unwrap()
    );
    assert_eq!(simulation.plan().interval(), [101, 401]);
    for _ in 0..3 {
        simulation.advance_queued(Duration::ZERO, small).unwrap();
    }
    let before = simulation.particles().to_vec();
    assert!(
        simulation
            .select_plan(
                Arc::try_unwrap(plan(&[e], &[], [1703, 1853], false)).unwrap(),
                end
            )
            .unwrap()
    );
    assert_eq!(simulation.particles(), before);
    assert_eq!(simulation.plan().frame(500), 1703);
}

fn moving_parent(global: i32, keys: &[(u32, [f32; 3])]) -> pcx_gfx::mdx_runtime::JointPlan {
    let mut raw = Vec::new();
    for (time, value) in keys {
        raw.extend(time.to_le_bytes());
        for component in value {
            raw.extend(component.to_le_bytes());
        }
    }
    let bone = pcx_mdl::Bone {
        offset: 0,
        span: 104,
        name: "parent".into(),
        object_id: 1,
        parent_id: -1,
        flags: 0x100,
        tracks: vec![pcx_mdl::BoneTrack {
            kind: pcx_mdl::TrackKind::Translation,
            count: keys.len() as u32,
            interpolation: Interpolation::Linear,
            global_sequence_id: global,
            offset: 0,
            stride: 16,
            raw,
        }],
        geoset_id: -1,
        geoset_anim_id: -1,
    };
    pcx_gfx::mdx_runtime::JointPlan::checked(&[bone], &[], &Pivots(vec![[0.; 3], [0.; 3]])).unwrap()
}

fn parented(flags: u32, joints: &pcx_gfx::mdx_runtime::JointPlan, globals: &[u32]) -> Arc<Plan> {
    let mut emitter = emitter(properties());
    emitter.parent_id = 1;
    emitter.flags = flags;
    finish(&mut emitter);
    Arc::new(
        Plan::checked_with_joints(
            &[emitter],
            &texture(),
            globals,
            Clip {
                interval: [0, 1000],
                looping: false,
            },
            "synthetic:parented",
            limits(),
            joints,
        )
        .unwrap(),
    )
}

#[test]
fn parented_emitters_follow_an_animated_joint_without_reintegrating_world_births() {
    let joints = moving_parent(-1, &[(0, [0.; 3]), (1000, [10., 0., 0.])]);
    let model_space = parented(mdx_particles::MODEL_SPACE, &joints, &[]);
    let local = Particle {
        emitter: 0,
        age: 0.1,
        position: [1., 0., 0.],
        velocity: [0.; 3],
        gravity: 0.,
    };
    let at = |elapsed| {
        model_space
            .world_particle(&local, elapsed)
            .unwrap()
            .position
    };
    assert_eq!(at(0), [1., 0., 0.]);
    assert_eq!(at(500), [6., 0., 0.]);
    assert_eq!(local.position, [1., 0., 0.]);
    let half = std::f32::consts::FRAC_1_SQRT_2;
    let mut raw = Vec::new();
    for (time, value) in [(0u32, [0., 0., 0., 1.]), (1000, [0., 0., half, half])] {
        raw.extend(time.to_le_bytes());
        for component in value {
            raw.extend(component.to_le_bytes());
        }
    }
    let bone = pcx_mdl::Bone {
        offset: 0,
        span: 104,
        name: "spin".into(),
        object_id: 1,
        parent_id: -1,
        flags: 0x100,
        tracks: vec![pcx_mdl::BoneTrack {
            kind: pcx_mdl::TrackKind::Rotation,
            count: 2,
            interpolation: Interpolation::Linear,
            global_sequence_id: -1,
            offset: 0,
            stride: 20,
            raw,
        }],
        geoset_id: -1,
        geoset_anim_id: -1,
    };
    let spun =
        pcx_gfx::mdx_runtime::JointPlan::checked(&[bone], &[], &Pivots(vec![[0.; 3], [0.; 3]]))
            .unwrap();
    let turned = parented(mdx_particles::MODEL_SPACE, &spun, &[])
        .world_particle(&local, 1000)
        .unwrap();
    assert!((turned.position[0]).abs() < 1e-5, "{:?}", turned.position);
    assert!(
        (turned.position[1] - 1.).abs() < 1e-5,
        "{:?}",
        turned.position
    );
    assert!((turned.position[2]).abs() < 1e-5, "{:?}", turned.position);

    let mut rate = properties();
    // Four per second stays under one particle on a 125ms tick. Eight emits one.
    rate.emission_rate = 8.;
    let mut world_emitter = emitter(rate);
    world_emitter.parent_id = 1;
    world_emitter.flags = mdx_particles::UNSHADED;
    finish(&mut world_emitter);
    let world = Arc::new(
        Plan::checked_with_joints(
            std::slice::from_ref(&world_emitter),
            &texture(),
            &[],
            Clip {
                interval: [0, 1000],
                looping: false,
            },
            "synthetic:parented",
            limits(),
            &joints,
        )
        .unwrap(),
    );
    let mut simulation = Simulation::new(world, 7, limits()).unwrap();
    simulation
        .advance(Duration::from_millis(125), limits())
        .unwrap();
    let born = simulation.particles()[0].position;
    assert!((born[0] - 1.25).abs() < 1e-4, "{born:?}");
    simulation
        .advance(Duration::from_millis(125), limits())
        .unwrap();
    assert_eq!(simulation.particles()[0].position, born);
    assert!((simulation.particles()[1].position[0] - 2.5).abs() < 1e-4);

    let global = moving_parent(0, &[(0, [0.; 3]), (1000, [10., 0., 0.])]);
    let clocked = parented(mdx_particles::MODEL_SPACE, &global, &[1000]);
    assert_eq!(
        clocked.world_particle(&local, 500).unwrap().position,
        [6., 0., 0.]
    );
}

#[test]
fn hierarchy_admission_refuses_missing_scaled_and_flagged_parents() {
    let joints = moving_parent(-1, &[(0, [0.; 3])]);
    let mut missing_parent = emitter(properties());
    missing_parent.parent_id = 9;
    finish(&mut missing_parent);
    let missing = Plan::checked_with_joints(
        &[missing_parent],
        &texture(),
        &[],
        Clip {
            interval: [0, 1000],
            looping: false,
        },
        "synthetic:missing-parent",
        limits(),
        &joints,
    )
    .unwrap_err();
    assert!(matches!(
        missing.failure,
        Failure::Unsupported("missing emitter parent")
    ));
    let mut scaled = pcx_mdl::Bone {
        offset: 0,
        span: 104,
        name: "scaled".into(),
        object_id: 1,
        parent_id: -1,
        flags: 0x100,
        tracks: vec![pcx_mdl::BoneTrack {
            kind: pcx_mdl::TrackKind::Scaling,
            count: 1,
            interpolation: Interpolation::None,
            global_sequence_id: -1,
            offset: 0,
            stride: 16,
            raw: {
                let mut raw = 0u32.to_le_bytes().to_vec();
                raw.extend(2f32.to_le_bytes());
                raw.extend(2f32.to_le_bytes());
                raw.extend(2f32.to_le_bytes());
                raw
            },
        }],
        geoset_id: -1,
        geoset_anim_id: -1,
    };
    let scaled_joints = pcx_gfx::mdx_runtime::JointPlan::checked(
        &[scaled.clone()],
        &[],
        &Pivots(vec![[0.; 3], [0.; 3]]),
    )
    .unwrap();
    let mut node = emitter(properties());
    node.parent_id = 1;
    finish(&mut node);
    let refused = Plan::checked_with_joints(
        &[node.clone()],
        &texture(),
        &[],
        Clip {
            interval: [0, 1000],
            looping: false,
        },
        "synthetic:scaled-parent",
        limits(),
        &scaled_joints,
    )
    .unwrap_err();
    assert!(matches!(
        refused.failure,
        Failure::Unsupported("scaled emitter parent")
    ));
    scaled.tracks.clear();
    scaled.flags |= 0x8;
    let flagged =
        pcx_gfx::mdx_runtime::JointPlan::checked(&[scaled], &[], &Pivots(vec![[0.; 3], [0.; 3]]))
            .unwrap();
    let refused = Plan::checked_with_joints(
        &[node],
        &texture(),
        &[],
        Clip {
            interval: [0, 1000],
            looping: false,
        },
        "synthetic:flagged-parent",
        limits(),
        &flagged,
    )
    .unwrap_err();
    assert!(matches!(
        refused.failure,
        Failure::Unsupported("emitter parent transform flags")
    ));
}

#[test]
fn shared_parent_pose_is_one_update_and_clip_changes_keep_the_hierarchy() {
    let mut props = properties();
    props.emission_rate = 80.;
    let joints = moving_parent(-1, &[(0, [0.; 3]), (1000, [10., 0., 0.])]);
    let mut node = emitter(props);
    node.parent_id = 1;
    finish(&mut node);
    let build = |interval: [u32; 2], joints: &pcx_gfx::mdx_runtime::JointPlan| {
        Plan::checked_with_joints(
            &[node.clone()],
            &texture(),
            &[],
            Clip {
                interval,
                looping: false,
            },
            "synthetic:shared-parent",
            limits(),
            joints,
        )
        .unwrap()
    };
    let parented = Arc::new(build([0, 1000], &joints));
    let plain = plan(&[emitter(props)], &[], [0, 1000], false);
    let mut parented_sim = Simulation::new(Arc::clone(&parented), 11, limits()).unwrap();
    let mut plain_sim = Simulation::new(plain, 11, limits()).unwrap();
    let parented_work = parented_sim
        .advance(Duration::from_millis(125), limits())
        .unwrap();
    let plain_work = plain_sim
        .advance(Duration::from_millis(125), limits())
        .unwrap();
    assert!(parented_sim.particles().len() > 1);
    assert_eq!(parented_sim.particles().len(), plain_sim.particles().len());
    assert_eq!(parented_work.updates, plain_work.updates + 1);
    let moved = build([0, 400], &joints);
    assert!(
        parented_sim
            .select_plan(moved, parented_sim.elapsed())
            .unwrap()
    );
    let shifted = moving_parent(-1, &[(0, [0.; 3]), (1000, [12., 0., 0.])]);
    let changed = build([0, 1000], &shifted);
    assert!(
        parented_sim
            .select_plan(changed, parented_sim.elapsed())
            .is_err()
    );

    let mut limits = limits();
    limits.max_keys = 1;
    let error = Plan::checked_with_joints(
        &[node],
        &texture(),
        &[],
        Clip {
            interval: [0, 1000],
            looping: false,
        },
        "synthetic:ancestor-keys",
        limits,
        &joints,
    )
    .unwrap_err();
    assert!(matches!(
        error.failure,
        Failure::Budget { kind: "keys", .. }
    ));
}

#[test]
fn identity_scale_and_large_global_elapsed_keep_the_parent_pose() {
    let mut raw = Vec::new();
    raw.extend(0u32.to_le_bytes());
    for component in [1f32, 1., 1.] {
        raw.extend(component.to_le_bytes());
    }
    let mut translation = Vec::new();
    for (time, value) in [(0u32, [0f32, 0., 0.]), (1000, [10., 0., 0.])] {
        translation.extend(time.to_le_bytes());
        for component in value {
            translation.extend(component.to_le_bytes());
        }
    }
    let bone = pcx_mdl::Bone {
        offset: 0,
        span: 104,
        name: "identity-scale".into(),
        object_id: 1,
        parent_id: -1,
        flags: 0x100,
        tracks: vec![
            pcx_mdl::BoneTrack {
                kind: pcx_mdl::TrackKind::Translation,
                count: 2,
                interpolation: Interpolation::Linear,
                global_sequence_id: 0,
                offset: 0,
                stride: 16,
                raw: translation,
            },
            pcx_mdl::BoneTrack {
                kind: pcx_mdl::TrackKind::Scaling,
                count: 1,
                interpolation: Interpolation::None,
                global_sequence_id: -1,
                offset: 0,
                stride: 16,
                raw,
            },
        ],
        geoset_id: -1,
        geoset_anim_id: -1,
    };
    let joints = pcx_gfx::mdx_runtime::JointPlan::checked(
        std::slice::from_ref(&bone),
        &[],
        &Pivots(vec![[0.; 3], [0.; 3]]),
    )
    .unwrap();
    let plan = parented(mdx_particles::MODEL_SPACE, &joints, &[1000]);
    let local = Particle {
        emitter: 0,
        age: 0.1,
        position: [1., 0., 0.],
        velocity: [0.; 3],
        gravity: 0.,
    };
    let elapsed = u64::from(u32::MAX) + 10;
    let frame = (elapsed % 1000) as f32 / 1000. * 10.;
    let position = plan.world_particle(&local, elapsed).unwrap().position;
    assert!(
        (position[0] - (1. + frame)).abs() < 1e-3,
        "{position:?} {frame}"
    );
    assert_eq!(position[1], 0.);
    let mut missing = bone.clone();
    missing.tracks[0].global_sequence_id = 4;
    let missing =
        pcx_gfx::mdx_runtime::JointPlan::checked(&[missing], &[], &Pivots(vec![[0.; 3], [0.; 3]]))
            .unwrap();
    let mut emitter = emitter(properties());
    emitter.parent_id = 1;
    finish(&mut emitter);
    let error = Plan::checked_with_joints(
        &[emitter.clone()],
        &texture(),
        &[],
        Clip {
            interval: [0, 1000],
            looping: false,
        },
        "synthetic:missing-global",
        limits(),
        &missing,
    )
    .unwrap_err();
    assert!(matches!(
        error.failure,
        Failure::Malformed("global reference")
    ));
    let mut zero_raw = Vec::new();
    for (time, value) in [(0u32, [0f32, 0., 0.]), (1, [1., 0., 0.])] {
        zero_raw.extend(time.to_le_bytes());
        for component in value {
            zero_raw.extend(component.to_le_bytes());
        }
    }
    let zero = pcx_mdl::Bone {
        offset: 0,
        span: 104,
        name: "zero-global".into(),
        object_id: 1,
        parent_id: -1,
        flags: 0x100,
        tracks: vec![pcx_mdl::BoneTrack {
            kind: pcx_mdl::TrackKind::Translation,
            count: 2,
            interpolation: Interpolation::Linear,
            global_sequence_id: 0,
            offset: 0,
            stride: 16,
            raw: zero_raw,
        }],
        geoset_id: -1,
        geoset_anim_id: -1,
    };
    let zero =
        pcx_gfx::mdx_runtime::JointPlan::checked(&[zero], &[], &Pivots(vec![[0.; 3], [0.; 3]]))
            .unwrap();
    let error = Plan::checked_with_joints(
        &[emitter],
        &texture(),
        &[0],
        Clip {
            interval: [0, 1000],
            looping: false,
        },
        "synthetic:zero-global",
        limits(),
        &zero,
    )
    .unwrap_err();
    assert!(matches!(
        error.failure,
        Failure::Unsupported("nonconstant zero-duration global track")
    ));
}

#[test]
#[ignore = "requires PCX_SELECTED_PARENTED_PRE2; reads one selected model in place and writes nothing"]
fn selected_parented_emitter_uses_its_joint_hierarchy() {
    let path = std::env::var("PCX_SELECTED_PARENTED_PRE2").expect("explicit selected model path");
    let bytes = std::fs::read(&path).unwrap();
    let mdl = pcx_mdl::Mdlx::parse(&bytes).unwrap();
    let emitters = mdl.particles2(&bytes).unwrap();
    assert!(
        emitters.iter().any(|emitter| emitter.parent_id >= 0),
        "selected model has no parented PRE2 emitter"
    );
    let textures = mdl.textures(&bytes).unwrap();
    let pivots = mdl.pivots(&bytes).unwrap();
    let bones = mdl.bones(&bytes).unwrap();
    let helpers = mdl.nodes(&bytes, pcx_mdl::tag::HELP).unwrap();
    let globals = mdl
        .payload(&bytes, b"GLBS")
        .map(|bytes| {
            bytes
                .as_chunks::<4>()
                .0
                .iter()
                .map(|word| u32::from_le_bytes(*word))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let joints = pcx_gfx::mdx_runtime::JointPlan::checked(&bones, &helpers, &pivots).unwrap();
    let clip = mdl
        .sequences(&bytes)
        .unwrap()
        .into_iter()
        .next()
        .map(|sequence| Clip {
            interval: [sequence.interval_start, sequence.interval_end],
            looping: !sequence.non_looping,
        })
        .unwrap_or(Clip {
            interval: [0, 0],
            looping: false,
        });
    let refused = Plan::checked(
        &emitters,
        &textures,
        &pivots,
        &globals,
        clip,
        &path,
        limits(),
    )
    .unwrap_err();
    assert!(matches!(
        refused.failure,
        Failure::Unsupported("parented emitter")
    ));
    let admitted = Plan::checked_with_joints(
        &emitters,
        &textures,
        &globals,
        clip,
        &path,
        limits(),
        &joints,
    )
    .unwrap();
    let samples = admitted.sample_emitters(0).unwrap();
    assert_eq!(samples.len(), emitters.len());
    assert!(samples.iter().any(|sample| sample.basis.is_some()));
    for sample in &samples {
        if let Some(basis) = sample.basis {
            assert!(basis.iter().all(|value| value.is_finite()), "{path}");
        }
    }
    let index = emitters
        .iter()
        .position(|emitter| emitter.parent_id >= 0)
        .unwrap();
    assert!(samples[index].basis.is_some());
}

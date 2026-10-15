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
fn signed_speed_node_rotation_translation_and_semiimplicit_gravity_are_preserved() {
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
    approx(x.position[2], 17.375);
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
fn asymmetric_atlas_repeat_exclusive_end_and_single_cell_are_distinct() {
    let p = plan(&[emitter(properties())], &[], [0, 1000], true);
    let a = p.appearance(&live(0.3)).unwrap();
    assert_eq!(a.head_cell, Some(3));
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
    let mut e = emitter(properties());
    e.flags |= mdx_particles::MODEL_SPACE;
    cases.push((
        e,
        Failure::Unsupported("model-space particles outside TFT profile"),
    ));
    let mut p = properties();
    p.squirt = 1;
    cases.push((
        emitter(p),
        Failure::Unsupported("squirt emission outside TFT profile"),
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
        *b"KP2E",
        Interpolation::None,
        NO_ID,
        &[(0, &[1.], &[], &[])],
    ));
    finish(&mut e);
    cases.push((
        e,
        Failure::Unsupported("animated scalar outside TFT STEP visibility"),
    ));
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

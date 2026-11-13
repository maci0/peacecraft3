use super::*;

fn rules() -> Rules {
    Rules {
        simple: clock::Rules {
            origin: 0.,
            cycle: 2.,
            minimum: 0.03125,
            epsilon: 0.0625,
        },
        numerator: 1,
        denominator: 8,
        step: 0.125f32.to_bits(),
        wrap_tolerance: 0.00001f32.to_bits(),
        frame: frame::Rules {
            unsigned_correction: [0f64.to_bits(), 4294967296f64.to_bits()],
            scale: 0.125f32.to_bits(),
            chunk: 1f32.to_bits(),
        },
        sleep: sleep::Rules {
            script: Domain::Real,
            ai: Domain::Game,
        },
        turn: turn::Rules {
            quantum_ms: 25,
            credit_cap: 4000,
            speed_steps: Arc::from([1, 2, 3]),
            scale: 1f32.to_bits(),
            modifier: 1,
            elapsed_numerator: 1,
            elapsed_denominator: 1,
            initial_carry: 0,
        },
    }
}

#[test]
fn paired_frames_keep_independent_cursors_and_causal_input_order() {
    let mut state = State::new(rules()).unwrap();
    let mut work = 32;
    state
        .enqueue_frame(
            FrameInput {
                game_elapsed: Duration::from_millis(250),
                real_elapsed_ms: 3,
            },
            &mut work,
        )
        .unwrap();
    let credit = state.credit;
    assert!(state.pace(Duration::ZERO).is_err());
    assert_eq!(state.credit, credit);
    state
        .enqueue_frame(
            FrameInput {
                game_elapsed: Duration::from_millis(125),
                real_elapsed_ms: 1,
            },
            &mut work,
        )
        .unwrap();
    let mut trace = Vec::new();
    while let Some((domain, segment)) = state.next_frame_segment(&mut work).unwrap() {
        trace.push((domain, segment.point.local));
        if domain == Domain::Game && trace.len() <= 2 {
            assert_eq!(
                state.queue_point(Domain::Real),
                Point { cycle: 0, local: 0 }
            );
        }
        if domain == Domain::Real && trace.len() == 3 {
            state
                .enter_domain_callback(
                    domain,
                    Point {
                        cycle: 0,
                        local: 0.125f32.to_bits(),
                    },
                )
                .unwrap();
            assert_eq!(state.queue_point(Domain::Game).local, 0.25f32.to_bits());
            assert_eq!(state.queue_point(domain).local, 0.125f32.to_bits());
            assert!(state.enter_callback(state.point()).is_err());
            state.leave_callback().unwrap();
            assert_eq!(state.queue_point(domain), segment.point);
        }
        state.finish_frame_segment(domain, segment).unwrap();
    }
    assert_eq!(
        trace,
        vec![
            (Domain::Game, 0.125f32.to_bits()),
            (Domain::Game, 0.25f32.to_bits()),
            (Domain::Real, 0.375f32.to_bits()),
            (Domain::Game, 0.375f32.to_bits()),
            (Domain::Real, 0.5f32.to_bits())
        ]
    );
    assert!(!state.frame_active());
}

#[test]
fn real_chunks_wrap_without_game_progress_and_resume_the_same_pending_drain() {
    let mut state = State::new(rules()).unwrap();
    state
        .enqueue_frame(
            FrameInput {
                game_elapsed: Duration::ZERO,
                real_elapsed_ms: 33,
            },
            &mut 8,
        )
        .unwrap();
    assert!(state.next_frame_segment(&mut 0).is_err());
    let mut work = 16;
    let mut wraps = 0;
    while let Some((domain, segment)) = state.next_frame_segment(&mut work).unwrap() {
        assert_eq!(domain, Domain::Real);
        assert_eq!(
            state.queue_point(Domain::Game),
            Point { cycle: 0, local: 0 }
        );
        assert_eq!(
            state.next_frame_segment(&mut 0).unwrap(),
            Some((domain, segment))
        );
        wraps += usize::from(segment.ends_cycle);
        state.finish_frame_segment(domain, segment).unwrap();
    }
    assert_eq!(wraps, 2);
    assert_eq!(
        state.queue_point(Domain::Real),
        Point {
            cycle: 2,
            local: 0.125f32.to_bits()
        }
    );
}

#[test]
fn real_counter_conversion_uses_its_loaded_sse_operands() {
    let mut rule = rules().frame;
    assert_eq!(rule.delta(0).unwrap(), None);
    assert_eq!(rule.delta(3).unwrap(), Some(0.375f32.to_bits()));
    assert_eq!(
        rule.delta(0x80000000).unwrap(),
        Some(268435456f32.to_bits())
    );
    rule.scale = 0.25f32.to_bits();
    assert_eq!(rule.delta(3).unwrap(), Some(0.75f32.to_bits()));
    rule.unsigned_correction[1] = 8589934592f64.to_bits();
    assert_eq!(
        rule.delta(0x80000000).unwrap(),
        Some(1610612736f32.to_bits())
    );
}

#[test]
fn counter_frames_keep_submillisecond_carry_and_get_tick_count_wrap() {
    let mut policy = rules();
    policy.frame.scale = 0.001f32.to_bits();
    let mut state = State::new(policy.clone()).unwrap();
    for _ in 0..10 {
        state
            .enqueue_counter_frame(Duration::from_micros(100), &mut 32)
            .unwrap();
        while let Some((domain, segment)) = state
            .next_frame_segment_with_settings(&mut 32, Some(1), false)
            .unwrap()
        {
            assert_eq!(domain, Domain::Real);
            state.finish_frame_segment(domain, segment).unwrap();
        }
    }
    assert_eq!(state.sub_ms_nanos, 0);
    assert_eq!(state.turn_credit_ms, 1);
    assert_eq!(state.completed_point().local, 0);
    assert_eq!(
        state.queue_point(Domain::Real).local,
        policy.frame.delta(1).unwrap().unwrap()
    );
    let mut wrapped = State::new(policy).unwrap();
    wrapped
        .enqueue_counter_frame(Duration::from_millis(u64::from(u32::MAX) + 2), &mut 32)
        .unwrap();
    while let Some((domain, segment)) = wrapped
        .next_frame_segment_with_settings(&mut 32, Some(1), false)
        .unwrap()
    {
        wrapped.finish_frame_segment(domain, segment).unwrap();
    }
    assert_eq!(wrapped.turn_credit_ms, state.turn_credit_ms);
    assert_eq!(
        wrapped.queue_point(Domain::Real),
        state.queue_point(Domain::Real)
    );
}

#[test]
fn counter_pause_and_speed_changes_apply_after_the_inflight_grant() {
    let mut policy = rules();
    policy.frame.scale = 0.001f32.to_bits();
    let mut state = State::new(policy).unwrap();
    state
        .enqueue_counter_frame(Duration::from_millis(50), &mut 64)
        .unwrap();
    let (domain, first) = state
        .next_frame_segment_with_settings(&mut 64, Some(2), false)
        .unwrap()
        .unwrap();
    assert_eq!(first.point.local, 0.125f32.to_bits());
    state.finish_frame_segment(domain, first).unwrap();
    let mut games = 1;
    while let Some((domain, segment)) = state
        .next_frame_segment_with_settings(&mut 64, Some(0), true)
        .unwrap()
    {
        games += usize::from(domain == Domain::Game);
        state.finish_frame_segment(domain, segment).unwrap();
    }
    assert_eq!(games, 3);
    assert_eq!(state.completed_point().local, 0.375f32.to_bits());
    state
        .enqueue_counter_frame(Duration::from_millis(25), &mut 64)
        .unwrap();
    while let Some((domain, segment)) = state
        .next_frame_segment_with_settings(&mut 64, Some(0), false)
        .unwrap()
    {
        state.finish_frame_segment(domain, segment).unwrap();
    }
    assert_eq!(state.completed_point().local, 0.5f32.to_bits());
}

#[test]
fn counter_grant_uses_sse_fractional_carry_and_retains_work_refusal() {
    let mut policy = rules();
    policy.frame.scale = 0.001f32.to_bits();
    policy.turn.scale = 0.5f32.to_bits();
    let mut state = State::new(policy).unwrap();
    state
        .enqueue_counter_frame(Duration::from_millis(25), &mut 64)
        .unwrap();
    assert!(
        state
            .next_frame_segment_with_settings(&mut 0, Some(0), false)
            .is_err()
    );
    while let Some((domain, segment)) = state
        .next_frame_segment_with_settings(&mut 64, Some(0), false)
        .unwrap()
    {
        state.finish_frame_segment(domain, segment).unwrap();
    }
    assert_eq!(state.turn_carry, 0.5f32.to_bits());
    assert_eq!(state.completed_point().local, 0);
    state
        .enqueue_counter_frame(Duration::from_millis(25), &mut 64)
        .unwrap();
    while let Some((domain, segment)) = state
        .next_frame_segment_with_settings(&mut 64, Some(0), false)
        .unwrap()
    {
        state.finish_frame_segment(domain, segment).unwrap();
    }
    assert_eq!(state.turn_carry, 0);
    assert_eq!(state.completed_point().local, 0.125f32.to_bits());
}

#[test]
fn counter_committed_duration_excludes_retained_partial_frames() {
    let mut policy = rules();
    policy.frame.scale = 0.001f32.to_bits();
    let mut state = State::new(policy).unwrap();
    let elapsed = Duration::from_millis(25);
    state.enqueue_counter_frame(elapsed, &mut 64).unwrap();
    assert!(state.counter_platform());
    assert_eq!(state.counter_progress_ns(), 0);
    assert_eq!(state.counter_pending_ns(), elapsed.as_nanos());
    let (domain, segment) = state
        .next_frame_segment_with_settings(&mut 64, Some(1), false)
        .unwrap()
        .unwrap();
    state.finish_frame_segment(domain, segment).unwrap();
    assert_eq!(state.counter_progress_ns(), 0);
    assert_eq!(state.counter_pending_ns(), elapsed.as_nanos());
    while let Some((domain, segment)) = state
        .next_frame_segment_with_settings(&mut 64, Some(1), false)
        .unwrap()
    {
        state.finish_frame_segment(domain, segment).unwrap();
    }
    assert_eq!(state.counter_progress_ns(), elapsed.as_nanos());
    assert_eq!(state.counter_pending_ns(), 0);
}

fn drain(state: &mut State, work: &mut usize) -> Vec<Segment> {
    let mut trace = Vec::new();
    while let Some(segment) = state.next_segment(work).unwrap() {
        trace.push(segment);
        state.finish_segment(segment).unwrap();
    }
    trace
}

#[test]
fn native_pacing_carries_fractional_steps_and_batches_without_endpoint_rounding() {
    let mut split = State::new(rules()).unwrap();
    let mut together = State::new(rules()).unwrap();
    let mut work = 100;
    let mut trace = Vec::new();
    for _ in 0..100 {
        split.pace(Duration::from_millis(37)).unwrap();
        trace.extend(drain(&mut split, &mut work));
    }
    together.pace(Duration::from_millis(3700)).unwrap();
    assert_eq!(trace, drain(&mut together, &mut 100));
    assert_eq!(
        split.completed_point(),
        Point {
            cycle: 1,
            local: 1.625f32.to_bits()
        }
    );
    assert_eq!(split.credit, 600_000_000);
    assert_eq!(split.credit, together.credit);
    assert_eq!(work, 71);
    split.pace(Duration::from_millis(50)).unwrap();
    assert_eq!(drain(&mut split, &mut work).len(), 1);
    assert_eq!(split.completed_point().local, 1.75f32.to_bits());
    assert_eq!(split.credit, 0);
}

#[test]
fn native_pending_wrap_restores_outer_cursor_and_retains_unconsumed_pacing() {
    let mut state = State::new(rules()).unwrap();
    state.pace(Duration::from_secs(2)).unwrap();
    let mut work = 16;
    for _ in 0..15 {
        let segment = state.next_segment(&mut work).unwrap().unwrap();
        state.finish_segment(segment).unwrap();
    }
    let before = state.next_segment(&mut work).unwrap().unwrap();
    assert!(before.ends_cycle);
    assert_eq!(
        before.point,
        Point {
            cycle: 0,
            local: 2f32.to_bits()
        }
    );
    assert_eq!(state.completed_point().local, 1.875f32.to_bits());
    assert_eq!(state.next_segment(&mut 0).unwrap(), Some(before));
    let callback = Point {
        cycle: 0,
        local: 1.9375f32.to_bits(),
    };
    state.enter_callback(callback).unwrap();
    let raw = state.rules().simple_deadline(state.point(), 0.5).unwrap();
    assert_eq!(
        raw,
        Point {
            cycle: 0,
            local: 2.4375f32.to_bits()
        }
    );
    assert!(state.finish_segment(before).is_err());
    state.leave_callback().unwrap();
    assert_eq!(state.point(), before.point);
    let rebased = state.rules().rebase(raw).unwrap();
    assert_eq!(
        rebased,
        Point {
            cycle: 1,
            local: 0.4375f32.to_bits()
        }
    );
    state.finish_segment(before).unwrap();
    assert_eq!(state.credit, 1_000_000_000);
    let after = state.next_segment(&mut 0).unwrap().unwrap();
    assert_eq!(
        after,
        Segment {
            point: Point { cycle: 1, local: 0 },
            ends_cycle: false
        }
    );
    state.finish_segment(after).unwrap();
    assert_eq!(state.credit, 0);
    assert_eq!(state.completed_point(), after.point);
    assert_eq!(state.next_segment(&mut 0).unwrap(), None);
}

#[test]
fn native_clock_preserves_work_and_state_on_refusal_and_clamps_from_source() {
    let mut state = State::new(rules()).unwrap();
    state.pace(Duration::from_millis(125)).unwrap();
    assert!(state.next_segment(&mut 0).is_err());
    assert!(state.pending.is_none());
    assert_eq!(state.completed_point().local, 0);
    let segment = state.next_segment(&mut 1).unwrap().unwrap();
    for delay in [0., -3.] {
        assert_eq!(
            state
                .rules()
                .simple_deadline(segment.point, delay)
                .unwrap()
                .local,
            0.15625f32.to_bits()
        );
    }
    assert_eq!(
        state
            .rules()
            .simple_deadline(segment.point, 99.)
            .unwrap()
            .local,
        2.0625f32.to_bits()
    );
    assert!(state.enter_callback(Point { cycle: 1, local: 0 }).is_err());
    assert!(
        state
            .enter_callback(Point {
                cycle: 0,
                local: 0.25f32.to_bits()
            })
            .is_err()
    );
    assert!(
        state
            .rules()
            .simple_deadline(segment.point, f32::NAN)
            .is_err()
    );
    assert!(
        state
            .rules()
            .simple_deadline(segment.point, f32::from_bits(1))
            .is_err()
    );
    let mut invalid = rules();
    invalid.step = 3f32.to_bits();
    assert!(State::new(invalid).is_err());
}

fn initializer(numerator: i8, denominator: i32, destination: u32) -> Vec<u8> {
    let mut b = vec![0x90; 56];
    for (at, bytes) in [
        (
            0,
            &[0x55, 0x8b, 0xec, 0x83, 0xec, 8, 0x8d, 0x45, 0xfc, 0x68][..],
        ),
        (14, &[0x50]),
        (20, &[0x83, 0xc4, 8, 0x50, 0x8d, 0x45, 0xf8, 0x6a]),
        (29, &[0x50]),
        (35, &[0x83, 0xc4, 8, 0x50, 0x68]),
        (49, &[0x83, 0xc4, 12, 0x8b, 0xe5, 0x5d, 0xc3]),
    ] {
        b[at..at + bytes.len()].copy_from_slice(bytes);
    }
    b[10..14].copy_from_slice(&denominator.to_le_bytes());
    b[28] = numerator as u8;
    b[40..44].copy_from_slice(&destination.to_le_bytes());
    for (at, target) in [(15, 0x9a1620u32), (30, 0x9a1620), (44, 0x9a02d0)] {
        b[at] = 0xe8;
        b[at + 1..at + 5]
            .copy_from_slice(&target.wrapping_sub(0x406730 + at as u32 + 5).to_le_bytes());
    }
    b
}

#[test]
fn native_step_ratio_is_read_from_nonstock_initializer_operands() {
    for (num, den, global) in [(17, 2003, 0x12345678), (3, 128, 0x700004)] {
        let b = initializer(num, den, global);
        assert_eq!(decode_step(&b).unwrap(), (num as u32, den as u32, global));
        for at in [14, 15, 20, 29, 30, 39, 44, 55] {
            let mut changed = b.clone();
            changed[at] ^= 1;
            assert!(decode_step(&changed).is_err(), "offset {at}");
        }
    }
    assert!(decode_step(&initializer(0, 20, 7)).is_err());
    assert!(decode_step(&initializer(3, -20, 7)).is_err());
    assert!(decode_step(&initializer(-1, 20, 7)).is_err());
}

#[test]
fn native_wrap_near_zero_rule_uses_source_tolerance_and_retains_two_drains() {
    let mut policy = rules();
    policy.step = 0.3f32.to_bits();
    policy.wrap_tolerance = 0.125f32.to_bits();
    policy.numerator = 3;
    policy.denominator = 10;
    let mut state = State::new(policy.clone()).unwrap();
    state.pace(Duration::from_millis(2100)).unwrap();
    let trace = drain(&mut state, &mut 7);
    assert_eq!(trace.len(), 8);
    assert_eq!(
        trace[6],
        Segment {
            point: Point {
                cycle: 0,
                local: 2f32.to_bits()
            },
            ends_cycle: true
        }
    );
    assert_eq!(
        trace[7],
        Segment {
            point: Point { cycle: 1, local: 0 },
            ends_cycle: false
        }
    );
    assert_eq!(state.completed_point(), trace[7].point);
}

#[test]
#[ignore = "requires explicit PCX_WAR3_EXE selected original executable"]
fn selected_native_game_queue_derives_observed_step_and_first_wrap_model() {
    use crate::{camera_margins::SourceWindow, scripts::AssetIdentity};
    use std::io::Read;
    let path = std::env::var("PCX_WAR3_EXE").unwrap();
    let file = std::fs::File::open(&path).unwrap();
    assert!(file.metadata().unwrap().len() <= 32 << 20);
    let mut image = Vec::new();
    file.take((32 << 20) + 1).read_to_end(&mut image).unwrap();
    let budget = RuleInputBudget {
        image_bytes: 32 << 20,
        profile_bytes: 64 << 10,
        sections: 96,
    };
    let pe = Pe::parse(&image, budget).unwrap();
    let w = |va, len, code| {
        let bytes = &pe.read_from(va, code).unwrap()[..len as usize];
        SourceWindow {
            va,
            len,
            digest: sha256(bytes),
        }
    };
    let numeric = NumericPolicy::read(
        &image,
        crate::script_numeric::Binding {
            origin: AssetIdentity {
                source: path,
                edition: "Legacy".into(),
                version: "1.29.2.9231".into(),
            },
            image_sha256: sha256(&image),
            literal: w(0x8df660, 238, true),
            division: w(0x9a02d0, 57, true),
            reciprocal: w(0x9a1d00, 103, true),
            reciprocal_table: w(0xed6cf0, 4100, false),
            equality: w(0x8d8d10, 53, true),
            equality_constant: w(0x10db054, 4, false),
            inequality: w(0x8d7e32, 162, true),
            math_sub: w(0x9a00b0, 264, true),
            math_add: w(0x9a01c0, 262, true),
            math_mul: w(0x99ffe0, 195, true),
            math_lzcnt: w(0x9a0330, 29, true),
            math_i32_to_real: w(0x9a1620, 92, true),
        },
        budget,
        4096,
    )
    .unwrap();
    let policy = Rules::read(&image, &numeric, budget).unwrap();
    assert_eq!(policy.step, 0x3ba3d70a, "independent VM_READ observation");
    assert_eq!(policy.simple.cycle.to_bits(), 0x43960000);
    let mut state = State::new(policy.clone()).unwrap();
    let steps = 60111u64; // Diagnostic model boundary; the native trace brackets the wrap.
    state
        .pace(Duration::from_nanos(
            steps * u64::from(policy.numerator) * 1_000_000_000 / u64::from(policy.denominator),
        ))
        .unwrap();
    let trace = drain(&mut state, &mut (steps as usize));
    assert_eq!(trace.iter().filter(|s| s.ends_cycle).count(), 1);
    assert_eq!(
        state.completed_point(),
        Point {
            cycle: 1,
            local: 0x3ba10000
        }
    );
}

#[test]
fn native_driver_domain_choice_is_one_way_and_noops_leave_fresh_state() {
    let mut state = State::new(rules()).unwrap();
    state.select_legacy(false).unwrap();
    assert_eq!(state.driver(), Driver::Fresh);
    let mut legacy = state.clone();
    legacy.select_legacy(true).unwrap();
    assert!(legacy.pace(Duration::from_secs(1)).is_err());
    assert_eq!(legacy.driver(), Driver::Legacy);
    assert_eq!(legacy.credit, 0);
    assert_eq!(legacy.point(), state.point());
    state.pace(Duration::ZERO).unwrap();
    assert_eq!(state.driver(), Driver::Native);
    assert!(state.select_legacy(false).is_err());
    assert!(state.select_legacy(true).is_err());
    assert_eq!(state.credit, 0);
    assert_eq!(state.completed_point().local, 0);
}

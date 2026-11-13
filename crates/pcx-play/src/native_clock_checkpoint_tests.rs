use super::*;
use pcx_script::checkpoint::Budget;

fn budget() -> Budget {
    Budget {
        bytes: 1 << 20,
        items: 4096,
        depth: 32,
    }
}
fn encode(state: &State) -> Vec<u8> {
    let mut out = Writer::new(budget()).unwrap();
    state.write_checkpoint(&mut out).unwrap();
    out.finish()
}
fn decode(rules: Rules, bytes: &[u8]) -> Result<State, String> {
    let mut input = Reader::new(bytes, budget())?;
    let state = State::read_checkpoint(rules, &mut input)?;
    input.finish()?;
    Ok(state)
}
fn roundtrip(state: &State) -> State {
    let next = decode(state.rules.clone(), &encode(state)).unwrap();
    assert_eq!(format!("{state:?}"), format!("{next:?}"));
    next
}
fn rules() -> Rules {
    crate::native_game_clock::tests::rules()
}

#[test]
fn clock_checkpoint_continues_both_queues_wraps_scopes_and_counter_fifo() {
    let mut live = State::new(rules()).unwrap();
    let mut work = 128;
    live.enqueue_frame(
        FrameInput {
            game_elapsed: Duration::from_millis(3125),
            real_elapsed_ms: 41,
        },
        &mut work,
    )
    .unwrap();
    for duration in [
        Duration::from_nanos(50_250_001),
        Duration::from_nanos(75_999_999),
    ] {
        live.enqueue_counter_frame(duration, &mut work).unwrap();
    }
    live.enqueue_frame(
        FrameInput {
            game_elapsed: Duration::from_millis(375),
            real_elapsed_ms: 2,
        },
        &mut work,
    )
    .unwrap();
    let mut seen = [false; 4];
    let mut domains = [false; 2];
    let mut wrapping = false;
    let mut complete = false;
    for _ in 0..2048 {
        if let Some(frame) = live.frames.front() {
            seen[match frame.phase {
                FramePhase::Queued => 0,
                FramePhase::CounterQuanta => 1,
                FramePhase::Game => 2,
                FramePhase::Real => 3,
            }] = true;
        }
        let mut restored = roundtrip(&live);
        let mut a = 1;
        let mut b = 1;
        let left = live.next_frame_segment_with_settings(&mut a, Some(1), false);
        let right = restored.next_frame_segment_with_settings(&mut b, Some(1), false);
        assert_eq!(format!("{left:?}"), format!("{right:?}"));
        assert_eq!(a, b);
        assert_eq!(format!("{live:?}"), format!("{restored:?}"));
        match left {
            Ok(Some((domain, segment))) => {
                domains[domain_byte(domain) as usize] = true;
                wrapping |= segment.ends_cycle;
                live.enter_domain_callback(domain, segment.point).unwrap();
                let mut selected = roundtrip(&live);
                assert_eq!(selected.queue_point(domain), segment.point);
                live.leave_callback().unwrap();
                selected.leave_callback().unwrap();
                live.finish_frame_segment(domain, segment).unwrap();
                selected.finish_frame_segment(domain, segment).unwrap();
                assert_eq!(format!("{live:?}"), format!("{selected:?}"));
            }
            Ok(None) => {
                complete = true;
                break;
            }
            Err(QueueError::Budget(_)) => {}
            Err(error) => panic!("{error}"),
        }
    }
    assert!(complete && wrapping && domains.into_iter().all(|x| x));
    assert!(
        seen.into_iter().all(|x| x),
        "all retained phases must be exercised"
    );
    assert_eq!(live.completed_counter_ns, 126_250_000);
    assert_eq!(live.sub_ms_nanos, 250_000);
    assert!(!roundtrip(&live).frame_active());
}

#[test]
fn clock_checkpoint_preserves_game_only_second_wrap_and_legacy_admission() {
    let mut state = State::new(rules()).unwrap();
    roundtrip(&state);
    state.select_legacy(true).unwrap();
    assert_eq!(roundtrip(&state).driver(), Driver::Legacy);
    let mut state = State::new(rules()).unwrap();
    state.pace(Duration::from_secs(3)).unwrap();
    let mut wraps = 0;
    while let Some(segment) = state.next_segment(&mut 1).unwrap() {
        let mut restored = roundtrip(&state);
        state.finish_segment(segment).unwrap();
        restored.finish_segment(segment).unwrap();
        assert_eq!(format!("{state:?}"), format!("{restored:?}"));
        wraps += usize::from(segment.ends_cycle);
        roundtrip(&state);
    }
    assert_eq!(wraps, 1);
}

#[test]
fn clock_checkpoint_refuses_corruption_foreign_rules_and_unbounded_counts() {
    let mut state = State::new(rules()).unwrap();
    state
        .enqueue_counter_frame(Duration::from_nanos(51_000_123), &mut 10)
        .unwrap();
    let bytes = encode(&state);
    for end in 0..bytes.len() {
        assert!(decode(rules(), &bytes[..end]).is_err(), "prefix {end}");
    }
    let mut foreign = rules();
    Arc::make_mut(&mut foreign.turn.speed_steps)[0] += 1;
    assert!(decode(foreign, &bytes).unwrap_err().contains("policy"));
    let mut corrupt = bytes.clone();
    corrupt[32] = 9;
    assert!(decode(rules(), &corrupt).is_err());
    let mut corrupt = bytes.clone();
    // The final counter carry is admitted independently of allocation or queue shape.
    let at = corrupt.len() - 21;
    corrupt[at..at + 4].copy_from_slice(&1_000_000u32.to_le_bytes());
    assert!(decode(rules(), &corrupt).is_err());
    let mut input = Reader::new(
        &bytes,
        Budget {
            items: 0,
            ..budget()
        },
    )
    .unwrap();
    assert!(State::read_checkpoint(rules(), &mut input).is_err());
    let mut bad = state.clone();
    Arc::make_mut(&mut bad.frames).front_mut().unwrap().phase = FramePhase::CounterQuanta;
    Arc::make_mut(&mut bad.frames).front_mut().unwrap().input =
        FrameSource::Independent(FrameInput {
            game_elapsed: Duration::ZERO,
            real_elapsed_ms: 1,
        });
    assert!(bad.validate_checkpoint().is_err());
    let mut bad = state.clone();
    bad.selected_domain = Some(Domain::Real);
    assert!(bad.validate_checkpoint().is_err());
    let mut bad = state.clone();
    bad.completed_counter_ns = u128::MAX;
    assert!(bad.validate_checkpoint().is_err());
    let mut bad = state;
    if let FrameSource::Counter { raw_ms, .. } =
        &mut Arc::make_mut(&mut bad.frames).front_mut().unwrap().input
    {
        *raw_ms += 1;
    }
    assert!(bad.validate_checkpoint().is_err());
}

#[test]
fn clock_checkpoint_keeps_counter_low_dword_and_source_initial_carry() {
    let mut source = rules();
    source.turn.initial_carry = 1.5f32.to_bits();
    let mut state = State::new(source).unwrap();
    state
        .enqueue_counter_frame(Duration::new(4_294_968, 123_456_789), &mut 10)
        .unwrap();
    let restored = roundtrip(&state);
    assert_eq!(restored.sub_ms_nanos, 456_789);
    assert_eq!(restored.turn_carry, 1.5f32.to_bits());
    assert!(matches!(
        restored.frames[0].input,
        FrameSource::Counter { raw_ms: 827, .. }
    ));
}

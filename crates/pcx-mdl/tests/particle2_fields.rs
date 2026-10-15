use pcx_mdl::{Interpolation, KeyTrack, NO_ID, PARTICLE2_FIXED, Particle2Properties};

fn track(frames: &[i32]) -> KeyTrack {
    let raw = frames
        .iter()
        .flat_map(|frame| frame.to_le_bytes().into_iter().chain(2.5f32.to_le_bytes()))
        .collect();
    KeyTrack {
        magic: *b"KP2E",
        count: u32::try_from(frames.len()).unwrap(),
        interpolation: Interpolation::None,
        global_seq_id: NO_ID,
        width: 1,
        stride: 8,
        raw,
    }
}
#[test]
fn signed_preroll_and_positive_times_preserve_unsigned_api_and_bytes() {
    let track = track(&[-17, 0, 91]);
    let before = track.raw.clone();
    let signed: Vec<_> = (0..3).map(|i| track.frame_signed(i).unwrap()).collect();
    assert_eq!(signed, [-17, 0, 91]);
    assert!(signed.windows(2).all(|w| w[0] < w[1]));
    assert_eq!(
        track.frame(0),
        Some(u32::from_le_bytes((-17i32).to_le_bytes()))
    );
    assert_eq!(track.frame(2), Some(91));
    assert_eq!(track.raw, before);
}
#[test]
fn signed_extremes_have_no_saturation_or_clock_interpretation() {
    let track = track(&[i32::MIN, i32::MAX]);
    assert_eq!(track.frame_signed(0), Some(i32::MIN));
    assert_eq!(track.frame_signed(1), Some(i32::MAX));
}
#[test]
fn declared_count_and_out_of_range_indices_are_checked() {
    let mut t = track(&[3]);
    assert_eq!(t.frame_signed(1), None);
    assert_eq!(t.frame_signed(usize::MAX), None);
    t.count = 0;
    assert_eq!(t.frame_signed(0), None);
    let t = track(&[]);
    assert_eq!(t.frame_signed(0), None);
}
#[test]
fn truncated_or_surplus_record_spans_are_refused() {
    let original = track(&[-17, 91]);
    for length in 0..original.raw.len() {
        let mut t = original.clone();
        t.raw.truncate(length);
        assert_eq!(t.frame_signed(0), None, "raw length {length}");
        assert_eq!(t.frame_signed(1), None, "raw length {length}");
    }
    let mut t = original;
    t.raw.push(0);
    assert_eq!(t.frame_signed(0), None);
}
#[test]
fn invalid_stride_and_overflowing_count_span_are_refused() {
    let mut t = track(&[1]);
    t.stride = 3;
    t.raw.truncate(3);
    assert_eq!(t.frame_signed(0), None);
    t.stride = usize::MAX;
    t.count = u32::MAX;
    assert_eq!(t.frame_signed(0), None);
    assert_eq!(t.frame_signed(usize::MAX), None);
}
#[test]
fn accessor_preserves_duplicate_and_reversed_frames_for_caller_validation() {
    for frames in [&[2, 2][..], &[3, -17][..]] {
        let t = track(frames);
        let read: Vec<_> = (0..frames.len())
            .map(|i| t.frame_signed(i).unwrap())
            .collect();
        assert_eq!(read, frames);
        assert!(!read.windows(2).all(|w| w[0] < w[1]));
    }
}
#[test]
fn unequal_extent_names_and_each_serializer_destination_are_discriminating() {
    let mut raw = [0; PARTICLE2_FIXED];
    raw[24..28].copy_from_slice(&7.25f32.to_le_bytes());
    raw[28..32].copy_from_slice(&19.5f32.to_le_bytes());
    let p = Particle2Properties::from_bytes(&raw);
    assert_eq!(p.width, 7.25);
    assert_eq!(p.length, 19.5);
    assert_eq!(p.to_bytes(), raw);
    let mut width = p;
    width.width = 11.75;
    let b = width.to_bytes();
    assert_eq!(&b[24..28], &11.75f32.to_le_bytes());
    assert_eq!(&b[28..32], &19.5f32.to_le_bytes());
    let mut length = p;
    length.length = 23.125;
    let b = length.to_bytes();
    assert_eq!(&b[24..28], &7.25f32.to_le_bytes());
    assert_eq!(&b[28..32], &23.125f32.to_le_bytes());
}
#[test]
fn every_fixed_state_bit_is_retained_including_unaligned_segments() {
    let raw = std::array::from_fn(|i| (i.wrapping_mul(53).wrapping_add(29)) as u8);
    let p = Particle2Properties::from_bytes(&raw);
    assert_eq!(p.to_bytes(), raw);
    assert_eq!(p.segment_alpha, raw[92..95]);
}

#[test]
#[ignore = "requires explicit source-qualified retained model path list; no archive scan"]
fn retained_model_states_and_preroll_remain_byte_exact() {
    let list = std::env::var_os("PEACECRAFT3_PARTICLE_MODEL_PATHS")
        .expect("explicit model path list required");
    let list = std::fs::read_to_string(list).unwrap();
    let mut emitters = 0;
    let mut unequal = 0;
    let mut negative = 0;
    for path in list.lines().filter(|s| !s.is_empty()) {
        let raw = std::fs::read(path).unwrap();
        let model = pcx_mdl::Mdlx::parse(&raw).unwrap();
        let mut count = 0;
        for emitter in model.particles2(&raw).unwrap() {
            let p = Particle2Properties::from_bytes(&emitter.state);
            assert_eq!(p.to_bytes(), emitter.state);
            let word =
                |at| f32::from_le_bytes(emitter.state[at..at + 4].try_into().unwrap()).to_bits();
            assert_eq!(p.width.to_bits(), word(24));
            assert_eq!(p.length.to_bits(), word(28));
            unequal += usize::from(p.width.to_bits() != p.length.to_bits());
            for original in emitter.node_tracks.iter().chain(&emitter.tracks) {
                let t = KeyTrack {
                    magic: original.magic,
                    count: original.count,
                    interpolation: original.interpolation,
                    global_seq_id: original.global_seq_id,
                    width: original.width,
                    stride: original.stride,
                    raw: original.raw.clone(),
                };
                let signed: Vec<_> = (0..t.count as usize)
                    .map(|i| t.frame_signed(i).unwrap())
                    .collect();
                assert!(signed.windows(2).all(|w| w[0] < w[1]));
                negative += signed.iter().filter(|frame| **frame < 0).count();
                assert_eq!(t.raw, original.raw);
            }
            count += 1;
        }
        emitters += count;
        eprintln!("source={path} emitters={count}");
    }
    assert!(
        emitters > 0 && unequal > 0 && negative > 0,
        "explicit fixtures must exercise all three cases"
    );
    eprintln!("emitters={emitters} unequal_extents={unequal} signed_preroll_keys={negative}");
}

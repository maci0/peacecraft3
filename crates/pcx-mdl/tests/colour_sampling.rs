//! Synthetic raw KGAC sampling; material channel conversion is checked separately.
use pcx_mdl::{AnimTrack, Interpolation, NO_ID, sample_colour, sample_scalar};

fn track(mode: Interpolation) -> AnimTrack {
    let mut raw = Vec::new();
    for (frame, value, incoming, outgoing) in [
        (0u32, [0f32, 0.25, 0.5], [-8f32, -9., -10.], [0f32, 2., 4.]),
        (100, [1., 0.75, 0.25], [2., 1., -2.], [7., 8., 9.]),
    ] {
        raw.extend(frame.to_le_bytes());
        for v in value {
            raw.extend(v.to_le_bytes());
        }
        if mode.has_tangents() {
            for v in incoming {
                raw.extend(v.to_le_bytes());
            }
            for v in outgoing {
                raw.extend(v.to_le_bytes());
            }
        }
    }
    AnimTrack {
        magic: *b"KGAC",
        count: 2,
        interpolation: mode,
        global_seq_id: NO_ID,
        raw,
    }
}

#[test]
fn every_interpolation_preserves_component_values_and_tangent_direction() {
    for (mode, expected) in [
        (Interpolation::None, [0., 0.25, 0.5]),
        (Interpolation::Linear, [0.5, 0.5, 0.375]),
        (Interpolation::Hermite, [0.25, 0.625, 1.125]),
        (Interpolation::Bezier, [0.875, 1.25, 0.84375]),
    ] {
        let t = track(mode);
        assert_eq!(sample_colour(&t, 50), expected, "{mode:?}");
        assert_eq!(sample_colour(&t, 0), [0., 0.25, 0.5]);
        assert_eq!(sample_colour(&t, 150), [1., 0.75, 0.25]);
        assert_eq!(sample_scalar(&t, 50), expected[0]);
    }
    let t = track(Interpolation::Hermite);
    assert_eq!(t.in_tangent(1), 2.);
    assert_eq!(t.out_tangent(0), 0.);
}

//! Synthetic geoset colour plan admission; not native renderer precedence claim.
use pcx_gfx::{mdx_opacity::OpacityPlan, mdx_runtime::RuntimePlan};
use pcx_mdl::{AnimTrack, Interpolation, NO_ID};

fn colour_track(keys: &[(u32, [f32; 3])]) -> AnimTrack {
    let mut raw = vec![];
    for &(frame, colour) in keys {
        raw.extend(frame.to_le_bytes());
        for value in colour {
            raw.extend(value.to_le_bytes());
        }
    }
    AnimTrack {
        magic: *b"KGAC",
        count: keys.len() as u32,
        interpolation: Interpolation::Linear,
        global_seq_id: NO_ID,
        raw,
    }
}

fn encode_track(track: &AnimTrack) -> Vec<u8> {
    let mut b = track.magic.to_vec();
    for word in [track.count, track.interpolation as u32, track.global_seq_id] {
        b.extend(word.to_le_bytes());
    }
    b.extend(&track.raw);
    b
}

fn record(payload: &[u8]) -> Vec<u8> {
    let mut b = ((payload.len() + 4) as u32).to_le_bytes().to_vec();
    b.extend(payload);
    b
}

fn chunk(out: &mut Vec<u8>, tag: &[u8; 4], payload: &[u8]) {
    out.extend(tag);
    out.extend((payload.len() as u32).to_le_bytes());
    out.extend(payload);
}

fn model(colour: Option<&AnimTrack>) -> Vec<u8> {
    model_flags(colour, 0x2)
}

fn model_flags(colour: Option<&AnimTrack>, geoset_flags: u32) -> Vec<u8> {
    let mut material = vec![];
    material.extend(0i32.to_le_bytes());
    material.extend(0u32.to_le_bytes());
    material.extend(b"LAYS");
    material.extend(1u32.to_le_bytes());
    for word in [28u32, 2, 0, 0, NO_ID, 0] {
        material.extend(word.to_le_bytes());
    }
    material.extend(1f32.to_le_bytes());
    let mut g = vec![];
    for tag in [b"VRTX", b"MTGC", b"MATS"] {
        g.extend(tag);
        g.extend(0u32.to_le_bytes());
    }
    for word in [0u32, 37, 4] {
        g.extend(word.to_le_bytes());
    }
    for value in [1f32, -2., -3., -4., 2., 3., 4.] {
        g.extend(value.to_le_bytes());
    }
    g.extend(0u32.to_le_bytes());
    let geosets = record(&g).repeat(3);
    let mut animations = vec![];
    for (id, rgb, track, flags) in [
        (NO_ID, [0.125f32; 3], None, geoset_flags),
        (2, [0.25, 0.5, 0.75], colour, geoset_flags),
    ] {
        let mut a = 1f32.to_le_bytes().to_vec();
        a.extend(flags.to_le_bytes());
        for value in rgb {
            a.extend(value.to_le_bytes());
        }
        a.extend(id.to_le_bytes());
        if let Some(t) = track {
            a.extend(encode_track(t));
        }
        animations.extend(record(&a));
    }
    let mut texture = vec![0u8; 268];
    let path = b"synthetic-colour";
    texture[4..4 + path.len()].copy_from_slice(path);
    let mut out = b"MDLX".to_vec();
    chunk(&mut out, b"VERS", &800u32.to_le_bytes());
    chunk(&mut out, b"MTLS", &record(&material));
    chunk(&mut out, b"TEXS", &texture);
    chunk(&mut out, b"GEOS", &geosets);
    chunk(&mut out, b"GEOA", &animations);
    out
}

#[test]
fn sparse_geoset_id_samples_authored_static_colour() {
    let bytes = model(None);
    let plan = RuntimePlan::parse(&bytes).expect("synthetic MDX");
    let opacity = OpacityPlan::checked(&plan, [0, 200]).expect("colour plan");
    assert_eq!(opacity.sample_material_colour(0, 0, 0).unwrap(), [1.0; 3]);
    assert_eq!(
        opacity.sample_material_colour(2, 0, 0).unwrap(),
        [0.25, 0.5, 0.75]
    );
}

#[test]
fn kgac_bgr_track_swizzles_red_and_blue_for_material() {
    let track = colour_track(&[(0, [1.0, 0.0, 0.0])]); // BGR: B=1
    let bytes = model(Some(&track));
    let plan = RuntimePlan::parse(&bytes).expect("synthetic MDX");
    let opacity = OpacityPlan::checked(&plan, [0, 200]).expect("colour plan");
    let rgb = opacity
        .sample_material_colour(2, 0, 0)
        .expect("material colour");
    assert!((rgb[0] - 0.0).abs() < 1e-6);
    assert!((rgb[1] - 0.0).abs() < 1e-6);
    assert!((rgb[2] - 1.0).abs() < 1e-6);
}

#[test]
fn duplicate_kgac_key_time_is_refused() {
    let track = colour_track(&[(20, [0.1, 0.2, 0.3]), (20, [0.4, 0.5, 0.6])]);
    let bytes = model(Some(&track));
    let plan = RuntimePlan::parse(&bytes).expect("synthetic MDX");
    let error = OpacityPlan::checked(&plan, [0, 200]).expect_err("duplicate keys");
    assert_eq!(error.geoset, 2);
    assert!(error.reason.contains(&format!("{:?}", b"KGAC")));
    assert!(error.reason.contains("precedence unverified"));
}

#[test]
fn local_colour_interval_and_global_clock_are_independent() {
    let track = colour_track(&[
        (0, [1., 0., 0.]),
        (100, [0., 1., 0.]),
        (300, [0., 0., 1.]),
        (400, [0.25, 0.5, 0.75]),
    ]);
    let mut plan = RuntimePlan::parse(&model(Some(&track))).unwrap();
    let local = OpacityPlan::checked(&plan, [300, 400]).unwrap();
    assert_eq!(
        local.sample_material_colour(2, 350, 9_999).unwrap(),
        [0.875, 0.25, 0.125]
    );
    assert_eq!(
        local.sample_material_colour(2, 0, 9_999).unwrap(),
        [1., 0., 0.]
    );
    plan.global_sequences = vec![200];
    plan.geoset_animations[1].tracks[0].global_seq_id = 0;
    let global = OpacityPlan::checked(&plan, [300, 400]).unwrap();
    assert_eq!(
        global.sample_material_colour(2, 350, 50).unwrap(),
        [0., 0.5, 0.5]
    );
    assert_eq!(
        global.sample_material_colour(2, 350, 250).unwrap(),
        [0., 0.5, 0.5]
    );
    plan.global_sequences[0] = 0;
    assert!(OpacityPlan::checked(&plan, [300, 400]).is_err());
    plan.global_sequences.clear();
    assert!(OpacityPlan::checked(&plan, [300, 400]).is_err());
}

#[test]
fn nonfinite_keys_tangents_and_bad_shapes_refuse_before_clipping() {
    let track = colour_track(&[(0, [0.; 3]), (100, [1.; 3])]);
    let mut plan = RuntimePlan::parse(&model(Some(&track))).unwrap();
    plan.geoset_animations[1].tracks[0].raw[12..16].copy_from_slice(&f32::NAN.to_le_bytes());
    let error = OpacityPlan::checked(&plan, [300, 400]).unwrap_err();
    assert_eq!(error.geoset, 2);
    assert!(error.reason.contains("nonfinite"));
    let mut plan = RuntimePlan::parse(&model(Some(&track))).unwrap();
    plan.geoset_animations[1].tracks[0].raw.pop();
    assert!(
        OpacityPlan::checked(&plan, [300, 400])
            .unwrap_err()
            .reason
            .contains("size mismatch")
    );
    let mut hermite = track;
    hermite.interpolation = Interpolation::Hermite;
    hermite.raw.clear();
    for frame in [0u32, 100] {
        hermite.raw.extend(frame.to_le_bytes());
        for _ in 0..9 {
            hermite.raw.extend(0f32.to_le_bytes());
        }
    }
    hermite.raw[24..28].copy_from_slice(&f32::INFINITY.to_le_bytes());
    let plan = RuntimePlan::parse(&model(Some(&hermite))).unwrap();
    assert!(
        OpacityPlan::checked(&plan, [300, 400])
            .unwrap_err()
            .reason
            .contains("tangent")
    );
}

#[test]
fn finite_colour_interpolation_overflow_is_reported() {
    let track = colour_track(&[(0, [-f32::MAX; 3]), (100, [f32::MAX; 3])]);
    let plan = RuntimePlan::parse(&model(Some(&track))).unwrap();
    let colours = OpacityPlan::checked(&plan, [0, 100]).unwrap();
    let error = colours.sample_material_colour(2, 50, 0).unwrap_err();
    assert_eq!(error.geoset, 2);
    assert!(error.reason.contains("nonfinite interpolated colour"));
    assert!(colours.sample_material_colour(7, 0, 0).is_err());
}

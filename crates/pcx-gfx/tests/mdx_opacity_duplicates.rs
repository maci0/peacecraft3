//! Synthetic imported-runtime admission; not a native renderer precedence claim.
use pcx_gfx::{mdx_opacity::OpacityPlan, mdx_runtime::RuntimePlan};
use pcx_mdl::{AnimTrack, Interpolation, NO_ID};
fn track(tag: [u8; 4], mode: Interpolation, global: u32, keys: &[(u32, f32)]) -> AnimTrack {
    let mut raw = vec![];
    for &(frame, value) in keys {
        raw.extend(frame.to_le_bytes());
        raw.extend(value.to_le_bytes());
        if mode.has_tangents() {
            raw.extend(0.25f32.to_le_bytes());
            raw.extend(0.75f32.to_le_bytes());
        }
    }
    AnimTrack {
        magic: tag,
        count: keys.len() as u32,
        interpolation: mode,
        global_seq_id: global,
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
fn model(layer: Option<&AnimTrack>, geoset: Option<&AnimTrack>) -> Vec<u8> {
    let mut material = vec![];
    material.extend(0i32.to_le_bytes());
    material.extend(0u32.to_le_bytes());
    material.extend(b"LAYS");
    material.extend(1u32.to_le_bytes());
    let layer_track = layer.map_or(vec![], encode_track);
    for word in [28 + layer_track.len() as u32, 2, 0, 0, NO_ID, 0] {
        material.extend(word.to_le_bytes());
    }
    material.extend(0.5f32.to_le_bytes());
    material.extend(layer_track);
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
    let one = record(&g);
    let geosets = one.repeat(3);
    let mut animations = vec![];
    for (id, alpha, t) in [(NO_ID, 0.75f32, None), (2, 0.25, geoset)] {
        let mut a = alpha.to_le_bytes().to_vec();
        a.extend(0u32.to_le_bytes());
        for value in [0.125f32, 0.375, 0.625] {
            a.extend(value.to_le_bytes());
        }
        a.extend(id.to_le_bytes());
        if let Some(t) = t {
            a.extend(encode_track(t));
        }
        animations.extend(record(&a));
    }
    let mut texture = vec![0u8; 268];
    texture[4..19].copy_from_slice(b"synthetic-alpha");
    let mut out = b"MDLX".to_vec();
    chunk(&mut out, b"VERS", &800u32.to_le_bytes());
    chunk(&mut out, b"MTLS", &record(&material));
    chunk(&mut out, b"TEXS", &texture);
    chunk(&mut out, b"GEOS", &geosets);
    chunk(&mut out, b"GEOA", &animations);
    let mut globals = 100u32.to_le_bytes().to_vec();
    globals.extend(200u32.to_le_bytes());
    chunk(&mut out, b"GLBS", &globals);
    out
}
fn modes() -> [Interpolation; 4] {
    [
        Interpolation::None,
        Interpolation::Linear,
        Interpolation::Hermite,
        Interpolation::Bezier,
    ]
}
fn refused(bytes: &[u8], interval: [u32; 2], geoset: usize, frame: u32, tag: &[u8; 4]) {
    let before = bytes.to_vec();
    let plan = RuntimePlan::parse(bytes).expect("synthetic raw MDX reaches imported runtime");
    assert_eq!(plan.geosets[2].animation_records, [1]);
    assert_eq!(plan.unbound_geoset_animations, [0]);
    let layers = plan.materials[0].layers.clone();
    let tracks = plan.geoset_animations[1].tracks.clone();
    let error = OpacityPlan::checked(&plan, interval)
        .expect_err("duplicate key-time precedence is unverified");
    assert_eq!(error.geoset, geoset);
    assert!(error.reason.contains(&frame.to_string()));
    assert!(error.reason.contains(&format!("{tag:?}")));
    assert!(error.reason.contains("precedence unverified"));
    assert_eq!(plan.materials[0].layers, layers);
    assert_eq!(plan.geoset_animations[1].tracks, tracks);
    assert_eq!(bytes, before);
}
#[test]
fn local_imported_duplicates() {
    for mode in modes() {
        for value in [0.125, 0.875] {
            let t = track(*b"KMTA", mode, NO_ID, &[(1200, 0.125), (1200, value)]);
            refused(&model(Some(&t), None), [1000, 1400], 0, 1200, b"KMTA");
        }
    }
    println!(
        "raw MDX import: local KMTA duplicate-time refusal, all four interpolation modes and equal/different values"
    );
}
#[test]
fn global_imported_duplicates() {
    for mode in modes() {
        let t = track(*b"KGAO", mode, 1, &[(20, 0.125), (20, 0.875)]);
        refused(&model(None, Some(&t)), [1000, 1400], 2, 20, b"KGAO");
    }
    println!(
        "raw MDX import: global KGAO duplicate-time refusal uses sparse authored GEOS 2, not GEOA ordinal 1"
    );
}
#[test]
fn projected_imported_duplicates() {
    for tag in [*b"KMTA", *b"KGAO"] {
        let t = track(tag, Interpolation::Linear, NO_ID, &[(5, 0.125), (5, 0.875)]);
        let bytes = if &tag == b"KMTA" {
            model(Some(&t), None)
        } else {
            model(None, Some(&t))
        };
        refused(
            &bytes,
            [1000, 1400],
            if &tag == b"KMTA" { 0 } else { 2 },
            5,
            &tag,
        );
    }
    println!(
        "raw MDX import: source duplicates outside selected interval refuse before projection"
    );
}
#[test]
fn valid_interpolation_and_boundaries() {
    for mode in modes() {
        let a = track(*b"KMTA", mode, NO_ID, &[(1000, 0.25), (1400, 0.75)]);
        let b = track(*b"KGAO", mode, NO_ID, &[(1000, 0.5), (1400, 1.0)]);
        let bytes = model(Some(&a), Some(&b));
        let raw = bytes.clone();
        let plan = RuntimePlan::parse(&bytes).unwrap();
        let opacity = OpacityPlan::checked(&plan, [1000, 1400]).unwrap();
        for frame in [999, 1000, 1001, 1100, 1200, 1399, 1400, 1401] {
            let expected = pcx_mdl::sample_scalar(&a, frame) * pcx_mdl::sample_scalar(&b, frame);
            assert_eq!(opacity.sample(2, frame, u64::MAX), Some(expected));
        }
        assert_eq!(plan.materials[0].layers[0].alpha_tracks, [a]);
        assert_eq!(plan.geoset_animations[1].tracks, [b]);
        assert_eq!(plan.geoset_animations[1].geoset_id, 2);
        assert_eq!(plan.geoset_animations[1].offset, 28);
        assert_eq!(opacity.sample(3, 1000, 0), None);
        assert_eq!(bytes, raw);
    }
    println!(
        "valid raw MDX: four existing interpolation paths and before/exact/after key boundaries preserve decoder sampling and authored records"
    );
}
#[test]
fn valid_clocks_static_and_single_key() {
    let a = track(
        *b"KMTA",
        Interpolation::Linear,
        NO_ID,
        &[(1000, 0.25), (1400, 0.75)],
    );
    let b = track(*b"KGAO", Interpolation::Linear, 1, &[(0, 0.), (200, 1.)]);
    let plan = RuntimePlan::parse(&model(Some(&a), Some(&b))).unwrap();
    let opacity = OpacityPlan::checked(&plan, [1000, 1400]).unwrap();
    for elapsed in [0, 50, 200, 250, 450, u64::MAX] {
        assert_eq!(
            opacity.sample(2, 1200, elapsed),
            Some(0.5 * (elapsed % 200) as f32 / 200.)
        );
    }
    assert_eq!(opacity.sample(2, 1400, 250), Some(0.1875));
    let single = track(*b"KGAO", Interpolation::Linear, NO_ID, &[(1200, 0.375)]);
    let plan = RuntimePlan::parse(&model(None, Some(&single))).unwrap();
    let opacity = OpacityPlan::checked(&plan, [1000, 1400]).unwrap();
    for frame in [0, 1199, 1200, 1201, u32::MAX] {
        assert_eq!(opacity.sample(2, frame, u64::MAX), Some(0.1875));
    }
    let outside = track(
        *b"KGAO",
        Interpolation::Linear,
        NO_ID,
        &[(0, 0.), (100, 1.)],
    );
    let plan = RuntimePlan::parse(&model(None, Some(&outside))).unwrap();
    let opacity = OpacityPlan::checked(&plan, [1000, 1400]).unwrap();
    assert_eq!(opacity.sample(2, 1200, 50), Some(0.125));
    let empty = track(*b"KGAO", Interpolation::Linear, NO_ID, &[]);
    let plan = RuntimePlan::parse(&model(None, Some(&empty))).unwrap();
    let opacity = OpacityPlan::checked(&plan, [1000, 1400]).unwrap();
    assert_eq!(opacity.sample(2, 1200, 0), Some(0.125));
    println!(
        "valid raw MDX: local frame and independent global elapsed/modulo unchanged; static, empty and single-key behavior preserved"
    );
}

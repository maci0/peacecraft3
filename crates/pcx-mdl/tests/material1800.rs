//! Synthetic v1800 material data, deliberately unlike the v800 layer framing.
use pcx_mdl::{Error, Interpolation, Material, Mdlx};

fn word(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend(value.to_le_bytes());
}

fn floats(bytes: &mut Vec<u8>, values: &[f32]) {
    for value in values {
        bytes.extend(value.to_le_bytes());
    }
}

fn track(tag: &[u8; 4], count: u32, kind: u32, raw: &[u8]) -> Vec<u8> {
    let mut bytes = tag.to_vec();
    for value in [count, kind, 13] {
        word(&mut bytes, value);
    }
    bytes.extend(raw);
    bytes
}

fn layer(textures: &[(u32, u32, Vec<u8>)], tracks: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for value in [0, 4, 0x4000, 99, u32::MAX, 7] {
        word(&mut bytes, value);
    }
    floats(&mut bytes, &[0.25, 2.5, 0.125, 0.375, 0.625, 0.875, 1.25]);
    word(&mut bytes, 0x8765_4321);
    word(&mut bytes, textures.len() as u32);
    for (texture, semantic, track) in textures {
        word(&mut bytes, *texture);
        word(&mut bytes, *semantic);
        bytes.extend(track);
    }
    bytes.extend(tracks);
    let size = bytes.len() as u32;
    bytes[..4].copy_from_slice(&size.to_le_bytes());
    bytes
}

fn material(layers: &[Vec<u8>]) -> Material {
    let mut bytes = Vec::new();
    word(&mut bytes, (-3i32) as u32);
    word(&mut bytes, 0x100);
    bytes.extend(b"LAYS");
    word(&mut bytes, layers.len() as u32);
    for layer in layers {
        bytes.extend(layer);
    }
    Material {
        offset: 0,
        len: bytes.len() as u32 + 4,
        data: bytes,
    }
}

fn container(version: u32, materials: &[Material]) -> Vec<u8> {
    let mut bytes = b"MDLXVERS".to_vec();
    word(&mut bytes, 4);
    word(&mut bytes, version);
    let mut payload = Vec::new();
    for material in materials {
        word(&mut payload, material.len);
        payload.extend(&material.data);
    }
    bytes.extend(b"MTLS");
    word(&mut bytes, payload.len() as u32);
    bytes.extend(payload);
    bytes
}

#[test]
fn fixed_fields_and_subtextures_are_authored_not_classic_defaults() {
    let source = material(&[layer(&[(17, 37, Vec::new()), (19, 89, Vec::new())], &[])]);
    assert!(matches!(
        source.checked_layers(),
        Err(Error::UnknownTrackMagic { .. })
    ));
    let bytes = container(1800, &[source.clone(), source.clone()]);
    let model = Mdlx::parse(&bytes).unwrap();
    let parsed = model.materials_v1800(&bytes).unwrap();
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0], source);
    assert_eq!(parsed[1].offset, source.len as usize);
    assert_eq!(parsed[1].data, source.data);
    let layers = parsed[0].checked_layers_v1800().unwrap();
    let layer = &layers[0];
    assert_eq!(layer.offset, 16);
    assert_eq!(layer.size, 60 + 2 * 8);
    assert_eq!(layer.prefix.texture_id, 99);
    assert_eq!(layer.prefix.static_alpha, 0.25);
    assert_eq!(layer.prefix.coord_id, 7);
    assert_eq!(layer.shader, 0x8765_4321);
    assert_eq!(layer.emissive_gain, 2.5);
    assert_eq!(layer.fresnel_color, [0.125, 0.375, 0.625]);
    assert_eq!(layer.fresnel_opacity, 0.875);
    assert_eq!(layer.fresnel_team_color, 1.25);
    assert_eq!(layer.subtextures[0].offset, 16 + 60);
    assert_eq!(layer.subtextures[0].texture_id, 17);
    assert_eq!(layer.subtextures[0].stored_semantic, 37);
    assert_eq!(layer.subtextures[1].texture_id, 19);
    assert_eq!(layer.subtextures[1].stored_semantic, 89);
    assert!(matches!(
        model.materials(&bytes),
        Err(Error::UnsupportedVersionedChunk { .. })
    ));
    let old_version = Mdlx::parse(&container(800, &[source])).unwrap();
    assert!(matches!(
        old_version.materials_v1800(&bytes),
        Err(Error::UnsupportedVersionedChunk { version: 800, .. })
    ));
}

#[test]
fn texture_tracks_and_cubic_float_tracks_keep_exact_framing_and_order() {
    let mut integer_keys = Vec::new();
    for (time, texture) in [(-7i32, 12), (31, 27)] {
        word(&mut integer_keys, time as u32);
        word(&mut integer_keys, texture);
    }
    let integer_track = track(b"KMTF", 2, 3, &integer_keys);
    let mut scalar_key = (-5i32).to_le_bytes().to_vec();
    floats(&mut scalar_key, &[0.25, 0.5, 0.75]);
    let mut vector_key = 23u32.to_le_bytes().to_vec();
    floats(
        &mut vector_key,
        &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0],
    );
    let mut tracks = Vec::new();
    for tag in [b"KMTA", b"KMTE", b"KFCA", b"KFTC"] {
        tracks.extend(track(tag, 1, 2, &scalar_key));
    }
    tracks.extend(track(b"KFC3", 1, 3, &vector_key));
    let material = material(&[layer(
        &[(17, 37, integer_track), (19, 89, Vec::new())],
        &tracks,
    )]);
    let parsed = material.checked_layers_v1800().unwrap();
    let layer = &parsed[0];
    let integer = layer.subtextures[0].texture_track.as_ref().unwrap();
    assert_eq!(integer.offset, layer.subtextures[0].offset + 8);
    assert_eq!(integer.raw, integer_keys);
    assert_eq!(integer.interpolation, Interpolation::Bezier);
    assert_eq!(integer.stride, 8, "integer headers do not add tangents");
    assert_eq!(
        layer.subtextures[1].offset,
        integer.offset + 16 + integer.raw.len()
    );
    assert_eq!(layer.tracks.len(), 5);
    let mut at = layer.subtextures[1].offset + 8;
    for (parsed, tag) in layer
        .tracks
        .iter()
        .zip([b"KMTA", b"KMTE", b"KFCA", b"KFTC", b"KFC3"])
    {
        assert_eq!(parsed.offset, at);
        assert_eq!(parsed.magic, *tag);
        assert_eq!(parsed.global_seq_id, 13);
        if tag == b"KFC3" {
            assert_eq!(parsed.stride, 40);
            assert_eq!(parsed.raw, vector_key);
        } else {
            assert_eq!(parsed.stride, 16);
            assert_eq!(parsed.raw, scalar_key);
        }
        at += 16 + parsed.raw.len();
    }
    assert_eq!(at, layer.offset + layer.size as usize);
}

#[test]
fn malformed_counts_records_and_unknown_tracks_are_typed_errors() {
    let source = material(&[layer(&[(17, 37, Vec::new())], &[])]);
    for (offset, value) in [(12, 2), (16, u32::MAX), (16 + 56, u32::MAX)] {
        let mut bad = source.clone();
        bad.data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        assert!(bad.checked_layers_v1800().is_err());
    }
    let mut raw = 1u32.to_le_bytes().to_vec();
    floats(&mut raw, &[0.5]);
    let source = material(&[layer(&[(17, 37, Vec::new())], &track(b"KMTA", 1, 1, &raw))]);
    let at = 16 + 60 + 8;
    let mut bad = source.clone();
    bad.data[at..at + 4].copy_from_slice(b"ZZZZ");
    assert!(
        matches!(bad.checked_layers_v1800(), Err(Error::UnknownTrackMagic { at: got, tag: [b'Z', b'Z', b'Z', b'Z'], .. }) if got == at)
    );
    let mut bad = source.clone();
    bad.data[at + 4..at + 8].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(
        matches!(bad.checked_layers_v1800(), Err(Error::TrackOverruns { at: got, .. }) if got == at)
    );
    let mut bad = source;
    bad.data[at + 8..at + 12].copy_from_slice(&9u32.to_le_bytes());
    assert!(
        matches!(bad.checked_layers_v1800(), Err(Error::UnknownTrackInterpolation { at: got, kind: 9, .. }) if got == at)
    );
    let mut bad = material(&[]);
    bad.data.extend(b"LAYS");
    assert!(matches!(
        bad.checked_layers_v1800(),
        Err(Error::RecordShape { chunk: "MTLS", .. })
    ));
}

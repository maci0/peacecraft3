use super::*;
use std::collections::BTreeMap;
fn origin() -> AssetIdentity {
    AssetIdentity {
        source: "synthetic".into(),
        edition: "fixture".into(),
        version: "nonstock".into(),
    }
}
fn emit(map: &mut BTreeMap<u32, u8>, at: u32, data: &[u8]) {
    for (offset, byte) in data.iter().enumerate() {
        assert!(map.insert(at + offset as u32, *byte).is_none());
    }
}
fn immediate(map: &mut BTreeMap<u32, u8>, at: u32, prefix: &[u8], value: u32) {
    let mut bytes = prefix.to_vec();
    bytes.extend(value.to_le_bytes());
    emit(map, at, &bytes);
}
fn fixture() -> BTreeMap<u32, u8> {
    let mut m = BTreeMap::new();
    for (site, target) in [
        (0x5d700d, 0x796c90),
        (0x5d7018, 0x9892a0),
        (0x5d8bf3, 0x480dd0),
        (0x5cf3f5, 0x480dd0),
        (0x5c322d, 0x480dd0),
        (0x698e5d, 0x480dd0),
        (0x5ce423, 0x796c90),
        (0x5ce477, 0x796c90),
        (0x8460d3, 0x796c90),
        (0x845d14, 0x796c90),
        (0x845d9f, 0x845500),
        (0x5ce446, 0x840370),
        (0x5db5c8, 0x840370),
        (0x4334c4, 0xa77598),
    ] {
        immediate(
            &mut m,
            site,
            &[0xe8],
            (i64::from(target) - i64::from(site) - 5) as i32 as u32,
        );
    }
    for (at, prefix) in [
        (0x5d7013, &[0xb9][..]),
        (0x5d8bee, &[0x68]),
        (0x5cf3f0, &[0x68]),
        (0x5c3228, &[0x68]),
        (0x698e58, &[0x68]),
    ] {
        immediate(&mut m, at, prefix, 0x2100);
    }
    emit(&mut m, 0x5d8bfb, &[0xf7, 0xe6]);
    emit(
        &mut m,
        0x845d75,
        &[
            0x8b, 0x46, 0x10, 0x2b, 0xc2, 0x85, 0xc0, 0x0f, 0x8f, 0xd0, 0, 0, 0,
        ],
    );
    emit(&mut m, 0x5ce488, &[0x3b, 0xc1, 0x76, 0x33]);
    for (at, prefix, value) in [
        (0x5d8b2d, &[0xff, 0x35][..], 0x2800),
        (0x710f00, &[0xa1][..], 0x2800),
        (0x5d8b3f, &[0x68], 96),
        (0x5d8b3a, &[0x68], 0x2400),
        (0x5d8b86, &[0x68], 0x2500),
        (0x5d6d30, &[0xc7, 0x05], 0x2200),
        (0x5ce482, &[0x8b, 0x0d], 0x2200),
        (0x8460df, &[0xf3, 0x0f, 0x59, 0x05], 0x2600),
        (0x5d6d44, &[0xc7, 0x04, 0x24], 0.011f32.to_bits()),
        (0x5db5c1, &[0xc7, 0x04, 0x24], 0.17f32.to_bits()),
        (0x5c3237, &[0x81, 0xe1], 0x003f_ffff),
        (0x5c323d, &[0x81, 0xc9], 1.0f32.to_bits()),
        (0x5c326f, &[0xf3, 0x0f, 0x5c, 0x05], 0x2700),
        (0x5d8dd1, &[0xa1], 0x2300),
        (0x5da426, &[0xb8], 9),
    ] {
        immediate(&mut m, at, prefix, value);
    }
    emit(
        &mut m,
        0xa99c23,
        &[0xb7, b'F', 0xb3, b'H', 0xb6, b'a' - b'A'],
    );
    emit(&mut m, 0x5cf407, &[0x83, 0xf8, 2]);
    emit(&mut m, 0x4a41b1, &[0x6a, 0xfd]);
    emit(&mut m, 0x5d6d36, &41u32.to_le_bytes());
    emit(&mut m, 0x10dc6dc, &1u32.to_le_bytes());
    emit(&mut m, 0x2300, &4u32.to_le_bytes());
    emit(&mut m, 0x2600, &500.0f32.to_le_bytes());
    emit(&mut m, 0x2700, &1.0f32.to_le_bytes());
    emit(&mut m, 0x2400, b"prefix-%s-edition-%d.list\0");
    emit(&mut m, 0x2500, b"|:\0");
    m
}
fn decode(map: &BTreeMap<u32, u8>) -> Result<Rules, String> {
    Rules::decode(
        &|at, len| {
            (0..len)
                .map(|i| {
                    map.get(&(at + i as u32))
                        .copied()
                        .ok_or_else(|| format!("missing {at:#x}"))
                })
                .collect()
        },
        origin(),
        Arc::new(crate::script_random::profile::fixture()),
    )
}
#[test]
fn regular_music_rules_derive_nonstock_callback_units_names_fold_and_limits() {
    let rules = decode(&fixture()).unwrap();
    assert_eq!(rules.default_fade_ms, 41);
    assert_eq!(rules.fade_interval_ms, 5);
    assert_eq!(rules.next_interval_ms, 85);
    assert_eq!(rules.repeat_attempts, 3);
    assert_eq!(rules.initial_level, 4);
    assert_eq!(rules.maximum_level, 9);
    assert_eq!(rules.random_index, -3);
    assert_eq!(
        rules.edition_key("Tune", 7).unwrap(),
        "prefix-Tune-edition-7.list"
    );
    assert_eq!(rules.tracks("|a.ogg::b.ogg|").unwrap(), ["a.ogg", "b.ogg"]);
    assert!(rules.same_path("FGH", "fgh"));
    assert!(!rules.same_path("EF", "ef"));
}
#[test]
fn regular_music_rules_refuse_broken_call_stream_fold_and_duration_relationships() {
    for at in [
        0x5d700d, 0x5d8bef, 0x845d7c, 0xa99c27, 0x5d6d30, 0x5ce482, 0x5d8b2d,
    ] {
        let mut map = fixture();
        *map.get_mut(&at).unwrap() ^= 0x40;
        assert!(decode(&map).is_err(), "accepted altered source at {at:#x}");
    }
    let mut map = fixture();
    map.remove(&0x5d6d36);
    assert!(decode(&map).is_err());
}
#[test]
#[ignore = "requires explicitly supplied selected Legacy client image"]
fn selected_client_supplies_regular_music_controller_rules() {
    let path = std::env::var("PCX_LIGHTING_CLIENT_IMAGE").expect("PCX_LIGHTING_CLIENT_IMAGE");
    let image = std::fs::read(&path).unwrap();
    let rules = Rules::read(
        &image,
        crate::campaign_inputs::client_identity(&image, &path),
    )
    .unwrap();
    assert!(rules.same_path("Sound/Track.MP3", "sound/track.mp3"));
    assert!(rules.fade_interval_ms > 0 && rules.next_interval_ms > rules.fade_interval_ms);
    assert!(rules.initial_level <= rules.maximum_level);
    assert_eq!(rules.image_sha256, *rules.random.image_sha256());
}

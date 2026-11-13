use super::*;
use std::collections::BTreeMap;

fn fixture() -> (BTreeMap<u32, Vec<u8>>, BTreeMap<u32, Vec<u8>>) {
    let mut main = BTreeMap::new();
    let mut dll = BTreeMap::new();
    let mut put = |at, prefix: &[u8], value: u32| {
        let mut data = prefix.to_vec();
        data.extend(value.to_le_bytes());
        main.insert(at, data);
    };
    put(0x5da426, &[0xb8], 4);
    put(0x5da430, &[0xa3], 0x1000);
    put(0x5d8dd1, &[0xa1], 0x1000);
    put(0x1000, &[], 2);
    put(0x7c6392, &[0x81, 0xfe], 5);
    for (at, prefix, target, value) in [
        (0x7c6333, &[0x0f, 0x28, 0x15][..], 0x1100, 1f32),
        (0x7c636a, &[0x0f, 0x28, 0x15][..], 0x1200, 0f32),
        (0x7c639a, &[0xf3, 0x0f, 0x10, 0x15][..], 0x1300, 4f32),
        (0x79fa44, &[0xf3, 0x0f, 0x10, 0x0d][..], 0x1400, 1f32),
    ] {
        put(at, prefix, target);
        put(target, &[], value.to_bits());
    }
    main.insert(0x5da5c2, vec![0x6a, 3]);
    main.insert(0x79db4c, vec![0x6a, 2]);
    main.insert(0x5da42b, vec![0x3b, 0xc8, 0x0f, 0x42, 0xc1]);
    for (at, prefix, value) in [
        (0x21114513, &[0xba][..], 4u32),
        (0x21113708, &[0xc7, 0x46, 0x10][..], 4),
        (0x211145a9, &[0xb8][..], 32),
        (0x211145b0, &[0xba][..], 64),
    ] {
        let mut bytes = prefix.to_vec();
        bytes.extend(value.to_le_bytes());
        dll.insert(at, bytes);
    }
    dll.insert(0x211145c2, vec![0xc1, 0xe6, 3]);
    dll.insert(0x2113d29e, vec![0xc1, 0xf8, 6]);
    for (at, prefix, value) in [
        (0x2111448e, &[0xc7, 0x46, 0x68][..], 2u32),
        (0x21114577, &[0xba][..], 0x2004),
        (0x21114590, &[0x0f, 0xb6, 0x87][..], 0x2000),
    ] {
        let mut data = prefix.to_vec();
        data.extend(value.to_le_bytes());
        dll.insert(at, data);
    }
    dll.insert(0x2111458a, vec![0xc1, 0xf8, 1]);
    dll.insert(0x2002, vec![16]);
    (main, dll)
}
fn read(map: &BTreeMap<u32, Vec<u8>>, va: u32, len: usize) -> Result<Vec<u8>, String> {
    map.get(&va)
        .map(|data| data[..len.min(data.len())].to_vec())
        .ok_or_else(|| format!("synthetic window {va:#x}"))
}
fn origin() -> AssetIdentity {
    AssetIdentity {
        source: "synthetic local mixer".into(),
        edition: "fixture".into(),
        version: "nonstock".into(),
    }
}
fn decode(main: &BTreeMap<u32, Vec<u8>>, dll: &BTreeMap<u32, Vec<u8>>) -> Result<Rules, String> {
    Rules::decode(
        &|at, len| read(main, at, len),
        &|at, len| read(dll, at, len),
        origin(),
    )
}
pub(crate) fn rules() -> Rules {
    let (main, dll) = fixture();
    decode(&main, &dll).unwrap()
}

#[test]
fn regular_volume_derives_nonstock_curve_default_channel_and_backend_unit() {
    let rules = rules();
    assert_eq!(
        (
            rules.initial_level(),
            rules.maximum_level(),
            rules.channel()
        ),
        (2, 4, 3)
    );
    assert_eq!(rules.curve, [0, 0, 1, 2, 4]);
    assert_eq!(rules.gain(4, 1.0, 1.0).unwrap(), 1.0);
    assert_eq!(rules.gain(3, 1.0, 1.0).unwrap(), 0.25);
    assert_eq!(rules.gain(4, 1.0, 0.5).unwrap(), 0.125);
    assert_eq!(rules.gain(4, 0.5, 1.0).unwrap(), 0.125);
    assert_eq!(rules.gain(4, 0.5, 0.5).unwrap(), 0.0);
    assert_eq!(rules.gain(4, 2.0, 1.0).unwrap(), 1.0);
    assert_eq!(rules.gain(4, 1.0, 0.0).unwrap(), 0.0);
}

#[test]
fn regular_volume_refuses_partial_sources_changed_relationships_and_unsupported_values() {
    let (main, dll) = fixture();
    for (at, data) in [
        (0x5da426, vec![0xb8]),
        (0x5da42b, vec![0; 5]),
        (0x5da5c2, vec![0x6a, 0xff]),
        (0x1000, 9u32.to_le_bytes().to_vec()),
    ] {
        let mut changed = main.clone();
        changed.insert(at, data);
        assert!(decode(&changed, &dll).is_err(), "{at:#x}");
    }
    let mut changed = dll.clone();
    changed.insert(0x2113d29e, vec![0xc1, 0xf8, 7]);
    assert!(
        decode(&main, &changed).is_err(),
        "mixer copy must agree with coefficient unity"
    );
    changed.insert(0x2113d29e, vec![0xc1, 0xf8, 32]);
    assert!(decode(&main, &changed).is_err());
    let mut changed = dll.clone();
    changed.insert(0x2002, vec![15]);
    assert!(decode(&main, &changed).is_err());
    let rules = rules();
    for level in [-1, 5, i32::MAX] {
        assert!(rules.level(level).is_err());
    }
    for (level, group, user) in [
        (5, 1., 1.),
        (1, f32::NAN, 1.),
        (1, 1., f32::INFINITY),
        (1, -1., 1.),
    ] {
        assert!(rules.gain(level, group, user).is_err());
    }
    assert!(Rules::read(b"missing client", b"missing backend", origin()).is_err());
}

#[test]
#[ignore = "requires explicit PCX_WAR3_EXE and PCX_MUSIC_BACKEND local selected images"]
fn selected_regular_volume_source_and_optional_native_lut_reference() {
    use std::io::Read as _;
    let read = |path: &std::ffi::OsStr, limit: u64| {
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .unwrap()
            .take(limit + 1)
            .read_to_end(&mut bytes)
            .unwrap();
        assert!(bytes.len() as u64 <= limit);
        bytes
    };
    let image = read(
        &std::env::var_os("PCX_WAR3_EXE").expect("explicit selected client"),
        32 << 20,
    );
    let backend_path = std::env::var_os("PCX_MUSIC_BACKEND").expect("explicit local backend");
    let backend = read(&backend_path, 4 << 20);
    let mut origin = origin();
    origin.source = backend_path.to_string_lossy().into_owned();
    origin.edition = "Legacy".into();
    origin.version = "3.0.0.0".into();
    let rules = Rules::read(&image, &backend, origin).unwrap();
    assert_eq!(rules.initial_level(), rules.maximum_level());
    assert_eq!(rules.curve.first(), Some(&0));
    assert_eq!(rules.curve.last(), Some(&rules.maximum));
    assert_eq!(rules.gain(rules.maximum, 1.0, 1.0).unwrap(), 1.0);
    if let Some(path) = std::env::var_os("PCX_MUSIC_LUT") {
        let reference = read(&path, (rules.curve.len() * 4) as u64);
        assert_eq!(reference.len(), rules.curve.len() * 4);
        for (index, bytes) in reference.as_chunks::<4>().0.iter().enumerate() {
            assert_eq!(
                rules.curve[index],
                u32::from_le_bytes(*bytes),
                "native LUT index {index}"
            );
        }
    }
}

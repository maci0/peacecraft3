#![allow(dead_code)]
#[path = "../src/sounds.rs"]
mod sounds;

use sounds::{SOUND_WORD_COUNT, SOUNDS_NAME, SOUNDS_VERSION, SoundError, Sounds};

fn make_record(name: &[u8], file: &[u8], env: &[u8], words: [u32; SOUND_WORD_COUNT]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(name);
    out.push(0);
    out.extend_from_slice(file);
    out.push(0);
    out.extend_from_slice(env);
    out.push(0);
    for w in words {
        out.extend(w.to_le_bytes());
    }
    out
}

#[test]
fn round_trips_one_record() {
    let words = [
        0u32,
        127,
        127,
        100,
        0x4f80_0000,
        0x4f80_0000,
        0xffff_ffff,
        0,
        0x4f80_0000,
        0x4f80_0000,
        0x4f80_0000,
        0x4f80_0000,
        0x4f80_0000,
        0xffff_ffff,
        0x4f80_0000,
        0x4f80_0000,
        0x4f80_0000,
    ];
    let mut data = 1u32.to_le_bytes().to_vec();
    data.extend(1u32.to_le_bytes());
    data.extend(make_record(
        b"gg_snd_X",
        b"Units\\X\\X.wav",
        b"DefaultEAXON",
        words,
    ));
    let parsed = Sounds::parse(&data).unwrap();
    assert_eq!(parsed.version, SOUNDS_VERSION);
    assert_eq!(parsed.records.len(), 1);
    let r = &parsed.records[0];
    assert_eq!(r.name, b"gg_snd_X");
    assert_eq!(r.file, b"Units\\X\\X.wav");
    assert_eq!(r.environment, b"DefaultEAXON");
    assert_eq!(r.words, words);
    assert_eq!(r.flags(), words[0]);
}

#[test]
fn refuses_unsupported_version() {
    let mut data = 2u32.to_le_bytes().to_vec();
    data.extend(0u32.to_le_bytes());
    assert!(matches!(
        Sounds::parse(&data),
        Err(SoundError::UnsupportedVersion { .. })
    ));
}

#[test]
fn refuses_trailing_bytes() {
    let mut data = 1u32.to_le_bytes().to_vec();
    data.extend(0u32.to_le_bytes());
    data.push(0);
    assert!(matches!(
        Sounds::parse(&data),
        Err(SoundError::TrailingBytes { .. })
    ));
}

#[test]
fn refuses_unterminated_string() {
    let mut data = 1u32.to_le_bytes().to_vec();
    data.extend(1u32.to_le_bytes());
    data.extend(b"nonul");
    data.extend(vec![1u8; 71]);
    let err = Sounds::parse(&data);
    assert!(matches!(err, Err(SoundError::UnterminatedString { .. })));
}

#[test]
fn refuses_too_many_records() {
    let data = {
        let mut v = 1u32.to_le_bytes().to_vec();
        v.extend(u32::MAX.to_le_bytes());
        v
    };
    assert!(matches!(
        Sounds::parse(&data),
        Err(SoundError::TooManyRecords { .. })
    ));
}

#[test]
fn parses_empty_record_list() {
    let mut data = 1u32.to_le_bytes().to_vec();
    data.extend(0u32.to_le_bytes());
    let parsed = Sounds::parse(&data).unwrap();
    assert!(parsed.records.is_empty());
}

#[test]
fn constants_are_stable() {
    assert_eq!(SOUNDS_NAME, "war3map.w3s");
    assert_eq!(SOUND_WORD_COUNT, 17);
}

#[test]
#[ignore = "opens shipped installs"]
fn sweep_shipped_maps() {
    // Exercise parseability on any available install.
    let mut dirs = Vec::new();
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let d = base.join("wc3/game/app");
    if d.is_dir() {
        dirs.push(d);
    }
    if let Ok(legacy) = std::env::var("PEACECRAFT3_LEGACY_DIR") {
        dirs.push(std::path::PathBuf::from(legacy));
    }
    for dir in dirs {
        let mut set = match pcx_mpq::MpqSet::open_wc3(&dir) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let maps = match pcx_map::shipped_maps(&mut set) {
            Ok(m) => m,
            Err(_) => continue,
        };
        for path in maps {
            let raw = match set.extract(&path) {
                Ok(b) => b,
                Err(_) => continue,
            };
            let map = match pcx_map::Map::parse(&raw) {
                Ok(m) => m,
                Err(_) => continue,
            };
            if let Ok(bytes) = map.extract("war3map.w3s")
                && let Err(e) = Sounds::parse(&bytes)
            {
                panic!("{dir:?} {path}: {e}");
            }
        }
    }
}

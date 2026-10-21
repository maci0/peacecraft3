use super::*;
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};

#[derive(Default)]
pub(crate) struct Memory {
    pub bytes: Mutex<Option<Vec<u8>>>,
    pub fail_read: AtomicBool,
    pub fail_write: AtomicBool,
}
impl Storage for Memory {
    fn load(&self, limit: usize) -> Result<Option<Vec<u8>>, String> {
        if self.fail_read.load(Ordering::Relaxed) {
            return Err("synthetic read failure".into());
        }
        let bytes = self.bytes.lock().unwrap().clone();
        if bytes.as_ref().is_some_and(|b| b.len() > limit) {
            return Err("synthetic read limit".into());
        }
        Ok(bytes)
    }
    fn save(&self, bytes: &[u8]) -> Result<(), String> {
        if self.fail_write.load(Ordering::Relaxed) {
            return Err("synthetic save failure".into());
        }
        *self.bytes.lock().unwrap() = Some(bytes.to_vec());
        Ok(())
    }
}
pub(crate) fn budget() -> Budget {
    Budget {
        caches: 4,
        entries: 16,
        text_bytes: 256,
        storage_bytes: 1024,
        handle_bytes: 128,
    }
}

#[test]
fn unit_profiles_preserve_typed_namespaces_and_read_old_primitive_profiles() {
    let b = budget();
    let saved = cached_unit::tests::fixture()
        .encode(b.storage_bytes)
        .unwrap();
    let mut profile = Profile::default();
    profile
        .put("cache", "m".into(), "k".into(), Stored::Integer(17), b)
        .unwrap();
    let mut old = profile.encode(b).unwrap();
    old[8] = 1;
    assert_eq!(Profile::decode(&old, b).unwrap(), profile);
    profile
        .put(
            "cache",
            "m".into(),
            "k".into(),
            Stored::Unit(saved.clone()),
            b,
        )
        .unwrap();
    let bytes = profile.encode(b).unwrap();
    assert_eq!(Profile::decode(&bytes, b).unwrap(), profile);
    let mut falsely_old = bytes;
    falsely_old[8] = 1;
    assert!(Profile::decode(&falsely_old, b).is_err());
    assert!(
        profile
            .put(
                "cache",
                "m".into(),
                "k2".into(),
                Stored::Unit(saved.clone()),
                Budget {
                    storage_bytes: saved.len(),
                    ..b
                }
            )
            .is_err()
    );
    profile.flush("cache", Some("m"), Some(("k", Kind::Unit)));
    assert_eq!(
        profile.get("cache", "m", "k", Kind::Integer),
        Some(&Stored::Integer(17))
    );
    profile
        .put("cache", "m".into(), "k2".into(), Stored::Unit(saved), b)
        .unwrap();
    profile.flush("cache", Some("m"), None);
    assert_eq!(profile.unit_bytes, 0);
}

#[test]
fn canonical_profile_roundtrip_preserves_independent_types_and_real_bits() {
    let b = budget();
    let mut first = Profile::default();
    let mut second = Profile::default();
    for (kind, value) in [
        ("a", Stored::Real((-0.0f32).to_bits())),
        ("b", Stored::Integer(-19)),
        ("c", Stored::String("héllo".into())),
        ("d", Stored::Boolean(false)),
    ] {
        first
            .put(kind, "mission".into(), "key".into(), value, b)
            .unwrap();
    }
    for (kind, value) in [
        ("d", Stored::Boolean(false)),
        ("c", Stored::String("héllo".into())),
        ("b", Stored::Integer(-19)),
        ("a", Stored::Real((-0.0f32).to_bits())),
    ] {
        second
            .put(kind, "mission".into(), "key".into(), value, b)
            .unwrap();
    }
    let bytes = first.encode(b).unwrap();
    assert_eq!(bytes, second.encode(b).unwrap());
    assert_eq!(Profile::decode(&bytes, b).unwrap(), first);
    assert_eq!(key("MixED/Name", 64).unwrap(), "mixed/name");
    assert_ne!(key("a/b", 64).unwrap(), key("a\\b", 64).unwrap());
    assert!(key("unicode-é", 64).is_err());
    assert!(key("nul\0key", 64).is_err());
}

#[test]
fn malformed_profile_and_limits_refuse_without_inventing_empty_data() {
    let b = budget();
    let mut p = Profile::default();
    p.put("cache", "m".into(), "k".into(), Stored::Integer(3), b)
        .unwrap();
    let bytes = p.encode(b).unwrap();
    for n in 0..bytes.len() {
        assert!(Profile::decode(&bytes[..n], b).is_err(), "prefix {n}");
    }
    let mut extra = bytes.clone();
    extra.push(0);
    assert!(Profile::decode(&extra, b).is_err());
    let mut changed = bytes.clone();
    changed[8] = 3;
    assert!(Profile::decode(&changed, b).is_err());
    changed = bytes.clone();
    changed[9..13].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(Profile::decode(&changed, b).is_err());
    assert!(Profile::decode(b"Warcraft III recorded game", b).is_err());
    assert!(
        Profile::decode(
            &bytes,
            Budget {
                storage_bytes: bytes.len() - 1,
                ..b
            }
        )
        .is_err()
    );
    assert!(Profile::decode(&bytes, Budget { entries: 0, ..b }).is_err());
    assert!(Profile::decode(&bytes, Budget { text_bytes: 1, ..b }).is_err());
    assert_eq!(p.encode(b).unwrap(), bytes);
    assert!(
        p.put(
            "cache",
            "m".into(),
            "other".into(),
            Stored::String("x".repeat(256)),
            b
        )
        .is_err()
    );
    assert_eq!(p.encode(b).unwrap(), bytes);
    p.put(
        "cache",
        "m".into(),
        "k".into(),
        Stored::Integer(7),
        Budget { entries: 1, ..b },
    )
    .unwrap();
    assert!(
        p.put(
            "cache",
            "m".into(),
            "other".into(),
            Stored::Integer(8),
            Budget { entries: 1, ..b }
        )
        .is_err()
    );
}

#[test]
fn typed_and_mission_flushes_preserve_other_values_and_budget_accounting() {
    let b = budget();
    let mut p = Profile::default();
    p.put("cache", "m".into(), "k".into(), Stored::Integer(0), b)
        .unwrap();
    p.put("cache", "m".into(), "k".into(), Stored::Boolean(false), b)
        .unwrap();
    p.put(
        "cache",
        "n".into(),
        "k".into(),
        Stored::String("other".into()),
        b,
    )
    .unwrap();
    p.flush("cache", Some("m"), Some(("k", Kind::Integer)));
    assert!(p.get("cache", "m", "k", Kind::Integer).is_none());
    assert_eq!(
        p.get("cache", "m", "k", Kind::Boolean),
        Some(&Stored::Boolean(false))
    );
    p.flush("cache", Some("m"), None);
    assert!(p.get("cache", "m", "k", Kind::Boolean).is_none());
    assert_eq!(p.entries, 1);
    assert_eq!(p, Profile::decode(&p.encode(b).unwrap(), b).unwrap());
    p.flush("cache", None, None);
    assert_eq!(p, Profile::default());
}

#[test]
fn saved_profile_rejects_invalid_typed_values_and_duplicate_cache_names() {
    let b = budget();
    let mut p = Profile::default();
    p.put("cache", "m".into(), "k".into(), Stored::Boolean(true), b)
        .unwrap();
    let mut bytes = p.encode(b).unwrap();
    *bytes.last_mut().unwrap() = 2;
    assert!(Profile::decode(&bytes, b).is_err());
    let mut p = Profile::default();
    p.put(
        "cache",
        "m".into(),
        "k".into(),
        Stored::String("x".into()),
        b,
    )
    .unwrap();
    let mut bytes = p.encode(b).unwrap();
    *bytes.last_mut().unwrap() = 0;
    assert!(Profile::decode(&bytes, b).is_err());
    let mut p = Profile::default();
    p.open("one", b).unwrap();
    p.open("two", b).unwrap();
    let mut bytes = p.encode(b).unwrap();
    let at = bytes.windows(3).position(|w| w == b"two").unwrap();
    bytes[at..at + 3].copy_from_slice(b"one");
    assert!(Profile::decode(&bytes, b).is_err());
}

use bevy::asset::io::memory::Dir;
use peacecraft3::browser_read::*;
use std::path::Path;

use crate::browser_mpq_fixtures as fixtures;
#[cfg_attr(test, test)]
pub fn cached_limits_and_receipt() {
    let store = Dir::default();
    store.insert_asset(Path::new("menu/input"), vec![1, 2, 3]);
    let read = read_checked(&store, &[], 42, "menu\\input", 3)
        .unwrap()
        .unwrap();
    assert_eq!(read.bytes, [1, 2, 3]);
    assert_eq!(read.receipt.key, "menu/input");
    assert_eq!(read.receipt.requested_path, "menu\\input");
    assert_eq!(read.receipt.revision, 42);
    assert_eq!(read.receipt.byte_len, 3);
    assert_eq!(read.receipt.archives, None);
    assert!(matches!(
        read_checked(&store, &[], 42, "menu/input", 2),
        Err(ReadError::BudgetExceeded {
            byte_len: 3,
            max_bytes: 2,
            revision: 42,
            ..
        })
    ));
    assert_eq!(
        store.get_asset(Path::new("menu/input")).unwrap().value(),
        [1, 2, 3]
    );
    assert!(
        read_checked(&store, &[], 43, "absent", 0)
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        read_checked(&store, &[], 43, "../", 3),
        Err(ReadError::InvalidPath { .. })
    ));
    store.insert_asset(Path::new("empty"), Vec::<u8>::new());
    assert!(read_checked(&store, &[], 43, "empty", 0).unwrap().is_some());
}
#[cfg_attr(test, test)]
pub fn decode_failure_and_missing() {
    let mut archives = [fixtures::archive(
        &[1, 0],
        4,
        fixtures::FLAGS | pcx_mpq::flags::COMPRESS,
        0,
        "exact bad source",
    )];
    match extract_fallible(&mut archives, 9, "fixture", 4).unwrap_err() {
        ReadError::Extraction {
            origin,
            key,
            source,
        } => {
            assert_eq!(origin.name, "exact bad source");
            assert_eq!(origin.ordinal, 0);
            assert_eq!(key, "fixture");
            assert!(matches!(
                *source,
                pcx_mpq::Error::Payload(pcx_mpq::payload::PayloadError::UnsupportedCompression {
                    mask: 1
                })
            ));
        }
        other => panic!("{other:?}"),
    }
    assert!(
        extract_fallible(&mut archives, 9, "missing", 0)
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        read_checked(&Dir::default(), &archives, 9, "fixture", 4),
        Err(ReadError::ArchiveBudgetUnsupported { revision: 9, .. })
    ));
}
#[cfg_attr(test, test)]
pub fn equal_origins_and_ambiguity() {
    let mut archives = [
        fixtures::archive(&[2, 3], 2, fixtures::FLAGS, 0, "first exact source"),
        fixtures::archive(&[2, 3], 2, fixtures::FLAGS, 0, "second exact source"),
    ];
    let read = extract_fallible(&mut archives, 11, "fixture", 2)
        .unwrap()
        .unwrap();
    assert_eq!(read.bytes, [2, 3]);
    assert_eq!(
        read.receipt.archives,
        Some(vec![
            ArchiveOrigin {
                ordinal: 0,
                name: "first exact source".into()
            },
            ArchiveOrigin {
                ordinal: 1,
                name: "second exact source".into()
            }
        ])
    );
    archives[1] = fixtures::archive(&[4, 5], 2, fixtures::FLAGS, 0, "second exact source");
    let error = extract_fallible(&mut archives, 11, "fixture", 2).unwrap_err();
    assert!(matches!(error, ReadError::Ambiguous { .. }));
    assert!(error.to_string().starts_with("AMBIGUOUS_MEMBER:"));
    assert!(
        matches!(read_checked(&Dir::default(),&archives,11,"fixture",2),Err(ReadError::ArchiveBudgetUnsupported{candidates,..}) if candidates.len()==2)
    );
}
#[cfg_attr(test, test)]
pub fn invalid_present_and_declared_limit() {
    let mut archives = [fixtures::archive(
        &[2, 3],
        2,
        fixtures::FLAGS,
        7,
        "invalid block",
    )];
    assert!(matches!(
        extract_fallible(&mut archives, 0, "fixture", 2),
        Err(ReadError::InvalidSelection { .. })
    ));
    assert!(matches!(
        read_checked(&Dir::default(), &archives, 0, "fixture", 2),
        Err(ReadError::ArchiveBudgetUnsupported { .. })
    ));
    archives[0] = fixtures::archive(&[2, 3], u32::MAX, fixtures::FLAGS, 0, "oversized");
    assert!(matches!(
        extract_fallible(&mut archives, 0, "fixture", 2),
        Err(ReadError::BudgetExceeded {
            byte_len: 4294967295,
            ..
        })
    ));
}

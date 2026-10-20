use crate::browser_mpq_fixtures as fixtures;
use crate::mpq_fixtures as codec_fixture;
use bevy::asset::io::memory::Dir;
use pcx_mpq::{ExtractionBudget, ExtractionCause, checked::ReadError as PreflightError};
use peacecraft3::browser_read::*;
fn policy(copy: usize, packed: usize, decoded: usize) -> BoundedReadPolicy<'static> {
    BoundedReadPolicy {
        max_copy_bytes: copy,
        extraction: ExtractionBudget {
            max_raw_packed_bytes: packed,
            max_decoded_bytes: decoded,
        },
        huffman: None,
    }
}
#[cfg_attr(test, test)]
pub fn missing_cached_receipts() {
    let store = Dir::default();
    let mut archives = [fixtures::archive(
        &[2, 3],
        2,
        fixtures::FLAGS,
        0,
        "supplied source",
    )];
    assert!(
        read_bounded(&store, &mut archives, 91, "missing", policy(0, 0, 0))
            .unwrap()
            .is_none()
    );
    store.insert_asset(std::path::Path::new("fixture"), vec![5, 6]);
    let read = read_bounded(&store, &mut archives, 91, "./fixture", policy(2, 0, 0))
        .unwrap()
        .unwrap();
    assert_eq!(read.bytes, [5, 6]);
    assert_eq!(read.receipt.key, "fixture");
    assert_eq!(read.receipt.requested_path, "./fixture");
    assert_eq!(read.receipt.revision, 91);
    assert_eq!(read.receipt.archives, None);
    assert!(matches!(
        read_bounded(&store, &mut archives, 91, "fixture", policy(1, 0, 0)),
        Err(ReadError::BudgetExceeded {
            byte_len: 2,
            max_bytes: 1,
            ..
        })
    ));
    assert_eq!(
        store
            .get_asset(std::path::Path::new("fixture"))
            .unwrap()
            .value(),
        [5, 6]
    );
}
#[cfg_attr(test, test)]
pub fn all_origins_and_ambiguity() {
    let store = Dir::default();
    let mut archives = [
        fixtures::archive(&[2, 3], 2, fixtures::FLAGS, 0, "first"),
        fixtures::archive(&[2, 3], 2, fixtures::FLAGS, 0, "second"),
        fixtures::archive(&[2, 3], 2, fixtures::FLAGS, 0, "third"),
    ];
    let read = read_bounded(&store, &mut archives, 92, "./fixture", policy(2, 2, 2))
        .unwrap()
        .unwrap();
    assert_eq!(read.bytes, [2, 3]);
    assert_eq!(read.receipt.requested_path, "./fixture");
    assert_eq!(read.receipt.revision, 92);
    assert_eq!(
        read.receipt
            .archives
            .unwrap()
            .iter()
            .map(|o| (o.ordinal, o.name.as_str()))
            .collect::<Vec<_>>(),
        [(0, "first"), (1, "second"), (2, "third")]
    );
    archives[2] = fixtures::archive(&[4, 5], 2, fixtures::FLAGS, 0, "third");
    match read_bounded(&store, &mut archives, 92, "./fixture", policy(2, 2, 2)).unwrap_err() {
        ReadError::BoundedAmbiguous {
            requested_path,
            key,
            revision,
            matching_archives,
            first,
            other,
        } => {
            assert_eq!(requested_path, "./fixture");
            assert_eq!(key, "fixture");
            assert_eq!(revision, 92);
            assert_eq!((first, other), (0, 2));
            assert_eq!(matching_archives.len(), 3);
        }
        error => panic!("{error:?}"),
    }
    assert!(store.get_asset(std::path::Path::new("fixture")).is_none());
}
#[cfg_attr(test, test)]
pub fn malformed_not_missing() {
    let store = Dir::default();
    let mut archives = [fixtures::archive(
        &[1, 0],
        4,
        fixtures::FLAGS | pcx_mpq::flags::COMPRESS,
        0,
        "malformed payload",
    )];
    match read_bounded(&store, &mut archives, 93, "./fixture", policy(4, 2, 4)).unwrap_err() {
        ReadError::BoundedExtraction {
            requested_path,
            key,
            revision,
            origin,
            matching_archives,
            source,
        } => {
            assert_eq!(requested_path, "./fixture");
            assert_eq!(key, "fixture");
            assert_eq!(revision, 93);
            assert_eq!(origin.name, "malformed payload");
            assert_eq!(matching_archives.len(), 1);
            assert!(matches!(
                source.cause.as_ref(),
                ExtractionCause::Mpq(pcx_mpq::Error::Payload(
                    pcx_mpq::payload::PayloadError::UnsupportedCompression { mask: 1 }
                ))
            ));
        }
        error => panic!("{error:?}"),
    }
    archives[0] = fixtures::archive(&[2, 3], 2, fixtures::FLAGS, 7, "malformed selection");
    assert!(
        matches!(read_bounded(&store,&mut archives,93,"fixture",policy(2,2,2)),Err(ReadError::BoundedExtraction{source,..}) if matches!(source.cause.as_ref(),ExtractionCause::Mpq(pcx_mpq::Error::BadBlockIndex{index:7,..})))
    );
}
#[cfg_attr(test, test)]
pub fn packed_decoded_and_copy_caps() {
    let store = Dir::default();
    let mut archives = [fixtures::archive(
        &[2, 3],
        2,
        fixtures::FLAGS,
        0,
        "bounded bytes",
    )];
    assert!(
        read_bounded(&store, &mut archives, 94, "fixture", policy(2, 2, 2))
            .unwrap()
            .is_some()
    );
    for (limits, region, max) in [
        (policy(2, 1, 2), "packed member", 1),
        (policy(2, 2, 1), "decoded member", 1),
        (policy(1, 2, 2), "decoded member", 1),
    ] {
        assert!(
            matches!(read_bounded(&store,&mut archives,94,"fixture",limits),Err(ReadError::BoundedExtraction{source,..}) if matches!(source.cause.as_ref(),ExtractionCause::Preflight(PreflightError::BudgetExceeded{member,max_bytes,..}) if *member==region&&*max_bytes==max))
        );
    }
    assert!(store.get_asset(std::path::Path::new("fixture")).is_none());
}
#[cfg_attr(test, test)]
pub fn archive_and_caller_supplied_codecs() {
    let store = Dir::default();
    let pe = codec_fixture::synthetic_pe();
    let table = pcx_mpq::huffman::HuffmanTable::from_pe(&pe, "explicit synthetic codec").unwrap();
    let mut archives = [fixtures::archive(
        &[1, 0, 0x82, 0],
        2,
        fixtures::FLAGS | pcx_mpq::flags::COMPRESS,
        0,
        "huffman member",
    )];
    archives[0]
        .set_huffman_table(&pe, "archive held synthetic codec")
        .unwrap();
    let imported = read_bounded(&store, &mut archives, 95, "fixture", policy(2, 4, 2))
        .unwrap()
        .unwrap();
    assert_eq!(imported.bytes, b"AA");
    let mut supplied = policy(2, 4, 2);
    supplied.huffman = Some(&table);
    let read = read_bounded(&store, &mut archives, 95, "fixture", supplied)
        .unwrap()
        .unwrap();
    assert_eq!(read.bytes, b"AA");
    assert_eq!(read.receipt.archives.unwrap()[0].name, "huffman member");
    assert_eq!(
        table.source,
        std::path::Path::new("explicit synthetic codec")
    );
}

#[path = "browser_unreachable_fixture.rs"]
pub(crate) mod unreachable_fixture;
#[cfg_attr(test, test)]
pub fn unreachable_no_fallback() {
    let mut sources = [
        unreachable_fixture::unreachable_probe(),
        fixtures::archive(&[2, 3], 2, fixtures::FLAGS, 0, "later valid archive"),
    ];
    assert!(
        matches!(read_bounded(&Dir::default(),&mut sources,113,"fixture",policy(2,2,2)),Err(ReadError::BoundedExtraction{source,matching_archives,..}) if matching_archives.len()==2&&matches!(source.cause.as_ref(),ExtractionCause::Mpq(pcx_mpq::Error::NotFound(_))))
    );
}

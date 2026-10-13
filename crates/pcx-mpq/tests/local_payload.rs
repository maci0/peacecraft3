//! Explicit local-fixture check; archive contents never enter the repository.
use pcx_mpq::{MpqArchive, flags};

#[test]
#[ignore = "requires PEACECRAFT3_FORMAT_MPQ pointing to a local original archive"]
fn original_archive_payloads_have_declared_sizes() {
    let path = std::env::var_os("PEACECRAFT3_FORMAT_MPQ")
        .expect("set PEACECRAFT3_FORMAT_MPQ to the original archive to test");
    let mut archive = MpqArchive::open(&path).expect("open original archive");
    // A patch without (listfile) can be checked against names derived from other
    // local archives; that is a candidate sweep, not proof of complete coverage.
    let candidate_source = std::env::var_os("PEACECRAFT3_FORMAT_MPQ_NAMES");
    let list = if let Some(source) = &candidate_source {
        std::fs::read(source).expect("read explicit local candidate names")
    } else {
        archive
            .extract("(listfile)")
            .expect("extract original listfile")
    };
    let mut supported = 0;
    let mut unsupported = 0;
    let mut single_compressed = 0;
    let mut legacy_imploded = 0;
    for name in String::from_utf8_lossy(&list)
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let Some(block) = archive.find_block(name).copied() else {
            continue;
        };
        match archive.extract(name) {
            Ok(bytes) => {
                assert_eq!(bytes.len(), block.file_size as usize, "{name}");
                supported += 1;
                single_compressed += usize::from(
                    block.flags & flags::SINGLE_UNIT != 0
                        && block.flags & (flags::COMPRESS | flags::IMPLODE) != 0
                        && block.compressed_size != block.file_size,
                );
                legacy_imploded += usize::from(block.flags & flags::IMPLODE != 0);
            }
            Err(pcx_mpq::Error::Payload(
                pcx_mpq::payload::PayloadError::UnsupportedCompression { .. },
            )) => unsupported += 1,
            Err(error) => panic!("{name}: {error}"),
        }
    }
    assert!(supported > 0, "no successfully extracted original files");
    eprintln!(
        "archive={path:?} names={candidate_source:?} supported={supported} unsupported={unsupported} compressed_single={single_compressed} legacy_imploded={legacy_imploded}"
    );
}

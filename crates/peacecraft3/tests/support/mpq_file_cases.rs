use crate::mpq_fixtures::*;
use pcx_mpq::{Error, ExtractionBudget, ExtractionCause, MpqArchive, flags, payload::PayloadError};
use std::{fs, path::PathBuf};
struct Directory(PathBuf);
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
#[test]
fn file_metadata_and_discovery_boundary() {
    let root = std::env::temp_dir().join(format!("synthetic-file-mpq-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let dir = Directory(root);
    let member = "File\\Huffman.bin";
    let file = dir.0.join("caller.mpq");
    let bytes = raw(
        member,
        &[1, 0, 0x82, 0],
        block(40, 4, 2, flags::SINGLE_UNIT | flags::COMPRESS),
        &[0],
        0,
    );
    fs::write(&file, &bytes).unwrap();
    fs::write(dir.0.join("Storm.dll"), b"malformed synthetic input").unwrap();
    let mut a = MpqArchive::open(&file).unwrap();
    let e = a
        .extract_bounded(
            member,
            ExtractionBudget {
                max_raw_packed_bytes: 4,
                max_decoded_bytes: 2,
            },
            None,
        )
        .unwrap_err();
    assert_eq!(e.archive, file);
    assert!(matches!(
        e.cause.as_ref(),
        ExtractionCause::Mpq(Error::Payload(PayloadError::UnsupportedCompression {
            mask: 1
        }))
    ));
    assert!(matches!(
        a.extract(member),
        Err(Error::Payload(PayloadError::Huffman(
            pcx_mpq::huffman::HuffmanError::Source { .. }
        )))
    ));
    let mut a = MpqArchive::open(&file).unwrap();
    fs::OpenOptions::new()
        .write(true)
        .open(&file)
        .unwrap()
        .set_len(42)
        .unwrap();
    let e = a
        .extract_bounded(
            member,
            ExtractionBudget {
                max_raw_packed_bytes: 4,
                max_decoded_bytes: 2,
            },
            None,
        )
        .unwrap_err();
    assert!(matches!(
        e.cause.as_ref(),
        ExtractionCause::Mpq(Error::Header(
            pcx_mpq::HeaderParseError::SizeMismatch { .. }
        ))
    ));
}

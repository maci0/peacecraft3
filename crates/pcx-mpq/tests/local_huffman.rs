//! Original audio stays local. Expected bytes are Storm's own archive-open output, dumped by the
//! native harness (tests/native/storm_compare.c) into a local directory, never committed.
use pcx_mpq::MpqArchive;
use std::path::{Path, PathBuf};

/// Native dump file name: the member path with separators replaced by underscores.
fn native_dump(directory: &Path, member: &str) -> PathBuf {
    directory.join(member.replace(['\\', '/'], "_"))
}

#[test]
#[ignore = "requires PEACECRAFT3_FORMAT_MPQ and PEACECRAFT3_STORM_NATIVE_DIR (native harness dump)"]
fn original_wav_members_match_storm_archive_open_bytes() {
    let archive = std::env::var_os("PEACECRAFT3_FORMAT_MPQ").expect("set original archive path");
    let directory = std::env::var_os("PEACECRAFT3_STORM_NATIVE_DIR")
        .map(PathBuf::from)
        .expect("set native dump directory");
    let mut archive = MpqArchive::open(archive).expect("open original archive");
    let listfile = archive.extract("(listfile)").expect("read listfile");
    let members: Vec<String> = String::from_utf8_lossy(&listfile)
        .lines()
        .filter(|member| member.to_ascii_lowercase().ends_with(".wav"))
        .map(str::to_string)
        .collect();
    let mut compared = 0;
    for member in &members {
        let dump = native_dump(&directory, member);
        if !dump.is_file() {
            continue;
        }
        let expected = std::fs::read(&dump).expect("read native dump");
        let actual = archive.extract(member).expect("decode original WAV");
        assert!(
            actual == expected,
            "{member}: differs from Storm archive-open bytes"
        );
        compared += 1;
    }
    assert!(compared > 0, "no native dumps matched the archive listfile");
    eprintln!("{compared} WAV members match Storm archive-open bytes");
}

/// Structure-only check that needs no native dump: RIFF chunk walk and decoded length.
#[test]
#[ignore = "requires PEACECRAFT3_FORMAT_MPQ pointing at the original Legacy War3.mpq"]
fn original_huffman_adpcm_wav_members_have_consistent_riff_structure() {
    let archive = std::env::var_os("PEACECRAFT3_FORMAT_MPQ").expect("set original archive path");
    let mut archive = MpqArchive::open(archive).expect("open original archive");
    for member in [
        "Sound\\Interface\\BigButtonClick.wav",
        "Sound\\Interface\\RightGlueScreenPopDown.wav",
    ] {
        let size = archive
            .find_block(member)
            .expect("member present")
            .file_size as usize;
        let bytes = archive.extract(member).expect("decode Huffman/ADPCM WAV");
        assert_eq!(bytes.len(), size, "{member}: decoded length");
        assert_eq!(&bytes[..4], b"RIFF", "{member}: RIFF tag");
        assert_eq!(&bytes[8..12], b"WAVE", "{member}: WAVE tag");
        let riff = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        assert_eq!(riff + 8, bytes.len(), "{member}: RIFF size");
        let (mut at, mut chunks) = (12, Vec::new());
        while at < bytes.len() {
            let id = &bytes[at..at + 4];
            let size = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize;
            chunks.push(id.to_vec());
            at += 8 + size + (size & 1);
        }
        assert_eq!(at, bytes.len(), "{member}: chunks end at the RIFF size");
        assert_eq!(
            chunks.first().map(Vec::as_slice),
            Some(&b"fmt "[..]),
            "{member}: fmt first"
        );
        assert!(
            chunks.iter().any(|id| id == b"data"),
            "{member}: data chunk"
        );
    }
}

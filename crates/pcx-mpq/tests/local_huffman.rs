//! Original audio stays local; the reference is produced by Storm's sector export #552.
use pcx_mpq::MpqArchive;

#[test]
#[ignore = "requires PEACECRAFT3_FORMAT_MPQ, PEACECRAFT3_STORM_AUDIO_MEMBER and PEACECRAFT3_STORM_AUDIO_OUTPUT"]
fn original_audio_matches_storm_sectors() {
    let archive = std::env::var_os("PEACECRAFT3_FORMAT_MPQ").expect("set original archive path");
    let member =
        std::env::var("PEACECRAFT3_STORM_AUDIO_MEMBER").expect("set original audio member");
    let reference =
        std::env::var_os("PEACECRAFT3_STORM_AUDIO_OUTPUT").expect("set Storm #552 output path");
    let expected = std::fs::read(reference).expect("read local native reference");
    let mut archive = MpqArchive::open(archive).expect("open original archive");
    if let Some(source) = std::env::var_os("PEACECRAFT3_STORM_DLL") {
        let bytes = std::fs::read(&source).expect("read explicit local Storm.dll");
        archive
            .set_huffman_table(&bytes, &source)
            .expect("import local Huffman table");
        eprintln!("explicit decoder source={source:?}");
    }
    let actual = archive
        .extract(&member)
        .expect("decode original Huffman/ADPCM audio");
    assert_eq!(actual, expected, "native sector output differs: {member}");
    eprintln!("{member}: {} bytes match Storm #552 output", actual.len());
}

/// Huffman+ADPCM WAV members (mask 0x41/0x81) decode through the archive directory's
/// codec source with no explicit table. Structure only: the RIFF chunk walk must end at the
/// RIFF size and the decoded length must equal the block size. Sample-level fidelity is not asserted here (see format-mpq.md).
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
        eprintln!("{member}: {} bytes, RIFF structure consistent", bytes.len());
    }
}

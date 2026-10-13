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

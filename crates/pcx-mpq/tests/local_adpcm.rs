//! Synthetic codec streams compared with local Storm.dll outputs; no game assets committed.
use pcx_mpq::payload::decompress_sector;

#[test]
#[ignore = "requires PEACECRAFT3_ADPCM_ORACLE with .packed sectors and matching .storm outputs"]
fn adpcm_matches_original_storm_output() {
    let directory = std::env::var_os("PEACECRAFT3_ADPCM_ORACLE")
        .expect("set PEACECRAFT3_ADPCM_ORACLE to the local oracle corpus");
    let mut paths: Vec<_> = std::fs::read_dir(&directory)
        .expect("read oracle directory")
        .map(|entry| entry.expect("oracle directory entry").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "packed")
        })
        .collect();
    paths.sort();
    assert!(
        !paths.is_empty(),
        "oracle directory contains no .packed sectors"
    );
    for path in &paths {
        let packed = std::fs::read(path).expect("read synthetic sector");
        let expected =
            std::fs::read(path.with_extension("storm")).expect("read native Storm output");
        let mut actual = vec![0; expected.len()];
        let produced = decompress_sector(&packed, &mut actual).expect("decode synthetic sector");
        assert_eq!(
            produced,
            expected.len(),
            "{}: output length",
            path.display()
        );
        assert_eq!(actual, expected, "{}: decoded bytes", path.display());
    }
    eprintln!(
        "{} synthetic sectors match original Storm output",
        paths.len()
    );
}

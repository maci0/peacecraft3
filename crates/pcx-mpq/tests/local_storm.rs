//! Full-file equality against the local statically imported original Storm.dll.
use std::collections::BTreeMap;

#[test]
#[ignore = "requires PEACECRAFT3_STORM_COMPARATOR, PEACECRAFT3_STORM_COMPARE_ARCHIVE and PEACECRAFT3_STORM_COMPARE_STREAM; Wine and local original assets"]
fn original_archive_files_match_storm() {
    let executable =
        std::env::var_os("PEACECRAFT3_STORM_COMPARATOR").expect("set built native comparator");
    let archive =
        std::env::var_os("PEACECRAFT3_STORM_COMPARE_ARCHIVE").expect("set Wine archive path");
    let stream = std::env::var_os("PEACECRAFT3_STORM_COMPARE_STREAM")
        .expect("set Wine comparison stream path");
    let result = std::process::Command::new("wine")
        .arg(executable)
        .arg(archive)
        .arg(stream)
        .env("WINEDEBUG", "-all")
        .output()
        .expect("run local Wine oracle");
    let stdout = String::from_utf8(result.stdout).expect("native result UTF-8");
    eprint!("{stdout}");
    assert!(
        result.status.success(),
        "native comparison failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let lines: Vec<BTreeMap<_, _>> = stdout
        .lines()
        .map(|line| {
            line.split_whitespace()
                .map(|field| {
                    let (key, value) = field.split_once('=').expect("native counter");
                    (key, value.parse::<u64>().expect("native numeric counter"))
                })
                .collect()
        })
        .collect();
    assert_eq!(lines.len(), 2, "native summary lines");
    let payload = &lines[0];
    let archive = &lines[1];
    let files = payload["payload_files"];
    assert!(files > 0, "no files compared");
    assert_eq!(payload["identical"], files);
    assert_eq!(archive["archive_files_opened"], files);
    assert_eq!(archive["identical"], files);
    for (key, value) in payload.iter().chain(archive) {
        if key.starts_with("refused_") || *key == "mismatched" {
            assert_eq!(*value, 0, "{key}");
        }
    }
}
